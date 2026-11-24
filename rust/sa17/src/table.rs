//! A table of an open file.

use std::fmt;

use crate::catalog::SystemCatalog;
use crate::error::Result;
use crate::record::{Columns, RecordReader, Row, RowAddresses, Rows};
use crate::row_address::RowAddress;
use crate::schema::{Column, ForeignKey, TableDefinition};
use crate::store::PageStore;

/// A table of an open database: its definition, and the way to its rows.
///
/// A table carries its database, so its rows are read from the file that its
/// definition came from. Copying one is cheap.
#[derive(Clone, Copy)]
pub struct Table<'db> {
    store: &'db PageStore,
    catalog: &'db SystemCatalog,
    definition: &'db TableDefinition,
}

impl<'db> Table<'db> {
    pub(crate) fn new(
        store: &'db PageStore,
        catalog: &'db SystemCatalog,
        definition: &'db TableDefinition,
    ) -> Self {
        Self {
            store,
            catalog,
            definition,
        }
    }

    /// The table as the catalog describes it.
    #[must_use]
    pub fn definition(self) -> &'db TableDefinition {
        self.definition
    }

    /// The table's identifier.
    #[must_use]
    pub fn id(self) -> u32 {
        self.definition.id()
    }

    /// The table's name.
    #[must_use]
    pub fn name(self) -> &'db str {
        self.definition.name()
    }

    /// The columns, ordered by their number.
    #[must_use]
    pub fn columns(self) -> &'db [Column] {
        self.definition.columns()
    }

    /// The table that `key`, one of this table's foreign keys, names a row of.
    #[must_use]
    pub fn target(self, key: &ForeignKey) -> Option<Self> {
        let definition = self.catalog.table_by_id(key.target_id())?;
        Some(Self { definition, ..self })
    }

    /// The live rows with every column.
    pub fn rows(self) -> Rows<'db> {
        Rows::all(self.store, self.definition)
    }

    /// The live rows, cut down to `columns` of the table.
    pub fn scan(self, columns: &Columns) -> Rows<'db> {
        Rows::new(self.store, self.definition, columns.clone())
    }

    /// The addresses of the live rows, without decoding the rows.
    pub fn row_addresses(self) -> RowAddresses<'db> {
        RowAddresses::new(self.store, self.definition)
    }

    /// The live row at `address`, cut down to `columns` of the table, or `None`
    /// when there is none.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Row`](crate::Error::Row) when the page is not one of
    /// the table's and when the row cannot be decoded, and
    /// [`Error::WrongColumns`](crate::Error::WrongColumns) for columns of
    /// another table.
    pub fn row(self, address: RowAddress, columns: &Columns) -> Result<Option<Row>> {
        columns.check(self.definition)?;
        let columns = columns.clone();
        let longest = self.store.longest_value();
        RecordReader::new(self.store, self.definition, columns, longest).at(address)
    }
}

impl fmt::Debug for Table<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Table")
            .field("owner", &self.definition.owner())
            .field("name", &self.definition.name())
            .finish_non_exhaustive()
    }
}
