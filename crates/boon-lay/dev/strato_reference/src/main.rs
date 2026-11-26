// SPDX-License-Identifier: AGPL-3.0-only
//
//! Reference-data generator for boon-lay's STRATO verification
//! (`crates/boon-lay/verification_and_validation/strato_triso_diffusion/PLAN.md`).
//!
//! Calls ONLY the public library API of UNMODIFIED upstream STRATO, the work
//! of Wee Zhern Ray (GitHub `weewzr`), <https://github.com/weewzr/STRATO>,
//! AGPL-3.0-only, commit `36f1a98848bec9d66fe515994e075cff09d6cefb`:
//!
//! - `strato::analytical::sturm_liouville::{Layer, ModalRelease}` (Code C);
//! - `strato::numerical::finite_volume::{build_mesh, run}` (Code B);
//! - `strato::config::{Config, RegionConfig, InitialConfig, TimeConfig, OutputConfig}`.
//!
//! Every floating-point number is written with `{:.17e}`, which round-trips
//! an `f64` exactly. Usage: `cargo run --release -- <output directory>`.
//!
//! Exempt from the workspace test-reach rule as `main`/CLI glue: it is a
//! standalone, non-member harness whose outputs are checked by boon-lay's
//! tests.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use strato::analytical::sturm_liouville::{Layer, ModalRelease};
use strato::config::{Config, InitialConfig, OutputConfig, RegionConfig, TimeConfig};
use strato::numerical::finite_volume::{build_mesh, run};

/// Upstream's `postprocessing::comparison::MAX_MODES` (20 000).
const MAX_MODES: usize = 20_000;

/// G5 outer radii (m): kernel, buffer, IPyC, SiC, OPyC.
const R5: [f64; 5] = [2.0e-4, 3.0e-4, 3.5e-4, 4.0e-4, 4.5e-4];

/// D5 diffusivities (m^2/s), STRATO's synthetic five-layer case.
const D5: [f64; 5] = [1.0e-12, 4.0e-10, 8.0e-13, 2.0e-14, 7.0e-13];

/// Output times (s) of the standard cases.
const TIMES: [f64; 4] = [2.0e3, 1.0e4, 5.0e4, 2.0e5];

const REGION_NAMES: [&str; 5] = ["kernel", "buffer", "ipyc", "sic", "opyc"];

/// Upstream's Python oracle, `tests/finite_volume_vs_analytical.rs:29`
/// (value quoted at line 26), STRATO commit 36f1a98.
const ORACLE_F_2E5: f64 = 0.4325556390930383;

fn layers(radii: &[f64], diffusivities: &[f64]) -> Vec<Layer> {
    radii
        .iter()
        .zip(diffusivities)
        .map(|(&r, &d)| Layer {
            outer_radius: r,
            diffusivity: d,
        })
        .collect()
}

fn e17(x: f64) -> String {
    format!("{x:.17e}")
}

fn write_file(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, body).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
}

