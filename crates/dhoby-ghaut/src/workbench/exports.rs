//! # Step 11: post-processing exports
//!
//! What a session hands on: CSV tables (runs, spectra, the reactivity map on a
//! grid), a **report** as kovan markdown (written through
//! [`kovan::artifact::render_artifact_block`] and readable by
//! [`kovan::artifact::parse_document`], like the recipe), the recipe itself
//! and, when Step 6 made one, the reactivity map's plain TOML.
//!
//! Every file states that it is research, education and V&V output only. The
//! report quotes the preset's V&V status from its record, and an edited recipe
//! is reported as derived, with no V&V standing.

use std::path::{Path, PathBuf};

use super::reactivity_map::{rho_pcm, sigma_rho_pcm, ReactivityMap};
use super::recipe::{ElementStatus, Recipe, RecipeError, RunRecord};

/// One run's lethargy spectrum, as Step 5 tallied it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// The run's label.
    pub label: String,
    /// Bin edges \[eV\].
    pub edges: Vec<f64>,
    /// Flux per unit lethargy (normalised to unit total flux) and its
    /// relative standard error, per bin.
    pub phi_per_lethargy: Vec<(f64, f64)>,
}

/// The runs as CSV, one row each, with `ρ` and `σ_ρ` beside `k ± σ`.
#[must_use]
pub fn runs_csv(runs: &[RunRecord]) -> String {
    let mut out = String::from(
        "label,temperature_k,rod_insertion,particles,inactive,active,seed,k,sigma_k,rho_pcm,sigma_rho_pcm,transport_s\n",
    );
    for r in runs {
        out.push_str(&format!(
            "{},{:.4},{:.4},{},{},{},{},{:.6},{:.6},{:.2},{:.2},{:.2}\n",
            r.label.replace(',', ";"),
            r.temperature_k,
            r.rod_insertion,
            r.particles,
            r.inactive,
            r.active,
            r.seed,
            r.k,
            r.sigma,
            rho_pcm(r.k),
            sigma_rho_pcm(r.k, r.sigma),
            r.transport_s
        ));
    }
    out
}

/// Every spectrum as long-format CSV (`run,e_lo_ev,e_hi_ev,phi_per_lethargy,rel_err`).
#[must_use]
pub fn spectra_csv(spectra: &[Spectrum]) -> String {
    let mut out = String::from("run,e_lo_ev,e_hi_ev,phi_per_lethargy,rel_err\n");
    for s in spectra {
        for (i, (v, e)) in s.phi_per_lethargy.iter().enumerate() {
            let (Some(lo), Some(hi)) = (s.edges.get(i), s.edges.get(i + 1)) else {
                continue;
            };
            out.push_str(&format!(
                "{},{lo:.6e},{hi:.6e},{v:.6e},{e:.4e}\n",
                s.label.replace(',', ";")
            ));
        }
    }
    out
}

fn artifact(id: &str, heading: &str, body: &str, timestamp: &str) -> Result<String, RecipeError> {
    let payload = kovan::artifact::ArtifactToml {
        extra: Default::default(),
        kovan: kovan::artifact::ArtifactMeta {
            extra: Default::default(),
            id: id.to_string(),
            kind: kovan::artifact::ArtifactKind::Note,
            created: timestamp.to_string(),
            modified: timestamp.to_string(),
            reviewed: None,
            origin: None,
        },
        source: None,
        classification: Default::default(),
        extraction: None,
        relation: None,
        connections: Vec::new(),
    };
    let mut block = kovan::artifact::render_artifact_block(1, heading, &payload, body)
        .map_err(RecipeError::Write)?;
    block.push('\n');
    Ok(block)
}

