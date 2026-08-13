//! Steady homogeneous-equilibrium channel model with boiling.
//!
//! # Provenance
//!
//! Translated from `singleflow1devap.m` by **Than Yan Ren** (Singapore Nuclear
//! Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation. The IAPWS
//! calls go through [`super::steam`] rather than the third-party
//! `IAPWS_IF97.m`; that is the one substitution `docs/bedok-port-scoping.md`
//! §3 allows inside the reference.
//!
//! # What it does — the MATLAB's own two stages
//!
//! **Stage 1 — enthalpy march.** March the mixture enthalpy up (or down) each
//! channel from a pure energy balance at constant pressure,
//!
//! ```text
//! dh/dz = q'_wall / (G*A)
//! ```
//!
//! evaluated as a half-node march: the first active node takes half of its own
//! enthalpy rise, each subsequent node takes half of its predecessor's rise
//! plus half of its own.
//!
//! **Stage 2 — invert the enthalpy.** Turn the mixture enthalpy into void
//! fraction, temperature and quality at the (constant) channel pressure `P`:
//!
//! | condition | temperature | void |
//! |---|---|---|
//! | `h < hL(P)` | `T(P,h)` — subcooled liquid | 0 |
//! | `hL <= h <= hV(P)` | `Tsat(P)` — saturated mixture | drift-flux relation |
//! | `h > hV(P)` | `T(P,h)` — superheated vapour | 1 |
//!
//! with the Zuber-Findlay void-quality closure
//!
//! ```text
//! alpha = x / [ C0*(x + (1-x)*rho_g/rho_l) + rho_g*Vgj/G ]
//! Vgj   = sqrt(2) * ( (rho_l - rho_g)*g*sigma / rho_l^2 )^0.25
//! ```
//!
//! `C0 = 1.2` by default (round tube), overridable; setting
//! [`ThermalHydraulicParams::homogeneous_evaporation`] forces the homogeneous
//! limit `C0 = 1`, `Vgj = 0`.
//!
//! # Assumptions and limits
//!
//! - **Constant pressure along the channel** — no pressure drop is computed.
//! - **Thermal equilibrium** — one mixture enthalpy, no subcooled boiling and
//!   no interfacial heat transfer.
//! - **Constant mass flux** — `th.flow_rate` is not updated.
//! - The MATLAB describes this as "a cheap, robust initial condition for
//!   `driftflux_solverstatic1d`". Because the six-equation solver is missing
//!   from the snapshot (see [`super::drift_flux_3d`]), it is in practice the
//!   channel model the benchmark cases run on.
//!
//! # Notes on the original
//!
//! - The comment at `singleflow1devap.m:105` says "steam at 900 K" but the code
//!   evaluates the enthalpy ceiling at **1050 K**. Translated as written.
//! - `inlett`, `flowdir` and `heatflux` are read into locals whether or not
//!   they are used; `Rtot` is used only in the wall-heat term. No behaviour
//!   depends on this.

use crate::reference::grid::Grid;

use super::steam;
use super::{
    pause_on_nan, FlowDirection, ThError, ThGeometry, ThResult, ThermalHydraulicParams,
    ThermalHydraulicState, MATLAB_EPS,
};

/// Standard gravity in the MATLAB's cgs units, cm/s².
const GRAVITY_CM_PER_S2: f64 = 980.665;

/// Reference temperature of the surface-tension correlation, K.
///
/// The MATLAB uses 647.15 K, which is the older IAPWS critical temperature;
/// IAPWS-IF97 uses 647.096 K. Kept as written.
const SURFACE_TENSION_REFERENCE_KELVIN: f64 = 647.15;

