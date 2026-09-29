// SPDX-License-Identifier: GPL-3.0

//! **Helical-coil once-through steam generator** -- spatially resolved,
//! counter-flow, hot fluid <-> tube metal <-> water/steam.
//!
//! # Provenance: moved here from `htgr_sim_v1` on 2026-09-27
//!
//! This exchanger was written and lived in
//! `outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/steam_generator.rs`.
//! **Maintainer direction, 2026-09-27: it belongs in a `tampines` library
//! file, and is to be tested here.** It is thermal-hydraulic component physics,
//! which is what this crate is for, and the engine crate's own `CLAUDE.md` says
//! in the first place that no new physics belongs in *its* library -- so moving
//! it out satisfies both rules at once.
//!
//! The move was **mechanical**: the 1 303 lines of production code carried
//! **zero** references to anything in the example (verified by grep for
//! `crate::physics` and `super::` across the whole of it), so nothing had to be
//! redesigned to make it portable. Only three things changed:
//!
//! 1. `use tampines::compressible::...` became `use crate::compressible::...`;
//! 2. doc links into the example's modules were rewritten, since they no longer
//!    resolve from here;
//! 3. the array substep, which the example held as a physics constant, is now
//!    [`DEFAULT_SUBSTEP_SECONDS`] here -- it is a property of *these arrays*,
//!    not of any particular plant, and keeping it beside them is what stops the
//!    two copies drifting.
//!
//! The example now re-exports this module rather than owning a second copy.
//!
//! # Why it was moved: it is the subject of an open defect
//!
//! GitHub #319 -- `htgr_sim_v1`'s cold side reaches 273.15 K at 4 MPa and
//! panics the IF97 flash. The maintainer's objection is the one that matters:
//! **a 4 MPa cold side has no physical route to 273 K when feedwater enters at
//! 313.15 K.** Feedwater enthalpy at the design point is ~171 kJ/kg; the
//! panicking cell sits at ~4 kJ/kg, a factor of 42 of energy unaccounted for.
//!
//! Diagnosing that inside a whole plant is hopeless -- point kinetics, the
//! helium loop, outer correctors, a feedwater controller, a protection system
//! and a timestep accumulator all reach the exchanger. [`standalone`] drives
//! **this** exchanger with fixed boundary conditions and nothing else, so what
//! it does is a property of the exchanger. That harness is the reason the move
//! happened now.
//!
//! # Scope
//!
//! Illustrative geometry and conductances (see
//! [`SteamGeneratorGeometry::htr10_illustrative`]), **not** a licensed design's,
//! and not validated against a measured HTR-10 steam generator. Research,
//! education and V&V only.

