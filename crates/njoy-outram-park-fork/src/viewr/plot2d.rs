// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/viewr.f90` (`set2d`, `erbar`, `legndb`,
// `tagit`, `ascalv`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Two-dimensional multi-curve plots: axes, curves, error bars, legend blocks
//! and curve tags.

use petir::mathf::RealMath;

use super::state::ViewrState;
use crate::endf::interp::{terp1, IntLaw};
use crate::NjoyError;

/// One 2-d curve as VIEWR reads it (`x`, `y`, and the error bars).
#[derive(Debug, Clone, Default)]
pub struct Curve2d {
    /// Abscissae (times `factx`).
    pub x: Vec<f64>,
    /// Ordinates (times `facty`).
    pub y: Vec<f64>,
    /// x error bar, left.
    pub dxm: Vec<f64>,
    /// x error bar, right.
    pub dxp: Vec<f64>,
    /// y error bar, below.
    pub dym: Vec<f64>,
    /// y error bar, above.
    pub dyp: Vec<f64>,
}

/// `ascalv(m,z1,z2,major,minor)` (`viewr.f90:1526`): automatic scaling of a
/// linear axis (from the Los Alamos SC4020 library). Returns the new limits and
/// the numbers of major and minor divisions: `(z1, z2, major, minor)`.
pub fn ascalv(m: i32, z1: f64, z2: f64) -> (f64, f64, i32, i32) {
    let one = 1.0_f64;
    let ten = 10.0_f64;
    let tentho = 10000.0_f64;
    let mut zmin = z1;
    let mut zmax = z2;
    if zmax <= zmin || m <= 0 || m > 20 {
        return (0.0, 2.0 * z2, 1, 0);
    }
    let fm = f64::from(m);
    if zmax == 0.0 || zmin == 0.0 {
        zmax -= zmax.abs() / 1000000.0;
        zmin += zmin.abs() / 1000000.0;
    } else {
        let mut zbar = zmax / zmin;
        if zbar.abs() / 1000.0 >= one {
            zmin = 0.0;
        }
        if zbar.abs() * 1000.0 <= one {
            zmax = 0.0;
            zmax -= zmax.abs() / 1000000.0;
            zmin += zmin.abs() / 1000000.0;
        } else if (zbar - 1.0).abs() - 5.0 * fm / 100000.0 > 0.0 {
            zmax -= zmax.abs() / 1000000.0;
            zmin += zmin.abs() / 1000000.0;
        } else {
            zbar = (zmax + zmin) / 2.0;
            let z = 26.0 * fm * zbar.abs() / 1000000.0;
            zmax = zbar + z;
            zmin = zbar - z;
        }
    }
    let mut p = (zmax - zmin) / fm;
    let mut iflag = 0;
    let mut tenk = 1.0_f64;
    if p < one {
        iflag = 1;
        p = 1.0 / p;
    }
    while p >= tentho {
        p /= 10000.0;
        tenk *= 10000.0;
    }
    while p >= ten {
        p /= 10.0;
        tenk *= 10.0;
    }
    if iflag != 0 {
        p = 10.0 / p;
        tenk = one / 10.0 / tenk;
    }
    let nm;
    let mut test = 2.0 + one / 100.0;
    if p <= test {
        test = 2.0 - one / 100.0;
        if p <= test {
            p = 1.0;
            nm = 5;
        } else {
            p = 2.0;
            nm = 4;
        }
    } else {
        test = 5.0 - one / 100.0;
        if p < test {
            p = 2.0;
            nm = 4;
        } else {
            p = 5.0;
            nm = 5;
        }
    }
    let dz = p * tenk;
    let mut n1 = (zmin / dz) as i32;
    let mut fnn = f64::from(n1);
    let mut z = fnn * dz;
    if z > zmin {
        z -= dz;
        n1 -= 1;
    }
    let lo = z;
    let mut n2 = (zmax / dz) as i32;
    fnn = f64::from(n2);
    z = fnn * dz;
    if z < zmax {
        n2 += 1;
        z += dz;
    }
    let hi = z;
    let major = n2 - n1;
    (lo, hi, major, nm * major)
}

