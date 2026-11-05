//! The low-fidelity side: TUAS's `SimpleShellAndTubeHeatExchanger` with one
//! tube, which is a double pipe (tube fluid | tube wall | shell fluid |
//! outer shell), run counterflow to steady.
//!
//! **What is TUAS's and what is set here.** Every heat-transfer and friction
//! relation is TUAS's: the nodal conductances (Nusselt number to h, in
//! series with each half of the cylindrical wall,
//! `get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance`
//! and its shell-side twin), the wall's axial conduction (`SolidColumn`),
//! the advection, the Churchill friction factor. This module sets the
//! geometry and constant fluid properties from [`super::fixed`], picks the
//! correlations, applies θ, and marches TUAS's own time step to steady.
//!
//! **The uncalibrated correlations (θ nominal).** Tube side: TUAS's
//! `NusseltCorrelation::PipeGnielinskiGeneric` (laminar: Gnielinski's
//! simultaneously developing, constant-flux mean Nu at the tube's L/D).
//! Shell side: the same correlation on the shell's hydraulic diameter, which
//! is how the TUAS shell-and-tube treats any shell; **no annulus Nusselt
//! correlation exists in TUAS or anywhere else in the workspace** (searched
//! 2026-11-03, V&V record). Friction: TUAS's pipe loss (Churchill) on each
//! side's hydraulic diameter, smooth walls. The outer shell is adiabatic
//! (parasitic Nu 0, no insulation, zero ambient h).
//!
//! **θ** is applied without touching TUAS: [`evaluate`] evaluates the TUAS
//! correlation itself, with the same arguments the component passes it,
//! multiplies it, folds the fouling resistance into the tube side (exact
//! for a series path), and hands the component the result as
//! `NusseltCorrelation::FixedNusselt`. At θ nominal this is the same model
//! as giving the component the correlation directly; the test
//! `nominal_theta_is_tuas_own_correlation_path` pins that.
//!
//! **Wall conductivity levels.** TUAS custom materials carry `fn` pointers,
//! which cannot capture a run-time conductivity, so the wall takes k_wall
//! from [`WALL_K_LEVELS`]; any other value is an error. The design grid
//! uses 1, 10 and 100; 16 is the stage-2 demo wall.
//!
//! **Axial resolution.** TUAS nodes (inner + its front and back nodes, all
//! of length L/N) = the CFD's layer count N = round(375 L), so both models
//! carry the same first-order axial discretisation (V&V record, "1-D mesh").
use tuas_boussinesq_solver::array_fluid_collections::fluid_array_lateral_coupling::fluid_component_calculation::DimensionlessDarcyLossCorrelations;
use tuas_boussinesq_solver::array_fluid_collections::fluid_array_lateral_coupling::FluidArray;
use tuas_boussinesq_solver::boundary_conditions::BCType;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::{LiquidMaterial, SolidMaterial};
use tuas_boussinesq_solver::heat_transfer_correlations::heat_transfer_interactions::heat_transfer_interaction_enums::HeatTransferInteractionType;
use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;
use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::input_structs::GnielinskiData;
use tuas_boussinesq_solver::pre_built_components::heat_transfer_entities::HeatTransferEntity;
use tuas_boussinesq_solver::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger;
use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Angle, Area, DynamicViscosity, HeatTransfer, Length, MassDensity, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity, ThermalConductance, ThermalConductivity, ThermodynamicTemperature, Time};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{atmosphere, pascal};
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::closure::Closures;
use super::{fixed, fouling_to_si, DoublePipeInput, DoublePipeOutput, Theta};

/// The wall conductivities the 1-D model can represent \[W/(m K)\] (module
/// docs, "Wall conductivity levels").
pub const WALL_K_LEVELS: [f64; 4] = [1.0, 10.0, 16.0, 100.0];

