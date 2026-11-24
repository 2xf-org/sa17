//! One record of a page of rows.

use std::fmt;

use crate::row_address::RowAddress;

/// A record of a page of rows: a row, a forward to one, or a piece of a long
/// value. [The format's description][format] says what its bytes mean.
///
/// [format]: https://github.com/2xf-org/sa17/blob/main/docs/format.md#records
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Record<'page> {
    address: RowAddress,
    bytes: &'page [u8],
}

impl<'page> Record<'page> {
    /// The record at `address`, whose bytes include its length and its flags.
    pub(crate) fn new(address: RowAddress, bytes: &'page [u8]) -> Self {
        Self { address, bytes }
    }

    /// The page and the slot that the record is in.
    #[must_use]
    pub fn address(self) -> RowAddress {
        self.address
    }

    /// The record's bytes: its length in two, its flags in one, and the rest.
    #[must_use]
    pub fn bytes(self) -> &'page [u8] {
        self.bytes
    }

    /// The record's flags.
    #[must_use]
    pub fn flags(self) -> u8 {
        self.bytes.get(2).copied().unwrap_or_default()
    }
}

impl fmt::Debug for Record<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Record")
            .field("address", &self.address)
            .field("flags", &self.flags())
            .field("length", &self.bytes.len())
            .finish()
    }
}
