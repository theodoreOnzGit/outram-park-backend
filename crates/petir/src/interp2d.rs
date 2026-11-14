//! Bilinear and bicubic interpolation on a rectangular grid, ported from
//! GSL 2.8 (gh:#1341).
//!
//! Upstream: GSL 2.8, `ampl/gsl` commit `cf180cd7`, `interpolation/interp2d.c`,
//! `bilinear.c` and `bicubic.c`; Copyright 2012 David Zaslavsky
//! (GPL-3.0-or-later). The bicubic polynomial sums (`bicubic_eval`,
//! `_deriv_x`, `_deriv_y`, `_deriv_xx`, `_deriv_xy`, `_deriv_yy`) live in
//! [`crate::interp2d_bicubic`], **generated from `bicubic.c` by
//! `reference-data/gsl/gen_bicubic_rs.py`** rather than typed. Research,
//! education and V&V only.
//!
//! # Layout
//!
//! `za` holds `ysize` rows of `xsize` values: `z(x_i, y_j) = za[j * xsize + i]`
//! (GSL's `IDX2D`). `xa` and `ya` are strictly increasing.
//!
//! # Bicubic
//!
//! Needs `xsize, ysize >= 4`. At construction the first and mixed derivatives
//! at every grid point come from natural cubic splines through each row and
//! column ([`crate::interp::Interp`], GSL's `gsl_interp_cspline`), exactly as
//! `bicubic_init`: `zx` from the rows, `zy` from the columns, `zxy` from the
//! rows of `zy`.
//!
//! # `spline2d.c` is not ported
//!
//! `gsl_spline2d` is `gsl_interp2d` plus owned copies of `x`, `y`, `z` and the
//! `set/get/idx` helpers; [`Interp2d`] owns its data (checked 2026-11-13
//! against `spline2d.c`).
//!
//! # Differences from upstream
//!
//! - Out-of-range `x` or `y` is [`PetirError::Domain`] (`GSL_EDOM`), also for
//!   `NaN` (GSL's comparison lets `NaN` through); [`Interp2d::eval_extrap`]
//!   is GSL's `gsl_interp2d_eval_extrap`.
//! - No accelerator (GSL's plain `gsl_interp_bsearch` path is what is ported).
//! - Every array access is a checked `get`.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

use crate::interp::{Interp, InterpMethod};
use crate::zip::zip2;
use crate::{PetirError, Result};

/// Which 2-D scheme an [`Interp2d`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp2dKind {
    /// GSL `gsl_interp2d_bilinear` (`min_size` 2).
    Bilinear,
    /// GSL `gsl_interp2d_bicubic` (`min_size` 4).
    Bicubic,
}

impl Interp2dKind {
    /// The fewest points per axis (`gsl_interp2d_type_min_size`).
    pub fn min_size(self) -> usize {
        match self {
            Interp2dKind::Bilinear => 2,
            Interp2dKind::Bicubic => 4,
        }
    }
}

/// Everything one grid cell contributes to a bicubic evaluation: the common
/// set-up of the six `bicubic_*` functions, with the powers of `t` and `u`
/// all formed (each function used a subset, with identical formulas).
#[derive(Debug, Clone, Copy)]
pub(crate) struct BicubicCell {
    pub(crate) t0: f64,
    pub(crate) t1: f64,
    pub(crate) t2: f64,
    pub(crate) t3: f64,
    pub(crate) u0: f64,
    pub(crate) u1: f64,
    pub(crate) u2: f64,
    pub(crate) u3: f64,
    pub(crate) dt: f64,
    pub(crate) du: f64,
    pub(crate) zminmin: f64,
    pub(crate) zminmax: f64,
    pub(crate) zmaxmin: f64,
    pub(crate) zmaxmax: f64,
    pub(crate) zxminmin: f64,
    pub(crate) zxminmax: f64,
    pub(crate) zxmaxmin: f64,
    pub(crate) zxmaxmax: f64,
    pub(crate) zyminmin: f64,
    pub(crate) zyminmax: f64,
    pub(crate) zymaxmin: f64,
    pub(crate) zymaxmax: f64,
    pub(crate) zxyminmin: f64,
    pub(crate) zxyminmax: f64,
    pub(crate) zxymaxmin: f64,
    pub(crate) zxymaxmax: f64,
}

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

