// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The external neutron source of GeN-Foam's `diffusionNeutronics`, against the real binary (gh:#1863)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, the unmodified binary, serial. Case: the shipped
//! `Tutorials/featureCases/2D_externalSourceDiffusion` (150-cell wedge, two groups, `eigenvalueNeutronics
//! false`, `externalSourceNeutronics true`, `ddtSchemes steadyState`, `laplacianSchemes Gauss linear corrected`,
//! `adjustTimeStep` with `maxPowerVariation 0.01`, `endTime 100`), tight protocol (`writePrecision 17`, every
//! linear solver `1e-14` / `relTol 0`; `scripts/run_external_source.sh`, `scripts/extract_external_source.sh`).
//! Variants: **A** as shipped (walls `fixedValue 0`); **B** `Alltest`'s form (walls `zeroGradient`, a closed
//! medium); **C** B with `ddtSchemes Euler`; **BO** B with the source off (upstream run too).
//!
//! The port reads the same committed bytes (`cases/external_source`, polyMesh gzipped) through
//! `ExternalSourceCase::read` and takes the same time loop (`run_stationary_transient`) with basic-lib's CG at
//! `1e-14`. Compared step by step: `Time`, `deltaT`, `power`, the neutron iteration count and the final
//! neutron residual; at `t = 100` and `t = 10`, `flux0`/`flux1` per cell; the `pTarget` written to
//! `reactorState`.
//!
//! Controls (the comparison must be able to fail): D, port B with the source off against upstream B; E, port A
//! with `uncorrected` against upstream A (the mesh is a non-orthogonal trapezoid); the unmodified port against
//! upstream BO is also compared (a source-free decay) so that the source term is seen to be the only
//! difference between B and BO.
//!
//! Analytic check (case B), independent of the solver: the closed form of the uniform two-group balance with
//! the source, built from the cross sections of the case alone, and `analysis.py`'s relations
//! `N_tot = S + sum nuSigmaF flux = S/(1 - ksrc)`.
//!
//! Results and predictions: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 32".

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::external_source::StationaryRecord;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::external_source_case::ExternalSourceCase;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let to = dst.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &to);
        } else if e.path().extension().is_some_and(|x| x == "gz") {
            let name = e.path().file_stem().expect("stem").to_owned();
            std::fs::write(dst.join(name), gunzip(&e.path())).expect("write");
        } else {
            std::fs::copy(e.path(), &to).expect("copy");
        }
    }
}

fn replace_in(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(text.contains(from), "{} does not contain {from:?}", path.display());
    std::fs::write(path, text.replacen(from, to, 1)).expect("write");
}

/// `Alltest`'s `foamDictionary ... -entry boundaryField.walls.type -set zeroGradient`: the `walls` block of a
/// boundaryField becomes `type zeroGradient;`.
fn zero_gradient_walls(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let at = text.find("walls").expect("walls patch");
    let open = at + text[at..].find('{').expect("{");
    let close = open + text[open..].find('}').expect("}");
    let edited = format!("{}{{ type zeroGradient; {}", &text[..open], &text[close..]);
    std::fs::write(path, edited).expect("write");
}

/// Stage a variant of the committed case in a scratch directory: `closed` makes the walls `zeroGradient`
/// (`Alltest`'s edit), `euler` selects `ddtSchemes Euler`, `source_off` sets `externalSourceNeutronics false`,
/// `uncorrected` sets `laplacianSchemes Gauss linear uncorrected`.
fn stage(name: &str, closed: bool, euler: bool, source_off: bool, uncorrected: bool) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1863_{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&vv_dir().join("cases/external_source"), &dst);
    if closed {
        for f in ["defaultFlux", "defaultFlux2", "defaultExternalSourceFlux"] {
            zero_gradient_walls(&dst.join("0/neutroRegion").join(f));
        }
    }
    if euler {
        replace_in(&dst.join("system/neutroRegion/fvSchemes"), "default         steadyState;", "default         Euler;");
    }
    if source_off {
        replace_in(
            &dst.join("constant/neutroRegion/neutronicsProperties"),
            "externalSourceNeutronics    true;",
            "externalSourceNeutronics    false;",
        );
    }
    if uncorrected {
        replace_in(&dst.join("system/neutroRegion/fvSchemes"), "Gauss linear corrected;", "Gauss linear uncorrected;");
    }
    // the tight protocol of the upstream run: writePrecision and solver tolerances do not affect the port
    dst
}

