[Docs](../README.md) / [Python](README.md) / Rows & Cursors

# Rows & Cursors

Streaming rows lazily, projecting columns, fetching rows by physical address, and extracting typed values.

## Streaming rows

`table.rows()` streams every column of every live row, one 4096-byte page at a time. Deleted rows and stale pages are skipped, and forwarded rows are resolved transparently.

When you only need a subset of columns, pass their names to `table.scan(columns)`. Unselected columns are stepped over without decoding text or following long-value chains:

```python
from decimal import Decimal
import sa17

with sa17.Database.open("garden.db") as db:
    plants = db["plants"]

    for row in plants.scan(["id", "name", "price"]):
        raw_price = row.get_decimal("price")
        price = Decimal(raw_price) if raw_price is not None else None
        print(row.address, row["id"], row["name"], price)
```

## Random lookup by address

Enumerate row locations without decoding any column values, then fetch individual rows on demand:

```python
for address in plants.row_addresses():
    row = plants.row(address, columns=["id", "name"])
    if row is not None:
        print(address.page, address.slot, row["name"])
```

---

## `Row`

A single decoded row returned by `RowCursor` or `table.row()`.

### Properties

| Property | Type | Description |
| --- | --- | --- |
| `address` | `RowAddress \| None` | Physical page and slot address of the row inside the file. |
| `columns` | `list[str]` | Column names present on this row, in the order requested. |
| `closed` | `bool` | Whether `close()` has released this handle. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `to_dict()` | `dict[str, Any]` | All values as a mapping from column name to JSON-compatible Python value. |
| `values()` | `list[Any]` | Positional list of values matching `row.columns`. |
| `get_bytes(column)` | `bytes \| None` | Exact raw bytes for `BINARY`, `VARBINARY`, `LONG BINARY`, or `LONG VARBIT` (`None` when `NULL`). |
| `get_date(column)` | `datetime.date \| None` | Exact `datetime.date` for a `DATE` column (`None` when `NULL`). |
| `get_decimal(column)` | `str \| None` | Exact base-10 decimal string for `NUMERIC` or `DECIMAL` (`None` when `NULL`). Pass to `decimal.Decimal`. |
| `close()` | `None` | Release the native handle. |

### Protocols

| Operation | Behavior |
| --- | --- |
| `len(row)` | Number of columns on this row. |
| `column in row` | `True` if `column` (ASCII case-insensitive) is present on this row. |
| `row[column]` | JSON-compatible value for `column` (ASCII case-insensitive). Raises `Sa17Error` (`unknown_column`) if absent. |

### Value representation in `row[column]` and `to_dict()`

| SQL type | Value returned by `row[column]` | Native typed alternative |
| --- | --- | --- |
| `NULL` | `None` | `None` |
| `BIT` | `bool` | `row[column]` or Arrow |
| Integer types | `int` | `row[column]` or Arrow |
| `FLOAT`, `DOUBLE` | `float` | `row[column]` or Arrow |
| `NUMERIC`, `DECIMAL` | `str` (exact base-10) | `row.get_decimal(col)` or Arrow `decimal128` |
| `CHAR`, `VARCHAR`, `LONG VARCHAR` | `str` | `row[column]` or Arrow |
| `BINARY`, `VARBINARY`, `LONG BINARY`, `LONG VARBIT` | `str` (lowercase hex) | `row.get_bytes(col)` or Arrow `binary` |
| `DATE` | `str` (`YYYY-MM-DD`) | `row.get_date(col)` or Arrow `date32` |
| `TIME` | `str` (`HH:MM:SS.ffffff`) | Arrow `time64[us]` |
| `TIMESTAMP` | `str` (`YYYY-MM-DD HH:MM:SS.ffffff`) | Arrow `timestamp[us]` |

---

## `RowCursor`

Lazy iterator returned by `table.rows()` and `table.scan(columns)`. Supports `for row in cursor`, `next(cursor)`, and `with cursor:`.

| Member | Type / Returns | Description |
| --- | --- | --- |
| `next_row()` | `Row \| None` | Decode and return the next live row, or `None` at the end of the table. |
| `closed` | `bool` | Whether `close()` has released this cursor. |
| `close()` | `None` | Release the cursor early. |

---

## `AddressCursor`

Lazy iterator returned by `table.row_addresses()`. Supports `for addr in cursor`, `next(cursor)`, and `with cursor:`.

| Member | Type / Returns | Description |
| --- | --- | --- |
| `next_address()` | `RowAddress \| None` | Yield the next live row's address, or `None` at the end of the table. |
| `closed` | `bool` | Whether `close()` has released this cursor. |
| `close()` | `None` | Release the cursor early. |

---

## `RowAddress`

Physical location of a row inside the database file.

| Field | Type | Description |
| --- | --- | --- |
| `page` | `int` | Zero-based page number. |
| `slot` | `int` | Zero-based slot index within the page. |
