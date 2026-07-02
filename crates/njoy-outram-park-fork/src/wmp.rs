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
//! **Before importing any WMP code or data into this crate, add a separate
//! `LICENSE-WMP` (the upstream MIT text) and a NOTICE entry crediting MIT CRPG.**
//!
//! # Role in OUTRAM PARK
//!
//! Per the workspace architecture (`docs/architecture.md`), **all nuclear-data
//! representation lives in this crate**; `openmc-libs` pulls cross sections from
//! here rather than owning any. WMP is the *low-fidelity, in-crate* data format:
//! compact enough to embed (KB–MB/nuclide) and — crucially — Doppler-broadenable
//! analytically, so it serves both OUTRAM PARK priorities:
//!
//! 1. **U-238 (n,γ) Doppler** — σ(E, T) is a closed-form evaluation of the pole
//!    sum via the Faddeeva function [`faddeeva`]; no per-temperature pointwise
//!    library. Compared against njoy's own BROADR kernel and an OpenMC `.h5`.
//! 2. **Bare-sphere Keff** — the cross-section magnitudes; ν̄/χ come from
//!    [`crate::nuclear_data::secondary`].
//!
//! # Status
//!
//! Scaffold. The window/curve-fit bookkeeping is stubbed; the one physics kernel
//! still to port is [`faddeeva`]. Faithful target: OpenMC
//! `WindowedMultipole::evaluate` (`/home/teddy0/Documents/research/openmc/`).

use crate::NjoyError;

/// Minimal complex number for the multipole sum. (The WMP inner loop needs only
/// `+`, `*`, and the Faddeeva evaluation; a full complex dependency is overkill.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cf64 {
    pub re: f64,
    pub im: f64,
}

impl Cf64 {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    /// Complex multiplication.
    pub fn mul(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    /// Complex addition.
    pub fn add(self, o: Cf64) -> Cf64 {
        Cf64::new(self.re + o.re, self.im + o.im)
    }
}

/// The three base cross sections a windowed-multipole evaluation yields, in barn.
///
/// Elastic scattering is `total − absorption`; capture is `absorption − fission`.
/// ν̄ is intentionally absent — WMP carries no secondary data. Combine with
/// [`crate::nuclear_data::secondary::NuBar`] to form the full
/// [`crate::nuclear_data::MicroXs`] the transport kernel consumes.
#[derive(Debug, Clone, Copy, Default)]
pub struct WmpXs {
    /// Total microscopic cross section σ_t(E, T) \[barn\].
    pub total: f64,
    /// Absorption (capture + fission) σ_a(E, T) \[barn\].
    pub absorption: f64,
    /// Fission σ_f(E, T) \[barn\] (0 for non-fissile nuclides).
    pub fission: f64,
}

impl WmpXs {
    /// Elastic scattering σ_s = σ_t − σ_a \[barn\].
    pub fn scatter(&self) -> f64 {
        (self.total - self.absorption).max(0.0)
    }
    /// Radiative capture σ_γ = σ_a − σ_f \[barn\] — the U-238 Doppler target.
    pub fn capture(&self) -> f64 {
        (self.absorption - self.fission).max(0.0)
    }
}

/// Base reaction channels carried by the WMP residues, in library order.
#[derive(Debug, Clone, Copy)]
pub enum WmpReaction {
    Total = 0,
    Absorption = 1,
    Fission = 2,
}

/// Windowed-multipole representation of one nuclide's resonance-range cross
/// sections. Field layout mirrors the MIT `WMP_Library` HDF5.
///
/// The evaluation splits the `√E` axis into equal-spacing **windows**; within a
/// window the cross section is a smooth **curve-fit** polynomial background plus
/// the Faddeeva contribution of the poles assigned to that window. Temperature
/// enters only through the Faddeeva argument, giving analytic Doppler broadening.
#[derive(Debug, Clone, Default)]
pub struct WindowedMultipole {
    /// Nuclide name, e.g. `"U238"`.
    pub name: String,
    /// Atomic weight ratio (target mass / neutron mass) — sets the Doppler width.
    pub awr: f64,
    /// Lower / upper energy bounds of the multipole representation \[eV\].
    pub e_min: f64,
    pub e_max: f64,
    /// Poles (complex, in `√E` space), one per resonance-like term.
    pub poles: Vec<Cf64>,
    /// Residues per pole per base reaction (order = [`WmpReaction`]).
    pub residues: Vec<[Cf64; 3]>,
    /// Curve-fit coefficients per window per order per reaction.
    pub curvefit: Vec<Vec<[f64; 3]>>,
    /// `[start_pole, end_pole]` index range for each window.
    pub windows: Vec<(usize, usize)>,
    /// Inverse window spacing in `√E`.
    pub inv_spacing: f64,
    /// Curve-fit polynomial order.
    pub fit_order: usize,
}

impl WindowedMultipole {
    /// Evaluate σ_t / σ_a / σ_f at incident energy `e` \[eV\] and temperature
    /// `temp_k` \[K\] via the windowed-multipole sum with analytic Doppler
    /// broadening. Ported from OpenMC `WindowedMultipole::evaluate`.
    ///
    /// Outside `[e_min, e_max]` the multipole form is invalid; the caller must
    /// fall back to a pointwise high-energy tail (see `docs/architecture.md`).
    pub fn evaluate(&self, _e_ev: f64, _temp_k: f64) -> WmpXs {
        // Scaffold. The port:
        //   1. sqrt_e = sqrt(E); locate window w = ((sqrt_e - sqrt(e_min)) * inv_spacing).
        //   2. Start from the curve-fit background: Σ_k curvefit[w][k][rxn] * sqrt_e^k.
        //   3. For each pole p in windows[w]: Doppler argument
        //        z = (sqrt_e - pole[p]) * sqrt(awr / (K_BOLTZMANN * temp_k)),
        //      add Re{ residue[p][rxn] * faddeeva(z) } / sqrt_e to each channel.
        //   4. Assemble WmpXs { total, absorption, fission }.
        todo!("WindowedMultipole::evaluate: window walk + Doppler pole sum (nuclide.cpp)")
    }

