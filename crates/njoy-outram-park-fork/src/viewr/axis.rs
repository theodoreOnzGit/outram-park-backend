// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/graph.f90` (`axis2`, `axis3`, lines 638-1139).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Axes: tick marks, numbers and labels, linear and logarithmic.
//!
//! `axis3` draws an axis in 3-d perspective and, as a side effect, loads the
//! scale of the axis it runs along (`fvx`, `dvx`, `logx`, `xmin`, `xmax`,
//! `xstp`, and the `y`, `z` counterparts) that [`Graph::xscale`] and the grid
//! use. Numbers of a linear axis are scaled by `10**nscale` (a multiple of 3)
//! and the factor is written as `*10^n`; log axes are labelled `10^n`, the
//! exponent raised with the `#EH.8<` instruction.

use petir::mathf::RealMath;

use super::fmt::{f, i as fmt_i};
use super::graph::Graph;
use super::text::stripv;

const SUBS: [f64; 8] = [0.301, 0.477, 0.602, 0.699, 0.778, 0.845, 0.903, 0.954];

/// `write(num,'(''<prefix>'',iN,''#HXEX<'')') n` with upstream's choice of `N`:
/// 2 for `-9..-1` and `>= 10`, 3 for `<= -10`, 1 otherwise. Returns the string
/// (its length is `lnum`).
fn exp_label(prefix: &str, n: i32) -> Vec<u8> {
    let w = if (n > -10 && n < 0) || n >= 10 {
        2
    } else if n <= -10 {
        3
    } else {
        1
    };
    format!("{prefix}{}#HXEX<", fmt_i(i64::from(n), w)).into_bytes()
}

impl Graph {
    /// `axis2` (`graph.f90:638`): a 2-d axis through [`Graph::axis3`].
    #[allow(clippy::too_many_arguments)] // upstream's signature
    pub fn axis2(
        &mut self,
        amin: f64,
        amax: f64,
        astp: f64,
        label: &[u8],
        nlabel: i32,
        itic: i32,
        nonum: i32,
        asize: f64,
        xo: f64,
        yo: f64,
        xx: f64,
        yx: f64,
        xy: f64,
        yy: f64,
        numr: i32,
    ) {
        self.axis3(amin, amax, astp, label, nlabel, itic, nonum, asize, xo, yo, 0.0, xx, yx, 0.0, xy, yy, 0.0, numr);
    }

