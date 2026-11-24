//! The values a row holds.

use std::fmt;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::decimal::Decimal;

/// One value of a row.
///
/// Its `Debug` output names the length of [`Self::Bytes`] and of an
/// [`Undecoded`] value, and not the bytes.
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// SQL `NULL`.
    Null,
    /// A `BIT`.
    Boolean(bool),
    /// A whole number of any of the integer types. The widest of them, signed
    /// and unsigned, fit in 128 bits, so a number has one representation.
    Integer(i128),
    /// A `FLOAT`.
    Float(f32),
    /// A `DOUBLE`.
    Double(f64),
    /// A `NUMERIC` or `DECIMAL`, exact.
    Decimal(Decimal),
    /// A `CHAR`, `VARCHAR` or `LONG VARCHAR`, decoded from the file's encoding.
    Text(String),
    /// A `BINARY`, `VARBINARY`, `LONG BINARY` or `LONG VARBIT`.
    Bytes(Vec<u8>),
    /// A `DATE`.
    Date(NaiveDate),
    /// A `TIME`.
    Time(NaiveTime),
    /// A `TIMESTAMP`, without a time zone.
    Timestamp(NaiveDateTime),
    /// A value this reader has no type for.
    Undecoded(Undecoded),
}

/// A value as a row stores it, which this reader has no [`Value`] for:
///
/// - a number of more digits than a [`Decimal`] holds, or with a digit that is
///   none of base 100;
/// - a `DATE` that is not the start of a day, a `TIME` past the end of the day,
///   and a `TIME` or a `TIMESTAMP` with more microseconds than a minute has;
/// - a `BIT` other than 0 and 1;
/// - a value of a domain the reader does not know.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Undecoded {
    domain: u16,
    bytes: Vec<u8>,
}

impl Undecoded {
    pub(crate) fn new(domain: u16, bytes: Vec<u8>) -> Self {
        Self { domain, bytes }
    }

    /// The domain identifier of the value's column.
    #[must_use]
    pub fn domain(&self) -> u16 {
        self.domain
    }

    /// The bytes that the row stores for the value.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for Undecoded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { domain, bytes } = self;
        write!(
            formatter,
            "Undecoded(domain {domain}, {} bytes)",
            bytes.len()
        )
    }
}

impl Value {
    /// What the value is, as a phrase for an error.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "NULL",
            Self::Boolean(_) => "a bit",
            Self::Integer(_) => "a whole number",
            Self::Float(_) => "a float",
            Self::Double(_) => "a double",
            Self::Decimal(_) => "a decimal",
            Self::Text(_) => "text",
            Self::Bytes(_) => "bytes",
            Self::Date(_) => "a date",
            Self::Time(_) => "a time",
            Self::Timestamp(_) => "a timestamp",
            Self::Undecoded(_) => "an undecoded value",
        }
    }

    /// Whether the value is SQL `NULL`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The value of a `BIT`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Self::Boolean(boolean) => Some(boolean),
            _ => None,
        }
    }

    /// A whole number.
    #[must_use]
    pub fn as_integer(&self) -> Option<i128> {
        match *self {
            Self::Integer(integer) => Some(integer),
            _ => None,
        }
    }

    /// A whole number as an `i64`, when it fits one.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        i64::try_from(self.as_integer()?).ok()
    }

    /// A whole number as a `u64`, when it is not negative and fits one.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        u64::try_from(self.as_integer()?).ok()
    }

    /// The value of a `FLOAT`.
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Self::Float(float) => Some(float),
            _ => None,
        }
    }

    /// The value of a `FLOAT` or a `DOUBLE`.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Self::Float(float) => Some(float.into()),
            Self::Double(double) => Some(double),
            _ => None,
        }
    }

    /// The value of a `NUMERIC` or `DECIMAL`.
    #[must_use]
    pub fn as_decimal(&self) -> Option<Decimal> {
        match *self {
            Self::Decimal(decimal) => Some(decimal),
            _ => None,
        }
    }

    /// The text of a character value.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The bytes of a binary value.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The value of a `DATE`.
    #[must_use]
    pub fn as_date(&self) -> Option<NaiveDate> {
        match *self {
            Self::Date(date) => Some(date),
            _ => None,
        }
    }

    /// The value of a `TIME`.
    #[must_use]
    pub fn as_time(&self) -> Option<NaiveTime> {
        match *self {
            Self::Time(time) => Some(time),
            _ => None,
        }
    }

    /// The value of a `TIMESTAMP`.
    #[must_use]
    pub fn as_timestamp(&self) -> Option<NaiveDateTime> {
        match *self {
            Self::Timestamp(timestamp) => Some(timestamp),
            _ => None,
        }
    }

    /// A value this reader has no type for.
    #[must_use]
    pub fn as_undecoded(&self) -> Option<&Undecoded> {
        match self {
            Self::Undecoded(undecoded) => Some(undecoded),
            _ => None,
        }
    }
}

macro_rules! from {
    ($($type:ty => $variant:ident),* $(,)?) => {$(
        impl From<$type> for Value {
            fn from(value: $type) -> Self {
                Self::$variant(value.into())
            }
        }
    )*};
}

