//! The error type of every operation that reads a file.

use std::io;
use std::sync::Arc;

use crate::cipher::StoreKey;
use crate::format_version::FormatVersion;
use crate::row_address::RowAddress;
use crate::row_id::RowId;

/// A `Result` whose error is [`enum@Error`] unless it says another.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a file, or one part of it, could not be read.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The operating system could not read the file, for example because
    /// there is none.
    #[error(transparent)]
    Read(Arc<io::Error>),

    /// The file is not a database file this reader knows.
    #[error("the file is not a SQL Anywhere 17 database: {0}")]
    Format(FormatProblem),

    /// No key decodes the pages to the page numbers they must start with.
    #[error("no page key fits the pages of this file")]
    NoKey,

    /// More than one key fits the pages equally well. A program that knows
    /// which is the file's opens it with that one, through
    /// [`OpenOptions::key`](crate::OpenOptions::key).
    #[error("more than one page key fits the pages of this file")]
    AmbiguousKey {
        /// The keys that fit. There is none when too many do.
        keys: Vec<StoreKey>,
    },

    /// The key that the file was to be opened with does not fit its pages.
    #[error("the page key does not fit the pages of this file")]
    WrongKey,

    /// The file declares a text encoding that this reader cannot decode.
    #[error("unsupported text encoding {label:?}")]
    UnsupportedEncoding {
        /// The encoding label found in the file's first page.
        label: String,
    },

    /// A page could not be read.
    #[error("page {page}: {problem}")]
    Page {
        /// The page's position in the file.
        page: u32,
        /// What is wrong with it.
        problem: PageProblem,
    },

    /// A page of a table's rows could not be read, and so none of its rows.
    #[error("table {table}, page {page}: {problem}")]
    TablePage {
        /// The table's name.
        table: String,
        /// The page's position in the file.
        page: u32,
        /// What is wrong with it.
        problem: PageProblem,
    },

    /// A row of a table could not be read.
    #[error("table {table}, {address}: {problem}")]
    Row {
        /// The table's name.
        table: String,
        /// Where the row is, or the forward to it.
        address: RowAddress,
        /// What is wrong with it.
        problem: RowProblem,
    },

    /// The list of a table's pages could not be read, and so none of its rows.
    #[error("the page list of table {table}: {problem}")]
    PageList {
        /// The table's name.
        table: String,
        /// What is wrong with the list.
        problem: PageListProblem,
    },

    /// The system catalog does not say what it has to.
    #[error("catalog: {0}")]
    Catalog(CatalogProblem),

    /// The database has no table of this name.
    #[error("there is no table called {name}")]
    UnknownTable {
        /// The name that was asked for.
        name: String,
    },

    /// More than one table can be meant by a name.
    #[error("more than one table is called {name}: {}", candidates.join(", "))]
    AmbiguousTable {
        /// The name that was asked for.
        name: String,
        /// The tables that can be meant, each with its owner's name and its
        /// identifier.
        candidates: Vec<String>,
    },

    /// A table has no column of this name.
    #[error("the table {table} has no column called {column}")]
    UnknownColumn {
        /// The table's name.
        table: String,
        /// The name that was asked for.
        column: String,
    },

    /// A table has no column at this position.
    #[error("the table {table} has no column at position {position}")]
    UnknownPosition {
        /// The table's name.
        table: String,
        /// The position that was asked for.
        position: usize,
    },

    /// The columns to read are not columns of the table, or are of the table
    /// of another open database.
    #[error("the columns are ones of a table {chosen_from}, and not of this table {table}")]
    WrongColumns {
        /// The name of the table to read.
        table: String,
        /// The name of the table that the columns were chosen from.
        chosen_from: String,
    },

    /// The definition of a table to read is one of another open database.
    #[error("the definition of the table {table} is not of this database")]
    WrongDatabase {
        /// The name of the table in the other one.
        table: String,
    },

    /// A value of a row is not of the type that was asked for.
    #[error("table {table}, {id}, column {column}: {problem}")]
    Value {
        /// The table's name.
        table: String,
        /// Which row it is.
        id: RowId,
        /// The column's name, or its position when the row has none there.
        column: String,
        /// How the value differs from what was asked for.
        problem: ValueProblem,
    },
}

impl From<FormatProblem> for Error {
    fn from(problem: FormatProblem) -> Self {
        Self::Format(problem)
    }
}

impl From<CatalogProblem> for Error {
    fn from(problem: CatalogProblem) -> Self {
        Self::Catalog(problem)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Read(Arc::new(error))
    }
}

/// How a value differs from what was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ValueProblem {
    /// The row has no such column.
    #[error("there is no such column")]
    Missing,
    /// The value is `NULL`.
    #[error("expected {expected}, found NULL")]
    Null {
        /// What was asked for.
        expected: &'static str,
    },
    /// The value is of another type.
    #[error("expected {expected}, found {found}")]
    Type {
        /// What was asked for.
        expected: &'static str,
        /// What the value is.
        found: &'static str,
    },
    /// The value is a whole number that does not fit the type.
    #[error("the number does not fit {expected}")]
    Range {
        /// What was asked for.
        expected: &'static str,
    },
}

