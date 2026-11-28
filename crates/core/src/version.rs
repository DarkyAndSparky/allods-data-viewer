//! Registry of known resource-database versions.
//!
//! Every `Bin/pack.bin` starts with a version signature: 8 bytes in the 32-bit layout (chunk 0),
//! a 12 byte buffer in the 64-bit layout. The signature is a hash of the struct definitions, not a
//! release number, so a new client build usually brings a new one. This table maps the signatures
//! we know to the client they belong to and to the record layout family the readers use.
//!
//! Where a row comes from is stored with it (`source`). Rows copied from the MIT project
//! AllodsOnlineEditorTools (see `NOTICE`) were not checked against files of our own; the row of
//! client 1.1.00.41 was checked against a real `Bin/pack.bin` and its `Profiles/game.version`.
//! The status of each version is in `docs/SUPPORTED-VERSIONS.md`.

use crate::db::{self, Layout};
use crate::pak::looks_like_zlib;
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Which record layouts the readers use for a database.
///
/// Only families the readers can tell apart are listed; everything else is [`Family::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Client 1.1.x: `Geometry` 400 bytes, `ModelElement` 112 bytes (see `docs/formats/model-binding.md`).
    Client11,
    /// Any other known or unknown client. Readers fall back to the 7.0 record layouts.
    Other,
}

/// One known signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownVersion {
    /// The signature bytes: 8 for [`Layout::Bits32`], 12 for [`Layout::V2`].
    pub signature: &'static [u8],
    /// Container layout of the database.
    pub layout: Layout,
    /// Client name as shown to the user.
    pub client: &'static str,
    /// Record layout family.
    pub family: Family,
    /// Where the row comes from.
    pub source: &'static str,
}

const EDITOR_TOOLS: &str = "AllodsOnlineEditorTools version table (not checked on our files)";

/// All known signatures.
pub const KNOWN: &[KnownVersion] = &[
    KnownVersion {
        signature: &[0x24, 0x19, 0xFD, 0x3C, 0x1B, 0x6C, 0x73, 0x8A],
        layout: Layout::Bits32,
        client: "Allods Online 1.1.00.41",
        family: Family::Client11,
        source: "client release 1.1.00.41: Bin/pack.bin and Profiles/game.version (checked 2026-11-26)",
    },
    KnownVersion {
        signature: &[0x58, 0x47, 0xDB, 0x46, 0x93, 0x64, 0x49, 0x3C],
        layout: Layout::Bits32,
        client: "Allods Online 1.1.02.0",
        family: Family::Client11,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x30, 0x4C, 0x70, 0xAC, 0x5A, 0x6F, 0x33, 0xD0],
        layout: Layout::Bits32,
        client: "Allods Online 1.1.04.44",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x20, 0x60, 0xB4, 0x0B, 0x8C, 0xBE, 0x5B, 0x8D],
        layout: Layout::Bits32,
        client: "Allods Online 3.0",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x64, 0x1A, 0xD1, 0xD4, 0x8E, 0x1F, 0xD7, 0xEC],
        layout: Layout::Bits32,
        client: "Allods Online 4.0.02",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x70, 0x25, 0xBD, 0x50, 0x27, 0x72, 0x4A, 0x6D],
        layout: Layout::Bits32,
        client: "Allods Online 7.0.00",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0xB0, 0x77, 0xEC, 0x9F, 0x77, 0xA4, 0x0A, 0xA0],
        layout: Layout::Bits32,
        client: "Cloud Pirates 1.7.7",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x44, 0x06, 0x8E, 0x78, 0xE8, 0xE6, 0x78, 0x76],
        layout: Layout::Bits32,
        client: "Allods Online 14.0.00.21",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[0x98, 0x3A, 0x36, 0xAC, 0x75, 0xDB, 0x9E, 0xC5],
        layout: Layout::Bits32,
        client: "Allods Online 14.0.01.71",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
    KnownVersion {
        signature: &[
            0xC4, 0x02, 0x2E, 0x59, 0x73, 0x04, 0x00, 0x00, 0x44, 0x1E, 0x2B, 0x41,
        ],
        layout: Layout::V2,
        client: "Allods Online 17.0.01.55",
        family: Family::Other,
        source: EDITOR_TOOLS,
    },
];

/// What [`identify`] found in a database file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbIdent {
    /// Container layout.
    pub layout: Layout,
    /// The signature bytes as stored (8 or 12).
    pub signature: Vec<u8>,
    /// The registry row, when the signature is known.
    pub known: Option<&'static KnownVersion>,
}

impl DbIdent {
    /// The signature as upper-case hex.
    #[must_use]
    pub fn hex(&self) -> String {
        hex(&self.signature)
    }

