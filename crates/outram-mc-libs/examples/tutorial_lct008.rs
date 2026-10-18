//! # Tutorial 6: LEU-COMP-THERM-008, a rectangular pin lattice
//!
//! **Pins in assemblies in a core, cut by a cylinder** (beginner API
//! `outram_mc_libs::model`, epic gh #925, repeated structures #926; the
//! tutorial list and its decisions on #943). Tutorial 4 repeated one TRISO
//! particle on a grid; this is the reactor-physics lattice: 4961 fuel pins in
//! 22 distinct assemblies of a 7 x 7 core. New in this lesson:
//!
//! - **a cylinder** (`Cylinder::radius(r)` is a *rod* that runs through its
//!   lattice tile, a fuel pin; `.height(h)` makes a finite cylinder, the
//!   core);
//! - **a lattice as a tile of a lattice** (an assembly of pins is one tile of
//!   the core's lattice), one `Lattice` per statement, the pattern written as
//!   the cards draw it;
//! - **the row-order trap**, said out loud: OpenMC's cards list a lattice's
//!   top row first, outram's CSG lattice starts at the lower-left. The
//!   constructor's name chooses (`Lattice::from_rows_top_down`, here), and an
//!   aside builds the other (`from_natural_order`) to show what the mistake
//!   looks like;
//! - **a lattice cut by its container**: the core's grid is 171.76 cm wide,
//!   the cylinder 152.4 cm, so the wall cuts the water tiles at the edge
//!   (and only water: all 4961 fuel pins sit wholly inside);
//! - **the fast path for a slow model**: 35 nuclides to process. See the
//!   table below.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_lct008                    # MCSettings::Smoke, data processed
//! cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --fast          # MCSettings::Fast
//! cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --auto          # MCSettings::Auto (hours)
//! cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --seed 3        # the record's 10 000 x [40 + 120], seed 3
//! cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --fast --ace-dir target/tutorials/lct008/ace
//! cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --geometry-only # draw and count only
//! crates/outram-mc-libs/examples/common/lct008_ace_library.sh                        # builds that ACE library, once
//! ```
//!
//! (Tutorials refactor existing examples (#943); the V&V case
//! `lct008_keff.rs` keeps its role. This file repurposes its material: the
//! committed OpenMC cards (`verification_and_validation/icsbep/leu-comp-therm-008/`,
//! parsed through `common/lct008_model.rs`) and its record. The beginner
//! statements are in `common/lct008_tutorial_model.rs`, shared with
//! `tests/model_lct008_tutorial.rs`.)
//!
//! ## The case
//!
//! ICSBEP **LEU-COMP-THERM-008 case 1** (B&W critical lattice, Core XI;
//! Paul Romano's OpenMC model in `mit-crpg/benchmarks`, MIT licence): 2.459
//! w/o UO2 pins in borated water (1511 ppm B).
//!
//! | level | what | numbers |
//! |---|---|---|
//! | pin | fuel rod, Al-6061 clad rod, water | radii 0.514858 / 0.602996 cm, pitch 1.63576 cm |
//! | assembly | 15 x 15 pin tiles, pins in 22 different patterns | pitch 24.5364 cm |
//! | core | 7 x 7 assembly tiles (37 hold pins, 12 are water) | cut by a cylinder r 76.2 cm, \|z\| 81.662 cm, vacuum outside |
//!
//! 35 nuclides, 3 materials; every temperature 293.6 K (the cards give none,
//! so OpenMC's default applies). Benchmark `k = 1.0000`, **uncertainty "about
//! 600 pcm" is a stand-in** (`lct008_keff.rs`'s PROVENANCE WARNING: the
//! case's own uncertainty is not in the repository).
//!
//! ## What this checks (V&V methodology)
//!
//! 1. **The model is the V&V model**, pinned without nuclear data by
//!    `tests/model_lct008_tutorial.rs` (200 000 lookups, the lattices' grids
//!    as numbers, and per-generation k bit for bit against `lct008_model`'s
//!    hand-built geometry on the embedded LOW tier).
//! 2. **The pictures are drawn from the assembled geometry** (crate HARD
//!    RULE), into `target/tutorials/lct008/`: the whole core (x-y and x-z),
//!    one assembly, one pin, the cylinder wall cutting the edge, and the
//!    row-order trap.
//! 3. **Counted from the geometry**: the census counts the cards' 4961 fuel
//!    pins from the assembled universe graph.
//! 4. **k against the record**: route 4 (ENDF/B-VIII.0 processed by the Rust
//!    NJOY port, LEAPR H in H2O, URR and DBRC on, after gh:#931) is `k =
//!    1.00135 +- 0.00023` (32 seeds at 10 000 x [40 + 120],
//!    `verification_and_validation/icsbep/five_route_keff_2026_09_29.md`,
//!    "Route 4 re-measured 2026-10-18"). A run must sit within 4 sigma
//!    of it. A regression gate sized off what the run resolves, not an
//!    accuracy claim; a Smoke run resolves almost nothing.
//!
//! ## The fast path (maintainer requirement, #943)
//!
//! What makes this case slow, measured (see Results): processing the 35
//! tapes, then transport. Three ways, none of which changes the physics:
//!
//! - **`MCSettings::Fast`** (5 000 x 120 active) or **`Smoke`** (1 000 x 50)
//!   for transport: a smoke test, not a result to quote; the result says so.
//! - **The ACE route** (`--ace-dir`, gh #936): tables read as they are,
//!   nothing processed. There is **no disk cache of processed nuclides** for
//!   the command line (the browser demos have one in IndexedDB, gh #818), so
//!   the ACE library *is* the cache: `reference-data/ace` holds only the four
//!   uranium nuclides and H in H2O at 283.6 K, none of LCT-008's other 31, so
//!   `examples/common/lct008_ace_library.sh` builds the 38 tapes of the
//!   five-route campaign once with `write_ace_library` (2.0 GB). That
//!   build is the "first run"; every run after reads it.
//! - **Not used for "fast"**: `SpeedTier::VeryFast`, an approximation that
//!   measured -65 +- 40 pcm on this case.
//!
//! ## Predictions (written before the quoted runs, gh #943)
//!
//! Posted on #943 before the first run of the new model:
//! 1. The census counts 4961 fuel pins; the beginner model equals the
//!    hand-built one at every lookup, and k per generation bit for bit.
//! 2. `Fast` k within 4 sigma of 1.00135 +- 0.00023.
//! 3. On 8 cores of an i9-13900K: processing 100 to 250 s; reading a built
//!    ACE library under 60 s; `Fast` transport 30 to 120 s.
//!
//! ## Results (2026-10-18)
//!
//! Hardware: Intel i9-13900K, 8 logical cores used (`taskset -c 0-7`),
//! 62.5 GiB, Linux, CPU only, machine shared (load about 3).
//!
//! - Processing the 35 tapes: 78.8 s. Building the ACE library: 122.6 s
//!   (2.0 GB). Reading it: 4.2 s (warm page cache).
//! - Transport: Smoke 2.0 s, Fast 13.1 s (13.9 s on ACE), Auto 43.8 s.
//! - k: Fast 0.99783 +- 0.00154 (ACE 0.99795 +- 0.00161), Auto processed
//!   1.00158 +- 0.00081 (+0.3 sigma from the record), Auto ACE 0.99938 +-
//!   0.00075 (-2.4 sigma, one draw, not resolved as a data difference);
//!   four seeds of the record's run size on ACE: mean 1.00069 +- 0.00040.
//! - Predictions: 1 held; 2 held (Fast at -2.3 sigma); 3 held for reading,
//!   missed for processing (78.8 s against 100 to 250) and transport (13 s
//!   against 30 to 120).
//!
//! The tutorial page (`docs/tutorial/src/tutorial6-lct008.md`) has the
//! tables and their reading.

