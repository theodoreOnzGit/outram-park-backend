// Ported from NJOY2016 `src/heatr.f90` (`hgtfle`, lines 4214-4403;
// `hgetco`, lines 4404-4552).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `hgtfle` / `hgetco`: MF=4 Legendre coefficients at an incident energy.
//!
//! These are HEATR's copies of GROUPR's `getfle` / `getco`
//! (`groupr::file4::File4Angular`), and they differ from them in ways that
//! change answers, which is why they are translated here rather than
//! reached through that type: isotropy returns `etop` as the next energy
//! (GROUPR: `1e10`); an LTT=3 section switches to its tabulated block when
//! the Legendre block runs out; the last panel is extended to `ehi + ehi/100`;
//! and a two-point section whose both points are isotropic counts as
//! isotropic.
//!
//! Two upstream behaviours are kept on purpose. When the low-energy
//! coefficients are replaced by the high-energy ones, only `nhi` of them are
//! copied (`:4302-4304`), so `flo` beyond `nhi` keeps older values, which
//! the next interpolation can use. And a request above the last energy of a
//! section is an error upstream (`:4318`), returned here as one.

use super::flat::{legndr1, list_flat, nint, tab1_flat, tab2_flat, terp1, terpa, QP64, QW64};
use super::state::Heatr;
use crate::endf::records::SectionCursor;
use crate::NjoyError;

/// `hgtfle`'s answer.
pub(super) struct Fle {
    /// `fle(1..=nle)`, 1-based (element 0 unused).
    pub fle: [f64; 66],
    pub nle: usize,
    pub enext: f64,
    pub idis: bool,
}

