//! Tests for the transient mean-flow turbine (bead `op-yi7m`).
//!
//! These check the coupling between the stage model and the 1-D HEM KNP hybrid
//! solver, not the solver itself, which has its own validation under
//! `openfoam_algorithms/`. What matters here is that the triangle is built on
//! the solved flow, that the work leaves through the energy equation, and that
//! the limits behave.
//!
//! They are deliberately short. The array's own Edwards blowdown case runs for
//! minutes; nothing here needs more than a handful of steps.

use approx::assert_relative_eq;
use uom::si::angle::degree;
use uom::si::angular_velocity::radian_per_second;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::pressure::{bar, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use crate::openfoam_algorithms::rhoPimpleFoam::SolverMode;

use super::*;

fn stage_geometry() -> StageGeometry {
    StageGeometry {
        mean_radius: Length::new::<meter>(0.52),
        nozzle_angle: Angle::new::<degree>(70.0),
        relative_exit_angle: Angle::new::<degree>(-54.0),
        nozzle_velocity_coefficient: Ratio::new::<ratio>(0.96),
    }
}

fn reference_blading() -> RotorBlading {
    RotorBlading {
        blade_velocity_coefficient: Ratio::new::<ratio>(0.92),
        lift_coefficient: Ratio::new::<ratio>(1.1),
        drag_coefficient: Ratio::new::<ratio>(0.03),
        solidity: Ratio::new::<ratio>(1.4),
    }
}

fn transient_stages(rotor_drop_fraction: f64) -> Vec<TransientStage> {
    (0..4)
        .map(|_| TransientStage {
            geometry: stage_geometry(),
            blading: reference_blading(),
            rotor_drop_fraction: Ratio::new::<ratio>(rotor_drop_fraction),
        })
        .collect()
}

fn build_machine(rotor_drop_fraction: f64) -> TransientMeanFlowTurbine {
    let machine = TransientMeanFlowTurbine::new(
        transient_stages(rotor_drop_fraction),
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.1),
        Time::new::<second>(1.0e-5),
        AngularVelocity::new::<radian_per_second>(420.0),
    )
    .expect("a four-cell 1-D mesh must build");

    let mut machine = machine;
    machine.initialise_uniform(
        Pressure::new::<bar>(40.0),
        ThermodynamicTemperature::new::<degree_celsius>(400.0),
    );

    machine
}

/// Imposes a descending pressure profile so the stages see a real local drop,
/// which is what gives them a reaction part.
fn impose_descending_pressure(machine: &mut TransientMeanFlowTurbine) {
    let profile_bar = [40.0_f64, 30.0, 22.0, 16.0];

    for (cell, pressure_bar) in profile_bar.iter().enumerate() {
        machine.array.p.internal[cell] = Pressure::new::<bar>(*pressure_bar).get::<pascal>();
    }

    machine.array.correct_thermo();
}

/// The constructor must select the KNP hybrid, not the array's own PIMPLE
/// default. Turbine passages run near-sonic, which is the regime the plain
/// pressure-based path rings in.
#[test]
fn constructor_selects_the_knp_hybrid_solver_mode() {
    let machine = build_machine(0.5);

    assert_eq!(machine.array.get_solver_mode(), SolverMode::HybridAllMach);
    assert_eq!(machine.array.mesh.n_cells, 4, "one cell per stage");
}

/// The array's own initial condition is liquid water at 1 bar and 300 K, so the
/// machine has to be seeded before it means anything.
#[test]
fn initialise_uniform_puts_superheated_steam_in_every_cell() {
    let machine = build_machine(0.5);

    for cell in 0..machine.array.mesh.n_cells {
        let control_volume = machine.stage_control_volume(cell);

        assert_relative_eq!(
            control_volume.get_pressure().get::<bar>(),
            40.0,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            control_volume.get_temperature().get::<degree_celsius>(),
            400.0,
            max_relative = 1e-3
        );
        assert_eq!(
            control_volume.get_quality(),
            1.0,
            "40 bar / 400 degC is superheated"
        );
    }
}

/// With no through-flow there is no work, whatever the blading says.
#[test]
fn still_steam_extracts_no_work() {
    let mut machine = build_machine(0.5);

    let outcome = machine.step();

    for stage_outcome in outcome.stage_outcomes.iter() {
        assert_relative_eq!(stage_outcome.shaft_power.get::<watt>(), 0.0, epsilon = 1e-9);
    }
    assert_relative_eq!(
        outcome.total_shaft_power().get::<watt>(),
        0.0,
        epsilon = 1e-9
    );
}

