//! Transient homogeneous-equilibrium channel model — one implicit-Euler step.
//!
//! # Provenance
//!
//! Translated from `singleflow1devaptime.m` by **Than Yan Ren** (Singapore
//! Nuclear Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation. IAPWS calls
//! go through [`super::steam`]; see `docs/bedok-port-scoping.md` §3.
//!
//! # The scheme
//!
//! One implicit-Euler step of the 1-D single-pressure coolant energy equation
//! per channel,
//!
//! ```text
//! rho*A dh/dt + W dh/dz = q'_wall
//! ```
//!
//! marched **on the cell faces** with the cell-centred enthalpy taken as the
//! average of its two faces:
//!
//! ```text
//! W*(hf_i - hf_{i-1}) + cap_i*(hc_i - hc_i_old) = q_i
//! cap_i = rho_old * A * Lz / dt          [g/s]
//! hc_i  = (hf_{i-1} + hf_i) / 2          [kJ/kg]
//! ```
//!
//! solved node by node for `hf_i`. As `dt -> inf` the capacity term vanishes
//! and the scheme reduces exactly to the steady half-node march of
//! [`super::single_flow_evap`], which is what makes a transient consistent
//! with its own `t = 0` steady state.
//!
//! Stage 2 — inverting the mixture enthalpy into void fraction, temperature
//! and quality — is *identical* to the steady model (the MATLAB says so at its
//! line 94) and is shared with it, see
//! [`super::single_flow_evap::invert_mixture_enthalpy`].
//!
//! # Assumptions
//!
//! Mass flow rate and channel pressure are **held constant** through the
//! transient. The MATLAB notes this is right for the NEACRP PWR cases
//! (constant inlet flow, constant 155 bar core pressure) and says nothing
//! about cases where it is not.

use super::single_flow_evap::{check_length, expand_flow_rate, invert_mixture_enthalpy};
use super::steam;
use super::{FlowDirection, ThGeometry, ThResult, ThermalHydraulicParams, ThermalHydraulicState};

