//! Fixed-order Gaussian quadrature for nine weight functions — GSL's
//! `gsl_integration_fixed` — ported from GSL 2.8 `integration/fixed.c`
//! (lines 30-410), `legendre.c`, `chebyshev.c`, `chebyshev2.c`,
//! `gegenbauer.c`, `jacobi.c`, `laguerre.c`, `hermite.c`, `exponential.c` and
//! `rational.c`, commit `cf180cd7`, read 2026-11-14. GSL's code is based on
//! IQPACK (Elhay and Kautsky, ACM TOMS Algorithm 655) in John Burkardt's LGPL
//! C version; `imtqlx` is EISPACK's implicit QL algorithm, modified to carry
//! the first components of the eigenvectors. Copyright (C) 2015, 2016, 2017,
//! 2018, 2019, 2020, 2021 Patrick Alken (GPL-3.0-or-later) for GSL's wrapper;
//! the IQPACK routine is distributed under the GNU LGPL. gh:#1340.
//! Research, education and V&V only.
//!
//! For a weight `w(x)` on `(a, b)` and `n` points, the rule is exact for
//! polynomials of degree `2n - 1`: `int w(x) f(x) dx ~ sum_i w_i f(x_i)`. The
//! nodes are the eigenvalues, and the weights follow from the first
//! eigenvector components, of the Jacobi matrix of the orthogonal
//! polynomials; each [`FixedType`] supplies its matrix.
//!
//! | [`FixedType`] | weight `w(x)` | interval |
//! |---|---|---|
//! | `Legendre` | `1` | `[a, b]` |
//! | `Chebyshev` | `1 / sqrt((b - x)(x - a))` | `[a, b]` |
//! | `Gegenbauer` | `((b - x)(x - a))^alpha` | `[a, b]` |
//! | `Jacobi` | `(b - x)^alpha (x - a)^beta` | `[a, b]` |
//! | `Laguerre` | `(x - a)^alpha exp(-b (x - a))` | `[a, inf)` |
//! | `Hermite` | `|x - a|^alpha exp(-b (x - a)^2)` | `(-inf, inf)` |
//! | `Exponential` | `|x - (a + b)/2|^alpha` | `[a, b]` |
//! | `Rational` | `(x - a)^alpha (x + b)^beta` | `[a, inf)` |
//! | `Chebyshev2` | `sqrt((b - x)(x - a))` | `[a, b]` |
//!
//! # Differences from upstream
//!
//! - GSL's `gsl_integration_fixed_type` (a struct of two function pointers) is
//!   the enum [`FixedType`]; `check` and `init` are `match` arms.
//! - Upstream's `gsl_integration_fixed_alloc` returns `NULL` (with `GSL_EDOM`)
//!   for a failed `check`, a non-positive zeroth moment and a failed `imtqlx`
//!   alike; the port returns [`PetirError::Domain`] for all three.
//! - `gsl_sf_gamma` is [`crate::specfunc::gamma`] (a port, compared with GSL
//!   in `tests/gsl_specfunc_code_to_code.rs`); `pow` is `Real::powf`.
//! - Every array access is a checked `get`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::DBL_EPSILON;
use crate::specfunc::gamma;
use crate::{PetirError, Result};

/// `M_SQRTPI`, `gsl_math.h`.
const SQRT_PI: f64 = 1.772_453_850_905_516_027_298_167_483_34;

/// The nine weight functions of `gsl_integration_fixed_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedType {
    /// `gsl_integration_fixed_legendre`.
    Legendre,
    /// `gsl_integration_fixed_chebyshev`.
    Chebyshev,
    /// `gsl_integration_fixed_gegenbauer`.
    Gegenbauer,
    /// `gsl_integration_fixed_jacobi`.
    Jacobi,
    /// `gsl_integration_fixed_laguerre`.
    Laguerre,
    /// `gsl_integration_fixed_hermite`.
    Hermite,
    /// `gsl_integration_fixed_exponential`.
    Exponential,
    /// `gsl_integration_fixed_rational`.
    Rational,
    /// `gsl_integration_fixed_chebyshev2`.
    Chebyshev2,
}

