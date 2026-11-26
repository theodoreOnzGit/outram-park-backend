// SPDX-License-Identifier: AGPL-3.0-only
//! # STRATO integration: TRISO multilayer diffusion verification (gh:#1800)
//!
//! Runs the predeclared matrix in
//! `verification_and_validation/strato_triso_diffusion/PLAN.md`. **Every gate
//! here is copied from that plan, which was committed before this file
//! existed**; results and their interpretation are recorded in that folder's
//! `README.md`.
//!
//! STRATO is the work of **Wee Zhern Ray (`weewzr`)**,
//! <https://github.com/weewzr/STRATO> (AGPL-3.0-only, commit `36f1a98`). Its
//! Sturm-Liouville solver is ported as `boon_lay::strato`. Its finite-volume
//! solver is run **unmodified, as upstream code**, by
//! `dev/strato_reference/`, whose output files are read from
//! `tests/data/strato_reference/`.
//!
//! **What is compared with what:**
//! - `U1`: the port against upstream STRATO's own Sturm-Liouville output.
//! - `U2`: the port against STRATO's Python oracle value.
//! - `A1`: the port against a homogeneous-sphere closed form, written below
//!   from `u_n = sin(n pi r / R)` and independent of every implementation.
//! - `A2` to `A7`: boon-lay's multilayer finite volume
//!   (`triso_atops_extensions::multilayer_diffusion`) against the port.
//! - `B1`: STRATO's FV against boon-lay's FV, each converging on the port.
//! - `B2`: the ported TRISO-ATOPS `booth_longlived` against the port's
//!   one-sphere adapter.
//! - `B3` and `B4`: BLOCKED, as the plan states. They are not tested.
//!
//! ```bash
//! cargo test --release -p boon-lay --test strato_triso_diffusion_verification -- --nocapture --test-threads=1
//! ```
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`). Numerical
//! agreement here is verification, not validation.

use std::path::PathBuf;
use std::time::Instant;

use boon_lay::strato::sturm_liouville::{Layer, ModalRelease};
use boon_lay::triso_atops_extensions::multilayer_diffusion::{LayeredDiffusion, LayeredSphere, SphereLayer};
use boon_lay::triso_atops_fork::release_models::steady_state::booth_longlived;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Frequency, Length, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;

// ---------------------------------------------------------------- inputs

/// G5 radii (m): kernel, buffer, IPyC, SiC, OPyC (STRATO's synthetic case).
const G5: [f64; 5] = [2.0e-4, 3.0e-4, 3.5e-4, 4.0e-4, 4.5e-4];
/// D5 (m^2/s), synthetic verification inputs, not literature data.
const D5: [f64; 5] = [1e-12, 4e-10, 8e-13, 2e-14, 7e-13];
/// Output times (s) of the A cases.
const TIMES: [f64; 4] = [2e3, 1e4, 5e4, 2e5];
/// End time of the FV runs (s).
const T_END: f64 = 2e5;
/// STRATO's `comparison::MAX_MODES`.
const MAX_MODES: usize = 20_000;
/// Machine epsilon, as the plan states it.
const EPS: f64 = 2.2e-16;

// ---------------------------------------------------------------- gates (from PLAN.md)

const GATE_U1_REL: f64 = 1e-12;
const GATE_U2_ABS: f64 = 1e-10;
const GATE_A1_ABS: f64 = 1e-10;
const GATE_FV_EXTRAP_ABS: f64 = 2e-5;
const GATE_SPACE_ORDER: (f64, f64) = (1.8, 2.2);
const GATE_TIME_ORDER: (f64, f64) = (0.8, 1.2);
const GATE_A4_FLOOR: f64 = 1e-8;
const GATE_A7_CHANGE: f64 = 1e-12;
const GATE_B2_ABS: f64 = 1e-10;

// ---------------------------------------------------------------- helpers

fn layers(d: &[f64], r: &[f64]) -> Vec<Layer> {
    r.iter().zip(d).map(|(&outer_radius, &diffusivity)| Layer { outer_radius, diffusivity }).collect()
}

fn series(d: &[f64], r: &[f64], t_min: f64) -> ModalRelease {
    ModalRelease::new(&layers(d, r), 45.0 / t_min, MAX_MODES).expect("series builds")
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/strato_reference")
}

/// Rows of a reference CSV (header dropped), each split on commas.
fn csv_rows(name: &str) -> Vec<Vec<String>> {
    let path = data_dir().join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines().skip(1).filter(|l| !l.trim().is_empty()).map(|l| l.split(',').map(|s| s.trim().to_string()).collect()).collect()
}

fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("not a number: {s}"))
}

fn meta(id: &str, key: &str) -> f64 {
    csv_rows(&format!("sl_{id}_meta.csv")).into_iter().find(|r| r[0] == key).map(|r| num(&r[1])).unwrap_or_else(|| panic!("{id}: no {key}"))
}

fn rel_diff(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Homogeneous sphere of radius `r`, diffusivity `d`, inventory starting in
/// `r < r1`: the remaining fraction, from `u_n = sin(k_n r)`, `k_n = n pi / R`.
/// Written for this study from the formulas in PLAN.md; shares no code with
/// STRATO or boon-lay. Summed until the terms are below 1e-30 of the first.
fn closed_form_remaining(r1: f64, r: f64, d: f64, t: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let mut sum = 0.0;
    let mut n = 1_u64;
    loop {
        let k = n as f64 * pi / r;
        let i1 = ((k * r1).sin() - k * r1 * (k * r1).cos()) / (k * k);
        let i_n = if n % 2 == 1 { r * r / (n as f64 * pi) } else { -r * r / (n as f64 * pi) };
        let w = 3.0 * i1 * i_n / (r1.powi(3) * (r / 2.0));
        let term = w * (-d * k * k * t).exp();
        sum += term;
        // Stop once the decaying exponential has killed the tail: |w| <= 3 here.
        if (-d * k * k * t).exp() * 3.0 < 1e-30 || n > 10_000_000 {
            break;
        }
        n += 1;
    }
    sum
}

/// One boon-lay FV run: release fraction at each of `times`, with the
/// diagnostics the plan requires.
struct FvRun {
    f_at: Vec<f64>,
    worst_balance: f64,
    balance_bound: f64,
    min_atoms_rel: f64,
    wall_s: f64,
}

fn boon_lay_fv(d: &[f64], r: &[f64], cells: usize, dt: f64, times: &[f64], uniform_layers: usize) -> FvRun {
    let sphere = LayeredSphere::new(
        &r.iter().map(|&x| SphereLayer { outer_radius: Length::new::<meter>(x), cells }).collect::<Vec<_>>(),
    )
    .expect("mesh");
    let dq: Vec<DiffusionCoefficient> = d.iter().map(|&x| DiffusionCoefficient::new::<square_meter_per_second>(x)).collect();
    // Stiffness bound of the conservation gate: max over layers of h D / dr^2.
    let mut inner = 0.0;
    let mut stiff: f64 = 1.0;
    for (&ro, &di) in r.iter().zip(d) {
        let dr = (ro - inner) / cells as f64;
        stiff = stiff.max(dt * di / (dr * dr));
        inner = ro;
    }
    let mut run = LayeredDiffusion::uniform_in_first_layers(sphere, uniform_layers);
    let t0 = Instant::now();
    let mut f_at = Vec::new();
    let (mut worst_balance, mut min_atoms_rel) = (0.0_f64, 0.0_f64);
    let end = times.iter().copied().fold(0.0, f64::max);
    let steps = (end / dt).round() as usize;
    for s in 1..=steps {
        run.step(Time::new::<second>(dt), &dq, Frequency::new::<hertz>(0.0)).expect("step");
        let atoms = run.cell_atoms();
        let inside: f64 = atoms.iter().sum();
        let released = run.released_fraction().get::<ratio>();
        worst_balance = worst_balance.max((1.0 - inside - released).abs());
        let max_a = atoms.iter().copied().fold(0.0, f64::max);
        let min_a = atoms.iter().copied().fold(f64::INFINITY, f64::min);
        if max_a > 0.0 {
            min_atoms_rel = min_atoms_rel.min(min_a / max_a);
        }
        let t = s as f64 * dt;
        if times.iter().any(|&x| rel_diff(x, t) < 1e-9) {
            f_at.push(released);
        }
    }
    assert_eq!(f_at.len(), times.len(), "every output time must be a step boundary");
    FvRun { f_at, worst_balance, balance_bound: EPS * stiff * steps as f64, min_atoms_rel, wall_s: t0.elapsed().as_secs_f64() }
}

fn check_fv_diagnostics(label: &str, run: &FvRun) {
    assert!(run.worst_balance <= run.balance_bound, "{label}: conservation {:.3e} > bound {:.3e}", run.worst_balance, run.balance_bound);
    assert!(run.min_atoms_rel >= -EPS, "{label}: positivity {:.3e}", run.min_atoms_rel);
}

fn hardware() -> String {
    outram_mc_libs::perf_report::HardwareInfo::detect().headline()
}

/// Richardson in time (backward Euler, order 1): 2 F(dt/2) - F(dt).
fn extrap_time(f_dt: f64, f_half: f64) -> f64 {
    2.0 * f_half - f_dt
}

/// Richardson in space (order 2, cells doubled): (4 F(h/2) - F(h)) / 3.
fn extrap_space(f_h: f64, f_half: f64) -> f64 {
    (4.0 * f_half - f_h) / 3.0
}

// ---------------------------------------------------------------- U1, U2

const SL_CASES: [&str; 15] = [
    "a1_homogeneous",
    "a2a_two_layer",
    "a2b_two_layer_reversed",
    "a3_g5d5",
    "a4_rho_m6",
    "a4_rho_m4",
    "a4_rho_m2",
    "a4_rho_p2",
    "a4_rho_p4",
    "a4_rho_p6",
    "a7_modes_m10",
    "a7_modes_m20",
    "a7_modes_m30",
    "a7_modes_m45",
    "a7_modes_m60",
];

/// The layers of an `sl_<id>` reference case, as `dev/strato_reference` builds them.
fn case_layers(id: &str) -> (Vec<f64>, Vec<f64>) {
    let ones = vec![1e-12; 5];
    match id {
        "a1_homogeneous" => (ones, G5.to_vec()),
        "a2a_two_layer" => (vec![1e-12, 1e-14], vec![2.0e-4, 4.5e-4]),
        "a2b_two_layer_reversed" => (vec![1e-14, 1e-12], vec![2.0e-4, 4.5e-4]),
        "a3_g5d5" => (D5.to_vec(), G5.to_vec()),
        _ if id.starts_with("a7_modes") => (D5.to_vec(), G5.to_vec()),
        _ if id.starts_with("a4_rho") => {
            let rho = match &id[7..] {
                "m6" => 1e-6,
                "m4" => 1e-4,
                "m2" => 1e-2,
                "p2" => 1e2,
                "p4" => 1e4,
                "p6" => 1e6,
                other => panic!("unknown rho {other}"),
            };
            let mut d = ones;
            d[3] = 1e-12 * rho;
            (d, G5.to_vec())
        }
        other => panic!("unknown case {other}"),
    }
}

/// U1: the port reproduces upstream STRATO's Sturm-Liouville output.
#[test]
fn u1_port_matches_upstream_sturm_liouville() {
    let (mut worst, mut values, mut identical) = (0.0_f64, 0_usize, 0_usize);
    for id in SL_CASES {
        let (d, r) = case_layers(id);
        let lambda_max = meta(id, "lambda_max");
        let s = ModalRelease::new(&layers(&d, &r), lambda_max, MAX_MODES).expect("series");
        let mut compare = |ours: f64, theirs: f64, what: &str| {
            let rd = rel_diff(ours, theirs);
            values += 1;
            if ours.to_bits() == theirs.to_bits() {
                identical += 1;
            }
            worst = worst.max(rd);
            assert!(rd <= GATE_U1_REL, "{id} {what}: port {ours:.17e} upstream {theirs:.17e} rel {rd:.2e}");
        };
        let modes = csv_rows(&format!("sl_{id}_modes.csv"));
        assert_eq!(modes.len(), s.eigenvalues.len(), "{id}: mode count");
        for (row, (l, w)) in modes.iter().zip(s.eigenvalues.iter().zip(&s.weights)) {
            compare(*l, num(&row[1]), "eigenvalue");
            compare(*w, num(&row[2]), "weight");
        }
        compare(s.lambda_covered, meta(id, "lambda_covered"), "lambda_covered");
        compare(s.accuracy_floor(), meta(id, "accuracy_floor"), "accuracy_floor");
        for row in csv_rows(&format!("sl_{id}_release.csv")) {
            let theirs = num(&row[1]);
            if theirs.is_finite() {
                compare(s.release_fraction(num(&row[0])), theirs, "F(t)");
            }
        }
    }
    println!("U1: {values} values over {} cases, {identical} bit-identical, worst relative difference {worst:.3e} (gate {GATE_U1_REL:.0e})", SL_CASES.len());
}

/// U2: the port against STRATO's independent Python oracle value.
#[test]
fn u2_port_matches_upstream_python_oracle() {
    let row = &csv_rows("oracle.csv")[0];
    let (t, oracle) = (num(&row[1]), num(&row[2]));
    let f = series(&D5, &G5, t).release_fraction(t);
    let diff = (f - oracle).abs();
    println!("U2: F({t:.0e} s) port {f:.16} oracle {oracle:.16} |diff| {diff:.3e} (gate {GATE_U2_ABS:.0e})");
    assert!(diff <= GATE_U2_ABS);
}

// ---------------------------------------------------------------- A1

/// A1: equal diffusivities; the Sturm-Liouville series against the
/// closed form, and boon-lay FV against both.
#[test]
fn a1_homogeneous_sphere_against_closed_form() {
    let d = [1e-12; 5];
    let s = series(&d, &G5, TIMES[0]);
    println!("A1: {} modes, min_valid_time {:.3e} s, accuracy_floor {:.3e}", s.eigenvalues.len(), s.min_valid_time(), s.accuracy_floor());
    println!("| t [s] | closed form F | SL F | abs diff |");
    let mut worst: f64 = 0.0;
    for &t in &TIMES {
        assert!(t >= s.min_valid_time());
        let exact = 1.0 - closed_form_remaining(G5[0], G5[4], 1e-12, t);
        let sl = s.release_fraction(t);
        let e = (sl - exact).abs();
        worst = worst.max(e);
        println!("| {t:.0e} | {exact:.15} | {sl:.15} | {e:.2e} |");
    }
    println!("A1 worst |SL - closed form| = {worst:.3e} (gate {GATE_A1_ABS:.0e})");
    assert!(worst <= GATE_A1_ABS);
    fv_against_reference("A1", &d, &G5, |t| 1.0 - closed_form_remaining(G5[0], G5[4], 1e-12, t));
}

/// The A5 protocol for one case: meshes 10 to 160 cells per layer, each at
/// `dt` and `dt/2`, extrapolated in time; error and order at `T_END` against
/// `reference`; then the spatial extrapolation of the two finest.
/// Returns the extrapolated value and its error.
fn fv_against_reference<F: Fn(f64) -> f64>(label: &str, d: &[f64], r: &[f64], reference: F) -> (f64, f64) {
    let dt = T_END / 400.0;
    let exact = reference(T_END);
    let mut errs = Vec::new();
    let mut f_time_extrap = Vec::new();
    println!("{label} boon-lay FV at t = {T_END:.0e} s (reference {exact:.12}):");
    println!("| cells/layer | F(dt) | F(dt/2) | time-extrapolated | error | order | balance | min c rel | wall [s] |");
    for cells in [10, 20, 40, 80, 160] {
        let a = boon_lay_fv(d, r, cells, dt, &[T_END], 1);
        let b = boon_lay_fv(d, r, cells, dt / 2.0, &[T_END], 1);
        check_fv_diagnostics(&format!("{label} c{cells}"), &a);
        check_fv_diagnostics(&format!("{label} c{cells}/2"), &b);
        let fe = extrap_time(a.f_at[0], b.f_at[0]);
        let e = fe - exact;
        let order = errs.last().map_or(f64::NAN, |&p: &f64| (p.abs() / e.abs()).log2());
        println!(
            "| {cells} | {:.12} | {:.12} | {fe:.12} | {e:+.3e} | {order:.3} | {:.1e} | {:.1e} | {:.2} |",
            a.f_at[0],
            b.f_at[0],
            a.worst_balance.max(b.worst_balance),
            a.min_atoms_rel.min(b.min_atoms_rel),
            a.wall_s + b.wall_s
        );
        errs.push(e);
        f_time_extrap.push(fe);
    }
    let n = errs.len();
    let orders = [(errs[n - 3].abs() / errs[n - 2].abs()).log2(), (errs[n - 2].abs() / errs[n - 1].abs()).log2()];
    let fx = extrap_space(f_time_extrap[n - 2], f_time_extrap[n - 1]);
    let ex = (fx - exact).abs();
    println!("{label}: spatial orders (40->80, 80->160) = {:.3}, {:.3} (gate {:?}); extrapolated F {fx:.12}, |error| {ex:.3e} (gate {GATE_FV_EXTRAP_ABS:.0e})", orders[0], orders[1], GATE_SPACE_ORDER);
    println!("{label}: hardware {}", hardware());
    for o in orders {
        assert!(o >= GATE_SPACE_ORDER.0 && o <= GATE_SPACE_ORDER.1, "{label}: spatial order {o:.3}");
    }
    assert!(ex <= GATE_FV_EXTRAP_ABS, "{label}: extrapolated error {ex:.3e}");
    (fx, ex)
}

// ---------------------------------------------------------------- A2, A3/A5

/// A2a: a 100x diffusivity drop at the kernel boundary.
#[test]
fn a2a_two_layer_fv_against_sturm_liouville() {
    two_layer("A2a", [1e-12, 1e-14]);
}

/// A2b: the same contrast reversed.
#[test]
fn a2b_two_layer_reversed_fv_against_sturm_liouville() {
    two_layer("A2b", [1e-14, 1e-12]);
}

fn two_layer(label: &str, d: [f64; 2]) {
    let r = [2.0e-4, 4.5e-4];
    let s = series(&d, &r, TIMES[0]);
    println!("{label}: {} modes, accuracy_floor {:.3e}", s.eigenvalues.len(), s.accuracy_floor());
    fv_against_reference(label, &d, &r, |t| s.release_fraction(t));
}

/// A3 and A5: the five-layer synthetic TRISO, mesh refinement.
#[test]
fn a3_a5_five_layer_mesh_refinement_against_sturm_liouville() {
    let s = series(&D5, &G5, TIMES[0]);
    println!("A3: {} modes, accuracy_floor {:.3e}, min_valid_time {:.3e} s", s.eigenvalues.len(), s.accuracy_floor(), s.min_valid_time());
    fv_against_reference("A3", &D5, &G5, |t| s.release_fraction(t));
}

// ---------------------------------------------------------------- A4

/// A4: SiC diffusivity contrast sweep.
#[test]
fn a4_diffusivity_contrast_sweep() {
    println!("| rho | mean exit time [s] | modes | accuracy_floor | usable | FV extrapolated error at 2 tau |");
    for rho in [1e-6, 1e-4, 1e-2, 1e2, 1e4, 1e6] {
        let mut d = [1e-12; 5];
        d[3] = 1e-12 * rho;
        let mut tau = G5[0] * G5[0] / (15.0 * d[0]);
        for l in 1..5 {
            tau += (G5[l] * G5[l] - G5[l - 1] * G5[l - 1]) / (6.0 * d[l]);
        }
        let times = [0.5 * tau, tau, 2.0 * tau];
        let s = series(&d, &G5, times[0]);
        let floor = s.accuracy_floor();
        let usable = floor <= GATE_A4_FLOOR;
        let mut worst = f64::NAN;
        if usable {
            // A5 protocol at the three times, with the end time 2 tau.
            let dt = times[2] / 400.0;
            let mut fe = Vec::new();
            for cells in [80, 160] {
                let a = boon_lay_fv(&d, &G5, cells, dt, &times, 1);
                let b = boon_lay_fv(&d, &G5, cells, dt / 2.0, &times, 1);
                check_fv_diagnostics(&format!("A4 rho {rho:e} c{cells}"), &a);
                check_fv_diagnostics(&format!("A4 rho {rho:e} c{cells}/2"), &b);
                fe.push(a.f_at.iter().zip(&b.f_at).map(|(x, y)| extrap_time(*x, *y)).collect::<Vec<_>>());
            }
            worst = 0.0;
            for (k, &t) in times.iter().enumerate() {
                let fx = extrap_space(fe[0][k], fe[1][k]);
                worst = worst.max((fx - s.release_fraction(t)).abs());
            }
        }
        println!("| {rho:.0e} | {tau:.4e} | {} | {floor:.3e} | {usable} | {worst:.3e} |", s.eigenvalues.len());
        assert!(usable, "A4 rho {rho:e}: accuracy floor {floor:.3e} above {GATE_A4_FLOOR:.0e}");
        assert!(worst <= GATE_FV_EXTRAP_ABS, "A4 rho {rho:e}: FV extrapolated error {worst:.3e}");
    }
}

// ---------------------------------------------------------------- A6

/// A6: temporal self-convergence at 160 cells per layer.
#[test]
fn a6_temporal_refinement() {
    let s = series(&D5, &G5, TIMES[0]);
    let mut f = Vec::new();
    println!("| dt [s] | F(T) | F(T) - SL |");
    for div in [50, 100, 200, 400, 800, 1600] {
        let run = boon_lay_fv(&D5, &G5, 160, T_END / div as f64, &[T_END], 1);
        check_fv_diagnostics(&format!("A6 dt T/{div}"), &run);
        println!("| {:.1} | {:.12} | {:+.3e} |", T_END / div as f64, run.f_at[0], run.f_at[0] - s.release_fraction(T_END));
        f.push(run.f_at[0]);
    }
    let orders: Vec<f64> = f.windows(3).map(|w| ((w[0] - w[1]).abs() / (w[1] - w[2]).abs()).log2()).collect();
    println!("A6 temporal self-convergence orders (T/50..T/1600 triples): {orders:.3?} (gate {GATE_TIME_ORDER:?} on the two finest)");
    for &o in &orders[orders.len() - 2..] {
        assert!(o >= GATE_TIME_ORDER.0 && o <= GATE_TIME_ORDER.1, "A6 order {o:.3}");
    }
}

// ---------------------------------------------------------------- A7

/// A7: modal convergence of the series.
#[test]
fn a7_modal_convergence() {
    let l = layers(&D5, &G5);
    let mut prev: Option<(f64, Vec<f64>)> = None;
    println!("| m | modes | lambda_covered [1/s] | min_valid_time [s] | F(2e3) | F(1e4) | F(5e4) | F(2e5) |");
    for m in [10.0, 20.0, 30.0, 45.0, 60.0] {
        let s = ModalRelease::new(&l, m / TIMES[0], MAX_MODES).expect("series");
        let valid = s.min_valid_time();
        let f: Vec<f64> = TIMES.iter().map(|&t| s.release_fraction(t)).collect();
        let shown: Vec<String> = TIMES.iter().zip(&f).map(|(&t, v)| if t >= valid { format!("{v:.15}") } else { "unresolved".into() }).collect();
        println!("| {m} | {} | {:.4e} | {valid:.3e} | {} |", s.eigenvalues.len(), s.lambda_covered, shown.join(" | "));
        if let Some((pvalid, pf)) = &prev {
            for (k, &t) in TIMES.iter().enumerate() {
                if t >= valid && t >= *pvalid {
                    let change = (f[k] - pf[k]).abs();
                    assert!(change <= GATE_A7_CHANGE, "A7 m {m}: F({t:e}) moved {change:.3e}");
                }
            }
        }
        prev = Some((valid, f));
    }
}

// ---------------------------------------------------------------- B1

/// B1: upstream STRATO FV and boon-lay FV, each against the series, on the
/// same meshes and steps.
#[test]
fn b1_strato_fv_against_boon_lay_fv() {
    for (case, d) in [("a1_homogeneous", [1e-12; 5]), ("a3_g5d5", D5)] {
        let s = series(&d, &G5, TIMES[0]);
        let exact = s.release_fraction(T_END);
        println!("B1 {case} at t = {T_END:.0e} s, SL {exact:.12}:");
        println!("| cells/layer | dt [s] | STRATO FV | boon-lay FV | STRATO - SL | boon-lay - SL | STRATO - boon-lay | STRATO balance | STRATO min c |");
        let mut strato_err = Vec::new();
        let mut boon_err = Vec::new();
        for cells in [10, 20, 40, 80, 160] {
            let mut fe = [0.0; 2];
            let mut be = [0.0; 2];
            for (k, dt) in [500.0, 250.0].into_iter().enumerate() {
                let rows = csv_rows(&format!("fv_{case}_c{cells}_dt{}.csv", dt as u64));
                let row = rows.iter().find(|r| rel_diff(num(&r[0]), T_END) < 1e-9).expect("end row");
                let strato = num(&row[1]);
                let ours = boon_lay_fv(&d, &G5, cells, dt, &[T_END], 1);
                check_fv_diagnostics(&format!("B1 {case} c{cells} dt{dt}"), &ours);
                println!(
                    "| {cells} | {dt} | {strato:.12} | {:.12} | {:+.3e} | {:+.3e} | {:+.3e} | {} | {} |",
                    ours.f_at[0],
                    strato - exact,
                    ours.f_at[0] - exact,
                    strato - ours.f_at[0],
                    row[4],
                    row[5]
                );
                fe[k] = strato;
                be[k] = ours.f_at[0];
            }
            strato_err.push(extrap_time(fe[0], fe[1]) - exact);
            boon_err.push(extrap_time(be[0], be[1]) - exact);
        }
        let order = |e: &[f64]| -> [f64; 2] {
            let n = e.len();
            [(e[n - 3].abs() / e[n - 2].abs()).log2(), (e[n - 2].abs() / e[n - 1].abs()).log2()]
        };
        let (os, ob) = (order(&strato_err), order(&boon_err));
        println!("B1 {case}: time-extrapolated errors STRATO {strato_err:.3e?}, boon-lay {boon_err:.3e?}");
        println!("B1 {case}: spatial orders STRATO {os:.3?}, boon-lay {ob:.3?} (gate {GATE_SPACE_ORDER:?})");
        for o in os.into_iter().chain(ob) {
            assert!(o >= GATE_SPACE_ORDER.0 && o <= GATE_SPACE_ORDER.1, "B1 {case}: order {o:.3}");
        }
    }
}

// ---------------------------------------------------------------- B2

/// B2: ATOPS's ported Booth series against the Sturm-Liouville one-sphere
/// adapter and boon-lay FV (one layer, uniform start).
#[test]
fn b2_atops_booth_against_sturm_liouville_single_sphere() {
    let (r, d) = (4.5e-4, 1e-12);
    let taus = [1e-3, 1e-2, 0.1, 1.0];
    let t_min = taus[0] * r * r / d;
    let s = ModalRelease::single_sphere(r, d, 45.0 / t_min, MAX_MODES).expect("one-sphere series");
    println!("B2: {} modes, min_valid_time {:.3e} s", s.eigenvalues.len(), s.min_valid_time());
    println!("| D t / R^2 | ATOPS booth_longlived | SL single_sphere | abs diff |");
    let mut worst: f64 = 0.0;
    for tau in taus {
        let t = tau * r * r / d;
        assert!(t >= s.min_valid_time());
        let booth = booth_longlived(
            DiffusionCoefficient::new::<square_meter_per_second>(d),
            Time::new::<second>(t),
            Length::new::<meter>(r),
        )
        .get::<ratio>();
        let sl = s.release_fraction(t);
        worst = worst.max((booth - sl).abs());
        println!("| {tau:.0e} | {booth:.15} | {sl:.15} | {:.2e} |", (booth - sl).abs());
    }
    println!("B2 worst |booth - SL| = {worst:.3e} (gate {GATE_B2_ABS:.0e})");
    assert!(worst <= GATE_B2_ABS);
}
