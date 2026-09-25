//! **Draw the assembled HTR-10 core as PNG images** — the geometry-drawing HARD
//! RULE of this crate's `CLAUDE.md`, applied to
//! [`assemble_explicit_triso`].
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_geometry_images
//! OUTRAM_HTR10_IMG_OUT=/some/dir cargo run --release -p nee_soon --example htr10_geometry_images
//! ```
//!
//! Every pixel is a [`Geometry::locate`] on the **assembled** geometry — the
//! geometry transport sees — through `outram_mc_libs::geometry::plot`, the
//! Rust port of OpenMC's slice plotter (pixel-for-pixel identical to
//! `openmc --plot` on the V&V cases in
//! `crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
//! Nothing here is drawn from the named constants; the constants are only used
//! to decide *where* to cut.
//!
//! Coloured by material with a fixed palette, a legend listing only the
//! materials actually present in each slice, and axes labelled in cm.
//!
//! # What it writes (default: `crates/nee_soon/verification_and_validation/htr10_geometry_images/`)
//!
//! | File | Slice |
//! |---|---|
//! | `htr10_rz_full.png` | x-z (R-Z) through the axis, whole model |
//! | `htr10_xy_bed_mid.png` | x-y at bed mid-height (z = 0) |
//! | `htr10_xy_conus.png` | x-y at the conus mid-height |
//! | `htr10_xy_cavity.png` | x-y at the core-cavity mid-height |
//! | `htr10_xz_pebbles.png` | x-z, 30 cm square round one fuel pebble: several pebbles |
//! | `htr10_xz_one_pebble.png` | x-z through one fuel pebble (and a row of its TRISO): its clipped shell |
//! | `htr10_xy_one_pebble.png` | x-y through the same pebble: its TRISO lattice |
//! | `htr10_xy_triso.png` | x-y, 0.8 cm square: a few TRISO particles and their coatings |
//! | `htr10_xz_conus_pebbles.png` | x-z through a TRISO row, 30 cm square at the conus mid-height: are the conus pebbles fuelled? |
//! | `htr10_xy_bed_wall.png` | x-y at z = 0, 24 cm square at the bed wall (x = 90 cm) |
//! | `htr10_xz_conus_wall.png` | x-z at y = 0, 30 cm square on the conus slope |
//!
//! The case size is the **reported** 14 rings x 25 layers (as
//! `htr10_geometry_export`), overridable with `OUTRAM_HTR10_RINGS` /
//! `OUTRAM_HTR10_LAYERS`.
//!
//! # NOT a validation artefact
//!
//! It runs no transport. What the images show, and what was and was not
//! checked in them, is recorded beside them in that folder's `README.md`.

use std::path::PathBuf;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat, HTR10_REFLECTOR_OUTER_CM};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::{Direction, Position};

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// Material palette, indexed by [`mat`]. Chosen so the TRISO layers read as a
/// warm-to-cool sequence from the kernel out and the graphites stay grey/brown.
fn palette() -> Vec<(Rgb, &'static str)> {
    let mut p = vec![(Rgb::new(0, 0, 0), ""); mat::HOMOG_DUMMY + 1];
    p[mat::KERNEL] = (Rgb::new(220, 20, 20), "UO2 KERNEL");
    p[mat::BUFFER] = (Rgb::new(255, 150, 0), "BUFFER PYC");
    p[mat::IPYC] = (Rgb::new(250, 225, 0), "IPYC");
    p[mat::SIC] = (Rgb::new(40, 160, 40), "SIC");
    p[mat::OPYC] = (Rgb::new(0, 190, 200), "OPYC");
    p[mat::GRAPHITE] = (Rgb::new(95, 95, 95), "MATRIX / SHELL GRAPHITE");
    p[mat::HELIUM] = (Rgb::new(205, 230, 255), "HELIUM");
    p[mat::REFLECTOR] = (Rgb::new(150, 115, 80), "REFLECTOR GRAPHITE");
    p[mat::BORONATED] = (Rgb::new(130, 40, 160), "BORONATED CARBON");
    p[mat::BORED_GRAPHITE] = (Rgb::new(190, 160, 120), "BORED REFLECTOR GRAPHITE");
    p[mat::HOMOG_DUMMY] = (Rgb::new(60, 70, 150), "HOMOG. DUMMY PEBBLES");
    p
}

