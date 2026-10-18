//! # Tutorial 5: one HTR-10 fuel pebble, its TRISO packed at random and in a lattice
//!
//! **A double-heterogeneity region, built by hand** (beginner API
//! `outram_mc_libs::model`, epic gh #925; repeated structures #926; the
//! tutorial list and the decisions this follows on #943). Tutorial 4 built
//! one TRISO particle and repeated it as an infinite regular array. A fuel
//! pebble holds 8335 of them in a 2.5 cm sphere, inside a 0.5 cm graphite
//! shell. New in this lesson:
//!
//! - **`DhRegion`**: the fuel zone is built as its own object, a sphere of
//!   matrix graphite with particles packed in it (maintainer, #943: "a
//!   `DhRegion` is constructed from the outset"; the code never detects
//!   double heterogeneity by itself);
//! - **`Packing::Random { fraction, seed }`**: random sequential addition,
//!   the workspace's port of OpenMC's `pack_spheres`, through
//!   `dh_universe::pack_in_ball`, which keeps whole particles only and
//!   rescales its request until the fraction **inside the zone** is within
//!   0.2 % of the one asked for;
//! - **`Packing::LatticeOfCount { particles: 8335, offset }`**: a regular
//!   lattice whose pitch places exactly 8335 whole particles, as the HTR-10
//!   benchmark builds it (Şeker & Çolak 2003, p.266: "the number of full fuel
//!   particles inside a fuel ball is verified to be 8335"). Tutorial 5
//!   compares random against regular on the same pebble (maintainer, #943);
//! - **whole-particle rejection**: a particle that would poke out of the
//!   zone is never placed, and never cut;
//! - **`.delta_tracking()`**, required on every `DhRegion` (#943: no
//!   default): the zone is transported by delta (Woodcock) tracking against
//!   a majorant over its materials. Delta tracking is unbiased under any
//!   valid majorant; it is a choice of method, not of answer;
//! - **the census reports what the geometry holds**: the particle count and
//!   the packing fraction it realises, beside the one asked for.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble                  # MCSettings::Smoke (default)
//! cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble -- --fast        # MCSettings::Fast
//! cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble -- --auto        # MCSettings::Auto
//! cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble -- --geometry-only
//! cargo run --release -p outram-mc-libs --example tutorial_htr10_pebble -- --dh          # + DhUniverse, same particles
//! ```
//!
//! (Tutorials refactor existing examples where one fits (#943). None does,
//! checked: `htr10_pebble_delta_tracking.rs` runs `DhUniverse` with a
//! hand-made material table and records the boron study (a V&V role, kept);
//! `triso_delta_tracking.rs` is a LOW-tier cube; `htr10_fuel_zone_kinf.rs`
//! smears the coatings. This file reuses tutorial 4's particle and
//! materials, copied, and `pebble_beds::htr10`'s numbers.)
//!
//! ## The case: the HTR-10 fuel pebble, bare, reflective at its surface
//!
//! | region | outer radius \[cm\] | material |
//! |---|---|---|
//! | TRISO particle | 0.0455 | tutorial 4's five layers (`TrisoRadii::HTR10`) |
//! | fuel zone | 2.5 | matrix graphite, 1.73 g/cm³, + 1.3 ppm B per C atom; 8335 particles |
//! | shell | 3.0 | the same graphite, fuel-free |
//!
//! - **Sources:** Li, Yu & Wei (2014) Table 2 and IAEA-TECDOC-1382 Table 4-2
//!   (`pebble_beds::htr10`, `TrisoSpec::HTR10_LI2014`,
//!   `PebbleParams::htr10_li2014`). The packing fraction is derived from the
//!   count, `8335 (0.0455/2.5)^3 = 5.0248 %`, as tutorial 4 derived it.
//! - **The packing seed** is `PebbleParams::htr10_li2014()`'s, so the random
//!   arm holds exactly the particles `DhUniverse::pebble` packs for HTR-10
//!   (`--dh` runs that independent code on them).
//! - **The lattice offset** `[0.13, 0.37, 0.71]` is the HTR-10 core's
//!   (`nee_soon::htr10_rmc::core_shell::TrisoBuild`, gh:#430): a symmetric
//!   offset moves the count in shells and cannot reach 8335.
//! - **Reflective at the pebble surface**: this is the bare pebble's own
//!   k-infinity, like `PebbleParams::htr10_li2014()`, with no helium (it is
//!   neutronically negligible) and no neighbours. Not a whole-core number.
//! - **Temperature:** 293.6 K everywhere, the tutorials' room temperature
//!   (the HTR-10 records below are at 300.15 K).
//!
//! ## What this checks (V&V methodology)
//!
//! Each check can fail; nothing is tuned.
//!
//! 1. **Counted from the geometry:** the census places exactly 8335 particles
//!    on the lattice, and the random packing realises 5.0248 % within 0.2 %.
//! 2. **No overlap in the random packing:** the closest pair of centres is at
//!    least one particle diameter apart.
//! 3. **Pictures** of the assembled geometry (crate HARD RULE), written to
//!    `target/tutorials/htr10_pebble/` (gitignored).
//! 4. **k against the crate's records** for this pebble (none is this exact
//!    model; see Results).
//!
//! ## Results
//!
//! 2026-10-18, ENDF/B-VIII.0 from `reference-data/endf/`, `MCSettings::Smoke`
//! (seed 1), `--dh`. Hardware: Intel i9-13900K, pinned to 4 of its 32
//! logical cores, transport on one thread, 62.5 GiB RAM, Linux, CPU only;
//! the machine was shared with other jobs.
//!
//! | check | result |
//! |---|---|
//! | 1. census, regular | **8335** particles, pitch 0.1951235 cm, 27 x 26 x 27 grid, packing fraction 5.024811 % |
//! | 1. census, random | **8345** particles, 5.03084 % realised against 5.02481 % asked (+0.12 %, inside the 0.2 %) |
//! | 2. no overlap | closest centres 0.09100 cm apart = one particle diameter (RSA allows touching) |
//! | k, random | **1.68283 ± 0.00515** (batch means 0.00382); 95 inactive + 50 active cycles of 1000 |
//! | k, regular | **1.69048 ± 0.00515** (batch means 0.00351); 85 inactive + 50 active |
//! | regular − random | +765 ± 728 pcm (+1.05 sigma): not resolved |
//! | random against `DhUniverse::pebble`, same 8345 particles | DhUniverse 1.68884 ± 0.00596; model − DhUniverse −600 pcm (−0.76 sigma) |
//!
//! - **SMOKE RUN, not a result to quote**: 1000 per cycle and 50 active
//!   cycles (the run's own note). sigma is about 500 pcm.
//! - **Against the crate's nearest record**, nee_soon's `DhUniverse` HTR-10
//!   pebble 1.68517 ± 0.00266 (2026-10-13, 300.15 K, 30P graphite, helium shell
//!   to 3.7221 cm; `nee_soon/tests/htr10_single_pebble_vs_dh_universe.rs`):
//!   random −234 pcm (−0.40 sigma), regular +531 pcm (+0.92 sigma). Not the
//!   same model (temperature, LEAPR-made laws against tapes, the helium gap),
//!   so this is consistency at Smoke's sigma, not a comparison of physics.
//!   `examples/htr10_pebble_delta_tracking.rs`'s 1.68515 ± 0.00178
//!   (2026-09-16) used crystalline graphite and weight-ppm boron, so it is
//!   further from this model and is not compared.
//! - **Predictions, posted on #943 before the runs**: both arms within 2
//!   sigma of 1.68517 (held), regular − random not resolved (held), model
//!   against `DhUniverse` on the same particles consistent with 0 (held).
//! - **Wall time**: data 115 s (13 nuclides, parallel on 4 threads);
//!   transport 848 s (random) and 530 s (regular) single-threaded, about
//!   5.8 and 3.9 ms per history; `DhUniverse` (its own delta tracking, the
//!   same size) 57 s. The CSG delta tracking still stops at every internal
//!   surface (`physics::delta_tracking::handoff`), so it is not the fast
//!   path; `DhUniverse` is 15x faster on this pebble.