fn fluid_cp(_: ThermodynamicTemperature) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(fixed::CP)
}
fn fluid_k(_: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(fixed::K_FLUID)
}
fn fluid_mu(_: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pascal_second>(fixed::NU * fixed::RHO)
}
fn fluid_rho(_: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(fixed::RHO)
}
fn wall_cp(_: ThermodynamicTemperature) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(fixed::CP_WALL)
}
fn wall_rho(_: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(fixed::RHO_WALL)
}
fn wall_k_1(_: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(1.0)
}
fn wall_k_10(_: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(10.0)
}
fn wall_k_16(_: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(16.0)
}
fn wall_k_100(_: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(100.0)
}

fn bounds() -> (ThermodynamicTemperature, ThermodynamicTemperature) {
    (ThermodynamicTemperature::new::<kelvin>(250.0), ThermodynamicTemperature::new::<kelvin>(450.0))
}

/// The constant-property water-like liquid of [`super::fixed`].
#[must_use]
pub fn liquid() -> LiquidMaterial {
    LiquidMaterial::CustomLiquid(bounds(), fluid_cp, fluid_k, fluid_mu, fluid_rho)
}

/// The wall material at `k_wall`, if it is one of [`WALL_K_LEVELS`]
/// (smooth: zero roughness).
#[must_use]
pub fn wall_material(k_wall: ThermalConductivity) -> Option<SolidMaterial> {
    let k = k_wall.get::<watt_per_meter_kelvin>();
    let f: fn(ThermodynamicTemperature) -> ThermalConductivity = match k {
        _ if k == 1.0 => wall_k_1,
        _ if k == 10.0 => wall_k_10,
        _ if k == 16.0 => wall_k_16,
        _ if k == 100.0 => wall_k_100,
        _ => return None,
    };
    Some(SolidMaterial::CustomSolid(bounds(), wall_cp, f, wall_rho, Length::new::<meter>(0.0)))
}

/// The error of a 1-D run.
#[derive(Debug, Clone, PartialEq)]
pub struct OneDError(pub String);

impl std::fmt::Display for OneDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TUAS double pipe: {}", self.0)
    }
}

impl std::error::Error for OneDError {}

impl From<TuasLibError> for OneDError {
    fn from(e: TuasLibError) -> Self {
        OneDError(e.to_string())
    }
}

/// Which Nusselt numbers the component is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NusseltSpec {
    /// TUAS's `PipeGnielinskiGeneric` on both sides, given to the component
    /// directly (θ cannot act on this path; it exists to pin [`evaluate`]).
    TextbookDirect,
    /// `FixedNusselt` on each side (hot, cold).
    Fixed(Ratio, Ratio),
}

/// Which friction law each side's `FluidArray` carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LossSpec {
    /// TUAS's pipe loss (Churchill) on the side's hydraulic diameter, scaled
    /// by the two multipliers (hot, cold).
    Textbook(Ratio, Ratio),
    /// `SimpleReynoldsPower(0, B, −1)`: `f L/D + K = B / Re` (hot B, cold B).
    ReynoldsInverse(Ratio, Ratio),
}

/// A 1-D run in detail.
#[derive(Debug, Clone, PartialEq)]
pub struct OneDRun {
    /// The outputs.
    pub output: DoublePipeOutput,
    /// Time steps to steady.
    pub steps: usize,
    /// Nodes (= the CFD's layers).
    pub nodes: usize,
    /// Nodal conductances: tube fluid → wall, wall → shell fluid.
    pub nodal_conductance: [ThermalConductance; 2],
}

/// Steady: no fluid or wall node changing faster than this \[K/s\]
/// (`max |ΔT| / Δt` over a step; with the slowest time constant here,
/// a few hundred seconds, the remaining drift is below 1e-7 K).
pub const STEADY_K_PER_S: f64 = 1.0e-10;
/// Courant number of the faster stream's advection. TUAS's fluid arrays
/// advect **explicitly** (`advance_timestep_for_parallel_tube_side_fluid_array_bundle`:
/// "− m_flow h_fluid(T_old) + m_flow h_fluid(adjacent T_old)"), so the step
/// must keep a node's through-flow below its heat capacity; Δt = 2 s
/// diverged at Re_hot 30, L 0.1 (Courant ≈ 2.3). The steady state of the
/// discrete equations does not depend on Δt.
pub const COURANT: f64 = 0.5;
/// Step cap.
pub const MAX_STEPS: usize = 400_000;

