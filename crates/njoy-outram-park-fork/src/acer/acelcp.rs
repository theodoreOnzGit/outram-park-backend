// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine first`, l.404-500 and 578-792: the particle-production list
//     (`nprod`, `iprod`, `mprod`, `kprod`, `lprod`) and thresholds `t201..t207`;
//   - `subroutine acelcp`, l.9178-10990: the IXS array and the HPD, MTRH,
//     TYRH, LSIGH, SIGH, LANDH, ANDH, LDLWH, DLWH and YH blocks;
//   - `acecm.f90` `eavl`/`fi1`/`fi2`, l.558-617.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Charged-particle production in a continuous-energy neutron table:
//! `acelcp`.
//!
//! For each light particle an evaluation produces (p, d, t, ³He, α), NJOY
//! appends one set of blocks after the table's `END` word:
//! - HPD: the production cross section and the heating it carries;
//! - MTRH/TYRH/LSIGH/SIGH: the contributing reactions and their yields;
//! - LANDH/ANDH: angular distributions;
//! - LDLWH/DLWH: energy distributions;
//! - YH: the reactions that produce the particle.
//!
//! `NXS(7)` counts the particle types, and `JXS(30..32)` locate the IXS
//! arrays that point to the blocks.
//!
//! **Ported branches:**
//! - MF=4 two-body reactions with isotropic angles (`LTT = 0`), written as
//!   law 33;
//! - MF=6 `LAW = 1` products with Legendre angles (`LANG = 1`, through
//!   `ptleg2`, law 61), tabulated angles (`LANG >= 11`, law 61) or
//!   Kalbach-Mann (`LANG = 2`, law 44).
//!
//! That covers every entry in ENDF/B-VIII.0 U-235 and U-238: MT=5, MT=649
//! and MT=800-835. **Not ported:**
//! - MF=6 `LAW = 2/3/4/6/7`;
//! - MF=4 with `LTT != 0`;
//! - elastic and capture recoils (MT=2, and MT=102 for light targets);
//! - incident charged particles.
//!
//! [`append`] returns without writing anything, rather than a partial block,
//! when a production item needs one of these.

