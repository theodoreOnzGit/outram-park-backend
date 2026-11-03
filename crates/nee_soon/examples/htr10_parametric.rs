// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **HTR-10 whole core, every setting on the command line** (gh:#1139,
//! epic #1137): the bed, the ten rods, a temperature for each of the eight
//! zones, and the statistics, through `nee_soon::htr10_rmc::parametric::
//! ParametricStudy`, the struct the browser demo builds its model with too.
//!
//! Each zone's data are processed exactly at its temperature (RECONR,
//! BROADR, HEATR, PURR for every nuclide; LEAPR for every bound law), then
//! k_eff is run with `run_keff_csg_hybrid`, as the recorded runs.
//!
//! ```text
//! cargo run --release -p nee_soon --example htr10_parametric -- \
//!     --layers 12 --temp-all 27 --temp fuel=900 --temp coolant=500 \
//!     --particles 2000 --inactive 30 --active 70 --threads 8
//! ```
//!
//! Words: every one `ParametricStudy::from_args` lists, plus `--threads N`
//! (default 4) and `--plan` (print the zones, the nuclide array and the jobs,
//! then stop without processing anything).
//!
//! **Research, education and V&V only.** Not for facility operation,
//! licensing or safety decisions.

use nee_soon::htr10_rmc::parametric::ParametricStudy;
use nee_soon::htr10_rmc::zones::Htr10Zone;
use uom::si::thermodynamic_temperature::degree_celsius;

/// CLI glue (exempt from the reaching-test rule: the parsing it relies on is
/// `ParametricStudy::from_args`, tested in the library).
fn main() -> Result<(), String> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let (study, rest) = ParametricStudy::from_args(&words)?;
    let (mut threads, mut plan_only) = (4usize, false);
    let mut it = rest.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--threads" => {
                threads = it
                    .next()
                    .ok_or("--threads needs a value")?
                    .parse()
                    .map_err(|e| format!("--threads: {e}"))?;
            }
            "--plan" => plan_only = true,
            other => return Err(format!("unknown word {other:?}")),
        }
    }
    study.check()?;
    println!("HTR-10 parametric study ({:?})", study.bed);
    for z in Htr10Zone::ALL {
        let c = study.temperatures.get(z).get::<degree_celsius>();
        let warn = if study.temperatures.above_triso_limit().contains(&z) {
            "  ** above the 1600 °C TRISO limit **"
        } else {
            ""
        };
        println!("  {:<18} {c:>7.1} °C{warn}", z.label());
    }
    println!("  rods {:?}", study.rod_insertions);
    let plan = study.plan().map_err(|e| e.to_string())?;
    println!(
        "  {} nuclides ({} slots), {} jobs",
        plan.entries.len(),
        plan.layout.len(),
        plan.jobs.len()
    );
    if plan_only {
        for j in &plan.jobs {
            println!("    {}", j.label());
        }
        return Ok(());
    }
    let t0 = std::time::Instant::now();
    let r = study.run_keff(threads, |line| eprintln!("  {line}"))?;
    println!(
        "k_eff = {:.5} ± {:.5}  ({} histories, {} lost, {:.0} s on {threads} threads)",
        r.k_mean,
        r.k_std,
        r.histories,
        r.lost_locate,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
