// Ported from NJOY2016 `src/heatr.f90` (`hout`, lines 5689-6322).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `hout`: write the kerma factors onto the output PENDF.
//!
//! Each of MT=301 and the partial MTs becomes an MF=3 TAB1 on `hinit`'s grid,
//! every value rounded to 7 significant figures (6 above `1e10`), energies
//! included, with the leading zeros dropped; an MT that is zero everywhere is
//! written as two zero points at the first and last energy. The MF=1/MT=451
//! directory gains (or updates) their entries, the new sections are merged
//! into MF=3 by MT, and the rest of the material is copied. With a plot unit,
//! the energy-balance check for `viewr` is written too.

use super::flat::{nint, rows_of_tab1};
use super::nheat::line14;
use super::state::Heatr;
use crate::endf::tape::Section;
use crate::endf::EndfKey;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

impl Heatr {
    /// `hout(iold)`: this temperature's output sections and, if asked, the
    /// plot text.
    pub(super) fn hout(
        &mut self,
        pendf: &[Section],
        plot: bool,
    ) -> Result<(Vec<Section>, Option<String>), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const BIG: f64 = 1.0e10;
        let matd = self.matd;
        let npk = self.npk;
        let npkk = self.npkk();
        let npkt = npk - 1;
        let mut npkd = npk;
        let mut npktd = npkt;
        for i in 2..=npk {
            if self.mtp[i] >= 442 {
                npkd -= 1;
                npktd -= 1;
            }
        }
        if self.iprint == 1 {
            let mut s = format!("\n final kerma factors\n{:14}e", "");
            for i in 2..=npk {
                s.push_str(&format!("{:11}{:3}", "", self.mtp[i]));
            }
            s.push('\n');
            self.listing.push_str(&s);
        }

        // The TAB1s.
        let mut ilist = 1usize;
        let mut new3: Vec<Section> = Vec::new();
        let mut ncds = vec![0usize; npk + 1];
        for inpk in 2..=npk {
            let mth = self.mtp[inpk];
            let mut pts: Vec<(f64, f64)> = Vec::new();
            for nn in 1..=self.ne {
                let mut c = self.rows[nn - 1].clone();
                for v in c.iter_mut().take(npkk + 1).skip(1) {
                    *v = if *v > BIG {
                        sigfig(*v, 6, 0)
                    } else {
                        sigfig(*v, 7, 0)
                    };
                }
                let e = sigfig(c[1], 9, 0);
                if self.iprint == 1 && (e - self.elist[ilist]).abs() <= SMALL * e {
                    self.final_kerma_lines(&c, npkd, npktd, npkt);
                }
                if e >= self.elist[ilist] {
                    ilist += 1;
                }
                if pts.is_empty() && c[inpk] == 0.0 {
                    continue;
                }
                pts.push((c[1], c[inpk]));
            }
            if pts.is_empty() {
                pts.push((self.efirst, 0.0));
                pts.push((self.elast, 0.0));
            }
            let n = pts.len();
            let mut flat = vec![0.0, 0.0, 0.0, 0.0, 1.0, n as f64, n as f64, 2.0];
            for (x, y) in &pts {
                flat.push(*x);
                flat.push(*y);
            }
            let mut rows = vec![[self.za, self.awr, 0.0, 0.0, 0.0, 0.0]];
            rows.extend(rows_of_tab1(&flat));
            new3.push(Section {
                key: EndfKey {
                    mat: matd,
                    mf: 3,
                    mt: mth,
                },
                rows,
            });
            ncds[inpk] = 3 + (n + 2) / 3;
        }

