//! A table to write: its columns and its rows.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::cell::{Cell, LongStorage};
use crate::pages::{Address, Pages, Writer};

/// The catalog's codes for what a table is.
const BASE_TABLE: u8 = 1;
const MATERIALIZED_VIEW: u8 = 2;
const TEMPORARY_TABLE: u8 = 3;
const TEXT_INDEX: u8 = 5;
const VIEW: u8 = 21;

/// The user that owns a table unless the test says otherwise.
pub(crate) const OWNER: u32 = 101;

/// Bytes of a long value that its row keeps.
const PREFIX: usize = 8;

/// The bits of a long value's tag.
const LINKED: u8 = 0x80;
const COMPRESSED: u8 = 0x40;

/// One column of a synthetic table.
#[derive(Clone, Debug)]
pub struct SyntheticColumn {
    pub(crate) name: String,
    pub(crate) domain: u16,
    pub(crate) width: u32,
    pub(crate) scale: u16,
    pub(crate) nullable: bool,
    pub(crate) default: Option<String>,
    pub(crate) first_schema: u32,
    pub(crate) absent: Option<Cell>,
}

impl SyntheticColumn {
    /// A column that is not nullable, with the catalog's domain identifier
    /// for its type: 2 is `INTEGER`, 9 is `VARCHAR`, and so on.
    #[must_use]
    pub fn new(name: &str, domain: u16) -> Self {
        Self {
            name: name.to_owned(),
            domain,
            width: 4,
            scale: 0,
            nullable: false,
            default: None,
            first_schema: 0,
            absent: None,
        }
    }

    /// Sets the declared width, or the precision of a `NUMERIC`.
    #[must_use]
    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Sets the digits after the decimal point of a `NUMERIC`.
    #[must_use]
    pub fn scale(mut self, scale: u16) -> Self {
        self.scale = scale;
        self
    }

    /// Lets the column hold `NULL`.
    #[must_use]
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Declares the column's `DEFAULT`, as SQL writes the expression.
    #[must_use]
    pub fn default_value(mut self, expression: &str) -> Self {
        self.default = Some(expression.to_owned());
        self
    }

    /// Makes the column one that a later schema version added. `absent` is
    /// the field that older rows have for it, as a row would store it.
    ///
    /// # Panics
    ///
    /// Panics when `absent` is `NULL` or a long value: the catalog keeps only
    /// a stored field.
    #[must_use]
    pub fn added_in(mut self, schema: u32, absent: Option<Cell>) -> Self {
        self.first_schema = schema;
        self.absent = absent.map(|cell| match cell {
            Cell::Stored(stored) => Cell::bytes(&stored),
            _ => panic!("the value of an added column is a stored field"),
        });
        self
    }

    /// The same, where the catalog keeps the field outside its own row.
    #[must_use]
    pub fn added_in_long(mut self, schema: u32, absent: &[u8], storage: LongStorage) -> Self {
        self.first_schema = schema;
        self.absent = Some(Cell::long(absent, storage));
        self
    }
}

/// How a row is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Live,
    Deleted,
    Replaced,
    Forwarded,
    /// No row at all: the slot is empty.
    Empty,
}

/// One row of a synthetic table.
#[derive(Clone, Debug)]
pub struct SyntheticRow {
    cells: Vec<Cell>,
    schema: Option<u8>,
    flags: u8,
    state: State,
    moved_flags: u8,
    slack: usize,
}

impl SyntheticRow {
    /// A live row with one cell per column of its schema version.
    #[must_use]
    pub fn new(cells: impl IntoIterator<Item = Cell>) -> Self {
        Self {
            cells: cells.into_iter().collect(),
            schema: None,
            flags: 0,
            state: State::Live,
            moved_flags: 0,
            slack: 0,
        }
    }

    /// No row: a slot that a page has and nothing is in.
    #[must_use]
    pub fn empty_slot() -> Self {
        let mut row = Self::new([]);
        row.state = State::Empty;
        row
    }

    /// Stamps the row with the schema version it was written at. A row
    /// without a stamp is at version zero.
    #[must_use]
    pub fn schema(mut self, version: u8) -> Self {
        self.schema = Some(version);
        self
    }

