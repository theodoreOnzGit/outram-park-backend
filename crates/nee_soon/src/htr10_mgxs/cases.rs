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

//! The registry of HTR-10 MGXS cases.
//!
//! **The registry is the specification of every run.** A case's model,
//! temperature, particle budget and seed are fixed here, in committed source,
//! before it runs; the record a run writes names its case and commit, so the
//! two can always be matched. Changing a case after it has produced a record
//! means giving it a new name.

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use super::cells::CellKind;

/// The model a case transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseModel {
    /// The full HTR-10 core: Şeker & Çolak (2003)'s 13-ball bed of `layers`
    /// layers with explicit TRISO, the explicit reflector and the ten rods
    /// withdrawn, as
    /// [`crate::htr10_rmc::core_model::assemble_explicit_triso`] builds it.
    /// `layers = 12` is the bed every recorded `k` in
    /// `verification_and_validation/htr10_seker_2026_10_07_10k/` is on.
    FullCore {
        /// Şeker layers N (bed height `9.798 N + 6` cm).
        layers: usize,
    },
    /// One of the small cells of [`super::cells`] (added 2026-10-13,
    /// gh:#849): the single pebble or a Şeker cluster, built from the core's
    /// own particle and pebbles, reflective all round.
    Cell(CellKind),
}

impl CaseModel {
    /// One-line description for the record.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::FullCore { layers } => format!(
                "full HTR-10 core, Şeker 13-ball bed N = {layers} ({:.3} cm), explicit TRISO, explicit reflector, rods withdrawn",
                9.798 * layers as f64 + 6.0
            ),
            Self::Cell(kind) => kind.describe(),
        }
    }
}

/// One named MGXS run. See the module docs: the fields are the run's
/// specification and are fixed in [`MgxsCase::all`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MgxsCase {
    /// Unique name; the record folder and the command-line argument.
    pub name: &'static str,
    /// Why the case exists, in one sentence.
    pub purpose: &'static str,
    /// The GitHub issue the result feeds.
    pub issue: &'static str,
    /// What is transported.
    pub model: CaseModel,
    /// Every material's temperature \[K\]; read through
    /// [`Self::temperature`] at the API.
    temperature_k: f64,
    /// Particles per generation.
    pub particles: usize,
    /// Inactive (discarded) generations.
    pub inactive: usize,
    /// Active (tallied) generations.
    pub active: usize,
    /// RNG seed; both passes use it.
    pub seed: u64,
    /// `false` for a check of the pipeline whose numbers are not a record
    /// (its output goes to a scratch folder, not the V&V tree).
    pub is_record: bool,
}

/// The ENDF/B-VIII.0 `k_eff` already recorded for the N = 12 core at
/// 300.15 K, `(k, sigma)`: 10 000 × \[5 + 135\], seed 20 260 917
/// (`verification_and_validation/htr10_seker_2026_10_07_10k/results_table.md`).
/// A case on the same core and temperature should agree with it within
/// statistics; it uses a different seed and more inactive generations, so
/// it is an independent repeat, not a copy.
pub const RECORDED_K_N12_T300: (f64, f64) = (0.995125, 0.001055);

/// The seed of the recorded cases (the date the registry was written).
pub const SEED: u64 = 20_261_012;
/// A second, independent seed: the replica case's.
pub const REPLICA_SEED: u64 = 20_261_013;

/// Particles per generation of a recorded case: the reference paper's own
/// 10 000 (Li, Yu & Wei 2014), as the 2026-10-07 `k` record uses.
pub const RECORD_PARTICLES: usize = 10_000;
/// Inactive generations of a recorded case.
///
/// 20, not the `k` record's 5: a tally is only as good as the source it is
/// scored from, and the HTR-10's dominance ratio is close to 1, so the source
/// converges slowly. The record keeps the entropy trace so this can be
/// checked rather than assumed.
pub const RECORD_INACTIVE: usize = 20;
/// Active generations of a recorded case (the `k` record's 135).
pub const RECORD_ACTIVE: usize = 135;

/// Particles per generation of a cell pilot (`pilot_*`).
pub const PILOT_PARTICLES: usize = 2_000;
/// Inactive generations of a cell pilot. A reflective cell has no leakage
/// mode to settle, so 10 suffices where the full core needs 20; the record's
/// entropy trace says whether it did.
pub const PILOT_INACTIVE: usize = 10;
/// Active generations of a cell pilot.
pub const PILOT_ACTIVE: usize = 40;

