// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! gh:#1465 / #1466: castellation of OpenFOAM's `flange` against
//! **OpenFOAM.com ESI v2506** `snappyHexMesh`, cell by cell.
//!
//! Full methodology, predictions (committed before the runs) and results:
//! `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`.
//!
//! **Case.** `tutorials/mesh/snappyHexMesh/flange` of v2506, reduced to what
//! the port expresses: `blockMesh` box (-0.03..0.03, -0.03..0.03, -0.03..0.01)
//! of 20 x 16 x 12 cells, `flange.stl` (four `solid` blocks, 6468 triangles),
//! `level (2 2)`, `nCellsBetweenLevels 1`, `locationInMesh (-9.23149e-05
//! -0.0025 -0.0025)`, castellation only, no features, no refinement regions.
//! Reference data (`tests/fixtures/esi_v2506_flange/`, from the v2506 binary
//! built per the recipe in the README, `writePrecision 16`): the cells of the
//! mesh after `surfaceOnlyRefine` (42788) and of the final castellated mesh
//! (18278), one `level i j k` per cell on the level-`level` integer lattice.
//!
//! **Results (2026-11-12, release).**
//! - Stage 1, `surfaceOnlyRefine`: with upstream's STL reader (every solid,
//!   single-precision corners) the per-iteration selected-cell counts are
//!   914 / 4294 / 344 / 12 / 0, equal to ESI, and **all 42788 cells and levels
//!   equal ESI's**. With the plain double-precision reader they are
//!   914 / 4294 / 370 / 14 / 0 (42984 leaves): pinned below as the measured
//!   effect of the single-precision corners.
//! - Final cells: the port keeps 18014, ESI 18278. 17974 are equal. ESI has 304
//!   more and the port 40 that ESI lacks. 304 = 8 x 38: ESI's
//!   `danglingCellRefine` splits 38 cells (32 level-1, 6 level-0) that the
//!   port leaves whole (~~NOT ported~~ **UPDATED 2026-11-12:** ported in
//!   `esi_split`, opt-in). The other 2 port-only cells are level-2
//!   cells ESI removes in `baffleAndSplitMesh` problem-cell handling
//!   (`markFacesOnProblemCells`; ~~not ported; hypothesis~~ **ported, hypothesis
//!   confirmed, 2026-11-12**). The residual of the DEFAULT removal is pinned as a
//!   known disagreement, not a pass; with `with_esi_split_mesh_removal()` the
//!   final cells equal ESI's (`round2_stages_equal_esi`,
//!   `castellate_esi_split_mesh_cells`: 18278 cells, 63194 faces, 12136 surface
//!   faces; one miss: 24646 problem faces against ESI's 25295).

use std::collections::HashSet;

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::refinement::refined_octree;
use outram_foam_mesh::snappy_hex_mesh::stl::{read_stl_ascii_str, read_stl_zoned_bytes};
use outram_foam_mesh::snappy_hex_mesh::{
    castellate, BackgroundMesh, Bounds, CastellationControls, TriangleSoup,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");
const ORG: [f64; 3] = [-0.03, -0.03, -0.03];
const SIZE0: [f64; 3] = [0.06 / 20.0, 0.06 / 16.0, 0.04 / 12.0];

/// Inflate a `gzip -n` file (10-byte header, 8-byte trailer).
fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b, "not gzip");
    assert_eq!(bytes[3], 0, "gzip -n expected");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn fixture(name: &str) -> Vec<u8> {
    gunzip(&std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture"))
}

/// Reference cells as `(level, i, j, k)`.
fn esi_cells(name: &str) -> HashSet<(usize, i64, i64, i64)> {
    let text = String::from_utf8(fixture(name)).unwrap();
    text.lines()
        .map(|l| {
            let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[0] as usize, v[1], v[2], v[3])
        })
        .collect()
}

/// Lattice key of a cell given its level and centre.
fn key(level: usize, c: Vector3) -> (usize, i64, i64, i64) {
    let one = |v: f64, a: usize| (((v - ORG[a]) / (SIZE0[a] / (1usize << level) as f64)) - 0.5).round() as i64;
    (level, one(c.x, 0), one(c.y, 1), one(c.z, 2))
}

fn controls() -> CastellationControls {
    let domain = Bounds::new(Vector3::new(-0.03, -0.03, -0.03), Vector3::new(0.03, 0.03, 0.01));
    CastellationControls::new(
        BackgroundMesh::uniform(domain, 20, 16, 12),
        2,
        Vector3::new(-9.23149e-05, -0.0025, -0.0025),
    )
    .with_surface_intersection_refinement()
    .with_baffle_region_removal()
}

