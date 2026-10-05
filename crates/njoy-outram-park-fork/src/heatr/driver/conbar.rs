// Ported from NJOY2016 `src/heatr.f90` (`conbar` 2054-2298, `hgtyld`
// 2299-2399, `anabar`/`r`/`rerfc` 2400-2500, `anadam` 2501-2593, `sed`
// 2594-2615, `tabbar` 2616-2707, `tabdam` 2708-2754).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `conbar`: the mean outgoing neutron energy and the damage energy of a
//! continuum reaction described by MF=5 (with MF=4 or none), as
//! `nheat` deposits them: `σ·(E + q0 − yld·Ē')`.
//!
//! `conbar`'s work array `c` is built exactly as upstream builds it (each
//! subsection's fractional-probability TAB1, then for a tabulated LF=1
//! subsection the TAB2 head and one `(E, Ē', d)` triple per incident energy,
//! with the raw spectrum read into the space after the triples), and indexed
//! as upstream indexes it, so the routine's few reads past a subsection's own
//! data land on what upstream would read there.

use super::flat::{list_flat, nint, tab1_flat, tab2_flat, terp1, terpa, terpa0, QP4, QW4};
use super::state::Heatr;
use crate::common::phys::PI;
use crate::endf::records::SectionCursor;
use crate::NjoyError;

/// Write `rec` into the 1-based array `c` at `c(l)`, growing `c` as needed;
/// returns the number of words written (`nw`, summed over `moreio`s).
fn put(c: &mut Vec<f64>, l: usize, rec: &[f64]) -> usize {
    if c.len() < l + rec.len() {
        c.resize(l + rec.len(), 0.0);
    }
    c[l..l + rec.len()].copy_from_slice(rec);
    rec.len()
}