/// `gsl_interp_bsearch(a, x, 0, n - 1)`.
fn bsearch(a: &[f64], x: f64) -> usize {
    let mut ilo = 0usize;
    let mut ihi = a.len().saturating_sub(1);
    while ihi > ilo + 1 {
        let i = (ihi + ilo) / 2;
        if at(a, i) > x {
            ihi = i;
        } else {
            ilo = i;
        }
    }
    ilo
}

/// A 2-D interpolant over a rectangular grid.
///
/// # Example
///
/// ```
/// use petir::interp2d::{Interp2d, Interp2dKind};
/// // z = 2x + 3y on a 2 x 2 grid: bilinear reproduces it exactly.
/// let x = [0.0, 1.0];
/// let y = [0.0, 2.0];
/// let z = [0.0, 2.0, 6.0, 8.0]; // z[j * 2 + i]
/// let s = Interp2d::new(Interp2dKind::Bilinear, &x, &y, &z).unwrap();
/// assert!((s.eval(0.25, 1.0).unwrap() - 3.5).abs() < 1e-14);
/// assert!((s.eval_deriv_x(0.25, 1.0).unwrap() - 2.0).abs() < 1e-14);
/// assert!((s.eval_deriv_y(0.25, 1.0).unwrap() - 3.0).abs() < 1e-14);
/// assert!(s.eval(2.0, 1.0).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct Interp2d {
    kind: Interp2dKind,
    xa: Vec<f64>,
    ya: Vec<f64>,
    za: Vec<f64>,
    zx: Vec<f64>,
    zy: Vec<f64>,
    zxy: Vec<f64>,
}

impl Interp2d {
    /// `gsl_interp2d_alloc` + `gsl_interp2d_init` (+ `bicubic_init`).
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if an axis has fewer points than
    ///   [`Interp2dKind::min_size`].
    /// - [`PetirError::LengthMismatch`] if `za.len() != xa.len() * ya.len()`.
    /// - [`PetirError::Domain`] if `xa` or `ya` is not strictly increasing.
    pub fn new(kind: Interp2dKind, xa: &[f64], ya: &[f64], za: &[f64]) -> Result<Self> {
        let (xsize, ysize) = (xa.len(), ya.len());
        if xsize < kind.min_size() || ysize < kind.min_size() {
            return Err(PetirError::Invalid);
        }
        if za.len() != xsize * ysize {
            return Err(PetirError::LengthMismatch {
                expected: xsize * ysize,
                found: za.len(),
            });
        }
        for axis in [xa, ya] {
            for (&lo, &hi) in zip2(axis.iter(), axis.iter().skip(1)) {
                if !(lo < hi) {
                    return Err(PetirError::Domain);
                }
            }
        }
        let mut s = Self {
            kind,
            xa: xa.to_vec(),
            ya: ya.to_vec(),
            za: za.to_vec(),
            zx: Vec::new(),
            zy: Vec::new(),
            zxy: Vec::new(),
        };
        if kind == Interp2dKind::Bicubic {
            s.bicubic_init()?;
        }
        Ok(s)
    }

    /// `IDX2D(i, j)`.
    fn idx(&self, i: usize, j: usize) -> usize {
        j * self.xa.len() + i
    }

    /// `bicubic_init` (bicubic.c:99-175): the three derivative grids.
    fn bicubic_init(&mut self) -> Result<()> {
        let (xsize, ysize) = (self.xa.len(), self.ya.len());
        let mut zx = vec![0.0_f64; xsize * ysize];
        let mut zy = vec![0.0_f64; xsize * ysize];
        let mut zxy = vec![0.0_f64; xsize * ysize];

        // zx: natural spline through every row (x varies), derivative at x_i.
        for j in 0..ysize {
            let col: Vec<f64> = (0..xsize).map(|i| at(&self.za, self.idx(i, j))).collect();
            let spline = Interp::new(InterpMethod::CubicSpline, &self.xa, &col)?;
            for i in 0..xsize {
                let d = spline.eval_deriv(at(&self.xa, i))?;
                if let Some(slot) = zx.get_mut(self.idx(i, j)) {
                    *slot = d;
                }
            }
        }
        // zy: through every column (y varies).
        for i in 0..xsize {
            let col: Vec<f64> = (0..ysize).map(|j| at(&self.za, self.idx(i, j))).collect();
            let spline = Interp::new(InterpMethod::CubicSpline, &self.ya, &col)?;
            for j in 0..ysize {
                let d = spline.eval_deriv(at(&self.ya, j))?;
                if let Some(slot) = zy.get_mut(self.idx(i, j)) {
                    *slot = d;
                }
            }
        }
        // zxy: x-derivative of zy, row by row.
        for j in 0..ysize {
            let col: Vec<f64> = (0..xsize).map(|i| at(&zy, self.idx(i, j))).collect();
            let spline = Interp::new(InterpMethod::CubicSpline, &self.xa, &col)?;
            for i in 0..xsize {
                let d = spline.eval_deriv(at(&self.xa, i))?;
                if let Some(slot) = zxy.get_mut(self.idx(i, j)) {
                    *slot = d;
                }
            }
        }
        self.zx = zx;
        self.zy = zy;
        self.zxy = zxy;
        Ok(())
    }

