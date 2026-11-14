//! gh:#1384: `setFields` against ESI OpenFOAM v2506, byte for byte.
//!
//! ## Methodology
//!
//! `tests/cases/esi_v2506/set_fields/case` is a 12x8x2 box (192 cells, meshed
//! by the real ESI `blockMesh`), with fields `T` and `U` and a
//! `setFieldsDict` of `defaultFieldValues` plus `boxToCell`, `sphereToCell`,
//! `cylinderToCell` and a second `boxToCell`. `T.gz`/`U.gz` are the files the
//! real ESI v2506 `setFields` wrote from it (`gzip -n`). The port runs on a
//! copy with `-flavour esi` and every written field is compared as bytes.
//! Predictions and the full record:
//! `verification_and_validation/set_fields_topo_set_extrude_gh1384.md`.
//!
//! ## Results (2026-11-14, release)
//!
//! See the V&V record; the assertions below are the pinned result.

use std::fs;
use std::path::{Path, PathBuf};

use outram_foam_cli::set_fields::run_set_fields;
use outram_foam_cli::WriteFlavour;

fn refs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/esi_v2506/set_fields")
}

/// The 6-cell variant (3x2x1): every nonuniform list is <= 10 long, so ESI
/// writes it inline as `6(a b c ...)`.
fn refs_small() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/esi_v2506/set_fields_small")
}

fn gunzip(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let raw = miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            fs::copy(e.path(), t).unwrap();
        }
    }
}

fn scratch_case(tag: &str) -> PathBuf {
    scratch_from(&refs(), tag)
}

fn scratch_from(src: &Path, tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("set_fields_{tag}"));
    let _ = fs::remove_dir_all(&dir);
    copy_dir(&src.join("case"), &dir);
    dir
}

fn first_diff(got: &str, want: &str) -> String {
    got.lines()
        .zip(want.lines())
        .position(|(a, b)| a != b)
        .map_or_else(|| "length".to_string(), |i| format!("line {}", i + 1))
}

#[test]
fn set_fields_esi_flavour_selects_the_same_cells_as_esi() {
    let case = scratch_case("counts");
    let r = run_set_fields(&case, WriteFlavour::Esi).expect("setFields");
    // ESI v2506 setFields printed: Selected 32/192, 48/192, 32/192, 32/192.
    assert_eq!(r.selected, vec![32, 48, 32, 32]);
}

#[test]
fn set_fields_esi_flavour_u_is_byte_identical_to_esi() {
    let case = scratch_case("u");
    run_set_fields(&case, WriteFlavour::Esi).expect("setFields");
    let got = fs::read_to_string(case.join("0/U")).unwrap();
    let want = gunzip(&refs().join("U.gz"));
    assert!(got == want, "U differs from ESI v2506 at {}", first_diff(&got, &want));
}

#[test]
fn set_fields_esi_flavour_t_is_byte_identical_to_esi() {
    let case = scratch_case("t");
    run_set_fields(&case, WriteFlavour::Esi).expect("setFields");
    let got = fs::read_to_string(case.join("0/T")).unwrap();
    let want = gunzip(&refs().join("T.gz"));
    assert!(got == want, "T differs from ESI v2506 at {}", first_diff(&got, &want));
}

#[test]
fn set_fields_default_flavour_runs_and_warns_about_a_missing_field() {
    let case = scratch_case("default");
    fs::remove_file(case.join("0/U")).unwrap();
    let r = run_set_fields(&case, WriteFlavour::Default).expect("setFields");
    assert!(r.log.iter().any(|l| l.contains("Field U not found")));
    let t = fs::read_to_string(case.join("0/T")).unwrap();
    assert!(t.contains("nonuniform List<scalar>"));
}

#[test]
fn set_fields_esi_flavour_small_lists_are_written_inline_like_esi() {
    let case = scratch_from(&refs_small(), "small");
    let r = run_set_fields(&case, WriteFlavour::Esi).expect("setFields");
    // ESI v2506: Selected 1/6, 0/6, 2/6, 2/6.
    assert_eq!(r.selected, vec![1, 0, 2, 2]);
    for name in ["T", "U"] {
        let got = fs::read_to_string(case.join("0").join(name)).unwrap();
        let want = gunzip(&refs_small().join(format!("{name}.gz")));
        assert!(got == want, "{name} differs from ESI v2506 at {}", first_diff(&got, &want));
    }
}
