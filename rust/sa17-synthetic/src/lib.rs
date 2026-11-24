//! Writes small scrambled SQL Anywhere 17 database files for tests.
//!
//! The writer follows the file format specification in `docs/format.md` and
//! shares no code with the reader, so round-trip tests check both against each
//! other.
//!
//! ```
//! use sa17_synthetic::{
//!     Cell, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable,
//! };
//!
//! let plants = SyntheticTable::new("plants")
//!     .column(SyntheticColumn::new("id", 2))
//!     .column(SyntheticColumn::new("name", 9).width(40).nullable())
//!     .primary_key(&["id"])
//!     .row(SyntheticRow::new([Cell::integer(1), Cell::text("Fern")]));
//! let file: Vec<u8> = SyntheticDatabase::new().table(plants).build();
//! assert_eq!(file.len() % 4096, 0);
//! ```

mod calendar;
mod cell;
mod keys;
mod pages;
mod scramble;
mod synthetic_database;
mod synthetic_table;

pub use cell::{Cell, LongStorage};
pub use scramble::Sign;
pub use synthetic_database::SyntheticDatabase;
pub use synthetic_table::{IndexOrder, SyntheticColumn, SyntheticRow, SyntheticTable};
