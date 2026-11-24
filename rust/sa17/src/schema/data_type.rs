//! The SQL types of columns.

/// The SQL type of a column, from the domain identifier in the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DataType {
    /// `SMALLINT`, 16 bits.
    SmallInt,
    /// `INTEGER`, 32 bits.
    Integer,
    /// `BIGINT`, 64 bits.
    BigInt,
    /// `TINYINT`, one unsigned byte.
    TinyInt,
    /// `UNSIGNED SMALLINT`.
    UnsignedSmallInt,
    /// `UNSIGNED INT`.
    UnsignedInt,
    /// `UNSIGNED BIGINT`.
    UnsignedBigInt,
    /// `BIT`.
    Bit,
    /// `NUMERIC` with a precision and a scale.
    Numeric,
    /// `DECIMAL` with a precision and a scale.
    Decimal,
    /// `FLOAT`, single precision.
    Float,
    /// `DOUBLE`.
    Double,
    /// `DATE`.
    Date,
    /// `TIME`.
    Time,
    /// `TIMESTAMP`.
    Timestamp,
    /// `CHAR` of a declared width.
    Char,
    /// `VARCHAR` of a declared width.
    VarChar,
    /// `LONG VARCHAR`.
    LongVarChar,
    /// `BINARY` of a declared width.
    Binary,
    /// `VARBINARY` of a declared width.
    VarBinary,
    /// `LONG BINARY`.
    LongBinary,
    /// `LONG VARBIT`. The catalog keeps its page lists in this type.
    LongVarBit,
    /// A type this reader does not know, by its domain identifier. It reads
    /// only `NULL` in such a column.
    Unknown(u16),
}

impl DataType {
    /// The type of a catalog domain identifier.
    #[must_use]
    pub fn from_domain(domain: u16) -> Self {
        match domain {
            1 => Self::SmallInt,
            2 => Self::Integer,
            3 => Self::Numeric,
            4 => Self::Float,
            5 => Self::Double,
            6 => Self::Date,
            7 | 8 => Self::Char,
            9 => Self::VarChar,
            10 => Self::LongVarChar,
            11 => Self::Binary,
            12 => Self::LongBinary,
            13 => Self::Timestamp,
            14 => Self::Time,
            19 => Self::TinyInt,
            20 => Self::BigInt,
            21 => Self::UnsignedInt,
            22 => Self::UnsignedSmallInt,
            23 => Self::UnsignedBigInt,
            24 => Self::Bit,
            27 => Self::Decimal,
            28 => Self::VarBinary,
            31 => Self::LongVarBit,
            _ => Self::Unknown(domain),
        }
    }

    /// The type's name in SQL, without a width or a precision.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::SmallInt => "smallint",
            Self::Integer => "integer",
            Self::BigInt => "bigint",
            Self::TinyInt => "tinyint",
            Self::UnsignedSmallInt => "unsigned smallint",
            Self::UnsignedInt => "unsigned int",
            Self::UnsignedBigInt => "unsigned bigint",
            Self::Bit => "bit",
            Self::Numeric => "numeric",
            Self::Decimal => "decimal",
            Self::Float => "float",
            Self::Double => "double",
            Self::Date => "date",
            Self::Time => "time",
            Self::Timestamp => "timestamp",
            Self::Char => "char",
            Self::VarChar => "varchar",
            Self::LongVarChar => "long varchar",
            Self::Binary => "binary",
            Self::VarBinary => "varbinary",
            Self::LongBinary => "long binary",
            Self::LongVarBit => "long varbit",
            Self::Unknown(_) => "unknown",
        }
    }

    /// Whether values of the type are text in the file's character encoding.
    #[must_use]
    pub fn is_text(self) -> bool {
        matches!(self, Self::Char | Self::VarChar | Self::LongVarChar)
    }
}