struct Run {
    history: Vec<StationaryRecord>,
    p_target: Option<f64>,
    flux: Vec<Vec<f64>>,
    volumes: Vec<f64>,
    case: ExternalSourceCase,
}

fn run_port(dir: &Path) -> Run {
    let mut case = ExternalSourceCase::read(dir, SolverSettings { tolerance: 1e-14, max_iter: 100_000 })
        .expect("read the case");
    let clock = case.clock;
    let step = case.step;
    let (history, p_target) = case.model.run_stationary_transient(&clock, &step).expect("run");
    let flux = case.model.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
    let volumes = case.mesh.cell_volumes.clone();
    Run { history, p_target, flux, volumes, case }
}

#[derive(Default)]
struct Ref {
    time: Vec<f64>,
    dt: Vec<f64>,
    power: Vec<f64>,
    niter: Vec<usize>,
    resid: Vec<f64>,
    flux0_100: Vec<f64>,
    flux1_100: Vec<f64>,
    p_target: f64,
}

fn reference(case: &str) -> Ref {
    reference_in("refs/external_source.ref.txt.gz", case)
}

fn reference_in(file: &str, case: &str) -> Ref {
    let text = String::from_utf8(gunzip(&vv_dir().join(file))).expect("utf8");
    let mut cur = String::new();
    let mut r = Ref::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(key) = it.next() else { continue };
        if key == "CASE" {
            cur = it.next().unwrap_or("").to_string();
            continue;
        }
        if cur != case {
            continue;
        }
        let nums: Vec<f64> = it.map(|w| w.parse().expect("number")).collect();
        match key {
            "TIME" => r.time = nums,
            "DT" => r.dt = nums,
            "POWER" => r.power = nums,
            "NITER" => r.niter = nums.iter().map(|x| *x as usize).collect(),
            "RESID" => r.resid = nums,
            "FLUX0_100" => r.flux0_100 = nums,
            "FLUX1_100" => r.flux1_100 = nums,
            "PTARGET" => r.p_target = nums[0],
            _ => {}
        }
    }
    r
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

/// Worst relative differences between port and upstream, and the first step at which a quantity exceeds `tol`.
struct Tally {
    steps: (usize, usize),
    worst_power: f64,
    first_power_over: Option<usize>,
    worst_time: f64,
    worst_dt: f64,
    niter_equal: usize,
    first_niter_differs: Option<usize>,
    worst_resid_first_equal_niter: f64,
    worst_resid_step: usize,
    resid_pair: (f64, f64),
    worst_flux: f64,
    p_target_rel: f64,
}

/// `power_floor`: steps whose upstream power is below it are left out of the power comparison (a source-free
/// decay reaches round-off noise, upstream's own `power` is then negative at 1e-36 W).
fn compare(run: &Run, r: &Ref, tol: f64, power_floor: f64) -> Tally {
    let n = run.history.len().min(r.power.len());
    let mut t = Tally {
        steps: (run.history.len(), r.power.len()),
        worst_power: 0.0,
        first_power_over: None,
        worst_time: 0.0,
        worst_dt: 0.0,
        niter_equal: 0,
        first_niter_differs: None,
        worst_resid_first_equal_niter: 0.0,
        worst_resid_step: 0,
        resid_pair: (0.0, 0.0),
        worst_flux: 0.0,
        p_target_rel: f64::NAN,
    };
    for i in 0..n {
        let h = &run.history[i];
        let dp = if r.power[i].abs() > power_floor { rel(h.step.power, r.power[i]) } else { 0.0 };
        if dp > t.worst_power {
            t.worst_power = dp;
        }
        if dp > tol && t.first_power_over.is_none() {
            t.first_power_over = Some(i + 1);
        }
        t.worst_time = t.worst_time.max(rel(h.time, r.time[i]));
        // `deltaT =` printed after step i is the step size of step i + 1: `next_delta_t`
        t.worst_dt = t.worst_dt.max(rel(h.next_delta_t, r.dt[i]));
        if h.step.neutron_iterations == r.niter[i] {
            t.niter_equal += 1;
            // the final residual is a physical number only above the solver noise floor (steady steps end
            // at 1e-13 and below, where two correct solvers differ by O(1) relative)
            if r.resid[i] > 1e-8 {
                let last = *h.step.residual_history.last().expect("residual");
                if rel(last, r.resid[i]) > t.worst_resid_first_equal_niter {
                    t.worst_resid_first_equal_niter = rel(last, r.resid[i]);
                    t.worst_resid_step = i + 1;
                    t.resid_pair = (last, r.resid[i]);
                }
            }
        } else if t.first_niter_differs.is_none() {
            t.first_niter_differs = Some(i + 1);
        }
    }
    let scale0 = r.flux0_100.iter().cloned().fold(0.0, f64::max);
    let scale1 = r.flux1_100.iter().cloned().fold(0.0, f64::max);
    for (c, (u0, u1)) in r.flux0_100.iter().zip(r.flux1_100.iter()).enumerate() {
        t.worst_flux = t.worst_flux.max((run.flux[0][c] - u0).abs() / scale0).max((run.flux[1][c] - u1).abs() / scale1);
    }
    if let Some(p) = run.p_target {
        t.p_target_rel = rel(p, r.p_target);
    }
    t
}

