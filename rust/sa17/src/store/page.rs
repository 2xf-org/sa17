//! A decoded page and the rows in it.
//!
//! Every page starts with the same 28 bytes: its number, its type and LSN, a
//! structure byte, a kind byte, and for row pages the slot count and the table
//! that owns the page. A row page then lists one 16-bit offset per slot, and
//! the rows fill the page from the end towards that list.

use std::fmt;
use std::ops::Range;

use crate::error::{Error, PageProblem, Result};
use crate::page_size::PAGE_SIZE;
use crate::row_address::RowAddress;
use crate::store::Record;

/// Where the slot directory starts. Slot offsets count from here too.
const DIRECTORY: usize = 28;

/// Where the row area ends and the checksum starts.
const ROW_AREA_END: usize = PAGE_SIZE - 4;

/// The structure byte of pages that hold rows.
const ROW_STRUCTURE: u8 = 4;

/// The structure byte of the page that lists the table catalog's own pages.
const PAGE_LIST_STRUCTURE: u8 = 20;

/// Kind bytes from here up mark pages that hold forwarded rows and the
/// pieces of long values, not a table's own rows.
const FIRST_EXTENSION_KIND: u8 = 0x10;

/// What a page holds, as far as this reader tells pages apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PageKind {
    /// Rows of the table that owns the page.
    Table,
    /// Forwarded rows and pieces of long values of the table that owns the page.
    Extension,
    /// The serialized list of the table catalog's own pages.
    PageList,
    /// Anything else: indexes, allocation maps, free pages.
    Other,
}

/// One page of the file, decoded.
#[derive(Clone, PartialEq, Eq)]
pub struct Page {
    number: u32,
    bytes: Box<[u8; PAGE_SIZE]>,
}

/// The bytes a slot's row may occupy: from its start to the next row's start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) index: u16,
    pub(crate) bytes: Range<usize>,
}

impl Page {
    pub(crate) fn new(number: u32, bytes: Box<[u8; PAGE_SIZE]>) -> Self {
        Self { number, bytes }
    }

    /// The page's position in the file.
    #[must_use]
    pub fn number(&self) -> u32 {
        self.number
    }

    /// The decoded bytes. The last four are the checksum of the stored page.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    /// The page number in the header. It differs from [`Self::number`] on
    /// checkpoint copies and on pages that were never written. The first page
    /// of a file has no header: this and the other fields of one are some of
    /// its own bytes there.
    #[must_use]
    pub fn stored_number(&self) -> u32 {
        self.u32_at(0)
    }

    /// The page's type, a letter that the engine keeps in the clear.
    #[must_use]
    pub fn page_type(&self) -> u8 {
        self.bytes[6]
    }

    /// The log sequence number of the page's last write.
    #[must_use]
    pub fn lsn(&self) -> u32 {
        self.u32_at(8)
    }

    /// What the page holds.
    #[must_use]
    pub fn kind(&self) -> PageKind {
        match (self.bytes[16], self.bytes[17]) {
            (ROW_STRUCTURE, kind) if kind < FIRST_EXTENSION_KIND => PageKind::Table,
            (ROW_STRUCTURE, _) => PageKind::Extension,
            (PAGE_LIST_STRUCTURE, _) => PageKind::PageList,
            _ => PageKind::Other,
        }
    }

    /// The identifier of the table that a page of rows belongs to.
    #[must_use]
    pub fn table_id(&self) -> Option<u32> {
        self.holds_rows().then(|| self.u32_at(24))
    }

    /// The number of slots on a page of rows, empty ones included.
    #[must_use]
    pub fn slot_count(&self) -> Option<u16> {
        self.holds_rows().then(|| self.u16_at(22))
    }

