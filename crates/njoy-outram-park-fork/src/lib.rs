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
//! This crate is a **derivative work** of NJOY2016 (~~v2016.79~~ **v2016.80**, synced 2026-11-10, gh:#1368), which is licensed
//! under a modified BSD 3-Clause license (the LANL/DOE variant). That license is
//! GPL-compatible, so this translation is distributed under ~~`GPL-3.0-only`~~ **`AGPL-3.0-only`** (relicensed 2026-10-20, gh:#973) — the
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

// `no_std` + `alloc` without the default `std` feature (gh:#1592, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// Without `std` the file, cache and stream entry points are compiled out, and
// the helpers only they call (WMP's temp-dir front end, RESXSR's and MIXR's
// stream drivers, the LEAPR disk cache) are left unused.
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports))]

extern crate alloc;
extern crate thiserror_nostd as thiserror;
extern crate sha2_nostd as sha2;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
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

/// `OnceLock` and `RwLock` under one name for both builds: `std`'s with
/// `std` (and in tests), [`nostd_sync`]'s without (gh:#1592).
pub(crate) mod once {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::sync::OnceLock;
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use crate::nostd_sync::OnceLock;
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::sync::RwLock;
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use crate::nostd_sync::RwLock;
}

/// `no_std` stand-ins for the two `std::sync` / `std::thread` tools this crate
/// uses (gh:#1592). Only built without `std`; a `std` build uses `std`'s own.
///
/// - [`nostd_sync::OnceLock`]: the embedded-library caches (`WmpLibrary::core`,
///   the fast MGXS core, the erfc and photon-angle tables), on `spin::Once`.
/// - [`nostd_sync::Local`]: the Fortran `save` state upstream keeps in module
///   variables (RECONR's `errmax`/`errint` override and scratch-tape mode,
///   GROUPR's weight-function state, ACER's binary-units flag). With `std`
///   each is a `thread_local!`, so parallel runs never see each other's.
///   **Without `std` each is ONE global value behind a spin lock**: the
///   `no_std` build is single-context. Two cores running RECONR or GROUPR at
///   the same time on bare metal would share that state, exactly as two
///   calls in one Fortran process do upstream.
#[cfg(not(any(feature = "std", test)))]
pub(crate) mod nostd_sync {
    /// `std::sync::OnceLock`'s `new` / `get_or_init`, on `spin::Once`.
    pub(crate) struct OnceLock<T>(spin::Once<T>);

    impl<T> OnceLock<T> {
        pub(crate) const fn new() -> Self {
            Self(spin::Once::new())
        }

        pub(crate) fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
            self.0.call_once(f)
        }
    }

    /// What a `thread_local!` key gives this crate (`with`), as one global
    /// value behind a spin lock. See the module docs for what that means.
    pub(crate) struct Local<T>(spin::Mutex<T>);

    impl<T> Local<T> {
        pub(crate) const fn new(value: T) -> Self {
            Self(spin::Mutex::new(value))
        }

        pub(crate) fn with<R>(&'static self, f: impl FnOnce(&T) -> R) -> R {
            f(&self.0.lock())
        }
    }

    /// `std::sync::RwLock`'s `new` / `read` / `write`, on `spin::RwLock`. A
    /// spin lock cannot be poisoned, so the `Result` is always `Ok`; it is
    /// there so the `std` call sites (`.expect(..)`, `if let Ok(..)`) build
    /// unchanged.
    #[derive(Debug, Default)]
    pub(crate) struct RwLock<T>(spin::RwLock<T>);

    impl<T> RwLock<T> {
        pub(crate) const fn new(value: T) -> Self {
            Self(spin::RwLock::new(value))
        }

        pub(crate) fn read(
            &self,
        ) -> Result<spin::RwLockReadGuard<'_, T>, core::convert::Infallible> {
            Ok(self.0.read())
        }

        pub(crate) fn write(
            &self,
        ) -> Result<spin::RwLockWriteGuard<'_, T>, core::convert::Infallible> {
            Ok(self.0.write())
        }
    }
}

/// A `no_std` stand-in for `std::path::Path`, for the functions whose path
/// only *labels* an error (the ACE byte readers, the gzip inflater): they
/// keep their `std` signatures exactly, and build without `std` against this
/// (gh:#1592). It holds a name, never touches a filesystem, and offers only
/// what those functions call: `new`, `display` and `to_string_lossy`.
#[cfg(not(feature = "std"))]
pub mod nostd_path {
    use alloc::borrow::Cow;

    /// A file name with no filesystem behind it. See the module docs.
    #[repr(transparent)]
    pub struct Path(str);

    impl Path {
        /// View a name as a `Path`.
        pub fn new(name: &str) -> &Path {
            // SAFETY: `Path` is `repr(transparent)` over `str`.
            unsafe { &*(name as *const str as *const Path) }
        }

        /// The name, for an error message.
        pub fn display(&self) -> &str {
            &self.0
        }

        /// The name, as `std`'s `Path::to_string_lossy` would give it.
        pub fn to_string_lossy(&self) -> Cow<'_, str> {
            Cow::Borrowed(&self.0)
        }
    }

    impl AsRef<Path> for str {
        fn as_ref(&self) -> &Path {
            Path::new(self)
        }
    }
}