fn upstream_surface() -> TriangleSoup {
    TriangleSoup::from_zoned(&read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap())
}

/// Level of a kept cell from its volume (cells are axis-aligned hexes).
fn level_of(volume: f64) -> usize {
    let v0 = SIZE0[0] * SIZE0[1] * SIZE0[2];
    ((v0 / volume).log2() / 3.0).round() as usize
}

/// PREDICTIONS (committed in the README before the run, then the misses
/// recorded there): P1 equal per-iteration counts (failed with `read_stl`,
/// holds with the upstream reader); the cell set equals ESI's mesh after
/// `surfaceOnlyRefine`.
#[test]
fn stage1_surface_only_refine_equals_esi() {
    let (cells, selected) = refined_octree(&upstream_surface(), &controls(), 100);
    assert_eq!(selected, vec![914, 4294, 344, 12, 0], "ESI log: Selected for refinement");
    let port: HashSet<_> = cells.iter().map(|(l, c)| key(*l, *c)).collect();
    assert_eq!(port.len(), cells.len(), "lattice keys are unique");
    assert_eq!(port, esi_cells("esi_stage1_cells.txt.gz"));
    assert_eq!(cells.len(), 42788);
}

/// First divergent operation 1 and 2 of the record: the plain readers.
/// `read_stl_ascii_str` now reads all four solids (it read only the first and
/// gave 758 / 3500 / 348 / 14, 36180 leaves); with double-precision corners the
/// counts are 914 / 4294 / 370 / 14 (single-precision corners give ESI's 344 /
/// 12).
#[test]
fn plain_double_precision_reader_effect_is_pinned() {
    let text = String::from_utf8(fixture("flange.stl.gz")).unwrap();
    let plain = read_stl_ascii_str(&text).unwrap();
    assert_eq!(plain.len(), 6468, "all four solids");
    assert_eq!(upstream_surface().len(), 6468);
    let (cells, selected) = refined_octree(&plain, &controls(), 100);
    assert_eq!(selected, vec![914, 4294, 370, 14, 0]);
    assert_eq!(cells.len(), 42984);
}

/// PREDICTION P8 (README) was 18012 kept cells and 304 / 38 residual; it
/// FAILED: the face-connected-region removal gives the same 18014 cells as
/// centre parity here, and ESI's extra removal of 2 cells is problem-cell
/// handling. Pinned: the measured residual against ESI's final mesh.
#[test]
fn final_cells_residual_against_esi_is_pinned() {
    let m = castellate(&upstream_surface(), &controls()).unwrap();
    assert_eq!(m.n_cells(), 18014);
    assert_eq!(m.fv_mesh.n_faces, 62644);
    let port: HashSet<_> = m
        .fv_mesh
        .cell_centres
        .iter()
        .zip(&m.fv_mesh.cell_volumes)
        .map(|(c, v)| key(level_of(*v), *c))
        .collect();
    let esi = esi_cells("esi_final_cells.txt.gz");
    assert_eq!(esi.len(), 18278);
    let only_esi: Vec<_> = esi.difference(&port).copied().collect();
    let only_port: Vec<_> = port.difference(&esi).copied().collect();
    assert_eq!(esi.intersection(&port).count(), 17974);
    assert_eq!(only_esi.len(), 304);
    assert_eq!(only_port.len(), 40);
    // Every ESI-only cell is a child of a port-only cell one level up
    // (danglingCellRefine splits it): 38 such parents x 8 children = 304.
    let parents: HashSet<_> = only_esi.iter().map(|&(l, i, j, k)| (l - 1, i >> 1, j >> 1, k >> 1)).collect();
    assert_eq!(parents.len(), 38);
    assert!(parents.iter().all(|p| port.contains(p) && !esi.contains(p)));
    // What is left: level-2 cells only the port keeps.
    let unexplained: Vec<_> = only_port.iter().filter(|c| !parents.contains(c)).collect();
    assert_eq!(unexplained.len(), 2);
    assert!(unexplained.iter().all(|c| c.0 == 2));
}

/// Cells of a stage as lattice keys.
fn keys(cells: &[(usize, Vector3)]) -> HashSet<(usize, i64, i64, i64)> {
    cells.iter().map(|(l, c)| key(*l, *c)).collect()
}

