// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM-dev 852527377c7e (OpenFOAM Foundation); the
// frozen hashes below come from runs of that commit's `blockMesh` and
// `polyDualMesh`.

//! Dev-line pins (gh:#1364, gh:#1458/#1462, gh:#1460, gh:#1459): the port's
//! DEFAULT (dev) flavour against a real OpenFOAM-dev build at commit
//! `852527377c7e` (partial build, dummy Pstream, `writePrecision 17`).
//!
//! **Method and results:** `verification_and_validation/upstream_dev_852527377c7e/`
//! (README.md has the build recipe, the scripts and the per-case table). The
//! binary is not needed here: this file holds FNV-1a hashes of the binary's
//! output (`scripts/dev_hashes.py` computes them from a polyMesh directory).
//!
//! - **points hash**: FNV-1a over the f64 bit patterns, pinned only where the
//!   port is bit-identical to the binary on every point.
//! - **topology hash**: FNV-1a over u64 values, per face `(len, verts...)`,
//!   then `owner`, then `neighbour`; pinned on every case (faces, owner and
//!   neighbour were identical on all 12 blockMesh and 12 polyDualMesh cases).
//! - the three petir-trig cases (T3A, `sweep_sphere`, `sweep_torus_minor`) are
//!   pinned by the exact list of points that differ from the binary and their
//!   dev bit patterns (`tests/fixtures/dev_trig_ulp_points.txt`).
//! - the two non-exact blockMesh cases the issues ask about (`sweep` torus,
//!   minor edges, factor 1, and the sphere block) are pinned by the worst
//!   residual measured on the BINARY's points, which the port reproduces to
//!   all printed digits: the dev binary is itself 1.157e-4 off the torus.

use outram_foam_basic_lib::io::PolyMesh as FoamMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::block_mesh::BlockMeshDict;
use outram_foam_mesh::poly_dual_mesh::PolyMesh as Primal;
use outram_foam_mesh::poly_dual_mesh_upstream::{poly_dual_mesh_upstream, UpstreamOptions};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn cavity_dict() -> String {
    std::fs::read_to_string(format!("{}/tests/blockMeshDict.cavity", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
}

fn fnv(it: impl Iterator<Item = u64>) -> u64 {
    it.fold(0xcbf29ce484222325u64, |h, b| (h ^ b).wrapping_mul(0x100000001b3))
}

/// FNV-1a over the bit patterns of the points (the `compare_polymesh` hash).
fn points_hash(m: &FoamMesh) -> u64 {
    fnv(m.points.iter().flat_map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]))
}

/// FNV-1a over `(len, verts...)` per face, then owner, then neighbour.
fn topology_hash(m: &FoamMesh) -> u64 {
    let faces = m.faces.iter().flat_map(|f| {
        std::iter::once(f.verts.len() as u64).chain(f.verts.iter().map(|&v| v as u64))
    });
    let owner = m.faces.iter().map(|f| f.owner as u64);
    let nei = m.faces.iter().filter_map(|f| f.neighbour).map(|n| n as u64);
    fnv(faces.chain(owner).chain(nei))
}

fn block_mesh(dict: &str) -> FoamMesh {
    BlockMeshDict::parse(dict).unwrap().build().unwrap().to_foam_poly_mesh()
}

