//! The pages of a table.

use crate::error::PageListProblem;

/// The pages that the catalog gives a table, for its rows or as extensions.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PageSet {
    /// These, in ascending order.
    Listed(Vec<u32>),
    /// Whichever carry the table's identifier. One table's extension pages are
    /// listed nowhere: those of the table that has the lists.
    Unlisted,
    /// Nothing says which.
    Damaged(PageListProblem),
}

impl PageSet {
    /// The pages that are known.
    pub(crate) fn numbers(&self) -> &[u32] {
        match self {
            Self::Listed(numbers) => numbers,
            Self::Unlisted | Self::Damaged(_) => &[],
        }
    }

    /// Whether the page can be one of them.
    pub(crate) fn allows(&self, page: u32) -> bool {
        match self {
            Self::Listed(numbers) => numbers.binary_search(&page).is_ok(),
            Self::Unlisted => true,
            Self::Damaged(_) => false,
        }
    }

    /// What is wrong with the list.
    pub(crate) fn problem(&self) -> Option<PageListProblem> {
        match *self {
            Self::Damaged(problem) => Some(problem),
            Self::Listed(_) | Self::Unlisted => None,
        }
    }
}
