// Ported from NJOY2016 `src/heatr.f90` (`sixbar`, lines 2755-3072).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `sixbar`: walk one MF=6 subsection over ascending incident energies,
//! returning the emitted particle's yield, mean energy and damage energy, or,
//! for a recoil (`irec > 0`), the recoil's.
//!
//! Upstream reads `nend6` as a tape: the subsection's yield TAB1, then its
//! per-incident-energy records one at a time as the energy walk needs them.
//! For a recoil it backs up to the start of the section and skips to the
//! driving particle's subsection (`:2819-2840`); after a LAW=4 two-body recoil
//! it records where to resume (`jrec`). Here the section is held in memory and
//! [`super::state::SixbarState::pos`] is the tape position: the index of the
//! next subsection a `tab1io` would read. One consequence is defined rather
//! than inherited: upstream would read the next subsection's records as this
//! one's if the energy walk ever asked for more records than a subsection
//! holds; this port returns an error there instead.

use super::flat::{nint, terp1, terpa0};
use super::state::Heatr;
use crate::NjoyError;

/// What `sixbar` returns besides `ebar`/`yld`/`dame`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SixIo {
    pub ebar: f64,
    pub yld: f64,
    pub dame: f64,
    pub iflag: i32,
}

impl Heatr {
    /// Read the next subsection (`tab1io` at the tape position).
    fn six_read_sub(&mut self) -> Result<(), NjoyError> {
        let s = &mut self.six;
        let sub = s.sec.subs.get(s.pos).ok_or_else(|| {
            NjoyError::EndfParse("sixbar: MF=6 section has fewer subsections than NK".into())
        })?;
        s.c_yld = sub.yld.clone();
        s.cur = s.pos;
        s.nread = 0;
        s.pos += 1;
        Ok(())
    }

