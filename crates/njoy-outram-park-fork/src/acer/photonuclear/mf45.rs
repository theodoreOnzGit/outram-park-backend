// Ported from NJOY2016.80 `src/acepn.f90` (subroutine `acephn`): the MF=4/MF=5
// neutron branches, l.348-366 (pass 1), l.742-833 (production cross section and
// yield), l.1152-1212 (MF=4 discrete levels, ACE law 33) and l.1214-1425
// (MF=5 energy distributions).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `acephn`'s MF=4/MF=5 representation of emitted neutrons (gh:#1369, deck 56).
//!
//! Upstream reads MF=4 and MF=5 "for secondary neutrons only" (`:345-347`):
//! - an MF=4 section counts one neutron-producing reaction, isotropic in the
//!   ACE table (`LANDP = 0`, `:956-962`); its own angular data are never read;
//! - its production cross section is `y * sigma`, with `y` from the MF=1 nubar
//!   for MT=18 and from a built-in multiplicity table otherwise (`:763-831`);
//! - MT=50-90 with MF=4 get an ACE law 33 discrete-level distribution
//!   (`:1166-1212`), and an MF=5 section an ACE law 4 (`LF = 1`) or law 7/9
//!   (`LF = 7`/`9`) distribution (`:1214-1425`).
//!
//! **The multiplicity is keyed on `mth`, not `mt` (upstream behaviour, kept).**
//! In the MF=4 yield branch (`:787-808`) the table is indexed by `mth`, the
//! `endf` module's MT of the **last record read**, while the reaction is `mt`.
//! At that point the last read was pass 1's loop ending on the MEND record
//! (`do while (math.gt.0)`, `:336`), so `mth = 0` and every MF=4 reaction other
//! than MT=18 gets `y = 1`, an (n,2n) included. Neutrons are always the first
//! production type, so nothing reads the tape in between. [`mf4_yields`] passes
//! that 0 ([`MTH_AFTER_PASS_1`]). In the MF=5 `LF = 7/9` heating (`:1360-1392`)
//! the same table is indexed by `mth` too, but there `findf`/`contio` have just
//! read MF=5/MT=`mt`'s own records, so `mth = mt`.
//!
//! **`erfc(x)`, not `erfc(sqrt(x))`** (`:1395`): the `LF = 7` mean energy's
//! normalisation passes `x` itself to `erfc`, kept as upstream writes it.

use super::records::{terp, Xss};
use crate::broadr::joint::erfc;
use crate::endf::records::{SectionCursor, Tab1};
use crate::mixr::mix::sigfig;
use crate::NjoyError;
use petir::mathf::RealMath;

const EMEV: f64 = 1.0e6;
const RMIN: f64 = 1.0e-30;

/// `mth` when the MF=4 yield branch reads it: the MEND record that ended
/// pass 1 (`acepn.f90:336`).
pub(super) const MTH_AFTER_PASS_1: i32 = 0;

/// The built-in neutron multiplicity of reaction `mth` (`acepn.f90:788-808`,
/// repeated at `:1366-1389`): 2 for (n,2n)-like MTs, 3 for (n,3n)-like, and so
/// on to 8; 1 otherwise.
pub(super) fn multiplicity(mth: i32) -> f64 {
    let m = mth;
    if m == 11 || m == 16 || m == 24 || m == 30 || m == 41 || m == 154 || m == 159 || m == 176 || m == 190 || (875..=891).contains(&m) {
        2.0
    } else if m == 17 || m == 25 || m == 42 || m == 157 || m == 172 || m == 177 || m == 179 || (181..=199).contains(&m) {
        3.0
    } else if m == 37 || m == 156 || m == 165 || m == 169 || m == 173 || m == 178 || (194..=196).contains(&m) {
        4.0
    } else if m == 152 || m == 162 || m == 166 || m == 170 || m == 174 || m == 200 {
        5.0
    } else if m == 153 || m == 163 || m == 167 || m == 171 || m == 175 {
        6.0
    } else if m == 160 || m == 164 || m == 168 {
        7.0
    } else if m == 161 {
        8.0
    } else {
        1.0
    }
}