impl Heatr {
    /// `conbar(e, ebar, dame, nendf, nscr, matd, mtd, ..., yld, q)`. `ebar`,
    /// `dame` and `yld` are in/out as upstream.
    pub(super) fn conbar(
        &mut self,
        e: f64,
        ebar: &mut f64,
        dame: &mut f64,
        mtd: i32,
        yld: &mut f64,
        q: f64,
        endf: &crate::endf::tape::Tape,
    ) -> Result<(), NjoyError> {
        const EMAX: f64 = 1.0e10;
        const STEP: f64 = 1.5;
        const SMALL: f64 = 1.0e-10;
        const NKMAX: usize = 12;
        let fission = (18..=21).contains(&mtd) || mtd == 38;
        if e == 0.0 {
            let mut l = 1usize;
            if !fission {
                *yld = 1.0;
                if matches!(mtd, 16 | 30 | 24 | 26 | 11 | 41) {
                    *yld = 2.0;
                }
                if matches!(mtd, 17 | 25 | 38 | 42) {
                    *yld = 3.0;
                }
                if mtd == 37 {
                    *yld = 4.0;
                }
            } else {
                self.con.mf1 = 1;
                self.con.mt1 = if self.nply > 0 { 456 } else { 452 };
                // The initialising call leaves `yld` as it was.
                self.hgtyld(e, self.con.mt1)?;
            }
            let matd = self.matd;
            let sec = endf
                .section(matd, 5, mtd)
                .ok_or(NjoyError::SectionNotFound {
                    mat: matd,
                    mf: 5,
                    mt: mtd,
                })?;
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            let mut c: Vec<f64> = vec![0.0];
            put(&mut c, l, &super::flat::cont_flat(&head));
            let nktot = usize::try_from(head.n1).unwrap_or(0);
            if nktot > NKMAX {
                return Err(NjoyError::EndfParse("conbar: nktot gt nkmax".into()));
            }
            let s = &mut self.con;
            s.nktot = nktot;
            s.awr = head.c2;
            s.awfac = 1.0 / s.awr;
            s.aw1fac = 1.0 / (s.awr + 1.0);
            s.z = nint(head.c1 / 1000.0) as f64;
            s.za = head.c1;
            s.mtt = match mtd {
                22 => 107,
                28 => 103,
                32 => 104,
                33 => 105,
                34 => 106,
                _ => 0,
            };
            s.loc = vec![0; NKMAX + 1];
            s.d1 = vec![0.0; NKMAX + 1];
            s.d2 = vec![0.0; NKMAX + 1];
            s.e1 = vec![0.0; NKMAX + 1];
            s.e2 = vec![0.0; NKMAX + 1];
            let (za, awr, mtt) = (s.za, s.awr, s.mtt);
            if mtt > 0 {
                let mut d = 0.0;
                self.capdam(e, &mut d, q, za, awr, mtt);
                *dame = d;
            }
            let mut il = 1usize;
            for ik in 1..=nktot {
                self.con.d2[ik] = 0.0;
                self.con.e2[ik] = 0.0;
                self.con.loc[ik] = il;
                l = il;
                let p = tab1_flat(&cur.read_tab1()?);
                l += put(&mut c, l, &p);
                let lf = nint(c[il + 3]);
                il = l;
                if lf > 1 {
                    // Analytic subsection.
                    if lf == 3 {
                        let (awr, aw1fac) = (self.con.awr, self.con.aw1fac);
                        self.con.t1 = (awr * awr + 1.0) * aw1fac * aw1fac;
                        self.con.t2 = awr * aw1fac;
                    } else {
                        let t = tab1_flat(&cur.read_tab1()?);
                        l += put(&mut c, l, &t);
                        il = l;
                        if lf == 5 || lf == 11 {
                            let t = tab1_flat(&cur.read_tab1()?);
                            l += put(&mut c, l, &t);
                            il = l;
                        }
                    }
                } else {
                    // Tabulated subsection.
                    let t2 = tab2_flat(&cur.read_tab2()?);
                    let nw = put(&mut c, l, &t2);
                    let ne = nint(c[l + 5]) as usize;
                    l += nw;
                    let lnow = l;
                    let iraw = lnow + 3 * ne;
                    for nne in 1..=ne {
                        let raw = tab1_flat(&cur.read_tab1()?);
                        put(&mut c, iraw, &raw);
                        let ehi = c[iraw + 1];
                        let fhi = tabbar(&c[iraw..], lf);
                        let mut dhi = 0.0;
                        if self.idame != 0 {
                            let (mtt, za, awr, z, awfac) = (
                                self.con.mtt,
                                self.con.za,
                                self.con.awr,
                                self.con.z,
                                self.con.awfac,
                            );
                            if mtt != 0 {
                                self.capdam(ehi, &mut dhi, q, za, awr, mtt);
                            } else {
                                dhi = self.tabdam(ehi, z, awr, awfac, &c[iraw..]);
                            }
                        }
                        let k = lnow + 3 * (nne - 1);
                        if c.len() < k + 3 {
                            c.resize(k + 3, 0.0);
                        }
                        c[k] = ehi;
                        c[k + 1] = fhi;
                        c[k + 2] = dhi;
                    }
                    il = lnow + 3 * ne;
                }
            }
            self.con.c = c;
            return Ok(());
        }

        // Normal entry.
        let mut enext = EMAX;
        let mut idis = false;
        *ebar = 0.0;
        *dame = 0.0;
        if fission {
            let (en, idd, y) = self.hgtyld(e, self.con.mt1)?;
            enext = en;
            idis = idd;
            *yld = y;
        }
        let nktot = self.con.nktot;
        for ik in 1..=nktot {
            let mut lnow = self.con.loc[ik];
            let c = &self.con.c;
            let lf = nint(c[lnow + 3]);
            let (pe, eihi, idisc) = terpa0(&c[lnow..], e);
            if (eihi - enext).abs() < SMALL * enext && idisc && !idis {
                idis = idisc;
            }
            if eihi < enext * (1.0 - SMALL) {
                idis = idisc;
                enext = eihi;
            }
            let nr = nint(c[lnow + 4]) as usize;
            let np = nint(c[lnow + 5]) as usize;
            let ll = lnow + 6 + 2 * nr + 2 * (np - 1);
            let etp = c[ll];
            if pe > 0.0 {
                if lf == 1 {
                    lnow = lnow + 6 + 2 * nr + 2 * np;
                    let ne = nint(c[lnow + 5]) as usize;
                    let nnr = nint(c[lnow + 4]) as usize;
                    let inn = 2;
                    lnow = lnow + 6 + 2 * nnr;
                    let mut ii = 0usize;
                    let mut idone = false;
                    let mut ie = 0usize;
                    while ie < ne && !idone {
                        ie += 1;
                        if c.get(lnow + 3 * ie).copied().unwrap_or(0.0) >= e * (1.0 - SMALL) {
                            ii = lnow + 3 * (ie - 1);
                            idone = true;
                        }
                    }
                    if !idone {
                        ii = lnow + 3 * (ne - 1);
                    }
                    let g = |k: usize| c.get(k).copied().unwrap_or(0.0);
                    let f = terp1(g(ii), g(ii + 1), g(ii + 3), g(ii + 4), e, inn);
                    *ebar += f * pe;
                    if self.idame != 0 {
                        let d = terp1(g(ii), g(ii + 2), g(ii + 3), g(ii + 5), e, inn);
                        *dame += d * pe;
                    }
                } else {
                    let s = anabar(e, &c[lnow..], self.con.t1, self.con.t2);
                    *ebar += s * pe;
                    if self.idame != 0 {
                        let (mtt, za, awr, z) =
                            (self.con.mtt, self.con.za, self.con.awr, self.con.z);
                        if mtt != 0 {
                            // Upstream passes `dame` itself, so capdam's
                            // answer replaces the running sum.
                            self.capdam(e, dame, q, za, awr, mtt);
                        } else {
                            if e >= self.con.e2[ik] * (1.0 - SMALL) {
                                loop {
                                    self.con.d1[ik] = self.con.d2[ik];
                                    self.con.e1[ik] = self.con.e2[ik];
                                    self.con.e2[ik] = STEP * self.con.e1[ik];
                                    if self.con.e2[ik] > etp * (1.0 + SMALL) {
                                        self.con.e2[ik] = etp;
                                    }
                                    if self.con.e1[ik] == 0.0 {
                                        self.con.e2[ik] = e;
                                    }
                                    let e2 = self.con.e2[ik];
                                    let sub = self.con.c[self.con.loc[ik]..].to_vec();
                                    self.con.d2[ik] = self.anadam(e2, z, awr, &sub);
                                    if (self.con.e2[ik] - etp).abs() < SMALL * etp {
                                        break;
                                    }
                                    if self.con.e1[ik] != 0.0
                                        && e <= self.con.e2[ik] * (1.0 + SMALL)
                                    {
                                        break;
                                    }
                                }
                            }
                            let s = &self.con;
                            let d = terp1(s.e1[ik], s.d1[ik], s.e2[ik], s.d2[ik], e, 2);
                            *dame += d * pe;
                        }
                    }
                }
            }
        }
        let _ = (enext, idis);
        Ok(())
    }

