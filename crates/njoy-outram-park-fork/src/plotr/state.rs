// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   the declarations of `subroutine plotr`, l.362-413, and the record calls it makes.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! [`Plotr`]: `subroutine plotr`'s locals and the `endf` module's variables,
//! the Fortran edit descriptors PLOTR writes with, and the record and getter
//! calls on a unit (see [`super::run`] for what upstream does with them).

use std::collections::BTreeMap;

use crate::deck::tapes::TapeUnits;
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::getters::{Gety1Tape, Gety4, Gety6, Gety6Args, Getz};
use super::run::{Res, MAXAA, MMAX, NWAMAX};
use super::stream::{EndfStream, Head};

pub(super) fn undefined(what: &'static str) -> NjoyError {
    NjoyError::NotPorted(what)
}

pub(super) fn perr(msg: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("plotr: {msg}"))
}

/// PLOTR's state: `subroutine plotr`'s locals and the `endf` module's.
#[derive(Debug, Clone)]
pub struct Plotr {
    pub(super) out: String,
    pub(super) a: Vec<f64>,
    pub(super) aa: Vec<f64>,
    pub(super) h: Head,
    pub(super) streams: BTreeMap<i32, EndfStream>,
    pub(super) g1: Gety1Tape,
    pub(super) gz: Getz,
    pub(super) g4: Gety4,
    pub(super) g6: Gety6,
    /// The `endf` module's `iverf` (`rname` reads it).
    pub(super) iverf: i32,
    pub(super) ninl: i32,
    pub(super) i3d: i32,
    pub(super) iplot: i32,
    pub(super) iwcol: i32,
    pub(super) ierrb: i32,
    pub(super) itype: i32,
    pub(super) jtype: i32,
    pub(super) igrid: i32,
    pub(super) ileg: i32,
    pub(super) nx: usize,
    pub(super) ny: usize,
    pub(super) nr: Option<i32>,
    pub(super) matd: i32,
    pub(super) mfd: i32,
    pub(super) mtd: i32,
    pub(super) nth: i32,
    pub(super) ntp: i32,
    pub(super) nkh: i32,
    pub(super) iauto: Option<i32>,
    pub(super) ipass: i32,
    pub(super) c9: [i32; 6],
    pub(super) ik: i32,
    pub(super) mmf: i32,
    pub(super) lf: Option<i32>,
    pub(super) lep: Option<i32>,
    pub(super) idis: i32,
    pub(super) jnoth: Option<i32>,
    pub(super) n: Option<usize>,
    pub(super) nin: i32,
    pub(super) nin2: i32,
    pub(super) matd2: i32,
    pub(super) mfd2: i32,
    pub(super) mtd2: i32,
    pub(super) temper: f64,
    pub(super) temper2: f64,
    pub(super) iverf2: i32,
    pub(super) loc2: Option<usize>,
    pub(super) l2: Option<usize>,
    pub(super) ltt: i32,
    pub(super) iyauto: i32,
    pub(super) factx: f64,
    pub(super) facty: f64,
    pub(super) xll: f64,
    pub(super) yll: f64,
    pub(super) ww: f64,
    pub(super) wh: f64,
    pub(super) wr: f64,
    pub(super) xtag: f64,
    pub(super) ytag: f64,
    pub(super) xpoint: f64,
    pub(super) xstep: f64,
    pub(super) xleft: f64,
    pub(super) xright: f64,
    pub(super) yb: f64,
    pub(super) yt: f64,
    pub(super) yystep: f64,
    pub(super) ybot: f64,
    pub(super) ytop: f64,
    pub(super) ystep: f64,
    pub(super) r: Option<[f64; 3]>,
    pub(super) view: [f64; 6],
    pub(super) elt: f64,
    pub(super) eht: f64,
    pub(super) enow: f64,
    pub(super) enext: f64,
    pub(super) yf: f64,
    pub(super) ee1: f64,
    pub(super) ee2: f64,
    pub(super) t1: String,
    pub(super) t2: String,
    pub(super) xl: String,
    pub(super) yl: String,
    pub(super) rl: Option<String>,
    pub(super) aleg: String,
    pub(super) nnn: i32,
}