use std::path::{Path, PathBuf};
use std::time::Instant;

use outram_mc_libs::dh_universe::{pack_in_ball, DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::triso_particle::TrisoRadii;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{Arrangement, NuclideKey};
use outram_mc_libs::pebble_beds::htr10;
use outram_mc_libs::perf_report::HardwareInfo;
use outram_mc_libs::physics::keff::KeffSettings as LowKeffSettings;
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::{part_per_million, ratio};
use uom::si::thermodynamic_temperature::kelvin;

/// HTR-10's fuel zone and pebble \[cm\] (Li 2014 Table 2) and particle count.
const FUEL_ZONE_R_CM: f64 = 2.5;
const PEBBLE_R_CM: f64 = 3.0;
const PARTICLES_PER_PEBBLE: usize = 8335;
/// The HTR-10 core lattice's offset (gh:#430).
const OFFSET: [f64; 3] = [0.13, 0.37, 0.71];
const T_K: f64 = 293.6;

/// The materials of tutorial 4, copied (`pebble_beds::htr10`'s numbers).
struct Materials {
    uo2: Material,
    buffer: Material,
    pyc: Material,
    sic: Material,
    matrix: Material,
    shell: Material,
}

fn materials() -> Materials {
    let g_cc = MassDensity::new::<gram_per_cubic_centimeter>;
    let ppm = Ratio::new::<part_per_million>;
    let graphite_law = Moderator::Graphite(GraphiteKind::ReactorPorosity30);
    let boron_in_carbon = Contaminant::ppm(
        ppm(htr10::B_PPM_GRAPHITE),
        Basis::Atom,
        PpmOf::PerHostAtom,
        Placement::AddOn,
    );
    let carbon = |rho: f64, name: &str| {
        Material::from_fractions(
            Basis::Atom,
            [(Element::Carbon.natural(), Ratio::new::<ratio>(1.0))],
        )
        .density(g_cc(rho))
        .contaminant(Element::Boron.natural(), boron_in_carbon)
        .bound_in(graphite_law)
        .named(name)
    };
    let uo2 = Material::fuel(
        HeavyMetal::Uranium(Enrichment::by_weight(
            FissileUranium::U235,
            Ratio::new::<ratio>(htr10::ENRICHMENT_WT),
        )),
        FuelForm::Oxide,
    )
    .density(g_cc(htr10::RHO_UO2))
    .contaminant(
        Element::Boron.natural(),
        Contaminant::ppm(
            ppm(htr10::B_PPM_URANIUM),
            Basis::Atom,
            PpmOf::HeavyMetal,
            Placement::AddOn,
        ),
    )
    .named("UO2 kernel");
    let sic = Material::from_fractions(
        Basis::Atom,
        [
            (Element::Silicon.natural(), Ratio::new::<ratio>(0.5)),
            (Element::Carbon.natural(), Ratio::new::<ratio>(0.5)),
        ],
    )
    .density(g_cc(htr10::RHO_SIC))
    .bound_in(Moderator::SiliconCarbide)
    .named("silicon carbide");
    Materials {
        uo2,
        buffer: carbon(htr10::RHO_BUFFER, "buffer carbon"),
        pyc: carbon(htr10::RHO_PYC, "pyrolytic carbon"),
        sic,
        matrix: carbon(htr10::RHO_GRAPHITE, "matrix graphite"),
        shell: carbon(htr10::RHO_GRAPHITE, "shell graphite"),
    }
}

/// Which way the particles are packed.
#[derive(Clone, Copy, PartialEq)]
enum Arm {
    Random,
    Regular,
}

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Arm::Random => "random",
            Arm::Regular => "regular",
        }
    }
}

