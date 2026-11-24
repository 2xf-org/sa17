//! What names a column of a row.

use crate::record::Columns;

/// What names one of a row's [`Columns`]: its position among them, or its
/// name, ignoring ASCII case.
pub trait ColumnIndex: sealed::Sealed {}

impl ColumnIndex for usize {}
impl ColumnIndex for &str {}

pub(crate) mod sealed {
    use crate::record::Columns;

    pub trait Sealed {
        /// The position of the column, if there is one.
        fn position(&self, columns: &Columns) -> Option<usize>;

        /// The column as it was asked for.
        fn name(&self, columns: &Columns) -> String;
    }
}

impl sealed::Sealed for usize {
    fn position(&self, _columns: &Columns) -> Option<usize> {
        Some(*self)
    }

    fn name(&self, columns: &Columns) -> String {
        let name = columns.names().nth(*self);
        name.map_or_else(|| self.to_string(), str::to_owned)
    }
}

impl sealed::Sealed for &str {
    fn position(&self, columns: &Columns) -> Option<usize> {
        columns.position(self)
    }

    fn name(&self, _columns: &Columns) -> String {
        (*self).to_owned()
    }
}
