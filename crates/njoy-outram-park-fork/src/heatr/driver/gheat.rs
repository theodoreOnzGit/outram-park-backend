// Ported from NJOY2016 `src/heatr.f90` (`gheat` 5071-5449, `gambar`
// 5450-5609, `tabsqr` 5610-5670, `disgam` 5671-5688).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `gheat`: subtract the energy the MF=12 and MF=13 photons carry away
//! (`local = 0` only), section by section and photon by photon.
//!
//! An MF=12 reaction's photons take `y·σ·Ē_γ`, with `Ē_γ` the line energy
//! (plus `E·A/(A+1)` for a primary photon, `LP = 2`) or, for a continuum, the
//! MF=15 mean (`gambar`). MF=13 photons take `σ_γ·Ē_γ`. Capture is an energy
//! balance instead: the photons' recoil (`disgam` for a line, `gambar`'s
//! second moment for a continuum) is deposited, and after the last photon
//! `E + Q − E/(A+1)` is taken away, whatever the photon lines add up to.
//! MF=12/MT=3 is done twice, once against the total cross section and once,
//! with the opposite sign, against elastic.
//!
//! `crate::photon` computes the same MT=442 for the ACE route on its own grid;
//! this is HEATR's routine on HEATR's grid, with the partial kermas, the
//! capture damage and MT=443's capture term.

use super::conbar::tabbar;
use super::flat::{nint, tab1_flat, tab2_flat, terp1, terpa, QP4, QW4};
use super::nheat::line14;
use super::state::{indx, Heatr};
use crate::endf::gety1::Gety1;
use crate::endf::records::SectionCursor;
use crate::endf::tape::{Section, Tape};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

impl Heatr {
    /// `gheat(iold, inew, nscr)`.
    pub(super) fn gheat(&mut self, endf: &Tape, pendf: &[Section]) -> Result<(), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const BIG: f64 = 1.0e10;
        const SMIN: f64 = 1.0e-9;
        const QTEST: f64 = 99.0e6;
        let matd = self.matd;
        let awr = self.awr;
        let za = self.za;
        let afact = awr / (awr + 1.0);
        let aw1fac = 1.0 / (awr + 1.0);
        let z = nint(za / 1000.0) as f64;
        self.df(0.0, z, awr, z, awr);
        let mut hk = 0.0;

        // The photon sections of the working copy, in order.
        let conv: Vec<Section> = self
            .conv
            .sections()
            .iter()
            .filter(|s| s.key.mat == matd && (s.key.mf == 12 || s.key.mf == 13))
            .cloned()
            .collect();
        let mgam_local = if conv.first().map(|s| s.key.mf) == Some(12) {
            1
        } else {
            2
        };
        let mut npkk = self.npk;
        if self.kchk == 1 {
            npkk = 3 * self.npk - 2;
        }
        npkk += 3;
        let npk = self.npk;
        let npkt = npk - 1;
        if mgam_local == 2 {
            self.mess("gheat", "no file 12 for this material.", " ");
        }

