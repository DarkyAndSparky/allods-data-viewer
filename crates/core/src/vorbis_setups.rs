//! Vorbis setup packets referenced by FSB5 streams (by CRC32 of the packet).
//!
//! FMOD stores no codebooks in its banks; every stream names one of a fixed list of setup
//! packets. The list (161 packets) comes from the table of the vgmstream project (ISC license,
//! see `NOTICE`), which got it from the python-fsb5 project. It is kept zlib compressed in
//! `data/vorbis_setups.bin.z`, made by `scripts/gen-vorbis-setups.py`.

use flate2::read::ZlibDecoder;
use std::io::Read;
use std::sync::OnceLock;

static PACKED: &[u8] = include_bytes!("../data/vorbis_setups.bin.z");

/// `(crc32, setup packet)` pairs, inflated on first use.
pub(crate) fn all() -> &'static [(u32, Vec<u8>)] {
    static LIST: OnceLock<Vec<(u32, Vec<u8>)>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut raw = Vec::new();
        if ZlibDecoder::new(PACKED).read_to_end(&mut raw).is_err() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut pos = 0usize;
        while pos + 8 <= raw.len() {
            let crc = u32::from_le_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]);
            let len = u32::from_le_bytes([raw[pos + 4], raw[pos + 5], raw[pos + 6], raw[pos + 7]])
                as usize;
            pos += 8;
            let Some(data) = raw.get(pos..pos.saturating_add(len)) else {
                break;
            };
            out.push((crc, data.to_vec()));
            pos += len;
        }
        out
    })
}