/// `gsl_integration_fixed_params`.
struct Params {
    a: f64,
    b: f64,
    alpha: f64,
    beta: f64,
    zemu: f64,
    shft: f64,
    slp: f64,
    al: f64,
    be: f64,
}

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(p) = v.get_mut(i) {
        *p = x;
    }
}

impl FixedType {
    /// The `*_check` function: `Ok(())` for acceptable parameters.
    #[allow(clippy::float_cmp)]
    fn check(self, n: usize, p: &Params) -> Result<()> {
        let narrow = (p.b - p.a).abs() <= DBL_EPSILON;
        let reversed = p.a >= p.b;
        let ok = match self {
            FixedType::Legendre => !narrow,
            FixedType::Chebyshev | FixedType::Chebyshev2 => !narrow && !reversed,
            FixedType::Gegenbauer | FixedType::Exponential => {
                !narrow && !reversed && !(p.alpha <= -1.0)
            }
            FixedType::Jacobi => !narrow && !reversed && !(p.alpha <= -1.0 || p.beta <= -1.0),
            FixedType::Laguerre | FixedType::Hermite => !(p.b <= 0.0) && !(p.alpha <= -1.0),
            FixedType::Rational => {
                let two_n = (2 * n) as f64;
                !narrow
                    && !(p.alpha <= -1.0)
                    && !(p.beta >= 0.0 || p.alpha + p.beta + two_n >= 0.0 || 0.0 >= p.alpha + two_n)
                    && !(p.a + p.b <= 0.0)
            }
        };
        if ok {
            Ok(())
        } else {
            Err(PetirError::Domain)
        }
    }

