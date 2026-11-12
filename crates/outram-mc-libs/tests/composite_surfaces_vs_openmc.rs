// SPDX-License-Identifier: AGPL-3.0-only

//! **V&V gate: OpenMC's composite surfaces (gh:#1389) against OpenMC's own
//! Python, against analytic volumes, and drawn.**
//!
//! # Methodology
//!
//! `geometry::composite` ports `openmc/model/surface_composite.py` (OpenMC
//! `8f79d9d0`): `CylinderSector`, `IsogonalOctagon`, `X/Y/ZConeOneSided`,
//! `Polygon`, `CruciformPrism`, `RectangularPrism` (rounded corners),
//! `ConicalFrustum` and `Vessel`. 27 cases cover every composite, every axis,
//! a reflex sector, square and rounded prisms, increasing and decreasing
//! cruciform distances, an oblique, an anti-parallel (cylinder) and two
//! axis-aligned frusta, and concave polygons in the `xy`, `yz`, `xz` and `rz`
//! bases (one given clockwise).
//!
//! 1. **Reference.**
//!    `verification_and_validation/composite_surfaces/openmc_inputs/make_reference.py`
//!    builds each case with OpenMC's Python API (imported from the upstream
//!    clone; numpy 2.4.6, scipy 1.17.1, matplotlib 3.11.2, CPython 3.11) and
//!    writes `reference.txt`: each case's box, a seed, the component
//!    surfaces' coefficients in `_surface_names` order (general `Cone` and
//!    `Cylinder` as the `Quadric` coefficients `to_xml_element` hands the C++
//!    code), and, for 2000 splitmix64 points in the box, `point in
//!    -composite` (`1`/`0`), or `x` for a point within 1e-9 of any surface
//!    the region uses. The script also checks `+composite` is the complement
//!    of `-composite` on every kept point.
//! 2. **Membership** (`membership_matches_openmc_python`): this test
//!    regenerates the same points (same integer generator, same double
//!    operations) and classifies them with `CompositeRegion::contains`, i.e.
//!    the transport's own `Cell::contains`, for both half-spaces. Pass: zero
//!    disagreements, and `+` the complement of `-`.
//! 3. **Coefficients** (`surface_coefficients_match_openmc_python`): every
//!    non-polygon component surface against the reference: same kind, same
//!    count, coefficients bit-identical. Polygon is excluded by design: its
//!    triangulation is ear clipping, not qhull, so its internal facets differ
//!    (see `geometry::composite::Polygon`); only its point set is compared.
//! 4. **Stochastic volume** (`stochastic_volume_matches_analytic`): the
//!    existing port of OpenMC's `VolumeCalculation`
//!    (`geometry::volume_calc`, material domains, 10^6 samples, master seed
//!    1389) on the two-cell model (`CompositeRegion::two_cell_geometry`),
//!    against the analytic volume of the composite inside the box (area times
//!    slab length for the infinite prisms; `pi r2 H^3 / 3` for a cone nap of
//!    height `H`; the frustum formula; cylinder plus two half-ellipsoids for
//!    the vessel; Pappus for the `rz` polygon). Pass: `|z| <= 4` per case,
//!    and the two cells together fill the box (every sample located once).
//! 5. **Drawing** (`draw_every_composite`): each case is drawn with the ported
//!    plotter (`render_material_slice`) from the two-cell model, one slice
//!    across the composite's axis and one along it, through the centroid of
//!    the inside sample points. The test checks both materials appear in
//!    every slice; `OUTRAM_PLOT_OUT=<dir>` keeps the PNGs (how
//!    `verification_and_validation/composite_surfaces/images/` was made).
//!
//! # Results (2026-11-12, x86_64 Linux, OpenMC Python `8f79d9d0`)
//!
//! Predictions were posted on gh:#1389 before the first run.
//!
//! - **Membership: 0 of 54 000 points disagree** (27 cases x 2000, none
//!   dropped as near-surface). Predicted 0, Polygon included.
//! - **Coefficients: 366 of 366 bit-identical** (4 are `+0.0` against `-0.0`).
//!   With a left-to-right `dot` it was 365: `sector_x`'s `plane2.d` was 1 ulp
//!   off (`0.6920903430595147` against `...148`), because numpy's `np.dot`
//!   (OpenBLAS `ddot`) is an FMA chain; measured, see `composite::dot`. The
//!   predicted few-ulp `ConicalFrustum` difference did not occur.
//! - **Volume: every case within 1.5 sigma** (largest `|z|` 1.50,
//!   `sector_z_reflex`; `chi^2` 14.85 over 27), every sample in exactly one
//!   cell. Per-case table:
//!   `verification_and_validation/composite_surfaces/README.md`.
//! - **Drawing:** 54 slices, both materials in each.
//!
//! **What this means:** the expansions are upstream's, surface for surface,
//! and enclose the volumes the geometry says. Not covered: transport
//! distances on the composite regions, points within 1e-9 of a surface, and
//! whether Polygon's convex pieces match upstream's (they need not).

