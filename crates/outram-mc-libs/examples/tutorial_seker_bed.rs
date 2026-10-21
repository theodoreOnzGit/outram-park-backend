//! # Tutorial 8: the full Şeker bed of HTR-10, stacked, inside the core
//!
//! **A hexagonal lattice stacked in a cylinder-cone-tube, with whole-ball
//! rejection** (beginner API `outram_mc_libs::model`, epic gh #925; the
//! tutorial list and its decisions on #943). Tutorial 7 built one Şeker layer
//! and repeated it without end. HTR-10's bed is that tile stacked in the
//! plane and in height inside the core cavity, which is not a box, so the
//! balls near its wall, cone and tube do not fit. New here:
//!
//! - **`Hopper`**: a cylinder narrowing through a cone into a tube, the core
//!   cavity (a new solid; it lowers to a cylinder, a cone and three planes);
//! - **`Lattice::hex_bed(tile, moderator, fuel_fraction, bed_top)`**: tutorial
//!   7's tile stacked to fill the hopper. **A ball that would cross the side
//!   wall, the cone, the tube or the top of the bed is rejected, never cut**:
//!   a cut pebble is wrong physics (maintainer, 2026-10-01), so its place is
//!   helium and the ball count falls short of Şeker's;
//! - **`AssembledModel::bed_census()`**: balls kept, balls rejected and at
//!   which wall, fuel and moderator, the bed's height by Şeker's definition;
//! - **tracking per region**: the lattice (pebble boundaries) is surface
//!   tracked; every fuel zone is a delta-tracked `DhRegion` (tutorial 5),
//!   lowered once and shared by all 9508 fuel balls.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_seker_bed                       # Smoke, N = 12 (123.576 cm)
//! cargo run --release -p outram-mc-libs --example tutorial_seker_bed -- --fast             # MCSettings::Fast
//! cargo run --release -p outram-mc-libs --example tutorial_seker_bed -- --layers 10,12,16  # a few heights
//! cargo run --release -p outram-mc-libs --example tutorial_seker_bed -- --geometry-only    # draw and count only
//! ```
//!
//! (Tutorials refactor existing examples where one fits (#943). None does,
//! checked: the bed is `nee_soon::htr10_rmc::bed::SekerBed` and
//! `core_model::assemble_explicit_triso`, whose V&V role stays, and
//! `outram-mc-libs` cannot call `nee_soon`. This file ports their
//! expressions, with citations, into `Hopper`, `Lattice::hex_bed` and
//! `model/bed.rs`; the statements are in `examples/common/seker_bed_model.rs`,
//! shared with `tests/model_seker_bed.rs`, which pins them to a hand-built
//! CSG written from `nee_soon`'s formulas.)
//!
//! ## The case
//!
//! | what | value | source |
//! |---|---|---|
//! | tile | tutorial 7's: hexagonal prism, pitch 16.392 cm, height 9.798 cm, 13 balls | Şeker & Çolak 2003 pp.266-267 |
//! | cavity | cylinder r 90 cm, cone 36.946 cm to a 25 cm tube, tube 221.236 cm; floor (cone top) at z = 0 | TECDOC-1382; Terry et al. 2005 Fig. 2; `core_model` constants |
//! | loading | `N` layers: `9.798 N + 6` cm, lowest ball's bottom to highest ball's top | Şeker p.267 |
//! | rejection | a ball is kept only if wholly inside; none is cut | Şeker p.267; `SekerBed::reject` |
//! | fuel : moderator | 0.57 : 0.43 of the balls above the floor, the cone and tube all moderator | Şeker p.267; Terry 2005 s.2; `SekerBed::assign` |
//! | reflector | solid graphite (TECDOC Table 4-3 zone 22) to r 167.793 cm, boronated brick (zone 17) to 190 cm, 130 cm of graphite above the cavity, model 610 cm tall | Terry 2005 Fig. 2; `nee_soon::htr10_rmc::reflector` |
//! | temperature | 300.15 K | Li, Yu & Wei 2014; the crate's HTR-10 records |
//!
//! ## What this checks (V&V methodology)
//!
//! Each check can fail; nothing is tuned.
//!
//! 1. **Counted from the geometry:** the census walks the assembled universe
//!    graph and counts fuel balls, moderator balls and TRISO particles; the
//!    bed census (from the lattice the geometry was lowered from) must give
//!    the same balls.
//! 2. **Every kept ball is in the geometry and every rejected one is not:**
//!    each kept ball's centre locates a ball of its kind, each rejected
//!    ball's centre locates helium.
//! 3. **Rejection against Şeker's count:** balls above the floor against
//!    `1346 N + 733` (Table 3), and the height `9.798 N + 6`.
//! 4. **Pictures** of the assembled geometry (crate HARD RULE), written to
//!    `target/tutorials/seker_bed/` (gitignored).
//! 5. **k against height** against `nee_soon`'s record and the published
//!    curves (see Results).
//!
//! ## Results
//!
//! See the page `docs/tutorial/src/tutorial8-seker-bed.md`; the numbers of
//! the last run are recorded there and on gh #943.