fn print(name: &str, t: &Tally) {
    println!(
        "{name}: steps port/upstream {:?}; power worst {:.3e} (first > tol at step {:?}); time {:.2e}; next dt {:.2e}; \
         iterations equal at {}/{} (first difference {:?}); final residual (steps with upstream residual > 1e-8) {:.2e}; \
         flux(t=100) worst/max {:.3e}; pTarget {:.2e}; worst residual at step {} (port, upstream) = {:?}",
        t.steps,
        t.worst_power,
        t.first_power_over,
        t.worst_time,
        t.worst_dt,
        t.niter_equal,
        t.steps.0.min(t.steps.1),
        t.first_niter_differs,
        t.worst_resid_first_equal_niter,
        t.worst_flux,
        t.p_target_rel,
        t.worst_resid_step,
        t.resid_pair
    );
}

/// `check_residual`: only for a case whose flux is not spatially uniform. In B and C the whole solution is
/// uniform (closed medium, uniform source and start flux), OpenFOAM's residual normalisation
/// `sum(|Ax - A xRef| + |b - A xRef|)` then divides a round-off remainder by a round-off remainder, and the final
/// `neutroResidual` is noise (measured 0.05 upstream against 0.56 here at step 65 of B, with the power equal to
/// 1e-13): both codes are above the 1e-6 threshold, so the iteration counts agree, but the number is not data.
fn gate(name: &str, run: &Run, r: &Ref, check_residual: bool) {
    let t = compare(run, r, 1e-9, 0.0);
    print(name, &t);
    assert_eq!(t.steps.0, t.steps.1, "{name}: number of steps");
    // `Time =` of the log is `timeName()`: `timePrecision 8` significant digits, so <= 5e-8 relative by print
    assert!(t.worst_time < 1e-7, "{name}: time {:e}", t.worst_time);
    let mut clock = 0.0;
    for h in &run.history {
        clock += h.delta_t;
        assert_eq!(h.time, clock, "{name}: the port's own time is the running sum of its steps");
    }
    assert!(!check_residual || t.worst_resid_first_equal_niter < 1e-6, "{name}: final neutroResidual {:e}", t.worst_resid_first_equal_niter);
    assert!(t.worst_dt < 1e-10, "{name}: dt {:e}", t.worst_dt);
    assert_eq!(t.niter_equal, t.steps.0, "{name}: neutron iteration counts differ first at {:?}", t.first_niter_differs);
    assert!(t.worst_power < 1e-9, "{name}: power {:e} first over at {:?}", t.worst_power, t.first_power_over);
    assert!(t.worst_flux < 1e-9, "{name}: final flux {:e}", t.worst_flux);
    assert!(t.p_target_rel < 1e-9, "{name}: pTarget {:e}", t.p_target_rel);
    let last = run.history.last().expect("history").step.power;
    assert_eq!(run.p_target, Some(last), "{name}: pTarget is the final power() bit for bit");
}

#[test]
fn a_shipped_case_fixed_value_walls_matches_upstream() {
    let run = run_port(&stage("a", false, false, false, false));
    gate("A", &run, &reference("A"), true);
}

