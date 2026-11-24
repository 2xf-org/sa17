//! The fields of a record as it stores them.

use crate::error::RowProblem;
use crate::record::Descriptor;
use crate::row_address::RowAddress;
use crate::schema::{Column, DataType};

/// A string field whose marker byte is below this holds that many bytes.
const LONG_MARKER: u8 = 0x80;

/// The marker byte of a string field with a 16-bit length.
const WIDE_MARKER: u8 = 0xff;

/// The tag of a long value whose pieces are linked.
const LINKED: u8 = 0x80;

/// The tags of a compressed long value, in one piece and in linked ones.
const COMPRESSED: [u8; 2] = [0x40, 0xc0];

/// How the values of a column are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    /// So many bytes.
    Fixed(usize),
    /// A digit count, a control byte and that many base-100 digits.
    Number,
    /// A marker byte, then bytes or the descriptor of a long value.
    Bytes {
        /// Whether the pieces of a long value can be linked.
        linked: bool,
        /// Whether a long value can be compressed.
        compressed: bool,
        /// Whether a long value's length counts its prefix.
        counts_prefix: bool,
    },
    /// A form this reader does not know. Only `NULL` takes no bytes.
    Unknown,
}

impl Form {
    /// The form of the values of `column`.
    pub(crate) fn of(column: &Column) -> Result<Self, RowProblem> {
        Ok(match column.data_type() {
            DataType::SmallInt | DataType::UnsignedSmallInt => Self::Fixed(2),
            DataType::Integer | DataType::UnsignedInt | DataType::Float | DataType::Date => {
                Self::Fixed(4)
            }
            DataType::BigInt
            | DataType::UnsignedBigInt
            | DataType::Double
            | DataType::Time
            | DataType::Timestamp => Self::Fixed(8),
            DataType::TinyInt if column.width != 1 => {
                return Err(RowProblem::Width {
                    column_id: column.id,
                    width: column.width,
                });
            }
            DataType::TinyInt | DataType::Bit => Self::Fixed(1),
            DataType::Numeric | DataType::Decimal => Self::Number,
            DataType::Char | DataType::Binary => Self::Bytes {
                linked: false,
                compressed: false,
                counts_prefix: true,
            },
            DataType::VarChar
            | DataType::LongVarChar
            | DataType::LongBinary
            | DataType::VarBinary => Self::Bytes {
                linked: true,
                compressed: true,
                counts_prefix: true,
            },
            DataType::LongVarBit => Self::Bytes {
                linked: true,
                compressed: false,
                counts_prefix: false,
            },
            DataType::Unknown(_) => Self::Unknown,
        })
    }
}

/// One field that holds a value, as its record stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredField<'record> {
    /// The bytes of a value of fixed size, or of a string.
    Bytes(&'record [u8]),
    /// A number: its control byte and its base-100 digits.
    Number { control: u8, digits: &'record [u8] },
    /// A value that is kept outside the row.
    Long(Descriptor<'record>),
}

/// Takes the next field off `rest`. `column` is the column it is of, for an
/// error.
pub(crate) fn take<'record>(
    rest: &mut &'record [u8],
    form: Form,
    column: &Column,
) -> Result<StoredField<'record>, RowProblem> {
    match form {
        Form::Fixed(size) => bytes(rest, size).map(StoredField::Bytes),
        Form::Number => {
            let [count, control] = array(rest)?;
            let digits = bytes(rest, count.into())?;
            Ok(StoredField::Number { control, digits })
        }
        Form::Bytes {
            linked,
            compressed,
            counts_prefix,
        } => {
            let [marker] = array(rest)?;
            if marker == WIDE_MARKER {
                let length = u16::from_be_bytes(array(rest)?);
                return bytes(rest, length.into()).map(StoredField::Bytes);
            }
            if marker < LONG_MARKER {
                return bytes(rest, marker.into()).map(StoredField::Bytes);
            }
            let unknown = RowProblem::Descriptor {
                column_id: column.id,
            };
            let prefix = bytes(rest, (marker & !LONG_MARKER).into())?;
            let [tag] = array(rest)?;
            let compressed = compressed && COMPRESSED.contains(&tag);
            if tag != 0 && !compressed && !(linked && tag == LINKED) {
                return Err(unknown);
            }
            let stored_length = u32::from_le_bytes(array(rest)?);
            let page = u32::from_le_bytes(array(rest)?);
            let _: [u8; 2] = array(rest)?;
            let slot = u16::from_le_bytes(array(rest)?);
            let decompressed_length = if compressed {
                Some(u32::from_le_bytes(array(rest)?))
            } else {
                None
            };
            // The length that the prefix is the start of.
            let length = decompressed_length.unwrap_or(stored_length);
            let shorter = (length as usize) < prefix.len();
            let empty = compressed && (length == 0 || stored_length == 0);
            if empty || (counts_prefix && shorter) {
                return Err(unknown);
            }
            Ok(StoredField::Long(Descriptor {
                prefix,
                stored_length,
                first: RowAddress::new(page, slot),
                decompressed_length,
            }))
        }
        Form::Unknown => Err(RowProblem::UnsupportedDomain {
            column_id: column.id,
            domain: column.domain,
        }),
    }
}

fn bytes<'record>(rest: &mut &'record [u8], size: usize) -> Result<&'record [u8], RowProblem> {
    let (taken, left) = rest.split_at_checked(size).ok_or(RowProblem::Truncated)?;
    *rest = left;
    Ok(taken)
}

fn array<const N: usize>(rest: &mut &[u8]) -> Result<[u8; N], RowProblem> {
    let (taken, left) = rest.split_first_chunk().ok_or(RowProblem::Truncated)?;
    *rest = left;
    Ok(*taken)
}
