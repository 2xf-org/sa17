//! Reading the keys and the indexes of every table.
//!
//! `ISYSIDX` has one row per index, the ones behind the keys included,
//! `ISYSIDXCOL` one row per column of an index, and `ISYSFKEY` one row per
//! foreign key, which shares its identifier with the index behind it.

use std::collections::BTreeMap;

use crate::catalog::bootstrap::SYS;
use crate::catalog::{Fields, Problems};
use crate::error::{CatalogProblem, Error, Result};
use crate::record::{Columns, Row, Rows};
use crate::schema::{ForeignKey, Index, IndexColumn, IndexKind, IndexOrder, TableDefinition};
use crate::store::PageStore;

const INDEXES: &str = "ISYSIDX";
const INDEX_COLUMNS: &str = "ISYSIDXCOL";
const FOREIGN_KEYS: &str = "ISYSFKEY";

/// The code for an index that lets rows share values.
const NOT_UNIQUE: u64 = 4;

/// An index, by its table and its number within the table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct IndexId {
    table: u32,
    number: u32,
}

impl IndexId {
    /// The index is not described in a way that can be used.
    fn problem(self) -> Error {
        CatalogProblem::Index {
            table_id: self.table,
            index_id: self.number,
        }
        .into()
    }
}

/// One column of an index as `ISYSIDXCOL` describes it.
struct Entry {
    sequence: u32,
    column: u32,
    /// `None` when it is neither of the two.
    order: Option<IndexOrder>,
    target_column: Option<u32>,
}

/// The columns of every index, in the index's order.
type Entries = BTreeMap<IndexId, Vec<Entry>>;

/// Gives `tables`, which are sorted by identifier, the keys and the indexes
/// that they declare. A file without `ISYSIDX` declares none. What cannot be
/// read is added to `problems`, and costs its own index, or every index when
/// it could be a column of any.
pub(crate) fn give(store: &PageStore, tables: &mut [TableDefinition], problems: &mut Problems) {
    match read(store, tables, problems) {
        Ok((indexes, foreign_keys)) => {
            for (id, index) in indexes {
                if let Some(table) = table_mut(tables, id.table) {
                    if index.kind == IndexKind::PrimaryKey {
                        let columns = index.columns.iter();
                        table.primary_key = columns.map(|column| column.position).collect();
                    }
                    table.indexes.push(index);
                }
            }
            for (id, key) in foreign_keys {
                if let Some(table) = table_mut(tables, id.table) {
                    table.foreign_keys.push(key);
                }
            }
        }
        Err(error) => problems.add(error),
    }
}

type Indexes = BTreeMap<IndexId, Index>;
type ForeignKeys = Vec<(IndexId, ForeignKey)>;

fn read(
    store: &PageStore,
    tables: &[TableDefinition],
    problems: &mut Problems,
) -> Result<(Indexes, ForeignKeys)> {
    let Some(catalog) = find(tables, INDEXES) else {
        return Ok((Indexes::new(), ForeignKeys::new()));
    };
    let entries = entries(store, required(tables, INDEX_COLUMNS)?)?;
    let mut indexes = Indexes::new();
    for row in rows(
        store,
        catalog,
        [
            "table_id",
            "index_id",
            "index_name",
            "index_category",
            "unique",
        ],
    )? {
        match row.and_then(|row| index(&row, tables, &entries)) {
            Ok((id, index)) => {
                indexes.insert(id, index);
            }
            Err(error) => problems.add(error),
        }
    }
    let mut foreign_keys = ForeignKeys::new();
    for row in rows(
        store,
        required(tables, FOREIGN_KEYS)?,
        ["foreign_table_id", "foreign_index_id", "primary_table_id"],
    )? {
        match row.and_then(|row| foreign_key(&row, tables, &entries, &indexes)) {
            Ok(key) => foreign_keys.push(key),
            Err(error) => problems.add(error),
        }
    }
    Ok((indexes, foreign_keys))
}

/// The columns called `names` of every row of a catalog table.
fn rows<'db, const N: usize>(
    store: &'db PageStore,
    catalog: &'db TableDefinition,
    names: [&str; N],
) -> Result<Rows<'db>> {
    Ok(Rows::new(store, catalog, Columns::new(catalog, names)?))
}

