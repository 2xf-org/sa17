//! The system catalog: every table of the file and its columns.
//!
//! The catalog is two tables that describe every table, themselves included.
//! `ISYSTAB` has one row per table and `ISYSTABCOL` one row per column.
//! Reading starts from the layout SQL Anywhere 17 gives both, and ends by
//! checking that the catalog describes the same layout for them.

use crate::catalog::Problems;
use crate::catalog::columns;
use crate::catalog::keys;
use crate::catalog::table_names;
use crate::catalog::tables;
use crate::catalog::users;
use crate::error::{Error, Result};
use crate::schema::TableDefinition;
use crate::store::PageStore;

/// Every table of a file.
pub(crate) struct SystemCatalog {
    /// The tables that store rows, then the others, each ordered by identifier.
    tables: Vec<TableDefinition>,
    /// How many of them store rows.
    stored: usize,
    problems: Problems,
}

impl SystemCatalog {
    /// Reads the catalog of `store`.
    pub(crate) fn load(store: &PageStore) -> Result<Self> {
        let mut problems = Problems::default();
        let rows = tables::read(store, &mut problems)?;
        let mut columns = columns::read(store, &rows, &mut problems)?;
        let mut tables: Vec<TableDefinition> = rows
            .into_iter()
            .map(|row| TableDefinition {
                columns: columns.remove(&row.id).unwrap_or_default().into(),
                id: row.id,
                name: row.name,
                kind: row.kind,
                owner_id: row.owner_id,
                owner: String::new(),
                system: false,
                row_count: row.row_count,
                schema: row.schema,
                pages: row.pages,
                extension_pages: row.extension_pages,
                primary_key: Vec::new(),
                foreign_keys: Vec::new(),
                indexes: Vec::new(),
            })
            .collect();
        keys::give(store, &mut tables, &mut problems);
        users::give(store, &mut tables, &mut problems);

        tables.sort_by_key(|table| (!table.stores_rows(), table.id));
        let stored = tables.partition_point(TableDefinition::stores_rows);
        Ok(Self {
            tables,
            stored,
            problems,
        })
    }

    /// Every table: those that store rows, then the others, each ordered by
    /// identifier.
    pub(crate) fn tables(&self) -> &[TableDefinition] {
        &self.tables
    }

    /// The tables that store rows, ordered by identifier.
    pub(crate) fn stored(&self) -> &[TableDefinition] {
        let (stored, _) = self.tables.split_at(self.stored);
        stored
    }

    /// What could not be read.
    pub(crate) fn problems(&self) -> &[Error] {
        self.problems.as_slice()
    }

    /// The table with identifier `id`.
    pub(crate) fn table_by_id(&self, id: u32) -> Option<&TableDefinition> {
        let (stored, others) = self.tables.split_at(self.stored);
        [stored, others].into_iter().find_map(|tables| {
            let position = tables.binary_search_by_key(&id, TableDefinition::id);
            tables.get(position.ok()?)
        })
    }

    /// The table that `name` means.
    pub(crate) fn table(&self, name: &str) -> Result<&TableDefinition> {
        table_names::find(&self.tables, name)
    }
}
