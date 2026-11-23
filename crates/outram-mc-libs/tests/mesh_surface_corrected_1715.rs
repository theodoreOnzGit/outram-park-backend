// SPDX-License-Identifier: AGPL-3.0-only

//! **gh:#1715: the corrected mesh-surface raytrace, beside the port.**
//!
//! `MeshSurfaceRaytrace::Corrected` (the default of `MeshSurfaceFilter`) fixes
//! three defects OpenMC 0.15.3 (`27e38e89`) itself has, all confirmed
//! code-to-code (gh:#1712, and test 4 here for B):
//!
//! - **A.** an inward current labelled by the next exit, not the face entered;
//! - **B.** a track starting outside a partial phi (or theta) range is lost;
//! - **C.** a crossing of a theta = pi/2 edge lost to round-off.
//!
//! `MeshSurfaceRaytrace::OpenMc` is the port, kept by name.
//!
//! # Methodology and predictions (written 2026-11-23, before the first run)
//!
//! 1. **Cartesian identity.** On regular and rectilinear meshes the corrected
//!    route equals the port bin for bin (A cannot differ on a Cartesian axis;
//!    B and C do not arise). 20 000 random tracks each. *Predicted: identical.*
//! 2. **Pairs.** Every crossing between neighbouring elements scores an
//!    outward bin on axis k and face m, followed by an inward bin on axis k and
//!    face !m (the face entered). (Amended after the first run, before any
//!    result was judged: a track that leaves the mesh and re-enters it in a
//!    non-neighbouring element is not a pair; the first check counted 2 such
//!    exits-and-re-entries, confirmed real by check 3, as violations.) Random tracks on cylindrical and spherical meshes, full and
//!    partial angular grids, theta edges at pi/2. *Predicted: holds on every
//!    track for the corrected route; fails for the port (A).*
//! 3. **Geometry.** The sequence of elements a track enters, read from the
//!    corrected route's bins, equals the sequence found by sampling the mesh's
//!    own element lookup (`MeshKindExt::bin`) every 1e-4 cm along the track.
//!    *Predicted: equal on every track for the corrected route; the port fails
//!    on tracks starting outside a partial angular range (B) and on some pi/2
//!    crossings (C).*
//! 4. **The port still matches OpenMC on partial-phi meshes**
//!    (`openmc_inputs/beams_partial_phi.py`, run 2026-11-23, finding B
//!    confirmed), and the corrected route scores the three tracks OpenMC loses.
//!    *Predicted: port 0 misses; corrected scores sites 0-2.*
//!
//! # Results (2026-11-23)
//!
//! Printed per test; recorded in
//! `verification_and_validation/mesh_surface_meshes_1712/README.md`, section
//! "gh:#1715".

