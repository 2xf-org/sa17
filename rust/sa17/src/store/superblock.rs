//! The first page of a file, which is stored in the clear.

use crate::error::FormatProblem;
use crate::format_version::FormatVersion;
use crate::page_size::PAGE_SIZE;

/// The four-byte mark at offset `0x14` of every database file.
const MAGIC: u32 = 0xda7a_ba5e;

/// Flag bits at offset `0x06` present in every file of this format.
const REQUIRED_FLAGS: u8 = 0x09;

/// What the first page says about the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Superblock {
    version: FormatVersion,
    lsn: u32,
    database_page_count: u32,
}

impl Superblock {
    /// Reads the fields from page 0 and refuses a file of another format.
    pub(crate) fn parse(page: &[u8; PAGE_SIZE]) -> Result<Self, FormatProblem> {
        let magic = u32_at(page, 0x14);
        if magic != MAGIC {
            return Err(FormatProblem::Magic { found: magic });
        }
        let version =
            FormatVersion::new(u32_at(page, 0x10), u16_at(page, 0x18), u16_at(page, 0x1a));
        if version != FormatVersion::SQL_ANYWHERE_17 {
            return Err(FormatProblem::Version { found: version });
        }
        let flags = page[0x06];
        if flags & REQUIRED_FLAGS != REQUIRED_FLAGS {
            return Err(FormatProblem::Flags { found: flags });
        }
        Ok(Self {
            version,
            lsn: u32_at(page, 0x08),
            database_page_count: u32_at(page, 0x1c),
        })
    }

    /// The version of the file's format.
    #[must_use]
    pub fn version(&self) -> FormatVersion {
        self.version
    }

    /// The file's log sequence number, which grows with every write.
    #[must_use]
    pub fn lsn(&self) -> u32 {
        self.lsn
    }

    /// The number of pages that belong to the database. The file's pages past
    /// them are copies that a checkpoint left.
    #[must_use]
    pub fn database_page_count(&self) -> u32 {
        self.database_page_count
    }
}

fn u16_at(page: &[u8; PAGE_SIZE], offset: usize) -> u16 {
    u16::from_le_bytes([page[offset], page[offset + 1]])
}

fn u32_at(page: &[u8; PAGE_SIZE], offset: usize) -> u32 {
    u32::from_le_bytes([
        page[offset],
        page[offset + 1],
        page[offset + 2],
        page[offset + 3],
    ])
}
