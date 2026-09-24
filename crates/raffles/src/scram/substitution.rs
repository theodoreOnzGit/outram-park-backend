// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/substitution.{h,cc}, the substitution handling of
//                     src/pdag.cc (`ConstructSubstitution`,
//                     `CollectSubstitution`) and src/zbdd.cc
//                     (`ApplySubstitutions`)
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: upstream's `Substitution::Target` is a
// `std::variant<BasicEvent*, bool>`, which becomes [`SubstitutionTarget`];
// its `std::optional<Type>` inference is [`Substitution::inferred_type`],
// ported branch for branch. The two APPLICATIONS live in different layers
// upstream and do here too: a declarative substitution is a tree rewrite
// (`Pdag::ConstructSubstitution`, applied in [`super::mef`]), and a
// non-declarative one is a pass over the generated products
// (`Zbdd::ApplySubstitutions`, which is [`apply_to_products`]).
// ---------------------------------------------------------------------------

//! Substitutions — delete terms, recovery rules and exchange events.
//!
//! A fault tree says how a system fails. A substitution says something the
//! tree cannot: that two events are mutually exclusive, that a recovery action
//! applies when a particular combination occurs, or that one event stands in
//! for another under some condition.
//!
//! # Two kinds, applied in two different places
//!
//! A substitution with **no `<source>`** is *declarative*: it is a statement
//! about the world, and upstream applies it to the **graph** as a logical
//! implication. `hypothesis -> target` becomes `or(not hypothesis, target)`,
//! and the analysed root becomes `and(root, that)`. A `false` target — a
//! delete term — has no target to imply, so the conjunct is just
//! `not hypothesis`.
//!
//! A substitution **with** a `<source>` is *non-declarative*: it is a
//! post-processing rule, and upstream applies it to the **products**. For
//! every product containing the whole hypothesis, the source events are
//! removed and the target added; the result is then re-minimised. That is
//! [`apply_to_products`].
//!
//! **Upstream refuses an exact analysis with non-declarative substitutions**
//! — "Non-declarative substitutions do not apply to exact analyses." The
//! substituted product list is no longer the minimal cut sets of any Boolean
//! function, so inclusion-exclusion over it means nothing; only the
//! rare-event and MCUB approximations, which treat the products as a list,
//! stay meaningful. That is measured, not inferred: the compiled binary
//! refuses `--probability` on `TwoTrain/nondeclarative_substitutions.xml` and
//! runs it under `--rare-event`.

use super::mef::{Formula, FormulaArg, MefConnective};
use super::probability::CutSet;
use crate::{RafflesError, Result};

/// The "traditional" substitution types of the MEF.
///
/// Upstream treats these as a *classification* rather than as behaviour: the
/// `type` attribute is optional, and `Substitution::type()` infers which one a
/// substitution amounts to from its shape. Nothing in the analysis branches on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstitutionType {
    /// `delete-terms`: the hypothesis is impossible.
    DeleteTerms,
    /// `recovery-rule`: the hypothesis implies the target.
    RecoveryRule,
    /// `exchange-event`: one source event is replaced by the target.
    ExchangeEvent,
}

impl SubstitutionType {
    /// The MEF attribute value — upstream's `kSubstitutionTypeToString`.
    pub fn as_str(self) -> &'static str {
        match self {
            SubstitutionType::DeleteTerms => "delete-terms",
            SubstitutionType::RecoveryRule => "recovery-rule",
            SubstitutionType::ExchangeEvent => "exchange-event",
        }
    }

    /// Reads the MEF attribute value.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] for anything outside the grammar's
    /// three values.
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "delete-terms" => SubstitutionType::DeleteTerms,
            "recovery-rule" => SubstitutionType::RecoveryRule,
            "exchange-event" => SubstitutionType::ExchangeEvent,
            other => {
                return Err(invalid(format!(
                    "`{other}` is not a substitution type; MEF has delete-terms, \
                     recovery-rule and exchange-event"
                )))
            }
        })
    }
}

