// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/ccf_group.{h,cc}
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: upstream's `CcfGroup` is an abstract base with four
// derived models overriding `min_level()`, `DoValidate()` and
// `CalculateProbabilities()`. That becomes one [`CcfModel`] enum dispatched
// by `match`, per the workspace no-trait-objects rule. The probability
// formulas, the combination reciprocal, the factor-level bookkeeping and the
// proxy-gate rewrite are ported as written.
//
// `CcfEvent`'s name is upstream's `MakeName`: the member names joined by a
// space inside square brackets, which is also how SCRAM's report identifies
// them, so the two can be compared directly.
// ---------------------------------------------------------------------------

//! Common-cause failure groups.
//!
//! A fault tree that treats two pumps as independent understates the risk:
//! one flood, one maintenance error or one bad batch fails both. A CCF group
//! says "these members share a cause", gives the probability of a member
//! failing at all, and gives the fractions belonging to each size of
//! simultaneous failure.
//!
//! # What applying a group does to the tree
//!
//! Upstream's `CcfGroup::ApplyModel` rewrites the tree rather than adjusting
//! numbers, and this port does the same:
//!
//! 1. every member basic event becomes a **proxy gate of the same name**;
//! 2. for each level `k` and each `k`-subset of the members, a new basic
//!    event `[A B …]` is created with that level's probability;
//! 3. each proxy gate becomes an `or` over every CCF event it belongs to.
//!
//! So a two-member group replaces `PumpOne` with
//! `or([PumpOne], [PumpOne PumpTwo])`, and the shared failure appears in the
//! cut sets as a single event — which is exactly what makes it show up as an
//! order-1 product where the independent model gave order 2.
//!
//! # Applied by default, ablated explicitly
//!
//! Upstream applies CCF groups only under `scram --ccf`. **This port applies
//! them by default**, following the workspace rule that physics the data
//! supplies is applied unless a caller explicitly ablates it: a model that
//! declares a CCF group has said its components are coupled, and a run that
//! silently ignores that reports a risk it has been told is wrong.
//! [`super::mef::MefModel::without_ccf`] is the visible ablation, and both
//! paths are verified against the corresponding SCRAM run.

use std::collections::HashMap;

use super::expression::{ensure_probability, Expression, Parameters};
use crate::{RafflesError, Result};

/// Which common-cause model a group uses.
///
/// Upstream's four `CcfGroup` subclasses. Each differs in what its factors
/// mean and therefore in `CalculateProbabilities`; they share everything
/// else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcfModel {
    /// `beta-factor`: if the common cause occurs, **all** members fail.
    /// One factor, at the group's own size.
    BetaFactor,
    /// `MGL`, the multiple-Greek-letter model: factor `k` is the fraction of
    /// failures of `k` or more given that `k - 1` failed. Factors start at
    /// level 2.
    Mgl,
    /// `alpha-factor`: factor `k` is the fraction of failures involving
    /// exactly `k` members.
    AlphaFactor,
    /// `phi-factor`: the fractions are given directly, `Q_k = phi_k Q`.
    PhiFactor,
}

impl CcfModel {
    /// The MEF attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            CcfModel::BetaFactor => "beta-factor",
            CcfModel::Mgl => "MGL",
            CcfModel::AlphaFactor => "alpha-factor",
            CcfModel::PhiFactor => "phi-factor",
        }
    }

    /// Reads the MEF attribute value.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] for anything outside the grammar's
    /// four values.
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "beta-factor" => CcfModel::BetaFactor,
            "MGL" => CcfModel::Mgl,
            "alpha-factor" => CcfModel::AlphaFactor,
            "phi-factor" => CcfModel::PhiFactor,
            other => {
                return Err(invalid(format!(
                    "`{other}` is not a CCF model; MEF has beta-factor, MGL, alpha-factor \
                     and phi-factor"
                )))
            }
        })
    }

    /// Upstream's `min_level()`: the lowest factor level the model takes.
    ///
    /// The beta-factor model's is the group's own size, because its single
    /// factor is the fraction of failures that take everything down.
    pub fn min_level(self, members: usize) -> usize {
        match self {
            CcfModel::BetaFactor => members,
            CcfModel::Mgl => 2,
            CcfModel::AlphaFactor | CcfModel::PhiFactor => 1,
        }
    }
}

fn invalid(reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: "CCF group".to_string(),
        value: 0.0,
        reason,
    }
}

