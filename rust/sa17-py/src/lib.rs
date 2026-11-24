//! Python bindings for `sa17`, generated with Bimetal.

mod arrow_bridge;
mod convert;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use bimetal::Model;
use bimetal::arrow::{RecordBatchStream, Schema as ArrowSchema, Table as ArrowTable};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Sign of a database file's page key formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum KeySign {
    /// Positive page keys.
    Positive,
    /// Negative page keys.
    Negative,
}

/// Constants that generate the scrambling key for every page in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct StoreKey {
    /// 25-bit XOR mask applied to the page number.
    pub mask: u32,
    /// 25-bit addend applied after the mask.
    pub addend: u32,
    /// Sign that selects the sector walk direction.
    pub sign: KeySign,
}

/// Options controlling how a database file or page store is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct OpenOptions {
    /// Explicit page key to use instead of searching for one.
    #[serde(default)]
    pub key: Option<StoreKey>,
    /// Maximum decompressed byte length allowed for a single compressed value
    /// (defaults to 16 MiB).
    #[serde(default)]
    pub longest_value: Option<u32>,
}

/// Character encoding declared by the database superblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
pub enum TextEncoding {
    /// Windows-1252 single-byte encoding.
    #[serde(rename = "windows-1252")]
    Windows1252,
    /// UTF-8 encoding.
    #[serde(rename = "utf-8")]
    Utf8,
}

/// Allocation and checksum status of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum PageState {
    /// Page contains data whose CRC-32 matches.
    Written,
    /// Page is all zero bytes and was never written.
    Unwritten,
}

/// Structural kind of a decoded page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    /// Rows belonging directly to a table.
    Table,
    /// Forwarded rows and pieces of long values.
    Extension,
    /// Serialized page list of `ISYSTAB`.
    PageList,
    /// Index, allocation map, or other non-row page.
    Other,
}

/// Kind of object described by a catalog table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    /// Ordinary base table.
    Base,
    /// Materialized view with stored rows.
    MaterializedView,
    /// Global temporary table (definition only).
    GlobalTemporary,
    /// Backing table of a text index (definition only).
    TextIndex,
    /// View (definition only).
    View,
    /// Unrecognized catalog table kind.
    Other,
}

/// Unit counted by a character or binary column's declared width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    /// Width counts bytes.
    Bytes,
    /// Width counts characters.
    Characters,
}

/// Role of a catalog index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum IndexKind {
    /// Index backing a primary key.
    PrimaryKey,
    /// Index backing a foreign key.
    ForeignKey,
    /// Secondary or unique index.
    Secondary,
    /// Text index.
    Text,
    /// Other index kind.
    Other,
}

/// Sort direction of a column in an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum IndexOrder {
    /// Ascending order.
    Ascending,
    /// Descending order.
    Descending,
}

/// Header fields read from page 0 of the database file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct Superblock {
    /// Format version string, such as `"3.201.12"`.
    pub version: String,
    /// Log sequence number recorded in page 0.
    pub lsn: u32,
    /// Number of pages belonging to the database before trailing checkpoint copies.
    pub database_page_count: u32,
}

/// Content fingerprint distinguishing a database file and its write state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct Identity {
    /// Total number of pages in the file.
    pub page_count: u32,
    /// Log sequence number from page 0.
    pub lsn: u32,
    /// Formatted identity string including the 128-bit digest.
    pub digest: String,
}

/// Physical location of a row inside the database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct RowAddress {
    /// Zero-based page number.
    pub page: u32,
    /// Zero-based slot index within the page.
    pub slot: u16,
}

/// Catalog metadata for a single table column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct ColumnInfo {
    /// Column identifier within its table.
    pub id: u32,
    /// Column name.
    pub name: String,
    /// SQL type name without width or precision.
    pub data_type: String,
    /// Formatted SQL type including width or precision and scale.
    pub sql_type: String,
    /// Raw catalog domain identifier.
    pub domain: u16,
    /// Declared width, precision, or fixed storage size.
    pub width: u32,
    /// Decimal scale, or character-width flag for character columns.
    pub scale: u16,
    /// Unit counted by `width` for length-limited string and binary columns.
    pub length_unit: Option<LengthUnit>,
    /// Whether the column accepts `NULL`.
    pub nullable: bool,
    /// Text of the column's `DEFAULT` expression, if declared.
    pub default_expression: Option<String>,
    /// Schema version in which the column was added.
    pub first_schema: u32,
}

