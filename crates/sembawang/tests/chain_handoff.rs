// SPDX-License-Identifier: GPL-3.0

//! The handoff from `sembawang` to `changi`: a released source term padded by
//! [`sembawang::chain::pad_for_dispersion`] and carried through `changi`'s
//! dilution factors and survey.
//!
//! # Consistency checks, not verification
//!
//! Neither the padding nor `changi::activity` has an upstream. These assert
//! properties any correct join must have:
//!
//! - padding adds and moves **no** activity, and makes the windows partition
//!   the whole run from `t = 0`;
//! - the air and ground results are **linear in the inventory**;
//! - noble gases deposit **exactly** zero;
//! - `changi::survey` accepts the padded term, i.e. the window and segment
//!   counts line up (it panics otherwise).
//!
//! # Results (2026-09-21)
//! All pass. Linearity holds to 1e-12 relative. That is expected, because
//! the whole chain is linear in the inventory and nothing here is iterative.

use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use changi::activity::survey::{survey, DepositionVelocities, SiteSurvey};
use changi::puff::simulate::{constant_wind, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::chain::pad_for_dispersion;
use sembawang::inventory::{CoreInventory, NuclideInventory};
use sembawang::scenario::TemperatureTransient;
use sembawang::units;
use uom::si::f64::{Frequency, Length, ThermodynamicTemperature, Time, Velocity};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const NUCLIDES: [&str; 3] = ["Kr-85", "I-131", "Cs-137"];
const STEP_S: f64 = 20.0;
const PUFF_LIFETIME_S: f64 = 800.0;
const DISTANCES_M: [f64; 3] = [500.0, 1000.0, 2000.0];

fn s(x: f64) -> Time {
    Time::new::<second>(x)
}

/// A short heat-up, so the dispersion run stays cheap.
fn transient() -> TemperatureTransient {
    TemperatureTransient::from_ramp(
        ThermodynamicTemperature::new::<degree_celsius>(700.0),
        ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        s(4000.0),
        s(20_000.0),
        11,
        2,
        3,
    )
    .unwrap()
}

fn inventory(curies_per_ring: f64) -> CoreInventory {
    CoreInventory::new(
        NUCLIDES
            .iter()
            .map(|n| NuclideInventory::uniform(n, units::from_curies(curies_per_ring), 2))
            .collect(),
        2,
        3,
    )
}

/// Release -> pad -> dilution factors -> survey, the same path as the
/// `npmhtgr_chain` example.
fn run_chain(curies_per_ring: f64) -> (SourceTerm, SiteSurvey) {
    // Accident-phase fractions are a shakedown choice, not cited values.
    let plant = PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0);
    let release = accident_release(&inventory(curies_per_ring), &transient(), &plant).unwrap();
    let term = pad_for_dispersion(&release.source_term, s(PUFF_LIFETIME_S));

    let run = term.end();
    let n_steps = (run.get::<second>() / STEP_S) as usize + 1;
    let config = RunConfig {
        sim_dt: s(STEP_S),
        puff_dt: s(STEP_S),
        output_dt: s(STEP_S),
        duration: run,
        puff_duration: s(PUFF_LIFETIME_S),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
    };
    let source = Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(30.0),
    };
    let receptors = |z: f64| -> Vec<Receptor> {
        DISTANCES_M
            .iter()
            .map(|d| Receptor {
                x: Length::new::<meter>(*d),
                y: Length::new::<meter>(0.0),
                z: Length::new::<meter>(z),
            })
            .collect()
    };
    let wind = constant_wind(
        Velocity::new::<meter_per_second>(4.0),
        Velocity::new::<meter_per_second>(0.0),
        n_steps,
    );
    let bounds = term.segment_boundaries();
    let factors = |z: f64| {
        dilution_factors(
            &[source],
            &bounds,
            &wind,
            &receptors(z),
            &config,
            StabilitySource::Fixed(StabilityClass::D),
        )
    };
    let result = survey(
        &term,
        &factors(1.5),
        &factors(0.0),
        &DepositionVelocities::order_of_magnitude_placeholder(),
    );
    (term, result)
}

