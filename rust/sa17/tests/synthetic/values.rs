//! The values of every type, and reading them as Rust types.

use sa17::chrono::{NaiveDate, NaiveTime};
use sa17::{DataType, Error, RowProblem, TextEncoding, Value};
use sa17_synthetic::{
    Cell, Sign, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable,
};

use crate::support;

#[test]
fn every_supported_type_is_read() {
    let table = SyntheticTable::new("types")
        .column(SyntheticColumn::new("small", 1).width(2))
        .column(SyntheticColumn::new("whole", 2))
        .column(SyntheticColumn::new("big", 20).width(8))
        .column(SyntheticColumn::new("tiny", 19).width(1))
        .column(SyntheticColumn::new("unsigned", 21))
        .column(SyntheticColumn::new("huge", 23).width(8))
        .column(SyntheticColumn::new("flag", 24).width(1))
        .column(SyntheticColumn::new("single", 4))
        .column(SyntheticColumn::new("ratio", 5).width(8))
        .column(SyntheticColumn::new("amount", 3).width(15).scale(2))
        .column(SyntheticColumn::new("price", 27).width(15).scale(5))
        .column(SyntheticColumn::new("born", 6))
        .column(SyntheticColumn::new("seen", 13).width(8))
        .column(SyntheticColumn::new("alarm", 14).width(8))
        .column(SyntheticColumn::new("code", 8).width(3))
        .column(SyntheticColumn::new("name", 9).width(40))
        .column(SyntheticColumn::new("digest", 28).width(16))
        .row(SyntheticRow::new([
            Cell::small_int(-2),
            Cell::integer(-70_000),
            Cell::big_int(-5_000_000_000),
            Cell::tiny_int(200),
            Cell::unsigned_int(4_000_000_000),
            Cell::unsigned_big_int(u64::MAX),
            Cell::bit(true),
            Cell::Stored(1.5_f32.to_le_bytes().to_vec()),
            Cell::double(-0.25),
            Cell::numeric(Sign::Negative, -1, &[99, 99, 99]),
            Cell::numeric(Sign::Positive, -1, &[50, 1]),
            Cell::date(210_296_160),
            Cell::timestamp(210_296_914, 56_789_000),
            Cell::timestamp(1439, 59_000_000),
            Cell::text("USD"),
            Cell::bytes(b"Caf\xe9 cr\xe8me"),
            Cell::bytes(&[0, 1, 0xfe, 0xff]),
        ]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    assert_eq!(database.store().text_encoding(), TextEncoding::Windows1252);

    let read = support::rows(&database, "types");
    let day = NaiveDate::from_ymd_opt(2000, 1, 2).unwrap();
    let expected = [
        Value::Integer(-2),
        Value::Integer(-70_000),
        Value::Integer(-5_000_000_000),
        Value::Integer(200),
        Value::Integer(4_000_000_000),
        Value::Integer(u64::MAX.into()),
        Value::Boolean(true),
        Value::Float(1.5),
        Value::Double(-0.25),
    ];
    assert_eq!(read[0][..9], expected);
    assert_eq!(read[0][9].to_string(), "-9999.99");
    assert_eq!(read[0][10].to_string(), "1.50000");
    assert_eq!(read[0][11], Value::Date(day));
    assert_eq!(
        read[0][12],
        Value::Timestamp(day.and_hms_milli_opt(12, 34, 56, 789).unwrap())
    );
    assert_eq!(
        read[0][13],
        Value::Time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
    );
    assert_eq!(read[0][14], support::text("USD"));
    assert_eq!(read[0][15], support::text("Caf\u{e9} cr\u{e8}me"));
    assert_eq!(read[0][16], Value::Bytes(vec![0, 1, 0xfe, 0xff]));

    let columns = database.table("types").unwrap().columns();
    assert_eq!(columns[9].sql_type(), "numeric(15,2)");
    assert_eq!(columns[15].sql_type(), "varchar(40)");
    assert_eq!(columns[12].data_type(), DataType::Timestamp);
}

// Nothing in a record says where a value of an unknown domain ends.
#[test]
fn a_column_of_an_unknown_domain_reads_only_null() {
    let mut table = SyntheticTable::new("unknown")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("mystery", 99).nullable())
        .column(SyntheticColumn::new("name", 9).width(20).nullable());
    for (id, mystery) in [(1, Cell::Null), (2, Cell::integer(7)), (3, Cell::Null)] {
        let cells = [Cell::integer(id), mystery, Cell::text("named")];
        table = table.row(SyntheticRow::new(cells));
    }
    let default = Cell::Stored(vec![1, 2, 3]);
    let table = table.column(SyntheticColumn::new("later", 98).added_in(1, Some(default)));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let unknown = support::definition(&database, "unknown");
    let column = &unknown.columns()[1];
    assert_eq!(column.data_type(), DataType::Unknown(99));
    assert_eq!(column.sql_type(), "unknown(99)");
    // The catalog keeps the bytes that older rows have for a column.
    let later = unknown.columns()[3].absent_value().unwrap();
    let later = later.as_undecoded().unwrap();
    assert_eq!((later.domain(), later.bytes()), (98, &[1, 2, 3][..]));

    let read: Vec<_> = database.table("unknown").unwrap().rows().collect();
    let values = read[0].as_ref().unwrap().values();
    assert_eq!(
        values[..3],
        [Value::Integer(1), Value::Null, support::text("named")]
    );
    assert!(matches!(
        read[1],
        Err(Error::Row {
            problem: RowProblem::UnsupportedDomain {
                column_id: 2,
                domain: 99
            },
            ..
        })
    ));
    assert!(read[2].is_ok());
}

// Thirty base-100 digits are sixty digits, more than an exact decimal holds.
#[test]
fn a_number_of_too_many_digits_is_kept_as_stored() {
    let digits = [12; 30];
    let table = SyntheticTable::new("numbers")
        .column(SyntheticColumn::new("huge", 3).width(60))
        .row(SyntheticRow::new([Cell::numeric(
            Sign::Positive,
            0,
            &digits,
        )]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let read = support::rows(&database, "numbers");
    let huge = read[0][0].as_undecoded().unwrap();
    assert_eq!((huge.domain(), huge.bytes().len()), (3, 2 + 30));
    assert!(huge.bytes().ends_with(&digits));
}

#[test]
fn nulls_and_short_rows_are_read() {
    let table = SyntheticTable::new("sparse")
        .column(SyntheticColumn::new("level", 19).width(1))
        .column(SyntheticColumn::new("note", 9).width(20).nullable())
        .column(SyntheticColumn::new("count", 2).nullable())
        // Each of these rows is shorter than nine bytes, so it is padded.
        .row(SyntheticRow::new([
            Cell::tiny_int(0),
            Cell::Null,
            Cell::Null,
        ]))
        .row(SyntheticRow::new([
            Cell::tiny_int(7),
            Cell::text(""),
            Cell::Null,
        ]))
        .row(SyntheticRow::new([
            Cell::tiny_int(9),
            Cell::Null,
            Cell::integer(-1),
        ]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let expected = [
        [Value::Integer(0), Value::Null, Value::Null],
        [Value::Integer(7), support::text(""), Value::Null],
        [Value::Integer(9), Value::Null, Value::Integer(-1)],
    ];
    assert_eq!(support::rows(&database, "sparse"), expected);
}

// 2 January 2000 starts 210296160 minutes after the engine's day zero. What is
// past the end of a day or of a minute is no time, and 2 is no bit.
#[test]
fn what_no_value_of_a_type_is_stored_as_is_kept_as_stored() {
    let table = SyntheticTable::new("moments")
        .column(SyntheticColumn::new("day", 6))
        .column(SyntheticColumn::new("moment", 13).width(8))
        .column(SyntheticColumn::new("time", 14).width(8))
        .column(SyntheticColumn::new("flag", 24).width(1))
        .column(SyntheticColumn::new("amount", 3).width(15).scale(2))
        .row(SyntheticRow::new([
            Cell::date(210_296_160 + 725),
            Cell::timestamp(210_296_160, 4_000_000_000),
            Cell::timestamp(10, 3_600_000_000),
            Cell::tiny_int(7),
            Cell::numeric(Sign::Positive, 0, &[1, 100]),
        ]))
        .row(SyntheticRow::new([
            Cell::date(210_296_160 + 1440),
            Cell::timestamp(u32::MAX, 59_999_999),
            Cell::timestamp(1440, 0),
            Cell::tiny_int(1),
            Cell::numeric(Sign::Positive, 0, &[1, 99]),
        ]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let read = support::rows(&database, "moments");
    let domains = |row: &[Value]| -> Vec<Option<u16>> {
        let undecoded = row.iter().map(Value::as_undecoded);
        undecoded
            .map(|value| value.map(|value| value.domain()))
            .collect()
    };
    let none = None;
    assert_eq!(
        domains(&read[0]),
        [Some(6), Some(13), Some(14), Some(24), Some(3)]
    );
    assert_eq!(domains(&read[1]), [none, none, Some(14), none, none]);
    assert_eq!(read[1][0].to_string(), "2000-01-03");
    assert_eq!(read[1][1].to_string(), "9766-04-15 04:15:59.999999");
    assert_eq!(read[0][3].as_undecoded().unwrap().bytes(), [7]);
}
