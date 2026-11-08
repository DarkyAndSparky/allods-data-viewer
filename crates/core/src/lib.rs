//! PakScope core: readers for game resource formats.
//!
//! This crate contains no user interface and makes no assumptions about where
//! data comes from. Everything is built so that the same code can run in the
//! command line tool, in the desktop app and (later) in the browser.
//!
//! Supported formats are documented in `docs/formats/`.

pub mod error;

pub use error::{Error, Result};

/// Version of the core library, taken from `Cargo.toml`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn version_looks_like_semver() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got {}",
            version()
        );
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
