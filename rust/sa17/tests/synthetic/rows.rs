//! Rows: dead, moved, old and new, and one by one.

use sa17::{
    Columns, Error, PageKind, PageProblem, Row, RowAddress, RowId, RowProblem, Source, Value,
    ValueProblem,
};
use sa17_synthetic::{Cell, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable};

use crate::support;

#[test]
fn a_row_keeps_the_columns_of_its_schema_version() {
    let table = SyntheticTable::new("versions")
        .column(SyntheticColumn::new("id", 2))
        .column(
            SyntheticColumn::new("note", 9)
                .width(20)
                .nullable()
                .added_in(1, None),
        )
        .column(
            SyntheticColumn::new("level", 1)
                .width(2)
                .added_in(2, Some(Cell::small_int(7))),
        )
        .row(SyntheticRow::new([Cell::integer(1)]))
        .row(SyntheticRow::new([Cell::integer(2), Cell::text("second")]).schema(1))
        .row(SyntheticRow::new([Cell::integer(3), Cell::Null, Cell::small_int(9)]).schema(2))
        // The forward of a row with a schema version carries the flag too.
        .row(
            SyntheticRow::new([Cell::integer(4), Cell::text("moved"), Cell::small_int(0)])
                .schema(2)
                .forwarded(),
        );
    let database = support::open(&SyntheticDatabase::new().table(table));
    let versions = support::definition(&database, "versions");
    assert_eq!(versions.schema(), 2);
    assert_eq!(versions.columns()[2].first_schema(), 2);
    assert_eq!(
        versions.columns()[2].absent_value(),
        Some(&Value::Integer(7))
    );

    let expected = [
        [Value::Integer(1), Value::Null, Value::Integer(7)],
        [
            Value::Integer(2),
            support::text("second"),
            Value::Integer(7),
        ],
        [Value::Integer(3), Value::Null, Value::Integer(9)],
        [Value::Integer(4), support::text("moved"), Value::Integer(0)],
    ];
    assert_eq!(support::rows(&database, "versions"), expected);
}

#[test]
fn deleted_rows_and_stale_pages_are_skipped_and_forwards_followed() {
    let row = |id, name: &str| SyntheticRow::new([Cell::integer(id), Cell::text(name)]);
    let table = SyntheticTable::new("moves")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("name", 9).width(20))
        .row(row(1, "kept").slack(7))
        .row(row(2, "deleted").deleted())
        .row(SyntheticRow::empty_slot())
        .row(row(3, "moved").forwarded())
        .row(row(4, "replaced").replaced())
        .row(row(5, "moved, with a flag").forwarded().moved_flags(0x08))
        .stale_row(row(99, "stale"));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let expected = [
        [Value::Integer(1), support::text("kept")],
        [Value::Integer(3), support::text("moved")],
        [Value::Integer(5), support::text("moved, with a flag")],
    ];
    assert_eq!(support::rows(&database, "moves"), expected);
    assert_eq!(support::definition(&database, "moves").row_count(), 3);
    assert!(database.verify().is_sound());
}

// The page after the table's own carries the table's identifier, and is not
// on its list.
#[test]
fn a_row_of_a_page_that_the_table_gave_up_is_not_read() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]);
    let table = support::numbers("seeds", &[1]).stale_row(row(99));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let seeds = database.table("seeds").unwrap();
    let definition = seeds.definition();
    let page = definition.pages()[0] + 1;
    let stale = database.store().page(page).unwrap();
    assert_eq!(stale.table_id(), Some(definition.id()));
    assert_eq!(stale.records().unwrap().count(), 1);

    let address = RowAddress::new(page, 0);
    let every = Columns::all(definition);
    let source: &dyn Source = &database;
    let read = [
        seeds.row(address, &every),
        source.fetch(definition, &every, address.into()),
    ];
    for read in read {
        assert!(
            matches!(
                read,
                Err(Error::Row {
                    address: at,
                    problem: RowProblem::Page {
                        problem: PageProblem::Unlisted,
                        ..
                    },
                    ..
                }) if at == address
            ),
            "{read:?}"
        );
    }
}