/// The report, as kovan markdown: one note artifact per section (`report`,
/// `report-model`, `report-runs`, `report-map` when there is a map,
/// `report-files`).
///
/// # Errors
///
/// [`RecipeError::Write`] if kovan refuses a block.
pub fn report_markdown(
    recipe: &Recipe,
    map: Option<&ReactivityMap>,
    spectra: &[Spectrum],
    files: &[String],
    timestamp: &str,
) -> Result<String, RecipeError> {
    let h = &recipe.header;
    let standing = if h.edited {
        "**Derived from the preset and edited: no V&V standing.**".to_string()
    } else {
        format!("Preset, V&V status as its record states it: {}", h.vv_status)
    };
    let mut out = artifact(
        "report",
        &format!("Report: {}", h.title),
        &format!(
            "A Dhoby Ghaut workbench report, written {timestamp}. Research, education and V&V \
             only: not for facility operation, licensing, safety decisions or any operational \
             use.\n\n{standing}\n\nRecipe model hash: `{}`. ENDF library: {} ({}).\n",
            super::reactivity_map::recipe_hash(recipe),
            recipe.nuclear_data.library,
            recipe.nuclear_data.endf_dir
        ),
        timestamp,
    )?;
    // What the model carries and lacks.
    let mut model = String::from("| part | status | note |\n|---|---|---|\n");
    for e in recipe.reflector.elements.iter().chain(&recipe.inserts.elements) {
        let badge = match e.status {
            ElementStatus::InModel => "in model",
            ElementStatus::Simplified => "simplified",
            ElementStatus::NotInModel => "NOT in model",
        };
        model.push_str(&format!("| {} | {badge} | {} |\n", e.name, e.note.replace('|', "/")));
    }
    if !recipe.monte_carlo.ablations.is_empty() {
        model.push_str(&format!(
            "\nAblations (physics switched off): {}.\n",
            recipe.monte_carlo.ablations.join(", ")
        ));
    }
    out.push_str(&artifact("report-model", "Model and its gaps", &model, timestamp)?);
    // Runs.
    let runs = &recipe.monte_carlo.runs;
    let mut body = if runs.is_empty() {
        "No Monte Carlo run was made.\n".to_string()
    } else {
        let mut t = String::from(
            "| run | T [K] | rods | statistics | k ± σ | ρ ± σ [pcm] |\n|---|---|---|---|---|---|\n",
        );
        for r in runs {
            t.push_str(&format!(
                "| {} | {:.2} | {:.2} | {} × [{} + {}] | {:.5} ± {:.5} | {:.0} ± {:.0} |\n",
                r.label,
                r.temperature_k,
                r.rod_insertion,
                r.particles,
                r.inactive,
                r.active,
                r.k,
                r.sigma,
                rho_pcm(r.k),
                sigma_rho_pcm(r.k, r.sigma)
            ));
        }
        t
    };
    body.push_str(&format!(
        "\nσ is each run's own 1σ over its active generations. Spectra exported for {} of {} runs \
         (a spectrum is kept for runs made in this session only).\n",
        spectra.len(),
        runs.len()
    ));
    out.push_str(&artifact("report-runs", "Monte Carlo runs", &body, timestamp)?);
    if let Some(m) = map {
        let f = &m.fit;
        let opt = |v: Option<f64>| v.map_or("n/a".to_string(), |x| format!("{x:.1}"));
        let axes: Vec<String> = m
            .axes
            .iter()
            .map(|a| format!("{} over [{}, {}] in {:?}", a.name.label(), a.lo, a.hi, a.basis))
            .collect();
        let fixed: Vec<String> = m
            .fixed
            .iter()
            .map(|x| format!("{} = {} ({})", x.name.label(), x.value, x.note))
            .collect();
        let body = format!(
            "**Low fidelity: a neutronics-only surrogate.** {}\n\n\
             Fitted axes: {}. Fixed: {}.\n\n\
             | quantity | value |\n|---|---|\n\
             | order (total degree) | {} |\n| runs / terms | {} / {} |\n\
             | held-out RMS (leave-one-out) [pcm] | {} |\n| held-out max [pcm] | {} |\n\
             | RMS Monte Carlo σ_ρ [pcm] | {:.1} |\n| χ²/dof (in-sample) | {} |\n\n\
             Quote the held-out error beside the Monte Carlo σ; the in-sample χ² cannot see \
             over-fitting. Synthetic runs: {}.\n",
            m.fidelity,
            if axes.is_empty() { "none".into() } else { axes.join("; ") },
            if fixed.is_empty() { "none".into() } else { fixed.join("; ") },
            f.order,
            f.n_runs,
            f.n_terms,
            opt(f.held_out_rms_pcm),
            opt(f.held_out_max_pcm),
            f.mc_sigma_rms_pcm,
            f.chi2_per_dof.map_or("n/a".to_string(), |x| format!("{x:.2}")),
            m.provenance.synthetic
        );
        out.push_str(&artifact("report-map", "Reactivity map", &body, timestamp)?);
    }
    let list = if files.is_empty() {
        "None.\n".to_string()
    } else {
        files.iter().map(|f| format!("- `{f}`\n")).collect()
    };
    out.push_str(&artifact("report-files", "Exported files", &list, timestamp)?);
    Ok(out)
}

/// Write the recipe out and read it back through kovan; `Ok` when the read
/// recipe equals the written one.
///
/// # Errors
///
/// The first section that differs, or the read error.
pub fn verify_round_trip(recipe: &Recipe) -> Result<(), String> {
    let md = recipe
        .to_markdown(&super::recipe::now_rfc3339())
        .map_err(|e| e.to_string())?;
    let back = Recipe::from_markdown(&md).map_err(|e| e.to_string())?;
    match differing_sections(recipe, &back).first() {
        None => Ok(()),
        Some(s) => Err(format!("section `{s}` differs after the round trip")),
    }
}

/// The recipe sections (by kovan id) in which `a` and `b` differ.
#[must_use]
pub fn differing_sections(a: &Recipe, b: &Recipe) -> Vec<&'static str> {
    let mut v = Vec::new();
    let checks = [
        ("recipe", a.header == b.header),
        ("step-0", a.nuclear_data == b.nuclear_data),
        ("step-1", a.pebble_bed == b.pebble_bed),
        ("step-2", a.pebble_design == b.pebble_design),
        ("step-3", a.reflector == b.reflector),
        ("step-4", a.inserts == b.inserts),
        ("review", a.review == b.review),
        ("step-5", a.monte_carlo == b.monte_carlo),
        ("step-6", a.branch == b.branch),
    ];
    for (id, same) in checks {
        if !same {
            v.push(id);
        }
    }
    v
}

