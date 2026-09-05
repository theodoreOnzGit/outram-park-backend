// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Discrete-oscillator convolution (`discre` + `bfact`/`bfill`/`exts`/`sint`).
//!
//! Ports `discre` (leapr.f90:1320-1661), `bfact` (1663-1796), `bfill`
//! (1798-1832), `exts` (1834-1865) and `sint` (1867-1934), plus the modified
//! Bessel functions `I0`/`I1` used by `bfact`. Up to a few dozen discrete
//! molecular-vibration oscillators are convolved with the continuous
//! `S(alpha, beta)`.
//!
//! ## Physics
//! Each oscillator contributes a ladder of delta functions weighted by modified
//! Bessel functions `I_n(alpha * ar)` with a Debye-Waller damping `exp(-alpha*dbw)`
//! (`bfact`). The ladders from all oscillators are convolved together, then folded
//! against the continuous law (interpolated / SCT-extrapolated by `sint`).
//!
//! ## Units
//! `alpha`, `beta`, `S` dimensionless; oscillator energies in eV; temperatures K.

use crate::leapr::input::LeaprInput;
use crate::leapr::SabMatrix;

const SMALL: f64 = 1e-8;
const VSMALL: f64 = 1e-10;
const TINY: f64 = 1e-20;
const SLIM: f64 = -225.0;
const THERM: f64 = 0.0253;
/// Cap on the number of accumulated discrete lines (`maxdd` in `discre`).
const MAXDD: usize = 500;

/// Modified Bessel function `I0(x)` (dimensionless), via the Abramowitz-Stegun
/// polynomial approximations used by NJOY `bfact` (leapr.f90:1713-1722). Returns
/// the *true* `I0(x)` (not the exponentially scaled form used internally by
/// `bfact`).
pub fn bessel_i0(x: f64) -> f64 {
    let y = x / 3.75;
    if y.abs() <= 1.0 {
        let u = y * y;
        1.0 + u
            * (3.5156229
                + u * (3.0899424
                    + u * (1.2067492 + u * (0.2659732 + u * (0.0360768 + u * 0.0045813)))))
    } else {
        let v = 1.0 / y;
        let poly = 0.39894228
            + v * (0.01328592
                + v * (0.00225319
                    + v * (-0.00157565
                        + v * (0.00916281
                            + v * (-0.02057706
                                + v * (0.02635537 + v * (-0.01647633 + v * 0.00392377)))))));
        poly / x.sqrt() * x.exp()
    }
}

/// Modified Bessel function `I1(x)` (dimensionless), via the Abramowitz-Stegun
/// polynomial approximations used by NJOY `bfact` (leapr.f90:1724-1733). Returns
/// the *true* `I1(x)`.
pub fn bessel_i1(x: f64) -> f64 {
    let y = x / 3.75;
    if y.abs() <= 1.0 {
        let u = y * y;
        (0.5 + u
            * (0.87890594
                + u * (0.51498869
                    + u * (0.15084934 + u * (0.02658733 + u * (0.00301532 + u * 0.00032411))))))
            * x
    } else {
        let v = 1.0 / y;
        let mut b = 0.02282967 + v * (-0.02895312 + v * (0.01787654 - v * 0.00420059));
        b = 0.39894228
            + v * (-0.03988024 + v * (-0.00362018 + v * (0.00163801 + v * (-0.01031555 + v * b))));
        b / x.sqrt() * x.exp()
    }
}

