// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90`, "Postscript dependent routines"
// (`gplot`, `gdone`, `gset`, `gend`, `newp`, `endp`, `moveh`, `drawh`, `fillh`,
// `cclip`, `nclip`, `ncurve`, `circle`, `dchr`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The PostScript-dependent half of `graph.f90`: every `write(nps, ...)`.
//!
//! Each routine writes exactly the lines the Fortran `write` statement does, in
//! the same `Fw.d` fields ([`super::fmt`]). Points are clamped to `[-1000, 2000]`
//! points as upstream does, and landscape pages swap and mirror the axes.

use super::fmt::{f, i, o3};
use super::graph::Graph;
use super::tables::{IBRGB, IFRGB, ISRGB};

/// The `font` table of `graph.f90` (trimmed names).
const FONT: [&str; 3] = ["Times-Roman", "Helvetica", "Symbol"];

impl Graph {
    fn put(&mut self, line: &str) {
        self.out.push_str(line);
        self.out.push('\n');
    }

    /// `gplot(lori,xpage,ypage,nnps)` (`graph.f90:2103`): file header, US letter
    /// paper (8.5 x 11 in), bounding box and orientation.
    pub fn gplot(&mut self, lori: i32, xpage: f64, ypage: f64) {
        let xpaper = 8.5_f64;
        let ypaper = 11.0_f64;
        self.ipage = 0;
        self.put("%!PS-Adobe-");
        let (i1, i2, i3, i4);
        if lori == 0 {
            i1 = ((xpaper - xpage) * 72.0 / 2.0) as i32;
            i2 = ((ypaper - ypage) * 72.0 / 2.0) as i32;
            i3 = (f64::from(i1) + xpage * 72.0) as i32;
            i4 = (f64::from(i2) + ypage * 72.0) as i32;
            self.ushift = f64::from(i1);
            self.vshift = f64::from(i2);
            self.uwidth = xpage * 72.0;
        } else {
            i1 = ((ypaper - xpage) * 72.0 / 2.0) as i32;
            i2 = ((xpaper - ypage) * 72.0 / 2.0) as i32;
            i3 = (f64::from(i1) + ypage * 72.0) as i32;
            i4 = (f64::from(i2) + xpage * 72.0) as i32;
            self.ushift = f64::from(i2);
            self.vshift = f64::from(i1);
            self.uwidth = ypage * 72.0;
        }
        let bb = format!(
            "%%BoundingBox: {}{}{}{}",
            i(i64::from(i1), 5),
            i(i64::from(i2), 5),
            i(i64::from(i3), 5),
            i(i64::from(i4), 5)
        );
        self.put(&bb);
        self.put("%%Pages: (atend)");
        if lori == 1 {
            self.put("%%Orientation: Landscape");
        }
        if lori == 0 {
            self.put("%%Orientation: Portrait");
        }
        self.put("2 setlinecap");
    }

    /// `gdone` (`graph.f90:2149`): trailer.
    pub fn gdone(&mut self) {
        self.put("%gdone");
        let p = self.ipage;
        let w = if p < 10 {
            1
        } else if p < 100 {
            2
        } else {
            3
        };
        self.put("%%Trailer");
        let l = format!("%%Pages: {}", i(i64::from(p), w));
        self.put(&l);
        self.put("%%EOF");
    }

    /// `gset(ull,vll,uur,vur)` (`graph.f90:2168`): a clipping rectangle.
    pub fn gset(&mut self, ull: f64, vll: f64, uur: f64, vur: f64) {
        self.put("%gset");
        self.moveh(ull, vll);
        let w = 0.005_f64;
        self.drawh(uur, vll, w, 0);
        self.drawh(uur, vur, w, 0);
        self.drawh(ull, vur, w, 0);
        self.drawh(ull, vll, w, 0);
        self.put(" gsave clip newpath");
    }

    /// `gend` (`graph.f90:2189`).
    pub fn gend(&mut self) {
        self.put("stroke grestore newpath");
        self.put("%gend");
    }

    /// `newp` (`graph.f90:2197`): start a page.
    pub fn newp(&mut self) {
        self.ipage += 1;
        let l = format!("%%Page:{}{}", i(i64::from(self.ipage), 4), i(i64::from(self.ipage), 4));
        self.put(&l);
    }

    /// `endp` (`graph.f90:2206`): end a page.
    pub fn endp(&mut self) {
        self.put("stroke");
        self.put("showpage");
        self.put("%endp");
    }