/// A common-cause failure group.
#[derive(Debug, Clone)]
pub struct CcfGroup {
    /// The group's id, as [`super::mef`] resolves names.
    pub name: String,
    /// Which model the factors belong to.
    pub model: CcfModel,
    /// Member basic-event ids, in declaration order. The order decides which
    /// subsets are formed and in what order, so it is not incidental.
    pub members: Vec<String>,
    /// The probability of a member failing at all — upstream's
    /// `distribution_`, which also becomes each member's own probability
    /// before the model is applied.
    pub distribution: Expression,
    /// `(level, factor)`, dense from [`CcfModel::min_level`] upwards.
    pub factors: Vec<(usize, Expression)>,
    /// The dotted path CCF events are named in, empty at model level.
    pub base_path: String,
    /// Whether the group is private, so its CCF events take their full path
    /// as their id.
    pub private: bool,
}

impl CcfGroup {
    /// Upstream's `CcfGroup::Validate`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the group has fewer than two
    /// members, a factor level is missing, the distribution or a factor is not
    /// a probability, or (for `phi-factor`) the factors do not sum to 1.
    pub fn validate(&self, parameters: &Parameters, mission_time: f64) -> Result<()> {
        if self.members.len() < 2 {
            return Err(invalid(format!(
                "`{}` has {} member(s); a CCF group must have at least 2",
                self.name,
                self.members.len()
            )));
        }
        if self.factors.is_empty() {
            return Err(invalid(format!("`{}` declares no factors", self.name)));
        }
        ensure_probability(
            &self.distribution,
            parameters,
            mission_time,
            "CCF group distribution",
        )?;
        let min = self.model.min_level(self.members.len());
        for (i, (level, factor)) in self.factors.iter().enumerate() {
            if *level != min + i {
                return Err(invalid(format!(
                    "`{}` is missing the CCF factor for level {}",
                    self.name,
                    min + i
                )));
            }
            if *level > self.members.len() {
                return Err(invalid(format!(
                    "`{}` has a factor for level {level}, more than its {} members",
                    self.name,
                    self.members.len()
                )));
            }
            ensure_probability(factor, parameters, mission_time, "CCF group factor")?;
        }
        // Upstream `PhiFactorModel::DoValidate`: the value and both interval
        // bounds must each be 1 to within 1e-4.
        if self.model == CcfModel::PhiFactor {
            let mut sum = 0.0;
            let mut sum_min = 0.0;
            let mut sum_max = 0.0;
            for (_, factor) in &self.factors {
                sum += factor.evaluate(parameters, mission_time)?;
                let interval = factor.interval(parameters, mission_time)?;
                sum_min += interval.lower();
                sum_max += interval.upper();
            }
            if [sum, sum_min, sum_max]
                .iter()
                .any(|s| (s - 1.0).abs() > 1e-4)
            {
                return Err(invalid(format!(
                    "`{}`: the factors for the phi model must sum to 1, found {sum}",
                    self.name
                )));
            }
        }
        Ok(())
    }

    /// Upstream's `CalculateProbabilities`, one arm per model.
    ///
    /// Returns `(level, probability)` for every level the model produces, in
    /// increasing level order.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the factor levels do not suit the
    /// model — the beta-factor model takes exactly one factor, at the group's
    /// own size.
    pub fn probabilities(&self) -> Result<Vec<(usize, Expression)>> {
        let q = || self.distribution.clone();
        let one = Expression::Float(1.0);
        Ok(match self.model {
            // Upstream `BetaFactorModel::CalculateProbabilities`:
            //   (1 - beta) * Q  at level 1, and  beta * Q  at the group size.
            CcfModel::BetaFactor => {
                if self.factors.len() != 1 || self.factors[0].0 != self.members.len() {
                    return Err(invalid(format!(
                        "`{}`: the beta-factor model takes exactly one factor, at level {} \
                         (the group's own size)",
                        self.name,
                        self.members.len()
                    )));
                }
                let beta = self.factors[0].1.clone();
                vec![
                    (
                        1,
                        Expression::Mul(vec![Expression::Sub(vec![one, beta.clone()]), q()]),
                    ),
                    (self.members.len(), Expression::Mul(vec![beta, q()])),
                ]
            }
            // Upstream `MglModel::CalculateProbabilities`.
            CcfModel::Mgl => {
                let max_level = self.factors[self.factors.len() - 1].0;
                let n = self.members.len();
                let mut out = Vec::with_capacity(max_level);
                for i in 0..max_level {
                    let mult = combination_reciprocal(n - 1, i);
                    let mut args = vec![Expression::Float(mult)];
                    for (_, f) in self.factors.iter().take(i) {
                        args.push(f.clone());
                    }
                    if i < max_level - 1 {
                        args.push(Expression::Sub(vec![
                            one.clone(),
                            self.factors[i].1.clone(),
                        ]));
                    }
                    args.push(q());
                    out.push((i + 1, Expression::Mul(args)));
                }
                out
            }
            // Upstream `AlphaFactorModel::CalculateProbabilities`.
            CcfModel::AlphaFactor => {
                let max_level = self.factors[self.factors.len() - 1].0;
                let n = self.members.len();
                let sum = Expression::Add(
                    self.factors
                        .iter()
                        .map(|(level, f)| {
                            Expression::Mul(vec![Expression::Float(*level as f64), f.clone()])
                        })
                        .collect(),
                );
                let mut out = Vec::with_capacity(max_level);
                for i in 0..max_level {
                    let mult = combination_reciprocal(n - 1, i);
                    out.push((
                        i + 1,
                        Expression::Mul(vec![
                            Expression::Float((i + 1) as f64),
                            Expression::Float(mult),
                            Expression::Div(vec![self.factors[i].1.clone(), sum.clone()]),
                            q(),
                        ]),
                    ));
                }
                out
            }
            // Upstream `PhiFactorModel::CalculateProbabilities`: Q_k = phi_k Q.
            CcfModel::PhiFactor => self
                .factors
                .iter()
                .map(|(level, f)| (*level, Expression::Mul(vec![f.clone(), q()])))
                .collect(),
        })
    }

