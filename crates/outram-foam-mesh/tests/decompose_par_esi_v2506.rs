// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 `decomposePar`; the frozen hashes come
// from runs of that release's `decomposePar` binary.

//! ESI v2506 `decomposePar` differential (gh:#1381): the `simple` and `hierarchical`
//! methods, the processor addressing and the per-processor `polyMesh`, byte for byte.
//!
//! **Method and results:** `verification_and_validation/decompose_par_gh1381/README.md`.
//! Three meshes written by ESI `blockMesh` at `writePrecision 17` are read from
//! `tests/fixtures/esi_decompose/<case>/constant/polyMesh` (2D cavity 20x20x1 with `empty`
//! patches, a graded 7x5x4 box, a 3-block L-shape); for each of 17 variants (method, `n`,
//! `order`, `delta`) the port writes `processorN/constant/polyMesh/*` and the FNV-1a of every
//! file is compared with the one of the file the ESI binary wrote
//! (`fnv_<case>_<variant>.txt`, made by `examples/fnv_tree.rs` from the ESI trees). A pass means
//! all 1098 files are byte-identical, headers and `note` strings included.
//!
//! Predictions D1 to D7 (committed before any run, `PREDICTIONS.md`): all held; 1098 of 1098
//! files identical on the first run. Ablation: replacing the libstdc++ introsort by a stable
//! sort breaks 2 of the 3 `delta 0` simple variants (96 files), so the tie order is load-bearing
//! and the comparison can fail.

use std::path::{Path, PathBuf};

use outram_foam_basic_lib::io::{EsiWriteOptions, PolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_mesh::decompose::{
    decompose_mesh, decompose_par, write_processor_cases, DecomposeError, GeomDecomp, GeomMethod,
};

struct Variant {
    case: &'static str,
    name: &'static str,
    method: GeomMethod,
    n: [usize; 3],
    order: &'static str,
    delta: f64,
}

const fn v(case: &'static str, name: &'static str, method: GeomMethod, n: [usize; 3], order: &'static str, delta: f64) -> Variant {
    Variant { case, name, method, n, order, delta }
}

/// The same table as `run_upstream.sh`.
fn variants() -> Vec<Variant> {
    use GeomMethod::{Hierarchical as H, Simple as S};
    vec![
        v("cavity", "simple_2_2_1", S, [2, 2, 1], "xyz", 0.001),
        v("cavity", "simple_4_1_1", S, [4, 1, 1], "xyz", 0.001),
        v("cavity", "simple_3_2_1_d0", S, [3, 2, 1], "xyz", 0.0),
        v("cavity", "hier_2_2_1_xyz", H, [2, 2, 1], "xyz", 0.001),
        v("cavity", "hier_3_2_1_yxz", H, [3, 2, 1], "yxz", 0.001),
        v("cavity", "hier_2_3_1_d0", H, [2, 3, 1], "xyz", 0.0),
        v("box3d", "simple_2_2_2", S, [2, 2, 2], "xyz", 0.001),
        v("box3d", "simple_3_2_2", S, [3, 2, 2], "xyz", 0.001),
        v("box3d", "simple_4_1_2_d0", S, [4, 1, 2], "xyz", 0.0),
        v("box3d", "hier_2_2_2_zyx", H, [2, 2, 2], "zyx", 0.001),
        v("box3d", "hier_3_2_2_xyz", H, [3, 2, 2], "xyz", 0.001),
        v("box3d", "hier_2_2_3_yzx", H, [2, 2, 3], "yzx", 0.001),
        v("box3d", "hier_4_1_2_d0", H, [4, 1, 2], "xyz", 0.0),
        v("lshape", "simple_2_2_1", S, [2, 2, 1], "xyz", 0.001),
        v("lshape", "simple_3_1_2_d0", S, [3, 1, 2], "xyz", 0.0),
        v("lshape", "hier_2_2_2_xyz", H, [2, 2, 2], "xyz", 0.001),
        v("lshape", "hier_3_2_1_yxz", H, [3, 2, 1], "yxz", 0.001),
    ]
}

fn fixtures() -> PathBuf {
    PathBuf::from(format!("{}/tests/fixtures/esi_decompose", env!("CARGO_MANIFEST_DIR")))
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn hash_tree(base: &Path, dir: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            hash_tree(base, &p, out);
        } else {
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            out.push(format!("{rel} {:016x}", fnv(&std::fs::read(&p).unwrap())));
        }
    }
}

