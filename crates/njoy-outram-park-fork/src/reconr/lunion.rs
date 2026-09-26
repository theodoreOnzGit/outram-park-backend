// Ported from NJOY2016 `src/reconr.f90` (`lunion`, lines 1771-2238), git
// commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da. NJOY2016 is under a
// modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative
// file is distributed under GPL-3.0-only. Modified, non-LANL version, not
// endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `lunion`: RECONR's **union energy grid** -- the one grid every reaction is
//! later evaluated on (GitHub #340).
//!
//! Starting from the resonance/range nodes, each cross-section section is
//! folded in turn onto the current grid, and every panel of the merged grid
//! is linearised against that section's own interpolation law:
//!
//! - below `elim = min(0.99 MeV, eresr)` a panel straddling a **decade point**
//!   (`10^p`, `5·10^(p-1)`, `2·10^(p-1)`, `p = -4..5`) or 0.0253 eV is split
//!   there;
//! - below `elim` a lin-lin panel wider than the ratio
//!   `stpmax = 1 + sqrt(5.3·err)` is bisected at 7-figure midpoints;
//! - a non-lin-lin panel is bisected until lin-lin reproduces its law to
//!   `err` (`err/5` below 0.4999 eV);
//! - discontinuities and histogram steps are shaded one unit in the 7th
//!   figure.
//!
//! Finally duplicates and the points sitting exactly on the resonance-range
//! boundaries are dropped (the shaded nodes either side remain).
//!
//! Upstream pages long records (`moreio`) and signs the grid entries it adds
//! by linearisation (negative). Records are held whole here, so the paging
//! offset `lr` is always 0; the sign is kept because the final pass counts
//! it and a point's sign travels with it through later sections.

use crate::endf::interp::{terp1, IntLaw};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const ELOW: f64 = 1.0e-5;
const UP: f64 = 1.001;
const DN: f64 = 0.999;
const THERM: f64 = 0.0253;
const SMALL: f64 = 1.0e-9;
const SSMALL: f64 = 1.0e-30;
const STPMIN: f64 = 1.001;
const OVFACT: f64 = 5.3;
const TRANGE: f64 = 0.4999;
const EMAX: f64 = 19.0e6;
const NDIM: usize = 50;

/// One TAB1 folded into the union: its interpolation table, its points, and
/// whether upstream skips the pseudo-threshold scan for it (MT=2, 18, 19,
/// 102).
#[derive(Debug, Clone)]
pub struct UnionInput {
    /// `(NBT, INT)` regions.
    pub interp: Vec<(u32, u32)>,
    /// `(E, y)` points, after `lunion`'s threshold raise.
    pub pairs: Vec<(f64, f64)>,
    /// MT=2, 18, 19 or 102: no pseudo-threshold skipping.
    pub keep_leading_zeros: bool,
}

/// The resonance-range boundaries the final pass strips.
#[derive(Debug, Clone, Copy)]
pub struct RangeBounds {
    /// `eresl`, `eresr`, `eresu`, `eresm`, `eresh`.
    pub eres: [f64; 5],
    /// `min(0.99e6, eresr)`.
    pub elim: f64,
}

fn int_law(i: u32) -> IntLaw {
    match i {
        1 => IntLaw::Histogram,
        3 => IntLaw::LinLog,
        4 => IntLaw::LogLin,
        5 => IntLaw::LogLog,
        _ => IntLaw::LinLin,
    }
}

fn terp(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, law: u32) -> f64 {
    terp1(x1, y1, x2, y2, x, int_law(law)).unwrap_or(y1)
}

