//! COVR library-option verification against NJOY2016 (`covr.f90`).
//!
//! Oracle: the upstream Fortran NJOY2016 (`ac5adf5`, 2016.79, gfortran
//! 13.3.0) `covr` run in the library option on each of the nine ERRORR
//! covariance tapes in `reference-data/errorr/` (themselves NJOY output), for
//! both output matrix types, with the decks in
//! `reference-data/covr/*.njoy-input`:
//!
//! ```text
//! covr
//! 20 21 0/            nin nout nplot      (nout>0: library option)
//! <matype> 1/         3=covariances / 4=correlations, ncase
//! 'w4cov'/            hinpid
//! 'oracle covr w4'/   hdescr
//! <mat> 0 0 0/        mat mt mat1 mt1     (mt=0: every pair, expndo)
//! ```
//!
//! `tape21` (the BOXER library) is the golden file.
//!
//! # Tiers (the pattern of `errorr_mf33_golden.rs`)
//!
//! **Tier 1 — engine isolation.** Input = NJOY's own ERRORR tape, so every
//! difference is COVR logic: the tape reader (`covard`), `expndo`, `corr`'s
//! rsd sourcing, the `press` run-length encoder and its Fortran text layout.
//! Prediction: **byte-identical** output — the encoder is deterministic on
//! `sigfig`-rounded values and the edit descriptors are fixed — so the test
//! asserts `text == golden` and, on failure, names the first differing line.
//!
//! **Tier 2 — end to end.** Input = the crate's own ERRORR run on NJOY's
//! PENDF (`errorr_mf33_golden.rs` tier 1: agrees with NJOY's tape to
//! 4.9e-7), written with `ErrorrResult::to_tape` + `Tape::write` and read
//! back with `Tape::read` — the real tape byte path — then COVR. A 4.9e-7
//! input difference can flip the 4th significant figure `press` keeps, which
//! also reshapes the run-length stream, so bytes are *reported* and values
//! are *asserted*: both BOXER texts are decoded and every element of every
//! record agrees within one unit of its printed precision (relative 2e-3 for
//! the `1p e10.3` fields, absolute 1.5e-4 for the `f7.4` correlations) above
//! the ERRORR test's own 1e-12 absolute floor.
//!
//! # Materials
//!
//! The nine ERRORR tapes (H-2 twice — relative and absolute covariances —
//! Be-9, Li-6, C-12, F-19, Si-30, U-234, U-238). U-238's 11 reactions
//! include the lumped `MT=851/852`, giving 66 pairs and multi-page (`nrowm >
//! 0`) records; Si-30 has 122 null pairs suppressed; the absolute H-2 tape
//! exercises the "do not translate to relative in the library option" rule.

use std::collections::BTreeMap;
use std::path::Path;

use njoy_outram_park_fork::covr::{
    decompress, parse_boxer_text, run_library, BoxerDataType, LibraryOptions, MatrixOutputType,
    ReactionSelector,
};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::{run_mf33, ErrorrWeight, Mf33Config};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const TEMP_K: f64 = 293.6;
const IGN: i32 = 3;
const HINPID: &str = "w4cov";
const HDESCR: &str = "oracle covr w4";
const TIER2_REL: f64 = 2e-3;
const TIER2_ABS_CORR: f64 = 1.5e-4;
/// `errorr_mf33_golden.rs`'s absolute floor: below it NJOY's tape holds
/// summation noise (U-238 carries `e-49`..`e-69` elements) that no port
/// reproduces bit-for-bit, and `press` encodes each such value verbatim.
const TIER2_ABS: f64 = 1e-12;

struct Case {
    label: &'static str,
    endf: &'static str,
    mat: i32,
    /// `<name>.errorr` under `reference-data/errorr/`; `<name>-matype<k>.boxer`
    /// under `reference-data/covr/`.
    name: &'static str,
    pendf: PendfSource,
    iwt: i32,
    irelco: i32,
}

