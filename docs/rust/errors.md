[Docs](../README.md) / [Rust](README.md) / Errors

# Errors

Fallible operations return `sa17::Result<T>` (`std::result::Result<T, sa17::Error>`). `Error` implements `std::error::Error`, `Clone`, `Debug`, and `Display`.

## Matching errors

```rust
use sa17::{Database, Error, OpenOptions};

let db = match Database::open("garden.db") {
    Ok(db) => db,
    Err(Error::AmbiguousKey { keys }) => {
        let options = OpenOptions::new().key(keys[0]);
        Database::open_with("garden.db", &options)?
    }
    Err(error) => return Err(error),
};
```

## `Error` variants

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Error::Read` | `Arc<std::io::Error>` | Underlying OS I/O error reading the file |
| `Error::Format` | `FormatProblem` | File is not a supported SQL Anywhere 17 page store |
| `Error::NoKey` | - | No page-scrambling key matches the file |
| `Error::AmbiguousKey` | `{ keys: Vec<StoreKey> }` | Multiple candidate keys fit equally well; pass one via `OpenOptions::key` |
| `Error::WrongKey` | - | Explicit key in `OpenOptions` does not match the file |
| `Error::UnsupportedEncoding` | `{ label: String }` | Superblock declares an encoding other than `windows-1252` or `UTF-8` |
| `Error::Page` | `{ page: u32, problem: PageProblem }` | Page failed CRC-32 or header/slot validation |
| `Error::TablePage` | `{ table: String, page: u32, problem: PageProblem }` | A table's data or extension page could not be read |
| `Error::Row` | `{ table: String, address: RowAddress, problem: RowProblem }` | A single row could not be decoded |
| `Error::PageList` | `{ table: String, problem: PageListProblem }` | A table's RLE page list could not be decoded |
| `Error::Catalog` | `CatalogProblem` | System catalog could not be loaded |
| `Error::UnknownTable` | `{ name: String }` | No table matches the requested name |
| `Error::AmbiguousTable` | `{ name: String, candidates: Vec<String> }` | Multiple users own a table of that name; qualify as `owner.table` |
| `Error::UnknownColumn` | `{ table: String, column: String }` | Table has no column with that name |
| `Error::UnknownPosition` | `{ table: String, position: usize }` | Column position is out of range |
| `Error::WrongColumns` | `{ table: String, columns_table: String }` | `Columns` projection belongs to another table or open database |
| `Error::WrongDatabase` | `{ table: String }` | `TableDefinition` belongs to another open database |
| `Error::Value` | `{ table: String, id: RowId, column: String, problem: ValueProblem }` | Column value did not match the requested `FromValue` type |

## Detail enums

Each structured problem enum is re-exported at the crate root:

| Enum | Used by | Covers |
| --- | --- | --- |
| `FormatProblem` | `Error::Format` | Empty/short file, bad magic, wrong page size, unsupported version, missing superblock flags |
| `PageProblem` | `Error::Page`, `Error::TablePage` | Out-of-bounds page number, CRC-32 mismatch, wrong page kind, damaged slot directory or record |
| `RowProblem` | `Error::Row` | Unknown flags, truncated record, bad forward pointer or loop, invalid UTF-8, `LongValueProblem` |
| `LongValueProblem` | `RowProblem` | Truncated descriptor, bad extension page/piece, chain loop or shared piece, `CompressionProblem` |
| `CompressionProblem` | `LongValueProblem` | Bad envelope header/block, zlib error, decompressed size exceeding `OpenOptions::longest_value` |
| `PageListProblem` | `Error::PageList` | Truncated RLE list, non-32-aligned bit count, page numbers past end of file |
| `CatalogProblem` | `Error::Catalog` | Missing or damaged `ISYSTAB`, `ISYSTABCOL`, or unexpected catalog schema |
| `ValueProblem` | `Error::Value` | Missing column, unexpected `NULL`, type mismatch, integer out of range |
