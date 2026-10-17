//! **The browser probe's work, run natively**: the fingerprints a
//! headless-Chromium run of `examples/nd_wasm_probe` must reproduce
//! (gh #935, #936; epic #925's wasm criterion). Same functions
//! (`examples/nd_wasm_probe/probe.rs`, included below), same input bytes.
//!
//! # Methodology
//!
//! - **Input**: four ENDF/B-VIII.0 tapes from `reference-data/endf`
//!   (F-19, U-234, U-235, U-238) named as NNDC's `ENDF-B-VIII.0_neutrons.zip`
//!   names them (`ENDF-B-VIII.0_neutrons/n-092_U_235.endf`), plus a
//!   `CHANGELOG.txt`; and NJOY2016's U-234 ACE table at 293.6 K
//!   (`reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz`).
//! - **Tasks**, as the page runs them: (1) zip the tapes with
//!   `archive::write_zip` and index the zip (`EndfLibraryIndex`); (2)
//!   generate F-19's ACE at 293.6 K with its 0 K companion from the zip and
//!   read both back by header (`AceSet`); (3) Godiva through
//!   `NuclearData::from_zip_bytes` and the beginner model, 200 neutrons ×
//!   [2 + 3] cycles, seed 5; (4) the U-234 ACE file indexed, its XSS
//!   fingerprinted, and built into a nuclide through
//!   `NuclearData::from_ace_files`.
//! - **Pass**: every fingerprint equals the value pinned below, which is
//!   what the native run printed; the browser record compares the same keys.
//!
//! # Results (2026-10-17)
//!
//! **Pass**, natively and in headless Chromium 154 alike: zip
//! `0xdbbc098a42d96f62`, index `0x0e80455b35bda55c`, generated ACE
//! `0xd2477939bbaba20a`, Godiva k trace `0xcedab1315589be07`, ACE XSS
//! `0x9a9dbebd27900700`. Record:
//! `verification_and_validation/nuclear_data_loading_web/README.md`.

#[path = "../examples/nd_wasm_probe/probe.rs"]
#[allow(dead_code)]
mod probe;

use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The page's file list: `(name in the zip, path under the repo root)`.
pub const ENDF_FILES: [(&str, &str); 5] = [
    (
        "ENDF-B-VIII.0_neutrons/CHANGELOG.txt",
        "reference-data/endf/README.md",
    ),
    (
        "ENDF-B-VIII.0_neutrons/n-009_F_019.endf",
        "reference-data/endf/n-009_F_019-ENDF8.0.endf",
    ),
    (
        "ENDF-B-VIII.0_neutrons/n-092_U_234.endf",
        "reference-data/endf/n-092_U_234-ENDF8.0.endf",
    ),
    (
        "ENDF-B-VIII.0_neutrons/n-092_U_235.endf",
        "reference-data/endf/n-092_U_235-ENDF8.0.endf",
    ),
    (
        "ENDF-B-VIII.0_neutrons/n-092_U_238.endf",
        "reference-data/endf/n-092_U_238.endf",
    ),
];

const ACE_FILE: (&str, &str) = (
    "293.6K/U234.ace.gz",
    "reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz",
);

fn endf_files() -> Option<Vec<(String, Vec<u8>)>> {
    ENDF_FILES
        .iter()
        .map(|(name, path)| {
            std::fs::read(root().join(path))
                .ok()
                .map(|b| (name.to_string(), b))
        })
        .collect()
}

fn value<'a>(report: &'a str, key: &str) -> &'a str {
    report
        .lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .unwrap_or_else(|| panic!("no {key} in {report}"))
}

/// Native fingerprints, x86_64 Linux, release, 2026-10-17.
const ZIP_FNV: &str = "0xdbbc098a42d96f62";
const INDEX_FNV: &str = "0x0e80455b35bda55c";
const ACE_FNV: &str = "0xd2477939bbaba20a";
const GODIVA_K_FNV: &str = "0xcedab1315589be07";
const ACE_XSS_FNV: &str = "0x9a9dbebd27900700";

fn check(report: &str, key: &str, pinned: &str) {
    let got = value(report, key);
    println!("{key}={got}");
    assert_eq!(got, pinned, "{key}");
}

#[test]
fn zip_index_and_generation_give_the_pinned_fingerprints() {
    let Some(files) = endf_files() else {
        eprintln!("SKIP: reference tapes not present");
        return;
    };
    let (zip, r1) = probe::zip_and_index(&files).unwrap();
    print!("{r1}");
    check(&r1, "zip_fnv", ZIP_FNV);
    check(&r1, "index_fnv", INDEX_FNV);
    assert_eq!(value(&r1, "main_library"), "ENDF/B-VIII.0");
    assert_eq!(value(&r1, "layout"), "NndcSubLibraryArchives");
    assert_eq!((value(&r1, "tapes"), value(&r1, "skipped")), ("4", "1"));
    assert_eq!(
        value(&r1, "search_u235"),
        "ENDF-B-VIII.0_neutrons/n-092_U_235.endf"
    );
    let r2 = probe::generate_f19(&zip).unwrap();
    print!("{r2}");
    check(&r2, "ace_fnv", ACE_FNV);
    assert_eq!(value(&r2, "ace_files"), "2");
    let ace = std::fs::read(root().join(ACE_FILE.1));
    if let Ok(bytes) = ace {
        let r4 = probe::ace_tables(vec![(ACE_FILE.0.to_string(), bytes)]).unwrap();
        print!("{r4}");
        check(&r4, "ace_xss_fnv", ACE_XSS_FNV);
        assert_eq!(
            value(&r4, "u234_dbrc"),
            "false",
            "no 0 K companion in the set"
        );
    } else {
        eprintln!("SKIP (ACE part): reference-data/ace not checked out");
    }
    assert!(probe::generate_f19(b"not a zip").is_err());
    assert_eq!(probe::fnv(b"", probe::FNV0), probe::FNV0);
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-234/235/238 (~2 min); runs by default"
)]
fn godiva_from_the_zip_gives_the_pinned_k_trace() {
    let Some(files) = endf_files() else {
        eprintln!("SKIP: reference tapes not present");
        return;
    };
    let (zip, _) = probe::zip_and_index(&files).unwrap();
    let r3 = probe::godiva_from_zip(&zip).unwrap();
    print!("{r3}");
    check(&r3, "godiva_k_fnv", GODIVA_K_FNV);
    assert_eq!(value(&r3, "godiva_cycles"), "5");
}
