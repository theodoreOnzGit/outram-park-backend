// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `ascale` l.2426-2567, `getz` l.2569-2705, `gety4` l.2707-2780,
//   `gety6` l.2782-2875, `fixl7` l.2877-2968; and the call of `endf.f90`'s
//   `gety1` (l.1940-2051) on a tape.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! PLOTR's value getters, each with its Fortran `save` state as a struct.
//!
//! - [`Gety1Tape`]: `gety1` on the next TAB1 of a tape, through the crate's
//!   [`Gety1`] (the whole TAB1 is read at the `x = 0` call; `gety1`'s paging
//!   gives the same values, see `endf/gety1.rs`). The first page is left in
//!   the caller's `a`, where PLOTR reads `a(2)` and `a(6)` back.
//! - [`Getz`]: `getz`, `gety1` for up to ten tapes at once (difference and
//!   ratio plots). Its `lt` is one `save` variable for every tape, as upstream.
//! - [`Gety4`]: Legendre coefficient `nth` of an MF=4 section against energy.
//! - [`Gety6`]: an MF=6 LIST, paged exactly as upstream pages it (the
//!   `ip2` test that ends a one-page LIST two panels early is kept).
//! - [`fixl7`]: an MF=6 LAW=7 angle-energy record summed over angle.
//! - [`ascale`]: the SC4020 linear axis scaler.

use crate::endf::gety1::Gety1;
use crate::endf::interp::{terp1 as endf_terp1, IntLaw};
use crate::endf::records::{Cont, Tab1};
use crate::NjoyError;

use super::stream::{nint, EndfStream, Head, NPAGE};

type Res<T> = Result<T, NjoyError>;

/// `terp1` (`endf.f90:1589`) through [`crate::endf::interp::terp1`].
///
/// # Errors
/// A log law on a non-positive argument, where upstream computes a NaN.
pub fn terp1(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, int: i32) -> Res<f64> {
    endf_terp1(x1, y1, x2, y2, x, IntLaw::from_code(int.max(0) as u32)).map_err(|_| {
        NjoyError::NotPorted("plotr: terp1 of a log law on a non-positive value (a NaN upstream)")
    })
}

/// `ascale(m, z1, z2, major, minor)`: returns `(z1, z2, major, minor)`.
pub fn ascale(m: i32, z1: f64, z2: f64) -> (f64, f64, i32, i32) {
    let mut zmin = z1;
    let mut zmax = z2;
    if zmax <= zmin || m <= 0 || m > 20 {
        return (0.0, 2.0 * z2, 1, 0);
    }
    let fm = f64::from(m);
    if zmax == 0.0 || zmin == 0.0 {
        zmax -= zmax.abs() / 1_000_000.0;
        zmin += zmin.abs() / 1_000_000.0;
    } else {
        let mut zbar = zmax / zmin;
        if zbar.abs() / 1000.0 >= 1.0 {
            zmin = 0.0;
        }
        if zbar.abs() * 1000.0 <= 1.0 {
            zmax = 0.0;
            zmax -= zmax.abs() / 1_000_000.0;
            zmin += zmin.abs() / 1_000_000.0;
        } else if (zbar - 1.0).abs() - 5.0 * fm / 100_000.0 > 0.0 {
            zmax -= zmax.abs() / 1_000_000.0;
            zmin += zmin.abs() / 1_000_000.0;
        } else {
            zbar = (zmax + zmin) / 2.0;
            let z = 26.0 * fm * zbar.abs() / 1_000_000.0;
            zmax = zbar + z;
            zmin = zbar - z;
        }
    }
    let mut p = (zmax - zmin) / fm;
    let mut iflag = 0;
    let mut tenk = 1.0;
    if p < 1.0 {
        iflag = 1;
        p = 1.0 / p;
    }
    while p >= 10000.0 {
        p /= 10000.0;
        tenk *= 10000.0;
    }
    while p >= 10.0 {
        p /= 10.0;
        tenk *= 10.0;
    }
    if iflag != 0 {
        p = 10.0 / p;
        tenk = 1.0 / 10.0 / tenk;
    }
    let nm;
    if p <= 2.0 + 1.0 / 100.0 {
        if p <= 2.0 - 1.0 / 100.0 {
            p = 1.0;
            nm = 5;
        } else {
            p = 2.0;
            nm = 4;
        }
    } else if p < 5.0 - 1.0 / 100.0 {
        p = 2.0;
        nm = 4;
    } else {
        p = 5.0;
        nm = 5;
    }
    let dz = p * tenk;
    let mut n1 = (zmin / dz) as i32;
    let mut z = f64::from(n1) * dz;
    if z > zmin {
        z -= dz;
        n1 -= 1;
    }
    zmin = z;
    let mut n2 = (zmax / dz) as i32;
    z = f64::from(n2) * dz;
    if z < zmax {
        n2 += 1;
        z += dz;
    }
    zmax = z;
    let major = n2 - n1;
    (zmin, zmax, major, nm * major)
}