    /// `hgtyld(e, enext, idis, yld, mat, 1, mt, nscr, ...)`: ν̄ from MF=1 of
    /// the converted tape. `e = 0` initialises for `mt`. Returns
    /// `(enext, idis, yld)`.
    pub(super) fn hgtyld(&mut self, e: f64, mt: i32) -> Result<(f64, bool, f64), NjoyError> {
        const EMAX: f64 = 1.0e10;
        if e == 0.0 {
            let matd = self.matd;
            let sec = self
                .conv
                .section(matd, 1, mt)
                .ok_or(NjoyError::SectionNotFound {
                    mat: matd,
                    mf: 1,
                    mt,
                })?;
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?;
            self.hyl.lnu = head.l2;
            let enext;
            if mt == 455 {
                let l = cur.read_list()?;
                let lnd = l.head.n1;
                if lnd != 6 && lnd != 8 {
                    return Err(NjoyError::EndfParse(
                        "hgtyld: illegal lnd, must be 6 or 8".into(),
                    ));
                }
                self.hyl.a = tab1_flat(&cur.read_tab1()?);
                self.hyl.ip = 2;
                self.hyl.ir = 1;
                let nr = nint(self.hyl.a[4]) as usize;
                enext = self.hyl.a[6 + 2 * nr];
            } else if self.hyl.lnu != 1 {
                self.hyl.a = tab1_flat(&cur.read_tab1()?);
                self.hyl.ip = 2;
                self.hyl.ir = 1;
                let nr = nint(self.hyl.a[4]) as usize;
                enext = self.hyl.a[6 + 2 * nr];
            } else {
                self.hyl.a = list_flat(&cur.read_list()?);
                enext = EMAX;
            }
            return Ok((enext, false, 0.0));
        }
        if self.hyl.lnu != 1 {
            let (mut ip, mut ir) = (self.hyl.ip, self.hyl.ir);
            let (y, en, idis) = terpa(&self.hyl.a, e, &mut ip, &mut ir);
            self.hyl.ip = ip;
            self.hyl.ir = ir;
            Ok((en, idis, y))
        } else {
            let a = &self.hyl.a;
            let mut yld = a[6];
            let mut term = 1.0;
            let nc = nint(a[4]) as usize;
            for i in 2..=nc {
                term *= e;
                yld += a[i + 5] * term;
            }
            Ok((EMAX, false, yld))
        }
    }

