// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the plot option of `subroutine covr`, l.150-505 (`nout <= 0`);
//   - `subroutine corr`, l.578-718 (plot branch); `subroutine covard`, l.720-937;
//   - `subroutine truncg`, l.939-1012; `subroutine plotit`, l.1014-1301;
//   - `subroutine matshd`, l.1303-1599; `function level`, l.1601-1619;
//   - `subroutine patlev`, l.1621-1647.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR's **plot option** (`nout <= 0`, gh:#1369): an ERRORR covariance tape in,
//! VIEWR's plot-command text out.
//!
//! Per case and reaction pair, `corr` reads the covariance block (through
//! [`crate::covr::tape`] and [`ErrorrCovarianceSection::to_dense`], upstream's
//! `covard`), takes `rsd = sqrt(diag)` and turns the matrix into correlations;
//! `truncg` cuts the low-energy end; `plotit` writes the two step curves (the
//! cross section, chi per eV or `rsd`, and `rsd` rotated by 90 degrees) and the
//! axis text; `matshd` floods the matrix into patterns of equal shade level and
//! writes each pattern's outline, then the legend. The module variables upstream
//! keeps across cases (`epmin`, which `corr` may raise; `einc`; `izero`) live in
//! [`PlotState`].
//!
//! Not translated: an MF=40 tape (`mfflg = -14`, `izap`), refused by name; and the
//! listing (`nsyso`), of which only the omitted-plot table is returned.

use petir::mathf::RealMath;

use crate::acer::fortran_fmt::fortran_e;
use crate::covr::covard::{expand_mt_pairs, ErrorrCovarianceSection};
use crate::covr::input::{CovarianceForm, PlotOptions, ReactionSelector};
use crate::covr::plot_label::{smilab, LabelContext};
use crate::covr::tape::{present_mts, read_covariance_rows, read_group_structure, read_vector, ErrorrTapeKind};
use crate::endf::tape::Tape;
use crate::viewr::fmt::{f as ff, i as fi};
use crate::NjoyError;

/// What the plot option wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct CovrPlot {
    /// The plot-command text for unit `nplot` (VIEWR's input).
    pub text: String,
    /// The pairs omitted from the plots: `(mat, mt, mat1, mt1, reason)`, reason
    /// `null`, `small` or `empty` (`covr.f90:396`).
    pub omitted: Vec<(i32, i32, i32, i32, &'static str)>,
}

/// The module variables of `covm` the plot path reads and writes.
#[derive(Debug, Clone)]
struct PlotState {
    icolor: i32,
    ndiv: i32,
    xlev: Vec<f64>,
    epmin: f64,
    irelco: i32,
    kind: ErrorrTapeKind,
    einc: f64,
    izero: i32,
    iverf: i32,
    out: String,
}

/// One `covard` call: the group bounds `y`, the two vectors and the dense matrix.
#[derive(Debug, Clone)]
struct Covard {
    ixmax: usize,
    y: Vec<f64>,
    xx: Vec<f64>,
    xy: Vec<f64>,
    cf: Vec<f64>,
}

/// What `corr` hands `covr` for one pair.
#[derive(Debug, Clone, Default)]
struct Corr {
    ismall: bool,
    ixmax: usize,
    x: Vec<f64>,
    y: Vec<f64>,
    xx: Vec<f64>,
    xy: Vec<f64>,
    cf: Vec<f64>,
    rsdx: Vec<f64>,
    rsdy: Vec<f64>,
}

fn e12(x: f64) -> String {
    fortran_e(x, 4, 12)
}

fn e13(x: f64) -> String {
    fortran_e(x, 4, 13)
}

impl PlotState {
    fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// `'(''-1 0 1. 1.'',5f8.3,''/'')'` (or with the leading `1 0`).
    fn frame(&mut self, lead: &str, v: [f64; 5]) {
        let mut s = lead.to_string();
        for x in v {
            s.push_str(&ff(x, 8, 3));
        }
        s.push('/');
        self.line(&s);
    }