impl Default for Plotr {
    fn default() -> Self {
        let blank = " ".repeat(60);
        Self {
            out: String::new(),
            a: vec![0.0; NWAMAX],
            aa: vec![0.0; MAXAA],
            h: Head::default(),
            streams: BTreeMap::new(),
            g1: Gety1Tape::default(),
            gz: Getz::default(),
            g4: Gety4::default(),
            g6: Gety6::default(),
            iverf: 0,
            ninl: 0,
            i3d: 0,
            iplot: 1,
            iwcol: 0,
            ierrb: 0,
            itype: 4,
            jtype: 0,
            igrid: 2,
            ileg: 0,
            nx: 0,
            ny: 0,
            nr: None,
            matd: 0,
            mfd: 0,
            mtd: 0,
            nth: 1,
            ntp: 1,
            nkh: 1,
            iauto: None,
            ipass: 0,
            c9: [0, 0, 0, 0, 1, 0],
            ik: 1,
            mmf: 3,
            lf: None,
            lep: None,
            idis: 0,
            jnoth: None,
            n: None,
            nin: 0,
            nin2: 0,
            matd2: 0,
            mfd2: 0,
            mtd2: 0,
            temper: 0.0,
            temper2: 0.0,
            iverf2: 0,
            loc2: None,
            l2: None,
            ltt: 0,
            iyauto: 0,
            factx: 1.0,
            facty: 1.0,
            xll: 0.0,
            yll: 0.0,
            ww: 0.0,
            wh: 0.0,
            wr: 0.0,
            xtag: 0.0,
            ytag: 0.0,
            xpoint: 0.0,
            xstep: 0.0,
            xleft: 0.0,
            xright: 0.0,
            yb: 0.0,
            yt: 0.0,
            yystep: 0.0,
            ybot: 0.0,
            ytop: 0.0,
            ystep: 0.0,
            r: None,
            view: super::cards::DVIEW,
            elt: 0.0,
            eht: 0.0,
            enow: 0.0,
            enext: 0.0,
            yf: 0.0,
            ee1: 0.0,
            ee2: 0.0,
            t1: blank.clone(),
            t2: blank.clone(),
            xl: blank.clone(),
            yl: blank.clone(),
            rl: None,
            aleg: blank,
            nnn: 0,
        }
    }
}

/// Fortran `a` edit of a `character(60)` variable.
pub(super) fn c60(s: &str) -> String {
    let mut v: String = s.chars().take(60).collect();
    while v.chars().count() < 60 {
        v.push(' ');
    }
    v
}

/// `1pEw.d`, with gfortran's sign on a negative zero.
pub(super) fn fe(x: f64, d: usize, w: usize) -> String {
    if x == 0.0 && x.is_sign_negative() {
        return format!("{:>w$}", format!("-0.{}E+00", "0".repeat(d)));
    }
    crate::acer::fortran_fmt::fortran_e(x, d, w)
}

/// `Iw`.
pub(super) fn fi(n: i32, w: usize) -> String {
    crate::viewr::fmt::i(i64::from(n), w)
}

/// `Fw.d`.
pub(super) fn ff(x: f64, w: usize, d: usize) -> String {
    crate::viewr::fmt::f(x, w, d)
}

impl Plotr {
    pub(super) fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    pub(super) fn x(&self, i: usize) -> f64 {
        self.aa[i - 1]
    }

    pub(super) fn set_x(&mut self, i: usize, v: f64) {
        self.aa[i - 1] = v;
    }

    pub(super) fn y(&self, i: usize) -> f64 {
        self.aa[MMAX + i - 1]
    }

    pub(super) fn set_y(&mut self, i: usize, v: f64) {
        self.aa[MMAX + i - 1] = v;
    }

    /// `x`, `y`, `dym`, `dyp`, `dxm`, `dxp` (equivalenced into `aa`).
    pub(super) fn set_eq(&mut self, k: usize, i: usize, v: f64) {
        self.aa[k * MMAX + i - 1] = v;
    }

    pub(super) fn eq(&self, k: usize, i: usize) -> f64 {
        self.aa[k * MMAX + i - 1]
    }

    pub(super) fn stream(&mut self, unit: i32) -> Res<&mut EndfStream> {
        self.streams
            .get_mut(&unit.abs())
            .ok_or_else(|| perr(&format!("unit {unit} is not open")))
    }

    /// The tape on `unit`, its stream, the module variables and both work arrays.
    pub(super) fn parts(
        &mut self,
        unit: i32,
    ) -> Res<(&mut EndfStream, &mut Head, &mut Vec<f64>, &mut Vec<f64>)> {
        let s = self
            .streams
            .get_mut(&unit.abs())
            .ok_or_else(|| perr(&format!("unit {unit} is not open")))?;
        Ok((s, &mut self.h, &mut self.a, &mut self.aa))
    }

    pub(super) fn contio(&mut self, unit: i32, at: usize) -> Res<()> {
        let (s, h, a, _) = self.parts(unit)?;
        s.contio(h, a, at)
    }