    /// `anadam(e, d, z, awr, a)`: damage for an analytic LF=9 subsection.
    pub(super) fn anadam(&mut self, e: f64, z: f64, awr: f64, a: &[f64]) -> f64 {
        const IMAX: usize = 10;
        const EPS: f64 = 0.05;
        const DELTA: f64 = 1.0e-7;
        let at = |k: usize| a[k - 1];
        let mut d = 0.0;
        let u = at(1);
        let nr = nint(at(5)) as usize;
        let np = nint(at(6)) as usize;
        let lf = nint(at(4));
        let awfac = 1.0 / awr;
        if lf != 9 || e <= u || (e - u) < DELTA * e || (e - u) < 10.0 {
            return d;
        }
        let lnext = 7 + 2 * nr + 2 * np;
        let theta = terpa0(&a[lnext - 1..], e).0;
        let mut x = [0.0; IMAX + 1];
        let mut y = [0.0; IMAX + 1];
        let mut i = 4usize;
        while i > 1 {
            i -= 1;
            let mut ep = 0.0;
            if i == 3 {
                ep = 1.0;
            }
            if i == 2 {
                ep = (e - u) / 2.0;
            }
            if i == 2 && theta < e - u {
                ep = theta;
            }
            if i == 1 {
                ep = e - u;
            }
            x[i] = ep;
            y[i] = 0.0;
            for iq in 0..4 {
                let er = (e - 2.0 * (e * ep).sqrt() * QP4[iq] + ep) * awfac;
                y[i] += QW4[iq] * self.df(er, z, awr, z, awr) / 2.0;
            }
            y[i] *= sed(e, ep, theta, u);
        }
        i = 3;
        let mut j = 0;
        let (mut xlast, mut ylast) = (0.0, 0.0);
        while i > 1 {
            let mut iflag = false;
            let (mut xm, mut yt) = (0.0, 0.0);
            if i > 1 && i < IMAX {
                xm = (x[i - 1] + x[i]) / 2.0;
                let ym = (y[i - 1] + y[i]) / 2.0;
                yt = 0.0;
                for iq in 0..4 {
                    let er = (e - 2.0 * (e * xm).sqrt() * QP4[iq] + xm) * awfac;
                    yt += QW4[iq] * self.df(er, z, awr, z, awr) / 2.0;
                }
                yt *= sed(e, xm, theta, u);
                if (yt - ym).abs() > EPS * yt {
                    iflag = true;
                }
            }
            if iflag {
                i += 1;
                x[i] = x[i - 1];
                x[i - 1] = xm;
                y[i] = y[i - 1];
                y[i - 1] = yt;
            } else {
                j += 1;
                if j > 1 {
                    d += (x[i] - xlast) * (y[i] + ylast) / 2.0;
                }
                xlast = x[i];
                ylast = y[i];
                i -= 1;
            }
        }
        d
    }

