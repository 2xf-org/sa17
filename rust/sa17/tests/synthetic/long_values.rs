//! Values that are too long for their row.

use sa17::{
    Columns, CompressionProblem, Database, Error, LongValueProblem, OpenOptions, PageProblem, Row,
    RowAddress, RowProblem, Value,
};
use sa17_synthetic::{
    Cell, LongStorage, SyntheticColumn, SyntheticDatabase, SyntheticRow, SyntheticTable,
};

use crate::support;

/// The bytes of a value that its row keeps.
const PREFIX: usize = 8;

/// A table of letters, with `bodies` outside their rows.
fn letters(bodies: &[(&[u8], LongStorage)]) -> SyntheticDatabase {
    let mut table = SyntheticTable::new("letters")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("body", 12).width(32767));
    for (id, (body, storage)) in (1..).zip(bodies) {
        let body = Cell::long(body, *storage);
        table = table.row(SyntheticRow::new([Cell::integer(id), body]));
    }
    SyntheticDatabase::new().table(table)
}

/// The bytes of a letter, which no other letter has and zlib makes shorter.
fn body(first: u8, length: usize) -> Vec<u8> {
    (0..length).map(|at| first ^ (at % 97) as u8).collect()
}

/// The file with `bytes` in the descriptor of a letter, `after` its prefix.
fn describe(
    file: &SyntheticDatabase,
    body: &[u8],
    after: usize,
    bytes: &[u8],
) -> SyntheticDatabase {
    let (page, offset) = support::find(file, "letters", &body[..PREFIX]);
    file.clone().patch(page, offset + PREFIX + after, bytes)
}

/// Where the parts of a descriptor are after its prefix.
const STORED_LENGTH: usize = 1;
const FIRST_PIECE: usize = 5;

/// The bytes of a piece's address in a descriptor.
fn piece_at(address: RowAddress) -> Vec<u8> {
    let mut bytes = address.page().to_le_bytes().to_vec();
    bytes.extend([0, 0]);
    bytes.extend(address.slot().to_le_bytes());
    bytes
}

/// The file with `bytes` at the start of the piece that has `contents`, where
/// its length and its flags are.
fn repiece(file: &SyntheticDatabase, contents: &[u8], bytes: &[u8]) -> SyntheticDatabase {
    let (page, offset) = piece(file, contents);
    file.clone().patch(page, offset, bytes)
}

/// Where the record of the piece with `contents` starts.
fn piece(file: &SyntheticDatabase, contents: &[u8]) -> (u32, usize) {
    let database = support::open(file);
    for &number in support::definition(&database, "letters").extension_pages() {
        let page = database.store().page(number).unwrap();
        let records = page.records().unwrap().map(Result::unwrap);
        for record in records {
            let holds = |window: &[u8]| window == contents;
            if record.bytes().windows(contents.len()).any(holds) {
                let mut found = page.bytes().windows(record.bytes().len());
                return (
                    number,
                    found.position(|bytes| bytes == record.bytes()).unwrap(),
                );
            }
        }
    }
    panic!("there is no such piece");
}

fn rows(file: &SyntheticDatabase) -> Vec<Result<Row, RowProblem>> {
    let database = support::open(file);
    let rows = database.table("letters").unwrap().rows();
    rows.map(|row| {
        row.map_err(|error| match error {
            Error::Row { table, problem, .. } if table == "letters" => problem,
            error => panic!("{error}"),
        })
    })
    .collect()
}

