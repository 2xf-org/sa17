[Docs](../README.md) / [Python](README.md) / Errors

# Errors

Every library failure raises `sa17.Sa17Error`, a subclass of `RuntimeError` with a typed `code` (`Sa17ErrorCode`) and structured `details` where applicable.

## Handling errors

Branch on `error.code` rather than parsing the message string:

```python
import sa17

try:
    with sa17.Database.open("garden.db") as db:
        table = db.table("missing")
except sa17.Sa17Error as error:
    if error.code == "unknown_table" and isinstance(error.details, sa17.Sa17ErrorUnknownTable):
        print(f"Table not found: {error.details.name}")
    elif error.code == "ambiguous_key" and isinstance(error.details, sa17.Sa17ErrorAmbiguousKey):
        print(f"Multiple candidate keys: {error.details.keys}")
    else:
        raise
```

## `Sa17Error`

| Attribute | Type | Description |
| --- | --- | --- |
| `code` | `Sa17ErrorCode` | Stable snake-case string literal identifying the error category. |
| `details` | Detail model or `None` | Pydantic model carrying structured fields for this error variant. |

## Error codes and detail models

| Code | Detail model | Fields | Meaning |
| --- | --- | --- | --- |
| `io` | `Sa17ErrorIo` | `message: str` | Operating system failed to read the file |
| `format` | `Sa17ErrorFormat` | `message: str` | File is not a supported SQL Anywhere 17 database |
| `no_key` | `None` | - | No page-scrambling key matches the file |
| `ambiguous_key` | `Sa17ErrorAmbiguousKey` | `keys: list[StoreKey]` | Multiple keys fit equally well; supply one in `OpenOptions` |
| `wrong_key` | `None` | - | Explicit key in `OpenOptions` does not fit the file |
| `unsupported_encoding` | `Sa17ErrorUnsupportedEncoding` | `label: str` | Superblock declares an encoding other than `windows-1252` or `UTF-8` |
| `page` | `Sa17ErrorPage` | `page: int`, `message: str` | Page could not be read or failed its CRC-32 checksum |
| `table_page` | `Sa17ErrorTablePage` | `table: str`, `page: int`, `message: str` | A table's data or extension page could not be read |
| `row` | `Sa17ErrorRow` | `table: str`, `page: int`, `slot: int`, `message: str` | A row could not be decoded |
| `page_list` | `Sa17ErrorPageList` | `table: str`, `message: str` | A table's RLE page list could not be decoded |
| `catalog` | `Sa17ErrorCatalog` | `message: str` | System catalog is invalid or inconsistent |
| `unknown_table` | `Sa17ErrorUnknownTable` | `name: str` | No table matches the requested name |
| `ambiguous_table` | `Sa17ErrorAmbiguousTable` | `name: str`, `candidates: list[str]` | Multiple users own a table of that name; qualify as `owner.table` |
| `unknown_column` | `Sa17ErrorUnknownColumn` | `table: str`, `column: str` | Table or row has no column with that name |
| `unknown_position` | `Sa17ErrorUnknownPosition` | `table: str`, `position: int` | Table has no column at that zero-based position |
| `value` | `Sa17ErrorValue` | `table: str`, `column: str`, `message: str` | Column value does not match the requested type |
| `arrow` | `Sa17ErrorArrow` | `message: str` | Converting table data to Apache Arrow failed |
