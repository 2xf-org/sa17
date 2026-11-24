[Docs](../README.md) / [Python](README.md) / Verification & Pages

# Verification & Pages

Full-database integrity verification and low-level access to 4096-byte pages and slotted records.

## Verifying a database

`db.verify()` checks every page's CRC-32 checksum, decodes every live row of every stored table, and compares readable row counts against the catalog.

```python
import sa17

with sa17.Database.open("garden.db") as db:
    report = db.verify()
    print(f"sound={report.is_sound}, pages={report.pages}, rows={report.rows}")
    for problem in report.problems:
        print(problem.kind.value, problem.table, problem.message)
```

### `Verification`

| Field | Type | Description |
| --- | --- | --- |
| `is_sound` | `bool` | `True` when no catalog, page, row, or count problems were found. |
| `catalog_problems` | `int` | Number of catalog issues encountered. |
| `pages` | `int` | Total pages checked. |
| `unwritten_pages` | `int` | All-zero pages that were never written. |
| `damaged_pages` | `int` | Pages failing CRC-32 verification. |
| `tables` | `int` | Stored tables scanned. |
| `differing_tables` | `int` | Tables whose readable row count differed from the catalog count. |
| `rows` | `int` | Rows successfully decoded. |
| `unreadable_rows` | `int` | Rows that failed to decode on readable pages. |
| `unreadable_pages` | `int` | Table pages that could not be read. |
| `problems` | `list[VerificationProblem]` | First 100 reported problems (counts continue past 100). |

### `VerificationProblem`

| Field | Type | Description |
| --- | --- | --- |
| `kind` | `VerificationProblemKind` | Category: `Catalog`, `Page`, `Rows`, or `RowCount`. |
| `message` | `str` | Human-readable description of the issue. |
| `table` | `str \| None` | Table name, when the issue pertains to a specific table. |

---

## `PageStore`

Low-level store for raw and decoded 4096-byte pages. Obtain one from `db.store()`, or open a file directly without parsing the catalog:

```python
with sa17.PageStore.open("garden.db") as store:
    state = store.check(10)
    page = store.page(10)
    print(page.number, page.kind, page.lsn, len(page.data()))
```

### Constructors

| Call | Returns | Description |
| --- | --- | --- |
| `PageStore(path, options=None)` | `PageStore` | Open a page store from disk without loading the catalog. |
| `PageStore.open(path, options=None)` | `PageStore` | Same as `PageStore(path, options)`. |
| `PageStore.from_bytes(data, options=None)` | `PageStore` | Open a page store from raw `bytes` in memory. |

### Properties

| Property | Type | Description |
| --- | --- | --- |
| `size` | `int` | Total file size in bytes. |
| `page_size` | `int` | Size of each page in bytes (`4096`). |
| `page_count` | `int` | Total number of pages in the file. |
| `superblock` | `Superblock` | Header metadata from page 0. |
| `key` | `StoreKey` | Recovered page-scrambling key. |
| `identity` | `Identity` | File content fingerprint. |
| `text_encoding` | `TextEncoding` | Declared character encoding. |
| `closed` | `bool` | Whether `close()` has released this handle. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `check(page_number)` | `PageState` | Verify the CRC-32 checksum of `page_number` and return `Written` or `Unwritten`. |
| `stored(page_number)` | `bytes` | Return the raw 4096 stored (still scrambled) bytes of `page_number`. |
| `page(page_number)` | `Page` | Check the checksum and decode `page_number`. |
| `close()` | `None` | Release the native handle. |

---

## `Page`

A single decoded 4096-byte page returned by `store.page(page_number)`.

### Properties

| Property | Type | Description |
| --- | --- | --- |
| `number` | `int` | Zero-based page number in the file. |
| `stored_number` | `int` | Page number recorded inside the page header (differs on checkpoint copies or unwritten pages). |
| `page_type` | `int` | Single-byte page type tag from the header. |
| `lsn` | `int` | Log sequence number of the page's last write. |
| `kind` | `PageKind` | Structural kind: `Table`, `Extension`, `PageList`, or `Other`. |
| `table_id` | `int \| None` | Owning table identifier, for `Table` and `Extension` pages. |
| `slot_count` | `int \| None` | Number of slot directory entries, for `Table` and `Extension` pages. |
| `closed` | `bool` | Whether `close()` has released this handle. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `data()` | `bytes` | Return all 4096 decoded bytes of the page. |
| `records()` | `list[RecordInfo]` | Return slot metadata for every record on a row or extension page. |
| `record_bytes(slot)` | `bytes \| None` | Return the raw record bytes in `slot`, or `None` if empty or past the end. |
| `close()` | `None` | Release the native handle. |

---

## `RecordInfo` & Enums

### `RecordInfo`

| Field | Type | Description |
| --- | --- | --- |
| `address` | `RowAddress` | Page number and slot index of the record. |
| `flags` | `int` | Raw record flag byte. |
| `length` | `int` | Byte length of the record. |

### Enums

| Enum | Members |
| --- | --- |
| `PageState` | `Written`, `Unwritten` |
| `PageKind` | `Table`, `Extension`, `PageList`, `Other` |
| `VerificationProblemKind` | `Catalog`, `Page`, `Rows`, `RowCount` |
