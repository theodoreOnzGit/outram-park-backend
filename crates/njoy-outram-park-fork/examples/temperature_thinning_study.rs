//! What a **thinned temperature grid** costs in accuracy, measured on the
//! ENDF/B-VIII.0 graphite thermal-scattering evaluation.
//!
//! # Why
//!
//! `tsl-crystalline-graphite.endf` is 8.7 MB, of which the incoherent-inelastic
//! `S(α,β)` (MF=7/MT=4) is 99.6 % and the elastic section only 0.36 % — while
//! the *elastic* channel carries about 90 % of graphite's thermal cross section
//! (4.55 b vs 0.49 b at 0.0253 eV). The data cost and the physics importance sit
//! in different places, so a natural low-fidelity option is to keep only a few
//! of the ten tabulated temperatures (296, 400, 500, 600, 700, 800, 1000, 1200,
//! 1600, 2000 K) and interpolate between them.
//!
//! # Methodology
//!
//! The evaluation is its own oracle. For each candidate thinned grid and each
//! **withheld** tabulated temperature, the study interpolates from the two kept
//! temperatures that bracket it — using the evaluation's own ENDF `LI` law,
//! through the same [`interp_s_temperature`] kernel the production reader uses
//! — and compares against the row the evaluation actually tabulates there. No
//! model, no external reference, no fitted parameter.
//!
//! Three measurements per (grid, withheld temperature):
//!
//! 1. **Coherent elastic** — relative error in `S(E)` at every Bragg edge.
//!    Since `σ_coh = S(E)/E` at temperature-independent edge energies, this *is*
//!    the relative error in the cross section.
//! 2. **Incoherent inelastic, table level** — relative error in `S(α,β)` over
//!    all 400 β × 150 α points, reported both unfiltered and restricted to
//!    points within 10⁻⁶ of the peak `S` (the far corners hold values decades
//!    below the peak where a relative error carries no physical weight).
//! 3. **Incoherent inelastic, cross-section level** — relative error in the
//!    integrated `σ_inel(E)`, obtained by running the reference and the
//!    reconstructed `S(α,β)` through the *same* production kernel at the same
//!    physical temperature. This is the physics-weighted number, and it is the
//!    one the size decision should turn on.
//!
//! Two extras: a **leave-one-out** pass on the full grid (drop one interior
//! temperature, keep its immediate neighbours) that characterises the accuracy
//! of the *existing* production interpolation; and a **log-space comparison**
//! (`LI = 4`, `ln S` linear in `T`) against the elastic channel's stated
//! `LI = 2`, testing the prior that Debye-Waller suppression is roughly
//! exponential in temperature. The log-space run is a *reported finding only* —
//! the production path keeps the law the evaluation states.
//!
//! # Running it
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example temperature_thinning_study
//! ```
//!
//! The tape is **not** checked in (`reference-data/endf/` deliberately holds
//! only a README). Set `GRAPHITE_TSL_DIR` to override the default location; the
//! example prints a skip note and exits 0 when the file is absent.
//!
//! Measured results and their interpretation are recorded in the crate's
//! `verification_and_validation/` write-up and in
//! `tests/thermal_temperature_thinning.rs`.
//!
//! [`interp_s_temperature`]: njoy_outram_park_fork::thermr::mf7

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7, parse_mf7_at_temperature};
use njoy_outram_park_fork::thermr::temperature_thinning::{
    coherent_elastic_thinned_row, coherent_elastic_thinning_error,
    coherent_elastic_thinning_error_below, inelastic_cross_section_thinning_error,
    mf7_mt4_endf_bytes, RelativeErrorStats, SabTemperatureStack, ThinnedTemperatureGrid,
    WorstPoint, ZeroPolicy,
};
use uom::si::energy::electronvolt;

const DEFAULT_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";
const FILE: &str = "tsl-crystalline-graphite.endf";
const MAT: i32 = 30;

/// Incident energies \[eV\] at which `σ_inel(E)` is compared: the thermal peak,
/// the 2200 m/s reference, and up into the epithermal tail where the
/// short-collision-time branch takes over.
const SIGMA_ENERGIES_EV: [f64; 6] = [1.0e-3, 5.0e-3, 0.0253, 0.1, 0.5, 3.9];

/// 2200 m/s reference energy \[eV\] — where graphite's coherent-elastic cross
/// section is quoted (4.55 b at 296 K).
const E_THERMAL: f64 = 0.0253;

/// Upper edge of the "where it matters" window \[eV\] for the coherent-elastic
/// channel. The graphite table runs to 5 eV, but `σ_coh = S/E` at 5 eV is two
/// decades below its thermal value; a thermal-reactor calculation cares about
/// the Bragg edges at or below the 2200 m/s point.
const E_RELEVANT_MAX: f64 = E_THERMAL;

