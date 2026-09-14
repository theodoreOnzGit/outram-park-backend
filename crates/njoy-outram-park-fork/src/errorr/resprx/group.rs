// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine rpxgrp`, l.5227-5367 — pointwise-to-groupwise conversion of the
//     resonance cross sections (or their finite differences) with the `egtwtf` weight.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `rpxgrp` — the "simplistic" group average the ERRORJ chain uses for the
//! resonance sensitivities: trapezoidal panels between consecutive
//! pointwise energies, each weighted linearly by `egtwtf`, normalised by
//! the same weight integral.
//!
//! Two upstream idioms are kept verbatim because the oracle carries them
//! (neither fires for a group structure whose groups are wider than one
//! energy step, which is the case for every built-in `ign`):
//!
//! - at label 1000 (`errorr.f90:5330-5346`), a step that jumps over a whole
//!   group writes `gsig = yl*zl + yr*zr/sumde` — the division binds only
//!   to the second term — and evaluates `yl`/`yr` from the `y1`/`y2` left
//!   over from the previous `j` loop (the capture column) for all three
//!   columns;
//! - the first point is looked up against the *current* `ig` only, so
//!   the search restarts from group 1 for the first in-range point.

use crate::NjoyError;

use super::super::weight::{ErrorrWeight, WeightSampler};

/// One pointwise entry `sig(i, 1:5)`: `[σ_t, σ_el, σ_f, σ_γ, E]`.
pub type SigRow = [f64; 5];

/// Group-average `sig` (columns 2–4, energies in column 5) over the group
/// bounds `egn` (`rpxgrp`, `errorr.f90:5227-5367`).
///
/// Returns `gsig[ig][0..4]` for `ig = 0..ngn` as `[total, elastic,
/// fission, capture]`; the total is the sum of the other three
/// (`errorr.f90:5362-5364`). Groups the points do not reach stay zero.
///
/// # Errors
/// Upstream's `i0 > ipoint not coded` (no point lies inside the group
/// structure) and any weight-function error.
#[allow(clippy::needless_range_loop)] // index loops mirror errorr.f90:5273-5359
pub fn rpxgrp(
    egn: &[f64],
    sig: &[SigRow],
    weight: &ErrorrWeight,
    tempin: f64,
) -> Result<Vec<[f64; 4]>, NjoyError> {
    let igx = egn.len() - 1;
    let ipoint = sig.len();
    let mut gsig = vec![[0.0f64; 4]; igx];

    // first point that lies inside some group (label 100/110)
    let mut i0 = 0usize;
    let mut ig;
    loop {
        let e = sig[i0][4];
        ig = (0..igx).find(|&g| e >= egn[g] && e < egn[g + 1]);
        if ig.is_some() {
            break;
        }
        if i0 + 1 < ipoint {
            i0 += 1;
        } else {
            return Err(NjoyError::EndfParse(
                "errorr::rpxgrp: i0>ipoint not coded (no pointwise energy inside the group structure)".into(),
            ));
        }
    }
    let mut ig = ig.unwrap(); // 0-based

    let mut sampler = WeightSampler::new(weight.clone(), tempin); // egtwtf(0)
    let mut x1 = sig[i0][4];
    let (mut wt1, _, _) = sampler.get(x1)?;
    let mut sumde = 0.0f64;

    // loop over all pointwise cross sections in a range (errorr.f90:5273-5359)
    for i in (i0 + 1)..ipoint {
        if ig >= igx {
            break; // upstream would read past egn(ngn+1); no point can follow ehg anyway
        }
        let wt2 = wt1;
        let x2 = x1;
        x1 = sig[i][4];
        let x12 = x1 - x2;
        let egnt = egn[ig];
        let egnt1 = egn[ig + 1];
        let (w, _, _) = sampler.get(x1)?;
        wt1 = w;
        if x1 >= egnt && x1 <= egnt1 {
            let de = 0.5 * (x1 - x2) * (wt1 + wt2);
            sumde += de;
            let z1 = (2.0 * wt1 + wt2) * x12 / 6.0;
            let z2 = (2.0 * wt2 + wt1) * x12 / 6.0;
            for j in 1..4 {
                let y1 = sig[i][j];
                let y2 = sig[i - 1][j];
                if y1 != 0.0 || y2 != 0.0 {
                    gsig[ig][j] += y1 * z1 + y2 * z2;
                }
            }
            if x1 == egnt1 {
                for j in 1..4 {
                    gsig[ig][j] /= sumde;
                }
                ig += 1;
                sumde = 0.0;
            }
        } else if x1 > egnt1 {
            // close the current group at its upper bound (errorr.f90:5305-5328)
            let ebb = egnt1;
            let mut ebx = ebb - x2;
            let wt12 = wt1 - wt2;
            let coef = ebx / x12;
            let wt3 = wt2 + wt12 * ebx / x12;
            let de = ebx * (wt2 + wt3) / 2.0;
            sumde += de;
            let z2 = (2.0 * wt2 + wt3) * ebx / 6.0;
            let z3 = (2.0 * wt3 + wt2) * ebx / 6.0;
            let mut y1 = 0.0;
            let mut y2 = 0.0;
            for j in 1..4 {
                y1 = sig[i][j];
                y2 = sig[i - 1][j];
                let y3 = y2 + (y1 - y2) * coef;
                if y2 != 0.0 || y3 != 0.0 {
                    gsig[ig][j] += y2 * z2 + y3 * z3;
                }
            }
            for j in 1..4 {
                gsig[ig][j] /= sumde;
            }
            // label 1000: whole groups jumped over in one step
            loop {
                ig += 1;
                if ig >= igx {
                    break;
                }
                if x1 > egn[ig + 1] {
                    let xl = egn[ig];
                    let xr = egn[ig + 1];
                    ebx = xr - xl;
                    let wtl = wt2 + wt12 * (xl - x2) / x12;
                    let wtr = wt2 + wt12 * (xr - x2) / x12;
                    sumde = ebx * (wtl + wtr) / 2.0;
                    let zl = (2.0 * wtl + wtr) * ebx / 6.0;
                    let zr = (2.0 * wtr + wtl) * ebx / 6.0;
                    for j in 1..4 {
                        // upstream: y1/y2 are the last j's (capture) values,
                        // and only the second product is divided by sumde.
                        let yl = y2 + (y1 - y2) * (xl - x2) / (x1 - x2);
                        let yr = y2 + (y1 - y2) * (xr - x2) / (x1 - x2);
                        if yl != 0.0 || yr != 0.0 {
                            gsig[ig][j] = yl * zl + yr * zr / sumde;
                        }
                    }
                    continue;
                }
                // open the new group from its lower bound (errorr.f90:5347-5358)
                ebx = x1 - egn[ig];
                let de = ebx * (wt3 + wt1) / 2.0;
                sumde = de;
                let z1 = (2.0 * wt3 + wt1) * ebx / 6.0;
                let z3 = (2.0 * wt1 + wt3) * ebx / 6.0;
                for j in 1..4 {
                    let y1 = sig[i][j];
                    let y2 = sig[i - 1][j];
                    let y3 = y2 + (y1 - y2) * coef;
                    if y3 != 0.0 || y1 != 0.0 {
                        gsig[ig][j] += y1 * z1 + y3 * z3;
                    }
                }
                break;
            }
        }
    }

    // group total cross section (errorr.f90:5362-5364)
    for g in gsig.iter_mut() {
        g[0] = g[1] + g[2] + g[3];
    }
    Ok(gsig)
}
