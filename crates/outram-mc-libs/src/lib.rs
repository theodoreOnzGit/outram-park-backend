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
//! ~~`GPL-3.0-only`~~ **`AGPL-3.0-only`** (relicensed 2026-10-20, gh:#973;
//! see `docs/relicensing-agpl-2026-10-20.md`) — the same license as the rest
//! of the OUTRAM PARK workspace, as permitted by the terms of the upstream MIT
//! license.
//!
//! **This is OUTRAM PARK's independent Rust translation of selected OpenMC
//! algorithms — it is not the official OpenMC software, and is not
//! affiliated with, endorsed by, or sanctioned by MIT or Argonne National
//! Laboratory.** See `TRADEMARKS.md` (this crate's directory, mirrored from
//! the workspace root) for the full attribution and non-affiliation notice.

// Rustdoc without source pages (teaching site, gh:#518): lessons show
// anchored snippets and link to GitHub at the build commit instead.
#![doc(html_no_source)]

// `no_std` + `alloc` without the default `std` feature (gh:#1605, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports))]

extern crate alloc;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
/// `std::sync::{RwLock, PoisonError}` in a `std` build, unchanged. Without
/// `std` (gh:#1605), a `spin::RwLock` behind the same `new` / `read` /
/// `write` signatures: a spin lock is never poisoned, so its `Result` is
/// always `Ok`, and `PoisonError::into_inner` keeps the `std` call sites'
/// `.unwrap_or_else(PoisonError::into_inner)` building unchanged.
pub(crate) mod sync {
    #[cfg(feature = "std")]
    pub(crate) use std::sync::{PoisonError, RwLock};

    #[cfg(not(feature = "std"))]
    pub(crate) use nostd::{PoisonError, RwLock};

    #[cfg(not(feature = "std"))]
    mod nostd {
        /// Never constructed: a spin lock cannot be poisoned.
        #[derive(Debug)]
        pub(crate) struct PoisonError<T>(T);

        impl<T> PoisonError<T> {
            pub(crate) fn into_inner(self) -> T {
                self.0
            }
        }

        #[derive(Debug, Default)]
        pub(crate) struct RwLock<T>(spin::RwLock<T>);

        impl<T> RwLock<T> {
            pub(crate) const fn new(value: T) -> Self {
                Self(spin::RwLock::new(value))
            }

            pub(crate) fn read(
                &self,
            ) -> Result<spin::RwLockReadGuard<'_, T>, PoisonError<spin::RwLockReadGuard<'_, T>>> {
                Ok(self.0.read())
            }

            pub(crate) fn write(
                &self,
            ) -> Result<spin::RwLockWriteGuard<'_, T>, PoisonError<spin::RwLockWriteGuard<'_, T>>>
            {
                Ok(self.0.write())
            }
        }
    }
}

/// `std::env::var` in a `std` build, unchanged. Without `std` (gh:#1605)
/// there is no environment, so every variable reads as unset and each
/// `OUTRAM_MC_*` switch takes its default.
pub(crate) mod env {
    #[cfg(feature = "std")]
    pub(crate) use std::env::var;

    /// The one error a `no_std` lookup gives: not present.
    #[cfg(not(feature = "std"))]
    #[derive(Debug)]
    pub(crate) struct VarError;

    #[cfg(not(feature = "std"))]
    pub(crate) fn var(_key: &str) -> Result<alloc::string::String, VarError> {
        Err(VarError)
    }
}

/// `std::thread::available_parallelism` in a `std` build, unchanged.
/// Without `std` (gh:#1605) there are no threads: one.
pub(crate) mod thread {
    #[cfg(feature = "std")]
    pub(crate) use std::thread::available_parallelism;

    #[cfg(not(feature = "std"))]
    pub(crate) fn available_parallelism() -> Result<core::num::NonZeroUsize, ()> {
        Ok(core::num::NonZeroUsize::MIN)
    }
}

/// `HashMap` / `HashSet`: `std`'s in a `std` build, `hashbrown`'s without
/// (workspace rule, `docs/claude-md/rust-design-rules.md`).
#[allow(unused_imports)]
pub(crate) mod hash {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::collections::{HashMap, HashSet};
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use hashbrown::{HashMap, HashSet};
}

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use nostd_prelude::*;

/// The error type every ENDF-reading entry point in this crate returns
/// (`Nuclide::from_endf_file`, `from_tape`, `from_endf`).
///
/// Re-exported so callers can name it without taking their own dependency on
/// `njoy-outram-park-fork`.
pub use njoy_outram_park_fork::NjoyError;

pub mod mathf;
pub mod rng;
pub mod geometry;
pub mod particle;
pub mod material;
pub mod source;
pub mod tally;
pub mod physics;
pub mod dh_universe;
pub mod pebble_beds;
pub mod stochastic;
pub mod depletion;
/// The radionuclide inventory a depletion run hands to a source-term
/// calculation (`sembawang`), or that a caller prescribes: nuclides by
/// Z/A/metastable state, `uom` amounts by region, the time point and who
/// produced it (gh:#951).
pub mod inventory;
/// Waste classification and disposal rating from an activated inventory:
/// a port of OpenMC's `openmc/waste.py` (gh:#1391).
pub mod waste;
pub mod plotter;
/// Beginner thermal profile: a coarse steady conduction solve on any CSG
/// shape (outram-foam-basic-lib), from a stated power and a ballpark
/// heat-removal preset, with an energy balance (gh:#949).
pub mod thermal_profile;
/// Optional headless GPU compute (wgpu) for embarrassingly-parallel MC kernels.
/// Desktop gets the real path; Android gets a CPU-only shim. GPU is acceleration
/// only — the CPU raw-`f64` path stays the trusted, deterministic reference.
pub mod gpu;
/// Per-machine performance-report generator: detects this host's GPU / CPU / OS
/// and renders a self-service "what performance is available on my PC" markdown
/// report from measured benchmark timings. Machine-specific output is written to
/// a gitignored local path — see [`perf_report`].
#[cfg(feature = "std")]
pub mod perf_report;
/// Per-run diagnostic record: which nuclear-data files a run used, how long
/// each took to process, and how long transport took — reported separately,
/// because they scale with different things. Written to the gitignored
/// local-perf directory. See [`run_diagnostics`].
#[cfg(feature = "std")]
pub mod run_diagnostics;
#[cfg(feature = "std")]
pub mod vv;
/// Statistics for Monte Carlo results on top of RAFFLES (epic GitHub #493):
/// seed ensembles, correlated-sample `σ`, source-convergence diagnostics,
/// learned weight windows, UQ and surrogate sweeps. Report-only or opt-in;
/// no driver calls it. See [`stats`].
pub mod stats;
/// **The beginner API** (epic GitHub #925): materials described as an
/// engineer or a benchmark describes them, shapes filled with materials, a
/// `Model` and a k_eff run sized by one `MCSettings` word, lowered onto the
/// transport below. Start at [`model::prelude`].
pub mod model;
pub mod prelude;

/// Serial stand-ins for the `rayon` surface this crate uses, on `wasm32` where
/// `rayon` does not build. Numerically exact here — see the module docs.
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
mod wasm_par;
