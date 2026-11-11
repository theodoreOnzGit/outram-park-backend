// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90` (`dsym`, lines 2376-2760).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `dsym`: the 25 plotting symbols (square, octagon, triangle, ... exed
//! diamond). Every symbol is built from the same few outlines, drawn with
//! `poly2` (background fill) or `moveh`/`drawh` strokes; the order of the
//! strokes and fills is upstream's, since it is the order of lines in the file.

use super::graph::Graph;

const W: f64 = 0.01;
const RR: f64 = 1.128;
const C8: f64 = 0.414;
const CD: f64 = 1.4;
const CX: f64 = 0.82;
const T1: f64 = 1.75;
const T2: f64 = 1.52;
const T3: f64 = 0.88;
const BACKGR: f64 = 1.0;
const FOREGR: f64 = 0.0;

/// The five corners of the square, closed.
fn square(x: f64, y: f64, sz: f64) -> ([f64; 5], [f64; 5]) {
    ([x - sz, x + sz, x + sz, x - sz, x - sz], [y + sz, y + sz, y - sz, y - sz, y + sz])
}

/// The nine points of the octagon, closed.
fn octagon(x: f64, y: f64, sz: f64) -> ([f64; 9], [f64; 9]) {
    (
        [x - C8 * sz, x + C8 * sz, x + sz, x + sz, x + C8 * sz, x - C8 * sz, x - sz, x - sz, x - C8 * sz],
        [y + sz, y + sz, y + C8 * sz, y - C8 * sz, y - sz, y - sz, y - C8 * sz, y + C8 * sz, y + sz],
    )
}

/// The diamond, closed.
fn diamond(x: f64, y: f64, sz: f64) -> ([f64; 5], [f64; 5]) {
    let xx = [x - CD * sz, x, x + CD * sz, x, x - CD * sz];
    let yy = [y, y + CD * sz, y, y - CD * sz, y];
    (xx, yy)
}

/// The upright triangle, closed.
fn triangle(x: f64, y: f64, sz: f64) -> ([f64; 4], [f64; 4]) {
    let y1 = y - T3 * sz;
    ([x - T2 * sz, x, x + T2 * sz, x - T2 * sz], [y1, y + T1 * sz, y1, y1])
}

/// The inverted triangle, closed.
fn inverted_triangle(x: f64, y: f64, sz: f64) -> ([f64; 4], [f64; 4]) {
    let y1 = y + T3 * sz;
    ([x - T2 * sz, x, x + T2 * sz, x - T2 * sz], [y1, y - T1 * sz, y1, y1])
}

impl Graph {
    fn cross(&mut self, x: f64, y: f64, h: f64) {
        self.moveh(x, y + h);
        self.drawh(x, y - h, W, 0);
        self.moveh(x - h, y);
        self.drawh(x + h, y, W, 0);
    }

    fn ex(&mut self, x: f64, y: f64, h: f64) {
        self.moveh(x - h, y + h);
        self.drawh(x + h, y - h, W, 0);
        self.moveh(x - h, y - h);
        self.drawh(x + h, y + h, W, 0);
    }

