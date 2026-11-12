// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90` (`grid3`, `curv2`, `curv3`,
// `poly2`, `poly3`, `hatch`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Grids, curves, polygons and hatching.

use super::graph::Graph;

const SUBS9: [f64; 9] = [0.301, 0.477, 0.602, 0.699, 0.778, 0.845, 0.903, 0.954, 1.000];

/// The start of a log grid (`grid3`'s `v=fv; i=nint(v); k=0 ...`): returns the
/// offset `xt` of the decade start and the first index `k` (1-based).
fn log_start(fv: f64, dv: f64, x1: f64) -> (f64, usize) {
    let i = fv.round() as i32;
    let mut k = 0usize;
    let test = 1.0_f64 / 10.0;
    if (fv - f64::from(i) - SUBS9[0]).abs() < test {
        k = 1;
    }
    if (fv - f64::from(i) - SUBS9[3]).abs() < test {
        k = 4;
    }
    let mut xt = x1;
    if k > 0 {
        xt = x1 - SUBS9[k - 1] / dv;
    }
    k += 1;
    (xt, k)
}

impl Graph {
    /// One direction of `grid3`: grid lines at positions between `lo` and `hi` of
    /// axis `dir` (0 x, 1 y, 2 z); `c` holds the ends of the two other
    /// coordinates (x-lines: `y1,y2,z1,z2`; y-lines: `x1,x2,z1,z2`; z-lines:
    /// `x1,x2,y1,y2`).
    fn grid_lines(&mut self, dir: u8, c: [f64; 4], lo: f64, hi: f64, stp: f64, n: i32, log: i32, fv: f64, dv: f64) {
        let mut x1 = lo;
        let (mut xt, mut k) = (0.0, 0usize);
        if log == 1 {
            (xt, k) = log_start(fv, dv, x1);
        }
        loop {
            if log == 0 {
                x1 += stp / f64::from(n);
            } else {
                x1 = xt + SUBS9[k - 1] / dv;
                k += 1;
                if k > 9 {
                    xt = x1;
                    k = 1;
                }
            }
            let test = 1.0_f64 / 100.0;
            if (hi - x1).abs() < test {
                break;
            }
            // c = [a1, a2, b1, b2]: the two fixed coordinates' ends.
            let l = match dir {
                0 => [x1, c[0], c[2], x1, c[1], c[3]],
                1 => [c[0], x1, c[2], c[1], x1, c[3]],
                _ => [c[0], c[2], x1, c[1], c[3], x1],
            };
            let wg = self.wg;
            self.vect3(l[0], l[1], l[2], l[3], l[4], l[5], wg, 0);
        }
    }

