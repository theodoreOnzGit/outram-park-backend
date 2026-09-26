//! Compare this crate's PENDF, stage by stage, against NJOY2016's own —
//! word for word, section by section (GitHub #340).
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --example pendf_stage_vs_njoy -- \
//!     <endf-tape> <mat> <njoy-pendf> [temperature-K|0] [err]
//! ```
//!
//! `temperature-K` of `0` (or absent) compares the RECONR stage; `err` is
//! the RECONR/BROADR tolerance, default `0.001`.
//!
//! **Methodology.** Ours: `reconr` at `err = 0.001`, then, when a temperature
//! is given, `broadr::broaden_result` (upstream's card defaults: `errthn =
//! 0.001`, `thnmax = 6.5e6`). The result goes through PENDF text
//! (`ReconrResult::through_pendf_text`), so it is compared as printed.
//! Theirs: the MF=3 sections of an NJOY2016 PENDF made with the same deck.
//! For RECONR that is `reconr 20 21 / 'x'/ <mat> 0/ .001/ 0/`; for BROADR,
//! add `broadr 20 21 22 / <mat> 1 0 0 0./ .001/ <T>/ 0/`.
//!
//! For each MT the report gives the point counts, the number of energies
//! both tables carry, and the number of `(E, sigma)` pairs identical in both
//! words. It also gives the first pair that differs. The pass criterion is
//! "SAME": every pair identical. No tolerance is applied.
//!
//! This separates a RECONR defect from a BROADR one, which the ACE-level
//! comparison (`ace_blocks_vs_reference`) cannot do: there, both reach ACER
//! as one grid.

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: pendf_stage_vs_njoy <endf> <mat> <njoy-pendf> [temperature-K|0] [err]");
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read ENDF tape");
    let mat: i32 = args[1].parse().expect("mat");
    let theirs = Tape::read_file(std::path::Path::new(&args[2])).expect("read NJOY PENDF");
    let temp: Option<f64> = args
        .get(3)
        .map(|t| t.parse::<f64>().expect("temperature"))
        .filter(|&t| t > 0.0);
    let err: f64 = args.get(4).map_or(1.0e-3, |e| e.parse().expect("err"));

    let t0 = std::time::Instant::now();
    let r0 = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: err,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let ours = match temp {
        Some(t) if (err - 1.0e-3).abs() < 1e-12 => broaden_result(&r0, t),
        Some(t) => njoy_outram_park_fork::broadr::joint::broadr_joint(
            &r0.through_pendf_text(),
            t,
            &njoy_outram_park_fork::broadr::BroadnTolerances::with_errthn(err),
            6.5e6,
        )
        .expect("MT=1 present"),
        None => r0,
    }
    .through_pendf_text();
    println!("built ours in {:.1} s", t0.elapsed().as_secs_f64());

    let mut all_same = true;
    let mut their_mts = Vec::new();
    for sec in theirs.sections() {
        if sec.key.mat != mat || sec.key.mf != 3 {
            continue;
        }
        let mt = sec.key.mt;
        their_mts.push(mt);
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont().expect("HEAD");
        let t = cur.read_tab1().expect("TAB1");
        let Some(mine) = ours.sections.iter().find(|s| s.mt.number() == mt) else {
            println!("MT={mt:4}  MISSING in ours (NJOY {} points)", t.pairs.len());
            all_same = false;
            continue;
        };
        let (a, b) = (&mine.pairs, &t.pairs);
        let same_pairs = a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y);
        let shared = {
            let (mut i, mut j, mut n) = (0, 0, 0);
            while i < a.len() && j < b.len() {
                if a[i].0 == b[j].0 {
                    n += 1;
                    i += 1;
                    j += 1;
                } else if a[i].0 < b[j].0 {
                    i += 1;
                } else {
                    j += 1;
                }
            }
            n
        };
        if same_pairs && mine.qi == t.head.c2 {
            println!("MT={mt:4}  SAME ({} points, every word)", b.len());
            continue;
        }
        all_same = false;
        let first = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        println!(
            "MT={mt:4}  DIFFERENT: ours {} points, NJOY {}, {} energies shared; QI ours {} NJOY {}",
            a.len(),
            b.len(),
            shared,
            mine.qi,
            t.head.c2
        );
        let lo = first.saturating_sub(2);
        for k in lo..(first + 4) {
            let x = a.get(k).map_or("-".to_string(), |p| format!("({:.9e}, {:.9e})", p.0, p.1));
            let y = b.get(k).map_or("-".to_string(), |p| format!("({:.9e}, {:.9e})", p.0, p.1));
            println!("        [{k}] ours {x}  NJOY {y}");
        }
    }
    for s in &ours.sections {
        if !their_mts.contains(&s.mt.number()) {
            println!("MT={:4}  EXTRA in ours ({} points)", s.mt.number(), s.pairs.len());
            all_same = false;
        }
    }
    println!("VERDICT: {}", if all_same { "SAME" } else { "DIFFERENT" });
}
