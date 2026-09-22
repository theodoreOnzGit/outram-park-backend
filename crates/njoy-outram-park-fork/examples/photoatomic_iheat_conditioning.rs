//! Is ACER's incoherent-heating integral well conditioned at high energy?
//!
//! ## Why this exists
//!
//! The port's photo-atomic heating column agrees with NJOY2016 to ~1e-7
//! relative everywhere except a band at high energy, and the disagreement
//! attributes entirely to `iheat`'s incoherent term (~7e-5 relative there).
//! Before calling that a translation defect, the question worth asking is
//! whether `iheat` is *conditioned* well enough for a 1-ulp difference in its
//! input — which is all a differently-contracted floating-point expression
//! produces — to be worth 7e-5 in its output.
//!
//! ## Methodology
//!
//! At each requested energy, evaluate `iheat` at `e` and at `e * (1 +- 1e-15)`
//! (one to two ulp) and report how far `heat` moves. A routine whose answer is
//! stable to 1e-12 under that perturbation is well conditioned and a 7e-5
//! disagreement would be a real defect; one whose answer moves by 1e-5 or more
//! is amplifying round-off, and the port cannot be expected to reproduce the
//! Fortran's last digits.
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork \
//!     --example photoatomic_iheat_conditioning -- <tape.endf> <mat> [E_MeV ...]
//! ```
//!
//! ## Results
//!
//! Recorded in
//! `verification_and_validation/acer_photoatomic_vs_njoy2016.md`.

use njoy_outram_park_fork::acer::photoatomic::incoherent_heating::{
    iheat, iheat_ablate_cancellation, iheat_refined,
};
use njoy_outram_park_fork::acer::photoatomic::incoherent_scattering_function;
use njoy_outram_park_fork::endf::tape::Tape;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: photoatomic_iheat_conditioning <tape.endf> <mat> [E_MeV ...]");
        std::process::exit(2);
    }
    let tape = Tape::read_file(std::path::Path::new(&args[0])).expect("read ENDF tape");
    let mat: i32 = args[1].parse().expect("mat");
    let sf = incoherent_scattering_function(&tape, mat).expect("MF=27/MT=504");

    let energies: Vec<f64> = if args.len() > 2 {
        args[2..].iter().map(|a| a.parse().expect("E")).collect()
    } else {
        vec![1.0e-3, 1.0e-2, 0.1, 1.0, 10.0, 100.0, 1.0e3, 1.4727e3, 1.0e4, 1.0e5]
    };
    // A fine sweep across one energy shows whether `heat(E)` is smooth there
    // or steps -- a step means the panel set changed, and two implementations
    // of the same algorithm can legitimately land on opposite sides of it.
    if let Ok(c) = std::env::var("OUTRAM_PARK_IHEAT_SWEEP") {
        let centre: f64 = c.parse().expect("centre energy in MeV");
        println!("sweep around E = {centre:.6e} MeV:");
        println!("{:>16}  {:>18}  {:>7}", "E (MeV)", "heat (eV)", "panels");
        for k in -10i32..=10 {
            let e_mev = centre * (1.0 + (k as f64) * 1.0e-7);
            let r = iheat(e_mev * 1.0e6, &sf);
            println!("{e_mev:>16.10e}  {:>18.10e}  {:>7}", r.heat, r.panels);
        }
        println!();
    }
    println!(
        "{:>12}  {:>16}  {:>16}  {:>10}  {:>10}",
        "E (MeV)", "heat (eV)", "siginc (b)", "d(heat)/heat", "verdict"
    );
    println!("  (then: the 1 - unow cancellation's worth, then the panel rule's own truncation)");
    for e_mev in energies {
        let e = e_mev * 1.0e6;
        let base = iheat(e, &sf);
        let up = iheat(e * (1.0 + 1.0e-15), &sf);
        let dn = iheat(e * (1.0 - 1.0e-15), &sf);
        // Subtract the input's own 1e-15 drift: `heat = e - ebar`, so a shift
        // of `e` moves `heat` by at least that much legitimately.
        let d = ((up.heat - base.heat).abs().max((dn.heat - base.heat).abs()) / base.heat.abs()
            - 1.0e-15)
            .max(0.0);
        let fine = iheat_refined(e, &sf, 128.0);
        let trunc = (fine.heat - base.heat).abs() / base.heat.abs();
        let stable = iheat_ablate_cancellation(e, &sf);
        let cancel = (stable.heat - base.heat).abs() / base.heat.abs();
        let verdict = if d > 1.0e-6 {
            "AMPLIFIES"
        } else if d > 1.0e-10 {
            "sensitive"
        } else {
            "stable"
        };
        println!(
            "{e_mev:>12.4e}  {:>16.8e}  {:>16.8e}  {d:>10.2e}  {verdict:>10}  \
             {cancel:>10.2e}  {trunc:>10.2e}  ({} -> {} panels)",
            base.heat, base.siginc, base.panels, fine.panels
        );
    }
}