    /// The id a CCF event over `members` takes.
    ///
    /// Upstream's `CcfEvent::MakeName` is the member **names** joined by a
    /// space inside square brackets, and the event is registered with the
    /// group's own base path and role — so a private group's events are known
    /// by their full path, exactly as any other private declaration is. SCRAM
    /// prints the same bracketed list in its report, which is what lets the
    /// two be compared directly.
    fn event_id(&self, members: &[&str]) -> String {
        let bare: Vec<&str> = members
            .iter()
            .map(|m| m.rsplit('.').next().unwrap_or(m))
            .collect();
        let name = format!("[{}]", bare.join(" "));
        if self.private {
            format!("{}.{name}", self.base_path)
        } else {
            name
        }
    }

    /// The CCF events this group creates, and which members each one couples.
    ///
    /// This is the data half of upstream's `ApplyModel`; the tree rewrite that
    /// consumes it lives in [`super::mef`], which owns the gates.
    ///
    /// # Errors
    ///
    /// As [`CcfGroup::probabilities`].
    pub fn events(&self) -> Result<Vec<CcfEvent>> {
        let mut out = Vec::new();
        for (level, probability) in self.probabilities()? {
            for combination in combinations(self.members.len(), level) {
                let members: Vec<&str> = combination
                    .iter()
                    .map(|&i| self.members[i].as_str())
                    .collect();
                out.push(CcfEvent {
                    id: self.event_id(&members),
                    group: self.name.clone(),
                    level,
                    members: members.iter().map(|s| (*s).to_string()).collect(),
                    probability: probability.clone(),
                });
            }
        }
        Ok(out)
    }
}

/// One generated common-cause basic event.
#[derive(Debug, Clone)]
pub struct CcfEvent {
    /// The id it is registered under — `[A B]`, or the full path if the
    /// group is private.
    pub id: String,
    /// The group that created it.
    pub group: String,
    /// How many members fail together.
    pub level: usize,
    /// The member ids it couples.
    pub members: Vec<String>,
    /// Its probability.
    pub probability: Expression,
}

/// Upstream's `CalculateCombinationReciprocal`: `1 / nCk`, by a product that
/// cannot overflow the way a factorial would.
///
/// ```text
/// if (n - k > k) k = n - k;
/// for (i = 1; i <= n - k; ++i) result *= i / (k + i);
/// ```
fn combination_reciprocal(n: usize, k: usize) -> f64 {
    debug_assert!(n >= k);
    let k = if n - k > k { n - k } else { k };
    let mut result = 1.0;
    for i in 1..=(n - k) {
        result *= i as f64 / (k + i) as f64;
    }
    result
}

/// Every `k`-subset of `0..n`, as index lists, in lexicographic order.
///
/// Upstream uses `ext::for_each_combination`, an in-place next-combination
/// over a range; the order is the same and it is the order the CCF events are
/// created in.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if k == 0 || k > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.clone());
        // Advance to the next combination, or stop.
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if idx[i] != i + n - k {
                idx[i] += 1;
                for j in i + 1..k {
                    idx[j] = idx[j - 1] + 1;
                }
                break;
            }
        }
    }
}

/// The proxy-gate arguments a set of CCF events implies: member id -> the
/// events that couple it.
///
/// Upstream builds this while generating the events; separating it keeps
/// [`CcfGroup`] free of the tree types.
pub fn proxy_arguments(events: &[CcfEvent]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for event in events {
        for member in &event.members {
            out.entry(member.clone())
                .or_default()
                .push(event.id.clone());
        }
    }
    out
}
