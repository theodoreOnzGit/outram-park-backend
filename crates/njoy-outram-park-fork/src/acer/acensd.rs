//! ACER's own MF=4 → AND conversion for the `newfor = 1` format: a faithful
//! port of `ptleg2` and `pttab2` (`acecm.f90:226-434`) and of the
//! tabulated-cosine branch of `acensd` (`acefc.f90:6333-6501`).
//!
//! # Why this exists beside [`super::angular::parse_mf4_angular`]
//!
//! `parse_mf4_angular` linearises a Legendre law with this crate's own adaptive
//! bisection (`ANGLE_TOL = 5e-3`, clamped at zero). That is a sound
//! representation for transport and it stays the ENDF route's parser. It is not
//! what NJOY writes into an ACE table, and a table meant to reproduce NJOY's has
//! to use NJOY's conversion. Measured 2026-09-26 against NJOY2016's own
//! U-234/235/238 tables: every Legendre-derived AND entry differed, and an
//! isotropic energy came out as locator `0` where NJOY writes three cosines.
//!
//! What upstream does, and this module reproduces:
//!
//! - **`newfor = 1` never writes a per-energy isotropic locator.** `acensd`
//!   writes `xss(il+j)=0` only when `n <= 2 .and. newfor /= 1` (`:6411`), so
//!   every energy gets a tabulated table. An isotropic Legendre row (`NL = 0`)
//!   becomes `ptleg2`'s `-1, 0, +1` with a flat density of 1/2.
//! - **`ptleg2`** reconstructs `f(μ)` on a 24-deep stack at `tol1 = 2e-4`
//!   (relative) + `1e-6` (absolute), midpoints rounded to 3 figures, forcing a
//!   split of any panel wider than 0.1 or whose midpoint ratio is outside
//!   `[1/5, 5]`; floors densities at `1e-10`; then thins the table at
//!   `tol2 = 2e-3` over windows no wider than 1 in cosine.
//! - **`pttab2`** takes an MF=4 `LTT = 2` table **as tabulated** — no
//!   linearisation, whatever its `INT` — and renormalises it by its trapezoid
//!   area.
//! - **`acensd`** rounds: energy `sigfig(E/1e6, 7)`, cosine and density
//!   `sigfig(·, 7)` (densities below `1e-30` become 0), the CDF accumulated
//!   from the **rounded** values at `sigfig(·, 7)`, then renormalised by its
//!   last entry and rounded at `sigfig(·, 9)`.
//! - `LTT = 3` keeps both series, Legendre first, **without sorting or
//!   de-duplicating** the shared boundary energy (`acensd`'s `ltt3` shuffle,
//!   `:6451-6480`).
//! - `LTT = 0` or `LI = 1` is whole-reaction isotropic: no energies, LAND
//!   locator `0` (`acefc.f90:5872-5875`).

use super::angular::{ElasticAngular, EnergyAngular};
use crate::endf::{records::SectionCursor, tape::Section};
use crate::NjoyError;
use crate::groupr::kinematics::shared::legndr;
use crate::mixr::mix::sigfig;

/// `ptleg2`'s adaptive-stack depth, `imax` (`acecm.f90:241`).
const IMAX: usize = 24;
/// `ptleg2`'s reconstruction tolerance, `tol1` (`acecm.f90:250`).
const TOL1: f64 = 0.0002;
/// `ptleg2`'s thinning tolerance, `tol2` (`acecm.f90:251`).
const TOL2: f64 = 0.002;
/// `ptleg2`'s density floor, `pmin` (`acecm.f90:252`).
const PMIN: f64 = 1.0e-10;
/// `ptleg2`'s point cap, `maxang` (`acecm.f90:240`).
const MAXANG: usize = 7000;
/// `ptleg2`'s Legendre-order cap, `ipmax - 1` (`acecm.f90:242`).
const MAX_ORDER: usize = 64;
/// `acensd`'s density floor, `rmin` (`acefc.f90:6349`).
const RMIN: f64 = 1.0e-30;