use std::path::{Path, PathBuf};
use std::time::Instant;

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::Progress;
use outram_mc_libs::perf_report::HardwareInfo;
use uom::si::volume::cubic_centimeter;

#[path = "common/lct008_model.rs"]
#[allow(dead_code)]
mod lct008_model;
#[path = "common/lct008_tutorial_model.rs"]
#[allow(dead_code)]
mod lct008_tutorial_model;
use lct008_tutorial_model::RowOrder;

/// The crate's record for this model on the ENDF route (route 4, after
/// gh:#931): `k` and its standard error, 32 seeds at 10 000 x [40 + 120]
/// (`five_route_keff_2026_09_29.md`, "Route 4 re-measured 2026-10-18").
const RECORDED_K: (f64, f64) = (1.00135, 0.00023);
/// Gate width in combined sigma.
const K_SIGMA: f64 = 4.0;
/// The benchmark's `k` and the stand-in band (`lct008_keff.rs`).
const BENCHMARK: (f64, f64) = (1.0000, 0.006);

/// Every `.ace` file of the library's `293.6K/` and `0K/` folders, with the
/// folder in its name, as `NuclearData::from_ace_files` takes them.
fn read_ace_library(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    for sub in ["293.6K", "0K"] {
        let folder = dir.join(sub);
        let read = std::fs::read_dir(&folder).map_err(|e| {
            format!(
                "no ACE library at {}: {e}. Build it once with \
                 crates/outram-mc-libs/examples/common/lct008_ace_library.sh",
                folder.display()
            )
        })?;
        let mut paths: Vec<PathBuf> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ace"))
            .collect();
        paths.sort();
        for p in paths {
            let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            files.push((format!("{sub}/{name}"), bytes));
        }
    }
    Ok(files)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let geometry_only = flag("--geometry-only");
    // `--seed N`: the record's own run size, 10 000 x [40 + 120], at seed N
    // (the presets do not take a seed), to compare with the record seed for seed.
    let preset = if let Some(seed) = arg("--seed").and_then(|s| s.parse::<u64>().ok()) {
        MCSettings::Custom(CustomRun {
            particles: 10_000,
            inactive: 40,
            active: 120,
            seed,
        })
    } else if flag("--auto") {
        MCSettings::Auto
    } else if flag("--fast") {
        MCSettings::Fast
    } else {
        MCSettings::Smoke
    };
    println!("{}", HardwareInfo::detect().headline());
    println!(
        "threads: {} (rayon; RAYON_NUM_THREADS sets it)",
        rayon::current_num_threads()
    );

    // ── 0. The nuclear data: processed from ENDF, or read from ACE ─────────
    // `from_folder`, not `workspace_reference()`: that one keeps tapes whose
    // header says NLIB = 0, and Fe-54's says 8, so the clad's Fe-54 would be
    // missing (found writing this tutorial; see its doc).
    let t_data = Instant::now();
    let (data, route) = match arg("--ace-dir") {
        Some(dir) => {
            let files = read_ace_library(&PathBuf::from(&dir))?;
            let d = NuclearData::from_ace_files(&format!("ACE library {dir}"), files)?
                .with_thermal_table(
                    njoy_outram_park_fork::leapr::decks::SabMaterial::HInH2O,
                    "lwtr",
                );
            (d, "ACE tables, read as they are (nothing processed)")
        }
        None => {
            let dir = arg("--endf")
                .map(PathBuf::from)
                .unwrap_or_else(njoy_outram_park_fork::reference_data::reference_endf_dir);
            match NuclearData::from_folder(&dir) {
                Ok(d) => (
                    d,
                    "ENDF/B-VIII.0 tapes processed now (RECONR, BROADR, PURR, LEAPR)",
                ),
                Err(e) => {
                    println!("No ENDF tapes: {e}. Clone with the reference data, or pass --endf <folder>.");
                    return Ok(());
                }
            }
        }
    };
    println!(
        "data route: {route}; indexed in {:.1} s",
        t_data.elapsed().as_secs_f64()
    );

    // ── 1. The model: pins, assemblies, a core (the statements are in
    // common/lct008_tutorial_model.rs, shown on the tutorial page) ──────────
    let t = lct008_tutorial_model::build(data.clone(), |_| true)?;
    let cards = &t.cards;
    println!(
        "cards: {} nuclides, pin pitch {} cm, assembly pitch {} cm; {} assemblies hold pins \
         ({} distinct patterns), {} fuel pins",
        cards.nuclides,
        cards.pin_pitch_cm,
        cards.assembly_pitch_cm,
        cards.assemblies,
        cards.distinct_assemblies,
        cards.fuel_pins
    );
    let t_assemble = Instant::now();
    let assembled = t.model.assemble()?;
    println!(
        "assembled in {:.2} s: {} surfaces, {} cells, {} universes, {} lattices, {} materials, \
         {} nuclides",
        t_assemble.elapsed().as_secs_f64(),
        assembled.geometry.surfaces.len(),
        assembled.geometry.cells.len(),
        assembled.geometry.universes.len(),
        assembled.geometry.lattices.len(),
        assembled.materials.len(),
        assembled.nuclides.len()
    );

    // ── 2. Look at it before running it (crate HARD RULE) ──────────────────
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/lct008");
    std::fs::create_dir_all(&out)?;
    let r_core = lct008_model::R_CORE; // the core's radius, for framing
    let draw = |basis: PlotBasis,
                centre: (f64, f64, f64),
                width: [f64; 2],
                pixels: usize,
                title: &str,
                name: &str|
     -> Result<(), Box<dyn std::error::Error>> {
        let img = assembled.draw_window(
            basis,
            Position::new(centre.0, centre.1, centre.2),
            width,
            pixels,
            title,
        );
        img.write_png(out.join(name))?;
        println!("drew {}", out.join(name).display());
        Ok(())
    };
    let whole = 2.0 * r_core * 1.12;
    draw(
        PlotBasis::Xy,
        (0.0, 0.0, 0.0),
        [whole, whole],
        1400,
        "LEU-COMP-THERM-008 case 1, x-y through the midplane",
        "lct008_xy.png",
    )?;
    draw(
        PlotBasis::Xz,
        (0.0, 0.0, 0.0),
        [whole, 1.12 * (lct008_model::Z_HI - lct008_model::Z_LO)],
        1000,
        "LEU-COMP-THERM-008 case 1, x-z through y = 0",
        "lct008_xz.png",
    )?;
    // One assembly: the centre one (all fuel), then the one at the top-left
    // of the core's fuel region, which the cylinder's wall cuts.
    let ap = cards.assembly_pitch_cm;
    draw(
        PlotBasis::Xy,
        (0.0, 0.0, 0.0),
        [ap, ap],
        1000,
        "the centre assembly: 15 x 15 pins (window = its 24.5364 cm pitch)",
        "lct008_assembly_xy.png",
    )?;
    let pp = cards.pin_pitch_cm;
    draw(
        PlotBasis::Xy,
        (0.0, 0.0, 0.0),
        [2.0 * pp, 2.0 * pp],
        800,
        "one fuel pin: fuel, clad, water (x-y)",
        "lct008_pin_xy.png",
    )?;
    draw(
        PlotBasis::Xy,
        (-2.0 * ap, 2.0 * ap, 0.0),
        [1.4 * ap, 1.4 * ap],
        1000,
        "the cylinder's wall cutting a corner assembly's water",
        "lct008_wall_xy.png",
    )?;

    // ── 3. Counted from the assembled geometry ─────────────────────────────
    let census = assembled.census()?;
    println!("{census}");
    let copies = |prefix: &str| -> usize {
        census
            .rows
            .iter()
            .filter(|r| r.material.starts_with(prefix))
            .map(|r| r.copies)
            .sum()
    };
    let fuel_pins = copies("Fuel");
    println!(
        "census: {fuel_pins} fuel-pin copies, {} clad rods (the cards: {})",
        copies("Aluminum"),
        cards.fuel_pins
    );
    assert_eq!(fuel_pins, cards.fuel_pins, "the census against the cards");
    assert_eq!(fuel_pins, 4961, "the benchmark's fuel pin count");
    // A wall cut: every fuel pin lies wholly inside the cylinder. Looked up
    // from the geometry (each pin's centre is in the fuel; its clad's
    // outermost radius is inside r = 76.2) via the census above, and
    // independently by the cards' arithmetic in `tests/model_lct008_tutorial.rs`.
    let fuel_volume: f64 = census
        .rows
        .iter()
        .filter(|r| r.material.starts_with("Fuel"))
        .map(|r| r.volume().get::<cubic_centimeter>())
        .sum();
    let expect = fuel_pins as f64
        * std::f64::consts::PI
        * lct008_model::R_FUEL.powi(2)
        * (lct008_model::Z_HI - lct008_model::Z_LO);
    println!("fuel volume {fuel_volume:.1} cm3 (4961 pins x pi r^2 x 163.324 cm = {expect:.1})");
    assert!((fuel_volume / expect - 1.0).abs() < 1e-12);

    // ── 4. The trap: the row order of the cards ────────────────────────────
    // Two mistakes, both with the right census. (a) Every lattice read
    // bottom-row-first: a mirror at each level, which composes to a mirror
    // of the whole core. (b) Only the core's lattice read that way, the
    // assemblies right: the assemblies sit at mirrored places but are not
    // themselves mirrored.
    let count_fuel = |a: &AssembledModel| -> Result<usize, ModelError> {
        Ok(a.census()?
            .rows
            .iter()
            .filter(|r| r.material.starts_with("Fuel"))
            .map(|r| r.copies)
            .sum())
    };
    let mut seed = 1_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let name_at = |a: &AssembledModel, p: Position| {
        a.geometry
            .locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material)
            .map(|m| a.materials[m].name.clone())
    };
    let points: Vec<Position> = (0..100_000)
        .map(|_| {
            Position::new(
                (2.0 * prn() - 1.0) * r_core,
                (2.0 * prn() - 1.0) * r_core,
                0.0,
            )
        })
        .collect();
    let right_at: Vec<Option<String>> = points.iter().map(|&p| name_at(&assembled, p)).collect();
    let mut differing = [0_usize; 2];
    for (i, (core, assembly, label)) in [
        (
            RowOrder::Natural,
            RowOrder::Natural,
            "every lattice bottom-row-first",
        ),
        (
            RowOrder::Natural,
            RowOrder::TopDown,
            "only the core's lattice bottom-row-first",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let wrong = lct008_tutorial_model::build_ordered(data.clone(), |_| true, core, assembly)?
            .model
            .assemble()?;
        let n_fuel = count_fuel(&wrong)?;
        differing[i] = points
            .iter()
            .zip(&right_at)
            .filter(|(p, r)| name_at(&wrong, **p) != **r)
            .count();
        println!(
            "row-order trap, {label}: the census still counts {n_fuel} fuel pins; the material \
             differs from the right model at {} of {} points",
            differing[i],
            points.len()
        );
        assert_eq!(n_fuel, fuel_pins, "the census cannot see the mistake");
        if i == 1 {
            let img = wrong.draw_window(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [whole, whole],
                1400,
                "the TRAP: only the core's rows read bottom-first (from_natural_order)",
            );
            img.write_png(out.join("lct008_xy_wrong_order.png"))?;
            println!("drew {}", out.join("lct008_xy_wrong_order.png").display());
        }
    }
    assert_eq!(
        differing[0], 0,
        "this core is symmetric in y, and reading every lattice bottom-first mirrors the whole \
         core, so (a) changes nothing here"
    );
    assert!(
        differing[1] > points.len() / 100,
        "(b) is a visibly different core"
    );

    if geometry_only {
        println!(
            "--geometry-only: not running transport. Pictures in {}",
            out.display()
        );
        return Ok(());
    }

    // ── 5. Run ──────────────────────────────────────────────────────────────
    let t_load = Instant::now();
    let nuclides = assembled.load_nuclides_with(&data, Progress::Print)?;
    let data_s = t_load.elapsed().as_secs_f64();
    println!(
        "nuclear data: {} nuclides in {data_s:.1} s ({route})",
        nuclides.len()
    );
    let t_run = Instant::now();
    let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, None))?;
    let run_s = t_run.elapsed().as_secs_f64();
    println!("\n{run}");
    println!(
        "  wall time: data {data_s:.1} s, transport {run_s:.1} s ({} threads)\n  hardware: {}",
        rayon::current_num_threads(),
        HardwareInfo::detect().headline()
    );

    // ── 6. The checks (V&V) ─────────────────────────────────────────────────
    let k = run.keff();
    let sigma = k.std.max(run.provenance.sigma_batch_means.unwrap_or(0.0));
    let z = |a: f64, sa: f64, b: f64, sb: f64| (a - b) / (sa * sa + sb * sb).sqrt();
    let to_record = z(k.mean, sigma, RECORDED_K.0, RECORDED_K.1);
    println!(
        "\nk_eff = {k} (batch means sigma {:.5}); {:+.0} +- {:.0} pcm from the benchmark {} +- ~{} (the band is a stand-in)",
        run.provenance.sigma_batch_means.unwrap_or(f64::NAN),
        (k.mean - BENCHMARK.0) * 1e5,
        sigma * 1e5,
        BENCHMARK.0,
        BENCHMARK.1
    );
    println!(
        "  against the record {:.5} +- {:.5} (route 4, 32 seeds, 10 000 x [40 + 120]): {:+.0} pcm = {to_record:+.2} sigma (gate {K_SIGMA})",
        RECORDED_K.0,
        RECORDED_K.1,
        (k.mean - RECORDED_K.0) * 1e5
    );
    assert!(
        to_record.abs() <= K_SIGMA,
        "LCT-008 through the beginner API is {to_record:+.2} sigma from the record. \
         Find out what changed; do not widen this gate."
    );
    Ok(())
}