/// Why the first letter cannot be read.
fn problem(file: &SyntheticDatabase) -> LongValueProblem {
    match rows(file).remove(0) {
        Err(RowProblem::LongValue {
            column_id: 2,
            problem,
            ..
        }) => problem,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_long_value_is_put_back_together_however_it_is_kept() {
    let storages = [
        LongStorage::OnePiece,
        LongStorage::Chained,
        LongStorage::Pieces(5),
        LongStorage::Compressed,
        LongStorage::ChainedCompressed,
        LongStorage::StoredBlocks,
    ];
    let bodies: Vec<Vec<u8>> = (1..7).map(|first| body(first * 16, 3000)).collect();
    let kept: Vec<(&[u8], LongStorage)> = bodies.iter().map(Vec::as_slice).zip(storages).collect();
    let file = letters(&kept);
    for (row, body) in rows(&file).into_iter().zip(bodies) {
        assert_eq!(row.unwrap().value(1), Some(&Value::Bytes(body)));
    }
    assert!(support::open(&file).verify().is_sound());
}

#[test]
fn a_long_text_is_decoded() {
    let story = "A value too long for its row. ".repeat(20);
    let table = SyntheticTable::new("documents")
        .column(SyntheticColumn::new("story", 10).width(32767).nullable())
        .column(SyntheticColumn::new("report", 9).width(32767).nullable())
        .row(SyntheticRow::new([
            Cell::long(story.as_bytes(), LongStorage::OnePiece),
            Cell::long(&b"caf\xe9 ".repeat(200), LongStorage::Compressed),
        ]))
        .row(SyntheticRow::new([Cell::Null, Cell::Null]));
    let database = support::open(&SyntheticDatabase::new().table(table));
    let read = support::rows(&database, "documents");
    let report = "caf\u{e9} ".repeat(200);
    assert_eq!(read[0], [support::text(&story), support::text(&report)]);
    assert_eq!(read[1], [Value::Null, Value::Null]);
}

// Three pieces of a hundred bytes each.
#[test]
fn a_value_ends_at_its_stored_length() {
    let body = body(1, 300);
    let file = letters(&[(&body, LongStorage::Pieces(3))]);
    let with_length = |length: u32| describe(&file, &body, STORED_LENGTH, &length.to_le_bytes());
    let read = |length| rows(&with_length(length)).remove(0);
    for length in [8, 99, 100, 150, 200, 299] {
        let row = read(length).unwrap();
        let expected = Value::Bytes(body[..length as usize].to_vec());
        assert_eq!(row.value(1), Some(&expected), "{length}");
    }
    // The third piece is not looked at when the value ends before it.
    let damaged = repiece(&with_length(200), &body[200..], &[0xff, 0xff]);
    assert!(rows(&damaged).remove(0).is_ok());
    assert_eq!(
        problem(&with_length(301)),
        LongValueProblem::Truncated {
            found: 300,
            expected: 301
        }
    );
}

// A value of no bytes has no piece to look for.
#[test]
fn an_empty_value_is_read_without_its_pieces() {
    let mut descriptor = vec![0x80, 0x80];
    descriptor.extend(0_u32.to_le_bytes());
    descriptor.extend(piece_at(RowAddress::new(u32::MAX, 9)));
    let table = SyntheticTable::new("letters")
        .column(SyntheticColumn::new("id", 2))
        .column(SyntheticColumn::new("body", 12).width(32767))
        .row(SyntheticRow::new([
            Cell::integer(1),
            Cell::Stored(descriptor),
        ]));
    let row = rows(&SyntheticDatabase::new().table(table)).remove(0);
    assert_eq!(row.unwrap().value(1), Some(&Value::Bytes(Vec::new())));
}

#[test]
fn a_chain_that_is_not_one_is_refused() {
    let body = body(1, 300);
    let file = letters(&[(&body, LongStorage::Pieces(3))]);
    let (page, second) = piece(&file, &body[100..200]);
    let patched = |offset, bytes: &[u8]| file.clone().patch(page, second + offset, bytes);
    // The flags are at 2, the next page at 3 and the next slot at 7.
    assert_eq!(
        problem(&patched(2, &[0x24])),
        LongValueProblem::Flags { flags: 0x24 }
    );
    assert_eq!(
        problem(&patched(2, &[0x08])),
        LongValueProblem::Flags { flags: 0x08 }
    );
    // A last piece is of flags 0, and what it has is all there is.
    assert!(matches!(
        problem(&patched(2, &[0])),
        LongValueProblem::Truncated { .. }
    ));
    // The header of a piece that another follows is nine bytes long.
    assert_eq!(problem(&patched(0, &[9, 0])), LongValueProblem::EmptyPiece);
    assert_eq!(problem(&patched(0, &[7, 0])), LongValueProblem::EmptyPiece);
    assert_eq!(
        problem(&patched(0, &[0xff, 0xff])),
        LongValueProblem::MissingPiece
    );
    assert_eq!(
        problem(&patched(7, &[200, 0])),
        LongValueProblem::MissingPiece
    );
    assert_eq!(
        problem(&patched(3, &[10, 0, 0, 0])),
        LongValueProblem::Page(PageProblem::Unlisted)
    );
    let extension_pages = support::open(&file);
    let pieces = support::definition(&extension_pages, "letters").extension_pages();
    let database = support::damaged(&file, pieces);
    let refused = database.table("letters").unwrap().rows().next().unwrap();
    let text = refused.unwrap_err().to_string();
    assert!(
        text.starts_with("table letters, page ")
            && text.contains("the long value of column 2, at page ")
            && text.ends_with("its page: it fails its checksum"),
        "{text}"
    );
}

// Two values on one damaged page are two rows that cannot be read, and each
// error says which.
#[test]
fn a_damaged_page_of_pieces_is_an_error_of_each_row() {
    let (one, two) = (body(1, 300), body(65, 300));
    let file = letters(&[(&one, LongStorage::OnePiece), (&two, LongStorage::OnePiece)]);
    let whole = support::open(&file);
    let pieces = support::definition(&whole, "letters").extension_pages();
    let database = support::damaged(&file, pieces);
    let letters = database.table("letters").unwrap();
    let addresses: Vec<RowAddress> = letters.row_addresses().map(Result::unwrap).collect();
    let refused = letters.rows().map(|row| match row {
        Err(Error::Row {
            table,
            address,
            problem:
                RowProblem::LongValue {
                    problem: LongValueProblem::Page(PageProblem::Checksum { .. }),
                    ..
                },
        }) => {
            assert_eq!(table, "letters");
            address
        }
        other => panic!("{other:?}"),
    });
    assert_eq!(refused.collect::<Vec<_>>(), addresses);
}

// The second piece names itself, and the first, as the next one.
#[test]
fn a_chain_that_loops_is_refused() {
    let body = body(1, 300);
    let file = letters(&[(&body, LongStorage::Pieces(3))]);
    let (page, second) = piece(&file, &body[100..200]);
    // The last piece was written first, into slot 0.
    for slot in [1, 2] {
        let looping = file.clone().patch(page, second + 7, &[slot, 0]);
        let refused = rows(&looping).remove(0);
        assert!(
            matches!(
                &refused,
                Err(RowProblem::LongValue {
                    piece,
                    problem: LongValueProblem::RepeatedPiece,
                    ..
                }) if *piece == RowAddress::new(page, slot.into())
            ),
            "{refused:?}"
        );
    }
}

// Every piece belongs to one value, so a scan takes no longer than the file is.
#[test]
fn two_values_of_a_scan_have_no_piece_in_common() {
    let (one, two) = (body(1, 300), body(65, 300));
    let file = letters(&[(&one, LongStorage::OnePiece), (&two, LongStorage::OnePiece)]);
    let (page, _) = piece(&file, &one);
    let shared = describe(
        &file,
        &two,
        FIRST_PIECE,
        &piece_at(RowAddress::new(page, 0)),
    );
    let read = rows(&shared);
    assert!(read[0].is_ok());
    assert!(matches!(
        read[1],
        Err(RowProblem::LongValue {
            problem: LongValueProblem::RepeatedPiece,
            ..
        })
    ));
    // Another scan, or who asks for the one row, reads it as if it were first.
    let database = support::open(&shared);
    let letters = database.table("letters").unwrap();
    let columns = Columns::new(letters.definition(), ["id"]).unwrap();
    assert_eq!(letters.scan(&columns).filter(Result::is_ok).count(), 2);
    let every = Columns::all(letters.definition());
    let second = letters.row_addresses().nth(1).unwrap().unwrap();
    let alone = letters.row(second, &every).unwrap_err();
    assert!(alone.to_string().ends_with("the prefix stored in the row"));
}

#[test]
fn a_value_starts_with_the_prefix_in_its_row() {
    let body = body(1, 300);
    for storage in [LongStorage::OnePiece, LongStorage::Compressed] {
        let file = letters(&[(&body, storage)]);
        let (page, offset) = support::find(&file, "letters", &body[..PREFIX]);
        let other = file.patch(page, offset + 7, &[0xff]);
        assert_eq!(problem(&other), LongValueProblem::Prefix);
    }
}

#[test]
fn a_compressed_value_that_is_damaged_says_how() {
    let body = body(1, 3000);
    let file = letters(&[(&body, LongStorage::Compressed)]);
    let (page, offset) = piece(&file, &[0, 4, 2, 0, 1, 10]);
    // The envelope starts after the record's header, and its block is ten
    // bytes of header and then a zlib stream.
    let envelope = offset + 3;
    let damaged = |at, bytes: &[u8]| {
        let file = file.clone().patch(page, envelope + at, bytes);
        match problem(&file) {
            LongValueProblem::Compression(problem) => problem,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(damaged(0, &[1]), CompressionProblem::Header);
    assert_eq!(damaged(4, &[3]), CompressionProblem::Block);
    assert_eq!(damaged(6, &[0xff, 0xff]), CompressionProblem::Block);
    assert_eq!(damaged(10, &[9]), CompressionProblem::Block);
    assert_eq!(damaged(14, &[0xff, 0xff]), CompressionProblem::Inflate);
    // The length once decompressed is the last thing in the descriptor.
    let longer = describe(&file, &body, 13, &3001_u32.to_le_bytes());
    assert_eq!(
        problem(&longer),
        LongValueProblem::Compression(CompressionProblem::Length)
    );
}

// A compressed value can be a thousand times as long as what the file has of it.
#[test]
fn a_value_is_taken_out_of_compression_up_to_a_length() {
    let body = vec![b'x'; 100_000];
    let file = letters(&[(&body, LongStorage::Compressed)]);
    let read = |options: &OpenOptions| {
        let database = Database::from_bytes_with(file.build(), options).unwrap();
        let row = database.table("letters").unwrap().rows().next().unwrap();
        row.map(|row| row.read::<Vec<u8>>("body").unwrap().len())
    };
    let options = OpenOptions::new();
    assert_eq!(read(&options).unwrap(), 100_000);
    assert_eq!(
        read(&options.clone().longest_value(100_000)).unwrap(),
        100_000
    );
    let refused = read(&options.longest_value(99_999)).unwrap_err();
    assert!(
        matches!(
            refused,
            Error::Row {
                problem: RowProblem::LongValue {
                    problem: LongValueProblem::TooLong { length: 100_000 },
                    ..
                },
                ..
            }
        ),
        "{refused:?}"
    );
    // What a row says of a value is enough to refuse it.
    let huge = describe(&file, &body, 13, &((16 << 20) + 1_u32).to_le_bytes());
    assert_eq!(
        problem(&huge),
        LongValueProblem::TooLong {
            length: (16 << 20) + 1
        }
    );
    let held = describe(&file, &body, 13, &(16_u32 << 20).to_le_bytes());
    assert_eq!(
        problem(&held),
        LongValueProblem::Compression(CompressionProblem::Length)
    );
}
