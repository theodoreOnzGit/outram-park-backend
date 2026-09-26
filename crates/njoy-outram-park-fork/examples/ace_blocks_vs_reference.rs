// SPDX-License-Identifier: GPL-3.0

//! **Every block of an ACE table, ours against NJOY2016's own** — the
//! reference tables in the `reference-data/ace` submodule.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --example ace_blocks_vs_reference -- U234 293.6K
//! cargo run --release -p njoy-outram-park-fork --example ace_blocks_vs_reference -- U235 0K
//! ```
//!
//! # Method
//!
//! Ours is built by the same deck NJOY's was, through the one assembly path the
//! crate has: `0K` is `RECONR -> ACER`; `293.6K` is
//! `RECONR -> BROADR -> PURR -> ACER` (PURR at the
//! reference deck's own 20 bins / 64 ladders), all through `acer::build_deck`
//! with `AceDeck::without_heatr()` because the reference decks run no HEATR --
//! their PENDF has no MT=301 and `acelod` writes a zero heating column. Both
//! tables are then **decoded by
//! the same readers** and compared block by block as *content*, so a block that
//! is right but laid out at a different offset is not reported as wrong, and a
//! block that is missing is reported as missing rather than as a large number.
//!
//! Grid-independent blocks are also compared **word for word** from their
//! locator to the next block present (`... words` rows): both sides pass
//! through Type-1 text, so equality there is equality of the printed value.
//! `OUTRAM_DIFF_DUMP=1` prints the differing words; `OUTRAM_SPAN_DIR=<dir>`
//! writes each compared span of both tables to files.
//!
//! Energy-grid-dependent blocks (ESZ, SIG) are compared at the grid points the
//! two tables **share**, because the two RECONR runs choose different adaptive
//! grids; the shared fraction is printed so the reader can see how much that
//! covers. Everything else is grid-independent and is compared value for value.
//!
//! The report ends with a verdict line per block: `SAME`, `CLOSE (worst …)`,
//! `DIFFERENT`, or `MISSING in ours` / `MISSING in NJOY's`.

use njoy_outram_park_fork::acer::ce_decode::{decode_ce, CeNeutronAce};
use njoy_outram_park_fork::acer::ce_laws::{decode_angular, decode_energy_law, n_neutron_reactions};
use njoy_outram_park_fork::acer::delayed::decode_delayed;
use njoy_outram_park_fork::acer::photon_read::{decode_photon_production, AcePhotonRate};
use njoy_outram_park_fork::acer::read::{self, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::{build_deck, jxs, AceDeck};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};
use std::collections::{BTreeMap, BTreeSet};

const K_BOLTZMANN_MEV: f64 = 8.617_333_262e-11;

/// `(name, MAT, tape)` for the evaluations the reference library holds.
const KNOWN: &[(&str, i32, &str)] = &[
    ("U234", 9225, "n-092_U_234-ENDF8.0.endf"),
    ("U235", 9228, "n-092_U_235-ENDF8.0.endf"),
    ("U238", 9237, "n-092_U_238.endf"),
];

/// Worst relative difference `|a-b|/|b|`, absolute where `b == 0`.
fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    }
}

fn verdict(worst: f64) -> String {
    if worst == 0.0 {
        "SAME".into()
    } else if worst < 1.0e-6 {
        format!("CLOSE (worst {worst:.2e})")
    } else {
        format!("DIFFERENT (worst {worst:.2e})")
    }
}