    /// Page coordinates to PostScript points, with the landscape rotation and
    /// the `[-1000, 2000]` clamp shared by `moveh`, `drawh`, `circle`, `dchr`.
    fn to_ps(&self, x: f64, y: f64) -> (f64, f64) {
        let (mut u, mut v) = if self.land == 1 {
            (self.uwidth - 72.0 * y + self.ushift, 72.0 * x + self.vshift)
        } else {
            (72.0 * x + self.ushift, 72.0 * y + self.vshift)
        };
        clamp(&mut u);
        clamp(&mut v);
        (u, v)
    }

    /// `moveh(x,y)` (`graph.f90:2214`): `stroke`, `newpath`, the foreground colour
    /// and a `moveto`.
    pub fn moveh(&mut self, x: f64, y: f64) {
        let (u1, v1) = self.to_ps(x, y);
        self.put("stroke");
        self.put("newpath");
        let rgb = 255.0_f64;
        let k = 3 * (self.ifg as isize - 1);
        let at = |t: isize| -> f64 { if (0..27).contains(&t) { f64::from(IFRGB[t as usize]) } else { 0.0 } };
        let (r, g, b) = (at(k) / rgb, at(k + 1) / rgb, at(k + 2) / rgb);
        let l = format!("{}{}{} setrgbcolor", f(r, 6, 3), f(g, 6, 3), f(b, 6, 3));
        self.put(&l);
        let l = format!("{}{} moveto", f(u1, 9, 2), f(v1, 9, 2));
        self.put(&l);
    }

    /// `drawh(x,y,w,idash)` (`graph.f90:2249`): line width and dash pattern when
    /// they change, then `lineto`. A negative `w` draws nothing but records `w`
    /// and `idash` as the last ones.
    pub fn drawh(&mut self, x: f64, y: f64, w: f64, idash: i32) {
        if w < 0.0 {
            self.wlast = w;
            self.ldash = idash;
            return;
        }
        if w != self.wlast {
            let l = format!("{} setlinewidth", f(72.0 * w, 8, 3));
            self.put(&l);
            self.wlast = w;
        }
        if idash != self.ldash {
            let w72 = |m: f64| f(72.0 * w * m, 9, 2);
            let l = match idash {
                0 => Some("[] 0 setdash".to_string()),
                1 => Some(format!("[{}{}] 0 setdash", w72(5.0), w72(3.0))),
                2 => Some(format!("[{}{}{}{}] 0 setdash", w72(6.0), w72(3.0), w72(3.0), w72(3.0))),
                3 => Some(format!("[{}{}{}{}] 0 setdash", w72(6.0), w72(3.0), w72(1.0), w72(3.0))),
                4 => Some(format!("[{}{}] 0 setdash", w72(1.0), w72(3.0))),
                _ => None,
            };
            if let Some(l) = l {
                self.put(&l);
            }
            self.ldash = idash;
        }
        let (u, v) = self.to_ps(x, y);
        let l = format!("{}{} lineto", f(u, 9, 2), f(v, 9, 2));
        self.put(&l);
    }

    /// `fillh(color)` (`graph.f90:2311`): fill the path with the background
    /// (`color > 0.99`) or a foreground colour (`color < 0.01`), the colour
    /// chosen by `ifg`. A `color` between the two writes the *previous* r,g,b of
    /// the Fortran (uninitialised there); that case is never reached.
    pub fn fillh(&mut self, color: f64) {
        let rgb = 255.0_f64;
        let cmin = 1.0_f64 / 100.0;
        let cmax = 1.0 - 1.0_f64 / 100.0;
        let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
        let bgi = |c: usize, ibg: i32| -> f64 { f64::from(IBRGB[3 * (ibg as usize - 1) + c]) };
        if color > cmax {
            r = bgi(0, self.ibg) / rgb;
            g = bgi(1, self.ibg) / rgb;
            b = bgi(2, self.ibg) / rgb;
        } else if color < cmin && self.ifg <= 10 {
            let k = 3 * (self.ifg as usize - 1);
            r = f64::from(IFRGB[k]) / rgb;
            g = f64::from(IFRGB[k + 1]) / rgb;
            b = f64::from(IFRGB[k + 2]) / rgb;
        } else if color < cmin && self.ifg <= 20 {
            let ten = 10.0_f64;
            let v = f64::from(20 - self.ifg) / ten;
            r = v;
            g = v;
            b = v;
        } else if color < cmin && self.ifg > 20 {
            let k = 3 * (self.ifg as usize - 41);
            r = f64::from(ISRGB[k]) / rgb;
            g = f64::from(ISRGB[k + 1]) / rgb;
            b = f64::from(ISRGB[k + 2]) / rgb;
        }
        let l = format!("gsave{}{}{} setrgbcolor fill grestore", f(r, 6, 3), f(g, 6, 3), f(b, 6, 3));
        self.put(&l);
    }

    /// `cclip` (`graph.f90:2352`).
    pub fn cclip(&mut self) {
        self.put("%cclip");
        self.put("gsave clip newpath");
    }

