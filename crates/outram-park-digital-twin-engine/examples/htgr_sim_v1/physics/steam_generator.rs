//! Nodalised counter-flow steam generator: hot fluid <-> tube metal <-> water/steam.
//!
//! A **spatially resolved** once-through steam generator, built by composing
//! three axial arrays that already exist in the workspace and coupling them
//! laterally through real thermal conductances:
//!
//! ```text
//!   hot fluid  (CompressibleFluidArray)   ---->  flows +x
//!        |  UA_hot / node
//!   tube metal (SolidColumn, SteelSS304L) ---- thermal mass, no flow
//!        |  UA_cold / node
//!   water/steam (TampinesSteamArray)      <----  flows -x  (COUNTER-FLOW)
//! ```
//!
//! ## The defect this replaces
//!
//! Until 2026-08-12 `super::primary_loop` modelled the secondary side as an
//! **isothermal sink at saturation** and took the duty from an
//! effectiveness-NTU lump:
//!
//! ```text
//! Q = eps * m_dot * c_p * (T_hot - T_sat)
//! ```
//!
//! That is the correct shape for an *evaporator*, where the cold side really is
//! isothermal. It is wrong for a **once-through** unit, which is an economiser,
//! an evaporator **and a superheater** in series: as the steam superheats, the
//! local driving temperature difference collapses toward zero, but against a
//! fixed saturation sink it never does. With helium near 973 K and saturation at
//! 523 K the model saw a permanent ~450 K driver, over-predicted the duty, and
//! the steam outlet -- computed downstream as `h_feed + Q/m_dot` -- ran far too
//! hot. A `max_absorbable_duty` cap in [`super::secondary_loop`] then clamped the
//! outlet at the *hot-side inlet temperature*, which turned a crash into a
//! visibly wrong number.
//!
//! Here the driving difference is evaluated **node by node at local
//! temperatures**, so the superheater's collapsing pinch is represented rather
//! than assumed away.
//!
//! ## No temperature cross, structurally
//!
//! Lateral heat between any two adjacent arrays is
//!
//! ```text
//! q_i = UA_i * (T_upstream_array_i - T_this_array_i)
//! ```
//!
//! at the **local node temperatures** of the two arrays. If the cold stream ever
//! became hotter than the hot stream at some station, `q_i` simply **changes
//! sign** -- heat flows back the other way, which is what the second law
//! requires. Nothing is clamped, nothing is capped, and no branch tests for a
//! cross. This is the property the effectiveness-NTU lump could never have had,
//! because a lump has only one temperature per side and therefore no local
//! difference to take the sign of.
//!
//! [`SteamGeneratorState::worst_node_cross_kelvin`] reports the measured margin.
//!
//! ## Counter-flow is expressed as an index mapping, not a negative flow
//!
//! Both fluid arrays run **forward in their own frame** (positive mass flow,
//! inlet at cell 0). The counter-flow arrangement is carried by the *lateral
//! index map*: cold cell `j` sits at the same physical station as hot cell
//! `n-1-j`, so the temperature vector handed to each array is reversed on the
//! way across.
//!
//! TUAS's own [`SimpleShellAndTubeHeatExchanger`] instead gives its shell side a
//! **negative** mass flowrate and swaps which terminal carries the inlet
//! boundary condition. Both are correct and they are the same exchanger; the
//! index map was chosen here because it keeps each array on the numerically
//! well-exercised forward-flow path, and because a counter-flow arrangement
//! genuinely *is* nothing more than a permutation of which node faces which.
//! [`tests::counter_flow_index_map_is_its_own_inverse`] pins the mapping.
//!
//! [`SimpleShellAndTubeHeatExchanger`]:
//!     tuas_boussinesq_solver::pre_built_components::shell_and_tube_heat_exchanger::SimpleShellAndTubeHeatExchanger
//!
//! ## The metal is a real thermal mass, and that is the point
//!
//! The tube wall is a [`SolidColumn`] of [`SolidMaterial::SteelSS304L`], whose
//! mass is **derived from the tube geometry** and whose specific heat comes from
//! the TUAS solid-property database. It therefore has a real thermal time
//! constant, and a step change in duty cannot be tracked instantly by the steam
//! outlet. Before this module the secondary loop's only integrated state was its
//! mass flow; the metal is what makes it genuinely transient.
//!
//! ## What is real
//!
//! - **Both fluids are real equations of state.** The hot side is a CoolProp
//!   Helmholtz EOS through [`CompressibleFluidArray`]; the cold side is
//!   IAPWS-IF97 through [`TampinesSteamArray`], which carries the water/steam
//!   phase change including the latent heat.
//! - **The metal is real steel.** Density and specific heat come from
//!   `SolidMaterial::SteelSS304L`, so the thermal capacity is derived, not typed
//!   in.
//! - **Both streams are advection-driven** through prescribed mass-flow inlets
//!   and TUAS upwind-advection junction terminals (see
//!   `crates/tampines-steam-tables/docs/boundary-conditions-convention.md`), so
//!   each stream develops a genuine axial temperature profile.
//! - **The energy balance closes** across all three arrays -- see
//!   [`tests::energy_balance_closes_across_the_exchanger`].
//!
//! ## What is illustrative
//!
//! - **The conductances are a calibration, not a correlation.** `UA_hot` and
//!   `UA_cold` are constructor parameters. The HTGR caller supplies values whose
//!   *series* combination is the same 1.0e5 W/K the previous effectiveness-NTU
//!   lump used -- deliberately **not** re-tuned as part of this change, so any
//!   movement in the design point is attributable to the nodalisation and not to
//!   a fitted number. There is no Dittus-Boelter / Gnielinski evaluation per
//!   node here, and no helical-coil correlation.
//! - **The tube geometry is part published, part invented.** See
//!   [`SteamGeneratorGeometry::htr10_illustrative`].
//! - **The steam pressure is held fixed**, so there is no sliding-pressure or
//!   inventory dynamics; the cold array's outlet pressure is prescribed.
//! - **The node count is small** (see [`SteamGeneratorConfig::node_count`]), so
//!   this resolves *that the zones exist and where the pinch is*, not a
//!   converged axial profile.
//! - **The coupling is explicit (Lie-split), so the energy balance closes to
//!   about 0.34%, not to round-off.** Each array's lateral conductance is
//!   evaluated against its neighbours' previous sub-timestep temperatures, and
//!   the two fluid arrays treat that source differently from the implicit solid.
//!   The residual converges with the sub-timestep and is measured, not assumed:
//!   see [`tests::energy_balance_closes_across_the_exchanger`].
//!
//! ## Two hard limits you will hit before the physics gets interesting
//!
//! Both are **panics in dependencies**, not errors this module can return, and
//! both are documented rather than guarded because guarding them would mean
//! silently continuing with a nonphysical state:
//!
//! - **The tube metal is tabulated to 1000 K.** `SolidMaterial::SteelSS304L`
//!   carries properties to 726.85 degC and TUAS panics rather than
//!   extrapolating. At the HTR-10 design point the hottest metal node sits at
//!   **765.5 K**, a 235 K margin, and the reactor protection system's
//!   core-outlet trip is at 750 degC = 1023 K -- but the protection system is
//!   *disarmed by default* in this simulator, so a deliberate excursion with it
//!   off can reach the ceiling. Measured 2026-08-12: throttling the feedwater to
//!   1.0 kg/s at full power for 100 s does it.
//! - **The water side is tabulated to 1073.15 K** (IAPWS-IF97 regions 1, 2, 4),
//!   and `tampines-steam-tables` panics rather than returning an error. The
//!   exchanger cannot itself drive the steam past the helium, so this is only
//!   reachable via a hot side above 800 degC.
//!
//! The plant's crash modal names the failing component, so either shows up as
//! "steam generator + secondary steam loop" rather than an anonymous panic.
//!
//! ## Cost -- this module *is* the simulator's cost
//!
//! Each `advance_timestep` runs three coupled array solves per sub-timestep,
//! and each solve's outer correctors each run the array's equation of state
//! over every cell. Those EOS flashes -- a CoolProp Helmholtz solve on the hot
//! side, an IAPWS-IF97 `(p, h)` flash on the cold -- are where essentially all
//! of the wall clock goes.
//!
//! **Measured 2026-08-13** (release, 8 nodes, the shipped 0.0125 s substep,
//! then 4 outer correctors): **1.9585 s of compute per second of simulated
//! time** on its own, against **1.9469** for the whole plant around it. The
//! exchanger is therefore about **96% of `htgr_sim_v1`'s compute**, and the
//! remaining 4% is everything else in the plant put together.
//!
//! Two consequences, both important:
//!
//! - **Its cost does not depend on the plant timestep.** It is a multi-rate
//!   sub-model on its own clock, so raising the plant step from 1 ms to 0.1 s
//!   moved the whole-plant real-time ratio only from 0.492 to 0.514.
//! - **Cost is very nearly exactly linear in `n_outer / substep`.** Measured
//!   the same day: 0.51 at (1 corrector, 0.0125 s), 0.99 at (2, 0.0125 s), 1.96
//!   at (4, 0.0125 s), 0.50 at (2, 0.025 s). Dropping the shipped corrector
//!   count from 4 to 2 halved the cost and moved the settled duty by nothing
//!   measurable -- see
//!   [`tests::the_corrector_substep_trade_is_measured`].
//!
//! The substep cannot be raised further because of the hot side's **Courant**
//! limit, and more correctors do not lift it -- see
//! [`tests::the_courant_number_bounds_the_array_substep`].
//!
//! This is a demonstration model, **not a validated steam-generator model**.
//!
//! ## Promotion
//!
//! This module is written to be moved into `tampines` unchanged (bead
//! `op-szmi.14`): it imports nothing from the example (`crate::`), knows nothing
//! about `HtgrSnapshot` or the GUI, and takes geometry, conductances, node count
//! and **both fluids** as constructor parameters. The hot side does not assume
//! helium -- `fhr_sim_v2` will pass a molten salt through the same
//! [`CoolPropFluid`] parameter.

use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use tampines_steam_tables::TampinesSteamArray;
use tuas_boussinesq_solver::array_fluid_collections::solid_array_lateral_coupling::SolidColumn;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::density::try_get_rho;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::{Material, SolidMaterial};
use tuas_boussinesq_solver::fluid_mechanics_correlations::courant_number::get_fluid_courant_number_one_dimension;

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, HeatCapacity, Length, Mass, MassRate, Power, Pressure,
    ThermalConductance, ThermodynamicTemperature, Time, Velocity, Volume,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

/// Tube-bundle geometry of a once-through steam generator.
///
/// Everything here is an **aggregate over the whole unit** -- all modules, all
/// tubes -- because the model resolves the exchanger axially, not tube by tube.
///
/// Units are spelled out even though `uom` enforces them:
///
/// | Field | Quantity | Unit |
/// |---|---|---|
/// | `tube_count` | number of parallel tubes (dimensionless) | - |
/// | `tube_length` | developed length of **one** tube | m |
/// | `tube_inner_diameter` | tube bore | m |
/// | `tube_outer_diameter` | tube outside diameter | m |
/// | `shell_flow_area` | aggregate hot-side free-flow area | m^2 |
/// | `shell_flow_length` | hot-side flow path length through the unit | m |
///
/// Valid ranges: every field must be strictly positive, and
/// `tube_outer_diameter > tube_inner_diameter`. [`Self::validate`] checks this.
#[derive(Clone, Copy, Debug)]
pub struct SteamGeneratorGeometry {
    /// Number of parallel heat-transfer tubes in the whole unit.
    pub tube_count: f64,
    /// Developed length of one tube \[m\]. For a helical-coil unit this is the
    /// wound length, not the module height.
    pub tube_length: Length,
    /// Tube bore \[m\] -- the water/steam side.
    pub tube_inner_diameter: Length,
    /// Tube outside diameter \[m\] -- the hot-fluid side.
    pub tube_outer_diameter: Length,
    /// Aggregate free-flow area seen by the hot fluid \[m^2\].
    pub shell_flow_area: Area,
    /// Hot-fluid flow path length through the unit \[m\]. Distinct from
    /// `tube_length` for a helical-coil unit, where the tubes are much longer
    /// than the shell they are wound inside.
    pub shell_flow_length: Length,
}

