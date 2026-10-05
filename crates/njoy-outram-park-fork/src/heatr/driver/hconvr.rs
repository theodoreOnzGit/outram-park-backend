// Ported from NJOY2016 `src/heatr.f90` (`hconvr`, lines 4553-5045).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `hconvr`: the copy of the material HEATR works from (`nscr`, `nend4`).
//! It differs from the evaluation in three places:
//!
//! - every MF=12 LO=2 section (transition probabilities) becomes an LO=1
//!   section of constant yields, one per photon line, sorted by descending
//!   photon energy, with a total-yield TAB1 when there is more than one line;
//! - MF=14 for the discrete levels MT=51-90 becomes an isotropic head once an
//!   LO=2 section has been converted;
//! - MF=1/MT=456 (prompt ν̄) is added as a copy of MT=452 when the evaluation
//!   has neither MT=455 nor MT=456.
//!
//! Before that it checks that the discrete-level MTs of MF=3 run without
//! gaps (a gap is an error upstream) and notes gaps in MF=12.
//!
//! `acer::photon_blocks::Lo2Cascade` does the same matrix algebra for ACER's
//! `convr`; this is HEATR's routine, translated as it stands, including the
//! level energies taken from MF=3 `−QI` and overwritten by MF=12's `ES`, and
//! the transition matrices kept across MTs.

use super::flat::{list_flat, nint, rows_of_list, rows_of_tab1, tab1_flat};
use super::state::Heatr;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::endf::EndfKey;
use crate::NjoyError;

const IMAX: usize = 50;
const LMAX: usize = 500;

/// Whether `mt` opens or continues a discrete-level sequence (`:4602-4639`).
fn level_start(mt: i32, iverf: i32) -> bool {
    mt == 51
        || (iverf >= 6 && matches!(mt, 601 | 651 | 701 | 751 | 801 | 876))
        || (iverf <= 5 && matches!(mt, 701 | 721 | 741 | 761 | 781))
}

fn level_continue(mt: i32, iverf: i32) -> bool {
    let r = |lo: i32, hi: i32| mt > lo && mt < hi;
    r(51, 91)
        || (iverf >= 6
            && (r(601, 648)
                || r(651, 698)
                || r(701, 748)
                || r(751, 798)
                || r(801, 848)
                || r(876, 890)))
        || (iverf <= 5 && (r(701, 718) || r(721, 738) || r(741, 758) || r(761, 778) || r(781, 798)))
}

