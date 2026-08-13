//! # njoy-outram-park-fork
//!
//! Pure-Rust port (**in progress**) of [NJOY2016], the modular nuclear-data
//! processing system that reads evaluated nuclear data in ENDF format and
//! transforms it into libraries used by transport codes. Within OUTRAM PARK its
//! purpose is to produce the **ACE** continuous-energy libraries that
//! [`outram-mc-libs`] consumes — NJOY is the data-preparation step that sits
//! *upstream* of an OpenMC run.
//!
//! ## License & provenance — read this first
//!
//! This crate is a **derivative work** of NJOY2016 (v2016.79), which is licensed
//! under a modified BSD 3-Clause license (the LANL/DOE variant). That license is
//! GPL-compatible, so this translation is distributed under `GPL-3.0-only` — the
//! same license as the rest of the OUTRAM PARK workspace.
//!
//! It is **not** the version available from LANL and is **not** endorsed by
//! LANL / Los Alamos / the U.S. Government. The upstream copyright notice is
//! preserved verbatim in `LICENSE.njoy`; the full provenance and modification
//! statement is in `NOTICE`. Both files must travel with any redistribution.
//!
//! ## Pipeline (what gets ported, in dependency order)
//!
//! NJOY is a sequence of independent modules that pass ENDF "tapes" to each
//! other. The path that yields an OpenMC-ready ACE file is:
//!
//! ```text
//!   MODER → RECONR → BROADR → [HEATR] → [GASPR] → [PURR] → [THERMR] → ACER
//! ```
//!
//! See `docs/porting-plan.md` for the full module list, the C-source map, the
//! phased porting order, and the verification strategy (golden-file comparison
//! against upstream Fortran NJOY in `upstream_source/NJOY2016`).
//!
//! [NJOY2016]: https://github.com/njoy/NJOY2016
//! [`outram-mc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend

pub mod acer;
/// Data acquisition and the crate's **single on-disk artifact cache**.
///
/// Two halves, with different gating:
///
/// - **The cache substrate is always compiled.** [`acquire::EndfCache`] does
///   platform cache-dir discovery, per-artifact advisory locking, a
///   double-checked acquire, fsync + atomic rename, and a SHA-256 sidecar. It
///   has two producers: the download path below, and the **offline LEAPR
///   regeneration** path ([`leapr::generate`]), which is on by default.
/// - **The download path is behind the `net-fetch` feature** (opt-in): fetching
///   raw ENDF tapes from a pinned upstream and reconstructing pointwise cross
///   sections on device. The default build carries no TLS/HTTP dependency.
///
/// See [`acquire`] and `docs/data-acquisition.md`.
pub mod acquire;
pub mod broadr;
pub mod common;
pub mod endf;
/// `GASPR` — gas-production cross sections (ENDF MT=203–207: H1/H2/H3/He3/He4)
/// from a reconstructed evaluation. Informational (depletion/swelling), not a
/// transport-path quantity — see [`gaspr`] module docs.
pub mod gaspr;
/// `HEATR` — heating (KERMA) cross section, ENDF MT=301. Kinematic-limit
/// subset (H1–H4, `docs/porting-plan.md`); the full photon energy-balance
/// method and damage energy (MT=444) are deferred — see [`heatr`] module docs.
pub mod heatr;
/// User friendly interface to access njoy library (used to be called library,
/// but I renamed as interface to avoid mixing with lib.rs)
///
///
pub mod interface;
pub mod modules;
/// Nuclear-data **provider** surface consumed by downstream transport crates
/// (`outram-mc-libs`, …). All cross-section representation lives in this crate; see
/// [`nuclear_data`] and `docs/architecture.md`.
pub mod nuclear_data;
pub mod photon;
pub mod prelude;
pub mod reconr;
/// Thermal neutron scattering (the THERMR domain): read MF=7 S(α,β) evaluations
/// and, in future, compute bound-atom thermal cross sections. Distinct from
/// [`acer::thermal`], which *writes* the thermal ACE table.
pub mod thermr;
pub mod units;
/// Windowed Multipole (WMP) cross-section import — **ported** (ACER sub-block
/// 4g). This is independent **MIT CRPG** work (not NJOY/LANL); see [`wmp`] for
/// provenance and the MIT attribution requirements. Provides the analytic
/// on-the-fly Doppler-broadening evaluator (Faddeeva `w(z)`), HDF5 import behind
/// the `wmp-hdf5` feature, the WMPB/WMPL blob codecs, and a 125-nuclide CORE
/// library baked into the crate.
pub mod wmp;

// --- NJOY processing modules — one home per module under `src/<name>/`, each
// with its Rust source (physics or `NotPorted` stub) and co-located `README.md`.
// Most are ported: reconr, broadr, heatr, gaspr, thermr, acer, and wmp are
// declared above next to their long-form docs; unresr, purr, samm, groupr,
// gaminr, errorr, covr, leapr, dtfr, resxsr, and mixr (below) are also ported
// (translation-level — V&V is the trust gate). The genuine `NotPorted` stubs
// are only ccccr, matxsr, powr, plotr, viewr, and wimsr (output formats OUTRAM
// PARK does not target). Dispatch is via the `NjoyModule` enum in `modules.rs`.
pub mod moder;
pub mod unresr;
pub mod purr;
pub mod groupr;
pub mod gaminr;
pub mod errorr;
pub mod covr;
pub mod leapr;
pub mod samm;
pub mod dtfr;
pub mod ccccr;
pub mod matxsr;
pub mod resxsr;
pub mod powr;
pub mod plotr;
pub mod viewr;
pub mod mixr;
pub mod wimsr;

/// **Optional GPU compute** for njoy's embarrassingly-parallel kernels
/// (windowed-multipole cross-section evaluation across an energy grid).
///
/// Desktop builds carry a target-gated [`wgpu`](https://docs.rs/wgpu) backend
/// behind this module; **Android** (`target_os = "android"`) gets a pure-CPU
/// shim with the same call surface, so the crate still builds headless with no
/// GPU stack. At runtime [`gpu::probe`] returns `None` whenever no usable GPU
/// adapter is present (headless CI, emulators) and the caller must fall back to
/// the CPU path. The CPU path is the trusted/deterministic reference; the GPU
/// path (f32 WGSL) is acceleration only. See the module docs for the contract.
pub mod gpu;

/// **Full-fidelity GPU windowed-multipole evaluation** — the complete WMP cross
/// section (curve-fit background **plus** the complex Faddeeva pole-sum over each
/// window's poles, all three channels) on the GPU, across an energy grid.
///
/// This extends [`gpu`] (which does only the background polynomial). It reuses
/// [`gpu::GpuContext`]/[`gpu::probe`], ports the CPU [`wmp::faddeeva`] Weideman
/// approximation to WGSL (`vec2<f32>` complex arithmetic), and keeps the
/// mandatory CPU fallback: on Android, or when `probe()` returns `None`, callers
/// use the `f64` CPU reference [`wmp::WindowedMultipole::evaluate`]. The GPU
/// `f32` path is acceleration only; the CPU path stays the trusted reference.
#[cfg(not(target_os = "android"))]
pub mod gpu_wmp;

/// **Per-machine performance-report generator** — hardware detection (GPU label,
/// CPU cores, OS) plus a small markdown formatter, so a benchmark can emit a
/// fresh report on the machine it runs on and write it to a git-ignored local
/// path. Pure `std` (no `wgpu`), so it builds on every target including Android.
/// The committed benchmark markdown stays a methodology template; per-machine
/// timings live in each user's local report.
pub mod perf_report;

mod error;
pub use endf::MtReaction;
pub use error::NjoyError;
