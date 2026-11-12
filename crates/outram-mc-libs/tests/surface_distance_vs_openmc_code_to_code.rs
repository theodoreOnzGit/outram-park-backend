// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! Surface `evaluate` / `distance` / `sense` of every CSG surface kind against
//! OpenMC's `src/surface.cpp`, compiled and run (gh:#1386 prio-1 row 2, epic
//! gh:#1336). Research, education and V&V only.
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_surfaces/run.sh` takes the
//! `evaluate`, `distance`, `normal` and `Surface::sense` bodies from OpenMC
//! `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (MIT) **verbatim** (line ranges
//! checked against the SHA-256 of `surface.cpp` by `extract.sh`), stubs only
//! the class declarations (S2 in the V&V README), compiles the real
//! `position.cpp` and `external/quartic_solver.cpp` unmodified, and builds with
//! `g++ -O2 -ffp-contract=off`. The driver evaluates all 15 surface kinds (X/Y/Z
//! planes, general plane, X/Y/Z cylinders, sphere, X/Y/Z cones, general quadric,
//! X/Y/Z tori; 1 to 3 parameter sets each, 33 surfaces) on a fixed list of
//! 20 800-odd rays: 14 generic origins (inside, outside, far, random) x 14
//! directions (axis-aligned, diagonal, near-axis, random), plus, for 3 to 6
//! points known to lie exactly on each surface, the same 14 directions and the
//! outward normal, inward normal and two tangent (grazing) directions, each with
//! `coincident = false` **and** `true`. Every input and output is printed as hex
//! bits.
//!
//! The Rust side rebuilds each surface from the printed parameters, runs the same
//! rays through `SurfaceKind::{evaluate, distance, sense}` and compares bit by bit.
//! One test per surface kind. Upstream's "no hit" is `DBL_MAX`, the port's is
//! `f64::INFINITY`: those two are counted as agreeing misses and reported
//! separately (they are a representation difference, not a numerical one).
//!
//! Torus parameters: OpenMC's `b` is the half-axis ALONG the axis of revolution
//! and `c` the tube radius. The port had them the other way round (#701); the
//! tests hand the port OpenMC's `(a, b, c)` as written.
//!
//! # Prediction (written before the first run)
//!
//! The port was translated by reading, not by line-by-line transcription, so the
//! claim "bit-identical unless operation order or FMA differs" was expected to
//! hold for `evaluate` (same expressions) and for the cylinders (same algebra),
//! and to **fail** where the port uses a different algorithm or threshold:
//! axis-aligned planes and the general plane (`1e-14` direction and `d >` hint
//! thresholds against upstream's `|f| < 1e-12` coincidence test), the sphere (the
//! port forces `c = 0` when coincident and has a `1e-10` floor), the cones and the
//! quadric (the port uses a generic `a d^2 + b d + c` solver with an epsilon floor
//! where upstream has the `k = b/2` form with explicit coincident branches), and
//! the tori (derivative-bracketing bisection against upstream's Orellana-De Michele
//! quartic solver: roots agree to round-off at best, not bits).
//!
//! # Results
//!
//! See the V&V README (`upstream_openmc_surfaces/README.md`) for the table of
//! measured counts, the cause of every difference and the issues filed.

use outram_mc_libs::geometry::surface::{
    BoundaryType, Plane, Quadric, Sphere, SurfaceKind, XCone, XCylinder, XPlane, XTorus, YCone,
    YCylinder, YPlane, YTorus, ZCone, ZCylinder, ZPlane, ZTorus,
};
use outram_mc_libs::geometry::position::{Direction, Position};
use std::collections::BTreeMap;

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_surfaces/reference.txt");

/// OpenMC `INFTY` (`include/openmc/constants.h:103`).
const OPENMC_INFTY: f64 = f64::MAX;
const BC: BoundaryType = BoundaryType::Transmissive;

fn hex(tok: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(tok, 16).unwrap_or_else(|_| panic!("bad hex {tok}")))
}

/// Surface parameters of one reference surface, by instance number.
fn surfaces(kind: &str) -> BTreeMap<usize, Vec<f64>> {
    let mut m = BTreeMap::new();
    for line in REFERENCE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first() == Some(&"S") && t[1] == kind {
            let inst: usize = t[2].parse().unwrap();
            let n: usize = t[3].parse().unwrap();
            m.insert(inst, t[4..4 + n].iter().map(|x| hex(x)).collect());
        }
    }
    m
}

