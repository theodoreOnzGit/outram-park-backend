// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90` (module `graph`: `initp`,
// `window`, `endw`, `transw`, `init2`, `frame2`, `endfr`, `init3`, `trans3`,
// `vect2`, `vect3`, `xscale`..`yinvrs`, `grid2`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The state of Fortran `module graph` and its page, window and perspective
//! routines.
//!
//! Every module-level variable of `graph.f90` is a field of [`Graph`]; the
//! `save`d locals of `drawh` (`wlast`, `ldash`) are fields too, so a second
//! VIEWR in one deck sees what the first left behind, as in the Fortran
//! process. The PostScript is accumulated in [`Graph::out`].

use petir::mathf::RealMath;

use super::tables::{CW1, CW10, CW11, CW12, CW2, CW3, CW4, CW5, CW6, CW7, CW8, CW9};

/// All module variables of `graph.f90`.
#[derive(Debug, Clone)]
pub struct Graph {
    /// The PostScript file being written (unit `nps`).
    pub out: String,
    pub(super) ipage: i32,
    pub(super) width: f64,
    pub(super) ht: f64,
    pub(super) ifont: i32,
    pub(super) ifg: i32,
    pub(super) ibg: i32,
    pub(super) land: i32,
    pub(super) wt: f64,
    pub(super) wg: f64,
    pub(super) wa: f64,
    pub(super) tic: f64,
    pub(super) gap: f64,
    pub(super) hf: f64,
    pub(super) hl: f64,
    pub(super) hn: f64,
    pub(super) lfont: i32,
    pub(super) fvx: f64,
    pub(super) dvx: f64,
    pub(super) fvy: f64,
    pub(super) dvy: f64,
    pub(super) fvz: f64,
    pub(super) dvz: f64,
    pub(super) logx: i32,
    pub(super) logy: i32,
    pub(super) logz: i32,
    pub(super) xwll: f64,
    pub(super) ywll: f64,
    pub(super) www: f64,
    pub(super) wwh: f64,
    pub(super) wwr: f64,
    pub(super) tspace: f64,
    pub(super) rs: f64,
    pub(super) ro: f64,
    pub(super) rp: f64,
    pub(super) ct: f64,
    pub(super) st: f64,
    pub(super) cp: f64,
    pub(super) sp: f64,
    pub(super) du: f64,
    pub(super) dv: f64,
    pub(super) ushift: f64,
    pub(super) vshift: f64,
    pub(super) uwidth: f64,
    pub(super) xmin: f64,
    pub(super) xmax: f64,
    pub(super) xstp: f64,
    pub(super) ymin: f64,
    pub(super) ymax: f64,
    pub(super) ystp: f64,
    pub(super) zmin: f64,
    pub(super) zmax: f64,
    pub(super) zstp: f64,
    /// `drawh`'s `wlast` (initial 0).
    pub(super) wlast: f64,
    /// `drawh`'s `ldash` (initial -1).
    pub(super) ldash: i32,
    /// `cw(128, 3)`, loaded by `initp`: `cw[font - 1][code]`.
    pub(super) cw: [[f64; 128]; 3],
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            out: String::new(),
            ipage: 0,
            width: 0.0,
            ht: 0.0,
            ifont: 0,
            ifg: 0,
            ibg: 0,
            land: 0,
            wt: 0.0,
            wg: 0.0,
            wa: 0.0,
            tic: 0.0,
            gap: 0.0,
            hf: 0.0,
            hl: 0.0,
            hn: 0.0,
            lfont: 0,
            fvx: 0.0,
            dvx: 0.0,
            fvy: 0.0,
            dvy: 0.0,
            fvz: 0.0,
            dvz: 0.0,
            logx: 0,
            logy: 0,
            logz: 0,
            xwll: 0.0,
            ywll: 0.0,
            www: 0.0,
            wwh: 0.0,
            wwr: 0.0,
            tspace: 0.0,
            rs: 0.0,
            ro: 0.0,
            rp: 0.0,
            ct: 0.0,
            st: 0.0,
            cp: 0.0,
            sp: 0.0,
            du: 0.0,
            dv: 0.0,
            ushift: 0.0,
            vshift: 0.0,
            uwidth: 0.0,
            xmin: 0.0,
            xmax: 0.0,
            xstp: 0.0,
            ymin: 0.0,
            ymax: 0.0,
            ystp: 0.0,
            zmin: 0.0,
            zmax: 0.0,
            zstp: 0.0,
            wlast: 0.0,
            ldash: -1,
            cw: [[0.0; 128]; 3],
        }
    }
}

