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
//! data/review_wizard.toml           the wizard's questions, sources and
//!                                   stamp gate (#769, embedded) -> [`wizard`]
//! ```
//!
//! ~~Test evidence (#766) is a fourth, workspace-level file,
//! `kovan_test_evidence.toml`~~ **CORRECTED 2026-10-07** (maintainer, #766):
//! test evidence lives in each folder's `kovan.toml` `[test_run]`, written
//! by `kovan-cli test` from a counted run -> [`evidence`],
//! [`crate::code_index::test_run`].
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
//! What an agent may write is an **unsigned justification**
//! (`kind = "justification"`, GitHub #1019; [`review_md`],
//! [`draft::draft_justification`], [`review_md_write::upsert_justification`]):
//! a verdict and evidence, labelled agent-drafted, which the engine ignores.
//!
//! # Not here yet
//!
//! - ~~Signature cryptography (GitHub #762): [`signing`] defines the field and
//!   the signed bytes; verification is a stub that never verifies.~~
//!   **CORRECTED 2026-10-07**: #762 landed; [`signing`] verifies ed25519
//!   stamps against the `[[reviewer]]` registry, and its native-only
//!   `keystore` generates and encrypts keys for desktop kovan.
//! - ~~Hashing nested functions, constants and types (ruled stampable on #739,
//!   2026-10-06): [`index::ItemKind`] has room for them; only functions are
//!   hashed by [`hash`] today.~~ **CORRECTED 2026-10-13** (#843): structs,
//!   enums, unions, traits, type aliases, constants and statics are hashed
//!   ([`hash::hash_items`]), written to `kovan.toml` with `item` and
//!   [`index::ItemShape`], and shown on the Code Review map. Still not here:
//!   hashing functions nested in another function's body; ~~the questions a
//!   review of a type or constant asks (stamping one is refused until they
//!   are built); and which functions use which type~~ (both built the same
//!   day: [`wizard::AskedOf`], [`index::FunctionIndex::uses`]); and the
//!   methods written for a type in another file than its definition.
//! - ~~Building `kovan.toml` from SCIP (#757) and the git side
//!   (kovan-discovery).~~ **CORRECTED 2026-10-07**: [`crate::code_index`]
//!   builds it (#767, `kovan-cli index`); the git facts the engine takes
//!   ([`engine::GitFacts`]) are ~~still~~ built by the caller (**CORRECTED
//!   2026-10-10**: `kovan::stamping::git_facts` now builds them from `git`,
//!   with the fields it leaves empty listed in its module doc).
//! - ~~The staleness engine itself (#765): `engine::evaluate` in the picture
//!   above is the next step and is not in this module yet.~~ **CORRECTED
//!   2026-10-07**: [`engine::evaluate`] is here (#765), with the state
//!   vocabulary in [`state`].
//! - Resolving a function's concept areas (for rung 5) from its review's
//!   `implements` relations: [`engine::evaluate`] takes them as data.
//!
//! Writing (GitHub #770, 2026-10-10): [`draft`] builds an unsigned stamp
//! from gathered facts, [`review_md_write`] splices one entry into a
//! `review.md` keeping every other byte, and [`root_append`] registers a
//! reviewer or key in `kovan_root.toml` as appended text. The gathering and
//! file I/O are `kovan::stamping`.
//!
//! Carry-forward (GitHub #838, #1018, 2026-10-22): when reviewed code
//! reaches a branch by a squash, merge, rebase or cherry-pick, an unsigned
//! record that anyone can re-check moves a stamp's key to the new commit,
//! and the signed stamp is never edited ([`carry_forward`]).
//!
//! Rung 5 is IV&V by a technically and managerially separate organisation
//! (GitHub #809, NUREG/BR-0167 §3.1): [`crate::review::ivv`].

pub mod carry_forward;
pub mod draft;
pub mod engine;
pub mod evidence;
pub mod hash;
pub mod id;
pub mod index;
pub mod ivv;
/// Plain-English text for rung-5 misses and registry warnings (#810).
pub mod ivv_text;
/// Rung 5 for the views: summaries, queue rows, attestation choices (#810).
pub mod ivv_view;
pub mod review_md;
pub mod review_md_write;
pub mod review_md_relocate;
pub mod root;
pub mod root_append;
pub mod rust_items;
pub mod scope;
pub mod signed_at;
pub mod signing;
pub mod state;
pub mod types;
pub mod wizard;
