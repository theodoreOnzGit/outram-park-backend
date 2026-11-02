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
//! | ~~rain-out by `sembawang`'s four passes (ends subsaturated)~~ since 2026-11-02 the same solve as here, the four passes its ablation (gh:#1118) | rain-out to saturation (RH = 1) by a solve; the four passes the ablation |
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
//! | flashing, the surface step, the sensible-only ablation | the wet well's `surface` (`sembawang`'s `Containment::pool_surface_step`); ~~the flash explicit, as there~~ **CORRECTED 2026-11-01** (gh:#1107): the flash's end state is solved with the pressure its own vapour raises (after MELCOR CVH/FL RM §4.2.1, §4.3, Eq 5-30; [`ControlVolume::without_implicit_flash`] keeps the ported explicit form) |
//! | the split of a two-phase source into pool liquid and bubbles | the wet well's `inject` |
//! | the pool's compression work on the atmosphere | the wet well's step 6 |
//! | the immersed heat exchanger's implicit form | the wet well's step 3 (`sembawang`'s `ww_shell_step`) |
//! | [`level::swollen_level`] | `sembawang::bwr_mark1::swollen_level` (gh:#1036) |
//! | the atmosphere's pressure, energy, relative humidity | `tampines_steam_tables::gas_mixture` (gh:#1093) |
//! | the port's draw by the level | `tampines_steam_tables::hem::VolumePort::AtHeight` (gh:#1073): liquid below the level with `ρ g (L − z)`, vapour above |
//!
//! ## Several non-condensables at the pool surface
//!
//! ~~MELCOR's surface correlations are ported for steam and **one**
//! non-condensable with N2's properties. The control volume passes the
//! **total** non-condensable moles as that gas: so every non-condensable's
//! mole fraction enters the steam's partial pressure and the diffusion
//! driving force `ln((p − p_w)/(p − p_sat(T_s)))`, but the diffusivity
//! (MELCOR's steam–air correlation), the mixture viscosity and conductivity,
//! the density (natural-convection buoyancy) and `c_p` are N2's. For an
//! H2-rich atmosphere this misstates the density (and so the convection) and
//! the diffusivity (H2–steam is several times steam–N2); `sembawang`'s
//! original carries H2's own properties, which a follow-up could port.~~
//! **CORRECTED 2026-11-02 (gh:#1114):** the pool surface takes the full
//! composition as a [`crate::components::wall_condensation::GasMixtureAtmosphere`]:
//! each non-condensable with its own viscosity, conductivity, molar mass,
//! `c_p` and steam diffusivity (steam, N2 and H2 MELCOR's; O2, CO, CO2 and
//! Ar by Chapman–Enskog and Eucken), mixed by MELCOR's rules.
//! [`ControlVolume::without_ncg_transport_properties`] restores the N2
//! stand-in, bit for bit.
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
//! | a layered pool's stable layers conduct | [`ControlVolume::without_pool_layer_conduction`] |
//! | a layered pool's surface sees its surface mixed block | [`ControlVolume::without_surface_mixed_block`] (the top layer alone) |
//! | the flash ends at `T_sat` of the pressure after it (gh:#1107) | [`ControlVolume::without_implicit_flash`] (at `T_sat` of the pressure before it: a one-step burst) |
//! | each non-condensable on its own transport properties at the pool surface (gh:#1114) | [`ControlVolume::without_ncg_transport_properties`] (every non-condensable as N2) |
//!
//! ## The layered pool and dry-out (gh:#1100)
//!
//! **Layers, a configuration choice.** [`ControlVolume::with_pool_layers`]
//! splits the pool into `n` horizontal layers (the default `n = 1` runs the
//! single-region code path unchanged, bit for bit:
//! `tests/control_volume_layers.rs` L1 checks 64 values against the code as
//! it was before). The totals still set the level, the density, the swell
//! and the atmosphere's volume. What enters at a port joins the layer at
//! the port's elevation; bubbles meet that layer's temperature; a liquid
//! outflow draws from its layer, then the layers above, then below; the
//! donor state at an elevation is that layer's liquid. ~~The surface exchange,
//! flashing and rain-out act on the top layer~~ **CORRECTED 2026-11-01**
//! (gh:#1100): the surface exchange and flashing act on the **surface mixed
//! block**, the layers from the top down that are not stably stratified
//! against the surface water (its mass for MELCOR's Eq 5-17 limit, its depth
//! as the pool-side length, what it gives up shared over its layers by
//! mass), so a mixed column is the one-layer pool whatever `n`
//! (`tests/control_volume_layers.rs` L9; the top layer alone made the
//! exchange depend on `n`, kept as
//! [`ControlVolume::without_surface_mixed_block`]); rain-out joins the top
//! layer; pool heat, the immersed heat
//! exchanger and the compression work are shared by mass, and
//! [`ControlVolumeInputs::pool_heat_at`] puts heat at elevations. The column
//! operations are in [`layers`]: DYRESM-style split and merge of Lagrangian
//! layers, `sembawang`'s mixing of an unstable column
//! (`bwr_mark1::containment::Containment::layer_pool`, generalised to `n`
//! layers by IF97 density), and molecular conduction across a stable one
//! (no eddy diffusivity: stated, the lower bound of the exchange).
//!
//! **Dry-out, always on.** A pool may hold zero mass, from construction or
//! by draining, flashing, evaporating or boiling. A removal that would leave
//! less than 1e-9 of the pool (or a layer) takes it all, so an empty pool is
//! exactly `(0, 0)`; a drained pool's last flow work comes from the
//! atmosphere, and an end-of-step sweep returns to the atmosphere what the
//! compression work leaves on an empty pool (and any pool under 1e-9 kg).
//! A dry pool has no surface exchange and no flashing, its ports draw the
//! atmosphere, the heat exchanger is uncovered, and pool heat goes to the
//! atmosphere (reported as [`ControlVolumeStepReport::pool_heat_to_atmosphere`],
//! as is heat beyond what boils the whole pool away). Rain-out, inflow and
//! condensate refill it. Evaporation that would leave the surface liquid
//! below the triple point evaporates it whole (MELCOR's Eq 5-17 limit over-
//! draws a pool that is nearly all evaporating).
//!
//! ## Atmosphere species sources and sinks (gh:#1100, gh:#1103)
//!
//! ~~**Atmosphere reactions:** `ControlVolumeInputs::atmosphere_reactions`
//! burns H2 and CO at the caller's rates, the heat of reaction closed here
//! on the gas mixture's own references.~~ **CORRECTED 2026-11-01**
//! (gh:#1103, maintainer direction: `tampines` holds thermal hydraulics,
//! no chemistry): the reactions moved to `sembawang`
//! (`sembawang::bwr_mark1::reaction`). What stays here is generic:
//! [`ControlVolumeInputs::atmosphere_species`], a mass rate per species
//! (steam and each non-condensable; negative removes), each carried at its
//! own code molar enthalpy at the **start-of-step** atmosphere
//! ([`ControlVolume::atmosphere_species_molar_enthalpies`], explicit like
//! the other caller inputs, so a caller can close its own energy balance on
//! exactly the values used). Applied after the atmosphere heat and steam
//! sink; a removal larger than what the atmosphere holds then is refused.
//! The step reports what moved; [`ControlVolumeBookkeeping`] counts it as
//! a boundary flow.
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
//! the pool at once); ~~pool stratification~~ (**CORRECTED 2026-11-01,
//! gh:#1100:** optional layers, above; mixing by stirring is still not
//! modelled); boiling at depth in a layered pool (only the surface mixed block, **CORRECTED 2026-11-01** from "the top layer",
//! flashes); heat structures (the caller's, as `pool_heat`); aerosols.

pub mod layers;
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
    AvailableEnergy, Energy, HeatTransfer, Length, Mass, MassDensity, MassRate, MolarEnergy, Power, Pressure,
    ThermodynamicTemperature, Time, Volume,
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
use pool_surface::{bubble_efficiency, exchange, exchange_mixture, k_water, p_sat_pa, rho_liquid, steam_h, t_sat, Atmosphere, PoolState, SurfaceGeometry, G, M_H2O, R_GAS};
use crate::components::wall_condensation::GasMixtureAtmosphere;

