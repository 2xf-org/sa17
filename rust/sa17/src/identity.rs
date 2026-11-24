//! What tells one file, and one state of it, from another.

use std::fmt;

const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

/// What tells one database file, and one state of it, from another: the
/// number of its pages, its log sequence number, and a digest of its first
/// page and of the checksum that each of its pages stores.
///
/// A copy of a file has the file's identity, until either is written to. The
/// text of an identity names the same file in another run of a program. The
/// digest tells states apart that came about, and is none to rely on against
/// someone who makes a file to fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Identity {
    page_count: u32,
    lsn: u32,
    digest: u128,
}

impl Identity {
    /// The identity of a file of `page_count` pages that starts with
    /// `first_page`, from the checksums that its pages store.
    pub(crate) fn new(
        page_count: u32,
        lsn: u32,
        first_page: &[u8],
        checksums: impl Iterator<Item = [u8; 4]>,
    ) -> Self {
        let mut digest = FNV_OFFSET;
        for byte in first_page.iter().copied().chain(checksums.flatten()) {
            digest = (digest ^ u128::from(byte)).wrapping_mul(FNV_PRIME);
        }
        Self {
            page_count,
            lsn,
            digest,
        }
    }

    /// Number of pages covered by this identity.
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    /// Log sequence number recorded in the first page.
    #[must_use]
    pub fn lsn(&self) -> u32 {
        self.lsn
    }

    /// 128-bit FNV-1a digest of page 0 and every page's stored checksum.
    #[must_use]
    pub fn digest(&self) -> u128 {
        self.digest
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            page_count,
            lsn,
            digest,
        } = self;
        write!(formatter, "{page_count}-{lsn}-{digest:032x}")
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use super::*;

    // The second digest is the published one of FNV-1a for "foobar".
    #[test]
    fn the_text_has_every_part() {
        let empty = Identity::new(27, 5000, &[], iter::empty());
        assert_eq!(
            empty.to_string(),
            "27-5000-6c62272e07bb014262b821756295c58d"
        );
        let foobar = Identity::new(1, 2, b"fo", [*b"obar"].into_iter());
        assert_eq!(foobar.to_string(), "1-2-343e1662793c64bf6f0d3597ba446f18");
    }
}
