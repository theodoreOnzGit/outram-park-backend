//! Derived log-error budgets for the `f32` mirrors (gh #1003). Test-only.
//!
//! **Why log error.** Maintainer direction, 2026-10-20: "for exp error
//! propagation you should measure by log error rather than error, to be
//! fair." An error `d` in the argument of `exp` comes out as a relative error
//! `d` in its result, so a quantity that passes through `exp` or `pow` is
//! judged by `|ln f_f32 - ln f_ref|`, and its budget is built from the
//! absolute error of each exponent: `|argument| * u` per rounding.
//!
//! **How the budget is built.** [`Sh`] runs one routine's arithmetic three
//! ways at once, operation by operation, in the shipped order:
//!
//! - `f`: the `f32` value, by the same `f32` operations the mirror performs
//!   (each test asserts `f` is bit-identical to the shipped function, so the
//!   model cannot drift from the code it describes);
//! - `v`: the same expression in `f64`, standing in for the exact value;
//! - `e`: an a-priori bound on `|f - exact|`, first order in `u = 2^-24`,
//!   every rounding at its bound and no cancellation between errors (a
//!   running error bound in Wilkinson's sense).
//!
//! The per-operation rules:
//!
//! | operation | `e` of the result |
//! |---|---|
//! | `a ± b` | `e_a + e_b + u|v|` |
//! | `a * b` | `|b| e_a + |a| e_b + u|v|` |
//! | `a / b` | `e_a/|b| + |v| e_b/|b| + u|v|` |
//! | `a * 2^k` | `2^k e_a` (exact) |
//! | `exp(a)` | `|v| e_a + ulp(v)`: the argument's absolute error becomes the result's relative error; `expf` faithful (TESTED) |
//! | `ln(a)` | `e_a/|a| + ulp(v)`; `logf` faithful (TESTED) |
//! | `sqrt(a)` | `|v| e_a/(2|a|) + u|v|`; correctly rounded by IEEE 754 |
//! | `powf(a, p')` | `|v| (|p' - p| |ln a| + |p| e_a/|a|) + ulp(v)`: the exponent's rounding times the size of the exp argument; `powf` faithful (TESTED) |
//! | Clenshaw | [`cheb_budget`] plus the truncation tail `sum |c_k|` over the coefficients the `f64` table carries and the `f32` one drops |
//!
//! `u|v|` bounds a round-to-nearest error, `ulp(v)/2 <= u|v|`, only for a
//! NORMAL result; for a subnormal (or flushed-to-zero) one [`Sh`] adds the
//! underflow term `2^-149`, Higham's `eta`, generously.
//!
//! A constant contributes its own rounding, `|c - exact|`, against the exact
//! value or the `f64` table entry it was rounded from; inside Clenshaw,
//! [`cheb_budget`] charges `u|c|` per coefficient instead.
//!
//! The log-error budget of a result is `-ln(1 - e/|v|)`.
//!
//! Left out everywhere: the `f64` reference's own error, and second-order
//! terms (`u^2`), both far below the budgets (1e-15 against 1e-7 and up).

#[allow(unused_imports)]
use crate::real::Real;

/// Unit roundoff of `f32`, `2^-24` (half of `f32::EPSILON`).
pub(crate) const U32: f64 = 5.960_464_477_539_063e-8;

/// `ulp(p) / p`: the relative size of one unit in the last place of `p`.
pub(crate) fn ulp_over(p: f32) -> f64 {
    let up = f32::from_bits(p.to_bits() + 1);
    f64::from(up - p) / f64::from(p)
}

/// One unit in the last place of `|p|`, measured upward (the larger of the
/// two ulps when `|p|` is a power of two).
pub(crate) fn ulp_abs(p: f32) -> f64 {
    let a = p.abs();
    f64::from(f32::from_bits(a.to_bits() + 1) - a)
}

/// `|ln(got / reference)|`, the log error.
pub(crate) fn log_err(got: f32, reference: f64) -> f64 {
    Real::ln(f64::from(got) / reference).abs()
}

/// `p` moved `k` ulps away from zero: the negative controls' defect.
pub(crate) fn ulps_up(p: f32, k: u32) -> f32 {
    f32::from_bits(p.to_bits() + k)
}

/// Clenshaw in GSL's convention (`x d - dd + c_0/2`), the operations every
/// `f32` mirror's `cheb` performs, in their order.
pub(crate) fn clenshaw_f32(c: &[f32], t: f32) -> f32 {
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * t;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    t * d - dd + 0.5 * c0
}

