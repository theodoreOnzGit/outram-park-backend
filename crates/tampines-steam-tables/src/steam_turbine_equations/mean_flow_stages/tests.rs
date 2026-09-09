//! Verification tests for the mean-flow stage model (bead `op-yi7m`).
//!
//! There is no published stage-by-stage reference case in this crate's test
//! data, so these tests anchor on the closed-form turbomachinery results the
//! model must reproduce, plus internal consistency of the two work mechanisms.
//!
//! A real stage is always partly impulse and partly reaction, and the machine
//! tests exercise it that way. The unit tests below deliberately isolate one
//! mechanism at a time, which the model supports: a rotor given no pressure
//! drop has no reaction part at all, and the impulse and reaction terms are
//! public so either can be evaluated on its own.

use approx::assert_relative_eq;
use uom::si::angle::{degree, radian};
use uom::si::angular_velocity::radian_per_second;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::pressure::bar;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;

use super::*;

fn unity() -> Ratio {
    Ratio::new::<ratio>(1.0)
}

fn zero_enthalpy_drop() -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(0.0)
}

/// A symmetric impulse blade turns the relative flow to the mirror of its
/// inlet angle, so `beta2 = -beta1`. Building the triangle twice is the only
/// way to get that angle, since `beta1` is an outcome of the inlet kinematics.
fn symmetric_impulse_triangle(
    blade_speed: Velocity,
    absolute_velocity_in: Velocity,
    nozzle_angle: Angle,
) -> VelocityTriangle {
    let probe = VelocityTriangle::new_from_nozzle_and_blade_angles(
        blade_speed,
        absolute_velocity_in,
        nozzle_angle,
        Angle::new::<radian>(0.0),
        unity(),
    );

    let relative_exit_angle = -probe.get_relative_angle_in();

    VelocityTriangle::new_from_nozzle_and_blade_angles(
        blade_speed,
        absolute_velocity_in,
        nozzle_angle,
        relative_exit_angle,
        unity(),
    )
}

fn reference_blading() -> RotorBlading {
    RotorBlading {
        blade_velocity_coefficient: Ratio::new::<ratio>(0.92),
        lift_coefficient: Ratio::new::<ratio>(1.1),
        drag_coefficient: Ratio::new::<ratio>(0.03),
        solidity: Ratio::new::<ratio>(1.4),
    }
}

/// The textbook symmetric impulse stage: `w = 2 U (c_theta1 - U)`.
///
/// This is the whole reason the impulse part goes through the triangle. If the
/// triangle is wired up correctly this identity is exact, not approximate.
#[test]
fn symmetric_impulse_matches_closed_form_euler_work() {
    let blade_speed = Velocity::new::<meter_per_second>(210.0);
    let absolute_velocity_in = Velocity::new::<meter_per_second>(447.0);
    let nozzle_angle = Angle::new::<degree>(70.0);

    let triangle = symmetric_impulse_triangle(blade_speed, absolute_velocity_in, nozzle_angle);

    let tangential_in = triangle.get_absolute_tangential_in();
    let expected: AvailableEnergy = (blade_speed * (tangential_in - blade_speed) * 2.0).into();

    assert_relative_eq!(
        triangle.euler_specific_work().get::<joule_per_kilogram>(),
        expected.get::<joule_per_kilogram>(),
        max_relative = 1e-12
    );
}

/// A symmetric impulse stage peaks at `U / c1 = sin(alpha1) / 2`.
///
/// The textbook writes this optimum as `cos(alpha1) / 2` because it measures
/// the nozzle angle from the **tangential** direction. This module measures
/// angles from **axial**, so the sine appears instead. The physics is the same:
/// the optimum is half the tangential component of the nozzle exit velocity.
///
/// Swept numerically rather than asserted from the formula, so the test would
/// catch a triangle that happens to satisfy the work identity at one point but
/// has the wrong shape across the range.
#[test]
fn impulse_optimum_blade_speed_ratio_is_half_sin_nozzle_angle() {
    let absolute_velocity_in = Velocity::new::<meter_per_second>(447.0);
    let nozzle_angle = Angle::new::<degree>(70.0);

    let mut best_ratio = 0.0_f64;
    let mut best_work = f64::NEG_INFINITY;

    for step in 1..=2000 {
        let speed_ratio = step as f64 / 2000.0;
        let blade_speed = absolute_velocity_in * speed_ratio;

        let triangle = symmetric_impulse_triangle(blade_speed, absolute_velocity_in, nozzle_angle);
        let work = triangle.euler_specific_work().get::<joule_per_kilogram>();

        if work > best_work {
            best_work = work;
            best_ratio = speed_ratio;
        }
    }

    let expected_ratio = nozzle_angle.sin().get::<ratio>() / 2.0;

    assert_relative_eq!(best_ratio, expected_ratio, max_relative = 2e-3);
}

