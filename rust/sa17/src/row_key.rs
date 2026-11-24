//! The values that a row is looked up by.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::decimal::Decimal;
use crate::value::Value;

/// The values of a key, to look a row up by: it can be hashed and sorted, and
/// two are equal when their values are.
///
/// A whole number and a decimal are equal when they are the same number,
/// whatever its scale. So are a `FLOAT` and a `DOUBLE`, and a float equals no
/// decimal. Texts are equal when every character is, letter case included,
/// which a database's own collation can see otherwise.
///
/// ```no_run
/// use std::collections::HashMap;
///
/// use sa17::{Columns, Database, RowId, RowKey};
///
/// let database = Database::open("garden.db")?;
/// let plants = database.table("plants")?;
/// for key in plants.definition().foreign_keys() {
///     let Some(target) = plants.target(key) else {
///         continue;
///     };
///     let named = Columns::at(target.definition(), key.target_positions())?;
///     let mut rows: HashMap<RowKey, RowId> = HashMap::new();
///     for row in target.scan(&named) {
///         let row = row?;
///         rows.extend(row.key().map(|key| (key, row.id())));
///     }
///     let naming = Columns::at(plants.definition(), key.positions())?;
///     for row in plants.scan(&naming) {
///         let row = row?;
///         let target = row.key().and_then(|key| rows.get(&key));
///         println!("{}: {target:?}", row.id());
///     }
/// }
/// # Ok::<(), sa17::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowKey(Vec<Part>);

/// One value of a key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Part {
    Boolean(bool),
    Number(Number),
    Float(Float),
    Text(String),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    Timestamp(NaiveDateTime),
    Undecoded { domain: u16, bytes: Vec<u8> },
}

/// An exact number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Number {
    /// The one whole number that a decimal does not hold.
    Least,
    Other(Decimal),
}

/// The bits of a float, changed so that they sort as the floats do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Float(u64);

impl RowKey {
    /// The key of `values`, or `None` when one of them is `NULL`: such a key
    /// names no row.
    #[must_use]
    pub fn new<'value>(values: impl IntoIterator<Item = &'value Value>) -> Option<Self> {
        let parts = values.into_iter().map(part);
        parts.collect::<Option<_>>().map(Self)
    }
}

fn part(value: &Value) -> Option<Part> {
    Some(match value {
        Value::Null => return None,
        Value::Boolean(boolean) => Part::Boolean(*boolean),
        Value::Integer(integer) => Part::Number(match Decimal::new(*integer, 0) {
            Some(decimal) => Number::Other(decimal),
            None => Number::Least,
        }),
        Value::Decimal(decimal) => Part::Number(Number::Other(*decimal)),
        Value::Float(float) => Part::Float(Float::new((*float).into())),
        Value::Double(double) => Part::Float(Float::new(*double)),
        Value::Text(text) => Part::Text(text.clone()),
        Value::Bytes(bytes) => Part::Bytes(bytes.clone()),
        Value::Date(date) => Part::Date(*date),
        Value::Time(time) => Part::Time(*time),
        Value::Timestamp(timestamp) => Part::Timestamp(*timestamp),
        Value::Undecoded(undecoded) => Part::Undecoded {
            domain: undecoded.domain(),
            bytes: undecoded.bytes().to_vec(),
        },
    })
}

impl Float {
    fn new(float: f64) -> Self {
        const SIGN: u64 = 1 << 63;
        // The two zeros are equal, and so is every value that is no number.
        let one_of_each = if float == 0.0 {
            0.0
        } else if float.is_nan() {
            f64::NAN
        } else {
            float
        };
        let bits = one_of_each.to_bits();
        Self(if bits & SIGN == 0 { bits | SIGN } else { !bits })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::value::Undecoded;

    fn key(value: impl Into<Value>) -> Option<RowKey> {
        RowKey::new(&[value.into()])
    }

    fn decimal(text: &str) -> Value {
        Value::Decimal(text.parse().unwrap())
    }

    #[test]
    fn equal_numbers_make_equal_keys() {
        assert_eq!(key(decimal("1.50")), key(decimal("1.5")));
        assert_eq!(key(7), key(decimal("7.00")));
        assert_ne!(key(7), key(decimal("7.01")));
        assert_eq!(key(0.5_f32), key(0.5));
        assert_eq!(key(-0.0), key(0.0));
        assert_eq!(key(f64::NAN), key(-f64::NAN));
        assert_ne!(key(7), key(7.0));
        assert_ne!(key(7), key("7"));
        assert_ne!(key(i128::MIN), key(i128::MIN + 1));
        let same: HashSet<_> = [key(7), key(decimal("7.0")), key(7_u8)].into();
        assert_eq!(same.len(), 1);
    }

    #[test]
    fn a_key_with_null_is_none() {
        assert_eq!(key(Value::Null), None);
        assert_eq!(RowKey::new(&[Value::from(1), Value::Null]), None);
        let none: [&Value; 0] = [];
        assert_eq!(RowKey::new(none), Some(RowKey(Vec::new())));
    }

    #[test]
    fn keys_sort_as_their_values_do() {
        let sorted = [f64::NEG_INFINITY, -2.5, -0.5, 0.0, 0.5, 2.5, f64::INFINITY];
        let keys = sorted.map(key);
        assert!(keys.is_sorted());
        let numbers = [key(i128::MIN), key(-3), key(decimal("-2.5")), key(4)];
        assert!(numbers.is_sorted());
        assert!(key("a") < key("b") && key(vec![1_u8]) < key(vec![1_u8, 0]));
    }

    #[test]
    fn every_other_value_has_a_key_of_its_own() {
        let day = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let values = [
            Value::from(true),
            day.into(),
            NaiveTime::MIN.into(),
            day.and_time(NaiveTime::MIN).into(),
            Value::Undecoded(Undecoded::new(36, vec![1])),
            Value::Undecoded(Undecoded::new(37, vec![1])),
        ];
        let keys: HashSet<_> = values.iter().map(|value| key(value.clone())).collect();
        assert_eq!(keys.len(), values.len());
    }
}
