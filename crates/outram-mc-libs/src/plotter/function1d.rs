// SPDX-License-Identifier: GPL-3.0-only
//! **`openmc.data.Tabulated1D` and `openmc.data.Polynomial`, evaluated exactly
//! as upstream evaluates them on an array.**
//!
//! Ported from OpenMC `openmc/data/function.py` at commit `d7d3284a1`
//! (0.16.1.dev25): `Tabulated1D.__init__` (`:145-155`), the array branch of
//! `Tabulated1D.__call__` (`:157-210`), `Tabulated1D.from_ace` (`:396-439`) and
//! `Polynomial` (`:442`, which is `numpy.polynomial.Polynomial`, so its
//! `__call__` is NumPy's `polyval`). OpenMC is MIT-licensed; notice in
//! `crates/outram-mc-libs/LICENSE.openmc`.
//!
//! # The two upstream behaviours a straightforward interpolator gets wrong
//!
//! 1. **Outside the table the value is zero, not clamped.** The array path
//!    starts from `np.zeros_like(x)` and only fills points whose bin index
//!    lies inside an interpolation region, so a threshold reaction evaluated on
//!    the whole nuclide grid is 0 below threshold.
//! 2. **"Close to an end" snaps to the end value** (`function.py:205-208`):
//!    every `x` with `np.isclose(x, self.x[0], atol=1e-14)` is set to
//!    `self.y[0]`, and likewise for the last point. `np.isclose` carries its
//!    default `rtol = 1e-5`, so this is a *relative* window of 10 ppm on both
//!    sides of each end, not an absolute 1e-14. A plotted material cross
//!    section that is off by one ULP from upstream at a nuclide's threshold
//!    is almost always this rule not being applied.

use super::numpy_ops::{isclose_scalar, searchsorted_right};
use super::PlotError;
use njoy_outram_park_fork::acer::read::RawAceTable;

/// eV per MeV, `openmc.data.EV_PER_MEV`.
pub const EV_PER_MEV: f64 = 1.0e6;

/// A tabulated function with ENDF interpolation regions.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabulated1D {
    /// Abscissae.
    pub x: Vec<f64>,
    /// Ordinates.
    pub y: Vec<f64>,
    /// One-based region end indices (`NBT`).
    pub breakpoints: Vec<usize>,
    /// ENDF interpolation law per region (`INT`: 1 histogram, 2 lin-lin,
    /// 3 lin-log, 4 log-lin, 5 log-log).
    pub interpolation: Vec<u32>,
}

impl Tabulated1D {
    /// `Tabulated1D(x, y)`: one linear-linear region spanning `len(x)` points.
    pub fn lin_lin(x: Vec<f64>, y: Vec<f64>) -> Self {
        let n = x.len();
        Self {
            x,
            y,
            breakpoints: vec![n],
            interpolation: vec![2],
        }
    }