use std::collections::BTreeMap;
use std::path::PathBuf;

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::tally::filter::FilterEvent;
use outram_mc_libs::tally::filter_extra::MeshSurfaceFilter;
use outram_mc_libs::tally::mesh::{
    mesh_surface_bins_crossed, CylindricalMesh, MeshKind, MeshKindExt, MeshSurfaceRaytrace, RectilinearMesh,
    RegularMesh, SphericalMesh,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

const BINS_PER_ELEMENT: usize = 12;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn unit(a: Position, b: Position) -> Direction {
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    Direction { u: d[0] / n, v: d[1] / n, w: d[2] / n }
}

fn random_track(g: &mut Lcg, half: f64) -> (Position, Position) {
    let mut p = || Position::new(half * (2.0 * g.next() - 1.0), half * (2.0 * g.next() - 1.0), half * (2.0 * g.next() - 1.0));
    (p(), p())
}

fn curvilinear_meshes() -> Vec<(&'static str, MeshKind)> {
    vec![
        (
            "cylindrical full phi",
            MeshKind::Cylindrical(CylindricalMesh {
                r_grid: vec![0.0, 1.5, 3.0, 5.0],
                phi_grid: vec![0.0, 1.0, 2.5, 4.0, TAU],
                z_grid: vec![-4.0, 0.0, 4.0],
                origin: Position::new(0.3, -0.2, 0.1),
            }),
        ),
        (
            "cylindrical partial phi",
            MeshKind::Cylindrical(CylindricalMesh {
                r_grid: vec![0.0, 2.0, 5.0],
                phi_grid: vec![0.0, 1.0, 2.5],
                z_grid: vec![-4.0, 0.0, 4.0],
                origin: Position::new(0.0, 0.0, 0.0),
            }),
        ),
        (
            "spherical, theta edge pi/2",
            MeshKind::Spherical(SphericalMesh {
                r_grid: vec![0.0, 1.0, 3.0, 5.5],
                theta_grid: vec![0.0, 0.7, FRAC_PI_2, 2.4, PI],
                phi_grid: vec![0.0, 2.0, 4.5, TAU],
                origin: Position::new(-0.4, 0.4, 0.3),
            }),
        ),
        (
            "spherical, partial theta and phi",
            MeshKind::Spherical(SphericalMesh {
                r_grid: vec![0.0, 2.0, 5.0],
                theta_grid: vec![0.6, FRAC_PI_2, 2.0],
                phi_grid: vec![0.0, 1.0, 2.5],
                origin: Position::new(0.0, 0.0, 0.0),
            }),
        ),
    ]
}

fn decode(b: usize) -> (usize, usize, bool, bool) {
    let e = b / BINS_PER_ELEMENT;
    let r = b % BINS_PER_ELEMENT;
    (e, r / 4, (r % 4) / 2 == 1, r % 2 == 1)
}

#[test]
fn the_corrected_route_equals_the_port_on_cartesian_meshes() {
    let meshes = [
        MeshKind::Regular(RegularMesh { lower_left: [-4.0, -3.0, -2.0], upper_right: [4.0, 3.0, 5.0], dimension: [4, 3, 7] }),
        MeshKind::Rectilinear(RectilinearMesh {
            grid: [vec![-4.0, -1.0, 0.5, 4.0], vec![-3.0, 0.0, 3.0], vec![-2.0, -1.5, 1.0, 5.0]],
        }),
    ];
    let mut g = Lcg(0x1715);
    for m in &meshes {
        let mut bins = 0;
        for _ in 0..20_000 {
            let (a, b) = random_track(&mut g, 6.0);
            let u = unit(a, b);
            let c = mesh_surface_bins_crossed(m, a, b, u, MeshSurfaceRaytrace::Corrected).unwrap();
            let o = mesh_surface_bins_crossed(m, a, b, u, MeshSurfaceRaytrace::OpenMc).unwrap();
            assert_eq!(c, o, "{a:?} -> {b:?}");
            bins += c.len();
        }
        println!("cartesian: 20000 tracks identical, {bins} bins");
        assert!(bins > 20_000);
    }
}

/// Every in-mesh crossing is an (outward k m, inward k !m) pair; returns the
/// number of violations. An outward bin followed by an inward one is a crossing
/// between neighbours only when the two elements are adjacent along axis k
/// (the phi axis wraps when it is full); otherwise the track left the mesh and
/// re-entered it, which is not a pair (e.g. out through a theta cone in one phi
/// sector and back in through the same cone in another).
fn pair_violations(bins: &[usize], shape: [usize; 3], wraps: [bool; 3]) -> usize {
    let idx = |e: usize| [e % shape[0], (e / shape[0]) % shape[1], e / (shape[0] * shape[1])];
    let mut bad = 0;
    for w in bins.windows(2) {
        let (e0, k0, m0, in0) = decode(w[0]);
        let (e1, k1, m1, in1) = decode(w[1]);
        if in0 || !in1 || k0 != k1 {
            continue;
        }
        let (a, b) = (idx(e0), idx(e1));
        let others_equal = (0..3).all(|j| j == k0 || a[j] == b[j]);
        let d = a[k0].abs_diff(b[k0]);
        let adjacent = d == 1 || (wraps[k0] && d == shape[k0] - 1 && shape[k0] > 1);
        if others_equal && adjacent && m0 == m1 {
            bad += 1;
        }
    }
    bad
}

/// Element shape and which axes wrap (full phi), for [`pair_violations`].
fn shape_of(m: &MeshKind) -> ([usize; 3], [bool; 3]) {
    match m {
        MeshKind::Cylindrical(c) => {
            let d = c.dimension();
            let full = c.phi_grid[0] == 0.0 && *c.phi_grid.last().unwrap() == TAU;
            (d, [false, full, false])
        }
        MeshKind::Spherical(sp) => {
            let d = sp.dimension();
            let full = sp.phi_grid[0] == 0.0 && *sp.phi_grid.last().unwrap() == TAU;
            (d, [false, false, full])
        }
        _ => unreachable!("curvilinear meshes only"),
    }
}

/// The elements entered, in order: the start element (if the track starts
/// inside) and each element an inward bin names; consecutive repeats merged.
fn walked_elements(bins: &[usize], start: Option<usize>) -> Vec<usize> {
    let mut seq: Vec<usize> = start.into_iter().collect();
    for &b in bins {
        let (e, _, _, inward) = decode(b);
        if inward && seq.last() != Some(&e) {
            seq.push(e);
        }
    }
    seq
}

/// The elements found by sampling `mesh.bin` every `step` cm along the track,
/// consecutive repeats and points outside the mesh dropped.
fn sampled_elements(mesh: &MeshKind, a: Position, b: Position, step: f64) -> Vec<usize> {
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let n = (len / step).ceil() as usize;
    let mut seq = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        if let Some(e) = mesh.bin(Position::new(a.x + t * d[0], a.y + t * d[1], a.z + t * d[2])) {
            if seq.last() != Some(&e) {
                seq.push(e);
            }
        }
    }
    seq
}

