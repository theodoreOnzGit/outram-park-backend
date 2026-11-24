//! `unionx`'s second discontinuity pair, `edl`/`edh` (`acefc.f90:1225-1243`,
//! gh:#873), against the NJOY2016 binary on an edited PENDF.
//!
//! # Why an edited tape
//!
//! The pair fires only when the MT=1 energy right after a photon-production
//! pair's `egh` (or after a previous `edh`) is tabulated **twice** in the
//! PENDF's MT=1 (`gety1`'s `idis`, `endf.f90:2131`). No PENDF an NJOY2016
//! module writes has a repeated MT=1 energy: none of the 43 MT=1 sections in
//! NJOY2016's own tests' reference PENDFs (`tests/*/referenceTape*`) has one,
//! and all are INT = 2 (surveyed 2026-11-24). This crate's incident-neutron
//! ACER takes only a PENDF its own RECONR/BROADR wrote. So no held tape reaches
//! the branch through a deck, in either code; NJOY2016 reaches it only from a
//! PENDF written elsewhere.
//!
//! # Methodology
//!
//! - Input: NJOY2016 test 07 (U-235, ENDF/B-V tape 511, MAT 1395): its ENDF
//!   tape (`tests/resources/t511`) and its 300 K PENDF (`tests/07/referenceTape28`,
//!   RECONR + BROADR + HEATR). `convr` finds one photon discontinuity there,
//!   1.09 MeV, so `egl = 1.089999 MeV` and `egh = 1.090001 MeV`; the MT=1
//!   energies after it are 1.1, 1.1125, 1.13125, 1.15, ..., 1.2, 1.2125 MeV.
//! - Three PENDFs, each written by this test from `referenceTape28`:
//!   - `base`: unedited;
//!   - `single`: 1.1125 MeV's energy set to 1.1 MeV (its cross section kept),
//!     so the point after `egh` repeats (the `egh` branch);
//!   - `chain`: as `single`, plus 1.15 MeV set to 1.13125 MeV (the point after
//!     `edh` repeats: the `edh` branch) and 1.2125 MeV set to 1.2 MeV (a repeat
//!     met by the ordinary walk, which `gety1` reads once).
//! - NJOY2016 runs `acer` (`20 28 0 26 27`, `iopt = 1`, MAT 1395 at 300 K) on
//!   each, and its ESZ energy column (`NXS(3)` words) is read from `tape26`.
//! - The port: `acer::build::unionx_grid` on the same edited MT=1 energies, with
//!   the photon discontinuities from `photon_blocks::gamma_discontinuities` on
//!   `t511`, written as the builder writes ESZ (`sigfig(E/1e6, 9)`).
//! - Pass: same length, and every energy equal to NJOY2016's printed word
//!   within 1e-11 relative (the 12 figures Type 1 prints).
//!
//! **Prediction, written before the NJOY runs (2026-11-24):** `single`'s grid
//! is `base`'s with 1.1 and 1.1125 MeV replaced by 1.099999 and 1.100001 MeV,
//! the same length (5 041); `chain` has 1.099999, 1.100001, 1.131249 and
//! 1.131251 MeV in place of 1.1, 1.1125, 1.13125 and 1.15 MeV, and 1.2125 MeV
//! gone (5 040). Without the pair the port gave one word fewer on `single`
//! (1.1 MeV once).
//!
//! # Results (2026-11-24, NJOY2016 build at `ab27c64`)
//!
//! All three cases: NJOY2016's ESZ energies in every word (5 041, 5 041 and
//! 5 040 words), and NJOY2016's grid moved exactly as predicted. The test
//! covers the grid only: this crate cannot build a whole incident-neutron
//! table from a PENDF no module in its run wrote (`deck::run`'s `NotPorted`),
//! so the cross sections at `edl`/`edh` are not compared.
//!
//! Skips (prints why) when the NJOY2016 binary or its test files are absent
//! (`NJOY2016_ROOT`).

// Runs the NJOY2016 binary in a work directory: needs `std`.
#![cfg(feature = "std")]

mod common;

use njoy_outram_park_fork::acer::{build::unionx_grid, photon_blocks::gamma_discontinuities, read::parse_type1};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::mixr::mix::sigfig;
use std::path::{Path, PathBuf};

const MAT: i32 = 1395;