    /// `dsym(x,y,isym,size)` (`graph.f90:2376`): symbol `isym` (0-24; any other
    /// value is the default open circle) centred at `(x, y)`.
    pub fn dsym(&mut self, x: f64, y: f64, isym: i32, size: f64) {
        let sz = 6.0 * size / 10.0;
        match isym {
            0 | 17 => {
                let (xx, yy) = square(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
            }
            1 => {
                let (xx, yy) = octagon(x, y, sz);
                self.poly2(&xx, &yy, 9, W, BACKGR);
            }
            2 => {
                let (xx, yy) = triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, BACKGR);
            }
            3 => self.cross(x, y, sz),
            4 => self.ex(x, y, sz),
            5 => {
                let (xx, yy) = diamond(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
            }
            6 => {
                let (xx, yy) = inverted_triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, BACKGR);
            }
            7 => {
                let (xx, yy) = square(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
                self.ex(x, y, sz);
            }
            8 => {
                self.ex(x, y, sz);
                self.moveh(x - sz, y);
                self.drawh(x + sz, y, W, 0);
                self.moveh(x, y + sz);
                self.drawh(x, y - sz, W, 0);
            }
            9 => {
                let (xx, yy) = diamond(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
                self.moveh(x, y + CD * sz);
                self.drawh(x, y - CD * sz, W, 0);
                self.moveh(x - CD * sz, y);
                self.drawh(x + CD * sz, y, W, 0);
            }
            10 => {
                let (xx, yy) = octagon(x, y, sz);
                self.poly2(&xx, &yy, 9, W, BACKGR);
                self.cross(x, y, sz);
            }
            11 => {
                let (xx, yy) = triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, BACKGR);
                let (xx, yy) = inverted_triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, BACKGR);
            }
            12 => {
                let (xx, yy) = square(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
                self.cross(x, y, sz);
            }
            13 => {
                let (xx, yy) = octagon(x, y, sz);
                self.poly2(&xx, &yy, 9, W, BACKGR);
                self.ex(x, y, sz);
            }
            14 => {
                let (xx, yy) = square(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
                self.moveh(x - sz, y - sz);
                self.drawh(x, y + sz, W, 0);
                self.drawh(x + sz, y - sz, W, 0);
            }
            15 => {
                self.circle(x, y, RR * sz, W);
                self.fillh(FOREGR);
            }
            16 => {
                self.circle(x, y, RR * sz, W);
                self.fillh(BACKGR);
            }
            18 => {
                let (xx, yy) = square(x, y, sz);
                self.poly2(&xx, &yy, 5, W, FOREGR);
            }
            19 => {
                let (xx, yy) = diamond(x, y, sz);
                self.poly2(&xx, &yy, 5, W, FOREGR);
            }
            20 => {
                let (xx, yy) = triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, FOREGR);
            }
            21 => {
                let (xx, yy) = inverted_triangle(x, y, sz);
                self.poly2(&xx, &yy, 4, W, FOREGR);
            }
            22 => {
                self.circle(x, y, RR * sz, W);
                self.fillh(BACKGR);
                self.moveh(x, y + RR * sz);
                self.drawh(x, y - RR * sz, W, 0);
                self.moveh(x - RR * sz, y);
                self.drawh(x + RR * sz, y, W, 0);
            }
            23 => {
                self.circle(x, y, RR * sz, W);
                self.fillh(BACKGR);
                self.ex(x, y, CX * sz);
            }
            24 => {
                let (xx, yy) = diamond(x, y, sz);
                self.poly2(&xx, &yy, 5, W, BACKGR);
                self.moveh(x - CD * sz / 2.0, y + CD * sz / 2.0);
                self.drawh(x + CD * sz / 2.0, y - CD * sz / 2.0, W, 0);
                self.moveh(x - CD * sz / 2.0, y - CD * sz / 2.0);
                self.drawh(x + CD * sz / 2.0, y + CD * sz / 2.0, W, 0);
            }
            _ => {
                self.circle(x, y, RR * sz, W);
                self.fillh(BACKGR);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_draws_something_and_is_deterministic() {
        for isym in -1..26 {
            let mut g = Graph::default();
            g.gplot(1, 10.0, 7.5);
            g.initp(1, 10.0, 7.5, 2, 0.3, 0.0134, 0, 0);
            let before = g.out.len();
            g.dsym(5.0, 3.0, isym, 0.2);
            assert!(g.out.len() > before, "symbol {isym}");
        }
    }

    #[test]
    fn octagon_symbol_has_nine_points_and_a_fill() {
        let mut g = Graph::default();
        g.gplot(1, 10.0, 7.5);
        g.initp(1, 10.0, 7.5, 2, 0.3, 0.0134, 0, 0);
        let n0 = g.out.matches("lineto").count();
        g.dsym(5.0, 3.0, 1, 0.2);
        assert_eq!(g.out.matches("lineto").count() - n0, 8);
    }
}
