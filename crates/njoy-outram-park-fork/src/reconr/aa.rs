//! Adler-Adler (LRF=4) cross-section evaluation.
//!
//! Implements the zero-temperature Adler-Adler cross sections from `csaa`
//! in NJOY2016 `reconr.f90` (lines ~4317-4439). Like [`super::slbw`]/
//! [`super::rm`], only the `tempr <= 0` branch is ported — Doppler
//! broadening is BROADR's job in this crate's architecture, and
//! `ReconrConfig::temperature` is unused by SLBW/Reich-Moore reconstruction
//! for the same reason.
//!
//! ## Physical model
//!
//! Multilevel Adler-Adler is an empirical multi-reaction fit, not a true
//! R-matrix: each reaction (total, fission, capture) gets its own set of
//! resonance amplitude parameters `(DE, DW, GR, GI)` *and* its own
//! background polynomial, but they all share **one** hard-sphere phase
//! factor computed from the potential-scattering radius alone (not
//! per-L — see [`eval_aa_range`]'s doc comment for why this L-independence
//! is ported as-is, not "fixed"):
//!
//! ```text
//! k       = WAVE_K × (A/(A+1)) × sqrt(E)
//! C       = CADLER / (A/(A+1))^2
//! omega   = 2 × k × AP                     (shared hard-sphere phase)
//! bg(E)   = (c0 + c1/E + c2/E^2 + c3/E^3 + c4*E + c5*E^2) × C/sqrt(E)
//! x       = (DE - E) / DW
//! psi     = 1 / (1 + x^2)                   [zero-temperature line shape]
//! chi     = x * psi
//! sigma_total_res   += ((GR_t*cos(omega) + GI_t*sin(omega))*psi
//!                       + (GI_t*cos(omega) - GR_t*sin(omega))*chi) / DW_t
//! sigma_fission_res += (GR_f*psi + GI_f*chi) / DW_f
//! sigma_capture_res += (GR_c*psi + GI_c*chi) / DW_c
//! ```
//!
//! Final combination (see [`eval_aa_range`] for the doc-flagged
//! double-scaling of the background terms, ported literally from `csaa`):
//! `total = (sigma_total_res + bg_total) * C/sqrt(E) + 2*C*(1-cos(omega))/E`,
//! `fission = (sigma_fission_res + bg_fission) * C/sqrt(E)`,
//! `capture = (sigma_capture_res + bg_capture) * C/sqrt(E)`,
//! `elastic = total - fission - capture` (unless `LI == 6`, in which case
//! `total` is redefined as `fission + capture` and `elastic = 0` — a second
//! upstream oddity ported as-is, see [`eval_aa_range`]).

use crate::reconr::{mf2::AaRange, slbw::WAVE_K};

/// `CADLER`, from `reconr.f90`'s `csaa` — an empirical constant folding in
/// the `4*pi/k^2`-style normalization specific to the Adler-Adler
/// parameterization (units chosen so the final cross sections come out in
/// barns given `E` in eV and the ENDF `DE`/`DW`/`GR`/`GI` parameters as
/// tabulated).
const CADLER: f64 = 6.509_989_7e5;

/// Cross-section contributions from one Adler-Adler evaluation \[barns\].
#[derive(Debug, Clone, Copy, Default)]
pub struct AaSigmas {
    /// Total \[b\].
    pub total: f64,
    /// Elastic scattering \[b\] (`total - fission - capture`, or `0.0` when
    /// `LI == 6` — see this module's doc comment).
    pub elastic: f64,
    /// Fission \[b\].
    pub fission: f64,
    /// Radiative capture \[b\].
    pub capture: f64,
}