/// The `f32` Clenshaw sum of `c` at the exact `t`, evaluated in `f64`,
/// and an a-priori bound on how far `cheb(c, t_f32)` can sit from it.
///
/// Every term is a first-order worst case (each rounding at its bound,
/// no cancellation between them). It is computed from the `f64`
/// recurrence values, which are condition numbers, not errors:
///
/// - each step `d = fl(fl(fl(2t d) - dd) + c_k)` rounds three times,
///   locally `u (|2t d| + |2t d - dd| + |d_new|)`; an error injected
///   into `d_k` reaches the result multiplied by `T_k(t)` (because
///   `t U_{k-1} - U_{k-2} = T_k`), and `|T_k| <= 1` on `[-1, 1]`, so
///   the local errors add without amplification;
/// - the closing `t d_1 - d_2 + c_0/2` rounds three times likewise;
/// - the coefficients are `f32` roundings of GSL's `f64` table:
///   `u (|c_0|/2 + sum |c_k|)`;
/// - `t` itself is off by at most `dt`, which costs `|C'(t)| dt` with
///   `C' = sum k c_k U_{k-1}(t)`.
///
/// Moved here from `mirror_synchrotron` (gh #1003) so every mirror's budget
/// uses the one derivation.
pub(crate) fn cheb_budget(c: &[f32], t: f64, dt: f64) -> (f64, f64) {
    let (mut d, mut dd, mut err) = (0.0_f64, 0.0_f64, 0.0_f64);
    for &ck in c[1..].iter().rev() {
        let m = 2.0 * t * d;
        let s = m - dd;
        let next = s + f64::from(ck);
        err += U32 * (m.abs() + s.abs() + next.abs());
        dd = d;
        d = next;
    }
    let m = t * d;
    let s = m - dd;
    let value = s + 0.5 * f64::from(c[0]);
    err += U32 * (m.abs() + s.abs() + value.abs());
    let coef: f64 = c[1..].iter().map(|v| f64::from(*v).abs()).sum();
    err += U32 * (0.5 * f64::from(c[0]).abs() + coef);
    let (mut u_prev, mut u_k, mut slope) = (0.0_f64, 1.0_f64, 0.0_f64);
    for (k, &ck) in c.iter().enumerate().skip(1) {
        slope += k as f64 * f64::from(ck) * u_k;
        let u_next = 2.0 * t * u_k - u_prev;
        u_prev = u_k;
        u_k = u_next;
    }
    err += slope.abs() * dt;
    (value, err)
}

/// A value carried three ways: the shipped `f32` arithmetic (`f`), the same
/// expression in `f64` (`v`), and a bound on `|f - exact|` (`e`). See the
/// module documentation for the rules.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sh {
    pub f: f32,
    pub v: f64,
    pub e: f64,
}

impl Sh {
    /// A rounded result. Where `f` is subnormal, or zero from a non-zero
    /// `v`, `u|v|` no longer bounds the rounding, so the underflow term
    /// `2^-149` (one subnormal ulp) is added.
    fn rounded(f: f32, v: f64, e: f64) -> Self {
        let underflow = if f.is_subnormal() || (f == 0.0 && v != 0.0) {
            f64::from(f32::from_bits(1))
        } else {
            0.0
        };
        Sh {
            f,
            v,
            e: e + underflow,
        }
    }

    /// An exact `f32` input.
    pub fn exact(x: f32) -> Self {
        Sh {
            f: x,
            v: f64::from(x),
            e: 0.0,
        }
    }

    /// A named constant whose exact value is known: its error is its own
    /// rounding.
    pub fn constant(c: f32, exact: f64) -> Self {
        Sh {
            f: c,
            v: exact,
            e: (f64::from(c) - exact).abs(),
        }
    }

    /// `f` moved `k` ulps away from zero (toward it for negative `k`),
    /// everything else kept: how a negative control injects a defect the
    /// budget does not allow for.
    pub fn bump(self, k: i32) -> Self {
        let bits = i64::from(self.f.to_bits()) + i64::from(k);
        Sh {
            f: f32::from_bits(bits as u32),
            ..self
        }
    }

    pub fn add(self, b: Sh) -> Self {
        let v = self.v + b.v;
        Sh::rounded(self.f + b.f, v, self.e + b.e + U32 * v.abs())
    }

    pub fn sub(self, b: Sh) -> Self {
        let v = self.v - b.v;
        Sh::rounded(self.f - b.f, v, self.e + b.e + U32 * v.abs())
    }

    pub fn mul(self, b: Sh) -> Self {
        let v = self.v * b.v;
        let e = b.v.abs() * self.e + self.v.abs() * b.e + U32 * v.abs();
        Sh::rounded(self.f * b.f, v, e)
    }

    pub fn div(self, b: Sh) -> Self {
        let v = self.v / b.v;
        let e = self.e / b.v.abs() + v.abs() * b.e / b.v.abs() + U32 * v.abs();
        Sh::rounded(self.f / b.f, v, e)
    }

    /// Multiplication by a power of two, which is exact.
    pub fn scale(self, p: f32) -> Self {
        assert!(p.abs().to_bits() & 0x007f_ffff == 0, "{p} is not 2^k");
        Sh::rounded(
            self.f * p,
            self.v * f64::from(p),
            self.e * f64::from(p).abs(),
        )
    }

