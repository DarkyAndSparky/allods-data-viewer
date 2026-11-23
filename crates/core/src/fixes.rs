//! Pointer-fix block (block 4) of the 7.0 pack database, 8-byte entries.
//!
//! Layout (inflated pack, little endian, see `docs/formats/pack-fixes.md`):
//! `u32 id = 4`, `u32 count`, then `count` entries of `u32 d`, `u32 v`.
//! `d = (offset / 4) << 3 | tag`, where `offset` is relative to the data block. The tag
//! values seen in the 7.0 pack are 0, 1, 5, 6 and 7. Tag 5 points at a pointer to an array or
//! sub-object header at the blob offset `v` (about 11 % of tag-5 targets are NUL-terminated strings).
//! Status: the layout and tag split are verified on the 7.0 pack; the meaning of tags 0, 1, 6 and 7
//! is not decoded, and the tag-5 target classes are measured, not decoded.

use crate::error::{Error, Result};
use crate::limits::Limits;

/// Chunk id of the fix block in the 7.0 pack.
pub const FIX_CHUNK_ID: u32 = 4;

/// One pointer-fix entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackFix {
    /// Byte offset of the fixed field, relative to the data block.
    pub offset: u64,
    /// Low three bits of the stored word.
    pub tag: u8,
    /// The stored value word.
    pub value: u32,
}

fn word_at(d: &[u8], at: usize) -> Result<u32> {
    let b = at
        .checked_add(4)
        .and_then(|end| d.get(at..end))
        .ok_or_else(|| Error::format("pack fix block is truncated"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parses the fix block that starts at byte `at` of the inflated pack.
///
/// # Errors
/// Returns an error when the chunk id is not 4, when the entry count exceeds
/// [`Limits::max_fixes`], or when the entries run past the end of the data.
pub fn parse_fixes(d: &[u8], at: usize, limits: &Limits) -> Result<Vec<PackFix>> {
    let id = word_at(d, at)?;
    if id != FIX_CHUNK_ID {
        return Err(Error::format(format!(
            "pack fix block id is {id}, expected {FIX_CHUNK_ID} at {at}"
        )));
    }
    let count = u64::from(word_at(d, at + 4)?);
    if count > limits.max_fixes {
        return Err(Error::limit("pack fix count exceeds the limit"));
    }
    let count = usize::try_from(count).map_err(|_| Error::limit("pack fix count is too large"))?;
    let body = at + 8;
    let bytes = count
        .checked_mul(8)
        .and_then(|n| body.checked_add(n))
        .and_then(|end| d.get(body..end))
        .ok_or_else(|| Error::format("pack fix entries run past the end of the data"))?;
    let mut out = Vec::with_capacity(bytes.len() / 8);
    for entry in bytes.as_chunks::<8>().0 {
        let [a, b, c, e, f, g, h, i] = *entry;
        let d_word = u32::from_le_bytes([a, b, c, e]);
        let value = u32::from_le_bytes([f, g, h, i]);
        out.push(PackFix {
            offset: u64::from(d_word >> 3) * 4,
            // The mask keeps the value in 0..=7, so the conversion cannot fail.
            tag: u8::try_from(d_word & 7).unwrap_or(0),
            value,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(entries: &[(u32, u32)], declared: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&FIX_CHUNK_ID.to_le_bytes());
        out.extend_from_slice(&declared.to_le_bytes());
        for (d, v) in entries {
            out.extend_from_slice(&d.to_le_bytes());
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_offset_tag_and_value() {
        // d = (16 / 4) << 3 | 5 : offset 16, tag 5 (pointer to an array or sub-object); value is a blob offset.
        let data = block(&[((4 << 3) | 5, 1234), ((8 << 3) | 1, 0)], 2);
        let fixes = parse_fixes(&data, 0, &Limits::default()).unwrap();
        assert_eq!(
            fixes,
            vec![
                PackFix {
                    offset: 16,
                    tag: 5,
                    value: 1234
                },
                PackFix {
                    offset: 32,
                    tag: 1,
                    value: 0
                },
            ]
        );
    }

    #[test]
    fn truncated_entries_are_rejected() {
        // Declares three entries but only two are present.
        let mut data = block(&[(8, 1), (16, 2)], 3);
        assert!(parse_fixes(&data, 0, &Limits::default()).is_err());
        data.truncate(5);
        assert!(parse_fixes(&data, 0, &Limits::default()).is_err());
    }

    #[test]
    fn corrupted_fields_are_rejected() {
        // Wrong chunk id.
        let mut data = block(&[(8, 1)], 1);
        data[0] = 5;
        assert!(parse_fixes(&data, 0, &Limits::default()).is_err());
        // A count above the limit is reported as a limit error, not allocated.
        let big = block(&[], 1 << 30);
        let limits = Limits {
            max_fixes: 1000,
            ..Limits::default()
        };
        assert!(parse_fixes(&big, 0, &limits).is_err());
    }
}
