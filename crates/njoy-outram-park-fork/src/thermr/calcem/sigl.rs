//! `sigl` — the cross section `σ(E→E')` and its angular representation
//! (equally-probable cosines, or Legendre moments) for one `(E, E')` pair.
//!
//! Faithful port of NJOY2016 `thermr.f90::sigl` (lines 2660-2872). `sigl`
//! itself calls the double-differential kernel (`sig` in the Fortran,
//! [`IncoherentInelastic::double_differential`] in this port — reused as-is,
//! not reimplemented) at an adaptively-refined set of scattering cosines `μ`,
//! then either:
//!
//! - integrates it to the cross section `σ(E→E')` \[barn\] (always), and
//! - (`nlin < 0`, the convention `calcem` always uses) partitions `[-1,1]`
//!   into `nbin = |nlin|-1` bins of **equal probability** and reports each
//!   bin's probability-weighted mean cosine, or
//! - (`nlin ≥ 0`) accumulates the Legendre moments `P_1..P_{nl-1}` of the
//!   normalized angular distribution instead.
//!
//! `calcem` only ever calls this with `nlin < 0` (`nnl = -nl`,
//! `thermr.f90:1637`), so the `nlin ≥ 0` branch is exercised by nothing in
//! this crate yet — ported anyway for fidelity to the general subroutine, and
//! left available for a future consumer.
//!
//! Two independent adaptive-linearisation sweeps over `μ ∈ [-1,1]` happen
//! here, matching the Fortran exactly: the first (labels 100-120) integrates
//! the total cross section; the second (labels 130-260), given that total,
//! re-linearises from scratch to locate the equally-probable-cosine
//! boundaries. NJOY recomputes `sig` from zero in the second sweep rather than
//! reusing the first sweep's points — this port does the same, on purpose
//! (translation, not "improvement").

use super::super::mf7::IncoherentInelastic;
use crate::error::NjoyError;
use crate::groupr::kinematics::legndr;
use crate::mixr::mix::sigfig;

/// Reference temperature `kT₀ = 0.0253 eV` (`tevz`/`therm` in `thermr.f90`).
/// Duplicated from [`super::super::inelastic::TEVZ`] (private there) rather
/// than exposed cross-module, since this is the same physical constant NJOY
/// names identically in both places. `pub(super)` so the `calcem` siblings
/// that also need it (the incident-energy panel walks in [`super::iform0`]
/// and [`super::iform1`]) can share this one definition.
pub(super) const TEVZ: f64 = 0.0253;

/// Bisection-stack depth cap (`imax` in `thermr.f90`, both `sigl` and
/// `calcem`): at most 20 simultaneously-open panel boundaries.
const IMAX: usize = 20;
/// `sigl`'s `xtol`: stop bisecting an interval narrower than this in cosine.
const XTOL: f64 = 0.00001;
/// `sigl`'s `ytol`: relative tolerance on the quadratic-root bin-boundary
/// solve.
const YTOL: f64 = 0.001;
/// `sigl`'s `sigmin`: a cross section at or below this is treated as zero.
const SIGMIN: f64 = 1.0e-32;
/// `sigl`'s `eps`: the near-edge-of-range guard on the initial peak-location
/// guess, and the absolute floor under `ymax`.
const EPS: f64 = 1.0e-3;
/// `sigl`'s `shade`: how close the running partial-bin sum may get to the
/// target `fract` before the bin boundary is forced to land here rather than
/// carrying into the next segment.
const SHADE: f64 = 0.99999999;
const THIRD: f64 = 1.0 / 3.0;

