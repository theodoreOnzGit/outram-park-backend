// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/viewr.f90` (`set3d`, lines 1249-1524).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Three-dimensional surface plots: a family of curves `z(x)` at stepped `y`,
//! drawn back to front with hidden-line removal by white fill.

use petir::mathf::RealMath;

use super::plot2d::ascalv;
use super::state::ViewrState;
use crate::NjoyError;

/// Limits of a log axis, `set3d`'s snapping of `top`/`bot` to
/// `1, 2 (.301), 5 (.699)` per decade: returns `(lo, hi, itop)`.
fn log_limits(lo_val: f64, hi_val: f64) -> (f64, f64, i32) {
    let d0 = 0.001_f64;
    let d3 = 0.301_f64;
    let d7 = 0.699_f64;
    let ten = 10.0_f64;
    let top = hi_val.r_log10();
    let mut itop = top as i32;
    if top < 0.0 {
        itop -= 1;
    }
    let mut hi = f64::from(itop + 1);
    if top <= f64::from(itop) + d7 + d0 {
        hi = f64::from(itop) + d7;
    }
    if top <= f64::from(itop) + d3 + d0 {
        hi = f64::from(itop) + d3;
    }
    if top <= f64::from(itop) + d0 {
        hi = f64::from(itop);
    }
    hi = ten.r_powf(hi);
    let bot = lo_val.r_log10();
    let mut ibot = bot as i32;
    if bot < 0.0 {
        ibot -= 1;
    }
    let mut lo = f64::from(ibot);
    if bot >= f64::from(ibot) + d3 - d0 {
        lo = f64::from(ibot) + d3;
    }
    if bot >= f64::from(ibot) + d7 - d0 {
        lo = f64::from(ibot) + d7;
    }
    if bot >= f64::from(ibot + 1) - d0 {
        lo = f64::from(ibot + 1);
    }
    lo = ten.r_powf(lo);
    if hi / lo > ten.r_powi(13) {
        lo = ten.r_powi(itop - 13);
    }
    (lo, hi, itop)
}

