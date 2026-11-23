//! CommonMaterialParams record (84 bytes) of the 7.0 pack data block, reached from a Geometry object.
//!
//! A Geometry object stores a pointer to its root material at data-block offset `+0x128` (a DbIdRef
//! pointer-fix, tag 0). The target is the 84-byte `CommonMaterialParams` record itself (no wrapper).
//! Texture fields are pointer-fixes into the data block. Field offsets follow the EditorTools
//! `CommonMaterialParams` layout (`StructSize 84`, `V7_0_00_76`, from Allods Online Editor Tools by
//! Paulus, MIT; cited, no code copied). Texture names are resolved by the caller from the pack entry at
//! the target offset.
//!
//! Status: the root pointer (`+0x128`, tag 0) and the texture pointers at `+40`, `+48`, `+56` are pinned
//! on 5,498 rootMaterial blocks (see `docs/formats/common-material.md`). The scalar and flag fields are
//! read at the same offsets but are constant in the sampled set, so they are not an independent check.

use crate::error::{Error, Result};
use crate::fixes::PackFix;
use std::collections::HashMap;

/// Size of the `CommonMaterialParams` record in bytes.
pub const COMMON_MATERIAL_SIZE: usize = 84;

/// Data-block offset of the rootMaterial pointer inside a Geometry object.
pub const ROOT_MATERIAL_OFFSET: u64 = 0x128;

/// Index of pointer-fixes keyed by their data-block offset.
#[derive(Debug, Clone, Default)]
pub struct FixIndex {
    by_offset: HashMap<u64, PackFix>,
}

impl FixIndex {
    /// Builds the index from parsed fixes. A later entry replaces an earlier one at the same offset.
    #[must_use]
    pub fn new(fixes: &[PackFix]) -> Self {
        let by_offset = fixes.iter().map(|f| (f.offset, *f)).collect();
        Self { by_offset }
    }

    /// Returns the fix stored at data-block offset `offset`, if any.
    #[must_use]
    pub fn get(&self, offset: u64) -> Option<PackFix> {
        self.by_offset.get(&offset).copied()
    }
}

/// A decoded `CommonMaterialParams` record. Texture fields hold the data-block offset of the
/// referenced texture object, or `None` when the record has no pointer-fix at that field.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonMaterialParams {
    /// Pointer at `+24`.
    pub bump_texture: Option<u32>,
    /// Integer at `+32`.
    pub color: u32,
    /// Integer at `+36`.
    pub contour_color_modifier: u32,
    /// Pointer at `+40`.
    pub effect_mask_texture: Option<u32>,
    /// Pointer at `+48`.
    pub env_reflection_texture: Option<u32>,
    /// Pointer at `+56`.
    pub env_specular_texture: Option<u32>,
    /// Float at `+64`.
    pub shininess: f32,
    /// Integer at `+68`.
    pub specular_color_modifier: u32,
    /// Float at `+72`.
    pub specular_intensity: f32,
    /// Flag at `+76`.
    pub bump_disabled: bool,
    /// Flag at `+77`.
    pub selfillum: bool,
    /// Flag at `+78`.
    pub use_env_contour_color_factor: bool,
    /// Flag at `+79`.
    pub use_env_specular_color_factor: bool,
    /// Flag at `+80`.
    pub use_mask_color: bool,
    /// Flag at `+81`.
    pub vertex_baked_light: bool,
}

/// A pointer field is a fix with tag 0, 1 or 5 (DbIdRef, Direct or Direct+External).
fn pointer_at(fixes: &FixIndex, offset: u64) -> Option<u32> {
    fixes
        .get(offset)
        .filter(|f| matches!(f.tag, 0 | 1 | 5))
        .map(|f| f.value)
}

