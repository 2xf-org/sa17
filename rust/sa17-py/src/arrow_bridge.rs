//! Conversion from `sa17` tables and row streams to Apache Arrow values.

use std::sync::Arc;

use bimetal::arrow::array::builder::{
    BinaryBuilder, BooleanBuilder, Date32Builder, Decimal128Builder, Float32Builder,
    Float64Builder, Int16Builder, Int32Builder, Int64Builder, LargeBinaryBuilder,
    LargeStringBuilder, NullBuilder, StringBuilder, Time64MicrosecondBuilder,
    TimestampMicrosecondBuilder, UInt8Builder, UInt16Builder, UInt32Builder, UInt64Builder,
};
use bimetal::arrow::array::{ArrayRef, RecordBatch, RecordBatchReader};
use bimetal::arrow::schema::{ArrowError, DataType as ArrowDataType, Field, Schema, TimeUnit};
use bimetal::arrow::{RecordBatchStream, SchemaRef, Table as ArrowTable};
use chrono::{NaiveDate, Timelike};
use sa17::{Column, Columns, DataType, OwnedRows, TableDefinition, Value};

use crate::Sa17Error;

/// Maximum precision supported by Arrow `Decimal128`.
const MAX_DECIMAL128_PRECISION: u32 = 38;

/// Unix epoch date (`1970-01-01`), used to compute `Date32` day offsets.
fn unix_epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a valid Gregorian date")
}

/// Column specification needed to map one SA17 column to Arrow.
#[derive(Clone)]
struct ColumnSpec {
    name: String,
    arrow_type: ArrowDataType,
    nullable: bool,
}

impl ColumnSpec {
    fn from_column(column: &Column) -> Self {
        let arrow_type = arrow_type_for(column);
        // Columns added in later schema versions may hold NULL in older rows
        // even when marked NOT NULL, and unknown domains only read NULL.
        let nullable = column.is_nullable()
            || matches!(column.data_type(), DataType::Unknown(_))
            || column.absent_value().is_some_and(Value::is_null);
        Self {
            name: column.name().to_owned(),
            arrow_type,
            nullable,
        }
    }

    fn field(&self) -> Field {
        Field::new(&self.name, self.arrow_type.clone(), self.nullable)
    }
}

/// Returns the Arrow `DataType` that faithfully represents `column`.
fn arrow_type_for(column: &Column) -> ArrowDataType {
    match column.data_type() {
        DataType::Bit => ArrowDataType::Boolean,
        DataType::TinyInt => ArrowDataType::UInt8,
        DataType::SmallInt => ArrowDataType::Int16,
        DataType::UnsignedSmallInt => ArrowDataType::UInt16,
        DataType::Integer => ArrowDataType::Int32,
        DataType::UnsignedInt => ArrowDataType::UInt32,
        DataType::BigInt => ArrowDataType::Int64,
        DataType::UnsignedBigInt => ArrowDataType::UInt64,
        DataType::Float => ArrowDataType::Float32,
        DataType::Double => ArrowDataType::Float64,
        DataType::Numeric | DataType::Decimal => {
            let precision = column.width();
            let scale = column.scale();
            if (1..=MAX_DECIMAL128_PRECISION).contains(&precision) && u32::from(scale) <= precision
            {
                ArrowDataType::Decimal128(precision as u8, scale as i8)
            } else {
                ArrowDataType::Utf8
            }
        }
        DataType::Char | DataType::VarChar => ArrowDataType::Utf8,
        DataType::LongVarChar => ArrowDataType::LargeUtf8,
        DataType::Binary | DataType::VarBinary => ArrowDataType::Binary,
        DataType::LongBinary | DataType::LongVarBit => ArrowDataType::LargeBinary,
        DataType::Date => ArrowDataType::Date32,
        DataType::Time => ArrowDataType::Time64(TimeUnit::Microsecond),
        DataType::Timestamp => ArrowDataType::Timestamp(TimeUnit::Microsecond, None),
        DataType::Unknown(_) => ArrowDataType::Null,
        _ => ArrowDataType::Utf8,
    }
}