    /// `grid3(nx,ny,nz)` (`graph.f90:1231`): grid lines `n` per axis step in the
    /// plane whose own count is 0 (at the minimum) or -1 (at the maximum).
    #[allow(clippy::many_single_char_names)] // upstream's names
    pub fn grid3(&mut self, nx: i32, ny: i32, nz: i32) {
        let (xmin, xmax, ymin, ymax, zmin, zmax) = (self.xmin, self.xmax, self.ymin, self.ymax, self.zmin, self.zmax);
        if nx > 0 {
            let (y1, y2, z1, z2);
            if nz == 0 {
                (y1, y2, z1, z2) = (ymin, ymax, zmin, zmin);
            } else if nz == -1 {
                (y1, y2, z1, z2) = (ymin, ymax, zmax, zmax);
            } else if ny == 0 {
                (y1, y2, z1, z2) = (ymin, ymin, zmin, zmax);
            } else {
                (y1, y2, z1, z2) = (ymax, ymax, zmin, zmax);
            }
            let (stp, log, fv, dv) = (self.xstp, self.logx, self.fvx, self.dvx);
            self.grid_lines(0, [y1, y2, z1, z2], xmin, xmax, stp, nx, log, fv, dv);
        }
        if ny > 0 {
            let (x1, x2, z1, z2);
            if nz == 0 {
                (x1, x2, z1, z2) = (xmin, xmax, zmin, zmin);
            } else if nz == -1 {
                (x1, x2, z1, z2) = (xmin, xmax, zmax, zmax);
            } else if nx == 0 {
                (x1, x2, z1, z2) = (xmin, xmin, zmin, zmax);
            } else {
                (x1, x2, z1, z2) = (xmax, xmax, zmin, zmax);
            }
            let (stp, log, fv, dv) = (self.ystp, self.logy, self.fvy, self.dvy);
            self.grid_lines(1, [x1, x2, z1, z2], ymin, ymax, stp, ny, log, fv, dv);
        }
        if nz > 0 {
            let (y1, y2, x1, x2);
            if nx == 0 {
                (y1, y2, x1, x2) = (ymin, ymax, xmin, xmin);
            } else if nx == -1 {
                (y1, y2, x1, x2) = (ymin, ymax, xmax, xmax);
            } else if ny == 0 {
                (y1, y2, x1, x2) = (ymin, ymin, xmin, xmax);
            } else {
                (y1, y2, x1, x2) = (ymax, ymax, xmin, xmax);
            }
            let (stp, log, fv, dv) = (self.zstp, self.logz, self.fvz, self.dvz);
            self.grid_lines(2, [x1, x2, y1, y2], zmin, zmax, stp, nz, log, fv, dv);
        }
    }

    /// `curv2(x,y,n,idash,width,icon,isym,ssym,ishade)` (`graph.f90:1430`): a 2-d
    /// curve, with symbols and a shading or hatching pattern. `width == 0`
    /// makes the curve invisible (shaded areas without a border).
    #[allow(clippy::too_many_arguments)] // upstream's signature
    pub fn curv2(&mut self, x: &[f64], y: &[f64], n: usize, idash: i32, width: f64, icon: i32, isym: i32, ssym: f64, ishade: i32) {
        let w = width;
        if icon >= 0 {
            let mut wu1 = 100.0_f64;
            let mut wu2 = -100.0_f64;
            let mut wv1 = 100.0_f64;
            let mut wv2 = -100.0_f64;
            let xn = self.xscale(x[0]);
            let yn = self.yscale(y[0]);
            let (wu, wv) = self.trans3(xn, yn, 0.0);
            if wu < wu1 {
                wu1 = wu;
            }
            if wu > wu2 {
                wu2 = wu;
            }
            if wv < wv1 {
                wv1 = wv;
            }
            if wv > wv2 {
                wv2 = wv;
            }
            let (u, v) = self.transw(wu, wv);
            self.moveh(u, v);
            for i in 1..n {
                let xn = self.xscale(x[i]);
                let yn = self.yscale(y[i]);
                let (wu, wv) = self.trans3(xn, yn, 0.0);
                if wu < wu1 {
                    wu1 = wu;
                }
                if wu > wu2 {
                    wu2 = wu;
                }
                if wv < wv1 {
                    wv1 = wv;
                }
                if wv > wv2 {
                    wv2 = wv;
                }
                let (u, v) = self.transw(wu, wv);
                self.drawh(u, v, w, idash);
            }
            let foregr = 0.0;
            if ishade > 0 && ishade <= 10 {
                self.ifg = ishade + 10;
                self.fillh(foregr);
                self.ifg = 0;
            } else if ishade > 40 {
                self.ifg = ishade;
                self.fillh(foregr);
                self.ifg = 0;
            }
            if ishade > 10 {
                self.hatch(ishade, wu1, wv1, wu2, wv2);
            }
            if width == 0.0 {
                self.ncurve();
            }
        }
        if icon != 0 {
            for i in 1..=n {
                if (i as i32) % icon.abs() == 0 {
                    let xn = self.xscale(x[i - 1]);
                    let yn = self.yscale(y[i - 1]);
                    let (wu, wv) = self.trans3(xn, yn, 0.0);
                    let (u, v) = self.transw(wu, wv);
                    self.dsym(u, v, isym, ssym);
                }
            }
        }
    }