use petir::mathf::RealMath;
use std::path::{Path, PathBuf};
use std::time::Instant;

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::annotate::draw_text;
use outram_mc_libs::geometry::plot::{
    annotate_image, annotate_slice, ColourScheme, ImageData, LegendEntry, PlotBasis, PlotColourBy,
    Rgb, SliceHit, SlicePlot, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::{KeptBall, NuclideKey, Wall};
use outram_mc_libs::perf_report::HardwareInfo;
use uom::si::f64::Length;
use uom::si::length::centimeter;

#[path = "common/seker_bed_model.rs"]
#[allow(dead_code)]
mod seker_bed_model;
use seker_bed_model as bed;
use bed::layer as seker;

// ── References ──────────────────────────────────────────────────────────────

/// Li, Yu & Wei (2014) RMC k_eff against loading height \[cm\] (Tables 3 and
/// 4), `nee_soon::htr10_rmc::RMC_KEFF_VS_HEIGHT`.
const RMC: &[(f64, f64)] = &[
    (94.182, 0.894_693),
    (103.980, 0.937_122),
    (113.778, 0.972_163),
    (123.576, 1.004_288),
    (133.374, 1.030_738),
    (143.172, 1.056_866),
    (152.970, 1.078_448),
    (162.768, 1.097_370),
    (172.566, 1.114_775),
    (182.364, 1.133_878),
    (192.162, 1.147_570),
    (201.960, 1.162_230),
];

/// Şeker & Çolak (2003) Table 3 (vacuum column), MCNP with ENDF/B-VI,
/// `nee_soon::htr10_rmc::MCNP_TABLE3_KEFF_VS_HEIGHT`.
const MCNP_T3: &[(f64, f64)] = &[
    (94.182, 0.888_56),
    (103.980, 0.930_52),
    (113.778, 0.969_98),
    (123.576, 1.003_3),
    (133.374, 1.033_66),
    (143.172, 1.058_4),
    (152.970, 1.085_12),
    (162.768, 1.102_9),
    (172.566, 1.122_37),
    (182.364, 1.139_04),
    (192.162, 1.158_46),
    (201.960, 1.171_93),
];

/// `nee_soon`'s record `htr10_seker_2026_10_07_10k` (10 000 x [5 + 135], one
/// seed per point, ENDF/B-VIII.0, 300.15 K): `(N, k, sigma)`.
const RECORD_VIII: &[(usize, f64, f64)] = &[
    (10, 0.925141, 0.001082),
    (11, 0.963975, 0.000923),
    (12, 0.995125, 0.001055),
    (13, 1.024086, 0.001088),
    (14, 1.048724, 0.001191),
    (15, 1.071474, 0.001065),
    (16, 1.095127, 0.001103),
    (17, 1.111591, 0.001044),
    (18, 1.129270, 0.001005),
    (19, 1.144219, 0.001110),
    (20, 1.160230, 0.001066),
];

/// Linear interpolation in a tabulated curve; `None` outside it
/// (`nee_soon::htr10_rmc::keff_curve_at_height`).
fn curve_at(curve: &[(f64, f64)], h: f64) -> Option<f64> {
    let (first, last) = (curve.first()?.0, curve.last()?.0);
    if h < first - 1e-6 || h > last + 1e-6 {
        return None;
    }
    curve.windows(2).find_map(|w| {
        let ((h0, k0), (h1, k1)) = (w[0], w[1]);
        (h0 - 1e-6..=h1 + 1e-6)
            .contains(&h)
            .then(|| k0 + (h - h0) / (h1 - h0) * (k1 - k0))
    })
}

/// The height \[cm\] at which Şeker's model holds `balls` balls,
/// `9.798 (balls - 733) / 1346 + 6.0` (`nee_soon::htr10_rmc::
/// seker_height_for_balls`, gh:#472): the reference is read here, at equal
/// ball count, because the whole-ball rule keeps fewer balls at one height.
fn seker_height_for_balls(balls: usize) -> f64 {
    9.798 * (balls as f64 - 733.0) / 1346.0 + 6.0
}

// ── Drawing ─────────────────────────────────────────────────────────────────

/// The colours of the first twelve materials, the beginner plots' palette.
const PALETTE: [Rgb; 12] = [
    Rgb::new(214, 96, 77),
    Rgb::new(67, 147, 195),
    Rgb::new(120, 180, 90),
    Rgb::new(240, 190, 70),
    Rgb::new(150, 110, 190),
    Rgb::new(110, 110, 110),
    Rgb::new(230, 130, 190),
    Rgb::new(80, 190, 190),
    Rgb::new(160, 82, 45),
    Rgb::new(20, 60, 110),
    Rgb::new(190, 190, 40),
    Rgb::new(30, 100, 60),
];
const RING: Rgb = Rgb::new(255, 0, 255);
const WALL_LINE: Rgb = Rgb::new(0, 0, 0);

/// A ring to draw on a slice: centre and radius in the slice's own
/// coordinates \[cm\].
struct Ring {
    h: f64,
    v: f64,
    r: f64,
}

/// What a slice picture is drawn from: the plane, the window, the title and
/// what is drawn over the geometry.
struct View {
    basis: PlotBasis,
    centre: Position,
    width_cm: f64,
    pixels: usize,
    title: String,
    rings: Vec<Ring>,
    lines: Vec<([f64; 2], [f64; 2])>,
    extra_legend: Vec<(Rgb, String)>,
}

impl View {
    fn new(
        basis: PlotBasis,
        centre: Position,
        width_cm: f64,
        pixels: usize,
        title: String,
    ) -> Self {
        Self {
            basis,
            centre,
            width_cm,
            pixels,
            title,
            rings: Vec::new(),
            lines: Vec::new(),
            extra_legend: Vec::new(),
        }
    }
}

fn dot(img: &mut ImageData, x: f64, y: f64, c: Rgb) {
    let (xi, yi) = (x.round() as i64, y.round() as i64);
    for dx in 0..2 {
        for dy in 0..2 {
            let (px, py) = (xi + dx, yi + dy);
            if px >= 0 && py >= 0 && (px as usize) < img.width && (py as usize) < img.height {
                img.set(px as usize, py as usize, c);
            }
        }
    }
}

/// **Draw the assembled geometry**: the plotter's material slice with a
/// legend and cm axes, and over it the rings and lines of `view`.
fn render(a: &AssembledModel, view: &View) -> ImageData {
    let plot = SlicePlot::new(
        view.basis,
        view.centre,
        [view.width_cm, view.width_cm],
        [view.pixels, view.pixels],
    );
    let ids = plot.id_map(&a.geometry);
    let n = a.resolved.len();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut scheme = ColourScheme::new(PlotColourBy::Material, n, &mut seed)
        .with_background(Rgb::new(235, 235, 235));
    for i in 0..n {
        scheme = scheme.with_colour(i, PALETTE[i % PALETTE.len()]);
    }
    let mut raw = plot.colour_id_map(&ids, &scheme);
    let mut present = vec![false; n];
    let (mut void, mut outside) = (false, false);
    for h in &ids.hits {
        match *h {
            SliceHit::Found {
                material: Some(m), ..
            } => present[m] = true,
            SliceHit::Found { material: None, .. } => void = true,
            SliceHit::NotFound => outside = true,
            SliceHit::Overlap => {}
        }
    }
    let mut legend: Vec<LegendEntry> = present
        .iter()
        .enumerate()
        .filter(|(_, p)| **p)
        .map(|(i, _)| LegendEntry::new(PALETTE[i % PALETTE.len()], a.resolved[i].name.clone()))
        .collect();
    if void {
        legend.push(LegendEntry::new(Rgb::new(255, 255, 255), "VOID"));
    }
    if outside {
        legend.push(LegendEntry::new(Rgb::new(235, 235, 235), "OUTSIDE MODEL"));
    }
    for (c, label) in &view.extra_legend {
        legend.push(LegendEntry::new(*c, label.clone()));
    }
    let (ah, av) = view.basis.axes();
    let o = [view.centre.x, view.centre.y, view.centre.z];
    let (lo_h, hi_v) = (o[ah] - 0.5 * view.width_cm, o[av] + 0.5 * view.width_cm);
    let px = view.pixels as f64;
    let to_px = |h: f64, v: f64| {
        (
            (h - lo_h) / view.width_cm * px - 0.5,
            (hi_v - v) / view.width_cm * px - 0.5,
        )
    };
    for ring in &view.rings {
        let steps = ((ring.r / view.width_cm * px * 6.5) as usize).max(24);
        for k in 0..steps {
            let t = 2.0 * std::f64::consts::PI * k as f64 / steps as f64;
            let (x, y) = to_px(ring.h + ring.r * t.r_cos(), ring.v + ring.r * t.r_sin());
            dot(&mut raw, x, y, RING);
        }
    }
    for (p, q) in &view.lines {
        let len = ((q[0] - p[0]).r_hypot(q[1] - p[1]) / view.width_cm * px * 2.0) as usize + 2;
        for k in 0..=len {
            let f = k as f64 / len as f64;
            let (x, y) = to_px(p[0] + f * (q[0] - p[0]), p[1] + f * (q[1] - p[1]));
            dot(&mut raw, x, y, WALL_LINE);
        }
    }
    annotate_slice(&raw, &plot, &view.title, &legend)
}

/// The material at a point, found the way the transport finds it.
fn material_at<'a>(a: &'a AssembledModel, p: [f64; 3]) -> Option<&'a str> {
    a.geometry
        .locate(
            Position::new(p[0], p[1], p[2]),
            Direction::new(0.0, 0.0, 1.0),
            SurfaceToken::NONE,
        )
        .and_then(|path| path.material)
        .map(|m| a.resolved[m].name.as_str())
}