    /// `covard` (`covr.f90:720-937`) for the plot option: the matrix of `(mat, mt)`
    /// against `(mat1, mt1)`, zeroed where a cross section is zero and divided by
    /// them for `irelco /= 1`. Sets `izero` (left as it was for a lumped
    /// placeholder, `nmt = 0`, as upstream does) and `einc`.
    fn covard(&mut self, tape: &Tape, mat: i32, mt: i32, mat1: i32, mt1: i32) -> Result<Covard, NjoyError> {
        let gs = read_group_structure(tape, mat)?;
        let ixmax = gs.ixmax;
        let mf35 = self.kind.mf35;
        let (xx, e1) = read_vector(tape, mat, mf35, mt, ixmax)?;
        let mut einc = e1;
        let xy = if mat == mat1 && mt == mt1 {
            xx.clone()
        } else {
            let (v, e2) = read_vector(tape, mat1, mf35, mt1, ixmax)?;
            einc = e2;
            v
        };
        if mf35 == 5 {
            self.einc = einc;
        }
        let mut cf = vec![0.0; ixmax * ixmax];
        if let Some(blocks) = read_covariance_rows(tape, self.kind, mat, mt, mat1, mt1)? {
            let sec = ErrorrCovarianceSection {
                ixmax,
                xx: xx.clone(),
                xy: xy.clone(),
                group_boundaries: gs.boundaries.clone(),
                blocks,
            };
            let form = if self.irelco != 1 { CovarianceForm::Absolute } else { CovarianceForm::Relative };
            let d = sec.to_dense(form)?;
            for (i, row) in cf.chunks_mut(ixmax).enumerate() {
                for (j, v) in row.iter_mut().enumerate() {
                    *v = d.matrix.get(i, j);
                }
            }
            self.izero = i32::from(!d.is_null);
        }
        Ok(Covard { ixmax, y: gs.boundaries, xx, xy, cf })
    }

    /// `corr` (`covr.f90:578-718`), plot branch: standard deviations and
    /// correlations, the `epmin` search, and `ismall`.
    fn corr(&mut self, tape: &Tape, mat: i32, mt: i32, mat1: i32, mt1: i32) -> Result<Corr, NjoyError> {
        let auto = mat == mat1 && mt == mt1;
        if !auto {
            self.covard(tape, mat, mt, mat1, mt1)?;
            if self.izero == 0 {
                return Ok(Corr::default());
            }
        }
        let a = self.covard(tape, mat1, mt1, mat1, mt1)?;
        let ixmax = a.ixmax;
        let x = a.y.clone();
        // the lowest energy that keeps 0.4 decades per inch over xsize = 4.25
        let mut epmn = 0.499999e-4_f64;
        let mut xcycle = 0.0_f64;
        while xcycle < 0.4 {
            epmn *= 2.0;
            xcycle = 4.25 / (x[ixmax] / epmn).r_log10();
        }
        if epmn > self.epmin {
            self.epmin = epmn;
        }
        let mut rsdy: Vec<f64> =
            (0..ixmax).map(|i| if a.cf[ixmax * i + i] > 0.0 { a.cf[ixmax * i + i].sqrt() } else { 0.0 }).collect();
        let mut rsdx = rsdy.clone();
        let mut c = a;
        if !auto {
            let b = self.covard(tape, mat, mt, mat, mt)?;
            if b.ixmax != ixmax {
                return Err(NjoyError::NotPorted("covr: materials with different group structures (icall restart)"));
            }
            rsdx = (0..ixmax).map(|i| b.cf[ixmax * i + i].sqrt()).collect();
            c = self.covard(tape, mat, mt, mat1, mt1)?;
        }
        let mut ismall = false;
        if self.izero != 0 {
            for i in 0..ixmax {
                for j in 0..ixmax {
                    let k = ixmax * i + j;
                    if c.cf[k] != 0.0 && rsdx[i] * rsdy[j] != 0.0 {
                        c.cf[k] /= rsdx[i] * rsdy[j];
                        if c.cf[k].abs() >= self.xlev[0] {
                            ismall = true;
                        }
                    } else {
                        c.cf[k] = 0.0;
                    }
                }
            }
        }
        if c.ixmax != ixmax {
            return Err(NjoyError::EndfParse("corr: group structures do not agree.".into()));
        }
        // rsdy is the column reaction's (mat1) and stays as computed above.
        rsdy.truncate(ixmax);
        Ok(Corr { ismall, ixmax, x, y: c.y, xx: c.xx, xy: c.xy, cf: c.cf, rsdx, rsdy })
    }
}

