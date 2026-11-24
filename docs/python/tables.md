[Docs](../README.md) / [Python](README.md) / Tables & Schema

# Tables & Schema

Inspecting catalog objects, columns, primary keys, foreign keys, and indexes.

## `Table`

Handle to a single table, view, or temporary table inside an open `Database`. Returned by `db.table()`, `db[name]`, `db.table_by_id()`, `db.tables()`, or `table.target()`.

```python
plants = db["plants"]
print(plants.owner, plants.name, plants.row_count)

for col in plants.columns:
    print(f"  {col.name}: {col.sql_type} (nullable={col.nullable})")

print("PK:", plants.primary_key)
for fk in plants.foreign_keys:
    print(f"  FK {fk.name}: {fk.columns} -> {fk.target_table}({fk.target_columns})")
```

### Properties

| Property | Type | Description |
| --- | --- | --- |
| `info` | `TableInfo` | Complete catalog metadata snapshot for this table. |
| `id` | `int` | Numeric catalog identifier (`table_id` in `ISYSTAB`). |
| `name` | `str` | Table name. |
| `owner` | `str` | Owning user name (empty if unlisted in `ISYSUSER`). |
| `kind` | `TableKind` | Kind of catalog entry (`Base`, `MaterializedView`, `View`, etc.). |
| `is_system` | `bool` | `True` when owned by a built-in engine user (such as `SYS`). |
| `stores_rows` | `bool` | `True` for base tables and materialized views; `False` for views and temporary tables. |
| `row_count` | `int` | Row count recorded in the catalog. |
| `columns` | `list[ColumnInfo]` | Columns in catalog order. |
| `primary_key` | `list[str]` | Primary key column names in key order (empty if none). |
| `foreign_keys` | `list[ForeignKeyInfo]` | Foreign keys declared on the table. |
| `indexes` | `list[IndexInfo]` | Indexes declared on the table. |
| `closed` | `bool` | Whether `close()` has released this handle. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `target(foreign_key_name)` | `Table \| None` | Resolve the target `Table` referenced by `foreign_key_name` (ASCII case-insensitive). |
| `rows()` | `RowCursor` | Stream all live rows with every column. See [Rows & Cursors](rows.md). |
| `scan(columns)` | `RowCursor` | Stream live rows decoding only the named `columns`. See [Rows & Cursors](rows.md). |
| `row(address, columns=None)` | `Row \| None` | Fetch a single live row by physical `RowAddress`. See [Rows & Cursors](rows.md). |
| `row_addresses()` | `AddressCursor` | Stream every live row's `RowAddress` without decoding values. See [Rows & Cursors](rows.md). |
| `arrow_schema(columns=None)` | `pyarrow.Schema` | Return the Arrow schema for the table or selected columns. See [Apache Arrow](arrow.md). |
| `to_arrow(columns=None, batch_size=65536)` | `pyarrow.Table` | Read the table into a zero-copy `pyarrow.Table`. See [Apache Arrow](arrow.md). |
| `batches(columns=None, batch_size=65536)` | `pyarrow.RecordBatchReader` | Stream lazy zero-copy Arrow batches. See [Apache Arrow](arrow.md). |
| `close()` | `None` | Release the native handle. |

---

## `TableInfo`

Pydantic snapshot of a catalog entry returned by `table.info` and `db.catalog()`.

| Field | Type | Description |
| --- | --- | --- |
| `id` | `int` | Catalog table identifier. |
| `name` | `str` | Table name. |
| `owner_id` | `int` | Numeric identifier of the owning user. |
| `owner` | `str` | Owning user name, or empty if unlisted. |
| `kind` | `TableKind` | Kind of catalog object. |
| `is_system` | `bool` | Whether the owner is a built-in engine user. |
| `stores_rows` | `bool` | Whether the file stores rows for this entry. |
| `row_count` | `int` | Row count recorded in the catalog. |
| `schema_version` | `int` | Current schema version of the table. |
| `columns` | `list[ColumnInfo]` | Columns in catalog order. |
| `primary_key` | `list[int]` | Zero-based column positions of the primary key. |
| `primary_key_columns` | `list[str]` | Column names of the primary key. |
| `foreign_keys` | `list[ForeignKeyInfo]` | Foreign keys declared on the table. |
| `indexes` | `list[IndexInfo]` | Indexes declared on the table. |
| `pages` | `list[int]` | Page numbers storing rows of the table. |
| `extension_pages` | `list[int]` | Extension page numbers storing forwarded rows and long values. |

---

## `ColumnInfo`

Metadata for a single column.

| Field | Type | Description |
| --- | --- | --- |
| `id` | `int` | Column identifier within its table (may have gaps where columns were dropped). |
| `name` | `str` | Column name. |
| `data_type` | `str` | SQL type name without width or precision (e.g. `"varchar"`, `"numeric"`). |
| `sql_type` | `str` | Formatted SQL type including width or precision and scale (e.g. `"varchar(40)"`, `"numeric(10,2)"`). |
| `domain` | `int` | Raw catalog domain identifier. |
| `width` | `int` | Declared width, decimal precision, or fixed storage size. |
| `scale` | `int` | Decimal scale, or character-width flag (`1`) for character columns. |
| `length_unit` | `LengthUnit \| None` | `Bytes` or `Characters` for length-limited string and binary columns; `None` otherwise. |
| `nullable` | `bool` | Whether the column accepts `NULL`. |
| `default_expression` | `str \| None` | Text of the column's `DEFAULT` expression, if declared. |
| `first_schema` | `int` | Table schema version in which this column was added. |

---

## `ForeignKeyInfo`

Metadata for a foreign key constraint.

| Field | Type | Description |
| --- | --- | --- |
| `name` | `str` | Index name backing the foreign key. |
| `positions` | `list[int]` | Zero-based column positions in the referencing table. |
| `columns` | `list[str]` | Column names in the referencing table. |
| `target_id` | `int` | Catalog identifier of the referenced table. |
| `target_table` | `str \| None` | Name of the referenced table, if present in the catalog. |
| `target_positions` | `list[int]` | Zero-based column positions in the referenced table. |
| `target_columns` | `list[str]` | Column names in the referenced table. |

---

## `IndexInfo` & `IndexColumnInfo`

Metadata for a declared catalog index.

### `IndexInfo`

| Field | Type | Description |
| --- | --- | --- |
| `id` | `int` | Index identifier within its table. |
| `name` | `str` | Index name. |
| `kind` | `IndexKind` | Role of the index (`PrimaryKey`, `ForeignKey`, `Secondary`, `Text`, `Other`). |
| `unique` | `bool` | Whether the index enforces uniqueness. |
| `columns` | `list[IndexColumnInfo]` | Indexed columns in key order. |

### `IndexColumnInfo`

| Field | Type | Description |
| --- | --- | --- |
| `position` | `int` | Zero-based column position in the table. |
| `name` | `str` | Column name. |
| `order` | `IndexOrder` | Sort direction (`Ascending` or `Descending`). |

---

## Enums

| Enum | Members |
| --- | --- |
| `TableKind` | `Base`, `MaterializedView`, `GlobalTemporary`, `TextIndex`, `View`, `Other` |
| `LengthUnit` | `Bytes`, `Characters` |
| `IndexKind` | `PrimaryKey`, `ForeignKey`, `Secondary`, `Text`, `Other` |
| `IndexOrder` | `Ascending`, `Descending` |
