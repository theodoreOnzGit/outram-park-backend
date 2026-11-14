//! QAWO and QAWF: adaptive integration of `f(x) cos(omega x)` or
//! `f(x) sin(omega x)` over a finite interval (QAWO) and over `[a, inf)`
//! (QAWF, the Fourier integral), ported from GSL 2.8 `integration/qawo.c`
//! (lines 30-460), `qawf.c` (lines 30-281), `qc25f.c` (lines 20-158) and
//! `qmomof.c` (lines 20-390), commit `cf180cd7`, read 2026-11-14. Copyright
//! (C) 1996-2000, 2007 Brian Gough (GPL-3.0-or-later); GSL's QAWO / QAWF are
//! QUADPACK's `dqawoe` / `dqawfe` (public domain). gh:#1340. Research,
//! education and V&V only.
//!
//! The weight is carried by a [`QawoTable`] (Chebyshev moments for
//! `levels` successive halvings of the interval length, `qmomof.c`).
//! Sub-intervals with `|omega| * half_length < 2` use the 15-point
//! Gauss-Kronrod rule on the weighted integrand; longer ones use the 25-point
//! Clenshaw-Curtis rule with the tabulated moments (`qc25f.c`). QAWO then
//! bisects with epsilon-algorithm extrapolation over the "large" intervals;
//! QAWF sums QAWO over successive half-periods and extrapolates the partial
//! sums.
//!
//! # Differences from upstream
//!
//! - The workspaces are allocated inside the call: `limit` entries for QAWO
//!   and for QAWF's interval list, `cycle_limit` entries for QAWF's per-cycle
//!   workspace. `limit == 0` is [`PetirError::Invalid`] (upstream would wrap
//!   `limit - 1`).
//! - Non-finite `a` is [`PetirError::Domain`] (upstream does not check).
//! - `GSL_ETABLE` (26, "exceeded limit of trigonometric table") has no
//!   [`PetirError`] variant (adding one edits the kovan-reviewed enum,
//!   deferred to the maintainer, gh:#1340); it is reported as
//!   [`PetirError::Failed`] with `gsl_status == 26`.
//! - `if (result != 0 && area != 0)` in upstream's QAWO tests the *pointer*
//!   `result`, which is never null, so only `area != 0` matters; the port
//!   keeps that behaviour (`area != 0.0`).
//! - Every array access is a checked `get`.

#[allow(unused_imports)]
use crate::real::Real;
use crate::scalar::{DBL_EPSILON, DBL_MIN};
use crate::{PetirError, Result};

use super::qagi::qagiu_with_status;
use super::qagp::append_interval;
use super::qags::{
    subinterval_too_small, test_positivity, ExtrapolationTable, Workspace,
};
use super::qcheb::qcheb;
use super::{kronrod, Integral, QkRule};

/// `GSL_DBL_MAX`.
const DBL_MAX: f64 = f64::MAX;

