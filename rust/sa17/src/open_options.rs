//! How to open a file.

use crate::cipher::StoreKey;

/// The most bytes of a value that was stored compressed, unless a program says
/// otherwise: 16 MiB.
const LONGEST_VALUE: u32 = 16 << 20;

/// How to open a file, for a program that wants it otherwise than
/// [`Database::open`](crate::Database::open) does it.
///
/// ```no_run
/// use sa17::{Database, OpenOptions};
///
/// let options = OpenOptions::new().longest_value(256 << 20);
/// let database = Database::open_with("garden.db", &options)?;
/// # Ok::<(), sa17::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct OpenOptions {
    pub(crate) key: Option<StoreKey>,
    pub(crate) longest_value: u32,
}

impl OpenOptions {
    /// Finds the key, and takes values of up to 16 MiB out of compression.
    #[must_use]
    pub fn new() -> Self {
        Self {
            key: None,
            longest_value: LONGEST_VALUE,
        }
    }

    /// Opens the file with `key`, and does not look for it.
    #[must_use]
    pub fn key(mut self, key: StoreKey) -> Self {
        self.key = Some(key);
        self
    }

    /// Takes values of up to `bytes` out of compression. A row with a longer
    /// one cannot be read with it.
    ///
    /// A few bytes can decompress to a thousand times as many, so this is what
    /// bounds the memory that a hostile file can use.
    #[must_use]
    pub fn longest_value(mut self, bytes: u32) -> Self {
        self.longest_value = bytes;
        self
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}