/// The centre of a fuel pebble near `(x, y, z)` and of one TRISO particle in
/// it, read off the located path: the leaf level is the particle universe
/// (entered through the TRISO lattice, its frame offset is the particle
/// centre) and the level above it the pebble universe (entered through the bed
/// lattice, its offset is the pebble centre).
fn find_fuel_pebble(g: &Geometry, near: Position) -> Option<(Position, Position)> {
    // A 2-D scan: the TRISO lattice puts particle centres at HALF-pitch
    // offsets in x and y (`TRISO_OFFSET = [0.5, 0.5, 0.0]` in `core_model`), so
    // a single line through a pebble centre can miss every kernel.
    let u = Direction::new(0.0, 0.0, 1.0);
    for j in 0..40 {
        for i in 0..4000 {
            let p = Position::new(near.x + 0.01 * i as f64, near.y + 0.01 * j as f64, near.z);
            if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                let n = path.levels.len();
                if path.material == Some(mat::KERNEL) && n >= 3 {
                    let peb = path.levels[n - 2].offset;
                    let part = path.levels[n - 1].offset;
                    // Printed because `assemble_explicit_triso`'s docs once
                    // claimed four levels; the located path is the evidence.
                    let lats: Vec<Option<usize>> = path.levels.iter().map(|c| c.lattice).collect();
                    println!("kernel located at coordinate depth {n} (lattice per level {lats:?})");
                    return Some((peb, part));
                }
            }
        }
    }
    None
}