/// blockMesh, dev flavour: topology identical to the dev binary on all twelve
/// fixture dicts; points bit-identical on nine of them (`Some(hash)`); the other
/// three differ only by petir trig, pinned by
/// `dev_block_mesh_petir_trig_differences_are_pinned`.
#[test]
fn dev_block_mesh_matches_the_dev_binary() {
    // (name, dict text, points, faces, points hash if bit-identical, topology hash)
    let cases: Vec<(&str, String, usize, usize, Option<u64>, u64)> = vec![
        ("cavity", cavity_dict(), 882, 1640, Some(0xbfc09ba0165b42f1), 0xfae50c490be2fe5b),
        ("T3A", fixture("T3A_blockMeshDict"), 54496, 107707, None, 0x679f3117dfb43b37),
        ("pitzDailySteady", fixture("pitzDailySteady_blockMeshDict"), 25012, 49180, Some(0x0ce06ce8718d0416), 0x9d7497b41a081079),
        ("proj_cyl", fixture("proj_cyl.dict"), 567, 1312, Some(0xd0d3cd7ad736e121), 0x7ee97b855d18eaf1),
        ("proj_edges", fixture("proj_edges.dict"), 7351, 21300, Some(0xa733ce51f1d6cd0f), 0x6b8d4b9a08a6acc9),
        ("proj_faces", fixture("proj_faces.dict"), 7351, 21300, Some(0xabb3ba6e93ca39a6), 0x6b8d4b9a08a6acc9),
        ("proj_two", fixture("proj_two.dict"), 567, 1312, Some(0xcc961f0716f02692), 0x7ee97b855d18eaf1),
        ("proj_vertex", fixture("proj_vertex.dict"), 210, 434, Some(0x81d60ecae0a68c88), 0x500279b355dde49d),
        ("sweep_sphere", fixture("sweep_sphere.dict"), 1331, 3300, None, 0x97d1e04aca89e01d),
        ("sweep_torus_major", fixture("sweep_torus_major.dict"), 637, 1476, Some(0xbeb91bc2ae7f59a7), 0x82d8bbda2aa3be31),
        ("sweep_torus_minor", fixture("sweep_torus_minor.dict"), 637, 1476, None, 0x82d8bbda2aa3be31),
        ("sweep_torus_minor_wrong", fixture("sweep_torus_minor_wrong.dict"), 637, 1476, Some(0xb720b2bc0527fc1f), 0x82d8bbda2aa3be31),
    ];
    for (name, dict, n_pts, n_faces, pts_hash, topo) in cases {
        let m = block_mesh(&dict);
        assert_eq!(m.points.len(), n_pts, "{name}: points");
        assert_eq!(m.faces.len(), n_faces, "{name}: faces");
        assert_eq!(topology_hash(&m), topo, "{name}: faces/owner/neighbour differ from OpenFOAM-dev");
        if let Some(h) = pts_hash {
            assert_eq!(points_hash(&m), h, "{name}: points not bit-identical to OpenFOAM-dev");
        }
    }
}

/// The three dicts whose points differ from the dev binary only through
/// petir's `sin`/`cos`/`tan`/`acos`/`asin` against glibc (accepted below the
/// 1e-8 round-off allowance, flagged for the maintainer; gh:#1364).
///
/// Methodology: the dev binary's `points` hash (`dev_hashes.py`) is pinned. The
/// port's points with the listed points replaced by the binary's bit patterns
/// (`tests/fixtures/dev_trig_ulp_points.txt`) must hash to it, which fixes
/// both the set of differing points and every other point's bit-identity. The
/// count and the worst absolute difference are pinned as well.
///
/// Results (2026-11-11, after the integer-pow and unclamped-division fixes):
/// T3A 80 of 54496 points differ, worst 5.55e-17 (46220 differing, worst
/// 1.78e-15 before the fixes); `sweep_sphere` 42 of 1331, worst 4.44e-16;
/// `sweep_torus_minor` 4 of 637, worst 1.11e-16. All far below 1e-8 relative.
#[test]
fn dev_block_mesh_petir_trig_differences_are_pinned() {
    let table = fixture("dev_trig_ulp_points.txt");
    // (name, dict file, dev points hash, differing points, worst abs diff)
    let cases = [
        ("T3A", "T3A_blockMeshDict", 0x2cdae0205111a726u64, 80usize, 5.551115123125783e-17f64),
        ("sweep_sphere", "sweep_sphere.dict", 0xb21561f6557af09b, 42, 4.440892098500626e-16),
        ("sweep_torus_minor", "sweep_torus_minor.dict", 0x4e2c8df34da90e66, 4, 1.1102230246251565e-16),
    ];
    for (name, file, dev_hash, n_diff, worst_pin) in cases {
        let m = block_mesh(&fixture(file));
        let mut bits: Vec<[u64; 3]> =
            m.points.iter().map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]).collect();
        let (mut n, mut worst) = (0usize, 0.0f64);
        for line in table.lines().filter(|l| !l.starts_with('#')) {
            let t: Vec<&str> = line.split_whitespace().collect();
            if t.len() != 5 || t[0] != name {
                continue;
            }
            let i: usize = t[1].parse().unwrap();
            let dev: Vec<u64> = t[2..5].iter().map(|h| u64::from_str_radix(h, 16).unwrap()).collect();
            for k in 0..3 {
                worst = worst.max((f64::from_bits(bits[i][k]) - f64::from_bits(dev[k])).abs());
            }
            assert_ne!(bits[i], [dev[0], dev[1], dev[2]], "{name} point {i} is bit-identical, drop it from the table");
            bits[i] = [dev[0], dev[1], dev[2]];
            n += 1;
        }
        assert_eq!(n, n_diff, "{name}: differing points");
        assert_eq!(worst, worst_pin, "{name}: worst absolute difference");
        let h = fnv(bits.iter().flatten().copied());
        assert_eq!(h, dev_hash, "{name}: not bit-identical to OpenFOAM-dev outside the listed points");
    }
}

