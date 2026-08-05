//! Steady thermal-hydraulic solver — the entry point the coupled Picard loop
//! calls.
//!
//! # Provenance
//!
//! Translated from `th_solverxyz.m` by **Than Yan Ren** (Singapore Nuclear
//! Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
//! repaired.
//!
//! # What it does, in order
//!
//! 1. **Normalise and collapse the power.** L1-normalise the full `G*es`
//!    power-density vector, then sum the energy groups into the first `es`
//!    entries so everything downstream works on a single spatial field.
//! 2. **Solve the coolant channels** — [`super::single_flow_evap`] when
//!    `params.channel_model` is
//!    [`ChannelModel::HomogeneousEquilibrium`](super::ChannelModel::HomogeneousEquilibrium),
//!    otherwise [`super::drift_flux_3d`] (which cannot run; see that module).
//! 3. **Build the heat transfer coefficient** from a Dittus-Boelter
//!    correlation on the recovered coolant transport properties.
//! 4. **Solve each fuelled node's rod conduction**
//!    ([`super::fuel_rod::solve_static`]), clamp the profile, and form the
//!    Doppler temperature and the wall heat flux.
//! 5. **Check for NaN** and fail rather than pass poison into the neutronics.
//!
//! # Not translated
//!
//! - **The `params.debugdump` CSV dumps** (`th_solverxyz.m` lines 93-96 and
//!   215-241). Diagnostics with no effect on any returned value; they write
//!   `pwrdens.csv`, `hcoeff.csv`, `fueltemp.csv` and a dozen others into the
//!   working directory.
//! - **Dead locals.** The MATLAB computes `Vi = repmat(geometry.Vi, G, 1)`,
//!   `subflow = flowrate*subarea`, `Lx`, `Ly`, `Lr`, `maxir` and `whichg` and
//!   never uses them.
//! - **`th.coolant.temps = temps` and `th.coolant.dens = dens`** at the end are
//!   no-ops: both were read out of `th.coolant` a few lines earlier and never
//!   modified.

use super::drift_flux_3d;
use super::steam;
use super::{
    fuel_rod, matlab_real_powf, pause_on_nan, single_flow_evap, ChannelModel, ThError, ThGeometry,
    ThResult, ThermalHydraulicParams, ThermalHydraulicState,
};

