//! ACER for an incident charged particle (gh:#1485): function-level gates
//! against NJOY2016.80's reference tapes (`tests/14`). Skips when the
//! upstream clone is absent (`NJOY2016_ROOT`, else `upstream_source/NJOY2016`).

use njoy_outram_park_fork::acer::{charged, read::parse_type1};
use njoy_outram_park_fork::endf::tape::Tape;
use std::path::{Path, PathBuf};

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

/// Which incident-charged-particle ACER branches the held evaluations reach
/// (gh:#1490). A map of reachability, pinned so that a newly held tape that
/// reaches a branch no deck exercises fails here, and is then added to a
/// deck-level comparison against NJOY2016.
///
/// **Methodology.** Every MF=6 subsection of the eight charged-particle
/// evaluations in NJOY2016's `tests/resources` (`epn14`, `p-001_H_001/002/003`,
/// `d-001_H_002`, `d-002_He_003`, `a-002_He_004`, and `d-078_Pt_184-TENDL2021`,
/// deck 71's tape, ~~left out before 2026-11-13~~; `reference-data/endf/` holds
/// the same alpha + He-4 tape and no other) is walked, and recorded: each
/// LAW = 5 LIST's `LTP` (`L1`), each LAW = 5 subsection's ZAP against the
/// projectile (`NSUB/10`), and each LAW = 4 subsection with ZAP > 2004 in an
/// MT above 2 (the recoil-heating branch, `acefc.f90:11036-11091`; MT = 2 is
/// outside that loop, `:10941`). The walk knows LAW 1, 2, 4, 5 and 6, the only
/// ones these tapes use; any other panics.
///
/// **Result (2026-11-12, Pt-184 added 2026-11-13):** no `LTP = 2` record (so
/// `coul`'s LTP = 2 branch is gated by the synthetic records of
/// `acer_charged_functions_code_to_code`; but d + He-3's first record, 100 eV,
/// has `LTP = 0`, which ENDF-6 does not define and which `coul`'s
/// `if (ltp.eq.1) ... else` sends down the LTP = 2 formula, at values near
/// 1e-238: deck 62's ACE is identical with it); `LTP = 12` in `epn14`,
/// p + H-2, alpha + He-4 and d + Pt-184 (decks 14, 51, 50, 71: ACE identical, and `acecpe`
/// calls `ptlegc` only for `ltp < 12`, `:6552`); no LAW = 5 product other than
/// the projectile; no LAW = 4 recoil heavier than an alpha above MT = 2 (Pt-184
/// has one, ZAP 78184, in MT = 2). ~~Both branches are refused.~~ **Since
/// 2026-11-13 both are translated** and gated only by the `synthetic_*`
/// differentials below, so the zero counts are pinned: a held tape that
/// reaches either must get a deck of its own. [`acelcp_recoil::recoil_heating_supported`]
/// (the tape-only refusals that remain) holds on every tape.
/// The full ENDF/B-VIII.0 charged-particle sublibraries are not held (gh:#1511).
#[test]
fn held_charged_evaluations_reach_only_ported_branches() {
    use njoy_outram_park_fork::acer::acelcp_recoil;
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
        "d-078_Pt_184-TENDL2021.endf",
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
                        if izap > 2004 && sec.key.mt > 2 {
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
        assert_eq!(law5_other, 0, "{f}: reaches the LAW = 5 non-projectile branch: give it a deck against NJOY2016");
        assert_eq!(law4_heavy, 0, "{f}: reaches the LAW = 4 recoil heating: give it a deck against NJOY2016");
        assert!(acelcp_recoil::recoil_heating_supported(&tape, mat), "{f}");
    }
    if tapes == files.len() {
        assert_eq!(ltp12_tapes, ["epn14", "p-001_H_002-ENDF8.0.endf", "a-002_He_004-ENDF8.0.endf", "d-078_Pt_184-TENDL2021.endf"]);
    }
}

// ---- gh:#1490: synthetic differentials for the two branches no held tape reaches ----

/// The integer in field `i` (0-based) of an ENDF line.
fn endf_int(line: &str, i: usize) -> i64 {
    line.get(11 * i..11 * (i + 1)).unwrap_or("").trim().parse().unwrap_or(0)
}

