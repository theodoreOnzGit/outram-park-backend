//! Measure the observed order of every integrator on ONE real burnup history.
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::integrators::Integrator;
use outram_mc_libs::depletion::operator::{deplete_with, BurnupSettings, OneGroupWeighting};

const TOTAL_DAYS: f64 = 40.0;

fn settings(step_days: f64) -> BurnupSettings {
    BurnupSettings {
        power_watts: 1.0e6,
        fuel_volume_cm3: 1.0e3,
        step_days,
        n_steps: (TOTAL_DAYS / step_days).round() as usize,
        temperature_k: 293.6,
        one_group_energy_ev: 0.0253,
        weighting: OneGroupWeighting::SingleEnergy,
    }
}
fn initial() -> Vec<(String, f64)> {
    vec![("U235".into(), 7.0e-4), ("U238".into(), 2.2e-2)]
}
fn eol(i: Integrator, h: f64) -> f64 {
    let chain = DepletionChain::simple();
    deplete_with(i, &chain, &initial(), &settings(h)).steps.last().unwrap().k_inf
}
fn order(a: f64, b: f64, c: f64) -> f64 {
    ((a - b).abs() / (b - c).abs()).log2()
}

fn main() {
    // ── Two regimes, measured separately, because mixing them is meaningless ─
    //
    // Xe-135's half-life is 9.14 h = 0.381 d. A step longer than that does not
    // resolve its transient, and NO method is in its asymptotic regime there —
    // the existing convergence test already measures the predictor's Xe-135
    // order as -3.70 at 5 d steps for exactly this reason. So the coarse band
    // says what a method is WORTH at a step someone would actually use, and
    // only the fine band can measure an ORDER.
    const XE135_HALF_LIFE_DAYS: f64 = 9.14 / 24.0;
    let coarse = [5.0_f64, 2.5, 1.25, 0.625];
    let fine = [0.25_f64, 0.125, 0.0625, 0.03125, 0.015625];
    assert!(coarse[coarse.len() - 1] > XE135_HALF_LIFE_DAYS);
    assert!(fine[0] < XE135_HALF_LIFE_DAYS);

    // ── The reference must not be the predictor's own finest run ────────────
    //
    // A first version of this scan used `eol(Predictor, 0.039 d)` as "truth".
    // That is wrong and the data said so: CeCm's and Cf4's errors came out
    // NON-MONOTONIC (2.95e-5 -> 4.99e-5 -> 5.03e-5 -> 3.51e-5 -> 9.31e-6),
    // which is the signature of a higher-order method CROSSING a biased
    // reference rather than converging to it. The predictor's finest answer
    // still carries first-order truncation error, so no error measured against
    // it can fall below that bias, and every "order" so measured is an artefact
    // of the reference.
    //
    // Richardson-extrapolate the predictor's two finest points instead. The
    // predictor is measured at p ~ 1 on this problem, so
    // `k* = 2 k(h/2) - k(h)` removes the leading term.
    // Reference: the highest-order method at the finest step, INSIDE the
    // asymptotic regime. Using the predictor's finest run instead (a first
    // version of this scan did) makes the reference carry first-order error, so
    // no measured error can fall below that bias and the higher-order methods
    // appear to diverge as they cross it.
    let reference = eol(Integrator::Cf4, 0.0078125);
    println!("Xe-135 half-life = {:.4} d; steps above it are outside every method's",
             XE135_HALF_LIFE_DAYS);
    println!("asymptotic regime, so order is measured only below it.");
    println!("reference: Cf4 at h=0.0078 d  k_inf = {reference:.12}\n");
    for (label, hs) in [("COARSE (step > Xe-135 half-life)", &coarse[..]),
                        ("FINE (step < Xe-135 half-life)", &fine[..])] {
        println!("===== {label} =====");
        for i in [Integrator::Predictor, Integrator::CeCm, Integrator::Cf4] {
            let ks: Vec<f64> = hs.iter().map(|&h| eol(i, h)).collect();
            print!("{i:?} (nominal {}, {} solves/step):",
                   i.nominal_order(), i.solves_per_step());
            println!();
            for (j, &h) in hs.iter().enumerate() {
                let err = (ks[j] - reference).abs();
                print!("   h={h:9.6} d  k_inf={:.12}  |err|={err:.3e}", ks[j]);
                if j + 2 < ks.len() {
                    print!("  p={:6.3}", order(ks[j], ks[j + 1], ks[j + 2]));
                }
                println!();
            }
            // Error relative to the predictor at the SAME step -- the practical
            // statement: what does the extra solve cost buy at a usable step?
            let p_err = (eol(Integrator::Predictor, hs[0]) - reference).abs();
            let m_err = (ks[0] - reference).abs();
            if m_err > 0.0 {
                println!("   at h={:.6} d this is {:.1}x the predictor's accuracy for {}x the solves",
                         hs[0], p_err / m_err, i.solves_per_step());
            }
            println!();
        }
    }
}
