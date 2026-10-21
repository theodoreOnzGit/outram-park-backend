// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/alignment.{h,cc}, and the phase application of
//                     src/risk_analysis.cc (`RiskAnalysis::RunAnalysis`)
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: upstream applies a phase by MUTATING the model — it
// scales `model_->mission_time()`, flips the named house events, and restores
// both with a `scope_guard` when the analysis returns. This port returns a
// new model instead ([`super::mef::MefModel::in_phase`]), so nothing has to
// be restored and two phases cannot interfere.
//
// A phase's instructions are `std::vector<SetHouseEvent*>` upstream: the one
// instruction type a phase may carry. The rest of `instruction.{h,cc}`
// belongs to event trees and is not ported here.
// ---------------------------------------------------------------------------

//! Alignments — the same model analysed in several operating configurations.
//!
//! A plant is not in one state all year. It runs normally most of the time,
//! with one pump out for maintenance some of it, and the risk is different in
//! each. An **alignment** names those configurations, gives each a fraction of
//! the mission time, and says which house events are set in it.
//!
//! # What a phase changes
//!
//! Two things, and upstream's `RiskAnalysis::RunAnalysis` does both before
//! running the whole analysis again:
//!
//! 1. the **mission time** becomes `time_fraction x mission_time`, so every
//!    `<exponential>` and `<periodic-test>` is evaluated over that phase's
//!    duration;
//! 2. the phase's `<set-house-event>` instructions flip the named house
//!    events, which prunes whole branches of the tree.
//!
//! There is **no single answer** for a model with an alignment: upstream
//! produces one result set per phase, tagged with the alignment and phase
//! names, and so does this. [`super::mef::MefModel::in_phase`] returns the
//! model as it stands in one phase; the caller loops.
//!
//! The fractions must sum to 1 — upstream checks it to `1e-4` — so a caller
//! wanting a single number can combine the per-phase results by those
//! fractions. That combination is the analyst's to make, and neither upstream
//! nor this port makes it for them.

use crate::{RafflesError, Result};

fn invalid(reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: "alignment".to_string(),
        value: 0.0,
        reason,
    }
}

/// One operating configuration within an [`Alignment`].
#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
    /// Its name, as the report tags the phase's results.
    pub name: String,
    /// The fraction of the mission time spent in it, in `(0, 1]`.
    pub time_fraction: f64,
    /// `<set-house-event>` instructions: `(house-event id, state)`.
    pub set_house_events: Vec<(String, bool)>,
}

/// A set of phases covering the whole mission time.
#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    /// Its id.
    pub name: String,
    /// Its phases, in declaration order.
    pub phases: Vec<Phase>,
}

impl Alignment {
    /// Upstream's `Phase::Phase` domain check and `Alignment::Validate`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a fraction is outside `(0, 1]`,
    /// if the alignment has no phase, or if the fractions do not sum to 1
    /// within `1e-4` — upstream's own tolerance, not one chosen here.
    pub fn validate(&self) -> Result<()> {
        if self.phases.is_empty() {
            return Err(invalid(format!("`{}` declares no phase", self.name)));
        }
        let mut sum = 0.0;
        for phase in &self.phases {
            if phase.time_fraction <= 0.0 || phase.time_fraction > 1.0 {
                return Err(invalid(format!(
                    "`{}`: the phase fraction of `{}` must be in (0, 1], found {}",
                    self.name, phase.name, phase.time_fraction
                )));
            }
            sum += phase.time_fraction;
        }
        if (sum - 1.0).abs() > 1e-4 {
            return Err(invalid(format!(
                "`{}`: the phases do not sum to 1, they sum to {sum}",
                self.name
            )));
        }
        Ok(())
    }

    /// The phase of that name.
    pub fn phase(&self, name: &str) -> Option<&Phase> {
        self.phases.iter().find(|p| p.name == name)
    }
}
