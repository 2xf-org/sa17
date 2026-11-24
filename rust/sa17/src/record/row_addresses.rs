//! The addresses of a table's rows.

use std::fmt;
use std::iter::FusedIterator;

use crate::error::Result;
use crate::record::{Columns, RecordReader, Scanner};
use crate::row_address::RowAddress;
use crate::schema::TableDefinition;
use crate::store::PageStore;

/// The addresses of a table's live rows, in page and slot order.
///
/// This reads each row's flags and none of its fields, so it is the quick
/// way to count a table or to index it.
#[must_use = "an iterator reads nothing until it is used"]
pub struct RowAddresses<'db> {
    scanner: Scanner<'db>,
}

impl<'db> RowAddresses<'db> {
    pub(crate) fn new(store: &'db PageStore, definition: &'db TableDefinition) -> Self {
        let none = Columns::none(definition);
        let reader = RecordReader::new(store, definition, none, store.longest_value());
        Self {
            scanner: Scanner::new(reader, None),
        }
    }
}

impl Iterator for RowAddresses<'_> {
    type Item = Result<RowAddress>;

    fn next(&mut self) -> Option<Self::Item> {
        self.scanner
            .next(|reader, page, slot, _met| reader.address(page, slot))
    }
}

impl FusedIterator for RowAddresses<'_> {}

impl fmt::Debug for RowAddresses<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RowAddresses")
            .finish_non_exhaustive()
    }
}
