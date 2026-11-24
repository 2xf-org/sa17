//! How to read the rows of one schema version into the columns asked for.

use crate::error::RowProblem;
use crate::record::stored_field;
use crate::record::stored_field::{Form, StoredField};
use crate::schema::Column;

/// The size of a record's length and flags.
const HEADER: usize = 3;

/// The shortest record the engine allocates. A row with fewer bytes is padded
/// with zeros, so that a forward to another page always fits in its place.
const SHORTEST_RECORD: usize = 9;

/// The columns that the rows of one schema version store, and where the values
/// of the wanted ones go.
pub(crate) struct Plan {
    bitmap_size: usize,
    steps: Vec<Step>,
    absent: Vec<Place>,
}

/// One stored column.
struct Step {
    /// The column's position in the table.
    column: usize,
    form: Form,
    nullable: bool,
    /// Where its value goes, when it is wanted.
    slot: Option<usize>,
}

/// A column's position in the table and where its value goes.
#[derive(Clone, Copy)]
pub(crate) struct Place {
    pub(crate) column: usize,
    pub(crate) slot: usize,
}

impl Plan {
    /// The plan for rows written at `schema`, which store every column that
    /// existed by then. `slots` says where the value of each column goes.
    pub(crate) fn new(
        columns: &[Column],
        slots: &[Option<usize>],
        schema: u32,
    ) -> Result<Self, RowProblem> {
        let mut plan = Self {
            bitmap_size: 0,
            steps: Vec::with_capacity(columns.len()),
            absent: Vec::new(),
        };
        let mut nullable = 0_usize;
        let mut last = 0;
        for (position, (column, &slot)) in columns.iter().zip(slots).enumerate() {
            // The numbers start at one, and the columns are sorted by them.
            if column.id == last {
                return Err(RowProblem::Columns);
            }
            last = column.id;
            if column.first_schema > schema {
                plan.absent.extend(slot.map(|slot| Place {
                    column: position,
                    slot,
                }));
                continue;
            }
            nullable += usize::from(column.nullable);
            plan.steps.push(Step {
                column: position,
                form: Form::of(column)?,
                nullable: column.nullable,
                slot,
            });
        }
        if plan.steps.is_empty() {
            return Err(RowProblem::Columns);
        }
        plan.bitmap_size = nullable.div_ceil(8);
        Ok(plan)
    }

    /// The wanted columns that rows of this version do not store.
    pub(crate) fn absent(&self) -> &[Place] {
        &self.absent
    }