impl Graph {
    /// `initp(iori,xpage,ypage,istyle,htt,wtt,ibord,ipcol)`: initialise the page
    /// (`graph.f90:212`). Loads the font tables, resets the view and starts
    /// page 1+: the page background is a filled polygon.
    pub fn initp(&mut self, iori: i32, xpage: f64, ypage: f64, istyle: i32, htt: f64, wtt: f64, _ibord: i32, ipcol: i32) {
        let tabs: [[&[f64; 32]; 4]; 3] = [[&CW1, &CW2, &CW3, &CW4], [&CW5, &CW6, &CW7, &CW8], [&CW9, &CW10, &CW11, &CW12]];
        for (f, quarter) in tabs.iter().enumerate() {
            for (q, t) in quarter.iter().enumerate() {
                for n in 0..32 {
                    self.cw[f][32 * q + n] = t[n];
                }
            }
        }
        // `wbord=.005e0` is a *single-precision* literal in `initp` (no `_kr`): 0.004999999888...
        // It differs from the double `.005e0_kr` of `gset`, so `drawh` sees a new width there.
        let wbord = f64::from(0.005_f32);
        self.land = iori;
        self.ifont = istyle;
        self.width = wtt;
        self.ht = htt;
        self.ibg = 1 + ipcol;
        self.ifg = 1;
        self.ro = 1.0;
        self.rs = 1.0;
        self.ct = 0.0;
        self.st = 1.0;
        self.cp = 0.0;
        self.sp = -1.0;
        self.du = 0.0;
        self.dv = 0.0;
        self.xwll = 0.0;
        self.ywll = 0.0;
        self.www = 0.0;
        self.wwh = 0.0;
        self.wwr = 0.0;
        self.newp();
        let x = [0.0, xpage, xpage, 0.0, 0.0];
        let y = [0.0, 0.0, ypage, ypage, 0.0];
        let mut w = wbord;
        if self.ibg == 1 {
            w = -1.0;
        }
        self.poly2(&x, &y, 5, w, 1.0);
    }

    /// `window(xll,yll,ww,wh,wr,t1,n1,t2,n2,ibord)` (`graph.f90:286`): set the
    /// window, clip to it and write the title lines. `t1`/`t2` are the 80-column
    /// blank-padded strings; `n1`/`n2` their counts.
    pub fn window(&mut self, xll: f64, yll: f64, ww: f64, wh: f64, wr: f64, t1: &[u8], n1: i32, t2: &[u8], n2: i32, ibord: i32) {
        self.ifg = 1;
        let wbord = 0.005_f64;
        self.xwll = xll;
        self.ywll = yll;
        self.www = ww;
        self.wwh = wh;
        self.wwr = wr;
        if ibord != 0 {
            let w = wbord;
            let (wh, ww) = (self.wwh, self.www);
            self.vect2(0.0, 0.0, 0.0, wh, w, 0);
            self.vect2(0.0, wh, ww, wh, w, 0);
            self.vect2(ww, wh, ww, 0.0, w, 0);
            self.vect2(ww, 0.0, 0.0, 0.0, w, 0);
        }
        let (ull, vll) = self.transw(0.0, 0.0);
        let (uur, vur) = self.transw(self.www, self.wwh);
        self.gset(ull, vll, uur, vur);
        self.tspace = 0.0;
        if n1 != 0 {
            let ht = self.ht;
            let x = 3.0 * ht;
            let mut y = self.wwh - 3.0 * ht / 2.0;
            self.tspace += 3.0 * ht / 2.0;
            let f = self.ifont;
            self.text2(t1, n1, f, ht, x, y, 1.0, 0.0, 0.0, 1.0);
            if n2 != 0 {
                y -= ht + ht / 5.0;
                self.tspace += 3.0 * ht / 2.0;
                self.text2(t2, n2, f, ht, x, y, 1.0, 0.0, 0.0, 1.0);
            }
        }
    }

