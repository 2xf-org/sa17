//! Tables, columns and the users that own them.

use sa17::{
    CatalogProblem, Columns, DataType, Database, Error, LengthUnit, PageKind, RowProblem, Source,
    Table, TableKind, TextEncoding, Value,
};
use sa17_synthetic::{
    Cell, LongStorage, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable,
};

use crate::support;

#[test]
fn a_table_spans_the_pages_its_list_names() {
    let database = support::open(&SyntheticDatabase::new().table(support::catalogue(400)));
    let table = support::definition(&database, "stars");
    assert!(table.pages().len() > 3, "{:?}", table.pages());
    assert_eq!(table.row_count(), 400);
    for &number in table.pages() {
        let page = database.store().page(number).unwrap();
        assert_eq!(page.kind(), PageKind::Table);
        assert_eq!(page.table_id(), Some(table.id()));
        assert_eq!(page.stored_number(), number);
    }
    let read = support::rows(&database, "stars");
    assert_eq!(
        read[399],
        [
            Value::Integer(399),
            support::text("star number 399 of the catalogue")
        ]
    );
}

#[test]
fn the_catalog_lists_tables_views_and_owners() {
    let database = support::open(&support::sky());
    let names: Vec<&str> = database.tables().map(Table::name).collect();
    assert_eq!(
        names,
        [
            "ISYSTAB",
            "ISYSTABCOL",
            "ISYSIDX",
            "ISYSIDXCOL",
            "ISYSFKEY",
            "ISYSUSER",
            "planets",
            "moons",
            "planet_names"
        ]
    );
    assert_eq!(database.tables().len(), database.catalog().len());
    // A view stores no rows, so it is not one of a source's tables.
    let source: &dyn Source = &database;
    assert_eq!(source.definitions().len(), names.len() - 1);
    assert!(matches!(
        source.definition("planet_names"),
        Err(Error::UnknownTable { name }) if name == "planet_names"
    ));
    assert_eq!(source.definition("MOONS").unwrap().name(), "moons");
    assert_eq!(source.text_encoding(), TextEncoding::Windows1252);

    let planets = support::definition(&database, "PLANETS");
    assert_eq!(planets.kind(), TableKind::Base);
    assert_eq!(planets.owner(), "owner");
    assert!(planets.stores_rows() && !planets.is_system());
    assert_eq!(planets.position("mass"), Some(2));
    assert_eq!(planets.columns()[4].data_type(), DataType::LongVarChar);
    assert!(planets.column("Notes").unwrap().is_nullable());
    assert!(planets.column("moons").is_none());
    assert_eq!(source.definition_by_id(planets.id()), Some(planets));

    let view = support::definition(&database, "planet_names");
    assert_eq!(view.kind(), TableKind::View);
    assert!(!view.stores_rows() && view.pages().is_empty());
    assert_eq!(source.definition_by_id(view.id()), None);
    assert_eq!(database.table_by_id(view.id()).unwrap().name(), view.name());

    let catalog = support::definition(&database, "ISYSTAB");
    assert!(catalog.is_system());
    assert_eq!(catalog.owner(), "SYS");
    assert_eq!(catalog.row_count(), 9);
    assert_eq!(support::rows(&database, "ISYSTAB").len(), 9);
    assert!(matches!(
        database.table("missing"),
        Err(Error::UnknownTable { name }) if name == "missing"
    ));
}

