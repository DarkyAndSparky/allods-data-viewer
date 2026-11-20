//! Blob lists: the container shared by geometry, animation and map chunk files.
//!
//! A file (after the optional zlib layer) is a sequence of blobs: `u32 localID`,
//! `u32 size`, `size` bytes of payload. See `docs/formats/geometry-bin.md`.

use crate::error::{Error, Result};

/// Header size of one blob.
const HEADER: usize = 8;
/// Upper bound for the number of blobs in one file.
const MAX_BLOBS: usize = 4096;

/// One blob inside a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blob {
    /// Local id (0 = first buffer, 1 = second, ...).
    pub id: u32,
    /// Offset of the payload from the start of the file.
    pub offset: usize,
    /// Payload size in bytes.
    pub size: usize,
}

/// Splits `d` into blobs. The list must cover the whole input exactly.
///
/// # Errors
///
/// [`Error::Format`] when a header or payload runs past the end, when the input is empty,
/// or when bytes are left over; [`Error::Limit`] for more than 4096 blobs.
pub fn parse_blobs(d: &[u8]) -> Result<Vec<Blob>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    if d.is_empty() {
        return Err(Error::format("empty blob list"));
    }
    while pos < d.len() {
        if out.len() >= MAX_BLOBS {
            return Err(Error::limit("too many blobs"));
        }
        let head = d
            .get(
                pos..pos
                    .checked_add(HEADER)
                    .ok_or_else(|| Error::format("offset overflow"))?,
            )
            .ok_or_else(|| Error::format(format!("blob header at {pos} is cut off")))?;
        let id = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        let offset = pos + HEADER;
        let end = offset
            .checked_add(size)
            .filter(|&e| e <= d.len())
            .ok_or_else(|| Error::format(format!("blob {id} at {pos} runs past the end")))?;
        out.push(Blob { id, offset, size });
        pos = end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(id: u32, data: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn lists_blobs() {
        let mut d = blob(0, &[1, 2, 3]);
        d.extend(blob(1, &[]));
        d.extend(blob(2, &[9; 5]));
        let b = parse_blobs(&d).unwrap();
        assert_eq!(b.len(), 3);
        assert_eq!(
            b[0],
            Blob {
                id: 0,
                offset: 8,
                size: 3
            }
        );
        assert_eq!(
            b[1],
            Blob {
                id: 1,
                offset: 19,
                size: 0
            }
        );
        assert_eq!(
            b[2],
            Blob {
                id: 2,
                offset: 27,
                size: 5
            }
        );
    }

    #[test]
    fn truncated_and_corrupt_lists_are_rejected() {
        assert!(parse_blobs(&[]).is_err());
        let d = blob(0, &[1, 2, 3]);
        assert!(parse_blobs(&d[..d.len() - 1]).is_err());
        assert!(parse_blobs(&d[..5]).is_err());
        let mut huge = blob(0, &[1]);
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_blobs(&huge).is_err());
    }
}
