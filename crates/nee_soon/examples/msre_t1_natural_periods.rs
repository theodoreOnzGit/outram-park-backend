//! **MSRE T-1 (gh:#1132): natural periods of oscillation against power**,
//! from `nee_soon::msre_frequency_response`, beside TM-1647 Fig. 29's
//! measured values.
//!
//! ```text
//! cargo run --release -p nee_soon --example msre_t1_natural_periods -- \
//!     --fuel-per-f -4.0e-5 --graphite-per-f -3.5e-5
//! ```
//!
//! The coefficients are in ORNL's units (δk/k per °F). With no flags it uses
//! TM-1070 Table 1's (−4.84, −3.7 × 10⁻⁵). T-1 proper uses N-3's Monte
//! Carlo split (gh:#1129). The results are recorded in
//! `verification_and_validation/msre_t1_frequency_response/README.md`.
//!
//! CLI glue (`main` and argument parsing; the workspace's exception for
//! `main` glue): the model and the measured table are reached by the module's
//! tests.

use nee_soon::msre_frequency_response::{
    per_f, MsreLoopModel, MEASURED_PERIODS_TM1647, ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F,
};
use uom::si::f64::Power;
use uom::si::power::megawatt;
use uom::si::time::second;

fn flag(args: &[String], name: &str) -> Option<f64> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|v| v.parse().expect("a number"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fuel = flag(&args, "--fuel-per-f").unwrap_or(ORNL_1965_FUEL_PER_F);
    let graphite = flag(&args, "--graphite-per-f").unwrap_or(ORNL_1965_GRAPHITE_PER_F);
    println!("fuel {fuel:.3e} /F, graphite {graphite:.3e} /F");
    println!("| P [MW] | measured [min] | model [min] | model / measured |");
    println!("|---|---|---|---|");
    for (p, t_meas) in MEASURED_PERIODS_TM1647 {
        let m = MsreLoopModel::tm1070(Power::new::<megawatt>(p), per_f(fuel), per_f(graphite));
        match m.natural_period(600) {
            Some(t) => {
                let t = t.get::<second>() / 60.0;
                println!("| {p:.3} | {t_meas:.2} | {t:.2} | {:.3} |", t / t_meas);
            }
            None => println!("| {p:.3} | {t_meas:.2} | no resonance | - |"),
        }
    }
}