fn temp_case(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("op_decomp_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn decomposer(var: &Variant) -> GeomDecomp {
    GeomDecomp::new(var.method, var.n).with_order(var.order).unwrap().with_delta(var.delta)
}

/// Every file of every processor of every variant is byte-identical to ESI v2506's.
#[test]
fn esi_decompose_par_is_byte_identical_on_every_variant() {
    let mut files = 0;
    for var in variants() {
        let mesh = PolyMesh::read(fixtures().join(var.case).join("constant/polyMesh")).unwrap();
        let d = decompose_par(&mesh, &decomposer(&var)).unwrap();
        let out = temp_case(&format!("{}_{}", var.case, var.name));
        write_processor_cases(&out, &mesh, &d, &EsiWriteOptions::default().with_precision(17)).unwrap();
        let mut ours = Vec::new();
        for p in 0..d.n_procs {
            hash_tree(&out, &out.join(format!("processor{p}")), &mut ours);
        }
        ours.sort();
        let frozen = std::fs::read_to_string(fixtures().join(format!("fnv_{}_{}.txt", var.case, var.name))).unwrap();
        let frozen: Vec<&str> = frozen.lines().collect();
        assert_eq!(ours.len(), frozen.len(), "{} {}: file count", var.case, var.name);
        for (a, b) in ours.iter().zip(&frozen) {
            assert_eq!(a, b, "{} {}", var.case, var.name);
        }
        files += ours.len();
        std::fs::remove_dir_all(&out).unwrap();
    }
    assert_eq!(files, 1098);
}

/// Structural invariants of the addressing, independent of ESI: every cell is on exactly one
/// processor, processor patches are mirrored (same size on both sides, neighbours ascending),
/// and the processor meshes together hold every global face once as an internal or boundary
/// face and twice as an inter-processor face.
#[test]
fn addressing_is_a_partition_with_mirrored_processor_patches() {
    for var in variants().iter().filter(|v| v.name.starts_with("hier_3_2_2") || v.name == "simple_2_2_1") {
        let mesh = PolyMesh::read(fixtures().join(var.case).join("constant/polyMesh")).unwrap();
        let d = decompose_par(&mesh, &decomposer(var)).unwrap();
        let n_cells: usize = d.procs.iter().map(|p| p.cells.len()).sum();
        assert_eq!(n_cells, mesh.n_cells);
        let mut uses = vec![0usize; mesh.n_faces()];
        for (p, a) in d.procs.iter().enumerate() {
            assert!(a.nbr_procs.windows(2).all(|w| w[0] < w[1]));
            for &f in &a.faces {
                uses[(f.unsigned_abs() - 1) as usize] += 1;
            }
            for (k, &q) in a.nbr_procs.iter().enumerate() {
                let back = d.procs[q].nbr_procs.iter().position(|&x| x == p).unwrap();
                assert_eq!(a.nbr_sizes[k], d.procs[q].nbr_sizes[back]);
            }
            let m = a.processor_mesh(&mesh, p);
            assert_eq!(m.n_cells, a.cells.len());
            assert_eq!(a.boundary_addressing(mesh.patches.len()).len(), m.patches.len());
        }
        let shared: usize = d.procs.iter().map(|a| a.nbr_sizes.iter().sum::<usize>()).sum::<usize>() / 2;
        assert!(uses.iter().all(|&u| u == 1 || u == 2));
        assert_eq!(uses.iter().filter(|&&u| u == 2).count(), shared);
    }
}

#[test]
fn unsupported_inputs_are_refused_not_guessed() {
    let mut mesh = PolyMesh::read(fixtures().join("lshape").join("constant/polyMesh")).unwrap();
    let cells = vec![0usize; mesh.n_cells];
    assert!(matches!(decompose_mesh(&mesh, &cells, 129), Err(DecomposeError::TooManyProcessors(129))));
    assert!(matches!(decompose_mesh(&mesh, &cells[1..], 2), Err(DecomposeError::BadCellToProc(_))));
    let mut bad = cells.clone();
    bad[0] = 2;
    assert!(matches!(decompose_mesh(&mesh, &bad, 2), Err(DecomposeError::BadCellToProc(_))));
    assert!(decompose_mesh(&mesh, &cells, 1).is_ok());
    let p = mesh.patches[0].clone();
    mesh.patches[0] = BoundaryPatch::new(p.name.clone(), p.start, p.size, PatchKind::Cyclic);
    let err = decompose_mesh(&mesh, &cells, 1).unwrap_err();
    assert!(matches!(err, DecomposeError::Cyclic(_)));
    assert!(err.to_string().contains("not ported"));
}

/// `write_esi_processor` refuses a neighbour list of the wrong length.
#[test]
fn processor_boundary_writer_checks_its_neighbour_list() {
    let mesh = PolyMesh::read(fixtures().join("cavity").join("constant/polyMesh")).unwrap();
    let d = decompose_par(&mesh, &GeomDecomp::new(GeomMethod::Simple, [2, 1, 1])).unwrap();
    let m = d.procs[0].processor_mesh(&mesh, 0);
    let out = temp_case("badnbr");
    let err = m.write_esi_processor(&out, "constant/polyMesh", &EsiWriteOptions::default(), 0, &[]);
    assert!(err.is_err());
    std::fs::remove_dir_all(&out).unwrap();
}