/// The blocks, in JXS order, with their 0-based JXS index.
const BLOCKS: &[(&str, usize)] = &[
    ("ESZ", jxs::ESZ),
    ("NU", jxs::NU),
    ("MTR", jxs::MTR),
    ("LQR", jxs::LQR),
    ("TYR", jxs::TYR),
    ("LSIG", jxs::LSIG),
    ("SIG", jxs::SIG),
    ("LAND", jxs::LAND),
    ("AND", jxs::AND),
    ("LDLW", jxs::LDLW),
    ("DLW", jxs::DLW),
    ("GPD", 11),
    ("MTRP", jxs::MTRP),
    ("LSIGP", jxs::LSIGP),
    ("SIGP", jxs::SIGP),
    ("LANDP", jxs::LANDP),
    ("ANDP", jxs::ANDP),
    ("LDLWP", jxs::LDLWP),
    ("DLWP", jxs::DLWP),
    ("YP", 19),
    ("FIS", 20),
    ("LUNR", jxs::LUNR),
    ("DNU", jxs::DNU),
    ("BDD", jxs::BDD),
    ("DNEDL", jxs::DNEDL),
    ("DNED", jxs::DNED),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args.first().map(String::as_str).unwrap_or("U234");
    let temp_dir = args.get(1).map(String::as_str).unwrap_or("293.6K");
    let Some(&(_, mat, tape_file)) = KNOWN.iter().find(|(n, _, _)| *n == name) else {
        eprintln!("unknown nuclide {name}; known: U234, U235, U238");
        std::process::exit(2);
    };
    let rel_path = format!("reference-njoy/endf-b-viii.0/{temp_dir}/{name}.ace.gz");
    let Some(ref_path) = ace_reference_file_or_skip(&rel_path, "ace-blocks") else {
        return;
    };
    let theirs = read::read(&ref_path).expect("read NJOY's table");
    // The deck's own card temperature (`293.6/`), not one recovered from the
    // table's printed kT: 2.5301e-8 MeV / k_B is not exactly 293.6 K.
    let t_k = temp_dir
        .trim_end_matches('K')
        .parse::<f64>()
        .unwrap_or(theirs.header.kt_mev / K_BOLTZMANN_MEV);
    let with_purr = theirs.jxs[jxs::LUNR] > 0;

    // ── Ours, by the same deck ─────────────────────────────────────────────
    let tape_path = reference_endf(tape_file).expect("reference tape");
    let tape = Tape::read_file(&tape_path).expect("parse tape");
    let t0 = std::time::Instant::now();
    let recon0 = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let recon = if t_k > 0.0 {
        broaden_result(&recon0, t_k)
    } else {
        recon0
    };
    let kt = theirs.header.kt_mev;
    let ace = if with_purr {
        build_deck(&tape, mat, &recon, kt, 0, &AceDeck::default().without_heatr().with_purr(20, 64, 10_000))
            .expect("build with PURR")
    } else {
        build_deck(&tape, mat, &recon, kt, 0, &AceDeck::default().without_heatr()).expect("build")
    };
    // Through Type-1 TEXT, so ours is compared exactly as it would be read
    // from a file, not as an in-memory structure NJOY never had.
    let text = ace
        .to_raw(AceFileType::Type1Ascii)
        .expect("to raw")
        .to_type1_string();
    let ours = read::parse_type1(&text).expect("parse ours");
    println!(
        "{name} {temp_dir}: built ours in {:.1} s ({} deck)",
        t0.elapsed().as_secs_f64(),
        if with_purr { "RECONR+BROADR+PURR+ACER" } else { "RECONR+ACER" }
    );

    report(name, temp_dir, &ours, &theirs, t_k);
}