use std::f64::consts::PI;
use std::path::PathBuf;

use outram_mc_libs::geometry::cell::HalfSpaceSense::{Inside, Outside};
use outram_mc_libs::geometry::composite::{
    Axis, CompositeRegion, ConeOneSided, ConicalFrustum, CruciformPrism, CylinderSector,
    IsogonalOctagon, Polygon, PolygonBasis, RectangularPrism, Vessel,
};
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::geometry::volume_calc::{BoundingBox, VolumeCalculation, VolumeDomain};

const ARROW: [[f64; 2]; 8] = [
    [0.0, 0.0],
    [3.0, 0.0],
    [3.0, -1.0],
    [5.0, 1.5],
    [3.0, 4.0],
    [3.0, 3.0],
    [0.0, 3.0],
    [1.0, 1.5],
];
const L_CW: [[f64; 2]; 6] = [
    [0.0, 0.0],
    [0.0, 2.0],
    [1.0, 2.0],
    [1.0, 1.0],
    [2.0, 1.0],
    [2.0, 0.0],
];
const RZ: [[f64; 2]; 7] = [
    [0.5, 0.0],
    [3.0, 0.0],
    [3.0, 1.0],
    [2.0, 1.5],
    [3.0, 3.0],
    [1.0, 2.5],
    [0.5, 3.0],
];

/// One case: the composite, its box, the axis it is drawn about, and its
/// analytic volume inside the box \[cm^3\].
struct Case {
    name: &'static str,
    region: CompositeRegion,
    lo: [f64; 3],
    hi: [f64; 3],
    axis: Axis,
    volume: f64,
    polygon: bool,
}

fn shoelace(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Pappus: volume of `p` (in `(r, z)`) revolved about z, `2 pi int r dA`.
fn pappus(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    let s: f64 = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            (a[0] + b[0]) * (a[0] * b[1] - b[0] * a[1])
        })
        .sum();
    2.0 * PI * (s / 6.0).abs()
}

fn octagon_area(r1: f64, r2: f64) -> f64 {
    let l = r2 * 2.0_f64.sqrt() - r1;
    4.0 * r1 * r1 - 2.0 * (r1 - l) * (r1 - l)
}

fn cruciform_area(d: &[f64]) -> f64 {
    let n = d.len();
    let rects: f64 = (0..n).map(|i| 4.0 * d[i] * d[n - 1 - i]).sum();
    let overlaps: f64 = (0..n - 1)
        .map(|i| 4.0 * d[i].min(d[i + 1]) * d[n - 1 - i].min(d[n - 2 - i]))
        .sum();
    rects - overlaps
}

fn frustum_volume(h: f64, r1: f64, r2: f64) -> f64 {
    PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2)
}

fn vessel_volume(r: f64, p1: f64, p2: f64, h1: f64, h2: f64) -> f64 {
    PI * r * r * (p2 - p1) + 2.0 / 3.0 * PI * r * r * (h1 + h2)
}

