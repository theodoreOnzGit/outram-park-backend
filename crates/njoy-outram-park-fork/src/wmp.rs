//! Windowed Multipole (WMP) cross-section import — **SCAFFOLD (not yet ported)**.
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
//!   of **C. Josey, P. Romano, B. Forget, K. Smith** (*J. Comput. Phys.* 2016,
//!   "Windowed multipole..."; *Ann. Nucl. Energy* 2015/2016). Credit MIT CRPG and
//!   these authors in any derived work; do **not** imply LANL endorsement.
//!
//! **Before importing any WMP code or data into this crate, add a separate
//! `LICENSE-WMP` (the upstream MIT text) and a NOTICE entry crediting MIT CRPG.**
//!
//! # What WMP is
//!
//! WMP represents a resonance cross section as a sum of complex **poles** and
//! **residues** in √E space, windowed into energy segments with a polynomial
//! background fit. Unlike the pointwise ACE/PENDF tables that RECONR + BROADR
//! produce, WMP allows **analytic on-the-fly Doppler broadening** at *any*
//! temperature by evaluating the Faddeeva function `w(z)` per pole — no
//! pre-broadened temperature grid, far smaller data footprint. OpenMC reads WMP
//! (HDF5) as an alternative to ACE for the resolved/unresolved resonance range.
//!
//! It therefore sits **parallel** to the ACE path, not inside it: a different
//! cross-section *representation*, consumed by the same downstream transport.
//!
//! # Status & scope (Phase 4 todo, after the thermal S(α,β) writer)
//!
//! **Stub.** Every entry point returns [`NjoyError::NotPorted`]. Planned work:
//!
//! 1. Parse a `WMP_Library` `*.h5` nuclide file (poles, residues, window bounds,
//!    broadening bounds, curve-fit, spacing) with **`hdf5-pure`** (already staged
//!    in the root `[workspace.dependencies]` with its `ndarray` feature — a
//!    pure-Rust HDF5 reader, no system `libhdf5`). Inherit it here when this lands
//!    via `hdf5-pure.workspace = true`.
//! 2. Define `WindowedMultipole` (poles/residues + windows + fit coefficients).
//! 3. Evaluate σ(E, T) analytically: per-pole Faddeeva `w(z)` broadening plus the
//!    windowed polynomial background — total/absorption/fission.
//! 4. Expose it through the same query surface as the ACE/`interface` path so a
//!    consumer can pick pointwise (ACE) or analytic (WMP) cross sections.
//!
//! Whether this lives in this crate long-term or moves to its own MIT-licensed
//! crate is open — keep it self-contained so it can be lifted out cleanly.

use crate::NjoyError;

/// Placeholder for a parsed Windowed Multipole nuclide (MIT CRPG `WMP_Library`).
///
/// **Not yet defined.** Will hold the complex poles and residues, the window
/// boundaries, the broadening bounds, and the polynomial curve-fit background
/// for one nuclide. See the [module docs](self) for provenance and the MIT
/// attribution that must accompany any real implementation.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct WindowedMultipole {
    // TODO(phase-4-wmp): poles: Vec<Complex<f64>>, residues, windows, fit, etc.
    // Populated from a WMP_Library *.h5 file once the HDF5 reader is added.
}

impl WindowedMultipole {
    /// Load a nuclide from a `WMP_Library` HDF5 file — **not yet ported**.
    ///
    /// # Errors
    /// Always returns [`NjoyError::NotPorted`]. See the [module docs](self).
    pub fn load_h5<P: AsRef<std::path::Path>>(_path: P) -> Result<Self, NjoyError> {
        Err(NjoyError::NotPorted(
            "Windowed Multipole import (MIT CRPG WMP_Library) — Phase 4, after thermal S(α,β)",
        ))
    }

    /// Evaluate the cross section at energy `e` \[eV\] and temperature `t` \[K\]
    /// by analytic multipole Doppler broadening — **not yet ported**.
    ///
    /// # Errors
    /// Always returns [`NjoyError::NotPorted`].
    pub fn evaluate(&self, _e_ev: f64, _t_kelvin: f64) -> Result<f64, NjoyError> {
        Err(NjoyError::NotPorted("Windowed Multipole σ(E,T) evaluation"))
    }
}