/// Distance of `p` from the torus centred (6,0,0) with axis y, R = 2, r = 1.
fn torus_residual(p: Vector3) -> f64 {
    let (x, z) = (p.x - 6.0, p.z);
    (((x * x + z * z).sqrt() - 2.0).powi(2) + p.y * p.y).sqrt() - 1.0
}

fn boundary_residuals(m: &FoamMesh, f: impl Fn(Vector3) -> f64) -> Vec<f64> {
    let mut idx: Vec<usize> = m.faces[m.n_internal_faces..].iter().flat_map(|f| f.verts.iter().copied()).collect();
    idx.sort_unstable();
    idx.dedup();
    idx.into_iter().map(|i| f(m.points[i])).collect()
}

/// gh:#1462 verdict: the dev binary's own `sweep` torus (minor edges, factor
/// 1: the case the dev tutorial calls "exact") is 1.1566824939e-4 off the
/// torus, measured on the binary's `points` (python, writePrecision 17), 270
/// boundary nodes off the torus; the port gives 1.1566824939102e-4. The major
/// case is exact to 2e-15 with the 50 end-disc interiors off the torus, and
/// the sphere block is 0.1389840342370774 short of the sphere in both.
#[test]
fn dev_sweep_torus_is_not_exact_in_the_binary_either() {
    let torus = |name: &str| {
        let m = block_mesh(&fixture(name));
        let r: Vec<f64> = boundary_residuals(&m, |p| torus_residual(p).abs());
        let off = r.iter().filter(|x| **x > 1e-9).count();
        let near = r.iter().copied().filter(|x| *x < 1e-3).fold(0.0f64, f64::max);
        (off, near)
    };
    let (off, near) = torus("sweep_torus_minor.dict");
    assert_eq!(off, 270);
    assert!((near - 1.156682493910e-4).abs() < 1e-15, "{near:e}");
    let (off, near) = torus("sweep_torus_major.dict");
    assert_eq!(off, 50);
    assert!(near < 1e-14, "{near:e}");
    let m = block_mesh(&fixture("sweep_sphere.dict"));
    let c = Vector3::new(8.0, 0.0, 1.0);
    let miss = boundary_residuals(&m, |p| ((p - c).mag() - 3.0f64.sqrt()).abs())
        .into_iter()
        .fold(0.0f64, f64::max);
    assert!((miss - 0.1389840342370774).abs() < 1e-15, "{miss:e}");
}

