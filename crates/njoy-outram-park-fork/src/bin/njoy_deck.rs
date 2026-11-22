// SPDX-License-Identifier: AGPL-3.0-only
//! `njoy-deck`: run an NJOY input deck (gh:#1369), like `njoy < deck`.
//!
//! `njoy-deck <deck> [workdir]`: `tape<n>` files are read from and the coded
//! output units written to `workdir` (default: the current directory).
//! CLI glue, no test of its own: [`njoy_outram_park_fork::deck::run_deck`] is
//! reached by `tests/deck_driver_code_to_code.rs`.

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: njoy-deck <deck> [workdir]");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
        eprintln!("njoy-deck: cannot read {}: {e}", args[1]);
        std::process::exit(2)
    });
    let workdir = args.get(2).map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from("."));
    match run_deck(&text, &DeckOptions { workdir: Some(workdir), parse_only: false, run_date: None }) {
        Ok(run) => {
            println!("modules run: {:?}", run.modules);
            print!("{}", run.listing);
            for n in &run.notes {
                eprintln!("njoy-deck: note: {n}");
            }
            if let Some((m, why)) = run.blocked {
                eprintln!("njoy-deck: stopped at {m}: {why}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("njoy-deck: {e}");
            std::process::exit(1);
        }
    }
}
