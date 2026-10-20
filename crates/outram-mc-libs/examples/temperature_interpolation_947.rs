//! **GitHub #947: how far OpenMC's stochastic temperature interpolation lands
//! from data processed exactly at the temperature.** A measurement, not a
//! gate; the grid decision (#945) is the maintainer's.
//!
//! Methodology, predictions (written before the first run) and results:
//! `crates/outram-mc-libs/verification_and_validation/temperature_interpolation_947/README.md`.
//!
//! Stages (each prints markdown tables):
//!
//! - `pointwise` — U-238, [300, 1500] K: stochastic arms at Δ = 600, 300,
//!   150, 75 K in T, √T and log T, the plug-in `E[σ]` arm and the Chebyshev
//!   fit arms, against data processed at each test temperature: pointwise
//!   error, band integrals, NR self-shielded integrals and the resonance
//!   escape probability of U-238 + C-12 (deterministic slowing down). Add
//!   `--urr` for the unresolved range (PURR at NJOY's 20/64/10 000).
//! - `mc` — the Monte Carlo check of the stochastic slowing-down arm.
//! - `sab` — H in H₂O from the LEAPR deck: σ_sab and a Pu-239 absorption
//!   fraction in an infinite H₂O medium.
//!
//! Run: `cargo run --release -p outram-mc-libs --example
//! temperature_interpolation_947 -- <stage> [--threads N] [--histories N] [--urr] [--sab-bracket I]` (`--urr` adds the unresolved range to `pointwise`; `--sab-bracket` picks which S(α,β) bracket the `sab` stage transports, default 11 = [400, 800] -> 600 K).
//!
//! No test reaches `main` or the stage functions: they are CLI glue around
//! the instruments of `outram_mc_libs::vv::temperature_interpolation_947` and
//! the port `material::temperature_interpolation`, which are tested there and
//! in `tests/temperature_interpolation_947.rs`.

use petir::mathf::RealMath;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, Reconstruction};
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::MtReaction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::material::temperature_interpolation::{
    bracket, chebyshev_nodes_k, chebyshev_weights, InterpolationVariable as Var,
};
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::slowing_down::{
    slowing_down_grid, solve_on_grid, InfiniteMediumMc, MixComponent, ScatterKernel,
    SlowingDownBand, SlowingDownGrid,
};
use outram_mc_libs::rng::lcg::prn;
use outram_mc_libs::vv::temperature_interpolation_947::{
    lethargy_integral, mix_slowing_down_grids, pointwise, slowing_down_grid_from, urr_band_weights,
    PointwiseError, ShieldedIntegrals,
};
use uom::si::thermodynamic_temperature::kelvin;

const T_LO: f64 = 300.0;
const T_HI: f64 = 1500.0;
const FINE: f64 = 37.5;
const SPACINGS: [f64; 4] = [600.0, 300.0, 150.0, 75.0];
const FIT_N: [usize; 4] = [3, 4, 5, 6];
const SIGMA0: [f64; 3] = [10.0, 100.0, 1000.0];
const SIGMA_B: [f64; 2] = [100.0, 1000.0];
const E_RES_HI: f64 = 2.0e4;
const BANDS: [(f64, f64, &str); 5] = [
    (1.0e-5, 1.0, "1e-5-1 eV"),
    (1.0, 10.0, "1-10 eV"),
    (10.0, 100.0, "10-100 eV"),
    (100.0, 1.0e3, "0.1-1 keV"),
    (1.0e3, E_RES_HI, "1-20 keV"),
];
const SD_BAND: SlowingDownBand = SlowingDownBand {
    e_top: 1.0e4,
    e_bot: 1.0,
};
const STUDY_ERRTHN: f64 = 1.0e-4;
const URR_SETTINGS: (usize, usize, usize) = (20, 64, 10_000);

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stage = args.first().cloned().unwrap_or_else(|| "pointwise".into());
    let threads: usize = arg_value(&args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let histories: usize = arg_value(&args, "--histories")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    println!(
        "hardware: {}",
        outram_mc_libs::perf_report::HardwareInfo::detect().headline()
    );
    println!("threads used: {threads}\n");
    match stage.as_str() {
        "pointwise" => stage_pointwise(threads, args.iter().any(|a| a == "--urr")),
        "mc" => stage_mc(threads, histories),
        "sab" => stage_sab(
            threads,
            histories,
            arg_value(&args, "--sab-bracket")
                .and_then(|s| s.parse().ok())
                .unwrap_or(11),
        ),
        other => panic!("unknown stage {other}: pointwise | mc | sab"),
    }
}

/// Run `jobs` on `threads` workers, results in job order.
fn parallel<T: Send, F: Fn(usize) -> T + Sync>(n_jobs: usize, threads: usize, f: F) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<T>>> = Mutex::new((0..n_jobs).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let j = next.fetch_add(1, Ordering::SeqCst);
                if j >= n_jobs {
                    break;
                }
                let r = f(j);
                out.lock().expect("lock")[j] = Some(r);
            });
        }
    });
    out.into_inner()
        .expect("lock")
        .into_iter()
        .map(|x| x.expect("job ran"))
        .collect()
}

fn tape(file: &str) -> (Arc<Tape>, i32) {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing tape {file}"));
    let t = Tape::read_file(&p).expect("tape");
    let mat = t.materials()[0];
    (Arc::new(t), mat)
}

fn settings(errthn: f64) -> CrossSectionSettings {
    CrossSectionSettings::default()
        .without_purr()
        .without_heatr()
        .with_broadr_tolerance(errthn)
}

/// One nuclide at `t_k` from a held reconstruction, through `process` and
/// `Nuclide::from_processed`, with optional URR tables.
fn nuclide_at(
    recon: &Reconstruction,
    name: &str,
    t_k: f64,
    errthn: f64,
    urr: Option<UrrProbabilityTables>,
) -> Nuclide {
    let ncs = process::cross_sections_at(recon, Temperature::new::<kelvin>(t_k), &settings(errthn))
        .expect("process");
    let processed = ProcessedEvaluation {
        recon: ncs.pendf,
        elastic_0k: ncs.elastic_0k.unwrap_or_default(),
        urr,
        temp_k: t_k,
    };
    Nuclide::from_processed(&recon.tape, recon.mat, name, t_k, processed).expect("nuclide")
}

