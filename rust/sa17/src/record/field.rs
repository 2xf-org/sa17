//! The fields of a record as values.

use crate::calendar;
use crate::decimal::Decimal;
use crate::error::RowProblem;
use crate::schema::{Column, DataType};
use crate::store::TextEncoding;
use crate::value::{Undecoded, Value};

/// The bit of a number's control byte that is set for a positive number.
const POSITIVE: u8 = 0x80;

/// What the control byte's other bits have added to the exponent.
const EXPONENT_BIAS: i16 = 64;

/// The largest base-100 digit.
const LARGEST_DIGIT: u8 = 99;

/// The value that the complete bytes of a field hold, other than a number's.
/// Bytes that no value of the column's type is stored as stay as they are.
pub(crate) fn value(
    bytes: &[u8],
    column: &Column,
    encoding: TextEncoding,
) -> Result<Value, RowProblem> {
    Ok(match column.data_type() {
        DataType::SmallInt => Value::Integer(i16::from_le_bytes(array(bytes)?).into()),
        DataType::Integer => Value::Integer(i32::from_le_bytes(array(bytes)?).into()),
        DataType::BigInt => Value::Integer(i64::from_le_bytes(array(bytes)?).into()),
        DataType::TinyInt => Value::Integer(u8::from_le_bytes(array(bytes)?).into()),
        DataType::UnsignedSmallInt => Value::Integer(u16::from_le_bytes(array(bytes)?).into()),
        DataType::UnsignedInt => Value::Integer(u32::from_le_bytes(array(bytes)?).into()),
        DataType::UnsignedBigInt => Value::Integer(u64::from_le_bytes(array(bytes)?).into()),
        DataType::Bit => match array(bytes)? {
            [0] => Value::Boolean(false),
            [1] => Value::Boolean(true),
            _ => undecoded(column, bytes),
        },
        DataType::Float => Value::Float(f32::from_le_bytes(array(bytes)?)),
        DataType::Double => Value::Double(f64::from_le_bytes(array(bytes)?)),
        DataType::Date => {
            let date = calendar::date(u32::from_le_bytes(array(bytes)?));
            date.map_or_else(|| undecoded(column, bytes), Value::Date)
        }
        DataType::Time => {
            let (minutes, microseconds) = minute_and_microseconds(bytes)?;
            let time = calendar::time(minutes, microseconds);
            time.map_or_else(|| undecoded(column, bytes), Value::Time)
        }
        DataType::Timestamp => {
            let (minutes, microseconds) = minute_and_microseconds(bytes)?;
            let timestamp = calendar::timestamp(minutes, microseconds);
            timestamp.map_or_else(|| undecoded(column, bytes), Value::Timestamp)
        }
        DataType::Char | DataType::VarChar | DataType::LongVarChar => {
            Value::Text(encoding.decode(bytes))
        }
        DataType::Binary | DataType::VarBinary | DataType::LongBinary | DataType::LongVarBit => {
            Value::Bytes(bytes.to_vec())
        }
        DataType::Numeric | DataType::Decimal | DataType::Unknown(_) => {
            return Err(RowProblem::UnsupportedDomain {
                column_id: column.id,
                domain: column.domain,
            });
        }
    })
}

/// The same for a value that was kept outside its row, whose bytes are its
/// own. `None` when there is no memory for the text they are.
pub(crate) fn long_value(bytes: Vec<u8>, column: &Column, encoding: TextEncoding) -> Option<Value> {
    if column.data_type().is_text() {
        encoding.try_decode(bytes).map(Value::Text)
    } else {
        Some(Value::Bytes(bytes))
    }
}

/// A number, or what its row stores for one that is none, or has more digits
/// than fit.
pub(crate) fn number(control: u8, digits: &[u8], column: &Column) -> Value {
    let are_digits = digits.iter().all(|&digit| digit <= LARGEST_DIGIT);
    let exponent = i16::from(control & !POSITIVE) - EXPONENT_BIAS;
    let negative = control & POSITIVE == 0;
    let decimal = are_digits
        .then(|| Decimal::from_base_100(digits, exponent, negative, column.scale))
        .flatten();
    decimal.map_or_else(
        || {
            let mut stored = vec![digits.len() as u8, control];
            stored.extend_from_slice(digits);
            undecoded(column, &stored)
        },
        Value::Decimal,
    )
}

fn undecoded(column: &Column, stored: &[u8]) -> Value {
    Value::Undecoded(Undecoded::new(column.domain, stored.to_vec()))
}

