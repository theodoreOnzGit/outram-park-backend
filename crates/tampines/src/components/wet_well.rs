// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Wet well (pressure-suppression pool) component (gh:#1075; RELAP-7 Theory
//! Manual §6.12, Eqs 389–400, cited, not quoted).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! A 0-D suppression chamber: a **gas space** (nitrogen plus steam, ideal
//! gases at one temperature) above a **liquid pool** (well mixed, one
//! temperature), in a prismatic vessel of cross-section `A_c` and effective
//! height `L_t`. The pool level is a state: `L_w = m_w / (ρ_w A_c)` (the
//! inverse of RELAP-7 Eq 398), and the gas occupies `A_c (L_t − L_w)` (Eq
//! 397), so the gas is compressed as the pool rises.
//!
//! ## Ported, not written fresh
//!
//! **The physics is ported from `sembawang::bwr_mark1`** (the Fukushima
//! Unit 1 torus, MELCOR-based, clean room from the MELCOR 2.2 reference
//! manuals). `tampines` cannot depend on `sembawang` (it would pull the whole
//! source-term chain into the thermal-hydraulic framework, and `sembawang`
//! is meant to consume this component later, which would close a cycle), so
//! the logic is copied with its operation order kept, and every function
//! names its counterpart so the two cannot drift unnoticed.
//! ~~`tests/wet_well.rs` checks the copies **bit for bit** against `sembawang`'s
//! public functions (a dev-dependency only).~~ **CORRECTED 2026-11-01**
//! (maintainer: `sembawang` depends on `tampines`, never the reverse): the
//! bit-for-bit check and the whole-torus comparison live in
//! `crates/sembawang/tests/wet_well_vs_tampines.rs`, and `tampines` has no
//! `sembawang` dependency of any kind. `sembawang` itself is unchanged.
//!
//! | Here | Ported from (`sembawang::bwr_mark1`) |
//! |---|---|
//! | [`exchange`] (pool-surface mass and energy transfer, MELCOR CVH/FL RM §5.1) | `pool_surface::exchange`, steam and N2 only; the gas properties since 2026-11-02 from [`crate::components::wall_condensation::Atmosphere`] with no H2 (gh:#1118) |
//! | [`bubble_efficiency`] (MELCOR CVH/FL RM Eqs 6-1 to 6-3) | `pool_surface::bubble_efficiency` |
//! | the injection's bubble step | `containment::Containment::bubble_through` |
//! | flashing, surface exchange and rain-out | `containment::Containment::pool_surface_step` (its four-pass rain-out since 2026-11-02 the ablation [`WetWell::without_equilibrium_rain_out`]; the default rain-out is ported from [`crate::control_volume::ControlVolume`], gh:#1118) |
//! | the vacuum breaker | the `ww_atmosphere` vacuum-breaker branch of `containment::Containment::step` (step 5) |
//! | the immersed heat exchanger's implicit form | `containment::Containment::ww_shell_step` (its implicit two-capacity exchange, the coolant an infinite capacity) |
//! | N2 `c_v(T)` and internal energy (MELCOR NCG library) | `gas::cv_n2`, `gas::u_n2`, `gas::ncg_u`, `gas::ncg_c` (since 2026-11-01 used from `tampines_steam_tables::gas_mixture::melcor_ncg`, where `gas` moved verbatim; gh:#1092, gh:#1098) |
//! | [`t_sat`], [`steam_h`] and the steam/water energy conventions | `pool_surface::t_sat`, `pool_surface::steam_h`, `containment::steam_u`, `CP_WATER` |
//! | the pool's density (IF97 region 1) | `sparc::Pool::rho_l` |
//!
//! **New here, not in `sembawang`** (each stated): the pool level and the
//! moving gas volume (RELAP-7 Eqs 397–398); the injection port's elevation,
//! giving the submergence and the junction pressure (Eq 400); the split of a
//! two-phase injection into its liquid (to the pool) and its vapour (through
//! the bubble model) by IF97's saturation enthalpies at the junction
//! pressure; the liquid outlet (Eqs 391–392); the immersed heat exchanger
//! (the `q̇` of Eq 393); the vacuum breaker's direction and its external
//! volume as a fixed-state reservoir; the pipe-end pressures (Eqs 395, 399,
//! 400); and the compression work `p ΔV` the pool does on the gas, which
//! RELAP-7 neglects (the remark after Eq 390) and `sembawang` has no need of
//! (its gas volume is fixed).
//!
//! **Generalised 2026-11-01 by [`crate::control_volume::ControlVolume`]**
//! (gh:#1092): any `V(z)`, six non-condensables, IF97 on both sides, ports
//! at elevations and the swollen level. The surface functions above now live
//! in `crate::control_volume::pool_surface` (moved verbatim, re-exported here).
//!
//! ## Mechanisms: ON by default, ablations named `without_*`
//!
//! | Default | Ablation | What the ablation does |
//! |---|---|---|
//! | MELCOR's pool surface (diffusion through the N2, natural convection, radiation) | [`WetWell::without_surface_mass_transfer`] | RELAP-7 Eq 390: sensible heat only, `A_c α (T_w − T_g)` with a given `α`, no mass transfer |
//! | MELCOR's bubble efficiency (part of the steam passes uncondensed within 5 K of saturation) | [`WetWell::without_bubble_pass_through`] | full interaction always (`ε = 1`) |
//! | N2 `c_v(T)` (MELCOR NCG fit) | [`WetWell::without_nitrogen_cv_t`] | constant 20.8 J/(mol K), `sembawang`'s ablation |
//! | the pool's compression work on the gas | [`WetWell::without_pressure_work`] | RELAP-7's omission |
//! | rain-out to saturation (RH = 1) by a solve, as [`crate::control_volume::ControlVolume`]'s (since 2026-11-02, gh:#1118) | [`WetWell::without_equilibrium_rain_out`] | `sembawang`'s four passes, which end subsaturated; bit for bit the wet well before 2026-11-02 |
//!
//! ## Energy conventions (ported, stated)
//!
//! Internal energies are `sembawang`'s: liquid `c_p (T − 273.15)` with
//! `c_p = 4186 J/(kg K)`; steam `2.5060e6 + 1500 (T − 373.15)` J/kg (IF97's
//! saturated vapour at 100 °C with a constant `c_v`); N2 from 0 K. An
//! injection's specific enthalpy is the caller's (IF97, whose reference is the
//! liquid at the triple point, within a fraction of a J/kg of the liquid
//! reference here). Total energy is conserved exactly by construction; what
//! the conventions approximate is the temperature a given energy maps to.
//!
//! ## Not modelled (stated)
//!
//! Hydrogen and other non-condensables (the gas is N2 and steam, RELAP-7's
//! assumption 4 plus steam); the gravity terms `(z − L_w/2) g` of Eq 393
//! (`g L_w ≈ 25 J/kg` against `h_fg ≈ 2.3e6 J/kg`); kinetic energy; pool
//! stratification (`sembawang`'s `pool_layers`); the shell as a heat
//! structure (`sembawang`'s `ww_shell`); swell and void in the pool;
//! aerosol scrubbing; the liquid outlet only draws (no inflow); the vessel
//! is prismatic. ~~The rain-out is `sembawang`'s four passes (kept for the
//! bit-for-bit pin), which end subsaturated and amplify a round-off
//! difference at saturation to about 1e-4 in steam mass at any step size;
//! [`crate::control_volume::ControlVolume`]'s equilibrium rain-out does not
//! (gh:#1118, `tests/control_volume.rs::wet_well_round_off_amplification_gh1118`).~~
//! **CORRECTED 2026-11-02 (gh:#1118, maintainer decision):** the default
//! rain-out is now the control volume's equilibrium solve. `sembawang`'s four
//! passes, which end subsaturated and amplify a round-off difference at
//! saturation to about 1e-4 in steam mass at any step size, are the
//! ablation [`WetWell::without_equilibrium_rain_out`]. That ablation
//! carries the bit-for-bit port check
//! (`tests/control_volume.rs::wet_well_round_off_amplification_gh1118`,
//! `crates/sembawang/tests/wet_well_vs_tampines.rs`).

