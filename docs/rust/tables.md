[Docs](../README.md) / [Rust](README.md) / Tables & Schema

# Tables & Schema

Inspecting catalog objects, columns, SQL types, primary keys, foreign keys, and indexes.

## `Table<'db>`

Cheap `Copy` handle binding a `TableDefinition` to its open `Database`. Returned by `db.table()`, `db.table_by_id()`, `db.tables()`, and `table.target()`.

```rust
let plants = db.table("plants")?;
let def = plants.definition();

println!("{} (owner={}, rows={})", def.name(), def.owner(), def.row_count());
for col in plants.columns() {
    println!("  {} {} nullable={}", col.name(), col.sql_type(), col.is_nullable());
}
for fk in def.foreign_keys() {
    if let Some(target) = plants.target(fk) {
        println!("  FK {} -> {}", fk.name(), target.name());
    }
}
```

| Method | Returns | Description |
| --- | --- | --- |
| `table.definition()` | `&'db TableDefinition` | Full catalog definition for this table. |
| `table.id()` | `u32` | Numeric catalog table identifier. |
| `table.name()` | `&'db str` | Table name. |
| `table.columns()` | `&'db [Column]` | Columns ordered by catalog column number. |
| `table.target(fk)` | `Option<Table<'db>>` | Resolve the target `Table` referenced by `fk: &ForeignKey`. |
| `table.rows()` | `Rows<'db>` | Stream every column of every live row. See [Rows & Values](rows.md). |
| `table.scan(&columns)` | `Rows<'db>` | Stream live rows projected to `columns`. See [Rows & Values](rows.md). |
| `table.row(address, &columns)` | `Result<Option<Row>>` | Fetch a single live row by physical `RowAddress`. See [Rows & Values](rows.md). |
| `table.row_addresses()` | `RowAddresses<'db>` | Stream every live row's `RowAddress` without decoding fields. See [Rows & Values](rows.md). |

---

## `TableDefinition`

Catalog description of a table, materialized view, view, or temporary table.

| Method | Returns | Description |
| --- | --- | --- |
| `def.id()` | `u32` | Catalog table identifier (`table_id` in `ISYSTAB`). |
| `def.name()` | `&str` | Table name. |
| `def.kind()` | `TableKind` | Kind of catalog entry (`Base`, `MaterializedView`, `View`, etc.). |
| `def.owner_id()` | `u32` | Numeric identifier of the owning user. |
| `def.owner()` | `&str` | Owning user name (empty if unlisted in `ISYSUSER`). |
| `def.is_system()` | `bool` | `true` when owned by a built-in engine user (such as `SYS`). |
| `def.stores_rows()` | `bool` | `true` for base tables and materialized views; `false` for views and temporary tables. |
| `def.row_count()` | `u64` | Row count recorded in the catalog. |
| `def.schema()` | `u32` | Current table schema version. |
| `def.columns()` | `&[Column]` | Columns ordered by column number. |
| `def.column(name)` | `Option<&Column>` | Look up a column by name (ASCII case-insensitive). |
| `def.position(name)` | `Option<usize>` | Zero-based index in `columns()` of the named column (ASCII case-insensitive). |
| `def.primary_key()` | `Option<&[usize]>` | Zero-based column positions forming the primary key, if declared. |
| `def.foreign_keys()` | `&[ForeignKey]` | Foreign keys declared on the table. |
| `def.indexes()` | `&[Index]` | Indexes declared on the table. |
| `def.pages()` | `&[u32]` | Page numbers storing rows of this table. |
| `def.extension_pages()` | `&[u32]` | Extension page numbers storing forwarded rows and long values. |

---

## `Column`

Catalog description of a single column.

| Method | Returns | Description |
| --- | --- | --- |
| `col.id()` | `u32` | Column number within its table (may have gaps where columns were dropped). |
| `col.name()` | `&str` | Column name. |
| `col.data_type()` | `DataType` | Parsed SQL data type enum. |
| `col.sql_type()` | `String` | Formatted SQL type string (e.g. `"varchar(40)"`, `"numeric(15,2)"`). |
| `col.domain()` | `u16` | Raw catalog domain identifier. |
| `col.width()` | `u32` | Declared width (characters/bytes), decimal precision, or fixed byte size. |
| `col.scale()` | `u16` | Decimal scale, or character-width flag (`1`) for character columns. |
| `col.length_unit()` | `Option<LengthUnit>` | `Bytes` or `Characters` for length-limited string and binary columns. |
| `col.is_nullable()` | `bool` | Whether the column accepts `NULL`. |
| `col.default()` | `Option<&str>` | Text of the column's `DEFAULT` expression, if declared. |
| `col.first_schema()` | `u32` | Schema version in which this column was added. |
| `col.absent_value()` | `Option<&Value>` | Value supplied for rows written before `first_schema()`, or `None` if unreadable. |

---

## `ForeignKey`, `Index`, and `IndexColumn`

### `ForeignKey`

| Method | Returns | Description |
| --- | --- | --- |
| `fk.name()` | `&str` | Index name backing the foreign key. |
| `fk.positions()` | `&[usize]` | Zero-based column positions in this table. |
| `fk.target_id()` | `u32` | Catalog identifier of the referenced table. |
| `fk.target_positions()` | `&[usize]` | Zero-based column positions in the referenced table. |

### `Index`

| Method | Returns | Description |
| --- | --- | --- |
| `idx.id()` | `u32` | Index identifier within its table. |
| `idx.name()` | `&str` | Index name. |
| `idx.kind()` | `IndexKind` | Role of the index (`PrimaryKey`, `ForeignKey`, `Secondary`, `Text`, `Other`). |
| `idx.is_unique()` | `bool` | Whether the index enforces uniqueness. |
| `idx.columns()` | `&[IndexColumn]` | Indexed columns in key order. |

### `IndexColumn`

| Method | Returns | Description |
| --- | --- | --- |
| `col.position()` | `usize` | Zero-based column position in the table. |
| `col.order()` | `IndexOrder` | Sort direction (`Ascending` or `Descending`). |

---

## Schema Enums

| Enum | Variants |
| --- | --- |
| `TableKind` | `Base`, `MaterializedView`, `GlobalTemporary`, `TextIndex`, `View`, `Other` |
| `DataType` | `Bit`, `TinyInt`, `SmallInt`, `UnsignedSmallInt`, `Integer`, `UnsignedInt`, `BigInt`, `UnsignedBigInt`, `Float`, `Double`, `Numeric`, `Decimal`, `Char`, `VarChar`, `LongVarChar`, `Binary`, `VarBinary`, `LongBinary`, `LongVarBit`, `Date`, `Time`, `Timestamp`, `Unknown(u16)` |
| `LengthUnit` | `Bytes`, `Characters` |
| `IndexKind` | `PrimaryKey`, `ForeignKey`, `Secondary`, `Text`, `Other` |
| `IndexOrder` | `Ascending`, `Descending` |
