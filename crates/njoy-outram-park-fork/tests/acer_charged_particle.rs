//! ACER for an incident charged particle (gh:#1485): function-level gates
//! against NJOY2016.80's reference tapes (`tests/14`). Skips when the
//! upstream clone is absent (`NJOY2016_ROOT`, else `upstream_source/NJOY2016`).

use njoy_outram_park_fork::acer::{charged, read::parse_type1};
use njoy_outram_park_fork::endf::tape::Tape;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::var("NJOY2016_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// `unionx`'s charged-particle union grid (deck 14, p + N-14) against the ESZ
/// energy column of NJOY2016's reference ACE (`referenceTape31`), MeV at 9
/// figures. Pass criterion: same length, every energy equal after `sigfig(E/1e6, 9)`.
#[test]
fn deck14_union_grid_matches_reference_esz() {
    let r = root();
    let (Ok(tape), Ok(ace)) = (Tape::read_file(&r.join("tests/resources/epn14")), std::fs::read_to_string(r.join("tests/14/referenceTape31"))) else {
        eprintln!("skip: upstream not present");
        return;
    };
    assert_eq!(charged::incident_particle(&tape, 725), 1001);
    let grid = charged::union_grid(&tape, 725).unwrap();
    let t = parse_type1(&ace).unwrap();
    let nes = t.nxs[2] as usize;
    let ours: Vec<f64> = grid.iter().map(|&e| njoy_outram_park_fork::mixr::mix::sigfig(e / 1.0e6, 9, 0)).collect();
    let refe = &t.xss[..nes];
    let bad = ours.iter().zip(refe).filter(|(a, b)| ((*a - *b) / *b).abs() > 1e-11).count();
    println!("grid {} vs ESZ {}; differing {bad}", ours.len(), nes);
    assert_eq!(ours.len(), nes);
    assert_eq!(bad, 0);
}

/// Whole table (deck 14) against the reference, block by block (prints the
/// comparison report; work in progress).
#[test]
fn deck14_table_blocks() {
    let r = root();
    let (Ok(tape), Ok(ace)) = (Tape::read_file(&r.join("tests/resources/epn14")), std::fs::read_to_string(r.join("tests/14/referenceTape31"))) else {
        return;
    };
    let t = charged::build_charged(&tape, 725, 0.0, 0, true, true).unwrap();
    let ours = parse_type1(&t.to_type1_text().unwrap()).unwrap();
    let theirs = parse_type1(&ace).unwrap();
    let c = njoy_outram_park_fork::acer::compare::compare(&ours, &theirs);
    for l in c.report_lines() {
        println!("{l}");
    }
    // Blocks built so far: ESZ (energy, total, disappearance, elastic), MTR..DLW.
    let dlw_end = theirs.jxs[11] as usize - 1; // GPD starts after DLW
    let nes = theirs.nxs[2] as usize;
    let mut worst = Vec::new();
    for (name, lo, hi) in [("ESZ cols 1-4", 0, 4 * nes), ("MTR..DLW", 5 * nes, dlw_end), ("GPD..YP", dlw_end, theirs.jxs[21] as usize), ("ESZ heating", 4 * nes, 5 * nes), ("after END (acelcp)", theirs.jxs[21] as usize, theirs.xss.len())] {
        let diffs: Vec<usize> = (lo..hi).filter(|&i| ours.xss[i] != theirs.xss[i]).collect();
        println!("{name}: {} of {} words differ; first {:?}", diffs.len(), hi - lo, diffs.first().map(|&i| (i + 1, ours.xss[i], theirs.xss[i])));
        worst.push(diffs.len());
    }
    let d0 = theirs.jxs[10] as usize - 1;
    println!("DLW ours   {:?}", &ours.xss[d0 + 215..d0 + 245]);
    println!("DLW theirs {:?}", &theirs.xss[d0 + 215..d0 + 245]);
    assert_eq!(worst, vec![0, 0, 0, 0, 0], "ESZ columns 1-4 and MTR..DLW must match NJOY2016 word for word");
}

/// `unionx`'s union grid (`acefc.f90:1550-1727`, with its stale-record and
/// repeated-energy quirks) and `acelod`'s read-back of it, for the seven
/// incident-charged-particle test decks, against the ESZ energy column of
/// NJOY2016's own ACE at the 12 figures the file prints
/// (`verification_and_validation/acer_charged/esz_energies.txt`, taken from
/// `tape31` of each deck run on 2026-11-11: decks 50 (alpha + He-4), 51 (p +
/// H-2), 52 (p + H-1), 53 (d + H-2), 54 (p + H-3), 62 (d + He-3), 71 (d +
/// Pt-184)). The table is built as the decks do, with binary ENDF and PENDF
/// units (`build_charged(.., false, false)`). Pass criterion: same length,
/// every energy equal within 1e-11 relative. Deck 14 has its own test above.
#[test]
fn union_grid_matches_esz_of_every_charged_deck() {
    let r = root();
    let text = include_str!("../verification_and_validation/acer_charged/esz_energies.txt");
    let parse = |t: &str| -> f64 {
        t.parse().unwrap_or_else(|_| {
            // Fortran prints a three-digit exponent without its `E`.
            let i = t.rfind(['+', '-']).unwrap();
            format!("{}e{}", &t[..i], &t[i..]).parse().unwrap()
        })
    };
    let mut lines = text.lines();
    let (mut decks, mut energies) = (0, 0);
    while let Some(h) = lines.next() {
        let f: Vec<&str> = h.split_whitespace().collect();
        let refe: Vec<f64> = lines.next().unwrap().split_whitespace().map(parse).collect();
        let Ok(tape) = Tape::read_file(&r.join("tests/resources").join(f[1])) else {
            eprintln!("skip {}: upstream not present", f[0]);
            continue;
        };
        let mat: i32 = f[2].parse().unwrap();
        let table = charged::build_charged(&tape, mat, 0.0, 0, false, false).unwrap();
        let ours: Vec<f64> = table.xss[..table.nxs[2] as usize].to_vec();
        let nes: usize = f[3].parse().unwrap();
        assert_eq!(refe.len(), nes);
        assert_eq!(ours.len(), nes, "{}: union grid length", f[0]);
        let bad = ours.iter().zip(&refe).filter(|(a, b)| ((*a - *b) / *b).abs() > 1e-11).count();
        println!("{}: {} energies, {} differing", f[0], nes, bad);
        assert_eq!(bad, 0, "{}", f[0]);
        decks += 1;
        energies += nes;
    }
    println!("{decks} decks, {energies} energies identical");
}

/// Which incident-charged-particle ACER branches the held evaluations can
/// reach (gh:#1490). A map, pinned so that a newly held tape that reaches a
/// refused branch fails here instead of being refused silently.
///
/// **Methodology.** Every MF=6 subsection of the seven charged-particle
/// evaluations in NJOY2016's `tests/resources` (`epn14`, `p-001_H_001/002/003`,
/// `d-001_H_002`, `d-002_He_003`, `a-002_He_004`; `reference-data/endf/` holds
/// the same alpha + He-4 tape and no other) is walked, and recorded: each
/// LAW = 5 LIST's `LTP` (`L1`), each LAW = 5 subsection's ZAP against the
/// projectile (`NSUB/10`), and each LAW = 4 subsection with ZAP > 2004 (the
/// recoil-heating branch, `acefc.f90:11036-11080`). The walk knows LAW 1, 2,
/// 4, 5 and 6, the only ones these tapes use; any other panics.
///
/// **Result (2026-11-12):** no `LTP = 2` record (so `coul`'s LTP = 2 branch
/// is gated by the synthetic records of `acer_charged_functions_code_to_code`;
/// but d + He-3's first record, 100 eV, has `LTP = 0`, which ENDF-6 does not
/// define and which `coul`'s `if (ltp.eq.1) ... else` sends down the LTP = 2
/// formula, at values near 1e-238: deck 62's ACE is identical with it);
/// `LTP = 12` in `epn14`, p + H-2 and alpha + He-4 (decks 14, 51, 50: ACE
/// identical, and `acecpe` calls `ptlegc` only for `ltp < 12`, `:6552`); no
/// LAW = 5 product other than the projectile (upstream `acelcp` has no LAW = 5
/// branch in its product loop, `:10318-10743`); no LAW = 4 recoil heavier
/// than an alpha, so [`acelcp::recoil_heating_supported`] holds on every tape.
/// The full ENDF/B-VIII.0 charged-particle sublibraries are not held (and
/// their hosts are refused by this environment's proxy), so they are **not
/// checked**.
#[test]
fn held_charged_evaluations_reach_only_ported_branches() {
    use njoy_outram_park_fork::acer::acelcp;
    use njoy_outram_park_fork::endf::records::SectionCursor;
    let r = root();
    let files = [
        "epn14",
        "p-001_H_001-ENDF8.0.endf",
        "p-001_H_002-ENDF8.0.endf",
        "p-001_H_003-ENDF8.0.endf",
        "d-001_H_002-ENDF8.0.endf",
        "d-002_He_003.endf",
        "a-002_He_004-ENDF8.0.endf",
    ];
    let (mut tapes, mut ltp12_tapes) = (0, Vec::new());
    for f in files {
        let Ok(tape) = Tape::read_file(&r.join("tests/resources").join(f)) else {
            eprintln!("skip {f}: upstream not present");
            continue;
        };
        tapes += 1;
        let mf1 = tape.sections().iter().find(|s| s.key.mf == 1 && s.key.mt == 451).unwrap();
        let mat = mf1.key.mat;
        let izai = (mf1.rows[2][4].round() as i32) / 10;
        assert!(izai > 1, "{f}: not an incident charged particle");
        let (mut ltps, mut law5_other, mut law4_heavy) = (Vec::new(), 0, 0);
        for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6) {
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont().unwrap();
            for _ in 0..head.n1 {
                let yt = cur.read_tab1().unwrap();
                let (izap, law) = (yt.head.c1.round() as i32, yt.head.l2);
                match law {
                    1 | 2 | 5 => {
                        let t2 = cur.read_tab2().unwrap();
                        for _ in 0..t2.head.n2 {
                            let l = cur.read_list().unwrap();
                            if law == 5 && !ltps.contains(&l.head.l1) {
                                ltps.push(l.head.l1);
                            }
                        }
                        if law == 5 && izap != izai {
                            law5_other += 1;
                        }
                    }
                    4 => {
                        if izap > 2004 {
                            law4_heavy += 1;
                        }
                    }
                    6 => {
                        cur.read_cont().unwrap();
                    }
                    _ => panic!("{f}: MF=6/MT={} LAW = {law} is not walked here", sec.key.mt),
                }
            }
        }
        println!("{f}: izai {izai}, LAW=5 LTP {ltps:?}, LAW=5 non-projectile {law5_other}, LAW=4 ZAP>2004 {law4_heavy}");
        assert!(!ltps.contains(&2), "{f}: an LTP = 2 record is held now; gate coul on it directly");
        if ltps.iter().any(|&l| l >= 12) {
            ltp12_tapes.push(f);
        }
        assert_eq!(law5_other, 0, "{f}: a LAW = 5 product other than the projectile");
        assert_eq!(law4_heavy, 0, "{f}: a LAW = 4 recoil heavier than an alpha (recoil heating is refused)");
        assert!(acelcp::recoil_heating_supported(&tape, mat), "{f}");
    }
    if tapes == files.len() {
        assert_eq!(ltp12_tapes, ["epn14", "p-001_H_002-ENDF8.0.endf", "a-002_He_004-ENDF8.0.endf"]);
    }
}