/// Resolves the Arrow [`Schema`] and column specs for `columns` of `definition`.
fn build_specs(
    definition: &TableDefinition,
    columns: &Columns,
) -> Result<(SchemaRef, Vec<ColumnSpec>), Sa17Error> {
    let mut specs = Vec::with_capacity(columns.len());
    for name in columns.names() {
        let Some(column) = definition.column(name) else {
            return Err(Sa17Error::UnknownColumn {
                table: definition.name().to_owned(),
                column: name.to_owned(),
            });
        };
        specs.push(ColumnSpec::from_column(column));
    }
    let fields: Vec<Field> = specs.iter().map(ColumnSpec::field).collect();
    let schema = Arc::new(Schema::new(fields));
    Ok((schema, specs))
}

/// Builds the Arrow [`Schema`] for the selected columns of `definition`.
pub(crate) fn schema_for(
    definition: &TableDefinition,
    columns: &Columns,
) -> Result<Schema, Sa17Error> {
    let (schema, _) = build_specs(definition, columns)?;
    Ok((*schema).clone())
}

/// Streams a table as lazy Arrow record batches of at most `batch_size` rows.
pub(crate) fn batch_stream(
    definition: &TableDefinition,
    columns: &Columns,
    rows: OwnedRows,
    batch_size: u32,
) -> Result<RecordBatchStream, Sa17Error> {
    if batch_size == 0 {
        return Err(Sa17Error::Arrow {
            message: "batch_size must be at least 1".to_owned(),
        });
    }
    let (schema, specs) = build_specs(definition, columns)?;
    let reader = TableBatchReader {
        schema,
        table_name: definition.name().to_owned(),
        specs,
        rows,
        batch_size: batch_size as usize,
        done: false,
    };
    Ok(RecordBatchStream::from_reader(reader))
}

/// Materializes a table into an Arrow [`ArrowTable`] in batches of `batch_size`.
pub(crate) fn read_table(
    definition: &TableDefinition,
    columns: &Columns,
    rows: OwnedRows,
    batch_size: u32,
) -> Result<ArrowTable, Sa17Error> {
    if batch_size == 0 {
        return Err(Sa17Error::Arrow {
            message: "batch_size must be at least 1".to_owned(),
        });
    }
    let (schema, specs) = build_specs(definition, columns)?;
    let reader = TableBatchReader {
        schema: Arc::clone(&schema),
        table_name: definition.name().to_owned(),
        specs,
        rows,
        batch_size: batch_size as usize,
        done: false,
    };
    let mut batches = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|error| Sa17Error::Arrow {
            message: error.to_string(),
        })?;
        batches.push(batch);
    }
    ArrowTable::try_new(schema, batches).map_err(|error| Sa17Error::Arrow {
        message: error.to_string(),
    })
}

/// Blocking `RecordBatchReader` that pulls rows lazily from an `OwnedRows`
/// cursor and builds one Arrow `RecordBatch` at a time.
struct TableBatchReader {
    schema: SchemaRef,
    table_name: String,
    specs: Vec<ColumnSpec>,
    rows: OwnedRows,
    batch_size: usize,
    done: bool,
}