    /// Walks over the fields of `record`, whose fields start `start` bytes
    /// after its header, and hands each wanted one that holds a value to
    /// `visit`. The others are only stepped over.
    pub(crate) fn walk<'record>(
        &self,
        record: &'record [u8],
        start: usize,
        columns: &[Column],
        mut visit: impl FnMut(Place, StoredField<'record>) -> Result<(), RowProblem>,
    ) -> Result<(), RowProblem> {
        let fields = record.get(HEADER + start..).ok_or(RowProblem::Truncated)?;
        let (bitmap, mut rest) = fields
            .split_at_checked(self.bitmap_size)
            .ok_or(RowProblem::Truncated)?;
        let mut bit = 0;
        for step in &self.steps {
            if step.nullable {
                let holds_value = bitmap[bit / 8] & (0x80 >> (bit % 8)) != 0;
                bit += 1;
                if !holds_value {
                    continue;
                }
            }
            let field = stored_field::take(&mut rest, step.form, &columns[step.column])?;
            if let Some(slot) = step.slot {
                let place = Place {
                    column: step.column,
                    slot,
                };
                visit(place, field)?;
            }
        }
        let padding = record.len() == SHORTEST_RECORD && rest.iter().all(|&byte| byte == 0);
        if rest.is_empty() || padding {
            Ok(())
        } else {
            Err(RowProblem::TrailingBytes { count: rest.len() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Descriptor;
    use crate::row_address::RowAddress;
    use crate::value::Value;

    fn column(id: u32, domain: u16, nullable: bool, first_schema: u32) -> Column {
        Column {
            id,
            name: format!("column_{id}"),
            domain,
            width: if domain == 19 { 1 } else { 40 },
            scale: 0,
            nullable,
            default: None,
            first_schema,
            absent: Some(Value::Null),
        }
    }

    /// The columns that a walk hands over, by position, and their fields.
    fn walk<'record>(
        columns: &[Column],
        wanted: &[usize],
        record: &'record [u8],
    ) -> Result<Vec<(usize, StoredField<'record>)>, RowProblem> {
        let mut slots = vec![None; columns.len()];
        for (slot, &column) in wanted.iter().enumerate() {
            slots[column] = Some(slot);
        }
        let mut found = Vec::new();
        let plan = Plan::new(columns, &slots, 0)?;
        plan.walk(record, 0, columns, |place, field| {
            found.push((place.column, field));
            Ok(())
        })?;
        Ok(found)
    }

    #[test]
    fn a_schema_version_stores_only_the_columns_it_knew() {
        let columns = [
            column(1, 2, false, 0),
            column(2, 1, false, 0),
            column(4, 2, false, 3),
        ];
        let slots = [Some(0), None, Some(1)];
        let old = Plan::new(&columns, &slots, 0).unwrap();
        assert_eq!((old.steps.len(), old.absent().len()), (2, 1));
        assert_eq!((old.absent()[0].column, old.absent()[0].slot), (2, 1));
        let new = Plan::new(&columns, &slots, 3).unwrap();
        assert_eq!((new.steps.len(), new.absent().len()), (3, 0));
    }

    #[test]
    fn columns_that_make_no_row_are_refused() {
        let problem = |columns: &[Column]| {
            let slots = vec![None; columns.len()];
            Plan::new(columns, &slots, 0).err()
        };
        assert_eq!(problem(&[]), Some(RowProblem::Columns));
        assert_eq!(
            problem(&[column(1, 2, false, 1)]),
            Some(RowProblem::Columns)
        );
        let twice = [column(1, 2, false, 0), column(1, 2, false, 0)];
        assert_eq!(problem(&twice), Some(RowProblem::Columns));
        assert_eq!(
            problem(&[column(0, 2, false, 0)]),
            Some(RowProblem::Columns)
        );
        let mut wide = column(3, 19, false, 0);
        wide.width = 2;
        assert_eq!(
            problem(&[wide]),
            Some(RowProblem::Width {
                column_id: 3,
                width: 2
            })
        );
    }

    #[test]
    fn a_short_row_is_padded_with_zeros_to_nine_bytes() {
        let columns = [column(1, 1, false, 0)];
        let padded = [9, 0, 0, 0x34, 0x12, 0, 0, 0, 0];
        assert_eq!(
            walk(&columns, &[0], &padded).unwrap(),
            [(0, StoredField::Bytes(&[0x34, 0x12]))]
        );
        // Bytes after the fields that are not zero are not padding.
        let not_padding = [9, 0, 0, 0x34, 0x12, 0, 0, 0, 1];
        assert_eq!(
            walk(&columns, &[0], &not_padding),
            Err(RowProblem::TrailingBytes { count: 4 })
        );
        let longer = [10, 0, 0, 0x34, 0x12, 0, 0, 0, 0, 0];
        assert_eq!(
            walk(&columns, &[0], &longer),
            Err(RowProblem::TrailingBytes { count: 5 })
        );
    }

    /// A record of a nullable integer, a number, a short string, a long
    /// string, a linked and a compressed long value, and a bit.
    fn mixed() -> (Vec<Column>, Vec<u8>) {
        let columns = vec![
            column(1, 2, true, 0),
            column(2, 3, false, 0),
            column(3, 9, true, 0),
            column(4, 8, false, 0),
            column(5, 10, false, 0),
            column(6, 12, false, 0),
            column(7, 24, false, 0),
        ];
        let mut record = vec![0, 0, 0, 0b0100_0000];
        record.extend([2, 0xbf, 50, 1]);
        record.extend([3, b'a', b'b', b'c']);
        record.extend([0xff, 0, 2, b'x', b'y']);
        record.extend([0x82, b'l', b'o', 0x80, 60, 0, 0, 0, 7, 0, 0, 0, 0, 0, 3, 0]);
        record.extend([0x81, b'z', 0x40, 30, 0, 0, 0, 8, 0, 0, 0, 0, 0, 4, 0]);
        record.extend([90, 0, 0, 0, 1]);
        record[0] = record.len() as u8;
        (columns, record)
    }

    #[test]
    fn only_wanted_fields_are_handed_over() {
        let (columns, record) = mixed();
        let long = Descriptor {
            prefix: b"lo",
            stored_length: 60,
            first: RowAddress::new(7, 3),
            decompressed_length: None,
        };
        let compressed = Descriptor {
            prefix: b"z",
            stored_length: 30,
            first: RowAddress::new(8, 4),
            decompressed_length: Some(90),
        };
        let every = walk(&columns, &[0, 1, 2, 3, 4, 5, 6], &record).unwrap();
        assert_eq!(
            every,
            [
                (
                    1,
                    StoredField::Number {
                        control: 0xbf,
                        digits: &[50, 1]
                    }
                ),
                (2, StoredField::Bytes(b"abc")),
                (3, StoredField::Bytes(b"xy")),
                (4, StoredField::Long(long)),
                (5, StoredField::Long(compressed)),
                (6, StoredField::Bytes(&[1])),
            ]
        );
        let some = walk(&columns, &[6, 3], &record).unwrap();
        assert_eq!(some, [every[2], every[5]]);
        assert_eq!(walk(&columns, &[], &record).unwrap(), []);
    }

    #[test]
    fn every_truncation_of_a_record_is_refused() {
        let (columns, record) = mixed();
        for length in 3..record.len() {
            let mut cut = record[..length].to_vec();
            cut[..2].copy_from_slice(&(length as u16).to_le_bytes());
            assert!(walk(&columns, &[], &cut).is_err(), "{length} bytes");
        }
    }

    #[test]
    fn a_descriptor_of_an_unknown_form_is_refused() {
        let columns = [column(5, 10, false, 0)];
        let record = |field: &[u8]| {
            let mut record = vec![0, 0, 0];
            record.extend_from_slice(field);
            record[0] = record.len() as u8;
            record
        };
        let refused = Err(RowProblem::Descriptor { column_id: 5 });
        let tail = [9, 0, 0, 0, 7, 0, 0, 0, 0, 0, 3, 0];
        let with_tag = |tag| record(&[&[0x80, tag][..], &tail].concat());
        assert!(walk(&columns, &[0], &with_tag(0)).is_ok());
        assert_eq!(walk(&columns, &[0], &with_tag(0x20)), refused);
        // A CHAR is never linked, and no value is shorter than its prefix.
        let char_column = [column(5, 8, false, 0)];
        assert_eq!(walk(&char_column, &[0], &with_tag(0x80)), refused);
        let short = record(&[&[0x8a][..], &[b'p'; 10], &[0], &tail].concat());
        assert_eq!(walk(&columns, &[0], &short), refused);
        let empty = record(&[&[0x80, 0x40][..], &tail, &[0, 0, 0, 0]].concat());
        assert_eq!(walk(&columns, &[0], &empty), refused);
    }
}
