//! Error type shared by all readers.

use std::fmt;

/// Everything that can go wrong while reading a resource.
#[derive(Debug)]
pub enum Error {
    /// Reading from the underlying source failed.
    Io(std::io::Error),
    /// The data does not match the expected format.
    Format(String),
    /// The format or its version is known but not supported (for example an unknown
    /// compression method).
    Unsupported(String),
    /// A stored checksum does not match the data.
    Crc(String),
    /// A configured safety limit was exceeded.
    Limit(String),
}

/// Result type used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Shortcut for building a [`Error::Format`] error.
    pub fn format(message: impl Into<String>) -> Self {
        Self::Format(message.into())
    }

    /// Shortcut for building a [`Error::Unsupported`] error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    /// Shortcut for building a [`Error::Crc`] error.
    pub fn crc(message: impl Into<String>) -> Self {
        Self::Crc(message.into())
    }

    /// Shortcut for building a [`Error::Limit`] error.
    pub fn limit(message: impl Into<String>) -> Self {
        Self::Limit(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Format(message) => write!(f, "invalid format: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported: {message}"),
            Self::Crc(message) => write!(f, "checksum mismatch: {message}"),
            Self::Limit(message) => write!(f, "limit exceeded: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn format_error_displays_message() {
        let err = Error::format("bad header");
        assert_eq!(err.to_string(), "invalid format: bad header");
        assert!(err.source().is_none());
    }

    #[test]
    fn io_error_keeps_its_source() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err = Error::from(io);
        assert!(err.to_string().starts_with("I/O error"));
        assert!(err.source().is_some());
    }

    #[test]
    fn other_kinds_have_distinct_messages() {
        assert!(
            Error::unsupported("x")
                .to_string()
                .starts_with("unsupported")
        );
        assert!(Error::crc("x").to_string().starts_with("checksum"));
        assert!(Error::limit("x").to_string().starts_with("limit"));
    }
}
