# SA17 On-Disk File Format

This document describes the scrambled SAP SQL Anywhere 17 database file format (version `3.201.12`, 4096-byte pages) as implemented by `sa17`. All multi-byte integers are little-endian unless noted otherwise.

## Pages

A database file is a sequence of 4096-byte pages numbered from zero:

- Page 0 is the superblock and is stored unscrambled.
- Every written page ends with the IEEE CRC-32 of its first 4092 stored bytes.
- An all-zero page was never written.

### Superblock (page 0)

| Offset | Size | Field |
| --- | --- | --- |
| `0x06` | 1 | Flags. Bits `0x09` are always set. Bits `0x20` and `0x40` appear in some files with unknown meaning. |
| `0x08` | 4 | Log sequence number (LSN), increasing with writes. |
| `0x10` | 4 | Format major version (`3`). |
| `0x14` | 4 | Magic constant `0xda7aba5e`. |
| `0x18`, `0x1a` | 2, 2 | Format minor (`201`) and patch (`12`) version numbers. |
| `0x1c` | 4 | Number of pages belonging to the database. Trailing pages beyond this count are checkpoint copies retaining the original page's number in their header. |
| `0x19c` | 32 | Null-padded ASCII character encoding name: `windows-1252` or `UTF-8`. |

`sa17` refuses any file with another magic number, another format version, missing `0x09` flag bits, or a page 0 checksum failure. SQL Anywhere supports page sizes from 2048 to 32768 bytes; if page 0 matches a CRC-32 at another of those sizes, `sa17` reports `FormatProblem::PageSize`.

A file's `Identity` combines its total page count, its LSN, and the 128-bit FNV-1a digest of page 0 together with the stored 4-byte checksum of every page.

## Page cipher

Every written page after page 0 is scrambled one 512-byte sector at a time.

### Page key derivation

A file has two 25-bit constants (`mask` and `addend`) and a `sign` (`Positive` or `Negative`). For page `n`:

```text
word = ((n XOR mask) + addend) mod 2^25
key  = 2^25 - 1 - word    (positive keys)
key  = -word              (negative keys)
```

Sector `s` of page `n` (for `s` in `0..8`) uses `key - s`.

### Sector walk

For a non-zero sector key with absolute magnitude `m`:

```text
low       = m mod 256
increment = ((m / 256) mod 256) OR 1
stride    = (((m / 65536) + low) OR 1) mod 512
seed      = low
```

Negative keys walk in reverse: `stride = 512 - stride` and `seed = increment * (tail + 1) - low`, where `tail` is the number of unscrambled bytes at the end of the sector (`0` for sectors `0..7`, `16` for sector `7`).

Decoding starts at `position = 0` with `offset = seed` and repeats once per scrambled byte:

```text
next            = (position + stride) mod 512
plain[position] = stored[next] + offset   (wrapping u8)
offset          = offset + increment      (wrapping u8)
position        = next
```

Because `stride` is odd, the walk visits every scrambled byte exactly once while stepping over the clear tail of sector 7.

### Clear header bytes

The final 16 bytes of a page are kept unscrambled: the 4-byte CRC-32 at `4092..4096` and 12 header bytes at `4080..4092` (page type, LSN, and row count). The 12 row bytes that belong at `4080..4092` are parked inside the scrambled header. After decoding all eight sectors, `sa17` swaps them back:

| Stored bytes | Logical position |
| --- | --- |
| `4080..4082` (clear) | `18..20` |
| `4082..4092` (clear) | `6..16` |
| decoded `18..20` | `4080..4082` |
| decoded `6..16` | `4082..4092` |

### Key recovery

Neither `mask` nor `addend` is stored in the file. Because every decoded page starts with its own 32-bit page number at offset 0, those four known bytes constrain `(stride, increment, seed)` of sector 0, giving up to four candidate 25-bit words per page.

`sa17` samples pages across the file and at the end of the database, then grows `mask` and `addend` one bit at a time from bit 0 upward, pruning combinations that disagree with the samples. If multiple candidates tie on sector 0 (which ignores two high bits read by later sectors), `sa17` decodes written pages and selects the candidate with the most matching page numbers, exact slot-filling records, and zero bytes. Candidates that decode every written page identically are collapsed as equivalent.

## Page header and slot directory

| Offset | Size | Field |
| --- | --- | --- |
| `0` | 4 | Page number |
| `6` | 1 | Page type ASCII tag |
| `8` | 4 | Page LSN |
| `16` | 1 | Structure byte: `4` for a row page, `20` for a page-list page |
| `17` | 1 | Subkind of a row page: `< 16` for table rows, `>= 16` for extension pages |
| `18` | 2 | On a page-list page, end offset of the serialized page list |
| `20` | 2 | Lowest row offset (on page 10, bytes `20..24` must be zero) |
| `22` | 2 | Number of slot directory entries |
| `24` | 4 | Owning table identifier |
| `28` | `2 * slot_count` | Slot directory |

Each non-zero slot entry is a byte offset counted from byte 28. Zero marks an empty slot. Rows grow downward from byte 4092; a slot's available span extends to the start of the next higher-offset slot (or 4092 for the last), and a record may be shorter than its slot.

## Records

