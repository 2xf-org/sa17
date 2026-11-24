//! The readable pages of a file while it is being laid out.

const PAGE: usize = 4096;
const DIRECTORY: usize = 28;
const ROW_AREA_END: usize = PAGE - 4;

/// The kind byte of a table's own pages.
pub(crate) const TABLE_PAGE: u8 = 3;
/// The kind byte of pages that hold forwarded rows.
pub(crate) const FORWARD_PAGE: u8 = 18;
/// The kind byte of pages that hold pieces of long values.
pub(crate) const PIECE_PAGE: u8 = 20;

/// Where a record was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Address {
    pub(crate) page: u32,
    pub(crate) slot: u16,
}

/// Every page of the file, readable, indexed by page number.
pub(crate) struct Pages {
    pages: Vec<[u8; PAGE]>,
}

impl Pages {
    /// The first eleven pages: the superblock, nine pages that were never
    /// written, and the page that lists the table catalog's pages.
    pub(crate) fn new() -> Self {
        Self {
            pages: vec![[0; PAGE]; 11],
        }
    }

    pub(crate) fn len(&self) -> u32 {
        self.pages.len() as u32
    }

    pub(crate) fn page_mut(&mut self, number: u32) -> &mut [u8; PAGE] {
        &mut self.pages[number as usize]
    }

    pub(crate) fn into_pages(self) -> Vec<[u8; PAGE]> {
        self.pages
    }

    /// Adds an empty row page of `kind` that `owner` owns.
    pub(crate) fn add_row_page(&mut self, owner: u32, kind: u8) -> u32 {
        let number = self.len();
        let mut page = [0; PAGE];
        page[..4].copy_from_slice(&number.to_le_bytes());
        page[6] = b'E';
        page[8..12].copy_from_slice(&(1000 + number).to_le_bytes());
        page[16] = 4;
        page[17] = kind;
        page[24..28].copy_from_slice(&owner.to_le_bytes());
        self.pages.push(page);
        number
    }

    /// Adds a copy of page `number` at the end of the file, as a checkpoint
    /// leaves one: it keeps the number of the page it copies.
    pub(crate) fn add_copy(&mut self, number: u32) {
        let copy = self.pages[number as usize];
        self.pages.push(copy);
    }

    /// Writes `record` into the lowest free bytes of a row page, if it fits.
    /// Without bytes it makes an empty slot.
    pub(crate) fn add_record(&mut self, number: u32, record: &[u8]) -> Option<u16> {
        let page = &mut self.pages[number as usize];
        let slots = usize::from(u16::from_le_bytes([page[22], page[23]]));
        let offsets = (0..slots).map(|slot| {
            let offset = [page[28 + 2 * slot], page[29 + 2 * slot]];
            usize::from(u16::from_le_bytes(offset))
        });
        let occupied = offsets.filter(|&offset| offset > 0);
        let lowest = occupied
            .min()
            .map_or(ROW_AREA_END, |offset| DIRECTORY + offset);
        let start = lowest.checked_sub(record.len())?;
        if start < DIRECTORY + 2 * (slots + 1) {
            return None;
        }
        page[start..lowest].copy_from_slice(record);
        // A slot without a record has the offset zero.
        let offset = if record.is_empty() {
            0
        } else {
            (start - DIRECTORY) as u16
        };
        page[28 + 2 * slots..30 + 2 * slots].copy_from_slice(&offset.to_le_bytes());
        let count = (slots + 1) as u16;
        page[18..20].copy_from_slice(&count.to_le_bytes());
        page[20..22].copy_from_slice(&offset.to_le_bytes());
        page[22..24].copy_from_slice(&count.to_le_bytes());
        Some(slots as u16)
    }
}

/// Fills row pages of one kind for one table, opening a new page when a
/// record does not fit the last.
pub(crate) struct Writer {
    owner: u32,
    kind: u8,
    pub(crate) pages: Vec<u32>,
}

impl Writer {
    pub(crate) fn new(owner: u32, kind: u8) -> Self {
        Self {
            owner,
            kind,
            pages: Vec::new(),
        }
    }

    /// Adds a page that stays empty.
    pub(crate) fn reserve(&mut self, pages: &mut Pages) {
        let page = pages.add_row_page(self.owner, self.kind);
        self.pages.push(page);
    }

    /// Writes one record and says where it went.
    ///
    /// # Panics
    ///
    /// Panics when the record is longer than an empty page can hold.
    pub(crate) fn add(&mut self, pages: &mut Pages, record: &[u8]) -> Address {
        if let Some(&page) = self.pages.last()
            && let Some(slot) = pages.add_record(page, record)
        {
            return Address { page, slot };
        }
        let page = pages.add_row_page(self.owner, self.kind);
        self.pages.push(page);
        let slot = pages
            .add_record(page, record)
            .expect("a record fits an empty page");
        Address { page, slot }
    }
}