/// `truncg` (`covr.f90:939-1012`): the first plotted group `ixmin` (1-based), and
/// the low ends of `x` and `y` raised to a decade or to `epmin`.
///
/// # Errors
/// `bad data` when every group falls below the thresholds.
fn truncg(epmin: f64, x: &mut [f64], y: &mut [f64], xx: &[f64], xy: &[f64], ixmax: usize) -> Result<usize, NjoyError> {
    const EMIN: f64 = 0.9999e6;
    const XSLIM: f64 = 1.0e-4;
    let (mut rlimx, mut rlimy) = (0.0_f64, 0.0_f64);
    let (mut ethrx, mut ethry) = (x[0], y[0]);
    for i in 2..=ixmax {
        rlimx += xx[i - 2] * (x[i - 1] - x[i - 2]);
        rlimy += xy[i - 2] * (y[i - 1] - y[i - 2]);
        if xx[i - 2] <= 0.0 {
            ethrx = x[i - 1];
        }
        if xy[i - 2] <= 0.0 {
            ethry = y[i - 1];
        }
    }
    if rlimx != 0.0 && rlimy != 0.0 {
        rlimx = XSLIM * rlimx / (y[ixmax - 1] - ethry);
        rlimy = XSLIM * rlimy / (x[ixmax - 1] - ethrx);
    }
    let first = |e: &[f64], v: &[f64], rlim: f64| -> Result<usize, NjoyError> {
        let mut imin = 1;
        for i in 1..=ixmax {
            if e[i] > epmin && (v[i - 1] >= XSLIM || v[i - 1] >= rlim || e[i] > EMIN) {
                return Ok(imin);
            }
            imin = i + 1;
        }
        Err(NjoyError::EndfParse("truncg: bad data.".into()))
    };
    let mut ixmin = first(x, xx, rlimx)?;
    let iymin = first(y, xy, rlimy)?;
    if iymin < ixmin {
        ixmin = iymin;
    }
    // limit the bin width of a thermal group to one decade
    if ixmin == 1 && 10.0 * x[0] < x[1] && 10.0 * y[0] < y[1] {
        let ielo = (x[1] / 10.0).r_log10().round() as i32;
        if x[0] < 10.0_f64.r_powi(ielo) {
            x[0] = 10.0_f64.r_powi(ielo);
        }
        let ielo = (y[1] / 10.0).r_log10().round() as i32;
        if y[0] < 10.0_f64.r_powi(ielo) {
            y[0] = 10.0_f64.r_powi(ielo);
        }
    }
    if x[ixmin - 1] < epmin {
        x[ixmin - 1] = epmin;
    }
    if y[ixmin - 1] < epmin {
        y[ixmin - 1] = epmin;
    }
    Ok(ixmin)
}

/// `level` (`covr.f90:1601-1619`): the index of `c` on the scale `xlev`, signed.
fn level(c: f64, xlev: &[f64]) -> i32 {
    let mut ilev = 0;
    for (i, &t) in xlev.iter().enumerate() {
        ilev = i as i32 + 1;
        if c.abs() < t {
            break;
        }
    }
    if c < 0.0 {
        -ilev
    } else {
        ilev
    }
}

/// `patlev` (`covr.f90:1621-1647`): VIEWR's fill pattern for shade level `ilevel`.
fn patlev(ilevel: i32, icolor: i32, nlev: i32, ndiv: i32) -> i32 {
    let mut jpat = 0;
    if icolor == 0 {
        if ilevel > 1 {
            jpat = 20 - (nlev - ilevel) * 2 / ndiv;
        }
        if ilevel < -1 {
            jpat = 40 - (nlev + ilevel) * 2 / ndiv;
        }
    } else {
        // `scale=10./(nlev-1)` and `float()` are default (single precision) reals
        let scale = f64::from(10.0_f32 / (nlev - 1) as f32);
        if ilevel > 1 {
            let t = f64::from((nlev - ilevel) as f32);
            jpat = 50 - (scale * t).round() as i32;
        }
        if ilevel < -1 {
            let t = f64::from((nlev + ilevel) as f32);
            jpat = 60 - (scale * t).round() as i32;
        }
    }
    jpat
}

