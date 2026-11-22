// SPDX-License-Identifier: AGPL-3.0-only

//! **gh:#1712: mesh-surface currents on rectilinear, cylindrical and spherical
//! meshes against OpenMC 0.15.3, bin for bin.**
//!
//! # Methodology
//!
//! The deck `verification_and_validation/mesh_surface_meshes_1712/openmc_inputs/
//! beams_mesh_surface.py` (written before any run and unchanged since) fires 66
//! deterministic single-neutron beams through a near-transparent box (O-16 at
//! 1e-10 atoms/b-cm, vacuum on every face of `[-10, 10]^3`, so one straight
//! track per run). Of these, 64 start at random sites (`RandomState(1712)`) and
//! 2 are hand-picked chords. Each run tallies `current` on a `MeshSurfaceFilter`
//! over a rectilinear, a cylindrical (partial phi coverage is not used: phi spans
//! `[0, 2 pi]`) and a spherical mesh, the latter two with origins off zero.
//!
//! OpenMC scores a leaking track once, from the source point to the exit point
//! pushed on by `TINY_BIT` (`particle.cpp:743-748`). This test builds the same
//! track (source -> box exit + 1e-8 cm along `u`) and calls
//! `MeshSurfaceFilter::matches`, one count per matched bin.
//!
//! **Pass:** every bin equal, exactly (the counts are integers). **Prediction,
//! before the run** (`mesh_surface_meshes_1712/README.md`, prediction 5):
//! identical bins except where a track ends within ~1e-15 cm of a mesh surface.
//!
//! The reference files keep only the nonzero bins (`site bin value`).
//!
//! # Results (2026-11-22)
//!
//! See `mesh_surface_meshes_1712/README.md`, "Code-to-code against OpenMC":
//! printed per mesh by this test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::tally::filter::{Filter, FilterEvent};
use outram_mc_libs::tally::filter_extra::MeshSurfaceFilter;
use outram_mc_libs::tally::mesh::{CylindricalMesh, MeshKind, RectilinearMesh, SphericalMesh};
use std::f64::consts::{PI, TAU};

const TINY_BIT: f64 = 1.0e-8;

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/mesh_surface_meshes_1712/openmc_reference")
}

fn sites() -> Vec<[f64; 7]> {
    std::fs::read_to_string(ref_dir().join("sites.txt"))
        .expect("sites.txt")
        .lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3], v[4], v[5], v[6]]
        })
        .collect()
}

/// `(site, bin) -> value` for the nonzero bins of one mesh.
fn reference(name: &str) -> BTreeMap<(usize, usize), f64> {
    std::fs::read_to_string(ref_dir().join(format!("{name}_mesh_surface.txt")))
        .expect("reference")
        .lines()
        .map(|l| {
            let v: Vec<&str> = l.split_whitespace().collect();
            ((v[0].parse().unwrap(), v[1].parse().unwrap()), v[2].parse().unwrap())
        })
        .collect()
}

/// The meshes of the deck, as written there.
fn meshes() -> Vec<(&'static str, MeshKind)> {
    vec![
        (
            "rectilinear",
            MeshKind::Rectilinear(RectilinearMesh {
                grid: [
                    vec![-8.0, -5.0, -1.0, 0.5, 4.0, 8.0],
                    vec![-7.0, -2.0, 0.0, 3.0, 7.0],
                    vec![-6.0, -1.0, 2.5, 6.0],
                ],
            }),
        ),
        (
            "cylindrical",
            MeshKind::Cylindrical(CylindricalMesh {
                r_grid: vec![0.0, 1.5, 3.0, 5.0, 8.0],
                phi_grid: vec![0.0, 1.0, 2.5, 4.0, TAU],
                z_grid: vec![-6.0, -2.0, 0.0, 3.0, 6.0],
                origin: Position::new(0.5, 0.5, 1.0),
            }),
        ),
        (
            "spherical",
            MeshKind::Spherical(SphericalMesh {
                r_grid: vec![0.0, 1.0, 3.0, 5.5, 8.0],
                theta_grid: vec![0.0, 0.7, PI / 2.0, 2.4, PI],
                phi_grid: vec![0.0, 2.0, 4.5, TAU],
                origin: Position::new(-1.0, 0.4, 1.0),
            }),
        ),
    ]
}

/// Distance from `r` along `u` to the vacuum box `[-10, 10]^3`.
fn exit_distance(r: [f64; 3], u: [f64; 3]) -> f64 {
    let mut d = f64::INFINITY;
    for k in 0..3 {
        if u[k] > 0.0 {
            d = d.min((10.0 - r[k]) / u[k]);
        } else if u[k] < 0.0 {
            d = d.min((-10.0 - r[k]) / u[k]);
        }
    }
    d
}

#[test]
fn mesh_surface_currents_match_openmc_on_three_mesh_types() {
    let sites = sites();
    let mut total_misses = 0;
    for (name, kind) in meshes() {
        let f = MeshSurfaceFilter::new(kind).expect("structured mesh");
        let theirs = reference(name);
        let mut ours: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (i, s) in sites.iter().enumerate() {
            let r0 = Position::new(s[0], s[1], s[2]);
            // OpenMC renormalises the monodirectional reference direction when
            // it reads it (`u_ref_ /= u_ref_.norm()`, `distribution_multi.cpp:47`,
            // `norm()` = `sqrt(x*x + y*y + z*z)`); the same input here.
            let n = (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]).sqrt();
            let uv = [s[3] / n, s[4] / n, s[5] / n];
            let u = Direction { u: uv[0], v: uv[1], w: uv[2] };
            let d = exit_distance([s[0], s[1], s[2]], uv) + TINY_BIT;
            let r1 = Position::new(s[0] + d * uv[0], s[1] + d * uv[1], s[2] + d * uv[2]);
            let ev = FilterEvent { position_last: r0, position: r1, direction: u, ..FilterEvent::default() };
            for b in f.matches(&ev) {
                assert!(b < f.n_bins());
                *ours.entry((i, b)).or_insert(0.0) += 1.0;
            }
        }
        let mut misses = 0;
        let keys: std::collections::BTreeSet<_> = ours.keys().chain(theirs.keys()).copied().collect();
        for k in &keys {
            let a = ours.get(k).copied().unwrap_or(0.0);
            let b = theirs.get(k).copied().unwrap_or(0.0);
            if a != b {
                misses += 1;
                println!("  MISS {name} site {} bin {}: ours {a} openmc {b}", k.0, k.1);
            }
        }
        println!(
            "[{name}] sites {} nonzero bins ours {} openmc {} misses {misses}",
            sites.len(),
            ours.len(),
            theirs.len()
        );
        total_misses += misses;
    }
    assert_eq!(total_misses, 0, "mesh-surface bins differ from OpenMC");
}
