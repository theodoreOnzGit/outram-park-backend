// SPDX-License-Identifier: AGPL-3.0-only
//! **Photo-atomic RECONR, GAMINR, DTFR and MATXSR from a deck, whole decks
//! against NJOY2016.80** (gh:#1369).
//!
//! # Methodology
//!
//! Upstream decks 03 (H and U photon interaction data, ENDF-4 `gam23` on unit
//! 30 and `gam27` on unit 32: RECONR, GAMINR, DTFR, MATXSR, VIEWR) and 30 (H-1
//! ENDF/B-VIII.0 neutron and photo-atomic: MODER, RECONR, BROADR, GROUPR, a
//! photo-atomic RECONR, MODER, GAMINR, MATXSR) are run **whole** by the
//! NJOY2016.80 binary (`NJOY2016_ROOT/build/njoy`) and by [`run_deck`] on the
//! same tapes. Every coded unit NJOY writes is compared line for line, and the
//! units upstream commits (`tests/03/referenceTape37`,
//! `tests/30/referenceTape43/44/45`) byte for byte. Pass criterion: zero
//! differing lines on every unit, zero differing bytes against the committed
//! references, and the deck runs to its `stop`. A case skips with a message when
//! the NJOY clone or a resource is absent.
//!
//! Upstream has no other deck for these modules, so two decks of our own reach
//! branches 03 and 30 do not: X1 (DTFR with a photon-production table,
//! `ifilm = 2`, MF=3 point plots, then VIEWR) and X2 (GAMINR with `igg = 1`,
//! `iwt = 2`, an explicit reaction list; MATXSR on the gamout tape alone). They
//! are judged the same way, against NJOY2016.80 run on the same deck.
//!
//! # Predictions (posted on gh:#1369 before each first run)
//!
//! Photo-atomic RECONR, then GAMINR, DTFR and MATXSR in turn, and X1/X2: every
//! unit identical line for line. Each held on its first run.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! - Deck 03: units 31 (2180 lines), 33 (604), 34 (340), 35 (211), 36 (1711)
//!   and 37 (9274) identical; 37 is `referenceTape37` byte for byte.
//! - Deck 30: units 31 (21 249), 43 (1477), 44 (18 924) and 45 (4408)
//!   identical; 43, 44 and 45 are the committed references byte for byte.
//! - X1: units 31, 50 (3858), 51 (35 730), 52 (40 988) identical. X2: units 43
//!   and 44 identical once MT=501 is listed first; listed last, NJOY prints an
//!   unset name (zero bytes), refused here by name. The X2 prediction failed on
//!   its first run because of that line; a first `diff` check had missed it
//!   (`diff` treats a file with NUL bytes as binary).

use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

fn differing(a: &str, b: &str) -> Vec<usize> {
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect()
}

