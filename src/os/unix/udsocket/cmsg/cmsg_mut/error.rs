use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// Error type returned by [`CmsgMut::reserve()`] and its variations.
#[derive(Debug)]
pub enum ReserveError {
    /// `reserve()` is unsupported for the buffer type.
    Unsupported,
    /// Memory allocation failed.
    Failed(Box<dyn Error>),
}
impl Display for ReserveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("growing the buffer is not supported"),
            Self::Failed(e) => Display::fmt(e, f),
        }
    }
}
impl Error for ReserveError {}

/// Result type returned by [`CmsgMut::reserve()`] and its variations.
pub type ReserveResult = Result<(), ReserveError>;
