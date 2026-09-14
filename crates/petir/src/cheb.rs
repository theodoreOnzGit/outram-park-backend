// Ported from the GNU Scientific Library `cheb/` module
// (cheb/init.c, cheb/eval.c, cheb/deriv.c, cheb/integ.c, cheb/gsl_chebyshev.h),
// GSL 2.8 at commit cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
//
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman  (upstream)
// Copyright (C) 2026 Theodore Ong and the outram-park contributors  (this port)
//
// GSL is distributed under the GNU General Public License, version 3 or later
// (verified from its per-file headers -- see NOTICE). This derivative work is
// distributed under GPL-3.0-only. The flow is ONE-WAY: this code cannot return
// upstream under a different licence.

//! Chebyshev series approximation — a port of GSL's `cheb/` module.
//!
//! # What this is
//!
//! A [`ChebSeries`] holds the coefficients `c[0..=order]` of a Chebyshev
//! expansion of a function on `[a, b]`, built by sampling that function at the
//! Chebyshev nodes. For smooth functions the coefficients fall off rapidly, so
//! a modest order reproduces the function to near machine precision and the
//! series can be differentiated, integrated and re-evaluated cheaply.
//!
//! # Fidelity to upstream
//!
//! Each routine below names the GSL function it ports and the file it came
//! from. The arithmetic is transcribed, not re-derived — including the details
//! that look like they could be tidied:
//!
//! - [`eval`](ChebSeries::eval) is Clenshaw's recurrence written exactly as
//!   `gsl_cheb_eval`, with the same `0.5 * c[0]` closing term.
//! - [`eval_err`](ChebSeries::eval_err) combines truncation and round-off the
//!   way upstream does: `|c[order]| + (sum |c[i]|) * DBL_EPSILON`.
//! - [`deriv`](ChebSeries::deriv) and [`integ`](ChebSeries::integ) keep
//!   upstream's index arithmetic and its `n == 1` / `n == 2` special cases.
//!
//! Where the C is unsound to transcribe literally it is noted at the site.
//!
//! # Differences forced by Rust and `no_std`
//!
//! - `gsl_cheb_alloc` + `gsl_cheb_init` become one fallible constructor,
//!   [`ChebSeries::new`]. Splitting them would leave a half-built series
//!   observable, which C tolerates and Rust need not.
//! - `gsl_function` (a C function pointer plus `void *params`) becomes a
//!   generic `F: Fn(f64) -> f64`.
//! - GSL's `f[]` workspace (the sampled function values) is **not** retained.
//!   Upstream keeps it in the struct, and its own `deriv`/`integ` note
//!   `FIXME: should probably set deriv->f[] as well` — i.e. upstream does not
//!   maintain it either. Holding a field that is stale for two of four
//!   constructors is worse than not holding it.
//! - Errors are returned. GSL's `GSL_ERROR_VAL` calls a handler that aborts by
//!   default.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{PetirError, Result};

/// `GSL_DBL_EPSILON` (`gsl_machine.h:17`). Identical to [`f64::EPSILON`];
/// spelled out so the port reads against upstream.
const GSL_DBL_EPSILON: f64 = 2.2204460492503131e-16;

/// A Chebyshev series approximating a function on `[a, b]`.
///
/// Ports `gsl_cheb_series` (`cheb/gsl_chebyshev.h`). Upstream's `order_sp`
/// (a single-precision order used by `gsl_cheb_eval_mode`) and `f` workspace
/// are omitted — see the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct ChebSeries {
    /// Coefficients `c[0..=order]`.
    c: Vec<f64>,
    /// Lower interval bound.
    a: f64,
    /// Upper interval bound.
    b: f64,
}