/// Catalog metadata for a foreign key constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct ForeignKeyInfo {
    /// Index name backing the foreign key.
    pub name: String,
    /// Zero-based column positions in the referencing table.
    pub positions: Vec<u32>,
    /// Column names in the referencing table.
    pub columns: Vec<String>,
    /// Catalog identifier of the referenced table.
    pub target_id: u32,
    /// Name of the referenced table, if present in the catalog.
    pub target_table: Option<String>,
    /// Zero-based column positions in the referenced table.
    pub target_positions: Vec<u32>,
    /// Column names in the referenced table.
    pub target_columns: Vec<String>,
}

/// One column belonging to an index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct IndexColumnInfo {
    /// Zero-based position of the column in its table.
    pub position: u32,
    /// Name of the column.
    pub name: String,
    /// Sort order of the column in the index.
    pub order: IndexOrder,
}

/// Catalog metadata for a table index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct IndexInfo {
    /// Index identifier within its table.
    pub id: u32,
    /// Index name.
    pub name: String,
    /// Role of the index.
    pub kind: IndexKind,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
    /// Indexed columns in key order.
    pub columns: Vec<IndexColumnInfo>,
}

/// Complete catalog metadata for a table, view, or temporary table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct TableInfo {
    /// Catalog table identifier.
    pub id: u32,
    /// Table name.
    pub name: String,
    /// Catalog identifier of the owning user.
    pub owner_id: u32,
    /// Owning user name, or empty if unlisted.
    pub owner: String,
    /// Kind of catalog object.
    pub kind: TableKind,
    /// Whether the table is owned by a built-in engine user.
    pub is_system: bool,
    /// Whether the reader reads rows for this object kind.
    pub stores_rows: bool,
    /// Row count recorded in the catalog.
    pub row_count: u64,
    /// Current schema version of the table.
    pub schema_version: u32,
    /// Columns in catalog order.
    pub columns: Vec<ColumnInfo>,
    /// Zero-based positions of the primary key columns.
    pub primary_key: Vec<u32>,
    /// Names of the primary key columns.
    pub primary_key_columns: Vec<String>,
    /// Foreign keys declared on the table.
    pub foreign_keys: Vec<ForeignKeyInfo>,
    /// Indexes declared on the table.
    pub indexes: Vec<IndexInfo>,
    /// Page numbers storing rows of the table.
    pub pages: Vec<u32>,
    /// Extension page numbers storing forwarded rows and long values.
    pub extension_pages: Vec<u32>,
}

/// Header summary for a raw record slot on a decoded page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct RecordInfo {
    /// Page and slot of the record.
    pub address: RowAddress,
    /// Raw record flag byte.
    pub flags: u8,
    /// Byte length of the record.
    pub length: u32,
}

/// Category of a problem found during full-database verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Model)]
#[serde(rename_all = "snake_case")]
pub enum VerificationProblemKind {
    /// Part of the system catalog could not be read.
    Catalog,
    /// A page failed its checksum or could not be checked.
    Page,
    /// One or more rows or pages of a table could not be decoded.
    Rows,
    /// Live row count differs from the catalog's recorded count.
    RowCount,
}

/// Single integrity issue reported by `Database.verify()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct VerificationProblem {
    /// Category of the issue.
    pub kind: VerificationProblemKind,
    /// Human-readable description of the issue.
    pub message: String,
    /// Table name, when the issue pertains to a table.
    pub table: Option<String>,
}

/// Result of checking every page checksum, catalog entry, and table row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Model)]
pub struct Verification {
    /// True when no catalog, page, row, or count problems were found.
    pub is_sound: bool,
    /// Number of catalog issues encountered.
    pub catalog_problems: u64,
    /// Total pages checked.
    pub pages: u64,
    /// Zero-filled unwritten pages.
    pub unwritten_pages: u64,
    /// Pages failing CRC-32 verification.
    pub damaged_pages: u64,
    /// Tables scanned.
    pub tables: u64,
    /// Tables whose readable row count differed from the catalog count.
    pub differing_tables: u64,
    /// Rows successfully decoded.
    pub rows: u64,
    /// Rows that failed to decode on readable pages.
    pub unreadable_rows: u64,
    /// Table pages that could not be read.
    pub unreadable_pages: u64,
    /// First hundred reported problems.
    pub problems: Vec<VerificationProblem>,
}