    /// `interp2d_eval`'s range test.
    fn check(&self, x: f64, y: f64) -> Result<()> {
        let (Some(&xmin), Some(&xmax), Some(&ymin), Some(&ymax)) = (
            self.xa.first(),
            self.xa.last(),
            self.ya.first(),
            self.ya.last(),
        ) else {
            return Err(PetirError::Invalid);
        };
        if !(x >= xmin && x <= xmax) || !(y >= ymin && y <= ymax) {
            return Err(PetirError::Domain);
        }
        Ok(())
    }

    /// The cell holding `(x, y)` with its corner data: `(xi, yi, xmin, xmax,
    /// ymin, ymax, zminmin, zminmax, zmaxmin, zmaxmax)`.
    fn cell(&self, x: f64, y: f64) -> (usize, usize, f64, f64, f64, f64, [f64; 4]) {
        let xi = bsearch(&self.xa, x);
        let yi = bsearch(&self.ya, y);
        let xmin = at(&self.xa, xi);
        let xmax = at(&self.xa, xi + 1);
        let ymin = at(&self.ya, yi);
        let ymax = at(&self.ya, yi + 1);
        let z = [
            at(&self.za, self.idx(xi, yi)),         // zminmin
            at(&self.za, self.idx(xi, yi + 1)),     // zminmax
            at(&self.za, self.idx(xi + 1, yi)),     // zmaxmin
            at(&self.za, self.idx(xi + 1, yi + 1)), // zmaxmax
        ];
        (xi, yi, xmin, xmax, ymin, ymax, z)
    }

    /// The bicubic set-up shared by the six evaluators.
    fn bicubic_cell(&self, x: f64, y: f64) -> BicubicCell {
        let (xi, yi, xmin, xmax, ymin, ymax, z) = self.cell(x, y);
        let [zminmin, zminmax, zmaxmin, zmaxmax] = z;
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let t = (x - xmin) / dx;
        let u = (y - ymin) / dy;
        let dt = 1. / dx; // partial t / partial x
        let du = 1. / dy; // partial u / partial y
        let g = |v: &[f64], i: usize, j: usize| at(v, self.idx(i, j));
        let t2 = t * t;
        let u2 = u * u;
        BicubicCell {
            t0: 1.0,
            t1: t,
            t2,
            t3: t * t2,
            u0: 1.0,
            u1: u,
            u2,
            u3: u * u2,
            dt,
            du,
            zminmin,
            zminmax,
            zmaxmin,
            zmaxmax,
            zxminmin: g(&self.zx, xi, yi) / dt,
            zxminmax: g(&self.zx, xi, yi + 1) / dt,
            zxmaxmin: g(&self.zx, xi + 1, yi) / dt,
            zxmaxmax: g(&self.zx, xi + 1, yi + 1) / dt,
            zyminmin: g(&self.zy, xi, yi) / du,
            zyminmax: g(&self.zy, xi, yi + 1) / du,
            zymaxmin: g(&self.zy, xi + 1, yi) / du,
            zymaxmax: g(&self.zy, xi + 1, yi + 1) / du,
            zxyminmin: g(&self.zxy, xi, yi) / (dt * du),
            zxyminmax: g(&self.zxy, xi, yi + 1) / (dt * du),
            zxymaxmin: g(&self.zxy, xi + 1, yi) / (dt * du),
            zxymaxmax: g(&self.zxy, xi + 1, yi + 1) / (dt * du),
        }
    }

    /// `bilinear_eval` (bilinear.c:34-68).
    fn bilinear_eval(&self, x: f64, y: f64) -> f64 {
        let (_, _, xmin, xmax, ymin, ymax, z) = self.cell(x, y);
        let [zminmin, zminmax, zmaxmin, zmaxmax] = z;
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let t = (x - xmin) / dx;
        let u = (y - ymin) / dy;
        (1. - t) * (1. - u) * zminmin
            + t * (1. - u) * zmaxmin
            + (1. - t) * u * zminmax
            + t * u * zmaxmax
    }

