//! The name of a row in its table.

use std::fmt;

use crate::row_address::RowAddress;

/// A row's identity within its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RowId {
    /// A row of the file, by where it is stored.
    Stored(RowAddress),
    /// A row that was laid over a file, by the number it was given.
    Added(u32),
}

impl RowId {
    /// Where the row is stored, for a row of the file.
    #[must_use]
    pub const fn address(self) -> Option<RowAddress> {
        match self {
            Self::Stored(address) => Some(address),
            Self::Added(_) => None,
        }
    }
}

impl From<RowAddress> for RowId {
    fn from(address: RowAddress) -> Self {
        Self::Stored(address)
    }
}

impl fmt::Display for RowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stored(address) => address.fmt(formatter),
            Self::Added(number) => write!(formatter, "added row {number}"),
        }
    }
}
