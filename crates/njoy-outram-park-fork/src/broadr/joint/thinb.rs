// SPDX-License-Identifier: AGPL-3.0-only
// Ported from NJOY2016 `src/broadr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine bfile3`, l.1152-1253, its `tempef <= 0` branch (l.1218-1222);
//   - `subroutine thinb`, l.1677-1760;
//   - `subroutine broadr`, l.354-562 and l.935-1000, as in the parent module.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **BROADR at `tempef <= 0`: thinning only** (`thinb`), gh:#619, gh:#1369 deck 43.
//!
//! When the target temperature is not above the input's, `bfile3` calls `thinb`
//! instead of `broadn` (`broadr.f90:1218-1222`): no broadening, only the removal of
//! points that linear interpolation from the last kept point reproduces within
//! `errthn` for every reaction at once, below `thnmax`; above it every point is
//! copied. The reaction selection, `thnmax`, the table and the output assembly are
//! the parent module's [`super::broadr_joint_rounding`], repeated here so that
//! function stays as it was (the port is the oracle, `AGENTS.md`).
//!
//! **Two places upstream's behaviour is not defined, refused here by name:**
//! - `thinb`'s `tt` is a local array without `save` (`:1691`). On every page after
//!   the first (`klow = 1`) it is read before it is written (`:1712-1717`), so a
//!   table longer than one page (`mpage = namax/(4(nreac+1))` points) thins from
//!   whatever that stack slot holds. A table that long is refused.
//! - When the walk ends below `thnmax` (every point up to the last thinnable), the
//!   last point is never written and `bfile3`'s paging does not end (`nj` never
//!   turns negative, `:1224`). Refused too.

use crate::mixr::mix::sigfig;
use crate::reconr::{ReconrResult, ReconrSection};
use crate::NjoyError;

use super::{section_reader, skipped, BroadnTolerances, NAMAX};

/// `thinb` on one page holding the whole table (`broadr.f90:1677-1760`, entered
/// from `bfile3` with `klow = nlow`, `khigh = nhigh`). `e` are energies, `s[k*nreac+i]`.
/// Returns the kept rows `(e, s(1..nreac))`, or `None` when the walk never
/// reached `thnmax` (upstream: the last point is not written, see the module doc).
fn thinb_one_page(e: &[f64], s: &[f64], nreac: usize, errthn: f64, thnmax: f64) -> Option<Vec<Vec<f64>>> {
    const STPMAX: f64 = 1.24;
    const EPS: f64 = 1.0e-5;
    let nhigh = e.len() - 1; // 0-based index of the last point
    let mut out: Vec<Vec<f64>> = Vec::new();
    let row = |k: usize| -> Vec<f64> {
        let mut r = Vec::with_capacity(1 + nreac);
        r.push(e[k]);
        r.extend_from_slice(&s[k * nreac..(k + 1) * nreac]);
        r
    };
    // keep first point (`klow /= 1`)
    let mut tt = row(0);
    out.push(tt.clone());
    let mut ilow = 1usize;
    // n = nmin..nhigh, 0-based 2..=nhigh
    let mut n = 2usize;
    while n <= nhigh {
        let nm1 = n - 1;
        let denom = e[n] - tt[0];
        if denom == 0.0 {
            n += 1;
            continue;
        }
        let denom = 1.0 / denom;
        let mut fails = false;
        'reac: for i in 0..nreac {
            let slope = (s[n * nreac + i] - tt[1 + i]) * denom;
            for k in ilow..=nm1 {
                if e[k] >= STPMAX * tt[0] {
                    fails = true;
                    break 'reac;
                }
                let sp = tt[1 + i] + slope * (e[k] - tt[0]);
                if (sp - s[k * nreac + i]).abs() > errthn * s[k * nreac + i] {
                    fails = true;
                    break 'reac;
                }
                if e[k] * (1.0 + EPS) > thnmax {
                    fails = true;
                    break 'reac;
                }
            }
        }
        if !fails {
            n += 1;
            continue;
        }
        // label 140: save the last point within the interval
        tt = row(nm1);
        if tt[0] * (1.0 + EPS) >= thnmax {
            // label 180/160: copy every point from nm1 to the end
            for k in nm1..=nhigh {
                out.push(row(k));
            }
            return Some(out);
        }
        out.push(tt.clone());
        ilow = n;
        n += 1;
    }
    None
}