/// Why a file is not a database file this reader knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FormatProblem {
    /// The file is not a whole number of pages, or is shorter than one.
    #[error("{size} bytes are not a whole number of pages")]
    Size {
        /// The size of the file in bytes.
        size: u64,
    },
    /// The file has pages of a size this reader does not decode.
    #[error("its pages have {found} bytes")]
    PageSize {
        /// The bytes in a page.
        found: usize,
    },
    /// The file has more pages than the four-byte page number in a page header.
    #[error("it has more pages than a page number can address")]
    TooManyPages,
    /// The first page does not carry the number that marks a database file.
    #[error("its first page carries the number {found:#010x}")]
    Magic {
        /// The number in the place of the mark.
        found: u32,
    },
    /// The file is of a format version this reader does not decode.
    #[error("its format version is {found}")]
    Version {
        /// The version the first page names.
        found: FormatVersion,
    },
    /// The first page lacks flags that every file of the format has.
    #[error("its first page has the flags {found:#04x}")]
    Flags {
        /// The flag byte.
        found: u8,
    },
}

/// What is wrong with a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PageProblem {
    /// The file has no such page.
    #[error("the file has {count} pages")]
    OutOfRange {
        /// The number of pages in the file.
        count: u32,
    },
    /// The page's bytes do not match the checksum stored with them.
    #[error("it fails its checksum")]
    Checksum {
        /// The checksum that the page stores.
        stored: u32,
        /// The checksum of the page's bytes.
        computed: u32,
    },
    /// The page is not one of those that the catalog lists for the table. It
    /// can be one that the table gave up.
    #[error("it is not one of the table's pages")]
    Unlisted,
    /// The page carries another page's number. Checkpoint copies past the end
    /// of the database do, and so does a page decoded with the wrong key.
    #[error("it carries the number of page {stored}")]
    WrongNumber {
        /// The number in the page's header.
        stored: u32,
    },
    /// The page is not of the kind that its reference needs.
    #[error("it is not {expected}")]
    WrongKind {
        /// The kind of page the reference needs.
        expected: &'static str,
    },
    /// The page belongs to another table.
    #[error("it belongs to table {table_id}")]
    WrongTable {
        /// The table identifier in the page's header.
        table_id: u32,
    },
    /// The slot directory runs past the row area or names a row outside it.
    #[error("its slot directory is out of bounds")]
    Directory,
    /// A record's stored length is shorter than its header or longer than
    /// its slot.
    #[error("the record in slot {slot} does not fit it")]
    Record {
        /// The slot.
        slot: u16,
    },
}

/// What is wrong with the system catalog.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CatalogProblem {
    /// The catalog describes one of its own tables differently from the
    /// layout this reader starts with.
    #[error("{table} is not laid out as SQL Anywhere 17 lays it out")]
    Layout {
        /// The catalog table's name.
        table: &'static str,
    },
    /// The catalog has no row for one of its own tables.
    #[error("{table} is missing")]
    MissingTable {
        /// The catalog table's name.
        table: &'static str,
    },
    /// More than one table has an identifier, so nothing says whose its pages
    /// and columns are. None of them is listed.
    #[error("more than one table has the identifier {table_id}")]
    RepeatedTable {
        /// The identifier.
        table_id: u32,
    },
    /// What the catalog says about a column cannot be used. The table has no
    /// rows to read if the rows cannot be walked without it.
    #[error("column {column_id} of table {table_id}: {field} holds an unexpected value")]
    Column {
        /// The identifier of the table.
        table_id: u32,
        /// The column's number within the table.
        column_id: u32,
        /// The column of `ISYSTABCOL` that holds the value.
        field: &'static str,
    },
    /// The columns of an index, or those that a foreign key names, cannot be
    /// made out: the table or a column is not there, or the order is unknown.
    #[error("the columns of index {index_id} of table {table_id} cannot be made out")]
    Index {
        /// The identifier of the table that has the index.
        table_id: u32,
        /// The index's number within the table.
        index_id: u32,
    },
    /// A catalog row holds a value of an unexpected type.
    #[error("{table}.{column} holds an unexpected value")]
    Field {
        /// The catalog table's name.
        table: &'static str,
        /// The column's name.
        column: &'static str,
    },
}

/// What is wrong with a serialized page list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PageListProblem {
    /// The list does not start with its size, or the size is not a multiple
    /// of 32 bits.
    #[error("it does not start with its size")]
    Size,
    /// The runs do not add up to the list's size.
    #[error("its runs do not add up to its size")]
    Runs,
    /// The list names a page past the end of the file.
    #[error("it names page {page}, past the end of the file")]
    PastEnd {
        /// The first such page.
        page: u32,
    },
    /// With the lists of the tables before it, the list names more pages than
    /// the file has. A page belongs to one table.
    #[error("it names more pages than are left for the table")]
    TooMany,
    /// The value that holds the list cannot be read from the catalog.
    #[error("it cannot be read from the catalog")]
    Unreadable,
}