/// Bessel-function terms for one discrete oscillator (`bfact`,
/// leapr.f90:1663-1796).
///
/// Returns `(bzero, bplus, bminus)`: the zero-phonon weight and the ladders of
/// up-scatter (`bplus[k-1]` = order `k`) and down-scatter (`bminus[k-1]`) weights,
/// each with the Debye-Waller and detailed-balance exponentials applied.
///
/// # Arguments
/// * `x` — `alpha * ar` (dimensionless).
/// * `dwc` — `alpha * dbw`, the oscillator Debye-Waller argument (dimensionless).
/// * `betai` — oscillator energy in beta units `bdel/tev` (dimensionless).
pub fn bfact(x: f64, dwc: f64, betai: f64) -> (f64, [f64; 50], [f64; 50]) {
    const BIG: f64 = 1e10;
    const TINY_B: f64 = 1e-30;
    let y = x / 3.75;

    // scaled I0, I1 exactly as bfact (I0 for y<=1, e^{-x} I0 for y>1; similarly I1)
    let (bessi0, bessi1) = if y <= 1.0 {
        (bessel_i0(x), bessel_i1(x))
    } else {
        // e^{-x} I0(x), e^{-x} I1(x)
        (bessel_i0(x) * (-x).exp(), bessel_i1(x) * (-x).exp())
    };

    // higher orders by reverse recursion; bn[k] holds order k (1..=50)
    let imax = 50usize;
    let mut bn = vec![0.0_f64; imax + 1];
    bn[imax] = 0.0;
    bn[imax - 1] = 1.0;
    let mut i = imax - 1;
    while i > 1 {
        bn[i - 1] = bn[i + 1] + (i as f64) * (2.0 / x) * bn[i];
        i -= 1;
        if bn[i] >= BIG {
            for item in bn.iter_mut().skip(i) {
                *item /= BIG;
            }
        }
    }
    let rat = bessi1 / bn[1];
    for k in 1..=imax {
        bn[k] *= rat;
        if bn[k] < TINY_B {
            bn[k] = 0.0;
        }
    }

    let mut bplus = [0.0_f64; 50];
    let mut bminus = [0.0_f64; 50];
    let bzero;
    if y <= 1.0 {
        bzero = bessi0 * (-dwc).exp();
        for k in 1..=imax {
            if bn[k] != 0.0 {
                let p = (-dwc - (k as f64) * betai / 2.0).exp() * bn[k];
                bplus[k - 1] = if p < TINY_B { 0.0 } else { p };
                let m = (-dwc + (k as f64) * betai / 2.0).exp() * bn[k];
                bminus[k - 1] = if m < TINY_B { 0.0 } else { m };
            }
        }
    } else {
        bzero = bessi0 * (-dwc + x).exp();
        for k in 1..=imax {
            if bn[k] != 0.0 {
                let p = (-dwc - (k as f64) * betai / 2.0 + x).exp() * bn[k];
                bplus[k - 1] = if p < TINY_B { 0.0 } else { p };
                let m = (-dwc + (k as f64) * betai / 2.0 + x).exp() * bn[k];
                bminus[k - 1] = if m < TINY_B { 0.0 } else { m };
            }
        }
    }
    (bzero, bplus, bminus)
}

/// Extended, symmetric beta grid + reciprocal spacings for `sint` (`bfill`,
/// leapr.f90:1798-1832).
///
/// Returns `(bex, rdbex, nbx)`: `bex[0..nbx]` is the beta grid extended to
/// negative beta (reversed), `rdbex` holds `1/(bex[i+1]-bex[i])`.
pub(crate) fn bfill(betan: &[f64]) -> (Vec<f64>, Vec<f64>, usize) {
    const SMALLB: f64 = 1e-9;
    let nbeta = betan.len();
    let mut bex = vec![0.0_f64; 2 * nbeta + 1];
    let mut k = nbeta; // 1-based index into betan
    for item in bex.iter_mut().take(nbeta) {
        *item = -betan[k - 1];
        k -= 1;
    }
    let mut kk;
    if betan[0] <= SMALLB {
        bex[nbeta - 1] = 0.0;
        kk = nbeta + 1;
    } else {
        kk = nbeta + 2;
        bex[nbeta] = betan[0];
    }
    for i in 2..=nbeta {
        bex[kk - 1] = betan[i - 1];
        kk += 1;
    }
    let nbxm = kk - 2;
    let nbx = kk - 1;
    let mut rdbex = vec![0.0_f64; 2 * nbeta + 1];
    for i in 1..=nbxm {
        rdbex[i - 1] = 1.0 / (bex[i] - bex[i - 1]);
    }
    (bex, rdbex, nbx)
}