impl MgxsCase {
    const fn record(
        name: &'static str,
        purpose: &'static str,
        issue: &'static str,
        layers: usize,
        temperature_k: f64,
        seed: u64,
    ) -> Self {
        Self {
            name,
            purpose,
            issue,
            model: CaseModel::FullCore { layers },
            temperature_k,
            particles: RECORD_PARTICLES,
            inactive: RECORD_INACTIVE,
            active: RECORD_ACTIVE,
            seed,
            is_record: true,
        }
    }

    const fn cell_record(
        name: &'static str,
        purpose: &'static str,
        kind: CellKind,
    ) -> Self {
        Self {
            name,
            purpose,
            issue: "#849 (#850)",
            model: CaseModel::Cell(kind),
            temperature_k: 300.15,
            particles: RECORD_PARTICLES,
            inactive: RECORD_INACTIVE,
            active: RECORD_ACTIVE,
            seed: SEED,
            is_record: true,
        }
    }

    /// A pilot of a cell case: the same cell, temperature and seed at
    /// [`PILOT_PARTICLES`] × \[[`PILOT_INACTIVE`] + [`PILOT_ACTIVE`]\], so the
    /// cell-size convergence can be measured on a 4-core machine in about an
    /// hour per cell. Not a record: the record is the `cell_*_t300` case.
    const fn cell_pilot(name: &'static str, kind: CellKind) -> Self {
        Self {
            name,
            purpose: "pilot of the matching cell_*_t300 case at a small budget: cell-size convergence of k_inf and the constants, not a record",
            issue: "#849",
            model: CaseModel::Cell(kind),
            temperature_k: 300.15,
            particles: PILOT_PARTICLES,
            inactive: PILOT_INACTIVE,
            active: PILOT_ACTIVE,
            seed: SEED,
            is_record: false,
        }
    }

