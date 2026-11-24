//! What the width of a column counts.

use std::fmt;

/// What the declared width of a string or binary column counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LengthUnit {
    /// Bytes, in the file's encoding for a text.
    Bytes,
    /// Characters.
    Characters,
}

impl fmt::Display for LengthUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bytes => "bytes",
            Self::Characters => "characters",
        })
    }
}