/// Run `deck` in NJOY2016.80 and here with each `(resource, unit)` mounted, and
/// compare `units` line for line and each `(unit, file)` against
/// `tests/<refs_case>/<file>` byte for byte. `None`: skipped.
fn compare(tag: &str, deck: &str, resources: &[(&str, u32)], units: &[u32], refs: &[(u32, &str)], refs_case: &str) -> Option<()> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_photon_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        for (r, u) in resources {
            let res = root.join("tests/resources").join(r);
            if !res.exists() {
                eprintln!("skip: {} not present", res.display());
                return None;
            }
            std::fs::copy(&res, d.join(format!("tape{u}"))).unwrap();
        }
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let st = Command::new("sh").arg("-c").arg(format!("{} < deck > log 2>&1", bin.display())).current_dir(&theirs).status().unwrap();
    assert!(st.success(), "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let read = |d: &Path, u: u32| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
    let mut bad = Vec::new();
    for &u in units {
        let (a, b) = (read(&ours, u), read(&theirs, u));
        let d = differing(&a, &b);
        println!("{tag} unit {u}: {} differing lines of {}", d.len(), b.lines().count());
        for &i in d.iter().take(4) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.lines().nth(i), b.lines().nth(i));
        }
        if !d.is_empty() || b.is_empty() {
            bad.push(u);
        }
    }
    for (u, f) in refs {
        let refp = root.join("tests").join(refs_case).join(f);
        let Ok(r) = std::fs::read(&refp) else { continue };
        let ok = std::fs::read(ours.join(format!("tape{u}"))).unwrap_or_default() == r;
        println!("{tag} unit {u} against upstream {f}: {}", if ok { "identical" } else { "DIFFERS" });
        if !ok {
            bad.push(*u);
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(bad.is_empty(), "{tag}: units {bad:?} differ from NJOY2016.80");
    Some(())
}

fn upstream_deck(case: &str) -> Option<String> {
    std::fs::read_to_string(njoy_root().join("tests").join(case).join("input")).ok()
}

/// **Deck 03 whole** (H and U photon interaction, ENDF-4): RECONR (two
/// photo-atomic materials), GAMINR (LANL 12 groups; U's incoherent matrix above
/// 1.14 MeV from H's `akn` cache), DTFR (tables and plots), MATXSR, VIEWR.
/// Result (2026-11-13): all six units identical; 37 is `referenceTape37`.
#[test]
fn test_03_photon_interaction_whole_deck() {
    let Some(deck) = upstream_deck("03") else { return };
    compare("t03", &deck, &[("gam23", 30), ("gam27", 32)], &[31, 33, 34, 35, 36, 37], &[(37, "referenceTape37")], "03");
}

/// **Deck 30 whole** (H-1 ENDF/B-VIII.0): the neutron GENDF, the photo-atomic
/// PENDF (MT=522 from the subshell MT=534), GAMINR (VITAMIN-J 42), and MATXSR
/// with `nscat`, `ng` and `gscat`.
/// Result (2026-11-13): units 31, 43, 44, 45 identical; 43-45 are upstream's
/// references byte for byte.
#[test]
fn test_30_h1_matxs_whole_deck() {
    let Some(deck) = upstream_deck("30") else { return };
    compare(
        "t30",
        &deck,
        &[("n-001_H_001-ENDF8.0-Beta6.endf", 20), ("photoat-001_H_000-ENDF8.0.endf", 40)],
        &[31, 43, 44, 45],
        &[(43, "referenceTape43"), (44, "referenceTape44"), (45, "referenceTape45")],
        "30",
    );
}

/// **X1** (our deck): deck 30 through GROUPR, then DTFR with a photon table
/// (MF=16), `ifilm = 2` and MF=3 point plots from the binary BROADR PENDF, then
/// VIEWR. Result (2026-11-13): units 31, 50, 51, 52 identical.
#[test]
fn test_x1_dtfr_photon_table_four_per_frame() {
    let Some(input) = upstream_deck("30") else { return };
    let mut deck = String::new();
    for line in input.lines() {
        if line.starts_with("reconr / Reconstruct photo") {
            break;
        }
        deck.push_str(line);
        deck.push('\n');
    }
    deck.push_str("dtfr\n31 50 -23 51/\n1 2 0/\n3 175 4 5 30 1 0/\n'capt'/\n1 102 1/\n1 42/\n'h1' 125 1 300./\n/\nviewr\n51 52/\nstop\n");
    compare(
        "x1",
        &deck,
        &[("n-001_H_001-ENDF8.0-Beta6.endf", 20), ("photoat-001_H_000-ENDF8.0.endf", 40)],
        &[31, 50, 51, 52],
        &[],
        "30",
    );
}

/// **X2** (our deck): H photo-atomic; GAMINR `igg = 1` (4 groups), `iwt = 2`,
/// `lord = 1`, reactions 23/501, 23/502, 26/504; MATXSR on the gamout tape.
/// Result (2026-11-13): the first version listed 23/501 last; NJOY then prints a
/// name `vector` never set (eight zero bytes, unit 44 line 11), which this port
/// now refuses by name (`x2_mt501_after_another_reaction_is_refused`). With
/// 23/501 first, units 43 and 44 are identical.
#[test]
fn test_x2_gaminr_read_in_groups_constant_weight() {
    let deck = "moder\n40 -41\nreconr\n-41 -42\n'photo H'/\n100 0/\n0.001 0. 0.003/\n0/\ngaminr\n-41 -42 0 43/\n100 1 2 1 0/\n'arbitrary groups'/\n4/\n1.e4 1.e5 1.e6 1.e7 1.e8/\n23 501/\n23 502/\n26 504/\n0/\n0/\nmatxsr\n0 43 44/\n1 'x2'/\n1 1 1 1\n'own deck x2'/\n'g'/\n4/\n'gscat'/\n1/\n1/\n'h' 100 100/\nstop\n";
    compare("x2", deck, &[("photoat-001_H_000-ENDF8.0.endf", 40)], &[43, 44], &[], "30");
}

/// X2 as first written, 23/501 after 23/502: upstream's `vector` reads a name it
/// never set, so the port refuses the run by name (it is blocked at MATXSR).
#[test]
fn x2_mt501_after_another_reaction_is_refused() {
    let root = njoy_root();
    let Ok(_) = std::fs::metadata(root.join("tests/resources/photoat-001_H_000-ENDF8.0.endf")) else { return };
    let tmp = std::env::temp_dir().join(format!("njoy_deck_photon_x2r_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::copy(root.join("tests/resources/photoat-001_H_000-ENDF8.0.endf"), tmp.join("tape40")).unwrap();
    let deck = "moder\n40 -41\nreconr\n-41 -42\n'photo H'/\n100 0/\n0.001 0. 0.003/\n0/\ngaminr\n-41 -42 0 43/\n100 1 2 1 0/\n'arbitrary groups'/\n4/\n1.e4 1.e5 1.e6 1.e7 1.e8/\n23 502/\n26 504/\n23 501/\n0/\n0/\nmatxsr\n0 43 44/\n1 'x2'/\n1 1 1 1\n'own deck x2'/\n'g'/\n4/\n'gscat'/\n1/\n1/\n'h' 100 100/\nstop\n";
    let run = run_deck(deck, &DeckOptions { workdir: Some(tmp.clone()), parse_only: false }).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    let (module, why) = run.blocked.expect("refused");
    assert_eq!(module, "matxsr");
    assert!(why.contains("unset hvps"), "{why}");
}
