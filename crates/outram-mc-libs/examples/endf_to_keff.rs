//! # Tutorial 1: Godiva, hello world
//!
//! **The first model in the beginner API** (`outram_mc_libs::model`, epic
//! gh #925; tutorial list on #943). A bare sphere of highly enriched uranium
//! metal: one shape, one material, a temperature, and one word for the run
//! size. No cells, no universes, no nuclide indices, no physics switches.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example endf_to_keff            # MCSettings::Auto
//! cargo run --release -p outram-mc-libs --example endf_to_keff -- --fast  # MCSettings::Fast (smoke test)
//! cargo run --release -p outram-mc-libs --example endf_to_keff -- --endf /path/to/endf/folder
//! ```
//!
//! (This file used to be the low-level "ENDF to k_eff" tutorial; it was
//! repurposed as tutorial 1 on 2026-10-17, as the maintainer asked on #943:
//! tutorials refactor existing examples. The low-level path it used to show
//! is what `model::Model` lowers to; `examples/godiva_keff_endf_local.rs`
//! still shows it, and stays the V&V case.)
//!
//! ## The case
//!
//! ICSBEP **HEU-MET-FAST-001** (Godiva): radius 8.7407 cm, room temperature
//! 293.6 K, U-234 / U-235 / U-238 at 4.9184e-4 / 4.4994e-2 / 2.4984e-3 atoms
//! per barn·cm, benchmark `k = 1.0000 ± 0.0010`. The numbers are read from
//! `outram_mc_libs::vv::godiva`, the one transcription every recorded Godiva
//! result in this crate uses (its source: the ICSBEP model as tabulated in
//! Mosteller, LA-UR-10-06230, and van der Marck, Nucl. Data Sheets 107
//! (2006) 3061), not typed again here.
//!
//! The specification gives **atom densities**, so the model enters them
//! exactly (`Material::from_atom_densities`: the spec wins, #943). The lesson
//! then builds the same material the way an engineer would say it (an
//! enrichment, a contaminant, a density) and shows the two agree.
//!
//! ## What this checks (V&V methodology)
//!
//! - **Number densities, both ways.** The fuel builder, given the
//!   enrichment, U-234 ppm and base density computed from the spec with the
//!   library's own masses, must reproduce the spec's atom densities to
//!   `1e-12` relative (the #937 acceptance test, on the real library).
//! - **k against the record.** The crate's recorded Godiva is
//!   `−6 ± 5 pcm` from the benchmark (1024 seeds, sd 165 pcm per run at
//!   5000 × [40 + 120], `examples/godiva_keff_ensemble.rs`, 2026-10-05). This
//!   run must sit within 4σ of it, σ combining this run's own (the larger of
//!   the naive and batch-means σ) and the record's ±5. That is a regression
//!   gate sized off what one run can resolve, not an accuracy claim. Nothing
//!   was tuned: the run size is the preset's.
//! - **The geometry is drawn** from the assembled CSG (crate HARD RULE): an
//!   x-y and an x-z slice, written to `target/tutorials/godiva/` (gitignored).
//!
//! ## Results (2026-10-17, ENDF/B-VIII.0 from `reference-data/endf/`)
//!
//! Hardware: Intel i9-13900K (16 logical cores, shared and loaded), 62.5 GiB
//! RAM, Linux, CPU only; transport single-threaded, data processed on up to
//! 8 threads. Commit: the one adding this text.
//!
//! - **Number densities both ways:** largest relative difference
//!   **3.1e-16** (U-235), i.e. identical to rounding.
//! - **`MCSettings::Auto`, seed 1:** `k = 1.00099 ± 0.00089` (batch-means
//!   σ 0.00079), **+99 ± 89 pcm** from the benchmark, **+105 pcm = 1.2σ**
//!   from the record. The watcher passed at cycle 50 (1 000/cycle) and
//!   re-confirmed at cycle 95 (10 000/cycle); 250 active cycles. URR and DBRC
//!   on for all three nuclides. Wall time: data 29 s, transport 11 s.
//! - **`--seeds 32`** (same size, seeds 1-32): **−2 ± 15 pcm** (sd 86 per
//!   run), **+4 pcm = 0.2σ** from the record; χ²/dof 1.09, consistent, no
//!   outlier seed. So the beginner path reproduces the recorded Godiva
//!   within statistics, nothing tuned.
//! - **`--fast`, seed 1:** `k = 1.00369 ± 0.00183` (+369 ± 188 pcm, 2.0σ
//!   from the record: a smoke test, as it says). Its watcher passed at
//!   1 000/cycle at cycle 50 but **never re-confirmed at 5 000/cycle before
//!   the 100-cycle cap**, so the result says NOT CONVERGED. Whether that is a
//!   stationarity false alarm on a 30-cycle window or a real transient is
//!   exactly what #927 is to measure; it is recorded here, not explained.

