// SPDX-License-Identifier: GPL-3.0

//! The three core-venting modes of `accident_release_with_venting` (GitHub
//! #446).
//!
//! # Methodology
//!
//! Consistency checks on the orchestration, with the same inventory and plant
//! as `accident_release.rs`. They pin:
//!
//! 1. **Upstream's uniform-constant-temperature branch**
//!    (`trisoatops.py::accident_case`, commit `de374c8`,
//!    `else: frac = np.ones(np.size(times))`). The port was missing it until
//!    2026-09-30, and an isothermal hold released **0 Bq**. An isothermal hold
//!    must now vent fully at every sample and release a positive amount.
//! 2. **`FullFlowThrough`** equals upstream on an isothermal hold, and bounds
//!    it from above on a heating transient: upstream's ideal-gas fraction
//!    `1 − T0/T` is at most 1.
//! 3. **`Prescribed`**: all ones equals `FullFlowThrough`; a constant 0.5
//!    halves the release; bad input is an error, not a clamp.
//!
//! # Results
//!
//! All pass, `cargo test --release -p sembawang --test venting_modes`,
//! 2026-09-30. The measured isothermal release is printed by
//! `the_isothermal_hold_vents_fully_as_upstream_does` (run with
//! `--nocapture`).

use sembawang::accident::release::{
    accident_release, accident_release_with_venting, AccidentRelease, PlantParameters, Venting,
};
use sembawang::error::Error;
use uom::si::time::hour;
use sembawang::inventory::CoreInventory;
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

const NUCLIDES: [&str; 3] = ["Kr-85", "I-131", "Cs-137"];
const SAMPLES: usize = 21;

fn c(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(x)
}

fn plant() -> PlantParameters {
    // A shakedown choice, not cited: the same as accident_release.rs.
    PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0)
}

fn inventory() -> CoreInventory {
    CoreInventory::unit(&NUCLIDES, 1, 1)
}

/// 1400 °C everywhere, for 1e5 s.
fn isothermal() -> TemperatureTransient {
    let times: Vec<Time> = (0..SAMPLES)
        .map(|i| Time::new::<second>(1.0e5 * i as f64 / (SAMPLES - 1) as f64))
        .collect();
    TemperatureTransient::from_nodes(times, vec![vec![vec![c(1400.0)]; SAMPLES]]).unwrap()
}

/// 600 → 1600 °C over 1e4 s, held to 1e5 s.
fn heating() -> TemperatureTransient {
    TemperatureTransient::from_ramp(
        c(600.0),
        c(1600.0),
        Time::new::<second>(1.0e4),
        Time::new::<second>(1.0e5),
        SAMPLES,
        1,
        1,
    )
    .unwrap()
}

fn totals(out: &AccidentRelease) -> Vec<(String, f64)> {
    out.source_term
        .nuclides
        .iter()
        .map(|r| (r.label.clone(), r.total_released().get::<becquerel>()))
        .collect()
}

fn run(t: &TemperatureTransient, v: &Venting) -> AccidentRelease {
    accident_release_with_venting(&inventory(), t, &plant(), v).unwrap()
}

#[test]
fn the_isothermal_hold_vents_fully_as_upstream_does() {
    let out = accident_release(&inventory(), &isothermal(), &plant()).unwrap();
    assert_eq!(out.venting.len(), SAMPLES);
    assert!(out.venting.fractions().iter().all(|&f| f == 1.0));
    assert!(
        !out.caveats.first_sample_forced_fully_vented,
        "coolant_release must not run on a uniform constant temperature"
    );
    for (n, bq) in totals(&out) {
        println!("isothermal 1400 C, {n}: {bq:.6e} Bq released");
        assert!(bq > 0.0, "{n} released {bq} Bq; before #446 this was exactly 0");
    }
}

#[test]
fn full_flow_through_equals_upstream_on_an_isothermal_hold() {
    let up = totals(&run(&isothermal(), &Venting::Upstream));
    let ff = totals(&run(&isothermal(), &Venting::FullFlowThrough));
    assert_eq!(up, ff);
}

#[test]
fn full_flow_through_bounds_upstream_on_a_heating_transient() {
    let up = totals(&run(&heating(), &Venting::Upstream));
    let ff = totals(&run(&heating(), &Venting::FullFlowThrough));
    for ((n, u), (_, f)) in up.iter().zip(&ff) {
        assert!(f >= u, "{n}: full flow-through {f:e} < upstream {u:e}");
    }
    // And strictly above for at least one nuclide, or the test proves nothing.
    assert!(up.iter().zip(&ff).any(|((_, u), (_, f))| f > u));
}

