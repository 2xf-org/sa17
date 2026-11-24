//! One column of a table.

use crate::schema::{DataType, LengthUnit};
use crate::value::Value;

/// The scale of a character column whose width counts characters.
const COUNTS_CHARACTERS: u16 = 1;

/// A column as the catalog describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) domain: u16,
    pub(crate) width: u32,
    pub(crate) scale: u16,
    pub(crate) nullable: bool,
    pub(crate) default: Option<String>,
    pub(crate) first_schema: u32,
    /// `None` when the catalog has one that cannot be read.
    pub(crate) absent: Option<Value>,
}

impl Column {
    /// The column's number within its table. Numbers start at one and can
    /// have gaps where columns were dropped.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The column's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The column's SQL type.
    #[must_use]
    pub fn data_type(&self) -> DataType {
        DataType::from_domain(self.domain)
    }

    /// The domain identifier that the catalog stores for the type.
    #[must_use]
    pub fn domain(&self) -> u16 {
        self.domain
    }

    /// The declared width: characters or bytes for strings, the precision for
    /// `NUMERIC` and `DECIMAL`, and the storage size for the other types.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// What the width counts, for a string or binary column whose width limits
    /// its values. That of a long one does not.
    #[must_use]
    pub fn length_unit(&self) -> Option<LengthUnit> {
        match self.data_type() {
            DataType::Char | DataType::VarChar if self.scale == COUNTS_CHARACTERS => {
                Some(LengthUnit::Characters)
            }
            DataType::Char | DataType::VarChar | DataType::Binary | DataType::VarBinary => {
                Some(LengthUnit::Bytes)
            }
            _ => None,
        }
    }

    /// The digits after the decimal point of a `NUMERIC` or `DECIMAL` column.
    #[must_use]
    pub fn scale(&self) -> u16 {
        self.scale
    }

    /// Whether the column accepts `NULL`.
    #[must_use]
    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    /// The `DEFAULT` expression, as the catalog keeps its text.
    #[must_use]
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// The table's schema version that added the column. Rows written before
    /// it do not store the column.
    #[must_use]
    pub fn first_schema(&self) -> u32 {
        self.first_schema
    }

    /// The value of the column in rows written before the column existed, or
    /// `None` when the catalog has one that cannot be read.
    #[must_use]
    pub fn absent_value(&self) -> Option<&Value> {
        self.absent.as_ref()
    }

    /// The type as SQL writes it, such as `numeric(15,2)` or `varchar(40)`.
    #[must_use]
    pub fn sql_type(&self) -> String {
        let name = self.data_type().name();
        match self.data_type() {
            DataType::Numeric | DataType::Decimal => {
                format!("{name}({},{})", self.width, self.scale)
            }
            DataType::Char | DataType::VarChar | DataType::Binary | DataType::VarBinary => {
                format!("{name}({})", self.width)
            }
            DataType::Unknown(domain) => format!("{name}({domain})"),
            _ => name.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_of_a_character_column_says_what_its_width_counts() {
        let unit = |domain, scale| {
            let column = Column {
                id: 1,
                name: "name".to_owned(),
                domain,
                width: 20,
                scale,
                nullable: false,
                default: None,
                first_schema: 0,
                absent: Some(Value::Null),
            };
            column.length_unit()
        };
        assert_eq!(unit(9, 0), Some(LengthUnit::Bytes));
        assert_eq!(unit(9, 1), Some(LengthUnit::Characters));
        assert_eq!(unit(8, 1), Some(LengthUnit::Characters));
        assert_eq!(unit(28, 1), Some(LengthUnit::Bytes));
        assert_eq!((unit(10, 1), unit(2, 0)), (None, None));
    }
}
