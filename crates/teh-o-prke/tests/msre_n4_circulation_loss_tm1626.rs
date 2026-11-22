//! MSRE N-4 (gh:#1130, epic gh:#1125), lumped-model part: the reactivity
//! lost to fuel circulation, `teh-o-prke`'s circulating point kinetics
//! (uncalibrated) against the MSRE measurement. Validation; research,
//! education and V&V only.
//!
//! # Reference (maintainer's choice, 2026-11-03)
//!
//! B.E. Prince, *Period Measurements on the Molten Salt Reactor Experiment
//! During Fuel Circulation: Theory and Experiment*, ORNL-TM-1626 (1966).
//! - **Measured:** 0.212 ± 0.004 % δk/k (U-235 loading, zero power).
//! - **Prince's spatial model:** 0.222 % (Eq. 14, w = 0).
//!
//! # Inputs (TM-1626 Table 1, report p. 24, PDF p. 30, read from the scan)
//!
//! - 10⁴β_i = 2.23, 14.57, 13.07, 26.28, 7.66, 2.80 (static,
//!   spectrum-corrected; β = 0.006661).
//! - λ_i = 0.0124, 0.0305, 0.1114, 0.3013, 1.140, 3.010 s⁻¹.
//! - Residence times: core (graphite-moderated region) 9.4 s, upper plenum
//!   3.9 s, external system 8.1 s, lower plenum 3.8 s; total 25.2 s.
//! - Λ = 2.40e-4 s (ORNL-TM-0728 Table 5.4); it does not enter the steady
//!   loss.
//!
//! # Method (pre-registered on gh:#1130)
//!
//! `CirculatingPointKinetics`, one well-mixed core of flat flux and flat
//! importance, no parameter adjusted.
//! - **Primary, core only:** τ_c = 9.4 s, τ_L = 15.8 s. Plenum-born delayed
//!   neutrons are counted as worthless, so it should read high.
//! - **Bracket, reported only:** core plus plenums, τ_c = 17.1 s,
//!   τ_L = 8.1 s. They are counted as fully worth, so it should read low.
//!
//! The steady loss is the analytic one T-C1 verified to round-off (gh:#1131).
//! This test also re-runs the solver at that loss to show it holds there.
//!
//! # Results (2026-11-09)
//!
//! | Model | Loss (% δk/k) | vs measured 0.212 ± 0.004 |
//! |---|---|---|
//! | Lumped, core only (primary) | **0.2301** | **+0.018, +8.5 %, +4.5σ** |
//! | Lumped, core + plenums (bracket) | 0.1249 | −0.087 |
//! | Prince's spatial model (TM-1626 Eq. 14) | 0.222 | +0.010 |
//!
//! As predicted: the uncalibrated lumped model reads high. It counts the
//! delayed neutrons born in the plenums as worthless, where their
//! importance is low but not zero. The measurement sits inside the bracket,
//! close to Prince's importance-weighted value. The solver holds n to
//! 1.1e-16 over 300 s at the lumped loss.
//!
//! **Do not tune τ_c, τ_L or β_i towards 0.212.** The 8.5 % is the
//! lumped model's structural limit, and closing it needs spatial importance
//! (Prince's model, or moltres). The moltres cross-check and the U-233 case
//! are not done here (gh:#1130).

use teh_o_prke::prelude::*;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

const BETA_E4: [f64; 6] = [2.23, 14.57, 13.07, 26.28, 7.66, 2.80];
const LAMBDA: [f64; 6] = [0.0124, 0.0305, 0.1114, 0.3013, 1.140, 3.010];
const MEASURED_PCT: f64 = 0.212;
const MEASURED_SIGMA_PCT: f64 = 0.004;
const PRINCE_MODEL_PCT: f64 = 0.222;

fn kinetics(core: f64, loop_: f64) -> CirculatingPointKinetics {
    CirculatingPointKinetics::with_groups(
        BETA_E4.iter().map(|b| Ratio::new::<ratio>(b * 1.0e-4)).collect(),
        LAMBDA.iter().map(|l| Frequency::new::<hertz>(*l)).collect(),
        Time::new::<second>(core),
        Time::new::<second>(loop_),
        Time::new::<second>(2.40e-4),
        Time::new::<second>(0.1),
    )
    .expect("TM-1626 kinetics")
}

#[test]
fn n4_lumped_circulation_loss_against_tm1626() {
    let primary = kinetics(9.4, 15.8);
    let bracket = kinetics(9.4 + 3.9 + 3.8, 8.1);
    let p = 100.0 * primary.circulation_reactivity_loss().get::<ratio>();
    let b = 100.0 * bracket.circulation_reactivity_loss().get::<ratio>();
    println!(
        "N-4: beta = {:.6}; lumped loss, core only = {p:.4} % dk/k ({:+.4} %, {:+.1} % rel, {:+.1} sigma of the measurement); core + plenums = {b:.4} %; measured {MEASURED_PCT} +- {MEASURED_SIGMA_PCT} %; Prince's spatial model {PRINCE_MODEL_PCT} %",
        primary.total_delayed_fraction().get::<ratio>(),
        p - MEASURED_PCT,
        100.0 * (p / MEASURED_PCT - 1.0),
        (p - MEASURED_PCT) / MEASURED_SIGMA_PCT
    );
    // The predictions pre-registered on gh:#1130, reproduced through the type.
    approx::assert_relative_eq!(p, 0.230088847, max_relative = 1e-6);
    approx::assert_relative_eq!(b, 0.124909764, max_relative = 1e-6);
    // The solver holds n at that loss (the steady state it reports exists).
    let mut pk = primary.clone();
    pk.set_circulating_equilibrium(VolumetricNumberDensity::new::<per_cubic_meter>(1.0));
    let zero = VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);
    for _ in 0..3000 {
        pk.step_implicit(primary.circulation_reactivity_loss(), zero).unwrap();
    }
    let drift = (pk.neutron_density().get::<per_cubic_meter>() - 1.0).abs();
    println!("N-4: |n/n0 - 1| after 300 s at the lumped loss = {drift:.2e}");
    assert!(drift < 1.0e-9);
}
