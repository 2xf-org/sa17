# SQL Anywhere® 17

Rust and Python reader for SQL Anywhere 17 database files.

[Documentation](docs/README.md) · [Rust](docs/rust/README.md) · [Python](docs/python/README.md) · [File format](docs/format.md)

## How it works

Point `sa17` at a `.db` file. It recovers the page-scrambling key, verifies every page's CRC-32 checksum, reads the catalog, and streams rows as typed Rust values, Python objects, or zero-copy Apache Arrow batches. Files are opened read-only and never modified.

## Installing

Rust:

```toml
[dependencies]
sa17 = "=202611.24.0"
```

Python (3.11+):

```bash
pip install sa17
```

## Example

Rust:

```rust
use sa17::{Columns, Database};

let db = Database::open("garden.db")?;
let plants = db.table("plants")?;
let columns = Columns::new(plants.definition(), ["name", "height"])?;

for row in plants.scan(&columns) {
    let row = row?;
    let name: &str = row.read("name")?;
    let height: Option<f64> = row.read("height")?;
    println!("{name}: {height:?}");
}
```

Python:

```python
import sa17

with sa17.Database.open("garden.db") as db:
    for table in db.tables():
        if not table.is_system and table.stores_rows:
            print(table.name, table.row_count)

    plants = db["plants"]
    for row in plants.scan(["name", "height"]):
        print(row["name"], row["height"])
```

Zero-copy Arrow to Polars and DuckDB:

```python
import duckdb
import polars as pl
import sa17

with sa17.Database.open("garden.db") as db:
    plants = db["plants"]

    frame = pl.from_arrow(plants.to_arrow(["name", "height"]))

    reader = plants.batches(batch_size=8192)
    rows = duckdb.sql("SELECT name, height FROM reader WHERE height > 1.0").fetchall()
```

## Supported

| Feature | Details |
| --- | --- |
| Format | Scrambled SQL Anywhere 17 (`3.201.12`), 4096-byte pages |
| Encodings | `windows-1252` and `UTF-8` |
| Schema | Tables, views, columns, defaults, primary keys, foreign keys, indexes |
| Values | Integers, floats, exact decimals, text, binary, dates, times, timestamps, compressed and chained long values |
| Integrity | Per-page CRC-32 checks and full verification via `verify()` |
| Safety | Zero `unsafe` Rust; 16 MiB default decompression bound |

Unscrambled files, password-encrypted files, other page sizes, index B-trees, and `.tlg` logs are not read.

## Documentation

- [Documentation](docs/README.md): overview of guides, API reference, and examples.
- [Rust API](docs/rust/README.md): opening databases, inspecting schemas, streaming rows, verification, and raw pages.
- [Python API](docs/python/README.md): opening databases, streaming rows, Apache Arrow (PyArrow, Polars, DuckDB), verification, and errors.
- [File format](docs/format.md): on-disk pages, cipher, records, compression, and system catalog.
- [Rust examples](rust/sa17/examples) and [Python examples](python/examples): runnable programs for tables, rows, keys, Arrow queries, and verification.

## Layout

| Path | Contents |
| --- | --- |
| [`rust/sa17`](rust/sa17) | Pure-Rust library crate (`sa17` on crates.io) |
| [`rust/sa17-py`](rust/sa17-py) | [Bimetal](https://github.com/2xf-org/bimetal) Python bindings crate |
| [`rust/sa17-synthetic`](rust/sa17-synthetic) | Private test-fixture writer |
| [`python/`](python) | Typed Python package (`sa17` on PyPI) |