#[derive(Default, Debug)]
struct Report {
    total: usize,
    eval_identical: usize,
    dist_identical: usize,
    /// upstream DBL_MAX vs port INFINITY: both "no hit".
    dist_both_miss: usize,
    sense_identical: usize,
    /// Largest relative difference over finite distance pairs that differ.
    worst_rel: f64,
    /// Hit versus miss disagreements.
    hit_vs_miss: usize,
    first_bad: Vec<String>,
}

impl Report {
    fn dist_agree(&self) -> usize {
        self.dist_identical + self.dist_both_miss
    }
    fn summary(&self, kind: &str) -> String {
        format!(
            "{kind}: {} cases; evaluate bit-identical {}; distance bit-identical {} (+{} both-miss) ; \
             sense identical {}; hit-vs-miss {}; worst finite rel diff {:.3e}",
            self.total,
            self.eval_identical,
            self.dist_identical,
            self.dist_both_miss,
            self.sense_identical,
            self.hit_vs_miss,
            self.worst_rel
        )
    }
    fn assert_all_identical(&self, kind: &str) {
        println!("{}", self.summary(kind));
        for l in &self.first_bad {
            println!("  e.g. {l}");
        }
        assert!(self.total > 0, "no reference cases for {kind}");
        assert_eq!(self.eval_identical, self.total, "{kind}: evaluate");
        assert_eq!(self.dist_agree(), self.total, "{kind}: distance");
        assert_eq!(self.sense_identical, self.total, "{kind}: sense");
    }
}

/// Run every reference case of `kind` through the port.
fn compare(kind: &str, build: impl Fn(&[f64]) -> SurfaceKind) -> Report {
    let surf = surfaces(kind);
    let built: BTreeMap<usize, SurfaceKind> = surf.iter().map(|(k, p)| (*k, build(p))).collect();
    let mut rep = Report::default();
    for line in REFERENCE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first() != Some(&"C") || t[1] != kind {
            continue;
        }
        let inst: usize = t[2].parse().unwrap();
        let r = Position::new(hex(t[3]), hex(t[4]), hex(t[5]));
        let u = Direction::new(hex(t[6]), hex(t[7]), hex(t[8]));
        let coinc = t[9] == "1";
        let e_up = hex(t[10]);
        let d_up = hex(t[11]);
        let s_up = t[12] == "1";
        let s = &built[&inst];
        let e = s.evaluate(r);
        let d = s.distance(r, u, coinc);
        let sn = s.sense(r, u);
        rep.total += 1;
        let mut ok = true;
        if e.to_bits() == e_up.to_bits() {
            rep.eval_identical += 1;
        } else {
            ok = false;
        }
        if d.to_bits() == d_up.to_bits() {
            rep.dist_identical += 1;
        } else if d_up == OPENMC_INFTY && d == f64::INFINITY {
            rep.dist_both_miss += 1;
        } else {
            ok = false;
            if d_up == OPENMC_INFTY || d == f64::INFINITY {
                rep.hit_vs_miss += 1;
            } else {
                let rel = ((d - d_up) / d_up).abs();
                if rel > rep.worst_rel {
                    rep.worst_rel = rel;
                }
            }
        }
        if sn == s_up {
            rep.sense_identical += 1;
        } else {
            ok = false;
        }
        if !ok && rep.first_bad.len() < 6 {
            rep.first_bad.push(format!(
                "inst {inst} r=({:e},{:e},{:e}) u=({:e},{:e},{:e}) coinc={coinc}: \
                 eval {e:e} vs {e_up:e}; dist {d:e} vs {d_up:e}; sense {sn} vs {s_up}",
                r.x, r.y, r.z, u.u, u.v, u.w
            ));
        }
    }
    rep
}

#[test]
fn xplane_matches_openmc() {
    compare("xplane", |p| SurfaceKind::XPlane(XPlane { x0: p[0], bc: BC }))
        .assert_all_identical("xplane");
}

#[test]
fn yplane_matches_openmc() {
    compare("yplane", |p| SurfaceKind::YPlane(YPlane { y0: p[0], bc: BC }))
        .assert_all_identical("yplane");
}

#[test]
fn zplane_matches_openmc() {
    compare("zplane", |p| SurfaceKind::ZPlane(ZPlane { z0: p[0], bc: BC }))
        .assert_all_identical("zplane");
}

#[test]
fn plane_matches_openmc() {
    compare("plane", |p| {
        SurfaceKind::Plane(Plane { a: p[0], b: p[1], c: p[2], d: p[3], bc: BC })
    })
    .assert_all_identical("plane");
}