// ---------------------------------------------------------------------------
// Original module documentation, carried over verbatim from htgr_sim_v1.
// ---------------------------------------------------------------------------
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
//! Until 2026-08-12 `htgr_sim_v1`'s primary loop modelled the secondary side as an
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
//! - ~~**The water side is tabulated to 1073.15 K** (IAPWS-IF97 regions 1, 2, 4),
//!   and `tampines-steam-tables` panics rather than returning an error. The
//!   exchanger cannot itself drive the steam past the helium, so this is only
//!   reachable via a hot side above 800 degC.~~
//!   **CORRECTED 2026-09-27 — the ceiling is 2273.15 K, and it IS reachable
//!   without a hot helium side.** Two separate errors:
//!   1. **The ceiling moved.** `tampines-steam-tables` commit `2ab91fefc3`
//!      (2026-09-14) gave the `(p,h)` flash an IF97 **Region 5** arm, so the
//!      water side now runs to **2273.15 K**, not 1073.15 K. The panic message
//!      the exchanger actually produces says so verbatim: `(p,h) point lies
//!      above the 2273.15 K isotherm, the upper temperature bound of
//!      IAPWS-IF97 Region 5.` It still panics rather than erring — that half
//!      stands.
//!   2. **"only reachable via a hot side above 800 degC" is false**, and this is
//!      the half that matters. Verified by running
//!      [`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`]
//!      (244.81 s): at the **steady design point**, with the hot side at its
//!      ordinary temperature, a **0.1 s substep** drives a cold cell past
//!      2273.15 K and hits exactly that panic — at 1 coupling iteration and at
//!      8 alike. A numerical instability, not a hot inlet, is what reaches the
//!      ceiling. Above Region 5 there is **no IF97 formulation at all**, so the
//!      panic is the correct behaviour; what was wrong was the claim about how
//!      one gets there.
//!   The Region 5 temperatures the flash returns *below* 2273.15 K are this
//!   crate's in-house Chebyshev fits, not IAPWS values — IAPWS publishes no
//!   Region 5 backward `(p,h)` equation. See
//!   `tampines-steam-tables`'s `ph_flash_eqm::t_ph_eqm`.
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
//! **Measured 2026-08-13** (release, 8 nodes, a ~~the shipped 0.0125 s~~
//! **0.0125 s** substep, then 4 outer correctors): **1.9585 s of compute per second of
//! simulated time** on its own, against **1.9469** for the whole plant around
//! it. The exchanger is therefore about **96% of `htgr_sim_v1`'s compute**, and
//! the remaining 4% is everything else in the plant put together.
//!
//! **CORRECTED 2026-09-27 — 0.0125 s is NOT "the shipped substep".** The shipped
//! substep is **0.05 s**: `htgr_sim_v1` computes it as `PLANT_TIMESTEP_S /
//! STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP` = `0.1 / 2`, verified at
//! `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/mod.rs:641`
//! (`PLANT_TIMESTEP_S = 0.1`) and `:720`
//! (`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP = 2`) — cited, not edited, that
//! file is outside this crate. The divisor was cut 8 → 2 by commit
//! `c0e85a503e` on 2026-08-13 at **10:59**, *after* the measurement above was
//! taken, and this sentence was never updated.
//!
//! **Which way the error goes matters:** the figures above were taken at a
//! substep **4x smaller** than ships, and cost is very nearly linear in
//! `1/substep` (see the next bullet), so they are roughly a **4x
//! over**-estimate of the shipped exchanger's cost — and the "96% of compute"
//! share is likewise an over-estimate. **Nobody has re-measured the cost at
//! 0.05 s**; that is stated as open rather than scaled by hand, because the
//! linearity was itself only measured over 0.0125-0.025 s. See
//! [`SteamGeneratorConfig::substep`] for the stability sweep that *was* re-run.
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
//! `tests::the_courant_number_bounds_the_array_substep`.
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

