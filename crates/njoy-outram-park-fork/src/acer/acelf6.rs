//! Faithful port of `acelf6` (`acefc.f90:7139-7947`) for `newfor = 1`: an
//! ENDF MF=6 neutron section to its complete ACE **DLW** entry, yields and
//! correlated energy-angle law included.
//!
//! # Why this replaced the Law 4 path for MF=6
//!
//! The crate used to write every MF=6 LAW=1 neutron reaction as **ACE law 4**
//! (energy only, the angular half dropped) behind one generic 9-word header
//! spanning the whole grid with probability 1. NJOY writes **law 44** (LANG=2,
//! Kalbach-Mann) or **law 61** (LANG=1 via `ptleg2`, LANG=11-13 tabulated),
//! preceded by the evaluation's own yield table, and chains several neutron
//! subsections through `LNW`. Measured 2026-09-26 against NJOY2016's own
//! tables: U-234 MT=16/17/37/91 (law 44) and U-235/U-238 MT=5/16/17/91 (law
//! 61) all differed, and U-235 MT=5's energy-dependent yield (`TYR = -101`)
//! came out as `-1`.
//!
//! # What is ported
//!
//! - both yield passes: constant integer yields (`TYR = ±ntyr`, doubled for
//!   MT=6-9 and 46-49) and the **generalized yield** table (`TYR = ±(100 +
//!   locator)`), including its union-grid walk, `sigfig(xn,7,-1)` shading at
//!   discontinuities and the `up = 1.00001` end nudges;
//! - LAW=1 with LANG=1 (law 61 through [`super::acensd::ptleg2`]), LANG=2
//!   (law 44, `bachaa` when `NA = 1`) and LANG=11-13 (law 61, tabulated);
//! - the `ismooth` low-energy extensions for LANG=2, both the `LEP = 1`
//!   histogram branch (with `fx = .8409` read as the **single-precision**
//!   literal it is) and the `LEP = 2` lin-lin branch, both only when the first
//!   outgoing energy is zero (NJOY2016 2016.80 `7d317f8`, gh:#1368; see
//!   [`ismooth_extend_low_energy`]);
//! - the `ep > e` patches for negative-Q reactions;
//! - LAW=6 (law 66, phase space) with its own grid;
//! - several neutron subsections, chained through `LNW`.
//!
//! ~~**Refused, by name:** LAW=7. For `newfor = 1`, `topfil` rewrites a LAW=7
//! subsection into a special TAB1 before `acelf6` reads it
//! (`acefc.f90:2485-2530`), and that rewrite is not ported.~~ **CORRECTED
//! 2026-11-09 (gh:#1309):** that reading was of the wrong branch. With
//! `newfor = 1` and `no7 = 1` (upstream's default) `topfil` rewrites LAW=7
//! into **LAW=1** (`acefc.f90:2320-2425`; the TAB1 at `:2485-2530` is the
//! `no7 = 0` route), and `acelf6` writes it as law 61. That rewrite is
//! ported as [`law7_rewrite`]; Be-9's MT=16 and charged particles now make a
//! table identical to NJOY2016's in every word
//! (`tests/msre_salt_nuclides_900k_vs_njoy2016.rs`).
//!
//! **Unexercised, stated rather than claimed:** LANG=11-13 (no held evaluation
//! uses it on neutrons), discrete lines (`ND > 0`), and multiple neutron
//! subsections. They are ported line for line; nothing here has compared them
//! with NJOY.

use petir::mathf::RealMath;
use crate::endf::interp::{terp1, terpa, IntLaw};
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Section;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const EMEV: f64 = 1.0e6;
const SMALL: f64 = 1.0e-30;
const ETOP: f64 = 1.0e10;
const UP: f64 = 1.00001;
const ELOW: f64 = 1.0e-5;

/// One XSS word of a DLW entry, before the entry's position in DLW is known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DlwWord {
    /// An integer word.
    Int(i64),
    /// A real word.
    Real(f64),
    /// A DLW-relative locator: `start + offset`, where `start` is the 1-based
    /// DLW-relative position of the entry's first word.
    Loc(usize),
}

/// The ACE `TYR` value an `acelf6` entry implies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Acelf6Tyr {
    /// A constant integer yield, sign carrying the frame.
    Fixed(i32),
    /// A generalized (energy-dependent) yield: `TYR = sign * (100 + L)`,
    /// `L` the DLW-relative locator of the yield table at the entry's start.
    Generalized {
        /// `+1` laboratory, `-1` centre of mass.
        sign: i32,
    },
}

/// A complete DLW entry as `acelf6` lays it out: an optional generalized-yield
/// table, then one or more laws chained through `LNW`.
#[derive(Debug, Clone, PartialEq)]
pub struct DlwEntry {
    /// The words, in XSS order.
    pub words: Vec<DlwWord>,
    /// Leading words belonging to the generalized-yield table (`0` for a
    /// constant yield). `LDLW` points just past them.
    pub yield_len: usize,
    /// The `TYR` this entry implies.
    pub tyr: Acelf6Tyr,
    /// ACE law number of the first law in the chain.
    pub law: i32,
    /// Whether `LAND` is `-1` for this reaction (MF=6 LAW=1 or 6,
    /// `acefc.f90:5846-5847`).
    pub correlated: bool,
}

impl DlwEntry {
    /// Resolve to `(value, is_integer)` words for an entry starting at the
    /// 1-based DLW-relative position `start`.
    pub fn resolve(&self, start: i32) -> Vec<(f64, bool)> {
        self.words
            .iter()
            .map(|w| match *w {
                DlwWord::Int(i) => (i as f64, true),
                DlwWord::Real(r) => (r, false),
                DlwWord::Loc(o) => ((start + o as i32) as f64, true),
            })
            .collect()
    }
}

/// One neutron subsection as `acelf6` sees it.
struct Sub {
    yield_tab: Tab1,
    law: i32,
    body: Body,
}

enum Body {
    /// An old-format (`newfor = 0`) law 7, as `fix6` + `topfil` leave it.
    Law7Old { interp: Vec<(u32, u32)>, energies: Vec<Law7Energy> },
    Law1 { lang: i32, lep: i32, interp: Vec<(u32, u32)>, incident: Vec<crate::endf::records::List> },
    Law6 { apsx: f64, npsx: i32 },
}

