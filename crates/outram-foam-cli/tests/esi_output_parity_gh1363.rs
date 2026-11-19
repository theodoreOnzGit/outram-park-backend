//! gh:#1363 row 7: the files the `outram-foam-cli` binaries write, byte for
//! byte against ESI OpenFOAM v2506 (`-flavour esi`).
//!
//! ## Methodology
//!
//! The references under `tests/cases/esi_v2506/` are the files written by the
//! REAL ESI v2506 `blockMesh`, `polyDualMesh` and `icoFoam` (gzipped) on the
//! shipped `incompressible/icoFoam/cavity` tutorial dictionaries (and one
//! two-zone `blockMeshDict`), with `writeFormat ascii`. The port is run on
//! the same `system/` dictionaries; every file it writes is compared with
//! the reference as raw bytes. The full procedure, the predictions made
//! before the comparison and the results (including the 457-dictionary
//! tutorial sweep) are in
//! `verification_and_validation/cli_output_vs_openfoam_gh1363.md`.
//!
//! ## Results (2026-11-12, release)
//!
//! - `blockMesh -flavour esi`, cavity: `points`, `faces`, `owner`, `neighbour`,
//!   `boundary` 5 of 5 byte-identical.
//! - `blockMesh -flavour esi`, two named zones (12 and 3 cells; short and long
//!   `cellZones` list forms): 6 of 6 byte-identical.
//! - `polyDualMesh 30 -upstream -flavour esi` on the cavity: 5 of 5 identical.
//! - `pimpleFoam -flavour esi` on the cavity with `fvSolution` tolerances
//!   tightened to 1e-14 (`relTol 0`): `U`, `phi`, `uniform/time` identical;
//!   `p` identical except the pinned reference cell (see
//!   [`p_differs_only_in_the_pinned_reference_cell`]).
//!
//! ## What does NOT match, and why (pinned below)
//!
//! - The pressure reference cell: both codes pin `p[0]` to 0 up to round-off
//!   (ESI `6.02365e-16`, port `-5.87933e-15`); the printed digits are noise.
//! - With the tutorial's own tolerances (`p` `relTol 0.05`) the port cannot
//!   follow ESI (its linear solvers have no relative tolerance, gh:#1291):
//!   most printed numbers differ.
//! - The default (non-ESI) layout differs from ESI in header text and layout
//!   by design: see [`default_layout_differs_from_esi_in_header_and_layout_only`].

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use outram_foam_appbuilder_lib::case_runner::{CaseRun, SolverKind};
use outram_foam_basic_lib::io::esi::format_g;
use outram_foam_cli::{CaseArgs, WriteFlavour};

#[allow(dead_code)]
#[path = "../src/bin/block_mesh.rs"]
mod block_mesh;

#[allow(dead_code)]
#[path = "../src/bin/ideas_unv_to_foam.rs"]
mod ideas_unv_to_foam;

#[allow(dead_code)]
#[path = "../src/bin/poly_dual_mesh.rs"]
mod poly_dual_mesh;

use poly_dual_mesh::PolyDualArgs;

fn refs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/esi_v2506")
}

/// Inflate a gzip file written by `gzip -n` (10-byte header, 8-byte trailer).
fn gunzip(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let raw = miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8])
        .expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("esi_parity_{tag}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_into(from: &Path, to: &Path, names: &[&str]) {
    fs::create_dir_all(to).unwrap();
    for n in names {
        fs::copy(from.join(n), to.join(n)).unwrap_or_else(|e| panic!("{n}: {e}"));
    }
}

/// Compare the written `polyMesh` files in `dir` with the gzipped references.
fn assert_mesh_identical(dir: &Path, reference: &Path, names: &[&str]) {
    for n in names {
        let got = fs::read_to_string(dir.join(n)).unwrap_or_else(|e| panic!("{n}: {e}"));
        let want = gunzip(&reference.join(format!("{n}.gz")));
        if got != want {
            let line = got
                .lines()
                .zip(want.lines())
                .position(|(a, b)| a != b)
                .map_or_else(|| "length".to_string(), |i| format!("line {}", i + 1));
            panic!("{n} differs from ESI v2506 at {line}");
        }
    }
}