/// The XSS offsets of one production block that the MF=4/5 branches write.
pub(super) struct Block {
    pub(super) esz: usize,
    pub(super) tot: usize,
    pub(super) thn: usize,
    pub(super) nes: usize,
    pub(super) it: usize,
    pub(super) pxs: usize,
    pub(super) phn: usize,
    pub(super) mtrp: usize,
    pub(super) tyrp: usize,
    pub(super) lsigp: usize,
    pub(super) sigp: usize,
    pub(super) ldlwp: usize,
    pub(super) dlwp: usize,
}

/// One MF=3 reaction as the `do while (mtt.ne.mt)` searches find it: its MT,
/// Q (MeV), the `k` of its SIG block and its first energy index `iaa`.
#[derive(Clone, Copy)]
pub(super) struct Reaction {
    pub(super) mt: i32,
    pub(super) q: f64,
    pub(super) k: usize,
    pub(super) iaa: usize,
}

/// `acepn.f90:742-833`: for each MF=4 reaction `mts` (dictionary order), the
/// MTRP/TYRP/LSIGP entries, `y*sigma` into PXS, `e+q` heating into THN and the
/// yield record (MF=12 form). `jp` and `nex` are upstream's counters.
pub(super) fn mf4_yields(x: &mut Xss, b: &Block, rxs: &[Reaction], fnubar: Option<&Tab1>, jp: &mut usize, nex: &mut usize) -> Result<(), NjoyError> {
    for rx in rxs {
        *jp += 1;
        let (mt, q, k, iaa) = (rx.mt, rx.q, rx.k, rx.iaa);
        x.set(b.mtrp + *jp - 1, f64::from(mt));
        x.set(b.tyrp + *jp - 1, 1.0);
        x.set(b.lsigp + *jp - 1, (*nex - b.sigp + 1) as f64);
        let heat = |x: &mut Xss, j: usize, ss: f64| {
            let e = x.get(b.esz + j - 1);
            if x.get(b.tot + j - 1) != 0.0 {
                let v = x.get(b.thn + j - 1) + ss * (e / EMEV + q) / x.get(b.tot + j - 1);
                x.set(b.thn + j - 1, v);
            }
        };
        if mt == 18 {
            let nu = fnubar.ok_or_else(|| {
                NjoyError::EndfParse("acephn: MF=4/MT=18 needs the MF=1 nubar (mt452 or mt456), and the tape has none".into())
            })?;
            for j in iaa..=b.nes {
                let e = x.get(b.esz + j - 1);
                let y = terp(nu, e);
                let ss = x.get(2 + k + j - iaa);
                let tt = x.get(b.pxs + 2 + j - b.it) + y * ss;
                x.set(b.pxs + 2 + j - b.it, sigfig(tt, 7, 0));
                heat(x, j, ss);
            }
            let ne = nu.pairs.len();
            x.set(*nex, 12.0);
            x.set(*nex + 1, f64::from(mt));
            x.set(*nex + 2, 0.0);
            x.set(*nex + 3, ne as f64);
            for (j, &(e, v)) in nu.pairs.iter().enumerate() {
                x.set(*nex + 4 + j, sigfig(e / EMEV, 7, 0));
                x.set(*nex + 4 + ne + j, sigfig(v, 7, 0));
            }
            *nex += 4 + 2 * ne;
        } else {
            // Keyed on the leftover `mth`, not `mt`: see the module doc.
            let y = multiplicity(MTH_AFTER_PASS_1);
            for j in iaa..=b.nes {
                let ss = x.get(2 + k + j - iaa);
                let tt = x.get(b.pxs + 2 + j - b.it) + y * ss;
                x.set(b.pxs + 2 + j - b.it, sigfig(tt, 7, 0));
                heat(x, j, ss);
            }
            x.set(*nex, 12.0);
            x.set(*nex + 1, f64::from(mt));
            x.set(*nex + 2, 0.0);
            x.set(*nex + 3, 2.0);
            x.set(*nex + 4, sigfig(x.get(b.esz + iaa - 1) / EMEV, 7, 0));
            x.set(*nex + 5, sigfig(x.get(b.esz + b.nes - 1) / EMEV, 7, 0));
            x.set(*nex + 6, y);
            x.set(*nex + 7, y);
            *nex += 8;
        }
    }
    Ok(())
}