    /// Read the current subsection's next per-energy record into `c(iraw)`.
    fn six_read_rec(&mut self) -> Result<(), NjoyError> {
        let s = &mut self.six;
        let sub = &s.sec.subs[s.cur];
        let r = sub.recs.get(s.nread).cloned().ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "sixbar: MF=6 subsection {} (LAW={}) asked for more incident energies than it holds",
                s.cur + 1,
                sub.law
            ))
        })?;
        s.c_raw = r;
        s.nread += 1;
        Ok(())
    }

    /// `getsix` or `tabsq6` at energy `ee` on the current record, as `sixbar`
    /// chooses between them (`:2958-2963`, `:3027-3032`).
    fn six_point(&mut self, ee: f64, mth: i32, irec: usize) -> Result<(f64, f64), NjoyError> {
        let raw = self.six.c_raw.clone();
        if mth != 102 || (self.zap == 0.0 && irec == 0) {
            let (law, lep) = (self.six.law, self.six.lep);
            let mut lang = self.six.lang;
            let r = self.getsix(ee, &raw, law, &mut lang, lep, irec)?;
            self.six.lang = lang;
            Ok(r)
        } else {
            let ztt = (self.zat / 1000.0).trunc();
            let yld = self.six.c_yld.clone();
            let awrt = self.awrt;
            Ok(self.tabsq6(&raw, ztt, awrt, ee, &yld))
        }
    }

    /// `sixbar(e, ebar, yld, dame, nend6, ..., n6, j6, irec, jrec, iflag)`.
    /// `io` holds `ebar`, `yld`, `dame` and `iflag` in and out, as upstream's
    /// arguments do; `irec` and `jrec` are in/out.
    pub(super) fn sixbar(
        &mut self,
        e: f64,
        io: &mut SixIo,
        mth: i32,
        j6: usize,
        irec: &mut usize,
        jrec: &mut usize,
    ) -> Result<(), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const EMAX: f64 = 1.0e10;
        let fission = (18..=21).contains(&mth) || mth == 38;
        if e <= 0.0 {
            let mut backup = *irec != 0;
            if backup && *jrec > 0 {
                *irec = *jrec;
            }
            loop {
                if backup {
                    // 100: back up to the driving particle, or restore the
                    // position after a recoil.
                    self.six.pos = irec.saturating_sub(1);
                    if *jrec == *irec {
                        *irec = 0;
                        *jrec = 0;
                    }
                }
                // 110
                self.six_read_sub()?;
                let yl = &self.six.c_yld;
                self.zap = yl[0];
                self.awp = yl[1];
                self.six.law = yl[3] as i32;
                if self.zap != 0.0 && self.awp == 0.0 {
                    return Err(NjoyError::EndfParse(format!(
                        "sixbar: awp is zero for mt{mth:3} particle {:5}",
                        nint(self.zap)
                    )));
                }
                io.iflag = 0;
                self.six.disc102 = 0.0;
                if self.zap == 0.0 {
                    io.iflag = 1;
                    self.six.disc102 = self.awp;
                    self.awp = 0.0;
                }
                io.ebar = 0.0;
                io.yld = 0.0;
                if self.six.law <= 0 {
                    self.mess(
                        "sixbar",
                        &format!(
                            "no distribution for mt{mth:3} particle {:5}",
                            nint(self.zap)
                        ),
                        " ",
                    );
                    io.ebar = -2.0;
                    return Ok(());
                }
                if self.six.law == 4 {
                    // 150: a two-body recoil; process the driving particle.
                    *irec = j6 - 1;
                    *jrec = j6 + 1;
                    backup = true;
                    continue;
                }
                break;
            }
            // 210
            let mut zp = (self.zap / 1000.0).trunc();
            let zt = (self.zat / 1000.0).trunc();
            if *irec > 0 {
                zp = zt - zp;
            }
            let mut ap = self.awp;
            if *irec > 0 {
                ap = self.awr + 1.0 - self.awp;
            }
            let at = self.awr;
            if self.zap == 0.0 {
                ap = self.awr + 1.0;
                zp = zt;
            }
            io.dame = self.df(e, zp, ap, zt, at);
            let s = &mut self.six;
            s.zp = zp;
            s.ap = ap;
            s.zt = zt;
            s.at = at;
            if s.disc102 > 0.0 {
                // 295
                io.yld = 1.0;
                return Ok(());
            }
            let law = s.law;
            if law != 3 && law != 6 {
                let t2 = &s.sec.subs[s.cur].tab2;
                s.lang = nint(t2[2]) as i32;
                s.lep = nint(t2[3]) as i32;
                s.ne = nint(t2[5]) as usize;
                s.intl = 2;
            }
            s.nne = 0;
            match law {
                3 => {
                    s.ehi = EMAX;
                    s.lang = 0;
                    s.c_raw = vec![0.0; 7];
                    return Ok(());
                }
                6 => {
                    self.six_read_rec()?;
                    let s = &mut self.six;
                    s.lang = 0;
                    s.lep = 2;
                    s.ehi = EMAX;
                    s.dhi = 0.0;
                    s.fhi = 0.0;
                    return Ok(());
                }
                _ => {
                    self.six_read_rec()?;
                    let s = &mut self.six;
                    s.nne = 1;
                    s.elo = s.c_raw[1];
                    s.ehi = 0.0;
                    s.dhi = 0.0;
                    s.fhi = 0.0;
                }
            }
            // 290
            let elo = self.six.elo;
            let (flo, dlo) = self.six_point(elo, mth, *irec)?;
            self.six.flo = flo;
            self.six.dlo = dlo;
            if fission {
                let mt1 = if self.nply > 0 { 456 } else { 452 };
                self.hgtyld(e, mt1)?;
            }
            return Ok(());
        }

        // Normal entry.
        if self.six.disc102 > 0.0 {
            // 430
            let (zp, ap, zt, at, d) = (
                self.six.zp,
                self.six.ap,
                self.six.zt,
                self.six.at,
                self.six.disc102,
            );
            let (eb, dm) = self.hgam102(e, d, *irec, zp, ap, zt, at);
            io.ebar = eb;
            io.dame = dm;
            io.yld = 1.0;
            return Ok(());
        }
        let mut pe = terpa0(&self.six.c_yld, e).0;
        if *irec > 0 {
            pe = 1.0;
        }
        loop {
            // 305
            let s = &self.six;
            if e < s.ehi * (1.0 - SMALL) {
                break;
            }
            if s.nne != 1 {
                if s.nne == s.ne && e <= s.ehi * (1.0 + SMALL) {
                    break;
                }
                if s.nne == s.ne {
                    // 450
                    io.ebar = 0.0;
                    io.dame = 0.0;
                    return Ok(());
                }
                let s = &mut self.six;
                s.elo = s.ehi;
                s.flo = s.fhi;
                s.dlo = s.dhi;
            }
            // 310
            match self.six.law {
                1 | 2 | 5 | 7 => {
                    self.six_read_rec()?;
                    let s = &mut self.six;
                    s.nne += 1;
                    s.ehi = s.c_raw[1];
                }
                _ => {
                    // LAW=6: upstream re-reads a CONT; `ehi = emax` means
                    // this is never reached.
                }
            }
            let ehi = self.six.ehi;
            let (fhi, dhi) = self.six_point(ehi, mth, *irec)?;
            self.six.fhi = fhi;
            self.six.dhi = dhi;
        }
        // 400
        let law = self.six.law;
        if law == 3 || law == 6 {
            if self.six.c_raw.len() < 2 {
                self.six.c_raw.resize(2, 0.0);
            }
            self.six.c_raw[1] = e;
            let raw = self.six.c_raw.clone();
            let lep = self.six.lep;
            let mut lang = self.six.lang;
            let (f, d) = self.getsix(e, &raw, law, &mut lang, lep, *irec)?;
            self.six.lang = lang;
            io.ebar = f;
            io.yld = pe;
            io.dame = pe * d;
            return Ok(());
        }
        // 420
        if self.six.nne == 1 {
            io.ebar = 0.0;
            io.dame = 0.0;
            return Ok(());
        }
        let s = &self.six;
        let sv = terp1(s.elo, s.flo, s.ehi, s.fhi, e, s.intl);
        io.ebar = sv;
        io.yld = pe;
        if fission {
            let (_, _, y) = self.hgtyld(e, 452)?;
            io.yld = y;
        }
        let s = &self.six;
        let d = terp1(s.elo, s.dlo, s.ehi, s.dhi, e, s.intl);
        io.dame = pe * d;
        Ok(())
    }
}
