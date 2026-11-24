//! A page of rows.

use crate::error::PageProblem;
use crate::store::page;
use crate::store::{Page, PageKind, Slot};

/// A decoded page of rows with its occupied slots, in slot order.
pub(crate) struct RowPage {
    page: Page,
    slots: Vec<Slot>,
}

impl RowPage {
    /// Lists the slots of `page`, which must be the current copy of a page of
    /// `kind` that the table with identifier `table_id` owns.
    pub(crate) fn new(page: Page, kind: PageKind, table_id: u32) -> Result<Self, PageProblem> {
        page.expect(kind, table_id)?;
        let slots = page.slots()?;
        Ok(Self { page, slots })
    }

    pub(crate) fn page(&self) -> &Page {
        &self.page
    }

    pub(crate) fn number(&self) -> u32 {
        self.page.number()
    }

    pub(crate) fn slots(&self) -> &[Slot] {
        &self.slots
    }

    /// The slot with number `index`, if it is occupied.
    pub(crate) fn slot(&self, index: u16) -> Option<&Slot> {
        let position = self.slots.binary_search_by_key(&index, |slot| slot.index);
        self.slots.get(position.ok()?)
    }

    /// The bytes that the row in `slot` may occupy.
    pub(crate) fn slot_bytes(&self, slot: &Slot) -> &[u8] {
        self.page.slot_bytes(slot)
    }

    /// The record in the slot with number `index`, cut to its stored length,
    /// if the slot holds one.
    pub(crate) fn record(&self, index: u16) -> Option<&[u8]> {
        page::record(self.slot_bytes(self.slot(index)?))
    }
}