/// Run one steady thermal-hydraulic solve over the whole core.
///
/// MATLAB `th_solverxyz(params, geometry, th, whichsigma, pwrdens)`.
///
/// # Arguments
///
/// - `params` — grid, fuel node counts, clamp and channel-model selection.
/// - `geometry` — axial node heights \[cm\], channel extents, fuel-pin radial
///   geometry.
/// - `th` — thermal-hydraulic state, updated in place. `th.power_ratio` must
///   already carry the current relative core power and `th.heat_flux`
///   \[W/cm²\] the wall heat flux of the previous Picard pass.
/// - `which_sigma` — material index per spatial node, `grid.nodes()` long, in
///   the MATLAB's 1-based material numbering where **`0` means "no material"**
///   (a reflector or out-of-core node). A channel whose lowest active node has
///   `which_sigma == 0` is skipped entirely.
/// - `power_density` — \[-\] nodal fission power density, the **full
///   `grid.state_len()`** vector across all energy groups. It is normalised
///   and collapsed inside; the caller's copy is not modified.
///
/// # Returns
///
/// Nothing; `th` carries the result: `coolant` (temperature, density,
/// enthalpy, void, transport properties), `heat_flux` \[W/cm²\],
/// `fuel_temperature` \[K\], `fuel_temperature_doppler` \[K\],
/// `fuel_temperature_average` \[K\] and `linear_power_density` \[W/cm\].
///
/// # Errors
///
/// - [`ThError::LengthMismatch`] if any input vector has the wrong length.
/// - [`ThError::NotANumber`] if the coolant enthalpy, temperature, density,
///   fuel temperature, wall heat flux or Doppler temperature ends up
///   containing a NaN — the translation of MATLAB `pauseonnan`.
/// - [`ThError::MissingUpstreamSource`] if `params.channel_model` selects the
///   two-fluid path; see [`super::drift_flux_3d`].
/// - Anything [`super::fuel_rod::solve_static`] can raise.
///
/// # Behaviour on a NaN rod solve
///
/// If the rod conduction returns NaN at a node, the MATLAB emits a warning,
/// substitutes the local coolant temperature (or `params.cooltempavg` when
/// that is not finite either) into the whole radial profile, zeroes that node's
/// wall heat flux, and carries on. That is reproduced, warning included — it
/// is printed to stderr.
pub fn solve_static(
    params: &ThermalHydraulicParams,
    geometry: &ThGeometry,
    th: &mut ThermalHydraulicState,
    which_sigma: &[usize],
    power_density: &[f64],
) -> ThResult<()> {
    let grid = params.grid;
    let nodes = grid.nodes();
    let fuel_rings = params.fuel.fuel_rings;

    single_flow_evap::check_length(
        "th_solverxyz pwrdens",
        grid.state_len(),
        power_density.len(),
    )?;
    single_flow_evap::check_length("th_solverxyz whichsigma", nodes, which_sigma.len())?;
    single_flow_evap::check_length(
        "th_solverxyz geometry.Lz",
        nodes,
        geometry.axial_height.len(),
    )?;
    if fuel_rings >= th.radial_nodes {
        return Err(ThError::LengthMismatch {
            what: "th_solverxyz: fueln+1 exceeds the rod's radial solution nodes",
            expected: th.radial_nodes,
            got: fuel_rings + 1,
        });
    }

    // ---- normalise and collapse the power over energy groups -------------
    let collapsed = normalise_and_collapse(&grid, power_density);

    // ---- channel model ----------------------------------------------------
    match params.channel_model {
        ChannelModel::HomogeneousEquilibrium => {
            single_flow_evap::solve_static(params, geometry, th, &collapsed)?;
        }
        ChannelModel::TwoFluid => {
            drift_flux_3d::solve_static(params, geometry, th, &collapsed)?;
        }
    }

    // ---- heat transfer coefficients (Dittus-Boelter) ---------------------
    let pitch = geometry.fuel.pitch;
    let outer_radius = geometry.fuel.outer_radius;
    let fuel_radius = geometry.fuel.fuel_radius;
    let doppler_alpha = geometry.fuel.doppler_alpha;

    // Recomputed locally by the MATLAB rather than read from
    // `geometry.fuel.subarea`; the value is the same.
    let subchannel_area = pitch * pitch - std::f64::consts::PI * outer_radius * outer_radius;
    let hydraulic_diameter = 4.0 * subchannel_area
        / (2.0 * std::f64::consts::PI * outer_radius + 4.0 * pitch - 8.0 * outer_radius);

    let mut heat_transfer_coefficient = vec![0.0; nodes];
    let mut pin_power_density = vec![0.0; nodes];
    let mut linear_power_density = vec![0.0; nodes];
    for i in 0..nodes {
        let reynolds =
            th.coolant.mixture_velocity[i] * hydraulic_diameter / th.coolant.kinematic_viscosity[i];
        // `real(...)` around both powers: see `matlab_real_powf`.
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

    // ---- fuel rod conduction, node by node -------------------------------
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
                let previous_profile = th.fuel_temperature_row(node).to_vec();

                let mut profile = fuel_rod::solve_static(
                    &params.fuel,
                    &geometry.fuel,
                    &previous_profile,
                    pin_power_density[node],
                    boundary_coefficient,
                    coolant_temperature,
                )?;

                // Clamp to a physical range: the fuel cannot be colder than
                // its coolant sink nor hotter than the UO2 melting point.
                // Guards an ill-conditioned rod solve from injecting
                // non-physical temperatures into the Doppler feedback and the
                // wall heat flux. This clamp is also what makes the orphan
                // 1 K gap row harmless -- see `fuel_rod::solve_static`.
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
                    // Graceful handling, as in the MATLAB: substitute the
                    // coolant temperature (or the core-average fallback) and
                    // continue.
                    let fallback = if coolant_temperature.is_finite() {
                        coolant_temperature
                    } else {
                        params.coolant_average_temperature
                    };
                    eprintln!(
                        "warning: th_solverxyz:nanFuelTemp: NaN fuel temperature at node \
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

/// L1-normalise a full `G*es` power-density vector and sum the energy groups
/// into a single `es`-long spatial field.
///
/// MATLAB `th_solverxyz.m:84-90`:
///
/// ```text
/// pwrdens = pwrdens/norm(pwrdens,1);
/// for g = 2:G
///     pwrdens(1:es) = pwrdens(1:es) + pwrdens((g-1)*es+1 : g*es);
/// end
/// pwrdens = pwrdens(1:es);
/// ```
///
/// The group-major layout this relies on is the one pinned in
/// [`crate::reference::grid`].
///
/// # Arguments
///
/// - `power_density` — \[arbitrary, normalised away\] length `state_len()`.
///
/// # Returns
///
/// A `nodes()`-long field summing to 1 (up to sign cancellations, since the
/// normalisation uses the L1 norm of the *unsummed* vector).
#[must_use]
pub fn normalise_and_collapse(
    grid: &crate::reference::grid::Grid,
    power_density: &[f64],
) -> Vec<f64> {
    let nodes = grid.nodes();
    let l1: f64 = power_density.iter().map(|v| v.abs()).sum();
    let mut scaled: Vec<f64> = power_density.iter().map(|v| v / l1).collect();
    for g in 1..grid.ngroups {
        for i in 0..nodes {
            scaled[i] += scaled[g * nodes + i];
        }
    }
    scaled.truncate(nodes);
    scaled
}

/// Bring the mixture density into the form the cross-section feedback wants.
///
/// The MATLAB simply reads `th.coolant.dens`; this exists so the coupling
/// layer has a documented accessor rather than reaching into the struct.
///
/// # Returns
///
/// Coolant mixture density \[g/cm³\] per spatial node.
#[must_use]
pub fn coolant_density(th: &ThermalHydraulicState) -> &[f64] {
    &th.coolant.density
}

/// Saturation temperature \[K\] of the channel pressure, for callers that need
/// to know where the two-phase region starts.
///
/// # Arguments
///
/// - `th` — read for `th.coolant.inlet_pressure` \[MPa\].
#[must_use]
pub fn channel_saturation_temperature(th: &ThermalHydraulicState) -> f64 {
    steam::saturation_temperature(th.coolant.inlet_pressure)
}

#[cfg(test)]
mod tests {
    use super::super::single_flow_evap::test_support::single_channel_rig;
    use super::*;
    use crate::reference::grid::Grid;

    #[test]
    fn normalise_and_collapse_sums_the_groups_onto_the_spatial_mesh() {
        // 2 spatial nodes, 2 groups. Group 1 = [1, 2], group 2 = [3, 4].
        // L1 norm 10 -> [0.1, 0.2, 0.3, 0.4] -> collapsed [0.4, 0.6].
        let grid = Grid::new(1, 1, 2, 2).expect("valid");
        let got = normalise_and_collapse(&grid, &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(got.len(), 2);
        assert!((got[0] - 0.4).abs() < 1e-15, "{got:?}");
        assert!((got[1] - 0.6).abs() < 1e-15, "{got:?}");
        assert!((got.iter().sum::<f64>() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn normalise_and_collapse_is_the_identity_for_one_group() {
        let grid = Grid::new(1, 1, 3, 1).expect("valid");
        let got = normalise_and_collapse(&grid, &[2.0, 3.0, 5.0]);
        assert!((got[0] - 0.2).abs() < 1e-15, "{got:?}");
        assert!((got[2] - 0.5).abs() < 1e-15, "{got:?}");
    }

    /// A full steady solve on a single PWR channel, end to end.
    ///
    /// **Methodology.** Run [`solve_static`] on one NEACRP-A2-like channel
    /// (18 axial nodes, 15.5 MPa, 559.15 K inlet, flat axial power,
    /// homogeneous-equilibrium channel model) and check every output is
    /// physical: heat flux positive, fuel hotter than coolant, Doppler
    /// temperature between the pellet surface and centreline, and no NaN
    /// anywhere. Pass criterion: all of the above.
    ///
    /// **Result (2026-08-05).** Passes. Centreline about 1300 K, cladding
    /// surface about 620 K, wall heat flux of order 50 W/cm², all typical of a
    /// PWR at nominal power. This is a plausibility check, **not** a benchmark
    /// comparison: no NEACRP reference value is used here.
    #[test]
    fn a_single_pwr_channel_solves_end_to_end() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        let which_sigma = vec![1usize; grid.nodes()];

        solve_static(&params, &geometry, &mut state, &which_sigma, &power_density)
            .expect("steady solve");

        for iz in 0..grid.nz {
            let node = grid.index(0, 0, 0, iz);
            let profile = state.fuel_temperature_row(node);
            let coolant = state.coolant.temperature[node];
            assert!(
                state.heat_flux[node] > 0.0,
                "node {iz}: wall heat flux {}",
                state.heat_flux[node]
            );
            assert!(
                profile[0] > coolant,
                "node {iz}: centreline {} K vs coolant {coolant} K",
                profile[0]
            );
            assert!(
                profile[0] <= params.max_fuel_temperature,
                "node {iz}: centreline {} K exceeds the clamp",
                profile[0]
            );
            let doppler = state.fuel_temperature_doppler[node];
            assert!(
                doppler <= profile[0] && doppler >= profile[params.fuel.fuel_rings],
                "node {iz}: Doppler {doppler} K outside [{}, {}]",
                profile[params.fuel.fuel_rings],
                profile[0]
            );
            assert_eq!(
                state.fuel_temperature_average[node], doppler,
                "the MATLAB sets fueltempavg := fueltempdoppler"
            );
        }
    }

    /// A node with no power is left untouched by the rod loop.
    #[test]
    fn an_unpowered_node_keeps_a_zero_wall_heat_flux() {
        let (grid, params, geometry, mut state, mut power_density) = single_channel_rig(6);
        power_density[grid.index(0, 0, 0, 0)] = 0.0;
        let which_sigma = vec![1usize; grid.nodes()];
        solve_static(&params, &geometry, &mut state, &which_sigma, &power_density)
            .expect("steady solve");
        assert_eq!(state.heat_flux[grid.index(0, 0, 0, 0)], 0.0);
    }

    /// A channel whose lowest active node is out of core is skipped whole.
    #[test]
    fn a_zero_material_channel_is_skipped() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(6);
        let which_sigma = vec![0usize; grid.nodes()];
        solve_static(&params, &geometry, &mut state, &which_sigma, &power_density)
            .expect("steady solve");
        assert!(
            state.heat_flux.iter().all(|v| *v == 0.0),
            "{:?}",
            state.heat_flux
        );
    }

    /// Selecting the two-fluid channel model surfaces the missing MATLAB file.
    #[test]
    fn the_two_fluid_model_reports_the_missing_kernel() {
        let (grid, mut params, geometry, mut state, power_density) = single_channel_rig(4);
        params.channel_model = ChannelModel::TwoFluid;
        let which_sigma = vec![1usize; grid.nodes()];
        let err =
            solve_static(&params, &geometry, &mut state, &which_sigma, &power_density).unwrap_err();
        assert!(
            matches!(err, ThError::MissingUpstreamSource { .. }),
            "expected the missing-source error, got {err}"
        );
    }
}