fn g(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

fn s(v: &mut [f64], i: usize, x: f64) {
    if let Some(p) = v.get_mut(i) {
        *p = x;
    }
}

/// `GSL_MAX_DBL`: `(a > b) ? a : b`, which returns `b` when either is NaN
/// (`f64::max` would return the other operand).
fn gsl_max(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// `enum gsl_integration_qawo_enum`: the trigonometric weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QawoKind {
    /// `GSL_INTEG_COSINE`.
    Cosine,
    /// `GSL_INTEG_SINE`.
    Sine,
}

/// `gsl_integration_qawo_table`: `omega`, the interval length `L` and the
/// Chebyshev moments of the weight for `levels` halvings of `L`.
#[derive(Debug, Clone, PartialEq)]
pub struct QawoTable {
    omega: f64,
    length: f64,
    par: f64,
    kind: QawoKind,
    chebmo: alloc::vec::Vec<[f64; 25]>,
}

impl QawoTable {
    /// `gsl_integration_qawo_table_alloc` (qmomof.c:30-85): `n` levels, the
    /// moments for level `i` computed at `par = omega L / 2 / 2^i`.
    ///
    /// # Errors
    ///
    /// [`PetirError::Domain`] if `n == 0` (`GSL_EDOM`).
    ///
    /// # Example
    ///
    /// ```
    /// use petir::integration::{qawo, QawoKind, QawoTable};
    /// // int_0^1 cos(10 x) dx = sin(10) / 10.
    /// let t = QawoTable::new(10.0, 1.0, QawoKind::Cosine, 10).unwrap();
    /// let r = qawo(|_x: f64| 1.0, 0.0, 0.0, 1e-10, 100, &t).unwrap();
    /// assert!((r.value - 10.0_f64.sin() / 10.0).abs() < 1e-10);
    /// ```
    pub fn new(omega: f64, length: f64, kind: QawoKind, n: usize) -> Result<Self> {
        if n == 0 {
            return Err(PetirError::Domain);
        }
        let mut t = Self {
            omega,
            length,
            par: 0.5 * omega * length,
            kind,
            chebmo: alloc::vec![[0.0; 25]; n],
        };
        t.recompute();
        Ok(t)
    }

    /// `gsl_integration_qawo_table_set` (qmomof.c:87-108).
    pub fn set(&mut self, omega: f64, length: f64, kind: QawoKind) {
        self.omega = omega;
        self.kind = kind;
        self.length = length;
        self.par = 0.5 * omega * length;
        self.recompute();
    }

    /// `gsl_integration_qawo_table_set_length` (qmomof.c:111-136): a no-op if
    /// `length` equals the current length.
    pub fn set_length(&mut self, length: f64) {
        if length == self.length {
            return;
        }
        self.length = length;
        self.par = 0.5 * self.omega * length;
        self.recompute();
    }

    /// The moment recomputation loop shared by the three setters.
    fn recompute(&mut self) {
        let mut scale = 1.0_f64;
        for level in self.chebmo.iter_mut() {
            *level = compute_moments(self.par * scale);
            scale *= 0.5;
        }
    }

    /// `omega`.
    pub fn omega(&self) -> f64 {
        self.omega
    }

    /// The interval length `L` the moments were computed for.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// The weight kind.
    pub fn kind(&self) -> QawoKind {
        self.kind
    }

    /// The number of levels `n`.
    pub fn levels(&self) -> usize {
        self.chebmo.len()
    }

    /// The 25 Chebyshev moments of level `level` (`None` past the table).
    pub fn moments(&self, level: usize) -> Option<&[f64; 25]> {
        self.chebmo.get(level)
    }
}

/// `qmomof.c`, `compute_moments`: the 25 Chebyshev moments of
/// `cos(par t)` (even slots) and `sin(par t)` (odd slots).
#[allow(clippy::too_many_lines)]
fn compute_moments(par: f64) -> [f64; 25] {
    let mut chebmo = [0.0_f64; 25];
    let mut v = [0.0_f64; 28];
    let mut d = [0.0_f64; 25];
    let mut d1 = [0.0_f64; 25];
    let mut d2 = [0.0_f64; 25];

    let noeq: usize = 25;

    let par2 = par * par;
    let par4 = par2 * par2;
    let par22 = par2 + 2.0;

    let sinpar = Real::sin(par);
    let cospar = Real::cos(par);

    // Compute the Chebyshev moments with respect to cosine.
    let mut ac = 8.0 * cospar;
    let mut as_ = 24.0 * par * sinpar;

    s(&mut v, 0, 2.0 * sinpar / par);
    s(&mut v, 1, (8.0 * cospar + (2.0 * par2 - 8.0) * sinpar / par) / par2);
    s(
        &mut v,
        2,
        (32.0 * (par2 - 12.0) * cospar + (2.0 * ((par2 - 80.0) * par2 + 192.0) * sinpar) / par)
            / par4,
    );

    if par.abs() <= 24.0 {
        // The moments as the solution of a boundary value problem using the
        // asymptotic expansion as an endpoint.
        let mut an = 6.0_f64;
        for k in 0..noeq - 1 {
            let an2 = an * an;
            s(&mut d, k, -2.0 * (an2 - 4.0) * (par22 - 2.0 * an2));
            s(&mut d2, k, (an - 1.0) * (an - 2.0) * par2);
            s(&mut d1, k + 1, (an + 3.0) * (an + 4.0) * par2);
            s(&mut v, k + 3, as_ - (an2 - 4.0) * ac);
            an += 2.0;
        }

        let an2 = an * an;

        s(&mut d, noeq - 1, -2.0 * (an2 - 4.0) * (par22 - 2.0 * an2));
        s(&mut v, noeq + 2, as_ - (an2 - 4.0) * ac);
        let v3 = g(&v, 3) - 56.0 * par2 * g(&v, 2);
        s(&mut v, 3, v3);

        let ass = par * sinpar;
        let asap = (((((210.0 * par2 - 1.0) * cospar - (105.0 * par2 - 63.0) * ass) / an2
            - (1.0 - 15.0 * par2) * cospar
            + 15.0 * ass)
            / an2
            - cospar
            + 3.0 * ass)
            / an2
            - cospar)
            / an2;
        let vl = g(&v, noeq + 2) - 2.0 * asap * par2 * (an - 1.0) * (an - 2.0);
        s(&mut v, noeq + 2, vl);

        if let Some(tail) = v.get_mut(3..) {
            dgtsl(noeq, &mut d1, &mut d, &mut d2, tail);
        }
    } else {
        // The moments by forward recursion.
        let mut an = 4.0_f64;
        for k in 3..13usize {
            let an2 = an * an;
            let vk = ((an2 - 4.0) * (2.0 * (par22 - 2.0 * an2) * g(&v, k - 1) - ac) + as_
                - par2 * (an + 1.0) * (an + 2.0) * g(&v, k - 2))
                / (par2 * (an - 1.0) * (an - 2.0));
            s(&mut v, k, vk);
            an += 2.0;
        }
    }

    for i in 0..13usize {
        s(&mut chebmo, 2 * i, g(&v, i));
    }

    // Compute the Chebyshev moments with respect to sine.
    s(&mut v, 0, 2.0 * (sinpar - par * cospar) / par2);
    s(
        &mut v,
        1,
        (18.0 - 48.0 / par2) * sinpar / par2 + (-2.0 + 48.0 / par2) * cospar / par,
    );

    ac = -24.0 * par * cospar;
    as_ = -8.0 * sinpar;

    if par.abs() <= 24.0 {
        let mut an = 5.0_f64;
        for k in 0..noeq - 1 {
            let an2 = an * an;
            s(&mut d, k, -2.0 * (an2 - 4.0) * (par22 - 2.0 * an2));
            s(&mut d2, k, (an - 1.0) * (an - 2.0) * par2);
            s(&mut d1, k + 1, (an + 3.0) * (an + 4.0) * par2);
            s(&mut v, k + 2, ac + (an2 - 4.0) * as_);
            an += 2.0;
        }

        let an2 = an * an;

        s(&mut d, noeq - 1, -2.0 * (an2 - 4.0) * (par22 - 2.0 * an2));
        s(&mut v, noeq + 1, ac + (an2 - 4.0) * as_);
        let v2 = g(&v, 2) - 42.0 * par2 * g(&v, 1);
        s(&mut v, 2, v2);

        let ass = par * cospar;
        let asap = (((((105.0 * par2 - 63.0) * ass - (210.0 * par2 - 1.0) * sinpar) / an2
            + (15.0 * par2 - 1.0) * sinpar
            - 15.0 * ass)
            / an2
            - sinpar
            - 3.0 * ass)
            / an2
            - sinpar)
            / an2;
        let vl = g(&v, noeq + 1) - 2.0 * asap * par2 * (an - 1.0) * (an - 2.0);
        s(&mut v, noeq + 1, vl);

        if let Some(tail) = v.get_mut(2..) {
            dgtsl(noeq, &mut d1, &mut d, &mut d2, tail);
        }
    } else {
        let mut an = 3.0_f64;
        for k in 2..12usize {
            let an2 = an * an;
            let vk = ((an2 - 4.0) * (2.0 * (par22 - 2.0 * an2) * g(&v, k - 1) + as_) + ac
                - par2 * (an + 1.0) * (an + 2.0) * g(&v, k - 2))
                / (par2 * (an - 1.0) * (an - 2.0));
            s(&mut v, k, vk);
            an += 2.0;
        }
    }

    for i in 0..12usize {
        s(&mut chebmo, 2 * i + 1, g(&v, i));
    }

    chebmo
}

/// `qmomof.c`, `dgtsl`: solve the tridiagonal system `A x = b` (LINPACK).
/// Returns `false` for `GSL_FAILURE` (a zero pivot).
fn dgtsl(n: usize, c: &mut [f64], d: &mut [f64], e: &mut [f64], b: &mut [f64]) -> bool {
    s(c, 0, g(d, 0));

    if n == 0 {
        return true;
    }

    if n == 1 {
        s(b, 0, g(b, 0) / g(d, 0));
        return true;
    }

    s(d, 0, g(e, 0));
    s(e, 0, 0.0);
    s(e, n - 1, 0.0);

    for k in 0..n - 1 {
        let k1 = k + 1;

        if g(c, k1).abs() >= g(c, k).abs() {
            let (t1, t0) = (g(c, k1), g(c, k));
            s(c, k1, t0);
            s(c, k, t1);
            let (t1, t0) = (g(d, k1), g(d, k));
            s(d, k1, t0);
            s(d, k, t1);
            let (t1, t0) = (g(e, k1), g(e, k));
            s(e, k1, t0);
            s(e, k, t1);
            let (t1, t0) = (g(b, k1), g(b, k));
            s(b, k1, t0);
            s(b, k, t1);
        }

        if g(c, k) == 0.0 {
            return false;
        }

        let t = -g(c, k1) / g(c, k);

        s(c, k1, g(d, k1) + t * g(d, k));
        s(d, k1, g(e, k1) + t * g(e, k));
        s(e, k1, 0.0);
        s(b, k1, g(b, k1) + t * g(b, k));
    }

    if g(c, n - 1) == 0.0 {
        return false;
    }

    s(b, n - 1, g(b, n - 1) / g(c, n - 1));

    s(b, n - 2, (g(b, n - 2) - g(d, n - 2) * g(b, n - 1)) / g(c, n - 2));

    let mut k = n;
    while k > 2 {
        let kb = k - 3;
        s(
            b,
            kb,
            (g(b, kb) - g(d, kb) * g(b, kb + 1) - g(e, kb) * g(b, kb + 2)) / g(c, kb),
        );
        k -= 1;
    }

    true
}

/// `qc25f.c`, `qc25f`: the 25-point Clenshaw-Curtis rule with the tabulated
/// moments, or the 15-point Kronrod rule on the weighted integrand when
/// `|omega| half_length < 2`. `(result, abserr, resabs, resasc)`; `None`
/// when `level` is past the table (upstream: `GSL_ESANITY`, outputs unset).
fn qc25f<F>(
    f: &F,
    a: f64,
    b: f64,
    wf: &QawoTable,
    level: usize,
) -> Option<(f64, f64, f64, f64)>
where
    F: Fn(f64) -> f64,
{
    let center = 0.5 * (a + b);
    let half_length = 0.5 * (b - a);
    let omega = wf.omega;

    let par = omega * half_length;

    if par.abs() < 2.0 {
        let r = if wf.kind == QawoKind::Sine {
            kronrod(
                QkRule::Qk15,
                |x: f64| {
                    let wx = omega * x;
                    let sinwx = Real::sin(wx);
                    f(x) * sinwx
                },
                a,
                b,
            )
        } else {
            kronrod(
                QkRule::Qk15,
                |x: f64| {
                    let wx = omega * x;
                    let coswx = Real::cos(wx);
                    f(x) * coswx
                },
                a,
                b,
            )
        };
        return Some((r.result, r.abserr, r.resabs, r.resasc));
    }

    let (cheb12, cheb24) = qcheb(f, a, b);

    let moment = wf.chebmo.get(level)?;

    let mut res12_cos = g(&cheb12, 12) * g(moment, 12);
    let mut res12_sin = 0.0_f64;

    for i in 0..6usize {
        let k = 10 - 2 * i;
        res12_cos += g(&cheb12, k) * g(moment, k);
        res12_sin += g(&cheb12, k + 1) * g(moment, k + 1);
    }

    let mut res24_cos = g(&cheb24, 24) * g(moment, 24);
    let mut res24_sin = 0.0_f64;

    let mut result_abs = g(&cheb24, 24).abs();

    for i in 0..12usize {
        let k = 22 - 2 * i;
        res24_cos += g(&cheb24, k) * g(moment, k);
        res24_sin += g(&cheb24, k + 1) * g(moment, k + 1);
        result_abs += g(&cheb24, k).abs() + g(&cheb24, k + 1).abs();
    }

    let est_cos = (res24_cos - res12_cos).abs();
    let est_sin = (res24_sin - res12_sin).abs();

    let c = half_length * Real::cos(center * omega);
    let sn = half_length * Real::sin(center * omega);

    let (result, abserr) = if wf.kind == QawoKind::Sine {
        (
            c * res24_sin + sn * res24_cos,
            (c * est_sin).abs() + (sn * est_cos).abs(),
        )
    } else {
        (
            c * res24_cos - sn * res24_sin,
            (c * est_cos).abs() + (sn * est_sin).abs(),
        )
    };

    Some((result, abserr, result_abs * half_length, DBL_MAX))
}

/// The outcome of [`qawo_with_status`] and [`qawf_with_status`]: GSL writes
/// `*result` and `*abserr` even when it returns an error status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QawoOutcome {
    /// `*result`, `*abserr`, and the interval-list size at exit.
    pub integral: Integral,
    /// `Ok(())` for `GSL_SUCCESS`, otherwise the error GSL returned (see the
    /// module notes for `GSL_ETABLE`).
    pub status: Result<()>,
    /// GSL's own integer status (0 success, 4 `EINVAL`, 5 `EFAILED`, 11
    /// `EMAXITER`, 13 `EBADTOL`, 18 `EROUND`, 21 `ESING`, 22 `EDIVERGE`, 26
    /// `ETABLE`; 1 `EDOM` for a non-finite `a`, PETIR only).
    pub gsl_status: i32,
}

