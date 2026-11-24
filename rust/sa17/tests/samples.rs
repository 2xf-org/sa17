//! Reads real database files, which the repository never holds.
//!
//! Point `SA17_SAMPLES` at a folder of them and run the ignored tests:
//!
//! ```console
//! SA17_SAMPLES=~/Databases cargo test --release -p sa17 --test samples -- --ignored
//! ```

// Every function in this file runs inside a test, where a failed unwrap is a
// failed test.
#![allow(clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::PathBuf;

use sa17::{Database, Error, IndexKind, TableDefinition};

/// Every database file in the folder, with its path to name in a failure.
fn samples() -> impl Iterator<Item = (PathBuf, Database)> {
    let folder = env::var_os("SA17_SAMPLES").expect("SA17_SAMPLES names a folder");
    let mut files = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            files.push(path);
        }
    }
    let mut databases = files
        .into_iter()
        .filter_map(|path| match Database::open(&path) {
            Ok(database) => Some((path, database)),
            // The folder can hold other files beside them.
            Err(Error::Format(_)) => None,
            Err(error) => panic!("{}: {error}", path.display()),
        })
        .peekable();
    assert!(databases.peek().is_some(), "SA17_SAMPLES holds no database");
    databases
}

// The catalog counts each table's rows, so a row that is skipped, read twice
// or taken from a stale page shows up as a difference.
#[test]
#[ignore = "needs database files in the folder SA17_SAMPLES names"]
fn every_row_reads_and_every_count_matches_the_catalog() {
    for (path, database) in samples() {
        let problems = database.catalog_problems();
        assert!(problems.is_empty(), "{}: {problems:?}", path.display());
        let stored = database
            .tables()
            .filter(|table| table.definition().stores_rows());
        for table in stored {
            let mut rows = 0;
            for row in table.rows() {
                row.unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                rows += 1;
            }
            assert_eq!(
                rows,
                table.definition().row_count(),
                "{}: {}",
                path.display(),
                table.name()
            );
        }
    }
}

/// Whether the table has a unique index of the columns at `positions`, in
/// whichever order.
fn are_unique(table: &TableDefinition, positions: &[usize]) -> bool {
    let sorted = |mut positions: Vec<usize>| {
        positions.sort_unstable();
        positions
    };
    let wanted = sorted(positions.to_vec());
    let mut unique = table.indexes().iter().filter(|index| index.is_unique());
    unique.any(|index| {
        let columns = index.columns().iter().map(|column| column.position());
        sorted(columns.collect()) == wanted
    })
}

// A foreign key holds the values of columns that tell the rows of its target
// apart, and the engine keeps an index behind every key.
#[test]
#[ignore = "needs database files in the folder SA17_SAMPLES names"]
fn every_declared_key_names_columns_that_can_be_one() {
    for (path, database) in samples() {
        let mut declared = 0;
        for table in database.catalog() {
            let place = format!("{}: {}", path.display(), table.name());
            let primary_key = table.primary_key();
            assert!(
                primary_key.is_none_or(|positions| are_unique(table, positions)),
                "{place}"
            );
            for key in table.foreign_keys() {
                let target = database.table_by_id(key.target_id());
                let target = target.unwrap_or_else(|| panic!("{place}")).definition();
                assert!(are_unique(target, key.target_positions()), "{place}");
                assert_eq!(
                    key.positions().len(),
                    key.target_positions().len(),
                    "{place}"
                );
            }
            let behind = table.indexes().iter().filter(|index| {
                matches!(index.kind(), IndexKind::PrimaryKey | IndexKind::ForeignKey)
            });
            let keys = usize::from(primary_key.is_some()) + table.foreign_keys().len();
            assert_eq!(behind.count(), keys, "{place}");
            declared += keys;
        }
        // The catalog's own tables have keys in every file.
        assert!(declared > 0, "{}", path.display());
    }
}
