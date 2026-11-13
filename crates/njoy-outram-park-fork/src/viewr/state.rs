// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/viewr.f90` (module `viewm`: the common
// variables `setup1`..`setup7`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The module variables of `viewm`, with `graph`'s inside.

use super::graph::Graph;

/// A blank-padded 80-column Fortran `character(80)`.
pub type Str80 = Vec<u8>;

/// A blank `character(80)`.
pub fn blank80() -> Str80 {
    vec![b' '; 80]
}

/// Everything VIEWR remembers: `viewm`'s variables (`setup1`..`setup7`), the
/// `save`d locals of `set2d` (`itic`) and `legndb` (`x`, `y`), and the
/// [`Graph`] module. One value lives for a whole deck run, as the Fortran
/// process's static storage does.
#[derive(Debug, Clone)]
pub struct ViewrState {
    /// `module graph`, with the PostScript text being written.
    pub g: Graph,
    // setup1
    pub(super) xpage: f64,
    pub(super) ypage: f64,
    pub(super) wline: f64,
    pub(super) csize: f64,
    pub(super) lori: i32,
    pub(super) istyle: i32,
    pub(super) ipcol: i32,
    // setup2
    pub(super) xll: f64,
    pub(super) yll: f64,
    pub(super) ww: f64,
    pub(super) wh: f64,
    pub(super) wr: f64,
    pub(super) xg: f64,
    pub(super) yg: f64,
    pub(super) iwcol: i32,
    // setup3
    pub(super) hlab: f64,
    pub(super) n1: i32,
    pub(super) n2: i32,
    pub(super) nx: i32,
    pub(super) ny: i32,
    pub(super) nr: i32,
    pub(super) t1: Str80,
    pub(super) t2: Str80,
    pub(super) xl: Str80,
    pub(super) yl: Str80,
    pub(super) rl: Str80,
    // setup4
    pub(super) xmin: f64,
    pub(super) xmax: f64,
    pub(super) xstp: f64,
    pub(super) ymin: f64,
    pub(super) ymax: f64,
    pub(super) ystp: f64,
    pub(super) zmin: f64,
    pub(super) zmax: f64,
    pub(super) zstp: f64,
    pub(super) itype: i32,
    pub(super) jtype: i32,
    pub(super) igrid: i32,
    pub(super) ileg: i32,
    // setup5
    pub(super) icon: i32,
    pub(super) isym: i32,
    pub(super) idash: i32,
    pub(super) ithick: i32,
    pub(super) ishade: i32,
    pub(super) iccol: i32,
    // setup6
    pub(super) xtag: f64,
    pub(super) ytag: f64,
    pub(super) xpoint: f64,
    pub(super) xpt: f64,
    pub(super) ypt: f64,
    pub(super) hleg: f64,
    pub(super) nleg: i32,
    pub(super) aleg: Str80,
    // setup7
    pub(super) x3: f64,
    pub(super) y3: f64,
    pub(super) z3: f64,
    pub(super) xv: f64,
    pub(super) yv: f64,
    pub(super) zv: f64,
    /// `set2d`'s saved `itic`.
    pub(super) itic: i32,
    /// `legndb`'s saved `x`, `y`.
    pub(super) leg_x: f64,
    pub(super) leg_y: f64,
}

impl Default for ViewrState {
    fn default() -> Self {
        Self {
            g: Graph::default(),
            xpage: 0.0,
            ypage: 0.0,
            wline: 0.0,
            csize: 0.0,
            lori: 0,
            istyle: 0,
            ipcol: 0,
            xll: 0.0,
            yll: 0.0,
            ww: 0.0,
            wh: 0.0,
            wr: 0.0,
            xg: 0.0,
            yg: 0.0,
            iwcol: 0,
            hlab: 0.0,
            n1: 0,
            n2: 0,
            nx: 0,
            ny: 0,
            nr: 0,
            t1: blank80(),
            t2: blank80(),
            xl: blank80(),
            yl: blank80(),
            rl: blank80(),
            xmin: 0.0,
            xmax: 0.0,
            xstp: 0.0,
            ymin: 0.0,
            ymax: 0.0,
            ystp: 0.0,
            zmin: 0.0,
            zmax: 0.0,
            zstp: 0.0,
            itype: 0,
            jtype: 0,
            igrid: 0,
            ileg: 0,
            icon: 0,
            isym: 0,
            idash: 0,
            ithick: 0,
            ishade: 0,
            iccol: 0,
            xtag: 0.0,
            ytag: 0.0,
            xpoint: 0.0,
            xpt: 0.0,
            ypt: 0.0,
            hleg: 0.0,
            nleg: 0,
            aleg: blank80(),
            x3: 0.0,
            y3: 0.0,
            z3: 0.0,
            xv: 0.0,
            yv: 0.0,
            zv: 0.0,
            itic: 0,
            leg_x: 0.0,
            leg_y: 0.0,
        }
    }
}