#[test]
fn xcylinder_matches_openmc() {
    compare("xcyl", |p| {
        SurfaceKind::XCylinder(XCylinder { y0: p[0], z0: p[1], r: p[2], bc: BC })
    })
    .assert_all_identical("xcyl");
}

#[test]
fn ycylinder_matches_openmc() {
    compare("ycyl", |p| {
        SurfaceKind::YCylinder(YCylinder { x0: p[0], z0: p[1], r: p[2], bc: BC })
    })
    .assert_all_identical("ycyl");
}

#[test]
fn zcylinder_matches_openmc() {
    compare("zcyl", |p| {
        SurfaceKind::ZCylinder(ZCylinder { x0: p[0], y0: p[1], r: p[2], bc: BC })
    })
    .assert_all_identical("zcyl");
}

#[test]
fn sphere_matches_openmc() {
    compare("sphere", |p| {
        SurfaceKind::Sphere(Sphere { x0: p[0], y0: p[1], z0: p[2], r: p[3], bc: BC })
    })
    .assert_all_identical("sphere");
}

#[test]
fn xcone_matches_openmc() {
    compare("xcone", |p| {
        SurfaceKind::XCone(XCone { x0: p[0], y0: p[1], z0: p[2], r_sq: p[3], bc: BC })
    })
    .assert_all_identical("xcone");
}

#[test]
fn ycone_matches_openmc() {
    compare("ycone", |p| {
        SurfaceKind::YCone(YCone { x0: p[0], y0: p[1], z0: p[2], r_sq: p[3], bc: BC })
    })
    .assert_all_identical("ycone");
}

#[test]
fn zcone_matches_openmc() {
    compare("zcone", |p| {
        SurfaceKind::ZCone(ZCone { x0: p[0], y0: p[1], z0: p[2], r_sq: p[3], bc: BC })
    })
    .assert_all_identical("zcone");
}

#[test]
fn quadric_matches_openmc() {
    compare("quadric", |p| {
        SurfaceKind::Quadric(Quadric {
            a: p[0], b: p[1], c: p[2], d: p[3], e: p[4], f: p[5], g: p[6], h: p[7], j: p[8],
            k: p[9], bc: BC,
        })
    })
    .assert_all_identical("quadric");
}

// Tori take OpenMC's (a, b, c) field for field (b along the axis of revolution, c
// perpendicular), as fixed in gh:#701 by this change, and solve the quartic with
// the port of `oqs::quartic_solver`.

fn xtorus(p: &[f64]) -> SurfaceKind {
    SurfaceKind::XTorus(XTorus { x0: p[0], y0: p[1], z0: p[2], a: p[3], b: p[4], c: p[5], bc: BC })
}
fn ytorus(p: &[f64]) -> SurfaceKind {
    SurfaceKind::YTorus(YTorus { x0: p[0], y0: p[1], z0: p[2], a: p[3], b: p[4], c: p[5], bc: BC })
}
fn ztorus(p: &[f64]) -> SurfaceKind {
    SurfaceKind::ZTorus(ZTorus { x0: p[0], y0: p[1], z0: p[2], a: p[3], b: p[4], c: p[5], bc: BC })
}

#[test]
fn xtorus_matches_openmc() {
    compare("xtorus", xtorus).assert_all_identical("xtorus");
}

#[test]
fn ytorus_matches_openmc() {
    compare("ytorus", ytorus).assert_all_identical("ytorus");
}

#[test]
fn ztorus_matches_openmc() {
    compare("ztorus", ztorus).assert_all_identical("ztorus");
}

// ---- integration: ray trace through a multi-surface cell sequence ---------

/// The ten-surface model of the driver's `ray_trace()`, in the same order.
fn trace_surfaces() -> Vec<SurfaceKind> {
    vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: 1.0, bc: BC }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: 2.0, bc: BC }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: 3.5, bc: BC }),
        SurfaceKind::Sphere(Sphere { x0: 0.5, y0: -0.25, z0: 0.0, r: 2.75, bc: BC }),
        SurfaceKind::ZPlane(ZPlane { z0: -4.0, bc: BC }),
        SurfaceKind::ZPlane(ZPlane { z0: 4.0, bc: BC }),
        SurfaceKind::XPlane(XPlane { x0: 2.5, bc: BC }),
        SurfaceKind::Plane(Plane { a: 1.0, b: 2.0, c: -2.0, d: 3.0, bc: BC }),
        SurfaceKind::XCone(XCone { x0: -1.0, y0: 0.0, z0: 0.0, r_sq: 0.25, bc: BC }),
        SurfaceKind::ZTorus(ZTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 2.0, c: 1.0, bc: BC }),
        SurfaceKind::Quadric(Quadric {
            a: 9.0, b: 4.0, c: 36.0, d: 0.0, e: 0.0, f: 0.0, g: 0.0, h: 0.0, j: 0.0, k: -36.0,
            bc: BC,
        }),
    ]
}