/// Errors returned when opening or reading a SQL Anywhere 17 database file.
#[derive(Debug, thiserror::Error, bimetal::Error)]
pub enum Sa17Error {
    /// The operating system could not read the file.
    #[error("{message}")]
    #[bimetal(details)]
    Io {
        /// OS error description.
        message: String,
    },
    /// The file is not a supported SQL Anywhere 17 database.
    #[error("{message}")]
    #[bimetal(details)]
    Format {
        /// Format validation message.
        message: String,
    },
    /// No scrambling key matches the file's pages.
    #[error("no page key fits the pages of this file")]
    NoKey,
    /// Multiple scrambling keys fit equally well; supply one in `OpenOptions`.
    #[error("more than one page key fits the pages of this file")]
    #[bimetal(details)]
    AmbiguousKey {
        /// Candidate keys found by the solver.
        keys: Vec<StoreKey>,
    },
    /// The key passed in `OpenOptions` does not fit the file's pages.
    #[error("the page key does not fit the pages of this file")]
    WrongKey,
    /// The database declares an unsupported character encoding.
    #[error("unsupported text encoding {label:?}")]
    #[bimetal(details)]
    UnsupportedEncoding {
        /// Encoding name stored in the superblock.
        label: String,
    },
    /// A page could not be read or failed its checksum.
    #[error("{message}")]
    #[bimetal(details)]
    Page {
        /// Zero-based page number.
        page: u32,
        /// Failure details.
        message: String,
    },
    /// A table page could not be read.
    #[error("{message}")]
    #[bimetal(details)]
    TablePage {
        /// Table name.
        table: String,
        /// Zero-based page number.
        page: u32,
        /// Failure details.
        message: String,
    },
    /// A single row could not be decoded.
    #[error("{message}")]
    #[bimetal(details)]
    Row {
        /// Table name.
        table: String,
        /// Zero-based page number.
        page: u32,
        /// Zero-based slot index.
        slot: u16,
        /// Failure details.
        message: String,
    },
    /// A table's page list could not be decoded.
    #[error("{message}")]
    #[bimetal(details)]
    PageList {
        /// Table name.
        table: String,
        /// Failure details.
        message: String,
    },
    /// The system catalog is invalid or inconsistent.
    #[error("{message}")]
    #[bimetal(details)]
    Catalog {
        /// Failure details.
        message: String,
    },
    /// No table matches the requested name.
    #[error("there is no table called {name}")]
    #[bimetal(details)]
    UnknownTable {
        /// Requested table name.
        name: String,
    },
    /// More than one table matches the requested name; qualify it as `owner.table`.
    #[error("more than one table is called {name}: {}", candidates.join(", "))]
    #[bimetal(details)]
    AmbiguousTable {
        /// Requested table name.
        name: String,
        /// Matching qualified table names.
        candidates: Vec<String>,
    },
    /// The table has no column with the requested name.
    #[error("the table {table} has no column called {column}")]
    #[bimetal(details)]
    UnknownColumn {
        /// Table name.
        table: String,
        /// Requested column name.
        column: String,
    },
    /// The table has no column at the requested position.
    #[error("the table {table} has no column at position {position}")]
    #[bimetal(details)]
    UnknownPosition {
        /// Table name.
        table: String,
        /// Requested zero-based position.
        position: u32,
    },
    /// A row value did not match the requested type.
    #[error("{message}")]
    #[bimetal(details)]
    Value {
        /// Table name.
        table: String,
        /// Column name.
        column: String,
        /// Failure details.
        message: String,
    },
    /// Converting table data to Apache Arrow failed.
    #[error("{message}")]
    #[bimetal(details)]
    Arrow {
        /// Failure details.
        message: String,
    },
}

/// An open SQL Anywhere 17 database file with its key recovered and catalog
/// loaded.
pub struct Database {
    inner: Arc<sa17::Database>,
}

/// An open SQL Anywhere 17 database file.
///
/// Use `Database.open(path)` (or `Database(path)`) to load a file from disk, or
/// `Database.from_bytes(data)` for in-memory bytes. Supports `with` blocks,
/// `len(db)`, `name in db`, and `db[name]`.
#[bimetal::export]
impl Database {
    /// Open a database file from disk.
    #[bimetal(constructor)]
    pub fn new(path: PathBuf, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        Self::open(path, options)
    }

