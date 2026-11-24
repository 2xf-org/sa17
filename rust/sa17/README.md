# SQL Anywhere® 17

Pure-Rust reader for SQL Anywhere 17 database files.

[Rust API](https://github.com/2xf-org/sa17/blob/main/docs/rust/README.md) · [File format](https://github.com/2xf-org/sa17/blob/main/docs/format.md) · [Examples](https://github.com/2xf-org/sa17/tree/main/rust/sa17/examples)

## Installing

```toml
[dependencies]
sa17 = "0.1"
```

## Using

```rust
use sa17::{Columns, Database};

fn main() -> sa17::Result<()> {
    let db = Database::open("garden.db")?;
    let plants = db.table("plants")?;
    let columns = Columns::new(plants.definition(), ["name", "height"])?;

    for row in plants.scan(&columns) {
        let row = row?;
        let name: &str = row.read("name")?;
        let height: Option<f64> = row.read("height")?;
        println!("{name}: {height:?}");
    }
    Ok(())
}
```

## Layers

- **Tables and rows** (`Database`, `Table`, `Columns`, `Row`, `Value`, `Source`): typed scans with column projection, random lookup by `RowAddress`, schema and keys, and `Database::verify`.
- **Pages** (`PageStore`, `Page`, `Record`, `Superblock`): CRC-checked, decoded 4096-byte pages and slot directories.
- **Cipher** (`StoreKey`, `PageKey`, `PageStore::stored`): recovered scrambling constants and raw stored bytes.

Zero `unsafe` Rust. Files are never modified, and decompressed long values default to a 16 MiB bound (`OpenOptions::longest_value`).

## Scope

Reads scrambled SQL Anywhere 17 files (format `3.201.12`, 4096-byte pages, `windows-1252` or `UTF-8`). Refuses unscrambled files, password-encrypted files, other page sizes, and other format versions. Does not read index B-trees or `.tlg` logs.