/// The march's time step at `x`: [`COURANT`] times the shorter node
/// residence time `ρ c_p A (L/N) / (ρ c_p Q)` of the two streams.
#[must_use]
pub fn time_step(x: &DoublePipeInput) -> Time {
    let l = x.length.get::<meter>();
    let n = fixed::layers(l) as f64;
    let (qh, qc) = x.flow_rates();
    let (ah, ac) = fixed::areas();
    Time::new::<second>(COURANT * (ah * l / n / qh).min(ac * l / n / qc))
}

fn gnielinski(l_over_d: f64) -> NusseltCorrelation {
    NusseltCorrelation::PipeGnielinskiGeneric(GnielinskiData {
        reynolds: Ratio::new::<ratio>(1.0),
        prandtl_bulk: Ratio::new::<ratio>(1.0),
        prandtl_wall: Ratio::new::<ratio>(1.0),
        darcy_friction_factor: Ratio::new::<ratio>(0.0),
        length_to_diameter: Ratio::new::<ratio>(l_over_d),
    })
}

/// The component at `x` with the given Nusselt numbers, at its initial
/// state (tube at the hot inlet temperature, shell at the cold).
///
/// # Errors
/// `k_wall` not in [`WALL_K_LEVELS`].
pub fn build(x: &DoublePipeInput, nu: NusseltSpec) -> Result<SimpleShellAndTubeHeatExchanger, OneDError> {
    let wall = wall_material(x.k_wall).ok_or_else(|| OneDError(format!("k_wall {:?} not in WALL_K_LEVELS {WALL_K_LEVELS:?}", x.k_wall)))?;
    let l = x.length.get::<meter>();
    let nodes = fixed::layers(l);
    let m = Length::new::<meter>;
    let p = Pressure::new::<atmosphere>(1.0);
    let (a_tube, a_shell) = fixed::areas();
    let (d_tube, d_shell) = (fixed::d_hot(), fixed::d_cold());
    let (nu_t, nu_s) = match nu {
        NusseltSpec::TextbookDirect => (gnielinski(l / d_tube), gnielinski(l / d_shell)),
        NusseltSpec::Fixed(h, c) => (NusseltCorrelation::FixedNusselt(h), NusseltCorrelation::FixedNusselt(c)),
    };
    let zero = Ratio::new::<ratio>(0.0);
    let mut s = SimpleShellAndTubeHeatExchanger::new_custom_circular_single_pass_sthe_with_insulation(
        1,
        nodes - 2,
        p,
        p,
        m(2.0 * fixed::R_TUBE_OUTER),
        m(2.0 * fixed::R_TUBE_INNER),
        m(d_tube),
        Area::new::<square_meter>(a_tube),
        // Outer shell: 1 mm thick, adiabatic (it carries no heat; its
        // thickness only sets a heat capacity the steady state ignores).
        m(2.0 * fixed::R_SHELL + 0.002),
        m(2.0 * fixed::R_SHELL),
        m(d_shell),
        Area::new::<square_meter>(a_shell),
        x.length,
        zero,
        zero,
        m(0.001),
        Angle::new::<degree>(0.0),
        Angle::new::<degree>(0.0),
        liquid(),
        liquid(),
        wall,
        wall,
        wall,
        ThermodynamicTemperature::new::<kelvin>(fixed::T_COLD_IN),
        HeatTransfer::new::<watt_per_square_meter_kelvin>(0.0),
        ThermodynamicTemperature::new::<kelvin>(fixed::T_HOT_IN),
        ThermodynamicTemperature::new::<kelvin>(fixed::T_COLD_IN),
        DimensionlessDarcyLossCorrelations::new_pipe(x.length, m(0.0), m(d_shell), zero),
        DimensionlessDarcyLossCorrelations::new_pipe(x.length, m(0.0), m(d_tube), zero),
        nu_t,
        nu_s,
        NusseltCorrelation::FixedNusselt(zero),
    );
    s.heat_exchanger_has_insulation = false;
    Ok(s)
}

