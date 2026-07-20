//! OpenMC data-notebooks as njoy verification tests — harness.
//!
//! Part of beads epic **op-6tz** (this crate's slice **op-6tz.6**): every
//! notebook in [openmc-dev/openmc-notebooks] becomes a verification test as the
//! workspace grows an OpenMC-like API. This crate owns the **data /
//! cross-section-generation** notebooks; the mapping of each notebook to the
//! njoy API it exercises (and what is a GAP) lives in
//! `docs/openmc-notebooks-data-verification.md`.
//!
//! Notebook provenance: `github.com/openmc-dev/openmc-notebooks`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT-licensed, OpenMC project).
//!
//! Each submodule below corresponds to one notebook. Tractable operations are
//! **live** tests; operations that need an API this crate does not yet have are
//! scaffolded `#[ignore]` with the missing capability named (and a per-notebook
//! bead). This suite is a **directory test target** (`tests/openmc_notebooks_data/`
//! with this `main.rs`), matching the crate's existing
//! `tests/u238_doppler_verification/main.rs` pattern, so all notebook modules
//! link into one test binary. Run it with:
//!
//! ```bash
//! crates/njoy-outram-park-fork/scripts/test.sh openmc_notebooks_data
//! ```
//!
//! [openmc-dev/openmc-notebooks]: https://github.com/openmc-dev/openmc-notebooks

mod nuclear_data;
mod nuclear_data_resonance_covariance;
mod search;
mod mgxs_part_i;
mod mgxs_part_ii;
mod mgxs_part_iii;
mod mdgxs_part_i;
mod mdgxs_part_ii;
