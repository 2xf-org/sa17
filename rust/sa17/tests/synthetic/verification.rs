//! Checking a whole file.

use sa17::{Error, VerificationProblem};
use sa17_synthetic::{Cell, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable};

use crate::support;

#[test]
fn a_verification_counts_pages_tables_and_rows() {
    let synthetic = SyntheticDatabase::new().table(support::catalogue(400));
    let sound = support::open(&synthetic).verify();
    assert!(sound.is_sound() && sound.problems().is_empty());
    // Seven tables, their columns and two users, next to the stars.
    let catalog_rows = 7 + 68 + 2;
    assert_eq!(
        (sound.unwritten_pages(), sound.tables(), sound.rows()),
        (9, 7, 400 + catalog_rows)
    );

    let pages = support::definition(&support::open(&synthetic), "stars")
        .pages()
        .to_vec();
    let damaged = support::damaged(&synthetic, &pages[1..3]).verify();
    assert!(!damaged.is_sound());
    // Nothing says how many rows the two pages have, so nothing is compared.
    assert_eq!(
        (
            damaged.damaged_pages(),
            damaged.unreadable_pages(),
            damaged.unreadable_rows(),
            damaged.differing_tables()
        ),
        (2, 2, 0, 0)
    );
    assert!(damaged.rows() > 200 && damaged.rows() < 400 + catalog_rows);
    let problems: Vec<String> = damaged.problems().iter().map(ToString::to_string).collect();
    let page = pages[1];
    assert_eq!(problems[0], format!("page {page}: it fails its checksum"));
    assert_eq!(
        problems[2],
        format!(
            "stars: 0 rows and 2 pages of rows could not be read; \
             the first: table stars, page {page}: it fails its checksum"
        )
    );
    assert_eq!(problems.len(), 3);
}

#[test]
fn a_row_that_cannot_be_read_is_one_of_those_that_are_there() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]);
    let table = support::numbers("seeds", &[1, 2])
        .row(row(3).flags(0x02))
        .row(row(4).deleted());
    let file = SyntheticDatabase::new().table(table);
    let verification = support::open(&file).verify();
    assert!(!verification.is_sound());
    assert_eq!(
        (
            verification.unreadable_rows(),
            verification.unreadable_pages(),
            verification.differing_tables()
        ),
        (1, 0, 0)
    );
    let [
        VerificationProblem::Rows {
            rows: 1,
            pages: 0,
            first: Error::Row { .. },
            ..
        },
    ] = verification.problems()
    else {
        panic!("{:?}", verification.problems());
    };

    // The count follows the identifier and the two zeros.
    let mut recorded = support::table_row(1000);
    let start = recorded.clone();
    recorded.extend(3_u64.to_le_bytes());
    let other = [start, 5_u64.to_le_bytes().to_vec()].concat();
    let file = support::replace(&file, "ISYSTAB", &recorded, &other);
    let verification = support::open(&file).verify();
    assert_eq!(verification.differing_tables(), 1);
    let problem = verification.problems().last().unwrap().to_string();
    assert_eq!(problem, "seeds: found 3 rows, the catalog records 5");
}

// Each thing that can be wrong is enough.
#[test]
fn a_file_with_one_thing_wrong_is_not_sound() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]);
    let file = SyntheticDatabase::new().table(support::numbers("seeds", &[1, 2]).stale_row(row(9)));
    assert!(support::open(&file).verify().is_sound());
    // The page that the table gave up holds nothing that is read.
    let stale = support::definition(&support::open(&file), "seeds").pages()[0] + 1;
    let verification = support::damaged(&file, &[stale]).verify();
    assert!(!verification.is_sound());
    assert_eq!(
        (
            verification.damaged_pages(),
            verification.unreadable_pages(),
            verification.differing_tables()
        ),
        (1, 0, 0)
    );
    let [VerificationProblem::Page(Error::Page { page, .. })] = verification.problems() else {
        panic!("{:?}", verification.problems());
    };
    assert_eq!(*page, stale);

    let counted = [support::table_row(1000), 2_u64.to_le_bytes().to_vec()].concat();
    let other = [support::table_row(1000), 1_u64.to_le_bytes().to_vec()].concat();
    let file = support::replace(&file, "ISYSTAB", &counted, &other);
    let verification = support::open(&file).verify();
    assert!(!verification.is_sound());
    assert_eq!(
        (
            verification.damaged_pages(),
            verification.differing_tables()
        ),
        (0, 1)
    );
}

// What is wrong with the catalog is wrong with the file.
#[test]
fn a_problem_of_the_catalog_is_one_of_the_file() {
    let sky = support::sky();
    let mut key = 1001_u32.to_le_bytes().to_vec();
    key.extend(1_u32.to_le_bytes());
    let mut dangling = key.clone();
    key.extend(1000_u32.to_le_bytes());
    dangling.extend(0xffff_u32.to_le_bytes());
    let database = support::open(&support::replace(&sky, "ISYSFKEY", &key, &dangling));
    let verification = database.verify();
    assert!(!verification.is_sound());
    assert_eq!(verification.catalog_problems(), 1);
    let problem = &verification.problems()[0];
    assert!(matches!(problem, VerificationProblem::Catalog(_)));
    assert_eq!(
        problem.to_string(),
        database.catalog_problems()[0].to_string()
    );
}

#[test]
fn a_verification_keeps_a_hundred_problems_and_counts_on() {
    let mut table =
        SyntheticTable::new("wide").column(SyntheticColumn::new("bytes", 28).width(4000));
    for _ in 0..120 {
        table = table.row(SyntheticRow::new([Cell::bytes(&[7; 3900])]));
    }
    let file = SyntheticDatabase::new().table(table);
    let pages = support::definition(&support::open(&file), "wide")
        .pages()
        .to_vec();
    assert_eq!(pages.len(), 120);
    let verification = support::damaged(&file, &pages).verify();
    assert_eq!(verification.damaged_pages(), 120);
    assert_eq!(verification.unreadable_pages(), 120);
    assert_eq!(verification.problems().len(), 100);
}
