//! **GitHub #948: windowed multipole (WMP) at the local temperature, against
//! data processed exactly at that temperature.** The #947 instrument, so the
//! two studies are comparable.
//!
//! Methodology, predictions (committed before any run at T > 0) and results:
//! `crates/outram-mc-libs/verification_and_validation/wmp_local_temperature_948/README.md`.
//!
//! Stages (each prints markdown tables):
//!
//! - `probe` — the CORE WMP sets' structure, and WMP at 0 K against RECONR at
//!   0 K of the ENDF/B-VII.0 and VIII.0 tapes (which evaluation is it?).
//! - `accuracy` — U-238 and U-235 (ENDF/B-VII.0) at 293.6, 600, 900, 1200,
//!   1500 and 2500 K: WMP against BROADR at errthn 1e-4. Pointwise, band
//!   integrals, NR self-shielded integrals, and the U-238 + C-12 resonance
//!   escape probability of #947. Plus the thinning floor at 900 K.
//! - `mismatch` — `Nuclide::with_core_multipole` on the ENDF/B-VIII.0 tapes
//!   (expected: refused) and on VII.0 (expected: accepted), with the numbers.
//! - `keff` — `k` of a U(3 %, VII.0) + C-nat (VII.0) sphere at 900 K, tables
//!   against WMP. `--histories N --active N`.
//! - `majorant` — `Majorant::bounding` + `Majorant::audit` on that material
//!   with WMP on, at each temperature.
//! - `timing` — ns per U-238 lookup in the resolved range, tables against WMP.
//! - `seam` — the diagnostic of the V&V record §5.0: where in 1–20 keV the
//!   U-238 WMP and table absorption integrals part, at several T and
//!   tolerances (`SEAM_RECONR_TOL=1e-4` sets RECONR's).
//!
//! Run: `cargo run --release -p outram-mc-libs --example
//! wmp_local_temperature_948 -- <stage> [--threads N]`.
//!
//! No test reaches `main` or the stage functions: they are CLI glue around
//! `Nuclide::with_multipole` (tested in `tests/wmp_local_temperature_948.rs`)
//! and the #947 instruments (tested in `vv::temperature_interpolation_947`).

use petir::mathf::RealMath;
use std::sync::Arc;
use std::time::Instant;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, Reconstruction};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::wmp::WmpLibrary;
use njoy_outram_park_fork::MtReaction;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::delta_tracking::majorant::Majorant;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::physics::slowing_down::{
    slowing_down_grid, solve_on_grid, MixComponent, SlowingDownBand, SlowingDownGrid,
};
use outram_mc_libs::vv::temperature_interpolation_947::{
    lethargy_integral, pointwise, slowing_down_grid_from, PointwiseError, ShieldedIntegrals,
};
use uom::si::thermodynamic_temperature::kelvin;

const TEMPS: [f64; 6] = [293.6, 600.0, 900.0, 1200.0, 1500.0, 2500.0];
const SIGMA_B: [f64; 2] = [100.0, 1000.0];
const SD_BAND: SlowingDownBand = SlowingDownBand {
    e_top: 1.0e4,
    e_bot: 1.0,
};
const STUDY_ERRTHN: f64 = 1.0e-4;
const U238_VII: &str = "n-092_U_238-ENDF7.0.endf";
const U235_VII: &str = "n-092_U_235-ENDF7.0.endf";

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stage = args.first().cloned().unwrap_or_else(|| "accuracy".into());
    let threads: usize = arg_value(&args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let histories: usize = arg_value(&args, "--histories")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    let active: usize = arg_value(&args, "--active")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    println!(
        "hardware: {}",
        outram_mc_libs::perf_report::HardwareInfo::detect().headline()
    );
    println!("threads used: {threads}\n");
    match stage.as_str() {
        "probe" => stage_probe(),
        "accuracy" => stage_accuracy(threads),
        "mismatch" => stage_mismatch(),
        "keff" => stage_keff(threads, histories, active),
        "majorant" => stage_majorant(),
        "timing" => stage_timing(),
        "seam" => stage_seam(),
        other => {
            panic!("unknown stage {other}: probe | accuracy | mismatch | keff | majorant | timing")
        }
    }
}

