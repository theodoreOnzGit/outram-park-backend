//! # outram-mc-libs
//!
//! Pure-Rust port of selected [OpenMC](https://openmc.org) Monte Carlo
//! neutron-transport kernels (RNG, geometry/CSG, particle tracking,
//! k-eigenvalue and fixed-source drivers, delta/Woodcock tracking). Data-free:
//! cross sections are pulled from `njoy-outram-park-fork`'s `XsProvider` surface.
//!
//! ## License & provenance — read this first
//!
//! This crate is a **derivative work** of [OpenMC](https://openmc.org),
//! copyright the OpenMC development team (MIT, Massachusetts Institute of
//! Technology) and Argonne National Laboratory, licensed MIT. That license
//! is GPL-compatible, so this translation is distributed under
//! `GPL-3.0-only` — the same license as the rest of the OUTRAM PARK
//! workspace, as permitted by the terms of the upstream MIT license.
//!
//! **This is OUTRAM PARK's independent Rust translation of selected OpenMC
//! algorithms — it is not the official OpenMC software, and is not
//! affiliated with, endorsed by, or sanctioned by MIT or Argonne National
//! Laboratory.** See `TRADEMARKS.md` (this crate's directory, mirrored from
//! the workspace root) for the full attribution and non-affiliation notice.

pub mod rng;
pub mod geometry;
pub mod particle;
pub mod material;
pub mod source;
pub mod tally;
pub mod physics;
pub mod pebble_beds;
pub mod stochastic;
pub mod depletion;
/// Optional headless GPU compute (wgpu) for embarrassingly-parallel MC kernels.
/// Desktop gets the real path; Android gets a CPU-only shim. GPU is acceleration
/// only — the CPU raw-`f64` path stays the trusted, deterministic reference.
pub mod gpu;
/// Per-machine performance-report generator: detects this host's GPU / CPU / OS
/// and renders a self-service "what performance is available on my PC" markdown
/// report from measured benchmark timings. Machine-specific output is written to
/// a gitignored local path — see [`perf_report`].
pub mod perf_report;
pub mod prelude;

/// Serial stand-ins for the `rayon` surface this crate uses, on `wasm32` where
/// `rayon` does not build. Numerically exact here — see the module docs.
#[cfg(target_arch = "wasm32")]
mod wasm_par;