/// `acelf6` for reaction `mt`. A LAW=7 neutron subsection is read as
/// `topfil`'s LAW=1 rewrite ([`law7_rewrite`], gh:#1309); `Ok(None)` is no
/// longer returned for it.
///
/// `q_mev` is the reaction's stored `LQR` value; `iza` the material ZA (for
/// `bachaa`); `ismooth` ACER card 2's flag (default on).
///
/// # Errors
/// [`NjoyError::EndfParse`] for malformed records, a missing neutron
/// subsection or a law `acelf6` itself rejects.
pub fn acelf6(
    section: &Section,
    mt: i32,
    q_mev: f64,
    iza: i32,
    ismooth: bool,
) -> Result<Option<DlwEntry>, NjoyError> {
    let izai = 1.0_f64;
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let jp = head.l1;
    let jpn = jp.rem_euclid(10);
    let lct = head.l2.min(2);
    let nk = head.n1;

    // ── pass 1: select the neutron subsections, as `acelf6`'s first loop ───
    let mut subs: Vec<Sub> = Vec::new();
    let mut first_law: Option<i32> = None;
    let mut last_law_read = 0;
    let test = 1.0 / 1000.0;
    for ikk in 1..=nk {
        let yt = cur.read_tab1()?;
        let zap = yt.head.c1;
        // `topfil` (`acefc.f90:2320`) rewrites a LAW=7 neutron's law field
        // to 1 on its scratch tape (`newfor = 1`, `no7 = 1`), so every later
        // reader (`acelod`'s LAND rule, this routine's TYR sign) sees LAW=1.
        let is_law7_neutron = yt.head.l2 == 7 && (zap - izai).abs() <= test;
        let law = if is_law7_neutron { 1 } else { yt.head.l2 };
        last_law_read = law;
        if first_law.is_none() {
            first_law = Some(law);
        }
        if zap == 0.0 || zap - izai > test || jpn == 2 || (jpn == 1 && ikk > 1) {
            break;
        }
        if (zap - izai).abs() > test {
            super::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
            continue;
        }
        let body = match if is_law7_neutron { 7 } else { law } {
            1 => {
                let t2 = cur.read_tab2()?;
                let mut incident = Vec::with_capacity(t2.head.n2 as usize);
                for _ in 0..t2.head.n2 {
                    incident.push(cur.read_list()?);
                }
                Body::Law1 { lang: t2.head.l1, lep: t2.head.l2, interp: t2.interp, incident }
            }
            6 => {
                let c = cur.read_cont()?;
                Body::Law6 { apsx: c.c1, npsx: c.n2 }
            }
            7 => law7_as_law1(&mut cur)?,
            _ => {
                return Err(NjoyError::EndfParse(format!(
                    "acelf6: illegal law {law} for endf6 file6 neutrons (mt {mt})"
                )))
            }
        };
        subs.push(Sub { yield_tab: yt, law, body });
    }
    if subs.is_empty() {
        return Err(NjoyError::EndfParse(format!(
            "acelf6: could not find required mf6 subsection, outgoing 1 mt {mt}"
        )));
    }

    emit_dlw(&subs, mt, q_mev, iza, ismooth, lct, last_law_read, first_law)
}