fn entries(store: &PageStore, catalog: &TableDefinition) -> Result<Entries> {
    let mut entries = Entries::new();
    for row in rows(
        store,
        catalog,
        [
            "table_id",
            "index_id",
            "sequence",
            "column_id",
            "order",
            "primary_column_id",
        ],
    )? {
        let row = row?;
        let fields = Fields::new(INDEX_COLUMNS, &row);
        let id = IndexId {
            table: fields.narrow("table_id")?,
            number: fields.narrow("index_id")?,
        };
        entries.entry(id).or_default().push(Entry {
            sequence: fields.narrow("sequence")?,
            column: fields.narrow("column_id")?,
            order: match fields.text("order")? {
                "A" => Some(IndexOrder::Ascending),
                "D" => Some(IndexOrder::Descending),
                _ => None,
            },
            target_column: fields.optional_narrow("primary_column_id")?,
        });
    }
    for columns in entries.values_mut() {
        columns.sort_by_key(|entry| entry.sequence);
    }
    Ok(entries)
}

fn index(row: &Row, tables: &[TableDefinition], entries: &Entries) -> Result<(IndexId, Index)> {
    let fields = Fields::new(INDEXES, row);
    let id = IndexId {
        table: fields.narrow("table_id")?,
        number: fields.narrow("index_id")?,
    };
    let table = table(tables, id.table).ok_or_else(|| id.problem())?;
    let columns: Option<Vec<IndexColumn>> = listed(entries, id)
        .iter()
        .map(|entry| {
            Some(IndexColumn {
                position: position(table, entry.column)?,
                order: entry.order?,
            })
        })
        .collect();
    let index = Index {
        id: id.number,
        name: fields.text("index_name")?.to_owned(),
        kind: IndexKind::from_catalog(fields.unsigned("index_category")?),
        unique: fields.unsigned("unique")? != NOT_UNIQUE,
        columns: columns.ok_or_else(|| id.problem())?,
    };
    Ok((id, index))
}

fn foreign_key(
    row: &Row,
    tables: &[TableDefinition],
    entries: &Entries,
    indexes: &Indexes,
) -> Result<(IndexId, ForeignKey)> {
    let fields = Fields::new(FOREIGN_KEYS, row);
    let id = IndexId {
        table: fields.narrow("foreign_table_id")?,
        number: fields.narrow("foreign_index_id")?,
    };
    let target_id: u32 = fields.narrow("primary_table_id")?;
    let (Some(table), Some(target)) = (table(tables, id.table), table(tables, target_id)) else {
        return Err(id.problem());
    };
    let pairs: Option<Vec<(usize, usize)>> = listed(entries, id)
        .iter()
        .map(|entry| {
            let own = position(table, entry.column)?;
            Some((own, position(target, entry.target_column?)?))
        })
        .collect();
    let pairs = pairs.filter(|pairs| !pairs.is_empty());
    let (positions, target_positions) = pairs.ok_or_else(|| id.problem())?.into_iter().unzip();
    let index = indexes.get(&id).ok_or_else(|| id.problem())?;
    let key = ForeignKey {
        name: index.name.clone(),
        positions,
        target_id,
        target_positions,
    };
    Ok((id, key))
}

fn listed(entries: &Entries, id: IndexId) -> &[Entry] {
    entries.get(&id).map_or(&[][..], Vec::as_slice)
}

/// The catalog table called `name`, which a file with indexes has.
fn required<'tables>(
    tables: &'tables [TableDefinition],
    name: &'static str,
) -> Result<&'tables TableDefinition> {
    Ok(find(tables, name).ok_or(CatalogProblem::MissingTable { table: name })?)
}

/// The engine's table called `name`.
fn find<'tables>(
    tables: &'tables [TableDefinition],
    name: &str,
) -> Option<&'tables TableDefinition> {
    let mut tables = tables.iter();
    tables.find(|table| table.owner_id == SYS && table.name == name)
}

fn table(tables: &[TableDefinition], id: u32) -> Option<&TableDefinition> {
    let position = tables.binary_search_by_key(&id, TableDefinition::id).ok()?;
    tables.get(position)
}

fn table_mut(tables: &mut [TableDefinition], id: u32) -> Option<&mut TableDefinition> {
    let position = tables.binary_search_by_key(&id, TableDefinition::id).ok()?;
    tables.get_mut(position)
}

/// The position of the column of `table` with the number `column`.
fn position(table: &TableDefinition, column: u32) -> Option<usize> {
    let columns = &table.columns;
    columns
        .binary_search_by_key(&column, |column| column.id)
        .ok()
}
