// SPDX-License-Identifier: AGPL-3.0-only
//! **ERRORR `mfcov = 40` from a deck, against NJOY2016.80** (gh:#1371).
//!
//! # Methodology
//!
//! No upstream deck runs ERRORR on MF=40. Upstream deck 60 (IRDFF-II Fe) was cut
//! before its ERRORR. Every MF=40 evaluation in the repository or in NJOY2016's
//! `tests/resources` has sections with more than one final state (`NS > 1`).
//! Upstream's `gridd` (`errorr.f90:1184-1191`) and `covcal` (`:1889-1907`) read
//! only the first state's CONT, so NJOY2016.80 stops in `covcal` with "mfcov mt
//! found not equal to input mt" on such a tape. The deck `M40` therefore runs on
//! TENDL-2023 Ar-37 (`tests/resources/n-018_Ar_37-tendl2023.endf`, MAT 1828)
//! edited here, in memory. In both MF=40 sections (MT 11 and 33), `NS` is set to 1
//! and the `LFS = 1` state (4 lines) is deleted. Nothing else changes.
//!
//! The deck:
//! - MODER, then RECONR at 0.01.
//! - GROUPR at 0 K: `ign = 1` (6 groups, 1e-5 eV to 30 MeV, so the MT 11 and 33
//!   thresholds fall inside), `iwt = 2`, card 9 `3 1/ 3 11/ 3 33/ 10/`, binary
//!   GENDF on unit 23.
//! - MODER of the GENDF to text unit 25.
//! - ERRORR `mfcov = 40` on that GENDF, with the same groups, writing unit 24.
//!
//! NJOY2016.80 and [`run_deck`] run the same deck on the same tape. Pass
//! criterion: units 25 and 24 identical line for line, compared as strings, and
//! the deck runs to its `stop`.
//!
//! # Prediction (posted on gh:#1371 before the first run)
//!
//! Unit 24 identical ("35 lines"; it has 32). The unedited tape is refused by name
//! at `errorr::gridd::mf40_final_state` (`NS /= 1`).
//!
//! # Results (2026-11-14, NJOY2016.80, release)
//!
//! - **First run: the prediction missed.** 2 of unit 24's 32 lines differed: the
//!   MF=3 coarse cross sections of MT 11 and 33 (1.05375e-10 against 1.33711e-10).
//!   The GENDF (unit 25, 492 lines) was identical. The cause was a defect older
//!   than MF=40: GROUPR's `10/` writes each nuclide production as a further
//!   MF=3/`mt` section with `C2 = izap`, after the reaction's own. `rdgout` takes
//!   the first match (`errorr.f90:2889-2891`). `mf31::union_group_xs` kept the
//!   last. Fixed to keep the first.
//! - **After the fix:** units 25 (492 lines) and 24 (32 lines) identical.
//! - **Ablation:** with AWR in place of `izap` in the section HEADs and `-11` in
//!   the MF=1 HEAD, 3 of unit 24's 32 lines differ.
//! - The unedited Ar-37 is refused by name, as predicted.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// Ar-37 with each MF=40 section cut to its first final state: the HEAD's `NS`
/// (columns 45-55) set to 1, and the second state's lines (sequence numbers 6 and
/// up, `LFS = 1`) dropped. Panics if the tape does not have that shape.
fn ar37_one_final_state(tape: &str) -> String {
    let mut out = String::with_capacity(tape.len());
    let mut edited = 0;
    for l in tape.lines() {
        let mf = l.get(70..72).and_then(|x| x.trim().parse::<i32>().ok()).unwrap_or(0);
        let mt = l.get(72..75).and_then(|x| x.trim().parse::<i32>().ok()).unwrap_or(0);
        let ns = l.get(75..80).and_then(|x| x.trim().parse::<i32>().ok()).unwrap_or(0);
        if mf == 40 && mt > 0 {
            if ns == 1 {
                assert_eq!(l[44..55].trim(), "2", "MF=40 MT={mt} HEAD: NS");
                out.push_str(&format!("{}{:11}{}\n", &l[..44], 1, &l[55..]));
                edited += 1;
                continue;
            }
            if ns >= 6 {
                continue;
            }
        }
        out.push_str(l);
        out.push('\n');
    }
    assert_eq!(edited, 2, "Ar-37 has two MF=40 sections");
    out
}

/// The `M40` deck.
const M40: &str = "moder\n20 -21 /\nreconr\n-21 -22 /\n'Ar-37' /\n1828 /\n.01 /\n0 /\n\
groupr\n-21 -22 0 -23 /\n1828 1 0 2 0 1 1 1 /\n'Ar-37' /\n0. /\n1.e10 /\n6 /\n\
1e-5 1e5 1e6 1e7 2.2e7 2.6e7 3e7 /\n3 1 /\n3 11 /\n3 33 /\n10 /\n0 /\n0 /\n\
moder\n-23 25/\nerrorr\n-21 0 -23 24 0 0 /\n1828 1 2 1 1 /\n0 0. /\n0 40 /\n6 /\n1e-5 1e5 1e6 1e7 2.2e7 2.6e7 3e7 /\nstop\n";

/// Run `deck` in NJOY2016.80 and here, with `tape` as unit 20; compare the text
/// units `units` line by line. Returns `(unit, differing lines, NJOY's lines)` per
/// unit, or `None` (skipped).
fn compare(tag: &str, deck: &str, tape: &str, units: &[u32]) -> Option<Vec<(u32, usize, usize)>> {
    let bin = njoy_root().join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_mf40_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), tape).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let mut out = Vec::new();
    for &u in units {
        let read = |d: &Path| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
        let (a, b) = (read(&ours), read(&theirs));
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let d: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
        println!("{tag} unit {u}: {} differing lines of {}", d.len(), bl.len());
        for &i in d.iter().take(6) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
        }
        out.push((u, d.len(), bl.len()));
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(out)
}

fn ar37() -> Option<String> {
    let p = njoy_root().join("tests/resources/n-018_Ar_37-tendl2023.endf");
    match std::fs::read_to_string(&p) {
        Ok(s) => Some(s),
        Err(_) => {
            eprintln!("skip: {} not present", p.display());
            None
        }
    }
}

/// **`M40`: ERRORR `mfcov = 40` from a binary GENDF**, on Ar-37 cut to one final
/// state per MF=40 section. Units 25 (the GENDF as text) and 24 are compared with
/// NJOY2016.80. **Result (2026-11-14): both identical**, after the `rdgout`
/// first-match fix in `mf31::union_group_xs`.
#[test]
fn m40_errorr_mf40_one_final_state() {
    let Some(t) = ar37() else { return };
    let Some(r) = compare("m40", M40, &ar37_one_final_state(&t), &[25, 24]) else { return };
    for (u, d, n) in r {
        assert!(n > 20 && d == 0, "m40 unit {u}: {d} of {n} lines differ");
    }
}

/// **The unedited Ar-37 (`NS = 2`) is refused by name**, where NJOY2016.80 stops in
/// `covcal` ("mfcov mt found not equal to input mt").
#[test]
fn m40_two_final_states_are_refused_by_name() {
    let Some(t) = ar37() else { return };
    let tmp = std::env::temp_dir().join(format!("njoy_deck_mf40_ns2_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("tape20"), t).unwrap();
    let run = run_deck(M40, &DeckOptions { workdir: Some(tmp.clone()), parse_only: false, run_date: None }).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    let (module, why) = run.blocked.expect("NS = 2 must stop the run");
    assert_eq!(module, "errorr");
    assert!(why.contains("NS /= 1"), "refusal names NS: {why}");
}
