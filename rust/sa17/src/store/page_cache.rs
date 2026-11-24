//! The pages of rows that were read last.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::store::RowPage;

/// The most pages a cache keeps.
const MOST_PAGES: usize = 32;

/// The last pages of rows that were decoded to read single records, the
/// latest first.
#[derive(Default)]
pub(crate) struct PageCache {
    pages: Mutex<VecDeque<Arc<RowPage>>>,
}

impl PageCache {
    /// The page with `number`, if it is here.
    pub(crate) fn get(&self, number: u32) -> Option<Arc<RowPage>> {
        let mut pages = self.pages.lock().expect(UNPOISONED);
        let position = pages.iter().position(|page| page.number() == number)?;
        let page = pages.remove(position)?;
        pages.push_front(Arc::clone(&page));
        Some(page)
    }

    /// Keeps `page`, and drops the page that was used longest ago when there
    /// are too many.
    pub(crate) fn put(&self, page: &Arc<RowPage>) {
        let mut pages = self.pages.lock().expect(UNPOISONED);
        pages.retain(|kept| kept.number() != page.number());
        pages.push_front(Arc::clone(page));
        pages.truncate(MOST_PAGES);
    }
}

const UNPOISONED: &str = "nothing that holds the lock panics";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_size::PAGE_SIZE;
    use crate::store::{Page, PageKind};

    /// An empty page of rows of table 7.
    fn page(number: u32) -> Arc<RowPage> {
        let mut bytes = Box::new([0; PAGE_SIZE]);
        bytes[..4].copy_from_slice(&number.to_le_bytes());
        bytes[16] = 4;
        bytes[24] = 7;
        let page = Page::new(number, bytes);
        Arc::new(RowPage::new(page, PageKind::Table, 7).unwrap())
    }

    #[test]
    fn the_pages_that_were_used_last_are_kept() {
        let cache = PageCache::default();
        let most = MOST_PAGES as u32;
        for number in 0..most {
            cache.put(&page(number));
        }
        // The first one is asked for, so the second one is the one to go.
        assert!(cache.get(0).is_some());
        cache.put(&page(most));
        let kept = |number| cache.get(number).map(|page| page.number());
        assert_eq!(kept(0), Some(0));
        assert_eq!(kept(1), None);
        assert!((2..=most).all(|number| kept(number) == Some(number)));
        // A page that is there already is kept once.
        for _ in 0..MOST_PAGES {
            cache.put(&page(most));
        }
        assert!((2..=most).all(|number| kept(number) == Some(number)));
    }
}
