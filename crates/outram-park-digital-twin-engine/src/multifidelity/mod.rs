//! Mapping high-fidelity models onto low-fidelity ones (epic gh:#1177).
//!
//! **Infrastructure, not physics** (this crate's `CLAUDE.md`, "No new physics
//! in this crate's library"): the modules here run a high-fidelity model and
//! a low-fidelity one that live in other crates at the same design points,
//! read closures off the first, and hand them to the second. Every
//! correlation, governing equation and property relation they evaluate
//! belongs to the crate that owns the model.
//!
//! | Module | Pair | Rung |
//! |---|---|---|
//! | [`double_pipe`] | the double-pipe conjugate-heat-transfer CFD (`outram-foam-appbuilder-lib`) → the TUAS 1-D shell-and-tube exchanger with one tube | 1, closure extraction (gh:#1178) |
pub mod double_pipe;