/// Candidate thinned grids \[K\], fixed before any error was measured so the
/// study cannot be steered toward a grid that flatters the approach. Every one
/// keeps 296 K (the base temperature carries the shared `α` grid) and 2000 K
/// (the top of the tabulated range; dropping it would mean extrapolating).
const CANDIDATE_GRIDS: [(&str, &[f64]); 5] = [
    ("A  296/600/1200/2000", &[296.0, 600.0, 1200.0, 2000.0]),
    ("B  296/800/2000", &[296.0, 800.0, 2000.0]),
    ("C  296/500/1000/2000", &[296.0, 500.0, 1000.0, 2000.0]),
    (
        "D  296/400/600/1000/2000",
        &[296.0, 400.0, 600.0, 1000.0, 2000.0],
    ),
    (
        "E  296/400/500/600/1000/2000",
        &[296.0, 400.0, 500.0, 600.0, 1000.0, 2000.0],
    ),
];

fn pct(x: f64) -> String {
    if x >= 100.0 {
        format!("{x:.0}%")
    } else if x >= 1.0 {
        format!("{x:.2}%")
    } else {
        format!("{x:.4}%")
    }
}

fn where_worst(w: WorstPoint) -> String {
    match w {
        WorstPoint::BraggEdge { energy } => {
            format!("E = {:.4e} eV", energy.get::<electronvolt>())
        }
        WorstPoint::IncidentEnergy { energy } => {
            format!("E = {:.4e} eV", energy.get::<electronvolt>())
        }
        WorstPoint::AlphaBeta { alpha, beta } => format!("α = {alpha:.4e}, β = {beta:.4e}"),
        WorstPoint::Nothing => "n/a".to_string(),
    }
}

fn row(label: &str, s: &RelativeErrorStats) {
    println!(
        "  {label:<26} max {:>9}  rms {:>9}   worst at {}  ({:.4e} → {:.4e})",
        pct(100.0 * s.max_rel),
        pct(100.0 * s.rms_rel),
        where_worst(s.worst),
        s.worst_reference,
        s.worst_approx,
    );
}