/// What the rows of `table` fail with, and the numbers of those that do not.
fn read_numbers(table: SyntheticTable) -> (Vec<i64>, Vec<RowProblem>) {
    let name = "moves";
    let database = support::open(&SyntheticDatabase::new().table(table));
    let table = database.table(name).unwrap();
    let mut read = Vec::new();
    let mut refused = Vec::new();
    for (row, address) in table.rows().zip(table.row_addresses()) {
        let address = address.unwrap();
        match row {
            Ok(row) => {
                assert_eq!(row.id(), address.into());
                read.push(row.read(0).unwrap());
            }
            Err(Error::Row {
                table,
                address: at,
                problem,
            }) => {
                assert_eq!((table.as_str(), at), (name, address));
                refused.push(problem);
            }
            Err(error) => panic!("{error}"),
        }
    }
    (read, refused)
}

#[test]
fn a_forward_that_does_not_lead_to_a_live_row_is_refused() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]).forwarded();
    let table = support::numbers("moves", &[])
        .row(row(1))
        .row(row(2).moved_flags(0x20))
        .row(row(3).moved_flags(0x01))
        .row(row(4).moved_flags(0x04))
        .row(row(5).moved_flags(0x02))
        // The table never had a second version.
        .row(row(6).schema(1))
        .row(row(7));
    let (read, refused) = read_numbers(table);
    assert_eq!(read, [1, 7]);
    // The extension page is the first one, and the moved rows are in order on it.
    let to = |slot| RowAddress::new(11, slot);
    assert_eq!(
        refused,
        [
            RowProblem::Forward { to: to(1) },
            RowProblem::Forward { to: to(2) },
            RowProblem::Forward { to: to(3) },
            RowProblem::Flags { flags: 0x02 },
            RowProblem::SchemaVersion { version: 1 },
        ]
    );
}

/// The nine bytes that are in the place of a row that moved.
fn forward(to: RowAddress) -> Vec<u8> {
    let mut record = vec![9, 0, 4];
    record.extend(to.page().to_le_bytes());
    record.extend(to.slot().to_le_bytes());
    record
}

// A row that moved is in one place, so no two forwards lead to it.
#[test]
fn a_scan_follows_no_two_forwards_to_one_row() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]).forwarded();
    let mut table = support::numbers("moves", &[]);
    for id in 1..=3 {
        table = table.row(row(id));
    }
    let file = SyntheticDatabase::new().table(table);
    let [first, second] = [0, 1].map(|slot| forward(RowAddress::new(11, slot)));
    let file = support::replace(&file, "moves", &second, &first);
    let database = support::open(&file);
    let moves = database.table("moves").unwrap();
    let read: Vec<_> = moves.rows().collect();
    assert_eq!(read.len(), 3);
    assert!(read[0].is_ok() && read[2].is_ok());
    assert!(matches!(
        &read[1],
        Err(Error::Row {
            problem: RowProblem::RepeatedForward { to },
            ..
        }) if *to == RowAddress::new(11, 0)
    ));
    // Each of them can still be asked for on its own, by a program that wants to look.
    let every = Columns::all(moves.definition());
    let addresses = moves.row_addresses().map(Result::unwrap);
    let alone = addresses.filter(|&address| moves.row(address, &every).is_ok());
    assert_eq!(alone.count(), 3);
}