    /// Adds `flags` to the ones the row has for its state, as for a flag that
    /// a reader does not know. On a forwarded row they are on the forward.
    #[must_use]
    pub fn flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }

    /// Adds `flags` to the record that a forward leads to.
    #[must_use]
    pub fn moved_flags(mut self, flags: u8) -> Self {
        self.moved_flags = flags;
        self
    }

    /// Leaves so many bytes after the record in its slot. They are no part of
    /// the row.
    #[must_use]
    pub fn slack(mut self, bytes: usize) -> Self {
        self.slack = bytes;
        self
    }

    /// Marks the row deleted. A reader must skip it.
    #[must_use]
    pub fn deleted(mut self) -> Self {
        self.state = State::Deleted;
        self
    }

    /// Marks the row replaced by a later version. A reader must skip it.
    #[must_use]
    pub fn replaced(mut self) -> Self {
        self.state = State::Replaced;
        self
    }

    /// Moves the row to an extension page and leaves a forward in its place.
    #[must_use]
    pub fn forwarded(mut self) -> Self {
        self.state = State::Forwarded;
        self
    }
}

/// A table to write into a synthetic file.
#[derive(Clone, Debug)]
pub struct SyntheticTable {
    pub(crate) name: String,
    pub(crate) kind: u8,
    pub(crate) owner: u32,
    pub(crate) columns: Vec<SyntheticColumn>,
    pub(crate) primary_key: Vec<String>,
    pub(crate) foreign_keys: Vec<ForeignKey>,
    pub(crate) indexes: Vec<Index>,
    pub(crate) long_page_list: Option<LongStorage>,
    spare_extension_pages: usize,
    rows: Vec<SyntheticRow>,
    stale_rows: Vec<SyntheticRow>,
}

/// A foreign key to declare: its name, its columns, and the table whose
/// primary key they hold.
#[derive(Clone, Debug)]
pub(crate) struct ForeignKey {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) target: String,
}

/// An index to declare. A descending column is kept largest first.
#[derive(Clone, Debug)]
pub(crate) struct Index {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) order: IndexOrder,
}

/// The order that an index keeps its columns in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexOrder {
    /// Smallest first.
    Ascending,
    /// Largest first.
    Descending,
}

/// What writing a table produced.
pub(crate) struct Written {
    pub(crate) pages: Vec<u32>,
    pub(crate) extension_pages: Vec<u32>,
    pub(crate) live_rows: u64,
}

