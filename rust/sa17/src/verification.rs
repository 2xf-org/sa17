//! Checking a whole file.

use std::fmt;

use crate::catalog::SystemCatalog;
use crate::error::Error;
use crate::record::Rows;
use crate::schema::TableDefinition;
use crate::store::{PageState, PageStore};

/// A verification keeps at most this many problems. The counts go on.
const MOST_PROBLEMS: usize = 100;

/// What checking a whole file found.
#[derive(Clone, Debug)]
pub struct Verification {
    catalog_problems: u64,
    pages: u64,
    unwritten_pages: u64,
    damaged_pages: u64,
    tables: u64,
    differing_tables: u64,
    rows: u64,
    unreadable_rows: u64,
    unreadable_pages: u64,
    problems: Vec<VerificationProblem>,
}

/// One thing that is wrong with a file.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum VerificationProblem {
    /// A part of the catalog could not be read.
    Catalog(Error),
    /// A page could not be checked, or fails its checksum.
    Page(Error),
    /// Rows of a table could not be read.
    Rows {
        /// The table's name.
        table: String,
        /// How many rows.
        rows: u64,
        /// How many pages, whatever number of rows each of them has.
        pages: u64,
        /// Why the first of the rows or pages could not be read.
        first: Error,
    },
    /// A table whose pages could all be read holds another number of rows than
    /// the catalog records.
    RowCount {
        /// The table's name.
        table: String,
        /// The rows that are there, readable or not.
        found: u64,
        /// The number the catalog records.
        recorded: u64,
    },
}

impl fmt::Display for VerificationProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(error) | Self::Page(error) => error.fmt(formatter),
            Self::Rows {
                table,
                rows,
                pages,
                first,
            } => write!(
                formatter,
                "{table}: {rows} rows and {pages} pages of rows could not be read; the first: {first}"
            ),
            Self::RowCount {
                table,
                found,
                recorded,
            } => write!(
                formatter,
                "{table}: found {found} rows, the catalog records {recorded}"
            ),
        }
    }
}

impl Verification {
    /// Checks every page of `store` and reads every row of `catalog`.
    pub(crate) fn new(store: &PageStore, catalog: &SystemCatalog) -> Self {
        let mut verification = Self {
            catalog_problems: 0,
            pages: 0,
            unwritten_pages: 0,
            damaged_pages: 0,
            tables: 0,
            differing_tables: 0,
            rows: 0,
            unreadable_rows: 0,
            unreadable_pages: 0,
            problems: Vec::new(),
        };
        for problem in catalog.problems() {
            verification.catalog_problems += 1;
            verification.keep(VerificationProblem::Catalog(problem.clone()));
        }
        for number in 0..store.page_count() {
            verification.check_page(store, number);
        }
        for table in catalog.stored() {
            verification.read_table(store, table);
        }
        verification
    }

    fn check_page(&mut self, store: &PageStore, number: u32) {
        self.pages += 1;
        match store.check(number) {
            Ok(PageState::Written) => {}
            Ok(PageState::Unwritten) => self.unwritten_pages += 1,
            Err(error) => {
                self.damaged_pages += 1;
                self.keep(VerificationProblem::Page(error));
            }
        }
    }

    fn read_table(&mut self, store: &PageStore, table: &TableDefinition) {
        let (mut read, mut rows, mut pages, mut first) = (0, 0, 0, None);
        for row in Rows::all(store, table) {
            let Err(error) = row else {
                read += 1;
                continue;
            };
            match error {
                Error::Row { .. } => rows += 1,
                _ => pages += 1,
            }
            first.get_or_insert(error);
        }
        self.tables += 1;
        self.rows += read;
        self.unreadable_rows += rows;
        self.unreadable_pages += pages;
        if let Some(first) = first {
            self.keep(VerificationProblem::Rows {
                table: table.name().to_owned(),
                rows,
                pages,
                first,
            });
        }
        // Nothing says how many rows a page has that could not be read.
        let (found, recorded) = (read + rows, table.row_count());
        if pages == 0 && found != recorded {
            self.differing_tables += 1;
            self.keep(VerificationProblem::RowCount {
                table: table.name().to_owned(),
                found,
                recorded,
            });
        }
    }

    fn keep(&mut self, problem: VerificationProblem) {
        if self.problems.len() < MOST_PROBLEMS {
            self.problems.push(problem);
        }
    }

    /// Whether nothing is wrong with the file.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.catalog_problems == 0
            && self.damaged_pages == 0
            && self.unreadable_rows == 0
            && self.unreadable_pages == 0
            && self.differing_tables == 0
    }

    /// The parts of the catalog that could not be read, as
    /// [`Database::catalog_problems`](crate::Database::catalog_problems) has
    /// them.
    #[must_use]
    pub fn catalog_problems(&self) -> u64 {
        self.catalog_problems
    }

    /// The number of pages checked.
    #[must_use]
    pub fn pages(&self) -> u64 {
        self.pages
    }

    /// The pages that were never written.
    #[must_use]
    pub fn unwritten_pages(&self) -> u64 {
        self.unwritten_pages
    }

    /// The pages that fail their checksum.
    #[must_use]
    pub fn damaged_pages(&self) -> u64 {
        self.damaged_pages
    }

    /// The number of tables read.
    #[must_use]
    pub fn tables(&self) -> u64 {
        self.tables
    }

    /// The tables whose pages could all be read, and whose rows are not as
    /// many as the catalog records.
    #[must_use]
    pub fn differing_tables(&self) -> u64 {
        self.differing_tables
    }

    /// The number of rows read.
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// The rows that could not be read, on pages that could be.
    #[must_use]
    pub fn unreadable_rows(&self) -> u64 {
        self.unreadable_rows
    }

    /// The pages of rows that could not be read, whatever number of rows each
    /// of them has. A table whose list of pages could not be read counts as one.
    #[must_use]
    pub fn unreadable_pages(&self) -> u64 {
        self.unreadable_pages
    }

    /// The first hundred things that are wrong: the catalog's, then pages,
    /// then tables.
    #[must_use]
    pub fn problems(&self) -> &[VerificationProblem] {
        &self.problems
    }
}