use crate::endf::interp::terpa;
use crate::endf::records::{List, SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::reconr::ReconrResult;

use super::{jxs, nxs, AceTable};

const EMEV: f64 = 1.0e6;
const ETOP: f64 = 1.0e10;
const DELT: f64 = 1.0e-10;
const SMALL: f64 = 1.0e-30;
/// `awr5` etc. (`acefc.f90:9205-9209`): the light particles' mass ratios.
const AWR_P: f64 = 0.99862;
const AWR_D: f64 = 1.99626;
const AWR_T: f64 = 2.98960;
const AWR_HE3: f64 = 2.98903;
const AWR_A: f64 = 3.96713;

/// One production item: `(mprod, iprod, kprod, lprod)`.
#[derive(Debug, Clone, Copy)]
struct Prod {
    mt: i32,
    ip: i32,
    mf: i32,
    l: i32,
}

/// `eavl` (`acecm.f90:558-583`).
fn eavl(akal: f64, amass: f64, avcm: f64, avadd: f64, fmsd: f64, sign: f64) -> f64 {
    let fi1 = |aa: f64| if aa == 0.0 { 2.0 } else { (aa.exp() - (-aa).exp()) / aa };
    let fi2 = |aa: f64| {
        if aa == 0.0 {
            0.0
        } else {
            ((aa.exp() + (-aa).exp()) / aa) - ((aa.exp() - (-aa).exp()) / (aa * aa))
        }
    };
    let r = avcm * avcm + avadd * avadd;
    let s = 2.0 * avcm * avadd;
    amass * (r / 2.0 + (fmsd * s * fi2(sign * akal) / (fi1(sign * akal) * 2.0)))
}

/// The flat `scr` array `tab1io` leaves: `[c1, c2, l1, l2, nr, np, nbt/int
/// pairs, x/y pairs]`, so that upstream's fixed offsets can be read as they
/// are.
fn flat(t: &Tab1) -> Vec<f64> {
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

/// First tabulated energy after `gety1`'s leading-zero scan.
fn first_energy(pairs: &[(f64, f64)]) -> f64 {
    let np = pairs.len() as i32;
    let t = Tab1 {
        head: crate::endf::records::Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: np },
        interp: vec![(np.max(1) as u32, 2)],
        pairs: pairs.to_vec(),
    };
    crate::endf::gety1::Gety1::new(&t).get(0.0).xnext
}

/// `first`'s production list and thresholds (`acefc.f90:404-500, 578-792`).
fn production(tape: &Tape, mat: i32, pendf: &ReconrResult, za: f64) -> (Vec<Prod>, [f64; 7]) {
    const IZAI: i32 = 1;
    let mut prods: Vec<Prod> = Vec::new();
    // Dictionary pass: MF=3/MT=102 for light targets, MF=4 elastic and
    // two-body charged-particle levels. The dictionary is in (MF, MT) order.
    let mut secs: Vec<(i32, i32)> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mt > 0)
        .map(|s| (s.key.mf, s.key.mt))
        .collect();
    secs.sort_unstable();
    secs.dedup();
    for (mfd, mtd) in secs {
        let iflag = (mfd == 3 && mtd == 102 && za + f64::from(IZAI) < 2004.1)
            || (mfd == 4 && mtd == 2)
            || (mfd == 4 && ((600..=648).contains(&mtd)
                || (650..=698).contains(&mtd)
                || (700..=748).contains(&mtd)
                || (750..=798).contains(&mtd)
                || (800..=848).contains(&mtd)));
        if !iflag {
            continue;
        }
        let base = (za + f64::from(IZAI)).round() as i32;
        let mut y = [0i32; 7]; // y201, _, y203..y207 by index 0,2..6
        let mut izr = 0;
        match mtd {
            2 => izr = base - IZAI,
            50..=91 => {
                izr = base - 1;
                y[0] = 1;
            }
            102 => izr = base,
            600..=649 => {
                izr = base - 1001;
                y[2] = 1;
            }
            650..=699 => {
                izr = base - 1002;
                y[3] = 1;
            }
            700..=749 => {
                izr = base - 1003;
                y[4] = 1;
            }
            750..=799 => {
                izr = base - 2003;
                y[5] = 1;
            }
            800..=849 => {
                izr = base - 2004;
                y[6] = 1;
            }
            _ => {}
        }
        match izr {
            1001 => y[2] += 1,
            1002 => y[3] += 1,
            1003 => y[4] += 1,
            2003 => y[5] += 1,
            2004 => y[6] += 1,
            4008 => y[6] += 2,
            _ => {}
        }
        let ip = if y[0] > 0 && IZAI != 1 {
            1
        } else if y[2] > 0 && IZAI != 1001 {
            1001
        } else if y[3] > 0 && IZAI != 1002 {
            1002
        } else if y[4] > 0 && IZAI != 1003 {
            1003
        } else if y[5] > 0 && IZAI != 2003 {
            2003
        } else if y[6] > 0 && IZAI != 2004 {
            2004
        } else {
            0
        };
        if ip != 0 {
            prods.push(Prod { mt: mtd, ip, mf: mfd, l: 0 });
        }
    }

    // Thresholds from the PENDF's MF=3, MT after 2 (`:578-615`).
    let mut t = [ETOP; 7]; // t201, _, t203..t207
    let slot = |ip: i32| match ip {
        1 => Some(0),
        1001 => Some(2),
        1002 => Some(3),
        1003 => Some(4),
        2003 => Some(5),
        2004 => Some(6),
        _ => None,
    };
    for s in pendf.sections.iter().filter(|s| s.mt.number() > 2) {
        let enext = first_energy(&s.pairs);
        for p in prods.iter().filter(|p| p.mt == s.mt.number()) {
            if let Some(k) = slot(p.ip) {
                if enext < t[k] {
                    t[k] = enext;
                }
            }
        }
    }

    // MF=6 products (`:640-744`).
    let nprod3 = prods.len();
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6) {
        let mtd = sec.key.mt;
        let xsthr = pendf
            .sections
            .iter()
            .find(|s| s.mt.number() == mtd)
            .map_or(0.0, |s| first_energy(&s.pairs));
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { continue };
        for ik in 1..=head.n1 {
            let Ok(yt) = cur.read_tab1() else { break };
            let izap = yt.head.c1.round() as i32;
            let law = yt.head.l2;
            let f = flat(&yt);
            let nn = yt.pairs.len();
            let mut jj = 0usize;
            for ii in 1..=nn {
                if jj == 0 && f.get(7 + 2 * ii).copied().unwrap_or(0.0) > 0.0 {
                    jj = ii;
                }
            }
            if jj > 1 {
                jj -= 1;
            }
            if let Some(k) = slot(izap) {
                if izap != IZAI {
                    let mut e = f.get(6 + 2 * jj).copied().unwrap_or(0.0);
                    if xsthr > e {
                        e = xsthr;
                    }
                    if e < t[k] {
                        t[k] = e;
                    }
                    prods.push(Prod { mt: mtd, ip: izap, mf: 6, l: ik });
                }
            }
            if super::energy::mf6::skip_mf6_subsection(&mut cur, law).is_err() {
                break;
            }
        }
    }
    // sort (`:747-761`)
    let n = prods.len();
    let key = |p: &Prod| 100_000 * i64::from(p.mt) + 10 * i64::from(p.ip) + i64::from(p.l);
    for i in 0..n.saturating_sub(1) {
        for j in i..n {
            if key(&prods[i]) > key(&prods[j]) {
                prods.swap(i, j);
            }
        }
    }
    // drop redundant MF=4/MF=6 duplicates (`:763-792`)
    if prods.len() != nprod3 && prods.len() > 1 {
        let mut nprod = prods.len();
        let mut nprodt = 2usize; // 1-based
        while nprodt <= nprod {
            let (a, b) = (prods[nprodt - 2], prods[nprodt - 1]);
            if b.mt == a.mt && b.ip == a.ip && b.mf != a.mf {
                nprod -= 1;
                if nprodt <= nprod {
                    nprodt -= 1;
                    prods.remove(nprodt - 1);
                } else {
                    prods.truncate(nprod);
                }
            }
            nprodt += 1;
        }
        prods.truncate(nprod);
    }
    (prods, t)
}

