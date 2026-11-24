[Docs](../README.md) / Python

# Python API

`sa17` wraps the Rust reader in a typed Python extension (`cp311-abi3`) with Pydantic metadata models and zero-copy Apache Arrow export.

```bash
pip install sa17
```

## Quick start

```python
import sa17

with sa17.Database.open("garden.db") as db:
    for table in db.tables():
        if not table.is_system and table.stores_rows:
            print(table.name, table.row_count)

    plants = db["plants"]
    for row in plants.scan(["id", "name", "price"]):
        print(row["id"], row["name"], row.get_decimal("price"))
```

## Reference

| Page | Covers |
| --- | --- |
| [Database & Options](database.md) | `Database`, `OpenOptions`, `StoreKey`, `Superblock`, `Identity`, `KeySign`, `TextEncoding` |
| [Tables & Schema](tables.md) | `Table`, `TableInfo`, `ColumnInfo`, `ForeignKeyInfo`, `IndexInfo`, `IndexColumnInfo` |
| [Rows & Cursors](rows.md) | `Row`, `RowCursor`, `AddressCursor`, `RowAddress` |
| [Apache Arrow](arrow.md) | `arrow_schema()`, `to_arrow()`, `batches()`, Polars, DuckDB, and SQL-to-Arrow type mapping |
| [Verification & Pages](pages.md) | `verify()`, `Verification`, `PageStore`, `Page`, `RecordInfo` |
| [Errors](errors.md) | `Sa17Error`, `Sa17ErrorCode`, and structured error detail models |

## Examples

Runnable scripts live in [`python/examples/`](../../python/examples):

- [`tables.py`](../../python/examples/tables.py): list tables, columns, primary keys, and foreign keys.
- [`rows.py`](../../python/examples/rows.py): stream rows with optional column projection.
- [`arrow_query.py`](../../python/examples/arrow_query.py): zero-copy Arrow export and DuckDB query.
- [`verify.py`](../../python/examples/verify.py): integrity check and raw page inspection.
