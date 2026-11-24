//! Keys and indexes, and looking rows up by them.

use std::collections::HashMap;

use sa17::{
    CatalogProblem, Columns, Error, IndexKind, IndexOrder, RowId, RowKey, TableDefinition, Value,
};
use sa17_synthetic::{Cell, SyntheticDatabase, SyntheticRow};

use crate::support;

fn parcels() -> SyntheticDatabase {
    let [parcels, boxes] = support::parcels();
    SyntheticDatabase::new().table(parcels).table(boxes)
}

#[test]
fn keys_and_indexes_are_read_with_their_columns() {
    let database = support::open(&parcels());
    let (parcels, boxes) = (
        support::definition(&database, "parcels"),
        support::definition(&database, "boxes"),
    );
    assert_eq!(parcels.primary_key(), Some(&[0][..]));
    assert_eq!(boxes.primary_key(), Some(&[1, 2][..]));

    let [to_box, to_parcel] = parcels.foreign_keys() else {
        panic!("{:?}", parcels.foreign_keys());
    };
    assert_eq!(
        (to_box.name(), to_box.target_id()),
        ("parcel_box", boxes.id())
    );
    // The shelf is the third column of a parcel and the second of a box.
    assert_eq!(to_box.positions(), [2, 1]);
    assert_eq!(to_box.target_positions(), [1, 2]);
    assert_eq!(
        (to_parcel.target_id(), to_parcel.positions()),
        (parcels.id(), &[3][..])
    );
    assert!(boxes.foreign_keys().is_empty());

    let kinds = |table: &TableDefinition| -> Vec<(IndexKind, bool)> {
        let indexes = table.indexes().iter();
        indexes
            .map(|index| (index.kind(), index.is_unique()))
            .collect()
    };
    assert_eq!(
        kinds(parcels),
        [
            (IndexKind::PrimaryKey, true),
            (IndexKind::ForeignKey, false),
            (IndexKind::ForeignKey, false)
        ]
    );
    let [_, by_label] = boxes.indexes() else {
        panic!("{:?}", boxes.indexes());
    };
    assert_eq!(
        (by_label.id(), by_label.name(), by_label.kind()),
        (1, "by_label", IndexKind::Secondary)
    );
    let column = by_label.columns()[0];
    assert_eq!(
        (column.position(), column.order()),
        (0, IndexOrder::Descending)
    );

    let plain = support::open(&SyntheticDatabase::new().table(support::catalogue(1)));
    let stars = support::definition(&plain, "stars");
    assert_eq!(stars.primary_key(), None);
    assert!(stars.foreign_keys().is_empty() && stars.indexes().is_empty());
}

// A program goes from the key of a parcel to the box that it names.
#[test]
fn a_row_is_found_by_the_key_that_another_has() {
    let [mut parcels, mut boxes] = support::parcels();
    for (label, shelf, number) in [("small", 1, 2), ("large", 2, 1)] {
        boxes = boxes.row(SyntheticRow::new([
            Cell::text(label),
            Cell::integer(shelf),
            Cell::integer(number),
        ]));
    }
    let places = [
        [Cell::integer(1), Cell::integer(2)],
        [Cell::integer(2), Cell::integer(1)],
        [Cell::Null, Cell::integer(2)],
    ];
    for (id, [number, shelf]) in (1..).zip(places) {
        parcels = parcels.row(SyntheticRow::new([
            Cell::integer(id),
            number,
            shelf,
            Cell::Null,
        ]));
    }
    let database = support::open(&SyntheticDatabase::new().table(parcels).table(boxes));
    let parcels = database.table("parcels").unwrap();
    let key = &parcels.definition().foreign_keys()[0];
    let boxes = parcels.target(key).unwrap();

    let named = Columns::at(boxes.definition(), key.target_positions()).unwrap();
    let found = boxes.scan(&named).map(|row| {
        let row = row.unwrap();
        (row.key().unwrap(), row.id())
    });
    let found: HashMap<RowKey, RowId> = found.collect();
    let naming = Columns::at(parcels.definition(), key.positions()).unwrap();
    assert_eq!(
        naming.names().collect::<Vec<_>>(),
        ["box_shelf", "box_number"]
    );
    let every = Columns::all(boxes.definition());
    let labels = parcels.scan(&naming).map(|row| {
        let id = *found.get(&row.unwrap().key()?).unwrap();
        let row = boxes.row(id.address().unwrap(), &every).unwrap().unwrap();
        Some(row.read::<String>("label").unwrap())
    });
    assert_eq!(
        labels.collect::<Vec<_>>(),
        [Some("large".to_owned()), Some("small".to_owned()), None]
    );

    let missing = Columns::at(boxes.definition(), &[0, 3]).unwrap_err();
    assert_eq!(
        missing.to_string(),
        "the table boxes has no column at position 3"
    );
}

