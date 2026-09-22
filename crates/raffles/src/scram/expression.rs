// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/expression/exponential.{h,cc}, src/expression/
//                     constant.{h,cc}, src/expression/numerical.{h,cc},
//                     src/expression/boolean.h, src/expression/
//                     conditional.{h,cc}, src/expression/
//                     random_deviate.{h,cc}, src/expression.{h,cc}
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: every probability formula is ported arithmetic-for-
// arithmetic, including the `p_exp` helpers and the `lambda == mu` branch
// that avoids a zero denominator. Upstream's class hierarchy
// (`Expression` -> `ExpressionFormula<T>` -> each operator, with a
// `flavor_` pointer selecting one of `PeriodicTest`'s three forms) becomes
// one [`Expression`] enum dispatched by `match`, per the workspace
// no-trait-objects rule; `PeriodicTest`'s flavour is chosen by which variant
// is constructed, so an ill-formed combination is unrepresentable rather
// than checked.
//
// NOT ported: `Expression::Sample()` and each deviate's `DoSample()`, which
// only the uncertainty analysis calls and which land with it — implementing
// them now would mean implementing them with nothing to check them against.
// Also not ported: `extern.{h,cc}`, which loads shared libraries (barred by
// the workspace's "no autonomous access" rule), `Switch`, and the
// trigonometric operators, none of which appears in any upstream input model.
//
// ~~and upstream's `Interval` arithmetic, which exists to validate sampled
// ranges and has no consumer here yet~~ **CORRECTED 2026-09-22** — it has a
// consumer now. The random deviates are what make an expression's *domain*
// differ from its value, and upstream's `EnsureNonNegative`/`EnsureWithin`
// check both. [`Interval`] and [`Expression::interval`] are ported with
// them.
//
// Upstream's `Validate()` checks are ported as [`Expression::validate`].
// ---------------------------------------------------------------------------

//! Expressions — how a basic event's probability is *computed* rather than
//! stated.
//!
//! A fault tree's leaves are rarely given as bare numbers. A component with a
//! failure rate and a mission time has an **exponential** probability; one
//! that is tested periodically and repaired has a probability that saws up
//! and down with the test interval. The Model Exchange Format writes these as
//! expression trees, and this module evaluates them.
//!
//! Two of the fixture's models need this to be read at all: `HIPPS` defines
//! every basic event by `<periodic-test>` or `<GLM>`, and its probabilities
//! appear nowhere in the input as literals — SCRAM computes them. Reproducing
//! those exact values is the verification, and
//! `tests/scram_expressions.rs` does it.
//!
//! # Random deviates
//!
//! The seven `<*-deviate>` and `<histogram>` elements are here too. Each has
//! two faces upstream: `value()` is the distribution's **mean**, which is
//! what an ordinary SCRAM run computes and prints, and `DoSample()` draws
//! from it, which only the uncertainty analysis calls. **This module
//! implements the first.** Sampling lands with the uncertainty analysis,
//! where `scram --uncertainty` is the oracle for it;
//! [`Expression::is_deviate`] is the hook that analysis will use and is
//! implemented now because it is checkable now.
//!
//! Their arrival is also what gives [`Expression::interval`] a purpose: a
//! normal deviate with a comfortable mean has a domain reaching six sigma
//! below it, and upstream rejects such an argument on the *domain* half of
//! its validation even though the value passes.
//!
//! # Example
//!
//! ```
//! use raffles::scram::expression::Expression;
//!
//! // A component with failure rate 1e-6/h over a 1000 h mission.
//! let e = Expression::exponential(
//!     Expression::float(1e-6),
//!     Expression::float(1000.0),
//! );
//! let p = e.value().unwrap();
//! assert!((p - (1.0 - (-1e-3f64).exp())).abs() < 1e-15);
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::{RafflesError, Result};

/// SCRAM's default mission time, in hours.
///
/// Upstream `Settings::mission_time_`; a model that does not set one gets
/// this, and `<system-mission-time/>` resolves to it.
pub const DEFAULT_MISSION_TIME: f64 = 8760.0;

/// A named parameter's value, resolved when an expression is evaluated.
///
/// Upstream models parameters as first-class `Parameter` expressions in a
/// graph with cycle detection (`src/parameter.{h,cc}`, `src/cycle.h`). Here a
/// parameter is a *name* looked up in this table, which keeps [`Expression`]
/// a tree rather than a graph — the cycle it could otherwise form is
/// unrepresentable instead of detected.
pub type Parameters = HashMap<String, f64>;