#[test]
fn each_kind_of_table_has_the_rows_that_it_stores() {
    let database = support::open(
        &SyntheticDatabase::new()
            .table(support::numbers("plain", &[1]))
            .table(support::numbers("kept", &[2]).materialized_view())
            .table(support::numbers("passing", &[]).temporary())
            .table(support::numbers("searched", &[4]).text_index())
            .table(support::numbers("seen", &[]).view()),
    );
    let kinds = [
        ("plain", TableKind::Base, "table"),
        ("kept", TableKind::MaterializedView, "materialized view"),
        ("passing", TableKind::GlobalTemporary, "temporary table"),
        ("searched", TableKind::TextIndex, "text index table"),
        ("seen", TableKind::View, "view"),
    ];
    for (name, kind, kind_name) in kinds {
        let definition = support::definition(&database, name);
        assert_eq!((definition.kind(), kind.name()), (kind, kind_name));
    }
    let stored = database
        .definitions()
        .iter()
        .filter(|table| !table.is_system());
    let stored: Vec<&str> = stored.map(|table| table.name()).collect();
    assert_eq!(stored, ["plain", "kept"]);
    assert_eq!(support::rows(&database, "plain"), [[Value::Integer(1)]]);
    assert_eq!(support::rows(&database, "kept"), [[Value::Integer(2)]]);
    assert!(database.verify().is_sound());
}

#[test]
fn a_column_has_its_default_and_what_its_width_counts() {
    let table = SyntheticTable::new("words")
        .column(SyntheticColumn::new("id", 2).default_value("autoincrement"))
        .column(
            SyntheticColumn::new("word", 9)
                .width(20)
                .default_value("'a'"),
        )
        .column(SyntheticColumn::new("letters", 9).width(20).scale(1));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let columns = support::definition(&database, "words").columns();
    let defaults: Vec<Option<&str>> = columns.iter().map(|column| column.default()).collect();
    assert_eq!(defaults, [Some("autoincrement"), Some("'a'"), None]);
    let units: Vec<Option<LengthUnit>> =
        columns.iter().map(|column| column.length_unit()).collect();
    assert_eq!(
        units,
        [None, Some(LengthUnit::Bytes), Some(LengthUnit::Characters)]
    );
}

// Two users may each own a table of one name.
#[test]
fn a_shared_name_needs_its_owner() {
    let mine = support::numbers("stars", &[1, 2, 3]);
    let theirs = support::numbers("Stars", &[7]).owned_by(102);
    let database = support::open(
        &SyntheticDatabase::new()
            .user(102, "other")
            .table(mine)
            .table(theirs)
            .table(support::numbers("dotted.name", &[])),
    );
    let expected = "more than one table is called stars: \
                    owner.stars (table 1000), other.Stars (table 1001)";
    let source: &dyn Source = &database;
    let errors = [
        database.table("stars").unwrap_err(),
        source.definition("stars").unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.to_string(), expected);
    }
    for (name, rows) in [("OTHER.stars", 1), ("owner.stars", 3)] {
        assert_eq!(support::rows(&database, name).len(), rows);
        assert_eq!(source.definition(name).unwrap().row_count(), rows as u64);
    }
    assert!(database.table("nobody.stars").is_err());
    assert!(database.table("owner.").is_err() && database.table(".stars").is_err());
    // A point can be part of a name.
    for name in ["dotted.name", "owner.dotted.name"] {
        assert_eq!(database.table(name).unwrap().name(), "dotted.name");
    }
    let table = database.table("other.stars").unwrap();
    assert_eq!(database.table_by_id(table.id()).unwrap().name(), "Stars");
    assert!(database.table_by_id(999).is_none());
}

// The name that an error offers has to work, and so does the one of a table
// whose own name looks like an owner's and a table's.
#[test]
fn every_split_of_a_name_with_points_is_tried() {
    let database = support::open(
        &SyntheticDatabase::new()
            .user(102, "a.b")
            .user(103, "a")
            .user(104, "a")
            .table(support::numbers("seeds", &[1]).owned_by(102))
            .table(support::numbers("b.seeds", &[2, 3]).owned_by(103))
            .table(support::numbers("twins", &[]).owned_by(103))
            .table(support::numbers("twins", &[]).owned_by(104)),
    );
    let candidates = |name: &str| match database.table(name) {
        Err(Error::AmbiguousTable { candidates, .. }) => candidates,
        other => panic!("{name}: {other:?}"),
    };
    assert_eq!(database.table("seeds").unwrap().id(), 1000);
    assert_eq!(database.table("b.seeds").unwrap().id(), 1001);
    assert_eq!(
        candidates("a.b.seeds"),
        ["a.b.seeds (table 1000)", "a.b.seeds (table 1001)"]
    );
    // Two users of one name own a table of one name: only the identifiers differ.
    for name in ["twins", "a.twins"] {
        assert_eq!(
            candidates(name),
            ["a.twins (table 1002)", "a.twins (table 1003)"]
        );
    }
    assert_eq!(database.table_by_id(1003).unwrap().name(), "twins");
}

