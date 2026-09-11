// Ported from NJOY2016 `src/broadr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine broadn`, l.1256-1508 (`broadn_section`) — the node walk (labels 120-130),
//     the midpoint-insertion stack with its convergence test (labels 140-170), the
//     threshold gating on output (l.1441-1445) and the copy-through above `thnmax`
//     (label 190); single-page, one-reaction (`nreac = 1`) form.
//   - `subroutine broadr` defaults `errmax = 10*errthn`, `errint = errthn/20000`
//     (l.205-209) and the lab threshold `emtr = -q*(awr+1)/awr` (l.592-600).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The adaptive output grid of BROADR — `broadn` (`broadr.f90:1256-1508`).
//!
//! Upstream never evaluates the SIGMA1 kernel only at the input grid points.
//! `broadn` walks the input grid picking *nodes* (a point where the slope
//! sign changes, a "round" 3-figure energy, `0.0253` eV, every 10th point,
//! or a doubling of energy — labels 120-130), then bisects each node
//! interval on a 12-deep stack until the broadened value at the midpoint is
//! within `errthn` of the linear interpolation between the interval ends
//! (labels 140-170, with the `errmax` / `errint` integral relaxation and
//! the `rmax = 3` ratio guard). Points the walk skips are dropped — that
//! is BROADR's "thinning" — and midpoints are *added* wherever the
//! broadened function needs them, which for a light nuclide is exactly the
//! free-gas `1/v` rise below ~1 eV that a flat elastic cross section grows
//! at room temperature.
//!
//! **Why this exists (2026-09-10).** The port used to broaden at the input
//! grid points only. For H-2 that left 178 points where NJOY writes 542, and
//! linear interpolation across the missing thermal rise made the elastic
//! cross section 3.0× (1e-3 eV) and 5.2× (1e-2 eV) too high against NJOY's
//! own PENDF, while agreeing exactly *at* the surviving grid points. Found by
//! the ERRORR tier-2 golden test (bead `op-tubm`).
//!
//! **Scope (honest).** One reaction at a time on its own grid (`nreac = 1`):
//! upstream walks all broadened reactions *together* on the union grid, so
//! its slope-sign test and convergence test are joint and its output grid is
//! one grid for every reaction. Each section here meets the tolerance on its
//! own; the grids differ from NJOY's union grid, the values do not (to
//! `errthn`). Paging (`bfile3`) is not needed in memory.

use crate::mixr::mix::sigfig;

use super::bsigma_scalar;

/// BROADR's thinning tolerances (card 3: `errthn thnmax errmax errint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BroadnTolerances {
    /// `errthn` — fractional tolerance for thinning / point insertion.
    pub errthn: f64,
    /// `errmax` — tolerance used when the integral criterion is met
    /// (`>= errthn`; default `10*errthn`).
    pub errmax: f64,
    /// `errint` — integral-thinning parameter (default `errthn/20000`).
    pub errint: f64,
}

impl BroadnTolerances {
    /// The upstream defaults for a given `errthn` (`broadr.f90:208-209`).
    pub fn with_errthn(errthn: f64) -> Self {
        BroadnTolerances {
            errthn,
            errmax: 10.0 * errthn,
            errint: errthn / 20_000.0,
        }
    }
}

impl Default for BroadnTolerances {
    /// `errthn = 0.001`, the value every deck in `reference-data/` uses.
    fn default() -> Self {
        Self::with_errthn(0.001)
    }
}

/// Lab-system threshold energy `emtr` for a reaction with Q-value `q`
/// \[eV\] (`broadr.f90:596-600`): `0` for `q = 0`, else `-q (awr+1)/awr`.
pub fn lab_threshold(q: f64, awr: f64) -> f64 {
    if q == 0.0 {
        0.0
    } else {
        -q * (awr + 1.0) / awr
    }
}