    /// The `*_init` function: fill the Jacobi matrix and the scaling.
    #[allow(clippy::too_many_lines)]
    fn init(self, n: usize, diag: &mut [f64], subdiag: &mut [f64], p: &mut Params) {
        match self {
            FixedType::Legendre => {
                for i in 1..=n {
                    let fi = i as f64;
                    put(diag, i - 1, 0.0);
                    put(subdiag, i - 1, fi / Real::sqrt(4.0 * fi * fi - 1.0));
                }
                p.zemu = 2.0;
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = 0.0;
                p.be = 0.0;
            }
            FixedType::Chebyshev => {
                put(diag, 0, 0.0);
                put(subdiag, 0, core::f64::consts::FRAC_1_SQRT_2);
                for i in 1..n {
                    put(diag, i, 0.0);
                    put(subdiag, i, 0.5);
                }
                p.zemu = core::f64::consts::PI;
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = -0.5;
                p.be = -0.5;
            }
            FixedType::Chebyshev2 => {
                for i in 0..n {
                    put(diag, i, 0.0);
                    put(subdiag, i, 0.5);
                }
                p.zemu = core::f64::consts::FRAC_PI_2;
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = 0.5;
                p.be = 0.5;
            }
            FixedType::Gegenbauer => {
                put(diag, 0, 0.0);
                put(subdiag, 0, Real::sqrt(1.0 / (2.0 * p.alpha + 3.0)));
                for i in 1..n {
                    let fi = i as f64;
                    put(diag, i, 0.0);
                    put(
                        subdiag,
                        i,
                        Real::sqrt(
                            (fi + 1.0) * (2.0 * p.alpha + fi + 1.0)
                                / (4.0 * (p.alpha + fi + 1.0) * (p.alpha + fi + 1.0) - 1.0),
                        ),
                    );
                }
                p.zemu = SQRT_PI * gamma(p.alpha + 1.0) / gamma(p.alpha + 1.5);
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = p.alpha;
                p.be = p.alpha;
            }
            FixedType::Jacobi => {
                let absum = p.beta + p.alpha;
                let abdiff = p.beta - p.alpha;
                let a2b2 = absum * abdiff; // beta^2 - alpha^2
                put(diag, 0, abdiff / (absum + 2.0));
                put(
                    subdiag,
                    0,
                    2.0 * Real::sqrt((p.alpha + 1.0) * (p.beta + 1.0) / (absum + 3.0))
                        / (absum + 2.0),
                );
                for i in 1..n {
                    let fi = i as f64;
                    put(
                        diag,
                        i,
                        a2b2 / ((absum + 2.0 * fi) * (absum + 2.0 * fi + 2.0)),
                    );
                    put(
                        subdiag,
                        i,
                        Real::sqrt(
                            4.0 * (fi + 1.0) * (p.alpha + fi + 1.0) * (p.beta + fi + 1.0)
                                * (absum + fi + 1.0)
                                / (Real::powf(absum + 2.0 * fi + 2.0, 2.0) - 1.0),
                        ) / (absum + 2.0 * fi + 2.0),
                    );
                }
                p.zemu = Real::powf(2.0, absum + 1.0) * gamma(p.alpha + 1.0) * gamma(p.beta + 1.0)
                    / gamma(absum + 2.0);
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = p.alpha;
                p.be = p.beta;
            }
            FixedType::Laguerre => {
                for i in 0..n {
                    let fi = i as f64;
                    put(diag, i, 2.0 * fi + 1.0 + p.alpha);
                    put(subdiag, i, Real::sqrt((fi + 1.0) * (p.alpha + fi + 1.0)));
                }
                p.zemu = gamma(p.alpha + 1.0);
                p.shft = p.a;
                p.slp = 1.0 / p.b;
                p.al = p.alpha;
                p.be = 0.0;
            }
            FixedType::Hermite => {
                for i in 1..=n {
                    let fi = i as f64;
                    put(diag, i - 1, 0.0);
                    put(
                        subdiag,
                        i - 1,
                        Real::sqrt(0.5 * (fi + p.alpha * ((i % 2) as f64))),
                    );
                }
                p.zemu = gamma(0.5 * (p.alpha + 1.0));
                p.shft = p.a;
                p.slp = 1.0 / Real::sqrt(p.b);
                p.al = p.alpha;
                p.be = 0.0;
            }
            FixedType::Exponential => {
                let mut a2i = p.alpha;
                for i in 1..=n {
                    let fi = i as f64;
                    put(diag, i - 1, 0.0);
                    a2i += 2.0;
                    put(
                        subdiag,
                        i - 1,
                        (fi + p.alpha * ((i % 2) as f64)) / Real::sqrt(a2i * a2i - 1.0),
                    );
                }
                p.zemu = 2.0 / (p.alpha + 1.0);
                p.shft = 0.5 * (p.b + p.a);
                p.slp = 0.5 * (p.b - p.a);
                p.al = p.alpha;
                p.be = 0.0;
            }
            FixedType::Rational => {
                let absum = p.beta + p.alpha;
                let a1 = p.alpha + 1.0;
                let aba1 = absum * a1;
                let mut ab2i = absum + 2.0;
                let fnn = n as f64;

                put(diag, 0, -a1 / (absum + 2.0));
                put(
                    subdiag,
                    0,
                    Real::sqrt(-at(diag, 0) * (p.beta + 1.0) / ((absum + 2.0) * (absum + 3.0))),
                );

                let mut i = 1usize;
                while i + 1 < n {
                    let fi = i as f64;
                    ab2i += 2.0;
                    put(
                        diag,
                        i,
                        (-aba1 - 2.0 * fi * (absum + fi + 1.0)) / (ab2i * (ab2i - 2.0)),
                    );
                    put(
                        subdiag,
                        i,
                        Real::sqrt(
                            (fi + 1.0) * (p.alpha + fi + 1.0) / (ab2i - 1.0) * (p.beta + fi + 1.0)
                                / (ab2i * ab2i)
                                * (absum + fi + 1.0)
                                / (ab2i + 1.0),
                        ),
                    );
                    i += 1;
                }

                put(
                    diag,
                    n - 1,
                    (-aba1 - 2.0 * (fnn - 1.0) * (absum + fnn))
                        / ((absum + 2.0 * fnn) * (absum + 2.0 * fnn - 2.0)),
                );
                put(subdiag, n - 1, 0.0);

                p.zemu = gamma(p.alpha + 1.0) * gamma(-absum - 1.0) / gamma(-p.beta);
                p.shft = p.a;
                p.slp = p.b + p.a;
                p.al = p.alpha;
                p.be = p.beta;
            }
        }
    }
}