/// The second half of `acelf6`: the yield words and one law per neutron
/// subsection, from the subsections [`acelf6`] (or [`acelf6_old_format`])
/// selected.
#[allow(clippy::too_many_arguments)]
fn emit_dlw(
    subs: &[Sub],
    mt: i32,
    q_mev: f64,
    iza: i32,
    ismooth: bool,
    mut lct: i32,
    last_law_read: i32,
    first_law: Option<i32>,
) -> Result<Option<DlwEntry>, NjoyError> {
    let test = 1.0 / 1000.0;
    // `ivar`: 0 constant integer, 1 energy-dependent, 2 non-integer.
    let mut ivar = 0;
    let mut eemx = 0.0;
    for s in subs {
        let pairs = &s.yield_tab.pairs;
        let jnt = s.yield_tab.interp.last().map_or(2, |&(_, i)| i);
        let mut nnn = pairs.len();
        if jnt == 1 {
            nnn -= 1;
        }
        if nnn >= 2 {
            for j in 1..nnn {
                if pairs[j].1 - pairs[j - 1].1 != 0.0 {
                    ivar = 1;
                }
            }
            eemx = pairs[nnn - 1].0;
        }
        if ivar == 0 && (pairs[0].1 - pairs[0].1.round()).abs() > test {
            ivar = 2;
        }
    }

    let mut words: Vec<DlwWord> = Vec::new();
    let mut yield_total = 0.0;
    let tyr;
    let mut gyl_table: Vec<(f64, f64)> = Vec::new();
    if ivar <= 0 {
        // Upstream indexes every subsection's first yield with the LAST
        // subsection's `m` (`ii=loc(j)+7+2*m`); with one NR per subsection
        // (every held evaluation) that is each subsection's own first yield.
        for s in subs {
            yield_total += s.yield_tab.pairs[0].1;
        }
        let mut ntyr = yield_total.round() as i32;
        if (6..=9).contains(&mt) || (46..=49).contains(&mt) {
            ntyr *= 2;
        }
        if mt == 18 {
            ntyr = 19;
            lct = 1;
        }
        tyr = if last_law_read == 6 {
            Acelf6Tyr::Fixed(-ntyr)
        } else {
            Acelf6Tyr::Fixed((3 - 2 * lct) * ntyr)
        };
    } else {
        tyr = Acelf6Tyr::Generalized { sign: 3 - 2 * lct };
        let mut xnext = 0.0;
        let mut k: i64 = -1;
        let mut rows: Vec<(f64, f64)> = Vec::new();
        while k <= 0 || xnext < ETOP {
            k += 1;
            let xx = xnext;
            let mut yy = 0.0;
            xnext = ETOP;
            for s in subs {
                let (y, mut xn, idis) = terpa(&s.yield_tab.interp, &s.yield_tab.pairs, xx);
                if k == 0 && xn < xnext {
                    xnext = xn;
                }
                if k != 0 {
                    yy += y;
                    if idis && xx < xn - xn / 10000.0 {
                        xn = sigfig(xn, 7, -1);
                    }
                    if xx < eemx {
                        xnext = xn;
                    }
                }
            }
            if k > 0 {
                rows.push((xx, sigfig(yy, 7, 0)));
                if 2 * k + 2 >= 500 {
                    return Err(NjoyError::EndfParse(
                        "acelf6: storage exceeded for generalized yield".into(),
                    ));
                }
            }
        }
        words.push(DlwWord::Int(0));
        words.push(DlwWord::Int(rows.len() as i64));
        for &(e, _) in &rows {
            words.push(DlwWord::Real(sigfig(e / EMEV, 7, 0)));
        }
        for &(_, y) in &rows {
            words.push(DlwWord::Real(sigfig(y, 7, 0)));
        }
        gyl_table = rows
            .iter()
            .map(|&(e, y)| (sigfig(e / EMEV, 7, 0), sigfig(y, 7, 0)))
            .collect();
    }
    let yield_len = words.len();

    // ── pass 2: one law per neutron subsection, chained through LNW ────────
    let mut prev_last: Option<usize> = None;
    for s in subs {
        if let Some(pl) = prev_last {
            words[pl] = DlwWord::Loc(words.len());
        }
        let last = words.len();
        prev_last = Some(last);
        let law_word = match s.law {
            1 => 44,
            7 => 67,
            _ => 66,
        };
        words.push(DlwWord::Int(0));
        words.push(DlwWord::Int(law_word));
        words.push(DlwWord::Int(0)); // IDAT, set below

        // Yield (law-applicability) table.
        let yt = &s.yield_tab;
        if ivar <= 0 {
            let m = yt.interp.len();
            let jnt = yt.interp.first().map_or(2, |&(_, i)| i);
            if m != 1 || jnt != 2 {
                words.push(DlwWord::Int(m as i64));
                for &(nbt, _) in &yt.interp {
                    words.push(DlwWord::Int(i64::from(nbt)));
                }
                for &(_, int) in &yt.interp {
                    words.push(DlwWord::Int(i64::from(int)));
                }
            } else {
                words.push(DlwWord::Int(0));
            }
            words.push(DlwWord::Int(yt.pairs.len() as i64));
            for &(x, _) in &yt.pairs {
                words.push(DlwWord::Real(sigfig(x / EMEV, 7, 0)));
            }
            for &(_, y) in &yt.pairs {
                words.push(DlwWord::Real(sigfig(y / yield_total, 7, 0)));
            }
        } else {
            let ngyl = gyl_table.len();
            words.push(DlwWord::Int(0));
            words.push(DlwWord::Int(ngyl as i64));
            let mut e_words = Vec::with_capacity(ngyl);
            let mut y_words = Vec::with_capacity(ngyl);
            for j in 0..ngyl {
                let mut eyl = gyl_table[j].0 * EMEV;
                if j == 0 {
                    eyl *= UP;
                }
                if j == ngyl - 1 {
                    eyl /= UP;
                }
                let mut gyl = gyl_table[j].1;
                if j == 0 && ngyl > 1 {
                    let (a, b) = (gyl_table[0], gyl_table[1]);
                    gyl = terp1(a.0, a.1, b.0, b.1, eyl / EMEV, IntLaw::LinLin)?;
                }
                if j == ngyl - 1 && ngyl > 1 {
                    let (a, b) = (gyl_table[ngyl - 2], gyl_table[ngyl - 1]);
                    gyl = terp1(a.0, a.1, b.0, b.1, eyl / EMEV, IntLaw::LinLin)?;
                }
                let (y, _, _) = terpa(&yt.interp, &yt.pairs, eyl);
                if j == 0 {
                    eyl /= UP;
                }
                if j == ngyl - 1 {
                    eyl *= UP;
                }
                e_words.push(DlwWord::Real(sigfig(eyl / EMEV, 7, 0)));
                let yy = if gyl > 0.0 { y / gyl } else { y };
                y_words.push(DlwWord::Real(sigfig(yy, 7, 0)));
            }
            words.extend(e_words);
            words.extend(y_words);
        }

        match &s.body {
            Body::Law6 { apsx, npsx } => {
                words[last + 2] = DlwWord::Loc(words.len());
                words.push(DlwWord::Int(i64::from(*npsx)));
                words.push(DlwWord::Real(*apsx));
                words.push(DlwWord::Int(2));
                law66_words(*npsx, &mut words);
            }
            Body::Law7Old { interp, energies } => {
                words[last + 2] = DlwWord::Loc(words.len());
                push_interp_header(interp, &mut words);
                let ne = energies.len();
                words.push(DlwWord::Int(ne as i64));
                for en in energies {
                    words.push(DlwWord::Real(sigfig(en.e / EMEV, 7, 0)));
                }
                let loc_base = words.len();
                for _ in 0..ne {
                    words.push(DlwWord::Int(0));
                }
                for (j, en) in energies.iter().enumerate() {
                    words[loc_base + j] = DlwWord::Loc(words.len());
                    law7_old_block(en, &mut words);
                }
            }
            Body::Law1 { lang, lep, interp, incident } => {
                let (lang, lep) = (*lang, *lep);
                if !(lang == 1 || lang == 2 || (11..=13).contains(&lang)) {
                    return Err(NjoyError::EndfParse(format!(
                        "acelf6: only lang=1,2,11-13 allowed for endf-6 file 6 neutrons (mt {mt})"
                    )));
                }
                if lang != 2 {
                    words[last + 1] = DlwWord::Int(61);
                }
                let law61 = lang != 2;
                words[last + 2] = DlwWord::Loc(words.len());
                let m = interp.len();
                let jnt0 = interp.first().map_or(2, |&(_, i)| (i % 10).min(2));
                if m != 1 || jnt0 != 2 {
                    words.push(DlwWord::Int(m as i64));
                    for &(nbt, _) in interp {
                        words.push(DlwWord::Int(i64::from(nbt)));
                    }
                    for &(_, int) in interp {
                        words.push(DlwWord::Int(i64::from((int % 10).min(2))));
                    }
                } else {
                    words.push(DlwWord::Int(0));
                }
                let ne = incident.len();
                words.push(DlwWord::Int(ne as i64));
                for l in incident {
                    words.push(DlwWord::Real(sigfig(l.head.c2 / EMEV, 7, 0)));
                }
                let loc_base = words.len();
                for _ in 0..ne {
                    words.push(DlwWord::Int(0)); // L(j), set below
                }
                for (j, l) in incident.iter().enumerate() {
                    words[loc_base + j] = DlwWord::Loc(words.len());
                    incident_block(l, lang, lep, law61, mt, q_mev, iza, ismooth, &mut words)?;
                }
            }
        }
    }

    Ok(Some(DlwEntry {
        words,
        yield_len,
        tyr,
        law: match subs[0].law {
            1 => match &subs[0].body {
                Body::Law1 { lang: 2, .. } => 44,
                _ => 61,
            },
            7 => 67,
            _ => 66,
        },
        correlated: matches!(first_law, Some(1) | Some(6)),
    }))
}

/// `topfil`'s rewrite of an MF=6 LAW=7 neutron subsection into LAW=1
/// (`acefc.f90:2330-2425`, taken when `newfor = 1` and `no7 = 1`, upstream's
/// default), read back as `acelf6` reads it.
///
/// Per incident energy: the union of every cosine's outgoing-energy grid
/// (`ordr`: sorted, near-duplicates within `1e-10` relative dropped), each
/// cosine's density evaluated on it (`terpa`), the cosine integral `ss`
/// (trapezoid, or histogram for `INTMU = 1`) stored as `f0`, and each
/// `f(mu)` divided by it (`1/2` where `ss = 0`). The TAB2 gets `LANG =
/// 10 + INTMU` and `LEP` = the last cosine's `INTEP` at the first incident
/// energy. Everything goes to the scratch tape, which is text for a
/// positive PENDF unit (`acefc.f90:177-179`), so every value here carries
/// `a11`'s rounding, as `acelf6` reads it.
fn law7_as_law1(cur: &mut SectionCursor) -> Result<Body, NjoyError> {
    let (interp, incident, lang, lep) = law7_rewrite(cur)?;
    Ok(Body::Law1 { lang, lep, interp, incident })
}

