// SPDX-License-Identifier: GPL-3.0
// Ported from TRISO-ATOPS (INL), https://github.com/IdahoLabResearch/TRISO-ATOPS
// Source file      : trisoatops/utility_functions/run_functions.py
//                    (process_run_file, read_profile, check_run_file)
// Upstream commit  : de374c8
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// Ported under GPL-3.0; see LICENSE.triso-atops and NOTICE.triso-atops.

//! Reading an **upstream / GUI-written** TRISO-ATOPS run file (GitHub #449).
//!
//! [`super::RunFile`] is this port's own JSON shape: bare numbers, one
//! inventory per nuclide, and no CSV references. It **cannot** read a file
//! upstream's GUI writes. This module reads that format, following
//! `process_run_file`, `read_profile` and `check_run_file`:
//!
//! - **Constants** are `[value, unit]` pairs, and each unit is **checked**:
//!   `''` for fractions, `'m'` for lengths, `'s^-1'` for rate constants.
//!   `run_time` and `irradiation_time` take any `convert_time` unit (s, min,
//!   hr, d, yr) and are converted to seconds.
//! - **`k_clean`** is read only when `hps_tog` is true. Otherwise it stays 0,
//!   as upstream leaves `constants[9]`.
//! - **The accident constants** `f_inc_acc`, `f_inc_sic_acc` and `x_liftoff`
//!   are read only when `accident_tog` is true.
//! - **`Nuclides`** is an inline list, or a CSV path (first column, header row).
//! - **`Inventories`** is an inline `n_nuclides × n_radial` array (Ci), or a
//!   CSV path (header row, first column dropped).
//! - **`Core_Temps` / `Graphite_Temps`** go through `read_profile`:
//!   `[path, has_header, has_index]`, giving `n_axial` rows × `n_radial`
//!   columns.
//! - **`Times`** is `[entry, unit]`. The entry is tried first as a CSV path
//!   (header row, first column), then as a `read_profile` entry, and the
//!   result is multiplied by `convert_time(unit)`.
//! - **`Accident_Temps`** is one CSV path per ring (header row, first column
//!   dropped: `n_times × n_axial` each), or an inline
//!   `[ring][time][axial]` array.
//! - **`check_run_file`** checks the shapes, and that the **sum** of the
//!   failure fractions (six in accident mode, the first four otherwise) is not
//!   above 1. A sum of exactly 1 is counted as an error in accident mode (the
//!   silver warning upstream increments `error_count` for), and is only a
//!   warning otherwise.
//!
//! The constants keep **upstream's positional 15-slot layout**
//! (`constants[0..15]`, the `accident_case` indices), because that is the
//! contract the code-to-code fixture verifies.
//!
//! # Deliberate differences, stated
//! - **Relative paths** resolve against the **run file's directory**. Upstream
//!   resolves them against the process working directory. The code-to-code
//!   fixture runs upstream from the run file's directory, so the two agree.
//! - **CSV parsing is plain comma-separated numbers**, with an optional header
//!   row and an optional index column. Quoted fields and pandas' type
//!   inference are not reproduced.
//! - Upstream raises, rather than counting an error, for several malformed
//!   inputs (a missing required key, for example). Here **every** failure is
//!   an [`UpstreamRunFileError`].

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::TimeUnit;

/// Upstream's constant names, in `constants[]` order (`const_names` plus
/// `accident_constants`).
pub const CONSTANT_NAMES: [&str; 15] = [
    "f_hm",
    "f_sic",
    "f_inc",
    "f_inc_sic",
    "a_graph",
    "a_grain",
    "k_plate",
    "run_time",
    "irradiation_time",
    "k_clean",
    "r_kernel",
    "a_SiC",
    "f_inc_acc",
    "f_inc_sic_acc",
    "x_liftoff",
];

/// Upstream's `const_units`, parallel to [`CONSTANT_NAMES`].
const CONSTANT_UNITS: [&str; 15] = [
    "", "", "", "", "m", "m", "s^-1", "yr", "yr", "s^-1", "m", "m", "", "", "",
];

