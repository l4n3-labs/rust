use std::fmt;

/// Errors that can occur during a sort operation.
#[derive(Debug)]
pub enum SortError {
    /// The input could not be parsed as JSON or JSONC.
    Parse(String),
    /// The byte range is out of bounds for the input string.
    InvalidRange {
        /// Start of the requested range.
        start: usize,
        /// End of the requested range.
        end: usize,
        /// Actual length of the input.
        len: usize,
    },
    /// The sorted value could not be serialized back to a string.
    Serialize(String),
}

impl fmt::Display for SortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "JSON parse error: {msg}"),
            Self::InvalidRange { start, end, len } => {
                write!(f, "invalid range {start}..{end} for document of length {len}")
            }
            Self::Serialize(msg) => write!(f, "JSON serialization error: {msg}"),
        }
    }
}

impl std::error::Error for SortError {}