/// `line` with field `i` replaced by `text` (11 characters).
fn endf_set(line: &str, i: usize, text: &str) -> String {
    assert_eq!(text.len(), 11);
    format!("{}{}{}", &line[..11 * i], text, &line[11 * (i + 1)..])
}

/// The edits that make the synthetic tapes, each a minimal change to MF=6/MT=50
/// of a held evaluation (gh:#1490). They are made in a temp dir at test time
/// and never committed. They test that two codes agree on the same input; they
/// say nothing about physics.
#[derive(Clone, Copy, Debug)]
enum Edit {
    /// Subsection 2 becomes LAW = 5, with subsection 1's TAB2 and LISTs as
    /// its body: a LAW = 5 product other than the projectile, last subsection.
    Sub2Law5,
    /// Subsection 1's `L2` set from 2 to 5, nothing else: the same, before the
    /// last subsection (upstream's energy pass reads its body as TAB1s).
    Sub1Law5,
    /// Subsection 2's ZAP set to 2005: a heavy LAW = 4 recoil.
    Sub2Zap2005,
    /// [`Edit::Sub2Zap2005`] plus a copy of subsection 1 appended as
    /// subsection 3 (`NK = 3`): the recoil is no longer the last subsection.
    Sub2Zap2005Copy1,
}

/// Apply `edit` to MF=6/`mt` of the ENDF text `text`.
fn synthetic_tape(text: &str, mt: i64, edit: Edit) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let is_sec = |l: &str| l.len() >= 75 && l[70..72].trim() == "6" && l[72..75].trim().parse::<i64>().ok() == Some(mt);
    let a = lines.iter().position(|l| is_sec(l)).expect("section");
    let b = a + lines[a..].iter().take_while(|l| is_sec(l)).count();
    let mut rows: Vec<String> = lines[a..b].iter().map(|s| s.to_string()).collect();
    // Per subsection: (TAB1 head row, end of TAB1, end of subsection).
    let nk = endf_int(&rows[0], 4) as usize;
    let mut subs = Vec::new();
    let mut pos = 1usize;
    for _ in 0..nk {
        let s = pos;
        let (law, nr, np) = (endf_int(&rows[pos], 3), endf_int(&rows[pos], 4) as usize, endf_int(&rows[pos], 5) as usize);
        pos += 1 + (2 * nr).div_ceil(6) + (2 * np).div_ceil(6);
        let te = pos;
        match law {
            1 | 2 | 5 => {
                let (nr2, ne) = (endf_int(&rows[pos], 4) as usize, endf_int(&rows[pos], 5) as usize);
                pos += 1 + (2 * nr2).div_ceil(6);
                for _ in 0..ne {
                    let nw = endf_int(&rows[pos], 4) as usize;
                    pos += 1 + nw.div_ceil(6);
                }
            }
            6 => pos += 1,
            _ => {}
        }
        subs.push((s, te, pos));
    }
    match edit {
        Edit::Sub2Law5 => {
            let (s, te, _) = subs[1];
            rows[s] = endf_set(&rows[s], 3, "          5");
            let body: Vec<String> = rows[subs[0].1..subs[0].2].to_vec();
            rows.splice(te..te, body);
        }
        Edit::Sub1Law5 => {
            let s = subs[0].0;
            rows[s] = endf_set(&rows[s], 3, "          5");
        }
        Edit::Sub2Zap2005 | Edit::Sub2Zap2005Copy1 => {
            let s = subs[1].0;
            rows[s] = endf_set(&rows[s], 0, " 2.005000+3");
            if matches!(edit, Edit::Sub2Zap2005Copy1) {
                rows[0] = endf_set(&rows[0], 4, &format!("{:11}", nk + 1));
                let copy: Vec<String> = rows[subs[0].0..subs[0].2].to_vec();
                let end = subs[nk - 1].2;
                rows.splice(end..end, copy);
            }
        }
    }
    let mut out: Vec<String> = lines[..a].iter().map(|s| s.to_string()).collect();
    out.extend(rows);
    out.extend(lines[b..].iter().map(|s| s.to_string()));
    out.join("\n") + "\n"
}

