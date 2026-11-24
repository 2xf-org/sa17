//! The lists of pages that the catalog has for each table.

use sa17::{Columns, Database, Error, PageKind, PageListProblem, RowAddress, RowProblem, Value};
use sa17_synthetic::{Cell, LongStorage, SyntheticDatabase, SyntheticRow};

use crate::support;

/// Three tables of one page each, which are the pages from 11 on.
fn three() -> SyntheticDatabase {
    let mut file = SyntheticDatabase::new();
    for (name, number) in [("one", 1), ("two", 2), ("three", 3)] {
        file = file.table(support::numbers(name, &[number]));
    }
    file
}

/// A page list as the writer makes it: 2048 bits, of which the first 32 are
/// `bits`.
fn list(bits: u32) -> Vec<u8> {
    let words = [2048, 1, bits, 63 << 7];
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// The three tables, the second of them with another list.
fn with_second(list_of_second: &[u8]) -> SyntheticDatabase {
    support::replace(&three(), "ISYSTAB", &list(1 << 12), list_of_second)
}

/// What is wrong with the list of the second table, which costs it its rows,
/// and the others nothing.
fn problem(file: &SyntheticDatabase) -> PageListProblem {
    let database = support::open(file);
    let [Error::PageList { table, problem }] = database.catalog_problems() else {
        panic!("{:?}", database.catalog_problems());
    };
    assert_eq!(table, "two");
    let two = database.table("two").unwrap();
    let every = Columns::all(two.definition());
    let errors = [
        two.rows().next().unwrap().unwrap_err(),
        two.row_addresses().next().unwrap().unwrap_err(),
        two.row(RowAddress::new(12, 0), &every).unwrap_err(),
    ];
    for error in errors {
        assert!(
            matches!(&error, Error::PageList { table, problem: found } if table == "two" && found == problem),
            "{error:?}"
        );
    }
    assert_eq!(two.rows().count(), 1);
    assert!(two.definition().pages().is_empty());
    assert_eq!(support::rows(&database, "one"), [[Value::Integer(1)]]);
    assert_eq!(support::rows(&database, "three"), [[Value::Integer(3)]]);

    let verification = database.verify();
    assert!(!verification.is_sound());
    assert_eq!(
        (
            verification.catalog_problems(),
            verification.unreadable_pages()
        ),
        (1, 1)
    );
    *problem
}

#[test]
fn a_list_that_does_not_decode_costs_its_table_the_rows() {
    let mut odd = list(1 << 12);
    odd[0] = 1;
    assert_eq!(problem(&with_second(&odd)), PageListProblem::Size);
    let mut short = list(1 << 12);
    short[12] = 0;
    assert_eq!(problem(&with_second(&short)), PageListProblem::Runs);
}

#[test]
fn a_list_names_no_page_past_the_end_of_the_file() {
    let count = support::open(&three()).store().page_count();
    assert!((14..32).contains(&count), "{count}");
    let last = with_second(&list(1 << (count - 1)));
    assert!(support::open(&last).catalog_problems().is_empty());
    let past = with_second(&list(1 << count));
    assert_eq!(problem(&past), PageListProblem::PastEnd { page: count });
}

// A page belongs to one table, so the lists of all have no more than there are.
#[test]
fn the_lists_together_name_no_more_pages_than_there_are() {
    let count = support::open(&three()).store().page_count();
    let every = with_second(&list((1 << count) - 1));
    assert_eq!(problem(&every), PageListProblem::TooMany);
    let database = support::open(&every);
    let listed = database.catalog().iter().map(|table| table.pages().len());
    assert!(listed.sum::<usize>() <= count as usize);
}

#[test]
fn a_list_that_is_kept_outside_its_row_is_read() {
    for storage in [LongStorage::OnePiece, LongStorage::Chained] {
        let file = SyntheticDatabase::new()
            .table(support::numbers("one", &[1]))
            .table(support::numbers("two", &[2]).long_page_list(storage))
            .table(support::numbers("three", &[3]));
        let whole = support::open(&file);
        assert_eq!(support::rows(&whole, "two"), [[Value::Integer(2)]]);
        assert!(whole.verify().is_sound());
        // The catalog lists no extension pages for ISYSTAB itself.
        let catalog = support::definition(&whole, "ISYSTAB");
        assert!(catalog.extension_pages().is_empty());
        let pieces = (0..whole.store().page_count()).find(|&number| {
            let page = whole.store().page(number).unwrap();
            page.kind() == PageKind::Extension && page.table_id() == Some(catalog.id())
        });
        // Without its piece the list is lost, and the row of the table is not.
        let database = support::damaged(&file, &[pieces.unwrap()]);
        let [read, lost] = database.catalog_problems() else {
            panic!("{:?}", database.catalog_problems());
        };
        assert!(matches!(
            read,
            Error::Row {
                table,
                problem: RowProblem::LongValue { .. },
                ..
            } if table == "ISYSTAB"
        ));
        assert!(matches!(
            lost,
            Error::PageList {
                table,
                problem: PageListProblem::Unreadable
            } if table == "two"
        ));
        assert_eq!(support::rows(&database, "one"), [[Value::Integer(1)]]);
        let mut two = database.table("two").unwrap().rows();
        assert!(matches!(two.next(), Some(Err(Error::PageList { .. }))));
        assert!(two.next().is_none());
    }
}

// A row of ISYSTAB that cannot be read is a table that is not there.
#[test]
fn a_table_whose_row_cannot_be_read_is_not_listed() {
    let file = three();
    let (page, row) = support::find(&file, "ISYSTAB", &support::table_row(1001));
    // The flags are before the one byte of the bitmap.
    let database = support::open(&file.patch(page, row - 2, &[0x02]));
    assert!(matches!(
        database.catalog_problems(),
        [Error::Row {
            problem: RowProblem::Flags { flags: 0x02 },
            ..
        }]
    ));
    assert!(matches!(
        database.table("two"),
        Err(Error::UnknownTable { .. })
    ));
    assert_eq!(support::rows(&database, "three"), [[Value::Integer(3)]]);
    assert!(!database.verify().is_sound());
}

/// A page of rows with `records`, up to its checksum.
fn row_page(number: u32, kind: u8, table_id: u32, records: &[Vec<u8>]) -> Vec<u8> {
    let mut page = vec![0; support::PAGE_SIZE - 4];
    page[..4].copy_from_slice(&number.to_le_bytes());
    page[16..18].copy_from_slice(&[4, kind]);
    page[22..24].copy_from_slice(&(records.len() as u16).to_le_bytes());
    page[24..28].copy_from_slice(&table_id.to_le_bytes());
    let mut start = page.len();
    for (slot, record) in records.iter().enumerate() {
        start -= record.len();
        page[start..start + record.len()].copy_from_slice(record);
        let offset = (start - 28) as u16;
        page[28 + 2 * slot..30 + 2 * slot].copy_from_slice(&offset.to_le_bytes());
    }
    page
}

// Pages of ISYSTAB that are nothing but forwards to one row of it, whose list
// names every page. The forwards are 369 to a page.
#[test]
fn a_file_of_forwards_to_one_table_has_it_once() {
    let moved = support::numbers("moved", &[1, 2, 3]);
    let spare = support::numbers("spare", &[]).spare_extension_pages(4);
    let file = SyntheticDatabase::new().table(moved).table(spare);
    let whole = support::open(&file);
    let own = support::definition(&whole, "ISYSTAB").pages()[0];
    let spare = support::definition(&whole, "spare").extension_pages();
    let count = whole.store().page_count();
    assert!(count < 32, "{count}");

    // The row of the first table, under another identifier, on an extension page.
    let page = whole.store().page(own).unwrap();
    let row = page.record(0).unwrap().unwrap().bytes().to_vec();
    let mut other = row.clone();
    other[4..8].copy_from_slice(&5000_u32.to_le_bytes());
    let (extension, forwards) = (spare[0], &spare[1..]);
    let mut forward = vec![9, 0, 4];
    forward.extend(extension.to_le_bytes());
    forward.extend([0, 0]);
    let mut hostile = file
        .clone()
        .patch(extension, 0, &row_page(extension, 18, 1, &[other]));
    let mut pages = 1 << own;
    for &number in forwards {
        let records = vec![forward.clone(); 369];
        hostile = hostile.patch(number, 0, &row_page(number, 3, 1, &records));
        pages |= 1 << number;
    }
    let hostile = hostile.patch(10, 24, &list(pages));
    let database = Database::from_bytes(hostile.build()).unwrap();
    let copies = database.catalog().iter().filter(|table| table.id() == 5000);
    assert_eq!(copies.count(), 1);
    // Each of the other forwards is a problem, and the first thousand are kept.
    assert_eq!(database.catalog_problems().len(), 1000);
    let refused = database.catalog_problems().iter().filter(|error| {
        matches!(
            error,
            Error::Row {
                problem: RowProblem::RepeatedForward { .. },
                ..
            }
        )
    });
    assert_eq!(refused.count(), 1000);
}

#[test]
fn a_catalog_of_many_tables_opens() {
    let mut file = SyntheticDatabase::new();
    for number in 0..300 {
        let name = format!("table number {number}");
        let row = SyntheticRow::new([Cell::integer(number)]);
        file = file.table(support::numbers(&name, &[]).row(row));
    }
    let database = support::open(&file);
    assert_eq!(database.catalog().len(), 306);
    let last = support::rows(&database, "table number 299");
    assert_eq!(last, [[Value::Integer(299)]]);
    assert!(database.verify().is_sound());
}