fn report(name: &str, temp_dir: &str, ours: &RawAceTable, theirs: &RawAceTable, t_k: f64) {
    let mut verdicts: Vec<(String, String)> = Vec::new();

    // ── Header ─────────────────────────────────────────────────────────────
    println!("\n== {name} {temp_dir}: header");
    println!(
        "   ZAID  ours {:<12} NJOY {}",
        ours.header.zaid, theirs.header.zaid
    );
    let hdr = [
        rel(ours.header.awr, theirs.header.awr),
        rel(ours.header.kt_mev, theirs.header.kt_mev),
    ]
    .into_iter()
    .fold(0.0f64, f64::max);
    verdicts.push((
        "header (ZAID, AWR, kT)".into(),
        if ours.header.zaid != theirs.header.zaid {
            "DIFFERENT (ZAID)".into()
        } else {
            verdict(hdr)
        },
    ));

    // ── NXS ────────────────────────────────────────────────────────────────
    println!("\n== NXS (1-based)");
    let mut nxs_diff = Vec::new();
    for i in 0..16 {
        let (a, b) = (ours.nxs[i], theirs.nxs[i]);
        if a != b {
            nxs_diff.push(format!("NXS({}) ours {a} NJOY {b}", i + 1));
        }
    }
    for d in &nxs_diff {
        println!("   {d}");
    }
    verdicts.push((
        "NXS".into(),
        if nxs_diff.is_empty() {
            "SAME".into()
        } else {
            format!("DIFFERENT ({} of 16)", nxs_diff.len())
        },
    ));

    // ── Block presence ─────────────────────────────────────────────────────
    println!("\n== block presence (JXS != 0)");
    let mut presence: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    for &(b, j) in BLOCKS {
        let (a, t) = (ours.jxs[j] > 0, theirs.jxs[j] > 0);
        presence.insert(b, (a, t));
        if a != t {
            println!(
                "   {b:<6} {}",
                if t { "MISSING in ours" } else { "MISSING in NJOY's" }
            );
        }
    }

    // ── ESZ and SIG at shared grid points ──────────────────────────────────
    let (Ok(co), Ok(ct)) = (decode_ce(ours), decode_ce(theirs)) else {
        println!("   cannot decode one of the CE tables; stopping");
        return;
    };
    esz_and_sig(&co, &ct, &mut verdicts);

    // ── Reaction list, Q-values, yields ────────────────────────────────────
    let mo: Vec<i32> = co.reactions.iter().map(|r| r.mt).collect();
    let mt: Vec<i32> = ct.reactions.iter().map(|r| r.mt).collect();
    let so: BTreeSet<i32> = mo.iter().copied().collect();
    let st: BTreeSet<i32> = mt.iter().copied().collect();
    println!("\n== MTR: ours {} reactions, NJOY {}", mo.len(), mt.len());
    let only_o: Vec<_> = so.difference(&st).collect();
    let only_t: Vec<_> = st.difference(&so).collect();
    if !only_o.is_empty() {
        println!("   only in ours: {only_o:?}");
    }
    if !only_t.is_empty() {
        println!("   only in NJOY's: {only_t:?}");
    }
    verdicts.push((
        "MTR (reaction set and order)".into(),
        if mo == mt {
            "SAME".into()
        } else if so == st {
            "SAME set, DIFFERENT order".into()
        } else {
            format!("DIFFERENT (+{} / -{})", only_o.len(), only_t.len())
        },
    ));
    let mut q_worst = 0.0f64;
    let mut ty_bad = Vec::new();
    for ro in &co.reactions {
        if let Some(rt) = ct.reactions.iter().find(|r| r.mt == ro.mt) {
            let r = rel(ro.q_value, rt.q_value);
            if r > 0.0 {
                println!("   LQR MT={}: ours {:e} NJOY {:e}", ro.mt, ro.q_value, rt.q_value);
            }
            q_worst = q_worst.max(r);
            if ro.ty != rt.ty {
                ty_bad.push(format!("MT={} ours {} NJOY {}", ro.mt, ro.ty, rt.ty));
            }
        }
    }
    verdicts.push(("LQR (Q-values)".into(), verdict(q_worst)));
    for t in &ty_bad {
        println!("   TYR {t}");
    }
    verdicts.push((
        "TYR (yields / frames)".into(),
        if ty_bad.is_empty() {
            "SAME".into()
        } else {
            format!("DIFFERENT ({} MTs)", ty_bad.len())
        },
    ));

    // ── NU: grid-independent, compared word for word ───────────────────────
    verdicts.push(("NU".into(), raw_block_words(ours, theirs, jxs::NU, jxs::MTR)));
    for (label, j) in [
        ("ESZ words", jxs::ESZ),
        ("MTR words", jxs::MTR),
        ("LSIG words", jxs::LSIG),
        ("SIG words", jxs::SIG),
        ("LQR words", jxs::LQR),
        ("TYR words", jxs::TYR),
        ("LAND words", jxs::LAND),
        ("AND words", jxs::AND),
        ("LDLW words", jxs::LDLW),
        ("DLW words", jxs::DLW),
        ("DNU words", jxs::DNU),
        ("BDD words", jxs::BDD),
        ("DNEDL words", jxs::DNEDL),
        ("DNED words", jxs::DNED),
        ("GPD words", 11),
        ("MTRP words", jxs::MTRP),
        ("LSIGP words", jxs::LSIGP),
        ("SIGP words", jxs::SIGP),
        ("LANDP words", jxs::LANDP),
        ("ANDP words", jxs::ANDP),
        ("LDLWP words", jxs::LDLWP),
        ("DLWP words", jxs::DLWP),
        ("YP words", 19),
        ("FIS words", 20),
        ("LUNR words", jxs::LUNR),
    ] {
        verdicts.push((label.into(), raw_span_words(ours, theirs, j)));
    }

    // ── AND / DLW per reaction ─────────────────────────────────────────────
    angular_and_laws(ours, theirs, &co, &ct, &mut verdicts);

    // ── UNR ────────────────────────────────────────────────────────────────
    let uo = UrrProbabilityTables::from_ace(ours, t_k).ok().flatten();
    let ut = UrrProbabilityTables::from_ace(theirs, t_k).ok().flatten();
    verdicts.push((
        "UNR (probability tables)".into(),
        match (&uo, &ut) {
            (None, None) => "absent in both".into(),
            (None, Some(_)) => "MISSING in ours".into(),
            (Some(_), None) => "MISSING in NJOY's".into(),
            (Some(a), Some(b)) => {
                let grid = a.len() == b.len()
                    && a
                        .energies()
                        .iter()
                        .zip(b.energies())
                        .all(|(x, y)| rel(*x, *y) < 1.0e-9);
                let flags = (a.lssf, a.inelastic_competition, a.absorption_competition, a.n_bands())
                    == (b.lssf, b.inelastic_competition, b.absorption_competition, b.n_bands());
                format!(
                    "grid {} ({} pts), flags/bands {}; band values are PURR samples \
                     (statistical, see tests/purr_u238_ptables_vs_njoy.rs)",
                    if grid { "SAME" } else { "DIFFERENT" },
                    b.len(),
                    if flags { "SAME" } else { "DIFFERENT" }
                )
            }
        },
    ));

    // ── Delayed neutrons ───────────────────────────────────────────────────
    let dlo = decode_delayed(ours).ok().flatten();
    let dlt = decode_delayed(theirs).ok().flatten();
    verdicts.push((
        "delayed (DNU, BDD)".into(),
        match (&dlo, &dlt) {
            (None, None) => "absent in both".into(),
            (None, Some(_)) => "MISSING in ours".into(),
            (Some(_), None) => "MISSING in NJOY's".into(),
            (Some(a), Some(b)) => {
                let mut w = 0.0f64;
                for (x, y) in a.lambda.iter().zip(&b.lambda) {
                    w = w.max(rel(*x, *y));
                }
                for (x, y) in a.nu_delayed.iter().zip(&b.nu_delayed) {
                    w = w.max(rel(*x, *y));
                }
                if a.lambda.len() != b.lambda.len() || a.nu_delayed.len() != b.nu_delayed.len() {
                    "DIFFERENT (group or grid count)".into()
                } else {
                    verdict(w)
                }
            }
        },
    ));
    verdicts.push(("DNED (delayed spectra)".into(), raw_presence(&presence, "DNED")));

    // ── Photon production ──────────────────────────────────────────────────
    let po = decode_photon_production(ours).unwrap_or_default();
    let pt = decode_photon_production(theirs).unwrap_or_default();
    let census = |v: &[njoy_outram_park_fork::acer::photon_read::AcePhotonEntry]| {
        let mut c: BTreeMap<String, usize> = BTreeMap::new();
        for e in v {
            let k = match &e.rate {
                AcePhotonRate::Yield { mftype, .. } => format!("MF{mftype}/law{}", e.law.code()),
                AcePhotonRate::Xs { .. } => format!("MF13/law{}", e.law.code()),
            };
            *c.entry(k).or_insert(0) += 1;
        }
        c
    };
    let (cpo, cpt) = (census(&po), census(&pt));
    println!("\n== photon production: ours {} entries {cpo:?}", po.len());
    println!("                        NJOY {} entries {cpt:?}", pt.len());
    let mtrp_same = po.iter().map(|e| e.mtrp).collect::<Vec<_>>()
        == pt.iter().map(|e| e.mtrp).collect::<Vec<_>>();
    verdicts.push((
        "photon production (MTRP..DLWP)".into(),
        if po.is_empty() && pt.is_empty() {
            "absent in both".into()
        } else if po.is_empty() {
            "MISSING in ours".into()
        } else if mtrp_same && cpo == cpt {
            "SAME entries and forms".into()
        } else {
            format!("DIFFERENT (ours {} / NJOY {})", po.len(), pt.len())
        },
    ));
    for b in ["GPD", "YP", "FIS"] {
        verdicts.push((b.into(), raw_presence(&presence, b)));
    }

    // ── Verdicts ───────────────────────────────────────────────────────────
    println!("\n== VERDICT, {name} {temp_dir}");
    for (k, v) in &verdicts {
        println!("   {k:<34} {v}");
    }
}