#[allow(clippy::too_many_lines)]
fn cases() -> Vec<Case> {
    let c = |name, region: &CompositeRegion, lo, hi, axis, volume| Case {
        name,
        region: region.clone(),
        lo,
        hi,
        axis,
        volume,
        polygon: false,
    };
    let sector = |deg: f64| deg / 360.0 * PI * (9.0 - 1.0) * 2.0;
    let mut v = vec![
        c(
            "sector_z",
            CylinderSector::new(1.0, 3.0, 10.0, 80.0, [0.5, -0.5], Axis::Z)
                .unwrap()
                .region(),
            [-3.0, -4.0, -1.0],
            [4.0, 3.0, 1.0],
            Axis::Z,
            sector(70.0),
        ),
        c(
            "sector_z_reflex",
            CylinderSector::new(1.0, 3.0, -30.0, 250.0, [0.0, 0.0], Axis::Z)
                .unwrap()
                .region(),
            [-3.5, -3.5, -1.0],
            [3.5, 3.5, 1.0],
            Axis::Z,
            sector(280.0),
        ),
        c(
            "sector_y",
            CylinderSector::new(1.0, 3.0, 0.0, 90.0, [0.2, 0.3], Axis::Y)
                .unwrap()
                .region(),
            [-3.0, -1.0, -3.0],
            [3.5, 1.0, 3.5],
            Axis::Y,
            sector(90.0),
        ),
        c(
            "sector_x",
            CylinderSector::new(1.0, 3.0, 20.0, 130.0, [0.2, 0.3], Axis::X)
                .unwrap()
                .region(),
            [-1.0, -3.0, -3.0],
            [1.0, 3.5, 3.5],
            Axis::X,
            sector(110.0),
        ),
        c(
            "octagon_z",
            IsogonalOctagon::new([0.5, -0.25], 2.0, 2.2, Axis::Z)
                .unwrap()
                .region(),
            [-2.0, -2.5, -1.0],
            [3.0, 2.5, 1.0],
            Axis::Z,
            2.0 * octagon_area(2.0, 2.2),
        ),
        c(
            "octagon_y",
            IsogonalOctagon::new([0.5, -0.25], 2.0, 2.2, Axis::Y)
                .unwrap()
                .region(),
            [-2.0, -1.0, -2.5],
            [3.0, 1.0, 2.5],
            Axis::Y,
            2.0 * octagon_area(2.0, 2.2),
        ),
        c(
            "octagon_x",
            IsogonalOctagon::new([0.5, -0.25], 2.0, 2.2, Axis::X)
                .unwrap()
                .region(),
            [-1.0, -2.0, -2.5],
            [1.0, 3.0, 2.5],
            Axis::X,
            2.0 * octagon_area(2.0, 2.2),
        ),
        c(
            "cone_x_up",
            ConeOneSided::new(Axis::X, [0.5, 0.0, 0.0], 0.25, true)
                .unwrap()
                .region(),
            [-1.5, -1.5, -1.5],
            [2.5, 1.5, 1.5],
            Axis::X,
            PI * 0.25 * 8.0 / 3.0,
        ),
        c(
            "cone_y_down",
            ConeOneSided::new(Axis::Y, [0.0, 0.5, 0.2], 0.36, false)
                .unwrap()
                .region(),
            [-1.5, -1.5, -1.3],
            [1.5, 2.5, 1.7],
            Axis::Y,
            PI * 0.36 * 8.0 / 3.0,
        ),
        c(
            "cone_z_up",
            ConeOneSided::new(Axis::Z, [0.3, -0.2, -1.0], 1.0, true)
                .unwrap()
                .region(),
            [-2.0, -2.5, -2.0],
            [2.6, 2.1, 1.0],
            Axis::Z,
            PI * 8.0 / 3.0,
        ),
        c(
            "rect_z_round",
            RectangularPrism::new(4.0, 3.0, Axis::Z, [0.5, -0.5], 0.5)
                .unwrap()
                .region(),
            [-2.0, -2.5, -1.0],
            [3.0, 1.5, 1.0],
            Axis::Z,
            2.0 * (12.0 - (4.0 - PI) * 0.25),
        ),
        c(
            "rect_y_round",
            RectangularPrism::new(4.0, 3.0, Axis::Y, [0.5, -0.5], 0.7)
                .unwrap()
                .region(),
            [-2.0, -1.0, -2.5],
            [3.0, 1.0, 1.5],
            Axis::Y,
            2.0 * (12.0 - (4.0 - PI) * 0.49),
        ),
        c(
            "rect_x_round",
            RectangularPrism::new(2.0, 5.0, Axis::X, [0.0, 1.0], 0.9)
                .unwrap()
                .region(),
            [-1.0, -1.5, -2.0],
            [1.0, 1.5, 4.0],
            Axis::X,
            2.0 * (10.0 - (4.0 - PI) * 0.81),
        ),
        c(
            "rect_z_square",
            RectangularPrism::new(4.0, 3.0, Axis::Z, [0.5, -0.5], 0.0)
                .unwrap()
                .region(),
            [-2.0, -2.5, -1.0],
            [3.0, 1.5, 1.0],
            Axis::Z,
            2.0 * 12.0,
        ),
        c(
            "cruciform_z_inc",
            CruciformPrism::new(&[1.0, 2.0, 3.0], [0.5, 0.5], Axis::Z)
                .unwrap()
                .region(),
            [-3.0, -3.0, -1.0],
            [4.0, 4.0, 1.0],
            Axis::Z,
            2.0 * cruciform_area(&[1.0, 2.0, 3.0]),
        ),
        c(
            "cruciform_x_dec",
            CruciformPrism::new(&[2.5, 1.5, 0.5], [0.0, 0.0], Axis::X)
                .unwrap()
                .region(),
            [-1.0, -3.0, -3.0],
            [1.0, 3.0, 3.0],
            Axis::X,
            2.0 * cruciform_area(&[2.5, 1.5, 0.5]),
        ),
        c(
            "frustum_oblique",
            ConicalFrustum::new([1.0, 2.0, 3.0], [1.0, 1.0, 2.0], 2.0, 1.0)
                .unwrap()
                .region(),
            [-1.0, 0.0, 1.5],
            [3.5, 4.5, 6.5],
            Axis::Z,
            frustum_volume(6.0_f64.sqrt(), 2.0, 1.0),
        ),
        c(
            "frustum_cylinder_down",
            ConicalFrustum::new([0.0, 0.0, 1.0], [0.0, 0.0, -2.0], 1.5, 1.5)
                .unwrap()
                .region(),
            [-2.0, -2.0, -1.5],
            [2.0, 2.0, 1.5],
            Axis::Z,
            frustum_volume(2.0, 1.5, 1.5),
        ),
        c(
            "frustum_z_expand",
            ConicalFrustum::new([0.5, 0.0, 0.0], [0.0, 0.0, 3.0], 1.0, 2.0)
                .unwrap()
                .region(),
            [-2.0, -2.5, -0.5],
            [3.0, 2.5, 3.5],
            Axis::Z,
            frustum_volume(3.0, 1.0, 2.0),
        ),
        c(
            "frustum_x",
            ConicalFrustum::new([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], 0.5, 1.5)
                .unwrap()
                .region(),
            [-0.5, -2.0, -2.0],
            [2.5, 2.0, 2.0],
            Axis::X,
            frustum_volume(2.0, 0.5, 1.5),
        ),
        c(
            "vessel_z",
            Vessel::new(2.0, -1.0, 3.0, 1.0, 1.5, [0.5, 0.5], Axis::Z)
                .unwrap()
                .region(),
            [-2.0, -2.0, -2.5],
            [3.0, 3.0, 5.0],
            Axis::Z,
            vessel_volume(2.0, -1.0, 3.0, 1.0, 1.5),
        ),
        c(
            "vessel_x",
            Vessel::new(1.5, 0.0, 2.0, 0.5, 2.0, [0.0, -0.5], Axis::X)
                .unwrap()
                .region(),
            [-1.0, -2.5, -2.0],
            [4.5, 1.5, 2.0],
            Axis::X,
            vessel_volume(1.5, 0.0, 2.0, 0.5, 2.0),
        ),
        c(
            "vessel_y",
            Vessel::new(1.0, -2.0, 2.0, 1.0, 1.0, [1.0, 0.0], Axis::Y)
                .unwrap()
                .region(),
            [-0.5, -3.5, -1.5],
            [2.5, 3.5, 1.5],
            Axis::Y,
            vessel_volume(1.0, -2.0, 2.0, 1.0, 1.0),
        ),
    ];
    let mut poly = |name, pts: &[[f64; 2]], basis, lo, hi, axis, volume| {
        let mut case = c(
            name,
            Polygon::new(pts, basis).unwrap().region(),
            lo,
            hi,
            axis,
            volume,
        );
        case.polygon = true;
        v.push(case);
    };
    poly(
        "polygon_xy_arrow",
        &ARROW,
        PolygonBasis::Xy,
        [-0.5, -1.5, -1.0],
        [5.5, 4.5, 1.0],
        Axis::Z,
        2.0 * shoelace(&ARROW),
    );
    poly(
        "polygon_yz_arrow",
        &ARROW,
        PolygonBasis::Yz,
        [-1.0, -0.5, -1.5],
        [1.0, 5.5, 4.5],
        Axis::X,
        2.0 * shoelace(&ARROW),
    );
    poly(
        "polygon_xz_l_cw",
        &L_CW,
        PolygonBasis::Xz,
        [-0.5, -1.0, -0.5],
        [2.5, 1.0, 2.5],
        Axis::Y,
        2.0 * shoelace(&L_CW),
    );
    poly(
        "polygon_rz",
        &RZ,
        PolygonBasis::Rz,
        [-3.5, -3.5, -0.5],
        [3.5, 3.5, 3.5],
        Axis::Z,
        pappus(&RZ),
    );
    v
}