        // The directory.
        let s451 = pendf
            .iter()
            .find(|s| s.key.mf == 1 && s.key.mt == 451)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 1,
                mt: 451,
            })?;
        let mut r451 = s451.rows.clone();
        let hd = match self.iverf {
            6 => 3usize,
            5 => 2,
            _ => 1,
        };
        let hd = hd.min(r451.len().saturating_sub(1));
        let nwd = nint(r451[hd][4]) as usize;
        let nx = if self.iverf >= 5 {
            nint(r451[hd][5]) as usize
        } else {
            nint(r451[0][5]) as usize
        };
        let dict_at = (hd + 1 + nwd).min(r451.len());
        let old: Vec<[f64; 6]> = r451.iter().skip(dict_at).take(nx).copied().collect();
        let mut b: Vec<[f64; 6]> = Vec::new();
        let mut inpk = 2usize;
        let mut mtn = self.mtp[inpk];
        for d in &old {
            let mut iowr = false;
            let mfi = nint(d[2]) as i32;
            let mti = nint(d[3]) as i32;
            while (mfi == 3 && mtn <= mti) || (mfi > 3 && inpk <= npk) {
                if mfi == 3 && mtn == mti {
                    iowr = true;
                }
                b.push([
                    0.0,
                    0.0,
                    3.0,
                    f64::from(self.mtp[inpk]),
                    ncds[inpk] as f64,
                    0.0,
                ]);
                inpk += 1;
                mtn = if inpk <= npk { self.mtp[inpk] } else { 10000 };
            }
            if mfi != 3 || !iowr {
                b.push([0.0, 0.0, f64::from(mfi), f64::from(mti), d[4], d[5]]);
            }
        }
        for i in inpk..=npk {
            b.push([0.0, 0.0, 3.0, f64::from(self.mtp[i]), ncds[i] as f64, 0.0]);
        }
        let nxn = b.len() as f64;
        if self.iverf <= 4 {
            r451[0][5] = nxn;
        } else {
            r451[hd][5] = nxn;
        }
        r451.truncate(dict_at);
        r451.extend(b);

        // The material: MF=1 and MF=2 copied, MF=3 merged, the rest copied.
        let mut out: Vec<Section> = Vec::with_capacity(pendf.len() + new3.len());
        let mut new_iter = new3.into_iter().peekable();
        let mut done3 = false;
        for s in pendf {
            if s.key.mf == 1 && s.key.mt == 451 {
                out.push(Section {
                    key: s.key,
                    rows: r451.clone(),
                });
                continue;
            }
            if s.key.mf == 3 {
                while let Some(n) = new_iter.peek() {
                    if n.key.mt < s.key.mt {
                        out.push(new_iter.next().unwrap_or_else(|| unreachable!()));
                    } else {
                        break;
                    }
                }
                if new_iter.peek().is_some_and(|n| n.key.mt == s.key.mt) {
                    out.push(new_iter.next().unwrap_or_else(|| unreachable!()));
                } else {
                    out.push(s.clone());
                }
                continue;
            }
            if s.key.mf > 3 && !done3 {
                out.extend(new_iter.by_ref());
                done3 = true;
            }
            out.push(s.clone());
        }
        if !done3 {
            out.extend(new_iter);
        }

        let plot_text = if plot {
            Some(self.plot_text(npkk, npkt))
        } else {
            None
        };
        Ok((out, plot_text))
    }

    /// The `iprint = 1` lines of the final kerma table at one energy.
    fn final_kerma_lines(&mut self, c: &[f64], npkd: usize, npktd: usize, npkt: usize) {
        let npk = self.npk;
        let (mut ilo, mut ihi) = (false, false);
        let mut klo = vec!["    "; npk + 1];
        let mut khi = vec!["    "; npk + 1];
        if self.kchk == 1 {
            for j in 2..=npkd {
                let test = c[j + npkt] - c[j + npkt] / 10.0;
                if c[j] < test {
                    klo[j] = "low ";
                    ilo = true;
                }
                let test = c[j + 2 * npkt] + c[j + 2 * npkt] / 10.0;
                if c[j] > test {
                    khi[j] = "high";
                    ihi = true;
                }
            }
        }
        if ilo {
            self.listing.push_str(" \n");
        }
        if self.kchk == 1 {
            let mut s = format!("{:15}", "");
            for k in klo.iter().take(npkd + 1).skip(2) {
                s.push_str(&format!("      {k}    "));
            }
            self.listing.push_str(&(s + "\n"));
            let mins: Vec<f64> = (1..=npktd).map(|j| c[npk + j]).collect();
            self.listing
                .push_str(&format!("{:12}min{}", "", &line14(&mins)[1..]));
        }
        let vals: Vec<f64> = (1..=npk).map(|j| c[j]).collect();
        self.listing.push_str(&line14(&vals));
        if self.kchk == 1 {
            let maxs: Vec<f64> = (1..=npktd).map(|j| c[npk + npkt + j]).collect();
            self.listing
                .push_str(&format!("{:12}max{}", "", &line14(&maxs)[1..]));
        }
        if ihi {
            let mut s = format!("{:15}", "");
            for k in khi.iter().take(npkd + 1).skip(2) {
                s.push_str(&format!("      {k}    "));
            }
            self.listing.push_str(&(s + "\n"));
        }
    }

    /// The `nplot` file: the energy-balance check, log-log and lin-lin, for
    /// the heating and (with MT=303 requested) the photon energy production.
    fn plot_text(&self, npkk: usize, npkt: usize) -> String {
        let mut out = String::from("/\n");
        let rows = &self.rows;
        let heat = |c: &Vec<f64>, k: usize| match k {
            1 => c[2 + 2 * npkt],
            2 => c[2],
            _ => c[2 + npkt],
        };
        let mt303 = self.mt303;
        let phot = |c: &Vec<f64>, k: usize| match k {
            1 => c[npkk - 1] + c[mt303] - c[mt303 + npkt] + c[npkk - 2],
            2 => c[npkk - 1],
            _ => c[npkk - 1] + c[mt303] - c[mt303 + 2 * npkt] + c[npkk - 2],
        };
        plot_block(
            &mut out,
            rows,
            true,
            "<h>eating (e<v>-barns)",
            "<e>nergy-balance heating",
            heat,
        );
        plot_block(
            &mut out,
            rows,
            false,
            "<h>eating (e<v>-barns)",
            "<e>nergy-balance heating",
            heat,
        );
        if mt303 != 0 {
            plot_block(
                &mut out,
                rows,
                true,
                "<p>hoton energy prod (e<v>-barns)",
                "<p>hoton energy production",
                phot,
            );
            plot_block(
                &mut out,
                rows,
                false,
                "<p>hoton energy prod (e<v>-barns)",
                "<p>hoton energy production",
                phot,
            );
        }
        out.push_str("99/\n");
        out
    }
}