fn failed(gsl_status: i32) -> QawoOutcome {
    let status = match gsl_status {
        0 => Ok(()),
        1 => Err(PetirError::Domain),
        4 => Err(PetirError::Invalid),
        11 => Err(PetirError::MaxIterations),
        13 => Err(PetirError::Tolerance),
        18 => Err(PetirError::RoundOff),
        21 => Err(PetirError::BadIntegrand),
        22 => Err(PetirError::Diverged),
        _ => Err(PetirError::Failed),
    };
    QawoOutcome {
        integral: Integral {
            value: 0.0,
            abserr: 0.0,
            subintervals: 0,
        },
        status,
        gsl_status,
    }
}

/// Upstream's `return_error` ladder for QAWO / QAWF: GSL's integer status
/// from `error_type` (after the `> 2` decrement).
fn status_of(error_type: i32) -> i32 {
    let e = if error_type > 2 { error_type - 1 } else { error_type };
    match e {
        0 => 0,
        1 => 11,
        2 => 18,
        3 => 21,
        4 => 18,
        5 => 22,
        -1 => 26,
        _ => 5,
    }
}

/// Integrate `f(x) cos(omega x)` or `f(x) sin(omega x)` over
/// `[a, a + L]` — GSL's `gsl_integration_qawo`, QUADPACK `dqawoe`. The
/// interval length `L` and `omega` come from `table`.
///
/// # Errors
///
/// As upstream: [`PetirError::Invalid`] for `limit == 0`,
/// [`PetirError::Tolerance`] (`GSL_EBADTOL`) for effectively zero tolerances,
/// [`PetirError::RoundOff`], [`PetirError::MaxIterations`],
/// [`PetirError::BadIntegrand`] (`GSL_ESING`), [`PetirError::Diverged`], and
/// [`PetirError::Failed`] for `GSL_ETABLE` (the table has too few levels for
/// the bisections requested). [`PetirError::Domain`] if `a` is not finite.
/// Use [`qawo_with_status`] to keep GSL's estimate when it fails.
///
/// # Example
///
/// ```
/// use petir::integration::{qawo, QawoKind, QawoTable};
/// let t = QawoTable::new(40.0, 1.0, QawoKind::Sine, 20).unwrap();
/// // int_0^1 x sin(40 x) dx = (sin 40 - 40 cos 40) / 1600.
/// let r = qawo(|x: f64| x, 0.0, 0.0, 1e-10, 200, &t).unwrap();
/// let want = (40.0_f64.sin() - 40.0 * 40.0_f64.cos()) / 1600.0;
/// assert!((r.value - want).abs() < 1e-9);
/// ```
pub fn qawo<F>(
    f: F,
    a: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
    table: &QawoTable,
) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qawo_with_status(f, a, epsabs, epsrel, limit, table);
    o.status.map(|()| o.integral)
}

