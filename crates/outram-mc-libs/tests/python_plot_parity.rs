// SPDX-License-Identifier: GPL-3.0

//! **V&V gate — `geometry::plot::ModelPlot` against OpenMC's own Python
//! `Model.plot`, pixel for pixel.**
//!
//! # Methodology
//!
//! `verification_and_validation/python_plotting_parity/model_plot/openmc_inputs/make_references.py`
//! runs OpenMC 0.16.1.dev25 (commit d7d3284a1) — `Model.plot`,
//! `Universe.plot`, `Cell.plot` and `Region.plot` — on fourteen cases over
//! five small CSG models, saves each figure with `plt.savefig` (matplotlib
//! defaults) and the id map it coloured (`Model.slice_data`, same arguments).
//! Both are committed in `.../model_plot/openmc_reference/`.
//!
//! This test builds the SAME models with this crate's types (cells in
//! ascending-id order and materials in list order, which is how OpenMC indexes
//! them), emits one standalone matplotlib script per case with
//! [`ModelPlot`], and — when `OUTRAM_PYTHON` names a Python with numpy and
//! matplotlib — runs each script and compares its PNG against the reference,
//! RGBA value by RGBA value. Without `OUTRAM_PYTHON` it prints `SKIP` for the
//! image comparison and still checks that emission is deterministic and that
//! the documented error cases are raised.
//!
//! Pass criterion, fixed before the comparison ran: **0 differing pixels in
//! every case.** The script runs the same numpy/matplotlib calls as upstream on
//! the same id map, so any difference is a defect in the id map, the default
//! resolution, the domain ordering or the transcription.
//!
//! The id maps are compared separately by
//! `.../model_plot/compare.py` (cell and material channels, exact).
//!
//! # Results (2026-09-26, x86_64 Linux/glibc, OpenMC d7d3284a1, matplotlib 3.11.2)
//!
//! Recorded in `verification_and_validation/python_plotting_parity/README.md`.
//!
//! `OUTRAM_PLOT_SCRIPT_OUT=<dir>` keeps the emitted scripts and PNGs (that is
//! how `.../model_plot/outram/` was produced).

use std::path::{Path, PathBuf};

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice, RectLattice};
use outram_mc_libs::geometry::plot::{
    decode_png, AxisUnits, ColorBy, DomainColour, ModelPlot, ModelPlotError, Outline, PlotBasis,
    PlotColour, Pixels,
};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::Material;

use HalfSpaceSense::{Inside as In, Outside as Out};