impl ViewrState {
    /// `set3d(iplot,xyz,nxyz)` (`viewr.f90:1249`). `xyz` is the packed family:
    /// per curve `y, n, (x_1, z_1), ... (x_n, z_n)`, ended by `0, 0`.
    ///
    /// # Errors
    /// More than 2000 curves (upstream's `array overflow` error).
    #[allow(clippy::too_many_lines)] // upstream's routine
    pub fn set3d(&mut self, iplot: i32, xyz: &[f64]) -> Result<(), NjoyError> {
        let length = 2000usize;
        let big = 1.0e10_f64;
        let kmax = 1999usize;
        let wt = self.wline;
        if self.csize > 0.0 {
            self.hlab = self.csize;
        }
        if self.csize < 0.0 {
            self.hlab = -self.csize * self.wh;
        }
        if iplot == 1 {
            self.g.initp(self.lori, self.xpage, self.ypage, self.istyle, self.hlab, self.wline, 0, self.ipcol);
        }
        self.g.window(self.xll, self.yll, self.ww, self.wh, self.wr, &self.t1, self.n1, &self.t2, self.n2, 0);
        self.g.init3(self.x3, self.y3, self.z3, self.xv, self.yv, self.zv, self.iwcol);

        let a = |i: usize| xyz[i - 1];
        let (mut xlo, mut xhi, mut ylo, mut yhi, mut zlo, mut zhi) = (big, -big, big, -big, big, -big);
        let mut lll: Vec<usize> = Vec::new();
        let mut i = 1usize;
        let mut j = 1usize;
        let mut nn = 1_i32;
        while nn != 0 {
            lll.push(i);
            let yn = a(i);
            nn = a(i + 1).round() as i32;
            if nn != 0 {
                if yn < ylo {
                    ylo = yn;
                }
                if yn > yhi {
                    yhi = yn;
                }
                for k in 1..=(nn as usize) {
                    let xn = a(i + 2 * k);
                    if xn < xlo {
                        xlo = xn;
                    }
                    if xn > xhi {
                        xhi = xn;
                    }
                    let zn = a(i + 2 * k + 1);
                    if zn < zlo {
                        zlo = zn;
                    }
                    if zn > zhi {
                        zhi = zn;
                    }
                }
                i += 2 + 2 * nn as usize;
                j += 1;
                if j > length {
                    return Err(NjoyError::InvalidInput("viewr set3d: array overflow, increase the length parameter".into()));
                }
            }
        }
        let ncurv = j - 1;

        if self.zstp == 0.0 {
            self.zmin = zlo;
            self.zmax = zhi;
            if self.jtype == 1 {
                let (lo, hi, major, _) = ascalv(2, self.zmin, self.zmax);
                self.zmin = lo;
                self.zmax = hi;
                self.zstp = (self.zmax - self.zmin) / f64::from(major);
            } else {
                let (lo, hi, _) = log_limits(self.zmin, self.zmax);
                self.zmin = lo;
                self.zmax = hi;
            }
        }
        if self.ystp == 0.0 {
            self.ymin = ylo;
            self.ymax = yhi;
            if self.itype == 1 || self.itype == 3 {
                let (lo, hi, major, _) = ascalv(2, self.ymin, self.ymax);
                self.ymin = lo;
                self.ymax = hi;
                self.ystp = (self.ymax - self.ymin) / f64::from(major);
            } else {
                let (lo, hi, _) = log_limits(self.ymin, self.ymax);
                self.ymin = lo;
                self.ymax = hi;
            }
        }
        if self.xstp == 0.0 {
            self.xmin = xlo;
            self.xmax = xhi;
            if self.itype == 1 || self.itype == 2 {
                let (lo, hi, major, _) = ascalv(2, self.xmin, self.xmax);
                self.xmin = lo;
                self.xmax = hi;
                self.xstp = (self.xmax - self.xmin) / f64::from(major);
            } else {
                let (lo, hi, _) = log_limits(self.xmin, self.xmax);
                self.xmin = lo;
                self.xmax = hi;
            }
        }

        let (xmin, xmax, ymin, ymax, zmin, zmax) = (self.xmin, self.xmax, self.ymin, self.ymax, self.zmin, self.zmax);
        let (x3, y3, z3) = (self.x3, self.y3, self.z3);
        let (nx, ny, nr) = (self.nx, self.ny, self.nr);
        let mut zop = self.zstp;
        if self.jtype == 2 {
            zop = 0.0;
        }
        let (zero, one) = (0.0_f64, 1.0_f64);
        let (xl, yl, rl) = (self.xl.clone(), self.yl.clone(), self.rl.clone());
        let pos = x3 > zero;
        // z axis (uses the right-axis parameters)
        let xo = if pos { zero } else { x3 };
        self.g.axis3(zmin, zmax, zop, &rl, nr, 1, 1, z3, xo, zero, zero, zero, zero, one, zero, -one, zero, 1);
        // x axis
        let mut xop = self.xstp;
        if self.itype == 3 || self.itype == 4 {
            xop = 0.0;
        }
        self.g.axis3(xmin, xmax, xop, &xl, -nx, -1, 0, x3, zero, zero, zero, one, zero, zero, zero, one, zero, 0);
        // y axis
        let mut yop = self.ystp;
        if self.itype == 2 || self.itype == 4 {
            yop = 0.0;
        }
        if pos {
            self.g.axis3(ymin, ymax, yop, &yl, -ny, -1, 0, y3, x3, zero, zero, zero, one, zero, -one, zero, zero, 0);
        } else {
            self.g.axis3(ymin, ymax, yop, &yl, -ny, -1, 0, y3, zero, zero, zero, zero, one, zero, -one, zero, zero, 0);
        }
        // extra axes completing the frame
        let xo = if pos { zero } else { x3 };
        self.g.axis3(ymin, ymax, yop, b" ", 0, 0, 0, y3, xo, zero, zero, zero, one, zero, -one, zero, zero, 0);
        self.g.axis3(ymin, ymax, yop, b" ", 0, 0, 0, y3, xo, zero, z3, zero, one, zero, -one, zero, zero, 0);
        if pos {
            self.g.axis3(zmin, zmax, zop, b" ", 0, 0, 0, z3, zero, y3, zero, zero, zero, one, zero, -one, zero, 0);
            self.g.axis3(zmin, zmax, zop, b" ", 0, 0, 0, z3, x3, y3, zero, zero, zero, one, zero, -one, zero, 0);
        } else {
            self.g.axis3(zmin, zmax, zop, b" ", 0, 0, 0, z3, x3, y3, zero, zero, zero, one, zero, -one, zero, 0);
            self.g.axis3(zmin, zmax, zop, b" ", 0, 0, 0, z3, zero, y3, zero, zero, zero, one, zero, -one, zero, 0);
        }
        self.g.axis3(xmin, xmax, xop, b" ", 0, 0, 0, x3, zero, y3, zero, one, zero, zero, zero, one, zero, 0);
        self.g.axis3(xmin, xmax, xop, b" ", 0, 0, 0, x3, zero, y3, z3, one, zero, zero, zero, one, zero, 0);

        if self.igrid == 1 {
            self.g.grid3(1, 1, 0);
            if pos {
                self.g.grid3(0, 1, 1);
            } else {
                self.g.grid3(-1, 1, 1);
            }
            self.g.grid3(1, -1, 1);
        }

        // the curves, last first
        let mut x = vec![0.0_f64; 2000];
        let mut y = vec![0.0_f64; 2000];
        let mut z = vec![0.0_f64; 2000];
        for jj in 1..=ncurv {
            let l = lll[ncurv - jj];
            let yy = a(l);
            if yy >= ymin && yy <= ymax {
                let n = a(l + 1).round() as usize;
                x[0] = a(l + 2);
                y[0] = yy;
                z[0] = zmin;
                let mut k = 2usize;
                for i in 1..=n {
                    if k < kmax {
                        x[k - 1] = a(l + 2 * i);
                        if x[k - 1] < xmin {
                            x[k - 1] = xmin;
                        }
                        if x[k - 1] > xmax {
                            x[k - 1] = xmax;
                        }
                        y[k - 1] = yy;
                        z[k - 1] = a(l + 2 * i + 1);
                        if z[k - 1] < zmin {
                            z[k - 1] = zmin;
                        }
                        if z[k - 1] > zmax {
                            z[k - 1] = zmax;
                        }
                        k += 1;
                    }
                }
                x[k - 1] = a(l + 2 * n);
                y[k - 1] = yy;
                z[k - 1] = zmin;
                self.g.curv3(&mut x, &mut y, &mut z, k, wt);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_limits_snap_to_one_two_five() {
        // 3 .. 70: 70 is above 10^1 + .699 -> 100; 3 is above 10^0 + .301 -> 2.
        let (lo, hi, itop) = log_limits(3.0, 70.0);
        assert_eq!(itop, 1);
        assert!((hi - 100.0).abs() < 1e-9);
        assert!((lo - 10.0_f64.powf(0.301)).abs() < 1e-9);
    }
}
