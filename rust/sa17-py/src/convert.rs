//! Conversions between `sa17` core types and the Bimetal Python boundary types.

use std::collections::BTreeMap;

use serde_json::{Number, Value as JsonValue};

use crate::{
    ColumnInfo, ForeignKeyInfo, Identity, IndexColumnInfo, IndexInfo, IndexKind, IndexOrder,
    KeySign, LengthUnit, OpenOptions, PageKind, PageState, RecordInfo, RowAddress, Sa17Error,
    StoreKey, Superblock, TableInfo, TableKind, TextEncoding, Verification, VerificationProblem,
    VerificationProblemKind,
};

pub(crate) fn open_options(options: Option<OpenOptions>) -> sa17::OpenOptions {
    let mut built = sa17::OpenOptions::new();
    let Some(options) = options else {
        return built;
    };
    if let Some(key) = options.key {
        built = built.key(store_key_to_core(key));
    }
    if let Some(longest) = options.longest_value {
        built = built.longest_value(longest);
    }
    built
}

pub(crate) fn store_key_to_core(key: StoreKey) -> sa17::StoreKey {
    let sign = match key.sign {
        KeySign::Positive => sa17::KeySign::Positive,
        KeySign::Negative => sa17::KeySign::Negative,
    };
    sa17::StoreKey::new(key.mask, key.addend, sign)
}

pub(crate) fn store_key_from_core(key: sa17::StoreKey) -> StoreKey {
    let sign = match key.sign() {
        sa17::KeySign::Positive => KeySign::Positive,
        sa17::KeySign::Negative => KeySign::Negative,
    };
    StoreKey {
        mask: key.mask(),
        addend: key.addend(),
        sign,
    }
}

pub(crate) fn superblock(superblock: &sa17::Superblock) -> Superblock {
    Superblock {
        version: superblock.version().to_string(),
        lsn: superblock.lsn(),
        database_page_count: superblock.database_page_count(),
    }
}

pub(crate) fn identity(identity: sa17::Identity) -> Identity {
    Identity {
        page_count: identity.page_count(),
        lsn: identity.lsn(),
        digest: identity.to_string(),
    }
}

pub(crate) fn text_encoding(encoding: sa17::TextEncoding) -> TextEncoding {
    match encoding {
        sa17::TextEncoding::Windows1252 => TextEncoding::Windows1252,
        sa17::TextEncoding::Utf8 => TextEncoding::Utf8,
        _ => TextEncoding::Utf8,
    }
}

pub(crate) fn page_state(state: sa17::PageState) -> PageState {
    match state {
        sa17::PageState::Written => PageState::Written,
        sa17::PageState::Unwritten => PageState::Unwritten,
        _ => PageState::Written,
    }
}

pub(crate) fn page_kind(kind: sa17::PageKind) -> PageKind {
    match kind {
        sa17::PageKind::Table => PageKind::Table,
        sa17::PageKind::Extension => PageKind::Extension,
        sa17::PageKind::PageList => PageKind::PageList,
        sa17::PageKind::Other | _ => PageKind::Other,
    }
}

pub(crate) fn table_kind(kind: sa17::TableKind) -> TableKind {
    match kind {
        sa17::TableKind::Base => TableKind::Base,
        sa17::TableKind::MaterializedView => TableKind::MaterializedView,
        sa17::TableKind::GlobalTemporary => TableKind::GlobalTemporary,
        sa17::TableKind::TextIndex => TableKind::TextIndex,
        sa17::TableKind::View => TableKind::View,
        sa17::TableKind::Other | _ => TableKind::Other,
    }
}

pub(crate) fn length_unit(unit: sa17::LengthUnit) -> LengthUnit {
    match unit {
        sa17::LengthUnit::Bytes => LengthUnit::Bytes,
        sa17::LengthUnit::Characters => LengthUnit::Characters,
        _ => LengthUnit::Bytes,
    }
}

pub(crate) fn index_kind(kind: sa17::IndexKind) -> IndexKind {
    match kind {
        sa17::IndexKind::PrimaryKey => IndexKind::PrimaryKey,
        sa17::IndexKind::ForeignKey => IndexKind::ForeignKey,
        sa17::IndexKind::Secondary => IndexKind::Secondary,
        sa17::IndexKind::Text => IndexKind::Text,
        sa17::IndexKind::Other | _ => IndexKind::Other,
    }
}

pub(crate) fn index_order(order: sa17::IndexOrder) -> IndexOrder {
    match order {
        sa17::IndexOrder::Ascending => IndexOrder::Ascending,
        sa17::IndexOrder::Descending => IndexOrder::Descending,
    }
}

pub(crate) fn row_address_from_core(address: sa17::RowAddress) -> RowAddress {
    RowAddress {
        page: address.page(),
        slot: address.slot(),
    }
}

