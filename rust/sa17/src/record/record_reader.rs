//! Reading the records on a table's pages.
//!
//! A record starts with a 16-bit length and a flags byte. The flags say
//! whether the row is live, whether it carries the schema version it was
//! written with, and whether it is only a forward to the row's new place on
//! an extension page.

use crate::error::{Error, LongValueProblem, PageProblem, RowProblem};
use crate::record::field;
use crate::record::long_value::LongValueReader;
use crate::record::{Columns, ExtensionPages, Met, Place, Repeat, Row, StoredField};
use crate::row_address::RowAddress;
use crate::schema::{Column, TableDefinition};
use crate::store::page;
use crate::store::{PageKind, PageStore, RowPage, Slot};
use crate::value::Value;

/// The row was deleted, or replaced by a later version.
const DEAD: u8 = 0x01 | 0x20;

/// The record is a forward: the row itself moved to an extension page.
const FORWARD: u8 = 0x04;

/// A flag of some live rows. What it means is not known, and the rows read
/// like the others.
const UNEXPLAINED: u8 = 0x08;

/// A byte with the row's schema version follows the flags.
const SCHEMA_VERSION: u8 = 0x40;

/// Every flag that a row can have.
const KNOWN: u8 = DEAD | FORWARD | UNEXPLAINED | SCHEMA_VERSION;

/// The size of a forward: length, flags, page and slot.
const FORWARD_SIZE: usize = 9;

/// Reads the records of one table into some of its columns.
pub(crate) struct RecordReader<'db> {
    store: &'db PageStore,
    definition: &'db TableDefinition,
    columns: Columns,
    extension_pages: ExtensionPages<'db>,
    long_values: LongValueReader<'db>,
}

impl<'db> RecordReader<'db> {
    /// A reader of `columns`, which are columns of the table, and of values of
    /// up to `longest_value` bytes out of compression.
    pub(crate) fn new(
        store: &'db PageStore,
        definition: &'db TableDefinition,
        columns: Columns,
        longest_value: u32,
    ) -> Self {
        let extension_pages = ExtensionPages::new(store, definition);
        Self {
            store,
            definition,
            columns,
            extension_pages,
            long_values: LongValueReader::new(extension_pages, longest_value),
        }
    }

