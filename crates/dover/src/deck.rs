// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The TOML input deck — a validated, typed document, not free text.
//!
//! # Shape
//!
//! ```toml
//! schema_version = 1
//!
//! [case]
//! name = "smr-base"
//!
//! [reactor]
//! kind = "cstr"
//! volume_m3 = 2.0
//! temperature_k = 1100.0
//! pressure_pa = 2.0e6
//!
//! [feed]
//! methane_mol_s = 1.0
//! steam_to_carbon = 3.0
//!
//! # The only fitted inputs in the whole model (see `crate::smr`).
//! [kinetics.reforming]
//! a_forward = 1.0e7
//! e_forward_j_mol = 2.4e5
//!
//! [kinetics.water_gas_shift]
//! a_forward = 1.0e6
//! e_forward_j_mol = 6.7e4
//!
//! # Optional: sweep one variable instead of running a single point.
//! [sweep]
//! variable = "temperature_k"
//! from = 800.0
//! to = 1300.0
//! steps = 11
//! ```
//!
//! # Why validation is separate from deserialisation
//!
//! `serde` will happily accept `volume_m3 = -2.0` or `steam_to_carbon = 0`:
//! both are well-formed floats. A deck that parses but describes an
//! impossible reactor is worse than one that fails to parse, because the
//! solver will return *a number* for it. [`Deck::validate`] is therefore run
//! by [`Deck::from_toml`] and is not optional — there is no path that hands
//! back an unvalidated deck.
//!
//! Unknown fields are **rejected** (`deny_unknown_fields`). A misspelled key
//! that is silently ignored is how a deck ends up not describing the run
//! someone thought it did.

use serde::{Deserialize, Serialize};

use crate::smr::{ForwardRate, SmrCase};

/// The deck schema version this crate reads and writes.
///
/// Bumped whenever a change would make an older deck mean something
/// different. A deck carrying any other value is rejected rather than
/// interpreted, since guessing at an unknown schema is how a deck silently
/// changes meaning between versions.
pub const SCHEMA_VERSION: u32 = 1;

/// Why a deck was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckError {
    /// The TOML did not parse, or carried an unknown key.
    Parse(String),
    /// It parsed, but describes something unphysical or unsupported.
    Invalid(String),
}

impl std::fmt::Display for DeckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeckError::Parse(m) => write!(f, "deck does not parse: {m}"),
            DeckError::Invalid(m) => write!(f, "deck is invalid: {m}"),
        }
    }
}

impl std::error::Error for DeckError {}

/// A complete DOVER input deck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    /// Must equal [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Case identification.
    pub case: CaseBlock,
    /// Reactor geometry and operating point.
    pub reactor: ReactorBlock,
    /// Inlet specification.
    pub feed: FeedBlock,
    /// The fitted forward rates.
    pub kinetics: KineticsBlock,
    /// Optional single-variable sweep. Absent means one operating point.
    #[serde(default)]
    pub sweep: Option<SweepBlock>,
}

/// Case identification. Carries no physics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseBlock {
    /// A short name, echoed into the output so a trace identifies itself.
    pub name: String,
}

/// Reactor geometry and operating point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReactorBlock {
    /// Reactor model. Only `"cstr"` is implemented; naming anything else is
    /// an error rather than a silent fallback.
    pub kind: String,
    /// Tank volume [m³].
    pub volume_m3: f64,
    /// Temperature [K], held constant.
    pub temperature_k: f64,
    /// Pressure [Pa].
    pub pressure_pa: f64,
}

/// Inlet specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedBlock {
    /// Methane molar flow [mol/s].
    pub methane_mol_s: f64,
    /// Steam-to-carbon ratio [-].
    pub steam_to_carbon: f64,
}

/// The fitted forward Arrhenius pairs — the only calibrated inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KineticsBlock {
    /// Forward rate for `CH4 + H2O <=> CO + 3H2`.
    pub reforming: RateBlock,
    /// Forward rate for `CO + H2O <=> CO2 + H2`.
    pub water_gas_shift: RateBlock,
}

/// One forward Arrhenius pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateBlock {
    /// Pre-exponential factor (units per the reaction's orders).
    pub a_forward: f64,
    /// Activation energy [J/mol].
    pub e_forward_j_mol: f64,
}

/// Sweep one variable across a range instead of running a single point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepBlock {
    /// Which variable: `"temperature_k"`, `"pressure_pa"`,
    /// `"steam_to_carbon"` or `"volume_m3"`.
    pub variable: String,
    /// First value (inclusive).
    pub from: f64,
    /// Last value (inclusive).
    pub to: f64,
    /// Number of points, at least 2.
    pub steps: usize,
}

impl SweepBlock {
    /// The swept values, `from` to `to` inclusive.
    #[must_use]
    pub fn values(&self) -> Vec<f64> {
        if self.steps < 2 {
            return vec![self.from];
        }
        let n = self.steps - 1;
        (0..self.steps)
            .map(|i| self.from + (self.to - self.from) * (i as f64) / (n as f64))
            .collect()
    }
}

