//! The rows of the catalog tables, read by column name.

use crate::error::{CatalogProblem, Result};
use crate::record::Row;
use crate::value::Value;

/// One row of a catalog table, read by column name.
pub(crate) struct Fields<'row> {
    catalog: &'static str,
    row: &'row Row,
}

impl<'row> Fields<'row> {
    /// The fields of `row`, a row of the catalog table called `catalog`.
    pub(crate) fn new(catalog: &'static str, row: &'row Row) -> Self {
        Self { catalog, row }
    }

    fn value(&self, column: &'static str) -> Result<&'row Value> {
        self.row
            .get(column)
            .ok_or_else(|| self.problem(column).into())
    }

    pub(crate) fn unsigned(&self, column: &'static str) -> Result<u64> {
        let value = self.value(column)?;
        value.as_u64().ok_or_else(|| self.problem(column).into())
    }

    pub(crate) fn narrow<T: TryFrom<u64>>(&self, column: &'static str) -> Result<T> {
        T::try_from(self.unsigned(column)?).map_err(|_| self.problem(column).into())
    }

    pub(crate) fn optional_narrow<T: TryFrom<u64>>(
        &self,
        column: &'static str,
    ) -> Result<Option<T>> {
        match self.value(column)? {
            Value::Null => Ok(None),
            _ => self.narrow(column).map(Some),
        }
    }

    pub(crate) fn text(&self, column: &'static str) -> Result<&'row str> {
        let value = self.value(column)?;
        value.as_str().ok_or_else(|| self.problem(column).into())
    }

    pub(crate) fn optional_text(&self, column: &'static str) -> Result<Option<&'row str>> {
        match self.value(column)? {
            Value::Null => Ok(None),
            value => value
                .as_str()
                .map(Some)
                .ok_or_else(|| self.problem(column).into()),
        }
    }

    pub(crate) fn optional_bytes(&self, column: &'static str) -> Result<Option<&'row [u8]>> {
        match self.value(column)? {
            Value::Null => Ok(None),
            value => value
                .as_bytes()
                .map(Some)
                .ok_or_else(|| self.problem(column).into()),
        }
    }

    pub(crate) fn problem(&self, column: &'static str) -> CatalogProblem {
        CatalogProblem::Field {
            table: self.catalog,
            column,
        }
    }
}
