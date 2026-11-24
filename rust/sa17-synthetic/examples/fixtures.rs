//! Writes synthetic SA17 database fixtures to a directory for integration and
//! Python tests.
//!
//! ```console
//! cargo run -p sa17-synthetic --example fixtures -- /tmp/sa17-fixtures
//! ```

use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sa17_synthetic::{
    Cell, IndexOrder, LongStorage, Sign, SyntheticColumn, SyntheticDatabase, SyntheticRow,
    SyntheticTable,
};

fn main() -> ExitCode {
    let Some(dir) = env::args_os().nth(1) else {
        eprintln!("usage: fixtures <output directory>");
        return ExitCode::FAILURE;
    };
    let dir = Path::new(&dir);
    if let Err(error) = write_all(dir) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn write_all(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join("garden.db"), garden().build())?;
    fs::write(dir.join("types.db"), all_types().build())?;
    fs::write(dir.join("negative_key.db"), negative_key().build())?;
    fs::write(dir.join("utf8.db"), utf8_database().build())?;
    fs::write(dir.join("damaged.db"), damaged_database())?;
    Ok(())
}

/// Multi-table database with primary keys, foreign keys, indexes, views,
/// nullable columns, schema evolution, forwarded rows, and deleted rows.
fn garden() -> SyntheticDatabase {
    let beds = SyntheticTable::new("beds")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("label", 9).width(40))
        .primary_key(&["id"])
        .row(SyntheticRow::new([Cell::integer(1), Cell::text("North")]))
        .row(SyntheticRow::new([Cell::integer(2), Cell::text("South")]));

    let plants = SyntheticTable::new("plants")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("name", 9).width(40))
        .column(SyntheticColumn::new("bed_id", 2).nullable())
        .column(SyntheticColumn::new("height", 5).nullable())
        .column(
            SyntheticColumn::new("price", 3)
                .width(10)
                .scale(2)
                .nullable(),
        )
        .column(
            SyntheticColumn::new("notes", 10)
                .width(32767)
                .nullable()
                .added_in(1, Some(Cell::text("legacy"))),
        )
        .primary_key(&["id"])
        .foreign_key("fk_plants_bed", &["bed_id"], "beds")
        .index("idx_plants_name", &["name"], IndexOrder::Ascending)
        .row(
            SyntheticRow::new([
                Cell::integer(1),
                Cell::text("Fern"),
                Cell::integer(1),
                Cell::double(0.45),
                Cell::decimal("12.50"),
            ])
            .schema(0),
        )
        .row(
            SyntheticRow::new([
                Cell::integer(2),
                Cell::text("Bamboo"),
                Cell::integer(2),
                Cell::double(3.25),
                Cell::decimal("29.99"),
                Cell::long(
                    b"Tall perennial evergreen grass. ".repeat(40).as_slice(),
                    LongStorage::Compressed,
                ),
            ])
            .schema(1)
            .forwarded(),
        )
        .row(
            SyntheticRow::new([
                Cell::integer(99),
                Cell::text("Removed"),
                Cell::Null,
                Cell::Null,
                Cell::Null,
                Cell::Null,
            ])
            .schema(1)
            .deleted(),
        )
        .row(
            SyntheticRow::new([
                Cell::integer(3),
                Cell::text("Moss"),
                Cell::Null,
                Cell::Null,
                Cell::decimal("-0.50"),
                Cell::Null,
            ])
            .schema(1),
        );

    let summary = SyntheticTable::new("plant_summary")
        .view()
        .column(SyntheticColumn::new("name", 9).width(40));

    let empty = SyntheticTable::new("empty_table")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("title", 9).width(20).nullable());

    SyntheticDatabase::new()
        .owner("gardener")
        .table(beds)
        .table(plants)
        .table(summary)
        .table(empty)
}