/// Port of `ptleg2` (`acecm.f90:226-394`): Legendre coefficients `a_1 … a_NL`
/// (normalised, `a_0 ≡ 1` implicit) to a tabulated `(μ, f(μ))` law whose
/// trapezoid area is 1. An empty slice is `NL = 0`, i.e. isotropic.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `NL` exceeds upstream's `ipmax` or the
/// reconstruction exceeds `maxang` points — both `error` calls upstream.
pub fn ptleg2(coeffs: &[f64]) -> Result<(Vec<f64>, Vec<f64>), NjoyError> {
    if coeffs.len() > MAX_ORDER {
        return Err(NjoyError::EndfParse(format!(
            "ptleg2: Legendre order {} exceeds upstream's ipmax - 1 = {MAX_ORDER}",
            coeffs.len()
        )));
    }
    // `nord = 0` is rewritten as one zero coefficient (`:264-267`).
    let fl: Vec<f64> = if coeffs.is_empty() { vec![0.0] } else { coeffs.to_vec() };
    let nord = fl.len();
    let density = |x: f64| -> f64 {
        let p = legndr(x, nord);
        let mut y = 0.5;
        for j in 1..=nord {
            y += 0.5 * (2 * j + 1) as f64 * fl[j - 1] * p[j];
        }
        y
    };

    // 1-based arrays, as upstream, so the index arithmetic reads across.
    let mut x = [0.0_f64; IMAX + 1];
    let mut y = [0.0_f64; IMAX + 1];
    let mut aco = vec![0.0_f64; 1];
    let mut cprob = vec![0.0_f64; 1];
    let mut cumm = vec![0.0_f64; 1];

    // Prime the stack: top of stack is μ = -1, below it μ = +1 (`:276-289`).
    let mut i = 2usize;
    x[2] = -1.0;
    y[2] = density(x[2]);
    x[1] = 1.0;
    y[1] = density(x[1]);
    // `test`, `xm`, `yt` persist across iterations as the Fortran locals do;
    // `test` is only ever read after it has been set, or against `dy = 0`.
    let (mut test, mut xm, mut yt) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut xl, mut yl) = (0.0_f64, 0.0_f64);
    while i > 0 {
        let mut dy = 0.0;
        if i > 1 && i < IMAX {
            let dm = x[i - 1] - x[i];
            xm = sigfig(0.5 * (x[i - 1] + x[i]), 3, 0);
            if xm > x[i] && xm < x[i - 1] {
                let ym = 0.5 * (y[i - 1] + y[i]);
                yt = density(xm);
                test = TOL1 * yt.abs() + 1.0 / 1_000_000.0;
                dy = (yt - ym).abs();
                if dm > 0.1 {
                    dy = 2.0 * test;
                }
                if ym != 0.0 && yt / ym > 5.0 {
                    dy = 2.0 * test;
                }
                if ym != 0.0 && yt / ym < 1.0 / 5.0 {
                    dy = 2.0 * test;
                }
            }
        }
        if dy > test {
            // Not converged: push the midpoint.
            i += 1;
            x[i] = x[i - 1];
            y[i] = y[i - 1];
            x[i - 1] = xm;
            y[i - 1] = yt;
        } else {
            // Converged: pop the top point into the table.
            if aco.len() > MAXANG {
                return Err(NjoyError::EndfParse("ptleg2: too many angles".into()));
            }
            if y[i] < PMIN {
                y[i] = PMIN;
            }
            let ii = aco.len();
            aco.push(x[i]);
            cprob.push(y[i]);
            let c = if ii > 1 { cumm[ii - 1] + 0.5 * (x[i] - xl) * (y[i] + yl) } else { 0.0 };
            cumm.push(c);
            xl = x[i];
            yl = y[i];
            i -= 1;
        }
    }
    let nn = aco.len() - 2; // upstream `nn = ii - 1`

    // Thin to the coarser tolerance (`:340-381`). Writes land at `ii <= i`,
    // reads at indices `>= i`, so separate output arrays are equivalent to
    // upstream's in-place update.
    let mut oa = vec![0.0_f64, -1.0];
    let mut oc = vec![0.0_f64, cprob[1]];
    let mut ocum = vec![0.0_f64, cumm[1]];
    let mut i = 1usize;
    let mut idone = false;
    while (i as i64) < nn as i64 - 1 && !idone {
        let mut check = 0.0_f64;
        let mut dco = 0.0_f64;
        let mut j = i + 1;
        let mut jj = j;
        while j < nn + 1 && check <= 0.0 && dco <= 1.0 {
            j += 1;
            jj = j - 1;
            dco = aco[j] - aco[i];
            if dco <= 1.0 {
                let mut k = i;
                while k < j - 1 && check <= 0.0 {
                    k += 1;
                    let f = (aco[j] - aco[k]) / dco;
                    let t = f * cprob[i] + (1.0 - f) * cprob[j];
                    let diff = 1.0 / 10_000_000.0 + TOL2 * cprob[k];
                    check = (t - cprob[k]).abs() - diff;
                }
            }
        }
        if check > 0.0 || dco > 1.0 {
            i = jj;
            push_thinned(&mut oa, &mut oc, &mut ocum, aco[i], cprob[i]);
        } else {
            idone = true;
        }
    }
    push_thinned(&mut oa, &mut oc, &mut ocum, aco[nn + 1], cprob[nn + 1]);

    let total = *ocum.last().unwrap();
    let pdf = oc[1..].iter().map(|&p| p / total).collect();
    Ok((oa[1..].to_vec(), pdf))
}

fn push_thinned(oa: &mut Vec<f64>, oc: &mut Vec<f64>, ocum: &mut Vec<f64>, a: f64, c: f64) {
    let ii = oa.len();
    oa.push(a);
    oc.push(c);
    let prev = ocum[ii - 1];
    ocum.push(prev + 0.5 * (oa[ii] - oa[ii - 1]) * (oc[ii] + oc[ii - 1]));
}

