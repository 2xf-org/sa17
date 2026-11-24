//! Exact `NUMERIC` and `DECIMAL` values.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Neg;
use std::str::FromStr;

use crate::wide_product::WideProduct;

/// An exact decimal number: a whole number of units of a power of ten.
///
/// A number keeps its scale, the digits it has after the point, and prints
/// with them. The scale does not count for comparing: 1.5 and 1.50 are equal,
/// sort together and hash the same. Arithmetic is exact, and gives `None`
/// where the result has more digits than 128 bits hold, about 38. Only
/// [`Self::round`] and [`Self::checked_mul_round`] drop digits.
///
/// ```
/// use sa17::Decimal;
///
/// let price: Decimal = "19.90".parse()?;
/// let quantity = Decimal::from(3);
/// let total = price.checked_mul(quantity).ok_or("too large")?;
/// assert_eq!(total.to_string(), "59.70");
/// assert_eq!(total, "59.7".parse()?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
// An alignment of 16 would make every value of a row 16 bytes larger.
#[derive(Clone, Copy, Default)]
#[repr(Rust, packed(8))]
pub struct Decimal {
    /// Never `i128::MIN`, so that every number has a negative.
    mantissa: i128,
    scale: u16,
}

impl Decimal {
    /// The number `mantissa` units of `10^-scale`, or `None` for the one
    /// mantissa whose negative 128 bits do not hold.
    #[must_use]
    pub const fn new(mantissa: i128, scale: u16) -> Option<Self> {
        if mantissa == i128::MIN {
            None
        } else {
            Some(Self { mantissa, scale })
        }
    }

    /// A stored number: base-100 digits, least significant first, times one
    /// hundred to the power of `exponent`, with at least `scale` digits after
    /// the point. `None` when it has more digits than fit.
    pub(crate) fn from_base_100(
        digits: &[u8],
        exponent: i16,
        negative: bool,
        scale: u16,
    ) -> Option<Self> {
        let mut mantissa: i128 = 0;
        for &digit in digits.iter().rev() {
            mantissa = mantissa.checked_mul(100)?.checked_add(digit.into())?;
        }
        if mantissa == 0 {
            return Self::new(0, scale);
        }
        let mut places = -2 * i32::from(exponent);
        // A fraction longer than the scale loses zeros, never a digit that carries value.
        while places > i32::from(scale) && mantissa % 10 == 0 {
            mantissa /= 10;
            places -= 1;
        }
        let kept = places.max(scale.into());
        let padding = u32::try_from(kept - places).ok()?;
        mantissa = mantissa.checked_mul(10_i128.checked_pow(padding)?)?;
        let number = Self::new(mantissa, u16::try_from(kept).ok()?)?;
        Some(if negative { -number } else { number })
    }

    /// The number as a whole number of units of `10^-scale`.
    #[must_use]
    pub const fn mantissa(self) -> i128 {
        self.mantissa
    }

    /// The digits the number has after the point.
    #[must_use]
    pub const fn scale(self) -> u16 {
        self.scale
    }

    /// The digits a column needs to hold the number with its scale: the
    /// precision of a `NUMERIC(precision, scale)`.
    #[must_use]
    pub fn precision(self) -> u32 {
        let digits = self
            .mantissa
            .unsigned_abs()
            .checked_ilog10()
            .map_or(1, |power| power + 1);
        digits.max(self.scale.into())
    }