const MESH_FILES: [&str; 5] = ["points", "faces", "owner", "neighbour", "boundary"];

#[test]
fn block_mesh_esi_flavour_is_byte_identical_to_esi_blockmesh_on_the_cavity() {
    let src = refs().join("cavity_mesh");
    let case = scratch("cavity_mesh");
    copy_into(&src, &case.join("system"), &["blockMeshDict", "controlDict"]);
    block_mesh::run_with_flavour(&CaseArgs { case: case.clone() }, WriteFlavour::Esi)
        .expect("blockMesh");
    assert_mesh_identical(&case.join("constant/polyMesh"), &src, &MESH_FILES);
}

#[test]
fn block_mesh_esi_flavour_matches_esi_for_named_zones_in_short_and_long_list_forms() {
    let src = refs().join("zones_mesh");
    let case = scratch("zones_mesh");
    copy_into(&src, &case.join("system"), &["blockMeshDict", "controlDict"]);
    block_mesh::run_with_flavour(&CaseArgs { case: case.clone() }, WriteFlavour::Esi)
        .expect("blockMesh");
    let names = ["points", "faces", "owner", "neighbour", "boundary", "cellZones"];
    assert_mesh_identical(&case.join("constant/polyMesh"), &src, &names);
}

/// `blockMesh.C` calls `IOstream::minPrecision(10)`: a `writePrecision 6`
/// controlDict still gives 10-digit points (the two-zone dict has 1/3-ish
/// coordinates: 2/12 = 0.1666666667).
#[test]
fn block_mesh_esi_writes_points_with_at_least_ten_digits() {
    let src = refs().join("zones_mesh");
    let case = scratch("min_precision");
    copy_into(&src, &case.join("system"), &["blockMeshDict", "controlDict"]);
    block_mesh::run_with_flavour(&CaseArgs { case: case.clone() }, WriteFlavour::Esi).unwrap();
    let points = fs::read_to_string(case.join("constant/polyMesh/points")).unwrap();
    assert!(points.contains("(0.1666666667 0 0)"), "{}", &points[..points.len().min(900)]);
}

#[test]
fn poly_dual_mesh_esi_flavour_is_byte_identical_to_esi_polydualmesh_on_the_cavity() {
    let src = refs().join("cavity_dual");
    let case = scratch("cavity_dual");
    copy_into(&src, &case.join("system"), &["blockMeshDict", "controlDict"]);
    block_mesh::run(&CaseArgs { case: case.clone() }).expect("blockMesh");
    let argv: Vec<std::ffi::OsString> = ["polyDualMesh", "-case", case.to_str().unwrap(), "30", "-upstream", "-flavour", "esi"]
        .iter()
        .map(std::ffi::OsString::from)
        .collect();
    let args = PolyDualArgs::parse_from(outram_foam_cli::openfoam_argv(argv));
    assert_eq!(poly_dual_mesh::write_flavour(&args).unwrap(), WriteFlavour::Esi);
    poly_dual_mesh::run_with(&args).expect("polyDualMesh");
    assert_mesh_identical(&case.join("0.005/polyMesh"), &src, &MESH_FILES);
}

#[test]
fn poly_dual_mesh_write_flavour_follows_flavour_unless_overridden() {
    let base = |extra: &[&str]| {
        let mut v: Vec<std::ffi::OsString> = vec!["polyDualMesh".into(), "30".into()];
        v.extend(extra.iter().map(std::ffi::OsString::from));
        PolyDualArgs::parse_from(outram_foam_cli::openfoam_argv(v))
    };
    assert_eq!(poly_dual_mesh::write_flavour(&base(&[])).unwrap(), WriteFlavour::Default);
    assert_eq!(
        poly_dual_mesh::write_flavour(&base(&["-flavour", "esi"])).unwrap(),
        WriteFlavour::Esi
    );
    assert_eq!(
        poly_dual_mesh::write_flavour(&base(&["-flavour", "esi", "-writeFlavour", "default"]))
            .unwrap(),
        WriteFlavour::Default
    );
    assert!(poly_dual_mesh::write_flavour(&base(&["-writeFlavour", "foundation"])).is_err());
}

