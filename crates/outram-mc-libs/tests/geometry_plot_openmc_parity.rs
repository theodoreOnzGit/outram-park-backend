// SPDX-License-Identifier: GPL-3.0

//! **V&V gate — the ported plotter against `openmc --plot`, pixel for pixel.**
//! GitHub #268.
//!
//! # Methodology
//!
//! `verification_and_validation/geometry_plotting/openmc_inputs/make_references.py`
//! builds four small CSG models with OpenMC's Python API and runs
//! `openmc --plot` (OpenMC 0.16.1-dev25, commit d7d3284a1, libpng). Its 17 PNGs
//! are committed in `openmc_reference/`. This test rebuilds the **same** models
//! with this crate's types — cells in ascending-id order and materials in list
//! order, which is how OpenMC indexes them, so default colours line up — renders
//! the same plots **in the same order from one plotter seed**, and compares
//! every pixel's RGB. RGB, not a mask: the colour stream is part of what is
//! being verified.
//!
//! Pass criteria, fixed before the comparison was run:
//!
//! - **slice plots: zero differing pixels.** A slice is a pure point-location
//!   classification with a bit-for-bit ported colour stream; any difference is
//!   a defect or must be explained pixel by pixel.
//! - **ray-traced plots: at most the recorded count**, each recorded count
//!   explained in the README. Shading goes through `exp`, `tan`, `sqrt` and
//!   truncation to a byte, and the tracers relocate differently after a
//!   crossing (see `geometry::plot::raytrace`), so exact equality was not
//!   promised; the recorded numbers are the measured ones (all 0), not a
//!   tolerance chosen to pass.
//!
//! # Results (2026-09-25, x86_64 Linux/glibc, OpenMC d7d3284a1)
//!
//! **0 differing pixels in all 17 images (241 041 pixels)** — the 12 slices
//! and the 5 ray traces alike. Negative controls: a plotter seed of 2 instead
//! of 1 changes 2733 Godiva pixels, a half-pixel origin shift 892 lattice
//! pixels, moving the guide tube to a corner 1985, and a diffuse fraction of
//! 0.31 instead of 0.30 changes 2231 pixels of the lit lattice. Per-image table
//! in `verification_and_validation/geometry_plotting/README.md`.
//!
//! Set `OUTRAM_PLOT_OUT=<dir>` to keep this crate's PNGs (that is how
//! `geometry_plotting/outram/` was produced).

use std::path::PathBuf;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::plot::{
    decode_png, Camera, ColourScheme, ImageData, PlotBasis, PlotColourBy, Projection, Rgb,
    SlicePlot, SolidRayTracePlot, WireframeRayTracePlot, BLACK, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::tally::mesh::RegularMesh;

use HalfSpaceSense::{Inside as In, Outside as Out};

fn hs(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

/// An intersection of half-spaces, in RPN.
fn and(parts: &[(usize, HalfSpaceSense)]) -> Vec<RegionToken> {
    let mut v = Vec::new();
    for (k, &(s, sense)) in parts.iter().enumerate() {
        v.push(hs(s, sense));
        if k > 0 {
            v.push(RegionToken::Intersection);
        }
    }
    v
}

const T: BoundaryType = BoundaryType::Transmissive;
const V: BoundaryType = BoundaryType::Vacuum;

fn xp(x0: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::XPlane(XPlane { x0, bc })
}
fn yp(y0: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::YPlane(YPlane { y0, bc })
}
fn zp(z0: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::ZPlane(ZPlane { z0, bc })
}
fn zcyl(r: f64) -> SurfaceKind {
    SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r,
        bc: T,
    })
}
fn sph(x0: f64, y0: f64, r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0,
        y0,
        z0: 0.0,
        r,
        bc,
    })
}
fn mat(id: i32, region: Vec<RegionToken>, m: usize) -> Cell {
    Cell::material(id, region, m, 293.6)
}

// ─── The four models, mirroring make_references.py ─────────────────────────