// ── The reference file ──────────────────────────────────────────────────────

struct RefCase {
    name: String,
    lo: [f64; 3],
    hi: [f64; 3],
    seed: u64,
    n: usize,
    surfaces: Vec<(String, Vec<f64>)>,
    inside: Vec<u8>,
}

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/composite_surfaces")
}

fn reference() -> Vec<RefCase> {
    let text = std::fs::read_to_string(vv_dir().join("reference.txt")).expect("reference.txt");
    let mut out: Vec<RefCase> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |s: &str| s.parse::<f64>().expect("number");
        match f[0] {
            "case" => out.push(RefCase {
                name: f[1].to_string(),
                lo: [num(f[2]), num(f[3]), num(f[4])],
                hi: [num(f[5]), num(f[6]), num(f[7])],
                seed: f[8].parse().expect("seed"),
                n: f[9].parse().expect("n"),
                surfaces: Vec::new(),
                inside: Vec::new(),
            }),
            "surface" => {
                let last = out.last_mut().expect("case before surface");
                last.surfaces
                    .push((f[1].to_string(), f[2..].iter().map(|s| num(s)).collect()));
            }
            "inside" => {
                out.last_mut().expect("case before inside").inside = f[1].as_bytes().to_vec()
            }
            other => panic!("unknown line kind {other}"),
        }
    }
    out
}

