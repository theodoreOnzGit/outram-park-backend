// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/cbl.f90 (subroutine cbl, function cuberoot)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (cbl.f90: Luca Mortarini, Massimo Cassiani)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Skewed convective-boundary-layer turbulence (Cassiani et al. 2015,
//! *Boundary-Layer Meteorol.* 154, 367–390): the drift and diffusion terms of
//! the Langevin equation for vertical velocity when `cblflag` is set.
//!
//! The vertical-velocity PDF is a bi-Gaussian (Luhar et al. 1996 closure, with
//! `costluar4 = 0.66667`) whose third moment follows the Lenschow profile
//! `w'^3 = 1.2 zeta (1 - zeta)^{3/2} w*^3`, tapered to Gaussian by a sine
//! `transition` as `-h/L` falls below 15. The routine returns the PDF value,
//! the flux terms and the drift/diffusion pair `(a, b)` that `advance.f90`
//! uses to update `w`.
//!
//! # Translation notes
//!
//! * The arithmetic is transcribed **operation for operation**, in Fortran's
//!   left-to-right order, so the port agrees with a `real(8)` build of upstream
//!   to the last bit or two. Fortran integer powers (`x**2`, `x**3`) are
//!   products; real powers (`x**1.5`, `x**0.5`, `x**(-2.)`) are `powf`, as
//!   gfortran emits them.
//! * `erf` is the gfortran **intrinsic** in upstream (the `real :: erf`
//!   declaration has no `external`, so the intrinsic wins over `erf.f90`).
//!   The port uses [`petir::specfunc::erf`], per the crate's reuse rule; the
//!   two are compared in the code-to-code test.
//! * `cuberoot` is upstream's own `sign(|x|**0.333333333, x)`, kept with its
//!   truncated exponent rather than replaced by `cbrt`.
//! * `ldirect` comes from `com_mod` upstream; here it is the `time_direction`
//!   argument.
//!
//! # Units
//!
//! Bare `f64` in FLEXPART's units: velocities m/s, heights and `L` m, times s,
//! density kg/m³, density gradient kg/m⁴.

use super::constants::PI;

/// Upstream's `C0`, the Lagrangian structure-function constant.
const C0: f64 = 3.0;
/// `1/sqrt(2)`, as upstream writes it (its 10-digit literal, not std's).
#[allow(clippy::approx_constant)]
const USURAD2: f64 = 0.707_106_781_2;
/// `1/sqrt(2 pi)`, as upstream writes it.
const USURAD2P: f64 = 0.398_942_280_4;
/// Luhar et al. closure constant.
const COSTLUAR4: f64 = 0.66667;
/// Offset keeping the third moment off zero at the surface.
const EPS: f64 = 0.000_001;

/// Upstream's `cuberoot`: `sign(|x|^0.333333333, x)`.
fn cuberoot(x: f64) -> f64 {
    #[allow(clippy::excessive_precision)]
    const THIRD: f64 = 0.333_333_333;
    x.abs().powf(THIRD).copysign(x)
}

/// What [`cbl`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CblTerms {
    /// Bi-Gaussian PDF of `w` at the particle's velocity, times density.
    pub ptot: f64,
    /// Upstream's `Q`.
    pub q: f64,
    /// Upstream's `Phi`, the flux term.
    pub phi: f64,
    /// Drift coefficient `a` of the Langevin equation.
    pub ath: f64,
    /// Diffusion coefficient `b` of the Langevin equation.
    pub bth: f64,
    /// True when the velocity sits more than six standard deviations from
    /// both Gaussian modes; upstream sets `flagrein = 1` and `advance.f90`
    /// re-initialises the particle's velocity.
    pub reinitialise: bool,
}