/// Maximum stack depth — upstream's `nstack = 12` raised to 40. **The one
/// deliberate deviation in this port.** When the stack is full upstream
/// accepts the interval unexamined (`if (is.ge.nstack) go to 150`). It
/// never gets there in NJOY because `broadn` runs on RECONR's *union*
/// grid, where consecutive nodes are close; this crate's RECONR keeps each
/// reaction on its own grid (H-2 elastic: 0.0253 eV is followed by 100 eV),
/// and eleven halvings from 100 eV only reach 0.12 eV, so the whole thermal
/// `1/v` rise was accepted as one chord (measured 2026-09-10: 6 % low at
/// 0.059 eV vs NJOY's PENDF). With a deeper stack every interval is judged
/// by the same `errthn`/`errmax`/`errint` test as upstream; on a grid where
/// upstream's stack never fills the two are identical.
const NSTACK: usize = 40;
/// `nmax` — take a node at least every 10 input points.
const NMAX: usize = 10;
const THERM: f64 = 0.0253;
const ESTP: f64 = 4.1;
const SMALL: f64 = 1.0e-9;
const STEP: f64 = 2.01;
const RMAX: f64 = 3.0;
const ERRMIN: f64 = 1.0e-15;
const TRANGE: f64 = 0.4999;
const SSMALL: f64 = 1.0e-6;
const TENTH: f64 = 0.1;