fn tape(file: &str) -> (Arc<Tape>, i32) {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing tape {file}"));
    let t = Tape::read_file(&p).expect("tape");
    let mat = t.materials()[0];
    (Arc::new(t), mat)
}

fn recon_of(file: &str) -> Reconstruction {
    recon_tol(file, 1.0e-3)
}

fn recon_tol(file: &str, tol: f64) -> Reconstruction {
    let (t, mat) = tape(file);
    Reconstruction {
        reconr: Nuclide::reconstruct_0k(&t, mat, tol).expect("RECONR"),
        tape: Arc::clone(&t),
        mat,
        reconr_tolerance: tol,
    }
}

fn nuclide_at(recon: &Reconstruction, name: &str, t_k: f64, errthn: f64) -> Nuclide {
    let s = CrossSectionSettings::default()
        .without_purr()
        .without_heatr()
        .with_reconr_tolerance(recon.reconr_tolerance)
        .with_broadr_tolerance(errthn);
    let ncs =
        process::cross_sections_at(recon, Temperature::new::<kelvin>(t_k), &s).expect("process");
    let processed = ProcessedEvaluation {
        recon: ncs.pendf,
        elastic_0k: ncs.elastic_0k.unwrap_or_default(),
        urr: None,
        temp_k: t_k,
    };
    Nuclide::from_processed(&recon.tape, recon.mat, name, t_k, processed).expect("nuclide")
}

fn grid_of(recon: &Reconstruction, lo: f64, hi: f64) -> Vec<f64> {
    recon
        .reconr
        .sections
        .iter()
        .find(|x| x.mt == MtReaction::Mt1Total)
        .expect("MT=1")
        .pairs
        .iter()
        .map(|p| p.0)
        .filter(|&e| e >= lo && e <= hi)
        .collect()
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

fn parallel<T: Send, F: Fn(usize) -> T + Sync>(n: usize, threads: usize, f: F) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<T>>> =
        std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let v = f(i);
                out.lock().expect("lock")[i] = Some(v);
            });
        }
    });
    out.into_inner()
        .expect("lock")
        .into_iter()
        .map(|v| v.expect("job ran"))
        .collect()
}

// --------------------------------------------------------------------- probe

fn stage_probe() {
    let lib = WmpLibrary::core();
    println!("| nuclide | E range (eV) | windows (broaden_poly) | poles | fit_order | sqrt(E) spacing | Doppler width @300 / 2500 K |\n|---|---|---|---|---|---|---|");
    for name in ["U238", "U235", "Pu239", "Fe56", "O16", "H1", "C0", "B10"] {
        let Ok(w) = lib.get(name) else {
            println!("| {name} | not in CORE |");
            continue;
        };
        let ds = |t: f64| (8.617333262e-5 * t / w.awr).sqrt();
        println!(
            "| {name} | {:.3e} – {:.3e} | {} ({}) | {} | {} | {:.3e} | {:.3e} / {:.3e} |",
            w.e_min,
            w.e_max,
            w.windows.len(),
            w.windows.iter().filter(|x| x.broaden_poly).count(),
            w.poles.len(),
            w.fit_order,
            1.0 / w.inv_spacing,
            ds(300.0),
            ds(2500.0)
        );
    }
    println!("\nWMP at 0 K against RECONR 0 K (tolerance 1e-3), relative, on RECONR's grid:\n");
    println!("| WMP vs tape | band | ∫σc dE/E | ∫σel | ∫σt | NR capture σ0 = 100 b | pointwise σc (max over / under) |\n|---|---|---|---|---|---|---|");
    for (name, file) in [
        ("U238", "n-092_U_238.endf"),
        ("U238", U238_VII),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U235", U235_VII),
        ("O16", "n-008_O_016-ENDF8.0.endf"),
        ("O16", "n-008_O_016-ENDF7.0.endf"),
    ] {
        let w = lib.get(name).expect("CORE");
        let r = recon_of(file);
        let grid = grid_of(&r, w.e_min, w.e_max);
        let ev = |mt| -> Vec<f64> { grid.iter().map(|&e| r.reconr.eval_mt(mt, e)).collect() };
        let (cap_r, el_r, tot_r) = (
            ev(MtReaction::Mt102Capture),
            ev(MtReaction::Mt2Elastic),
            ev(MtReaction::Mt1Total),
        );
        let ws: Vec<_> = grid.iter().map(|&e| w.evaluate(e, 0.0)).collect();
        let cap_w: Vec<f64> = ws.iter().map(|x| x.absorption - x.fission).collect();
        let el_w: Vec<f64> = ws.iter().map(|x| x.scatter).collect();
        let tot_w: Vec<f64> = ws.iter().map(|x| x.total()).collect();
        let mut lo = 1.0e-5;
        while lo < w.e_max {
            let hi = (lo * 100.0).min(w.e_max);
            let rel = |a: &[f64], b: &[f64]| {
                lethargy_integral(&grid, a, lo, hi) / lethargy_integral(&grid, b, lo, hi) - 1.0
            };
            let si = |c: &[f64], t: &[f64]| ShieldedIntegrals::of(&grid, c, t, 100.0, lo, hi).n;
            println!(
                "| {name} vs {file} | {lo:.0e}–{hi:.1e} | {:+.2e} | {:+.2e} | {:+.2e} | {:+.2e} | {} |",
                rel(&cap_w, &cap_r),
                rel(&el_w, &el_r),
                rel(&tot_w, &tot_r),
                si(&cap_w, &tot_w) / si(&cap_r, &tot_r) - 1.0,
                fmt_pw(&pointwise(&grid, &cap_w, &cap_r, lo, hi))
            );
            lo = hi;
        }
    }
}