/// A growable view of XSS with upstream's 1-based indexing.
struct Xss<'a> {
    t: &'a mut AceTable,
}

impl Xss<'_> {
    fn get(&self, i: usize) -> f64 {
        self.t.xss.get(i - 1).copied().unwrap_or(0.0)
    }
    fn set(&mut self, i: usize, v: f64, int: bool) {
        if self.t.xss.len() < i {
            self.t.xss.resize(i, 0.0);
            self.t.xss_is_int.resize(i, false);
        }
        self.t.xss[i - 1] = v;
        self.t.xss_is_int[i - 1] = int;
    }
    fn r(&mut self, i: usize, v: f64) {
        self.set(i, v, false);
    }
    fn n(&mut self, i: usize, v: f64) {
        self.set(i, v, true);
    }
}

/// Append `acelcp`'s charged-particle blocks to a neutron table (`izai = 1`).
///
/// `pendf` is the PENDF the table was made from. The table must be complete
/// (ESZ, SIG and LQR final), as `acelcp` runs after `acelod`'s final pass
/// (`acefc.f90:6323`).
pub fn append(table: &mut AceTable, tape: &Tape, mat: i32, pendf: &ReconrResult) {
    let za = f64::from(table.nxs[nxs::ZA]);
    let awr = table.awr;
    let awi = 1.0f64;
    let (prods, t) = production(tape, mat, pendf, za);
    // Types present (`:9258-9282`), in upstream's order.
    let kinds: [(i32, f64, usize); 5] = [(1001, 9.0, 2), (1002, 31.0, 3), (1003, 32.0, 4), (2003, 33.0, 5), (2004, 34.0, 6)];
    let types: Vec<(i32, f64, f64)> = kinds
        .iter()
        .filter(|&&(_, _, k)| t[k] < ETOP)
        .map(|&(ip, code, k)| (ip, code, t[k]))
        .collect();
    if types.is_empty() {
        return;
    }
    // Refuse what is not ported, before touching the table.
    for p in &prods {
        if !types.iter().any(|&(ip, _, _)| ip == p.ip) || p.mf == 0 {
            continue;
        }
        let ok = match p.mf {
            4 => p.mt != 2 && mf4_ltt(tape, mat, p.mt) == Some(0),
            6 => mf6_supported(tape, mat, p),
            _ => false,
        };
        if !ok {
            return;
        }
    }

    let nes = table.nxs[nxs::NES] as usize;
    let esz = table.jxs[jxs::ESZ] as usize;
    let mtr = table.jxs[jxs::MTR] as usize;
    let lqr = table.jxs[jxs::LQR] as usize;
    let lsig = table.jxs[jxs::LSIG] as usize;
    let sig = table.jxs[jxs::SIG] as usize;
    let ntr = table.nxs[nxs::NTR] as usize;
    let end = table.xss.len();
    // `emc2=amassn*amu*clight*clight/ev/emev` (`:9253`), left to right.
    use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
    let emc2 = AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG / EMEV;
    let ntype = types.len();

    let mut x = Xss { t: table };
    let ptype = end + 1;
    let ntro = ptype + ntype;
    let ploct = ntro + ntype;
    for (i, &(ip, code, _)) in types.iter().enumerate() {
        x.n(ptype + i, code);
        let ntrh = prods.iter().filter(|p| p.ip == ip && p.mf > 0).count();
        x.n(ntro + i, ntrh as f64);
    }
    let mut next = ploct + 10 * ntype;
    for i in 0..10 * ntype {
        x.n(ploct + i, 0.0);
    }

    // The neutron SIG entry of `mt`: (k, iaa, q).
    let find_sig = |x: &Xss, mt: i32| -> Option<(usize, usize, f64)> {
        for ir in 1..=ntr {
            if x.get(mtr + ir - 1).round() as i32 == mt {
                let k = x.get(lsig + ir - 1).round() as usize + sig - 1;
                let iaa = x.get(k).round() as usize;
                return Some((k, iaa, x.get(lqr + ir - 1)));
            }
        }
        None
    };

    for (itype, &(ip, _, thr)) in types.iter().enumerate() {
        let thresh = thr / EMEV;
        let ntrh = prods.iter().filter(|p| p.ip == ip && p.mf > 0).count();
        let mut it = 1usize;
        while x.get(esz + it - 1) < thresh * (1.0 - DELT) {
            it += 1;
        }
        let hpd = next;
        let pl = ploct + 10 * itype;
        x.n(pl, hpd as f64);
        x.n(hpd, it as f64);
        x.n(hpd + 1, (nes - it + 1) as f64);
        let naa = nes - it + 1;
        let mtrh = hpd + 2 + 2 * naa;
        x.n(pl + 1, mtrh as f64);
        let tyrh = mtrh + ntrh;
        x.n(pl + 2, tyrh as f64);
        let lsigh = tyrh + ntrh;
        x.n(pl + 3, lsigh as f64);
        let sigh = lsigh + ntrh;
        x.n(pl + 4, sigh as f64);
        next = sigh;
        for ie in it..=nes {
            x.r(hpd + 2 + ie - it, 0.0);
            x.r(hpd + 2 + naa + ie - it, 0.0);
        }

        // ── MTRH, TYRH, LSIGH, SIGH and the production cross section ──
        let mut jp = 0usize;
        for p in prods.iter().filter(|p| p.ip == ip && p.mf != 0) {
            jp += 1;
            x.n(mtrh + jp - 1, f64::from(p.mt));
            x.n(lsigh + jp - 1, (next - sigh + 1) as f64);
            let Some((k, iaa, _q)) = find_sig(&x, p.mt) else { return };
            if p.mf == 4 {
                x.n(tyrh + jp - 1, -1.0);
                for ie in iaa..=nes {
                    let tt = x.get(hpd + 2 + ie - it) + x.get(2 + k + ie - iaa);
                    x.r(hpd + 2 + ie - it, sigfig(tt, 7, 0));
                }
                x.n(next, 12.0);
                x.n(next + 1, f64::from(p.mt));
                x.n(next + 2, 0.0);
                x.n(next + 3, 2.0);
                let (e1, e2) = (sigfig(x.get(esz + it - 1), 7, 0), sigfig(x.get(esz + nes - 1), 7, 0));
                x.r(next + 4, e1);
                x.r(next + 5, e2);
                x.r(next + 6, 1.0);
                x.r(next + 7, 1.0);
                next += 8;
            } else {
                let sec = tape.section(mat, 6, p.mt).expect("checked");
                let mut cur = SectionCursor::new(&sec.rows);
                let head = cur.read_cont().expect("checked");
                x.n(tyrh + jp - 1, if head.l2 == 1 { 1.0 } else { -1.0 });
                let yt = nth_product(sec, p.l).expect("checked").0;
                for ie in iaa..=nes {
                    let e = x.get(esz + ie - 1) * EMEV;
                    let (mut y, _, _) = terpa(&yt.interp, &yt.pairs, e);
                    if y < DELT {
                        y = 0.0;
                    }
                    let ss = x.get(2 + k + ie - iaa);
                    let tt = x.get(hpd + 2 + ie - it) + y * ss;
                    x.r(hpd + 2 + ie - it, sigfig(tt, 7, 0));
                }
                x.n(next, 12.0);
                x.n(next + 1, f64::from(p.mt));
                next += 2;
                let nrint = yt.interp.len();
                if nrint == 1 && yt.interp[0].1 == 2 {
                    x.n(next, 0.0);
                } else {
                    x.n(next, nrint as f64);
                    for (i, &(nbt, int)) in yt.interp.iter().enumerate() {
                        x.n(next + 1 + i, f64::from(nbt));
                        x.n(next + 1 + nrint + i, f64::from(int));
                    }
                    next += 2 * nrint;
                }
                next += 1;
                let ne = yt.pairs.len();
                x.n(next, ne as f64);
                for (i, &(e, y)) in yt.pairs.iter().enumerate() {
                    x.r(next + 1 + i, sigfig(e / EMEV, 7, 0));
                    x.r(next + 1 + ne + i, sigfig(y, 7, 0));
                }
                next += 1 + 2 * ne;
            }
        }

        // ── LANDH, ANDH ──
        let landh = next;
        x.n(pl + 5, landh as f64);
        let andh = landh + ntrh;
        x.n(pl + 6, andh as f64);
        for i in 0..ntrh {
            x.n(landh + i, 0.0);
        }
        next = andh;
        let mut jp = 0usize;
        for p in prods.iter().filter(|p| p.ip == ip && p.mf != 0) {
            jp += 1;
            if p.mf == 6 {
                // LAW=1: the angles live in DLWH (`xss(landh+jp-1)=-1`).
                x.n(landh + jp - 1, -1.0);
            }
            // MF=4 with LTT = 0: isotropic, LAND stays 0.
        }

        // ── LDLWH, DLWH ──
        let ldlwh = next;
        x.n(pl + 7, ldlwh as f64);
        let dlwh = ldlwh + ntrh;
        x.n(pl + 8, dlwh as f64);
        next = dlwh;
        let mut jp = 0usize;
        for p in prods.iter().filter(|p| p.ip == ip && p.mf != 0) {
            jp += 1;
            x.n(ldlwh + jp - 1, (next - dlwh + 1) as f64);
            let last = next;
            x.n(next, 0.0);
            x.n(next + 1, 0.0);
            x.n(next + 2, 0.0);
            next += 3;
            let Some((k, iaa, q)) = find_sig(&x, p.mt) else { return };
            let amass = awr / awi;
            if p.mf == 4 {
                let awp = match p.mt {
                    600..=649 => AWR_P,
                    650..=699 => AWR_D,
                    700..=749 => AWR_T,
                    750..=799 => AWR_HE3,
                    800..=849 => AWR_A,
                    _ => 1.0,
                };
                x.n(last + 1, 33.0);
                x.n(next, 0.0);
                x.n(next + 1, 2.0);
                for i in 2..6 {
                    x.r(next + i, 0.0);
                }
                next += 2 + 2 * 2;
                x.n(last + 2, (next - dlwh + 1) as f64);
                let aprime = awp / awi;
                let d1 = sigfig((1.0 + amass) * (-q) / amass, 7, 0);
                let d2 = sigfig(amass * (amass + 1.0 - aprime) / (1.0 + amass).powi(2), 7, 0);
                x.r(next, d1);
                x.r(next + 1, d2);
                // heating, LTT = 0
                for ie in it..=nes {
                    let e = x.get(esz + ie - 1);
                    let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
                    let tt = d2 * (e - d1) * ss;
                    let h0 = x.get(hpd + 2 + naa + ie - it);
                    x.r(hpd + 2 + naa + ie - it, h0 + tt);
                }
                next += 2;
            } else {
                let sec = tape.section(mat, 6, p.mt).expect("checked");
                let (yt, awp, lists, lang, lep) = nth_product_law1(sec, p.l).expect("checked");
                let isocp = !lists.iter().any(|l| l.head.l2 > 0);
                let lawnow = if lang == 2 {
                    44
                } else if lang >= 11 {
                    61
                } else if isocp {
                    4
                } else {
                    61
                };
                x.n(last + 1, f64::from(lawnow));
                if isocp {
                    x.n(landh + jp - 1, 0.0);
                }
                let ne = lists.len();
                let lee = next;
                x.n(next, 0.0);
                x.n(next + 1, 2.0);
                for i in 2..6 {
                    x.r(next + i, 0.0);
                }
                next += 2 + 2 * 2;
                x.n(last + 2, (next - dlwh + 1) as f64);
                x.n(next, 0.0);
                x.n(next + 1, ne as f64);
                let lle = next + 2;
                next = lle + 2 * ne;
                let mut heat: Vec<(f64, f64)> = Vec::with_capacity(ne);
                for (ie, l) in lists.iter().enumerate() {
                    let ee_in = sigfig(l.head.c2 / EMEV, 7, 0);
                    if ie == 0 {
                        x.r(lee + 2, ee_in);
                        x.r(lee + 4, 1.0);
                    } else if ie == ne - 1 {
                        x.r(lee + 3, ee_in);
                        x.r(lee + 5, 1.0);
                    }
                    x.r(lle + ie, ee_in);
                    let ee = ee_in;
                    x.n(lle + ne + ie, (next - dlwh + 1) as f64);
                    let nd = l.head.l1.max(0) as usize;
                    let na = l.head.l2.max(0) as usize;
                    let ncyc = na + 2;
                    let ng = l.head.n2.max(0) as usize;
                    let d = &l.data;
                    let row = |ig: usize, c: usize| d[ncyc * (ig - 1) + c];
                    x.n(next, if lawnow == 4 { f64::from(lep) } else { f64::from(lep + 10 * nd as i32) });
                    x.n(next + 1, ng as f64);
                    let mut nexcd = next + 4 * ng + 2;
                    let pamass = awp * emc2;
                    let avadd = awi * (2.0 * ee / (emc2 * awi)).sqrt() / (awi + awr);
                    let mut avlab = 0.0;
                    let mut avll = 0.0;
                    for ig in 1..=ng {
                        x.r(next + 1 + ig, sigfig(row(ig, 0) / EMEV, 7, 0));
                        let mut pv = sigfig(row(ig, 1) * EMEV, 7, 0);
                        if pv > 0.0 && pv < SMALL {
                            pv = SMALL;
                        }
                        x.r(next + 1 + ig + ng, pv);
                        let mut c = 0.0;
                        if ig <= nd {
                            c += row(ig, 1);
                        }
                        if nd > 0 && ig == nd + 1 {
                            c = x.get(next + ig + 2 * ng);
                        }
                        if ig > nd + 1 && lep == 1 {
                            c = x.get(next + ig + 2 * ng) + row(ig - 1, 1) * (row(ig, 0) - row(ig - 1, 0));
                        }
                        if ig > nd + 1 && lep == 2 {
                            c = x.get(next + ig + 2 * ng)
                                + ((row(ig - 1, 1) + row(ig, 1)) / 2.0) * (row(ig, 0) - row(ig - 1, 0));
                        }
                        x.r(next + 1 + ig + 2 * ng, c);
                        let (mut rkal, mut akal) = (0.0, 0.0);
                        if lang == 2 {
                            rkal = row(ig, 2);
                            x.r(next + 1 + ig + 3 * ng, sigfig(rkal, 7, 0));
                            let ep = x.get(next + 1 + ig);
                            akal = if na == 2 {
                                row(ig, 3)
                            } else {
                                super::acelf6::bachaa(1, ip, za.round() as i32, ee, ep).unwrap_or(0.0)
                            };
                            x.r(next + 1 + ig + 4 * ng, sigfig(akal, 7, 0));
                        } else if lawnow == 61 {
                            // LANG = 1: normalised Legendre coefficients through
                            // `ptleg2`, lin-lin; LANG >= 11: the tabulated
                            // (mu, f) pairs as they are (`:10440-10460`).
                            let (intmu, mus, fs): (i32, Vec<f64>, Vec<f64>) = if lang == 1 {
                                let b0 = row(ig, 1);
                                let coeffs: Vec<f64> = (1..=na)
                                    .map(|ia| if b0 != 0.0 { row(ig, 1 + ia) / b0 } else { 0.0 })
                                    .collect();
                                let (m, f) = super::acensd::ptleg2(&coeffs).unwrap_or_default();
                                (2, m, f)
                            } else {
                                let nmu = na / 2;
                                (
                                    lang - 10,
                                    (1..=nmu).map(|i| row(ig, 2 * i)).collect(),
                                    (1..=nmu).map(|i| row(ig, 2 * i + 1)).collect(),
                                )
                            };
                            let nmu = mus.len();
                            x.n(next + 1 + 3 * ng + ig, (nexcd - dlwh + 1) as f64);
                            x.n(nexcd, f64::from(intmu));
                            x.n(nexcd + 1, nmu as f64);
                            let mu = |imu: usize| mus[imu - 1];
                            let fv = |imu: usize| fs[imu - 1];
                            let mut sum = 0.0;
                            for imu in 1..=nmu {
                                x.r(nexcd + 1 + imu, mu(imu));
                                x.r(nexcd + 1 + nmu + imu, fv(imu));
                                if imu == 1 {
                                    sum = 0.0;
                                    x.r(nexcd + 1 + 2 * nmu + imu, 0.0);
                                } else {
                                    let del = mu(imu) - mu(imu - 1);
                                    if intmu == 1 {
                                        sum += del * fv(imu - 1);
                                    } else {
                                        let av = (fv(imu) + fv(imu - 1)) / 2.0;
                                        sum += del * av;
                                    }
                                    x.r(nexcd + 1 + 2 * nmu + imu, sum);
                                }
                            }
                            for imu in 1..=nmu {
                                for off in [0, nmu, 2 * nmu] {
                                    let v = x.get(nexcd + 1 + off + imu) / sum;
                                    x.r(nexcd + 1 + off + imu, sigfig(v, 7, 0));
                                }
                            }
                            nexcd += 2 + 3 * nmu;
                        }
                        if ig != 1 {
                            let eavi = x.get(next + 1 + ig);
                            let avl = if na == 0 {
                                eavi
                            } else {
                                let avcm = (2.0 * eavi / pamass).sqrt();
                                eavl(akal, pamass, avcm, avadd, rkal, 1.0)
                            };
                            let dele = x.get(next + 1 + ig) - x.get(next + ig);
                            let avav = if lep == 1 {
                                x.get(next + ig + ng) * (avll + avl) / 2.0
                            } else {
                                (x.get(next + ig + ng) * avll + x.get(next + ig + 1 + ng) * avl) / 2.0
                            };
                            avlab += avav * dele;
                            avll = avl;
                        }
                    }
                    let renorm = 1.0 / x.get(next + 1 + 3 * ng);
                    for ig in 1..=ng {
                        let pv = x.get(next + 1 + ng + ig);
                        x.r(next + 1 + ng + ig, sigfig(renorm * pv, 7, 0));
                        let cv = x.get(next + 1 + 2 * ng + ig);
                        x.r(next + 1 + 2 * ng + ig, sigfig(renorm * cv, 9, 0));
                    }
                    heat.push((ee, avlab));
                    next = if lawnow == 61 { nexcd } else { next + 2 + (2 * na + 3) * ng };
                }
                // heating for this subsection
                let hint = vec![(heat.len() as u32, 2)];
                for ie in it..=nes {
                    let e = x.get(esz + ie - 1);
                    let (y, _, _) = terpa(&yt.interp, &yt.pairs, e * EMEV);
                    let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
                    let (h, _, _) = terpa(&hint, &heat, e);
                    let h0 = x.get(hpd + 2 + naa + ie - it);
                    x.r(hpd + 2 + naa + ie - it, h0 + h * (y * ss));
                }
            }
        }

        // heating per unit production cross section (`:10934-10947`)
        for ie in it..=nes {
            let mut h = x.get(hpd + 2 + naa + ie - it);
            let tot = x.get(esz + nes + ie - 1);
            if tot != 0.0 {
                h /= tot;
            }
            if h < DELT {
                h = 0.0;
            }
            x.r(hpd + 2 + naa + ie - it, sigfig(h, 7, 0));
            let eh = x.get(esz + 4 * nes + ie - 1);
            x.r(esz + 4 * nes + ie - 1, sigfig(eh, 7, 0));
        }

        // YH (`:10950-10960`)
        let yh = next;
        x.n(pl + 9, yh as f64);
        next += 1;
        for p in prods.iter().filter(|p| p.ip == ip) {
            x.n(next, f64::from(p.mt));
            next += 1;
        }
        x.n(yh, (next - yh - 1) as f64);
    }

    let len2 = next - 1;
    table.xss.truncate(len2);
    table.xss_is_int.truncate(len2);
    table.nxs[nxs::LEN_XSS] = len2 as i32;
    table.nxs[6] = ntype as i32;
    table.jxs[29] = ptype as i32;
    table.jxs[30] = ntro as i32;
    table.jxs[31] = ploct as i32;
}