fn raw_presence(p: &BTreeMap<&str, (bool, bool)>, b: &str) -> String {
    match p.get(b) {
        Some((true, true)) => "present in both (content not compared here)".into(),
        Some((false, true)) => "MISSING in ours".into(),
        Some((true, false)) => "MISSING in NJOY's".into(),
        _ => "absent in both".into(),
    }
}

/// Compare the words from block `j`'s locator up to the next block present in
/// that table (whatever it is), exactly -- both sides went through Type-1
/// text, so equality is equality of the printed value.
fn raw_span_words(ours: &RawAceTable, theirs: &RawAceTable, j: usize) -> String {
    fn span(t: &RawAceTable, j: usize) -> Option<Vec<f64>> {
        let a = t.jxs[j];
        if a <= 0 {
            return None;
        }
        // FIS (JXS 21) points INTO SIG, at MT=18's entry, so it is not a
        // block boundary; treating it as one cut the SIG span off after the
        // first reaction.
        let end = BLOCKS
            .iter()
            .filter(|&&(name, _)| name != "FIS")
            .map(|&(_, k)| t.jxs[k])
            .filter(|&l| l > a)
            .min()
            .unwrap_or(t.xss.len() as i32 + 1);
        Some(t.xss[(a - 1) as usize..(end - 1) as usize].to_vec())
    }
    if let Ok(dir) = std::env::var("OUTRAM_SPAN_DIR") {
        for (who, t) in [("ours", ours), ("njoy", theirs)] {
            if let Some(v) = span(t, j) {
                let text: String = v.iter().map(|x| format!("{x:e}\n")).collect();
                let _ = std::fs::write(format!("{dir}/span_{j}_{who}.txt"), text);
            }
        }
    }
    match (span(ours, j), span(theirs, j)) {
        (None, None) => "absent in both".into(),
        (None, Some(_)) => "MISSING in ours".into(),
        (Some(_), None) => "MISSING in NJOY's".into(),
        (Some(a), Some(b)) if a.len() != b.len() => {
            let first = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            if std::env::var("OUTRAM_DIFF_DUMP").is_ok() {
                let lo = first.saturating_sub(4);
                for i in lo..(first + 24).min(a.len()).min(b.len()) {
                    println!("      word {i}: ours {:e} NJOY {:e}", a[i], b[i]);
                }
            }
            format!(
                "DIFFERENT (ours {} words, NJOY {}; first difference at word {first})",
                a.len(),
                b.len()
            )
        }
        (Some(a), Some(b)) => {
            let n = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            if n == 0 {
                format!("SAME ({} words, every word)", a.len())
            } else {
                let first = a.iter().zip(&b).position(|(x, y)| x != y).unwrap();
                if std::env::var("OUTRAM_DIFF_DUMP").is_ok() {
                    for (i, (x, y)) in a.iter().zip(&b).enumerate().filter(|(_, (x, y))| x != y).take(40) {
                        println!("      word {i}: ours {x:e} NJOY {y:e}");
                    }
                }
                format!(
                    "DIFFERENT ({n} of {} words; first at word {first}: ours {:e} NJOY {:e})",
                    a.len(),
                    a[first],
                    b[first]
                )
            }
        }
    }
}