/// Mass flow rates (hot +, cold −: counterflow) \[kg/s\].
fn mass_flows(x: &DoublePipeInput) -> (MassRate, MassRate) {
    let (qh, qc) = x.flow_rates();
    (MassRate::new::<kilogram_per_second>(fixed::RHO * qh), MassRate::new::<kilogram_per_second>(-fixed::RHO * qc))
}

/// TUAS's textbook Nusselt numbers at `x` (hot, cold), evaluated with the
/// arguments the component itself passes its correlations: Re = ṁ D / (A μ)
/// on the side's hydraulic diameter and flow area, the bulk Prandtl number
/// (also as the wall's: no wall correction), the modified friction factor
/// `(f L/D + K) / (L/D)` of the side's loss correlation.
///
/// # Errors
/// TUAS property or correlation errors.
pub fn textbook_nusselt(x: &DoublePipeInput) -> Result<(Ratio, Ratio), OneDError> {
    let l = x.length.get::<meter>();
    let (mh, mc) = mass_flows(x);
    let mu = fluid_mu(ThermodynamicTemperature::new::<kelvin>(fixed::T_HOT_IN));
    let pr = liquid().try_get_prandtl_liquid(ThermodynamicTemperature::new::<kelvin>(0.5 * (fixed::T_HOT_IN + fixed::T_COLD_IN)), Pressure::new::<atmosphere>(1.0))?;
    let (a_tube, a_shell) = fixed::areas();
    let side = |m: MassRate, a: f64, d: f64| -> Result<(Ratio, Ratio), OneDError> {
        let re: Ratio = (m / Area::new::<square_meter>(a) * Length::new::<meter>(d) / mu).abs();
        let loss = DimensionlessDarcyLossCorrelations::new_pipe(x.length, Length::new::<meter>(0.0), Length::new::<meter>(d), Ratio::new::<ratio>(0.0));
        let fmod = loss.fldk_based_on_darcy_friction_factor(re)? / Ratio::new::<ratio>(l / d);
        Ok((re, fmod))
    };
    let (re_t, f_t) = side(mh, a_tube, fixed::d_hot())?;
    let (re_s, f_s) = side(mc, a_shell, fixed::d_cold())?;
    let nu_t = gnielinski(l / fixed::d_hot()).estimate_based_on_prandtl_darcy_and_reynolds_wall_correction(pr, pr, f_t, re_t)?;
    let nu_s = gnielinski(l / fixed::d_cold()).estimate_based_on_prandtl_darcy_and_reynolds_no_wall_correction(pr, f_s, re_s)?;
    Ok((nu_t, nu_s))
}

/// Fold an area-specific resistance `r_f` \[m² K/W\] on the tube's outer
/// area into the tube side's Nusselt number: in series per unit length,
/// `1/(h' π D_i) = 1/(h π D_i) + r_f / (π D_o)`, so
/// `Nu' = 1 / (1/Nu + r_f k / D_o)`. Exact for the series path TUAS builds
/// (convection, then the wall halves, then the shell's convection).
#[must_use]
pub fn fold_fouling(nu_hot: f64, r_f: f64) -> f64 {
    1.0 / (1.0 / nu_hot + r_f * fixed::K_FLUID / (2.0 * fixed::R_TUBE_OUTER))
}

