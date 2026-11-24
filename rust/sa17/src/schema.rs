//! The tables of a file and their columns, as the catalog describes them.

mod column;
mod data_type;
mod foreign_key;
mod index;
mod length_unit;
mod page_set;
mod table_definition;

pub use column::Column;
pub use data_type::DataType;
pub use foreign_key::ForeignKey;
pub use index::{Index, IndexColumn, IndexKind, IndexOrder};
pub use length_unit::LengthUnit;
pub(crate) use page_set::PageSet;
pub use table_definition::{TableDefinition, TableKind};