/// Extend the asymmetric column `sexpb` (negative-beta side) to the full symmetric
/// beta grid `sex` for `sint` (`exts`, leapr.f90:1834-1865).
pub(crate) fn exts(sexpb: &[f64], exb: &[f64], betan: &[f64], nbx_len: usize) -> Vec<f64> {
    const SMALLB: f64 = 1e-9;
    let nbeta = betan.len();
    let mut sex = vec![0.0_f64; nbx_len.max(2 * nbeta + 1)];
    let mut k = nbeta;
    for item in sex.iter_mut().take(nbeta) {
        *item = sexpb[k - 1];
        k -= 1;
    }
    let mut kk;
    if betan[0] <= SMALLB {
        sex[nbeta - 1] = sexpb[0];
        kk = nbeta + 1;
    } else {
        kk = nbeta + 2;
        sex[nbeta] = sexpb[0];
    }
    for i in 2..=nbeta {
        sex[kk - 1] = sexpb[i - 1] * exb[i - 1] * exb[i - 1];
        kk += 1;
    }
    sex
}

/// Interpolate in the scattering function, or extrapolate with the SCT
/// approximation outside the tabulated range (`sint`, leapr.f90:1867-1934).
///
/// # Arguments
/// * `x` — the target beta (dimensionless).
/// * `bex`, `rdbex`, `sex`, `nbx` — from [`bfill`]/[`exts`].
/// * `alph` — the (scaled) alpha (dimensionless).
/// * `wt` — total weight `tbeta + twt` (dimensionless).
/// * `tbart` — effective-temperature ratio (dimensionless).
/// * `betan` — the scaled beta grid.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sint(
    x: f64,
    bex: &[f64],
    rdbex: &[f64],
    sex: &[f64],
    nbx: usize,
    alph: f64,
    wt: f64,
    tbart: f64,
    betan: &[f64],
) -> f64 {
    let pi = crate::common::phys::PI;
    let nbeta = betan.len();
    // SCT approximation outside the range
    if x.abs() > betan[nbeta - 1] {
        if alph <= 0.0 {
            return 0.0;
        }
        let mut ex = -(wt * alph - x.abs()) * (wt * alph - x.abs()) / (4.0 * wt * alph * tbart);
        if x > 0.0 {
            ex -= x;
        }
        return ex.exp() / (4.0 * pi * wt * alph * tbart);
    }
    // bisection search for x in bex
    let mut k1 = 1usize;
    let mut k2 = nbeta;
    let mut k3 = nbx;
    loop {
        if x == bex[k2 - 1] {
            return sex[k2 - 1];
        } else if x > bex[k2 - 1] {
            k1 = k2;
            k2 = (k3 - k2) / 2 + k2;
            if k3 - k1 <= 1 {
                break;
            }
        } else {
            k3 = k2;
            k2 = (k2 - k1) / 2 + k1;
            if k3 - k1 <= 1 {
                break;
            }
        }
    }
    let ss1 = if sex[k1 - 1] <= 0.0 {
        SLIM
    } else {
        sex[k1 - 1].ln()
    };
    let ss3 = if sex[k3 - 1] <= 0.0 {
        SLIM
    } else {
        sex[k3 - 1].ln()
    };
    let ex = ((bex[k3 - 1] - x) * ss1 + (x - bex[k1 - 1]) * ss3) * rdbex[k1 - 1];
    if ex > SLIM {
        ex.exp()
    } else {
        0.0
    }
}