/// With flow and a local pressure drop, both mechanisms contribute and the
/// machine extracts power.
#[test]
fn moving_steam_extracts_work_from_both_mechanisms() {
    let mut machine = build_machine(0.5);
    impose_descending_pressure(&mut machine);
    machine
        .array
        .set_uniform_velocity_field(Velocity::new::<meter_per_second>(120.0));

    let outcome = machine.step();

    for (index, stage_outcome) in outcome.stage_outcomes.iter().enumerate() {
        let impulse = stage_outcome
            .work_split
            .impulse
            .get::<uom::si::available_energy::joule_per_kilogram>();
        let reaction = stage_outcome
            .work_split
            .reaction
            .get::<uom::si::available_energy::joule_per_kilogram>();

        assert!(
            impulse > 0.0,
            "stage {} took no impulse work: {impulse}",
            index + 1
        );
        assert!(
            reaction > 0.0,
            "stage {} took no reaction work: {reaction}",
            index + 1
        );
        assert!(
            stage_outcome
                .mass_flow
                .get::<uom::si::mass_rate::kilogram_per_second>()
                > 0.0,
            "stage {} has no mass flow",
            index + 1
        );
    }

    assert!(
        outcome.total_shaft_power().get::<watt>() > 0.0,
        "a running machine must extract power"
    );
}

/// A rotor given none of the pressure drop is a pure impulse stage, even though
/// the blading still carries lift parameters. This is the same limit the steady
/// model asserts, reached through the solver instead.
#[test]
fn zero_rotor_drop_fraction_gives_a_pure_impulse_stage() {
    let mut machine = build_machine(0.0);
    impose_descending_pressure(&mut machine);
    machine
        .array
        .set_uniform_velocity_field(Velocity::new::<meter_per_second>(120.0));

    let outcome = machine.step();

    for stage_outcome in outcome.stage_outcomes.iter() {
        assert_relative_eq!(
            stage_outcome
                .work_split
                .reaction
                .get::<uom::si::available_energy::joule_per_kilogram>(),
            0.0,
            epsilon = 1e-12
        );
        assert!(
            stage_outcome
                .work_split
                .impulse
                .get::<uom::si::available_energy::joule_per_kilogram>()
                > 0.0,
            "the impulse part must still be doing the work"
        );
    }
}

/// Power sources are per-timestep registrations. The step must consume them, or
/// the next step would double count every stage.
#[test]
fn power_sources_are_consumed_by_the_step() {
    let mut machine = build_machine(0.5);
    impose_descending_pressure(&mut machine);
    machine
        .array
        .set_uniform_velocity_field(Velocity::new::<meter_per_second>(120.0));

    machine.step();

    assert!(
        machine.array.q_vector.is_empty(),
        "the array clears registered sources once per step"
    );
    assert!(machine.array.q_fraction_vector.is_empty());
}

/// The reported total is the sum of the stages, not an independently computed
/// number that could drift from them.
#[test]
fn total_shaft_power_is_the_sum_of_the_stages() {
    let mut machine = build_machine(0.5);
    impose_descending_pressure(&mut machine);
    machine
        .array
        .set_uniform_velocity_field(Velocity::new::<meter_per_second>(120.0));

    let outcome = machine.step();

    let summed: f64 = outcome
        .stage_outcomes
        .iter()
        .map(|stage_outcome| stage_outcome.shaft_power.get::<watt>())
        .sum();

    assert_relative_eq!(
        outcome.total_shaft_power().get::<watt>(),
        summed,
        max_relative = 1e-12
    );
}

/// A short run must stay bounded: no NaN, and every cell inside the solver's
/// own pressure bounds.
#[test]
fn short_run_stays_bounded() {
    let mut machine = build_machine(0.5);
    impose_descending_pressure(&mut machine);
    machine
        .array
        .set_uniform_velocity_field(Velocity::new::<meter_per_second>(120.0));

    machine.run(20);

    let (p_min, p_max) = machine.array.get_pressure_bounds();

    for cell in 0..machine.array.mesh.n_cells {
        let pressure = machine.array.p.internal[cell];
        let specific_enthalpy = machine.array.he.internal[cell];

        assert!(
            pressure.is_finite() && specific_enthalpy.is_finite(),
            "cell {cell} went non-finite: p = {pressure}, he = {specific_enthalpy}"
        );
        assert!(
            pressure >= p_min.get::<pascal>() && pressure <= p_max.get::<pascal>(),
            "cell {cell} left the solver's pressure bounds at {pressure} Pa"
        );
    }
}