    /// Every case, in the order they should be run.
    ///
    /// | case | model | T \[K\] | budget per pass | feeds |
    /// |---|---|---|---|---|
    /// | `smoke` | full core N = 12 | 300.15 | 500 × \[2 + 3\] | a pipeline check, not a record |
    /// | `verify_tallies` | full core N = 12 | 300.15 | 4000 × \[10 + 40\] | the tally checks at a resolvable budget, not queued |
    /// | `core_n12_t300` | full core N = 12 | 300.15 | 10 000 × \[20 + 135\] | #850 / #851 / #854 reference state |
    /// | `core_n12_t600` | as above | 600 | as above | #852 |
    /// | `core_n12_t900` | as above | 900 | as above | #852 |
    /// | `core_n12_t1200` | as above | 1200 | as above | #852 |
    /// | `core_n12_t300_replica` | as `core_n12_t300`, seed 20 261 013 | 300.15 | as above | the MGXS statistical spread |
    /// | `cell_pebble_t300` | single pebble, reflective 6 cm cube | 300.15 | 10 000 × \[20 + 135\] | #850's simplest gate cell |
    /// | `cell_seker1_t300` | Şeker cluster, 1 tile | 300.15 | as above | #849 side-face convergence |
    /// | `cell_seker7_t300` | Şeker cluster, 7 tiles | 300.15 | as above | #849 side-face convergence |
    /// | `cell_seker19_t300` | Şeker cluster, 19 tiles | 300.15 | as above | #849 side-face convergence, #850's production cell |
    /// | `pilot_pebble`, `pilot_seker1`, `pilot_seker7`, `pilot_seker19` | the four cells above | 300.15 | 2000 × \[10 + 40\] | #849's convergence pilot, not records |
    ///
    /// 300.15 K (27 °C) is the temperature of Li, Yu & Wei (2014) and Şeker
    /// & Çolak (2003), and of every recorded HTR-10 `k` here.
    #[must_use]
    pub fn all() -> Vec<MgxsCase> {
        vec![
            Self {
                name: "smoke",
                purpose: "check the whole pipeline on this machine in minutes before a long case is started",
                issue: "#845",
                model: CaseModel::FullCore { layers: 12 },
                temperature_k: 300.15,
                particles: 500,
                inactive: 2,
                active: 3,
                seed: SEED,
                is_record: false,
            },
            Self {
                name: "verify_tallies",
                purpose: "check the tallies on the full core at a budget a 4-core machine finishes in about an hour: tallied production/absorption against k_eff, Sigma_f <= Sigma_a <= Sigma_t, nu-bar, balance",
                issue: "#845",
                model: CaseModel::FullCore { layers: 12 },
                temperature_k: 300.15,
                particles: 4000,
                inactive: 10,
                active: 40,
                seed: SEED,
                is_record: false,
            },
            Self::record(
                "core_n12_t300",
                "in-core MGXS of the recorded N = 12 core at the benchmark temperature: the reference state",
                "#845 (#850, #851, #854)",
                12,
                300.15,
                SEED,
            ),
            Self::record(
                "core_n12_t600",
                "isothermal state point at 600 K",
                "#852",
                12,
                600.0,
                SEED,
            ),
            Self::record(
                "core_n12_t900",
                "isothermal state point at 900 K",
                "#852",
                12,
                900.0,
                SEED,
            ),
            Self::record(
                "core_n12_t1200",
                "isothermal state point at 1200 K",
                "#852",
                12,
                1200.0,
                SEED,
            ),
            Self::record(
                "core_n12_t300_replica",
                "core_n12_t300 at an independent seed, so the statistical spread of the constants is measured rather than assumed",
                "#850",
                12,
                300.15,
                REPLICA_SEED,
            ),
            Self::cell_record(
                "cell_pebble_t300",
                "MGXS of one explicit-TRISO pebble in a reflective 6 cm cube (packing 0.524): the simplest cell for the MG-vs-CE gate",
                CellKind::SinglePebble,
            ),
            Self::cell_record(
                "cell_seker1_t300",
                "MGXS of one Şeker tile, reflective: the smallest cluster, for the side-face convergence",
                CellKind::SekerCluster { rings: 1 },
            ),
            Self::cell_record(
                "cell_seker7_t300",
                "MGXS of a 7-tile Şeker cluster, reflective: side-face convergence",
                CellKind::SekerCluster { rings: 2 },
            ),
            Self::cell_record(
                "cell_seker19_t300",
                "MGXS of a 19-tile Şeker cluster, reflective: side-face convergence and the production cell at the core's packing",
                CellKind::SekerCluster { rings: 3 },
            ),
            Self::cell_pilot("pilot_pebble", CellKind::SinglePebble),
            Self::cell_pilot("pilot_seker1", CellKind::SekerCluster { rings: 1 }),
            Self::cell_pilot("pilot_seker7", CellKind::SekerCluster { rings: 2 }),
            Self::cell_pilot("pilot_seker19", CellKind::SekerCluster { rings: 3 }),
        ]
    }

    /// The case named `name`, if there is one.
    #[must_use]
    pub fn find(name: &str) -> Option<MgxsCase> {
        Self::all().into_iter().find(|c| c.name == name)
    }

    /// Every material's temperature.
    #[must_use]
    pub fn temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.temperature_k)
    }

    /// Histories one pass simulates, inactive generations included. A case
    /// runs two passes.
    #[must_use]
    pub fn histories_per_pass(&self) -> u64 {
        (self.particles * (self.inactive + self.active)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Names are unique (they are folder names), the recorded cases share one
    /// budget, and only `smoke`, `verify_tallies` and the cell pilots are not
    /// records.
    #[test]
    fn the_registry_is_consistent() {
        let all = MgxsCase::all();
        let mut names: Vec<&str> = all.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len(), "case names must be unique");
        for c in &all {
            assert!(c.name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
            if c.is_record {
                assert_eq!(
                    (c.particles, c.inactive, c.active),
                    (RECORD_PARTICLES, RECORD_INACTIVE, RECORD_ACTIVE)
                );
                assert_eq!(c.histories_per_pass(), 1_550_000);
            } else {
                assert!(
                    ["smoke", "verify_tallies"].contains(&c.name) || c.name.starts_with("pilot_"),
                    "{}",
                    c.name
                );
            }
        }
        let t = MgxsCase::find("core_n12_t900").expect("listed");
        assert!((t.temperature().get::<kelvin>() - 900.0).abs() < 1e-9);
        assert!(MgxsCase::find("no_such_case").is_none());
        assert_ne!(
            MgxsCase::find("core_n12_t300").map(|c| c.seed),
            MgxsCase::find("core_n12_t300_replica").map(|c| c.seed)
        );
    }
}