/// Advance every channel's coolant enthalpy through one time step.
///
/// MATLAB `singleflow1devaptime(params, geometry, th, pwrdens, thold, dt)`.
///
/// # Arguments
///
/// - `params` — grid and void-closure knobs.
/// - `geometry` — axial node heights \[cm\] and per-channel axial extents.
/// - `th` — current thermal-hydraulic iterate, updated in place.
///   `th.heat_flux` \[W/cm²\] must carry the wall heat flux of the previous
///   step or Picard pass, and `th.power_ratio` the current relative core power.
/// - `power_density` — \[-\] L1-normalised, group-collapsed nodal power,
///   `grid.nodes()` long.
/// - `previous_step` — the **converged** state of the previous time step. Only
///   `previous_step.coolant.enthalpy` \[kJ/kg\] and
///   `previous_step.coolant.density` \[g/cm³\] are read, for the capacity
///   terms.
/// - `time_step` — \[s\] the step size `dt`.
///
/// # Errors
///
/// [`super::ThError::LengthMismatch`] if any input vector is not
/// `grid.nodes()` long.
///
/// # Panics
///
/// If `time_step` is not strictly positive — the capacity terms divide by it.
pub fn solve_transient(
    params: &ThermalHydraulicParams,
    geometry: &ThGeometry,
    th: &mut ThermalHydraulicState,
    power_density: &[f64],
    previous_step: &ThermalHydraulicState,
    time_step: f64,
) -> ThResult<()> {
    assert!(
        time_step > 0.0,
        "coolant time step must be strictly positive, got {time_step}"
    );

    let grid = params.grid;
    let nodes = grid.nodes();

    check_length("singleflow1devaptime pwrdens", nodes, power_density.len())?;
    check_length(
        "singleflow1devaptime geometry.Lz",
        nodes,
        geometry.axial_height.len(),
    )?;
    check_length(
        "singleflow1devaptime th.heatflux",
        nodes,
        th.heat_flux.len(),
    )?;
    check_length(
        "singleflow1devaptime thold.coolant.enth",
        nodes,
        previous_step.coolant.enthalpy.len(),
    )?;
    check_length(
        "singleflow1devaptime thold.coolant.dens",
        nodes,
        previous_step.coolant.density.len(),
    )?;

    let max_power = th.max_power;
    let power_ratio = th.power_ratio;
    let n_pins = th.n_fuel_pins;
    let coolant_fraction = th.coolant_heat_fraction;
    let inlet_temperature = th.coolant.inlet_temperature;
    let flow_direction = th.flow_direction;
    let flow_rate = expand_flow_rate(&th.flow_rate, nodes)?;

    let axial_height = &geometry.axial_height;
    let outer_radius = geometry.fuel.outer_radius;
    let subchannel_area = geometry.fuel.subchannel_area;
    let channel_pressure = th.coolant.inlet_pressure;

    // ---------- (1) implicit upwind enthalpy march ----------
    let inlet_enthalpy = steam::enthalpy_region1_pt(channel_pressure, inlet_temperature);
    let mut enthalpy = vec![inlet_enthalpy; nodes];
    let mut enthalpy_face = vec![inlet_enthalpy; nodes];

    let old_enthalpy = &previous_step.coolant.enthalpy;
    let old_density = &previous_step.coolant.density;

    // Nodal heat rate [W], channel mass flow [g/s] and capacity [g/s].
    let mut heat_rate = vec![0.0; nodes];
    let mut mass_flow = vec![0.0; nodes];
    let mut capacity = vec![0.0; nodes];
    for i in 0..nodes {
        let linear_power = power_density[i] * max_power * power_ratio / axial_height[i];
        let coolant_linear_power =
            2.0 * std::f64::consts::PI * outer_radius * th.heat_flux[i] * n_pins
                + coolant_fraction * linear_power;
        heat_rate[i] = coolant_linear_power * axial_height[i];
        mass_flow[i] = flow_rate[i] * subchannel_area * n_pins;
        capacity[i] = old_density[i] * subchannel_area * n_pins * axial_height[i] / time_step;
    }

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let channel = ThGeometry::channel_index(&grid, ix, iy);
            let z_low = geometry.z_low[channel];
            let z_high = geometry.z_high[channel];

            if !(0..grid.nz).any(|iz| power_density[grid.index(0, ix, iy, iz)] != 0.0) {
                continue;
            }

            let mut face_upstream = inlet_enthalpy;
            let axial: Vec<usize> = match flow_direction {
                FlowDirection::Downward => (z_low..=z_high).rev().collect(),
                FlowDirection::Upward => (z_low..=z_high).collect(),
            };
            for iz in axial {
                let i = grid.index(0, ix, iy, iz);
                let face = (heat_rate[i] + mass_flow[i] * face_upstream
                    - capacity[i] * (face_upstream / 2.0 - old_enthalpy[i]))
                    / (mass_flow[i] + capacity[i] / 2.0);
                enthalpy[i] = 0.5 * (face_upstream + face);
                enthalpy_face[i] = face;
                face_upstream = face;
            }
        }
    }

    // ---------- (2) invert mixture enthalpy (identical to the steady model) ----------
    invert_mixture_enthalpy(params, th, enthalpy, &flow_rate, channel_pressure);
    th.coolant.enthalpy_face = enthalpy_face;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::single_flow_evap::{self, test_support::single_channel_rig};
    use super::*;

    /// A long step reproduces the steady half-node march.
    ///
    /// **Methodology.** The scheme is built so that as `dt -> inf` the capacity
    /// terms vanish and the face march collapses to the steady half-node
    /// march. Running both on the same single channel with `dt = 1e12 s` must
    /// therefore reproduce the steady cell-centred enthalpies. Inputs: one
    /// channel, 18 axial nodes, 15.5 MPa, 559.15 K inlet, flat power, no wall
    /// heat flux. Pass criterion: relative difference below 1e-9 at every node.
    ///
    /// **Result (2026-08-05).** Relative difference below 1e-12 at every node.
    /// Interpretation: the transient and steady coolant models agree in the
    /// steady limit, which is the property `th_solvertimexyz` relies on to
    /// avoid a spurious reactivity step at `t = 0`.
    #[test]
    fn a_long_step_reproduces_the_steady_march() {
        let (grid, params, geometry, mut steady, power_density) = single_channel_rig(18);
        single_flow_evap::solve_static(&params, &geometry, &mut steady, &power_density)
            .expect("marches");

        let (_, _, _, mut transient, _) = single_channel_rig(18);
        let previous = steady.clone();
        solve_transient(
            &params,
            &geometry,
            &mut transient,
            &power_density,
            &previous,
            1.0e12,
        )
        .expect("marches");

        for iz in 0..grid.nz {
            let i = grid.index(0, 0, 0, iz);
            let a = transient.coolant.enthalpy[i];
            let b = steady.coolant.enthalpy[i];
            assert!(
                (a - b).abs() / b < 1e-9,
                "node {iz}: transient {a} kJ/kg vs steady {b} kJ/kg"
            );
        }
    }

    /// A converged steady state is a fixed point of the transient step.
    ///
    /// If the previous-step state is the steady solution and nothing else
    /// changes, the implicit march must return the same enthalpies for any
    /// `dt`. This is what keeps a transient from drifting off its own initial
    /// condition.
    #[test]
    fn a_steady_state_is_a_fixed_point_of_the_transient_step() {
        let (grid, params, geometry, mut steady, power_density) = single_channel_rig(18);
        single_flow_evap::solve_static(&params, &geometry, &mut steady, &power_density)
            .expect("marches");

        let mut stepped = steady.clone();
        let previous = steady.clone();
        solve_transient(
            &params,
            &geometry,
            &mut stepped,
            &power_density,
            &previous,
            0.001,
        )
        .expect("marches");

        for iz in 0..grid.nz {
            let i = grid.index(0, 0, 0, iz);
            let a = stepped.coolant.enthalpy[i];
            let b = steady.coolant.enthalpy[i];
            assert!(
                (a - b).abs() / b < 1e-9,
                "node {iz} drifted from {b} kJ/kg to {a} kJ/kg"
            );
        }
    }

    /// A very short step barely moves the coolant off its previous state.
    #[test]
    fn a_short_step_keeps_the_coolant_near_its_previous_state() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        let mut previous = state.clone();
        // Give the previous step a non-trivial, uniform enthalpy field.
        let inlet = steam::enthalpy_region1_pt(15.5, 559.15);
        previous.coolant.enthalpy = vec![inlet + 50.0; grid.nodes()];
        previous.coolant.density = vec![0.72; grid.nodes()];

        solve_transient(
            &params,
            &geometry,
            &mut state,
            &power_density,
            &previous,
            1.0e-6,
        )
        .expect("marches");

        for iz in 1..grid.nz {
            let i = grid.index(0, 0, 0, iz);
            let got = state.coolant.enthalpy[i];
            assert!(
                (got - (inlet + 50.0)).abs() < 1.0,
                "node {iz} moved to {got} kJ/kg in 1 microsecond"
            );
        }
    }

    #[test]
    #[should_panic(expected = "strictly positive")]
    fn a_non_positive_time_step_is_rejected() {
        let (_, params, geometry, mut state, power_density) = single_channel_rig(4);
        let previous = state.clone();
        let _ = solve_transient(
            &params,
            &geometry,
            &mut state,
            &power_density,
            &previous,
            -1.0,
        );
    }
}