    /// `tabdam(e, d, z, awr, awfac, a)`: damage for a tabulated MF=5
    /// spectrum. `e = 0` initialises `df`.
    pub(super) fn tabdam(&mut self, e: f64, z: f64, awr: f64, awfac: f64, a: &[f64]) -> f64 {
        if e == 0.0 {
            return self.df(e, z, awr, z, awr);
        }
        let at = |k: usize| a[k - 1];
        let mut d = 0.0;
        let nr = nint(at(5)) as usize;
        let np = nint(at(6)) as usize;
        let ibase = 6 + 2 * nr;
        let mut xh = at(ibase + 1);
        let mut fl = 0.0;
        for i in 1..=np {
            let xl = xh;
            xh = at(ibase + 2 * i - 1);
            let yh = at(ibase + 2 * i);
            let mut f = 0.0;
            for iq in 0..4 {
                let er = (e - 2.0 * (e * xh).sqrt() * QP4[iq] + xh) * awfac;
                f += QW4[iq] * self.df(er, z, awr, z, awr) / 2.0;
            }
            f *= yh;
            if i > 1 {
                d += (xh - xl) * (f + fl) / 2.0;
            }
            fl = f;
        }
        d
    }
}

/// `anabar(s, e, a, t1, t2)`: the mean energy of an analytic MF=5
/// subsection; `a` starts at the subsection's fractional-probability TAB1.
pub(super) fn anabar(e: f64, a: &[f64], t1: f64, t2: f64) -> f64 {
    const THRHAF: f64 = 1.5;
    let rpi2 = PI.sqrt() / 2.0;
    let at = |k: usize| a[k - 1];
    let mut s = 0.0;
    let u = at(1);
    if e <= u {
        return s;
    }
    let lf = nint(at(4));
    let nr = nint(at(5)) as usize;
    let np = nint(at(6)) as usize;
    let lnext = 7 + 2 * nr + 2 * np;
    let theta = terpa0(&a[lnext - 1..], e).0;
    match lf {
        3 => s = e * t1 - t2 * theta,
        5 => {
            let nrn = nint(at(lnext + 4)) as usize;
            let npn = nint(at(lnext + 5)) as usize;
            let lnxt = lnext + 2 * nrn + 2 * npn + 6;
            s = tabbar(&a[lnxt - 1..], lf) * theta;
        }
        7 => {
            if theta != 0.0 {
                let b1 = (e - u) / theta;
                let b2 = b1.sqrt();
                let b3 = (-b1).exp();
                let expa = (-b2 * b2).exp();
                s = theta * (THRHAF - b1 * b2 * b3 / (rpi2 * (1.0 - expa * rerfc(b2)) - b2 * b3));
            }
        }
        9 => {
            if theta != 0.0 {
                let b1 = (e - u) / theta;
                let b3 = (-b1).exp();
                if b1 >= 1.0 / 1000.0 {
                    s = theta * (2.0 - b1 * b1 * b3 / (1.0 - (b1 + 1.0) * b3));
                } else {
                    s = 4.0 * (e - u) / 3.0;
                }
            }
        }
        11 => {
            let nrn = nint(at(lnext + 4)) as usize;
            let npn = nint(at(lnext + 5)) as usize;
            let lnxt = lnext + 6 + 2 * nrn + 2 * npn;
            let b = terpa0(&a[lnxt - 1..], e).0;
            s = theta * (THRHAF + theta * b / 4.0);
        }
        12 => s = ((at(lnext) + at(lnext + 1)) / 2.0) + (4.0 * theta / 3.0),
        _ => {}
    }
    s
}

/// `rerfc` (internal to `anabar`): the rational approximation of
/// `exp(z²)·erfc(z)` upstream uses.
fn rerfc(z: f64) -> f64 {
    const A0: f64 = 0.3275911;
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    let r = 1.0 / (1.0 + A0 * z.abs());
    r * (A1 + r * (A2 + r * (A3 + r * (A4 + A5 * r))))
}

/// `sed(e, ep, theta, u)`: the evaporation spectrum `anadam` integrates.
fn sed(e: f64, ep: f64, theta: f64, u: f64) -> f64 {
    let xeu = (e - u) / theta;
    let aeu = xeu.abs();
    if aeu >= 1.0 / 10000.0 {
        ep * (-ep / theta).exp() / (theta * theta * (1.0 - (-xeu).exp() * (1.0 + xeu)))
    } else {
        ep * (-ep / theta).exp() / (theta * theta * xeu * xeu * (1.0 - xeu) / 2.0)
    }
}