enum PendfSource {
    Committed(&'static str),
    Env(&'static str),
}

const CASES: &[Case] = &[
    Case {
        label: "h2-iwt6-rel",
        endf: "n-001_H_002-ENDF8.0.endf",
        mat: 128,
        name: "h2-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("h2-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "h2-iwt3-abs",
        endf: "n-001_H_002-ENDF8.0.endf",
        mat: 128,
        name: "h2-ENDF8.0-293.6K-ign3-iwt3-abs",
        pendf: PendfSource::Committed("h2-ENDF8.0-293.6K"),
        iwt: 3,
        irelco: 0,
    },
    Case {
        label: "be9",
        endf: "n-004_Be_009-ENDF8.0.endf",
        mat: 425,
        name: "be9-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("be9-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "li6",
        endf: "n-003_Li_006-ENDF8.0.endf",
        mat: 325,
        name: "li6-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("li6-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "c12",
        endf: "n-006_C_012-ENDF8.0.endf",
        mat: 625,
        name: "c12-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("c12-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "f19",
        endf: "n-009_F_019-ENDF8.0.endf",
        mat: 925,
        name: "f19-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("f19-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "si30",
        endf: "n-014_Si_030-ENDF8.0.endf",
        mat: 1431,
        name: "si30-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Committed("si30-ENDF8.0-293.6K"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "u234",
        endf: "n-092_U_234-ENDF8.0.endf",
        mat: 9225,
        name: "u234-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Env("OUTRAM_PARK_NJOY_U234_PENDF"),
        iwt: 6,
        irelco: 1,
    },
    Case {
        label: "u238",
        endf: "n-092_U_238.endf",
        mat: 9237,
        name: "u238-ENDF8.0-293.6K-ign3-iwt6-rel",
        pendf: PendfSource::Env("OUTRAM_PARK_NJOY_U238_PENDF"),
        iwt: 6,
        irelco: 1,
    },
];

fn case(label: &str) -> &'static Case {
    CASES.iter().find(|c| c.label == label).unwrap()
}

fn options(matype: i32) -> LibraryOptions {
    LibraryOptions {
        matrix_type: MatrixOutputType::from_matype(matype),
        lib_id: HINPID.into(),
        description: HDESCR.into(),
    }
}

fn all_pairs(mat: i32) -> Vec<ReactionSelector> {
    vec![ReactionSelector {
        mat,
        mt: 0,
        mat1: 0,
        mt1: 0,
    }]
}

fn golden_text(case: &Case, matype: i32) -> Option<String> {
    let p = reference_file_or_skip(
        "covr",
        &format!("{}-matype{matype}.boxer", case.name),
        "covr-boxer-golden",
    )?;
    Some(std::fs::read_to_string(p).expect("golden BOXER reads"))
}

/// First differing line, for a useful failure message.
fn first_diff(got: &str, want: &str) -> Option<(usize, String, String)> {
    got.lines()
        .zip(want.lines())
        .enumerate()
        .find(|(_, (g, w))| g != w)
        .map(|(i, (g, w))| (i + 1, g.to_string(), w.to_string()))
        .or_else(|| {
            let (ng, nw) = (got.lines().count(), want.lines().count());
            (ng != nw).then(|| {
                (
                    ng.min(nw) + 1,
                    format!("<{ng} lines>"),
                    format!("<{nw} lines>"),
                )
            })
        })
}

// ---------------------------------------------------------------------------
// Tier 1
// ---------------------------------------------------------------------------

fn tier1(case: &Case) {
    let Some(errorr_path) = reference_file_or_skip(
        "errorr",
        &format!("{}.errorr", case.name),
        "covr-boxer-golden",
    ) else {
        return;
    };
    let tape = Tape::read_file(&errorr_path).expect("NJOY ERRORR tape parses");
    for matype in [3, 4] {
        let tag = format!("tier1 {} matype={matype}", case.label);
        let Some(want) = golden_text(case, matype) else {
            return;
        };
        let t0 = std::time::Instant::now();
        let out = run_library(&tape, &options(matype), &all_pairs(case.mat)).expect("run_library");
        let written = out.pairs.iter().filter(|p| p.written).count();
        println!(
            "[{tag}] {} pairs, {written} written, {} suppressed, {} lines, {:.2?}",
            out.pairs.len(),
            out.pairs.len() - written,
            out.text.lines().count(),
            t0.elapsed()
        );
        if let Some((line, g, w)) = first_diff(&out.text, &want) {
            panic!("{tag}: first difference at line {line}:\n  got:  {g:?}\n  njoy: {w:?}");
        }
        assert_eq!(out.text, want, "{tag}: byte-identical BOXER library");
    }
}

#[test]
fn covr_tier1_h2_relative() {
    tier1(case("h2-iwt6-rel"));
}
#[test]
fn covr_tier1_h2_absolute() {
    tier1(case("h2-iwt3-abs"));
}
#[test]
fn covr_tier1_be9() {
    tier1(case("be9"));
}
#[test]
fn covr_tier1_li6() {
    tier1(case("li6"));
}
#[test]
fn covr_tier1_c12() {
    tier1(case("c12"));
}
#[test]
fn covr_tier1_f19() {
    tier1(case("f19"));
}
#[test]
fn covr_tier1_si30() {
    tier1(case("si30"));
}
#[test]
fn covr_tier1_u234() {
    tier1(case("u234"));
}
#[test]
fn covr_tier1_u238() {
    tier1(case("u238"));
}

// ---------------------------------------------------------------------------
// Tier 2
// ---------------------------------------------------------------------------

type RecordKey = (i32, i32, i32, i32, i32);

fn decode_all(text: &str) -> BTreeMap<RecordKey, Vec<f64>> {
    parse_boxer_text(text)
        .expect("BOXER parses")
        .into_iter()
        .map(|r| {
            let h = &r.header;
            let key = (h.itype.code(), h.mat, h.mt, h.mat1, h.mt1);
            (key, decompress(&r.data).expect("BOXER decodes"))
        })
        .collect()
}

fn pendf_path(case: &Case) -> Option<std::path::PathBuf> {
    match case.pendf {
        PendfSource::Committed(name) => {
            reference_file_or_skip("errorr", &format!("{name}.pendf"), "covr-boxer-golden")
        }
        PendfSource::Env(var) => match std::env::var(var) {
            Ok(p) => Some(std::path::PathBuf::from(p)),
            Err(_) => {
                println!(
                    "[tier2 {}] SKIP: set {var} to NJOY's 293.6 K PENDF",
                    case.label
                );
                None
            }
        },
    }
}

fn tier2(case: &Case) {
    let Some(endf_path) = reference_endf_or_skip(case.endf, "covr-boxer-golden") else {
        return;
    };
    let Some(pendf_path) = pendf_path(case) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let pendf = Tape::read_file(Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let cfg = Mf33Config {
        matd: case.mat,
        ign: IGN,
        user_egn: None,
        weight: ErrorrWeight::from_iwt(case.iwt, None, None).unwrap(),
        tempin: TEMP_K,
        irelco: case.irelco,
        dap: 0.0,
    };
    let result = run_mf33(&endf, &pendf, &cfg).expect("run_mf33");
    // The real tape byte path: write the ERRORR tape, read it back.
    let mut bytes = Vec::new();
    result
        .to_tape()
        .write(&mut bytes)
        .expect("ERRORR tape writes");
    let tape = Tape::read(bytes.as_slice()).expect("crate ERRORR tape parses");

    for matype in [3, 4] {
        let tag = format!("tier2 {} matype={matype}", case.label);
        let Some(want) = golden_text(case, matype) else {
            return;
        };
        let out = run_library(&tape, &options(matype), &all_pairs(case.mat)).expect("run_library");
        let same_lines = out
            .text
            .lines()
            .zip(want.lines())
            .filter(|(g, w)| g == w)
            .count();
        let got = decode_all(&out.text);
        let exp = decode_all(&want);
        let got_keys: Vec<_> = got.keys().collect();
        let exp_keys: Vec<_> = exp.keys().collect();
        assert_eq!(
            got_keys, exp_keys,
            "{tag}: record set (itype,mat,mt,mat1,mt1)"
        );
        let mut worst = (0.0f64, String::new());
        let mut elements = 0usize;
        for (key, want_vals) in &exp {
            let got_vals = &got[key];
            assert_eq!(got_vals.len(), want_vals.len(), "{tag}: {key:?} shape");
            let floor = if key.0 == BoxerDataType::Correlation.code() {
                TIER2_ABS_CORR
            } else {
                TIER2_ABS
            };
            for (k, (&g, &w)) in got_vals.iter().zip(want_vals).enumerate() {
                elements += 1;
                let scale = g.abs().max(w.abs());
                let excess = ((g - w).abs() - floor).max(0.0);
                let rel = if scale > 0.0 { excess / scale } else { 0.0 };
                if rel > worst.0 {
                    worst = (rel, format!("{key:?}[{k}] got {g:.4e} njoy {w:.4e}"));
                }
            }
        }
        println!(
            "[{tag}] {} records, {elements} elements, {same_lines}/{} lines byte-identical, worst {:.3e} at {}",
            exp.len(),
            want.lines().count(),
            worst.0,
            worst.1
        );
        assert!(
            worst.0 <= TIER2_REL,
            "{tag}: worst deviation {:.3e} at {}",
            worst.0,
            worst.1
        );
    }
}

#[test]
fn covr_tier2_h2_relative() {
    tier2(case("h2-iwt6-rel"));
}
#[test]
fn covr_tier2_h2_absolute() {
    tier2(case("h2-iwt3-abs"));
}
#[test]
fn covr_tier2_be9() {
    tier2(case("be9"));
}
#[test]
fn covr_tier2_li6() {
    tier2(case("li6"));
}
#[test]
fn covr_tier2_c12() {
    tier2(case("c12"));
}
#[test]
fn covr_tier2_f19() {
    tier2(case("f19"));
}
#[test]
fn covr_tier2_si30() {
    tier2(case("si30"));
}
#[test]
fn covr_tier2_u234() {
    tier2(case("u234"));
}
#[test]
fn covr_tier2_u238() {
    tier2(case("u238"));
}