/// How a quantity is computed.
///
/// One enum rather than upstream's class hierarchy, per the workspace
/// no-trait-objects rule. Arguments are `Arc`-shared because the MEF lets one
/// parameter feed several expressions, and because the rules forbid `Box`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal number. Upstream `ConstantExpression`.
    Float(f64),
    /// A literal boolean, evaluating to 1 or 0. Upstream `ConstantExpression`
    /// with a `bool`.
    Bool(bool),
    /// `<system-mission-time/>` — the model's mission time.
    MissionTime,
    /// A named parameter, resolved against a [`Parameters`] table.
    Parameter(String),

    /// `1 - exp(-lambda * t)`. Upstream `Exponential`.
    Exponential {
        /// Failure rate, per hour.
        lambda: Arc<Expression>,
        /// Mission time, hours.
        time: Arc<Expression>,
    },
    /// The GLM (failure-on-demand plus repairable) model. Upstream `Glm`.
    Glm {
        /// Probability of failure on demand.
        gamma: Arc<Expression>,
        /// Failure rate, per hour.
        lambda: Arc<Expression>,
        /// Repair rate, per hour.
        mu: Arc<Expression>,
        /// Mission time, hours.
        time: Arc<Expression>,
    },
    /// The Weibull failure model. Upstream `Weibull`.
    Weibull {
        /// Scale parameter.
        alpha: Arc<Expression>,
        /// Shape parameter.
        beta: Arc<Expression>,
        /// Time shift before failures can begin.
        t0: Arc<Expression>,
        /// Mission time, hours.
        time: Arc<Expression>,
    },
    /// Periodic testing with **instant repair** — upstream's four-argument
    /// `PeriodicTest`, its `InstantRepair` flavour.
    ///
    /// The component is tested every `tau` hours starting at `theta`, and a
    /// discovered failure is repaired instantly, so the probability resets at
    /// every test. This is the flavour the fixture's `HIPPS` uses.
    PeriodicTestInstantRepair {
        /// Failure rate while functioning, per hour.
        lambda: Arc<Expression>,
        /// Time between tests, hours.
        tau: Arc<Expression>,
        /// Time before the first test, hours.
        theta: Arc<Expression>,
        /// Mission time, hours.
        time: Arc<Expression>,
    },
    /// Periodic testing with an **instant test** and a repair rate —
    /// upstream's five-argument `PeriodicTest`, its `InstantTest` flavour.
    PeriodicTestInstantTest {
        /// Failure rate while functioning, per hour.
        lambda: Arc<Expression>,
        /// Repair rate, per hour.
        mu: Arc<Expression>,
        /// Time between tests, hours.
        tau: Arc<Expression>,
        /// Time before the first test, hours.
        theta: Arc<Expression>,
        /// Mission time, hours.
        time: Arc<Expression>,
    },

    /// Sum of the arguments. Upstream `Add`.
    Add(Vec<Expression>),
    /// First argument minus the rest. Upstream `Sub`.
    Sub(Vec<Expression>),
    /// Product of the arguments. Upstream `Mul`.
    Mul(Vec<Expression>),
    /// First argument divided by the rest. Upstream `Div`.
    Div(Vec<Expression>),
    /// Arithmetic negation. Upstream `Neg`.
    Neg(Arc<Expression>),
    /// Absolute value. Upstream `Abs`.
    Abs(Arc<Expression>),
    /// Smallest argument. Upstream `Min`.
    Min(Vec<Expression>),
    /// Largest argument. Upstream `Max`.
    Max(Vec<Expression>),
    /// Arithmetic mean of the arguments. Upstream `Mean`.
    Mean(Vec<Expression>),
    /// `e` raised to the argument. Upstream `Exp`.
    Exp(Arc<Expression>),
    /// Natural logarithm. Upstream `Log`.
    Log(Arc<Expression>),
    /// Base-10 logarithm. Upstream `Log10`.
    Log10(Arc<Expression>),
    /// First argument raised to the second. Upstream `Pow`.
    Pow(Arc<Expression>, Arc<Expression>),
    /// Square root. Upstream `Sqrt`.
    Sqrt(Arc<Expression>),
    /// Remainder of integer division. Upstream `Mod`.
    Mod(Arc<Expression>, Arc<Expression>),
    /// Round towards zero. Upstream `Trunc` / `Integer`.
    Trunc(Arc<Expression>),
    /// Round to the nearest integer. Upstream `Round`.
    Round(Arc<Expression>),
    /// Round down. Upstream `Floor`.
    Floor(Arc<Expression>),
    /// Round up. Upstream `Ceil`.
    Ceil(Arc<Expression>),

    /// `if condition then consequent else alternate`. Upstream `Ite`.
    Ite {
        /// Evaluated as a boolean: non-zero is true.
        condition: Arc<Expression>,
        /// Taken when the condition holds.
        consequent: Arc<Expression>,
        /// Taken otherwise.
        alternate: Arc<Expression>,
    },
    /// Logical negation: 1 when the argument is zero, else 0. Upstream `Not`.
    Not(Arc<Expression>),
    /// All arguments non-zero. Upstream `And`.
    And(Vec<Expression>),
    /// Any argument non-zero. Upstream `Or`.
    Or(Vec<Expression>),
    /// Equality. Upstream `Eq`.
    Eq(Arc<Expression>, Arc<Expression>),
    /// Strictly less than. Upstream `Lt`.
    Lt(Arc<Expression>, Arc<Expression>),
    /// Strictly greater than. Upstream `Gt`.
    Gt(Arc<Expression>, Arc<Expression>),
    /// `<uniform-deviate>`. Upstream `UniformDeviate`.
    ///
    /// Its deterministic value is the midpoint, which is the distribution's
    /// mean.
    UniformDeviate {
        /// Lower bound.
        min: Arc<Expression>,
        /// Upper bound.
        max: Arc<Expression>,
    },
    /// `<normal-deviate>`. Upstream `NormalDeviate`.
    NormalDeviate {
        /// Mean.
        mean: Arc<Expression>,
        /// Standard deviation.
        sigma: Arc<Expression>,
    },
    /// The three-argument `<lognormal-deviate>` — upstream's `Logarithmic`
    /// flavour, parametrised by the distribution's **own** mean and an error
    /// factor at a confidence level.
    ///
    /// `EF = exp(z_level * sigma)`, so `sigma = ln(EF) / z_level`, and
    /// `mu = ln(mean) - sigma^2 / 2`. This is the flavour `SmallTree` and
    /// `BSCU` use.
    LognormalDeviate {
        /// Mean of the log-normal distribution (not of the underlying normal).
        mean: Arc<Expression>,
        /// Error factor.
        ef: Arc<Expression>,
        /// Confidence level the error factor is quoted at, in `(0, 1)`.
        level: Arc<Expression>,
    },
    /// The two-argument `<lognormal-deviate>` — upstream's `Normal` flavour,
    /// parametrised by the **underlying normal's** mean and standard
    /// deviation.
    LognormalDeviateNormal {
        /// Location parameter: mean of `ln X`.
        mu: Arc<Expression>,
        /// Scale parameter: standard deviation of `ln X`.
        sigma: Arc<Expression>,
    },
    /// `<gamma-deviate>`. Upstream `GammaDeviate`, shape and **scale**.
    GammaDeviate {
        /// Shape parameter.
        k: Arc<Expression>,
        /// Scale parameter.
        theta: Arc<Expression>,
    },
    /// `<beta-deviate>`. Upstream `BetaDeviate`.
    BetaDeviate {
        /// First shape parameter.
        alpha: Arc<Expression>,
        /// Second shape parameter.
        beta: Arc<Expression>,
    },
    /// `<histogram>`. Upstream `Histogram`: a piecewise-constant density.
    ///
    /// `boundaries` has one more entry than `weights`: the first is the lower
    /// bound of the first bin, and each later one closes a bin. Upstream keeps
    /// them in one argument list split at the midpoint; here they are two
    /// fields, which makes the length invariant representable.
    Histogram {
        /// Bin boundaries, strictly increasing, `weights.len() + 1` of them.
        boundaries: Vec<Expression>,
        /// Positive weight of each bin.
        weights: Vec<Expression>,
    },
}

