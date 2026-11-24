//! The two catalog tables as SQL Anywhere 17 lays them out, which is what
//! reading the catalog starts from.

use crate::catalog::Fields;
use crate::error::{Error, RowProblem};
use crate::record::{Columns, RecordReader, Row};
use crate::schema::{Column, PageSet, TableDefinition, TableKind};
use crate::store::PageStore;
use crate::value::Value;

/// No column of the two tables is declared compressed, so none of their values
/// is taken out of compression.
pub(crate) const LONGEST_VALUE: u32 = 0;

/// The identifier of the user that owns the catalog.
pub(crate) const SYS: u32 = 0;

/// A catalog table as this reader assumes it before the catalog is read.
pub(crate) struct Bootstrap {
    pub(crate) id: u32,
    pub(crate) name: &'static str,
    columns: &'static [BootstrapColumn],
    /// The columns that can be kept outside their row.
    long: &'static [&'static str],
}

struct BootstrapColumn {
    name: &'static str,
    domain: u16,
    width: u32,
    nullable: bool,
}

const fn required(name: &'static str, domain: u16, width: u32) -> BootstrapColumn {
    BootstrapColumn {
        name,
        domain,
        width,
        nullable: false,
    }
}

const fn optional(name: &'static str, domain: u16, width: u32) -> BootstrapColumn {
    BootstrapColumn {
        name,
        domain,
        width,
        nullable: true,
    }
}

/// The columns of `ISYSTAB` with the pages of a table.
pub(crate) const TABLE_PAGES: &str = "tab_page_list";
pub(crate) const EXTENSION_PAGES: &str = "ext_page_list";

/// The columns of `ISYSTABCOL` with the default of a column, and with what it
/// holds in rows that are older.
pub(crate) const DEFAULT: &str = "default";
pub(crate) const ABSENT_VALUE: &str = "nonmaterialized_value";

pub(crate) const TABLES: Bootstrap = Bootstrap {
    id: 1,
    name: "ISYSTAB",
    columns: &[
        required("table_id", 21, 4),
        required("dbspace_id", 1, 2),
        required("count", 23, 8),
        required("creator", 21, 4),
        required("table_page_count", 2, 4),
        required("ext_page_count", 2, 4),
        required("commit_action", 2, 4),
        required("share_type", 2, 4),
        required("object_id", 23, 8),
        required("last_modified_at", 13, 8),
        required("table_name", 8, 128),
        required("table_type", 19, 1),
        required("replicate", 8, 1),
        required("server_type", 19, 1),
        optional(TABLE_PAGES, 31, 32767),
        optional(EXTENSION_PAGES, 31, 32767),
        optional("pct_free", 21, 4),
        optional("clustered_index_id", 21, 4),
        required("encrypted", 8, 1),
        required("last_modified_tsn", 23, 8),
        required("current_schema", 21, 4),
    ],
    long: &[TABLE_PAGES, EXTENSION_PAGES],
};

pub(crate) const COLUMNS: Bootstrap = Bootstrap {
    id: 2,
    name: "ISYSTABCOL",
    columns: &[
        required("table_id", 21, 4),
        required("column_id", 21, 4),
        required("domain_id", 1, 2),
        required("nulls", 8, 1),
        required("width", 21, 4),
        required("scale", 1, 2),
        required("object_id", 23, 8),
        required("max_identity", 20, 8),
        required("column_name", 8, 128),
        optional(DEFAULT, 10, 32767),
        optional("user_type", 1, 2),
        required("column_type", 8, 1),
        required("compressed", 19, 1),
        required("collect_stats", 19, 1),
        optional("inline_max", 1, 2),
        optional("inline_long", 1, 2),
        optional("lob_index", 19, 1),
        optional(ABSENT_VALUE, 12, 32767),
        required("start_schema", 21, 4),
    ],
    long: &[DEFAULT, ABSENT_VALUE],
};

impl Bootstrap {
    /// The table with rows on `pages`, as this layout describes it.
    pub(crate) fn table(&self, pages: PageSet, extension_pages: PageSet) -> TableDefinition {
        let numbered = (1..).zip(self.columns);
        let columns: Vec<Column> = numbered
            .map(|(id, column)| Column {
                id,
                name: column.name.to_owned(),
                domain: column.domain,
                width: column.width,
                scale: 0,
                nullable: column.nullable,
                default: None,
                first_schema: 0,
                absent: Some(Value::Null),
            })
            .collect();
        TableDefinition {
            id: self.id,
            name: self.name.to_owned(),
            kind: TableKind::Base,
            owner_id: SYS,
            owner: String::new(),
            system: true,
            columns: columns.into(),
            row_count: 0,
            schema: 0,
            pages,
            extension_pages,
            primary_key: Vec::new(),
            foreign_keys: Vec::new(),
            indexes: Vec::new(),
        }
    }

    /// The row of `table`, this one, that could not be read for `error`, when
    /// it was for one of its long values: without that one, or without any.
    pub(crate) fn without_long_values(
        &self,
        store: &PageStore,
        table: &TableDefinition,
        error: &Error,
    ) -> Option<Row> {
        let Error::Row {
            address,
            problem: RowProblem::LongValue { .. },
            ..
        } = *error
        else {
            return None;
        };
        let each = self.long.chunks(1);
        let mut left_out = each.chain([self.long]);
        left_out.find_map(|left_out| {
            let names = self.columns.iter().map(|column| column.name);
            let columns = Columns::new(table, names.filter(|name| !left_out.contains(name)));
            let reader = RecordReader::new(store, table, columns.ok()?, LONGEST_VALUE);
            reader.at(address).ok()?
        })
    }

    /// The fields of a row of this catalog table.
    pub(crate) fn fields<'row>(&self, row: &'row Row) -> Fields<'row> {
        Fields::new(self.name, row)
    }

    /// Whether the catalog describes this table the way the bootstrap does.
    pub(crate) fn matches(&self, described: &[Column]) -> bool {
        described.len() == self.columns.len()
            && (1..)
                .zip(self.columns)
                .zip(described)
                .all(|((id, expected), column)| {
                    column.id == id
                        && column.name == expected.name
                        && column.domain == expected.domain
                        && column.nullable == expected.nullable
                        && column.first_schema == 0
                })
    }
}
