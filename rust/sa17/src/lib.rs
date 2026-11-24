//! Pure-Rust reader for scrambled SAP SQL Anywhere 17 database files (format
//! 3.201.12), without a database server.
//!
//! [`Database::open`] reads a file, recovers the key that scrambles its pages,
//! and loads the system catalog. Each table then yields its schema, keys,
//! indexes, and rows as typed values:
//!
//! ```no_run
//! use sa17::{Columns, Database};
//!
//! let database = Database::open("garden.db")?;
//! let plants = database.table("plants")?;
//! let columns = Columns::new(plants.definition(), ["name", "height"])?;
//! for row in plants.scan(&columns) {
//!     let row = row?;
//!     let (name, height): (&str, Option<f64>) = (row.read("name")?, row.read("height")?);
//!     println!("{name}: {height:?}");
//! }
//! # Ok::<(), sa17::Error>(())
//! ```
//!
//! A program can work at any of three layers:
//!
//! - A [`Database`] provides [`Table`]s, and each table streams [`Row`]s. Every
//!   stored row carries its [`RowAddress`] (page number and slot index).
//! - [`Database::store`] exposes the underlying [`PageStore`], which also opens
//!   on its own. A [`Page`] of rows gives the raw [`Record`] of each slot.
//! - [`PageStore::key`] and [`PageStore::stored`] expose the recovered
//!   [`StoreKey`] and the raw bytes of each page.
//!
//! Code that reads tables can accept a [`Source`] (implemented by [`Database`])
//! so callers can wrap or substitute a database behind a uniform interface.
//!
//! Reading never modifies the file.
//!
//! # Scope and limitations
//!
//! The reader supports scrambled SQL Anywhere 17 page stores (format 3.201.12,
//! 4096-byte pages):
//!
//! - A file with another format version or another page size is refused with
//!   [`Error::Format`].
//! - Pages must use the additive-progression page cipher. Unscrambled files and
//!   password-encrypted files are refused with [`Error::NoKey`].
//! - Text columns must use Windows-1252 or UTF-8. Other encodings fail with
//!   [`Error::UnsupportedEncoding`].
//! - `XML`, `NCHAR`, `UNIQUEIDENTIFIER`, and `TIMESTAMP WITH TIME ZONE` columns
//!   appear as [`DataType::Unknown`] and read only `NULL`; a non-null value in
//!   such a column returns an error.
//! - Views, global temporary tables, and text-index tables expose their catalog
//!   definitions only. [`TableDefinition::stores_rows`] distinguishes them from
//!   tables that store rows.
//! - Indexes are read from the catalog, not from index B-tree pages. Rows are
//!   fetched by [`RowAddress`] or by scanning the table.
//! - The entire file is held in memory. Decompressed long values are bounded to
//!   16 MiB by default; override this with [`OpenOptions::longest_value`].
//!
//! What the format alone cannot guarantee:
//!
//! - A file copied while a database server had it open may lack uncommitted or
//!   uncheckpointed changes stored in the separate transaction log (`.tlg`),
//!   which is not read. Two flag bits (`0x20` and `0x40`) in the superblock are
//!   observed in some files with unknown meaning.
//! - Each row stores a single byte for its schema version. If a table was
//!   altered more than 255 times, older rows wrap around modulo 256.
//! - Extension pages of `ISYSTAB` are not listed in the catalog and are
//!   identified from their own page headers.

pub mod calendar;

mod catalog;
mod cipher;
mod database;
mod decimal;
mod error;
mod format_version;
mod from_value;
mod identity;
mod open_options;
mod page_size;
mod record;
mod row_address;
mod row_id;
mod row_key;
mod schema;
mod source;
mod store;
mod table;
mod value;
mod verification;
mod wide_product;

pub use chrono;

pub use cipher::{KeySign, PageKey, StoreKey};
pub use database::Database;
pub use decimal::{Decimal, ParseDecimalError};
pub use error::{
    CatalogProblem, CompressionProblem, Error, FormatProblem, LongValueProblem, PageListProblem,
    PageProblem, Result, RowProblem, ValueProblem,
};
pub use format_version::FormatVersion;
pub use from_value::FromValue;
pub use identity::Identity;
pub use open_options::OpenOptions;
pub use record::{ColumnIndex, Columns, OwnedRowAddresses, OwnedRows, Row, RowAddresses, Rows};
pub use row_address::RowAddress;
pub use row_id::RowId;
pub use row_key::RowKey;
pub use schema::{
    Column, DataType, ForeignKey, Index, IndexColumn, IndexKind, IndexOrder, LengthUnit,
    TableDefinition, TableKind,
};
pub use source::Source;
pub use store::{Page, PageKind, PageState, PageStore, Record, Superblock, TextEncoding};
pub use table::Table;
pub use value::{Undecoded, Value};
pub use verification::{Verification, VerificationProblem};