use crate::compressible::{CompressibleFluidArray, CoolPropFluid};
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
    /// | ~~0.05 s~~ | ~~0.888~~ | ~~--~~ | ~~**fails** (also at 4, 8, 16 outer correctors); enthalpy goes odd-even and clamps~~ **CORRECTED 2026-09-27 — 0.05 s COMPLETES CLEANLY, and so does 0.075 s** |
    /// | 0.025 s | 0.444 | 9.8244 MW | stable, but **+1.44%** off converged |
    /// | **0.0125 s** | **0.222** | **9.6854 MW** | **stable, +0.003% off converged** |
    /// | 0.00625 s | 0.111 | 9.6851 MW | reference |
    /// | 0.001 s | 0.018 | -- | water side resolves its own acoustic transient; the IF97 `(p,h)` flash leaves Region 5 range and **panics** |
    ///
    /// ## ~~0.05 s fails~~ — **CORRECTED 2026-09-27**
    ///
    /// **What was verified, by running rather than reading.**
    /// [`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`]
    /// (`#[ignore]`d for cost; `cargo test --release -j 3 -p tampines --lib
    /// how_far_the_implicit_coupling_raises_the_stable_substep -- --ignored
    /// --nocapture`, **244.81 s**) sweeps the substep at the steady design point
    /// with 1 coupling iteration (the old block-Jacobi behaviour) and with 8:
    ///
    /// ```text
    /// substep  0.0125 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=3
    /// substep  0.0250 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=4
    /// substep  0.0500 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=4
    /// substep  0.0750 s  | n=1: Completed coldest=320.2K iters=1 | n=8: Completed coldest=320.2K iters=6
    /// substep  0.1000 s  | n=1: PANIC                            | n=8: PANIC
    /// ```
    ///
    /// The 0.1 s row still fails, and fails the way the table says — the panic is
    /// `(p,h) point lies above the 2273.15 K isotherm`, i.e. the cold side leaves
    /// IF97 entirely, which is the odd-even blow-up's signature. **0.05 s and
    /// 0.075 s complete**, and complete even at `n=1`, so the coupling iterations
    /// added on 2026-09-27 are *not* what rescued them.
    ///
    /// **RE-MEASURED 2026-09-29 (gh:#319) — the 0.075 s row no longer holds.**
    /// `TampinesSteamArray`'s energy convection moved that day from an
    /// unbounded linear face value to a bounded van Leer scheme (the linear one
    /// walked the cold side's inlet cell to the IF97 273.15 K floor). The same
    /// sweep, 115.74 s, `cargo test --release -j2 -p tampines --lib
    /// how_far_the_implicit_coupling_raises_the_stable_substep -- --ignored
    /// --nocapture`:
    ///
    /// ```text
    /// substep  0.0125 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.3K iters=3
    /// substep  0.0250 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.4K iters=4
    /// substep  0.0500 s  | n=1: Completed coldest=320.3K iters=1 | n=8: Completed coldest=320.3K iters=4
    /// substep  0.0750 s  | n=1: PANIC                            | n=8: PANIC
    /// substep  0.1000 s  | n=1: PANIC                            | n=8: PANIC
    /// ```
    ///
    /// The ceiling moved from (0.075, 0.1) s to **(0.05, 0.075) s**. The
    /// 0.1 s panic is preceded by a drained-cell hold in vapour cell 6
    /// (`rho_old = 9.59 kg/m3`, `rho_old - dt div(phi) = -2.67 kg/m3`): the
    /// array's **explicit continuity** is over-draining a low-density cell, a
    /// mass-Courant limit inside the array. 0.05 s, which `htgr_sim_v1`
    /// ships, still completes.
    ///
    /// **Why the row was wrong, confirmed from the history.** Three commits
    /// landed within 47 minutes on 2026-08-13 (`git log -1 --date=iso`):
    ///
    /// | Time | Commit | What it did |
    /// |---|---|---|
    /// | 10:12 | `3acc95362e` | **wrote this table**, including the `0.05 s … fails` row (`git show 3acc95362e` adds the line verbatim) |
    /// | 10:27 | `68e35551c2` | put the helium side's energy convection on **`EnergyBalanceMode::Implicit`** — 26 added lines, removing exactly the `Co < 1` ceiling the row was measuring |
    /// | 10:59 | `c0e85a503e` | cut the substep divisor **8 → 2**, i.e. 0.0125 s → 0.05 s, on the strength of that |
    ///
    /// So the row was measured **15 minutes before** the fix that made 0.05 s
    /// viable, the caller was moved onto 0.05 s 32 minutes after the fix, and the
    /// table was never re-measured. The row has been contradicted by the shipped
    /// configuration ever since.
    ///
    /// **The shipped substep is 0.05 s**, not the 0.0125 s this doc used to claim:
    /// `htgr_sim_v1` computes it as `PLANT_TIMESTEP_S /
    /// STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP` = `0.1 / 2`
    /// (`crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/mod.rs:641`
    /// and `:720` — cited, not edited; that file is outside this crate).
    ///
    /// **Completing is NOT the same as being clean.** The corrected reading is
    /// *"0.05 s and 0.075 s complete"*, **not** *"0.05 s and 0.075 s are good"*.
    /// The sibling test
    /// [`super::helical_coil_sg_standalone::tests::the_implicit_coupling_converges_and_beats_real_time_at_the_largest_stable_substep`]
    /// runs 60 s at the 0.075 s substep and reports **`odd-even roughness =
    /// 75.3475 K`**
    /// ([`super::helical_coil_sg_standalone::odd_even_roughness`] — the mean
    /// `|T[i-1] - 2 T[i] + T[i+1]|` over the cold nodes). **That number is
    /// recorded, not interpreted**: no smooth-profile baseline has been measured
    /// for this 8-node cold side, which crosses the saturation dome and so
    /// carries large *genuine* curvature, and without one it cannot be said
    /// whether 75 K is checkerboard or physics. Measuring that baseline is open
    /// work; guessing which it is would be exactly the kind of
    /// reason-from-the-answer this table already got wrong once.
    ///
    /// **What is NOT claimed.** Nothing here re-measures the *accuracy* columns.
    /// 0.05 s completing is not 0.05 s being converged: the surviving
    /// `0.025 s → +1.44%` row is evidence the duty is still moving at four times
    /// the shipped step, so the shipped configuration is very likely *further*
    /// from converged than +1.44%, and nobody has measured by how much. That is
    /// an open question, not a resolved one.
    ///
    /// The **upper** bound is a Courant limit on the hot gas side and is not
    /// negotiable by raising the outer-corrector count -- see
    /// [`PimpleCorrectors`] and
    /// `tests::the_courant_number_bounds_the_array_substep`.
    ///
    /// The **lower** bound used to be the one that bit in practice: until
    /// 2026-08-13 `htgr_sim_v1`'s GUI stepped its plant at **1 ms**, and handed
    /// straight to the arrays that is the panicking case -- which is what the
    /// simulator did on its first wiring, dying in the crash modal within 30 s
    /// of launch. The plant now steps at
    /// the caller's plant timestep = 0.1 s, which is above the window
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

    /// **Maximum lateral-coupling (Picard) iterations per substep.**
    ///
    /// # Why this exists: the coupling used to be explicit, and that is what
    /// # capped the timestep
    ///
    /// The three arrays are linked by
    /// `lateral_link_new_temperature_vector_avg_conductance`, and TUAS puts that
    /// conductance **on the coefficient matrix diagonal** while the *neighbour's*
    /// temperature enters as a **source term** from a snapshot
    /// (`calculation.rs`: `coefficient_matrix[[i,i]] += sum_of_lateral_conductances[i]`
    /// against `power_source_vector[i] = sum_of_lateral_conductance_times_lateral_temperatures[i]`).
    /// Each array is therefore implicit in **its own** temperature and explicit
    /// in its neighbour's — **block Jacobi**, one sweep per substep.
    ///
    /// That lag is one thing bounding the stable timestep, and it is why raising
    /// the PIMPLE outer-corrector count never bought a larger one: those
    /// correctors iterate *inside* one array and never touch the coupling
    /// *between* arrays.
    ///
    /// ~~Measured before this change: 0.05 s "fails -- enthalpy goes odd-even and
    /// clamps", and 0.05 s panicked at 4, 8 and 16 correctors alike.~~
    /// **CORRECTED 2026-09-27** — that quote was repeated here from
    /// [`SteamGeneratorConfig::substep`]'s table, which had been stale since
    /// 2026-08-13. **It was never the "before this change" state.** Verified by
    /// running the sweep this doc cites
    /// ([`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`],
    /// 244.81 s): at **1** coupling iteration — which *is* the old single-sweep
    /// block Jacobi — 0.05 s and 0.075 s both complete, `coldest = 320.2 K`,
    /// `iters = 1`. So the coupling lag was **not** what stopped 0.05 s, and
    /// this mechanism must not be credited with having rescued it. What the
    /// iterations measurably do buy is recorded in the sweep's own output
    /// (`iters` rises 3 → 4 → 4 → 6 as the substep grows, i.e. the coupling
    /// residual genuinely needs more sweeps at a larger step); what bounds the
    /// substep between 0.075 s and 0.1 s is **not identified**, and both counts
    /// fail at 0.1 s alike.
    ///
    /// # What iterating buys
    ///
    /// Repeating the sweep, each time restarting from the **start-of-substep**
    /// state and re-linking from the **latest** iterate, is block Gauss-Seidel /
    /// Picard on the helium-tube-steam system. At convergence the lagged
    /// neighbour temperature equals the new one, which **is** the solution of the
    /// monolithic implicit three-way matrix — without assembling one, and reusing
    /// every array's own solver unchanged.
    ///
    /// Maintainer direction, 2026-09-27: the exchanger is to run at the caller's
    /// 0.1 s timestep with **no substepping**, via an implicit
    /// helium-tube-steam solve. This is that solve.
    ///
    /// `1` reproduces the old single-sweep Jacobi behaviour exactly, which is how
    /// the before/after comparison is made.
    pub max_coupling_iterations: usize,
    /// Convergence tolerance for the coupling iteration \[K\]: the largest node
    /// temperature change between successive iterates that counts as converged.
    ///
    /// Compared against the **maximum over all three arrays and all nodes**, so
    /// it is a worst-node criterion rather than an average. A residual this test
    /// cannot reach within [`Self::max_coupling_iterations`] is reported on
    /// [`SteamGeneratorState::coupling_residual_kelvin`] rather than silently
    /// accepted -- a non-converged step must be visible.
    pub coupling_tolerance_kelvin: f64,
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
///   state with the latest iterate. Because ~~both arrays carry~~ the cold
///   array carries (**CORRECTED 2026-09-29**: the hot array has been
///   `EnergyBalanceMode::Implicit` since 2026-08-13, and the cold array's
///   source was an unlimited linear `fvc::div` until 2026-09-29, gh:#319) its
///   enthalpy convection as an **explicit** `fvc::div_limited` source inside this loop,
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
    /// ~~Raising the count further does **not** buy a larger substep -- a 0.05 s
    /// substep panics at 4, 8 and 16 correctors alike.~~
    /// **CORRECTED 2026-09-27.** The *conclusion* survives and the *evidence*
    /// does not. Correctors still do not buy a larger substep — they iterate
    /// inside one array — but a **0.05 s substep no longer panics at all**, at
    /// any corrector count, because commit `68e35551c2` (2026-08-13, 15 minutes
    /// after that measurement) put the helium side's energy convection on
    /// `EnergyBalanceMode::Implicit` and removed the `Co < 1` ceiling the
    /// measurement was made against. Verified by running
    /// [`super::helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`]
    /// (244.81 s): 0.05 s and 0.075 s both complete, 0.1 s panics. Full sweep
    /// output and the commit timeline are in [`SteamGeneratorConfig::substep`].
    /// (**Re-measured 2026-09-29, gh:#319:** under the bounded cold-side
    /// convection scheme 0.075 s panics too; 0.05 s completes.)
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
    /// The cold side is nowhere near its Courant limit (`Co = 0.045` at a
    /// 0.0125 s substep against the hot side's 0.222 — 2026-08-12 numbers, and
    /// **not** the shipped substep: **CORRECTED 2026-09-27**, that is 0.05 s,
    /// so both Courant numbers are 4x these), so it is the hot side that sets
    /// the count and this side simply matches it. The 4.97x hot/cold ratio is
    /// what the argument rests on and is unaffected by the scaling.
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

    /// How many lateral-coupling (Picard) iterations the last substep used.
    ///
    /// `1` means the first sweep already met
    /// [`SteamGeneratorConfig::coupling_tolerance_kelvin`]. Equal to
    /// [`SteamGeneratorConfig::max_coupling_iterations`] means it ran out of
    /// iterations -- check [`Self::coupling_residual_kelvin`] before trusting the
    /// step.
    pub coupling_iterations: usize,
    /// Largest node temperature change \[K\] between the last two coupling
    /// iterates, over all three arrays.
    ///
    /// The convergence residual. Reported rather than asserted so a caller can
    /// see a step that did not converge instead of being told nothing.
    pub coupling_residual_kelvin: f64,
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
        // many correctors are used (measured 2026-08-12, i.e. WITH CONVECTION
        // EXPLICIT, which is the only regime that sentence is about: 0.05 s
        // panics at 4, 8 and 16; 0.1 s at 8 and 32). Putting `div(phi h)` in the
        // matrix removes that ceiling; the limiter survives as a deferred
        // correction.
        //
        // CONFIRMED 2026-09-27 that it did remove it, by running rather than
        // assuming: with this line in place a 0.05 s substep and a 0.075 s one
        // both complete at the steady design point, at 1 coupling iteration and
        // at 8 alike; 0.1 s still panics. See
        // `helical_coil_sg_standalone::tests::how_far_the_implicit_coupling_raises_the_stable_substep`
        // (244.81 s) and the corrected table in `SteamGeneratorConfig::substep`.
        // Several docs elsewhere in this file still said "0.05 s fails" until
        // that run; they were measured 15 minutes BEFORE this line landed.
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
            // Nothing has been solved yet, so no coupling iteration has run.
            // Zero rather than one, so a caller can tell "not advanced" from
            // "converged on the first sweep".
            coupling_iterations: 0,
            coupling_residual_kelvin: 0.0,
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
    /// ~~**Both** arrays carry the enthalpy convection term **explicitly** --
    /// their energy equation adds `fvc::div_limited(phi, he, limiter)` as a
    /// source rather than an `fvm::div` matrix contribution -- inside the PIMPLE
    /// outer corrector loop.~~ **CORRECTED 2026-09-27 — only the COLD array
    /// does.** The constructor of
    /// [`NodalisedCounterFlowSteamGenerator`] sets the **hot** (helium) array to
    /// `EnergyBalanceMode::Implicit` explicitly (see the comment block beside
    /// that call), so `div(phi h)` is in its matrix and the limiter survives
    /// there only as a deferred correction. That has been true since commit
    /// `68e35551c2` (2026-08-13) and this doc was never updated.
    ///
    /// **This is the load-bearing half of the sentence, not a detail**, because
    /// the hot side is the side that binds the timestep (`Co_hot` is 4.97x
    /// `Co_cold`). The explicit-Picard argument below therefore describes the
    /// array that was *never* the constraint.
    ///
    /// The cold (water/steam) array does carry convection explicitly --
    /// **CORRECTED 2026-09-29 (gh:#319):** ~~as `fvc::div_limited`~~ until that
    /// date it was `fvc::div`, an unlimited **linear** face value, not the
    /// limited one this doc named; `TampinesSteamArray` now uses
    /// `fvc::div_limited` with a van Leer default
    /// (`tampines_steam_tables::EnergyConvectionScheme`). For an
    /// explicit array the outer loop is a Picard iteration on an explicit
    /// convection source, whose contraction factor is the cell Courant
    /// number: it converges (and the scheme is then effectively implicit) while
    /// `Co < 1`, and diverges above it however many outer correctors are used.
    /// ~~So the Courant number is the hard constraint on
    /// [`SteamGeneratorConfig::substep`]~~ — **a** Courant constraint still
    /// bounds the substep, and `tests::the_courant_number_bounds_the_array_substep`
    /// records the measured values, but it is no longer the *hot* side's
    /// explicit `Co < 1`: measured 2026-09-27, a 0.05 s substep
    /// (`Co_hot = 0.888` on the 2026-08-12 scaling) and a 0.075 s one both
    /// complete, and 0.1 s fails — so whatever sets the ceiling now sits between
    /// 0.075 s and 0.1 s and has not been identified. Recorded as open rather
    /// than guessed. More correctors still do not relax it. **Re-measured
    /// 2026-09-29 (gh:#319):** with the cold side's energy convection bounded
    /// (van Leer), the ceiling sits between 0.05 s and 0.075 s — see
    /// [`SteamGeneratorConfig::substep`].
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

        let max_iters = self.config.max_coupling_iterations.max(1);
        let tol = self.config.coupling_tolerance_kelvin.max(0.0);
        let mut iterations_used = 1usize;
        let mut residual_k = f64::INFINITY;

        for _ in 0..substeps {
            // The start-of-substep state. Every coupling iteration restarts from
            // THIS, so iterating converges the coupling rather than marching the
            // exchanger `max_iters` substeps. All three arrays are `Clone` and
            // carry `node_count` nodes, so the copy is cheap.
            //
            // Cloning BEFORE any lateral link matters: linking pushes onto each
            // array's `lateral_adjacent_array_temperature_vector`, so restoring a
            // pre-link clone is also what clears the previous iterate's links.
            // Without it the vectors would accumulate one stale neighbour per
            // iteration and every iteration would double-count the coupling.
            let hot_at_step_start = self.hot.clone();
            let metal_at_step_start = self.metal.clone();
            let cold_at_step_start = self.cold.clone();

            // The neighbour temperatures each iteration links from. Seeded with
            // the start-of-substep values, so iteration 1 is exactly the old
            // single-sweep Jacobi step -- `max_coupling_iterations = 1`
            // reproduces the previous behaviour and makes the comparison honest.
            let mut link_hot = self.hot.get_temperature_vector();
            let mut link_metal = self
                .metal
                .get_temperature_vector()
                .map_err(|e| SteamGeneratorError::Array(format!("metal temps: {e:?}")))?;
            let mut link_cold = self.cold.get_temperature_vector();

            for iteration in 1..=max_iters {
                // Restore, so this iteration re-solves the SAME timestep with a
                // better estimate of the neighbours -- Picard, not marching.
                if iteration > 1 {
                    self.hot = hot_at_step_start.clone();
                    self.metal = metal_at_step_start.clone();
                    self.cold = cold_at_step_start.clone();
                }

                // 1. Boundary conditions. Mass-flow inlets, not velocity inlets:
                //    the velocity route derives a velocity from an assumed
                //    density and was measured up to +100% wrong, opening a
                //    1.33 MW imbalance across a converged exchanger.
                self.hot.set_inlet_mass_flowrate(hot_mass_flow);
                self.hot.set_inlet_enthalpy(hot_inlet_enthalpy);
                self.hot.set_outlet_pressure(self.config.hot_pressure);

                self.cold.set_inlet_mass_flowrate(cold_mass_flow);
                self.cold.set_inlet_enthalpy(cold_inlet_enthalpy);
                self.cold.set_outlet_pressure(self.config.cold_pressure);

                // 2. The counter-flow index map: cold cell j sits at hot station
                //    n-1-j.
                let cold_temps_in_hot_order = reverse(&link_cold);
                let metal_temps_in_cold_order = reverse(&link_metal);

                // 3. Reciprocal lateral links, from the LATEST iterate.
                self.hot
                    .lateral_link_new_temperature_vector_avg_conductance(
                        self.hot_node_conductance,
                        link_metal.clone(),
                    )
                    .map_err(|e| SteamGeneratorError::Array(format!("hot<-metal: {e:?}")))?;
                self.metal
                    .lateral_link_new_temperature_vector_avg_conductance(
                        self.hot_node_conductance,
                        link_hot.clone(),
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

                // 5. Residual: how far the neighbour estimates moved. At
                //    convergence the temperatures linked FROM equal those solved
                //    TO, which IS the fully implicit helium-tube-steam solution.
                let new_hot = self.hot.get_temperature_vector();
                let new_metal = self
                    .metal
                    .get_temperature_vector()
                    .map_err(|e| SteamGeneratorError::Array(format!("metal temps: {e:?}")))?;
                let new_cold = self.cold.get_temperature_vector();
                residual_k = max_temperature_change_kelvin(&link_hot, &new_hot)
                    .max(max_temperature_change_kelvin(&link_metal, &new_metal))
                    .max(max_temperature_change_kelvin(&link_cold, &new_cold));
                link_hot = new_hot;
                link_metal = new_metal;
                link_cold = new_cold;
                iterations_used = iteration;

                if residual_k <= tol {
                    break;
                }
            }
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
            coupling_iterations: iterations_used,
            coupling_residual_kelvin: residual_k,
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

/// Largest absolute difference between two temperature vectors \[K\].
///
/// The coupling iteration's convergence measure. Takes the **maximum** rather
/// than a norm, so one stubborn node cannot hide behind seven settled ones --
/// and an odd-even oscillation is exactly the failure mode that would.
///
/// Mismatched lengths return infinity rather than panicking or comparing a
/// prefix: that can only happen if an array were renodalised mid-step, and
/// silently declaring convergence there would be the worst available answer.
fn max_temperature_change_kelvin(
    before: &[ThermodynamicTemperature],
    after: &[ThermodynamicTemperature],
) -> f64 {
    if before.len() != after.len() {
        return f64::INFINITY;
    }
    before
        .iter()
        .zip(after.iter())
        .map(|(a, b)| (a.get::<kelvin>() - b.get::<kelvin>()).abs())
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