/// Write every export into `dir`: `runs.csv`, `spectra.csv` (when there are
/// spectra), `reactivity_map.toml` and `reactivity_map_grid.csv` (when there
/// is a map), `recipe.md` and `report.md`. Returns the paths written.
///
/// # Errors
///
/// The first file that could not be written, or a serialisation error.
pub fn write_all(
    dir: &Path,
    recipe: &Recipe,
    map: Option<&ReactivityMap>,
    spectra: &[Spectrum],
    timestamp: &str,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<(String, String)> = vec![("runs.csv".into(), runs_csv(&recipe.monte_carlo.runs))];
    if !spectra.is_empty() {
        files.push(("spectra.csv".into(), spectra_csv(spectra)));
    }
    if let Some(m) = map {
        files.push(("reactivity_map.toml".into(), m.to_toml().map_err(|e| e.to_string())?));
        files.push(("reactivity_map_grid.csv".into(), m.grid_csv(31)));
    }
    files.push((
        "recipe.md".into(),
        recipe.to_markdown(timestamp).map_err(|e| e.to_string())?,
    ));
    let mut names: Vec<String> = files.iter().map(|f| f.0.clone()).collect();
    names.push("report.md".into());
    let report = report_markdown(recipe, map, spectra, &names, timestamp).map_err(|e| e.to_string())?;
    files.push(("report.md".into(), report));
    let mut written = Vec::new();
    for (name, text) in files {
        let p = dir.join(name);
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        written.push(p);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::super::reactivity_map::{AxisBasis, MapSettings, Provenance};
    use super::*;

    /// A recipe whose runs are SYNTHETIC (made up for the test, not Monte
    /// Carlo), at four temperatures on a √T law.
    fn synthetic_recipe() -> Recipe {
        let mut r = super::super::recipe::tests::sample();
        r.monte_carlo.runs = [300.0, 500.0, 800.0, 1200.0]
            .iter()
            .enumerate()
            .map(|(i, &t)| RunRecord {
                label: format!("Synthetic {}", i + 1),
                rod_insertion: 0.0,
                temperature_k: t,
                particles: 0,
                inactive: 0,
                active: 0,
                seed: 0,
                k: 1.0 / (1.0 - (2000.0 - 100.0 * f64::sqrt(t)) * 1.0e-5),
                sigma: 3.0e-4,
                transport_s: 0.0,
            })
            .collect();
        r
    }

    fn synthetic_map(r: &Recipe) -> ReactivityMap {
        let mut m = ReactivityMap::fit(
            &r.monte_carlo.runs,
            MapSettings {
                order: 1,
                temperature_basis: AxisBasis::Sqrt,
            },
        )
        .expect("fit");
        m.provenance = Provenance::from_recipe(r, "2026-10-05T00:00:00Z", true);
        m
    }

    /// The report is kovan markdown: kovan reads every section back as a note
    /// artifact, with no problems.
    #[test]
    fn the_report_is_read_back_by_kovan() {
        let r = synthetic_recipe();
        let m = synthetic_map(&r);
        let spectra = vec![Spectrum {
            label: "Synthetic 1".into(),
            edges: vec![1.0e-5, 1.0, 2.0e7],
            phi_per_lethargy: vec![(0.4, 0.01), (0.6, 0.02)],
        }];
        let md = report_markdown(&r, Some(&m), &spectra, &["runs.csv".into()], "2026-10-05T00:00:00Z")
            .expect("report");
        let doc = kovan::artifact::parse_document(&md);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        for id in ["report", "report-model", "report-runs", "report-map", "report-files"] {
            assert!(doc.get(id).is_some(), "{id} missing");
        }
        assert!(md.contains("Low fidelity") && md.contains("Synthetic runs: true"));
        assert_eq!(spectra_csv(&spectra).lines().count(), 3);
        assert_eq!(runs_csv(&r.monte_carlo.runs).lines().count(), 5);
    }

    /// Every export lands in the folder; the recipe written there loads back
    /// equal, and the map TOML reads back equal.
    #[test]
    fn write_all_writes_files_that_load_back() {
        let r = synthetic_recipe();
        let m = synthetic_map(&r);
        let dir = std::env::temp_dir().join(format!("dhoby_ghaut_exports_{}", std::process::id()));
        let written = write_all(&dir, &r, Some(&m), &[], "2026-10-05T00:00:00Z").expect("write");
        let names: Vec<String> = written
            .iter()
            .map(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default())
            .collect();
        assert_eq!(
            names,
            ["runs.csv", "reactivity_map.toml", "reactivity_map_grid.csv", "recipe.md", "report.md"]
        );
        let text = std::fs::read_to_string(dir.join("recipe.md")).expect("recipe.md");
        let back = Recipe::from_markdown(&text).expect("read");
        assert!(differing_sections(&r, &back).is_empty());
        let text = std::fs::read_to_string(dir.join("reactivity_map.toml")).expect("toml");
        assert_eq!(ReactivityMap::from_toml(&text).expect("map"), m);
        assert!(verify_round_trip(&r).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
