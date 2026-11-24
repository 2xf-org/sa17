//! One table of the catalog.

use std::sync::Arc;

use crate::error::Error;
use crate::schema::{Column, ForeignKey, Index, PageSet};

/// What kind of object a catalog table entry describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TableKind {
    /// An ordinary table. Its rows are in the file.
    Base,
    /// A materialized view. Its rows are in the file.
    MaterializedView,
    /// A global temporary table. Only its definition is in the file.
    GlobalTemporary,
    /// The table behind a text index, which the engine fills. How its rows are
    /// stored has not been seen in a file, so this reader reads only its
    /// definition.
    TextIndex,
    /// A view. Only its definition is in the file.
    View,
    /// A kind this reader does not name.
    Other,
}

impl TableKind {
    pub(crate) fn from_catalog(table_type: u64) -> Self {
        match table_type {
            1 => Self::Base,
            2 => Self::MaterializedView,
            3 => Self::GlobalTemporary,
            5 => Self::TextIndex,
            21 => Self::View,
            _ => Self::Other,
        }
    }

    /// A short name for listings.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Base => "table",
            Self::MaterializedView => "materialized view",
            Self::GlobalTemporary => "temporary table",
            Self::TextIndex => "text index table",
            Self::View => "view",
            Self::Other => "other",
        }
    }
}

/// A table as the catalog describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct TableDefinition {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) kind: TableKind,
    pub(crate) owner_id: u32,
    pub(crate) owner: String,
    pub(crate) system: bool,
    /// Shared with whoever reads rows into some of them.
    pub(crate) columns: Arc<[Column]>,
    pub(crate) row_count: u64,
    pub(crate) schema: u32,
    pub(crate) pages: PageSet,
    pub(crate) extension_pages: PageSet,
    pub(crate) primary_key: Vec<usize>,
    pub(crate) foreign_keys: Vec<ForeignKey>,
    pub(crate) indexes: Vec<Index>,
}

impl TableDefinition {
    /// The table's identifier. Every page of the table carries it.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The table's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What kind of object the entry describes.
    #[must_use]
    pub fn kind(&self) -> TableKind {
        self.kind
    }

    /// The identifier of the user that owns the table.
    #[must_use]
    pub fn owner_id(&self) -> u32 {
        self.owner_id
    }

    /// The name of the user that owns the table, such as `SYS`. It is empty
    /// when the catalog does not list the user.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The columns, ordered by their number.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The column called `name`, ignoring ASCII case.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.get(self.position(name)?)
    }

    /// The position in [`Self::columns`] of the column called `name`,
    /// ignoring ASCII case.
    #[must_use]
    pub fn position(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|column| column.name.eq_ignore_ascii_case(name))
    }

    /// The positions in [`Self::columns`] of the columns whose values together
    /// tell the table's rows apart, if the table declares any.
    #[must_use]
    pub fn primary_key(&self) -> Option<&[usize]> {
        (!self.primary_key.is_empty()).then_some(&self.primary_key)
    }

    /// The columns that hold the keys of rows elsewhere, in the catalog's order.
    #[must_use]
    pub fn foreign_keys(&self) -> &[ForeignKey] {
        &self.foreign_keys
    }

    /// The indexes, ordered by their number. Every key has one behind it.
    #[must_use]
    pub fn indexes(&self) -> &[Index] {
        &self.indexes
    }

    /// Why nothing says which pages the table's rows are on.
    pub(crate) fn unlisted(&self) -> Option<Error> {
        let problem = self.pages.problem()?;
        Some(Error::PageList {
            table: self.name.clone(),
            problem,
        })
    }

    /// Whether both describe one table of one file.
    pub(crate) fn shares_columns(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.columns, &other.columns)
    }

    /// Whether one of the engine's own users owns the table, such as `SYS` and
    /// `dbo`. Those tables hold the catalog and the engine's settings.
    #[must_use]
    pub fn is_system(&self) -> bool {
        self.system
    }

    /// Whether the file stores rows for the table that this reader reads. It
    /// keeps only the definition of a view or of a temporary table.
    #[must_use]
    pub fn stores_rows(&self) -> bool {
        matches!(self.kind, TableKind::Base | TableKind::MaterializedView)
    }

    /// The number of rows the catalog records for the table.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    /// The table's current schema version. Each `ALTER TABLE` that adds a
    /// column raises it, and each row carries the version it was written with.
    #[must_use]
    pub fn schema(&self) -> u32 {
        self.schema
    }

    /// The pages that hold the table's rows, in ascending order. There is none
    /// when the list of them is damaged: reading the table says so.
    #[must_use]
    pub fn pages(&self) -> &[u32] {
        self.pages.numbers()
    }

    /// The pages that hold its moved rows and the pieces of its long values, in
    /// ascending order. The catalog does not list those of `ISYSTAB`, the table
    /// that has the lists.
    #[must_use]
    pub fn extension_pages(&self) -> &[u32] {
        self.extension_pages.numbers()
    }
}