/// Compare a block word for word, from its locator to the next block's.
fn raw_block_words(ours: &RawAceTable, theirs: &RawAceTable, j: usize, next: usize) -> String {
    fn span(t: &RawAceTable, j: usize, next: usize) -> Option<Vec<f64>> {
        let a = t.jxs[j];
        let b = t.jxs[next];
        (a > 0 && b > a).then(|| t.xss[(a - 1) as usize..(b - 1) as usize].to_vec())
    }
    match (span(ours, j, next).as_deref(), span(theirs, j, next).as_deref()) {
        (None, None) => "absent in both".into(),
        (None, Some(_)) => "MISSING in ours".into(),
        (Some(_), None) => "MISSING in NJOY's".into(),
        (Some(a), Some(b)) if a.len() != b.len() => {
            format!("DIFFERENT (ours {} words, NJOY {})", a.len(), b.len())
        }
        (Some(a), Some(b)) => {
            let w = a.iter().zip(b).map(|(x, y)| rel(*x, *y)).fold(0.0f64, f64::max);
            format!("{} ({} words)", verdict(w), a.len())
        }
    }
}

fn esz_and_sig(co: &CeNeutronAce, ct: &CeNeutronAce, verdicts: &mut Vec<(String, String)>) {
    // Shared grid points, by exact energy match (both are 7-figure rounded).
    let mut shared: Vec<(usize, usize)> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < co.energy.len() && j < ct.energy.len() {
        let (a, b) = (co.energy[i], ct.energy[j]);
        if rel(a, b) < 1.0e-12 {
            shared.push((i, j));
            i += 1;
            j += 1;
        } else if a < b {
            i += 1;
        } else {
            j += 1;
        }
    }
    println!(
        "\n== ESZ: ours {} points, NJOY {}, shared {} ({:.2} % of NJOY's)",
        co.energy.len(),
        ct.energy.len(),
        shared.len(),
        100.0 * shared.len() as f64 / ct.energy.len().max(1) as f64
    );
    verdicts.push((
        "ESZ energy grid".into(),
        if co.energy == ct.energy {
            "SAME".into()
        } else {
            format!(
                "DIFFERENT ({} vs {} points, {:.2} % shared)",
                co.energy.len(),
                ct.energy.len(),
                100.0 * shared.len() as f64 / ct.energy.len().max(1) as f64
            )
        },
    ));
    for (label, a, b) in [
        ("ESZ total", &co.total, &ct.total),
        ("ESZ absorption", &co.absorption, &ct.absorption),
        ("ESZ elastic", &co.elastic, &ct.elastic),
        ("ESZ heating", &co.heating, &ct.heating),
    ] {
        let (w, at) = shared
            .iter()
            .map(|&(i, j)| (rel(a[i], b[j]), (i, j)))
            .fold((0.0f64, (0, 0)), |acc, x| if x.0 > acc.0 { x } else { acc });
        if w > 0.0 {
            println!(
                "   {label} worst at E={:.6e}: ours {:e} NJOY {:e}",
                co.energy[at.0], a[at.0], b[at.1]
            );
        }
        verdicts.push((format!("{label} (at shared points)"), verdict(w)));
    }
    // SIG per reaction, at shared points inside both reactions' ranges.
    let mut worst = 0.0f64;
    let mut worst_mt = 0;
    for ro in &co.reactions {
        let Some(rt) = ct.reactions.iter().find(|r| r.mt == ro.mt) else {
            continue;
        };
        for &(i, j) in &shared {
            if i < ro.threshold_index || j < rt.threshold_index {
                continue;
            }
            let (a, b) = (
                ro.xs.get(i - ro.threshold_index).copied(),
                rt.xs.get(j - rt.threshold_index).copied(),
            );
            if let (Some(a), Some(b)) = (a, b) {
                let d = rel(a, b);
                if d > worst {
                    worst = d;
                    worst_mt = ro.mt;
                    if std::env::var("OUTRAM_DIFF_DUMP").is_ok() {
                        println!(
                            "   SIG MT={} at E={:e}: ours {a:e} (IE {}) NJOY {b:e} (IE {})",
                            ro.mt, co.energy[i], ro.threshold_index, rt.threshold_index
                        );
                    }
                }
            }
        }
    }
    verdicts.push((
        format!("SIG (at shared points; worst MT={worst_mt})"),
        verdict(worst),
    ));
}

