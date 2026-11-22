// SPDX-License-Identifier: AGPL-3.0-only

//! **V&V gate: a region of bare half-spaces is their implicit intersection**
//! (GitHub #1576).
//!
//! # Methodology
//!
//! Upstream OpenMC inserts an `OP_INTERSECTION` between adjacent operands
//! when it parses a region (`src/cell.cpp:626`, `Region::Region`, OpenMC
//! `8f79d9d0`). `Cell::contains` used to test only the **last** token of a
//! region with no operators, so `[inside(cyl), outside(zlo), inside(zhi)]`
//! was the half-space `z < 4` and the first picture of the #259 wedge model
//! showed a plane where a cylinder was meant.
//!
//! Reference: the same finite cylinder written with explicit `Intersection`
//! tokens, which `contains` always evaluated correctly. Both are rasterised
//! with `render_material_slice` on an xy slice (through the axis) and an xz
//! slice; the pass criteria are
//!
//! 1. the raw pixel arrays are **identical** (zero differing pixels);
//! 2. the fuel area matches the analytic area, `pi R^2` on the xy slice and
//!    `2 R x 2 H` on the xz slice, within 1 % (bound stated before running:
//!    the boundary band is about two pixels of perimeter, a few tenths of a
//!    percent at 360 x 360 over 12 cm);
//! 3. `locate` finds no cell at a point outside the cylinder, which the
//!    last-token rule got wrong.
//!
//! # Results (2026-11-14, Rust 1.99.1, release)
//!
//! Recorded in `verification_and_validation/implicit_region_1576/README.md`
//! together with the images (set `OUTRAM_PLOT_OUT` to write them).

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;

const R: f64 = 3.0;
const HALF_H: f64 = 4.0;

fn hs(i: usize, s: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx: i, sense: s }
}

fn model(implicit: bool) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: -HALF_H, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF_H, bc: BoundaryType::Vacuum }),
    ];
    let mut region = vec![
        hs(0, HalfSpaceSense::Inside),
        hs(1, HalfSpaceSense::Outside),
        hs(2, HalfSpaceSense::Inside),
    ];
    if !implicit {
        region.insert(2, RegionToken::Intersection);
        region.push(RegionToken::Intersection);
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn implicit_and_explicit_regions_draw_the_same_cylinder() {
    let out = std::env::var_os("OUTRAM_PLOT_OUT").map(std::path::PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).expect("create plot dir");
    }
    let fuel = Rgb::new(200, 90, 40);
    let palette = [(fuel, "fuel cylinder")];
    let n = 360usize;
    let width = 12.0;
    let px_area = (width / n as f64) * (width / n as f64);
    for (basis, tag, analytic) in [
        (PlotBasis::Xy, "xy_z0", std::f64::consts::PI * R * R),
        (PlotBasis::Xz, "xz_y0", 2.0 * R * 2.0 * HALF_H),
    ] {
        let plot = SlicePlot::new(basis, Position::new(0.0, 0.0, 0.0), [width, width], [n, n]);
        let (raw_i, img_i) = render_material_slice(
            &model(true),
            &plot,
            &palette,
            &format!("{tag}: implicit region [in cyl, out zlo, in zhi]"),
        );
        let (raw_e, _) = render_material_slice(&model(false), &plot, &palette, "explicit");
        let differing = raw_i.pixels.iter().zip(&raw_e.pixels).filter(|(a, b)| a != b).count();
        assert_eq!(differing, 0, "{tag}: implicit and explicit regions differ in {differing} pixels");
        let count = raw_i.pixels.iter().filter(|c| **c == fuel).count();
        let area = count as f64 * px_area;
        let rel = (area - analytic).abs() / analytic;
        println!(
            "{tag}: {count} fuel pixels, area {area:.4} cm2 vs analytic {analytic:.4} ({:+.3} %)",
            100.0 * (area - analytic) / analytic
        );
        assert!(rel < 0.01, "{tag}: fuel area {area} vs analytic {analytic}");
        if let Some(dir) = &out {
            img_i.write_png(dir.join(format!("implicit_{tag}.png"))).expect("write png");
        }
    }
}

#[test]
fn locate_rejects_a_point_outside_the_cylinder_for_an_implicit_region() {
    let g = model(true);
    let u = Direction::new(0.0, 0.0, 1.0);
    let outside_radially = Position::new(R + 1.0, 0.0, 0.0);
    assert!(g.locate(outside_radially, u, SurfaceToken::NONE).is_none());
    let inside = Position::new(R - 1.0, 0.0, 0.0);
    assert!(g.locate(inside, u, SurfaceToken::NONE).is_some());
    let above = Position::new(0.0, 0.0, HALF_H + 1.0);
    assert!(g.locate(above, u, SurfaceToken::NONE).is_none());
}