/// One synthetic deck run on both codes.
struct SyntheticRun {
    ours: PathBuf,
    njoy: PathBuf,
    njoy_status: i32,
    njoy_log: String,
    ours_result: Result<(), String>,
    tape: Tape,
}

/// Build the synthetic tape from `resource`, then run upstream deck `case`
/// (cut before `viewr`) on it in the NJOY2016 binary and in `run_deck`.
/// `None` (skip) when the upstream clone or its binary is absent.
fn synthetic_run(tag: &str, case: &str, resource: &str, edit: Edit) -> Option<SyntheticRun> {
    use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
    let r = root();
    let (bin, res, input) = (r.join("build/njoy"), r.join("tests/resources").join(resource), r.join(format!("tests/{case}/input")));
    if !bin.exists() || !res.exists() || !input.exists() {
        eprintln!("skip {tag}: NJOY2016 clone or binary not present (set NJOY2016_ROOT)");
        return None;
    }
    let text = synthetic_tape(&std::fs::read_to_string(&res).unwrap(), 50, edit);
    let mut deck = String::new();
    for line in std::fs::read_to_string(&input).unwrap().lines() {
        if line.trim() == "viewr" || line.trim() == "stop" {
            break;
        }
        deck.push_str(line);
        deck.push('\n');
    }
    deck.push_str("stop\n");
    let tmp = std::env::temp_dir().join(format!("acer1490_{tag}_{}", std::process::id()));
    let (ours, njoy) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &njoy] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), &text).unwrap();
    }
    std::fs::write(njoy.join("deck"), &deck).unwrap();
    let st = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&njoy)
        .status()
        .unwrap();
    let njoy_log = std::fs::read_to_string(njoy.join("log")).unwrap_or_default();
    let ours_result = match run_deck(&deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false }) {
        Ok(run) => match run.blocked {
            None => Ok(()),
            Some((m, why)) => Err(format!("{m}: {why}")),
        },
        Err(e) => Err(e.to_string()),
    };
    let tape = Tape::read_file(&ours.join("tape20")).unwrap();
    Some(SyntheticRun { ours, njoy, njoy_status: st.code().unwrap_or(-1), njoy_log, ours_result, tape })
}

impl SyntheticRun {
    fn mat(&self) -> i32 {
        self.tape.sections().iter().find(|s| s.key.mf == 1 && s.key.mt == 451).unwrap().key.mat
    }

    fn clean(&self) {
        if std::env::var("KEEP_DECK_DIRS").is_err() {
            let _ = std::fs::remove_dir_all(self.ours.parent().unwrap());
        } else {
            println!("kept {}", self.ours.parent().unwrap().display());
        }
    }
}

/// A run both codes finish: every line of units 31-35 (the ACE, its directory
/// line, and the `iopt = 7` check run's ACE, listing and directory) identical,
/// and unit 31 identical in every XSS word.
fn assert_identical_units(run: &SyntheticRun, tag: &str) {
    use njoy_outram_park_fork::acer::acelcp_recoil::recoil_heating_supported;
    assert_eq!(run.njoy_status, 0, "{tag}: NJOY2016 failed: {}", run.njoy_log);
    assert!(recoil_heating_supported(&run.tape, run.mat()), "{tag}: tape-level refusal");
    assert_eq!(run.ours_result, Ok(()), "{tag}: the deck must run to its end here");
    for unit in 31..=35 {
        let name = format!("tape{unit}");
        let (Ok(a), Ok(b)) = (std::fs::read_to_string(run.ours.join(&name)), std::fs::read_to_string(run.njoy.join(&name))) else {
            panic!("{tag}: {name} missing on one side")
        };
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let differing: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
        println!("{tag} unit {unit}: {} lines, {} differing; first {:?}", bl.len(), differing.len(), differing.first().map(|&i| (i + 1, al.get(i), bl.get(i))));
        assert!(differing.is_empty(), "{tag}: unit {unit} differs from NJOY2016 in {} lines", differing.len());
    }
    let read = |d: &Path| parse_type1(&std::fs::read_to_string(d.join("tape31")).unwrap()).unwrap();
    let (ours, theirs) = (read(&run.ours), read(&run.njoy));
    let nes = theirs.nxs[2] as usize;
    println!("{tag}: {} XSS words; ESZ heating at the top energy {:e}", theirs.xss.len(), theirs.xss[5 * nes - 1]);
    assert!(njoy_outram_park_fork::acer::compare::compare(&ours, &theirs).identical(), "{tag}: unit 31 differs from NJOY2016");
}

