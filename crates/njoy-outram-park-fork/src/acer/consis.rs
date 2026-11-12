// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher): the part of `subroutine consis` (`acefc.f90:14218-15180`) that changes the table,
// `--check energy distributions`, laws 4 and 44 (`:14400-14700`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! `consis`' one edit to the table in memory (gh:#1369).
//!
//! `acefix` calls `consis` ("basic consistency checks") on the table it has
//! read **before** it writes the edited file and the plot file. Almost all of
//! `consis` only prints to the output listing. One block does not: for a law 4
//! (tabulated) or law 44 (Kalbach-Mann) energy distribution whose outgoing
//! energies exceed the kinematic maximum `epmax` (an incident energy for a
//! laboratory-frame reaction with `q < 0`, the centre-of-mass limit otherwise)
//! it *shifts* the distribution: the points above `epmax` are dropped but one,
//! which becomes `sigfig(epmax,7,-1)`, and the pdf and cdf are renormalised.
//! The edited ACE file and every plot are written from the edited table, so
//! the check file of an `iopt = 7` run is not a copy of its input. The
//! listing text of the other checks is not reproduced (nothing reads it).
//!
//! Found by comparing deck 71 (TENDL-2021 d + Pt-184): NJOY's unit 34 had
//! `4.94590800000E+00` where its own unit 31 had `5.00000000000E+00`.

use crate::mixr::mix::sigfig;

/// What `consis` reads: `NXS`/`JXS` pointers and the header masses.
#[derive(Debug, Clone, Copy)]
pub struct ConsisIndex {
    /// `nr`: the number of reactions with secondary distributions.
    pub nr: i64,
    /// `mtr`.
    pub mtr: i64,
    /// `lqr`.
    pub lqr: i64,
    /// `tyr`.
    pub tyr: i64,
    /// `ldlw`.
    pub ldlw: i64,
    /// `dlw`.
    pub dlw: i64,
    /// `aw0`.
    pub aw0: f64,
    /// `awi`.
    pub awi: f64,
}

struct Mem<'a> {
    x: &'a mut Vec<f64>,
}

impl Mem<'_> {
    fn g(&self, i: i64) -> f64 {
        if i >= 1 && (i as usize) <= self.x.len() {
            self.x[(i - 1) as usize]
        } else {
            0.0
        }
    }

    fn gi(&self, i: i64) -> i64 {
        self.g(i).round() as i64
    }

    fn s(&mut self, i: i64, v: f64) {
        if i >= 1 && (i as usize) <= self.x.len() {
            self.x[(i - 1) as usize] = v;
        }
    }
}

/// Apply `consis`' edits to `xss` (1-based in the Fortran, 0-based here).
///
/// Returns the number of distributions shifted.
pub fn consis_shift_distributions(xss: &mut Vec<f64>, c: &ConsisIndex) -> usize {
    let mut m = Mem { x: xss };
    let mut shifted = 0;
    for n in 1..=c.nr {
        let mt = m.gi(c.mtr + n - 1).abs();
        let aprime = 1.0f64;
        let q = m.g(c.lqr + n - 1);
        let mut nlaw = 1;
        let mut l = nint_sum(&m, c.ldlw + n - 1, c.dlw);
        let mut locl = vec![l];
        while m.gi(l) != 0 {
            l = m.gi(l);
            l = l + c.dlw - 1;
            nlaw += 1;
            locl.push(l);
        }
        let _ = nlaw;
        let icm = if m.gi(c.tyr + n - 1) < 0 { 1 } else { 0 };
        for &loc in &locl {
            let mut l = loc + 1;
            let law = m.gi(l);
            l += 2;
            let mut j = m.gi(l);
            if j != 0 {
                l += 2 * j;
            }
            l += 1;
            j = m.gi(l);
            l = l + 2 * j + 1;
            if law != 4 && law != 44 {
                continue;
            }
            j = m.gi(l);
            if j != 0 {
                l += 2 * j;
            }
            l += 1;
            let ne = m.gi(l);
            for ie in 1..=ne {
                let e = m.g(ie + l);
                let mut loci = nint_sum(&m, ie + ne + l, c.dlw);
                let intt = m.gi(loci);
                let nn = m.gi(loci + 1);
                loci += 1;
                let mut epmax = e;
                if icm == 1 {
                    let d = c.aw0 + c.awi;
                    let t = e.sqrt() - (aprime * e / (d * d)).sqrt();
                    epmax = t * t;
                    epmax = sigfig(epmax, 7, -1);
                }
                let mut n2big = 0i64;
                for j in 1..=nn {
                    let ep = m.g(j + loci);
                    if law == 4 {
                        if ep > epmax && q < 0.0 {
                            n2big += 1;
                        }
                    } else if ep > epmax {
                        if mt != 5 && q < 0.0 {
                            n2big += 1;
                        } else if mt == 5 && c.aw0 < 180.0 {
                            n2big += 1;
                        }
                    }
                }
                if n2big > 0 {
                    shifted += 1;
                    let ishift = n2big - 1;
                    let nnew = nn - ishift;
                    let width: i64 = if law == 4 { 3 } else { 5 };
                    m.s(loci, nnew as f64);
                    m.s(loci + nnew, sigfig(epmax, 7, -1));
                    if intt == 1 {
                        let p = (1.0 - m.g(loci + nnew - 1 + 2 * nn)) / (m.g(loci + nnew) - m.g(loci + nnew - 1));
                        m.s(loci + nnew - 1 + nn, p);
                        m.s(loci + nnew + nn, p);
                    } else {
                        let p = 2.0 * (1.0 - m.g(loci + nnew - 1 + 2 * nn)) / (m.g(loci + nnew) - m.g(loci + nnew - 1))
                            - m.g(loci + nnew - 1 + nn);
                        m.s(loci + nnew + nn, p);
                    }
                    m.s(loci + nnew + 2 * nn, 1.0);
                    for j in (nnew + 1)..=nn {
                        for k in 0..width {
                            m.s(loci + j + k * nn, 0.0);
                        }
                    }
                    if nnew != nn {
                        for i in 1..width {
                            for j in (i * nnew + 1)..=(width * nn) {
                                let v = m.g(loci + j + ishift);
                                m.s(loci + j, v);
                            }
                        }
                        for j in (width * nnew + 1)..=(width * nn) {
                            m.s(loci + j, 0.0);
                        }
                    }
                }
            }
        }
    }
    shifted
}

/// `nint(xss(i) + base - 1)`.
fn nint_sum(m: &Mem, i: i64, base: i64) -> i64 {
    (m.g(i) + base as f64 - 1.0).round() as i64
}