/// A symmetric impulse stage has essentially zero degree of reaction.
#[test]
fn symmetric_impulse_reports_near_zero_degree_of_reaction() {
    let triangle = symmetric_impulse_triangle(
        Velocity::new::<meter_per_second>(210.0),
        Velocity::new::<meter_per_second>(447.0),
        Angle::new::<degree>(70.0),
    );

    assert_relative_eq!(
        triangle.get_degree_of_reaction().get::<ratio>(),
        0.0,
        epsilon = 1e-12
    );
}

/// A rotor that takes no pressure drop has no reaction part.
///
/// This is the limit that makes the two-part split honest: with `dh_rotor = 0`
/// the relative flow is turned but not accelerated, so the lift term has
/// nothing to act on and the stage reduces exactly to the classical impulse
/// stage.
#[test]
fn reaction_part_vanishes_without_a_rotor_pressure_drop() {
    let triangle = symmetric_impulse_triangle(
        Velocity::new::<meter_per_second>(210.0),
        Velocity::new::<meter_per_second>(447.0),
        Angle::new::<degree>(70.0),
    );

    let split = reference_blading().specific_work_split(&triangle, zero_enthalpy_drop());

    assert_relative_eq!(
        split.reaction.get::<joule_per_kilogram>(),
        0.0,
        epsilon = 1e-12
    );
    assert_relative_eq!(
        split.total().get::<joule_per_kilogram>(),
        triangle.euler_specific_work().get::<joule_per_kilogram>(),
        max_relative = 1e-12
    );
}

/// The reaction part grows with the rotor pressure drop, monotonically.
#[test]
fn reaction_part_grows_with_rotor_enthalpy_drop() {
    let triangle = VelocityTriangle::new_from_nozzle_and_blade_angles(
        Velocity::new::<meter_per_second>(180.0),
        Velocity::new::<meter_per_second>(420.0),
        Angle::new::<degree>(68.0),
        Angle::new::<degree>(-60.0),
        unity(),
    );

    let blading = reference_blading();
    let mut previous = f64::NEG_INFINITY;

    for kilojoules in [0.0_f64, 5.0, 10.0, 20.0, 40.0] {
        let drop = AvailableEnergy::new::<joule_per_kilogram>(kilojoules * 1000.0);
        let reaction = blading
            .reaction_specific_work(&triangle, drop)
            .get::<joule_per_kilogram>();

        assert!(
            reaction > previous,
            "reaction work must rise with rotor drop: {reaction} after {previous}"
        );
        previous = reaction;
    }
}