    /// Whether the number is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.mantissa == 0
    }

    /// Whether the number is below zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.mantissa < 0
    }

    /// The number without its sign.
    #[must_use]
    pub const fn abs(self) -> Self {
        Self {
            mantissa: self.mantissa.abs(),
            scale: self.scale,
        }
    }

    /// The same number with exactly `scale` digits after the point, or
    /// `None` when that would drop a digit other than zero or has too many
    /// digits.
    #[must_use]
    pub fn rescale(self, scale: u16) -> Option<Self> {
        if let Some(added) = scale.checked_sub(self.scale) {
            let units = match 10_i128.checked_pow(added.into()) {
                Some(power) => self.mantissa.checked_mul(power)?,
                None if self.is_zero() => 0,
                None => return None,
            };
            return Self::new(units, scale);
        }
        let Some(power) = 10_i128.checked_pow((self.scale - scale).into()) else {
            return self.is_zero().then_some(Self { mantissa: 0, scale });
        };
        (self.mantissa % power == 0).then_some(Self {
            mantissa: self.mantissa / power,
            scale,
        })
    }

    /// The number to the nearest with at most `scale` digits after the point.
    ///
    /// Half of the last digit goes away from zero, as it does on paper: 2.345
    /// to two digits is 2.35, and -2.345 is -2.35.
    #[must_use]
    pub fn round(self, scale: u16) -> Self {
        let Some(places) = self.scale.checked_sub(scale).filter(|&places| places > 0) else {
            return self;
        };
        let mantissa = match 10_i128.checked_pow(places.into()) {
            Some(power) => {
                let away = (self.mantissa % power).abs() >= power / 2;
                let nearest = if away { self.mantissa.signum() } else { 0 };
                self.mantissa / power + nearest
            }
            // Less than half of a power of ten that 128 bits do not hold.
            None => 0,
        };
        Self { mantissa, scale }
    }

    /// The same number without zeros at the end of its fraction.
    #[must_use]
    pub fn normalize(self) -> Self {
        if self.is_zero() {
            return Self::default();
        }
        let mut number = self;
        while number.scale > 0 && number.mantissa % 10 == 0 {
            number.mantissa /= 10;
            number.scale -= 1;
        }
        number
    }

    /// The sum, with the larger of the two scales.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let scale = self.scale.max(other.scale);
        let (left, right) = (self.rescale(scale)?, other.rescale(scale)?);
        Self::new(left.mantissa.checked_add(right.mantissa)?, scale)
    }

    /// The difference, with the larger of the two scales.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_add(-other)
    }

    /// The product, with the sum of the two scales.
    #[must_use]
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let scale = self.scale.checked_add(other.scale)?;
        if let Some(mantissa) = self.mantissa.checked_mul(other.mantissa) {
            return Self::new(mantissa, scale);
        }
        // Zeros at the end of a fraction can be what makes a product too long.
        let (left, right) = (self.normalize(), other.normalize());
        let mantissa = left.mantissa.checked_mul(right.mantissa)?;
        Self::new(mantissa, left.scale.checked_add(right.scale)?)
    }

    /// The product to the nearest with at most `scale` digits after the point,
    /// where half goes away from zero as in [`Self::round`].
    ///
    /// Only the rounded product has to fit: the exact one may have more digits
    /// than a decimal holds.
    #[must_use]
    pub fn checked_mul_round(self, other: Self, scale: u16) -> Option<Self> {
        let exact = u32::from(self.scale) + u32::from(other.scale);
        let Some(places) = exact.checked_sub(scale.into()) else {
            return self.checked_mul(other);
        };
        let units = WideProduct::new(self.mantissa.unsigned_abs(), other.mantissa.unsigned_abs());
        let units = i128::try_from(units.rounded(places)?).ok()?;
        let negative = self.is_negative() != other.is_negative();
        Self::new(if negative { -units } else { units }, scale)
    }

    /// The nearest `f64`. Numbers with more than fifteen digits lose precision.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        // The text is a sign, digits and at most one point, which always parse.
        self.to_string().parse().unwrap_or(f64::NAN)
    }

    /// Compares the numbers when `self` has the shorter fraction or neither does.
    fn compare_shorter(self, other: Self) -> Ordering {
        let added = other.scale - self.scale;
        let scaled = 10_i128
            .checked_pow(added.into())
            .and_then(|power| self.mantissa.checked_mul(power));
        match scaled {
            Some(scaled) => scaled.cmp(&other.mantissa()),
            // Past 128 bits, so further from zero than `other` can be.
            None if self.is_zero() => 0.cmp(&other.mantissa()),
            None => self.mantissa().cmp(&0),
        }
    }
}