/// `sembawang`'s constant-`c_p` liquid, J/(kg K) (`containment::CP_WATER`).
const CP_WATER: f64 = 4186.0;
/// `sembawang`'s ideal steam (`containment::{U_STEAM_REF, CV_STEAM, T_REF}`).
const U_STEAM_REF: f64 = 2.5060e6;
const CV_STEAM: f64 = 1500.0;
const T_REF: f64 = 373.15;

/// Dry-out (gh:#1100): a removal that would leave less than this fraction
/// of what a pool (or a layer) held takes all of it, so a drained pool is
/// exactly empty rather than a round-off residue whose temperature
/// `u = E / m` means nothing. Numerical, not physical: it acts only within
/// a part in 1e9 of empty.
const DRY_FRACTION: f64 = 1e-9;
/// Dry-out: a pool below this mass at the end of a step is swept into the
/// atmosphere as vapour with its energy, kg (numerical, as
/// [`DRY_FRACTION`]).
const DRY_KG: f64 = 1e-9;

/// Whether removing `m` from `before` leaves it dry.
fn leaves_dry(before: f64, m: f64) -> bool {
    before - m <= (DRY_FRACTION * before).max(DRY_KG)
}

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
    /// Mass leaving, ≥ 0; pool liquid or atmosphere by the level, unless
    /// [`Self::liquid_fraction`] splits it.
    pub outflow: MassRate,
    /// The pool-liquid mass fraction of the outflow, in [0, 1], the rest
    /// atmosphere (gh:#1117: a junction straddling the surface moves both
    /// phases, as its path splits them; MELCOR's FL junction opening
    /// fractions). `None`: one phase by the level at the port's elevation
    /// (the point draw).
    pub liquid_fraction: Option<f64>,
}

impl PortInput {
    /// Nothing through this port.
    pub fn closed() -> Self {
        Self { inflow: Source::none(), outflow: MassRate::new::<kilogram_per_second>(0.0), liquid_fraction: None }
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

/// Heat into the pool water between two elevations (gh:#1100): a heat
/// structure, a sparger's condensation, the decay heat of what is
/// scrubbed where it is scrubbed. In a layered pool it is shared over the
/// layers by the mass of each that lies between `bottom` and `top` (by the
/// cumulative mass below each elevation at the pool's mean density, as a
/// port finds its layer); a band of zero thickness, or one wholly above
/// the level, goes into the layer at `bottom` (the top layer above the
/// level). In a one-layer pool it is [`ControlVolumeInputs::pool_heat`]
/// exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevatedPoolHeat {
    /// The band's lower elevation.
    pub bottom: Length,
    /// The band's upper elevation (at least `bottom`).
    pub top: Length,
    /// The heat, W; negative removes.
    pub power: Power,
}

impl ElevatedPoolHeat {
    /// Heat into the layer at elevation `z`.
    pub fn at(z: Length, power: Power) -> Self {
        Self { bottom: z, top: z, power }
    }
    /// Heat shared by mass over the water between `bottom` and `top`.
    pub fn between(bottom: Length, top: Length, power: Power) -> Self {
        Self { bottom, top, power }
    }
}

/// Gas sources (positive) and sinks (negative) on a volume's atmosphere,
/// per species, kg/s (gh:#1103): a caller's chemistry, a structure's
/// outgassing, an absorber. Each species enters or leaves at its own code
/// molar enthalpy at the start-of-step atmosphere
/// ([`ControlVolume::atmosphere_species_molar_enthalpies`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereSpeciesRates {
    /// Steam, kg/s.
    pub steam: MassRate,
    /// Each non-condensable, kg/s, in [`NonCondensable::ALL`] order.
    pub ncg: [MassRate; 6],
}

impl AtmosphereSpeciesRates {
    /// No source or sink.
    pub fn none() -> Self {
        Self { steam: MassRate::new::<kilogram_per_second>(0.0), ncg: [MassRate::new::<kilogram_per_second>(0.0); 6] }
    }
    /// Whether every rate is zero.
    pub fn is_none(&self) -> bool {
        self.steam.get::<kilogram_per_second>() == 0.0 && self.ncg.iter().all(|m| m.get::<kilogram_per_second>() == 0.0)
    }
}

impl Default for AtmosphereSpeciesRates {
    fn default() -> Self {
        Self::none()
    }
}

/// What drives one step.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolumeInputs {
    /// One input per port, in the geometry's port order.
    pub ports: Vec<PortInput>,
    /// Heat into the pool (a heat structure, a core), W; negative removes.
    /// Shared by mass in a layered pool (heat at an elevation:
    /// [`Self::pool_heat_at`]); into the atmosphere when the pool is dry
    /// (gh:#1100).
    pub pool_heat: Power,
    /// Heat into the pool at elevations ([`ElevatedPoolHeat`]; gh:#1100),
    /// beside `pool_heat`. Like it explicit, into the atmosphere when the
    /// pool is dry, and counted with it for the pool boiling dry within the
    /// step. Empty by default (the step is then the code without it, bit
    /// for bit).
    pub pool_heat_at: Vec<ElevatedPoolHeat>,
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
    /// Gas into (+) or out of (−) the atmosphere per species
    /// ([`AtmosphereSpeciesRates`]; gh:#1103), each at its own code molar
    /// enthalpy at the start-of-step atmosphere. Applied after the
    /// atmosphere heat and steam sink. None by default (the step is then
    /// the code without it, bit for bit).
    pub atmosphere_species: AtmosphereSpeciesRates,
}

