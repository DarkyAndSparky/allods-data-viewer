//! Texture binding of model elements in the 32-bit object image (client 7.0 and client 1.1).
//!
//! A Geometry object (`X.(Geometry).xdb`) carries `modelElements`; every element has a LOD list
//! (index and vertex ranges) and a material whose `diffuseTexture` is a pointer to a Texture
//! object. The pointers are resolved with the pointer-fix table (see `docs/formats/pack-fixes.md`
//! and `docs/formats/model-binding.md`). Status: draft. Client 7.0 was checked on `Warlock` and `ArchdemonMage`; the client 1.1
//! offsets come from two independent sources (see the document).

use std::collections::HashMap;

use crate::db32::Database32;
use crate::error::{Error, Result};
use crate::fixes::PackFix;
use crate::limits::Limits;
use crate::material::FixIndex;

/// Byte size of one `Lod` entry (both layouts).
const LOD_SIZE: u64 = 20;

/// Where the fields of a `Geometry` record and of its `ModelElement` entries lie.
///
/// All offsets are in bytes; `lods_at`, `lod_fields` and the material offsets are relative to
/// the element (`lod_fields` to the `Lod` entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementLayout {
    /// Geometry head: begin of the `modelElements` array triple (byte size follows at +4).
    pub elements_at: u64,
    /// Byte size of one `ModelElement`.
    pub element_size: u64,
    /// Element: `lods` array triple.
    pub lods_at: u64,
    /// `Lod`: offsets of index begin, index end, vertex begin, vertex end.
    pub lod_fields: [u64; 4],
    /// Element: `material.diffuseTexture` pointer.
    pub diffuse_at: u64,
    /// Element: `material.params` pointer to a `CommonMaterialParams` record, when that record
    /// is known for the layout.
    pub params_at: Option<u64>,
}

impl ElementLayout {
    /// Client 7.0 (`V7_0_00_76` structs): element 120 bytes.
    pub const V7_0: Self = Self {
        elements_at: 228,
        element_size: 120,
        lods_at: 20,
        lod_fields: [4, 8, 12, 16],
        diffuse_at: 44,
        params_at: Some(52),
    };

    /// Client 1.1 (`V1_1_02_0` structs): element 112 bytes. A `Lod` stores vertex end, vertex
    /// begin, index end and index begin in that order. `material.params` (+80) is not read: the
    /// layout of its record was not checked.
    pub const CLIENT_1_1: Self = Self {
        elements_at: 180,
        element_size: 112,
        lods_at: 96,
        lod_fields: [16, 12, 8, 4],
        diffuse_at: 84,
        params_at: None,
    };

    /// The layout for a database of the given family; unknown families use 7.0.
    #[must_use]
    pub fn for_family(family: crate::version::Family) -> Self {
        match family {
            crate::version::Family::Client11 => Self::CLIENT_1_1,
            crate::version::Family::Other => Self::V7_0,
        }
    }
}

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

/// Reads the element bindings of one Geometry object of a 7.0 database.
///
/// See [`read_geometry_with`].
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
    read_geometry_with(&ElementLayout::V7_0, db, payload, fixes, names, root)
}

