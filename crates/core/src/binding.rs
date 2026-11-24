//! Texture binding of model elements in the 7.0 pack (32-bit object image).
//!
//! A Geometry object (`X.(Geometry).xdb`) carries `modelElements`; every element has a LOD list
//! (index and vertex ranges) and a material whose `diffuseTexture` is a pointer to a Texture
//! object. The pointers are resolved with the pointer-fix table (see `docs/formats/pack-fixes.md`
//! and `docs/formats/model-binding.md`). Status: draft, checked on `Warlock` and `ArchdemonMage`.

use std::collections::HashMap;

use crate::db32::Database32;
use crate::error::{Error, Result};
use crate::fixes::PackFix;
use crate::limits::Limits;
use crate::material::FixIndex;

/// Byte size of `ModelElement` (V7_0_00_76).
const ELEMENT_SIZE: u64 = 120;
/// Byte size of one `Lod` entry.
const LOD_SIZE: u64 = 20;
/// Geometry head: `modelElements` array triple (begin at +228, byte size at +232).
const ELEMENTS_AT: u64 = 228;
/// Element: `lods` array triple.
const LODS_AT: u64 = 20;
/// Element: `material.diffuseTexture` pointer (`material` at +36, `diffuseTexture` at +8).
const DIFFUSE_AT: u64 = 44;
/// Element: `material.params` pointer to a `CommonMaterialParams` record (`material` at +36, `params` at +16).
const PARAMS_AT: u64 = 52;
/// More elements than this in one model are treated as corrupted input.
const MAX_ELEMENTS: u64 = 4096;

/// One model element: its index and vertex range in LOD 0 and the diffuse texture resource.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementBinding {
    /// First index of the element in the index buffer.
    pub index_begin: u32,
    /// One past the last index.
    pub index_end: u32,
    /// First vertex of the element in the vertex buffer.
    pub vertex_begin: u32,
    /// One past the last vertex.
    pub vertex_end: u32,
    /// Lower-case resource name of the diffuse texture (`…(Texture).xdb`), when the element has one.
    pub diffuse: Option<String>,
    /// Further material textures and scalars of the element, when its `params` record is readable.
    pub params: Option<ElementParams>,
}

/// Material parameters of one element (`CommonMaterialParams`, see `docs/formats/common-material.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementParams {
    /// Bump (normal) texture resource name.
    pub bump: Option<String>,
    /// Effect mask texture resource name.
    pub effect_mask: Option<String>,
    /// Environment reflection texture resource name.
    pub env_reflection: Option<String>,
    /// Environment specular (contour) texture resource name.
    pub env_specular: Option<String>,
    /// Shininess exponent.
    pub shininess: f32,
    /// Specular intensity.
    pub specular_intensity: f32,
}

