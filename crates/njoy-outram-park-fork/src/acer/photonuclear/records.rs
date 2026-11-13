// Ported from NJOY2016 `src/acepn.f90` (subroutine `acephn`, lines 27-1853):
// the record handling `acephn` does through its `scr` array, `copynubar`
// (`:3755-3784`) and `skip6` (`endf.f90:1437-1476`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The ENDF records and the XSS buffer [`super::build`] works on (gh:#1369).

use crate::endf::records::{List, SectionCursor, Tab1, Tab2};
use crate::NjoyError;

/// One MF=6 subsection as `acephn` reads it: the multiplicity TAB1 (`ZAP`,
/// `AWP`, `LAW`) and, for laws 1, 2 and 5, the TAB2 and its LIST records.
/// Laws 6 and 7 are read past (`skip6`, `endf.f90:1437-1476`) and kept empty.
#[derive(Debug, Clone)]
pub(super) struct Sub6 {
    pub(super) y: Tab1,
    pub(super) tab2: Option<Tab2>,
    pub(super) lists: Vec<List>,
}

impl Sub6 {
    pub(super) fn izap(&self) -> i32 {
        self.y.head.c1.round() as i32
    }
    pub(super) fn law(&self) -> i32 {
        self.y.head.l2
    }
}

/// An MF=6 section: its HEAD (`LCT = L2`, `NK = N1`) and subsections in order.
#[derive(Debug, Clone)]
pub(super) struct Mf6 {
    pub(super) mt: i32,
    pub(super) lct: i32,
    pub(super) subs: Vec<Sub6>,
}

pub(super) fn read_mf6(mt: i32, rows: &[[f64; 6]]) -> Result<Mf6, NjoyError> {
    let mut cur = SectionCursor::new(rows);
    let h = cur.read_cont()?;
    let mut subs = Vec::new();
    for _ in 0..h.n1.max(0) {
        let y = cur.read_tab1()?;
        let (mut tab2, mut lists) = (None, Vec::new());
        match y.head.l2 {
            1 | 2 | 5 => {
                let t = cur.read_tab2()?;
                for _ in 0..t.head.n2.max(0) {
                    lists.push(cur.read_list()?);
                }
                tab2 = Some(t);
            }
            6 => {
                cur.read_cont()?;
            }
            7 => {
                let t = cur.read_tab2()?;
                for _ in 0..t.head.n2.max(0) {
                    let m = cur.read_tab2()?;
                    for _ in 0..m.head.n2.max(0) {
                        cur.read_tab1()?;
                    }
                }
            }
            _ => {}
        }
        subs.push(Sub6 { y, tab2, lists });
    }
    Ok(Mf6 { mt, lct: h.l2, subs })
}

/// The flat `scr` array `tab1io` leaves, `[c1, c2, l1, l2, nr, np, nbt/int
/// pairs, x/y pairs]`, so upstream's fixed offsets (`scr(7+2*i)`, which
/// assume `NR = 1`) read the same words here.
pub(super) fn flat(t: &Tab1) -> Vec<f64> {
    let mut v = vec![t.head.c1, t.head.c2, f64::from(t.head.l1), f64::from(t.head.l2)];
    v.push(t.interp.len() as f64);
    v.push(t.pairs.len() as f64);
    for &(n, i) in &t.interp {
        v.push(f64::from(n));
        v.push(f64::from(i));
    }
    for &(x, y) in &t.pairs {
        v.push(x);
        v.push(y);
    }
    v
}

/// `copynubar` (`acepn.f90:3755-3784`): the yield's interpolation table and
/// points replaced by the MF=1 nubar's; `ZAP`, `AWP` and `LAW` are kept.
pub(super) fn copynubar(y: &Tab1, nubar: &Tab1) -> Tab1 {
    Tab1 { head: y.head.clone(), interp: nubar.interp.clone(), pairs: nubar.pairs.clone() }
}

/// `terpa` on a TAB1.
pub(super) fn terp(t: &Tab1, x: f64) -> f64 {
    crate::endf::interp::terpa(&t.interp, &t.pairs, x).0
}

/// `terpa` on the lin-lin heating tables `acephn` builds in `scr(llh)`
/// (`[0,0,0,0,1,ne,ne,2]`).
pub(super) fn terp_lin(pairs: &[(f64, f64)], x: f64) -> f64 {
    crate::endf::interp::terpa(&[(pairs.len() as u32, 2)], pairs, x).0
}

/// XSS as upstream holds it: 1-based, zero-filled, written at fixed offsets.
pub(super) struct Xss(pub(super) Vec<f64>);

impl Xss {
    pub(super) fn at(&mut self, k: usize) -> &mut f64 {
        if k >= self.0.len() {
            self.0.resize(k + 1024, 0.0);
        }
        &mut self.0[k]
    }
    pub(super) fn get(&self, k: usize) -> f64 {
        self.0.get(k).copied().unwrap_or(0.0)
    }
    pub(super) fn set(&mut self, k: usize, v: f64) {
        *self.at(k) = v;
    }
}