impl SteamGeneratorGeometry {
    /// The HTR-10 steam generator, **part published and part invented**.
    ///
    /// **Published** (`docs/reactor-scoping/htr10-plant-data.md` section 5,
    /// which cites Wu, Lin & Zhong (2002) and Zhang et al. (2009)):
    ///
    /// - once-through, modular **helical tube**, **30 modules**;
    /// - **34 m** developed length per tube;
    /// - tube material **2.25Cr1Mo**, maximum design temperature 500 degC.
    ///
    /// **Invented, because no source in that sheet carries it** -- the sheet
    /// records "Tube diameter and wall thickness per section: *not stated in any
    /// of the five sources*", "Coil pitch: *not stated*" and "Number of tubes
    /// per module: *not stated*":
    ///
    /// - **3 tubes per module**, so 90 tubes in total;
    /// - **19 mm** outside diameter on a **2.5 mm** wall, so a 14 mm bore;
    /// - **0.25 m^2** aggregate shell-side free-flow area, sized for a ~10 m/s
    ///   helium velocity at the rated 4.3 kg/s and 3.0 MPa;
    /// - **5.0 m** shell flow length.
    ///
    /// **The published 56 m^2 total heat-transfer area is deliberately NOT used.**
    /// The scoping sheet flags it as "arithmetically implausible" -- it implies a
    /// 179 kW/m^2 average heat flux on a gas-heated surface -- and instructs
    /// "do not size the simulator's SG from it". This geometry gives 182.6 m^2
    /// instead, which is 3.3x that figure; the disagreement is recorded rather
    /// than reconciled, since the source is unreliable in the direction that
    /// would matter.
    ///
    /// **The material substitution is deliberate and is a known approximation.**
    /// The published tube material is 2.25Cr1Mo; the workspace's solid-property
    /// database (`SolidMaterial`) does not carry it, so
    /// [`SolidMaterial::SteelSS304L`] stands in. Both are steels of similar
    /// density and specific heat, so the *thermal mass* -- the only property this
    /// model uses the metal for -- is close; the creep and corrosion behaviour
    /// that actually distinguishes them is not modelled at all.
    pub fn htr10_illustrative() -> Self {
        Self {
            tube_count: 30.0 * 3.0,
            tube_length: Length::new::<meter>(34.0),
            tube_inner_diameter: Length::new::<meter>(0.014),
            tube_outer_diameter: Length::new::<meter>(0.019),
            shell_flow_area: Area::new::<square_meter>(0.25),
            shell_flow_length: Length::new::<meter>(5.0),
        }
    }

    /// Aggregate bore cross-section the water/steam flows through \[m^2\].
    pub fn tube_flow_area(&self) -> Area {
        let d = self.tube_inner_diameter.get::<meter>();
        Area::new::<square_meter>(self.tube_count * std::f64::consts::PI * 0.25 * d * d)
    }

    /// Aggregate **metal** cross-section of the tube walls \[m^2\]: the annulus
    /// `pi/4 (d_o^2 - d_i^2)` summed over every tube. This is what carries the
    /// metal's thermal mass and its axial conduction.
    pub fn metal_cross_section(&self) -> Area {
        let d_o = self.tube_outer_diameter.get::<meter>();
        let d_i = self.tube_inner_diameter.get::<meter>();
        Area::new::<square_meter>(
            self.tube_count * std::f64::consts::PI * 0.25 * (d_o * d_o - d_i * d_i),
        )
    }

    /// Total volume of tube metal \[m^3\] = metal cross-section x tube length.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn metal_volume(&self) -> Volume {
        Volume::new::<cubic_meter>(
            self.metal_cross_section().get::<square_meter>() * self.tube_length.get::<meter>(),
        )
    }

    /// Mass of tube metal \[kg\], **derived** from [`Self::metal_volume`] and the
    /// material's own density -- never a typed-in number.
    ///
    /// `SolidMaterial::SteelSS304L` has a temperature-independent density of
    /// 8030 kg/m^3 (Zou, Hu & Charpentier, ANL/NSE-19/11), so `temperature` and
    /// `pressure` are carried for interface generality and for materials whose
    /// density does vary.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn metal_mass(
        &self,
        material: SolidMaterial,
        temperature: ThermodynamicTemperature,
        pressure: Pressure,
    ) -> Mass {
        let rho = try_get_rho(Material::Solid(material), temperature, pressure)
            .map(|d| d.get::<uom::si::mass_density::kilogram_per_cubic_meter>())
            .unwrap_or(0.0);
        Mass::new::<kilogram>(rho * self.metal_volume().get::<cubic_meter>())
    }

    /// Thermal capacity of the tube metal \[J/K\] = `m c_p`, both derived from
    /// the material database. This is what sets the metal time constant
    /// `tau = C / UA`.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn metal_thermal_capacity(
        &self,
        material: SolidMaterial,
        temperature: ThermodynamicTemperature,
        pressure: Pressure,
    ) -> HeatCapacity {
        let cp = try_get_cp(Material::Solid(material), temperature, pressure)
            .map(|c| c.get::<joule_per_kilogram_kelvin>())
            .unwrap_or(0.0);
        HeatCapacity::new::<joule_per_kelvin>(
            self.metal_mass(material, temperature, pressure)
                .get::<kilogram>()
                * cp,
        )
    }

    /// Outside heat-transfer area \[m^2\] -- the hot-fluid-wetted surface,
    /// `pi d_o L` per tube.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn outer_heat_transfer_area(&self) -> Area {
        Area::new::<square_meter>(
            self.tube_count
                * std::f64::consts::PI
                * self.tube_outer_diameter.get::<meter>()
                * self.tube_length.get::<meter>(),
        )
    }

    /// Inside heat-transfer area \[m^2\] -- the water/steam-wetted surface.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn inner_heat_transfer_area(&self) -> Area {
        Area::new::<square_meter>(
            self.tube_count
                * std::f64::consts::PI
                * self.tube_inner_diameter.get::<meter>()
                * self.tube_length.get::<meter>(),
        )
    }

    /// `Ok(())` if the geometry is physically constructible: every dimension
    /// strictly positive and the outside diameter larger than the bore.
    pub fn validate(&self) -> Result<(), SteamGeneratorError> {
        let positive = self.tube_count > 0.0
            && self.tube_length.get::<meter>() > 0.0
            && self.tube_inner_diameter.get::<meter>() > 0.0
            && self.shell_flow_area.get::<square_meter>() > 0.0
            && self.shell_flow_length.get::<meter>() > 0.0;
        if !positive {
            return Err(SteamGeneratorError::NonPositiveGeometry);
        }
        if self.tube_outer_diameter <= self.tube_inner_diameter {
            return Err(SteamGeneratorError::TubeWallNotPositive);
        }
        Ok(())
    }
}

/// Everything needed to build a [`NodalisedCounterFlowSteamGenerator`].
///
/// Deliberately a plain parameter struct with no defaults sourced from any one
/// plant: the same exchanger serves the HTGR (helium hot side) and the FHR
/// (molten salt hot side), so nothing here may assume a working fluid.
#[derive(Clone, Copy, Debug)]
pub struct SteamGeneratorConfig {
    /// Tube-bundle geometry.
    pub geometry: SteamGeneratorGeometry,
    /// Hot-side working fluid. **Not assumed to be helium** -- this is the
    /// parameter that lets the FHR pass a salt through the same exchanger.
    pub hot_fluid: CoolPropFluid,
    /// Hot-side operating pressure \[Pa\]. Held fixed; the hot array's outlet
    /// pressure is prescribed at this value.
    pub hot_pressure: Pressure,
    /// Cold-side (water/steam) operating pressure \[Pa\]. Held fixed.
    pub cold_pressure: Pressure,
    /// Tube-wall material.
    pub metal: SolidMaterial,
    /// Total conductance between the **hot fluid and the metal** \[W/K\], summed
    /// over the whole exchanger. Divided equally between nodes internally.
    pub hot_side_conductance: ThermalConductance,
    /// Total conductance between the **metal and the water/steam** \[W/K\].
    pub cold_side_conductance: ThermalConductance,
    /// Number of axial nodes. Must be at least 3 (the [`SolidColumn`] carries a
    /// back node, a front node and at least one interior node).
    pub node_count: usize,
    /// The **fixed timestep the three arrays are advanced with** \[s\].
    ///
    /// The exchanger runs on its own clock. Whatever `dt` the caller passes to
    /// [`NodalisedCounterFlowSteamGenerator::advance_timestep`] is accumulated,
    /// and the arrays are advanced in whole steps of exactly this size; any
    /// remainder is carried to the next call, so no simulated time is lost or
    /// double-counted.
    ///
    /// # Why a fixed step and not "at most this large"
    ///
    /// **The arrays are unstable both above and below a window**, so a caller's
    /// timestep cannot simply be subdivided or passed through. Measured
    /// 2026-08-12, with the duty column re-measured 2026-08-13 against a
    /// 0.00625 s reference:
    ///
    /// | Array timestep | `Co_hot` | Settled `Q_hot` | Outcome |
    /// |---|---|---|---|
    /// | 0.1 s | 1.776 | -- | **fails** (also at 8 and 32 outer correctors) |
    /// | 0.05 s | 0.888 | -- | **fails** (also at 4, 8, 16 outer correctors); enthalpy goes odd-even and clamps |
    /// | 0.025 s | 0.444 | 9.8244 MW | stable, but **+1.44%** off converged |
    /// | **0.0125 s** | **0.222** | **9.6854 MW** | **stable, +0.003% off converged** |
    /// | 0.00625 s | 0.111 | 9.6851 MW | reference |
    /// | 0.001 s | 0.018 | -- | water side resolves its own acoustic transient; the IF97 `(p,h)` flash leaves Region 5 range and **panics** |
    ///
    /// The **upper** bound is a Courant limit on the hot gas side and is not
    /// negotiable by raising the outer-corrector count -- see
    /// [`PimpleCorrectors`] and
    /// [`tests::the_courant_number_bounds_the_array_substep`].
    ///
    /// The **lower** bound used to be the one that bit in practice: until
    /// 2026-08-13 `htgr_sim_v1`'s GUI stepped its plant at **1 ms**, and handed
    /// straight to the arrays that is the panicking case -- which is what the
    /// simulator did on its first wiring, dying in the crash modal within 30 s
    /// of launch. The plant now steps at
    /// [`crate::physics::PLANT_TIMESTEP_S`] = 0.1 s, which is above the window
    /// rather than below it, so the accumulator is now protecting against the
    /// *upper* bound; either way it is needed.
    ///
    /// Accumulating to a fixed substep makes the exchanger a **multi-rate**
    /// sub-model, which is also what its physics wants: nothing in it moves on a
    /// millisecond scale, the fastest thing being the shell transport at a few
    /// hundred milliseconds and the slowest the 38 s tube metal. It is also what
    /// makes the exchanger's cost independent of the plant timestep -- and,
    /// since it is 96% of that cost, what made raising the plant timestep worth
    /// so little on its own.
    pub substep: Time,
    /// Temperature the **hot-inlet end** of the exchanger is seeded at.
    ///
    /// All three arrays are seeded on one linear *station* profile running from
    /// this value at station 0 to [`Self::initial_cold_end_temperature`] at
    /// station `n-1`. Seeding all three on the same profile means every node
    /// starts at its neighbours' temperature, so the exchanger opens at zero
    /// transfer with no temperature cross and no start-up shock. A uniform seed
    /// instead slams a 450 K step onto the hot inlet cell, which at a gas
    /// array's acoustic timescale is a genuine numerical transient rather than a
    /// physical one.
    pub initial_hot_end_temperature: ThermodynamicTemperature,
    /// Temperature the **cold-inlet end** of the exchanger is seeded at. See
    /// [`Self::initial_hot_end_temperature`].
    pub initial_cold_end_temperature: ThermodynamicTemperature,
    /// Temperature the **cold stream's own outlet** is seeded at -- its state at
    /// station 0, the hot-inlet end.
    ///
    /// This is a third seed rather than a reuse of
    /// [`Self::initial_hot_end_temperature`] because the two streams do **not**
    /// meet at the hot end: the whole point of a counter-flow exchanger is that
    /// the cold stream leaves *below* the hot stream's inlet, by the hot-end
    /// approach. Seeding the cold stream at the hot stream's temperature makes
    /// the exchanger open with steam as hot as the helium, which is a start-up
    /// artefact that looks exactly like the defect this module was built to
    /// remove. Measured 2026-08-12: seeding the two streams together drove the
    /// downstream second-law backstop to 99.6% utilisation on the very first
    /// plant step; seeding them apart drops it well clear.
    pub initial_cold_outlet_temperature: ThermodynamicTemperature,
    /// PIMPLE corrector counts and under-relaxation for the **hot** fluid
    /// array.
    pub hot_correctors: PimpleCorrectors,
    /// PIMPLE corrector counts and under-relaxation for the **cold**
    /// (water/steam) array.
    pub cold_correctors: PimpleCorrectors,
}

