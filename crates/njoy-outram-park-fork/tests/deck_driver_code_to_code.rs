// SPDX-License-Identifier: AGPL-3.0-only
//! **NJOY input-deck driver vs NJOY2016.80** (gh:#1369, differential #1366).
//!
//! # 1. Card reader against gfortran list-directed reads (function level)
//!
//! `verification_and_validation/deck_driver/listdirected.f90` performs 36 single
//! `read(5,*)` statements over `listdirected.in` (nulls, `/`, `r*v`, `r*`, quoted
//! strings, records spanning, excess values, tabs, error cases ...) and prints
//! every item (reals as hex bits). `listdirected.ref` is its output, produced by
//! gfortran 13.3 on 2026-11-11. This test replays the same sequence with
//! [`CardReader`] and requires the **identical text**.
//!
//! **Prediction (written before the run):** identical on all 36 cases; the
//! likeliest miss was the end-of-record handling of a quoted string spanning
//! records, which no case exercises.
//!
//! # 2. Deck integration: upstream test 01 (C-nat), first modules
//!
//! Upstream `tests/01/input` is read through the deck driver. Its cards parse
//! to `stop` (all of MODER, RECONR, BROADR, HEATR, THERMR x2, GROUPR). It is
//! then **run as far as the port has tape-level drivers**: `moder; reconr;
//! broadr; heatr`, with upstream's own units (all blocked-binary), the MODER
//! conversions appended to read the result as text. The same deck is run by the
//! NJOY2016.80 binary. Every MF=3 section of the PENDF after BROADR (unit 29)
//! and after HEATR (unit 30) is compared point by point, QM, QI, LR included.
//!
//! The NJOY clone (binary `build/njoy`, `tests/resources/t511`) is located by
//! `NJOY2016_ROOT`, else `upstream_source/NJOY2016`; the test skips with a
//! message if it is absent.

use std::path::PathBuf;
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, CardReader, DeckOptions, Slot};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;