/// A run file as upstream's `process_run_file` returns it.
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamRunFile {
    /// `constants[0..15]` in SI (times in seconds), upstream's positional
    /// layout. Unread slots are 0, as upstream's `np.zeros`.
    pub constants: [f64; 15],
    /// `hps_tog`.
    pub hps: bool,
    /// `accident_tog`.
    pub accident: bool,
    /// `n_radial`.
    pub n_radial: usize,
    /// `n_axial`.
    pub n_axial: usize,
    /// Nuclide names, in file order.
    pub nuclides: Vec<String>,
    /// Inventories \[Ci\], `[nuclide][ring]`.
    pub inventories: Vec<Vec<f64>>,
    /// Normal-operation fuel temperatures, `[axial][ring]`, in the file's units.
    pub core_temperatures: Vec<Vec<f64>>,
    /// Normal-operation graphite temperatures, `[axial][ring]`.
    pub graphite_temperatures: Vec<Vec<f64>>,
    /// Accident times \[s\], when `accident_tog`.
    pub times: Option<Vec<f64>>,
    /// Accident temperatures, `[ring][time][axial]`, when `accident_tog`.
    pub accident_temperatures: Option<Vec<Vec<Vec<f64>>>>,
}

/// Why an upstream-format run file was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum UpstreamRunFileError {
    /// The file could not be read or is not JSON.
    Unreadable(String),
    /// A key upstream requires is absent (`required_keys`, `accident_keys`).
    MissingKey(String),
    /// A constant is not a `[number, unit]` pair.
    BadConstant(String),
    /// A constant's unit is not the one upstream accepts for it.
    WrongUnit {
        /// The constant.
        constant: String,
        /// The unit found.
        found: String,
    },
    /// A time unit `convert_time` does not know.
    UnknownTimeUnit(String),
    /// `n_radial` or `n_axial` is not an integer.
    NotAnInteger(String),
    /// A table or CSV could not be read as numbers.
    BadTable {
        /// Which key.
        key: String,
        /// Why.
        reason: String,
    },
    /// `check_run_file`: a shape disagrees with `n_radial` / `n_axial` / the
    /// time count.
    ShapeMismatch(String),
    /// `check_run_file`: the failure fractions sum above 1, or to exactly 1 in
    /// accident mode.
    FractionSum(f64),
}

/// Read and check an upstream-format run file.
///
/// # Errors
/// Every problem found, as upstream's `error_count` would count it (see the
/// module docs for where this is stricter).
pub fn read_upstream_run_file(path: &Path) -> Result<UpstreamRunFile, Vec<UpstreamRunFileError>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| vec![UpstreamRunFileError::Unreadable(e.to_string())])?;
    let json: Value = serde_json::from_str(&text)
        .map_err(|e| vec![UpstreamRunFileError::Unreadable(e.to_string())])?;
    let dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    parse(&json, &dir)
}