    /// Open a database file from disk.
    #[bimetal(constructor)]
    pub fn open(path: PathBuf, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        let options = convert::open_options(options);
        let inner = sa17::Database::open_with(path, &options).map_err(convert::error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Open a database from raw file bytes already in memory.
    #[bimetal(constructor)]
    pub fn from_bytes(data: Vec<u8>, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        let options = convert::open_options(options);
        let inner = sa17::Database::from_bytes_with(data, &options).map_err(convert::error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Superblock metadata from page 0.
    #[bimetal(getter)]
    #[must_use]
    pub fn superblock(&self) -> Superblock {
        convert::superblock(self.inner.store().superblock())
    }

    /// Character encoding declared by the file.
    #[bimetal(getter)]
    #[must_use]
    pub fn text_encoding(&self) -> TextEncoding {
        convert::text_encoding(self.inner.store().text_encoding())
    }

    /// Content fingerprint identifying the file and its current write state.
    #[bimetal(getter)]
    #[must_use]
    pub fn identity(&self) -> Identity {
        convert::identity(self.inner.store().identity())
    }

    /// Recovered page-scrambling key.
    #[bimetal(getter)]
    #[must_use]
    pub fn key(&self) -> StoreKey {
        convert::store_key_from_core(self.inner.store().key())
    }

    /// Non-fatal problems encountered while reading the system catalog.
    #[bimetal(getter)]
    #[must_use]
    pub fn catalog_problems(&self) -> Vec<String> {
        self.inner
            .catalog_problems()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Catalog metadata for every table, view, and temporary table.
    #[must_use]
    pub fn catalog(&self) -> Vec<TableInfo> {
        self.inner
            .catalog()
            .iter()
            .map(|definition| convert::table_info(&self.inner, definition))
            .collect()
    }

    /// Handles for every entry in the catalog.
    #[must_use]
    pub fn tables(&self) -> Vec<Table> {
        self.inner
            .catalog()
            .iter()
            .map(|definition| Table {
                database: Arc::clone(&self.inner),
                table_id: definition.id(),
            })
            .collect()
    }

    /// Look up a table by name (ASCII case-insensitive), optionally qualified
    /// as `owner.table`.
    pub fn table(&self, name: &str) -> Result<Table, Sa17Error> {
        let table = self.inner.table(name).map_err(convert::error)?;
        Ok(Table {
            database: Arc::clone(&self.inner),
            table_id: table.id(),
        })
    }

    /// Look up a table by its catalog identifier.
    #[must_use]
    pub fn table_by_id(&self, id: u32) -> Option<Table> {
        let table = self.inner.table_by_id(id)?;
        Some(Table {
            database: Arc::clone(&self.inner),
            table_id: table.id(),
        })
    }

    /// Look up a table with `db[name]`, raising `KeyError` if absent.
    #[bimetal(getitem)]
    pub fn get_table(&self, name: &str) -> Result<Option<Table>, Sa17Error> {
        match self.inner.table(name) {
            Ok(table) => Ok(Some(Table {
                database: Arc::clone(&self.inner),
                table_id: table.id(),
            })),
            Err(sa17::Error::UnknownTable { .. }) => Ok(None),
            Err(error) => Err(convert::error(error)),
        }
    }

    /// Whether the catalog contains a unique table matching `name`.
    #[bimetal(contains)]
    #[must_use]
    pub fn has_table(&self, name: &str) -> bool {
        self.inner.table(name).is_ok()
    }

    /// Number of entries in the system catalog.
    #[bimetal(len)]
    #[must_use]
    pub fn table_count(&self) -> usize {
        self.inner.catalog().len()
    }

    /// Verify every page checksum, decode every row, and compare counts against
    /// the catalog.
    #[must_use]
    pub fn verify(&self) -> Verification {
        convert::verification(&self.inner.verify())
    }

    /// Return a handle to the underlying decoded page store.
    #[must_use]
    pub fn store(&self) -> PageStore {
        PageStore {
            inner: PageStoreBacking::Database(Arc::clone(&self.inner)),
        }
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        let superblock = self.inner.store().superblock();
        format!(
            "Database(version={:?}, pages={}, tables={})",
            superblock.version().to_string(),
            self.inner.store().page_count(),
            self.inner.catalog().len()
        )
    }
}

/// Handle to a single table of an open database.
pub struct Table {
    database: Arc<sa17::Database>,
    table_id: u32,
}

impl Table {
    fn core(&self) -> sa17::Table<'_> {
        self.database
            .table_by_id(self.table_id)
            .expect("table handle references an existing catalog table")
    }

    fn resolve_columns(&self, names: Option<Vec<String>>) -> Result<sa17::Columns, Sa17Error> {
        let definition = self.core().definition();
        match names {
            None => Ok(sa17::Columns::all(definition)),
            Some(names) => sa17::Columns::new(definition, names.iter().map(String::as_str))
                .map_err(convert::error),
        }
    }
}

/// A table of an open database, exposing schema metadata, streaming row
/// iteration, random row lookup, and zero-copy Apache Arrow export.
#[bimetal::export]
impl Table {
    /// Complete catalog metadata for this table.
    #[bimetal(getter)]
    #[must_use]
    pub fn info(&self) -> TableInfo {
        convert::table_info(&self.database, self.core().definition())
    }

    /// Catalog identifier of the table.
    #[bimetal(getter)]
    #[must_use]
    pub fn id(&self) -> u32 {
        self.table_id
    }

    /// Table name.
    #[bimetal(getter)]
    #[must_use]
    pub fn name(&self) -> String {
        self.core().name().to_owned()
    }

    /// Name of the user owning the table.
    #[bimetal(getter)]
    #[must_use]
    pub fn owner(&self) -> String {
        self.core().definition().owner().to_owned()
    }

    /// Kind of catalog object.
    #[bimetal(getter)]
    #[must_use]
    pub fn kind(&self) -> TableKind {
        convert::table_kind(self.core().definition().kind())
    }

    /// Whether the table belongs to a built-in engine user.
    #[bimetal(getter)]
    #[must_use]
    pub fn is_system(&self) -> bool {
        self.core().definition().is_system()
    }

    /// Whether the file stores rows for this catalog entry.
    #[bimetal(getter)]
    #[must_use]
    pub fn stores_rows(&self) -> bool {
        self.core().definition().stores_rows()
    }

    /// Row count recorded in the catalog.
    #[bimetal(getter)]
    #[must_use]
    pub fn row_count(&self) -> u64 {
        self.core().definition().row_count()
    }

    /// Columns of the table in catalog order.
    #[bimetal(getter)]
    #[must_use]
    pub fn columns(&self) -> Vec<ColumnInfo> {
        self.core()
            .columns()
            .iter()
            .map(convert::column_info)
            .collect()
    }

    /// Names of the primary key columns in key order.
    #[bimetal(getter)]
    #[must_use]
    pub fn primary_key(&self) -> Vec<String> {
        let definition = self.core().definition();
        definition
            .primary_key()
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|&pos| definition.columns().get(pos))
                    .map(|column| column.name().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Foreign keys declared on the table.
    #[bimetal(getter)]
    #[must_use]
    pub fn foreign_keys(&self) -> Vec<ForeignKeyInfo> {
        let definition = self.core().definition();
        definition
            .foreign_keys()
            .iter()
            .map(|key| convert::foreign_key_info(&self.database, definition, key))
            .collect()
    }

    /// Indexes declared on the table.
    #[bimetal(getter)]
    #[must_use]
    pub fn indexes(&self) -> Vec<IndexInfo> {
        let definition = self.core().definition();
        definition
            .indexes()
            .iter()
            .map(|index| convert::index_info(definition, index))
            .collect()
    }

    /// Return the target table referenced by the foreign key named
    /// `foreign_key_name` (ASCII case-insensitive), if present.
    #[must_use]
    pub fn target(&self, foreign_key_name: &str) -> Option<Table> {
        let table = self.core();
        let key = table
            .definition()
            .foreign_keys()
            .iter()
            .find(|key| key.name().eq_ignore_ascii_case(foreign_key_name))?;
        let target = table.target(key)?;
        Some(Table {
            database: Arc::clone(&self.database),
            table_id: target.id(),
        })
    }

    /// Stream all live rows with every column in storage order.
    #[must_use]
    pub fn rows(&self) -> RowCursor {
        let definition = self.core().definition();
        let columns = sa17::Columns::all(definition);
        let iter = self.database.scan_owned(definition, &columns);
        RowCursor {
            table_name: definition.name().to_owned(),
            iter,
        }
    }

    /// Stream live rows decoded only for the requested `columns`.
    ///
    /// Columns omitted from `columns` are stepped over without decoding text or
    /// following long-value chains.
    pub fn scan(&self, columns: Vec<String>) -> Result<RowCursor, Sa17Error> {
        let definition = self.core().definition();
        let selected = sa17::Columns::new(definition, columns.iter().map(String::as_str))
            .map_err(convert::error)?;
        let iter = self.database.scan_owned(definition, &selected);
        Ok(RowCursor {
            table_name: definition.name().to_owned(),
            iter,
        })
    }

    /// Read a single live row at `address`, or `None` if the slot holds no live
    /// row.
    pub fn row(
        &self,
        address: RowAddress,
        columns: Option<Vec<String>>,
    ) -> Result<Option<Row>, Sa17Error> {
        let selected = self.resolve_columns(columns)?;
        let row = self
            .core()
            .row(convert::row_address_to_core(address), &selected)
            .map_err(convert::error)?;
        Ok(row.map(|inner| Row { inner }))
    }

    /// Stream the physical `RowAddress` of every live row without decoding
    /// column values.
    #[must_use]
    pub fn row_addresses(&self) -> AddressCursor {
        let definition = self.core().definition();
        AddressCursor {
            iter: self.database.row_addresses_owned(definition),
        }
    }

    /// Return the Apache Arrow `Schema` for this table or a projection of its
    /// columns.
    #[bimetal(returns(arrow))]
    pub fn arrow_schema(&self, columns: Option<Vec<String>>) -> Result<ArrowSchema, Sa17Error> {
        let selected = self.resolve_columns(columns)?;
        arrow_bridge::schema_for(self.core().definition(), &selected)
    }

    /// Read the table (or selected `columns`) into a zero-copy `pyarrow.Table`.
    #[bimetal(returns(arrow))]
    pub fn to_arrow(
        &self,
        columns: Option<Vec<String>>,
        #[bimetal(default = 65_536)] batch_size: u32,
    ) -> Result<ArrowTable, Sa17Error> {
        let definition = self.core().definition();
        let selected = self.resolve_columns(columns)?;
        let rows = self.database.scan_owned(definition, &selected);
        arrow_bridge::read_table(definition, &selected, rows, batch_size)
    }

    /// Stream the table (or selected `columns`) as a lazy zero-copy
    /// `pyarrow.RecordBatchReader`, usable directly by PyArrow, Polars, and
    /// DuckDB.
    #[bimetal(returns(arrow))]
    pub fn batches(
        &self,
        columns: Option<Vec<String>>,
        #[bimetal(default = 65_536)] batch_size: u32,
    ) -> Result<RecordBatchStream, Sa17Error> {
        let definition = self.core().definition();
        let selected = self.resolve_columns(columns)?;
        let rows = self.database.scan_owned(definition, &selected);
        arrow_bridge::batch_stream(definition, &selected, rows, batch_size)
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        let definition = self.core().definition();
        format!(
            "Table(id={}, name={:?}, rows={})",
            definition.id(),
            definition.name(),
            definition.row_count()
        )
    }
}

/// One decoded row of a table.
pub struct Row {
    inner: sa17::Row,
}

/// A decoded row of a table.
///
/// Supports `row[column]`, `column in row`, `len(row)`, `row.to_dict()`, and
/// typed accessors for binary, date, and exact decimal columns.
#[bimetal::export]
impl Row {
    /// Physical page and slot address of the row.
    #[bimetal(getter)]
    #[must_use]
    pub fn address(&self) -> Option<RowAddress> {
        self.inner
            .id()
            .address()
            .map(convert::row_address_from_core)
    }

    /// Column names in the order present on this row.
    #[bimetal(getter)]
    #[must_use]
    pub fn columns(&self) -> Vec<String> {
        self.inner.columns().names().map(str::to_owned).collect()
    }

    /// Number of columns on this row.
    #[bimetal(len)]
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.inner.columns().len()
    }

    /// Whether `column` (ASCII case-insensitive) is present on this row.
    #[bimetal(contains)]
    #[must_use]
    pub fn has_column(&self, column: &str) -> bool {
        self.inner.columns().position(column).is_some()
    }

    /// Value of `column` (ASCII case-insensitive), or raises `Sa17Error` if the
    /// column is not on this row.
    #[bimetal(getitem)]
    pub fn get_value(&self, column: &str) -> Result<JsonValue, Sa17Error> {
        let Some(value) = self.inner.get(column) else {
            return Err(Sa17Error::UnknownColumn {
                table: self.inner.columns().table_name().to_owned(),
                column: column.to_owned(),
            });
        };
        Ok(convert::value_to_json(value))
    }

    /// All values as a mapping from column name to JSON-compatible Python value.
    #[must_use]
    pub fn to_dict(&self) -> BTreeMap<String, JsonValue> {
        convert::row_to_dict(&self.inner)
    }

    /// Positional list of values matching `self.columns`.
    #[must_use]
    pub fn values(&self) -> Vec<JsonValue> {
        self.inner
            .values()
            .iter()
            .map(convert::value_to_json)
            .collect()
    }

    /// Exact raw bytes of a `BINARY`, `VARBINARY`, `LONG BINARY`, or
    /// `LONG VARBIT` column, or `None` when `NULL`.
    pub fn get_bytes(&self, column: &str) -> Result<Option<Vec<u8>>, Sa17Error> {
        self.inner
            .read::<Option<Vec<u8>>>(column)
            .map_err(convert::error)
    }

    /// Exact `datetime.date` of a `DATE` column, or `None` when `NULL`.
    pub fn get_date(&self, column: &str) -> Result<Option<NaiveDate>, Sa17Error> {
        self.inner
            .read::<Option<NaiveDate>>(column)
            .map_err(convert::error)
    }

    /// Exact decimal string of a `NUMERIC` or `DECIMAL` column, or `None` when
    /// `NULL`.
    pub fn get_decimal(&self, column: &str) -> Result<Option<String>, Sa17Error> {
        let value = self
            .inner
            .read::<Option<sa17::Decimal>>(column)
            .map_err(convert::error)?;
        Ok(value.map(|decimal| decimal.to_string()))
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        format!(
            "Row(id={}, columns={})",
            self.inner.id(),
            self.inner.columns().len()
        )
    }
}

/// Lazy iterator over the live rows of a table.
pub struct RowCursor {
    table_name: String,
    iter: sa17::OwnedRows,
}

/// Streaming row cursor returned by `Table.rows()` and `Table.scan()`.
#[bimetal::export]
impl RowCursor {
    /// Yield the next live row, or `None` at the end of the table.
    #[bimetal(next)]
    pub fn next_row(&mut self) -> Result<Option<Row>, Sa17Error> {
        match self.iter.next() {
            Some(Ok(inner)) => Ok(Some(Row { inner })),
            Some(Err(error)) => Err(convert::error(error)),
            None => Ok(None),
        }
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        format!("RowCursor(table={:?})", self.table_name)
    }
}

/// Lazy iterator over the live row addresses of a table.
pub struct AddressCursor {
    iter: sa17::OwnedRowAddresses,
}

/// Streaming cursor over `RowAddress` values returned by
/// `Table.row_addresses()`.
#[bimetal::export]
impl AddressCursor {
    /// Yield the next live row address, or `None` at the end of the table.
    #[bimetal(next)]
    pub fn next_address(&mut self) -> Result<Option<RowAddress>, Sa17Error> {
        match self.iter.next() {
            Some(Ok(address)) => Ok(Some(convert::row_address_from_core(address))),
            Some(Err(error)) => Err(convert::error(error)),
            None => Ok(None),
        }
    }
}

/// Backing storage for a `PageStore`, either owned directly or shared with a
/// `Database`.
enum PageStoreBacking {
    Standalone(sa17::PageStore),
    Database(Arc<sa17::Database>),
}

impl PageStoreBacking {
    fn as_store(&self) -> &sa17::PageStore {
        match self {
            Self::Standalone(store) => store,
            Self::Database(database) => database.store(),
        }
    }
}

/// Decoded page store below the system catalog layer.
pub struct PageStore {
    inner: PageStoreBacking,
}

/// Low-level page store giving direct access to raw and decoded 4096-byte pages
/// and checksum verification.
#[bimetal::export]
impl PageStore {
    /// Open a page store directly from disk without loading the catalog.
    #[bimetal(constructor)]
    pub fn new(path: PathBuf, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        Self::open(path, options)
    }

    /// Open a page store directly from disk without loading the catalog.
    #[bimetal(constructor)]
    pub fn open(path: PathBuf, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        let options = convert::open_options(options);
        let store = sa17::PageStore::open_with(path, &options).map_err(convert::error)?;
        Ok(Self {
            inner: PageStoreBacking::Standalone(store),
        })
    }

    /// Open a page store from raw bytes without loading the catalog.
    #[bimetal(constructor)]
    pub fn from_bytes(data: Vec<u8>, options: Option<OpenOptions>) -> Result<Self, Sa17Error> {
        let options = convert::open_options(options);
        let store = sa17::PageStore::from_bytes_with(data, &options).map_err(convert::error)?;
        Ok(Self {
            inner: PageStoreBacking::Standalone(store),
        })
    }

    /// File size in bytes.
    #[bimetal(getter)]
    #[must_use]
    pub fn size(&self) -> u64 {
        self.inner.as_store().size()
    }

    /// Size of each page in bytes (4096).
    #[bimetal(getter)]
    #[must_use]
    pub fn page_size(&self) -> u32 {
        self.inner.as_store().page_size() as u32
    }

    /// Total number of pages in the file.
    #[bimetal(getter)]
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.inner.as_store().page_count()
    }

    /// Superblock fields from page 0.
    #[bimetal(getter)]
    #[must_use]
    pub fn superblock(&self) -> Superblock {
        convert::superblock(self.inner.as_store().superblock())
    }

    /// Recovered scrambling key.
    #[bimetal(getter)]
    #[must_use]
    pub fn key(&self) -> StoreKey {
        convert::store_key_from_core(self.inner.as_store().key())
    }

    /// File identity fingerprint.
    #[bimetal(getter)]
    #[must_use]
    pub fn identity(&self) -> Identity {
        convert::identity(self.inner.as_store().identity())
    }

    /// Character encoding declared in page 0.
    #[bimetal(getter)]
    #[must_use]
    pub fn text_encoding(&self) -> TextEncoding {
        convert::text_encoding(self.inner.as_store().text_encoding())
    }

    /// Verify the CRC-32 checksum of `page_number` and report whether it was
    /// written.
    pub fn check(&self, page_number: u32) -> Result<PageState, Sa17Error> {
        let state = self
            .inner
            .as_store()
            .check(page_number)
            .map_err(convert::error)?;
        Ok(convert::page_state(state))
    }

    /// Return the raw stored (still scrambled) bytes of `page_number`.
    pub fn stored(&self, page_number: u32) -> Result<Vec<u8>, Sa17Error> {
        let bytes = self
            .inner
            .as_store()
            .stored(page_number)
            .map_err(convert::error)?;
        Ok(bytes.to_vec())
    }

    /// Check and decode `page_number`.
    pub fn page(&self, page_number: u32) -> Result<Page, Sa17Error> {
        let inner = self
            .inner
            .as_store()
            .page(page_number)
            .map_err(convert::error)?;
        Ok(Page { inner })
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        let store = self.inner.as_store();
        format!(
            "PageStore(pages={}, size={})",
            store.page_count(),
            store.size()
        )
    }
}

/// One decoded 4096-byte page from a `PageStore`.
pub struct Page {
    inner: sa17::Page,
}

/// A decoded database page, exposing header fields, decoded bytes, and slotted
/// record inspection.
#[bimetal::export]
impl Page {
    /// Zero-based page number in the file.
    #[bimetal(getter)]
    #[must_use]
    pub fn number(&self) -> u32 {
        self.inner.number()
    }

    /// Page number recorded inside the page header.
    #[bimetal(getter)]
    #[must_use]
    pub fn stored_number(&self) -> u32 {
        self.inner.stored_number()
    }

    /// Single-byte page type tag from the header.
    #[bimetal(getter)]
    #[must_use]
    pub fn page_type(&self) -> u8 {
        self.inner.page_type()
    }

    /// Log sequence number of the page's last write.
    #[bimetal(getter)]
    #[must_use]
    pub fn lsn(&self) -> u32 {
        self.inner.lsn()
    }

    /// Structural kind of the page.
    #[bimetal(getter)]
    #[must_use]
    pub fn kind(&self) -> PageKind {
        convert::page_kind(self.inner.kind())
    }

    /// Owning table identifier, for row and extension pages.
    #[bimetal(getter)]
    #[must_use]
    pub fn table_id(&self) -> Option<u32> {
        self.inner.table_id()
    }

    /// Number of slots on a row or extension page.
    #[bimetal(getter)]
    #[must_use]
    pub fn slot_count(&self) -> Option<u16> {
        self.inner.slot_count()
    }

    /// Return the 4096 decoded bytes of the page.
    #[must_use]
    pub fn data(&self) -> Vec<u8> {
        self.inner.bytes().to_vec()
    }

    /// Return metadata for each record on a row or extension page.
    pub fn records(&self) -> Result<Vec<RecordInfo>, Sa17Error> {
        let records = self.inner.records().map_err(convert::error)?;
        let mut info = Vec::new();
        for record in records {
            let record = record.map_err(convert::error)?;
            info.push(convert::record_info(&record));
        }
        Ok(info)
    }

    /// Return the raw record bytes in `slot`, or `None` if the slot is empty or
    /// past the end of the directory.
    pub fn record_bytes(&self, slot: u16) -> Result<Option<Vec<u8>>, Sa17Error> {
        let record = self.inner.record(slot).map_err(convert::error)?;
        Ok(record.map(|record| record.bytes().to_vec()))
    }

    /// Developer-oriented string representation.
    #[bimetal(repr)]
    #[must_use]
    pub fn repr_string(&self) -> String {
        format!(
            "Page(number={}, kind={:?})",
            self.inner.number(),
            self.inner.kind()
        )
    }
}
