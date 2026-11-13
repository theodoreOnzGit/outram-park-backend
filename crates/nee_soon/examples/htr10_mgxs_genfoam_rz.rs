// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # HTR-10: Monte Carlo MGXS -> GeN-Foam R-Z diffusion, `k` against the Monte Carlo's
//!
//! gh:#1187. Read top to bottom:
//!
//! 1. the four isothermal whole-core Monte Carlo records are the state
//!    points (300.15, 600, 900, 1200 K; each carries its own `k`);
//! 2. the R-Z mesh is the `htr10_rz` mesh cropped to the Monte Carlo model,
//!    each cell's region read from the assembled Monte Carlo geometry
//!    (drawn to `images/zones.png`);
//! 3. the constants: regions -> 4 groups -> rebalanced -> descending -> SI,
//!    stacked with `xsVariables { TFuel log; }`;
//! 4. GeN-Foam's diffusion, every cell at the state's `TFuel`, against the
//!    Monte Carlo `k` of the same record; then the ablations.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_mgxs_genfoam_rz
//! ```
//!
//! Writes `crates/nee_soon/verification_and_validation/htr10_mgxs_genfoam_rz/`
//! `images/zones.png` and `results.csv`. The prediction, written before the
//! first run, and the results are in that folder's `README.md`.

use std::time::Instant;

use nee_soon::htr10_mgxs::Collapse;
use nee_soon::mgxs_genfoam::{Htr10MgxsRegions, Htr10RzNeutronicsMesh, MgxsStatePoints};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

fn main() -> Result<(), String> {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/htr10_mgxs_genfoam_rz");
    std::fs::create_dir_all(out.join("images")).map_err(|e| e.to_string())?;

    // 1. State points.
    let points = MgxsStatePoints::htr10_records().map_err(|e| e.to_string())?;
    for p in &points.points {
        println!(
            "record {:<16} T = {:>7.2} K  MC k = {:.5} +/- {:.5}",
            p.case,
            p.temperature.get::<kelvin>(),
            p.k_eff.get::<ratio>(),
            p.k_std.get::<ratio>()
        );
    }

    // 2. The mesh, its regions read from the Monte Carlo geometry.
    let t0 = Instant::now();
    let mesh = Htr10RzNeutronicsMesh::for_records(&points).map_err(|e| e.to_string())?;
    println!(
        "mesh: {} cells, {} regions, bed solid fraction {:.4}, cone/tube {:.4} ({:.1} s)",
        mesh.n_cells(),
        mesh.regions.len(),
        mesh.solid_fractions.0,
        mesh.solid_fractions.1,
        t0.elapsed().as_secs_f64()
    );
    for (k, (n, r)) in mesh.regions.iter().enumerate() {
        let cells = mesh.zone_of_cell.iter().filter(|z| **z == k).count();
        println!("  region {k:>2} {n:<10} {cells:>4} cells  {r:?}");
    }
    mesh.write_png(out.join("images/zones.png"), 2.0).map_err(|e| e.to_string())?;

    // 3-4. Constants and diffusion at every state point.
    let mut csv = String::from("case,variant,temperature_k,k_mc,k_mc_sigma,k_det,residual_pcm,residual_sigma\n");
    let mut run = |variant: &str, regions: &Htr10MgxsRegions, temps: &[f64]| -> Result<Vec<f64>, String> {
        let nd = regions.nuclear_data(&points).map_err(|e| e.to_string())?;
        let mut ks = Vec::new();
        for &t in temps {
            let tf = vec![ThermodynamicTemperature::new::<kelvin>(t); mesh.n_cells()];
            let t1 = Instant::now();
            let sol = mesh.solve_diffusion(&nd, &tf).map_err(|e| e.to_string())?;
            let mc = points.points.iter().find(|p| (p.temperature.get::<kelvin>() - t).abs() < 1e-9);
            let (k_mc, s_mc) = mc.map_or((f64::NAN, f64::NAN), |p| (p.k_eff.get::<ratio>(), p.k_std.get::<ratio>()));
            let res = (sol.k_eff - k_mc) * 1e5;
            println!(
                "{variant:<28} T = {t:>7.2} K  k_det = {:.5} ({} outer, converged {})  MC {:.5}  residual {:+.0} pcm ({:+.1} sigma)  {:.1} s",
                sol.k_eff,
                sol.outer_iterations,
                sol.converged,
                k_mc,
                res,
                res / (s_mc * 1e5),
                t1.elapsed().as_secs_f64()
            );
            csv.push_str(&format!(
                "{},{variant},{t},{k_mc},{s_mc},{},{res},{}\n",
                mc.map_or("interpolated".to_string(), |p| p.case.clone()),
                sol.k_eff,
                res / (s_mc * 1e5)
            ));
            ks.push(sol.k_eff);
        }
        Ok(ks)
    };
    let base = Htr10MgxsRegions::new(&mesh);
    let ks = run("default (4g)", &base, &[300.15, 600.0, 900.0, 1200.0, 750.0])?;
    println!(
        "trend k(300.15) - k(1200): deterministic {:.0} pcm, Monte Carlo {:.0} pcm",
        (ks[0] - ks[3]) * 1e5,
        (points.points[0].k_eff.get::<ratio>() - points.points[3].k_eff.get::<ratio>()) * 1e5
    );
    run("2 groups", &base.clone().with_collapse(Collapse::Two), &[300.15])?;
    run("8 groups", &base.clone().with_collapse(Collapse::Eight), &[300.15])?;
    run("ablation: no rebalance", &base.clone().without_rebalance(), &[300.15])?;
    run("ablation: ascending order", &base.clone().without_descending_order(), &[300.15])?;
    run("ablation: cm units", &base.clone().without_unit_conversion(), &[300.15])?;
    std::fs::write(out.join("results.csv"), csv).map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}