#[test]
fn b_closed_medium_matches_upstream_and_the_analytic_balance() {
    let run = run_port(&stage("b", true, false, false, false));
    let r = reference("B");
    gate("B", &run, &r, false);

    // Analytic check, from the cross sections of the case alone (cell 0 of the uniform zone).
    let xs = run.case.model.xs_fields();
    let ext = run.case.model.external_source().expect("source");
    let at = |v: &outram_foam_basic_lib::prelude::VolScalarField| v.internal.as_slice()[0];
    let beta = at(&xs.beta_tot);
    let chi = |g: usize| (1.0 - beta) * at(&xs.chi_prompt[g]) + beta * at(&xs.chi_delayed[g]);
    let (nf0, nf1) = (at(&xs.nu_sigma_f[0]), at(&xs.nu_sigma_f[1]));
    let (r0, r1) = (at(&xs.sigma_removal[0]), at(&xs.sigma_removal[1]));
    let (s01, s10) = (at(&xs.scattering[0][1]), at(&xs.scattering[1][0]));
    let (e0, e1) = (ext.flux[0][0], ext.flux[1][0]);
    // r0 p0 - s10 p1 = chi0 F + e0 ; r1 p1 - s01 p0 = chi1 F + e1 ; F = nf0 p0 + nf1 p1
    let a11 = r0 - chi(0) * nf0;
    let a12 = -s10 - chi(0) * nf1;
    let a21 = -s01 - chi(1) * nf0;
    let a22 = r1 - chi(1) * nf1;
    let det = a11 * a22 - a12 * a21;
    let p0 = (e0 * a22 - a12 * e1) / det;
    let p1 = (a11 * e1 - a21 * e0) / det;
    let fission = nf0 * p0 + nf1 * p1;
    let k_src = fission / (fission + e0 + e1);
    let v: f64 = run.volumes.iter().sum();
    let power_closed_form = (p0 + p1) * v;

    // the port's final integrals
    let integral = |g: usize| -> f64 { run.flux[g].iter().zip(run.volumes.iter()).map(|(f, vol)| f * vol).sum() };
    let (i0, i1) = (integral(0), integral(1));
    let (s0, s1) = (ext.group_integral(0, &run.volumes), ext.group_integral(1, &run.volumes));
    let n_prod_fiss = chi(0) * (nf0 * i0 + nf1 * i1) + chi(1) * (nf0 * i0 + nf1 * i1);
    let n_prod_scatt = s10 * i1 + s01 * i0;
    let n_abs = r0 * i0 + r1 * i1;
    let eq0 = chi(0) * (nf0 * i0 + nf1 * i1) - r0 * i0 + s10 * i1 + s0;
    let eq1 = chi(1) * (nf0 * i0 + nf1 * i1) - r1 * i1 + s01 * i0 + s1;
    let k_src_run = n_prod_fiss / (n_prod_fiss + s0 + s1);
    let k_bal = n_prod_fiss / (n_abs - n_prod_scatt);
    let n_tot = n_prod_fiss + s0 + s1;
    let final_power = run.history.last().expect("history").step.power;
    println!(
        "B analytic: closed-form power {power_closed_form:.12e} port {final_power:.12e} (rel {:.2e}); upstream expects 53214.3; \
         ksrc closed form {k_src:.12} run {k_src_run:.12} balance k {k_bal:.12}; eq0 {eq0:.2e} eq1 {eq1:.2e} (flux scale {:.3e}); \
         N_tot {n_tot:.9e} S/(1-ksrc) {:.9e}; S0 {s0:.6} S1 {s1:.6}",
        rel(final_power, power_closed_form),
        i0 + i1,
        (s0 + s1) / (1.0 - k_src)
    );
    assert!((final_power - 53214.3).abs() / 53214.3 < 1e-6, "upstream's Alltest value 53214.3 is 6 digits");
    assert!(rel(final_power, power_closed_form) < 1e-9, "closed form");
    assert!(rel(k_src_run, k_src) < 1e-9);
    assert!(eq0.abs() / (i0 + i1) < 1e-9 && eq1.abs() / (i0 + i1) < 1e-9);
    assert!(rel(n_tot, (s0 + s1) / (1.0 - k_src)) < 1e-9);
    // approach from below: the start flux 1e3 is under the steady one
    let first = run.history[0].step.power;
    assert!(first < final_power);
}

#[test]
fn c_euler_ddt_transient_matches_upstream() {
    let run = run_port(&stage("c", true, true, false, false));
    gate("C", &run, &reference("C"), false);
}

