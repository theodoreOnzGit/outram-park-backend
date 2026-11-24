// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]
// Four NJOY2016 runs and three of the port on H in H2O: several minutes.
#![cfg(feature = "long-tests")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **THERMR's latent ENDF-6 branches against NJOY2016.80: `LASYM = 1`, a second
//! SCT atom (`B(7) = 0`), and the `T_eff` TAB1's interpolation law**
//! (gh:#632, gh:#636).
//!
//! # Methodology
//!
//! No held thermal tape reaches these branches: every `tsl-*` here has
//! `LASYM = 0`, a free-gas or absent second atom (`B(7) ≠ 0` or `NI = 6`) and
//! `INT = 2` effective-temperature tables. So each variant is upstream deck 68
//! (ENDF/B-VIII.0 H in H2O at 400 K, `natom = 2`, `iinc = 2`, `iform = 0`,
//! `emax = 10` eV, `tol = 0.001`), cut before its first ACER, with
//! `tsl-HinH2O-ENDF8.0-Beta6.endf` edited as text in this test:
//!
//! - **`lasym`** (`thermr.f90:2517-2531`): `LASYM` 0 → 1, and the 317 β rows
//!   mirrored to 633. The row at `β = −β_j` carries the `S(α)` rows (all 18
//!   temperatures) of `β_{j+1}` (of `β_316` for the last), so the table is
//!   asymmetric and a `|β|` lookup gives a different kernel.
//! - **`sct2`** (`:1678-1690`, `:1835-1856`, `:2588-2594`): `B(7)` 1 → 0, so
//!   oxygen (`B(8) = 3.7939`, `B(9) = 15.85751`, `B(12) = 1`) becomes an SCT
//!   atom, and a second `T_eff` TAB1 follows the first: a copy of it with
//!   `INT = 3` and the 400 K point moved to 410 K.
//! - **`teff_int`** (`:1814-1834`): the `T_eff` TAB1's `INT` 2 → 5 and its
//!   400 K point moved to 410 K, so `T_eff(400 K)` is a log-log interpolation
//!   between 373.6 K and 410 K.
//!
//! These are code-to-code devices: the edited tapes have no physical meaning.
//! NJOY2016.80 and [`run_deck`] run the same deck on the same text. **Pass
//! criterion:** unit 60 (MODER of THERMR's output) identical line for line on
//! every variant. **Control:** each variant's NJOY unit 60 differs from the
//! unedited deck 68's NJOY unit 60, so each edit reaches the output.
//!
//! # Prediction (posted on gh:#632 and gh:#636 before the first run)
//!
//! Unit 60 identical on all three; each control differs in MF=3/6 MT=221; the
//! second SCT term raises σ_inel where `calcem` reaches `sig` label 170.
//!
//! # Results (2026-11-24, NJOY2016.80 `build2/njoy`, gfortran, release)
//!
//! All three identical on the first run, 219 s for the whole test:
//!
//! | variant | unit 60 lines | MF=3/6 MT=221 | differing | control: NJOY against deck 68 |
//! |---|---|---|---|---|
//! | `lasym` | 387 188 | 386 515 | **0** | 386 508 differ |
//! | `sct2` | 345 977 | 345 304 | **0** | 73 127 differ |
//! | `teff_int` | 324 470 | 323 797 | **0** | 24 034 differ |
//!
//! Ablations (the port edited by hand for one run, then restored):
//! - `|β|` lookup restored on `lasym` (the port before gh:#632): **386 500**
//!   lines differ.
//! - `T_eff` read lin-lin, ignoring `INT` (the port before gh:#636): `teff_int`
//!   differs in **11 412** lines.
//! - Before gh:#632 `sct2` did not run at all: the deck path refused a
//!   `B(7) = 0` tape by name (`NotPorted`).

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::endf::parse::{format_endf_float, parse_endf_float};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// Deck 68 up to its first ACER (MODERs, RECONR, BROADR, THERMR, `moder -50 60`).
fn deck68(root: &Path) -> Option<String> {
    let input = std::fs::read_to_string(root.join("tests/68/input")).ok()?;
    let mut out = String::new();
    for line in input.lines() {
        if line.trim() == "acer" {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("stop\n");
    Some(out)
}

fn field(line: &str, k: usize) -> &str {
    &line[11 * k..11 * k + 11]
}

fn int_field(line: &str, k: usize) -> usize {
    field(line, k).trim().parse().unwrap()
}

fn set_field(line: &str, k: usize, text: &str) -> String {
    assert_eq!(text.len(), 11, "an ENDF field is 11 columns: {text:?}");
    format!("{}{}{}", &line[..11 * k], text, &line[11 * k + 11..])
}

fn is_mt4(line: &str) -> bool {
    line.len() >= 75 && &line[70..75] == " 7  4"
}

/// The tape as lines, and the index range of its MF=7/MT=4 lines (SEND excluded).
fn split(text: &str) -> (Vec<String>, usize, usize) {
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let start = lines.iter().position(|l| is_mt4(l)).unwrap();
    let end = start + lines[start..].iter().take_while(|l| is_mt4(l)).count();
    (lines, start, end)
}

fn lines_of(n: usize) -> usize {
    n.div_ceil(6)
}

/// MT=4's records: `[HEAD, B LIST, TAB2]` (lines), one block per β (its TAB1
/// and the `LT` temperature LISTs), and the rest (the `T_eff` TAB1(s)).
struct Mt4 {
    head: Vec<String>,
    blocks: Vec<Vec<String>>,
    rest: Vec<String>,
}

fn parse_mt4(mt4: &[String]) -> Mt4 {
    let npl = int_field(&mt4[1], 4);
    let mut k = 2 + lines_of(npl);
    let nr2 = int_field(&mt4[k], 4);
    let nb = int_field(&mt4[k], 5);
    k += 1 + lines_of(2 * nr2);
    let head = mt4[..k].to_vec();
    let mut blocks = Vec::with_capacity(nb);
    for _ in 0..nb {
        let s = k;
        let lt = int_field(&mt4[k], 2);
        let (nr, np) = (int_field(&mt4[k], 4), int_field(&mt4[k], 5));
        k += 1 + lines_of(2 * nr) + lines_of(2 * np);
        for _ in 0..lt {
            k += 1 + lines_of(int_field(&mt4[k], 4));
        }
        blocks.push(mt4[s..k].to_vec());
    }
    Mt4 { head, blocks, rest: mt4[k..].to_vec() }
}

/// The `T_eff` TAB1 at the start of `rest`, with its law and 400 K point edited.
fn edited_teff(rest: &[String], int: usize) -> Vec<String> {
    let nr = int_field(&rest[0], 4);
    let np = int_field(&rest[0], 5);
    assert_eq!(nr, 1);
    let n = 2 + lines_of(2 * np);
    let mut t: Vec<String> = rest[..n].to_vec();
    t[1] = set_field(&t[1], 1, &format!("{int:>11}"));
    let mut moved = 0;
    for l in t[2..].iter_mut() {
        for k in [0, 2, 4] {
            if field(l, k) == " 4.000000+2" {
                *l = set_field(l, k, " 4.100000+2");
                moved += 1;
            }
        }
    }
    assert_eq!(moved, 1, "the T_eff TAB1 has one 400 K point");
    t
}

fn join(lines: &[String], start: usize, end: usize, mt4: Vec<String>) -> String {
    let mut out: Vec<String> = lines[..start].to_vec();
    out.extend(mt4);
    out.extend(lines[end..].iter().cloned());
    out.join("\n") + "\n"
}

/// `LASYM = 1` with mirrored, asymmetric β rows.
fn lasym(text: &str) -> String {
    let (lines, start, end) = split(text);
    let m = parse_mt4(&lines[start..end]);
    let nb = m.blocks.len();
    let mut head = m.head.clone();
    head[0] = set_field(&head[0], 4, &format!("{:>11}", 1));
    let tab2 = head.len() - 2;
    head[tab2] = set_field(&head[tab2], 5, &format!("{:>11}", 2 * nb - 1));
    head[tab2 + 1] = set_field(&head[tab2 + 1], 0, &format!("{:>11}", 2 * nb - 1));
    let mut out = head;
    for j in (1..nb).rev() {
        let beta = parse_endf_float(field(&m.blocks[j][0], 1)).unwrap();
        assert!(beta > 0.0);
        let neg = format_endf_float(-beta);
        let mut b = m.blocks[(j + 1).min(nb - 1)].clone();
        b[0] = set_field(&b[0], 1, &neg);
        out.extend(b);
    }
    assert_eq!(parse_endf_float(field(&m.blocks[0][0], 1)).unwrap(), 0.0, "the first β is 0");
    for b in &m.blocks {
        out.extend(b.iter().cloned());
    }
    out.extend(m.rest.iter().cloned());
    join(&lines, start, end, out)
}

/// `B(7) = 0` (oxygen an SCT atom) and a second `T_eff` TAB1.
fn sct2(text: &str) -> String {
    let (lines, start, end) = split(text);
    let m = parse_mt4(&lines[start..end]);
    let mut head = m.head.clone();
    // B(7..12) is the B LIST's second data line.
    assert_eq!(field(&head[3], 0), " 1.000000+0", "B(7) = 1 on tsl-HinH2O");
    head[3] = set_field(&head[3], 0, " 0.000000+0");
    let teff2 = edited_teff(&m.rest, 3);
    let mut out = head;
    for b in &m.blocks {
        out.extend(b.iter().cloned());
    }
    out.extend(m.rest.iter().cloned());
    out.extend(teff2);
    join(&lines, start, end, out)
}

/// The `T_eff` TAB1 at `INT = 5`, its 400 K point moved to 410 K.
fn teff_int(text: &str) -> String {
    let (lines, start, end) = split(text);
    let m = parse_mt4(&lines[start..end]);
    let teff = edited_teff(&m.rest, 5);
    let n = teff.len();
    assert_eq!(n, m.rest.len(), "tsl-HinH2O has one T_eff TAB1");
    let mut out = m.head.clone();
    for b in &m.blocks {
        out.extend(b.iter().cloned());
    }
    out.extend(teff);
    join(&lines, start, end, out)
}

fn lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap_or_default().lines().map(str::to_string).collect()
}

fn differing(a: &[String], b: &[String]) -> Vec<usize> {
    (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect()
}

/// Lines of `MF=3/6 MT=221` in `v`.
fn mt221(v: &[String]) -> usize {
    v.iter().filter(|l| l.len() >= 75 && (&l[70..75] == " 3221" || &l[70..75] == " 6221")).count()
}

/// Run one variant through NJOY and the port; return (ours, NJOY's) unit 60.
fn run_variant(root: &Path, tmp: &Path, name: &str, tsl: &str, deck: &str) -> (Vec<String>, Vec<String>) {
    let bin = root.join("build/njoy");
    let h1 = root.join("tests/resources/n-001_H_001-ENDF8.0-Beta6.endf");
    let (ours, theirs) = (tmp.join(format!("{name}_ours")), tmp.join(format!("{name}_njoy")));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&h1, d.join("tape20")).unwrap();
        std::fs::write(d.join("tape21"), tsl).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 failed on variant {name}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "variant {name} must run to its end");
    (lines(&ours.join("tape60")), lines(&theirs.join("tape60")))
}