    /// `endw` (`graph.f90:343`): end the window clipping, restore the default
    /// transformation.
    pub fn endw(&mut self) {
        self.gend();
        self.ro = 1.0;
        self.rs = 1.0;
        self.ct = 0.0;
        self.st = 1.0;
        self.cp = 0.0;
        self.sp = -1.0;
        self.du = 0.0;
        self.dv = 0.0;
        self.www = 0.0;
        self.wwh = 0.0;
        self.wwr = 0.0;
    }

    /// `transw(x,y,u,v)` (`graph.f90:363`): window transformation. `pi` is
    /// `physics`' `pi = 3.141592653589793238e0_kr`.
    pub fn transw(&self, x: f64, y: f64) -> (f64, f64) {
        if self.www == 0.0 {
            (x, y)
        } else {
            let pi = std::f64::consts::PI;
            let ct = (2.0 * pi * self.wwr / 360.0).r_cos();
            let st = (2.0 * pi * self.wwr / 360.0).r_sin();
            (self.xwll + x * ct - y * st, self.ywll + x * st + y * ct)
        }
    }

    /// `init2(uo,vo,xg,yg,iright,iwcol)` (`graph.f90:386`): returns `(uo, vo,
    /// xg, yg)`, the origin and axis lengths of the graph in the window, and
    /// colours the inside of the frame.
    pub fn init2(&mut self, iright: i32, iwcol: i32) -> (f64, f64, f64, f64) {
        let wframe = 0.005_f64;
        let ht = self.ht;
        let uo = 4.0 * ht;
        let vo = 4.0 * ht;
        let mut xg = self.www - uo - 2.0 * ht;
        if iright > 0 {
            xg -= 2.0 * ht;
        }
        let mut yg = self.wwh - self.tspace - vo - ht;
        if self.tspace == 0.0 {
            yg -= ht;
        }
        self.wt = self.width;
        self.wg = self.wt / 2.0;
        self.wa = 2.0 * self.wt;
        self.tic = ht / 2.0;
        self.gap = 4.0 * ht / 10.0;
        self.lfont = self.ifont;
        self.hf = 3.0;
        self.hf /= 4.0;
        self.hl = ht;
        self.hn = 8.0 * self.hl / 10.0;
        self.ro = 1.0;
        self.rs = 1.0;
        self.ct = 0.0;
        self.st = 1.0;
        self.cp = 0.0;
        self.sp = -1.0;
        self.du = uo;
        self.dv = vo;
        self.ibg = 1 + iwcol;
        self.ifg = 1;
        let (ull, vll) = self.transw(uo, vo);
        let (uur, vur) = self.transw(uo + xg, vo + yg);
        let x = [ull, uur, uur, ull, ull];
        let y = [vll, vll, vur, vur, vll];
        self.poly2(&x, &y, 5, wframe, 1.0);
        (uo, vo, xg, yg)
    }

    /// `frame2(xg,yg,grace,iccol)` (`graph.f90:460`): clip just outside the frame
    /// and set the curve colour.
    pub fn frame2(&mut self, xg: f64, yg: f64, grace: f64, iccol: i32) {
        self.ifg = 1 + iccol;
        let (xll, yll) = self.trans3(-grace, -grace, 0.0);
        let (xur, yur) = self.trans3(xg + grace, yg + grace, 0.0);
        let (ull, vll) = self.transw(xll, yll);
        let (uur, vur) = self.transw(xur, yur);
        self.gset(ull, vll, uur, vur);
    }

    /// `endfr` (`graph.f90:480`).
    pub fn endfr(&mut self) {
        self.gend();
    }

