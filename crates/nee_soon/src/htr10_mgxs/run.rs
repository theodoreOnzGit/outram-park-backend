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

//! Running one case: data, model, two passes, condensation, collapse, checks.

use std::time::Instant;

use outram_mc_libs::physics::keff::{ComputeType, KeffResult, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg_hybrid_with_progress, GenerationProgress,
};
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use outram_mc_libs::tally::tally::Tally;
use uom::si::thermodynamic_temperature::kelvin;

use super::cases::{CaseModel, MgxsCase, RECORDED_K_N12_T300};
use super::groups::{tally_structure, Collapse};
use super::record::{CaseRecord, Check, CollapsedSummary, PassRecord, Provenance};
use crate::htr10_rmc::core_model::{assemble_explicit_triso, mat};
use crate::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use crate::htr10_rmc::keff_vs_height::{
    bed_majorant, fissile_entropy_mesh, fissile_source_box, RINGS,
};
use crate::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use crate::mgxs::{condense, matrix_tally, scalar_tally, MgxsError, MgxsLibrary};

/// Why a case could not produce a record.
#[derive(Debug, thiserror::Error)]
pub enum CaseError {
    /// The nuclear data could not be planned or loaded (a missing tape
    /// names the submodule to fetch).
    #[error("nuclear data: {0}")]
    Data(String),
    /// The tallies could not be condensed.
    #[error("condensation: {0}")]
    Condense(#[from] MgxsError),
    /// A thread count of zero.
    #[error("threads must be at least 1")]
    NoThreads,
}

/// Run `case` on `threads` pinned threads and return its record. Nothing is
/// written; [`CaseRecord::write`] does that.
///
/// `say` receives a short line at each stage and every tenth of a pass, for
/// a log; a caller that wants silence passes `|_| {}`.
///
/// # Errors
///
/// [`CaseError`] if the data cannot be loaded or the tallies cannot be
/// condensed. A run that completes always returns a record, even when its
/// checks fail: a failed check is a result to report, not an error.
pub fn run_case(
    case: &MgxsCase,
    threads: usize,
    provenance: Provenance,
    mut say: impl FnMut(&str) + Send,
) -> Result<CaseRecord, CaseError> {
    if threads == 0 {
        return Err(CaseError::NoThreads);
    }
    let t_all = Instant::now();
    let t_k = case.temperature().get::<kelvin>();
    say(&format!(
        "case {}: {}, {} K, {} x [{} + {}], seed {}, {threads} threads",
        case.name,
        case.model.describe(),
        t_k,
        case.particles,
        case.inactive,
        case.active,
        case.seed
    ));

    // 1. Data and materials at the case temperature.
    let cfg = Htr10DataConfig {
        temperature: case.temperature(),
        ..Htr10DataConfig::default()
    };
    let layout = Htr10NuclideLayout::plan(&cfg).map_err(|e| CaseError::Data(e.to_string()))?;
    let mut diag = RunDiagnostics::new(format!("htr10_mgxs {}", case.name));
    say("processing nuclear data");
    let nucs = load_htr10_nuclides(&cfg, &layout, &mut diag)
        .map_err(|e| CaseError::Data(e.to_string()))?;
    let data_s = diag.data_seconds();
    say(&format!("nuclear data done in {data_s:.0} s"));
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(t_k));
    let mut data_notes = vec![
        format!(
            "{:?}, graphite S(a,b) {:?}, thermal {:?}, carbon {:?}, coolant {:?}, rod metal {}",
            cfg.library,
            cfg.graphite_law,
            cfg.thermal,
            cfg.carbon,
            cfg.coolant,
            cfg.rod_metal.label()
        ),
        "TECDOC-1382 reflector zone 22, natural boron (benchmark default materials)".to_string(),
    ];
    data_notes.extend(layout.notes.iter().cloned());

    // 2. The model.
    let core = match case.model {
        CaseModel::FullCore { layers } => assemble_explicit_triso(RINGS, layers, 0),
    };
    let majorant = bed_majorant(&mats, &nucs);
    let entropy = fissile_entropy_mesh(&core);
    let source = fissile_source_box(&core);
    let settings = KeffSettings {
        n_particles: case.particles,
        n_inactive: case.inactive,
        n_active: case.active,
        temperature_k: t_k,
        seed: case.seed,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };
    data_notes.push(format!(
        "delta-tracked bed tallied with the {:?} estimator (gh:#598)",
        settings.delta_tally_estimator
    ));

