//! The key of a whole file.
//!
//! Every page's key comes from its number and two constants of the file:
//! `(number XOR mask) + addend`, kept to 25 bits. The file stores neither
//! constant.

use std::fmt;

use crate::cipher::page_key::PageKey;

/// Bits of a page key that the cipher reads.
pub(crate) const KEY_BITS: u32 = 25;
pub(crate) const KEY_MASK: u32 = (1 << KEY_BITS) - 1;

/// Which way a file's page keys turn the sector walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeySign {
    /// Page keys are positive.
    Positive,
    /// Page keys are negative.
    Negative,
}

impl fmt::Display for KeySign {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Positive => "positive",
            Self::Negative => "negative",
        })
    }
}

/// The two constants and the sign that give every page of a file its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreKey {
    mask: u32,
    addend: u32,
    sign: KeySign,
}

impl StoreKey {
    /// A key from its parts. Only the low 25 bits of each constant count.
    #[must_use]
    pub const fn new(mask: u32, addend: u32, sign: KeySign) -> Self {
        Self {
            mask: mask & KEY_MASK,
            addend: addend & KEY_MASK,
            sign,
        }
    }

    /// The constant that is XORed with a page number.
    #[must_use]
    pub const fn mask(self) -> u32 {
        self.mask
    }

    /// The constant that is added after the XOR.
    #[must_use]
    pub const fn addend(self) -> u32 {
        self.addend
    }

    /// The sign of every page key.
    #[must_use]
    pub const fn sign(self) -> KeySign {
        self.sign
    }

    /// The key of the page at position `page`.
    #[must_use]
    pub fn page_key(self, page: u32) -> PageKey {
        let word = (page ^ self.mask).wrapping_add(self.addend) & KEY_MASK;
        PageKey::new(match self.sign {
            KeySign::Positive => (!word & KEY_MASK) as i32,
            KeySign::Negative => -(word as i32),
        })
    }

    /// Whether both keys walk every sector of each of `pages` the same way.
    pub(crate) fn walks_like(self, other: Self, pages: &[u32]) -> bool {
        pages
            .iter()
            .all(|&page| self.page_key(page).walks() == other.page_key(page).walks())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_low_bits_of_the_constants_count() {
        let key = StoreKey::new(u32::MAX, u32::MAX, KeySign::Negative);
        assert_eq!(key.mask(), KEY_MASK);
        assert_eq!(key.addend(), KEY_MASK);
    }

    // The first sector reads neither bit 16 of a key nor the bit it forces
    // to one, but the second sector does.
    #[test]
    fn keys_that_share_a_first_sector_can_differ_in_the_second() {
        let key = StoreKey::new(0, 0x0012_3456, KeySign::Negative);
        let twin = StoreKey::new(0, 0x0013_3456, KeySign::Negative);
        let (walks, twin_walks) = (key.page_key(0).walks(), twin.page_key(0).walks());
        assert_eq!(walks[0], twin_walks[0]);
        assert_ne!(walks[1], twin_walks[1]);
        assert!(!key.walks_like(twin, &[0]));
    }
}