/// [`law7_as_law1`]'s rewrite as parts: the incident-energy interpolation,
/// the LAW=1 LISTs, `LANG` and `LEP`. `acelcp` reads charged-particle
/// LAW=7 products through it, as upstream's `acelcp` reads `topfil`'s
/// rewritten tape.
#[allow(clippy::type_complexity)]
pub(crate) fn law7_rewrite(
    cur: &mut SectionCursor,
) -> Result<(Vec<(u32, u32)>, Vec<crate::endf::records::List>, i32, i32), NjoyError> {
    use crate::endf::parse::{format_endf_float, parse_endf_float};
    use crate::endf::records::{Cont, List};
    let rt = |x: f64| parse_endf_float(&format_endf_float(x)).unwrap_or(x);
    let outer = cur.read_tab2()?;
    let ne = outer.head.n2.max(0) as usize;
    let mut incident = Vec::with_capacity(ne);
    let (mut lang, mut lep) = (12, 2);
    for ie in 0..ne {
        let t2 = cur.read_tab2()?;
        let ei = t2.head.c2;
        let intmu = t2.interp.first().map_or(2, |&(_, i)| i as i32);
        let nmu = t2.head.n2.max(0) as usize;
        let mut mus: Vec<Tab1> = Vec::with_capacity(nmu);
        let mut intep = 2;
        for _ in 0..nmu {
            let t = cur.read_tab1()?;
            intep = t.interp.first().map_or(2, |&(_, i)| i as i32);
            mus.push(t);
        }
        if ie == 0 {
            lang = 10 + intmu;
            lep = intep;
        }
        let mut grid: Vec<f64> = mus.iter().flat_map(|t| t.pairs.iter().map(|p| p.0)).collect();
        ordr(&mut grid);
        let mut data = Vec::with_capacity(grid.len() * (2 + 2 * nmu));
        for &ep in &grid {
            let ff: Vec<f64> = mus.iter().map(|t| terpa(&t.interp, &t.pairs, ep).0).collect();
            let mut ss = 0.0;
            for imu in 1..nmu {
                let dmu = mus[imu].head.c2 - mus[imu - 1].head.c2;
                if intmu == 1 {
                    ss += dmu * ff[imu - 1];
                } else {
                    ss += dmu * (ff[imu] + ff[imu - 1]) / 2.0;
                }
            }
            data.push(rt(ep));
            data.push(rt(ss));
            for imu in 0..nmu {
                data.push(rt(mus[imu].head.c2));
                data.push(rt(if ss != 0.0 { ff[imu] / ss } else { 0.5 }));
            }
        }
        let head = Cont {
            c1: 0.0,
            c2: rt(ei),
            l1: 0,
            l2: (2 * nmu) as i32,
            n1: data.len() as i32,
            n2: grid.len() as i32,
        };
        incident.push(List { head, data });
    }
    Ok((outer.interp, incident, lang, lep))
}

/// `ordr` (`acefc.f90:2550-2591`): sort ascending, then drop each value
/// within `1e-10` (relative to itself) of the one before. Upstream returns
/// unsorted for `n <= 2`.
fn ordr(x: &mut Vec<f64>) {
    if x.len() <= 2 {
        return;
    }
    x.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    let mut i = 1;
    while i < x.len() {
        if (x[i] - x[i - 1]).abs() <= 1.0e-10 * x[i] {
            x.remove(i);
        } else {
            i += 1;
        }
    }
}

/// The LAW=6 phase-space shape `acelf6` writes (`acefc.f90:7443-7488`), with
/// its own rounding. `law66_shape_table` builds the same grid for the reader
/// and does not round, so it is not reused here.
fn law66_words(npsx: i32, words: &mut Vec<DlwWord>) {
    let step1 = 10f64.r_powf(1.0 / 5.0);
    let step2 = 1.0 / 50.0;
    let test1 = 1.0 + 1.0 / 100_000.0;
    let test2 = 1.0 / 10.0 - 1.0 / 1_000_000.0;
    let test3 = 1.0 - 1.0 / 10_000.0;
    let mut xx = ELOW;
    let mut n = 1;
    while xx < test1 {
        n += 1;
        if xx < test2 {
            xx *= step1;
        } else {
            xx += step2;
        }
    }
    let nn = n;
    let mut x = vec![0.0_f64];
    let mut p = vec![0.0_f64];
    let mut c = vec![0.0_f64];
    let (mut xl, mut pl, mut yn) = (0.0_f64, 0.0_f64, 0.0_f64);
    xx = ELOW;
    let t = 1.0 / 10.0;
    let t = t - t / 10000.0;
    while xx < test1 {
        let pn = if xx > test3 {
            xx = 1.0;
            0.0
        } else {
            let rn = 3.0;
            xx.sqrt() * (1.0 - xx).r_powf(rn * f64::from(npsx) / 2.0 - 4.0)
        };
        yn += (xx - xl) * (pn + pl) / 2.0;
        x.push(sigfig(xx, 7, 0));
        p.push(pn);
        c.push(yn);
        xl = xx;
        pl = pn;
        if xx < t {
            xx *= step1;
        } else {
            xx += step2;
        }
    }
    let sum = yn;
    words.push(DlwWord::Int(nn as i64));
    words.extend(x.iter().map(|&v| DlwWord::Real(v)));
    words.extend(p.iter().map(|&v| DlwWord::Real(sigfig(v / sum, 7, 0))));
    words.extend(c.iter().map(|&v| DlwWord::Real(sigfig(v / sum, 9, 0))));
}

