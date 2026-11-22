//! Allow-list sanitiser for SVG files.
//!
//! The viewer never shows the original file as a picture. The file is parsed with a small
//! XML reader, and only elements, attributes and values from fixed lists are written to a new
//! document. Scripts, `foreignObject`, animation, `style` elements, event handlers, outside
//! links, `data:` links other than inline raster images, entities and DOCTYPE subsets do not
//! get through. Anything that is not well-formed XML is refused as a whole.

use crate::error::{Error, Result};

/// Largest file the sanitiser reads.
pub const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_EVENTS: usize = 200_000;
const MAX_ATTRS: usize = 64;
const MAX_REPORTED: usize = 20;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "symbol",
    "use",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "textPath",
    "title",
    "desc",
    "linearGradient",
    "radialGradient",
    "stop",
    "pattern",
    "clipPath",
    "mask",
    "marker",
    "image",
    "filter",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
];

const TEXT_ELEMENTS: &[&str] = &["text", "tspan", "textPath", "title", "desc"];
const HREF_ELEMENTS: &[&str] = &[
    "use",
    "image",
    "textPath",
    "linearGradient",
    "radialGradient",
    "pattern",
    "filter",
];

const ATTRIBUTES: &[&str] = &[
    "id",
    "class",
    "x",
    "y",
    "width",
    "height",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "x1",
    "y1",
    "x2",
    "y2",
    "fx",
    "fy",
    "d",
    "points",
    "transform",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-opacity",
    "fill-opacity",
    "fill-rule",
    "clip-rule",
    "opacity",
    "viewBox",
    "preserveAspectRatio",
    "version",
    "offset",
    "stop-color",
    "stop-opacity",
    "gradientUnits",
    "gradientTransform",
    "spreadMethod",
    "patternUnits",
    "patternContentUnits",
    "patternTransform",
    "clip-path",
    "mask",
    "marker-start",
    "marker-mid",
    "marker-end",
    "filter",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "text-anchor",
    "text-decoration",
    "letter-spacing",
    "word-spacing",
    "dominant-baseline",
    "alignment-baseline",
    "baseline-shift",
    "dx",
    "dy",
    "rotate",
    "textLength",
    "lengthAdjust",
    "startOffset",
    "markerWidth",
    "markerHeight",
    "refX",
    "refY",
    "orient",
    "markerUnits",
    "maskUnits",
    "maskContentUnits",
    "clipPathUnits",
    "filterUnits",
    "primitiveUnits",
    "in",
    "in2",
    "result",
    "stdDeviation",
    "mode",
    "type",
    "values",
    "operator",
    "k1",
    "k2",
    "k3",
    "k4",
    "flood-color",
    "flood-opacity",
    "tableValues",
    "slope",
    "intercept",
    "amplitude",
    "exponent",
    "radius",
    "display",
    "visibility",
    "overflow",
    "color",
    "vector-effect",
    "shape-rendering",
    "text-rendering",
    "image-rendering",
    "color-interpolation-filters",
    "style",
];

/// Result of a successful sanitising.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    /// The clean document.
    pub svg: String,
    /// How many elements, attributes and values were removed.
    pub removed: usize,
    /// Names of the first removed things (`<script>`, `@onload`, ...), without repeats.
    pub removed_names: Vec<String>,
}

#[derive(Debug)]
enum Event {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        empty: bool,
    },
    End(String),
    Text(String),
}

/// Reads `data` (UTF-8 or UTF-16 text) and returns the sanitised document.
///
/// # Errors
/// Fails for files that are too large, not well-formed XML, use entities or a DOCTYPE
/// subset, nest too deeply, or whose root element is not `svg`.
pub fn sanitize(data: &[u8]) -> Result<Sanitized> {
    if data.len() > MAX_SVG_BYTES {
        return Err(Error::limit("SVG file is too large"));
    }
    let text = crate::preview::decode_text(data);
    let events = parse(&text)?;
    build(events)
}