/// The axis limits `plotit` rounds a lower end to (`covr.f90:1098-1108`, `:1219-1229`):
/// a 1, 2 or 5 times a power of ten at or below `v`. `always` is the `x` form,
/// which sets the decade unconditionally; the `y` form tests `t - i > 0`.
fn lower_limit(v: f64, always: bool) -> f64 {
    let t = v.r_log10();
    let t1 = if t < 0.0 { t - 0.99999 } else { t + 0.00001 };
    let i = t1 as i32;
    let d = t - f64::from(i);
    let mut out = v;
    if always || d > 0.0 {
        out = 10.0_f64.r_powi(i);
    }
    if d > 2.0_f64.r_log10() {
        out = 2.0 * 10.0_f64.r_powi(i);
    }
    if d > 5.0_f64.r_log10() {
        out = 5.0 * 10.0_f64.r_powi(i);
    }
    out
}

/// The upper axis limit (`covr.f90:1109-1115`): 1, 2 or 5 times a power of ten above `v`.
fn upper_limit(v: f64) -> f64 {
    let mut t = v.r_log10();
    t -= t / 200.0;
    let i = t as i32;
    let d = t - f64::from(i);
    let mut out = 10.0_f64.r_powi(i + 1);
    if d < 5.0_f64.r_log10() {
        out = 5.0 * 10.0_f64.r_powi(i);
    }
    if d < 2.0_f64.r_log10() {
        out = 2.0 * 10.0_f64.r_powi(i);
    }
    out
}

/// What `plotit` needs of one pair.
struct PlotPair<'a> {
    mat: i32,
    mt: i32,
    mat1: i32,
    mt1: i32,
    iza: i32,
    iza1: i32,
    x: &'a [f64],
    y: &'a [f64],
    rsdx: Vec<f64>,
    rsdy: &'a [f64],
    xx: &'a [f64],
    ixmax: usize,
    cf: &'a [f64],
    ixmin: usize,
}

/// The `yrtest` clamp of a `% rsd` ordinate (`covr.f90:1149-1161`, `:1264-1276`).
fn clamp_rsd(yig: f64, yrange: f64, iwarn: &mut i32) -> f64 {
    let mut yyy = 100.0 * yig;
    if yrange > 10.0 && yyy < 0.1001 {
        if yyy != 0.0 {
            *iwarn = 1;
        }
        yyy = 0.1001;
    }
    if yrange > 10.0 && yyy > 99.9 {
        *iwarn = 1;
        yyy = 99.9;
    } else if yrange <= 10.0 && yyy > 59.999 {
        *iwarn = 1;
        yyy = 59.999;
    }
    yyy
}

/// The step curve of `v` over bounds `e` from 0-based index `ii` (`covr.f90:1086-1097`),
/// 1-based `(xig, yig)` with element 0 unused, and `(ymin, ymax)`.
fn step_curve(e: &[f64], v: &[f64], ii: usize, ixn: usize) -> (Vec<f64>, Vec<f64>, f64, f64) {
    let npts = 2 * (ixn - ii) + 2;
    let mut xig = vec![0.0; npts + 1];
    let mut yig = vec![0.0; npts + 1];
    xig[1] = e[ii];
    let (mut ymin, mut ymax) = (1.0e20_f64, -1.0e20_f64);
    let mut j = 0;
    for i in ii..ixn {
        j += 1;
        xig[2 * j] = e[i];
        yig[2 * j] = v[i];
        if v[i] > 0.0 && v[i] < ymin {
            ymin = v[i];
        }
        if v[i] > ymax {
            ymax = v[i];
        }
        xig[2 * j + 1] = e[i + 1];
        yig[2 * j + 1] = v[i];
    }
    xig[npts] = e[ixn];
    yig[npts] = 0.0;
    (xig, yig, ymin, ymax)
}

