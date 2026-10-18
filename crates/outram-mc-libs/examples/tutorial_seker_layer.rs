//! # Tutorial 7: one Şeker layer of the HTR-10 pebble bed, an infinite stack of it
//!
//! **A hexagonal lattice of pebbles** (beginner API `outram_mc_libs::model`,
//! epic gh #925; repeated structures #926; the tutorial list and the decisions
//! this follows on #943). Tutorial 5 built one fuel pebble. HTR-10's bed is
//! many of them, packed with moderator ("dummy") graphite balls; Şeker &
//! Çolak (2003) model the packing as a **hexagonal lattice** of one tile, 13
//! balls per tile, and that lattice is what this lesson builds. New here:
//!
//! - **`HexPrism`** and **`Lattice::hex(tile)`**: a hexagonal tile repeated
//!   without end (a new solid and a new lattice; they lower to the CSG
//!   `HexLattice`, one ring);
//! - **a ball may cross the tile's faces**: list each ball once, in the tile
//!   that holds its centre; the lattice draws the piece that sticks into a
//!   neighbour in that neighbour too. A ball is never cut (a cut pebble is
//!   wrong physics, maintainer 2026-10-01). Şeker's tile owns 13 balls and
//!   draws pieces of 23;
//! - **an infinite stack**: one layer, 9.798 cm high, in a prism of its own
//!   size with `Boundary::Reflective` on every face, so the result is the
//!   bed's **k-infinity** at Şeker's packing, not a core.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_seker_layer                  # MCSettings::Smoke (default)
//! cargo run --release -p outram-mc-libs --example tutorial_seker_layer -- --fast        # MCSettings::Fast
//! cargo run --release -p outram-mc-libs --example tutorial_seker_layer -- --auto        # MCSettings::Auto
//! cargo run --release -p outram-mc-libs --example tutorial_seker_layer -- --geometry-only
//! cargo run --release -p outram-mc-libs --example tutorial_seker_layer -- --fuel-balls 8   # ablation: 8 of 13 fuel
//! ```
//!
//! (Tutorials refactor existing examples where one fits (#943). None does,
//! checked: the Şeker bed is built by `nee_soon::htr10_rmc` (V&V role, kept),
//! and its one-tile cell `nee_soon::htr10_mgxs::cells::Htr10Cell::seker_cluster(1)`
//! is a low-level builder of the same geometry that `nee_soon` owns and
//! `outram-mc-libs` cannot call. The statements are in
//! `examples/common/seker_layer_model.rs`, shared with
//! `tests/model_seker_layer.rs`, which pins them to a hand-built CSG.)
//!
//! ## The case
//!
//! | what | value | source |
//! |---|---|---|
//! | tile | hexagonal prism, faces perpendicular to y, pitch 16.392 cm (apothem 8.196), height 9.798 cm | Şeker & Çolak 2003 p.266-267; `SekerCell::from_paper` (gh:#472) |
//! | balls | 13 per tile: 7 on the bottom face (a flower), 3 `Inner`, 3 `Outer`; pieces of 23 drawn | same |
//! | fuel ball | tutorial 5's: shell 3.0 cm, fuel zone 2.5 cm, 8335 TRISO particles (regular lattice) | Li, Yu & Wei 2014 Table 2 |
//! | moderator ball | graphite sphere, 3.0 cm | `htr10_rmc` |
//! | fuel : moderator | **7 : 6** by the core's rule on the 13 (paper: 0.57:0.43) | Şeker p.267; `SekerBed::assign` |
//! | helium | ideal gas, 101.33 kPa (an assumption, gh:#426) | `htr10_rmc::materials` |
//! | temperature | 300.15 K | Li 2014; the crate's HTR-10 records |
//!
//! ## What this checks (V&V methodology)
//!
//! Each check can fail; nothing is tuned.
//!
//! 1. **Counted from the geometry:** the census counts the balls (13), the
//!    fuel balls (7) and particles (7 x 8335) from the assembled universe
//!    graph, and the packing fraction they make against Şeker's 0.6448.
//! 2. **The pieces are present:** 23 sphere surfaces (13 owned balls, 10
//!    pieces), 8 planes on the container.
//! 3. **Pictures** of the assembled geometry (crate HARD RULE), written to
//!    `target/tutorials/seker_layer/` (gitignored).
//! 4. **k-infinity against the crate's records** for this cell (see Results).
//!
//! ## Results
//!
//! See the tutorial page `docs/tutorial/src/tutorial7-seker-layer.md` and the
//! comment on gh #943 for the run of record; this file prints every number.
//! Hardware for any timing: `HardwareInfo::headline()` is printed with it.