/// One Sturm-Liouville case: meta, modes and release files.
fn sl_case(
    dir: &Path,
    id: &str,
    layer_set: &[Layer],
    lambda_max: f64,
    times: &[f64],
    extra_meta: &[(&str, f64)],
) -> f64 {
    let series = ModalRelease::new(layer_set, lambda_max, MAX_MODES)
        .unwrap_or_else(|e| panic!("ModalRelease::new for {id}: {e}"));

    let mut meta = String::from("key,value\n");
    let _ = writeln!(meta, "lambda_max,{}", e17(lambda_max));
    let _ = writeln!(meta, "max_modes,{MAX_MODES}");
    let _ = writeln!(meta, "n_modes,{}", series.eigenvalues.len());
    let _ = writeln!(meta, "lambda_covered,{}", e17(series.lambda_covered));
    let _ = writeln!(meta, "min_flux_ratio,{}", e17(series.min_flux_ratio));
    let _ = writeln!(meta, "accuracy_floor,{}", e17(series.accuracy_floor()));
    let _ = writeln!(meta, "min_valid_time,{}", e17(series.min_valid_time()));
    for (k, v) in extra_meta {
        let _ = writeln!(meta, "{k},{}", e17(*v));
    }
    write_file(dir, &format!("sl_{id}_meta.csv"), &meta);

    let mut modes = String::from("n,eigenvalue_per_s,weight\n");
    for (n, (l, w)) in series.eigenvalues.iter().zip(&series.weights).enumerate() {
        let _ = writeln!(modes, "{},{},{}", n + 1, e17(*l), e17(*w));
    }
    write_file(dir, &format!("sl_{id}_modes.csv"), &modes);

    // Upstream's `release_fraction` returns a plain f64 at every t (only the
    // convenience `exact_release` maps t < min_valid_time to None), so the
    // value is written as returned; `min_valid_time` is in the meta file.
    let mut rel = String::from("t_s,release_fraction\n");
    let mut last = f64::NAN;
    for &t in times {
        last = series.release_fraction(t);
        let _ = writeln!(rel, "{},{}", e17(t), e17(last));
    }
    write_file(dir, &format!("sl_{id}_release.csv"), &rel);
    println!(
        "sl_{id}: {} modes, lambda_covered {:.3e}, F(t_last) {last:.17e}",
        series.eigenvalues.len(),
        series.lambda_covered
    );
    last
}

fn t_min(times: &[f64]) -> f64 {
    times.iter().copied().fold(f64::INFINITY, f64::min)
}

/// tau = R1^2/(15 D1) + sum over coating layers l = 2..N of (Ro^2 - Ri^2)/(6 D_l).
fn mean_exit_time(layer_set: &[Layer]) -> f64 {
    let r1 = layer_set[0].outer_radius;
    let mut tau = r1 * r1 / (15.0 * layer_set[0].diffusivity);
    for pair in layer_set.windows(2) {
        let (ri, ro) = (pair[0].outer_radius, pair[1].outer_radius);
        tau += (ro * ro - ri * ri) / (6.0 * pair[1].diffusivity);
    }
    tau
}

fn fv_config(case: &str, diffusivities: &[f64; 5], cells: usize, dt: f64) -> Config {
    Config {
        case_name: case.to_string(),
        species: "synthetic".to_string(),
        temperature_k: 1000.0,
        regions: REGION_NAMES
            .iter()
            .zip(R5.iter().zip(diffusivities))
            .map(|(name, (&r, &d))| RegionConfig {
                name: name.to_string(),
                outer_radius_m: r,
                diffusivity_m2_s: d,
                cells,
            })
            .collect(),
        initial: InitialConfig {
            kernel_concentration_mol_m3: 1.0,
        },
        time: TimeConfig {
            dt_s: dt,
            end_time_s: 2.0e5,
            output_every_steps: 1,
        },
        output: OutputConfig {
            directory: "unused".to_string(),
        },
    }
}

