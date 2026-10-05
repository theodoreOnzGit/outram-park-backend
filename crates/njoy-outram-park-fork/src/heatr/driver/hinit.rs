// Ported from NJOY2016 `src/heatr.f90` (`hinit`, lines 437-983).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `hinit`: everything HEATR settles before the first reaction is heated.
//!
//! 1. The ENDF dictionary sets the flags `nheat` branches on: photon data
//!    present (`mgam`), MT=19 and its spectrum (`mt19`), partial charged-
//!    particle levels (`mt103`..`mt107`, `mt16`), MF=6 reactions (`mt6`), and
//!    levels with neither MF=4 nor MF=6 (`miss4`, isotropy assumed).
//! 2. MF=1/MT=458 gives the fission energy components: the thermal point or
//!    polynomial (`cpoly`, `hpoly`), or the tabulated ones (`afr`, `anp`,
//!    `agp`), and the delayed part `qdel` taken out of fission Q.
//! 3. MF=6 is scanned for subsections whose recoil is not given
//!    (`mt6no`: the recoil is then generated from the heavy particle, or from
//!    the photon momentum for capture) and for photons (`mt6yp`).
//! 4. `hconvr` builds the working copy of the material.
//! 5. The energy grid is the PENDF's MT=1 grid, walked with `gety1` as
//!    upstream walks it: a discontinuity puts an extra point a factor
//!    `0.99999` below it.

use super::flat::{list_flat, nint, tab1_flat, tab2_flat};
use super::state::{Heatr, Mf6Section, Mf6Sub, MAXMF6};
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::NjoyError;

/// Read one MF=6 section into memory: its head and every subsection, with
/// the records `sixbar` will read, in tape order.
pub(super) fn parse_mf6(rows: &[[f64; 6]]) -> Result<Mf6Section, NjoyError> {
    let mut cur = SectionCursor::new(rows);
    let head = cur.read_cont()?;
    let mut out = Mf6Section {
        head: super::flat::cont_flat(&head),
        subs: Vec::new(),
    };
    for _ in 0..head.n1.max(0) {
        let y = cur.read_tab1()?;
        let law = y.head.l2;
        let mut sub = Mf6Sub {
            yld: tab1_flat(&y),
            law,
            tab2: Vec::new(),
            recs: Vec::new(),
        };
        match law {
            1 | 2 | 5 => {
                let t2 = cur.read_tab2()?;
                let ne = t2.head.n2.max(0);
                sub.tab2 = tab2_flat(&t2);
                for _ in 0..ne {
                    sub.recs.push(list_flat(&cur.read_list()?));
                }
            }
            6 => sub.recs.push(super::flat::cont_flat(&cur.read_cont()?)),
            7 => {
                let t2 = cur.read_tab2()?;
                let ne = t2.head.n2.max(0);
                sub.tab2 = tab2_flat(&t2);
                for _ in 0..ne {
                    let inner = cur.read_tab2()?;
                    let nmu = inner.head.n2.max(0);
                    let mut rec = tab2_flat(&inner);
                    for _ in 0..nmu {
                        rec.extend(tab1_flat(&cur.read_tab1()?));
                    }
                    sub.recs.push(rec);
                }
            }
            0 | 3 | 4 => {}
            _ => {
                return Err(NjoyError::NotPorted(
                    "hinit: MF=6 LAW < 0 (NJOY-internal formats)",
                ))
            }
        }
        out.subs.push(sub);
    }
    Ok(out)
}

