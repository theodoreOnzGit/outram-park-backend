// SPDX-License-Identifier: AGPL-3.0-only
//! **GROUPR `getmf6` LAW = 4 and LAW = 5 for neutrons, against NJOY2016.80**
//! (gh:#1370), on a synthetic tape derived from Si-30.
//!
//! # Methodology
//!
//! No tape in `reference-data/endf/` has a neutron File-6 subsection with
//! `LAW = 4` or `LAW = 5`, so the maintainer approved (2026-11-14) a check on a
//! synthetic ("dummy") input fed to both codes. [`synthetic_tape`] builds it in
//! this file from Si-30 ENDF/B-VIII.0 (`n-014_Si_030-ENDF8.0.endf`, MAT 1431),
//! whose MF=6 MT 51-56 are a neutron `LAW = 2` subsection followed by a Si-30
//! recoil `LAW = 4` one, by rewriting MF=6 MT 51-55 and their MF=1 directory
//! counts; everything else, MT 56 included (the `LAW = 2` control), is the
//! evaluation's own text. It is not physics and nothing is claimed from it beyond
//! "the two codes agree on identical input".
//!
//! | MT | first subsection | neutron subsection | upstream path |
//! |---|---|---|---|
//! | 51 | Si-30, `LAW = 2` (the evaluation's neutron Legendre data) | `LAW = 4` | `getfle` reads the first subsection's LISTs; `getdis` flips `wqp` |
//! | 52 | Si-30, `LAW = 3` | `LAW = 4` | `law = -4`, `iso = 1`; `getdis` flips `wqp` |
//! | 53 | neutron, `LAW = 5`, `LTP = 2`, `a_0 /= 1` | (same) | `getco` `lang = 2` copy |
//! | 54 | neutron, `LAW = 5`, `LTP = 2`, `SPI = 2.5`, `LIDP = 1` | (same) | as 53; `SPI`, `LIDP` unused for a neutron |
//! | 55 | neutron, `LAW = 5`, `LTP = 14` | (same) | refused: `conver` mangles it (below) |
//!
//! The deck `L45` (ours) runs MODER, RECONR at 0.005, BROADR to 293.6 K, then
//! GROUPR with LANL-30, `iwt = 3`, `lord = 3`, one sigma-zero and card 9
//! `3 1/ 3 51/` and `8 51/` through `8 54/` and `8 56/`, GENDF as text on unit 25.
//! NJOY2016.80 (`$NJOY2016_ROOT/build/njoy`) and [`run_deck`] run the same deck on
//! the same tape. Pass criterion: unit 25 identical line for line, compared as
//! strings.
//!
//! # Prediction (posted on gh:#1370 before the first run)
//!
//! Unit 25 identical. At that time MT 54 was `LTP = 12` and MT 55 `LTP = 14`.
//!
//! # Results (2026-11-14, NJOY2016.80, release)
//!
//! - **First run** (MT 54 `LTP = 12`, MT 55 `LTP = 14`): 114 of 326 lines differed,
//!   all of them in MF=6 MT 54 and 55 (36 and 35 lines); MT 51, 52, 53 and 56 were
//!   identical. An instrumented NJOY2016 (`write` in `getfle`/`getco`) showed why:
//!   GROUPR's `conver` (`groupr.f90:11572-11624`) rewrites every `LAW = 5`
//!   `LTP > 2` record as `LTP = 2` after multiplying `p` by `sig(E)(1-mu)`, and the
//!   `getsig` it calls leaves `l1h`/`n2h` at the PENDF's last record, which
//!   `getco` then reads as `lang` and `nmu`. The records read back as `NW = 1`,
//!   `a_0 = 0`. That is refused by name (`File4Angular::from_mf6_law5`), the deck
//!   no longer asks for MT 55, and MT 54 became `LTP = 2` with `SPI`/`LIDP` set.
//! - **Second run**: unit 25 identical, 295 of 295 lines.
//! - **Ablations**: without the LAW-4 cosine flip, MT 51 differs in 34 of 44 lines
//!   and MT 52 in 32 of 41; with `a_0` forced to 1, MT 53 differs in 30 of 37 and
//!   MT 54 in 29 of 37; MT 56 (control) in 0 of 37. Record: deck-driver README §35a.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// An ENDF `E11.4`-style real, `" 1.234567+6"`.
fn ef(x: f64) -> String {
    if x == 0.0 {
        return " 0.000000+0".to_string();
    }
    let s = format!("{:.6e}", x.abs());
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let sign = if x < 0.0 { "-" } else { " " };
    if e.abs() < 10 {
        format!("{sign}{m}{}{}", if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        let s = format!("{:.5e}", x.abs());
        let (m, e) = s.split_once('e').unwrap();
        let e: i32 = e.parse().unwrap();
        format!("{sign}{m}{}{}", if e < 0 { '-' } else { '+' }, e.abs())
    }
}

fn ei(i: i64) -> String {
    format!("{i:>11}")
}

/// Parse an ENDF real field such as `"2.310200+6"`.
fn pf(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    let b = t.as_bytes();
    let k = (1..b.len()).rev().find(|&i| (b[i] == b'+' || b[i] == b'-') && b[i - 1] != b'e' && b[i - 1] != b'E');
    match k {
        Some(k) => format!("{}e{}", &t[..k], &t[k..]).parse().unwrap(),
        None => t.parse().unwrap(),
    }
}

fn pi(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

fn field(line: &str, k: usize) -> &str {
    &line[11 * k..11 * (k + 1)]
}

fn set_field(line: &str, k: usize, v: &str) -> String {
    format!("{}{}{}", &line[..11 * k], v, &line[11 * (k + 1)..])
}

/// A data line for MF=6/`mt` of MAT 1431 (sequence filled in later).
fn data_line(f: &[String]) -> String {
    let mut s: String = f.concat();
    while s.len() < 66 {
        s.push(' ');
    }
    s
}

fn reals(v: &[f64]) -> Vec<String> {
    let mut out = Vec::new();
    for c in v.chunks(6) {
        out.push(data_line(&c.iter().map(|&x| ef(x)).collect::<Vec<_>>()));
    }
    out
}

fn cont(c1: f64, c2: f64, l1: i64, l2: i64, n1: i64, n2: i64) -> String {
    data_line(&[ef(c1), ef(c2), ei(l1), ei(l2), ei(n1), ei(n2)])
}

fn lines_for(n: i64) -> usize {
    (n.max(0) as usize).div_ceil(6)
}

/// The record layout of one original MF=6/MT section (body only, 66 columns):
/// HEAD, neutron TAB1 + its `LAW = 2` TAB2/LISTs, recoil TAB1.
struct Parsed {
    head: String,
    tab1_n: Vec<String>,
    data_n: Vec<String>,
    tab1_r: Vec<String>,
    /// First and last incident energy of the neutron LISTs.
    e_first: f64,
    e_last: f64,
}

fn tab1_len(head: &str) -> usize {
    1 + lines_for(2 * pi(field(head, 4))) + lines_for(2 * pi(field(head, 5)))
}

fn parse_section(body: &[String]) -> Parsed {
    let head = body[0].clone();
    let mut i = 1;
    let k = tab1_len(&body[i]);
    let tab1_n = body[i..i + k].to_vec();
    i += k;
    let start = i;
    let tab2 = &body[i];
    let ne = pi(field(tab2, 5));
    i += 1 + lines_for(2 * pi(field(tab2, 4)));
    let (mut e_first, mut e_last) = (0.0, 0.0);
    for j in 0..ne {
        let l = &body[i];
        let e = pf(field(l, 1));
        if j == 0 {
            e_first = e;
        }
        e_last = e;
        i += 1 + lines_for(pi(field(l, 4)));
    }
    let data_n = body[start..i].to_vec();
    let k = tab1_len(&body[i]);
    let tab1_r = body[i..i + k].to_vec();
    assert_eq!(i + k, body.len(), "unexpected MF=6 layout");
    Parsed { head, tab1_n, data_n, tab1_r, e_first, e_last }
}

fn with_law(tab1: &[String], law: i64) -> Vec<String> {
    let mut v = tab1.to_vec();
    v[0] = set_field(&v[0], 3, &ei(law));
    v
}

/// One `LAW = 5` LIST `[0, E, LTP, 0, NW, NL]`.
fn law5_list(e: f64, ltp: i64, data: &[f64], nl: i64) -> Vec<String> {
    let mut v = vec![cont(0.0, e, ltp, 0, data.len() as i64, nl)];
    v.extend(reals(data));
    v
}

/// The `LAW = 5` body for MT 53-55: TAB2 `[SPI, 0, LIDP, 0, 1, 3]`, lin-lin in
/// energy, three LISTs at the evaluation's first energy, 10 MeV and its last.
/// MT 53 and 54 are `LTP = 2` (54 with `SPI = 2.5`, `LIDP = 1`, which a neutron
/// ignores); MT 55 is `LTP = 14`, which upstream's `conver` mangles (refused).
fn law5_body(mt: i32, e1: f64, e3: f64) -> Vec<String> {
    let mut v = if mt == 54 { vec![cont(2.5, 0.0, 1, 0, 1, 3)] } else { vec![cont(0.5, 0.0, 0, 0, 1, 3)] };
    v.push(data_line(&[ei(3), ei(2)]));
    let mu = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let pairs = |p: &[f64]| -> Vec<f64> { mu.iter().zip(p).flat_map(|(&m, &y)| [m, y]).collect() };
    match mt {
        53 => {
            v.extend(law5_list(e1, 2, &[0.5, 0.1], 1));
            v.extend(law5_list(1.0e7, 2, &[0.45, 0.2, 0.05], 2));
            v.extend(law5_list(e3, 2, &[0.4, 0.25, 0.1, 0.02], 3));
        }
        54 => {
            v.extend(law5_list(e1, 2, &[1.2, -0.3], 1));
            v.extend(law5_list(1.0e7, 2, &[1.1, 0.4, 0.1, -0.05], 3));
            v.extend(law5_list(e3, 2, &[1.0, 0.6, 0.2], 2));
        }
        _ => {
            v.extend(law5_list(e1, 14, &pairs(&[0.2, 0.35, 0.5, 0.65, 0.8]), 5));
            v.extend(law5_list(1.0e7, 14, &pairs(&[0.1, 0.2, 0.4, 0.8, 1.5]), 5));
            v.extend(law5_list(e3, 14, &pairs(&[0.05, 0.1, 0.3, 0.9, 2.5]), 5));
        }
    }
    v
}

/// The synthetic tape: Si-30 with MF=6 MT 51-55 rewritten as in the module doc,
/// and the MF=1 directory's line counts brought in line.
fn synthetic_tape(si30: &str) -> String {
    let lines: Vec<&str> = si30.lines().collect();
    let key = |l: &str| -> (i32, i32) {
        if l.len() < 75 {
            return (0, 0);
        }
        (l[70..72].trim().parse().unwrap_or(0), l[72..75].trim().parse().unwrap_or(0))
    };
    let mut new_body: std::collections::BTreeMap<i32, Vec<String>> = Default::default();
    for mt in 51..=55 {
        let body: Vec<String> = lines.iter().filter(|l| key(l) == (6, mt)).map(|l| l[..66].to_string()).collect();
        let p = parse_section(&body);
        let mut out = Vec::new();
        match mt {
            51 => {
                out.push(p.head.clone());
                out.extend(with_law(&p.tab1_r, 2));
                out.extend(p.data_n.clone());
                out.extend(with_law(&p.tab1_n, 4));
            }
            52 => {
                out.push(p.head.clone());
                out.extend(with_law(&p.tab1_r, 3));
                out.extend(with_law(&p.tab1_n, 4));
            }
            _ => {
                out.push(set_field(&p.head, 4, &ei(1)));
                out.extend(with_law(&p.tab1_n, 5));
                out.extend(law5_body(mt, p.e_first, p.e_last));
            }
        }
        new_body.insert(mt, out);
    }
    let mut text = String::new();
    let mut done = std::collections::BTreeSet::new();
    for l in &lines {
        let (mf, mt) = key(l);
        if mf == 6 && (51..=55).contains(&mt) {
            if done.insert(mt) {
                for (ns, b) in new_body[&mt].iter().enumerate() {
                    text.push_str(&format!("{b}1431 6{mt:>3}{:>5}\n", ns + 1));
                }
            }
            continue;
        }
        if mf == 1 && mt == 451 && l.len() >= 66 && pi(field(l, 2)) == 6 && (51..=55).contains(&(pi(field(l, 3)) as i32)) && field(l, 0).trim().is_empty() {
            let n = new_body[&(pi(field(l, 3)) as i32)].len() as i64;
            text.push_str(&set_field(l, 4, &ei(n)));
            text.push('\n');
            continue;
        }
        text.push_str(l);
        text.push('\n');
    }
    text
}

/// The `L45` deck.
const L45: &str = "moder\n20 -21/\nreconr\n-21 -22/\n'si30 law 4/5'/\n1431/\n0.005/\n0/\n\
broadr\n-21 -22 -23/\n1431 1 0 0 0/\n0.001/\n293.6/\n0/\n\
groupr\n-21 -23 0 25/\n1431 3 0 3 3 1 1 1/\n'si30 law 4/5'/\n293.6/\n1.e10/\n\
3 1/\n3 51/\n8 51/\n8 52/\n8 53/\n8 54/\n8 56/\n0/\n0/\nstop\n";

/// `L45` asking for MT 55 (`LAW = 5`, `LTP = 14`) as well: refused by name.
const L45_WITH_55: &str = "moder\n20 -21/\nreconr\n-21 -22/\n'si30 law 4/5'/\n1431/\n0.005/\n0/\n\
broadr\n-21 -22 -23/\n1431 1 0 0 0/\n0.001/\n293.6/\n0/\n\
groupr\n-21 -23 0 25/\n1431 3 0 3 3 1 1 1/\n'si30 law 4/5'/\n293.6/\n1.e10/\n\
3 1/\n8 55/\n0/\n0/\nstop\n";

/// Runs `deck` through both codes on `tape`; returns (ours, NJOY's) unit 25.
fn run_both(tag: &str, deck: &str, tape: &str) -> Option<(String, String)> {
    let bin = njoy_root().join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_law45_{tag}_{}", std::process::id()));
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
    let read = |d: &Path| std::fs::read_to_string(d.join("tape25")).unwrap_or_default();
    let out = (read(&ours), read(&theirs));
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(out)
}

fn si30_text() -> Option<String> {
    let res = njoy_outram_park_fork::reference_data::reference_endf("n-014_Si_030-ENDF8.0.endf")?;
    Some(std::fs::read_to_string(res).unwrap())
}

/// The synthetic tape's MF=6 sections read back through the crate's own
/// `File4Angular` readers: each law reaches the reader it is meant to, and the
/// tape structure is what the module doc says (this guards the generator).
#[test]
fn synthetic_tape_has_the_intended_laws() {
    let Some(si) = si30_text() else {
        eprintln!("skip: Si-30 tape not present");
        return;
    };
    let dir = std::env::temp_dir().join(format!("njoy_law45_tape_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("tape20");
    std::fs::write(&p, synthetic_tape(&si)).unwrap();
    let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&p).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    use njoy_outram_park_fork::groupr::file4::{File4Angular, NLD};
    let a51 = File4Angular::from_mf6_law4(&tape, 1431, 51, 1, NLD).unwrap();
    assert!(!a51.isotropic && a51.len() > 10);
    assert!(File4Angular::from_mf6_law4(&tape, 1431, 52, 1, NLD).unwrap().isotropic);
    let a53 = File4Angular::from_mf6_law5(&tape, 1431, 53, 1, NLD).unwrap();
    // `a_0` is copied, not set to 1 (`getco:10563-10573`).
    let c = a53.coefficients_at(a53.first_energy(), NLD).unwrap();
    assert_eq!((c.fle[0], c.fle[1], c.fle[2], c.nle), (0.5, 0.1, 0.0, 3));
    let a54 = File4Angular::from_mf6_law5(&tape, 1431, 54, 1, NLD).unwrap();
    let c = a54.coefficients_at(a54.first_energy(), NLD).unwrap();
    assert_eq!((c.fle[0], c.fle[1]), (1.2, -0.3));
    assert!(File4Angular::from_mf6_law5(&tape, 1431, 55, 1, NLD).is_err());
    // A LAW = 2 subsection is not read as LAW = 4 or 5.
    assert!(File4Angular::from_mf6_law4(&tape, 1431, 56, 1, NLD).is_err());
    assert!(File4Angular::from_mf6_law5(&tape, 1431, 56, 1, NLD).is_err());
}

/// The named refusals: `LTP = 1` and `LTP > 2` for a neutron (undefined upstream),
/// a request for `LAW = 4` on a subsection that is not, and a `LAW = 4` whose first
/// subsection is not `LAW = 2/3`.
#[test]
fn law4_and_law5_refuse_undefined_input_by_name() {
    let Some(si) = si30_text() else {
        eprintln!("skip: Si-30 tape not present");
        return;
    };
    let tape_text = synthetic_tape(&si);
    // MT 53: turn every LTP = 2 LIST into LTP = 1.
    let ltp1 = tape_text
        .lines()
        .map(|l| {
            if l.len() >= 75 && &l[70..75] == " 6 53" && l[22..33].trim() == "2" && pf(field(l, 0)) == 0.0 && pf(field(l, 1)) > 0.0 && l[33..44].trim() == "0" {
                set_field(l, 2, &ei(1))
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // MT 51: relabel the first (Si-30) subsection LAW = 5.
    let first5 = tape_text
        .lines()
        .map(|l| if l.len() >= 80 && &l[70..80] == " 6 51    2" { set_field(l, 3, &ei(5)) } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    let dir = std::env::temp_dir().join(format!("njoy_law45_refuse_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    use njoy_outram_park_fork::groupr::file4::{File4Angular, NLD};
    use njoy_outram_park_fork::NjoyError;
    let read = |name: &str, text: &str| {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        njoy_outram_park_fork::endf::tape::Tape::read_file(&p).unwrap()
    };
    let t = read("ltp1", &ltp1);
    let e = File4Angular::from_mf6_law5(&t, 1431, 53, 1, NLD).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(m) if m.contains("LTP = 1")), "{e:?}");
    let t = read("first5", &first5);
    let e = File4Angular::from_mf6_law4(&t, 1431, 51, 1, NLD).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(m) if m.contains("not LAW = 2 or 3")), "{e:?}");
    let t = read("synthetic", &tape_text);
    // MT 55 is LTP = 14.
    let e = File4Angular::from_mf6_law5(&t, 1431, 55, 1, NLD).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(m) if m.contains("LTP > 2")), "{e:?}");
    // MT 53's only subsection is LAW = 5: asking it for LAW = 4 is refused.
    let e = File4Angular::from_mf6_law4(&t, 1431, 53, 1, NLD).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(m) if m.contains("not LAW = 4")), "{e:?}");
    // Si-30's own MT 56 asked for its recoil (ZAP 14030, LAW = 4, AWP = 29.716)
    // is refused for AWP /= 1.
    let e = File4Angular::from_mf6_law4(&t, 1431, 56, 14030, NLD).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(m) if m.contains("AWP")), "{e:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The deck driver stops at GROUPR, by name, when card 9 asks for MT 55.
#[test]
fn l45_deck_with_ltp14_is_refused_by_name() {
    let Some(si) = si30_text() else {
        eprintln!("skip: Si-30 tape not present");
        return;
    };
    let dir = std::env::temp_dir().join(format!("njoy_law45_deck55_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tape20"), synthetic_tape(&si)).unwrap();
    let run = run_deck(L45_WITH_55, &DeckOptions { workdir: Some(dir.clone()), parse_only: false, run_date: None }).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let (module, why) = run.blocked.expect("MT 55 must be refused");
    assert_eq!(module, "groupr");
    assert!(why.contains("LTP > 2"), "{why}");
}

/// **`L45`: File-6 `LAW = 4` and `LAW = 5` neutrons grouped through `getdis`.**
#[test]
fn l45_groupr_mf6_law4_law5_synthetic_si30() {
    let Some(si) = si30_text() else {
        eprintln!("skip: Si-30 tape not present");
        return;
    };
    let Some((a, b)) = run_both("l45", L45, &synthetic_tape(&si)) else { return };
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let diff: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("L45 unit 25: {} differing lines of {}", diff.len(), bl.len());
    for &i in diff.iter().take(8) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    for mt in [51, 52, 53, 54, 56] {
        let n = bl.iter().filter(|l| l.len() >= 75 && &l[70..75] == format!(" 6{mt:>3}")).count();
        println!("  MF=6 MT={mt}: {n} lines");
        assert!(n > 5, "NJOY wrote no MF=6/MT={mt}");
    }
    assert!(bl.len() > 100, "NJOY's unit 25 has only {} lines", bl.len());
    assert!(diff.is_empty(), "L45: {} of {} lines differ", diff.len(), bl.len());
}