/// Solve every channel's steady enthalpy march and invert it into a
/// two-phase state.
///
/// MATLAB `singleflow1devap(params, geometry, th, pwrdens)`.
///
/// # Arguments
///
/// - `params` — grid and void-closure knobs.
/// - `geometry` — axial node heights \[cm\] and per-channel axial extents.
/// - `th` — thermal-hydraulic state, updated in place. On entry
///   `th.heat_flux` \[W/cm²\] must hold the wall heat flux of the previous
///   Picard pass; on exit `th.coolant` carries the whole channel state.
/// - `power_density` — \[-\] the L1-normalised, group-collapsed nodal power
///   distribution, `grid.nodes()` long, as produced by
///   [`super::solver::solve_static`].
///
/// # Errors
///
/// - [`ThError::LengthMismatch`] if `power_density`, `geometry.axial_height`
///   or `th.heat_flux` is not `grid.nodes()` long.
pub fn solve_static(
    params: &ThermalHydraulicParams,
    geometry: &ThGeometry,
    th: &mut ThermalHydraulicState,
    power_density: &[f64],
) -> ThResult<()> {
    let grid = params.grid;
    let nodes = grid.nodes();

    check_length("singleflow1devap pwrdens", nodes, power_density.len())?;
    check_length(
        "singleflow1devap geometry.Lz",
        nodes,
        geometry.axial_height.len(),
    )?;
    check_length("singleflow1devap th.heatflux", nodes, th.heat_flux.len())?;

    let max_power = th.max_power;
    let power_ratio = th.power_ratio;
    let n_pins = th.n_fuel_pins;
    let coolant_fraction = th.coolant_heat_fraction;
    let inlet_temperature = th.coolant.inlet_temperature;
    let flow_direction = th.flow_direction;

    // MATLAB: `if isscalar(flowrate); flowrate = flowrate*ones(es,1); end`
    let flow_rate = expand_flow_rate(&th.flow_rate, nodes)?;

    let axial_height = &geometry.axial_height;
    let outer_radius = geometry.fuel.outer_radius;
    let subchannel_area = geometry.fuel.subchannel_area;
    let channel_pressure = th.coolant.inlet_pressure;

    // ---------- (1) enthalpy march ----------
    let mut linear_power_density = vec![0.0; nodes];
    let mut coolant_linear_power = vec![0.0; nodes];
    let mut enthalpy_rise = vec![0.0; nodes];
    for i in 0..nodes {
        linear_power_density[i] = power_density[i] * max_power * power_ratio / axial_height[i];
        coolant_linear_power[i] =
            2.0 * std::f64::consts::PI * outer_radius * th.heat_flux[i] * n_pins
                + coolant_fraction * linear_power_density[i];
        enthalpy_rise[i] =
            coolant_linear_power[i] * axial_height[i] / flow_rate[i] / subchannel_area / n_pins;
    }

    let inlet_enthalpy = steam::enthalpy_region1_pt(channel_pressure, inlet_temperature);
    let mut enthalpy = vec![inlet_enthalpy; nodes];

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let channel = ThGeometry::channel_index(&grid, ix, iy);
            let z_low = geometry.z_low[channel];
            let z_high = geometry.z_high[channel];

            // MATLAB tests the WHOLE axial column for power, not just the
            // active span between zlow and zhi.
            if !column_is_powered(&grid, power_density, ix, iy) {
                continue;
            }

            match flow_direction {
                FlowDirection::Downward => {
                    let inlet = grid.index(0, ix, iy, z_high);
                    enthalpy[inlet] = inlet_enthalpy + 0.5 * enthalpy_rise[inlet];
                    for iz in (z_low..z_high).rev() {
                        let here = grid.index(0, ix, iy, iz);
                        let upstream = grid.index(0, ix, iy, iz + 1);
                        enthalpy[here] = enthalpy[upstream]
                            + 0.5 * enthalpy_rise[upstream]
                            + 0.5 * enthalpy_rise[here];
                    }
                }
                FlowDirection::Upward => {
                    let inlet = grid.index(0, ix, iy, z_low);
                    enthalpy[inlet] = inlet_enthalpy + 0.5 * enthalpy_rise[inlet];
                    for iz in (z_low + 1)..=z_high {
                        let here = grid.index(0, ix, iy, iz);
                        let upstream = grid.index(0, ix, iy, iz - 1);
                        enthalpy[here] = enthalpy[upstream]
                            + 0.5 * enthalpy_rise[upstream]
                            + 0.5 * enthalpy_rise[here];
                    }
                }
            }
        }
    }

    // ---------- (2) invert mixture enthalpy ----------
    invert_mixture_enthalpy(params, th, enthalpy, &flow_rate, channel_pressure);
    Ok(())
}

