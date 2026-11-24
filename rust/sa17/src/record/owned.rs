//! Owned iterators over a table's rows and row addresses backed by `Arc<Database>`.

use std::fmt;
use std::iter::FusedIterator;
use std::sync::Arc;

use crate::database::Database;
use crate::error::{Error, Result};
use crate::record::{Columns, RecordReader, Row, ScannerState};
use crate::row_address::RowAddress;
use crate::schema::TableDefinition;

/// An owned, `'static` iterator over the live rows of a table.
///
/// Created by [`Database::scan_owned`]. Each call to [`Iterator::next`] decodes
/// one page at a time, matching [`Rows`](crate::Rows) without borrowing the
/// database.
#[must_use = "an iterator reads nothing until it is used"]
pub struct OwnedRows {
    database: Arc<Database>,
    table_id: u32,
    columns: Columns,
    state: ScannerState,
}

impl OwnedRows {
    pub(crate) fn new(
        database: Arc<Database>,
        definition: &TableDefinition,
        columns: Columns,
        refusal: Option<Error>,
    ) -> Self {
        let refusal = refusal.or_else(|| columns.check(definition).err());
        let state = ScannerState::new(Some(definition), refusal);
        Self {
            database,
            table_id: definition.id(),
            columns,
            state,
        }
    }

    pub(crate) fn refused(database: Arc<Database>, columns: Columns, error: Error) -> Self {
        Self {
            database,
            table_id: 0,
            columns,
            state: ScannerState::new(None, Some(error)),
        }
    }

    /// The columns that each yielded row holds.
    #[must_use]
    pub fn columns(&self) -> &Columns {
        &self.columns
    }
}

impl Iterator for OwnedRows {
    type Item = Result<Row>;

    fn next(&mut self) -> Option<Self::Item> {
        let definition = self.database.definition_by_id(self.table_id);
        let Some(definition) = definition else {
            return self.state.refusal_only();
        };
        let store = self.database.store();
        let reader = RecordReader::new(
            store,
            definition,
            self.columns.clone(),
            store.longest_value(),
        );
        self.state.next(&reader, RecordReader::row)
    }
}

impl FusedIterator for OwnedRows {}

impl fmt::Debug for OwnedRows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedRows")
            .field("table_id", &self.table_id)
            .finish_non_exhaustive()
    }
}

/// An owned, `'static` iterator over the [`RowAddress`]es of a table's live
/// rows.
///
/// Created by [`Database::row_addresses_owned`].
#[must_use = "an iterator reads nothing until it is used"]
pub struct OwnedRowAddresses {
    database: Arc<Database>,
    table_id: u32,
    columns: Columns,
    state: ScannerState,
}

impl OwnedRowAddresses {
    pub(crate) fn new(database: Arc<Database>, definition: &TableDefinition) -> Self {
        let columns = Columns::none(definition);
        let state = ScannerState::new(Some(definition), None);
        Self {
            database,
            table_id: definition.id(),
            columns,
            state,
        }
    }

    pub(crate) fn refused(database: Arc<Database>, columns: Columns, error: Error) -> Self {
        Self {
            database,
            table_id: 0,
            columns,
            state: ScannerState::new(None, Some(error)),
        }
    }
}

impl Iterator for OwnedRowAddresses {
    type Item = Result<RowAddress>;

    fn next(&mut self) -> Option<Self::Item> {
        let definition = self.database.definition_by_id(self.table_id);
        let Some(definition) = definition else {
            return self.state.refusal_only();
        };
        let store = self.database.store();
        let reader = RecordReader::new(
            store,
            definition,
            self.columns.clone(),
            store.longest_value(),
        );
        self.state.next(&reader, |reader, page, slot, _met| {
            reader.address(page, slot)
        })
    }
}

impl FusedIterator for OwnedRowAddresses {}

impl fmt::Debug for OwnedRowAddresses {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedRowAddresses")
            .field("table_id", &self.table_id)
            .finish_non_exhaustive()
    }
}