// The catalog lists the pages that moved rows are on, too.
#[test]
fn a_row_that_moved_to_a_page_that_is_not_the_tables_is_refused() {
    let moved = SyntheticRow::new([Cell::integer(1)]).forwarded();
    let file = SyntheticDatabase::new()
        .table(support::numbers("moves", &[]).row(moved))
        .table(support::numbers("others", &[2]));
    let whole = support::open(&file);
    let moves = support::definition(&whole, "moves");
    assert_eq!(moves.extension_pages(), [11]);
    // The same page under another number: it carries the table's identifier,
    // and is not on its list.
    let page = whole.store().page(11).unwrap();
    let stale = support::definition(&whole, "others").pages()[0];
    let mut copy = page.bytes()[..4092].to_vec();
    copy[..4].copy_from_slice(&stale.to_le_bytes());
    let file = support::replace(
        &file.patch(stale, 0, &copy),
        "moves",
        &forward(RowAddress::new(11, 0)),
        &forward(RowAddress::new(stale, 0)),
    );
    let database = support::open(&file);
    let refused = database.table("moves").unwrap().rows().next().unwrap();
    assert!(
        matches!(
            refused,
            Err(Error::Row {
                problem: RowProblem::Page {
                    page,
                    problem: PageProblem::Unlisted
                },
                ..
            }) if page == stale
        ),
        "{refused:?}"
    );
}

#[test]
fn moved_old_and_long_rows_read_as_written() {
    let database = support::open(&support::sky());
    let planets = support::rows(&database, "planets");
    let names: Vec<&Value> = planets.iter().map(|row| &row[1]).collect();
    assert_eq!(
        names,
        [
            &support::text("First"),
            &support::text("Second"),
            &support::text("Third")
        ]
    );
    assert_eq!(planets[1][2].to_string(), "-99.99");
    assert_eq!(planets[2][4], support::text(&"Seen at dusk. ".repeat(40)));

    let moons = support::rows(&database, "moons");
    let remarks: Vec<&Value> = moons.iter().map(|row| &row[2]).collect();
    let expected = [
        &Value::Null,
        &Value::Null,
        &support::text("Faint"),
        &Value::Null,
    ];
    assert_eq!(remarks, expected);
}

// 0x08 is on live rows and changes nothing. The other bits are not known.
#[test]
fn a_row_with_an_unknown_flag_is_refused() {
    let row = |id| SyntheticRow::new([Cell::integer(id)]);
    let table = SyntheticTable::new("flags")
        .column(SyntheticColumn::new("id", 2))
        .row(row(1).flags(0x08))
        .row(row(2).flags(0x02))
        .row(row(3).flags(0x10).forwarded())
        .row(row(4).deleted().flags(0x80))
        .row(row(5));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let table = database.table("flags").unwrap();
    let mut refused = Vec::new();
    let mut read = Vec::new();
    for row in table.rows() {
        match row {
            Ok(row) => read.push(row.read::<i64>(0).unwrap()),
            Err(Error::Row {
                problem: RowProblem::Flags { flags },
                ..
            }) => refused.push(flags),
            Err(error) => panic!("{error}"),
        }
    }
    assert_eq!((read, refused), (vec![1, 5], vec![0x02, 0x10, 0x80]));
}

#[test]
fn an_iterator_that_ended_stays_ended() {
    let database = support::open(&SyntheticDatabase::new().table(support::catalogue(2)));
    let table = database.table("stars").unwrap();
    let mut rows = table.rows();
    assert_eq!(rows.by_ref().count(), 2);
    assert!(rows.next().is_none() && rows.next().is_none());
    let mut addresses = table.row_addresses();
    assert_eq!(addresses.by_ref().count(), 2);
    assert!(addresses.next().is_none() && addresses.next().is_none());
}