/// Build the pebble model for one arm. Everything but the `Packing` line is
/// the same for both.
fn pebble_model(data: NuclearData, arm: Arm) -> (Model, [Component; 4]) {
    let m = materials();
    let mut model = Model::new(data);
    let kernel_c = model.component("Kernel");
    let coatings_c = model.component("Coatings");
    let matrix_c = model.component("Matrix");
    let shell_c = model.component("Shell");

    // ── The particle: tutorial 4's, kernel outwards, one .containing each ──
    let r = TrisoRadii::HTR10;
    let cm = Length::new::<centimeter>;
    let kernel = Sphere::radius(cm(r.kernel))
        .filled_with(m.uo2)
        .in_component(kernel_c);
    let buffer = Sphere::radius(cm(r.buffer))
        .filled_with(m.buffer)
        .in_component(coatings_c)
        .containing(kernel);
    let ipyc = Sphere::radius(cm(r.ipyc))
        .filled_with(m.pyc.clone())
        .in_component(coatings_c)
        .containing(buffer);
    let sic = Sphere::radius(cm(r.sic))
        .filled_with(m.sic)
        .in_component(coatings_c)
        .containing(ipyc);
    let particle = Sphere::radius(cm(r.opyc))
        .filled_with(m.pyc)
        .in_component(coatings_c)
        .containing(sic);

    // ── The fuel zone: a DhRegion, built by hand ────────────────────────────
    let pf = PARTICLES_PER_PEBBLE as f64 * (r.opyc / FUEL_ZONE_R_CM).powi(3);
    let packing = match arm {
        Arm::Random => Packing::Random {
            fraction: Ratio::new::<ratio>(pf),
            seed: PebbleParams::htr10_li2014().seed,
        },
        Arm::Regular => Packing::LatticeOfCount {
            particles: PARTICLES_PER_PEBBLE,
            offset: OFFSET,
        },
    };
    let fuel_zone = DhRegion::new(Sphere::radius(cm(FUEL_ZONE_R_CM)), m.matrix)
        .in_component(matrix_c)
        .with_particles(particle, packing)
        .delta_tracking();

    // ── The pebble: a graphite shell around the zone, mirrored outside ─────
    let pebble = Sphere::radius(cm(PEBBLE_R_CM))
        .filled_with(m.shell)
        .in_component(shell_c)
        .containing(fuel_zone)
        .boundary(Boundary::Reflective);
    model.add(pebble);
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(T_K),
    ));
    (model, [kernel_c, coatings_c, matrix_c, shell_c])
}