impl Deck {
    /// Parse and validate a deck from TOML text.
    ///
    /// # Errors
    ///
    /// [`DeckError::Parse`] if the TOML is malformed or names an unknown key;
    /// [`DeckError::Invalid`] if it parses but fails [`Deck::validate`].
    pub fn from_toml(text: &str) -> Result<Self, DeckError> {
        let deck: Deck = toml::from_str(text).map_err(|e| DeckError::Parse(e.to_string()))?;
        deck.validate()?;
        Ok(deck)
    }

    /// Serialise back to TOML.
    ///
    /// # Errors
    ///
    /// [`DeckError::Parse`] if serialisation fails, which in practice it does
    /// not for this schema.
    pub fn to_toml(&self) -> Result<String, DeckError> {
        toml::to_string_pretty(self).map_err(|e| DeckError::Parse(e.to_string()))
    }

    /// Check the deck describes a physically meaningful, supported run.
    ///
    /// # Errors
    ///
    /// [`DeckError::Invalid`] naming the first problem found.
    pub fn validate(&self) -> Result<(), DeckError> {
        let bad = |m: String| Err(DeckError::Invalid(m));

        if self.schema_version != SCHEMA_VERSION {
            return bad(format!(
                "schema_version {} is not the {SCHEMA_VERSION} this build reads",
                self.schema_version
            ));
        }
        if self.case.name.trim().is_empty() {
            return bad("case.name is empty".into());
        }
        if self.reactor.kind != "cstr" {
            return bad(format!(
                "reactor.kind {:?} is not implemented; only \"cstr\" is",
                self.reactor.kind
            ));
        }
        for (label, v) in [
            ("reactor.volume_m3", self.reactor.volume_m3),
            ("reactor.temperature_k", self.reactor.temperature_k),
            ("reactor.pressure_pa", self.reactor.pressure_pa),
            ("feed.methane_mol_s", self.feed.methane_mol_s),
            ("feed.steam_to_carbon", self.feed.steam_to_carbon),
        ] {
            if !(v > 0.0) || !v.is_finite() {
                return bad(format!("{label} must be finite and positive, got {v}"));
            }
        }
        for (label, r) in [
            ("kinetics.reforming", self.kinetics.reforming),
            ("kinetics.water_gas_shift", self.kinetics.water_gas_shift),
        ] {
            if !(r.a_forward > 0.0) || !r.a_forward.is_finite() {
                return bad(format!(
                    "{label}.a_forward must be finite and positive, got {}",
                    r.a_forward
                ));
            }
            if r.e_forward_j_mol < 0.0 || !r.e_forward_j_mol.is_finite() {
                return bad(format!(
                    "{label}.e_forward_j_mol must be finite and non-negative, got {}",
                    r.e_forward_j_mol
                ));
            }
        }
        if let Some(s) = &self.sweep {
            const KNOWN: [&str; 4] = [
                "temperature_k",
                "pressure_pa",
                "steam_to_carbon",
                "volume_m3",
            ];
            if !KNOWN.contains(&s.variable.as_str()) {
                return bad(format!(
                    "sweep.variable {:?} is not one of {KNOWN:?}",
                    s.variable
                ));
            }
            if s.steps < 2 {
                return bad(format!("sweep.steps must be at least 2, got {}", s.steps));
            }
            if !s.from.is_finite() || !s.to.is_finite() {
                return bad("sweep bounds must be finite".into());
            }
            if !(s.from > 0.0) || !(s.to > 0.0) {
                return bad("sweep bounds must be positive for every supported variable".into());
            }
        }
        Ok(())
    }

    /// The single operating point this deck describes, ignoring any sweep.
    #[must_use]
    pub fn base_case(&self) -> SmrCase {
        SmrCase {
            methane_feed: self.feed.methane_mol_s,
            steam_to_carbon: self.feed.steam_to_carbon,
            temperature: self.reactor.temperature_k,
            pressure: self.reactor.pressure_pa,
            volume: self.reactor.volume_m3,
            forward: [
                ForwardRate {
                    a: self.kinetics.reforming.a_forward,
                    e: self.kinetics.reforming.e_forward_j_mol,
                },
                ForwardRate {
                    a: self.kinetics.water_gas_shift.a_forward,
                    e: self.kinetics.water_gas_shift.e_forward_j_mol,
                },
            ],
        }
    }

    /// Every case the deck describes: one, or the sweep expanded.
    #[must_use]
    pub fn cases(&self) -> Vec<(f64, SmrCase)> {
        let base = self.base_case();
        match &self.sweep {
            None => vec![(f64::NAN, base)],
            Some(s) => s
                .values()
                .into_iter()
                .map(|v| {
                    let mut c = base.clone();
                    match s.variable.as_str() {
                        "temperature_k" => c.temperature = v,
                        "pressure_pa" => c.pressure = v,
                        "steam_to_carbon" => c.steam_to_carbon = v,
                        "volume_m3" => c.volume = v,
                        // `validate` has already rejected anything else.
                        _ => unreachable!("validate rejects unknown sweep variables"),
                    }
                    (v, c)
                })
                .collect(),
        }
    }
}
