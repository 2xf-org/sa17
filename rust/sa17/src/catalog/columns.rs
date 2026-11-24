//! Reading `ISYSTABCOL`, which has one row per column.

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::bootstrap::{ABSENT_VALUE, COLUMNS, DEFAULT, LONGEST_VALUE, TABLES};
use crate::catalog::tables::TableRow;
use crate::catalog::{Fields, Problems};
use crate::error::{CatalogProblem, Result};
use crate::record::field;
use crate::record::stored_field;
use crate::record::stored_field::Form;
use crate::record::{Columns, RecordReader, Rows, StoredField};
use crate::schema::{Column, DataType};
use crate::store::{PageStore, TextEncoding};
use crate::value::{Undecoded, Value};

/// The columns of every table, by the table's identifier, after a check that
/// the catalog describes its own two tables as the bootstrap does.
///
/// A column whose row is not what it has to be is added to `problems`, and
/// costs its table the columns, or only itself what rows older than it hold. A
/// row that cannot be read at all could be a column of any table, so it is an
/// error.
pub(crate) fn read(
    store: &PageStore,
    tables: &[TableRow],
    problems: &mut Problems,
) -> Result<BTreeMap<u32, Vec<Column>>> {
    let own = tables.binary_search_by_key(&COLUMNS.id, |table| table.id);
    let Some(own) = own.ok().and_then(|own| tables.get(own)) else {
        // What could not be read of the tables is why there is none.
        let reason = problems.as_slice().first().cloned();
        let missing = CatalogProblem::MissingTable {
            table: COLUMNS.name,
        };
        return Err(reason.unwrap_or(missing.into()));
    };
    let catalog = COLUMNS.table(own.pages.clone(), own.extension_pages.clone());
    if let Some(error) = catalog.unlisted() {
        return Err(error);
    }
    let mut columns: BTreeMap<u32, Vec<Column>> = BTreeMap::new();
    let mut unusable = BTreeSet::new();
    let every = Columns::all(&catalog);
    let reader = RecordReader::new(store, &catalog, every, LONGEST_VALUE);
    for row in Rows::of(reader) {
        let row = match row {
            Ok(row) => row,
            Err(error) => COLUMNS
                .without_long_values(store, &catalog, &error)
                .ok_or(error)?,
        };
        let fields = COLUMNS.fields(&row);
        let table_id: u32 = fields.narrow("table_id")?;
        let column_id = fields.narrow("column_id")?;
        let problem = |field| CatalogProblem::Column {
            table_id,
            column_id,
            field,
        };
        match column(&fields, column_id) {
            Ok(mut column) => {
                column.absent = absent_value(&fields, &column, store.text_encoding());
                if column.absent.is_none() {
                    problems.add(problem(ABSENT_VALUE));
                }
                match fields.optional_text(DEFAULT) {
                    Ok(default) => column.default = default.map(str::to_owned),
                    Err(_) => problems.add(problem(DEFAULT)),
                }
                columns.entry(table_id).or_default().push(column);
            }
            Err(field) => {
                unusable.insert(table_id);
                problems.add(problem(field));
            }
        }
    }
    // Without one of its columns, the rows of a table cannot be walked.
    columns.retain(|table, _| !unusable.contains(table));
    for described in columns.values_mut() {
        described.sort_by_key(|column| column.id);
    }
    for bootstrap in [&TABLES, &COLUMNS] {
        let described = columns.get(&bootstrap.id);
        if !described.is_some_and(|described| bootstrap.matches(described)) {
            return Err(CatalogProblem::Layout {
                table: bootstrap.name,
            }
            .into());
        }
    }
    Ok(columns)
}

/// The column that a row of the catalog describes, or the field of the row
/// that does not say what it has to.
fn column(fields: &Fields<'_>, id: u32) -> Result<Column, &'static str> {
    Ok(Column {
        id,
        name: fields
            .text("column_name")
            .map_err(|_| "column_name")?
            .to_owned(),
        domain: fields.narrow("domain_id").map_err(|_| "domain_id")?,
        width: fields.narrow("width").map_err(|_| "width")?,
        scale: fields.narrow("scale").map_err(|_| "scale")?,
        nullable: match fields.text("nulls") {
            Ok("Y") => true,
            Ok("N") => false,
            _ => return Err("nulls"),
        },
        default: None,
        first_schema: fields.narrow("start_schema").map_err(|_| "start_schema")?,
        absent: Some(Value::Null),
    })
}

/// The value that rows older than `column` have for it, if it can be read. The
/// catalog stores it as the bytes the field would have in a row.
fn absent_value(fields: &Fields<'_>, column: &Column, encoding: TextEncoding) -> Option<Value> {
    let Some(stored) = fields.optional_bytes(ABSENT_VALUE).ok()? else {
        return Some(Value::Null);
    };
    if let DataType::Unknown(domain) = column.data_type() {
        return Some(Value::Undecoded(Undecoded::new(domain, stored.to_vec())));
    }
    let mut rest = stored;
    let field = stored_field::take(&mut rest, Form::of(column).ok()?, column).ok()?;
    let value = match field {
        StoredField::Bytes(bytes) => field::value(bytes, column, encoding).ok()?,
        StoredField::Number { control, digits } => field::number(control, digits, column),
        StoredField::Long(_) => return None,
    };
    // What is left would be padding, as in a record.
    rest.iter().all(|&byte| byte == 0).then_some(value)
}