/// `1 - exp(-lambda * t)` — upstream's `p_exp(lambda, time)`.
fn p_exp(lambda: f64, time: f64) -> f64 {
    1.0 - (-lambda * time).exp()
}

/// Upstream's four-argument `p_exp`, the two-state (fail/repair) form.
///
/// The `lambda == mu` branch is upstream's and is not an approximation: the
/// general expression divides by `lambda - mu`, and this is its limit.
fn p_exp2(p_mu: f64, p_lambda: f64, mu: f64, lambda: f64, time: f64) -> f64 {
    if lambda == mu {
        p_lambda - (1.0 - p_lambda) * lambda * time
    } else {
        (lambda * p_mu - mu * p_lambda) / (lambda - mu)
    }
}

impl Expression {
    /// A literal number.
    pub fn float(value: f64) -> Self {
        Expression::Float(value)
    }

    /// `1 - exp(-lambda * t)`.
    pub fn exponential(lambda: Expression, time: Expression) -> Self {
        Expression::Exponential {
            lambda: Arc::new(lambda),
            time: Arc::new(time),
        }
    }

    /// The GLM model: failure on demand `gamma`, failure rate `lambda`,
    /// repair rate `mu`, over `time`.
    pub fn glm(gamma: Expression, lambda: Expression, mu: Expression, time: Expression) -> Self {
        Expression::Glm {
            gamma: Arc::new(gamma),
            lambda: Arc::new(lambda),
            mu: Arc::new(mu),
            time: Arc::new(time),
        }
    }

    /// The Weibull model.
    pub fn weibull(alpha: Expression, beta: Expression, t0: Expression, time: Expression) -> Self {
        Expression::Weibull {
            alpha: Arc::new(alpha),
            beta: Arc::new(beta),
            t0: Arc::new(t0),
            time: Arc::new(time),
        }
    }

    /// Periodic testing with instant repair — the four-argument form.
    pub fn periodic_test_instant_repair(
        lambda: Expression,
        tau: Expression,
        theta: Expression,
        time: Expression,
    ) -> Self {
        Expression::PeriodicTestInstantRepair {
            lambda: Arc::new(lambda),
            tau: Arc::new(tau),
            theta: Arc::new(theta),
            time: Arc::new(time),
        }
    }

    /// Periodic testing with an instant test and a repair rate — the
    /// five-argument form.
    pub fn periodic_test_instant_test(
        lambda: Expression,
        mu: Expression,
        tau: Expression,
        theta: Expression,
        time: Expression,
    ) -> Self {
        Expression::PeriodicTestInstantTest {
            lambda: Arc::new(lambda),
            mu: Arc::new(mu),
            tau: Arc::new(tau),
            theta: Arc::new(theta),
            time: Arc::new(time),
        }
    }

    /// Evaluates the expression with no parameters and the default mission
    /// time.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a parameter is referenced (there
    /// being no table to resolve it against) or a domain check fails.
    pub fn value(&self) -> Result<f64> {
        self.evaluate(&Parameters::new(), DEFAULT_MISSION_TIME)
    }

