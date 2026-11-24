//! Where a value that is too long for its row is kept.

use crate::row_address::RowAddress;

/// What a row keeps in the place of a long value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Descriptor<'record> {
    /// The first bytes of the value.
    pub(crate) prefix: &'record [u8],
    /// The length of what the pieces hold: the value, or its compressed form.
    pub(crate) stored_length: u32,
    /// Where the first piece is.
    pub(crate) first: RowAddress,
    /// The length of the value, when its pieces hold it compressed.
    pub(crate) decompressed_length: Option<u32>,
}