/// **LAW = 4 heavy recoil, case (c): d + H-2 (deck 53), MT = 50's He-3 recoil
/// given ZAP 2005** (gh:#1490). `acelcp`'s LAW = 4 recoil heating
/// (`acefc.f90:11036-11091`, `acelcp_recoil::recoil_law4_heating`) against
/// NJOY2016.80's own binary on the same tape and deck.
///
/// Prediction (posted on #1490 before the run): stale `ne = 67` (the angular
/// pass re-reading MT = 600 subsection 1, `:9929`), `e = esz(nes)` in MeV
/// (`:10042`), `amass = awr/awi` (`:10579`); every word identical.
///
/// **Result (2026-11-13):** NJOY2016 runs; units 31-35 are identical line for
/// line, and unit 31 in every XSS word (9 590 words).
#[test]
fn synthetic_law4_recoil_heating_d_h2() {
    let Some(run) = synthetic_run("c", "53", "d-001_H_002-ENDF8.0.endf", Edit::Sub2Zap2005) else { return };
    assert_identical_units(&run, "c");
    run.clean();
}

/// **LAW = 4 heavy recoil, case (d): p + H-3 (deck 54), the same edit.** The
/// last production type is the triton elastic recoil (MT = 2), so the stale
/// `ne` is the projectile elastic AND's `NE` (`:9558`), fewer than the 67
/// energies of MT = 50's first subsection: the heating table keeps only the
/// first `ne` of them (upstream's `NP = NBT = ne`).
///
/// **Result (2026-11-13):** units 31-35 identical line for line, unit 31 in
/// every XSS word.
#[test]
fn synthetic_law4_recoil_heating_p_h3() {
    let Some(run) = synthetic_run("d", "54", "p-001_H_003-ENDF8.0.endf", Edit::Sub2Zap2005) else { return };
    assert_identical_units(&run, "d");
    run.clean();
}

/// **LAW = 4 heavy recoil, case (e): (c) with subsection 1 copied to
/// subsection 3.** Upstream's `findf` rewinds to subsection 1 but `do ik=1,nk`
/// keeps counting, so iteration 3 reads subsection 2, the recoil, again and
/// runs the branch a second time, now with `e = esz(nes)*emev` **in eV** from
/// the first pass (`:11084`) against `q` in MeV.
///
/// **Result (2026-11-13):** units 31-35 identical line for line, unit 31 in
/// every XSS word.
#[test]
fn synthetic_law4_recoil_heating_runs_twice() {
    let Some(run) = synthetic_run("e", "53", "d-001_H_002-ENDF8.0.endf", Edit::Sub2Zap2005Copy1) else { return };
    assert_identical_units(&run, "e");
    run.clean();
}