// The addresses come from the rows' flags alone, so they must name the same
// rows, in the same order, as a full read.
#[test]
fn rows_are_read_one_by_one_by_address() {
    let database = support::open(&support::sky());
    let planets = database.table("planets").unwrap();
    let addresses: Vec<_> = planets.row_addresses().map(Result::unwrap).collect();
    assert_eq!(addresses.len(), 3);

    let every = Columns::all(planets.definition());
    let read = |address| planets.row(address, &every).unwrap();
    for (address, row) in addresses.iter().zip(planets.rows()) {
        let row = row.unwrap();
        assert_eq!(row.id(), RowId::Stored(*address));
        assert_eq!(read(*address), Some(row));
    }
    // Reading out of order reads the same rows.
    let name = |address| read(address).unwrap().get("name").cloned();
    assert_eq!(name(addresses[2]), Some(support::text("Third")));
    assert_eq!(name(addresses[0]), Some(support::text("First")));

    // Slot 1 of the first page holds the deleted planet, and there is no slot 9.
    for slot in [1, 9] {
        assert_eq!(read(RowAddress::new(addresses[0].page(), slot)), None);
    }
    // A page of another table is not one of this one's, and nor is one that is
    // not there.
    let moons = support::definition(&database, "moons").pages()[0];
    for page in [moons, 0, 10, u32::MAX] {
        let elsewhere = planets.row(RowAddress::new(page, 0), &every);
        assert!(matches!(
            elsewhere,
            Err(Error::Row {
                problem: RowProblem::Page {
                    problem: PageProblem::Unlisted,
                    ..
                },
                ..
            })
        ));
    }
}

#[test]
fn a_source_scans_and_fetches_the_columns_asked_for() {
    let database = support::open(&support::sky());
    let source: &dyn Source = &database;
    let planets = source.definition("planets").unwrap();
    // In another order than the table's, one of them twice, and one it lacks.
    let columns =
        Columns::with_optional(planets, ["mass", "NAME", "id"], ["rings", "name"]).unwrap();
    assert_eq!((columns.len(), columns.table_name()), (5, "planets"));
    assert_eq!(columns.position("rings"), Some(3));
    assert_eq!(columns.column(0).unwrap().name(), "mass");
    assert!(columns.column(3).is_none() && columns.column(9).is_none());

    let mut found = Vec::new();
    for row in source.scan(planets, &columns) {
        let row = row.unwrap();
        assert_eq!(
            source.fetch(planets, &columns, row.id()).unwrap(),
            Some(row.clone())
        );
        assert_eq!(row.value(1), row.value(4));
        let (id, name): (i32, &str) = (row.read(2).unwrap(), row.read(1).unwrap());
        let rings: Option<bool> = row.read(3).unwrap();
        found.push((id, name.to_owned(), row.values()[0].to_string(), rings));
    }
    let planet = |id, name: &str, mass: &str| (id, name.to_owned(), mass.to_owned(), None);
    let expected = [
        planet(1, "First", "1234.50"),
        planet(3, "Second", "-99.99"),
        planet(4, "Third", "0.00"),
    ];
    assert_eq!(found, expected);
    // A file holds no row that something else added.
    let added = source.fetch(planets, &columns, RowId::Added(0));
    assert_eq!(added.unwrap(), None);
}