/// PIMPLE outer/inner corrector counts and under-relaxation factors for one
/// fluid array.
///
/// These used to be hardcoded inside
/// [`NodalisedCounterFlowSteamGenerator::new`]. They are configuration now
/// because the **outer** corrector count is the knob that trades wall-clock
/// cost against the largest usable [`SteamGeneratorConfig::substep`], and that
/// trade is the whole of this exchanger's cost: it is ~96% of `htgr_sim_v1`'s
/// plant compute (measured 2026-08-13, see
/// [`super::PLANT_TIMESTEP_S`]).
///
/// # What each does
///
/// - `n_outer` -- outer (PIMPLE/SIMPLE-like) correctors per array timestep.
///   Each one re-solves momentum, pressure and energy from the same old-time
///   state with the latest iterate. Because both arrays carry their enthalpy
///   convection as an **explicit** `fvc::div_limited` source inside this loop,
///   the loop is a Picard iteration whose contraction factor is the cell
///   Courant number `Co`: residual reduction over the step is roughly
///   `Co^n_outer`. So raising `n_outer` **can** buy a larger substep while
///   `Co < 1`, and cannot help at all once `Co >= 1`.
/// - `n_inner` -- inner pressure correctors within each outer corrector (the
///   PISO loop). These fix the pressure-velocity coupling, not the advection,
///   and are not the Courant knob.
/// - `pressure_relaxation` / `velocity_relaxation` -- outer-loop
///   under-relaxation, in `(0, 1]`. Lower is more robust and slower to
///   converge.
///
/// # Cost
///
/// Each outer corrector costs one full pass of the array's equation of state
/// over every cell, and the EOS flashes (CoolProp Helmholtz on the hot side,
/// IAPWS-IF97 on the cold) are what this model spends its time in. Cost is
/// therefore very close to linear in `n_outer * substeps_per_second`, which is
/// why `n_outer` and `substep` trade against each other almost exactly.
/// [`tests::the_corrector_substep_trade_is_measured`] records the measured
/// grid.
#[derive(Clone, Copy, Debug)]
pub struct PimpleCorrectors {
    /// Outer correctors per array timestep. Clamped to at least 1 by the array.
    pub n_outer: usize,
    /// Inner (PISO) pressure correctors per outer corrector.
    pub n_inner: usize,
    /// Outer-loop pressure under-relaxation factor, `(0, 1]`.
    pub pressure_relaxation: f64,
    /// Outer-loop velocity under-relaxation factor, `(0, 1]`.
    pub velocity_relaxation: f64,
}

impl PimpleCorrectors {
    /// Hot-gas-array settings: **2** outer correctors, 2 inner, both
    /// under-relaxations 0.5.
    ///
    /// # Why 2 and not the 4 this shipped with until 2026-08-13
    ///
    /// Measured that day (see
    /// [`tests::the_corrector_substep_trade_is_measured`]): at the 0.0125 s
    /// substep the settled duty is **9.6854 MW at 1, 2 and 4 outer
    /// correctors** -- identical to five significant figures -- while wall
    /// clock is very nearly exactly linear in the count. Four correctors were
    /// costing 2x for no measurable change in the answer, and this exchanger is
    /// ~96% of `htgr_sim_v1`'s compute, so that factor was the whole difference
    /// between running at half real time and running at real time.
    ///
    /// Two rather than one because the correctors are not useless, they are
    /// merely not *binding* at the design point: they are a Picard iteration on
    /// an explicit convection source whose contraction factor is the cell
    /// Courant number, and the operator can raise the circulator to its 8 kg/s
    /// ceiling (1.86x nominal). Measured 2026-08-13 at that ceiling --
    /// `super::super::tests::the_exchanger_holds_its_courant_margin_at_maximum_circulator_flow`
    /// -- `Co_hot` rises from 0.222 to **0.3604**, still inside the explicit
    /// limiter's window, with **zero** enthalpy clamp events over 100 s. One
    /// corrector is a bare explicit advance with no correction at all; two
    /// keeps a real one, and the worst flow the GUI can command was checked
    /// rather than assumed.
    ///
    /// Raising the count further does **not** buy a larger substep -- a 0.05 s
    /// substep panics at 4, 8 and 16 correctors alike.
    pub fn hot_gas_default() -> Self {
        Self {
            n_outer: 2,
            n_inner: 2,
            pressure_relaxation: 0.5,
            velocity_relaxation: 0.5,
        }
    }

    /// Water/steam-array settings: **2** outer correctors, 2 inner, both
    /// under-relaxations 0.3. See [`Self::hot_gas_default`] for the measurement
    /// behind the corrector count.
    ///
    /// The heavier relaxation reflects the phase change: the `(p, h)` flash's
    /// density swings by three orders of magnitude across the saturation dome.
    /// The cold side is nowhere near its Courant limit (`Co = 0.045` at the
    /// 0.0125 s substep against the hot side's 0.222), so it is the hot side
    /// that sets the count and this side simply matches it.
    pub fn water_steam_default() -> Self {
        Self {
            n_outer: 2,
            n_inner: 2,
            pressure_relaxation: 0.3,
            velocity_relaxation: 0.3,
        }
    }

    /// The same settings with a different outer-corrector count.
    #[allow(dead_code)] // part of the promotable public API; exercised by the corrector-sweep test
    pub fn with_outer(self, n_outer: usize) -> Self {
        Self { n_outer, ..self }
    }
}

/// Things that can go wrong building or driving the exchanger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamGeneratorError {
    /// A geometric dimension was zero or negative.
    NonPositiveGeometry,
    /// The tube outside diameter did not exceed the bore.
    TubeWallNotPositive,
    /// Fewer than 3 axial nodes were requested.
    TooFewNodes(usize),
    /// `substep` was zero, negative or non-finite.
    NonPositiveSubstep,
    /// One of the composed arrays refused to build or advance. Carries the
    /// upstream message, because the three arrays have three unrelated error
    /// types with no conversions between them.
    Array(String),
}

impl std::fmt::Display for SteamGeneratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonPositiveGeometry => {
                write!(f, "steam-generator geometry has a non-positive dimension")
            }
            Self::TubeWallNotPositive => {
                write!(f, "tube outer diameter must exceed the inner diameter")
            }
            Self::TooFewNodes(n) => {
                write!(f, "steam generator needs at least 3 axial nodes, got {n}")
            }
            Self::NonPositiveSubstep => write!(
                f,
                "steam-generator sub-timestep must be positive and finite"
            ),
            Self::Array(m) => write!(f, "composed array failed: {m}"),
        }
    }
}

impl std::error::Error for SteamGeneratorError {}

/// What one [`NodalisedCounterFlowSteamGenerator::advance_timestep`] produced.
///
/// All temperatures are in kelvin and all powers in watts, `uom`-typed. The node
/// vectors are all in **hot-side index order**: element 0 is the hot inlet end,
/// element `n-1` the hot outlet end. The cold stream's own inlet is therefore at
/// element `n-1`.
#[derive(Clone, Debug)]
#[allow(dead_code)] // part of the promotable public API; exercised by the tests
pub struct SteamGeneratorState {
    /// Heat rate leaving the hot fluid, `m_dot (h_in - h_out)` on the hot side.
    /// This is what the hot loop is cooled by.
    pub hot_side_duty: Power,
    /// Heat rate entering the water/steam, `m_dot (h_out - h_in)` on the cold
    /// side. This is what the steam cycle absorbs.
    ///
    /// It differs from [`Self::hot_side_duty`] by the rate of change of energy
    /// stored in the tube metal -- which is exactly the transient behaviour the
    /// metal exists to provide, not a bookkeeping error.
    pub cold_side_duty: Power,
    /// Hot-fluid outlet temperature.
    pub hot_outlet_temperature: ThermodynamicTemperature,
    /// Water/steam outlet temperature -- the live steam temperature.
    pub cold_outlet_temperature: ThermodynamicTemperature,
    /// Water/steam outlet specific enthalpy.
    pub cold_outlet_enthalpy: AvailableEnergy,
    /// Hot-fluid node temperatures, hot-inlet-first.
    pub hot_node_temperatures: Vec<ThermodynamicTemperature>,
    /// Tube-metal node temperatures, hot-inlet-first.
    pub metal_node_temperatures: Vec<ThermodynamicTemperature>,
    /// Water/steam node temperatures **re-indexed into hot-side order**, so
    /// `cold_node_temperatures[i]` is at the same physical station as
    /// `hot_node_temperatures[i]`.
    pub cold_node_temperatures: Vec<ThermodynamicTemperature>,
}

impl SteamGeneratorState {
    /// Local driving temperature difference at each station \[K\],
    /// `T_hot,i - T_cold,i`, hot-inlet-first.
    ///
    /// A negative entry is a **temperature cross** at that node. It is not
    /// forbidden by a clamp -- the lateral heat term simply reverses sign there,
    /// which is what the second law requires of a real exchanger whose cold
    /// stream has overtaken its hot stream.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn node_driving_differences_kelvin(&self) -> Vec<f64> {
        self.hot_node_temperatures
            .iter()
            .zip(self.cold_node_temperatures.iter())
            .map(|(h, c)| h.get::<kelvin>() - c.get::<kelvin>())
            .collect()
    }

    /// Largest **cross** over all nodes \[K\]: `max(0, max_i(T_cold,i - T_hot,i))`.
    ///
    /// Zero means no station anywhere in the exchanger has its cold stream
    /// hotter than its hot stream. This is the node-by-node no-cross measure, and
    /// it is strictly stronger than checking only the two outlets.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn worst_node_cross_kelvin(&self) -> f64 {
        self.node_driving_differences_kelvin()
            .into_iter()
            .fold(0.0_f64, |worst, d| worst.max(-d))
    }

    /// Driving temperature difference at the **hot inlet** end \[K\] -- the
    /// superheater pinch, and the number the isothermal-sink model got wrong.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn hot_end_driving_difference_kelvin(&self) -> f64 {
        self.node_driving_differences_kelvin()
            .first()
            .copied()
            .unwrap_or(0.0)
    }
}

/// A nodalised counter-flow once-through steam generator.
///
/// See the module docs for the arrangement, the no-cross argument and what is
/// real versus illustrative. Build with [`Self::new`], drive with
/// [`Self::advance_timestep`].
pub struct NodalisedCounterFlowSteamGenerator {
    hot: CompressibleFluidArray,
    metal: SolidColumn,
    cold: TampinesSteamArray,
    config: SteamGeneratorConfig,
    /// Per-node hot-side conductance = `hot_side_conductance / node_count`.
    hot_node_conductance: ThermalConductance,
    /// Per-node cold-side conductance = `cold_side_conductance / node_count`.
    cold_node_conductance: ThermalConductance,
    /// Most recent state, so a caller can read the exchanger without stepping it.
    last_state: SteamGeneratorState,
    /// Simulated time handed in by callers but not yet advanced through the
    /// arrays. See [`SteamGeneratorConfig::substep`].
    pending: Time,
}

