// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Two-region non-equilibrium control volume: a **pool** and an
//! **atmosphere**, each at its own temperature, with the level from a
//! volume–altitude table and the swollen level (gh:#1092, epic gh:#1091;
//! after MELCOR 2.2's CVH package, CVH/FL reference manual §2 and §5, cited by
//! section and equation, not quoted; no MELCOR source used).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! This generalises [`crate::components::wet_well::WetWell`] (gh:#1075):
//!
//! | Wet well | Control volume |
//! |---|---|
//! | prismatic, `A_c` and `L_t` | [`VolumeAltitudeTable`]: any `V(z)`, piecewise linear |
//! | steam + N2, ideal, sembawang's constant-`c_v` steam | [`GasMixture`]: steam + N2, H2, O2, CO, CO2, Ar, closed by [`GasMixtureModel`] (IF97 steam by default, gh:#1093) |
//! | liquid `c_p (T − 273.15)` | IF97 region-1 liquid by default; `sembawang`'s constant `c_p` the ablation |
//! | one injection port, one liquid outlet, a vacuum breaker | any number of [`Port`]s at elevations, each with a source and an outflow |
//! | no swell | MELCOR's swollen level (Eqs 5-26 to 5-30), ported from `sembawang` |
//! | rain-out by `sembawang`'s four passes (ends subsaturated) | rain-out to saturation (RH = 1) by a solve; the four passes the ablation |
//!
//! The wet well's vacuum breaker is a flow path, not part of a volume: it is
//! gh:#1094's job here (see "For gh:#1094" below).
//!
//! ## Ported or reused, not written fresh
//!
//! | Here | From |
//! |---|---|
//! | [`pool_surface::exchange`], [`pool_surface::bubble_efficiency`] | **moved verbatim** from the wet well (themselves `sembawang::bwr_mark1::pool_surface::{exchange, bubble_efficiency}`), shared by both |
//! | the bubble step at a submerged port | the wet well's `bubble_through` (`sembawang`'s `Containment::bubble_through`), with every non-condensable instead of N2 |
//! | flashing, the surface step, the sensible-only ablation | the wet well's `surface` (`sembawang`'s `Containment::pool_surface_step`) |
//! | the split of a two-phase source into pool liquid and bubbles | the wet well's `inject` |
//! | the pool's compression work on the atmosphere | the wet well's step 6 |
//! | the immersed heat exchanger's implicit form | the wet well's step 3 (`sembawang`'s `ww_shell_step`) |
//! | [`level::swollen_level`] | `sembawang::bwr_mark1::swollen_level` (gh:#1036) |
//! | the atmosphere's pressure, energy, relative humidity | `tampines_steam_tables::gas_mixture` (gh:#1093) |
//! | the port's draw by the level | `tampines_steam_tables::hem::VolumePort::AtHeight` (gh:#1073): liquid below the level with `ρ g (L − z)`, vapour above |
//!
//! ## Several non-condensables at the pool surface (a limit, stated)
//!
//! MELCOR's surface correlations are ported for steam and **one**
//! non-condensable with N2's properties. The control volume passes the
//! **total** non-condensable moles as that gas: so every non-condensable's
//! mole fraction enters the steam's partial pressure and the diffusion
//! driving force `ln((p − p_w)/(p − p_sat(T_s)))`, but the diffusivity
//! (MELCOR's steam–air correlation), the mixture viscosity and conductivity,
//! the density (natural-convection buoyancy) and `c_p` are N2's. For an
//! H2-rich atmosphere this misstates the density (and so the convection) and
//! the diffusivity (H2–steam is several times steam–N2); `sembawang`'s
//! original carries H2's own properties, which a follow-up could port.
//!
//! ## State, energy and conservation
//!
//! The state is the pool's mass and internal energy and the atmosphere's
//! contents ([`GasMixture`]) and internal energy. Temperatures, the pool
//! density, the level, the atmosphere's volume and pressure are derived (the
//! density and the atmosphere's pressure by three fixed-point passes, as the
//! wet well). Every transfer adds to one inventory what it takes from
//! another or from the boundary, so mass, per-species mass and energy close
//! to round-off by construction; [`ControlVolumeBookkeeping`] carries what
//! crossed the boundary.
//!
//! The liquid's internal energy in the default model is IF97 region 1 on the
//! saturation line (`p = 1.0001 p_sat(T)`); its pressure dependence
//! (≈ −0.01 J/kg per kPa near 300 K) is neglected. Liquid leaving or arriving
//! carries `h = u + p v`, `p` the pressure at the port; the pool's
//! compression work `p ΔV` moves the flow work between the pool and the
//! atmosphere. Under the constant-`c_p` ablation `h = u = c_p (T −
//! 273.15)` (`sembawang`'s, as the wet well).
//!
//! ## Mechanisms: ON by default, ablations named `without_*`
//!
//! | Default | Ablation |
//! |---|---|
//! | MELCOR's pool surface | [`ControlVolume::without_surface_mass_transfer`] (sensible heat only) |
//! | MELCOR's bubble efficiency | [`ControlVolume::without_bubble_pass_through`] (`ε = 1`) |
//! | the pool's compression work | [`ControlVolume::without_pressure_work`] |
//! | the swollen level decides what a port draws | [`ControlVolume::without_level_swell`] (the collapsed level) |
//! | rain-out to saturation | [`ControlVolume::without_equilibrium_rain_out`] (`sembawang`'s four passes) |
//! | IF97 liquid | [`ControlVolume::without_real_liquid`] (constant `c_p` 4186 J/(kg K)) |
//! | IF97 steam in the atmosphere | [`ControlVolume::without_real_steam`] |
//! | non-condensable `c_v(T)` | [`ControlVolume::without_ncg_cv_t`] |
//!
//! ## For gh:#1094 (flow paths) and gh:#1095 (implicit network)
//!
//! A flow path between two volumes needs, at each end: the pressure at the
//! opening's elevation ([`ControlVolume::pressure_at_elevation`]), and the
//! donor's state if flow leaves through it ([`ControlVolume::port_donor`]:
//! what is drawn by the level, its density, specific enthalpy, temperature
//! and composition). A network evaluates both for every volume, computes the
//! flows, and drives each volume with one [`PortInput`] per port:
//! `outflow` for flow leaving (the volume decides pool or atmosphere by the
//! level), `inflow` a [`Source`] carrying the donor's water, enthalpy and
//! non-condensables. [`ControlVolume::step`] applies them and returns, per
//! port, what actually moved ([`PortTransfer`]); the network passes that, not
//! its own estimate, to the receiving volume, so the network conserves what
//! each volume conserves. An implicit scheme (gh:#1095) can linearise around
//! [`ControlVolume::pressure_at_elevation`] by stepping a clone.
//!
//! ## Not modelled (stated)
//!
//! Kinetic energy and gravity in the energy equation; gravity in the
//! atmosphere (the pressure above the level is the atmosphere's); the void's
//! own effect on what a port draws (a port below the swollen level draws
//! liquid at the pool's state, not a two-phase mixture); fog (rain-out goes to
//! the pool at once); pool stratification; heat structures (the caller's, as
//! `pool_heat`); aerosols.

pub mod level;
pub mod pool_surface;