/// A point inside a kernel of the ball at `centre`, found by scanning the
/// assembled geometry on the plane `z`, and the centroid of that kernel.
fn find_kernel(a: &AssembledModel, centre: [f64; 3], z: f64) -> Option<Position> {
    let at = |x: f64, y: f64| material_at(a, [x, y, z]) == Some("UO2 kernel");
    let step = 0.004;
    let mut first = None;
    'scan: for i in 0..125 {
        for j in 0..125 {
            let (x, y) = (centre[0] + i as f64 * step, centre[1] + j as f64 * step);
            if at(x, y) {
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
            if at(x, y) {
                sx += x;
                sy += y;
                n += 1.0;
            }
        }
    }
    Some(Position::new(sx / n, sy / n, z))
}

/// The TRISO lattice pitch the fuel zone finds (so a slice can be put on a
/// layer of particle centres).
fn triso_pitch(m: &seker::Materials, c: &seker::Components) -> Result<f64, ModelError> {
    let particle = seker::triso_particle(m, c);
    let cube = Cube::for_whole_particles_in_sphere(
        &particle,
        Sphere::radius(Length::new::<centimeter>(seker::FUEL_ZONE_R_CM)),
        seker::PARTICLES_PER_PEBBLE,
        seker::OFFSET,
    )?;
    Ok(cube.sides_cm()[0])
}