impl ViewrState {
    /// `set2d(iplot,x,y,dxm,dxp,dym,dyp,n,ierrb)` (`viewr.f90:757`): one curve of
    /// a 2-d plot. `iplot = 1` starts a page, `+-1` new axes, larger values add
    /// to the axes on the page, `< -1` starts a set on the alternate y axis.
    ///
    /// # Errors
    /// A non-positive log-interpolation argument in the curve tag (`terp1`).
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // upstream's routine
    pub fn set2d(&mut self, iplot: i32, c: &Curve2d, ierrb: i32) -> Result<(), NjoyError> {
        let n = c.x.len();
        let shade = 0.99_f64;
        let bard = 0.03_f64;
        let small = 1.0e-15_f64;
        let twenty = 20e6_f64;
        let grace = 0.0_f64;
        let ten = 10.0_f64;
        let wt = self.wline;

        if iplot == 1 {
            if self.csize > 0.0 {
                self.hlab = self.csize;
            }
            if self.csize < 0.0 {
                self.hlab = -self.csize * self.wh;
            }
            self.hleg = 2.0 * self.hlab / 3.0;
            self.g.initp(self.lori, self.xpage, self.ypage, self.istyle, self.hlab, self.wline, 0, self.ipcol);
        }

        if iplot.abs() == 1 {
            self.g.window(self.xll, self.yll, self.ww, self.wh, self.wr, &self.t1, self.n1, &self.t2, self.n2, 0);
            if self.itype == 0 {
                return Ok(());
            }
            if n == 0 {
                return Err(NjoyError::InvalidInput("viewr: a curve with no data points".into()));
            }
            let (_xpos, _ypos, xg, yg) = self.g.init2(self.jtype, self.iwcol);
            self.xg = xg;
            self.yg = yg;
        }

        if iplot <= 1 {
            if iplot < -1 {
                self.ymin = self.zmin;
                self.ymax = self.zmax;
                self.ystp = self.zstp;
            }
            let mut xstep = self.xstp;
            let mut ystep = self.ystp;
            if self.xstp == 0.0 || self.ystp == 0.0 {
                if self.xstp == 0.0 {
                    self.xmin = c.x[0];
                    self.xmax = c.x[0];
                }
                if self.ystp == 0.0 || iplot >= -1 {
                    if self.ystp == 0.0 {
                        self.ymax = c.y[0];
                        self.ymin = c.y[0];
                    }
                    for i in 1..n {
                        if self.xstp == 0.0 && iplot >= -1 {
                            if c.x[i] < self.xmin {
                                self.xmin = c.x[i];
                            }
                            if c.x[i] > self.xmax {
                                self.xmax = c.x[i];
                            }
                        }
                        if self.ystp == 0.0 {
                            if c.y[i] < self.ymin && c.y[i] != small {
                                self.ymin = c.y[i];
                            }
                            if c.y[i] > self.ymax {
                                self.ymax = c.y[i];
                            }
                        }
                    }
                    if self.itype <= 2 && iplot >= -1 && self.xstp == 0.0 {
                        let (lo, hi, major, _minor) = ascalv(4, self.xmin, self.xmax);
                        self.xmin = lo;
                        self.xmax = hi;
                        xstep = (self.xmax - self.xmin) / f64::from(major);
                    }
                    if (iplot < -1 || self.itype == 1 || self.itype == 3) && (iplot >= -1 || self.jtype == 1) && self.ystp == 0.0 {
                        let (lo, hi, major, _minor) = ascalv(4, self.ymin, self.ymax);
                        self.ymin = lo;
                        self.ymax = hi;
                        ystep = (self.ymax - self.ymin) / f64::from(major);
                    }
                }
            }

            // log scales: round to decades, limit the number of decades.
            if iplot >= -1 && self.itype >= 3 && self.xstp == 0.0 {
                let top = self.xmax.r_log10();
                if self.xmax != twenty {
                    if top >= 0.0 {
                        self.xmax = ten.r_powi((top + shade) as i32);
                    }
                    if top < 0.0 {
                        self.xmax = ten.r_powi(top as i32);
                    }
                }
                let xlft = self.xmin.r_log10();
                let ixlft = xlft as i32;
                if xlft > 0.0 {
                    self.xmin = ten.r_powi(ixlft);
                }
                if xlft < 0.0 {
                    self.xmin = ten.r_powi(ixlft - 1);
                }
                if self.xmax / self.xmin > ten.r_powi(13) {
                    self.xmin = ten.r_powi((top - 13.0) as i32);
                }
            }
            if (iplot < -1 || self.itype == 2 || self.itype == 4) && (iplot >= -1 || self.jtype == 2) && self.ystp == 0.0 {
                let top = self.ymax.r_log10();
                if top >= 0.0 {
                    self.ymax = ten.r_powi((top + shade) as i32);
                }
                if top < 0.0 {
                    self.ymax = ten.r_powi(top as i32);
                }
                let bot = self.ymin.r_log10();
                let ibot = bot as i32;
                if bot > 0.0 {
                    self.ymin = ten.r_powi(ibot);
                }
                if bot < 0.0 {
                    self.ymin = ten.r_powi(ibot - 1);
                }
                if self.ymax / self.ymin > ten.r_powi(8) {
                    self.ymin = ten.r_powi((top - 8.0) as i32);
                }
            }

            // axes and grid
            if iplot >= -1 {
                self.itic = 0;
                if self.igrid == 2 {
                    self.itic = -1;
                }
                if self.igrid == 3 {
                    self.itic = 1;
                }
                let itic = self.itic;
                let mut xop = xstep;
                let mut yop = ystep;
                if self.itype == 3 || self.itype == 4 {
                    xop = 0.0;
                }
                if self.itype == 2 || self.itype == 4 {
                    yop = 0.0;
                }
                let (xg, yg) = (self.xg, self.yg);
                let (xmin, xmax, ymin, ymax) = (self.xmin, self.xmax, self.ymin, self.ymax);
                let (nx, ny) = (self.nx, self.ny);
                self.g.axis2(xmin, xmax, xop, &self.xl, -nx, itic, 0, xg, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0);
                self.g.axis2(xmin, xmax, xop, b" ", 0, -itic, 0, xg, 0.0, yg, 1.0, 0.0, 0.0, 1.0, 0);
                self.g.axis2(ymin, ymax, yop, &self.yl, ny, -itic, 0, yg, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 1);
                self.g.axis2(ymin, ymax, yop, b" ", 0, itic, 0, yg, xg, 0.0, 0.0, 1.0, -1.0, 0.0, 1);
                if self.igrid == 1 {
                    self.g.grid2(1, 1);
                }
            } else {
                let rmin = self.ymin;
                let rmax = self.ymax;
                let rstep = ystep;
                let mut rop = rstep;
                if self.jtype == 2 {
                    rop = 0.0;
                }
                let (xg, yg, itic, nr) = (self.xg, self.yg, self.itic, self.nr);
                self.g.axis2(rmin, rmax, rop, &self.rl, -nr, -itic, 0, yg, xg, 0.0, 0.0, 1.0, -1.0, 0.0, 1);
                self.g.axis2(rmin, rmax, rop, b" ", 0, itic, 0, yg, 0.0, 0.0, 0.0, 1.0, -1.0, 0.0, 1);
            }

            // legend block position
            if self.ileg == 1 {
                if self.xtag == 0.0 && self.ytag == 0.0 {
                    self.xpt = self.g.xscale(self.xmin) + self.hleg;
                    self.ypt = self.g.yscale(self.ymax) - 3.0 * self.hleg / 2.0;
                } else {
                    self.xpt = self.g.xscale(self.xtag);
                    self.ypt = self.g.yscale(self.ytag);
                }
            }
        }

        // the curves, inside a clipping path just outside the frame
        self.g.frame2(self.xg, self.yg, grace, self.iccol);
        let bar = bard;
        if ierrb > 0 {
            self.erbar(c, ierrb, bar);
        }
        let w = f64::from(self.ithick) * wt;
        let ssym = 3.0 * self.hleg / 10.0;
        self.g.curv2(&c.x, &c.y, n, self.idash, w, self.icon, self.isym, ssym, self.ishade);
        if self.ileg == 1 {
            self.legndb(iplot);
        }
        if self.ileg == 2 {
            self.tagit(c, self.itype)?;
        }
        self.g.endfr();
        Ok(())
    }

