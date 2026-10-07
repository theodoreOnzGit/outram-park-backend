//! **Code review data model and staleness engine** (GitHub #764, #765;
//! design decided on #739 and #740, 2026-10-06/07). Pure data and pure
//! functions: no filesystem, no git, no clock. Builds for wasm, so desktop
//! kovan, web-kovan and CI compute the same states from the same inputs.
//!
//! # The three files
//!
//! ```text
//! <workspace>/kovan_root.toml       reviewers and keys, pinned rust-analyzer,
//!                                   deleted-crate history       -> [`root`]
//! <folder>/kovan.toml               machine-owned, disposable cache: per file
//!                                   and function id, hash, doc_hash, callees,
//!                                   reaching tests, test evidence -> [`index`]
//! <folder>/review.md                human-owned: reviews, needs-fix,
//!                                   highlights, upstream confirmation,
//!                                   deleted-functions history,
//!                                   architecture nodes         -> [`review_md`]
//! ```
//!
//! # From source to state
//!
//! ```text
//!   source .rs ──[rust_items + hash]──> FnHashes ──┐
//!                                                  ├──> FolderIndex (kovan.toml)
//!   call graph / SCIP (#757) ──> callees, tests ───┘            │
//!                                                               v
//!   review.md ──[review_md]──> ReviewDocument ──> engine::evaluate ──> per-function StampState
//!                                                               ^
//!   git (kovan-discovery) ──> GitFacts (as data) ───────────────┘
//! ```
//!
//! # AI agents never stamp
//!
//! A review entry records that **a human** reviewed a function. This module
//! reads and writes the format; nothing here creates a review on anyone's
//! behalf, and the engine shows any stamp whose commit carries the agent
//! attribution trailer as unverified.
//!
//! # Not here yet
//!
//! - ~~Signature cryptography (GitHub #762): [`signing`] defines the field and
//!   the signed bytes; verification is a stub that never verifies.~~
//!   **CORRECTED 2026-10-07**: #762 landed; [`signing`] verifies ed25519
//!   stamps against the `[[reviewer]]` registry, and its native-only
//!   `keystore` generates and encrypts keys for desktop kovan.
//! - Hashing nested functions, constants and types (ruled stampable on #739,
//!   2026-10-06): [`index::ItemKind`] has room for them; only functions are
//!   hashed by [`hash`] today.
//! - Building `kovan.toml` from SCIP (#757) and the git side (kovan-discovery).
//! - The staleness engine itself (#765): `engine::evaluate` in the picture
//!   above is the next step and is not in this module yet.

pub mod hash;
pub mod index;
pub mod review_md;
pub mod root;
pub mod rust_items;
pub mod scope;
pub mod signing;
pub mod types;