/// A TAB1 read whole (`tab1io` and every `moreio`), its first page left in
/// `a(at..)` as upstream leaves it. Returns the flat record.
fn read_tab1_whole(s: &mut EndfStream, h: &mut Head, a: &mut [f64], at: usize) -> Res<Vec<f64>> {
    let (mut nb, nw) = s.tab1io(h, a, at)?;
    let mut full = a[at - 1..at - 1 + nw].to_vec();
    let mut buf = vec![0.0; NPAGE];
    while nb != 0 {
        let (b, w) = s.moreio(&mut buf, 1, nb)?;
        full.extend_from_slice(&buf[..w]);
        nb = b;
    }
    Ok(full)
}

/// `gety1(x, xnext, idis, y1, itape, a)` with its `save` state.
#[derive(Debug, Clone, Default)]
pub struct Gety1Tape {
    g: Option<Gety1>,
}

impl Gety1Tape {
    /// One call: `x <= 0` reads the next TAB1 on `s`. Returns `(y, xnext, idis)`.
    ///
    /// # Errors
    /// A malformed TAB1, or `x > 0` before any `x = 0` call (undefined upstream).
    pub fn call(
        &mut self,
        x: f64,
        s: &mut EndfStream,
        h: &mut Head,
        a: &mut [f64],
    ) -> Res<(f64, f64, i32)> {
        if x > 0.0 {
            let g = self.g.as_mut().ok_or(NjoyError::NotPorted(
                "plotr: gety1 before its first record (undefined upstream)",
            ))?;
            let v = g.get(x);
            return Ok((v.y, v.xnext, i32::from(v.idis)));
        }
        let f = read_tab1_whole(s, h, a, 1)?;
        let nr = nint(f[4]).max(0) as usize;
        let np = nint(f[5]).max(0) as usize;
        let t = Tab1 {
            head: Cont {
                c1: f[0],
                c2: f[1],
                l1: nint(f[2]),
                l2: nint(f[3]),
                n1: nr as i32,
                n2: np as i32,
            },
            interp: (0..nr)
                .map(|i| (nint(f[6 + 2 * i]) as u32, nint(f[7 + 2 * i]) as u32))
                .collect(),
            pairs: (0..np)
                .map(|i| (f[6 + 2 * nr + 2 * i], f[7 + 2 * nr + 2 * i]))
                .collect(),
        };
        let mut g = Gety1::new(&t);
        let v = g.get(0.0);
        self.g = Some(g);
        Ok((v.y, v.xnext, 0))
    }
}

/// One tape's pointers in `getz` (`nrt`, `npt`, `irt`, `ipt`, `ip1t`, `ip2t`,
/// `nbt`, `nwt`), with its record held whole (so `nb = 0` throughout).
#[derive(Debug, Clone)]
struct GetzTape {
    itape: i32,
    rec: Vec<f64>,
    np: usize,
    ir: usize,
    ip: usize,
    ip1: usize,
    ip2: usize,
    nwtot: usize,
}