/// `sigl(e, ep, nlin, ii, natom, tolin)` — cross section and angular data for
/// scattering `e → ep` \[eV\] at temperature `ii.temperature_k` \[K\].
///
/// Returns a 1-based-content `Vec<f64>` of length `|nlin| + 1` mirroring
/// NJOY's `s(nlmax)` array exactly, so index arithmetic here matches the
/// Fortran line-for-line: index `0` is unused padding, `[1]` is the cross
/// section `σ(E→E')` \[barn\] (NJOY's `s(1)`), and `[2..=|nlin|]` holds either
/// the `|nlin|-1` equally-probable mean cosines (`nlin < 0`, dimensionless,
/// ascending) or the Legendre moments `P_1..P_{|nlin|-1}` of the normalized
/// angular distribution (`nlin ≥ 0`).
///
/// `natom` is the number of principal-scatterer atoms per molecule (passed
/// straight through to [`IncoherentInelastic::sigma_bound`]/`double_differential`).
/// `tolin` is `calcem`'s reconstruction tolerance (`tol = tolin`  in `calcem`,
/// halved here to `sigl`'s own `tol = tolin/2`, matching `thermr.f90:2696`).
///
/// # Errors
/// [`NjoyError::NotConvergent`] where NJOY's own `sigl` would `call
/// error('sigl', 'no legal solution...', ...)` — a genuine failure of the
/// quadratic/linear bin-boundary solve, not expected in ordinary use.
pub fn sigl(
    ii: &IncoherentInelastic,
    e: f64,
    ep: f64,
    nlin: i32,
    natom: f64,
    tolin: f64,
) -> Result<Vec<f64>, NjoyError> {
    let nl: usize = nlin.unsigned_abs() as usize;
    let az = ii.mass_ratio();
    let temp_k = ii.temperature_k;
    let tev = crate::common::phys::BK_EV_PER_K * temp_k;
    let lat = ii.lat;
    let sig = |mu: f64| ii.double_differential(e, ep, mu, temp_k, natom);

    let tol = 0.5 * tolin;

    // Constant factors (thermr.f90:2694-2701). `iinc.eq.2` is NJOY's "compute
    // from tabulated ENDF S(alpha,beta)" mode, which is the only mode this
    // port's `IncoherentInelastic` ever represents (the free-gas `iinc=1`
    // main-scatterer path — thermr.f90:1857-1916 — builds its own synthetic
    // 45-point beta grid and is NOT ported; see the module docs), so the
    // `iinc.eq.2` guard collapses to just `lat.eq.1` here.
    let mut b = (ep - e) / tev;
    if lat == 1 {
        b *= tev / TEVZ;
    }
    b = b.abs();
    let s1bb = (1.0 + b * b).sqrt();
    let seep = if ep != 0.0 {
        1.0 / (e * ep).sqrt()
    } else {
        0.0
    };

    let mut s = vec![0.0f64; nl + 1];

    // ── Sweep 1 (labels 100-120): adaptive integration of the total cross
    // section over mu in [-1,1]. ──────────────────────────────────────────
    let mut x = [0.0f64; IMAX + 1];
    let mut y = [0.0f64; IMAX + 1];
    let mut i: usize = 3;
    let mut sum: f64;
    x[3] = -1.0;
    let mut xl = x[3];
    y[3] = sig(x[3]);
    let mut yl = y[3];
    x[2] = if ep == 0.0 {
        0.0
    } else {
        0.5 * (e + ep - (s1bb - 1.0) * az * tev) * seep
    };
    if x[2].abs() > 1.0 - EPS {
        x[2] = 0.99;
    }
    x[2] = sigfig(x[2], 8, 0);
    y[2] = sig(x[2]);
    x[1] = 1.0;
    y[1] = sig(x[1]);
    let mut ymax = y[2].max(y[1]).max(y[3]);
    if ymax < EPS {
        ymax = EPS;
    }

    sum = 0.0;
    loop {
        // label 110
        let accept = if i == IMAX {
            true
        } else {
            let xm = sigfig(0.5 * (x[i - 1] + x[i]), 8, 0);
            let ym = 0.5 * (y[i - 1] + y[i]);
            let yt = sig(xm);
            let test = tol * yt.abs() + tol * ymax / 50.0;
            let test2 = ym + ymax / 100.0;
            if (yt - ym).abs() <= test
                && (y[i - 1] - y[i]).abs() <= test2
                && (x[i - 1] - x[i]) < 0.5
            {
                true
            } else if x[i - 1] - x[i] < XTOL {
                true
            } else {
                i += 1;
                x[i] = x[i - 1];
                y[i] = y[i - 1];
                x[i - 1] = xm;
                y[i - 1] = yt;
                false
            }
        };
        if !accept {
            continue;
        }
        // label 120
        sum += 0.5 * (y[i] + yl) * (x[i] - xl);
        xl = x[i];
        yl = y[i];
        i -= 1;
        if i > 1 {
            continue;
        }
        if i == 1 {
            continue; // Fortran: "if (i.eq.1) go to 120" — re-run this block at i=1.
        }
        break; // i == 0
    }

    s[1] = sum;
    if sum <= SIGMIN {
        for v in s.iter_mut() {
            *v = 0.0;
        }
        return Ok(s);
    }

    // ── Sweep 2 (labels 130-260): re-linearise from scratch and extract the
    // `nbin` equally-probable-cosine bin boundaries (or Legendre moments). ──
    let nbin = nl - 1;
    let rnbin = 1.0 / nbin as f64;
    let fract = sum * rnbin;
    let rfract = 1.0 / fract;
    let mut sum2 = 0.0f64; // sweep 2's own running partial-bin sum ("sum" reused in Fortran)
    let mut gral = 0.0f64;
    for v in s.iter_mut().skip(2) {
        *v = 0.0;
    }
    let mut j: usize = 0;

    let mut x = [0.0f64; IMAX + 1];
    let mut y = [0.0f64; IMAX + 1];
    let mut i: usize = 3;
    x[3] = -1.0;
    let mut xl2 = x[3];
    y[3] = sig(x[3]);
    x[2] = if ep == 0.0 {
        0.0
    } else {
        0.5 * (e + ep - (s1bb - 1.0) * az * tev) * seep
    };
    if x[2].abs() > 1.0 - EPS {
        x[2] = 0.99;
    }
    x[2] = sigfig(x[2], 8, 0);
    y[2] = sig(x[2]);
    x[1] = 1.0;
    y[1] = sig(x[1]);
    let mut ymax = y[1].max(y[2]).max(y[3]);
    if ymax < EPS {
        ymax = EPS;
    }
    let mut yl2 = y[3];
    // `xn` persists across bin extractions the way the Fortran local does
    // (the `f == 0` corner of the quadratic solve, thermr.f90:2839-2841,
    // leaves it unassigned that iteration and the stale value is reused).
    let mut xn = 0.0f64;

    'panels: loop {
        // label 150
        let accept = if i == IMAX {
            true
        } else {
            let xm = sigfig(0.5 * (x[i - 1] + x[i]), 8, 0);
            let ym = 0.5 * (y[i - 1] + y[i]);
            let yt = sig(xm);
            let test = tol * yt.abs() + tol * ymax / 50.0;
            let test2 = ym + ymax / 100.0;
            if (yt - ym).abs() <= test
                && (y[i - 1] - y[i]).abs() <= test2
                && (x[i - 1] - x[i]) < 0.5
            {
                true
            } else if x[i - 1] - x[i] < XTOL {
                true
            } else {
                i += 1;
                x[i] = x[i - 1];
                y[i] = y[i - 1];
                x[i - 1] = xm;
                y[i - 1] = yt;
                false
            }
        };
        if !accept {
            continue 'panels;
        }

        // label 160: check bins for this panel. Every "label 250" exit below
        // (duplicated 3x, matching the 3 Fortran `go to 250` sites) moves the
        // trailing boundary up to x[i], decrements i (*after* the decrement,
        // not before — thermr.f90:2848-2850), and either resumes bisecting
        // the next-smaller panel (new i > 1), re-enters this same
        // "check bins" logic once more at the collapsed i == 1, or (i == 0,
        // not expected in well-formed input) stops.
        'this_panel: loop {
            let add = 0.5 * (y[i] + yl2) * (x[i] - xl2);
            if x[i] == xl2 {
                // label 250
                xl2 = x[i];
                yl2 = y[i];
                i -= 1;
                if i > 1 {
                    continue 'panels;
                }
                if i == 1 {
                    continue 'this_panel;
                }
                break 'this_panel;
            }
            let xil = 1.0 / (x[i] - xl2);

            if i == 1 && j == nbin - 1 {
                // label 165
                xn = x[i];
                j += 1;
            } else if sum2 + add >= fract * SHADE && j < nbin - 1 {
                // label 170
                j += 1;
                let mut solved = false;
                if yl2 >= SIGMIN {
                    let test = (fract - sum2) * (y[i] - yl2) / ((x[i] - xl2) * yl2 * yl2);
                    if test.abs() <= YTOL {
                        let cand = xl2 + (fract - sum2) / yl2;
                        if cand > x[i] {
                            xn = x[i]; // label 180
                            solved = true;
                        } else if cand >= xl2 && cand <= x[i] {
                            xn = cand;
                            solved = true;
                        } else {
                            return Err(NjoyError::NotConvergent {
                                routine: "sigl: no legal solution",
                            });
                        }
                    }
                }
                if !solved {
                    // label 175
                    let f = (y[i] - yl2) * xil;
                    let rf = 1.0 / f;
                    let mut disc = (yl2 * rf).powi(2) + 2.0 * (fract - sum2) * rf;
                    if disc < 0.0 {
                        disc = disc.abs(); // NJOY: warn and continue with |disc|
                    }
                    let cand = if f > 0.0 {
                        Some(xl2 - (yl2 * rf) + disc.sqrt())
                    } else if f < 0.0 {
                        Some(xl2 - (yl2 * rf) - disc.sqrt())
                    } else {
                        None // f == 0: xn keeps its stale value, per Fortran
                    };
                    if let Some(cand) = cand {
                        xn = cand;
                    }
                    if xn > xl2 && xn <= x[i] {
                        // -> 190, xn already set
                    } else if xn > xl2 && xn < x[i] + YTOL * (x[i] - xl2) {
                        xn = x[i]; // label 180
                    } else {
                        return Err(NjoyError::NotConvergent {
                            routine: "sigl: no legal solution (quadratic path)",
                        });
                    }
                }
            } else {
                // accept the whole segment into the running partial-bin sum, no
                // boundary here — label 250 follows.
                sum2 += add;
                gral += 0.5 * (yl2 * x[i] - y[i] * xl2) * (x[i] + xl2)
                    + THIRD * (y[i] - yl2) * (x[i] * x[i] + x[i] * xl2 + xl2 * xl2);
                // label 250
                xl2 = x[i];
                yl2 = y[i];
                i -= 1;
                if i > 1 {
                    continue 'panels;
                }
                if i == 1 {
                    continue 'this_panel;
                }
                break 'this_panel;
            }

            // label 190: a bin boundary was found at xn.
            let yn = yl2 + (y[i] - yl2) * (xn - xl2) * xil;
            gral += (xn - xl2)
                * (yl2 * 0.5 * (xn + xl2)
                    + (y[i] - yl2)
                        * xil
                        * (-xl2 * 0.5 * (xn + xl2) + THIRD * (xn * xn + xn * xl2 + xl2 * xl2)));
            let xbar = gral * rfract;

            if nlin >= 0 {
                let p = legndr(xbar, nl.saturating_sub(1).max(1));
                for il in 2..=nl {
                    s[il] += p.get(il - 1).copied().unwrap_or(0.0) * rnbin;
                }
            } else {
                s[j + 1] = xbar;
            }

            xl2 = xn;
            yl2 = yn;
            sum2 = 0.0;
            gral = 0.0;
            if j == nbin {
                return Ok(s); // label 260
            }
            if xl2 < x[i] {
                continue 'this_panel; // label 160 again, same panel
            }
            // label 250
            xl2 = x[i];
            yl2 = y[i];
            i -= 1;
            if i > 1 {
                continue 'panels;
            }
            if i == 1 {
                continue 'this_panel;
            }
            break 'this_panel;
        }
    }
}