fn key(t: f64) -> i64 {
    (t * 1.0e4).round() as i64
}

/// U-238's numbers at one temperature, on the shared grids.
struct Table {
    total: Vec<f64>,
    capture: Vec<f64>,
    /// U-238 micro (scatter, absorb) on the slowing-down energies.
    sd: Vec<(f64, f64)>,
    /// Per URR energy: per σ0 (N_c, N_t, D), then (mean σc, mean σt).
    urr: Option<Vec<([(f64, f64, f64); 3], f64, f64)>>,
}

fn table_from(n: &Nuclide, t_k: f64, grid: &[f64], sd_e: &[f64]) -> Table {
    let (mut total, mut capture) = (
        Vec::with_capacity(grid.len()),
        Vec::with_capacity(grid.len()),
    );
    for &e in grid {
        let x = n.xs_at_energy(e, t_k);
        total.push(x.total);
        capture.push(x.absorption - x.fission);
    }
    let sd = sd_e
        .iter()
        .map(|&e| {
            let x = n.xs_at_energy(e, t_k);
            (
                ((x.total - x.absorption - x.inelastic - x.n2n).max(0.0)),
                x.absorption,
            )
        })
        .collect();
    Table {
        total,
        capture,
        sd,
        urr: None,
    }
}

fn urr_numbers(
    n: &Nuclide,
    tables: &UrrProbabilityTables,
    t_k: f64,
    energies: &[f64],
) -> Vec<([(f64, f64, f64); 3], f64, f64)> {
    energies
        .iter()
        .map(|&e| {
            let w = urr_band_weights(&tables.band_representatives(e)).expect("band intervals");
            let mut acc = [(0.0, 0.0, 0.0); 3];
            let (mut mc, mut mt) = (0.0, 0.0);
            for (xi, p) in w {
                let x = n.xs_at_energy_urr(e, t_k, xi);
                let (sc, st) = (x.absorption - x.fission, x.total);
                mc += p * sc;
                mt += p * st;
                for (k, s0) in SIGMA0.iter().enumerate() {
                    acc[k].0 += p * sc / (st + s0);
                    acc[k].1 += p * st / (st + s0);
                    acc[k].2 += p / (st + s0);
                }
            }
            (acc, mc, mt)
        })
        .collect()
}

fn mix(a: &[f64], b: &[f64], w0: f64, w1: f64) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| w0 * x + w1 * y).collect()
}

fn pct(x: f64) -> String {
    format!("{:+.4}", 100.0 * x)
}

fn fmt_pw(p: &PointwiseError) -> String {
    format!(
        "{} @ {:.4e} / {} @ {:.4e}",
        pct(p.max_over),
        p.e_over,
        pct(p.max_under),
        p.e_under
    )
}