fn read_u32(block: &[u8], at: usize) -> Result<u32> {
    let b = block
        .get(at..at + 4)
        .ok_or_else(|| Error::format("common material record is truncated"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_f32(block: &[u8], at: usize) -> Result<f32> {
    read_u32(block, at).map(f32::from_bits)
}

fn read_flag(block: &[u8], at: usize) -> Result<bool> {
    match block.get(at) {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        Some(_) => Err(Error::format("common material flag is not 0 or 1")),
        None => Err(Error::format("common material record is truncated")),
    }
}

/// Decodes the `CommonMaterialParams` record at data-block offset `at`.
///
/// `block` is the pack data block. `fixes` must be the pointer-fix index of the same pack.
///
/// # Errors
/// Returns an error when the 84-byte record does not fit in `block`, or when a flag byte is not 0 or 1.
pub fn parse_common_material(
    block: &[u8],
    fixes: &FixIndex,
    at: u64,
) -> Result<CommonMaterialParams> {
    let base = usize::try_from(at).map_err(|_| Error::format("material offset is too large"))?;
    if base
        .checked_add(COMMON_MATERIAL_SIZE)
        .is_none_or(|end| end > block.len())
    {
        return Err(Error::format(
            "common material record runs past the data block",
        ));
    }
    let field = |off: usize| at + off as u64;
    Ok(CommonMaterialParams {
        bump_texture: pointer_at(fixes, field(24)),
        color: read_u32(block, base + 32)?,
        contour_color_modifier: read_u32(block, base + 36)?,
        effect_mask_texture: pointer_at(fixes, field(40)),
        env_reflection_texture: pointer_at(fixes, field(48)),
        env_specular_texture: pointer_at(fixes, field(56)),
        shininess: read_f32(block, base + 64)?,
        specular_color_modifier: read_u32(block, base + 68)?,
        specular_intensity: read_f32(block, base + 72)?,
        bump_disabled: read_flag(block, base + 76)?,
        selfillum: read_flag(block, base + 77)?,
        use_env_contour_color_factor: read_flag(block, base + 78)?,
        use_env_specular_color_factor: read_flag(block, base + 79)?,
        use_mask_color: read_flag(block, base + 80)?,
        vertex_baked_light: read_flag(block, base + 81)?,
    })
}

/// Returns the data-block offset of the root material of the Geometry object at `geometry_offset`,
/// or `None` when that object has no DbIdRef (tag 0) fix at [`ROOT_MATERIAL_OFFSET`].
#[must_use]
pub fn root_material_offset(fixes: &FixIndex, geometry_offset: u64) -> Option<u32> {
    let f = fixes.get(geometry_offset + ROOT_MATERIAL_OFFSET)?;
    (f.tag == 0).then_some(f.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(shininess: f32, spec: f32, bools: [u8; 6]) -> Vec<u8> {
        let mut b = vec![0u8; COMMON_MATERIAL_SIZE];
        b[32..36].copy_from_slice(&7u32.to_le_bytes());
        b[36..40].copy_from_slice(&9u32.to_le_bytes());
        b[64..68].copy_from_slice(&shininess.to_le_bytes());
        b[68..72].copy_from_slice(&3u32.to_le_bytes());
        b[72..76].copy_from_slice(&spec.to_le_bytes());
        b[76..82].copy_from_slice(&bools);
        b
    }

    fn fix(offset: u64, tag: u8, value: u32) -> PackFix {
        PackFix { offset, tag, value }
    }

    #[test]
    fn decodes_scalars_flags_and_texture_targets() {
        let block = record(25.0, 1.0, [0, 1, 1, 1, 0, 1]);
        let fixes = FixIndex::new(&[fix(40, 0, 1000), fix(48, 1, 2000), fix(56, 5, 3000)]);
        let m = parse_common_material(&block, &fixes, 0).unwrap();
        assert_eq!(m.color, 7);
        assert_eq!(m.contour_color_modifier, 9);
        assert_eq!(m.shininess, 25.0);
        assert_eq!(m.specular_color_modifier, 3);
        assert_eq!(m.specular_intensity, 1.0);
        assert!(!m.bump_disabled && m.selfillum && m.use_env_contour_color_factor);
        assert!(m.use_env_specular_color_factor && !m.use_mask_color && m.vertex_baked_light);
        assert_eq!(m.effect_mask_texture, Some(1000));
        assert_eq!(m.env_reflection_texture, Some(2000));
        assert_eq!(m.env_specular_texture, Some(3000));
        assert_eq!(m.bump_texture, None);
    }

    #[test]
    fn ignores_non_pointer_tags_and_missing_fixes() {
        // Tag 7 (type reference) at +40 is not a texture pointer; +48 has no fix at all.
        let block = record(1.0, 1.0, [0; 6]);
        let fixes = FixIndex::new(&[fix(40, 7, 55), fix(0, 7, 23)]);
        let m = parse_common_material(&block, &fixes, 0).unwrap();
        assert_eq!(m.effect_mask_texture, None);
        assert_eq!(m.env_reflection_texture, None);
        assert_eq!(m.env_specular_texture, None);
    }

    #[test]
    fn rejects_truncated_records_and_bad_flags() {
        let fixes = FixIndex::default();
        let short = vec![0u8; COMMON_MATERIAL_SIZE - 1];
        assert!(parse_common_material(&short, &fixes, 0).is_err());
        let mut bad = record(1.0, 1.0, [0; 6]);
        bad[77] = 2;
        assert!(parse_common_material(&bad, &fixes, 0).is_err());
    }

    #[test]
    fn root_material_pointer_is_tag_zero_at_0x128() {
        let fixes = FixIndex::new(&[fix(ROOT_MATERIAL_OFFSET, 0, 4242), fix(0x200, 1, 77)]);
        assert_eq!(root_material_offset(&fixes, 0), Some(4242));
        // Non-zero tag at the same field is not a root material pointer.
        assert_eq!(
            root_material_offset(&fixes, 0x200 - ROOT_MATERIAL_OFFSET),
            None
        );
        // No fix at all.
        assert_eq!(root_material_offset(&fixes, 0x1000), None);
    }
}