impl Heatr {
    /// `hgtfle(e, enext, idis, fle, nle, lcd, matd, 4, mtd, nend4, ...)`.
    /// Call with `e = 0` to initialise for reaction `mtd`.
    pub(super) fn hgtfle(
        &mut self,
        e: f64,
        nle_in: usize,
        lcd: i32,
        mtd: i32,
    ) -> Result<Fle, NjoyError> {
        const SMALL: f64 = 1.0e-10;
        let mut out = Fle {
            fle: [0.0; 66],
            nle: nle_in,
            enext: 0.0,
            idis: false,
        };
        if e <= 0.0 {
            let matd = self.matd;
            let sec = self
                .conv
                .section(matd, 4, mtd)
                .ok_or(NjoyError::SectionNotFound {
                    mat: matd,
                    mf: 4,
                    mt: mtd,
                })?
                .clone();
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            let fle = &mut self.fle;
            fle.iso = 0;
            fle.awr = head.c2;
            let lvt = head.l1;
            fle.ltt = head.l2;
            fle.ltt3 = fle.ltt;
            if fle.ltt == 3 {
                fle.ltt = 1;
                fle.lttn = 1;
            }
            let second = if lvt == 0 {
                cur.read_cont()?
            } else {
                cur.read_list()?.head
            };
            fle.iso = second.l1;
            fle.lct = second.l2;
            if fle.lct == 1 && fle.awr >= 10.0 && (51..=90).contains(&mtd) {
                self.mess(
                    "hgtfle",
                    &format!("lab distribution changed to cm for mt {mtd:3}"),
                    " ",
                );
                self.fle.lct = 2;
            }
            let fle = &mut self.fle;
            fle.tab2.clear();
            fle.recs.clear();
            fle.next = 0;
            fle.tab2b.clear();
            fle.recsb.clear();
            if fle.iso != 1 {
                // The first block: a TAB2 and its NE records.
                let t2 = cur.read_tab2()?;
                let ne = usize::try_from(t2.head.n2).unwrap_or(0);
                fle.tab2 = tab2_flat(&t2);
                fle.ne = ne;
                let ltt_first = fle.ltt;
                for _ in 0..ne {
                    fle.recs.push(if ltt_first == 1 {
                        list_flat(&cur.read_list()?)
                    } else {
                        tab1_flat(&cur.read_tab1()?)
                    });
                }
                // LTT=3: the tabulated block follows.
                if fle.ltt3 == 3 {
                    let t2b = cur.read_tab2()?;
                    let neb = usize::try_from(t2b.head.n2).unwrap_or(0);
                    fle.tab2b = tab2_flat(&t2b);
                    for _ in 0..neb {
                        fle.recsb.push(tab1_flat(&cur.read_tab1()?));
                    }
                }
                let (lct, ltt, awr) = (fle.lct, fle.ltt, fle.awr);
                let r0 = self.fle_next_record()?;
                self.fle.elo = r0[1];
                let mut nlo = nle_in;
                let mut flo = [0.0; 66];
                hgetco(&mut flo, &mut nlo, lcd, &r0, lct, ltt, awr)?;
                self.fle.flo = flo;
                self.fle.nlo = nlo;
                let r1 = self.fle_next_record()?;
                self.fle.ehi = r1[1];
                let mut nhi = nle_in;
                let mut fhi = [0.0; 66];
                hgetco(&mut fhi, &mut nhi, lcd, &r1, lct, ltt, awr)?;
                self.fle.fhi = fhi;
                self.fle.nhi = nhi;
                if self.fle.ne == 2 && nlo == 1 && nhi == 1 {
                    self.fle.iso = 1;
                }
            }
            let fle = &mut self.fle;
            if fle.iso == 1 {
                out.nle = 1;
                out.enext = self.etop;
            } else {
                out.enext = fle.elo;
            }
            fle.nne = 2;
            fle.ir = 1;
            fle.inn = if fle.tab2.len() >= 8 {
                nint(fle.tab2[7])
            } else {
                0
            };
            return Ok(out);
        }

        // Normal entry.
        let nle = nle_in;
        if nle == 1 || self.fle.iso == 1 {
            return Ok(isotropic(nle, self.etop));
        }
        let goto_130 = e < self.fle.ehi * (1.0 - SMALL)
            || (self.fle.nne == self.fle.ne && e < self.fle.ehi + self.fle.ehi / 100.0);
        if !goto_130 {
            loop {
                // 120: slide the high-energy data down.
                let nhi = self.fle.nhi;
                for i in 1..=nhi {
                    self.fle.flo[i] = self.fle.fhi[i];
                }
                self.fle.nlo = nhi;
                self.fle.elo = self.fle.ehi;
                if self.fle.nne == self.fle.ne && self.fle.ltt3 == 3 && self.fle.lttn == 1 {
                    let fle = &mut self.fle;
                    fle.tab2 = std::mem::take(&mut fle.tab2b);
                    fle.recs = std::mem::take(&mut fle.recsb);
                    fle.next = 0;
                    fle.ne = nint(fle.tab2[5]) as usize;
                    fle.nne = 0;
                    fle.ir = 1;
                    fle.ltt = 2;
                    fle.lttn = 2;
                } else if self.fle.nne == self.fle.ne {
                    return Err(NjoyError::EndfParse(format!(
                        "hgtfle: desired energy {e:e} above highest given (MF=4 MT={mtd})"
                    )));
                }
                let r = self.fle_next_record()?;
                self.fle.ehi = r[1];
                let mut nhi = nle;
                let mut fhi = self.fle.fhi;
                let (lct, ltt, awr) = (self.fle.lct, self.fle.ltt, self.fle.awr);
                hgetco(&mut fhi, &mut nhi, lcd, &r, lct, ltt, awr)?;
                let fle = &mut self.fle;
                fle.fhi = fhi;
                fle.nhi = nhi;
                fle.nne += 1;
                let nbt_at = 4 + 2 * fle.ir; // c(5+2*ir), 0-based
                if nbt_at < fle.tab2.len() && fle.nne > nint(fle.tab2[nbt_at]) as usize {
                    fle.ir += 1;
                }
                let int_at = 5 + 2 * fle.ir; // c(6+2*ir), 0-based
                if int_at < fle.tab2.len() {
                    fle.inn = nint(fle.tab2[int_at]);
                }
                if !(fle.ehi <= e * (1.0 + SMALL) && fle.nne < fle.ne) {
                    break;
                }
            }
        }
        // 130
        let fle = &self.fle;
        if e < fle.elo * (1.0 - SMALL) {
            // 140: isotropic below the first point.
            let mut o = Fle {
                fle: [0.0; 66],
                nle: 1,
                enext: fle.ehi,
                idis: true,
            };
            o.fle[1] = 1.0;
            return Ok(o);
        }
        let nlmax = fle.nlo.max(fle.nhi);
        for i in 1..=nle {
            if i <= nlmax {
                let mut innt = fle.inn;
                if (innt == 4 || innt == 5) && fle.flo[i] * fle.fhi[i] <= 0.0 {
                    innt -= 2;
                }
                out.fle[i] = terp1(fle.elo, fle.flo[i], fle.ehi, fle.fhi[i], e, innt);
            } else {
                out.fle[i] = 0.0;
            }
        }
        out.nle = nlmax;
        out.enext = fle.ehi;
        out.idis = fle.inn == 1;
        Ok(out)
    }