use tampines_steam_tables::prelude::h_tp_eqm_two_phase;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    Area, AvailableEnergy, Energy, HeatTransfer, Length, Mass, MassRate, Pressure, ThermalConductance,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use crate::TampinesError;

// The constants, IF97 helpers, MELCOR's pool surface and bubble efficiency
// were moved verbatim to `crate::control_volume::pool_surface` on 2026-11-01
// (gh:#1092), shared with the control volume; they are re-exported here so
// this module's public API is unchanged.
pub use crate::control_volume::pool_surface::{
    bubble_efficiency, exchange, p_sat_pa, steam_h, t_sat, Atmosphere, PoolState, SurfaceExchange, SurfaceGeometry, M_H2O, M_N2, R_GAS,
};
use crate::control_volume::pool_surface::{rho_liquid, G};
/// N2's `c_v(T)` and internal energy (MELCOR NCG fit): ~~a copy of
/// `sembawang::bwr_mark1::gas`~~ since 2026-11-01 (gh:#1092) the shared
/// `tampines_steam_tables::gas_mixture::melcor_ncg` functions, which have the
/// identical operation order (that module was moved there verbatim from
/// `sembawang`, gh:#1093).
///
/// The N2-only energy and heat-capacity sums (`gas::ncg_u`/`ncg_c` with no
/// H2) were also local copies until 2026-11-01 (gh:#1098 item 4); they are
/// now `melcor_ncg::{ncg_u, ncg_c}` with zero H2, bit for bit (pinned on a
/// grid by `tests/wet_well.rs::n2_fits_are_melcor_ncg_bit_for_bit`).
pub use tampines_steam_tables::gas_mixture::melcor_ncg::{cv_n2, u_n2};
use tampines_steam_tables::gas_mixture::melcor_ncg::{ncg_c, ncg_u};

// ── Constants (`sembawang::bwr_mark1::containment`) ─────────────────────────

/// Steam's constant `c_v`, J/(kg K), and IF97's saturated-vapour internal
/// energy at 373.15 K, J/kg (`containment::{CV_STEAM, U_STEAM_REF, T_REF}`).
const CV_STEAM: f64 = 1500.0;
const U_STEAM_REF: f64 = 2.5060e6;
const T_REF: f64 = 373.15;
/// Liquid water's constant `c_p`, J/(kg K) (`containment::CP_WATER`).
const CP_WATER: f64 = 4186.0;

/// The steam mass the gas holds saturated at `t` in `v` m³, kg (the
/// expression of `Containment::pool_surface_step`'s rain-out, operation for
/// operation).
fn steam_capacity(t: f64, v: f64) -> f64 {
    p_sat_pa(t.min(640.0)) * v * M_H2O / (R_GAS * t)
}

/// Steam's internal energy, J/kg (`containment::steam_u`).
fn steam_u(t: f64) -> f64 {
    U_STEAM_REF + CV_STEAM * (t - T_REF)
}

// ── The component ────────────────────────────────────────────────────────────

/// Which way the vacuum breaker passes gas. It is a check valve: it opens
/// only when the upstream side exceeds the downstream by its set point, and
/// never passes gas the other way.
///
/// The direction is the caller's choice, because the two installations
/// differ: a Mark I's wetwell-to-drywell breakers (and RELAP-7 Eq 389's
/// `ṁ_v`, the flow to the dry well) pass gas **out of** the wet well; its
/// reactor-building-to-torus breakers pass air **into** it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VacuumBreakerDirection {
    /// From the external volume into the gas space, when the external
    /// pressure exceeds the gas space's by the set point.
    IntoGasSpace,
    /// From the gas space to the external volume, when the gas space's
    /// pressure exceeds the external's by the set point (`sembawang`'s
    /// wetwell-to-drywell breakers).
    OutOfGasSpace,
}

/// The vacuum breaker: a one-way orifice `C_d A √(2 ρ ΔP)`, with `ΔP` the
/// pressure difference beyond the set point and `ρ` the donor's density
/// (`sembawang`'s vacuum-breaker orifice).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VacuumBreaker {
    /// Which way it passes gas.
    pub direction: VacuumBreakerDirection,
    /// The pressure difference at which it opens.
    pub opening_pressure_difference: Pressure,
    /// Flow area.
    pub flow_area: Area,
    /// Discharge coefficient (dimensionless).
    pub discharge_coefficient: f64,
}