/// Build the union grid. `nodes` are `rdfil2`'s, in the order upstream adds
/// them (it does not sort them: the first section's walk does).
///
/// # Errors
/// [`NjoyError::EndfParse`] when the linearisation stack exceeds upstream's
/// `ndim = 50`.
pub fn lunion(
    nodes: &[f64],
    inputs: &[UnionInput],
    err: f64,
    bounds: RangeBounds,
) -> Result<Vec<f64>, NjoyError> {
    let stpmax = 1.0 + (OVFACT * err).sqrt();
    let elim = bounds.elim;
    let mut old: Vec<f64> = nodes.to_vec();

    for inp in inputs {
        let n2h = inp.pairs.len();
        if n2h == 0 {
            continue;
        }
        let mut x: Vec<f64> = inp.pairs.iter().map(|p| p.0).collect();
        let y: Vec<f64> = inp.pairs.iter().map(|p| p.1).collect();
        let nbt = |jr: usize| inp.interp.get(jr - 1).map_or(n2h, |r| r.0 as usize);
        let itp = |jr: usize| inp.interp.get(jr - 1).map_or(2, |r| r.1);
        let ngo = old.len();
        let get_old = |ig: usize| old[ig - 1];
        let mut new: Vec<f64> = Vec::with_capacity(ngo + n2h);

        // 1-based pair index `ir`, region `jr`.
        let mut ir: usize = 0;
        let mut jr: usize = 1;
        let mut nbta: usize;
        let mut inta: u32;
        let mut ig: usize = 1;
        let mut eg: f64;
        let mut er: f64;
        let mut sr: f64;

        // 205: first point, skipping a pseudo-threshold run of zeros.
        loop {
            ir += 1;
            if ir > nbt(jr) {
                jr += 1;
            }
            nbta = nbt(jr);
            inta = itp(jr);
            er = x[ir - 1];
            sr = y[ir - 1];
            eg = er;
            if inp.keep_leading_zeros || ir >= n2h {
                break;
            }
            let (ernext, srnext) = (x[ir], y[ir]);
            if sr < SSMALL && srnext < SSMALL && ir < n2h - 1 {
                continue;
            }
            // Initial discontinuity.
            if (er - ernext).abs() <= SMALL * er {
                er = sigfig(er, 7, 0);
                x[ir] = sigfig(ernext, 7, 1);
            }
            break;
        }
        // 210: carry grid points below the first energy.
        let mut enl: f64;
        loop {
            if ig > ngo {
                break;
            }
            eg = get_old(ig);
            if eg.abs() >= er * (1.0 - SMALL) {
                break;
            }
            new.push(eg);
            ig += 1;
        }
        // 220
        let test = sigfig(ELOW, 7, 1);
        if er >= test {
            if (er - sigfig(eg.abs(), 7, -1)) > 1.0e-8 * er {
                er = eg.abs();
            } else if sr != 0.0 {
                new.push(sigfig(er, 7, 0));
                er = sigfig(er, 7, 1);
            }
        }
        // 225
        enl = er;
        let mut snl = sr;
        let mut erl = er;
        let mut srl = sr;
        if ig < ngo && eg.abs() <= er * (1.0 + SMALL) {
            ig += 1;
            eg = get_old(ig);
        }
        ir += 1;
        if ir <= n2h {
            er = x[ir - 1];
            sr = y[ir - 1];
        }
        if ir > nbta {
            jr += 1;
            nbta = nbt(jr);
            inta = itp(jr);
        }
        if ir < nbta && ir < n2h {
            let ernext = x[ir];
            if (er - ernext).abs() <= SMALL * er {
                er = sigfig(er, 7, -1);
                x[ir] = sigfig(ernext, 7, 1);
            }
        }
        let mut et = 0.0;
        let mut en: f64 = enl;
        let mut sn: f64;
        let mut intn: u32;

        'outer: loop {
            // 240
            if ir > n2h {
                break 'outer; // 350
            }
            if ig < ngo && eg.abs() < er * (1.0 - SMALL) {
                en = eg.abs();
                sn = terp(erl.abs(), srl, er, sr, en, inta);
                if eg < 0.0 {
                    en = -en;
                }
                intn = inta;
                ig += 1;
                eg = get_old(ig);
            } else {
                // 250
                let mut take_step = false;
                if inta == 1 && ir < n2h && er >= (1.0 + SMALL) * enl {
                    if (et - enl).abs() < SMALL * et {
                        er = sigfig(er, 7, 1); // 252
                    } else {
                        take_step = true;
                    }
                }
                if take_step {
                    et = sigfig(er, 7, -1);
                    en = et;
                    sn = snl;
                    intn = inta;
                    erl = en;
                    srl = sr;
                } else {
                    // 255
                    en = er;
                    sn = sr;
                    intn = inta;
                    erl = er;
                    srl = sr;
                    // 260
                    loop {
                        ir += 1;
                        if ir > n2h {
                            break;
                        }
                        er = x[ir - 1];
                        sr = y[ir - 1];
                        if ir > nbta {
                            jr += 1;
                            nbta = nbt(jr);
                            inta = itp(jr);
                        }
                        if ir >= n2h {
                            break;
                        }
                        let (ernext, srnext) = (x[ir], y[ir]);
                        if (ernext - er).abs() > SMALL * er {
                            break;
                        }
                        if (srnext - sr).abs() < SMALL * sr {
                            continue;
                        }
                        er = sigfig(er, 7, -1);
                        x[ir] = sigfig(ernext, 7, 1);
                        break;
                    }
                    // 280
                    if eg.abs() <= erl * (1.0 + SMALL) {
                        ig += 1;
                        if ig <= ngo {
                            eg = get_old(ig);
                        }
                    }
                }
            }
            // 300
            if en.abs() <= enl.abs() * (1.0 + SMALL) {
                continue 'outer;
            }
            // Linearise the panel (|enl|, |en|).
            let mut xs = [0.0f64; NDIM + 1];
            let mut ys = [0.0f64; NDIM + 1];
            let mut i = 2usize;
            xs[2] = enl.abs();
            ys[2] = snl;
            xs[1] = en.abs();
            ys[1] = sn;
            loop {
                // 310
                let mut split: Option<(f64, f64)> = None;
                let accept;
                if xs[i - 1] / xs[i] < STPMIN {
                    accept = true;
                } else {
                    let mut decade: Option<f64> = None;
                    if xs[i] <= elim {
                        'dec: for ipwr in -4..=5 {
                            let mut xm = 10f64.powi(ipwr);
                            let inside = |xm: f64| xs[i - 1] > UP * xm && xs[i] < DN * xm;
                            if inside(xm) {
                                decade = Some(xm);
                                break 'dec;
                            }
                            xm = sigfig(xm / 2.0, 1, 0);
                            if inside(xm) {
                                decade = Some(xm);
                                break 'dec;
                            }
                            xm = sigfig(4.0 * xm / 10.0, 1, 0);
                            if inside(xm) {
                                decade = Some(xm);
                                break 'dec;
                            }
                        }
                        if decade.is_none() && xs[i - 1] > UP * THERM && xs[i] < DN * THERM {
                            decade = Some(THERM);
                        }
                    }
                    if let Some(xm) = decade {
                        // 315
                        split = Some((xm, terp(xs[i], ys[i], xs[i - 1], ys[i - 1], xm, intn)));
                        accept = false;
                    } else if intn <= 2 {
                        // 320
                        if xs[i] > elim || xs[i - 1] / xs[i] <= stpmax {
                            accept = true;
                        } else {
                            let xm = sigfig((xs[i - 1] + xs[i]) / 2.0, 7, 0);
                            split = Some((xm, terp(xs[i], ys[i], xs[i - 1], ys[i - 1], xm, 2)));
                            accept = false;
                        }
                    } else {
                        // 322
                        let xm = sigfig((xs[i - 1] + xs[i]) / 2.0, 7, 0);
                        let ym = terp(xs[i], ys[i], xs[i - 1], ys[i - 1], xm, intn);
                        let yl = terp(xs[i], ys[i], xs[i - 1], ys[i - 1], xm, 2);
                        let errn = if xs[i - 1] < TRANGE { err / 5.0 } else { err };
                        let t = errn * ym + SSMALL;
                        if (ym - yl).abs() < t {
                            accept = true;
                        } else {
                            split = Some((xm, ym));
                            accept = false;
                        }
                    }
                }
                if accept {
                    // 325
                    let mut ent = -xs[i];
                    if (enl.abs() - xs[i]).abs() < SMALL * xs[i] {
                        ent = enl;
                    }
                    new.push(ent);
                    i -= 1;
                    if i > 1 {
                        continue;
                    }
                    enl = en;
                    snl = sn;
                    break;
                }
                let (xm, ym) = split.expect("split point");
                // 330
                i += 1;
                if i > NDIM {
                    return Err(NjoyError::EndfParse("lunion: exceeded stack.".into()));
                }
                xs[i] = xs[i - 1];
                ys[i] = ys[i - 1];
                xs[i - 1] = xm;
                ys[i - 1] = ym;
            }
        }
        // 350
        new.push(en);
        old = new;
    }

    // 410: drop duplicates and the exact range boundaries.
    let ngo = old.len();
    let mut out = Vec::with_capacity(ngo);
    let mut egl = 0.0f64;
    for (k, &e) in old.iter().enumerate() {
        let ig = k + 1;
        if (e.abs() - egl).abs() <= SMALL * egl {
            continue;
        }
        let eg = e.abs();
        let keep_always = ig == 1 || ig == ngo || eg > EMAX;
        if !keep_always
            && bounds
                .eres
                .iter()
                .any(|&b| b > 0.0 && b < 9.0e9 && (eg - b).abs() <= SMALL * b)
        {
            continue;
        }
        egl = eg;
        out.push(eg);
    }
    Ok(out)
}