impl NodalisedCounterFlowSteamGenerator {
    /// Build the exchanger and seed all three arrays at
    /// [`SteamGeneratorConfig::initial_temperature`] and their operating
    /// pressures.
    ///
    /// # Errors
    ///
    /// [`SteamGeneratorError`] if the geometry is not constructible, fewer than
    /// 3 nodes were asked for, the sub-timestep is not positive, or one of the
    /// composed arrays refuses to build.
    pub fn new(config: SteamGeneratorConfig) -> Result<Self, SteamGeneratorError> {
        config.geometry.validate()?;
        let n = config.node_count;
        if n < 3 {
            return Err(SteamGeneratorError::TooFewNodes(n));
        }
        let dt_sub = config.substep.get::<second>();
        if !(dt_sub > 0.0) || !dt_sub.is_finite() {
            return Err(SteamGeneratorError::NonPositiveSubstep);
        }

        let geometry = config.geometry;
        // Two linear profiles over physical stations -- one per stream -- and
        // the metal seeded between them, which is where a conductance network in
        // balance would put it. Seeding each node at its own neighbours'
        // temperature means the exchanger opens at approximately zero transfer,
        // with no cross and no start-up shock.
        let hot_seed = linear_station_profile(
            config.initial_hot_end_temperature,
            config.initial_cold_end_temperature,
            n,
        );
        let cold_station_seed = linear_station_profile(
            config.initial_cold_outlet_temperature,
            config.initial_cold_end_temperature,
            n,
        );
        let metal_seed: Vec<ThermodynamicTemperature> = hot_seed
            .iter()
            .zip(cold_station_seed.iter())
            .map(|(h, c)| {
                ThermodynamicTemperature::new::<kelvin>(
                    0.5 * (h.get::<kelvin>() + c.get::<kelvin>()),
                )
            })
            .collect();
        // The cold stream is indexed the other way round (counter-flow), so its
        // own cell order is the reverse of the station order.
        let cold_seed = reverse(&cold_station_seed);

        // --- Hot fluid: forward flow, inlet at cell 0. ---
        let mut hot_side_helium = CompressibleFluidArray::new(
            config.hot_fluid,
            geometry.shell_flow_length,
            geometry.shell_flow_area,
            n as i64,
            config.substep,
        )
        .map_err(|e| SteamGeneratorError::Array(format!("hot array: {e:?}")))?;
        for c in 0..n {
            hot_side_helium.p.internal[c] = config.hot_pressure.get::<pascal>();
        }
        hot_side_helium
            .set_temperature_vector(hot_seed.clone())
            .map_err(|e| SteamGeneratorError::Array(format!("hot seed: {e:?}")))?;
        hot_side_helium.correct_transport();
        // A gas exchanger is deeply subsonic, so the enthalpy convection scheme
        // -- not the Mach-blended dissipation -- is what keeps the enthalpy
        // bounded. VanLeer is the crate default; state it rather than inherit it.
        hot_side_helium.set_he_convection_scheme(
            outram_park_fork_coolprop::openfoam_algorithms::rhoPimpleFoam::EnergyConvectionScheme::VanLeer,
        );
        // IMPLICIT energy convection on the helium side.
        //
        // The hot side is what binds this exchanger's timestep: `Co_hot` is
        // 4.97x `Co_cold`, so the substep count exists for the helium array
        // alone. With convection explicit, that limit is hard -- the PIMPLE
        // outer-corrector loop is a Picard iteration whose contraction factor
        // *is* the cell Courant number, so above `Co = 1` it diverges however
        // many correctors are used (measured: 0.05 s panics at 4, 8 and 16;
        // 0.1 s at 8 and 32). Putting `div(phi h)` in the matrix removes that
        // ceiling; the limiter survives as a deferred correction.
        //
        // This costs accuracy at LOW Courant and the trade is deliberate.
        // Implicit upwind's numerical diffusion is `(u dx/2)(1 + Co)` against
        // the explicit `(u dx/2)(1 - Co)`, so implicit smears where explicit
        // sharpens. At this exchanger's operating point the two modes agree to
        // 0.033% of span, while dropping the limiter would cost 8.6% -- the
        // deferred correction carries roughly 300x more than the time
        // treatment does. Maintainer's call, 2026-08-13: "I prioritise
        // stability over all Courant numbers, the error isn't even that bad."
        //
        // Set explicitly rather than inherited: the crate default is
        // `Explicit`, deliberately, because most consumers run well below
        // `Co = 1` where explicit is the more accurate choice.
        hot_side_helium.set_he_balance_mode(
            outram_park_fork_coolprop::openfoam_algorithms::rhoPimpleFoam::EnergyBalanceMode::Implicit,
        );
        hot_side_helium.set_pimple_algorithm(
            config.hot_correctors.n_outer,
            config.hot_correctors.n_inner,
            ratio_of(config.hot_correctors.pressure_relaxation),
            ratio_of(config.hot_correctors.velocity_relaxation),
        );
        hot_side_helium.set_outlet_pressure(config.hot_pressure);

        // --- Tube metal. ---
        //
        // `new_block` rather than `new_cylindrical_shell`, with an *equivalent
        // rectangular section* carrying the aggregate annular metal area of the
        // whole bundle. Two reasons: the exchanger is a 90-tube bundle, not one
        // tube, so the aggregate cross-section is the physically meaningful
        // number; and `new_cylindrical_shell` builds its back/front boundary
        // control volumes from `SingleCVNode::new_cylinder(node_length,
        // INNER_diameter, ..)`, i.e. a SOLID rod of the bore, so those two CVs
        // carry a mass that does not even vanish as the wall thickness goes to
        // zero. That defect does not reach the solved temperatures (the matrix
        // uses `xs_area`, which the shell constructor gets right) but the
        // block constructor avoids relying on that. Filed as a bead.
        let metal_xs = geometry.metal_cross_section();
        let thickness = geometry.tube_outer_diameter - geometry.tube_inner_diameter;
        let width = Length::new::<meter>(metal_xs.get::<square_meter>() / thickness.get::<meter>());
        let mut metal = SolidColumn::new_block(
            geometry.tube_length,
            thickness,
            width,
            metal_seed[0],
            config.cold_pressure,
            config.metal,
            n - 2,
        );
        metal
            .set_temperature_vector(metal_seed.clone())
            .map_err(|e| SteamGeneratorError::Array(format!("metal seed: {e:?}")))?;

        // --- Water/steam: forward flow in its own frame, inlet at cell 0. ---
        let mut cold_side_feedwater_steam = TampinesSteamArray::new(
            geometry.tube_length,
            geometry.tube_flow_area(),
            n as i64,
            config.substep,
        )
        .map_err(|e| SteamGeneratorError::Array(format!("cold array: {e:?}")))?;
        for c in 0..n {
            cold_side_feedwater_steam.p.internal[c] = config.cold_pressure.get::<pascal>();
        }
        cold_side_feedwater_steam
            .set_temperature_vector(cold_seed.clone())
            .map_err(|e| SteamGeneratorError::Array(format!("cold seed: {e:?}")))?;
        cold_side_feedwater_steam.set_pimple_algorithm(
            config.cold_correctors.n_outer,
            config.cold_correctors.n_inner,
            ratio_of(config.cold_correctors.pressure_relaxation),
            ratio_of(config.cold_correctors.velocity_relaxation),
        );
        cold_side_feedwater_steam.set_outlet_pressure(config.cold_pressure);

        let hot_node_conductance = config.hot_side_conductance / (n as f64);
        let cold_node_conductance = config.cold_side_conductance / (n as f64);

        let last_state = SteamGeneratorState {
            hot_side_duty: Power::new::<watt>(0.0),
            cold_side_duty: Power::new::<watt>(0.0),
            hot_outlet_temperature: config.initial_cold_end_temperature,
            cold_outlet_temperature: config.initial_cold_outlet_temperature,
            cold_outlet_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(0.0),
            hot_node_temperatures: hot_seed,
            metal_node_temperatures: metal_seed,
            cold_node_temperatures: cold_station_seed,
        };

        Ok(Self {
            hot: hot_side_helium,
            metal,
            cold: cold_side_feedwater_steam,
            config,
            hot_node_conductance,
            cold_node_conductance,
            last_state,
            pending: Time::new::<second>(0.0),
        })
    }

    /// Number of axial nodes.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn node_count(&self) -> usize {
        self.config.node_count
    }

    /// The geometry this exchanger was built from.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn geometry(&self) -> SteamGeneratorGeometry {
        self.config.geometry
    }

    /// Tube-metal thermal capacity \[J/K\], derived from the geometry and the
    /// material database at the given temperature.
    #[allow(dead_code)] // part of the promotable public API; exercised by the tests
    pub fn metal_thermal_capacity(&self, temperature: ThermodynamicTemperature) -> HeatCapacity {
        self.config.geometry.metal_thermal_capacity(
            self.config.metal,
            temperature,
            self.config.cold_pressure,
        )
    }

    /// Series overall conductance `UA` \[W/K\]: the hot-side and cold-side
    /// conductances as two resistances in series through the metal.
    pub fn overall_conductance(&self) -> ThermalConductance {
        let g_h = self.config.hot_side_conductance.get::<watt_per_kelvin>();
        let g_c = self.config.cold_side_conductance.get::<watt_per_kelvin>();
        ThermalConductance::new::<watt_per_kelvin>(1.0 / (1.0 / g_h + 1.0 / g_c))
    }

    /// Metal thermal time constant \[s\], `tau = C_metal / UA_series`, evaluated
    /// at `temperature`.
    ///
    /// This is the lag a step change in duty is filtered through before it
    /// reaches the steam outlet. It is a *derived* diagnostic -- nothing in the
    /// solver reads it -- and it is what makes the secondary side genuinely
    /// transient.
    pub fn metal_time_constant(&self, temperature: ThermodynamicTemperature) -> Time {
        let c = self
            .metal_thermal_capacity(temperature)
            .get::<joule_per_kelvin>();
        let ua = self.overall_conductance().get::<watt_per_kelvin>();
        Time::new::<second>(c / ua)
    }

    /// The most recently computed state, without stepping.
    pub fn state(&self) -> &SteamGeneratorState {
        &self.last_state
    }

    /// Advective Courant numbers `Co = |u| dt / dx` on the two fluid arrays at
    /// a candidate array timestep, as `(hot_max, cold_max)`.
    ///
    /// **Measured, not assumed.** The velocities are read out of each array's
    /// own live `u` field, so this reports the Courant number the solver is
    /// actually running at, not one derived from a nominal density. `dx` is the
    /// uniform cell length each array was constructed with
    /// (`shell_flow_length / n` on the hot side, `tube_length / n` on the cold
    /// side).
    ///
    /// # Why this matters here
    ///
    /// Both arrays carry the enthalpy convection term **explicitly** -- their
    /// energy equation adds `fvc::div_limited(phi, he, limiter)` as a source
    /// rather than an `fvm::div` matrix contribution -- inside the PIMPLE outer
    /// corrector loop. That makes the outer loop a Picard iteration on an
    /// explicit convection source, whose contraction factor is the cell Courant
    /// number: it converges (and the scheme is then effectively implicit) while
    /// `Co < 1`, and diverges above it however many outer correctors are used.
    /// So the Courant number is the hard constraint on
    /// [`SteamGeneratorConfig::substep`], and more correctors do not relax it.
    /// [`tests::the_courant_number_bounds_the_array_substep`] records the
    /// measured values.
    ///
    /// The value uses [`get_fluid_courant_number_one_dimension`] from TUAS
    /// rather than re-deriving `u dt / dx`; that function returns `Err(value)`
    /// above 1, and both branches carry the same number, so the maximum is
    /// taken over either.
    #[allow(dead_code)] // part of the promotable public API; exercised by the Courant V&V test
    pub fn max_courant_numbers(&self, dt: Time) -> (f64, f64) {
        let n = self.config.node_count as f64;
        let hot_dx = self.config.geometry.shell_flow_length / n;
        let cold_dx = self.config.geometry.tube_length / n;
        let hot = max_courant_over_speeds(
            self.hot.u.internal.as_slice().iter().map(|v| v.mag()),
            dt,
            hot_dx,
        );
        let cold = max_courant_over_speeds(
            self.cold.u.internal.as_slice().iter().map(|v| v.mag()),
            dt,
            cold_dx,
        );
        (hot, cold)
    }

    /// Number of times either fluid array has clamped its enthalpy field
    /// against its own `[h_min, h_max]` bounds since construction.
    ///
    /// A nonzero count is the fingerprint of the odd-even (checkerboard)
    /// breakdown an over-large array timestep produces: the enthalpy field
    /// leaves the range spanned by its own boundary and initial data and the
    /// array limits it rather than handing the EOS an invalid state. Zero over
    /// a long run is therefore direct evidence the substep is inside the
    /// stability window, and is stronger than "it did not panic".
    ///
    /// Only the hot (CoolProp) array publishes a clamp counter; the cold
    /// (IF97) array does not, so this reports the hot side alone.
    #[allow(dead_code)] // part of the promotable public API; exercised by the V&V tests
    pub fn hot_enthalpy_clamp_events(&self) -> usize {
        self.hot.h_clamp_events
    }

    /// Advance the exchanger by `dt`.
    ///
    /// The exchanger runs on **its own fixed clock**: `dt` is accumulated and
    /// the arrays are advanced in whole [`SteamGeneratorConfig::substep`]s, with
    /// the remainder carried over. A call that does not complete a whole substep
    /// returns the previous state unchanged -- a zero-order hold. Read that
    /// field's documentation before changing anything here; the arrays are
    /// unstable *below* the substep as well as above it.
    ///
    /// The hot stream enters at `hot_inlet_temperature` carrying
    /// `hot_mass_flow`; the water/steam enters at `cold_inlet_enthalpy` carrying
    /// `cold_mass_flow`. Enthalpy rather than temperature on the cold side
    /// because the stream boils: a `(p, T)` flash is not invertible inside the
    /// saturation dome, while `(p, h)` is.
    ///
    /// The order within each substep -- which follows TUAS's
    /// `SimpleShellAndTubeHeatExchanger::lateral_and_miscellaneous_connections`:
    ///
    /// 1. impose both inlets (mass flow and junction enthalpy) and both outlet
    ///    pressures;
    /// 2. **snapshot all three temperature vectors before any linking**, so
    ///    every lateral link sees the same old-time state and the coupling is
    ///    symmetric rather than half-implicit;
    /// 3. register the four reciprocal lateral links -- hot<->metal with
    ///    `UA_hot/n`, metal<->cold with `UA_cold/n`, the cold vectors reversed
    ///    by the counter-flow index map;
    /// 4. advance all three arrays.
    ///
    /// Registrations are consumed and cleared by each array's own advance, so
    /// they are re-made every substep.
    ///
    /// # Errors
    ///
    /// [`SteamGeneratorError::Array`] if any composed array refuses.
    pub fn advance_timestep(
        &mut self,
        dt: Time,
        hot_inlet_temperature: ThermodynamicTemperature,
        hot_mass_flow: MassRate,
        cold_inlet_enthalpy: AvailableEnergy,
        cold_mass_flow: MassRate,
    ) -> Result<SteamGeneratorState, SteamGeneratorError> {
        let n = self.config.node_count;

        // Accumulate onto the exchanger's own clock. Below one whole substep
        // there is nothing to do: hand back the last state unchanged, which is a
        // zero-order hold on a sub-model whose fastest mode is hundreds of
        // milliseconds. See `SteamGeneratorConfig::substep` for why this is a
        // fixed step rather than a subdivision.
        let dt_sub = self.config.substep;
        self.pending += Time::new::<second>(dt.get::<second>().max(0.0));
        let substeps = (self.pending.get::<second>() / dt_sub.get::<second>()).floor() as usize;
        if substeps == 0 {
            return Ok(self.last_state.clone());
        }
        self.pending -= dt_sub * (substeps as f64);

        let hot_inlet_enthalpy = hot_fluid_enthalpy(
            self.config.hot_fluid,
            hot_inlet_temperature,
            self.config.hot_pressure,
        );

        for _ in 0..substeps {
            // 1. Boundary conditions. Mass-flow inlets, not velocity inlets: the
            //    velocity route derives a velocity from an assumed density and
            //    was measured up to +100% wrong, opening a 1.33 MW imbalance
            //    across a converged exchanger.
            self.hot.set_inlet_mass_flowrate(hot_mass_flow);
            self.hot.set_inlet_enthalpy(hot_inlet_enthalpy);
            self.hot.set_outlet_pressure(self.config.hot_pressure);

            self.cold.set_inlet_mass_flowrate(cold_mass_flow);
            self.cold.set_inlet_enthalpy(cold_inlet_enthalpy);
            self.cold.set_outlet_pressure(self.config.cold_pressure);

            // 2. Snapshot every temperature vector BEFORE any linking.
            let hot_temps = self.hot.get_temperature_vector();
            let metal_temps = self
                .metal
                .get_temperature_vector()
                .map_err(|e| SteamGeneratorError::Array(format!("metal temps: {e:?}")))?;
            let cold_temps = self.cold.get_temperature_vector();

            // The counter-flow index map: cold cell j sits at hot station n-1-j.
            let cold_temps_in_hot_order = reverse(&cold_temps);
            let metal_temps_in_cold_order = reverse(&metal_temps);

            // 3. Reciprocal lateral links.
            self.hot
                .lateral_link_new_temperature_vector_avg_conductance(
                    self.hot_node_conductance,
                    metal_temps.clone(),
                )
                .map_err(|e| SteamGeneratorError::Array(format!("hot<-metal: {e:?}")))?;
            self.metal
                .lateral_link_new_temperature_vector_avg_conductance(
                    self.hot_node_conductance,
                    hot_temps.clone(),
                )
                .map_err(|e| SteamGeneratorError::Array(format!("metal<-hot: {e:?}")))?;
            self.metal
                .lateral_link_new_temperature_vector_avg_conductance(
                    self.cold_node_conductance,
                    cold_temps_in_hot_order,
                )
                .map_err(|e| SteamGeneratorError::Array(format!("metal<-cold: {e:?}")))?;
            self.cold
                .lateral_link_new_temperature_vector_avg_conductance(
                    self.cold_node_conductance,
                    metal_temps_in_cold_order,
                )
                .map_err(|e| SteamGeneratorError::Array(format!("cold<-metal: {e:?}")))?;

            // 4. Advance.
            self.hot
                .advance_timestep(dt_sub)
                .map_err(|e| SteamGeneratorError::Array(format!("hot advance: {e:?}")))?;
            self.metal
                .advance_timestep(dt_sub)
                .map_err(|e| SteamGeneratorError::Array(format!("metal advance: {e:?}")))?;
            self.cold
                .advance_timestep(dt_sub)
                .map_err(|e| SteamGeneratorError::Array(format!("cold advance: {e:?}")))?;
        }

        // --- Post-processing: the two stream energy balances. ---
        let hot_out_h = self.hot.get_outlet_enthalpy();
        let hot_side_duty = Power::new::<watt>(
            hot_mass_flow.get::<kilogram_per_second>()
                * (hot_inlet_enthalpy.get::<joule_per_kilogram>()
                    - hot_out_h.get::<joule_per_kilogram>()),
        );
        let cold_out_h = self.cold.get_outlet_enthalpy();
        let cold_side_duty = Power::new::<watt>(
            cold_mass_flow.get::<kilogram_per_second>()
                * (cold_out_h.get::<joule_per_kilogram>()
                    - cold_inlet_enthalpy.get::<joule_per_kilogram>()),
        );

        let hot_node_temperatures = self.hot.get_temperature_vector();
        let metal_node_temperatures = self
            .metal
            .get_temperature_vector()
            .map_err(|e| SteamGeneratorError::Array(format!("metal temps: {e:?}")))?;
        let cold_node_temperatures = reverse(&self.cold.get_temperature_vector());
        debug_assert_eq!(hot_node_temperatures.len(), n);

        let state = SteamGeneratorState {
            hot_side_duty,
            cold_side_duty,
            hot_outlet_temperature: self.hot.get_outlet_temperature(),
            cold_outlet_temperature: self.cold.get_outlet_temperature(),
            cold_outlet_enthalpy: cold_out_h,
            hot_node_temperatures,
            metal_node_temperatures,
            cold_node_temperatures,
        };
        self.last_state = state.clone();
        Ok(state)
    }
}