/// `imtqlx`: diagonalise the symmetric tridiagonal matrix (`d`, `e`) by the
/// implicit QL algorithm, applying the rotations to `z`; the eigenvalues end
/// up in `d` in ascending order and `z` holds `Q' z`. IQPACK, EISPACK;
/// Burkardt's C version.
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
fn imtqlx(n: usize, d: &mut [f64], e: &mut [f64], z: &mut [f64]) -> Result<()> {
    let itn: usize = 30;

    if n == 1 {
        return Ok(());
    }

    put(e, n - 1, 0.0);

    for l in 1..=n {
        let mut j: usize = 0;
        loop {
            let mut m = l;
            while m <= n {
                if m == n {
                    break;
                }
                if at(e, m - 1).abs() <= DBL_EPSILON * (at(d, m - 1).abs() + at(d, m).abs()) {
                    break;
                }
                m += 1;
            }
            let mut p = at(d, l - 1);
            if m == l {
                break;
            }
            if itn <= j {
                return Err(PetirError::MaxIterations);
            }
            j += 1;
            let mut g = (at(d, l) - p) / (2.0 * at(e, l - 1));
            let mut r = Real::sqrt(g * g + 1.0);
            let sign = if g >= 0.0 { 1.0 } else { -1.0 };
            g = at(d, m - 1) - p + at(e, l - 1) / (g + r.abs() * sign);
            let mut s = 1.0_f64;
            let mut c = 1.0_f64;
            p = 0.0;
            let mml = m - l;

            for ii in 1..=mml {
                let i = m - ii;
                let mut f = s * at(e, i - 1);
                let b = c * at(e, i - 1);

                if g.abs() <= f.abs() {
                    c = g / f;
                    r = Real::sqrt(c * c + 1.0);
                    put(e, i, f * r);
                    s = 1.0 / r;
                    c *= s;
                } else {
                    s = f / g;
                    r = Real::sqrt(s * s + 1.0);
                    put(e, i, g * r);
                    c = 1.0 / r;
                    s *= c;
                }
                g = at(d, i) - p;
                r = (at(d, i - 1) - g) * s + 2.0 * c * b;
                p = s * r;
                put(d, i, g + p);
                g = c * r - b;
                f = at(z, i);
                put(z, i, s * at(z, i - 1) + c * f);
                put(z, i - 1, c * at(z, i - 1) - s * f);
            }
            put(d, l - 1, at(d, l - 1) - p);
            put(e, l - 1, g);
            put(e, m - 1, 0.0);
        }
    }

    // Sorting. (Upstream's `m` here is the value left by the last pass,
    // which is `n`.)
    for ii in 2..=n {
        let i = ii - 1;
        let mut k = i;
        let mut p = at(d, i - 1);

        for j in ii..=n {
            if at(d, j - 1) < p {
                k = j;
                p = at(d, j - 1);
            }
        }

        if k != i {
            put(d, k - 1, at(d, i - 1));
            put(d, i - 1, p);
            let t = at(z, i - 1);
            put(z, i - 1, at(z, k - 1));
            put(z, k - 1, t);
        }
    }

    Ok(())
}

/// `gsl_integration_fixed_workspace`: the nodes and weights of one rule.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedWorkspace {
    kind: FixedType,
    x: Vec<f64>,
    weights: Vec<f64>,
}

