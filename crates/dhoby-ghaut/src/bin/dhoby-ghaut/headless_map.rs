//! `--headless-map`: Step 6 (A) and Step 11's exports with no window.
//!
//! ```text
//! dhoby-ghaut --headless-map --synthetic [--out dir]
//! dhoby-ghaut --headless-map --sweep 300,600,900 [--particles 200 --inactive 5 --active 10 --threads 4]
//! dhoby-ghaut --headless-map --recipe my_recipe.md      # the runs saved in the recipe
//!     [--order 1] [--basis sqrt|ln|linear] [--out dir]
//! ```
//!
//! Fits the reactivity map over the runs and writes, into `--out` (default
//! `target/dhoby-ghaut_out/map`): `reactivity_map.toml`, `runs.csv`,
//! `reactivity_map_grid.csv`, `spectra.csv` (sweep only), `recipe.md` and
//! `report.md`.
//!
//! `--synthetic` replaces the runs with SYNTHETIC ones (made up from a stated
//! law, NOT Monte Carlo) and the map's provenance says `synthetic = true`; it
//! exists to pin the export path in a test. `--sweep` runs real Monte Carlo,
//! one run per temperature, on the recipe's geometry and Step 5 settings.

use std::path::Path;

use dhoby_ghaut::web_demo::link::NativeEngine;
use dhoby_ghaut::workbench::exports::{write_all, Spectrum};
use dhoby_ghaut::workbench::reactivity_map::{AxisBasis, Provenance, ReactivityMap};
use dhoby_ghaut::workbench::recipe::{now_rfc3339, Branch, Recipe, RunRecord};

use crate::engine::{Engine, Ev, KeffJob, Req};

/// The made-up law the synthetic runs follow, stated in every synthetic map.
pub const SYNTHETIC_LAW: &str =
    "SYNTHETIC (made up for a test, not Monte Carlo): rho_pcm = 4000 - 150 sqrt(T/K) \
     + (+/-)30 alternating, sigma_k = 3e-4";

/// SYNTHETIC runs at `temps` on [`SYNTHETIC_LAW`].
#[must_use]
pub fn synthetic_runs(temps: &[f64]) -> Vec<RunRecord> {
    temps
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let noise = if i % 2 == 0 { 30.0 } else { -30.0 };
            let rho = 4000.0 - 150.0 * t.sqrt() + noise;
            RunRecord {
                label: format!("SYNTHETIC {}", i + 1),
                rod_insertion: 0.0,
                temperature_k: t,
                particles: 0,
                inactive: 0,
                active: 0,
                seed: 0,
                k: 1.0 / (1.0 - rho * 1.0e-5),
                sigma: 3.0e-4,
                transport_s: 0.0,
            }
        })
        .collect()
}

/// Real Monte Carlo at each temperature in `temps`, on `r`'s geometry and
/// Step 5 settings. The runs are appended to `r`; the spectra returned.
pub fn sweep(r: &mut Recipe, temps: &[f64]) -> Result<Vec<Spectrum>, String> {
    let mut engine = Engine::default();
    let mut ok = false;
    // Built to the recipe's design (gh:#566); what it cannot represent is
    // printed, never silently dropped.
    let plan = crate::design::plan(r);
    for n in &plan.not_built {
        eprintln!("NOT built: {n}");
    }
    engine.handle(
        Req::Assemble {
            rings: r.pebble_bed.rings,
            layers: r.pebble_bed.layers,
            design: plan.design,
        },
        &mut |e| {
            if let Ev::Assembled(i, _) = e {
                println!("assembled: {} cells, {} tiles ({:.1} s)", i.cells, i.tiles, i.seconds);
                ok = true;
            }
        },
    );
    if !ok {
        return Err("assembly failed".into());
    }
    let mut spectra = Vec::new();
    for &t in temps {
        let mc = r.monte_carlo.clone();
        let label = format!("Run {} (map, {t:.0} K)", mc.runs.len() + 1);
        let job = KeffJob {
            live: Default::default(),
            label: label.clone(),
            particles: mc.particles,
            inactive: mc.inactive,
            active: mc.active,
            seed: mc.seed,
            threads: mc.threads,
            temperature_k: t,
            bins_per_decade: mc.spectrum_bins_per_decade,
            tapes: crate::engine::tape_source(std::path::Path::new(&r.nuclear_data.endf_dir)),
        };
        let (mut outcome, mut err) = (None, None);
        engine.handle(Req::RunKeff(job), &mut |e| match e {
            Ev::DataReady { seconds, cached } => {
                println!("  {t:.0} K: nuclear data {} ({seconds:.1} s)", if cached { "reused" } else { "processed" })
            }
            Ev::KeffDone(o) => outcome = Some(o),
            Ev::Error(m) => err = Some(m),
            _ => {}
        });
        if let Some(m) = err {
            return Err(m);
        }
        let o = outcome.ok_or("no result")?;
        println!("  {label}: k = {:.5} +/- {:.5} ({:.1} s transport)", o.k, o.sigma, o.transport_s);
        r.monte_carlo.runs.push(RunRecord {
            label: label.clone(),
            rod_insertion: mc.rod_insertion,
            temperature_k: t,
            particles: mc.particles,
            inactive: mc.inactive,
            active: mc.active,
            seed: mc.seed,
            k: o.k,
            sigma: o.sigma,
            transport_s: o.transport_s,
        });
        spectra.push(Spectrum {
            label,
            edges: o.edges,
            phi_per_lethargy: o.phi_per_lethargy,
        });
    }
    Ok(spectra)
}

