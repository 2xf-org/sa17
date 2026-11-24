//! The files that the tests share, and what most of them do with one.

use sa17::{Database, TableDefinition, Value};
use sa17_synthetic::{
    Cell, IndexOrder, LongStorage, Sign, SyntheticColumn, SyntheticDatabase, SyntheticRow,
    SyntheticTable,
};

/// The bytes in a page of every file here.
pub(crate) const PAGE_SIZE: usize = 4096;

pub(crate) fn open(database: &SyntheticDatabase) -> Database {
    Database::from_bytes(database.build()).unwrap()
}

pub(crate) fn rows(database: &Database, table: &str) -> Vec<Vec<Value>> {
    let table = database.table(table).unwrap();
    table.rows().map(|row| row.unwrap().into_values()).collect()
}

pub(crate) fn definition<'db>(database: &'db Database, table: &str) -> &'db TableDefinition {
    database.table(table).unwrap().definition()
}

pub(crate) fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// The file with one byte of page `page` changed, so that it fails its checksum.
pub(crate) fn damaged(file: &SyntheticDatabase, pages: &[u32]) -> Database {
    let mut bytes = file.build();
    for &page in pages {
        bytes[page as usize * PAGE_SIZE + 100] ^= 0xff;
    }
    Database::from_bytes(bytes).unwrap()
}

/// Where the first page of `table` has `bytes`, once it can be read.
pub(crate) fn find(file: &SyntheticDatabase, table: &str, bytes: &[u8]) -> (u32, usize) {
    let database = open(file);
    let page = definition(&database, table).pages()[0];
    let plain = database.store().page(page).unwrap();
    let mut found = plain.bytes().windows(bytes.len());
    let offset = found.position(|window| window == bytes).unwrap();
    (page, offset)
}

/// The file with `new` in the place of `old` in the first row of `table` that
/// has it.
pub(crate) fn replace(
    file: &SyntheticDatabase,
    table: &str,
    old: &[u8],
    new: &[u8],
) -> SyntheticDatabase {
    let (page, offset) = find(file, table, old);
    file.clone().patch(page, offset, new)
}

/// What the row of `ISYSTAB` for the table with identifier `id` starts with:
/// the identifier, and the two zeros of the file it is in.
pub(crate) fn table_row(id: u32) -> Vec<u8> {
    let mut start = id.to_le_bytes().to_vec();
    start.extend([0, 0]);
    start
}

/// What the row of `ISYSTABCOL` for a column starts with.
pub(crate) fn column_row(table: u32, number: u32) -> Vec<u8> {
    let mut start = table.to_le_bytes().to_vec();
    start.extend(number.to_le_bytes());
    start
}

/// A table with enough rows to need several pages.
pub(crate) fn catalogue(rows: i32) -> SyntheticTable {
    let mut table = SyntheticTable::new("stars")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("name", 9).width(60));
    for id in 0..rows {
        let name = format!("star number {id} of the catalogue");
        table = table.row(SyntheticRow::new([Cell::integer(id), Cell::text(&name)]));
    }
    table
}

/// A table of one number to a row.
pub(crate) fn numbers(name: &str, numbers: &[i32]) -> SyntheticTable {
    let mut table = SyntheticTable::new(name).column(SyntheticColumn::new("id", 2));
    for &number in numbers {
        table = table.row(SyntheticRow::new([Cell::integer(number)]));
    }
    table
}

/// 2 January 2000, in minutes since 29 February 1600.
const A_DAY: u32 = 210_296_160;

/// Planets and their moons, and a view of the planets. One planet's row was
/// deleted, one moved to an extension page, and one has notes too long for its
/// row. The moons' remark is newer than the first two of them.
pub(crate) fn sky() -> SyntheticDatabase {
    let planet = |id, name: &str, mass: Cell, notes| {
        SyntheticRow::new([
            Cell::integer(id),
            Cell::text(name),
            mass,
            Cell::date(A_DAY),
            notes,
        ])
    };
    let notes = Cell::long(
        "Seen at dusk. ".repeat(40).as_bytes(),
        LongStorage::Compressed,
    );
    let zero = || Cell::numeric(Sign::Positive, 0, &[]);
    let planets = SyntheticTable::new("planets")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("name", 9).width(40))
        .column(SyntheticColumn::new("mass", 3).width(15).scale(2))
        .column(SyntheticColumn::new("seen", 6))
        .column(SyntheticColumn::new("notes", 10).width(32767).nullable())
        .primary_key(&["id"])
        .row(planet(
            1,
            "First",
            Cell::numeric(Sign::Positive, -1, &[50, 34, 12]),
            Cell::Null,
        ))
        .row(planet(2, "Lost", zero(), Cell::Null).deleted())
        .row(
            planet(
                3,
                "Second",
                Cell::numeric(Sign::Negative, -1, &[99, 99]),
                Cell::Null,
            )
            .forwarded(),
        )
        .row(planet(4, "Third", zero(), notes));
    let moon = |id, planet| [Cell::integer(id), Cell::integer(planet)];
    let remarked = |id, planet, remark| {
        let [id, planet] = moon(id, planet);
        SyntheticRow::new([id, planet, remark]).schema(1)
    };
    let moons = SyntheticTable::new("moons")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("planet_id", 2))
        .column(
            SyntheticColumn::new("remark", 9)
                .width(60)
                .nullable()
                .added_in(1, None),
        )
        .primary_key(&["id"])
        .foreign_key("moon_planet", &["planet_id"], "planets")
        .row(SyntheticRow::new(moon(100, 1)))
        .row(SyntheticRow::new(moon(101, 1)))
        .row(remarked(102, 3, Cell::text("Faint")))
        .row(remarked(103, 4, Cell::Null));
    let names = SyntheticTable::new("planet_names")
        .view()
        .column(SyntheticColumn::new("name", 9).width(40));
    SyntheticDatabase::new()
        .table(planets)
        .table(moons)
        .table(names)
}

/// Parcels in boxes: a box has a key of two columns, which a parcel holds.
pub(crate) fn parcels() -> [SyntheticTable; 2] {
    let boxes = SyntheticTable::new("boxes")
        .column(SyntheticColumn::new("label", 9).width(20))
        .column(SyntheticColumn::new("shelf", 2))
        .column(SyntheticColumn::new("number", 2))
        .primary_key(&["shelf", "number"])
        .index("by_label", &["label"], IndexOrder::Descending);
    let parcels = SyntheticTable::new("parcels")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("box_number", 2).nullable())
        .column(SyntheticColumn::new("box_shelf", 2).nullable())
        .column(SyntheticColumn::new("inside", 2).nullable())
        .primary_key(&["id"])
        .foreign_key("parcel_box", &["box_shelf", "box_number"], "boxes")
        .foreign_key("parcel_parcel", &["inside"], "parcels");
    [parcels, boxes]
}
