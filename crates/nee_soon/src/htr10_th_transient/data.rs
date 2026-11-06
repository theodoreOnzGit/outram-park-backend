// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Embedded validation tables of the HTR-10 coupled transients (gh:#1151),
//! moved from `tampines::htr10_primary_loop::data` on 2026-11-03.
//!
//! Each file under `data/` is a byte-for-byte copy of the file of the same
//! name in `docs/reactor-scoping/htr10-th-data/`, which carries the citation
//! header, the digitisation method and the read error. The test
//! `data_copies_match_the_input_sheet` fails if a copy drifts. The loop's own
//! tables (power map, coastdown) stay in `tampines`.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

/// Chen et al. (2009) Fig 6, measured relative fission power, 0-10800 s.
pub const POWER_LONG_C09_FIG6: &str = include_str!("data/lofc-power-long-c09-fig6.csv");
/// Chen et al. (2009) Fig 4, measured relative fission power, 0-500 s.
pub const POWER_SHORT_C09_FIG4: &str = include_str!("data/lofc-power-short-c09-fig4.csv");
/// TECDOC-1694 Tab 2.60, CRW inserted reactivity (mk) against s, 0-128 s.
pub const CRW_REACTIVITY_TAB2_60: &str = include_str!("data/crw-reactivity-tab2-60.csv");
/// Chen et al. (2009) Figs 8-11, the 17 measured internals thermocouple
/// traces of the LOFC test (180 s bins).
pub const LOFC_THERMOCOUPLES_C09: &str = include_str!("data/lofc-thermocouples-c09-figs8-11.csv");

use tampines::TampinesError;

/// One measured thermocouple trace of [`LOFC_THERMOCOUPLES_C09`]: its label
/// (`SR6`, `TR1_TR4`, ...) and `(t s, T deg C, read band deg C)` rows.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredTrace {
    /// Label as in [C09].
    pub label: String,
    /// `(t, T, band)`.
    pub rows: Vec<(f64, f64, f64)>,
}

impl MeasuredTrace {
    /// Linear interpolation in `T` (held at the ends).
    pub fn value_at(&self, t: f64) -> f64 {
        let r = &self.rows;
        if t <= r[0].0 {
            return r[0].1;
        }
        for w in r.windows(2) {
            if t <= w[1].0 {
                return w[0].1 + (w[1].1 - w[0].1) * (t - w[0].0) / (w[1].0 - w[0].0);
            }
        }
        r[r.len() - 1].1
    }
}

/// The 17 traces of [`LOFC_THERMOCOUPLES_C09`], in file order.
///
/// # Errors
/// [`TampinesError::InvalidInput`] for a row that does not parse.
pub fn lofc_thermocouple_traces() -> Result<Vec<MeasuredTrace>, TampinesError> {
    let mut out: Vec<MeasuredTrace> = Vec::new();
    for line in LOFC_THERMOCOUPLES_C09.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("tc,") {
            continue;
        }
        let v: Vec<&str> = line.split(',').collect();
        let num = |s: &str| {
            s.trim()
                .parse::<f64>()
                .map_err(|e| TampinesError::InvalidInput(format!("thermocouple row {line}: {e}")))
        };
        if v.len() != 5 {
            return Err(TampinesError::InvalidInput(format!("thermocouple row {line}: 5 fields expected")));
        }
        let row = (num(v[2])?, num(v[3])?, num(v[4])?);
        match out.iter_mut().find(|m| m.label == v[0]) {
            Some(m) => m.rows.push(row),
            None => out.push(MeasuredTrace { label: v[0].to_string(), rows: vec![row] }),
        }
    }
    Ok(out)
}