/// What is wrong with one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RowProblem {
    /// A page that holds the row, or a part of it, could not be read.
    #[error("page {page}: {problem}")]
    Page {
        /// The page's position in the file.
        page: u32,
        /// What is wrong with it.
        problem: PageProblem,
    },
    /// The stored length is shorter than a row header or longer than the slot.
    #[error("its stored length does not fit its slot")]
    Length,
    /// The row is a forward that does not lead to a row.
    #[error("it forwards to {to}, which holds no row")]
    Forward {
        /// The place that the forward names.
        to: RowAddress,
    },
    /// The row is a forward to where another row of the scan is.
    #[error("it forwards to {to}, as another row does")]
    RepeatedForward {
        /// The place that the forward names.
        to: RowAddress,
    },
    /// The row has flags whose meaning this reader does not know.
    #[error("it has the unknown flags {flags:#04x}")]
    Flags {
        /// The flags without the known ones.
        flags: u8,
    },
    /// The row carries a schema version newer than the table's.
    #[error("it carries schema version {version}, which the table never had")]
    SchemaVersion {
        /// The version byte in the row.
        version: u8,
    },
    /// The table's columns do not make a row: there is none, or two share a
    /// number.
    #[error("the table's columns do not make a row")]
    Columns,
    /// The row is older than a column, and what the catalog has for the column
    /// in such rows cannot be read.
    #[error("nothing says what column {column_id} holds in a row this old")]
    AbsentValue {
        /// The column's number within the table.
        column_id: u32,
    },
    /// A column holds a value of a type whose stored form this reader does
    /// not know, so that nothing says where the value ends.
    #[error(
        "column {column_id} holds a value of domain {domain}, which this reader does not decode"
    )]
    UnsupportedDomain {
        /// The column's number within the table.
        column_id: u32,
        /// The domain identifier of the column's type.
        domain: u16,
    },
    /// A column's declared width is not one its type is stored with.
    #[error("column {column_id} is declared {width} bytes wide")]
    Width {
        /// The column's number within the table.
        column_id: u32,
        /// The declared width.
        width: u32,
    },
    /// The row ends before its fields do.
    #[error("it ends before its fields do")]
    Truncated,
    /// Bytes are left after the last field.
    #[error("{count} bytes are left after its last field")]
    TrailingBytes {
        /// How many bytes are left.
        count: usize,
    },
    /// A long value's descriptor is of a form this reader does not know.
    #[error("column {column_id} has an unknown long-value descriptor")]
    Descriptor {
        /// The column's number within the table.
        column_id: u32,
    },
    /// A value stored outside the row could not be put back together.
    #[error("the long value of column {column_id}, at {piece}: {problem}")]
    LongValue {
        /// The column's number within the table.
        column_id: u32,
        /// The piece that failed, or the first one for a value that is wrong
        /// as a whole.
        piece: RowAddress,
        /// What is wrong with it.
        problem: LongValueProblem,
    },
}

/// What is wrong with a value stored outside its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LongValueProblem {
    /// The page of a piece could not be read.
    #[error("its page: {0}")]
    Page(PageProblem),
    /// The chain names a slot that holds no piece.
    #[error("the slot holds no piece of the value")]
    MissingPiece,
    /// A piece has flags other than "last" and "continues".
    #[error("the piece has unknown flags {flags:#04x}")]
    Flags {
        /// The piece's flag byte.
        flags: u8,
    },
    /// A piece that another one follows has no bytes.
    #[error("the piece continues, and has no bytes")]
    EmptyPiece,
    /// The piece was read before: the chain loops, or another value of the
    /// scan has it.
    #[error("the piece was read before")]
    RepeatedPiece,
    /// The pieces hold fewer bytes than the row says the value has.
    #[error("the pieces hold {found} of {expected} bytes")]
    Truncated {
        /// Bytes the pieces hold.
        found: usize,
        /// Bytes the row's descriptor declares.
        expected: usize,
    },
    /// The value does not start with the bytes that the row keeps of it.
    #[error("the value does not start with the prefix stored in the row")]
    Prefix,
    /// The value is longer than [`OpenOptions`](crate::OpenOptions) allows, or
    /// than there is memory for.
    #[error("{length} bytes are more than are taken into memory for a value")]
    TooLong {
        /// The length of the value.
        length: u32,
    },
    /// The compressed stream is not valid.
    #[error(transparent)]
    Compression(#[from] CompressionProblem),
}

/// What is wrong with a compressed value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompressionProblem {
    /// The stream does not start the way every compressed value does.
    #[error("the compressed value starts with unknown bytes")]
    Header,
    /// A block is cut short, is empty, or is of an unknown kind.
    #[error("a compressed block is cut short or of an unknown kind")]
    Block,
    /// A block's bytes are not one complete zlib stream.
    #[error("a compressed block is not one complete zlib stream")]
    Inflate,
    /// The blocks do not hold as many bytes as the row says the value has.
    #[error("the compressed value has another length than the row declares")]
    Length,
}
