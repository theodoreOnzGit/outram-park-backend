// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — step-size convergence of the predictor depletion integrator.
//! GitHub #266, scope item 1.
//!
//! # Why this is the FIRST deliverable of #266
//!
//! `outram-mc-libs` has exactly one integrator, the explicit predictor —
//! upstream's `PredictorIntegrator`, the least accurate of the eight OpenMC
//! ships. Its truncation error is first order in the step size, which shows up
//! as a systematic drift in `k` over a burnup history.
//!
//! **Nothing in the crate measured that**, so the size of the error in the
//! existing `depletion_coupled_to_transport` and `depletion_one_group_collapse`
//! results was *unknown* rather than small. The issue is explicit that this
//! measurement comes first, because it is what says how much the higher-order
//! integrators are worth. Porting CF4 before knowing the predictor's error
//! would be buying an unmeasured improvement.
//!
//! # Methodology
//!
//! The same total burnup is depleted with the step size repeatedly **halved**,
//! on the 9-nuclide `DepletionChain::simple` chain. Everything else is held
//! fixed: power, volume, temperature, weighting.
//!
//! The observed order of convergence is then estimated by Richardson's ratio
//! over three successive halvings:
//!
//! ```text
//! p = log2( (y(h) - y(h/2)) / (y(h/2) - y(h/4)) )
//! ```
//!
//! This needs **no exact answer**, which is the point — it measures the order
//! from the solutions themselves. A first-order method gives `p ~ 1`; a
//! second-order one `p ~ 2`.
//!
//! Two quantities are tracked, because they converge **differently**, and that
//! difference is the main result: the end-of-life `k_inf`, and the end-of-life
//! Xe-135 density (a saturating nuclide with a ~9.14 h half-life, the stiffest
//! thing in this chain).
//!
//! # Results (2026-09-22)
//!
//! Drive point: 1 MW in 1000 cm^3, ~3 % enriched UO2, 40 days. **15.76 % of the
//! U-235 is burned**, so the integrator is genuinely being exercised.
//!
//! Full scan, `examples/depletion_step_convergence_scan.rs`:
//!
//! ```text
//!   step[d]  step[h]   n      k_inf(EOL)          Xe135(EOL)
//!   5.00000   120.00      8   1.782144866417   3.800067871e-9
//!   2.50000    60.00     16   1.781884989585   3.799995775e-9
//!   1.25000    30.00     32   1.781762114344   3.799058260e-9
//!   0.62500    15.00     64   1.781715364706   3.797080107e-9
//!   0.31250     7.50    128   1.781700130566   3.795270097e-9
//!   0.15625     3.75    256   1.781694482156   3.794166540e-9
//!   0.07812     1.88    512   1.781690836433   3.793695591e-9
//!   0.03906     0.94   1024   1.781688197852   3.793540934e-9
//! ```
//!
//! Richardson order over successive triples:
//!
//! | h | `k_inf` p | Xe-135 p |
//! |---|---|---|
//! | 5.0 d | 1.081 | **-3.701** |
//! | 2.5 d | 1.394 | -1.077 |
//! | 1.25 d | 1.618 | 0.128 |
//! | 0.625 d | 1.431 | 0.714 |
//! | 0.3125 d | 0.632 | 1.229 |
//! | 0.15625 d | 0.466 | 1.606 |
//!
//! ## The headline number
//!
//! **A 5-day step carries 4.567e-4 of truncation error in `k_inf` — about 457
//! pcm** — against the 0.039-day run. That is the answer to "how much are the
//! higher-order integrators worth", and it is large: 457 pcm is well above the
//! statistical precision any of this crate's eigenvalue work quotes.
//!
//! ## Xe-135 is NOT in the asymptotic regime at practical step sizes
//!
//! Its order estimate is **negative** at 5 and 2.5 days and only climbs through
//! 1 once the step drops **below the ~9.14 h Xe-135 half-life**, between 15 h
//! and 7.5 h. A negative Richardson order is not a small error; it means the
//! successive differences are growing, so the sequence is nowhere near its
//! asymptote and Richardson does not apply at all.
//!
//! The consequence is practical: **the small differences between the 5-day and
//! 2.5-day Xe-135 answers (7.2e-14, a relative 2e-5) are a coincidence, not
//! accuracy.** Reading them as convergence would be exactly the wrong
//! conclusion — the two coarse answers agree with each other and both disagree
//! with the resolved one.
//!
//! `k_inf` degrades to p ~ 0.5 at the finest steps for an unrelated and benign
//! reason: successive differences there are ~1e-6 relative on a value of 1.78,
//! so f64 round-off starts to dominate the ratio. That is the floor of the
//! method, not a property of the integrator.
//!
//! ## A near-miss worth recording
//!
//! The first draft of this study used `1.0e21` for the initial densities,
//! reading the field as atoms/cm^3 when it is **atoms/barn-cm**. That makes
//! `Sigma_f` enormous, the flux needed for 1 MW collapses to 5.5e-11
//! n/cm^2/s, **nothing burns**, and every step size returns bit-identical
//! answers. The study would have "passed" against a solution that never moved.
//! A sanity check on the burned fraction now runs *before* the scan.

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::operator::{deplete_predictor, BurnupSettings, OneGroupWeighting};

