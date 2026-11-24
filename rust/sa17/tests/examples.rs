//! The examples, run on a file that the writer makes.

// Every function here runs inside a test, where a failed unwrap is a failed test.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sa17_synthetic::{Cell, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable};

/// A file called `name` of beds, and of the plants in them.
fn garden(name: &str) -> PathBuf {
    let beds = SyntheticTable::new("beds")
        .column(SyntheticColumn::new("id", 2))
        .primary_key(&["id"])
        .row(SyntheticRow::new([Cell::integer(1)]));
    let plants = SyntheticTable::new("plants")
        .column(SyntheticColumn::new("name", 9).width(20))
        .column(SyntheticColumn::new("bed", 2).nullable())
        .foreign_key("plant_bed", &["bed"], "beds")
        .row(SyntheticRow::new([Cell::text("Fern"), Cell::integer(1)]))
        .row(SyntheticRow::new([Cell::text("Moss"), Cell::Null]));
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.db"));
    let garden = SyntheticDatabase::new().table(beds).table(plants);
    fs::write(&file, garden.build()).unwrap();
    file
}

fn run(example: &str, file: &Path, arguments: &[&str]) -> Output {
    let cargo = ["run", "--quiet", "--package", "sa17"];
    Command::new(env!("CARGO"))
        .args(cargo)
        .args(["--example", example, "--"])
        .arg(file)
        .args(arguments)
        .output()
        .unwrap()
}

fn printed(example: &str, arguments: &[&str]) -> String {
    let output = run(example, &garden(example), arguments);
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn tables_lists_the_tables_that_are_not_the_engines() {
    assert_eq!(
        printed("tables", &[]),
        "         1  beds\n         2  plants\n"
    );
}

#[test]
fn rows_prints_every_value_of_a_table() {
    let columns = ["name", "bed"].map(|column| format!("{column:<28}"));
    let [name, bed] = columns;
    assert_eq!(
        printed("rows", &["plants"]),
        format!("{name} Fern\n{bed} 1\n\n{name} Moss\n{bed} NULL\n\n")
    );
}

#[test]
fn keys_prints_both_sides_of_a_foreign_key() {
    assert_eq!(printed("keys", &[]), "plants(bed) -> beds(id)\n");
}

#[test]
fn verify_reports_a_sound_database() {
    let output = printed("verify", &[]);
    assert!(output.starts_with("sound=true "), "{output}");
}

#[test]
fn an_example_says_why_it_failed() {
    let output = run("rows", &garden("unknown"), &["weeds"]);
    assert!(!output.status.success());
    let said = String::from_utf8(output.stderr).unwrap();
    assert!(said.ends_with("there is no table called weeds\n"), "{said}");
}
