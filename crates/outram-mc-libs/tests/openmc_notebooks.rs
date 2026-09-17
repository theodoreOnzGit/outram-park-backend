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
//!   a real result. Live slices today: `pincell` (Godiva bare-sphere k-eff +
//!   reflective CSG pin cell), `hexagonal_lattice`, `triso`, `depletion`,
//!   `flux_spectrum` (energy-binned track-length flux spectrum, op-6tz.9),
//!   `mg_mode_part_i`, and (added 2026-07-17, op-6tz-notebooks-batch)
//!   `tally_arithmetic`, `tally_power_normalization`, `expansion_filters`
//!   (z-Legendre), `post_processing` (regular-mesh flux), `capi`
//!   (in-memory build/run/introspect/edit/rerun), and `candu`
//!   (reduced concentric-ring cluster).
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
//! | `candu` | LIVE (reduced 7-pin concentric-ring cluster + per-pin flux tally; distribcell + D₂O are gaps) | op-6tz.11 |
//! | `cad_based_geometry` | ignored (WON'T PORT — DAGMC/CAD backend out of scope) | op-6tz.17 |
//! | `unstructured_mesh_part_i` | ignored (deferred to polyMesh) | op-6tz.32 |
//! | `unstructured_mesh_part_ii` | ignored (deferred to polyMesh) | op-6tz.32 |
//! | `tally_arithmetic` | LIVE (derived-tally +−×÷ w/ error propagation + analytic identities) | op-6tz.22 |
//! | `tally_power_normalization` | LIVE (kappa-fission tally + power-normalization round-trip) | op-6tz.22 |
//! | `expansion_filters` | LIVE (z-axis SpatialLegendre moments; Zernike is a gap) | op-6tz.14 |
//! | `flux_spectrum` | LIVE (energy-binned track-length flux spectrum) | op-6tz.9 |
//! | `gamma_detector` | ignored (photon transport OUT OF SCOPE per crate CLAUDE.md) | op-6tz.19 |
//! | `post_processing` | LIVE (RegularMesh + MeshFilter flux grid; on-disk StatePoint is a gap) | op-6tz.13 |
//! | `pandas_dataframes` | ignored (ON HOLD — Rust-native repr TBD, not pandas) | op-6tz.22 |
//! | `mg_mode_part_i` | LIVE (multigroup k∞ vs analytic 2-group) | op-6tz.15 |
//! | `mg_mode_part_ii` | ignored (MGXS-from-CE generation = njoy track) | op-6tz.6.3/.15 |
//! | `mg_mode_part_iii` | ignored (spatial MGXS-from-CE + lattice MG driver) | op-6tz.6.3/.15 |
//! | `depletion` | LIVE (one-group burnup: CRAM + chain_simple, inventory & k_inf trends) | op-6tz.18 |
//! | `capi` | LIVE partial (in-memory build/run/introspect/edit/rerun; batch-stepping is a gap) | op-6tz.20 |
//! | `shielded_room_weight_window` | ignored (weight-window VR + notebook absent upstream) | op-6tz.21 |
//! | `search` | LIVE (`search_for_keff` bisection driver, verified on the offline Godiva critical-radius analogue; exact boron-ppm PWR case is thermal-S(α,β)-data-gated) | op-6tz.6.5 |
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
#[path = "openmc_notebooks/search.rs"]
mod search;
