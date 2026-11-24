# SQL Anywhere® 17

Typed Python reader for SQL Anywhere 17 database files.

[Python API](https://github.com/2xf-org/sa17/blob/main/docs/python/README.md) · [File format](https://github.com/2xf-org/sa17/blob/main/docs/format.md) · [Examples](https://github.com/2xf-org/sa17/tree/main/python/examples)

## Installing

```bash
pip install sa17
```

Requires Python 3.11 or newer. Wheels target CPython's stable ABI (`cp311-abi3`).

## Using

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

## Arrow, Polars, and DuckDB

Export any table or column projection without copying:

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