/// `cbl.f90`: drift and diffusion of `w` in the skewed convective boundary
/// layer.
///
/// # Arguments
/// - `wp` — particle vertical velocity, m/s.
/// - `zp` — particle height, m.
/// - `ust` — friction velocity, m/s (unused upstream, kept for the signature).
/// - `wst` — convective velocity scale `w*`, m/s.
/// - `h` — mixing height, m.
/// - `rhoa`, `rhograd` — air density and its vertical gradient.
/// - `sigmaw`, `dsigmawdz` — `sigma_w` and its gradient from [`super::turbulence`].
/// - `tlw` — Lagrangian time scale of `w`, s.
/// - `ol` — Obukhov length, m.
/// - `time_direction` — `+1` forward, `-1` backward (`ldirect`).
#[allow(clippy::too_many_arguments)]
pub fn cbl(
    wp: f64,
    zp: f64,
    _ust: f64,
    wst: f64,
    h: f64,
    rhoa: f64,
    rhograd: f64,
    sigmaw: f64,
    dsigmawdz: f64,
    tlw: f64,
    ol: f64,
    time_direction: f64,
) -> CblTerms {
    let dens = rhoa;
    let ddens = rhograd;
    let timedir = time_direction;

    let z = zp / h;
    let mut transition = 1.0;
    if -h / ol < 15.0 {
        transition = ((-h / ol + 10.0) / 10.0 * PI).sin() / 2.0 + 0.5;
    }

    let w2 = sigmaw * sigmaw;
    let dw2 = 2.0 * sigmaw * dsigmawdz;
    let alfa = 2.0 * w2 / (C0 * tlw);
    let wold = timedir * wp;

    let wst3 = wst * wst * wst;
    let w3 = (1.2 * z * (1.0 - z).powf(1.5) + EPS) * wst3 * transition;
    let dw3 = 1.2
        * ((1.0 - z).powf(1.5) + -(z * 1.5 * (1.0 - z).powf(0.5)))
        * wst3
        * (1.0 / h)
        * transition;

    let skew = w3 / w2.powf(1.5);
    let skew2 = skew * skew;
    let dskew = (dw3 * w2.powf(1.5) - w3 * 1.5 * w2.powf(0.5) * dw2) / (w2 * w2 * w2);
    let radw2 = w2.powf(0.5);
    let dradw2 = 0.5 * w2.powf(-0.5) * dw2;
    let fluarw = COSTLUAR4 * cuberoot(skew);
    let fluarw2 = fluarw * fluarw;

    let (dfluarw, rluarw, drluarw, xluarw, dxluarw);
    if skew != 0.0 {
        dfluarw = COSTLUAR4 * (1.0 / 3.0) * cuberoot(skew.powf(-2.0)) * dskew;
        let p1 = 1.0 + fluarw2;
        let p3 = 3.0 + fluarw2;
        let p1_2 = p1 * p1;
        let p1_3 = p1 * p1 * p1;
        let p3_2 = p3 * p3;
        rluarw = p1.powf(3.0) * skew2 / (p3_2 * fluarw2);
        xluarw = p1.powf(1.5) * skew / (p3 * fluarw);
        let d2f = 2.0 * fluarw * dfluarw;
        let den = p3_2 * fluarw2;
        drluarw = ((3.0 * p1_2 * d2f * skew2 + p1_3 * 2.0 * skew * dskew) * p3_2 * fluarw2
            - p1_3 * skew2 * (2.0 * p3 * d2f * fluarw2 + p3_2 * 2.0 * fluarw * dfluarw))
            / (den * den);
        let den = p3 * fluarw;
        dxluarw = ((1.5 * p1.powf(0.5) * d2f * skew + p1.powf(1.5) * dskew) * p3 * fluarw
            - p1.powf(1.5) * skew * (3.0 * dfluarw + 3.0 * fluarw2 * dfluarw))
            / (den * den);
    } else {
        dfluarw = 0.0;
        rluarw = 0.0;
        drluarw = 0.0;
        xluarw = 0.0;
        dxluarw = 0.0;
    }

    let aluarw = 0.5 * (1.0 - xluarw / (4.0 + rluarw).powf(0.5));
    let bluarw = 1.0 - aluarw;
    let daluarw = -(0.5
        * (dxluarw * (4.0 + rluarw).powf(0.5)
            - 0.5 * xluarw * (4.0 + rluarw).powf(-0.5) * drluarw)
        / (4.0 + rluarw));
    let dbluarw = -daluarw;

    let p1 = 1.0 + fluarw2;
    let ra = bluarw / (aluarw * p1);
    let rb = aluarw / (bluarw * p1);
    let sigmawa = radw2 * ra.powf(0.5);
    let sigmawb = radw2 * rb.powf(0.5);
    let a_p1 = aluarw * p1;
    let b_p1 = bluarw * p1;
    let dsigmawa = dradw2 * ra.powf(0.5)
        + radw2
            * (0.5
                * ra.powf(-0.5)
                * ((dbluarw * a_p1 - bluarw * (daluarw * p1 + aluarw * 2.0 * fluarw * dfluarw))
                    / (a_p1 * a_p1)));
    let dsigmawb = dradw2 * rb.powf(0.5)
        + radw2
            * (0.5
                * rb.powf(-0.5)
                * ((daluarw * b_p1 - aluarw * (dbluarw * p1 + bluarw * 2.0 * fluarw * dfluarw))
                    / (b_p1 * b_p1)));

    let wa = fluarw * sigmawa;
    let wb = fluarw * sigmawb;
    let dwa = dfluarw * sigmawa + fluarw * dsigmawa;
    let dwb = dfluarw * sigmawb + fluarw * dsigmawb;
    let deltawa = wold - wa;
    let deltawb = wold + wb;
    let wold2 = wold * wold;
    let sigmawa2 = sigmawa * sigmawa;
    let sigmawb2 = sigmawb * sigmawb;

    let reinitialise = deltawa.abs() > 6.0 * sigmawa && deltawb.abs() > 6.0 * sigmawb;

    let ga = deltawa / sigmawa;
    let gb = deltawb / sigmawb;
    let pa = USURAD2P * (1.0 / sigmawa) * (-(0.5 * (ga * ga))).exp();
    let pb = USURAD2P * (1.0 / sigmawb) * (-(0.5 * (gb * gb))).exp();
    let ptot = dens * aluarw * pa + dens * bluarw * pb;

    let aperfa = deltawa * USURAD2 / sigmawa;
    let aperfb = deltawb * USURAD2 / sigmawb;
    let erf = petir::specfunc::erf;

    let phi =
        -(0.5 * (aluarw * dens * dwa + dens * wa * daluarw + aluarw * wa * ddens) * erf(aperfa))
            + sigmawa
                * (aluarw * dens * dsigmawa * (wold2 / sigmawa2 + 1.0)
                    + sigmawa * dens * daluarw
                    + sigmawa * ddens * aluarw
                    + aluarw * wold * dens / sigmawa2 * (sigmawa * dwa - wa * dsigmawa))
                * pa
            + 0.5 * (bluarw * dens * dwb + wb * dens * dbluarw + wb * bluarw * ddens) * erf(aperfb)
            + sigmawb
                * (bluarw * dens * dsigmawb * (wold2 / sigmawb2 + 1.0)
                    + sigmawb * dens * dbluarw
                    + sigmawb * ddens * bluarw
                    + bluarw * wold * dens / sigmawb2 * (-(sigmawb * dwb) + wb * dsigmawb))
                * pb;

    let q = timedir
        * (aluarw * dens * deltawa / sigmawa2 * pa + bluarw * dens * deltawb / sigmawb2 * pb);

    let ath = (1.0 / ptot) * (-(C0 / 2.0 * alfa * q) + phi);
    let bth = (C0 * alfa).sqrt();

    CblTerms {
        ptot,
        q,
        phi,
        ath,
        bth,
        reinitialise,
    }
}