/// The meridional profile of the hopper as line segments in (x, z), both
/// sides of the axis.
fn profile_lines(h: &Hopper, top: f64) -> Vec<([f64; 2], [f64; 2])> {
    let (r, ro) = (h.radius_cm(), h.outlet_radius_cm());
    let (zf, zc, zb) = (h.floor_z_cm(), h.cone_bottom_z_cm(), h.bottom_z_cm());
    let mut v = Vec::new();
    for s in [1.0, -1.0] {
        v.push(([s * r, top], [s * r, zf]));
        v.push(([s * r, zf], [s * ro, zc]));
        v.push(([s * ro, zc], [s * ro, zb]));
    }
    v.push(([-r, top], [r, top]));
    v.push(([-ro, zb], [ro, zb]));
    v
}

/// The wall each rejection colour stands for.
fn wall_colour(w: Wall) -> Rgb {
    match w {
        Wall::SideWall => Rgb::new(200, 30, 30),
        Wall::Cone => Rgb::new(230, 140, 20),
        Wall::TubeWall => Rgb::new(30, 120, 30),
        Wall::TubeBottom => Rgb::new(30, 60, 200),
        Wall::Top => Rgb::new(130, 30, 160),
    }
}

/// **The rejected balls in a meridional picture** (rho against z): every
/// ball kept as a grey dot, every rejected ball as a dot in the colour of the
/// wall it would have crossed, the hopper's profile in black. From the bed
/// census (not a geometry slice: the other pictures are those).
fn meridional(h: &Hopper, c: &BedCensus, bed_top: f64) -> ImageData {
    let scale = 2.7; // pixels per cm on both axes
    let (rho_max, z_lo, z_hi) = (111.0, h.bottom_z_cm() - 8.0, bed_top + 14.0);
    let (w, ht) = ((rho_max * scale) as usize, ((z_hi - z_lo) * scale) as usize);
    let to_px = |rho: f64, z: f64| (rho * scale, (z_hi - z) * scale);
    let mut img = ImageData::filled(w, ht, Rgb::new(255, 255, 255));
    let grey = Rgb::new(225, 225, 225);
    let mut z = (z_lo / 50.0).ceil() * 50.0;
    while z <= z_hi {
        let (_, y) = to_px(0.0, z);
        for x in 0..w {
            img.set(x, (y as usize).min(ht - 1), grey);
        }
        draw_text(
            &mut img,
            4,
            y as i64 - 9,
            &format!("z {z:.0}"),
            Rgb::new(90, 90, 90),
        );
        z += 50.0;
    }
    for rho in [0.0, 25.0, 50.0, 75.0, 90.0] {
        let (x, _) = to_px(rho, 0.0);
        for y in 0..ht {
            img.set((x as usize).min(w - 1), y, grey);
        }
        draw_text(
            &mut img,
            x as i64 + 3,
            ht as i64 - 14,
            &format!("{rho:.0}"),
            Rgb::new(90, 90, 90),
        );
    }
    let disc = |img: &mut ImageData, rho: f64, zz: f64, rad_px: f64, col: Rgb| {
        let (x, y) = to_px(rho, zz);
        let r = rad_px.ceil() as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                if ((dx * dx + dy * dy) as f64) <= rad_px * rad_px {
                    let (px, py) = (x as i64 + dx, y as i64 + dy);
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < ht {
                        img.set(px as usize, py as usize, col);
                    }
                }
            }
        }
    };
    for k in &c.kept {
        let [x, y, zz] = k.centre_cm;
        let col = if k.fuel {
            Rgb::new(150, 170, 200)
        } else {
            Rgb::new(190, 190, 190)
        };
        disc(&mut img, x.r_hypot(y), zz, 1.6, col);
    }
    for r in &c.rejected {
        let [x, y, zz] = r.centre_cm;
        disc(&mut img, x.r_hypot(y), zz, 3.2, wall_colour(r.wall));
    }
    let (r, ro) = (h.radius_cm(), h.outlet_radius_cm());
    let prof = [
        [0.0, bed_top],
        [r, bed_top],
        [r, h.floor_z_cm()],
        [ro, h.cone_bottom_z_cm()],
        [ro, h.bottom_z_cm()],
        [0.0, h.bottom_z_cm()],
    ];
    for w2 in prof.windows(2) {
        let (p, q) = (w2[0], w2[1]);
        let n = 4000;
        for k in 0..=n {
            let f = k as f64 / n as f64;
            let (x, y) = to_px(p[0] + f * (q[0] - p[0]), p[1] + f * (q[1] - p[1]));
            dot(&mut img, x, y, WALL_LINE);
        }
    }
    let legend = vec![
        LegendEntry::new(
            Rgb::new(150, 170, 200),
            format!("kept fuel ball ({})", c.fuel_balls),
        ),
        LegendEntry::new(
            Rgb::new(190, 190, 190),
            format!(
                "kept moderator ball ({})",
                c.moderator_balls_in_bed + c.cone_and_tube_balls
            ),
        ),
        LegendEntry::new(
            wall_colour(Wall::SideWall),
            format!("rejected at the side wall ({})", c.rejected_at_side_wall),
        ),
        LegendEntry::new(
            wall_colour(Wall::Cone),
            format!("rejected at the cone ({})", c.rejected_at_cone),
        ),
        LegendEntry::new(
            wall_colour(Wall::TubeWall),
            format!("rejected at the tube wall ({})", c.rejected_at_tube_wall),
        ),
        LegendEntry::new(
            wall_colour(Wall::TubeBottom),
            format!(
                "rejected at the tube bottom ({})",
                c.rejected_at_tube_bottom
            ),
        ),
        LegendEntry::new(
            wall_colour(Wall::Top),
            format!("rejected at the bed top ({})", c.rejected_at_top),
        ),
    ];
    annotate_image(
        &img,
        &format!(
            "Every ball by radius and height, cm: {} kept, {} rejected whole (never cut)",
            c.loaded_balls, c.rejected_balls
        ),
        &legend,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let preset = if has("--auto") {
        MCSettings::Auto
    } else if has("--fast") {
        MCSettings::Fast
    } else {
        // Smoke by default (maintainer, 2026-10-18: smoke for tutorials).
        MCSettings::Smoke
    };
    let layers: Vec<usize> = match value("--layers") {
        Some(s) => s
            .split(',')
            .map(|n| n.trim().parse::<usize>())
            .collect::<Result<_, _>>()?,
        None => vec![12],
    };
    if let Err(e) = NuclearData::workspace_reference() {
        println!("No ENDF tapes: {e}. Clone with the reference data.");
        return Ok(());
    }
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/seker_bed");
    std::fs::create_dir_all(&out)?;
    let mats = seker::materials(seker::T_K);
    let refl = bed::reflector_materials();

    // ── Build, assemble, count, draw (the first height); run k at each ────
    let mut shared_nuclides: Option<(Vec<NuclideKey>, Vec<Nuclide>)> = None;
    let mut rows: Vec<Row> = Vec::new();
    for (idx, &n) in layers.iter().enumerate() {
        println!(
            "\n=== N = {n} Şeker layers: loading height {:.3} cm ===",
            bed::loading_height_cm(n)
        );
        let core = bed::build(
            NuclearData::workspace_reference()?,
            &mats,
            &refl,
            &seker::FuelBall::Triso,
            n,
            seker::T_K,
        );
        let c = core.components;
        let t = Instant::now();
        let a = core.model.assemble()?;
        println!(
            "assembled in {:.1} s: {} surfaces, {} cells, {} universes, {} lattice(s), {} delta-tracked region(s)",
            t.elapsed().as_secs_f64(),
            a.geometry.surfaces.len(),
            a.geometry.cells.len(),
            a.geometry.universes.len(),
            a.geometry.lattices.len(),
            a.delta_regions().len()
        );
        let census = a.bed_census()?;
        println!("{census}");
        let bed_balls = census.bed_balls;
        let reference = bed::seker_reference_balls(n);
        println!(
            "Seker's Table 3 at N = {n}: {reference} balls (1346 N + 733); this bed keeps {bed_balls}: {:+} ({:+.2} %); height {:.3} cm against 9.798 N + 6 = {:.3} cm",
            census.bed_balls_against(reference),
            100.0 * census.bed_balls_against(reference) as f64 / reference as f64,
            census.bed_height_cm,
            9.798 * n as f64 + 6.0
        );
        let basal: Vec<usize> = census.planes.iter().step_by(2).map(|p| p.1).collect();
        let central: Vec<usize> = census
            .planes
            .iter()
            .skip(1)
            .step_by(2)
            .map(|p| p.1)
            .collect();
        println!(
            "  per plane: {} basal planes of {}..{} balls (Seker: 733), {} central planes of {}..{} (Seker: 613)",
            basal.len(),
            basal.iter().min().unwrap_or(&0),
            basal.iter().max().unwrap_or(&0),
            central.len(),
            central.iter().min().unwrap_or(&0),
            central.iter().max().unwrap_or(&0)
        );

        if idx == 0 {
            // 1. Counted from the geometry.
            let t = Instant::now();
            let geometry_census = a.census()?;
            println!(
                "\ncounted from the assembled geometry in {:.1} s:",
                t.elapsed().as_secs_f64()
            );
            let fuel_balls = geometry_census.copies(c.layer.shell);
            let moderator_balls = geometry_census.copies(c.layer.moderator);
            let particles: usize = geometry_census
                .packings
                .iter()
                .filter(|p| p.delta_tracked)
                .map(|p| p.particles)
                .sum();
            println!(
                "  {fuel_balls} fuel balls + {moderator_balls} moderator balls = {} (bed census: {} kept), {particles} TRISO particles (= {fuel_balls} x {})",
                fuel_balls + moderator_balls,
                census.loaded_balls,
                seker::PARTICLES_PER_PEBBLE
            );
            assert_eq!(fuel_balls, census.fuel_balls, "fuel balls in the geometry");
            assert_eq!(
                moderator_balls,
                census.moderator_balls_in_bed + census.cone_and_tube_balls,
                "moderator balls in the geometry"
            );
            assert_eq!(particles, fuel_balls * seker::PARTICLES_PER_PEBBLE);

            // 2. Every kept ball is in the geometry, every rejected one is not.
            let (mut fuel_found, mut mod_found, mut bad) = (0, 0, 0);
            for k in &census.kept {
                match (material_at(&a, k.centre_cm), k.fuel) {
                    (
                        Some(
                            "matrix graphite" | "UO2 kernel" | "buffer carbon" | "pyrolytic carbon"
                            | "silicon carbide",
                        ),
                        true,
                    ) => fuel_found += 1,
                    (Some("shell graphite"), false) => mod_found += 1,
                    _ => bad += 1,
                }
            }
            let rejected_empty = census
                .rejected
                .iter()
                .filter(|r| material_at(&a, r.centre_cm) == Some("helium"))
                .count();
            println!(
                "  located by the transport's own search: {fuel_found} fuel and {mod_found} moderator balls at their kept centres ({bad} wrong); {rejected_empty} of {} rejected centres are helium",
                census.rejected.len()
            );
            assert_eq!(bad, 0);
            assert_eq!(rejected_empty, census.rejected.len());

            // 3. Pictures.
            let pitch_t = triso_pitch(&mats, &c.layer)?;
            draw_all(&a, &census, &core, pitch_t, &out)?;
        }
        if has("--geometry-only") {
            continue;
        }

        // ── Run k ───────────────────────────────────────────────────────────
        // The nuclides depend on the materials and the temperature, not on
        // the height: process them once and reuse them at every height.
        if shared_nuclides.as_ref().map(|(keys, _)| keys) != Some(&a.nuclides) {
            let t = Instant::now();
            let loaded = a.load_nuclides(core.model.data())?;
            println!(
                "  nuclear data: {} nuclides processed in {:.1} s",
                loaded.len(),
                t.elapsed().as_secs_f64()
            );
            shared_nuclides = Some((a.nuclides.clone(), loaded));
        }
        let nuclides = &shared_nuclides.as_ref().expect("just set").1;
        let t = Instant::now();
        let run = a.keff_with(nuclides, &KeffSettings::new(preset, None))?;
        let run_s = t.elapsed().as_secs_f64();
        println!("\n{run}");
        println!(
            "  transport {run_s:.1} s\n  hardware: {} (threads pinned by the caller)",
            HardwareInfo::detect().headline()
        );
        let k = run.keff();
        rows.push(Row {
            layers: n,
            bed_balls,
            built_height: census.bed_height_cm,
            k: k.mean,
            sigma: k.std,
            transport_s: run_s,
        });
    }

    if !rows.is_empty() {
        println!(
            "\n{:>3} {:>8} {:>9} {:>9} | {:>17} | {:>10} {:>10} {:>10} | {:>17} {:>8}",
            "N",
            "balls",
            "built cm",
            "ref cm",
            "k (this run)",
            "record",
            "RMC",
            "MCNP T3",
            "k - record [pcm]",
            "sigmas"
        );
        for r in &rows {
            let href = seker_height_for_balls(r.bed_balls);
            let rec = RECORD_VIII.iter().find(|x| x.0 == r.layers);
            let rmc = curve_at(RMC, href);
            let t3 = curve_at(MCNP_T3, href);
            let f = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.5}"));
            let (dk, ds) = rec.map_or((f64::NAN, f64::NAN), |x| {
                (
                    (r.k - x.1) * 1e5,
                    (r.k - x.1) / (r.sigma * r.sigma + x.2 * x.2).sqrt(),
                )
            });
            println!(
                "{:>3} {:>8} {:>9.3} {:>9.3} | {:>8.5} +- {:.5} | {:>10} {:>10} {:>10} | {:>+17.0} {:>+8.2}   ({:.0} s)",
                r.layers, r.bed_balls, r.built_height, href, r.k, r.sigma,
                f(rec.map(|x| x.1)), f(rmc), f(t3), dk, ds, r.transport_s
            );
        }
        println!(
            "\nLOW STATISTICS ({preset:?}), one seed per height; the record is 10 000 x [5 + 135]. The shell around the bed is simplified (solid graphite and brick: no coolant channels, control-rod borings or zone map), so k above the record is expected."
        );
    }
    Ok(())
}