/// The frame and axis cards of one semi-log plot (`covr.f90:1119-1132`).
fn axis_cards(st: &mut PlotState, lead: &str, pos: [f64; 5], label: &str, yrange: f64, lo: f64, hi: f64) {
    st.frame(lead, pos);
    st.line(&format!("'{label}'/"));
    st.line("/");
    st.line(if yrange > 10.0 { "4/" } else { "3/" });
    st.line(&format!("{}{}/", e12(lo), e12(hi)));
    st.line("'.'/");
    st.line("/");
    st.line("'.'/");
    st.line("/");
    st.line("0 0 0 3 2/");
    st.line("0");
}

/// `plotit` (`covr.f90:1014-1301`). Returns `ishade`, the patterns with a fill.
fn plotit(st: &mut PlotState, p: PlotPair) -> i32 {
    const XSIZE: f64 = 5.0;
    const YSIZE: f64 = 3.38;
    let ixn = p.ixmax - p.ixmin + 1;
    let off = p.ixmax - ixn;
    let mut rsdx = p.rsdx.clone();
    let mut mtflg = 0;
    if p.mat == p.mat1 && p.mt == p.mt1 {
        mtflg = 1;
        for i in 0..ixn {
            rsdx[i] = if st.kind.mf35 != 5 { p.xx[i + off] } else { p.xx[i + off] / (p.y[i + 1] - p.y[i]) };
        }
    }
    let mut ii = 0;
    while ii < ixn && rsdx[ii] <= 0.0 {
        ii += 1;
    }
    let mut jj = 0;
    while jj < ixn && p.rsdy[jj] <= 0.0 {
        jj += 1;
    }
    if jj > ii {
        ii = jj;
    }
    let ctx = LabelContext { mf35: st.kind.mf35, mfflg: st.kind.mfflg, einc: st.einc, iverf: st.iverf };

    // the first semi-log plot: rsd, or the cross section / chi per eV
    let (xig, yig, ymin, ymax) = step_curve(p.x, &rsdx, ii, ixn);
    let npts = xig.len() - 1;
    let yrange = if ymin != 0.0 { ymax / ymin } else { 0.0 };
    let mut ydec = ymin.r_log10();
    if ydec < 0.0 {
        ydec -= 1.0;
    }
    let yymin = 10.0_f64.r_powi(ydec as i32);
    let xmin1 = xig[1];
    let xmin = lower_limit(xig[1], true);
    let xmax = upper_limit(xig[npts]);
    let label = smilab(p.iza, p.mt, 0, mtflg, &ctx);
    axis_cards(st, "1 0 1. 1.", [YSIZE - 0.75, XSIZE - 1.0, XSIZE, YSIZE, 0.0], &label, yrange, xmin, xmax);
    let mut iwarn = 0;
    for i in 1..=npts {
        let yyy = if mtflg == 0 {
            clamp_rsd(yig[i], yrange, &mut iwarn)
        } else {
            let mut yyy = yig[i];
            if yrange > 10.0 && yyy <= yymin {
                if yyy != 0.0 {
                    iwarn = 1;
                }
                yyy = 1.0001 * yymin;
            }
            yyy
        };
        st.line(&format!("{}{}/", e13(xig[i]), e13(yyy)));
    }
    st.line("/");

    // the second, rotated: rsd of the column reaction
    let (xig, yig, ymin2, ymax2) = step_curve(p.y, p.rsdy, ii, ixn);
    let yrange = if ymin2 != 0.0 { ymax2 / ymin2 } else { 0.0 };
    let ymin = lower_limit(if mtflg == 1 { xmin1 } else { xig[1] }, false);
    let ymax = upper_limit(xig[npts]);
    let label = smilab(p.iza1, p.mt1, 0, 0, &ctx);
    axis_cards(st, "-1 0 1. 1.", [YSIZE + 0.5, -0.25, XSIZE, YSIZE, 90.0], &label, yrange, ymin, ymax);
    for i in 1..=npts {
        let yyy = clamp_rsd(yig[i], yrange, &mut iwarn);
        st.line(&format!("{}{}/", e13(xig[i]), e13(yyy)));
    }
    st.line("/");

    // the text describing the axes
    let mut xpos = 1.9;
    let ypos = XSIZE + -0.75;
    st.frame("-1 0 1. 1.", [xpos, ypos, YSIZE, 1.0, 90.0]);
    let (a, b) = if mtflg == 0 {
        ("#H.75<o>rdinate scale is %", "#H.75<>relative standard deviation.")
    } else if p.mt == 251 {
        ("#H.75<o>rdinate scales are % relative", "#H.75<>standard deviation and mu-bar.")
    } else if (452..=456).contains(&p.mt) {
        ("#H.75<o>rdinate scales are % relative", "#H.75<>standard deviation and nu-bar.")
    } else if st.kind.mf35 == 5 {
        ("#H.75<o>rdinate scales are % standard", "#H.75<>deviation and spectrum/eV.")
    } else {
        ("#H.75<o>rdinate scales are % relative", "#H.75<>standard deviation and barns.")
    };
    st.line(&format!("'{a}'/"));
    st.line(&format!("'{b}'/"));
    st.line("0/");
    xpos += 0.75;
    st.frame("-1 0 1. 1.", [xpos, ypos, YSIZE, 1.0, 90.0]);
    st.line("'#H.75<a>bscissa scales are energy (e<v>).'/");
    st.line("/");
    if iwarn != 0 {
        xpos += 0.45;
        st.line("0/");
        st.frame("-1 0 1. 1.", [xpos, ypos, YSIZE, 1.0, 90.0]);
        st.line("'#H.75<w>arning:  some uncertainty'/");
        st.line("'#H.75<>data were suppressed.'/");
    }
    st.line("0/");
    matshd(st, &p, ixn, [xmin, xmax, ymin, ymax])
}