        for sec in &conv {
            let (mfh, mth) = (sec.key.mf, sec.key.mt);
            let mfd: i32 = if mfh == 12 { 3 } else { 13 };
            if mfh == 12 && mth == 460 {
                continue;
            }
            if self.mt6yp[1..].contains(&mth) {
                self.mess(
                    "gheat ",
                    &format!("skipping mf{mfh:2}/mt = {mth:3}"),
                    "photons were already processed in mf6",
                );
                continue;
            }
            let lo = nint(sec.rows[0][2]);
            if lo == 2 {
                return Err(NjoyError::EndfParse("gheat: lo=2 not coded.".into()));
            }
            let mfx = mfh;
            let mtx = mth;
            let mut mtd = if mth == 3 { 1 } else { mth };
            self.gheat_header(mfx, mtd);
            loop {
                // 110
                let mut cur = SectionCursor::new(&sec.rows);
                let head = cur.read_cont()?;
                let nk = head.n1.max(0) as usize;
                let mut tabs = Vec::new();
                while cur.remaining() > 0 {
                    tabs.push(cur.read_tab1()?);
                }
                let (scan, partials) = if nk > 1 {
                    (&tabs[0], &tabs[1..])
                } else {
                    (&tabs[0], &tabs[..])
                };
                // Leading and trailing zeros: where this partial is defined.
                let mut g = Gety1::new(scan);
                let mut e = g.get(0.0).xnext;
                let mut el = sigfig(e, 7, 0);
                let mut elow = BIG;
                let mut ehigh = 0.0;
                let test = BIG - BIG / 100.0;
                while e < test {
                    let v = g.get(e);
                    if v.y > 0.0 && e < elow {
                        elow = el;
                    }
                    if v.y > 0.0 {
                        ehigh = 0.0;
                    }
                    if ehigh == 0.0 && v.y == 0.0 {
                        ehigh = sigfig(e, 7, 0);
                    }
                    el = sigfig(e, 7, 0);
                    e = v.xnext;
                }
                if ehigh == 0.0 {
                    ehigh = sigfig(e, 7, 0);
                }
                let mut cerr = 0.0;
                let mut imt: Vec<usize> = Vec::new();
                for (ik0, tk) in partials.iter().enumerate().take(nk.max(1)) {
                    let ik = ik0 + 1;
                    // 130
                    let mut gy = Gety1::new(tk);
                    let mut thresh = gy.get(0.0).xnext;
                    let mut ilist = 1usize;
                    let egkr = tk.head.c1;
                    let (mut egam, mut edam) = (0.0f64, 0.0f64);
                    if egkr == 0.0 {
                        self.gambar(0.0, mtx, endf)?;
                    }
                    let lp = tk.head.l1;
                    if self.iprint == 1 {
                        if egkr == 0.0 {
                            self.listing
                                .push_str(&format!("{ik:6}  continuum gammas\n"));
                        } else {
                            self.listing.push_str(&format!(
                                "{ik:6}{} ev gamma\n",
                                crate::acer::fortran_fmt::fortran_e(egkr, 4, 13)
                            ));
                        }
                    }
                    let mut lqx = 0usize;
                    let mut gx: Option<Gety1> = None;
                    let (mut ipx, mut irx) = (2usize, 1usize);
                    if mfd != 13 {
                        let s3 = pendf
                            .iter()
                            .find(|s| s.key.mf == 3 && s.key.mt == mtd)
                            .ok_or(NjoyError::SectionNotFound {
                                mat: matd,
                                mf: 3,
                                mt: mtd,
                            })?;
                        let mut c3 = SectionCursor::new(&s3.rows);
                        c3.read_cont()?;
                        let t3 = c3.read_tab1()?;
                        let mut g3 = Gety1::new(&t3);
                        let enxt = g3.get(0.0).xnext;
                        if mth == 102 {
                            self.q = t3.head.c2;
                            for i in 1..=self.nqa {
                                if self.mta[i] == mth {
                                    self.q = self.qa[i];
                                    if self.q >= QTEST {
                                        lqx = self.lqs[i];
                                    }
                                }
                            }
                            let mut d = 0.0;
                            let q = self.q;
                            self.capdam(0.0, &mut d, q, za, awr, mth);
                            ipx = 2;
                            irx = 1;
                        }
                        if enxt > thresh {
                            thresh = enxt;
                        }
                        gx = Some(g3);
                    }
                    let _ = thresh;
                    // 150
                    if ik == 1 {
                        imt.clear();
                        if npk >= 3 {
                            let mut mty = mtx;
                            if mty == 3 {
                                mty = 1;
                            }
                            if mty == 4 {
                                mty = 51;
                            }
                            if mty == 18 && self.mt19 == 1 {
                                mty = 19;
                            }
                            imt = indx(npk, &self.mtp, self.mt19, mty, self.iverf);
                        }
                    }
                    // 160: all energies of the grid.
                    for i in 1..=self.ne {
                        let mut c = std::mem::take(&mut self.rows[i - 1]);
                        let e = sigfig(c[1], 9, 0);
                        let inside = !(e < elow * (1.0 - SMALL) || e > ehigh * (1.0 + SMALL));
                        if inside {
                            let y = gy.get(e).y;
                            let mut x = 0.0;
                            let mut zero_x = false;
                            if mfd == 3 {
                                x = gx.as_mut().map_or(0.0, |g3| g3.get(e).y);
                                zero_x = x == 0.0;
                            }
                            if !zero_x {
                                if lqx != 0 {
                                    self.q = terpa(&self.qbar[lqx..], e, &mut ipx, &mut irx).0;
                                }
                                let q = self.q;
                                let mut egk = egkr;
                                if lp == 2 {
                                    egk = egkr + e * afact;
                                }
                                let mut ebar_g = egk;
                                let mut dame = 0.0;
                                if ik == 1 {
                                    c[npkk] = 0.0;
                                }
                                if egkr == 0.0 {
                                    let (eb, esqb, esqd) = self.gambar(e, mtx, endf)?;
                                    ebar_g = eb;
                                    if mtx == 102 {
                                        egam = esqb;
                                        edam = esqd;
                                    }
                                }
                                let h;
                                let mut hx;
                                if mfd == 13 {
                                    // 170
                                    h = -y * ebar_g;
                                    c[npkk - 1] -= h;
                                    c[npkk] += h;
                                    hx = h;
                                    c[2] += h;
                                    for &ix in &imt {
                                        if self.mtp[ix] < 442 {
                                            c[ix] += h;
                                        }
                                        if self.mtp[ix] == 442 {
                                            c[ix] -= h;
                                        }
                                    }
                                    if self.iprint == 1
                                        && (e - self.elist[ilist]).abs() <= SMALL * e
                                        && self.photon_print_ok(ik, nk, y, c[npkk])
                                    {
                                        self.listing.push_str(&line14(&[
                                            e,
                                            sigfig(ebar_g, 9, 0),
                                            sigfig(y, 9, 0),
                                            -sigfig(hx, 9, 0),
                                            sigfig(h, 9, 0),
                                        ]));
                                        self.subtot_line(ik, nk, c[npkk]);
                                    }
                                } else {
                                    let mut hh;
                                    if mth == 102 {
                                        if egkr != 0.0 {
                                            let (eg, ed) = self.disgam(egkr, z, awr);
                                            egam = eg;
                                            edam = ed;
                                        }
                                        hh = egam * x * y;
                                        hk = hh;
                                        c[npkk - 1] += x * y * ebar_g;
                                        c[npkk] += y * ebar_g;
                                        dame = edam * x * y;
                                        if ik == nk.max(1) {
                                            if self.idame > 0 {
                                                let mut damn = 0.0;
                                                self.capdam(e, &mut damn, q, za, awr, mth);
                                                dame -= damn * x;
                                            }
                                            hk -=
                                                x * self.rtm * (q + e * awr * aw1fac).powi(2) / 2.0;
                                            let cfix = (e + q - e * aw1fac) * x;
                                            hh -= cfix;
                                            c[npkk - 2] = cfix - x * c[npkk];
                                            let eava = afact * e + q;
                                            cerr = 100.0 * (c[npkk] - eava) / eava;
                                        }
                                        if self.iprint == 1
                                            && (e - self.elist[ilist]).abs() <= SMALL * e
                                            && y > SMIN
                                        {
                                            let v = if self.idame == 0 {
                                                vec![
                                                    e,
                                                    sigfig(ebar_g, 9, 0),
                                                    sigfig(x, 9, 0),
                                                    sigfig(y, 9, 0),
                                                    sigfig(hh, 9, 0),
                                                ]
                                            } else {
                                                vec![
                                                    e,
                                                    sigfig(ebar_g, 9, 0),
                                                    sigfig(egam, 9, 0),
                                                    sigfig(edam, 9, 0),
                                                    sigfig(x, 9, 0),
                                                    sigfig(y, 9, 0),
                                                    sigfig(hh, 9, 0),
                                                    sigfig(dame, 9, 0),
                                                ]
                                            };
                                            self.listing.push_str(&line14(&v));
                                            if ik == nk.max(1) {
                                                self.listing.push_str(&format!(
                                                    " {}{:10.1} pc\n",
                                                    crate::acer::fortran_fmt::fortran_e(e, 4, 14),
                                                    sigfig(cerr, 4, 0)
                                                ));
                                            }
                                        }
                                    } else {
                                        // 164: other MF=12 reactions.
                                        hh = 0.0;
                                        if mtd != 2 && mtd != 460 {
                                            hh = -y * x * ebar_g;
                                        }
                                        if mtd == 2 {
                                            hh = y * x * ebar_g;
                                        }
                                        c[npkk - 1] -= hh;
                                        c[npkk] += hh;
                                    }
                                    // 166
                                    h = hh;
                                    hx = h;
                                    c[2] += h;
                                    for &ix in &imt {
                                        let m = self.mtp[ix];
                                        if m >= 444 {
                                            c[ix] += dame;
                                        }
                                        if m < 442 {
                                            c[ix] += h;
                                        }
                                        if m == 442 {
                                            c[ix] -= h;
                                        }
                                        if m == 443 && mth == 102 {
                                            c[ix] += hk;
                                        }
                                    }
                                    if self.iprint != 0
                                        && e == self.elist[ilist]
                                        && mth != 102
                                        && self.photon_print_ok(ik, nk, y, c[npkk])
                                    {
                                        let _ = &mut hx;
                                        self.listing.push_str(&line14(&[
                                            e,
                                            sigfig(ebar_g, 9, 0),
                                            sigfig(x, 9, 0),
                                            sigfig(y, 9, 0),
                                            -sigfig(hx, 9, 0),
                                            sigfig(h, 9, 0),
                                        ]));
                                        self.subtot_line(ik, nk, c[npkk]);
                                    }
                                }
                            }
                        }
                        self.rows[i - 1] = c;
                        if (e - self.elist[ilist]).abs() < SMALL * e {
                            ilist += 1;
                        }
                    }
                }
                if mfd == 3 && mtx == 3 && mtd == 1 {
                    // 200: MF=12/MT=3 again, against elastic.
                    mtd = 2;
                    continue;
                }
                break;
            }
        }

