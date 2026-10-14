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
    let ours = read::parse_type1(&ace.to_type1_text().expect("type 1")).expect("re-read ours");
    let bytes = std::fs::read(njoy_path).expect("read the NJOY table");
    let njoy = read::read_library_from_bytes(bytes, njoy_path).expect("read NJOY's").remove(0);
    let c = compare(&ours, &njoy);
    println!("{tape_name} at {t_k} K, heatr {heatr}, purr {purr} ({:.1} s): first = ours, second = NJOY2016", t0.elapsed().as_secs_f64());
    for l in c.report_lines() {
        println!("  {l}");
    }
    std::process::exit(if c.identical() { 0 } else { 3 });
}