impl TableBatchReader {
    fn next_batch(&mut self) -> Result<Option<RecordBatch>, ArrowError> {
        let builders: Result<Vec<ColumnBuilder>, ArrowError> = self
            .specs
            .iter()
            .map(|spec| ColumnBuilder::new(&spec.arrow_type, self.batch_size))
            .collect();
        let mut builders = builders?;

        let mut count = 0;
        while count < self.batch_size {
            let Some(row) = self.rows.next() else {
                self.done = true;
                break;
            };
            let row = row.map_err(|error| ArrowError::ExternalError(Box::new(error)))?;
            for ((builder, spec), value) in builders.iter_mut().zip(&self.specs).zip(row.values()) {
                builder.push(value, &self.table_name, &spec.name)?;
            }
            count += 1;
        }

        if count == 0 {
            return Ok(None);
        }

        let arrays: Result<Vec<ArrayRef>, ArrowError> =
            builders.iter_mut().map(ColumnBuilder::finish).collect();
        let batch = RecordBatch::try_new(Arc::clone(&self.schema), arrays?)?;
        Ok(Some(batch))
    }
}

impl Iterator for TableBatchReader {
    type Item = Result<RecordBatch, ArrowError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.next_batch() {
            Ok(Some(batch)) => Some(Ok(batch)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}

impl RecordBatchReader for TableBatchReader {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

/// Typed Arrow array builder for a single column in a batch.
enum ColumnBuilder {
    Boolean(BooleanBuilder),
    UInt8(UInt8Builder),
    Int16(Int16Builder),
    UInt16(UInt16Builder),
    Int32(Int32Builder),
    UInt32(UInt32Builder),
    Int64(Int64Builder),
    UInt64(UInt64Builder),
    Float32(Float32Builder),
    Float64(Float64Builder),
    Decimal128 {
        builder: Decimal128Builder,
        precision: u8,
        scale: i8,
    },
    Utf8(StringBuilder),
    LargeUtf8(LargeStringBuilder),
    Binary(BinaryBuilder),
    LargeBinary(LargeBinaryBuilder),
    Date32(Date32Builder),
    Time64(Time64MicrosecondBuilder),
    Timestamp(TimestampMicrosecondBuilder),
    Null(NullBuilder),
}

impl ColumnBuilder {
    fn new(data_type: &ArrowDataType, capacity: usize) -> Result<Self, ArrowError> {
        let builder = match *data_type {
            ArrowDataType::Boolean => Self::Boolean(BooleanBuilder::with_capacity(capacity)),
            ArrowDataType::UInt8 => Self::UInt8(UInt8Builder::with_capacity(capacity)),
            ArrowDataType::Int16 => Self::Int16(Int16Builder::with_capacity(capacity)),
            ArrowDataType::UInt16 => Self::UInt16(UInt16Builder::with_capacity(capacity)),
            ArrowDataType::Int32 => Self::Int32(Int32Builder::with_capacity(capacity)),
            ArrowDataType::UInt32 => Self::UInt32(UInt32Builder::with_capacity(capacity)),
            ArrowDataType::Int64 => Self::Int64(Int64Builder::with_capacity(capacity)),
            ArrowDataType::UInt64 => Self::UInt64(UInt64Builder::with_capacity(capacity)),
            ArrowDataType::Float32 => Self::Float32(Float32Builder::with_capacity(capacity)),
            ArrowDataType::Float64 => Self::Float64(Float64Builder::with_capacity(capacity)),
            ArrowDataType::Decimal128(precision, scale) => Self::Decimal128 {
                builder: Decimal128Builder::with_capacity(capacity)
                    .with_data_type(ArrowDataType::Decimal128(precision, scale)),
                precision,
                scale,
            },
            ArrowDataType::Utf8 => {
                Self::Utf8(StringBuilder::with_capacity(capacity, capacity * 16))
            }
            ArrowDataType::LargeUtf8 => {
                Self::LargeUtf8(LargeStringBuilder::with_capacity(capacity, capacity * 32))
            }
            ArrowDataType::Binary => {
                Self::Binary(BinaryBuilder::with_capacity(capacity, capacity * 16))
            }
            ArrowDataType::LargeBinary => {
                Self::LargeBinary(LargeBinaryBuilder::with_capacity(capacity, capacity * 32))
            }
            ArrowDataType::Date32 => Self::Date32(Date32Builder::with_capacity(capacity)),
            ArrowDataType::Time64(TimeUnit::Microsecond) => {
                Self::Time64(Time64MicrosecondBuilder::with_capacity(capacity))
            }
            ArrowDataType::Timestamp(TimeUnit::Microsecond, None) => {
                Self::Timestamp(TimestampMicrosecondBuilder::with_capacity(capacity))
            }
            ArrowDataType::Null => Self::Null(NullBuilder::new()),
            ref other => {
                return Err(ArrowError::InvalidArgumentError(format!(
                    "unsupported Arrow column type: {other}"
                )));
            }
        };
        Ok(builder)
    }

    fn push(&mut self, value: &Value, table: &str, column: &str) -> Result<(), ArrowError> {
        if let Value::Undecoded(undecoded) = value {
            return Err(ArrowError::InvalidArgumentError(format!(
                "table {table}, column {column}: undecoded value for domain {} ({} bytes) cannot be represented in Arrow",
                undecoded.domain(),
                undecoded.bytes().len()
            )));
        }

        match self {
            Self::Boolean(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Boolean(flag) => builder.append_value(*flag),
                other => return Err(mismatch(table, column, "boolean", other)),
            },
            Self::UInt8(builder) => push_int(
                value,
                table,
                column,
                "uint8",
                |number| u8::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::Int16(builder) => push_int(
                value,
                table,
                column,
                "int16",
                |number| i16::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::UInt16(builder) => push_int(
                value,
                table,
                column,
                "uint16",
                |number| u16::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::Int32(builder) => push_int(
                value,
                table,
                column,
                "int32",
                |number| i32::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::UInt32(builder) => push_int(
                value,
                table,
                column,
                "uint32",
                |number| u32::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::Int64(builder) => push_int(
                value,
                table,
                column,
                "int64",
                |number| i64::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::UInt64(builder) => push_int(
                value,
                table,
                column,
                "uint64",
                |number| u64::try_from(number).ok(),
                |number| builder.append_option(number),
            )?,
            Self::Float32(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Float(number) => builder.append_value(*number),
                other => return Err(mismatch(table, column, "float32", other)),
            },
            Self::Float64(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Double(number) => builder.append_value(*number),
                Value::Float(number) => builder.append_value(f64::from(*number)),
                other => return Err(mismatch(table, column, "float64", other)),
            },
            Self::Decimal128 {
                builder,
                precision,
                scale,
            } => match value {
                Value::Null => builder.append_null(),
                Value::Decimal(decimal) => {
                    let target_scale = u8::try_from(*scale).map_err(|_| {
                        ArrowError::InvalidArgumentError(format!(
                            "table {table}, column {column}: negative decimal scale {scale}"
                        ))
                    })?;
                    let rescaled = decimal.rescale(u16::from(target_scale)).ok_or_else(|| {
                        ArrowError::InvalidArgumentError(format!(
                            "table {table}, column {column}: decimal {decimal} does not fit scale {target_scale}"
                        ))
                    })?;
                    let mantissa = rescaled.mantissa();
                    if !fits_decimal_precision(mantissa, *precision) {
                        return Err(ArrowError::InvalidArgumentError(format!(
                            "table {table}, column {column}: decimal {decimal} exceeds precision {precision}"
                        )));
                    }
                    builder.append_value(mantissa);
                }
                other => return Err(mismatch(table, column, "decimal128", other)),
            },
            Self::Utf8(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Text(text) => builder.append_value(text),
                Value::Decimal(decimal) => builder.append_value(decimal.to_string()),
                other => return Err(mismatch(table, column, "utf8", other)),
            },
            Self::LargeUtf8(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Text(text) => builder.append_value(text),
                other => return Err(mismatch(table, column, "large_utf8", other)),
            },
            Self::Binary(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Bytes(bytes) => builder.append_value(bytes),
                other => return Err(mismatch(table, column, "binary", other)),
            },
            Self::LargeBinary(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Bytes(bytes) => builder.append_value(bytes),
                other => return Err(mismatch(table, column, "large_binary", other)),
            },
            Self::Date32(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Date(date) => {
                    let days = (*date - unix_epoch()).num_days();
                    let days = i32::try_from(days).map_err(|_| {
                        ArrowError::InvalidArgumentError(format!(
                            "table {table}, column {column}: date {date} out of Date32 range"
                        ))
                    })?;
                    builder.append_value(days);
                }
                other => return Err(mismatch(table, column, "date32", other)),
            },
            Self::Time64(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Time(time) => {
                    let micros = i64::from(time.num_seconds_from_midnight()) * 1_000_000
                        + i64::from(time.nanosecond() / 1_000);
                    builder.append_value(micros);
                }
                other => return Err(mismatch(table, column, "time64[us]", other)),
            },
            Self::Timestamp(builder) => match value {
                Value::Null => builder.append_null(),
                Value::Timestamp(timestamp) => {
                    builder.append_value(timestamp.and_utc().timestamp_micros());
                }
                other => return Err(mismatch(table, column, "timestamp[us]", other)),
            },
            Self::Null(builder) => match value {
                Value::Null => builder.append_empty_value(),
                other => return Err(mismatch(table, column, "null", other)),
            },
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<ArrayRef, ArrowError> {
        let array: ArrayRef = match self {
            Self::Boolean(builder) => Arc::new(builder.finish()),
            Self::UInt8(builder) => Arc::new(builder.finish()),
            Self::Int16(builder) => Arc::new(builder.finish()),
            Self::UInt16(builder) => Arc::new(builder.finish()),
            Self::Int32(builder) => Arc::new(builder.finish()),
            Self::UInt32(builder) => Arc::new(builder.finish()),
            Self::Int64(builder) => Arc::new(builder.finish()),
            Self::UInt64(builder) => Arc::new(builder.finish()),
            Self::Float32(builder) => Arc::new(builder.finish()),
            Self::Float64(builder) => Arc::new(builder.finish()),
            Self::Decimal128 { builder, .. } => Arc::new(builder.finish()),
            Self::Utf8(builder) => Arc::new(builder.finish()),
            Self::LargeUtf8(builder) => Arc::new(builder.finish()),
            Self::Binary(builder) => Arc::new(builder.finish()),
            Self::LargeBinary(builder) => Arc::new(builder.finish()),
            Self::Date32(builder) => Arc::new(builder.finish()),
            Self::Time64(builder) => Arc::new(builder.finish()),
            Self::Timestamp(builder) => Arc::new(builder.finish()),
            Self::Null(builder) => Arc::new(builder.finish()),
        };
        Ok(array)
    }
}

fn push_int<T>(
    value: &Value,
    table: &str,
    column: &str,
    expected: &str,
    convert: impl FnOnce(i128) -> Option<T>,
    append: impl FnOnce(Option<T>),
) -> Result<(), ArrowError> {
    match value {
        Value::Null => {
            append(None);
            Ok(())
        }
        Value::Integer(number) => {
            let converted = convert(*number).ok_or_else(|| {
                ArrowError::InvalidArgumentError(format!(
                    "table {table}, column {column}: integer {number} does not fit {expected}"
                ))
            })?;
            append(Some(converted));
            Ok(())
        }
        other => Err(mismatch(table, column, expected, other)),
    }
}

fn fits_decimal_precision(mantissa: i128, precision: u8) -> bool {
    if precision >= 38 {
        return true;
    }
    let limit = 10_i128.pow(u32::from(precision));
    mantissa > -limit && mantissa < limit
}

fn mismatch(table: &str, column: &str, expected: &str, found: &Value) -> ArrowError {
    ArrowError::InvalidArgumentError(format!(
        "table {table}, column {column}: expected {expected}, found {found:?}"
    ))
}
