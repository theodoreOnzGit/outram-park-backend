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

//! # Per-point residuals of the experimental Chebyshev backward correlations
//!
//! Writes the **per-grid-point** round-trip residuals of the three experimental
//! Chebyshev backward correlations to CSV, so they can be plotted as residual
//! maps rather than quoted as summary statistics.
//!
//! ```bash
//! cargo run --release -p tampines-steam-tables --example chebyshev_residual_export
//! cargo run --release -p tampines-steam-tables --example chebyshev_residual_export -- --out-dir <DIR>
//! ```
//!
//! ## Why this exists
//!
//! `verification_and_validation/generated/region_5_backward_t_ph_t_ps.md` and
//! `region_4_near_critical_p_hs.md` report **max and RMS** (and percentiles)
//! over their sweeps. Those are the right summary for a V&V gate, but a figure
//! showing *where in the domain* the error lives needs the grid itself, and
//! reducing a 3600-point sweep to two numbers throws that away.
//!
//! This example sweeps the **same grids, through the same public functions**
//! the tests use, and emits every point. It computes nothing the tests do not;
//! it only declines to reduce.
//!
//! ## Provenance
//!
//! - Region 5 grid: 60 x 60, pressure log-spaced `1e-4` to `50` MPa,
//!   temperature linear `1073.15` to `2273.15` K — the fit domain, matching
//!   `tests/region_5_t_ph_ps.rs`.
//! - Region 4 grid: 40 saturation temperatures linear over `623.15` to
//!   `647.04` K, 20 qualities over `0.05` to `0.95` (endpoints avoided, where
//!   the two-phase flash is ill-conditioned) — matching
//!   `tests/region_4_near_critical_hs.rs`.
//!
//! Deterministic: no RNG, no clock, no network. Re-running reproduces the CSVs
//! byte-for-byte.
//!
//! ## What these numbers are NOT
//!
//! These are **experimental, non-IAPWS** correlations (GitHub issue #34).
//! IAPWS-IF97 publishes no backward equations for Region 5, so the reference is
//! this crate's **own forward equations**, not a published backward-equation
//! reference value. Per `RESPONSIBLE_USE.md` this is AI-assisted draft material
//! and **no human has reviewed it**; the residuals are measurements, not a
//! validation sign-off.

use std::fmt::Write as _;

use uom::si::{
    available_energy::kilojoule_per_kilogram, f64::*, pressure::megapascal,
    specific_heat_capacity::kilojoule_per_kilogram_kelvin, thermodynamic_temperature::kelvin,
};

use tampines_steam_tables::backward_eqn_chebyshev_experimental::{
    h_f_near_critical_explicit, h_g_near_critical_explicit, p_hs_4_near_critical_explicit, t_ph_5,
    t_ps_5,
};
use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    s_ph_eqm, x_ph_flash,
};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;
use tampines_steam_tables::region_5_steam_at_800_plus_degc::{h_tp_5, s_tp_5};

/// Region 5 fit domain — matches `tests/region_5_t_ph_ps.rs`.
const P_MIN_MPA: f64 = 1.0e-4;
const P_MAX_MPA: f64 = 50.0;
const T_MIN_K: f64 = 1073.15;
const T_MAX_K: f64 = 2273.15;
const REGION_5_N: usize = 60;

/// Region 4 near-critical band — matches `tests/region_4_near_critical_hs.rs`.
const T_LO_K: f64 = 623.15;
const T_HI_K: f64 = 647.04;
const REGION_4_N_T: usize = 40;
const REGION_4_N_X: usize = 20;