/// Fit the map over `r`'s runs and write every export to `out`.
pub fn fit_and_export(r: &mut Recipe, spectra: &[Spectrum], synthetic: bool, out: &Path) -> Result<ReactivityMap, String> {
    r.branch.branch = Branch::ReactivityMap;
    let mut m = ReactivityMap::fit(&r.monte_carlo.runs, r.branch.map).map_err(|e| e.to_string())?;
    let now = now_rfc3339();
    m.provenance = Provenance::from_recipe(r, &now, synthetic);
    if synthetic {
        m.provenance.notes.insert(0, SYNTHETIC_LAW.into());
    }
    let files = write_all(out, r, Some(&m), spectra, &now)?;
    for f in files {
        println!("wrote {}", f.display());
    }
    Ok(m)
}

/// The `--headless-map` entry point.
pub fn run(args: &[String], recipe: Option<Recipe>) -> Result<(), String> {
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let mut r = recipe.unwrap_or_else(crate::preset::htr10);
    if let Some(o) = arg("--order").and_then(|v| v.parse().ok()) {
        r.branch.map.order = o;
    }
    if let Some(b) = arg("--basis") {
        r.branch.map.temperature_basis = match b.as_str() {
            "sqrt" => AxisBasis::Sqrt,
            "ln" => AxisBasis::Ln,
            "linear" => AxisBasis::Linear,
            other => return Err(format!("--basis {other}: expected sqrt, ln or linear")),
        };
    }
    let num = |k: &str| arg(k).and_then(|v| v.parse::<usize>().ok());
    let mc = &mut r.monte_carlo;
    mc.particles = num("--particles").unwrap_or(mc.particles);
    mc.inactive = num("--inactive").unwrap_or(mc.inactive);
    mc.active = num("--active").unwrap_or(mc.active);
    mc.threads = num("--threads").unwrap_or(mc.threads);
    let out = arg("--out").unwrap_or_else(|| "target/dhoby-ghaut_out/map".into());
    let synthetic = args.iter().any(|a| a == "--synthetic");
    let mut spectra = Vec::new();
    if synthetic {
        r.monte_carlo.runs = synthetic_runs(&[300.0, 450.0, 600.0, 750.0, 900.0, 1200.0]);
    } else if let Some(list) = arg("--sweep") {
        let temps: Vec<f64> = list.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        spectra = sweep(&mut r, &temps)?;
    }
    let m = fit_and_export(&mut r, &spectra, synthetic, Path::new(&out))?;
    let f = &m.fit;
    println!(
        "map: order {}, {} runs, {} terms; held-out RMS {} pcm beside MC sigma_rho RMS {:.1} pcm; chi2/dof {}",
        f.order,
        f.n_runs,
        f.n_terms,
        f.held_out_rms_pcm.map_or("n/a".into(), |x| format!("{x:.1}")),
        f.mc_sigma_rms_pcm,
        f.chi2_per_dof.map_or("n/a".into(), |x| format!("{x:.2}"))
    );
    for h in &m.held_out {
        println!(
            "  {:<24} rho {:>8.1} +/- {:>5.1}  held-out {:>8}  miss {:>7}",
            h.label,
            h.rho_pcm,
            h.sigma_rho_pcm,
            h.predicted_pcm.map_or("n/a".into(), |v| format!("{v:.1}")),
            h.error_pcm().map_or("n/a".into(), |v| format!("{v:+.1}"))
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The headless map export on SYNTHETIC runs (made up, not Monte Carlo):
    /// every file is written, the map TOML reads back with `synthetic = true`
    /// and the law stated, the recipe written beside it loads back equal, and
    /// an order-1 √T map recovers the law to within the ±30 pcm made-up noise.
    #[test]
    fn the_headless_map_export_on_synthetic_runs() {
        let dir = std::env::temp_dir().join(format!("dhoby_ghaut_map_{}", std::process::id()));
        let mut r = crate::preset::htr10();
        r.branch.map.order = 1;
        r.monte_carlo.runs = synthetic_runs(&[300.0, 450.0, 600.0, 750.0, 900.0, 1200.0]);
        let m = fit_and_export(&mut r, &[], true, &dir).expect("export");
        let text = std::fs::read_to_string(dir.join("reactivity_map.toml")).expect("toml");
        let back = ReactivityMap::from_toml(&text).expect("read");
        assert_eq!(back, m);
        assert!(back.provenance.synthetic);
        assert!(back.provenance.notes[0].starts_with("SYNTHETIC"));
        assert_eq!(back.provenance.runs.len(), 6);
        assert!(back.provenance.recipe_hash.starts_with("sha256:"));
        assert!(text.contains("LOW FIDELITY"));
        let law = |t: f64| 4000.0 - 150.0 * t.sqrt();
        for t in [350.0, 800.0, 1100.0] {
            let v = back.rho_pcm_at(&[(dhoby_ghaut::workbench::reactivity_map::AxisName::TemperatureK, t)]).expect("eval");
            assert!((v - law(t)).abs() < 30.0, "{t} K: {v} vs {}", law(t));
        }
        let ho = back.fit.held_out_rms_pcm.expect("held out");
        assert!(ho > 20.0 && ho < 80.0, "held-out RMS {ho} pcm against ±30 pcm noise");
        let md = std::fs::read_to_string(dir.join("recipe.md")).expect("recipe");
        assert_eq!(Recipe::from_markdown(&md).expect("recipe reads"), r);
        for f in ["runs.csv", "reactivity_map_grid.csv", "report.md"] {
            assert!(dir.join(f).is_file(), "{f}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
