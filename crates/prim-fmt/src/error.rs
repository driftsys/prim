//! Engine error types — part of the public contract.

/// An error returned by [`crate::format`] when a source cannot be formatted.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// The source could not be parsed as its format. The string carries the
    /// underlying parser's message (including location, when available).
    #[error("{0}")]
    Parse(String),
}
