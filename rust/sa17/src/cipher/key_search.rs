//! The search for the keys that fit a file.
//!
//! Each page starts with its own number, and those four known bytes are
//! enough to find the walk of a page's first sector. Pages spread over the
//! file then pin both constants of the file's key bit by bit.
//!
//! The first sector cannot tell every key apart: the cipher ignores two bits
//! of a key there that it reads in other sectors. Large files pin those bits
//! through the carries of the addition. Small files may not, so the search
//! gives back every key that ties.

use crate::cipher::page_key::SectorWalk;
use crate::cipher::store_key::{KEY_BITS, KEY_MASK, KeySign, StoreKey};
use crate::page_size::{PAGE_SIZE, SECTOR_SIZE};

/// The share of samples a key may miss: one in fifty. A sampled page can be a
/// stale copy that happens to pass the first-sector test.
const MISSES_ALLOWED: usize = 50;

/// The most partial keys the search carries from one bit to the next. A real
/// file leaves a handful; more means the samples pin nothing.
const MOST_FITS: usize = 4096;

/// Stored pages that a key has to decode to the numbers they start with.
#[derive(Default)]
pub(crate) struct KeySearch {
    samples: Vec<Sample>,
}

/// A page whose first four bytes allow exactly one first-sector walk.
struct Sample {
    page: u32,
    walk: SectorWalk,
}

impl KeySearch {
    /// Takes the stored page at position `page` as a sample, if its first
    /// sector pins a walk.
    pub(crate) fn add(&mut self, page: u32, stored: &[u8; PAGE_SIZE]) {
        let mut walks = first_sector_walks(&stored[..SECTOR_SIZE], page);
        if let (Some(walk), None) = (walks.next(), walks.next()) {
            self.samples.push(Sample { page, walk });
        }
    }

    /// Every key that fits the samples best, or `None` when there are too many
    /// to tell apart.
    pub(crate) fn keys(&self) -> Option<Vec<StoreKey>> {
        let mut keys = Vec::new();
        for sign in [KeySign::Positive, KeySign::Negative] {
            let fits = fits(&self.samples, self.misses_allowed(), sign)?;
            let fits = fits.iter();
            keys.extend(fits.map(|fit| StoreKey::new(fit.mask, fit.addend, sign)));
        }
        Some(keys)
    }

    /// Whether `key` decodes as many of the samples as the file's own has to.
    pub(crate) fn allows(&self, key: StoreKey) -> bool {
        let misses = self.samples.iter().filter(|sample| {
            let [first, ..] = key.page_key(sample.page).walks();
            first != Some(sample.walk)
        });
        misses.count() <= self.misses_allowed()
    }

    /// How many samples a key may miss.
    fn misses_allowed(&self) -> usize {
        self.samples.len() / MISSES_ALLOWED
    }
}

/// Every walk that decodes the first four bytes of `sector` to `page`.
///
/// The walk writes position `k` at step `k / stride`, so the four known bytes
/// fix the seed and the increment for each stride, and check them twice.
fn first_sector_walks(sector: &[u8], page: u32) -> impl Iterator<Item = SectorWalk> + '_ {
    let known = page.to_le_bytes();
    (1..SECTOR_SIZE).step_by(2).filter_map(move |stride| {
        let step_of_one = inverse(stride, SECTOR_SIZE);
        let seed = known[0].wrapping_sub(sector[stride]);
        let increment = known[1]
            .wrapping_sub(sector[(1 + stride) % SECTOR_SIZE])
            .wrapping_sub(seed)
            .wrapping_mul(inverse(step_of_one % 256, 256) as u8);
        if increment.is_multiple_of(2) {
            return None;
        }
        let fits = (2..4).all(|position| {
            let step = (position * step_of_one % SECTOR_SIZE) as u8;
            let decoded = sector[(position + stride) % SECTOR_SIZE]
                .wrapping_add(seed)
                .wrapping_add(step.wrapping_mul(increment));
            decoded == known[position]
        });
        fits.then_some(SectorWalk {
            stride,
            increment,
            seed,
            clear_tail: 0,
        })
    })
}

/// The inverse of an odd number modulo a power of two, by Newton's iteration.
fn inverse(odd: usize, modulus: usize) -> usize {
    let mut inverse = odd;
    for _ in 0..4 {
        inverse = inverse.wrapping_mul(2_usize.wrapping_sub(odd.wrapping_mul(inverse)));
    }
    inverse % modulus
}

/// Every 25-bit word whose page key walks the first sector as `walk` does.
///
/// The cipher forces two bits of a key to one, so up to four words share a walk.
fn words(walk: SectorWalk, sign: KeySign) -> Vec<u32> {
    let (low, forward) = match sign {
        KeySign::Positive => (walk.seed, walk.stride),
        KeySign::Negative => (
            walk.increment.wrapping_sub(walk.seed),
            (SECTOR_SIZE - walk.stride) % SECTOR_SIZE,
        ),
    };
    let mut words = Vec::with_capacity(4);
    for middle in [walk.increment & !1, walk.increment] {
        for sum in [forward, forward - 1] {
            let high = (sum + SECTOR_SIZE - usize::from(low)) % SECTOR_SIZE;
            let magnitude = (high as u32) << 16 | u32::from(middle) << 8 | u32::from(low);
            words.push(match sign {
                KeySign::Positive => !magnitude & KEY_MASK,
                KeySign::Negative => magnitude,
            });
        }
    }
    words
}

