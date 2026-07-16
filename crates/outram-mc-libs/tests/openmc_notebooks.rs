//! OpenMC-notebooks verification harness (outram-mc subset).
//!
//! ⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.
//!
//! Each OpenMC notebook in
//! <https://github.com/openmc-dev/openmc-notebooks> (pinned commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT) becomes a verification test
//! for the OUTRAM PARK Monte Carlo path. This binary hosts the **outram-mc
//! subset** (transport / geometry / tally / depletion / variance-reduction);
//! the nuclear-data notebooks live in the parallel njoy track.
//!
//! Tracked under beads epic **op-6tz**. The notebook → test → required-API map
//! is `docs/openmc-notebooks-verification.md`.
//!
//! ## How this harness reports honestly
//!
//! - **Live** tests run a *faithful* slice against the current API and assert on
//!   a real result. Today only `pincell` has a live slice (the Godiva
//!   bare-sphere k-eff, the sole working end-to-end path).
//! - **Ignored** tests carry `#[ignore = "requires API X (op-6tz.N)"]` and an
//!   `unimplemented!(...)` body, so they never report a fake green and, if the
//!   ignore is removed before the API exists, they fail loudly. Each names the
//!   gap bead that tracks the missing OpenMC-equivalent API.
//!
//! ## Module map
//!
//! | Notebook module | Status | Gap bead |
//! |---|---|---|
//! | `pincell` | partial LIVE (Godiva bare-sphere k-eff) | op-6tz.7/.10/.12 |
//! | `hexagonal_lattice` | LIVE (geometry correctness + k smoke, no notebook reference) | op-6tz.11 |
//! | `triso` | LIVE (random-packed doubly-het k∞ by delta tracking) | op-6tz.16/.25 |
//! | `candu` | ignored | op-6tz.11 |
//! | `cad_based_geometry` | ignored | op-6tz.17 |
//! | `unstructured_mesh_part_i` | ignored | op-6tz.17 |
//! | `unstructured_mesh_part_ii` | ignored | op-6tz.17 |
//! | `tally_arithmetic` | ignored | op-6tz.22 |
//! | `tally_power_normalization` | ignored | op-6tz.22 |
//! | `expansion_filters` | ignored | op-6tz.14 |
//! | `flux_spectrum` | ignored | op-6tz.9 |
//! | `gamma_detector` | ignored | op-6tz.19 |
//! | `post_processing` | ignored | op-6tz.22 |
//! | `pandas_dataframes` | ignored | op-6tz.22 |
//! | `mg_mode_part_i` | ignored | op-6tz.15 |
//! | `mg_mode_part_ii` | ignored | op-6tz.15 |
//! | `mg_mode_part_iii` | ignored | op-6tz.15 |
//! | `depletion` | LIVE (one-group burnup: CRAM + chain_simple, inventory & k_inf trends) | op-6tz.18 |
//! | `capi` | ignored | op-6tz.20 |
//! | `shielded_room_weight_window` | ignored (notebook absent upstream) | op-6tz.21 |
//!
//! Run the live subset:
//! ```text
//! cargo test -p outram-mc-libs --release --test openmc_notebooks
//! ```
//! List every mapped (incl. ignored) case:
//! ```text
//! cargo test -p outram-mc-libs --release --test openmc_notebooks -- --list --ignored
//! ```

// Submodules live in tests/openmc_notebooks/<name>.rs. They are declared with
// #[path] (not `mod openmc_notebooks;`) so the entry-file basename does not
// collide with the directory name, and so cargo does not treat the per-notebook
// files as separate test binaries (only tests/*.rs are test targets).

#[path = "openmc_notebooks/pincell.rs"]
mod pincell;

#[path = "openmc_notebooks/hexagonal_lattice.rs"]
mod hexagonal_lattice;
#[path = "openmc_notebooks/triso.rs"]
mod triso;
#[path = "openmc_notebooks/candu.rs"]
mod candu;
#[path = "openmc_notebooks/cad_based_geometry.rs"]
mod cad_based_geometry;
#[path = "openmc_notebooks/unstructured_mesh_part_i.rs"]
mod unstructured_mesh_part_i;
#[path = "openmc_notebooks/unstructured_mesh_part_ii.rs"]
mod unstructured_mesh_part_ii;
#[path = "openmc_notebooks/tally_arithmetic.rs"]
mod tally_arithmetic;
#[path = "openmc_notebooks/tally_power_normalization.rs"]
mod tally_power_normalization;
#[path = "openmc_notebooks/expansion_filters.rs"]
mod expansion_filters;
#[path = "openmc_notebooks/flux_spectrum.rs"]
mod flux_spectrum;
#[path = "openmc_notebooks/gamma_detector.rs"]
mod gamma_detector;
#[path = "openmc_notebooks/post_processing.rs"]
mod post_processing;
#[path = "openmc_notebooks/pandas_dataframes.rs"]
mod pandas_dataframes;
#[path = "openmc_notebooks/mg_mode_part_i.rs"]
mod mg_mode_part_i;
#[path = "openmc_notebooks/mg_mode_part_ii.rs"]
mod mg_mode_part_ii;
#[path = "openmc_notebooks/mg_mode_part_iii.rs"]
mod mg_mode_part_iii;
#[path = "openmc_notebooks/depletion.rs"]
mod depletion;
#[path = "openmc_notebooks/capi.rs"]
mod capi;
#[path = "openmc_notebooks/shielded_room_weight_window.rs"]
mod shielded_room_weight_window;
