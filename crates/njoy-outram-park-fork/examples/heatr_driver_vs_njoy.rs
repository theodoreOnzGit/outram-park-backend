// SPDX-License-Identifier: GPL-3.0

//! Instrument: run `heatr::heatr` (the full HEATR translation) on NJOY2016's
//! own RECONR PENDF and compare every heating MT it writes with NJOY2016's
//! HEATR output, word for word after both have been through ENDF text.
//!
//! ```text
//! cargo run --release --example heatr_driver_vs_njoy -- \
//!     <endf> <njoy pendf (ascii)> <njoy heatr tape (ascii)> "<heatr cards>"
//! ```
//!
//! The cards are HEATR's (card 1 included, its units ignored). For each MT
//! of 301-449 on NJOY's tape it prints the number of points, how many differ,
//! the worst relative difference and the first few differing points.

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{heatr, HeatrInput};
use std::path::Path;

fn pairs(t: &Tape, mat: i32, mt: i32) -> Option<Vec<(f64, f64)>> {
    let s = t.section(mat, 3, mt)?;
    let mut c = SectionCursor::new(&s.rows);
    c.read_cont().ok()?;
    Some(c.read_tab1().ok()?.pairs)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: heatr_driver_vs_njoy <endf> <pendf> <njoy heatr> \"<cards>\"");
        std::process::exit(2);
    }
    let endf = Tape::read_file(Path::new(&a[1])).expect("endf");
    let pendf = Tape::read_file(Path::new(&a[2])).expect("pendf");
    let theirs = Tape::read_file(Path::new(&a[3])).expect("njoy heatr tape");
    let cards = a[4].replace("\\n", "\n");
    let (_, input) = HeatrInput::from_cards(&cards).expect("cards");
    let t0 = std::time::Instant::now();
    let out = match heatr(&endf, &pendf, &input) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("heatr failed: {e}");
            std::process::exit(1);
        }
    };
    let secs = t0.elapsed().as_secs_f64();
    // Through ENDF text, as NJOY's tape is.
    let mut buf = Vec::new();
    out.tape.write(&mut buf).expect("write");
    let ours = Tape::read(&buf[..]).expect("read back");
    if std::env::var("HEATR_LISTING").is_ok() {
        println!("{}", out.listing);
    }
    let mat = input.matd;
    println!("heatr ran in {secs:.1} s");
    let mut total_diff = 0usize;
    let mts: Vec<i32> = theirs.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3 && (301..450).contains(&s.key.mt)).map(|s| s.key.mt).collect();
    for mt in mts {
        let t = pairs(&theirs, mat, mt).unwrap_or_default();
        let Some(o) = pairs(&ours, mat, mt) else {
            println!("MT={mt}: MISSING from ours ({} points in NJOY's)", t.len());
            total_diff += 1;
            continue;
        };
        let n = t.len().min(o.len());
        let mut ndiff = 0usize;
        let mut worst = 0.0f64;
        let mut worst_at = (0.0, 0.0, 0.0);
        let mut shown = 0;
        for i in 0..n {
            let (te, tv) = t[i];
            let (oe, ov) = o[i];
            if te != oe || tv != ov {
                ndiff += 1;
                let rel = if tv != 0.0 { ((ov - tv) / tv).abs() } else if ov != 0.0 { f64::INFINITY } else { 0.0 };
                if te == oe && rel > worst {
                    worst = rel;
                    worst_at = (te, tv, ov);
                }
                if shown < 4 && std::env::var("HEATR_SHOW").is_ok() {
                    println!("    [{i}] njoy ({te:.9e}, {tv:.9e})  ours ({oe:.9e}, {ov:.9e})");
                    shown += 1;
                }
            }
        }
        let same_len = t.len() == o.len();
        println!(
            "MT={mt}: njoy {} pts, ours {} pts{}; {ndiff} differ; worst rel {worst:.3e} at E={:.6e} (njoy {:.6e}, ours {:.6e})",
            t.len(),
            o.len(),
            if same_len { "" } else { " (LENGTH DIFFERS)" },
            worst_at.0,
            worst_at.1,
            worst_at.2
        );
        total_diff += ndiff + usize::from(!same_len);
    }
    println!("total differing words or sections: {total_diff}");
    // The whole tape, byte for byte.
    let ours_text = String::from_utf8_lossy(&buf).to_string();
    let theirs_text = std::fs::read_to_string(&a[3]).unwrap_or_default();
    let ol: Vec<&str> = ours_text.lines().collect();
    let tl: Vec<&str> = theirs_text.lines().collect();
    let same = ol.iter().zip(tl.iter()).filter(|(x, y)| x == y).count();
    let cut = |l: &str| l.get(..75).unwrap_or(l).to_string();
    let same75 = ol.iter().zip(tl.iter()).filter(|(x, y)| cut(x) == cut(y)).count();
    println!(
        "whole tape: {same} / {} lines identical, {same75} identical in columns 1-75 (ours {} lines)",
        tl.len(),
        ol.len()
    );
    if std::env::var("HEATR_SHOW").is_ok() {
        for (i, (x, y)) in ol.iter().zip(tl.iter()).enumerate().filter(|(_, (x, y))| cut(x) != cut(y)).take(6) {
            println!("  line {}:\n    ours  |{x}|\n    njoy  |{y}|", i + 1);
        }
    }
}