fn word(payload: &[u8], at: u64) -> Option<u32> {
    let at = usize::try_from(at).ok()?;
    let b = payload.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

/// Reads the element bindings of one Geometry object at data-block offset `root`.
///
/// Returns an empty list when the object has no elements or the fix table does not describe them.
///
/// # Errors
/// Returns an error when the element count is not plausible.
pub fn read_geometry(
    db: &Database32,
    payload: &[u8],
    fixes: &FixIndex,
    names: &HashMap<u64, &str>,
    root: u64,
) -> Result<Vec<ElementBinding>> {
    let data = db.data.start as u64;
    let rd = |off: u64| word(payload, data.checked_add(off)?);
    let (Some(at), Some(bytes)) = (
        root.checked_add(ELEMENTS_AT),
        root.checked_add(ELEMENTS_AT + 4),
    ) else {
        return Ok(Vec::new());
    };
    let (Some(fix), Some(bytes)) = (fixes.get(at), rd(bytes)) else {
        return Ok(Vec::new());
    };
    let bytes = u64::from(bytes);
    if bytes % ELEMENT_SIZE != 0 || bytes / ELEMENT_SIZE > MAX_ELEMENTS {
        return Err(Error::format("model element array size is not plausible"));
    }
    let base = u64::from(fix.value);
    let mut out = Vec::new();
    for i in 0..bytes / ELEMENT_SIZE {
        let Some(e) = i
            .checked_mul(ELEMENT_SIZE)
            .and_then(|o| base.checked_add(o))
        else {
            break;
        };
        let mut b = ElementBinding {
            index_begin: 0,
            index_end: 0,
            vertex_begin: 0,
            vertex_end: 0,
            diffuse: None,
            params: None,
        };
        if let (Some(lods), Some(lod_bytes)) = (fixes.get(e + LODS_AT), rd(e + LODS_AT + 4))
            && u64::from(lod_bytes) >= LOD_SIZE
        {
            let l = u64::from(lods.value);
            if let (Some(a), Some(z), Some(va), Some(vz)) =
                (rd(l + 4), rd(l + 8), rd(l + 12), rd(l + 16))
            {
                (b.index_begin, b.index_end, b.vertex_begin, b.vertex_end) = (a, z, va, vz);
            }
        }
        if let Some(d) = fixes.get(e + DIFFUSE_AT)
            && d.tag == 0
        {
            b.diffuse = names
                .get(&u64::from(d.value))
                .map(|n| n.to_ascii_lowercase());
        }
        if let Some(p) = fixes.get(e + PARAMS_AT)
            && p.tag == 0
            && let Some(block) = payload.get(db.data.clone())
            && let Ok(m) = crate::material::parse_common_material(block, fixes, u64::from(p.value))
        {
            let name = |v: Option<u32>| {
                v.and_then(|id| names.get(&u64::from(id)))
                    .map(|n| n.to_ascii_lowercase())
            };
            b.params = Some(ElementParams {
                bump: name(m.bump_texture),
                effect_mask: name(m.effect_mask_texture),
                env_reflection: name(m.env_reflection_texture),
                env_specular: name(m.env_specular_texture),
                shininess: m.shininess,
                specular_intensity: m.specular_intensity,
            });
        }
        out.push(b);
    }
    Ok(out)
}

/// Element bindings of every Geometry resource of the pack, keyed by lower-case resource name
/// (`…(Geometry).xdb`). Geometry objects that cannot be read are skipped.
///
/// # Errors
/// Returns an error when the pointer-fix table is larger than the limit.
pub fn all_geometries(
    db: &Database32,
    payload: &[u8],
    fixes: &[PackFix],
    limits: &Limits,
) -> Result<HashMap<String, Vec<ElementBinding>>> {
    if fixes.len() as u64 > limits.max_fixes {
        return Err(Error::limit("pack fix count exceeds the limit"));
    }
    let index = FixIndex::new(fixes);
    let names: HashMap<u64, &str> = db
        .files
        .iter()
        .map(|f| (f.db_id, f.name.as_str()))
        .collect();
    let mut out = HashMap::new();
    for f in &db.files {
        if !f.name.ends_with("(Geometry).xdb") {
            continue;
        }
        if let Ok(list) = read_geometry(db, payload, &index, &names, f.db_id)
            && !list.is_empty()
        {
            out.insert(f.name.to_ascii_lowercase(), list);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbFile;

    fn put(buf: &mut [u8], at: usize, v: u32) {
        buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Image with a Geometry at 0 (one element at 400, one lod at 600) and a Texture at 800.
    fn synthetic() -> (Database32, Vec<u8>, Vec<PackFix>) {
        let mut d = vec![0u8; 1000];
        put(&mut d, 232, 120);
        put(&mut d, 400 + 24, 20);
        put(&mut d, 600 + 4, 3);
        put(&mut d, 600 + 8, 30);
        put(&mut d, 600 + 12, 1);
        put(&mut d, 600 + 16, 11);
        let fix = |offset, tag, value| PackFix { offset, tag, value };
        let fixes = vec![
            fix(228, 1, 400),
            fix(400 + 20, 1, 600),
            fix(400 + 44, 0, 800),
        ];
        let db = Database32 {
            structs: Vec::new(),
            files: vec![
                DbFile {
                    name: "A/B.(Geometry).xdb".into(),
                    db_id: 0,
                },
                DbFile {
                    name: "A/Skin.(Texture).xdb".into(),
                    db_id: 800,
                },
            ],
            data: 0..1000,
        };
        (db, d, fixes)
    }

    #[test]
    fn element_ranges_and_texture_are_read() {
        let (db, d, fixes) = synthetic();
        let all = all_geometries(&db, &d, &fixes, &Limits::default()).unwrap();
        let g = &all["a/b.(geometry).xdb"];
        assert_eq!(g.len(), 1);
        assert_eq!(
            (
                g[0].index_begin,
                g[0].index_end,
                g[0].vertex_begin,
                g[0].vertex_end
            ),
            (3, 30, 1, 11)
        );
        assert_eq!(g[0].diffuse.as_deref(), Some("a/skin.(texture).xdb"));
    }

    #[test]
    fn element_params_are_read_through_their_pointers() {
        let (db, mut d, mut fixes) = synthetic();
        d.resize(1300, 0);
        // `params` record at 900: effect mask pointer at +40 -> texture at 800, shininess 25.
        let db = Database32 {
            data: 0..1300,
            ..db
        };
        d[900 + 64..900 + 68].copy_from_slice(&25f32.to_le_bytes());
        fixes.push(PackFix {
            offset: 400 + 52,
            tag: 0,
            value: 900,
        });
        fixes.push(PackFix {
            offset: 900 + 40,
            tag: 0,
            value: 800,
        });
        let all = all_geometries(&db, &d, &fixes, &Limits::default()).unwrap();
        let p = all["a/b.(geometry).xdb"][0].params.clone().expect("params");
        assert_eq!(p.effect_mask.as_deref(), Some("a/skin.(texture).xdb"));
        assert_eq!((p.bump, p.shininess), (None, 25.0));
    }

    #[test]
    fn missing_fix_gives_an_element_without_texture() {
        let (db, d, mut fixes) = synthetic();
        fixes.pop();
        let all = all_geometries(&db, &d, &fixes, &Limits::default()).unwrap();
        assert_eq!(all["a/b.(geometry).xdb"][0].diffuse, None);
    }

    #[test]
    fn truncated_image_and_bad_sizes_never_panic() {
        let (db, d, fixes) = synthetic();
        for cut in 0..d.len() {
            let _ = all_geometries(&db, &d[..cut], &fixes, &Limits::default());
        }
        let mut bad = d.clone();
        put(&mut bad, 232, 121);
        assert!(
            all_geometries(&db, &bad, &fixes, &Limits::default())
                .unwrap()
                .is_empty()
        );
        put(&mut bad, 232, 120 * 5000);
        assert!(
            all_geometries(&db, &bad, &fixes, &Limits::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_pointer_outside_the_image_is_ignored() {
        let (db, d, mut fixes) = synthetic();
        fixes[1].value = u32::MAX;
        fixes[2].value = u32::MAX;
        let all = all_geometries(&db, &d, &fixes, &Limits::default()).unwrap();
        let e = &all["a/b.(geometry).xdb"][0];
        assert_eq!((e.index_end, e.diffuse.clone()), (0, None));
    }
}