fn decode_entities(s: &str) -> Result<String> {
    if !s.contains('&') {
        return Ok(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let end = rest
            .find(';')
            .filter(|e| *e <= 10)
            .ok_or_else(|| Error::format("SVG: unknown or broken entity"))?;
        let ent = &rest[..end];
        let ch = match ent {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| ent.strip_prefix('#').map(|d| d.parse::<u32>()))
                .and_then(std::result::Result::ok)
                .and_then(char::from_u32)
                .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
                .ok_or_else(|| Error::format("SVG: unknown or broken entity"))?,
        };
        out.push(ch);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn parse(s: &str) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    let mut rest = s;
    let cut = |r: &str, pat: &str| -> Result<usize> {
        r.find(pat)
            .map(|i| i + pat.len())
            .ok_or_else(|| Error::format("SVG: unterminated markup"))
    };
    while !rest.is_empty() {
        if events.len() > MAX_EVENTS {
            return Err(Error::limit("SVG has too many elements"));
        }
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = &r[cut(r, "-->")?..];
        } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r
                .find("]]>")
                .ok_or_else(|| Error::format("SVG: unterminated CDATA"))?;
            events.push(Event::Text(r[..end].to_string()));
            rest = &r[end + 3..];
        } else if let Some(r) = rest.strip_prefix("<?") {
            rest = &r[cut(r, "?>")?..];
        } else if let Some(r) = rest.strip_prefix("<!") {
            let end = r
                .find('>')
                .ok_or_else(|| Error::format("SVG: unterminated DOCTYPE"))?;
            let decl = &r[..end];
            if !decl.to_ascii_uppercase().starts_with("DOCTYPE") || decl.contains('[') {
                return Err(Error::format(
                    "SVG: DOCTYPE with a subset or entities is refused",
                ));
            }
            rest = &r[end + 1..];
        } else if let Some(r) = rest.strip_prefix("</") {
            let end = r
                .find('>')
                .ok_or_else(|| Error::format("SVG: unterminated end tag"))?;
            events.push(Event::End(r[..end].trim().to_string()));
            rest = &r[end + 1..];
        } else if let Some(r) = rest.strip_prefix('<') {
            let (event, used) = parse_start(r)?;
            events.push(event);
            rest = &r[used..];
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            events.push(Event::Text(decode_entities(&rest[..end])?));
            rest = &rest[end..];
        }
    }
    Ok(events)
}

fn parse_start(r: &str) -> Result<(Event, usize)> {
    let bytes = r.as_bytes();
    let mut i = 0;
    let is_name_end = |b: u8| b.is_ascii_whitespace() || matches!(b, b'/' | b'>' | b'=');
    while i < bytes.len() && !is_name_end(bytes[i]) {
        i += 1;
    }
    let name = &r[..i];
    if name.is_empty() {
        return Err(Error::format("SVG: empty tag name"));
    }
    let mut attrs: Vec<(String, String)> = Vec::new();
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => return Err(Error::format("SVG: unterminated tag")),
            Some(b'>') => {
                return Ok((
                    Event::Start {
                        name: name.into(),
                        attrs,
                        empty: false,
                    },
                    i + 1,
                ));
            }
            Some(b'/') => {
                if bytes.get(i + 1) == Some(&b'>') {
                    return Ok((
                        Event::Start {
                            name: name.into(),
                            attrs,
                            empty: true,
                        },
                        i + 2,
                    ));
                }
                return Err(Error::format("SVG: stray '/' in a tag"));
            }
            _ => {}
        }
        let start = i;
        while i < bytes.len() && !is_name_end(bytes[i]) {
            i += 1;
        }
        let key = &r[start..i];
        if key.is_empty() {
            return Err(Error::format("SVG: broken attribute"));
        }
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            return Err(Error::format("SVG: attribute without a value"));
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = match bytes.get(i) {
            Some(q @ (b'"' | b'\'')) => *q,
            _ => return Err(Error::format("SVG: attribute value is not quoted")),
        };
        i += 1;
        let vstart = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(Error::format("SVG: unterminated attribute value"));
        }
        let value = decode_entities(&r[vstart..i])?;
        i += 1;
        if attrs.iter().any(|(k, _)| k == key) {
            return Err(Error::format("SVG: repeated attribute"));
        }
        if attrs.len() >= MAX_ATTRS {
            return Err(Error::limit("SVG element has too many attributes"));
        }
        attrs.push((key.to_string(), value));
    }
}

fn escape(s: &str, attr: bool) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' if attr => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// A value is refused when it could pull in something from outside or run code.
fn value_is_safe(value: &str) -> bool {
    let flat: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    if flat.contains('\\')
        || flat.contains("javascript:")
        || flat.contains("vbscript:")
        || flat.contains("expression(")
        || flat.contains("@import")
        || flat.contains("data:")
        || flat.contains("behavior")
        || flat.contains("-moz-binding")
    {
        return false;
    }
    let mut rest = flat.as_str();
    while let Some(i) = rest.find("url(") {
        rest = &rest[i + 4..];
        let r = rest.trim_start_matches(['\'', '"']);
        if !r.starts_with('#') {
            return false;
        }
    }
    true
}