/// The vessel's geometry: prismatic, of cross-section `A_c` and effective
/// height `L_t` (RELAP-7 Eqs 397–398), with the injection port's exit at
/// `z_i` above the pool bottom (Eq 400) and its opening height (MELCOR's
/// `h` in `ε_z`; `sembawang` uses a downcomer's diameter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellGeometry {
    /// Cross-section `A_c`.
    pub cross_section: Area,
    /// Effective total height `L_t`.
    pub total_height: Length,
    /// Elevation of the injection port's exit above the pool bottom, `z_i`.
    pub injection_elevation: Length,
    /// The injection opening's height.
    pub injection_opening: Length,
}

/// The initial state (RELAP-7 initialises from the level, the gas pressure
/// and the gas temperature; the pool temperature and the gas's relative
/// humidity are added here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellInitialState {
    /// Pool level `L_w(0)`.
    pub pool_level: Length,
    /// Gas-space pressure `p_g(0)`.
    pub gas_pressure: Pressure,
    /// Pool temperature.
    pub pool_temperature: ThermodynamicTemperature,
    /// Gas temperature `T_g(0)`.
    pub gas_temperature: ThermodynamicTemperature,
    /// Steam's partial pressure over its saturation pressure at the gas
    /// temperature, in [0, 1]; the rest of the pressure is nitrogen.
    pub relative_humidity: f64,
}

/// Steam, two-phase mixture or water discharged at the injection port below
/// the pool surface (a relief-valve quencher, a downcomer, an RCIC exhaust),
/// optionally with nitrogen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Injection {
    /// Water mass flow (any phase), ≥ 0.
    pub mass_rate: MassRate,
    /// Its specific enthalpy (IF97 reference).
    pub specific_enthalpy: AvailableEnergy,
    /// Nitrogen carried with it, ≥ 0.
    pub nitrogen_mass_rate: MassRate,
    /// The nitrogen's temperature (it arrives with its enthalpy).
    pub nitrogen_temperature: ThermodynamicTemperature,
}

impl Injection {
    /// No injection.
    pub fn none() -> Self {
        Self {
            mass_rate: MassRate::new::<kilogram_per_second>(0.0),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(0.0),
            nitrogen_mass_rate: MassRate::new::<kilogram_per_second>(0.0),
            nitrogen_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        }
    }
    /// Water only (steam, two-phase or liquid) at `h`.
    pub fn water(mass_rate: MassRate, specific_enthalpy: AvailableEnergy) -> Self {
        Self { mass_rate, specific_enthalpy, ..Self::none() }
    }
}

/// The external volume on the far side of the vacuum breaker: a reservoir of
/// fixed pressure, temperature and composition (as RELAP-7's time-dependent
/// volume, §6.13), whatever passes through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExternalVolume {
    /// Pressure.
    pub pressure: Pressure,
    /// Temperature.
    pub temperature: ThermodynamicTemperature,
    /// Steam's mole fraction, in [0, 1); the rest is nitrogen.
    pub steam_mole_fraction: f64,
}

/// The immersed heat exchanger: `q̇ = UA (T_pool − T_hx)` (the `q̇` of
/// RELAP-7 Eq 393), with the coolant at a fixed temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImmersedHeatExchanger {
    /// `UA`, ≥ 0.
    pub conductance: ThermalConductance,
    /// The coolant's temperature `T_hx`.
    pub coolant_temperature: ThermodynamicTemperature,
}

impl ImmersedHeatExchanger {
    /// Not running.
    pub fn off() -> Self {
        Self {
            conductance: ThermalConductance::new::<watt_per_kelvin>(0.0),
            coolant_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        }
    }
}

/// What drives one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellInputs {
    /// The injection port.
    pub injection: Injection,
    /// The vacuum breaker's external volume.
    pub external: ExternalVolume,
    /// The immersed heat exchanger.
    pub heat_exchanger: ImmersedHeatExchanger,
    /// Liquid drawn from the pool at its own enthalpy (RELAP-7 Eqs 391–392),
    /// ≥ 0.
    pub liquid_outflow: MassRate,
}

/// The surface model in use (see [`WetWell::without_surface_mass_transfer`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceModel {
    /// MELCOR's pool surface ([`exchange`]): the default.
    Melcor,
    /// RELAP-7 Eq 390: sensible heat `A_c α (T_w − T_g)` only.
    SensibleOnly {
        /// `α`.
        coefficient: HeatTransfer,
    },
}

/// Which mechanisms are on. All are on by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellMechanisms {
    /// The surface model.
    pub surface: SurfaceModel,
    /// MELCOR's bubble efficiency (else `ε = 1`).
    pub bubble_pass_through: bool,
    /// N2 `c_v(T)` (else constant).
    pub nitrogen_cv_t: bool,
    /// The pool's compression work on the gas.
    pub pressure_work: bool,
    /// Rain-out to saturation by a solve (else `sembawang`'s four passes).
    pub equilibrium_rain_out: bool,
}

impl Default for WetWellMechanisms {
    fn default() -> Self {
        Self { surface: SurfaceModel::Melcor, bubble_pass_through: true, nitrogen_cv_t: true, pressure_work: true, equilibrium_rain_out: true }
    }
}

/// What one step did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellStepReport {
    /// The bubble efficiency at the injection port this step.
    pub bubble_efficiency: f64,
    /// Injected steam that passed through the pool uncondensed (plus the
    /// vapour carried by the injected nitrogen), into the gas space.
    pub steam_to_gas: Mass,
    /// Gas through the vacuum breaker, positive into the gas space.
    pub vacuum_breaker_flow: Mass,
    /// Heat removed by the immersed heat exchanger.
    pub heat_exchanger_heat: Energy,
    /// Net evaporation at the pool surface (negative is condensation),
    /// including flashing and rain-out.
    pub surface_evaporation: Mass,
}

/// Running totals of what crossed the boundary since construction, for mass
/// and energy bookkeeping. `total_energy_now − initial = energy_in −
/// energy_out` holds to round-off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetWellBookkeeping {
    /// Water in (injection, steam through the breaker).
    pub water_in: Mass,
    /// Water out (breaker, liquid outlet).
    pub water_out: Mass,
    /// Nitrogen in.
    pub nitrogen_in: Mass,
    /// Nitrogen out.
    pub nitrogen_out: Mass,
    /// Energy in (injected enthalpy, breaker inflow's enthalpy).
    pub energy_in: Energy,
    /// Energy out (breaker outflow's enthalpy, liquid outlet, heat
    /// exchanger).
    pub energy_out: Energy,
}