    /// Evaluates the expression.
    ///
    /// `mission_time` resolves [`Expression::MissionTime`]; `parameters`
    /// resolves [`Expression::Parameter`].
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a parameter is not in the table,
    /// or an argument is outside the domain upstream's `Validate()` requires
    /// (see [`Expression::validate`]).
    pub fn evaluate(&self, parameters: &Parameters, mission_time: f64) -> Result<f64> {
        let ev = |e: &Expression| e.evaluate(parameters, mission_time);
        let all = |xs: &[Expression]| -> Result<Vec<f64>> { xs.iter().map(ev).collect() };
        let truthy = |x: f64| x != 0.0;

        Ok(match self {
            Expression::Float(v) => *v,
            Expression::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Expression::MissionTime => mission_time,
            Expression::Parameter(name) => {
                *parameters
                    .get(name)
                    .ok_or_else(|| RafflesError::InvalidParameter {
                        parameter: "parameter".to_string(),
                        value: 0.0,
                        reason: format!("the expression references `{name}`, which is not defined"),
                    })?
            }

            // Upstream: `return p_exp(lambda, time);`
            Expression::Exponential { lambda, time } => p_exp(ev(lambda)?, ev(time)?),

            // Upstream:
            //   double r = lambda + mu;
            //   return (lambda - (lambda - gamma * r) * exp(-r * time)) / r;
            Expression::Glm {
                gamma,
                lambda,
                mu,
                time,
            } => {
                let (gamma, lambda, mu, time) = (ev(gamma)?, ev(lambda)?, ev(mu)?, ev(time)?);
                let r = lambda + mu;
                (lambda - (lambda - gamma * r) * (-r * time).exp()) / r
            }

            // Upstream:
            //   return time <= t0 ? 0 : 1 - exp(-pow((time - t0) / alpha, beta));
            Expression::Weibull {
                alpha,
                beta,
                t0,
                time,
            } => {
                let (alpha, beta, t0, time) = (ev(alpha)?, ev(beta)?, ev(t0)?, ev(time)?);
                if time <= t0 {
                    0.0
                } else {
                    1.0 - (-((time - t0) / alpha).powf(beta)).exp()
                }
            }

            Expression::PeriodicTestInstantRepair {
                lambda,
                tau,
                theta,
                time,
            } => {
                let (lambda, tau, theta, time) = (ev(lambda)?, ev(tau)?, ev(theta)?, ev(time)?);
                periodic_test_instant_repair(lambda, tau, theta, time)
            }

            Expression::PeriodicTestInstantTest {
                lambda,
                mu,
                tau,
                theta,
                time,
            } => {
                let (lambda, mu, tau, theta, time) =
                    (ev(lambda)?, ev(mu)?, ev(tau)?, ev(theta)?, ev(time)?);
                periodic_test_instant_test(lambda, mu, tau, theta, time)
            }

            Expression::Add(xs) => all(xs)?.iter().sum(),
            Expression::Sub(xs) => {
                let v = all(xs)?;
                v[1..].iter().fold(v[0], |a, b| a - b)
            }
            Expression::Mul(xs) => all(xs)?.iter().product(),
            Expression::Div(xs) => {
                let v = all(xs)?;
                for (i, d) in v[1..].iter().enumerate() {
                    if *d == 0.0 {
                        return Err(RafflesError::InvalidParameter {
                            parameter: "divisor".to_string(),
                            value: 0.0,
                            reason: format!("argument {} of a division is zero", i + 2),
                        });
                    }
                }
                v[1..].iter().fold(v[0], |a, b| a / b)
            }
            Expression::Neg(x) => -ev(x)?,
            Expression::Abs(x) => ev(x)?.abs(),
            Expression::Min(xs) => all(xs)?.into_iter().fold(f64::INFINITY, f64::min),
            Expression::Max(xs) => all(xs)?.into_iter().fold(f64::NEG_INFINITY, f64::max),
            Expression::Mean(xs) => {
                let v = all(xs)?;
                v.iter().sum::<f64>() / v.len() as f64
            }
            Expression::Exp(x) => ev(x)?.exp(),
            Expression::Log(x) => ev(x)?.ln(),
            Expression::Log10(x) => ev(x)?.log10(),
            Expression::Pow(a, b) => ev(a)?.powf(ev(b)?),
            Expression::Sqrt(x) => ev(x)?.sqrt(),
            Expression::Mod(a, b) => {
                let (a, b) = (ev(a)?, ev(b)?);
                if b == 0.0 {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "modulus".to_string(),
                        value: 0.0,
                        reason: "the modulus of a `mod` expression is zero".to_string(),
                    });
                }
                (a as i64 % b as i64) as f64
            }
            Expression::Trunc(x) => ev(x)?.trunc(),
            Expression::Round(x) => ev(x)?.round(),
            Expression::Floor(x) => ev(x)?.floor(),
            Expression::Ceil(x) => ev(x)?.ceil(),

            Expression::Ite {
                condition,
                consequent,
                alternate,
            } => {
                if truthy(ev(condition)?) {
                    ev(consequent)?
                } else {
                    ev(alternate)?
                }
            }
            Expression::Not(x) => (!truthy(ev(x)?)) as u8 as f64,
            Expression::And(xs) => all(xs)?.into_iter().all(truthy) as u8 as f64,
            Expression::Or(xs) => all(xs)?.into_iter().any(truthy) as u8 as f64,
            Expression::Eq(a, b) => (ev(a)? == ev(b)?) as u8 as f64,
            Expression::Lt(a, b) => (ev(a)? < ev(b)?) as u8 as f64,
            Expression::Gt(a, b) => (ev(a)? > ev(b)?) as u8 as f64,

            // The random deviates evaluate to upstream's `value()`, which is
            // the distribution's MEAN, not a draw. That is what SCRAM's
            // deterministic analysis uses and what its report prints; drawing
            // is `Expression::sample`.
            Expression::UniformDeviate { min, max } => (ev(min)? + ev(max)?) / 2.0,
            Expression::NormalDeviate { mean, .. } => ev(mean)?,
            Expression::LognormalDeviate { mean, .. } => ev(mean)?,
            // Upstream `Normal::mean()`: exp(location + scale^2 / 2).
            Expression::LognormalDeviateNormal { mu, sigma } => {
                let (mu, sigma) = (ev(mu)?, ev(sigma)?);
                (mu + sigma.powi(2) / 2.0).exp()
            }
            Expression::GammaDeviate { k, theta } => ev(k)? * ev(theta)?,
            Expression::BetaDeviate { alpha, beta } => {
                let a = ev(alpha)?;
                a / (a + ev(beta)?)
            }
            Expression::Histogram {
                boundaries,
                weights,
            } => histogram_value(&all(boundaries)?, &all(weights)?)?,
        })
    }

    /// Upstream's `Validate()`: the domain checks each formula requires.
    ///
    /// Kept separate from [`Expression::evaluate`] because upstream separates
    /// them — a model is validated once at load, then evaluated many times.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] naming the offending argument, in
    /// the same words upstream uses.
    pub fn validate(&self, parameters: &Parameters, mission_time: f64) -> Result<()> {
        let bad = |what: &str, value: f64, reason: &str| RafflesError::InvalidParameter {
            parameter: what.to_string(),
            value,
            reason: reason.to_string(),
        };
        let ev = |e: &Expression| e.evaluate(parameters, mission_time);

        match self {
            Expression::Exponential { lambda, time } => {
                // Upstream: EnsureNonNegative(lambda, "rate of failure"),
                //           EnsureNonNegative(time, "mission time").
                for (e, what) in [(lambda, "rate of failure"), (time, "mission time")] {
                    let v = ev(e)?;
                    if v < 0.0 {
                        return Err(bad(what, v, "must be non-negative"));
                    }
                }
            }
            Expression::Glm {
                gamma,
                lambda,
                mu,
                time,
            } => {
                let l = ev(lambda)?;
                if l <= 0.0 {
                    return Err(bad("rate of failure", l, "must be positive"));
                }
                for (e, what) in [(mu, "rate of repair"), (time, "mission time")] {
                    let v = ev(e)?;
                    if v < 0.0 {
                        return Err(bad(what, v, "must be non-negative"));
                    }
                }
                let g = ev(gamma)?;
                if !(0.0..=1.0).contains(&g) {
                    return Err(bad(
                        "failure on demand",
                        g,
                        "must be a probability in [0, 1]",
                    ));
                }
            }
            Expression::Weibull {
                alpha,
                beta,
                t0,
                time,
            } => {
                for (e, what) in [
                    (alpha, "scale parameter for Weibull distribution"),
                    (beta, "shape parameter for Weibull distribution"),
                ] {
                    let v = ev(e)?;
                    if v <= 0.0 {
                        return Err(bad(what, v, "must be positive"));
                    }
                }
                for (e, what) in [(t0, "time shift"), (time, "mission time")] {
                    let v = ev(e)?;
                    if v < 0.0 {
                        return Err(bad(what, v, "must be non-negative"));
                    }
                }
            }
            Expression::PeriodicTestInstantRepair {
                lambda,
                tau,
                theta,
                time,
            } => validate_periodic(&bad, &ev, lambda, tau, theta, time, None)?,
            Expression::PeriodicTestInstantTest {
                lambda,
                mu,
                tau,
                theta,
                time,
            } => validate_periodic(&bad, &ev, lambda, tau, theta, time, Some(mu))?,

            // Upstream `UniformDeviate::Validate`.
            Expression::UniformDeviate { min, max } => {
                let (lo, hi) = (ev(min)?, ev(max)?);
                if lo >= hi {
                    return Err(bad(
                        "Uniform distribution",
                        lo,
                        "min value is more than max",
                    ));
                }
            }
            // Upstream `NormalDeviate::Validate` and
            // `LognormalDeviate::Normal::Validate` -- the same check, in the
            // same words.
            Expression::NormalDeviate { sigma, .. }
            | Expression::LognormalDeviateNormal { sigma, .. } => {
                let s = ev(sigma)?;
                if s <= 0.0 {
                    return Err(bad("standard deviation", s, "cannot be negative or zero"));
                }
            }
            // Upstream `LognormalDeviate::Logarithmic::Validate`, checked in
            // upstream's own order: level, then error factor, then mean.
            Expression::LognormalDeviate { mean, ef, level } => {
                let l = ev(level)?;
                if l <= 0.0 || l >= 1.0 {
                    return Err(bad("confidence level", l, "is not within (0, 1)"));
                }
                let e = ev(ef)?;
                if e <= 1.0 {
                    return Err(bad(
                        "error factor for log-normal distribution",
                        e,
                        "cannot be less than 1",
                    ));
                }
                let m = ev(mean)?;
                if m <= 0.0 {
                    return Err(bad(
                        "mean of log-normal distribution",
                        m,
                        "cannot be negative or zero",
                    ));
                }
            }
            // Upstream `GammaDeviate::Validate`.
            Expression::GammaDeviate { k, theta } => {
                for (e, what) in [
                    (k, "k shape parameter for Gamma distribution"),
                    (theta, "theta scale parameter for Gamma distribution"),
                ] {
                    let v = ev(e)?;
                    if v <= 0.0 {
                        return Err(bad(what, v, "cannot be negative or zero"));
                    }
                }
            }
            // Upstream `BetaDeviate::Validate`.
            Expression::BetaDeviate { alpha, beta } => {
                for (e, what) in [
                    (alpha, "alpha shape parameter for Beta distribution"),
                    (beta, "beta shape parameter for Beta distribution"),
                ] {
                    let v = ev(e)?;
                    if v <= 0.0 {
                        return Err(bad(what, v, "cannot be negative or zero"));
                    }
                }
            }
            // Upstream `Histogram::Validate`, plus the length invariant its
            // constructor enforces.
            Expression::Histogram {
                boundaries,
                weights,
            } => {
                let bounds: Vec<f64> = boundaries.iter().map(&ev).collect::<Result<_>>()?;
                let w: Vec<f64> = weights.iter().map(&ev).collect::<Result<_>>()?;
                histogram_shape(&bounds, &w)?;
                if let Some(neg) = w.iter().find(|x| **x < 0.0) {
                    return Err(bad("histogram weight", *neg, "cannot be negative"));
                }
                // Upstream's message says "strictly increasing" but its
                // comparator is `lhs <= rhs`, so equal boundaries pass. The
                // behaviour is ported, not the message's stricter reading.
                if let Some(w) = bounds.windows(2).find(|w| w[0] > w[1]) {
                    return Err(bad(
                        "histogram upper boundaries",
                        w[1],
                        "are not increasing",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Upstream `PeriodicTest::InstantRepair::Validate` and `InstantTest::Validate`.
fn validate_periodic(
    bad: &dyn Fn(&str, f64, &str) -> RafflesError,
    ev: &dyn Fn(&Expression) -> Result<f64>,
    lambda: &Expression,
    tau: &Expression,
    theta: &Expression,
    time: &Expression,
    mu: Option<&Expression>,
) -> Result<()> {
    for (e, what) in [(lambda, "rate of failure"), (tau, "time between tests")] {
        let v = ev(e)?;
        if v <= 0.0 {
            return Err(bad(what, v, "must be positive"));
        }
    }
    for (e, what) in [(theta, "time before tests"), (time, "mission time")] {
        let v = ev(e)?;
        if v < 0.0 {
            return Err(bad(what, v, "must be non-negative"));
        }
    }
    if let Some(mu) = mu {
        let v = ev(mu)?;
        if v < 0.0 {
            return Err(bad("rate of repair", v, "must be non-negative"));
        }
    }
    Ok(())
}

/// Upstream `PeriodicTest::InstantRepair::Compute`.
///
/// Before the first test the component is a plain exponential. After it, only
/// the time since the last test matters, because repair is instantaneous —
/// which is why the probability saws back down at every interval.
fn periodic_test_instant_repair(lambda: f64, tau: f64, theta: f64, time: f64) -> f64 {
    if time <= theta {
        return p_exp(lambda, time);
    }
    let delta = time - theta;
    let time_after_test = delta - (delta / tau) as i64 as f64 * tau;
    // Upstream: `p_exp(lambda, time_after_test ? time_after_test : tau)` --
    // landing exactly on a test means a full interval has elapsed, not none.
    p_exp(
        lambda,
        if time_after_test != 0.0 {
            time_after_test
        } else {
            tau
        },
    )
}

/// Upstream `PeriodicTest::InstantTest::Compute`.
///
/// Repair now takes time, so the probability does not reset fully at a test;
/// what carries over is summed as a geometric progression across the whole
/// periods, then the partial period is applied.
fn periodic_test_instant_test(lambda: f64, mu: f64, tau: f64, theta: f64, time: f64) -> f64 {
    if time <= theta {
        return p_exp(lambda, time);
    }
    let carry = |p_lambda: f64, p_mu: f64, t: f64| {
        let p_mu_lambda = p_exp2(p_lambda, p_mu, lambda, mu, t);
        1.0 - p_mu + p_mu_lambda - p_lambda
    };
    let mut prob = p_exp(lambda, theta);
    let delta = time - theta;
    let num_periods = (delta / tau) as i64;
    let fraction = carry(p_exp(lambda, tau), p_exp(mu, tau), tau);
    let compound = fraction.powi(num_periods as i32);
    prob = prob * compound + p_exp(lambda, tau) * (compound - 1.0) / (fraction - 1.0);
    let time_after_test = delta - num_periods as f64 * tau;
    let (p_lambda, p_mu) = (p_exp(lambda, time_after_test), p_exp(mu, time_after_test));
    prob * carry(p_lambda, p_mu, time_after_test) + p_lambda
}

/// A value's domain, for validation only — upstream's `Interval`.
///
/// Upstream is `boost::icl::continuous_interval<double>`, and the only
/// operations it uses are the four constructors, `lower`/`upper`, `contains`
/// and `within`. Those are what this carries; nothing here is a general
/// interval-arithmetic type.
///
/// **What it is for.** Upstream validates an argument twice: its *value* must
/// be in range, and its *sample domain* must be too
/// (`EnsureNonNegative`/`EnsurePositive`/`EnsureWithin` in `expression.cc`).
/// The second check only bites when a random deviate is involved — a normal
/// deviate used as a failure rate has a domain reaching six sigma below its
/// mean, and upstream rejects it however comfortable the mean looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    lower: f64,
    upper: f64,
    lower_open: bool,
    upper_open: bool,
}

impl Interval {
    /// `[lower, upper]`.
    pub fn closed(lower: f64, upper: f64) -> Self {
        Interval {
            lower,
            upper,
            lower_open: false,
            upper_open: false,
        }
    }

    /// `(lower, upper]`.
    pub fn left_open(lower: f64, upper: f64) -> Self {
        Interval {
            lower,
            upper,
            lower_open: true,
            upper_open: false,
        }
    }

    /// The degenerate interval `[value, value]`, which is upstream's default
    /// `Expression::interval()` for anything that does not deviate.
    pub fn point(value: f64) -> Self {
        Interval::closed(value, value)
    }

    /// The lower bound.
    pub fn lower(&self) -> f64 {
        self.lower
    }

    /// The upper bound.
    pub fn upper(&self) -> f64 {
        self.upper
    }

    /// Whether `value` lies in the interval — upstream's `Contains`.
    pub fn contains(&self, value: f64) -> bool {
        let above = if self.lower_open {
            value > self.lower
        } else {
            value >= self.lower
        };
        let below = if self.upper_open {
            value < self.upper
        } else {
            value <= self.upper
        };
        above && below
    }

    /// Whether this interval lies entirely inside `other` — upstream's
    /// `boost::icl::within`.
    pub fn within(&self, other: &Interval) -> bool {
        other.contains(self.lower) && other.contains(self.upper)
    }

    /// Upstream's `IsNonNegative`: **the lower bound alone**, open or not.
    pub fn is_non_negative(&self) -> bool {
        self.lower >= 0.0
    }

    /// Upstream's `IsPositive`: non-negative and not containing zero.
    pub fn is_positive(&self) -> bool {
        self.is_non_negative() && !self.contains(0.0)
    }

    /// Upstream's `IsProbability`: within `[0, 1]`.
    pub fn is_probability(&self) -> bool {
        self.within(&Interval::closed(0.0, 1.0))
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}, {}{}",
            if self.lower_open { '(' } else { '[' },
            self.lower,
            self.upper,
            if self.upper_open { ')' } else { ']' }
        )
    }
}

/// The scale (`sigma`) and location (`mu`) of a log-normal deviate's
/// underlying normal, for the three-argument `Logarithmic` flavour.
///
/// Upstream:
///
/// ```cpp
/// double z = -std::sqrt(2) * boost::math::erfc_inv(2 * level_.value());
/// return std::log(ef_.value()) / z;                       // scale
/// return std::log(mean_.value()) - std::pow(scale(), 2) / 2;  // location
/// ```
///
/// `-sqrt(2) * erfc_inv(2 p)` **is** the standard normal quantile at `p`, so
/// this uses [`crate::distributions::Normal`]'s inverse CDF rather than
/// introducing a second `erfc_inv`. The two agree to `1e-12` at the levels
/// MEF models use, which `scram_deviates` asserts rather than assumes.
fn lognormal_logarithmic(mean: f64, ef: f64, level: f64) -> (f64, f64) {
    let z = crate::distributions::special::norm_ppf_std(level);
    let scale = ef.ln() / z;
    (scale, mean.ln() - scale.powi(2) / 2.0)
}

/// Upstream `Histogram::value()`: the weighted mean of the bin midpoints.
///
/// ```cpp
/// sum_product += (cur_bound + prev_bound) * cur_weight;
/// sum_weights += cur_weight;
/// return sum_product / (2 * sum_weights);
/// ```
fn histogram_value(boundaries: &[f64], weights: &[f64]) -> Result<f64> {
    histogram_shape(boundaries, weights)?;
    let mut sum_weights = 0.0;
    let mut sum_product = 0.0;
    let mut prev = boundaries[0];
    for (i, w) in weights.iter().enumerate() {
        let cur = boundaries[i + 1];
        sum_product += (cur + prev) * w;
        sum_weights += w;
        prev = cur;
    }
    if sum_weights == 0.0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "histogram weights".to_string(),
            value: 0.0,
            reason: "the weights sum to zero, so the distribution has no mass".to_string(),
        });
    }
    Ok(sum_product / (2.0 * sum_weights))
}