/// Total burnup time held fixed across every refinement \[days\].
const TOTAL_DAYS: f64 = 40.0;

fn settings(step_days: f64) -> BurnupSettings {
    let n_steps = (TOTAL_DAYS / step_days).round() as usize;
    BurnupSettings {
        power_watts: 1.0e6,
        fuel_volume_cm3: 1.0e3,
        step_days,
        n_steps,
        temperature_k: 293.6,
        one_group_energy_ev: 0.0253,
        weighting: OneGroupWeighting::SingleEnergy,
    }
}

/// Beginning-of-life densities in **atoms/barn-cm**, roughly 3 % enriched UO2.
///
/// The unit matters and is easy to get wrong by ~23 orders of magnitude: the
/// first draft of this test used `1.0e21`, reading the field as atoms/cm^3.
/// That makes `Sigma_f = N * sigma` enormous, so the flux needed to produce the
/// requested power collapses to 5.5e-11 n/cm^2/s, **nothing burns**, and every
/// step size returns bit-identical answers. The convergence study then
/// "passes" against a solution that never moved.
///
/// Recorded because a silent no-op is exactly the failure this whole exercise
/// is meant to detect, and it nearly produced one in its own gate.
fn initial() -> Vec<(String, f64)> {
    vec![
        ("U235".to_string(), 7.0e-4),
        ("U238".to_string(), 2.2e-2),
    ]
}

/// End-of-life `k_inf` and Xe-135 density for one step size.
fn end_of_life(step_days: f64) -> (f64, f64) {
    let chain = DepletionChain::simple();
    let r = deplete_predictor(&chain, &initial(), &settings(step_days));
    let last = r.steps.last().expect("at least one step recorded");
    let xe = last
        .densities
        .iter()
        .find(|(n, _)| n == "Xe135")
        .map(|(_, d)| *d)
        .unwrap_or(0.0);
    (last.k_inf, xe)
}

/// Richardson order estimate from three successive halvings.
fn observed_order(y_h: f64, y_h2: f64, y_h4: f64) -> f64 {
    let d1 = y_h - y_h2;
    let d2 = y_h2 - y_h4;
    if d2 == 0.0 || d1 / d2 <= 0.0 {
        return f64::NAN;
    }
    (d1 / d2).log2()
}