// Nothing but the engine keeps a catalog from having one name twice.
#[test]
fn columns_are_told_apart_by_their_positions() {
    let table = SyntheticTable::new("twins")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("id", 2))
        .row(SyntheticRow::new([Cell::integer(1), Cell::integer(2)]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let twins = database.table("twins").unwrap();
    let at = |position| Columns::at(twins.definition(), &[position]).unwrap();
    assert_ne!(at(0), at(1));
    assert_eq!(at(0), Columns::new(twins.definition(), ["id"]).unwrap());
    let read = |position| {
        let mut rows = twins.scan(&at(position));
        rows.next().unwrap().unwrap().read::<i64>(0).unwrap()
    };
    assert_eq!((read(0), read(1)), (1, 2));
}

// The moons' remark is in the rows of schema version 1, and older rows have its default.
#[test]
fn a_projected_row_has_the_values_of_the_whole_one() {
    let database = support::open(&support::sky());
    for name in ["planets", "moons", "ISYSTAB", "ISYSTABCOL", "ISYSIDX"] {
        let table = database.table(name).unwrap();
        for position in 0..table.columns().len() {
            let column = table.columns()[position].name();
            let columns = Columns::new(table.definition(), [column]).unwrap();
            assert_eq!(
                columns,
                Columns::at(table.definition(), &[position]).unwrap()
            );
            for (whole, part) in table.rows().zip(table.scan(&columns)) {
                let (whole, part) = (whole.unwrap(), part.unwrap());
                assert_eq!(whole.id(), part.id());
                assert_eq!(part.values(), [whole.values()[position].clone()]);
            }
        }
        let none = Columns::none(table.definition());
        assert!(none.is_empty());
        assert_eq!(table.scan(&none).count(), table.rows().count());
    }
}

// The notes of the third planet are on an extension page, which only they need.
#[test]
fn a_damaged_long_value_costs_only_who_asks_for_it() {
    let sky = support::sky();
    let whole = support::open(&sky);
    let extension = (0..whole.store().page_count())
        .find(|&number| {
            let page = whole.store().page(number).unwrap();
            page.kind() == PageKind::Extension && page.slot_count() == Some(1)
        })
        .unwrap();
    let database = support::damaged(&sky, &[extension]);
    let planets = database.table("planets").unwrap();
    let readable = |names: &[&str]| {
        let columns = Columns::new(planets.definition(), names).unwrap();
        planets.scan(&columns).filter(Result::is_ok).count()
    };
    let unreadable = planets.rows().filter(Result::is_err).count();
    assert_eq!(unreadable, 1);
    assert_eq!(readable(&["id", "notes"]), 2);
    // The page holds the moved row or the long notes, whichever comes first.
    assert!(readable(&["id"]) >= 2);
}

#[test]
fn values_are_read_as_types() {
    let database = support::open(&support::sky());
    let moons = database.table("moons").unwrap();
    let columns = Columns::new(moons.definition(), ["planet_id", "remark"]).unwrap();
    let mut rows = moons.scan(&columns).map(Result::unwrap);
    let row = rows.next().unwrap();
    assert_eq!(row.read::<u16>(0).unwrap(), 1);
    assert_eq!(row.read::<Option<&str>>(1).unwrap(), None);
    assert_eq!(row.get("Planet_Id"), Some(&Value::Integer(1)));
    assert_eq!((row.get("id"), row.value(9)), (None, None));
    let refused = |result: sa17::Result<String>| match result {
        Err(Error::Value {
            column, problem, ..
        }) => (column, problem),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        refused(row.read(0)),
        (
            "planet_id".to_owned(),
            ValueProblem::Type {
                expected: "text",
                found: "a whole number"
            }
        )
    );
    assert_eq!(
        refused(row.read(1)),
        ("remark".to_owned(), ValueProblem::Null { expected: "text" })
    );
    assert_eq!(
        refused(row.read(9)),
        ("9".to_owned(), ValueProblem::Missing)
    );
    // A name does as well as a position.
    assert_eq!(row.read::<u16>("PLANET_ID").unwrap(), 1);
    assert_eq!(row.read::<Option<&str>>("remark").unwrap(), None);
    assert_eq!(
        refused(row.read("Remark")),
        ("Remark".to_owned(), ValueProblem::Null { expected: "text" })
    );
    assert_eq!(
        refused(row.read("id")),
        ("id".to_owned(), ValueProblem::Missing)
    );
    let error = row.read::<String>(1).unwrap_err().to_string();
    assert!(error.starts_with("table moons, page "), "{error}");
}

#[test]
fn a_row_is_made_of_a_value_for_each_column() {
    let database = support::open(&support::sky());
    let moons = support::definition(&database, "moons");
    let columns = Columns::new(moons, ["id", "remark"]).unwrap();
    let values = vec![Value::Integer(7), support::text("Laid over")];
    let mut row = Row::new(RowId::Added(3), &columns, values).unwrap();
    assert_eq!((row.id(), row.id().address()), (RowId::Added(3), None));
    row.values_mut()[0] = Value::Integer(8);
    assert_eq!(row.read::<i64>(0).unwrap(), 8);
    assert!(Row::new(RowId::Added(3), &columns, vec![Value::Null]).is_none());
    assert!(RowId::Stored(RowAddress::new(u32::MAX, 9)) < RowId::Added(0));
}