        // 250: the photon energy production check.
        if self.kchk == 1 && self.mt303 != 0 {
            let mut ilist = 1usize;
            self.listing.push_str(&format!("\n photon energy production check\n{:14}e      ev-barns           min           max\n", ""));
            let mt303 = self.mt303;
            for i in 1..=self.ne {
                let c = &self.rows[i - 1];
                let e = sigfig(c[1], 9, 0);
                if (e - self.elist[ilist]).abs() <= SMALL * e {
                    ilist += 1;
                    let elo = c[npkk - 1] + c[mt303] - c[mt303 + 2 * npkt] + c[npkk - 2];
                    let ehi = c[npkk - 1] + c[mt303] - c[mt303 + npkt] + c[npkk - 2];
                    let v = [e, c[npkk - 1], elo, ehi];
                    let mut line = line14(&v);
                    if c[npkk - 1] > ehi + ehi / 10.0 {
                        line = line.trim_end().to_string() + "   ++++\n";
                    } else if c[npkk - 1] < elo - elo / 10.0 {
                        line = line.trim_end().to_string() + "   ----\n";
                    }
                    self.listing.push_str(&line);
                }
            }
        }
        Ok(())
    }

    fn photon_print_ok(&self, ik: usize, nk: usize, y: f64, cnpkk: f64) -> bool {
        if ik < nk && y < SMIN_P {
            return false;
        }
        if nk == 1 && y < SMIN_P {
            return false;
        }
        !(nk > 1 && ik == nk && cnpkk > -1.0 / 10000.0)
    }

    fn subtot_line(&mut self, ik: usize, nk: usize, cnpkk: f64) {
        if ik == nk && nk > 1 {
            let subtot = sigfig(-cnpkk, 9, 0);
            self.listing.push_str(&format!(
                "{:48}subtot   {}\n",
                "",
                crate::acer::fortran_fmt::fortran_e(subtot, 4, 14)
            ));
        }
    }

    fn gheat_header(&mut self, mfx: i32, mtd: i32) {
        if self.iprint != 1 {
            return;
        }
        let s = if mfx == 12 && self.idame > 0 && mtd == 102 {
            format!("\n photon energy (from yields) mf{mfx:2}, mt{mtd:3}\n{:14}e      ebar/err          egam          edam          xsec         yield       heating        damage\n", "")
        } else if mfx == 12 {
            format!("\n photon energy (from yields) mf{mfx:2}, mt{mtd:3}\n{:14}e          ebar          xsec         yield        energy       heating\n", "")
        } else {
            format!("\n photon energy (from xsecs) mf{mfx:2}, mt{mtd:3}\n{:14}e          ebar          xsec        energy       heating\n", "")
        };
        self.listing.push_str(&s);
    }

    /// `gambar(e, ebar, esqb, esqd, nendf, matd, mtd, ...)`: the mean photon
    /// energy of an MF=15 continuum and, for capture, the mean recoil and its
    /// damage. `e = 0` initialises. Returns `(ebar, esqb, esqd)`.
    pub(super) fn gambar(
        &mut self,
        e: f64,
        mtd: i32,
        endf: &Tape,
    ) -> Result<(f64, f64, f64), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const UP: f64 = 1.000001;
        if e <= 0.0 {
            let matd = self.matd;
            let sec = endf
                .section(matd, 15, mtd)
                .ok_or(NjoyError::SectionNotFound {
                    mat: matd,
                    mf: 15,
                    mt: mtd,
                })?;
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            let g = &mut self.gam;
            g.z = nint(head.c1 / 1000.0) as f64;
            g.awr = head.c2;
            cur.read_tab1()?;
            g.lf = 1;
            let t2 = cur.read_tab2()?;
            g.ne = t2.head.n2.max(0) as usize;
            g.tab2 = tab2_flat(&t2);
            g.nnt = 7;
            g.nbt = nint(g.tab2[6]) as usize;
            g.inn = 2;
            g.recs.clear();
            for _ in 0..g.ne {
                g.recs.push(tab1_flat(&cur.read_tab1()?));
            }
            g.lo_rec = g.recs.first().cloned().unwrap_or_default();
            g.next = 1;
            g.ehi = g.lo_rec.get(1).copied().unwrap_or(0.0);
            g.elo = g.ehi;
            g.nne = 1;
            g.ilo = 0;
            g.ihi = 0;
            g.fhi = 0.0;
            g.ghi = 0.0;
            g.hhi = 0.0;
            return Ok((0.0, 0.0, 0.0));
        }
        let mut need_read = false;
        if e >= self.gam.ehi * (1.0 - SMALL) {
            if self.gam.nne == 1 {
                need_read = true;
            } else if self.gam.nne == self.gam.ne {
                // 160
                let g = &self.gam;
                if g.inn != 1 && e >= UP * g.ehi {
                    return Ok((0.0, 0.0, 0.0));
                }
                return Ok((g.fhi, g.ghi, g.hhi));
            } else {
                self.gambar_slide();
                need_read = true;
            }
        }
        if need_read {
            loop {
                // 130
                if self.gam.nne == self.gam.ne {
                    return Err(NjoyError::EndfParse(
                        "gambar: requested energy gt highest given.".into(),
                    ));
                }
                let g = &mut self.gam;
                g.hi_rec = g.recs[g.next].clone();
                g.next += 1;
                g.nne += 1;
                g.ihi = 0;
                g.ehi = g.hi_rec[1];
                if g.ehi < e * (1.0 - SMALL) && g.nne < g.ne {
                    self.gambar_slide();
                    continue;
                }
                break;
            }
            let lf = self.gam.lf;
            let (z, awr) = (self.gam.z, self.gam.awr);
            if self.gam.ilo <= 0 {
                let lo = self.gam.lo_rec.clone();
                self.gam.flo = tabbar(&lo, lf);
                if mtd == 102 {
                    let (gg, hh) = self.tabsqr(&lo, z, awr);
                    self.gam.glo = gg;
                    self.gam.hlo = hh;
                }
            }
            let hi = self.gam.hi_rec.clone();
            self.gam.fhi = tabbar(&hi, lf);
            if mtd == 102 {
                let (gg, hh) = self.tabsqr(&hi, z, awr);
                self.gam.ghi = gg;
                self.gam.hhi = hh;
            }
            self.gam.ihi = 1;
        }
        // 140
        let g = &mut self.gam;
        if g.nne == 1 {
            return Ok((0.0, 0.0, 0.0));
        }
        if g.nne > g.nbt {
            g.nnt += 2;
            g.nbt = g
                .tab2
                .get(g.nnt - 1)
                .map_or(usize::MAX, |v| nint(*v) as usize);
            g.inn = 2;
        }
        if g.elo == g.ehi {
            g.elo = sigfig(g.elo, 7, -1);
            g.ehi = sigfig(g.ehi, 7, 1);
        }
        let sen = terp1(g.elo, g.flo, g.ehi, g.fhi, e, g.inn);
        let (mut esqb, mut esqd) = (0.0, 0.0);
        if mtd == 102 {
            esqb = terp1(g.elo, g.glo, g.ehi, g.ghi, e, g.inn);
            esqd = terp1(g.elo, g.hlo, g.ehi, g.hhi, e, g.inn);
        }
        Ok((sen, esqb, esqd))
    }

    /// `gambar`'s label 120: the high-energy record becomes the low one.
    fn gambar_slide(&mut self) {
        let g = &mut self.gam;
        g.lo_rec = g.hi_rec.clone();
        g.elo = g.ehi;
        g.flo = g.fhi;
        g.glo = g.ghi;
        g.hlo = g.hhi;
        g.ilo = g.ihi;
    }

    /// `tabsqr(g, h, a, law, z, awr)`: the mean of `E'²/(2 m c² (A+1))` over
    /// an MF=15 spectrum, and its damage.
    pub(super) fn tabsqr(&mut self, a: &[f64], z: f64, awr: f64) -> (f64, f64) {
        let at = |k: usize| a[k - 1];
        let ein = 2.0 * self.tm;
        let rein = 1.0 / ein;
        let (mut g, mut h, mut s) = (0.0, 0.0, 0.0);
        let nr = nint(at(5)) as usize;
        let np = nint(at(6)) as usize;
        let ibase = 6 + 2 * nr;
        let mut ir = 1usize;
        let mut nbt = nint(at(7)) as usize;
        let mut inn = nint(at(8));
        let mut xh = at(ibase + 1);
        let mut yh = at(ibase + 2);
        for i in 2..=np {
            let xl = xh;
            xh = at(ibase + 2 * i - 1);
            let yl = yh;
            yh = at(ibase + 2 * i);
            if i > nbt {
                ir += 1;
                nbt = nint(at(6 + 2 * ir - 1)) as usize;
                inn = nint(at(6 + 2 * ir));
            }
            if xl != xh {
                let dx = xh - xl;
                for j in 0..4 {
                    let x = xl + (1.0 + QP4[j]) * dx / 2.0;
                    let y = terp1(xl, yl, xh, yh, x, inn);
                    let xr = x * x * rein;
                    g += QW4[j] * xr * dx * y;
                    h += QW4[j] * self.df(xr, z, awr + 1.0, z, awr) * dx * y;
                    s += QW4[j] * dx * y;
                }
            }
        }
        (g / s, h / s)
    }

    /// `disgam(e, egam, edam, z, awr)`: the recoil and damage energy of a
    /// discrete capture photon of energy `e`.
    pub(super) fn disgam(&mut self, e: f64, z: f64, awr: f64) -> (f64, f64) {
        let egam = e * e * self.rtm / 2.0;
        (egam, self.df(egam, z, awr + 1.0, z, awr))
    }
}

const SMIN_P: f64 = 1.0e-9;