fn main() {
    let dir = std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());
    let path = std::path::Path::new(&dir).join(FILE);
    if !path.exists() {
        eprintln!(
            "SKIP temperature_thinning_study: {FILE} not found under {dir} \
             (set GRAPHITE_TSL_DIR to the ENDF/B-VIII.0 thermal_scatt directory)"
        );
        return;
    }
    let file_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let tape = Tape::read(std::fs::File::open(&path).unwrap()).unwrap();

    println!("# Temperature-grid thinning study — graphite, ENDF/B-VIII.0 (MAT {MAT})");
    println!("# tape: {} ({file_bytes} bytes)\n", path.display());

    // ---------------------------------------------------------------- elastic
    let mf7 = parse_mf7(&tape, MAT).unwrap();
    let ce = mf7.coherent_elastic.expect("graphite has coherent elastic");
    let temps = ce.temperatures_k.clone();
    println!("## Coherent elastic (MF=7/MT=2, LTHR=1)");
    println!(
        "tabulated T [K]: {temps:?}\nBragg edges: {}   LI codes: {:?}\n",
        ce.bragg_energies_ev.len(),
        ce.temp_interp
    );

    // σ_coh(0.0253 eV, T) reconstructed by a thinned grid vs the tabulated row.
    let sigma_thermal = |grid: &ThinnedTemperatureGrid, j: usize, li: Option<u32>| -> (f64, f64) {
        let want = ce.cross_section(E_THERMAL, temps[j]).unwrap();
        let got = coherent_elastic_thinned_row(&ce, grid, j, li)
            .unwrap()
            .cross_section(E_THERMAL, temps[j])
            .unwrap();
        (want, got)
    };

    println!("### Leave-one-out on the FULL grid (accuracy of the PRODUCTION interpolation)");
    println!("      (all 221 edges | edges <= 0.0253 eV | sigma at 0.0253 eV)");
    for j in 1..temps.len() - 1 {
        let grid = ThinnedTemperatureGrid::leave_one_out(temps.len(), j).unwrap();
        let (lo, hi) = grid.bracket(j).unwrap();
        let all = coherent_elastic_thinning_error(&ce, &grid, j, None).unwrap();
        let rel =
            coherent_elastic_thinning_error_below(&ce, &grid, j, None, E_RELEVANT_MAX).unwrap();
        let log = coherent_elastic_thinning_error(&ce, &grid, j, Some(4)).unwrap();
        let log_rel =
            coherent_elastic_thinning_error_below(&ce, &grid, j, Some(4), E_RELEVANT_MAX).unwrap();
        let (want, got) = sigma_thermal(&grid, j, None);
        let (_, got_log) = sigma_thermal(&grid, j, Some(4));
        println!(
            "  {:>5} K from {:>5}/{:>5} K | LI=2 all max {:>8} | thermal max {:>8} \
             | sigma_th {:>8} | LI=4 all max {:>8} | thermal max {:>8} | sigma_th {:>8}",
            temps[j],
            temps[lo],
            temps[hi],
            pct(100.0 * all.max_rel),
            pct(100.0 * rel.max_rel),
            pct(100.0 * (got - want).abs() / want),
            pct(100.0 * log.max_rel),
            pct(100.0 * log_rel.max_rel),
            pct(100.0 * (got_log - want).abs() / want),
        );
    }
    println!();

    for (name, keep) in CANDIDATE_GRIDS {
        let grid = ThinnedTemperatureGrid::from_kept_temperatures(&temps, keep).unwrap();
        println!("### Grid {name}");
        for j in grid.withheld_indices(temps.len()) {
            let Some(stated) = coherent_elastic_thinning_error(&ce, &grid, j, None) else {
                continue;
            };
            let log = coherent_elastic_thinning_error(&ce, &grid, j, Some(4)).unwrap();
            let rel =
                coherent_elastic_thinning_error_below(&ce, &grid, j, None, E_RELEVANT_MAX).unwrap();
            let log_rel =
                coherent_elastic_thinning_error_below(&ce, &grid, j, Some(4), E_RELEVANT_MAX)
                    .unwrap();
            let (want, got) = sigma_thermal(&grid, j, None);
            let (_, got_log) = sigma_thermal(&grid, j, Some(4));
            let (lo, hi) = grid.bracket(j).unwrap();
            println!(
                "  T = {:>5} K (from {:>5}/{:>5} K)",
                temps[j], temps[lo], temps[hi]
            );
            row("S(E) all edges, LI=2", &stated);
            row("S(E) E<=0.0253 eV, LI=2", &rel);
            row("S(E) all edges, LI=4", &log);
            row("S(E) E<=0.0253 eV, LI=4", &log_rel);
            println!(
                "  {:<26} ref {want:.4} b -> LI=2 {got:.4} b ({}), LI=4 {got_log:.4} b ({})",
                "sigma_coh(0.0253 eV)",
                pct(100.0 * (got - want).abs() / want),
                pct(100.0 * (got_log - want).abs() / want),
            );
        }
        println!();
    }

    // -------------------------------------------------------------- inelastic
    println!("## Incoherent inelastic (MF=7/MT=4, S(alpha,beta))");
    let t_start = std::time::Instant::now();
    let stack = SabTemperatureStack::from_tape(&tape, MAT).unwrap();
    let n_beta = stack.kernels[0].beta.len();
    let n_alpha = stack.kernels[0].s_tables[0].alpha.len();
    let s_max = stack.max_s();
    let floor = 1.0e-6 * s_max;
    println!(
        "parsed {} temperatures in {:.1} s; grid {n_beta} beta x {n_alpha} alpha; \
         LAT = {}, LI codes {:?}",
        stack.kernels.len(),
        t_start.elapsed().as_secs_f64(),
        stack.kernels[0].lat,
        stack.temp_interp,
    );
    println!("max tabulated S = {s_max:.6e}; significance floor = 1e-6 x S_max = {floor:.3e}");
    print!("exactly-zero (LEAPR-floored) cells per temperature:");
    for j in 0..stack.temperatures_k.len() {
        let (z, n) = stack.zero_cell_count(j);
        print!(
            " {:.0}K {z}/{n} ({:.1}%)",
            stack.temperatures_k[j],
            100.0 * z as f64 / n as f64
        );
    }
    println!("\n");

    // Reference σ_inel(E) per tabulated temperature, computed once.
    let natom = stack.kernels[0].b.get(5).copied().unwrap_or(1.0);
    println!("### sigma_inel(E) [barn] at every tabulated temperature (natom = {natom})");
    print!("{:>7}", "T [K]");
    for e in SIGMA_ENERGIES_EV {
        print!("{:>12}", format!("{e:.4} eV"));
    }
    println!();
    for (j, &t) in stack.temperatures_k.iter().enumerate() {
        print!("{t:>7.0}");
        for e in SIGMA_ENERGIES_EV {
            print!("{:>12.5}", stack.kernels[j].cross_section(e, t, natom));
        }
        println!();
    }
    println!();

    // Consistency check on the reconstruction itself: hold the *physical*
    // temperature at 400 K and swap only the S(α,β) table. If the interpolated
    // table's σ lies between the two bracketing tables' σ (evaluated at the same
    // 400 K), the reconstruction is behaving monotonically and the large errors
    // below are a property of the data, not of this study's machinery.
    println!("### Reconstruction consistency: sigma_inel(E) at a FIXED 400 K, varying only S(a,b)");
    let grid_a =
        ThinnedTemperatureGrid::from_kept_temperatures(&stack.temperatures_k, CANDIDATE_GRIDS[0].1)
            .unwrap();
    let interp_400 = stack.thinned_kernel(&grid_a, 1, None).unwrap();
    for e in SIGMA_ENERGIES_EV {
        let with_296 = stack.kernels[0].cross_section(e, 400.0, natom);
        let with_600 = stack.kernels[3].cross_section(e, 400.0, natom);
        let with_400 = stack.kernels[1].cross_section(e, 400.0, natom);
        let with_int = interp_400.cross_section(e, 400.0, natom);
        let (min, max) = if with_296 < with_600 {
            (with_296, with_600)
        } else {
            (with_600, with_296)
        };
        println!(
            "  E = {e:>7.4} eV  S296 -> {with_296:.4}  S600 -> {with_600:.4}  \
             S_interp -> {with_int:.4} ({})  |  true S400 -> {with_400:.4} b",
            if with_int >= min - 1e-9 && with_int <= max + 1e-9 {
                "between"
            } else {
                "NOT between"
            }
        );
    }
    println!();

    // Bracket check on the PRODUCTION path: σ_inel(E, T) is monotone in T across
    // the tabulated grid, so an interpolated temperature whose σ falls outside
    // its tabulated bracket is a defect, not an approximation error.
    println!("### PRODUCTION interpolation at non-tabulated T — is sigma_inel bracketed?");
    for &t in &[393.15f64, 523.15, 900.0, 1400.0] {
        let (lo, hi) = {
            let hi = stack.temperatures_k.partition_point(|&x| x < t);
            (hi - 1, hi)
        };
        let interp = parse_mf7_at_temperature(&tape, MAT, Some(t))
            .unwrap()
            .incoherent_inelastic
            .unwrap();
        print!(
            "  T = {t:>7.2} K (bracket {:.0}/{:.0} K):",
            stack.temperatures_k[lo], stack.temperatures_k[hi]
        );
        for e in SIGMA_ENERGIES_EV {
            let a = stack.kernels[lo].cross_section(e, stack.temperatures_k[lo], natom);
            let b = stack.kernels[hi].cross_section(e, stack.temperatures_k[hi], natom);
            let x = interp.cross_section(e, t, natom);
            let (min, max) = if a < b { (a, b) } else { (b, a) };
            let flag = if x < min - 1e-9 || x > max + 1e-9 {
                "OUTSIDE"
            } else {
                "ok"
            };
            print!("  {e:.4}eV {x:.4} in [{min:.4},{max:.4}] {flag};");
        }
        println!();
    }
    println!();

    println!("### Leave-one-out on the FULL grid (accuracy of the PRODUCTION interpolation)");
    for j in 1..stack.temperatures_k.len() - 1 {
        let grid = ThinnedTemperatureGrid::leave_one_out(stack.temperatures_k.len(), j).unwrap();
        let sab = stack.thinning_error(&grid, j, None, floor).unwrap();
        let sig = inelastic_cross_section_thinning_error(
            &stack,
            &grid,
            j,
            None,
            &SIGMA_ENERGIES_EV,
            natom,
            ZeroPolicy::AsProduction,
        )
        .unwrap();
        let sig_z = inelastic_cross_section_thinning_error(
            &stack,
            &grid,
            j,
            None,
            &SIGMA_ENERGIES_EV,
            natom,
            ZeroPolicy::PreserveZeros,
        )
        .unwrap();
        let (lo, hi) = grid.bracket(j).unwrap();
        println!(
            "  {:>5} K from {:>5}/{:>5} K   S(a,b) max {:>9} rms {:>9} | sigma max {:>9} rms {:>9} \
             | sigma(zeros kept) max {:>9} rms {:>9}",
            stack.temperatures_k[j],
            stack.temperatures_k[lo],
            stack.temperatures_k[hi],
            pct(100.0 * sab.max_rel),
            pct(100.0 * sab.rms_rel),
            pct(100.0 * sig.max_rel),
            pct(100.0 * sig.rms_rel),
            pct(100.0 * sig_z.max_rel),
            pct(100.0 * sig_z.rms_rel),
        );
    }
    println!();

    for (name, keep) in CANDIDATE_GRIDS {
        let grid =
            ThinnedTemperatureGrid::from_kept_temperatures(&stack.temperatures_k, keep).unwrap();
        println!("### Grid {name}");
        for j in grid.withheld_indices(stack.temperatures_k.len()) {
            let Some(all) = stack.thinning_error(&grid, j, None, 0.0) else {
                continue;
            };
            let sig = stack.thinning_error(&grid, j, None, floor).unwrap();
            let xs = inelastic_cross_section_thinning_error(
                &stack,
                &grid,
                j,
                None,
                &SIGMA_ENERGIES_EV,
                natom,
                ZeroPolicy::AsProduction,
            )
            .unwrap();
            let xs_thermal = inelastic_cross_section_thinning_error(
                &stack,
                &grid,
                j,
                None,
                &SIGMA_ENERGIES_EV[..4],
                natom,
                ZeroPolicy::AsProduction,
            )
            .unwrap();
            let (lo, hi) = grid.bracket(j).unwrap();
            println!(
                "  T = {:>5} K (from {:>5}/{:>5} K)",
                stack.temperatures_k[j], stack.temperatures_k[lo], stack.temperatures_k[hi]
            );
            row("S(a,b), all points", &all);
            row("S(a,b), >= 1e-6 S_max", &sig);
            row("sigma_inel(E), all E", &xs);
            row("sigma_inel(E), E<=0.1eV", &xs_thermal);
        }
        println!();
    }

    // --------------------------------------------------- combined bottom line
    // The decision turns on the TOTAL thermal cross section, because the two
    // channels are thinned together but carry very different weights: at
    // 0.0253 eV / 296 K graphite is 4.55 b coherent-elastic and 0.49 b
    // incoherent-inelastic, so a 15 % inelastic error is ~1.5 % of the total.
    println!("## Combined sigma_total(0.0253 eV) = coherent elastic + incoherent inelastic");
    println!("   (both channels thinned on the SAME grid; elastic keeps its stated LI=2)");
    for (name, keep) in CANDIDATE_GRIDS {
        let grid_e = ThinnedTemperatureGrid::from_kept_temperatures(&temps, keep).unwrap();
        let grid_i =
            ThinnedTemperatureGrid::from_kept_temperatures(&stack.temperatures_k, keep).unwrap();
        println!("### Grid {name}");
        for j in grid_e.withheld_indices(temps.len()) {
            let t = temps[j];
            let coh_ref = ce.cross_section(E_THERMAL, t).unwrap();
            let coh_got = coherent_elastic_thinned_row(&ce, &grid_e, j, None)
                .unwrap()
                .cross_section(E_THERMAL, t)
                .unwrap();
            let inel_ref = stack.kernels[j].cross_section(E_THERMAL, t, natom);
            let inel_got = stack
                .thinned_kernel(&grid_i, j, None)
                .unwrap()
                .cross_section(E_THERMAL, t, natom);
            let (tot_ref, tot_got) = (coh_ref + inel_ref, coh_got + inel_got);
            println!(
                "  {t:>5.0} K  coh {coh_ref:.4}->{coh_got:.4} ({:>8})  \
                 inel {inel_ref:.4}->{inel_got:.4} ({:>8})  \
                 TOTAL {tot_ref:.4}->{tot_got:.4} ({:>8})",
                pct(100.0 * (coh_got - coh_ref).abs() / coh_ref),
                pct(100.0 * (inel_got - inel_ref).abs() / inel_ref),
                pct(100.0 * (tot_got - tot_ref).abs() / tot_ref),
            );
        }
    }
    println!();

    // ------------------------------------------------------------------ bytes
    println!("## Bytes (ENDF text; MT=4 only — MT=2 is 0.36 % of the tape and kept in full)");
    let full = mf7_mt4_endf_bytes(stack.temperatures_k.len(), n_beta, n_alpha);
    println!(
        "  full 10-temperature MT=4 model: {full} B ({:.2} % of the {file_bytes} B tape)",
        100.0 * full as f64 / file_bytes as f64
    );
    for (name, keep) in CANDIDATE_GRIDS {
        let n = ThinnedTemperatureGrid::from_kept_temperatures(&stack.temperatures_k, keep)
            .unwrap()
            .len();
        let b = mf7_mt4_endf_bytes(n, n_beta, n_alpha);
        println!(
            "  {name:<30} {n:>2} T   MT=4 {b:>9} B   tape ~{:>9} B   saved {:>5.1} %",
            b + (file_bytes as usize - full),
            100.0 * (full - b) as f64 / file_bytes as f64
        );
    }
}