/// Run the component to steady.
///
/// # Errors
/// [`build`]'s, TUAS errors, or no steady state within [`MAX_STEPS`].
pub fn run(x: &DoublePipeInput, nu: NusseltSpec, loss: LossSpec) -> Result<OneDRun, OneDError> {
    let mut s = build(x, nu)?;
    let (mh, mc) = mass_flows(x);
    let rho = fluid_rho(ThermodynamicTemperature::new::<kelvin>(fixed::T_HOT_IN));
    let mut hot_in: HeatTransferEntity = BCType::new_const_temperature(ThermodynamicTemperature::new::<kelvin>(fixed::T_HOT_IN)).into();
    let mut cold_in: HeatTransferEntity = BCType::new_const_temperature(ThermodynamicTemperature::new::<kelvin>(fixed::T_COLD_IN)).into();
    let mut outlet: HeatTransferEntity = BCType::new_adiabatic_bc().into();
    let dt = time_step(x);
    let snapshot = |s: &mut SimpleShellAndTubeHeatExchanger| -> Result<Vec<f64>, OneDError> {
        let mut v: Vec<f64> = Vec::new();
        for t in s.inner_tube_fluid_array_temperature()?.iter().chain(s.shell_side_fluid_array_temperature()?.iter()).chain(s.inner_pipe_shell_temperature()?.iter()) {
            v.push(t.get::<kelvin>());
        }
        Ok(v)
    };
    let mut prev = snapshot(&mut s)?;
    let mut steps = 0;
    loop {
        let adv = |m: MassRate| HeatTransferInteractionType::new_advection_interaction(m, rho, rho);
        s.tube_side_fluid_array_for_single_tube.link_to_back(&mut hot_in, adv(mh))?;
        s.tube_side_fluid_array_for_single_tube.link_to_front(&mut outlet, adv(mh))?;
        s.shell_side_fluid_array.link_to_front(&mut cold_in, adv(mc))?;
        s.shell_side_fluid_array.link_to_back(&mut outlet, adv(mc))?;
        s.lateral_and_miscellaneous_connections(false, mh, mc)?;
        s.advance_timestep(dt)?;
        steps += 1;
        let now = snapshot(&mut s)?;
        let change = now.iter().zip(&prev).fold(0.0_f64, |a, (b, c)| a.max((b - c).abs()));
        if !change.is_finite() {
            return Err(OneDError(format!("diverged at step {steps}")));
        }
        prev = now;
        if change / dt.get::<second>() < STEADY_K_PER_S {
            break;
        }
        if steps >= MAX_STEPS {
            return Err(OneDError(format!("not steady after {steps} steps (last change {change:.3e} K)")));
        }
    }
    let tube = s.inner_tube_fluid_array_temperature()?;
    let shell = s.shell_side_fluid_array_temperature()?;
    let t_hot_out = *tube.last().expect("nodes");
    let t_cold_out = *shell.first().expect("nodes");
    let g_t = s.get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance(false)?;
    let g_s = s.get_shell_side_fluid_to_single_inner_pipe_shell_nodal_conductance(false)?;
    let nodes = fixed::layers(x.length.get::<meter>());
    let ua = (nodes as f64) / (1.0 / g_t.get::<watt_per_kelvin>() + 1.0 / g_s.get::<watt_per_kelvin>());
    let (ch, cc) = x.capacity_rates();
    let q = ch * (fixed::T_HOT_IN - t_hot_out.get::<kelvin>());
    let eps = q / (ch.min(cc) * (fixed::T_HOT_IN - fixed::T_COLD_IN));
    // Pressure drop, through each side's FluidArray (TUAS's Bejan relation).
    let dp = |e: &HeatTransferEntity, m: MassRate, law: Option<Ratio>, mult: Ratio| -> Result<f64, OneDError> {
        let mut fa: FluidArray = e.clone().try_into()?;
        if let Some(b) = law {
            fa.fluid_component_loss_properties = DimensionlessDarcyLossCorrelations::new_simple_reynolds_power_component(Ratio::new::<ratio>(0.0), b, -1.0);
        }
        Ok(fa.get_pressure_loss_immutable(m.abs()).get::<pascal>().abs() * mult.get::<ratio>())
    };
    let one = Ratio::new::<ratio>(1.0);
    let (dp_hot, dp_cold) = match loss {
        LossSpec::Textbook(fh, fc) => (dp(&s.tube_side_fluid_array_for_single_tube, mh, None, fh)?, dp(&s.shell_side_fluid_array, mc, None, fc)?),
        LossSpec::ReynoldsInverse(bh, bc) => (dp(&s.tube_side_fluid_array_for_single_tube, mh, Some(bh), one)?, dp(&s.shell_side_fluid_array, mc, Some(bc), one)?),
    };
    let (nu_hot, nu_cold) = match nu {
        NusseltSpec::Fixed(h, c) => (h, c),
        NusseltSpec::TextbookDirect => textbook_nusselt(x)?,
    };
    Ok(OneDRun {
        output: DoublePipeOutput {
            ua: ThermalConductance::new::<watt_per_kelvin>(ua),
            q: Power::new::<watt>(q),
            effectiveness: Ratio::new::<ratio>(eps),
            t_hot_out,
            t_cold_out,
            dp_hot: Pressure::new::<pascal>(dp_hot),
            dp_cold: Pressure::new::<pascal>(dp_cold),
            nu_hot,
            nu_cold,
        },
        steps,
        nodes,
        nodal_conductance: [g_t, g_s],
    })
}