// ------------------------------------------------------------------ accuracy

/// One nuclide's numbers at one temperature on the shared grids.
struct Row {
    total: Vec<f64>,
    capture: Vec<f64>,
    fission: Vec<f64>,
    /// (scatter, absorb) on the slowing-down energies (U-238 only).
    sd: Vec<(f64, f64)>,
}

fn row(n: &Nuclide, t_k: f64, grid: &[f64], sd_e: &[f64]) -> Row {
    let xs: Vec<_> = grid.iter().map(|&e| n.xs_at_energy(e, t_k)).collect();
    Row {
        total: xs.iter().map(|x| x.total).collect(),
        capture: xs.iter().map(|x| x.absorption - x.fission).collect(),
        fission: xs.iter().map(|x| x.fission).collect(),
        sd: sd_e
            .iter()
            .map(|&e| {
                let x = n.xs_at_energy(e, t_k);
                (
                    (x.total - x.absorption - x.inelastic - x.n2n).max(0.0),
                    x.absorption,
                )
            })
            .collect(),
    }
}

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

#[allow(clippy::too_many_lines)]
fn stage_accuracy(threads: usize) {
    let t0 = Instant::now();
    let u8r = recon_tol(U238_VII, 1.0e-4);
    let u5r = recon_tol(U235_VII, 1.0e-4);
    let c_r = recon_of("n-006_C_012-ENDF8.0.endf");
    let w8 = WmpLibrary::core().get("U238").expect("U238");
    let w5 = WmpLibrary::core().get("U235").expect("U235");
    println!("RECONR x3: {:.1} s", t0.elapsed().as_secs_f64());
    let g8 = grid_of(&u8r, 1.0e-5, w8.e_max);
    let g5 = grid_of(&u5r, 1.0e-5, w5.e_max);
    println!(
        "grids: U-238 {} points, U-235 {} points",
        g8.len(),
        g5.len()
    );

    let c12 = nuclide_at(&c_r, "C12", 300.0, STUDY_ERRTHN);
    let sigma_p_c = c12.xs_at_energy(100.0, 300.0).elastic;
    println!("sigma_p(C-12) at 100 eV = {sigma_p_c:.5} b");

    // Lethargy grid from the U-238 energy grid (it depends only on that grid,
    // the band and the alphas), built through the library's own constructor.
    let u_first = nuclide_at(&u8r, "U238", TEMPS[0], STUDY_ERRTHN);
    let band_grid: Vec<f64> = g8
        .iter()
        .copied()
        .filter(|&e| e >= SD_BAND.e_bot && e <= SD_BAND.e_top)
        .collect();
    let mix = vec![
        MixComponent {
            nuclide_idx: 0,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: SIGMA_B[0] / sigma_p_c,
        },
    ];
    let base = slowing_down_grid(
        &[u_first.clone(), c12.clone()],
        &mix,
        SD_BAND,
        TEMPS[0],
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
    let r0 = row(&u_first, TEMPS[0], &g8, &sd_e);
    let reb = sd_grid(&r0.sd, &c_sd, SIGMA_B[0] / sigma_p_c, &lethargy, &alpha);
    println!(
        "instrument check: p(lib grid) = {:.10}, p(rebuilt) = {:.10}\n",
        solve_on_grid(&base).escaped,
        solve_on_grid(&reb).escaped
    );

    // 0 K floor, U-238 and U-235: WMP(0 K) against RECONR on the same grid.
    for (name, r, w, g) in [("U-238", &u8r, &w8, &g8), ("U-235", &u5r, &w5, &g5)] {
        let cr: Vec<f64> = g
            .iter()
            .map(|&e| r.reconr.eval_mt(MtReaction::Mt102Capture, e))
            .collect();
        let cw: Vec<f64> = g
            .iter()
            .map(|&e| {
                let x = w.evaluate(e, 0.0);
                x.absorption - x.fission
            })
            .collect();
        let tr: Vec<f64> = g
            .iter()
            .map(|&e| r.reconr.eval_mt(MtReaction::Mt1Total, e))
            .collect();
        let tw: Vec<f64> = g.iter().map(|&e| w.evaluate(e, 0.0).total()).collect();
        let hi = w.e_max;
        let ri = |c: &[f64], t: &[f64]| ShieldedIntegrals::of(g, c, t, 100.0, 1.0, hi).n;
        println!(
            "0 K floor {name}: pointwise capture 1 eV–E_max {}; NR capture σ0=100 {}",
            fmt_pw(&pointwise(g, &cw, &cr, 1.0, hi)),
            pct(ri(&cw, &tw) / ri(&cr, &tr) - 1.0)
        );
    }

    // Per temperature: exact tables at T and the same nuclide with WMP.
    let t1 = Instant::now();
    let results = parallel(TEMPS.len(), threads, |i| {
        let t = TEMPS[i];
        let tt = Instant::now();
        let ex8 = nuclide_at(&u8r, "U238", t, STUDY_ERRTHN);
        let ex5 = nuclide_at(&u5r, "U235", t, STUDY_ERRTHN);
        let c8 = ex8.multipole_consistency(&w8).expect("consistency");
        let c5 = ex5.multipole_consistency(&w5).expect("consistency");
        let mp8 = ex8
            .clone()
            .with_multipole(w8.clone())
            .expect("U238 VII.0 accepts the CORE set");
        let mp5 = ex5
            .clone()
            .with_multipole(w5.clone())
            .expect("U235 VII.0 accepts the CORE set");
        let out = (
            row(&ex8, t, &g8, &sd_e),
            row(&mp8, t, &g8, &sd_e),
            row(&ex5, t, &g5, &[]),
            row(&mp5, t, &g5, &[]),
            c8,
            c5,
        );
        eprintln!("  T = {t} K done in {:.1} s", tt.elapsed().as_secs_f64());
        out
    });
    println!(
        "per-temperature processing ({threads} threads): {:.1} s\n",
        t1.elapsed().as_secs_f64()
    );

    println!("consistency check (with_multipole, at the table temperature, worst decade band):\n");
    println!("| T K | U-238 | U-235 |\n|---|---|---|");
    for (t, r) in TEMPS.iter().zip(&results) {
        println!(
            "| {t} | {} {:+.2e} [{:.0e}, {:.0e}] | {} {:+.2e} [{:.0e}, {:.0e}] |",
            r.4.channel,
            r.4.worst_relative,
            r.4.e_lo,
            r.4.e_hi,
            r.5.channel,
            r.5.worst_relative,
            r.5.e_lo,
            r.5.e_hi
        );
    }

    println!("\n### U-238 pointwise, WMP against exact (%)\n");
    println!("| T K | capture 1e-5–1 eV | capture 1 eV–1 keV | capture 1–20 keV | total 1 eV–20 keV |\n|---|---|---|---|---|");
    for (t, r) in TEMPS.iter().zip(&results) {
        let (ex, mp) = (&r.0, &r.1);
        println!(
            "| {t} | {} | {} | {} | {} |",
            fmt_pw(&pointwise(&g8, &mp.capture, &ex.capture, 1.0e-5, 1.0)),
            fmt_pw(&pointwise(&g8, &mp.capture, &ex.capture, 1.0, 1.0e3)),
            fmt_pw(&pointwise(&g8, &mp.capture, &ex.capture, 1.0e3, 2.0e4)),
            fmt_pw(&pointwise(&g8, &mp.total, &ex.total, 1.0, 2.0e4)),
        );
    }

    let bands = [
        (1.0e-5, 1.0),
        (1.0, 10.0),
        (10.0, 100.0),
        (100.0, 1.0e3),
        (1.0e3, 2.0e4),
    ];
    println!("\n### U-238 integrals, WMP against exact (%)\n");
    println!("| T K | capture band integrals (5 bands, worst) | total band (worst) | NR RI σ0 = 10 | 100 | 1000 b | **p σb = 100 b** | p σb = 1000 b |\n|---|---|---|---|---|---|---|---|");
    for (t, r) in TEMPS.iter().zip(&results) {
        let (ex, mp) = (&r.0, &r.1);
        let worst = |a: &[f64], b: &[f64]| {
            bands
                .iter()
                .map(|&(lo, hi)| {
                    lethargy_integral(&g8, a, lo, hi) / lethargy_integral(&g8, b, lo, hi) - 1.0
                })
                .fold(0.0_f64, |m, v| if v.abs() > m.abs() { v } else { m })
        };
        let ri = |s0: f64| {
            ShieldedIntegrals::of(&g8, &mp.capture, &mp.total, s0, 1.0, 2.0e4).n
                / ShieldedIntegrals::of(&g8, &ex.capture, &ex.total, s0, 1.0, 2.0e4).n
                - 1.0
        };
        let p = |sb: f64| {
            let nc = sb / sigma_p_c;
            let pe = solve_on_grid(&sd_grid(&ex.sd, &c_sd, nc, &lethargy, &alpha)).escaped;
            let pm = solve_on_grid(&sd_grid(&mp.sd, &c_sd, nc, &lethargy, &alpha)).escaped;
            (pe, pm / pe - 1.0)
        };
        let (pe100, d100) = p(SIGMA_B[0]);
        let (_, d1000) = p(SIGMA_B[1]);
        println!(
            "| {t} | {} | {} | {} | {} | {} | **{}** (p = {pe100:.6}) | {} |",
            pct(worst(&mp.capture, &ex.capture)),
            pct(worst(&mp.total, &ex.total)),
            pct(ri(10.0)),
            pct(ri(100.0)),
            pct(ri(1000.0)),
            pct(d100),
            pct(d1000)
        );
    }

    println!("\n### U-235, WMP against exact (%), 1 eV – 2.25 keV\n");
    println!("| T K | pointwise capture | pointwise fission | capture RI σ0 = 10 / 100 / 1000 | fission RI σ0 = 10 / 100 / 1000 | 1e-5–1 eV fission (pointwise) |\n|---|---|---|---|---|---|");
    for (t, r) in TEMPS.iter().zip(&results) {
        let (ex, mp) = (&r.2, &r.3);
        let hi = w5.e_max;
        let ri = |x_m: &[f64], x_e: &[f64], s0: f64| {
            ShieldedIntegrals::of(&g5, x_m, &mp.total, s0, 1.0, hi).n
                / ShieldedIntegrals::of(&g5, x_e, &ex.total, s0, 1.0, hi).n
                - 1.0
        };
        println!(
            "| {t} | {} | {} | {} / {} / {} | {} / {} / {} | {} |",
            fmt_pw(&pointwise(&g5, &mp.capture, &ex.capture, 1.0, hi)),
            fmt_pw(&pointwise(&g5, &mp.fission, &ex.fission, 1.0, hi)),
            pct(ri(&mp.capture, &ex.capture, 10.0)),
            pct(ri(&mp.capture, &ex.capture, 100.0)),
            pct(ri(&mp.capture, &ex.capture, 1000.0)),
            pct(ri(&mp.fission, &ex.fission, 10.0)),
            pct(ri(&mp.fission, &ex.fission, 100.0)),
            pct(ri(&mp.fission, &ex.fission, 1000.0)),
            fmt_pw(&pointwise(&g5, &mp.fission, &ex.fission, 1.0e-5, 1.0)),
        );
    }

    // Processing ladder at 900 K: how far each (RECONR, BROADR) tolerance
    // pair, and WMP, sit from the finest processing (1e-5, 1e-5).
    let tf = Instant::now();
    let u8_prod = recon_tol(U238_VII, 1.0e-3);
    let u8_fine = recon_tol(U238_VII, 1.0e-5);
    println!(
        "\nRECONR 1e-5: {} points ({:.1} s)",
        grid_of(&u8_fine, 1.0e-5, w8.e_max).len(),
        tf.elapsed().as_secs_f64()
    );
    let arms: Vec<(String, Nuclide)> = vec![
        (
            "RECONR 1e-3 / BROADR 1e-3 (production)".into(),
            nuclide_at(&u8_prod, "U238", 900.0, 1.0e-3),
        ),
        (
            "RECONR 1e-3 / BROADR 1e-4 (#947's exact arm)".into(),
            nuclide_at(&u8_prod, "U238", 900.0, 1.0e-4),
        ),
        (
            "RECONR 1e-4 / BROADR 1e-4 (this study's exact arm)".into(),
            nuclide_at(&u8r, "U238", 900.0, 1.0e-4),
        ),
        (
            "RECONR 1e-4 / BROADR 1e-5".into(),
            nuclide_at(&u8r, "U238", 900.0, 1.0e-5),
        ),
        (
            "WMP".into(),
            nuclide_at(&u8r, "U238", 900.0, 1.0e-4)
                .with_multipole(w8.clone())
                .expect("VII.0"),
        ),
    ];
    let fine = nuclide_at(&u8_fine, "U238", 900.0, 1.0e-5);
    println!("ladder built ({:.1} s)", tf.elapsed().as_secs_f64());
    let rf = row(&fine, 900.0, &g8, &sd_e);
    let nc = SIGMA_B[0] / sigma_p_c;
    let p_of = |r: &Row| solve_on_grid(&sd_grid(&r.sd, &c_sd, nc, &lethargy, &alpha)).escaped;
    let ri = |r: &Row, s0: f64| ShieldedIntegrals::of(&g8, &r.capture, &r.total, s0, 1.0, 2.0e4).n;
    let (pf, rf10, rf100) = (p_of(&rf), ri(&rf, 10.0), ri(&rf, 100.0));
    println!("\n### Processing ladder at 900 K, against RECONR 1e-5 / BROADR 1e-5 (%)\n");
    println!("| arm | p σb = 100 b | RI σ0 = 10 b | RI σ0 = 100 b | worst capture band integral |\n|---|---|---|---|---|");
    for (label, n) in &arms {
        let r = row(n, 900.0, &g8, &sd_e);
        let worst = bands
            .iter()
            .map(|&(lo, hi)| {
                lethargy_integral(&g8, &r.capture, lo, hi)
                    / lethargy_integral(&g8, &rf.capture, lo, hi)
                    - 1.0
            })
            .fold(0.0_f64, |m, v| if v.abs() > m.abs() { v } else { m });
        println!(
            "| {label} | {} | {} | {} | {} |",
            pct(p_of(&r) / pf - 1.0),
            pct(ri(&r, 10.0) / rf10 - 1.0),
            pct(ri(&r, 100.0) / rf100 - 1.0),
            pct(worst)
        );
    }
    println!("\nladder total {:.1} s", tf.elapsed().as_secs_f64());
    println!("\nwall time {:.1} s", t0.elapsed().as_secs_f64());
}

// ------------------------------------------------------------------ mismatch

fn stage_mismatch() {
    println!("| tape | T K | result | worst band |\n|---|---|---|---|");
    for (name, file) in [
        ("U238", U238_VII),
        ("U238", "n-092_U_238.endf"),
        ("U235", U235_VII),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("O16", "n-008_O_016-ENDF7.0.endf"),
        ("O16", "n-008_O_016-ENDF8.0.endf"),
    ] {
        let r = recon_of(file);
        let n = nuclide_at(&r, name, 900.0, 1.0e-3);
        let w = WmpLibrary::core().get(name).expect("CORE");
        let c = n.multipole_consistency(&w).expect("consistency");
        let t = Instant::now();
        let verdict = match n.with_core_multipole() {
            Ok(_) => "accepted".to_string(),
            Err(e) => format!("refused: {e}"),
        };
        println!(
            "| {file} | 900 | {verdict} ({:.2} s) | {} {:+.3e} over [{:.1e}, {:.1e}] eV, {} points |",
            t.elapsed().as_secs_f64(),
            c.channel,
            c.worst_relative,
            c.e_lo,
            c.e_hi,
            c.points
        );
    }
}

// ---------------------------------------------------------------------- keff

fn keff_case(t_k: f64, with_mp: bool) -> (Material, Vec<Nuclide>) {
    let u5 = nuclide_at(&recon_of(U235_VII), "U235", 900.0, 1.0e-3);
    let u8 = nuclide_at(&recon_of(U238_VII), "U238", 900.0, 1.0e-3);
    let c = nuclide_at(&recon_of("n-006_C_000-ENDF7.0.endf"), "C0", 900.0, 1.0e-3);
    let (u5, u8) = if with_mp {
        (
            u5.with_core_multipole().expect("U235 VII.0"),
            u8.with_core_multipole().expect("U238 VII.0"),
        )
    } else {
        (u5, u8)
    };
    let n_c = 0.08;
    let n_u = n_c / 300.0;
    let m = Material {
        id: 1,
        name: "U(3%)+C".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 0.03 * n_u,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 0.97 * n_u,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: n_c,
            },
        ],
        temperature: t_k,
    };
    (m, vec![u5, u8, c])
}