use std::path::PathBuf;
use std::time::Instant;

use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{AVOGADRO_PER_MOL, BARN_CM_PER_CUBIC_CM};
use outram_mc_libs::perf_report::HardwareInfo;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// The recorded Godiva offset from HEU-MET-FAST-001 \[pcm\]:
/// `examples/godiva_keff_ensemble.rs`, 1024 seeds, 2026-10-05.
const RECORDED_PCM: f64 = -6.0;
/// Its standard error \[pcm\].
const RECORDED_SEM_PCM: f64 = 5.0;
/// Gate width in combined σ.
const K_SIGMA: f64 = 4.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let fast = args.iter().any(|a| a == "--fast");
    let folder = args
        .iter()
        .position(|a| a == "--endf")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    // ── 1. The nuclear data ─────────────────────────────────────────────────
    // One handle for the library. Nuclides are found by the identity in each
    // tape's own header, never by file name.
    let data = match folder {
        Some(dir) => NuclearData::endf_folder(&dir),
        None => NuclearData::workspace_reference(),
    };
    let data = match data {
        Ok(d) => d,
        Err(e) => {
            println!("No ENDF tapes: {e}. Clone with the reference data, or pass --endf <folder>.");
            return Ok(());
        }
    };

    // ── 2. The model, and the one component it has ──────────────────────────
    let mut godiva = Model::new(data);
    let core = godiva.component("Core");

    // ── 3. The material: the specification's own atom densities ─────────────
    let spec = |name: &str| {
        godiva::NUCLIDES
            .iter()
            .find(|n| n.0 == name)
            .map(|n| n.2)
            .unwrap_or(0.0)
    };
    let heu = Material::from_atom_densities([
        (U234, atoms_per_barn_cm(spec("U234"))),
        (U235, atoms_per_barn_cm(spec("U235"))),
        (U238, atoms_per_barn_cm(spec("U238"))),
    ])
    .named("Godiva HEU");

    // ── 3b. Aside: the same metal, as an engineer would say it ──────────────
    // An enrichment (U-235 over U-235 + U-238), U-234 as a contaminant, and
    // the base metal's density. Here they are computed from the spec with
    // the library's own masses, to show the builder lands on the same atoms.
    let masses = godiva.data().atomic_masses();
    let m = |i: Isotope| masses.mass_u(i).unwrap_or(f64::NAN);
    let (n4, n5, n8) = (spec("U234"), spec("U235"), spec("U238"));
    let enrichment = Ratio::new::<ratio>(n5 / (n5 + n8));
    let u234_ppm = Ratio::new::<ratio>(n4 / (n5 + n8));
    let rho_base = (n5 * m(U235) + n8 * m(U238)) / AVOGADRO_PER_MOL / BARN_CM_PER_CUBIC_CM;
    let engineer = Material::fuel(
        HeavyMetal::Uranium(Enrichment::by_atom(FissileUranium::U235, enrichment)),
        FuelForm::Metallic,
    )
    .density(MassDensity::new::<gram_per_cubic_centimeter>(rho_base))
    .contaminant(
        U234,
        Contaminant::ppm(u234_ppm, Basis::Atom, PpmOf::HeavyMetal, Placement::AddOn),
    )
    .named("enrichment + ppm");
    let both = heu
        .resolve(&masses)?
        .compare_number_densities(&engineer.resolve(&masses)?);
    println!("The material two ways (enrichment {:.4} % by atom, U-234 {:.0} ppm by atom of the uranium,", 100.0 * enrichment.get::<ratio>(), 1e6 * u234_ppm.get::<ratio>());
    println!(
        "base density {rho_base:.4} g/cm3; masses: {}):\n{both}\n",
        masses.source()
    );
    assert!(
        both.max_relative_difference() < 1e-12,
        "the two descriptions must give the same atoms"
    );

    // ── 4. The geometry: a sphere filled with it; vacuum outside ────────────
    let sphere = Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM));
    godiva.add(sphere.filled_with(heu).in_component(core));

    // ── 5. The temperature, set on the geometry ─────────────────────────────
    godiva.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));

    // ── 6. Look at it before running it (Brown's first recommendation) ──────
    let assembled = godiva.assemble()?;
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/godiva");
    std::fs::create_dir_all(&out)?;
    for (basis, name) in [
        (PlotBasis::Xy, "godiva_xy.png"),
        (PlotBasis::Xz, "godiva_xz.png"),
    ] {
        let img = assembled.draw_slice(
            basis,
            600,
            &format!("Godiva, {} slice through the centre", basis.name()),
        );
        img.write_png(out.join(name))?;
        println!("drew {}", out.join(name).display());
    }

    // ── 7. Run ──────────────────────────────────────────────────────────────
    let preset = if fast {
        MCSettings::Fast
    } else {
        MCSettings::Auto
    };
    let t_data = Instant::now();
    let nuclides = assembled.load_nuclides(godiva.data())?;
    let data_s = t_data.elapsed().as_secs_f64();
    let t_run = Instant::now();
    let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, None))?;
    let run_s = t_run.elapsed().as_secs_f64();
    // (`godiva.keff(KeffSettings::new(preset, None))?` does steps 6-7 in one
    // call; they are split here only to time data and transport separately.)

    println!("\n{run}");
    println!(
        "  wall time: data {data_s:.1} s (processed in parallel), transport {run_s:.1} s single-threaded\n  hardware: {}",
        HardwareInfo::detect().headline()
    );

    // ── 8. Against the benchmark and the record ─────────────────────────────
    let k = run.keff();
    let sigma = k.std.max(run.provenance.sigma_batch_means.unwrap_or(0.0));
    let pcm = (k.mean - godiva::BENCHMARK_K) * 1e5;
    let combined = ((sigma * 1e5).powi(2) + RECORDED_SEM_PCM.powi(2)).sqrt();
    println!(
        "\nk_eff = {k}  ({pcm:+.0} ± {:.0} pcm from HEU-MET-FAST-001, benchmark ±{:.0} pcm)",
        sigma * 1e5,
        godiva::BENCHMARK_SIGMA * 1e5
    );
    println!(
        "recorded: {RECORDED_PCM:+.0} ± {RECORDED_SEM_PCM:.0} pcm; this run differs by {:+.0} pcm = {:.1} sigma (gate: {K_SIGMA} sigma)",
        pcm - RECORDED_PCM,
        (pcm - RECORDED_PCM).abs() / combined
    );
    assert!(
        (pcm - RECORDED_PCM).abs() <= K_SIGMA * combined,
        "Godiva through the beginner API moved {:.0} pcm from the recorded {RECORDED_PCM:+.0} pcm: \
         more than {K_SIGMA} sigma. Find out what changed; do not widen this gate.",
        pcm - RECORDED_PCM
    );

    // ── 9. Optional: how sure is one run? Pool independent ones ─────────────
    // Brown: "the true uncertainties may be higher ... It may be helpful to
    // make independent Monte Carlo runs" (LA-UR-08-06558, Conclusions).
    // `--seeds N` repeats the run N times at the same size (the inactive
    // count the watcher chose, then the preset's active cycles), seeds
    // 1..=N, through `MCSettings::Custom`, and pools them.
    let n_seeds: usize = args
        .iter()
        .position(|a| a == "--seeds")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if n_seeds > 1 {
        use outram_mc_libs::stats::ensemble::{
            consecutive_seeds, default_workers, run_seeds, EnsembleReport,
        };
        let p = &run.provenance;
        let size = |seed| CustomRun {
            particles: p.active_particles,
            inactive: p.inactive_cycles,
            active: p.active_cycles,
            seed,
        };
        let workers = default_workers();
        let t = Instant::now();
        let runs = run_seeds(&consecutive_seeds(n_seeds), workers, |seed| {
            let r = assembled
                .keff_with(
                    &nuclides,
                    &KeffSettings::new(MCSettings::Custom(size(seed)), None),
                )
                .expect("a seed run");
            ((r.keff().mean - 1.0) * 1e5, r.keff().std * 1e5)
        });
        let rep = EnsembleReport::from_runs(runs);
        let sigma = (rep.sem.powi(2) + RECORDED_SEM_PCM.powi(2)).sqrt();
        println!(
            "\n{n_seeds} seeds x Custom {{ {} per cycle, {} inactive, {} active }}: mean {:+.0} pcm, \
             sd {:.0}, sem ±{:.0}; against the record {:+.0} pcm = {:.1} sigma ({:.0} s on {workers} workers)",
            p.active_particles,
            p.inactive_cycles,
            p.active_cycles,
            rep.mean,
            rep.sd,
            rep.sem,
            rep.mean - RECORDED_PCM,
            (rep.mean - RECORDED_PCM).abs() / sigma,
            t.elapsed().as_secs_f64()
        );
        print!("{}", rep.consistency_summary("pcm"));
        assert!(
            (rep.mean - RECORDED_PCM).abs() <= K_SIGMA * sigma,
            "the pooled Godiva moved more than {K_SIGMA} sigma from the record"
        );
    }
    Ok(())
}