/// The `ismooth` low-energy sqrt(E) extensions of one LAW=1 incident-energy record, ported from
/// `acelf6` (`acefc.f90:7547-7657`, NJOY2016 2016.80 `ab27c64`): `d` is the record's
/// `(E', pdf, A..)` rows, `ncyc = NA + 2` words each, `n` the number of rows, `nd` the number of
/// discrete lines. Returns the new row count; `d` is edited in place. Only for LANG=2 neutron
/// spectra with no discrete lines (`zap = 1` is the caller's).
///
/// **2016.80 guard (gh:#1368, upstream commit `7d317f8`, "Fix bug in smoothing distributions
/// when outgoing energy grid does not start at 0").** Both branches run only when
/// `eout1 = scr(7)/emev` is zero, i.e. the first tabulated outgoing energy is exactly 0. This
/// port ran them unconditionally until 2026-11-10; without the guard the inserted rows precede a
/// first energy above them and the outgoing energies come out of order. Differential:
/// `tests/acelf6_ismooth_vs_njoy2016_units.rs`.
pub fn ismooth_extend_low_energy(
    d: &mut Vec<f64>,
    mut n: usize,
    ncyc: usize,
    nd: usize,
    lang: i32,
    lep: i32,
) -> usize {
    // `eout1=scr(7)/emev` (MeV): zero exactly when the first E' is zero.
    let eout1 = d[0] / EMEV;
    if lang == 2 && lep == 1 && nd == 0 && eout1 == 0.0 {
        // `fx=.8409` has no kind suffix: a default-real (single-precision)
        // literal promoted to double, exactly as in `acelf5`.
        let fx = 0.8409_f32 as f64;
        let ex = 40.0;
        let mut cx = d[ncyc] * d[1];
        while n > 2 {
            let cxx = cx + d[ncyc + 1] * (d[2 * ncyc] - d[ncyc]);
            if (cxx / d[2 * ncyc].r_powf(1.5) - cx / d[ncyc].r_powf(1.5)).abs()
                > cx / d[ncyc].r_powf(1.5) / 50.0
            {
                break;
            }
            d[1] = (d[1] * d[ncyc] + d[ncyc + 1] * (d[2 * ncyc] - d[ncyc])) / d[2 * ncyc];
            d.drain(ncyc..2 * ncyc);
            cx = cxx;
            n -= 1;
        }
        while d[ncyc] > ex {
            let row0: Vec<f64> = d[0..ncyc].to_vec();
            d.splice(0..0, row0);
            d[ncyc] = sigfig(fx * d[2 * ncyc], 6, 0);
            let val = d[1];
            d[1] = sigfig(fx.sqrt() * val, 6, 0);
            d[ncyc + 1] = sigfig((1.0 - fx * fx.sqrt()) * val / (1.0 - fx), 6, 0);
            n += 1;
        }
    } else if lang == 2 && lep == 2 && n > 3 && nd == 0 && eout1 == 0.0 {
        let ex = 40.0;
        let fx = 0.50;
        let mut nn = 0;
        if d[0] > ex {
            d.splice(0..0, std::iter::repeat(0.0).take(ncyc));
            n += 1;
        }
        while d[ncyc] > ex {
            nn += 1;
            let row1: Vec<f64> = d[ncyc..2 * ncyc].to_vec();
            d.splice(ncyc..ncyc, row1);
            d[ncyc] = sigfig(fx * d[2 * ncyc], 6, 0);
            for ii in 1..ncyc {
                d[ncyc + ii] = d[2 * ncyc + ii] * (d[ncyc] / d[2 * ncyc]).sqrt();
            }
            n += 1;
        }
        if nn > 0 {
            let mut cxx = 0.0;
            let (mut e1, mut p1) = (d[0], d[1]);
            for r in 1..n {
                let (e2, p2) = (d[r * ncyc], d[r * ncyc + 1]);
                cxx += (e2 - e1) * (p1 + p2) / 2.0;
                e1 = e2;
                p1 = p2;
            }
            if cxx != 0.0 {
                let cxx = 1.0 / cxx;
                for r in 1..n {
                    d[r * ncyc + 1] *= cxx;
                }
            }
        }
    }
    n
}

/// One incident energy of a LAW=1 subsection: `[INTT, N, E', pdf, cdf, R|LC,
/// A]` for law 44, `[INTT, N, E', pdf, cdf, LC]` plus the angular tables for
/// law 61 (`acefc.f90:7529-7892`).
#[allow(clippy::too_many_arguments)]
fn incident_block(
    list: &crate::endf::records::List,
    lang: i32,
    lep: i32,
    law61: bool,
    mt: i32,
    q_mev: f64,
    iza: i32,
    ismooth: bool,
    words: &mut Vec<DlwWord>,
) -> Result<(), NjoyError> {
    let ee = list.head.c2 / EMEV;
    let nd = list.head.l1 as usize;
    let na = list.head.l2 as usize;
    let ncyc = na + 2;
    let mut d: Vec<f64> = list.data.clone();
    let mut n = list.head.n2 as usize;

    // `ismooth` low-energy extensions, LANG=2 only, no discrete lines, first E' = 0.
    if ismooth {
        n = ismooth_extend_low_energy(&mut d, n, ncyc, nd, lang, lep);
    }

    let e = ee * EMEV;
    let mut eo = vec![0.0_f64; n];
    let mut pdf = vec![0.0_f64; n];
    let mut cdf = vec![0.0_f64; n];
    let mut c4 = vec![DlwWord::Real(0.0); n];
    let mut c5 = vec![0.0_f64; n];
    let mut angular: Vec<DlwWord> = Vec::new();
    // Where the angular tables start, relative to this block's first word.
    let ang_start = 2 + 4 * n;
    for ki in 1..=n {
        let r = ncyc * (ki - 1);
        let mut ep = d[r];
        if ep > e - e / 1000.0 && ki < n && mt != 5 && q_mev < 0.0 {
            ep = e - (n - ki) as f64 * 1000.0;
            d[r] = ep;
        } else if ep > e && ki == n && mt != 5 && q_mev < 0.0 {
            ep = e - (n - ki) as f64 * 1000.0;
            d[r] = ep;
        }
        eo[ki - 1] = sigfig(d[r] / EMEV, 7, 0);
        pdf[ki - 1] = if ki <= nd { d[r + 1] } else { sigfig(d[r + 1] * EMEV, 7, 0) };
        if pdf[ki - 1] > 0.0 && pdf[ki - 1] < SMALL {
            pdf[ki - 1] = SMALL;
        }
        let prev = if ki >= 2 { cdf[ki - 2] } else { 0.0 };
        if ki <= nd {
            cdf[ki - 1] = prev + d[r + 1];
        }
        if nd > 0 && ki == nd + 1 {
            cdf[ki - 1] = prev;
        }
        if ki > nd + 1 && lep == 1 {
            let rp = ncyc * (ki - 2);
            cdf[ki - 1] = prev + d[rp + 1] * (d[r] - d[rp]);
        }
        if ki > nd + 1 && lep == 2 {
            let rp = ncyc * (ki - 2);
            cdf[ki - 1] = prev + ((d[rp + 1] + d[r + 1]) / 2.0) * (d[r] - d[rp]);
        }
        let ep_mev = eo[ki - 1];

        if lang == 2 {
            c4[ki - 1] = DlwWord::Real(d[r + 2]);
            let aa = if na == 2 { d[r + 3] } else { bachaa(1, 1, iza, ee, ep_mev)? };
            c5[ki - 1] = sigfig(aa, 7, 0);
        } else if lang == 1 {
            let f0 = d[r + 1];
            let coeffs: Vec<f64> = (1..=na)
                .map(|ia| if f0 != 0.0 { d[r + 1 + ia] / f0 } else { 0.0 })
                .collect();
            let (mu, p) = super::acensd::ptleg2(&coeffs)?;
            c4[ki - 1] = DlwWord::Loc(ang_start + angular.len());
            let nmu = mu.len();
            angular.push(DlwWord::Int(2));
            angular.push(DlwWord::Int(nmu as i64));
            angular.extend(mu.iter().map(|&v| DlwWord::Real(sigfig(v, 7, 0))));
            angular.extend(p.iter().map(|&v| DlwWord::Real(sigfig(v, 7, 0))));
            let mut c = 0.0;
            for imu in 0..nmu {
                c = if imu == 0 {
                    0.0
                } else if imu == nmu - 1 {
                    1.0
                } else {
                    let del = mu[imu] - mu[imu - 1];
                    let av = (p[imu] + p[imu - 1]) / 2.0;
                    sigfig(c + del * av, 7, 0)
                };
                angular.push(DlwWord::Real(c));
            }
        } else {
            // LANG = 11-13: tabulated cosines, all three columns divided by
            // the running sum exactly as upstream writes them (`:7832-7869`).
            c4[ki - 1] = DlwWord::Loc(ang_start + angular.len());
            let intmu = lang - 10;
            let nmu = na / 2;
            let pair = |imu: usize| (d[r + 2 * imu], d[r + 2 * imu + 1]);
            let mut sum = 0.0;
            let mut col_mu = Vec::with_capacity(nmu);
            let mut col_p = Vec::with_capacity(nmu);
            let mut col_c = Vec::with_capacity(nmu);
            for imu in 1..=nmu {
                let (m_i, p_i) = pair(imu);
                col_mu.push(m_i);
                col_p.push(p_i);
                if imu == 1 {
                    sum = 0.0;
                    col_c.push(0.0);
                } else {
                    let (m_p, p_p) = pair(imu - 1);
                    let del = m_i - m_p;
                    if intmu == 1 {
                        sum += del * p_p;
                    } else {
                        sum += del * (p_i + p_p) / 2.0;
                    }
                    col_c.push(sum);
                }
            }
            angular.push(DlwWord::Int(i64::from(intmu)));
            angular.push(DlwWord::Int(nmu as i64));
            angular.extend(col_mu.iter().map(|&v| DlwWord::Real(sigfig(v / sum, 7, 0))));
            angular.extend(col_p.iter().map(|&v| DlwWord::Real(sigfig(v / sum, 7, 0))));
            angular.extend(col_c.iter().map(|&v| DlwWord::Real(sigfig(v / sum, 7, 0))));
        }
    }
    let renorm = if cdf[n - 1] != 0.0 { 1.0 / cdf[n - 1] } else { 1.0 };
    for k in 0..n {
        pdf[k] = sigfig(pdf[k] * renorm, 7, 0);
        cdf[k] = sigfig(cdf[k] * renorm, 9, 0);
    }

    // Block-relative locators become entry-relative here.
    let base = words.len();
    let rebase = |w: DlwWord| match w {
        DlwWord::Loc(o) => DlwWord::Loc(base + o),
        other => other,
    };
    words.push(DlwWord::Int((lep + 10 * nd as i32) as i64));
    words.push(DlwWord::Int(n as i64));
    words.extend(eo.iter().map(|&v| DlwWord::Real(v)));
    words.extend(pdf.iter().map(|&v| DlwWord::Real(v)));
    words.extend(cdf.iter().map(|&v| DlwWord::Real(v)));
    words.extend(c4.iter().map(|&w| rebase(w)));
    if law61 {
        words.extend(angular);
    } else {
        words.extend(c5.iter().map(|&v| DlwWord::Real(v)));
    }
    Ok(())
}