#[test]
fn the_corrected_route_follows_the_geometry_and_pairs_its_faces() {
    let mut g = Lcg(0x1715_0002);
    for (name, m) in curvilinear_meshes() {
        let (mut c_pairs, mut o_pairs, mut c_geom, mut o_geom, mut tracks) = (0, 0, 0, 0, 0);
        while tracks < 400 {
            let (a, b) = random_track(&mut g, 6.5);
            let u = unit(a, b);
            let c = mesh_surface_bins_crossed(&m, a, b, u, MeshSurfaceRaytrace::Corrected).unwrap();
            let o = mesh_surface_bins_crossed(&m, a, b, u, MeshSurfaceRaytrace::OpenMc).unwrap();
            // The start element, by the mesh's own lookup a hair along the track.
            let s = Position::new(a.x + 1e-8 * u.u, a.y + 1e-8 * u.v, a.z + 1e-8 * u.w);
            let start = m.bin(s);
            let truth = sampled_elements(&m, a, b, 1e-4);
            // Skip tracks whose sampled sequence could miss a sliver (an element
            // crossed for under ~1e-3 cm): rare, and not what is tested here.
            let walked_c = walked_elements(&c, start);
            let (shape, wraps) = shape_of(&m);
            let pv = pair_violations(&c, shape, wraps);
            if pv > 0 {
                println!("  {name}: pair violation {:?} on {a:?} -> {b:?}", c.iter().map(|&x| decode(x)).collect::<Vec<_>>());
            }
            c_pairs += pv;
            o_pairs += pair_violations(&o, shape, wraps);
            if walked_c != truth {
                c_geom += 1;
                println!("  {name}: corrected {walked_c:?} vs sampled {truth:?} on {a:?} -> {b:?}");
            }
            if walked_elements(&o, start) != truth {
                o_geom += 1;
            }
            tracks += 1;
        }
        println!(
            "[{name}] {tracks} tracks: pair violations corrected {c_pairs}, port {o_pairs}; \
             geometry mismatches corrected {c_geom}, port {o_geom}"
        );
        assert_eq!(c_pairs, 0, "{name}: corrected route labels a face it did not enter");
        assert_eq!(c_geom, 0, "{name}: corrected route disagrees with the geometry");
    }
}