/// splitmix64, as `make_reference.py` writes it.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn points(seed: u64, lo: [f64; 3], hi: [f64; 3], n: usize) -> Vec<Position> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            let mut p = [0.0; 3];
            for k in 0..3 {
                let u = (splitmix64(&mut state) >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0);
                p[k] = lo[k] + (hi[k] - lo[k]) * u;
            }
            Position::new(p[0], p[1], p[2])
        })
        .collect()
}

fn paired() -> Vec<(Case, RefCase)> {
    let refs = reference();
    let cases = cases();
    assert_eq!(refs.len(), cases.len(), "case count");
    cases
        .into_iter()
        .zip(refs)
        .map(|(c, r)| {
            assert_eq!(c.name, r.name);
            assert_eq!((c.lo, c.hi), (r.lo, r.hi), "{}: box", c.name);
            (c, r)
        })
        .collect()
}

#[test]
fn membership_matches_openmc_python() {
    let mut total = 0;
    let mut compared = 0;
    let mut bad = Vec::new();
    for (case, r) in paired() {
        let pts = points(r.seed, r.lo, r.hi, r.n);
        assert_eq!(r.inside.len(), r.n);
        let mut n_in = 0;
        for (p, &want) in pts.iter().zip(&r.inside) {
            total += 1;
            if want == b'x' {
                continue;
            }
            compared += 1;
            let inside = case.region.contains(*p, Inside);
            let outside = case.region.contains(*p, Outside);
            assert_ne!(
                inside, outside,
                "{}: + is not the complement of - at {p:?}",
                case.name
            );
            n_in += usize::from(inside);
            if inside != (want == b'1') {
                bad.push(format!(
                    "{} at {p:?}: ours {inside}, OpenMC {}",
                    case.name,
                    want == b'1'
                ));
            }
        }
        println!("{:<22} {:>5} inside of {}", case.name, n_in, r.n);
    }
    println!(
        "compared {compared} of {total} points, {} disagree",
        bad.len()
    );
    assert!(
        bad.is_empty(),
        "{} disagreements:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// `(kind, coefficients)` as `make_reference.py` writes them.
fn coefficients(s: &SurfaceKind) -> (&'static str, Vec<f64>) {
    match s {
        SurfaceKind::XPlane(p) => ("xplane", vec![p.x0]),
        SurfaceKind::YPlane(p) => ("yplane", vec![p.y0]),
        SurfaceKind::ZPlane(p) => ("zplane", vec![p.z0]),
        SurfaceKind::Plane(p) => ("plane", vec![p.a, p.b, p.c, p.d]),
        SurfaceKind::XCylinder(c) => ("xcylinder", vec![c.y0, c.z0, c.r]),
        SurfaceKind::YCylinder(c) => ("ycylinder", vec![c.x0, c.z0, c.r]),
        SurfaceKind::ZCylinder(c) => ("zcylinder", vec![c.x0, c.y0, c.r]),
        SurfaceKind::XCone(c) => ("xcone", vec![c.x0, c.y0, c.z0, c.r_sq]),
        SurfaceKind::YCone(c) => ("ycone", vec![c.x0, c.y0, c.z0, c.r_sq]),
        SurfaceKind::ZCone(c) => ("zcone", vec![c.x0, c.y0, c.z0, c.r_sq]),
        SurfaceKind::Quadric(q) => (
            "quadric",
            vec![q.a, q.b, q.c, q.d, q.e, q.f, q.g, q.h, q.j, q.k],
        ),
        other => panic!("composites make no {other:?}"),
    }
}

#[test]
fn surface_coefficients_match_openmc_python() {
    let mut n_coeff = 0;
    let mut n_identical = 0;
    let mut n_signed_zero = 0;
    let mut worst = (0.0_f64, String::new());
    for (case, r) in paired() {
        if case.polygon {
            continue;
        }
        assert_eq!(
            case.region.surfaces.len(),
            r.surfaces.len(),
            "{}: surface count",
            case.name
        );
        for (k, (s, (kind, want))) in case.region.surfaces.iter().zip(&r.surfaces).enumerate() {
            let (our_kind, ours) = coefficients(s);
            assert_eq!(our_kind, kind, "{} surface {k}: kind", case.name);
            assert_eq!(ours.len(), want.len());
            let scale = want
                .iter()
                .fold(0.0_f64, |m, v| m.max(v.abs()))
                .max(f64::MIN_POSITIVE);
            for (i, (a, b)) in ours.iter().zip(want).enumerate() {
                n_coeff += 1;
                if a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0) {
                    n_identical += 1;
                    n_signed_zero += usize::from(a.to_bits() != b.to_bits());
                    continue;
                }
                let rel = (a - b).abs() / scale;
                println!("{} surface {k} ({kind}) coeff {i}: ours {a:e} OpenMC {b:e}, {rel:.2e} of scale", case.name);
                if rel > worst.0 {
                    worst = (rel, format!("{} surface {k} coeff {i}", case.name));
                }
            }
        }
    }
    println!(
        "{n_identical} of {n_coeff} coefficients bit-identical ({n_signed_zero} of them +0 against -0); worst {:.2e} ({})",
        worst.0, worst.1
    );
    assert_eq!(n_identical, n_coeff, "worst {:.2e} at {}", worst.0, worst.1);
}

#[test]
fn stochastic_volume_matches_analytic() {
    let mut zs = Vec::new();
    for (k, case) in cases().into_iter().enumerate() {
        let geom = case.region.two_cell_geometry();
        let calc = VolumeCalculation {
            domain: VolumeDomain::Material,
            ids: vec![0, 1],
            box_: BoundingBox {
                lower: Position::new(case.lo[0], case.lo[1], case.lo[2]),
                upper: Position::new(case.hi[0], case.hi[1], case.hi[2]),
            },
            n_samples: 1_000_000,
            master_seed: 1389 + k as i64,
        };
        let (res, located) = calc.execute(&geom);
        assert_eq!(
            located, 1.0,
            "{}: a sample fell in neither half-space",
            case.name
        );
        assert_eq!(
            res[0].hits + res[1].hits,
            calc.n_samples,
            "{}: overlap",
            case.name
        );
        let z = (res[0].volume - case.volume) / res[0].std_dev;
        println!(
            "{:<22} V = {:>9.5} +- {:.5}  analytic {:>9.5}  z = {:+.2}",
            case.name, res[0].volume, res[0].std_dev, case.volume, z
        );
        zs.push((case.name, z));
    }
    let chi2: f64 = zs.iter().map(|(_, z)| z * z).sum();
    println!("chi2 = {chi2:.2} over {} cases", zs.len());
    for (name, z) in &zs {
        assert!(z.abs() <= 4.0, "{name}: z = {z:+.2}");
    }
}

#[test]
fn draw_every_composite() {
    let out = std::env::var_os("OUTRAM_PLOT_OUT").map(PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).expect("create plot dir");
    }
    let palette = [
        (Rgb::new(200, 90, 40), "inside (-composite)"),
        (Rgb::new(70, 130, 200), "outside (+composite)"),
    ];
    for case in cases() {
        let geom = case.region.two_cell_geometry();
        // Slice through the centroid of the inside sample points.
        let pts = points(7, case.lo, case.hi, 4000);
        let inside: Vec<&Position> = pts
            .iter()
            .filter(|p| case.region.contains(**p, Inside))
            .collect();
        let n = inside.len() as f64;
        let centroid = Position::new(
            inside.iter().map(|p| p.x).sum::<f64>() / n,
            inside.iter().map(|p| p.y).sum::<f64>() / n,
            inside.iter().map(|p| p.z).sum::<f64>() / n,
        );
        let (across, along) = match case.axis {
            Axis::Z => (PlotBasis::Xy, PlotBasis::Xz),
            Axis::Y => (PlotBasis::Xz, PlotBasis::Xy),
            Axis::X => (PlotBasis::Yz, PlotBasis::Xy),
        };
        for (tag, basis) in [("across", across), ("along", along)] {
            let (h, v) = match basis {
                PlotBasis::Xy => (0, 1),
                PlotBasis::Xz => (0, 2),
                PlotBasis::Yz => (1, 2),
            };
            let width = [case.hi[h] - case.lo[h], case.hi[v] - case.lo[v]];
            let mut origin = [centroid.x, centroid.y, centroid.z];
            origin[h] = 0.5 * (case.lo[h] + case.hi[h]);
            origin[v] = 0.5 * (case.lo[v] + case.hi[v]);
            let px = 240;
            let pixels = [px, ((px as f64) * width[1] / width[0]).round() as usize];
            let plot = SlicePlot::new(
                basis,
                Position::new(origin[0], origin[1], origin[2]),
                width,
                pixels,
            );
            let title = format!("{} ({tag} the axis)", case.name);
            let (raw, img) = render_material_slice(&geom, &plot, &palette, &title);
            let inside_px = raw.pixels.iter().filter(|c| **c == palette[0].0).count();
            let outside_px = raw.pixels.iter().filter(|c| **c == palette[1].0).count();
            assert!(
                inside_px > 0 && outside_px > 0,
                "{title}: in {inside_px}, out {outside_px}"
            );
            if let Some(dir) = &out {
                img.write_png(dir.join(format!("{}_{tag}.png", case.name)))
                    .expect("write png");
            }
        }
    }
}