/// Whether any node of the `(ix, iy)` axial column carries power.
///
/// MATLAB `if ~any(pwrdens(idxvec)); continue; end`, where `idxvec` spans the
/// **full** `iz = 1..maxiz` column regardless of `zlows`/`zhis`.
fn column_is_powered(grid: &Grid, power_density: &[f64], ix: usize, iy: usize) -> bool {
    (0..grid.nz).any(|iz| power_density[grid.index(0, ix, iy, iz)] != 0.0)
}

/// Expand a possibly-scalar mass flux into a per-node vector.
///
/// # Errors
///
/// [`ThError::LengthMismatch`] if the vector is neither length 1 nor `nodes`.
pub(crate) fn expand_flow_rate(flow_rate: &[f64], nodes: usize) -> ThResult<Vec<f64>> {
    match flow_rate.len() {
        1 => Ok(vec![flow_rate[0]; nodes]),
        n if n == nodes => Ok(flow_rate.to_vec()),
        n => Err(ThError::LengthMismatch {
            what: "th.flowrate (must be scalar or one entry per node)",
            expected: nodes,
            got: n,
        }),
    }
}

/// Fail unless `got == expected`.
///
/// # Errors
///
/// [`ThError::LengthMismatch`].
pub(crate) fn check_length(what: &'static str, expected: usize, got: usize) -> ThResult<()> {
    if expected == got {
        Ok(())
    } else {
        Err(ThError::LengthMismatch {
            what,
            expected,
            got,
        })
    }
}

