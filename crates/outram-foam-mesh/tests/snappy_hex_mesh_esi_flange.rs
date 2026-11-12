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
}