fn minute_and_microseconds(bytes: &[u8]) -> Result<(u32, u32), RowProblem> {
    let (minutes, microseconds) = bytes.split_at_checked(4).ok_or(RowProblem::Truncated)?;
    Ok((
        u32::from_le_bytes(array(minutes)?),
        u32::from_le_bytes(array(microseconds)?),
    ))
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], RowProblem> {
    bytes.try_into().map_err(|_| RowProblem::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(domain: u16) -> Column {
        Column {
            id: 1,
            name: "amount".to_owned(),
            domain,
            width: 15,
            scale: 2,
            nullable: false,
            default: None,
            first_schema: 0,
            absent: Some(Value::Null),
        }
    }

    fn read(domain: u16, bytes: &[u8]) -> Value {
        value(bytes, &column(domain), TextEncoding::Windows1252).unwrap()
    }

    // The exponent counts base-100 digits and is stored with 64 added.
    #[test]
    fn the_control_byte_holds_the_sign_and_the_exponent() {
        let decimal = |control, digits: &[u8]| number(control, digits, &column(3)).to_string();
        assert_eq!(decimal(63, &[99, 99, 99]), "-9999.99");
        assert_eq!(decimal(POSITIVE | 64, &[56, 34, 12]), "123456.00");
        assert_eq!(decimal(POSITIVE | 65, &[1]), "100.00");
        assert_eq!(decimal(1, &[]), "0.00");
        assert_eq!(decimal(POSITIVE | 64, &[99]), "99.00");
    }

    #[test]
    fn a_number_that_is_none_or_too_long_stays_as_stored() {
        for (control, digits) in [(POSITIVE | 64, &[99; 30][..]), (64, &[1, 100])] {
            let number = number(control, digits, &column(27));
            let undecoded = number.as_undecoded().unwrap();
            assert_eq!(undecoded.domain(), 27);
            let (start, rest) = undecoded.bytes().split_at(2);
            assert_eq!((start, rest), (&[digits.len() as u8, control][..], digits));
        }
    }

    // A day has 1440 minutes, and a minute 60 million microseconds.
    #[test]
    fn what_is_no_time_and_no_bit_stays_as_stored() {
        let late = [0xa0, 5, 0, 0, 0, 0, 0, 0];
        let full = [1, 0, 0, 0, 0, 0x87, 0x93, 3];
        let last = [1, 0, 0, 0, 0xff, 0x86, 0x93, 3];
        let stored: [(u16, &[u8]); 6] = [
            (6, &[1, 0, 0, 0]),
            (6, &[0xa1, 5, 0, 0]),
            (13, &full),
            (14, &full),
            (14, &late),
            (24, &[2]),
        ];
        for (domain, bytes) in stored {
            let read = read(domain, bytes);
            let undecoded = read.as_undecoded().unwrap();
            assert_eq!((undecoded.domain(), undecoded.bytes()), (domain, bytes));
        }
        assert!(read(13, &last).as_timestamp().is_some());
        assert!(read(14, &last).as_time().is_some());
        assert!(read(13, &late).as_timestamp().is_some());
        assert!(read(6, &[0xa0, 5, 0, 0]).as_date().is_some());
        assert_eq!(read(24, &[1]), Value::Boolean(true));
        assert_eq!(read(24, &[0]), Value::Boolean(false));
    }

    #[test]
    fn every_integer_type_reads_as_one_kind_of_number() {
        let seven = Value::Integer(7);
        assert_eq!(read(1, &[7, 0]), seven);
        assert_eq!(read(2, &[7, 0, 0, 0]), seven);
        assert_eq!(read(19, &[7]), seven);
        assert_eq!(read(20, &[7, 0, 0, 0, 0, 0, 0, 0]), seven);
        assert_eq!(read(22, &[7, 0]), seven);
        assert_eq!(read(23, &[7, 0, 0, 0, 0, 0, 0, 0]), seven);
        assert_eq!(read(1, &[0xff, 0xff]), Value::Integer(-1));
        assert_eq!(read(22, &[0xff, 0xff]), Value::Integer(0xffff));
    }

    #[test]
    fn a_field_of_the_wrong_size_is_refused() {
        for domain in [1, 2, 4, 5, 6, 13, 14, 19, 20] {
            let wrong = value(&[1, 2, 3], &column(domain), TextEncoding::Windows1252);
            assert_eq!(wrong, Err(RowProblem::Truncated), "domain {domain}");
        }
    }
}
