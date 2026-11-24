//! A file's pages as it stores them, still scrambled.

use crate::error::{Error, FormatProblem, PageProblem, Result};
use crate::page_size::PAGE_SIZE;
use crate::store::Superblock;

/// The sizes of a page that SQL Anywhere has, the one that is read first.
const PAGE_SIZES: [usize; 5] = [PAGE_SIZE, 2048, 8192, 16384, 32768];

/// The bytes of a checksum, at the end of a page.
const CHECKSUM_SIZE: usize = 4;

/// Whether the engine ever wrote a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PageState {
    /// The page holds what the engine wrote, and its checksum fits.
    Written,
    /// The page is all zeros: the file has room for it, and nothing in it.
    Unwritten,
}

/// The bytes of a file, page by page.
pub(crate) struct StoredPages {
    bytes: Vec<u8>,
    count: u32,
}

impl StoredPages {
    /// Takes the bytes of a file whose first page is in order, with what that
    /// page says.
    pub(crate) fn new(bytes: Vec<u8>) -> Result<(Self, Superblock)> {
        let size = bytes.len() as u64;
        let not_pages = || FormatProblem::Size { size };
        let first: &[u8; PAGE_SIZE] = bytes.first_chunk().ok_or_else(not_pages)?;
        let superblock = Superblock::parse(first)?;
        check_page_size(&bytes)?;
        if !bytes.len().is_multiple_of(PAGE_SIZE) {
            return Err(not_pages().into());
        }
        let count =
            u32::try_from(bytes.len() / PAGE_SIZE).map_err(|_| FormatProblem::TooManyPages)?;
        let stored = Self { bytes, count };
        stored
            .check(0)
            .map_err(|problem| Error::Page { page: 0, problem })?;
        Ok((stored, superblock))
    }

    /// The size of the file in bytes.
    pub(crate) fn size(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// The number of pages in the file.
    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    /// The stored bytes of the page at position `number`.
    pub(crate) fn bytes(&self, number: u32) -> Result<&[u8; PAGE_SIZE], PageProblem> {
        let index = usize::try_from(number).map_err(|_| self.out_of_range())?;
        let start = index
            .checked_mul(PAGE_SIZE)
            .ok_or_else(|| self.out_of_range())?;
        let end = start
            .checked_add(PAGE_SIZE)
            .ok_or_else(|| self.out_of_range())?;
        let slice = self
            .bytes
            .get(start..end)
            .ok_or_else(|| self.out_of_range())?;
        slice.try_into().map_err(|_| self.out_of_range())
    }

    /// The checksum that every page stores.
    pub(crate) fn checksums(&self) -> impl Iterator<Item = [u8; CHECKSUM_SIZE]> {
        self.bytes.as_chunks::<PAGE_SIZE>().0.iter().map(|page| {
            let tail = &page[PAGE_SIZE - CHECKSUM_SIZE..];
            [tail[0], tail[1], tail[2], tail[3]]
        })
    }

    /// Checks the page at position `number` against its checksum.
    pub(crate) fn check(&self, number: u32) -> Result<PageState, PageProblem> {
        let page = self.bytes(number)?;
        if page.iter().all(|&byte| byte == 0) {
            return Ok(PageState::Unwritten);
        }
        let (body, stored_tail) = page.split_at(PAGE_SIZE - CHECKSUM_SIZE);
        let stored = u32::from_le_bytes([
            stored_tail[0],
            stored_tail[1],
            stored_tail[2],
            stored_tail[3],
        ]);
        let computed = crc32fast::hash(body);
        if stored == computed {
            Ok(PageState::Written)
        } else {
            Err(PageProblem::Checksum { stored, computed })
        }
    }

    fn out_of_range(&self) -> PageProblem {
        PageProblem::OutOfRange { count: self.count }
    }
}

/// Refuses a file whose first page fits its checksum in another size. The
/// checksum of a page is in its last bytes, so it fits in the page's own size.
fn check_page_size(file: &[u8]) -> Result<(), FormatProblem> {
    let mut sizes = PAGE_SIZES.into_iter();
    match sizes.find(|&size| file.get(..size).is_some_and(fits_checksum)) {
        Some(found) if found != PAGE_SIZE => Err(FormatProblem::PageSize { found }),
        _ => Ok(()),
    }
}

/// Whether the last bytes of `page` are the checksum of the others.
fn fits_checksum(page: &[u8]) -> bool {
    let parts = page.split_last_chunk();
    parts.is_some_and(|(rest, stored)| crc32fast::hash(rest).to_le_bytes() == *stored)
}