    /// Client name, or `unknown` for a signature that is not in the registry.
    #[must_use]
    pub fn client(&self) -> &'static str {
        self.known.map_or("unknown", |k| k.client)
    }

    /// Record layout family; [`Family::Other`] for unknown signatures.
    #[must_use]
    pub fn family(&self) -> Family {
        self.known.map_or(Family::Other, |k| k.family)
    }
}

/// Upper-case hex text of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Looks a signature up in the registry.
#[must_use]
pub fn lookup(signature: &[u8]) -> Option<&'static KnownVersion> {
    KNOWN.iter().find(|k| k.signature == signature)
}

/// First 20 bytes of the database, inflated when the file is a zlib stream.
fn head(data: &[u8]) -> Option<[u8; 20]> {
    let mut out = [0u8; 20];
    if looks_like_zlib(data) {
        ZlibDecoder::new(data).read_exact(&mut out).ok()?;
    } else {
        out.copy_from_slice(data.get(..20)?);
    }
    Some(out)
}

/// Recognises a resource database and reads its signature without inflating the whole file.
///
/// The 32-bit layout is told by its first chunk: id 0, size 8, then the signature, then a chunk
/// with id 1. The 64-bit layout is recognised by [`db::db_signature`]. Returns `None` for
/// anything else, including the 18.0 revision of the 64-bit layout (its signature is not told
/// from other data by that check).
#[must_use]
pub fn identify(data: &[u8]) -> Option<DbIdent> {
    let h = head(data)?;
    let word = |at: usize| u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]]);
    if word(0) == 0 && word(4) == 8 && word(16) == 1 {
        let signature = h[8..16].to_vec();
        return Some(DbIdent {
            layout: Layout::Bits32,
            known: lookup(&signature),
            signature,
        });
    }
    let signature = db::db_signature(data)?.to_vec();
    Some(DbIdent {
        layout: Layout::V2,
        known: lookup(&signature),
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn db32_head(sig: [u8; 8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(0u32.to_le_bytes());
        v.extend(8u32.to_le_bytes());
        v.extend(sig);
        v.extend(1u32.to_le_bytes());
        v.extend(0u32.to_le_bytes());
        v.extend([0u8; 16]);
        v
    }

    fn zlib(d: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::fast());
        e.write_all(d).expect("write");
        e.finish().expect("finish")
    }

    #[test]
    fn registry_rows_are_consistent() {
        for (i, k) in KNOWN.iter().enumerate() {
            let want = if k.layout == Layout::Bits32 { 8 } else { 12 };
            assert_eq!(k.signature.len(), want, "{}", k.client);
            for o in &KNOWN[i + 1..] {
                assert_ne!(k.signature, o.signature, "{} / {}", k.client, o.client);
            }
        }
    }

    #[test]
    fn client_1_1_row_is_found_plain_and_in_zlib() {
        let raw = db32_head([0x24, 0x19, 0xFD, 0x3C, 0x1B, 0x6C, 0x73, 0x8A]);
        for data in [raw.clone(), zlib(&raw)] {
            let id = identify(&data).expect("identified");
            assert_eq!(id.layout, Layout::Bits32);
            assert_eq!(id.hex(), "2419FD3C1B6C738A");
            assert_eq!(id.client(), "Allods Online 1.1.00.41");
            assert_eq!(id.family(), Family::Client11);
        }
    }

    #[test]
    fn unknown_32_bit_signature_is_reported_as_unknown() {
        let id = identify(&db32_head([1, 2, 3, 4, 5, 6, 7, 8])).expect("identified");
        assert_eq!(id.client(), "unknown");
        assert_eq!(id.family(), Family::Other);
        assert!(id.known.is_none());
    }

    #[test]
    fn sixty_four_bit_buffer_is_recognised() {
        let mut v = vec![
            0xC4, 0x02, 0x2E, 0x59, 0x73, 4, 0, 0, 0x44, 0x1E, 0x2B, 0x41,
        ];
        v.extend([0u8; 16]);
        let id = identify(&v).expect("identified");
        assert_eq!(id.layout, Layout::V2);
        assert_eq!(id.client(), "Allods Online 17.0.01.55");
    }

    #[test]
    fn other_data_is_not_a_database() {
        assert!(identify(b"hello world, this is not a database").is_none());
        assert!(identify(&[]).is_none());
        // Chunk 0 with a wrong size, or a missing chunk 1, is not the 32-bit layout.
        let mut bad = db32_head([9; 8]);
        bad[4] = 7;
        assert!(identify(&bad).is_none());
        let mut bad = db32_head([9; 8]);
        bad[16] = 2;
        assert!(identify(&bad).is_none());
    }
}
