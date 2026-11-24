//! The rows of a table.

use std::fmt;
use std::iter::FusedIterator;

use crate::error::Result;
use crate::record::{Columns, RecordReader, Row, Scanner};
use crate::schema::TableDefinition;
use crate::store::PageStore;

/// The live rows of one table, in page and slot order.
///
/// A row that cannot be decoded is an `Err` item, and the iterator goes on to
/// the next row. So is a page that cannot be read, in place of its rows.
#[must_use = "an iterator reads nothing until it is used"]
pub struct Rows<'db> {
    scanner: Scanner<'db>,
}

impl<'db> Rows<'db> {
    /// The rows of `definition`, cut down to `columns` of it.
    pub(crate) fn new(
        store: &'db PageStore,
        definition: &'db TableDefinition,
        columns: Columns,
    ) -> Self {
        let refusal = columns.check(definition).err();
        let reader = RecordReader::new(store, definition, columns, store.longest_value());
        Self {
            scanner: Scanner::new(reader, refusal),
        }
    }

    /// The rows of `definition` with every column.
    pub(crate) fn all(store: &'db PageStore, definition: &'db TableDefinition) -> Self {
        Self::new(store, definition, Columns::all(definition))
    }

    /// The rows that `reader` reads.
    pub(crate) fn of(reader: RecordReader<'db>) -> Self {
        Self {
            scanner: Scanner::new(reader, None),
        }
    }
}

impl Iterator for Rows<'_> {
    type Item = Result<Row>;

    fn next(&mut self) -> Option<Self::Item> {
        self.scanner.next(RecordReader::row)
    }
}

impl FusedIterator for Rows<'_> {}

impl fmt::Debug for Rows<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Rows").finish_non_exhaustive()
    }
}