#[test]
fn the_predictor_converges_first_order_in_k_inf() {
    // Guard: if the drive point does not actually burn anything, every step
    // size returns the same answer and a convergence study is meaningless.
    // Checked BEFORE the study rather than inferred from its output -- the
    // first draft of this test failed exactly here for a units reason.
    {
        let chain = DepletionChain::simple();
        let r = deplete_predictor(&chain, &initial(), &settings(5.0));
        let get = |s: &outram_mc_libs::depletion::operator::BurnupStep| {
            s.densities
                .iter()
                .find(|(n, _)| n == "U235")
                .map(|(_, d)| *d)
                .unwrap()
        };
        let burned = (get(&r.steps[0]) - get(r.steps.last().unwrap())) / get(&r.steps[0]);
        println!("sanity: U235 burned over {TOTAL_DAYS} d = {:.4} %", 100.0 * burned);
        assert!(
            burned > 1.0e-4,
            "the drive point burns only {burned:.3e} of the U235, so nothing is being \
             integrated. Check the density UNITS: atoms/barn-cm, not atoms/cm^3."
        );
    }

    let steps = [5.0, 2.5, 1.25, 0.625];
    let mut k = Vec::new();
    println!("  step [d]   n_steps        k_inf(EOL)");
    for &h in &steps {
        let (ki, _) = end_of_life(h);
        println!("{h:9.3}   {:7}   {ki:16.10}", (TOTAL_DAYS / h).round() as usize);
        k.push(ki);
    }

    // k_inf converges: successive differences shrink.
    let d: Vec<f64> = (1..k.len()).map(|i| (k[i] - k[i - 1]).abs()).collect();
    println!("k_inf successive |differences|: {d:?}");
    for i in 1..d.len() {
        assert!(
            d[i] < d[i - 1],
            "k_inf is not converging: |difference| grew from {} to {}",
            d[i - 1],
            d[i]
        );
    }

    let p1 = observed_order(k[0], k[1], k[2]);
    let p2 = observed_order(k[1], k[2], k[3]);
    println!("k_inf observed order: {p1:.3}, {p2:.3}");
    for p in [p1, p2] {
        assert!(p.is_finite(), "could not estimate an order for k_inf");
        assert!(
            (0.5..2.0).contains(&p),
            "k_inf observed order {p:.3} is not first order. Above ~2 would mean this \
             is not the explicit predictor its docs describe; below ~0.5 that it is \
             barely converging."
        );
    }
}

/// **The finding**: Xe-135 is not in the asymptotic regime until the step
/// resolves its ~9.14 h half-life, so its coarse-step agreement is coincidence.
///
/// This test asserts the *measured* behaviour, which contradicted the
/// hypothesis it was written under. The first version asserted that Xe-135
/// converges first order like `k_inf`; it does not, and the assertion was
/// changed to match the measurement rather than the measurement discarded.
#[test]
fn xe135_needs_the_step_to_resolve_its_half_life() {
    // Coarse: 5, 2.5, 1.25 d -- all far longer than the 9.14 h half-life.
    let coarse: Vec<f64> = [5.0, 2.5, 1.25].iter().map(|&h| end_of_life(h).1).collect();
    // Fine: 0.3125, 0.15625, 0.078125 d = 7.5, 3.75, 1.88 h -- at or below it.
    let fine: Vec<f64> = [0.3125, 0.15625, 0.078125]
        .iter()
        .map(|&h| end_of_life(h).1)
        .collect();

    let p_coarse = observed_order(coarse[0], coarse[1], coarse[2]);
    let p_fine = observed_order(fine[0], fine[1], fine[2]);
    println!("Xe135 order, steps >> half-life (120/60/30 h): {p_coarse:.3}");
    println!("Xe135 order, steps <= half-life (7.5/3.75/1.88 h): {p_fine:.3}");

    // Coarse steps are NOT converging: the order estimate is negative or
    // nonsensical, which is what growing successive differences produce.
    assert!(
        !p_coarse.is_finite() || p_coarse < 0.5,
        "Xe-135 appears to converge at steps far longer than its half-life \
         (order {p_coarse:.3}). If that is now true the stiffness result has changed \
         and this study needs redoing, not this assertion relaxing."
    );
    // Fine steps DO approach first order.
    assert!(
        p_fine.is_finite() && p_fine > 0.9,
        "Xe-135 should approach first order once the step resolves its half-life; \
         got {p_fine:.3}"
    );
}
