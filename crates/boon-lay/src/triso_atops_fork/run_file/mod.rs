// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/run_functions.py
//                    (`convert_time`, `read_save_file`, `process_run_file`,
//                     `check_run_file`, `read_profile`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// Ported under GPL-3.0; see LICENSE.triso-atops and NOTICE.triso-atops.

//! The JSON run file: the document TRISO-ATOPS' GUI writes and its CLI reads.
//!
//! Upstream passes a bare `np.ndarray` of fifteen constants between every
//! layer, indexed by position — `constants[6]` is the plate-out rate,
//! `constants[14]` is the lift-off fraction, and nothing in the type system
//! says so. This module parses that document once into a named, `uom`-typed
//! [`RunConfig`], so an index slip becomes impossible rather than silent.
//!
//! # What is deliberately NOT ported
//!
//! Three `run_functions.py` entries have no Rust counterpart here, and their
//! absence is a decision rather than an omission:
//!
//! - **`create_log`** configures Python's `logging` module. Rust callers pick
//!   their own facade (`log`, `tracing`, or none); a library that installs a
//!   global logger is badly behaved. Diagnostics surface as
//!   [`RunFileError`] values instead, which a caller can log however it likes.
//! - **`count_errors`** is a counter upstream threads through every function
//!   because Python has no `Result`. Its job is done by `Result` here.
//! - **`trisoatops()`** prints a version banner to stdout.
//!
//! # Unit convention
//!
//! The JSON carries **bare numbers**, and upstream attaches units positionally
//! through a parallel `const_units` list: lengths in metres, rate constants in
//! s⁻¹, and **`run_time` and `irradiation_time` in years**. Those two are the
//! trap — a caller who assumes seconds is out by a factor of 3.15e7 — so
//! [`RunFile::to_config`] converts them explicitly and the field docs say so.

use serde::{Deserialize, Serialize};
use uom::si::f64::{Frequency, Length, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::time::second;

/// A time unit the run file may express a duration in.
///
/// Ports `convert_time`. Upstream returns `None` for an unrecognised unit and
/// the caller then multiplies by it, raising `TypeError` well away from the
/// mistake; here an unknown unit is a parse failure at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeUnit {
    /// Seconds (`"s"`), factor 1.
    Second,
    /// Minutes (`"min"`), factor 60.
    Minute,
    /// Hours (`"hr"`), factor 3600.
    Hour,
    /// Days (`"d"`), factor 86 400.
    Day,
    /// Years (`"yr"`), factor 31 536 000.
    ///
    /// **A 365-day year**, not the 365.25-day Julian year. Upstream spells it
    /// `365 * 24 * 3600`; the 0.07 % difference against a Julian year is small
    /// but systematic, so the port keeps upstream's definition rather than
    /// silently improving it.
    Year,
}

impl TimeUnit {
    /// Seconds per unit. Ports `convert_time`'s factor table exactly.
    #[must_use]
    pub fn seconds(self) -> f64 {
        match self {
            Self::Second => 1.0,
            Self::Minute => 60.0,
            Self::Hour => 3600.0,
            Self::Day => 3600.0 * 24.0,
            Self::Year => 365.0 * 24.0 * 3600.0,
        }
    }

    /// Parse upstream's unit string (`"s"`, `"min"`, `"hr"`, `"d"`, `"yr"`).
    ///
    /// # Returns
    /// `None` for anything else — upstream's `factor = None` case, surfaced as
    /// an `Option` instead of a `None` that only fails later.
    #[must_use]
    pub fn parse(unit: &str) -> Option<Self> {
        match unit {
            "s" => Some(Self::Second),
            "min" => Some(Self::Minute),
            "hr" => Some(Self::Hour),
            "d" => Some(Self::Day),
            "yr" => Some(Self::Year),
            _ => None,
        }
    }

    /// Convert a duration in this unit to a `uom` [`Time`].
    #[must_use]
    pub fn to_time(self, value: f64) -> Time {
        Time::new::<second>(value * self.seconds())
    }
}

/// Why a run file could not be turned into a [`RunConfig`].
///
/// Ports the conditions `check_run_file` and `process_run_file` log and count.
#[derive(Debug, Clone, PartialEq)]
pub enum RunFileError {
    /// A key the run demands is absent.
    MissingKey {
        /// The absent key, spelled as the JSON uses it.
        key: String,
    },
    /// A value is present but outside its physically admissible range.
    OutOfRange {
        /// Which key.
        key: String,
        /// The offending value.
        value: f64,
        /// What was required, in words (e.g. `"a fraction in [0, 1]"`).
        expected: String,
    },
    /// A temperature or inventory table does not match the declared node counts.
    ShapeMismatch {
        /// Which table.
        key: String,
        /// Elements found.
        found: usize,
        /// Elements the `n_radial x n_axial` declaration implies.
        expected: usize,
    },
}

