//! One row of a table.

use std::fmt;

use crate::error::{Error, Result, ValueProblem};
use crate::from_value::FromValue;
use crate::record::{ColumnIndex, Columns};
use crate::row_id::RowId;
use crate::row_key::RowKey;
use crate::value::Value;

/// One row: which one it is, and a value for each of the [`Columns`] it was read
/// with.
#[derive(Clone, PartialEq)]
pub struct Row {
    id: RowId,
    columns: Columns,
    values: Vec<Value>,
}

impl Row {
    /// A row from its values, or `None` when there is not one for each of
    /// `columns`.
    #[must_use]
    pub fn new(id: RowId, columns: &Columns, values: Vec<Value>) -> Option<Self> {
        let columns = columns.clone();
        (values.len() == columns.len()).then_some(Self {
            id,
            columns,
            values,
        })
    }

    /// Which row of its table this is.
    #[must_use]
    pub fn id(&self) -> RowId {
        self.id
    }

    /// The columns that the row has values of.
    #[must_use]
    pub fn columns(&self) -> &Columns {
        &self.columns
    }

    /// The values, in the order of [`Self::columns`].
    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The values, to change them in place.
    #[must_use]
    pub fn values_mut(&mut self) -> &mut [Value] {
        &mut self.values
    }

    /// Gives up the row for its values.
    #[must_use]
    pub fn into_values(self) -> Vec<Value> {
        self.values
    }

    /// The value at a position among [`Self::columns`].
    #[must_use]
    pub fn value(&self, position: usize) -> Option<&Value> {
        self.values.get(position)
    }

    /// The value of the column called `column`, ignoring ASCII case, if it is
    /// one of [`Self::columns`].
    #[must_use]
    pub fn get(&self, column: &str) -> Option<&Value> {
        self.value(self.columns.position(column)?)
    }

    /// The key that the row's values make, to look a row up by, or `None` when
    /// one of them is `NULL`.
    ///
    /// The values of [`Columns::at`] the positions of a declared key are the
    /// key's.
    #[must_use]
    pub fn key(&self) -> Option<RowKey> {
        RowKey::new(&self.values)
    }

    /// The value at a position among [`Self::columns`], or of the column of a
    /// name, as a Rust type. `Option` takes `NULL`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Value`] when the row has no such column, and when the
    /// value is of another type, does not fit, or is `NULL` where the type has
    /// no place for it.
    pub fn read<'row, T: FromValue<'row>>(&'row self, column: impl ColumnIndex) -> Result<T> {
        let position = column.position(&self.columns);
        let value = position.and_then(|position| self.value(position));
        let read = value.ok_or(ValueProblem::Missing).and_then(T::from_value);
        read.map_err(|problem| Error::Value {
            table: self.columns.table_name().to_owned(),
            id: self.id,
            column: column.name(&self.columns),
            problem,
        })
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Row")
            .field("id", &self.id)
            .field("columns", &self.columns)
            .field("values", &self.values)
            .finish()
    }
}
