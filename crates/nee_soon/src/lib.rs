//! # NEE_SOON
//!
//! **N**eutron **E**nergy-dependent **S**imulation using **O**pen-source
//! **O**bject-**O**riented **N**umerics.
//!
//! NEE_SOON is the **coupling / integration layer** of the OUTRAM PARK suite.
//! It does not implement transport, nuclear-data processing, or kinetics
//! itself — those live in dedicated crates. Instead it composes them behind a
//! single, human-navigable object-oriented API so that a user can assemble the
//! simulation pieces they want without wiring the crates together by hand.
//!
//! ## What it composes
//!
//! | Piece | Provided by | Role |
//! |---|---|---|
//! | Nuclear data / cross sections | [`njoy_outram_park_fork`] | energy-dependent σ(E), ν̄, χ, WMP |
//! | Monte Carlo transport | [`outram_mc_libs`] | CSG geometry, k-eigenvalue, Woodcock tracking |
//! | Point reactor kinetics | [`teh_o_prke`] | PRKE precursor/reactivity time response |
//! | Prompt excursion (Nordheim-Fuchs) | [`teh_o_prke::nordheim_fuchs`] | real-time-friendly closed-form prompt excursion + adiabatic fuel feedback, the "Prompt Excursion Layer" beneath full PRKE |
//! | GeN-Foam SP3 multiphysics | [`outram_foam_appbuilder_lib::genfoam`] | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang workflow) |
//!
//! ## Worked coupling: the Xin Wang SP3 workflow
//!
//! [`xin_wang_sp3_workflow`] is a **scaffold** of the four-stage
//! njoy → openmc → genfoam pipeline that reproduces Figure 4.29 (Mk1 PB-FHR
//! control-rod-removal transient) of Xin Wang's 2018 UC Berkeley PhD
//! dissertation. Each stage is a documented, beaded placeholder; the extracted
//! thesis methodology and case data live in the crate's `docs/xin-wang-thesis/`.
//!
//! ## Entry point
//!
//! The whole crate is reached through **one struct**, [`NeeSoon`]. It is the
//! object-oriented facade: the user constructs a `NeeSoon`, then asks it to
//! create the relevant simulation pieces (a data provider, a transport model, a
//! kinetics model, a coupled run) rather than importing each underlying crate
//! directly. This keeps the mental context load low — one type to learn, with
//! `rust-analyzer` autocompletion revealing the available pieces.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** orchestration, the object-oriented facade, cross-crate
//!   glue types, ergonomic constructors, coupling schedules, and any *new*
//!   user-facing functionality that only makes sense once the pieces are joined.
//! - **Does NOT belong here:** raw physics kernels. New cross-section code goes
//!   to `njoy-outram-park-fork`; new transport code to `outram-mc-libs`; new
//!   kinetics to `teh-o-prke`. NEE_SOON only *exposes and integrates* them.
//!
//! ## Status
//!
//! ~~**Mostly scaffold.**~~ **CORRECTED 2026-09-19.** The claim that "the
//! nuclear-data and Monte Carlo integration points are not wired yet" is no
//! longer true, and is struck rather than deleted because it shaped how this
//! crate was described for months.
//!
//! What is real and tested today:
//!
//! | Module | What it does |
//! |---|---|
//! | [`htr10_rmc`] | one shared HTR-10 geometry and composition, so the stochastic and deterministic ends cannot drift apart |
//! | [`mgxs`] | condenses a Monte Carlo run into multigroup constants: flux-weighted reaction rates, a nu-scatter matrix, and a **measured** fission spectrum |
//! | [`genfoam_xs`] | hands those constants to GeN-Foam through its own `nuclearData` input path |
//! | [`coupling`] | [`coupling::McToGenFoam`], the facade: construct, `generate_mgxs()`, `solve_infinite_medium()` |
//! | [`direct_coupling`] | [`direct_coupling::McGenFoamDirect`], Monte Carlo iterated directly against GeN-Foam's lumped thermal region |
//!
//! [`NeeSoon::new_prompt_excursion_model`] remains real, wired code exposing
//! `teh-o-prke`'s Nordheim-Fuchs exact timestepper.
//!
//! Measured 2026-09-19: on a leakage-free medium GeN-Foam reproduces the Monte
//! Carlo eigenvalue to **+328 pcm, 1.7 sigma**; the HTR-10 core condenses to
//! four zones GeN-Foam accepts and solves; the direct loop converges in 8 outer
//! iterations over a +248 K temperature swing.
//!
//! **What is still NOT here:** delayed-neutron data, so everything above is
//! **steady state only** and a transient would run without delayed neutrons;
//! `P0` scattering only, so no SP3 `P1` and an untransport-corrected diffusion
//! coefficient; one state point per run, so no feedback parametrisation; and
//! **no validation of any kind** -- no experiment and no published benchmark.
//! The [`xin_wang_sp3_workflow`] scaffold is still a scaffold.

#![forbid(unsafe_code)]

pub mod xin_wang_sp3_workflow;