/// [`qawo`], keeping GSL's estimate and integer status when it fails.
pub fn qawo_with_status<F>(
    f: F,
    a: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
    table: &QawoTable,
) -> QawoOutcome
where
    F: Fn(f64) -> f64,
{
    qawo_with_workspace(&f, a, epsabs, epsrel, limit, limit, table)
}

/// `gsl_integration_qawo` with a workspace of `workspace_limit` entries
/// (QAWF's cycle workspace may be larger than `limit`; `limit >
/// workspace_limit` is `GSL_EINVAL`).
pub(super) fn qawo_with_workspace<F>(
    f: &F,
    a: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
    workspace_limit: usize,
    table: &QawoTable,
) -> QawoOutcome
where
    F: Fn(f64) -> f64,
{
    if !a.is_finite() {
        return failed(1);
    }
    if limit == 0 || workspace_limit == 0 {
        return failed(4);
    }
    match qawo_core(f, a, epsabs, epsrel, limit, workspace_limit, table) {
        Some(o) => o,
        None => failed(4),
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments, clippy::float_cmp)]
fn qawo_core<F>(
    f: &F,
    a: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
    workspace_limit: usize,
    wf: &QawoTable,
) -> Option<QawoOutcome>
where
    F: Fn(f64) -> f64,
{
    let mut ertest = 0.0_f64;
    let mut error_over_large_intervals = 0.0_f64;
    let mut reseps = 0.0_f64;
    let mut abseps = 0.0_f64;
    let mut correc = 0.0_f64;
    let mut ktmin: usize = 0;
    let (mut roundoff_type1, mut roundoff_type2, mut roundoff_type3) = (0u32, 0u32, 0u32);
    let mut error_type: i32 = 0;
    let mut error_type2 = false;

    let mut extrapolate = false;
    let mut extall = false;
    let mut disallow_extrapolation = false;

    let b = a + wf.length;
    let abs_omega = wf.omega.abs();

    // Initialise results.
    let mut workspace = Workspace::new(workspace_limit, a, b)?;

    let out = |value: f64, abserr: f64, w: &Workspace, gsl_status: i32| {
        let mut o = failed(gsl_status);
        o.integral = Integral {
            value,
            abserr,
            subintervals: w.size,
        };
        o
    };

    if limit > workspace_limit {
        return Some(failed(4));
    }

    // Test on accuracy.
    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        let mut o = failed(13);
        o.integral.subintervals = workspace.size;
        return Some(o);
    }

    // Perform the first integration.
    let (result0, abserr0, resabs0, resasc0) = qc25f(f, a, b, wf, 0)?;

    workspace.set_initial_result(result0, abserr0)?;

    let mut tolerance = gsl_max(epsabs, epsrel * result0.abs());

    if abserr0 <= 100.0 * DBL_EPSILON * resabs0 && abserr0 > tolerance {
        return Some(out(result0, abserr0, &workspace, 18));
    } else if (abserr0 <= tolerance && abserr0 != resasc0) || abserr0 == 0.0 {
        return Some(out(result0, abserr0, &workspace, 0));
    } else if limit == 1 {
        return Some(out(result0, abserr0, &workspace, 11));
    }

    // Initialisation.
    let mut etable = ExtrapolationTable::new();

    if 0.5 * abs_omega * (b - a).abs() <= 2.0 {
        etable.append(result0)?;
        extall = true;
    }

    let mut area = result0;
    let mut errsum = abserr0;

    let mut res_ext = result0;
    let mut err_ext = DBL_MAX;

    let positive_integrand = test_positivity(result0, resabs0);

    let mut iteration: usize = 1;

    // `goto compute_result` in upstream.
    let mut compute_result = false;

    loop {
        // One pass of the do-while body; `break 'body` is C's `continue`
        // (jump to the `while (iteration < limit)` test).
        'body: {
            // Bisect the subinterval with the largest error estimate.
            let (a_i, b_i, r_i, e_i) = workspace.retrieve()?;

            let current_level = *workspace.level.get(workspace.i)? + 1;

            if current_level >= wf.levels() {
                error_type = -1; // exceeded limit of table
                // C `break`: leave the loop without the while test.
                iteration = usize::MAX;
                break 'body;
            }

            let a1 = a_i;
            let b1 = 0.5 * (a_i + b_i);
            let a2 = b1;
            let b2 = b_i;

            iteration += 1;

            let (area1, error1, resabs1, resasc1) = qc25f(f, a1, b1, wf, current_level)?;
            let (area2, error2, resabs2, resasc2) = qc25f(f, a2, b2, wf, current_level)?;
            let _ = (resabs1, resabs2);

            let area12 = area1 + area2;
            let error12 = error1 + error2;
            let last_e_i = e_i;

            // Written, as upstream, like QUADPACK so the rounding is the same.
            errsum = errsum + error12 - e_i;
            area = area + area12 - r_i;

            tolerance = gsl_max(epsabs, epsrel * area.abs());

            if resasc1 != error1 && resasc2 != error2 {
                let delta = r_i - area12;

                if delta.abs() <= 1.0e-5 * area12.abs() && error12 >= 0.99 * e_i {
                    if !extrapolate {
                        roundoff_type1 += 1;
                    } else {
                        roundoff_type2 += 1;
                    }
                }
                if iteration > 10 && error12 > e_i {
                    roundoff_type3 += 1;
                }
            }

            // Test for roundoff and eventually set the error flag.
            if roundoff_type1 + roundoff_type2 >= 10 || roundoff_type3 >= 20 {
                error_type = 2; // round off error
            }

            if roundoff_type2 >= 5 {
                error_type2 = true;
            }

            // Bad integrand behaviour at a point of the range.
            if subinterval_too_small(a1, a2, b2) {
                error_type = 4;
            }

            // Append the newly-created intervals to the list.
            workspace.update(a1, b1, area1, error1, a2, b2, area2, error2)?;

            if errsum <= tolerance {
                compute_result = true;
                iteration = usize::MAX;
                break 'body;
            }

            if error_type != 0 {
                iteration = usize::MAX;
                break 'body;
            }

            if iteration >= limit - 1 {
                error_type = 1;
                iteration = usize::MAX;
                break 'body;
            }

            // Set up variables on the first iteration.
            if iteration == 2 && extall {
                error_over_large_intervals = errsum;
                ertest = tolerance;
                etable.append(area)?;
                break 'body;
            }

            if disallow_extrapolation {
                break 'body;
            }

            let mut goto_label70 = false;
            if extall {
                error_over_large_intervals += -last_e_i;

                if current_level < workspace.maximum_level {
                    error_over_large_intervals += error12;
                }

                if extrapolate {
                    goto_label70 = true;
                }
            }

            if !goto_label70 {
                if workspace.large_interval()? {
                    break 'body;
                }

                if extall {
                    extrapolate = true;
                    workspace.nrmax = 1;
                } else {
                    // Is the interval to be bisected next the smallest?
                    let i = workspace.i;
                    let width = *workspace.blist.get(i)? - *workspace.alist.get(i)?;

                    if 0.25 * width.abs() * abs_omega > 2.0 {
                        break 'body;
                    }

                    extall = true;
                    error_over_large_intervals = errsum;
                    ertest = tolerance;
                    break 'body;
                }
            }

            // label70:
            if !error_type2 && error_over_large_intervals > ertest && workspace.increase_nrmax()? {
                break 'body;
            }

            // Perform extrapolation.
            etable.append(area)?;

            if etable.n < 3 {
                workspace.reset_nrmax()?;
                extrapolate = false;
                error_over_large_intervals = errsum;
                break 'body;
            }

            let (r, e) = etable.qelg()?;
            reseps = r;
            abseps = e;

            ktmin += 1;

            if ktmin > 5 && err_ext < 0.001 * errsum {
                error_type = 5;
            }

            if abseps < err_ext {
                ktmin = 0;
                err_ext = abseps;
                res_ext = reseps;
                correc = error_over_large_intervals;
                ertest = gsl_max(epsabs, epsrel * reseps.abs());
                if err_ext <= ertest {
                    iteration = usize::MAX;
                    break 'body;
                }
            }

            // Prepare bisection of the smallest interval.
            if etable.n == 1 {
                disallow_extrapolation = true;
            }

            if error_type == 5 {
                iteration = usize::MAX;
                break 'body;
            }

            // Work on the interval with the largest error.
            workspace.reset_nrmax()?;
            extrapolate = false;
            error_over_large_intervals = errsum;
        }
        // `iteration == usize::MAX` marks a C `break`/`goto` out of the loop.
        if iteration == usize::MAX || !(iteration < limit) {
            break;
        }
    }
    let _ = (reseps, abseps);

    let mut result;
    let mut abserr;

    if !compute_result {
        result = res_ext;
        abserr = err_ext;

        'ret: {
            if err_ext == DBL_MAX {
                compute_result = true;
                break 'ret;
            }

            if error_type != 0 || error_type2 {
                if error_type2 {
                    err_ext += correc;
                }

                if error_type == 0 {
                    error_type = 3;
                }

                // Upstream tests `result != 0` (a pointer: always true).
                if area != 0.0 {
                    if err_ext / res_ext.abs() > errsum / area.abs() {
                        compute_result = true;
                        break 'ret;
                    }
                } else if err_ext > errsum {
                    compute_result = true;
                    break 'ret;
                } else if area == 0.0 {
                    break 'ret;
                }
            }

            // Test on divergence.
            let max_area = gsl_max(res_ext.abs(), area.abs());

            if !positive_integrand && max_area < 0.01 * resabs0 {
                break 'ret;
            }

            let ratio = res_ext / area;

            if ratio < 0.01 || ratio > 100.0 || errsum > area.abs() {
                error_type = 6;
            }
        }
    } else {
        result = 0.0;
        abserr = 0.0;
    }

    if compute_result {
        result = workspace.sum_results();
        abserr = errsum;
    }

    Some(out(result, abserr, &workspace, status_of(error_type)))
}