/// The wet well. See the module documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct WetWell {
    area: f64,
    total_height: f64,
    z_inj: f64,
    opening: f64,
    breaker: VacuumBreaker,
    mech: WetWellMechanisms,
    n_n2: f64,
    steam_kg: f64,
    t_gas: f64,
    pool_kg: f64,
    t_pool: f64,
    water_in: f64,
    water_out: f64,
    n2_in: f64,
    n2_out: f64,
    e_in: f64,
    e_out: f64,
}

/// Quantities derived from the state.
#[derive(Clone, Copy, Debug)]
struct Derived {
    rho: f64,
    level: f64,
    v_gas: f64,
    p_gas: f64,
}

fn finite_nonneg(x: f64, what: &str) -> Result<f64, TampinesError> {
    if x.is_finite() && x >= 0.0 { Ok(x) } else { Err(TampinesError::InvalidInput(format!("{what} = {x}"))) }
}

impl WetWell {
    /// A wet well of `geometry` at `initial`, its vacuum breaker `breaker`,
    /// every mechanism on.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive or non-finite
    /// dimension, a level not below the total height, a relative humidity
    /// outside [0, 1], a gas pressure not above the steam's partial pressure,
    /// or a pool above its saturation temperature at the gas pressure.
    pub fn new(geometry: WetWellGeometry, breaker: VacuumBreaker, initial: WetWellInitialState) -> Result<Self, TampinesError> {
        let area = geometry.cross_section.get::<square_meter>();
        let lt = geometry.total_height.get::<meter>();
        let opening = geometry.injection_opening.get::<meter>();
        let z_inj = geometry.injection_elevation.get::<meter>();
        let level = initial.pool_level.get::<meter>();
        let p = initial.gas_pressure.get::<pascal>();
        let t_p = initial.pool_temperature.get::<kelvin>();
        let t_g = initial.gas_temperature.get::<kelvin>();
        let rh = initial.relative_humidity;
        for (x, what) in [(area, "cross-section"), (lt, "total height"), (opening, "injection opening"), (level, "pool level"), (p, "gas pressure"), (t_p, "pool temperature"), (t_g, "gas temperature")] {
            if !(x.is_finite() && x > 0.0) {
                return Err(TampinesError::InvalidInput(format!("{what} = {x}")));
            }
        }
        if !z_inj.is_finite() || z_inj < 0.0 {
            return Err(TampinesError::InvalidInput(format!("injection elevation = {z_inj}")));
        }
        if level >= lt {
            return Err(TampinesError::InvalidInput(format!("pool level {level} m not below the total height {lt} m")));
        }
        if !(0.0..=1.0).contains(&rh) {
            return Err(TampinesError::InvalidInput(format!("relative humidity = {rh}")));
        }
        let cd = breaker.discharge_coefficient;
        let a_vb = breaker.flow_area.get::<square_meter>();
        let set = breaker.opening_pressure_difference.get::<pascal>();
        if !(cd.is_finite() && cd >= 0.0 && a_vb.is_finite() && a_vb >= 0.0 && set.is_finite() && set >= 0.0) {
            return Err(TampinesError::InvalidInput(format!("vacuum breaker {breaker:?}")));
        }
        let p_steam = rh * p_sat_pa(t_g.min(640.0));
        if p <= p_steam {
            return Err(TampinesError::InvalidInput(format!("gas pressure {p} Pa not above the steam's {p_steam} Pa")));
        }
        if t_p > t_sat(p) {
            return Err(TampinesError::InvalidInput(format!("pool at {t_p} K above saturation at {p} Pa")));
        }
        let rho = rho_liquid(t_p, p);
        let v_gas = area * (lt - level);
        let n_tot = p * v_gas / (R_GAS * t_g);
        let n_steam = n_tot * p_steam / p;
        Ok(Self {
            area,
            total_height: lt,
            z_inj,
            opening,
            breaker,
            mech: WetWellMechanisms::default(),
            n_n2: n_tot - n_steam,
            steam_kg: n_steam * M_H2O,
            t_gas: t_g,
            pool_kg: rho * area * level,
            t_pool: t_p,
            water_in: 0.0,
            water_out: 0.0,
            n2_in: 0.0,
            n2_out: 0.0,
            e_in: 0.0,
            e_out: 0.0,
        })
    }

    /// Ablation: RELAP-7 Eq 390's sensible heat `A_c α (T_w − T_g)` with the
    /// given `α` replaces MELCOR's surface; no evaporation or condensation at
    /// the surface (flashing and rain-out still keep the state valid).
    pub fn without_surface_mass_transfer(mut self, coefficient: HeatTransfer) -> Self {
        self.mech.surface = SurfaceModel::SensibleOnly { coefficient };
        self
    }
    /// Ablation: full bubble interaction always (`ε = 1`), no steam passing
    /// uncondensed near saturation.
    pub fn without_bubble_pass_through(mut self) -> Self {
        self.mech.bubble_pass_through = false;
        self
    }
    /// Ablation: N2 at a constant 20.8 J/(mol K) (`sembawang`'s
    /// `without_gas_cv_t`).
    pub fn without_nitrogen_cv_t(mut self) -> Self {
        self.mech.nitrogen_cv_t = false;
        self
    }
    /// Ablation: no compression work from the pool on the gas (RELAP-7's
    /// omission).
    pub fn without_pressure_work(mut self) -> Self {
        self.mech.pressure_work = false;
        self
    }
    /// Ablation: `sembawang`'s rain-out (`Containment::pool_surface_step`):
    /// four passes, each condensing down to saturation at the current gas
    /// temperature while the latent heat stays in the gas, so they end
    /// subsaturated. This is the wet well's rain-out before 2026-11-02, bit
    /// for bit. It amplifies a round-off difference at saturation to about
    /// 1e-4 in steam mass (gh:#1118).
    pub fn without_equilibrium_rain_out(mut self) -> Self {
        self.mech.equilibrium_rain_out = false;
        self
    }
    /// The mechanisms in use.
    pub fn mechanisms(&self) -> WetWellMechanisms {
        self.mech
    }