/// Reads the element bindings of one Geometry object at data-block offset `root`.
///
/// Returns an empty list when the object has no elements or the fix table does not describe them.
///
/// # Errors
/// Returns an error when the element count is not plausible.
pub fn read_geometry_with(
    layout: &ElementLayout,
    db: &Database32,
    payload: &[u8],
    fixes: &FixIndex,
    names: &HashMap<u64, &str>,
    root: u64,
) -> Result<Vec<ElementBinding>> {
    let data = db.data.start as u64;
    let rd = |off: u64| word(payload, data.checked_add(off)?);
    let (Some(at), Some(bytes)) = (
        root.checked_add(layout.elements_at),
        root.checked_add(layout.elements_at + 4),
    ) else {
        return Ok(Vec::new());
    };
    let (Some(fix), Some(bytes)) = (fixes.get(at), rd(bytes)) else {
        return Ok(Vec::new());
    };
    let bytes = u64::from(bytes);
    if bytes % layout.element_size != 0 || bytes / layout.element_size > MAX_ELEMENTS {
        return Err(Error::format("model element array size is not plausible"));
    }
    let base = u64::from(fix.value);
    let mut out = Vec::new();
    for i in 0..bytes / layout.element_size {
        let Some(e) = i
            .checked_mul(layout.element_size)
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
        if let (Some(lods), Some(lod_bytes)) =
            (fixes.get(e + layout.lods_at), rd(e + layout.lods_at + 4))
            && u64::from(lod_bytes) >= LOD_SIZE
        {
            let l = u64::from(lods.value);
            let [f0, f1, f2, f3] = layout.lod_fields;
            if let (Some(a), Some(z), Some(va), Some(vz)) =
                (rd(l + f0), rd(l + f1), rd(l + f2), rd(l + f3))
            {
                (b.index_begin, b.index_end, b.vertex_begin, b.vertex_end) = (a, z, va, vz);
            }
        }
        if let Some(d) = fixes.get(e + layout.diffuse_at)
            && d.tag == 0
        {
            b.diffuse = names
                .get(&u64::from(d.value))
                .map(|n| n.to_ascii_lowercase());
        }
        if let Some(params_at) = layout.params_at
            && let Some(p) = fixes.get(e + params_at)
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

/// Element bindings of every Geometry resource of a 7.0 pack.
///
/// See [`all_geometries_with`].
///
/// # Errors
/// Returns an error when the pointer-fix table is larger than the limit.
pub fn all_geometries(
    db: &Database32,
    payload: &[u8],
    fixes: &[PackFix],
    limits: &Limits,
) -> Result<HashMap<String, Vec<ElementBinding>>> {
    all_geometries_with(&ElementLayout::V7_0, db, payload, fixes, limits)
}

/// Element bindings of every Geometry resource of the pack, keyed by lower-case resource name
/// (`…(Geometry).xdb`). Geometry objects that cannot be read are skipped.
///
/// # Errors
/// Returns an error when the pointer-fix table is larger than the limit.
pub fn all_geometries_with(
    layout: &ElementLayout,
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
        if let Ok(list) = read_geometry_with(layout, db, payload, &index, &names, f.db_id)
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

    /// Client 1.1 image: Geometry at 0 (elements at 400), one lod at 600, a Texture at 800.
    fn synthetic_client_1_1() -> (Database32, Vec<u8>, Vec<PackFix>) {
        let mut d = vec![0u8; 1000];
        put(&mut d, 184, 112);
        put(&mut d, 400 + 100, 20);
        // Lod: vertex end, vertex begin, index end, index begin.
        put(&mut d, 600 + 4, 11);
        put(&mut d, 600 + 8, 1);
        put(&mut d, 600 + 12, 30);
        put(&mut d, 600 + 16, 3);
        let fix = |offset, tag, value| PackFix { offset, tag, value };
        let fixes = vec![
            fix(180, 1, 400),
            fix(400 + 96, 1, 600),
            fix(400 + 84, 0, 800),
        ];
        let (db, _, _) = synthetic();
        (db, d, fixes)
    }

    #[test]
    fn client_1_1_layout_reads_ranges_in_its_own_field_order() {
        let (db, d, fixes) = synthetic_client_1_1();
        let all = all_geometries_with(
            &ElementLayout::CLIENT_1_1,
            &db,
            &d,
            &fixes,
            &Limits::default(),
        )
        .unwrap();
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
        assert!(g[0].params.is_none());
        // The 7.0 reader finds nothing in a client 1.1 image.
        assert!(
            all_geometries(&db, &d, &fixes, &Limits::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn layout_follows_the_family() {
        use crate::version::Family;
        assert_eq!(
            ElementLayout::for_family(Family::Client11),
            ElementLayout::CLIENT_1_1
        );
        assert_eq!(
            ElementLayout::for_family(Family::Other),
            ElementLayout::V7_0
        );
    }

    #[test]
    fn client_1_1_truncated_image_and_bad_sizes_never_panic() {
        let (db, d, fixes) = synthetic_client_1_1();
        for cut in 0..d.len() {
            let _ = all_geometries_with(
                &ElementLayout::CLIENT_1_1,
                &db,
                &d[..cut],
                &fixes,
                &Limits::default(),
            );
        }
        let mut bad = d.clone();
        put(&mut bad, 184, 113);
        assert!(
            all_geometries_with(
                &ElementLayout::CLIENT_1_1,
                &db,
                &bad,
                &fixes,
                &Limits::default()
            )
            .unwrap()
            .is_empty()
        );
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