fn stage_keff(threads: usize, histories: usize, active: usize) {
    let mut res = Vec::new();
    for (label, mp, seed) in [("tables", false, 11_u64), ("WMP", true, 23_u64)] {
        let (m, nucs) = keff_case(900.0, mp);
        let s = KeffSettings {
            n_particles: histories,
            n_inactive: 20,
            n_active: active,
            temperature_k: 900.0,
            seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
            ..KeffSettings::default()
        };
        let t = Instant::now();
        let r = run_keff(200.0, &m, &nucs, &s);
        println!(
            "{label}: k = {:.5} ± {:.5} ({} × [20 + {active}], {:.1} s)",
            r.k_mean,
            r.k_std,
            histories,
            t.elapsed().as_secs_f64()
        );
        res.push((r.k_mean, r.k_std));
    }
    let d = res[1].0 - res[0].0;
    let s = (res[0].1.r_powi(2) + res[1].1.r_powi(2)).sqrt();
    println!(
        "WMP − tables = {:+.1} ± {:.1} pcm ({:+.2} σ)",
        1e5 * d,
        1e5 * s,
        d / s
    );
}

// ---------------------------------------------------------------------- seam

/// Where in 1–20 keV the U-238 WMP and table absorption integrals part.
fn stage_seam() {
    let tol: f64 = std::env::var("SEAM_RECONR_TOL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0e-3);
    println!("RECONR tolerance {tol}");
    let r = recon_tol(U238_VII, tol);
    let w = WmpLibrary::core().get("U238").expect("U238");
    let g = grid_of(&r, 1.0e3, 2.0e4);
    for e in [1.99e4, 1.9999e4, 1.99999e4, 2.0e4] {
        let iw = (((e as f64).sqrt() - w.e_min.sqrt()) * w.inv_spacing) as usize;
        let iw = iw.min(w.windows.len() - 1);
        let win = w.windows[iw];
        print!(
            "E {e}: window {iw} poles {}..={} broaden_poly {} |",
            win.start, win.end, win.broaden_poly
        );
        for t in [0.0, 293.6, 900.0] {
            let x = w.evaluate(e, t);
            print!(" T {t}: s {:.4} a {:.4} |", x.scatter, x.absorption);
        }
        println!(
            " table0K a {:.4}",
            r.reconr.eval_mt(MtReaction::Mt102Capture, e)
                + r.reconr.eval_mt(MtReaction::Mt18Fission, e)
        );
    }
    let bands = [
        (1.0e3, 5.0e3),
        (5.0e3, 1.0e4),
        (1.0e4, 1.5e4),
        (1.5e4, 1.9e4),
        (1.9e4, 1.99e4),
        (1.99e4, 2.0e4),
    ];
    for &(t, errthn) in &[
        (293.6, 1.0e-3),
        (900.0, 1.0e-3),
        (900.0, 1.0e-4),
        (2500.0, 1.0e-4),
    ] {
        let n = nuclide_at(&r, "U238", t, errthn);
        let tab: Vec<f64> = g.iter().map(|&e| n.xs_at_energy(e, t).absorption).collect();
        let wm: Vec<f64> = g.iter().map(|&e| w.evaluate(e, t).absorption).collect();
        let w0: Vec<f64> = g.iter().map(|&e| w.evaluate(e, 0.0).absorption).collect();
        let r0: Vec<f64> = g
            .iter()
            .map(|&e| {
                r.reconr.eval_mt(MtReaction::Mt102Capture, e)
                    + r.reconr.eval_mt(MtReaction::Mt18Fission, e)
            })
            .collect();
        print!("T {t} errthn {errthn}:");
        for &(lo, hi) in &bands {
            let rel =
                lethargy_integral(&g, &wm, lo, hi) / lethargy_integral(&g, &tab, lo, hi) - 1.0;
            let rel0 =
                lethargy_integral(&g, &w0, lo, hi) / lethargy_integral(&g, &r0, lo, hi) - 1.0;
            print!(" [{lo:.2e},{hi:.2e}] {rel:+.2e} (0K {rel0:+.1e})");
        }
        println!();
        println!(
            "   pointwise 19-20 keV: {}",
            fmt_pw(&pointwise(&g, &wm, &tab, 1.9e4, 2.0e4))
        );
    }
}

// ------------------------------------------------------------------ majorant

fn stage_majorant() {
    let (m0, nucs) = keff_case(900.0, true);
    println!("| material T K | nodes | worst Σt/Σmaj | at E eV | energies checked | build s |\n|---|---|---|---|---|---|");
    for &t in &TEMPS {
        let mut m = m0.clone();
        m.temperature = t;
        let tb = Instant::now();
        let maj = Majorant::bounding(std::slice::from_ref(&m), &nucs, 1.0e-5, 2.0e7, 4096, 8, 0.0);
        let bs = tb.elapsed().as_secs_f64();
        let a = maj.audit(std::slice::from_ref(&m), &nucs, 1.0e-5, 2.0e7, 4000);
        println!(
            "| {t} | {} | {:.6} | {:.4e} | {} | {bs:.1} |",
            maj.len(),
            a.worst_ratio,
            a.energy_ev,
            a.energies_checked
        );
    }
    // The control: a majorant built at 293.6 K, audited at 1500 K.
    let mut cold = m0.clone();
    cold.temperature = 293.6;
    let maj = Majorant::bounding(
        std::slice::from_ref(&cold),
        &nucs,
        1.0e-5,
        2.0e7,
        4096,
        8,
        0.0,
    );
    let mut hot = m0;
    hot.temperature = 1500.0;
    let a = maj.audit(std::slice::from_ref(&hot), &nucs, 1.0e-5, 2.0e7, 4000);
    println!(
        "\ncontrol: majorant built at 293.6 K, audited at 1500 K: worst {:.4} at {:.4e} eV",
        a.worst_ratio, a.energy_ev
    );
}

// -------------------------------------------------------------------- timing

fn stage_timing() {
    let r = recon_of(U238_VII);
    let tab = nuclide_at(&r, "U238", 900.0, 1.0e-3);
    let mp = tab.clone().with_core_multipole().expect("VII.0");
    let n = 1_000_000;
    let energies: Vec<f64> = (0..n)
        .map(|i| {
            let x =
                ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 11) as f64 / (1u64 << 53) as f64;
            (2.0e4_f64).r_powf(x)
        })
        .collect();
    println!("| lookup (U-238, 900 K, 1 eV–20 keV, 1e6 energies) | tables ns | WMP ns | ratio |\n|---|---|---|---|");
    let time = |f: &dyn Fn(f64) -> f64| {
        let t = Instant::now();
        let mut acc = 0.0;
        for &e in &energies {
            acc += f(e);
        }
        (t.elapsed().as_secs_f64() * 1e9 / n as f64, acc)
    };
    let (a, _) = time(&|e| tab.xs_at_energy(e, 900.0).total);
    let (b, _) = time(&|e| mp.xs_at_energy(e, 900.0).total);
    println!("| xs_at_energy | {a:.0} | {b:.0} | {:.1} |", b / a);
    let (a, _) = time(&|e| tab.total_at_energy(e, 900.0));
    let (b, _) = time(&|e| mp.total_at_energy(e, 900.0));
    println!("| total_at_energy | {a:.0} | {b:.0} | {:.1} |", b / a);
}
