// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `LEAPR` — generate the thermal scattering law S(alpha, beta) (ENDF MF=7).
//!
//! LEAPR is the *generator* upstream of THERMR: it builds S(alpha, beta) for a
//! bound moderator from a phonon/vibrational model (incoherent-Gaussian
//! approximation), where THERMR only *reads* an existing MF=7 evaluation. The
//! model combines a solid-type **phonon expansion** from a frequency spectrum
//! rho(E), an optional **translational** (free-gas or diffusion) term, and
//! optional **discrete oscillators**.
//!
//! ## Module map (what belongs where)
//!
//! - [`input`] — the typed card deck ([`LeaprInput`] and its enums).
//! - [`frequency`] — `start`/`fsum`: phonon-spectrum integrals, Debye-Waller
//!   lambda, effective temperature, and the first phonon term `T_1(beta)`.
//! - [`continuous`] — `contin`/`terpt`/`convol`: the phonon-expansion sum
//!   `S = e^{-alpha lambda} sum_n (alpha lambda)^n/n! T_n(beta)`, plus the
//!   short-collision-time tail fill and moment checks.
//! - [`translation`] — `trans`/`stable`/`terps`/`sbfill`/`besk1`: the
//!   translational (free-gas / diffusion) term.
//! - [`discrete`] — `discre`/`bfact`/`bfill`/`exts`/`sint` and the modified
//!   Bessel `I0`/`I1`: discrete-oscillator convolution.
//! - [`coldh`] — the tractable Young-Koppel cold-hydrogen/deuterium helpers
//!   (`bt`, `sumh`, `cn`, `sjbes`, `terpk`). The full `coldh` orchestrator is
//!   **not** ported.
//! - [`sct`] — the free-gas / short-collision-time Gaussian used by several
//!   kernels.
//!
//! ## Ported vs. not ported
//!
//! The physics kernels above are ported and unit-tested. **Not ported** (return
//! [`crate::NjoyError::NotPorted`] or are simply absent):
//! - `endout` + `copys` (leapr.f90:2972-3623, 2468-2487): the MF=7 tape writer
//!   and scratch plumbing. [`run`] therefore returns `NotPorted`.
//! - `coher`/`formf`/`tausq`/`taufcc`/`taubcc` (2489-2814, 2924-2970): the
//!   coherent-elastic (Bragg) calculation — the consuming side already lives in
//!   [`crate::thermr::coherent`].
//! - `skold` (2816-2922): the Sköld pair-correlation correction.
//! - the full `coldh` convolution orchestrator (2005-2183); its helpers are
//!   ported (see [`coldh`]).
//!
//! **Untrusted AI draft.** These kernels are unit-tested against closed-form and
//! self-consistency checks but have **not** been validated end-to-end against a
//! reference LEAPR MF=7 tape. See `README.md`.
//!
//! **Upstream:** `leapr.f90` (~3.6k lines). **Manual:** LA-UR-17-20093 §LEAPR.

use crate::NjoyError;

pub mod coldh;
pub mod continuous;
pub mod discrete;
pub mod frequency;
pub mod input;
pub mod sct;
pub mod translation;

pub use frequency::FrequencyModel;
pub use input::{
    ColdOption, ContinuousDist, DiscreteOscillator, ElasticOption, LeaprInput, TranslationKind,
};

/// The thermal energy 0.0253 eV used as the `lat` scaling reference (eV).
pub const THERM_EV: f64 = 0.0253;

/// Single-temperature asymmetric scattering-law table `S_s(alpha, -beta)`.
///
/// Stores the LEAPR working array with **beta as the fastest index** (matching
/// NJOY's `ssm(nbeta, nalpha)` layout). Entries are the dimensionless asymmetric
/// scattering law evaluated on the negative-beta side; the physical S at positive
/// beta is recovered with the usual `exp(-beta)` / `exp(-beta/2)` detailed-balance
/// factors.
#[derive(Debug, Clone, PartialEq)]
pub struct SabMatrix {
    /// Number of alpha grid points.
    pub nalpha: usize,
    /// Number of beta grid points.
    pub nbeta: usize,
    data: Vec<f64>,
}

impl SabMatrix {
    /// Allocate a zeroed `nbeta x nalpha` table.
    pub fn zeros(nbeta: usize, nalpha: usize) -> Self {
        Self { nalpha, nbeta, data: vec![0.0; nbeta * nalpha] }
    }

    /// Read `S(alpha_ialpha, -beta_ibeta)` (dimensionless).
    #[inline]
    pub fn get(&self, ibeta: usize, ialpha: usize) -> f64 {
        self.data[ialpha * self.nbeta + ibeta]
    }

    /// Write `S(alpha_ialpha, -beta_ibeta)` (dimensionless).
    #[inline]
    pub fn set(&mut self, ibeta: usize, ialpha: usize, v: f64) {
        self.data[ialpha * self.nbeta + ibeta] = v;
    }
}

/// Run the LEAPR card-input driver (NJOY module entry point).
///
/// **Status:** the physics kernels are ported and exposed through the module API
/// (see the module map above), but the NJOY card-input driver plus the `endout`
/// MF=7 tape writer are **not** ported, so this returns
/// [`crate::NjoyError::NotPorted`]. Build a job with [`LeaprInput`] and call the
/// module functions directly.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "leapr driver + MF=7 endout writer (physics kernels ported — use the module API)",
    ))
}