    /// `init3(bx,by,bz,vx,vy,vz,iwcol)` (`graph.f90:487`): 3-d perspective set-up,
    /// scaled and centred in the window.
    pub fn init3(&mut self, bx: f64, by: f64, bz: f64, vx: f64, vy: f64, vz: f64, iwcol: i32) {
        self.ibg = 1 + iwcol;
        self.ifg = 1;
        let ht = self.ht;
        self.wt = self.width;
        self.wg = self.wt / 2.0;
        self.wa = self.wt;
        self.tic = ht / 2.0;
        self.gap = 4.0 * ht / 10.0;
        self.lfont = self.ifont;
        self.hf = 3.0;
        self.hf /= 4.0;
        self.hl = ht;
        self.hn = ht;
        self.ro = (vx * vx + vy * vy + vz * vz).sqrt();
        self.rp = (vx * vx + vy * vy).sqrt();
        self.ct = self.rp / self.ro;
        self.st = vz / self.ro;
        self.cp = vx / self.rp;
        self.sp = vy / self.rp;
        self.rs = self.ro / (bx * bx + by * by + bz * bz).sqrt();
        self.du = 0.0;
        self.dv = 0.0;
        let mut umin = 1000.0_f64;
        let mut umax = -1000.0_f64;
        let mut vmin = 1000.0_f64;
        let mut vmax = -1000.0_f64;
        let mut u = 0.0_f64;
        let mut v = 0.0_f64;
        let upd = |u: f64, v: f64, umin: &mut f64, umax: &mut f64, vmin: &mut f64, vmax: &mut f64| {
            if u < *umin {
                *umin = u;
            }
            if u > *umax {
                *umax = u;
            }
            if v < *vmin {
                *vmin = v;
            }
            if v > *vmax {
                *vmax = v;
            }
        };
        let mut yy = -5.0 * ht;
        if bx > 0.0 {
            (u, v) = self.trans3(0.0, yy, 0.0);
        }
        if bx < 0.0 {
            (u, v) = self.trans3(bx, yy, 0.0);
        }
        upd(u, v, &mut umin, &mut umax, &mut vmin, &mut vmax);
        if bx > 0.0 {
            (u, v) = self.trans3(0.0, by, bz + ht);
        }
        if bx < 0.0 {
            (u, v) = self.trans3(bx, by, bz + ht);
        }
        upd(u, v, &mut umin, &mut umax, &mut vmin, &mut vmax);
        let xx = bx + 2.0 * ht;
        yy = -7.0 * ht / 2.0;
        if bx > 0.0 {
            (u, v) = self.trans3(xx, yy, 0.0);
        }
        if bx < 0.0 {
            (u, v) = self.trans3(0.0, yy, 0.0);
        }
        upd(u, v, &mut umin, &mut umax, &mut vmin, &mut vmax);
        let dx = 3.0 * ht;
        let dy = 2.0 * ht;
        if bx > 0.0 {
            (u, v) = self.trans3(bx + dx, by + dy, 0.0);
        }
        if bx < 0.0 {
            (u, v) = self.trans3(dx, by + dy, 0.0);
        }
        upd(u, v, &mut umin, &mut umax, &mut vmin, &mut vmax);
        let su = self.www / (umax - umin);
        let sv = self.wwh / (vmax - vmin);
        let mut s = su;
        if sv < su {
            s = sv;
        }
        self.rs *= s;
        self.du = (self.www - s * (umax - umin)) / 2.0 - s * umin;
        self.dv = (self.wwh - s * (vmax - vmin)) / 2.0 - s * vmin;
    }

    /// `trans3(x,y,z,u,v)` (`graph.f90:571`): the perspective transformation.
    pub fn trans3(&self, x: f64, y: f64, z: f64) -> (f64, f64) {
        let xo = self.ct * (self.cp * x + self.sp * y) + self.st * z;
        let s = self.rs / (self.ro - xo);
        let u = s * (-self.sp * x + self.cp * y) + self.du;
        let v = s * (-self.st * (self.cp * x + self.sp * y) + self.ct * z) + self.dv;
        (u, v)
    }

    /// `vect2` (`graph.f90:585`).
    pub fn vect2(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, w: f64, ivec: i32) {
        self.vect3(x1, y1, 0.0, x2, y2, 0.0, w, ivec);
    }