/// LTT of MF=4/`mt`.
fn mf4_ltt(tape: &Tape, mat: i32, mt: i32) -> Option<i32> {
    let sec = tape.section(mat, 4, mt)?;
    let mut cur = SectionCursor::new(&sec.rows);
    Some(cur.read_cont().ok()?.l2)
}

/// Subsection `l` (1-based) of an MF=6 section: its yield TAB1 and LAW.
fn nth_product(sec: &crate::endf::tape::Section, l: i32) -> Option<(Tab1, i32)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    for ik in 1..=head.n1 {
        let y = cur.read_tab1().ok()?;
        let law = y.head.l2;
        if ik == l {
            return Some((y, law));
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    None
}

/// Subsection `l` of an MF=6 section, `LAW = 1`: yield, AWP, the LISTs,
/// LANG and LEP.
fn nth_product_law1(sec: &crate::endf::tape::Section, l: i32) -> Option<(Tab1, f64, Vec<List>, i32, i32)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    for ik in 1..=head.n1 {
        let y = cur.read_tab1().ok()?;
        let law = y.head.l2;
        if ik == l {
            if law != 1 {
                return None;
            }
            let t2 = cur.read_tab2().ok()?;
            let (lang, lep) = (t2.head.l1, t2.head.l2);
            let mut lists = Vec::new();
            for _ in 0..t2.head.n2 {
                lists.push(cur.read_list().ok()?);
            }
            let awp = y.head.c2;
            return Some((y, awp, lists, lang, lep));
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    None
}

/// Whether a production item from MF=6 is a ported branch: `LAW = 1` with
/// `LANG = 2` or `LANG >= 11`.
fn mf6_supported(tape: &Tape, mat: i32, p: &Prod) -> bool {
    let Some(sec) = tape.section(mat, 6, p.mt) else { return false };
    if p.mt == 102 || p.mt == 2 {
        return false;
    }
    match nth_product_law1(sec, p.l) {
        Some((_, _, _, lang, _)) => lang == 1 || lang == 2 || lang >= 11,
        None => false,
    }
}