#[derive(Clone, Copy)]
struct Constants {
    mask: u32,
    addend: u32,
}

impl Constants {
    /// Samples that none of their candidate words match in the given low bits.
    fn misses(self, candidates: &[(u32, Vec<u32>)], low_bits: u32) -> usize {
        candidates
            .iter()
            .filter(|(page, words)| {
                let word = (page ^ self.mask).wrapping_add(self.addend) & low_bits;
                !words.iter().any(|candidate| candidate & low_bits == word)
            })
            .count()
    }
}

/// Every pair of constants that misses the fewest samples under `sign`, and
/// no more than `allowed`. `None` when there are too many to go through.
///
/// The search grows both constants one bit at a time, from the lowest, and
/// keeps the combinations that still fit. Addition only carries upwards, so a
/// low bit never depends on a higher one.
fn fits(samples: &[Sample], allowed: usize, sign: KeySign) -> Option<Vec<Constants>> {
    let Some(highest) = samples.iter().map(|sample| sample.page).max() else {
        return Some(Vec::new());
    };
    // A mask bit above every page number only shifts the addend, so it stays zero.
    let page_bits = u32::BITS - highest.leading_zeros();
    let candidates: Vec<(u32, Vec<u32>)> = samples
        .iter()
        .map(|sample| (sample.page, words(sample.walk, sign)))
        .collect();

    let mut fits = vec![Constants { mask: 0, addend: 0 }];
    for bit in 0..KEY_BITS {
        let low_bits = (1 << (bit + 1)) - 1;
        let mask_bits: &[u32] = if bit < page_bits { &[0, 1] } else { &[0] };
        let mut grown = Vec::new();
        for constants in &fits {
            for mask_bit in mask_bits {
                for addend_bit in [0, 1] {
                    let next = Constants {
                        mask: constants.mask | mask_bit << bit,
                        addend: constants.addend | addend_bit << bit,
                    };
                    if next.misses(&candidates, low_bits) <= allowed {
                        grown.push(next);
                    }
                }
            }
        }
        if grown.len() > MOST_FITS {
            return None;
        }
        fits = grown;
    }

    let misses = |constants: &Constants| constants.misses(&candidates, KEY_MASK);
    let fewest = fits.iter().map(misses).min();
    fits.retain(|constants| Some(misses(constants)) == fewest);
    Some(fits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_gives_back_the_word_that_made_it() {
        for word in [1, 0x0012_3456, 0x01ab_cdef, KEY_MASK - 1] {
            for sign in [KeySign::Positive, KeySign::Negative] {
                let key = StoreKey::new(0, word, sign).page_key(0);
                let walk = SectorWalk::new(key.value(), 0).unwrap();
                assert!(
                    words(walk, sign).contains(&word),
                    "word {word:#x}, {sign:?}"
                );
            }
        }
    }

    #[test]
    fn inverse_undoes_multiplication() {
        for odd in (1..SECTOR_SIZE).step_by(2) {
            assert_eq!(odd * inverse(odd, SECTOR_SIZE) % SECTOR_SIZE, 1);
        }
    }

    /// Samples of the pages of a file with `key`, as a search finds them.
    fn search(key: StoreKey, pages: impl Iterator<Item = u32>) -> KeySearch {
        let samples = pages.filter_map(|page| {
            let [walk, ..] = key.page_key(page).walks();
            walk.map(|walk| Sample { page, walk })
        });
        KeySearch {
            samples: samples.collect(),
        }
    }

    // One page in fifty can be a stale copy that happens to have a walk.
    #[test]
    fn a_key_may_miss_a_sample_in_fifty() {
        let key = StoreKey::new(0x0001_2345, 0x0098_7654, KeySign::Positive);
        let other = StoreKey::new(0x0054_3210, 0x0012_3456, KeySign::Negative);
        let mut search = search(key, 1..100);
        let alike = |found: &StoreKey| {
            let mut pages = 1..100;
            pages.all(|page| found.page_key(page).walks()[0] == key.page_key(page).walks()[0])
        };
        assert!(search.keys().unwrap().iter().any(alike));
        assert!(search.allows(key) && !search.allows(other));
        for (stale, page) in [(1, 60), (2, 70)] {
            let [walk, ..] = other.page_key(page).walks();
            search.samples[page as usize - 1].walk = walk.unwrap();
            // Ninety-nine samples allow one miss, and not two.
            assert_eq!(search.keys().unwrap().iter().any(alike), stale == 1);
            assert_eq!(search.allows(key), stale == 1);
        }
    }

    #[test]
    fn no_sample_gives_no_key() {
        let mut search = KeySearch::default();
        assert_eq!(search.keys(), Some(Vec::new()));
        // Zeros decode to page number zero only with an even increment, which no walk has.
        search.add(0, &[0; PAGE_SIZE]);
        assert!(search.samples.is_empty());
    }
}