fn main() {
    let rings = env_usize("OUTRAM_HTR10_RINGS", 14);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 25);
    let out = std::env::var("OUTRAM_HTR10_IMG_OUT").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("verification_and_validation/htr10_geometry_images")
        },
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("create output directory");

    let t0 = std::time::Instant::now();
    let core = assemble_explicit_triso(rings, layers, 0);
    let g = &core.geometry;
    println!(
        "assembled {rings} x {layers}: {} tiles, {} cells, {} universes, bed r {:.3} cm, \
         half-height {:.4} cm, pitch {:.4} cm, tile height {:.4} cm ({:.1} s)",
        core.tiles,
        core.cells,
        core.universes,
        core.bed_radius,
        core.bed_half_height,
        core.lat_pitch,
        core.lat_height,
        t0.elapsed().as_secs_f64()
    );
    println!(
        "axial: model {:.3} .. {:.3} cm, conus floor {:.3}, bed {:.3} .. {:.3}, cavity top {:.3}",
        core.refl_bottom,
        core.refl_top,
        core.conus_floor,
        -core.bed_half_height,
        core.bed_half_height,
        core.cavity_top
    );

    let pal = palette();
    let tag = format!("HTR-10 {rings}X{layers}");
    let render = |name: &str, plot: SlicePlot, what: &str| {
        let t = std::time::Instant::now();
        let (_raw, img) = render_material_slice(g, &plot, &pal, &format!("{tag}: {what}"));
        let path = out.join(name);
        img.write_png(&path).expect("write png");
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!(
            "wrote {} ({}x{} px raster, {} kB, {:.1} s)",
            path.display(),
            plot.pixels[0],
            plot.pixels[1],
            bytes / 1024,
            t.elapsed().as_secs_f64()
        );
    };

    // Whole model, 0.4 cm per pixel.
    let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
    let z_lo = core.refl_bottom - 5.0;
    let z_hi = core.refl_top + 5.0;
    let px = |w: f64, cm_per_px: f64| (w / cm_per_px).round() as usize;
    let w = 2.0 * half_w;
    let h = z_hi - z_lo;
    render(
        "htr10_rz_full.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, 0.0, 0.5 * (z_lo + z_hi)),
            [w, h],
            [px(w, 0.4), px(h, 0.4)],
        ),
        "R-Z (X-Z) SLICE, Y = 0",
    );
    let xy = |z: f64| {
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(0.0, 0.0, z),
            [w, w],
            [px(w, 0.4), px(w, 0.4)],
        )
    };
    render("htr10_xy_bed_mid.png", xy(0.0), "X-Y AT BED MID-HEIGHT");
    let z_conus = 0.5 * (core.conus_floor - core.bed_half_height);
    render("htr10_xy_conus.png", xy(z_conus), "X-Y THROUGH THE CONUS");
    let z_cavity = 0.5 * (core.bed_half_height + core.cavity_top);
    render(
        "htr10_xy_cavity.png",
        xy(z_cavity),
        "X-Y THROUGH THE CAVITY",
    );

    // Where the bed meets the reflector: the bed wall at bed mid-height, and
    // the conus slope half-way down it (the cone runs from (bed_radius,
    // -bed_half_height) to (discharge radius, conus_floor) in r-z).
    render(
        "htr10_xy_bed_wall.png",
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(84.0, 0.0, 0.0),
            [24.0, 24.0],
            [800, 800],
        ),
        "X-Y AT THE BED WALL, Z = 0",
    );
    let z_mid_cone = 0.5 * (core.conus_floor - core.bed_half_height);
    let r_mid_cone =
        0.5 * (core.bed_radius + nee_soon::htr10_rmc::core_model::HTR10_DISCHARGE_TUBE_RADIUS_CM);
    render(
        "htr10_xz_conus_wall.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(r_mid_cone, 0.0, z_mid_cone),
            [30.0, 30.0],
            [900, 900],
        ),
        "X-Z ON THE CONUS SLOPE, Y = 0",
    );

    // Zooms round one fuel pebble near the axis at bed mid-height.
    let Some((peb, part)) = find_fuel_pebble(g, Position::new(0.0, 0.0, 0.0)) else {
        eprintln!("no fuel pebble found near the axis; zoomed images skipped");
        return;
    };
    println!(
        "fuel pebble centre ({:.4}, {:.4}, {:.4}); a TRISO particle at ({:.4}, {:.4}, {:.4})",
        peb.x, peb.y, peb.z, part.x, part.y, part.z
    );
    // The x-z zooms are cut at the PARTICLE's y (0.1 cm off the pebble
    // centre) and the x-y zoom at the particle's z, so each plane runs through
    // a row of TRISO centres; the pebble's exact centre plane in y falls
    // between TRISO rows and would show the fuel zone as bare matrix.
    let through_row = Position::new(peb.x, part.y, peb.z);
    render(
        "htr10_xz_pebbles.png",
        SlicePlot::new(PlotBasis::Xz, through_row, [30.0, 30.0], [1000, 1000]),
        "X-Z, 30 CM ROUND ONE FUEL PEBBLE",
    );
    render(
        "htr10_xz_one_pebble.png",
        SlicePlot::new(PlotBasis::Xz, through_row, [7.0, 7.0], [1000, 1000]),
        "X-Z THROUGH ONE FUEL PEBBLE",
    );
    render(
        "htr10_xy_one_pebble.png",
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(peb.x, peb.y, part.z),
            [7.0, 7.0],
            [1000, 1000],
        ),
        "X-Y THROUGH ONE FUEL PEBBLE",
    );
    // The conus, cut through the same TRISO row (y = particle y): Terry
    // (2005) says it holds only DUMMY pebbles, so no TRISO should appear below
    // the bed floor at z = -bed_half_height.
    render(
        "htr10_xz_conus_pebbles.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, part.y, z_conus),
            [30.0, 30.0],
            [1000, 1000],
        ),
        "X-Z THROUGH A TRISO ROW, CONUS MID-HEIGHT",
    );
    render(
        "htr10_xy_triso.png",
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(part.x, part.y, part.z),
            [0.8, 0.8],
            [800, 800],
        ),
        "X-Y, 0.8 CM ROUND ONE TRISO PARTICLE",
    );
}
