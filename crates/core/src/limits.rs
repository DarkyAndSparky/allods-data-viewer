//! Safety limits applied to every untrusted input.
//!
//! The defaults are a proposal from `docs/SECURITY-AND-QUALITY.md` and have not been
//! tuned on the full set of game archives yet.

/// One mebibyte.
const MIB: u64 = 1024 * 1024;
/// One gibibyte.
const GIB: u64 = 1024 * MIB;

/// Upper bounds for sizes and counts read from files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of entries in one archive.
    pub max_entries: u64,
    /// Maximum length of an entry path in bytes.
    pub max_path_len: usize,
    /// Maximum size of one entry after the ZIP layer.
    pub max_entry_size: u64,
    /// Maximum size of one entry after the extra zlib layer.
    pub max_inflated_size: u64,
    /// Maximum ratio of unpacked to packed size for entries larger than
    /// [`Limits::ratio_floor`] (archive bomb guard).
    pub max_ratio: u64,
    /// Entries up to this size are exempt from the ratio check.
    pub ratio_floor: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 10_000_000,
            max_path_len: 1024,
            max_entry_size: 2 * GIB,
            max_inflated_size: GIB,
            max_ratio: 2000,
            ratio_floor: MIB,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_known_samples() {
        let limits = Limits::default();
        // The largest inflated .loc seen so far is 71 577 688 bytes.
        assert!(limits.max_inflated_size > 71_577_688);
        assert!(limits.max_path_len >= 255);
    }
}