// The keys say how tables belong together, and the rows read without them.
#[test]
fn a_file_whose_key_columns_cannot_be_read_opens_without_keys() {
    let sky = support::sky();
    let whole = support::open(&sky);
    assert!(whole.catalog_problems().is_empty());
    let columns_page = support::definition(&whole, "ISYSIDXCOL").pages()[0];
    let database = support::damaged(&sky, &[columns_page]);
    assert!(matches!(
        database.catalog_problems(),
        [Error::TablePage { page, .. }] if *page == columns_page
    ));
    let moons = support::definition(&database, "moons");
    assert!(moons.primary_key().is_none() && moons.indexes().is_empty());
    assert_eq!(moons.owner(), "owner");
    assert_eq!(support::rows(&database, "moons").len(), 4);
    assert!(!database.verify().is_sound());
}

/// What the row of `ISYSIDXCOL` for a column of an index starts with, its
/// place in the index last.
fn index_column(table: u32, index: u32, sequence: u16) -> Vec<u8> {
    let mut start = table.to_le_bytes().to_vec();
    start.extend(index.to_le_bytes());
    start.extend(sequence.to_le_bytes());
    start
}

#[test]
fn the_columns_of_an_index_are_in_its_own_order() {
    let file = parcels();
    let [first, second] = [0, 1].map(|sequence| index_column(1001, 0, sequence));
    let (page, at_first) = support::find(&file, "ISYSIDXCOL", &first);
    let (_, at_second) = support::find(&file, "ISYSIDXCOL", &second);
    let turned = file
        .patch(page, at_first, &second)
        .patch(page, at_second, &first);
    let database = support::open(&turned);
    let boxes = support::definition(&database, "boxes");
    assert_eq!(boxes.primary_key(), Some(&[2, 1][..]));
}

#[test]
fn a_key_that_names_what_is_not_there_costs_only_itself() {
    let sky = support::sky();
    // The numbers of the index that the key is, then its target's.
    let mut key = 1001_u32.to_le_bytes().to_vec();
    key.extend(1_u32.to_le_bytes());
    let mut dangling = key.clone();
    key.extend(1000_u32.to_le_bytes());
    dangling.extend(0xffff_u32.to_le_bytes());
    let database = support::open(&support::replace(&sky, "ISYSFKEY", &key, &dangling));
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::Index {
            table_id: 1001,
            index_id: 1
        })]
    ));
    let moons = support::definition(&database, "moons");
    assert!(moons.foreign_keys().is_empty());
    assert_eq!(moons.primary_key(), Some(&[0][..]));
    assert_eq!(moons.indexes().len(), 2);
    assert_eq!(support::rows(&database, "planets").len(), 3);
}

// The index behind the key has its columns, and the key has none to go to.
#[test]
fn a_key_to_a_column_that_is_not_there_costs_only_itself() {
    let sky = support::sky();
    // After the column's own number and order comes the one in the other table.
    let mut column = index_column(1001, 1, 0);
    column.extend(2_u32.to_le_bytes());
    column.extend([1, b'A']);
    let [known, unknown] = [1_u32, 9].map(|target| [&column[..], &target.to_le_bytes()].concat());
    let database = support::open(&support::replace(&sky, "ISYSIDXCOL", &known, &unknown));
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::Index {
            table_id: 1001,
            index_id: 1
        })]
    ));
    let moons = support::definition(&database, "moons");
    assert!(moons.foreign_keys().is_empty());
    assert_eq!(moons.indexes().len(), 2);
}

#[test]
fn an_index_of_an_unknown_order_costs_only_itself() {
    // The label's order is in the row of the one column of its index.
    let mut row = index_column(1001, 1, 0);
    row.extend(1_u32.to_le_bytes());
    let mut unknown = row.clone();
    row.extend([1, b'D']);
    unknown.extend([1, b'X']);
    let file = parcels();
    let database = support::open(&support::replace(&file, "ISYSIDXCOL", &row, &unknown));
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::Index {
            table_id: 1001,
            index_id: 1
        })]
    ));
    let boxes = support::definition(&database, "boxes");
    assert_eq!(
        (boxes.indexes().len(), boxes.primary_key().is_some()),
        (1, true)
    );
    // A place below zero is none, and nothing says whose column it is.
    let file = support::replace(
        &file,
        "ISYSIDXCOL",
        &index_column(1001, 1, 0),
        &index_column(1001, 1, 0xffff),
    );
    let database = support::open(&file);
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::Field {
            table: "ISYSIDXCOL",
            column: "sequence"
        })]
    ));
    assert!(support::definition(&database, "boxes").indexes().is_empty());
}

#[test]
fn a_file_without_one_of_the_key_tables_has_no_keys() {
    let file = parcels();
    let renamed = |name: &str, new: &[u8]| {
        support::open(&support::replace(&file, "ISYSTAB", name.as_bytes(), new))
    };
    let database = renamed("ISYSFKEY", b"NOTHINGS");
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::MissingTable {
            table: "ISYSFKEY"
        })]
    ));
    assert!(
        support::definition(&database, "boxes")
            .primary_key()
            .is_none()
    );
    // A file without ISYSIDX declares none, and that is no problem. Its name
    // is the start of another's, so the length in front of it tells them apart.
    let database = renamed("\x07ISYSIDX", b"\x07NOTHING");
    assert!(database.catalog_problems().is_empty());
    assert!(support::definition(&database, "boxes").indexes().is_empty());
    assert_eq!(support::rows(&database, "boxes"), [[Value::Null; 0]; 0]);
}