/// The 1-D model as a function of (x, θ), the rung-3 interface: TUAS's
/// textbook correlations scaled by θ (module docs). `Theta::nominal()` is
/// the uncalibrated model.
///
/// # Errors
/// [`run`]'s; `k_wall` must be one of [`WALL_K_LEVELS`].
pub fn try_evaluate(x: &DoublePipeInput, theta: &Theta) -> Result<DoublePipeOutput, OneDError> {
    let (nt, ns) = textbook_nusselt(x)?;
    let nu_hot = fold_fouling(nt.get::<ratio>() * theta.nu_mult_hot.get::<ratio>(), fouling_to_si(theta.wall_resistance));
    let nu_cold = ns * theta.nu_mult_cold;
    Ok(run(x, NusseltSpec::Fixed(Ratio::new::<ratio>(nu_hot), nu_cold), LossSpec::Textbook(theta.friction_mult_hot, theta.friction_mult_cold))?.output)
}

/// The 1-D model's UA at (x, θ) **without the time march**: the component
/// is built with θ's Nusselt numbers ([`try_evaluate`]'s folding), its
/// lateral connections made once at the initial state, and UA read off
/// TUAS's two nodal conductances by [`run`]'s formula. With constant fluid
/// and wall properties the conductances do not depend on temperature, so
/// this is the march's UA (pinned bit for bit by
/// `march_free_ua_is_the_marched_ua`). Microseconds instead of seconds,
/// which is what makes gh:#1180's simulator design affordable.
///
/// # Errors
/// [`build`]'s or TUAS errors.
pub fn ua(x: &DoublePipeInput, theta: &Theta) -> Result<ThermalConductance, OneDError> {
    let (nt, ns) = textbook_nusselt(x)?;
    let nu_hot = fold_fouling(nt.get::<ratio>() * theta.nu_mult_hot.get::<ratio>(), fouling_to_si(theta.wall_resistance));
    let nu_cold = ns * theta.nu_mult_cold;
    ua_with_nusselt(x, Ratio::new::<ratio>(nu_hot), nu_cold)
}

/// [`ua`] for given tube- and annulus-side Nusselt numbers (any fouling
/// already folded into `nu_hot`, [`fold_fouling`]): TUAS's two nodal
/// conductances, march-free. gh:#1191's filtering pilot calls it with the
/// CFD's own Nusselt numbers. [`ua`] is this function after its θ folding,
/// so it is unchanged bit for bit (`march_free_ua_is_the_marched_ua`).
///
/// # Errors
/// [`build`]'s or TUAS errors.
pub fn ua_with_nusselt(x: &DoublePipeInput, nu_hot: Ratio, nu_cold: Ratio) -> Result<ThermalConductance, OneDError> {
    let mut s = build(x, NusseltSpec::Fixed(nu_hot, nu_cold))?;
    let (mh, mc) = mass_flows(x);
    s.lateral_and_miscellaneous_connections(false, mh, mc)?;
    let g_t = s.get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance(false)?;
    let g_s = s.get_shell_side_fluid_to_single_inner_pipe_shell_nodal_conductance(false)?;
    let nodes = fixed::layers(x.length.get::<meter>());
    Ok(ThermalConductance::new::<watt_per_kelvin>((nodes as f64) / (1.0 / g_t.get::<watt_per_kelvin>() + 1.0 / g_s.get::<watt_per_kelvin>())))
}

