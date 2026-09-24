//! Windowed Multipole (WMP) cross sections — analytic on-the-fly Doppler broadening.
//!
//! # Provenance — read first (this is NOT NJOY / NOT LANL)
//!
//! Windowed Multipole is **independent of NJOY2016 and of LANL**. It is the work
//! of the **MIT Computational Reactor Physics Group (CRPG)**. Nothing in this
//! module derives from the NJOY BSD/LANL sources, and its attribution must be
//! kept separate from the crate's NJOY `LICENSE.njoy` / `NOTICE` files.
//!
//! - **Data library:** <https://github.com/mit-crpg/WMP_Library> — © MIT CRPG,
//!   distributed under the **MIT License** (GPL-compatible, so it may coexist
//!   with this crate's GPL-3.0 licensing).
//! - **Method:** the multipole representation of R-matrix cross sections
//!   (R. N. Hwang, *Nucl. Sci. Eng.* 1987) with the windowing/curve-fit scheme
//!   of **C. Josey, P. Romano, B. Forget, K. Smith** (*J. Comput. Phys.* 2016;
//!   *Ann. Nucl. Energy* 2015/2016). Credit MIT CRPG and these authors in any
//!   derived work; do **not** imply LANL endorsement.
//!
//! The CORE data blob (`src/data/wmp_core.wmpl`) is now embedded; its MIT CRPG
//! provenance is credited in `LICENSE-WMP` (upstream MIT text) and `NOTICE`, kept
//! separate from the NJOY attribution. (The *algorithm* below is an independent
//! re-implementation; the reference is OpenMC's MIT-licensed `src/wmp.cpp`, at
//! `/home/teddy0/Documents/research/openmc/`.)
//!
//! # Role in OUTRAM PARK
//!
//! Per the workspace architecture (`docs/architecture.md`), **all nuclear-data
//! representation lives in this crate**; `outram-mc-libs` pulls cross sections from
//! here rather than owning any. WMP is the *low-fidelity, in-crate* data format:
//! compact enough to embed (KB–MB/nuclide) and — crucially — Doppler-broadenable
//! analytically, so it serves both OUTRAM PARK priorities:
//!
//! 1. **U-238 (n,γ) Doppler** — σ(E, T) is a closed-form evaluation of the pole
//!    sum via the Faddeeva function [`crate::wmp::faddeeva`]; no per-temperature pointwise
//!    library. Compared against njoy's own BROADR kernel and an OpenMC `.h5`.
//! 2. **Bare-sphere Keff** — the cross-section magnitudes; ν̄/χ come from
//!    [`crate::nuclear_data::secondary`].
//!
//! # Status
//!
//! The evaluator ([`crate::wmp::WindowedMultipole::evaluate`]) and the analytic Doppler
//! kernel ([`crate::wmp::faddeeva`]) are **implemented** and unit-tested; they are faithful
//! re-implementations of OpenMC `WindowedMultipole::evaluate` / `faddeeva`.
//! [`crate::wmp::WindowedMultipole::load_h5`] reads real `WMP_Library` HDF5 files
//! (pure-Rust `hdf5-pure`, always available) — see `tests/wmp_u238.rs` — and
//! since 2026-09-24 [`crate::wmp::WindowedMultipole::write_h5`] **writes** them (gh:#270),
//! verified both by an exact round trip and by **OpenMC itself reading the
//! result** and agreeing on sigma(E, T) to 1.2e-12 relative
//! (`tests/wmp_h5_write_round_trip.rs`, `tests/wmp_h5_vs_openmc.rs`). The embedded, zero-dependency
//! shipping path — [`crate::wmp::WindowedMultipole::to_blob`] (offline bake) and
//! [`crate::wmp::WindowedMultipole::from_blob`] (runtime decode) of the pure-Rust **WMPB v1**
//! format — is **implemented and round-trip tested**. The curated **CORE**
//! 125-nuclide set is baked into `src/data/wmp_core.wmpl` and always embedded,
//! exposed offline via [`crate::wmp::WmpLibrary::core`] (no feature gate — it ships in every
//! build); re-bake it with the `bake_wmp` example. See
//! `docs/wmp-nuclide-manifest.md`.
//!
//! # Layout
//!
//! Split by responsibility (crate file-size rule, `docs/porting-plan.md` §5):
//!
//! - `types.rs` — [`crate::wmp::Cf64`], [`crate::wmp::WmpXs`], [`crate::wmp::WmpReaction`], [`crate::wmp::WmpWindow`], and the
//!   [`WindowedMultipole`] data record.
//! - `evaluate.rs` — the Doppler-broadened evaluator
//!   ([`crate::wmp::WindowedMultipole::evaluate`]) and its numerics: [`crate::wmp::faddeeva`], the
//!   Weideman rational approximation, and the broadened curve-fit polynomial.
//! - `h5.rs` — [`crate::wmp::WindowedMultipole::load_h5`] (MIT `WMP_Library` HDF5 reader).
//! - `h5_write.rs` — [`crate::wmp::WindowedMultipole::write_h5`], its inverse, plus the
//!   [`crate::wmp::WMP_VERSION`] the format stamps. Kept beside the reader so the two
//!   cannot drift unnoticed.
//! - `blob.rs` — the pure-Rust **WMPB**/**WMPL** embedded formats
//!   ([`crate::wmp::WindowedMultipole::to_blob`] / [`crate::wmp::WindowedMultipole::from_blob`],
//!   [`crate::wmp::WmpLibrary`]).

mod blob;
mod evaluate;
mod h5;
mod h5_write;
#[cfg(test)]
mod tests;
mod types;

pub use blob::WmpLibrary;
pub use evaluate::faddeeva;
pub(crate) use evaluate::{weideman_coeffs, weideman_l};
pub use h5_write::WMP_VERSION;
pub use types::{Cf64, WindowedMultipole, WmpReaction, WmpWindow, WmpXs};

/// Boltzmann constant in \[eV/K\] (so `kT` in eV = `K_BOLTZMANN * T[K]`).
///
/// `pub(crate)` so the GPU port ([`crate::gpu_wmp`]) computes the identical
/// `sqrt_kt = sqrt(K_BOLTZMANN * T)` the CPU evaluator uses.
pub(crate) const K_BOLTZMANN: f64 = 8.617_333_262e-5;

/// √π — appears in every Doppler-broadened pole term.
///
/// `pub(crate)` so the GPU port matches the CPU pole-term scaling exactly.
pub(crate) const SQRT_PI: f64 = 1.772_453_850_905_516;

// Residue / curve-fit channel order (matches OpenMC `wmp.h`: RS, RA, RF).
const CH_SCATTER: usize = 0;
const CH_ABSORPTION: usize = 1;
const CH_FISSION: usize = 2;