from! {
    bool => Boolean,
    u8 => Integer,
    i16 => Integer,
    u16 => Integer,
    i32 => Integer,
    u32 => Integer,
    i64 => Integer,
    u64 => Integer,
    i128 => Integer,
    f32 => Float,
    f64 => Double,
    Decimal => Decimal,
    &str => Text,
    String => Text,
    &[u8] => Bytes,
    Vec<u8> => Bytes,
    NaiveDate => Date,
    NaiveTime => Time,
    NaiveDateTime => Timestamp,
}

/// `None` is `NULL`.
impl<T: Into<Self>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("Null"),
            Self::Boolean(boolean) => write!(formatter, "Boolean({boolean})"),
            Self::Integer(integer) => write!(formatter, "Integer({integer})"),
            Self::Float(float) => write!(formatter, "Float({float:?})"),
            Self::Double(double) => write!(formatter, "Double({double:?})"),
            Self::Decimal(decimal) => write!(formatter, "{decimal:?}"),
            Self::Text(text) => write!(formatter, "Text({text:?})"),
            Self::Bytes(bytes) => write!(formatter, "Bytes({} bytes)", bytes.len()),
            Self::Date(date) => write!(formatter, "Date({date})"),
            Self::Time(time) => write!(formatter, "Time({time})"),
            Self::Timestamp(timestamp) => write!(formatter, "Timestamp({timestamp})"),
            Self::Undecoded(undecoded) => write!(formatter, "{undecoded:?}"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("NULL"),
            Self::Boolean(boolean) => boolean.fmt(formatter),
            Self::Integer(integer) => integer.fmt(formatter),
            Self::Float(float) => float.fmt(formatter),
            Self::Double(double) => double.fmt(formatter),
            Self::Decimal(decimal) => decimal.fmt(formatter),
            Self::Text(text) => formatter.write_str(text),
            Self::Bytes(bytes) => hexadecimal(bytes, formatter),
            Self::Date(date) => date.fmt(formatter),
            Self::Time(time) => time.fmt(formatter),
            Self::Timestamp(timestamp) => timestamp.fmt(formatter),
            Self::Undecoded(undecoded) => hexadecimal(undecoded.bytes(), formatter),
        }
    }
}

fn hexadecimal(bytes: &[u8], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_print_as_hex_and_null_as_a_word() {
        let bytes = vec![0, 0xab, 0x10];
        assert_eq!(Value::Bytes(bytes.clone()).to_string(), "00ab10");
        assert_eq!(Value::Null.to_string(), "NULL");
        let undecoded = Value::Undecoded(Undecoded::new(99, bytes.clone()));
        assert_eq!(undecoded.to_string(), "00ab10");
        assert_eq!(
            format!("{:?} {undecoded:?}", Value::Bytes(bytes)),
            "Bytes(3 bytes) Undecoded(domain 99, 3 bytes)"
        );
    }

    #[test]
    fn a_value_is_made_of_what_it_reads_as() {
        assert_eq!(Value::from(true), Value::Boolean(true));
        assert_eq!(Value::from(-3_i16), Value::Integer(-3));
        assert_eq!(Value::from(u64::MAX), Value::Integer(u64::MAX.into()));
        assert_eq!(Value::from(0.5_f32), Value::Float(0.5));
        assert_eq!(Value::from(0.5), Value::Double(0.5));
        assert_eq!(Value::from("a"), Value::Text("a".to_owned()));
        assert_eq!(Value::from(&[1_u8][..]), Value::Bytes(vec![1]));
        assert_eq!(Value::from(Some(1_u8)), Value::Integer(1));
        assert_eq!(Value::from(None::<String>), Value::Null);
    }

    #[test]
    fn a_whole_number_is_read_in_the_width_that_holds_it() {
        let largest = Value::Integer(u64::MAX.into());
        assert_eq!((largest.as_u64(), largest.as_i64()), (Some(u64::MAX), None));
        assert_eq!(largest.as_integer(), Some(u64::MAX.into()));
        let below = Value::Integer(-7);
        assert_eq!((below.as_u64(), below.as_i64()), (None, Some(-7)));
        assert_eq!(Value::Boolean(true).as_i64(), None);
    }

    #[test]
    fn each_accessor_reads_its_own_type() {
        let number: Decimal = "1.50".parse().unwrap();
        assert_eq!(Value::Decimal(number).as_decimal(), Some(number));
        assert_eq!(Value::Boolean(true).as_bool(), Some(true));
        assert_eq!(Value::Float(0.5).as_f32(), Some(0.5));
        assert_eq!(Value::Float(0.5).as_f64(), Some(0.5));
        assert_eq!(Value::Double(0.5).as_f32(), None);
        assert_eq!(Value::Text("a".to_owned()).as_str(), Some("a"));
        assert_eq!(Value::Bytes(vec![1]).as_bytes(), Some(&[1][..]));
        let day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let noon = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        assert_eq!(Value::Date(day).as_date(), Some(day));
        assert_eq!(Value::Time(noon).as_time(), Some(noon));
        assert_eq!(
            Value::Timestamp(day.and_time(noon)).as_timestamp(),
            Some(day.and_time(noon))
        );
        let undecoded = Value::Undecoded(Undecoded::new(36, vec![1]));
        assert_eq!(undecoded.as_undecoded().map(Undecoded::domain), Some(36));
        assert!(Value::Null.is_null() && Value::Null.as_date().is_none());
    }
}
