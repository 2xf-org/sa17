//! The key of one page and the sector cipher it drives.
//!
//! A page is eight 512-byte sectors, each scrambled on its own. The engine
//! walks a sector in steps of an odd stride and stores each byte one step
//! back, minus an offset that grows with every step. Sector `n` uses the
//! page's key minus `n`. The last sixteen bytes of a page stay in the clear:
//! twelve header bytes and the checksum.

use crate::page_size::{PAGE_SIZE, SECTOR_SIZE};

/// Bytes at the end of a page that the cipher leaves alone.
const CLEAR_TAIL: usize = 16;

/// Sectors in a page.
const SECTORS: usize = PAGE_SIZE / SECTOR_SIZE;

/// The walk of each sector of a page, or `None` for one that is not scrambled.
pub(crate) type PageWalks = [Option<SectorWalk>; SECTORS];

/// Where the engine keeps twelve header bytes in the clear.
const CLEAR_HEADER: usize = PAGE_SIZE - CLEAR_TAIL;

/// The signed key that scrambles one page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageKey(i32);

impl PageKey {
    pub(crate) const fn new(value: i32) -> Self {
        Self(value)
    }

    /// The key as a signed integer. Its sign picks the direction of the walk.
    #[must_use]
    pub const fn value(self) -> i32 {
        self.0
    }

    /// Restores the page that `stored` holds, or `None` when that is not the
    /// bytes of one page.
    ///
    /// The header fields and the last twelve row bytes are back in their
    /// places. The final four bytes are still the checksum of the stored page.
    #[must_use]
    pub fn decode(self, stored: &[u8]) -> Option<Vec<u8>> {
        let stored = stored.try_into().ok()?;
        Some(self.restore(stored).to_vec())
    }

    /// Restores the page that `stored` holds.
    pub(crate) fn restore(self, stored: &[u8; PAGE_SIZE]) -> Box<[u8; PAGE_SIZE]> {
        let mut plain = Box::new(*stored);
        for (sector, walk) in self.walks().into_iter().enumerate() {
            let Some(walk) = walk else {
                continue;
            };
            let bytes = sector * SECTOR_SIZE..(sector + 1) * SECTOR_SIZE;
            walk.decode(&stored[bytes.clone()], &mut plain[bytes]);
        }
        restore_header(stored, &mut plain);
        plain
    }

    /// The walk of each sector of the page. Sector `n` uses the key minus
    /// `n`, and the last sector leaves the clear tail alone.
    pub(crate) fn walks(self) -> PageWalks {
        let mut walks = [None; SECTORS];
        for (sector, walk) in (0..).zip(&mut walks) {
            let clear_tail = if sector as usize == SECTORS - 1 {
                CLEAR_TAIL
            } else {
                0
            };
            *walk = SectorWalk::new(self.0.wrapping_sub(sector), clear_tail);
        }
        walks
    }
}

/// How the cipher walks one sector: the step between positions and the offset
/// it adds along the way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SectorWalk {
    pub(crate) stride: usize,
    pub(crate) increment: u8,
    pub(crate) seed: u8,
    pub(crate) clear_tail: usize,
}

impl SectorWalk {
    /// The walk of a sector key, or `None` for key zero, which scrambles nothing.
    pub(crate) fn new(key: i32, clear_tail: usize) -> Option<Self> {
        if key == 0 {
            return None;
        }
        let magnitude = key.unsigned_abs();
        let low = magnitude as u8;
        let increment = ((magnitude >> 8) as u8) | 1;
        let forward = (((magnitude >> 16) as usize + usize::from(low)) | 1) % SECTOR_SIZE;
        let (stride, seed) = if key < 0 {
            let steps = (clear_tail + 1) as u8;
            (
                (SECTOR_SIZE - forward) % SECTOR_SIZE,
                increment.wrapping_mul(steps).wrapping_sub(low),
            )
        } else {
            (forward, low)
        };
        Some(Self {
            stride,
            increment,
            seed,
            clear_tail,
        })
    }