    /// `erbar(xx,yy,n,dxm,dxp,dym,dyp,ixy,w)` (`viewr.f90:1042`): error bars on
    /// the curve just drawn (`ixy` 1 y, 2 x, 3 both; `w` the cap size, 0.02 when
    /// zero).
    fn erbar(&mut self, c: &Curve2d, ixy: i32, w: f64) {
        let capd = 0.02_f64;
        let wt = self.wline;
        let mut wd = w;
        if wd == 0.0 {
            wd = capd;
        }
        for np in 0..c.x.len() {
            let xn = self.g.xscale(c.x[np]);
            let yn = self.g.yscale(c.y[np]);
            if ixy > 1 && (c.dxm[np] != 0.0 || c.dxp[np] != 0.0) {
                let x1 = self.g.xscale(c.x[np] - c.dxm[np]);
                let x2 = self.g.xscale(c.x[np] + c.dxp[np]);
                self.g.vect3(x1, yn, 0.0, x2, yn, 0.0, wt, 0);
                self.g.vect3(x1, yn + wd, 0.0, x1, yn - wd, 0.0, wt, 0);
                self.g.vect3(x2, yn + wd, 0.0, x2, yn - wd, 0.0, wt, 0);
            }
            if ixy != 0
                && ixy != 2
                && c.x[np] >= self.xmin
                && c.x[np] <= self.xmax
                && c.y[np] >= self.ymin
                && c.y[np] <= self.ymax
                && (c.dym[np] != 0.0 || c.dyp[np] != 0.0)
            {
                let y1 = self.g.yscale(c.y[np] - c.dym[np]);
                let y2 = self.g.yscale(c.y[np] + c.dyp[np]);
                self.g.vect3(xn, y1, 0.0, xn, y2, 0.0, wt, 0);
                self.g.vect3(xn - wd, y1, 0.0, xn + wd, y1, 0.0, wt, 0);
                self.g.vect3(xn - wd, y2, 0.0, xn + wd, y2, 0.0, wt, 0);
            }
        }
    }