    /// The array branch of `Tabulated1D.__call__` (`function.py:157-210`).
    ///
    /// # Panics
    ///
    /// Where upstream would raise `IndexError`: an `x` whose bin index points
    /// past the end of `y` (a table with fewer ordinates than abscissae).
    pub fn eval(&self, xs: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; xs.len()];
        let idx: Vec<isize> = xs
            .iter()
            .map(|&v| searchsorted_right(&self.x, v) as isize - 1)
            .collect();
        for k in 0..self.breakpoints.len() {
            let i_begin = if k > 0 {
                self.breakpoints[k - 1] as isize - 1
            } else {
                0
            };
            let i_end = self.breakpoints[k] as isize - 1;
            let law = self.interpolation[k];
            for (o, (&xk, &i)) in out.iter_mut().zip(xs.iter().zip(idx.iter())) {
                if !(i >= i_begin && i < i_end) {
                    continue;
                }
                let i = i as usize;
                let (xi, xi1, yi, yi1) = (self.x[i], self.x[i + 1], self.y[i], self.y[i + 1]);
                *o = match law {
                    1 => yi,
                    2 => yi + (xk - xi) / (xi1 - xi) * (yi1 - yi),
                    3 => yi + (xk / xi).ln() / (xi1 / xi).ln() * (yi1 - yi),
                    4 => yi * ((xk - xi) / (xi1 - xi) * (yi1 / yi).ln()).exp(),
                    5 => yi * ((xk / xi).ln() / (xi1 / xi).ln() * (yi1 / yi).ln()).exp(),
                    // Upstream leaves the zero from `zeros_like` in place for
                    // an unrecognised law.
                    _ => *o,
                };
            }
        }
        if let (Some(&x0), Some(&y0), Some(&xl), Some(&yl)) =
            (self.x.first(), self.y.first(), self.x.last(), self.y.last())
        {
            for (o, &v) in out.iter_mut().zip(xs.iter()) {
                if isclose_scalar(v, x0, 1.0e-14) {
                    *o = y0;
                }
            }
            for (o, &v) in out.iter_mut().zip(xs.iter()) {
                if isclose_scalar(v, xl, 1.0e-14) {
                    *o = yl;
                }
            }
        }
        out
    }

    /// `Tabulated1D.from_ace(ace, idx, convert_units=True)`
    /// (`function.py:396-439`). `idx` is upstream's **one-based** XSS index,
    /// kept one-based so the call sites read like `reaction.py`.
    ///
    /// Not delegated to `njoy_outram_park_fork::acer::ce_laws::read_ace_tab1`,
    /// which drops the interpolation regions (it serves the NU block, whose
    /// consumer interpolates lin-lin regardless), nor to its region-keeping
    /// sibling `read_tab1_full`, which is crate-private there. A plot of a
    /// histogram-law yield has to see the histogram.
    pub fn from_ace(t: &RawAceTable, idx: usize) -> Result<Self, PlotError> {
        let n_regions = px(t, idx)? as usize;
        let n_pairs = px(t, idx + 1 + 2 * n_regions)? as usize;
        let idx = idx + 1;
        let (breakpoints, interpolation) = if n_regions > 0 {
            let mut b = Vec::with_capacity(n_regions);
            let mut l = Vec::with_capacity(n_regions);
            for r in 0..n_regions {
                b.push(px(t, idx + r)? as usize);
                l.push(px(t, idx + n_regions + r)? as u32);
            }
            (b, l)
        } else {
            (vec![n_pairs], vec![2])
        };
        let idx = idx + 2 * n_regions + 1;
        let mut x = Vec::with_capacity(n_pairs);
        let mut y = Vec::with_capacity(n_pairs);
        for k in 0..n_pairs {
            x.push(px(t, idx + k)? * EV_PER_MEV);
            y.push(px(t, idx + n_pairs + k)?);
        }
        Ok(Self {
            x,
            y,
            breakpoints,
            interpolation,
        })
    }
}

/// Python-style **one-based** read of `ace.xss[i]` (upstream pads `xss` with a
/// leading zero so that JXS locators index it directly; `RawAceTable::xss` is
/// zero-based).
pub(crate) fn px(t: &RawAceTable, i: usize) -> Result<f64, PlotError> {
    if i == 0 || i > t.xss.len() {
        return Err(PlotError::Ace(format!(
            "XSS index {i} outside the table (length {})",
            t.xss.len()
        )));
    }
    Ok(t.xss[i - 1])
}

/// A power series in increasing degree — `openmc.data.Polynomial`.
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    /// Coefficients, lowest degree first.
    pub coef: Vec<f64>,
}

impl Polynomial {
    /// NumPy's `polyval` as `Polynomial.__call__` runs it: the identity domain
    /// map, then `c0 = c[-1] + x*0` and `c0 = c[-i] + c0*x`.
    pub fn eval(&self, xs: &[f64]) -> Vec<f64> {
        let c = &self.coef;
        xs.iter()
            .map(|&x| {
                let x = 0.0 + 1.0 * x;
                let Some(&last) = c.last() else {
                    return 0.0;
                };
                let mut c0 = last + x * 0.0;
                for i in 2..=c.len() {
                    c0 = c[c.len() - i] + c0 * x;
                }
                c0
            })
            .collect()
    }
}

/// The two `Function1D` kinds a neutron yield can take in OpenMC's data model.
#[derive(Debug, Clone, PartialEq)]
pub enum Function1D {
    /// A tabulated function.
    Tabulated(Tabulated1D),
    /// A polynomial.
    Polynomial(Polynomial),
}

impl Function1D {
    /// Evaluate on an array, as `Function1D.__call__` would.
    pub fn eval(&self, xs: &[f64]) -> Vec<f64> {
        match self {
            Self::Tabulated(t) => t.eval(xs),
            Self::Polynomial(p) => p.eval(xs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_outside_and_snap_at_ends() {
        let t = Tabulated1D::lin_lin(vec![1.0, 2.0, 3.0], vec![5.0, 7.0, 9.0]);
        let got = t.eval(&[0.5, 1.0, 1.5, 3.0, 3.5, 3.0 * (1.0 + 5.0e-6)]);
        // 3.0*(1+5e-6) is outside the table but within isclose's 1e-5
        // relative window of the last point, so it snaps to y[-1].
        assert_eq!(got, vec![0.0, 5.0, 6.0, 9.0, 0.0, 9.0]);
    }

    #[test]
    fn polynomial_is_horner_from_the_top() {
        let p = Polynomial {
            coef: vec![1.0, 2.0, 3.0],
        };
        assert_eq!(p.eval(&[2.0]), vec![17.0]);
    }
}