/// The length invariant upstream enforces in `Histogram`'s constructor.
fn histogram_shape(boundaries: &[f64], weights: &[f64]) -> Result<()> {
    if boundaries.len() != weights.len() + 1 || weights.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "histogram".to_string(),
            value: weights.len() as f64,
            reason: format!(
                "the number of weights is not equal to the number of intervals: {} \
                 boundaries against {} weights",
                boundaries.len(),
                weights.len()
            ),
        });
    }
    Ok(())
}

impl Expression {
    /// `<uniform-deviate>`.
    pub fn uniform_deviate(min: Expression, max: Expression) -> Self {
        Expression::UniformDeviate {
            min: Arc::new(min),
            max: Arc::new(max),
        }
    }

    /// `<normal-deviate>`.
    pub fn normal_deviate(mean: Expression, sigma: Expression) -> Self {
        Expression::NormalDeviate {
            mean: Arc::new(mean),
            sigma: Arc::new(sigma),
        }
    }

    /// The three-argument `<lognormal-deviate>`: mean, error factor, level.
    pub fn lognormal_deviate(mean: Expression, ef: Expression, level: Expression) -> Self {
        Expression::LognormalDeviate {
            mean: Arc::new(mean),
            ef: Arc::new(ef),
            level: Arc::new(level),
        }
    }