/// Database exercising every supported SQL data type and long storage mode.
fn all_types() -> SyntheticDatabase {
    let payload = b"binary-blob-\x00\xff\x10\x20";
    let long_bin: Vec<u8> = (0..1200_u32).map(|index| (index % 251) as u8).collect();
    let long_text = "Long chained narrative. ".repeat(60);

    // 2024-01-01 is 222_917_760 minutes after 1600-02-29.
    let ts_minutes = 222_917_760 + 14 * 60 + 30;
    let time_minutes = 14 * 60 + 30;

    let table = SyntheticTable::new("samples")
        .column(SyntheticColumn::new("c_bit", 24).width(1).nullable())
        .column(SyntheticColumn::new("c_tinyint", 19).width(1).nullable())
        .column(SyntheticColumn::new("c_smallint", 1).width(2).nullable())
        .column(SyntheticColumn::new("c_usmallint", 22).width(2).nullable())
        .column(SyntheticColumn::new("c_int", 2).width(4).nullable())
        .column(SyntheticColumn::new("c_uint", 21).width(4).nullable())
        .column(SyntheticColumn::new("c_bigint", 20).width(8).nullable())
        .column(SyntheticColumn::new("c_ubigint", 23).width(8).nullable())
        .column(SyntheticColumn::new("c_float", 4).width(4).nullable())
        .column(SyntheticColumn::new("c_double", 5).width(8).nullable())
        .column(
            SyntheticColumn::new("c_numeric", 3)
                .width(15)
                .scale(3)
                .nullable(),
        )
        .column(
            SyntheticColumn::new("c_char", 8)
                .width(8)
                .scale(1)
                .nullable(),
        )
        .column(
            SyntheticColumn::new("c_varchar", 9)
                .width(64)
                .scale(1)
                .nullable(),
        )
        .column(
            SyntheticColumn::new("c_longvarchar", 10)
                .width(32767)
                .nullable(),
        )
        .column(SyntheticColumn::new("c_binary", 11).width(16).nullable())
        .column(SyntheticColumn::new("c_varbinary", 28).width(64).nullable())
        .column(
            SyntheticColumn::new("c_longbinary", 12)
                .width(32767)
                .nullable(),
        )
        .column(SyntheticColumn::new("c_date", 6).width(4).nullable())
        .column(SyntheticColumn::new("c_time", 14).width(8).nullable())
        .column(SyntheticColumn::new("c_timestamp", 13).width(8).nullable())
        .row(SyntheticRow::new([
            Cell::bit(true),
            Cell::tiny_int(200),
            Cell::small_int(-32000),
            Cell::Stored(65000_u16.to_le_bytes().to_vec()),
            Cell::integer(-1_000_000),
            Cell::unsigned_int(3_000_000_000),
            Cell::big_int(-9_000_000_000_000),
            Cell::unsigned_big_int(18_000_000_000_000_000_000),
            Cell::float(1.5),
            Cell::double(1_234.567_89),
            Cell::decimal("123456.789"),
            Cell::text("abc"),
            Cell::bytes(b"caf\xe9"),
            Cell::long(long_text.as_bytes(), LongStorage::Chained),
            Cell::bytes(payload),
            Cell::bytes(&[0xde, 0xad, 0xbe, 0xef]),
            Cell::long(&long_bin, LongStorage::StoredBlocks),
            Cell::day("2024-01-01"),
            Cell::timestamp(time_minutes, 45_123_456),
            Cell::timestamp(ts_minutes, 45_654_321),
        ]))
        .row(SyntheticRow::new(vec![Cell::Null; 20]));

    SyntheticDatabase::new().table(table)
}

/// Database scrambled with a negative page key.
fn negative_key() -> SyntheticDatabase {
    let mut items = SyntheticTable::new("items")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("code", 9).width(16));
    for id in 1..=60 {
        items = items.row(SyntheticRow::new([
            Cell::integer(id),
            Cell::text(&format!("ITEM-{id:03}")),
        ]));
    }
    SyntheticDatabase::new()
        .key(0x0001_2345, 0x0098_7654, Sign::Negative)
        .table(items)
}

/// Database declaring UTF-8 text encoding.
fn utf8_database() -> SyntheticDatabase {
    let words = SyntheticTable::new("words")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("word", 9).width(40))
        .row(SyntheticRow::new([Cell::integer(1), Cell::text("日本語")]))
        .row(SyntheticRow::new([Cell::integer(2), Cell::text("résumé")]));
    SyntheticDatabase::new()
        .table(words)
        .patch(0, 0x19c, b"UTF-8\0\0\0\0\0\0\0")
}

/// Database with one corrupted data page checksum for verification tests.
fn damaged_database() -> Vec<u8> {
    let mut table = SyntheticTable::new("events")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("label", 9).width(40));
    for id in 1..=300 {
        table = table.row(SyntheticRow::new([
            Cell::integer(id),
            Cell::text(&format!("event number {id:04}")),
        ]));
    }
    let mut bytes = SyntheticDatabase::new().table(table).build();
    // Corrupt page 11 (the first data page of `events`) so the catalog still
    // opens cleanly while verify() flags the damaged page and table scan.
    bytes[11 * 4096 + 100] ^= 0xff;
    bytes
}