    /// `axis3` (`graph.f90:655`). `astp = 0` is a log axis; `nlabel` sign puts
    /// numbers and label above (`> 0`) or below (`< 0`); `itic` ticks above/below;
    /// `nonum` omits the first (1), last (2) or both (3) numbers; `numr != 0`
    /// turns the numbers 90 degrees.
    #[allow(clippy::too_many_arguments, clippy::cognitive_complexity)] // upstream's signature and flow
    pub fn axis3(
        &mut self,
        amin: f64,
        amax: f64,
        astp: f64,
        label: &[u8],
        nlabel: i32,
        itic: i32,
        nonum: i32,
        asize: f64,
        xo: f64,
        yo: f64,
        zo: f64,
        xx: f64,
        yx: f64,
        zx: f64,
        xy: f64,
        yy: f64,
        zy: f64,
        numr: i32,
    ) {
        let ds = 0.01_f64;
        let sc1 = 0.099_f64;
        let sc2 = 0.901_f64;
        let ten = 10.0_f64;
        self.ifg = 1;
        let xe = xo + asize * xx;
        let ye = yo + asize * yx;
        let ze = zo + asize * zx;
        self.vect3(xo, yo, zo, xe, ye, ze, self.wa, 0);
        let mut shl = 0.0_f64;
        let (lfont, hn, hf, gap, tic, wt) = (self.lfont, self.hn, self.hf, self.gap, self.tic, self.wt);
        let (mut xc, mut yc, mut zc) = (0.0_f64, 0.0_f64, 0.0_f64);
        let mut test;

        if astp != 0.0 {
            // ---- linear scale
            let mut n = ((amax - amin) / astp).round() as i32;
            let step = asize / f64::from(n);
            n += 1;
            test = 1.0_f64 / 1000.0;
            if (xx - 1.0).abs() < test {
                self.fvx = amin;
                self.dvx = (amax - amin) / asize;
                self.logx = 0;
                self.xmin = xo;
                self.xmax = xo + asize;
                self.xstp = step;
            } else if (yx - 1.0).abs() < test {
                self.fvy = amin;
                self.dvy = (amax - amin) / asize;
                self.logy = 0;
                self.ymin = yo;
                self.ymax = yo + asize;
                self.ystp = step;
            } else if (zx - 1.0).abs() < test {
                self.fvz = amin;
                self.dvz = (amax - amin) / asize;
                self.logz = 0;
                self.zmin = zo;
                self.zmax = zo + asize;
                self.zstp = step;
            }
            let (mut x, mut y, mut z) = (xo, yo, zo);
            let mut v = amin;
            let mut nscale = 0_i32;
            if astp.abs() < sc1 || astp.abs() > sc2 {
                nscale = astp.abs().r_log10().round() as i32;
                if nscale < 0 {
                    nscale -= 2;
                }
                nscale = 3 * (nscale / 3);
            }
            let scale = ten.r_powi(nscale);
            let mut iv = (astp / scale).round() as i32;
            let mut vv = astp / scale;
            test = 1.0 / 100.0;
            let mut ifracs = 0;
            if (vv - f64::from(iv)).abs() > test {
                ifracs = 1;
            }
            for i in 1..=n {
                if itic < 0 {
                    self.vect3(x, y, z, x - tic * xy, y - tic * yy, z - tic * zy, wt, 0);
                } else if itic > 0 {
                    self.vect3(x, y, z, x + tic * xy, y + tic * yy, z + tic * zy, wt, 0);
                }
                if nlabel != 0 && (i != 1 || (nonum != 1 && nonum != 3)) {
                    iv = (v / scale).round() as i32;
                    vv = v / scale;
                    let raw = if ifracs == 1 { f(vv, 6, 1) } else { fmt_i(i64::from(iv), 5) };
                    let (num, lnum) = stripv(raw.as_bytes());
                    let ww = self.txtlen(&num, lnum as i32, lfont, hn) / 2.0;
                    let mut wwt = ww;
                    if i == 1 && itic > 0 && nlabel < 0 {
                        wwt = ww / 2.0;
                    }
                    if i == 1 && itic < 0 && nlabel > 0 {
                        wwt = ww / 2.0;
                    }
                    if i == n && itic > 0 && nlabel < 0 {
                        wwt = 3.0 * ww / 2.0;
                    }
                    if i == n && itic < 0 && nlabel > 0 {
                        wwt = 3.0 * ww / 2.0;
                    }
                    if nlabel < 0 {
                        let mut sh = gap + hf * hn;
                        if itic < 0 {
                            sh = sh + tic - gap / 2.0;
                        }
                        if numr == 0 {
                            xc = x - sh * xy - wwt * xx;
                            yc = y - sh * yy - wwt * yx;
                            zc = z - sh * zy - wwt * zx;
                            if sh + hf * hn > shl {
                                shl = sh + hf * hn;
                            }
                        } else {
                            xc = x - sh * xy - hf * hn * xx / 2.0;
                            yc = y - sh * yy - hf * hn * yx / 2.0;
                            zc = z - sh * zy - hf * hn * zx / 2.0;
                            test = sh + 2.0 * ww + hn / 2.0;
                            if test > shl {
                                shl = test;
                            }
                        }
                    } else {
                        let mut sh = gap;
                        if itic > 0 {
                            sh = sh + tic - gap / 2.0;
                        }
                        if numr == 0 {
                            xc = x + sh * xy - wwt * xx;
                            yc = y + sh * yy - wwt * yx;
                            zc = z + sh * zy - wwt * zx;
                            test = sh + 2.0 * hf * hn;
                            if test > shl {
                                shl = test;
                            }
                        } else {
                            xc = x + (sh + 2.0 * ww) * xy - hf * hn * xx / 2.0;
                            yc = y + (sh + 2.0 * ww) * yy - hf * hn * yx / 2.0;
                            zc = z + (sh + 2.0 * ww) * zy - hf * hn * zx / 2.0;
                            test = sh + 2.0 * ww + hn / 2.0;
                            if test > shl {
                                shl = test;
                            }
                        }
                    }
                    if numr == 0 {
                        self.text3(&num, lnum as i32, lfont, hn, xc, yc, zc, xx, yx, zx, xy, yy, zy);
                    } else {
                        self.text3(&num, lnum as i32, lfont, hn, xc, yc, zc, -xy, -yy, -zy, xx, yx, zx);
                    }
                }
                x += step * xx;
                y += step * yx;
                z += step * zx;
                v += astp;
            }
            if nscale != 0 && nlabel != 0 && nonum != 2 && nonum != 3 {
                let num = exp_label("*10#EH.8<", nscale);
                let lnum = num.len() as i32;
                let www = self.txtlen(&num, lnum, lfont, hn);
                if nlabel < 0 {
                    if numr == 0 {
                        xc -= 6.0 * hn * xy / 5.0;
                        yc -= 6.0 * hn * yy / 5.0;
                        zc -= 6.0 * hn * zy / 5.0;
                    } else {
                        xc -= 6.0 * hn * xx / 5.0;
                        yc -= 6.0 * hn * yx / 5.0;
                        zc -= 6.0 * hn * zx / 5.0;
                    }
                } else if numr == 0 {
                    xc += 6.0 * hn * xy / 5.0;
                    yc += 6.0 * hn * yy / 5.0;
                    zc += 6.0 * hn * zy / 5.0;
                } else {
                    let dd = hf * hn / 2.0 + 6.0 * hn / 5.0;
                    xc = x + (www + gap) * xy - dd * xx - step * xx;
                    yc = y + (www + gap) * yy - dd * yx - step * yx;
                    zc = z + (www + gap) * zy - dd * zx - step * zx;
                }
                if numr == 0 {
                    self.text3(&num, lnum, lfont, hn, xc, yc, zc, xx, yx, zx, xy, yy, zy);
                } else {
                    self.text3(&num, lnum, lfont, hn, xc, yc, zc, -xy, -yy, -zy, xx, yx, zx);
                }
            }
        } else {
            // ---- log scale
            let mut origen = amin.r_log10();
            let mut i = origen as i32;
            if origen < 0.0 {
                i -= 1;
            }
            let mut v = origen - f64::from(i);
            if v < ds {
                v = 0.0;
            } else if v < SUBS[0] + ds {
                v = SUBS[0];
            } else if v < SUBS[3] + ds {
                v = SUBS[3];
            } else {
                i += 1;
                v = 0.0;
            }
            origen = f64::from(i) + v;
            let mut imin = i;
            if v > 0.0 {
                imin = i + 1;
            }
            let num = exp_label("10#EH.8<", imin);
            let mut ww = self.txtlen(&num, num.len() as i32, lfont, hn);
            let mut www = ww;
            let mut aend = amax.r_log10();
            let mut j = aend as i32;
            if aend < 0.0 {
                j -= 1;
            }
            v = aend - f64::from(j);
            if v < ds {
                v = 0.0;
            } else if v < SUBS[0] + ds {
                v = SUBS[0];
            } else if v < SUBS[3] + ds {
                v = SUBS[3];
            } else {
                j += 1;
                v = 0.0;
            }
            aend = f64::from(j) + v;
            let mut imax = j;
            if v > 0.0 {
                imax -= 1;
            }
            let num = exp_label("10#EH.8<", imax);
            ww = self.txtlen(&num, num.len() as i32, lfont, hn);
            if ww > www {
                www = ww;
            }
            let cycles = (aend - origen) / asize;
            test = 1.0 / 10.0;
            let room = if (xx * xy + yx * yy + zx * zy - 1.0).abs() < test { 6.0 * hn / 5.0 } else { 6.0 * www / 5.0 };
            let mut iskip = (room * cycles.abs()) as i32;
            iskip += 1;
            test = 1.0 / 1000.0;
            if (xx - 1.0).abs() < test {
                self.fvx = origen;
                self.dvx = cycles;
                self.logx = 1;
                self.xmin = xo;
                self.xmax = xo + asize;
            } else if (yx - 1.0).abs() < test {
                self.fvy = origen;
                self.dvy = cycles;
                self.logy = 1;
                self.ymin = yo;
                self.ymax = yo + asize;
            } else if (zx - 1.0).abs() < test {
                self.fvz = origen;
                self.dvz = cycles;
                self.logz = 1;
                self.zmin = zo;
                self.zmax = zo + asize;
            }
            let (xt, yt, zt) = if itic < 0 { (-tic * xy, -tic * yy, -tic * zy) } else { (tic * xy, tic * yy, tic * zy) };
            i = origen as i32;
            test = 1.0 / 10.0;
            if origen - f64::from(i) > test {
                i += 1;
            }
            let mut v10 = ten.r_powi(i);
            let mut x = xo + ((f64::from(i) - origen) / cycles) * xx;
            let mut y = yo + ((f64::from(i) - origen) / cycles) * yx;
            let mut z = zo + ((f64::from(i) - origen) / cycles) * zx;
            if itic != 0 {
                let test1 = 1.0_f64;
                if cycles <= test1 {
                    let mut k1 = 0usize;
                    test = 1.0 / 10.0;
                    if (origen - f64::from(i) + 1.0 - SUBS[0]).abs() < test {
                        k1 = 1;
                    }
                    if (origen - f64::from(i) + 1.0 - SUBS[3]).abs() < test {
                        k1 = 4;
                    }
                    if k1 != 0 {
                        for k in k1..=8 {
                            let sh = (SUBS[k - 1] - 1.0) / cycles;
                            let xs = x + sh * xx;
                            let ys = y + sh * yx;
                            let zs = z + sh * zx;
                            let (dx, dy, dz) = (xt / 2.0, yt / 2.0, zt / 2.0);
                            self.vect3(xs, ys, zs, xs + dx, ys + dy, zs + dz, wt, 0);
                        }
                    }
                }
            }
            loop {
                if itic != 0 {
                    self.vect3(x, y, z, x + xt, y + yt, z + zt, wt, 0);
                    test = 1.0 / 10.0;
                    if (aend - f64::from(i)).abs() >= test && cycles <= 1.0 {
                        let mut k2 = 8usize;
                        test = 1.0 / 10.0;
                        if (aend - f64::from(i) - SUBS[0]).abs() < test {
                            k2 = 1;
                        }
                        if (aend - f64::from(i) - SUBS[3]).abs() < test {
                            k2 = 4;
                        }
                        for k in 1..=k2 {
                            let sh = SUBS[k - 1] / cycles;
                            let xs = x + sh * xx;
                            let ys = y + sh * yx;
                            let zs = z + sh * zx;
                            let (dx, dy, dz) = (xt / 2.0, yt / 2.0, zt / 2.0);
                            self.vect3(xs, ys, zs, xs + dx, ys + dy, zs + dz, wt, 0);
                        }
                    }
                }
                if nlabel != 0 {
                    test = 1.0 / 1000.0;
                    if ((f64::from(i) - origen).abs() >= test || (nonum != 1 && nonum != 3))
                        && (v10.r_log10() < aend || (nonum != 2 && nonum != 3))
                    {
                        let iii = (f64::from(i) - origen) as i32;
                        if iii % iskip == 0 {
                            let num = exp_label("10#EH.8<", i);
                            let lnum = num.len() as i32;
                            let mut ww = self.txtlen(&num, lnum, lfont, hn) / 2.0;
                            if f64::from(i) == origen {
                                if itic == 0 || (itic < 0 && nlabel > 0) || (itic > 0 && nlabel < 0) {
                                    ww /= 2.0;
                                }
                            }
                            if nlabel < 0 {
                                let mut sh = gap + hf * hn;
                                if itic < 0 {
                                    sh = sh + tic - gap / 2.0;
                                }
                                if numr == 0 {
                                    xc = x - sh * xy - ww * xx;
                                    yc = y - sh * yy - ww * yx;
                                    zc = z - sh * zy - ww * zx;
                                    test = sh + hf * hn;
                                    if test > shl {
                                        shl = test;
                                    }
                                } else {
                                    xc = x - sh * xy - hf * hn * xx / 2.0;
                                    yc = y - sh * yy - hf * hn * yx / 2.0;
                                    zc = z - sh * zy - hf * hn * zx / 2.0;
                                    test = sh + www + hn / 2.0;
                                    if test > shl {
                                        shl = test;
                                    }
                                }
                            } else {
                                let mut sh = gap;
                                if itic > 0 {
                                    sh = sh + tic - gap / 2.0;
                                }
                                if numr == 0 {
                                    xc = x + sh * xy - ww * xx;
                                    yc = y + sh * yy - ww * yx;
                                    zc = z + sh * zy - ww * zx;
                                    test = sh + 2.0 * hf * hn;
                                    if test > shl {
                                        shl = test;
                                    }
                                } else {
                                    xc = x + (sh + www) * xy - hf * hn * xx / 2.0;
                                    yc = y + (sh + www) * yy - hf * hn * yx / 2.0;
                                    zc = z + (sh + www) * zy - hf * hn * zx / 2.0;
                                    test = sh + www + hn / 2.0;
                                    if test > shl {
                                        shl = test;
                                    }
                                }
                            }
                            if numr == 0 {
                                self.text3(&num, lnum, lfont, hn, xc, yc, zc, xx, yx, zx, xy, yy, zy);
                            } else {
                                self.text3(&num, lnum, lfont, hn, xc, yc, zc, -xy, -yy, -zy, xx, yx, zx);
                            }
                        }
                    }
                }
                if i >= j {
                    break;
                }
                x += (1.0 / cycles) * xx;
                y += (1.0 / cycles) * yx;
                z += (1.0 / cycles) * zx;
                i += 1;
                v10 *= 10.0;
            }
        }

        // ---- the centred axis label
        if nlabel != 0 {
            let dot = {
                let t: Vec<u8> = label.iter().rev().skip_while(|&&c| c == b' ').copied().collect();
                t == [b'.']
            };
            if !dot {
                let hl = self.hl;
                let mut ww = self.txtlen(label, nlabel.abs(), lfont, hl);
                ww = (asize - ww) / 2.0;
                let (xl, yl, zl);
                if nlabel < 0 {
                    let sh = shl + hf * hl;
                    xl = xo - sh * xy + ww * xx;
                    yl = yo - sh * yy + ww * yx;
                    zl = zo - sh * zy + ww * zx;
                } else {
                    let sh = shl;
                    xl = xo + sh * xy + ww * xx;
                    yl = yo + sh * yy + ww * yx;
                    zl = zo + sh * zy + ww * zx;
                }
                self.text3(label, nlabel.abs(), lfont, hl, xl, yl, zl, xx, yx, zx, xy, yy, zy);
            }
        }
    }
}
