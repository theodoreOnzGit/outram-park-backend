// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Embedded HTR-10 input and validation tables (gh:#1151).
//!
//! Each file under `data/` is a byte-for-byte copy of the file of the same
//! name in `docs/reactor-scoping/htr10-th-data/`, which carries the citation
//! header, the digitisation method and the read error. The test
//! `data_copies_match_the_input_sheet` fails if a copy drifts from the sheet.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use crate::TampinesError;

/// TECDOC-1694 Tab 2.56, the 3 MW LOFC power-density map (cell edges, cm;
/// W/cm^3).
pub const POWER_MAP_TAB2_56: &str = include_str!("data/power-density-3mw-lofc-tab2-56.csv");
/// Chen et al. (2009) Fig 4, measured relative blower speed after the trip.
pub const BLOWER_SPEED_C09_FIG4: &str = include_str!("data/lofc-blower-speed-c09-fig4.csv");
/// Jun et al. (2009) Fig 6, the GAMMA+ input flow coastdown (a TUNED input).
pub const FLOW_COASTDOWN_J09_FIG6: &str = include_str!("data/lofc-flow-coastdown-j09-fig6.csv");

/// The numeric rows of a citation-headed CSV: `#` lines and the one header
/// line (the first non-`#` line) are skipped; every remaining field must
/// parse as `f64`.
///
/// # Errors
/// [`TampinesError::InvalidInput`] naming the line that does not parse.
pub fn numeric_rows(csv: &str) -> Result<Vec<Vec<f64>>, TampinesError> {
    let mut rows = Vec::new();
    let mut header_seen = false;
    for (n, line) in csv.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !header_seen {
            header_seen = true;
            continue;
        }
        let row: Result<Vec<f64>, _> = line.split(',').map(|v| v.trim().parse::<f64>()).collect();
        rows.push(row.map_err(|e| {
            TampinesError::InvalidInput(format!("data line {}: {e}: {line}", n + 1))
        })?);
    }
    Ok(rows)
}

/// A piecewise-linear table `y(x)`, held constant outside its first and
/// last abscissa. The abscissae must be strictly increasing.
#[derive(Debug, Clone, PartialEq)]
pub struct TabulatedSeries {
    points: Vec<(f64, f64)>,
}

impl TabulatedSeries {
    /// Build from `(x, y)` pairs.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] if empty, non-finite or not strictly
    /// increasing in `x`.
    pub fn new(points: Vec<(f64, f64)>) -> Result<Self, TampinesError> {
        if points.is_empty() {
            return Err(TampinesError::InvalidInput("empty table".into()));
        }
        for w in points.windows(2) {
            if !(w[1].0 > w[0].0) {
                return Err(TampinesError::InvalidInput(format!(
                    "table abscissae not strictly increasing at {} -> {}",
                    w[0].0, w[1].0
                )));
            }
        }
        if points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
            return Err(TampinesError::InvalidInput("non-finite table entry".into()));
        }
        Ok(Self { points })
    }

    /// Two columns of a citation-headed CSV as a table.
    ///
    /// # Errors
    /// As [`numeric_rows`] and [`Self::new`], and if a column is missing.
    pub fn from_csv_columns(csv: &str, x_col: usize, y_col: usize) -> Result<Self, TampinesError> {
        let rows = numeric_rows(csv)?;
        let mut pts = Vec::with_capacity(rows.len());
        for r in rows {
            let (Some(x), Some(y)) = (r.get(x_col), r.get(y_col)) else {
                return Err(TampinesError::InvalidInput("missing CSV column".into()));
            };
            pts.push((*x, *y));
        }
        Self::new(pts)
    }

    /// The points.
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// Linear interpolation at `x`, held at the end values outside.
    pub fn value_at(&self, x: f64) -> f64 {
        let p = &self.points;
        if x <= p[0].0 {
            return p[0].1;
        }
        let last = p[p.len() - 1];
        if x >= last.0 {
            return last.1;
        }
        let i = p.partition_point(|q| q.0 <= x);
        let (x0, y0) = p[i - 1];
        let (x1, y1) = p[i];
        y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    }
}

/// One cell of a TECDOC power map: `z` and `r` edges in cm, `q` in W/cm^3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerMapCell {
    /// `[z_lo, z_hi]`, cm, TECDOC `z` (downward from the bed top).
    pub z_cm: [f64; 2],
    /// `[r_lo, r_hi]`, cm.
    pub r_cm: [f64; 2],
    /// Power density, W/cm^3.
    pub q_w_per_cm3: f64,
}

/// The Tab 2.56 map as cells.
///
/// # Errors
/// As [`numeric_rows`]; and if a row does not have five fields.
pub fn lofc_power_map() -> Result<Vec<PowerMapCell>, TampinesError> {
    numeric_rows(POWER_MAP_TAB2_56)?
        .into_iter()
        .map(|r| {
            if r.len() != 5 {
                return Err(TampinesError::InvalidInput("power map row needs 5 fields".into()));
            }
            Ok(PowerMapCell {
                z_cm: [r[0], r[1]],
                r_cm: [r[2], r[3]],
                q_w_per_cm3: r[4],
            })
        })
        .collect()
}

/// Power (W) of the map inside the axial band `[z_lo, z_hi]` cm, all radii,
/// by exact overlap of each cell with the band.
pub fn map_power_in_band_w(map: &[PowerMapCell], z_lo: f64, z_hi: f64) -> f64 {
    map.iter()
        .map(|c| {
            let lo = c.z_cm[0].max(z_lo);
            let hi = c.z_cm[1].min(z_hi);
            if hi <= lo {
                return 0.0;
            }
            c.q_w_per_cm3
                * std::f64::consts::PI
                * (c.r_cm[1] * c.r_cm[1] - c.r_cm[0] * c.r_cm[0])
                * (hi - lo)
        })
        .sum()
}