impl Heatr {
    /// `hinit(iold, nend4, nend6, nscr)`. `pendf` is this temperature's
    /// sections of `matd`.
    pub(super) fn hinit(&mut self, endf: &Tape, pendf: &[Section]) -> Result<(), NjoyError> {
        const FACT: f64 = 0.99999;
        const RUP: f64 = 1.000000001;
        const BIG: f64 = 1.0e10;
        const MEVEV: f64 = 1.0e-6;
        const TOL: f64 = 1.0e-5;
        let matd = self.matd;

        // The dictionary.
        let mut lrel = 0;
        let mut nmod = 0;
        self.mt6 = vec![0];
        self.mt103 = 0;
        self.mt104 = 0;
        self.mt105 = 0;
        self.mt106 = 0;
        self.mt107 = 0;
        self.mt16 = 0;
        self.miss4 = vec![0];
        self.mgam = 0;
        let mut ifiss = 0;
        self.mt19 = 0;
        let mut i519 = 0;
        self.mt458 = 0;
        self.qdel = 0.0;
        self.jp = 0;
        self.jpn = 0;
        self.jpp = 0;
        self.etop = 20_000_000.0;
        self.ebot = 1.0 / 100_000.0;
        self.etabmax = -1.0;
        let s451 = endf
            .section(matd, 1, 451)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 1,
                mt: 451,
            })?;
        let r = &s451.rows;
        let n6 = nint(r[0][5]) as usize;
        let (iverf, hd) = if nint(r[1][4]) != 0 {
            (4, 1usize)
        } else if nint(r[1][5]) == 0 {
            (5, 2usize)
        } else {
            (6, 3usize)
        };
        self.iverf = iverf;
        if iverf == 6 {
            if r[2][1] > self.etop {
                self.etop = r[2][1];
            }
            lrel = nint(r[2][2]) as i32;
            nmod = nint(r[2][5]) as i32;
        }
        let nwd = nint(r[hd][4]) as usize;
        let nx = if iverf >= 5 {
            nint(r[hd][5]) as usize
        } else {
            n6
        };
        let dict: Vec<(i32, i32)> = r
            .iter()
            .skip(hd + 1 + nwd)
            .take(nx)
            .map(|row| (nint(row[2]) as i32, nint(row[3]) as i32))
            .collect();
        let iverf6 = iverf >= 6;
        for (i, &(mfd, mtd)) in dict.iter().enumerate() {
            if mfd == 2 {
                continue;
            }
            if (mfd == 12 && mtd != 460) || mfd == 13 {
                self.mgam = 1;
            }
            if mfd >= 13 {
                break;
            }
            if mfd <= 3 {
                if mfd == 1 && mtd == 458 {
                    self.mt458 = 1;
                }
                if mtd == 19 {
                    self.mt19 = 1;
                }
                if mtd == 18 || mtd == 19 {
                    ifiss = 1;
                }
                let ranges: [(i32, i32); 6] = if iverf6 {
                    [
                        (600, 650),
                        (650, 700),
                        (700, 750),
                        (750, 800),
                        (800, 850),
                        (875, 891),
                    ]
                } else {
                    [
                        (700, 720),
                        (720, 740),
                        (740, 760),
                        (760, 780),
                        (780, 800),
                        (i32::MAX, i32::MAX),
                    ]
                };
                for (k, &(lo, hi)) in ranges.iter().enumerate() {
                    if mtd >= lo && mtd < hi {
                        match k {
                            0 => self.mt103 = 1,
                            1 => self.mt104 = 1,
                            2 => self.mt105 = 1,
                            3 => self.mt106 = 1,
                            4 => self.mt107 = 1,
                            _ => self.mt16 = 1,
                        }
                    }
                }
                let (lo, hi) = if iverf6 { (600, 850) } else { (700, 800) };
                if mtd >= lo && mtd <= hi {
                    let found = dict[i..]
                        .iter()
                        .any(|&(mf4, mt4)| (mf4 == 4 || mf4 == 6) && mt4 == mtd);
                    if !found {
                        self.mess(
                            "hinit",
                            &format!("mf4 and 6 missing, isotropy assumed for mt {mtd:3}"),
                            " ",
                        );
                        self.miss4.push(mtd);
                    }
                }
            } else if mfd == 5 {
                if mtd == 19 {
                    i519 = 1;
                }
            } else if mfd == 6 {
                if self.i6() >= MAXMF6 {
                    return Err(NjoyError::EndfParse("hinit: too many mf6 reactions".into()));
                }
                self.mt6.push(mtd);
            }
        }
        if self.mt19 == 1 && i519 == 0 {
            self.mt19 = 2;
        }
        if self.mt19 == 1 {
            self.mess(
                "hinit",
                "mt18 is redundant",
                "19, 20, 21, ... will be used.",
            );
        }
        if self.mt19 == 2 {
            self.mess(
                "hinit",
                "mt19 has no spectrum",
                "mt18 spectrum will be used.",
            );
        }

        // MT=458: the fission energy components.
        if ifiss != 0 {
            self.c458.clear();
            self.cpoly.clear();
            self.hpoly.clear();
            self.afr.clear();
            self.anp.clear();
            self.agp.clear();
            if self.mt458 == 1 {
                self.read_mt458(endf, lrel, nmod, MEVEV, TOL)?;
            } else {
                self.lfc = 0;
                self.nply = 0;
                self.cpoly = vec![0.0];
                self.mess("hinit", "mt458 is missing for this mat", " ");
            }
        }

        // MF=6: missing recoils and photons.
        self.mt6yp = vec![0];
        self.mt6no = vec![0];
        self.mf6.clear();
        let mf6_secs: Vec<&Section> = endf
            .sections()
            .iter()
            .filter(|s| s.key.mat == matd && s.key.mf == 6)
            .collect();
        for s in mf6_secs {
            let mth = s.key.mt;
            let head = s.rows[0];
            if mth == 18 {
                self.jp = nint(head[2]) as i32;
                self.jpn = self.jp % 10;
                self.jpp = self.jp - 10 * self.jpn;
            }
            if mth == 18 && self.jp != 0 {
                // Delete MT=18 from the MF=6 list and skip the section.
                let i6t = self.i6();
                let mut i6 = i6t;
                let mut v = self.mt6.clone();
                for i in 1..=i6t {
                    if v[i] < 18 {
                        continue;
                    }
                    if v[i] == 18 {
                        i6 -= 1;
                    }
                    if v[i] > 18 && i6 != i6t {
                        v[i - 1] = v[i];
                    }
                }
                v.truncate(i6 + 1);
                self.mt6 = v;
                continue;
            }
            let sec = parse_mf6(&s.rows)?;
            let nk = sec.subs.len();
            self.mt6no.push(0);
            let ii6 = self.mt6no.len() - 1;
            let mut zar = head[0] + 1.0;
            let mut awrr = head[1] + 1.0;
            let ielem = nint(head[0]) % 1000;
            for (ik0, sub) in sec.subs.iter().enumerate() {
                let ik = ik0 + 1;
                let nr = nint(sub.yld[4]) as usize;
                let zap = sub.yld[0];
                let lf = nint(sub.yld[2]);
                if nint(zap) == 0 {
                    self.mt6yp.push(mth);
                }
                if mth == 5 {
                    zar = 0.0;
                }
                if mth != 5 {
                    let yld = sub.yld[6 + 2 * nr + 1];
                    zar -= zap * yld;
                    awrr -= sub.yld[1] * yld;
                    if zap == 0.0
                        && (ik > 1 || nk == 1)
                        && nk != 1
                        && zar != 0.0
                        && (ielem != 0 || zar.abs() >= 3000.0)
                    {
                        self.recoil_message(mth, zar);
                        self.mt6no[ii6] = ik - 1;
                        zar = 0.0;
                    }
                }
                if mth == 102 && zap > 0.0 && lf == 0 {
                    self.mess(
                        "hinit",
                        &format!("mf6, mt{mth:3} has recoil with no spectrum"),
                        "photon momentum recoil used.",
                    );
                    self.mt6no[ii6] = nk;
                }
                if zap == 0.0 {
                    self.mgam = 10 + self.mgam % 10;
                }
            }
            if zar != 0.0 && (ielem != 0 || zar.abs() >= 3000.0) {
                self.recoil_message(mth, zar);
                self.mt6no[ii6] = nk;
            }
            let _ = awrr;
            self.mf6.push((mth, sec));
        }
        if self.mgam == 0 {
            self.listing.push_str(
                "\n no photon production files...all photon energy will be deposited locally.\n",
            );
        }

        // The working copy of the material.
        self.conv = self.hconvr(endf)?;

        // The energy grid of the total cross section.
        let mt1 = pendf
            .iter()
            .find(|s| s.key.mf == 3 && s.key.mt == 1)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 3,
                mt: 1,
            })?;
        let mut cur = SectionCursor::new(&mt1.rows);
        cur.read_cont()?;
        let t = cur.read_tab1()?;
        let mut g = Gety1::new(&t);
        let npkk = self.npkk();
        let mut e = 0.0;
        let v = g.get(e);
        let mut enext = v.xnext * RUP;
        self.efirst = enext;
        let mut idnx = 0;
        self.rows.clear();
        loop {
            let test = enext * FACT;
            if idnx > 0 && test > e {
                enext = test;
            }
            e = enext;
            let v = g.get(e);
            enext = v.xnext;
            let mut c = vec![0.0; npkk + 1];
            c[1] = e;
            self.rows.push(c);
            idnx = i32::from(v.idis);
            if enext > BIG - BIG / 100.0 {
                break;
            }
        }
        self.ne = self.rows.len();
        self.elast = e;
        self.mt303 = 0;
        if self.npk >= 3 {
            for i in 3..=self.npk {
                if self.mtp[i] == 303 {
                    self.mt303 = i;
                }
            }
        }
        Ok(())
    }

    fn recoil_message(&mut self, mth: i32, zar: f64) {
        let l1 = format!("mf6, mt{mth:3} does not give recoil za={:6}", nint(zar));
        if mth == 102 {
            self.mess("hinit", &l1, "photon momentum recoil used.");
        } else if mth != 10 {
            self.mess("hinit", &l1, "one-particle recoil approx. used.");
        }
    }

    /// MF=1/MT=458 (`:618-776`).
    fn read_mt458(
        &mut self,
        endf: &Tape,
        lrel: i32,
        nmod: i32,
        mevev: f64,
        tol: f64,
    ) -> Result<(), NjoyError> {
        let matd = self.matd;
        let sec = endf
            .section(matd, 1, 458)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 1,
                mt: 458,
            })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        self.lfc = head.l2;
        let nfc = head.n2.max(0) as usize;
        self.ifc1 = 0;
        self.ifc2 = 0;
        self.ifc3 = 0;
        self.ifc4 = 0;
        self.ifc5 = 0;
        self.ifc6 = 0;
        let scr = list_flat(&cur.read_list()?);
        let sc = |k: usize| scr.get(k - 1).copied().unwrap_or(0.0);
        self.nply = nint(sc(4)) as usize;
        let nply = self.nply;
        if self.lfc == 0 {
            self.cpoly = vec![0.0; nply + 1];
            self.hpoly = vec![0.0; nply + 1];
        }
        let n458 = (nint(sc(5)) as usize).max(18 * (nply + 1));
        self.c458 = vec![0.0; n458 + 1];
        self.listing.push_str("\n fission energy components\n\n");
        for k in 1..=18 {
            self.c458[k] = sc(6 + k);
        }
        if self.lfc == 0 {
            let c = &self.c458;
            self.qdel = c[5] + c[9] + c[11];
            if nply >= 1 {
                for k in 19..=36 {
                    self.c458[k] = sc(6 + k);
                }
                if nply > 1 {
                    for n in 2..=nply {
                        let efix = if nmod != 7 || lrel != 1 {
                            1.0
                        } else {
                            mevev.powi(n as i32 - 1)
                        };
                        for k in 1 + 18 * n..=18 + 18 * n {
                            self.c458[k] = sc(6 + k) * efix;
                        }
                    }
                }
                self.listing.push_str(&format!(
                    "   fission products: polynomial of order {nply:2}\n   prompt neutrons : polynomial of order {nply:2}\n   prompt gammas   : polynomial of order {nply:2}\n"
                ));
            } else {
                self.listing.push_str("   fission products: thermal point\n   prompt neutrons : thermal point\n   prompt gammas   : thermal point\n");
            }
            for i in 0..=nply {
                let c = &self.c458;
                self.cpoly[i] = c[1 + i * 18] + c[3 + i * 18] + c[7 + i * 18];
                self.hpoly[i] = c[1 + i * 18] + c[7 + i * 18];
            }
        } else if self.lfc == 1 {
            self.qdel = 0.0;
            for _ in 0..nfc {
                let t = cur.read_tab1()?;
                let ifc = t.head.l2;
                let f = tab1_flat(&t);
                let nl = f.len();
                let last_x = f[nl - 2];
                if self.etabmax < 0.0 {
                    self.etabmax = last_x;
                } else if ((self.etabmax - last_x).abs() / self.etabmax) > tol {
                    return Err(NjoyError::EndfParse(format!(
                        "hinit: upper energy mismatch for ifc={ifc:2} in mt=458."
                    )));
                }
                let first_y = f[6 + 2 * t.interp.len() + 1];
                match ifc {
                    1 => {
                        self.ifc1 = 1;
                        self.afr = f;
                    }
                    2 => {
                        self.ifc2 = 1;
                        self.anp = f;
                    }
                    3 => {
                        self.ifc3 = 1;
                        self.qdel += first_y;
                    }
                    4 => {
                        self.ifc4 = 1;
                        self.agp = f;
                    }
                    5 => {
                        self.ifc5 = 1;
                        self.qdel += first_y;
                    }
                    6 => {
                        self.ifc6 = 1;
                        self.qdel += first_y;
                    }
                    _ => {}
                }
            }
            if self.ifc3 == 0 {
                self.qdel += self.c458[5];
            }
            if self.ifc5 == 0 {
                self.qdel += self.c458[9];
            }
            if self.ifc6 == 0 {
                self.qdel += self.c458[11];
            }
            let line = |on: i32, what: &str| {
                format!(
                    "   {what}: {}\n",
                    if on == 1 {
                        "tabulated values"
                    } else {
                        "thermal point"
                    }
                )
            };
            let s = line(self.ifc1, "fission products")
                + &line(self.ifc2, "prompt neutrons ")
                + &line(self.ifc4, "prompt gammas   ");
            self.listing.push_str(&s);
            if self.etabmax < 0.0 {
                return Err(NjoyError::EndfParse(
                    "nheat: no tabulated fission q components found in mt=458.".into(),
                ));
            }
        } else {
            return Err(NjoyError::EndfParse("hinit: bad LFC in mt=458.".into()));
        }
        Ok(())
    }
}