#[test]
fn the_type_of_a_user_says_whose_its_tables_are() {
    let file = SyntheticDatabase::new()
        .user(1, "person")
        .engine_user(150, "engine")
        .table(support::numbers("plants", &[]).owned_by(1))
        .table(support::numbers("settings", &[]).owned_by(150))
        .table(support::numbers("nobodys", &[]).owned_by(99))
        .table(support::numbers("strangers", &[]).owned_by(100));
    let is_system = |file: &SyntheticDatabase| {
        let database = support::open(file);
        ["plants", "settings", "nobodys", "strangers"].map(|name| {
            let definition = support::definition(&database, name);
            assert_eq!(
                definition.owner().is_empty(),
                definition.owner_id() == 99 || definition.owner_id() == 100
            );
            definition.is_system()
        })
    };
    assert_eq!(is_system(&file), [false, true, true, false]);
    // Without the types, or for a user who is not listed, the number says it.
    assert_eq!(
        is_system(&file.without_user_types()),
        [true, false, true, false]
    );
}

#[test]
fn a_user_table_that_cannot_be_read_costs_the_names_of_the_owners() {
    let sky = support::sky();
    let users = support::definition(&support::open(&sky), "ISYSUSER").pages()[0];
    let database = support::damaged(&sky, &[users]);
    assert!(matches!(
        database.catalog_problems(),
        [Error::TablePage { table, page, .. }] if table == "ISYSUSER" && *page == users
    ));
    let planets = support::definition(&database, "planets");
    assert!(planets.owner().is_empty() && !planets.is_system());
    assert!(support::definition(&database, "ISYSTAB").is_system());
    assert_eq!(support::rows(&database, "planets").len(), 3);
    assert!(!database.verify().is_sound());
}

// The catalog's own two tables have to be what the reader starts from.
#[test]
fn a_catalog_of_another_layout_is_refused() {
    let file = SyntheticDatabase::new().table(support::numbers("plain", &[1]));
    // The second column of ISYSTAB is of domain 1.
    let mut old = support::column_row(1, 2);
    let mut new = old.clone();
    old.extend([1, 0]);
    new.extend([2, 0]);
    let refused = support::replace(&file, "ISYSTABCOL", &old, &new);
    let refused = Database::from_bytes(refused.build()).unwrap_err();
    assert!(matches!(
        refused,
        Error::Catalog(CatalogProblem::Layout { table: "ISYSTAB" })
    ));
    let refused = support::replace(&file, "ISYSTAB", &support::table_row(2), &[0xff, 0xff]);
    let refused = Database::from_bytes(refused.build()).unwrap_err();
    assert!(matches!(
        refused,
        Error::Catalog(CatalogProblem::MissingTable {
            table: "ISYSTABCOL"
        })
    ));
}

#[test]
fn a_damaged_catalog_page_says_which_it_is() {
    let file = SyntheticDatabase::new().table(support::numbers("plain", &[1]));
    let whole = support::open(&file);
    for name in ["ISYSTAB", "ISYSTABCOL"] {
        let pages = support::definition(&whole, name).pages();
        let mut bytes = file.build();
        bytes[pages[0] as usize * support::PAGE_SIZE + 100] ^= 0xff;
        let refused = Database::from_bytes(bytes).unwrap_err();
        assert!(
            matches!(&refused, Error::TablePage { table, page, .. } if table == name && *page == pages[0]),
            "{refused:?}"
        );
    }
}

