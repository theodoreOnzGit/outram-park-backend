// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # HTR-10: whole-core Monte Carlo iterated against the R-Z thermal-hydraulics
//!
//! gh:#1186. Read top to bottom:
//!
//! 1. the Monte Carlo model is a `ParametricStudy` (the web demo's), at a
//!    starting guess of the eight zone temperatures;
//! 2. the TH target is the TECDOC-1694 R-Z core at FPIC (10 MW, 250 °C,
//!    3 MPa, 4.32 kg/s), or GeN-Foam's lumped node;
//! 3. `Htr10DirectCoupling::solve` runs the Picard loop: Monte Carlo with an
//!    R-Z power tally, the tally put on the TH cells, the TH steady state,
//!    the zone temperatures back, under-relaxed, the nuclear data reprocessed
//!    exactly at them (cached);
//! 4. the history is written as a CSV and a PNG.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_direct_coupling
//! ```
//!
//! Knobs (environment, all optional): `OUTRAM_COUPLING_LAYERS` (Şeker N,
//! default 17: 172.6 cm, the closest to the R-Z model's 180 cm bed),
//! `OUTRAM_COUPLING_PARTICLES` (1000), `OUTRAM_COUPLING_INACTIVE` (10),
//! `OUTRAM_COUPLING_ACTIVE` (20), `OUTRAM_COUPLING_ITERATIONS` (4),
//! `OUTRAM_COUPLING_THREADS` (4), `OUTRAM_COUPLING_TARGET` (`rz` or
//! `lumped`).
//!
//! Output: `crates/nee_soon/verification_and_validation/htr10_direct_coupling/`
//! (`history.csv`, `images/history.png`, `images/power_shape_rz.png`).
//! Its `README.md` records what a run cost and gave.

use nee_soon::direct_coupling::Convergence;
use nee_soon::htr10_coupling::{write_history_png, Htr10CouplingError, Htr10DirectCoupling, PicardIteration, ThermalTarget};
use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed};
use nee_soon::htr10_rmc::zones::{Htr10Zone, ZoneTemperatures};
use outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::LumpedThermal;
use uom::si::f64::{HeatCapacity, Length, Power, Ratio, TemperatureInterval, ThermalConductance, ThermodynamicTemperature, Time, Volume};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::length::centimeter;
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::temperature_interval;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

fn knob(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() -> Result<(), String> {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/htr10_direct_coupling");
    std::fs::create_dir_all(out.join("images")).map_err(|e| e.to_string())?;
    let layers = knob("OUTRAM_COUPLING_LAYERS", 17);
    let threads = knob("OUTRAM_COUPLING_THREADS", 4);

    // 1. The Monte Carlo model at a starting guess (K).
    let mut study = ParametricStudy { bed: StudyBed::Seker { layers }, ..ParametricStudy::default() };
    study.statistics.particles = knob("OUTRAM_COUPLING_PARTICLES", 1000);
    study.statistics.inactive = knob("OUTRAM_COUPLING_INACTIVE", 10);
    study.statistics.active = knob("OUTRAM_COUPLING_ACTIVE", 20);
    study.temperatures = ZoneTemperatures::from_kelvin([900.0, 850.0, 700.0, 600.0, 650.0, 700.0, 600.0, 550.0]);

    // 2. The thermal-hydraulic target.
    let target = match std::env::var("OUTRAM_COUPLING_TARGET").as_deref() {
        Ok("lumped") => ThermalTarget::Lumped {
            // 10 MW over 1000 K: a node that settles at 500 K + P/G = 1000 K.
            thermal: LumpedThermal::new(
                "core",
                HeatCapacity::new::<joule_per_kelvin>(1.0e7),
                ThermalConductance::new::<watt_per_kelvin>(2.0e4),
                ThermodynamicTemperature::new::<kelvin>(900.0),
                ThermodynamicTemperature::new::<kelvin>(500.0),
                5.0,
            ),
            volume: Volume::new::<cubic_meter>(5.0),
            time_step: Time::new::<second>(100.0),
        },
        _ => ThermalTarget::htr10_rz_fpic().map_err(|e| e.to_string())?,
    };

    // 3. The coupling.
    let mut coupling = Htr10DirectCoupling::new(study, target, Power::new::<megawatt>(10.0))
        .with_relaxation(Ratio::new::<ratio>(0.7))
        .with_threads(threads);
    let criteria = Convergence::new(
        TemperatureInterval::new::<temperature_interval::kelvin>(10.0),
        // Above the Monte Carlo sigma at these statistics (~0.003).
        Ratio::new::<ratio>(5.0e-3),
        knob("OUTRAM_COUPLING_ITERATIONS", 4),
    );
    let t0 = std::time::Instant::now();
    let (history, shape) = match coupling.solve(criteria, |l| println!("{l}")) {
        Ok(s) => {
            println!("CONVERGED: k = {:.5} +/- {:.5}", s.k_eff.get::<ratio>(), s.k_std.get::<ratio>());
            (s.history, Some(s.power_shape))
        }
        Err(Htr10CouplingError::NotConverged { history, last_dt_k, last_dk, .. }) => {
            println!("NOT CONVERGED in the cap: last max |dT| {last_dt_k:.2} K, |dk| {last_dk:.5}");
            (history, None)
        }
        Err(e) => return Err(e.to_string()),
    };
    println!("total {:.0} s, {} jobs cached", t0.elapsed().as_secs_f64(), coupling.cached_jobs());

    // 4. The record.
    let mut csv = String::from("iteration,k,k_sigma,max_dT_K,peaking,majorant_violations,jobs_processed,jobs_cached,data_s,mc_s,th_s");
    for z in Htr10Zone::ALL {
        csv.push_str(&format!(",{}_in_K,{}_th_K", z.key(), z.key()));
    }
    csv.push('\n');
    for h in &history {
        csv.push_str(&row(h));
    }
    std::fs::write(out.join("history.csv"), csv).map_err(|e| e.to_string())?;
    write_history_png(&history, out.join("images/history.png")).map_err(|e| e.to_string())?;
    if let Some(s) = shape {
        let l = Length::new::<centimeter>;
        s.write_rz_png(out.join("images/power_shape_rz.png"), "MEASURED MC POWER SHAPE (R-Z, MC FRAME)", l(90.0), [l(-90.0), l(90.0)], 2.0)
            .map_err(|e| e.to_string())?;
    }
    println!("wrote {}", out.display());
    Ok(())
}

fn row(h: &PicardIteration) -> String {
    let mut s = format!(
        "{},{:.6},{:.6},{:.3},{:.4},{},{},{},{:.1},{:.1},{:.2}",
        h.iteration,
        h.k_eff.get::<ratio>(),
        h.k_std.get::<ratio>(),
        h.temperature_residual.get::<temperature_interval::kelvin>(),
        h.peaking.get::<ratio>(),
        h.majorant_violations,
        h.jobs_processed,
        h.jobs_cached,
        h.processing_time.get::<second>(),
        h.transport_time.get::<second>(),
        h.thermal_time.get::<second>()
    );
    for z in Htr10Zone::ALL {
        s.push_str(&format!(",{:.2},{:.2}", h.zone_temperatures_in.kelvin(z), h.zone_temperatures_th.kelvin(z)));
    }
    s.push('\n');
    s
}