fn parse(json: &Value, dir: &Path) -> Result<UpstreamRunFile, Vec<UpstreamRunFileError>> {
    use UpstreamRunFileError as E;
    const REQUIRED: [&str; 19] = [
        "f_hm", "f_sic", "f_inc", "f_inc_sic", "a_graph", "a_grain", "k_plate", "run_time",
        "irradiation_time", "r_kernel", "a_SiC", "hps_tog", "accident_tog", "n_radial",
        "n_axial", "Nuclides", "Inventories", "Core_Temps", "Graphite_Temps",
    ];
    let missing: Vec<E> = REQUIRED
        .iter()
        .filter(|k| json.get(**k).is_none())
        .map(|k| E::MissingKey((*k).to_string()))
        .collect();
    if !missing.is_empty() {
        return Err(missing);
    }
    let hps = json["hps_tog"] == Value::Bool(true);
    let accident = json["accident_tog"] == Value::Bool(true);
    let mut errors = Vec::new();

    // -- constants
    let mut constants = [0.0_f64; 15];
    let n_const = if accident { 15 } else { 12 };
    for i in 0..n_const {
        let name = CONSTANT_NAMES[i];
        if name == "k_clean" && !hps {
            continue;
        }
        let Some(entry) = json.get(name) else {
            errors.push(E::MissingKey(name.to_string()));
            continue;
        };
        let (Some(value), Some(unit)) = (
            entry.get(0).and_then(Value::as_f64),
            entry.get(1).and_then(Value::as_str),
        ) else {
            errors.push(E::BadConstant(name.to_string()));
            continue;
        };
        if name == "run_time" || name == "irradiation_time" {
            match TimeUnit::parse(unit) {
                Some(u) => constants[i] = value * u.seconds(),
                None => errors.push(E::UnknownTimeUnit(unit.to_string())),
            }
        } else if unit == CONSTANT_UNITS[i] {
            constants[i] = value;
        } else {
            errors.push(E::WrongUnit { constant: name.to_string(), found: unit.to_string() });
        }
    }

    // -- configuration
    let as_int = |k: &str| -> Option<usize> {
        let v = &json[k];
        v.as_u64()
            .map(|n| n as usize)
            .or_else(|| v.as_f64().filter(|x| x.fract() == 0.0 && *x >= 0.0).map(|x| x as usize))
    };
    let n_radial = as_int("n_radial").unwrap_or_else(|| {
        errors.push(E::NotAnInteger("n_radial".into()));
        0
    });
    let n_axial = as_int("n_axial").unwrap_or_else(|| {
        errors.push(E::NotAnInteger("n_axial".into()));
        0
    });

    // -- nuclides, inventories, profiles
    let nuclides = match &json["Nuclides"] {
        Value::Array(a) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).or_else(|| {
                v.get(0).and_then(Value::as_str).map(str::to_string)
            }))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| E::BadTable { key: "Nuclides".into(), reason: "not strings".into() }),
        Value::String(p) => read_csv_strings(&dir.join(p)).map_err(|r| E::BadTable {
            key: "Nuclides".into(),
            reason: r,
        }),
        _ => Err(E::BadTable { key: "Nuclides".into(), reason: "not a list or a path".into() }),
    };
    let inventories = match &json["Inventories"] {
        Value::String(p) => read_csv(&dir.join(p), true, true),
        v => inline_2d(v),
    }
    .map_err(|r| E::BadTable { key: "Inventories".into(), reason: r });
    let core = read_profile(&json["Core_Temps"], dir)
        .map_err(|r| E::BadTable { key: "Core_Temps".into(), reason: r });
    let graphite = read_profile(&json["Graphite_Temps"], dir)
        .map_err(|r| E::BadTable { key: "Graphite_Temps".into(), reason: r });

    // -- accident
    let (mut times, mut accident_temperatures) = (None, None);
    if accident {
        for k in ["f_inc_acc", "f_inc_sic_acc", "x_liftoff", "Times", "Accident_Temps"] {
            if json.get(k).is_none() {
                errors.push(E::MissingKey(k.to_string()));
            }
        }
        if let Some(t) = json.get("Times") {
            match (t.get(0), t.get(1).and_then(Value::as_str)) {
                (Some(entry), Some(unit)) => match TimeUnit::parse(unit) {
                    Some(u) => {
                        let raw = match entry {
                            Value::String(p) => {
                                read_csv(&dir.join(p), true, false).map(|t| column(&t, 0))
                            }
                            e => read_profile(e, dir).map(|t| t.into_iter().flatten().collect()),
                        };
                        match raw {
                            Ok(v) => times = Some(v.iter().map(|x| x * u.seconds()).collect()),
                            Err(r) => errors.push(E::BadTable { key: "Times".into(), reason: r }),
                        }
                    }
                    None => errors.push(E::UnknownTimeUnit(unit.to_string())),
                },
                _ => errors.push(E::BadTable { key: "Times".into(), reason: "not [entry, unit]".into() }),
            }
        }
        if let Some(a) = json.get("Accident_Temps") {
            let per_ring: Result<Vec<Vec<Vec<f64>>>, String> = match a {
                Value::Array(rings) if rings.iter().all(Value::is_string) => rings
                    .iter()
                    .map(|p| read_csv(&dir.join(p.as_str().unwrap_or_default()), true, true))
                    .collect(),
                v => inline_3d(v),
            };
            match per_ring {
                Ok(t) => accident_temperatures = Some(t),
                Err(r) => errors.push(E::BadTable { key: "Accident_Temps".into(), reason: r }),
            }
        }
    }

    let (nuclides, inventories, core, graphite) = match (nuclides, inventories, core, graphite) {
        (Ok(n), Ok(i), Ok(c), Ok(g)) => (n, i, c, g),
        (n, i, c, g) => {
            errors.extend([n.err(), i.err(), c.err(), g.err()].into_iter().flatten());
            return Err(errors);
        }
    };
    if !errors.is_empty() {
        return Err(errors);
    }

    // -- check_run_file
    let rows = |t: &Vec<Vec<f64>>| t.len();
    let cols = |t: &Vec<Vec<f64>>| t.first().map_or(0, Vec::len);
    let mut shape = |ok: bool, what: String| {
        if !ok {
            errors.push(E::ShapeMismatch(what));
        }
    };
    shape(rows(&inventories) == nuclides.len(), format!("Inventories rows {} vs {} nuclides", rows(&inventories), nuclides.len()));
    shape(cols(&inventories) == n_radial, format!("Inventories columns {} vs n_radial {n_radial}", cols(&inventories)));
    for (key, t) in [("Core_Temps", &core), ("Graphite_Temps", &graphite)] {
        shape(rows(t) == n_axial, format!("{key} rows {} vs n_axial {n_axial}", rows(t)));
        shape(cols(t) == n_radial, format!("{key} columns {} vs n_radial {n_radial}", cols(t)));
    }
    let sum4: f64 = constants[0..4].iter().sum();
    if accident {
        let acc = accident_temperatures.as_ref().expect("read above");
        let n_t = times.as_ref().map_or(0, Vec::len);
        shape(acc.len() == n_radial, format!("Accident_Temps rings {} vs n_radial {n_radial}", acc.len()));
        shape(
            acc.first().and_then(|r| r.first()).map_or(0, Vec::len) == n_axial,
            "Accident_Temps axial count vs n_axial".to_string(),
        );
        shape(acc.first().map_or(0, Vec::len) == n_t, format!("Accident_Temps time count vs {n_t} times"));
        let sum = sum4 + constants[12] + constants[13];
        if sum >= 1.0 {
            errors.push(E::FractionSum(sum));
        }
    } else if sum4 > 1.0 {
        errors.push(E::FractionSum(sum4));
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(UpstreamRunFile {
        constants,
        hps,
        accident,
        n_radial,
        n_axial,
        nuclides,
        inventories,
        core_temperatures: core,
        graphite_temperatures: graphite,
        times,
        accident_temperatures,
    })
}