/// Region 5: recover `T` from `(p,h)` and `(p,s)` and record the deviation
/// from the temperature the state was generated at.
fn region_5_residuals() -> String {
    let mut csv = String::from("p_mpa,t_k,h_kj_kg,s_kj_kg_k,t_ph_k,t_ps_k,dt_ph_k,dt_ps_k\n");
    let (log_p_min, log_p_max) = (P_MIN_MPA.log10(), P_MAX_MPA.log10());

    for i in 0..REGION_5_N {
        let frac_p = i as f64 / (REGION_5_N - 1) as f64;
        let p_mpa = 10.0_f64.powf(log_p_min + frac_p * (log_p_max - log_p_min));
        let p = Pressure::new::<megapascal>(p_mpa);

        for j in 0..REGION_5_N {
            let frac_t = j as f64 / (REGION_5_N - 1) as f64;
            let t_k = T_MIN_K + frac_t * (T_MAX_K - T_MIN_K);
            let t = ThermodynamicTemperature::new::<kelvin>(t_k);

            // Forward equations supply the reference state.
            let h = h_tp_5(t, p);
            let s = s_tp_5(t, p);

            let t_ph_k = t_ph_5(p, h).get::<kelvin>();
            let t_ps_k = t_ps_5(p, s).get::<kelvin>();

            let _ = writeln!(
                csv,
                "{p_mpa:.10e},{t_k:.6},{:.8},{:.8},{t_ph_k:.6},{t_ps_k:.6},{:.6e},{:.6e}",
                h.get::<kilojoule_per_kilogram>(),
                s.get::<kilojoule_per_kilogram_kelvin>(),
                t_ph_k - t_k,
                t_ps_k - t_k,
            );
        }
    }
    csv
}

/// Region 4 near-critical: recover the saturation pressure from `(h,s)` and
/// record the relative error against the IAPWS reference `p_sat(T)`.
fn region_4_residuals() -> String {
    let mut csv =
        String::from("t_sat_k,quality,h_kj_kg,s_kj_kg_k,p_ref_mpa,p_cheb_mpa,rel_err\n");

    for i in 0..REGION_4_N_T {
        let frac_t = i as f64 / (REGION_4_N_T - 1) as f64;
        let t_sat_k = T_LO_K + frac_t * (T_HI_K - T_LO_K);
        let t_sat = ThermodynamicTemperature::new::<kelvin>(t_sat_k);

        let p_reference = sat_pressure_4(t_sat);
        let p_reference_mpa = p_reference.get::<megapascal>();
        if !p_reference_mpa.is_finite() || p_reference_mpa <= 0.0 {
            continue;
        }

        let h_f = h_f_near_critical_explicit(t_sat_k);
        let h_g = h_g_near_critical_explicit(t_sat_k);

        for j in 0..REGION_4_N_X {
            // Endpoints avoided: the two-phase flash is ill-conditioned there.
            let quality = 0.05 + 0.90 * (j as f64 / (REGION_4_N_X - 1) as f64);
            let h_kj_kg = h_f + quality * (h_g - h_f);
            let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj_kg);

            // Confirm the state really is two-phase before using it.
            let x_check = x_ph_flash(p_reference, h);
            if !x_check.is_finite() || !(0.0..=1.0).contains(&x_check) {
                continue;
            }
            let s_kj_kg_k = s_ph_eqm(p_reference, h).get::<kilojoule_per_kilogram_kelvin>();
            if !s_kj_kg_k.is_finite() {
                continue;
            }

            let p_fitted_mpa = p_hs_4_near_critical_explicit(h_kj_kg, s_kj_kg_k);
            let rel_err = (p_fitted_mpa - p_reference_mpa) / p_reference_mpa;

            let _ = writeln!(
                csv,
                "{t_sat_k:.6},{quality:.6},{h_kj_kg:.8},{s_kj_kg_k:.8},\
                 {p_reference_mpa:.10e},{p_fitted_mpa:.10e},{rel_err:.6e}"
            );
        }
    }
    csv
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args
        .iter()
        .position(|a| a == "--out-dir")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "{}/verification_and_validation/generated",
                env!("CARGO_MANIFEST_DIR")
            )
        });
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    for (name, body) in [
        ("region_5_backward_residuals.csv", region_5_residuals()),
        ("region_4_near_critical_residuals.csv", region_4_residuals()),
    ] {
        let path = format!("{out_dir}/{name}");
        std::fs::write(&path, &body).expect("write CSV");
        println!("wrote {path}  ({} rows)", body.lines().count() - 1);
    }
}