pub(crate) fn row_address_to_core(address: RowAddress) -> sa17::RowAddress {
    sa17::RowAddress::new(address.page, address.slot)
}

pub(crate) fn column_info(column: &sa17::Column) -> ColumnInfo {
    ColumnInfo {
        id: column.id(),
        name: column.name().to_owned(),
        data_type: column.data_type().name().to_owned(),
        sql_type: column.sql_type(),
        domain: column.domain(),
        width: column.width(),
        scale: column.scale(),
        length_unit: column.length_unit().map(length_unit),
        nullable: column.is_nullable(),
        default_expression: column.default().map(str::to_owned),
        first_schema: column.first_schema(),
    }
}

pub(crate) fn foreign_key_info(
    database: &sa17::Database,
    definition: &sa17::TableDefinition,
    key: &sa17::ForeignKey,
) -> ForeignKeyInfo {
    let target = database.table_by_id(key.target_id());
    let columns: Vec<String> = key
        .positions()
        .iter()
        .filter_map(|&pos| definition.columns().get(pos))
        .map(|column| column.name().to_owned())
        .collect();
    let target_columns: Vec<String> = target
        .map(|table| {
            key.target_positions()
                .iter()
                .filter_map(|&pos| table.columns().get(pos))
                .map(|column| column.name().to_owned())
                .collect()
        })
        .unwrap_or_default();
    ForeignKeyInfo {
        name: key.name().to_owned(),
        positions: key.positions().iter().map(|&pos| pos as u32).collect(),
        columns,
        target_id: key.target_id(),
        target_table: target.map(|table| table.name().to_owned()),
        target_positions: key
            .target_positions()
            .iter()
            .map(|&pos| pos as u32)
            .collect(),
        target_columns,
    }
}

pub(crate) fn index_info(definition: &sa17::TableDefinition, index: &sa17::Index) -> IndexInfo {
    let columns: Vec<IndexColumnInfo> = index
        .columns()
        .iter()
        .map(|column| {
            let name = definition
                .columns()
                .get(column.position())
                .map(|col| col.name().to_owned())
                .unwrap_or_default();
            IndexColumnInfo {
                position: column.position() as u32,
                name,
                order: index_order(column.order()),
            }
        })
        .collect();
    IndexInfo {
        id: index.id(),
        name: index.name().to_owned(),
        kind: index_kind(index.kind()),
        unique: index.is_unique(),
        columns,
    }
}

pub(crate) fn table_info(
    database: &sa17::Database,
    definition: &sa17::TableDefinition,
) -> TableInfo {
    let primary_key: Vec<u32> = definition
        .primary_key()
        .map(|positions| positions.iter().map(|&pos| pos as u32).collect())
        .unwrap_or_default();
    let primary_key_columns: Vec<String> = definition
        .primary_key()
        .map(|positions| {
            positions
                .iter()
                .filter_map(|&pos| definition.columns().get(pos))
                .map(|column| column.name().to_owned())
                .collect()
        })
        .unwrap_or_default();
    let foreign_keys: Vec<ForeignKeyInfo> = definition
        .foreign_keys()
        .iter()
        .map(|key| foreign_key_info(database, definition, key))
        .collect();
    let indexes: Vec<IndexInfo> = definition
        .indexes()
        .iter()
        .map(|index| index_info(definition, index))
        .collect();
    TableInfo {
        id: definition.id(),
        name: definition.name().to_owned(),
        owner_id: definition.owner_id(),
        owner: definition.owner().to_owned(),
        kind: table_kind(definition.kind()),
        is_system: definition.is_system(),
        stores_rows: definition.stores_rows(),
        row_count: definition.row_count(),
        schema_version: definition.schema(),
        columns: definition.columns().iter().map(column_info).collect(),
        primary_key,
        primary_key_columns,
        foreign_keys,
        indexes,
        pages: definition.pages().to_vec(),
        extension_pages: definition.extension_pages().to_vec(),
    }
}

pub(crate) fn record_info(record: &sa17::Record<'_>) -> RecordInfo {
    RecordInfo {
        address: row_address_from_core(record.address()),
        flags: record.flags(),
        length: record.bytes().len() as u32,
    }
}

