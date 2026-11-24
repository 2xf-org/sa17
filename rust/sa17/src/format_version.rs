//! The version of a database file's format.

use std::fmt;

/// The version of the file format, which the first page names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormatVersion {
    major: u32,
    minor: u16,
    patch: u16,
}

impl FormatVersion {
    /// The version that SQL Anywhere 17 writes: 3.201.12.
    pub const SQL_ANYWHERE_17: Self = Self::new(3, 201, 12);

    pub(crate) const fn new(major: u32, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// The first of the version's three numbers.
    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    /// The second of them.
    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    /// The last of them.
    #[must_use]
    pub const fn patch(self) -> u16 {
        self.patch
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
