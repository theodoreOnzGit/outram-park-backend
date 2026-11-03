// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Tests of `power_shape` (gh:#1184): power conservation for every shape, the
//! exact integrals the ring quadrature promises, the expansion round trips
//! through the Monte Carlo filters' own basis, and the frames.

use super::*;
use outram_mc_libs::tally::filter::FilterEvent;
use uom::si::power::megawatt;

fn l(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

fn map() -> FrameMap {
    // N = 12 Şeker bed: 9.798 * 12 + 6 = 123.576 cm.
    FrameMap::new(l(61.788))
}

/// A 6 x 8 R-Z grid over the full-core bed, in the TECDOC frame (`z` down).
fn tecdoc_bed_rings() -> PowerCells {
    let mut cells = Vec::new();
    for j in 0..8 {
        for i in 0..6 {
            cells.push(CellGeometry::Annulus {
                r: [l(15.0 * i as f64), l(15.0 * (i + 1) as f64)],
                z: [l(22.5 * j as f64), l(22.5 * (j + 1) as f64)],
            });
        }
    }
    PowerCells::new(Frame::Tecdoc, FrameMap::new(l(90.0)), cells)
}

fn all_shapes() -> Vec<(&'static str, PowerShape)> {
    vec![
        ("uniform", PowerShape::Uniform),
        (
            "zones",
            PowerShape::Zones(vec![
                ZoneLump { r: [l(0.0), l(40.0)], z: [l(-90.0), l(10.0)], fraction: 0.7 },
                ZoneLump { r: [l(40.0), l(90.0)], z: [l(-30.0), l(90.0)], fraction: 0.3 },
            ]),
        ),
        ("chopped cosine", PowerShape::ChoppedCosine { centre: l(5.0), extrapolated_height: l(230.0) }),
        ("sine", PowerShape::Sine { bottom: l(-90.0), height: l(180.0) }),
        (
            "J0 cos",
            PowerShape::J0Cosine { centre: l(0.0), extrapolated_radius: l(100.0), extrapolated_height: l(220.0) },
        ),
        ("Legendre-z", PowerShape::LegendreZ { z_min: l(-90.0), z_max: l(90.0), coefficients: vec![1.0, -0.3, -0.2, 0.05] }),
        (
            "Zernike-Legendre",
            PowerShape::ZernikeLegendre {
                centre: [l(10.0), l(-5.0)],
                radius: l(90.0),
                z_min: l(-90.0),
                z_max: l(90.0),
                coefficients: vec![vec![1.0, 0.1, -0.05, -0.2, 0.0, 0.02], vec![-0.2, 0.0, 0.03, 0.0, 0.0, 0.0]],
            },
        ),
        (
            "spherical harmonics",
            PowerShape::SphericalHarmonics {
                centre: [l(0.0), l(0.0), l(0.0)],
                radius: l(85.0),
                coefficients: vec![1.0, 0.0, 0.3, 0.0, 0.0, 0.0, -0.1, 0.0, 0.0],
                radial: RadialProfile::BareSphere,
            },
        ),
        (
            "R-Z table",
            PowerShape::RzTable {
                r_edges: vec![l(0.0), l(33.0), l(70.0), l(90.0)],
                z_edges: vec![l(-90.0), l(-17.0), l(50.0), l(90.0)],
                values: vec![1.0, 0.8, 0.5, 1.4, 1.1, 0.7, 0.9, 0.6, 0.4],
            },
        ),
    ]
}

/// **Power conservation, every shape**: the cells' powers add up to the
/// total to round-off, on rings, on boxes and on points.
#[test]
fn every_shape_conserves_the_total_power() {
    let total = Power::new::<megawatt>(10.0);
    let rings = tecdoc_bed_rings();
    let boxes = PowerCells::new(
        Frame::Mc,
        map(),
        (0..4)
            .map(|k| CellGeometry::Box {
                min: [l(-60.0), l(-60.0), l(-80.0 + 40.0 * k as f64)],
                max: [l(60.0), l(60.0), l(-40.0 + 40.0 * k as f64)],
            })
            .collect(),
    );
    let points = PowerCells::new(
        Frame::Mc,
        map(),
        (0..20)
            .map(|k| CellGeometry::Point {
                centre: [l(3.0 * k as f64), l(1.0), l(-60.0 + 6.0 * k as f64)],
                volume: Volume::new::<cubic_centimeter>(100.0 + k as f64),
            })
            .collect(),
    );
    for (name, s) in all_shapes() {
        for (kind, cells) in [("rings", &rings), ("boxes", &boxes), ("points", &points)] {
            let q = s.cell_power_density(cells, total).unwrap_or_else(|e| panic!("{name} on {kind}: {e}"));
            let sum: f64 = q
                .iter()
                .zip(&cells.cells)
                .map(|(q, c)| q.get::<watt_per_cubic_meter>() * c.volume().get::<uom::si::volume::cubic_meter>())
                .sum();
            assert!((sum / 1.0e7 - 1.0).abs() < 1e-12, "{name} on {kind}: {sum} W");
        }
    }
}

/// The ring quadrature is **exact** where the module says it is: a
/// piecewise-constant table on misaligned rings, and the chopped cosine
/// against its closed-form integral.
#[test]
fn ring_integrals_are_exact() {
    let table = PowerShape::RzTable {
        r_edges: vec![l(0.0), l(33.0), l(90.0)],
        z_edges: vec![l(-50.0), l(20.0), l(50.0)],
        values: vec![2.0, 1.0, 3.0, 0.5],
    };
    // A ring straddling every table edge.
    let i = table.integrate_ring([10.0, 60.0], [-40.0, 30.0]);
    let ring = |a: f64, b: f64| PI * (b * b - a * a);
    let exact = 2.0 * ring(10.0, 33.0) * 60.0
        + 1.0 * ring(33.0, 60.0) * 60.0
        + 3.0 * ring(10.0, 33.0) * 10.0
        + 0.5 * ring(33.0, 60.0) * 10.0;
    assert!((i / exact - 1.0).abs() < 1e-13, "{i} vs {exact}");

    let cosine = PowerShape::ChoppedCosine { centre: l(0.0), extrapolated_height: l(200.0) };
    // int_{-50}^{70} cos(pi z / 200) dz = (200/pi)(sin(0.35 pi) + sin(0.25 pi)).
    let i = cosine.integrate_ring([0.0, 90.0], [-50.0, 70.0]);
    let exact = ring(0.0, 90.0) * 200.0 / PI * ((0.35 * PI).sin() + (0.25 * PI).sin());
    assert!((i / exact - 1.0).abs() < 1e-12, "{i} vs {exact}");

    // A region lump: its fraction of the power lands in its ring exactly.
    let lumps = PowerShape::Zones(vec![ZoneLump { r: [l(20.0), l(50.0)], z: [l(-10.0), l(30.0)], fraction: 0.25 }]);
    assert!((lumps.integrate_ring([0.0, 90.0], [-90.0, 90.0]) - 0.25).abs() < 1e-14);
}

/// Score `q` through a Monte Carlo tally's filter at quadrature points of
/// the cylinder `r < 90`, `|z| < 90` (a deterministic stand-in for the
/// transport's scoring: every point deposits `w q` times the filter's
/// weights), then read the tally back.
fn score_cylinder(tally: &mut Tally, q: impl Fn(f64, f64, f64) -> f64) {
    let (nodes, weights) = gauss_legendre_table(12).unwrap();
    let n_theta = 24;
    let mut acc = vec![0.0; tally.bins.len()];
    for (xr, wr) in nodes.iter().zip(weights) {
        let r = 45.0 + 45.0 * xr;
        for (xz, wz) in nodes.iter().zip(weights) {
            let z = 90.0 * xz;
            for k in 0..n_theta {
                let t = (k as f64 + 0.5) * 2.0 * PI / n_theta as f64;
                let (x, y) = (r * t.cos(), r * t.sin());
                let w = wr * wz * 45.0 * 90.0 * r * 2.0 * PI / n_theta as f64 * q(x, y, z);
                let ev = FilterEvent { position: Position::new(x, y, z), ..Default::default() };
                let f = &tally.filters[0];
                if let Some(m) = f.expansion_moments(&ev) {
                    for (b, p) in m.iter().enumerate() {
                        acc[b] += w * p;
                    }
                } else if let Some(b) = f.get_bin(&ev) {
                    acc[b] += w;
                }
            }
        }
    }
    for (bin, v) in tally.bins.iter_mut().zip(acc) {
        bin.score(v); // one realisation
    }
}

/// **Legendre round trip**: a cubic axial power scored through the
/// `SpatialLegendreFilter`'s own `P_n` and reconstructed by `from_tally`
/// is the cubic, to round-off.
#[test]
fn legendre_round_trip_recovers_a_cubic() {
    let a = [2.0, -0.5, 0.3, 0.1];
    let lambda = |z: f64| {
        let xi = z / 90.0;
        let p = legendre_pn(3, xi);
        a.iter().zip(&p).map(|(a, p)| a * p).sum::<f64>()
    };
    let area = PI * 90.0 * 90.0;
    let mut t = McPowerTally::legendre_z(3, l(-90.0), l(90.0)).unwrap();
    score_cylinder(&mut t, |_, _, z| lambda(z) / area);
    let PowerShape::LegendreZ { coefficients, .. } = PowerShape::from_tally(&t, 1).unwrap() else {
        panic!("not Legendre")
    };
    for (got, want) in coefficients.iter().zip(&a) {
        assert!((got - want).abs() < 1e-12, "{coefficients:?}");
    }
}

/// **Zernike round trip**: an off-axis tilt and a radial bowl, scored
/// through the `ZernikeFilter`, come back with the right coefficients (the
/// disc norms `pi/(n+1)`, `pi/(2(n+1))` applied).
#[test]
fn zernike_round_trip_recovers_the_coefficients() {
    // Order 2: (0,0) (1,-1) (1,1) (2,-2) (2,0) (2,2).
    let c = [1.0, 0.2, -0.1, 0.05, -0.3, 0.07];
    let sigma = |x: f64, y: f64| zernike_eval(&[l(0.0), l(0.0)], l(90.0), &c, x, y);
    let mut t = McPowerTally::zernike(2, [l(0.0), l(0.0)], l(90.0)).unwrap();
    score_cylinder(&mut t, |x, y, _| sigma(x, y) / 180.0);
    let PowerShape::Zernike { coefficients, .. } = PowerShape::from_tally(&t, 1).unwrap() else {
        panic!("not Zernike")
    };
    for (got, want) in coefficients.iter().zip(&c) {
        assert!((got - want).abs() < 1e-12, "{coefficients:?}");
    }
    // And the separable product of the two tallies is the product shape.
    let mut lt = McPowerTally::legendre_z(1, l(-90.0), l(90.0)).unwrap();
    let q = |x: f64, y: f64, z: f64| sigma(x, y) * (1.0 + 0.4 * z / 90.0);
    score_cylinder(&mut lt, q);
    let mut zt = McPowerTally::zernike(2, [l(0.0), l(0.0)], l(90.0)).unwrap();
    score_cylinder(&mut zt, q);
    let s = PowerShape::zernike_legendre_from_tallies(&zt, &lt, 1).unwrap();
    for p in [(10.0, 20.0, -30.0), (-50.0, 5.0, 60.0), (0.0, 0.0, 0.0)] {
        let got = s.eval(p.0, p.1, p.2);
        // sigma_int(x, y) lambda(z) / P is q itself for a separable q.
        let want = q(p.0, p.1, p.2);
        assert!((got / want - 1.0).abs() < 1e-11, "{got} vs {want}");
    }
}

/// **R-Z mesh round trip**: a piecewise-constant density on the mesh's own
/// bins is recovered as a density (mean over bin volume).
#[test]
fn rz_mesh_tally_round_trip() {
    let r = [l(0.0), l(30.0), l(90.0)];
    let z = [l(-90.0), l(0.0), l(90.0)];
    let mut t = McPowerTally::rz_mesh(&r, &z).unwrap();
    let dens = |x: f64, y: f64, z: f64| {
        let rr = (x * x + y * y).sqrt();
        match (rr < 30.0, z < 0.0) {
            (true, true) => 4.0,
            (false, true) => 2.0,
            (true, false) => 3.0,
            (false, false) => 1.0,
        }
    };
    // Exact bin integrals instead of point scoring (the bins are the cells).
    let vol = |a: f64, b: f64| PI * (b * b - a * a) * 90.0;
    for (k, v) in [4.0 * vol(0.0, 30.0), 2.0 * vol(30.0, 90.0), 3.0 * vol(0.0, 30.0), 1.0 * vol(30.0, 90.0)]
        .iter()
        .enumerate()
    {
        t.bins[k].score(*v);
    }
    let s = PowerShape::from_tally(&t, 1).unwrap();
    for p in [(10.0, 0.0, -40.0), (50.0, 20.0, -10.0), (5.0, 5.0, 80.0), (70.0, 0.0, 10.0)] {
        assert!((s.eval(p.0, p.1, p.2) - dens(p.0, p.1, p.2)).abs() < 1e-12);
    }
    assert!(McPowerTally::rz_mesh(&[l(1.0)], &z).is_err());
}

/// **Frames**: MC <-> TECDOC <-> GeN-Foam metres round trip, and the fixed
/// bed floor maps to `z_T = 180 cm` at every loading.
#[test]
fn frames_round_trip() {
    for h in [55.0, 61.788, 90.0] {
        let m = FrameMap::new(l(h));
        let p = [l(12.0), l(-3.0), l(17.5)];
        let back = m.from_mc(Frame::Tecdoc, m.to_mc(Frame::Tecdoc, p));
        for k in 0..3 {
            assert!((cm(back[k]) - cm(p[k])).abs() < 1e-12);
        }
        assert_eq!(cm(m.to_mc(Frame::Mc, p)[2]), 17.5);
        // Bed floor (MC z = -h) is TECDOC z = 180; the bed top (MC z = h) is
        // 180 - 2h.
        assert!((cm(m.z_from_mc(Frame::Tecdoc, l(-h))) - 180.0).abs() < 1e-12);
        assert!((cm(m.z_from_mc(Frame::Tecdoc, l(h))) - (180.0 - 2.0 * h)).abs() < 1e-12);
        let g = m.mc_cm_to_genfoam_m([12.0, -3.0, 17.5]);
        let b = m.genfoam_m_to_mc_cm(g);
        assert!((b[2] - 17.5).abs() < 1e-12 && (b[0] - 12.0).abs() < 1e-12);
        assert!((g[2] * 100.0 - cm(m.z_from_mc(Frame::Tecdoc, l(17.5)))).abs() < 1e-12);
    }
    // Full core: the bed top is TECDOC z = 0.
    assert!(cm(FrameMap::new(l(90.0)).z_from_mc(Frame::Tecdoc, l(90.0))).abs() < 1e-12);
}

/// A shape put on an `htr10_rz` mesh through `PowerCells::from_rz_mesh`
/// lands on the bed cells only and conserves the power; the captured
/// fraction of a support the cells cover is one.
#[test]
fn rz_mesh_cells_receive_the_power() {
    use outram_foam_appbuilder_lib::htr10_rz::{build_mesh, geometry::idx, Htr10RzOptions};
    let mesh = build_mesh(&Htr10RzOptions::default()).unwrap();
    let fm = FrameMap::new(l(90.0));
    let cells = PowerCells::from_rz_mesh(&mesh, fm, |c| mesh.cell_region(c) == idx::BED);
    assert!(!cells.cells.is_empty());
    let shape = PowerShape::J0Cosine { centre: l(0.0), extrapolated_radius: l(100.0), extrapolated_height: l(220.0) };
    let q = shape.cell_power_density(&cells, Power::new::<megawatt>(10.0)).unwrap();
    let sum: f64 = q
        .iter()
        .zip(&cells.cells)
        .map(|(q, c)| q.get::<watt_per_cubic_meter>() * c.volume().get::<uom::si::volume::cubic_meter>())
        .sum();
    assert!((sum / 1e7 - 1.0).abs() < 1e-12);
    let f = PowerShape::Uniform.captured_fraction(&cells, l(90.0), [l(-90.0), l(90.0)]);
    assert!(f > 1.0, "the cone and tube add bed volume below the cylinder: {f}");
    let cyl = PowerShape::Sine { bottom: l(-90.0), height: l(180.0) };
    assert!((cyl.captured_fraction(&cells, l(90.0), [l(-90.0), l(90.0)]) - 1.0).abs() < 1e-10);
}

/// The PNG writers produce a file.
#[test]
fn shapes_draw() {
    let dir = std::env::temp_dir().join("nee_soon_power_shape_test");
    std::fs::create_dir_all(&dir).unwrap();
    let s = &all_shapes()[6].1;
    s.write_rz_png(dir.join("rz.png"), "ZERNIKE-LEGENDRE", l(90.0), [l(-90.0), l(90.0)], 1.0).unwrap();
    s.write_xy_png(dir.join("xy.png"), "ZERNIKE-LEGENDRE XY", l(0.0), l(90.0), 1.0).unwrap();
    assert!(std::fs::metadata(dir.join("rz.png")).unwrap().len() > 100);
    assert!(!s.is_axisymmetric());
    assert!(PowerShape::Uniform.is_axisymmetric());
}