/// Linear temperature profile over `n` physical stations, `start` at station 0
/// and `end` at station `n-1`.
fn linear_station_profile(
    start: ThermodynamicTemperature,
    end: ThermodynamicTemperature,
    n: usize,
) -> Vec<ThermodynamicTemperature> {
    let a = start.get::<kelvin>();
    let b = end.get::<kelvin>();
    (0..n)
        .map(|i| {
            let f = if n > 1 {
                i as f64 / (n - 1) as f64
            } else {
                0.0
            };
            ThermodynamicTemperature::new::<kelvin>(a + f * (b - a))
        })
        .collect()
}

/// Largest one-dimensional advective Courant number over a sequence of cell
/// speeds \[m/s\], `max_c |u_c| dt / dx`.
///
/// Takes bare speeds rather than the arrays' vector cells because the two
/// arrays' vector types come from two different crates
/// (`outram-park-fork-coolprop` and `tampines-steam-tables`) whose primitive
/// modules are private, so neither type can be named here; each caller maps its
/// own cells through the inherent `mag()` first.
///
/// Delegates the arithmetic to TUAS's
/// [`get_fluid_courant_number_one_dimension`], which returns `Err(value)` once
/// the value exceeds 1; both branches carry the same number and the caller
/// wants the measurement either way, so both are unwrapped to the value.
fn max_courant_over_speeds(speeds: impl Iterator<Item = f64>, dt: Time, dx: Length) -> f64 {
    speeds
        .map(|u| {
            let speed = Velocity::new::<meter_per_second>(u.abs());
            match get_fluid_courant_number_one_dimension(speed, dt, dx) {
                Ok(c) => c,
                Err(c) => c,
            }
        })
        .fold(0.0_f64, f64::max)
}

/// Reverse a node vector -- the counter-flow index map. Its own inverse.
fn reverse<T: Copy>(v: &[T]) -> Vec<T> {
    v.iter().rev().copied().collect()
}

/// `uom` dimensionless ratio from a bare `f64`, for solver under-relaxation.
fn ratio_of(x: f64) -> uom::si::f64::Ratio {
    uom::si::f64::Ratio::new::<ratio>(x)
}

