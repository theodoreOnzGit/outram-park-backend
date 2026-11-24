// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **OpenMC's complete thermal deck, end to end against NJOY2016.80** (gh:#866).
//!
//! # Methodology
//!
//! `openmc.data.njoy.make_ace_thermal` writes a deck that no upstream NJOY2016 test
//! runs: RECONR, BROADR at every temperature, a **free-gas THERMR first**
//! (`0 mat 12 ntemp 1 0 iform 1 221 1`, MT=221 on the incident PENDF), then the
//! bound THERMR on that tape (`mat_thermal mat 16 ntemp 2 0 iform natom 222 1`),
//! then one ACER `iopt = 2` per temperature with OpenMC's card 9
//! `222 64 mte ielas nmix emax iwt` (64 outgoing energies, `iwt = 2`, the
//! continuous IFENG = 2 form). All units are positive, so every module hands its
//! tape to the next as text, as OpenMC's run does.
//!
//! The deck below is that template filled in for H in H₂O, ENDF/B-VIII.0 beta 6
//! (`tests/resources/n-001_H_001-ENDF8.0-Beta6.endf` MAT 125, unit 20;
//! `tsl-HinH2O-ENDF8.0-Beta6.endf` MAT 1, unit 21), two temperatures (293.6 and
//! 600 K), `emax = 4` eV, `error = 0.001`, `natom = 2`, no elastic (`mte = 0`).
//! It is run by the NJOY2016.80 binary (`NJOY2016_ROOT/build/njoy`) and by
//! [`run_deck`] in two directories on the same tapes, with one run date.
//!
//! Compared as strings, line for line: unit 23 (BROADR's PENDF), 24 (after the
//! free-gas THERMR), 25 (after the bound THERMR), and each ACER's ACE table and
//! directory line (28/29 at 293.6 K, 30/31 at 600 K). Pass criterion: every unit
//! identical and the deck runs to its `stop`. Skips when the NJOY clone or a tape
//! is absent.
//!
//! # Prediction (written here and committed before the first run, 2026-11-24)
//!
//! Every unit identical. The parts are each gated already (THERMR free gas in
//! deck 01 and the deck-driver README §5, the bound THERMR and ACER `iopt = 2` in
//! decks 67-69, IFENG = 2 in `acer_thermal_word_parity_large.rs`), but no gate
//! chains a free-gas THERMR into a bound one, runs `ntemp = 2` into ACER, or uses
//! 64 bins with `iwt = 2` through the deck driver.
//!
//! # Results
//!
//! (2026-11-24, NJOY2016.80 `ab27c64`, release.) **The four ACE units are
//! identical** (28: 298 761 lines, 30: 274 759, and both directory lines), and so
//! is BROADR's unit 23 (1 334 lines).
//!
//! **The prediction missed on the THERMR tapes.** First run: units 24 and 25
//! differed in 7 949 and 18 661 lines in columns 1-66, every one a last-digit
//! difference of the MF=6 density (MT 221 and 222). Cause: `calcem` writes its
//! rows to the scratch unit `nscr`, which is text when THERMR's `nout` is
//! (`thermr.f90:297-298`), and `tpend` reads the density back at `a11`'s
//! precision before dividing by the cross section. The deck driver skipped that
//! round trip; `acer::acesix::tpend_rows` already had it for the ACE builder,
//! which is why the ACE units agreed. Fixed in `deck::thermr` for `nout > 0`
//! (deck 01's free gas and decks 67-69 run THERMR on binary units and are
//! unchanged). After the fix every line of units 24 and 25 is identical in
//! columns 1-75.
//!
//! **Still open, and pinned:** the sequence numbers (columns 76-80) of 1 559 lines
//! of unit 24 and 515 485 of unit 25. NJOY's `tpend` copies the input PENDF's
//! sections and its own scratch sections with a running count that the copied
//! SEND and FEND lines do not reset; this port writes them as `asend`/`afend`
//! would. The test asserts those counts exactly, so a fix (or a regression) shows.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// OpenMC's `make_ace_thermal` deck for H in H₂O at 293.6 and 600 K.
const OM1: &str = "reconr\n20 22/\n'ENDF/B-VIII.0 PENDF for H1'/\n125 2/\n0.001/\n'ENDF/B-VIII.0: H1'/\n'Processed by NJOY'/\n0/\n\
broadr\n20 22 23\n125 2 0 0 0./\n0.001/\n293.6 600.0/\n0/\n\
thermr\n0 23 24\n0 125 12 2 1 0 0 1 221 1/\n293.6 600.0/\n0.001 4.0\n\
thermr\n21 24 25\n1 125 16 2 2 0 0 2 222 1/\n293.6 600.0/\n0.001 4.0\n\
acer\n20 25 0 28 29\n2 0 1 .20/\n'ENDF/B-VIII.0: H_in_H2O processed by NJOY'/\n125 293.6 'lwtr' /\n1001 /\n222 64 0 0 1 4.0 2/\n\
acer\n20 25 0 30 31\n2 0 1 .21/\n'ENDF/B-VIII.0: H_in_H2O processed by NJOY'/\n125 600.0 'lwtr' /\n1001 /\n222 64 0 0 1 4.0 2/\n\
stop\n";

/// `OM1`: OpenMC's thermal deck for H in H₂O, every unit against NJOY2016.80.
#[test]
fn om1_openmc_thermal_deck_h_in_h2o_two_temperatures() {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let res = root.join("tests/resources");
    let tapes = [(20, "n-001_H_001-ENDF8.0-Beta6.endf"), (21, "tsl-HinH2O-ENDF8.0-Beta6.endf")];
    if !bin.exists() || tapes.iter().any(|(_, t)| !res.join(t).exists()) {
        eprintln!("skip: {} or the H-1 / H in H2O tapes not present (set NJOY2016_ROOT)", bin.display());
        return;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_openmc_thermal_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        for (u, t) in tapes {
            std::fs::copy(res.join(t), d.join(format!("tape{u}"))).unwrap();
        }
    }
    std::fs::write(theirs.join("deck"), OM1).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on OM1");
    let run = run_deck(OM1, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "OM1 must run to its end");
    let mut bad = Vec::new();
    let mut seq_only = Vec::new();
    for u in [23, 24, 25, 28, 29, 30, 31] {
        let read = |d: &Path| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
        let (a, b) = (read(&ours), read(&theirs));
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let d: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
        // A line that differs only in its sequence number (columns 76-80).
        let head = |l: Option<&&str>| l.map(|x| x.get(..75).unwrap_or(x).to_string());
        let (seq, data): (Vec<usize>, Vec<usize>) = d.iter().partition(|&&i| head(al.get(i)) == head(bl.get(i)));
        println!("OM1 unit {u}: {} differing lines of {} ({} in the sequence number only)", d.len(), bl.len(), seq.len());
        for &i in data.iter().take(4) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
        }
        assert!(!bl.is_empty(), "OM1: NJOY2016 wrote no unit {u}");
        if !data.is_empty() {
            bad.push((u, data.len()));
        }
        if !seq.is_empty() {
            seq_only.push((u, seq.len()));
        }
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(bad.is_empty(), "OM1: units differ from NJOY2016.80 in columns 1-75: {bad:?}");
    // Pinned, not accepted: THERMR's text PENDFs number their lines as `tpend`'s
    // copies do (a running count through copied SEND/FEND), which this port does
    // not reproduce yet (gh:#866). 0 here means it was fixed: tighten to identity.
    assert_eq!(seq_only, vec![(24, 1559), (25, 515485)], "OM1: the pinned sequence-number residual moved");
}