use std::path::{Path, PathBuf};
use std::time::Instant;

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::perf_report::HardwareInfo;
use uom::si::volume::cubic_centimeter;

#[path = "common/seker_layer_model.rs"]
#[allow(dead_code)]
mod seker_layer_model;
use seker_layer_model as seker;

/// nee_soon's pilot for exactly this cell (`Htr10Cell::seker_cluster(1)`:
/// the same tile in a reflective hexagonal prism one tile high), 2000 x
/// [10 + 40], seed 20 261 012, 300.15 K, 2026-10-13, 4-core Xeon @ 2.10 GHz,
/// `nee_soon/verification_and_validation/htr10_mgxs_cells/README.md` section
/// 4. A pilot, not a record. It holds 13 fuel and 10 moderator PIECES (23
/// sites assigned independently, 0.576 of the ball volume fuel), where this
/// model holds 7 fuel and 6 moderator balls (0.538).
const PILOT_K: f64 = 1.74844;
const PILOT_SIGMA: f64 = 0.00309;
/// nee_soon's `DhUniverse` HTR-10 single pebble, the same fuel ball alone
/// (tutorial 5): 1.68517 +- 0.00266 (2026-10-13).
const PEBBLE_K: f64 = 1.68517;
/// The finite core's k at 20 Şeker layers (N = 20, 199.543 cm), ENDF/B-VIII.0,
/// 10 000 x [5 + 135] (`htr10_seker_2026_10_07_10k`): 1.160230 +- 0.001066.
const CORE_K_N20: f64 = 1.160230;

fn cm(x: f64) -> uom::si::f64::Length {
    uom::si::f64::Length::new::<uom::si::length::centimeter>(x)
}

/// The TRISO lattice pitch the fuel zone finds (the planes pictures are
/// drawn in), found the way the geometry finds it.
fn triso_pitch(m: &seker::Materials, c: &seker::Components) -> Result<f64, ModelError> {
    let particle = seker::triso_particle(m, c);
    let cube = Cube::for_whole_particles_in_sphere(
        &particle,
        Sphere::radius(cm(seker::FUEL_ZONE_R_CM)),
        seker::PARTICLES_PER_PEBBLE,
        seker::OFFSET,
    )?;
    Ok(cube.sides_cm()[0])
}

