//! Multichannel wrapper for the staggered six-equation two-fluid solver.
//!
//! # Provenance
//!
//! Translated from `driftflux6_solverstatic3d.m` by **Than Yan Ren**
//! (Singapore Nuclear Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05.
//!
//! # THE SINGLE-CHANNEL SOLVER IS MISSING FROM THE SNAPSHOT
//!
//! **`driftflux6_solverstatic3d.m` calls `driftflux6_solverstatic1d` at its
//! line 157. No file in the snapshot defines it.** Yan Ren handed the code
//! over unfinished and that kernel was never written — there is nothing to
//! translate, and per `docs/bedok-port-scoping.md` §1.0 nothing is invented to
//! fill the gap. [`solve_single_channel_static`] is the named hole; it returns
//! [`ThError::MissingUpstreamSource`] and calling it is the only way to reach
//! that error.
//!
//! Everything *around* the missing call is translated faithfully — the inlet
//! state, the warm-start bookkeeping, the previous-state defaults for
//! unpowered columns, and the whole derived-field recovery tail — so that when
//! the kernel is eventually written the wrapper it plugs into is already
//! here and already reviewed.
//!
//! **Use [`super::single_flow_evap`] instead.** The homogeneous-equilibrium
//! path is complete, and it is what the benchmark cases run
//! (`neacrpd1t.m` sets `params.th_model = 'hem'`).
//!
//! # Deviation from the MATLAB, and why
//!
//! The MATLAB wraps each channel solve in `try/catch` and, on failure, keeps
//! that channel's previous state and continues (`driftflux6_solverstatic3d.m`
//! lines 165-168) — the surrounding Picard under-relaxation is expected to
//! absorb one stale channel-cycle. Reproducing that here would mean **every**
//! channel failing silently, the recovery tail running on inlet-state
//! defaults, and the function returning a plausible-looking converged result
//! built from nothing. [`solve_static`] therefore propagates the error instead
//! of swallowing it. This is a deliberate, documented departure from faithful
//! translation, made because the faithful path is unreachable: no channel can
//! ever succeed while the kernel is absent.
//!
//! # Also not translated
//!
//! - **The `parfor` channel sharding.** `params.stag6_par` / `stag6_nworkers`
//!   select a MATLAB process pool. The channels are independent, so this
//!   changes throughput and not results; the translation is serial.
//! - **The `evalc` log suppression** around the channel call, and the `verb`
//!   progress line. Diagnostics only.

use super::single_flow_evap::check_length;
use super::steam;
use super::{ThError, ThGeometry, ThResult, ThermalHydraulicParams, ThermalHydraulicState};

/// The primary six-equation state one channel solve returns.
///
/// MATLAB `[Pc, Ac, VLc, VGc, TLc, TGc, Ust, qr, rel, stp, warm, fail]` out of
/// the local `stag6_channel` helper. Each vector is `nz` long, bottom node
/// first.
#[derive(Debug, Clone, PartialEq)]
pub struct SingleChannelSolution {
    /// Pressure \[MPa\] at each axial node.
    pub pressure: Vec<f64>,
    /// Void fraction \[-\] at each axial node.
    pub void_fraction: Vec<f64>,
    /// Liquid velocity \[cm/s\] at each axial node.
    pub liquid_velocity: Vec<f64>,
    /// Vapour velocity \[cm/s\] at each axial node.
    pub gas_velocity: Vec<f64>,
    /// Liquid temperature \[K\] at each axial node.
    pub liquid_temperature: Vec<f64>,
    /// Vapour temperature \[K\] at each axial node.
    pub gas_temperature: Vec<f64>,
    /// Staggered state vector, `6*nz` long, kept as the next warm start.
    /// MATLAB `r.Ustag`.
    pub staggered_state: Vec<f64>,
    /// Relative residual reached \[-\]. MATLAB `r.relerr`.
    pub relative_error: f64,
    /// JFNK steps taken. MATLAB `r.nsteps`.
    pub steps: usize,
}

