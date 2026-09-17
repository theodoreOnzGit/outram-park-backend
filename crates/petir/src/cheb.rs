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
use crate::linalg::{Matrix, QrDecomposition};
// Under a std-linked build (`cargo test`) f64's inherent sqrt shadows the
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;
use crate::zip::zip_flat;

/// `GSL_DBL_EPSILON` (`gsl_machine.h:17`). Identical to [`f64::EPSILON`];
/// spelled out so the port reads against upstream.
const GSL_DBL_EPSILON: f64 = 2.2204460492503131e-16;

/// Evaluation precision — ports `gsl_mode_t`'s `GSL_MODE_PREC` (`gsl_mode.h`).
///
/// An enum rather than the bit-masked `unsigned int` upstream uses, per the
/// workspace rule that a closed set of choices is dispatched by enum: the
/// compiler then forces every `match` to handle a new variant, where a stray
/// bit pattern in `gsl_mode_t` silently falls through to the `else`.
///
/// # What the reduced modes actually do
///
/// [`Single`](Precision::Single) and [`Approx`](Precision::Approx) both select
/// [`ChebSeries::order_sp`], which upstream leaves equal to the full order.
/// So unless you set it, **all three modes give the same answer** — that is
/// upstream's behaviour, faithfully, and it is documented here rather than
/// papered over with an invented reduction rule.
///
/// Upstream distinguishes `GSL_PREC_SINGLE` from `GSL_PREC_APPROX` elsewhere
/// (in `specfunc`, where different series are selected); `cheb/eval.c` tests
/// only `== GSL_PREC_DOUBLE`, so the two behave identically here and this port
/// keeps both variants rather than collapsing them, so a caller porting GSL
/// code can pass what their source passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// `GSL_PREC_DOUBLE` — the full series order.
    Double,
    /// `GSL_PREC_SINGLE` — [`ChebSeries::order_sp`].
    Single,
    /// `GSL_PREC_APPROX` — [`ChebSeries::order_sp`], as `Single`.
    Approx,
}

impl Precision {
    /// The order `cheb/eval.c:163-166` selects for this mode.
    fn eval_order(self, cs: &ChebSeries) -> usize {
        match self {
            Precision::Double => cs.order(),
            Precision::Single | Precision::Approx => cs.order_sp(),
        }
    }
}

/// A Chebyshev series approximating a function on `[a, b]`.
///
/// Ports `gsl_cheb_series` (`cheb/gsl_chebyshev.h`). Upstream's `f` workspace
/// is omitted (it is scratch for `gsl_cheb_init`, not state); `order_sp` is
/// carried — see [`order_sp`](Self::order_sp).
#[derive(Debug, Clone, PartialEq)]
pub struct ChebSeries {
    /// Coefficients `c[0..=order]`.
    c: Vec<f64>,
    /// Upstream's `order_sp`: the order [`eval_mode`](Self::eval_mode) uses at
    /// reduced precision. Defaults to the full order, as upstream's
    /// `gsl_cheb_alloc` does.
    order_sp: usize,
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