/// Both codes stop in `change` on the empty DLWH entry, and every XSS word
/// NJOY2016 wrote before it stopped is the port's word.
///
/// NJOY2016's unit 31 holds the lines `change` completed (`typen` writes four
/// words a line). Each is compared as printed: an integer field against the
/// port's word rounded, a real field against the port's word through the same
/// `1pe20.11` edit. Past the table's end both read zeros (`xss=0`, `:113`).
fn assert_same_stop_in_change(run: &SyntheticRun, tag: &str) {
    use njoy_outram_park_fork::acer::fortran_fmt::fortran_e20;
    assert_eq!(run.njoy_status, 77, "{tag}: NJOY2016 was expected to stop in change: {}", run.njoy_log);
    assert!(run.njoy_log.contains("***error in change***Undefined law for dlwh block:   0"), "{tag}: {}", run.njoy_log);
    let err = run.ours_result.clone().expect_err("the port must stop too");
    assert!(err.contains("Undefined law for dlwh block: 0"), "{tag}: the port stopped elsewhere: {err}");
    // The table the port built, as the deck builds it (binary units 21, 22).
    let table = charged::build_charged(&run.tape, run.mat(), 0.0, 10, false, false).unwrap();
    let x = &table.xss;
    let w = |i: usize| x[i - 1].round() as usize;
    let (ntype, ntro, ploct) = (table.nxs[6] as usize, table.jxs[30] as usize, table.jxs[31] as usize);
    let mut entry = None;
    for t in 0..ntype {
        let (ldlwh, dlwh, ntrh) = (w(ploct + 10 * t + 7), w(ploct + 10 * t + 8), w(ntro + t));
        for jp in 0..ntrh {
            let nn = w(ldlwh + jp);
            if nn > 0 && entry.is_none() && w(dlwh + nn) == 0 {
                entry = Some(dlwh + nn - 1);
            }
        }
    }
    let entry = entry.expect("an empty DLWH entry");
    assert_eq!((x[entry - 1], x[entry], x[entry + 1]), (0.0, 0.0, 0.0), "{tag}: LNW, LAW, IDAT");
    let text = std::fs::read_to_string(run.njoy.join("tape31")).unwrap();
    let mut words = 0usize;
    let mut bad = Vec::new();
    for line in text.lines().skip(12) {
        for f in 0..line.len() / 20 {
            let tok = &line[20 * f..20 * (f + 1)];
            let v = x.get(words).copied().unwrap_or(0.0);
            let same = if tok.contains('.') { fortran_e20(v) == tok } else { tok.trim().parse::<i64>().ok() == Some(v.round() as i64) };
            if !same {
                bad.push((words + 1, tok.to_string(), v));
            }
            words += 1;
        }
    }
    println!(
        "{tag}: NJOY2016 wrote {words} XSS words before stopping (table {} words, empty DLWH entry at word {entry}); {} differ; first {:?}",
        x.len(),
        bad.len(),
        bad.first()
    );
    assert!(words >= entry + 2, "{tag}: NJOY2016's file stops before the entry");
    assert!(bad.is_empty(), "{tag}: {} words differ from NJOY2016's", bad.len());
}

/// **LAW = 5 product other than the projectile, case (a): d + H-2 (deck 53),
/// MT = 50 subsection 2 (He-3) made LAW = 5 with subsection 1's body**
/// (gh:#1490).
///
/// Prediction (posted on #1490 before the run): NJOY2016 runs to completion,
/// and the He-3 production block has LANDH = -1 and a DLWH entry `0 0 0`.
/// **Half wrong:** the entry is as predicted, but NJOY2016 then stops in
/// `change` ("Undefined law for dlwh block: 0", `acefc.f90:13945`, STOP 77):
/// the ACE writer knows no law 0. So upstream's behaviour for such a product
/// is an error stop. The port stops with the same error from
/// `acefix_change::xss_lines`.
///
/// **Result (2026-11-13):** both stop there; every XSS word NJOY2016 wrote
/// before stopping (printed count) equals the port's.
#[test]
fn synthetic_law5_non_projectile_last_subsection() {
    let Some(run) = synthetic_run("a", "53", "d-001_H_002-ENDF8.0.endf", Edit::Sub2Law5) else { return };
    assert_same_stop_in_change(&run, "a");
    run.clean();
}

/// **LAW = 5 product other than the projectile, case (b): MT = 50 subsection
/// 1 (the neutron) made LAW = 5, before the last subsection.** Upstream's
/// energy pass does not skip the body, so its next `tab1io` reads the body's
/// TAB2 record (binary scratch tape: one record, ZAP 0, LAW 0, a no-op); then
/// it stops in `change` as in (a).
///
/// **Result (2026-11-13):** both stop there; every XSS word NJOY2016 wrote
/// before stopping equals the port's.
#[test]
fn synthetic_law5_non_projectile_before_last_subsection() {
    let Some(run) = synthetic_run("b", "53", "d-001_H_002-ENDF8.0.endf", Edit::Sub1Law5) else { return };
    assert_same_stop_in_change(&run, "b");
    run.clean();
}
