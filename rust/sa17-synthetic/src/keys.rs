//! The catalog tables of the keys and the indexes.

use crate::cell::Cell;
use crate::synthetic_database::{catalog, optional, required};
use crate::synthetic_table::{IndexOrder, SyntheticRow, SyntheticTable};

/// The number of the index behind a primary key.
const PRIMARY_INDEX: u32 = 0;

const PRIMARY_KEY: u8 = 1;
const FOREIGN_KEY: u8 = 2;
const SECONDARY: u8 = 3;

const UNIQUE_CONSTRAINT: u8 = 2;
const NOT_UNIQUE: u8 = 4;

/// The three catalog tables, with their rows.
pub(crate) struct KeyCatalogs {
    pub(crate) indexes: SyntheticTable,
    pub(crate) columns: SyntheticTable,
    pub(crate) foreign_keys: SyntheticTable,
}

/// One index to list: the columns it covers, and for a foreign key the
/// columns of the target that they hold the values of.
struct Listed<'table> {
    table: u32,
    number: u32,
    name: &'table str,
    category: u8,
    unique: u8,
    order: IndexOrder,
    columns: Vec<u32>,
    target_columns: Option<Vec<u32>>,
}

/// The catalog of the keys and the indexes that `tables` declare, where the
/// tables get identifiers from `first` on.
pub(crate) fn catalogs(first: u32, tables: &[SyntheticTable]) -> KeyCatalogs {
    let mut catalogs = empty();
    for (id, table) in (first..).zip(tables) {
        if !table.primary_key.is_empty() {
            catalogs.list(&Listed {
                table: id,
                number: PRIMARY_INDEX,
                name: &table.name,
                category: PRIMARY_KEY,
                unique: UNIQUE_CONSTRAINT,
                order: IndexOrder::Ascending,
                columns: numbers(table, &table.primary_key),
                target_columns: None,
            });
        }
        for (number, key) in (PRIMARY_INDEX + 1..).zip(&table.foreign_keys) {
            let (target, referenced) = (first..)
                .zip(tables)
                .find(|(_, other)| other.name == key.target)
                .unwrap_or_else(|| panic!("there is no table {}", key.target));
            let target_columns = numbers(referenced, &referenced.primary_key);
            assert_eq!(
                target_columns.len(),
                key.columns.len(),
                "the key {} has as many columns as the primary key it holds",
                key.name
            );
            catalogs.list(&Listed {
                table: id,
                number,
                name: &key.name,
                category: FOREIGN_KEY,
                unique: NOT_UNIQUE,
                order: IndexOrder::Ascending,
                columns: numbers(table, &key.columns),
                target_columns: Some(target_columns),
            });
            catalogs.reference(id, number, target);
        }
        let numbers_left = (PRIMARY_INDEX + 1..).skip(table.foreign_keys.len());
        for (number, index) in numbers_left.zip(&table.indexes) {
            catalogs.list(&Listed {
                table: id,
                number,
                name: &index.name,
                category: SECONDARY,
                unique: NOT_UNIQUE,
                order: index.order,
                columns: numbers(table, &index.columns),
                target_columns: None,
            });
        }
    }
    catalogs
}

impl KeyCatalogs {
    /// Adds an index and its columns.
    fn list(&mut self, index: &Listed<'_>) {
        self.indexes.push_row(SyntheticRow::new([
            Cell::unsigned_int(index.table),
            Cell::unsigned_int(index.number),
            Cell::unsigned_big_int(u64::from(index.table) << 16 | u64::from(index.number)),
            Cell::Null,
            Cell::small_int(0),
            Cell::tiny_int(index.category),
            Cell::tiny_int(index.unique),
            Cell::text(index.name),
            Cell::text("N"),
        ]));
        let order = match index.order {
            IndexOrder::Ascending => "A",
            IndexOrder::Descending => "D",
        };
        for (sequence, &column) in (0..).zip(&index.columns) {
            let target = index.target_columns.as_ref();
            let target = target.and_then(|columns| columns.get(sequence as usize));
            self.columns.push_row(SyntheticRow::new([
                Cell::unsigned_int(index.table),
                Cell::unsigned_int(index.number),
                Cell::small_int(sequence),
                Cell::unsigned_int(column),
                Cell::text(order),
                target.map_or(Cell::Null, |&column| Cell::unsigned_int(column)),
            ]));
        }
    }

    /// Adds the foreign key behind index `number` of `table`.
    fn reference(&mut self, table: u32, number: u32, target: u32) {
        self.foreign_keys.push_row(SyntheticRow::new([
            Cell::unsigned_int(table),
            Cell::unsigned_int(number),
            Cell::unsigned_int(target),
            Cell::unsigned_int(PRIMARY_INDEX),
            Cell::tiny_int(0),
            Cell::text("N"),
            Cell::text("N"),
        ]));
    }
}

fn empty() -> KeyCatalogs {
    KeyCatalogs {
        indexes: catalog(
            "ISYSIDX",
            vec![
                required("table_id", 21, 4),
                required("index_id", 21, 4),
                required("object_id", 23, 8),
                optional("phys_index_id", 21, 4),
                required("dbspace_id", 1, 2),
                required("index_category", 19, 1),
                required("unique", 19, 1),
                required("index_name", 8, 128),
                required("not_enforced", 8, 1),
            ],
        ),
        columns: catalog(
            "ISYSIDXCOL",
            vec![
                required("table_id", 21, 4),
                required("index_id", 21, 4),
                required("sequence", 1, 2),
                required("column_id", 21, 4),
                required("order", 8, 1),
                optional("primary_column_id", 21, 4),
            ],
        ),
        foreign_keys: catalog(
            "ISYSFKEY",
            vec![
                required("foreign_table_id", 21, 4),
                required("foreign_index_id", 21, 4),
                required("primary_table_id", 21, 4),
                required("primary_index_id", 21, 4),
                required("match_type", 19, 1),
                required("check_on_commit", 8, 1),
                required("nulls", 8, 1),
            ],
        ),
    }
}

/// The numbers of the columns of `table` called `names`.
fn numbers(table: &SyntheticTable, names: &[String]) -> Vec<u32> {
    let number = |name: &String| {
        let mut columns = (1..).zip(&table.columns);
        let found = columns.find(|(_, column)| column.name == *name);
        let (number, _) =
            found.unwrap_or_else(|| panic!("table {} has no column {name}", table.name));
        number
    };
    names.iter().map(number).collect()
}