// A row that cannot be read could describe a column of any table, and without
// one of its columns a table's rows read as something else.
#[test]
fn a_column_row_that_cannot_be_read_keeps_the_file_shut() {
    let file = SyntheticDatabase::new().table(support::numbers("plain", &[1]));
    let (page, row) = support::find(&file, "ISYSTABCOL", &support::column_row(1000, 1));
    // The flags are before the one byte of the bitmap.
    let refused = file.patch(page, row - 2, &[0x02]);
    let refused = Database::from_bytes(refused.build()).unwrap_err();
    assert!(matches!(
        refused,
        Error::Row {
            problem: RowProblem::Flags { flags: 0x02 },
            ..
        }
    ));
}

#[test]
fn tables_that_share_an_identifier_are_not_listed() {
    let file = SyntheticDatabase::new()
        .table(support::numbers("one", &[1]))
        .table(support::numbers("two", &[2]))
        .table(support::numbers("three", &[3]));
    let twice = support::replace(
        &file,
        "ISYSTAB",
        &support::table_row(1001),
        &support::table_row(1000),
    );
    let database = support::open(&twice);
    assert!(matches!(
        database.catalog_problems(),
        [Error::Catalog(CatalogProblem::RepeatedTable {
            table_id: 1000
        })]
    ));
    assert!(database.table("one").is_err() && database.table("two").is_err());
    assert_eq!(support::rows(&database, "three"), [[Value::Integer(3)]]);
    assert!(!database.verify().is_sound());
    // There is no catalog with two of one of its own tables.
    for id in [1, 2] {
        let (own, other) = (support::table_row(1000), support::table_row(id));
        let twice = support::replace(&file, "ISYSTAB", &own, &other);
        let refused = Database::from_bytes(twice.build()).unwrap_err();
        assert!(
            matches!(
                refused,
                Error::Catalog(CatalogProblem::RepeatedTable { table_id }) if table_id == id
            ),
            "{refused:?}"
        );
    }
}

// Without the kind of one of its columns, nothing says where the others are.
#[test]
fn a_column_that_is_not_described_costs_its_table_the_rows() {
    let file = SyntheticDatabase::new()
        .table(support::numbers("whole", &[1]))
        .table(
            SyntheticTable::new("broken")
                .column(SyntheticColumn::new("id", 2))
                .column(SyntheticColumn::new("other", 2))
                .row(SyntheticRow::new([Cell::integer(2), Cell::integer(4)]))
                .row(SyntheticRow::new([Cell::integer(3), Cell::integer(5)])),
        );
    // After the numbers, the domain, and the one letter of whether it is nullable.
    let mut nulls = support::column_row(1001, 1);
    nulls.extend([2, 0, 1]);
    let mut other = nulls.clone();
    nulls.push(b'N');
    other.push(b'X');
    let mut domain = support::column_row(1001, 1);
    domain.extend([0xff, 0xff]);
    for (new, field) in [(other, "nulls"), (domain, "domain_id")] {
        let database = support::open(&support::replace(&file, "ISYSTABCOL", &nulls, &new));
        let [
            Error::Catalog(CatalogProblem::Column {
                table_id: 1001,
                column_id: 1,
                field: found,
            }),
        ] = database.catalog_problems()
        else {
            panic!("{:?}", database.catalog_problems());
        };
        assert_eq!(*found, field);
        let broken = database.table("broken").unwrap();
        assert!(broken.columns().is_empty());
        assert_eq!(broken.rows().filter(Result::is_err).count(), 2);
        let none = Columns::none(broken.definition());
        assert_eq!(broken.scan(&none).filter(Result::is_err).count(), 2);
        assert_eq!(support::rows(&database, "whole"), [[Value::Integer(1)]]);
    }
}

