//! The page cipher, in the direction that scrambles.

const PAGE: usize = 4096;
const SECTOR: usize = 512;
const CLEAR_TAIL: usize = 16;
const KEY_MASK: u32 = (1 << 25) - 1;

/// A sign: of every page key of a file, or of one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    /// Zero or above.
    Positive,
    /// Below zero.
    Negative,
}

/// The constants that give each page of a file its key.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Key {
    pub(crate) mask: u32,
    pub(crate) addend: u32,
    pub(crate) sign: Sign,
}

impl Key {
    fn of_page(self, page: u32) -> i32 {
        let word = (page ^ self.mask).wrapping_add(self.addend) & KEY_MASK;
        match self.sign {
            Sign::Positive => (!word & KEY_MASK) as i32,
            Sign::Negative => -(word as i32),
        }
    }

    /// Turns a readable page into the bytes a file stores for it.
    ///
    /// Twelve header bytes move to the end of the page, where they stay in the
    /// clear, and the row bytes they replace take their place. Each sector is
    /// then scrambled, and the checksum of the result closes the page.
    pub(crate) fn scramble(self, number: u32, plain: &[u8; PAGE]) -> [u8; PAGE] {
        let header = PAGE - CLEAR_TAIL;
        let mut swapped = *plain;
        swapped[18..20].copy_from_slice(&plain[header..header + 2]);
        swapped[6..16].copy_from_slice(&plain[header + 2..header + 12]);
        swapped[header..header + 2].copy_from_slice(&plain[18..20]);
        swapped[header + 2..header + 12].copy_from_slice(&plain[6..16]);

        let mut stored = swapped;
        let key = self.of_page(number);
        for sector in 0..PAGE / SECTOR {
            let clear_tail = if sector == PAGE / SECTOR - 1 {
                CLEAR_TAIL
            } else {
                0
            };
            let bytes = sector * SECTOR..(sector + 1) * SECTOR;
            scramble_sector(
                &swapped[bytes.clone()],
                &mut stored[bytes],
                key.wrapping_sub(sector as i32),
                clear_tail,
            );
        }
        stamp(&mut stored);
        stored
    }
}

/// Writes the checksum of a page's first 4092 bytes into its last four.
pub(crate) fn stamp(page: &mut [u8; PAGE]) {
    let checksum = crc32fast::hash(&page[..PAGE - 4]);
    page[PAGE - 4..].copy_from_slice(&checksum.to_le_bytes());
}

/// Moves each byte one stride forward and subtracts the running offset.
fn scramble_sector(plain: &[u8], stored: &mut [u8], key: i32, clear_tail: usize) {
    if key == 0 {
        return;
    }
    let magnitude = key.unsigned_abs();
    let low = magnitude as u8;
    let increment = ((magnitude >> 8) as u8) | 1;
    let forward = (((magnitude >> 16) as usize + usize::from(low)) | 1) % SECTOR;
    let (stride, mut offset) = if key < 0 {
        let steps = (clear_tail + 1) as u8;
        (
            (SECTOR - forward) % SECTOR,
            increment.wrapping_mul(steps).wrapping_sub(low),
        )
    } else {
        (forward, low)
    };
    let active = SECTOR - clear_tail;
    let mut position = 0;
    for _ in 0..active {
        let mut next = (position + stride) % SECTOR;
        while next >= active {
            next = (next + stride) % SECTOR;
        }
        stored[next] = plain[position].wrapping_sub(offset);
        offset = offset.wrapping_add(increment);
        position = next;
    }
}
