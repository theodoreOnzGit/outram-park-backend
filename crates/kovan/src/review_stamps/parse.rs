//! Finding a function in one Rust source file with a real parser (`syn`),
//! and the normalised text a stamp's hash is taken over.
//!
//! **Moved 2026-10-07** to `kovan_common::review::rust_items` (GitHub #764;
//! placement decided on #743), so web-kovan and the staleness engine hash
//! functions with the same code. Every item is re-exported here unchanged;
//! the module documentation (which functions exist, how a path names one,
//! the normalisation) lives there.

pub use kovan_common::review::rust_items::*;