/// Evaluate Adler-Adler cross sections at energy `e` \[eV\] for one whole
/// resonance range.
///
/// Ported from `csaa` in NJOY2016 `reconr.f90`. Unlike
/// [`super::rm::eval_rm_lstate`]/[`super::slbw::eval_slbw_lstate`] (which
/// take one l-state and are summed by the caller), this takes the
/// **entire** [`AaRange`] and loops internally over every l-state's
/// resonances, because the background polynomials and the shared
/// hard-sphere phase factor are computed once per range, not per l-state —
/// mirroring `csaa`'s own structure rather than forcing it into the
/// per-l-state shape the other formalisms happen to have for unrelated
/// reasons (their phase/channel-radius genuinely is per-L; Adler-Adler's
/// isn't).
///
/// # Known-as-coded oddities (ported literally, not "fixed")
///
/// 1. **The hard-sphere phase (`omega = 2*k*AP`) is computed once, without
///    any L-dependence**, and reused for every l-state's resonances. This
///    matches `csaa` exactly — Adler-Adler is an empirical multi-reaction
///    fit rather than a true R-matrix, so L-dependence is presumably
///    already absorbed into the fitted `GR`/`GI`/background coefficients,
///    and the explicit phase factor is just a shared reference term for
///    the total cross section's interference contribution. Not re-derived
///    from `facphi`/`phase_shift`, since upstream doesn't call it either.
/// 2. **The background terms are scaled by `C/sqrt(E)` twice**: once
///    inside their own polynomial evaluation (`bakt=...*c/eroot` in
///    `csaa`), and again when combined with the resonance sum
///    (`sigp(1)=(sigp(1)+bakt)*c/eroot`). This looks unusual (the
///    resonance sum only gets the factor once) but is exactly what `csaa`
///    computes; ported as-is rather than guessed-and-corrected. Flagged
///    for Opus verification against a real Adler-Adler evaluation with a
///    non-negligible background (e.g. an older ENDF/B-V fit).
/// 3. **`LI == 6` redefines `total` as `fission + capture`** rather than
///    using the resonance+background+potential-scattering sum just
///    computed, and leaves `elastic` at `0.0` — also ported as-is.
pub fn eval_aa_range(e: f64, aa: &AaRange, ap: f64) -> AaSigmas {
    if e <= 0.0 {
        return AaSigmas::default();
    }

    let arat = aa.awri / (aa.awri + 1.0);
    let eroot = e.abs().sqrt();
    let esq = e * e;
    let ecub = esq * e;
    let k = WAVE_K * arat * eroot;
    let c = CADLER / (arat * arat);
    let omega = 2.0 * k * ap;
    let snf = omega.sin();
    let csf = omega.cos();

    let poly_bg = |coef: &[f64; 6]| -> f64 {
        let raw =
            coef[0] + coef[1] / e + coef[2] / esq + coef[3] / ecub + coef[4] * e + coef[5] * esq;
        raw * c / eroot
    };
    let bakt = if aa.li != 6 {
        poly_bg(&aa.background.total)
    } else {
        0.0
    };
    let bakf = if aa.li != 5 {
        poly_bg(&aa.background.fission)
    } else {
        0.0
    };
    let bakc = poly_bg(&aa.background.capture);

    let mut sig_total = 0.0_f64;
    let mut sig_fission = 0.0_f64;
    let mut sig_capture = 0.0_f64;

    for ls in &aa.l_states {
        for res in &ls.resonances {
            let x = (res.de_total - e) / res.dw_total;
            let psi = 1.0 / (1.0 + x * x);
            let chi = x * psi;
            sig_total += ((res.gr_total * csf + res.gi_total * snf) * psi
                + (res.gi_total * csf - res.gr_total * snf) * chi)
                / res.dw_total;

            let x = (res.de_fission - e) / res.dw_fission;
            let psi = 1.0 / (1.0 + x * x);
            let chi = x * psi;
            sig_fission += (res.gr_fission * psi + res.gi_fission * chi) / res.dw_fission;

            let x = (res.de_capture - e) / res.dw_capture;
            let psi = 1.0 / (1.0 + x * x);
            let chi = x * psi;
            sig_capture += (res.gr_capture * psi + res.gi_capture * chi) / res.dw_capture;
        }
    }

    let mut total = (sig_total + bakt) * c / eroot;
    total += 2.0 * c * (1.0 - csf) / e;
    let fission = (sig_fission + bakf) * c / eroot;
    let capture = (sig_capture + bakc) * c / eroot;

    let (total, elastic) = if aa.li != 6 {
        (total, total - fission - capture)
    } else {
        (fission + capture, 0.0)
    };

    AaSigmas {
        total,
        elastic,
        fission,
        capture,
    }
}