#[test]
fn prescribed_ones_equal_full_flow_through_and_a_half_halves_the_release() {
    let ones = totals(&run(&heating(), &Venting::Prescribed(vec![1.0; SAMPLES])));
    let ff = totals(&run(&heating(), &Venting::FullFlowThrough));
    assert_eq!(ones, ff);

    // Halving holds for the FUEL release only: the circuit term (circulating +
    // x_liftoff x plate-out) is added OUTSIDE frac, as upstream's
    // `accident_totals` does. So this part runs on the empty-pool ablation,
    // where the circuit term is zero (#448).
    let empty = plant().without_normal_operation_pools();
    let run0 = |v: &Venting| {
        accident_release_with_venting(&inventory(), &heating(), &empty, v).unwrap()
    };
    let ff = totals(&run0(&Venting::FullFlowThrough));
    let half = totals(&run0(&Venting::Prescribed(vec![0.5; SAMPLES])));
    for ((n, h), (_, f)) in half.iter().zip(&ff) {
        let rel = (h - 0.5 * f).abs() / f.max(f64::MIN_POSITIVE);
        assert!(rel < 1e-12, "{n}: 0.5 venting gave {h:e}, expected {:e}", 0.5 * f);
    }
}

#[test]
fn a_bad_prescription_is_an_error_not_a_clamp() {
    let short = accident_release_with_venting(
        &inventory(),
        &heating(),
        &plant(),
        &Venting::Prescribed(vec![1.0; SAMPLES - 1]),
    );
    assert_eq!(
        short.unwrap_err(),
        Error::VentingLengthMismatch { expected: SAMPLES, got: SAMPLES - 1 }
    );

    let mut over = vec![0.5; SAMPLES];
    over[3] = 1.5;
    let bad = accident_release_with_venting(
        &inventory(),
        &heating(),
        &plant(),
        &Venting::Prescribed(over),
    );
    assert_eq!(bad.unwrap_err(), Error::VentingFractionOutOfRange { index: 3, value: 1.5 });
}

/// #447 item 1: a field that is **constant in time but not uniform** is an
/// error under upstream venting. Upstream raises `IndexError` there; the port
/// used to return frac = [1, 0, 0, …], i.e. a silent 0 Bq fuel release.
#[test]
fn a_non_uniform_isothermal_field_is_an_error_not_a_silent_zero() {
    let times: Vec<Time> = (0..SAMPLES)
        .map(|i| Time::new::<second>(1.0e5 * i as f64 / (SAMPLES - 1) as f64))
        .collect();
    // Two axial nodes held at different constant temperatures.
    let field = vec![vec![vec![c(1400.0), c(900.0)]; SAMPLES]];
    let t = TemperatureTransient::from_nodes(times, field).unwrap();
    let inv = CoreInventory::unit(&NUCLIDES, 1, 2);
    let out = accident_release(&inv, &t, &plant());
    assert_eq!(out.unwrap_err(), Error::NoVentingTransport);
    // An explicit transport works on the same field.
    for v in [Venting::FullFlowThrough, Venting::gao_shi_htr10_cavity_ventilation()] {
        let ok = accident_release_with_venting(&inv, &t, &plant(), &v).unwrap();
        assert!(totals(&ok).iter().all(|(_, bq)| *bq > 0.0), "{v:?}");
    }
}

/// The Gao & Shi 2002 §5.3.2 cavity ventilation: 100 %/day, well mixed, cut
/// off at 72 h. frac = 1 − e^(−t/1 d) until 72 h, then frozen at
/// 1 − e⁻³ = 0.9502.
#[test]
fn gao_shi_ventilation_is_one_air_change_per_day_cut_off_at_72_h() {
    let hours = [0.0, 24.0, 48.0, 72.0, 96.0];
    let times: Vec<Time> = hours.iter().map(|h| Time::new::<hour>(*h)).collect();
    let t = TemperatureTransient::from_nodes(times, vec![vec![vec![c(1400.0)]; hours.len()]])
        .unwrap();
    let out = accident_release_with_venting(
        &inventory(),
        &t,
        &plant(),
        &Venting::gao_shi_htr10_cavity_ventilation(),
    )
    .unwrap();
    let expected = [0.0, 1.0 - (-1.0f64).exp(), 1.0 - (-2.0f64).exp(), 1.0 - (-3.0f64).exp(), 1.0 - (-3.0f64).exp()];
    for (f, e) in out.venting.fractions().iter().zip(expected) {
        assert!((f - e).abs() < 1e-12, "{f} against {e}");
    }
    // Never more than full flow-through, nuclide by nuclide.
    let ff = totals(&run(&t, &Venting::FullFlowThrough));
    for ((n, v), (_, f)) in totals(&out).iter().zip(&ff) {
        assert!(v <= f, "{n}: ventilation {v:e} > full flow-through {f:e}");
    }
}