/// A point inside a kernel near the centre of the plane `z`, by scanning the
/// assembled geometry (so the zoom is on what the solver sees), and the
/// centroid of the kernel around it.
fn find_kernel(a: &AssembledModel, z: f64) -> Option<Position> {
    let uo2 = a.resolved.iter().position(|r| r.name == "UO2 kernel")?;
    let at = |x: f64, y: f64| {
        a.geometry
            .locate(
                Position::new(x, y, z),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE,
            )
            .and_then(|p| p.material)
    };
    let step = 0.004;
    let mut first = None;
    'scan: for i in 0..125 {
        for j in 0..125 {
            let (x, y) = (i as f64 * step, j as f64 * step);
            if at(x, y) == Some(uo2) {
                first = Some((x, y));
                break 'scan;
            }
        }
    }
    let (x0, y0) = first?;
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
    for i in 0..60 {
        for j in 0..60 {
            let (x, y) = (x0 - 0.06 + i as f64 * 0.002, y0 - 0.06 + j as f64 * 0.002);
            if at(x, y) == Some(uo2) {
                sx += x;
                sy += y;
                n += 1.0;
            }
        }
    }
    Some(Position::new(sx / n, sy / n, z))
}

/// The pictures of one arm (crate HARD RULE: draw what the solver sees).
fn draw(a: &AssembledModel, arm: Arm, pitch: f64, out: &Path) -> std::io::Result<Vec<PathBuf>> {
    // Slices through a plane of particle centres of the regular lattice: a
    // slice through the pebble's centre misses every one of them, because
    // the offset puts the nearest centres 0.29 pitch below z = 0 and 0.37
    // pitch beside y = 0 (further than a particle radius).
    let z = (OFFSET[2] - 1.0) * pitch;
    let y = OFFSET[1] * pitch;
    let w = 2.2 * PEBBLE_R_CM;
    let name = arm.name();
    let mut files = Vec::new();
    let mut save = |img: outram_mc_libs::geometry::plot::ImageData, file: String| {
        let p = out.join(file);
        img.write_png(&p).map(|_| {
            println!("  drew {}", p.display());
            files.push(p);
        })
    };
    save(
        a.draw_window(
            PlotBasis::Xy,
            Position::new(0.0, 0.0, z),
            [w, w],
            1000,
            &format!("HTR-10 pebble, {name} packing: x-y at z = {z:.4} cm"),
        ),
        format!("{name}_pebble_xy.png"),
    )?;
    save(
        a.draw_window(
            PlotBasis::Xz,
            Position::new(0.0, y, 0.0),
            [w, w],
            1000,
            &format!("HTR-10 pebble, {name} packing: x-z at y = {y:.4} cm"),
        ),
        format!("{name}_pebble_xz.png"),
    )?;
    save(
        a.draw_window(
            PlotBasis::Xy,
            Position::new(FUEL_ZONE_R_CM - 0.2, 0.0, z),
            [0.8, 0.8],
            800,
            &format!("{name}: the fuel-zone edge (x = 2.5 cm), whole particles only"),
        ),
        format!("{name}_zone_edge_xy.png"),
    )?;
    if let Some(c) = find_kernel(a, z) {
        save(
            a.draw_window(
                PlotBasis::Xy,
                c,
                [0.12, 0.12],
                800,
                &format!(
                    "{name}: one particle near ({:.3}, {:.3}, {:.3}) cm, 0.12 cm window",
                    c.x, c.y, c.z
                ),
            ),
            format!("{name}_particle_xy.png"),
        )?;
    }
    Ok(files)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let preset = if has("--auto") {
        MCSettings::Auto
    } else if has("--fast") {
        MCSettings::Fast
    } else {
        // Smoke by default (maintainer, 2026-10-18: smoke for tutorials).
        MCSettings::Smoke
    };
    let data = match NuclearData::workspace_reference() {
        Ok(d) => d,
        Err(e) => {
            println!("No ENDF tapes: {e}. Clone with the reference data.");
            return Ok(());
        }
    };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/htr10_pebble");
    std::fs::create_dir_all(&out)?;

    let mut runs = Vec::new();
    for arm in [Arm::Random, Arm::Regular] {
        println!("\n══ {} packing ══", arm.name());
        let (model, [kernel_c, coatings_c, matrix_c, shell_c]) = pebble_model(data.clone(), arm);
        let t = Instant::now();
        let assembled = model.assemble()?;
        println!(
            "  assembled in {:.1} s: {} surfaces, {} cells, {} universes",
            t.elapsed().as_secs_f64(),
            assembled.geometry.surfaces.len(),
            assembled.geometry.cells.len(),
            assembled.geometry.universes.len()
        );

        // ── Count what the geometry holds ───────────────────────────────────
        let census = assembled.census()?;
        let p = &census.packings[0];
        println!(
            "  census: {} particles; packing fraction {:.5} % realised, {:.5} % asked; kernels {}",
            p.particles,
            100.0 * p.realised.get::<ratio>(),
            100.0 * p.requested.get::<ratio>(),
            census.copies(kernel_c)
        );
        for (c, label) in [
            (kernel_c, "kernel"),
            (coatings_c, "coatings"),
            (matrix_c, "matrix"),
            (shell_c, "shell"),
        ] {
            println!(
                "    {label:<9} {:.6} cm3",
                census.volume(c).get::<uom::si::volume::cubic_centimeter>()
            );
        }
        let pf = PARTICLES_PER_PEBBLE as f64 * (TrisoRadii::HTR10.opyc / FUEL_ZONE_R_CM).powi(3);
        let pitch = match (arm, p.arrangement) {
            (Arm::Regular, Arrangement::RegularLattice { pitch_cm, .. }) => {
                assert_eq!(p.particles, PARTICLES_PER_PEBBLE, "8335 whole particles");
                pitch_cm[0]
            }
            (Arm::Random, Arrangement::Random { .. }) => {
                let rel = p.realised.get::<ratio>() / pf - 1.0;
                println!(
                    "  random: realised − asked = {:+.3} % of the fraction (pack_in_ball's tolerance is 0.2 %)",
                    100.0 * rel
                );
                assert!(rel.abs() <= 2.0e-3, "packing fraction off by {rel}");
                // The closest pair: the same packer call, read for its gap.
                let packed = pack_in_ball(
                    TrisoRadii::HTR10.opyc,
                    FUEL_ZONE_R_CM,
                    pf,
                    PebbleParams::htr10_li2014().seed,
                )?;
                let gap = packed.packing.min_center_distance().unwrap_or(f64::NAN);
                println!(
                    "  closest centres {gap:.5} cm apart (particle diameter {:.5} cm); {} particles from the packer, {} in the geometry",
                    2.0 * TrisoRadii::HTR10.opyc,
                    packed.packing.len(),
                    p.particles
                );
                assert!(gap >= 2.0 * TrisoRadii::HTR10.opyc, "particles overlap");
                assert_eq!(packed.packing.len(), p.particles);
                // Draw the random arm in the regular arm's planes.
                regular_pitch()?
            }
            _ => unreachable!("each arm lowers to its own arrangement"),
        };
        println!("\n{census}");

        // ── Look at it before running it (crate HARD RULE) ─────────────────
        draw(&assembled, arm, pitch, &out)?;
        runs.push((arm, model, assembled));
    }
    if has("--geometry-only") {
        return Ok(());
    }

    // ── Run k-infinity of each arm (no diagnostics: a delta-tracked region
    //    scores none) ───────────────────────────────────────────────────────
    let mut ks = Vec::new();
    let mut loaded: Option<(
        Vec<NuclideKey>,
        Vec<outram_mc_libs::material::nuclide::Nuclide>,
    )> = None;
    for (arm, model, assembled) in &runs {
        let t = Instant::now();
        // Both arms need the same nuclides at the same temperature: process
        // them once.
        let nuclides = match loaded.take() {
            Some((keys, n)) if keys == assembled.nuclides => n,
            _ => assembled.load_nuclides(model.data())?,
        };
        let data_s = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, None))?;
        let run_s = t.elapsed().as_secs_f64();
        println!("\n{} packing:\n{run}", arm.name());
        println!(
            "  wall time: data {data_s:.1} s ({} nuclides, parallel), transport {run_s:.1} s single-threaded\n  hardware: {}",
            nuclides.len(),
            HardwareInfo::detect().headline()
        );
        ks.push((*arm, run.keff(), run.provenance.sigma_batch_means));
        let dh_nuclides = nuclides;
        if *arm == Arm::Random && has("--dh") {
            // The same particles, through independent code: DhUniverse's own
            // packing call and point query, and its own delta-tracked power
            // iteration, at Smoke's size.
            let lm = &assembled.materials;
            let name = |n: &str| lm.iter().position(|m| m.name == n).expect("material");
            let table: Vec<_> = [
                "UO2 kernel",
                "buffer carbon",
                "pyrolytic carbon",
                "silicon carbide",
                "pyrolytic carbon",
                "matrix graphite",
                "shell graphite",
            ]
            .iter()
            .map(|n| lm[name(n)].clone())
            .collect();
            let dh = DhUniverse::pebble(
                PebbleParams::htr10_li2014().with_materials(table),
                DhTreatment::DeltaTracking,
            )?;
            let p = &run.provenance;
            let low = LowKeffSettings {
                n_particles: p.active_particles,
                n_inactive: p.inactive_cycles,
                n_active: p.active_cycles,
                temperature_k: T_K,
                seed: p.seed,
                ..LowKeffSettings::default()
            };
            let t = Instant::now();
            let r = dh.keff(&dh_nuclides, &low);
            println!(
                "  DhUniverse (same particles, {} of them): k = {:.5} ± {:.5} ({:.1} s single-threaded)",
                dh.particle_count(),
                r.k_mean,
                r.k_std,
                t.elapsed().as_secs_f64()
            );
            let z =
                (run.keff().mean - r.k_mean) / (run.keff().std.powi(2) + r.k_std.powi(2)).sqrt();
            println!(
                "  model − DhUniverse = {:+.0} pcm ({z:+.2} sigma)",
                (run.keff().mean - r.k_mean) * 1e5
            );
        }
        loaded = Some((assembled.nuclides.clone(), dh_nuclides));
    }

    // ── Read the results ───────────────────────────────────────────────────
    let z = |a: f64, sa: f64, b: f64, sb: f64| (a - b) / (sa * sa + sb * sb).sqrt();
    if let [(_, kr, _), (_, kg, _)] = ks.as_slice() {
        println!(
            "\nregular − random = {:+.0} ± {:.0} pcm ({:+.2} sigma)",
            (kg.mean - kr.mean) * 1e5,
            (kg.std.powi(2) + kr.std.powi(2)).sqrt() * 1e5,
            z(kg.mean, kg.std, kr.mean, kr.std)
        );
        // The crate's nearest records (none is this model; see the docs):
        // nee_soon's DhUniverse pebble with a helium shell to 3.7221 cm,
        // 300.15 K, 30P graphite: 1.68517 ± 0.00266 (2026-10-13).
        let (rec, srec) = (1.68517, 0.00266);
        for (arm, k, _) in &ks {
            println!(
                "  {}: k_inf = {k}; against nee_soon's DhUniverse pebble record {rec} ± {srec}: {:+.0} pcm ({:+.2} sigma)",
                arm.name(),
                (k.mean - rec) * 1e5,
                z(k.mean, k.std, rec, srec)
            );
        }
    }
    Ok(())
}

/// The regular arm's pitch (the planes both arms are drawn in), found the
/// way the regular arm finds it.
fn regular_pitch() -> Result<f64, ModelError> {
    let m = materials();
    let particle =
        Sphere::radius(Length::new::<centimeter>(TrisoRadii::HTR10.opyc)).filled_with(m.pyc);
    let cube = Cube::for_whole_particles_in_sphere(
        &particle,
        Sphere::radius(Length::new::<centimeter>(FUEL_ZONE_R_CM)),
        PARTICLES_PER_PEBBLE,
        OFFSET,
    )?;
    Ok(cube.sides_cm()[0])
}