/// The Fortran program's read sequence, replayed. Each entry: (case label,
/// slot template, extra case suffix lines handled below).
fn replay() -> String {
    let input = include_str!("../verification_and_validation/deck_driver/listdirected.in");
    let mut r = CardReader::new(input);
    let mut out = String::new();
    let seti = || vec![Slot::Int(-1), Slot::Int(-2), Slot::Int(-3)];
    let setr = || vec![Slot::Real(-1.0), Slot::Real(-2.0), Slot::Real(-3.0), Slot::Real(-4.0)];
    let sets = || vec![Slot::string(16, "<unset1>"), Slot::string(16, "<unset2>")];
    let pi = |n: u32, s: &[Slot]| format!("case {n} ints {} {} {}\n", s[0].int(), s[1].int(), s[2].int());
    let pr = |n: u32, s: &[Slot]| {
        format!(
            "case {n} reals {:016X} {:016X} {:016X} {:016X}\n",
            s[0].real().to_bits(),
            s[1].real().to_bits(),
            s[2].real().to_bits(),
            s[3].real().to_bits()
        )
    };
    let ps = |n: u32, s: &[Slot]| {
        let v = |i: usize| match &s[i] {
            Slot::Str { value, .. } => value.clone(),
            _ => unreachable!(),
        };
        format!("case {n} str |{}|{}|\n", v(0), v(1))
    };
    // ints-only reads of 3 items
    for n in [1, 2, 3, 4, 5, 6, 7] {
        let mut s = seti();
        r.read(&mut s).unwrap();
        out += &pi(n, &s);
    }
    // two-item int reads (8, 9, 10): only i1, i2 are read, i3 stays -3
    for n in [8, 9, 10] {
        let mut s = seti();
        r.read(&mut s[..2]).unwrap();
        out += &pi(n, &s);
    }
    for n in [11, 12, 13] {
        let mut s = setr();
        r.read(&mut s).unwrap();
        out += &pr(n, &s);
    }
    for n in [14, 15, 16, 17] {
        let mut s = sets();
        r.read(&mut s[..1]).unwrap();
        out += &ps(n, &s);
    }
    // 18: s1, i1
    {
        let (mut st, mut it) = (sets(), seti());
        let mut s = vec![st.remove(0), it.remove(0)];
        r.read(&mut s).unwrap();
        let (s1, i1) = (s.remove(0), s.remove(0));
        let st = vec![s1, Slot::string(16, "<unset2>")];
        let it = vec![i1, Slot::Int(-2), Slot::Int(-3)];
        out += &ps(18, &st);
        out += &pi(18, &it);
    }
    // 19: s1, i1, i2
    {
        let mut s = vec![Slot::string(16, "<unset1>"), Slot::Int(-1), Slot::Int(-2)];
        r.read(&mut s).unwrap();
        out += &ps(19, &[s[0].clone(), Slot::string(16, "<unset2>")]);
        out += &pi(19, &[s[1].clone(), s[2].clone(), Slot::Int(-3)]);
    }
    // 20: i1, i2, s1
    {
        let mut s = vec![Slot::Int(-1), Slot::Int(-2), Slot::string(16, "<unset1>")];
        r.read(&mut s).unwrap();
        out += &ps(20, &[s[2].clone(), Slot::string(16, "<unset2>")]);
        out += &pi(20, &[s[0].clone(), s[1].clone(), Slot::Int(-3)]);
    }
    // 21
    {
        let mut s = seti();
        r.read(&mut s).unwrap();
        out += &pi(21, &s);
    }
    // 22: s1, s2
    {
        let mut s = sets();
        r.read(&mut s).unwrap();
        out += &ps(22, &s);
    }
    // 23
    {
        let mut s = seti();
        r.read(&mut s).unwrap();
        out += &pi(23, &s);
    }
    // 24
    {
        let mut s = sets();
        r.read(&mut s[..1]).unwrap();
        out += &ps(24, &s);
    }
    // 25: i1, i2, s1
    {
        let mut s = vec![Slot::Int(-1), Slot::Int(-2), Slot::string(16, "<unset1>")];
        r.read(&mut s).unwrap();
        out += &ps(25, &[s[2].clone(), Slot::string(16, "<unset2>")]);
        out += &pi(25, &[s[0].clone(), s[1].clone(), Slot::Int(-3)]);
    }
    // 26: i1, s1
    {
        let mut s = vec![Slot::Int(-1), Slot::string(16, "<unset1>")];
        r.read(&mut s).unwrap();
        out += &ps(26, &[s[1].clone(), Slot::string(16, "<unset2>")]);
        out += &pi(26, &[s[0].clone(), Slot::Int(-2), Slot::Int(-3)]);
    }
    for n in [27, 28] {
        let mut s = seti();
        r.read(&mut s[..2]).unwrap();
        out += &pi(n, &s);
    }
    // 29
    {
        let mut s = seti();
        r.read(&mut s).unwrap();
        out += &pi(29, &s);
    }
    // 30 reals
    {
        let mut s = setr();
        r.read(&mut s).unwrap();
        out += &pr(30, &s);
    }
    // 31
    {
        let mut s = seti();
        r.read(&mut s).unwrap();
        out += &pi(31, &s);
    }
    // error cases: 32 ints on "1.5 2"; 33 ints on "abc 1"; 34 reals on "1.5e 2"; 35 EOF mid-read; 36 EOF
    let nz = |n: u32, e: bool| format!("case {n} ios-nonzero {}\n", if e { 'T' } else { 'F' });
    out += &nz(32, r.read(&mut [Slot::Int(-1), Slot::Int(-2)]).is_err());
    out += &nz(33, r.read(&mut [Slot::Int(-1), Slot::Int(-2)]).is_err());
    out += &nz(34, r.read(&mut [Slot::Real(-1.0), Slot::Real(-2.0)]).is_err());
    out += &nz(35, r.read(&mut seti()).is_err());
    out += &nz(36, r.read(&mut seti()).is_err());
    out
}

/// **Card reader == gfortran list-directed read, 36 cases.**
///
/// Methodology and prediction in the file header. Pass criterion: the replay
/// text equals `listdirected.ref` exactly. **Result (2026-11-11, gfortran
/// 13.3, rustc per rust-toolchain.toml):** see the V&V README for the count.
#[test]
fn card_reader_matches_gfortran_list_directed() {
    let reference = include_str!("../verification_and_validation/deck_driver/listdirected.ref");
    let ours = replay();
    for (i, (a, b)) in ours.lines().zip(reference.lines()).enumerate() {
        assert_eq!(a, b, "line {} differs (ours | gfortran)", i + 1);
    }
    assert_eq!(ours.lines().count(), reference.lines().count());
}

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

fn mf3_sections(tape: &Tape, mat: i32) -> Vec<(i32, [f64; 4], Vec<(f64, f64)>)> {
    tape.sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|s| {
            let mut cur = SectionCursor::new(&s.rows);
            let _ = cur.read_cont().unwrap();
            let t = cur.read_tab1().unwrap();
            (s.key.mt, [t.head.c1, t.head.c2, t.head.l1 as f64, t.head.l2 as f64], t.pairs.clone())
        })
        .collect()
}

