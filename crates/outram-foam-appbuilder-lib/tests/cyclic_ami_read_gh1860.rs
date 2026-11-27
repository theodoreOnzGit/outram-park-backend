// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1860: `read_poly_mesh` couples `cyclicAMI` patch pairs.
//!
//! **Methodology.** Two unit boxes side by side, meshed by the blockMesh port
//! as two blocks with no shared vertex: block A at 2 x 2 x 2, block B at
//! 3 x 3 x 3. Their facing sides (x = 1) are the patches `a_to_b` and
//! `b_to_a`, written as `cyclicAMI` with each other as `neighbourPatch`.
//! The polyMesh is read back.
//!
//! **Pass criteria.** Every face of the first half (`a_to_b`, 4 faces) has
//! one AMI coupling to the other half's cells, and the second half none:
//! the operators stamp a coupling as a full internal face, so one side
//! carries the seam (both sides counted it twice, 2026-11-27). Weights sum to
//! 1 to 1e-12 (the halves cover the same unit square), overlap areas sum to
//! the face's area, and the overlap areas sum to the seam's area (1 m²). A `neighbourPatch` that does not name its partner back
//! is refused.
//!
//! **Results (2026-11-27).** 4 couplings; worst weight-sum error and area
//! error printed by the test, round-off (1.1e-16 and 5.6e-17 m² when both
//! sides were coupled); the one-sided `neighbourPatch` is refused.

use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::mesh::PatchKind;

const DICT: &str = r#"
scale 1;
vertices
(
    (0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1)
    (1 0 0) (2 0 0) (2 1 0) (1 1 0) (1 0 1) (2 0 1) (2 1 1) (1 1 1)
);
blocks
(
    hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1)
    hex (8 9 10 11 12 13 14 15) (3 3 3) simpleGrading (1 1 1)
);
boundary
(
    a_to_b { type patch; faces ((1 2 6 5)); }
    b_to_a { type patch; faces ((8 12 15 11)); }
    walls { type wall; faces (
        (0 4 7 3) (0 1 5 4) (3 7 6 2) (0 3 2 1) (4 5 6 7)
        (9 10 14 13) (8 9 13 12) (11 15 14 10) (8 11 10 9) (12 13 14 15)
    ); }
);
"#;

/// Write the two-block mesh with `a_to_b` / `b_to_a` as cyclicAMI, naming
/// `nbr_a` and `nbr_b` as their neighbours, and return the directory.
fn write_case(tag: &str, nbr_a: &str, nbr_b: &str) -> std::path::PathBuf {
    let mut poly = outram_foam_mesh::block_mesh::block_mesh(DICT).expect("blockMesh").to_foam_poly_mesh();
    for p in &mut poly.patches {
        if p.name == "a_to_b" || p.name == "b_to_a" {
            p.kind = PatchKind::CyclicAmi;
        }
    }
    let dir = std::env::temp_dir().join(format!("cyclic_ami_read_{tag}_{}", std::process::id()));
    poly.write(&dir).expect("write");
    let path = dir.join("boundary");
    let mut text = std::fs::read_to_string(&path).unwrap();
    for (me, nbr) in [("a_to_b", nbr_a), ("b_to_a", nbr_b)] {
        let at = text.find(&format!("    {me}\n")).expect("patch block");
        let close = at + text[at..].find("    }").unwrap();
        text.insert_str(close, &format!("        neighbourPatch  {nbr};\n"));
    }
    std::fs::write(&path, text).unwrap();
    dir
}

#[test]
fn cyclic_ami_pairs_are_coupled_on_read() {
    let dir = write_case("ok", "b_to_a", "a_to_b");
    let mesh = read_poly_mesh(&dir).expect("reads");
    let ia = mesh.patches.iter().position(|p| p.name == "a_to_b").unwrap();
    let ib = mesh.patches.iter().position(|p| p.name == "b_to_a").unwrap();
    assert_eq!(mesh.patches[ia].kind, PatchKind::CyclicAmi);
    assert_eq!((mesh.patches[ia].size, mesh.patches[ib].size), (4, 9));
    assert_eq!(mesh.ami_couplings.len(), 4);
    assert!(mesh.ami_couplings.iter().all(|c| c.target_patch == ia && c.source_patch == ib));
    let seam: f64 = mesh.ami_couplings.iter().flat_map(|c| c.weights.iter()).map(|w| w.overlap_area).sum();
    assert!((seam - 1.0).abs() < 1e-12, "seam area {seam}");
    let (mut worst_w, mut worst_a) = (0.0f64, 0.0f64);
    for c in &mesh.ami_couplings {
        let other = if c.target_patch == ia { ib } else { ia };
        assert_eq!(c.source_patch, other);
        let sum: f64 = c.weights.iter().map(|w| w.weight).sum();
        let area: f64 = c.weights.iter().map(|w| w.overlap_area).sum();
        worst_w = worst_w.max((sum - 1.0).abs());
        worst_a = worst_a.max((area - mesh.face_areas[c.target_face]).abs());
        for w in &c.weights {
            let p = &mesh.patches[other];
            assert!(w.source_face >= p.start && w.source_face < p.start + p.size);
        }
    }
    eprintln!("worst weight-sum error {worst_w:e}, worst area error {worst_a:e}");
    assert!(worst_w < 1e-12 && worst_a < 1e-12);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_neighbour_patch_that_does_not_name_back_is_refused() {
    let dir = write_case("bad", "b_to_a", "walls");
    let err = read_poly_mesh(&dir).err().expect("refused");
    assert!(format!("{err}").contains("does not name it back"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