/// Port of `bachaa` (`acecm.f90:436-556`): the Kalbach-86 slope `a` with the
/// incident and emitted energies in **MeV**.
///
/// Not [`crate::groupr::kinematics::bach`], deliberately: that port takes eV
/// and folds `1e-6` into each energy (`ecm*tomev`), and builds `emc2` in a
/// different operation order, so the two differ in the last bits and ACE
/// stores `a` to 7 figures where such a difference can flip a digit.
///
/// # Errors
/// [`NjoyError::EndfParse`] when the target has no known dominant isotope.
pub fn bachaa(iza1i: i32, iza2: i32, izat: i32, e: f64, ep: f64) -> Result<f64, NjoyError> {
    use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
    const THIRD: f64 = 0.333_333_333;
    const TWOTH: f64 = 0.666_666_667;
    const FOURTH: f64 = 1.333_333_33;
    const C1: f64 = 15.68;
    const C2: f64 = -28.07;
    const C3: f64 = -18.56;
    const C4: f64 = 33.22;
    const C5: f64 = -0.717;
    const C6: f64 = 1.211;
    const S2: f64 = 2.22;
    const S3: f64 = 8.48;
    const S4: f64 = 7.72;
    const S5: f64 = 28.3;
    const B1: f64 = 0.04;
    const B2: f64 = 1.8e-6;
    const B3: f64 = 6.7e-7;
    const D1: f64 = 9.3;
    const EA1: f64 = 41.0;
    const EA2: f64 = 130.0;

    let emc2 = AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG / EMEV;
    let iza1 = if iza1i == 0 { 1 } else { iza1i };
    let iza = match izat {
        6000 => 6012,
        12000 => 12024,
        14000 => 14028,
        16000 => 16032,
        17000 => 17035,
        19000 => 19039,
        20000 => 20040,
        22000 => 22048,
        23000 => 23051,
        24000 => 24052,
        26000 => 26056,
        28000 => 28058,
        29000 => 29063,
        31000 => 31069,
        40000 => 40090,
        42000 => 42096,
        48000 => 48112,
        49000 => 49115,
        50000 => 50120,
        63000 => 63151,
        72000 => 72178,
        74000 => 74184,
        82000 => 82208,
        other => other,
    };
    let aa = f64::from(iza % 1000);
    if aa == 0.0 {
        return Err(NjoyError::EndfParse(format!(
            "bachaa: dominant isotope not known for {iza}"
        )));
    }
    let za = f64::from(iza / 1000);
    let ac = aa + f64::from(iza1 % 1000);
    let zc = za + f64::from(iza1 / 1000);
    let ab = ac - f64::from(iza2 % 1000);
    let zb = zc - f64::from(iza2 / 1000);
    let na = (aa - za).round();
    let nb = (ab - zb).round();
    let nc = (ac - zc).round();
    let sq = |x: f64| x * x;
    let mut sa = C1 * (ac - aa)
        + C2 * (sq(nc - zc) / ac - sq(na - za) / aa)
        + C3 * (ac.r_powf(TWOTH) - aa.r_powf(TWOTH))
        + C4 * (sq(nc - zc) / ac.r_powf(FOURTH) - sq(na - za) / aa.r_powf(FOURTH))
        + C5 * (sq(zc) / ac.r_powf(THIRD) - sq(za) / aa.r_powf(THIRD))
        + C6 * (sq(zc) / ac - sq(za) / aa);
    match iza1 {
        1002 => sa -= S2,
        1003 => sa -= S3,
        2003 => sa -= S4,
        2004 => sa -= S5,
        _ => {}
    }
    let mut sb = C1 * (ac - ab)
        + C2 * (sq(nc - zc) / ac - sq(nb - zb) / ab)
        + C3 * (ac.r_powf(TWOTH) - ab.r_powf(TWOTH))
        + C4 * (sq(nc - zc) / ac.r_powf(FOURTH) - sq(nb - zb) / ab.r_powf(FOURTH))
        + C5 * (sq(zc) / ac.r_powf(THIRD) - sq(zb) / ab.r_powf(THIRD))
        + C6 * (sq(zc) / ac - sq(zb) / ab);
    match iza2 {
        1002 => sb -= S2,
        1003 => sb -= S3,
        2003 => sb -= S4,
        2004 => sb -= S5,
        _ => {}
    }
    let ecm = aa * e / ac;
    let ea = ecm + sa;
    let eb = ep * ac / ab + sb;
    let x1 = if ea > EA2 { EA2 * eb / ea } else { eb };
    let x3 = if ea > EA1 { EA1 * eb / ea } else { eb };
    let fa = if iza1 == 2004 { 0.0 } else { 1.0 };
    let fb = if iza2 == 1 {
        0.5
    } else if iza2 == 2004 {
        2.0
    } else {
        1.0
    };
    let mut bb = B1 * x1 + B2 * x1.r_powi(3) + B3 * fa * fb * x3.r_powi(4);
    if iza1i == 0 {
        let mut fact = D1;
        if ep != 0.0 {
            fact /= ep.sqrt();
        }
        fact = fact.clamp(1.0, 4.0);
        bb *= (e / (2.0 * emc2)).sqrt() * fact;
    }
    Ok(bb)
}

