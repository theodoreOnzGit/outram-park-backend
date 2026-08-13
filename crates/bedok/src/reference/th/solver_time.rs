//! Transient thermal-hydraulic solver — one implicit-Euler step of the
//! coupled channel + rod system.
//!
//! # Provenance
//!
//! Translated from `th_solvertimexyz.m` by **Than Yan Ren** (Singapore Nuclear
//! Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
//! repaired.
//!
//! # Relationship to the steady solver
//!
//! Structurally identical to [`super::solver::solve_static`], with the two
//! steady kernels swapped for their transient counterparts:
//!
//! | steady | transient |
//! |---|---|
//! | [`super::single_flow_evap::solve_static`] | [`super::single_flow_evap_time::solve_transient`] |
//! | [`super::fuel_rod::solve_static`] | [`super::fuel_rod_time::solve_transient`] |
//!
//! There is **no channel-model choice here**: the transient always marches the
//! homogeneous-equilibrium model. That is why `th_solverxyz.m` offers
//! `params.th_model = 'hem'` at all — a transient needs its `t = 0` steady
//! state from the same model, or the density mismatch shows up as a spurious
//! reactivity step at `t = 0`.
//!
//! # Not translated
//!
//! The `params.debugdump` CSV dumps (`th_solvertimexyz.m` lines 149-161) and
//! the same dead locals the steady solver carries. See
//! [`super::solver`].

use super::solver::normalise_and_collapse;
use super::{
    fuel_rod_time, matlab_real_powf, pause_on_nan, single_flow_evap, single_flow_evap_time,
    ThError, ThGeometry, ThResult, ThermalHydraulicParams, ThermalHydraulicState,
};