impl Heatr {
    /// `hconvr(nendf, nend4, nscr)` for material `matd` of `endf`.
    pub(super) fn hconvr(&mut self, endf: &Tape) -> Result<Tape, NjoyError> {
        let matd = self.matd;
        let iverf = self.iverf;
        let secs: Vec<&Section> = endf
            .sections()
            .iter()
            .filter(|s| s.key.mat == matd)
            .collect();

        // MT continuity checks, MF=3 then MF=12, in file order.
        let mut mtq: Vec<i32> = Vec::new();
        let mut eeq: Vec<f64> = Vec::new();
        let mut mtnow = 0;
        let mut mtmess = false;
        for s in &secs {
            let (mf, mt) = (s.key.mf, s.key.mt);
            if mf > 12 {
                break;
            }
            if mf != 3 && mf != 12 {
                continue;
            }
            if level_start(mt, iverf) {
                mtnow = mt;
            } else if level_continue(mt, iverf) {
                if mtnow + 1 != mt {
                    if mf == 3 {
                        mtmess = true;
                        self.mess("hconvr", &format!("mf3, mt{:2} is missing", mt - 1), "");
                    } else {
                        self.mess(
                            "hconvr",
                            &format!("mf12, mt{:2} may be missing", mt - 1),
                            "discrete photon data may be incomplete",
                        );
                    }
                }
                mtnow = mt;
            }
            if mf == 3 {
                mtq.push(mt);
                eeq.push(s.rows.get(1).map_or(0.0, |r| r[1]));
            }
        }
        if mtmess {
            return Err(NjoyError::EndfParse(
                "hconvr: missing mf3 mt's, probable endf error".into(),
            ));
        }

        let mut out: Vec<Section> = Vec::with_capacity(secs.len() + 1);
        let mut ngam = [0usize; 300];
        let mut l2flg = false;
        let mut mt0 = 0i32;
        let mut mt0old = 0i32;
        let mut e = vec![0.0f64; IMAX + 1];
        let mut aa = vec![0.0f64; IMAX * IMAX + 1];
        let mut r = vec![0.0f64; IMAX * IMAX + 1];
        let mut istor = false;
        let mut i = 0usize;
        while i < secs.len() {
            let s = secs[i];
            let (mf, mt) = (s.key.mf, s.key.mt);
            if mf == 1 {
                // 550: MT=451 is copied, then MT=456 is added if needed.
                out.push(s.clone());
                i += 1;
                let mut j = i;
                while j < secs.len() && secs[j].key.mf == 1 {
                    j += 1;
                }
                let mf1 = &secs[i..j];
                if let Some(first) = mf1.first() {
                    if first.key.mt == 452 {
                        out.push((*first).clone());
                        let nu = &first.rows;
                        let lnu = nint(nu[0][3]) as i32;
                        let mut no455 = false;
                        let mut inserted_at: Option<usize> = None;
                        let mut stop = false;
                        for (k, s1) in mf1.iter().enumerate().skip(1) {
                            if s1.key.mt == 456 {
                                stop = true;
                                break;
                            }
                            if s1.key.mt == 455 {
                                no455 = true;
                            }
                            if s1.key.mt > 456 && !no455 {
                                inserted_at = Some(k);
                                break;
                            }
                        }
                        let insert = !stop && !no455;
                        let at = inserted_at.unwrap_or(mf1.len());
                        for (k, s1) in mf1.iter().enumerate().skip(1) {
                            if insert && k == at {
                                out.push(mt456(matd, s1.rows[0][0], s1.rows[0][1], lnu, nu));
                            }
                            out.push((*s1).clone());
                        }
                        if insert && at == mf1.len() {
                            out.push(mt456(matd, 0.0, 0.0, lnu, nu));
                        }
                    } else {
                        for s1 in mf1 {
                            out.push((*s1).clone());
                        }
                    }
                }
                i = j;
                continue;
            }
            if mf == 12 && mt != 460 && s.rows.first().map_or(0, |r| nint(r[2])) == 2 {
                // 150: convert transition probabilities to yields.
                if !istor {
                    istor = true;
                    e[1] = 0.0;
                    for k in 1..=IMAX {
                        r[IMAX * (k - 1) + k] = 1.0;
                    }
                }
                let mut cur = SectionCursor::new(&s.rows);
                let head = cur.read_cont()?;
                let (za, awr, lg) = (head.c1, head.c2, head.l2 as usize);
                l2flg = true;
                let list = cur.read_list()?;
                let scr = list_flat(&list);
                let sc = |k: usize| scr.get(k - 1).copied().unwrap_or(0.0);
                if (51..=91).contains(&mt) && mt0 != 49 {
                    mt0 = 49;
                }
                if iverf >= 6 {
                    for (lo, hi, base) in [
                        (601, 649, 599),
                        (651, 699, 649),
                        (701, 749, 699),
                        (751, 799, 749),
                        (801, 849, 799),
                        (876, 891, 874),
                    ] {
                        if mt >= lo && mt <= hi && mt0 != base {
                            mt0 = base;
                        }
                    }
                } else {
                    for (lo, hi, base) in [
                        (701, 719, 699),
                        (721, 739, 719),
                        (741, 759, 739),
                        (761, 779, 759),
                        (781, 799, 779),
                    ] {
                        if mt >= lo && mt <= hi && mt0 != base {
                            mt0 = base;
                        }
                    }
                }
                if mt0 != mt0old {
                    e.iter_mut().for_each(|v| *v = 0.0);
                    mt0old = mt0;
                    let m1 = mt0 + 2;
                    let m2 = if mt0 == 49 {
                        91
                    } else if iverf >= 6 && mt0 != 874 {
                        m1 + 48
                    } else if iverf >= 6 {
                        m1 + 15
                    } else {
                        m1 + 17
                    };
                    let mut nn = 0usize;
                    let mut mttst = -1;
                    while mttst < m2 && nn < mtq.len() {
                        mttst = mtq[nn];
                        if mttst >= m1 && mttst <= m2 {
                            e[(mttst - mt0) as usize] = -eeq[nn];
                        }
                        nn += 1;
                    }
                }
                let j = usize::try_from(mt - mt0).map_err(|_| {
                    NjoyError::NotPorted("hconvr: LO=2 outside a discrete-level range")
                })?;
                if j == 0 || j > IMAX {
                    return Err(NjoyError::NotPorted(
                        "hconvr: LO=2 outside a discrete-level range",
                    ));
                }
                e[j] = sc(1);
                let n = nint(sc(6)) as usize;
                let jm1 = j - 1;
                let ix = |a: usize, b: usize| (a - 1) * IMAX + b;
                for kk in 1..=jm1 {
                    let k = jm1 - kk + 1;
                    let mut ii = 0usize;
                    let mut idone = false;
                    let mut ifound = 0usize;
                    while ii < n && !idone {
                        ii += 1;
                        ifound = ii;
                        let ei = sc(6 + (lg + 1) * ii - lg);
                        if ei == 0.0 && e[k] == 0.0 {
                            idone = true;
                        } else if ei != 0.0 && ((ei - e[k]) / ei).abs() <= 0.0001 {
                            idone = true;
                        }
                    }
                    if !idone {
                        aa[ix(k, j)] = 0.0;
                        r[ix(k, j)] = 0.0;
                    } else {
                        let p = sc(7 + (lg + 1) * ifound - lg);
                        let mut g = 1.0;
                        if lg == 2 {
                            g = sc(6 + (lg + 1) * ifound);
                        }
                        aa[ix(k, j)] = p * g;
                        r[ix(k, j)] = p;
                    }
                    if k != jm1 {
                        for ii in k + 1..=jm1 {
                            r[ix(k, j)] += r[ix(ii, j)] * aa[ix(k, ii)];
                        }
                    }
                }
                let mut eg = Vec::new();
                let mut es = Vec::new();
                let mut y = Vec::new();
                let mut ysum = 0.0;
                for ia in 2..=j {
                    let ja = j + 2 - ia;
                    let eja = e[ja];
                    for ib in 1..=jm1 {
                        let jb = j - ib;
                        let ejb = e[jb];
                        let yy = aa[ix(jb, ja)] * r[ix(ja, j)];
                        if yy > 0.0 {
                            if eg.len() >= LMAX {
                                return Err(NjoyError::EndfParse(
                                    "hconvr: too many lo=2 gammas".into(),
                                ));
                            }
                            eg.push(eja - ejb);
                            es.push(eja);
                            y.push(yy);
                            ysum += yy;
                        }
                    }
                }
                e[1] = 0.0;
                let l = eg.len();
                // Descending photon energy, by upstream's exchange sort.
                for a in 0..l.saturating_sub(1) {
                    for b in a + 1..l {
                        if eg[a] < eg[b] {
                            eg.swap(a, b);
                            y.swap(a, b);
                            es.swap(a, b);
                        }
                    }
                }
                let mtl = if iverf <= 5 && mt >= 700 {
                    649
                } else if iverf >= 6 && mt >= 600 {
                    549
                } else {
                    49
                };
                if let Some(slot) = ngam.get_mut((mt - mtl) as usize) {
                    *slot = l;
                }
                let mut rows = vec![[za, awr, 1.0, 0.0, l as f64, 0.0]];
                let (ebot, etop) = (self.ebot, self.etop);
                if l > 1 {
                    rows.extend(rows_of_tab1(&[
                        0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, ebot, ysum, etop, ysum,
                    ]));
                }
                for k in 0..l {
                    rows.extend(rows_of_tab1(&[
                        eg[k], es[k], 0.0, 2.0, 1.0, 2.0, 2.0, 2.0, ebot, y[k], etop, y[k],
                    ]));
                }
                out.push(Section { key: s.key, rows });
                i += 1;
                continue;
            }
            if (matd == 1149 && mf == 13 && mt >= 51 && iverf < 5)
                || (matd == 1150 && mf == 13 && mt <= 54 && iverf < 5)
            {
                // 140: the gamma production patch.
                self.mess(
                    "hconvr",
                    &format!("gamma prod patch made for mt {mt:3}"),
                    " ",
                );
                i += 1;
                continue;
            }
            if mf == 14 && l2flg && (51..=90).contains(&mt) {
                // 500: isotropic, NK from the converted section.
                let h = s.rows[0];
                out.push(Section {
                    key: s.key,
                    rows: vec![[h[0], h[1], 1.0, 0.0, ngam[(mt - 50) as usize] as f64, 0.0]],
                });
                i += 1;
                continue;
            }
            out.push(s.clone());
            i += 1;
        }
        Ok(Tape::from_sections(endf.tpid.clone(), out))
    }
}

/// MF=1/MT=456, a copy of MT=452's data under a head whose `C1, C2` are
/// whatever record upstream had just read (`:4991-5004`).
fn mt456(matd: i32, c1: f64, c2: f64, lnu: i32, nu452: &[[f64; 6]]) -> Section {
    let mut rows = vec![[c1, c2, 0.0, f64::from(lnu), 0.0, 0.0]];
    let mut cur = SectionCursor::new(nu452);
    let _ = cur.read_cont();
    if lnu == 1 {
        if let Ok(l) = cur.read_list() {
            rows.extend(rows_of_list(&list_flat(&l)));
        }
    } else if let Ok(t) = cur.read_tab1() {
        rows.extend(rows_of_tab1(&tab1_flat(&t)));
    }
    Section {
        key: EndfKey {
            mat: matd,
            mf: 1,
            mt: 456,
        },
        rows,
    }
}