    // ── Derived state ────────────────────────────────────────────────────

    /// Density, level, gas volume and pressure. The density is IF97's at
    /// the gas pressure (as `sembawang` evaluates it at the wetwell's), which
    /// depends on the level through the gas volume: three fixed-point
    /// passes, starting from the density at saturation, settle it (the
    /// liquid's compressibility, 4.5e-10 1/Pa, makes the coupling 1e-5 weak).
    fn derived(&self) -> Derived {
        let n = self.n_n2 + self.steam_kg / M_H2O;
        let mut rho = rho_liquid(self.t_pool, 0.0);
        let mut d = Derived { rho, level: 0.0, v_gas: 0.0, p_gas: 0.0 };
        for _ in 0..3 {
            let level = self.pool_kg / (rho * self.area);
            let v_gas = self.area * (self.total_height - level);
            let p_gas = n * R_GAS * self.t_gas / v_gas;
            d = Derived { rho, level, v_gas, p_gas };
            rho = rho_liquid(self.t_pool, p_gas);
        }
        d
    }

    fn n_gas(&self) -> f64 {
        self.n_n2 + self.steam_kg / M_H2O
    }
    /// The gas's energy and heat capacity (`Containment::ww_energy`,
    /// `ww_heat_capacity`).
    fn gas_energy(&self) -> f64 {
        ncg_u(self.n_n2, 0.0, self.t_gas, self.mech.nitrogen_cv_t) + self.steam_kg * steam_u(self.t_gas)
    }
    fn gas_heat_capacity(&self) -> f64 {
        ncg_c(self.n_n2, 0.0, self.t_gas, self.mech.nitrogen_cv_t) + self.steam_kg * CV_STEAM
    }
    /// The gas temperature from its energy, by Newton
    /// (`Containment::set_ww_energy`).
    fn set_gas_energy(&mut self, e: f64) {
        for _ in 0..50 {
            let dt = (e - self.gas_energy()) / self.gas_heat_capacity();
            self.t_gas += dt;
            if dt.abs() < 1e-9 * self.t_gas {
                break;
            }
        }
    }
    /// The pool's energy and its inverse (`Containment::pool_energy`,
    /// `set_pool_energy`).
    fn pool_energy(&self) -> f64 {
        self.pool_kg * CP_WATER * (self.t_pool - 273.15)
    }
    fn set_pool_energy(&mut self, e: f64) {
        self.t_pool = 273.15 + e / (self.pool_kg * CP_WATER);
    }

    // ── Getters ──────────────────────────────────────────────────────────