#[test]
fn control_d_source_off_differs_from_upstream_with_source() {
    let off = run_port(&stage("d", true, false, true, false));
    let r = reference("B");
    let t = compare(&off, &r, 1e-9, 0.0);
    print("D (port source off vs upstream B)", &t);
    let step1 = rel(off.history[0].step.power, r.power[0]);
    let end = off.history.last().expect("history").step.power / r.power.last().copied().expect("power");
    println!("D: step-1 relative difference {step1:.3e}; final port/upstream power {end:.3e}");
    assert!(step1 > 0.1);
    assert!(end < 1e-3);
    assert!(t.worst_power > 0.1);
    assert!(off.p_target.is_none(), "no pTarget is written with the source off");

    // and the source-off port IS upstream's source-off run, so the source term is the only difference
    let bo = reference("BO");
    let t_bo = compare(&off, &bo, 1e-9, 1e-12);
    print("D2 (port source off vs upstream BO, steps with upstream power > 1e-12 W)", &t_bo);
    assert_eq!(t_bo.steps.0, t_bo.steps.1);
    assert!(t_bo.worst_power < 1e-9, "the source-free decay above the noise floor: {:e}", t_bo.worst_power);
}

#[test]
fn control_e_uncorrected_laplacian_differs_from_upstream_corrected() {
    let run = run_port(&stage("e", false, false, false, true));
    let r = reference("A");
    let t = compare(&run, &r, 1e-9, 0.0);
    print("E (port uncorrected vs upstream A)", &t);
    let end = rel(run.history.last().expect("history").step.power, r.power.last().copied().expect("power"));
    println!("E: final power relative difference {end:.3e}");
    assert!(t.worst_power > 1e-6 || t.steps.0 != t.steps.1, "the correction must matter on this mesh");
}

#[test]
fn the_source_is_refused_for_sp3_and_sn_and_with_circulating_fuel_by_name() {
    use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::external_source::refuse_external_source_for_transport;
    assert!(refuse_external_source_for_transport("SP3Neutronics", true).is_err());
    assert!(refuse_external_source_for_transport("SNNeutronics", true).is_err());
    // an SP3 region selected in the case's controlDict is refused by the reader
    let dir = stage("sp3_refused", true, false, false, false);
    replace_in(&dir.join("system/controlDict"), "neutroRegion     diffusionNeutronics;", "neutroRegion     SP3Neutronics;");
    let err = ExternalSourceCase::read(&dir, SolverSettings { tolerance: 1e-14, max_iter: 1000 }).expect_err("refused");
    assert!(err.to_string().contains("SP3Neutronics"), "{err}");
}

// ---------------------------------------------------------------------------------------------------------
// Round 32b (gh:#1863): `externalSourceModulationTimeProfile` of `constant/<region>/externalSource`.
//
// Methodology: variants F and G of `README.md` "Round 32b" (`cases/external_source_profile/`, upstream run by
// `scripts/run_external_source_profile.sh`, reference `refs/external_source_profile.ref.txt.gz`), both with
// `ddtSchemes Euler`. F: closed medium (round 32's B), inline `table table (...)` with `startTime 5`, so the
// modulation is 1 until t = 20, 3 on [40, 60], 0.5 from t = 80. G: shipped walls (round 32's A, non-uniform
// flux, `corrected` laplacian), the `values (...)` form, no `startTime`. The port reads the same bytes through
// `ExternalSourceCase::read`. Gate: the round 32 gate (`gate`): step counts, `deltaT`, iteration counts, power
// <= 1e-9 relative at every step, flux at t = 100, `pTarget`. Controls: H, the file ignored; I, `startTime`
// ignored. Results: README "Round 32b".

/// Index of the first step whose `Time` (printed to 8 digits) is above `t` by more than the print.
fn first_step_after(r: &Ref, t: f64) -> usize {
    r.time.iter().position(|x| *x > t + 1e-6).expect("a later step")
}

fn reference_profile(case: &str) -> Ref {
    reference_in("refs/external_source_profile.ref.txt.gz", case)
}

/// Variant F (`g = false`) or G (`g = true`) of round 32b staged in a scratch directory.
fn stage_profile(name: &str, g: bool) -> PathBuf {
    let dir = stage(name, !g, true, false, false);
    let file = if g { "externalSource_G" } else { "externalSource_F" };
    std::fs::copy(
        vv_dir().join("cases/external_source_profile").join(file),
        dir.join("constant/neutroRegion/externalSource"),
    )
    .expect("copy externalSource");
    dir
}

