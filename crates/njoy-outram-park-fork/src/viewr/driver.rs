// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/viewr.f90` (`viewr`, lines 42-755: the
// plot-command reader and the page loop).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! VIEWR proper: read a plot-command file (written by PLOTR, COVR, DTFR or
//! ACER's `aplots`/`tplots`) with list-directed input and draw each plot.
//!
//! **Not reproduced:** the listing VIEWR prints on the output unit (the echo of
//! every card, `nsyso`); only the PostScript file is. Upstream's `mess` warning
//! for more than 500 000 words of 3-d data is not printed either (the data is
//! truncated as upstream truncates it).

use super::plot2d::Curve2d;
use super::state::{blank80, Str80, ViewrState};
use crate::deck::cards::{CardReader, Slot};
use crate::NjoyError;

/// `mmax` (same in plotr and viewr): points per 2-d curve.
const MMAX: usize = 20000;
/// `maxaa`: words of 3-d data.
const MAXAA: usize = 500_000;

/// `read(infile,*) text` into a `character(80)`: the blank-padded text and the
/// index of its last non-blank character (`n1` and friends).
fn read_text(r: &mut CardReader) -> Result<(Str80, i32), NjoyError> {
    let t = r.read_string(80)?;
    let mut s = blank80();
    for (k, b) in t.bytes().take(80).enumerate() {
        s[k] = b;
    }
    let n = s.iter().rposition(|&c| c != b' ').map_or(0, |p| p as i32 + 1);
    Ok((s, n))
}

fn too_many(what: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("viewr: too many {what} (upstream arrays hold {MMAX} points per curve)"))
}

