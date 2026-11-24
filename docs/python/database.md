[Docs](../README.md) / [Python](README.md) / Database & Options

# Database & Options

Opening a database file, configuring open options, and reading file-level metadata.

## `Database`

An open SQL Anywhere 17 database file with its page-scrambling key recovered and system catalog loaded. Supports context managers (`with` / `async with`), `len(db)`, `name in db`, and `db[name]`.

```python
import sa17

with sa17.Database.open("garden.db") as db:
    print(db.superblock.version)
    print(db.text_encoding)
    print(len(db))
    plants = db["plants"]
```

### Constructors

| Call | Returns | Description |
| --- | --- | --- |
| `Database(path, options=None)` | `Database` | Open a database file from disk (`path: str`). Same as `Database.open()`. |
| `Database.open(path, options=None)` | `Database` | Open a database file from disk. Loads the file into memory without modifying it. |
| `Database.from_bytes(data, options=None)` | `Database` | Open a database from raw file `bytes` already in memory. |

Raises `Sa17Error` on I/O failure (`io`), unsupported format (`format`), key failure (`no_key`, `ambiguous_key`, `wrong_key`), unsupported encoding (`unsupported_encoding`), or unreadable catalog (`catalog`, `page`).

### Properties

| Property | Type | Description |
| --- | --- | --- |
| `superblock` | `Superblock` | Header fields from page 0 (format version, LSN, database page count). |
| `text_encoding` | `TextEncoding` | Character encoding declared by the file (`Windows1252` or `Utf8`). |
| `identity` | `Identity` | Content fingerprint identifying the file and its current write state. |
| `key` | `StoreKey` | Page-scrambling key recovered for (or supplied to) the file. |
| `catalog_problems` | `list[str]` | Non-fatal problems encountered while reading optional parts of the catalog. |
| `closed` | `bool` | Whether `close()` has already released the native handle. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `catalog()` | `list[TableInfo]` | Catalog metadata for every table, view, and temporary table. |
| `tables()` | `list[Table]` | Live `Table` handles for every entry in the catalog. |
| `table(name)` | `Table` | Look up a table by name (ASCII case-insensitive; accepts `owner.table`). Raises `Sa17Error` (`unknown_table` or `ambiguous_table`). |
| `table_by_id(id)` | `Table \| None` | Look up a table by its numeric catalog identifier. |
| `verify()` | `Verification` | Check every page checksum, decode every row, and compare counts against the catalog. See [Verification & Pages](pages.md). |
| `store()` | `PageStore` | Return a handle to the underlying 4096-byte page store. See [Verification & Pages](pages.md). |
| `close()` | `None` | Release the native resources held by this handle (called automatically on exiting `with`). |

### Protocols

| Operation | Behavior |
| --- | --- |
| `len(db)` | Number of entries in the system catalog. |
| `name in db` | `True` if a unique table matches `name` (ASCII case-insensitive, or `owner.table`). |
| `db[name]` | Same lookup as `table(name)`, but raises `KeyError` when no table matches. |
| `with db:` | Closes the native handle when leaving the block. |

---

## `OpenOptions`

Optional settings passed to `Database.open()`, `Database.from_bytes()`, `PageStore.open()`, or `PageStore.from_bytes()`.

```python
options = sa17.OpenOptions(
    key=sa17.StoreKey(mask=0x12345, addend=0x987654, sign=sa17.KeySign.Negative),
    longest_value=64 * 1024 * 1024,
)
with sa17.Database.open("garden.db", options=options) as db:
    ...
```

| Field | Type | Default | Description |
| --- | --- | --- | --- |
| `key` | `StoreKey \| None` | `None` | Explicit scrambling key to use instead of automatic key search. |
| `longest_value` | `int \| None` | `None` (16 MiB) | Maximum decompressed byte length allowed for a single compressed value. |

---

## `StoreKey`

Constants that derive the scrambling key for every page in a file.

| Field | Type | Description |
| --- | --- | --- |
| `mask` | `int` | 25-bit XOR mask applied to the page number. |
| `addend` | `int` | 25-bit addend applied after the mask. |
| `sign` | `KeySign` | Sign selecting the sector walk direction (`Positive` or `Negative`). |

---

## `Superblock`

Header metadata decoded from page 0.

| Field | Type | Description |
| --- | --- | --- |
| `version` | `str` | Format version string (e.g. `"3.201.12"`). |
| `lsn` | `int` | Log sequence number recorded in page 0. |
| `database_page_count` | `int` | Number of pages belonging to the database before trailing checkpoint copies. |

---

## `Identity`

Content fingerprint distinguishing a database file and its write state.

| Field | Type | Description |
| --- | --- | --- |
| `page_count` | `int` | Total number of pages in the file. |
| `lsn` | `int` | Log sequence number from page 0. |
| `digest` | `str` | Formatted identity string including the 128-bit digest of page 0 and all page checksums. |

---

## Enums

### `KeySign`

| Member | Wire value | Meaning |
| --- | --- | --- |
| `KeySign.Positive` | `"positive"` | Positive page keys |
| `KeySign.Negative` | `"negative"` | Negative page keys |

### `TextEncoding`

| Member | Wire value | Meaning |
| --- | --- | --- |
| `TextEncoding.Windows1252` | `"windows-1252"` | Windows-1252 single-byte encoding |
| `TextEncoding.Utf8` | `"utf-8"` | UTF-8 encoding |
