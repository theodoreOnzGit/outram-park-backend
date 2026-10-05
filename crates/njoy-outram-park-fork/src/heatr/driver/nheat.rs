// Ported from NJOY2016 `src/heatr.f90` (`nheat`, lines 984-1665).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `nheat`: the heating from the neutron files, reaction by reaction over
//! the PENDF's MF=3 sections, accumulated on `hinit`'s grid.
//!
//! Each reaction deposits `σ·(E + q0 − yld·Ē')`, with `Ē'` from `disbar`
//! (two-body), `conbar` (MF=5 continuum) or nothing (capture and other
//! particle-only exits), and `q0` the deposited Q: 0 for an elastic or level
//! reaction (`QM` when `LR` flags a breakup), `QI` for a continuum or other
//! reaction, `QM` for a charged-particle level, the user's when given, and for
//! fission the MT=458 components at the incident energy. A reaction with MF=6
//! data is taken subsection by subsection (`sixbar`): each particle's
//! `yld·Ē'` is added to a running balance, a charged particle's or recoil's
//! is deposited, a neutron's is not, a photon's only with `local = 1`, and
//! after the last subsection whatever of `E + q0` the subsections did not
//! account for is deposited (`ebal6`). When MF=6 gives no recoil, a final
//! pass generates one from the heavy particle (`irec`).
//!
//! Partial kermas, damage MTs and the kinematic limits (`kchk`) are
//! accumulated in the same pass, in the columns `hinit` laid out.

use super::flat::{nint, terpa};
use super::sixbar::SixIo;
use super::state::{indx, Heatr, ILMAX};
use crate::acer::fortran_fmt::fortran_e;
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `1x,1p,Ne14.4`.
pub(super) fn line14(vals: &[f64]) -> String {
    let mut s = String::from(" ");
    for &v in vals {
        s.push_str(&fortran_e(v, 4, 14));
    }
    s.push('\n');
    s
}

/// Whether `nheat` skips reaction `mth` (`heatr.f90:1072-1093`).
fn skipped(h: &Heatr, mth: i32) -> bool {
    let iv5 = h.iverf <= 5;
    matches!(mth, 3 | 4 | 10 | 26 | 27 | 101)
        || (mth == 18 && h.mt19 == 1)
        || ((19..=21).contains(&mth) && h.mt19 == 2)
        || (mth == 38 && h.mt19 == 2)
        || (mth == 103 && h.mt103 > 0)
        || (mth == 104 && h.mt104 > 0)
        || (mth == 105 && h.mt105 > 0)
        || (mth == 106 && h.mt106 > 0)
        || (mth == 107 && h.mt107 > 0)
        || (mth == 16 && h.mt16 > 0)
        || (iv5 && matches!(mth, 719 | 739 | 759 | 779 | 799))
        || (mth > 120 && mth < 152)
        || (mth > 200 && mth < 600)
}

impl Heatr {
    /// `nheat(iold, inew, nscr, nend4, nend6, local)`.
    pub(super) fn nheat(&mut self, endf: &Tape, pendf: &[Section]) -> Result<(), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const BIG: f64 = 1.0e10;
        const EFIS: f64 = 15.0e6;
        const FQ1: f64 = 8.07e6;
        const FQ2: f64 = 1.307;
        const QTEST: f64 = 99.0e6;
        const SMIN: f64 = 1.0e-9;
        const BREAK1: f64 = 1.0e6;
        const BREAK2: f64 = 2.0e6;
        const BREAK3: f64 = 5.0e6;
        const BREAK4: f64 = 20.0e6;
        const STEP1: f64 = 0.2e6;
        const STEP2: f64 = 0.5e6;
        const STEP3: f64 = 1.0e6;
        const STEP4: f64 = 5.0e6;
        const UP: f64 = 1.1;
        const TOL: f64 = 1.0e-5;