/// `getz`'s `save` state: up to ten tapes and one `lt`.
#[derive(Debug, Clone, Default)]
pub struct Getz {
    tapes: Vec<GetzTape>,
    lt: usize,
}

const GETZ_BIG: f64 = 1.0e10;

impl Getz {
    /// `getz` with `x < 0`: clear the pointers.
    pub fn clear(&mut self) {
        self.tapes.clear();
    }

    /// `getz` with `x = 0`: read the next TAB1 of `itape` (first page into
    /// `a(at..)`). Returns `xnext`.
    ///
    /// # Errors
    /// A malformed TAB1; more than ten tapes (upstream's arrays hold ten).
    pub fn init(
        &mut self,
        itape: i32,
        s: &mut EndfStream,
        h: &mut Head,
        a: &mut [f64],
        at: usize,
    ) -> Res<f64> {
        if self.tapes.len() == 10 {
            return Err(NjoyError::NotPorted(
                "plotr getz: more than ten tapes (upstream's arrays hold ten)",
            ));
        }
        let rec = read_tab1_whole(s, h, a, at)?;
        let nr = nint(rec[4]).max(0) as usize;
        let np = nint(rec[5]).max(0) as usize;
        self.lt = 6 + 2 * nr;
        let nwtot = rec.len();
        let ip2 = (nwtot - self.lt) / 2 + 2;
        let xnext = rec.get(self.lt).copied().unwrap_or(0.0);
        self.tapes.push(GetzTape {
            itape,
            rec,
            np,
            ir: 1,
            ip: 2,
            ip1: 1,
            ip2,
            nwtot,
        });
        Ok(xnext)
    }

    /// `getz` with `x > 0`. Returns `(z, xnext, idis)`.
    ///
    /// # Errors
    /// "not properly initialized"; a log law on a non-positive value.
    pub fn get(&mut self, x: f64, itape: i32) -> Res<(f64, f64, i32)> {
        if self.tapes.is_empty() {
            return Err(NjoyError::InvalidInput(
                "getz: not properly initialized".into(),
            ));
        }
        let lt = self.lt;
        let Some(t) = self.tapes.iter_mut().find(|t| t.itape == itape) else {
            return Ok((0.0, GETZ_BIG, 0));
        };
        let at = |k: usize| t.rec.get(k.wrapping_sub(1)).copied().unwrap_or(0.0);
        let mut idis = 0;
        loop {
            // 125
            let ln = 2 * (t.ip - t.ip1) + lt;
            if x < at(ln - 1) {
                // 135
                return Ok((0.0, at(ln - 1), idis));
            }
            if x < at(ln + 1) {
                break;
            }
            if t.ip == t.np {
                // 140
                return Ok((at(ln + 2), GETZ_BIG, idis));
            }
            t.ip += 1;
            let nbx = nint(at(5 + 2 * t.ir)) as usize;
            if t.ip > nbx {
                t.ir += 1;
            }
            if t.ip < t.ip2 {
                continue;
            }
            // `nb = 0`: go to 130.
            break;
        }
        // 130
        let ln = 2 * (t.ip - t.ip1) + lt;
        let int = nint(at(6 + 2 * t.ir));
        if int == 1 {
            idis = 1;
        }
        let z = terp1(at(ln - 1), at(ln), at(ln + 1), at(ln + 2), x, int)?;
        let xnext = at(ln + 1);
        if ln + 3 > t.nwtot {
            return Ok((z, xnext, idis));
        }
        if at(ln + 3) == xnext {
            idis = 1;
        }
        Ok((z, xnext, idis))
    }
}