fn godiva() -> Geometry {
    Geometry {
        surfaces: vec![sph(0.0, 0.0, 8.7407, V)],
        cells: vec![mat(1, and(&[(0, In)]), 0)],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Materials: 0 fuel, 1 clad, 2 water. Cells by ascending id 1..=11.
fn pin_lattice() -> Geometry {
    let surfaces = vec![
        zcyl(0.4096), // 0: fuel_or   (id 1)
        zcyl(0.475),  // 1: clad_or   (id 2)
        zcyl(0.56),   // 2: gt_ir     (id 3)
        zcyl(0.60),   // 3: gt_or     (id 4)
        xp(-1.89, T), // 4: xl
        xp(1.89, T),  // 5: xr
        yp(-1.89, T), // 6: yl
        yp(1.89, T),  // 7: yr
        zp(-10.0, V), // 8: zb
        zp(10.0, V),  // 9: zt
        xp(-3.0, V),  // 10: xo_l
        xp(3.0, V),   // 11: xo_r
        yp(-3.0, V),  // 12: yo_l
        yp(3.0, V),   // 13: yo_r
    ];
    let z = [(8, Out), (9, In)];
    let with_z = |p: &[(usize, HalfSpaceSense)]| {
        let mut v = p.to_vec();
        v.extend_from_slice(&z);
        and(&v)
    };
    let cells = vec![
        mat(1, and(&[(0, In)]), 0),
        mat(2, and(&[(0, Out), (1, In)]), 1),
        mat(3, and(&[(1, Out)]), 2),
        mat(4, and(&[(2, In)]), 2),
        mat(5, and(&[(2, Out), (3, In)]), 1),
        mat(6, and(&[(3, Out)]), 2),
        Cell::fill(
            7,
            with_z(&[(4, Out), (5, In), (6, Out), (7, In)]),
            CellFill::Lattice(0),
            Position::ZERO,
        ),
        mat(8, with_z(&[(5, Out), (11, In), (12, Out), (13, In)]), 2),
        mat(9, with_z(&[(10, Out), (4, In), (12, Out), (13, In)]), 2),
        mat(10, with_z(&[(4, Out), (5, In), (7, Out), (13, In)]), 2),
        mat(11, with_z(&[(4, Out), (5, In), (12, Out), (6, In)]), 2),
    ];
    let universes = vec![
        Universe {
            id: 0,
            cell_indices: vec![6, 7, 8, 9, 10],
        },
        Universe {
            id: 1,
            cell_indices: vec![0, 1, 2],
        },
        Universe {
            id: 2,
            cell_indices: vec![3, 4, 5],
        },
    ];
    let mut u = vec![1usize; 9];
    u[4] = 2;
    let lat = RectLattice {
        id: 10,
        n: [3, 3, 1],
        lower_left: Position::new(-1.89, -1.89, 0.0),
        pitch: [1.26, 1.26, 1.0],
        universes: u,
        outer: None,
    };
    Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![Lattice::Rect(lat)],
        root_universe: 0,
    }
}

/// Materials a, b, c = 0, 1, 2. Cells 1 and 2 overlap on purpose; cell 4 is void.
fn overlap() -> Geometry {
    let surfaces = vec![
        sph(-1.0, 0.0, 2.0, T), // 0: s1
        sph(1.0, 0.0, 2.0, T),  // 1: s2
        sph(0.0, 3.0, 0.5, T),  // 2: s3
        xp(-4.0, V),
        xp(4.0, V),
        yp(-4.0, V),
        yp(4.0, V),
        zp(-4.0, V),
        zp(4.0, V),
    ];
    let cells = vec![
        mat(1, and(&[(0, In)]), 0),
        mat(2, and(&[(1, In)]), 1),
        mat(
            3,
            and(&[
                (0, Out),
                (1, Out),
                (2, Out),
                (3, Out),
                (4, In),
                (5, Out),
                (6, In),
                (7, Out),
                (8, In),
            ]),
            2,
        ),
        Cell::fill(4, and(&[(2, In)]), CellFill::Void, Position::ZERO),
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2, 3],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Materials ball, box = 0, 1.
fn sphere_in_box() -> Geometry {
    let surfaces = vec![
        sph(0.0, 0.0, 3.0, T),
        xp(-5.0, V),
        xp(5.0, V),
        yp(-5.0, V),
        yp(5.0, V),
        zp(-5.0, V),
        zp(5.0, V),
    ];
    let cells = vec![
        mat(1, and(&[(0, In)]), 0),
        mat(
            2,
            and(&[
                (0, Out),
                (1, Out),
                (2, In),
                (3, Out),
                (4, In),
                (5, Out),
                (6, In),
            ]),
            1,
        ),
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

// ─── Comparison harness ─────────────────────────────────────────────────────

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/geometry_plotting")
}

fn reference(name: &str) -> ImageData {
    let p = vv_dir()
        .join("openmc_reference")
        .join(format!("{name}.png"));
    let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    decode_png(&bytes).unwrap_or_else(|e| panic!("decode {}: {e:?}", p.display()))
}

/// Differing pixels, the largest channel difference, and up to 8 examples.
fn compare(name: &str, ours: &ImageData) -> (usize, u8) {
    if let Ok(dir) = std::env::var("OUTRAM_PLOT_OUT") {
        std::fs::create_dir_all(&dir).expect("create OUTRAM_PLOT_OUT");
        ours.write_png(PathBuf::from(dir).join(format!("{name}.png")))
            .expect("write png");
    }
    let r = reference(name);
    assert_eq!(
        (ours.width, ours.height),
        (r.width, r.height),
        "{name}: size"
    );
    let mut n = 0;
    let mut max_d = 0u8;
    let mut shown = 0;
    for y in 0..r.height {
        for x in 0..r.width {
            let (a, b) = (ours.get(x, y), r.get(x, y));
            if a != b {
                n += 1;
                let d =
                    a.r.abs_diff(b.r)
                        .max(a.g.abs_diff(b.g))
                        .max(a.b.abs_diff(b.b));
                max_d = max_d.max(d);
                if shown < 8 {
                    eprintln!("  {name}: ({x},{y}) ours {a:?} openmc {b:?}");
                    shown += 1;
                }
            }
        }
    }
    eprintln!(
        "{name}: {n} of {} pixels differ ({:.3} %), max channel diff {max_d}",
        r.width * r.height,
        100.0 * n as f64 / (r.width * r.height) as f64
    );
    (n, max_d)
}

fn slice(basis: PlotBasis, o: (f64, f64, f64), w: [f64; 2], px: [usize; 2]) -> SlicePlot {
    SlicePlot::new(basis, Position::new(o.0, o.1, o.2), w, px)
}

// ─── Slice plots: zero differences required ────────────────────────────────

#[test]
fn slice_godiva_matches_openmc() {
    let g = godiva();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let s = ColourScheme::new(PlotColourBy::Cell, g.cells.len(), &mut seed);
    let img = slice(PlotBasis::Xy, (0.0, 0.0, 0.0), [24.0, 24.0], [81, 81]).create_image(&g, &s);
    assert_eq!(compare("godiva_xy_cell", &img).0, 0);
}

#[test]
fn slice_pin_lattice_matches_openmc() {
    let g = pin_lattice();
    let (nc, nm) = (g.cells.len(), 3);
    let mut seed = DEFAULT_PLOTTER_SEED;
    let cell = |seed: &mut u64| ColourScheme::new(PlotColourBy::Cell, nc, seed);
    let matl = |seed: &mut u64| ColourScheme::new(PlotColourBy::Material, nm, seed);
    let xy = || slice(PlotBasis::Xy, (0.0, 0.0, 0.0), [6.4, 6.4], [160, 160]);

    // Plot order and seed consumption exactly as plots.xml.
    let s1 = cell(&mut seed);
    let s2 = matl(&mut seed);
    let s3 = cell(&mut seed);
    let s4 = matl(&mut seed);
    let s5 = cell(&mut seed);
    let s6 = matl(&mut seed);

    let mut bad = 0;
    bad += compare("lattice_xy_cell", &xy().create_image(&g, &s1)).0;
    bad += compare("lattice_xy_material", &xy().create_image(&g, &s2)).0;
    bad += compare(
        "lattice_xz_cell",
        &slice(PlotBasis::Xz, (0.0, 0.2, 0.0), [6.4, 22.0], [64, 220]).create_image(&g, &s3),
    )
    .0;
    bad += compare(
        "lattice_yz_material",
        &slice(PlotBasis::Yz, (0.1, 0.0, 5.0), [6.4, 6.4], [100, 100]).create_image(&g, &s4),
    )
    .0;
    bad += compare("lattice_xy_level0", &xy().at_level(0).create_image(&g, &s5)).0;
    let mesh = RegularMesh {
        lower_left: [-3.0, -3.0, -10.0],
        upper_right: [3.0, 3.0, 10.0],
        dimension: [4, 4, 1],
    };
    bad += compare(
        "lattice_xy_meshlines",
        &xy().with_meshlines(mesh, 1, BLACK).create_image(&g, &s6),
    )
    .0;
    assert_eq!(bad, 0, "lattice slices differ from OpenMC");
}

#[test]
fn slice_overlap_mask_material_matches_openmc() {
    let g = overlap();
    let (nc, nm) = (g.cells.len(), 3);
    let mut seed = DEFAULT_PLOTTER_SEED;
    let p = || slice(PlotBasis::Xy, (0.0, 0.0, 0.0), [10.0, 10.0], [100, 100]);

    let s1 = ColourScheme::new(PlotColourBy::Cell, nc, &mut seed);
    let s2 = ColourScheme::new(PlotColourBy::Material, nm, &mut seed)
        .with_background(Rgb::new(10, 20, 30))
        .with_overlap_colour(Rgb::new(0, 255, 0));
    let s3 = ColourScheme::new(PlotColourBy::Cell, nc, &mut seed)
        .with_colour(2, Rgb::new(200, 200, 0))
        .with_mask(&[1], Some(Rgb::new(40, 40, 40)));
    let s4 = ColourScheme::new(PlotColourBy::Material, nm, &mut seed).with_mask(&[2], None);
    let s5 = ColourScheme::new(PlotColourBy::Cell, nc, &mut seed);

    let mut bad = 0;
    bad += compare(
        "overlap_cell_show",
        &p().showing_overlaps().create_image(&g, &s1),
    )
    .0;
    bad += compare(
        "overlap_material_show",
        &p().showing_overlaps().create_image(&g, &s2),
    )
    .0;
    bad += compare("mask_cell", &p().create_image(&g, &s3)).0;
    bad += compare("mask_material", &p().create_image(&g, &s4)).0;
    bad += compare("level1_cell", &p().at_level(1).create_image(&g, &s5)).0;
    assert_eq!(bad, 0, "overlap/mask slices differ from OpenMC");
}

// ─── Ray-traced plots: recorded counts ─────────────────────────────────────

fn camera(pos: (f64, f64, f64), at: (f64, f64, f64), px: [usize; 2], fov: f64) -> Camera {
    Camera {
        position: Position::new(pos.0, pos.1, pos.2),
        look_at: Position::new(at.0, at.1, at.2),
        up: Direction::new(0.0, 0.0, 1.0),
        pixels: px,
        projection: Projection::Perspective {
            horizontal_fov_deg: fov,
        },
    }
}

/// Differing pixels allowed in a ray-traced image.
///
/// **Measured 2026-09-25 on x86_64 Linux/glibc: 0 on all five ray-traced
/// images** (12 000 or 10 000 pixels each), so the gate there is exact. The
/// shading passes through `tan`, `exp` and a truncation to a byte, and the
/// references were produced against glibc's libm; on another platform's libm a
/// last-bit difference can flip a byte. That allowance (0.5 % of an image) is
/// NOT a measurement — no non-glibc host has run this — and is stated as such.
const RAYTRACE_MAX_DIFF: usize = if cfg!(all(target_os = "linux", target_env = "gnu")) {
    0
} else {
    60
};
const SPHERE_WIREFRAME_MAX_DIFF: usize = RAYTRACE_MAX_DIFF;
const SPHERE_SOLID_MAX_DIFF: usize = RAYTRACE_MAX_DIFF;
const LATTICE_WIREFRAME_MAX_DIFF: usize = RAYTRACE_MAX_DIFF;
const LATTICE_SOLID_MAX_DIFF: usize = RAYTRACE_MAX_DIFF;
const LATTICE_ORTHO_MAX_DIFF: usize = RAYTRACE_MAX_DIFF;

#[test]
fn raytrace_sphere_matches_openmc() {
    let g = sphere_in_box();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let cam = camera((20.0, 15.0, 10.0), (0.0, 0.0, 0.0), [120, 100], 45.0);

    let s1 = ColourScheme::new(PlotColourBy::Material, 2, &mut seed)
        .with_colour(1, Rgb::new(80, 160, 220));
    let w = WireframeRayTracePlot::new(cam, 2).with_xs(1, 0.05);
    let s2 = ColourScheme::new(PlotColourBy::Material, 2, &mut seed);
    let s = SolidRayTracePlot::new(cam, 2).with_opaque(0);

    let (a, _) = compare("sphere_wireframe", &w.create_image(&g, &s1));
    let (b, _) = compare("sphere_solid", &s.create_image(&g, &s2));
    assert!(
        a <= SPHERE_WIREFRAME_MAX_DIFF && b <= SPHERE_SOLID_MAX_DIFF,
        "wire {a} solid {b}"
    );
}

#[test]
fn raytrace_lattice_matches_openmc() {
    let g = pin_lattice();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let cam = camera((12.0, 9.0, 18.0), (0.0, 0.0, 5.0), [120, 100], 50.0);

    let s1 = ColourScheme::new(PlotColourBy::Material, 3, &mut seed)
        .with_colour(2, Rgb::new(120, 170, 255))
        .with_colour(1, Rgb::new(150, 150, 150));
    let mut w = WireframeRayTracePlot::new(cam, 3)
        .with_xs(2, 0.02)
        .with_xs(1, 0.5);
    w.wireframe_thickness = 2;

    let s2 = ColourScheme::new(PlotColourBy::Material, 3, &mut seed);
    let mut s = SolidRayTracePlot::new(cam, 3).with_opaque(0).with_opaque(1);
    s.light_position = Some(Position::new(-10.0, 20.0, 30.0));
    s.diffuse_fraction = 0.3;

    let s3 = ColourScheme::new(PlotColourBy::Cell, g.cells.len(), &mut seed);
    let ortho = WireframeRayTracePlot::new(
        Camera {
            position: Position::new(0.0, -30.0, 0.0),
            look_at: Position::ZERO,
            up: Direction::new(0.0, 0.0, 1.0),
            pixels: [100, 100],
            projection: Projection::Orthographic { width: 8.0 },
        },
        g.cells.len(),
    );

    let (a, _) = compare("lattice_wireframe", &w.create_image(&g, &s1));
    let (b, _) = compare("lattice_solid", &s.create_image(&g, &s2));
    let (c, _) = compare("lattice_wireframe_ortho", &ortho.create_image(&g, &s3));
    assert!(
        a <= LATTICE_WIREFRAME_MAX_DIFF
            && b <= LATTICE_SOLID_MAX_DIFF
            && c <= LATTICE_ORTHO_MAX_DIFF,
        "wire {a} solid {b} ortho {c}"
    );
}

// ─── Negative controls: the harness must be able to fail ───────────────────

/// A comparison that cannot fail proves nothing. Each perturbation below is
/// the smallest change of its kind — one plotter seed, half a pixel of origin,
/// 0.01 in the diffuse fraction, one lattice tile's universe — and each must
/// be seen.
#[test]
fn harness_detects_small_perturbations() {
    // Seed 2 instead of 1: a different default colour for the one cell.
    let g = godiva();
    let mut seed = DEFAULT_PLOTTER_SEED + 1;
    let s = ColourScheme::new(PlotColourBy::Cell, 1, &mut seed);
    let img = slice(PlotBasis::Xy, (0.0, 0.0, 0.0), [24.0, 24.0], [81, 81]).create_image(&g, &s);
    assert!(
        compare_quiet("godiva_xy_cell", &img) > 0,
        "seed change not detected"
    );

    // Origin moved by half a pixel (0.02 cm of 6.4 cm / 160 px).
    let g = pin_lattice();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let s = ColourScheme::new(PlotColourBy::Cell, g.cells.len(), &mut seed);
    let img = slice(PlotBasis::Xy, (0.02, 0.0, 0.0), [6.4, 6.4], [160, 160]).create_image(&g, &s);
    assert!(
        compare_quiet("lattice_xy_cell", &img) > 0,
        "half-pixel shift not detected"
    );

    // The guide tube moved to a corner tile.
    let mut g2 = pin_lattice();
    if let Lattice::Rect(l) = &mut g2.lattices[0] {
        l.universes[4] = 1;
        l.universes[0] = 2;
    }
    let mut seed = DEFAULT_PLOTTER_SEED;
    let s = ColourScheme::new(PlotColourBy::Cell, g2.cells.len(), &mut seed);
    let img = slice(PlotBasis::Xy, (0.0, 0.0, 0.0), [6.4, 6.4], [160, 160]).create_image(&g2, &s);
    assert!(
        compare_quiet("lattice_xy_cell", &img) > 0,
        "moved lattice tile not detected"
    );

    // Diffuse fraction 0.31 instead of 0.3 in the lit lattice.
    let g = pin_lattice();
    let mut seed = DEFAULT_PLOTTER_SEED;
    let cam = camera((12.0, 9.0, 18.0), (0.0, 0.0, 5.0), [120, 100], 50.0);
    let _ = ColourScheme::new(PlotColourBy::Material, 3, &mut seed);
    let s2 = ColourScheme::new(PlotColourBy::Material, 3, &mut seed);
    let mut s = SolidRayTracePlot::new(cam, 3).with_opaque(0).with_opaque(1);
    s.light_position = Some(Position::new(-10.0, 20.0, 30.0));
    s.diffuse_fraction = 0.31;
    assert!(
        compare_quiet("lattice_solid", &s.create_image(&g, &s2)) > 0,
        "shading change not detected"
    );
}

fn compare_quiet(name: &str, ours: &ImageData) -> usize {
    let r = reference(name);
    let n = r.count_differences(ours).expect("same size");
    eprintln!("negative control vs {name}: {n} pixels differ");
    n
}