/// `acepn.f90:1166-1212`: reaction `rx` (an MF=4 MT=50-90) at slot `jp` gets
/// an ACE law 33 discrete-level distribution, and its neutron and local
/// heating.
pub(super) fn law33_discrete(x: &mut Xss, b: &Block, rx: Reaction, awr: f64, jp: usize, nex: &mut usize) {
    let (q, k, iaa) = (rx.q, rx.k, rx.iaa);
    x.set(b.ldlwp + jp - 1, (*nex - b.dlwp + 1) as f64);
    let last = *nex;
    x.set(*nex, 0.0);
    x.set(*nex + 1, 33.0);
    *nex += 3;
    x.set(*nex, 0.0);
    x.set(*nex + 1, 2.0);
    x.set(*nex + 2, sigfig(x.get(b.esz + iaa - 1) / EMEV, 7, 0));
    x.set(*nex + 3, sigfig(x.get(b.esz + b.nes - 1) / EMEV, 7, 0));
    x.set(*nex + 4, 1.0);
    x.set(*nex + 5, 1.0);
    *nex += 2 + 2 * 2;
    x.set(last + 2, (*nex - b.dlwp + 1) as f64);
    // "MF4 is for outgoing neutrons".
    let awp = 1.0f64;
    x.set(*nex, sigfig(-q, 7, 0));
    x.set(*nex + 1, sigfig((awr - awp) / awr, 7, 0));
    *nex += 2;
    for j in iaa..=b.nes {
        let e = x.get(b.esz + j - 1) / EMEV;
        let ebar = awr * (e - q.abs()) / (1.0 + awr);
        let s = x.get(2 + k + j - iaa);
        let hh = ebar * s;
        let v = x.get(b.phn + 2 + j - b.it) + hh;
        x.set(b.phn + 2 + j - b.it, v);
        let hh = (e - q.abs()) * s - hh;
        if x.get(b.tot + j - 1) != 0.0 {
            let v = x.get(b.thn + j - 1) + hh / x.get(b.tot + j - 1);
            x.set(b.thn + j - 1, v);
        }
    }
}

/// The interpolation table of a TAB1 as `acephn` stores it: `NR, NBT, INT`
/// when `NR /= 1` or the law is not lin-lin, else a single 0.
fn store_interp(x: &mut Xss, at: usize, t: &Tab1) -> usize {
    let m = t.interp.len();
    let jnt = t.interp.first().map_or(0, |r| r.1);
    if m != 1 || jnt != 2 {
        x.set(at, m as f64);
        for (j, &(nbt, int)) in t.interp.iter().enumerate() {
            x.set(at + 1 + j, f64::from(nbt));
            x.set(at + 1 + m + j, f64::from(int));
        }
        at + 2 * m + 1
    } else {
        x.set(at, 0.0);
        at + 1
    }
}

