//! Where a row is stored.

use std::fmt;

/// The place of a row in the file: a table page and a slot on it.
///
/// Reading never changes the file, so an address names the same row for as
/// long as the database is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowAddress {
    page: u32,
    slot: u16,
}

impl RowAddress {
    /// The address of the row in `slot` of `page`.
    #[must_use]
    pub const fn new(page: u32, slot: u16) -> Self {
        Self { page, slot }
    }

    /// The table page that holds the row, or the forward to it.
    #[must_use]
    pub const fn page(self) -> u32 {
        self.page
    }

    /// The row's slot on that page.
    #[must_use]
    pub const fn slot(self) -> u16 {
        self.slot
    }
}

impl fmt::Display for RowAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "page {}, slot {}", self.page, self.slot)
    }
}
