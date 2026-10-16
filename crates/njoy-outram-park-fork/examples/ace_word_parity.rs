// SPDX-License-Identifier: GPL-3.0

//! **One nuclide's ACE table, ours against an NJOY2016 table, word for word**
//! (GitHub #872).
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --example ace_word_parity -- \
//!     n-001_H_001-ENDF8.0-Beta6.endf 293.6 path/to/njoy/tape24 [--heatr] [--no-purr]
//! ```
//!
//! Ours is `process::cross_sections` on the tape from `reference-data/endf/`
//! at the given temperature (`0` gives the `RECONR -> ACER` deck, with no
//! BROADR and no PURR), with HEATR off unless `--heatr` is passed (the
//! five-route NJOY2016 deck runs none), and PURR at its defaults unless
//! `--no-purr`. Both tables then go through Type-1 text and are compared by
//! `acer::compare`, which names the first differing word of each block. There
//! is no tolerance anywhere.
//!
//! `OUTRAM_ACE_OUT=<file>` also writes our Type-1 table there. A Type-2 NJOY
//! table is compared twice: first **unrounded**, our doubles against
//! NJOY2016's bit for bit, each differing word named by its JXS block (the
//! `EXACT` lines; on 2026-10-15 they found `sigfig`'s bias in GPD and LUNR and
//! B-10's fractional MTRP word, none visible in Type-1 text); then at Type-1
//! print precision (see the code), which alone sets the exit code.
//!
//! Exit code 0 when the tables are identical, 3 when they differ, 1 on an
//! error. CLI glue; `acer::compare` and `process` are tested in the crate.

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings};
use njoy_outram_park_fork::reference_data::reference_endf;
use uom::si::f64::ThermodynamicTemperature as Temperature;
use uom::si::thermodynamic_temperature::kelvin;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [tape_name, temp, njoy_path] = pos[..] else {
        eprintln!("usage: ace_word_parity <tape in reference-data/endf> <T [K], 0 for RECONR->ACER> <njoy ace> [--heatr] [--no-purr]");
        std::process::exit(2);
    };
    let t_k: f64 = temp.parse().expect("temperature in K");
    let heatr = args.iter().any(|a| a == "--heatr");
    let purr = !args.iter().any(|a| a == "--no-purr");

    let path = reference_endf(tape_name).unwrap_or_else(|| panic!("{tape_name} not in reference-data/endf"));
    let tape = Tape::read_file(&path).expect("parse tape");
    let mut settings = CrossSectionSettings::default();
    if !heatr {
        settings = settings.without_heatr();
    }
    if !purr || t_k == 0.0 {
        settings = settings.without_purr();
    }
    let t0 = std::time::Instant::now();
    let ace = match process::ace(&tape, Temperature::new::<kelvin>(t_k), &settings) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("ERROR building ours: {e}");
            std::process::exit(1);
        }
    };
    let text = ace.to_type1_text().expect("type 1");
    // `OUTRAM_ACE_OUT=<file>` keeps our table for inspection.
    if let Ok(out) = std::env::var("OUTRAM_ACE_OUT") {
        std::fs::write(&out, &text).expect("write OUTRAM_ACE_OUT");
    }
    let ours = read::parse_type1(&text).expect("re-read ours");
    let bytes = std::fs::read(njoy_path).expect("read the NJOY table");
    let mut njoy = read::read_library_from_bytes(bytes, njoy_path).expect("read NJOY's").remove(0);
    // A Type-2 (binary) NJOY table carries unrounded doubles. NJOY2016's own
    // Type-1 writer would print it through `change`, which writes integer
    // words with `nint` and reals to 12 figures; reproduce that with our
    // table's integer mask (valid where the layouts agree, which NXS/JXS
    // check separately) and compare at Type-1 print precision. NJOY2016 needs
    // this for B-10, whose Type-1 formatter aborts (`change`, "Undefined law
    // for dlwh block: 0").
    if njoy.file_type == read::AceFileType::Type2Binary {
        // First, the strict test the print-precision one cannot make: our
        // unrounded doubles against NJOY2016's, word for word, bit for bit.
        exact_type2(&ace.nxs, &ace.jxs, &ace.xss, &njoy);
        if njoy.xss.len() == ace.xss_is_int.len() {
            njoy.xss_is_int = Some(ace.xss_is_int.clone());
        }
        let text = njoy.to_type1_string();
        if let Ok(out) = std::env::var("OUTRAM_NJOY_OUT") {
            std::fs::write(&out, &text).expect("write OUTRAM_NJOY_OUT");
        }
        njoy = read::parse_type1(&text).expect("NJOY's Type 2 as Type 1");
        println!("(NJOY2016's table is Type 2: compared at Type-1 print precision, integer words as ours)");
    }
    let c = compare(&ours, &njoy);
    println!("{tape_name} at {t_k} K, heatr {heatr}, purr {purr} ({:.1} s): first = ours, second = NJOY2016", t0.elapsed().as_secs_f64());
    for l in c.report_lines() {
        println!("  {l}");
    }
    std::process::exit(if c.identical() { 0 } else { 3 });
}

/// A Type-2 NJOY table carries the doubles NJOY2016 held in memory. Compare
/// ours against them exactly (no rounding, no mask) and name, for each
/// differing word, the JXS block it falls in. Prints one summary line and up
/// to 20 differing words; it does not change the exit code, which the
/// print-precision comparison below decides.
fn exact_type2(nxs: &[i32; 16], jxs: &[i32; 32], xss: &[f64], njoy: &read::RawAceTable) {
    let same_header = nxs[..] == njoy.nxs[..] && jxs[..] == njoy.jxs[..];
    let n = xss.len().min(njoy.xss.len());
    let block = |i: usize| -> usize {
        // 1-based JXS index of the nearest block start at or below word i+1.
        let w = i as i32 + 1;
        (0..32).filter(|&k| jxs[k] > 0 && jxs[k] <= w).max_by_key(|&k| jxs[k]).map_or(0, |k| k + 1)
    };
    let diffs: Vec<usize> = (0..n).filter(|&i| xss[i].to_bits() != njoy.xss[i].to_bits()).collect();
    let worst = diffs
        .iter()
        .map(|&i| ((xss[i] - njoy.xss[i]) / njoy.xss[i].abs().max(f64::MIN_POSITIVE)).abs())
        .fold(0.0_f64, f64::max);
    println!(
        "EXACT (Type 2, unrounded): NXS/JXS {}, XSS lengths {} / {}, {} of {} words differ in any bit, worst relative {:.3e}",
        if same_header { "equal" } else { "DIFFER" },
        xss.len(),
        njoy.xss.len(),
        diffs.len(),
        n,
        worst
    );
    // Per block: count, worst relative difference and the word it is at.
    let mut per: std::collections::BTreeMap<usize, (usize, f64, usize)> = Default::default();
    for &i in &diffs {
        let r = ((xss[i] - njoy.xss[i]) / njoy.xss[i].abs().max(f64::MIN_POSITIVE)).abs();
        let e = per.entry(block(i)).or_insert((0, 0.0, i));
        e.0 += 1;
        if r > e.1 {
            e.1 = r;
            e.2 = i;
        }
    }
    for (b, (count, r, i)) in &per {
        println!(
            "  EXACT JXS({b}) block (starts at word {}): {count} words differ, worst {r:.3e} at word {} (ours {:e}, NJOY2016 {:e})",
            if *b > 0 { jxs[b - 1] } else { 0 },
            i + 1,
            xss[*i],
            njoy.xss[*i]
        );
    }
}
