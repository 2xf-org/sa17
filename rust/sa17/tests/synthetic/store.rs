//! The key of a file, its pages, and the files that are refused.

use std::array;
use std::collections::HashSet;
use std::io::ErrorKind;

use sa17::{
    Database, Error, FormatProblem, FormatVersion, KeySign, OpenOptions, PageKind, PageProblem,
    PageState, PageStore, RowAddress, StoreKey,
};
use sa17_synthetic::{
    Cell, Sign, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable,
};

use crate::support;
use crate::support::PAGE_SIZE;

/// The constants that a page of every key is tried on.
const MASK: u32 = 0x0001_2345;
const ADDEND: u32 = 0x0098_7654;

/// Whether two keys decode every page of a file the same way. The first ten
/// pages are not scrambled, so no key is used on them.
fn decode_alike(key: StoreKey, other: StoreKey, pages: u32) -> bool {
    let stored: [u8; PAGE_SIZE] = array::from_fn(|position| position as u8);
    let decode = |key: StoreKey, page| key.page_key(page).decode(&stored).unwrap();
    (10..pages).all(|page| decode(key, page) == decode(other, page))
}

// The solver may return other constants than the writer used, as long as
// they decode every page of the file the same way.
#[test]
fn the_key_is_found_for_either_sign() {
    for (sign, expected) in [
        (Sign::Positive, KeySign::Positive),
        (Sign::Negative, KeySign::Negative),
    ] {
        let database = support::open(
            &SyntheticDatabase::new()
                .key(MASK, ADDEND, sign)
                .table(support::catalogue(400)),
        );
        let key = database.store().key();
        assert_eq!(key.sign(), expected);
        let written = StoreKey::new(MASK, ADDEND, expected);
        let pages = database.store().page_count();
        assert!(decode_alike(key, written, pages));
        assert_eq!(support::rows(&database, "stars").len(), 400);
    }
}

// In a file this small, keys that tie on the first sector of every page are
// told apart by the rest of them.
#[test]
fn the_key_of_a_small_file_is_the_one_it_was_written_with() {
    let file = SyntheticDatabase::new().table(support::catalogue(100));
    let mut state = 0x9e37_79b9_u32;
    let mut random = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state >> 7
    };
    for _ in 0..300 {
        let (mask, addend) = (random(), random());
        let (sign, expected) = if random() % 2 == 0 {
            (Sign::Positive, KeySign::Positive)
        } else {
            (Sign::Negative, KeySign::Negative)
        };
        let written = StoreKey::new(mask, addend, expected);
        let database = support::open(&file.clone().key(mask, addend, sign));
        let store = database.store();
        assert!(
            decode_alike(store.key(), written, store.page_count()),
            "{written:?}"
        );
        assert_eq!(support::rows(&database, "stars").len(), 100);
    }
}

#[test]
fn a_file_opens_with_a_key_that_is_known() {
    let file = SyntheticDatabase::new()
        .key(MASK, ADDEND, Sign::Negative)
        .table(support::catalogue(40));
    let open = |key| Database::from_bytes_with(file.build(), &OpenOptions::new().key(key));
    let written = StoreKey::new(MASK, ADDEND, KeySign::Negative);
    let database = open(written).unwrap();
    assert_eq!(database.store().key(), written);
    assert_eq!(support::rows(&database, "stars").len(), 40);
    for wrong in [
        StoreKey::new(MASK, ADDEND, KeySign::Positive),
        StoreKey::new(MASK, ADDEND + 1, KeySign::Negative),
    ] {
        assert!(matches!(open(wrong), Err(Error::WrongKey)));
    }
}

/// A file whose pages are never written, but for the first and page 10.
fn almost_empty() -> Vec<u8> {
    let mut file = SyntheticDatabase::new().build();
    file[PAGE_SIZE..10 * PAGE_SIZE].fill(0);
    file[11 * PAGE_SIZE..].fill(0);
    file
}