    /// `vect3(x1,y1,z1,x2,y2,z2,w,ivec)` (`graph.f90:599`): a 3-d vector, with an
    /// arrow head when `ivec != 0`.
    pub fn vect3(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, w: f64, ivec: i32) {
        let ahead = 0.2_f64;
        let (wu1, wv1) = self.trans3(x1, y1, z1);
        let (wu2, wv2) = self.trans3(x2, y2, z2);
        let (u1, v1) = self.transw(wu1, wv1);
        let (u2, v2) = self.transw(wu2, wv2);
        if ivec == 0 {
            self.moveh(u1, v1);
            self.drawh(u2, v2, w, 0);
        } else {
            let head = ahead;
            let r = ((u2 - u1).r_powi(2) + (v2 - v1).r_powi(2)).sqrt();
            let wx = w * (u2 - u1) / r;
            let wy = w * (v2 - v1) / r;
            let dx = head * (u2 - u1) / r;
            let dy = head * (v2 - v1) / r;
            let hx = dx / 2.0;
            let hy = dy / 2.0;
            self.moveh(u1, v1);
            self.drawh(u2 - wx, v2 - wy, w, 0);
            self.moveh(u2 - dx - hy, v2 - dy + hx);
            self.drawh(u2, v2, w, 0);
            self.drawh(u2 - dx + hy, v2 - dy - hx, w, 0);
        }
    }

    /// `xscale` (`graph.f90:1141`): user x to axis coordinate.
    pub fn xscale(&self, x: f64) -> f64 {
        let mut xn = x;
        if self.logx == 1 {
            xn = xn.r_log10();
        }
        (xn - self.fvx) / self.dvx
    }

    /// `yscale` (`graph.f90:1156`).
    pub fn yscale(&self, y: f64) -> f64 {
        let mut yn = y;
        if self.logy == 1 {
            yn = yn.r_log10();
        }
        (yn - self.fvy) / self.dvy
    }

    /// `zscale` (`graph.f90:1171`).
    pub fn zscale(&self, z: f64) -> f64 {
        let mut zn = z;
        if self.logz == 1 {
            zn = zn.r_log10();
        }
        (zn - self.fvz) / self.dvz
    }

    /// `xinvrs` (`graph.f90:1186`): axis coordinate back to user x.
    pub fn xinvrs(&self, x: f64) -> f64 {
        let xn = x * self.dvx + self.fvx;
        if self.logx == 1 {
            10.0_f64.r_powf(xn)
        } else {
            xn
        }
    }

    /// `yinvrs` (`graph.f90:1202`).
    pub fn yinvrs(&self, y: f64) -> f64 {
        let yn = y * self.dvy + self.fvy;
        if self.logy == 1 {
            10.0_f64.r_powf(yn)
        } else {
            yn
        }
    }

    /// `grid2(nx,ny)` (`graph.f90:1218`).
    pub fn grid2(&mut self, nx: i32, ny: i32) {
        self.zmin = 0.0;
        self.zmax = 0.0;
        self.grid3(nx, ny, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_start_writes_the_background_polygon() {
        let mut g = Graph::default();
        g.land = 1;
        g.initp(1, 10.0, 7.5, 2, 0.3, 0.0134, 0, 1);
        assert!(g.out.starts_with("%%Page:   1   1\nstroke\nnewpath\n"));
        assert!(g.out.contains(" setrgbcolor fill grestore"));
        assert_eq!(g.cw[1][b'A' as usize], 0.667);
        assert_eq!(g.cw[2][b'a' as usize], 0.631);
    }

    #[test]
    fn perspective_is_flat_for_the_default_view() {
        let mut g = Graph::default();
        g.initp(0, 7.5, 10.0, 2, 0.3, 0.0134, 0, 0);
        g.ro = 1.0;
        g.rs = 1.0;
        g.ct = 0.0;
        g.st = 1.0;
        g.cp = 0.0;
        g.sp = -1.0;
        let (u, v) = g.trans3(2.0, 3.0, 0.0);
        assert_eq!((u, v), (2.0, 3.0));
    }
}