impl FixedWorkspace {
    /// `gsl_integration_fixed_alloc` followed by `fixed_compute`
    /// (fixed.c:34-93 and 150-215): the `n`-point rule of weight `kind` on
    /// `(a, b)`, with parameters `alpha` and `beta` where the weight uses
    /// them (see the table in the module notes; unused ones are ignored but
    /// still enter the checks as upstream's do).
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] for `n == 0`, parameters the type refuses
    /// (`|b - a|` too small, `a >= b` where required, `alpha`/`beta <= -1`,
    /// `b <= 0` for Laguerre and Hermite, the Rational conditions), a
    /// non-positive zeroth moment, or an `imtqlx` that does not converge.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::integration::{FixedType, FixedWorkspace};
    /// // 3-point Gauss-Legendre on [0, 1] integrates x^5 exactly (= 1/6).
    /// let w = FixedWorkspace::new(FixedType::Legendre, 3, 0.0, 1.0, 0.0, 0.0).unwrap();
    /// let i = w.integrate(|x: f64| x.powi(5));
    /// assert!((i - 1.0 / 6.0).abs() < 1e-15);
    /// ```
    pub fn new(kind: FixedType, n: usize, a: f64, b: f64, alpha: f64, beta: f64) -> Result<Self> {
        if n < 1 {
            return Err(PetirError::Domain);
        }
        let mut p = Params {
            a,
            b,
            alpha,
            beta,
            zemu: 0.0,
            shft: 0.0,
            slp: 0.0,
            al: 0.0,
            be: 0.0,
        };
        let mut diag = vec![0.0_f64; n];
        let mut subdiag = vec![0.0_f64; n];
        let mut weights = vec![0.0_f64; n];

        // Check input parameters; initialise the Jacobi matrix.
        kind.check(n, &p)?;
        kind.init(n, &mut diag, &mut subdiag, &mut p);

        if p.zemu <= 0.0 {
            return Err(PetirError::Domain);
        }

        let mut x = diag;
        put(&mut weights, 0, Real::sqrt(p.zemu));

        // Diagonalise the Jacobi matrix.
        imtqlx(n, &mut x, &mut subdiag, &mut weights).map_err(|_| PetirError::Domain)?;

        for w in weights.iter_mut() {
            *w = *w * *w;
        }

        // The weights and nodes are valid for a = 0, b = 1: scale them.
        let pw = Real::powf(p.slp, p.al + p.be + 1.0);
        for (xk, wk) in x.iter_mut().zip(weights.iter_mut()) {
            *xk = p.shft + p.slp * *xk;
            *wk *= pw;
        }

        Ok(Self { kind, x, weights })
    }

    /// `gsl_integration_fixed_n`.
    pub fn n(&self) -> usize {
        self.x.len()
    }

    /// `gsl_integration_fixed_nodes`.
    pub fn nodes(&self) -> &[f64] {
        &self.x
    }

    /// `gsl_integration_fixed_weights`.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// The weight function this rule was built for.
    pub fn kind(&self) -> FixedType {
        self.kind
    }

    /// `gsl_integration_fixed`: `sum_i weights[i] * f(x[i])`, accumulated in
    /// node order.
    pub fn integrate<F>(&self, f: F) -> f64
    where
        F: Fn(f64) -> f64,
    {
        let mut sum = 0.0_f64;
        for (xi, wi) in self.x.iter().zip(self.weights.iter()) {
            let fi = f(*xi);
            sum += *wi * fi;
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_integrates_its_weight_moment_exactly() {
        let n = 6usize;
        // int 1 w(x) dx is the zeroth moment: the weights sum to it.
        let sum = |k: FixedType, a, b, al, be| {
            FixedWorkspace::new(k, n, a, b, al, be).unwrap().integrate(|_| 1.0)
        };
        assert!((sum(FixedType::Legendre, -1.0, 1.0, 0.0, 0.0) - 2.0).abs() < 1e-14);
        assert!((sum(FixedType::Chebyshev, -1.0, 1.0, 0.0, 0.0) - core::f64::consts::PI).abs() < 1e-13);
        assert!(
            (sum(FixedType::Chebyshev2, -1.0, 1.0, 0.0, 0.0) - core::f64::consts::FRAC_PI_2).abs()
                < 1e-13
        );
        // Gegenbauer alpha = 0 is Legendre; Jacobi (0,0) likewise.
        assert!((sum(FixedType::Gegenbauer, -1.0, 1.0, 0.0, 0.0) - 2.0).abs() < 1e-13);
        assert!((sum(FixedType::Jacobi, -1.0, 1.0, 0.0, 0.0) - 2.0).abs() < 1e-13);
        // Laguerre alpha = 0, a = 0, b = 1: int_0^inf e^-x = 1.
        assert!((sum(FixedType::Laguerre, 0.0, 1.0, 0.0, 0.0) - 1.0).abs() < 1e-13);
        // Hermite alpha = 0: int e^-x^2 = sqrt(pi).
        assert!((sum(FixedType::Hermite, 0.0, 1.0, 0.0, 0.0) - SQRT_PI).abs() < 1e-13);
        // Exponential alpha = 0 on [0, 1]: int |x - 1/2|^0 = 1... zemu 2/(alpha+1)
        // times slp = 1/2 gives 1.
        assert!((sum(FixedType::Exponential, 0.0, 1.0, 0.0, 0.0) - 1.0).abs() < 1e-13);
        // Rational: needs beta < alpha + beta + 2n < 0.
        let r = FixedWorkspace::new(FixedType::Rational, n, 1.0, 2.0, 0.0, -20.0).unwrap();
        assert!(r.integrate(|_| 1.0).is_finite() && r.n() == n && r.kind() == FixedType::Rational);
        // Exactness: 3 points integrate x^5 on [0, 1].
        let w = FixedWorkspace::new(FixedType::Legendre, 3, 0.0, 1.0, 0.0, 0.0).unwrap();
        assert!((w.integrate(|x: f64| x * x * x * x * x) - 1.0 / 6.0).abs() < 1e-15);
        assert_eq!(w.nodes().len(), 3);
        assert!((w.weights().iter().sum::<f64>() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn bad_parameters_are_refused() {
        let e = |k, n, a, b, al, be| FixedWorkspace::new(k, n, a, b, al, be).err();
        assert_eq!(e(FixedType::Legendre, 0, -1.0, 1.0, 0.0, 0.0), Some(PetirError::Domain));
        assert!(e(FixedType::Legendre, 3, 1.0, 1.0, 0.0, 0.0).is_some());
        assert!(e(FixedType::Chebyshev, 3, 1.0, -1.0, 0.0, 0.0).is_some());
        assert!(e(FixedType::Gegenbauer, 3, -1.0, 1.0, -1.0, 0.0).is_some());
        assert!(e(FixedType::Jacobi, 3, -1.0, 1.0, 0.0, -1.0).is_some());
        assert!(e(FixedType::Laguerre, 3, 0.0, 0.0, 0.0, 0.0).is_some());
        assert!(e(FixedType::Hermite, 3, 0.0, 1.0, -1.5, 0.0).is_some());
        assert!(e(FixedType::Exponential, 3, 1.0, 0.0, 0.0, 0.0).is_some());
        assert!(e(FixedType::Rational, 3, 1.0, 2.0, 0.0, 1.0).is_some());
        assert!(e(FixedType::Rational, 3, -1.0, 0.5, 0.0, -20.0).is_some());
    }

    #[test]
    fn one_point_rules_and_imtqlx_edge() {
        let w = FixedWorkspace::new(FixedType::Legendre, 1, -1.0, 1.0, 0.0, 0.0).unwrap();
        assert_eq!(w.nodes(), &[0.0]);
        assert!((w.weights()[0] - 2.0).abs() < 1e-15);
        let r = FixedWorkspace::new(FixedType::Rational, 1, 1.0, 2.0, 0.0, -5.0).unwrap();
        assert_eq!(r.n(), 1);
    }
}
