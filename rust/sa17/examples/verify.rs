//! Verifies every page checksum, row, and catalog row count of a database file.
//!
//! ```console
//! cargo run -p sa17 --example verify -- database.db
//! ```

use std::env;
use std::ffi::OsStr;
use std::process::ExitCode;

use sa17::{Database, Error};

fn main() -> ExitCode {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("usage: verify <database file>");
        return ExitCode::FAILURE;
    };
    match verify(&path) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn verify(path: &OsStr) -> Result<bool, Error> {
    let database = Database::open(path)?;
    let report = database.verify();
    println!(
        "sound={} pages={} tables={} rows={}",
        report.is_sound(),
        report.pages(),
        report.tables(),
        report.rows(),
    );
    for problem in report.problems() {
        eprintln!("{problem}");
    }
    Ok(report.is_sound())
}