/// The default layout is kept callable and unchanged in kind: it differs from
/// ESI in exactly the documented ways (banner text, no `arch`/`note`/`inGroups`,
/// blank-line layout, shortest round-trip numbers) and in nothing else: the
/// connectivity DATA of `faces`, `owner`, `neighbour` is the same.
#[test]
fn default_layout_differs_from_esi_in_header_and_layout_only() {
    let src = refs().join("cavity_mesh");
    let case = scratch("default_layout");
    copy_into(&src, &case.join("system"), &["blockMeshDict", "controlDict"]);
    block_mesh::run(&CaseArgs { case: case.clone() }).expect("blockMesh");
    let dir = case.join("constant/polyMesh");
    for n in MESH_FILES {
        let got = fs::read_to_string(dir.join(n)).unwrap();
        let want = gunzip(&src.join(format!("{n}.gz")));
        assert_ne!(got, want, "{n}: the default layout is not ESI's");
        assert!(got.contains("OUTRAM PARK"), "{n}: default banner names this project");
        assert!(!got.contains("arch "), "{n}: no arch entry in the default layout");
        assert!(want.contains("arch        \"LSB;label=32;scalar=64\""));
    }
    // integer data lines agree after dropping header, blanks and footer
    let data = |s: &str| -> Vec<String> {
        s.lines()
            .skip_while(|l| !l.starts_with("//"))
            .skip(1)
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .map(str::to_string)
            .collect()
    };
    for n in ["faces", "owner", "neighbour"] {
        let got = fs::read_to_string(dir.join(n)).unwrap();
        assert_eq!(data(&got), data(&gunzip(&src.join(format!("{n}.gz")))), "{n}");
    }
}

#[test]
fn flavour_words_parse_and_unknown_ones_are_refused() {
    assert_eq!(WriteFlavour::parse("esi").unwrap(), WriteFlavour::Esi);
    assert_eq!(WriteFlavour::parse("default").unwrap(), WriteFlavour::Default);
    assert_eq!(WriteFlavour::parse("dev").unwrap(), WriteFlavour::Default);
    assert!(WriteFlavour::parse("openfoam9").is_err());
}

#[test]
fn write_precision_reads_the_controldict_and_defaults_to_six() {
    let case = scratch("precision");
    assert_eq!(outram_foam_cli::write_precision(&case), 6);
    fs::create_dir_all(case.join("system")).unwrap();
    fs::write(
        case.join("system/controlDict"),
        "FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }\nwritePrecision 17;\n",
    )
    .unwrap();
    assert_eq!(outram_foam_cli::write_precision(&case), 17);
    assert_eq!(format_g(0.1 + 0.2, 17), "0.30000000000000004");
}

// ── pimpleFoam fields ───────────────────────────────────────────────────────

/// The cavity case: the shipped tutorial dictionaries, with `fvSolution`
/// tolerances tightened to 1e-14 and `relTol 0` (the row-1 protocol), and the
/// mesh written by the port's own `blockMesh -flavour esi`.
fn run_tight_cavity(tag: &str) -> PathBuf {
    let src = refs().join("cavity_tight");
    let case = scratch(tag);
    copy_into(&src.join("system"), &case.join("system"),
        &["blockMeshDict", "controlDict", "fvSchemes", "fvSolution"]);
    copy_into(&src.join("constant"), &case.join("constant"), &["transportProperties"]);
    copy_into(&src.join("0"), &case.join("0"), &["U", "p"]);
    block_mesh::run_with_flavour(&CaseArgs { case: case.clone() }, WriteFlavour::Esi).unwrap();
    let mut run = CaseRun::from_case(&case, SolverKind::PimpleFoam).expect("case");
    while !run.is_done() {
        run.step().expect("step");
    }
    outram_foam_cli::write_run_fields(&run, &case, WriteFlavour::Esi).expect("write");
    case
}