/// Specific enthalpy of the hot fluid at `(T, p)` from the CoolProp-derived
/// Helmholtz EOS.
///
/// Returns 0 J/kg if the flash fails to converge. That is a *reference-state*
/// value, not a fabricated property: the hot-side duty is computed from an
/// enthalpy **difference** across the exchanger, so a consistent offset cancels;
/// a failed flash on one end only would show up as an obviously wrong duty
/// rather than as a silently plausible one. Neither fluid the callers pass
/// (helium at 3 MPa, molten salt) is anywhere near a flash failure at operating
/// conditions.
fn hot_fluid_enthalpy(
    fluid: CoolPropFluid,
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
) -> AvailableEnergy {
    match outram_park_fork_coolprop::state_pt(
        fluid,
        temperature.get::<kelvin>(),
        pressure.get::<pascal>(),
    ) {
        Ok(s) if s.enthalpy.is_finite() => AvailableEnergy::new::<joule_per_kilogram>(s.enthalpy),
        _ => AvailableEnergy::new::<joule_per_kilogram>(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_density::kilogram_per_cubic_meter;

    /// The HTR-10 exchanger roughly as the primary loop builds it, for tests
    /// that do not need to reach back into that module.
    ///
    /// **It is NOT identical to the plant's configuration, and nothing checks
    /// that it is.** This helper's doc comment used to claim it was "kept in
    /// step with `super::super::primary_loop::steam_generator_config` by
    /// `the_test_configuration_matches_the_plant`" -- a test of that name has
    /// never existed, and the two configurations have since diverged: the plant
    /// uses `SolidMaterial::SteelSS304LHighTemp` (Kim, ANL-75-55, 300-1700 K)
    /// while this still uses `SolidMaterial::SteelSS304L` (Zou et al.,
    /// 250-1000 K). Measured 2026-08-13, that shifts the settled steam outlet
    /// by about 11 K (647.7 K here against 658.4 K with the plant's material)
    /// because the high-temperature correlation carries a
    /// temperature-dependent density where the other is a flat 8030 kg/m^3.
    ///
    /// Every figure recorded in this module's tests is therefore a figure for
    /// **this** configuration, not for the plant. Reconciling the two will move
    /// those recorded numbers and is deliberately left as separate work.
    fn htr10() -> SteamGeneratorConfig {
        let ua = 4.26e4_f64;
        let hot_fraction = 0.75_f64;
        SteamGeneratorConfig {
            geometry: SteamGeneratorGeometry::htr10_illustrative(),
            hot_fluid: CoolPropFluid::Helium,
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: Pressure::new::<pascal>(4.0e6),
            metal: SolidMaterial::SteelSS304L,
            hot_side_conductance: ThermalConductance::new::<watt_per_kelvin>(ua / hot_fraction),
            cold_side_conductance: ThermalConductance::new::<watt_per_kelvin>(
                ua / (1.0 - hot_fraction),
            ),
            node_count: 8,
            // Derived from the global plant timestep, exactly as the plant's
            // own configuration derives it -- see
            // `super::super::steam_generator_substep_seconds`.
            substep: Time::new::<second>(crate::physics::steam_generator_substep_seconds()),
            initial_hot_end_temperature: ThermodynamicTemperature::new::<kelvin>(973.15),
            initial_cold_end_temperature: ThermodynamicTemperature::new::<kelvin>(320.0),
            initial_cold_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(713.15),
            hot_correctors: PimpleCorrectors::hot_gas_default(),
            cold_correctors: PimpleCorrectors::water_steam_default(),
        }
    }

    fn hot_inlet() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(973.15)
    }
    fn hot_flow() -> MassRate {
        MassRate::new::<kilogram_per_second>(4.3)
    }
    fn feedwater() -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(168.73e3)
    }
    fn cold_flow() -> MassRate {
        MassRate::new::<kilogram_per_second>(3.19)
    }
    /// The plant timestep the exchanger's callers drive it at.
    fn dt() -> Time {
        crate::physics::plant_timestep()
    }

    /// V&V: **the Courant number is what bounds the array substep, and outer
    /// correctors do not lift that bound.**
    ///
    /// # Why this test exists
    ///
    /// The exchanger is ~96% of `htgr_sim_v1`'s compute (measured 2026-08-13),
    /// and its cost is almost exactly linear in
    /// `outer_correctors / substep`. So the only way to make this simulator
    /// faster is a larger substep, and the standing question is whether more
    /// PIMPLE outer correctors buy one -- which is what an outer corrector does
    /// for an implicitly-discretised convection term. Here it does not, and the
    /// reason is structural: both arrays add their enthalpy convection as an
    /// **explicit** `fvc::div_limited` source *inside* the outer loop, so the
    /// loop is a Picard iteration on that source whose contraction factor is
    /// the cell Courant number. Below `Co = 1` correctors converge it (and the
    /// scheme is effectively implicit); at and above `Co = 1` no count
    /// converges. In practice the bound bites well below 1, because the
    /// explicit TVD limiter itself needs roughly `Co <= 0.5`.
    ///
    /// # Methodology
    ///
    /// The exchanger is settled for 200 s of plant time at the design point
    /// (helium 973.15 K at 4.3 kg/s, feedwater 168.73 kJ/kg at 3.19 kg/s), then
    /// [`NodalisedCounterFlowSteamGenerator::max_courant_numbers`] is evaluated
    /// from the arrays' own live velocity fields at a series of candidate
    /// substeps. Separately, exchangers are built at 0.05 s and 0.1 s substeps
    /// with 4, 8, 16 and 32 outer correctors and driven at the design point;
    /// each is expected to fail rather than to be rescued by the correctors.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// Courant numbers at the settled design point, from the live `u` fields
    /// (hot cells 0.625 m, cold cells 4.25 m):
    ///
    /// | Array substep | `Co_hot` | `Co_cold` | Outcome |
    /// |---|---|---|---|
    /// | 0.00625 s | 0.111 | 0.022 | stable |
    /// | **0.0125 s (shipped)** | **0.222** | **0.045** | **stable, converged** |
    /// | 0.025 s | 0.444 | 0.089 | stable, duty 1.4% high |
    /// | 0.05 s | 0.888 | 0.178 | **fails** |
    /// | 0.1 s | 1.776 | 0.357 | **fails** |
    ///
    /// Corrector sweep at the failing substeps -- every one failed:
    ///
    /// | Substep | outer correctors tried | Result |
    /// |---|---|---|
    /// | 0.05 s | 4, 8, 16 | panicked in all three |
    /// | 0.1 s | 8, 32 | panicked in both |
    ///
    /// # Interpretation
    ///
    /// The hot helium side is the binding constraint -- it is 5x the cold
    /// side's Courant number, because the shell is 5 m of 11 m/s gas against
    /// 34 m of tube carrying water that is liquid for most of its length. The
    /// substep therefore cannot be raised past ~0.025 s at this nodalisation,
    /// **whatever the corrector count**, and the plant timestep cannot be
    /// handed straight through to the arrays at all. Making the exchanger
    /// cheaper needs a coarser mesh, a cheaper equation of state, or an
    /// implicit convection operator -- not more correctors.
    ///
    /// This test asserts the *ordering*, not the exact figures, since the
    /// velocities depend on the settled state.
    ///
    /// **Results (re-measured 2026-08-13, helium side implicit, sub-steps 8 -> 2).**
    ///
    /// | substep (s) | `Co_hot` | `Co_cold` | hot/cold |
    /// |---|---|---|---|
    /// | 0.00625 | 0.1108 | 0.0221 | 5.02 |
    /// | 0.01250 | 0.2215 | 0.0442 | 5.02 |
    /// | 0.02500 | 0.4431 | 0.0883 | 5.02 |
    /// | **0.05000 (shipped)** | **0.8861** | 0.1766 | 5.02 |
    /// | 0.10000 (plant step) | 1.7722 | 0.3533 | 5.02 |
    ///
    /// The hot side binds throughout at a constant 5.02x the cold, and the
    /// shipped substep now runs at `Co_hot` = 0.886 -- above the 0.5 window the
    /// old explicit limiter needed, and stable, with the clamp counter at zero.
    /// Interpretation: the sub-step reduction is spending exactly the margin
    /// that implicit convection freed, and nothing more.
    #[test]
    fn the_courant_number_bounds_the_array_substep() {
        let sg = settled(200.0);
        let mut previous = 0.0_f64;
        for substep_s in [0.00625_f64, 0.0125, 0.025, 0.05, 0.1] {
            let (hot, cold) = sg.max_courant_numbers(Time::new::<second>(substep_s));
            println!(
                "substep {substep_s:>8.5} s: Co_hot = {hot:.4}, Co_cold = {cold:.4}, \
                 ratio hot/cold = {:.2}",
                hot / cold
            );
            assert!(
                hot > cold,
                "the hot side must be the binding Courant constraint"
            );
            assert!(hot > previous, "Courant number must grow with the substep");
            previous = hot;
        }

        // The shipped substep no longer has to sit inside the explicit-TVD-safe
        // window, and deliberately does not.
        //
        // This previously asserted `Co_hot < 0.5`, which encoded the *explicit*
        // limiter's safe window. The helium side now runs
        // `EnergyBalanceMode::Implicit`, so that window does not apply, and the
        // sub-step count was cut 8 -> 2 precisely to spend the margin the
        // implicit treatment freed. At the shipped 0.05 s substep `Co_hot` is
        // about 0.89 -- above the old bound and entirely expected.
        //
        // What is still asserted is the property that actually matters: the
        // clamp counter at the end of this test stays at zero, i.e. the array
        // never needed its enthalpy field rescued. That is a direct stability
        // statement rather than a proxy.
        let (shipped_hot, _) = sg.max_courant_numbers(Time::new::<second>(
            super::super::steam_generator_substep_seconds(),
        ));
        println!(
            "shipped substep {:.5} s: Co_hot = {shipped_hot:.4} \
             (implicit convection -- no explicit stability bound applies)",
            super::super::steam_generator_substep_seconds()
        );

        // And the plant timestep must NOT be handed straight to the arrays.
        let (plant_hot, _) = sg.max_courant_numbers(crate::physics::plant_timestep());
        assert!(
            plant_hot > 1.0,
            "the plant timestep gives Co_hot = {plant_hot}; if this has dropped below 1 the \
             exchanger may no longer need to be multi-rate, which is worth revisiting"
        );
        assert_eq!(sg.hot_enthalpy_clamp_events(), 0);
    }

    /// V&V: **the corrector count trades against wall clock, not against
    /// accuracy** -- at this exchanger's Courant number.
    ///
    /// # Methodology
    ///
    /// Exchangers are built at the shipped 0.0125 s substep with 1, 2 and 4
    /// outer correctors on both fluid arrays, settled for 120 s of plant time
    /// at the design point, then run 20 s more. The settled hot-side duty, the
    /// two-stream closure and the steam outlet are compared. Pass criterion:
    /// the duty must agree across corrector counts to better than 0.1%, which
    /// is the claim the shipped count rests on.
    ///
    /// # Results
    ///
    /// **This test, measured 2026-08-13** (release, `htr10()` configuration,
    /// which uses `SolidMaterial::SteelSS304L`):
    ///
    /// | Substep | outer correctors | `Q_hot` | closure | steam |
    /// |---|---|---|---|---|
    /// | 0.0125 s | 1 | 9.6900 MW | +1.742% | 647.68 K |
    /// | **0.0125 s** | **2 (shipped)** | **9.6903 MW** | **+1.743%** | **647.71 K** |
    /// | 0.0125 s | 4 (previous default) | 9.6904 MW | +1.749% | 647.64 K |
    ///
    /// **A wider sweep measured the same day**, on the *plant's* configuration
    /// (`SolidMaterial::SteelSS304LHighTemp`, which is what
    /// `super::super::primary_loop::steam_generator_config` builds -- see the
    /// note on `htr10()` about that divergence), settling 120 s and timing 20 s
    /// more:
    ///
    /// | Substep | outer correctors | `Co_hot` | `Q_hot` | closure | steam | compute per second of plant time |
    /// |---|---|---|---|---|---|---|
    /// | 0.00625 s | 4 | 0.111 | 9.6851 MW (reference) | +0.817% | 658.75 K | 4.088 |
    /// | 0.0125 s | 1 | 0.222 | 9.6854 MW | +0.815% | 658.80 K | 0.511 |
    /// | **0.0125 s** | **2 (shipped)** | **0.222** | **9.6854 MW** | **+0.843%** | **658.44 K** | **0.987** |
    /// | 0.0125 s | 3 | 0.222 | 9.6854 MW | +0.844% | 658.43 K | 1.478 |
    /// | 0.025 s | 2 | 0.444 | 9.8244 MW | +2.239% | 658.52 K | 0.496 |
    /// | 0.025 s | 4 | 0.444 | 9.8244 MW | +2.231% | 658.61 K | 0.983 |
    /// | 0.025 s | 6 | 0.444 | 9.8244 MW | +2.230% | 658.63 K | 1.466 |
    /// | 0.05 s | 4, 8, 16 | 0.888 | panicked | | | |
    /// | 0.1 s | 8, 32 | 1.776 | panicked | | | |
    ///
    /// Against the 0.00625 s reference the shipped 0.0125 s substep is
    /// **+0.003%** on duty and 0.025 s is **+1.44%**.
    ///
    /// # Interpretation
    ///
    /// Two things, and the second is the useful one:
    ///
    /// 1. **Cost is linear in `n_outer / substep`** -- 0.511 at (1, 0.0125),
    ///    0.987 at (2, 0.0125), 1.478 at (3, 0.0125), 4.088 at (4, 0.00625),
    ///    and 0.496 at (2, 0.025) where the ratio matches (1, 0.0125). Each
    ///    outer corrector is one more
    ///    equation-of-state pass over every cell, and the EOS flashes are where
    ///    this model spends its time.
    /// 2. **The duty does not move with the corrector count at all** -- 9.6854
    ///    MW at 1, 2 and 3 correctors; 9.8244 MW at 2, 4 and 6 correctors on
    ///    the 0.025 s substep. The 1.44% duty error at 0.025 s is therefore
    ///    **not**
    ///    something correctors fix, because it does not come from the arrays'
    ///    internal PIMPLE loop: it comes from the **exchanger-level Lie split**,
    ///    the lateral conductances being registered once per substep against
    ///    the neighbours' previous-substep temperatures. Removing that would
    ///    need a corrector loop *around* [`NodalisedCounterFlowSteamGenerator::advance_timestep`]'s
    ///    link-and-advance sequence, with all three arrays rolled back each
    ///    iteration -- which is not implemented here.
    ///
    /// So the shipped configuration takes the 2x that costs nothing (4 -> 2
    /// correctors) and declines the further 2x that would cost 1.4% of duty
    /// (0.0125 -> 0.025 s substep).
    #[test]
    fn the_corrector_substep_trade_is_measured() {
        let mut duties = Vec::new();
        for n_outer in [1_usize, 2, 4] {
            let mut c = htr10();
            c.hot_correctors = c.hot_correctors.with_outer(n_outer);
            c.cold_correctors = c.cold_correctors.with_outer(n_outer);
            let mut sg = NodalisedCounterFlowSteamGenerator::new(c).unwrap();
            for _ in 0..steps_for(120.0) {
                sg.advance_timestep(dt(), hot_inlet(), hot_flow(), feedwater(), cold_flow())
                    .unwrap();
            }
            let st = sg.state();
            let q = st.hot_side_duty.get::<watt>() / 1.0e6;
            let q_cold = st.cold_side_duty.get::<watt>() / 1.0e6;
            println!(
                "{n_outer} outer corrector(s): Q_hot = {q:.4} MW, closure = {:+.3}%, \
                 steam = {:.3} K, clamp events = {}",
                100.0 * (q - q_cold) / q,
                st.cold_outlet_temperature.get::<kelvin>(),
                sg.hot_enthalpy_clamp_events(),
            );
            assert_eq!(
                sg.hot_enthalpy_clamp_events(),
                0,
                "{n_outer} outer correctors left the enthalpy field clamping"
            );
            duties.push(q);
        }
        let q0 = duties[0];
        for (i, q) in duties.iter().enumerate() {
            assert!(
                ((q - q0) / q0).abs() < 1.0e-3,
                "duty moved {:+.4}% between 1 and {} outer correctors; the shipped count \
                 assumes it does not",
                100.0 * (q - q0) / q0,
                [1, 2, 4][i]
            );
        }
    }

    /// Drive the exchanger to a settled state at fixed boundary conditions for
    /// `plant_seconds` of **simulated time**.
    ///
    /// Takes simulated seconds rather than a step count so that changing
    /// [`crate::physics::PLANT_TIMESTEP_S`] does not silently change how long
    /// every test in this module settles for. It used to take a step count, and
    /// when the plant timestep went from 0.05 s to 0.1 s on 2026-08-13 that
    /// would have doubled every settling window at once.
    fn settled(plant_seconds: f64) -> NodalisedCounterFlowSteamGenerator {
        let mut sg = NodalisedCounterFlowSteamGenerator::new(htr10()).unwrap();
        for _ in 0..steps_for(plant_seconds) {
            sg.advance_timestep(dt(), hot_inlet(), hot_flow(), feedwater(), cold_flow())
                .unwrap();
        }
        sg
    }

    /// Number of plant timesteps in `plant_seconds` of simulated time.
    fn steps_for(plant_seconds: f64) -> usize {
        (plant_seconds / crate::physics::PLANT_TIMESTEP_S).round() as usize
    }

    /// The counter-flow index map must be an involution: applying it twice must
    /// return the original ordering.
    ///
    /// This is the whole of the counter-flow arrangement -- get it wrong and the
    /// exchanger silently becomes co-current, which is a different (and worse)
    /// machine that still runs and still produces plausible-looking numbers.
    #[test]
    fn counter_flow_index_map_is_its_own_inverse() {
        let v: Vec<usize> = (0..8).collect();
        assert_eq!(reverse(&reverse(&v)), v);
        assert_eq!(reverse(&v), vec![7, 6, 5, 4, 3, 2, 1, 0]);
        // Odd lengths must keep their fixed centre point.
        let odd: Vec<usize> = (0..7).collect();
        assert_eq!(reverse(&odd)[3], 3);
    }

    /// The station seed must be monotone and hit both stated end values exactly.
    #[test]
    fn the_station_seed_spans_its_two_end_temperatures() {
        let a = ThermodynamicTemperature::new::<kelvin>(973.15);
        let b = ThermodynamicTemperature::new::<kelvin>(320.0);
        let p = linear_station_profile(a, b, 8);
        assert_eq!(p.len(), 8);
        assert!((p[0].get::<kelvin>() - 973.15).abs() < 1e-9);
        assert!((p[7].get::<kelvin>() - 320.0).abs() < 1e-9);
        for w in p.windows(2) {
            assert!(w[1].get::<kelvin>() < w[0].get::<kelvin>());
        }
    }

    /// V&V: the tube-metal thermal mass is **derived from the tube geometry and
    /// the material database**, not typed in.
    ///
    /// # Methodology
    ///
    /// The aggregate metal cross-section of the bundle is
    /// `N pi/4 (d_o^2 - d_i^2)`; its volume is that times the developed tube
    /// length; its mass is that times the density
    /// `SolidMaterial::SteelSS304L` reports through TUAS's
    /// `try_get_rho`. Each step is recomputed here from the published /
    /// illustrative dimensions and checked against the geometry helper, so a
    /// future edit to either the dimensions or the helper is caught.
    ///
    /// The comparator is the **3069 kg** an earlier (since-deleted) attempt at
    /// this exchanger measured for the same unit. It is a sanity target, not a
    /// reference: that attempt's tube diameters and tube count were not
    /// recorded, so agreement to a few percent is all that can be asked of it.
    /// Pass criterion: within 10% of 3069 kg.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Tubes | 90 (30 published modules x 3 invented tubes) |
    /// | Developed length per tube | 34 m (published) |
    /// | Outside / bore diameter | 19 mm / 14 mm (invented) |
    /// | Aggregate metal cross-section | 1.166316e-2 m^2 |
    /// | Metal volume | 0.39655 m^3 |
    /// | `SteelSS304L` density | 8030 kg/m^3 (temperature-independent) |
    /// | **Metal mass** | **3184.3 kg** |
    /// | vs the 3069 kg earlier figure | **+3.8%** |
    /// | `c_p` at 600 K | 513.9 J/(kg K) |
    /// | **Thermal capacity** | **1.6366e6 J/K** |
    /// | Outside heat-transfer area | 182.65 m^2 |
    /// | Bore heat-transfer area | 134.59 m^2 |
    ///
    /// # Interpretation
    ///
    /// The mass is a consequence of the geometry, so changing a tube diameter
    /// moves the metal time constant without anyone editing a number that says
    /// "metal mass". The 3.8% agreement with the earlier independent attempt
    /// suggests both picked similar tubing, which is weak corroboration and
    /// nothing more -- **the tube diameters and the tubes-per-module count are
    /// invented**, because no source in
    /// `docs/reactor-scoping/htr10-plant-data.md` carries them.
    ///
    /// Note the outside area of 182.65 m^2 is **3.3x** the 56 m^2 that sheet
    /// records from Wu, Lin & Zhong (2002). That figure is flagged there as
    /// arithmetically implausible (it implies a 179 kW/m^2 average heat flux on
    /// a gas-heated surface) with the instruction not to size a simulator from
    /// it, so the disagreement is recorded rather than reconciled.
    #[test]
    fn metal_mass_is_derived_from_tube_geometry() {
        let g = SteamGeneratorGeometry::htr10_illustrative();
        let t = ThermodynamicTemperature::new::<kelvin>(600.0);
        let p = Pressure::new::<pascal>(4.0e6);

        // Recompute the chain independently of the helpers.
        let d_o = g.tube_outer_diameter.get::<meter>();
        let d_i = g.tube_inner_diameter.get::<meter>();
        let xs = g.tube_count * std::f64::consts::PI * 0.25 * (d_o * d_o - d_i * d_i);
        let volume = xs * g.tube_length.get::<meter>();
        let rho = try_get_rho(Material::Solid(SolidMaterial::SteelSS304L), t, p)
            .unwrap()
            .get::<kilogram_per_cubic_meter>();
        let mass = rho * volume;

        let cp = try_get_cp(Material::Solid(SolidMaterial::SteelSS304L), t, p)
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        println!(
            "tube metal: xs = {xs:.6e} m2, volume = {volume:.5} m3, rho = {rho:.1} kg/m3, \
             mass = {mass:.1} kg (earlier independent figure 3069 kg, {:+.1}%), \
             cp(600 K) = {cp:.1} J/(kg K), C = {:.4e} J/K; \
             A_out = {:.2} m2, A_in = {:.2} m2",
            100.0 * (mass - 3069.0) / 3069.0,
            mass * cp,
            g.outer_heat_transfer_area().get::<square_meter>(),
            g.inner_heat_transfer_area().get::<square_meter>(),
        );

        assert!(
            (g.metal_cross_section().get::<square_meter>() - xs).abs() / xs < 1e-12,
            "metal_cross_section disagrees with the geometry"
        );
        assert!(
            (g.metal_mass(SolidMaterial::SteelSS304L, t, p)
                .get::<kilogram>()
                - mass)
                .abs()
                / mass
                < 1e-12,
            "metal_mass disagrees with rho * V"
        );
        assert!(
            (g.metal_thermal_capacity(SolidMaterial::SteelSS304L, t, p)
                .get::<joule_per_kelvin>()
                - mass * cp)
                .abs()
                / (mass * cp)
                < 1e-12,
            "metal_thermal_capacity disagrees with m * cp"
        );
        assert!(
            (mass - 3069.0).abs() / 3069.0 < 0.10,
            "metal mass {mass} kg is more than 10% from the 3069 kg sanity target"
        );
    }

    /// V&V: **the energy balance closes across the exchanger** at steady state.
    ///
    /// # Methodology
    ///
    /// With fixed boundary conditions -- 973.15 K helium at 4.3 kg/s into the
    /// shell, 168.73 kJ/kg water at 3.19 kg/s into the tubes -- the exchanger is
    /// marched 4000 steps of 0.05 s (200 s, i.e. more than five metal time
    /// constants). At steady state the tube metal stores nothing on average, so
    /// the heat the helium gives up must equal the heat the water takes:
    ///
    /// ```text
    /// Q_hot  = m_hot  * (h_hot_in  - h_hot_out)     [shell stream]
    /// Q_cold = m_cold * (h_cold_out - h_cold_in)    [tube stream]
    /// ```
    ///
    /// Both are the streams' **own** enthalpy balances across their own
    /// terminals -- neither is computed from the lateral conductances, so this is
    /// a real closure check on the coupling and on the upwind advection
    /// terminals, not an identity. Pass criterion: closure within 2%.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// At the 0.0125 s sub-timestep the model ships with:
    /// `Q_hot = 9.671903 MW`, `Q_cold = 9.638549 MW`, closure **+0.3449%**.
    ///
    /// The residual is **not round-off, and it converges**. The three arrays are
    /// coupled explicitly (Lie-split) -- each one's lateral conductance is
    /// evaluated against its neighbours' previous sub-timestep temperatures, and
    /// the two fluid arrays treat that source differently from the implicit
    /// solid -- so the closure is `O(dt)`. Sweeping the sub-timestep at the same
    /// 200 s design-point run:
    ///
    /// | Sub-timestep | `Q_hot` | Closure |
    /// |---|---|---|
    /// | 0.025 s | 9.8111 MW | +1.72% |
    /// | **0.0125 s** | **9.6719 MW** | **+0.34%** |
    /// | 0.00625 s | 9.6718 MW | +0.35% |
    ///
    /// Both the duty and the closure have converged by 0.0125 s; halving again
    /// buys nothing. The remaining 0.34% is the floor of this splitting, and
    /// **it is reported rather than asserted away**. An earlier hand-rolled
    /// implicit version of this exchanger closed to 1e-8% because it solved all
    /// three streams in one matrix; this composition trades that for reuse of
    /// three separately verified arrays.
    ///
    /// # Interpretation
    ///
    /// Closure at this level says the coupling is wired correctly and no energy
    /// is being created or lost at the junctions. It does not say the exchanger
    /// is well-sized -- see [`SteamGeneratorConfig::hot_side_conductance`].
    #[test]
    fn energy_balance_closes_across_the_exchanger() {
        let sg = settled(200.0);
        let st = sg.state();
        let q_hot = st.hot_side_duty.get::<watt>();
        let q_cold = st.cold_side_duty.get::<watt>();
        let closure = (q_hot - q_cold) / q_hot;
        println!(
            "steady-state closure: Q_hot = {:.6} MW, Q_cold = {:.6} MW, \
             (Q_hot - Q_cold)/Q_hot = {:+.4}%",
            q_hot / 1.0e6,
            q_cold / 1.0e6,
            100.0 * closure
        );
        assert!(
            q_hot > 0.0 && q_cold > 0.0,
            "both streams must transfer heat"
        );
        assert!(
            closure.abs() < 0.02,
            "energy balance closes only to {:.3}%",
            100.0 * closure
        );
    }

    /// V&V: **a step change in duty is filtered by the tube metal's thermal time
    /// constant** -- the steam outlet cannot track it instantly.
    ///
    /// # Why this matters
    ///
    /// Before 2026-08-12 the secondary side's only integrated state was its mass
    /// flow: the steam-generator outlet was `h_feed + Q/m_dot`, an algebraic
    /// function of the duty offered on that step. A duty step therefore appeared
    /// at the turbine inlet in one timestep, which no steam generator does. The
    /// tube metal is what makes the secondary genuinely transient, and this test
    /// measures that it does.
    ///
    /// # Methodology
    ///
    /// The exchanger is settled at 973.15 K helium, then the hot inlet is
    /// **stepped down 100 K** to 873.15 K and held. The steam-outlet temperature
    /// is recorded after the first plant timestep and after 200 s (long enough
    /// to re-settle -- five metal time constants). The reported quantity is the
    /// fraction of the eventual response achieved in the first step:
    ///
    /// ```text
    /// f_1 = (T_steam(dt) - T_steam(0)) / (T_steam(200 s) - T_steam(0))
    /// ```
    ///
    /// For a first-order lag of time constant `tau` this is
    /// `1 - exp(-dt/tau)`, so the measurement is also an indirect read on `tau`.
    /// Pass criterion: `f_1 < 0.05` -- at most 5% of the response in one step,
    /// i.e. unambiguously not instantaneous -- and a nonzero eventual response,
    /// so the test cannot pass by the exchanger simply not responding at all.
    ///
    /// # Results (measured 2026-08-12 at a 0.05 s timestep; **re-measured
    /// 2026-08-13** at the 0.1 s plant timestep with the arrays at 2 outer
    /// correctors -- current figures are printed by the test)
    ///
    /// | Quantity | 2026-08-12, dt = 0.05 s | **2026-08-13, dt = 0.1 s** |
    /// |---|---|---|
    /// | Steam outlet before the step | 663.000 K | **663.000 K** |
    /// | after the first step | 662.999 K | **662.965 K** |
    /// | after 200 s | 523.595 K | **523.595 K** |
    /// | Total response | -139.404 K | **-139.404 K** |
    /// | **Fraction tracked in the first step** | 0.0005% | **0.0248%** |
    /// | `tau = C_metal/UA` at 600 K | 38.42 s | **38.42 s** (1.6366e6 J/K / 4.26e4 W/K) |
    /// | `1 - exp(-dt/38.42)`, a pure first-order lag | 0.1301% | **0.2600%** |
    ///
    /// Doubling the timestep doubles the pure-first-order comparator, and the
    /// measured fraction grew from 0.0005% to 0.0248% -- faster than
    /// proportionally, because the first step now covers a larger slice of the
    /// shell transport delay. The settled end state is **identical to three
    /// decimal places** (523.595 K), which is the more important reading: the
    /// timestep changed how the first step is resolved, not where the exchanger
    /// ends up.
    ///
    /// The point of the test is the **ratio** between the measured fraction and
    /// the pure-first-order comparator, which is a property of the transport
    /// delay rather than of the timestep: 0.0038 at 0.05 s, 0.095 at 0.1 s.
    ///
    /// The 2026-08-12 measurement of 0.0005% is **250x slower than even the
    /// metal lag alone**,
    /// and that is expected rather than suspicious: the metal is not the first
    /// thing in the path. A change at the helium inlet must first be advected
    /// down the shell array before the metal at the far end sees it at all, so
    /// the exchanger's response to a hot-inlet step is a transport delay
    /// followed by the metal lag, not a single exponential. The steam outlet is
    /// therefore, to five significant figures, **unmoved** one timestep after a
    /// 140 K step -- which is the property being asserted.
    ///
    /// For comparison, the model this replaces computed the steam outlet as
    /// `h_feed + Q/m_dot` with `Q` taken from the current step's duty: it would
    /// have tracked **100%** of the step in the first timestep, at any
    /// timestep.
    ///
    /// # Interpretation
    ///
    /// The metal lag is the dominant filter between the primary and the turbine
    /// inlet, and it is **derived** -- from the tube geometry, the steel's own
    /// density and specific heat, and the conductances -- rather than being a
    /// tuned first-order time constant like [`super::primary_loop`]'s core and
    /// return lags. That does not make it validated: the conductances it is
    /// divided by are a calibration.
    #[test]
    fn a_duty_step_is_filtered_by_the_metal_time_constant() {
        let dt_s = crate::physics::PLANT_TIMESTEP_S;
        let mut sg = settled(200.0);
        let before = sg.state().cold_outlet_temperature.get::<kelvin>();

        let stepped = ThermodynamicTemperature::new::<kelvin>(873.15);
        let after_one = sg
            .advance_timestep(dt(), stepped, hot_flow(), feedwater(), cold_flow())
            .unwrap()
            .cold_outlet_temperature
            .get::<kelvin>();

        for _ in 0..steps_for(200.0) {
            sg.advance_timestep(dt(), stepped, hot_flow(), feedwater(), cold_flow())
                .unwrap();
        }
        let eventual = sg.state().cold_outlet_temperature.get::<kelvin>();

        let total = eventual - before;
        let first = after_one - before;
        let fraction = if total.abs() > 1e-9 {
            first / total
        } else {
            0.0
        };
        let tau = sg
            .metal_time_constant(ThermodynamicTemperature::new::<kelvin>(600.0))
            .get::<second>();

        println!(
            "100 K hot-inlet step down: steam outlet {before:.3} K -> {after_one:.3} K after one \
             {dt_s} s step -> {eventual:.3} K after 200 s.\n  \
             first step tracked {:.4}% of the {total:.3} K total response \
             (first-order lag of tau = {tau:.2} s would give {:.4}%)",
            100.0 * fraction,
            100.0 * (1.0 - (-dt_s / tau).exp()),
        );

        assert!(
            total.abs() > 5.0,
            "the 100 K hot-inlet step moved the steam outlet only {total} K; \
             the test is not exercising a response"
        );
        assert!(
            fraction < 0.05,
            "the steam outlet tracked {:.2}% of a duty step in one {dt_s} s timestep; \
             the metal is not providing a real lag",
            100.0 * fraction
        );
        assert!(
            tau > 1.0,
            "the metal time constant {tau} s is implausibly short"
        );
    }

    /// The resolved tube side must show all three zones of a once-through steam
    /// generator: an economiser, an evaporator pinned on the saturation line,
    /// and a superheater.
    ///
    /// **Methodology.** Settle the exchanger at the design point and inspect the
    /// water-side node temperatures. The evaporator is identified as nodes
    /// sitting within 1 K of `T_sat(4.0 MPa) = 523.51 K`; the superheater as
    /// nodes above it; the economiser as nodes below.
    ///
    /// **Results (measured 2026-08-12).** Water-side node temperatures at the
    /// design point, hot-inlet first \[K\]:
    ///
    /// ```text
    /// [663.00, 523.49, 523.59, 523.51, 523.58, 464.61, 450.91, 335.47]
    ///  ^superheat  ^-------- evaporating --------^  ^--- economising ---^
    /// ```
    ///
    /// **1 superheating, 4 evaporating, 3 economising** node, against
    /// `T_sat(4.0 MPa) = 523.51 K`. The four plateau nodes sit within 0.08 K of
    /// the saturation line, which is the evaporator appearing on its own out of
    /// the IF97 `(p,h)` flash rather than being imposed. With only 8 nodes the
    /// plateau is visible but the boiling-front *position* is resolved to about
    /// one eighth of the tube, so do not read a front location off it. Pass
    /// criterion: at least one node in each zone.
    ///
    /// **Interpretation.** This is the structural difference from the model it
    /// replaces. The old secondary had one control volume and one enthalpy, so
    /// it could not have said where the water boiled; the isothermal-sink IHX
    /// assumed the whole cold side was the evaporator, which is precisely the
    /// assumption that made the superheater's collapsing pinch invisible.
    #[test]
    fn the_cold_stream_resolves_all_three_zones() {
        let sg = settled(200.0);
        let st = sg.state();
        let t_sat = 523.51_f64;
        let cold: Vec<f64> = st
            .cold_node_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let superheat = cold.iter().filter(|t| **t > t_sat + 1.0).count();
        let evaporating = cold.iter().filter(|t| (**t - t_sat).abs() <= 1.0).count();
        let economising = cold.iter().filter(|t| **t < t_sat - 1.0).count();
        println!(
            "water-side nodes (hot-inlet first): {cold:?}\n  \
             superheating {superheat}, evaporating {evaporating}, economising {economising} \
             (T_sat(4.0 MPa) = {t_sat} K)"
        );
        assert!(superheat >= 1, "no superheating node");
        assert!(evaporating >= 1, "no evaporating node");
        assert!(economising >= 1, "no economising node");
    }

    /// V&V (regression): **the accumulator is caller-rate-independent** -- the
    /// exchanger must reach the same state whether its caller hands it the
    /// plant timestep or 1 ms slices of it.
    ///
    /// # Why this test exists -- it caught a real crash
    ///
    /// Until 2026-08-13 `htgr_sim_v1`'s physics thread stepped the plant at
    /// `PHYSICS_DT_S = 1.0e-3 s`, ten sub-steps per 10 ms tick, to keep the
    /// animation smooth, while every test here drove the exchanger at 0.05 s.
    /// Handed straight through, 1 ms is *below* the arrays' stability window:
    /// the water side begins resolving its own acoustic transient and the
    /// IAPWS-IF97 `(p,h)` flash leaves its Region 5 range and panics.
    ///
    /// **The plant now steps at
    /// [`crate::physics::PLANT_TIMESTEP_S`] = 0.1 s, so the 1 ms case is no
    /// longer the application's rate.** The test is kept, and kept at 1 ms,
    /// because the property it checks is the accumulator's -- that simulated
    /// time is neither lost nor double-counted however finely it is delivered
    /// -- and 1 ms is the hardest case for it: 100 caller calls per array
    /// substep, 99 of which must be pure zero-order holds.
    ///
    /// That is not hypothetical. Measured 2026-08-12, before
    /// [`SteamGeneratorConfig::substep`] was made a fixed accumulate-and-advance
    /// clock, launching the simulator killed the physics thread within 30 s:
    ///
    /// ```text
    /// [physics thread panicked] htgr-physics failed in helium primary loop
    /// (circulator + hot gas duct): (p,h) point lies above the 1073.15 K
    /// isotherm. (p,h) flash into IAPWS-IF97 Region 5 is unsupported...
    /// ```
    ///
    /// The whole test suite was green at the time, because nothing in it drove
    /// the plant at the rate the application does.
    ///
    /// # Methodology
    ///
    /// Two exchangers with identical configuration and boundary conditions are
    /// marched over the **same 100 s of simulated time**: one at the plant
    /// timestep (1000 calls at 0.1 s), one at 1 ms (100 000 calls). Asserted:
    ///
    /// 1. neither panics;
    /// 2. their steam-outlet temperatures agree to within 1 K, i.e. the
    ///    accumulator neither loses nor double-counts simulated time;
    /// 3. their duties agree to within 1%.
    ///
    /// The two are expected to agree closely rather than exactly: the 1 ms
    /// caller supplies boundary conditions 100x more often, and the exchanger
    /// zero-order-holds whichever values were latest when a whole substep
    /// completed. Here the boundary conditions are constant, so that difference
    /// vanishes and what remains is the substep alignment.
    ///
    /// # Results (measured 2026-08-12; re-measured 2026-08-13 with the coarse
    /// leg moved to the 0.1 s plant timestep and the arrays at 2 outer
    /// correctors)
    ///
    /// **Measured 2026-08-13**, over 100 s of simulated time:
    ///
    /// | Caller rate | Calls | Steam outlet | `Q_hot` |
    /// |---|---|---|---|
    /// | 0.1 s (the plant timestep) | 1 000 | 671.619 K | 9.7194 MW |
    /// | 0.001 s | 100 000 | 671.619 K | 9.7194 MW |
    /// | **Difference** | | **0.0000 K** | **+0.0000%** |
    ///
    /// Both runs advance the arrays exactly 8000 times over 100 s, because both
    /// accumulate to the same 0.0125 s clock -- and with the boundary
    /// conditions constant the agreement is exact rather than merely close,
    /// which is the strongest form of the property. (At the 0.05 s caller rate
    /// used before 2026-08-13 the same comparison also agreed.)
    #[test]
    fn the_gui_millisecond_timestep_reaches_the_same_state() {
        // The GUI's plant timestep -- `crate::app::mod::PHYSICS_DT_S`.
        let gui_dt = Time::new::<second>(1.0e-3);

        let coarse_calls = (100.0 / crate::physics::PLANT_TIMESTEP_S).round() as usize;
        let mut coarse = NodalisedCounterFlowSteamGenerator::new(htr10()).unwrap();
        for _ in 0..coarse_calls {
            coarse
                .advance_timestep(dt(), hot_inlet(), hot_flow(), feedwater(), cold_flow())
                .unwrap();
        }

        let mut fine = NodalisedCounterFlowSteamGenerator::new(htr10()).unwrap();
        for _ in 0..100_000 {
            fine.advance_timestep(gui_dt, hot_inlet(), hot_flow(), feedwater(), cold_flow())
                .unwrap();
        }

        let c = coarse.state();
        let f = fine.state();
        let t_c = c.cold_outlet_temperature.get::<kelvin>();
        let t_f = f.cold_outlet_temperature.get::<kelvin>();
        let q_c = c.hot_side_duty.get::<watt>();
        let q_f = f.hot_side_duty.get::<watt>();
        println!(
            "100 s of simulated time, two caller rates:\n  \
             at the plant timestep ({coarse_calls} calls): steam {t_c:.3} K, Q_hot {:.4} MW\n  \
             at 0.001 s (100000 calls): steam {t_f:.3} K, Q_hot {:.4} MW\n  \
             difference: {:.4} K, {:+.4}%",
            q_c / 1.0e6,
            q_f / 1.0e6,
            (t_c - t_f).abs(),
            100.0 * (q_f - q_c) / q_c,
        );

        assert!(
            (t_c - t_f).abs() < 1.0,
            "the two caller rates disagree by {} K on the steam outlet; the substep \
             accumulator is losing or double-counting simulated time",
            (t_c - t_f).abs()
        );
        assert!(
            ((q_f - q_c) / q_c).abs() < 0.01,
            "the two caller rates disagree by {:.3}% on the duty",
            100.0 * (q_f - q_c) / q_c
        );
    }

    /// A call shorter than one substep must advance nothing and hand back the
    /// previous state unchanged -- and the time must not be lost: enough such
    /// calls must eventually complete a substep.
    #[test]
    fn sub_substep_calls_accumulate_rather_than_advancing_or_vanishing() {
        let mut sg = NodalisedCounterFlowSteamGenerator::new(htr10()).unwrap();
        let seeded = sg.state().cold_outlet_temperature.get::<kelvin>();
        let tiny = Time::new::<second>(1.0e-3);

        // Derive how many 1 ms calls fall JUST SHORT of one substep, rather
        // than hardcoding it. The previous version pinned 12 (0.012 s against
        // the then-0.0125 s substep) and so broke the moment the sub-step count
        // changed, reporting a stale expectation instead of the accumulate-
        // don't-advance property it exists to guard.
        let substep_s = crate::physics::steam_generator_substep_seconds();
        let short_calls = ((substep_s / 1.0e-3).ceil() as usize) - 1;
        for _ in 0..short_calls {
            let st = sg
                .advance_timestep(tiny, hot_inlet(), hot_flow(), feedwater(), cold_flow())
                .unwrap();
            assert!(
                (st.cold_outlet_temperature.get::<kelvin>() - seeded).abs() < 1e-12,
                "the exchanger advanced before a whole substep had accumulated"
            );
        }
        // The next one completes it.
        let st = sg
            .advance_timestep(tiny, hot_inlet(), hot_flow(), feedwater(), cold_flow())
            .unwrap();
        assert!(
            (st.cold_outlet_temperature.get::<kelvin>() - seeded).abs() > 1e-9,
            "the accumulated time never reached the arrays"
        );
    }

    /// The exchanger must reject configurations it cannot build, rather than
    /// producing a plausible-looking exchanger from impossible geometry.
    #[test]
    fn impossible_configurations_are_rejected() {
        let mut c = htr10();
        c.node_count = 2;
        assert_eq!(
            NodalisedCounterFlowSteamGenerator::new(c)
                .err()
                .expect("must be rejected"),
            SteamGeneratorError::TooFewNodes(2)
        );

        let mut c = htr10();
        c.geometry.tube_outer_diameter = c.geometry.tube_inner_diameter;
        assert_eq!(
            NodalisedCounterFlowSteamGenerator::new(c)
                .err()
                .expect("must be rejected"),
            SteamGeneratorError::TubeWallNotPositive
        );

        let mut c = htr10();
        c.geometry.tube_count = 0.0;
        assert_eq!(
            NodalisedCounterFlowSteamGenerator::new(c)
                .err()
                .expect("must be rejected"),
            SteamGeneratorError::NonPositiveGeometry
        );

        let mut c = htr10();
        c.substep = Time::new::<second>(0.0);
        assert_eq!(
            NodalisedCounterFlowSteamGenerator::new(c)
                .err()
                .expect("must be rejected"),
            SteamGeneratorError::NonPositiveSubstep
        );
    }
}
