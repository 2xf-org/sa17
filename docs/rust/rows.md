[Docs](../README.md) / [Rust](README.md) / Rows & Values

# Rows & Values

Projecting columns, streaming rows, random lookup by address, typed extraction with `FromValue`, and exact `Decimal` arithmetic.

## `Columns`

A compiled column projection bound to a single `TableDefinition`. Scanning with `Columns` decodes only the requested fields and steps over everything else (avoiding text decoding and long-value chain reads). Cloning is cheap (`Arc`-backed).

```rust
use sa17::{Columns, Decimal};

let columns = Columns::new(plants.definition(), ["id", "name", "price"])?;

for row in plants.scan(&columns) {
    let row = row?;
    let id: i32 = row.read(0)?;             // by zero-based position
    let name: &str = row.read("name")?;     // or by name (ASCII case-insensitive)
    let price: Option<Decimal> = row.read("price")?;
    println!("{id}: {name} ({price:?})");
}
```

### Constructors

| Method | Returns | Description |
| --- | --- | --- |
| `Columns::all(table)` | `Columns` | Select every column in catalog order. |
| `Columns::none(table)` | `Columns` | Select zero columns (useful for counting or checking row presence). |
| `Columns::new(table, names)` | `Result<Columns>` | Select columns by name (ASCII case-insensitive). Fails on any unknown column. |
| `Columns::at(table, positions)` | `Result<Columns>` | Select columns by zero-based index in `table.columns()` (such as primary key positions). |
| `Columns::with_optional(table, required, optional)` | `Result<Columns>` | Require `required` columns; missing columns in `optional` read as `NULL`. |

### Methods

| Method | Returns | Description |
| --- | --- | --- |
| `cols.table_name()` | `&str` | Name of the table these columns belong to. |
| `cols.len()` | `usize` | Number of projected columns. |
| `cols.is_empty()` | `bool` | Whether zero columns were selected. |
| `cols.names()` | `impl ExactSizeIterator<Item = &str>` | Projected column names in selection order. |
| `cols.position(name)` | `Option<usize>` | Zero-based position of `name` within this projection. |
| `cols.column(position)` | `Option<&Column>` | Catalog `Column` definition for the projected slot, if present. |
| `cols.check(table)` | `Result<()>` | Verify that this projection belongs to `table`. |

---

## `Row`

A single decoded row carrying its `RowId`, `Columns`, and `Value` slice.

| Method | Returns | Description |
| --- | --- | --- |
| `row.id()` | `RowId` | Identifier of this row (`RowId::Stored(RowAddress)` for rows from disk). |
| `row.columns()` | `&Columns` | Column projection this row was read with. |
| `row.values()` | `&[Value]` | Decoded values in the order of `row.columns()`. |
| `row.values_mut()` | `&mut [Value]` | Mutable access to the row's values. |
| `row.into_values()` | `Vec<Value>` | Consume the row and return its values. |
| `row.value(position)` | `Option<&Value>` | Value at zero-based `position` in `row.columns()`. |
| `row.get(name)` | `Option<&Value>` | Value of column `name` (ASCII case-insensitive). |
| `row.read::<T>(column)` | `Result<T>` | Convert column (name `&str` or position `usize`) to any `T: FromValue`. |
| `row.key()` | `Option<RowKey>` | Build a hashable, comparable `RowKey` from the row's values (`None` if any value is `NULL`). |

---

## Iterators & Identifiers

| Type | Item / Fields | Description |
| --- | --- | --- |
| `Rows<'db>` | `Result<Row>` | Borrowed streaming row iterator returned by `table.rows()` and `table.scan()`. |
| `OwnedRows` | `Result<Row>` | `'static` streaming row iterator returned by `Arc::scan_owned()`. |
| `RowAddresses<'db>` | `Result<RowAddress>` | Borrowed iterator over live row addresses without decoding fields. |
| `OwnedRowAddresses` | `Result<RowAddress>` | `'static` address iterator returned by `Arc::row_addresses_owned()`. |
| `RowAddress` | `page() -> u32`, `slot() -> u16` | Physical zero-based page number and slot index inside the file. |
| `RowId` | `RowId::Stored(RowAddress)` | Stable identifier for a row; `id.address()` returns `Option<RowAddress>`. |
| `RowKey` | `RowKey::new(&[Value])` | Comparable, hashable key built from non-null column values (for lookups and joins). |