fn is_inline_image(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("data:image/") else {
        return false;
    };
    let Some((kind, b64)) = rest.split_once(";base64,") else {
        return false;
    };
    matches!(kind, "png" | "jpeg" | "gif" | "webp")
        && !b64.is_empty()
        && b64
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'\n' | b'\r'))
}

struct Report {
    count: usize,
    names: Vec<String>,
}

impl Report {
    fn note(&mut self, name: String) {
        self.count += 1;
        if self.names.len() < MAX_REPORTED && !self.names.contains(&name) {
            self.names.push(name);
        }
    }
}

fn clean_attrs(
    element: &str,
    attrs: Vec<(String, String)>,
    rep: &mut Report,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (key, value) in attrs {
        let keep = if key == "xmlns" || key == "xmlns:xlink" {
            false // written again on the root
        } else if key == "href" || key == "xlink:href" {
            HREF_ELEMENTS.contains(&element)
                && (value.starts_with('#') && value_is_safe(&value)
                    || element == "image" && is_inline_image(&value))
        } else if key == "xml:space" {
            value == "preserve" || value == "default"
        } else if ATTRIBUTES.contains(&key.as_str()) {
            value_is_safe(&value)
        } else {
            false
        };
        if keep {
            out.push((key, value));
        } else if !key.starts_with("xmlns") {
            rep.note(format!("@{key}"));
        }
    }
    out
}