/// Integrate `f(x) cos(omega x)` or `f(x) sin(omega x)` over `[a, inf)` —
/// GSL's `gsl_integration_qawf`, QUADPACK `dqawfe`. `table` supplies `omega`
/// and the kind; its length is reset to one period-sum cycle (so it is
/// mutated, as upstream). `limit` is the number of cycles and the size of the
/// interval list; `cycle_limit` is the size of the per-cycle QAWO workspace.
///
/// # Errors
///
/// As [`qawo`]; [`PetirError::Tolerance`] (`GSL_EBADTOL`) for
/// `epsabs <= 0`. A `cycle_limit` below `limit` makes every inner QAWO
/// return `GSL_EINVAL`, as upstream.
///
/// # Example
///
/// ```
/// use petir::integration::{qawf, QawoKind, QawoTable};
/// // int_0^inf e^-x cos(x) dx = 1/2.
/// let mut t = QawoTable::new(1.0, 1.0, QawoKind::Cosine, 25).unwrap();
/// let r = qawf(|x: f64| (-x).exp(), 0.0, 1e-10, 50, 50, &mut t).unwrap();
/// assert!((r.value - 0.5).abs() < 1e-9);
/// ```
pub fn qawf<F>(
    f: F,
    a: f64,
    epsabs: f64,
    limit: usize,
    cycle_limit: usize,
    table: &mut QawoTable,
) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    let o = qawf_with_status(f, a, epsabs, limit, cycle_limit, table);
    o.status.map(|()| o.integral)
}