    /// The next raw record of the current block (`listio`/`tab1io` into
    /// `c(iraw)`).
    fn fle_next_record(&mut self) -> Result<Vec<f64>, NjoyError> {
        let fle = &mut self.fle;
        let r = fle
            .recs
            .get(fle.next)
            .cloned()
            .ok_or_else(|| NjoyError::EndfParse("hgtfle: MF=4 section ended early".into()))?;
        fle.next += 1;
        Ok(r)
    }
}

/// Label 150: the isotropic answer.
fn isotropic(nle: usize, etop: f64) -> Fle {
    let mut o = Fle {
        fle: [0.0; 66],
        nle: 1,
        enext: etop,
        idis: false,
    };
    o.fle[1] = 1.0;
    let _ = nle;
    o
}

/// `hgetco(fl, nl, lcd, c, lct, ltt, awr, idis)`: Legendre coefficients from
/// one raw MF=4 record (`c`, flat), in the system `lcd` (1 lab, 2 CM).
/// `fl` is 1-based; on return `nl` is the number required.
pub(super) fn hgetco(
    fl: &mut [f64; 66],
    nl: &mut usize,
    lcd: i32,
    c: &[f64],
    lct: i32,
    ltt: i32,
    awr: f64,
) -> Result<(), NjoyError> {
    const TOLER: f64 = 1.0e-6;
    const RTOLER: f64 = 1.0e6;
    const HALF: f64 = 0.5;
    const NLMAX: usize = 65;
    let at = |k: usize| c[k - 1];
    let l = nl.saturating_sub(1);
    if *nl > NLMAX {
        return Err(NjoyError::EndfParse(
            "hgetco: limited to 64 legendre coefficients".into(),
        ));
    }
    let integrate = ltt == 2 || (lcd == 1 && lct != 1) || (lcd == 2 && lct < 2);
    if !integrate {
        // Coefficients straight from the raw data.
        let np = nint(at(5)) as usize + 1;
        fl[1] = 1.0;
        for il in 2..=*nl {
            fl[il] = if il <= np { at(il + 5) } else { 0.0 };
        }
        if np < *nl {
            *nl = np;
        }
        return Ok(());
    }
    // 110: integrate for fl in the desired system.
    fl[1] = 1.0;
    for v in fl.iter_mut().take(*nl + 1).skip(2) {
        *v = 0.0;
    }
    let (mut ipc, mut irc) = (2usize, 1usize);
    for iq in 0..64 {
        let x = QP64[iq];
        let mut y = if ltt != 2 {
            let np = nint(at(5)) as usize;
            let p = legndr1(x, np);
            let mut y = HALF;
            for ip in 1..=np {
                y += (2 * ip + 1) as f64 * p[ip + 1] * at(ip + 6) / 2.0;
            }
            y
        } else {
            terpa(c, x, &mut ipc, &mut irc).0
        };
        if lcd == 2 && lct == 1 {
            return Err(NjoyError::NotPorted(
                "hgetco: lab to cm conversion (not coded upstream either)",
            ));
        }
        let xn = if lcd == 1 && lct >= 2 {
            (1.0 + awr * x) / (1.0 + awr * awr + 2.0 * awr * x).sqrt()
        } else {
            x
        };
        let p = legndr1(xn, l);
        y *= QW64[iq];
        for il in 2..=*nl {
            fl[il] += y * p[il];
        }
    }
    // Reduce the significant figures and count the coefficients required.
    let mut nlz = 1;
    for il in 2..=*nl {
        let j = nint(fl[il] * RTOLER);
        fl[il] = j as f64 * TOLER;
        if fl[il].abs() > 0.0 {
            nlz = il;
        }
    }
    *nl = nlz;
    Ok(())
}