// One page says too little. A program that knows the key opens it all the same.
#[test]
fn a_file_that_pins_no_key_has_the_keys_that_fit() {
    let Err(Error::AmbiguousKey { keys }) = PageStore::from_bytes(almost_empty()) else {
        panic!("one page pins a key");
    };
    assert!(keys.len() > 64, "{}", keys.len());
    let written = StoreKey::new(0x0000_1234, 0x0123_4567, KeySign::Positive);
    let options = OpenOptions::new().key(written);
    let store = PageStore::from_bytes_with(almost_empty(), &options).unwrap();
    assert_eq!(store.page(10).unwrap().kind(), PageKind::PageList);
    let fits = keys
        .iter()
        .any(|key| key.page_key(10) == written.page_key(10));
    assert!(fits, "{keys:?}");
}

#[test]
fn a_file_without_scrambled_pages_has_no_key() {
    let mut file = SyntheticDatabase::new().build();
    file[PAGE_SIZE..].fill(0);
    assert!(matches!(Database::from_bytes(file), Err(Error::NoKey)));
}

fn identity(file: &SyntheticDatabase) -> String {
    support::open(file).store().identity().to_string()
}

#[test]
fn an_identity_tells_files_and_their_states_apart() {
    let stars = SyntheticDatabase::new().table(support::catalogue(40));
    let first = identity(&stars);
    assert_eq!(first, identity(&stars));
    let (page, offset) = support::find(&stars, "stars", b"star number 7");
    let others = [
        stars.clone().key(MASK, ADDEND, Sign::Positive),
        stars.clone().key(MASK, ADDEND, Sign::Negative),
        SyntheticDatabase::new().table(support::catalogue(400)),
        // The log sequence number is at 0x08 of the first page.
        stars.clone().patch(0, 0x08, &5001_u32.to_le_bytes()),
        // The same key, size and log sequence number, and another row.
        stars.clone().patch(page, offset, b"Star"),
        // Another byte of the first page.
        stars.clone().patch(0, 0x300, &[1]),
        stars.clone().checkpoint_copy(),
    ];
    let mut identities: HashSet<String> = others.iter().map(identity).collect();
    identities.insert(first);
    assert_eq!(identities.len(), others.len() + 1, "{identities:?}");
}

// Only page 10 is written, so many keys decode it alike.
#[test]
fn an_identity_does_not_depend_on_the_key_that_was_found() {
    let Err(Error::AmbiguousKey { keys }) = PageStore::from_bytes(almost_empty()) else {
        panic!("one page pins a key");
    };
    let identities = keys.iter().map(|&key| {
        let options = OpenOptions::new().key(key);
        let store = PageStore::from_bytes_with(almost_empty(), &options).unwrap();
        store.identity().to_string()
    });
    let identities: HashSet<String> = identities.collect();
    assert_eq!(identities.len(), 1, "{identities:?}");
}

#[test]
fn a_checkpoint_copy_past_the_database_is_not_read() {
    let plain = SyntheticDatabase::new().table(support::catalogue(40));
    let with_copy = plain.clone().checkpoint_copy();
    let database = support::open(&with_copy);
    let store = database.store();
    let pages = store.page_count();
    assert_eq!(pages, support::open(&plain).store().page_count() + 1);
    assert_eq!(store.superblock().database_page_count(), pages - 1);
    assert_eq!(support::rows(&database, "stars").len(), 40);

    let copy = store.page(pages - 1).unwrap();
    let original = support::definition(&database, "stars").pages()[0];
    assert_eq!(copy.stored_number(), original);
    assert_ne!(copy.number(), original);
}

#[test]
fn a_page_past_the_end_is_an_error() {
    let database = support::open(&SyntheticDatabase::new());
    let store = database.store();
    let count = store.page_count();
    let refused = [
        store.page(count).map(|_| ()),
        store.check(u32::MAX).map(|_| ()),
        store.stored(count).map(|_| ()),
    ];
    for refused in refused {
        assert!(
            matches!(
                refused,
                Err(Error::Page {
                    problem: PageProblem::OutOfRange { count: pages },
                    ..
                }) if pages == count
            ),
            "{refused:?}"
        );
    }
    assert_eq!(store.page_size(), PAGE_SIZE);
    assert_eq!(store.size(), u64::from(count) * PAGE_SIZE as u64);
    // Page 10 lists the pages of the table catalog.
    assert_eq!(store.page(10).unwrap().kind(), PageKind::PageList);
    // Pages 1 to 9 of a synthetic file were never written.
    assert_eq!(store.page(5).unwrap().bytes(), [0; PAGE_SIZE]);
    assert_eq!(store.check(5).unwrap(), PageState::Unwritten);
    assert_eq!(store.check(10).unwrap(), PageState::Written);
}

