//! One-at-a-time input sensitivity of the Fukushima Unit 1 model
//! (`sembawang::bwr_mark1`), to rank which unverified inputs plan item F13
//! (gh:#1044) checks first. Prints the Markdown table recorded in
//! `verification_and_validation/bwr_mark1/input-sensitivity.md`, which holds
//! the methodology, the predictions and the results.
//!
//! ```text
//! cargo run --release -p sembawang --example bwr_mark1_sensitivity [-- <rel>]
//! ```
//!
//! `<rel>` is the relative step, default 0.01. The runs are independent and
//! deterministic, so they are spread over the machine's threads; the table
//! does not depend on how many. Research, education and V&V only.

use sembawang::bwr_mark1::compare::{elasticity, measure, sensitivity_report, Elasticity};
use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::Mechanisms;

/// CLI glue around `compare::elasticity` and `sensitivity_report`, which the
/// library's tests reach; the example itself has no test.
fn main() {
    let rel: f64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(0.01);
    let base = measure(Mechanisms::default());
    let names: Vec<&'static str> = Params::INPUTS.iter().map(|(n, _)| *n).collect();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = names.len().div_ceil(threads);
    let rows: Vec<Elasticity> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .chunks(chunk)
            .map(|c| s.spawn(|| c.iter().filter_map(|n| elasticity(n, rel, &base)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("a sensitivity run panicked")).collect()
    });
    println!("Baseline: {base:?}\nRelative step: {rel}\n");
    print!("{}", sensitivity_report(&rows));
}