pub use level::{swollen_level, swollen_level_from_vapour_flow, VolumeAltitudeTable, BUBBLE_RISE_M_S, MAX_POOL_VOID};
pub use tampines_steam_tables::gas_mixture::{GasMixture, GasMixtureModel, NonCondensable};

use tampines_steam_tables::gas_mixture::SteamModel;
use tampines_steam_tables::prelude::h_tp_eqm_two_phase;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::u_tp_1;
use tampines_steam_tables::region_2_vapour::{h_tp_2, u_tp_2, v_tp_2};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AvailableEnergy, Energy, HeatTransfer, Length, Mass, MassDensity, MassRate, Power, Pressure, ThermodynamicTemperature, Time, Volume,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use crate::components::wet_well::{ImmersedHeatExchanger, SurfaceModel};
use crate::TampinesError;
use pool_surface::{bubble_efficiency, exchange, p_sat_pa, rho_liquid, steam_h, t_sat, Atmosphere, PoolState, SurfaceGeometry, G, M_H2O, R_GAS};

/// `sembawang`'s constant-`c_p` liquid, J/(kg K) (`containment::CP_WATER`).
const CP_WATER: f64 = 4186.0;
/// `sembawang`'s ideal steam (`containment::{U_STEAM_REF, CV_STEAM, T_REF}`).
const U_STEAM_REF: f64 = 2.5060e6;
const CV_STEAM: f64 = 1500.0;
const T_REF: f64 = 373.15;

// ── Inputs and outputs ───────────────────────────────────────────────────────

/// An opening at an elevation: a flow path's end, a vent, a drain, a sparger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Port {
    /// Elevation of the opening (the table's datum).
    pub elevation: Length,
    /// The opening's height (MELCOR's `h` in the bubble efficiency's `ε_z`).
    pub opening_height: Length,
}

/// What a port draws, decided by the level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDraw {
    /// Pool liquid (the port is below the level).
    Liquid,
    /// The atmosphere's mixture (the port is at or above the level).
    Atmosphere,
}

/// What enters through a port: water (any phase) at a specific enthalpy (IF97
/// reference) and non-condensables at a temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// Water mass flow, ≥ 0.
    pub water_mass_rate: MassRate,
    /// The water's specific enthalpy.
    pub water_specific_enthalpy: AvailableEnergy,
    /// Each non-condensable's mass flow, [`NonCondensable::ALL`] order, ≥ 0.
    pub ncg_mass_rates: [MassRate; 6],
    /// The non-condensables' temperature (they arrive with their enthalpy).
    pub ncg_temperature: ThermodynamicTemperature,
}

impl Source {
    /// Nothing.
    pub fn none() -> Self {
        Self {
            water_mass_rate: MassRate::new::<kilogram_per_second>(0.0),
            water_specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ncg_mass_rates: [MassRate::new::<kilogram_per_second>(0.0); 6],
            ncg_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        }
    }
    /// Water only, at `h`.
    pub fn water(mass_rate: MassRate, specific_enthalpy: AvailableEnergy) -> Self {
        Self { water_mass_rate: mass_rate, water_specific_enthalpy: specific_enthalpy, ..Self::none() }
    }
    /// Non-condensables only, at `t`.
    pub fn gas(rates: &[(NonCondensable, MassRate)], t: ThermodynamicTemperature) -> Self {
        Self::none().with_gas(rates, t)
    }
    /// This source plus non-condensables at `t`.
    pub fn with_gas(mut self, rates: &[(NonCondensable, MassRate)], t: ThermodynamicTemperature) -> Self {
        for (g, m) in rates {
            self.ncg_mass_rates[g.index()] += *m;
        }
        self.ncg_temperature = t;
        self
    }
}

/// One port's drive for a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortInput {
    /// What enters.
    pub inflow: Source,
    /// Mass leaving, ≥ 0; pool liquid or atmosphere by the level.
    pub outflow: MassRate,
}

impl PortInput {
    /// Nothing through this port.
    pub fn closed() -> Self {
        Self { inflow: Source::none(), outflow: MassRate::new::<kilogram_per_second>(0.0) }
    }
    /// Inflow only.
    pub fn inflow(source: Source) -> Self {
        Self { inflow: source, ..Self::closed() }
    }
    /// Outflow only.
    pub fn outflow(rate: MassRate) -> Self {
        Self { outflow: rate, ..Self::closed() }
    }
}

/// What drives one step.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolumeInputs {
    /// One input per port, in the geometry's port order.
    pub ports: Vec<PortInput>,
    /// Heat into the pool (a heat structure, a core), W; negative removes.
    pub pool_heat: Power,
    /// The immersed heat exchanger (implicit in the pool temperature).
    pub heat_exchanger: ImmersedHeatExchanger,
    /// Heat into the atmosphere (a heat structure's convection, a hot
    /// insulation), W; negative removes. Explicit, as `pool_heat`
    /// (gh:#1097: the caller's heat structures, MELCOR's HS package not
    /// being here).
    pub atmosphere_heat: Power,
    /// Steam condensed out of the atmosphere onto a structure (the caller's),
    /// ≥ 0: it leaves with its enthalpy at the atmosphere's temperature and
    /// steam partial pressure, and the caller returns the condensate, if at
    /// all, as a port's water source (gh:#1097; MELCOR HS §2.7.3's
    /// condensation, the structure being the caller's).
    pub atmosphere_steam_sink: MassRate,
}

impl ControlVolumeInputs {
    /// Every port closed, no heat, for `n_ports` ports.
    pub fn closed(n_ports: usize) -> Self {
        Self {
            ports: vec![PortInput::closed(); n_ports],
            pool_heat: Power::new::<watt>(0.0),
            heat_exchanger: ImmersedHeatExchanger::off(),
            atmosphere_heat: Power::new::<watt>(0.0),
            atmosphere_steam_sink: MassRate::new::<kilogram_per_second>(0.0),
        }
    }
}

/// What one port moved in a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortTransfer {
    /// The pressure at the port at the start of the step.
    pub pressure: Pressure,
    /// What the outflow drew.
    pub draw: PortDraw,
    /// Water in.
    pub water_in: Mass,
    /// Non-condensables in, [`NonCondensable::ALL`] order.
    pub ncg_in: [Mass; 6],
    /// Energy in.
    pub energy_in: Energy,
    /// Water out (liquid, or the atmosphere's steam).
    pub water_out: Mass,
    /// Non-condensables out.
    pub ncg_out: [Mass; 6],
    /// Energy out.
    pub energy_out: Energy,
    /// The bubble efficiency the inflow met (0 above the level).
    pub bubble_efficiency: f64,
    /// Inflowing steam that reached the atmosphere uncondensed.
    pub steam_to_atmosphere: Mass,
}

/// What one step did.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolumeStepReport {
    /// Per port.
    pub ports: Vec<PortTransfer>,
    /// Heat removed by the immersed heat exchanger.
    pub heat_exchanger_heat: Energy,
    /// Pool liquid flashed to vapour.
    pub flashing: Mass,
    /// Net evaporation at the pool surface (negative is condensation).
    pub surface_evaporation: Mass,
    /// Vapour condensed out of a supersaturated atmosphere into the pool.
    pub rain_out: Mass,
    /// Vapour volume flow through the pool top this step, m³/s (sets the
    /// swell for the next step).
    pub vapour_flow_m3_s: f64,
    /// Steam taken by [`ControlVolumeInputs::atmosphere_steam_sink`], and
    /// the enthalpy it carried away.
    pub steam_sink: Mass,
    /// The enthalpy the sunk steam carried.
    pub steam_sink_energy: Energy,
}