/// The drag contribution to tangential force carries the sign of the mean
/// relative flow angle, because drag acts along the mean relative velocity.
///
/// Both cases are asserted. A lightly turned rotor keeps `beta_m` positive and
/// drag adds; a strongly turned one takes the flow past axial, `beta_m` goes
/// negative, and drag subtracts. Getting this backwards was the first bug these
/// tests caught.
#[test]
fn cascade_drag_follows_the_sign_of_the_mean_relative_angle() {
    let lightly_turned = VelocityTriangle::new_from_nozzle_and_blade_angles(
        Velocity::new::<meter_per_second>(50.0),
        Velocity::new::<meter_per_second>(420.0),
        Angle::new::<degree>(68.0),
        Angle::new::<degree>(-30.0),
        unity(),
    );

    let strongly_turned = VelocityTriangle::new_from_nozzle_and_blade_angles(
        Velocity::new::<meter_per_second>(180.0),
        Velocity::new::<meter_per_second>(420.0),
        Angle::new::<degree>(68.0),
        Angle::new::<degree>(-60.0),
        unity(),
    );

    assert!(
        lightly_turned.get_mean_relative_angle() > Angle::new::<radian>(0.0),
        "the lightly turned case is meant to keep a positive mean relative angle"
    );
    assert!(
        strongly_turned.get_mean_relative_angle() < Angle::new::<radian>(0.0),
        "the strongly turned case is meant to push the mean relative angle negative"
    );

    let drop = AvailableEnergy::new::<joule_per_kilogram>(20_000.0);

    let loss_free = RotorBlading {
        drag_coefficient: Ratio::new::<ratio>(0.0),
        ..reference_blading()
    };
    let draggy = RotorBlading {
        drag_coefficient: Ratio::new::<ratio>(0.05),
        ..reference_blading()
    };

    assert!(
        draggy.reaction_specific_work(&lightly_turned, drop)
            > loss_free.reaction_specific_work(&lightly_turned, drop),
        "with beta_m positive, drag must add to the tangential force"
    );
    assert!(
        draggy.reaction_specific_work(&strongly_turned, drop)
            < loss_free.reaction_specific_work(&strongly_turned, drop),
        "with beta_m negative, drag must subtract from the tangential force"
    );
}

/// Builds a superheated admission state at 40 bar, 400 degC.
fn admission_steam() -> TampinesSteamTableCV {
    TampinesSteamTableCV::new_from_tp_quality_1(
        ThermodynamicTemperature::new::<degree_celsius>(400.0),
        Pressure::new::<bar>(40.0),
        Volume::new::<cubic_meter>(1.0),
    )
}

fn stage_geometry() -> StageGeometry {
    StageGeometry {
        mean_radius: Length::new::<meter>(0.52),
        nozzle_angle: Angle::new::<degree>(70.0),
        relative_exit_angle: Angle::new::<degree>(-54.0),
        nozzle_velocity_coefficient: Ratio::new::<ratio>(0.96),
    }
}

/// An eight-stage machine expanding 40 bar / 400 degC down to 0.35 bar.
///
/// The exit pressures are a geometric series, so each stage takes a similar
/// isentropic drop and therefore a similar nozzle exit velocity. That is what
/// lets one shaft speed and one mean radius sit near the optimum blade-speed
/// ratio for every stage, and it is why real machines stage an expansion
/// instead of taking it in one drop.
///
/// Each stator exit pressure is the geometric mean of the stage's inlet and
/// exit pressures, so every rotor takes a genuine pressure drop and every stage
/// is a mix of impulse and reaction.
fn mixed_stages() -> Vec<TurbineStage> {
    let exit_pressures = [22.12_f64, 12.23, 6.76, 3.74, 2.07, 1.14, 0.633, 0.35];

    let mut inlet_pressure = 40.0_f64;
    let mut stages = Vec::with_capacity(exit_pressures.len());

    for exit_pressure in exit_pressures {
        let stator_exit = (inlet_pressure * exit_pressure).sqrt();

        stages.push(TurbineStage {
            geometry: stage_geometry(),
            blading: reference_blading(),
            stator_exit_pressure: Pressure::new::<bar>(stator_exit),
            rotor_exit_pressure: Pressure::new::<bar>(exit_pressure),
        });

        inlet_pressure = exit_pressure;
    }

    stages
}

fn shaft_speed() -> AngularVelocity {
    AngularVelocity::new::<radian_per_second>(420.0)
}

