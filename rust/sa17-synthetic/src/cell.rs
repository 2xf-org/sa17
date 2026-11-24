//! One field of a synthetic row.

use crate::calendar;
use crate::scramble::Sign;

const MINUTES_PER_DAY: u32 = 24 * 60;

/// How a long value is kept outside its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongStorage {
    /// One piece on an extension page.
    OnePiece,
    /// Two linked pieces on extension pages.
    Chained,
    /// So many linked pieces on extension pages, at least one.
    Pieces(usize),
    /// One piece that holds the value compressed.
    Compressed,
    /// Two linked pieces that hold the value compressed.
    ChainedCompressed,
    /// One piece that holds the value in two blocks, of which one is stored as
    /// it is.
    StoredBlocks,
}

/// The value of one column in one row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    /// SQL `NULL`. The column must be nullable.
    Null,
    /// The bytes of the field exactly as a row stores them.
    Stored(Vec<u8>),
    /// A string or binary value kept outside the row.
    Long {
        /// The complete value.
        value: Vec<u8>,
        /// How the value is kept.
        storage: LongStorage,
    },
}

impl Cell {
    /// A `SMALLINT`.
    #[must_use]
    pub fn small_int(value: i16) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// An `INTEGER`.
    #[must_use]
    pub fn integer(value: i32) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// A `BIGINT`.
    #[must_use]
    pub fn big_int(value: i64) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// A `TINYINT`.
    #[must_use]
    pub fn tiny_int(value: u8) -> Self {
        Self::Stored(vec![value])
    }

    /// An `UNSIGNED INT`.
    #[must_use]
    pub fn unsigned_int(value: u32) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// An `UNSIGNED BIGINT`.
    #[must_use]
    pub fn unsigned_big_int(value: u64) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// A `BIT`.
    #[must_use]
    pub fn bit(value: bool) -> Self {
        Self::Stored(vec![value.into()])
    }

    /// A `FLOAT`.
    #[must_use]
    pub fn float(value: f32) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// A `DOUBLE`.
    #[must_use]
    pub fn double(value: f64) -> Self {
        Self::Stored(value.to_le_bytes().to_vec())
    }

    /// A `DATE`, as minutes since 29 February 1600.
    #[must_use]
    pub fn date(minutes: u32) -> Self {
        Self::Stored(minutes.to_le_bytes().to_vec())
    }

    /// A `DATE`, from a day written as `2024-01-31`.
    ///
    /// # Panics
    ///
    /// Panics when `text` is not a day of 1601 or later.
    #[must_use]
    pub fn day(text: &str) -> Self {
        let days = calendar::days(text);
        Self::date(days * MINUTES_PER_DAY)
    }

    /// A `TIMESTAMP` or a `TIME`: minutes, then microseconds past the minute.
    #[must_use]
    pub fn timestamp(minutes: u32, microseconds: u32) -> Self {
        let mut stored = minutes.to_le_bytes().to_vec();
        stored.extend_from_slice(&microseconds.to_le_bytes());
        Self::Stored(stored)
    }

    /// A `NUMERIC`: base-100 digits, least significant first, times one
    /// hundred to the power of `exponent`.
    #[must_use]
    pub fn numeric(sign: Sign, exponent: i8, digits: &[u8]) -> Self {
        let sign = match sign {
            Sign::Positive => 0x80,
            Sign::Negative => 0,
        };
        let mut stored = vec![digits.len() as u8, sign | (exponent + 64) as u8];
        stored.extend_from_slice(digits);
        Self::Stored(stored)
    }

    /// A `NUMERIC`, from a number written as `-1234.50`.
    ///
    /// # Panics
    ///
    /// Panics when `text` is not a number, or has more than 100 digits.
    #[must_use]
    pub fn decimal(text: &str) -> Self {
        let (sign, digits) = match text.strip_prefix('-') {
            Some(digits) => (Sign::Negative, digits),
            None => (Sign::Positive, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        // Two decimal digits make one of base 100, counted from the point.
        let whole = format!("{}{whole}", "0".repeat(whole.len() % 2));
        let fraction = format!("{fraction}{}", "0".repeat(fraction.len() % 2));
        let pairs = format!("{whole}{fraction}");
        let pairs = pairs.as_bytes().chunks(2).rev();
        let pairs: Vec<u8> = pairs
            .map(|pair| {
                let pair = str::from_utf8(pair).expect("a number is written in ASCII");
                pair.parse().expect("a number is written with digits")
            })
            .collect();
        let exponent = i8::try_from(fraction.len() / 2).expect("a number has 100 digits at most");
        // A number has no zeros at either end, and zero has no digits.
        let zeros = pairs.iter().take_while(|&&pair| pair == 0).count();
        let (zeros, digits) = match pairs.iter().rposition(|&pair| pair != 0) {
            Some(last) => (zeros as i8, &pairs[zeros..=last]),
            None => return Self::numeric(Sign::Positive, 0, &[]),
        };
        Self::numeric(sign, zeros - exponent, digits)
    }

    /// A string or binary value kept in the row. Text is the bytes of its
    /// Windows-1252 form.
    ///
    /// # Panics
    ///
    /// Panics when the value has 65536 bytes or more.
    #[must_use]
    pub fn bytes(value: &[u8]) -> Self {
        let mut stored = match u8::try_from(value.len()) {
            Ok(short) if short < 0x80 => vec![short],
            _ => {
                let length = u16::try_from(value.len()).expect("an inline value fits 16 bits");
                let mut marker = vec![0xff];
                marker.extend_from_slice(&length.to_be_bytes());
                marker
            }
        };
        stored.extend_from_slice(value);
        Self::Stored(stored)
    }

    /// An ASCII string kept in the row.
    #[must_use]
    pub fn text(value: &str) -> Self {
        Self::bytes(value.as_bytes())
    }

    /// A string or binary value kept outside the row.
    #[must_use]
    pub fn long(value: &[u8], storage: LongStorage) -> Self {
        Self::Long {
            value: value.to_vec(),
            storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decimal_is_stored_in_base_100() {
        let numeric = |sign, exponent, digits: &[u8]| Cell::numeric(sign, exponent, digits);
        assert_eq!(
            Cell::decimal("-9999.99"),
            numeric(Sign::Negative, -1, &[99, 99, 99])
        );
        assert_eq!(
            Cell::decimal("123456.00"),
            numeric(Sign::Positive, 0, &[56, 34, 12])
        );
        assert_eq!(Cell::decimal("202"), numeric(Sign::Positive, 0, &[2, 2]));
        assert_eq!(Cell::decimal("100"), numeric(Sign::Positive, 1, &[1]));
        assert_eq!(Cell::decimal("0.5"), numeric(Sign::Positive, -1, &[50]));
        assert_eq!(
            Cell::decimal("1.105"),
            numeric(Sign::Positive, -2, &[50, 10, 1])
        );
        assert_eq!(Cell::decimal("-0.00"), numeric(Sign::Positive, 0, &[]));
    }

    #[test]
    fn a_day_is_stored_in_minutes() {
        assert_eq!(Cell::day("1604-02-29"), Cell::date(1461 * 24 * 60));
        assert_eq!(Cell::day("2000-01-02"), Cell::date(210_296_160));
        assert_eq!(Cell::day("2024-01-01"), Cell::date(222_917_760));
    }
}