impl SyntheticTable {
    /// An ordinary table without columns or rows, owned by the file's owner.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: BASE_TABLE,
            owner: OWNER,
            columns: Vec::new(),
            primary_key: Vec::new(),
            foreign_keys: Vec::new(),
            indexes: Vec::new(),
            long_page_list: None,
            spare_extension_pages: 0,
            rows: Vec::new(),
            stale_rows: Vec::new(),
        }
    }

    /// Turns the table into a view: the catalog lists it, and it has no rows.
    #[must_use]
    pub fn view(mut self) -> Self {
        self.kind = VIEW;
        self
    }

    /// Turns the table into a materialized view, which has rows.
    #[must_use]
    pub fn materialized_view(mut self) -> Self {
        self.kind = MATERIALIZED_VIEW;
        self
    }

    /// Turns the table into a global temporary table, which has no rows.
    #[must_use]
    pub fn temporary(mut self) -> Self {
        self.kind = TEMPORARY_TABLE;
        self
    }

    /// Turns the table into the one behind a text index.
    #[must_use]
    pub fn text_index(mut self) -> Self {
        self.kind = TEXT_INDEX;
        self
    }

    /// Whether the catalog lists pages for the table.
    pub(crate) fn lists_pages(&self) -> bool {
        !matches!(self.kind, VIEW | TEMPORARY_TABLE)
    }

    /// Keeps the list of the table's pages outside the catalog's row.
    #[must_use]
    pub fn long_page_list(mut self, storage: LongStorage) -> Self {
        self.long_page_list = Some(storage);
        self
    }

    /// Adds so many extension pages that nothing is on, for a test that
    /// [patches](crate::SyntheticDatabase::patch) them.
    #[must_use]
    pub fn spare_extension_pages(mut self, pages: usize) -> Self {
        self.spare_extension_pages = pages;
        self
    }

    /// Sets the identifier of the user that owns the table.
    #[must_use]
    pub fn owned_by(mut self, user: u32) -> Self {
        self.owner = user;
        self
    }

    /// Adds a column after the ones already there.
    #[must_use]
    pub fn column(mut self, column: SyntheticColumn) -> Self {
        self.columns.push(column);
        self
    }

    /// Declares the columns called `columns` the table's primary key.
    #[must_use]
    pub fn primary_key(mut self, columns: &[&str]) -> Self {
        self.primary_key = owned(columns);
        self
    }

    /// Declares that the columns called `columns` hold the primary key of a
    /// row of the table called `target`.
    #[must_use]
    pub fn foreign_key(mut self, name: &str, columns: &[&str], target: &str) -> Self {
        self.foreign_keys.push(ForeignKey {
            name: name.to_owned(),
            columns: owned(columns),
            target: target.to_owned(),
        });
        self
    }

    /// Declares an index on the columns called `columns`. The catalog lists
    /// it; its pages are not written.
    #[must_use]
    pub fn index(mut self, name: &str, columns: &[&str], order: IndexOrder) -> Self {
        self.indexes.push(Index {
            name: name.to_owned(),
            columns: owned(columns),
            order,
        });
        self
    }

    /// Adds a row.
    #[must_use]
    pub fn row(mut self, row: SyntheticRow) -> Self {
        self.rows.push(row);
        self
    }

    /// Adds a row to a table that is being filled in place.
    pub(crate) fn push_row(&mut self, row: SyntheticRow) {
        self.rows.push(row);
    }

    /// Adds a row on a page that carries the table's identifier but is
    /// missing from the table's page list. A reader must not see it.
    #[must_use]
    pub fn stale_row(mut self, row: SyntheticRow) -> Self {
        self.stale_rows.push(row);
        self
    }

    /// The table's current schema version: the newest one that added a column.
    pub(crate) fn schema(&self) -> u32 {
        let versions = self.columns.iter().map(|column| column.first_schema);
        versions.max().unwrap_or(0)
    }

    /// Writes the rows into new pages owned by table `id`.
    pub(crate) fn write(&self, id: u32, pages: &mut Pages) -> Written {
        let mut own = Writer::new(id, crate::pages::TABLE_PAGE);
        let mut forwards = Writer::new(id, crate::pages::FORWARD_PAGE);
        let mut pieces = Writer::new(id, crate::pages::PIECE_PAGE);
        let mut live_rows = 0;
        for row in &self.rows {
            if row.state == State::Empty {
                own.add(pages, &[]);
                continue;
            }
            let mut record = self.record(row, pages, &mut pieces);
            match row.state {
                State::Live => live_rows += 1,
                State::Deleted | State::Replaced | State::Empty => {}
                State::Forwarded => {
                    live_rows += 1;
                    let moved = forwards.add(pages, &record);
                    record = forward(moved, row);
                }
            }
            // The slot is longer than the length that its record says.
            record.resize(record.len() + row.slack, 0xee);
            own.add(pages, &record);
        }
        let mut stale = Writer::new(id, crate::pages::TABLE_PAGE);
        for row in &self.stale_rows {
            let record = self.record(row, pages, &mut pieces);
            stale.add(pages, &record);
        }
        for _ in 0..self.spare_extension_pages {
            pieces.reserve(pages);
        }
        let mut extension_pages = forwards.pages;
        extension_pages.extend(pieces.pages);
        Written {
            pages: own.pages,
            extension_pages,
            live_rows,
        }
    }

    /// One row as a record: length, flags, schema version, the bitmap of the
    /// nullable columns that hold a value, and the fields.
    fn record(&self, row: &SyntheticRow, pages: &mut Pages, pieces: &mut Writer) -> Vec<u8> {
        let schema = u32::from(row.schema.unwrap_or(0));
        let stored: Vec<&SyntheticColumn> = self
            .columns
            .iter()
            .filter(|column| column.first_schema <= schema)
            .collect();
        assert_eq!(
            stored.len(),
            row.cells.len(),
            "table {}: a row has one cell per column of its schema version",
            self.name
        );

        let nullable = stored.iter().filter(|column| column.nullable).count();
        let mut bitmap = vec![0; nullable.div_ceil(8)];
        let mut fields = Vec::new();
        let mut bit = 0;
        for (column, cell) in stored.iter().zip(&row.cells) {
            if column.nullable {
                if *cell != Cell::Null {
                    bitmap[bit / 8] |= 0x80 >> (bit % 8);
                }
                bit += 1;
            } else {
                assert!(
                    *cell != Cell::Null,
                    "column {} is not nullable",
                    column.name
                );
            }
            match cell {
                Cell::Null => {}
                Cell::Stored(bytes) => fields.extend_from_slice(bytes),
                Cell::Long { value, storage } => {
                    fields.extend(long_field(value, *storage, pages, pieces));
                }
            }
        }

        let mut flags = match row.state {
            State::Live => row.flags,
            State::Deleted => row.flags | 0x20,
            State::Replaced => row.flags | 0x01,
            State::Forwarded => row.moved_flags,
            State::Empty => 0,
        };
        let mut record = vec![0, 0, 0];
        if let Some(version) = row.schema {
            flags |= 0x40;
            record.push(version);
        }
        record[2] = flags;
        record.extend(bitmap);
        record.extend(fields);
        // A record is never shorter than a forward, which may have to replace it.
        record.resize(record.len().max(9), 0);
        let length = record.len() as u16;
        record[..2].copy_from_slice(&length.to_le_bytes());
        record
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

/// The nine-byte record left where a row used to be.
fn forward(moved: Address, row: &SyntheticRow) -> Vec<u8> {
    let flags = if row.schema.is_some() { 0x44 } else { 0x04 };
    let mut record = vec![9, 0, flags | row.flags];
    record.extend_from_slice(&moved.page.to_le_bytes());
    record.extend_from_slice(&moved.slot.to_le_bytes());
    record
}

/// Writes the pieces of a long value and returns the descriptor that the row
/// keeps: a prefix of the value, a tag, the stored length and the first piece.
fn long_field(
    value: &[u8],
    storage: LongStorage,
    pages: &mut Pages,
    pieces: &mut Writer,
) -> Vec<u8> {
    let prefix = &value[..value.len().min(PREFIX)];
    let (tag, stored, count) = match storage {
        LongStorage::OnePiece => (0x00, value.to_vec(), 1),
        LongStorage::Chained => (LINKED, value.to_vec(), 2),
        LongStorage::Pieces(count) => (LINKED, value.to_vec(), count),
        LongStorage::Compressed => (COMPRESSED, envelope(&[Block::Zlib(value)]), 1),
        LongStorage::ChainedCompressed => (COMPRESSED | LINKED, envelope(&[Block::Zlib(value)]), 2),
        LongStorage::StoredBlocks => {
            let (head, tail) = value.split_at(value.len() / 2);
            let blocks = [Block::Stored(head), Block::Zlib(tail)];
            (COMPRESSED, envelope(&blocks), 1)
        }
    };
    let first = chain(&stored, count, pages, pieces);
    let mut field = vec![0x80 | prefix.len() as u8];
    field.extend_from_slice(prefix);
    field.push(tag);
    field.extend_from_slice(&(stored.len() as u32).to_le_bytes());
    field.extend_from_slice(&first.page.to_le_bytes());
    field.extend_from_slice(&[0, 0]);
    field.extend_from_slice(&first.slot.to_le_bytes());
    if tag & COMPRESSED != 0 {
        field.extend_from_slice(&(value.len() as u32).to_le_bytes());
    }
    field
}

/// Writes `stored` as `count` linked pieces, the last one first, because each
/// names the one after it.
fn chain(stored: &[u8], count: usize, pages: &mut Pages, pieces: &mut Writer) -> Address {
    let length = stored.len().div_ceil(count).max(1);
    let parts: Vec<&[u8]> = stored.chunks(length).collect();
    let mut next = None;
    for part in parts.into_iter().rev() {
        next = Some(pieces.add(pages, &piece(next, part)));
    }
    next.unwrap_or_else(|| pieces.add(pages, &piece(None, stored)))
}

/// One piece of a long value: a record whose flags say whether another
/// piece follows, and where.
fn piece(next: Option<Address>, bytes: &[u8]) -> Vec<u8> {
    let mut record = vec![0, 0, 0];
    if let Some(next) = next {
        record[2] = 0x04;
        record.extend_from_slice(&next.page.to_le_bytes());
        record.extend_from_slice(&next.slot.to_le_bytes());
    }
    record.extend_from_slice(bytes);
    let length = record.len() as u16;
    record[..2].copy_from_slice(&length.to_le_bytes());
    record
}

/// A block of the engine's compression envelope, by what it decompresses to.
enum Block<'value> {
    Zlib(&'value [u8]),
    Stored(&'value [u8]),
}

/// The engine's compression envelope.
fn envelope(blocks: &[Block<'_>]) -> Vec<u8> {
    const HEADER: [u8; 4] = [0, 4, 2, 0];
    const ZLIB_BLOCK: u16 = 0x0a01;
    const STORED_BLOCK: u16 = 0x0a02;
    let mut envelope = HEADER.to_vec();
    for block in blocks {
        let (kind, bytes) = match block {
            Block::Zlib(value) => (ZLIB_BLOCK, deflated(value)),
            Block::Stored(value) => (STORED_BLOCK, value.to_vec()),
        };
        // Each block records its own position in the envelope.
        let position = envelope.len() as u32;
        envelope.extend_from_slice(&kind.to_le_bytes());
        envelope.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        envelope.extend_from_slice(&position.to_le_bytes());
        envelope.extend(bytes);
    }
    envelope
}

fn deflated(value: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(value).expect("a vector takes every byte");
    encoder.finish().expect("a vector takes every byte")
}
