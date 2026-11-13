// SPDX-License-Identifier: AGPL-3.0-only
//! **PLOTR from a deck, whole decks against NJOY2016.80** (gh:#1369).
//!
//! # Methodology
//!
//! Upstream decks 06 (C-12 and Li-6, ENDF/B-V tape `t511` on unit 30) and 12
//! (Ni-61 ENDF/B-VI.1 `eni61` on unit 20: RECONR, GASPR, PLOTR on the PENDF,
//! VIEWR) are run **whole** by the NJOY2016.80 binary
//! (`NJOY2016_ROOT/build/njoy`) and by [`run_deck`] on the same tape. Compared
//! line by line: PLOTR's plot-command file (deck 06 unit 31, deck 12 unit 23),
//! VIEWR's PostScript of it (32, 24) and, for deck 12, the RECONR and GASPR
//! PENDFs (21, 22). The PostScript is also compared with upstream's own
//! committed `referenceTape32` / `referenceTape24`. Pass criterion: zero
//! differing lines on every unit, and the deck runs to its `stop`. A case skips
//! with a message when the NJOY clone is absent.
//!
//! Deck 06 exercises the ENDF 2-d walk with its log-log and lin-lin thinning
//! (MF=3 MT=1, MT=107), input data with and without error bars, `ad3d` (MF=4
//! Legendre, 101 cosines, incident-grid thinning), `ed3d` (MF=5 LF=1) and MF=5
//! 2-d spectra picked by `nth` with tag labels (`ileg = 2`). Deck 12 exercises
//! the PENDF path (`iverf = 6`, temperature match) with a legend block.
//!
//! Upstream has no other PLOTR deck, so five decks of our own (the constants
//! below) reach the branches 06 and 12 do not: `gety4`, `getz` (ratio plot),
//! the automatic reaction loop, MF=7, MF=6 and GENDF with `gg3d`. They are
//! judged the same way, against NJOY2016.80 run on the same deck.
//!
//! # Prediction (posted on gh:#1369 before the first run of decks 06 and 12)
//!
//! Both decks identical on every unit; if not, a 1-ulp step in the thinning
//! moving a kept point, or the last figure of a `1p,2e16.8` word. **The five
//! decks of our own were run before a prediction was written for them**; that
//! is recorded on gh:#1369 with their results.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! - Deck 12: units 21, 22, 23 and 24 identical on the first run.
//! - Deck 06: the first run stopped at curve 2 (MT=107, automatic x axis):
//!   after the frame search `findf` must back up through the section to the
//!   card before its HEAD (`endf.f90:1568-1578`, `if (mth.gt.0) go to 300`);
//!   the port stopped one card short. After that fix, units 31 and 32 are
//!   identical (17 344 and 22 422 lines), and unit 32 is upstream's
//!   `referenceTape32` byte for byte; deck 12's unit 24 is `referenceTape24`.
//! - Our decks: see each test. All identical, except MF=6, where NJOY2016.80's
//!   output depends on an undefined variable and this port refuses the curve.

use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016")
        })
}

fn differing(a: &str, b: &str) -> Vec<usize> {
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    (0..al.len().max(bl.len()))
        .filter(|&i| al.get(i) != bl.get(i))
        .collect()
}

/// What [`run_both`] leaves: our work directory, NJOY's, and where our run stopped.
type Pair = (PathBuf, PathBuf, Option<(String, String)>);

/// Run `deck` in NJOY2016.80 and here, with each `(resource, unit)` mounted.
/// `None`: skipped (no NJOY clone or resource).
fn run_both(tag: &str, deck: &str, resources: &[(&str, u32)]) -> Option<Pair> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!(
        "njoy_deck_plotr_{}_{}",
        tag.replace([' ', '/'], "_"),
        std::process::id()
    ));
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
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&theirs)
        .status()
        .unwrap();
    assert!(st.success(), "NJOY2016 binary failed on {tag}");
    let run = run_deck(
        deck,
        &DeckOptions {
            workdir: Some(ours.clone()),
            parse_only: false,
        },
    )
    .unwrap();
    Some((ours, theirs, run.blocked))
}