// ─────────────────────────────────────────────────────────────────────────
// `newfor = 0`: `topfil`'s MF=6 conversion, `fix6`, and `acelf6`'s law 7
// (gh:#1449)
// ─────────────────────────────────────────────────────────────────────────

/// One cosine's outgoing-energy table after `fix6` and `topfil`'s law-7
/// normalisation: `y` is already divided by the table's own integral.
struct CosDist {
    cosine: f64,
    intep: i32,
    e_out: Vec<f64>,
    y: Vec<f64>,
}

/// One incident energy of an old-format law 7: `topfil`'s special TAB1 (only
/// its `INTMU`, `NMU` and energy are read back) and the per-cosine tables.
struct Law7Energy {
    e: f64,
    intmu: i32,
    cos: Vec<CosDist>,
}

/// The result of [`acelf6_old_format`]: the DLW entry and the angular law
/// `acensd` stores in AND for the same reaction.
pub struct OldFormat6 {
    /// The reaction's DLW entry (ACE law 67).
    pub entry: DlwEntry,
    /// The overall angular distribution per incident energy, as the old
    /// format's equally probable bins (isotropic energies empty).
    pub angular: super::angular::ElasticAngular,
}

fn push_interp_header(interp: &[(u32, u32)], words: &mut Vec<DlwWord>) {
    let m = interp.len();
    let jnt0 = interp.first().map_or(2, |&(_, i)| (i % 10).min(2));
    if m != 1 || jnt0 != 2 {
        words.push(DlwWord::Int(m as i64));
        for &(nbt, _) in interp {
            words.push(DlwWord::Int(i64::from(nbt)));
        }
        for &(_, int) in interp {
            words.push(DlwWord::Int(i64::from((int % 10).min(2))));
        }
    } else {
        words.push(DlwWord::Int(0));
    }
}

/// `acelf6`'s law-7 branch for one incident energy (`acefc.f90:7878-7946`):
/// `INTMU`, `NMU`, the cosines and the locators of their tables, then each
/// table as `INTEP`, `NP`, outgoing energy, pdf, cdf, renormalised by the
/// cdf's last value.
fn law7_old_block(en: &Law7Energy, words: &mut Vec<DlwWord>) {
    let nmu = en.cos.len();
    words.push(DlwWord::Int(i64::from(en.intmu)));
    words.push(DlwWord::Int(nmu as i64));
    let mus = words.len();
    for c in &en.cos {
        words.push(DlwWord::Real(c.cosine));
    }
    for _ in 0..nmu {
        words.push(DlwWord::Int(0));
    }
    for (i, c) in en.cos.iter().enumerate() {
        words[mus + nmu + i] = DlwWord::Loc(words.len());
        let npep = c.e_out.len();
        words.push(DlwWord::Int(i64::from(c.intep)));
        words.push(DlwWord::Int(npep as i64));
        let mut cdf = vec![0.0f64; npep];
        for ki in 1..npep {
            let dx = c.e_out[ki] - c.e_out[ki - 1];
            if c.intep == 1 {
                cdf[ki] = cdf[ki - 1] + c.y[ki - 1] * dx;
            }
            if c.intep == 2 {
                cdf[ki] = cdf[ki - 1] + (c.y[ki - 1] + c.y[ki]) * dx / 2.0;
            }
        }
        let renorm = 1.0 / cdf[npep - 1];
        for &e in &c.e_out {
            words.push(DlwWord::Real(sigfig(e / EMEV, 7, 0)));
        }
        for &y in &c.y {
            words.push(DlwWord::Real(sigfig(renorm * sigfig(y * EMEV, 7, 0), 7, 0)));
        }
        for &v in &cdf {
            words.push(DlwWord::Real(sigfig(renorm * v, 9, 0)));
        }
    }
}

/// `fix6` (`acefc.f90:3351-3570`) for one LIST (one incident energy) of a
/// LAW=1 subsection: the 33 cosines (2 when only `b_0` is given), and for each
/// the outgoing-energy table reconstructed from the Legendre series (or the
/// tabulated pairs), in the laboratory frame, with the cm-to-lab Jacobian when
/// `lct != 1`. `flat` is the LIST as `fix6` holds it: `[C1, C2, L1, L2, N1,
/// N2, data...]`, so `flat[k]` is upstream's `a(l1+k)`.
fn fix6_energy(flat: &[f64], lang: i32, lct: i32, awr: f64) -> Result<Vec<(f64, Vec<(f64, f64)>)>, NjoyError> {
    use crate::groupr::kinematics::shared::legndr;
    let ein = flat[1];
    let nep = flat[5].round() as usize;
    let nw = flat[4].round() as usize;
    let ncyc = nw / nep;
    let nl = ncyc - 1;
    let ncos = 33usize;
    let dmu = 2.0 / (ncos as f64 - 1.0);
    let amu: Vec<f64> = (0..ncos).map(|i| -1.0 + i as f64 * dmu).collect();
    let nmu = if nl == 1 { 2 } else { ncos };
    let mut out = Vec::with_capacity(nmu);
    for imu in 1..=nmu {
        let acos = if nl == 1 {
            if imu == 1 { -1.0 } else { 1.0 }
        } else {
            amu[imu - 1]
        };
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for iep in 1..=nep {
            let off = ncyc * (iep - 1);
            let ep = flat[6 + off];
            let mut csn = acos;
            let mut elb = ep;
            let mut drv = 1.0f64;
            if lct != 1 {
                let aw1 = awr + 1.0;
                let mut clb = csn;
                let mut cmn = -1.0f64;
                let mut qq = 1.0 - aw1 * aw1 * ep / ein;
                if qq > 0.0 {
                    cmn = qq.sqrt();
                }
                if clb < cmn {
                    clb = cmn;
                    if imu < nmu && amu[imu] <= cmn {
                        drv = 0.0;
                    }
                }
                qq = ep - ein * (1.0 - clb * clb) / (aw1 * aw1);
                if qq < 0.0 {
                    qq = 0.0;
                }
                elb = clb * ein.sqrt() / aw1 + qq.sqrt();
                elb *= elb;
                if ep > 0.0 {
                    csn = clb * (elb / ep).sqrt() - (ein / ep).sqrt() / aw1;
                }
                if csn < -1.0 {
                    csn = -1.0;
                }
                if csn > 1.0 {
                    csn = 1.0;
                }
                if ep != 0.0 {
                    drv *= (elb / ep).sqrt();
                }
            }
            let fmu = if lang <= 10 {
                let p = legndr(csn, nl);
                let mut f = 0.0f64;
                for ll in 1..=nl {
                    f += ((2 * ll - 1) as f64) * p[ll - 1] * flat[6 + off + ll] / 2.0;
                }
                f
            } else {
                // `fix6` indexes the pairs from `a(l1+5+2*kmu+off)`, one word
                // before the first (mu, f) pair (which follows `f0`): ported
                // as written, not corrected.
                let mmu = (ncyc - 1) / 2;
                let mut f = 0.0f64;
                for kmu in 1..mmu {
                    let ll = 5 + 2 * kmu + off;
                    if ll + 3 < flat.len() && csn >= flat[ll] && csn <= flat[ll + 2] {
                        f = terp1(flat[ll], flat[ll + 1], flat[ll + 2], flat[ll + 3], csn, IntLaw::from_code((lang - 10) as u32))?;
                    }
                }
                f
            };
            match pts.last() {
                None => pts.push((elb, fmu * drv)),
                Some(&(le, _)) if elb > le => pts.push((elb, fmu * drv)),
                _ => {}
            }
        }
        if pts.len() == 1 {
            pts.push((2.0 * pts[0].0, 0.0));
        }
        out.push((acos, pts));
    }
    Ok(out)
}