impl ChebSeries {
    /// Build the order-`order` Chebyshev expansion of `func` on `[a, b]`.
    ///
    /// Ports `gsl_cheb_alloc` + `gsl_cheb_init` (`cheb/init.c:28-95`). The
    /// function is sampled at the `order + 1` Chebyshev nodes
    ///
    /// ```text
    ///   y_k = cos(pi (k + 1/2) / (order + 1)),   x_k = y_k * (b-a)/2 + (b+a)/2
    /// ```
    ///
    /// and the coefficients follow from the discrete cosine sum
    ///
    /// ```text
    ///   c[j] = 2/(order+1) * sum_k f(x_k) cos(pi j (k + 1/2) / (order + 1))
    /// ```
    ///
    /// This is `O(order^2)`, as upstream is; a DCT would be `O(n log n)` but
    /// would no longer be a transcription of `gsl_cheb_init`.
    ///
    /// # Errors
    /// [`PetirError::Domain`] if `a >= b` (`GSL_EDOM`, `cheb/init.c:70`).
    pub fn new<F>(order: usize, a: f64, b: f64, func: F) -> Result<Self>
    where
        F: Fn(f64) -> f64,
    {
        if a >= b {
            // cheb/init.c:70 -- "null function interval [a,b]", GSL_EDOM.
            return Err(PetirError::Domain);
        }
        let n = order + 1;
        let bma = 0.5 * (b - a);
        let bpa = 0.5 * (b + a);
        let fac = 2.0 / (n as f64);

        let mut fk: Vec<f64> = vec![0.0; n];
        for (k, slot) in fk.iter_mut().enumerate() {
            let y = libm::cos(core::f64::consts::PI * (k as f64 + 0.5) / (n as f64));
            *slot = func(y * bma + bpa);
        }

        let mut c: Vec<f64> = vec![0.0; n];
        for (j, cj) in c.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (k, &f) in fk.iter().enumerate() {
                sum += f * libm::cos(
                    core::f64::consts::PI * (j as f64) * (k as f64 + 0.5) / (n as f64),
                );
            }
            *cj = fac * sum;
        }