impl ControlVolumeInputs {
    /// Every port closed, no heat, for `n_ports` ports.
    pub fn closed(n_ports: usize) -> Self {
        Self {
            ports: vec![PortInput::closed(); n_ports],
            pool_heat: Power::new::<watt>(0.0),
            pool_heat_at: Vec::new(),
            heat_exchanger: ImmersedHeatExchanger::off(),
            atmosphere_heat: Power::new::<watt>(0.0),
            atmosphere_steam_sink: MassRate::new::<kilogram_per_second>(0.0),
            atmosphere_species: AtmosphereSpeciesRates::none(),
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
    /// Pool heat that went into the atmosphere because the pool was dry, or
    /// the pool boiled dry on it within the step (gh:#1100).
    pub pool_heat_to_atmosphere: Energy,
    /// What [`ControlVolumeInputs::atmosphere_species`] moved this step,
    /// signed (positive in): steam, each non-condensable, and the energy
    /// carried (gh:#1103).
    pub species_water: Mass,
    pub species_ncg: [Mass; 6],
    pub species_energy: Energy,
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
    /// (~~H2 and CO burned and the heat of reaction~~ **CORRECTED
    /// 2026-11-01**, gh:#1103: no reactions here; the species sources and
    /// sinks count among the boundary flows above.)
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
    /// A layered pool's stable layers exchange heat by molecular conduction
    /// (gh:#1100; no effect on a one-layer pool).
    pub pool_layer_conduction: bool,
    /// A layered pool's surface (exchange, flashing) acts on the surface
    /// mixed block, the layers from the top down that are not stably
    /// stratified against the surface water, rather than on the top layer
    /// alone (gh:#1100; no effect on a one-layer pool). See
    /// [`ControlVolume::without_surface_mixed_block`].
    pub surface_mixed_block: bool,
    /// A pool above saturation flashes to the state consistent with the
    /// pressure its own vapour raises (gh:#1107; MELCOR CVH/FL RM §4.2.1,
    /// §4.3, Eq 5-30), rather than explicitly at the pressure before it.
    /// See [`ControlVolume::without_implicit_flash`].
    pub implicit_flash: bool,
    /// Each non-condensable on its own transport properties at the pool
    /// surface (gh:#1114), rather than all of them as N2. See
    /// [`ControlVolume::without_ncg_transport_properties`].
    pub ncg_transport_properties: bool,
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
            pool_layer_conduction: true,
            surface_mixed_block: true,
            implicit_flash: true,
            ncg_transport_properties: true,
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
    /// The pool's layers, bottom first, `(kg, J)`, summing to `pool_kg` and
    /// `pool_e`; empty for one well-mixed pool (gh:#1100).
    layers: Vec<(f64, f64)>,
    /// The layers' temperatures, K (derived; empty with `layers`).
    layer_t: Vec<f64>,
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
    /// [`TampinesError::InvalidInput`] for a negative pool mass (zero, a dry
    /// pool, is allowed), a non-positive temperature, an empty atmosphere, a port outside the table, a
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
        // A dry pool (zero mass) is allowed (gh:#1100); its temperature is
        // then the one a first drop of liquid is guessed at.
        if !(pool_kg.is_finite() && pool_kg >= 0.0) {
            return Err(TampinesError::InvalidInput(format!("pool mass = {pool_kg}")));
        }
        for (x, what) in [(t_p, "pool temperature"), (t_g, "atmosphere temperature")] {
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
            layers: Vec::new(),
            layer_t: Vec::new(),
        };
        cv.reset_energies(t_p, t_g)?;
        Ok(cv)
    }

    /// Set the energies from the temperatures under the current models.
    fn reset_energies(&mut self, t_p: f64, t_g: f64) -> Result<(), TampinesError> {
        if self.layers.is_empty() {
            self.pool_e = self.pool_kg * self.u_liquid(t_p);
        } else {
            // Each layer keeps its own temperature under the new model.
            let es: Vec<f64> = self.layers.iter().zip(&self.layer_t).map(|(l, t)| l.0 * self.u_liquid(*t)).collect();
            let mut e = 0.0;
            for (l, x) in self.layers.iter_mut().zip(es) {
                l.1 = x;
                e += x;
            }
            self.pool_e = e;
        }
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

    // ── The layered pool and dry-out (gh:#1100) ──────────────────────────

    /// Split the pool into `n` horizontal layers of equal mass, each at the
    /// pool's present specific energy (see "The layered pool" in the module
    /// documentation). `n = 1` is the one well-mixed pool, the default,
    /// which runs the single-region code path unchanged (bit for bit; it
    /// merges an already layered pool back into one).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for `n = 0`.
    pub fn with_pool_layers(mut self, n: usize) -> Result<Self, TampinesError> {
        if n == 0 {
            return Err(TampinesError::InvalidInput("a pool needs at least one layer".into()));
        }
        if n == 1 {
            self.layers.clear();
            self.layer_t.clear();
            return Ok(self);
        }
        let u = if self.pool_kg > 0.0 { self.pool_e / self.pool_kg } else { 0.0 };
        let target = self.pool_kg / n as f64;
        let mut layers: Vec<(f64, f64)> = (0..n - 1).map(|_| (target, target * u)).collect();
        let e_rest: f64 = layers.iter().map(|l| l.1).sum();
        layers.push((self.pool_kg - target * (n - 1) as f64, self.pool_e - e_rest));
        self.layers = layers;
        self.layer_t = vec![self.d.t_pool; n];
        self.refresh_layers();
        Ok(self)
    }

    /// Ablation: a layered pool's stable layers exchange nothing (as
    /// `sembawang`'s two-layer pool: unstable layers still mix).
    pub fn without_pool_layer_conduction(mut self) -> Self {
        self.mech.pool_layer_conduction = false;
        self
    }

    /// Ablation: a layered pool's surface exchange and flashing see the top
    /// layer alone (its mass for MELCOR's Eq 5-17 limit, its thickness as
    /// the pool-side natural-convection length), as before gh:#1100's fix.
    /// That makes the exchange depend on the layer count through the top
    /// layer's thickness (`tests/control_volume_layers.rs` L9: +5 to +7 Pa
    /// a doubling on a mixed pool); the default sees the surface mixed
    /// block, which MELCOR's one pool per volume (CVH/FL RM §5.1.2, Eqs
    /// 5-20, 5-22 to 5-25: `X = min(D_s, L)`, `L` the pool's depth) reduces
    /// to when the column is mixed.
    pub fn without_surface_mixed_block(mut self) -> Self {
        self.mech.surface_mixed_block = false;
        self
    }

    /// Ablation: the flash explicit, as before gh:#1107: a pool (or a
    /// layered pool's surface mixed block) above `T_sat(p)` flashes in full
    /// to `T_sat` at the pressure **before** its vapour joins the
    /// atmosphere, so it ends the step subcooled against its own pressure
    /// and the next flash waits for the pressure to fall back: a one-step
    /// burst whose overshoot is the loop gain `G = (M c / h_fg) (dT_sat/dp)
    /// (∂p/∂m)` (about 85 for `sembawang`'s torus after the vent;
    /// `tests/control_volume_flash.rs` F1). The default solves the flash
    /// with its own pressure response, the single-volume form of MELCOR's
    /// end-of-step consistency (CVH/FL RM §4.2.1, §4.3, Eq 5-30).
    pub fn without_implicit_flash(mut self) -> Self {
        self.mech.implicit_flash = false;
        self
    }

    /// Ablation (gh:#1114): the pool surface treats every non-condensable as
    /// N2 (their total moles as N2: N2's viscosity, conductivity, molar mass,
    /// `c_p` and steam diffusivity), as before 2026-11-02, bit for bit. The
    /// mole fractions in the driving force are unchanged either way.
    pub fn without_ncg_transport_properties(mut self) -> Self {
        self.mech.ncg_transport_properties = false;
        self
    }

    /// Set each layer's temperature, bottom first (an initial
    /// stratification), keeping the layers' masses; one value for a
    /// one-layer pool. No mixing is applied until the next step.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for the wrong number of values or a
    /// non-positive temperature; [`TampinesError::Unphysical`] if the
    /// atmosphere's closure fails.
    pub fn set_pool_layer_temperatures(&mut self, t: &[ThermodynamicTemperature]) -> Result<(), TampinesError> {
        let n = self.layers.len().max(1);
        if t.len() != n {
            return Err(TampinesError::InvalidInput(format!("{} layer temperatures for {n} layers", t.len())));
        }
        let ts: Vec<f64> = t.iter().map(|x| x.get::<kelvin>()).collect();
        if ts.iter().any(|x| !(x.is_finite() && *x > 0.0)) {
            return Err(TampinesError::InvalidInput(format!("layer temperatures {ts:?}")));
        }
        if self.layers.is_empty() {
            self.pool_e = self.pool_kg * self.u_liquid(ts[0]);
            self.d.t_pool = ts[0];
        } else {
            let es: Vec<f64> = self.layers.iter().zip(&ts).map(|(l, t)| l.0 * self.u_liquid(*t)).collect();
            let mut e = 0.0;
            for (l, x) in self.layers.iter_mut().zip(es) {
                l.1 = x;
                e += x;
            }
            self.pool_e = e;
            self.layer_t = ts;
        }
        self.refresh()
    }

    /// The number of pool layers (1 for the well-mixed pool).
    pub fn pool_layer_count(&self) -> usize {
        self.layers.len().max(1)
    }
    /// Each layer's temperature, bottom first (the pool's for one layer; an
    /// empty layer reports the pool's).
    pub fn pool_layer_temperatures(&self) -> Vec<ThermodynamicTemperature> {
        if self.layers.is_empty() { vec![kelv(self.d.t_pool)] } else { self.layer_t.iter().map(|t| kelv(*t)).collect() }
    }
    /// Each layer's mass, bottom first (the pool's for one layer).
    pub fn pool_layer_masses(&self) -> Vec<Mass> {
        if self.layers.is_empty() { vec![self.pool_mass()] } else { self.layers.iter().map(|l| Mass::new::<kilogram>(l.0)).collect() }
    }
    /// Each layer's internal energy, bottom first (the pool's for one layer).
    pub fn pool_layer_energies(&self) -> Vec<Energy> {
        if self.layers.is_empty() { vec![Energy::new::<joule>(self.pool_e)] } else { self.layers.iter().map(|l| Energy::new::<joule>(l.1)).collect() }
    }
    /// The temperature of the pool's surface (top holding) layer (the
    /// pool's for one layer). ~~The one the surface exchange sees~~
    /// **CORRECTED 2026-11-01:** the exchange sees the surface mixed block,
    /// whose temperature equals this layer's to round-off when it is mixed
    /// and is this layer's when the block is one layer.
    pub fn pool_surface_temperature(&self) -> ThermodynamicTemperature {
        kelv(self.t_layer(self.top_layer()))
    }
    /// Whether the pool is dry (holds no liquid).
    pub fn is_pool_dry(&self) -> bool {
        !(self.pool_kg > 0.0)
    }

    /// The layer holding elevation `z` (0 for one layer): by the mass below
    /// `z` at the pool's mean density, the top layer at and above the level.
    fn layer_at(&self, z: f64) -> usize {
        if self.layers.is_empty() {
            return 0;
        }
        let m_z = self.d.rho * self.table.volume_below_m3(z);
        let mut c = 0.0;
        for (i, l) in self.layers.iter().enumerate() {
            c += l.0;
            if m_z < c {
                return i;
            }
        }
        self.top_layer()
    }
    /// The highest layer holding liquid (the top one if all are empty).
    fn top_layer(&self) -> usize {
        self.layers.iter().rposition(|l| l.0 > 0.0).unwrap_or(self.layers.len().saturating_sub(1))
    }
    /// Layer `i`'s temperature (the pool's for one layer).
    fn t_layer(&self, i: usize) -> f64 {
        if self.layers.is_empty() { self.d.t_pool } else { self.layer_t[i] }
    }
    /// The liquid's temperature at elevation `z`.
    fn local_t(&self, z: f64) -> f64 {
        self.t_layer(self.layer_at(z))
    }
    /// `dm` kg and `de` J into layer `layer` (the pool for one layer).
    fn add_pool_at(&mut self, layer: usize, dm: f64, de: f64) -> Result<(), TampinesError> {
        if let Some(l) = self.layers.get_mut(layer) {
            l.0 += dm;
            l.1 += de;
        }
        self.add_pool(dm, de)
    }
    /// `de` J spread over the layers in proportion to their mass (the top
    /// holding layer takes the remainder), the totals unchanged: the caller
    /// adds `de` to `pool_e`.
    fn spread_by_mass(&mut self, de: f64) {
        if self.layers.is_empty() || !(self.pool_kg > 0.0) {
            return;
        }
        let top = self.top_layer();
        let mut used = 0.0;
        for (i, l) in self.layers.iter_mut().enumerate() {
            if i != top && l.0 > 0.0 {
                let x = de * l.0 / self.pool_kg;
                l.1 += x;
                used += x;
            }
        }
        self.layers[top].1 += de - used;
    }
    /// Empty the pool: its mass and energy to zero exactly, every layer too.
    fn empty_pool(&mut self) {
        self.pool_kg = 0.0;
        self.pool_e = 0.0;
        for l in self.layers.iter_mut() {
            *l = (0.0, 0.0);
        }
    }
    /// The layers' heat capacities, J/K, and the conductances between
    /// neighbours, W/K: `k A / Δz`, `k` MELCOR's liquid conductivity at the
    /// two layers' mean temperature, `A` the free cross-section at their
    /// interface, `Δz` the distance between their mid-mass elevations, all
    /// at the pool's mean density.
    fn layer_conductances(&self) -> (Vec<f64>, Vec<f64>) {
        let n = self.layers.len();
        let rho = self.d.rho;
        let cap: Vec<f64> = self.layers.iter().zip(&self.layer_t).map(|(l, t)| if l.0 > 0.0 { l.0 * self.cv_liquid(*t) } else { 0.0 }).collect();
        let mut mid = Vec::with_capacity(n);
        let mut top = Vec::with_capacity(n);
        let mut c = 0.0;
        for l in &self.layers {
            mid.push(self.table.height_of_volume_m((c + 0.5 * l.0) / rho));
            c += l.0;
            top.push(self.table.height_of_volume_m(c / rho));
        }
        let g = (0..n.saturating_sub(1))
            .map(|k| {
                let dz = mid[k + 1] - mid[k];
                if !(dz > 0.0) {
                    return 0.0;
                }
                let k_w = k_water(0.5 * (self.layer_t[k] + self.layer_t[k + 1]));
                k_w * self.table.area_at(Length::new::<meter>(top[k])).get::<uom::si::area::square_meter>() / dz
            })
            .collect();
        (cap, g)
    }
    /// The top holding layer's thickness, m.
    fn top_layer_thickness(&self) -> f64 {
        let top = self.top_layer();
        let below: f64 = self.layers.iter().take(top).map(|l| l.0).sum();
        self.d.level - self.table.height_of_volume_m(below / self.d.rho)
    }
    /// The layered pool's column step: split and merge so no layer is thick
    /// or empty, conduction across the layers (implicit), and the convective
    /// adjustment.
    fn column_step(&mut self, dt: f64) -> Result<(), TampinesError> {
        if self.layers.is_empty() || !(self.pool_kg > 0.0) {
            return Ok(());
        }
        if layers::split_merge(&mut self.layers) > 0 {
            self.refresh_layers();
        }
        if self.mech.pool_layer_conduction {
            let (cap, g) = self.layer_conductances();
            let t = self.layer_t.clone();
            layers::conduct(&mut self.layers, &t, &cap, &g, dt);
            self.refresh_layers();
        }
        self.mix_unstable();
        Ok(())
    }
    /// The convective adjustment, by IF97 density at the atmosphere's
    /// pressure (the constant-`c_p` ablation's energy mapped to a
    /// temperature by that model).
    fn mix_unstable(&mut self) {
        if self.layers.is_empty() {
            return;
        }
        let p = self.d.p_gas;
        let guess = self.d.t_pool;
        let mut layers = std::mem::take(&mut self.layers);
        let merges = layers::convective_adjustment(&mut layers, |u| rho_liquid(self.t_liquid(u, guess), p));
        self.layers = layers;
        if merges > 0 {
            self.refresh_layers();
        }
    }
    /// The end-of-step dry-out sweep: a pool below [`DRY_KG`] (and an empty
    /// pool left holding energy by the step's compression work) goes into
    /// the atmosphere, mass as vapour with its energy.
    fn sweep_dry(&mut self) -> Result<(), TampinesError> {
        if self.pool_kg <= DRY_KG && (self.pool_kg != 0.0 || self.pool_e != 0.0) {
            self.gas.steam_kg += self.pool_kg;
            self.gas_e += self.pool_e;
            self.empty_pool();
            self.refresh()?;
        }
        Ok(())
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
        // A dry pool keeps its last temperature (gh:#1100).
        let t_pool = if self.pool_kg > 0.0 { self.t_liquid(self.pool_e / self.pool_kg, self.d.t_pool) } else { self.d.t_pool };
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
        self.refresh_layers();
        Ok(())
    }

    /// Re-derive the layers' temperatures (an empty layer takes the pool's).
    fn refresh_layers(&mut self) {
        if self.layers.is_empty() {
            return;
        }
        let mut ts = std::mem::take(&mut self.layer_t);
        ts.resize(self.layers.len(), self.d.t_pool);
        for (i, l) in self.layers.iter().enumerate() {
            let guess = if ts[i].is_finite() && ts[i] > 0.0 { ts[i] } else { self.d.t_pool };
            ts[i] = if l.0 > 0.0 { self.t_liquid(l.1 / l.0, guess) } else { self.d.t_pool };
        }
        self.layer_t = ts;
    }

    fn add_pool(&mut self, dm: f64, de: f64) -> Result<(), TampinesError> {
        self.pool_kg += dm;
        self.pool_e += de;
        if !(self.pool_kg >= 0.0) {
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
        // The liquid of the layer at `z` (the pool's for one layer).
        let t_l = self.local_t(z.get::<meter>());
        let liquid = DonorState {
            draw: PortDraw::Liquid,
            pressure: Pressure::new::<pascal>(p),
            density: self.pool_density(),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(self.h_liquid(t_l, p)),
            temperature: kelv(t_l),
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
    /// order) through the bubble model, the pool heat, the atmosphere heat,
    /// steam sink and reactions (gh:#1097, gh:#1100), the heat exchanger,
    /// every port's outflow (port order) by the level, the pool surface
    /// (flashing, exchange, rain-out), and the pool's compression work.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`, a wrong
    /// number of port inputs, a negative or non-finite flow or burn rate, or an
    /// atmosphere outflow not below the atmosphere's mass (a liquid outflow
    /// larger than the pool takes the pool and reports what moved: dry-out,
    /// gh:#1100); [`TampinesError::Unphysical`] if the pool fills the vessel
    /// or a closure fails.
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
        let mut heat_at: Vec<(f64, f64, f64)> = Vec::with_capacity(inputs.pool_heat_at.len());
        for h in &inputs.pool_heat_at {
            let (z0, z1, w) = (h.bottom.get::<meter>(), h.top.get::<meter>(), h.power.get::<watt>());
            if !(z0.is_finite() && z1.is_finite() && z1 >= z0 && w.is_finite()) {
                return Err(TampinesError::InvalidInput(format!("pool heat at an elevation {h:?}")));
            }
            heat_at.push((z0, z1, w * dt));
        }
        let start = self.d;
        let species_h = if inputs.atmosphere_species.is_none() { None } else { Some(self.species_molar_h()) };
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
        let mut q_to_atm = 0.0;
        if q_heat != 0.0 {
            let q = q_heat * dt;
            if q > 0.0 {
                self.e_in += q;
            } else {
                self.e_out -= q;
            }
            q_to_atm = self.heat_pool(q)?;
        }
        if !heat_at.is_empty() {
            for &(_, _, q) in &heat_at {
                if q > 0.0 {
                    self.e_in += q;
                } else {
                    self.e_out -= q;
                }
            }
            q_to_atm += self.heat_pool_at(&heat_at)?;
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
        // 2c. The caller's species sources and sinks (gh:#1103), at the
        //    start-of-step enthalpies.
        let (sp_w, sp_n, sp_e) = match species_h {
            Some(h) => self.apply_species(dt, &inputs.atmosphere_species, &h)?,
            None => (0.0, [0.0; 6], 0.0),
        };
        let k = ua * dt;
        // A dry pool uncovers the exchanger: no heat (gh:#1100).
        let q_hx = if !(self.pool_kg > 0.0) {
            0.0
        } else if self.layers.is_empty() {
            k * (self.d.t_pool - t_hx) / (1.0 + k / (self.pool_kg * self.cv_liquid(self.d.t_pool)))
        } else {
            // Each layer sees its share of the exchanger, by mass (stated:
            // the tubes span the pool), implicit in its own temperature.
            let mut q = 0.0;
            for i in 0..self.layers.len() {
                let (m_l, t_l) = (self.layers[i].0, self.layer_t[i]);
                if m_l > 0.0 {
                    let k_l = k * m_l / self.pool_kg;
                    let q_l = k_l * (t_l - t_hx) / (1.0 + k_l / (m_l * self.cv_liquid(t_l)));
                    self.layers[i].1 -= q_l;
                    q += q_l;
                }
            }
            self.pool_e -= q;
            self.refresh()?;
            q
        };
        if q_hx != 0.0 && self.layers.is_empty() {
            self.add_pool(0.0, -q_hx)?;
        }
        if q_hx != 0.0 {
            self.e_out += q_hx;
        }

        // 3. Outflows, by the level, or split by the port's liquid fraction
        //    (gh:#1117).
        for i in 0..self.ports.len() {
            let m = finite_nonneg(inputs.ports[i].outflow.get::<kilogram_per_second>(), "outflow")? * dt;
            let z = self.ports[i].elevation.get::<meter>();
            let draw = if z < self.draw_level() { PortDraw::Liquid } else { PortDraw::Atmosphere };
            transfers[i].draw = draw;
            if m <= 0.0 {
                continue;
            }
            // The liquid and atmosphere masses: the point draw puts all of
            // `m` on one phase; a split puts `f m` on the liquid and the rest
            // on the atmosphere (exactly `m` and 0 when `f` is 1 or 0).
            let (m_liq, m_atm) = match inputs.ports[i].liquid_fraction {
                None => match draw {
                    PortDraw::Liquid => (m, 0.0),
                    PortDraw::Atmosphere => (0.0, m),
                },
                Some(f) => {
                    if !(0.0..=1.0).contains(&f) {
                        return Err(TampinesError::InvalidInput(format!("port {i} liquid fraction {f}")));
                    }
                    if f == 1.0 {
                        (m, 0.0)
                    } else if f == 0.0 {
                        (0.0, m)
                    } else {
                        (f * m, m - f * m)
                    }
                }
            };
            if m_liq > 0.0 {
                let (m, e) = self.draw_liquid(z, m_liq, pressures[i])?;
                self.water_out += m;
                self.e_out += e;
                transfers[i].water_out += Mass::new::<kilogram>(m);
                transfers[i].energy_out += Energy::new::<joule>(e);
            }
            if m_atm > 0.0 {
                let m = m_atm;
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
                transfers[i].water_out += Mass::new::<kilogram>(ds);
                transfers[i].ncg_out = out.map(|x| Mass::new::<kilogram>(x));
                transfers[i].energy_out += Energy::new::<joule>(e);
            }
        }

        // 3b. A layered pool's column: split and merge, conduction, mixing.
        self.column_step(dt)?;

        // 4. The pool surface.
        let (flash, evap, rain) = self.surface(dt)?;
        vapour_volume += flash * R_GAS * self.d.t_pool / (M_H2O * self.d.p_gas);

        // 5. The pool's compression work on the atmosphere, `p ΔV_pool` at
        //    the mean of the start and end pressures (internal: the vessel is
        //    rigid).
        if self.mech.pressure_work {
            let w = 0.5 * (start.p_gas + self.d.p_gas) * (self.pool_kg / self.d.rho - v_pool_start);
            if w != 0.0 {
                self.spread_by_mass(-w);
                self.pool_e -= w;
                self.gas_e += w;
                self.refresh()?;
            }
        }
        // 6. The surface may have left the top layer denser (evaporative
        //    cooling): mix again, so the step ends stable. Then the dry-out
        //    sweep (gh:#1100).
        self.mix_unstable();
        self.sweep_dry()?;
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
            pool_heat_to_atmosphere: Energy::new::<joule>(q_to_atm),
            species_water: Mass::new::<kilogram>(sp_w),
            species_ncg: sp_n.map(|x| Mass::new::<kilogram>(x)),
            species_energy: Energy::new::<joule>(sp_e),
        })
    }

    /// Each species' code molar enthalpy, J/mol, steam first, then the
    /// non-condensables in [`NonCondensable::ALL`] order, at temperature
    /// `t` and steam partial pressure `p_steam` (steam: `M_H2O h_steam(t,
    /// p_steam)` on the closure's steam model; a non-condensable: its
    /// `u(t) + R t`). The references a caller's chemistry anchors to
    /// (gh:#1103).
    pub fn species_molar_enthalpies(&self, t: ThermodynamicTemperature, p_steam: Pressure) -> [MolarEnergy; 7] {
        Self::molar_h_raw(self, t.get::<kelvin>(), p_steam.get::<pascal>()).map(MolarEnergy::new::<uom::si::molar_energy::joule_per_mole>)
    }
    /// [`Self::species_molar_enthalpies`] at the atmosphere's present
    /// temperature and steam partial pressure: what
    /// [`ControlVolumeInputs::atmosphere_species`] carries in the next step.
    pub fn atmosphere_species_molar_enthalpies(&self) -> [MolarEnergy; 7] {
        self.species_molar_h().map(MolarEnergy::new::<uom::si::molar_energy::joule_per_mole>)
    }
    fn species_molar_h(&self) -> [f64; 7] {
        let n_s = self.gas.steam_kg / M_H2O;
        let n = n_s + self.gas.ncg_total_moles();
        let p_s = if n > 0.0 { self.d.p_gas * n_s / n } else { 0.0 };
        self.molar_h_raw(self.d.t_gas, p_s)
    }
    fn molar_h_raw(&self, t: f64, p_s: f64) -> [f64; 7] {
        let mut h = [0.0; 7];
        h[0] = M_H2O * self.h_steam(t, p_s);
        for g in NonCondensable::ALL {
            h[1 + g.index()] = self.model.ncg_u(g, t) + R_GAS * t;
        }
        h
    }

    /// Apply the caller's species sources and sinks for `dt` at the molar
    /// enthalpies `h` (J/mol, taken at the start of the step): each
    /// species' mass `rate · Δt` added or removed, the energy `Σ (m / M) h`
    /// with it. A removal of more than the atmosphere holds (beyond 1e-12
    /// of it, round-off) is refused; within that it takes it all, exactly.
    /// Nothing at all (not even a refresh) when every rate is zero. Returns
    /// the steam, each gas and the energy moved, signed.
    fn apply_species(&mut self, dt: f64, rates: &AtmosphereSpeciesRates, h: &[f64; 7]) -> Result<(f64, [f64; 6], f64), TampinesError> {
        let fin = |x: f64, what: &str| if x.is_finite() { Ok(x) } else { Err(TampinesError::InvalidInput(format!("{what} {x}"))) };
        let take = |have: f64, dm: f64, what: &str| -> Result<f64, TampinesError> {
            if dm >= 0.0 {
                return Ok(have + dm);
            }
            if -dm > have * (1.0 + 1e-12) {
                return Err(TampinesError::InvalidInput(format!("{what} sink {} not within the atmosphere's {have}", -dm)));
            }
            Ok(if -dm >= have { 0.0 } else { have + dm })
        };
        let m_s = fin(rates.steam.get::<kilogram_per_second>(), "steam source")? * dt;
        let mut m_n = [0.0; 6];
        for g in NonCondensable::ALL {
            m_n[g.index()] = fin(rates.ncg[g.index()].get::<kilogram_per_second>(), "gas source")? * dt;
        }
        let mut e = 0.0;
        if m_s != 0.0 {
            self.gas.steam_kg = take(self.gas.steam_kg, m_s, "steam")?;
            e += m_s / M_H2O * h[0];
            if m_s > 0.0 {
                self.water_in += m_s;
            } else {
                self.water_out -= m_s;
            }
        }
        for g in NonCondensable::ALL {
            let j = g.index();
            let m = m_n[j];
            if m == 0.0 {
                continue;
            }
            let dn = m / g.molar_mass();
            self.gas.ncg_moles[j] = take(self.gas.ncg_moles[j], dn, "gas")?;
            e += dn * h[1 + j];
            if m > 0.0 {
                self.ncg_in[j] += m;
            } else {
                self.ncg_out[j] -= m;
            }
        }
        if m_s == 0.0 && m_n.iter().all(|m| *m == 0.0) {
            return Ok((0.0, [0.0; 6], 0.0));
        }
        self.gas_e += e;
        if e > 0.0 {
            self.e_in += e;
        } else {
            self.e_out -= e;
        }
        self.refresh()?;
        Ok((m_s, m_n, e))
    }

    /// `q` J of pool heat. Into the pool (by mass over the layers); into the
    /// atmosphere when the pool is dry; and when it is more than turns the
    /// whole pool into saturated vapour at the atmosphere's pressure, the
    /// pool boils dry: its mass goes to the atmosphere as vapour carrying
    /// its energy and `q` (gh:#1100). Returns the heat that went to the
    /// atmosphere, J.
    fn heat_pool(&mut self, q: f64) -> Result<f64, TampinesError> {
        if !(self.pool_kg > 0.0) {
            self.gas_e += q;
            self.refresh()?;
            return Ok(q);
        }
        if q > 0.0 {
            let t_s = t_sat(self.d.p_gas);
            let e_cap = self.pool_kg * self.h_steam(t_s, p_sat_pa(t_s)) - self.pool_e;
            if q >= e_cap {
                self.gas.steam_kg += self.pool_kg;
                self.gas_e += self.pool_e + q;
                self.empty_pool();
                self.refresh()?;
                return Ok(q);
            }
        }
        self.spread_by_mass(q);
        self.add_pool(0.0, q)?;
        Ok(0.0)
    }

    /// Heat into the pool at elevations, `(z0, z1, q J)` each
    /// ([`ElevatedPoolHeat`]), after `pool_heat` (gh:#1100). One layer, or a
    /// dry pool, or heat enough to boil the whole pool dry: their sum
    /// through [`Self::heat_pool`], exactly as `pool_heat`. Otherwise each
    /// band into its layers ([`Self::band_shares`]). Returns the heat that
    /// went to the atmosphere, J.
    fn heat_pool_at(&mut self, bands: &[(f64, f64, f64)]) -> Result<f64, TampinesError> {
        // No heat: nothing at all (not even a refresh), so a zero band is
        // the step without it, bit for bit.
        if bands.iter().all(|b| b.2 == 0.0) {
            return Ok(0.0);
        }
        let total: f64 = bands.iter().map(|b| b.2).sum();
        if self.layers.is_empty() || !(self.pool_kg > 0.0) {
            return self.heat_pool(total);
        }
        if total > 0.0 {
            let t_s = t_sat(self.d.p_gas);
            if total >= self.pool_kg * self.h_steam(t_s, p_sat_pa(t_s)) - self.pool_e {
                return self.heat_pool(total);
            }
        }
        for &(z0, z1, q) in bands {
            if q == 0.0 {
                continue;
            }
            let shares = self.band_shares(z0, z1);
            let last = shares.len() - 1;
            let mut used = 0.0;
            for (k, (i, f)) in shares.iter().enumerate() {
                let x = if k == last { q - used } else { q * f };
                self.layers[*i].1 += x;
                used += x;
            }
            self.pool_e += q;
        }
        self.refresh()?;
        Ok(0.0)
    }

    /// The layers holding the water between `z0` and `z1` and the share of
    /// that water in each, `(layer, fraction)` bottom first, by the
    /// cumulative mass below each elevation at the pool's mean density (as
    /// [`Self::layer_at`]); a band holding no water (zero thickness, or
    /// above the level) is the layer at `z0`, whole.
    fn band_shares(&self, z0: f64, z1: f64) -> Vec<(usize, f64)> {
        let m0 = (self.d.rho * self.table.volume_below_m3(z0)).clamp(0.0, self.pool_kg);
        let m1 = (self.d.rho * self.table.volume_below_m3(z1)).clamp(0.0, self.pool_kg);
        let mut out = Vec::new();
        if m1 > m0 {
            let mut c = 0.0;
            for (i, l) in self.layers.iter().enumerate() {
                let (a, b) = (c, c + l.0);
                c = b;
                let o = b.min(m1) - a.max(m0);
                if l.0 > 0.0 && o > 0.0 {
                    out.push((i, o / (m1 - m0)));
                }
            }
        }
        if out.is_empty() {
            out.push((self.layer_at(z0), 1.0));
        }
        out
    }

    /// Draw `m` kg of liquid through a port at `z` at pressure `p`, carrying
    /// `h_liquid(T, p)` per kg at the temperature of the layer it comes
    /// from. Returns what moved, kg and J.
    ///
    /// One layer: the old step, `m h` from the pool. Layered: the port's
    /// layer first, then the layers above it (the liquid over the port
    /// flows down to it), then those below; each gives its share of its
    /// energy, and the flow work `m (h − u)` is taken from the pool that
    /// remains, by mass, as the one-layer pool takes it from its total.
    /// Dry-out (gh:#1100): a draw that would leave less than
    /// [`DRY_FRACTION`] of the pool takes the whole pool, which is then
    /// exactly empty; the flow work comes from the atmosphere (the gas
    /// pushing the last liquid out), and the step's compression work is
    /// returned by the end-of-step sweep, so neither is counted twice.
    fn draw_liquid(&mut self, z: f64, m: f64, p: f64) -> Result<(f64, f64), TampinesError> {
        if leaves_dry(self.pool_kg, m) {
            let m_all = self.pool_kg;
            let e = if self.layers.is_empty() {
                m_all * self.h_liquid(self.d.t_pool, p)
            } else {
                self.layers.iter().zip(&self.layer_t).map(|(l, t)| l.0 * self.h_liquid(*t, p)).sum()
            };
            self.gas_e -= e - self.pool_e;
            self.empty_pool();
            self.refresh()?;
            return Ok((m_all, e));
        }
        if self.layers.is_empty() {
            let e = m * self.h_liquid(self.d.t_pool, p);
            self.add_pool(-m, -e)?;
            return Ok((m, e));
        }
        let n = self.layers.len();
        let from = self.layer_at(z);
        let order: Vec<usize> = (from..n).chain((0..from).rev()).collect();
        let (mut rem, mut taken, mut e_out, mut u_out) = (m, 0.0, 0.0, 0.0);
        for l in order {
            if !(rem > 0.0) {
                break;
            }
            let (m_l, e_l) = self.layers[l];
            if !(m_l > 0.0) {
                continue;
            }
            let h = self.h_liquid(self.layer_t[l], p);
            if leaves_dry(m_l, rem) {
                e_out += m_l * h;
                u_out += e_l;
                taken += m_l;
                rem -= m_l;
                self.layers[l] = (0.0, 0.0);
            } else {
                let du = e_l * (rem / m_l);
                e_out += rem * h;
                u_out += du;
                taken += rem;
                self.layers[l] = (m_l - rem, e_l - du);
                rem = 0.0;
            }
        }
        self.pool_kg -= taken;
        self.spread_by_mass(u_out - e_out);
        self.add_pool(0.0, -e_out)?;
        Ok((taken, e_out))
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
                // The liquid joins the layer at the port (the top one when
                // the port is above the level).
                let layer = self.layer_at(z);
                self.add_pool_at(layer, m_l, e_l)?;
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
        let z = self.ports[i].elevation.get::<meter>();
        let depth = self.d.level - z;
        bubble_efficiency(depth, self.ports[i].opening_height.get::<meter>(), self.d.p_gas + self.d.rho * G * depth.max(0.0), self.local_t(z))
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
        // The liquid the bubbles meet: the layer at the port.
        let layer = self.layer_at(self.ports[i].elevation.get::<meter>());
        let t_p = self.t_layer(layer);
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
        self.add_pool_at(layer, m_s - to_atm_steam, e_in - to_atm_e)?;
        let vol = (n_tot + to_atm_steam / M_H2O) * R_GAS * t_p / p_g;
        Ok((eps, to_atm_steam, vol))
    }

    /// The pool surface. **The wet well's `surface`** (`sembawang`'s
    /// `Containment::pool_surface_step`): flashing of a pool above the
    /// saturation temperature at the atmosphere's pressure (since gh:#1107
    /// to the saturation temperature of the pressure after the flash,
    /// [`Self::flash_temperature`]), MELCOR's surface
    /// exchange (or the sensible-only ablation), and rain-out. Returns the
    /// flashed, the surface-evaporated and the rained-out masses, kg.
    fn surface(&mut self, dt: f64) -> Result<(f64, f64, f64), TampinesError> {
        // The liquid at the surface: the pool, or a layered pool's top
        // holding layer (gh:#1100). A dry pool has no surface: only
        // rain-out acts, and refills it.
        let top = self.surface_block();
        let (mut flash, mut evap) = (0.0, 0.0);
        if !(self.surface_liquid().0 > 0.0) {
            let rain = self.rain_out()?;
            return Ok((flash, evap, rain));
        }
        // Flashing: the vapour leaves as saturated steam at `T_s`; the pool
        // ends at `T_s` (energy: `E_p = (m − x) u_l(T_s) + x h_g(T_s)`). A
        // flash that would leave it dry takes it all, with all its energy.
        // By default `T_s` is the saturation temperature of the pressure
        // **after** the flash (gh:#1107, [`Self::flash_temperature`]); the
        // ablation takes it before, explicitly.
        let t_s = t_sat(self.d.p_gas);
        let (m_s, e_s, t_l, _) = self.surface_liquid();
        if t_l > t_s {
            let t_f = if self.mech.implicit_flash { self.flash_temperature(top, t_s, t_l, m_s, e_s)? } else { t_s };
            flash = self.apply_flash(top, t_f, m_s, e_s)?;
        }
        if !(self.surface_liquid().0 > 0.0) {
            let rain = self.rain_out()?;
            return Ok((flash, evap, rain));
        }
        let top = self.surface_block();
        match self.mech.surface {
            SurfaceModel::Melcor => {
                let d = self.d;
                let (m_s, e_s, t_l, depth) = self.surface_liquid();
                let area = self.table.area_at(Length::new::<meter>(d.level)).get::<uom::si::area::square_meter>();
                let cp = self.cv_liquid(t_l);
                let pool = PoolState { t_k: t_l, kg: m_s, rho: d.rho, cp };
                let geometry = SurfaceGeometry { area_m2: area, pool_depth_m: depth, gas_depth_m: d.v_gas / area };
                let ex = if self.mech.ncg_transport_properties {
                    let gas = GasMixtureAtmosphere { t_k: d.t_gas, p_pa: d.p_gas, n_steam: self.gas.steam_kg / M_H2O, n_ncg: self.gas.ncg_moles };
                    exchange_mixture(&pool, &gas, &geometry, dt)
                } else {
                    exchange(&pool, &Atmosphere { t_k: d.t_gas, p_pa: d.p_gas, n_steam: self.gas.steam_kg / M_H2O, n_n2: self.gas.ncg_total_moles() }, &geometry, dt)
                };
                let m = ex.evaporation_kg_s * dt;
                // The vapour leaves the pool at the liquid's enthalpy and joins
                // the atmosphere at the steam's; MELCOR's balance set `m` with
                // `exchange`'s own `h_fg`, so the difference (zero under the
                // constant-`c_p`, ideal-steam ablations) is taken from the
                // atmosphere's heat so energy closes on this volume's values.
                let p_s = self.gas.steam_kg / M_H2O / (self.gas.steam_kg / M_H2O + self.gas.ncg_total_moles()) * d.p_gas;
                let h_f = self.h_liquid(t_l, d.p_gas);
                let h_g = self.h_steam(d.t_gas, p_s);
                let corr = m * ((h_g - h_f) - (steam_h(d.t_gas) - cp * (t_l - 273.15)));
                let e_p = m * h_f + ex.q_pool_w * dt;
                let e_a = m * h_g - ex.q_atm_w * dt - corr;
                // Dry-out by evaporation (gh:#1100): the surface liquid goes
                // to the atmosphere whole, with its energy, the gas
                // supplying the rest of the latent heat, when the
                // evaporation would leave it dry, or would leave what
                // remains below the triple point. MELCOR's Eq 5-17 limit
                // caps the pool's heat by the whole pool's sensible heat
                // while the evaporated mass leaves at the pool's enthalpy,
                // so what remains is colder than the surface by `m (p/ρ +
                // c_p ΔT) / (m_s − m)` per kg: negligible for a pool, but
                // unbounded as a film evaporates (found on a 0.05 kg film,
                // 2026-11-01: 221 K, then a negative energy). Liquid below
                // 273.16 K is outside the model (no ice), so that is the
                // floor, not a tuned margin.
                let too_cold = m > 0.0 && m < m_s && (e_s - e_p) < (m_s - m) * self.u_liquid(273.16);
                if m > 0.0 && (leaves_dry(m_s, m) || too_cold) {
                    self.gas.steam_kg += m_s;
                    self.gas_e += e_s;
                    self.take_surface(top, m_s, e_s, m_s);
                    self.refresh()?;
                    evap = m_s;
                } else {
                    self.take_surface(top, m, e_p, m_s);
                    self.gas.steam_kg += m;
                    self.gas_e += e_a;
                    self.refresh()?;
                    evap = m;
                }
            }
            SurfaceModel::SensibleOnly { coefficient } => {
                let d = self.d;
                let (m_s, _, t_l, _) = self.surface_liquid();
                let area = self.table.area_at(Length::new::<meter>(d.level)).get::<uom::si::area::square_meter>();
                let k = area * coefficient.get::<watt_per_square_meter_kelvin>() * dt;
                let c_g = self
                    .model
                    .heat_capacity(&self.gas, Volume::new::<cubic_meter>(d.v_gas), kelv(d.t_gas))
                    .map_err(|e| TampinesError::Unphysical(format!("atmosphere: {e}")))?
                    .get::<uom::si::heat_capacity::joule_per_kelvin>();
                let c_p = m_s * self.cv_liquid(t_l);
                let q = k * (t_l - d.t_gas) / (1.0 + k / c_g + k / c_p);
                self.take_surface(top, 0.0, q, m_s);
                self.gas_e += q;
                self.refresh()?;
            }
        }
        let rain = self.rain_out()?;
        Ok((flash, evap, rain))
    }

    /// Flash the surface liquid `(m_s, e_s)` of block `top` to `t_f`: the
    /// vapour, `m = (E_s − m_s u_l(T_f)) / (h_g(T_f) − u_l(T_f))`, joins
    /// the atmosphere as saturated steam at `t_f` (all of the liquid, with
    /// all its energy, if it would leave it dry; nothing if `m ≤ 0`).
    /// Returns the flashed mass, kg.
    fn apply_flash(&mut self, top: (usize, usize), t_f: f64, m_s: f64, e_s: f64) -> Result<f64, TampinesError> {
        let h_g = self.h_steam(t_f, p_sat_pa(t_f));
        let u_l = self.u_liquid(t_f);
        let m = (e_s - m_s * u_l) / (h_g - u_l);
        if !(m > 0.0) {
            return Ok(0.0);
        }
        let (m, e) = if leaves_dry(m_s, m) { (m_s, e_s) } else { (m, m * h_g) };
        self.gas.steam_kg += m;
        self.gas_e += e;
        self.take_surface(top, m, e, m_s);
        self.refresh()?;
        Ok(m)
    }

    /// The flash's end temperature (gh:#1107): the root `T_f` of
    /// `T_sat(p(T_f)) = T_f`, `p(T_f)` the atmosphere's pressure after the
    /// flash to `T_f` ([`Self::apply_flash`] on a probe, with the pool's
    /// volume and density re-derived), between `t_s = T_sat(p)` before the
    /// flash (where the explicit flash's vapour has raised `p`, so the
    /// residual is positive) and the liquid's own temperature `t_l` (no
    /// flash, residual `t_s − t_l < 0`). The residual falls monotonically
    /// across the bracket (less vapour, lower pressure), so the Illinois
    /// variant of regula falsi converges; it stops at a bracket of 1e-10 K
    /// (a solver control, not physics: about 3 orders above the 50-step
    /// bisection in [`t_sat`]).
    ///
    /// **Why** (MELCOR CVH/FL RM §5.1.3, Eq 5-30; §4.2.1, Eqs 4-26 to 4-36;
    /// §4.3, cited, not quoted): MELCOR sets the pool's vapour from its
    /// mass and enthalpy at the end-of-step pressure, solves the vapour's
    /// separation implicitly with the pressures, and requires end-of-step
    /// pressures consistent with the contents. The explicit flash (the
    /// [`Self::without_implicit_flash`] ablation) takes `T_sat` before its
    /// own vapour raises the pressure, and overshoots by the loop gain
    /// `G = (M c / h_fg) (dT_sat/dp) (∂p/∂m)`; the network's Picard
    /// iteration then couples this end state to the flows.
    fn flash_temperature(&self, top: (usize, usize), t_s: f64, t_l: f64, m_s: f64, e_s: f64) -> Result<f64, TampinesError> {
        let resid = |t: f64| -> Result<f64, TampinesError> {
            let mut probe = self.clone();
            probe.apply_flash(top, t, m_s, e_s)?;
            Ok(t_sat(probe.d.p_gas) - t)
        };
        let (mut a, mut b) = (t_s, t_l);
        let mut fa = resid(a)?;
        if !(fa > 0.0) {
            return Ok(a);
        }
        let mut fb = t_s - t_l;
        let mut side = 0i8;
        for _ in 0..100 {
            if b - a <= 1e-10 {
                break;
            }
            let mut c = (a * fb - b * fa) / (fb - fa);
            if !(c > a && c < b) {
                c = 0.5 * (a + b);
            }
            let fc = resid(c)?;
            if fc > 0.0 {
                a = c;
                fa = fc;
                if side == 1 {
                    fb *= 0.5;
                }
                side = 1;
            } else {
                b = c;
                fb = fc;
                if side == -1 {
                    fa *= 0.5;
                }
                side = -1;
            }
        }
        Ok(0.5 * (a + b))
    }

    /// The surface mixed block of a layered pool, `(lowest, top)` layer
    /// indices, inclusive (gh:#1100): from the top holding layer down, every
    /// holding layer whose IF97 density at the atmosphere's pressure is not
    /// above the block's (the column between them neutral or unstable, so
    /// it convects with the surface water), stopping at the first stable
    /// density step. Empty layers are passed over. The comparison allows
    /// 1e-12 of the density for round-off (the convective adjustment hands
    /// a merged block's top layer the energy remainder, a few ulp; 1e-12 of
    /// the density is about 3e-9 K), not as a physical coefficient. Densities
    /// are taken as [`Self::mix_unstable`] takes them, so a block the
    /// adjustment has just merged is recognised whole. With the
    /// [`Self::without_surface_mixed_block`] ablation, the top layer alone.
    fn surface_block(&self) -> (usize, usize) {
        let top = self.top_layer();
        if self.layers.is_empty() || !self.mech.surface_mixed_block || !(self.layers[top].0 > 0.0) {
            return (top, top);
        }
        let p = self.d.p_gas;
        let guess = self.d.t_pool;
        let rho = |m: f64, e: f64| rho_liquid(self.t_liquid(e / m, guess), p);
        let (mut m, mut e) = self.layers[top];
        let mut rho_b = rho(m, e);
        let mut lo = top;
        for i in (0..top).rev() {
            let l = self.layers[i];
            if !(l.0 > 0.0) {
                continue;
            }
            if rho(l.0, l.1) > rho_b * (1.0 + 1e-12) {
                break;
            }
            m += l.0;
            e += l.1;
            rho_b = rho(m, e);
            lo = i;
        }
        (lo, top)
    }
    /// The surface liquid: mass, energy, temperature and depth (the pool's,
    /// or a layered pool's surface mixed block, [`Self::surface_block`]:
    /// its mass, energy, temperature from its specific energy, and its
    /// depth, the level less the height of the water below it).
    fn surface_liquid(&self) -> (f64, f64, f64, f64) {
        if self.layers.is_empty() {
            return (self.pool_kg, self.pool_e, self.d.t_pool, self.d.level - self.table.bottom_m());
        }
        let (lo, top) = self.surface_block();
        if lo == top {
            return (self.layers[top].0, self.layers[top].1, self.layer_t[top], self.top_layer_thickness());
        }
        let (m, e) = self.layers[lo..=top].iter().fold((0.0, 0.0), |a, l| (a.0 + l.0, a.1 + l.1));
        let below: f64 = self.layers.iter().take(lo).map(|l| l.0).sum();
        let depth = self.d.level - self.table.height_of_volume_m(below / self.d.rho);
        (m, e, self.t_liquid(e / m, self.layer_t[top]), depth)
    }
    /// Take `dm` kg and `de` J from the surface liquid (the block `(lo,
    /// top)`), which held `held` kg, shared over the block's layers by mass
    /// (the top one taking the remainder; a block of one layer takes it all,
    /// as before the block); when `dm` is all of it, it ends exactly empty.
    fn take_surface(&mut self, block: (usize, usize), dm: f64, de: f64, held: f64) {
        let (lo, top) = block;
        let whole = dm > 0.0 && dm == held;
        if top < self.layers.len() {
            if whole {
                for l in &mut self.layers[lo..=top] {
                    *l = (0.0, 0.0);
                }
            } else {
                let (mut um, mut ue) = (0.0, 0.0);
                for l in &mut self.layers[lo..top] {
                    if l.0 > 0.0 {
                        let x = l.0 / held;
                        let (a, b) = (dm * x, de * x);
                        l.0 -= a;
                        l.1 -= b;
                        um += a;
                        ue += b;
                    }
                }
                let l = &mut self.layers[top];
                l.0 -= dm - um;
                l.1 -= de - ue;
            }
        }
        self.pool_kg -= dm;
        self.pool_e -= de;
        if whole && (self.layers.is_empty() || self.layers.iter().all(|l| l.0 == 0.0)) {
            self.empty_pool();
        }
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
                let top = self.top_layer();
                self.add_pool_at(top, cond, e)?;
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
        let top = self.top_layer();
        self.add_pool_at(top, cond, e)?;
        total += cond;
        Ok(total)
    }
}