    /// Load a nuclide from a `WMP_Library` HDF5 file — **not yet ported**.
    ///
    /// Uses `hdf5-pure` (pure-Rust, no system `libhdf5`; staged in the root
    /// `[workspace.dependencies]`). Enable via `hdf5-pure.workspace = true` when
    /// this lands. For the *embedded* offline path, prefer [`Self::from_blob`].
    ///
    /// # Errors
    /// Always returns [`NjoyError::NotPorted`].
    pub fn load_h5<P: AsRef<std::path::Path>>(_path: P) -> Result<Self, NjoyError> {
        Err(NjoyError::NotPorted(
            "WMP HDF5 import (MIT CRPG WMP_Library) — add hdf5-pure + LICENSE-WMP first",
        ))
    }

    /// Decode a nuclide from a compact embedded blob (the in-crate delivery path).
    ///
    /// A maintainer bakes the MIT `WMP_Library` HDF5 into a zstd blob offline;
    /// this crate `include_bytes!`s it and decodes here — no HDF5 dependency in
    /// the shipped build. See `docs/architecture.md` for the blob schema.
    ///
    /// # Errors
    /// Always returns [`NjoyError::NotPorted`].
    pub fn from_blob(_bytes: &[u8]) -> Result<Self, NjoyError> {
        Err(NjoyError::NotPorted("WMP embedded-blob decode"))
    }
}

/// Faddeeva function `w(z) = e^{−z²} · erfc(−i z)` — the analytic Doppler kernel.
///
/// This single evaluation is where temperature broadening enters the multipole
/// sum, and (per the workspace split) the Faddeeva kernel lives in *this* crate.
/// Port a standard pure-Rust algorithm (Zaghloul & Ali, TOMS 916, or the
/// rational approximation OpenMC uses in `include/openmc/faddeeva.h`). **No FFI**
/// (workspace rule: no `extern "C"` to a system `w(z)`).
pub fn faddeeva(_z: Cf64) -> Cf64 {
    todo!("faddeeva: pure-Rust w(z) (TOMS 916 or the OpenMC rational approximation)")
}