    /// `bilinear_deriv_x` (bilinear.c:70-105).
    fn bilinear_deriv_x(&self, x: f64, y: f64) -> f64 {
        let (_, _, xmin, xmax, ymin, ymax, z) = self.cell(x, y);
        let [zminmin, zminmax, zmaxmin, zmaxmax] = z;
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let dt = 1. / dx;
        let u = (y - ymin) / dy;
        dt * (-(1. - u) * zminmin + (1. - u) * zmaxmin - u * zminmax + u * zmaxmax)
    }

    /// `bilinear_deriv_y` (bilinear.c:107-142).
    fn bilinear_deriv_y(&self, x: f64, y: f64) -> f64 {
        let (_, _, xmin, xmax, ymin, ymax, z) = self.cell(x, y);
        let [zminmin, zminmax, zmaxmin, zmaxmax] = z;
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let t = (x - xmin) / dx;
        let du = 1. / dy;
        du * (-(1. - t) * zminmin - t * zmaxmin + (1. - t) * zminmax + t * zmaxmax)
    }

    /// `bilinear_derivxy` (bilinear.c:156-190).
    fn bilinear_deriv_xy(&self, x: f64, y: f64) -> f64 {
        let (_, _, xmin, xmax, ymin, ymax, z) = self.cell(x, y);
        let [zminmin, zminmax, zmaxmin, zmaxmax] = z;
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let dt = 1. / dx;
        let du = 1. / dy;
        dt * du * (zminmin - zmaxmin - zminmax + zmaxmax)
    }

    /// Interpolated value at `(x, y)`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] outside the grid or for `NaN`.
    pub fn eval(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(self.eval_extrap(x, y))
    }

    /// `gsl_interp2d_eval_extrap`: [`Self::eval`] without the range check
    /// (outside the grid the end cell's polynomial is extended). `NaN` gives
    /// `NaN`.
    pub fn eval_extrap(&self, x: f64, y: f64) -> f64 {
        match self.kind {
            Interp2dKind::Bilinear => self.bilinear_eval(x, y),
            Interp2dKind::Bicubic => crate::interp2d_bicubic::eval(&self.bicubic_cell(x, y)),
        }
    }

