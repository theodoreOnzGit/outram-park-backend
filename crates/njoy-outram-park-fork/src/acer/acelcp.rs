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
//! **Ported branches** (for an incident neutron, `izai = 1`):
//! - the neutron capture recoil on a light target (MT=102 from MF=3 or
//!   MF=6): straight-ahead angles, law 33;
//! - MF=4 two-body reactions with `LTT = 0` (isotropic), `1` (Legendre,
//!   through `ptleg2`) or `2` (tabulated, through `pttab2`), law 33, with the
//!   two-body heating from each energy's mean cosine;
//! - MF=6 `LAW = 1` products with Legendre angles (`LANG = 1`, through
//!   `ptleg2`, law 61), tabulated angles (`LANG >= 11`, law 61) or
//!   Kalbach-Mann (`LANG = 2`, law 44);
//! - MF=6 `LAW = 2` two-body products (`LANG = 0` or `12`), `LAW = 3`
//!   (isotropic two-body), `LAW = 4` recoils (angles from the section's
//!   first, `LAW = 2`, subsection with the odd Legendre terms negated), all
//!   law 33, and `LAW = 0`, which upstream writes as an empty law-0 entry.
//!
//! ~~That covers every entry in ENDF/B-VIII.0 U-235 and U-238~~ (extended
//! 2026-10-14, GitHub #872): with the branches above, H-1, F-19, Al-27,
//! Si-28/29/30 and Mn-55 are NJOY2016's tables in every word. **Not ported:**
//! - MF=6 `LAW = 5` (~~`/6`~~ **CORRECTED 2026-11-11: LAW = 6 is ported**,
//!   gh:#1485, in `acelcp_ext::law6_grid`/`law6_h`; ~~`/7`~~: **LAW = 7 is
//!   read since 2026-11-09**, gh:#1309, as `topfil`'s LAW = 1 rewrite,
//!   `acelf6::law7_rewrite`);
//! - ~~the elastic recoil (MT=2)~~ **CORRECTED 2026-11-11: ported** (gh:#1485,
//!   `acelcp_ext::elastic_recoil_angular`; ESZ's elastic column, no DLWH entry);
//! - ~~incident charged particles~~ **CORRECTED 2026-11-11: `izai` branches
//!   ported** (gh:#1485): test decks 14, 50-54, 62 and 71 give NJOY2016's ACE
//!   in every word (`tests/deck_driver_tapes_vs_njoy2016.rs`). Still refused:
//!   a LAW = 5 product other than the projectile's own, and a LAW = 4 recoil
//!   heating of an incident charged particle (`recoil_heating_supported`).
//!
//! [`append`] returns without writing anything, rather than a partial block,
//! when a production item needs one of these.

