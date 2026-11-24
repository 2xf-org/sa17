[Docs](../README.md) / [Rust](README.md) / Database & Options

# Database & Options

Opening database files, configuring options, reading file metadata, and programming against the `Source` trait.

## `Database`

An open database file with its page-scrambling key recovered and system catalog loaded. `Database` is `Send + Sync`, so it can be shared across threads behind `&Database` or `Arc<Database>`.

```rust
use sa17::{Database, OpenOptions};

let db = Database::open("garden.db")?;
println!("version:  {}", db.store().superblock().version());
println!("encoding: {:?}", db.store().text_encoding());
println!("tables:   {}", db.catalog().len());
```

### Constructors

| Method | Returns | Description |
| --- | --- | --- |
| `Database::open(path)` | `Result<Database>` | Read a file from disk into memory, recover its key, and load the catalog. |
| `Database::open_with(path, &options)` | `Result<Database>` | Open a file from disk with explicit `OpenOptions`. |
| `Database::from_bytes(bytes)` | `Result<Database>` | Open a database from raw file `Vec<u8>` already in memory. |
| `Database::from_bytes_with(bytes, &options)` | `Result<Database>` | Open from bytes with explicit `OpenOptions`. |
| `Database::try_new(store)` | `Result<Database>` | Load the system catalog from an already-open `PageStore`. |

### Catalog & Table Lookup

| Method | Returns | Description |
| --- | --- | --- |
| `db.store()` | `&PageStore` | Access the underlying 4096-byte page store, superblock, key, encoding, and identity. |
| `db.catalog()` | `&[TableDefinition]` | Every table, view, and temporary table in the catalog (row-storing entries first, ordered by id). |
| `db.tables()` | `impl ExactSizeIterator<Item = Table<'_>>` | Live `Table` handles for every catalog entry. |
| `db.table(name)` | `Result<Table<'_>>` | Look up a table by name (ASCII case-insensitive; accepts `owner.table`). |
| `db.table_by_id(id)` | `Option<Table<'_>>` | Look up a table by its numeric catalog identifier. |
| `db.catalog_problems()` | `&[Error]` | Non-fatal issues encountered while loading optional parts of the catalog (first 1000). |
| `db.verify()` | `Verification` | Verify every page checksum, decode every row, and compare counts against the catalog. |

### Owned `'static` Scans (`Arc<Database>`)

| Method | Returns | Description |
| --- | --- | --- |
| `Arc::scan_owned(&db, table, columns)` | `OwnedRows` | Stream live rows while holding an `Arc<Database>` (sendable across threads or FFI). |
| `Arc::row_addresses_owned(&db, table)` | `OwnedRowAddresses` | Stream live `RowAddress` values while holding an `Arc<Database>`. |

```rust
use std::sync::Arc;
use sa17::{Columns, Database};

let db = Arc::new(Database::open("garden.db")?);
let def = db.table("plants")?.definition().clone();
let cols = Columns::new(&def, ["id", "name"])?;

let handle = std::thread::spawn(move || -> sa17::Result<usize> {
    let mut count = 0;
    for row in db.scan_owned(&def, &cols) {
        let _row = row?;
        count += 1;
    }
    Ok(count)
});
```

---

## `OpenOptions`

Builder controlling key recovery and decompression limits.

```rust
use sa17::{Database, KeySign, OpenOptions, StoreKey};

let options = OpenOptions::new()
    .key(StoreKey::new(0x12345, 0x987654, KeySign::Negative))
    .longest_value(64 * 1024 * 1024);
let db = Database::open_with("garden.db", &options)?;
```

| Method | Returns | Description |
| --- | --- | --- |
| `OpenOptions::new()` | `OpenOptions` | Automatic key search and a 16 MiB decompression cap. |
| `options.key(store_key)` | `OpenOptions` | Open with an explicit `StoreKey` instead of searching for one. |
| `options.longest_value(bytes)` | `OpenOptions` | Set maximum allowed decompressed size (in bytes) for a single compressed value. |

---

## `Source`

Trait implemented by `Database` (and `&S`, `Box<S>`, `Arc<S>` for any `S: Source`) so downstream code can accept either a raw file or a wrapped/overlaid database behind a uniform `Send + Sync` interface.

| Method | Returns | Description |
| --- | --- | --- |
| `definitions(&self)` | `&[TableDefinition]` | Definitions of every row-storing table, ordered by identifier. |
| `definition(&self, name)` | `Result<&TableDefinition>` | Look up a row-storing table definition by name (or `owner.table`). |
| `definition_by_id(&self, id)` | `Option<&TableDefinition>` | Look up a row-storing table definition by numeric identifier. |
| `scan(&self, table, columns)` | `Box<dyn Iterator<Item = Result<Row>> + Send + '_>` | Stream live rows of `table` projected to `columns`. |
| `fetch(&self, table, columns, id)` | `Result<Option<Row>>` | Fetch a single row by `RowId`. |
| `text_encoding(&self)` | `TextEncoding` | Character encoding of the database. |
| `identity(&self)` | `Option<Identity>` | File fingerprint, or `None` if the source differs from the underlying file. |

---

## File Metadata Types

### `StoreKey` & `KeySign`

Constants deriving each page's scrambling key. `KeySign` is `Positive` or `Negative`.

| Method | Returns | Description |
| --- | --- | --- |
| `StoreKey::new(mask, addend, sign)` | `StoreKey` | Construct a key (low 25 bits of `mask` and `addend` are kept). |
| `key.mask()` | `u32` | 25-bit XOR mask applied to the page number. |
| `key.addend()` | `u32` | 25-bit addend applied after the mask. |
| `key.sign()` | `KeySign` | Sign selecting sector walk direction. |
| `key.page_key(page)` | `PageKey` | Derive the `PageKey` for zero-based page number `page`. |

### `Superblock` & `FormatVersion`

Header fields parsed from page 0 (`db.store().superblock()`).

| Method | Returns | Description |
| --- | --- | --- |
| `superblock.version()` | `FormatVersion` | Format version (`major()`, `minor()`, `patch()`; formats as `"3.201.12"`). |
| `superblock.lsn()` | `u32` | Log sequence number recorded in page 0. |
| `superblock.database_page_count()` | `u32` | Page count belonging to the database before trailing checkpoint copies. |

### `Identity`

Content fingerprint (`db.store().identity()`), computed lazily on first access.

| Method | Returns | Description |
| --- | --- | --- |
| `identity.page_count()` | `u32` | Total number of pages in the file. |
| `identity.lsn()` | `u32` | Log sequence number from page 0. |
| `identity.digest()` | `u128` | 128-bit FNV-1a digest of page 0 and every page's stored CRC-32. |

### `TextEncoding`

| Variant | Meaning |
| --- | --- |
| `TextEncoding::Windows1252` | Windows-1252 single-byte encoding |
| `TextEncoding::Utf8` | UTF-8 encoding |
