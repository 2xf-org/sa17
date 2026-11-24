//! The sizes that every file of the format shares.

/// The bytes in a page.
pub(crate) const PAGE_SIZE: usize = 4096;

/// The bytes in a sector, which the cipher scrambles on its own.
pub(crate) const SECTOR_SIZE: usize = 512;