/// `acepn.f90:1214-1425`: the MF=5 section `rows` of reaction `rx`, at slot
/// `jp`: one DLW law per subsection (`LF = 1` → law 4, `LF = 7/9` → law 7/9),
/// chained through `LNW`, and for `LF = 7/9` the mean emitted energy taken out
/// of the local heating into the neutron's.
///
/// # Errors
/// `InvalidInput` for any other `LF` (upstream: "file 5 law not ready").
#[allow(clippy::too_many_arguments)]
pub(super) fn mf5_distribution(x: &mut Xss, b: &Block, rx: Reaction, rows: &[[f64; 6]], fnubar: Option<&Tab1>, jp: usize, nex: &mut usize) -> Result<(), NjoyError> {
    let (k, iaa) = (rx.k, rx.iaa);
    // `findf(matd,5,mt)` + `contio` have just read MF=5/MT=mt: `mth = mt`.
    let mth = rx.mt;
    let mut c = SectionCursor::new(rows);
    let head = c.read_cont()?;
    let nk = head.n1.max(0) as usize;
    x.set(b.ldlwp + jp - 1, (*nex - b.dlwp + 1) as f64);
    let mut last = 0usize;
    for ik in 1..=nk {
        let p = c.read_tab1()?;
        let u = p.head.c1;
        let lf = p.head.l2;
        if ik > 1 {
            x.set(last, (*nex - b.dlwp + 1) as f64);
        }
        last = *nex;
        x.set(*nex, 0.0);
        x.set(*nex + 1, f64::from(lf));
        if lf == 1 || lf == 12 {
            x.set(*nex + 1, 4.0);
        }
        // `ja = nex+3`: the probability table's NR/NBT/INT.
        *nex = store_interp(x, *nex + 3, &p);
        let n = p.pairs.len();
        x.set(*nex, n as f64);
        for (j, &(e, pv)) in p.pairs.iter().enumerate() {
            x.set(*nex + 1 + j, sigfig(e / EMEV, 7, 0));
            x.set(*nex + 1 + n + j, pv);
        }
        *nex += 2 * n + 1;
        x.set(last + 2, (*nex - b.dlwp + 1) as f64);
        if lf == 1 {
            let t2 = c.read_tab2()?;
            let m = t2.interp.len();
            let jnt0 = t2.interp.first().map_or(0, |r| r.1);
            let clamp = |j: u32| -> u32 { (j % 10).min(2) };
            if m != 1 || clamp(jnt0) != 2 {
                x.set(*nex, m as f64);
                for (j, &(nbt, int)) in t2.interp.iter().enumerate() {
                    x.set(*nex + 1 + j, f64::from(nbt));
                    x.set(*nex + 1 + m + j, f64::from(clamp(int)));
                }
                *nex += 1 + 2 * m;
            } else {
                x.set(*nex, 0.0);
                *nex += 1;
            }
            let ne = t2.head.n2.max(0) as usize;
            x.set(*nex, ne as f64);
            let nexn = *nex + ne;
            let mut nexd = nexn + ne + 1;
            for j in 1..=ne {
                let g = c.read_tab1()?;
                let e = g.head.c2;
                x.set(*nex + j, sigfig(e / EMEV, 6, 0));
                x.set(nexn + j, (nexd - b.dlwp + 1) as f64);
                // `jnt=nint(scr(6+2*m))`: the last region's law.
                let jnt = g.interp.last().map_or(0, |r| r.1);
                let n = g.pairs.len();
                x.set(nexd, f64::from(jnt));
                x.set(nexd + 1, n as f64);
                nexd += 1;
                x.set(nexd + 1 + 2 * n, 0.0);
                for ki in 1..=n {
                    let (ep, gv) = g.pairs[ki - 1];
                    x.set(ki + nexd, sigfig(ep / EMEV, 7, 0));
                    let mut pd = sigfig(gv * EMEV, 7, 0);
                    if pd < RMIN {
                        pd = 0.0;
                    }
                    x.set(ki + n + nexd, pd);
                    if ki > 1 {
                        let (epl, gl) = g.pairs[ki - 2];
                        if jnt == 1 {
                            let v = x.get(ki + 2 * n - 1 + nexd) + gl * (ep - epl);
                            x.set(ki + 2 * n + nexd, v);
                        }
                        if jnt == 2 {
                            let v = x.get(ki + 2 * n - 1 + nexd) + ((gl + gv) / 2.0) * (ep - epl);
                            x.set(ki + 2 * n + nexd, v);
                        }
                    }
                }
                let mut renorm = 1.0f64;
                if x.get(3 * n + nexd) != 0.0 {
                    renorm = 1.0 / x.get(3 * n + nexd);
                }
                for ki in 1..=n {
                    let v = sigfig(x.get(ki + n + nexd) * renorm, 7, 0);
                    x.set(ki + n + nexd, v);
                    let v = sigfig(x.get(ki + 2 * n + nexd) * renorm, 9, 0);
                    x.set(ki + 2 * n + nexd, v);
                }
                nexd += 3 * n + 1;
            }
            *nex = nexd;
        } else if lf == 7 || lf == 9 {
            let th = c.read_tab1()?;
            *nex = store_interp(x, *nex, &th);
            let n = th.pairs.len();
            x.set(*nex, n as f64);
            for (j, &(e, t)) in th.pairs.iter().enumerate() {
                x.set(*nex + 1 + j, sigfig(e / EMEV, 7, 0));
                x.set(*nex + 1 + n + j, sigfig(t / EMEV, 7, 0));
            }
            *nex += 1 + 2 * n;
            x.set(*nex, u / EMEV);
            *nex += 1;
            for ie in iaa..=b.nes {
                let e = x.get(b.esz + ie - 1);
                let y = if mth == 18 {
                    let nu = fnubar.ok_or_else(|| NjoyError::EndfParse("acephn: MF=5/MT=18 needs the MF=1 nubar, and the tape has none".into()))?;
                    terp(nu, e)
                } else {
                    multiplicity(mth)
                };
                let theta = terp(&th, e);
                let xx = if theta > 0.0 { (e - u) / theta } else { 0.0 };
                let mut ebar = 0.0f64;
                if lf == 7 && xx > 0.0 {
                    let st = theta.sqrt();
                    let sx = xx.sqrt();
                    let anorm = st.r_powi(3) * (std::f64::consts::PI.sqrt() * (1.0 - erfc(xx)) / 2.0 - sx * (-xx).r_exp());
                    ebar = 3.0 * theta / 2.0 - (st.r_powi(2) * sx.r_powi(3) * (-xx).r_exp()) / anorm;
                } else if lf == 9 && xx > 0.0 {
                    let anorm = theta.r_powi(2) * (1.0 - (-xx).r_exp() * (1.0 + xx));
                    ebar = 2.0 * theta - (theta.r_powi(3) * xx.r_powi(2) * (-xx).r_exp()) / anorm;
                }
                let hh = y * (ebar / EMEV) * x.get(2 + k + ie - iaa);
                let v = x.get(b.phn + 2 + ie - b.it) + hh;
                x.set(b.phn + 2 + ie - b.it, v);
                if x.get(b.tot + ie - 1) != 0.0 {
                    let v = x.get(b.thn + ie - 1) - hh / x.get(b.tot + ie - 1);
                    x.set(b.thn + ie - 1, v);
                }
            }
        } else {
            return Err(NjoyError::InvalidInput(format!("acephn: file 5 law not ready (MT={mth}, LF={lf}, acepn.f90:1420)")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The multiplicity table, and the leftover-`mth` quirk: the MF=4 branch
    /// reads `mth = 0`, so an (n,2n) given in MF=4 gets `y = 1`, as upstream.
    #[test]
    fn multiplicity_table_and_the_leftover_mth() {
        assert_eq!(multiplicity(16), 2.0);
        assert_eq!(multiplicity(17), 3.0);
        assert_eq!(multiplicity(37), 4.0);
        assert_eq!(multiplicity(152), 5.0);
        assert_eq!(multiplicity(153), 6.0);
        assert_eq!(multiplicity(160), 7.0);
        assert_eq!(multiplicity(161), 8.0);
        assert_eq!(multiplicity(91), 1.0);
        assert_eq!(multiplicity(MTH_AFTER_PASS_1), 1.0);
    }
}