    /// `curv3(x,y,z,n,w)` (`graph.f90:1507`): a 3-d curve as a polygon with
    /// background fill; later curves hide earlier ones. `x`, `y`, `z` have room
    /// for `n + 1` points (the closing baseline point is written at `n`).
    pub fn curv3(&mut self, x: &mut [f64], y: &mut [f64], z: &mut [f64], n: usize, w: f64) {
        x[n] = x[0];
        y[n] = y[0];
        z[n] = z[0];
        let max = n + 1;
        let xn = self.xscale(x[0]);
        let yn = self.yscale(y[0]);
        let zn = self.zscale(z[0]);
        let (wu, wv) = self.trans3(xn, yn, zn);
        let (u, v) = self.transw(wu, wv);
        self.moveh(u, v);
        for i in 1..max {
            let xn = self.xscale(x[i]);
            let yn = self.yscale(y[i]);
            let zn = self.zscale(z[i]);
            let (wu, wv) = self.trans3(xn, yn, zn);
            let (u, v) = self.transw(wu, wv);
            self.drawh(u, v, w, 0);
        }
        self.fillh(1.0);
    }

    /// `poly2(x,y,n,w,fill)` (`graph.f90:1545`): a filled polygon; `w < 0` omits
    /// the outline.
    pub fn poly2(&mut self, x: &[f64], y: &[f64], n: usize, w: f64, fill: f64) {
        self.moveh(x[0], y[0]);
        for i in 1..n {
            self.drawh(x[i], y[i], w, 0);
        }
        self.fillh(fill);
    }

    /// `poly3` (`graph.f90:1564`): 3-d polygon outline.
    pub fn poly3(&mut self, x: &[f64], y: &[f64], z: &[f64], n: usize, w: f64) {
        let (wu, wv) = self.trans3(x[0], y[0], z[0]);
        let (u, v) = self.transw(wu, wv);
        self.moveh(u, v);
        for i in 1..n {
            let (wu, wv) = self.trans3(x[i], y[i], z[i]);
            let (u, v) = self.transw(wu, wv);
            self.drawh(u, v, w, 0);
        }
    }

    /// `hatch(ihatch,wu1,wv1,wu2,wv2)` (`graph.f90:1586`): hatch or cross-hatch
    /// inside the current path, clipped to it.
    pub fn hatch(&mut self, ihatch: i32, wu1: f64, wv1: f64, wu2: f64, wv2: f64) {
        let w = 0.003_f64;
        let oh2 = 0.02_f64;
        let iop = (ihatch - 1) / 10;
        let dx = oh2 * f64::from(10 - ihatch + 10 * iop + 1);
        let ww = wv2 - wv1;
        self.cclip();
        if iop == 1 || iop == 3 {
            let mut x1 = wu1 - dx * f64::from((ww / dx) as i32) - dx;
            let y1 = wv1;
            let y2 = wv2;
            while x1 < wu2 + w {
                x1 += dx;
                let (u, v) = self.transw(x1, y1);
                self.moveh(u, v);
                let x2 = x1 + ww;
                let (u, v) = self.transw(x2, y2);
                self.drawh(u, v, w, 0);
            }
        }
        if iop == 2 || iop == 3 {
            let mut x1 = wu2 - dx * f64::from(((wu2 - wu1 + ww) / dx) as i32) - dx;
            let mut x2 = wu2 + w - 1.0;
            let y1 = wv1;
            let y2 = wv2;
            while x2 < wu2 + w {
                x1 += dx;
                let (u, v) = self.transw(x1, y1);
                self.moveh(u, v);
                x2 = x1 - ww;
                let (u, v) = self.transw(x2, y2);
                self.drawh(u, v, w, 0);
            }
        }
        self.nclip();
    }
}