        let awr = self.awr;
        let awfac = 1.0 / awr;
        let aw1fac = 1.0 / (awr + 1.0);
        let npkk = self.npkk();
        let npk = self.npk;
        let mut npkkk = 0usize;
        for ipk in 3..=npk {
            if self.mtp[ipk] == 442 {
                npkkk = ipk;
            }
        }
        self.idame = 0;
        let mut ipk = 2;
        while self.idame == 0 && ipk < npk {
            ipk += 1;
            if self.mtp[ipk] >= 444 {
                self.idame = 1;
            }
        }
        let (mut n6, mut j6) = (0usize, 0usize);
        self.i6g = 0;
        let (mut irec, mut jrec) = (0usize, 0usize);
        let (mut last6, mut new6) = (0, 0);
        let mut qsave = 0.0;
        let mut pnue = 0.0;
        let mut io = SixIo::default();
        let mut yld = 0.0f64;
        let mut ebar = 0.0f64;
        let mut dame = 0.0f64;

        let secs: Vec<&Section> = pendf
            .iter()
            .filter(|s| s.key.mf == 3 && s.key.mt != 1)
            .collect();
        let mut si = 0usize;
        while si < secs.len() {
            let sec = secs[si];
            let mth = sec.key.mt;
            if skipped(self, mth) {
                si += 1;
                continue;
            }
            self.zat = sec.rows[0][0];
            self.awrt = sec.rows[0][1];
            self.izat = nint(self.zat);

            // 120: the reaction's cross section.
            let mut cur = SectionCursor::new(&sec.rows);
            cur.read_cont()?;
            let tab = cur.read_tab1()?;
            let mut g = Gety1::new(&tab);
            let thresh = sigfig(g.get(0.0).xnext, 7, 0);
            let t = tab.head.c1;
            self.q = tab.head.c2;
            let mut qendf = self.q;
            let lr = tab.head.l2;
            let mut qs = 0.0;
            let mut q0 = 0.0;
            let mut h0 = 0.0;
            let mut icon = 0i32;
            ebar = 0.0;
            dame = 0.0;
            let mtd = mth;
            let fission = (18..=21).contains(&mtd) || mtd == 38;

            // An ENDF-6 File 6.
            let mut has6 = j6 > 0;
            if !has6 && self.mt6[1..].contains(&mtd) {
                // 121
                let s6 = self
                    .mf6
                    .iter()
                    .find(|(m, _)| *m == mtd)
                    .map(|(_, s)| s.clone())
                    .ok_or(NjoyError::SectionNotFound {
                        mat: self.matd,
                        mf: 6,
                        mt: mtd,
                    })?;
                n6 = nint(s6.head[4]) as usize;
                self.lct = nint(s6.head[3]) as i32;
                self.jp = nint(s6.head[2]) as i32;
                self.jpn = self.jp % 10;
                self.jpp = self.jp - 10 * self.jpn;
                if mth == 18 && self.jpn != 0 {
                    si += 1;
                    continue;
                }
                self.six.sec = s6;
                self.six.pos = 0;
                self.i6g += 1;
                has6 = true;
            }
            if has6 {
                // 123
                j6 += 1;
                if irec > 0 && self.mt6no[self.i6g] > 0 {
                    j6 -= 1;
                }
                last6 = 0;
                if j6 == n6 && irec > 0 {
                    last6 = 1;
                }
                if j6 == n6 && self.mt6no[self.i6g] == 0 {
                    last6 = 1;
                }
                new6 = 0;
                if j6 == 1 && irec == 0 {
                    new6 = 1;
                }
            }

            // 124: fission Q at E = 0, prompt.
            if fission {
                if self.mt458 == 1 {
                    qendf = self.c458[15];
                    self.q = qendf - self.qdel;
                    let l1 = format!(
                        "changed q from {} to {}",
                        fortran_e(qendf, 6, 14),
                        fortran_e(self.q, 6, 14)
                    );
                    let l2 = format!("for mt {mth:3} by taking out delayed components");
                    self.mess("nheat", &l1, &l2);
                } else {
                    self.q = qendf;
                    if self.cpoly.is_empty() {
                        self.cpoly.push(0.0);
                    }
                    self.cpoly[0] = self.q;
                }
            }

            // The Q value, the heating routine, the neutron yield.
            let mut iimt = 0usize;
            for i in 1..=self.nqa {
                if self.mta[i] == mth {
                    qs = self.qa[i];
                    iimt = i;
                    break;
                }
            }
            let q = self.q;
            let za = self.za;
            let mut skip_to_291 = false;
            if j6 > 0 {
                // 175: File 6.
                self.sixbar(0.0, &mut io, mth, j6, &mut irec, &mut jrec)?;
                ebar = io.ebar;
                yld = io.yld;
                dame = io.dame;
                if ebar < 0.0 {
                    skip_to_291 = true;
                } else {
                    icon = -1;
                    q0 = t;
                    if iimt != 0 {
                        q0 = qs;
                    }
                    if fission {
                        q0 = q;
                    }
                }
            } else if mth > 15 && mth < 51 {
                // 150, 160
                q0 = q;
                if iimt != 0 {
                    q0 = qs;
                }
                icon = 2;
                self.conbar(0.0, &mut ebar, &mut dame, mtd, &mut yld, q, endf)?;
            } else if mth > 100 {
                // 170
                let charged_range = (self.iverf <= 5 && (700..=799).contains(&mth))
                    || (self.iverf >= 6 && (600..=849).contains(&mth));
                if !charged_range {
                    q0 = q;
                    if self.idame > 0 {
                        self.capdam(0.0, &mut dame, q, za, awr, mtd);
                    }
                } else {
                    // 171
                    let cont = (self.iverf <= 5 && matches!(mth, 718 | 738 | 758 | 778 | 798))
                        || (self.iverf >= 6 && matches!(mth, 649 | 699 | 749 | 799 | 849));
                    if !cont {
                        icon = 1;
                        if self.iverf <= 5 && matches!(mth, 700 | 720 | 740 | 760 | 780) {
                            qsave = q;
                        }
                        q0 = if self.iverf <= 5 { qsave } else { t };
                        yld = 1.0;
                        self.disbar(0.0, &mut ebar, &mut dame, q, za, awr, mtd)?;
                    } else {
                        // 174
                        yld = 1.0;
                        q0 = if self.iverf <= 5 { qsave } else { t };
                        if self.idame > 0 {
                            self.capdam(0.0, &mut dame, q, za, awr, mtd);
                        }
                    }
                }
            } else {
                icon = 1;
                q0 = 0.0;
                if lr != 0 && lr != 31 {
                    q0 = t;
                }
                if iimt != 0 {
                    q0 = qs;
                }
                if mth == 91 {
                    icon = 2;
                    self.conbar(0.0, &mut ebar, &mut dame, mtd, &mut yld, q, endf)?;
                } else {
                    yld = 1.0;
                    if matches!(lr, 16 | 21 | 24 | 26 | 30) {
                        yld = 2.0;
                    }
                    if matches!(lr, 17 | 25 | 38) {
                        yld = 3.0;
                    }
                    if lr == 37 {
                        yld = 4.0;
                    }
                    self.disbar(0.0, &mut ebar, &mut dame, q, za, awr, mtd)?;
                }
            }

            if !skip_to_291 {
                // 180
                let imt = if npk > 2 {
                    indx(npk, &self.mtp, self.mt19, mtd, self.iverf)
                } else {
                    Vec::new()
                };
                if icon < 0 {
                    // 179
                    self.izap = nint(self.zap);
                    if irec > 0 {
                        if self.izat % 1000 == 0 {
                            self.izap = 1000 * (self.izat / 1000 - self.izap / 1000);
                        } else {
                            self.izap = nint(self.zat + 1.0 - self.zap);
                        }
                    }
                }
                if self.iprint == 1 {
                    self.nheat_header(icon, mtd, q0, qendf, qs, fission);
                }
                // 182
                let (mut ipx, mut irx) = (2usize, 1usize);
                let (mut ipfr, mut irfr, mut ipnp, mut irnp, mut ipgp, mut irgp) =
                    (2usize, 1usize, 2usize, 1usize, 2usize, 1usize);
                let mut elst = 0.0;
                let mut ilist = 1usize;
                if icon == 0 {
                    yld = 0.0;
                }
                let mut yld0 = 0.0;
                let ne = self.ne;
                for nn in 1..=ne {
                    let mut c = std::mem::take(&mut self.rows[nn - 1]);
                    let e = sigfig(c[1], 9, 0);
                    if new6 == 1 {
                        c[npkk] = 0.0;
                    }
                    if mtd == 2 && !(e < elst && nn < ne) {
                        if ilist > ILMAX {
                            return Err(NjoyError::EndfParse("nheat: storage exceeded.".into()));
                        }
                        self.elist[ilist] = e;
                        self.elist[ilist + 1] = BIG;
                        if e < BREAK1 {
                            let lg = (e * (1.0 + SMALL)).log10();
                            let mut iii = (lg + SMALL) as i32;
                            if lg < 0.0 {
                                iii -= 1;
                            }
                            let xxx = 10f64.powi(iii);
                            if 2.0 * xxx > e * (1.0 + SMALL) && e >= 1.0 - SMALL {
                                elst = 2.0 * xxx;
                            } else if 5.0 * xxx > e * (1.0 + SMALL) && e >= 1.0 - SMALL {
                                elst = 5.0 * xxx;
                            } else {
                                elst = 10.0 * xxx;
                            }
                        } else {
                            let up_step = |step: f64| step * ((e / step + UP) as i64) as f64;
                            elst = if e >= sigfig(BREAK4, 9, 1) {
                                up_step(STEP4)
                            } else {
                                up_step(STEP3)
                            };
                            if e < sigfig(BREAK3, 9, 1) {
                                elst = up_step(STEP2);
                            }
                            if e < sigfig(BREAK2, 9, 1) {
                                elst = up_step(STEP1);
                            }
                        }
                        elst -= elst / 10_000_000.0;
                    }
                    // 193
                    if e >= thresh {
                        let y = g.get(e).y;
                        if iimt > 0 && self.qa[iimt] >= QTEST {
                            let lq0 = self.lqs[iimt];
                            let (qv, _, _) = terpa(&self.qbar[lq0..], e, &mut ipx, &mut irx);
                            q0 = qv;
                            self.q = q0;
                        }
                        let q = self.q;
                        if icon == 1 {
                            self.disbar(e, &mut ebar, &mut dame, q, za, awr, mtd)?;
                        }
                        if icon == 2 {
                            self.conbar(e, &mut ebar, &mut dame, mtd, &mut yld, q, endf)?;
                        }
                        if icon == 0 && self.idame > 0 {
                            self.capdam(e, &mut dame, q, za, awr, mtd);
                        }
                        if icon < 0 {
                            self.sixbar(e, &mut io, mth, j6, &mut irec, &mut jrec)?;
                            ebar = io.ebar;
                            yld = io.yld;
                            dame = io.dame;
                        }
                        if yld0 == 0.0 {
                            yld0 = yld;
                        }
                        if fission {
                            if self.lfc == 0 {
                                if self.nply == 0 {
                                    q0 = self.cpoly[0] - (yld - yld0) * FQ1 + FQ2 * e;
                                    h0 = if self.mt458 == 1 {
                                        self.hpoly[0]
                                    } else {
                                        q0 - ebar * yld
                                    };
                                } else {
                                    let nply = self.nply;
                                    q0 = self.cpoly[nply];
                                    h0 = self.hpoly[nply];
                                    for i in (0..nply).rev() {
                                        q0 = q0 * e + self.cpoly[i];
                                        h0 = h0 * e + self.hpoly[i];
                                    }
                                }
                                pnue = q0 - h0;
                            } else {
                                if self.etabmax < e
                                    && ((self.etabmax - e).abs() / self.etabmax) > TOL
                                {
                                    return Err(NjoyError::EndfParse("nheat: upper energy tabulated fission q components is too low.".into()));
                                }
                                let qfr = if self.ifc1 == 1 {
                                    terpa(&self.afr, e, &mut ipfr, &mut irfr).0
                                } else {
                                    self.c458[1]
                                };
                                let qnp = if self.ifc2 == 1 {
                                    terpa(&self.anp, e, &mut ipnp, &mut irnp).0
                                } else {
                                    self.c458[3] - (yld - yld0) * FQ1 + FQ2 * e
                                };
                                let qgp = if self.ifc4 == 1 {
                                    terpa(&self.agp, e, &mut ipgp, &mut irgp).0
                                } else {
                                    self.c458[7]
                                };
                                pnue = qnp;
                                h0 = qfr + qgp;
                                q0 = h0 + pnue;
                            }
                            q0 -= e;
                        }
                        let mut h;
                        let mut ebal6;
                        if icon >= 0 {
                            ebal6 = 0.0;
                            dame *= y;
                            h = if !fission {
                                (e + q0 - ebar * yld) * y
                            } else {
                                h0 * y
                            };
                        } else {
                            h = if !fission { ebar * yld * y } else { h0 * y };
                            dame *= y;
                            c[npkk] += h;
                            if self.izap == 0 {
                                c[npkk - 1] += h;
                            }
                            if self.izap == 0 && npkkk > 0 {
                                c[npkkk] += h;
                            }
                            if self.izap == 1 {
                                h = 0.0;
                            }
                            if self.izap == 0 && self.local == 0 {
                                h = 0.0;
                            }
                            if self.izap <= 1 {
                                dame = 0.0;
                            }
                            ebal6 = 0.0;
                            if last6 == 1 && mth != 5 && mth != 102 {
                                ebal6 = (e + q0) * y - c[npkk];
                            }
                            if fission {
                                ebal6 = 0.0;
                            }
                        }
                        if (46..=49).contains(&mtd) {
                            if qs >= 0.0 {
                                return Err(NjoyError::EndfParse(
                                    "nheat: binding energy for sequential n2n needed.".into(),
                                ));
                            }
                            h = (qs - ebar) * y;
                        }

                        // Total and partial heating.
                        c[2] += h + ebal6;
                        if fission {
                            ebar = pnue / yld;
                        }
                        if self.iprint == 1
                            && (e - self.elist[ilist]).abs() <= SMALL * e
                            && y > SMIN
                        {
                            let row = if qs >= QTEST || fission {
                                let mut v = vec![
                                    e,
                                    q0,
                                    sigfig(ebar, 9, 0),
                                    sigfig(yld, 9, 0),
                                    sigfig(y, 9, 0),
                                    sigfig(h, 9, 0),
                                ];
                                if self.idame != 0 {
                                    v.push(sigfig(dame, 9, 0));
                                }
                                v
                            } else {
                                let mut v = vec![
                                    e,
                                    sigfig(ebar, 9, 0),
                                    sigfig(yld, 9, 0),
                                    sigfig(y, 9, 0),
                                    sigfig(h, 9, 0),
                                ];
                                if self.idame != 0 {
                                    v.push(sigfig(dame, 9, 0));
                                }
                                v
                            };
                            self.listing.push_str(&line14(&row));
                            if ebal6.abs() >= 100.0 * y {
                                let pad = if qs < QTEST { 53 } else { 67 };
                                self.listing.push_str(&format!(
                                    "{:pad$}ebal{}\n",
                                    "",
                                    fortran_e(sigfig(ebal6, 9, 0), 4, 14)
                                ));
                            }
                        }
                        for &ix in &imt {
                            if self.mtp[ix] >= 444 {
                                c[ix] += dame;
                            }
                            if self.mtp[ix] < 442 {
                                c[ix] += h + ebal6;
                            }
                        }

                        // Kinematic limits.
                        if self.kchk != 0 {
                            let (hmin, hmax) = if icon < 0 {
                                (h, h)
                            } else if mtd == 2 {
                                (h, h)
                            } else if mtd == 102 {
                                let hmin = y * e * aw1fac;
                                let tt = q + awr * e * aw1fac;
                                (hmin, hmin + y * tt * tt * self.rtm / 2.0)
                            } else if self.level_like(mtd) {
                                let hmin = (e + q - ebar * yld) * y;
                                let hmax = if lr != 0 && lr != 31 {
                                    (e + q0 - ebar * yld) * y
                                } else {
                                    hmin
                                };
                                (hmin, hmax)
                            } else if mtd == 91 {
                                let hmin = y * (e + ebar) * awfac;
                                let hmax = if lr != 0 && lr != 31 {
                                    (e + q0 - ebar) * y
                                } else {
                                    hmin
                                };
                                (hmin, hmax)
                            } else if mtd >= 103 {
                                (y * e * awfac, h)
                            } else if matches!(mtd, 18 | 20 | 21 | 38) {
                                (y * (e + q0 - ebar * yld / 2.0 - EFIS), h)
                            } else if mtd == 16 && awr >= 10.0 {
                                let mut hmax = y * (e + ebar) / (awr - 1.0);
                                if hmax > h {
                                    hmax = h;
                                }
                                (0.0, hmax)
                            } else if mtd == 17 && awr >= 10.0 {
                                let mut hmax = y * (e + 2.0 * ebar) / (awr - 2.0);
                                if hmax > h {
                                    hmax = h;
                                }
                                (0.0, hmax)
                            } else {
                                (0.0, h)
                            };
                            // 285
                            let npkt = npk - 1;
                            if self.kchk == 1 {
                                c[2 + npkt] += hmin;
                                c[2 + 2 * npkt] += hmax;
                            }
                            for &ix in &imt {
                                let m = self.mtp[ix];
                                if m == 442 || m >= 444 {
                                    continue;
                                }
                                if m == 443 {
                                    c[ix] += hmax;
                                    continue;
                                }
                                if self.kchk == 1 {
                                    c[ix + npkt] += hmin;
                                    c[ix + 2 * npkt] += hmax;
                                }
                            }
                        }
                    }
                    // 290
                    self.rows[nn - 1] = c;
                    if e == self.elist[ilist] {
                        ilist += 1;
                    }
                }
            }

            // 291: File 6 subsections.
            if j6 < n6 {
                // The same section again, for the next subsection.
                continue;
            }
            if j6 != 0 {
                // 296
                let goto_293 = irec > 0 && self.mt6no[self.i6g] > 0;
                if !goto_293 {
                    if self.iprint == 1 && self.i6g > 0 && self.izap != 0 && io.iflag == 0 {
                        self.listing.push_str(&format!(
                            "\n   no explicit file 6 photon production for mt{mtd:3}\n"
                        ));
                    }
                    if self.mt6no[self.i6g] != 0 {
                        if mth != 102 {
                            self.listing
                                .push_str("\n   generating recoil with one-particle approx.\n");
                        } else {
                            self.listing
                                .push_str("\n   generating recoil from photon momentum.\n");
                        }
                        irec = self.mt6no[self.i6g];
                        continue;
                    }
                }
                // 293
                irec = 0;
                jrec = 0;
                j6 = 0;
                n6 = 0;
            }
            si += 1;
        }
        let _ = (ebar, dame, yld);
        Ok(())
    }

    /// The level and charged-particle-level MTs of the `kchk` branch at
    /// label 221 (`:1501-1513`).
    fn level_like(&self, mtd: i32) -> bool {
        let r = |lo: i32, hi: i32| mtd >= lo && mtd < hi;
        if r(51, 91) {
            return true;
        }
        if self.iverf <= 5 {
            r(700, 718) || r(720, 738) || r(740, 758) || r(760, 778) || r(780, 798)
        } else {
            r(600, 649) || r(650, 699) || r(700, 749) || r(750, 799) || r(800, 849)
        }
    }

    /// The `iprint = 1` heading `nheat` writes before a reaction.
    fn nheat_header(&mut self, icon: i32, mtd: i32, q0: f64, qendf: f64, qs: f64, fission: bool) {
        const QTEST: f64 = 99.0e6;
        let dam = if self.idame > 0 { "        damage" } else { "" };
        let s = if icon < 0 {
            if qs < QTEST {
                format!(
                    "\n file six heating for mt{mtd:3}, particle ={:6}     q = {}\n{:14}e          ebar         yield          xsec       heating{dam}\n",
                    self.izap,
                    fortran_e(q0, 4, 12),
                    ""
                )
            } else {
                format!(
                    "\n file six heating for mt{mtd:3}, particle ={:6}     q0 = variable\n{:14}e          qbar          ebar         yield          xsec       heating{dam}\n",
                    self.izap, ""
                )
            }
        } else if qs >= QTEST {
            format!(
                "\n neutron heating for mt{mtd:3}   q0= variable     q ={}\n{:14}e          qbar          ebar         yield          xsec       heating{dam}\n",
                fortran_e(qendf, 4, 12),
                ""
            )
        } else if fission {
            format!("\n neutron heating for mt{mtd:3}   q0= variable fission q\n{:14}e            q0          ebar         yield          xsec       heating{dam}\n", "")
        } else {
            format!(
                "\n neutron heating for mt{mtd:3}   q0 ={}     q ={}\n{:14}e          ebar         yield          xsec       heating{dam}\n",
                fortran_e(q0, 4, 12),
                fortran_e(qendf, 4, 12),
                ""
            )
        };
        self.listing.push_str(&s);
    }
}