/// Convolve discrete oscillators with the continuous scattering law (`discre`,
/// leapr.f90:1320-1661).
///
/// Mutates `sab` in place, updates the Debye-Waller lambda `dwpix` and the
/// effective temperature `tempf`. Faithful to NJOY, including its cross-alpha
/// accumulation of the running effective-temperature ratio `tbart` and its
/// single-delta placement quirk (see source comments).
///
/// # Arguments
/// * `sab` — the continuous law (modified in place).
/// * `input` — the LEAPR job (alpha/beta grids, oscillators, `tbeta`, `twt`).
/// * `dwpix` — Debye-Waller lambda `f0`; updated in place.
/// * `tempf` — effective temperature \[K\]; updated in place.
pub fn add_discrete_oscillators(
    sab: &mut SabMatrix,
    input: &LeaprInput,
    dwpix: &mut f64,
    tempf: &mut f64,
) {
    let nd = input.oscillators.len();
    if nd == 0 {
        return;
    }
    let nalpha = input.alpha.len();
    let nbeta = input.beta.len();
    let tev = input.tev();
    let sc = if input.lat { THERM / tev } else { 1.0 };
    let arat = input.arat;
    let tbeta = input.continuous.tbeta;
    let twt = input.continuous.twt;
    let tempr = input.temperature_k;
    // Boltzmann constant from the job's own constant set, so that a run
    // reproducing a pre-2018 evaluation uses that evaluation's value
    // (see `crate::leapr::vintage`).
    let bk = input.constants.bk_ev_per_k();

    // oscillator parameters
    let mut bdeln = vec![0.0_f64; nd];
    let mut ar = vec![0.0_f64; nd];
    let mut dist = vec![0.0_f64; nd];
    let mut dbw = vec![0.0_f64; nd];
    let mut tsave = 0.0_f64;
    let dw0 = *dwpix;
    for i in 0..nd {
        let bdel = input.oscillators[i].energy_ev;
        let adel = input.oscillators[i].weight;
        bdeln[i] = bdel / tev;
        let eb = (bdeln[i] / 2.0).exp();
        let sn = (eb - 1.0 / eb) / 2.0;
        let cn = (eb + 1.0 / eb) / 2.0;
        ar[i] = adel / (sn * bdeln[i]);
        dist[i] = adel * bdel * cn / (2.0 * sn);
        tsave += dist[i] / bk;
        dbw[i] = ar[i] * cn;
        if *dwpix > 0.0 {
            *dwpix += dbw[i];
        }
    }

    // functions of beta
    let mut exb = vec![0.0_f64; nbeta];
    let mut betan = vec![0.0_f64; nbeta];
    for i in 0..nbeta {
        let be = input.beta[i] * sc;
        exb[i] = (-be / 2.0).exp();
        betan[i] = be;
    }
    let (bex, rdbex, nbx) = bfill(&betan);

    // NOTE: tbart accumulates across the alpha loop, exactly as in NJOY.
    let mut tbart = *tempf / tempr;

    for nal in 0..nalpha {
        let al = input.alpha[nal] * sc / arat;
        let dwf = (-al * dw0).exp();

        let col: Vec<f64> = (0..nbeta).map(|j| sab.get(j, nal)).collect();
        let sex = exts(&col, &exb, &betan, nbx);
        let mut sexpb = vec![0.0_f64; nbeta];

        // accumulate delta-function ladder over all oscillators
        let mut ben = vec![0.0_f64; MAXDD];
        let mut wtn = vec![0.0_f64; MAXDD];
        ben[0] = 0.0;
        wtn[0] = 1.0;
        let mut nn = 1usize;

        for i in 0..nd {
            let dwc = al * dbw[i];
            let xx = al * ar[i];
            let (bzero, bplus, bminus) = bfact(xx, dwc, bdeln[i]);

            let mut bes = vec![0.0_f64; MAXDD];
            let mut wts = vec![0.0_f64; MAXDD];
            let mut n = 0usize;

            // n = 0 term
            for m in 0..nn {
                let besn = ben[m];
                let wtsn = wtn[m] * bzero;
                if (besn <= 0.0 || wtsn >= SMALL) && n < MAXDD {
                    bes[n] = besn;
                    wts[n] = wtsn;
                    n += 1;
                }
            }
            // negative n terms
            let mut k = 0usize;
            let mut idone = false;
            while k < 50 && !idone {
                k += 1;
                if bminus[k - 1] <= 0.0 {
                    idone = true;
                } else {
                    for m in 0..nn {
                        let besn = ben[m] - (k as f64) * bdeln[i];
                        let wtsn = wtn[m] * bminus[k - 1];
                        if wtsn >= SMALL && n < MAXDD {
                            bes[n] = besn;
                            wts[n] = wtsn;
                            n += 1;
                        }
                    }
                }
            }
            // positive n terms
            k = 0;
            idone = false;
            while k < 50 && !idone {
                k += 1;
                if bplus[k - 1] <= 0.0 {
                    idone = true;
                } else {
                    for m in 0..nn {
                        let besn = ben[m] + (k as f64) * bdeln[i];
                        let wtsn = wtn[m] * bplus[k - 1];
                        if wtsn >= SMALL && n < MAXDD {
                            bes[n] = besn;
                            wts[n] = wtsn;
                            n += 1;
                        }
                    }
                }
            }

            nn = n;
            ben[..nn].copy_from_slice(&bes[..nn]);
            wtn[..nn].copy_from_slice(&wts[..nn]);
            tbart += dist[i] / bk / tempr;
        }

        // final line list is in ben/wtn with count nn
        let mut bes = ben.clone();
        let mut wts = wtn.clone();
        let mut n = nn;

        // sort discrete lines (descending by weight), leaving bes[0] (beta=0) fixed
        if n >= 2 {
            let mut nns = n - 1; // matches Fortran nn=n-1
            for ii in 2..=(n.saturating_sub(1)) {
                for jj in (ii + 1)..=n {
                    if wts[jj - 1] >= wts[ii - 1] {
                        wts.swap(jj - 1, ii - 1);
                        bes.swap(jj - 1, ii - 1);
                    }
                }
            }
            // throw out the smallest lines
            let mut i = 0usize;
            let mut idone = false;
            while i < nns && !idone {
                i += 1;
                n = i;
                if wts[i - 1] < 100.0 * SMALL && i > 5 {
                    idone = true;
                }
            }
            let _ = &mut nns;
        }

        // add the continuum part
        for m in 1..=n {
            for j in 1..=nbeta {
                let be = -betan[j - 1] - bes[m - 1];
                let st = sint(be, &bex, &rdbex, &sex, nbx, al, tbeta + twt, tbart, &betan);
                let add = wts[m - 1] * st;
                if add >= TINY {
                    sexpb[j - 1] += add;
                }
            }
        }

        // add the delta functions (only when there is no translational term).
        // NOTE (faithful to NJOY): `idone` is shared with the inner nearest-beta
        // search, so at most one delta is placed per alpha — a known upstream
        // quirk, preserved here for line-traceability.
        if twt <= 0.0 {
            let mut m = 0usize;
            let mut idone = false;
            while m < n && !idone {
                m += 1;
                if dwf < VSMALL {
                    idone = true;
                } else if bes[m - 1] < 0.0 {
                    let be = -bes[m - 1];
                    if nbeta >= 2 && be <= betan[nbeta - 2] {
                        let mut db = 1000.0_f64;
                        idone = false;
                        let mut j = 0usize;
                        let mut jj = 0usize;
                        while j < nbeta && !idone {
                            j += 1;
                            jj = j;
                            if (be - betan[j - 1]).abs() > db {
                                idone = true;
                            } else {
                                db = (be - betan[j - 1]).abs();
                            }
                        }
                        let mut add = if jj <= 2 {
                            wts[m - 1] / betan[jj - 1]
                        } else {
                            2.0 * wts[m - 1] / (betan[jj - 1] - betan[jj - 3])
                        };
                        add *= dwf;
                        if add >= TINY {
                            sexpb[jj - 2] += add;
                        }
                    }
                }
            }
        }

        // record results
        for j in 0..nbeta {
            sab.set(j, nal, sexpb[j]);
        }
    }

    *tempf = (tbeta + twt) * *tempf + tsave;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leapr::input::{ContinuousDist, DiscreteOscillator};

    /// Methodology: compare `bessel_i0`/`bessel_i1` at `x = 1` to the standard
    /// values `I0(1) = 1.2660658`, `I1(1) = 0.5651591`. Result (2026-07-15): both
    /// match to `< 5e-7` absolute (the A-S polynomial accuracy).
    #[test]
    fn bessel_reference_values() {
        assert!(
            (bessel_i0(1.0) - 1.2660658).abs() < 5e-7,
            "I0(1) = {}",
            bessel_i0(1.0)
        );
        assert!(
            (bessel_i1(1.0) - 0.5651591).abs() < 5e-7,
            "I1(1) = {}",
            bessel_i1(1.0)
        );
    }

    /// Large-argument branch: `I0(5) = 27.2399`, `I1(5) = 24.3356`.
    /// Result (2026-07-15): relative error `< 1e-4` (A-S asymptotic form).
    #[test]
    fn bessel_large_argument_branch() {
        assert!(
            (bessel_i0(5.0) - 27.2399).abs() / 27.2399 < 1e-4,
            "I0(5) = {}",
            bessel_i0(5.0)
        );
        assert!(
            (bessel_i1(5.0) - 24.3356).abs() / 24.3356 < 1e-4,
            "I1(5) = {}",
            bessel_i1(5.0)
        );
    }

    /// `bfact` sanity: the zero-phonon weight `bzero` must be positive and the
    /// down-scatter ladder `bminus` must dominate the up-scatter ladder `bplus`
    /// term-by-term (detailed balance, `bminus/bplus = exp(k*betai)` before
    /// truncation). Result (2026-07-15): `bzero > 0`; where both are non-zero,
    /// `bminus[k] >= bplus[k]`.
    #[test]
    fn bfact_detailed_balance_ladder() {
        let betai = 0.8;
        let (bzero, bplus, bminus) = bfact(0.5, 0.3, betai);
        assert!(bzero > 0.0 && bzero.is_finite(), "bzero = {bzero}");
        for k in 0..50 {
            if bplus[k] > 0.0 && bminus[k] > 0.0 {
                assert!(
                    bminus[k] >= bplus[k] * 0.999,
                    "k={k} bminus {} < bplus {}",
                    bminus[k],
                    bplus[k]
                );
            }
        }
    }

    /// Smoke test: convolving a single discrete oscillator onto a positive
    /// continuous law keeps S finite and non-negative and grows the Debye-Waller
    /// lambda. Result (2026-07-15): no NaN/negative entries; `dwpix` increases.
    #[test]
    fn add_discrete_runs_and_is_finite() {
        let nbeta = 30;
        let beta: Vec<f64> = (0..nbeta).map(|i| i as f64 * 0.4).collect();
        let alpha = vec![0.5, 1.0, 2.0];
        let input = LeaprInput {
            alpha: alpha.clone(),
            beta: beta.clone(),
            lat: false,
            arat: 1.0,
            nphon: 50,
            temperature_k: 293.6,
            continuous: ContinuousDist {
                delta_ev: 0.002,
                rho: vec![0.0, 1.0, 2.0],
                twt: 0.0,
                c: 0.0,
                tbeta: 0.6,
            },
            oscillators: vec![DiscreteOscillator {
                energy_ev: 0.2,
                weight: 0.4,
            }],
            constants: crate::leapr::vintage::PhysicalConstants::default(),
        };
        let mut sab = SabMatrix::zeros(nbeta, alpha.len());
        for j in 0..alpha.len() {
            for k in 0..nbeta {
                sab.set(k, j, (-(beta[k]) / 2.0).exp() * 0.02);
            }
        }
        let mut dwpix = 5.0_f64;
        let dw_before = dwpix;
        let mut tempf = 1.2 * input.temperature_k;
        add_discrete_oscillators(&mut sab, &input, &mut dwpix, &mut tempf);
        for j in 0..alpha.len() {
            for k in 0..nbeta {
                let v = sab.get(k, j);
                assert!(v.is_finite() && v >= 0.0, "sab[{k},{j}] = {v}");
            }
        }
        assert!(
            dwpix > dw_before,
            "dwpix {dwpix} did not grow from {dw_before}"
        );
        assert!(tempf.is_finite());
    }
}
