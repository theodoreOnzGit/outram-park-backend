// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **Which graphite carries the HTR-10 moderator temperature coefficient**
//! (gh:#1174, epic gh:#1146): the whole-core Monte Carlo model with every
//! zone at one base temperature, then one graphite part hotter at a time
//! (`nee_soon::htr10_rmc::parametric::graphite_worth_cases`): the fuel
//! pebbles' graphite (matrix and shell), the dummy pebbles, the top, side
//! and bottom reflectors, and the kernels with their coatings (the fuel
//! coefficient's own part, a check).
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_graphite_zone_worth -- \
//!     --base 600 --delta 600 --particles 3000 --inactive 20 --active 80 \
//!     --threads 4 [--cases BASE,DUMMIES] [--layers 12]
//! ```
//!
//! Prints one line per case: `k`, its standard deviation, and the worth
//! `1/k_base - 1/k_case` in pcm with the uncorrelated standard deviation.
//! The same seed is used for every case (correlated sampling; the quoted
//! uncertainty ignores the correlation, so it is an upper bound).
//!
//! **Research, education and V&V only.** Not for facility operation,
//! licensing or safety decisions.

use nee_soon::htr10_rmc::keff_vs_height::SweepStatistics;
use nee_soon::htr10_rmc::parametric::{graphite_worth_cases, ParametricStudy, StudyBed};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// CLI glue (exempt from the reaching-test rule: the cases are
/// `graphite_worth_cases` and the plan `plan_with_slot_temperatures`, both
/// tested in the library; the transport is `ParametricStudy::transport`).
fn main() -> Result<(), String> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let (mut base, mut delta, mut threads, mut layers) = (600.0, 600.0, 4usize, 12usize);
    let mut stats = SweepStatistics { particles: 3000, inactive: 20, active: 80, ..SweepStatistics::QUICK };
    let mut only: Option<Vec<String>> = None;
    let mut it = words.iter();
    while let Some(w) = it.next() {
        let mut val = || it.next().ok_or(format!("{w} needs a value")).cloned();
        let num = |s: String| s.parse::<f64>().map_err(|e| format!("{w}: {e}"));
        match w.as_str() {
            "--base" => base = num(val()?)?,
            "--delta" => delta = num(val()?)?,
            "--threads" => threads = num(val()?)? as usize,
            "--layers" => layers = num(val()?)? as usize,
            "--particles" => stats.particles = num(val()?)? as usize,
            "--inactive" => stats.inactive = num(val()?)? as usize,
            "--active" => stats.active = num(val()?)? as usize,
            "--cases" => only = Some(val()?.split(',').map(String::from).collect()),
            other => return Err(format!("unknown word {other:?}")),
        }
    }
    println!(
        "HTR-10 graphite-zone worth: N = {layers}, base {base} K, +{delta} K, {} x [{} + {}], seed {}, {threads} threads",
        stats.particles, stats.inactive, stats.active, stats.seed
    );
    let mut k_base: Option<(f64, f64)> = None;
    for case in graphite_worth_cases(ThermodynamicTemperature::new::<kelvin>(base), delta) {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == case.name)) {
            continue;
        }
        let study = ParametricStudy {
            bed: StudyBed::Seker { layers },
            temperatures: case.temperatures,
            statistics: stats,
            ..ParametricStudy::default()
        };
        study.check()?;
        let t0 = std::time::Instant::now();
        let plan = study.plan_with_slot_temperatures(&case.slots).map_err(|e| e.to_string())?;
        let nuclides = study.process_all(&plan, threads, |line| eprintln!("  {line}"))?;
        let t_data = t0.elapsed().as_secs_f64();
        let r = study.transport(&plan, &nuclides, threads)?;
        let worth = k_base.map(|(kb, sb)| {
            let w = 1.0 / kb - 1.0 / r.k_mean;
            let s = ((sb / (kb * kb)).powi(2) + (r.k_std / (r.k_mean * r.k_mean)).powi(2)).sqrt();
            (w * 1e5, s * 1e5)
        });
        if case.name == "BASE" {
            k_base = Some((r.k_mean, r.k_std));
        }
        println!(
            "{:<14} k = {:.5} +- {:.5}  worth {}  ({} histories, {} lost, data {:.0} s, transport {:.0} s)",
            case.name,
            r.k_mean,
            r.k_std,
            worth.map_or("-".into(), |(w, s)| format!("{w:+.0} +- {s:.0} pcm ({:+.2} +- {:.2} pcm/K)", w / delta, s / delta)),
            r.histories,
            r.lost_locate,
            t_data,
            t0.elapsed().as_secs_f64() - t_data
        );
    }
    Ok(())
}
