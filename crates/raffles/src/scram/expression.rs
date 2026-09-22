// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/expression/exponential.{h,cc}, src/expression/
//                     constant.{h,cc}, src/expression/numerical.{h,cc},
//                     src/expression/boolean.h, src/expression/
//                     conditional.{h,cc}, src/expression.{h,cc}
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
// NOT ported: `Sample()` and the `random_deviate` expressions, which belong
// to uncertainty analysis; `extern.{h,cc}`, which loads shared libraries
// (and is barred by the workspace's "no autonomous access" rule); and
// upstream's `Interval` arithmetic, which exists to validate sampled ranges
// and has no consumer here yet. Upstream's `Validate()` checks are ported as
// [`Expression::validate`].
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