/// `matshd` (`covr.f90:1303-1599`): flood the correlation matrix into patterns of
/// one shade level, write each pattern's outline, then the legend. Returns the
/// number of filled patterns (`ishade`).
fn matshd(st: &mut PlotState, p: &PlotPair, ixnow: usize, lim: [f64; 4]) -> i32 {
    const XSIZE: f64 = 5.0;
    const YSIZE: f64 = 3.38;
    // `cofa`/`none`/`ntwo` (coefficients > 1 or > 2, `:1356-1388`) only feed `mess`.
    let nlev = st.xlev.len() as i32;
    let (ixmin, ixmax) = (p.ixmin, p.ixmax);
    let idx = |i: usize, j: usize| ixnow * (i - 1) + (j - 1);
    let mut ilcf = vec![0_i32; ixnow * ixnow];
    for (ii, i) in (ixmin..=ixmax).enumerate() {
        for (jj, j) in (ixmin..=ixmax).enumerate() {
            let cof = p.cf[ixmax * (i - 1) + (j - 1)].clamp(-1.0, 1.0);
            ilcf[ixnow * ii + jj] = level(cof, &st.xlev);
        }
    }
    let (x, y) = (p.x, p.y);
    let mut ixmip = vec![0usize; ixnow + 1];
    let mut ixmap = vec![0usize; ixnow + 1];
    let mut ishade = 0;
    let mut ipat = 0;
    let (mut inext, mut jnext) = (0usize, 1usize);
    'patterns: loop {
        ipat += 1;
        // find the next square not yet in a pattern
        loop {
            inext += 1;
            if inext > ixnow {
                jnext += 1;
            }
            if jnext > ixnow {
                break 'patterns;
            }
            if inext > ixnow {
                inext = 1;
            }
            if ilcf[idx(inext, jnext)] != 0 {
                break;
            }
        }
        let mut il = inext;
        let mut jl = jnext;
        let ilevel = ilcf[idx(il, jl)];
        ilcf[idx(il, jl)] = 0;
        ixmip[jl] = il;
        ixmap[jl] = il;
        let jfirst = jl;
        let mut jlast = jl;
        // its extent in its first row
        loop {
            il += 1;
            if il > ixnow || ilcf[idx(il, jl)] != ilevel {
                break;
            }
            ilcf[idx(il, jl)] = 0;
            ixmap[jl] = il;
            inext = il;
        }
        // the rows above, each started from a square under the row below
        loop {
            jl += 1;
            if jl > ixnow {
                break;
            }
            let (i1, i2) = (ixmip[jl - 1], ixmap[jl - 1]);
            let Some(istart) = (i1..=i2).find(|&i| ilcf[idx(i, jl)] == ilevel) else { break };
            jlast = jl;
            ixmip[jl] = istart;
            ixmap[jl] = istart;
            ilcf[idx(istart, jl)] = 0;
            for i in 1..istart {
                if ilcf[idx(istart - i, jl)] != ilevel {
                    break;
                }
                ixmip[jl] = istart - i;
                ilcf[idx(istart - i, jl)] = 0;
            }
            for i in 1..=(ixnow - istart) {
                if ilcf[idx(istart + i, jl)] != ilevel {
                    break;
                }
                ixmap[jl] = istart + i;
                ilcf[idx(istart + i, jl)] = 0;
            }
        }
        // the boundary of this pattern
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for jl in jfirst..=jlast {
            let ilast = ixmap[jl];
            pts.push((x[ilast], y[jl - 1]));
            pts.push((x[ilast], y[jl]));
        }
        for jl in (jfirst..=jlast).rev() {
            let ifirst = ixmip[jl];
            pts.push((x[ifirst - 1], y[jl]));
            pts.push((x[ifirst - 1], y[jl - 1]));
        }
        if ipat == 1 {
            st.frame("-1 0 1. 1.", [YSIZE + -0.75, -0.25, XSIZE, XSIZE, 0.0]);
            st.line("/");
            st.line("/");
            st.line("4 0 0/");
            st.line(&format!("{}{}/", e12(lim[0]), e12(lim[1])));
            st.line("/");
            st.line(&format!("{}{}/", e12(lim[2]), e12(lim[3])));
            st.line("/");
        } else {
            st.line("2/");
        }
        st.line("/");
        let jpat = patlev(ilevel, st.icolor, nlev, st.ndiv);
        if jpat != 0 {
            ishade += 1;
        }
        st.line(&format!("0 0 0 0 0{}/", fi(i64::from(jpat), 3)));
        st.line("0");
        for (a, b) in pts {
            st.line(&format!("{}{}/", e13(a), e13(b)));
        }
        st.line("/");
    }

    // the legend
    let mut xpos = YSIZE - 0.75 + XSIZE + 0.60;
    let mut ypos = 0.75;
    st.frame("-1 0 1. 1.", [xpos, ypos, xpos, 1.0, 90.0]);
    st.line("'<c>orrelation <m>atrix'/");
    st.line("/");
    st.line("0/");
    xpos += 1.75;
    ypos = 0.625;
    for (part, ytop, ystep) in [(1, 1.0, 0.2_f32), (-1, -1.0, -0.2_f32)] {
        if part == -1 {
            ypos += 1.375;
        }
        st.frame("-1 0 1. 1.", [xpos, ypos, 1.75, 2.50, 90.0]);
        st.line("/");
        st.line("/");
        st.line("1 0 0/");
        st.line(&format!("{}{}{}/", e12(0.0), e12(1.0), e12(1.0)));
        st.line("/");
        st.line(&format!("{}{}{}/", e12(0.0), e12(ytop), e12(f64::from(ystep))));
        st.line("'.'/");
        st.line("/");
        let mut tlow = 0.0_f64;
        for il in 1..=nlev {
            let ilevel = part * il;
            let mut jpat = patlev(ilevel, st.icolor, nlev, st.ndiv);
            if part == 1 && st.icolor == 0 && jpat != 0 {
                jpat += 10;
            }
            st.line(&format!("0 0 0 0 0{}/", fi(i64::from(jpat), 3)));
            st.line("0");
            let thi = f64::from(part) * st.xlev[(il - 1) as usize];
            for (a, b) in [(0, tlow), (1, tlow), (1, thi), (0, thi), (0, tlow)] {
                st.line(&format!("{a}. {}/", ff(b, 6, 3)));
            }
            st.line("/");
            if il < nlev {
                st.line("2/");
                st.line("/");
            }
            tlow = thi;
        }
    }
    ishade
}

