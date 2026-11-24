//! Products of more digits than 128 bits hold, on the way to a result that fits.

const LIMB_BITS: u32 = 64;

/// The largest power of ten that a limb holds is ten to the power of this.
const LIMB_DIGITS: u32 = 19;

/// The exact product of two whole numbers.
pub(crate) struct WideProduct {
    /// The least significant limb comes first.
    limbs: [u64; 4],
}

impl WideProduct {
    pub(crate) fn new(left: u128, right: u128) -> Self {
        let mut limbs = [0; 4];
        for (shift, left) in halves(left).into_iter().enumerate() {
            let mut carry = 0;
            for (place, right) in (shift..).zip(halves(right)) {
                // The largest factors, limb and carry add up to 128 bits of ones.
                let sum = u128::from(left) * u128::from(right) + u128::from(limbs[place]) + carry;
                limbs[place] = sum as u64;
                carry = sum >> LIMB_BITS;
            }
            limbs[shift + 2] = carry as u64;
        }
        Self { limbs }
    }

    /// The product over ten to the power of `places`, as the nearest whole
    /// number, where half goes up. `None` when 128 bits do not hold it.
    pub(crate) fn rounded(mut self, places: u32) -> Option<u128> {
        let Some(mut left) = places.checked_sub(1) else {
            return self.whole();
        };
        while left > 0 {
            let digits = left.min(LIMB_DIGITS);
            self.divide(10_u64.pow(digits));
            left -= digits;
        }
        let last_digit = self.divide(10);
        self.whole()?.checked_add((last_digit >= 5).into())
    }

    /// Divides in place and gives the remainder.
    fn divide(&mut self, divisor: u64) -> u64 {
        let mut remainder = 0;
        for limb in self.limbs.iter_mut().rev() {
            let dividend = u128::from(remainder) << LIMB_BITS | u128::from(*limb);
            *limb = (dividend / u128::from(divisor)) as u64;
            remainder = (dividend % u128::from(divisor)) as u64;
        }
        remainder
    }

    fn whole(&self) -> Option<u128> {
        let [low, high, 0, 0] = self.limbs else {
            return None;
        };
        Some(u128::from(high) << LIMB_BITS | u128::from(low))
    }
}

/// The low and the high limb of a number.
fn halves(number: u128) -> [u64; 2] {
    [number as u64, (number >> LIMB_BITS) as u64]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_product_of_256_bits_is_exact() {
        // (2^128 - 1)^2 is 2^256 - 2^129 + 1.
        let product = WideProduct::new(u128::MAX, u128::MAX);
        assert_eq!(product.limbs, [1, 0, u64::MAX - 1, u64::MAX]);
        let crossed = WideProduct::new(u128::from(u64::MAX) + 2, 3);
        assert_eq!(crossed.whole(), Some(3 * (u128::from(u64::MAX) + 2)));
        assert_eq!(WideProduct::new(u128::MAX, 2).whole(), None);
    }

    #[test]
    fn a_power_of_ten_is_divided_away_to_the_nearest() {
        let rounded = |left, right, places| WideProduct::new(left, right).rounded(places);
        assert_eq!(rounded(2345, 1, 1), Some(235));
        assert_eq!(rounded(2344, 1, 1), Some(234));
        // 0.5 is not enough one digit up, whatever follows it.
        assert_eq!(rounded(4999, 1, 4), Some(0));
        assert_eq!(rounded(5000, 1, 4), Some(1));
        assert_eq!(rounded(7, 6, 0), Some(42));
        let power = 10_u128.pow(38);
        assert_eq!(rounded(power, power, 76), Some(1));
        assert_eq!(rounded(power, power, 38), Some(power));
        assert_eq!(rounded(power, power, 30), None);
        assert_eq!(rounded(power, power, 1000), Some(0));
        assert_eq!(rounded(u128::MAX, u128::MAX, 0), None);
    }
}