/// A point inside a kernel of the ball at `centre`, by scanning the assembled
/// geometry on the plane `z` (so the zoom is on what the solver sees), and the
/// centroid of the kernel around it.
fn find_kernel(a: &AssembledModel, centre: [f64; 3], z: f64) -> Option<Position> {
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
            let (x, y) = (centre[0] + i as f64 * step, centre[1] + j as f64 * step);
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

/// The pictures (crate HARD RULE: draw what the solver sees, from the
/// assembled geometry).
fn draw(
    a: &AssembledModel,
    sites: &[seker::Site],
    fuel: &[bool],
    pitch_t: f64,
    out: &Path,
) -> std::io::Result<Vec<PathBuf>> {
    let h = seker::height_cm();
    let apothem = seker::apothem_cm();
    let circum = apothem / (std::f64::consts::PI / 6.0).cos();
    let n_fuel = fuel.iter().filter(|f| **f).count();
    let n_mod = fuel.len() - n_fuel;
    // The TRISO lattice puts layers of particle centres at (k + 0.71) pitch
    // from a ball's centre, none on its centre plane: draw the central plane
    // 0.0566 cm below it, and the basal plane 0.1385 cm above the bottom face.
    let z_central = (seker::OFFSET[2] - 1.0) * pitch_t;
    let z_basal = -0.5 * h + seker::OFFSET[2] * pitch_t;
    let w = 2.0 * circum * 1.08;
    let mut files = Vec::new();
    let mut save = |img: outram_mc_libs::geometry::plot::ImageData, file: &str| {
        let p = out.join(file);
        img.write_png(&p).map(|_| {
            println!("  drew {}", p.display());
            files.push(p);
        })
    };
    save(
        a.draw_window(
            PlotBasis::Xy,
            Position::new(0.0, 0.0, z_central),
            [w, w],
            1100,
            &format!(
                "Seker tile, central plane z = {z_central:.4} cm: 6 balls own ({} fuel, {} moderator), 3 enter from the neighbours",
                fuel[7..].iter().filter(|f| **f).count(),
                fuel[7..].iter().filter(|f| !**f).count()
            ),
        ),
        "seker_xy_central.png",
    )?;
    save(
        a.draw_window(
            PlotBasis::Xy,
            Position::new(0.0, 0.0, z_basal),
            [w, w],
            1100,
            &format!(
                "Seker tile, basal plane z = {z_basal:.4} cm: the 7-ball flower ({} fuel, {} moderator)",
                fuel[..7].iter().filter(|f| **f).count(),
                fuel[..7].iter().filter(|f| !**f).count()
            ),
        ),
        "seker_xy_basal.png",
    )?;
    // x-z through the flower's centre row (y = 0) and through the Inner(0) ball
    // (y = d/sqrt(3)).
    // Square windows: `draw_window` has one pixel count for both axes, so a
    // wide-and-short window would stretch the balls into ellipses.
    let wz = [w, w];
    save(
        a.draw_window(
            PlotBasis::Xz,
            Position::new(0.0, 0.0, 0.0),
            wz,
            1100,
            &format!(
                "Seker layer x-z at y = 0, height {h:.3} cm: flower balls are hemispheres on the top and bottom faces ({n_fuel} fuel, {n_mod} moderator balls in all)"
            ),
        ),
        "seker_xz_y0.png",
    )?;
    let y_inner = seker::BALL_DIAMETER_CM / 3.0_f64.sqrt();
    save(
        a.draw_window(
            PlotBasis::Xz,
            Position::new(0.0, y_inner, 0.0),
            wz,
            1100,
            &format!("Seker layer x-z at y = {y_inner:.4} cm, through the Inner(0) ball and the Outer(1) ball"),
        ),
        "seker_xz_inner.png",
    )?;
    // Zoom: the first fuel ball whose centre is in the central plane, at the
    // TRISO layer, and one particle in it.
    if let Some((i, _)) = sites
        .iter()
        .zip(fuel)
        .enumerate()
        .find(|(_, (s, f))| **f && !matches!(s, seker::Site::Flower(_)))
    {
        let c = seker::site_centre(sites[i]);
        let zc = c[2] + (seker::OFFSET[2] - 1.0) * pitch_t;
        save(
            a.draw_window(
                PlotBasis::Xy,
                Position::new(c[0], c[1], zc),
                [7.0, 7.0],
                1000,
                &format!(
                    "{:?}: one fuel ball at ({:.3}, {:.3}, {:.3}) cm, TRISO layer z = {zc:.4}",
                    sites[i], c[0], c[1], c[2]
                ),
            ),
            "seker_fuel_ball_xy.png",
        )?;
        if let Some(k) = find_kernel(a, [c[0] - 0.3, c[1] - 0.3, 0.0], zc) {
            save(
                a.draw_window(
                    PlotBasis::Xy,
                    k,
                    [0.12, 0.12],
                    800,
                    &format!(
                        "one TRISO particle in that ball, near ({:.3}, {:.3}, {:.3}) cm, 0.12 cm window",
                        k.x, k.y, k.z
                    ),
                ),
                "seker_triso_xy.png",
            )?;
        }
    }
    Ok(files)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
    };
    let preset = if has("--auto") {
        MCSettings::Auto
    } else if has("--fast") {
        MCSettings::Fast
    } else {
        // Smoke by default (maintainer, 2026-10-18: smoke for tutorials).
        MCSettings::Smoke
    };
    let fuel = match value("--fuel-balls") {
        Some(n) if n <= 13 => seker::fuel_flags_for_count(n),
        Some(n) => return Err(format!("--fuel-balls {n}: the tile owns 13 balls").into()),
        None => seker::fuel_flags(seker::FUEL_BALL_FRACTION),
    };
    let data = match NuclearData::workspace_reference() {
        Ok(d) => d,
        Err(e) => {
            println!("No ENDF tapes: {e}. Clone with the reference data.");
            return Ok(());
        }
    };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/seker_layer");
    std::fs::create_dir_all(&out)?;

    // ── Build ──────────────────────────────────────────────────────────────
    let mats = seker::materials(seker::T_K);
    let layer = seker::build(data, &mats, &seker::FuelBall::Triso, &fuel, seker::T_K);
    let c = layer.components;
    let t = Instant::now();
    let assembled = layer.model.assemble()?;
    println!(
        "assembled in {:.1} s: {} surfaces, {} cells, {} universes, {} lattice(s)",
        t.elapsed().as_secs_f64(),
        assembled.geometry.surfaces.len(),
        assembled.geometry.cells.len(),
        assembled.geometry.universes.len(),
        assembled.geometry.lattices.len()
    );

    // ── Count what the geometry holds ──────────────────────────────────────
    // Ball surfaces: the spheres of the ball's own radius (the TRISO particle's
    // shells and the fuel zone have others).
    let ball_r = 0.5 * seker::BALL_DIAMETER_CM;
    let spheres = assembled
        .geometry
        .surfaces
        .iter()
        .filter(|s| matches!(s, SurfaceKind::Sphere(sp) if (sp.r - ball_r).abs() < 1e-9))
        .count();
    let planes = assembled
        .geometry
        .surfaces
        .iter()
        .filter(|s| matches!(s, SurfaceKind::Plane(_) | SurfaceKind::ZPlane(_)))
        .count();
    let census = assembled.census()?;
    let n_fuel = fuel.iter().filter(|f| **f).count();
    let fuel_balls = census.copies(c.shell);
    let moderator_balls = census.copies(c.moderator);
    let particles: usize = census.packings.iter().map(|p| p.particles).sum();
    let tile_volume = census.total_volume().get::<cubic_centimeter>();
    let helium_volume = census.volume(c.helium).get::<cubic_centimeter>();
    let packing = 1.0 - helium_volume / tile_volume;
    let ball_v = 4.0 / 3.0 * std::f64::consts::PI * (0.5 * seker::BALL_DIAMETER_CM).powi(3);
    let expected_packing =
        13.0 * ball_v / (2.0 * 3.0_f64.sqrt() * seker::apothem_cm().powi(2) * seker::height_cm());
    println!("\n{census}");
    println!(
        "\ncounted from the assembled geometry:\n  balls: {} fuel + {} moderator = {}  (tile owns 13; {} fuel asked)\n  ball surfaces {spheres} (13 owned balls + {} pieces from the neighbours and the layer above), container planes {planes} (6 faces + 2 ends)\n  TRISO particles: {particles} (= {fuel_balls} x {})\n  tile volume {tile_volume:.4} cm3 (hexagon x height), helium {helium_volume:.4} cm3\n  packing fraction {:.6} (Seker's interior value {:.4}; 13 balls over the tile, {expected_packing:.6})",
        fuel_balls,
        moderator_balls,
        fuel_balls + moderator_balls,
        n_fuel,
        spheres.saturating_sub(13),
        seker::PARTICLES_PER_PEBBLE,
        packing,
        seker::SEKER_INTERIOR_PACKING,
    );
    assert_eq!(fuel_balls, n_fuel, "fuel balls counted from the geometry");
    assert_eq!(fuel_balls + moderator_balls, 13, "13 balls per tile");
    assert_eq!(spheres, 23, "pieces of 23 balls");
    assert_eq!(planes, 8);
    assert_eq!(particles, n_fuel * seker::PARTICLES_PER_PEBBLE);
    assert!(
        (packing / expected_packing - 1.0).abs() < 1.0e-9,
        "packing fraction {packing} against 13 balls over the tile {expected_packing}"
    );
    assert!((expected_packing - seker::SEKER_INTERIOR_PACKING).abs() < 5.0e-5);
    for p in &census.packings {
        assert_eq!(p.particles, seker::PARTICLES_PER_PEBBLE);
    }

    // ── Look at it before running it (crate HARD RULE) ─────────────────────
    let pitch_t = triso_pitch(&mats, &c)?;
    draw(&assembled, &layer.sites, &fuel, pitch_t, &out)?;
    if has("--geometry-only") {
        return Ok(());
    }

    // ── Run k-infinity ─────────────────────────────────────────────────────
    let t = Instant::now();
    let nuclides = assembled.load_nuclides(layer.model.data())?;
    let data_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, None))?;
    let run_s = t.elapsed().as_secs_f64();
    println!("\n{run}");
    println!(
        "  wall time: data {data_s:.1} s ({} nuclides, parallel), transport {run_s:.1} s\n  hardware: {}",
        nuclides.len(),
        HardwareInfo::detect().headline()
    );
    let k = run.keff();
    let z = |a: f64, sa: f64, b: f64, sb: f64| (a - b) / (sa * sa + sb * sb).sqrt();
    println!(
        "\nk_inf = {k}\n  against nee_soon's pilot for the one-tile cell {PILOT_K} +- {PILOT_SIGMA} (a different fuel split, 13 fuel + 10 moderator pieces): {:+.0} pcm ({:+.2} sigma)\n  against the lone fuel pebble {PEBBLE_K} (tutorial 5): {:+.0} pcm\n  against the finite core at 20 layers {CORE_K_N20} (must be below k_inf): {:+.0} pcm",
        (k.mean - PILOT_K) * 1e5,
        z(k.mean, k.std, PILOT_K, PILOT_SIGMA),
        (k.mean - PEBBLE_K) * 1e5,
        (k.mean - CORE_K_N20) * 1e5
    );
    Ok(())
}