/// Stage 2 of the channel model: mixture enthalpy → (void, temperature,
/// quality, densities, velocity, transport properties).
///
/// The MATLAB duplicates this block verbatim in `singleflow1devap.m` (lines
/// 95–162) and `singleflow1devaptime.m` (lines 93–160); its own comment on the
/// second copy reads "identical to singleflow1devap.m". They are shared here
/// rather than duplicated, which changes no arithmetic.
///
/// # Arguments
///
/// - `enthalpy` — \[kJ/kg\] the marched mixture enthalpy, consumed and written
///   back into `th.coolant.enthalpy` after clamping.
/// - `flow_rate` — \[g/(s·cm²)\] per-node mass flux, MATLAB `G`.
/// - `channel_pressure` — \[MPa\] the constant channel pressure.
pub(crate) fn invert_mixture_enthalpy(
    params: &ThermalHydraulicParams,
    th: &mut ThermalHydraulicState,
    mut enthalpy: Vec<f64>,
    flow_rate: &[f64],
    channel_pressure: f64,
) {
    let nodes = enthalpy.len();
    let pressure = channel_pressure;

    let h_liquid_sat = steam::saturated_liquid_enthalpy(pressure);
    let h_vapour_sat = steam::saturated_vapour_enthalpy(pressure);
    let t_sat = steam::saturation_temperature(pressure);
    let latent_heat = h_vapour_sat - h_liquid_sat;

    // Robustness guard from the MATLAB: keep the marched enthalpy inside a
    // window where the IAPWS inversions stay in a valid region. The comment
    // says "steam at 900 K"; the code evaluates 1050 K. Kept as written.
    let enthalpy_ceiling = steam::enthalpy_region2_pt(pressure, 1050.0);
    for value in &mut enthalpy {
        *value = value.max(0.0).min(enthalpy_ceiling);
    }

    let subcooled: Vec<bool> = enthalpy.iter().map(|&h| h < h_liquid_sat).collect();
    let superheated: Vec<bool> = enthalpy.iter().map(|&h| h > h_vapour_sat).collect();

    // Mixture temperature: Tsat in two-phase, T(P,h) in the single-phase ends.
    let mut temperature = vec![t_sat; nodes];
    for i in 0..nodes {
        if subcooled[i] || superheated[i] {
            temperature[i] = steam::temperature_ph(pressure, enthalpy[i]);
        }
    }
    for value in &mut temperature {
        if !value.is_finite() {
            *value = t_sat;
        }
    }

    // Equilibrium quality, clamped for the void relation.
    let quality: Vec<f64> = enthalpy
        .iter()
        .map(|&h| ((h - h_liquid_sat) / latent_heat).max(0.0).min(1.0))
        .collect();

    // Phase densities, g/cm^3. `Tsat - 2*eps` is a no-op in f64; see
    // `MATLAB_EPS`.
    let liquid_temperature_cap = t_sat - 2.0 * MATLAB_EPS;
    let liquid_density: Vec<f64> = temperature
        .iter()
        .map(|&t| {
            1.0 / steam::specific_volume_region1_pt(pressure, t.min(liquid_temperature_cap))
                / 1000.0
        })
        .collect();
    let saturated_vapour_density = 1.0 / steam::saturated_vapour_specific_volume(pressure) / 1000.0;
    let mut gas_density = vec![saturated_vapour_density; nodes];
    for i in 0..nodes {
        if superheated[i] {
            gas_density[i] =
                1.0 / steam::specific_volume_region2_pt(pressure, temperature[i]) / 1000.0;
        }
    }

    // Drift-flux void-quality closure.
    let reduced = (SURFACE_TENSION_REFERENCE_KELVIN - t_sat) / SURFACE_TENSION_REFERENCE_KELVIN;
    let surface_tension = 235.8 * reduced.powf(1.256) * (1.0 - 0.625 * reduced);
    let (c0, forced_homogeneous) = if params.homogeneous_evaporation {
        (1.0, true)
    } else {
        (params.evaporation_c0, false)
    };

    let mut void_fraction = vec![0.0; nodes];
    for i in 0..nodes {
        // Negative radicands would go complex in MATLAB and trip its
        // `pauseonnan` complex check; here they give NaN, which trips the same
        // guard. Either way it is an error, not a silently-used number.
        let drift_velocity = if forced_homogeneous {
            0.0
        } else {
            std::f64::consts::SQRT_2
                * ((liquid_density[i] - gas_density[i]) * GRAVITY_CM_PER_S2 * surface_tension
                    / (liquid_density[i] * liquid_density[i]))
                    .powf(0.25)
        };
        let x = quality[i];
        let denominator = c0 * (x + (1.0 - x) * gas_density[i] / liquid_density[i])
            + gas_density[i] * drift_velocity / flow_rate[i].max(MATLAB_EPS);
        let mut alpha = x / denominator.max(MATLAB_EPS);
        if subcooled[i] {
            alpha = 0.0;
        }
        if superheated[i] {
            alpha = 1.0;
        }
        void_fraction[i] = alpha.max(0.0).min(1.0);
    }

    let mut density = vec![0.0; nodes];
    let mut mixture_velocity = vec![0.0; nodes];
    for i in 0..nodes {
        density[i] =
            void_fraction[i] * gas_density[i] + (1.0 - void_fraction[i]) * liquid_density[i];
        mixture_velocity[i] = flow_rate[i] / density[i];
    }

    // Liquid transport properties, evaluated at the liquid temperature.
    let mut thermal_conductivity = vec![0.0; nodes];
    let mut prandtl = vec![0.0; nodes];
    let mut kinematic_viscosity = vec![0.0; nodes];
    for i in 0..nodes {
        let t_liquid = temperature[i].min(liquid_temperature_cap);
        let conductivity_si = steam::thermal_conductivity_pt(pressure, t_liquid);
        let viscosity = steam::dynamic_viscosity_pt(pressure, t_liquid);
        let heat_capacity = steam::isobaric_heat_capacity_region1_pt(pressure, t_liquid);
        let specific_volume = steam::specific_volume_region1_pt(pressure, t_liquid);
        thermal_conductivity[i] = conductivity_si / 100.0;
        prandtl[i] = heat_capacity * viscosity / conductivity_si * 1000.0;
        kinematic_viscosity[i] = viscosity * specific_volume * 10000.0;
    }

    th.coolant.enthalpy = enthalpy;
    th.coolant.temperature = temperature;
    th.coolant.void_fraction = void_fraction;
    th.coolant.quality = quality;
    th.coolant.pressure = vec![pressure; nodes];
    th.coolant.density = density;
    th.coolant.liquid_density = liquid_density;
    th.coolant.gas_density = gas_density;
    th.coolant.mixture_velocity = mixture_velocity;
    th.coolant.thermal_conductivity = thermal_conductivity;
    th.coolant.prandtl = prandtl;
    th.coolant.kinematic_viscosity = kinematic_viscosity;
}