/// Sign of the slope between two consecutive input points, with the
/// `|dn| < |s(k)|/1000` dead band (`broadr.f90:1329-1333`, `:1374-1381`).
fn slope_sign(s: &[f64], k: usize) -> f64 {
    if k + 1 >= s.len() {
        return 1.0;
    }
    let mut dn = s[k + 1] - s[k];
    if dn.abs() < s[k].abs() / 1000.0 {
        dn = 0.0;
    }
    if dn >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Doppler-broaden one reaction onto BROADR's adaptive grid
/// (`subroutine broadn`, `broadr.f90:1256-1508`, `nreac = 1`, one page).
///
/// - `e`/`s` — the input lin-lin grid (ascending energies \[eV\], barn).
/// - `alpha` — `awr / (bk * tempef)` \[1/eV\].
/// - `thnmax` — upper energy for broadening; points above are copied through
///   unbroadened, exactly as label 190 does (the last table point always is).
/// - `emtr` — lab threshold; an output point at or below it gets `0`, as
///   `broadr.f90:1441-1445` writes it.
///
/// Returns the new `(energy, sigma)` grid. Energies of nodes are rounded to
/// 7 figures and midpoints to 8–9 (`sigfig`), as upstream.
pub fn broadn_section(
    e: &[f64],
    s: &[f64],
    alpha: f64,
    tol: &BroadnTolerances,
    thnmax: f64,
    emtr: f64,
) -> Vec<(f64, f64)> {
    let n = e.len();
    if n < 2 {
        return e.iter().copied().zip(s.iter().copied()).collect();
    }
    let eu: Vec<f64> = e
        .iter()
        .map(|&x| if x > 0.0 { (alpha * x).sqrt() } else { 0.0 })
        .collect();
    let sig: Vec<f64> = s.iter().map(|&x| x.max(0.0)).collect();
    let bsig = |et: f64| bsigma_scalar((alpha * et).sqrt(), &eu, &sig);

    let nhigh = n - 1; // 0-based index of the last point
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(n);
    // stack (1-based `is` in upstream; index 0 unused)
    let mut es = [0.0f64; NSTACK + 1];
    let mut ss = [0.0f64; NSTACK + 1];
    let mut ks = [0usize; NSTACK + 1]; // input index + 1 (0 = inserted midpoint)
    let mut js = [0i32; NSTACK + 1];
    let mut tt1 = 0.0f64; // last output energy (tt(1))
    let mut tt1old = 0.0f64;
    let mut j = 0usize; // output count

    // first node in the stack (broadr.f90:1318-1338; klow != 1 on the first page)
    let mut k = 0usize;
    let mut klast = k;
    ks[2] = k + 1;
    es[2] = sigfig(e[k], 7, 0);
    let mut dl = slope_sign(s, k);
    ss[2] = bsig(es[2]);
    js[2] = i32::from((ss[2] - s[k]).abs() > tol.errthn * s[k]);
    k += 1;

    loop {
        // label 120: locate the next node
        let mut et;
        loop {
            et = e[k];
            if et >= thnmax || k >= nhigh {
                break;
            }
            // Deviation #2 (2026-09-10): the last input point below `thnmax`
            // is always a node. Upstream judges the interval up to the
            // `thnmax` node against that node's *broadened* value, then
            // writes the node's *unbroadened* copy (label 190), so unless the
            // last resolved point happens to be a node the output chord across
            // the seam is wrong on the resolved side. NJOY's U-238 PENDF keeps
            // (19999.99 eV, 0.2809805 b) there (its union-grid walk made it a
            // node); on a per-reaction grid this rule reproduces that.
            if e[k + 1] >= thnmax {
                break;
            }
            let test = sigfig(et, 7, 0);
            let skip = (es[2] - test).abs() < SMALL * test;
            if !skip {
                if k > klast + NMAX || et > STEP * es[2] {
                    break;
                }
                let test3 = sigfig(et, 3, 0);
                if (et - test3).abs() < SMALL * test3 || (et - THERM).abs() < SMALL * THERM {
                    break;
                }
                if slope_sign(s, k) != dl {
                    break;
                }
            }
            k += 1; // label 126
        }
        // label 130: new top node
        es[1] = sigfig(et, 7, 0);
        ss[1] = bsig(es[1]);
        ks[1] = k + 1;
        js[1] = i32::from((ss[1] - s[k]).abs() > tol.errthn * s[k]);

        // labels 140-170: add points between nodes if needed
        let mut is = 2usize;
        loop {
            let mut converged = false;
            if is >= NSTACK {
                converged = true;
            } else if ks[is - 1] == ks[is] + 1 && js[is - 1] == 0 && js[is] == 0 {
                converged = true;
            }
            let mut em = 0.0;
            let mut sn = 0.0;
            if !converged {
                em = 0.5 * (es[is - 1] + es[is]);
                let mut ndig = 9;
                if em > TENTH && em < 1.0 {
                    ndig = 8;
                }
                if em > sigfig(es[is], 7, 1) && em < sigfig(es[is - 1], 7, -1) {
                    em = sigfig(em, 7, 0);
                } else {
                    em = sigfig(em, ndig, 0);
                }
                if em < sigfig(es[is], ndig, 1) || em > sigfig(es[is - 1], ndig, -1) {
                    converged = true; // interval too narrow to split
                } else {
                    let (mut errt, mut errm) = (tol.errthn, tol.errmax);
                    if es[is - 1] < TRANGE {
                        errt /= 5.0;
                        errm /= 5.0;
                    }
                    sn = bsig(em);
                    let dx = es[is - 1] - es[is];
                    let f = (em - es[is]) / dx;
                    let mut insert = f > 1.0 - 1.0 / 100.0;
                    if !insert {
                        let stot = sn;
                        if stot >= ERRMIN && (sn / stot).abs() >= SSMALL {
                            if ss[is - 1] > RMAX * ss[is] || ss[is - 1] < ss[is] / RMAX {
                                insert = true;
                            } else {
                                let si = f * ss[is - 1] + (1.0 - f) * ss[is];
                                let dy = (sn - si).abs();
                                if dy > errt * sn.abs() + ERRMIN
                                    && (dy > errm * sn.abs() + ERRMIN
                                        || dy * dx / 2.0 > tol.errint * em)
                                {
                                    insert = true;
                                }
                            }
                        }
                        if !insert {
                            // don't allow a big increase in the step size
                            let est = ESTP * (es[is] - tt1);
                            if j > 3 && dx > est {
                                insert = true;
                            }
                        }
                    }
                    converged = !insert;
                }
            }
            if converged {
                // label 150: store the top point of the stack
                tt1 = es[is];
                let v = if tt1 > emtr && tt1old >= emtr {
                    ss[is]
                } else {
                    0.0
                };
                tt1old = tt1;
                j += 1;
                out.push((tt1, v));
                is -= 1;
                if is > 1 {
                    continue;
                }
                break;
            }
            // label 170: not converged, push the midpoint
            is += 1;
            es[is] = es[is - 1];
            es[is - 1] = em;
            ks[is] = ks[is - 1];
            ks[is - 1] = 0;
            js[is] = js[is - 1];
            js[is - 1] = 1; // km == 0 for an inserted point
            ss[is] = ss[is - 1];
            ss[is - 1] = sn;
        }

        // stack exhausted: get a new node
        es[2] = es[1];
        ks[2] = ks[1];
        js[2] = js[1];
        ss[2] = ss[1];
        dl = slope_sign(s, k);
        if k >= nhigh || es[1] > thnmax {
            break; // labels 180/190
        }
        klast = k;
        k += 1;
    }
    // label 190: copy the rest of the table unbroadened
    while k <= nhigh {
        out.push((e[k], s[k]));
        k += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groupr::weights::BOLTZMANN_EV_PER_K;

    /// A flat elastic cross section on a coarse grid grows the free-gas
    /// `1/v` rise at thermal energies; `broadn` must *insert* points there so
    /// that lin-lin interpolation of the output reproduces the kernel to
    /// `errthn`, where the input grid alone cannot.
    #[test]
    fn inserts_points_across_the_thermal_rise() {
        let awr = 1.9968; // H-2
        let alpha = awr / (BOLTZMANN_EV_PER_K * 293.6);
        let e: Vec<f64> = vec![1e-5, 1e-4, 1e-3, 1e-2, 0.1, 1.0, 10.0, 100.0, 1e3];
        let s = vec![3.4; e.len()];
        let tol = BroadnTolerances::default();
        let out = broadn_section(&e, &s, alpha, &tol, 1e3, 0.0);
        assert!(out.len() > e.len(), "no points inserted: {}", out.len());
        assert!(out.windows(2).all(|w| w[1].0 > w[0].0), "not ascending");
        // interpolate the output at random interior energies and compare to
        // the kernel evaluated directly
        let eu: Vec<f64> = e.iter().map(|&x| (alpha * x).sqrt()).collect();
        let xs: Vec<f64> = out.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = out.iter().map(|p| p.1).collect();
        for &et in &[2e-4, 5e-4, 3e-3, 7e-3, 0.03, 0.3, 3.0] {
            let want = bsigma_scalar((alpha * et).sqrt(), &eu, &s);
            let i = xs.partition_point(|&x| x <= et) - 1;
            let t = (et - xs[i]) / (xs[i + 1] - xs[i]);
            let got = ys[i] + t * (ys[i + 1] - ys[i]);
            let rel = (got - want).abs() / want;
            assert!(
                rel < 2.0 * tol.errthn,
                "E={et}: {got} vs {want} (rel {rel:.2e})"
            );
        }
        // the last point is copied through unbroadened (label 190)
        assert_eq!(*out.last().unwrap(), (1e3, 3.4));
    }

    /// Points above `thnmax` are copied through, the node at `thnmax` is
    /// broadened, and a threshold reaction is zero at and below `emtr`.
    #[test]
    fn thnmax_copy_through_and_threshold_gating() {
        let alpha = 1.0 / (BOLTZMANN_EV_PER_K * 300.0);
        let e: Vec<f64> = (1..=40).map(|i| i as f64 * 0.5).collect();
        let s: Vec<f64> = e
            .iter()
            .map(|&x| if x < 5.0 { 0.0 } else { x - 5.0 })
            .collect();
        let tol = BroadnTolerances::default();
        let out = broadn_section(&e, &s, alpha, &tol, 10.0, 5.0);
        for &(x, y) in &out {
            if x > 10.0 {
                assert_eq!(y, x - 5.0, "copied through above thnmax");
            }
            if x <= 5.0 {
                assert_eq!(y, 0.0, "zero at/below emtr");
            }
        }
        assert!(out.iter().any(|&(x, _)| x > 5.0 && x <= 10.0));
    }
}