    /// `d z / d x` at `(x, y)`.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv_x(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(match self.kind {
            Interp2dKind::Bilinear => self.bilinear_deriv_x(x, y),
            Interp2dKind::Bicubic => crate::interp2d_bicubic::deriv_x(&self.bicubic_cell(x, y)),
        })
    }

    /// `d z / d y` at `(x, y)`.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv_y(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(match self.kind {
            Interp2dKind::Bilinear => self.bilinear_deriv_y(x, y),
            Interp2dKind::Bicubic => crate::interp2d_bicubic::deriv_y(&self.bicubic_cell(x, y)),
        })
    }

    /// `d^2 z / d x^2` at `(x, y)` (identically 0 for bilinear).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv_xx(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(match self.kind {
            Interp2dKind::Bilinear => 0.0,
            Interp2dKind::Bicubic => crate::interp2d_bicubic::deriv_xx(&self.bicubic_cell(x, y)),
        })
    }

    /// `d^2 z / d y^2` at `(x, y)` (identically 0 for bilinear).
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv_yy(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(match self.kind {
            Interp2dKind::Bilinear => 0.0,
            Interp2dKind::Bicubic => crate::interp2d_bicubic::deriv_yy(&self.bicubic_cell(x, y)),
        })
    }

    /// `d^2 z / d x d y` at `(x, y)`.
    ///
    /// # Errors
    ///
    /// As [`Self::eval`].
    pub fn eval_deriv_xy(&self, x: f64, y: f64) -> Result<f64> {
        self.check(x, y)?;
        Ok(match self.kind {
            Interp2dKind::Bilinear => self.bilinear_deriv_xy(x, y),
            Interp2dKind::Bicubic => crate::interp2d_bicubic::deriv_xy(&self.bicubic_cell(x, y)),
        })
    }

    /// The method.
    pub fn kind(&self) -> Interp2dKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(n: usize, m: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let x: Vec<f64> = (0..n)
            .map(|i| 0.5 * i as f64 + 0.01 * (i * i) as f64)
            .collect();
        let y: Vec<f64> = (0..m)
            .map(|j| 0.7 * j as f64 + 0.02 * (j * j) as f64)
            .collect();
        let mut z = vec![0.0; n * m];
        for j in 0..m {
            for i in 0..n {
                z[j * n + i] = 1.0 + 2.0 * x[i] - 3.0 * y[j] + 0.5 * x[i] * y[j];
            }
        }
        (x, y, z)
    }

    #[test]
    fn bilinear_is_exact_for_a_bilinear_function() {
        let (x, y, z) = grid(4, 5);
        let s = Interp2d::new(Interp2dKind::Bilinear, &x, &y, &z).unwrap();
        let (px, py) = (0.9, 1.3);
        let want = 1.0 + 2.0 * px - 3.0 * py + 0.5 * px * py;
        assert!((s.eval(px, py).unwrap() - want).abs() < 1e-12);
        assert!((s.eval_deriv_x(px, py).unwrap() - (2.0 + 0.5 * py)).abs() < 1e-12);
        assert!((s.eval_deriv_y(px, py).unwrap() - (-3.0 + 0.5 * px)).abs() < 1e-12);
        assert!((s.eval_deriv_xy(px, py).unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(s.eval_deriv_xx(px, py).unwrap(), 0.0);
        assert_eq!(s.eval_deriv_yy(px, py).unwrap(), 0.0);
    }

    #[test]
    fn bicubic_interpolates_the_nodes_and_reproduces_a_bilinear_function() {
        let (x, y, z) = grid(5, 6);
        let s = Interp2d::new(Interp2dKind::Bicubic, &x, &y, &z).unwrap();
        for j in 0..6 {
            for i in 0..5 {
                let v = s.eval(x[i], y[j]).unwrap();
                assert!((v - z[j * 5 + i]).abs() < 1e-11, "node ({i},{j})");
            }
        }
        // The natural-spline end conditions are exact for a linear function
        // along each axis, so the interior of this bilinear function is
        // reproduced.
        let (px, py) = (1.1, 1.9);
        let want = 1.0 + 2.0 * px - 3.0 * py + 0.5 * px * py;
        assert!((s.eval(px, py).unwrap() - want).abs() < 1e-9);
        assert!((s.eval_deriv_x(px, py).unwrap() - (2.0 + 0.5 * py)).abs() < 1e-8);
        assert!((s.eval_deriv_y(px, py).unwrap() - (-3.0 + 0.5 * px)).abs() < 1e-8);
        assert!((s.eval_deriv_xy(px, py).unwrap() - 0.5).abs() < 1e-7);
        assert!(s.eval_deriv_xx(px, py).unwrap().abs() < 1e-7);
        assert!(s.eval_deriv_yy(px, py).unwrap().abs() < 1e-7);
    }

    #[test]
    fn refusals() {
        let (x, y, z) = grid(4, 4);
        assert!(Interp2d::new(Interp2dKind::Bicubic, &x[..3], &y, &z).is_err());
        assert!(Interp2d::new(Interp2dKind::Bilinear, &x[..1], &y, &z).is_err());
        assert!(Interp2d::new(Interp2dKind::Bilinear, &x, &y, &z[..15]).is_err());
        let bad = [0.0, 1.0, 1.0, 2.0];
        assert!(Interp2d::new(Interp2dKind::Bilinear, &bad, &y, &z).is_err());
        assert!(Interp2d::new(Interp2dKind::Bilinear, &x, &bad, &z).is_err());
        let s = Interp2d::new(Interp2dKind::Bicubic, &x, &y, &z).unwrap();
        assert_eq!(s.kind(), Interp2dKind::Bicubic);
        assert_eq!(s.eval(-1.0, 0.0), Err(PetirError::Domain));
        assert_eq!(s.eval(0.0, f64::NAN), Err(PetirError::Domain));
        assert!(s.eval_deriv_x(100.0, 0.0).is_err());
        assert!(s.eval_deriv_y(0.0, 100.0).is_err());
        assert!(s.eval_deriv_xx(100.0, 0.0).is_err());
        assert!(s.eval_deriv_yy(0.0, 100.0).is_err());
        assert!(s.eval_deriv_xy(100.0, 0.0).is_err());
        assert!(s.eval_extrap(100.0, 0.0).is_finite());
        assert_eq!(Interp2dKind::Bilinear.min_size(), 2);
    }
}
