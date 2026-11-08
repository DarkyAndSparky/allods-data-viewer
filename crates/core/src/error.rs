//! Error type shared by all readers.

use std::fmt;

/// Everything that can go wrong while reading a resource.
#[derive(Debug)]
pub enum Error {
    /// Reading from the underlying source failed.
    Io(std::io::Error),
    /// The data does not match the expected format.
    Format(String),
}

/// Result type used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Shortcut for building a [`Error::Format`] error.
    pub fn format(message: impl Into<String>) -> Self {
        Self::Format(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Format(message) => write!(f, "invalid format: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Format(_) => None,
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
}
