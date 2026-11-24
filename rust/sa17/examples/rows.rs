//! Prints the rows of one table, one value per line.
//!
//! ```console
//! cargo run -p sa17 --example rows -- database.db plants
//! ```

use std::env;
use std::process::ExitCode;

use sa17::{Database, Error};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [path, table] = &arguments[..] else {
        eprintln!("usage: rows <database file> <table>");
        return ExitCode::FAILURE;
    };
    match print(path, table) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn print(path: &str, name: &str) -> Result<(), Error> {
    let database = Database::open(path)?;
    let table = database.table(name)?;
    for row in table.rows() {
        let row = row?;
        for (column, value) in table.columns().iter().zip(row.values()) {
            println!("{:<28} {value}", column.name());
        }
        println!();
    }
    Ok(())
}