| Part | Size | Notes |
| --- | --- | --- |
| Length | 2 | Stored record length (minimum 9 bytes, zero-padded if shorter) |
| Flags | 1 | Record state and layout flags |
| Schema version | 1 | Present only when flag `0x40` is set; defaults to `0` |
| Null bitmap | `ceil(nullable_columns / 8)` | MSB first; `1` means non-null |
| Fields | variable | Non-null column values in catalog order |

Record flags:

| Flag | Meaning |
| --- | --- |
| `0x01`, `0x20` | Dead row (replaced or deleted); skipped by scans |
| `0x04` | Forward pointer (9 bytes: target page at bytes `3..7`, target slot at bytes `7..9`) |
| `0x08` | Observed on live rows; accepted |
| `0x40` | Schema version byte follows the flags byte |

Any other flag bit causes the row to be refused. During a scan, each forward target may be visited at most once, and the target record on an extension page must have none of `0x01`, `0x04`, or `0x20`.

A row stores only the columns whose `start_schema <= row_schema`. Columns added in later schema versions take the `nonmaterialized_value` stored in `ISYSTABCOL`.

### Field encodings

| Domain IDs | SQL types | Wire layout |
| --- | --- | --- |
| `1`, `22` | `SMALLINT`, `UNSIGNED SMALLINT` | 2 bytes LE |
| `2`, `21` | `INTEGER`, `UNSIGNED INT` | 4 bytes LE |
| `20`, `23` | `BIGINT`, `UNSIGNED BIGINT` | 8 bytes LE |
| `19`, `24` | `TINYINT`, `BIT` | 1 byte |
| `4`, `5` | `FLOAT`, `DOUBLE` | 4 or 8 bytes IEEE 754 LE |
| `6` | `DATE` | 4 bytes LE: minutes since 1600-02-29 |
| `13` | `TIMESTAMP` | 4 bytes LE minutes since 1600-02-29, then 4 bytes LE microseconds past the minute |
| `14` | `TIME` | 4 bytes LE minutes since midnight, then 4 bytes LE microseconds past the minute |
| `3`, `27` | `NUMERIC`, `DECIMAL` | 1-byte digit count, 1-byte control (bit 7 = positive sign, bits `0..6` = base-100 exponent + 64), then base-100 digits least significant first |
| `7..12`, `28`, `31` | Character, binary, and bit strings | 1-byte marker followed by inline bytes or long-value descriptor |

Values outside the valid domain (non-day `DATE`, `TIME >= 1440` minutes, `>= 60_000_000` microseconds, `BIT` not in `{0, 1}`, base-100 digit `>= 100`, or decimals exceeding 38 digits) are preserved as `Value::Undecoded`.

For string and binary fields:

- Marker `< 0x80`: inline payload of that exact length.
- Marker `0xff`: 2-byte big-endian length followed by inline payload.
- Marker `0x80..0xff`: long-value descriptor with prefix length `marker & 0x7f`.

## Long values and compression

A long-value descriptor in a row has the layout:

| Part | Size | Notes |
| --- | --- | --- |
| Marker | 1 | `0x80 \| prefix_len` |
| Prefix | `prefix_len` | Leading bytes of the logical value |
| Tag | 1 | `0x00` single piece, `0x80` linked chain, `0x40` compressed single piece, `0xc0` compressed chain |
| Stored length | 4 | Byte length of the stored (or compressed) stream |
| First piece | 8 | 4-byte page number, 2 zero bytes, 2-byte slot index |
| Logical length | 4 | Present only when `tag & 0x40 != 0`; decompressed byte length |

Pieces live on the table's extension pages. A piece with flags `0x00` is the final piece (payload from byte 3); flags `0x04` indicates a linked piece naming the next page (`3..7`) and slot (`7..9`) with payload from byte 9. Each piece may be visited at most once per scan to prevent cycles or shared-chain amplification.

A compressed stream starts with the 4-byte header `00 04 02 00` followed by blocks. Each block has a 10-byte header:

- 2-byte kind: `0x0a01` (zlib stream) or `0x0a02` (raw uncompressed bytes)
- 4-byte payload size
- 4-byte offset of the block within the envelope

Decompression stops with an error if the output exceeds `OpenOptions::longest_value` (16 MiB by default).

## System catalog

- `ISYSTAB` (table 1): table identifier, name, type (`1` base table, `2` materialized view, `3` global temporary table, `5` text-index table, `21` view), owner id, row count, schema version, and `tab_page_list` / `ext_page_list`.
- `ISYSTABCOL` (table 2): table id, column id, name, domain, width, scale (`1` = character width for `CHAR`/`VARCHAR`, `0` = byte width), nullability (`Y`/`N`), `DEFAULT` text, `start_schema`, and `nonmaterialized_value`.
- `ISYSUSER`: user id, name, and user type (bit 0 set for built-in engine users).
- `ISYSIDX`, `ISYSIDXCOL`, `ISYSFKEY`: primary keys, foreign keys, and indexes.

Page 10 stores the run-length encoded page list of `ISYSTAB` starting at byte 24 up to the offset in header bytes `18..20`.

### Serialized page lists

A page list is an RLE-encoded bitset:

1. Leading 32-bit word: total bit count (a multiple of 32, or 0 for empty).
2. Repeated 32-bit control words:
   - bits `7..32`: number of 32-bit fill words
   - bit `6`: fill bit value (`0` or `1`)
   - bits `0..6`: number of literal 32-bit words immediately following
3. Literal 32-bit words with LSB corresponding to the lowest page number.

Pages not present in a table's page list are ignored even if their header still carries that table's identifier.