#[test]
fn f_inline_table_profile_with_start_time_matches_upstream() {
    let t0 = std::time::Instant::now();
    let run = run_port(&stage_profile("f", false));
    let r = reference_profile("F");
    gate("F (profile)", &run, &r, false);
    let wall = t0.elapsed().as_secs_f64();
    let steady_b = 53214.292175917675;
    let last = run.history.last().expect("history").step.power;
    let max = run.history.iter().map(|h| h.step.power).fold(0.0, f64::max);
    println!("F: final power {last:.6e} = {:.5} x B steady; max power {max:.6e}; wall {wall:.1} s", last / steady_b);
    assert!(last / steady_b > 0.49 && last / steady_b < 0.52, "R5 final ratio");
    assert!(max > 1.45e5 && max < 1.65e5, "R5 max power");

    // R4, upstream against upstream: F and round 32's C (the same case without the profile) are the same run
    // while the modulation is 1, i.e. up to t = 20, and differ after it
    let c = reference("C");
    let mut first_diff = None;
    let mut worst_before = 0.0_f64;
    for i in 0..c.power.len().min(r.power.len()) {
        let d = rel(r.power[i], c.power[i]);
        if r.time[i] < 19.999 {
            worst_before = worst_before.max(d);
        } else if d > 1e-6 && first_diff.is_none() {
            first_diff = Some(i);
        }
    }
    let at = first_diff.expect("F must leave C");
    println!(
        "R4: upstream F vs upstream C, worst relative power difference before t = 20: {worst_before:.2e}; \
         first step over 1e-6 at Time = {} (step {})",
        r.time[at],
        at + 1
    );
    assert!(worst_before < 1e-12);
    assert_eq!(at, first_step_after(&r, 20.0), "the first step with Time > 20");
}

#[test]
fn g_values_form_profile_with_non_uniform_flux_matches_upstream() {
    let run = run_port(&stage_profile("g", true));
    let r = reference_profile("G");
    gate("G (profile)", &run, &r, true);
    let a_final = 18709.241363204968;
    let last = run.history.last().expect("history").step.power;
    println!("G: final power {last:.6e} = {:.5} x A steady", last / a_final);
    assert!(last / a_final > 0.49 && last / a_final < 0.52, "R6 final ratio");
}

#[test]
fn control_h_profile_ignored_differs_from_upstream() {
    let dir = stage_profile("h", false);
    std::fs::remove_file(dir.join("constant/neutroRegion/externalSource")).expect("rm");
    let run = run_port(&dir);
    let r = reference_profile("F");
    let t = compare(&run, &r, 1e-6, 0.0);
    print("H (port profile ignored vs upstream F)", &t);
    let first = t.first_power_over.expect("must differ");
    println!("H: first step over 1e-6 at Time = {}", r.time[first - 1]);
    let end = rel(run.history.last().expect("history").step.power, *r.power.last().expect("power"));
    println!("H: final power relative difference {end:.3e}");
    assert_eq!(first - 1, first_step_after(&r, 20.0), "the first step with Time > 20");
    assert!(end > 0.5);
}

#[test]
fn control_i_start_time_ignored_differs_from_upstream() {
    let dir = stage_profile("i", false);
    replace_in(&dir.join("constant/neutroRegion/externalSource"), "startTime   5;", "startTime   0;");
    let run = run_port(&dir);
    let r = reference_profile("F");
    let t = compare(&run, &r, 1e-6, 0.0);
    print("I (port startTime 0 vs upstream F)", &t);
    let first = t.first_power_over.expect("must differ");
    println!("I: first step over 1e-6 at Time = {}", r.time[first - 1]);
    let end = rel(run.history.last().expect("history").step.power, *r.power.last().expect("power"));
    println!("I: final power relative difference {end:.3e}");
    assert_eq!(first - 1, first_step_after(&r, 15.0), "the first step with Time > 15");
    assert!(end < 1e-2);
}

#[test]
fn the_unported_profile_forms_are_refused_by_name_when_the_case_is_read() {
    let lin = SolverSettings { tolerance: 1e-14, max_iter: 1000 };
    for (edit_from, edit_to, word) in [
        ("type        table;", "type        fmi;", "fmi"),
        ("type        table;", "type        sine;", "sine"),
        ("startTime   5;", "startTime   5; outOfBounds repeat;", "repeat"),
    ] {
        let dir = stage_profile("refused", false);
        replace_in(&dir.join("constant/neutroRegion/externalSource"), edit_from, edit_to);
        let err = ExternalSourceCase::read(&dir, lin).expect_err("refused");
        assert!(err.to_string().contains(word), "{err}");
    }
    // and the same case without the file reads, with modulation 1
    let dir = stage_profile("no_file", false);
    std::fs::remove_file(dir.join("constant/neutroRegion/externalSource")).expect("rm");
    let case = ExternalSourceCase::read(&dir, lin).expect("reads");
    assert!(case.model.external_source().expect("source").modulation.is_none());
}