    /// The two-argument `<lognormal-deviate>`: the underlying normal's
    /// parameters.
    pub fn lognormal_deviate_normal(mu: Expression, sigma: Expression) -> Self {
        Expression::LognormalDeviateNormal {
            mu: Arc::new(mu),
            sigma: Arc::new(sigma),
        }
    }

    /// `<gamma-deviate>`: shape and scale.
    pub fn gamma_deviate(k: Expression, theta: Expression) -> Self {
        Expression::GammaDeviate {
            k: Arc::new(k),
            theta: Arc::new(theta),
        }
    }

    /// `<beta-deviate>`.
    pub fn beta_deviate(alpha: Expression, beta: Expression) -> Self {
        Expression::BetaDeviate {
            alpha: Arc::new(alpha),
            beta: Arc::new(beta),
        }
    }

    /// Whether the expression's value deviates from its mean — upstream's
    /// `IsDeviate()`.
    ///
    /// A random deviate answers yes for itself; anything else answers yes if
    /// any argument does. This is the hook uncertainty analysis uses to decide
    /// whether a model needs sampling at all.
    pub fn is_deviate(&self) -> bool {
        match self {
            Expression::UniformDeviate { .. }
            | Expression::NormalDeviate { .. }
            | Expression::LognormalDeviate { .. }
            | Expression::LognormalDeviateNormal { .. }
            | Expression::GammaDeviate { .. }
            | Expression::BetaDeviate { .. }
            | Expression::Histogram { .. } => true,
            _ => self.args().iter().any(|a| a.is_deviate()),
        }
    }