/// One block of `hout`'s plot file: three curves (upper limit, the value,
/// lower limit) on log-log or lin-lin axes.
fn plot_block<F: Fn(&Vec<f64>, usize) -> f64>(
    out: &mut String,
    rows: &[Vec<f64>],
    log: bool,
    yname: &str,
    k2name: &str,
    f: F,
) {
    const SMALL: f64 = 1.0e-10;
    const BIG: f64 = 1.0e10;
    const EMAX: f64 = 20.0e6;
    const ECUT: f64 = 0.01e6;
    let qu = '\'';
    let fmt2 = |x: f64, y: f64| {
        format!(
            "{}{}/\n",
            crate::acer::fortran_fmt::fortran_e(x, 6, 14),
            crate::acer::fortran_fmt::fortran_e(y, 6, 14)
        )
    };
    let elo = 1.0 / 1000.0;
    let thin = if log {
        10f64.powf((EMAX / elo).log10() / 2500.0)
    } else {
        EMAX / 2500.0
    };
    let start = if log { elo } else { ECUT };
    let (mut ylo, mut yhi) = (BIG, -BIG);
    let mut xlast = start;
    for c in rows {
        let x = c[1];
        let pass = if log {
            x >= thin * xlast
        } else {
            x >= xlast + thin
        };
        if pass {
            for k in 1..=3 {
                let y = f(c, k);
                if y > yhi {
                    yhi = y;
                }
                if y < ylo {
                    ylo = y;
                }
            }
            xlast = x;
        }
    }
    if log {
        if ylo <= 0.0 {
            ylo = SMALL;
        }
        if yhi <= 0.0 {
            yhi = SMALL;
        }
    }
    for k in 1..=3 {
        match k {
            1 => {
                out.push_str("1/\n");
                out.push_str(&format!("{qu}Energy-Balance Check{qu}/\n/\n"));
                out.push_str(if log {
                    "4 0 2 1/\n/\n"
                } else {
                    "1 0 2 1/\n/\n"
                });
                out.push_str(&format!("{qu}<e>nergy (e<v>){qu}/\n/\n"));
                out.push_str(&format!("{qu}{yname}{qu}/\n/\n"));
                out.push_str("0 0 4/\n");
                let upper = if !log && yname.starts_with("<h>") {
                    "<u>pper limit>"
                } else {
                    "<u>pper limit"
                };
                out.push_str(&format!("{qu}{upper}{qu}/\n"));
            }
            2 => out.push_str(&format!("2/\n/\n/\n{qu}{k2name}{qu}/\n")),
            _ => out.push_str(&format!("3/\n/\n0 0 4/\n{qu}<l>ower limit{qu}/\n")),
        }
        out.push_str("0/\n");
        let mut xlast = start;
        let mut j = 0;
        let mut x = 0.0;
        for c in rows {
            x = c[1];
            let mut y = f(c, k);
            let pass = if log {
                x >= thin * xlast
            } else {
                x >= xlast + thin
            };
            if pass && j < 2500 {
                j += 1;
                if log && y <= 0.0 {
                    y = SMALL;
                }
                out.push_str(&fmt2(x, y));
                xlast = x;
            }
        }
        if k == 1 {
            out.push_str(&fmt2(x, ylo));
            out.push_str(&fmt2(x, yhi));
        }
        out.push_str("/ end\n");
    }
}