#[test]
fn thermr_lasym1_second_sct_atom_and_teff_law_match_njoy2016() {
    let root = njoy_root();
    let tsl = root.join("tests/resources/tsl-HinH2O-ENDF8.0-Beta6.endf");
    for p in [root.join("build/njoy"), tsl.clone()] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT)", p.display());
            return;
        }
    }
    let Some(deck) = deck68(&root) else { return };
    let text = std::fs::read_to_string(&tsl).unwrap();
    let tmp = std::env::temp_dir().join(format!("njoy_thermr_latent_{}", std::process::id()));

    // Control: deck 68 as NJOY runs it.
    let base_dir = tmp.join("base_njoy");
    std::fs::create_dir_all(&base_dir).unwrap();
    std::fs::copy(root.join("tests/resources/n-001_H_001-ENDF8.0-Beta6.endf"), base_dir.join("tape20")).unwrap();
    std::fs::write(base_dir.join("tape21"), &text).unwrap();
    std::fs::write(base_dir.join("deck"), &deck).unwrap();
    let (ok, _) = common::run_njoy_dated(&root.join("build/njoy"), &base_dir);
    assert!(ok, "NJOY2016 failed on deck 68");
    let base = lines(&base_dir.join("tape60"));

    let mut bad = Vec::new();
    for (name, tsl) in [("lasym", lasym(&text)), ("sct2", sct2(&text)), ("teff_int", teff_int(&text))] {
        let (a, b) = run_variant(&root, &tmp, name, &tsl, &deck);
        let d = differing(&a, &b);
        let ctrl = differing(&b, &base);
        println!(
            "{name}: {} differing lines of {} (NJOY: {} MF=3/6 MT=221 lines); control, NJOY {name} against deck 68: {} lines differ",
            d.len(),
            b.len(),
            mt221(&b),
            ctrl.len()
        );
        for &i in d.iter().take(6) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.get(i), b.get(i));
        }
        assert!(mt221(&b) > 100, "{name}: NJOY's unit 60 must carry MT=221");
        assert!(!ctrl.is_empty(), "{name}: control, the edit must reach NJOY's output");
        if !d.is_empty() {
            bad.push((name, d.len()));
        }
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(bad.is_empty(), "unit 60 differs from NJOY2016: {bad:?}");
}