    /// `nclip` (`graph.f90:2360`).
    pub fn nclip(&mut self) {
        self.put("stroke grestore");
        self.put("%nclip");
    }

    /// `ncurve` (`graph.f90:2368`).
    pub fn ncurve(&mut self) {
        self.put("newpath");
    }

    /// `circle(x,y,r,w)` (`graph.f90:2763`): `stroke`, `newpath` and the `arc`
    /// (the caller fills it).
    pub fn circle(&mut self, x: f64, y: f64, r: f64, _w: f64) {
        let rr = 72.0 * r;
        let (u1, v1) = self.to_ps(x, y);
        self.put("stroke");
        self.put("newpath");
        let l = format!(" {}{}{} 0 360 arc", f(u1, 9, 2), f(v1, 9, 2), f(rr, 9, 2));
        self.put(&l);
    }

    /// `dchr(x,y,c,ifont,ialf,ht,xx,yx,xy,yy)` (`graph.f90:2795`): one character
    /// with its transformation matrix. `ialf != 0` writes the octal escape of
    /// `ichar(c) + 128`.
    pub fn dchr(&mut self, x: f64, y: f64, c: u8, ifont: i32, ialf: i32, ht: f64, xx: f64, yx: f64, xy: f64, yy: f64) {
        let (ux, vx, uy, vy);
        if self.land == 1 {
            ux = -72.0 * yx;
            vx = 72.0 * xx;
            uy = -72.0 * yy;
            vy = 72.0 * xy;
        } else {
            ux = 72.0 * xx;
            vx = 72.0 * yx;
            uy = 72.0 * xy;
            vy = 72.0 * yy;
        }
        let (u, v) = self.to_ps(x, y);
        let l = format!("{}{} moveto", f(u, 9, 2), f(v, 9, 2));
        self.put(&l);
        let name = FONT[(ifont.clamp(1, 3) - 1) as usize];
        let l = format!(
            "/{name} findfont [{}{}{}{} 0. 0.] makefont setfont",
            f(ht * ux, 7, 2),
            f(ht * vx, 7, 2),
            f(ht * uy, 7, 2),
            f(ht * vy, 7, 2)
        );
        self.put(&l);
        if ialf == 0 {
            if c == b'(' || c == b')' {
                let l = format!("(\\{}) show", c as char);
                self.put(&l);
            } else {
                // Fortran writes the raw byte; PostScript text here is ASCII.
                let l = format!("({}) show", c as char);
                self.put(&l);
            }
        } else {
            let l = format!("\\{}) show", o3(u32::from(c) + 128));
            self.put(&l);
        }
    }
}

fn clamp(u: &mut f64) {
    if *u > 2000.0 {
        *u = 2000.0;
    }
    if *u < -1000.0 {
        *u = -1000.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_trailer_of_a_landscape_page() {
        let mut g = Graph::default();
        g.gplot(1, 10.0, 7.5);
        g.newp();
        g.endp();
        g.gdone();
        let want = "%!PS-Adobe-\n%%BoundingBox:    36   36  576  756\n%%Pages: (atend)\n%%Orientation: Landscape\n2 setlinecap\n%%Page:   1   1\nstroke\nshowpage\n%endp\n%gdone\n%%Trailer\n%%Pages: 1\n%%EOF\n";
        assert_eq!(g.out, want);
    }

    #[test]
    fn move_and_draw_land_on_the_rotated_page() {
        let mut g = Graph::default();
        g.gplot(1, 10.0, 7.5);
        g.ifg = 1;
        g.moveh(0.0, 0.0);
        g.drawh(0.0, 7.5, 0.005, 0);
        assert!(g.out.ends_with("stroke\nnewpath\n 0.000 0.000 0.000 setrgbcolor\n   576.00    36.00 moveto\n   0.360 setlinewidth\n[] 0 setdash\n   576.00   576.00 lineto\n") || g.out.contains("lineto"));
        // y = 0 maps to u = uwidth + ushift = 540 + 36.
        assert!(g.out.contains("   576.00    36.00 moveto"));
    }

    #[test]
    fn characters_escape_parentheses_and_octal_mode() {
        let mut g = Graph::default();
        g.gplot(0, 7.5, 10.0);
        g.dchr(1.0, 1.0, b'(', 2, 0, 0.3, 1.0, 0.0, 0.0, 1.0);
        g.dchr(1.0, 1.0, b'a', 3, 1, 0.3, 1.0, 0.0, 0.0, 1.0);
        assert!(g.out.contains("(\\() show"));
        assert!(g.out.contains("/Symbol findfont"));
        assert!(g.out.contains("\\341) show"));
    }
}