/// One mixed stage through the steam tables. Both mechanisms must contribute,
/// the work must be positive, and the control volume must have lost exactly
/// the work that was taken out.
#[test]
fn single_mixed_stage_conserves_energy_and_uses_both_mechanisms() {
    let inlet = admission_steam();

    let stage = TurbineStage {
        geometry: stage_geometry(),
        blading: reference_blading(),
        stator_exit_pressure: Pressure::new::<bar>(29.7),
        rotor_exit_pressure: Pressure::new::<bar>(22.12),
    };

    let outcome = stage.expand(inlet, shaft_speed());

    let impulse = outcome.work_split.impulse.get::<joule_per_kilogram>();
    let reaction = outcome.work_split.reaction.get::<joule_per_kilogram>();

    assert!(
        impulse > 0.0,
        "the impulse part must do work, got {impulse}"
    );
    assert!(
        reaction > 0.0,
        "the reaction part must do work, got {reaction}"
    );

    let work = outcome.specific_work().get::<joule_per_kilogram>();
    let enthalpy_drop = (inlet.get_specific_enthalpy() - outcome.outlet.get_specific_enthalpy())
        .get::<joule_per_kilogram>();

    assert_relative_eq!(enthalpy_drop, work, max_relative = 1e-9);

    let reaction_fraction = outcome.work_split.reaction_fraction().get::<ratio>();
    assert!(
        reaction_fraction > 0.0 && reaction_fraction < 1.0,
        "a mixed stage must sit strictly between the two extremes, got {reaction_fraction}"
    );
}

/// A rotor with no pressure drop is a pure impulse stage, and its reaction
/// fraction must be exactly zero even though the blading carries lift
/// parameters.
#[test]
fn impulse_only_stage_reports_zero_reaction_fraction() {
    let inlet = admission_steam();

    let stage = TurbineStage {
        geometry: stage_geometry(),
        blading: reference_blading(),
        stator_exit_pressure: Pressure::new::<bar>(22.12),
        rotor_exit_pressure: Pressure::new::<bar>(22.12),
    };

    let outcome = stage.expand(inlet, shaft_speed());

    assert_relative_eq!(
        outcome.work_split.reaction.get::<joule_per_kilogram>(),
        0.0,
        epsilon = 1e-12
    );
    assert_relative_eq!(
        outcome.work_split.reaction_fraction().get::<ratio>(),
        0.0,
        epsilon = 1e-12
    );
}

/// The whole machine must conserve energy across the control-volume chain.
#[test]
fn multistage_expansion_conserves_energy() {
    let inlet = admission_steam();

    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(inlet);

    assert_eq!(outcome.stage_outcomes.len(), 8);

    let total_work = outcome.total_specific_work().get::<joule_per_kilogram>();
    let enthalpy_drop = (inlet.get_specific_enthalpy() - outcome.outlet.get_specific_enthalpy())
        .get::<joule_per_kilogram>();

    assert_relative_eq!(enthalpy_drop, total_work, max_relative = 1e-9);
}

/// The exhaust must come out wet because the expansion crossed the saturation
/// line, not because anyone imposed a quality.
///
/// This is what the equilibrium control volume buys: the moisture appears on
/// its own, part-way down the machine, at the stage where the `(p,h)` flash
/// first lands inside the dome.
#[test]
fn multistage_exhaust_is_wet_and_moisture_appears_part_way_down() {
    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(admission_steam());

    let exhaust_quality = outcome.outlet.get_quality();
    assert!(
        exhaust_quality < 1.0 && exhaust_quality > 0.85,
        "the last stages should land inside the dome, got quality {exhaust_quality}"
    );

    let first_wet_stage = outcome
        .stage_outcomes
        .iter()
        .position(|stage_outcome| stage_outcome.outlet.get_quality() < 1.0)
        .expect("some stage must go wet");

    assert!(
        first_wet_stage > 0,
        "admission steam is superheated, so the first stage must not already be wet"
    );
    assert!(
        first_wet_stage < outcome.stage_outcomes.len() - 1,
        "moisture should appear before the final stage, not only at the exhaust"
    );
}

/// Every stage of the machine must be a genuine mix, not silently one or the
/// other. This is the property the model exists to have.
#[test]
fn every_stage_of_the_machine_mixes_both_mechanisms() {
    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(admission_steam());

    for (index, stage_outcome) in outcome.stage_outcomes.iter().enumerate() {
        let fraction = stage_outcome.work_split.reaction_fraction().get::<ratio>();

        assert!(
            fraction > 0.0 && fraction < 1.0,
            "stage {} is not a mix, reaction fraction {fraction}",
            index + 1
        );
    }
}

