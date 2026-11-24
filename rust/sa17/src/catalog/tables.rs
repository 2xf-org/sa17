//! Reading `ISYSTAB`, which has one row per table.

use crate::catalog::Problems;
use crate::catalog::bootstrap::{COLUMNS, EXTENSION_PAGES, TABLE_PAGES, TABLES};
use crate::error::{CatalogProblem, Error, PageListProblem, Result};
use crate::record::{Row, Rows};
use crate::schema::{PageSet, TableKind};
use crate::store::page_list;
use crate::store::{PageKind, PageStore};

/// The page that lists the pages of `ISYSTAB`. The table cannot list its own
/// pages in its own rows, so the engine keeps that one list at a fixed place.
const TABLE_PAGE_LIST: u32 = 10;

/// What `ISYSTAB` says about a table.
pub(crate) struct TableRow {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) kind: TableKind,
    pub(crate) owner_id: u32,
    pub(crate) row_count: u64,
    pub(crate) schema: u32,
    pub(crate) pages: PageSet,
    pub(crate) extension_pages: PageSet,
}

/// A row of `ISYSTAB` with its two lists of pages as it stores them.
struct StoredRow {
    table: TableRow,
    pages: StoredList,
    extension_pages: StoredList,
}

/// A list of pages as a row stores it, where `NULL` is the empty one.
type StoredList = Result<Vec<u8>, PageListProblem>;

/// Gives out the pages of a file, each of which belongs to one table.
struct PageLists {
    page_count: u32,
    /// How many are not given out yet.
    left: u32,
}

impl PageLists {
    /// The pages that a stored list names.
    fn read(&mut self, stored: &[u8]) -> Result<Vec<u32>, PageListProblem> {
        let pages = page_list::pages(stored, self.page_count, self.left)?;
        // There are no more of them than are left.
        self.left -= pages.len() as u32;
        Ok(pages)
    }

    fn set(&mut self, stored: StoredList) -> PageSet {
        match stored {
            Ok(empty) if empty.is_empty() => PageSet::Listed(Vec::new()),
            Ok(stored) => match self.read(&stored) {
                Ok(pages) => PageSet::Listed(pages),
                Err(problem) => PageSet::Damaged(problem),
            },
            Err(problem) => PageSet::Damaged(problem),
        }
    }
}

/// Every table, ordered by identifier. What cannot be read is added to
/// `problems`, and costs its own table or that table's rows.
pub(crate) fn read(store: &PageStore, problems: &mut Problems) -> Result<Vec<TableRow>> {
    let mut lists = PageLists {
        page_count: store.page_count(),
        left: store.page_count(),
    };
    let own = own_pages(store, &mut lists)?;
    let mut rows = rows(store, &own, problems);
    rows.sort_by_key(|row| row.table.id);
    refuse_repeated(&mut rows, problems)?;

    // The engine's own tables have the lowest identifiers, so no list of
    // another table can take their pages.
    let mut tables = Vec::with_capacity(rows.len());
    for StoredRow {
        mut table,
        pages,
        extension_pages,
    } in rows
    {
        if table.id == TABLES.id {
            table.pages = PageSet::Listed(own.clone());
            table.extension_pages = PageSet::Unlisted;
        } else {
            table.pages = lists.set(pages);
            table.extension_pages = lists.set(extension_pages);
        }
        for set in [&table.pages, &table.extension_pages] {
            if let Some(problem) = set.problem() {
                problems.add(Error::PageList {
                    table: table.name.clone(),
                    problem,
                });
            }
        }
        tables.push(table);
    }
    Ok(tables)
}

/// Every row of `ISYSTAB`, which is on the pages `own`, that can be read.
fn rows(store: &PageStore, own: &[u32], problems: &mut Problems) -> Vec<StoredRow> {
    let catalog = TABLES.table(PageSet::Listed(own.to_vec()), PageSet::Unlisted);
    let mut rows = Vec::new();
    for row in Rows::all(store, &catalog) {
        let row = match row {
            Ok(row) => Some(row),
            Err(error) => {
                let without_lists = TABLES.without_long_values(store, &catalog, &error);
                problems.add(error);
                without_lists
            }
        };
        match row.as_ref().map(stored_row) {
            Some(Ok(row)) => rows.push(row),
            Some(Err(error)) => problems.add(error),
            None => {}
        }
    }
    rows
}

/// The pages of `ISYSTAB` itself.
fn own_pages(store: &PageStore, lists: &mut PageLists) -> Result<Vec<u32>> {
    let page = store.page(TABLE_PAGE_LIST)?;
    let list = page.expect(PageKind::PageList, TABLES.id);
    let list = list.and_then(|()| page.page_list());
    let list = list.map_err(|problem| Error::Page {
        page: TABLE_PAGE_LIST,
        problem,
    })?;
    lists.read(list).map_err(|problem| Error::PageList {
        table: TABLES.name.to_owned(),
        problem,
    })
}

fn stored_row(row: &Row) -> Result<StoredRow> {
    let fields = TABLES.fields(row);
    let stored = |column| match fields.optional_bytes(column) {
        Ok(stored) => Ok(stored.unwrap_or_default().to_vec()),
        Err(_) => Err(PageListProblem::Unreadable),
    };
    Ok(StoredRow {
        table: TableRow {
            id: fields.narrow("table_id")?,
            name: fields.text("table_name")?.to_owned(),
            kind: TableKind::from_catalog(fields.unsigned("table_type")?),
            owner_id: fields.narrow("creator")?,
            row_count: fields.unsigned("count")?,
            schema: fields.narrow("current_schema")?,
            pages: PageSet::Listed(Vec::new()),
            extension_pages: PageSet::Listed(Vec::new()),
        },
        pages: stored(TABLE_PAGES),
        extension_pages: stored(EXTENSION_PAGES),
    })
}

/// Takes out the tables that share an identifier. Nothing says which of them
/// has the pages and the columns of that identifier, so there is no catalog
/// when it is one of the catalog's own.
fn refuse_repeated(rows: &mut Vec<StoredRow>, problems: &mut Problems) -> Result<()> {
    let mut repeated: Vec<u32> = Vec::new();
    let neighbours = rows.iter().zip(rows.iter().skip(1));
    for (row, next) in neighbours {
        let id = row.table.id;
        if id == next.table.id && repeated.last() != Some(&id) {
            repeated.push(id);
        }
    }
    rows.retain(|row| repeated.binary_search(&row.table.id).is_err());
    for table_id in repeated {
        let problem = CatalogProblem::RepeatedTable { table_id };
        if [TABLES.id, COLUMNS.id].contains(&table_id) {
            return Err(problem.into());
        }
        problems.add(problem);
    }
    Ok(())
}
