//! **Building the code index** (GitHub #767): the per-folder `kovan.toml`
//! files and the per-crate compact link index, from one rust-analyzer SCIP
//! run (#757), the source, `review.md` and git. Pure: the I/O (reading
//! files, running `rust-analyzer scip` and `git`) is `kovan-cli index` in
//! the `kovan` crate (`commands::index`).
//!
//! ```text
//!  rust-analyzer scip ──> SCIP index ──┬──> call graph (#757) ──┐
//!                                      │                        ├─[folders::build]──> <folder>/kovan.toml
//!  source .rs ──[review::hash]─────────┼────────────────────────┤      ^ ids: [ids] from review.md + previous kovan.toml
//!  review.md (read only) ──────────────┼────────────────────────┘      ^ [test_run] carried over (evidence)
//!  git log -D review.md ──[deleted]────┘──> crate root kovan.toml [[deleted_folder]]
//!                                      └──[links::build]──> <crate>/kovan_links.json
//!
//!  no rust-analyzer: source .rs ──[refresh]──> kovan.toml (hashes now; callees kept or
//!                                               flagged "index out of date")
//! ```
//!
//! - [`ids`]: stable `fn:` ids (claims from `review.md`, the previous index,
//!   minting with [`crate::review::id::mint_fn_id`]).
//! - [`folders`]: the full build of every folder's index.
//! - [`refresh`]: the rust-analyzer-free path (maintainer, #767).
//! - [`heal`]: when a `kovan.toml` on disk is rewritten or removed.
//! - [`deleted`]: deleted folders' review history from git.
//! - [`links`]: the per-crate link index format (definitions and
//!   references for web-kovan's go-to-definition, #745).
//! - [`upstream_draft`]: proposed `[upstream]` entries from provenance
//!   headers, printed for a human to confirm (never written).
//!
//! `review.md` is human-owned: nothing here writes it.

pub mod deleted;
pub mod folders;
pub mod heal;
pub mod ids;
pub mod links;
pub mod physical;
pub mod refresh;
pub mod test_run;
pub mod upstream_draft;
