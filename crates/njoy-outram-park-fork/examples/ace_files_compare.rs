// SPDX-License-Identifier: AGPL-3.0-only
//! Compare two Type-1 ACE files word for word and block by block
//! (`acer::compare`): `ace_files_compare <first.ace> <second.ace>`.
//!
//! Used by the deck-driver differential (gh:#1369) on the ACE file the deck
//! driver writes and the one the NJOY2016.80 binary writes for the same deck.
//! Exit code 0 when identical in every word, 3 otherwise. Example program:
//! `main` glue, reached by `tests/deck_driver_tapes_vs_njoy2016.rs` through the
//! library functions it calls.

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read::parse_type1;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [a, b] = &args[..] else {
        eprintln!("usage: ace_files_compare <first.ace> <second.ace>");
        std::process::exit(2);
    };
    let read = |p: &String| {
        let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        parse_type1(&text).unwrap_or_else(|e| panic!("{p}: {e}"))
    };
    let c = compare(&read(a), &read(b));
    for l in c.report_lines() {
        println!("{l}");
    }
    std::process::exit(if c.identical() { 0 } else { 3 });
}