#[allow(clippy::too_many_lines)]
fn stage_pointwise(threads: usize, with_urr: bool) {
    let t0 = Instant::now();
    let (u_tape, u_mat) = tape("n-092_U_238.endf");
    let s = settings(STUDY_ERRTHN);
    let recon = Reconstruction {
        reconr: Nuclide::reconstruct_0k(&u_tape, u_mat, 1.0e-3).expect("RECONR"),
        tape: Arc::clone(&u_tape),
        mat: u_mat,
        reconr_tolerance: s.reconr_tolerance,
    };
    println!("RECONR U-238: {:.1} s", t0.elapsed().as_secs_f64());
    let grid: Vec<f64> = recon
        .reconr
        .sections
        .iter()
        .find(|x| x.mt == MtReaction::Mt1Total)
        .expect("MT=1")
        .pairs
        .iter()
        .map(|p| p.0)
        .filter(|&e| (1.0e-5..=E_RES_HI).contains(&e))
        .collect();
    println!(
        "comparison grid (0 K RECONR, 1e-5..20 keV): {} points",
        grid.len()
    );

    // C-12, fixed at 300 K in every arm.
    let (c_tape, c_mat) = tape("n-006_C_012-ENDF8.0.endf");
    let c_recon = Reconstruction {
        reconr: Nuclide::reconstruct_0k(&c_tape, c_mat, 1.0e-3).expect("RECONR C"),
        tape: Arc::clone(&c_tape),
        mat: c_mat,
        reconr_tolerance: 1.0e-3,
    };
    let c12 = nuclide_at(&c_recon, "C12", 300.0, STUDY_ERRTHN, None);
    let sigma_p_c = c12.xs_at_energy(100.0, 300.0).elastic;
    println!("sigma_p(C-12) at 100 eV = {sigma_p_c:.5} b");

    // The lethargy grid, from the 300 K table (it depends only on the energy
    // grid, the band and the mixture's alphas).
    let t1 = Instant::now();
    let u300 = nuclide_at(&recon, "U238", 300.0, STUDY_ERRTHN, None);
    println!(
        "BROADR U-238 300 K (errthn {STUDY_ERRTHN}): {:.1} s",
        t1.elapsed().as_secs_f64()
    );
    let band_grid: Vec<f64> = grid
        .iter()
        .copied()
        .filter(|&e| e >= SD_BAND.e_bot && e <= SD_BAND.e_top)
        .collect();
    let mix_for = |sb: f64| {
        vec![
            MixComponent {
                nuclide_idx: 0,
                atom_density: 1.0,
            },
            MixComponent {
                nuclide_idx: 1,
                atom_density: sb / sigma_p_c,
            },
        ]
    };
    let base = slowing_down_grid(
        &[u300.clone(), c12.clone()],
        &mix_for(SIGMA_B[0]),
        SD_BAND,
        300.0,
        &band_grid,
        20,
    );
    let lethargy = base.lethargy.clone();
    let alpha = base.alpha.clone();
    let sd_e: Vec<f64> = lethargy
        .iter()
        .map(|u| SD_BAND.e_top * (-u).r_exp())
        .collect();
    let c_sd: Vec<(f64, f64)> = sd_e
        .iter()
        .map(|&e| {
            let x = c12.xs_at_energy(e, 300.0);
            (
                (x.total - x.absorption - x.inelastic - x.n2n).max(0.0),
                x.absorption,
            )
        })
        .collect();
    println!("slowing-down grid: {} lethargy points", lethargy.len());

    // Instrument check: the exact arm through `slowing_down_grid_from` is
    // the library's own grid.
    let t300 = table_from(&u300, 300.0, &grid, &sd_e);
    let rebuilt = sd_grid(&t300.sd, &c_sd, SIGMA_B[0] / sigma_p_c, &lethargy, &alpha);
    let (p_lib, p_reb) = (
        solve_on_grid(&base).escaped,
        solve_on_grid(&rebuilt).escaped,
    );
    println!(
        "instrument check: p(lib grid) = {p_lib:.10}, p(rebuilt) = {p_reb:.10}, diff {:.2e}\n",
        p_reb - p_lib
    );
    drop(u300);

    // Floor: the exact 600 K table at errthn 1e-3 against 1e-4, and 1e-4
    // against 1e-5 (is the study's own tolerance converged?).
    {
        let tabs: Vec<Table> = parallel(3, threads, |j| {
            let tol = [1.0e-3, STUDY_ERRTHN, 1.0e-5][j];
            table_from(
                &nuclide_at(&recon, "U238", 600.0, tol, None),
                600.0,
                &grid,
                &sd_e,
            )
        });
        println!("## Thinning floor at 600 K (P14)\n");
        println!("| errthn pair | channel | pointwise 1e-5..1 eV (max over / under, %) | 1..20 keV | band integrals (rel, %) | p σb=100 (rel, %) |\n|---|---|---|---|---|---|");
        for (pair, a, b) in [
            ("1e-3 vs 1e-4", &tabs[0], &tabs[1]),
            ("1e-4 vs 1e-5", &tabs[1], &tabs[2]),
        ] {
            let nc = SIGMA_B[0] / sigma_p_c;
            let pr = p_of(&a.sd, &c_sd, nc, &lethargy, &alpha)
                / p_of(&b.sd, &c_sd, nc, &lethargy, &alpha)
                - 1.0;
            for (name, x, y) in [
                ("capture", &a.capture, &b.capture),
                ("total", &a.total, &b.total),
            ] {
                let bi: Vec<String> = BANDS
                    .iter()
                    .map(|(lo, hi, _)| {
                        pct(lethargy_integral(&grid, x, *lo, *hi)
                            / lethargy_integral(&grid, y, *lo, *hi)
                            - 1.0)
                    })
                    .collect();
                println!(
                    "| {pair} | {name} | {} | {} | {} | {} |",
                    fmt_pw(&pointwise(&grid, x, y, 1e-5, 1.0)),
                    fmt_pw(&pointwise(&grid, x, y, 1.0, E_RES_HI)),
                    bi.join(", "),
                    pct(pr)
                );
            }
        }
        println!();
    }

    // Every temperature: the 37.5 K grid and the fit nodes.
    let mut temps: Vec<f64> = (0..=((T_HI - T_LO) / FINE) as usize)
        .map(|k| T_LO + FINE * k as f64)
        .collect();
    for v in [Var::SqrtT, Var::Kt] {
        for n in FIT_N {
            temps.extend(chebyshev_nodes_k(n, T_LO, T_HI, v).expect("nodes"));
        }
    }
    let urr_temps: Vec<f64> = (0..=((T_HI - T_LO) / 75.0) as usize)
        .map(|k| T_LO + 75.0 * k as f64)
        .collect();
    let urr_energies: Vec<f64> = if with_urr {
        let t = UrrProbabilityTables::from_endf(&u_tape, u_mat, 300.0, 20, 1, 100)
            .expect("purr")
            .expect("urr range");
        let e = t.energies();
        e[1..e.len() - 1].to_vec()
    } else {
        Vec::new()
    };
    let t2 = Instant::now();
    let tables: Vec<Table> = parallel(temps.len(), threads, |j| {
        let t = temps[j];
        let is_urr = with_urr && urr_temps.iter().any(|&u| key(u) == key(t));
        let urr = if is_urr {
            let (nb, nl, ns) = URR_SETTINGS;
            UrrProbabilityTables::from_endf(&u_tape, u_mat, t, nb, nl, ns).expect("purr")
        } else {
            None
        };
        let n = nuclide_at(&recon, "U238", t, STUDY_ERRTHN, urr.clone());
        let mut tab = table_from(&n, t, &grid, &sd_e);
        if let Some(u) = &urr {
            tab.urr = Some(urr_numbers(&n, u, t, &urr_energies));
        }
        tab
    });
    println!(
        "processed {} temperatures ({} with PURR) in {:.1} s\n",
        temps.len(),
        if with_urr { urr_temps.len() } else { 0 },
        t2.elapsed().as_secs_f64()
    );
    let by_t: BTreeMap<i64, &Table> = temps
        .iter()
        .zip(&tables)
        .map(|(t, x)| (key(*t), x))
        .collect();
    let at = |t: f64| {
        *by_t
            .get(&key(t))
            .unwrap_or_else(|| panic!("no table at {t} K"))
    };

    // Per-table integrals.
    let band_int = |x: &[f64]| -> Vec<f64> {
        BANDS
            .iter()
            .map(|(lo, hi, _)| lethargy_integral(&grid, x, *lo, *hi))
            .collect()
    };
    let nr = |t: &Table| -> Vec<ShieldedIntegrals> {
        SIGMA0
            .iter()
            .map(|&s0| ShieldedIntegrals::of(&grid, &t.capture, &t.total, s0, 1.0, E_RES_HI))
            .collect()
    };
    let p_exact = |t: &Table, sb: f64| {
        solve_on_grid(&sd_grid(&t.sd, &c_sd, sb / sigma_p_c, &lethargy, &alpha)).escaped
    };

    // ── Stochastic and plug-in arms ──────────────────────────────────────
    println!("## Stochastic arms: worst over all brackets, and the lowest bracket\n");
    println!(
        "Test temperature = the bracket's midpoint in T. Errors are (approx/exact - 1) in %.\n"
    );
    let mut rows_pw = Vec::new();
    let mut rows_int = Vec::new();
    let mut rows_low = Vec::new();
    for &d in &SPACINGS {
        let grid_t: Vec<f64> = (0..=((T_HI - T_LO) / d) as usize)
            .map(|k| T_LO + d * k as f64)
            .collect();
        for arm in ["T", "sqrt T", "log T", "plug-in T"] {
            let v = match arm {
                "sqrt T" => Var::SqrtT,
                "log T" => Var::LogT,
                _ => Var::Kt,
            };
            let plug = arm == "plug-in T";
            let mut worst_pw_res = (0.0f64, String::new());
            let mut worst_pw_th = 0.0f64;
            let mut worst_tot = 0.0f64;
            let mut worst_low = (0.0f64, String::new());
            let mut worst_band = 0.0f64;
            let mut worst_ri = [0.0f64; 3];
            let mut worst_p = [0.0f64; 2];
            for k in 0..grid_t.len() - 1 {
                let m = 0.5 * (grid_t[k] + grid_t[k + 1]);
                let b = bracket(&grid_t, m, v).expect("bracket");
                let [(i0, w0), (i1, w1)] = b.weights();
                let (lo, hi, ex) = (at(grid_t[i0]), at(grid_t[i1]), at(m));
                let cap = mix(&lo.capture, &hi.capture, w0, w1);
                let tot = mix(&lo.total, &hi.total, w0, w1);
                let pw_res = pointwise(&grid, &cap, &ex.capture, 1.0, E_RES_HI);
                let pw_th = pointwise(&grid, &cap, &ex.capture, 1.0e-5, 1.0);
                if pw_res.max_abs() > worst_pw_res.0.abs() {
                    worst_pw_res = (pw_res.max_abs(), format!("{} K: {}", m, fmt_pw(&pw_res)));
                }
                worst_pw_th = worst_pw_th.max(pw_th.max_abs());
                let pw_low = pointwise(&grid, &cap, &ex.capture, 1.0, 1.0e3);
                if pw_low.max_abs() > worst_low.0 {
                    worst_low = (pw_low.max_abs(), format!("{} K: {}", m, fmt_pw(&pw_low)));
                }
                worst_tot =
                    worst_tot.max(pointwise(&grid, &tot, &ex.total, 1.0, E_RES_HI).max_abs());
                let (bl, bh, be) = (
                    band_int(&lo.capture),
                    band_int(&hi.capture),
                    band_int(&ex.capture),
                );
                for q in 0..BANDS.len() {
                    let r = (w0 * bl[q] + w1 * bh[q]) / be[q] - 1.0;
                    if r.abs() > worst_band.abs() {
                        worst_band = r;
                    }
                }
                let (nl, nh, ne) = (nr(lo), nr(hi), nr(ex));
                let mut ri = [0.0; 3];
                for q in 0..3 {
                    let approx = if plug {
                        ShieldedIntegrals::of(&grid, &cap, &tot, SIGMA0[q], 1.0, E_RES_HI)
                    } else {
                        ShieldedIntegrals::mix(&[(nl[q], w0), (nh[q], w1)])
                    };
                    ri[q] = approx.n / ne[q].n - 1.0;
                    if ri[q].abs() > worst_ri[q].abs() {
                        worst_ri[q] = ri[q];
                    }
                }
                let mut pp = [0.0; 2];
                for (q, &sb) in SIGMA_B.iter().enumerate() {
                    let nc = sb / sigma_p_c;
                    let g = if plug {
                        let sd: Vec<(f64, f64)> = lo
                            .sd
                            .iter()
                            .zip(&hi.sd)
                            .map(|(a, c)| (w0 * a.0 + w1 * c.0, w0 * a.1 + w1 * c.1))
                            .collect();
                        sd_grid(&sd, &c_sd, nc, &lethargy, &alpha)
                    } else {
                        let (gl, gh) = (
                            sd_grid(&lo.sd, &c_sd, nc, &lethargy, &alpha),
                            sd_grid(&hi.sd, &c_sd, nc, &lethargy, &alpha),
                        );
                        mix_slowing_down_grids(&[(&gl, w0), (&gh, w1)])
                    };
                    pp[q] = solve_on_grid(&g).escaped / p_exact(ex, sb) - 1.0;
                    if pp[q].abs() > worst_p[q].abs() {
                        worst_p[q] = pp[q];
                    }
                }
                if k == 0 || k == grid_t.len() - 2 {
                    rows_low.push(format!(
                        "| {d} | {arm} | [{}, {}] -> {m} | {} | {} | {} | {} | {} | {} |",
                        grid_t[k],
                        grid_t[k + 1],
                        pct(pw_res.max_over),
                        pct(pw_res.max_under),
                        pct(ri[0]),
                        pct(ri[1]),
                        pct(pp[0]),
                        pct(pp[1])
                    ));
                }
            }
            rows_pw.push(format!(
                "| {d} | {arm} | {} | {} | {} | {} |",
                worst_pw_res.1,
                worst_low.1,
                pct(worst_pw_th),
                pct(worst_tot)
            ));
            rows_int.push(format!(
                "| {d} | {arm} | {} | {} | {} | {} | {} | {} |",
                pct(worst_band),
                pct(worst_ri[0]),
                pct(worst_ri[1]),
                pct(worst_ri[2]),
                pct(worst_p[0]),
                pct(worst_p[1])
            ));
        }
    }
    println!("### Pointwise capture, worst bracket (max over / max under, %, at eV)\n");
    println!("| Δ K | arm | worst bracket, 1 eV-20 keV | worst bracket, 1 eV-1 keV | worst max abs, 1e-5-1 eV | total: worst max abs, 1 eV-20 keV |\n|---|---|---|---|---|---|");
    rows_pw.iter().for_each(|r| println!("{r}"));
    println!("\n### Integrals, worst signed error over brackets (%)\n");
    println!("| Δ K | arm | capture band integral (any band) | NR RI σ0=10 | σ0=100 | σ0=1000 | p σb=100 | p σb=1000 |\n|---|---|---|---|---|---|---|---|");
    rows_int.iter().for_each(|r| println!("{r}"));
    println!("\n### Lowest and highest bracket per spacing (%)\n");
    println!("| Δ K | arm | bracket -> T | pw over | pw under | RI σ0=10 | RI σ0=100 | p σb=100 | p σb=1000 |\n|---|---|---|---|---|---|---|---|---|");
    rows_low.iter().for_each(|r| println!("{r}"));

    // ── Chebyshev fit arms ───────────────────────────────────────────────
    println!("\n## Chebyshev fit arms over [{T_LO}, {T_HI}] K, worst over the 37.5 K grid (%)\n");
    println!("| variable | n | pw capture 1-20 keV max abs | at T, E | pw capture 1 eV-1 keV max abs | min fitted capture (b) | NR RI σ0=100 | p σb=100 | p σb=1000 |\n|---|---|---|---|---|---|---|---|---|");
    let fine: Vec<f64> = (0..=((T_HI - T_LO) / FINE) as usize)
        .map(|k| T_LO + FINE * k as f64)
        .collect();
    for v in [Var::SqrtT, Var::Kt] {
        for n in FIT_N {
            let nodes = chebyshev_nodes_k(n, T_LO, T_HI, v).expect("nodes");
            let node_tabs: Vec<&Table> = nodes.iter().map(|&t| at(t)).collect();
            let (mut w_pw, mut w_at, mut minv, mut w_ri, mut w_p) =
                (0.0f64, String::new(), f64::INFINITY, 0.0f64, [0.0f64; 2]);
            let mut w_low = 0.0f64;
            for &t in &fine {
                let w = chebyshev_weights(n, T_LO, T_HI, v, t).expect("weights");
                let cap = combine(&node_tabs, &w, |tb| &tb.capture);
                let tot = combine(&node_tabs, &w, |tb| &tb.total);
                let ex = at(t);
                minv = minv.min(cap.iter().copied().fold(f64::INFINITY, f64::min));
                let pw = pointwise(&grid, &cap, &ex.capture, 1.0, E_RES_HI);
                w_low = w_low.max(pointwise(&grid, &cap, &ex.capture, 1.0, 1.0e3).max_abs());
                if pw.max_abs() > w_pw {
                    w_pw = pw.max_abs();
                    w_at = format!(
                        "{t} K, {:.4e} eV",
                        if pw.max_over >= -pw.max_under {
                            pw.e_over
                        } else {
                            pw.e_under
                        }
                    );
                }
                let ri = ShieldedIntegrals::of(&grid, &cap, &tot, 100.0, 1.0, E_RES_HI).n
                    / ShieldedIntegrals::of(&grid, &ex.capture, &ex.total, 100.0, 1.0, E_RES_HI).n
                    - 1.0;
                if ri.abs() > w_ri.abs() {
                    w_ri = ri;
                }
                let sd: Vec<(f64, f64)> = (0..sd_e.len())
                    .map(|i| {
                        let a: f64 = node_tabs
                            .iter()
                            .zip(&w)
                            .map(|(tb, wk)| wk * tb.sd[i].0)
                            .sum();
                        let b: f64 = node_tabs
                            .iter()
                            .zip(&w)
                            .map(|(tb, wk)| wk * tb.sd[i].1)
                            .sum();
                        (a, b)
                    })
                    .collect();
                for (q, &sb) in SIGMA_B.iter().enumerate() {
                    let r = solve_on_grid(&sd_grid(&sd, &c_sd, sb / sigma_p_c, &lethargy, &alpha))
                        .escaped
                        / p_exact(ex, sb)
                        - 1.0;
                    if r.abs() > w_p[q].abs() {
                        w_p[q] = r;
                    }
                }
            }
            println!(
                "| {} | {n} | {} | {w_at} | {} | {minv:.4e} | {} | {} | {} |",
                v.label(),
                pct(w_pw),
                pct(w_low),
                pct(w_ri),
                pct(w_p[0]),
                pct(w_p[1])
            );
        }
    }

    // Instrument check: E[σ] at a tabulated temperature is that table.
    {
        let g: Vec<f64> = vec![300.0, 600.0, 900.0];
        let b = bracket(&g, 600.0, Var::Kt).expect("bracket");
        let [(i0, w0), (i1, w1)] = b.weights();
        let cap = mix(&at(g[i0]).capture, &at(g[i1]).capture, w0, w1);
        let e = pointwise(&grid, &cap, &at(600.0).capture, 1e-5, E_RES_HI);
        println!(
            "\ninstrument check: E[sigma] at a tabulated 600 K vs its table: max abs {:.1e}",
            e.max_abs()
        );
    }

    if with_urr {
        println!(
            "\n## URR (U-238, {} energies inside the range), PURR {:?}\n",
            urr_energies.len(),
            URR_SETTINGS
        );
        println!("Worst over URR energies of (approx/exact - 1) in %, worst over brackets; group = lethargy-integrated over the URR energies.\n");
        println!("| Δ K | arm | σc,eff σ0=10 max abs | σ0=100 | σ0=1000 | σc mean (inf. dil.) | σt,eff σ0=10 | group σc,eff σ0=10 |\n|---|---|---|---|---|---|---|---|");
        for &d in &[600.0, 300.0, 150.0] {
            let grid_t: Vec<f64> = (0..=((T_HI - T_LO) / d) as usize)
                .map(|k| T_LO + d * k as f64)
                .collect();
            for v in Var::ALL {
                let (mut w_c, mut w_m, mut w_t, mut w_g) = ([0.0f64; 3], 0.0f64, 0.0f64, 0.0f64);
                for k in 0..grid_t.len() - 1 {
                    let m = 0.5 * (grid_t[k] + grid_t[k + 1]);
                    let [(i0, w0), (i1, w1)] = bracket(&grid_t, m, v).expect("bracket").weights();
                    let (lo, hi, ex) = (
                        at(grid_t[i0]).urr.as_ref().expect("urr"),
                        at(grid_t[i1]).urr.as_ref().expect("urr"),
                        at(m).urr.as_ref().expect("urr"),
                    );
                    let (mut ga, mut ge) =
                        (vec![0.0; urr_energies.len()], vec![0.0; urr_energies.len()]);
                    for j in 0..urr_energies.len() {
                        for q in 0..3 {
                            let a = (w0 * lo[j].0[q].0 + w1 * hi[j].0[q].0)
                                / (w0 * lo[j].0[q].2 + w1 * hi[j].0[q].2);
                            let x = ex[j].0[q].0 / ex[j].0[q].2;
                            let r = a / x - 1.0;
                            if r.abs() > w_c[q].abs() {
                                w_c[q] = r;
                            }
                            if q == 0 {
                                ga[j] = a;
                                ge[j] = x;
                                let at_ = (w0 * lo[j].0[0].1 + w1 * hi[j].0[0].1)
                                    / (w0 * lo[j].0[0].2 + w1 * hi[j].0[0].2);
                                let rt = at_ / (ex[j].0[0].1 / ex[j].0[0].2) - 1.0;
                                if rt.abs() > w_t.abs() {
                                    w_t = rt;
                                }
                            }
                        }
                        let rm = (w0 * lo[j].1 + w1 * hi[j].1) / ex[j].1 - 1.0;
                        if rm.abs() > w_m.abs() {
                            w_m = rm;
                        }
                    }
                    let rg = lethargy_integral(&urr_energies, &ga, 0.0, f64::INFINITY)
                        / lethargy_integral(&urr_energies, &ge, 0.0, f64::INFINITY)
                        - 1.0;
                    if rg.abs() > w_g.abs() {
                        w_g = rg;
                    }
                }
                println!(
                    "| {d} | {} | {} | {} | {} | {} | {} | {} |",
                    v.label(),
                    pct(w_c[0]),
                    pct(w_c[1]),
                    pct(w_c[2]),
                    pct(w_m),
                    pct(w_t),
                    pct(w_g)
                );
            }
        }
        // How much the URR self-shielding moves with T at all, for scale.
        let (a, b) = (
            at(300.0).urr.as_ref().expect("urr"),
            at(1500.0).urr.as_ref().expect("urr"),
        );
        let mv = a
            .iter()
            .zip(b)
            .map(|(x, y)| ((y.0[0].0 / y.0[0].2) / (x.0[0].0 / x.0[0].2) - 1.0).abs())
            .fold(0.0, f64::max);
        println!("\nscale: largest |σc,eff(1500 K)/σc,eff(300 K) - 1| at σ0 = 10 b over URR energies: {}", pct(mv));
    }
    println!("\nwall time {:.1} s", t0.elapsed().as_secs_f64());
}

