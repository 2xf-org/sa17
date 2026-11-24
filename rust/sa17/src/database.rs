//! An open database file.

use std::fmt;
use std::iter;
use std::path::Path;
use std::sync::Arc;

use crate::catalog::SystemCatalog;
use crate::error::{Error, Result};
use crate::identity::Identity;
use crate::open_options::OpenOptions;
use crate::record::{Columns, OwnedRowAddresses, OwnedRows, Row};
use crate::row_id::RowId;
use crate::schema::TableDefinition;
use crate::source::Source;
use crate::store::{PageStore, TextEncoding};
use crate::table::Table;
use crate::verification::Verification;

/// A database file with its key found and its catalog read.
///
/// ```no_run
/// use sa17::Database;
///
/// let database = Database::open("garden.db")?;
/// for definition in database.catalog() {
///     println!("{} has {} rows", definition.name(), definition.row_count());
/// }
/// # Ok::<(), sa17::Error>(())
/// ```
pub struct Database {
    store: PageStore,
    catalog: SystemCatalog,
}

impl Database {
    /// Opens the file at `path`. The whole file is read into memory.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`PageStore::open`], and an error for a catalog
    /// this reader cannot follow.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(path, &OpenOptions::new())
    }

    /// Opens the file at `path`, as `options` say.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`PageStore::open_with`], and an error for a
    /// catalog this reader cannot follow.
    pub fn open_with(path: impl AsRef<Path>, options: &OpenOptions) -> Result<Self> {
        Self::try_new(PageStore::open_with(path, options)?)
    }

    /// Opens a file from its bytes.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`PageStore::from_bytes`], and an error for a
    /// catalog this reader cannot follow.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        Self::from_bytes_with(bytes, &OpenOptions::new())
    }

    /// Opens a file from its bytes, as `options` say.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`PageStore::from_bytes_with`], and an error for a
    /// catalog this reader cannot follow.
    pub fn from_bytes_with(bytes: Vec<u8>, options: &OpenOptions) -> Result<Self> {
        Self::try_new(PageStore::from_bytes_with(bytes, options)?)
    }

    /// Reads the catalog of `store`.
    ///
    /// # Errors
    ///
    /// Returns an error for a catalog this reader cannot follow.
    pub fn try_new(store: PageStore) -> Result<Self> {
        let catalog = SystemCatalog::load(&store)?;
        Ok(Self { store, catalog })
    }

    /// The file's pages.
    #[must_use]
    pub fn store(&self) -> &PageStore {
        &self.store
    }

    /// The definition of every table, view and temporary table of the
    /// catalog: those that store rows, then the others, each ordered by
    /// identifier.
    #[must_use]
    pub fn catalog(&self) -> &[TableDefinition] {
        self.catalog.tables()
    }

    /// Every entry of [`Self::catalog`], as a table to read.
    pub fn tables(&self) -> impl ExactSizeIterator<Item = Table<'_>> {
        self.catalog()
            .iter()
            .map(|definition| self.handle(definition))
    }

    /// Why a part of the catalog could not be read, the first thousand reasons.
    /// The file opens without that part: a table is not listed, has no rows to
    /// read or no owner's name, a column has no value in older rows, or a key is
    /// not declared.
    #[must_use]
    pub fn catalog_problems(&self) -> &[Error] {
        self.catalog.problems()
    }

    /// The table that `name` means, ignoring ASCII case: one of that name, or
    /// one whose owner's name and own name it is, with a point between them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTable`] when there is no such table, and
    /// [`Error::AmbiguousTable`] when more than one can be meant. Where more
    /// than one user owns a table of a name, that is so until the owner is in
    /// it. [`Self::table_by_id`] tells apart those that no name does.
    pub fn table(&self, name: &str) -> Result<Table<'_>> {
        Ok(self.handle(self.catalog.table(name)?))
    }

    /// The table with identifier `id`.
    #[must_use]
    pub fn table_by_id(&self, id: u32) -> Option<Table<'_>> {
        Some(self.handle(self.catalog.table_by_id(id)?))
    }

    /// Checks every page's checksum, reads every row of every table, and
    /// compares each table's rows with the number the catalog records. What
    /// could not be read of the catalog is part of what it finds.
    #[must_use]
    pub fn verify(&self) -> Verification {
        Verification::new(&self.store, &self.catalog)
    }

    /// Streams the live rows of `table`, cut down to `columns`, holding an
    /// [`Arc`] handle to this database so the iterator is `'static`.
    pub fn scan_owned(self: &Arc<Self>, table: &TableDefinition, columns: &Columns) -> OwnedRows {
        match self.own(table) {
            Ok(own) => OwnedRows::new(Arc::clone(self), own.definition(), columns.clone(), None),
            Err(error) => OwnedRows::refused(Arc::clone(self), columns.clone(), error),
        }
    }

    /// Streams the addresses of the live rows of `table`, holding an [`Arc`]
    /// handle to this database so the iterator is `'static`.
    pub fn row_addresses_owned(self: &Arc<Self>, table: &TableDefinition) -> OwnedRowAddresses {
        match self.own(table) {
            Ok(own) => OwnedRowAddresses::new(Arc::clone(self), own.definition()),
            Err(error) => OwnedRowAddresses::refused(Arc::clone(self), Columns::none(table), error),
        }
    }

    pub(crate) fn definition_by_id(&self, id: u32) -> Option<&TableDefinition> {
        self.catalog.table_by_id(id)
    }

    /// The table of this database that `definition` is the definition of.
    fn own(&self, definition: &TableDefinition) -> Result<Table<'_>> {
        let own = self.catalog.table_by_id(definition.id());
        let own = own.filter(|own| own.shares_columns(definition));
        own.map(|own| self.handle(own))
            .ok_or_else(|| Error::WrongDatabase {
                table: definition.name().to_owned(),
            })
    }

    fn handle<'db>(&'db self, definition: &'db TableDefinition) -> Table<'db> {
        Table::new(&self.store, &self.catalog, definition)
    }
}

impl Source for Database {
    fn definitions(&self) -> &[TableDefinition] {
        self.catalog.stored()
    }

    fn scan<'source>(
        &'source self,
        table: &'source TableDefinition,
        columns: &'source Columns,
    ) -> Box<dyn Iterator<Item = Result<Row>> + Send + 'source> {
        match self.own(table) {
            Ok(own) => Box::new(own.scan(columns)),
            Err(error) => Box::new(iter::once(Err(error))),
        }
    }

    fn fetch(&self, table: &TableDefinition, columns: &Columns, id: RowId) -> Result<Option<Row>> {
        let table = self.own(table)?;
        match id.address() {
            Some(address) => table.row(address, columns),
            None => columns.check(table.definition()).map(|()| None),
        }
    }

    fn text_encoding(&self) -> TextEncoding {
        self.store.text_encoding()
    }

    fn identity(&self) -> Option<Identity> {
        Some(self.store.identity())
    }
}

impl fmt::Debug for Database {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Database")
            .field("store", &self.store)
            .field("tables", &self.catalog().len())
            .finish()
    }
}