        Ok(ChebSeries::with_default_order_sp(c, a, b))
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
        Ok(ChebSeries::with_default_order_sp(c, a, b))
    }

    /// Build from coefficients in the **plain** convention, where the
    /// constant term is `c[0] * T_0` rather than `c[0]/2 * T_0`.
    ///
    /// # Why this exists — two conventions, and mixing them is silent
    ///
    /// GSL's `gsl_cheb_init` produces a coefficient set whose zeroth entry is
    /// **doubled**, which `gsl_cheb_eval` undoes with its closing
    /// `0.5 * c[0]` (`cheb/eval.c:46`). Much published Chebyshev data — and
    /// `tampines-steam-tables`' backward correlations, whose evaluator closes
    /// with `c[0] + x*b1 - b2` — uses the other convention, where `c[0]` is
    /// the plain coefficient of `T_0`.
    ///
    /// Feeding one convention's table to the other evaluator is wrong by
    /// exactly `c[0]/2`, silently and everywhere. Measured on
    /// `[3, 2, 1]` at `x = -0.7`: plain gives 1.58 (the closed form), GSL's
    /// evaluator on the same bytes gives 0.08.
    ///
    /// The conversion is exact and is the whole of this constructor:
    /// `c[0] *= 2`. No numerics are invented — GSL's evaluator still does the
    /// arithmetic.
    ///
    /// # Errors
    /// As [`from_coefficients`](Self::from_coefficients).
    pub fn from_plain_coefficients(mut c: Vec<f64>, a: f64, b: f64) -> Result<Self> {
        if let Some(c0) = c.first_mut() {
            *c0 *= 2.0;
        }
        Self::from_coefficients(c, a, b)
    }

    /// Build with `order_sp` at the full order, which is what
    /// `gsl_cheb_alloc` (`cheb/init.c:39`) does.
    fn with_default_order_sp(c: Vec<f64>, a: f64, b: f64) -> Self {
        let order_sp = c.len().saturating_sub(1);
        ChebSeries { c, order_sp, a, b }
    }

    /// The reduced order [`eval_mode`](Self::eval_mode) uses at
    /// [`Precision::Single`] or [`Precision::Approx`].
    ///
    /// # Upstream computes nothing here, and says so
    ///
    /// `gsl_cheb_alloc` sets `order_sp = order` (`cheb/init.c:39`) and never
    /// changes it, so `gsl_cheb_eval_mode` at single precision is identical to
    /// `gsl_cheb_eval` unless the caller sets the field themselves. GSL's own
    /// header is explicit about why:
    ///
    /// > Users can use it if they like, but only they know how to calculate
    /// > it, since it is specific to the approximated function.
    ///
    /// That is upstream declining to supply a degree-selection rule, not an
    /// oversight — and it is independent confirmation that there is nothing in
    /// GSL to port for adaptive degree selection (`bn:op-0sl9`).
    ///
    /// # How to choose one
    ///
    /// [`eval_n_err`](Self::eval_n_err) reports upstream's error estimate at
    /// any order, so the smallest order meeting a tolerance you name can be
    /// found by asking it:
    ///
    /// ```
    /// use petir::ChebSeries;
    /// let cs = ChebSeries::new(40, -1.0, 1.0, |x: f64| x.exp()).unwrap();
    /// // The smallest order whose error estimate is within 1e-10, judged at
    /// // the worst of a few sample points.
    /// let chosen = (0..=cs.order())
    ///     .find(|&n| {
    ///         [-0.9_f64, -0.3, 0.4, 0.95]
    ///             .iter()
    ///             .all(|&x| cs.eval_n_err(n, x).1 <= 1e-10)
    ///     })
    ///     .unwrap_or(cs.order());
    /// assert!(chosen < cs.order(), "exp needs far fewer than 40 terms");
    /// ```
    ///
    /// This is a search over upstream's own error estimate, **not** a
    /// tail-chopping rule: it has no plateau or envelope analysis and it needs
    /// a tolerance from you, where Chebfun's `standardChop` infers one. It
    /// does not close `bn:op-0sl9`.
    pub fn order_sp(&self) -> usize {
        self.order_sp
    }

    /// Set the reduced order used at [`Precision::Single`].
    ///
    /// Clamped to the series order — upstream does no such check, and a
    /// larger value there would read past the coefficient array.
    pub fn set_order_sp(&mut self, order_sp: usize) {
        self.order_sp = order_sp.min(self.order());
    }

    /// The series order (`gsl_cheb_order`, `cheb/init.c:98`) — one less than
    /// the number of coefficients.
    pub fn order(&self) -> usize {
        // Saturating rather than `- 1`: every constructor rejects an empty
        // coefficient vector, so this is always `len - 1`, but a bare
        // subtraction would wrap-panic in a debug build if that ever changed.
        self.c.len().saturating_sub(1)
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
        // `c[1 ..= eval_order]`, walked backwards. `eval_order` is at most
        // `order() == len - 1`, so the slice always exists; an empty series
        // evaluates to zero, which is what the zero polynomial is.
        let Some((&c0, c_tail)) = self.c.split_first() else {
            return 0.0;
        };
        let Some(used) = c_tail.get(..eval_order) else {
            return 0.0;
        };
        for &c_i in used.iter().rev() {
            let temp = d1;
            d1 = y2 * d1 - d2 + c_i;
            d2 = temp;
        }
        y * d1 - d2 + 0.5 * c0
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
        let Some(used) = self.c.get(..=eval_order) else {
            // Unreachable: `eval_order <= order()`. An unknown error bound is
            // reported as infinite rather than as a confident small number.
            return (result, f64::INFINITY);
        };
        let absc: f64 = used.iter().map(|v| libm::fabs(*v)).sum();
        let Some(&c_top) = used.last() else {
            return (result, f64::INFINITY);
        };
        let abserr = libm::fabs(c_top) + absc * GSL_DBL_EPSILON;
        (result, abserr)
    }

    /// Evaluate at a precision (`gsl_cheb_eval_mode`, `cheb/eval.c:190`).
    ///
    /// [`Precision::Double`] uses the full order;
    /// [`Single`](Precision::Single) and [`Approx`](Precision::Approx) use
    /// [`order_sp`](Self::order_sp), which defaults to the full order — see
    /// [`Precision`] for why all three therefore agree unless you have set it.
    ///
    /// Upstream calls these "not meant for casual use"; [`eval`](Self::eval)
    /// and [`eval_n`](Self::eval_n) are the direct ways to say the same thing.
    /// This exists so that code being ported from GSL can be transcribed
    /// rather than reinterpreted.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::{ChebSeries, Precision};
    /// let mut cs = ChebSeries::new(24, -1.0, 1.0, |x: f64| x.exp()).unwrap();
    /// // Untouched, every mode agrees -- upstream's default.
    /// assert_eq!(cs.eval_mode(0.3, Precision::Single), cs.eval(0.3));
    /// // Set a reduced order and the cheaper mode truncates.
    /// cs.set_order_sp(6);
    /// assert_eq!(cs.eval_mode(0.3, Precision::Single), cs.eval_n(6, 0.3));
    /// assert_eq!(cs.eval_mode(0.3, Precision::Double), cs.eval(0.3));
    /// ```
    pub fn eval_mode(&self, x: f64, mode: Precision) -> f64 {
        self.eval_n_unchecked(mode.eval_order(self).min(self.order()), x)
    }

    /// As [`eval_mode`](Self::eval_mode), with an error estimate
    /// (`gsl_cheb_eval_mode_e`, `cheb/eval.c:148`).
    ///
    /// Returns `(result, abserr)`, the same pair and the same estimate as
    /// [`eval_n_err`](Self::eval_n_err).
    pub fn eval_mode_err(&self, x: f64, mode: Precision) -> (f64, f64) {
        self.eval_n_err_unchecked(mode.eval_order(self).min(self.order()), x)
    }

    /// The series of the derivative (`gsl_cheb_calc_deriv`, `cheb/deriv.c:26-59`).
    ///
    /// Returns a new series of the **same order** on the same interval, as
    /// upstream requires (it errors unless `deriv->order == f->order`); the
    /// top coefficient is zero by construction.
    pub fn deriv(&self) -> Self {
        let n = self.size();
        let con = 2.0 / (self.b - self.a);
        let flat = |c| ChebSeries::with_default_order_sp(c, self.a, self.b);
        // Upstream sets d[n-1] = 0 and stops; a one-coefficient series is a
        // constant, whose derivative is zero.
        let Some(&c_last) = self.c.last() else {
            return flat(vec![0.0; n]);
        };
        if n == 1 {
            return flat(vec![0.0; 1]);
        }

        // C: d[n-1] = 0; d[n-2] = 2*(n-1)*c[n-1];
        //    for (i = n; i >= 3; i--) d[i-3] = d[i-1] + 2*(i-2)*c[i-2];
        //
        // Substituting m = i - 3, the loop descends m = n-3 ..= 0 writing d[m]
        // and reading d[m+2] -- the value produced two steps earlier. Carrying
        // those two in locals lets `d` be built by pushing, so nothing is read
        // back through a subscript. The vector comes out reversed and is
        // flipped once at the end.
        let mut rev: Vec<f64> = Vec::with_capacity(n);
        let d_top = 0.0_f64; // d[n-1]
        let d_next = 2.0 * (n as f64 - 1.0) * c_last; // d[n-2]
        rev.push(d_top);
        rev.push(d_next);
        let mut ahead2 = d_top; // the d[m+2] read at m = n-3
        let mut ahead1 = d_next; // ... and at m = n-4

        // c[m+1] for m = n-3 ..= 0 is c[1 .. n-1], walked backwards.
        let Some(mid) = self.c.get(1..n - 1) else {
            return flat(vec![0.0; n]);
        };
        for (m, &c_m1) in (0..n - 2).zip(mid.iter()).rev() {
            let v = ahead2 + 2.0 * (m as f64 + 1.0) * c_m1;
            ahead2 = ahead1;
            ahead1 = v;
            rev.push(v);
        }
        rev.reverse();
        for v in rev.iter_mut() {
            *v *= con;
        }
        flat(rev)
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
        let flat = |c| ChebSeries::with_default_order_sp(c, self.a, self.b);
        let Some(&c0) = self.c.first() else {
            return flat(vec![0.0; n]);
        };

        if n == 1 {
            return flat(vec![0.0; 1]);
        }
        if n == 2 {
            let g1 = con * c0;
            return flat(vec![2.0 * g1, g1]);
        }

        // Upstream's `for (i = 1; i <= n-2; i++)` reads c[i-1] and c[i+1],
        // which is `c` zipped against itself offset by two; `g` is built by
        // pushing, with a placeholder for g[0] that the alternating sum fills
        // in at the end (upstream writes it last too).
        let mut g: Vec<f64> = Vec::with_capacity(n);
        g.push(0.0);
        let mut sum = 0.0;
        let mut fac = 1.0;
        for (i, (&c_lo, &c_hi)) in (1..).zip(zip_flat!(self.c.iter(), self.c.iter().skip(2))) {
            let g_i = con * (c_lo - c_hi) / (i as f64);
            sum += fac * g_i;
            fac = -fac;
            g.push(g_i);
        }

        // g[n-1] = con * c[n-2] / (n-1). `n >= 3` here, so `n - 2 >= 1`.
        let Some(&c_nm2) = self.c.get(n - 2) else {
            return flat(vec![0.0; n]);
        };
        let g_last = con * c_nm2 / (n as f64 - 1.0);
        sum += fac * g_last;
        g.push(g_last);

        if let Some(g0) = g.first_mut() {
            *g0 = 2.0 * sum;
        }
        flat(g)
    }
}