/// The missing single-channel staggered six-equation solver.
///
/// MATLAB `driftflux6_solverstatic1d(pc, gch, thch, pwch)` — **a file that
/// does not exist in the handed-over snapshot**. It is named here so the gap
/// is a findable, typed item rather than a comment, and so that whoever writes
/// the kernel has the call signature the wrapper expects.
///
/// From the wrapper's own use of the result, the kernel is expected to return
/// per-node pressure \[MPa\], void fraction \[-\], liquid and vapour velocity
/// \[cm/s\], liquid and vapour temperature \[K\], a `6*nz` staggered state
/// vector for warm starting, a relative residual and a step count. Nothing
/// else about it can be established from the snapshot, and nothing is assumed.
///
/// # Errors
///
/// Always [`ThError::MissingUpstreamSource`].
pub fn solve_single_channel_static(
    _params: &ThermalHydraulicParams,
    _geometry: &ThGeometry,
    _th: &ThermalHydraulicState,
    _channel_power_density: &[f64],
) -> ThResult<SingleChannelSolution> {
    Err(ThError::MissingUpstreamSource {
        missing: "driftflux6_solverstatic1d.m",
        caller: "driftflux6_solverstatic3d.m",
    })
}

/// Solve every fuelled channel with the six-equation two-fluid model and
/// recover the derived thermodynamic fields over the whole domain.
///
/// MATLAB `driftflux6_solverstatic3d(params, geometry, th, pwrdens)`.
///
/// # Arguments
///
/// - `params` — grid and solver knobs.
/// - `geometry` — axial node heights \[cm\] and per-channel extents.
/// - `th` — thermal-hydraulic state, updated in place.
/// - `power_density` — \[-\] L1-normalised, group-collapsed nodal power,
///   `grid.nodes()` long. A channel is "fuelled" if any node of its axial
///   column is non-zero.
///
/// # Errors
///
/// - [`ThError::MissingUpstreamSource`] as soon as the first fuelled channel
///   needs [`solve_single_channel_static`] — which is to say, always, for any
///   case with power in it. See this module's header.
/// - [`ThError::LengthMismatch`] if an input vector is not `grid.nodes()` long.
pub fn solve_static(
    params: &ThermalHydraulicParams,
    geometry: &ThGeometry,
    th: &mut ThermalHydraulicState,
    power_density: &[f64],
) -> ThResult<()> {
    let grid = params.grid;
    let nodes = grid.nodes();
    let channels = grid.nx * grid.ny;

    check_length(
        "driftflux6_solverstatic3d pwrdens",
        nodes,
        power_density.len(),
    )?;
    check_length(
        "driftflux6_solverstatic3d geometry.Lz",
        nodes,
        geometry.axial_height.len(),
    )?;

    // ---- inlet state -----------------------------------------------------
    let inlet_pressure = th.coolant.inlet_pressure;
    let inlet_temperature = th.coolant.inlet_temperature;
    let inlet_void = th.coolant.inlet_void;
    let inlet_saturation_temperature = steam::saturation_temperature(inlet_pressure);
    let inlet_liquid_density =
        1.0 / steam::specific_volume_region1_pt(inlet_pressure, inlet_temperature) / 1000.0;
    let inlet_gas_density =
        1.0 / steam::specific_volume_region2_pt(
            inlet_pressure,
            inlet_saturation_temperature + 2.0 * super::MATLAB_EPS,
        ) / 1000.0;
    let peak_flow_rate = th
        .flow_rate
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let inlet_mixture_velocity = peak_flow_rate
        / (inlet_void * inlet_gas_density + (1.0 - inlet_void) * inlet_liquid_density);

    // ---- warm-start store, sized lazily exactly as the MATLAB does -------
    if th.stag6_u_stag.len() != 6 * grid.nz * channels {
        th.stag6_u_stag = vec![0.0; 6 * grid.nz * channels];
        th.stag6_q_ref = vec![0.0; nodes];
        th.stag6_rel_err = vec![f64::NAN; channels];
    }

    // ---- previous-state defaults for unpowered columns and failed solves --
    let mut pressure = default_or_previous(&th.coolant.pressure, nodes, inlet_pressure);
    let mut void_fraction =
        default_or_previous(&th.coolant.void_fraction, nodes, inlet_void.max(1e-9));
    let mut liquid_velocity =
        default_or_previous(&th.coolant.liquid_velocity, nodes, inlet_mixture_velocity);
    let mut gas_velocity =
        default_or_previous(&th.coolant.gas_velocity, nodes, inlet_mixture_velocity);
    let mut liquid_temperature =
        default_or_previous(&th.coolant.liquid_temperature, nodes, inlet_temperature);
    let mut gas_temperature = default_or_previous(
        &th.coolant.gas_temperature,
        nodes,
        inlet_saturation_temperature,
    );

    // ---- per-channel solves ----------------------------------------------
    // MATLAB runs this as a `parfor` over a process pool; the channels are
    // independent 1-D solves so the translation is serial (see the module
    // header).
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let column: Vec<usize> = (0..grid.nz).map(|iz| grid.index(0, ix, iy, iz)).collect();
            let channel_power: Vec<f64> = column.iter().map(|&i| power_density[i]).collect();
            if !channel_power.iter().any(|v| *v != 0.0) {
                // Unpowered / reflector column: keep its previous state.
                continue;
            }

            // The MATLAB decides here whether to reuse the channel's previous
            // solution as a warm start: only if that solve converged
            // (relerr < 1e-3) AND the wall flux has moved less than 20 %
            // since. An unconverged mid-march state is a poisoned seed, and
            // under a hard flux ramp the evaporation seed rebuilt from the
            // current flux tracks the problem better. That logic is inert
            // while the kernel below is missing, so it is described rather
            // than executed.
            let solution = solve_single_channel_static(params, geometry, th, &channel_power)?;

            for (slot, &node) in column.iter().enumerate() {
                pressure[node] = solution.pressure[slot];
                void_fraction[node] = solution.void_fraction[slot];
                liquid_velocity[node] = solution.liquid_velocity[slot];
                gas_velocity[node] = solution.gas_velocity[slot];
                liquid_temperature[node] = solution.liquid_temperature[slot];
                gas_temperature[node] = solution.gas_temperature[slot];
            }
        }
    }

    // ---- pack primary fields ---------------------------------------------
    // MATLAB compat: `temps := tempsliq`.
    let mixture_temperature = liquid_temperature.clone();

    // ---- recover derived fields over the whole domain ---------------------
    let mut liquid_density = vec![0.0; nodes];
    let mut gas_density = vec![0.0; nodes];
    let mut density = vec![0.0; nodes];
    let mut mixture_velocity = vec![0.0; nodes];
    let mut enthalpy = vec![0.0; nodes];
    let mut quality = vec![0.0; nodes];
    let mut prandtl = vec![0.0; nodes];
    let mut kinematic_viscosity = vec![0.0; nodes];
    let mut thermal_conductivity = vec![0.0; nodes];

    for i in 0..nodes {
        let p = pressure[i];
        let t_sat = steam::saturation_temperature(p);
        // Force the liquid and vapour branches. Unlike the `Tsat - 2*eps`
        // elsewhere in the MATLAB, the 1e-3 K offset here is large enough to
        // actually move the state off the saturation line.
        let t_liquid = liquid_temperature[i].min(t_sat - 1.0e-3);
        let t_gas = gas_temperature[i].max(t_sat + 1.0e-3);

        liquid_density[i] = 1.0 / steam::specific_volume_region1_pt(p, t_liquid) / 1000.0;
        gas_density[i] = 1.0 / steam::specific_volume_region2_pt(p, t_gas) / 1000.0;
        density[i] =
            void_fraction[i] * gas_density[i] + (1.0 - void_fraction[i]) * liquid_density[i];
        mixture_velocity[i] = (void_fraction[i] * gas_density[i] * gas_velocity[i]
            + (1.0 - void_fraction[i]) * liquid_density[i] * liquid_velocity[i])
            / density[i];

        let enthalpy_liquid = steam::enthalpy_region1_pt(p, t_liquid);
        let enthalpy_gas = steam::enthalpy_region2_pt(p, t_gas);
        enthalpy[i] = (void_fraction[i] * gas_density[i] * enthalpy_gas
            + (1.0 - void_fraction[i]) * liquid_density[i] * enthalpy_liquid)
            / density[i];

        let h_vapour_sat = steam::saturated_vapour_enthalpy(p);
        let h_liquid_sat = steam::saturated_liquid_enthalpy(p);
        // Note the MATLAB divides by the latent heat but subtracts the LOCAL
        // liquid enthalpy, not the saturated one, so this is not the
        // equilibrium quality outside saturation. Left as written.
        quality[i] = (enthalpy[i] - enthalpy_liquid) / (h_vapour_sat - h_liquid_sat);

        let conductivity_si = steam::thermal_conductivity_pt(p, t_liquid);
        let viscosity = steam::dynamic_viscosity_pt(p, t_liquid);
        let heat_capacity = steam::isobaric_heat_capacity_region1_pt(p, t_liquid);
        prandtl[i] = heat_capacity * viscosity / conductivity_si * 1000.0;
        kinematic_viscosity[i] =
            viscosity * steam::specific_volume_region1_pt(p, t_liquid) * 10000.0;
        thermal_conductivity[i] = conductivity_si / 100.0;
    }

    th.coolant.pressure = pressure;
    th.coolant.void_fraction = void_fraction;
    th.coolant.liquid_velocity = liquid_velocity;
    th.coolant.gas_velocity = gas_velocity;
    th.coolant.liquid_temperature = liquid_temperature;
    th.coolant.gas_temperature = gas_temperature;
    th.coolant.temperature = mixture_temperature;
    th.coolant.liquid_density = liquid_density;
    th.coolant.gas_density = gas_density;
    th.coolant.density = density;
    th.coolant.mixture_velocity = mixture_velocity;
    th.coolant.enthalpy = enthalpy;
    th.coolant.quality = quality;
    th.coolant.prandtl = prandtl;
    th.coolant.kinematic_viscosity = kinematic_viscosity;
    th.coolant.thermal_conductivity = thermal_conductivity;

    Ok(())
}

