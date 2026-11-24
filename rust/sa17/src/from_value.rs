//! Values as Rust types.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::decimal::Decimal;
use crate::error::ValueProblem;
use crate::value::Value;

/// A type that a value of a row can be read as, with
/// [`Row::read`](crate::Row::read).
///
/// An integer type takes any whole number that fits it, `f64` takes a `FLOAT`
/// and a `DOUBLE`, and every other type only the values it is made for.
pub trait FromValue<'value>: Sized {
    /// Reads `value`.
    ///
    /// # Errors
    ///
    /// Returns how the value differs from what the type takes.
    fn from_value(value: &'value Value) -> Result<Self, ValueProblem>;
}

impl<'value, T: FromValue<'value>> FromValue<'value> for Option<T> {
    fn from_value(value: &'value Value) -> Result<Self, ValueProblem> {
        match value {
            Value::Null => Ok(None),
            value => T::from_value(value).map(Some),
        }
    }
}

impl<'value> FromValue<'value> for &'value Value {
    fn from_value(value: &'value Value) -> Result<Self, ValueProblem> {
        Ok(value)
    }
}

impl FromValue<'_> for bool {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_bool(), "a bit")
    }
}

impl FromValue<'_> for u8 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "u8")
    }
}

impl FromValue<'_> for i16 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "i16")
    }
}

impl FromValue<'_> for u16 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "u16")
    }
}

impl FromValue<'_> for i32 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "i32")
    }
}

impl FromValue<'_> for u32 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "u32")
    }
}

impl FromValue<'_> for i64 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "i64")
    }
}

impl FromValue<'_> for u64 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "u64")
    }
}

impl FromValue<'_> for i128 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        integer(value, "i128")
    }
}

impl FromValue<'_> for f32 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_f32(), "a float")
    }
}

impl FromValue<'_> for f64 {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_f64(), "a double")
    }
}

impl FromValue<'_> for Decimal {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_decimal(), "a decimal")
    }
}

impl<'value> FromValue<'value> for &'value str {
    fn from_value(value: &'value Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_str(), "text")
    }
}

impl FromValue<'_> for String {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        <&str>::from_value(value).map(str::to_owned)
    }
}

impl<'value> FromValue<'value> for &'value [u8] {
    fn from_value(value: &'value Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_bytes(), "bytes")
    }
}

impl FromValue<'_> for Vec<u8> {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        <&[u8]>::from_value(value).map(<[u8]>::to_vec)
    }
}

impl FromValue<'_> for NaiveDate {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_date(), "a date")
    }
}

impl FromValue<'_> for NaiveTime {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_time(), "a time")
    }
}

impl FromValue<'_> for NaiveDateTime {
    fn from_value(value: &Value) -> Result<Self, ValueProblem> {
        typed(value, value.as_timestamp(), "a timestamp")
    }
}

/// What an accessor of `value` found, or why it found nothing.
fn typed<T>(value: &Value, found: Option<T>, expected: &'static str) -> Result<T, ValueProblem> {
    found.ok_or_else(|| match value {
        Value::Null => ValueProblem::Null { expected },
        value => ValueProblem::Type {
            expected,
            found: value.type_name(),
        },
    })
}

fn integer<T: TryFrom<i128>>(value: &Value, expected: &'static str) -> Result<T, ValueProblem> {
    let integer = typed(value, value.as_integer(), expected)?;
    T::try_from(integer).map_err(|_| ValueProblem::Range { expected })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_number_reads_as_any_type_it_fits() {
        let number = Value::Integer(300);
        assert_eq!(i16::from_value(&number), Ok(300));
        assert_eq!(u64::from_value(&number), Ok(300));
        assert_eq!(
            u8::from_value(&number),
            Err(ValueProblem::Range { expected: "u8" })
        );
        assert_eq!(
            u32::from_value(&Value::Integer(-1)),
            Err(ValueProblem::Range { expected: "u32" })
        );
        assert_eq!(
            i64::from_value(&Value::Boolean(true)),
            Err(ValueProblem::Type {
                expected: "i64",
                found: "a bit"
            })
        );
    }

    #[test]
    fn null_needs_an_option() {
        assert_eq!(Option::<i64>::from_value(&Value::Null), Ok(None));
        assert_eq!(Option::<i64>::from_value(&Value::Integer(1)), Ok(Some(1)));
        assert_eq!(
            String::from_value(&Value::Null),
            Err(ValueProblem::Null { expected: "text" })
        );
        assert_eq!(<&Value>::from_value(&Value::Null), Ok(&Value::Null));
    }

    #[test]
    fn every_other_type_reads_its_own_values() {
        let text = Value::Text("a".to_owned());
        assert_eq!(<&str>::from_value(&text), Ok("a"));
        let bytes = Value::Bytes(vec![1]);
        assert_eq!(<&[u8]>::from_value(&bytes), Ok(&[1][..]));
        assert_eq!(Vec::<u8>::from_value(&bytes), Ok(vec![1]));
        assert_eq!(f64::from_value(&Value::Float(0.5)), Ok(0.5));
        let number: Decimal = "2.5".parse().unwrap();
        assert_eq!(Decimal::from_value(&Value::Decimal(number)), Ok(number));
        let day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        assert_eq!(NaiveDate::from_value(&Value::Date(day)), Ok(day));
        assert!(NaiveDateTime::from_value(&Value::Date(day)).is_err());
        assert!(bool::from_value(&Value::Integer(1)).is_err());
    }
}