/// A table whose second column is newer than its first row, with `absent` for
/// what the catalog keeps for such rows.
fn with_absent(column: SyntheticColumn) -> SyntheticTable {
    SyntheticTable::new("versions")
        .column(SyntheticColumn::new("id", 2))
        .column(column)
        .row(SyntheticRow::new([Cell::integer(1)]))
        .row(SyntheticRow::new([Cell::integer(2), Cell::Null]).schema(1))
}

#[test]
fn a_value_of_older_rows_that_cannot_be_read_costs_only_them() {
    let note = || SyntheticColumn::new("note", 10).width(32767).nullable();
    let long = vec![0x80, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    // A long value of its own, one with bytes after it, one that is cut short.
    let unreadable = [
        Cell::Stored(long),
        Cell::Stored(vec![1, b'a', 0, 1]),
        Cell::Stored(vec![9, b'a']),
    ];
    for absent in unreadable {
        let table = with_absent(note().added_in(1, Some(absent)));
        let database = support::open(
            &SyntheticDatabase::new()
                .table(table)
                .table(support::numbers("other", &[1])),
        );
        assert!(matches!(
            database.catalog_problems(),
            [Error::Catalog(CatalogProblem::Column {
                table_id: 1000,
                column_id: 2,
                field: "nonmaterialized_value"
            })]
        ));
        let versions = database.table("versions").unwrap();
        assert_eq!(versions.columns()[1].absent_value(), None);
        let read: Vec<_> = versions.rows().collect();
        assert!(matches!(
            &read[0],
            Err(Error::Row {
                problem: RowProblem::AbsentValue { column_id: 2 },
                ..
            })
        ));
        assert!(read[1].is_ok());
        // Who does not ask for the column reads the older rows, too.
        let columns = Columns::new(versions.definition(), ["id"]).unwrap();
        assert_eq!(versions.scan(&columns).filter(Result::is_ok).count(), 2);
        assert_eq!(support::rows(&database, "other").len(), 1);
    }
    // Zeros after it are padding, as in a row.
    let padded = Cell::Stored(vec![1, b'a', 0, 0]);
    let table = with_absent(note().added_in(1, Some(padded)));
    let database = support::open(&SyntheticDatabase::new().table(table));
    assert!(database.catalog_problems().is_empty());
    assert_eq!(
        support::rows(&database, "versions")[0][1],
        support::text("a")
    );
}

// What the catalog keeps of a column can be too long for its row, and on a page
// of its own.
#[test]
fn a_long_value_of_the_catalog_that_is_damaged_costs_only_its_column() {
    let note = || {
        let note = SyntheticColumn::new("note", 9).width(400).nullable();
        note.default_value("'b'")
    };
    let mut stored = vec![0xff, 1, 44];
    stored.resize(303, b'x');
    let chained = note().added_in_long(1, &stored, LongStorage::Chained);
    let file = SyntheticDatabase::new().table(with_absent(chained));
    let whole = support::open(&file);
    assert_eq!(
        support::rows(&whole, "versions")[0][1],
        support::text(&"x".repeat(300))
    );
    let pieces = support::definition(&whole, "ISYSTABCOL").extension_pages();
    let damaged = support::damaged(&file, pieces);

    // The catalog's own columns are not declared compressed, and a value in one
    // that is compressed all the same is not taken out of it.
    let compressed = note().added_in_long(1, &stored, LongStorage::Compressed);
    let compressed = SyntheticDatabase::new().table(with_absent(compressed));
    for database in [damaged, support::open(&compressed)] {
        assert!(matches!(
            database.catalog_problems(),
            [Error::Catalog(CatalogProblem::Column {
                table_id: 1000,
                column_id: 2,
                field: "nonmaterialized_value"
            })]
        ));
        let versions = database.table("versions").unwrap();
        assert_eq!(versions.columns()[1].default(), Some("'b'"));
        assert_eq!(versions.rows().filter(Result::is_ok).count(), 1);
    }
}