    pub(super) fn listio(&mut self, unit: i32, at: usize) -> Res<(usize, usize)> {
        let (s, h, a, _) = self.parts(unit)?;
        s.listio(h, a, at)
    }

    pub(super) fn tab1io(&mut self, unit: i32, at: usize) -> Res<(usize, usize)> {
        let (s, h, a, _) = self.parts(unit)?;
        s.tab1io(h, a, at)
    }

    pub(super) fn tab2io(&mut self, unit: i32, at: usize) -> Res<usize> {
        let (s, h, a, _) = self.parts(unit)?;
        s.tab2io(h, a, at)
    }

    pub(super) fn moreio(&mut self, unit: i32, at: usize, nb: usize) -> Res<(usize, usize)> {
        let (s, _, a, _) = self.parts(unit)?;
        s.moreio(a, at, nb)
    }

    /// A record's remaining pages read over one another at `a(at)`
    /// (`do while (nb.ne.0); call moreio(nin,0,0,a(at),nb,nw); enddo`).
    pub(super) fn skip_pages(&mut self, unit: i32, at: usize, mut nb: usize) -> Res<()> {
        while nb != 0 {
            nb = self.moreio(unit, at, nb)?.0;
        }
        Ok(())
    }

    pub(super) fn gety1(&mut self, x: f64) -> Res<(f64, f64, i32)> {
        let nin = self.nin;
        let s = self
            .streams
            .get_mut(&nin.abs())
            .ok_or_else(|| perr("gety1 on a unit that is not open"))?;
        self.g1.call(x, s, &mut self.h, &mut self.a)
    }

    pub(super) fn gety4(&mut self, x: f64) -> Res<(f64, f64)> {
        let (nin, nth) = (self.nin, self.nth);
        let s = self
            .streams
            .get_mut(&nin.abs())
            .ok_or_else(|| perr("gety4 on a unit that is not open"))?;
        self.g4.call(x, nth, s, &mut self.h, &mut self.a)
    }

    /// `gety6(i6, x, ..., a, lep, law, aa(off..), maa)` on `nin`, `y6 = yf`.
    pub(super) fn gety6(
        &mut self,
        i6: usize,
        x: f64,
        lep: i32,
        law: i32,
        off: usize,
        maa: usize,
    ) -> Res<(f64, i32)> {
        let nin = self.nin;
        let s = self
            .streams
            .get_mut(&nin.abs())
            .ok_or_else(|| perr("gety6 on a unit that is not open"))?;
        let args = Gety6Args { i6, lep, law, maa };
        let mut y = self.yf;
        let r = self.g6.call(
            args,
            x,
            &mut y,
            s,
            &mut self.h,
            &mut self.a,
            &mut self.aa[off..],
        )?;
        self.yf = y;
        Ok(r)
    }

    pub(super) fn getz_init(&mut self, unit: i32, at: usize) -> Res<f64> {
        let s = self
            .streams
            .get_mut(&unit.abs())
            .ok_or_else(|| perr("getz on a unit that is not open"))?;
        self.gz.init(unit, s, &mut self.h, &mut self.a, at)
    }

    pub(super) fn open_in(&mut self, units: &mut TapeUnits, unit: i32) -> Res<()> {
        if unit == 0 {
            return Ok(());
        }
        // A blocked-binary GENDF is read at the precision GROUPR wrote it
        // (`deck::covr::tape_from_records`), a coded one as its text.
        let tape = if let Some(g) = units.binary_gendf(unit) {
            crate::deck::covr::tape_from_records(&g)
        } else if let Some(t) = units.text(unit) {
            Tape::read(t.as_bytes())?
        } else {
            units.openz(unit, 0)?;
            units.get(unit)?.tape.clone()
        };
        self.streams
            .insert(unit.abs(), EndfStream::from_tape(&tape));
        Ok(())
    }

    pub(super) fn mfd_name_t2(&mut self) -> Res<()> {
        let name = super::rname::rname(self.mtd, self.iverf)?;
        self.t2 = c60(&format!(
            "mf={}  mt={}  {name}",
            fi(self.mfd, 2),
            fi(self.mtd, 3)
        ));
        Ok(())
    }

    /// The `if (iauto.gt.0) call rname ... write(t2,...)` of every plot writer.
    pub(super) fn auto_title(&mut self) -> Res<()> {
        if self.iauto.ok_or(undefined(
            "plotr: iauto read before any card 8 set it (undefined upstream)",
        ))? > 0
        {
            self.mfd_name_t2()?;
        }
        Ok(())
    }
}