fn hs(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

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
fn zcyl(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::ZCylinder(ZCylinder {
        x0: 0.0,
        y0: 0.0,
        r,
        bc,
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
fn materials(list: &[(i32, &str)]) -> Vec<Material> {
    list.iter()
        .map(|&(id, name)| Material {
            id,
            name: name.into(),
            components: vec![],
            temperature: 293.6,
        })
        .collect()
}

// ─── The models, mirroring make_references.py ──────────────────────────────

fn godiva() -> (Geometry, Vec<Material>) {
    (
        Geometry {
            surfaces: vec![sph(0.0, 0.0, 8.7407, V)],
            cells: vec![mat(1, and(&[(0, In)]), 0)],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        },
        materials(&[(1, "heu")]),
    )
}

/// Materials: 0 fuel, 1 clad, 2 water. Universe 1 is the pin.
fn pin_lattice() -> (Geometry, Vec<Material>) {
    let surfaces = vec![
        zcyl(0.4096, T),
        zcyl(0.475, T),
        zcyl(0.56, T),
        zcyl(0.60, T),
        xp(-1.89, T),
        xp(1.89, T),
        yp(-1.89, T),
        yp(1.89, T),
        zp(-10.0, V),
        zp(10.0, V),
        xp(-3.0, V),
        xp(3.0, V),
        yp(-3.0, V),
        yp(3.0, V),
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
    (
        Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![Lattice::Rect(lat)],
            root_universe: 0,
        },
        materials(&[(1, "fuel"), (2, "clad"), (3, "water")]),
    )
}

/// Materials a, b, c = 0, 1, 2. Cells 1 and 2 overlap on purpose; cell 4 is void.
fn overlap() -> (Geometry, Vec<Material>) {
    let surfaces = vec![
        sph(-1.0, 0.0, 2.0, T),
        sph(1.0, 0.0, 2.0, T),
        sph(0.0, 3.0, 0.5, T),
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
    (
        Geometry {
            surfaces,
            cells,
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1, 2, 3],
            }],
            lattices: vec![],
            root_universe: 0,
        },
        materials(&[(1, "a"), (2, "b"), (3, "c")]),
    )
}

/// Three pin universes (a, b, c = universe indices 1, 2, 3) in a three-ring
/// hex lattice, pitch 1.5, inside a z-cylinder r = 4.2. With `axial`, two
/// levels of height 2 (z in [-2, 2]); level 1 has each ring rotated by one.
/// Cells by ascending id: 1 (root), 11, 12, 21, 22, 31, 32.
fn hex(orientation: HexOrientation, axial: bool) -> (Geometry, Vec<Material>) {
    let mut surfaces = vec![zcyl(0.5, T), zcyl(4.2, V)];
    let mut root_region = vec![(1, In)];
    if axial {
        surfaces.push(zp(-2.0, V));
        surfaces.push(zp(2.0, V));
        root_region.extend_from_slice(&[(2, Out), (3, In)]);
    }
    let cells = vec![
        Cell::fill(1, and(&root_region), CellFill::Lattice(0), Position::ZERO),
        mat(11, and(&[(0, In)]), 0),
        mat(12, and(&[(0, Out)]), 3),
        mat(21, and(&[(0, In)]), 1),
        mat(22, and(&[(0, Out)]), 3),
        mat(31, and(&[(0, In)]), 2),
        mat(32, and(&[(0, Out)]), 3),
    ];
    let universes = vec![
        Universe {
            id: 0,
            cell_indices: vec![0],
        },
        Universe {
            id: 1,
            cell_indices: vec![1, 2],
        },
        Universe {
            id: 2,
            cell_indices: vec![3, 4],
        },
        Universe {
            id: 3,
            cell_indices: vec![5, 6],
        },
    ];
    let (a, b, c) = (1usize, 2usize, 3usize);
    let level0 = vec![
        vec![c, a, b, c, a, b, c, a, b, c, a, b],
        vec![b, c, a, b, c, a],
        vec![a],
    ];
    let lat = if axial {
        let mut level1: Vec<Vec<usize>> = level0[..2]
            .iter()
            .map(|r| {
                let mut v = r[1..].to_vec();
                v.push(r[0]);
                v
            })
            .collect();
        level1.push(vec![b]);
        HexLattice::from_rings_3d(
            5,
            orientation,
            Position::ZERO,
            1.5,
            2.0,
            &[level0, level1],
            Some(a),
        )
    } else {
        HexLattice::from_rings(5, orientation, Position::ZERO, 1.5, &level0, Some(a))
    };
    (
        Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![Lattice::Hex(lat)],
            root_universe: 0,
        },
        materials(&[(1, "m1"), (2, "m2"), (3, "m3"), (4, "m4")]),
    )
}

fn source_points() -> Vec<Position> {
    (0..60)
        .map(|i: i64| {
            Position::new(
                ((i % 13) - 6) as f64 * 0.9,
                ((i % 7) - 3) as f64 * 1.7,
                ((i % 5) - 2) as f64 * 0.8,
            )
        })
        .collect()
}

fn kw(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn origin_width(p: &mut ModelPlot, o: [f64; 3], w: [f64; 2]) {
    p.origin = Some(Position::new(o[0], o[1], o[2]));
    p.width = Some(w);
}

/// Every case, as `(name, script)`, in make_references.py's order.
fn cases() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let emit = |p: &ModelPlot, g: &Geometry, m: &[Material]| p.emit(g, m).expect("emits");

    let (g, m) = godiva();
    out.push(("godiva_default", emit(&ModelPlot::new(), &g, &m)));

    let (g, m) = pin_lattice();
    out.push(("lattice_default", emit(&ModelPlot::new(), &g, &m)));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [6.4, 6.4]);
    p.pixels = Pixels::Exact([160, 160]);
    p.color_by = ColorBy::Material;
    p.legend = true;
    p.colors = Some(vec![
        DomainColour {
            id: 1,
            name: "fuel".into(),
            colour: PlotColour::Rgb([255, 0, 0]),
        },
        DomainColour {
            id: 2,
            name: "clad".into(),
            colour: PlotColour::Named("gray".into()),
        },
        DomainColour {
            id: 3,
            name: "water".into(),
            colour: PlotColour::Named("LightBlue".into()),
        },
    ]);
    out.push(("lattice_material_colors_legend", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    p.basis = PlotBasis::Xz;
    origin_width(&mut p, [0.0, 0.2, 0.0], [6.4, 22.0]);
    p.pixels = Pixels::Exact([100, 300]);
    p.seed = Some(3);
    p.legend = true;
    p.axis_units = AxisUnits::Mm;
    out.push(("lattice_xz_seed_legend_mm", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    p.basis = PlotBasis::Yz;
    origin_width(&mut p, [0.1, 0.0, 5.0], [6.4, 6.4]);
    p.pixels = Pixels::Total(22500);
    p.color_by = ColorBy::Material;
    p.outline = Outline::On;
    out.push(("lattice_yz_outline", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [6.4, 6.4]);
    p.pixels = Pixels::Exact([120, 120]);
    p.outline = Outline::Only;
    out.push(("lattice_outline_only", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [6.4, 6.4]);
    p.pixels = Pixels::Exact([120, 120]);
    p.imshow_kwargs = kw(&[("alpha", "0.6"), ("interpolation", "'nearest'")]);
    out.push(("lattice_imshow_kwargs", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    p.pixels = Pixels::Exact([100, 100]);
    out.push((
        "universe_default",
        p.emit_universe(&g, 1, &m).expect("emits"),
    ));
    let mut p = ModelPlot::new();
    p.pixels = Pixels::Exact([80, 80]);
    out.push(("cell_default", p.emit_cell(&g, 1, &m).expect("emits")));

    let (g, m) = overlap();
    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [10.0, 10.0]);
    p.pixels = Pixels::Exact([100, 100]);
    p.show_overlaps = true;
    p.overlap_color = PlotColour::Named("yellow".into());
    out.push(("overlap_show_yellow", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [10.0, 10.0]);
    p.pixels = Pixels::Exact([100, 100]);
    p.color_by = ColorBy::Material;
    p.seed = Some(5);
    p.legend = true;
    p.legend_kwargs = kw(&[("loc", "'lower left'")]);
    out.push(("overlap_material_seed_legend", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    p.width = Some([8.0, 8.0]);
    p.pixels = Pixels::Exact([100, 100]);
    let region = vec![hs(0, In), hs(1, In), RegionToken::Union];
    out.push((
        "region_union",
        p.emit_region(&g, region, &m).expect("emits"),
    ));

    let (g, m) = godiva();
    let mut p = ModelPlot::new();
    p.pixels = Pixels::Exact([120, 120]);
    p.source_points = Some(source_points());
    p.plane_tolerance = 2.0;
    p.source_kwargs = kw(&[("color", "'k'"), ("s", "4")]);
    out.push(("godiva_source_scatter", emit(&p, &g, &m)));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [9.0, 9.0]);
    p.pixels = Pixels::Exact([150, 150]);
    p.seed = Some(7);
    p.legend = true;
    let (g, m) = hex(HexOrientation::Y, false);
    out.push(("hex_seed_legend", emit(&p, &g, &m)));
    let (g, m) = hex(HexOrientation::X, false);
    out.push(("hex_x_seed_legend", emit(&p, &g, &m)));

    let (g, m) = hex(HexOrientation::Y, true);
    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0, 0.0, 1.0], [9.0, 9.0]);
    p.pixels = Pixels::Exact([150, 150]);
    p.color_by = ColorBy::Material;
    out.push(("hex3d_xy_upper", emit(&p, &g, &m)));
    let mut p = ModelPlot::new();
    p.basis = PlotBasis::Xz;
    origin_width(&mut p, [0.0; 3], [9.0, 4.4]);
    p.pixels = Pixels::Exact([180, 88]);
    p.color_by = ColorBy::Material;
    p.seed = Some(2);
    p.legend = true;
    out.push(("hex3d_xz", emit(&p, &g, &m)));
    out
}

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/python_plotting_parity/model_plot")
}

#[test]
fn emission_is_deterministic() {
    let a = cases();
    let b = cases();
    assert_eq!(a.len(), 17);
    for ((n, s1), (_, s2)) in a.iter().zip(&b) {
        assert_eq!(s1, s2, "{n}: two emissions differ");
    }
}

#[test]
fn upstream_errors_are_raised() {
    let (g, m) = godiva();
    let mut p = ModelPlot::new();
    p.legend = true;
    assert_eq!(p.emit(&g, &m), Err(ModelPlotError::LegendWithoutColours));
    let mut p = ModelPlot::new();
    p.pixels = Pixels::Exact([0, 10]);
    assert_eq!(p.emit(&g, &m), Err(ModelPlotError::Pixels));
    let mut p = ModelPlot::new();
    p.plane_tolerance = 0.0;
    assert_eq!(p.emit(&g, &m), Err(ModelPlotError::PlaneTolerance));
    let mut p = ModelPlot::new();
    p.overlap_color = PlotColour::Named("notacolour".into());
    assert!(matches!(
        p.emit(&g, &m),
        Err(ModelPlotError::UnknownColourName(_))
    ));
    assert_eq!(
        ModelPlot::new().emit(&g, &[]),
        Err(ModelPlotError::MaterialIndex(0))
    );
}

fn run_script(py: &str, script: &Path, png: &Path) {
    let st = std::process::Command::new(py)
        .arg(script)
        .arg(png)
        .status()
        .expect("python runs");
    assert!(st.success(), "{} failed", script.display());
}

/// Returns (differing pixels, total pixels).
fn compare_png(a: &Path, b: &Path) -> (usize, usize) {
    let ia = decode_png(&std::fs::read(a).expect("read a")).expect("decode a");
    let ib = decode_png(&std::fs::read(b).expect("read b")).expect("decode b");
    assert_eq!(
        (ia.width, ia.height),
        (ib.width, ib.height),
        "{}: size differs",
        a.display()
    );
    let diff = ia
        .pixels
        .iter()
        .zip(&ib.pixels)
        .filter(|(p, q)| p != q)
        .count();
    (diff, ia.pixels.len())
}

#[test]
fn scripts_reproduce_openmc_model_plot_pixel_for_pixel() {
    let Ok(py) = std::env::var("OUTRAM_PYTHON") else {
        println!("SKIP: set OUTRAM_PYTHON to a python with numpy + matplotlib to run the image comparison");
        return;
    };
    let out = std::env::var("OUTRAM_PLOT_SCRIPT_OUT").map_or_else(
        |_| std::env::temp_dir().join("outram_python_plot_parity"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("out dir");
    let refdir = vv_dir().join("openmc_reference");
    let mut failures = Vec::new();
    for (name, script) in cases() {
        let sp = out.join(format!("{name}.py"));
        std::fs::write(&sp, &script).expect("write script");
        let png = out.join(format!("{name}.png"));
        run_script(&py, &sp, &png);
        let (d, n) = compare_png(&png, &refdir.join(format!("{name}.png")));
        println!("{name:<32} {d:>6} / {n} pixels differ");
        if d != 0 {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "differing cases: {failures:?}");
}

/// The comparison can fail: the smallest perturbation of each kind must change
/// pixels. Measured counts are recorded in the README.
#[test]
fn harness_detects_small_perturbations() {
    let Ok(py) = std::env::var("OUTRAM_PYTHON") else {
        println!("SKIP: set OUTRAM_PYTHON to run the negative controls");
        return;
    };
    let out = std::env::temp_dir().join("outram_python_plot_parity_neg");
    std::fs::create_dir_all(&out).expect("out dir");
    let refdir = vv_dir().join("openmc_reference");
    let mut controls: Vec<(&str, &str, String)> = Vec::new();

    let (g, m) = pin_lattice();
    let mut p = ModelPlot::new();
    p.basis = PlotBasis::Xz;
    origin_width(&mut p, [0.0, 0.2, 0.0], [6.4, 22.0]);
    p.pixels = Pixels::Exact([100, 300]);
    p.seed = Some(4);
    p.legend = true;
    p.axis_units = AxisUnits::Mm;
    controls.push(("seed 4 instead of 3", "lattice_xz_seed_legend_mm", p.emit(&g, &m).unwrap()));

    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.02, 0.0, 0.0], [6.4, 6.4]);
    p.pixels = Pixels::Exact([120, 120]);
    p.outline = Outline::Only;
    controls.push(("origin +0.02 cm (half a pixel)", "lattice_outline_only", p.emit(&g, &m).unwrap()));

    // Rings rotated by one position: the fill-order defect this harness found.
    let (mut g, m) = hex(HexOrientation::Y, false);
    if let Lattice::Hex(h) = &mut g.lattices[0] {
        let (a, b, c) = (1usize, 2usize, 3usize);
        *h = HexLattice::from_rings(
            5,
            HexOrientation::Y,
            Position::ZERO,
            1.5,
            &[
                vec![a, b, c, a, b, c, a, b, c, a, b, c],
                vec![c, a, b, c, a, b],
                vec![a],
            ],
            Some(a),
        );
    }
    let mut p = ModelPlot::new();
    origin_width(&mut p, [0.0; 3], [9.0, 9.0]);
    p.pixels = Pixels::Exact([150, 150]);
    p.seed = Some(7);
    p.legend = true;
    controls.push(("hex rings rotated one position", "hex_seed_legend", p.emit(&g, &m).unwrap()));

    for (label, case, script) in controls {
        let sp = out.join(format!("{case}.py"));
        std::fs::write(&sp, script).expect("write");
        let png = out.join(format!("{case}.png"));
        run_script(&py, &sp, &png);
        let (d, n) = compare_png(&png, &refdir.join(format!("{case}.png")));
        println!("NEGATIVE CONTROL {label:<34} {d:>6} / {n} pixels differ");
        assert!(d > 0, "{label}: the harness did not detect the perturbation");
    }
}