/// Round 2 of the record (P10-P14): every ESI stage after `surfaceOnlyRefine`
/// against the ESI intermediate meshes, cell for cell. P10, P11, P12 and the
/// cell set of P13 held as predicted; the problem-face count of P13 missed
/// (24646, predicted 25295; see the README).
#[test]
fn round2_stages_equal_esi() {
    let st = outram_foam_mesh::snappy_hex_mesh::esi_split::esi_stage_cells(&upstream_surface(), &controls()).unwrap();
    assert_eq!(keys(&st.after_remove_inside), esi_cells("esi_removeinside_cells.txt.gz"));
    assert_eq!(st.after_remove_inside.len(), 30264);
    assert_eq!(st.dangling_selected, [vec![22, 4, 4, 4, 2, 0], vec![2, 0]]);
    assert_eq!(keys(&st.after_dangling), esi_cells("esi_dangling_cells.txt.gz"));
    assert_eq!(st.after_dangling.len(), 30530);
    assert_eq!(st.baffle.n_hit_internal, 12145);
    assert_eq!(st.baffle.n_hit_boundary, 0);
    assert_eq!(st.baffle.n_problem_faces, 24646, "ESI: 25295 (P13 missed)");
    assert_eq!(keys(&st.final_cells), esi_cells("esi_final_cells.txt.gz"));
    assert_eq!(st.baffle.n_kept, 18278);
}

/// The same chain through `castellate` with `with_esi_split_mesh_removal()`:
/// the final mesh has ESI's 18278 cells.
#[test]
fn castellate_esi_split_mesh_cells() {
    let c = controls().with_esi_split_mesh_removal();
    let m = castellate(&upstream_surface(), &c).unwrap();
    eprintln!(
        "cells {} faces {} internal {} by level {:?}",
        m.n_cells(),
        m.fv_mesh.n_faces,
        m.fv_mesh.n_internal_faces,
        m.cells_by_level
    );
    for p in &m.fv_mesh.patches {
        eprintln!("patch {} {}", p.name, p.size);
    }
    let port: HashSet<_> = m
        .fv_mesh
        .cell_centres
        .iter()
        .zip(&m.fv_mesh.cell_volumes)
        .map(|(c, v)| key(level_of(*v), *c))
        .collect();
    assert_eq!(port, esi_cells("esi_final_cells.txt.gz"));
    // P14 held: ESI's 63194 faces and 12136 surface-patch faces.
    assert_eq!(m.fv_mesh.n_faces, 63194);
    assert_eq!(m.fv_mesh.patches.iter().map(|p| p.size).sum::<usize>(), 12136);
}

/// Round 2, task 2 (P15, P16): region patches. ESI writes `flange_patch1..4`
/// with 9310 / 1182 / 248 / 1396 faces.
#[test]
fn region_patches_against_esi() {
    let zoned = read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap();
    let names: Vec<String> = zoned.names.iter().map(|n| format!("flange_{n}")).collect();
    assert_eq!(names, ["flange_patch1", "flange_patch2", "flange_patch3", "flange_patch4"]);
    let c = controls().with_esi_split_mesh_removal().with_surface_region_patches(names.clone(), zoned.zones.clone());
    let surface = upstream_surface();
    let m = castellate(&surface, &c).unwrap();
    let counts: Vec<usize> = names
        .iter()
        .map(|n| m.fv_mesh.patches.iter().find(|p| &p.name == n).map_or(0, |p| p.size))
        .collect();
    assert_eq!(counts, vec![9310, 1182, 248, 1396], "ESI v2506 flange_patch1..4 (P15 held)");
    assert_eq!(counts.iter().sum::<usize>(), 12136);
    // P16: the cfMesh rule (region of the triangle nearest the face centre).
    let mut moved = 0usize;
    let mut cf_counts = vec![0usize; names.len()];
    for (pi, p) in m.fv_mesh.patches.iter().enumerate() {
        let Some(region) = names.iter().position(|n| n == &p.name) else { continue };
        let _ = pi;
        for f in p.start..p.start + p.size {
            let fc = m.fv_mesh.face_centres[f];
            let nearest = (0..surface.triangles.len())
                .min_by(|&a, &b| {
                    (surface.triangles[a].closest_point(fc) - fc)
                        .mag()
                        .total_cmp(&(surface.triangles[b].closest_point(fc) - fc).mag())
                })
                .unwrap();
            let r = zoned.zones[nearest];
            cf_counts[r] += 1;
            if r != region {
                moved += 1;
            }
        }
    }
    // P16 held: the cfMesh rule differs from ESI's, by 10 faces (0.08 %).
    assert_eq!(cf_counts, vec![9320, 1182, 242, 1392]);
    assert_eq!(moved, 10);
}