fn build(events: Vec<Event>) -> Result<Sanitized> {
    let mut out = String::new();
    let mut rep = Report {
        count: 0,
        names: Vec::new(),
    };
    // (name, kept)
    let mut stack: Vec<(String, bool)> = Vec::new();
    let mut skip = 0usize; // depth inside a removed element
    let mut roots = 0usize;
    for ev in events {
        match ev {
            Event::Start { name, attrs, empty } => {
                if stack.is_empty() && skip == 0 {
                    roots += 1;
                    if roots > 1 || name != "svg" {
                        return Err(Error::format("SVG: the root element is not <svg>"));
                    }
                }
                if skip > 0 {
                    if !empty {
                        skip += 1;
                    }
                    continue;
                }
                if !ELEMENTS.contains(&name.as_str()) {
                    rep.note(format!("<{name}>"));
                    if !empty {
                        skip = 1;
                    }
                    continue;
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(Error::limit("SVG is nested too deeply"));
                }
                let mut attrs = clean_attrs(&name, attrs, &mut rep);
                if name == "svg" && stack.is_empty() {
                    attrs.insert(0, ("xmlns:xlink".into(), XLINK_NS.into()));
                    attrs.insert(0, ("xmlns".into(), SVG_NS.into()));
                }
                out.push('<');
                out.push_str(&name);
                for (k, v) in &attrs {
                    out.push(' ');
                    out.push_str(k);
                    out.push_str("=\"");
                    out.push_str(&escape(v, true));
                    out.push('"');
                }
                if empty {
                    out.push_str("/>");
                } else {
                    out.push('>');
                    stack.push((name, true));
                }
            }
            Event::End(name) => {
                if skip > 0 {
                    skip -= 1;
                    continue;
                }
                match stack.pop() {
                    Some((open, _)) if open == name => {
                        out.push_str("</");
                        out.push_str(&name);
                        out.push('>');
                    }
                    _ => return Err(Error::format("SVG: tags are not nested correctly")),
                }
            }
            Event::Text(t) => {
                if skip > 0 {
                    continue;
                }
                match stack.last() {
                    Some((name, _)) if TEXT_ELEMENTS.contains(&name.as_str()) => {
                        out.push_str(&escape(&t, false));
                    }
                    _ => {}
                }
            }
        }
    }
    if roots == 0 || !stack.is_empty() || skip > 0 {
        return Err(Error::format("SVG: the document is incomplete"));
    }
    Ok(Sanitized {
        svg: out,
        removed: rep.count,
        removed_names: rep.names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> Sanitized {
        sanitize(s.as_bytes()).expect("sanitize")
    }

    const CLEAN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" width="20"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/></linearGradient></defs><rect x="1" y="1" width="8" height="8" fill="url(#g)" style="stroke:#000;stroke-width:1"/><text x="1" y="9">a &lt; b</text><use href="#g"/></svg>"##;

    #[test]
    fn clean_file_keeps_shapes_and_text() {
        let r = ok(CLEAN);
        assert_eq!(r.removed, 0, "{:?}", r.removed_names);
        assert!(r.svg.contains("<rect x=\"1\""));
        assert!(r.svg.contains("fill=\"url(#g)\""));
        assert!(r.svg.contains("a &lt; b"));
        assert!(r.svg.contains("href=\"#g\""));
        assert!(
            r.svg
                .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=")
        );
    }

    #[test]
    fn namespace_is_always_written_on_the_root() {
        let r = ok("<svg><g/></svg>");
        assert!(r.svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        let r = ok("<svg xmlns=\"http://evil.example/\"/>");
        assert!(!r.svg.contains("evil"));
    }

    #[test]
    fn scripts_handlers_and_foreign_content_are_removed() {
        let r = ok(
            r#"<svg onload="x()"><script>alert(1)</script><g onclick="y()"><foreignObject><div>hi</div></foreignObject><circle r="1"/></g></svg>"#,
        );
        assert!(!r.svg.contains("script"));
        assert!(!r.svg.contains("alert"));
        assert!(!r.svg.contains("onload"));
        assert!(!r.svg.contains("onclick"));
        assert!(!r.svg.contains("foreignObject"));
        assert!(!r.svg.contains("div"));
        assert!(r.svg.contains("<circle r=\"1\"/>"));
        assert_eq!(r.removed, 4);
        assert!(r.removed_names.contains(&"<script>".to_string()));
        assert!(r.removed_names.contains(&"@onload".to_string()));
    }

    #[test]
    fn animation_style_and_link_elements_are_removed() {
        let r = ok(
            r##"<svg><a href="https://e.org"><rect/></a><style>@import url(http://e.org/x.css);</style><animate attributeName="href" values="javascript:x"/><set attributeName="href" to="javascript:x"/><iframe src="x"/><rect id="ok"/></svg>"##,
        );
        for bad in [
            "<a",
            "style",
            "animate",
            "<set",
            "iframe",
            "e.org",
            "javascript",
        ] {
            assert!(!r.svg.contains(bad), "{bad} in {}", r.svg);
        }
        assert!(r.svg.contains("id=\"ok\""));
    }

    #[test]
    fn links_are_limited_to_fragments_and_inline_images() {
        let r = ok(
            r##"<svg><use href="https://e.org/a.svg#x"/><use xlink:href="javascript:alert(1)"/><use href="#a"/><image href="http://e.org/a.png"/><image href="data:text/html;base64,AAAA"/><image href="data:image/svg+xml;base64,AAAA"/><image href="data:image/png;base64,iVBORw0KGgo="/><rect href="#a"/></svg>"##,
        );
        assert!(!r.svg.contains("e.org"));
        assert!(!r.svg.contains("javascript"));
        assert!(!r.svg.contains("text/html"));
        assert!(!r.svg.contains("svg+xml"));
        assert!(r.svg.contains("data:image/png;base64,iVBORw0KGgo="));
        assert_eq!(r.svg.matches("href=").count(), 2);
    }

    #[test]
    fn urls_in_values_must_point_at_fragments() {
        let r = ok(
            r##"<svg><rect fill="url(http://e.org/p)" stroke="url('#s')" style="fill:url(//e.org/x)"/><rect style="background:url( #ok )"/><rect style="x:expression(alert(1))"/><rect style="c:\75rl(x)"/><rect fill="u&#82;l(x)" filter="url(&quot;#f&quot;)"/></svg>"##,
        );
        assert!(!r.svg.contains("e.org"));
        assert!(!r.svg.contains("expression"));
        assert!(!r.svg.contains("\\"));
        assert!(r.svg.contains("stroke=\"url('#s')\""));
        assert!(r.svg.contains("filter=\"url(&quot;#f&quot;)\""));
    }

    #[test]
    fn unknown_attributes_and_prefixed_names_are_removed() {
        let r = ok(r#"<svg><rect data-x="1" xlink:role="a" foo:bar="b" width="3"/></svg>"#);
        assert!(r.svg.contains("width=\"3\""));
        assert!(!r.svg.contains("data-x"));
        assert!(!r.svg.contains("role"));
        assert!(!r.svg.contains("foo"));
    }

    #[test]
    fn comments_processing_instructions_and_plain_doctype_are_dropped() {
        let r = ok(
            "<?xml version=\"1.0\"?><!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\"><!-- hi --><svg><!-- c --><g/></svg>",
        );
        assert!(!r.svg.contains("xml version"));
        assert!(!r.svg.contains("hi"));
        assert!(r.svg.contains("<g/>"));
    }

    #[test]
    fn cdata_and_text_are_escaped_on_output() {
        let r = ok("<svg><text><![CDATA[<b>&x</b>]]></text></svg>");
        assert!(r.svg.contains("&lt;b&gt;&amp;x&lt;/b&gt;"));
        assert!(!r.svg.contains("<b>"));
    }

    #[test]
    fn text_outside_text_elements_is_dropped() {
        let r = ok("<svg>stray<g>more<title>t</title></g></svg>");
        assert!(!r.svg.contains("stray"));
        assert!(!r.svg.contains("more"));
        assert!(r.svg.contains("<title>t</title>"));
    }

    #[test]
    fn entities_and_doctype_subsets_are_refused() {
        let bomb = "<!DOCTYPE svg [<!ENTITY a \"aaaa\"><!ENTITY b \"&a;&a;&a;\">]><svg><text>&b;</text></svg>";
        assert!(sanitize(bomb.as_bytes()).is_err());
        assert!(sanitize(b"<svg><text>&nbsp;</text></svg>").is_err());
        assert!(sanitize(b"<svg><text>&#0;</text></svg>").is_err());
        assert!(sanitize(b"<svg><text>&amp</text></svg>").is_err());
        let r = ok("<svg><text>&#65;&#x42;&amp;&apos;</text></svg>");
        assert!(r.svg.contains("AB&amp;'"));
    }

    #[test]
    fn malformed_documents_are_refused() {
        for bad in [
            "",
            "plain text",
            "<svg>",
            "<svg></g>",
            "<svg><g></svg></g>",
            "<svg a=1/>",
            "<svg a=\"1\" a=\"2\"/>",
            "<svg a/>",
            "<svg a=\"1/>",
            "<svg><!-- no end</svg>",
            "<svg><![CDATA[ no end</svg>",
            "<html><svg/></html>",
            "<svg/><svg/>",
            "<svg><g></svg>",
        ] {
            assert!(sanitize(bad.as_bytes()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn limits_are_enforced() {
        let deep = format!("<svg>{}{}</svg>", "<g>".repeat(200), "</g>".repeat(200));
        assert!(sanitize(deep.as_bytes()).is_err());
        let many = format!("<svg>{}</svg>", "<g/>".repeat(MAX_EVENTS + 10));
        assert!(sanitize(many.as_bytes()).is_err());
        let attrs: String = (0..80).map(|i| format!(" a{i}=\"1\"")).collect();
        assert!(sanitize(format!("<svg{attrs}/>").as_bytes()).is_err());
        assert!(sanitize(&vec![b' '; MAX_SVG_BYTES + 1]).is_err());
    }

    #[test]
    fn removed_names_are_reported_once() {
        let r = ok("<svg><script/><script/><script/></svg>");
        assert_eq!(r.removed, 3);
        assert_eq!(r.removed_names, vec!["<script>".to_string()]);
    }

    #[test]
    fn utf16_files_are_read() {
        let mut bytes = vec![0xff, 0xfe];
        for u in "<svg><title>я</title></svg>".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert!(sanitize(&bytes).expect("utf16").svg.contains("я"));
    }

    #[test]
    fn every_cut_and_damaged_file_is_handled_without_panic() {
        for cut in 0..CLEAN.len() {
            let _ = sanitize(&CLEAN.as_bytes()[..cut]);
        }
        let mut seed = 3u32;
        for _ in 0..300 {
            let mut d = CLEAN.as_bytes().to_vec();
            for _ in 0..4 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let i = (seed >> 8) as usize % d.len();
                d[i] = (seed >> 24) as u8;
            }
            let _ = sanitize(&d);
        }
    }

    #[test]
    fn output_never_contains_active_content() {
        let nasty = r##"<svg onload="a()"><script>1</script><use href="javascript:1"/><image href="http://e"/><foreignObject/><rect style="fill:url(http://e)" onclick="1" fill="url(javascript:1)"/></svg>"##;
        let r = ok(nasty);
        let low = r.svg.to_ascii_lowercase();
        for bad in [
            "<script",
            "javascript",
            "http://e",
            "onload",
            "onclick",
            "foreignobject",
        ] {
            assert!(!low.contains(bad), "{bad}");
        }
    }
}