/// Running totals across the boundary since construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlVolumeBookkeeping {
    /// Water in.
    pub water_in: Mass,
    /// Water out.
    pub water_out: Mass,
    /// Each non-condensable in.
    pub ncg_in: [Mass; 6],
    /// Each non-condensable out.
    pub ncg_out: [Mass; 6],
    /// Energy in (sources' enthalpy, pool heat if positive).
    pub energy_in: Energy,
    /// Energy out (outflows' enthalpy, heat exchanger, pool heat if negative).
    pub energy_out: Energy,
}

/// The state an outflow through a port would carry (for a flow path's donor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DonorState {
    /// Pool or atmosphere.
    pub draw: PortDraw,
    /// The pressure at the port.
    pub pressure: Pressure,
    /// The donor's density.
    pub density: MassDensity,
    /// The donor's specific enthalpy.
    pub specific_enthalpy: AvailableEnergy,
    /// The donor's temperature.
    pub temperature: ThermodynamicTemperature,
    /// Water's mass fraction (1 for liquid).
    pub water_mass_fraction: f64,
    /// Each non-condensable's mass fraction.
    pub ncg_mass_fractions: [f64; 6],
}

/// The liquid's thermodynamics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidModel {
    /// IAPWS-IF97 region 1 on the saturation line. The default.
    Iapws97,
    /// `sembawang`'s constant `c_p = 4186 J/(kg K)` from 273.15 K.
    ConstantCp,
}

/// Which mechanisms are on. All on by default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlVolumeMechanisms {
    /// The pool surface.
    pub surface: SurfaceModel,
    /// MELCOR's bubble efficiency (else `ε = 1`).
    pub bubble_pass_through: bool,
    /// The pool's compression work on the atmosphere.
    pub pressure_work: bool,
    /// The swollen level decides what a port draws.
    pub level_swell: bool,
    /// Rain-out to saturation (else `sembawang`'s four passes).
    pub equilibrium_rain_out: bool,
    /// The liquid.
    pub liquid: LiquidModel,
}

impl Default for ControlVolumeMechanisms {
    fn default() -> Self {
        Self {
            surface: SurfaceModel::Melcor,
            bubble_pass_through: true,
            pressure_work: true,
            level_swell: true,
            equilibrium_rain_out: true,
            liquid: LiquidModel::Iapws97,
        }
    }
}

// ── The volume ───────────────────────────────────────────────────────────────

/// Derived from the state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Derived {
    t_pool: f64,
    t_gas: f64,
    rho: f64,
    level: f64,
    v_gas: f64,
    p_gas: f64,
    rh: f64,
}

/// The control volume. See the module documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolume {
    table: VolumeAltitudeTable,
    ports: Vec<Port>,
    mech: ControlVolumeMechanisms,
    model: GasMixtureModel,
    pool_kg: f64,
    pool_e: f64,
    gas: GasMixture,
    gas_e: f64,
    d: Derived,
    vapour_flow: f64,
    water_in: f64,
    water_out: f64,
    ncg_in: [f64; 6],
    ncg_out: [f64; 6],
    e_in: f64,
    e_out: f64,
}

fn finite_nonneg(x: f64, what: &str) -> Result<f64, TampinesError> {
    if x.is_finite() && x >= 0.0 { Ok(x) } else { Err(TampinesError::InvalidInput(format!("{what} = {x}"))) }
}