/// `Σ_k w_k f(table_k)[i]` at every grid point: the fit arm's values.
fn combine<F: Fn(&Table) -> &Vec<f64>>(tabs: &[&Table], w: &[f64], f: F) -> Vec<f64> {
    let n = tabs.first().map_or(0, |t| f(t).len());
    (0..n)
        .map(|i| tabs.iter().zip(w).map(|(tb, wk)| wk * f(tb)[i]).sum())
        .collect()
}

/// Resonance escape probability of the U-238 + C-12 grid.
fn p_of(u: &[(f64, f64)], c: &[(f64, f64)], nc: f64, lethargy: &[f64], alpha: &[f64]) -> f64 {
    solve_on_grid(&sd_grid(u, c, nc, lethargy, alpha)).escaped
}

/// The slowing-down grid of U-238 (micro (scatter, absorb) per point, atom
/// density 1) and C-12 (atom density `nc`).
fn sd_grid(
    u: &[(f64, f64)],
    c: &[(f64, f64)],
    nc: f64,
    lethargy: &[f64],
    alpha: &[f64],
) -> SlowingDownGrid {
    slowing_down_grid_from(lethargy, SD_BAND.e_top, alpha, |j, _| {
        vec![u[j], (nc * c[j].0, nc * c[j].1)]
    })
}

