//! Files whose contents are damaged.

use sa17::{Columns, Database, Error, PageProblem, RowAddress, RowProblem};
use sa17_synthetic::SyntheticDatabase;

use crate::support;
use crate::support::PAGE_SIZE;

#[test]
fn a_damaged_page_fails_its_rows_and_no_others() {
    let synthetic = SyntheticDatabase::new().table(support::catalogue(400));
    let pages = support::definition(&support::open(&synthetic), "stars")
        .pages()
        .to_vec();
    let database = support::damaged(&synthetic, &pages[1..2]);
    let stars = database.table("stars").unwrap();
    let mut read = 0;
    let mut failures = Vec::new();
    for row in stars.rows() {
        match row {
            Ok(_) => read += 1,
            Err(error) => failures.push(error),
        }
    }
    let [
        Error::TablePage {
            table,
            page,
            problem: PageProblem::Checksum { .. },
        },
    ] = &failures[..]
    else {
        panic!("{failures:?}");
    };
    assert_eq!((table.as_str(), *page), ("stars", pages[1]));
    assert!(read > 200 && read < 400, "{read} rows");
    assert_eq!(stars.row_addresses().filter(Result::is_ok).count(), read);
    // A row of the page is an error of its own.
    let every = Columns::all(stars.definition());
    let address = RowAddress::new(pages[1], 2);
    let refused = stars.row(address, &every).unwrap_err();
    assert_eq!(
        refused.to_string(),
        format!("table stars, page {page}, slot 2: page {page}: it fails its checksum")
    );
}

// Page 10 lists the pages of the table catalog, and the planets come after it.
const FIRST_PLANETS_PAGE: u32 = 11;

/// What reading the planets fails with when their first page is patched.
fn planets_error(offset: usize, bytes: &[u8]) -> Error {
    let patched = support::sky().patch(FIRST_PLANETS_PAGE, offset, bytes);
    let database = support::open(&patched);
    let table = database.table("planets").unwrap();
    let mut errors = table.rows().filter_map(Result::err);
    errors.next().unwrap()
}

#[test]
fn a_page_that_is_not_what_its_list_says_is_refused() {
    let problem = |offset, bytes: &[u8]| match planets_error(offset, bytes) {
        Error::TablePage {
            table,
            page: FIRST_PLANETS_PAGE,
            problem,
        } if table == "planets" => problem,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        problem(0, &[0xff, 0, 0, 0]),
        PageProblem::WrongNumber { stored: 0xff }
    );
    assert!(matches!(problem(16, &[9]), PageProblem::WrongKind { .. }));
    assert_eq!(
        problem(24, &[7, 0, 0, 0]),
        PageProblem::WrongTable { table_id: 7 }
    );
    // More slots than the page has room for, and a slot inside the directory.
    assert_eq!(problem(22, &[0xff, 0xff]), PageProblem::Directory);
    assert_eq!(problem(28, &[1, 0]), PageProblem::Directory);
}

#[test]
fn a_row_that_does_not_fit_its_columns_is_refused() {
    let file = support::open(&support::sky());
    let page = file.store().page(FIRST_PLANETS_PAGE).unwrap();
    // The first record is the one at the end of the page.
    let first = PAGE_SIZE - 4 - page.record(0).unwrap().unwrap().bytes().len();
    let problem = |at: usize, bytes: &[u8]| match planets_error(first + at, bytes) {
        Error::Row { problem, .. } => problem,
        other => panic!("{other:?}"),
    };
    assert_eq!(problem(0, &[2, 0]), RowProblem::Length);
    assert_eq!(problem(0, &[0xff, 0xff]), RowProblem::Length);
    assert_eq!(problem(2, &[0x04]), RowProblem::Length);
    // With the flag, the byte that was the bitmap is the schema version.
    assert_eq!(
        problem(2, &[0x40, 9]),
        RowProblem::SchemaVersion { version: 9 }
    );
    // The bitmap says that the notes hold a value, and the row ends before it.
    assert_eq!(problem(3, &[0x80]), RowProblem::Truncated);
    // The name is declared longer than the row is.
    assert_eq!(problem(8, &[0x7f]), RowProblem::Truncated);
}

#[test]
fn a_damaged_file_is_never_a_panic() {
    let mut state = 0x2545_f491_u32;
    let mut random = |below: usize| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as usize % below
    };
    let pages = support::open(&support::sky()).store().page_count() as usize;
    let mut opened = 0;
    for _ in 0..400 {
        let mut damaged = support::sky();
        for _ in 0..1 + random(3) {
            let page = 10 + random(pages - 10);
            let offset = random(PAGE_SIZE - 4);
            damaged = damaged.patch(page as u32, offset, &[random(256) as u8]);
        }
        if let Ok(database) = Database::from_bytes(damaged.build()) {
            opened += 1;
            let _ = database.verify();
            for definition in database.catalog() {
                let _ = database.table(definition.name());
            }
        }
    }
    assert!(opened > 50, "only {opened} of the files opened");
}
