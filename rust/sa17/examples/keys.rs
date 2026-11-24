//! Lists the foreign keys of a database file.
//!
//! ```console
//! cargo run -p sa17 --example keys -- database.db
//! ```

use std::env;
use std::ffi::OsStr;
use std::process::ExitCode;

use sa17::{Column, Database, Error, Table};

fn main() -> ExitCode {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("usage: keys <database file>");
        return ExitCode::FAILURE;
    };
    match list(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn list(path: &OsStr) -> Result<(), Error> {
    let database = Database::open(path)?;
    for table in database.tables() {
        for key in table.definition().foreign_keys() {
            if let Some(target) = table.target(key) {
                let own = names(table, key.positions());
                let others = names(target, key.target_positions());
                println!("{}({own}) -> {}({others})", table.name(), target.name());
            }
        }
    }
    Ok(())
}

/// The names of the columns at `positions`, between commas.
fn names(table: Table<'_>, positions: &[usize]) -> String {
    let columns = positions.iter().filter_map(|&at| table.columns().get(at));
    let names: Vec<&str> = columns.map(Column::name).collect();
    names.join(", ")
}