/// `acelf6` for `newfor = 0` (the old ACE angular format) on an MF=6 section
/// whose neutron subsection is LAW=1 with `LANG != 2`: `topfil` first converts
/// the section with `fix6` to LAW=7 with 33 laboratory cosines
/// (`acefc.f90:2233-2350`, `:3351-3570`), then reduces each incident energy's
/// cosine table to its overall angular law (`pttab`, equally probable bins,
/// `:2490-2530`), and `acelf6` writes the result as ACE law 67
/// (`:7878-7946`); the section's `LCT` is forced to 1 (laboratory).
///
/// `Ok(None)` when `topfil` would not convert (no neutron subsection, or
/// `LANG = 2`): the caller takes the ordinary [`acelf6`]. Values go through no
/// text round-trip, as for binary units (`nsix = -|nsix|`, `mscr < 0`); a
/// text unit would round them with `a11`, which is not applied here.
///
/// # Errors
/// `NotPorted` when a converted subsection is not LAW=1; the record errors of
/// [`acelf6`]; `pttab`'s.
pub fn acelf6_old_format(
    section: &Section,
    mt: i32,
    q_mev: f64,
    iza: i32,
    ismooth: bool,
) -> Result<Option<OldFormat6>, NjoyError> {
    let test = 1.0 / 1000.0;
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?;
    let (jp, lct_raw, awr, nk) = (head.l1, head.l2, head.c2, head.n1);
    // `topfil`'s scan up to the neutron subsection.
    let (mut ik, mut dzap, mut new6) = (0, 1.0f64, false);
    while dzap > test && ik < nk {
        ik += 1;
        let yt = cur.read_tab1()?;
        dzap = (yt.head.c1 - 1.0).abs();
        let lf = yt.head.l2;
        if lf == 1 {
            let t2 = cur.read_tab2()?;
            if dzap <= test && t2.head.l1 != 2 {
                new6 = true;
            }
            for _ in 0..t2.head.n2 {
                cur.read_list()?;
            }
        } else {
            super::energy::mf6::skip_mf6_subsection(&mut cur, lf)?;
        }
    }
    if !new6 {
        return Ok(None);
    }
    // `fix6` over the first `ik` subsections, then `topfil`/`acelf6` on them.
    let mut cur = SectionCursor::new(&section.rows);
    cur.read_cont()?;
    let jpn = jp.rem_euclid(10);
    let mut subs: Vec<Sub> = Vec::new();
    let mut angular = Vec::new();
    for ikk in 1..=ik {
        let mut yt = cur.read_tab1()?;
        if yt.head.l2 != 1 {
            return Err(NjoyError::NotPorted("fix6 on a subsection that is not LAW=1"));
        }
        yt.head.l2 = 7;
        let t2 = cur.read_tab2()?;
        let (lang, lep) = (t2.head.l1, t2.head.l2);
        let ne = t2.head.n2.max(0) as usize;
        let mut energies = Vec::with_capacity(ne);
        let mut ang_e = Vec::with_capacity(ne);
        for _ in 0..ne {
            let l = cur.read_list()?;
            let mut flat = vec![l.head.c1, l.head.c2, f64::from(l.head.l1), f64::from(l.head.l2), f64::from(l.head.n1), f64::from(l.head.n2)];
            flat.extend_from_slice(&l.data);
            let ein = l.head.c2;
            let tabs = fix6_energy(&flat, lang, lct_raw, awr)?;
            let mut cos = Vec::with_capacity(tabs.len());
            let mut fpairs = Vec::with_capacity(tabs.len());
            for (acos, pts) in tabs {
                let interp = [(pts.len() as u32, lep as u32)];
                let f = crate::endf::interp::intega(&interp, &pts, 0.0, 1.0e9)?;
                let y: Vec<f64> = pts.iter().map(|p| if f != 0.0 { p.1 / f } else { p.1 }).collect();
                fpairs.push((acos, f));
                cos.push(CosDist { cosine: acos, intep: lep, e_out: pts.iter().map(|p| p.0).collect(), y });
            }
            let edges = crate::acer::photon_angular::pttab(&[(fpairs.len() as u32, 2)], &fpairs)?;
            ang_e.push(super::acensd::old_format_entry(ein, edges));
            energies.push(Law7Energy { e: ein, intmu: 2, cos });
        }
        let zap = yt.head.c1;
        if zap == 0.0 || zap - 1.0 > test || jpn == 2 || (jpn == 1 && ikk > 1) {
            break;
        }
        if (zap - 1.0).abs() > test {
            continue;
        }
        angular = ang_e;
        subs.push(Sub { yield_tab: yt, law: 7, body: Body::Law7Old { interp: t2.interp, energies } });
    }
    if subs.is_empty() {
        return Err(NjoyError::EndfParse(format!("acelf6: could not find required mf6 subsection, outgoing 1 mt {mt}")));
    }
    let entry = emit_dlw(&subs, mt, q_mev, iza, ismooth, 1, 7, Some(7))?
        .ok_or_else(|| NjoyError::EndfParse("acelf6 old format: no entry".into()))?;
    Ok(Some(OldFormat6 { entry, angular: super::angular::ElasticAngular { energies: angular, lct: 1 } }))
}
