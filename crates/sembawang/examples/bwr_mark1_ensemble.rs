//! A perturbation ensemble of the Fukushima Unit 1 model
//! (`sembawang::bwr_mark1`): the fourth input-sensitivity run found the run
//! chaotic at the 1e-9 level (a perturbation that small moves the lower
//! head's failure between branches), so a single run's numbers are one
//! trajectory. This runs `n` members with every input scaled by
//! `1 + rel u`, `u` uniform in [-1, 1] (`compare::ensemble_member`), and
//! prints the distribution (`compare::ensemble_report`) recorded in
//! `verification_and_validation/bwr_mark1/ensemble.md`.
//!
//! ```text
//! cargo run --release -p sembawang --example bwr_mark1_ensemble [-- <n> <rel> <seed>]
//! ```
//!
//! Defaults: 64 members, `rel` 1e-9, seed 1. Members are independent and
//! deterministic, spread over the machine's threads; the result does not
//! depend on how many. Research, education and V&V only.

use sembawang::bwr_mark1::compare::{ensemble_member, ensemble_report, Metrics};

/// CLI glue around `compare::ensemble_member` and `ensemble_report`, which
/// the library's tests reach; the example itself has no test.
fn main() {
    let arg = |i: usize| std::env::args().nth(i);
    let n: u64 = arg(1).and_then(|a| a.parse().ok()).unwrap_or(64);
    let rel: f64 = arg(2).and_then(|a| a.parse().ok()).unwrap_or(1e-9);
    let seed: u64 = arg(3).and_then(|a| a.parse().ok()).unwrap_or(1);
    let threads = std::thread::available_parallelism().map_or(1, |t| t.get()) as u64;
    let runs: Vec<Metrics> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| s.spawn(move || (0..n).filter(|k| k % threads == t).map(|k| (k, ensemble_member(k, rel, seed))).collect::<Vec<_>>()))
            .collect();
        let mut all: Vec<(u64, Metrics)> = handles.into_iter().flat_map(|h| h.join().expect("an ensemble member panicked")).collect();
        all.sort_by_key(|(k, _)| *k);
        all.into_iter().map(|(_, m)| m).collect()
    });
    println!("Relative perturbation: {rel:e}, seed {seed}\n");
    print!("{}", ensemble_report(&runs));
}