// A program can go from a row to the page that it is on, and to its record.
#[test]
fn a_page_of_rows_has_the_records_of_its_slots() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]);
    let table = SyntheticTable::new("seeds")
        .column(SyntheticColumn::new("id", 2))
        .row(row(1).slack(5))
        .row(SyntheticRow::empty_slot())
        .row(row(3).deleted());
    let database = support::open(&SyntheticDatabase::new().table(table));
    let seeds = database.table("seeds").unwrap();
    let first = seeds.row_addresses().next().unwrap().unwrap();
    let page = database.store().page(first.page()).unwrap();
    assert_eq!(page.table_id(), Some(seeds.id()));
    assert_eq!(page.slot_count(), Some(3));

    let record = page.record(first.slot()).unwrap().unwrap();
    assert_eq!((record.address(), record.flags()), (first, 0));
    assert_eq!(record.bytes(), [9, 0, 0, 1, 0, 0, 0, 0, 0]);
    let records = page.records().unwrap().map(Result::unwrap);
    let records: Vec<(u16, u8)> = records
        .map(|record| (record.address().slot(), record.flags()))
        .collect();
    assert_eq!(records, [(0, 0), (2, 0x20)]);
    assert!(page.record(1).unwrap().is_none());
    assert!(page.record(9).unwrap().is_none());
}

#[test]
fn a_page_without_rows_has_neither_a_table_nor_slots() {
    let database = support::open(&SyntheticDatabase::new());
    for number in [0, 5, 10] {
        let page = database.store().page(number).unwrap();
        assert_eq!((page.table_id(), page.slot_count()), (None, None));
        let refused = page.records().err();
        assert!(
            matches!(
                refused,
                Some(Error::Page {
                    page,
                    problem: PageProblem::WrongKind { .. }
                }) if page == number
            ),
            "{refused:?}"
        );
        assert!(page.record(0).is_err());
    }
}

#[test]
fn a_record_that_does_not_fit_its_slot_costs_no_other() {
    let file = SyntheticDatabase::new().table(support::catalogue(3));
    let database = support::open(&file);
    let number = support::definition(&database, "stars").pages()[0];
    let page = database.store().page(number).unwrap();
    // The first record is the one at the end of the page.
    let length = page.record(0).unwrap().unwrap().bytes().len();
    let long = file.clone().patch(number, 4092 - length, &[0xff, 0]);
    let database = support::open(&long);
    let page = database.store().page(number).unwrap();
    let refused = page.record(0);
    assert!(
        matches!(
            refused,
            Err(Error::Page {
                problem: PageProblem::Record { slot: 0 },
                ..
            })
        ),
        "{refused:?}"
    );
    let records = page.records().unwrap();
    assert_eq!(
        records.map(|record| record.is_ok()).collect::<Vec<_>>(),
        [false, true, true]
    );
    // More slots than the page has room for.
    let database = support::open(&file.patch(number, 22, &[0xff, 0xff]));
    let page = database.store().page(number).unwrap();
    assert!(page.records().is_err() && page.record(1).is_err());
}