/// [`run_both`], then `units` compared line by line, and each `(unit, file)`
/// against upstream's committed `tests/<refs_case>/<file>`.
fn compare(
    tag: &str,
    deck: &str,
    resources: &[(&str, u32)],
    units: &[u32],
    refs: &[(u32, &str)],
    refs_case: &str,
) -> Option<()> {
    let root = njoy_root();
    let (ours, theirs, blocked) = run_both(tag, deck, resources)?;
    assert_eq!(blocked, None, "{tag} must run to its end");
    let read =
        |d: &Path, u: u32| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
    let mut bad = Vec::new();
    for &u in units {
        let (a, b) = (read(&ours, u), read(&theirs, u));
        let d = differing(&a, &b);
        let n = b.lines().count();
        println!("{tag} unit {u}: {} differing lines of {n}", d.len());
        for &i in d.iter().take(4) {
            println!(
                "  line {}\n  ours: {:?}\n  njoy: {:?}",
                i + 1,
                a.lines().nth(i),
                b.lines().nth(i)
            );
        }
        if !d.is_empty() || n < 20 {
            bad.push((u, d.len(), n));
        }
    }
    for &(u, f) in refs {
        let r = std::fs::read_to_string(root.join(format!("tests/{refs_case}/{f}"))).unwrap();
        let d = differing(&read(&ours, u), &r).len();
        println!("{tag} unit {u} against upstream's {f}: {d} differing lines");
        if d != 0 {
            bad.push((u, d, r.lines().count()));
        }
    }
    if std::env::var_os("DECK_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    }
    assert!(bad.is_empty(), "{tag}: (unit, differing, lines) {bad:?}");
    Some(())
}

/// Upstream deck `case` whole, through [`compare`].
fn plotr_whole_deck(
    case: &str,
    resource: &str,
    unit_in: u32,
    units: &[u32],
    refs: &[(u32, &str)],
) -> Option<()> {
    let input = njoy_root().join(format!("tests/{case}/input"));
    let Ok(deck) = std::fs::read_to_string(&input) else {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", input.display());
        return None;
    };
    compare(
        &format!("deck {case}"),
        &deck,
        &[(resource, unit_in)],
        units,
        refs,
        case,
    )
}

/// **Deck 06 (C-12, Li-6 ENDF/B-V), whole deck**: PLOTR unit 31 and VIEWR unit
/// 32. Result (2026-11-13): 0 differing lines on both; unit 32 is upstream's
/// `referenceTape32`.
#[test]
fn test_06_plotr_viewr_whole_deck() {
    let _ = plotr_whole_deck("06", "t511", 30, &[31, 32], &[(32, "referenceTape32")]);
}

/// **Deck 12 (Ni-61 ENDF/B-VI.1), whole deck**: RECONR 21, GASPR 22, PLOTR 23,
/// VIEWR 24. Result (2026-11-13): 0 differing lines on all four; 22 and 24 are
/// upstream's `referenceTape22` and `referenceTape24`.
#[test]
fn test_12_plotr_viewr_whole_deck() {
    let _ = plotr_whole_deck(
        "12",
        "eni61",
        20,
        &[21, 22, 23, 24],
        &[(22, "referenceTape22"), (24, "referenceTape24")],
    );
}

/// MF=4 Legendre `P1` against energy (`gety4`, Fe-56 JEFF-3.3, `LTT = 3`) and a
/// ratio plot of MT=2 to MT=1 from two tapes (`ntp = 3`, `getz`), with VIEWR.
const DECK_GETY4_GETZ: &str = "plotr\n 41/\n /\n 1/\n 'fe56 p1'/\n /\n 3/\n /\n /\n /\n /\n 6 20 2631 4 2 0. 1/\n /\n \
1/\n 'ratio'/\n /\n 3/\n 1e5 2e7/\n /\n /\n /\n 6 20 2631 3 2 0. 1 3/\n 6 21 2631 3 1/\n /\n 99/\nviewr\n 41 42/\nstop\n";

/// Every MF=3 reaction of C-12 ENDF/B-V (`mtd = 0`): the automatic axes, the
/// passes per reaction, `rname`, and `findf` back up between passes.
const DECK_AUTO: &str = "plotr\n 43/\n /\n 1/\n 'auto'/\n /\n 4/\n /\n /\n /\n /\n 5 30 1306 3 0/\n /\n 99/\nviewr\n 43 44/\nstop\n";

/// MF=7 `S(alpha, beta)` of H in H2O against alpha (`ntp = 1`) and against
/// beta (`ntp = 2`, `nkh = 2`). PLOTR only: this port's VIEWR panics (an `i32`
/// overflow at `viewr/axis.rs:263`, a log axis whose minimum is 0) on NJOY's own
/// plot file of this deck, gh:#1543.
const DECK_MF7: &str = "plotr\n 45/\n /\n 1/\n 'h in h2o'/\n /\n 4/\n 1e-2 1e2/\n /\n 1e-6 1e1/\n /\n \
6 20 1 7 4 293.6 1 1 1/\n /\n 1/\n 'beta'/\n /\n 1/\n /\n /\n /\n /\n 6 20 1 7 4 293.6 3 2 2/\n /\n 99/\nstop\n";

/// An MF=6 LAW=1 spectrum against secondary energy (Al-27 ENDF/B-VIII.0 MT=16,
/// `nth = 5`): `gety6` reads its unsaved `ylast` at the first point.
const DECK_MF6: &str =
    "plotr\n 47/\n /\n 1/\n 'al27 mf6'/\n /\n 4/\n /\n /\n /\n /\n 6 20 1325 6 16 0. 5 1 1/\n /\n 99/\nstop\n";

/// GENDF curves after deck 01's GROUPR: MF=3 cross section from the coded copy
/// (27), the flux per unit lethargy (`nth = 0`) from the blocked-binary unit
/// (-24), a group-30 MF=6 spectrum, and the `gg3d` matrix header.
const DECK_GENDF_TAIL: &str = "moder\n -24 27/\nplotr\n 51/\n /\n 1/\n 'gendf total'/\n /\n 4/\n /\n /\n /\n /\n \
1 27 1306 3 1 296./\n /\n 2/\n 1 -24 1306 3 2 296. 0/\n /\n 1/\n 'gendf spectrum'/\n /\n 4/\n /\n /\n /\n /\n \
1 27 1306 6 2 296. 30/\n /\n 1/\n 'matrix'/\n /\n -4 2/\n /\n /\n /\n /\n /\n /\n 1 27 1306 6 2 296./\n /\n 99/\nstop\n";

/// `gety4` and `getz`. Result (2026-11-13): units 41 (9 241 lines) and 42
/// (10 275 lines) identical, first run.
#[test]
fn plotr_mf4_legendre_and_ratio_plots() {
    let fe = "n-026_Fe_056-JEFF3.3.endf";
    let _ = compare(
        "gety4-getz",
        DECK_GETY4_GETZ,
        &[(fe, 20), (fe, 21)],
        &[41, 42],
        &[],
        "",
    );
}

/// The automatic reaction loop. Result (2026-11-13): units 43 (6 591 lines)
/// and 44 (28 075 lines) identical, first run.
#[test]
fn plotr_automatic_reaction_loop() {
    let _ = compare("auto", DECK_AUTO, &[("t511", 30)], &[43, 44], &[], "");
}

/// MF=7. Result (2026-11-13): unit 45 (153 lines) identical, first run.
#[test]
fn plotr_mf7_alpha_and_beta_curves() {
    let _ = compare(
        "mf7",
        DECK_MF7,
        &[("tsl-HinH2O-ENDF8.0-Beta6.endf", 20)],
        &[45],
        &[],
        "",
    );
}

/// MF=6 2-d: undefined upstream, refused here. NJOY2016.80 runs the deck and
/// plots `8.245559e-8` at the first points, a value of an earlier LIST left in
/// `ylast`'s stack slot, where the panel's own value is `5.462661e-8`. This port
/// stops at PLOTR with the reason.
#[test]
fn plotr_mf6_curve_is_refused_as_undefined_upstream() {
    let Some((ours, _, blocked)) = run_both("mf6", DECK_MF6, &[("n-013_Al_027-ENDF8.0.endf", 20)])
    else {
        return;
    };
    let (module, why) = blocked.expect("the MF=6 curve must be refused");
    assert_eq!(module, "plotr");
    assert!(why.contains("ylast"), "{why}");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// GENDF. Result (2026-11-13): units 27 (1 330 lines) and 51 (172 lines)
/// identical, first run.
#[test]
fn plotr_gendf_curves_and_matrix_header() {
    let Ok(input) = std::fs::read_to_string(njoy_root().join("tests/01/input")) else {
        return;
    };
    let mut deck: String = input
        .lines()
        .take_while(|l| l.trim() != "stop")
        .map(|l| format!("{l}\n"))
        .collect();
    deck.push_str(DECK_GENDF_TAIL);
    let _ = compare(
        "gendf",
        &deck,
        &[("t511", 20), ("t322", 26)],
        &[27, 51],
        &[],
        "",
    );
}