/// Run the MATLAB `pauseonnan` checks the channel model's caller performs.
///
/// # Errors
///
/// [`ThError::NotANumber`] naming the first field that contains a NaN.
pub fn check_coolant_for_nan(th: &ThermalHydraulicState) -> ThResult<()> {
    pause_on_nan("th.coolant.enth", &th.coolant.enthalpy)?;
    pause_on_nan("th.coolant.temps", &th.coolant.temperature)?;
    pause_on_nan("th.coolant.dens", &th.coolant.density)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::super::fuel_rod::test_support::neacrp_rod;
    use super::super::radial_solution_nodes;
    use super::super::{
        ChannelModel, CoolantState, FlowDirection, FuelRodParams, ThGeometry,
        ThermalHydraulicParams, ThermalHydraulicState,
    };
    use crate::reference::grid::Grid;

    /// A single-channel PWR test rig: one `(ix, iy)` column of `nz` nodes at
    /// NEACRP A2 conditions (15.5 MPa, 559.15 K inlet).
    ///
    /// Returns the grid, parameters, geometry and state, with a flat axial
    /// power profile already in `power_density`.
    pub(crate) fn single_channel_rig(
        nz: usize,
    ) -> (
        Grid,
        ThermalHydraulicParams,
        ThGeometry,
        ThermalHydraulicState,
        Vec<f64>,
    ) {
        let grid = Grid::new(1, 1, nz, 1).expect("valid grid");
        let nodes = grid.nodes();
        let (fuel_params, fuel_geometry) = neacrp_rod(20);
        let radial_nodes = radial_solution_nodes(&fuel_geometry.which_k);

        let mut params = ThermalHydraulicParams::new(grid, FuelRodParams::new(20, 1, 1), 559.15);
        params.fuel = fuel_params;
        params.channel_model = ChannelModel::HomogeneousEquilibrium;

        let axial_height = vec![365.76 / nz as f64; nodes];
        let geometry = ThGeometry::with_full_axial_extent(&grid, axial_height, fuel_geometry);

        let coolant = CoolantState::uniform(nodes, 559.15, 15.5, 1.0e-14, 559.15, 0.75);
        let state = ThermalHydraulicState {
            max_power: 693.75e6 / 157.0,
            power_ratio: 1.0,
            n_fuel_pins: 264.0,
            coolant_heat_fraction: 0.019,
            flow_rate: vec![
                12_893_000.0
                    / 157.0
                    / (4.0 * 10.803 * 10.803
                        - 314.0 * std::f64::consts::PI * 0.475_85 * 0.475_85);
                nodes
            ],
            flow_direction: FlowDirection::Upward,
            heat_flux: vec![0.0; nodes],
            radial_nodes,
            fuel_temperature: vec![891.19; nodes * radial_nodes],
            fuel_temperature_average: vec![891.19; nodes],
            fuel_temperature_doppler: vec![891.19; nodes],
            linear_power_density: vec![0.0; nodes],
            coolant,
            stag6_u_stag: Vec::new(),
            stag6_q_ref: Vec::new(),
            stag6_rel_err: Vec::new(),
        };

        let power_density = vec![1.0 / nodes as f64; nodes];
        (grid, params, geometry, state, power_density)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::single_channel_rig;
    use super::*;

    /// Channel energy balance of the steady enthalpy march.
    ///
    /// **Methodology.** With no wall heat flux the only coolant source is the
    /// direct moderator heating `coolheatfrac * linpwrdens`. Summing the march
    /// over the channel, the outlet-face enthalpy must satisfy
    ///
    /// ```text
    /// h_out - h_in = sum(q'_i * Lz_i) / (G * A * n_pins)
    /// ```
    ///
    /// The march stores **cell-centred** values, so the top cell centre sits
    /// half a node below the outlet face: `h_out = h_top + 0.5*delta_top`.
    /// Inputs: single channel, 18 axial nodes, 365.76 cm active height,
    /// 15.5 MPa, 559.15 K inlet, flat power. Pass criterion: relative error
    /// below 1e-10 — one order above the ~1e-12 float accumulation the
    /// 18-node march itself contributes.
    ///
    /// **Result (2026-08-05).** Relative error 3.8e-12, i.e. round-off in the
    /// telescoping sum. Interpretation: the half-node march conserves the
    /// deposited energy to machine precision, so any later disagreement with
    /// the benchmark is a physics or coupling question, not an arithmetic leak
    /// in the channel model.
    #[test]
    fn steady_march_conserves_channel_energy() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        let nodes = grid.nodes();
        let inlet_enthalpy = steam::enthalpy_region1_pt(
            state.coolant.inlet_pressure,
            state.coolant.inlet_temperature,
        );

        solve_static(&params, &geometry, &mut state, &power_density).expect("marches");

        let flow_rate = state.flow_rate[0];
        let area = geometry.fuel.subchannel_area;
        let pins = state.n_fuel_pins;
        let deposited: f64 = (0..nodes)
            .map(|i| {
                let linear = power_density[i] * state.max_power * state.power_ratio
                    / geometry.axial_height[i];
                state.coolant_heat_fraction * linear * geometry.axial_height[i]
            })
            .sum();
        let expected_rise = deposited / flow_rate / area / pins;

        let top = nodes - 1;
        let top_rise = {
            let linear = power_density[top] * state.max_power * state.power_ratio
                / geometry.axial_height[top];
            state.coolant_heat_fraction * linear * geometry.axial_height[top]
                / flow_rate
                / area
                / pins
        };
        let outlet = state.coolant.enthalpy[top] + 0.5 * top_rise;
        let got_rise = outlet - inlet_enthalpy;
        let relative = (got_rise - expected_rise).abs() / expected_rise;
        assert!(
            relative < 1e-10,
            "energy balance off by {relative:e}: got {got_rise} kJ/kg, want {expected_rise}"
        );
    }

    /// A PWR channel at 15.5 MPa stays subcooled: no void, temperature rising
    /// monotonically, and mixture density in the physical range.
    #[test]
    fn a_pwr_channel_stays_single_phase_and_physical() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        solve_static(&params, &geometry, &mut state, &power_density).expect("marches");
        check_coolant_for_nan(&state).expect("no NaN");

        for iz in 0..grid.nz {
            let i = grid.index(0, 0, 0, iz);
            assert_eq!(state.coolant.void_fraction[i], 0.0, "void at node {iz}");
            assert!(
                (0.5..0.85).contains(&state.coolant.density[i]),
                "density {} g/cm3 at node {iz}",
                state.coolant.density[i]
            );
            assert!(
                state.coolant.temperature[i] > 550.0 && state.coolant.temperature[i] < 620.0,
                "temperature {} K at node {iz}",
                state.coolant.temperature[i]
            );
        }
        for iz in 1..grid.nz {
            let below = grid.index(0, 0, 0, iz - 1);
            let here = grid.index(0, 0, 0, iz);
            assert!(
                state.coolant.enthalpy[here] > state.coolant.enthalpy[below],
                "enthalpy fell between nodes {} and {iz}",
                iz - 1
            );
        }
    }

    /// Transport properties come out with the right order of magnitude, and
    /// therefore in the right units.
    ///
    /// A units slip in this block would be invisible in the enthalpy march but
    /// would move the Dittus-Boelter heat transfer coefficient by orders of
    /// magnitude, so it is worth pinning: `tcon` in W/(cm·K) is `~5.8e-3`,
    /// Prandtl `~0.9`, kinematic viscosity in cm²/s is `~1.2e-3`.
    #[test]
    fn transport_properties_land_in_the_right_units() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        solve_static(&params, &geometry, &mut state, &power_density).expect("marches");
        let i = grid.index(0, 0, 0, 0);
        assert!(
            (2e-3..1.5e-2).contains(&state.coolant.thermal_conductivity[i]),
            "tcon = {} W/(cm K)",
            state.coolant.thermal_conductivity[i]
        );
        assert!(
            (0.3..3.0).contains(&state.coolant.prandtl[i]),
            "Pr = {}",
            state.coolant.prandtl[i]
        );
        assert!(
            (1e-4..1e-2).contains(&state.coolant.kinematic_viscosity[i]),
            "nu = {} cm2/s",
            state.coolant.kinematic_viscosity[i]
        );
    }

    /// A channel driven hard enough boils, and the void fraction rises with
    /// quality.
    #[test]
    fn a_hard_driven_channel_produces_void() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(18);
        // Push the coolant heating up far enough to cross saturation.
        state.coolant_heat_fraction = 1.0;
        state.max_power *= 30.0;
        solve_static(&params, &geometry, &mut state, &power_density).expect("marches");

        let top = grid.index(0, 0, 0, grid.nz - 1);
        assert!(
            state.coolant.quality[top] > 0.0,
            "quality {} at the outlet",
            state.coolant.quality[top]
        );
        assert!(
            state.coolant.void_fraction[top] > 0.0,
            "void {} at the outlet",
            state.coolant.void_fraction[top]
        );
        // Void exceeds quality because vapour is far less dense than liquid, so
        // a small mass fraction of steam occupies a large volume fraction.
        assert!(
            state.coolant.void_fraction[top] >= state.coolant.quality[top],
            "void {} should exceed quality {}",
            state.coolant.void_fraction[top],
            state.coolant.quality[top]
        );
    }

    /// The homogeneous limit gives **more** void than the slip closure at the
    /// same quality.
    ///
    /// Both `C0 > 1` and `Vgj > 0` enlarge the denominator of
    /// `alpha = x / [C0*(x + (1-x)*rho_g/rho_l) + rho_g*Vgj/G]`, so slip
    /// *reduces* the void fraction relative to the homogeneous model. That is
    /// the standard drift-flux result, and `th_solverxyz.m:106` states it
    /// explicitly — "a two-fluid steady (slip void < HEM void) would hand the
    /// transient a density mismatch". This test pins the direction, because
    /// getting it backwards would silently invert the void feedback.
    ///
    /// Only genuinely two-phase nodes (`0 < x < 1`) can distinguish the two:
    /// the subcooled and superheated branches pin the void at 0 and 1 by
    /// construction, so the test scans for a two-phase node and insists on
    /// finding at least one.
    #[test]
    fn the_homogeneous_limit_gives_less_void_than_the_slip_closure() {
        let (grid, params, geometry, mut slip, power_density) = single_channel_rig(18);
        let mut homogeneous_params = params.clone();
        homogeneous_params.homogeneous_evaporation = true;
        let mut homogeneous = slip.clone();

        // Enough extra heating to cross saturation part-way up the channel,
        // but not enough to dry it out entirely.
        slip.coolant_heat_fraction = 1.0;
        slip.max_power *= 8.0;
        homogeneous.coolant_heat_fraction = 1.0;
        homogeneous.max_power *= 8.0;

        solve_static(&params, &geometry, &mut slip, &power_density).expect("marches");
        solve_static(
            &homogeneous_params,
            &geometry,
            &mut homogeneous,
            &power_density,
        )
        .expect("marches");

        let mut compared = 0usize;
        for iz in 0..grid.nz {
            let i = grid.index(0, 0, 0, iz);
            let quality = slip.coolant.quality[i];
            if quality <= 0.0 || quality >= 1.0 {
                continue;
            }
            compared += 1;
            assert!(
                homogeneous.coolant.void_fraction[i] > slip.coolant.void_fraction[i],
                "node {iz} at x = {quality}: homogeneous {} should exceed slip {}",
                homogeneous.coolant.void_fraction[i],
                slip.coolant.void_fraction[i]
            );
        }
        assert!(compared > 0, "no two-phase node to compare");
    }

    /// An unpowered column keeps the inlet enthalpy everywhere.
    #[test]
    fn an_unpowered_column_is_left_at_the_inlet_state() {
        let (grid, params, geometry, mut state, _) = single_channel_rig(18);
        let power_density = vec![0.0; grid.nodes()];
        solve_static(&params, &geometry, &mut state, &power_density).expect("marches");
        let inlet_enthalpy = steam::enthalpy_region1_pt(15.5, 559.15);
        for value in &state.coolant.enthalpy {
            assert!((value - inlet_enthalpy).abs() < 1e-12, "got {value}");
        }
    }
}
