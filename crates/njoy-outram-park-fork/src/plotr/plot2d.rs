// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine plotr`, l.1688-1786 (input data, and the 2-d plot commands).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Input data points (label 500) and the 2-d plot writer (label 610).

use super::run::{c60, fe, ff, fi, undefined, Go, Plotr, Res, MMAX, XLABLD, YLABLD};
use super::run::perr;

impl Plotr {
    /// 500 to 1727: input data points.
    pub(super) fn at_500(&mut self, pts: &[[f64; 6]]) -> Res<Go> {
        self.ierrb = 0;
        if pts.len() > MMAX {
            return Err(perr("more input data points than mmax"));
        }
        for (k, z) in pts.iter().enumerate() {
            let i = k + 1;
            self.set_eq(0, i, z[0] * self.factx);
            self.set_eq(1, i, z[1] * self.facty);
            self.set_eq(5, i, z[2] * self.facty);
            self.set_eq(6, i, z[3] * self.facty);
            if self.eq(6, i) == 0.0 {
                self.set_eq(6, i, self.eq(5, i));
            }
            self.set_eq(3, i, z[4] * self.factx);
            self.set_eq(4, i, z[5] * self.factx);
            if self.eq(4, i) == 0.0 {
                self.set_eq(4, i, self.eq(3, i));
            }
            if self.eq(5, i) != 0.0 {
                self.ierrb = 1;
            }
            if self.eq(3, i) != 0.0 {
                self.ierrb = 2;
            }
            if self.eq(5, i) != 0.0 && self.eq(3, i) != 0.0 {
                self.ierrb = 3;
            }
        }
        self.n = Some(pts.len());
        self.at_610()
    }

    /// 610 to 1786: write the 2-d plot.
    pub(super) fn at_610(&mut self) -> Res<Go> {
        let n = self.n.ok_or(undefined(
            "plotr: a GENDF curve with no non-zero value and no earlier curve (n undefined)",
        ))?;
        let mut l = format!("{}{}", fi(self.iplot, 4), fi(self.iwcol, 8));
        for v in [1.0, 1.0, self.xll, self.yll, self.ww, self.wh, self.wr] {
            l.push_str(&ff(v, 7, 2));
        }
        l.push_str("/ 2d plot");
        self.line(&l);
        if self.iplot.abs() <= 1 {
            if self.nx == 0 {
                self.xl = c60(XLABLD);
            }
            if self.ny == 0 {
                self.yl = c60(YLABLD);
            }
            let t = format!(" '{}'/", self.t1);
            self.line(&t);
            self.auto_title()?;
            let t = format!(" '{}'/", self.t2);
            self.line(&t);
            let t = format!(
                "{}{}{}{}{}{}/",
                fi(self.itype, 6),
                fi(self.jtype, 6),
                fi(self.igrid, 6),
                fi(self.ileg, 6),
                fe(self.xtag, 4, 13),
                fe(self.ytag, 4, 13)
            );
            self.line(&t);
            self.axis_line(self.xstep, [self.xleft, self.xright, self.xstep]);
            let t = format!(" '{}'/", self.xl);
            self.line(&t);
            self.axis_line(self.ystep, [self.ybot, self.ytop, self.ystep]);
            let t = format!(" '{}'/", self.yl);
            self.line(&t);
            if self.jtype > 0 {
                let r = self
                    .r
                    .ok_or(undefined("plotr: card 7 never read (undefined upstream)"))?;
                self.axis_line(r[2], r);
                let t = format!(" '{}'/", self.rl.clone().unwrap_or_default());
                self.line(&t);
            }
        }
        self.line("/");
        let c = self.c9;
        let t = format!(
            "{}{}{}{}{}{}/",
            fi(c[0], 6),
            fi(c[1], 6),
            fi(c[2], 6),
            fi(c[3], 6),
            fi(c[4], 6),
            fi(c[5], 8)
        );
        self.line(&t);
        if self.ileg != 0 {
            let t = format!(" '{}'/", self.aleg);
            self.line(&t);
        }
        if self.ileg == 2 {
            let t = format!(
                "{}{}{}/",
                fe(self.xtag, 4, 13),
                fe(self.ytag, 4, 13),
                fe(self.xpoint, 4, 13)
            );
            self.line(&t);
        }
        self.line(" 0/");
        for i in 1..=n {
            let t = if self.ierrb == 0 {
                format!("{}{}/", fe(self.x(i), 8, 16), fe(self.y(i), 8, 16))
            } else {
                let mut t = String::new();
                for k in [0, 1, 5, 6, 3, 4] {
                    t.push_str(&fe(self.eq(k, i), 8, 16));
                }
                t.push('/');
                t
            };
            self.line(&t);
        }
        self.line(" /");
        let iauto = self.iauto.ok_or(undefined(
            "plotr: iauto read before any card 8 set it (undefined upstream)",
        ))?;
        match self.jnoth {
            Some(0) => self.ipass = 0,
            Some(1) if self.ipass == 5 => self.ipass = 0,
            None if iauto > 0 => return Err(undefined("plotr: jnoth undefined")),
            _ => {}
        }
        if iauto > 0 && self.ipass > 0 {
            return Ok(Go::L320);
        }
        if iauto > 0 {
            return Ok(Go::L321);
        }
        Ok(Go::L110)
    }

    /// `if (step.eq.zero) write '/' else write (1p,3e13.4,'/')`.
    pub(super) fn axis_line(&mut self, step: f64, v: [f64; 3]) {
        if step == 0.0 {
            self.line("/");
        } else {
            let t = format!("{}{}{}/", fe(v[0], 4, 13), fe(v[1], 4, 13), fe(v[2], 4, 13));
            self.line(&t);
        }
    }
}