    fn decode(self, stored: &[u8], plain: &mut [u8]) {
        let mut offset = self.seed;
        let mut position = 0;
        for _ in 0..SECTOR_SIZE - self.clear_tail {
            let next = self.next(position);
            plain[position] = stored[next].wrapping_add(offset);
            offset = offset.wrapping_add(self.increment);
            position = next;
        }
    }

    // The stride is odd, so the walk visits every position before it repeats.
    fn next(self, position: usize) -> usize {
        let active = SECTOR_SIZE - self.clear_tail;
        let mut next = (position + self.stride) % SECTOR_SIZE;
        while next >= active {
            next = (next + self.stride) % SECTOR_SIZE;
        }
        next
    }
}

/// Swaps the clear header bytes at the end of the page with the row bytes
/// that the engine parked in the header to make room for them.
fn restore_header(stored: &[u8; PAGE_SIZE], plain: &mut [u8; PAGE_SIZE]) {
    let mut parked = [0; 12];
    parked[..2].copy_from_slice(&plain[18..20]);
    parked[2..].copy_from_slice(&plain[6..16]);
    plain[18..20].copy_from_slice(&stored[CLEAR_HEADER..CLEAR_HEADER + 2]);
    plain[6..16].copy_from_slice(&stored[CLEAR_HEADER + 2..CLEAR_HEADER + 12]);
    plain[CLEAR_HEADER..CLEAR_HEADER + 12].copy_from_slice(&parked);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(length: usize) -> Vec<u8> {
        let mut state = 0x1234_5678_u32;
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            bytes.push((state >> 24) as u8);
        }
        bytes
    }

    // The engine unscrambles with the opposite key, so two walks cancel.
    #[test]
    fn opposite_keys_cancel() {
        let stored = noise(SECTOR_SIZE);
        for key in [1, 7, 255, 256, 65_537, 0x0123_4567, -1, -255, -0x0123_4567] {
            for clear_tail in [0, CLEAR_TAIL] {
                let mut plain = stored.clone();
                SectorWalk::new(key, clear_tail)
                    .unwrap()
                    .decode(&stored, &mut plain);
                let mut again = plain.clone();
                SectorWalk::new(-key, clear_tail)
                    .unwrap()
                    .decode(&plain, &mut again);
                assert_eq!(again, stored, "key {key}, clear tail {clear_tail}");
            }
        }
    }

    #[test]
    fn the_clear_tail_and_the_checksum_stay() {
        let stored: [u8; PAGE_SIZE] = noise(PAGE_SIZE).try_into().unwrap();
        let plain = PageKey::new(0x00ab_cdef).restore(&stored);
        assert_eq!(plain[PAGE_SIZE - 4..], stored[PAGE_SIZE - 4..]);
        assert_eq!(plain[18..20], stored[CLEAR_HEADER..CLEAR_HEADER + 2]);
        assert_eq!(plain[6..16], stored[CLEAR_HEADER + 2..CLEAR_HEADER + 12]);
    }

    #[test]
    fn only_one_whole_page_is_decoded() {
        let key = PageKey::new(0x00ab_cdef);
        let stored: [u8; PAGE_SIZE] = noise(PAGE_SIZE).try_into().unwrap();
        assert_eq!(
            key.decode(&stored).as_deref(),
            Some(&key.restore(&stored)[..])
        );
        assert_eq!(key.decode(&stored[1..]), None);
        assert_eq!(key.decode(&[]), None);
    }

    // Sector zero uses the page key itself, and the later ones count down from it.
    #[test]
    fn key_zero_leaves_only_the_first_sector() {
        let stored: [u8; PAGE_SIZE] = noise(PAGE_SIZE).try_into().unwrap();
        let plain = PageKey::new(0).restore(&stored);
        assert_eq!(plain[..6], stored[..6]);
        assert_eq!(plain[20..SECTOR_SIZE], stored[20..SECTOR_SIZE]);
        assert_ne!(
            plain[SECTOR_SIZE..2 * SECTOR_SIZE],
            stored[SECTOR_SIZE..2 * SECTOR_SIZE]
        );
        assert_eq!(plain[CLEAR_HEADER..CLEAR_HEADER + 2], stored[18..20]);
    }
}