fn stage_mc(threads: usize, histories: usize) {
    let t0 = Instant::now();
    let (u_tape, u_mat) = tape("n-092_U_238.endf");
    let recon = Reconstruction {
        reconr: Nuclide::reconstruct_0k(&u_tape, u_mat, 1.0e-3).expect("RECONR"),
        tape: Arc::clone(&u_tape),
        mat: u_mat,
        reconr_tolerance: 1.0e-3,
    };
    let (c_tape, c_mat) = tape("n-006_C_012-ENDF8.0.endf");
    let c_recon = Reconstruction {
        reconr: Nuclide::reconstruct_0k(&c_tape, c_mat, 1.0e-3).expect("RECONR C"),
        tape: Arc::clone(&c_tape),
        mat: c_mat,
        reconr_tolerance: 1.0e-3,
    };
    let temps = [300.0, 1500.0, 900.0];
    let mut nucs: Vec<Nuclide> = parallel(3, threads, |j| {
        nuclide_at(&recon, "U238", temps[j], STUDY_ERRTHN, None)
    });
    let c12 = nuclide_at(&c_recon, "C12", 300.0, STUDY_ERRTHN, None);
    let sigma_p_c = c12.xs_at_energy(100.0, 300.0).elastic;
    nucs.push(c12);
    let target = 900.0;
    let lib_t = [300.0, 1500.0];
    let b = bracket(&lib_t, target, Var::Kt).expect("bracket");
    println!("## MC check: U-238 + C-12, 10 keV -> 1 eV, sigma_b = 100 b; {target} K from tables {lib_t:?} (T), f = {:.4}\n", b.upper_probability);
    let sb = 100.0;
    let mix_ex = [
        MixComponent {
            nuclide_idx: 2,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 3,
            atom_density: sb / sigma_p_c,
        },
    ];
    let mix_st = [
        MixComponent {
            nuclide_idx: 0,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 3,
            atom_density: sb / sigma_p_c,
        },
    ];

    // Deterministic, on the union of the three tables' own grids.
    let mut ge: Vec<f64> = (0..3)
        .flat_map(|j| nucs[j].native_energy_grid(SD_BAND.e_bot, SD_BAND.e_top))
        .collect();
    ge.sort_by(f64::total_cmp);
    ge.dedup();
    let det = |m: &[MixComponent], t| slowing_down_grid(&nucs, m, SD_BAND, t, &ge, 20);
    let g_ex = det(&mix_ex, target);
    let g_lo = det(&mix_st, 300.0);
    let mix_hi = [
        MixComponent {
            nuclide_idx: 1,
            ..mix_st[0]
        },
        mix_st[1],
    ];
    let g_hi = det(&mix_hi, 1500.0);
    let [(_, w0), (_, w1)] = b.weights();
    let p_det_ex = solve_on_grid(&g_ex).escaped;
    let p_det_st = solve_on_grid(&mix_slowing_down_grids(&[(&g_lo, w0), (&g_hi, w1)])).escaped;

    // Monte Carlo, per-collision draws, three arms in parallel.
    let t1 = Instant::now();
    let runs = parallel(3, threads, |j| {
        let mc = InfiniteMediumMc {
            histories,
            seed: 0x0947_0000_0000_0001 + 0x1000 * j as u64,
            kernel: ScatterKernel::IsotropicCmAtRest,
            max_collisions: 100_000,
        };
        match j {
            0 => mc.run(&nucs, &mix_ex, SD_BAND, target),
            _ => mc.run_with_tables(&nucs, &mix_st, SD_BAND, target, |i, _, seed| {
                if i == 0 {
                    b.pick(prn(seed))
                } else {
                    3
                }
            }),
        }
    });
    let mc = InfiniteMediumMc {
        histories,
        ..InfiniteMediumMc::default()
    };
    let s = |p: f64| mc.stderr_of(p);
    let (pe, p1, p2) = (runs[0].escaped, runs[1].escaped, runs[2].escaped);
    let pst = 0.5 * (p1 + p2);
    let sst = 0.5 * (s(p1).r_powi(2) + s(p2).r_powi(2)).sqrt();
    println!("| arm | deterministic p | MC p ± 1σ | MC histories |\n|---|---|---|---|");
    println!(
        "| exact {target} K | {p_det_ex:.6} | {pe:.6} ± {:.6} | {histories} |",
        s(pe)
    );
    println!("| stochastic (T) | {p_det_st:.6} | {pst:.6} ± {sst:.6} (two seeds: {p1:.6}, {p2:.6}) | {} |", 2 * histories);
    let d_mc = pst - pe;
    let s_mc = (s(pe).r_powi(2) + sst.r_powi(2)).sqrt();
    println!(
        "\nstochastic - exact: deterministic {:+.6} ({}%), MC {:+.6} ± {:.6} ({:.1} sigma)",
        p_det_st - p_det_ex,
        pct(p_det_st / p_det_ex - 1.0),
        d_mc,
        s_mc,
        d_mc / s_mc
    );
    println!(
        "MC stochastic - deterministic stochastic: {:+.6} ± {sst:.6} ({:.1} sigma); MC exact - deterministic exact: {:+.6} ± {:.6} ({:.1} sigma)",
        pst - p_det_st,
        (pst - p_det_st) / sst,
        pe - p_det_ex,
        s(pe),
        (pe - p_det_ex) / s(pe)
    );
    println!(
        "MC wall time {:.1} s on {threads} threads; total {:.1} s",
        t1.elapsed().as_secs_f64(),
        t0.elapsed().as_secs_f64()
    );
}

