//! What is wrong with a catalog.

use crate::error::Error;

/// A catalog keeps at most this many problems, so that a file of nothing else
/// does not take many times its size in memory.
const MOST_PROBLEMS: usize = 1000;

/// What was found wrong with a catalog, in the order it was found.
#[derive(Default)]
pub(crate) struct Problems {
    kept: Vec<Error>,
}

impl Problems {
    pub(crate) fn add(&mut self, problem: impl Into<Error>) {
        if self.kept.len() < MOST_PROBLEMS {
            self.kept.push(problem.into());
        }
    }

    pub(crate) fn as_slice(&self) -> &[Error] {
        &self.kept
    }
}