/// [`super::broadr_joint`] at `tempef = temp_k - T_in <= 0`: BROADR's thinning-only
/// pass (`thinb`), gh:#619. `Ok(None)` when there is nothing to walk (no MT=1 or
/// fewer than two points), as [`super::broadr_joint`] returns; `Ok(Some)` otherwise.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for `tempef > 0` (use [`super::broadr_joint`]);
/// [`NjoyError::NotPorted`] for the two undefined cases in the module doc (a table
/// longer than one page; a walk that ends below `thnmax`).
#[allow(clippy::too_many_lines)]
pub fn broadr_joint_thinb(
    result: &ReconrResult,
    temp_k: f64,
    tol: &BroadnTolerances,
    thnmx: f64,
) -> Result<Option<ReconrResult>, NjoyError> {
    const E6PT5: f64 = 6.5e6;
    const FACT: f64 = 0.99999;
    let secs = &result.sections;
    let Some(mt1) = secs.iter().find(|s| s.mt.number() == 1) else { return Ok(None) };
    let tempef = temp_k - result.temperature_k;
    if tempef > 0.0 {
        return Err(NjoyError::InvalidInput("broadr_joint_thinb: tempef > 0 broadens (broadr_joint)".into()));
    }
    if mt1.pairs.len() < 2 {
        return Ok(None);
    }
    let lrp = result.material.lrp;

    // thnmax and emin (broadr.f90:354-441), as in `broadr_joint_rounding`.
    let thnmx = sigfig(thnmx, 7, 0);
    let mut eresh = thnmx;
    if let Some(eh) = result.resonance_upper_limit {
        eresh = eh;
    }
    if eresh > E6PT5 {
        eresh = E6PT5;
    }
    let mut thnmax = if thnmx == 0.0 {
        eresh
    } else if thnmx > 0.0 {
        thnmx.min(eresh)
    } else {
        thnmx
    };
    let mut emin = 1.0;
    if thnmx < 0.0 {
        emin = -thnmx;
    }
    if lrp == 1 {
        emin = eresh;
    }
    let emax = result.material.emax;
    if emax > 0.0 && emax < thnmax {
        thnmax = emax;
    }
    let grid: Vec<f64> = mt1.pairs.iter().map(|p| p.0).collect();

    // Reaction selection (broadr.f90:497-562).
    let mut mtr: Vec<usize> = Vec::new();
    for (idx, sec) in secs.iter().enumerate() {
        let mt = i32::from(sec.mt.number());
        if mt == 1 || skipped(mt) || sec.pairs.is_empty() {
            continue;
        }
        let enext = section_reader(sec).get(0.0).xnext;
        if mt == 18 || enext <= emin || (lrp == 1 && enext < eresh) {
            mtr.push(idx);
            continue;
        }
        if lrp == 0 && FACT * enext < thnmax {
            thnmax = FACT * enext;
        }
    }
    if thnmax == 0.0 && thnmx == 0.0 {
        thnmax = E6PT5;
    }
    if thnmax < 0.0 {
        thnmax = -thnmax;
    }
    let nreac = mtr.len();
    if nreac == 0 {
        let mut copy = result.clone();
        copy.temperature_k = temp_k;
        return Ok(Some(copy));
    }
    let mpage = NAMAX / (4 * (nreac + 1));
    if grid.len() >= mpage {
        return Err(NjoyError::NotPorted(
            "BROADR thinb over more than one page (upstream reads thinb's unsaved tt there)",
        ));
    }
    let mut sig = vec![0.0f64; grid.len() * nreac];
    for (i, &idx) in mtr.iter().enumerate() {
        let mut g = section_reader(&secs[idx]);
        let _ = g.get(0.0);
        for (k, &e) in grid.iter().enumerate() {
            sig[k * nreac + i] = g.get(e).y;
        }
    }
    let rows = thinb_one_page(&grid, &sig, nreac, tol.errthn, thnmax).ok_or(NjoyError::NotPorted(
        "BROADR thinb whose walk ends below thnmax (upstream writes no last point and bfile3 does not end)",
    ))?;
    let mt_of = |i: usize| i32::from(secs[mtr[i]].mt.number());
    let n2out = rows.len();

    // Output (labels 245-310), as in `broadr_joint_rounding`.
    let mti: Vec<usize> = (0..nreac)
        .map(|i| rows.iter().position(|r| r[1 + i] > 0.0).map_or(1, |p| p + 1))
        .collect();
    let mt4br = (0..nreac).any(|i| (51..=91).contains(&mt_of(i)));
    let has = |m: i32| (0..nreac).any(|i| mt_of(i) == m);
    let (mt103, mt104, mt105, mt106, mt107) = (has(103), has(104), has(105), has(106), has(107));
    let mut out_secs = Vec::with_capacity(secs.len());
    for sec in secs {
        let mth = i32::from(sec.mt.number());
        let summed = mth == 1 || mth == 3 || (mth == 4 && mt4br) || (46..=49).contains(&mth) || mth == 19;
        let ireac = if summed { None } else { (0..nreac).rev().find(|&i| mt_of(i) == mth) };
        if !summed && ireac.is_none() {
            out_secs.push(sec.clone());
            continue;
        }
        let mut jjj = 1usize;
        for i in 0..nreac {
            if mth == mt_of(i) {
                jjj = mti[i];
            }
            if mth == 4 && mt_of(i) == 51 {
                jjj = mti[i];
            }
            if mth == 4 && jjj == 1 && mt_of(i) == 91 {
                jjj = mti[i];
            }
        }
        if jjj > 1 {
            jjj -= 1;
        }
        let mut orig = section_reader(sec);
        let _ = orig.get(0.0);
        let mut pairs = Vec::with_capacity(n2out + 1 - jjj);
        for r in &rows[jjj - 1..] {
            let e = r[0];
            let v = if let Some(i) = ireac {
                r[1 + i]
            } else if e > thnmax {
                orig.get(e).y
            } else {
                let mut acc = 0.0;
                for i in 0..nreac {
                    let m = mt_of(i);
                    let iflag = (mth == 3 && m == 2)
                        || (mth == 4 && !(51..=91).contains(&m))
                        || (mth == 19 && m != 18)
                        || ((46..=49).contains(&mth) && m != mth - 40)
                        || mth >= 201
                        || (mt103 && (600..=649).contains(&m))
                        || (mt104 && (650..=699).contains(&m))
                        || (mt105 && (700..=749).contains(&m))
                        || (mt106 && (750..=799).contains(&m))
                        || (mt107 && (800..=849).contains(&m));
                    if !iflag {
                        acc += r[1 + i];
                    }
                }
                acc
            };
            pairs.push((e, sigfig(v, 7, 0)));
        }
        out_secs.push(ReconrSection { lr: sec.lr, mt: sec.mt, qi: sec.qi, pairs });
    }
    Ok(Some(ReconrResult {
        material: result.material.clone(),
        sections: out_secs,
        resonance_upper_limit: result.resonance_upper_limit,
        unresolved_table: result.unresolved_table.clone(),
        temperature_k: temp_k,
    }))
}