/// The outcome of a least-squares Chebyshev fit.
///
/// Returned by [`ChebSeries::fit`]. The residual is carried alongside the
/// series because a fit without one is an assertion rather than a measurement:
/// the coefficients alone cannot tell you whether the degree was adequate.
#[derive(Debug, Clone)]
pub struct ChebFit {
    /// The fitted series, ready to [`eval`](ChebSeries::eval).
    pub series: ChebSeries,
    /// `y_i - p(x_i)` at each sample, in the order the samples were given.
    ///
    /// Look at its **shape**, not only its size. A residual that still has
    /// visible structure — sign runs, a trend, a bump — means the degree is
    /// too low and there is signal left to capture. One that looks like noise
    /// means you have reached the data's own scatter and raising the degree
    /// will fit that scatter instead.
    pub residual: Vec<f64>,
    /// Root-mean-square residual, `sqrt(sum r_i^2 / m)`.
    pub rms_residual: f64,
    /// The rank indicator from the underlying factorisation; see
    /// [`QrDecomposition::rank`](crate::linalg::QrDecomposition::rank) for
    /// what it does and does not establish. Near `f64::EPSILON` means the
    /// sample points barely determine the requested degree.
    pub conditioning: f64,
}

impl ChebSeries {
    /// Least-squares fit of a Chebyshev series of the given `order` to
    /// scattered samples.
    ///
    /// # How this differs from [`ChebSeries::new`], and when to use which
    ///
    /// [`new`](ChebSeries::new) **interpolates a function you can call**, at
    /// abscissae it chooses — the Chebyshev nodes — and recovers the
    /// coefficients by a cosine transform. It is exact, it is fast, and it
    /// requires that you can evaluate the function wherever it likes.
    ///
    /// This fits **data you already have**, at whatever points it was measured
    /// or sampled at, with more points than coefficients. There is no
    /// transform available for that, so it solves the overdetermined system in
    /// the Chebyshev basis by QR ([`crate::linalg::qr`]). Reach for it when
    /// the samples come from an experiment, from a simulation you cannot
    /// cheaply re-run, or from a sampler that chose the points for its own
    /// reasons.
    ///
    /// Given samples exactly at the Chebyshev nodes and `order + 1` of them,
    /// the two agree to rounding — which is what
    /// `fit_reproduces_the_interpolant_at_the_nodes` checks.
    ///
    /// # Why the Chebyshev basis rather than fitting a monomial polynomial
    ///
    /// The monomial design matrix is a Vandermonde matrix, whose condition
    /// number grows exponentially with degree; past degree 10 or so a
    /// least-squares fit in `1, x, x^2, ...` is dominated by rounding. The
    /// Chebyshev basis is near-orthogonal on `[a, b]`, so the same fit stays
    /// solvable to much higher degree. That is the entire reason this routine
    /// exists rather than a `polyfit`.
    ///
    /// # Arguments
    ///
    /// - `order` — the highest Chebyshev degree to include; the fit has
    ///   `order + 1` coefficients.
    /// - `a`, `b` — the interval the series is defined on, `a < b`. Samples
    ///   are mapped onto `[-1, 1]` by [`crate::scale`].
    /// - `xs`, `ys` — the samples. Order does not matter and repeats are
    ///   allowed, provided enough *distinct* points remain to determine the
    ///   coefficients.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Domain`] if `a >= b`, or if any sample lies outside
    ///   `[a, b]`. Outside the interval the Chebyshev basis grows without
    ///   bound, so such a point would dominate the fit; that is a mistake
    ///   worth reporting rather than absorbing.
    /// - [`PetirError::LengthMismatch`] if `xs` and `ys` differ in length.
    /// - [`PetirError::Invalid`] if there are fewer samples than
    ///   `order + 1` — the system would be underdetermined.
    /// - [`PetirError::Singular`] if the samples do not determine the
    ///   requested degree, naming the first Chebyshev degree that is not
    ///   pinned down. The usual cause is too few *distinct* abscissae: five
    ///   samples at three distinct points cannot fix a cubic.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::ChebSeries;
    /// // Sample y = x^2 at six points and fit a quadratic; it is exact.
    /// let xs = [-1.0, -0.6, -0.2, 0.2, 0.6, 1.0];
    /// let ys: Vec<f64> = xs.iter().map(|x| x * x).collect();
    /// let fit = ChebSeries::fit(2, -1.0, 1.0, &xs, &ys).unwrap();
    /// assert!(fit.rms_residual < 1e-14);
    /// assert!((fit.series.eval(0.5) - 0.25).abs() < 1e-13);
    /// ```
    pub fn fit(order: usize, a: f64, b: f64, xs: &[f64], ys: &[f64]) -> Result<ChebFit> {
        if !(a < b) {
            // Matches `new`: cheb/init.c:70, "null function interval [a,b]".
            return Err(PetirError::Domain);
        }
        if xs.len() != ys.len() {
            return Err(PetirError::LengthMismatch {
                expected: xs.len(),
                found: ys.len(),
            });
        }
        let n_coeff = order + 1;
        let m = xs.len();
        if m < n_coeff {
            return Err(PetirError::Invalid);
        }

        // Design matrix: row i is T_0(t_i) .. T_order(t_i), with t_i the
        // sample mapped onto [-1, 1]. Built through the same recurrence the
        // rest of the crate evaluates with -- see `cheb_slice::basis_into`.
        let mut design = Matrix::zeros(m, n_coeff)?;
        let mut row = vec![0.0_f64; n_coeff];
        for (i, (&x, &y)) in zip_flat!(xs.iter(), ys.iter()).enumerate() {
            if !(x >= a && x <= b) || !y.is_finite() {
                // Also catches NaN, which compares false against everything.
                return Err(PetirError::Domain);
            }
            let t = crate::cheb_slice::scale(x, a, b);
            crate::cheb_slice::basis_into(t, &mut row);
            for (j, &tj) in row.iter().enumerate() {
                design.set(i, j, tj);
            }
        }

        let qr = QrDecomposition::new(design)?;
        let conditioning = qr.diagonal_ratio();
        let fit = qr.least_squares(ys)?;

        let mut sum_sq = 0.0_f64;
        for &r in &fit.residual {
            sum_sq += r * r;
        }
        let rms_residual = (sum_sq / (m as f64)).sqrt();

        // `least_squares` returns coefficients of T_0 .. T_order directly,
        // which is the PLAIN convention; `ChebSeries` stores GSL's, where the
        // zeroth is doubled and halved again on evaluation.
        let series = ChebSeries::from_plain_coefficients(fit.solution, a, b)?;

        Ok(ChebFit {
            series,
            residual: fit.residual,
            rms_residual,
            conditioning,
        })
    }
}