/// S(α,β) brackets: (lower, upper, target) deck temperatures \[K\].
const SAB_BRACKETS: [(f64, f64, f64); 12] = [
    (300.0, 350.0, 323.6),
    (400.0, 450.0, 423.6),
    (500.0, 550.0, 523.6),
    (550.0, 600.0, 573.6),
    (300.0, 400.0, 350.0),
    (400.0, 500.0, 450.0),
    (500.0, 600.0, 550.0),
    (300.0, 500.0, 400.0),
    (400.0, 600.0, 500.0),
    (300.0, 600.0, 450.0),
    (350.0, 650.0, 500.0),
    (400.0, 800.0, 600.0),
];

fn stage_sab(threads: usize, histories: usize, mc_bracket: usize) {
    let t0 = Instant::now();
    let mut temps: Vec<f64> = SAB_BRACKETS.iter().flat_map(|b| [b.0, b.1, b.2]).collect();
    temps.sort_by(f64::total_cmp);
    temps.dedup();
    let sab: Vec<ThermalScattering> = parallel(temps.len(), threads, |j| {
        ThermalScattering::from_embedded_leapr_deck(SabMaterial::HInH2O, temps[j], "H in H2O")
            .expect("LEAPR deck")
    });
    println!(
        "LEAPR + bake at {} temperatures: {:.1} s",
        temps.len(),
        t0.elapsed().as_secs_f64()
    );
    let idx = |t: f64| {
        temps
            .iter()
            .position(|&x| key(x) == key(t))
            .expect("temperature")
    };
    let cutoff = sab
        .iter()
        .map(ThermalScattering::cutoff_ev)
        .fold(f64::INFINITY, f64::min);
    let mut grid: Vec<f64> = sab
        .iter()
        .flat_map(ThermalScattering::breakpoints)
        .filter(|&e| e > 0.0 && e < cutoff)
        .collect();
    grid.extend(
        (0..=4000)
            .map(|k| 1.0e-5 * (cutoff / 1.0e-5).r_powf(k as f64 / 4000.0))
            .filter(|&e| e < cutoff),
    );
    grid.sort_by(f64::total_cmp);
    grid.dedup();
    println!(
        "cutoff {cutoff} eV, comparison grid {} points\n",
        grid.len()
    );
    let sab_bands = [
        (1.0e-5, 1.0e-2, "1e-5-0.01 eV"),
        (1.0e-2, 0.1, "0.01-0.1"),
        (0.1, 1.0, "0.1-1"),
        (1.0, cutoff, "1-cutoff"),
    ];
    println!("## σ_sab per H, stochastic arms (%)\n");
    println!("| bracket -> T | Δ K | arm | pointwise max over @ eV / under @ eV | band integrals ({}) |\n|---|---|---|---|---|", sab_bands.map(|b| b.2).join(", "));
    let vals: Vec<Vec<f64>> = sab
        .iter()
        .map(|s| grid.iter().map(|&e| s.total_xs(e)).collect())
        .collect();
    for &(lo, hi, t) in &SAB_BRACKETS {
        for v in Var::ALL {
            let [(i0, w0), (i1, w1)] = bracket(&[lo, hi], t, v).expect("bracket").weights();
            let (a, c) = (&vals[idx([lo, hi][i0])], &vals[idx([lo, hi][i1])]);
            let m = mix(a, c, w0, w1);
            let ex = &vals[idx(t)];
            let pw = pointwise(&grid, &m, ex, 1.0e-5, cutoff);
            let bi: Vec<String> = sab_bands
                .iter()
                .map(|(l, h, _)| {
                    pct(
                        lethargy_integral(&grid, &m, *l, *h) / lethargy_integral(&grid, ex, *l, *h)
                            - 1.0,
                    )
                })
                .collect();
            println!(
                "| [{lo}, {hi}] -> {t} | {} | {} | {} | {} |",
                hi - lo,
                v.label(),
                fmt_pw(&pw),
                bi.join(", ")
            );
        }
    }

    // Transport: H2O + Pu-239, absorbed fraction in Pu-239, widest bracket.
    let (lo, hi, t) = SAB_BRACKETS[mc_bracket];
    let (h_tape, h_mat) = tape("n-001_H_001-ENDF8.0-Beta6.endf");
    let (o_tape, o_mat) = tape("n-008_O_016-ENDF8.0.endf");
    let (p_tape, p_mat) = tape("n-094_Pu_239-JENDL3.3.endf");
    let rc = |tp: &Arc<Tape>, m: i32| Reconstruction {
        reconr: Nuclide::reconstruct_0k(tp, m, 1.0e-3).expect("RECONR"),
        tape: Arc::clone(tp),
        mat: m,
        reconr_tolerance: 1.0e-3,
    };
    let (hr, or, pr) = (rc(&h_tape, h_mat), rc(&o_tape, o_mat), rc(&p_tape, p_mat));
    // ThermalScattering is not Clone: move the three tables out.
    let mut sab: Vec<Option<ThermalScattering>> = sab.into_iter().map(Some).collect();
    let mut h_at = |tt: f64| {
        let th = sab[idx(tt)].take().expect("each table used once");
        nuclide_at(&hr, "H1", tt, 1.0e-3, None).with_thermal_scattering(th)
    };
    // 0: H at lo, 1: H at hi, 2: H at t, 3: O-16 at t, 4: Pu-239 at t.
    let nucs = vec![
        h_at(lo),
        h_at(hi),
        h_at(t),
        nuclide_at(&or, "O16", t, 1.0e-3, None),
        nuclide_at(&pr, "Pu239", t, 1.0e-3, None),
    ];
    let n_pu = 6.5e-4;
    let mix_ex = [
        MixComponent {
            nuclide_idx: 2,
            atom_density: 2.0,
        },
        MixComponent {
            nuclide_idx: 3,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 4,
            atom_density: n_pu,
        },
    ];
    let mix_st = [
        MixComponent {
            nuclide_idx: 0,
            ..mix_ex[0]
        },
        mix_ex[1],
        mix_ex[2],
    ];
    let b = bracket(&[lo, hi], t, Var::Kt).expect("bracket");
    let band = SlowingDownBand {
        e_top: 4.0,
        e_bot: 1.0e-5,
    };
    let t1 = Instant::now();
    let runs = parallel(3, threads, |j| {
        let mc = InfiniteMediumMc {
            histories,
            seed: 0x0947_5AB0_0000_0001 + 0x1000 * j as u64,
            kernel: ScatterKernel::Production,
            max_collisions: 1_000_000,
        };
        match j {
            0 => mc.run(&nucs, &mix_ex, band, t),
            _ => mc.run_with_tables(&nucs, &mix_st, band, t, |i, _, seed| {
                if i == 0 {
                    b.pick(prn(seed))
                } else {
                    mix_st[i].nuclide_idx
                }
            }),
        }
    });
    let mc = InfiniteMediumMc {
        histories,
        ..InfiniteMediumMc::default()
    };
    let s = |p: f64| mc.stderr_of(p);
    let (fe, f1, f2) = (
        runs[0].absorbed_by[2],
        runs[1].absorbed_by[2],
        runs[2].absorbed_by[2],
    );
    let fst = 0.5 * (f1 + f2);
    let sst = 0.5 * (s(f1).r_powi(2) + s(f2).r_powi(2)).sqrt();
    println!("\n## Transport: H2O + Pu-239 (N_Pu/N_H2O = {n_pu}), source 4 eV, absorbed fraction in Pu-239, {t} K from [{lo}, {hi}] (T)\n");
    println!("| arm | fraction ± 1σ | histories |\n|---|---|---|");
    println!("| exact | {fe:.6} ± {:.6} | {histories} |", s(fe));
    println!(
        "| stochastic | {fst:.6} ± {sst:.6} (seeds: {f1:.6}, {f2:.6}) | {} |",
        2 * histories
    );
    let d = fst - fe;
    let sd = (s(fe).r_powi(2) + sst.r_powi(2)).sqrt();
    println!(
        "\nstochastic - exact = {d:+.6} ± {sd:.6} ({:.1} sigma); escaped below 1e-5 eV: {:.2e}",
        d / sd,
        runs[0].escaped
    );
    println!(
        "MC wall {:.1} s on {threads} threads; total {:.1} s",
        t1.elapsed().as_secs_f64(),
        t0.elapsed().as_secs_f64()
    );
}