#[test]
fn pimple_foam_esi_flavour_matches_icofoam_for_u_phi_and_uniform_time() {
    let case = run_tight_cavity("pimple_tight");
    let want = refs().join("cavity_tight/expected_0.5");
    for (got, reference) in [("U", "U.gz"), ("phi", "phi.gz"), ("uniform/time", "uniform/time.gz")] {
        let g = fs::read_to_string(case.join("0.5").join(got)).unwrap();
        assert_eq!(g, gunzip(&want.join(reference)), "{got} differs from ESI icoFoam");
    }
}

/// `p` is pinned to 0 at `pRefCell 0` by both codes only up to round-off:
/// ESI prints `6.02365e-16`, the port `-5.87933e-15`. Every other line must be
/// identical; the lines that differ must be near zero on BOTH sides.
#[test]
fn p_differs_only_in_the_pinned_reference_cell() {
    let case = run_tight_cavity("pimple_p");
    let got = fs::read_to_string(case.join("0.5/p")).unwrap();
    let want = gunzip(&refs().join("cavity_tight/expected_0.5/p.gz"));
    assert_eq!(got.lines().count(), want.lines().count());
    let mut differing = Vec::new();
    for (i, (a, b)) in got.lines().zip(want.lines()).enumerate() {
        if a != b {
            differing.push((i, a.to_string(), b.to_string()));
        }
    }
    assert!(differing.len() <= 1, "more than the reference cell differs: {differing:?}");
    for (i, a, b) in &differing {
        let (x, y): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        assert!(x.abs() < 1e-12 && y.abs() < 1e-12, "line {}: {a} vs {b}", i + 1);
    }
}

/// The three defects found by running the shipped dictionaries (fixed in this
/// change): `constant/transportProperties` was ignored (ν fell back to the
/// solver default), `object` of `U` was written as `HbyA`, and `$p;` in
/// `fvSolution` made the case unreadable.
#[test]
fn the_written_u_is_named_u_and_the_shipped_dictionaries_are_read() {
    let case = run_tight_cavity("pimple_defects");
    let u = fs::read_to_string(case.join("0.5/U")).unwrap();
    assert!(u.contains("object      U;"), "U must not be written as HbyA");
    let run = CaseRun::from_case(&case, SolverKind::PimpleFoam).unwrap();
    match &run.solver {
        outram_foam_appbuilder_lib::case_runner::SolverState::PimpleFoam(s) => {
            assert!((s.nu.internal.as_slice()[0] - 0.01).abs() < 1e-15, "nu from transportProperties");
            assert_eq!(s.p_linear_solver.tolerance, 1e-14);
            assert_eq!(s.u_linear_solver.tolerance, 1e-14);
            assert_eq!(s.schemes.default_div, outram_foam_appbuilder_lib::io::fv_schemes::DivScheme::GaussLinear);
        }
        _ => panic!("pimpleFoam"),
    }
}