/// Advance the whole core's thermal hydraulics through one time step.
///
/// MATLAB `th_solvertimexyz(params, geometry, th, whichsigma, pwrdens, thold,
/// dt)`.
///
/// # Arguments
///
/// - `params` — grid, fuel node counts and the fuel-temperature clamp.
///   `params.channel_model` is **ignored**: the transient always uses the
///   homogeneous-equilibrium channel march.
/// - `geometry` — axial node heights \[cm\], channel extents, fuel-pin radial
///   geometry (including the volumetric heat capacities the transient needs).
/// - `th` — the current T-H iterate, updated in place. `th.heat_flux`
///   \[W/cm²\] feeds the coolant energy source as the wall flux of the
///   previous step or Picard pass, and `th.power_ratio` must already carry the
///   current relative core power.
/// - `which_sigma` — material index per spatial node, `grid.nodes()` long,
///   `0` meaning "no material".
/// - `power_density` — \[-\] nodal fission power, the full
///   `grid.state_len()` vector; normalised and group-collapsed inside.
/// - `previous_step` — the **converged** T-H state of the previous time step,
///   supplying the capacity terms for both the coolant and the rods.
/// - `time_step` — \[s\] the step size `dt`.
///
/// # Errors
///
/// As [`super::solver::solve_static`], plus anything
/// [`super::fuel_rod_time::solve_transient`] can raise.
///
/// # Panics
///
/// If `time_step` is not strictly positive.
pub fn solve_transient(
    params: &ThermalHydraulicParams,
    geometry: &ThGeometry,
    th: &mut ThermalHydraulicState,
    which_sigma: &[usize],
    power_density: &[f64],
    previous_step: &ThermalHydraulicState,
    time_step: f64,
) -> ThResult<()> {
    assert!(
        time_step > 0.0,
        "thermal-hydraulic time step must be strictly positive, got {time_step}"
    );

    let grid = params.grid;
    let nodes = grid.nodes();
    let fuel_rings = params.fuel.fuel_rings;

    single_flow_evap::check_length(
        "th_solvertimexyz pwrdens",
        grid.state_len(),
        power_density.len(),
    )?;
    single_flow_evap::check_length("th_solvertimexyz whichsigma", nodes, which_sigma.len())?;
    single_flow_evap::check_length(
        "th_solvertimexyz geometry.Lz",
        nodes,
        geometry.axial_height.len(),
    )?;
    single_flow_evap::check_length(
        "th_solvertimexyz thold.fueltemp",
        th.radial_nodes * nodes,
        previous_step.fuel_temperature.len(),
    )?;
    if fuel_rings >= th.radial_nodes {
        return Err(ThError::LengthMismatch {
            what: "th_solvertimexyz: fueln+1 exceeds the rod's radial solution nodes",
            expected: th.radial_nodes,
            got: fuel_rings + 1,
        });
    }
    if previous_step.radial_nodes != th.radial_nodes {
        return Err(ThError::LengthMismatch {
            what: "th_solvertimexyz: thold has a different radial mesh",
            expected: th.radial_nodes,
            got: previous_step.radial_nodes,
        });
    }

    let collapsed = normalise_and_collapse(&grid, power_density);

    // ---- transient coolant channel update --------------------------------
    single_flow_evap_time::solve_transient(
        params,
        geometry,
        th,
        &collapsed,
        previous_step,
        time_step,
    )?;

    // ---- heat transfer coefficients (Dittus-Boelter) ---------------------
    let pitch = geometry.fuel.pitch;
    let outer_radius = geometry.fuel.outer_radius;
    let fuel_radius = geometry.fuel.fuel_radius;
    let doppler_alpha = geometry.fuel.doppler_alpha;

    let subchannel_area = pitch * pitch - std::f64::consts::PI * outer_radius * outer_radius;
    let hydraulic_diameter = 4.0 * subchannel_area
        / (2.0 * std::f64::consts::PI * outer_radius + 4.0 * pitch - 8.0 * outer_radius);

    let mut heat_transfer_coefficient = vec![0.0; nodes];
    let mut pin_power_density = vec![0.0; nodes];
    let mut linear_power_density = vec![0.0; nodes];
    for i in 0..nodes {
        let reynolds =
            th.coolant.mixture_velocity[i] * hydraulic_diameter / th.coolant.kinematic_viscosity[i];
        let nusselt =
            0.023 * matlab_real_powf(th.coolant.prandtl[i], 0.4) * matlab_real_powf(reynolds, 0.8);
        heat_transfer_coefficient[i] =
            th.coolant.thermal_conductivity[i] * nusselt / hydraulic_diameter;
        linear_power_density[i] =
            collapsed[i] * th.max_power * th.power_ratio / geometry.axial_height[i];
        pin_power_density[i] = (1.0 - th.coolant_heat_fraction) * linear_power_density[i]
            / th.n_fuel_pins
            / (std::f64::consts::PI * fuel_radius * fuel_radius);
    }

    // ---- transient fuel rod conduction, node by node ----------------------
    let mut heat_flux = vec![0.0; nodes];

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let channel = ThGeometry::channel_index(&grid, ix, iy);
            let z_low = geometry.z_low[channel];
            let z_high = geometry.z_high[channel];

            if which_sigma[grid.index(0, ix, iy, z_low)] == 0 {
                continue;
            }

            for iz in z_low..=z_high {
                let node = grid.index(0, ix, iy, iz);
                if pin_power_density[node] == 0.0 {
                    continue;
                }

                let coolant_temperature = th.coolant.temperature[node];
                let boundary_coefficient = heat_transfer_coefficient[node] * outer_radius;
                let current_profile = th.fuel_temperature_row(node).to_vec();
                let old_profile = previous_step.fuel_temperature_row(node);

                let mut profile = fuel_rod_time::solve_transient(
                    &params.fuel,
                    &geometry.fuel,
                    &current_profile,
                    old_profile,
                    pin_power_density[node],
                    boundary_coefficient,
                    coolant_temperature,
                    time_step,
                )?;

                // Same clamp as the static solver.
                let floor = if coolant_temperature.is_finite() {
                    coolant_temperature
                } else {
                    0.0
                };
                for value in &mut profile {
                    *value = value.max(floor).min(params.max_fuel_temperature);
                }

                let doppler =
                    (1.0 - doppler_alpha) * profile[0] + doppler_alpha * profile[fuel_rings];
                th.fuel_temperature_doppler[node] = doppler;
                th.fuel_temperature_average[node] = doppler;
                heat_flux[node] = heat_transfer_coefficient[node]
                    * (profile[th.radial_nodes - 1] - coolant_temperature);

                if profile.iter().any(|v| v.is_nan()) {
                    let fallback = if coolant_temperature.is_finite() {
                        coolant_temperature
                    } else {
                        params.coolant_average_temperature
                    };
                    eprintln!(
                        "warning: th_solvertimexyz:nanFuelTemp: NaN fuel temperature at node \
                         idx={node} (pinpowdens={:.4e}, coolant temp={:.4e}); substituting \
                         {fallback:.1} K and continuing.",
                        pin_power_density[node], coolant_temperature
                    );
                    for value in &mut profile {
                        *value = fallback;
                    }
                    th.fuel_temperature_doppler[node] = fallback;
                    th.fuel_temperature_average[node] = fallback;
                    heat_flux[node] = 0.0;
                }

                th.fuel_temperature_row_mut(node).copy_from_slice(&profile);
            }
        }
    }

    th.heat_flux = heat_flux;
    th.linear_power_density = linear_power_density;

    // ---- pauseonnan ------------------------------------------------------
    pause_on_nan("th.coolant.enth", &th.coolant.enthalpy)?;
    pause_on_nan("th.coolant.temps", &th.coolant.temperature)?;
    pause_on_nan("th.coolant.dens", &th.coolant.density)?;
    pause_on_nan("th.fueltemp", &th.fuel_temperature)?;
    pause_on_nan("th.heatflux", &th.heat_flux)?;
    pause_on_nan("th.fueltempavg", &th.fuel_temperature_average)?;
    pause_on_nan("th.fueltempdoppler", &th.fuel_temperature_doppler)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::single_flow_evap::test_support::single_channel_rig;
    use super::super::solver;
    use super::*;

    /// A converged steady state is a fixed point of the coupled transient step.
    ///
    /// **Methodology.** Drive the channel to a Picard-converged steady state,
    /// then take one transient step at unchanged power with that state as both
    /// the current iterate and the previous step. Both sub-solvers are
    /// individually fixed-point-preserving (see their own tests), so the
    /// coupled step must be too. Inputs: one NEACRP-A2-like channel, 18 axial
    /// nodes, 15.5 MPa, 559.15 K inlet, flat power, `dt = 0.001 s`. Pass
    /// criterion: Doppler temperature within 0.01 K and wall heat flux within
    /// 0.1 % at every node.
    ///
    /// **Convergence matters here, and it is worth saying why.** A *single*
    /// call to [`solver::solve_static`] is one Picard pass, not a converged
    /// state: the rod solve evaluates its temperature-dependent conductivities
    /// at the *incoming* profile and returns a different one. Stepping the
    /// transient from such a state moves the Doppler temperature by about
    /// 0.14 K, which is the Picard residual, not transient drift. In the real
    /// code that outer iteration lives in `thdiffusion_solverxyz.m`; the loop
    /// below stands in for it.
    ///
    /// **Result (2026-08-05).** After 60 Picard passes the transient step moves
    /// the Doppler temperature by under 1e-6 K and the wall heat flux by under
    /// 1e-8 relative. Interpretation: the transient path does not drift off a
    /// converged initial condition, so a transient starting from a converged
    /// steady state sees no spurious `t = 0` reactivity step.
    #[test]
    fn a_steady_state_is_a_fixed_point_of_the_coupled_transient_step() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        let which_sigma = vec![1usize; grid.nodes()];

        // Stand-in for the Picard loop of `thdiffusion_solverxyz.m`.
        for _ in 0..60 {
            solver::solve_static(&params, &geometry, &mut state, &which_sigma, &power_density)
                .expect("steady solve");
        }

        let previous = state.clone();
        let mut stepped = state.clone();
        solve_transient(
            &params,
            &geometry,
            &mut stepped,
            &which_sigma,
            &power_density,
            &previous,
            0.001,
        )
        .expect("transient step");

        for iz in 0..grid.nz {
            let node = grid.index(0, 0, 0, iz);
            let before = state.fuel_temperature_doppler[node];
            let after = stepped.fuel_temperature_doppler[node];
            assert!(
                (after - before).abs() < 0.01,
                "node {iz}: Doppler moved {before} K -> {after} K"
            );
            let q_before = state.heat_flux[node];
            let q_after = stepped.heat_flux[node];
            assert!(
                (q_after - q_before).abs() / q_before < 1e-3,
                "node {iz}: heat flux moved {q_before} -> {q_after} W/cm2"
            );
        }
    }

    /// A power step raises the fuel temperature within one time step.
    #[test]
    fn a_power_step_heats_the_fuel() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        let which_sigma = vec![1usize; grid.nodes()];
        solver::solve_static(&params, &geometry, &mut state, &which_sigma, &power_density)
            .expect("steady solve");

        let previous = state.clone();
        let mut stepped = state.clone();
        stepped.power_ratio = 2.0;
        solve_transient(
            &params,
            &geometry,
            &mut stepped,
            &which_sigma,
            &power_density,
            &previous,
            0.1,
        )
        .expect("transient step");

        for iz in 0..grid.nz {
            let node = grid.index(0, 0, 0, iz);
            assert!(
                stepped.fuel_temperature_doppler[node] > previous.fuel_temperature_doppler[node],
                "node {iz}: Doppler did not rise on a power doubling"
            );
        }
    }

    #[test]
    #[should_panic(expected = "strictly positive")]
    fn a_non_positive_time_step_is_rejected() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(4);
        let which_sigma = vec![1usize; grid.nodes()];
        let previous = state.clone();
        let _ = solve_transient(
            &params,
            &geometry,
            &mut state,
            &which_sigma,
            &power_density,
            &previous,
            0.0,
        );
    }
}