/// The sum of per-stage isentropic drops exceeds the isentropic drop of the
/// machine taken in one step. That difference is the reheat effect, and a
/// stage-resolved model is the only kind that can show it.
#[test]
fn per_stage_isentropic_drops_exceed_the_single_step_drop() {
    let inlet = admission_steam();

    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(inlet);

    let mut single_step = inlet;
    single_step.expand_isentropically(outcome.outlet.get_pressure());
    let single_step_drop = (inlet.get_specific_enthalpy() - single_step.get_specific_enthalpy())
        .get::<joule_per_kilogram>();

    let summed_drop = outcome
        .total_isentropic_specific_work()
        .get::<joule_per_kilogram>();

    assert!(
        summed_drop > single_step_drop,
        "reheat means the staged isentropic drops must sum above the single-step \
         drop: {summed_drop} vs {single_step_drop}"
    );
}

/// Pressure must fall through the machine, stage by stage.
#[test]
fn multistage_pressure_descends_monotonically() {
    let inlet = admission_steam();

    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(inlet);

    let mut previous = inlet.get_pressure();
    for stage_outcome in outcome.stage_outcomes.iter() {
        let current = stage_outcome.outlet.get_pressure();
        assert!(
            current < previous,
            "pressure must fall across every stage: {current:?} after {previous:?}"
        );
        previous = current;
    }
}

/// A stage handed a rising pressure produces no nozzle velocity and therefore
/// no work, rather than a NaN that would poison the rest of the chain.
#[test]
fn rising_pressure_stage_degrades_to_zero_work() {
    let inlet = admission_steam();

    let stage = TurbineStage {
        geometry: stage_geometry(),
        blading: reference_blading(),
        stator_exit_pressure: Pressure::new::<bar>(45.0),
        rotor_exit_pressure: Pressure::new::<bar>(45.0),
    };

    let outcome = stage.expand(inlet, shaft_speed());
    let work = outcome.specific_work().get::<joule_per_kilogram>();

    assert!(work.is_finite(), "a rising pressure must not produce NaN");
    assert!(
        work <= 0.0,
        "no nozzle velocity means no positive work, got {work}"
    );
}

/// Diagnostic — prints the stage-by-stage expansion so a reader can see where
/// the work, the impulse/reaction split and the moisture actually appear.
/// Asserts nothing, and is `#[ignore]`d by design, following this crate's
/// existing diagnostic pattern.
///
/// Run with:
/// `cargo test --release -p tampines-steam-tables --lib diagnose_multistage -- --ignored --nocapture`
#[test]
#[ignore = "diagnostic — prints a per-stage table, asserts nothing"]
fn diagnose_multistage_expansion() {
    let inlet = admission_steam();

    let turbine = MeanFlowTurbine::new(mixed_stages(), shaft_speed());
    let outcome = turbine.expand(inlet);

    eprintln!(
        "inlet: p = {:.3} bar, h = {:.1} kJ/kg, x = {:.4}",
        inlet.get_pressure().get::<bar>(),
        inlet.get_specific_enthalpy().get::<joule_per_kilogram>() / 1000.0,
        inlet.get_quality()
    );

    for (index, stage_outcome) in outcome.stage_outcomes.iter().enumerate() {
        eprintln!(
            "stage {:>2}: p = {:>8.4} bar  w_imp = {:>7.2}  w_rea = {:>7.2}  \
             w_s = {:>7.2} kJ/kg  eta = {:>6.3}  R_w = {:>5.3}  x = {:.4}",
            index + 1,
            stage_outcome.outlet.get_pressure().get::<bar>(),
            stage_outcome.work_split.impulse.get::<joule_per_kilogram>() / 1000.0,
            stage_outcome
                .work_split
                .reaction
                .get::<joule_per_kilogram>()
                / 1000.0,
            stage_outcome
                .isentropic_specific_work
                .get::<joule_per_kilogram>()
                / 1000.0,
            stage_outcome.stage_efficiency().get::<ratio>(),
            stage_outcome.work_split.reaction_fraction().get::<ratio>(),
            stage_outcome.outlet.get_quality(),
        );
    }

    eprintln!(
        "total: w = {:.2} kJ/kg, exhaust x = {:.4}",
        outcome.total_specific_work().get::<joule_per_kilogram>() / 1000.0,
        outcome.outlet.get_quality()
    );
}