pub use teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper;
pub use teh_o_prke::teh_o_prke_error::TehOPrkeError;

use uom::si::f64::{HeatCapacity, Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time};

pub mod htr10_rmc;
pub mod mgxs;
pub mod genfoam_xs;
pub mod det_six_factor;
pub mod rod_insertion;
pub mod coupling;
pub mod direct_coupling;

/// Object-oriented facade for the OUTRAM PARK neutronics + kinetics suite.
///
/// `NeeSoon` is the single entry point of the crate (the "one big struct"): a
/// user constructs one of these and then creates the relevant simulation pieces
/// through it — a nuclear-data provider ([`njoy_outram_park_fork`]), a Monte
/// Carlo transport model ([`outram_mc_libs`]), a point-kinetics model
/// ([`teh_o_prke`]), and, ultimately, coupled runs that thread data between
/// them.
///
/// # Physical scope
///
/// This type owns no physics of its own; it is a builder/orchestrator over the
/// composed crates. Physical quantities exchanged across its API are dimensioned
/// via [`uom`] (never bare `f64`).
///
/// # Status
///
/// [`Self::new_prompt_excursion_model`] is wired to `teh-o-prke`'s
/// [`NordheimFuchsExactTimestepper`]. Nuclear-data-provider and Monte
/// Carlo transport / coupled-run construction are not implemented yet.
/// The planned shape is a builder that holds:
/// - a nuclear-data provider handle (cross-section source),
/// - an optional transport model,
/// - an optional kinetics model,
/// - coupling / orchestration configuration.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct NeeSoon {}

impl NeeSoon {
    /// Creates a Nordheim-Fuchs exact-timestepper prompt-excursion model
    /// (`teh-o-prke`'s [`NordheimFuchsExactTimestepper`]) -- the "Prompt
    /// Excursion Layer" of the recommended Outram Park architecture: a
    /// real-time-friendly, closed-form model of a prompt reactivity
    /// excursion with adiabatic fuel-temperature feedback, distinct from
    /// (and much cheaper than) full point reactor kinetics. See that
    /// type's doc comment for the governing equations, preconditions
    /// (`alpha_f < 0`, `Lambda > 0`, `C_f > 0`), and limitations.
    ///
    /// This is a thin pass-through -- `NeeSoon` does not reimplement or
    /// wrap the physics, it only exposes `teh-o-prke`'s constructor
    /// through the crate's single-facade entry point.
    #[allow(clippy::too_many_arguments)] // mirrors NordheimFuchsExactTimestepper::new exactly
    pub fn new_prompt_excursion_model(
        &self,
        prompt_neutron_generation_time: Time,
        delayed_neutron_fraction: Ratio,
        fuel_heat_capacity: HeatCapacity,
        fuel_feedback_coefficient: TemperatureCoefficient,
        fuel_reference_temperature: ThermodynamicTemperature,
        initial_fuel_temperature: ThermodynamicTemperature,
        initial_power: Power,
    ) -> Result<NordheimFuchsExactTimestepper, TehOPrkeError> {
        NordheimFuchsExactTimestepper::new(
            prompt_neutron_generation_time,
            delayed_neutron_fraction,
            fuel_heat_capacity,
            fuel_feedback_coefficient,
            fuel_reference_temperature,
            initial_fuel_temperature,
            initial_power,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NeeSoon`'s facade constructor should behave identically to
    /// calling `teh-o-prke` directly -- confirms the wiring (and the
    /// `chem-eng-real-time-process-control-simulator` -> `teh-o-prke` ->
    /// `nee_soon` dependency chain) actually compiles and runs, not just
    /// that the types line up on paper.
    #[test]
    fn prompt_excursion_model_matches_direct_teh_o_prke_construction() {
        use uom::si::heat_capacity::joule_per_kelvin;
        use uom::si::power::watt;
        use uom::si::ratio::ratio;
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;
        use uom::si::time::second;

        let nee_soon = NeeSoon::default();
        let mut via_facade = nee_soon
            .new_prompt_excursion_model(
                Time::new::<second>(1.0e-5),
                Ratio::new::<ratio>(0.007),
                HeatCapacity::new::<joule_per_kelvin>(1.0e5),
                TemperatureCoefficient::new::<per_kelvin>(-1.0e-5),
                ThermodynamicTemperature::new::<kelvin>(900.0),
                ThermodynamicTemperature::new::<kelvin>(900.0),
                Power::new::<watt>(1.0),
            )
            .unwrap();

        let mut direct = NordheimFuchsExactTimestepper::default();

        via_facade.set_external_reactivity(Ratio::new::<ratio>(1.5 * 0.007));
        direct.set_external_reactivity(Ratio::new::<ratio>(1.5 * 0.007));

        let dt = Time::new::<second>(1.0e-3);
        for _ in 0..100 {
            via_facade.step(dt);
            direct.step(dt);
        }

        assert_eq!(via_facade.power.get::<watt>(), direct.power.get::<watt>());
    }
}