/// MATLAB `getfield_or(s, f, d)`: keep the previous field if it has the right
/// length, otherwise fill with the default.
fn default_or_previous(previous: &[f64], nodes: usize, default: f64) -> Vec<f64> {
    if previous.len() == nodes {
        previous.to_vec()
    } else {
        vec![default; nodes]
    }
}

#[cfg(test)]
mod tests {
    use super::super::single_flow_evap::test_support::single_channel_rig;
    use super::*;

    /// The missing kernel is reported, by name, rather than silently faked.
    #[test]
    fn the_missing_single_channel_solver_names_itself() {
        let (_, params, geometry, state, power_density) = single_channel_rig(4);
        let err =
            solve_single_channel_static(&params, &geometry, &state, &power_density).unwrap_err();
        match err {
            ThError::MissingUpstreamSource { missing, caller } => {
                assert_eq!(missing, "driftflux6_solverstatic1d.m");
                assert_eq!(caller, "driftflux6_solverstatic3d.m");
            }
            other => panic!("wrong error: {other}"),
        }
    }

    /// Any powered case propagates the missing-source error out of the
    /// wrapper. This is the deliberate deviation documented in the module
    /// header: the MATLAB would swallow it and return inlet-state defaults.
    #[test]
    fn a_powered_case_propagates_the_missing_source_error() {
        let (_, params, geometry, mut state, power_density) = single_channel_rig(4);
        let err = solve_static(&params, &geometry, &mut state, &power_density).unwrap_err();
        assert!(
            matches!(err, ThError::MissingUpstreamSource { .. }),
            "expected the missing-source error, got {err}"
        );
    }

    /// With no power anywhere no channel solve is attempted, so the wrapper
    /// runs its derived-field recovery tail to completion. That is the only
    /// path through this file that can be exercised today, and it checks the
    /// tail's units and the inlet-state defaults.
    #[test]
    fn an_unpowered_case_runs_the_recovery_tail() {
        let (grid, params, geometry, mut state, _) = single_channel_rig(4);
        let power_density = vec![0.0; grid.nodes()];
        solve_static(&params, &geometry, &mut state, &power_density).expect("no channel solved");

        for i in 0..grid.nodes() {
            assert!(
                (0.5..0.85).contains(&state.coolant.liquid_density[i]),
                "liquid density {} g/cm3",
                state.coolant.liquid_density[i]
            );
            assert!(
                state.coolant.gas_density[i] > 0.0 && state.coolant.gas_density[i] < 0.5,
                "gas density {} g/cm3",
                state.coolant.gas_density[i]
            );
            assert!(
                (0.3..3.0).contains(&state.coolant.prandtl[i]),
                "Pr = {}",
                state.coolant.prandtl[i]
            );
        }
    }
}