impl Neg for Decimal {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            mantissa: -self.mantissa,
            scale: self.scale,
        }
    }
}

impl From<i64> for Decimal {
    fn from(whole: i64) -> Self {
        Self {
            mantissa: whole.into(),
            scale: 0,
        }
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.scale <= other.scale {
            self.compare_shorter(*other)
        } else {
            other.compare_shorter(*self).reverse()
        }
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Decimal {}

impl Hash for Decimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let number = self.normalize();
        number.mantissa().hash(state);
        number.scale().hash(state);
    }
}

/// Why a text is not a decimal number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseDecimalError {
    /// The text is not written as a number is.
    #[error("a decimal is digits with at most one point, and may start with a minus sign")]
    Syntax,
    /// The number has more digits than fit.
    #[error("the number has more digits than a decimal holds")]
    TooLong,
}

impl FromStr for Decimal {
    type Err = ParseDecimalError;

    /// Reads a number such as `-9999.99`. Its scale is the number of digits
    /// after the point. A plus sign is allowed, and a zero has no sign.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(digits) => (true, digits),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (integer, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if integer.is_empty() || !all_digits(integer) || !all_digits(fraction) {
            return Err(ParseDecimalError::Syntax);
        }
        if digits.contains('.') && fraction.is_empty() {
            return Err(ParseDecimalError::Syntax);
        }
        let scale = u16::try_from(fraction.len()).map_err(|_| ParseDecimalError::TooLong)?;
        let mut mantissa: i128 = 0;
        for digit in integer.bytes().chain(fraction.bytes()) {
            mantissa = mantissa
                .checked_mul(10)
                .and_then(|mantissa| mantissa.checked_add((digit - b'0').into()))
                .ok_or(ParseDecimalError::TooLong)?;
        }
        let number = Self { mantissa, scale };
        Ok(if negative { -number } else { number })
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = self.mantissa.unsigned_abs().to_string();
        let scale = usize::from(self.scale);
        if scale > 0 {
            if text.len() <= scale {
                text.insert_str(0, &"0".repeat(scale + 1 - text.len()));
            }
            text.insert(text.len() - scale, '.');
        }
        formatter.pad_integral(!self.is_negative(), "", &text)
    }
}