fn angular_and_laws(
    ours: &RawAceTable,
    theirs: &RawAceTable,
    co: &CeNeutronAce,
    ct: &CeNeutronAce,
    verdicts: &mut Vec<(String, String)>,
) {
    // Elastic AND.
    let eo = decode_angular(ours, 0, 2).ok().flatten();
    let et = decode_angular(theirs, 0, 2).ok().flatten();
    verdicts.push((
        "AND, elastic".into(),
        match (eo, et) {
            (None, None) => "isotropic in both".into(),
            (None, Some(_)) => "MISSING in ours".into(),
            (Some(_), None) => "MISSING in NJOY's".into(),
            (Some(a), Some(b)) => compare_angular(&a, &b),
        },
    ));
    // Per neutron-producing reaction: AND and DLW law.
    let no = n_neutron_reactions(ours);
    let nt = n_neutron_reactions(theirs);
    let mut and_diff = Vec::new();
    let mut law_diff = Vec::new();
    let mut law_worst = 0.0f64;
    for i in 0..nt {
        let mt = ct.reactions[i].mt;
        let Some(k) = (0..no).find(|&k| co.reactions[k].mt == mt) else {
            law_diff.push(format!("MT={mt}: no DLW entry in ours"));
            continue;
        };
        let lct_o = if co.reactions[k].ty < 0 { 2 } else { 1 };
        let lct_t = if ct.reactions[i].ty < 0 { 2 } else { 1 };
        let ao = decode_angular(ours, k + 1, lct_o).ok().flatten();
        let at = decode_angular(theirs, i + 1, lct_t).ok().flatten();
        match (ao, at) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                let v = compare_angular(&a, &b);
                if v != "SAME" {
                    and_diff.push(format!("MT={mt}: {v}"));
                }
            }
            (a, b) => and_diff.push(format!(
                "MT={mt}: ours {}, NJOY {}",
                if a.is_some() { "tabulated" } else { "isotropic/in-law" },
                if b.is_some() { "tabulated" } else { "isotropic/in-law" }
            )),
        }
        let lo = decode_energy_law(ours, k);
        let lt = decode_energy_law(theirs, i);
        match (lo, lt) {
            (Ok(a), Ok(b)) => {
                if a.code() != b.code() {
                    law_diff.push(format!("MT={mt}: law ours {} NJOY {}", a.code(), b.code()));
                } else if let (
                    njoy_outram_park_fork::acer::ce_laws::AceEnergyLaw::Tabulated { rows: ra, .. },
                    njoy_outram_park_fork::acer::ce_laws::AceEnergyLaw::Tabulated { rows: rb, .. },
                ) = (&a, &b)
                {
                    if ra.len() != rb.len() {
                        law_diff.push(format!(
                            "MT={mt}: law {} incident rows ours {} NJOY {}",
                            a.code(),
                            ra.len(),
                            rb.len()
                        ));
                    } else {
                        for (x, y) in ra.iter().zip(rb) {
                            law_worst = law_worst.max(rel(x.e_in, y.e_in));
                            if x.eout.e_out.len() != y.eout.e_out.len() {
                                law_worst = law_worst.max(1.0);
                                continue;
                            }
                            for (p, q) in x.eout.e_out.iter().zip(&y.eout.e_out) {
                                law_worst = law_worst.max(rel(*p, *q));
                            }
                            for (p, q) in x.eout.cdf.iter().zip(&y.eout.cdf) {
                                law_worst = law_worst.max((p - q).abs());
                            }
                        }
                    }
                }
            }
            (Err(e), _) => law_diff.push(format!("MT={mt}: ours does not decode: {e}")),
            (_, Err(e)) => law_diff.push(format!("MT={mt}: NJOY's does not decode: {e}")),
        }
    }
    for d in and_diff.iter().take(12) {
        println!("   AND {d}");
    }
    for d in law_diff.iter().take(12) {
        println!("   DLW {d}");
    }
    verdicts.push((
        format!("AND, {nt} producing reactions"),
        if and_diff.is_empty() {
            "SAME".into()
        } else {
            format!("DIFFERENT ({} MTs)", and_diff.len())
        },
    ));
    verdicts.push((
        format!("DLW, {nt} producing reactions"),
        if law_diff.is_empty() {
            format!("same laws; tabulated values {}", verdict(law_worst))
        } else {
            format!("DIFFERENT ({} MTs); tabulated {}", law_diff.len(), verdict(law_worst))
        },
    ));
}

fn compare_angular(
    a: &njoy_outram_park_fork::acer::angular::ElasticAngular,
    b: &njoy_outram_park_fork::acer::angular::ElasticAngular,
) -> String {
    if a.energies.len() != b.energies.len() {
        return format!(
            "DIFFERENT ({} vs {} incident energies)",
            a.energies.len(),
            b.energies.len()
        );
    }
    let mut w = 0.0f64;
    for (x, y) in a.energies.iter().zip(&b.energies) {
        w = w.max(rel(x.e_mev, y.e_mev));
        if x.cosines.len() != y.cosines.len() {
            return format!(
                "DIFFERENT (cosine count {} vs {} at {:.4e} MeV)",
                x.cosines.len(),
                y.cosines.len(),
                y.e_mev
            );
        }
        for (p, q) in x.cosines.iter().zip(&y.cosines) {
            w = w.max((p - q).abs());
        }
        for (p, q) in x.cdf.iter().zip(&y.cdf) {
            w = w.max((p - q).abs());
        }
    }
    verdict(w)
}