struct Row {
    layers: usize,
    bed_balls: usize,
    built_height: f64,
    k: f64,
    sigma: f64,
    transport_s: f64,
}

/// The pictures: every level of the assembled model, from the whole core to
/// one TRISO particle, and the rejected balls.
fn draw_all(
    a: &AssembledModel,
    c: &BedCensus,
    core: &bed::SekerBedCore,
    pitch_t: f64,
    out: &Path,
) -> std::io::Result<()> {
    let r = 0.5 * seker::BALL_DIAMETER_CM;
    let h = seker::height_cm();
    let floor = c.floor_z_cm;
    let n = core.layers;
    let top = core.bed_top_cm;
    // The TRISO lattice has no particle centre on a ball's centre plane:
    // draw 0.29 pitch below it (tutorial 7).
    let dz_t = (seker::OFFSET[2] - 1.0) * pitch_t;
    let hopper = Hopper::new(
        Length::new::<centimeter>(bed::CORE_RADIUS_CM),
        Length::new::<centimeter>(bed::CAVITY_HEIGHT_CM),
    )
    .conical_floor(
        Length::new::<centimeter>(bed::CONE_HEIGHT_CM),
        Length::new::<centimeter>(bed::TUBE_RADIUS_CM),
    )
    .outlet_tube(Length::new::<centimeter>(bed::TUBE_DEPTH_CM));
    let mut written: Vec<PathBuf> = Vec::new();
    let mut save = |img: ImageData, file: &str| -> std::io::Result<()> {
        let p = out.join(file);
        img.write_png(&p)?;
        println!("  drew {}", p.display());
        written.push(p);
        Ok(())
    };
    let model_top = bed::CAVITY_HEIGHT_CM + bed::TOP_REFLECTOR_CM;
    let z_mid = model_top - 0.5 * bed::MODEL_HEIGHT_CM;
    println!("\ndrawing the assembled model:");

    // R-Z slices (x-z at y = 0).
    let mut v = View::new(
        PlotBasis::Xz,
        Position::new(0.0, 0.0, z_mid),
        1.08 * bed::MODEL_HEIGHT_CM,
        1100,
        format!(
            "Whole core, x-z at y = 0: bed loaded to {:.3} cm ({} balls kept), cavity, tube, graphite reflector, boronated brick to r = 190 cm",
            c.bed_height_cm, c.bed_balls
        ),
    );
    v.lines = profile_lines(&hopper, top);
    save(render(a, &v), "core_rz.png")?;

    let mut v = View::new(
        PlotBasis::Xz,
        Position::new(0.0, 0.0, 70.0),
        210.0,
        1100,
        format!(
            "The bed, x-z at y = 0: floor z = {floor:.0}, top of the highest ball z = {:.3} cm, helium cavity above",
            floor + c.bed_height_cm
        ),
    );
    v.lines = profile_lines(&hopper, top);
    save(render(a, &v), "bed_rz.png")?;

    let mut v = View::new(
        PlotBasis::Xz,
        Position::new(0.0, 0.0, -110.0),
        300.0,
        1100,
        format!(
            "Cone and tube, x-z at y = 0: graphite balls on the same lattice, {} kept below the floor",
            c.cone_and_tube_balls
        ),
    );
    v.lines = profile_lines(&hopper, top);
    save(render(a, &v), "cone_tube_rz.png")?;

    // Radial slices at the heights that matter, each on a plane of ball
    // centres (a basal plane at floor + r + j h) and below it by dz_t.
    let plane = |j: i32| floor + r + f64::from(j) * h;
    let kept_in = |z: f64| {
        c.kept
            .iter()
            .filter(|k| (k.centre_cm[2] - z).abs() < 1e-6)
            .count()
    };
    let fuel_in = |z: f64| {
        c.kept
            .iter()
            .filter(|k| (k.centre_cm[2] - z).abs() < 1e-6 && k.fuel)
            .count()
    };
    let rejected_in = |z: f64| {
        c.rejected
            .iter()
            .filter(|k| (k.centre_cm[2] - z).abs() < 1e-6)
            .count()
    };
    let radial = |z: f64, width: f64, name: &str, what: &str| -> (View, String) {
        let title = format!(
            "{what}: x-y at z = {:.3} cm, a plane of ball centres: {} balls kept ({} fuel, {} moderator), {} rejected here",
            z,
            kept_in(z),
            fuel_in(z),
            kept_in(z) - fuel_in(z),
            rejected_in(z)
        );
        (
            View::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, z + dz_t),
                width,
                1100,
                title,
            ),
            name.to_string(),
        )
    };
    let mut radial_views = Vec::new();
    radial_views.push(radial(
        plane(n as i32),
        200.0,
        "xy_bed_top.png",
        "Top of the bed",
    ));
    radial_views.push(radial(
        plane(0),
        200.0,
        "xy_bed_floor.png",
        "Floor of the bed",
    ));
    radial_views.push(radial(plane(-2), 140.0, "xy_cone.png", "In the cone"));
    radial_views.push(radial(plane(-10), 60.0, "xy_tube.png", "In the tube"));
    for (mut v, name) in radial_views {
        let z = v.centre.z - dz_t;
        // Mark the wall (the hopper's radius at this height) as a ring.
        let rad = if z >= floor {
            hopper.radius_cm()
        } else {
            let t = (z - hopper.cone_bottom_z_cm()) / hopper.cone_height_cm();
            if z >= hopper.cone_bottom_z_cm() {
                hopper.outlet_radius_cm() + t * (hopper.radius_cm() - hopper.outlet_radius_cm())
            } else {
                hopper.outlet_radius_cm()
            }
        };
        v.rings.push(Ring {
            h: 0.0,
            v: 0.0,
            r: rad,
        });
        save(render(a, &v), &name)?;
    }

    // Zoom: the tile at the axis, its central plane.
    let zc = floor + r + 0.5 * h;
    let in_tile = |k: &KeptBall| {
        let [x, y, z] = k.centre_cm;
        (z - zc).abs() < 1e-6 && x.r_hypot(y) < seker::pitch_cm() * 0.55
    };
    let nf = c.kept.iter().filter(|k| in_tile(k) && k.fuel).count();
    let nm = c.kept.iter().filter(|k| in_tile(k) && !k.fuel).count();
    let mut v = View::new(
        PlotBasis::Xy,
        Position::new(0.0, 0.0, zc + dz_t),
        32.0,
        1100,
        format!(
            "One tile (black hexagon), central plane z = {zc:.3} cm: {nf} fuel + {nm} moderator balls centred in it"
        ),
    );
    // The tile is not a surface of the geometry: its outline is drawn for the reader.
    let apo = 0.5 * seker::pitch_cm();
    let circ = apo / (std::f64::consts::PI / 6.0).r_cos();
    let corners: Vec<[f64; 2]> = vec![
        [circ, 0.0],
        [0.5 * circ, apo],
        [-0.5 * circ, apo],
        [-circ, 0.0],
        [-0.5 * circ, -apo],
        [0.5 * circ, -apo],
    ];
    for i in 0..6 {
        v.lines.push((corners[i], corners[(i + 1) % 6]));
    }
    save(render(a, &v), "tile_xy.png")?;

    // Zoom: one fuel ball in that plane, then one particle in it.
    if let Some(fb) = c
        .kept
        .iter()
        .filter(|k| in_tile(k) && k.fuel)
        .min_by(|p, q| {
            let d = |k: &&KeptBall| k.centre_cm[0].r_hypot(k.centre_cm[1]);
            d(p).total_cmp(&d(q))
        })
    {
        let ctr = fb.centre_cm;
        let v = View::new(
            PlotBasis::Xy,
            Position::new(ctr[0], ctr[1], ctr[2] + dz_t),
            7.0,
            1000,
            format!(
                "One fuel ball at ({:.3}, {:.3}, {:.3}) cm: graphite shell to 3.0 cm, fuel zone 2.5 cm, TRISO layer z = {:.4}",
                ctr[0], ctr[1], ctr[2], ctr[2] + dz_t
            ),
        );
        save(render(a, &v), "fuel_ball_xy.png")?;
        if let Some(k) = find_kernel(a, [ctr[0] - 0.3, ctr[1] - 0.3, 0.0], ctr[2] + dz_t) {
            let v = View::new(
                PlotBasis::Xy,
                k,
                0.12,
                800,
                format!(
                    "One TRISO particle in that ball, near ({:.3}, {:.3}, {:.3}) cm, 0.12 cm window",
                    k.x, k.y, k.z
                ),
            );
            save(render(a, &v), "triso_xy.png")?;
        }
    }

    // The rejected balls.
    let zp = plane((n / 2) as i32);
    let mut v = View::new(
        PlotBasis::Xy,
        Position::new(50.0, 50.0, zp + dz_t),
        100.0,
        1100,
        format!("Rejected balls, x-y, z = {zp:.3} cm, quadrant x, y > 0"),
    );
    let in_window = |x: f64, y: f64| (0.0..100.0).contains(&x) && (0.0..100.0).contains(&y);
    let mut shown = 0;
    for rj in c
        .rejected
        .iter()
        .filter(|k| (k.centre_cm[2] - zp).abs() < 1e-6)
    {
        if in_window(rj.centre_cm[0], rj.centre_cm[1]) {
            v.rings.push(Ring {
                h: rj.centre_cm[0],
                v: rj.centre_cm[1],
                r,
            });
            shown += 1;
        }
    }
    v.rings.push(Ring {
        h: 0.0,
        v: 0.0,
        r: hopper.radius_cm(),
    });
    v.extra_legend = vec![(
        RING,
        "rejected ball's would-be place (ring), and the wall r = 90 cm".to_string(),
    )];
    v.title = format!(
        "{}: {shown} rings here of {} rejected in the plane, {} kept",
        v.title,
        rejected_in(zp),
        kept_in(zp)
    );
    save(render(a, &v), "rejected_xy_wall.png")?;

    for (centre, width, name, what) in [
        (
            Position::new(45.0, 0.0, floor + 0.5 * (top - floor)),
            130.0,
            "rejected_rz_wall.png",
            "side wall",
        ),
        (
            Position::new(0.0, 0.0, -105.0),
            300.0,
            "rejected_rz_lower.png",
            "cone and tube",
        ),
    ] {
        let mut v = View::new(PlotBasis::Xz, centre, width, 1100, String::new());
        let (mut shown, mut total) = (0, 0);
        for rj in &c.rejected {
            let [x, y, z] = rj.centre_cm;
            total += 1;
            if y.abs() < r {
                let cut_r = (r * r - y * y).sqrt();
                v.rings.push(Ring {
                    h: x,
                    v: z,
                    r: cut_r,
                });
                shown += 1;
            }
        }
        v.lines = profile_lines(&hopper, top);
        v.extra_legend = vec![(
            RING,
            "cross-section a rejected ball would have had in this slice".to_string(),
        )];
        v.title = format!(
            "Rejected at the {what}, x-z at y = 0: {shown} of {total} rejected balls cross this slice"
        );
        save(render(a, &v), name)?;
    }
    save(meridional(&hopper, c, top), "rejected_meridional.png")?;
    println!("  {} pictures", written.len());
    Ok(())
}