/// The six start rays of the driver, as printed (origin and direction are
/// reconstructed by the same expressions the driver used: `unit()` divides by
/// `sqrt(x*x+y*y+z*z)`).
fn trace_rays() -> Vec<(Position, Direction)> {
    fn unit(x: f64, y: f64, z: f64) -> Direction {
        let n = (x * x + y * y + z * z).sqrt();
        Direction::new(x / n, y / n, z / n)
    }
    vec![
        (Position::new(-6.0, 0.1, 0.2), Direction::new(1.0, 0.0, 0.0)),
        (Position::new(-5.0, -4.5, -3.0), unit(1.0, 1.0, 0.6)),
        (Position::new(0.05, 0.02, -6.0), Direction::new(0.0, 0.0, 1.0)),
        (Position::new(4.5, 3.1, 1.7), unit(-1.3, -0.8, -0.4)),
        (Position::new(0.3, -0.2, 0.1), unit(0.2, 0.3, 0.9)),
        (Position::new(-3.0, 2.0, -3.5), unit(0.7, -0.5, 0.45)),
    ]
}

/// Ray trace through ten surfaces of seven kinds, from six start rays, up to 24
/// crossings each, using OpenMC's nearest-surface rule
/// (`Region::distance_to_nearest_surface`, `src/cell.cpp:955-995`) and
/// `r += d * u`, against the driver's trace of OpenMC's surface routines.
///
/// Pass criterion: every step's surface index, distance bits and position bits
/// equal. The result (counts of steps identical before the first divergence, per
/// ray) is in the V&V README.
fn run_trace() -> (usize, usize, Vec<String>) {
    let surfs = trace_surfaces();
    let rays = trace_rays();
    let mut reference: BTreeMap<usize, Vec<(usize, usize, f64, [f64; 3])>> = BTreeMap::new();
    for line in REFERENCE.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first() == Some(&"T") {
            let ray: usize = t[1].parse().unwrap();
            reference.entry(ray).or_default().push((
                t[2].parse().unwrap(),
                t[3].parse().unwrap(),
                hex(t[4]),
                [hex(t[5]), hex(t[6]), hex(t[7])],
            ));
        }
    }
    let (mut total, mut identical) = (0usize, 0usize);
    let mut notes = Vec::new();
    for (k, (r0, u)) in rays.iter().enumerate() {
        let want = &reference[&k];
        let mut r = *r0;
        let mut on: Option<usize> = None;
        let mut step = 0usize;
        let mut diverged = false;
        loop {
            let mut min_dist = OPENMC_INFTY;
            let mut isurf: Option<usize> = None;
            for (i, s) in surfs.iter().enumerate() {
                let d = s.distance(r, *u, on == Some(i));
                if d < min_dist && min_dist - d >= 1.0e-14 * min_dist {
                    min_dist = d;
                    isurf = Some(i);
                }
            }
            let Some(i) = isurf else { break };
            r = Position::new(r.x + min_dist * u.u, r.y + min_dist * u.v, r.z + min_dist * u.w);
            on = Some(i);
            if step < want.len() {
                total += 1;
                let (_, wsurf, wd, wr) = &want[step];
                let same = *wsurf == i
                    && wd.to_bits() == min_dist.to_bits()
                    && wr[0].to_bits() == r.x.to_bits()
                    && wr[1].to_bits() == r.y.to_bits()
                    && wr[2].to_bits() == r.z.to_bits();
                if same && !diverged {
                    identical += 1;
                } else if !diverged {
                    diverged = true;
                    notes.push(format!(
                        "ray {k} step {step}: surface {i} vs {wsurf}, d {min_dist:e} vs {wd:e}"
                    ));
                }
            }
            step += 1;
            if step >= 24 {
                break;
            }
        }
        if step != want.len() && !diverged {
            notes.push(format!("ray {k}: {step} crossings vs {} upstream", want.len()));
        }
    }
    (total, identical, notes)
}

/// Integration test: the whole nearest-surface loop, a ray at a time.
#[test]
fn ray_trace_through_ten_surfaces_matches_openmc() {
    let (total, identical, notes) = run_trace();
    println!("ray trace: {identical}/{total} crossings identical before first divergence");
    for n in &notes {
        println!("  {n}");
    }
    assert!(total > 0);
    assert_eq!(identical, total, "ray trace diverged: {notes:?}");
}
