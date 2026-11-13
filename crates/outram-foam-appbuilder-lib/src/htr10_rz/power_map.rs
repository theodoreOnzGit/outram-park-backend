// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! The FPIC 10 MW power-density map, TECDOC-1694 Tab 2.6 with the
//! Sec 2.1.1.6 revision, mapped onto an R-Z mesh by exact volume overlap.
//!
//! The data file `data/power-density-fpic-10mw-tab2-6.csv` is a copy of
//! `docs/reactor-scoping/htr10-th-data/power-density-fpic-10mw-tab2-6.csv`
//! (gh:#1147; the test `power_map_copy_matches_the_input_sheet` fails if they
//! drift). Its layout is the input sheet's: each value applies from its
//! `z_lo` to `z_hi` and `r_lo` to `r_hi` (cm), W/cm^3. Its volume integral
//! is 9.984 MW (input sheet Sec 5.1, Derived).
//!
//! **Mapping.** A mesh cell `[r1, r2] x [z1, z2]` receives
//! `sum_k q_k V(cell n map_k) / V(cell)`, with the axisymmetric overlap
//! volume `pi (b^2 - a^2) dz` for the radial overlap `[a, b]`. The integral
//! is therefore preserved exactly on any mesh (the map's last row ends at
//! `z = 187 cm`, inside the mesh interval 180-187.5).

/// The CSV, embedded.
const CSV: &str = include_str!("data/power-density-fpic-10mw-tab2-6.csv");

/// One map cell, cm and W/cm^3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerMapCell {
    /// `[z_lo, z_hi]`, cm (TECDOC `z`, downward).
    pub z_cm: [f64; 2],
    /// `[r_lo, r_hi]`, cm.
    pub r_cm: [f64; 2],
    /// Power density, W/cm^3.
    pub q_w_per_cm3: f64,
}

/// The parsed Tab 2.6 map (110 cells).
///
/// # Panics
/// Never for the embedded file (its parse is pinned by a test).
#[must_use]
pub fn fpic_power_map() -> Vec<PowerMapCell> {
    parse(CSV)
}

/// TECDOC-1694 Tab 2.56, the 3 MW map of the LOFC benchmark (gh:#1151
/// part B), a copy of the input sheet's
/// `power-density-3mw-lofc-tab2-56.csv` (pinned by a drift test). Same
/// layout as Tab 2.6; volume integral 2.991 MW (input sheet Sec 6,
/// Derived).
#[must_use]
pub fn lofc_power_map() -> Vec<PowerMapCell> {
    parse(CSV_LOFC)
}

/// TECDOC-1694 Tab 2.61, the 3 MW map of the CRW benchmark; differs from
/// Tab 2.56 in the one cell (z 180-187, r 83.5-90), input sheet N1. Volume
/// integral 2.999 MW (Derived).
#[must_use]
pub fn crw_power_map() -> Vec<PowerMapCell> {
    parse(CSV_CRW)
}

const CSV_LOFC: &str = include_str!("data/power-density-3mw-lofc-tab2-56.csv");
const CSV_CRW: &str = include_str!("data/power-density-3mw-crw-tab2-61.csv");

fn parse(csv: &str) -> Vec<PowerMapCell> {
    csv.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("z_lo") && !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.trim().parse::<f64>().expect("numeric CSV")).collect();
            PowerMapCell {
                z_cm: [v[0], v[1]],
                r_cm: [v[2], v[3]],
                q_w_per_cm3: v[4],
            }
        })
        .collect()
}

/// Volume integral of the map, W (full cylinder).
#[must_use]
pub fn map_power_w(map: &[PowerMapCell]) -> f64 {
    map.iter()
        .map(|c| {
            c.q_w_per_cm3
                * std::f64::consts::PI
                * (c.r_cm[1] * c.r_cm[1] - c.r_cm[0] * c.r_cm[0])
                * (c.z_cm[1] - c.z_cm[0])
        })
        .sum()
}

/// Power density, W/m^3, averaged over the axisymmetric cell
/// `[r1, r2] x [z1, z2]` (cm).
#[must_use]
pub fn cell_average_w_per_m3(map: &[PowerMapCell], r_cm: [f64; 2], z_cm: [f64; 2]) -> f64 {
    let vol = (r_cm[1] * r_cm[1] - r_cm[0] * r_cm[0]) * (z_cm[1] - z_cm[0]);
    if vol <= 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    for c in map {
        let a = r_cm[0].max(c.r_cm[0]);
        let b = r_cm[1].min(c.r_cm[1]);
        let lo = z_cm[0].max(c.z_cm[0]);
        let hi = z_cm[1].min(c.z_cm[1]);
        if b > a && hi > lo {
            sum += c.q_w_per_cm3 * (b * b - a * a) * (hi - lo);
        }
    }
    // W/cm^3 -> W/m^3.
    sum / vol * 1.0e6
}