#[test]
fn padding_adds_no_activity_and_partitions_the_run_from_zero() {
    let plant = PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0);
    let release = accident_release(&inventory(1.0), &transient(), &plant).unwrap();
    let original = &release.source_term;
    let padded = pad_for_dispersion(original, s(PUFF_LIFETIME_S));

    let bounds = padded.segment_boundaries();
    assert_eq!(bounds[0].get::<second>(), 0.0, "the run must start at t = 0");
    assert_eq!(
        padded.end().get::<second>(),
        original.end().get::<second>() + PUFF_LIFETIME_S,
        "the run must end one tail after the last release"
    );

    for (a, b) in original.nuclides.iter().zip(&padded.nuclides) {
        assert_eq!(a.label, b.label);
        assert_eq!(a.deposition_group, b.deposition_group);
        assert_eq!(a.decay_constant, b.decay_constant);
        assert_eq!(
            a.total_released().get::<becquerel>(),
            b.total_released().get::<becquerel>(),
            "{}: padding must not change the total released",
            a.label
        );
        assert_eq!(b.released.last().unwrap().get::<becquerel>(), 0.0);
    }
}

#[test]
fn a_release_starting_after_zero_gets_a_leading_empty_window() {
    let bq = |x: f64| uom::si::f64::Radioactivity::new::<becquerel>(x);
    let term = SourceTerm::new(
        vec![ReleaseWindow::new(s(100.0), s(200.0))],
        vec![NuclideRelease {
            label: "I-131".to_string(),
            decay_constant: Frequency::new::<hertz>(1.0e-6),
            deposition_group: DepositionGroup::Halogen,
            released: vec![bq(5.0)],
        }],
    );
    let padded = pad_for_dispersion(&term, s(50.0));

    let bounds: Vec<f64> = padded
        .segment_boundaries()
        .iter()
        .map(|t| t.get::<second>())
        .collect();
    assert_eq!(bounds, vec![0.0, 100.0, 200.0, 250.0]);
    let released: Vec<f64> = padded.nuclides[0]
        .released
        .iter()
        .map(|a| a.get::<becquerel>())
        .collect();
    assert_eq!(released, vec![0.0, 5.0, 0.0]);
}

#[test]
fn the_chain_is_linear_in_the_inventory() {
    let (_, one) = run_chain(1.0);
    let (_, three) = run_chain(3.0);

    for r in 0..DISTANCES_M.len() {
        for (a, b) in one.at(r).iter().zip(three.at(r)) {
            let (air1, air3) = (
                a.air.becquerel_seconds_per_cubic_meter(),
                b.air.becquerel_seconds_per_cubic_meter(),
            );
            assert!(air1 > 0.0, "{} at {} m: air should be positive", a.label, DISTANCES_M[r]);
            let rel = (air3 - 3.0 * air1).abs() / (3.0 * air1);
            assert!(rel < 1e-12, "{} air not linear: rel {rel:e}", a.label);

            let (g1, g3) = (
                a.ground.becquerel_per_square_meter(),
                b.ground.becquerel_per_square_meter(),
            );
            if g1 > 0.0 {
                let rel = (g3 - 3.0 * g1).abs() / (3.0 * g1);
                assert!(rel < 1e-12, "{} ground not linear: rel {rel:e}", a.label);
            }
        }
    }
}

#[test]
fn noble_gases_deposit_exactly_zero_and_the_others_do_not() {
    let (term, result) = run_chain(1.0);
    for r in 0..DISTANCES_M.len() {
        for (n, t) in term.nuclides.iter().zip(result.at(r)) {
            let g = t.ground.becquerel_per_square_meter();
            if n.deposition_group == DepositionGroup::NobleGas {
                assert_eq!(g, 0.0, "{} is a noble gas and must not deposit", n.label);
            } else {
                assert!(g > 0.0, "{} at {} m should deposit", n.label, DISTANCES_M[r]);
            }
        }
    }
}