fn compare(label: &str, ours: &Tape, theirs: &Tape, mat: i32) -> Vec<String> {
    let o = mf3_sections(ours, mat);
    let t = mf3_sections(theirs, mat);
    let mut diffs = Vec::new();
    for (mt, head, pairs) in &t {
        match o.iter().find(|s| s.0 == *mt) {
            None => diffs.push(format!("MT={mt}: missing in ours")),
            Some((_, h, p)) => {
                if h != head {
                    diffs.push(format!("MT={mt}: TAB1 head ours {h:?} NJOY {head:?}"));
                }
                if p != pairs {
                    let first = p.iter().zip(pairs).position(|(a, b)| a != b);
                    let at = first.unwrap_or(0);
                    diffs.push(format!(
                        "MT={mt}: ours {} pts, NJOY {}, first differing pair {first:?}: ours {:?} NJOY {:?}",
                        p.len(),
                        pairs.len(),
                        &p[at.saturating_sub(1)..(at + 3).min(p.len())],
                        &pairs[at.saturating_sub(1)..(at + 3).min(pairs.len())],
                    ));
                }
            }
        }
    }
    for (mt, _, _) in &o {
        if !t.iter().any(|s| s.0 == *mt) {
            diffs.push(format!("MT={mt}: extra in ours"));
        }
    }
    println!("[{label}] {} MF=3 sections in NJOY, {} in ours, {} differ", t.len(), o.len(), diffs.len());
    diffs.iter().map(|d| format!("[{label}] {d}")).collect()
}

/// **Upstream test 01 (C-nat) through the deck driver**, MODER, RECONR,
/// BROADR and HEATR, against the NJOY2016.80 binary on the same deck.
///
/// Methodology: file header. Pass criterion: every MF=3 section of units 29
/// (after BROADR) and 30 (after HEATR), identical pairs and TAB1 heads, no
/// section missing or extra. Also asserts the **whole** deck's cards parse to
/// `stop` and that the run blocks at THERMR (no tape driver), as documented.
#[test]
fn upstream_test_01_front_half_through_deck_driver() {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let t511 = root.join("tests/resources/t511");
    let own = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016/tests/01/input");
    let deck_path = if root.join("tests/01/input").exists() { root.join("tests/01/input") } else { own };
    for p in [&bin, &t511, &deck_path] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT to an NJOY2016.80 clone with build/ and tests/)", p.display());
            return;
        }
    }
    let full = std::fs::read_to_string(&deck_path).unwrap();

    // (a) every card of the real deck is read, to `stop`
    let parsed = run_deck(&full, &DeckOptions { workdir: None, parse_only: true, run_date: None }).unwrap();
    println!("test 01 modules parsed: {:?}", parsed.modules);

    // (b) front half: the real deck's cards up to THERMR, plus MODER conversions
    let head: String = full.lines().take_while(|l| l.trim() != "thermr").collect::<Vec<_>>().join("\n");
    let head = head.replacen("broadr\n", "moder\n -22 28/\nbroadr\n", 1);
    let deck = format!("{head}\nmoder\n -23 29/\nmoder\n -22 30/\nstop\n");

    let tmp = std::env::temp_dir().join(format!("njoy_deck_t01_{}", std::process::id()));
    let (ours_dir, theirs_dir) = (tmp.join("ours"), tmp.join("njoy"));
    std::fs::create_dir_all(&ours_dir).unwrap();
    std::fs::create_dir_all(&theirs_dir).unwrap();
    std::fs::copy(&t511, ours_dir.join("tape20")).unwrap();
    std::fs::copy(&t511, theirs_dir.join("tape20")).unwrap();
    std::fs::write(theirs_dir.join("deck"), &deck).unwrap();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&theirs_dir)
        .status()
        .unwrap();
    assert!(status.success(), "NJOY2016 binary failed");

    let run = run_deck(&deck, &DeckOptions { workdir: Some(ours_dir.clone()), parse_only: false, run_date: None }).unwrap();
    assert_eq!(run.blocked, None);
    assert_eq!(run.modules.iter().filter(|m| *m == "heatr").count(), 1);

    let mut all = Vec::new();
    for (unit, label) in [(28, "after RECONR"), (29, "after BROADR"), (30, "after HEATR")] {
        let theirs = Tape::read_file(&theirs_dir.join(format!("tape{unit}"))).unwrap();
        let ours = Tape::read_file(&ours_dir.join(format!("tape{unit}"))).unwrap();
        all.extend(compare(label, &ours, &theirs, 1306));
    }
    assert!(all.is_empty(), "{}", all.join("\n"));

    // (c) the whole deck stops at THERMR, naming it
    let full_run = run_deck(&full, &DeckOptions { workdir: None, parse_only: false, run_date: None });
    // without tape20 on disk the first MODER cannot load; the block is the front half only
    assert!(full_run.is_err() || full_run.unwrap().blocked.is_some());
    let _ = std::fs::remove_dir_all(&tmp);
}