fn fv_case(dir: &Path, case: &str, diffusivities: &[f64; 5], cells: usize, dt: f64) -> f64 {
    let cfg = fv_config(case, diffusivities, cells, dt);
    cfg.validate()
        .unwrap_or_else(|e| panic!("Config::validate {case}: {e}"));
    let start = Instant::now();
    let mesh = build_mesh(&cfg).unwrap_or_else(|e| panic!("build_mesh {case}: {e}"));
    let (series, _final_c) = run(&cfg, &mesh).unwrap_or_else(|e| panic!("run {case}: {e}"));
    let wall = start.elapsed().as_secs_f64();

    let mut out = String::from(
        "t_s,release_fraction,inventory,cumulative_release,balance_residual,min_concentration\n",
    );
    for &t in &TIMES {
        let row = series
            .iter()
            .find(|(ts, _)| ((ts - t) / t).abs() <= 1e-6)
            .unwrap_or_else(|| panic!("{case} c{cells} dt{dt}: no output row at t = {t}"));
        let d = &row.1;
        let _ = writeln!(
            out,
            "{},{},{},{},{},{}",
            e17(row.0),
            e17(d.release_fraction),
            e17(d.inventory),
            e17(d.cumulative_release),
            e17(d.balance_residual),
            e17(d.min_concentration)
        );
    }
    write_file(
        dir,
        &format!("fv_{case}_c{cells}_dt{}.csv", dt as u64),
        &out,
    );
    println!("fv_{case}_c{cells}_dt{}: {wall:.3} s", dt as u64);
    wall
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: strato-reference <output directory>"),
    );
    fs::create_dir_all(&dir).expect("creating output directory");
    let t0 = Instant::now();

    // 1. Sturm-Liouville cases.
    let lm = 45.0 / t_min(&TIMES);
    sl_case(&dir, "a1_homogeneous", &layers(&R5, &[1.0e-12; 5]), lm, &TIMES, &[]);
    sl_case(
        &dir,
        "a2a_two_layer",
        &layers(&[2.0e-4, 4.5e-4], &[1.0e-12, 1.0e-14]),
        lm,
        &TIMES,
        &[],
    );
    sl_case(
        &dir,
        "a2b_two_layer_reversed",
        &layers(&[2.0e-4, 4.5e-4], &[1.0e-14, 1.0e-12]),
        lm,
        &TIMES,
        &[],
    );
    let a3_f = sl_case(&dir, "a3_g5d5", &layers(&R5, &D5), lm, &TIMES, &[]);

    let rhos: [(f64, &str); 6] = [
        (1.0e-6, "m6"),
        (1.0e-4, "m4"),
        (1.0e-2, "m2"),
        (1.0e2, "p2"),
        (1.0e4, "p4"),
        (1.0e6, "p6"),
    ];
    for (rho, tag) in rhos {
        let mut d = [1.0e-12; 5];
        d[3] = 1.0e-12 * rho;
        let ls = layers(&R5, &d);
        let tau = mean_exit_time(&ls);
        let times = [0.5 * tau, tau, 2.0 * tau];
        sl_case(
            &dir,
            &format!("a4_rho_{tag}"),
            &ls,
            45.0 / t_min(&times),
            &times,
            &[("mean_exit_time_s", tau)],
        );
    }

    for m in [10u32, 20, 30, 45, 60] {
        sl_case(
            &dir,
            &format!("a7_modes_m{m}"),
            &layers(&R5, &D5),
            f64::from(m) / 2.0e3,
            &TIMES,
            &[],
        );
    }

    // 2. Finite-volume runs (upstream Code B).
    let mut timing = String::from("case,cells,dt,wall_s\n");
    let fv_cases: [(&str, [f64; 5]); 2] = [("a1_homogeneous", [1.0e-12; 5]), ("a3_g5d5", D5)];
    for (case, d) in &fv_cases {
        for cells in [10usize, 20, 40, 80, 160] {
            for dt in [500.0, 250.0] {
                let wall = fv_case(&dir, case, d, cells, dt);
                let _ = writeln!(timing, "{case},{cells},{},{wall:.6}", dt as u64);
            }
        }
    }
    write_file(&dir, "fv_timing.csv", &timing);

    // 3. Upstream Python oracle.
    let oracle = format!(
        "case,t_s,release_fraction,source\na3_g5d5,{},{},STRATO@36f1a98 tests/finite_volume_vs_analytical.rs:29\n",
        e17(2.0e5),
        e17(ORACLE_F_2E5)
    );
    write_file(&dir, "oracle.csv", &oracle);

    println!(
        "a3_g5d5 F(2e5) = {a3_f:.17e}; oracle {ORACLE_F_2E5:.17e}; |diff| = {:.3e}",
        (a3_f - ORACLE_F_2E5).abs()
    );
    println!("total {:.3} s", t0.elapsed().as_secs_f64());
}
