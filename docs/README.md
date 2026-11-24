# Documentation

`sa17` reads SQL Anywhere 17 database files (`3.201.12`, 4096-byte pages) directly in Rust and Python without a running database server. Files are opened read-only and never modified.

## Languages

| Section | Contents |
| --- | --- |
| [Python](python/README.md) | Quick start and full API reference for `sa17` on PyPI, including Apache Arrow, Polars, and DuckDB |
| [Rust](rust/README.md) | Quick start and full API reference for the `sa17` crate on crates.io |
| [File format](format.md) | On-disk pages, superblock, cipher, key recovery, records, compression, and system catalog |

## Examples

### Python

Run from `python/` with `uv run python examples/<name>.py <args>`:

| Example | Shows |
| --- | --- |
| [`python/examples/tables.py`](../python/examples/tables.py) | List tables, columns, primary keys, and foreign keys |
| [`python/examples/rows.py`](../python/examples/rows.py) | Stream rows with optional column projection |
| [`python/examples/arrow_query.py`](../python/examples/arrow_query.py) | Export zero-copy Arrow tables and query lazily with DuckDB |
| [`python/examples/verify.py`](../python/examples/verify.py) | Verify database integrity and inspect raw pages |

### Rust

Run from the repository root with `cargo run -p sa17 --example <name> -- <args>`:

| Example | Shows |
| --- | --- |
| [`rust/sa17/examples/tables.rs`](../rust/sa17/examples/tables.rs) | List user tables with catalog row counts |
| [`rust/sa17/examples/rows.rs`](../rust/sa17/examples/rows.rs) | Stream and print all rows of a table |
| [`rust/sa17/examples/keys.rs`](../rust/sa17/examples/keys.rs) | Print foreign key relationships between tables |
| [`rust/sa17/examples/verify.rs`](../rust/sa17/examples/verify.rs) | Verify every page checksum, row, and row count |