/// `gety4`'s `save` state (`xlast`, `ylast`, `np`, `ip`, `na`, `nal`).
#[derive(Debug, Clone, Default)]
pub struct Gety4 {
    xlast: f64,
    ylast: f64,
    np: usize,
    ip: usize,
    na: i32,
}

const BIG: f64 = 1.0e10;

impl Gety4 {
    /// `gety4(x, xnext, idis, y4, nth, nin, a)`. Returns `(y4, xnext)`; `idis`
    /// is not set by upstream and so is left to the caller.
    ///
    /// # Errors
    /// "only Legendre coefficient representations supported"; tape errors.
    pub fn call(
        &mut self,
        x: f64,
        nth: i32,
        s: &mut EndfStream,
        h: &mut Head,
        a: &mut [f64],
    ) -> Res<(f64, f64)> {
        let th = |a: &[f64], na: i32| {
            if nth <= na {
                a[(5 + nth) as usize]
            } else {
                0.0
            }
        };
        if x == 0.0 {
            let ltt = nint(a[3]);
            s.contio(h, a, 1)?;
            let li = nint(a[3]);
            if (ltt != 1 && ltt != 3) || li == 1 {
                return Err(NjoyError::InvalidInput(
                    "gety4: only Legendre coefficient representations supported".into(),
                ));
            }
            s.tab2io(h, a, 1)?;
            self.np = nint(a[5]).max(0) as usize;
            s.listio(h, a, 1)?;
            self.na = nint(a[4]);
            self.xlast = a[1];
            self.ylast = th(a, self.na);
            self.ip = 2;
        }
        loop {
            // 110
            if x < self.xlast {
                return Ok((0.0, self.xlast));
            }
            if x < a[1] {
                let ynow = th(a, self.na);
                let y = terp1(self.xlast, self.ylast, a[1], ynow, x, 2)?;
                return Ok((y, a[1]));
            }
            if self.ip == self.np {
                self.xlast = BIG;
                return Ok((a[(5 + nth) as usize], BIG));
            }
            self.xlast = a[1];
            self.ylast = th(a, self.na);
            self.ip += 1;
            s.listio(h, a, 1)?;
            self.na = nint(a[4]);
        }
    }
}

/// `gety6`'s `save` state.
///
/// `ylast` is not in upstream's `save` list (`plotr.f90:2803`), so it is
/// defined only inside the call that set it: a call that interpolates (label
/// 120) without first moving up a panel reads it undefined. That is every
/// first point of an MF=6 curve, and NJOY2016.80 then plots a value from
/// whatever held that stack slot (measured on Al-27 ENDF/B-VIII.0 MF=6 MT=16:
/// `8.245559e-8`, a point of an earlier LIST, where the panel's own value is
/// `5.462661e-8`). This port refuses such a call by name.
#[derive(Debug, Clone, Default)]
pub struct Gety6 {
    xlast: f64,
    nwtot: usize,
    nep: i32,
    ncyc: usize,
    lt: usize,
    ip1: i32,
    ip2: i32,
    ip: i32,
    nb: usize,
}

/// The arguments of `gety6` that stay fixed through a walk.
#[derive(Debug, Clone, Copy)]
pub struct Gety6Args {
    /// `i6`: the dependent variable in the cycle.
    pub i6: usize,
    /// `lep`: the interpolation law for secondary energy.
    pub lep: i32,
    /// `law`: the MF=6 law (7 converts through [`fixl7`]).
    pub law: i32,
    /// `maa`: the length of the LAW=7 scratch `aa`.
    pub maa: usize,
}

