//! Going through the records of a table, page by page.

use crate::error::{Error, Result};
use crate::record::{Met, RecordReader};
use crate::schema::TableDefinition;
use crate::store::{RowPage, Slot};

/// Owned scan cursor state, independent of borrows into the database.
pub(crate) struct ScannerState {
    /// Why there is nothing to scan, until it has been reported once.
    refusal: Option<Error>,
    /// Index into the table's page list for the next page to open.
    next_page: usize,
    /// Currently open page and the number of its slots visited so far.
    open: Option<OpenPage>,
    /// Extension-page addresses already visited during this scan.
    met: Met,
    /// Set once a refusal stops the scan from reading any pages.
    stopped: bool,
}

/// A page with the number of its slots that were gone through.
struct OpenPage {
    page: RowPage,
    done: usize,
}

impl ScannerState {
    /// Creates a scan cursor for `definition`, or one that yields only `refusal`.
    pub(crate) fn new(definition: Option<&TableDefinition>, refusal: Option<Error>) -> Self {
        let refusal = refusal.or_else(|| definition.and_then(TableDefinition::unlisted));
        let stopped = refusal.is_some() || definition.is_none();
        Self {
            refusal,
            next_page: 0,
            open: None,
            met: Met::default(),
            stopped,
        }
    }

    /// Returns any pending refusal without opening pages.
    pub(crate) fn refusal_only<T>(&mut self) -> Option<Result<T>> {
        self.refusal.take().map(Err)
    }

    /// Advances to the next slot for which `read` produces a value.
    pub(crate) fn next<'db, T>(
        &mut self,
        reader: &RecordReader<'db>,
        read: impl Fn(&RecordReader<'db>, &RowPage, &Slot, &mut Met) -> Result<Option<T>>,
    ) -> Option<Result<T>> {
        if let Some(error) = self.refusal.take() {
            return Some(Err(error));
        }
        if self.stopped {
            return None;
        }
        let pages = reader.definition().pages();
        loop {
            if let Some(OpenPage { page, done }) = &mut self.open {
                while let Some(slot) = page.slots().get(*done) {
                    *done += 1;
                    let found = read(reader, page, slot, &mut self.met);
                    if let Some(found) = found.transpose() {
                        return Some(found);
                    }
                }
                self.open = None;
            }
            let &number = pages.get(self.next_page)?;
            self.next_page += 1;
            match reader.page(number) {
                Ok(page) => self.open = Some(OpenPage { page, done: 0 }),
                Err(problem) => {
                    return Some(Err(Error::TablePage {
                        table: reader.definition().name.clone(),
                        page: number,
                        problem,
                    }));
                }
            }
        }
    }
}

/// The occupied slots of a table's pages, in page and slot order.
pub(crate) struct Scanner<'db> {
    reader: RecordReader<'db>,
    state: ScannerState,
}

impl<'db> Scanner<'db> {
    /// The slots of the table that `reader` reads, or only `refusal`.
    pub(crate) fn new(reader: RecordReader<'db>, refusal: Option<Error>) -> Self {
        let state = ScannerState::new(Some(reader.definition()), refusal);
        Self { reader, state }
    }

    /// The next thing that `read` makes of a slot. A slot it makes nothing of
    /// is passed over. After an error the scan goes on with the next slot, or
    /// with the next page when a page cannot be opened.
    pub(crate) fn next<T>(
        &mut self,
        read: impl Fn(&RecordReader<'db>, &RowPage, &Slot, &mut Met) -> Result<Option<T>>,
    ) -> Option<Result<T>> {
        self.state.next(&self.reader, read)
    }
}
