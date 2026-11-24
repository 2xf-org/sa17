[Docs](../README.md) / [Rust](README.md) / Verification & Pages

# Verification & Pages

Full-database integrity verification and low-level access to 4096-byte pages, slotted records, and page keys.

## `Verification`

Returned by `db.verify()`. Checks every page's CRC-32 checksum, decodes every live row of every stored table, and compares readable counts against the catalog.

```rust
let report = db.verify();
println!(
    "sound={} pages={} damaged={} tables={} rows={}",
    report.is_sound(),
    report.pages(),
    report.damaged_pages(),
    report.tables(),
    report.rows(),
);
for problem in report.problems() {
    eprintln!("  {problem}");
}
```

| Method | Returns | Description |
| --- | --- | --- |
| `report.is_sound()` | `bool` | `true` when no catalog, page, row, or count problems were found. |
| `report.catalog_problems()` | `u64` | Number of catalog problems found. |
| `report.pages()` | `u64` | Total pages checked. |
| `report.unwritten_pages()` | `u64` | All-zero pages that were never written. |
| `report.damaged_pages()` | `u64` | Pages failing CRC-32 verification. |
| `report.tables()` | `u64` | Stored tables scanned. |
| `report.differing_tables()` | `u64` | Tables whose readable row count differed from the catalog count. |
| `report.rows()` | `u64` | Rows successfully decoded. |
| `report.unreadable_rows()` | `u64` | Rows failing to decode on readable pages. |
| `report.unreadable_pages()` | `u64` | Table pages that could not be read. |
| `report.problems()` | `&[VerificationProblem]` | First 100 reported problems (counts continue past 100). |

### `VerificationProblem`

| Variant | Fields / Payload | Meaning |
| --- | --- | --- |
| `Catalog(Error)` | `Error` | A part of the system catalog could not be read. |
| `Page(Error)` | `Error` | A page failed its CRC-32 checksum or could not be checked. |
| `Rows { table, rows, pages, first }` | `String`, `u64`, `u64`, `Error` | Unreadable rows or pages in `table`, with the first failure. |
| `RowCount { table, found, recorded }` | `String`, `u64`, `u64` | Live row count of `table` differed from the catalog count. |

---

## `PageStore`

Low-level decoded page store. Obtain from `db.store()`, or open a file directly without parsing the catalog:

```rust
use sa17::PageStore;

let store = PageStore::open("garden.db")?;
let state = store.check(10)?;
let page = store.page(10)?;
println!("page {} kind={:?} lsn={}", page.number(), page.kind(), page.lsn());
```

### Constructors

| Method | Returns | Description |
| --- | --- | --- |
| `PageStore::open(path)` | `Result<PageStore>` | Read file into memory and recover its key. |
| `PageStore::open_with(path, &options)` | `Result<PageStore>` | Open from disk with explicit `OpenOptions`. |
| `PageStore::from_bytes(bytes)` | `Result<PageStore>` | Open from `Vec<u8>` in memory. |
| `PageStore::from_bytes_with(bytes, &options)` | `Result<PageStore>` | Open from bytes with explicit `OpenOptions`. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `store.size()` | `u64` | Total file size in bytes. |
| `store.page_size()` | `usize` | Page size in bytes (`4096`). |
| `store.page_count()` | `u32` | Total number of pages in the file. |
| `store.superblock()` | `&Superblock` | Header metadata from page 0. |
| `store.key()` | `StoreKey` | Recovered page-scrambling key. |
| `store.identity()` | `Identity` | Content fingerprint of page 0 and all page checksums. |
| `store.text_encoding()` | `TextEncoding` | Character encoding declared in page 0. |
| `store.check(number)` | `Result<PageState>` | Verify CRC-32 of page `number` and return `Written` or `Unwritten`. |
| `store.stored(number)` | `Result<&[u8]>` | Raw 4096 stored (still scrambled) bytes of page `number`. |
| `store.page(number)` | `Result<Page>` | Verify CRC-32 and decode page `number`. |

---

## `Page` & `Record<'page>`

A single decoded 4096-byte page returned by `store.page(number)`.

### `Page`

| Method | Returns | Description |
| --- | --- | --- |
| `page.number()` | `u32` | Zero-based page position in the file. |
| `page.stored_number()` | `u32` | Page number recorded in the header (differs on checkpoint copies or unwritten pages). |
| `page.page_type()` | `u8` | Single-byte page type tag from the header. |
| `page.lsn()` | `u32` | Log sequence number of the page's last write. |
| `page.kind()` | `PageKind` | Structural kind: `Table`, `Extension`, `PageList`, or `Other`. |
| `page.table_id()` | `Option<u32>` | Owning table identifier, for `Table` and `Extension` pages. |
| `page.slot_count()` | `Option<u16>` | Number of slot entries, for `Table` and `Extension` pages. |
| `page.bytes()` | `&[u8]` | All 4096 decoded bytes of the page. |
| `page.records()` | `Result<impl Iterator<Item = Result<Record<'_>>>>` | Records of a row or extension page in slot order. |
| `page.record(slot)` | `Result<Option<Record<'_>>>` | Record in `slot`, or `None` if empty or past the end. |

### `Record<'page>`

| Method | Returns | Description |
| --- | --- | --- |
| `record.address()` | `RowAddress` | Page number and slot index of the record. |
| `record.flags()` | `u8` | Record flag byte (byte 2). |
| `record.bytes()` | `&'page [u8]` | Raw record bytes including the 2-byte length and 1-byte flags. |

---

## `PageKey` & Enums

`PageKey` is derived from `store_key.page_key(page_number)` and decodes one 4096-byte page (`page_key.decode(&mut buf)`).

| Enum | Variants |
| --- | --- |
| `PageState` | `Written`, `Unwritten` |
| `PageKind` | `Table`, `Extension`, `PageList`, `Other` |