fn kelv(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

impl ControlVolume {
    /// A volume of shape `table` with `ports`, holding `pool_mass` of liquid
    /// at `pool_temperature` and the atmosphere `atmosphere` at
    /// `atmosphere_temperature`, every mechanism on, the atmosphere closed by
    /// [`GasMixtureModel::new`].
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive pool mass or
    /// temperature, an empty atmosphere, a port outside the table, a
    /// non-positive opening; [`TampinesError::Unphysical`] if the pool fills
    /// the vessel or the atmosphere's closure fails.
    pub fn new(
        table: VolumeAltitudeTable,
        ports: Vec<Port>,
        pool_mass: Mass,
        pool_temperature: ThermodynamicTemperature,
        atmosphere: GasMixture,
        atmosphere_temperature: ThermodynamicTemperature,
    ) -> Result<Self, TampinesError> {
        let pool_kg = pool_mass.get::<kilogram>();
        let t_p = pool_temperature.get::<kelvin>();
        let t_g = atmosphere_temperature.get::<kelvin>();
        for (x, what) in [(pool_kg, "pool mass"), (t_p, "pool temperature"), (t_g, "atmosphere temperature")] {
            if !(x.is_finite() && x > 0.0) {
                return Err(TampinesError::InvalidInput(format!("{what} = {x}")));
            }
        }
        if !(atmosphere.steam_kg.is_finite() && atmosphere.steam_kg >= 0.0) || atmosphere.ncg_moles.iter().any(|n| !(n.is_finite() && *n >= 0.0)) {
            return Err(TampinesError::InvalidInput(format!("atmosphere {atmosphere:?}")));
        }
        if atmosphere.steam_kg + atmosphere.ncg_total_moles() <= 0.0 {
            return Err(TampinesError::InvalidInput("an empty atmosphere".into()));
        }
        for p in &ports {
            let (z, h) = (p.elevation.get::<meter>(), p.opening_height.get::<meter>());
            if !(z.is_finite() && z >= table.bottom_m() && z <= table.top_m() && h.is_finite() && h > 0.0) {
                return Err(TampinesError::InvalidInput(format!("port {p:?} outside the vessel or with no opening")));
            }
        }
        let mut cv = Self {
            table,
            ports,
            mech: ControlVolumeMechanisms::default(),
            model: GasMixtureModel::new(),
            pool_kg,
            pool_e: 0.0,
            gas: atmosphere,
            gas_e: 0.0,
            d: Derived { t_pool: t_p, t_gas: t_g, rho: 1000.0, level: 0.0, v_gas: 0.0, p_gas: 0.0, rh: 0.0 },
            vapour_flow: 0.0,
            water_in: 0.0,
            water_out: 0.0,
            ncg_in: [0.0; 6],
            ncg_out: [0.0; 6],
            e_in: 0.0,
            e_out: 0.0,
        };
        cv.reset_energies(t_p, t_g)?;
        Ok(cv)
    }

    /// Set the energies from the temperatures under the current models.
    fn reset_energies(&mut self, t_p: f64, t_g: f64) -> Result<(), TampinesError> {
        self.pool_e = self.pool_kg * self.u_liquid(t_p);
        self.d.t_pool = t_p;
        // The atmosphere's volume depends on the pool density, which depends
        // on the pressure: settle it at the given temperature first.
        let mut rho = rho_liquid(t_p, 0.0);
        for _ in 0..3 {
            let v_gas = self.table.total_m3() - self.pool_kg / rho;
            if !(v_gas > 0.0) {
                return Err(TampinesError::Unphysical(format!("the pool fills the vessel ({} m3 of gas space)", v_gas)));
            }
            let st = self.gas_state(v_gas, t_g)?;
            rho = rho_liquid(t_p, st.0);
            self.gas_e = st.1;
        }
        self.d.t_gas = t_g;
        self.refresh()
    }

    fn rebuild(mut self) -> Result<Self, TampinesError> {
        let (t_p, t_g) = (self.d.t_pool, self.d.t_gas);
        self.reset_energies(t_p, t_g)?;
        Ok(self)
    }

    /// Ablation: RELAP-7 Eq 390's sensible heat `A α (T_pool − T_atm)`
    /// replaces MELCOR's surface (no surface evaporation or condensation;
    /// flashing and rain-out still act), `A` the free cross-section at the
    /// level.
    pub fn without_surface_mass_transfer(mut self, coefficient: HeatTransfer) -> Self {
        self.mech.surface = SurfaceModel::SensibleOnly { coefficient };
        self
    }
    /// Ablation: full bubble interaction always (`ε = 1`).
    pub fn without_bubble_pass_through(mut self) -> Self {
        self.mech.bubble_pass_through = false;
        self
    }
    /// Ablation: no compression work from the pool on the atmosphere.
    pub fn without_pressure_work(mut self) -> Self {
        self.mech.pressure_work = false;
        self
    }
    /// Ablation: ports draw by the collapsed level.
    pub fn without_level_swell(mut self) -> Self {
        self.mech.level_swell = false;
        self
    }
    /// Ablation: `sembawang`'s rain-out (four passes, each condensing to
    /// saturation at the current temperature and leaving the latent heat in
    /// the gas, so it ends subsaturated).
    pub fn without_equilibrium_rain_out(mut self) -> Self {
        self.mech.equilibrium_rain_out = false;
        self
    }
    /// Ablation: `sembawang`'s constant-`c_p` liquid. The pool's temperature
    /// is kept and its energy re-expressed.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn without_real_liquid(mut self) -> Result<Self, TampinesError> {
        self.mech.liquid = LiquidModel::ConstantCp;
        self.rebuild()
    }
    /// Ablation: `sembawang`'s ideal constant-`c_v` steam in the atmosphere
    /// ([`GasMixtureModel::without_real_steam`]). Temperatures kept.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn without_real_steam(mut self) -> Result<Self, TampinesError> {
        self.model = self.model.without_real_steam();
        self.rebuild()
    }
    /// Ablation: constant non-condensable `c_v`
    /// ([`GasMixtureModel::without_ncg_cv_t`]). Temperatures kept.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn without_ncg_cv_t(mut self) -> Result<Self, TampinesError> {
        self.model = self.model.without_ncg_cv_t();
        self.rebuild()
    }
    /// The mechanisms in use.
    pub fn mechanisms(&self) -> ControlVolumeMechanisms {
        self.mech
    }
    /// The atmosphere's closure.
    pub fn gas_model(&self) -> GasMixtureModel {
        self.model
    }

    // ── Thermodynamics ───────────────────────────────────────────────────

    /// The liquid's specific internal energy at `t`, J/kg.
    fn u_liquid(&self, t: f64) -> f64 {
        match self.mech.liquid {
            LiquidModel::ConstantCp => CP_WATER * (t - 273.15),
            LiquidModel::Iapws97 => {
                let tc = t.clamp(273.16, 640.0);
                u_tp_1(kelv(t), Pressure::new::<pascal>(1.0001 * p_sat_pa(tc))).get::<joule_per_kilogram>()
            }
        }
    }
    /// The liquid's `du/dT`, J/(kg K).
    fn cv_liquid(&self, t: f64) -> f64 {
        match self.mech.liquid {
            LiquidModel::ConstantCp => CP_WATER,
            LiquidModel::Iapws97 => (self.u_liquid(t + 0.01) - self.u_liquid(t - 0.01)) / 0.02,
        }
    }
    /// The liquid's specific enthalpy at `t` and `p`, J/kg.
    fn h_liquid(&self, t: f64, p: f64) -> f64 {
        match self.mech.liquid {
            LiquidModel::ConstantCp => CP_WATER * (t - 273.15),
            LiquidModel::Iapws97 => self.u_liquid(t) + p / rho_liquid(t, p),
        }
    }
    /// The pool temperature from its specific energy `u`.
    fn t_liquid(&self, u: f64, guess: f64) -> f64 {
        match self.mech.liquid {
            LiquidModel::ConstantCp => 273.15 + u / CP_WATER,
            LiquidModel::Iapws97 => {
                let mut t = guess;
                for _ in 0..50 {
                    let dt = (u - self.u_liquid(t)) / self.cv_liquid(t);
                    t += dt;
                    if dt.abs() < 1e-13 * t {
                        break;
                    }
                }
                t
            }
        }
    }
    /// Steam's specific internal energy at `t` and partial pressure `p_s`.
    fn u_steam(&self, t: f64, p_s: f64) -> f64 {
        match self.model.steam {
            SteamModel::IdealConstantCv => U_STEAM_REF + CV_STEAM * (t - T_REF),
            SteamModel::Iapws97 => u_tp_2(kelv(t), Pressure::new::<pascal>(p_s.max(1.0))).get::<joule_per_kilogram>(),
        }
    }
    /// Steam's specific enthalpy at `t` and `p_s`.
    fn h_steam(&self, t: f64, p_s: f64) -> f64 {
        match self.model.steam {
            SteamModel::IdealConstantCv => steam_h(t),
            SteamModel::Iapws97 => h_tp_2(kelv(t), Pressure::new::<pascal>(p_s.max(1.0))).get::<joule_per_kilogram>(),
        }
    }
    /// The steam mass `v` m³ holds saturated at `t`, kg.
    fn steam_capacity(&self, t: f64, v: f64) -> f64 {
        match self.model.steam {
            SteamModel::IdealConstantCv => p_sat_pa(t.min(640.0)) * v * M_H2O / (R_GAS * t),
            SteamModel::Iapws97 => {
                let tc = t.min(640.0);
                v / v_tp_2(kelv(tc), Pressure::new::<pascal>(p_sat_pa(tc))).get::<cubic_meter_per_kilogram>()
            }
        }
    }
    /// The atmosphere's pressure and energy at `(v, t)`.
    fn gas_state(&self, v: f64, t: f64) -> Result<(f64, f64, f64), TampinesError> {
        let st = self
            .model
            .state(&self.gas, Volume::new::<cubic_meter>(v), kelv(t))
            .map_err(|e| TampinesError::Unphysical(format!("atmosphere: {e}")))?;
        Ok((st.pressure.get::<pascal>(), st.internal_energy.get::<joule>(), st.relative_humidity))
    }
    /// The atmosphere's temperature holding energy `e` in `v`, by Newton from
    /// `guess` (as the wet well's `set_gas_energy`).
    fn t_gas_from(&self, e: f64, v: f64, guess: f64) -> Result<f64, TampinesError> {
        let mut t = guess;
        for _ in 0..60 {
            let u = self.gas_state(v, t)?.1;
            let c = self
                .model
                .heat_capacity(&self.gas, Volume::new::<cubic_meter>(v), kelv(t))
                .map_err(|e| TampinesError::Unphysical(format!("atmosphere: {e}")))?
                .get::<uom::si::heat_capacity::joule_per_kelvin>();
            let dt = (e - u) / c;
            t += dt;
            if !(t.is_finite() && t > 0.0) {
                return Err(TampinesError::Unphysical(format!("atmosphere temperature {t} K")));
            }
            if dt.abs() < 1e-12 * t {
                return Ok(t);
            }
        }
        Err(TampinesError::Unphysical("atmosphere temperature did not converge".into()))
    }

    /// Re-derive temperatures, density, level, gas volume and pressure from
    /// the state: the density at the atmosphere's pressure, three
    /// fixed-point passes from the density at saturation (the wet well's
    /// `derived`).
    fn refresh(&mut self) -> Result<(), TampinesError> {
        let t_pool = self.t_liquid(self.pool_e / self.pool_kg, self.d.t_pool);
        let mut t_gas = self.d.t_gas;
        let mut rho = rho_liquid(t_pool, 0.0);
        let mut d = self.d;
        for _ in 0..3 {
            let v_pool = self.pool_kg / rho;
            let v_gas = self.table.total_m3() - v_pool;
            if !(v_gas > 0.0) {
                return Err(TampinesError::Unphysical(format!("the pool fills the vessel ({v_gas} m3 of gas space)")));
            }
            let level = self.table.height_of_volume_m(v_pool);
            t_gas = self.t_gas_from(self.gas_e, v_gas, t_gas)?;
            let (p_gas, _, rh) = self.gas_state(v_gas, t_gas)?;
            d = Derived { t_pool, t_gas, rho, level, v_gas, p_gas, rh };
            rho = rho_liquid(t_pool, p_gas);
        }
        if !(t_pool.is_finite() && t_pool > 0.0) {
            return Err(TampinesError::Unphysical(format!("pool temperature {t_pool} K")));
        }
        self.d = d;
        Ok(())
    }

    fn add_pool(&mut self, dm: f64, de: f64) -> Result<(), TampinesError> {
        self.pool_kg += dm;
        self.pool_e += de;
        if !(self.pool_kg > 0.0) {
            return Err(TampinesError::Unphysical(format!("pool mass {} kg", self.pool_kg)));
        }
        self.refresh()
    }

    // ── Getters ──────────────────────────────────────────────────────────

    /// The atmosphere's pressure.
    pub fn atmosphere_pressure(&self) -> Pressure {
        Pressure::new::<pascal>(self.d.p_gas)
    }
    /// The atmosphere's temperature.
    pub fn atmosphere_temperature(&self) -> ThermodynamicTemperature {
        kelv(self.d.t_gas)
    }
    /// The atmosphere's contents.
    pub fn atmosphere(&self) -> GasMixture {
        self.gas
    }
    /// The atmosphere's volume (the vessel less the collapsed pool).
    pub fn atmosphere_volume(&self) -> Volume {
        Volume::new::<cubic_meter>(self.d.v_gas)
    }
    /// The atmosphere's relative humidity (`p_s / p_sat(T)`).
    pub fn relative_humidity(&self) -> f64 {
        self.d.rh
    }
    /// Pool temperature.
    pub fn pool_temperature(&self) -> ThermodynamicTemperature {
        kelv(self.d.t_pool)
    }
    /// Pool mass.
    pub fn pool_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.pool_kg)
    }
    /// Pool density (IF97 region 1 at the atmosphere's pressure).
    pub fn pool_density(&self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.d.rho)
    }
    /// The collapsed pool level.
    pub fn pool_level(&self) -> Length {
        Length::new::<meter>(self.d.level)
    }
    /// The swollen level, from the vapour that rose through the pool in the
    /// last step (MELCOR CVH/FL §5.1.3; `sembawang` likewise uses the last
    /// step's vapour). Reported whether or not the swell is used for ports.
    pub fn swollen_level(&self) -> Length {
        let t = &self.table;
        Length::new::<meter>(swollen_level_from_vapour_flow(
            |z| t.volume_below_m3(z),
            |v| t.height_of_volume_m(v),
            self.d.level,
            self.vapour_flow,
            BUBBLE_RISE_M_S,
            MAX_POOL_VOID,
        ))
    }
    /// The level ports draw by: swollen, or collapsed when ablated.
    fn draw_level(&self) -> f64 {
        if self.mech.level_swell { self.swollen_level().get::<meter>() } else { self.d.level }
    }
    /// Water in the volume (pool plus steam).
    pub fn water_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.pool_kg + self.gas.steam_kg)
    }
    /// Each non-condensable's mass.
    pub fn ncg_masses(&self) -> [Mass; 6] {
        NonCondensable::ALL.map(|g| Mass::new::<kilogram>(self.gas.ncg_moles[g.index()] * g.molar_mass()))
    }
    /// Total internal energy, pool plus atmosphere.
    pub fn total_energy(&self) -> Energy {
        Energy::new::<joule>(self.pool_e + self.gas_e)
    }
    /// The ports.
    pub fn ports(&self) -> &[Port] {
        &self.ports
    }
    /// The shape.
    pub fn table(&self) -> &VolumeAltitudeTable {
        &self.table
    }
    /// Running totals across the boundary.
    pub fn bookkeeping(&self) -> ControlVolumeBookkeeping {
        let kg = |n: f64| Mass::new::<kilogram>(n);
        ControlVolumeBookkeeping {
            water_in: kg(self.water_in),
            water_out: kg(self.water_out),
            ncg_in: self.ncg_in.map(kg),
            ncg_out: self.ncg_out.map(kg),
            energy_in: Energy::new::<joule>(self.e_in),
            energy_out: Energy::new::<joule>(self.e_out),
        }
    }
    /// The pressure at elevation `z`: the atmosphere's plus `ρ g (L − z)`
    /// below the collapsed level, the atmosphere's above it (as
    /// `VolumePort::AtHeight`, and the wet well's junction pressure).
    pub fn pressure_at_elevation(&self, z: Length) -> Pressure {
        Pressure::new::<pascal>(self.p_at(z.get::<meter>()))
    }
    fn p_at(&self, z: f64) -> f64 {
        self.d.p_gas + self.d.rho * G * (self.d.level - z).max(0.0)
    }
    /// What an outflow through port `i` would carry now.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such port.
    pub fn port_donor(&self, i: usize) -> Result<DonorState, TampinesError> {
        let port = self.ports.get(i).ok_or_else(|| TampinesError::InvalidInput(format!("no port {i}")))?;
        let z = port.elevation.get::<meter>();
        let (liq, gas) = self.phase_states_at(port.elevation);
        Ok(if z < self.draw_level() { liq } else { gas })
    }

    /// The pool liquid's and the atmosphere's states at elevation `z`, both
    /// at the pressure there, whatever the level (gh:#1095: a flow path needs
    /// both phases' properties at each end, and decides the split itself).
    pub fn phase_states_at(&self, z: Length) -> (DonorState, DonorState) {
        let p = self.p_at(z.get::<meter>());
        let liquid = DonorState {
            draw: PortDraw::Liquid,
            pressure: Pressure::new::<pascal>(p),
            density: self.pool_density(),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(self.h_liquid(self.d.t_pool, p)),
            temperature: kelv(self.d.t_pool),
            water_mass_fraction: 1.0,
            ncg_mass_fractions: [0.0; 6],
        };
        let m = self.gas.total_mass().get::<kilogram>();
        let gas = DonorState {
            draw: PortDraw::Atmosphere,
            pressure: Pressure::new::<pascal>(p),
            density: MassDensity::new::<kilogram_per_cubic_meter>(m / self.d.v_gas),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>((self.gas_e + self.d.p_gas * self.d.v_gas) / m),
            temperature: kelv(self.d.t_gas),
            water_mass_fraction: self.gas.steam_kg / m,
            ncg_mass_fractions: NonCondensable::ALL.map(|g| self.gas.ncg_moles[g.index()] * g.molar_mass() / m),
        };
        (liquid, gas)
    }


    // ── Step ─────────────────────────────────────────────────────────────

    /// Advance by `dt`, in the wet well's order: every port's inflow (port
    /// order) through the bubble model, the pool heat and the heat exchanger,
    /// every port's outflow (port order) by the level, the pool surface
    /// (flashing, exchange, rain-out), and the pool's compression work.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`, a wrong
    /// number of port inputs, a negative or non-finite flow, or an outflow
    /// larger than what it draws on; [`TampinesError::Unphysical`] if the pool
    /// fills the vessel or a closure fails.
    pub fn step(&mut self, dt: Time, inputs: &ControlVolumeInputs) -> Result<ControlVolumeStepReport, TampinesError> {
        let dt = dt.get::<second>();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("dt = {dt}")));
        }
        if inputs.ports.len() != self.ports.len() {
            return Err(TampinesError::InvalidInput(format!("{} port inputs for {} ports", inputs.ports.len(), self.ports.len())));
        }
        let q_heat = inputs.pool_heat.get::<watt>();
        let ua = finite_nonneg(inputs.heat_exchanger.conductance.get::<watt_per_kelvin>(), "heat-exchanger UA")?;
        let t_hx = inputs.heat_exchanger.coolant_temperature.get::<kelvin>();
        if !q_heat.is_finite() || !(t_hx.is_finite() && t_hx > 0.0) {
            return Err(TampinesError::InvalidInput(format!("pool heat {q_heat} W / heat exchanger {:?}", inputs.heat_exchanger)));
        }
        let start = self.d;
        let v_pool_start = self.pool_kg / start.rho;
        let pressures: Vec<f64> = self.ports.iter().map(|p| self.p_at(p.elevation.get::<meter>())).collect();
        let mut transfers: Vec<PortTransfer> = pressures
            .iter()
            .map(|p| PortTransfer {
                pressure: Pressure::new::<pascal>(*p),
                draw: PortDraw::Atmosphere,
                water_in: Mass::new::<kilogram>(0.0),
                ncg_in: [Mass::new::<kilogram>(0.0); 6],
                energy_in: Energy::new::<joule>(0.0),
                water_out: Mass::new::<kilogram>(0.0),
                ncg_out: [Mass::new::<kilogram>(0.0); 6],
                energy_out: Energy::new::<joule>(0.0),
                bubble_efficiency: 0.0,
                steam_to_atmosphere: Mass::new::<kilogram>(0.0),
            })
            .collect();
        let mut vapour_volume = 0.0;

        // 1. Inflows.
        for i in 0..self.ports.len() {
            let src = inputs.ports[i].inflow;
            let m = finite_nonneg(src.water_mass_rate.get::<kilogram_per_second>(), "water inflow")? * dt;
            let h = src.water_specific_enthalpy.get::<joule_per_kilogram>();
            let t_n = src.ncg_temperature.get::<kelvin>();
            let mut dn = [0.0; 6];
            for g in NonCondensable::ALL {
                dn[g.index()] = finite_nonneg(src.ncg_mass_rates[g.index()].get::<kilogram_per_second>(), "ncg inflow")? * dt / g.molar_mass();
            }
            if !h.is_finite() || !(t_n.is_finite() && t_n > 0.0) {
                return Err(TampinesError::InvalidInput(format!("source {src:?}")));
            }
            let submerged = self.ports[i].elevation.get::<meter>() < self.d.level;
            let (eps, to_atm, vol) = self.inject(i, m, h, dn, t_n)?;
            if submerged {
                vapour_volume += vol;
            }
            let tr = &mut transfers[i];
            tr.water_in = Mass::new::<kilogram>(m);
            tr.ncg_in = NonCondensable::ALL.map(|g| Mass::new::<kilogram>(dn[g.index()] * g.molar_mass()));
            tr.energy_in = Energy::new::<joule>(m * h + self.ncg_enthalpy(&dn, t_n));
            tr.bubble_efficiency = eps;
            tr.steam_to_atmosphere = Mass::new::<kilogram>(to_atm);
        }

        // 2. Pool heat (explicit) and the immersed heat exchanger, implicit
        //    in the pool temperature (`q = k ΔT / (1 + k / C_pool)`).
        if q_heat != 0.0 {
            let q = q_heat * dt;
            if q > 0.0 {
                self.e_in += q;
            } else {
                self.e_out -= q;
            }
            self.add_pool(0.0, q)?;
        }
        // 2b. The caller's structures on the atmosphere (gh:#1097): heat,
        //     then condensation, both explicit.
        let q_atm = inputs.atmosphere_heat.get::<watt>();
        if !q_atm.is_finite() {
            return Err(TampinesError::InvalidInput(format!("atmosphere heat {q_atm} W")));
        }
        if q_atm != 0.0 {
            let q = q_atm * dt;
            if q > 0.0 {
                self.e_in += q;
            } else {
                self.e_out -= q;
            }
            self.gas_e += q;
            self.refresh()?;
        }
        let m_sink = finite_nonneg(inputs.atmosphere_steam_sink.get::<kilogram_per_second>(), "atmosphere steam sink")? * dt;
        let mut e_sink = 0.0;
        if m_sink > 0.0 {
            if m_sink >= self.gas.steam_kg {
                return Err(TampinesError::InvalidInput(format!("steam sink {m_sink} kg not below the atmosphere's {} kg", self.gas.steam_kg)));
            }
            let n_s = self.gas.steam_kg / M_H2O;
            let p_s = self.d.p_gas * n_s / (n_s + self.gas.ncg_total_moles());
            e_sink = m_sink * self.h_steam(self.d.t_gas, p_s);
            self.gas.steam_kg -= m_sink;
            self.gas_e -= e_sink;
            self.water_out += m_sink;
            self.e_out += e_sink;
            self.refresh()?;
        }
        let k = ua * dt;
        let q_hx = k * (self.d.t_pool - t_hx) / (1.0 + k / (self.pool_kg * self.cv_liquid(self.d.t_pool)));
        if q_hx != 0.0 {
            self.add_pool(0.0, -q_hx)?;
            self.e_out += q_hx;
        }

        // 3. Outflows, by the level.
        for i in 0..self.ports.len() {
            let m = finite_nonneg(inputs.ports[i].outflow.get::<kilogram_per_second>(), "outflow")? * dt;
            let z = self.ports[i].elevation.get::<meter>();
            let draw = if z < self.draw_level() { PortDraw::Liquid } else { PortDraw::Atmosphere };
            transfers[i].draw = draw;
            if m <= 0.0 {
                continue;
            }
            match draw {
                PortDraw::Liquid => {
                    if m >= self.pool_kg {
                        return Err(TampinesError::InvalidInput(format!("outflow {m} kg not below the pool's {} kg", self.pool_kg)));
                    }
                    let e = m * self.h_liquid(self.d.t_pool, pressures[i]);
                    self.add_pool(-m, -e)?;
                    self.water_out += m;
                    self.e_out += e;
                    transfers[i].water_out = Mass::new::<kilogram>(m);
                    transfers[i].energy_out = Energy::new::<joule>(e);
                }
                PortDraw::Atmosphere => {
                    let m_gas = self.gas.total_mass().get::<kilogram>();
                    if m >= m_gas {
                        return Err(TampinesError::InvalidInput(format!("outflow {m} kg not below the atmosphere's {m_gas} kg")));
                    }
                    let f = m / m_gas;
                    let e = f * (self.gas_e + self.d.p_gas * self.d.v_gas);
                    let ds = f * self.gas.steam_kg;
                    let mut out = [0.0; 6];
                    for g in NonCondensable::ALL {
                        let j = g.index();
                        let dn = f * self.gas.ncg_moles[j];
                        self.gas.ncg_moles[j] -= dn;
                        out[j] = dn * g.molar_mass();
                        self.ncg_out[j] += out[j];
                    }
                    self.gas.steam_kg -= ds;
                    self.gas_e -= e;
                    self.refresh()?;
                    self.water_out += ds;
                    self.e_out += e;
                    transfers[i].water_out = Mass::new::<kilogram>(ds);
                    transfers[i].ncg_out = out.map(|x| Mass::new::<kilogram>(x));
                    transfers[i].energy_out = Energy::new::<joule>(e);
                }
            }
        }

        // 4. The pool surface.
        let (flash, evap, rain) = self.surface(dt)?;
        vapour_volume += flash * R_GAS * self.d.t_pool / (M_H2O * self.d.p_gas);

        // 5. The pool's compression work on the atmosphere, `p ΔV_pool` at
        //    the mean of the start and end pressures (internal: the vessel is
        //    rigid).
        if self.mech.pressure_work {
            let w = 0.5 * (start.p_gas + self.d.p_gas) * (self.pool_kg / self.d.rho - v_pool_start);
            if w != 0.0 {
                self.pool_e -= w;
                self.gas_e += w;
                self.refresh()?;
            }
        }
        self.vapour_flow = vapour_volume / dt;
        Ok(ControlVolumeStepReport {
            ports: transfers,
            heat_exchanger_heat: Energy::new::<joule>(q_hx),
            flashing: Mass::new::<kilogram>(flash),
            surface_evaporation: Mass::new::<kilogram>(evap),
            rain_out: Mass::new::<kilogram>(rain),
            vapour_flow_m3_s: self.vapour_flow,
            steam_sink: Mass::new::<kilogram>(m_sink),
            steam_sink_energy: Energy::new::<joule>(e_sink),
        })
    }

    /// The enthalpy of `dn` moles of non-condensables at `t`, J.
    fn ncg_enthalpy(&self, dn: &[f64; 6], t: f64) -> f64 {
        NonCondensable::ALL.iter().map(|g| dn[g.index()] * (self.model.ncg_u(*g, t) + R_GAS * t)).sum()
    }

    /// One port's inflow: `m` kg of water at `h` and `dn` mol of
    /// non-condensables at `t_n`. **The wet well's `inject`**: a two-phase
    /// source is split by IF97's saturation enthalpies at the port's
    /// pressure; its liquid joins the pool, its vapour and the gases go
    /// through [`Self::bubble_through`]. Returns `ε`, the steam reaching the
    /// atmosphere, kg, and the gas volume that rose, m³ (at the pool's
    /// temperature and the atmosphere's pressure, ideal).
    fn inject(&mut self, port: usize, m: f64, h: f64, dn: [f64; 6], t_n: f64) -> Result<(f64, f64, f64), TampinesError> {
        let n_tot: f64 = dn.iter().sum();
        if m <= 0.0 && n_tot <= 0.0 {
            return Ok((0.0, 0.0, 0.0));
        }
        let e_in = m * h;
        let e_n = self.ncg_enthalpy(&dn, t_n);
        self.water_in += m;
        for g in NonCondensable::ALL {
            self.ncg_in[g.index()] += dn[g.index()] * g.molar_mass();
        }
        self.e_in += e_in + e_n;
        let z = self.ports[port].elevation.get::<meter>();
        let (m_v, e_v) = if m > 0.0 {
            let p_j = self.p_at(z).min(2.0e7);
            let t_s = kelv(t_sat(p_j));
            let p = Pressure::new::<pascal>(p_j);
            let h_f = h_tp_eqm_two_phase(t_s, p, 0.0).get::<joule_per_kilogram>();
            let h_g = h_tp_eqm_two_phase(t_s, p, 1.0).get::<joule_per_kilogram>();
            let x = ((h - h_f) / (h_g - h_f)).clamp(0.0, 1.0);
            let m_l = (1.0 - x) * m;
            let e_l = m_l * h.min(h_f);
            if m_l > 0.0 {
                self.add_pool(m_l, e_l)?;
            }
            (m - m_l, e_in - e_l)
        } else {
            (0.0, 0.0)
        };
        self.bubble_through(port, dn, m_v, e_v + e_n)
    }

    /// The bubble efficiency at port `i` now (0 above the level).
    fn efficiency(&self, i: usize) -> f64 {
        if !self.mech.bubble_pass_through {
            return 1.0;
        }
        let depth = self.d.level - self.ports[i].elevation.get::<meter>();
        bubble_efficiency(depth, self.ports[i].opening_height.get::<meter>(), self.d.p_gas + self.d.rho * G * depth.max(0.0), self.d.t_pool)
    }

    /// Gas entering at port `i`: `dn` mol of non-condensables and `m_s` kg of
    /// steam carrying `e_in` J. **The wet well's `bubble_through`**
    /// (`sembawang`'s `Containment::bubble_through`, MELCOR CVH/FL RM §6.1)
    /// with every non-condensable: between no interaction (all to the
    /// atmosphere unchanged) and full interaction (the gases leave at the
    /// pool's temperature with vapour at 0.99 of its saturation pressure, the
    /// rest of the steam and energy stay in the pool), linearly in `ε`.
    fn bubble_through(&mut self, i: usize, dn: [f64; 6], m_s: f64, e_in: f64) -> Result<(f64, f64, f64), TampinesError> {
        let p_g = self.d.p_gas;
        let t_p = self.d.t_pool;
        let eps = self.efficiency(i);
        let n_tot: f64 = dn.iter().sum();
        let p_v = (0.99 * p_sat_pa(t_p)).min(0.99 * p_g);
        let m_v = n_tot * p_v / (p_g - p_v) * M_H2O;
        let u_n: f64 = NonCondensable::ALL.iter().map(|g| dn[g.index()] * self.model.ncg_u(*g, t_p)).sum();
        let e_full = u_n + m_v * self.u_steam(t_p, p_v);
        let to_atm_steam = eps * m_v + (1.0 - eps) * m_s;
        let to_atm_e = eps * e_full + (1.0 - eps) * e_in;
        for g in NonCondensable::ALL {
            self.gas.ncg_moles[g.index()] += dn[g.index()];
        }
        self.gas.steam_kg += to_atm_steam;
        self.gas_e += to_atm_e;
        self.refresh()?;
        self.add_pool(m_s - to_atm_steam, e_in - to_atm_e)?;
        let vol = (n_tot + to_atm_steam / M_H2O) * R_GAS * t_p / p_g;
        Ok((eps, to_atm_steam, vol))
    }

    /// The pool surface. **The wet well's `surface`** (`sembawang`'s
    /// `Containment::pool_surface_step`): flashing of a pool above the
    /// saturation temperature at the atmosphere's pressure, MELCOR's surface
    /// exchange (or the sensible-only ablation), and rain-out. Returns the
    /// flashed, the surface-evaporated and the rained-out masses, kg.
    fn surface(&mut self, dt: f64) -> Result<(f64, f64, f64), TampinesError> {
        // Flashing: the vapour leaves as saturated steam at `T_s`; the pool
        // ends at `T_s` (energy: `E_p = (m − x) u_l(T_s) + x h_g(T_s)`).
        let t_s = t_sat(self.d.p_gas);
        let mut flash = 0.0;
        if self.d.t_pool > t_s {
            let h_g = self.h_steam(t_s, p_sat_pa(t_s));
            let u_l = self.u_liquid(t_s);
            let m = (self.pool_e - self.pool_kg * u_l) / (h_g - u_l);
            let e = m * h_g;
            self.gas.steam_kg += m;
            self.gas_e += e;
            self.pool_kg -= m;
            self.pool_e -= e;
            self.refresh()?;
            flash = m;
        }
        let mut evap = 0.0;
        match self.mech.surface {
            SurfaceModel::Melcor => {
                let d = self.d;
                let area = self.table.area_at(Length::new::<meter>(d.level)).get::<uom::si::area::square_meter>();
                let cp = self.cv_liquid(d.t_pool);
                let ex = exchange(
                    &PoolState { t_k: d.t_pool, kg: self.pool_kg, rho: d.rho, cp },
                    &Atmosphere { t_k: d.t_gas, p_pa: d.p_gas, n_steam: self.gas.steam_kg / M_H2O, n_n2: self.gas.ncg_total_moles() },
                    &SurfaceGeometry { area_m2: area, pool_depth_m: d.level - self.table.bottom_m(), gas_depth_m: d.v_gas / area },
                    dt,
                );
                let m = ex.evaporation_kg_s * dt;
                // The vapour leaves the pool at the liquid's enthalpy and joins
                // the atmosphere at the steam's; MELCOR's balance set `m` with
                // `exchange`'s own `h_fg`, so the difference (zero under the
                // constant-`c_p`, ideal-steam ablations) is taken from the
                // atmosphere's heat so energy closes on this volume's values.
                let p_s = self.gas.steam_kg / M_H2O / (self.gas.steam_kg / M_H2O + self.gas.ncg_total_moles()) * d.p_gas;
                let h_f = self.h_liquid(d.t_pool, d.p_gas);
                let h_g = self.h_steam(d.t_gas, p_s);
                let corr = m * ((h_g - h_f) - (steam_h(d.t_gas) - cp * (d.t_pool - 273.15)));
                let e_p = m * h_f + ex.q_pool_w * dt;
                let e_a = m * h_g - ex.q_atm_w * dt - corr;
                self.pool_kg -= m;
                self.pool_e -= e_p;
                self.gas.steam_kg += m;
                self.gas_e += e_a;
                self.refresh()?;
                evap = m;
            }
            SurfaceModel::SensibleOnly { coefficient } => {
                let d = self.d;
                let area = self.table.area_at(Length::new::<meter>(d.level)).get::<uom::si::area::square_meter>();
                let k = area * coefficient.get::<watt_per_square_meter_kelvin>() * dt;
                let c_g = self
                    .model
                    .heat_capacity(&self.gas, Volume::new::<cubic_meter>(d.v_gas), kelv(d.t_gas))
                    .map_err(|e| TampinesError::Unphysical(format!("atmosphere: {e}")))?
                    .get::<uom::si::heat_capacity::joule_per_kelvin>();
                let c_p = self.pool_kg * self.cv_liquid(d.t_pool);
                let q = k * (d.t_pool - d.t_gas) / (1.0 + k / c_g + k / c_p);
                self.pool_e -= q;
                self.gas_e += q;
                self.refresh()?;
            }
        }
        let rain = self.rain_out()?;
        Ok((flash, evap, rain))
    }

    /// Condense vapour above saturation into the pool. The condensate leaves
    /// at the liquid's enthalpy at the atmosphere's temperature before
    /// condensing; its latent heat stays in the atmosphere.
    ///
    /// Default: solve, by bisection on the condensed mass, for the state in
    /// which the remaining steam saturates the atmosphere at the temperature
    /// its energy gives (RH = 1 to 1e-12 of the steam). Ablated:
    /// `sembawang`'s four passes, each condensing to saturation at the
    /// current temperature, which end subsaturated because the latent heat
    /// warms the gas after each.
    fn rain_out(&mut self) -> Result<f64, TampinesError> {
        let mut total = 0.0;
        if !self.mech.equilibrium_rain_out {
            for _ in 0..4 {
                let keep = self.steam_capacity(self.d.t_gas, self.d.v_gas);
                if self.gas.steam_kg <= keep {
                    break;
                }
                let cond = self.gas.steam_kg - keep;
                let e = cond * self.h_liquid(self.d.t_gas, self.d.p_gas);
                self.gas.steam_kg = keep;
                self.gas_e -= e;
                self.refresh()?;
                self.add_pool(cond, e)?;
                total += cond;
            }
            return Ok(total);
        }
        let keep0 = self.steam_capacity(self.d.t_gas, self.d.v_gas);
        let s0 = self.gas.steam_kg;
        if s0 <= keep0 {
            return Ok(0.0);
        }
        let h_l = self.h_liquid(self.d.t_gas, self.d.p_gas);
        let (e0, v, t0, rho) = (self.gas_e, self.d.v_gas, self.d.t_gas, self.d.rho);
        // Residual: steam left minus what the atmosphere holds saturated at
        // the temperature the remaining energy gives, in the atmosphere's
        // volume less the condensate's `cond / ρ` (the pool's density held at
        // its value before; the last `refresh` takes up the rest).
        let mut probe = self.clone();
        let mut resid = |cond: f64| -> Result<(f64, f64), TampinesError> {
            probe.gas.steam_kg = s0 - cond;
            let vc = v - cond / rho;
            let t = probe.t_gas_from(e0 - cond * h_l, vc, t0)?;
            Ok((s0 - cond - probe.steam_capacity(t, vc), t))
        };
        let (mut lo, mut hi) = (0.0, s0 - keep0);
        // At `cond = s0 - keep0` the latent heat has warmed the gas, so the
        // residual is negative there; the root lies in [0, s0 - keep0].
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if resid(mid)?.0 > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo <= 1e-13 * s0 {
                break;
            }
        }
        let cond = 0.5 * (lo + hi);
        let e = cond * h_l;
        self.gas.steam_kg = s0 - cond;
        self.gas_e -= e;
        self.refresh()?;
        self.add_pool(cond, e)?;
        total += cond;
        Ok(total)
    }
}