#[test]
fn a_file_that_is_not_a_page_store_is_refused() {
    let problem = |file: Vec<u8>| match Database::from_bytes(file) {
        Err(Error::Format(problem)) => problem,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        problem(vec![0; 2 * PAGE_SIZE]),
        FormatProblem::Magic { found: 0 }
    );
    assert_eq!(problem(vec![0; 100]), FormatProblem::Size { size: 100 });
    assert_eq!(problem(Vec::new()), FormatProblem::Size { size: 0 });
    let mut cut = SyntheticDatabase::new().build();
    cut.pop();
    let size = cut.len() as u64;
    assert_eq!(problem(cut), FormatProblem::Size { size });

    let mut newer = SyntheticDatabase::new().build();
    newer[0x1a] = 13;
    let FormatProblem::Version { found } = problem(newer) else {
        panic!("a newer version is no other problem");
    };
    assert_eq!((found.major(), found.minor(), found.patch()), (3, 201, 13));
    assert!(found > FormatVersion::SQL_ANYWHERE_17);
    let mut unflagged = SyntheticDatabase::new().build();
    unflagged[0x06] = 0x08;
    assert_eq!(problem(unflagged), FormatProblem::Flags { found: 0x08 });
}

// Each page ends with its checksum, so it is the first page that says how long
// the pages are.
#[test]
fn a_file_of_another_page_size_is_refused_by_its_size() {
    for size in [2048, 8192, 16384, 32768] {
        let mut file = SyntheticDatabase::new().build();
        file.resize(2 * 32768, 0);
        file[0x300] = 1;
        let checksum = crc32fast::hash(&file[..size - 4]);
        file[size - 4..size].copy_from_slice(&checksum.to_le_bytes());
        let refused = Database::from_bytes(file);
        assert!(
            matches!(
                refused,
                Err(Error::Format(FormatProblem::PageSize { found })) if found == size
            ),
            "{size}: {refused:?}"
        );
    }
}

#[test]
fn a_file_in_another_encoding_is_refused() {
    let file = SyntheticDatabase::new().patch(0, 0x19c, b"sjis\0\0\0\0\0\0\0\0");
    assert!(matches!(
        Database::from_bytes(file.build()),
        Err(Error::UnsupportedEncoding { label }) if label == "sjis"
    ));
    let text = SyntheticTable::new("words")
        .column(SyntheticColumn::new("word", 9).width(40))
        .row(SyntheticRow::new([Cell::text("\u{4e2d}")]));
    let file = SyntheticDatabase::new()
        .table(text)
        .patch(0, 0x19c, b"UTF-8\0\0\0\0\0\0\0");
    let read = support::rows(&support::open(&file), "words");
    assert_eq!(read, [[support::text("\u{4e2d}")]]);
}

#[test]
fn a_file_that_is_not_there_is_told_apart_from_a_damaged_one() {
    let missing = Database::open("no-such-file.db").unwrap_err();
    assert!(
        matches!(&missing, Error::Read(error) if error.kind() == ErrorKind::NotFound),
        "{missing:?}"
    );
    // One bit of the number of the database's pages, or of anything else that
    // the first page says.
    for offset in [0x1c, 0x09, 0x700] {
        let mut file = SyntheticDatabase::new().build();
        file[offset] ^= 0x40;
        let damaged = Database::from_bytes(file).unwrap_err();
        assert!(
            matches!(
                damaged,
                Error::Page {
                    page: 0,
                    problem: PageProblem::Checksum { .. }
                }
            ),
            "{damaged:?}"
        );
    }
}

// The list on page 10 has no second page in any file that was looked at.
#[test]
fn a_page_list_that_goes_on_elsewhere_is_refused() {
    for (offset, bytes) in [(20, [11, 0]), (18, [0xff, 0xff]), (18, [23, 0])] {
        let file = SyntheticDatabase::new().patch(10, offset, &bytes);
        let refused = Database::from_bytes(file.build()).unwrap_err();
        assert!(
            matches!(
                refused,
                Error::Page {
                    page: 10,
                    problem: PageProblem::Directory
                }
            ),
            "{refused:?}"
        );
    }
}

#[test]
fn debug_output_holds_no_contents() {
    let database = support::open(&SyntheticDatabase::new().table(support::catalogue(3)));
    let page = support::definition(&database, "stars").pages()[0];
    let page = database.store().page(page).unwrap();
    let record = page.record(0).unwrap().unwrap();
    assert_eq!(record.address(), RowAddress::new(page.number(), 0));
    let shown = format!("{database:?} {:?} {page:?} {record:?}", database.store());
    assert!(!shown.contains("star") && shown.len() < 1000, "{shown}");
}
