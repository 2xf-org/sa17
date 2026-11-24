//! A file's pages, with the key that decodes them.

use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::cipher::StoreKey;
use crate::error::{Error, PageProblem, Result};
use crate::identity::Identity;
use crate::open_options::OpenOptions;
use crate::page_size::PAGE_SIZE;
use crate::store::key_choice;
use crate::store::{
    Page, PageCache, PageKind, PageState, RowPage, StoredPages, Superblock, TextEncoding,
};

/// The pages of a database file, decoded on request.
///
/// The file's bytes are held in memory and never change.
pub struct PageStore {
    stored: StoredPages,
    superblock: Superblock,
    key: StoreKey,
    encoding: TextEncoding,
    longest_value: u32,
    identity: OnceLock<Identity>,
    cache: PageCache,
}

impl PageStore {
    /// Opens the file at `path`, reads it into memory and finds its key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Read`] when the file cannot be read, and the errors
    /// of [`Self::from_bytes`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(path, &OpenOptions::new())
    }

    /// Opens the file at `path`, as `options` say.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Read`] when the file cannot be read, and the errors
    /// of [`Self::from_bytes_with`].
    pub fn open_with(path: impl AsRef<Path>, options: &OpenOptions) -> Result<Self> {
        Self::from_bytes_with(fs::read(path)?, options)
    }

    /// Takes the bytes of a file and finds its key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Format`] for bytes that are not a SQL Anywhere 17
    /// database, [`Error::Page`] for a damaged first page, [`Error::NoKey`] or
    /// [`Error::AmbiguousKey`] when the pages do not pin one key, and
    /// [`Error::UnsupportedEncoding`] for text in an encoding this reader does
    /// not decode.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        Self::from_bytes_with(bytes, &OpenOptions::new())
    }

    /// Takes the bytes of a file, as `options` say.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::from_bytes`], and [`Error::WrongKey`] when
    /// the key of `options` does not fit the pages.
    pub fn from_bytes_with(bytes: Vec<u8>, options: &OpenOptions) -> Result<Self> {
        let (stored, superblock) = StoredPages::new(bytes)?;
        let key = key_choice::choose(&stored, &superblock, options.key)?;
        let first = stored.bytes(0).map_err(page_error(0))?;
        let encoding = TextEncoding::detect(first)?;
        Ok(Self {
            stored,
            superblock,
            key,
            encoding,
            longest_value: options.longest_value,
            identity: OnceLock::new(),
            cache: PageCache::default(),
        })
    }

    /// The size of the file in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.stored.size()
    }

    /// The bytes in a page of the file.
    #[must_use]
    pub fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    /// The number of pages in the file. The ones from
    /// [`Superblock::database_page_count`] on are not the database's.
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.stored.count()
    }

    /// What the first page says about the file.
    #[must_use]
    pub fn superblock(&self) -> &Superblock {
        &self.superblock
    }

    /// The key that decodes the file's pages.
    #[must_use]
    pub fn key(&self) -> StoreKey {
        self.key
    }

    /// What tells the file, and the state it is in, from others. It is worked
    /// out the first time.
    #[must_use]
    pub fn identity(&self) -> Identity {
        *self.identity.get_or_init(|| {
            let first = self.stored.bytes(0).map_or(&[][..], |first| &first[..]);
            let checksums = self.stored.checksums();
            Identity::new(self.page_count(), self.superblock.lsn(), first, checksums)
        })
    }

    /// The encoding of the file's character columns.
    #[must_use]
    pub fn text_encoding(&self) -> TextEncoding {
        self.encoding
    }

    /// The bytes of the page at position `number` as the file stores them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Page`] when the file has no such page.
    pub fn stored(&self, number: u32) -> Result<&[u8]> {
        let stored = self.stored.bytes(number).map_err(page_error(number))?;
        Ok(&stored[..])
    }

    /// Checks the page at position `number` against its checksum.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Page`] when they differ, and when the file has no such
    /// page.
    pub fn check(&self, number: u32) -> Result<PageState> {
        self.stored.check(number).map_err(page_error(number))
    }

    /// Decodes the page at position `number`, after its checksum passes.
    ///
    /// The first page is not scrambled, and a page that was never written
    /// holds nothing, so both come back as they are stored.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Page`] when the checksum does not pass, and when the
    /// file has no such page.
    pub fn page(&self, number: u32) -> Result<Page> {
        self.decoded(number).map_err(page_error(number))
    }

    /// The same page, for a reader that says itself what the page was for.
    pub(crate) fn decoded(&self, number: u32) -> Result<Page, PageProblem> {
        let state = self.stored.check(number)?;
        let stored = self.stored.bytes(number)?;
        let decoded = if number == 0 || state == PageState::Unwritten {
            Box::new(*stored)
        } else {
            self.key.page_key(number).restore(stored)
        };
        Ok(Page::new(number, decoded))
    }

    /// The page of rows at position `number`, which must be of `kind` and
    /// belong to the table with identifier `table_id`.
    pub(crate) fn rows(
        &self,
        number: u32,
        kind: PageKind,
        table_id: u32,
    ) -> Result<RowPage, PageProblem> {
        RowPage::new(self.decoded(number)?, kind, table_id)
    }

    /// The same page, kept for the next one who asks for it.
    pub(crate) fn shared_rows(
        &self,
        number: u32,
        kind: PageKind,
        table_id: u32,
    ) -> Result<Arc<RowPage>, PageProblem> {
        if let Some(kept) = self.cache.get(number) {
            kept.page().expect(kind, table_id)?;
            return Ok(kept);
        }
        let page = Arc::new(self.rows(number, kind, table_id)?);
        self.cache.put(&page);
        Ok(page)
    }

    /// The most bytes of a value that was stored compressed.
    pub(crate) fn longest_value(&self) -> u32 {
        self.longest_value
    }
}

/// The error of a problem with the page at position `page`.
fn page_error(page: u32) -> impl Fn(PageProblem) -> Error {
    move |problem| Error::Page { page, problem }
}

impl fmt::Debug for PageStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageStore")
            .field("size", &self.size())
            .field("superblock", &self.superblock)
            .field("encoding", &self.encoding)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use sa17_synthetic::{Cell, SyntheticColumn, SyntheticDatabase};
    use sa17_synthetic::{SyntheticRow, SyntheticTable};

    use super::*;

    // A page that was read for one table is not another's because it is at hand.
    #[test]
    fn a_kept_page_is_checked_as_a_new_one_is() {
        let table = SyntheticTable::new("seeds")
            .column(SyntheticColumn::new("id", 2))
            .row(SyntheticRow::new([Cell::integer(1)]));
        let file = SyntheticDatabase::new().table(table).build();
        let store = PageStore::from_bytes(file).unwrap();
        let read = |kind, table_id| store.shared_rows(11, kind, table_id).err();
        for _ in 0..2 {
            assert_eq!(
                read(PageKind::Table, 1001),
                Some(PageProblem::WrongTable { table_id: 1000 })
            );
            assert!(matches!(
                read(PageKind::Extension, 1000),
                Some(PageProblem::WrongKind { .. })
            ));
            assert_eq!(read(PageKind::Table, 1000), None);
        }
    }
}
