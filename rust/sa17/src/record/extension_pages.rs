//! The extension pages of a table, where moved rows and the pieces of long
//! values are.

use std::collections::HashSet;
use std::sync::Arc;

use crate::error::PageProblem;
use crate::row_address::RowAddress;
use crate::schema::TableDefinition;
use crate::store::{PageKind, PageStore, RowPage};

/// The records of extension pages that a scan has read. Each of them belongs to
/// one row, so a scan reads it once, and takes no longer than the file is.
#[derive(Default)]
pub(crate) struct Met(HashSet<RowAddress>);

impl Met {
    /// Whether the scan gets to the record for the first time.
    pub(crate) fn first(&mut self, address: RowAddress) -> bool {
        self.0.insert(address)
    }
}

/// The extension pages of one table.
#[derive(Clone, Copy)]
pub(crate) struct ExtensionPages<'db> {
    store: &'db PageStore,
    definition: &'db TableDefinition,
}

impl<'db> ExtensionPages<'db> {
    pub(crate) fn new(store: &'db PageStore, definition: &'db TableDefinition) -> Self {
        Self { store, definition }
    }

    /// The page at position `number`, when it is one of the table's.
    pub(crate) fn page(&self, number: u32) -> Result<Arc<RowPage>, PageProblem> {
        if !self.definition.extension_pages.allows(number) {
            return Err(PageProblem::Unlisted);
        }
        self.store
            .shared_rows(number, PageKind::Extension, self.definition.id)
    }
}