/// The primal of a dual case: the port's own blockMesh (bit-identical to the
/// dev binary's primal on all four, checked with `compare_polymesh`).
fn primal(dict: &str) -> (Primal, Vec<String>) {
    let io = block_mesh(dict);
    let p = Primal {
        points: io.points.clone(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
        n_internal_faces: io.n_internal_faces,
        patches: io.patches.clone(),
        n_cells: io.n_cells,
    };
    let names = io.patches.iter().map(|p| p.name.clone()).collect();
    (p, names)
}

/// polyDualMesh, dev flavour (the default `UpstreamOptions`, feature angle 60):
/// all twelve case x variant duals are bit-identical to the dev binary's.
#[test]
fn dev_poly_dual_mesh_matches_the_dev_binary() {
    // (case, variant, points, faces, points hash, topology hash)
    let rows: [(&str, &str, usize, usize, u64, u64); 12] = [
        ("cavity", "base", 1832, 3475, 0xc70d57282e84247e, 0x95dc014b3863d5fc),
        ("cavity", "split", 4321, 6154, 0xb6393d22445e45c9, 0x8b1f6c36657cc4f2),
        ("cavity", "concave", 1832, 3475, 0xc70d57282e84247e, 0x95dc014b3863d5fc),
        ("annulus", "base", 924, 1670, 0x5af2dee849024f74, 0xb8416673db318d81),
        ("annulus", "split", 2288, 3478, 0x80ca5a0854dc5035, 0x511272fc6be08972),
        ("annulus", "concave", 924, 1670, 0x5af2dee849024f74, 0xb8416673db318d81),
        ("warped", "base", 480, 833, 0xc8e74dd971f08259, 0x08b2828fc4e770ea),
        ("warped", "split", 1133, 1750, 0xda286fc1785322f7, 0x61156310976036cc),
        ("warped", "concave", 480, 833, 0xc8e74dd971f08259, 0x08b2828fc4e770ea),
        ("lshape", "base", 522, 830, 0x5fdaabe20c0f621d, 0xf9c56a6e76e5d9e2),
        ("lshape", "split", 1000, 1516, 0xff2954a5da464b85, 0x8b190538001a7f36),
        ("lshape", "concave", 531, 857, 0xdc9aedb0172be8ff, 0x60ceda4a8b7709cd),
    ];
    let dicts = [
        ("cavity", cavity_dict()),
        ("annulus", fixture("dual_annulus.dict")),
        ("warped", fixture("dual_warped.dict")),
        ("lshape", fixture("dual_lshape.dict")),
    ];
    for (case, variant, n_pts, n_faces, pts_hash, topo) in rows {
        let dict = &dicts.iter().find(|(c, _)| *c == case).unwrap().1;
        let (p, names) = primal(dict);
        let o = UpstreamOptions::new(60.0);
        let o = match variant {
            "split" => o.with_split_all_faces(),
            "concave" => o.with_concave_multi_cells(),
            _ => o,
        };
        let dual = poly_dual_mesh_upstream(&p, &o).unwrap().to_foam_poly_mesh(&names).unwrap();
        assert_eq!(dual.points.len(), n_pts, "{case} {variant}");
        assert_eq!(dual.faces.len(), n_faces, "{case} {variant}");
        assert_eq!(points_hash(&dual), pts_hash, "{case} {variant}: points differ from OpenFOAM-dev");
        assert_eq!(topology_hash(&dual), topo, "{case} {variant}: topology differs from OpenFOAM-dev");
    }
}

/// The comparison discriminates: the v2506 flavour of the dual differs from
/// the dev binary's output on the annulus (points not bit-identical), so the
/// pin above is not a flavour-blind check.
#[test]
fn dev_dual_pin_discriminates_the_esi_flavour() {
    let (p, names) = primal(&fixture("dual_annulus.dict"));
    let o = UpstreamOptions::new(60.0).with_esi2506_flavour();
    let dual = poly_dual_mesh_upstream(&p, &o).unwrap().to_foam_poly_mesh(&names).unwrap();
    assert_ne!(points_hash(&dual), 0x5af2dee849024f74);
}