// ───────────────────────── Round 3 (2026-11-12) ─────────────────────────────

type Key = (usize, i64, i64, i64);

/// Lines of a gzipped fixture as integer vectors.
fn int_lines(name: &str) -> Vec<Vec<i64>> {
    String::from_utf8(fixture(name))
        .unwrap()
        .lines()
        .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
        .collect()
}

/// ESI's cell order at the mesh `markFacesOnProblemCells` runs on (time 16).
fn esi_baffle_order() -> Vec<[usize; 4]> {
    int_lines("esi_baffle_cell_order.txt.gz")
        .iter()
        .map(|v| [v[0] as usize, v[1] as usize, v[2] as usize, v[3] as usize])
        .collect()
}

/// P17 (README, round 3): visiting the 8-anchor loop in ESI's cell order the port
/// marks ESI's 25295 problem faces, the same faces; in the port's own leaf order it
/// marks 24646 (P19, pinned in `round2_stages_equal_esi`). Cell sets are equal either way.
#[test]
fn problem_faces_in_esi_cell_order_equal_esi() {
    let order = esi_baffle_order();
    assert_eq!(order.len(), 30530);
    let c = controls().with_esi_split_mesh_removal().with_esi_problem_cell_order(order);
    let st = outram_foam_mesh::snappy_hex_mesh::esi_split::esi_stage_cells(&upstream_surface(), &c).unwrap();
    assert_eq!(st.baffle.n_problem_faces, 25295, "ESI log: marked 25295 additional internal faces");
    assert_eq!(st.problem_faces.len(), 25295);
    let port: HashSet<(Vec<i64>, Vec<i64>)> = st
        .problem_faces
        .iter()
        .map(|(a, b)| (a.iter().map(|&x| x as i64).collect(), b.iter().map(|&x| x as i64).collect()))
        .collect();
    let esi: HashSet<(Vec<i64>, Vec<i64>)> =
        int_lines("esi_problem_faces.txt.gz").into_iter().map(|v| (v[..4].to_vec(), v[4..].to_vec())).collect();
    assert_eq!(esi.len(), 25295);
    assert_eq!(port, esi, "the same faces as ESI's 16/polyMesh/sets/problemFaces");
    assert_eq!(keys(&st.final_cells), esi_cells("esi_final_cells.txt.gz"));
    assert_eq!(st.baffle.n_kept, 18278);
}

/// A ring of lattice points rotated to start at its smallest point.
fn canonical_ring(mut ring: Vec<[i64; 3]>) -> Vec<[i64; 3]> {
    let m = (0..ring.len()).min_by_key(|&i| ring[i]).unwrap();
    ring.rotate_left(m);
    ring
}

/// A face as `(ring pointing from the smaller cell key to the larger, the two cells)`
/// for an internal face, `(outward ring, cell and patch)` for a boundary face.
type FaceRecord = (Vec<[i64; 3]>, String);

fn face_record(ring: Vec<[i64; 3]>, own: Key, nei: Result<Key, &str>) -> FaceRecord {
    match nei {
        Ok(nb) => {
            let mut ring = ring;
            let (a, b) = if own <= nb { (own, nb) } else { (nb, own) };
            if own > nb {
                ring.reverse();
            }
            (canonical_ring(ring), format!("{a:?}|{b:?}"))
        }
        Err(patch) => (canonical_ring(ring), format!("{own:?}|{patch}")),
    }
}