#[cfg(test)]
mod tests {
    use super::thinb_one_page;

    /// A straight line thins to its two ends below `thnmax`, and every point at or
    /// above `thnmax` is copied (`broadr.f90:1730-1758`).
    #[test]
    fn a_line_thins_to_its_ends_and_the_top_is_copied() {
        let e: Vec<f64> = (1..=10).map(|k| f64::from(k)).collect();
        let s: Vec<f64> = e.iter().map(|&x| 2.0 * x).collect();
        let rows = thinb_one_page(&e, &s, 1, 1.0e-3, 8.5).expect("walk reaches thnmax");
        let kept: Vec<f64> = rows.iter().map(|r| r[0]).collect();
        // Worked by hand from `thinb`: `stpmax = 1.24` forces a node whenever an
        // intervening e(k) >= 1.24 * the last kept energy (so 2, 3, 4, 5 and 7 are
        // kept on a straight line), and the first node at or above thnmax/(1+eps)
        // (9) starts the copy of the rest.
        assert_eq!(kept, vec![1.0, 2.0, 3.0, 4.0, 5.0, 7.0, 9.0, 10.0]);
    }

    /// A walk that never reaches `thnmax` is upstream's endless paging: refused.
    #[test]
    fn a_walk_below_thnmax_is_refused() {
        let e = [1.0, 1.1, 1.2];
        let s = [1.0, 1.0, 1.0];
        assert!(thinb_one_page(&e, &s, 1, 1.0e-3, 1.0e6).is_none());
    }
}