    /// The records of a page of rows, in the order of their slots. One that
    /// does not fit its slot is an `Err` item.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Page`] when this is no page of rows, and when its slot
    /// directory is damaged.
    pub fn records(&self) -> Result<impl Iterator<Item = Result<Record<'_>>>> {
        let slots = self.slots().map_err(|problem| self.error(problem))?;
        Ok(slots.into_iter().map(|slot| self.slot_record(&slot)))
    }

    /// The record in `slot` of a page of rows, or `None` when the slot is
    /// empty or past the last one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Page`] when this is no page of rows, when its slot
    /// directory is damaged, and when the record does not fit its slot.
    pub fn record(&self, slot: u16) -> Result<Option<Record<'_>>> {
        let slots = self.slots().map_err(|problem| self.error(problem))?;
        let found = slots.iter().find(|found| found.index == slot);
        found.map(|slot| self.slot_record(slot)).transpose()
    }

    fn slot_record(&self, slot: &Slot) -> Result<Record<'_>> {
        let address = RowAddress::new(self.number, slot.index);
        let bytes = record(self.slot_bytes(slot));
        let bytes = bytes.ok_or_else(|| self.error(PageProblem::Record { slot: slot.index }))?;
        Ok(Record::new(address, bytes))
    }

    /// How many records of a page of rows fill their slots exactly, or 1 for a
    /// page list that ends where it says. What a wrong key decodes almost never
    /// does.
    pub(crate) fn exact_records(&self) -> usize {
        match self.kind() {
            PageKind::Table | PageKind::Extension => {
                let slots = self.slots().unwrap_or_default();
                let exact = slots.iter().map(|slot| self.slot_bytes(slot));
                exact
                    .filter(|bytes| record(bytes).is_some_and(|record| record.len() == bytes.len()))
                    .count()
            }
            PageKind::PageList => usize::from(self.page_list().is_ok()),
            PageKind::Other => 0,
        }
    }

    fn holds_rows(&self) -> bool {
        matches!(self.kind(), PageKind::Table | PageKind::Extension)
    }

    /// Checks that this is the current copy of a page of `kind` that the table
    /// with identifier `table_id` owns.
    pub(crate) fn expect(&self, kind: PageKind, table_id: u32) -> Result<(), PageProblem> {
        if self.stored_number() != self.number {
            return Err(PageProblem::WrongNumber {
                stored: self.stored_number(),
            });
        }
        if self.kind() != kind {
            return Err(PageProblem::WrongKind {
                expected: match kind {
                    PageKind::Table => "a table page",
                    PageKind::Extension => "an extension page",
                    PageKind::PageList => "a page list",
                    PageKind::Other => "a page of another kind",
                },
            });
        }
        match self.table_id() {
            Some(found) if found != table_id => Err(PageProblem::WrongTable { table_id: found }),
            _ => Ok(()),
        }
    }

    /// The bytes that the row in `slot` may occupy.
    pub(crate) fn slot_bytes(&self, slot: &Slot) -> &[u8] {
        &self.bytes[slot.bytes.clone()]
    }

    /// The occupied slots of a page of rows, in slot order.
    pub(crate) fn slots(&self) -> Result<Vec<Slot>, PageProblem> {
        let Some(slot_count) = self.slot_count() else {
            return Err(PageProblem::WrongKind {
                expected: "a page of rows",
            });
        };
        let count = usize::from(slot_count);
        let rows_start = DIRECTORY + 2 * count;
        if rows_start > ROW_AREA_END {
            return Err(PageProblem::Directory);
        }
        let mut starts = Vec::with_capacity(count);
        for index in 0..slot_count {
            let offset = usize::from(self.u16_at(DIRECTORY + 2 * usize::from(index)));
            // A zero offset is an empty slot.
            if offset == 0 {
                continue;
            }
            let start = DIRECTORY + offset;
            if !(rows_start..ROW_AREA_END).contains(&start) {
                return Err(PageProblem::Directory);
            }
            starts.push((start, index));
        }
        // A row ends where the next one on the page starts.
        starts.sort_unstable();
        let mut slots = Vec::with_capacity(starts.len());
        for (position, &(start, index)) in starts.iter().enumerate() {
            let end = starts
                .get(position + 1)
                .map_or(ROW_AREA_END, |&(next, _)| next);
            if end == start {
                return Err(PageProblem::Directory);
            }
            slots.push(Slot {
                index,
                bytes: start..end,
            });
        }
        slots.sort_unstable_by_key(|slot| slot.index);
        Ok(slots)
    }

    /// The bytes of a page list: they follow the slot count's place, and the
    /// header word that otherwise counts rows holds where they end.
    pub(crate) fn page_list(&self) -> Result<&[u8], PageProblem> {
        const STREAM: usize = 24;
        let end = usize::from(self.u16_at(18));
        // A link to a second page would sit before the stream. None of the
        // files this reader was built from has one.
        if self.u32_at(20) != 0 || !(STREAM..=ROW_AREA_END).contains(&end) {
            return Err(PageProblem::Directory);
        }
        Ok(&self.bytes[STREAM..end])
    }

    fn error(&self, problem: PageProblem) -> Error {
        Error::Page {
            page: self.number,
            problem,
        }
    }

    fn u16_at(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.bytes[offset], self.bytes[offset + 1]])
    }

    fn u32_at(&self, offset: usize) -> u32 {
        let low = u32::from(self.u16_at(offset));
        let high = u32::from(self.u16_at(offset + 2));
        high << 16 | low
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Page")
            .field("number", &self.number)
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

/// A slot's bytes cut to the record's stored length. A record starts with a
/// 16-bit length and a flags byte; a slot may be longer than its record.
pub(crate) fn record(slot: &[u8]) -> Option<&[u8]> {
    let length = usize::from(u16::from_le_bytes(*slot.first_chunk()?));
    if length < 3 {
        return None;
    }
    slot.get(..length)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(number: u32, fill: impl FnOnce(&mut [u8; PAGE_SIZE])) -> Page {
        let mut bytes = Box::new([0; PAGE_SIZE]);
        bytes[..4].copy_from_slice(&number.to_le_bytes());
        bytes[16] = ROW_STRUCTURE;
        bytes[17] = 3;
        bytes[24] = 7;
        fill(&mut bytes);
        Page::new(number, bytes)
    }

    fn put_slots(bytes: &mut [u8; PAGE_SIZE], offsets: &[u16]) {
        bytes[22..24].copy_from_slice(&(offsets.len() as u16).to_le_bytes());
        for (index, offset) in offsets.iter().enumerate() {
            let at = DIRECTORY + 2 * index;
            bytes[at..at + 2].copy_from_slice(&offset.to_le_bytes());
        }
    }

    #[test]
    fn a_row_ends_where_the_next_row_starts() {
        let page = page(5, |bytes| put_slots(bytes, &[4000, 0, 3900]));
        let slots = page.slots().unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(
            slots[0],
            Slot {
                index: 0,
                bytes: 4028..ROW_AREA_END
            }
        );
        assert_eq!(
            slots[1],
            Slot {
                index: 2,
                bytes: 3928..4028
            }
        );
    }

    #[test]
    fn a_record_is_cut_to_its_stored_length() {
        let page = page(5, |bytes| {
            put_slots(bytes, &[4000]);
            bytes[4028..4033].copy_from_slice(&[5, 0, 0, 0xaa, 0xbb]);
        });
        let slots = page.slots().unwrap();
        let bytes = page.slot_bytes(&slots[0]);
        assert_eq!(record(bytes), Some(&[5, 0, 0, 0xaa, 0xbb][..]));
        assert_eq!(record(&[2, 0, 0]), None);
        assert_eq!(record(&[9, 0, 0]), None);
    }

    // Every slot is empty, so no offset says that the directory is too long.
    #[test]
    fn a_directory_longer_than_the_page_is_rejected() {
        let slots = |count: u16| {
            let page = page(5, |bytes| {
                bytes[22..24].copy_from_slice(&count.to_le_bytes())
            });
            page.slots()
        };
        assert_eq!(slots(2032), Ok(Vec::new()));
        assert_eq!(slots(2033), Err(PageProblem::Directory));
        assert_eq!(slots(u16::MAX), Err(PageProblem::Directory));
    }

    #[test]
    fn a_slot_inside_the_directory_is_rejected() {
        let page = page(5, |bytes| put_slots(bytes, &[1]));
        assert_eq!(page.slots(), Err(PageProblem::Directory));
    }

    #[test]
    fn a_checkpoint_copy_is_not_the_page_it_copies() {
        let copy = Page::new(9, page(5, |_| {}).bytes);
        assert_eq!(
            copy.expect(PageKind::Table, 7),
            Err(PageProblem::WrongNumber { stored: 5 })
        );
        let own = page(5, |_| {});
        assert_eq!(own.expect(PageKind::Table, 7), Ok(()));
        assert_eq!(
            own.expect(PageKind::Table, 8),
            Err(PageProblem::WrongTable { table_id: 7 })
        );
    }
}