    /// The expression's direct arguments, in upstream's registration order.
    ///
    /// A `Vec` rather than an iterator: the workspace forbids trait objects,
    /// and the alternative -- one enum wrapper per arity -- would be more
    /// machinery than a short borrowed list is worth. Nothing calls this in a
    /// hot loop.
    fn args(&self) -> Vec<&Expression> {
        match self {
            Expression::Float(_)
            | Expression::Bool(_)
            | Expression::MissionTime
            | Expression::Parameter(_) => Vec::new(),
            Expression::Exponential { lambda, time } => vec![lambda, time],
            Expression::Glm {
                gamma,
                lambda,
                mu,
                time,
            } => vec![gamma, lambda, mu, time],
            Expression::Weibull {
                alpha,
                beta,
                t0,
                time,
            } => vec![alpha, beta, t0, time],
            Expression::PeriodicTestInstantRepair {
                lambda,
                tau,
                theta,
                time,
            } => vec![lambda, tau, theta, time],
            Expression::PeriodicTestInstantTest {
                lambda,
                mu,
                tau,
                theta,
                time,
            } => vec![lambda, mu, tau, theta, time],
            Expression::Add(xs)
            | Expression::Sub(xs)
            | Expression::Mul(xs)
            | Expression::Div(xs)
            | Expression::Min(xs)
            | Expression::Max(xs)
            | Expression::Mean(xs)
            | Expression::And(xs)
            | Expression::Or(xs) => xs.iter().collect(),
            Expression::Neg(x)
            | Expression::Abs(x)
            | Expression::Exp(x)
            | Expression::Log(x)
            | Expression::Log10(x)
            | Expression::Sqrt(x)
            | Expression::Trunc(x)
            | Expression::Round(x)
            | Expression::Floor(x)
            | Expression::Ceil(x)
            | Expression::Not(x) => vec![x],
            Expression::Pow(a, b)
            | Expression::Mod(a, b)
            | Expression::Eq(a, b)
            | Expression::Lt(a, b)
            | Expression::Gt(a, b) => vec![a, b],
            Expression::Ite {
                condition,
                consequent,
                alternate,
            } => vec![condition, consequent, alternate],
            Expression::UniformDeviate { min, max } => vec![min, max],
            Expression::NormalDeviate { mean, sigma } => vec![mean, sigma],
            Expression::LognormalDeviate { mean, ef, level } => vec![mean, ef, level],
            Expression::LognormalDeviateNormal { mu, sigma } => vec![mu, sigma],
            Expression::GammaDeviate { k, theta } => vec![k, theta],
            Expression::BetaDeviate { alpha, beta } => vec![alpha, beta],
            Expression::Histogram {
                boundaries,
                weights,
            } => boundaries.iter().chain(weights.iter()).collect(),
        }
    }