/// P18 (README, round 3): per-face equality of the final castellated mesh with
/// ESI's `32/polyMesh`: every face by its vertex ring on the level-2 lattice (up to
/// cyclic rotation), its orientation relative to the owner, its two cells, and its
/// patch. Face and cell NUMBERING is not compared (the port's order is its own).
#[test]
fn final_mesh_faces_equal_esi_per_face() {
    let zoned = read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap();
    let names: Vec<String> = zoned.names.iter().map(|n| format!("flange_{n}")).collect();
    let c = controls()
        .with_esi_split_mesh_removal()
        .with_esi_problem_cell_order(esi_baffle_order())
        .with_surface_region_patches(names.clone(), zoned.zones.clone());
    let m = castellate(&upstream_surface(), &c).unwrap();

    // ESI side.
    let esi_cell: Vec<Key> = int_lines("esi_final_cells_ordered.txt.gz")
        .iter()
        .map(|v| (v[0] as usize, v[1], v[2], v[3]))
        .collect();
    let esi_pts: Vec<[i64; 3]> = int_lines("esi_final_points.txt.gz").iter().map(|v| [v[0], v[1], v[2]]).collect();
    assert_eq!(esi_pts.len(), 26662);
    let mut esi: std::collections::HashMap<FaceRecord, usize> = std::collections::HashMap::new();
    let esi_faces = int_lines("esi_final_faces.txt.gz");
    assert_eq!(esi_faces.len(), 63194);
    for f in &esi_faces {
        let ring: Vec<[i64; 3]> = f[2..].iter().map(|&p| esi_pts[p as usize]).collect();
        let nei = if f[1] >= 0 { Ok(esi_cell[f[1] as usize]) } else { Err(names[(-1 - f[1]) as usize].as_str()) };
        *esi.entry(face_record(ring, esi_cell[f[0] as usize], nei)).or_default() += 1;
    }
    assert_eq!(esi.len(), 63194, "ESI face records are unique");

    // Port side.
    let port_cell: Vec<Key> =
        m.fv_mesh.cell_centres.iter().zip(&m.fv_mesh.cell_volumes).map(|(c, v)| key(level_of(*v), *c)).collect();
    let lat = |p: Vector3| -> [i64; 3] {
        let one = |v: f64, a: usize| {
            let q = (v - ORG[a]) / (SIZE0[a] / 4.0);
            assert!((q - q.round()).abs() < 1e-6, "point off the level-2 lattice");
            q.round() as i64
        };
        [one(p.x, 0), one(p.y, 1), one(p.z, 2)]
    };
    let t = &m.topology;
    let mut patch_of = vec![String::new(); t.faces.len()];
    for p in &m.fv_mesh.patches {
        for f in p.start..p.start + p.size {
            patch_of[f] = p.name.clone();
        }
    }
    let mut port: std::collections::HashMap<FaceRecord, usize> = std::collections::HashMap::new();
    for (fi, face) in t.faces.iter().enumerate() {
        let ring: Vec<[i64; 3]> = face.iter().map(|&p| lat(t.points[p])).collect();
        let nei = if fi < t.n_internal_faces { Ok(port_cell[t.neighbour[fi]]) } else { Err(patch_of[fi].as_str()) };
        *port.entry(face_record(ring, port_cell[t.owner[fi]], nei)).or_default() += 1;
    }
    assert_eq!(port.len(), 63194);
    let only_esi: Vec<_> = esi.keys().filter(|k| !port.contains_key(*k)).collect();
    let only_port: Vec<_> = port.keys().filter(|k| !esi.contains_key(*k)).collect();
    eprintln!("only ESI {} only port {}", only_esi.len(), only_port.len());
    {
        // Classify: same ring and cell, different patch, versus anything else.
        let sorted = |r: &Vec<[i64; 3]>| { let mut v = r.clone(); v.sort(); v };
        let port_by_ring: std::collections::HashMap<Vec<[i64; 3]>, &String> = only_port.iter().map(|r| (sorted(&r.0), &r.1)).collect();
        let mut same_ring = 0usize;
        let mut same_cell = 0usize;
        let mut pair: std::collections::BTreeMap<(String, String), usize> = Default::default();
        for r in &only_esi {
            if let Some(o) = port_by_ring.get(&sorted(&r.0)) {
                same_ring += 1;
                let cell = |s: &str| s.split('|').next().unwrap().to_string();
                if cell(&r.1) == cell(o) {
                    same_cell += 1;
                }
                let patch = |s: &str| s.split('|').nth(1).unwrap().to_string();
                *pair.entry((patch(&r.1), patch(o))).or_default() += 1;
            }
        }
        for r in only_esi.iter().take(3) { eprintln!("ESI  {:?}", r); }
        for r in only_port.iter().take(3) { eprintln!("PORT {:?}", r); }
        eprintln!("same ring {same_ring} same cell {same_cell} (esi patch, port patch) {pair:?}");
    }
    let internal_only = |v: &[&FaceRecord]| v.iter().filter(|r| !r.1.contains("flange_")).count();
    assert_eq!(internal_only(&only_esi), 0);
    assert_eq!(internal_only(&only_port), 0);
    let esi_points: HashSet<[i64; 3]> = esi_faces.iter().flat_map(|f| f[2..].iter().map(|&p| esi_pts[p as usize])).collect();
    let port_points: HashSet<[i64; 3]> = t.faces.iter().flat_map(|f| f.iter().map(|&p| lat(t.points[p]))).collect();
    assert_eq!(esi_points, port_points);
    assert_eq!(only_esi.len(), only_port.len());
    assert_eq!(only_esi.len(), 0, "P18: every face equal, patch included");
}
