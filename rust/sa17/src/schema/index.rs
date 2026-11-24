//! The indexes of a table, and the keys they stand behind.

/// An index as the catalog describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) kind: IndexKind,
    pub(crate) unique: bool,
    pub(crate) columns: Vec<IndexColumn>,
}

/// What an index is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IndexKind {
    /// The index behind the table's primary key.
    PrimaryKey,
    /// The index behind one of the table's foreign keys.
    ForeignKey,
    /// An index of its own, or one behind a `UNIQUE` constraint.
    Secondary,
    /// A text index.
    Text,
    /// A kind this reader does not name.
    Other,
}

impl IndexKind {
    pub(crate) fn from_catalog(category: u64) -> Self {
        match category {
            1 => Self::PrimaryKey,
            2 => Self::ForeignKey,
            3 => Self::Secondary,
            4 => Self::Text,
            _ => Self::Other,
        }
    }
}

/// One column of an index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IndexColumn {
    pub(crate) position: usize,
    pub(crate) order: IndexOrder,
}

/// The order that an index keeps a column in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexOrder {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

impl Index {
    /// The index's number within its table.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The index's name. The engine names the index behind a key after the
    /// key.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What the index is for.
    #[must_use]
    pub fn kind(&self) -> IndexKind {
        self.kind
    }

    /// Whether no two rows may share the values of the index's columns.
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.unique
    }

    /// The columns, in the index's order.
    #[must_use]
    pub fn columns(&self) -> &[IndexColumn] {
        &self.columns
    }
}

impl IndexColumn {
    /// The column's position among the table's columns.
    #[must_use]
    pub fn position(self) -> usize {
        self.position
    }

    /// The order the index keeps the column in.
    #[must_use]
    pub fn order(self) -> IndexOrder {
        self.order
    }
}