    /// `legndb(iplot)` (`viewr.f90:1105`): one legend line.
    fn legndb(&mut self, iplot: i32) {
        let wd = 0.005_f64;
        let backgr = 1.0_f64;
        let wt = self.wline;
        let hleg = self.hleg;
        let istyle = self.istyle;
        let aleg = self.aleg.clone();
        if iplot.abs() == 1 {
            self.leg_x = self.xpt;
            self.leg_y = self.ypt;
            let (x, y) = (self.leg_x, self.leg_y);
            let (xta, yta) = self.g.trans3(x, y, 0.0);
            let wleg = self.g.txtlen(&aleg, self.nleg, istyle, hleg);
            let xb = [
                xta - 2.0 * hleg / 10.0,
                xta + wleg + 7.0 * 1.0 / 10.0 + 4.0 * hleg / 10.0,
                xta + wleg + 7.0 * 1.0 / 10.0 + 4.0 * hleg / 10.0,
                xta - 2.0 * hleg / 10.0,
                xta - 2.0 * hleg / 10.0,
            ];
            let yb = [yta - 3.0 * hleg / 10.0, yta - 3.0 * hleg / 10.0, yta, yta, yta - 3.0 * hleg / 10.0];
            self.g.poly2(&xb, &yb, 5, -wd, backgr);
        }
        let (x, y) = (self.leg_x, self.leg_y);
        let xc = [self.g.xinvrs(x), self.g.xinvrs(x + 5.0 * 1.0 / 10.0)];
        let yc = [self.g.yinvrs(y - 7.0 * hleg / 10.0), self.g.yinvrs(y - 7.0 * hleg / 10.0)];
        let (xta, yta) = self.g.trans3(x, y, 0.0);
        let wleg = self.g.txtlen(&aleg, self.nleg, istyle, hleg);
        let xb = [
            xta - 2.0 * hleg / 10.0,
            xta + wleg + 7.0 * 1.0 / 10.0 + 4.0 * hleg / 10.0,
            xta + wleg + 7.0 * 1.0 / 10.0 + 4.0 * hleg / 10.0,
            xta - 2.0 * hleg / 10.0,
            xta - 2.0 * hleg / 10.0,
        ];
        let yb = [
            yta - 14.0 * hleg / 10.0,
            yta - 14.0 * hleg / 10.0,
            yta - 3.0 * hleg / 10.0,
            yta - 3.0 * hleg / 10.0,
            yta - 14.0 * hleg / 10.0,
        ];
        self.g.poly2(&xb, &yb, 5, -wd, backgr);
        let w = f64::from(self.ithick) * wt;
        if self.icon >= 0 {
            self.g.curv2(&xc, &yc, 2, self.idash, w, 0, 0, 0.0, 0);
        }
        let ssym = 3.0 * hleg / 10.0;
        let (u, v) = self.g.trans3(x + 1.0 / 4.0, y - 7.0 * hleg / 10.0, 0.0);
        if self.icon != 0 {
            self.g.dsym(u, v, self.isym, ssym);
        }
        self.g.text3(&aleg, self.nleg, istyle, hleg, x + 7.0 * 1.0 / 10.0, y - 11.0 * hleg / 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        self.leg_y = y - 11.0 * hleg / 10.0;
    }

    /// `tagit(x,y,n,it)` (`viewr.f90:1179`): the tag string at `(xtag, ytag)`
    /// and an arrow to the curve at `xpoint`.
    fn tagit(&mut self, c: &Curve2d, it: i32) -> Result<(), NjoyError> {
        let wd = 0.005_f64;
        let ivec = 1021;
        let wt = self.wline;
        let backgr = 0.0_f64;
        let n = c.x.len();
        let hleg = self.hleg;
        let istyle = self.istyle;
        let mut ypoint = 0.0;
        if self.xpoint > 0.0 {
            let mut j = 1usize;
            while c.x[j - 1] < self.xpoint && j < n {
                j += 1;
            }
            if j == 1 {
                j = 2;
            }
            let jt = (it + 1) as u32;
            ypoint = terp1(c.x[j - 2], c.y[j - 2], c.x[j - 1], c.y[j - 1], self.xpoint, IntLaw::from_code(jt))?;
        }
        let xtp = self.g.xscale(self.xtag);
        let ytp = self.g.yscale(self.ytag);
        let (xta, yta) = self.g.trans3(xtp, ytp, 0.0);
        let aleg = self.aleg.clone();
        let wleg = self.g.txtlen(&aleg, self.nleg, istyle, hleg);
        let xb = [
            xta - 2.0 * hleg / 10.0,
            xta + wleg + 4.0 * hleg / 10.0,
            xta + wleg + 4.0 * hleg / 10.0,
            xta - 2.0 * hleg / 10.0,
            xta - 2.0 * hleg / 10.0,
        ];
        let yb = [
            yta - 2.0 * hleg / 10.0,
            yta - 2.0 * hleg / 10.0,
            yta + 12.0 * hleg / 10.0,
            yta + 12.0 * hleg / 10.0,
            yta - 2.0 * hleg / 10.0,
        ];
        self.g.poly2(&xb, &yb, 5, -wd, backgr);
        self.g.text3(&aleg, self.nleg, istyle, hleg, xtp, ytp, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        if self.xpoint <= 0.0 {
            return Ok(());
        }
        let xpp = self.g.xscale(self.xpoint);
        let ypp = self.g.yscale(ypoint);
        let xxx = xtp - hleg / 2.0;
        let yyy = ytp + hleg / 2.0;
        self.g.vect3(xxx, yyy, 0.0, xpp, ypp, 0.0, wt, ivec);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascalv_picks_round_limits() {
        // 0.3 .. 7.2 with m = 4: p = 1.72 selects a step of 1 (five minors), limits 0 .. 8.
        let (lo, hi, major, minor) = ascalv(4, 0.3, 7.2);
        assert_eq!((lo, hi, major, minor), (0.0, 8.0, 8, 40));
    }

    #[test]
    fn ascalv_refuses_unreasonable_input() {
        assert_eq!(ascalv(4, 5.0, 5.0), (0.0, 10.0, 1, 0));
        assert_eq!(ascalv(0, 1.0, 5.0), (0.0, 10.0, 1, 0));
        assert_eq!(ascalv(21, 1.0, 5.0), (0.0, 10.0, 1, 0));
    }

    #[test]
    fn ascalv_small_span_branch() {
        let (lo, hi, major, _) = ascalv(4, 0.001, 0.0095);
        assert!(lo <= 0.001 && hi >= 0.0095 && major > 0);
    }
}