/// A non-zero `relTol` (the tutorial's `p`, gh:#1291) is not representable by
/// this port's absolute-tolerance linear solvers: the entry is skipped and the
/// solver's own default stays; an entry with `relTol 0` is read. Pinned so the
/// limitation cannot disappear silently.
#[test]
fn a_nonzero_reltol_is_not_representable_and_leaves_the_default_tolerance() {
    let case = scratch("reltol");
    let src = refs().join("cavity_tight");
    copy_into(
        &src.join("system"),
        &case.join("system"),
        &["blockMeshDict", "controlDict", "fvSchemes"],
    );
    fs::write(
        case.join("system/fvSolution"),
        "FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }\n\
         solvers { p { solver PCG; tolerance 1e-06; relTol 0.05; } pFinal { $p; } \
         U { solver smoothSolver; tolerance 1e-05; relTol 0; } }\n\
         PISO { nCorrectors 2; }\n",
    )
    .unwrap();
    copy_into(&src.join("constant"), &case.join("constant"), &["transportProperties"]);
    copy_into(&src.join("0"), &case.join("0"), &["U", "p"]);
    block_mesh::run(&CaseArgs { case: case.clone() }).unwrap();
    let run = CaseRun::from_case(&case, SolverKind::PimpleFoam).unwrap();
    match &run.solver {
        outram_foam_appbuilder_lib::case_runner::SolverState::PimpleFoam(s) => {
            assert_eq!(s.p_linear_solver.tolerance, 1e-8, "p keeps the solver default");
            assert_eq!(s.u_linear_solver.tolerance, 1e-5, "U relTol 0 is read");
        }
        _ => panic!("pimpleFoam"),
    }
}

/// Found by the 449-dictionary sweep (gh:#1363 row 7): the TJunction tutorials
/// declare `defaultFaces { type wall; faces (); }`. ESI gives that patch the
/// unassigned faces (one patch, type wall); the port wrote it empty AND a
/// second `defaultFaces` of type empty.
#[test]
fn a_declared_default_faces_patch_takes_the_unassigned_faces() {
    let case = scratch("declared_default");
    fs::create_dir_all(case.join("system")).unwrap();
    fs::write(
        case.join("system/blockMeshDict"),
        "convertToMeters 1;\n\
         vertices ((0 0 0) (1 0 0) (1 1 0) (0 1 0) (0 0 1) (1 0 1) (1 1 1) (0 1 1));\n\
         blocks (hex (0 1 2 3 4 5 6 7) (2 2 2) simpleGrading (1 1 1));\n\
         boundary ( inlet { type patch; faces ((0 4 7 3)); } \
         defaultFaces { type wall; faces (); } );\n",
    )
    .unwrap();
    block_mesh::run(&CaseArgs { case: case.clone() }).unwrap();
    let mesh = outram_foam_basic_lib::io::poly_mesh::PolyMesh::read(case.join("constant/polyMesh"))
        .unwrap();
    let names: Vec<&str> = mesh.patches.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["inlet", "defaultFaces"]);
    assert_eq!(mesh.patches[0].size, 4);
    assert_eq!(mesh.patches[1].size, 20, "5 faces of 4 cells each");
    assert_eq!(
        mesh.patches[1].kind,
        outram_foam_basic_lib::mesh::PatchKind::Wall,
        "the declared type wins over the default `empty`"
    );
}

/// `ideasUnvToFoam -flavour esi` writes ESI's file layout. The CONTENT of the
/// conversion is not claimed equal to ESI's (their point/face numbering and
/// face-group handling differ, see the V&V record); this pins the layout only:
/// banner, `arch`, `note`, the short-list form for a single hex's 6 faces.
#[test]
fn ideas_unv_to_foam_esi_flavour_writes_esi_layout() {
    let case = scratch("unv_esi");
    fs::create_dir_all(&case).unwrap();
    let unv = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/unv/cube.unv");
    ideas_unv_to_foam::run_with_unv_flavour(
        &CaseArgs { case: case.clone() },
        Some(unv),
        WriteFlavour::Esi,
    )
    .expect("ideasUnvToFoam");
    let owner = fs::read_to_string(case.join("constant/polyMesh/owner")).unwrap();
    assert!(owner.contains("Version:  2506"));
    assert!(owner.contains("arch        \"LSB;label=32;scalar=64\";"));
    assert!(owner.contains("note        \"nPoints:8  nCells:1  nFaces:6  nInternalFaces:0\";"));
    assert!(owner.contains("\n6{0}\n"), "uniform owner list: {owner}");
    let neighbour = fs::read_to_string(case.join("constant/polyMesh/neighbour")).unwrap();
    assert!(neighbour.contains("\n0()\n"), "empty neighbour list: {neighbour}");
}