/// [`qawf`], keeping GSL's estimate and integer status when it fails.
pub fn qawf_with_status<F>(
    f: F,
    a: f64,
    epsabs: f64,
    limit: usize,
    cycle_limit: usize,
    table: &mut QawoTable,
) -> QawoOutcome
where
    F: Fn(f64) -> f64,
{
    if !a.is_finite() {
        return failed(1);
    }
    if limit == 0 || cycle_limit == 0 {
        return failed(4);
    }
    match qawf_core(&f, a, epsabs, limit, cycle_limit, table) {
        Some(o) => o,
        None => failed(4),
    }
}

#[allow(clippy::too_many_lines, clippy::float_cmp)]
fn qawf_core<F>(
    f: &F,
    a: f64,
    epsabs: f64,
    limit: usize,
    cycle_limit: usize,
    wf: &mut QawoTable,
) -> Option<QawoOutcome>
where
    F: Fn(f64) -> f64,
{
    let mut ktmin: usize = 0;
    let mut total_error = 0.0_f64;
    let mut truncation_error = 0.0_f64;
    let mut correc = 0.0_f64;

    let omega = wf.omega;

    let p = 0.9_f64;
    let mut factor = 1.0_f64;
    let mut error_type: i32 = 0;

    // Initialise results.
    let mut workspace = Workspace::new(limit, a, a)?;

    let out = |value: f64, abserr: f64, w: &Workspace, gsl_status: i32| {
        let mut o = failed(gsl_status);
        o.integral = Integral {
            value,
            abserr,
            subintervals: w.size,
        };
        o
    };

    // Test on accuracy.
    if epsabs <= 0.0 {
        return Some(out(0.0, 0.0, &workspace, 13));
    }

    if omega == 0.0 {
        if wf.kind == QawoKind::Sine {
            // sin(w x) f(x) is always zero for w = 0.
            return Some(out(0.0, 0.0, &workspace, 0));
        }
        // cos(w x) f(x) is always f(x) for w = 0.
        let o = qagiu_with_status(f, a, epsabs, 0.0, cycle_limit);
        let mut r = failed(0);
        r.integral = Integral {
            value: o.integral.value,
            abserr: o.integral.abserr,
            subintervals: workspace.size,
        };
        r.status = o.status;
        r.gsl_status = match o.status {
            Ok(()) => 0,
            Err(PetirError::Tolerance) => 13,
            Err(PetirError::MaxIterations) => 11,
            Err(PetirError::RoundOff) => 18,
            Err(PetirError::BadIntegrand) => 21,
            Err(PetirError::Diverged) => 22,
            Err(PetirError::Invalid) => 4,
            Err(PetirError::Domain) => 1,
            Err(_) => 5,
        };
        return Some(r);
    }

    let mut eps = if epsabs > DBL_MIN / (1.0 - p) {
        epsabs * (1.0 - p)
    } else {
        epsabs
    };

    let initial_eps = eps;

    let mut area = 0.0_f64;
    let mut errsum = 0.0_f64;

    let mut res_ext = 0.0_f64;
    let mut err_ext = DBL_MAX;

    let cycle = (2.0 * Real::floor(omega.abs()) + 1.0) * core::f64::consts::PI / omega.abs();

    wf.set_length(cycle);

    let mut etable = ExtrapolationTable::new();

    let mut compute_result = false;
    let mut iteration: usize = 0;

    'cycles: while iteration < limit {
        let a1 = a + (iteration as f64) * cycle;
        let b1 = a1 + cycle;

        let epsabs1 = eps * factor;

        let inner = qawo_with_workspace(f, a1, epsabs1, 0.0, limit, cycle_limit, wf);
        let (area1, error1) = (inner.integral.value, inner.integral.abserr);
        let status = inner.gsl_status;

        append_interval(&mut workspace, a1, b1, area1, error1)?;

        factor *= p;

        area += area1;
        errsum += error1;

        // Estimate the truncation error as 50 times the final term.
        truncation_error = 50.0 * area1.abs();

        total_error = errsum + truncation_error;

        if total_error < epsabs && iteration > 4 {
            compute_result = true;
            break 'cycles;
        }

        if error1 > correc {
            correc = error1;
        }

        if status != 0 {
            eps = gsl_max(initial_eps, correc * (1.0 - p));
        }

        if status != 0 && total_error < 10.0 * correc && iteration > 3 {
            compute_result = true;
            break 'cycles;
        }

        etable.append(area)?;

        if etable.n < 2 {
            iteration += 1;
            continue 'cycles;
        }

        let (reseps, erreps) = etable.qelg()?;

        ktmin += 1;

        if ktmin >= 15 && err_ext < 0.001 * total_error {
            error_type = 4;
        }

        if erreps < err_ext {
            ktmin = 0;
            err_ext = erreps;
            res_ext = reseps;

            if err_ext + 10.0 * correc <= epsabs {
                break 'cycles;
            }
            if err_ext <= epsabs && 10.0 * correc >= epsabs {
                break 'cycles;
            }
        }

        iteration += 1;
    }

    let mut result = 0.0_f64;
    let mut abserr = 0.0_f64;

    if !compute_result {
        if iteration == limit {
            error_type = 1;
        }

        'ret: {
            if err_ext == DBL_MAX {
                compute_result = true;
                break 'ret;
            }

            err_ext += 10.0 * correc;

            result = res_ext;
            abserr = err_ext;

            if error_type == 0 {
                return Some(out(result, abserr, &workspace, 0));
            }

            if res_ext != 0.0 && area != 0.0 {
                if err_ext / res_ext.abs() > errsum / area.abs() {
                    compute_result = true;
                    break 'ret;
                }
            } else if err_ext > errsum {
                compute_result = true;
                break 'ret;
            } else if area == 0.0 {
                break 'ret;
            }

            if error_type == 4 {
                err_ext += truncation_error;
            }
        }
    }

    if compute_result {
        result = area;
        abserr = total_error;
    }
    let _ = err_ext;

    Some(out(result, abserr, &workspace, status_of(error_type)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_moments_match_the_closed_form_at_level_zero() {
        // Chebyshev moment 0 of cos(par t) over [-1, 1] is 2 sin(par)/par,
        // of sin(par t) it is 0 for an odd function times an even basis...
        // here the table stores v[0] = 2 sin(par)/par in the even slot 0.
        let t = QawoTable::new(4.0, 2.0, QawoKind::Cosine, 3).unwrap();
        let m = t.moments(0).unwrap();
        let par = 4.0_f64;
        assert!((m[0] - 2.0 * Real::sin(par) / par).abs() < 1e-14);
        assert!(t.moments(3).is_none());
        assert!(QawoTable::new(1.0, 1.0, QawoKind::Sine, 0).is_err());
        // Both moment branches (|par| <= 24 and > 24) are finite.
        let big = QawoTable::new(100.0, 1.0, QawoKind::Sine, 2).unwrap();
        assert!(big.moments(0).unwrap().iter().all(|x| x.is_finite()));
    }

    #[test]
    fn setters_recompute_the_moments() {
        let mut t = QawoTable::new(3.0, 1.0, QawoKind::Cosine, 2).unwrap();
        let before = *t.moments(0).unwrap();
        t.set_length(1.0);
        assert_eq!(before, *t.moments(0).unwrap());
        t.set_length(2.0);
        assert_ne!(before, *t.moments(0).unwrap());
        t.set(3.0, 1.0, QawoKind::Sine);
        assert_eq!(t.kind(), QawoKind::Sine);
        assert!((t.omega() - 3.0).abs() == 0.0 && (t.length() - 1.0).abs() == 0.0);
    }

    #[test]
    fn qawo_and_qawf_closed_forms() {
        let t = QawoTable::new(10.0, 1.0, QawoKind::Cosine, 12).unwrap();
        let r = qawo(|_x: f64| 1.0, 0.0, 0.0, 1e-10, 100, &t).unwrap();
        assert!((r.value - Real::sin(10.0) / 10.0).abs() < 1e-10);
        // A table with one level cannot bisect a long interval: ETABLE.
        let one = QawoTable::new(200.0, 1.0, QawoKind::Cosine, 1).unwrap();
        let o = qawo_with_status(|x: f64| Real::exp(x), 0.0, 0.0, 1e-12, 100, &one);
        assert_eq!(o.gsl_status, 26);
        assert_eq!(o.status, Err(PetirError::Failed));
        // QAWF: int_0^inf e^-x cos x = 1/2; sine with omega 0 is zero.
        let mut c = QawoTable::new(1.0, 1.0, QawoKind::Cosine, 25).unwrap();
        let r = qawf(|x: f64| Real::exp(-x), 0.0, 1e-10, 50, 50, &mut c).unwrap();
        assert!((r.value - 0.5).abs() < 1e-9);
        let mut z = QawoTable::new(0.0, 1.0, QawoKind::Sine, 5).unwrap();
        assert_eq!(qawf(|x: f64| x, 0.0, 1e-8, 10, 10, &mut z).unwrap().value, 0.0);
        let mut zc = QawoTable::new(0.0, 1.0, QawoKind::Cosine, 5).unwrap();
        let r = qawf(|x: f64| Real::exp(-x), 0.0, 1e-10, 10, 100, &mut zc).unwrap();
        assert!((r.value - 1.0).abs() < 1e-9);
        // Argument errors.
        assert_eq!(
            qawo_with_status(|x: f64| x, 0.0, 0.0, 0.0, 10, &t).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            qawf_with_status(|x: f64| x, 0.0, 0.0, 10, 10, &mut c).status,
            Err(PetirError::Tolerance)
        );
        assert_eq!(
            qawf_with_status(|x: f64| x, 0.0, 1e-8, 0, 10, &mut c).status,
            Err(PetirError::Invalid)
        );
        assert_eq!(
            qawo_with_status(|x: f64| x, f64::NAN, 0.0, 1e-8, 10, &t).status,
            Err(PetirError::Domain)
        );
    }
}