    // 3. Two passes at one seed.
    let groups = tally_structure();
    let all: Vec<usize> = (0..mats.len()).collect();
    let names: Vec<String> = mats.iter().map(|m| m.name.clone()).collect();
    let mut scalar = scalar_tally(1, &groups, all.clone());
    let mut matrix = matrix_tally(2, &groups, all);
    let generations = case.inactive + case.active;
    let mut passes = Vec::with_capacity(2);
    for (label, tally) in [("scalar", &mut scalar), ("matrix", &mut matrix)] {
        let (res, secs) = transport(&core, &mats, &nucs, &majorant, &entropy, source, &settings, tally, generations, label, &mut say);
        say(&format!(
            "{label} pass: k = {:.6} +/- {:.6}, {:.0} s",
            res.k_mean, res.k_std, secs
        ));
        passes.push(PassRecord {
            pass: label.to_string(),
            k: res.k_mean,
            k_sigma: res.k_std,
            histories: res.histories,
            lost_locate: res.lost_locate,
            leak_vacuum: res.leak_vacuum,
            entropy: res.entropy.clone(),
            transport_s: secs,
        });
    }

    // 4. Condense and collapse.
    let library = condense(&groups, &names, &scalar, &matrix, case.active as u64)?;
    let mut collapsed = Vec::with_capacity(Collapse::ALL.len());
    for c in Collapse::ALL {
        let lib = library.collapse(&c.structure())?;
        collapsed.push(CollapsedSummary {
            structure: c.label().to_string(),
            n_groups: lib.groups.n_groups(),
            k_inf: lib.k_inf(),
            max_balance_residual: max_balance_residual(&lib),
            weighted_balance_residual: weighted_balance_residual(&lib),
        });
    }

    let checks = checks(case, &passes, &collapsed, &library);
    let hardware = diag.hardware().headline();
    Ok(CaseRecord {
        case: case.name.to_string(),
        purpose: case.purpose.to_string(),
        issue: case.issue.to_string(),
        model: case.model.describe(),
        temperature_k: t_k,
        particles: case.particles,
        inactive: case.inactive,
        active: case.active,
        seed: case.seed,
        threads,
        data_notes,
        provenance,
        hardware,
        data_s,
        passes,
        library,
        collapsed,
        checks,
        total_s: t_all.elapsed().as_secs_f64(),
    })
}

/// One transport pass, reporting every tenth of its generations.
#[allow(clippy::too_many_arguments)]
fn transport(
    core: &crate::htr10_rmc::core_model::AssembledCore,
    mats: &[outram_mc_libs::material::material::Material],
    nucs: &[outram_mc_libs::material::nuclide::Nuclide],
    majorant: &outram_mc_libs::pebble_beds::delta_tracking::Majorant,
    entropy: &outram_mc_libs::tally::mesh::RegularMesh,
    source: outram_mc_libs::physics::transport_csg::SourceBox,
    settings: &KeffSettings,
    tally: &mut Tally,
    generations: usize,
    label: &str,
    say: &mut (impl FnMut(&str) + Send),
) -> (KeffResult, f64) {
    let t = Instant::now();
    let step = (generations / 10).max(1);
    let res = run_keff_csg_hybrid_with_progress(
        &core.geometry,
        mats,
        nucs,
        std::slice::from_ref(majorant),
        Some(entropy),
        source,
        settings,
        Some(tally),
        |g: GenerationProgress| {
            if (g.index + 1) % step == 0 {
                say(&format!(
                    "{label} pass: generation {}/{generations}, k {:.5}, {:.0} s",
                    g.index + 1,
                    g.k,
                    t.elapsed().as_secs_f64()
                ));
            }
        },
    );
    (res, t.elapsed().as_secs_f64())
}

/// Largest relative balance residual over every zone and visited group.
fn max_balance_residual(lib: &MgxsLibrary) -> f64 {
    lib.balance_check()
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .filter(|(t, _, _)| *t > 0.0)
        .map(|(_, _, r)| r.abs())
        .fold(0.0, f64::max)
}

