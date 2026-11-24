[Docs](../README.md) / Rust

# Rust API

`sa17` is a pure-Rust, `#![forbid(unsafe_code)]` library for reading SQL Anywhere 17 database files. Generated API docs are also published on [docs.rs/sa17](https://docs.rs/sa17).

```toml
[dependencies]
sa17 = "=202611.24.0"
```

## Quick start

```rust
use sa17::{Columns, Database, Decimal};

fn main() -> sa17::Result<()> {
    let db = Database::open("garden.db")?;
    let plants = db.table("plants")?;
    let columns = Columns::new(plants.definition(), ["id", "name", "price"])?;

    for row in plants.scan(&columns) {
        let row = row?;
        let id: i32 = row.read("id")?;
        let name: &str = row.read("name")?;
        let price: Option<Decimal> = row.read("price")?;
        println!("{id}  {name}  {price:?}");
    }
    Ok(())
}
```

## Reference

| Page | Covers |
| --- | --- |
| [Database & Options](database.md) | `Database`, `OpenOptions`, `Source`, `StoreKey`, `KeySign`, `Superblock`, `FormatVersion`, `Identity`, `TextEncoding` |
| [Tables & Schema](tables.md) | `Table`, `TableDefinition`, `Column`, `DataType`, `LengthUnit`, `ForeignKey`, `Index`, `IndexColumn` |
| [Rows & Values](rows.md) | `Columns`, `Row`, `Rows`, `OwnedRows`, `RowAddresses`, `RowAddress`, `RowId`, `RowKey`, `Value`, `FromValue`, `Decimal` |
| [Verification & Pages](pages.md) | `Verification`, `VerificationProblem`, `PageStore`, `Page`, `Record`, `PageKey`, `PageKind`, `PageState` |
| [Errors](errors.md) | `Error`, `Result`, and all `*Problem` detail enums |

## Examples

Runnable programs live in [`rust/sa17/examples/`](../../rust/sa17/examples):

- [`tables.rs`](../../rust/sa17/examples/tables.rs): list user tables with catalog row counts.
- [`rows.rs`](../../rust/sa17/examples/rows.rs): stream and print all rows of a table.
- [`keys.rs`](../../rust/sa17/examples/keys.rs): print foreign key relationships between tables.
- [`verify.rs`](../../rust/sa17/examples/verify.rs): verify page checksums, rows, and counts.