/// Port of `pttab2` (`acecm.f90:396-434`): renormalise a tabulated `(μ, f)`
/// law by its trapezoid area. The table is taken as given; `INT` is not
/// consulted, exactly as upstream.
pub fn pttab2(pairs: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
    let mut cumm = 0.0;
    for w in pairs.windows(2) {
        cumm += (w[1].0 - w[0].0) * (w[1].1 + w[0].1) / 2.0;
    }
    (
        pairs.iter().map(|&(m, _)| m).collect(),
        pairs.iter().map(|&(_, p)| p / cumm).collect(),
    )
}

/// `acensd`'s `newfor = 1` rounding of one `(μ, pdf)` table into the words
/// NJOY stores (`acefc.f90:6419-6443`).
fn acensd_entry(e_ev: f64, mu: &[f64], p: &[f64]) -> EnergyAngular {
    let n = mu.len();
    let cosines: Vec<f64> = mu.iter().map(|&m| sigfig(m, 7, 0)).collect();
    let pdf: Vec<f64> = p
        .iter()
        .map(|&v| {
            let r = sigfig(v, 7, 0);
            if r < RMIN {
                0.0
            } else {
                r
            }
        })
        .collect();
    let mut cdf = vec![0.0_f64; n];
    for i in 1..n {
        let sum = cdf[i - 1] + (pdf[i] + pdf[i - 1]) * (cosines[i] - cosines[i - 1]) / 2.0;
        cdf[i] = sigfig(sum, 7, 0);
    }
    let renorm = 1.0 / cdf[n - 1];
    for c in &mut cdf {
        *c = sigfig(renorm * *c, 9, 0);
    }
    EnergyAngular {
        e_mev: sigfig(e_ev / 1.0e6, 7, 0),
        cosines,
        pdf,
        cdf,
    }
}

/// Read an MF=4 section the way ACER does for `newfor = 1`: `ptleg2` on the
/// Legendre series, `pttab2` on the tabulated one, `acensd`'s rounding on both.
/// The returned values are the exact AND words NJOY would store.
///
/// # Errors
/// Propagates record-structure errors and `ptleg2`'s upstream limits.
pub fn parse_mf4_for_acer(section: &Section) -> Result<ElasticAngular, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, LVT, LTT, 0, 0
    let ltt = head.l2;
    let trans = cur.read_cont()?; // 0, AWR, LI, LCT, 0, 0
    let li = trans.l1;
    let lct = trans.l2;
    if ltt == 0 || li == 1 {
        return Ok(ElasticAngular {
            energies: Vec::new(),
            lct,
        });
    }
    let mut energies = Vec::new();
    if ltt == 1 || ltt == 3 {
        let tab2 = cur.read_tab2()?;
        for _ in 0..tab2.head.n2 {
            let list = cur.read_list()?;
            let (mu, p) = ptleg2(&list.data)?;
            energies.push(acensd_entry(list.head.c2, &mu, &p));
        }
    }
    if ltt == 2 || ltt == 3 {
        let tab2 = cur.read_tab2()?;
        for _ in 0..tab2.head.n2 {
            let tab1 = cur.read_tab1()?;
            let (mu, p) = pttab2(&tab1.pairs);
            energies.push(acensd_entry(tab1.head.c2, &mu, &p));
        }
    }
    Ok(ElasticAngular { energies, lct })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An isotropic row is `-1, 0, +1` at density 1/2: the three cosines NJOY
    /// writes where this crate used to write locator 0.
    #[test]
    fn isotropic_row_is_three_points() {
        let (mu, p) = ptleg2(&[]).unwrap();
        assert_eq!(mu, vec![-1.0, 0.0, 1.0]);
        assert!(p.iter().all(|&v| (v - 0.5).abs() < 1e-15));
        let e = acensd_entry(1.0e-5, &mu, &p);
        // `sigfig` carries upstream's 1.0000000000001 bias, so compare as a
        // Type-1 file prints it.
        let printed: Vec<String> = e.cdf.iter().map(|c| format!("{c:.11e}")).collect();
        assert_eq!(printed, ["0.00000000000e0", "5.00000000000e-1", "1.00000000000e0"]);
        assert_eq!(e.e_mev, sigfig(1.0e-11, 7, 0));
    }

    /// The table integrates to 1 and reproduces the Legendre density within
    /// `tol2` at its own nodes, for a strongly forward-peaked law.
    #[test]
    fn forward_peaked_law_is_normalised_and_accurate() {
        let a = [0.6, 0.3, 0.1];
        let (mu, p) = ptleg2(&a).unwrap();
        let area: f64 = mu.windows(2).zip(p.windows(2)).map(|(m, q)| 0.5 * (m[1] - m[0]) * (q[1] + q[0])).sum();
        assert!((area - 1.0).abs() < 1e-12, "area {area}");
        assert_eq!(mu[0], -1.0);
        assert_eq!(*mu.last().unwrap(), 1.0);
        assert!(mu.windows(2).all(|w| w[1] > w[0]));
    }
}