    /// Gas-space pressure `p_g`.
    pub fn gas_pressure(&self) -> Pressure {
        Pressure::new::<pascal>(self.derived().p_gas)
    }
    /// Pool level `L_w` above the bottom.
    pub fn pool_level(&self) -> Length {
        Length::new::<meter>(self.derived().level)
    }
    /// Pool temperature.
    pub fn pool_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_pool)
    }
    /// Gas temperature.
    pub fn gas_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_gas)
    }
    /// Pool mass.
    pub fn pool_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.pool_kg)
    }
    /// Steam in the gas space.
    pub fn steam_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.steam_kg)
    }
    /// Nitrogen in the gas space.
    pub fn nitrogen_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.n_n2 * M_N2)
    }
    /// Gas-space volume `A_c (L_t − L_w)` (RELAP-7 Eq 397).
    pub fn gas_volume(&self) -> Volume {
        Volume::new::<cubic_meter>(self.derived().v_gas)
    }
    /// Pool density `m_w / (A_c L_w)` (Eq 398).
    pub fn pool_density(&self) -> uom::si::f64::MassDensity {
        uom::si::f64::MassDensity::new::<uom::si::mass_density::kilogram_per_cubic_meter>(self.derived().rho)
    }
    /// The water space's reference pressure at mid-depth, `p_g + L_w ρ_w g /
    /// 2` (RELAP-7 Eq 395).
    pub fn pool_reference_pressure(&self) -> Pressure {
        let d = self.derived();
        Pressure::new::<pascal>(d.p_gas + 0.5 * d.level * d.rho * G)
    }
    /// The pressure at elevation `z` above the pool bottom: `p_w + (L_w/2 −
    /// z) ρ_w g` below the surface (RELAP-7 Eq 400), the gas pressure above
    /// it.
    pub fn pressure_at_elevation(&self, z: Length) -> Pressure {
        let d = self.derived();
        let z = z.get::<meter>();
        if z >= d.level {
            return Pressure::new::<pascal>(d.p_gas);
        }
        let p_w = d.p_gas + 0.5 * d.level * d.rho * G;
        Pressure::new::<pascal>(p_w + (0.5 * d.level - z) * d.rho * G)
    }
    /// The boundary pressure a connected pipe end at elevation `z` sees,
    /// RELAP-7 Eq 399 solved for `p_in`: `p_i + (K − 1) ρu² / 2`, with
    /// `ρu²` the pipe end's momentum flux, Pa, and `K` its form loss.
    pub fn pipe_end_pressure(&self, z: Length, form_loss: f64, rho_u_squared: Pressure) -> Pressure {
        self.pressure_at_elevation(z) + 0.5 * (form_loss - 1.0) * rho_u_squared
    }
    /// Water in the component (pool plus gas-space steam).
    pub fn water_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.pool_kg + self.steam_kg)
    }
    /// Total internal energy, gas plus pool, in this module's conventions.
    pub fn total_energy(&self) -> Energy {
        Energy::new::<joule>(self.gas_energy() + self.pool_energy())
    }
    /// Running totals across the boundary.
    pub fn bookkeeping(&self) -> WetWellBookkeeping {
        WetWellBookkeeping {
            water_in: Mass::new::<kilogram>(self.water_in),
            water_out: Mass::new::<kilogram>(self.water_out),
            nitrogen_in: Mass::new::<kilogram>(self.n2_in * M_N2),
            nitrogen_out: Mass::new::<kilogram>(self.n2_out * M_N2),
            energy_in: Energy::new::<joule>(self.e_in),
            energy_out: Energy::new::<joule>(self.e_out),
        }
    }

    // ── Step ─────────────────────────────────────────────────────────────

    /// Advance by `dt`, in `sembawang`'s order: the injection through the
    /// bubble model, the vacuum breaker, the heat exchanger and the liquid
    /// outlet, then the pool surface (flashing, exchange, rain-out), and
    /// finally the compression work of the pool's change of volume.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`, a negative or
    /// non-finite flow, a steam mole fraction outside [0, 1), or an outflow
    /// larger than the pool; [`TampinesError::Unphysical`] if the pool fills
    /// the vessel or a temperature leaves the finite positive range.
    pub fn step(&mut self, dt: Time, inputs: &WetWellInputs) -> Result<WetWellStepReport, TampinesError> {
        let dt = dt.get::<second>();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("dt = {dt}")));
        }
        let m_inj = finite_nonneg(inputs.injection.mass_rate.get::<kilogram_per_second>(), "injection mass rate")? * dt;
        let h_inj = inputs.injection.specific_enthalpy.get::<joule_per_kilogram>();
        let n2_inj = finite_nonneg(inputs.injection.nitrogen_mass_rate.get::<kilogram_per_second>(), "injection N2 rate")? * dt / M_N2;
        let t_n2 = inputs.injection.nitrogen_temperature.get::<kelvin>();
        let m_out = finite_nonneg(inputs.liquid_outflow.get::<kilogram_per_second>(), "liquid outflow")? * dt;
        let ua = finite_nonneg(inputs.heat_exchanger.conductance.get::<watt_per_kelvin>(), "heat-exchanger UA")?;
        let t_hx = inputs.heat_exchanger.coolant_temperature.get::<kelvin>();
        let p_ext = inputs.external.pressure.get::<pascal>();
        let t_ext = inputs.external.temperature.get::<kelvin>();
        let x_ext = inputs.external.steam_mole_fraction;
        if !h_inj.is_finite() || !(t_n2.is_finite() && t_n2 > 0.0) || !(t_hx.is_finite() && t_hx > 0.0) {
            return Err(TampinesError::InvalidInput(format!("injection {:?} / heat exchanger {:?}", inputs.injection, inputs.heat_exchanger)));
        }
        if !(p_ext.is_finite() && p_ext > 0.0 && t_ext.is_finite() && t_ext > 0.0 && (0.0..1.0).contains(&x_ext)) {
            return Err(TampinesError::InvalidInput(format!("external volume {:?}", inputs.external)));
        }
        let start = self.derived();
        let v_pool_start = self.pool_kg / start.rho;

        // 1. Injection.
        let (eps, steam_to_gas) = self.inject(m_inj, h_inj, n2_inj, t_n2);

        // 2. Vacuum breaker.
        let vb = self.vacuum_breaker(dt, p_ext, t_ext, x_ext);

        // 3. Immersed heat exchanger, implicit in the pool temperature (the
        //    form of `Containment::ww_shell_step` with the coolant an
        //    infinite capacity): `q = k ΔT / (1 + k / C_pool)`, `k = UA dt`.
        let k = ua * dt;
        let q_hx = k * (self.t_pool - t_hx) / (1.0 + k / (self.pool_kg * CP_WATER));
        let e = self.pool_energy() - q_hx;
        self.set_pool_energy(e);
        self.e_out += q_hx;

        // 4. Liquid outlet at the pool's enthalpy (RELAP-7 Eqs 391–392;
        //    kinetic energy neglected).
        if m_out > 0.0 {
            if m_out >= self.pool_kg {
                return Err(TampinesError::InvalidInput(format!("outflow {m_out} kg not below the pool's {} kg", self.pool_kg)));
            }
            let e_out = m_out * CP_WATER * (self.t_pool - 273.15);
            let e = self.pool_energy() - e_out;
            self.pool_kg -= m_out;
            self.set_pool_energy(e);
            self.water_out += m_out;
            self.e_out += e_out;
        }

        // 5. The pool surface.
        let evap = self.surface(dt);

        // 6. The pool's compression work on the gas: `p ΔV_pool` at the
        //    mean of the step's start and end pressures, from the pool to
        //    the gas (an internal transfer: the vessel is rigid).
        let end = self.derived();
        if !(end.level < self.total_height) {
            return Err(TampinesError::Unphysical(format!("the pool fills the vessel (level {} m)", end.level)));
        }
        if self.mech.pressure_work {
            let w = 0.5 * (start.p_gas + end.p_gas) * (self.pool_kg / end.rho - v_pool_start);
            let e_p = self.pool_energy() - w;
            self.set_pool_energy(e_p);
            let e_g = self.gas_energy() + w;
            self.set_gas_energy(e_g);
        }
        for (t, what) in [(self.t_pool, "pool temperature"), (self.t_gas, "gas temperature")] {
            if !(t.is_finite() && t > 0.0) {
                return Err(TampinesError::Unphysical(format!("{what} = {t} K")));
            }
        }
        Ok(WetWellStepReport {
            bubble_efficiency: eps,
            steam_to_gas: Mass::new::<kilogram>(steam_to_gas),
            vacuum_breaker_flow: Mass::new::<kilogram>(vb),
            heat_exchanger_heat: Energy::new::<joule>(q_hx),
            surface_evaporation: Mass::new::<kilogram>(evap),
        })
    }

    /// The injection: `m` kg of water at `h` J/kg and `n2` mol of N2 at
    /// `t_n2`. A two-phase injection is split by IF97's saturation
    /// enthalpies at the junction pressure (new here): its liquid joins the
    /// pool, its vapour (and the N2) goes through
    /// [`Self::bubble_through`]. Returns the bubble efficiency and the steam
    /// that reached the gas space, kg.
    fn inject(&mut self, m: f64, h: f64, n2: f64, t_n2: f64) -> (f64, f64) {
        if m <= 0.0 && n2 <= 0.0 {
            return (self.efficiency(), 0.0);
        }
        let e_in = m * h;
        let e_n2 = n2 * (u_n2(t_n2, self.mech.nitrogen_cv_t) + R_GAS * t_n2);
        self.water_in += m;
        self.n2_in += n2;
        self.e_in += e_in + e_n2;
        let (m_v, e_v) = if m > 0.0 {
            let p_j = self.junction_pressure().min(2.0e7);
            let t_s = ThermodynamicTemperature::new::<kelvin>(t_sat(p_j));
            let p = Pressure::new::<pascal>(p_j);
            let h_f = h_tp_eqm_two_phase(t_s, p, 0.0).get::<joule_per_kilogram>();
            let h_g = h_tp_eqm_two_phase(t_s, p, 1.0).get::<joule_per_kilogram>();
            let x = ((h - h_f) / (h_g - h_f)).clamp(0.0, 1.0);
            let m_l = (1.0 - x) * m;
            let e_l = m_l * h.min(h_f);
            if m_l > 0.0 {
                let e = self.pool_energy() + e_l;
                self.pool_kg += m_l;
                self.set_pool_energy(e);
            }
            (m - m_l, e_in - e_l)
        } else {
            (0.0, 0.0)
        };
        self.bubble_through(n2, m_v, e_v + e_n2)
    }

    /// The pressure at the injection port, `p_g + ρ g (L_w − z_i)` (RELAP-7
    /// Eqs 395 and 400 combined; `sembawang` uses `p_ww +
    /// vent_submergence_pa`).
    fn junction_pressure(&self) -> f64 {
        let d = self.derived();
        d.p_gas + d.rho * G * (d.level - self.z_inj).max(0.0)
    }

    /// The bubble efficiency at the injection port now.
    fn efficiency(&self) -> f64 {
        if !self.mech.bubble_pass_through {
            return 1.0;
        }
        let d = self.derived();
        let depth = d.level - self.z_inj;
        bubble_efficiency(depth, self.opening, d.p_gas + d.rho * G * depth.max(0.0), self.t_pool)
    }

    /// Gas entering the pool below its surface, `dn2` mol and `m_s` kg of
    /// steam carrying `e_in` J, by MELCOR's bubble model (CVH/FL RM §6.1).
    /// **Port of `sembawang::bwr_mark1::containment::Containment::bubble_through`**
    /// (hydrogen removed; the depth from the level instead of the
    /// submergence head): with efficiency `ε` between no interaction
    /// (everything joins the gas unchanged) and full interaction (the N2
    /// leaves at the pool's temperature with vapour at 0.99 of its
    /// saturation pressure; the rest of the steam and the energy stay in the
    /// pool), linearly. Returns `ε` and the steam into the gas, kg.
    fn bubble_through(&mut self, dn2: f64, m_s: f64, e_in: f64) -> (f64, f64) {
        let tv = self.mech.nitrogen_cv_t;
        let p_ww = self.derived().p_gas;
        let eps = self.efficiency();
        let p_v = (0.99 * p_sat_pa(self.t_pool)).min(0.99 * p_ww);
        let m_v = dn2 * p_v / (p_ww - p_v) * M_H2O;
        let e_full = ncg_u(dn2, 0.0, self.t_pool, tv) + m_v * steam_u(self.t_pool);
        let to_atm_steam = eps * m_v + (1.0 - eps) * m_s;
        let to_atm_e = eps * e_full + (1.0 - eps) * e_in;
        let e_a = self.gas_energy() + to_atm_e;
        self.n_n2 += dn2;
        self.steam_kg += to_atm_steam;
        self.set_gas_energy(e_a);
        let e_p = self.pool_energy() + (e_in - to_atm_e);
        self.pool_kg += m_s - to_atm_steam;
        self.set_pool_energy(e_p);
        (eps, to_atm_steam)
    }

    /// The vacuum breaker. **Port of the `ww_atmosphere` vacuum-breaker
    /// branch of `sembawang::bwr_mark1::containment::Containment::step`**
    /// (step 5): the orifice `C_d A √(2 ρ ΔP) dt / M` at the donor's
    /// density, capped at the equalising amount. With a fixed-state external
    /// volume the cap counts only the gas space's change (the limit of
    /// `sembawang`'s two-sided cap as the other volume grows without bound):
    /// out of the gas space, the fraction `ΔP / p_g`; into it, `ΔP n_g /
    /// p_g` mol (isothermal). Outflow leaves with the gas's enthalpy;
    /// inflow arrives with the external gas's (the direction into the gas
    /// space is `sembawang`'s branch mirrored). Returns the mass moved, kg,
    /// positive into the gas space.
    fn vacuum_breaker(&mut self, dt: f64, p_ext: f64, t_ext: f64, x_ext: f64) -> f64 {
        let tv = self.mech.nitrogen_cv_t;
        let p_g = self.derived().p_gas;
        let set = self.breaker.opening_pressure_difference.get::<pascal>();
        let cd = self.breaker.discharge_coefficient;
        let a = self.breaker.flow_area.get::<square_meter>();
        match self.breaker.direction {
            VacuumBreakerDirection::OutOfGasSpace => {
                let suck = p_g - p_ext - set;
                if suck <= 0.0 {
                    return 0.0;
                }
                let n_g = self.n_gas();
                let molar = (self.n_n2 * M_N2 + self.steam_kg) / n_g;
                let rho = p_g * molar / (R_GAS * self.t_gas);
                let orifice = cd * a * (2.0 * rho * suck).sqrt() * dt / molar;
                let f = (orifice / n_g).min(suck / p_g);
                let (dn2, dsteam) = (self.n_n2 * f, self.steam_kg * f);
                let e_out = f * (self.gas_energy() + n_g * R_GAS * self.t_gas);
                let e_a = self.gas_energy() - e_out;
                self.n_n2 -= dn2;
                self.steam_kg -= dsteam;
                self.set_gas_energy(e_a);
                self.n2_out += dn2;
                self.water_out += dsteam;
                self.e_out += e_out;
                -(dn2 * M_N2 + dsteam)
            }
            VacuumBreakerDirection::IntoGasSpace => {
                let push = p_ext - p_g - set;
                if push <= 0.0 {
                    return 0.0;
                }
                let molar = x_ext * M_H2O + (1.0 - x_ext) * M_N2;
                let rho = p_ext * molar / (R_GAS * t_ext);
                let orifice = cd * a * (2.0 * rho * push).sqrt() * dt / molar;
                let n = orifice.min(push * self.n_gas() / p_g);
                let (dn2, dsteam) = ((1.0 - x_ext) * n, x_ext * n * M_H2O);
                let e_in = dn2 * (u_n2(t_ext, tv) + R_GAS * t_ext) + dsteam * steam_h(t_ext);
                let e_a = self.gas_energy() + e_in;
                self.n_n2 += dn2;
                self.steam_kg += dsteam;
                self.set_gas_energy(e_a);
                self.n2_in += dn2;
                self.water_in += dsteam;
                self.e_in += e_in;
                dn2 * M_N2 + dsteam
            }
        }
    }

    /// The pool surface. **Port of
    /// `sembawang::bwr_mark1::containment::Containment::pool_surface_step`**:
    /// flashing of a pool above the gas's saturation temperature (the vapour
    /// leaving at once as saturated steam), then MELCOR's surface exchange
    /// ([`exchange`]; or, ablated, RELAP-7 Eq 390's sensible heat, implicit
    /// in the two temperatures as `ww_shell_step`'s exchange), then rain-out
    /// of vapour above saturation in the gas (its latent heat staying in the
    /// gas; by default to saturation by [`Self::rain_out_equilibrium`], the
    /// ablation `sembawang`'s four passes). The pool depth is the level and the gas depth the gas volume
    /// over the area (`sembawang`: pool volume and fixed gas volume over the
    /// area). Returns the net evaporation, kg.
    fn surface(&mut self, dt: f64) -> f64 {
        let mut net = 0.0;
        let d = self.derived();
        let t_s = t_sat(d.p_gas);
        if self.t_pool > t_s {
            let h_fg = steam_h(t_s) - CP_WATER * (t_s - 273.15);
            let m = self.pool_kg * CP_WATER * (self.t_pool - t_s) / h_fg;
            let e_a = self.gas_energy() + m * steam_h(t_s);
            self.steam_kg += m;
            self.set_gas_energy(e_a);
            self.pool_kg -= m;
            self.t_pool = t_s;
            net += m;
        }
        match self.mech.surface {
            SurfaceModel::Melcor => {
                let d = self.derived();
                let ex = exchange(
                    &PoolState { t_k: self.t_pool, kg: self.pool_kg, rho: d.rho, cp: CP_WATER },
                    &Atmosphere { t_k: self.t_gas, p_pa: d.p_gas, n_steam: self.steam_kg / M_H2O, n_n2: self.n_n2 },
                    &SurfaceGeometry { area_m2: self.area, pool_depth_m: d.level, gas_depth_m: d.v_gas / self.area },
                    dt,
                );
                let m = ex.evaporation_kg_s * dt;
                let h_f = CP_WATER * (self.t_pool - 273.15);
                let h_g = steam_h(self.t_gas);
                let e_p = self.pool_energy() - (m * h_f + ex.q_pool_w * dt);
                let e_a = self.gas_energy() + (m * h_g - ex.q_atm_w * dt);
                self.pool_kg -= m;
                self.set_pool_energy(e_p);
                self.steam_kg += m;
                self.set_gas_energy(e_a);
                net += m;
            }
            SurfaceModel::SensibleOnly { coefficient } => {
                let k = self.area * coefficient.get::<watt_per_square_meter_kelvin>() * dt;
                let c_g = self.gas_heat_capacity();
                let c_p = self.pool_kg * CP_WATER;
                let q = k * (self.t_pool - self.t_gas) / (1.0 + k / c_g + k / c_p);
                let e_p = self.pool_energy() - q;
                self.set_pool_energy(e_p);
                let e_a = self.gas_energy() + q;
                self.set_gas_energy(e_a);
            }
        }
        if self.mech.equilibrium_rain_out {
            net -= self.rain_out_equilibrium();
            return net;
        }
        for _ in 0..4 {
            let v_gas = self.derived().v_gas;
            let keep = steam_capacity(self.t_gas, v_gas);
            if self.steam_kg <= keep {
                break;
            }
            let cond = self.steam_kg - keep;
            let h_l = CP_WATER * (self.t_gas - 273.15);
            let e_a = self.gas_energy() - cond * h_l;
            self.steam_kg = keep;
            self.set_gas_energy(e_a);
            let e_p = self.pool_energy() + cond * h_l;
            self.pool_kg += cond;
            self.set_pool_energy(e_p);
            net -= cond;
        }
        net
    }

    /// The default rain-out: condense the mass `c` that leaves the remaining
    /// steam saturating the gas at the temperature its energy then gives,
    /// in the gas volume the grown pool then leaves (RH = 1 to 1e-13 of the
    /// steam). The condensate leaves the gas, and joins the pool, at the
    /// liquid enthalpy at the gas's starting temperature, so its latent heat
    /// stays in the gas, as in the four passes. Returns the mass condensed,
    /// kg.
    ///
    /// **Port of [`crate::control_volume::ControlVolume`]'s `rain_out`**
    /// (gh:#1092, its default): the same bisection on the condensed mass, the
    /// same bracket `[0, s0 − keep0]`, and the same 80-pass and `1e-13 s0`
    /// limits. The difference is stated: the gas volume in the residual is
    /// the wet well's own `derived` at the trial pool mass (its density
    /// re-evaluated), where the control volume holds the density fixed.
    fn rain_out_equilibrium(&mut self) -> f64 {
        let v0 = self.derived().v_gas;
        let s0 = self.steam_kg;
        let keep0 = steam_capacity(self.t_gas, v0);
        if s0 <= keep0 {
            return 0.0;
        }
        let h_l = CP_WATER * (self.t_gas - 273.15);
        let e0 = self.gas_energy();
        let resid = |cond: f64| {
            let mut probe = self.clone();
            probe.steam_kg = s0 - cond;
            probe.pool_kg += cond;
            probe.set_gas_energy(e0 - cond * h_l);
            let v = probe.derived().v_gas;
            s0 - cond - steam_capacity(probe.t_gas, v)
        };
        // At `cond = s0 − keep0` the latent heat has warmed the gas, so the
        // residual is negative there; the root lies in the bracket.
        let (mut lo, mut hi) = (0.0, s0 - keep0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if resid(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo <= 1e-13 * s0 {
                break;
            }
        }
        let cond = 0.5 * (lo + hi);
        let e_a = self.gas_energy() - cond * h_l;
        self.steam_kg = s0 - cond;
        self.set_gas_energy(e_a);
        let e_p = self.pool_energy() + cond * h_l;
        self.pool_kg += cond;
        self.set_pool_energy(e_p);
        cond
    }
}
