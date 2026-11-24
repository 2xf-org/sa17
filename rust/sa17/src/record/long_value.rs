//! Values that are too long for their row.
//!
//! A row keeps a descriptor in the value's place: where the first piece is,
//! how many bytes the value has, and its first few bytes. The pieces are
//! records on the table's extension pages. A piece with flags 4 names the
//! next piece before its bytes; a piece with flags 0 is the last.

use crate::error::LongValueProblem;
use crate::record::compression;
use crate::record::{Descriptor, ExtensionPages, Met};
use crate::row_address::RowAddress;

const LAST_PIECE: u8 = 0;
const LINKED_PIECE: u8 = 4;

/// The size of a piece's length and flags.
const HEADER: usize = 3;

/// The size of a linked piece's header: length, flags, next page, next slot.
const LINK_HEADER: usize = 9;

/// Reads the long values of one table.
#[derive(Clone, Copy)]
pub(crate) struct LongValueReader<'db> {
    pages: ExtensionPages<'db>,
    /// The most bytes of a value that was stored compressed.
    longest: u32,
}

/// Why a long value was not read, and the piece that it was not read for.
pub(crate) struct Failure {
    pub(crate) piece: RowAddress,
    pub(crate) problem: LongValueProblem,
}

/// One piece as its record holds it.
struct Piece<'page> {
    bytes: &'page [u8],
    next: Option<RowAddress>,
}

impl<'db> LongValueReader<'db> {
    /// A reader of values on `pages`, the extension pages of a table, which
    /// takes values of up to `longest` bytes out of compression.
    pub(crate) fn new(pages: ExtensionPages<'db>, longest: u32) -> Self {
        Self { pages, longest }
    }

    /// The complete bytes of a value.
    pub(crate) fn read(
        &self,
        descriptor: &Descriptor<'_>,
        met: &mut Met,
    ) -> Result<Vec<u8>, Failure> {
        let fails = |problem| Failure {
            piece: descriptor.first,
            problem,
        };
        let stored = self.pieces(descriptor, met)?;
        let longest = self.longest as usize;
        let bytes = match descriptor.decompressed_length {
            None => stored,
            Some(length) => compression::decode(&stored, length, longest).map_err(fails)?,
        };
        // The row keeps a prefix of the value itself, not of its compressed form.
        if bytes.starts_with(descriptor.prefix) {
            Ok(bytes)
        } else {
            Err(fails(LongValueProblem::Prefix))
        }
    }

    /// The bytes that the pieces of a value hold, up to their stored length.
    /// The last piece can be longer than what is left of the value.
    fn pieces(&self, descriptor: &Descriptor<'_>, met: &mut Met) -> Result<Vec<u8>, Failure> {
        let length = descriptor.stored_length;
        let expected = length as usize;
        let mut bytes = Vec::new();
        let mut next = Some(descriptor.first);
        while bytes.len() < expected {
            let Some(at) = next else {
                return Err(Failure {
                    piece: descriptor.first,
                    problem: LongValueProblem::Truncated {
                        found: bytes.len(),
                        expected,
                    },
                });
            };
            let fails = |problem| Failure { piece: at, problem };
            if !met.first(at) {
                return Err(fails(LongValueProblem::RepeatedPiece));
            }
            let page = self.pages.page(at.page());
            let page = page.map_err(|problem| fails(LongValueProblem::Page(problem)))?;
            let record = page.record(at.slot());
            let record = record.ok_or_else(|| fails(LongValueProblem::MissingPiece))?;
            let piece = Piece::read(record).map_err(fails)?;
            let wanted = piece.bytes.len().min(expected - bytes.len());
            let room = bytes.try_reserve(wanted);
            room.map_err(|_| fails(LongValueProblem::TooLong { length }))?;
            bytes.extend_from_slice(&piece.bytes[..wanted]);
            next = piece.next;
        }
        Ok(bytes)
    }
}

impl<'page> Piece<'page> {
    /// The piece that `record` is. A piece that another one follows has bytes,
    /// or a chain could be longer than the value is.
    fn read(record: &'page [u8]) -> Result<Self, LongValueProblem> {
        match record[2] {
            LAST_PIECE => Ok(Self {
                bytes: &record[HEADER..],
                next: None,
            }),
            LINKED_PIECE if record.len() > LINK_HEADER => Ok(Self {
                bytes: &record[LINK_HEADER..],
                next: Some(RowAddress::new(
                    u32::from_le_bytes([record[3], record[4], record[5], record[6]]),
                    u16::from_le_bytes([record[7], record[8]]),
                )),
            }),
            LINKED_PIECE => Err(LongValueProblem::EmptyPiece),
            flags => Err(LongValueProblem::Flags { flags }),
        }
    }
}