fn root() -> PathBuf {
    std::env::var("NJOY2016_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// `pendf` with each `(old, new)` 22-column (E, sigma) field of MF=3/MT=1
/// replaced, each edit applying to exactly one line.
fn edit(pendf: &str, edits: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = pendf.lines().map(str::to_string).collect();
    for (old, new) in edits {
        let hits: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].len() >= 75 && &lines[i][66..75] == "1395 3  1" && lines[i][..66].contains(old)).collect();
        assert_eq!(hits.len(), 1, "edit {old:?} must match one MT=1 line");
        lines[hits[0]] = lines[hits[0]].replacen(old, new, 1);
    }
    lines.join("\n") + "\n"
}

/// MF=3/MT=1's energies as tabulated [eV].
fn mt1_energies(pendf: &Path) -> Vec<f64> {
    let tape = Tape::read_file(pendf).unwrap();
    let sec = tape.section(MAT, 3, 1).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().unwrap();
    cur.read_tab1().unwrap().pairs.iter().map(|&(x, _)| x).collect()
}

const DECK: &str = "acer\n 20 28 0 26 27/\n 1/\n 'edl edh probe'/\n 1395 300./\n 0/\n /\nstop\n";

#[test]
fn unionx_edl_edh_grid_is_njoy2016s() {
    let r = root();
    let bin = r.join("build/njoy");
    let (t511, tape28) = (r.join("tests/resources/t511"), r.join("tests/07/referenceTape28"));
    for p in [&bin, &t511, &tape28] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT)", p.display());
            return;
        }
    }
    let base = std::fs::read_to_string(&tape28).unwrap();
    let single_edits = [(" 1.112500+6 6.749695+0", " 1.100000+6 6.749695+0")];
    let chain_edits = [
        (" 1.112500+6 6.749695+0", " 1.100000+6 6.749695+0"),
        (" 1.150000+6 6.731231+0", " 1.131250+6 6.731231+0"),
        (" 1.212500+6 6.715962+0", " 1.200000+6 6.715962+0"),
    ];
    let cases = [("base", base.clone()), ("single", edit(&base, &single_edits)), ("chain", edit(&base, &chain_edits))];
    let disc = gamma_discontinuities(&Tape::read_file(&t511).unwrap(), MAT);
    assert_eq!(disc, vec![1.09e6], "test 07's one photon discontinuity");

    let tmp = std::env::temp_dir().join(format!("njoy_unionx_edl_edh_{}", std::process::id()));
    let mut grids = Vec::new();
    for (name, pendf) in &cases {
        let dir = tmp.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(&t511, dir.join("tape20")).unwrap();
        std::fs::write(dir.join("tape28"), pendf).unwrap();
        std::fs::write(dir.join("deck"), DECK).unwrap();
        let (ok, _) = common::run_njoy_dated(&bin, &dir);
        assert!(ok, "NJOY2016 failed on {name}");
        let ace = parse_type1(&std::fs::read_to_string(dir.join("tape26")).unwrap()).unwrap();
        let nes = ace.nxs[2] as usize;
        let theirs = ace.xss[..nes].to_vec();
        let ours: Vec<f64> = unionx_grid(&mt1_energies(&dir.join("tape28")), &disc).iter().map(|&e| sigfig(e / 1.0e6, 9, 0)).collect();
        let bad: Vec<usize> = (0..nes.min(ours.len())).filter(|&i| ((ours[i] - theirs[i]) / theirs[i]).abs() > 1e-11).collect();
        println!("{name}: port {} words, NJOY2016 {nes}; differing {}; first {:?}", ours.len(), bad.len(), bad.first().map(|&i| (ours[i], theirs[i])));
        assert_eq!(ours.len(), nes, "{name}: grid length");
        assert!(bad.is_empty(), "{name}: {} energies differ", bad.len());
        grids.push(theirs);
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // NJOY2016's own grid moved as predicted (the branch was reached).
    let has = |g: &[f64], e: f64| g.iter().any(|&x| (x - e).abs() < 1e-12);
    let (b, s, c) = (&grids[0], &grids[1], &grids[2]);
    assert_eq!((b.len(), s.len(), c.len()), (5041, 5041, 5040));
    assert!(has(b, 1.1) && has(b, 1.1125) && !has(b, 1.099999));
    assert!(has(s, 1.099999) && has(s, 1.100001) && !has(s, 1.1) && !has(s, 1.1125));
    for e in [1.099999, 1.100001, 1.131249, 1.131251, 1.2] {
        assert!(has(c, e), "chain lacks {e}");
    }
    for e in [1.1, 1.13125, 1.15, 1.2125] {
        assert!(!has(c, e), "chain keeps {e}");
    }
}
