//! The crate reads SQL Anywhere 17 databases and knows nothing of any specific
//! application that fills one.

// Every function in this file runs inside a test, where a failed unwrap is a
// failed test.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// Word prefixes that would name an accounting application or its schema.
const FOREIGN_STARTS: &[&str] = &["abmc", "book", "compan", "qb", "quickbooks", "intuit"];

/// Domain words that do not belong in a generic database storage reader.
const FOREIGN_WORDS: &[&str] = &[
    "customer",
    "customers",
    "invoice",
    "invoices",
    "ledger",
    "ledgers",
    "memo",
    "memos",
    "payroll",
    "vendor",
    "vendors",
];

fn files(folder: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, found);
        } else {
            found.push(path);
        }
    }
}

#[test]
fn nothing_in_the_crate_names_an_application_or_its_data() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = vec![root.join("Cargo.toml")];
    for folder in ["src", "examples", "tests"] {
        files(&root.join(folder), &mut sources);
    }
    let this_file = root.join("tests/neutral.rs");
    let mut found = Vec::new();
    for path in sources.iter().filter(|path| **path != this_file) {
        let text = fs::read_to_string(path).unwrap().to_lowercase();
        let words = text.split(|character: char| !character.is_alphanumeric());
        let foreign = words.filter(|word| {
            let mut starts = FOREIGN_STARTS.iter();
            starts.any(|start| word.starts_with(start)) || FOREIGN_WORDS.contains(word)
        });
        let name = path.strip_prefix(root).unwrap().display().to_string();
        let name = name.replace('\\', "/");
        found.extend(foreign.map(|word| format!("{name}: {word}")));
    }
    assert!(
        found.is_empty(),
        "unexpected application-specific terms: {found:?}"
    );
    assert!(sources.len() > 30, "only {} files were read", sources.len());
}
