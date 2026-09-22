// Ported from NJOY2016 `src/endf.f90` (subroutine `gety1`, lines 1467-1571).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `gety1` — the **sequential** TAB1 getter, and why it is not `terpa`.
//!
//! NJOY has two ways to read a value out of a TAB1, and they do not agree at
//! the boundaries. [`crate::endf::interp::terpa`] is the random-access one.
//! `gety1` is the sequential one used by the module drivers that walk a
//! cross-section's own grid — ACER's photoatomic path (`acepa.f90:104-118`) is
//! one, GROUPR and MIXR are others. Its differences from `terpa` are not
//! incidental:
//!
//! | situation | `terpa` | `gety1` |
//! |---|---|---|
//! | leading zero-valued points | interpolates through them | **skips** them at initialisation (`endf.f90:1497-1516`, "check for zero extension as in mf13") |
//! | first retained abscissa | `x_1` | `0.999999 x_k` when `y_k != 0` |
//! | at/above the last point | `y_NP` out to `1.00001 x_NP`, then 0 | `y_NP`, with `xnext = xlast = 1e12` |
//! | below the retained range | `y = 0`, `xnext = x_1` | `y = 0`, `xnext = xlast` |
//!
//! A caller that reaches for `terpa` where upstream used `gety1` gets an
//! energy grid that differs at its first point and its last, which is exactly
//! the kind of silent off-by-one-panel a shared-grid comparison cannot see.
//!
//! Upstream pages the tape in blocks (`moreio`); this port holds the whole
//! TAB1 in memory, which is the `nb = 0` case of every branch. The sequential
//! state (`ip`, `ir`, `xlast`, `ylast`) is kept because it is *semantic* here,
//! not an optimisation: `xlast` is what the "below the range" branch returns.

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::Tab1;

/// `down` (`endf.f90:1480`) — how far below a retained abscissa the panel is
/// considered to start.
const DOWN: f64 = 0.999_999;
/// `xbig` (`endf.f90:1481`).
const XBIG: f64 = 1.0e12;

/// One value retrieved from a TAB1 by [`Gety1::get`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gety1Value {
    /// `y1` — the interpolated value.
    pub y: f64,
    /// `xnext` — the first tabulated abscissa above `x`, or `xbig` at the end.
    pub xnext: f64,
    /// `idis` — `xnext` is a discontinuity (histogram region, or the next
    /// point duplicates this abscissa).
    pub idis: bool,
}

/// A TAB1 being read in ascending `x`, with `gety1`'s state and conventions.
#[derive(Debug, Clone)]
pub struct Gety1 {
    interp: Vec<(u32, u32)>,
    pairs: Vec<(f64, f64)>,
    /// 1-based index of the panel's upper point.
    ip: usize,
    /// 1-based interpolation-region index.
    ir: usize,
    xlast: f64,
    ylast: f64,
}

impl Gety1 {
    /// Upstream's `x = 0` call: read the table and position at the first point
    /// the zero-extension scan retains (`endf.f90:1491-1530`).
    ///
    /// The scan walks forward while **both** this point's `y` and the next
    /// point's `y` are zero, so a table padded with leading zeros (MF=13's
    /// convention, and common in MF=23) starts at its last zero rather than at
    /// its first. `idis` is `false` after this call — upstream sets it at the
    /// top of the routine and the initialisation branch never raises it, which
    /// is what the ACER grid walk's first iteration depends on.
    pub fn new(t: &Tab1) -> Self {
        let pairs: Vec<(f64, f64)> = t.pairs.clone();
        let interp: Vec<(u32, u32)> = if t.interp.is_empty() {
            vec![(pairs.len().max(1) as u32, 2)]
        } else {
            t.interp.clone()
        };
        let np = pairs.len();
        let mut ip = 1usize;
        let mut ir = 1usize;
        let (mut xlast, mut ylast) = (0.0f64, 0.0f64);
        while np > 0 {
            let (x_ip, y_ip) = pairs[ip - 1];
            xlast = x_ip;
            ylast = y_ip;
            if ylast != 0.0 {
                xlast *= DOWN;
                break;
            }
            if np < 2 || ip >= np - 1 {
                break;
            }
            if pairs[ip].1 != 0.0 {
                break;
            }
            ip += 1;
            let nbt = interp[(ir - 1).min(interp.len() - 1)].0 as usize;
            if ip > nbt && ir < interp.len() {
                ir += 1;
            }
        }
        Gety1 {
            interp,
            pairs,
            ip,
            ir,
            xlast,
            ylast,
        }
    }

