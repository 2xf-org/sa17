//! The columns of a table that hold the key of a row elsewhere.

/// A foreign key as the catalog describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignKey {
    pub(crate) name: String,
    pub(crate) positions: Vec<usize>,
    pub(crate) target_id: u32,
    pub(crate) target_positions: Vec<usize>,
}

impl ForeignKey {
    /// The key's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The positions of the key's columns among the table's own columns.
    #[must_use]
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    /// The identifier of the table that the key names a row of.
    #[must_use]
    pub fn target_id(&self) -> u32 {
        self.target_id
    }

    /// The positions among the target's columns that each of
    /// [`Self::positions`] holds the value of, in the same order.
    #[must_use]
    pub fn target_positions(&self) -> &[usize] {
        &self.target_positions
    }
}