/// The run file exactly as it appears on disk.
///
/// Field names match the JSON keys upstream's `required_keys` /
/// `accident_keys` lists demand, so `serde` reads a GUI-written file directly.
/// Every quantity is a bare number here; [`RunFile::to_config`] is what
/// attaches units and validates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunFile {
    /// Heavy-metal contamination fraction, dimensionless.
    pub f_hm: f64,
    /// As-manufactured SiC-defective fraction, dimensionless.
    pub f_sic: f64,
    /// Incremental in-service failure fraction, dimensionless.
    pub f_inc: f64,
    /// Incremental SiC-only failure fraction, dimensionless.
    pub f_inc_sic: f64,
    /// Matrix graphite thickness, **metres**.
    pub a_graph: f64,
    /// Fuel grain size, **metres**.
    pub a_grain: f64,
    /// Plate-out rate constant, **s⁻¹**.
    pub k_plate: f64,
    /// Reactor run time, **years** — see the module's unit note.
    pub run_time: f64,
    /// Irradiation time, **years** — see the module's unit note.
    pub irradiation_time: f64,
    /// Clean-up (HPS) rate constant, **s⁻¹**. Ignored when `hps_tog` is false;
    /// upstream `continue`s past it rather than reading it, so a run with the
    /// HPS off need not supply a meaningful value.
    #[serde(default)]
    pub k_clean: f64,
    /// Fuel kernel radius, **metres**.
    pub r_kernel: f64,
    /// SiC layer thickness, **metres**.
    #[serde(rename = "a_SiC")]
    pub a_sic: f64,
    /// Whether a helium purification (clean-up) system is fitted.
    pub hps_tog: bool,
    /// Whether to run the depressurisation accident after normal operation.
    pub accident_tog: bool,
    /// Number of radial rings.
    pub n_radial: usize,
    /// Number of axial nodes.
    pub n_axial: usize,
    /// Nuclide names, in the run file's own spelling.
    #[serde(rename = "Nuclides")]
    pub nuclides: Vec<String>,
    /// Per-nuclide inventories, curies.
    #[serde(rename = "Inventories")]
    pub inventories: Vec<f64>,
    /// Core temperature per node, °C, row-major `n_radial x n_axial`.
    #[serde(rename = "Core_Temps")]
    pub core_temps: Vec<f64>,
    /// Graphite temperature per node, °C, row-major `n_radial x n_axial`.
    #[serde(rename = "Graphite_Temps")]
    pub graphite_temps: Vec<f64>,
    /// Additional incremental failure fraction from the accident. Required
    /// only when `accident_tog`.
    #[serde(default)]
    pub f_inc_acc: f64,
    /// Additional incremental SiC-only failure from the accident. Accident only.
    #[serde(default)]
    pub f_inc_sic_acc: f64,
    /// Lift-off fraction — the share of plated-out activity re-entrained by the
    /// blowdown. Accident only, dimensionless.
    #[serde(default)]
    pub x_liftoff: f64,
    /// Accident transient sample times, seconds. Accident only.
    #[serde(default)]
    #[serde(rename = "Times")]
    pub times: Vec<f64>,
    /// Accident transient temperatures, °C. Accident only.
    #[serde(default)]
    #[serde(rename = "Accident_Temps")]
    pub accident_temps: Vec<f64>,
}

/// A validated, unit-carrying run configuration.
///
/// This is what upstream's positional `constants` array becomes once every
/// index has a name and a unit.
#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    /// Heavy-metal contamination fraction.
    pub f_hm: f64,
    /// As-manufactured SiC-defective fraction.
    pub f_sic: f64,
    /// Incremental in-service failure fraction.
    pub f_inc: f64,
    /// Incremental SiC-only failure fraction.
    pub f_inc_sic: f64,
    /// Matrix graphite thickness.
    pub graphite_thickness: Length,
    /// Fuel grain size.
    pub grain_size: Length,
    /// Plate-out rate constant.
    pub k_plate: Frequency,
    /// Reactor run time, converted from the file's years.
    pub run_time: Time,
    /// Irradiation time, converted from the file's years.
    pub irradiation_time: Time,
    /// Clean-up rate constant; exactly zero when no HPS is fitted.
    pub k_clean: Frequency,
    /// Fuel kernel radius.
    pub kernel_radius: Length,
    /// SiC layer thickness.
    pub sic_thickness: Length,
    /// Whether a clean-up system is fitted.
    pub hps: bool,
    /// Whether the accident case runs.
    pub accident: bool,
    /// Radial ring count.
    pub n_radial: usize,
    /// Axial node count.
    pub n_axial: usize,
    /// Nuclide names as supplied.
    pub nuclides: Vec<String>,
    /// Per-nuclide inventories, curies.
    pub inventories: Vec<f64>,
}

