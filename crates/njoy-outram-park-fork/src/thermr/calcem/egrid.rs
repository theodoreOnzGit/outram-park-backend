//! `calcem`'s fixed incident-energy grid and the `temp > break` rescaling.
//!
//! Faithful port of the `egrid` table and the small pieces of `calcem` that
//! consume it directly: NJOY2016 `thermr.f90:1575,1587-1607` (the `parameter`
//! declarations and the `egrid` data statement) and the two rescale formulas
//! at `thermr.f90:1971-1975` (`iform=0`, label 310) and `thermr.f90:2288`
//! (`iform=1`, label 520).

use crate::common::phys::BK_EV_PER_K;

/// `calcem`'s `ngrid` — the fixed number of incident-energy grid points
/// (`thermr.f90:1575`).
pub const NGRID: usize = 118;

/// `calcem`'s `break` \[K\]: above this physical temperature the fixed
/// `egrid` (built for room temperature) is rescaled upward so its upper edge
/// still reaches into the epithermal range (`thermr.f90:1614`).
pub const BREAK: f64 = 3000.0;

/// `calcem`'s `therm` \[eV\]: the reference thermal energy `kT₀ = 0.0253 eV`
/// used by the `iform=0` rescale formula (`thermr.f90:1613`). Same physical
/// constant as [`super::sigl::TEVZ`]/`inelastic::TEVZ`, kept as its own named
/// constant here to mirror the Fortran `parameter` declaration it is ported
/// from.
pub const THERM: f64 = 0.0253;

/// `calcem`'s fixed incident-energy grid `egrid(1:118)` \[eV\], ascending —
/// `thermr.f90:1587-1607`. Densely spaced near thermal energies (1e-5 eV to a
/// few tenths of an eV) and coarser above ~1 eV, up to 10 MeV... actually up
/// to 10 eV — see `EGRID[NGRID-1]` (the table does **not** reach fast-neutron
/// energies; THERMR only ever processes the thermal range). Transcribed
/// verbatim from the Fortran `real(kr),dimension(ngrid),parameter::egrid=(/…/)`
/// literal; do not "clean up" the values — several are intentionally
/// irregular (e.g. `.030613`, `.2510392`) to land on specific ENDF/B
/// evaluation grid points.
#[rustfmt::skip]
pub const EGRID: [f64; NGRID] = [
    1.0e-5, 1.78e-5, 2.5e-5, 3.5e-5, 5.0e-5, 7.0e-5, 1.0e-4,
    1.26e-4, 1.6e-4, 2.0e-4, 0.000253, 0.000297, 0.000350,
    0.00042, 0.000506, 0.000615, 0.00075, 0.00087,
    0.001012, 0.00123, 0.0015, 0.0018, 0.00203, 0.002277,
    0.0026, 0.003, 0.0035, 0.004048, 0.0045, 0.005,
    0.0056, 0.006325, 0.0072, 0.0081, 0.009108, 0.01,
    0.01063, 0.0115, 0.012397, 0.0133, 0.01417, 0.015,
    0.016192, 0.0182, 0.0199, 0.020493, 0.0215, 0.0228,
    0.0253, 0.028, 0.030613, 0.0338, 0.0365, 0.0395,
    0.042757, 0.0465, 0.050, 0.056925, 0.0625, 0.069,
    0.075, 0.081972, 0.09, 0.096, 0.1035, 0.111573,
    0.120, 0.128, 0.1355, 0.145728, 0.160, 0.172,
    0.184437, 0.20, 0.2277, 0.2510392, 0.2705304,
    0.2907501, 0.3011332, 0.3206421, 0.3576813, 0.39,
    0.4170351, 0.45, 0.5032575, 0.56, 0.625,
    0.70, 0.78, 0.86, 0.95, 1.05, 1.16, 1.28,
    1.42, 1.55, 1.70, 1.855, 2.02, 2.18,
    2.36, 2.59, 2.855, 3.12, 3.42, 3.75,
    4.07, 4.46, 4.90, 5.35, 5.85, 6.40,
    7.00, 7.65, 8.40, 9.15, 9.85, 10.00,
];

/// `calcem`'s `nne` computation (`thermr.f90:1639-1643`): the number of
/// leading [`EGRID`] points at or below `emax` \[eV\], plus the first point
/// past it (matching NJOY's `>` — the first grid point strictly greater than
/// `emax` is index `nne`, one-based; if none exceeds `emax` every point is
/// used).
pub fn egrid_count(emax: f64) -> usize {
    for (i, &e) in EGRID.iter().enumerate() {
        if e > emax {
            return i + 1;
        }
    }
    NGRID
}

/// `iform=0`'s incident-energy rescale for `temp > break` — `thermr.f90`
/// label 310, lines 1971-1975:
///
/// ```text
/// tone=therm/bk
/// elo=egrid(1)
/// enow=elo*exp(log(enow/elo)*log((temp/tone)*egrid(ngrid)/elo)/log(egrid(ngrid)/elo))
/// ```
///
/// Stretches the whole (room-temperature-tuned) grid so its top still spans
/// up to `(T/T₀)·E_max` at the requested physical temperature `temp_k` \[K\].
/// A no-op at or below [`BREAK`].
pub fn rescale_iform0(enow: f64, temp_k: f64) -> f64 {
    if temp_k <= BREAK {
        return enow;
    }
    let tone = THERM / BK_EV_PER_K;
    let elo = EGRID[0];
    let last = EGRID[NGRID - 1];
    let ratio_num = (temp_k / tone) * last / elo;
    elo * ((enow / elo).ln() * ratio_num.ln() / (last / elo).ln()).exp()
}

/// `iform=1`'s incident-energy rescale for `temp > break` — `thermr.f90`
/// label 520, line 2288: `if (ie.gt.1.and.temp.gt.break) enow=enow*temp/break`.
/// Simpler than [`rescale_iform0`] — a plain linear stretch — and, notably,
/// **never applied to the first grid point** (`ie == 1`, `EGRID[0]`), which
/// stays at its literal `1.e-5 eV` regardless of temperature.
///
/// `ie_one_based` is the 1-based index into [`EGRID`] (matching the Fortran
/// `ie` loop variable) so the `ie > 1` guard translates directly.
pub fn rescale_iform1(enow: f64, temp_k: f64, ie_one_based: usize) -> f64 {
    if ie_one_based > 1 && temp_k > BREAK {
        enow * temp_k / BREAK
    } else {
        enow
    }
}