#[test]
fn a_theta_half_pi_crossing_is_scored_by_the_corrected_route() {
    // Site 55 of the gh:#1712 deck: OpenMC (and the port) lose its z = 0
    // crossing of the theta = pi/2 edge.
    let m = &curvilinear_meshes()[2].1;
    let MeshKind::Spherical(sph) = m else { unreachable!() };
    let o = sph.origin;
    // Straight down through the theta = pi/2 plane, off the axis.
    let a = Position::new(o.x + 2.0, o.y + 0.5, o.z + 1.0);
    let b = Position::new(o.x + 2.0, o.y + 0.5, o.z - 1.0);
    let u = unit(a, b);
    let c = mesh_surface_bins_crossed(m, a, b, u, MeshSurfaceRaytrace::Corrected).unwrap();
    let theta_crossings = c.iter().filter(|&&x| decode(x).1 == 1).count();
    assert_eq!(theta_crossings, 2, "one theta crossing: outward + inward, {c:?}");
}

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/mesh_surface_meshes_1712/openmc_reference")
}

#[test]
fn partial_phi_port_matches_openmc_and_the_corrected_route_scores_the_lost_tracks() {
    let sites: Vec<[f64; 7]> = std::fs::read_to_string(ref_dir().join("partial_phi_sites.txt"))
        .expect("sites")
        .lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3], v[4], v[5], v[6]]
        })
        .collect();
    let meshes = [
        (
            "cylindrical",
            MeshKind::Cylindrical(CylindricalMesh {
                r_grid: vec![0.0, 2.0, 5.0, 8.0],
                phi_grid: vec![0.0, 1.0, 2.5],
                z_grid: vec![-4.0, 0.0, 4.0],
                origin: Position::new(0.0, 0.0, 0.0),
            }),
        ),
        (
            "spherical",
            MeshKind::Spherical(SphericalMesh {
                r_grid: vec![0.0, 2.0, 5.0, 8.0],
                theta_grid: vec![0.0, 1.2, 2.0, PI],
                phi_grid: vec![0.0, 1.0, 2.5],
                origin: Position::new(0.0, 0.0, 0.0),
            }),
        ),
    ];
    for (name, kind) in meshes {
        let port = MeshSurfaceFilter::new(kind.clone()).unwrap().with_openmc_raytrace();
        let corrected = MeshSurfaceFilter::new(kind).unwrap();
        let theirs: BTreeMap<(usize, usize), f64> =
            std::fs::read_to_string(ref_dir().join(format!("partial_{name}_mesh_surface.txt")))
                .expect("reference")
                .lines()
                .map(|l| {
                    let v: Vec<&str> = l.split_whitespace().collect();
                    ((v[0].parse().unwrap(), v[1].parse().unwrap()), v[2].parse().unwrap())
                })
                .collect();
        let mut ours: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        let mut corrected_per_site = vec![0usize; sites.len()];
        for (i, s) in sites.iter().enumerate() {
            // As OpenMC reads it: renormalised (`distribution_multi.cpp:47`).
            let n = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
            let u = [s[3] / n, s[4] / n, s[5] / n];
            let mut d = f64::INFINITY;
            for k in 0..3 {
                if u[k] > 0.0 {
                    d = d.min((10.0 - s[k]) / u[k]);
                } else if u[k] < 0.0 {
                    d = d.min((-10.0 - s[k]) / u[k]);
                }
            }
            d += 1.0e-8;
            let ev = FilterEvent {
                position_last: Position::new(s[0], s[1], s[2]),
                position: Position::new(s[0] + d * u[0], s[1] + d * u[1], s[2] + d * u[2]),
                direction: Direction { u: u[0], v: u[1], w: u[2] },
                ..FilterEvent::default()
            };
            for b in port.matches(&ev) {
                *ours.entry((i, b)).or_insert(0.0) += 1.0;
            }
            corrected_per_site[i] = corrected.matches(&ev).len();
        }
        let keys: std::collections::BTreeSet<_> = ours.keys().chain(theirs.keys()).copied().collect();
        let misses = keys.iter().filter(|k| ours.get(k) != theirs.get(k)).count();
        println!(
            "[partial {name}] port vs OpenMC: {} / {} nonzero bins, misses {misses}; corrected bins on sites 0-2: {:?}",
            ours.len(),
            theirs.len(),
            &corrected_per_site[..3]
        );
        assert_eq!(misses, 0, "{name}: the port must match OpenMC");
        for i in 0..3 {
            assert!(theirs.keys().all(|k| k.0 != i), "OpenMC loses site {i} (finding B)");
            assert!(corrected_per_site[i] > 0, "{name}: the corrected route scores site {i}");
        }
    }
}