    /// The table that the records are of.
    pub(crate) fn definition(&self) -> &'db TableDefinition {
        self.definition
    }

    /// The page of the table's rows at position `number`.
    pub(crate) fn page(&self, number: u32) -> Result<RowPage, PageProblem> {
        self.store.rows(number, PageKind::Table, self.definition.id)
    }

    /// The row at `address`, or `None` when the table has no live row there.
    pub(crate) fn at(&self, address: RowAddress) -> Result<Option<Row>, Error> {
        if let Some(error) = self.definition.unlisted() {
            return Err(error);
        }
        let number = address.page();
        let page = if self.definition.pages.allows(number) {
            let id = self.definition.id;
            self.store.shared_rows(number, PageKind::Table, id)
        } else {
            Err(PageProblem::Unlisted)
        };
        let page = page.map_err(|problem| {
            let page = number;
            self.error(address, RowProblem::Page { page, problem })
        })?;
        match page.slot(address.slot()) {
            Some(slot) => self.row(&page, slot, &mut Met::default()),
            None => Ok(None),
        }
    }

    /// The address of the row in `slot`, or `None` when the row is dead.
    pub(crate) fn address(&self, page: &RowPage, slot: &Slot) -> Result<Option<RowAddress>, Error> {
        let at = RowAddress::new(page.number(), slot.index);
        let live = self.live(page, slot, at)?;
        Ok(live.map(|_| at))
    }

    /// The row in `slot`, or `None` when the row is dead. `met` has what was
    /// read of the table's extension pages so far.
    pub(crate) fn row(
        &self,
        page: &RowPage,
        slot: &Slot,
        met: &mut Met,
    ) -> Result<Option<Row>, Error> {
        let at = RowAddress::new(page.number(), slot.index);
        let Some(stored) = self.live(page, slot, at)? else {
            return Ok(None);
        };
        let values = if stored[2] & FORWARD == 0 {
            self.values(stored, met)
        } else {
            self.moved_values(stored, met)
        };
        match values {
            Ok(values) => Ok(Row::new(at.into(), &self.columns, values)),
            Err(problem) => Err(self.error(at, problem)),
        }
    }

    /// The record in `slot`, cut to its stored length, unless its row is dead.
    fn live<'page>(
        &self,
        page: &'page RowPage,
        slot: &Slot,
        at: RowAddress,
    ) -> Result<Option<&'page [u8]>, Error> {
        let stored = page::record(page.slot_bytes(slot));
        let stored = stored.ok_or_else(|| self.error(at, RowProblem::Length))?;
        check_flags(stored).map_err(|problem| self.error(at, problem))?;
        Ok((stored[2] & DEAD == 0).then_some(stored))
    }

    /// The values of the row that the forward `stored` leads to.
    fn moved_values(&self, stored: &[u8], met: &mut Met) -> Result<Vec<Value>, RowProblem> {
        let to = forward(stored).ok_or(RowProblem::Length)?;
        if !met.first(to) {
            return Err(RowProblem::RepeatedForward { to });
        }
        let page = to.page();
        let extension = self.extension_pages.page(page);
        let extension = extension.map_err(|problem| RowProblem::Page { page, problem })?;
        let moved = extension
            .record(to.slot())
            .filter(|moved| moved[2] & (DEAD | FORWARD) == 0)
            .ok_or(RowProblem::Forward { to })?;
        check_flags(moved)?;
        self.values(moved, met)
    }

    /// The wanted values of a record that holds fields.
    fn values(&self, stored: &[u8], met: &mut Met) -> Result<Vec<Value>, RowProblem> {
        let versioned = stored[2] & SCHEMA_VERSION != 0;
        let version = if versioned {
            *stored.get(3).ok_or(RowProblem::Length)?
        } else {
            0
        };
        if u32::from(version) > self.definition.schema {
            return Err(RowProblem::SchemaVersion { version });
        }
        let plan = self.columns.plan(version.into())?;
        let columns = self.columns.table_columns();
        let mut values = vec![Value::Null; self.columns.len()];
        // A column added after the row was written keeps its absent value.
        for &Place { column, slot } in plan.absent() {
            let column = &columns[column];
            let absent = column.absent.as_ref().ok_or(RowProblem::AbsentValue {
                column_id: column.id,
            })?;
            values[slot].clone_from(absent);
        }
        plan.walk(
            stored,
            usize::from(versioned),
            columns,
            |Place { column, slot }, field| {
                values[slot] = self.value(field, &columns[column], met)?;
                Ok(())
            },
        )?;
        for &Repeat { from, to } in self.columns.repeated() {
            values[to] = values[from].clone();
        }
        Ok(values)
    }

    /// The value of one field, put together from its pieces if it is long.
    fn value(
        &self,
        field: StoredField<'_>,
        column: &Column,
        met: &mut Met,
    ) -> Result<Value, RowProblem> {
        let encoding = self.store.text_encoding();
        match field {
            StoredField::Bytes(bytes) => field::value(bytes, column, encoding),
            StoredField::Number { control, digits } => Ok(field::number(control, digits, column)),
            StoredField::Long(descriptor) => {
                let problem = |piece, problem| RowProblem::LongValue {
                    column_id: column.id,
                    piece,
                    problem,
                };
                let bytes = self.long_values.read(&descriptor, met);
                let bytes = bytes.map_err(|failure| problem(failure.piece, failure.problem))?;
                let length = bytes.len() as u32;
                field::long_value(bytes, column, encoding)
                    .ok_or_else(|| problem(descriptor.first, LongValueProblem::TooLong { length }))
            }
        }
    }

    fn error(&self, at: RowAddress, problem: RowProblem) -> Error {
        Error::Row {
            table: self.definition.name.clone(),
            address: at,
            problem,
        }
    }
}

/// Refuses a record whose flags could change how it is read.
fn check_flags(stored: &[u8]) -> Result<(), RowProblem> {
    match stored[2] & !KNOWN {
        0 => Ok(()),
        flags => Err(RowProblem::Flags { flags }),
    }
}

/// The page and slot that a forward names.
fn forward(stored: &[u8]) -> Option<RowAddress> {
    let bytes: &[u8; FORWARD_SIZE] = stored.try_into().ok()?;
    Some(RowAddress::new(
        u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]),
        u16::from_le_bytes([bytes[7], bytes[8]]),
    ))
}