---

## `FromValue` Conversions

`row.read::<T>(column)` accepts any `T` implementing `FromValue`. Wrap any type in `Option<T>` for nullable columns.

| Rust type | SQL types accepted |
| --- | --- |
| `bool` | `BIT` |
| `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `i128` | Any integer type whose value fits the target range |
| `f32` | `FLOAT` |
| `f64` | `FLOAT`, `DOUBLE` |
| `sa17::Decimal` | `NUMERIC`, `DECIMAL` (exact base-10, up to 38 digits) |
| `&str`, `String` | `CHAR`, `VARCHAR`, `LONG VARCHAR` |
| `&[u8]`, `Vec<u8>` | `BINARY`, `VARBINARY`, `LONG BINARY`, `LONG VARBIT` |
| `chrono::NaiveDate` | `DATE` (re-exported via `sa17::chrono`) |
| `chrono::NaiveTime` | `TIME` |
| `chrono::NaiveDateTime` | `TIMESTAMP` |
| `&Value` | Any value, including `Value::Null` and `Value::Undecoded` |

---

## `Value` & `Undecoded`

Dynamic representation of any decoded column cell.

| Variant | Payload | Accessor |
| --- | --- | --- |
| `Value::Null` | - | `is_null() -> bool` |
| `Value::Boolean` | `bool` | `as_bool() -> Option<bool>` |
| `Value::Integer` | `i128` | `as_integer() -> Option<i128>` |
| `Value::Float` | `f32` | `as_f32() -> Option<f32>` |
| `Value::Double` | `f64` | `as_f64() -> Option<f64>` |
| `Value::Decimal` | `Decimal` | `as_decimal() -> Option<Decimal>` |
| `Value::Text` | `String` | `as_str() -> Option<&str>` |
| `Value::Bytes` | `Vec<u8>` | `as_bytes() -> Option<&[u8]>` |
| `Value::Date` | `NaiveDate` | `as_date() -> Option<NaiveDate>` |
| `Value::Time` | `NaiveTime` | `as_time() -> Option<NaiveTime>` |
| `Value::Timestamp` | `NaiveDateTime` | `as_timestamp() -> Option<NaiveDateTime>` |
| `Value::Undecoded` | `Undecoded` | `domain() -> u16`, `bytes() -> &[u8]` (out-of-range or unknown domain values) |

---

## `Decimal`

Exact base-10 number (`mantissa * 10^-scale`, up to 38 digits). Values compare and hash numerically (`1.5 == 1.50`), while keeping their scale for formatting.

```rust
use sa17::Decimal;

let price: Decimal = "19.90".parse()?;
let qty = Decimal::from(3);
let total = price.checked_mul(qty).expect("fits in 38 digits");
assert_eq!(total.to_string(), "59.70");
```

| Method | Returns | Description |
| --- | --- | --- |
| `Decimal::new(mantissa, scale)` | `Option<Decimal>` | Construct from `i128` mantissa and `u16` scale (`None` only for `i128::MIN`). |
| `d.mantissa()` | `i128` | Unscaled integer mantissa. |
| `d.scale()` | `u16` | Digits after the decimal point. |
| `d.precision()` | `u32` | Total decimal digits needed to hold the value with its scale. |
| `d.is_zero()` / `d.is_negative()` | `bool` | Sign predicates. |
| `d.abs()` | `Decimal` | Absolute value. |
| `d.rescale(scale)` | `Option<Decimal>` | Exact rescaling without losing non-zero digits. |
| `d.round(scale)` | `Option<Decimal>` | Round half away from zero to at most `scale` decimal places. |
| `d.checked_add(rhs)` / `checked_sub` / `checked_mul` | `Option<Decimal>` | Exact overflow-checked arithmetic. |
| `d.checked_mul_round(rhs, scale)` | `Option<Decimal>` | Multiply and round to `scale` decimal places. |
