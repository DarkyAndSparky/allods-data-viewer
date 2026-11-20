//! Resource descriptions in `.xdb` files (plain XML).
//!
//! Only what is needed to convert textures is read: the flat texture description. This
//! is not a general XML parser; it looks for simple `<tag>value</tag>` pairs and never
//! follows references.

use crate::error::{Error, Result};
use crate::texture::Dxt;

/// Largest `.xdb` text we accept.
pub const MAX_XDB_BYTES: usize = 1 << 20;

/// Returns the text between `<name>` and `</name>` (first occurrence), trimmed.
#[must_use]
pub fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

/// Texture parameters stored in a `(UITexture).xdb` or `(Texture).xdb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureDesc {
    /// Width of mip level 0 in the file.
    pub width: u32,
    /// Height of mip level 0 in the file.
    pub height: u32,
    /// Width of the used picture inside the (power of two) texture.
    pub real_width: u32,
    /// Height of the used picture inside the texture.
    pub real_height: u32,
    /// Block format.
    pub format: Dxt,
    /// Number of mip levels stated by the description.
    pub mips: u32,
}

fn number(xml: &str, name: &str) -> Result<u32> {
    tag(xml, name)
        .ok_or_else(|| Error::format(format!("missing <{name}>")))?
        .parse()
        .map_err(|_| Error::format(format!("<{name}> is not a number")))
}

impl TextureDesc {
    /// Parses a texture description.
    ///
    /// # Errors
    ///
    /// [`Error::Format`] when a field is missing or malformed, [`Error::Unsupported`] for a
    /// format other than DXT1, DXT3 or DXT5, [`Error::Limit`] for text above 1 MiB or
    /// dimensions above 16384.
    pub fn parse(xml: &str) -> Result<Self> {
        if xml.len() > MAX_XDB_BYTES {
            return Err(Error::limit("xdb file too large"));
        }
        let format = match tag(xml, "type") {
            Some("DXT1") => Dxt::Dxt1,
            Some("DXT3") => Dxt::Dxt3,
            Some("DXT5") => Dxt::Dxt5,
            Some(other) => return Err(Error::unsupported(format!("texture type {other}"))),
            None => return Err(Error::format("missing <type>")),
        };
        let width = number(xml, "width")?;
        let height = number(xml, "height")?;
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(Error::limit(format!("unsupported size {width}x{height}")));
        }
        let real_width = number(xml, "realWidth").unwrap_or(width);
        let real_height = number(xml, "realHeight").unwrap_or(height);
        let mips = number(xml, "mipsNumber").unwrap_or(1);
        Ok(Self {
            width,
            height,
            real_width: real_width.min(width),
            real_height: real_height.min(height),
            format,
            mips,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<?xml version=\"1.0\"?>\r\n<UITexture>\r\n  <mipSW>-1</mipSW>\r\n  <mipsNumber>1</mipsNumber>\r\n  <type>DXT5</type>\r\n  <width>512</width>\r\n  <height>256</height>\r\n  <realWidth>386</realWidth>\r\n  <realHeight>200</realHeight>\r\n</UITexture>";

    #[test]
    fn parses_a_description() {
        let d = TextureDesc::parse(SAMPLE).expect("parse");
        assert_eq!(
            (d.width, d.height, d.real_width, d.real_height),
            (512, 256, 386, 200)
        );
        assert_eq!((d.format, d.mips), (Dxt::Dxt5, 1));
    }

    #[test]
    fn real_size_defaults_and_is_clamped() {
        let x = "<type>DXT1</type><width>64</width><height>64</height><realWidth>999</realWidth>";
        let d = TextureDesc::parse(x).expect("parse");
        assert_eq!((d.real_width, d.real_height, d.mips), (64, 64, 1));
    }

    #[test]
    fn bad_descriptions_fail() {
        assert!(TextureDesc::parse("<width>4</width>").is_err());
        assert!(
            TextureDesc::parse("<type>ARGB8</type><width>4</width><height>4</height>").is_err()
        );
        assert!(TextureDesc::parse("<type>DXT1</type><width>x</width><height>4</height>").is_err());
        assert!(TextureDesc::parse("<type>DXT1</type><width>0</width><height>4</height>").is_err());
        assert!(
            TextureDesc::parse("<type>DXT1</type><width>99999</width><height>4</height>").is_err()
        );
        assert_eq!(tag("<a> x </a>", "a"), Some("x"));
        assert_eq!(tag("<a>", "a"), None);
    }
}
