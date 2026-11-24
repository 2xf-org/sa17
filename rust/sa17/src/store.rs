//! The page store: a file's pages, decoded, the lists of pages a table owns,
//! and the encoding of the file's text.

mod key_choice;
pub(crate) mod page;
mod page_cache;
pub(crate) mod page_list;
mod page_store;
mod record;
mod row_page;
mod stored_pages;
mod superblock;
mod text_encoding;

pub(crate) use page::Slot;
pub use page::{Page, PageKind};
pub(crate) use page_cache::PageCache;
pub use page_store::PageStore;
pub use record::Record;
pub(crate) use row_page::RowPage;
pub use stored_pages::PageState;
pub(crate) use stored_pages::StoredPages;
pub use superblock::Superblock;
pub use text_encoding::TextEncoding;
