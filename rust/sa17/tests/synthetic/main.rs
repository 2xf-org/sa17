//! Reads files that `sa17-synthetic` writes. The writer follows the
//! format's description and shares no code with the reader.

// Every function here runs inside a test, where a failed unwrap is a failed test.
#![allow(clippy::unwrap_used)]

mod catalog;
mod damage;
mod keys;
mod long_values;
mod page_lists;
mod rows;
mod sources;
mod store;
mod support;
mod values;
mod verification;