    /// The domain of values this expression can take — upstream's
    /// `interval()`.
    ///
    /// Upstream's default is the degenerate `[value, value]`: an expression
    /// with no randomness under it can only produce its own value. The
    /// overrides are what matter, and they are ported one for one — the four
    /// probability formulas are `[0, 1]`, the operators propagate their
    /// arguments' bounds through the corners, and each deviate states its own.
    ///
    /// # Errors
    ///
    /// As [`Expression::evaluate`], since the bounds are computed from
    /// argument values.
    pub fn interval(&self, parameters: &Parameters, mission_time: f64) -> Result<Interval> {
        let iv = |e: &Expression| e.interval(parameters, mission_time);
        let ev = |e: &Expression| e.evaluate(parameters, mission_time);
        let hull = |values: &[f64]| {
            Interval::closed(
                values.iter().copied().fold(f64::INFINITY, f64::min),
                values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            )
        };
        // Upstream `NaryExpression<T, 1>::interval`.
        let unary = |x: &Expression, f: fn(f64) -> f64| -> Result<Interval> {
            let a = iv(x)?;
            Ok(hull(&[f(a.lower()), f(a.upper())]))
        };
        // Upstream `NaryExpression<T, 2>::interval` -- all four corners.
        let corners = |x: &Interval, y: &Interval, f: fn(f64, f64) -> f64| {
            [
                f(x.upper(), y.upper()),
                f(x.upper(), y.lower()),
                f(x.lower(), y.upper()),
                f(x.lower(), y.lower()),
            ]
        };
        let binary = |a: &Expression, b: &Expression, f: fn(f64, f64) -> f64| -> Result<Interval> {
            Ok(hull(&corners(&iv(a)?, &iv(b)?, f)))
        };
        // Upstream `NaryExpression<T, -1>::interval` -- a left fold, matching
        // how the value itself is computed.
        let nary = |xs: &[Expression], f: fn(f64, f64) -> f64| -> Result<Interval> {
            let mut acc = iv(&xs[0])?;
            for x in &xs[1..] {
                acc = hull(&corners(&acc, &iv(x)?, f));
            }
            Ok(acc)
        };

        Ok(match self {
            // The four probability formulas: upstream states [0, 1] outright.
            Expression::Exponential { .. }
            | Expression::Glm { .. }
            | Expression::Weibull { .. }
            | Expression::PeriodicTestInstantRepair { .. }
            | Expression::PeriodicTestInstantTest { .. } => Interval::closed(0.0, 1.0),

            Expression::Add(xs) => nary(xs, |a, b| a + b)?,
            Expression::Sub(xs) => nary(xs, |a, b| a - b)?,
            Expression::Mul(xs) => nary(xs, |a, b| a * b)?,
            Expression::Div(xs) => nary(xs, |a, b| a / b)?,
            Expression::Min(xs) => nary(xs, f64::min)?,
            Expression::Max(xs) => nary(xs, f64::max)?,
            // Upstream `Mean::interval`: the mean of the lower bounds and the
            // mean of the upper bounds, NOT a corner sweep.
            Expression::Mean(xs) => {
                let mut lower = 0.0;
                let mut upper = 0.0;
                for x in xs {
                    let a = iv(x)?;
                    lower += a.lower();
                    upper += a.upper();
                }
                let n = xs.len() as f64;
                Interval::closed(lower / n, upper / n)
            }
            Expression::Neg(x) => unary(x, |v| -v)?,
            Expression::Abs(x) => unary(x, f64::abs)?,
            Expression::Exp(x) => unary(x, f64::exp)?,
            Expression::Log(x) => unary(x, f64::ln)?,
            Expression::Log10(x) => unary(x, f64::log10)?,
            Expression::Sqrt(x) => unary(x, f64::sqrt)?,
            Expression::Trunc(x) => unary(x, f64::trunc)?,
            Expression::Round(x) => unary(x, f64::round)?,
            Expression::Floor(x) => unary(x, f64::floor)?,
            Expression::Ceil(x) => unary(x, f64::ceil)?,
            Expression::Pow(a, b) => binary(a, b, f64::powf)?,
            // `std::modulus<int>` upstream. A zero divisor is undefined there
            // and would panic here, so the corner is reported as zero; the
            // value path rejects it outright.
            Expression::Mod(a, b) => binary(a, b, |x, y| {
                let (x, y) = (x as i64, y as i64);
                if y == 0 {
                    0.0
                } else {
                    (x % y) as f64
                }
            })?,
            // Upstream `Ite::interval`: the hull of the two branches.
            Expression::Ite {
                consequent,
                alternate,
                ..
            } => {
                let (t, f) = (iv(consequent)?, iv(alternate)?);
                Interval::closed(t.lower().min(f.lower()), t.upper().max(f.upper()))
            }

            // Upstream `UniformDeviate::interval`.
            Expression::UniformDeviate { min, max } => Interval::closed(ev(min)?, ev(max)?),
            // Upstream `NormalDeviate::interval`: the ~99.9 % band.
            Expression::NormalDeviate { mean, sigma } => {
                let (m, delta) = (ev(mean)?, 6.0 * ev(sigma)?);
                Interval::closed(m - delta, m + delta)
            }
            // Upstream `LognormalDeviate::interval`: `exp(3 * scale +
            // location)`, the 99.9th percentile estimate, over `(0, high]`.
            Expression::LognormalDeviate { mean, ef, level } => {
                let (scale, location) = lognormal_logarithmic(ev(mean)?, ev(ef)?, ev(level)?);
                Interval::left_open(0.0, (3.0 * scale + location).exp())
            }
            Expression::LognormalDeviateNormal { mu, sigma } => {
                Interval::left_open(0.0, (3.0 * ev(sigma)? + ev(mu)?).exp())
            }
            // Upstream `GammaDeviate::interval`, ported as written:
            //
            //   theta * pow(gamma_q(k, gamma_q(k, 0) - 0.99), -1)
            //
            // `gamma_q(k, 0)` is 1 for every k, so the inner argument is the
            // constant 0.01 and this is `theta / Q(k, 0.01)` rather than a
            // quantile. It reads like it was meant to be `gamma_q_inv`, and
            // BetaDeviate below has the same shape. It is reproduced as
            // upstream wrote it because upstream's behaviour is the
            // specification; it affects validation bounds only, never a
            // reported probability.
            Expression::GammaDeviate { k, theta } => {
                use crate::distributions::special::gamma_q;
                let (k, theta) = (ev(k)?, ev(theta)?);
                let high = theta * gamma_q(k, gamma_q(k, 0.0) - 0.99).powi(-1);
                Interval::left_open(0.0, high)
            }
            // Upstream `BetaDeviate::interval`: `pow(ibeta(alpha, beta, 0.99),
            // -1)`. See the note on GammaDeviate above.
            Expression::BetaDeviate { alpha, beta } => {
                use crate::distributions::special::beta_inc_reg;
                let high = beta_inc_reg(ev(alpha)?, ev(beta)?, 0.99).powi(-1);
                Interval::closed(0.0, high)
            }
            // Upstream `Histogram::interval`: first boundary to last.
            Expression::Histogram { boundaries, .. } => {
                if boundaries.is_empty() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "histogram".to_string(),
                        value: 0.0,
                        reason: "a histogram needs at least two boundaries".to_string(),
                    });
                }
                Interval::closed(ev(&boundaries[0])?, ev(&boundaries[boundaries.len() - 1])?)
            }

            // Upstream's default: an expression with no randomness under it
            // can only produce its own value.
            other => Interval::point(ev(other)?),
        })
    }
}
