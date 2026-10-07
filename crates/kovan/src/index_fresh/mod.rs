//! **Index fresh** (GitHub #780): index ANY Rust workspace or single crate
//! into that repository itself, from the desktop app or
//! `kovan-cli index --fresh --workspace <dir>`. Both call [`run_fresh`],
//! which drives the one indexer, [`crate::commands::index::run_controlled`]
//! (#767); nothing here is a second indexer.
//!
//! Into the target it writes:
//!
//! 1. every folder's `kovan.toml` and each crate's `kovan_links.json`
//!    (the index run; the `kovan.toml` files are a disposable cache,
//!    regenerated silently by the existing heal logic);
//! 2. `kovan_root.toml` (maintainer decision on #780, option (b): the
//!    repository becomes its own kovan library), handled by
//!    [`root_file`] under **Leak Before Break** (`docs/kovan.md`): a valid
//!    one is kept untouched, a missing one is created (founder from the
//!    user's keystore identity or a visible "UNSET", an empty reviewer
//!    registry, an empty key history, the rust-analyzer version used), and
//!    a corrupted one is never silently overwritten: its parse error is
//!    shown, it is kept as `kovan_root.toml.corrupt-<date>`, and only then,
//!    by the user's choice, the last committed version that parses is
//!    restored or a fresh one written;
//! 3. a `review.md` skeleton in each indexed folder that has none
//!    ([`skeleton`]), a separate step, through the existing review.md
//!    writer. An existing `review.md` is never written; one with unreadable
//!    entries is reported for the existing redo flow.
//!
//! Nothing is committed or pushed: the user commits.
//!
//! Workspace or crate is decided from `Cargo.toml` alone
//! ([`detect::detect_project`]; maintainer, 2026-10-07: `[workspace]` vs
//! `[package]`, no other heuristic).

pub mod cli;
pub mod detect;
pub mod plan;
pub mod recent;
pub mod root_file;
pub mod skeleton;

#[cfg(test)]
mod tests;

pub use cli::{resolve_index_root, run_cli};
pub use detect::{detect_project, DetectError, ProjectKind};
pub use plan::{
    cost_message, plan_fresh, run_fresh, CostEstimate, FileAction, FreshChoices, FreshError,
    FreshPlan, FreshReport, PlannedFile,
};
pub use root_file::{CommittedRoot, CorruptAction, FounderChoice, RootOutcome, RootState, ROOT_FILE};

/// Today's UTC date, `YYYY-MM-DD`.
pub fn today() -> String {
    let now = crate::digitiser::dataset::utc_now_iso8601();
    now.get(..10).unwrap_or(&now).to_string()
}
