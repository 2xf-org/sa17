//! What answers for the stored tables of a database.

use std::sync::Arc;

use crate::catalog::table_names;
use crate::error::Result;
use crate::identity::Identity;
use crate::record::{Columns, Row};
use crate::row_id::RowId;
use crate::schema::TableDefinition;
use crate::store::TextEncoding;

/// What answers for the stored tables of a database: a file, or a file with
/// changes laid over it.
///
/// A [`Database`](crate::Database) is one. Code that takes a `&dyn Source`
/// reads either, and can do so from many threads. A reference, a `Box` or an
/// `Arc` of a source is a source, too.
///
/// # Implementing it
///
/// A source of its own wraps another, as one that lays changes over a database
/// does. Nothing else can: only a database has definitions to give. Whoever
/// wraps one keeps what its callers rely on:
///
/// - [`Self::definitions`] are the wrapped source's, in its order, and they
///   stay where and as they are for as long as the source is there.
/// - [`Self::scan`] gives each row once, and [`Self::fetch`] the same row for
///   the id it has there, cut down to the columns asked for. A row that a
///   source has and a file does not is [`RowId::Added`], with a number that no
///   other row of its table has, or had while the source was there.
/// - Columns that [`Columns::check`] refuses for the table are its error, from
///   both of them and for any row. A definition of another database is
///   [`Error::WrongDatabase`](crate::Error::WrongDatabase), which the wrapped
///   source gives.
/// - An error is an item of a scan in the place of what could not be read, and
///   the scan goes on after it.
/// - [`Self::identity`] is `None` as soon as any answer differs from the
///   wrapped source's.
pub trait Source: Send + Sync {
    /// The definition of every table that stores rows, ordered by identifier.
    fn definitions(&self) -> &[TableDefinition];

    /// The live rows of `table`, cut down to `columns` of it, in the order the
    /// source keeps them: a file's in storage order.
    ///
    /// A row that cannot be read is an `Err` item, and the iterator goes on.
    /// Columns of another table are one `Err` item, and nothing after it.
    fn scan<'source>(
        &'source self,
        table: &'source TableDefinition,
        columns: &'source Columns,
    ) -> Box<dyn Iterator<Item = Result<Row>> + Send + 'source>;

    /// One row of `table`, cut down to `columns` of it, or `None` when the
    /// source has no such row, or no longer.
    ///
    /// # Errors
    ///
    /// Returns an error when the row cannot be read, when it is on a page that
    /// is not one of the table's, and for columns of another table, whichever
    /// row is asked for.
    fn fetch(&self, table: &TableDefinition, columns: &Columns, id: RowId) -> Result<Option<Row>>;

    /// The encoding of the database's character columns.
    fn text_encoding(&self) -> TextEncoding;

    /// The identity of the file that the source answers for, exactly as the
    /// file is. `None` when the source does not say, or answers differently,
    /// as a file with changes laid over it does.
    fn identity(&self) -> Option<Identity> {
        None
    }

    /// The definition of the table that `name` means among those that store
    /// rows, as [`Database::table`](crate::Database::table) finds it among all.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTable`](crate::Error::UnknownTable) when there
    /// is no such table, and
    /// [`Error::AmbiguousTable`](crate::Error::AmbiguousTable) when more than
    /// one can be meant.
    fn definition(&self, name: &str) -> Result<&TableDefinition> {
        table_names::find(self.definitions(), name)
    }

    /// The definition of the table with identifier `id`, when it is one that
    /// stores rows.
    fn definition_by_id(&self, id: u32) -> Option<&TableDefinition> {
        let definitions = self.definitions();
        let position = definitions.binary_search_by_key(&id, TableDefinition::id);
        definitions.get(position.ok()?)
    }
}

macro_rules! answer_through {
    ($($pointer:ty),*) => {$(
        impl<S: Source + ?Sized> Source for $pointer {
            fn definitions(&self) -> &[TableDefinition] {
                (**self).definitions()
            }

            fn scan<'source>(
                &'source self,
                table: &'source TableDefinition,
                columns: &'source Columns,
            ) -> Box<dyn Iterator<Item = Result<Row>> + Send + 'source> {
                (**self).scan(table, columns)
            }

            fn fetch(
                &self,
                table: &TableDefinition,
                columns: &Columns,
                id: RowId,
            ) -> Result<Option<Row>> {
                (**self).fetch(table, columns, id)
            }

            fn text_encoding(&self) -> TextEncoding {
                (**self).text_encoding()
            }

            fn identity(&self) -> Option<Identity> {
                (**self).identity()
            }

            fn definition(&self, name: &str) -> Result<&TableDefinition> {
                (**self).definition(name)
            }

            fn definition_by_id(&self, id: u32) -> Option<&TableDefinition> {
                (**self).definition_by_id(id)
            }
        }
    )*};
}

answer_through!(&S, Box<S>, Arc<S>);