impl Gety6 {
    /// `gety6(i6, x, xnext, idis, y6, itape, a, lep, law, aa, maa)`. `y6` is
    /// left unchanged where upstream does not set it (the `x = 0` call).
    /// Returns `(xnext, idis)`.
    ///
    /// # Errors
    /// Tape errors; those of [`fixl7`].
    #[allow(clippy::too_many_arguments)] // upstream's argument list
    pub fn call(
        &mut self,
        g: Gety6Args,
        x: f64,
        y6: &mut f64,
        s: &mut EndfStream,
        h: &mut Head,
        a: &mut [f64],
        aa: &mut [f64],
    ) -> Res<(f64, i32)> {
        let mut idis = 0;
        let i6 = g.i6;
        if x == 0.0 {
            let (nb, nw) = if g.law == 7 {
                fixl7(s, h, aa, g.maa, a)?
            } else {
                s.listio(h, a, 1)?
            };
            self.nb = nb;
            self.nwtot = nw;
            if h.mth < 221 || h.mth > 250 {
                self.ncyc = (h.l2h + 2).max(1) as usize;
                self.nep = h.n2h;
            } else {
                self.ncyc = h.n2h.max(1) as usize;
                self.nep = h.n1h / self.ncyc as i32;
            }
            self.lt = 7;
            self.xlast = a[self.lt - 1];
            self.ip1 = 1;
            self.ip2 = ((nw - 6) / self.ncyc) as i32;
            self.ip = 2;
            return Ok((a[self.lt - 1], idis));
        }
        let ncyc = self.ncyc;
        let mut ln;
        let mut ylast: Option<f64> = None;
        loop {
            // 110
            if x < self.xlast {
                *y6 = 0.0;
                return Ok((self.xlast, idis));
            }
            ln = ncyc * (self.ip - self.ip1) as usize + self.lt;
            if x < a[ln - 1] {
                break;
            }
            if self.ip == self.nep {
                *y6 = a[ln - 1 + i6];
                self.xlast = BIG;
                return Ok((BIG, idis));
            }
            self.xlast = a[ln - 1];
            ylast = Some(a[ln - 1 + i6]);
            self.ip += 1;
            if self.ip + 2 <= self.ip2 {
                continue;
            }
            if self.nb == 0 {
                break;
            }
            let nc = self.nwtot - ln + 1;
            for i in 1..=nc {
                a[self.lt + i - 2] = a[self.nwtot - nc + i - 1];
            }
            let (nb, nw) = s.moreio(a, self.lt + nc, self.nb)?;
            self.nb = nb;
            self.nwtot = nw + self.lt + nc - 1;
            self.ip1 = self.ip;
            self.ip2 = self.ip1 + (nw / ncyc) as i32 + 1;
            if self.nb == 0 {
                self.ip2 += 2;
            }
        }
        // 120
        let int = g.lep;
        if int == 1 {
            idis = 1;
        }
        let ylast = ylast.ok_or(NjoyError::NotPorted(
            "plotr gety6: interpolation before moving up a panel reads ylast, which is not saved (undefined upstream)",
        ))?;
        *y6 = terp1(self.xlast, ylast, a[ln - 1], a[ln - 1 + i6], x, int)?;
        let xnext = a[ln - 1];
        if ln + ncyc > self.nwtot && self.nb == 0 {
            return Ok((xnext, idis));
        }
        if a[ln + ncyc - 1] == xnext {
            idis = 1;
        }
        Ok((xnext, idis))
    }
}