    pub fn neg(self) -> Self {
        Sh {
            f: -self.f,
            v: -self.v,
            e: self.e,
        }
    }

    /// `exp`, faithful: the argument's absolute error becomes the result's
    /// relative error, plus one ulp of its own.
    pub fn exp(self) -> Self {
        let f = Real::exp(self.f);
        let v = Real::exp(self.v);
        Sh::rounded(
            f,
            v,
            v.abs() * self.e + ulp_abs(f.abs().max(v.abs() as f32)),
        )
    }

    /// `ln`, faithful.
    pub fn ln(self) -> Self {
        let f = Real::ln(self.f);
        let v = Real::ln(self.v);
        Sh::rounded(
            f,
            v,
            self.e / self.v.abs() + ulp_abs(f.abs().max(v.abs() as f32)),
        )
    }

    /// `sqrt`, correctly rounded.
    pub fn sqrt(self) -> Self {
        let v = Real::sqrt(self.v);
        let e = v * self.e / (2.0 * self.v) + U32 * v;
        Sh::rounded(Real::sqrt(self.f), v, e)
    }

    /// `powf(self, p)`, faithful, with `p` the `f32` rounding of `p_exact`.
    pub fn powf(self, p: f32, p_exact: f64) -> Self {
        let f = Real::powf(self.f, p);
        let v = Real::powf(self.v, p_exact);
        let l = (f64::from(p) - p_exact).abs() * Real::ln(self.v).abs()
            + p_exact.abs() * self.e / self.v.abs();
        Sh::rounded(f, v, v.abs() * l + ulp_abs(f.abs().max(v.abs() as f32)))
    }

    /// GSL's Clenshaw on the `f32` table `c32` at `self`, against the `f64`
    /// table `c64` it was rounded and truncated from (GSL's single-precision
    /// order): [`cheb_budget`] plus the dropped coefficients, `|T_k| <= 1`.
    pub fn cheb(self, c32: &[f32], c64: &[f64]) -> Self {
        assert!(c64.len() >= c32.len());
        let (v, err) = cheb_budget(c32, self.v, self.e);
        let tail: f64 = c64[c32.len()..].iter().map(|c| c.abs()).sum();
        Sh::rounded(clenshaw_f32(c32, self.f), v, err + tail)
    }

    /// Horner, ascending coefficients, `c_i + x * ans` (GSL's
    /// `gsl_poly_eval`), on the `f32` table `c32`; each coefficient carries
    /// its exact distance from the `f64` table `c64` it was rounded from.
    pub fn horner(self, c32: &[f32], c64: &[f64]) -> Self {
        assert_eq!(c32.len(), c64.len());
        let Some((&last, head)) = c32.split_last() else {
            return Sh::exact(0.0);
        };
        let mut ans = Sh::constant(last, c64[head.len()]);
        for (i, &ci) in head.iter().enumerate().rev() {
            ans = Sh::constant(ci, c64[i]).add(self.mul(ans));
        }
        ans
    }

    /// The log-error budget, `-ln(1 - e/|v|)`; infinite when the bound
    /// reaches the value.
    pub fn log_bound(&self) -> f64 {
        let r = self.e / self.v.abs();
        if r >= 1.0 {
            f64::INFINITY
        } else {
            -Real::ln(1.0 - r)
        }
    }
}

/// The worst error of an `f32` routine against its `f64` counterpart, in
/// units of the `f32` result's ulp, over every `stride`-th `f32` in
/// `[lo, hi]` — the check behind every "faithful (TESTED)" in the table
/// above. Returns `(worst ulps, where)`.
pub(crate) fn worst_ulps(
    f: impl Fn(f32) -> f32,
    g: impl Fn(f64) -> f64,
    lo: f32,
    hi: f32,
    stride: i64,
) -> (f64, f32) {
    // Order-preserving map from f32 to integers, so negative ranges walk too.
    let key = |x: f32| -> i64 {
        let b = i64::from(x.to_bits() & 0x7fff_ffff);
        if x.is_sign_negative() {
            -b
        } else {
            b
        }
    };
    let unkey = |k: i64| -> f32 {
        if k < 0 {
            -f32::from_bits((-k) as u32)
        } else {
            f32::from_bits(k as u32)
        }
    };
    let (mut worst, mut at) = (0.0_f64, lo);
    let mut k = key(lo);
    while k <= key(hi) {
        let x = unkey(k);
        let exact = g(f64::from(x));
        let got = f64::from(f(x));
        // The ulp of the binade holding |exact| (not of its rounding, which
        // may sit one binade up and be twice as large).
        let mut q = exact.abs() as f32;
        if f64::from(q) > exact.abs() {
            q = f32::from_bits(q.to_bits() - 1);
        }
        let e = (got - exact).abs() / ulp_abs(q);
        if e > worst {
            worst = e;
            at = x;
        }
        k += stride;
    }
    (worst, at)
}