impl RunFile {
    /// Validate and convert to a [`RunConfig`].
    ///
    /// Ports `process_run_file` + `check_run_file`. Upstream logs each problem
    /// and increments a counter, then proceeds only if the count is zero; this
    /// collects every problem and returns them together, so a caller sees all
    /// of them in one pass rather than the first.
    ///
    /// # Checks applied
    /// - the six failure/lift-off fractions lie in `[0, 1]`;
    /// - the four lengths and two rate constants are non-negative;
    /// - `run_time` and `irradiation_time` are strictly positive;
    /// - `n_radial` and `n_axial` are at least 1;
    /// - `Nuclides` and `Inventories` have equal length;
    /// - both temperature tables hold exactly `n_radial * n_axial` entries;
    /// - when `accident_tog`, `Times` and `Accident_Temps` are non-empty and
    ///   `Accident_Temps` is a whole number of time samples.
    ///
    /// # Returns
    /// The config, or every [`RunFileError`] found.
    ///
    /// # Note on `k_clean`
    /// When `hps_tog` is false, upstream skips reading `k_clean` entirely.
    /// This forces it to exactly zero, so a stray non-zero value in the file
    /// cannot leak into a no-HPS run.
    pub fn to_config(&self) -> Result<RunConfig, Vec<RunFileError>> {
        let mut errs = Vec::new();

        let mut fraction = |key: &str, v: f64| {
            if !(0.0..=1.0).contains(&v) {
                errs.push(RunFileError::OutOfRange {
                    key: key.to_string(),
                    value: v,
                    expected: "a fraction in [0, 1]".to_string(),
                });
            }
        };
        fraction("f_hm", self.f_hm);
        fraction("f_sic", self.f_sic);
        fraction("f_inc", self.f_inc);
        fraction("f_inc_sic", self.f_inc_sic);
        if self.accident_tog {
            fraction("f_inc_acc", self.f_inc_acc);
            fraction("f_inc_sic_acc", self.f_inc_sic_acc);
            fraction("x_liftoff", self.x_liftoff);
        }

        let mut non_negative = |key: &str, v: f64| {
            if !(v >= 0.0) {
                errs.push(RunFileError::OutOfRange {
                    key: key.to_string(),
                    value: v,
                    expected: "a non-negative value".to_string(),
                });
            }
        };
        non_negative("a_graph", self.a_graph);
        non_negative("a_grain", self.a_grain);
        non_negative("r_kernel", self.r_kernel);
        non_negative("a_SiC", self.a_sic);
        non_negative("k_plate", self.k_plate);
        if self.hps_tog {
            non_negative("k_clean", self.k_clean);
        }

        let mut positive = |key: &str, v: f64| {
            if !(v > 0.0) {
                errs.push(RunFileError::OutOfRange {
                    key: key.to_string(),
                    value: v,
                    expected: "a strictly positive duration".to_string(),
                });
            }
        };
        positive("run_time", self.run_time);
        positive("irradiation_time", self.irradiation_time);

        if self.n_radial == 0 {
            errs.push(RunFileError::OutOfRange {
                key: "n_radial".to_string(),
                value: 0.0,
                expected: "at least 1".to_string(),
            });
        }
        if self.n_axial == 0 {
            errs.push(RunFileError::OutOfRange {
                key: "n_axial".to_string(),
                value: 0.0,
                expected: "at least 1".to_string(),
            });
        }

        if self.nuclides.len() != self.inventories.len() {
            errs.push(RunFileError::ShapeMismatch {
                key: "Inventories".to_string(),
                found: self.inventories.len(),
                expected: self.nuclides.len(),
            });
        }

        let nodes = self.n_radial * self.n_axial;
        for (key, table) in [
            ("Core_Temps", &self.core_temps),
            ("Graphite_Temps", &self.graphite_temps),
        ] {
            if table.len() != nodes {
                errs.push(RunFileError::ShapeMismatch {
                    key: key.to_string(),
                    found: table.len(),
                    expected: nodes,
                });
            }
        }

        if self.accident_tog {
            if self.times.is_empty() {
                errs.push(RunFileError::MissingKey {
                    key: "Times".to_string(),
                });
            }
            if self.accident_temps.is_empty() {
                errs.push(RunFileError::MissingKey {
                    key: "Accident_Temps".to_string(),
                });
            } else if !self.times.is_empty() && self.accident_temps.len() % self.times.len() != 0 {
                errs.push(RunFileError::ShapeMismatch {
                    key: "Accident_Temps".to_string(),
                    found: self.accident_temps.len(),
                    expected: self.times.len() * nodes,
                });
            }
        }

        if !errs.is_empty() {
            return Err(errs);
        }

        Ok(RunConfig {
            f_hm: self.f_hm,
            f_sic: self.f_sic,
            f_inc: self.f_inc,
            f_inc_sic: self.f_inc_sic,
            graphite_thickness: Length::new::<meter>(self.a_graph),
            grain_size: Length::new::<meter>(self.a_grain),
            k_plate: Frequency::new::<hertz>(self.k_plate),
            run_time: TimeUnit::Year.to_time(self.run_time),
            irradiation_time: TimeUnit::Year.to_time(self.irradiation_time),
            k_clean: Frequency::new::<hertz>(if self.hps_tog { self.k_clean } else { 0.0 }),
            kernel_radius: Length::new::<meter>(self.r_kernel),
            sic_thickness: Length::new::<meter>(self.a_sic),
            hps: self.hps_tog,
            accident: self.accident_tog,
            n_radial: self.n_radial,
            n_axial: self.n_axial,
            nuclides: self.nuclides.clone(),
            inventories: self.inventories.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> RunFile {
        RunFile {
            f_hm: 1e-4,
            f_sic: 1e-4,
            f_inc: 2.3e-5,
            f_inc_sic: 3.6e-5,
            a_graph: 0.0045,
            a_grain: 1e-5,
            k_plate: 7.5e-4,
            run_time: 40.0,
            irradiation_time: 3.0,
            k_clean: 8.77e-5,
            r_kernel: 2.13e-4,
            a_sic: 3.5e-5,
            hps_tog: true,
            accident_tog: false,
            n_radial: 2,
            n_axial: 3,
            nuclides: vec!["Cs-137".into(), "I-131".into()],
            inventories: vec![44.5, 12.0],
            core_temps: vec![900.0; 6],
            graphite_temps: vec![850.0; 6],
            f_inc_acc: 0.0,
            f_inc_sic_acc: 0.0,
            x_liftoff: 0.0,
            times: vec![],
            accident_temps: vec![],
        }
    }

    #[test]
    fn time_units_match_upstream_factors() {
        assert_eq!(TimeUnit::Second.seconds(), 1.0);
        assert_eq!(TimeUnit::Minute.seconds(), 60.0);
        assert_eq!(TimeUnit::Hour.seconds(), 3600.0);
        assert_eq!(TimeUnit::Day.seconds(), 86_400.0);
        // Upstream's 365-day year, NOT the 365.25-day Julian year.
        assert_eq!(TimeUnit::Year.seconds(), 31_536_000.0);
        assert!(TimeUnit::parse("fortnight").is_none());
        assert_eq!(TimeUnit::parse("yr"), Some(TimeUnit::Year));
    }

    #[test]
    fn a_good_run_file_converts_and_carries_years_into_seconds() {
        let cfg = minimal().to_config().expect("should validate");
        assert_eq!(cfg.run_time.get::<second>(), 40.0 * 31_536_000.0);
        assert_eq!(cfg.irradiation_time.get::<second>(), 3.0 * 31_536_000.0);
        assert_eq!(cfg.k_clean.get::<hertz>(), 8.77e-5);
    }

    #[test]
    fn k_clean_is_forced_to_zero_when_the_hps_is_off() {
        let mut f = minimal();
        f.hps_tog = false;
        f.k_clean = 999.0; // stray value that must not leak in
        let cfg = f.to_config().unwrap();
        assert_eq!(cfg.k_clean.get::<hertz>(), 0.0);
    }

    #[test]
    fn every_problem_is_reported_not_just_the_first() {
        let mut f = minimal();
        f.f_hm = 1.5; // out of range
        f.run_time = 0.0; // not positive
        f.core_temps = vec![900.0; 5]; // wrong shape
        let errs = f.to_config().unwrap_err();
        assert!(errs.len() >= 3, "expected at least 3 errors, got {errs:?}");
        assert!(errs
            .iter()
            .any(|e| matches!(e, RunFileError::ShapeMismatch { key, .. } if key == "Core_Temps")));
    }

    #[test]
    fn accident_keys_are_only_required_when_the_accident_runs() {
        let mut f = minimal();
        assert!(f.to_config().is_ok(), "no accident ⇒ Times may be empty");
        f.accident_tog = true;
        let errs = f.to_config().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, RunFileError::MissingKey { key } if key == "Times")));
    }

    #[test]
    fn round_trips_through_json() {
        let f = minimal();
        let json = serde_json::to_string(&f).expect("serialise");
        let back: RunFile = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(f, back);
    }
}