    /// Retrieve `y(x)`. `x` must not decrease between calls — that is what
    /// "sequential" means, and upstream's `x < xlast` branch returns zero
    /// rather than re-seeking.
    pub fn get(&mut self, x: f64) -> Gety1Value {
        let np = self.pairs.len();
        if np == 0 {
            return Gety1Value {
                y: 0.0,
                xnext: XBIG,
                idis: false,
            };
        }
        // `:1544` — below the current panel.
        if x < self.xlast {
            return Gety1Value {
                y: 0.0,
                xnext: self.xlast,
                idis: false,
            };
        }
        loop {
            if x < self.pairs[self.ip - 1].0 {
                break;
            }
            if self.ip == np {
                // `:1566` — the last point and above.
                let y = self.pairs[np - 1].1;
                self.xlast = XBIG;
                return Gety1Value {
                    y,
                    xnext: XBIG,
                    idis: false,
                };
            }
            self.xlast = self.pairs[self.ip - 1].0;
            self.ylast = self.pairs[self.ip - 1].1;
            self.ip += 1;
            let nbt = self.interp[(self.ir - 1).min(self.interp.len() - 1)].0 as usize;
            if self.ip > nbt && self.ir < self.interp.len() {
                self.ir += 1;
            }
            if x < self.xlast {
                return Gety1Value {
                    y: 0.0,
                    xnext: self.xlast,
                    idis: false,
                };
            }
        }
        // `:1556` — interpolate inside this panel.
        let law = self.interp[(self.ir - 1).min(self.interp.len() - 1)].1;
        let mut idis = law == 1;
        let (x2, y2) = self.pairs[self.ip - 1];
        let y = terp1(
            self.xlast,
            self.ylast,
            x2,
            y2,
            x,
            IntLaw::from_code(law),
        )
        .unwrap_or(self.ylast);
        if self.ip < np && self.pairs[self.ip].0 == x2 {
            idis = true;
        }
        Gety1Value {
            y,
            xnext: x2,
            idis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::records::Cont;

    fn tab1(pairs: &[(f64, f64)], law: u32) -> Tab1 {
        Tab1 {
            head: Cont {
                c1: 0.0,
                c2: 0.0,
                l1: 0,
                l2: 0,
                n1: 1,
                n2: pairs.len() as i32,
            },
            interp: vec![(pairs.len() as u32, law)],
            pairs: pairs.to_vec(),
        }
    }

    /// The zero-extension scan is the difference that changes an energy grid's
    /// first point, so it is pinned rather than described.
    #[test]
    fn leading_zeros_are_skipped_at_initialisation() {
        let t = tab1(&[(1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 5.0), (5.0, 6.0)], 2);
        let g = Gety1::new(&t);
        // Walks to the last zero whose successor is still zero: x = 3, y = 0.
        assert_eq!((g.xlast, g.ylast), (3.0, 0.0));
        // `terpa` on the same table starts at x = 1 instead.
        let (y, xnext, idis) = crate::endf::interp::terpa(&t.interp, &t.pairs, 1.0);
        assert_eq!((y, xnext, idis), (0.0, 2.0, false));
    }

    /// A table whose first point already carries a value keeps it, shaded down
    /// by `down` so the first `get` at that abscissa lands inside the panel.
    #[test]
    fn a_nonzero_first_point_is_shaded_down() {
        let t = tab1(&[(10.0, 1.0), (20.0, 2.0)], 2);
        let g = Gety1::new(&t);
        assert!((g.xlast - 10.0 * DOWN).abs() < 1e-12);
        assert_eq!(g.ylast, 1.0);
    }

    /// At and above the last point `gety1` holds the last value out to
    /// infinity; `terpa` drops to zero just past `shade` times it.
    #[test]
    fn last_point_and_above_differs_from_terpa() {
        let t = tab1(&[(10.0, 1.0), (20.0, 2.0)], 2);
        let mut g = Gety1::new(&t);
        let v = g.get(20.0);
        assert_eq!((v.y, v.xnext), (2.0, XBIG));
        let v = g.get(1.0e6);
        assert_eq!((v.y, v.xnext), (0.0, XBIG));
        assert_eq!(
            crate::endf::interp::terpa(&t.interp, &t.pairs, 1.0e6),
            (0.0, crate::endf::interp::TERPA_XBIG, false)
        );
    }

    /// Interpolation inside a panel agrees with `terp1`, and a duplicated
    /// abscissa above the panel raises `idis`.
    #[test]
    fn panel_interpolation_and_discontinuity_flag() {
        let t = tab1(&[(1.0, 1.0), (2.0, 2.0), (2.0, 9.0), (3.0, 9.0)], 2);
        let mut g = Gety1::new(&t);
        let v = g.get(1.5);
        assert!((v.y - 1.5).abs() < 1e-12);
        assert_eq!(v.xnext, 2.0);
        assert!(v.idis, "the next point duplicates x = 2, so xnext is a jump");
    }
}