impl ViewrState {
    /// `viewr` (`viewr.f90:42`): turn the plot-command text `plot` into the
    /// PostScript file, returned as text (what upstream writes to unit `nps`).
    ///
    /// # Errors
    /// A malformed or truncated command file (list-directed read errors), a
    /// curve of more than 20 000 points, or an error of a routine it calls.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // upstream's routine
    pub fn viewr(&mut self, plot: &str) -> Result<String, NjoyError> {
        let xpaper = 8.5_f64;
        let ypaper = 11.0_f64;
        let xmarg = 1.0_f64;
        let ymarg = 1.0_f64;
        let dsize = 0.3_f64;
        let dline = 0.0134_f64;
        let (dxv, dyv, dzv, dx3, dy3, dz3) = (15.0_f64, -15.0_f64, 15.0_f64, 2.5_f64, 6.5_f64, 2.5_f64);
        let mut r = CardReader::new(plot);

        let mut s = [Slot::Int(1), Slot::Int(2), Slot::Real(dsize), Slot::Int(0)];
        r.read(&mut s)?;
        self.lori = s[0].int();
        self.istyle = s[1].int();
        self.csize = s[2].real();
        self.ipcol = s[3].int();
        if self.lori == 1 {
            self.xpage = ypaper - ymarg;
            self.ypage = xpaper - xmarg;
        } else {
            self.xpage = xpaper - xmarg;
            self.ypage = ypaper - ymarg;
        }
        self.wline = dline;

        self.g.out.clear();
        self.g.gplot(self.lori, self.xpage, self.ypage);

        let mut nplot = 0;
        let mut i3d = 0;
        let mut iplot = 1;
        let mut nform = 0;
        let mut curve = Curve2d::default();
        let mut aa: Vec<f64> = Vec::new();
        while iplot != 99 {
            let mut s = [
                Slot::Int(1),
                Slot::Int(0),
                Slot::Real(1.0),
                Slot::Real(1.0),
                Slot::Real(0.0),
                Slot::Real(0.0),
                Slot::Real(self.xpage),
                Slot::Real(self.ypage),
                Slot::Real(0.0),
            ];
            r.read(&mut s)?;
            iplot = s[0].int();
            if iplot == 99 {
                break;
            }
            self.iwcol = s[1].int();
            let factx = s[2].real();
            let facty = s[3].real();
            self.xll = s[4].real();
            self.yll = s[5].real();
            self.ww = s[6].real();
            self.wh = s[7].real();
            self.wr = s[8].real();
            let mut ierrb = 0;

            if iplot.abs() == 1 {
                let (t1, n1) = read_text(&mut r)?;
                self.t1 = t1;
                self.n1 = n1;
                let (t2, n2) = read_text(&mut r)?;
                self.t2 = t2;
                self.n2 = n2;
                let mut s = [Slot::Int(4), Slot::Int(0), Slot::Int(2), Slot::Int(0), Slot::Real(0.0), Slot::Real(0.0)];
                r.read(&mut s)?;
                self.itype = s[0].int();
                self.jtype = s[1].int();
                self.igrid = s[2].int();
                self.ileg = s[3].int();
                self.xtag = s[4].real();
                self.ytag = s[5].real();
                if self.itype != 0 {
                    i3d = 0;
                    if self.itype <= 0 {
                        i3d = 1;
                        self.itype = -self.itype;
                    }
                    let mut s = [Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0)];
                    r.read(&mut s)?;
                    self.xmin = s[0].real();
                    self.xmax = s[1].real();
                    self.xstp = s[2].real();
                    if (self.itype == 3 || self.itype == 4) && (self.xmin != 0.0 || self.xmax != 0.0) {
                        self.xstp = 1.0;
                    }
                    let (xl, nx) = read_text(&mut r)?;
                    self.xl = xl;
                    self.nx = nx;
                    let mut s = [Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0)];
                    r.read(&mut s)?;
                    self.ymin = s[0].real();
                    self.ymax = s[1].real();
                    self.ystp = s[2].real();
                    if (self.itype == 2 || self.itype == 4) && (self.ymin != 0.0 || self.ymax != 0.0) {
                        self.ystp = 1.0;
                    }
                    let (yl, ny) = read_text(&mut r)?;
                    self.yl = yl;
                    self.ny = ny;
                    if self.jtype != 0 {
                        let mut s = [Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0)];
                        r.read(&mut s)?;
                        self.zmin = s[0].real();
                        self.zmax = s[1].real();
                        self.zstp = s[2].real();
                        if self.jtype == 2 && (self.zmin != 0.0 || self.zmax != 0.0) {
                            self.zstp = 1.0;
                        }
                        let (rl, nr) = read_text(&mut r)?;
                        self.rl = rl;
                        self.nr = nr;
                    }
                }
            }

            if self.itype != 0 {
                // the dummy card, for consistency with plotr
                r.read(&mut [])?;
                if i3d == 0 {
                    let mut s = [Slot::Int(0), Slot::Int(0), Slot::Int(0), Slot::Int(0), Slot::Int(1), Slot::Int(0)];
                    r.read(&mut s)?;
                    self.icon = s[0].int();
                    self.isym = s[1].int();
                    self.idash = s[2].int();
                    self.iccol = s[3].int();
                    self.ithick = s[4].int();
                    self.ishade = s[5].int();
                    if self.ileg != 0 {
                        let (aleg, nleg) = read_text(&mut r)?;
                        self.aleg = aleg;
                        self.nleg = nleg;
                        if self.ileg == 2 {
                            let mut s = [Slot::Real(0.0), Slot::Real(0.0), Slot::Real(0.0)];
                            r.read(&mut s)?;
                            self.xtag = s[0].real();
                            self.ytag = s[1].real();
                            self.xpoint = s[2].real();
                        }
                    }
                } else {
                    let mut s = [
                        Slot::Real(dxv),
                        Slot::Real(dyv),
                        Slot::Real(dzv),
                        Slot::Real(dx3),
                        Slot::Real(dy3),
                        Slot::Real(dz3),
                    ];
                    r.read(&mut s)?;
                    self.xv = s[0].real();
                    self.yv = s[1].real();
                    self.zv = s[2].real();
                    self.x3 = s[3].real();
                    self.y3 = s[4].real();
                    self.z3 = s[5].real();
                }

                let mut s = [Slot::Int(0)];
                r.read(&mut s)?;
                nform = s[0].int();

                if nform == 0 {
                    // 2-d data, until the card with neither x nor y
                    curve = Curve2d::default();
                    let flag = -99.0_f64;
                    loop {
                        let mut z = [
                            Slot::Real(flag),
                            Slot::Real(flag),
                            Slot::Real(0.0),
                            Slot::Real(0.0),
                            Slot::Real(0.0),
                            Slot::Real(0.0),
                        ];
                        r.read(&mut z)?;
                        let (z1, z2) = (z[0].real(), z[1].real());
                        if z1 == flag && z2 == flag {
                            break;
                        }
                        if curve.x.len() >= MMAX {
                            return Err(too_many("points"));
                        }
                        curve.x.push(z1 * factx);
                        curve.y.push(z2 * facty);
                        let dym = z[2].real() * facty;
                        let mut dyp = z[3].real() * facty;
                        if dyp == 0.0 {
                            dyp = dym;
                        }
                        let dxm = z[4].real() * factx;
                        let mut dxp = z[5].real() * factx;
                        if dxp == 0.0 {
                            dxp = dxm;
                        }
                        curve.dym.push(dym);
                        curve.dyp.push(dyp);
                        curve.dxm.push(dxm);
                        curve.dxp.push(dxp);
                        if dym != 0.0 {
                            ierrb = 1;
                        }
                        if dxm != 0.0 {
                            ierrb = 2;
                        }
                        if dym != 0.0 && dxm != 0.0 {
                            ierrb = 3;
                        }
                    }
                } else if nform == 1 {
                    // 3-d family of curves, same x grid assumed
                    aa.clear();
                    let mut iskip = 0;
                    let flag = -99.0_f64;
                    let mut ll = 0usize;
                    loop {
                        let mut s = [Slot::Real(flag)];
                        r.read(&mut s)?;
                        let yy = s[0].real();
                        if yy == flag {
                            break;
                        }
                        if iskip == 0 {
                            if aa.len() + 5000 >= MAXAA {
                                iskip = 1;
                            }
                            aa.push(yy);
                            aa.push(0.0);
                            ll = aa.len() - 1;
                        }
                        let mut nn = 0.0_f64;
                        loop {
                            let mut s = [Slot::Real(flag), Slot::Real(flag)];
                            r.read(&mut s)?;
                            let (xx, zz) = (s[0].real(), s[1].real());
                            if xx == flag && zz == flag {
                                break;
                            }
                            if iskip == 0 {
                                nn += 1.0;
                                aa.push(xx);
                                aa.push(zz);
                            }
                        }
                        if iskip == 0 {
                            aa[ll] = nn;
                        }
                    }
                    aa.push(0.0);
                    aa.push(0.0);
                }
            }

            if nplot != 0 && iplot == 1 {
                self.g.endp();
            }
            if iplot == -1 {
                self.g.endw();
            }
            if nform == 0 {
                self.set2d(iplot, &curve, ierrb)?;
            } else {
                self.set3d(iplot, &aa)?;
            }
            nplot = 1;
        }
        self.g.endw();
        self.g.endp();
        self.g.gdone();
        Ok(self.g.out.clone())
    }
}