/// COVR's plot option on an ERRORR covariance tape (`covr.f90:150-505`,
/// `nout <= 0`): the plot-command text VIEWR reads.
///
/// `icolor` is card 2; `opts` holds cards 2', 2a and 3a; `cases` card 4.
///
/// # Errors
/// An illegal tape (`mfflg` not -11, -12 or -14), a missing section, `truncg`'s
/// `bad data`; [`NjoyError::NotPorted`] for an MF=40 tape (`mfflg = -14`).
pub fn run_plot(tape: &Tape, opts: &PlotOptions, cases: &[ReactionSelector]) -> Result<CovrPlot, NjoyError> {
    let kind = ErrorrTapeKind::from_tape(tape)?;
    if kind.mfflg == -14 {
        return Err(NjoyError::NotPorted("covr plot of an MF=40 tape (izap)"));
    }
    let icolor = opts.color.to_icolor();
    let mut st = PlotState {
        icolor,
        ndiv: if opts.ndiv == 0 { 1 } else { opts.ndiv },
        xlev: opts.shade_levels()?,
        epmin: opts.epmin_scaled(),
        irelco: opts.covariance_form.to_irelco(),
        kind,
        einc: 0.0,
        izero: 0,
        iverf: 0,
        out: String::new(),
    };
    st.line(&format!("1 2 .22{}/", fi(i64::from(icolor), 3)));
    let mut omitted = Vec::new();
    for case in cases {
        let mat = case.mat;
        let gs = read_group_structure(tape, mat)?;
        st.iverf = gs.iverf;
        let iza = gs.iza;
        let iza1 = if case.mat1 != mat && case.mat1 > 0 { read_group_structure(tape, case.mat1)?.iza } else { iza };
        let pairs: Vec<(i32, i32, i32)> = if case.mt <= 0 {
            let mstrip = [-case.mt, -case.mat1, -case.mt1];
            expand_mt_pairs(&present_mts(tape, mat, kind.mf35), mstrip).into_iter().map(|(a, b)| (a, 0, b)).collect()
        } else {
            vec![(case.mt, case.mat1, case.mt1)]
        };
        for (mt, m1, t1) in pairs {
            let mat1 = if m1 <= 0 { mat } else { m1 };
            let mt1 = if t1 == 0 { mt } else { t1 };
            let mut c = st.corr(tape, mat, mt, mat1, mt1)?;
            if c.ismall {
                let ixmin = truncg(st.epmin, &mut c.x, &mut c.y, &c.xx, &c.xy, c.ixmax)?;
                if st.izero == 0 {
                    continue;
                }
                let o = ixmin - 1;
                let pair = PlotPair {
                    mat,
                    mt,
                    mat1,
                    mt1,
                    iza,
                    iza1,
                    x: &c.x[o..],
                    y: &c.y[o..],
                    rsdx: c.rsdx[o..].to_vec(),
                    rsdy: &c.rsdy[o..],
                    xx: &c.xx,
                    ixmax: c.ixmax,
                    cf: &c.cf,
                    ixmin,
                };
                if plotit(&mut st, pair) != 0 {
                    continue;
                }
                st.izero = 2;
            }
            let reason = ["null", "small", "empty"][st.izero.clamp(0, 2) as usize];
            omitted.push((mat, mt, mat1, mt1, reason));
        }
    }
    st.line("99/");
    Ok(CovrPlot { text: st.out, omitted })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `level` and `patlev` on the default six levels (`xlev` 0.001 ... 1.001).
    #[test]
    fn levels_and_patterns() {
        let xlev = [0.001, 0.1, 0.2, 0.3, 0.6, 1.001];
        assert_eq!(level(0.0005, &xlev), 1);
        assert_eq!(level(-0.25, &xlev), -4);
        assert_eq!(level(1.0, &xlev), 6);
        assert_eq!(patlev(6, 1, 6, 1), 50);
        assert_eq!(patlev(2, 1, 6, 1), 42);
        assert_eq!(patlev(-3, 1, 6, 1), 54);
        assert_eq!(patlev(1, 1, 6, 1), 0);
        assert_eq!(patlev(6, 0, 6, 1), 20);
        assert_eq!(patlev(-2, 0, 6, 1), 32);
    }

    /// The axis limits of NJOY2016's deck 34 plot file (`2.0000E+02 2.0000E+07` for a
    /// curve from 454 eV to 2e7 eV).
    #[test]
    fn axis_limits() {
        assert_eq!(lower_limit(454.0, true), 200.0);
        assert_eq!(upper_limit(2.0e7), 2.0e7);
        assert_eq!(lower_limit(1.0e3, true), 1.0e3);
        assert_eq!(lower_limit(0.3, false), 0.2);
    }
}
