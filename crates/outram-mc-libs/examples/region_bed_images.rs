// SPDX-License-Identifier: AGPL-3.0-only
//! **Pictures of `pebble_beds::region_bed`** (gh:#1656): the test region (a
//! z-cylinder of radius 10 cm centred at x = 15, cut by the plane x = 8,
//! 0 < z < 40) filled four ways, each drawn x-z at y = 0 from an assembled
//! geometry (region minus the pebbles, plus one sphere cell per pebble):
//! a cubic lattice, random addition, a DEM pour under the default gravity
//! (−z), and one with gravity along +x, each 20 000 DEM steps (0.7 s).
//! Then HTR-10's DEM vessel as CSG (`region_bed::htr10_vessel_geometry`,
//! gh:#1656): 4000 pebbles poured in metres, the route pinned bit for bit
//! against `htr10_fill`, 15 000 steps (0.5 s), drawn x-z at y = 0 and x-y at
//! z = −20 cm (in the conus), at ×100 so the axes read in cm.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example region_bed_images -- <out dir>
//! ```
//!
//! CLI glue (writes PNGs); the packing is `region_bed`'s, reached by its
//! tests.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::pebble_beds::region_bed::{htr10_vessel_geometry, pack_in_region, pack_in_region_in, LengthUnit, RegionBed, RegionPacking};
use std::path::PathBuf;

fn hs(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

fn region() -> Geometry {
    let t = BoundaryType::Transmissive;
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 15.0, y0: 0.0, r: 10.0, bc: t }),
        SurfaceKind::ZPlane(ZPlane { z0: 0.0, bc: t }),
        SurfaceKind::ZPlane(ZPlane { z0: 40.0, bc: t }),
        SurfaceKind::XPlane(XPlane { x0: 8.0, bc: t }),
    ];
    let region = vec![
        hs(0, HalfSpaceSense::Inside),
        hs(1, HalfSpaceSense::Outside),
        RegionToken::Intersection,
        hs(2, HalfSpaceSense::Inside),
        RegionToken::Intersection,
        hs(3, HalfSpaceSense::Outside),
        RegionToken::Intersection,
    ];
    Geometry { surfaces, cells: vec![Cell::material(1, region, 0, 293.6)], universes: vec![Universe { id: 0, cell_indices: vec![0] }], lattices: vec![], root_universe: 0 }
}

/// `g` (one cell) with the bed in it: pebbles material 1, the rest
/// material 0.
fn assembled(mut g: Geometry, bed: &RegionBed) -> Geometry {
    for (i, c) in bed.centres.iter().enumerate() {
        g.surfaces.push(SurfaceKind::Sphere(Sphere { x0: c.x, y0: c.y, z0: c.z, r: bed.radius, bc: BoundaryType::Transmissive }));
        let s = g.surfaces.len() - 1;
        g.cells[0].region.extend([hs(s, HalfSpaceSense::Outside), RegionToken::Intersection]);
        g.cells.push(Cell::material(100 + i as i32, vec![hs(s, HalfSpaceSense::Inside)], 1, 293.6));
        let n = g.cells.len() - 1;
        g.universes[0].cell_indices.push(n);
    }
    g
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&dir).expect("output directory");
    let g = region();
    let r = 2.0;
    let cases = [
        ("lattice", RegionPacking::Lattice { pitch: 4.2 }, "CUBIC LATTICE, PITCH 4.2 CM"),
        ("random", RegionPacking::Random { fraction: 0.3, seed: 7, max_attempts: 5000 }, "RANDOM ADDITION TO 0.30 OF THE BOX"),
        ("pour_default_gravity", RegionPacking::Pour { n: 40, seed: 3, gravity: None, steps: 20000 }, "DEM POUR, 40 PEBBLES, GRAVITY NONE (= -Z)"),
        ("pour_gravity_plus_x", RegionPacking::Pour { n: 40, seed: 3, gravity: Some([1.0, 0.0, 0.0]), steps: 20000 }, "DEM POUR, 40 PEBBLES, GRAVITY +X"),
    ];
    for (stem, packing, title) in cases {
        let bed = pack_in_region(&g, 0, r, packing).expect("bed");
        let geom = assembled(region(), &bed);
        let plot = SlicePlot { origin: Position::new(15.0, 0.0, 20.0), basis: PlotBasis::Xz, width: [24.0, 44.0], pixels: [300, 550], level: None, show_overlaps: true, meshlines: None };
        let palette = [(Rgb::new(60, 130, 200), "region (between pebbles)"), (Rgb::new(70, 70, 70), "pebble")];
        let (_raw, img) = render_material_slice(&geom, &plot, &palette, &format!("REGION_BED {title}: {} PEBBLES, X-Z AT Y = 0", bed.centres.len()));
        let path = dir.join(format!("{stem}.png"));
        img.write_png(&path).expect("png");
        eprintln!("drew {} ({} pebbles)", path.display(), bed.centres.len());
    }
    let seed = outram_park_fork_liggghts::htr10_fill::Htr10FillSettings::default().seed;
    let rm = outram_park_fork_liggghts::htr10_fill::PEBBLE_RADIUS_M;
    let bed_m = pack_in_region_in(&htr10_vessel_geometry(LengthUnit::Metre), 0, rm, RegionPacking::Pour { n: 4000, seed, gravity: None, steps: 15_000 }, LengthUnit::Metre).expect("HTR-10 pour");
    let bed = RegionBed { centres: bed_m.centres.iter().map(|c| Position::new(c.x * 100.0, c.y * 100.0, c.z * 100.0)).collect(), radius: rm * 100.0 };
    let geom = assembled(htr10_vessel_geometry(LengthUnit::Centimetre), &bed);
    let palette = [(Rgb::new(60, 130, 200), "vessel (between pebbles)"), (Rgb::new(70, 70, 70), "pebble")];
    let views = [
        ("htr10_vessel_xz", Position::new(0.0, 0.0, 30.0), PlotBasis::Xz, [200.0, 200.0], [600, 600], "X-Z AT Y = 0"),
        ("htr10_vessel_xy_conus", Position::new(0.0, 0.0, -20.0), PlotBasis::Xy, [200.0, 200.0], [600, 600], "X-Y AT Z = -20 CM (CONUS)"),
    ];
    for (stem, origin, basis, width, pixels, what) in views {
        let plot = SlicePlot { origin, basis, width, pixels, level: None, show_overlaps: true, meshlines: None };
        let (_raw, img) = render_material_slice(&geom, &plot, &palette, &format!("HTR-10 DEM VESSEL AS CSG, {} PEBBLES POURED (GH:#1656), {what}", bed.centres.len()));
        let path = dir.join(format!("{stem}.png"));
        img.write_png(&path).expect("png");
        eprintln!("drew {}", path.display());
    }
}
