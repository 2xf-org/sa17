[Docs](../README.md) / [Python](README.md) / Apache Arrow

# Apache Arrow, Polars, and DuckDB

Export any table or column projection across the Arrow C Data and C Stream interfaces with zero copies and the Python GIL released.

## Methods on `Table`

| Method | Returns | Description |
| --- | --- | --- |
| `table.arrow_schema(columns=None)` | `pyarrow.Schema` | Return the Arrow schema for all columns or a selected subset, without scanning rows. |
| `table.to_arrow(columns=None, batch_size=65536)` | `pyarrow.Table` | Materialize the table (or selected columns) into a zero-copy `pyarrow.Table`. |
| `table.batches(columns=None, batch_size=65536)` | `pyarrow.RecordBatchReader` | Stream lazy zero-copy Arrow record batches of at most `batch_size` rows. |

Passing `batch_size=0` raises `Sa17Error` with `code == "arrow"`.

## Usage

### PyArrow & Polars

```python
import polars as pl
import sa17

with sa17.Database.open("garden.db") as db:
    plants = db["plants"]

    pa_table = plants.to_arrow(["id", "name", "height", "price"])
    frame = pl.from_arrow(pa_table)
    print(frame)
```

### Lazy streaming into DuckDB

`table.batches()` returns a `pyarrow.RecordBatchReader` that decodes one batch at a time as DuckDB pulls from it, keeping memory bounded on large tables:

```python
import duckdb
import sa17

with sa17.Database.open("garden.db") as db:
    stream = db["plants"].batches(columns=["name", "price"], batch_size=16_384)
    conn = duckdb.connect()
    conn.register("plants", stream)
    rows = conn.sql(
        "SELECT name, price FROM plants WHERE price > 0 ORDER BY price DESC"
    ).fetchall()
```

## SQL to Arrow type mapping

| SQL Anywhere 17 type | Arrow type | Python scalar (via PyArrow) |
| --- | --- | --- |
| `BIT` | `bool_` | `bool` |
| `TINYINT` | `uint8` | `int` |
| `SMALLINT` | `int16` | `int` |
| `UNSIGNED SMALLINT` | `uint16` | `int` |
| `INTEGER` | `int32` | `int` |
| `UNSIGNED INT` | `uint32` | `int` |
| `BIGINT` | `int64` | `int` |
| `UNSIGNED BIGINT` | `uint64` | `int` |
| `FLOAT` | `float32` | `float` |
| `DOUBLE` | `float64` | `float` |
| `NUMERIC(p, s)`, `DECIMAL(p, s)` (`1 <= p <= 38`) | `decimal128(p, s)` | `decimal.Decimal` |
| `NUMERIC` / `DECIMAL` (`p > 38`) | `string` (`utf8`) | `str` |
| `CHAR`, `VARCHAR` | `string` (`utf8`) | `str` |
| `LONG VARCHAR` | `large_string` (`large_utf8`) | `str` |
| `BINARY`, `VARBINARY` | `binary` | `bytes` |
| `LONG BINARY`, `LONG VARBIT` | `large_binary` | `bytes` |
| `DATE` | `date32` | `datetime.date` |
| `TIME` | `time64[us]` | `datetime.time` |
| `TIMESTAMP` | `timestamp[us]` | `datetime.datetime` |
| Unknown domain | `null` | `None` |

Columns added in a later schema version whose absent value is `NULL` are marked nullable in the Arrow schema even if declared `NOT NULL`.