/// `tabbar(f, a, law)`: the mean energy of a tabulated spectrum. For
/// `law > 0`, `a` is an MF=5 TAB1 (LF=1 or 5); for `law < 0`, an MF=6 LAW=1
/// LIST, with `|law|` its interpolation law.
pub(super) fn tabbar(a: &[f64], law: i64) -> f64 {
    let at = |k: usize| a[k - 1];
    let mut f = 0.0;
    let (np, ibase, ncyc, mut inn, mut ir, mut nbt);
    if law >= 0 {
        let nr = nint(at(5)) as usize;
        np = nint(at(6)) as usize;
        ibase = 6 + 2 * nr;
        ir = 1usize;
        nbt = nint(at(7)) as usize;
        inn = nint(at(8));
        ncyc = 2usize;
    } else {
        np = nint(at(6)) as usize;
        inn = law.abs();
        ncyc = nint(at(4)) as usize + 2;
        ibase = 6;
        ir = 0;
        nbt = 0;
    }
    let mut xh = at(ibase + 1);
    let mut xhsq = xh * xh;
    let mut yh = at(ibase + 2);
    for i in 2..=np {
        let xl = xh;
        xh = at(ibase + ncyc * (i - 1) + 1);
        let xlsq = xhsq;
        xhsq = xh * xh;
        let delxsq = xhsq - xlsq;
        let yl = yh;
        yh = at(ibase + ncyc * (i - 1) + 2);
        if law > 0 && i > nbt {
            ir += 1;
            nbt = nint(at(6 + 2 * ir - 1)) as usize;
            inn = nint(at(6 + 2 * ir));
        }
        if xl != xh {
            match inn {
                1 => f += yl * delxsq / 2.0,
                2 => {
                    let b = (yh - yl) / (xh - xl);
                    f += (yl - xl * b) * delxsq / 2.0 + b * (xh * xhsq - xl * xlsq) / 3.0;
                }
                3 => {
                    let (alxl, alxh) = (xl.ln(), xh.ln());
                    let b = (yh - yl) / (alxh - alxl);
                    f += (yl - b * alxl + b / 2.0) * delxsq / 2.0
                        + b * (xhsq * alxh - xlsq * alxl) / 2.0;
                }
                4 => {
                    let (alyl, alyh) = (yl.ln(), yh.ln());
                    let b = (alyh - alyl) / (xh - xl);
                    f += (yh * (b * xh - 1.0) - yl * (b * xl - 1.0)) / (b * b);
                }
                5 => {
                    let (alxl, alxh, alyl, alyh) = (xl.ln(), xh.ln(), yl.ln(), yh.ln());
                    let b = (alyh - alyl) / (alxh - alxl);
                    f += yl * xlsq * ((xh / xl).powf(b + 2.0) - 1.0) / (b + 2.0);
                }
                _ => {}
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat spectrum on [0, 2] has mean energy 1.
    #[test]
    fn tabbar_of_a_flat_spectrum() {
        let a = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 0.0, 0.5, 2.0, 0.5];
        assert!((tabbar(&a, 1) - 1.0).abs() < 1.0e-15);
    }

    /// LF=9 evaporation: upstream's closed form, which tends to `2θ` far
    /// above the restriction energy.
    #[test]
    fn anabar_evaporation_limit() {
        // p(E) TAB1: U = 0, one region, two points; θ TAB1: constant 1 MeV.
        let mut a = vec![0.0, 0.0, 0.0, 9.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 2.0e7, 1.0];
        a.extend_from_slice(&[
            0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0e6, 2.0e7, 1.0e6,
        ]);
        let (e, theta) = (1.0e7, 1.0e6);
        let b1: f64 = e / theta;
        let want = theta * (2.0 - b1 * b1 * (-b1).exp() / (1.0 - (b1 + 1.0) * (-b1).exp()));
        let s = anabar(e, &a, 0.0, 0.0);
        assert!((s - want).abs() < 1.0e-9 * want, "{s} vs {want}");
        // At E/θ = 10 the closed form is 0.45 % below 2θ.
        assert!((s - 2.0 * theta).abs() < 0.005 * theta);
    }
}