/// `fixl7(itape, aa, maa, a, nb, nw)`: an MF=6 LAW=7 incident energy, summed
/// over cosine by the trapezoid rule onto a secondary-energy grid, as a LAW=1
/// LIST in `a`. Returns `(nb, nw)` = `(0, 6 + 2 ne)`.
///
/// # Errors
/// "not enough storage to convert file 7"; more than 200 grid energies
/// (upstream's `egrid(200)` would overflow); tape errors.
pub fn fixl7(
    s: &mut EndfStream,
    h: &mut Head,
    aa: &mut [f64],
    maa: usize,
    a: &mut [f64],
) -> Res<(usize, usize)> {
    const ETOP: f64 = 1.0e10;
    const SPAN: f64 = 1.0e6;
    s.tab2io(h, aa, 1)?;
    let ei = aa[1];
    let nmu = nint(aa[5]).max(0) as usize;
    let mut emin = ETOP;
    let mut emax = 0.0;
    let mut loc = 1 + nmu;
    for imu in 1..=nmu {
        aa[imu - 1] = loc as f64;
        let (mut nb, nw) = s.tab1io(h, aa, loc)?;
        let ir = nint(aa[loc + 3]).max(0) as usize;
        if aa[loc + 5 + 2 * ir] < emin {
            emin = aa[loc + 5 + 2 * ir];
        }
        loc += nw;
        while nb != 0 {
            if loc > maa {
                return Err(NjoyError::InvalidInput(
                    "fixl7: not enough storage to convert file 7".into(),
                ));
            }
            let (b, w) = s.moreio(aa, loc, nb)?;
            nb = b;
            loc += w;
        }
        if aa[loc - 3] > emax {
            emax = aa[loc - 3];
        }
    }
    if emin < emax / SPAN {
        emin = emax / SPAN;
    }
    let de = (emax - emin) / 150.0;
    let mut egrid = vec![emin];
    let mut e = emin;
    while e < emax {
        if e < emin + 10.0 * de {
            e += e / 4.0;
            if e > emin + 10.0 * de {
                e = emin + 10.0 * de;
            }
        } else {
            e += de;
        }
        egrid.push(e);
        if egrid.len() > 200 {
            return Err(NjoyError::NotPorted(
                "plotr fixl7: more than 200 grid energies (egrid(200) overflows upstream)",
            ));
        }
    }
    let ne = egrid.len();
    egrid[ne - 1] = emax;
    let loca = 6;
    for imu in 1..nmu {
        let loc1 = nint(aa[imu - 1]) as usize;
        let loc2 = nint(aa[imu]) as usize;
        let u1 = aa[loc1];
        let u2 = aa[loc2];
        let (mut ip, mut ir) = (2usize, 1usize);
        for (ie0, &e) in egrid.iter().enumerate() {
            let ie = ie0 + 1;
            let (f1, _, _) = crate::errorr::weight::terpa(&aa[loc1 - 1..], e, &mut ip, &mut ir)?;
            let (f2, _, _) = crate::errorr::weight::terpa(&aa[loc2 - 1..], e, &mut ip, &mut ir)?;
            if imu != 1 {
                a[loca + 2 * ie - 1] += (u2 - u1) * (f1 + f2) / 2.0;
            } else {
                a[loca - 2 + 2 * ie] = e;
                a[loca + 2 * ie - 1] = (u2 - u1) * (f1 + f2) / 2.0;
            }
        }
    }
    a[0] = 0.0;
    a[1] = ei;
    a[2] = 0.0;
    a[3] = 0.0;
    a[4] = (2 * ne) as f64;
    a[5] = ne as f64;
    Ok((0, 6 + 2 * ne))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::tape::Tape;

    #[test]
    fn ascale_picks_one_two_five_steps() {
        assert_eq!(ascale(4, 0.0, 4.9), (0.0, 5.0, 5, 25));
        let (z1, z2, major, _) = ascale(4, 0.0, 23.0);
        assert_eq!((z1, z2, major), (0.0, 25.0, 5));
        assert_eq!(ascale(4, 3.0, 1.0), (0.0, 2.0, 1, 0));
    }

    fn line(f: [&str; 6], mat: i32, mf: i32, mt: i32) -> String {
        format!(
            "{}{}{}{}{}{}{mat:4}{mf:2}{mt:3}    1\n",
            f[0], f[1], f[2], f[3], f[4], f[5]
        )
    }

    fn tape(body: &[String]) -> EndfStream {
        let mut t = String::from(
            " label                                                               1 0  0    0\n",
        );
        for l in body {
            t.push_str(l);
        }
        t.push_str(&line(["           "; 6], 1, 3, 0));
        t.push_str(&line(["           "; 6], 1, 0, 0));
        t.push_str(&line(["           "; 6], 0, 0, 0));
        t.push_str(&line(["           "; 6], -1, 0, 0));
        let mut s = EndfStream::from_tape(&Tape::read(t.as_bytes()).unwrap());
        let mut h = Head::default();
        s.tpidio(&mut h).unwrap();
        s
    }

    const B: &str = "           ";

    #[test]
    fn getz_and_gety1_walk_a_tab1() {
        let body = vec![
            line(
                [
                    " 0.000000+0",
                    " 0.000000+0",
                    "          0",
                    "          0",
                    "          1",
                    "          3",
                ],
                1,
                3,
                1,
            ),
            line(["          3", "          2", B, B, B, B], 1, 3, 1),
            line(
                [
                    " 1.000000+0",
                    " 2.000000+0",
                    " 2.000000+0",
                    " 4.000000+0",
                    " 3.000000+0",
                    " 5.000000+0",
                ],
                1,
                3,
                1,
            ),
        ];
        let mut h = Head::default();
        let mut a = vec![0.0; 100];
        let mut s = tape(&body);
        let mut z = Getz::default();
        z.clear();
        assert_eq!(z.init(30, &mut s, &mut h, &mut a, 1).unwrap(), 1.0);
        assert_eq!(z.get(1.5, 30).unwrap(), (3.0, 2.0, 0));
        assert_eq!(z.get(3.0, 30).unwrap(), (5.0, GETZ_BIG, 0));
        assert_eq!(z.get(1.0, 31).unwrap(), (0.0, GETZ_BIG, 0));
        let mut s = tape(&body);
        let mut g = Gety1Tape::default();
        assert!(Gety1Tape::default()
            .call(1.0, &mut s, &mut h, &mut a)
            .is_err());
        assert_eq!(g.call(0.0, &mut s, &mut h, &mut a).unwrap().1, 1.0);
        assert_eq!(a[5], 3.0);
        assert_eq!(g.call(2.5, &mut s, &mut h, &mut a).unwrap(), (4.5, 3.0, 0));
    }

    #[test]
    fn gety4_reads_one_legendre_coefficient() {
        let body = vec![
            line(
                [
                    " 0.000000+0",
                    " 0.000000+0",
                    "          0",
                    "          0",
                    "          0",
                    "          0",
                ],
                1,
                4,
                2,
            ),
            line(
                [
                    " 0.000000+0",
                    " 0.000000+0",
                    "          1",
                    "          0",
                    "          1",
                    "          3",
                ],
                1,
                4,
                2,
            ),
            line(["          3", "          2", B, B, B, B], 1, 4, 2),
            line(
                [
                    " 0.000000+0",
                    " 1.000000+0",
                    "          0",
                    "          0",
                    "          1",
                    "          0",
                ],
                1,
                4,
                2,
            ),
            line([" 1.000000-1", B, B, B, B, B], 1, 4, 2),
            line(
                [
                    " 0.000000+0",
                    " 3.000000+0",
                    "          0",
                    "          0",
                    "          1",
                    "          0",
                ],
                1,
                4,
                2,
            ),
            line([" 3.000000-1", B, B, B, B, B], 1, 4, 2),
            line(
                [
                    " 0.000000+0",
                    " 5.000000+0",
                    "          0",
                    "          0",
                    "          1",
                    "          0",
                ],
                1,
                4,
                2,
            ),
            line([" 5.000000-1", B, B, B, B, B], 1, 4, 2),
        ];
        let mut s = tape(&body);
        let mut h = Head::default();
        let mut a = vec![0.0; 100];
        a[3] = 1.0; // the HEAD's LTT, as plotr leaves it in `a`
        let mut g = Gety4::default();
        assert_eq!(g.call(0.0, 1, &mut s, &mut h, &mut a).unwrap(), (0.0, 1.0));
        let (y, xn) = g.call(2.0, 1, &mut s, &mut h, &mut a).unwrap();
        assert!((y - 0.2).abs() < 1e-15 && xn == 3.0);
        // At the last panel upstream returns the coefficient of the energy it
        // holds, E(np-1), and never reads E(np).
        assert_eq!(g.call(3.0, 1, &mut s, &mut h, &mut a).unwrap(), (0.3, BIG));
    }

    #[test]
    fn gety6_and_fixl7_follow_upstream() {
        // An MF=6 LAW=1 LIST: 3 secondary energies, na = 0 (ncyc = 2).
        let body = vec![
            line(
                [
                    " 0.000000+0",
                    " 2.000000+6",
                    "          0",
                    "          0",
                    "          6",
                    "          3",
                ],
                1,
                6,
                16,
            ),
            line(
                [
                    " 1.000000+0",
                    " 1.000000+0",
                    " 2.000000+0",
                    " 3.000000+0",
                    " 3.000000+0",
                    " 5.000000+0",
                ],
                1,
                6,
                16,
            ),
        ];
        let mut s = tape(&body);
        let mut h = Head::default();
        let mut a = vec![0.0; 100];
        let mut aa = vec![0.0; 10];
        let mut g = Gety6::default();
        let args = Gety6Args {
            i6: 1,
            lep: 2,
            law: 1,
            maa: 1,
        };
        let mut y = -7.0;
        assert_eq!(
            g.call(args, 0.0, &mut y, &mut s, &mut h, &mut a, &mut aa)
                .unwrap(),
            (1.0, 0)
        );
        assert_eq!(y, -7.0, "upstream leaves y6 unset on the first call");
        // 1.5 lies in the first panel: interpolating there reads the unsaved
        // `ylast`, which is refused.
        assert!(g
            .call(args, 1.5, &mut y, &mut s, &mut h, &mut a, &mut aa)
            .is_err());
        // `ip2 = nep` on a one-page LIST: past the second point the walk
        // interpolates in a stale, empty panel and returns xnext below x.
        let (xn, _) = g
            .call(args, 2.5, &mut y, &mut s, &mut h, &mut a, &mut aa)
            .unwrap();
        assert_eq!((y, xn), (3.0, 2.0));
        // LAW=7: two cosines, each a flat spectrum of 1 on [1, 10] eV.
        let tab = |mu: &str| {
            vec![
                line(
                    [
                        " 0.000000+0",
                        mu,
                        "          0",
                        "          0",
                        "          1",
                        "          2",
                    ],
                    1,
                    6,
                    16,
                ),
                line(["          2", "          2", B, B, B, B], 1, 6, 16),
                line(
                    [
                        " 1.000000+0",
                        " 1.000000+0",
                        " 1.000000+1",
                        " 1.000000+0",
                        B,
                        B,
                    ],
                    1,
                    6,
                    16,
                ),
            ]
        };
        let mut body = vec![
            line(
                [
                    " 0.000000+0",
                    " 5.000000+6",
                    "          0",
                    "          0",
                    "          1",
                    "          2",
                ],
                1,
                6,
                16,
            ),
            line(["          2", "          2", B, B, B, B], 1, 6, 16),
        ];
        body.extend(tab("-1.000000+0"));
        body.extend(tab(" 1.000000+0"));
        let mut s = tape(&body);
        let mut aa = vec![0.0; 200];
        let mut a = vec![0.0; 1000];
        let (nb, nw) = fixl7(&mut s, &mut h, &mut aa, 200, &mut a).unwrap();
        let ne = nint(a[5]) as usize;
        assert_eq!((nb, nw), (0, 6 + 2 * ne));
        assert_eq!(a[1], 5.0e6);
        assert_eq!((a[6], a[7]), (1.0, 2.0));
        assert_eq!(a[6 + 2 * ne - 2], 10.0);
    }
}