/// Relative balance residual weighted by each bin's collision rate
/// `Sigma_t phi`, over every zone and group.
fn weighted_balance_residual(lib: &MgxsLibrary) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for z in &lib.zones {
        for g in 0..z.flux.len() {
            let w = z.total[g] * z.flux[g];
            if w > 0.0 {
                let row: f64 = z.scatter[g].iter().sum();
                num += w * ((z.absorption[g] + row - z.total[g]) / z.total[g]).abs();
                den += w;
            }
        }
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// The checks every record carries. Each one is recorded whether it holds or
/// not; none of them stops the run.
fn checks(
    case: &MgxsCase,
    passes: &[PassRecord],
    collapsed: &[CollapsedSummary],
    library: &MgxsLibrary,
) -> Vec<Check> {
    let mut out = Vec::new();
    if let [a, b] = passes {
        out.push(Check {
            name: "the two passes ran identical histories".into(),
            passed: a.k == b.k && a.histories == b.histories,
            detail: format!(
                "k {:.9} vs {:.9}, histories {} vs {}; if they differ the scalar flux is not the matrix pass's denominator",
                a.k, b.k, a.histories, b.histories
            ),
        });
    }
    let lost: u64 = passes.iter().map(|p| p.lost_locate).sum();
    out.push(Check {
        name: "no history was lost".into(),
        passed: lost == 0,
        detail: format!("{lost} lost over both passes"),
    });
    if let Some(fine) = collapsed.last() {
        let worst = collapsed
            .iter()
            .map(|c| (c.k_inf - fine.k_inf).abs() / fine.k_inf.abs().max(1e-300))
            .fold(0.0, f64::max);
        out.push(Check {
            name: "collapse conserves k_inf".into(),
            passed: worst <= 1e-9,
            detail: format!("largest relative change {worst:.1e} (exact collapse: rounding only)"),
        });
    }
    // Pointwise, Sigma_f <= Sigma_a <= Sigma_t at every energy and every
    // point, and all three are scored on the same track or collision, so the
    // condensed ratios must obey it exactly in every visited bin. A violation
    // is a mis-wired score, not noise.
    let mut worst = (0.0_f64, String::new());
    for z in &library.zones {
        for g in 0..z.flux.len() {
            if !(z.flux[g] > 0.0) {
                continue;
            }
            let over = (z.fission[g] - z.absorption[g])
                .max(z.absorption[g] - z.total[g])
                / z.total[g].max(1e-300);
            if over > worst.0 {
                worst = (over, format!("'{}' group {g}", z.name));
            }
        }
    }
    out.push(Check {
        name: "fission <= absorption <= total in every visited bin".into(),
        passed: worst.0 <= 1e-12,
        detail: if worst.0 > 0.0 {
            format!("worst excess {:.2e} of Sigma_t in {}", worst.0, worst.1)
        } else {
            "no bin violates it".to_string()
        },
    });
    if let Some(kernel) = library.zones.get(mat::KERNEL) {
        // nu-bar of uranium fission runs from ~2.4 (thermal) to ~5 (20 MeV)
        // pointwise, so every group's nu Sigma_f / Sigma_f must fall inside a
        // bracket of that range. Outside it, Fission or NuFission is scoring
        // the wrong quantity.
        let nus: Vec<f64> = (0..kernel.flux.len())
            .filter(|&g| kernel.fission[g] > 0.0)
            .map(|g| kernel.nu_fission[g] / kernel.fission[g])
            .collect();
        let (lo, hi) = nus
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
        out.push(Check {
            name: "kernel nu-bar is physical in every group".into(),
            passed: !nus.is_empty() && lo >= 2.0 && hi <= 5.5,
            detail: format!("{} groups with fission, nu-bar {lo:.4} to {hi:.4} (bracket 2.0 to 5.5)", nus.len()),
        });
    }
    // The constants' own production / absorption is a second estimate of the
    // eigenvalue, from the tallies rather than the collision estimator. With
    // leakage L per history, k_eff = P / (A + L), so k_inf * A / (A + L) must
    // reproduce k_eff. The two estimators share histories, so 3 sigma of k is
    // a loose bound; a miss beyond it is a tally that scores the wrong rate.
    if let (Some(p), Some(fine)) = (passes.first(), collapsed.last()) {
        let leak = p.leak_vacuum as f64 / p.histories.max(1) as f64;
        // Absorption per history is (1 - leak) of the losses.
        let k_pred = fine.k_inf * (1.0 - leak);
        let miss = k_pred - p.k;
        out.push(Check {
            name: "tallied production/absorption reproduces k_eff".into(),
            passed: miss.abs() <= 3.0 * p.k_sigma,
            detail: format!(
                "k_inf {:.6} x (1 - leakage {leak:.2e}) = {k_pred:.6} vs k_eff {:.6} +/- {:.6}: {:+.0} pcm",
                fine.k_inf,
                p.k,
                p.k_sigma,
                miss * 1e5
            ),
        });
    }
    if let Some(kernel) = library.zones.get(mat::KERNEL) {
        let empty = kernel.flux.iter().filter(|f| !(**f > 0.0)).count();
        out.push(Check {
            name: "the fuel kernel saw flux in every group".into(),
            passed: empty == 0,
            detail: format!(
                "{empty} of {} groups empty in '{}' (an empty group has no constants)",
                kernel.flux.len(),
                kernel.name
            ),
        });
    }
    if let (CaseModel::FullCore { layers: 12 }, Some(p)) = (case.model, passes.first()) {
        let t_k = case.temperature().get::<kelvin>();
        if (t_k - 300.15).abs() < 1e-9 {
            let (k0, s0) = RECORDED_K_N12_T300;
            let comb = (p.k_sigma * p.k_sigma + s0 * s0).sqrt();
            let z = (p.k - k0) / comb;
            out.push(Check {
                name: "k agrees with the 2026-10-07 record (independent seed)".into(),
                passed: z.abs() <= 3.0,
                detail: format!(
                    "{:.6} +/- {:.6} vs {k0:.6} +/- {s0:.6}: {:+.0} pcm, {z:+.2} sigma (within-run sigma; HTR-10's dominance ratio near 1 makes it an underestimate, #495)",
                    p.k,
                    p.k_sigma,
                    (p.k - k0) * 1e5
                ),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mgxs::{GroupStructure, ZoneMgxs};

    fn pass(k: f64, lost: u64) -> PassRecord {
        PassRecord {
            pass: "p".into(),
            k,
            k_sigma: 0.001,
            histories: 100,
            lost_locate: lost,
            leak_vacuum: 0,
            entropy: vec![],
            transport_s: 1.0,
        }
    }

    fn lib(kernel_flux: Vec<f64>) -> MgxsLibrary {
        let n = kernel_flux.len();
        let edges: Vec<f64> = (0..=n).map(|i| 10f64.powi(i as i32)).collect();
        MgxsLibrary {
            groups: GroupStructure::new(edges).expect("ascending"),
            zones: vec![ZoneMgxs {
                name: "kernel".into(),
                flux: kernel_flux,
                total: vec![1.0; n],
                absorption: vec![0.5; n],
                fission: vec![0.2; n],
                nu_fission: vec![0.5; n],
                kappa_fission: vec![0.0; n],
                scatter: vec![vec![0.5 / n as f64; n]; n],
                chi: vec![1.0 / n as f64; n],
                sigma: None,
                scatter_only: Vec::new(),
            }],
        }
    }

    /// The checks report what they see: a clean run passes all of them, and
    /// each defect fails exactly its own check.
    #[test]
    fn checks_fail_on_the_defect_they_name() {
        let case = MgxsCase::find("core_n12_t300").expect("listed");
        let summary = |k: f64| CollapsedSummary {
            structure: "x".into(),
            n_groups: 1,
            k_inf: k,
            max_balance_residual: 0.0,
            weighted_balance_residual: 0.0,
        };
        let good = checks(
            &case,
            &[pass(0.9955, 0), pass(0.9955, 0)],
            &[summary(0.9958), summary(0.9958)],
            &lib(vec![1.0, 2.0]),
        );
        assert_eq!(good.len(), 8);
        assert!(good.iter().all(|c| c.passed), "{good:#?}");

        // Fission above absorption: breaks the ordering and nu-bar (0.83).
        let mut wrong = lib(vec![0.0, 2.0]);
        wrong.zones[0].fission = vec![0.6, 0.6];
        let bad = checks(
            &case,
            &[pass(1.02, 3), pass(1.03, 0)],
            &[summary(1.2), summary(1.3)],
            &wrong,
        );
        assert!(bad.iter().all(|c| !c.passed), "{bad:#?}");
    }

    /// The residual is the largest |relative| one over visited groups.
    #[test]
    fn balance_residual_is_the_worst_group() {
        let mut l = lib(vec![1.0, 1.0]);
        l.zones[0].total = vec![1.0, 1.1];
        // row sum 0.5, absorption 0.5: group 0 balanced, group 1 off by 0.1/1.1.
        let r = max_balance_residual(&l);
        assert!((r - 0.1 / 1.1).abs() < 1e-12, "{r}");
        // Weighted by Sigma_t phi = 1.0 and 1.1: only group 1 is off.
        let w = weighted_balance_residual(&l);
        assert!((w - 1.1 * (0.1 / 1.1) / 2.1).abs() < 1e-12, "{w}");
    }
}