/// What a substitution puts in place of its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubstitutionTarget {
    /// A basic event, by resolved id.
    Event(String),
    /// `<constant value="true|false"/>`.
    Constant(bool),
}

fn invalid(reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: "substitution".to_string(),
        value: 0.0,
        reason,
    }
}

/// A substitution.
#[derive(Debug, Clone)]
pub struct Substitution {
    /// Its id.
    pub name: String,
    /// The `type` attribute, if the model gave one. Optional in the grammar.
    pub declared_type: Option<SubstitutionType>,
    /// A simple Boolean formula over basic events only.
    pub hypothesis: Formula,
    /// What replaces the source, or what the hypothesis implies.
    pub target: SubstitutionTarget,
    /// The events replaced, by resolved id. **Empty means declarative.**
    pub source: Vec<String>,
}

impl Substitution {
    /// Upstream's `declarative()`: no source events.
    pub fn declarative(&self) -> bool {
        self.source.is_empty()
    }

    /// Upstream's `Substitution::Validate`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the hypothesis has a complemented
    /// argument, uses a connective the kind of substitution does not allow, or
    /// the target makes the substitution a no-op.
    pub fn validate(&self) -> Result<()> {
        if self.hypothesis.args.iter().any(|a| {
            matches!(
                a,
                FormulaArg::Event {
                    complement: true,
                    ..
                }
            )
        }) {
            return Err(invalid(format!(
                "`{}`: substitution hypotheses must be coherent",
                self.name
            )));
        }
        if self
            .hypothesis
            .args
            .iter()
            .any(|a| matches!(a, FormulaArg::Constant(_)))
        {
            return Err(invalid(format!(
                "`{}`: substitution hypothesis must be built over basic events only",
                self.name
            )));
        }
        if self.declarative() {
            if !matches!(
                self.hypothesis.connective,
                MefConnective::Null
                    | MefConnective::And
                    | MefConnective::Atleast { .. }
                    | MefConnective::Or
            ) {
                return Err(invalid(format!(
                    "`{}`: substitution hypotheses must be coherent, not <{:?}>",
                    self.name, self.hypothesis.connective
                )));
            }
            if self.target == SubstitutionTarget::Constant(true) {
                return Err(invalid(format!(
                    "`{}`: substitution has no effect",
                    self.name
                )));
            }
        } else {
            if !matches!(
                self.hypothesis.connective,
                MefConnective::Null | MefConnective::And | MefConnective::Or
            ) {
                return Err(invalid(format!(
                    "`{}`: non-declarative substitution hypotheses only allow AND/OR/NULL \
                     connectives",
                    self.name
                )));
            }
            if self.target == SubstitutionTarget::Constant(false) {
                return Err(invalid(format!(
                    "`{}`: substitution source set is irrelevant",
                    self.name
                )));
            }
        }
        Ok(())
    }

    /// Upstream's `Substitution::type()`: which "traditional" type this
    /// amounts to, if any.
    ///
    /// Ported branch for branch. Note that a declarative delete term is
    /// recognised only when its hypothesis is *mutually exclusive* — an `and`
    /// of exactly two arguments, or an `atleast` with `min == 2` — which is
    /// narrower than "a hypothesis that cannot happen".
    pub fn inferred_type(&self) -> Option<SubstitutionType> {
        let hypothesis_names: Vec<&str> = self
            .hypothesis
            .args
            .iter()
            .filter_map(|a| match a {
                FormulaArg::Event { name, .. } => Some(name.as_str()),
                FormulaArg::Constant(_) => None,
            })
            .collect();
        let in_hypothesis = |s: &String| hypothesis_names.iter().any(|h| *h == s.as_str());
        let mutually_exclusive = match self.hypothesis.connective {
            MefConnective::Atleast { min } => min == 2,
            MefConnective::And => self.hypothesis.args.len() == 2,
            _ => false,
        };

        if self.source.is_empty() {
            return match &self.target {
                SubstitutionTarget::Constant(_) if mutually_exclusive => {
                    Some(SubstitutionType::DeleteTerms)
                }
                SubstitutionTarget::Event(_)
                    if self.hypothesis.connective == MefConnective::And =>
                {
                    Some(SubstitutionType::RecoveryRule)
                }
                _ => None,
            };
        }
        if !matches!(self.target, SubstitutionTarget::Event(_)) {
            return None;
        }
        if !matches!(
            self.hypothesis.connective,
            MefConnective::And | MefConnective::Null
        ) {
            return None;
        }
        if self.source.len() == hypothesis_names.len() {
            if self.source.iter().all(in_hypothesis) {
                return Some(SubstitutionType::RecoveryRule);
            }
        } else if self.source.len() == 1 && in_hypothesis(&self.source[0]) {
            return Some(SubstitutionType::ExchangeEvent);
        }
        None
    }
}

