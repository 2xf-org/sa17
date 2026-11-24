//! Finding a table by its name.

use crate::error::{Error, Result};
use crate::schema::TableDefinition;

/// The one of `tables` that `name` can mean, ignoring ASCII case: the one of
/// that name, or of an owner and a name with a point between them. Either can
/// have points of its own.
pub(crate) fn find<'table>(
    tables: impl IntoIterator<Item = &'table TableDefinition>,
    name: &str,
) -> Result<&'table TableDefinition> {
    let tables = tables.into_iter();
    let found: Vec<&TableDefinition> = tables.filter(|table| is_called(table, name)).collect();
    match found[..] {
        [table] => Ok(table),
        [] => Err(Error::UnknownTable {
            name: name.to_owned(),
        }),
        _ => Err(Error::AmbiguousTable {
            name: name.to_owned(),
            candidates: found.into_iter().map(candidate).collect(),
        }),
    }
}

fn is_called(table: &TableDefinition, name: &str) -> bool {
    if table.name.eq_ignore_ascii_case(name) {
        return true;
    }
    let mut points = name.match_indices('.');
    points.any(|(point, _)| {
        let (owner, rest) = name.split_at(point);
        table.owner.eq_ignore_ascii_case(owner) && table.name.eq_ignore_ascii_case(&rest[1..])
    })
}

fn candidate(table: &TableDefinition) -> String {
    format!("{}.{} (table {})", table.owner, table.name, table.id)
}