pub mod acer;
/// Data acquisition and the crate's **single on-disk artifact cache**.
///
/// Two halves, with different gating:
///
/// - ~~**The cache substrate is always compiled.**~~ **CORRECTED 2026-11-16 (gh:#1592):** the cache
///   substrate is compiled with the default `std` feature, not without it.
///   [`acquire::EndfCache`] does
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
/// **ZIP archives in memory**, read and written over `miniz_oxide` (gh #935):
/// a whole ENDF library arrives as a zip, natively or as a browser upload.
/// See [`archive`].
pub mod archive;
pub mod broadr;
/// NJOY input-deck driver (gh:#1369): card reader, tape units, `run_deck`. See [`deck`].
pub mod deck;
pub mod common;
pub mod endf;
/// A folder of ENDF tapes, flat or an extracted library with sub-library
/// folders: list it, and find a tape in it by MAT and NSUB rather than by
/// file name (gh:#581). See [`endf_folder`].
#[cfg(any(feature = "std", test))]
pub mod endf_folder;
/// **A whole ENDF library** from a folder, a zip or uploaded files: one index
/// for all three, layout read from the names, identity from each tape's
/// header (gh #935); ACE generation from it for the browser (gh #936). See
/// [`endf_library`].
pub mod endf_library;
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
/// **The high-level way in** (GitHub #871): an incident ENDF tape and a
/// temperature to cross sections ([`process::cross_sections`]) or an ACE file
/// ([`process::ace`]); with a LEAPR deck, to a thermal scattering law
/// ([`process::thermal_scattering`]) or a thermal ACE file
/// ([`process::thermal_ace`]). Runs on `wasm32`.
pub mod process;
pub mod reconr;
/// Locating the repository's reference ENDF tapes for V&V. The tapes live at
/// the repo root (`reference-data/endf/`), never inside a crate — they are far
/// too large for a crates.io package.
#[cfg(any(feature = "std", test))]
pub mod reference_data;
/// Isomeric branching ratios from MF=8/9/10, collapsed on a group spectrum
/// (gh:#1628).
pub mod isomeric_branching;
/// Thermal neutron scattering (the THERMR domain): read MF=7 S(α,β) evaluations
/// and, in future, compute bound-atom thermal cross sections. Distinct from
/// [`acer::thermal`], which *writes* the thermal ACE table.
pub mod thermr;
pub mod units;
#[cfg(any(feature = "std", test))]
pub mod vv;
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
// gaminr, errorr, covr, leapr, dtfr, resxsr, mixr and wimsr (below) are also
// ported (translation-level — V&V is the trust gate). The genuine `NotPorted`
// stubs are only ~~ccccr,~~ ~~matxsr,~~ ~~powr~~ ~~and plotr~~ (output formats OUTRAM
// PARK does not target). **CORRECTED 2026-11-14 (gh:#1373):** powr `lib = 1` and `lib = 2` are
// translated (`src/powr/`, libraries identical to NJOY2016.80 on seven decks); `lib = 3` (CPM)
// and the bare `run()` stay `NotPorted`. **CORRECTED 2026-11-13 (gh:#1369):** matxsr is translated (`src/matxsr/`, the MATXS
// files of decks 03 and 30 identical to NJOY2016.80); only its bare `run()` stays `NotPorted`. **CORRECTED 2026-11-13 (gh:#1369):** plotr is translated (`src/plotr/`,
// decks 06 and 12 identical to NJOY2016.80); only its bare `run()` stays `NotPorted`. **CORRECTED 2026-11-13 (gh:#1369):** ccccr is ported (`src/ccccr/`, ISOTXS/BRKOXS/DLAYXS byte-identical to NJOY2016.80 on decks 02 and 76); only its bare `run()` stays `NotPorted`. ~~…and viewr~~ **CORRECTED 2026-11-12 (gh:#1369):** viewr and
// graph.f90 are ported (`src/viewr/`, PostScript identical to NJOY2016.80 on 17 upstream decks);
// only its bare `run()` stays `NotPorted` (it needs the deck's tape units). ~~…and wimsr~~ CORRECTED 2026-10-04: wimsr was ported
// 2026-09-11 (`src/wimsr/`, `wimsr::run_gendf`, byte-identical library vs
// NJOY2016 in `tests/wimsr_u238_njoy_golden.rs`); only its card-deck `run()`
// stays `NotPorted`. Dispatch is via the `NjoyModule` enum in `modules.rs`.
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
#[cfg(feature = "std")]
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
#[cfg(all(feature = "std", not(target_os = "android")))]
pub mod gpu_wmp;

/// OpenMC's per-collision record as plain data, in every build (gh:#1605);
/// [`hdf5::collision_track`] writes it to `collision_track.h5`.
pub mod collision_track_site;
/// Read and write for the **OpenMC interchange formats** (HDF5 nuclide, MGXS,
/// WMP and statepoint files, `cross_sections.xml`, the depletion chain) —
/// see the module docs. (CORRECTED 2026-10-04: the `perf_report` doc comment
/// below used to sit on this line, so rustdoc showed it for `hdf5`.)
/// (CORRECTED 2026-11-18: this comment then sat above `collision_track_site`,
/// so rustdoc showed it for that module.) ~~`std` only.~~ **CHANGED
/// 2026-11-18 (gh:#1648):** without `std` the module carries the depletion
/// chain from text (`depletion_chain_xml`, `depletion_chain_ops`,
/// `depletion_chain_from_endf`); the file and HDF5 readers and writers are
/// `std` only.
pub mod hdf5;
/// **Per-machine performance-report generator** — hardware detection (GPU label,
/// CPU cores, OS) plus a small markdown formatter, so a benchmark can emit a
/// fresh report on the machine it runs on and write it to a git-ignored local
/// path. Pure `std` (no `wgpu`), so it builds on every target including Android.
/// The committed benchmark markdown stays a methodology template; per-machine
/// timings live in each user's local report.
#[cfg(any(feature = "std", test))]
pub mod perf_report;

mod error;
pub use endf::MtReaction;
pub use error::NjoyError;

/// Serial stand-ins for the `rayon` adapters this crate uses, on `wasm32`
/// where `rayon` does not build. See the module docs for why the arithmetic is
/// unaffected.
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
mod wasm_par;