pub(crate) fn verification(report: &sa17::Verification) -> Verification {
    let problems: Vec<VerificationProblem> = report
        .problems()
        .iter()
        .map(|problem| {
            let (kind, table) = match problem {
                sa17::VerificationProblem::Catalog(_) => (VerificationProblemKind::Catalog, None),
                sa17::VerificationProblem::Page(_) => (VerificationProblemKind::Page, None),
                sa17::VerificationProblem::Rows { table, .. } => {
                    (VerificationProblemKind::Rows, Some(table.clone()))
                }
                sa17::VerificationProblem::RowCount { table, .. } => {
                    (VerificationProblemKind::RowCount, Some(table.clone()))
                }
                _ => (VerificationProblemKind::Catalog, None),
            };
            VerificationProblem {
                kind,
                message: problem.to_string(),
                table,
            }
        })
        .collect();
    Verification {
        is_sound: report.is_sound(),
        catalog_problems: report.catalog_problems(),
        pages: report.pages(),
        unwritten_pages: report.unwritten_pages(),
        damaged_pages: report.damaged_pages(),
        tables: report.tables(),
        differing_tables: report.differing_tables(),
        rows: report.rows(),
        unreadable_rows: report.unreadable_rows(),
        unreadable_pages: report.unreadable_pages(),
        problems,
    }
}

/// Converts a single `sa17::Value` into a JSON-compatible Python value.
///
/// Exact decimals, dates, times, and timestamps serialize to their canonical
/// ISO/decimal string form, and binary values serialize to lowercase hex. Use
/// `Row.get_bytes`, `Row.get_date`, or `Table.to_arrow` for native Python
/// `bytes`, `datetime`, and `decimal.Decimal` values.
pub(crate) fn value_to_json(value: &sa17::Value) -> JsonValue {
    match value {
        sa17::Value::Null => JsonValue::Null,
        sa17::Value::Boolean(flag) => JsonValue::Bool(*flag),
        sa17::Value::Integer(number) => {
            if let Ok(signed) = i64::try_from(*number) {
                JsonValue::Number(Number::from(signed))
            } else if let Ok(unsigned) = u64::try_from(*number) {
                JsonValue::Number(Number::from(unsigned))
            } else {
                JsonValue::String(number.to_string())
            }
        }
        sa17::Value::Float(number) => {
            Number::from_f64(f64::from(*number)).map_or(JsonValue::Null, JsonValue::Number)
        }
        sa17::Value::Double(number) => {
            Number::from_f64(*number).map_or(JsonValue::Null, JsonValue::Number)
        }
        sa17::Value::Decimal(decimal) => JsonValue::String(decimal.to_string()),
        sa17::Value::Text(text) => JsonValue::String(text.clone()),
        sa17::Value::Bytes(bytes) => JsonValue::String(hex_encode(bytes)),
        sa17::Value::Date(date) => JsonValue::String(date.to_string()),
        sa17::Value::Time(time) => JsonValue::String(time.to_string()),
        sa17::Value::Timestamp(timestamp) => JsonValue::String(timestamp.to_string()),
        sa17::Value::Undecoded(undecoded) => JsonValue::String(hex_encode(undecoded.bytes())),
        _ => JsonValue::Null,
    }
}

pub(crate) fn row_to_dict(row: &sa17::Row) -> BTreeMap<String, JsonValue> {
    row.columns()
        .names()
        .zip(row.values())
        .map(|(name, value)| (name.to_owned(), value_to_json(value)))
        .collect()
}

pub(crate) fn error(err: sa17::Error) -> Sa17Error {
    let message = err.to_string();
    match err {
        sa17::Error::Read(_) => Sa17Error::Io { message },
        sa17::Error::Format(_) => Sa17Error::Format { message },
        sa17::Error::NoKey => Sa17Error::NoKey,
        sa17::Error::AmbiguousKey { keys } => Sa17Error::AmbiguousKey {
            keys: keys.into_iter().map(store_key_from_core).collect(),
        },
        sa17::Error::WrongKey => Sa17Error::WrongKey,
        sa17::Error::UnsupportedEncoding { label } => Sa17Error::UnsupportedEncoding { label },
        sa17::Error::Page { page, .. } => Sa17Error::Page { page, message },
        sa17::Error::TablePage { table, page, .. } => Sa17Error::TablePage {
            table,
            page,
            message,
        },
        sa17::Error::Row { table, address, .. } => Sa17Error::Row {
            table,
            page: address.page(),
            slot: address.slot(),
            message,
        },
        sa17::Error::PageList { table, .. } => Sa17Error::PageList { table, message },
        sa17::Error::Catalog(_) => Sa17Error::Catalog { message },
        sa17::Error::UnknownTable { name } => Sa17Error::UnknownTable { name },
        sa17::Error::AmbiguousTable { name, candidates } => {
            Sa17Error::AmbiguousTable { name, candidates }
        }
        sa17::Error::UnknownColumn { table, column } => Sa17Error::UnknownColumn { table, column },
        sa17::Error::UnknownPosition { table, position } => Sa17Error::UnknownPosition {
            table,
            position: position as u32,
        },
        sa17::Error::WrongColumns { .. } | sa17::Error::WrongDatabase { .. } => {
            Sa17Error::Catalog { message }
        }
        sa17::Error::Value { table, column, .. } => Sa17Error::Value {
            table,
            column,
            message,
        },
        _ => Sa17Error::Format { message },
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