/// [`try_evaluate`], for callers that hold x on the design grid.
///
/// # Panics
/// If [`try_evaluate`] fails: `k_wall` not in [`WALL_K_LEVELS`], or a TUAS
/// error. Justified: the rung-3 signature returns an output, and both
/// failures are a caller passing an x the model was never built for.
#[must_use]
pub fn evaluate(x: &DoublePipeInput, theta: &Theta) -> DoublePipeOutput {
    match try_evaluate(x, theta) {
        Ok(o) => o,
        Err(e) => panic!("{e}"),
    }
}

/// The 1-D model with closures read off the CFD (step (b) of gh:#1178).
///
/// # Errors
/// [`run`]'s.
pub fn evaluate_with_closures(x: &DoublePipeInput, c: &Closures) -> Result<DoublePipeOutput, OneDError> {
    Ok(run(x, NusseltSpec::Fixed(c.nu_hot, c.nu_cold), LossSpec::ReynoldsInverse(c.loss_hot, c.loss_cold))?.output)
}

/// ε-NTU (dwsim-libs, counter-current) at an output's UA and `x`'s capacity
/// rates.
#[must_use]
pub fn eps_ntu(x: &DoublePipeInput, ua: ThermalConductance) -> f64 {
    use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
    use outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness;
    use uom::si::f64::TemperatureInterval;
    let (ch, cc) = x.capacity_rates();
    ntu_effectiveness::evaluate(
        FlowArrangement::CounterCurrent,
        ua,
        ThermalConductance::new::<watt_per_kelvin>(ch),
        ThermalConductance::new::<watt_per_kelvin>(cc),
        TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(fixed::T_HOT_IN - fixed::T_COLD_IN),
    )
    .effectiveness
    .get::<ratio>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`ua`] against the marched [`try_evaluate`] at three x and three θ
    /// (nominal, and two with every heat-transfer component moved).
    /// **Prediction:** bit-identical (same conductances, same formula).
    /// **Result (2026-11-04):** bit-identical at all 9 (asserted with `==`).
    #[test]
    fn march_free_ua_is_the_marched_ua() {
        let r = |v: f64| Ratio::new::<ratio>(v);
        let mut a = Theta::nominal();
        a.nu_mult_hot = r(1.3);
        a.nu_mult_cold = r(0.7);
        a.wall_resistance = super::super::fouling_si(4.0e-4);
        let mut b = Theta::nominal();
        b.nu_mult_hot = r(0.8);
        b.nu_mult_cold = r(1.6);
        b.friction_mult_hot = r(2.0);
        for x in [DoublePipeInput::si(30.0, 15.0, 10.0, 0.1), DoublePipeInput::si(90.0, 5.0, 100.0, 0.1), DoublePipeInput::si(10.0, 45.0, 1.0, 0.1)] {
            for th in [Theta::nominal(), a, b] {
                assert_eq!(ua(&x, &th).unwrap(), try_evaluate(&x, &th).unwrap().ua, "{x:?} {th:?}");
            }
        }
        assert!(ua(&DoublePipeInput::si(30.0, 15.0, 3.0, 0.1), &Theta::nominal()).is_err());
    }

    /// θ nominal through `FixedNusselt` is the same model as giving TUAS
    /// its own `PipeGnielinskiGeneric` directly (module docs): the march,
    /// the outlet temperatures and UA agree to round-off. Also the fouling
    /// fold: TUAS's tube-side nodal conductance with the folded Nu equals
    /// the original in series with `r_f / (π D_o L_node)`.
    #[test]
    fn nominal_theta_is_tuas_own_correlation_path() {
        let x = DoublePipeInput::si(30.0, 15.0, 10.0, 0.1);
        let t0 = std::time::Instant::now();
        let direct = run(&x, NusseltSpec::TextbookDirect, LossSpec::Textbook(Ratio::new::<ratio>(1.0), Ratio::new::<ratio>(1.0))).unwrap();
        let via = try_evaluate(&x, &Theta::nominal()).unwrap();
        println!("direct {:?} steps {} ({:.2} s)\nvia    {:?}", direct.output, direct.steps, t0.elapsed().as_secs_f64(), via);
        let rel = |a: f64, b: f64| (a / b - 1.0).abs();
        assert!(rel(direct.output.ua.get::<watt_per_kelvin>(), via.ua.get::<watt_per_kelvin>()) < 1e-12);
        assert!((direct.output.t_hot_out.get::<kelvin>() - via.t_hot_out.get::<kelvin>()).abs() < 1e-9);
        assert!((direct.output.t_cold_out.get::<kelvin>() - via.t_cold_out.get::<kelvin>()).abs() < 1e-9);
        assert!(rel(direct.output.dp_hot.get::<pascal>(), via.dp_hot.get::<pascal>()) < 1e-12);
        assert_eq!(direct.output.nu_hot, via.nu_hot);
        // Physical bounds: heat flows hot to cold, outlets inside the inlets.
        let (th, tc) = (via.t_hot_out.get::<kelvin>(), via.t_cold_out.get::<kelvin>());
        assert!(th < fixed::T_HOT_IN && th > fixed::T_COLD_IN && tc > fixed::T_COLD_IN && tc < fixed::T_HOT_IN);
        let e = via.effectiveness.get::<ratio>();
        assert!(e > 0.0 && e < 1.0);
        // Energy: hot loss equals cold gain.
        let (ch, cc) = x.capacity_rates();
        assert!(rel(ch * (fixed::T_HOT_IN - th), cc * (tc - fixed::T_COLD_IN)) < 1e-6, "{} {}", ch * (fixed::T_HOT_IN - th), cc * (tc - fixed::T_COLD_IN));
        // epsilon-NTU at the model's own UA is close (same layer error only).
        let en = eps_ntu(&x, via.ua);
        assert!(e <= en + 1e-9 && e > 0.95 * en, "{e} vs {en}");

        // Fouling fold.
        let r_f = 2.0e-3;
        let nu = 5.0;
        let mut a = build(&x, NusseltSpec::Fixed(Ratio::new::<ratio>(nu), Ratio::new::<ratio>(6.0))).unwrap();
        let mut b = build(&x, NusseltSpec::Fixed(Ratio::new::<ratio>(fold_fouling(nu, r_f)), Ratio::new::<ratio>(6.0))).unwrap();
        let (mh, mc) = mass_flows(&x);
        a.lateral_and_miscellaneous_connections(false, mh, mc).unwrap();
        b.lateral_and_miscellaneous_connections(false, mh, mc).unwrap();
        let ga = a.get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance(false).unwrap().get::<watt_per_kelvin>();
        let gb = b.get_single_tube_side_fluid_array_node_to_inner_pipe_shell_nodal_conductance(false).unwrap().get::<watt_per_kelvin>();
        let node = 0.1 / fixed::layers(0.1) as f64;
        let expect = 1.0 / (1.0 / ga + r_f / (std::f64::consts::PI * 2.0 * fixed::R_TUBE_OUTER * node));
        assert!(rel(gb, expect) < 1e-12, "{gb} vs {expect}");
        // Unsupported wall conductivity is refused, and evaluate's theta acts.
        assert!(try_evaluate(&DoublePipeInput::si(30.0, 15.0, 3.0, 0.1), &Theta::nominal()).is_err());
        assert!(wall_material(ThermalConductivity::new::<watt_per_meter_kelvin>(16.0)).is_some());
        let mut th2 = Theta::nominal();
        th2.nu_mult_cold = Ratio::new::<ratio>(1.3);
        th2.friction_mult_hot = Ratio::new::<ratio>(2.0);
        let y2 = evaluate(&x, &th2);
        assert!(y2.ua > via.ua && y2.q > via.q);
        assert!(rel(y2.dp_hot.get::<pascal>(), 2.0 * via.dp_hot.get::<pascal>()) < 1e-12);
        assert!(!OneDError("x".into()).to_string().is_empty());
    }
}