impl fmt::Debug for Decimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Decimal({self})")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const LARGEST: &str = "170141183460469231731687303715884105727";

    fn decimal(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn stored(digits: &[u8], exponent: i16, scale: u16) -> Option<String> {
        let number = Decimal::from_base_100(digits, exponent, false, scale)?;
        Some(number.to_string())
    }

    #[test]
    fn base_100_digits_are_least_significant_first() {
        assert_eq!(stored(&[99, 99, 99], -1, 2).unwrap(), "9999.99");
        assert_eq!(stored(&[56, 34, 12], 0, 2).unwrap(), "123456.00");
        assert_eq!(stored(&[1], 1, 0).unwrap(), "100");
        assert_eq!(stored(&[1], -3, 0).unwrap(), "0.000001");
        let negative = Decimal::from_base_100(&[99, 99, 99], -1, true, 2);
        assert_eq!(negative.unwrap().to_string(), "-9999.99");
    }

    #[test]
    fn the_scale_pads_and_trims_only_zeros() {
        assert_eq!(stored(&[50, 1], -1, 5).unwrap(), "1.50000");
        assert_eq!(stored(&[50, 1], -1, 1).unwrap(), "1.5");
        assert_eq!(stored(&[25, 50, 1], -2, 1).unwrap(), "1.5025");
        assert_eq!(stored(&[50, 1], -1, 0).unwrap(), "1.5");
    }

    #[test]
    fn a_stored_zero_has_no_sign_and_keeps_its_scale() {
        assert_eq!(stored(&[], -63, 2).unwrap(), "0.00");
        let zero = Decimal::from_base_100(&[0, 0], -1, true, 2).unwrap();
        assert_eq!(zero.to_string(), "0.00");
    }

    #[test]
    fn a_stored_number_of_more_digits_than_fit_is_not_rounded() {
        assert!(stored(&[99; 19], 0, 0).is_some());
        assert_eq!(stored(&[99; 20], 0, 0), None);
        assert_eq!(stored(&[1], 30, 0), None);
        assert_eq!(stored(&[1], 0, 60), None);
    }

    #[test]
    fn the_scale_does_not_count_for_comparing() {
        assert_eq!(decimal("1.5"), decimal("1.50"));
        assert_eq!(decimal("0.00"), decimal("-0"));
        assert!(decimal("1.5") < decimal("1.51") && decimal("-1.5") > decimal("-1.51"));
        assert!(decimal("2") > decimal("1.999999"));
        let same: HashSet<Decimal> = ["7", "7.0", "7.000"].map(decimal).into();
        assert_eq!(same.len(), 1);
        let zeros: HashSet<Decimal> = ["0", "0.0"].map(decimal).into();
        assert_eq!(zeros.len(), 1);
    }

    #[test]
    fn numbers_too_far_apart_to_rescale_still_compare() {
        let (large, small) = (
            decimal(LARGEST),
            decimal("0.00000000000000000000000000000000000001"),
        );
        assert!(large > small && -large < small && -large < -small);
        let tiny = Decimal::new(1, u16::MAX).unwrap();
        assert!(Decimal::default() < tiny && tiny < decimal("1") && -tiny > decimal("-1"));
    }

    #[test]
    fn sums_and_products_are_exact_or_none() {
        assert_eq!(
            decimal("0.1").checked_add(decimal("0.20")),
            Some(decimal("0.30"))
        );
        assert_eq!(
            decimal("1")
                .checked_sub(decimal("0.01"))
                .unwrap()
                .to_string(),
            "0.99"
        );
        assert_eq!(
            decimal("-1.5")
                .checked_mul(decimal("2.50"))
                .unwrap()
                .to_string(),
            "-3.750"
        );
        assert_eq!(decimal(LARGEST).checked_add(decimal("1")), None);
        assert_eq!((-decimal(LARGEST)).checked_sub(decimal("1")), None);
        assert_eq!(decimal(LARGEST).checked_mul(decimal("10")), None);
        // The zeros of the fractions are what would not fit.
        let padded = decimal("10000000000.0000000000");
        assert_eq!(
            padded.checked_mul(padded),
            Some(decimal("100000000000000000000"))
        );
        assert_eq!(Decimal::new(i128::MIN, 0), None);
        assert_eq!((-decimal("5")).abs(), decimal("5"));
    }

    #[test]
    fn a_scale_pads_but_never_drops_a_digit() {
        let number = decimal("1.5");
        assert_eq!(number.rescale(2).unwrap().to_string(), "1.50");
        assert_eq!(number.rescale(1).unwrap().to_string(), "1.5");
        assert_eq!(number.rescale(0), None);
        assert_eq!(number.rescale(60), None);
        assert_eq!(decimal("12.00").rescale(0).unwrap().to_string(), "12");
        assert_eq!(decimal("0.00").rescale(60).unwrap().scale(), 60);
        assert_eq!(decimal("12.00").normalize().scale(), 0);
        assert_eq!(decimal("100").normalize().to_string(), "100");
    }

    #[test]
    fn rounding_goes_to_the_nearest_and_half_away_from_zero() {
        let rounded = |text: &str, scale| decimal(text).round(scale).to_string();
        assert_eq!(rounded("2.345", 2), "2.35");
        assert_eq!(rounded("-2.345", 2), "-2.35");
        assert_eq!(rounded("2.335", 2), "2.34");
        assert_eq!(rounded("2.3449", 2), "2.34");
        assert_eq!(rounded("-0.4", 0), "0");
        assert_eq!(rounded("99.995", 2), "100.00");
        // A number that is short enough already keeps its scale.
        assert_eq!(rounded("2.5", 2), "2.5");
        assert_eq!(rounded("2.50", 2), "2.50");
        assert_eq!(decimal(LARGEST).round(0), decimal(LARGEST));
    }

    #[test]
    fn rounding_works_at_every_length() {
        let shifted = |scale| Decimal::new(i128::MAX, scale).unwrap();
        // 1.7 and 0.17, as whole numbers.
        assert_eq!(shifted(38).round(0), decimal("2"));
        assert_eq!(shifted(39).round(0), decimal("0"));
        assert_eq!((-shifted(38)).round(0), decimal("-2"));
        assert_eq!(shifted(u16::MAX).round(2).to_string(), "0.00");
    }

    #[test]
    fn a_rounded_product_may_come_from_a_longer_one() {
        let product = |left: &str, right: &str, scale| {
            let product = decimal(left).checked_mul_round(decimal(right), scale);
            product.map(|product| product.to_string())
        };
        assert_eq!(product("3", "1.105", 2).unwrap(), "3.32");
        assert_eq!(product("-3", "1.105", 2).unwrap(), "-3.32");
        assert_eq!(product("-3", "-1.105", 2).unwrap(), "3.32");
        assert_eq!(product("1.5", "0.5", 3).unwrap(), "0.75");
        assert_eq!(product("1.5", "0.5", 2).unwrap(), "0.75");
        let third = "0.33333333333333333333333333333333333333";
        assert_eq!(decimal(third).checked_mul(decimal(third)), None);
        assert_eq!(product(third, third, 5).unwrap(), "0.11111");
        assert_eq!(product(third, "3000", 2).unwrap(), "1000.00");
        assert_eq!(product(LARGEST, "10", 0), None);
        assert_eq!(product(LARGEST, "1.0", 0).unwrap(), LARGEST);
        assert_eq!(product(LARGEST, "2", 0), None);
        let tiny = Decimal::new(1, u16::MAX).unwrap();
        assert_eq!(tiny.checked_mul_round(tiny, 2), Some(decimal("0.00")));
    }

    #[test]
    fn the_precision_is_what_a_column_needs() {
        assert_eq!(decimal("12.00").precision(), 4);
        assert_eq!(decimal("0.50").precision(), 2);
        assert_eq!(decimal("0.005").precision(), 3);
        assert_eq!(decimal("0").precision(), 1);
    }

    #[test]
    fn text_parses_to_the_number_it_prints() {
        for text in ["-9999.99", "0", "0.00", "0.005", LARGEST] {
            assert_eq!(decimal(text).to_string(), text);
        }
        assert_eq!(decimal("007.50").to_string(), "7.50");
        assert_eq!(decimal("+12").to_string(), "12");
        assert_eq!(decimal("-0.00").to_string(), "0.00");
        let parse = |text: &str| text.parse::<Decimal>();
        for wrong in ["", "-", "1.", ".5", "1,5", "1e3", "12a", "--1"] {
            assert_eq!(parse(wrong), Err(ParseDecimalError::Syntax), "{wrong:?}");
        }
        assert_eq!(
            parse("170141183460469231731687303715884105728"),
            Err(ParseDecimalError::TooLong)
        );
        assert_eq!(
            parse(&format!("0.{}", "0".repeat(70_000))),
            Err(ParseDecimalError::TooLong)
        );
        // The longest fraction there can be prints without a panic.
        let tiny = Decimal::new(1, u16::MAX).unwrap();
        assert_eq!(tiny.to_string().len(), 2 + usize::from(u16::MAX));
    }

    #[test]
    fn a_width_and_a_sign_apply_to_the_text() {
        assert_eq!(
            format!("{:>8}|{:+}", decimal("-1.50"), decimal("2")),
            "   -1.50|+2"
        );
    }

    #[test]
    fn to_f64_reads_the_text() {
        assert!((decimal("-9999.99").to_f64() + 9999.99).abs() < 1e-9);
    }
}