/// `read_profile`: `[path, has_header, has_index]`, 2-D.
fn read_profile(entry: &Value, dir: &Path) -> Result<Vec<Vec<f64>>, String> {
    let path = entry.get(0).and_then(Value::as_str).ok_or("profile entry is not [path, header, index]")?;
    let header = entry.get(1) == Some(&Value::Bool(true));
    let index = entry.get(2) == Some(&Value::Bool(true));
    read_csv(&dir.join(path), header, index)
}

/// A plain numeric CSV, optionally skipping a header row and dropping the
/// first (index) column.
fn read_csv(path: &PathBuf, header: bool, drop_index: bool) -> Result<Vec<Vec<f64>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .skip(usize::from(header))
        .map(|line| {
            line.split(',')
                .skip(usize::from(drop_index))
                .map(|c| c.trim().parse::<f64>().map_err(|_| format!("non-float value {c:?}")))
                .collect()
        })
        .collect()
}

/// First column of a CSV with a header row, as strings (`Nuclides` path form).
fn read_csv_strings(path: &PathBuf) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .skip(1)
        .map(|l| l.split(',').next().unwrap_or_default().trim().to_string())
        .collect())
}

fn column(table: &[Vec<f64>], c: usize) -> Vec<f64> {
    table.iter().filter_map(|r| r.get(c).copied()).collect()
}

fn inline_2d(v: &Value) -> Result<Vec<Vec<f64>>, String> {
    v.as_array()
        .ok_or("not an array")?
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or("not a 2-D array")?
                .iter()
                .map(|x| x.as_f64().ok_or_else(|| "non-float value".to_string()))
                .collect()
        })
        .collect()
}

fn inline_3d(v: &Value) -> Result<Vec<Vec<Vec<f64>>>, String> {
    v.as_array().ok_or("not an array")?.iter().map(inline_2d).collect()
}