use petir::mathf::RealMath;
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
    let fi1 = |aa: f64| if aa == 0.0 { 2.0 } else { (aa.r_exp() - (-aa).r_exp()) / aa };
    let fi2 = |aa: f64| {
        if aa == 0.0 {
            0.0
        } else {
            ((aa.r_exp() + (-aa).r_exp()) / aa) - ((aa.r_exp() - (-aa).r_exp()) / (aa * aa))
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
fn production(tape: &Tape, mat: i32, pendf: &ReconrResult, za: f64, izai: i32) -> (Vec<Prod>, [f64; 7]) {
    let mut prods: Vec<Prod> = Vec::new();
    // `first` walks the MF=1/MT=451 dictionary (`dictio`, `nw = nx`,
    // `:373-376`): an evaluation whose dictionary is empty (NXC = 0 in the
    // fourth record, e.g. the d + He-3 tape of test 62) has no MF=4/MF=6
    // item, so `nsix = 0`, `nprod = 0` and every `t20x` stays `etop`.
    if let Some(sec) = tape.section(mat, 1, 451) {
        if sec.rows.get(3).is_some_and(|r| r[5] as i64 == 0) {
            return (prods, [ETOP; 7]);
        }
    }
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
        let iflag = (mfd == 3 && mtd == 102 && izai == 1 && za + f64::from(izai) < 2004.1)
            || (mfd == 4 && mtd == 2)
            || (mfd == 4 && ((600..=648).contains(&mtd)
                || (650..=698).contains(&mtd)
                || (700..=748).contains(&mtd)
                || (750..=798).contains(&mtd)
                || (800..=848).contains(&mtd)));
        if !iflag {
            continue;
        }
        let base = (za + f64::from(izai)).round() as i32;
        let mut y = [0i32; 7]; // y201, _, y203..y207 by index 0,2..6
        let mut izr = 0;
        match mtd {
            2 => izr = base - izai,
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
        let ip = if y[0] > 0 && izai != 1 {
            1
        } else if y[2] > 0 && izai != 1001 {
            1001
        } else if y[3] > 0 && izai != 1002 {
            1002
        } else if y[4] > 0 && izai != 1003 {
            1003
        } else if y[5] > 0 && izai != 2003 {
            2003
        } else if y[6] > 0 && izai != 2004 {
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
                if izap != izai {
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

/// The `awr` `acelcp` is handed. `acelod` overwrites its module variable with
/// the AWR field (`c2h`) of the MF=4 or MF=6 section of every reaction it
/// writes angles for (`awr=c2h`, `acefc.f90:5805`), MT=2 first and then the MTR
/// list in order, and nothing resets it: `acelcp` and the DLW pass see the
/// *last* such section's AWR, which in an incident-charged-particle
/// evaluation differs from MF=1's in the sixth figure (2.989596 against
/// 2.9896 for p + H-3). `awr0` is kept when no reaction has an MF=4/6 section.
fn acelod_awr(tape: &Tape, mat: i32, mtr: &[i32], awr0: f64) -> f64 {
    let mut keys: Vec<(i32, i32)> = tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mt > 0).map(|s| (s.key.mf, s.key.mt)).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut awr = awr0;
    let mut mt418 = false;
    for ij in 1..=mtr.len() + 1 {
        let mt = if ij == 1 { 2 } else { mtr[ij - 2] };
        let mut mf = 0;
        for &(kmf, kmt) in keys.iter().filter(|&&(_, kmt)| kmt == mt) {
            if mt != 18 && (kmf == 4 || kmf == 6) {
                mf = kmf;
            } else if mt == 18 && kmf == 4 && !mt418 {
                mt418 = true;
                mf = kmf;
                break;
            } else if mt == 18 && kmf == 6 {
                mf = kmf;
                break;
            }
        }
        if mf == 4 || mf == 6 {
            if let Some(sec) = tape.section(mat, mf, mt) {
                awr = sec.rows[0][1];
            }
        }
    }
    awr
}

/// A growable view of XSS with upstream's 1-based indexing.
pub(super) struct Xss<'a> {
    t: &'a mut AceTable,
}

impl Xss<'_> {
    pub(super) fn get(&self, i: usize) -> f64 {
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
    pub(super) fn r(&mut self, i: usize, v: f64) {
        self.set(i, v, false);
    }
    pub(super) fn n(&mut self, i: usize, v: f64) {
        self.set(i, v, true);
    }
}

/// Append `acelcp`'s charged-particle blocks to a neutron table (`izai = 1`).
///
/// `pendf` is the PENDF the table was made from. The table must be complete
/// (ESZ, SIG and LQR final), as `acelcp` runs after `acelod`'s final pass
/// (`acefc.f90:6323`).
///
/// `newfor = 0` writes nothing: `first` collects production items only
/// `if (iflag.ne.0.and.newfor.ne.0)` for the MF=3/MF=4 dictionary pass
/// (`acefc.f90:423`) and `if (newfor.ne.0)` for the MF=6 pass (`:621`), so with
/// the old format every `t201..t207` stays `etop` and `acelcp` exits at
/// `ntype.eq.0` (`:9280`). Test 08 (Ni-61) wrote NXS(7)=2 and JXS(30-32)
/// where NJOY wrote 0 (gh:#1449).
pub fn append(table: &mut AceTable, tape: &Tape, mat: i32, pendf: &ReconrResult, newfor: i32) {
    append_incident(table, tape, mat, pendf, newfor, 1, 1.0);
}

/// [`append`] with `topfil`'s scratch tape binary (`text = false`, a negative
/// ACER PENDF unit): the LAW = 7 rewrite keeps full doubles (gh:#1492).
pub fn append_mode(table: &mut AceTable, tape: &Tape, mat: i32, pendf: &ReconrResult, newfor: i32, text: bool) {
    append_incident_mode(table, tape, mat, pendf, newfor, 1, 1.0, text);
}

/// [`append`] for an incident particle `izai` of mass ratio `awi` (`acelcp`'s
/// `izai` branches, gh:#1485): the projectile's own kind is left out of the
/// type list (it is the table's DLW), the neutron joins it, `ip = 1`
/// production carries no heating, and for `izai > 1` the production heating
/// is added to ESZ's heating column and the heavy recoils' heating follows
/// (`acefc.f90:10907-11126`).
pub fn append_incident(table: &mut AceTable, tape: &Tape, mat: i32, pendf: &ReconrResult, newfor: i32, izai: i32, awi: f64) {
    append_incident_mode(table, tape, mat, pendf, newfor, izai, awi, true);
}

/// [`append_incident`] with the scratch-tape mode of [`append_mode`].
#[allow(clippy::too_many_arguments)]
pub fn append_incident_mode(table: &mut AceTable, tape: &Tape, mat: i32, pendf: &ReconrResult, newfor: i32, izai: i32, awi: f64, text: bool) {
    if newfor == 0 {
        return;
    }
    let za = f64::from(table.nxs[nxs::ZA]);
    let ntr_all = table.nxs[nxs::NTR] as usize;
    let mtr_all = table.jxs[jxs::MTR] as usize;
    let mts_all: Vec<i32> = (0..ntr_all).map(|i| table.xss.get(mtr_all + i - 1).copied().unwrap_or(0.0).round() as i32).collect();
    let awr = acelod_awr(tape, mat, &mts_all, table.awr);
    let (prods, t) = production(tape, mat, pendf, za, izai);
    // Types present (`:9258-9282`), in upstream's order.
    let kinds: [(i32, f64, usize); 6] = [(1, 1.0, 0), (1001, 9.0, 2), (1002, 31.0, 3), (1003, 32.0, 4), (2003, 33.0, 5), (2004, 34.0, 6)];
    let types: Vec<(i32, f64, f64)> = kinds
        .iter()
        .filter(|&&(ip, _, k)| t[k] < ETOP && ip != izai)
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
            // Neutron capture recoil on a light target (`:9372`).
            3 => p.mt == 102 && izai == 1,
            // Two-body levels and the elastic recoil (MT=2).
            4 => mf4_ltt(tape, mat, p.mt).is_some(),
            6 => (p.mt == 102 && izai == 1) || mf6_supported(tape, mat, p),
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
    let table_land = table.jxs[jxs::LAND] as usize;
    let table_and = table.jxs[jxs::AND] as usize;
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
        // MT=2 is in no ACE MTR: upstream's search runs on through the other
        // blocks and ends with a meaningless `k`, then sets `iaa = 1` (`:9370`);
        // every MT=2 branch reads the elastic column of ESZ instead.
        if mt == 2 {
            return Some((0, 1, 0.0));
        }
        for ir in 1..=ntr {
            if x.get(mtr + ir - 1).round() as i32 == mt {
                let k = x.get(lsig + ir - 1).round() as usize + sig - 1;
                let iaa = x.get(k).round() as usize;
                return Some((k, iaa, x.get(lqr + ir - 1)));
            }
        }
        None
    };

    // Upstream's `awp` is one variable for the whole routine; the capture
    // recoil branch reads it without setting it. Zero until an item sets it.
    let mut stale_awp = 0.0f64;
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
            // Special branch for MF=4 and the neutron capture recoil, whatever
            // file the MT=102 item came from (`:9370-9392`).
            if p.mf == 4 || (p.mt == 102 && izai == 1) {
                x.n(tyrh + jp - 1, if p.mt == 102 && izai == 1 { 1.0 } else { -1.0 });
                for ie in iaa..=nes {
                    if p.mt == 2 {
                        // the elastic column, indexed `ie-it` (`:9379-9381`)
                        let v = x.get(esz + 3 * nes + ie - it);
                        x.r(hpd + 2 + ie - it, sigfig(v, 7, 0));
                    } else {
                        let tt = x.get(hpd + 2 + ie - it) + x.get(2 + k + ie - iaa);
                        x.r(hpd + 2 + ie - it, sigfig(tt, 7, 0));
                    }
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
                    let ss = if p.mt == 2 { x.get(esz + 3 * nes + ie - 1) } else { x.get(2 + k + ie - iaa) };
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
            let Some((k, iaa, q)) = find_sig(&x, p.mt) else { return };
            let amass = awr / awi;
            if p.mt == 2 {
                // Elastic recoil: the scattered particle's distribution
                // reversed (`:9555-9636`).
                let (land, and) = (table_land, table_and);
                super::acelcp_ext::elastic_recoil_angular(&mut x, &mut next, landh + jp - 1, andh, land, and, hpd, it, nes, esz, amass);
            } else if p.mt == 102 && izai == 1 {
                // Neutron capture recoil: a straight-ahead distribution
                // (`:9618-9641`).
                x.n(landh + jp - 1, (next - andh + 1) as f64);
                x.n(next, 2.0);
                x.r(next + 1, x.get(esz + iaa - 1));
                x.r(next + 2, x.get(esz + nes - 1));
                x.n(next + 3, -((next - andh + 6) as f64));
                x.n(next + 4, -((next - andh + 17) as f64));
                next += 5;
                for _ in 0..2 {
                    x.n(next, 2.0);
                    x.n(next + 1, 3.0);
                    x.r(next + 2, -1.0);
                    x.r(next + 3, 1.0 - 1.0 / 100.0);
                    x.r(next + 4, 1.0);
                    x.r(next + 5, 0.0);
                    x.r(next + 6, 0.0);
                    x.r(next + 7, 200.0);
                    x.r(next + 8, 0.0);
                    x.r(next + 9, 0.0);
                    x.r(next + 10, 1.0);
                    next += 11;
                }
            } else if p.mf == 4 {
                // Two-body levels from MF=4: LTT = 1 through `ptleg2`, LTT = 2
                // through `pttab2`, with the two-body heating from each
                // energy's mean cosine (`:9642-9850`). LTT = 0 and 3 write
                // nothing: LANDH stays 0.
                let Some((ltt, rows)) = mf4_rows(tape, mat, p.mt) else { return };
                if ltt == 1 || ltt == 2 {
                    let aprime = mf4_awp(p.mt) / awi;
                    let heat = write_angular(&mut x, &mut next, landh + jp - 1, andh, &rows, ltt == 2, amass, aprime, q);
                    add_two_body_heating(&mut x, &heat, hpd, naa, it, nes, esz, k, iaa);
                }
            } else {
                // MF=6: LANDH = -1 for the item's subsection; LAW = 2 writes
                // its angles here, and a LAW = 4 recoil takes them from the
                // section's first subsection when that is LAW = 2, with the
                // odd Legendre terms negated (`:9851-10070`).
                x.n(landh + jp - 1, -1.0);
                let sec = tape.section(mat, 6, p.mt).expect("checked");
                let Some((_, law)) = nth_product(sec, p.l) else { return };
                let (lists, aprime, recoil) = match law {
                    2 => {
                        let Some((_, awp, lists)) = nth_product_law2(sec, p.l) else { return };
                        (Some(lists), awp / awi, false)
                    }
                    4 => {
                        let awprec = nth_product(sec, p.l).map_or(0.0, |(y, _)| y.head.c2);
                        (nth_product_law2(sec, 1).map(|(_, _, l)| l), awprec / awi, true)
                    }
                    _ => (None, 0.0, false),
                };
                if let Some(lists) = lists {
                    let mut rows = Vec::with_capacity(lists.len());
                    for l in &lists {
                        let nl = l.head.n2.max(0) as usize;
                        if l.head.l1 == 0 {
                            let mut a: Vec<f64> = l.data[..nl.min(l.data.len())].to_vec();
                            if recoil {
                                for (i, v) in a.iter_mut().enumerate() {
                                    if i % 2 == 0 {
                                        *v = -*v; // a_1, a_3, ...
                                    }
                                }
                            }
                            let Ok((mu, pp)) = super::acensd::ptleg2(&a) else { return };
                            rows.push((l.head.c2, mu, pp, 2));
                        } else {
                            let pairs: Vec<(f64, f64)> = l.data.chunks_exact(2).take(nl).map(|c| (c[0], c[1])).collect();
                            let (mu, pp) = super::acensd::pttab2(&pairs);
                            rows.push((l.head.c2, mu, pp, l.head.l1 - 10));
                        }
                    }
                    let heat = write_angular(&mut x, &mut next, landh + jp - 1, andh, &rows, true, amass, aprime, q);
                    add_two_body_heating(&mut x, &heat, hpd, naa, it, nes, esz, k, iaa);
                }
            }
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
            if p.mt == 2 {
                // no energy distribution for the elastic recoil (`:10134-10137`)
                x.n(ldlwh + jp - 1, 0.0);
                continue;
            }
            x.n(ldlwh + jp - 1, (next - dlwh + 1) as f64);
            let last = next;
            x.n(next, 0.0);
            x.n(next + 1, 0.0);
            x.n(next + 2, 0.0);
            next += 3;
            let Some((k, iaa, q)) = find_sig(&x, p.mt) else { return };
            let amass = awr / awi;
            if p.mt == 102 && (izai == 1 || p.mf == 6) {
                // Neutron capture recoil, law 33 (`:10153-10175`). `aprime`
                // is upstream's `awp`, which this branch never sets: it holds
                // whatever the previous item left (`stale_awp`). For an
                // incident charged particle the MF=6 `mt102` subsection branch
                // (`:10287-10316`) is the same block with the subsection's own
                // AWP.
                x.n(last + 1, 33.0);
                x.n(next, 0.0);
                x.n(next + 1, 2.0);
                let (e1, e2) = (sigfig(x.get(esz + it - 1), 7, 0), sigfig(x.get(esz + nes - 1), 7, 0));
                x.r(next + 2, e1);
                x.r(next + 3, e2);
                x.r(next + 4, 1.0);
                x.r(next + 5, 1.0);
                next += 2 + 2 * 2;
                x.n(last + 2, (next - dlwh + 1) as f64);
                let awp_now = if izai == 1 {
                    stale_awp
                } else {
                    let sec = tape.section(mat, 6, p.mt).expect("checked");
                    nth_product(sec, p.l).expect("checked").0.head.c2
                };
                let aprime = awp_now / awi;
                x.r(next, sigfig((1.0 + amass) * (-q) / amass, 7, 0));
                x.r(next + 1, sigfig(amass * (amass + 1.0 - aprime) / (1.0 + amass).r_powi(2), 7, 0));
                next += 2;
                for ie in it..=nes {
                    let tt = x.get(esz + ie - 1) * x.get(hpd + 2 + ie - it) * awi / (awr + awi);
                    let h0 = x.get(hpd + 2 + naa + ie - it);
                    x.r(hpd + 2 + naa + ie - it, h0 + tt);
                }
            } else if p.mf == 4 {
                let awp = mf4_awp(p.mt);
                stale_awp = awp;
                let ltt = mf4_ltt(tape, mat, p.mt).unwrap_or(0);
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
                let d2 = sigfig(amass * (amass + 1.0 - aprime) / (1.0 + amass).r_powi(2), 7, 0);
                x.r(next, d1);
                x.r(next + 1, d2);
                // heating, LTT = 0 only: the others added theirs with the
                // angles (`:10198-10207`)
                for ie in (it..=nes).filter(|_| ltt == 0) {
                    let e = x.get(esz + ie - 1);
                    let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
                    let tt = d2 * (e - d1) * ss;
                    let h0 = x.get(hpd + 2 + naa + ie - it);
                    x.r(hpd + 2 + naa + ie - it, h0 + tt);
                }
                next += 2;
            } else {
                let sec = tape.section(mat, 6, p.mt).expect("checked");
                let (yt_l, law) = nth_product(sec, p.l).expect("checked");
                stale_awp = yt_l.head.c2;
                if law != 1 {
                    // LAW = 2 two-body, LAW = 3 isotropic two-body and LAW = 4
                    // recoil: law 33 (`:10289-10330`, `:10506-10515`,
                    // `:10567-10594`); LAW = 0: an empty entry, law 0.
                    let aprime = yt_l.head.c2 / awi;
                    let d = |x: &Xss| {
                        let _ = x;
                        (
                            sigfig((1.0 + amass) * (-q) / amass, 7, 0),
                            sigfig(amass * (amass + 1.0 - aprime) / (1.0 + amass).r_powi(2), 7, 0),
                        )
                    };
                    match law {
                        2 => {
                            let (_, _, lists) = nth_product_law2(sec, p.l).expect("checked");
                            let ne = lists.len();
                            x.n(last + 1, 33.0);
                            let lee = next;
                            x.n(next, 0.0);
                            x.n(next + 1, 2.0);
                            for i in 2..6 {
                                x.r(next + i, 0.0);
                            }
                            for (ie, l) in lists.iter().enumerate() {
                                let ee = sigfig(l.head.c2 / EMEV, 7, 0);
                                if ie == 0 {
                                    x.r(lee + 2, ee);
                                    x.r(lee + 4, 1.0);
                                } else if ie + 1 == ne {
                                    x.r(lee + 3, ee);
                                    x.r(lee + 5, 1.0);
                                }
                            }
                            next += 2 + 2 * 2;
                            x.n(last + 2, (next - dlwh + 1) as f64);
                            let (d1, d2) = d(&x);
                            x.r(next, d1);
                            x.r(next + 1, d2);
                            next += 2;
                        }
                        3 | 4 => {
                            x.n(last + 1, 33.0);
                            if law == 3 {
                                x.n(landh + jp - 1, 0.0);
                            }
                            x.n(next, 0.0);
                            x.n(next + 1, 2.0);
                            for i in 2..6 {
                                x.r(next + i, 0.0);
                            }
                            next += 2 + 2 * 2;
                            x.n(last + 2, (next - dlwh + 1) as f64);
                            let (d1, d2) = d(&x);
                            x.r(next, d1);
                            x.r(next + 1, d2);
                            if law == 3 {
                                for ie in it..=nes {
                                    let e = x.get(esz + ie - 1);
                                    let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
                                    let h0 = x.get(hpd + 2 + naa + ie - it);
                                    x.r(hpd + 2 + naa + ie - it, h0 + d2 * (e - d1) * ss);
                                }
                            }
                            next += 2;
                        }
                        6 => {
                            // Phase space, law 66 (`:10601-10742`).
                            let (_, apsx, npsx) = nth_product_law6(sec, p.l).expect("checked");
                            x.n(last + 1, 66.0);
                            x.n(next, 0.0);
                            x.n(next + 1, 2.0);
                            for i in 2..6 {
                                x.r(next + i, 0.0);
                            }
                            next += 2 + 2 * 2;
                            x.n(last + 2, (next - dlwh + 1) as f64);
                            x.n(next, f64::from(npsx));
                            x.r(next + 1, apsx);
                            x.n(next + 2, 2.0);
                            next += 3;
                            let (xs, ps, cs) = super::acelcp_ext::law6_grid(npsx);
                            let nn = xs.len();
                            x.n(next, nn as f64);
                            for n in 0..nn {
                                x.r(next + 1 + n, xs[n]);
                                x.r(next + nn + 1 + n, ps[n]);
                                x.r(next + 2 * nn + 1 + n, cs[n]);
                            }
                            // this law's part of the heating, from the grid as stored
                            let awp = yt_l.head.c2;
                            let ysx: Vec<f64> = (0..nn).map(|n| x.get(next + 1 + n)).collect();
                            let psx: Vec<f64> = (0..nn).map(|n| x.get(next + nn + 1 + n)).collect();
                            for ie in it..=nes {
                                let e = x.get(esz + ie - 1);
                                let h = super::acelcp_ext::law6_h(e, awi, awr, awp, apsx, q, &ysx, &psx);
                                let (y, _, _) = terpa(&yt_l.interp, &yt_l.pairs, e * EMEV);
                                let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
                                let naa_now = x.get(hpd + 1).round() as usize;
                                let h0 = x.get(hpd + 2 + naa_now + ie - it);
                                x.r(hpd + 2 + naa_now + ie - it, h0 + h * y * ss);
                            }
                            next += 1 + 3 * nn;
                        }
                        _ => {} // LAW = 0
                    }
                    continue;
                }
                let (yt, awp, lists, lang, lep) = nth_product_law1(sec, p.l, text).expect("checked");
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
                                super::acelf6::bachaa(izai, ip, za.round() as i32, ee, ep).unwrap_or(0.0)
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
            if ip == 1 {
                h = 0.0;
            }
            if izai > 1 {
                // The production heating also goes into ESZ's heating column
                // (`:10927-10929`).
                let eh0 = x.get(esz + 4 * nes + ie - 1);
                x.r(esz + 4 * nes + ie - 1, eh0 + h);
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

    if izai > 1 {
        recoil_heating(&mut x, tape, mat, izai, nes, esz, mtr, lqr, lsig, sig, ntr);
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

/// AWP `acelcp` uses for a two-body charged-particle level given in MF=4
/// (`awr1..awr5`, `:9645-9650`).
fn mf4_awp(mt: i32) -> f64 {
    match mt {
        600..=649 => AWR_P,
        650..=699 => AWR_D,
        700..=749 => AWR_T,
        750..=799 => AWR_HE3,
        800..=849 => AWR_A,
        _ => 1.0,
    }
}

/// Write one reaction's tabulated angular distribution as `acelcp` does for
/// MF=4 LTT = 1/2 and MF=6 LAW = 2 (`:9668-9748` and its copies), and return
/// the two-body heating table `(E [MeV], h)` built from each energy's mean
/// cosine. `small_cut` zeroes pdf values below 1e-30, as every copy but the
/// MF=4 Legendre one does. The `int = 1` mean-cosine sum keeps upstream's
/// `+ (mu_i + mu_{i-1})/2` term as written.
#[allow(clippy::too_many_arguments)]
fn write_angular(
    x: &mut Xss,
    next: &mut usize,
    land_slot: usize,
    andh: usize,
    rows: &[(f64, Vec<f64>, Vec<f64>, i32)],
    small_cut: bool,
    amass: f64,
    aprime: f64,
    q: f64,
) -> Vec<(f64, f64)> {
    let ne = rows.len();
    x.n(land_slot, (*next - andh + 1) as f64);
    x.n(*next, ne as f64);
    let ie0 = *next;
    let il = ie0 + ne;
    *next = il + ne + 1;
    let mut heat = Vec::with_capacity(ne);
    for (j, (e_ev, mu, pp, int)) in rows.iter().enumerate() {
        let iie = j + 1;
        x.r(ie0 + iie, sigfig(e_ev / EMEV, 7, 0));
        x.n(il + iie, -((*next - andh + 1) as f64));
        let n = mu.len();
        let nx = *next;
        x.n(nx, f64::from(*int));
        x.n(nx + 1, n as f64);
        let mut ubar = 0.0;
        for i in 1..=n {
            x.r(nx + 1 + i, sigfig(mu[i - 1], 7, 0));
            let mut pv = sigfig(pp[i - 1], 7, 0);
            if small_cut && pv < SMALL {
                pv = 0.0;
            }
            x.r(nx + 1 + n + i, pv);
            if i == 1 {
                x.r(nx + 1 + 2 * n + i, 0.0);
                ubar = 0.0;
            } else {
                let (m1, m0) = (x.get(nx + 1 + i), x.get(nx + i));
                let (p1, p0) = (x.get(nx + 1 + n + i), x.get(nx + n + i));
                let c0 = x.get(nx + 2 * n + i);
                if *int == 1 {
                    x.r(nx + 1 + 2 * n + i, sigfig(c0 + p0 * (m1 - m0), 7, 0));
                    ubar += p0 * (m1 - m0) + (m1 + m0) / 2.0;
                } else if *int == 2 {
                    x.r(nx + 1 + 2 * n + i, sigfig(c0 + (p1 + p0) * (m1 - m0) / 2.0, 7, 0));
                    ubar += (p1 + p0) * (m1 - m0) * (m1 + m0) / 4.0;
                }
            }
        }
        let renorm = 1.0 / x.get(nx + 1 + 3 * n);
        for i in 1..=n {
            let pv = x.get(nx + 1 + n + i);
            x.r(nx + 1 + n + i, sigfig(renorm * pv, 7, 0));
            let cv = x.get(nx + 1 + 2 * n + i);
            x.r(nx + 1 + 2 * n + i, sigfig(renorm * cv, 9, 0));
        }
        *next = nx + 2 + 3 * n;
        let e = x.get(ie0 + iie);
        let th = (1.0 + amass) * q / amass;
        let r1 = amass * (amass + 1.0 - aprime) / aprime;
        let r2 = aprime / (1.0 + amass).r_powi(2);
        let mut betasq = r1 * (1.0 + th / e);
        if betasq < 0.0 {
            betasq = 0.0;
        }
        heat.push((e, e * r2 * (betasq + 1.0 + 2.0 * ubar * betasq.sqrt())));
    }
    heat
}

/// Add `h(E) * sigma(E)` to the production heating, `h` lin-lin in the
/// table `write_angular` returned (`:9750-9760`).
#[allow(clippy::too_many_arguments)]
fn add_two_body_heating(x: &mut Xss, heat: &[(f64, f64)], hpd: usize, naa: usize, it: usize, nes: usize, esz: usize, k: usize, iaa: usize) {
    let hint = vec![(heat.len() as u32, 2)];
    for ie in it..=nes {
        let e = x.get(esz + ie - 1);
        let (h, _, _) = terpa(&hint, heat, e);
        let ss = if ie >= iaa { x.get(2 + k + ie - iaa) } else { 0.0 };
        let h0 = x.get(hpd + 2 + naa + ie - it);
        x.r(hpd + 2 + naa + ie - it, h0 + h * ss);
    }
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
            // LAW=7 reads as LAW=1 after `topfil`'s rewrite (`:2320`).
            return Some((y, if law == 7 { 1 } else { law }));
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    None
}

/// Subsection `l` of an MF=6 section, `LAW = 1`: yield, AWP, the LISTs,
/// LANG and LEP.
fn nth_product_law1(sec: &crate::endf::tape::Section, l: i32, text: bool) -> Option<(Tab1, f64, Vec<List>, i32, i32)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    for ik in 1..=head.n1 {
        let y = cur.read_tab1().ok()?;
        let law = y.head.l2;
        if ik == l {
            // `topfil` rewrites every LAW=7 subsection to LAW=1 on the
            // scratch tape `acelcp` reads (`acefc.f90:2320-2425`).
            if law == 7 {
                let (_, lists, lang, lep) = super::acelf6::law7_rewrite(&mut cur, text).ok()?;
                let awp = y.head.c2;
                return Some((y, awp, lists, lang, lep));
            }
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
    match nth_product(sec, p.l).map(|x| x.1) {
        Some(1) => matches!(nth_product_law1(sec, p.l, true), Some((_, _, _, lang, _)) if lang == 1 || lang == 2 || lang >= 11),
        // LAW = 2 two-body with Legendre or lin-lin tabulated angles; LAW = 3
        // isotropic two-body; LAW = 4 recoil; LAW = 0 (unknown: upstream
        // writes an empty law-0 entry, `:10289-10565`).
        Some(2) => nth_product_law2(sec, p.l).is_some_and(|(_, _, lists)| lists.iter().all(|l| l.head.l1 == 0 || l.head.l1 == 12)),
        Some(0 | 3 | 4) => true,
        // LAW = 6, phase space (`:10601-10742`).
        Some(6) => nth_product_law6(sec, p.l).is_some(),
        // LAW = 5 is not ported.
        _ => false,
    }
}

/// Subsection `l` of an MF=6 section, `LAW = 6`: the yield, `APSX` and `NPSX`
/// of the CONT record that follows it.
fn nth_product_law6(sec: &crate::endf::tape::Section, l: i32) -> Option<(Tab1, f64, i32)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    for ik in 1..=head.n1 {
        let y = cur.read_tab1().ok()?;
        let law = y.head.l2;
        if ik == l {
            if law != 6 {
                return None;
            }
            let c = cur.read_cont().ok()?;
            return Some((y, c.c1, c.n2));
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    None
}

/// Subsection `l` of an MF=6 section, `LAW = 2`: yield, AWP and the LISTs.
fn nth_product_law2(sec: &crate::endf::tape::Section, l: i32) -> Option<(Tab1, f64, Vec<List>)> {
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    for ik in 1..=head.n1 {
        let y = cur.read_tab1().ok()?;
        let law = y.head.l2;
        if ik == l {
            if law != 2 {
                return None;
            }
            let t2 = cur.read_tab2().ok()?;
            let mut lists = Vec::new();
            for _ in 0..t2.head.n2 {
                lists.push(cur.read_list().ok()?);
            }
            let awp = y.head.c2;
            return Some((y, awp, lists));
        }
        super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
    }
    None
}

/// MF=4/`mt`'s angular data as ACER's `acelcp` reads it: `(LTT, rows)`,
/// each row `(E [eV], mu, pdf, INT)` after `ptleg2` (LTT = 1) or `pttab2`
/// (LTT = 2). LTT = 0 and 3 give no rows (upstream writes none for them).
fn mf4_rows(tape: &Tape, mat: i32, mt: i32) -> Option<(i32, Vec<(f64, Vec<f64>, Vec<f64>, i32)>)> {
    let sec = tape.section(mat, 4, mt)?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    let ltt = head.l2;
    let mut rows = Vec::new();
    if ltt == 1 || ltt == 2 {
        let _ = cur.read_cont().ok()?; // 0, AWR, LI, LCT
        let t2 = cur.read_tab2().ok()?;
        for _ in 0..t2.head.n2 {
            if ltt == 1 {
                let l = cur.read_list().ok()?;
                let (mu, p) = super::acensd::ptleg2(&l.data).ok()?;
                rows.push((l.head.c2, mu, p, 2));
            } else {
                let t = cur.read_tab1().ok()?;
                let (mu, p) = super::acensd::pttab2(&t.pairs);
                let int = t.interp.first().map_or(2, |&(_, i)| i as i32);
                rows.push((t.head.c2, mu, p, int));
            }
        }
    }
    Some((ltt, rows))
}

/// Heating from the heavy recoils (`ZAP > 2004`) of the MF=6 reactions, for an
/// incident charged particle (`acelcp`, `acefc.f90:10962-11126`): each
/// `LAW = 1` recoil's mean outgoing energy, weighted by its yield, is
/// interpolated onto the ACE grid and added to ESZ's heating column.
///
/// Not ported (and refused by [`recoil_heating_supported`] before a table is
/// built): `LAW = 4` recoils, whose two-body kinematics branch
/// (`:11060-11110`) needs the first subsection re-read.
#[allow(clippy::too_many_arguments)]
fn recoil_heating(x: &mut Xss, tape: &Tape, mat: i32, _izai: i32, nes: usize, esz: usize, mtr: usize, _lqr: usize, lsig: usize, sig: usize, ntr: usize) {
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6 && s.key.mt > 2) {
        let mt = sec.key.mt;
        let mut ir = 0usize;
        let mut found = false;
        for i in 1..=ntr {
            if x.get(mtr + i - 1).round() as i32 == mt {
                ir = i;
                found = true;
                break;
            }
        }
        if !found {
            continue;
        }
        let k = x.get(lsig + ir - 1).round() as usize + sig - 1;
        let iaa = x.get(k).round() as usize;
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { continue };
        for _ in 0..head.n1 {
            let Ok(yt) = cur.read_tab1() else { break };
            let izap = yt.head.c1.round() as i32;
            let law = yt.head.l2;
            if izap > 2004 && law == 1 {
                let Ok(t2) = cur.read_tab2() else { break };
                let ne = t2.head.n2 as usize;
                let mut heat_tab: Vec<(f64, f64)> = Vec::with_capacity(ne);
                for _ in 0..ne {
                    let Ok(l) = cur.read_list() else { break };
                    let e = l.head.c2;
                    let np = l.head.n2 as usize;
                    let (y, _, _) = terpa(&yt.interp, &yt.pairs, e);
                    let (mut heat, mut epl, mut gl) = (0.0f64, 0.0f64, 0.0f64);
                    for ipp in 1..=np {
                        let ep = l.data.get(2 * (ipp - 1)).copied().unwrap_or(0.0);
                        let g = l.data.get(2 * (ipp - 1) + 1).copied().unwrap_or(0.0);
                        if ipp > 1 {
                            heat += (ep - epl) * gl * (ep + epl) / 2.0;
                        }
                        epl = ep;
                        gl = g;
                    }
                    heat_tab.push((e, y * heat));
                }
                let hint = vec![(heat_tab.len() as u32, 2)];
                for ie in iaa..=nes {
                    let e = x.get(esz + ie - 1) * EMEV;
                    let (h, _, _) = terpa(&hint, &heat_tab, e);
                    let h = (h / EMEV) * x.get(2 + k + ie - iaa) / x.get(esz + nes + ie - 1);
                    let prev = x.get(esz + 4 * nes + ie - 1);
                    x.r(esz + 4 * nes + ie - 1, sigfig(prev + h, 7, 0));
                }
            } else if super::energy::mf6::skip_mf6_subsection(&mut cur, law).is_err() {
                break;
            }
        }
    }
}

/// Whether [`recoil_heating`] covers every heavy recoil of the evaluation
/// (`LAW = 1` only; a `LAW = 4` recoil needs the unported kinematics branch).
pub fn recoil_heating_supported(tape: &Tape, mat: i32) -> bool {
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6 && s.key.mt > 2) {
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { return false };
        for _ in 0..head.n1 {
            let Ok(yt) = cur.read_tab1() else { return false };
            if yt.head.c1.round() as i32 > 2004 && yt.head.l2 == 4 {
                return false;
            }
            if super::energy::mf6::skip_mf6_subsection(&mut cur, yt.head.l2).is_err() {
                return false;
            }
        }
    }
    true
}