        Ok(ChebSeries { c, a, b })
    }

    /// Build directly from known coefficients on `[a, b]`.
    ///
    /// Not a GSL entry point — upstream exposes `gsl_cheb_coeffs` for reading
    /// and expects `gsl_cheb_init` for writing. Provided because
    /// [`deriv`](Self::deriv) and [`integ`](Self::integ) must construct a
    /// series from coefficients rather than from samples, and because a caller
    /// holding a published coefficient table should not have to fit it.
    ///
    /// # Errors
    /// [`PetirError::Domain`] if `a >= b`; [`PetirError::Invalid`] if `c` is
    /// empty.
    pub fn from_coefficients(c: Vec<f64>, a: f64, b: f64) -> Result<Self> {
        if a >= b {
            return Err(PetirError::Domain);
        }
        if c.is_empty() {
            return Err(PetirError::Invalid);
        }
        Ok(ChebSeries { c, a, b })
    }

    /// The series order (`gsl_cheb_order`, `cheb/init.c:98`) — one less than
    /// the number of coefficients.
    pub fn order(&self) -> usize {
        self.c.len() - 1
    }

    /// Number of coefficients (`gsl_cheb_size`, `cheb/init.c:104`).
    pub fn size(&self) -> usize {
        self.c.len()
    }

    /// The coefficients (`gsl_cheb_coeffs`, `cheb/init.c:110`).
    pub fn coefficients(&self) -> &[f64] {
        &self.c
    }

    /// The interval the series was built on.
    pub fn interval(&self) -> (f64, f64) {
        (self.a, self.b)
    }

    /// Evaluate the series at `x` (`gsl_cheb_eval`, `cheb/eval.c:29-46`).
    ///
    /// Clenshaw's recurrence. Note upstream does **not** range-check `x`
    /// against `[a, b]`: evaluating outside the interval extrapolates, and
    /// does so badly, but it is not an error. That behaviour is preserved.
    pub fn eval(&self, x: f64) -> f64 {
        self.eval_n_unchecked(self.order(), x)
    }

    /// Evaluate using at most the first `n` orders
    /// (`gsl_cheb_eval_n`, `cheb/eval.c:49-69`).
    ///
    /// `n` is clamped to the series order, as upstream's `GSL_MIN` does.
    pub fn eval_n(&self, n: usize, x: f64) -> f64 {
        self.eval_n_unchecked(n.min(self.order()), x)
    }

    /// The shared Clenshaw body. `eval_order` must already be `<= order`.
    fn eval_n_unchecked(&self, eval_order: usize, x: f64) -> f64 {
        let mut d1 = 0.0;
        let mut d2 = 0.0;
        let y = (2.0 * x - self.a - self.b) / (self.b - self.a);
        let y2 = 2.0 * y;

        // C: for (i = eval_order; i >= 1; i--). The C loop relies on `size_t`
        // stopping at 1; the Rust range is the same sweep, and is empty when
        // eval_order == 0, which is what the C does too.
        for i in (1..=eval_order).rev() {
            let temp = d1;
            d1 = y2 * d1 - d2 + self.c[i];
            d2 = temp;
        }
        y * d1 - d2 + 0.5 * self.c[0]
    }

    /// Evaluate with an error estimate
    /// (`gsl_cheb_eval_err`, `cheb/eval.c:72-104`).
    ///
    /// Returns `(result, abserr)`. The estimate is upstream's: the first
    /// dropped term `|c[order]|` for truncation, plus `DBL_EPSILON` times the
    /// sum of `|c[i]|` for accumulated round-off.
    pub fn eval_err(&self, x: f64) -> (f64, f64) {
        self.eval_n_err_unchecked(self.order(), x)
    }

    /// As [`eval_err`](Self::eval_err), using at most `n` orders
    /// (`gsl_cheb_eval_n_err`, `cheb/eval.c:107-144`).
    pub fn eval_n_err(&self, n: usize, x: f64) -> (f64, f64) {
        self.eval_n_err_unchecked(n.min(self.order()), x)
    }

    fn eval_n_err_unchecked(&self, eval_order: usize, x: f64) -> (f64, f64) {
        let result = self.eval_n_unchecked(eval_order, x);
        // C sums |c[i]| for i = 0 ..= eval_order, then adds |c[eval_order]|.
        let absc: f64 = self.c[..=eval_order].iter().map(|v| libm::fabs(*v)).sum();
        let abserr = libm::fabs(self.c[eval_order]) + absc * GSL_DBL_EPSILON;
        (result, abserr)
    }

    /// The series of the derivative (`gsl_cheb_calc_deriv`, `cheb/deriv.c:26-59`).
    ///
    /// Returns a new series of the **same order** on the same interval, as
    /// upstream requires (it errors unless `deriv->order == f->order`); the
    /// top coefficient is zero by construction.
    pub fn deriv(&self) -> Self {
        let n = self.size();
        let con = 2.0 / (self.b - self.a);
        let mut d = vec![0.0; n];

        d[n - 1] = 0.0;
        if n > 1 {
            d[n - 2] = 2.0 * (n as f64 - 1.0) * self.c[n - 1];
            // C: for (i = n; i >= 3; i--) d[i-3] = d[i-1] + 2*(i-2)*c[i-2];
            for i in (3..=n).rev() {
                d[i - 3] = d[i - 1] + 2.0 * (i as f64 - 2.0) * self.c[i - 2];
            }
            for v in d.iter_mut().take(n) {
                *v *= con;
            }
        }
        ChebSeries {
            c: d,
            a: self.a,
            b: self.b,
        }
    }

    /// The series of the integral (`gsl_cheb_calc_integ`, `cheb/integ.c:26-64`).
    ///
    /// Returns a new series of the same order on the same interval. The
    /// constant of integration is fixed by upstream's convention: `c[0]` is
    /// twice the alternating sum of the other coefficients, which makes the
    /// integral vanish at `x = a`.
    pub fn integ(&self) -> Self {
        let n = self.size();
        let con = 0.25 * (self.b - self.a);
        let mut g = vec![0.0; n];

        if n == 1 {
            g[0] = 0.0;
        } else if n == 2 {
            g[1] = con * self.c[0];
            g[0] = 2.0 * g[1];
        } else {
            let mut sum = 0.0;
            let mut fac = 1.0;
            for i in 1..=n - 2 {
                g[i] = con * (self.c[i - 1] - self.c[i + 1]) / (i as f64);
                sum += fac * g[i];
                fac = -fac;
            }
            g[n - 1] = con * self.c[n - 2] / (n as f64 - 1.0);
            sum += fac * g[n - 1];
            g[0] = 2.0 * sum;
        }
        ChebSeries {
            c: g,
            a: self.a,
            b: self.b,
        }
    }
}
