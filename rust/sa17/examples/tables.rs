//! Lists the tables of a database file with the rows each one holds.
//!
//! ```console
//! cargo run -p sa17 --example tables -- database.db
//! ```

use std::env;
use std::ffi::OsStr;
use std::process::ExitCode;

use sa17::{Database, Error, Source};

fn main() -> ExitCode {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("usage: tables <database file>");
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
    for table in database.definitions() {
        if !table.is_system() {
            println!("{:>10}  {}", table.row_count(), table.name());
        }
    }
    Ok(())
}