/// A non-declarative substitution, in basic-event indices — upstream's
/// `Pdag::Substitution`.
///
/// Upstream splits an `or` hypothesis into one of these per argument, so that
/// every entry's hypothesis is a plain conjunction to test against a product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSubstitution {
    /// Every one of these must be in a product for the rule to fire.
    pub hypothesis: Vec<usize>,
    /// Events removed from the product.
    pub source: Vec<usize>,
    /// The event added, if any. `None` is upstream's index `0`, which it uses
    /// for a constant target.
    pub target: Option<usize>,
}

/// Upstream's `Zbdd::ApplySubstitutions`, as a pass over products.
///
/// For every product, each rule whose whole hypothesis is present removes its
/// source events and adds its target; the rewritten products are then
/// re-minimised, since a substitution can make one product a superset of
/// another.
///
/// Upstream does this inside the ZBDD, where `Apply<kAnd>`/`Apply<kOr>`
/// rebuild the set and `Minimize` subsumes; here the products are already a
/// list, so the same operations are set operations on it. The **result** is
/// what is checked, against SCRAM's own reported products.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if a substitution empties a product
/// entirely, which would make the top event unconditional — upstream's
/// `Apply<kAnd>` over no arguments is the `Base` set, and a product list
/// containing it is the UNITY case the generators here refuse by name.
pub fn apply_to_products(
    products: &[CutSet],
    substitutions: &[ProductSubstitution],
) -> Result<Vec<CutSet>> {
    if substitutions.is_empty() {
        return Ok(products.to_vec());
    }
    let mut rewritten: Vec<Vec<usize>> = Vec::with_capacity(products.len());
    for product in products {
        let members: Vec<usize> = product.members().to_vec();
        let mut to_remove: Vec<usize> = Vec::new();
        let mut to_add: Vec<usize> = Vec::new();
        for rule in substitutions {
            if rule.hypothesis.iter().all(|h| members.contains(h)) {
                to_remove.extend(rule.source.iter().copied());
                if let Some(target) = rule.target {
                    to_add.push(target);
                }
            }
        }
        let mut next: Vec<usize> = members
            .into_iter()
            .filter(|m| !to_remove.contains(m))
            .collect();
        for target in to_add {
            if !next.contains(&target) {
                next.push(target);
            }
        }
        next.sort_unstable();
        next.dedup();
        if next.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "substitution".to_string(),
                value: 0.0,
                reason: "a substitution emptied a product, so the top event is \
                         unconditional -- upstream calls this set UNITY/Base and it is \
                         not a cut set"
                    .to_string(),
            });
        }
        rewritten.push(next);
    }

    // Re-minimise: upstream's `Minimize` after the rebuild. A product that
    // contains another is subsumed.
    let mut minimal: Vec<Vec<usize>> = Vec::new();
    rewritten.sort_by_key(|p| p.len());
    rewritten.dedup();
    for candidate in rewritten {
        if minimal
            .iter()
            .any(|kept| kept.iter().all(|m| candidate.contains(m)))
        {
            continue;
        }
        minimal.push(candidate);
    }
    minimal.iter().map(|p| CutSet::new(p)).collect()
}
