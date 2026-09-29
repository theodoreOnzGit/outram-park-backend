//! One-node (whole-bed-as-a-single-spatial-volume) pebble-bed physics,
//! HTR-10-shaped -- the geometry/correlation home for every fidelity tier in
//! [`super`], and the site of the one real "one spatial node" thermal solve,
//! [`PebbleBedPorousMediaNode`] (the **`ReactorModelKind::OneNodePorousMedia`**
//! tier, and the default `htgr_sim_v1` opens on). See [`super`] for the
//! fidelity-selection enum this model is one variant of.
//!
//! Replaces this simulator's former prismatic-block core with a single lumped
//! **graphite-matrix pebble** control volume sitting in an HTR-10-sized bed:
//! fission power heats the pebbles, and the pebbles hand that heat to the
//! helium through one overall heat-transfer coefficient over the total pebble
//! surface area. The structure is deliberately the same as `fhr_sim_v2`'s
//! [`PebbleBedThermalHydraulics`] -- one enthalpy state, an externally supplied
//! coolant temperature, an enthalpy-to-temperature relation -- with the UO2
//! property relation replaced by a graphite-matrix one, because an HTR-10
//! pebble is 97% graphite by mass.
//!
//! [`PebbleBedThermalHydraulics`]: ../../../fhr_sim_v2/app/prke_backend/pebble_bed_thermal_hydraulics.rs
//!
//! **History: this module used to also hold `PebbleBedCore`, a simpler
//! effectiveness-NTU closure that treated the helium as external (zero fluid
//! capacitance).** It was the `ReactorModelKind::OneNode` tier from
//! 2026-08-16 until it was removed on 2026-08-17 once
//! `PebbleBedPorousMediaNode` -- a more physically complete two-temperature
//! (solid + fluid) implicit balance -- became the default. The removed
//! struct's derivation (why an effectiveness-NTU exponential form is exact
//! and bounded for a single isothermal-wall node, and why an earlier
//! arithmetic-mean version of it produced a second-law violation above
//! `NTU = 2`) is preserved in git history and in the workspace root
//! `CLAUDE.md`'s "Human review caught what the tests did not" section --
//! nothing here still depends on it.
//!
//! Also the geometry/correlation home for the placeholder fidelity tiers: the
//! HTR-10 core geometry, the Wakao film correlation and the pebble properties
//! defined here are properties of the *bed*, not of any one thermal solve, so
//! [`super::axial_seven_node`] and [`super::coarse_mesh_genfoam`] reuse the
//! free functions in this module rather than duplicating them. Only the
//! *thermal solve* -- [`PebbleBedPorousMediaNode::step`] -- is
//! solve-specific.
//!
//! ## Nodalisation -- read this first
//!
//! **The whole bed is ONE control volume.** All 27,000 pebbles, and the whole
//! of each pebble, are a single temperature node with a single enthalpy state.
//! Concretely:
//!
//! | Region | Nodes | What is assumed uniform inside |
//! |---|---|---|
//! | Pebble bed (all 27,000 elements) | **1** | temperature, burnup, power density, graphite properties |
//! | Inside a single pebble | ~~**1**~~ **2** (2026-09-28) | ~~fuel kernel, graphite matrix and outer shell are one temperature~~ the TRISO particles are the **fuel node** (`physics::kinetics`); matrix and shell are this node, with the resolved profile between them |
//! | Helium in the bed | ~~**0 (external)**~~ **1** (**CORRECTED 2026-09-29**: its own LTNE fluid node since 2026-08-17, on enthalpy since 2026-09-29) | ~~supplied by the caller as one bulk mean temperature~~ one well-mixed enthalpy state over the 197 cm bed's void; the caller supplies the inlet enthalpy |
//! | Reflector, core barrel, vessel | **0** | not modelled at all |
//!
//! The node boundary is the **pebble surface**: everything inside it is the
//! graphite node, everything outside is the caller's helium, and the two are
//! joined by one conductance `h A` over the total pebble surface area.
//!
//! **What that costs.** With one bed node this model *cannot* represent:
//!
//! - any **axial** temperature profile -- the real bed runs from the 250 degC
//!   inlet at the top to the 700 degC outlet at the bottom, and none of that
//!   gradient exists here;
//! - any **radial** profile, so no near-wall porosity effect, no hot channel,
//!   and no peak-to-average power factor;
//! - a **peak fuel temperature** -- [`PebbleBedPorousMediaNode::pebble_temperature`]
//!   is a bed average, and quoting it as a fuel temperature limit would be
//!   wrong;
//! - the **temperature drop inside a pebble**, from fuel kernel to ball
//!   surface, which is folded into the one overall coefficient;
//! - **multi-pass fuelling**, since with one node every pebble is identical and
//!   has the same residence history.
//!
//! **The refinement path**, in the order worth doing it: split the bed
//! **axially** first, into 5-10 stacked control volumes with the helium
//! marching down through them -- that is what buys the inlet-to-outlet gradient
//! and makes the outlet temperature a computed result rather than a
//! whole-bed lump. Then split **inside the pebble** radially (fuel zone, shell,
//! surface) so a real peak fuel temperature exists. Radial bed channels and a
//! reflector node come after both. Each of those needs an effective bed
//! conductivity, and the workspace now **has** one --
//! [`outram_park_digital_twin_engine::htr10::zbs`] -- so that closure is no
//! longer the blocker; the missing piece is the nodalisation for it to act on.
//!
//! ## What is real
//!
//! - **The bed geometry is the published HTR-10 core, read from the library.**
//!   Every published figure below now comes from
//!   [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`], the
//!   workspace's single provenance-checked transcription of IAEA-TECDOC-1382
//!   (*Evaluation of high temperature gas cooled reactor performance: Benchmark
//!   analysis related to initial testing of the HTTR and HTR-10*), rather than
//!   being re-typed here. Core diameter 1.8 m, mean height 1.97 m, 27,000
//!   spherical fuel elements of 6.0 cm diameter, volumetric filling fraction of
//!   balls 0.61 (void fraction 0.39), graphite density in the matrix and outer
//!   shell 1.73 g/cm^3, heavy metal 5.0 g per ball. Everything geometric below
//!   is *derived from those figures*, not chosen: the bed volume, the free-flow
//!   area, the total pebble surface area, and the graphite mass. There is no
//!   second copy of any of them to drift out of step with the library's.
//! - **The derived geometry closes against the report's own numbers.** The
//!   27,000 pebbles fill 60.9% of the cylinder against the published 0.61
//!   filling fraction, and the cylinder is 5.013 m^3 against the published
//!   5.0 m^3 (see [`tests::bed_geometry_reproduces_the_published_core`]).
//! - **The energy balance is a real first-order balance** on the pebble
//!   enthalpy: `C dT/dt = Q_fission - h A (T_pebble - T_helium)`, integrated
//!   explicitly. The graphite thermal inertia it carries -- ~~about 9.0 MJ/K over
//!   5.28 t of graphite~~ **8.87 MJ/K at 950 K over 5.13 t** since 2026-09-28
//!   (the coated particles, 2.9 % of the ball volume, moved to the fuel node,
//!   and `c_p(T)` replaced the constant) -- is a genuine consequence of the
//!   published geometry and density, and it is what makes a pebble-bed core
//!   respond slowly. ~~"fission power heats the pebbles"~~ **CHANGED
//!   2026-09-28 (gh:#360):** fission and decay heat are deposited in the fuel
//!   node, which conducts to this node; see [`FuelBedCoupling`].
//!
//! ## What is still illustrative -- read this before trusting any number
//!
//! **This is a placeholder, not a packed-bed model.** In plain terms:
//!
//! - **The bed friction is now real, but it lives next door and it is not a
//!   resolved bed.** The KTA packed-bed correlation
//!   ([`outram_park_digital_twin_engine::htr10::kta`]) is evaluated by
//!   [`super::super::primary_loop::bed_pressure_drop`] on this module's geometry, and
//!   it reproduces the published Virtual Test Bed worked example exactly. What
//!   it does *not* buy is a nodalised bed: the correlation is applied once at
//!   the bulk mean, not integrated down an axial profile, and the pressure drop
//!   still cannot feed back on the flow because there is no momentum equation.
//!   Wiring in a friction correlation makes the **friction** real. It does not
//!   make the **discretisation** real.
//! - **The effective bed conductivity exists but is unused, deliberately.**
//!   [`outram_park_digital_twin_engine::htr10::zbs`] now carries the
//!   Zehner-Bauer-Schlunder tabulation (11.94 to 44.95 W/(m K), 300-2000 K), so
//!   the closure is no longer missing from the workspace. It is not in this
//!   model's heat path because a single control volume has no internal
//!   temperature gradient for a conductivity to act on. What it is used for
//!   here is *quantifying the omission*
//!   ([`conduction_only_axial_heat_rate`]): at rated power the bed could carry
//!   11.74 kW by conduction, 0.117% of the 10 MW convected away, which is what
//!   justifies leaving it out **while forced flow exists**. With the circulator
//!   stopped that same conductivity is the whole heat path, and this model has
//!   nothing to say about that case.
//! - **The surface coefficient is EVALUATED, not invented (2026-08-14), and
//!   the pebble behind it is now RESOLVED (2026-09-22).**
//!   [`overall_htc_at_flow`] is two resistances in series: an evaluated
//!   **Wakao** packed-bed film (`Nu = 2 + 1.1 Re_p^0.6 Pr^(1/3)`, with the
//!   helium conductivity, viscosity and Prandtl number from the real CoolProp
//!   helium models at the published 3.0 MPa), and the **intra-pebble
//!   conduction**. Both replaced an invented lumped constant that put the bed
//!   204.7 K above the helium at rated power -- roughly *twice* the published
//!   peak. The evaluated path gives **68.1 K**, which correctly sits below the
//!   100.7 K peak of Gao & Shi (2002) Table 2 (918.7 / 876.7 / 818 degC
//!   maximum fuel, fuel-surface and coolant temperatures at 100% load). See
//!   [`tests::the_evaluated_coefficient_beats_the_old_invented_one`].
//!
//!   ~~"What this still does **not** buy is a resolved pebble: the fuel zone,
//!   shell and surface remain one temperature node, so the intra-pebble term is
//!   a bed-average drop and there is still no peak fuel temperature here."~~
//!   **CORRECTED 2026-09-22 -- it does now.** The intra-pebble leg was
//!   `h = 10 k/d`, the uniform-heated-sphere result, with an invented
//!   `k = 25 W/(m K)`. An HTR-10 ball does not generate uniformly out to its
//!   surface: it has a 5 mm unfuelled shell conducting the full power with
//!   none of its own, which no choice of `k` in that form can represent.
//!   [`intra_pebble_conduction_coefficient`] now solves
//!   [`tampines::pebble_bed::pebble`]'s two-zone pebble with
//!   temperature- and fluence-dependent A3 graphite and the TRISO dispersion,
//!   and [`resolved_pebble_profile`] **inverts** it so the node temperature is
//!   genuinely the ball's volume average rather than its surface.
//!
//!   **Measured cost of that (2026-09-22): the overall coefficient moved
//!   1.1 %**, 486.1 to 480.7 W/(m^2 K), because the film is ~88 % of the
//!   resistance and dilutes any pebble-side correction. What it bought instead
//!   is a **peak kernel temperature**, 21.3 K above the node at rated power --
//!   a fuel temperature the uniform ball could not produce at all. See
//!   [`tests::the_resolved_pebble_beats_the_uniform_ball`] and
//!   [`PebbleBedPorousMediaNode::peak_kernel_temperature`]. Still absent: a
//!   power peaking factor, so this is the peak kernel of a *core-average*
//!   pebble, and there is no burnup, so fluence is zero.
//! - ~~**Graphite `c_p` is one constant**, representative of graphite near
//!   1000 K. Real graphite `c_p` rises from about 710 J/(kg K) at 300 K to
//!   about 1700 J/(kg K) at 1000 K, so the constant is badly wrong cold and
//!   roughly right hot. No temperature- or fluence-dependent graphite property
//!   set exists in this workspace.~~ **CORRECTED 2026-09-28** -- the claim was
//!   false when written (`tuas_boussinesq_solver` has carried Butland &
//!   Maddison graphite cp since 2026-08-11), and the constant is gone: the bed
//!   reads [`graphite_specific_heat`] (`tuas`'s
//!   `NuclearGraphiteMatrixA3HighTemp`, Butland & Maddison polynomial 3,
//!   250-3000 K) at the live bed temperature and closes its balance on the
//!   exact graphite enthalpy. Fluence-dependent *conductivity* exists in `tuas`
//!   but is not threaded here (fluence is zero; gh:#361).
//! - **The illustrative constants are grouped**, deliberately, in the
//!   `Illustrative closure constants` block below, so no invented number is
//!   mixed in with the published geometry above it. Replacing the invented
//!   figures with sourced ones is tracked as bead `op-szmi.6`.
//! - **There is no multi-pass pebble flow and no burnup distribution.**
//! - **There is no reflector, barrel or cavity-cooling path IN THIS MODULE**,
//!   and the bed node has no second surface to lose heat through.
//!   ~~The HTR-10's passive decay-heat route is not modelled.~~
//!   **CORRECTED 2026-09-17** -- it is now modelled next door, in
//!   [`super::super::decay_heat_removal`], as a lumped bed -> reflector ->
//!   RPV -> RCCS chain whose heat rate [`super::super::HtgrPlant::step`]
//!   subtracts from the source term handed to [`PebbleBedPorousMediaNode::step`]
//!   before the bed is advanced. So this module sees the passive path only as
//!   a reduced source, never as a boundary condition of its own, and its `UA`
//!   values are placeholders.
//!
//! It is an offline demonstration model. It is **not** a validated pebble-bed
//! core model and must not be used for any purpose `RESPONSIBLE_USE.md`
//! excludes.

// The geometry helpers and state accessors below are the module's public
// surface: they exist so the app layer can put bed quantities on the snapshot
// and so a future nodalised bed can reuse the derived geometry. Not every one
// has a caller inside this example yet.
#![allow(dead_code)]

use outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint;
use outram_park_digital_twin_engine::htr10::zbs::zbs_effective_conductivity;
use uom::si::area::square_meter;
use uom::si::{f64::*, temperature_interval};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use outram_park_digital_twin_engine::htr10::kta;
use tampines::pebble_bed::pebble::{
    htr10_silicon_carbide_density, htr10_uranium_dioxide_density, Pebble, PebbleTemperatureProfile,
};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_enthalpy::{
    try_get_h, try_get_temperature_from_h,
};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::{Material, SolidMaterial};
use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
use outram_park_fork_offbeat::materials::MaterialState;
use outram_foam_basic_lib::prelude::SquareMatrix;
use outram_park_fork_coolprop::{Fluid, FluidState, conductivity, state_ph, state_pt, viscosity};
use super::super::decay_heat_removal::{CoreToRccsPath, PassiveCoupling};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::thermal_resistance::kelvin_per_watt;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::DynamicViscosity;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

// ---------------------------------------------------------------------------
// Published HTR-10 core geometry -- READ FROM THE LIBRARY, NOT RE-TYPED HERE
//
// `outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint` is the
// workspace's single transcription of IAEA-TECDOC-1382 Table 4-1 / section 4.1,
// with a citation on every field and unit tests that close its internal
// consistency (core volume against diameter and height, filling fraction
// against pebble count and diameter, porosity against filling fraction). This
// module reads that struct instead of holding a second copy: two copies of an
// operating point drift, and a drift between them would be silent.
// ---------------------------------------------------------------------------

/// The published HTR-10 design point this module derives all of its geometry
/// from. Cheap to construct (plain `Copy` data, no I/O), so it is called at each
/// use site rather than cached.
pub fn design() -> Htr10DesignPoint {
    Htr10DesignPoint::iaea_benchmark()
}

/// Pebble-bed core diameter: 180 cm (published, via [`design`]).
pub fn core_diameter() -> Length {
    design().core_diameter
}

/// Mean pebble-bed height: 197 cm (published, via [`design`]). This is also the
/// bed length the packed-bed pressure drop is integrated over.
pub fn core_mean_height() -> Length {
    design().average_core_height
}

/// Number of spherical fuel elements in the equilibrium core: 27,000
/// (published, via [`design`]).
pub fn pebble_count() -> f64 {
    design().fuel_element_count as f64
}

/// Spherical fuel-element diameter: 6.0 cm (published, via [`design`]). This is
/// the characteristic length (`D_h`) the KTA packed-bed correlation uses.
pub fn pebble_diameter() -> Length {
    design().pebble_diameter
}

/// Bed void fraction (porosity), dimensionless: `1 - 0.61` from the published
/// volumetric filling fraction of balls in the core, 0.61 — computed by
/// [`Htr10DesignPoint::bed_porosity`], so the 0.39 is derived, not asserted.
///
/// The same 0.39 void fraction is what the benchmark participants were required
/// to preserve when idealising the random packing as a lattice, and it is what
/// the KTA correlation consumes.
pub fn bed_porosity() -> Ratio {
    design().bed_porosity()
}

/// Density of the graphite matrix and outer shell of a fuel element:
/// 1.73 g/cm^3 (published, via [`design`]).
pub fn graphite_density() -> MassDensity {
    design().graphite_density
}

/// Heavy-metal (uranium) loading per fuel element: 5.0 g (published, via
/// [`design`]). Carried for completeness -- the lumped thermal model treats the
/// pebble as graphite, since the heavy metal is under 3% of the ball mass.
pub fn heavy_metal_per_pebble() -> Mass {
    design().heavy_metal_per_ball
}

// ---------------------------------------------------------------------------
// Illustrative closure constants -- NOT published data
// ---------------------------------------------------------------------------

/// ~~Graphite isobaric specific heat \[J/(kg K)\], held **constant**
/// (illustrative). 1700 J/(kg K) is representative of nuclear graphite near
/// 1000 K, which is where this core operates; it is badly wrong below about
/// 600 K, where real graphite `c_p` falls toward 710 J/(kg K). A
/// temperature-dependent graphite property set is recorded as MISSING in
/// `docs/reactor-scoping/htr10.md` and is not implemented here.~~
///
/// **RETIRED 2026-09-28 — no longer in the heat path.** ~~"A
/// temperature-dependent graphite property set is ... MISSING"~~ **CORRECTED
/// 2026-09-28** — it was not missing: `tuas_boussinesq_solver` has carried the
/// Butland & Maddison graphite cp since 2026-08-11. The bed now reads
/// [`graphite_specific_heat`] — `tuas`'s
/// [`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`], Butland & Maddison
/// polynomial 3, evaluated at the live bed temperature every step (713 J/(kg K)
/// at 300 K, 1760 at 1000 K, 2023 at 2000 K). Kept, renamed, only so the
/// before/after comparison in the tests can be made against the number it
/// replaced.
pub const LEGACY_GRAPHITE_CP_J_PER_KG_K: f64 = 1700.0;

/// The pebble matrix and shell graphite, as a `tuas_boussinesq_solver`
/// material: [`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`] (maintainer
/// direction 2026-09-28: "wire the graphite from tuas into the htgr_sim_v1").
///
/// The high-temperature variant, not the base `NuclearGraphiteMatrixA3`,
/// because this plant's shipped opening condition takes the bed past 2000 K
/// (gh:#350, gh:#351), where the base variant refuses. Below 2000 K the two
/// have the same conductivity to the last bit; the cp is Butland & Maddison
/// polynomial 3 in both (thermochemical vs International Table calorie,
/// 0.067 % apart). **Above 2000 K the conductivity is extrapolated** — see the
/// variant's doc comment.
pub const PEBBLE_GRAPHITE: SolidMaterial = SolidMaterial::NuclearGraphiteMatrixA3HighTemp;

/// Pressure handed to the `tuas` solid-property dispatchers. Solid properties
/// in `tuas` ignore it; the primary pressure is passed so the call reads as
/// what it is.
fn property_pressure() -> Pressure {
    design().primary_pressure
}

/// Isobaric specific heat of the pebble graphite at `temperature`, from
/// [`PEBBLE_GRAPHITE`] (Butland & Maddison polynomial 3).
///
/// # Panics
///
/// Outside the variant's 300-3000 K window. Below 300 K the bed would be
/// colder than the 50 degC RCCS boundary it can only lose heat to; above
/// 3000 K the fuel has long since failed and there is no property set to
/// extrapolate to. Either is a model defect, and per this workspace's
/// stale-state policy it stops the run rather than carrying on with a number
/// from outside the correlation.
pub fn graphite_specific_heat(temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    try_get_cp(
        Material::Solid(PEBBLE_GRAPHITE),
        temperature,
        property_pressure(),
    )
    .unwrap_or_else(|e| {
        panic!(
            "pebble graphite cp: bed temperature {} K left the 300-3000 K window of \
                 NuclearGraphiteMatrixA3HighTemp: {e:?}",
            temperature.get::<kelvin>()
        )
    })
}

/// Legacy lumped pebble-to-helium coefficient at nominal flow \[W/(m^2 K)\]
/// (**illustrative**), retained only as the comparison baseline.
///
/// **This is no longer the model's heat path.** Until 2026-08-14 this single
/// invented number lumped the internal pebble conduction and the surface film
/// together, and it was documented as low by a factor of two to three: it put
/// the bed 204.7 K above the bulk helium at rated power, against the 100.7 K
/// *peak* fuel-to-coolant difference of Gao & Shi (2002) Table 2, which a bed
/// *average* must sit below.
///
/// [`overall_htc_at_flow`] now evaluates the two resistances separately -- an
/// evaluated Wakao film in series with the intra-pebble conduction -- so no
/// invented overall coefficient enters the heat balance. This constant is kept
/// so [`tests::the_evaluated_coefficient_beats_the_old_invented_one`] can show
/// the improvement rather than merely asserting it.
pub const LEGACY_LUMPED_HTC_W_PER_M2_K: f64 = 160.0;

/// ~~Thermal conductivity of the pebble's graphite matrix \[W/(m K)\]
/// (**illustrative**, representative of A3-3 matrix graphite near the
/// operating temperature).~~
///
/// **RETIRED 2026-09-22 — no longer in the heat path.** It is kept only so
/// [`tests::the_resolved_pebble_beats_the_uniform_ball`] can show what
/// replacing it bought, and so the history of the number is not erased by a
/// silent delete.
///
/// ~~"No temperature- or fluence-dependent graphite property set exists in
/// this workspace, so this is one constant."~~ **CORRECTED 2026-09-22 — that
/// claim was false when written.** `tampines::pebble_bed::pebble` consumes
/// `tuas_boussinesq_solver`'s A3-grade matrix-graphite correlation, which is
/// both temperature- and fluence-dependent over 300-2000 K, and `tampines` was
/// already a dependency of this crate. The constant was not filling a gap in
/// the workspace; it was a second implementation of something the workspace
/// already had, which is exactly what the search-before-building rule exists
/// to prevent. [`intra_pebble_conduction_coefficient`] now calls that model.
///
/// Note this was also a different quantity from
/// [`outram_park_digital_twin_engine::htr10::zbs::zbs_effective_conductivity`],
/// which is the *bed-effective* conductivity (solid contact plus pebble-to-
/// pebble radiation across the voids) and is the wrong number for conduction
/// *inside* a ball. That distinction still holds.
pub const LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K: f64 = 25.0;

/// Reynolds-number exponent of the **Wakao** packed-bed particle-to-fluid
/// Nusselt correlation, dimensionless.
///
/// Retained as a named constant because it appears in the correlation
/// [`wakao_nusselt`] evaluates: `Nu = 2 + 1.1 Re_p^0.6 Pr^(1/3)`. Before
/// 2026-08-14 *only* this exponent was borrowed, and it scaled an invented
/// coefficient; the full correlation is now evaluated.
pub const HTC_FLOW_EXPONENT: f64 = 0.6;

/// Nominal helium mass flow the overall coefficient is anchored at: 4.3 kg/s at
/// full power (published, via [`design`]). Gao & Shi (2002) Table 2 carries
/// 4.32 kg/s for the equilibrium core at 100% load; the library field records
/// both readings and returns the 4.3 kg/s benchmark figure.
pub fn nominal_helium_flow() -> MassRate {
    design().helium_mass_flow
}

/// Nominal helium mass flow as a bare scalar \[kg/s\], for the ratio
/// arithmetic that does not want a `uom` round-trip.
pub fn nominal_helium_flow_kg_per_s() -> f64 {
    nominal_helium_flow().get::<kilogram_per_second>()
}

/// Bed temperature the model is seeded at \[K\] (illustrative, ~677 degC).
///
/// Chosen as the settled full-power bed average so the simulator opens near its
/// operating point instead of spending ten minutes of simulated time warming
/// 5.3 t of graphite up from cold.
const SEED_BED_TEMPERATURE_K: f64 = 950.0;

/// Bed temperature seeded as 3 MPA (or 3e6 Pa)
///
/// shown in literature to be operating pressure of helium
const SEED_BED_PRESSURE_PA: f64 = 3e6_f64;

// ---------------------------------------------------------------------------
// Derived geometry -- computed from the published figures above
// ---------------------------------------------------------------------------

/// Bed cylinder volume `pi D^2 H / 4` \[m^3\], derived from the published core
/// diameter and mean height. Comes out at 5.0130 m^3 against the report's own
/// stated 5.0 m^3.
pub fn bed_volume() -> Volume {
    core_diameter()
        * core_diameter()
        * core_mean_height()
        * Ratio::new::<ratio>(std::f64::consts::FRAC_PI_4)
}

/// Helium-filled void volume in the bed, `epsilon * V_bed` \[m^3\].
pub fn bed_void_volume() -> Volume {
    bed_volume() * bed_porosity()
}

/// Superficial (empty-cylinder) cross-sectional area of the bed \[m^2\]. This
/// is the area the KTA superficial mass flux `mdot/A` is formed on -- the whole
/// bed cross-section, *not* the pore area.
pub fn superficial_area() -> Area {
    core_diameter() * core_diameter() * Ratio::new::<ratio>(std::f64::consts::FRAC_PI_4)
}

/// Free-flow (interstitial) area available to the helium, `epsilon * A`
/// \[m^2\]. Do **not** feed this to the KTA correlation -- that closure is
/// written on the superficial area, with the porosity entering separately
/// through the `(1-eps)/eps^3` geometry factor.
pub fn free_flow_area() -> Area {
    superficial_area() * bed_porosity()
}

/// Volume of one spherical fuel element `pi d^3 / 6` \[m^3\].
pub fn pebble_volume() -> Volume {
    pebble_diameter()
        * pebble_diameter()
        * pebble_diameter()
        * Ratio::new::<ratio>(std::f64::consts::PI / 6.0)
}

/// Volume of **graphite** in one fuel element \[m^3\]: the ball less the
/// coated particles dispersed in it, `pi d^3/6 - N_p V_particle` (113.10 -
/// 3.29 = 109.81 cm^3 for HTR-10).
///
/// ~~"graphite only (the 5 g of heavy metal is under 3% of the ball and is not
/// counted in the thermal mass)"~~ **CHANGED 2026-09-28 (gh:#360)** — the
/// particles are no longer smeared into the graphite. They are the fuel node
/// ([`fuel_node_heat_capacity`]), so counting their 2.9 % of the ball volume
/// as graphite here as well would count that mass twice.
pub fn pebble_graphite_volume() -> Volume {
    let pebble = resolved_pebble();
    pebble_volume() - pebble.particle.particle_volume() * pebble.particles_per_pebble
}

/// Graphite mass of one spherical fuel element \[kg\]: matrix plus shell,
/// the coated particles excluded (see [`pebble_graphite_volume`]).
pub fn pebble_mass() -> Mass {
    graphite_density() * pebble_graphite_volume()
}

/// Total graphite mass held in the bed \[kg\]: `N * m_pebble`.
pub fn graphite_mass() -> Mass {
    pebble_mass() * pebble_count()
}

/// Total pebble surface area available for heat transfer to the helium
/// \[m^2\]: `N pi d^2`.
pub fn heat_transfer_area() -> Area {
    pebble_diameter()
        * pebble_diameter()
        * Ratio::new::<ratio>(std::f64::consts::PI * pebble_count())
}

/// Lumped thermal capacitance of the bed's graphite \[J/K\] at
/// `temperature`: `m_graphite * c_p(T)`, with `c_p` from [`PEBBLE_GRAPHITE`].
///
/// ~~"using the constant graphite `c_p` above"~~ **CHANGED 2026-09-28** —
/// temperature dependent now: 8.87 MJ/K at the 950 K design point (against
/// 8.98 MJ/K from the retired 1700 J/(kg K) on the old, particle-inclusive
/// mass), 3.64 MJ/K at 300 K and 10.23 MJ/K at 2000 K. The bed step itself
/// integrates the **enthalpy** rather than this capacitance, so the balance is
/// exact even though `c_p` moves inside a step; see
/// [`PebbleBedPorousMediaNode::step`].
pub fn bed_heat_capacity(temperature: ThermodynamicTemperature) -> HeatCapacity {
    graphite_mass() * graphite_specific_heat(temperature)
}

/// Fraction of the bed cylinder occupied by pebbles, derived from the published
/// pebble count and diameter -- the quantity the report itself states as the
/// "volumetric filling fraction of balls in the core", 0.61.
pub fn derived_filling_fraction() -> f64 {
    pebble_count() * pebble_volume().get::<cubic_meter>() / bed_volume().get::<cubic_meter>()
}

/// Axial heat rate the bed could carry by **conduction alone** across a
/// temperature difference `delta_t_kelvin` \[K\] spread over the full bed
/// height, evaluated at bed temperature `temperature`:
/// `Q = k_eff(T) * A_superficial * dT / H`.
///
/// `k_eff` is the Zehner-Bauer-Schlunder effective pebble-bed conductivity from
/// [`outram_park_digital_twin_engine::htr10::zbs`] -- the solid/gas/contact/
/// radiation bed-continuum property, tabulated 300-2000 K.
///
/// **This is a diagnostic, not a term in the model.** The bed here is one
/// control volume, so it carries no internal temperature gradient for a
/// conductivity to act on; this function exists to *quantify* what that
/// omission costs, and it is what justifies keeping the lumped surface
/// coefficient at power (see
/// [`tests::zbs_conduction_is_negligible_beside_convection_at_power`]). The
/// answer changes completely with the forced flow removed, which is exactly the
/// regime this model cannot enter.
pub fn conduction_only_axial_heat_rate(
    temperature: ThermodynamicTemperature,
    delta_t_kelvin: f64,
) -> Power {
    // `uom` treats a temperature *interval* as a distinct kind from an absolute
    // temperature, so the kelvin difference is carried as a plain scalar here
    // and the product is rebuilt as a `Power`; every other factor stays typed.
    let k_eff = zbs_effective_conductivity(temperature).get::<watt_per_meter_kelvin>();
    let area = superficial_area().get::<square_meter>();
    let height = core_mean_height().get::<meter>();
    Power::new::<watt>(k_eff * area * delta_t_kelvin / height)
}

// ---------------------------------------------------------------------------
// Correlations shared by every fidelity tier (moved above the removed
// PebbleBedCore section on 2026-08-17; see this file's module doc comment)
// ---------------------------------------------------------------------------

/// Wakao packed-bed particle-to-fluid Nusselt number,
/// `Nu = 2 + 1.1 Re_p^0.6 Pr^(1/3)` (dimensionless).
///
/// Source: Wakao, N., Kaguei, S., & Funazkri, T. (1979), "Effect of fluid
/// dispersion coefficients on particle-to-fluid heat transfer coefficients in
/// packed beds". The same correlation is implemented and verified against the
/// paper in this workspace at
/// `tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations`;
/// it is re-evaluated here rather than called because TUAS's form is bound to
/// its own fluid-array plumbing.
///
/// The additive `2` is the conduction limit of a sphere in stagnant fluid, so
/// the correlation degrades gracefully to pure conduction as the flow stops --
/// which is what keeps a tripped circulator physical rather than adiabatic.
pub fn wakao_nusselt(reynolds: f64, prandtl: f64) -> f64 {
    2.0 + 1.1 * reynolds.max(0.0).powf(HTC_FLOW_EXPONENT) * prandtl.max(0.0).cbrt()
}

/// The published HTR-10 fuel element, resolved: a fuelled zone of 5.0 cm
/// diameter carrying 8335 TRISO particles, inside a 6.0 cm ball with a 5 mm
/// unfuelled graphite shell.
///
/// Reused wholesale from [`tampines::pebble_bed::pebble`] rather than
/// re-entered here. `tampines` is already a dependency of this crate, that
/// module transcribes the geometry from IAEA-TECDOC-1382 part 2 Chapter 4,
/// and its conductivities come from `tuas_boussinesq_solver`'s A3 graphite
/// correlation. A second copy of any of that here would be a copy that drifts.
///
/// **CHANGED 2026-09-28 (gh:#350, gh:#351): the high-temperature window.**
/// ~~`Pebble::htr10()`~~ — now [`Pebble::htr10_high_temperature`], which is
/// bit-identical at or below 2000 K and keeps resolving to 3000 K with the
/// matrix graphite from `tuas`'s `NuclearGraphiteMatrixA3HighTemp`. So the
/// resolved pebble, the fuel-to-bed coupling and the kernel/SiC temperatures
/// no longer drop out at 2000 K. **Above 2000 K every conductivity in the
/// stack is extrapolated** (see `tampines::pebble_bed::triso::CorrelationWindow`);
/// the energy balance and the feedback stay continuous, but a fuel temperature
/// read above 2000 K is not a validated number.
pub fn resolved_pebble() -> Pebble {
    Pebble::htr10_high_temperature()
}

// ---------------------------------------------------------------------------
// The fuel node -- TRISO kernels and coatings (2026-09-28, gh:#360)
// ---------------------------------------------------------------------------

/// The TRISO coated particles of the whole core, as one lumped **fuel node**
/// heat capacity \[J/K\] at fuel temperature `fuel_temperature`.
///
/// ```text
/// C_fuel = N_pebbles N_particles sum_layers rho_i V_i c_p,i(T_fuel)
/// ```
///
/// Every input is published or comes from a workspace property library:
///
/// | Layer | Volume | Density | `c_p` |
/// |---|---|---|---|
/// | UO2 kernel | radius 250 um | 10.4 g/cm^3 ([`htr10_uranium_dioxide_density`]) | OFFBEAT `MatproUo2` (MATPRO-v11), 300-3113 K |
/// | buffer | 250-340 um | 1.1 g/cm^3 (particle field) | graphite, [`PEBBLE_GRAPHITE`] |
/// | IPyC | 340-380 um | 1.9 g/cm^3 (particle field) | graphite, [`PEBBLE_GRAPHITE`] |
/// | SiC | 380-415 um | 3.18 g/cm^3 ([`htr10_silicon_carbide_density`]) | OFFBEAT `SneadSiC` (Snead et al. 2007), 200-2400 K |
/// | OPyC | 415-455 um | 1.9 g/cm^3 (particle field) | graphite, [`PEBBLE_GRAPHITE`] |
///
/// Geometry and densities: IAEA-TECDOC-1382 part 2, Table 4-17, via
/// `tampines`. 27 000 pebbles x 8335 particles.
///
/// **Assumptions, stated.** (1) Pyrolytic carbon and the porous buffer carry
/// graphite's specific heat per unit mass — cp is phonon-dominated and
/// insensitive to microstructure, the same argument `tuas` makes for treating
/// graphite cp as grade-insensitive; the porosity enters through the density.
/// (2) All five layers are evaluated at the one fuel temperature: the whole
/// particle spans ~8 K at core-average power, and the layers equilibrate with
/// the kernel in tens of milliseconds (particle diffusion time
/// `r^2 / alpha ~ (0.46 mm)^2 / 1e-5 m^2/s ~ 0.02 s`), against a 1 ms kinetics
/// substep that does resolve it and a 0.1 s plant step that does not need to.
/// (3) **Above 2400 K the SiC cp is held at its 2400 K value** (OFFBEAT's
/// `value` clamps to the stated window rather than extrapolating); SiC is
/// ~22 % of this capacity and its cp is nearly flat there, and SiC
/// decomposes in that regime in any case.
///
/// **Magnitude (2026-09-28):** see
/// `tests::the_fuel_node_capacity_is_the_particles_and_nothing_else`: about
/// 0.27 MJ/K at 950 K, ~3 % of the bed's graphite.
///
/// # Panics
///
/// Outside 300-3000 K (the graphite and UO2 windows), per the stale-state
/// policy stated on [`graphite_specific_heat`].
pub fn fuel_node_heat_capacity(fuel_temperature: ThermodynamicTemperature) -> HeatCapacity {
    let pebble = resolved_pebble();
    let particle = pebble.particle;
    let shell = |r_in: Length, r_out: Length| -> Volume {
        (r_out * r_out * r_out - r_in * r_in * r_in)
            * Ratio::new::<ratio>(4.0 * std::f64::consts::PI / 3.0)
    };
    let zero = Length::new::<meter>(0.0);
    let t_k = fuel_temperature.get::<kelvin>();

    let cp_uo2 = HeatCapacityModel::MatproUo2
        .value_checked(&MaterialState::fresh(t_k))
        .unwrap_or_else(|e| panic!("UO2 kernel cp at {t_k} K: {e:?}"));
    // Clamped at 2400 K above its stated window -- see the doc comment.
    let cp_sic = HeatCapacityModel::SneadSiC.value(&MaterialState::fresh(t_k));
    let cp_carbon = graphite_specific_heat(fuel_temperature).get::<joule_per_kilogram_kelvin>();

    let kg = |v: Volume, rho: MassDensity| (v * rho).get::<kilogram>();
    let per_particle_j_per_k = kg(
        shell(zero, particle.kernel_radius),
        htr10_uranium_dioxide_density(),
    ) * cp_uo2
        + kg(
            shell(particle.kernel_radius, particle.buffer_outer_radius),
            particle.buffer_density,
        ) * cp_carbon
        + kg(
            shell(
                particle.buffer_outer_radius,
                particle.inner_pyc_outer_radius,
            ),
            particle.pyrocarbon_density,
        ) * cp_carbon
        + kg(
            shell(
                particle.inner_pyc_outer_radius,
                particle.silicon_carbide_outer_radius,
            ),
            htr10_silicon_carbide_density(),
        ) * cp_sic
        + kg(
            shell(
                particle.silicon_carbide_outer_radius,
                particle.outer_pyc_outer_radius,
            ),
            particle.pyrocarbon_density,
        ) * cp_carbon;

    HeatCapacity::new::<joule_per_kelvin>(
        per_particle_j_per_k * pebble.particles_per_pebble * pebble_count(),
    )
}

/// The temperatures of the fuel stack a downstream model needs, each placed
/// by [`FuelBedCoupling::stack`] on the line between the bed and the fuel
/// node (2026-09-28, gh:#360).
///
/// All are **core-average-pebble** temperatures (one bed node: no peaking
/// factor, no axial shape, no burnup).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelStackTemperatures {
    /// Inventory-averaged kernel temperature -- the fuel node itself. What
    /// TRISO-ATOPS's per-node fuel temperature and the Doppler feedback want.
    pub kernel: ThermodynamicTemperature,
    /// Mean temperature of the average particle's SiC layer -- governs silver
    /// breakthrough and SiC pressure-vessel failure.
    pub silicon_carbide: ThermodynamicTemperature,
    /// Fuelled-zone matrix graphite mean -- the diffusion path TRISO-ATOPS's
    /// graphite hold-up term describes (the 5 mm unfuelled shell is cooler
    /// and carries no fission products of its own).
    pub fuelled_zone_matrix: ThermodynamicTemperature,
    /// Hottest kernel centre (hottest particle at the pebble centre) -- for
    /// limits and display, not for an inventory-weighted release.
    ///
    /// **A linear placement, not a property evaluation:** it scales the
    /// steady profile's peak-to-average ratio with the fuel node's offset, so
    /// in a large prompt burst it can read above the 3000 K window and past
    /// UO2 melting (measured 4324.9 K in the gh:#351 +12.97 $ excursion,
    /// `physics::tests::the_gh351_excursion_runs_through_2000_k_with_a_resolved_fuel_stack`)
    /// without anything refusing. Treat any value above ~3000 K as "beyond
    /// the model", not as a temperature.
    pub peak_kernel: ThermodynamicTemperature,
}

/// How the fuel node couples to the bed, read off one resolved pebble solve.
///
/// # The two-node picture this belongs to (gh:#360, maintainer direction 2026-09-28)
///
/// ```text
///  f_prompt P + P_decay
///          |
///          v
///   [FUEL NODE T_fuel] --(T_fuel - T_bed)/R--> [BED T_bed] --hA--> [helium] --> loop
///   kernels + coatings                         matrix+shell    \
///   C_fuel ~0.27 MJ/K                          C_bed ~8.9 MJ/K  +--(ZBS conduction+radiation)--> [reflector] --> RPV --> RCCS
/// ```
///
/// `T_fuel` is the **inventory-averaged kernel temperature** — every kernel
/// weighted equally, which is what both the Doppler feedback (a whole-core
/// fuel temperature) and TRISO-ATOPS's per-node fuel temperature want. The
/// bed is the ball's volume-average graphite temperature.
///
/// # Where `R` comes from — derived, not fitted
///
/// At steady state the average kernel sits above the ball's volume average by
///
/// ```text
/// (<T_matrix>_fuelled zone - <T>_ball) + (<T_kernel> - T_particle surface)
/// ```
///
/// — the fuelled-zone matrix runs hotter than the whole-ball average because
/// the unfuelled 5 mm shell is cooler, and each particle adds its own internal
/// rise. The fuelled-zone mean of the parabolic profile is `T_a + (2/5)(T_0 - T_a)`
/// (the same 2/5 `tampines::pebble_bed::pebble::Pebble::volume_average_temperature`
/// derives), and the kernel's own volume mean above the particle surface is
/// `(T_ks - T_s) + (2/5)(T_kc - T_ks)`. Dividing by the pebble's power gives a
/// per-pebble resistance; over 27 000 pebbles in parallel, the core value.
///
/// The fractions below place the other layers on the same line, so any
/// temperature in the stack follows the dynamic fuel node rather than a
/// separate quasi-static solve:
///
/// ```text
/// T_x = T_bed + fraction_x (T_fuel - T_bed)
/// ```
///
/// which at steady state reproduces the resolved profile exactly and in a
/// transient scales the whole stack with the heat actually leaving the fuel.
///
/// # What it assumes
///
/// Two nodes, so the matrix's own heat capacity is all in the bed: after a
/// power step the kernel offset reaches its new steady value on the fuel
/// node's own time constant (`R C_fuel`, a fraction of a second), whereas the
/// real fuelled-zone matrix profile develops over the pebble's conduction time
/// (`a^2/alpha ~ 60 s`). The fuel temperature therefore responds somewhat
/// **faster** to a power change than a fully resolved pebble would — the same
/// approximation the retired kernel Doppler channel made with its
/// instantaneous `R P`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelBedCoupling {
    /// Core-level conduction resistance from the inventory-averaged kernel to
    /// the bed's volume average \[K/W of the core's fuel-to-bed heat flow\].
    pub resistance: ThermalResistance,
    /// Where the **fuelled-zone matrix** mean sits on the fuel-bed line
    /// (dimensionless, in `(0, 1)`). The graphite temperature TRISO-ATOPS's
    /// matrix hold-up wants.
    pub fuelled_zone_matrix_fraction: f64,
    /// Where the average particle's **SiC layer** mean sits (in `(0, 1)`) —
    /// the temperature that governs silver breakthrough and SiC failure.
    pub silicon_carbide_fraction: f64,
    /// Where the **peak kernel centre** (hottest particle, pebble centre)
    /// sits — above 1, since it is hotter than the inventory average. For
    /// limits and display.
    pub peak_kernel_fraction: f64,
}

impl FuelBedCoupling {
    /// Read the coupling off a resolved `profile` whose volume average is the
    /// bed temperature, solved at per-pebble power `pebble_power`. `None` when
    /// the power is too small for the quotient to mean anything or the rise is
    /// not positive.
    pub fn from_profile(profile: &PebbleTemperatureProfile, pebble_power: Power) -> Option<Self> {
        let pebble = resolved_pebble();
        let power_w = pebble_power.get::<watt>();
        if !(power_w > 0.0) {
            return None;
        }
        let k = |t: ThermodynamicTemperature| t.get::<kelvin>();
        let ball_average = k(pebble.volume_average_temperature(profile));
        let t_a = k(profile.fuelled_zone_boundary);
        let fuelled_zone_mean = t_a + 0.4 * (k(profile.centre) - t_a);
        let particle = profile.hottest_particle;
        let t_s = k(particle.particle_surface);
        let kernel_rise = (k(particle.kernel_surface) - t_s)
            + 0.4 * (k(particle.kernel_centre) - k(particle.kernel_surface));
        let sic_rise =
            0.5 * (k(particle.inner_pyc_outer) + k(particle.silicon_carbide_outer)) - t_s;

        let matrix_offset = fuelled_zone_mean - ball_average;
        let total = matrix_offset + kernel_rise;
        if !(total.is_finite() && total > 0.0) {
            return None;
        }
        let per_pebble_k_per_w = total / power_w;
        Some(Self {
            resistance: ThermalResistance::new::<kelvin_per_watt>(
                per_pebble_k_per_w / pebble_count(),
            ),
            fuelled_zone_matrix_fraction: matrix_offset / total,
            silicon_carbide_fraction: (matrix_offset + sic_rise) / total,
            peak_kernel_fraction: (k(profile.peak_kernel_centre) - ball_average) / total,
        })
    }

    /// The design-point coupling: the resolved pebble at the bed's seed
    /// temperature and the core-average pebble power. Used to set the fuel
    /// node's reference temperature and initial state so the plant opens at
    /// its design point with the fuel already sitting its steady offset above
    /// the bed.
    ///
    /// # Panics
    ///
    /// If the design point does not resolve — a construction-time defect, not a
    /// transient excursion.
    pub fn at_design_point() -> Self {
        let design_bed = ThermodynamicTemperature::new::<kelvin>(SEED_BED_TEMPERATURE_K);
        let power = core_average_pebble_power();
        resolved_pebble_profile(design_bed, power)
            .as_ref()
            .and_then(|p| Self::from_profile(p, power))
            .expect("the HTR-10 design point must resolve a fuel-to-bed coupling")
    }

    /// Every temperature of the fuel stack, placed on the fuel-bed line
    /// between the bed node `bed` and the fuel node `fuel`. See
    /// [`FuelStackTemperatures`].
    pub fn stack(
        &self,
        bed: ThermodynamicTemperature,
        fuel: ThermodynamicTemperature,
    ) -> FuelStackTemperatures {
        FuelStackTemperatures {
            kernel: fuel,
            silicon_carbide: Self::interpolate(self.silicon_carbide_fraction, bed, fuel),
            fuelled_zone_matrix: Self::interpolate(self.fuelled_zone_matrix_fraction, bed, fuel),
            peak_kernel: Self::interpolate(self.peak_kernel_fraction, bed, fuel),
        }
    }

    /// A temperature at `fraction` of the way from `bed` to `fuel`.
    pub fn interpolate(
        fraction: f64,
        bed: ThermodynamicTemperature,
        fuel: ThermodynamicTemperature,
    ) -> ThermodynamicTemperature {
        let b = bed.get::<kelvin>();
        ThermodynamicTemperature::new::<kelvin>(b + fraction * (fuel.get::<kelvin>() - b))
    }
}

/// Core-average power of one fuel element \[W\]: rated thermal power over the
/// published pebble count. 10 MW / 27 000 = 370.37 W.
pub fn core_average_pebble_power() -> Power {
    design().thermal_power / pebble_count()
}

/// Intra-pebble conduction coefficient \[W/(m^2 K)\], referred to the pebble
/// surface area, from the **resolved two-zone pebble** at `surface_temperature`
/// and pebble power `pebble_power`.
///
/// ## What changed on 2026-09-22, and why it is not just a constant swap
///
/// This used to be `h = 10 k / d` with an invented `k = 25 W/(m K)` — the
/// closed-form volume-average-to-surface result for a sphere generating
/// **uniformly right out to its own surface**. An HTR-10 ball does not: heat
/// is released only inside the 5.0 cm fuelled zone, and the 5 mm unfuelled
/// shell conducts the whole of it with none of its own. That shell is a
/// resistance the uniform-ball form has no way to represent, whatever `k` is
/// set to, so this was a **geometry** error wearing a coefficient's clothes.
///
/// It now solves [`tampines::pebble_bed::pebble::Pebble::steady_state_temperatures`]
/// — two-zone conduction with temperature- and fluence-dependent A3 graphite,
/// the TRISO dispersion lowering the fuelled zone's conductivity below plain
/// graphite — takes the ball's **volume average** (which is what this node's
/// capacitance makes it hold, see
/// [`tampines::pebble_bed::pebble::Pebble::volume_average_temperature`]), and
/// forms the conductance that reproduces it:
///
/// ```text
/// h_int = Q_pebble / (A_pebble (T_avg - T_surface))
/// ```
///
/// **The particle-scale rise is deliberately NOT in here.** The 34.25 K
/// kernel rise `tampines` reports is the *hottest* particle, at the pebble
/// centre, and it is a fuel temperature for feedback and limits — not a term
/// in the ball's averaged heat path. Folding it into this conductance would
/// inflate the resistance by treating a peak as a mean. See
/// [`PebbleBedPorousMediaNode::peak_kernel_temperature`] for where it does
/// belong.
///
/// ## Arguments
///
/// `node_temperature` is this node's own temperature, i.e. the ball's volume
/// average. [`resolved_pebble_profile`] **inverts** for the pebble surface
/// that produces it rather than treating the two as interchangeable — see
/// that function for why, and for the measurement that decided it.
///
/// `pebble_power` is floored at 1 % of core-average. In the linear-conduction
/// limit the conductance is power-independent — `T_avg - T_surface` scales
/// with `Q`, so the ratio does not — and the floor exists only to keep the
/// zero-power case from forming `0/0` rather than to model anything.
///
/// Fluence is **zero**: this simulator has no burnup and no multi-pass pebble
/// flow, so there is no fluence to supply. Irradiated A3 graphite is
/// substantially less conductive, so a real mid-life core sits on the
/// resistive side of this.
///
/// ## Failure
///
/// ~~Outside `tampines`' 300-2000 K correlation window, or if either
/// fixed-point iteration fails, this falls back to the retired uniform-ball
/// form and says so in the returned value's provenance only by being finite —
/// the caller cannot tell. That is deliberate: a plant step must not panic on
/// a transient excursion, and the fallback is the *old* model, which was
/// usable.~~ **CHANGED 2026-09-28 (maintainer direction: graphite
/// conductivity "wired correctly", fail loud rather than fall back).** The
/// silent fallback to an invented `k = 25 W/(m K)` is gone: outside the
/// high-temperature pebble's 300-3000 K window, or on a non-converged solve,
/// this **panics** with the reason `tampines` gave (see
/// [`resolved_pebble_profile_checked`]) — the same policy as
/// [`graphite_specific_heat`]. Below 3000 K there is no longer any excursion
/// that reaches it; above 3000 K there is no property set to fall back to.
pub fn intra_pebble_conduction_coefficient(
    node_temperature: ThermodynamicTemperature,
    pebble_power: Power,
) -> HeatTransfer {
    let power = floored_pebble_power(pebble_power);
    let profile = resolved_pebble_profile_checked(node_temperature, power)
        .unwrap_or_else(|reason| panic!("{reason}"));
    conduction_coefficient_from(Some(&profile), power)
}

/// Pebble power floored at 1 % of core-average -- see
/// [`intra_pebble_conduction_coefficient`] for why the floor is a guard on
/// `0/0` and not a physical model.
fn floored_pebble_power(pebble_power: Power) -> Power {
    let floor = core_average_pebble_power() * 0.01;
    if pebble_power > floor {
        pebble_power
    } else {
        floor
    }
}

/// Solve the resolved two-zone pebble for the profile whose **volume average
/// is `node_temperature`**, at pebble power `pebble_power`.
///
/// ## Why this inverts instead of just calling `tampines`
///
/// [`tampines::pebble_bed::pebble::Pebble::steady_state_temperatures`] takes
/// the pebble **surface** temperature as its boundary condition. This node
/// does not hold that — its capacitance is the whole graphite mass, so what it
/// holds is the ball's volume average, which sits ~9 K above the surface at
/// rated power.
///
/// Handing the node temperature straight in as the surface would anchor the
/// whole profile ~9 K too hot. **That was measured, not assumed:** the first
/// cut of this wiring did exactly that, and
/// [`tests::the_resolved_pebble_beats_the_uniform_ball`] found it moved
/// `h_int` by 1.14 % over a 15 K offset — small, but an error with a known
/// sign and no reason to keep. Documenting a bound for it was the tempting
/// move; removing it costs two extra fixed-point passes.
///
/// So the surface is solved for: start at `T_s = T_node`, solve, measure the
/// volume-average rise `r`, set `T_s <- T_node - r`, repeat. ~~The rise
/// depends on the surface only through `k(T)`, so this contracts hard and
/// converges in two or three passes; 1e-6 K is the tolerance and 12 passes the
/// cap.~~ **CORRECTED 2026-09-28** -- true at core-average power, false in a
/// prompt excursion: at 14.4 kW/pebble (measured on the plant's opening
/// transient) the rise is ~370 K and the fixed point contracts by only ~0.3 per
/// pass, oscillating, so it needed ~17 passes and the 12-pass cap returned
/// `None` -- which the bed step silently turned into the invented
/// `k = 25 W/(m K)` fallback. It is now one fixed-point pass followed by
/// secant iteration on the residual `T_node - r(T_s) - T_s` (tolerance 1e-6 K,
/// cap 60 passes), in [`resolved_pebble_profile_checked`].
///
/// The returned profile's `surface` field is therefore the **true** pebble
/// surface, and `volume_average_temperature` of it reproduces `node_temperature`
/// to within the tolerance — which
/// [`tests::the_inverted_profile_reproduces_the_node_temperature`] checks.
///
/// ## Failure
///
/// `None` when `tampines` rejects the inputs (outside its 300-2000 K
/// correlation window) or either iteration fails to converge. That is a real
/// outcome, not an error to unwrap: a plant step must not panic because a
/// transient took the bed briefly out of range. Callers fall back to the
/// retired uniform-ball form, which is the model this replaced and was usable.
pub fn resolved_pebble_profile(
    node_temperature: ThermodynamicTemperature,
    pebble_power: Power,
) -> Option<PebbleTemperatureProfile> {
    resolved_pebble_profile_checked(node_temperature, pebble_power).ok()
}

/// [`resolved_pebble_profile`] **with the reason** when it fails — the fix
/// gh:#350 asked for first ("surface it": `.ok()?` was throwing away a
/// specific out-of-range message). The bed step and the intra-pebble
/// coefficient use this and panic with the message rather than substitute
/// anything.
///
/// Fluence is **zero** in both solves: this simulator has no burnup, so
/// the unirradiated conductivity is used. Irradiated graphite and PyC conduct
/// substantially worse, so at burnup this **over-states k and under-states the
/// kernel temperature** — a known simplification (gh:#361 tracks threading a
/// fluence through).
pub fn resolved_pebble_profile_checked(
    node_temperature: ThermodynamicTemperature,
    pebble_power: Power,
) -> Result<PebbleTemperatureProfile, String> {
    const TOLERANCE_K: f64 = 1.0e-6;
    const MAX_PASSES: usize = 60;

    let pebble = resolved_pebble();
    let power = floored_pebble_power(pebble_power);
    let node_k = node_temperature.get::<kelvin>();
    let solve = |surface_k: f64| {
        pebble
            .steady_state_temperatures(
                power,
                ThermodynamicTemperature::new::<kelvin>(surface_k),
                Ratio::new::<ratio>(0.0),
            )
            .map_err(|e| {
                format!(
                    "resolved pebble at node {node_k} K, surface {surface_k} K, pebble power {} W: {e:?}",
                    power.get::<watt>()
                )
            })
    };
    // Residual of the inversion: f(T_s) = T_node - rise(T_s) - T_s, zero when
    // the profile's volume average is the node temperature.
    let residual = |surface_k: f64| -> Result<(f64, PebbleTemperatureProfile), String> {
        let profile = solve(surface_k)?;
        let rise = pebble.volume_average_temperature(&profile).get::<kelvin>() - surface_k;
        if !rise.is_finite() {
            return Err(format!(
                "resolved pebble at node {node_k} K: non-finite rise {rise}"
            ));
        }
        Ok((node_k - rise - surface_k, profile))
    };

    // ~~Plain fixed-point iteration T_s <- T_node - rise(T_s), 12 passes.~~
    // CHANGED 2026-09-28: the fixed point contracts only by |d rise/d T_s|,
    // which is ~0.3 (oscillating) at the ~14 kW/pebble a prompt excursion
    // conducts -- it needs ~17 passes there, and the old 12-pass cap then
    // returned `None`, which the bed step silently turned into the invented
    // `k = 25 W/(m K)` fallback. Found the day that fallback was made to fail
    // loud. Now: a linear starting estimate, one fixed-point pass to get a
    // second point, then secant on the residual, which converges in a handful
    // of passes at any power.
    // Starting point: the LINEAR estimate `T_s = T_node - (r/P)_low P`, with
    // the rise per watt taken from a 1 %-of-core-average solve at the node
    // temperature (always inside the window). Starting at `T_s = T_node`
    // instead puts the whole profile one rise too hot, which at excursion
    // powers (~20 kW/pebble) pushes the trial kernel past the 3000 K window
    // even though the converged profile sits well inside it.
    let low_power = core_average_pebble_power() * 0.01;
    let low_profile = pebble
        .steady_state_temperatures(low_power, node_temperature, Ratio::new::<ratio>(0.0))
        .map_err(|e| format!("resolved pebble at node {node_k} K, low-power start: {e:?}"))?;
    let rise_per_watt = (pebble
        .volume_average_temperature(&low_profile)
        .get::<kelvin>()
        - node_k)
        / low_power.get::<watt>();
    let mut s0 = node_k - rise_per_watt * power.get::<watt>();
    let (mut f0, mut p0) = residual(s0)?;
    if f0.abs() < TOLERANCE_K {
        return Ok(p0);
    }
    let mut s1 = s0 + f0;
    for _ in 0..MAX_PASSES {
        let (f1, p1) = residual(s1)?;
        if f1.abs() < TOLERANCE_K {
            return Ok(p1);
        }
        let denominator = f1 - f0;
        let next = if denominator.abs() > 1e-12 {
            s1 - f1 * (s1 - s0) / denominator
        } else {
            s1 + f1
        };
        s0 = s1;
        f0 = f1;
        p0 = p1;
        s1 = next;
    }
    let _ = p0;
    Err(format!(
        "resolved pebble at node {node_k} K: surface inversion did not converge in \
         {MAX_PASSES} secant passes (pebble power {} W, last surface {s1} K)",
        power.get::<watt>()
    ))
}

/// The conduction coefficient implied by an already-solved profile, so a
/// caller that needs the profile anyway does not solve it twice.
///
/// ~~Falls back to the retired uniform-ball `10 k / d` when there is no
/// profile.~~ **CHANGED 2026-09-28** — panics instead (see
/// [`intra_pebble_conduction_coefficient`]): the fallback put an invented
/// `k = 25 W/(m K)` into the heat path silently, exactly where the real
/// correlation had refused. [`LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K`]
/// survives only for the before/after tests.
///
/// # Panics
///
/// When `profile` is `None` or its volume-average rise is not positive.
pub fn conduction_coefficient_from(
    profile: Option<&PebbleTemperatureProfile>,
    pebble_power: Power,
) -> HeatTransfer {
    let power = floored_pebble_power(pebble_power);
    let rise = profile
        .map(|p| {
            let average = resolved_pebble().volume_average_temperature(p);
            average.get::<kelvin>() - p.surface.get::<kelvin>()
        })
        .filter(|rise| rise.is_finite() && *rise > 0.0);

    match rise {
        Some(rise) => {
            let area_one_pebble = heat_transfer_area().get::<square_meter>() / pebble_count();
            HeatTransfer::new::<watt_per_square_meter_kelvin>(
                power.get::<watt>() / (area_one_pebble * rise),
            )
        }
        None => panic!(
            "intra-pebble conduction: no resolved pebble profile with a positive \
             volume-average rise (pebble power {:?}); the high-temperature pebble's \
             300-3000 K correlation window was left or the solve did not converge -- \
             no fallback conductivity is substituted (2026-09-28)",
            power
        ),
    }
}

/// Overall pebble-to-helium heat-transfer coefficient at helium flow `m_dot`
/// and bulk temperature `helium_temperature`, as **two resistances in series**.
///
/// ```text
/// 1/U = 1/h_film + 1/h_internal
/// ```
///
/// - `h_film` is the evaluated [`wakao_nusselt`] correlation,
///   `h = Nu k_He / d_p`, with the helium Reynolds, Prandtl and conductivity
///   taken from the **real** CoolProp-derived helium EOS and transport models
///   at the loop pressure. The Reynolds number is the superficial packed-bed
///   form and is built with the workspace's own
///   [`outram_park_digital_twin_engine::htr10::kta`] helpers, so it is the same
///   Reynolds number the KTA pressure-drop correlation uses.
/// - `h_internal` is [`intra_pebble_conduction_coefficient`], evaluated on the
///   **resolved two-zone pebble** at the bed's own temperature and the current
///   per-pebble power. It is the smaller resistance by far (~12 % at rated
///   flow), which is why the film's correctness was the thing worth getting
///   right first and why resolving the pebble moves the total only a few
///   percent — see [`tests::the_resolved_pebble_beats_the_uniform_ball`].
///
/// **This replaced an invented lumped constant on 2026-08-14** (see
/// [`LEGACY_LUMPED_HTC_W_PER_M2_K`]). The film resistance dominates at rated
/// flow, which is why the old flow-scaling shape was roughly the right *shape*
/// while being the wrong *magnitude*.
///
/// The flow is floored at 1% of nominal rather than zero. Note this floor now
/// matters far less than it did: with the correlation evaluated, the Wakao
/// additive `2` already supplies the stagnant-fluid conduction limit, so a
/// stopped circulator leaves a real (small) coefficient rather than a
/// arbitrarily scaled one.
pub fn overall_htc_at_flow(
    helium_mass_flow: MassRate,
    helium_temperature: ThermodynamicTemperature,
    bed_temperature: ThermodynamicTemperature,
    reactor_thermal_power: Power,
) -> HeatTransfer {
    let h_film = film_htc_at_flow(helium_mass_flow, helium_temperature);
    let h_internal = intra_pebble_conduction_coefficient(
        bed_temperature,
        reactor_thermal_power / pebble_count(),
    );
    series_coefficient(h_film, h_internal)
}

/// The **convective film** leg alone \[W/(m^2 K)\]: `h = Nu k_He / d_p` with
/// [`wakao_nusselt`] on the superficial packed-bed Reynolds number.
///
/// Split out of [`overall_htc_at_flow`] on 2026-09-22 so that
/// [`PebbleBedPorousMediaNode::step`] can compose the two legs itself and pay
/// for the resolved pebble's fixed-point solve **once** per step rather than
/// twice -- once for the coefficient and again for the kernel temperature.
/// Both entry points call the same two functions, so they cannot disagree.
pub fn film_htc_at_flow(
    helium_mass_flow: MassRate,
    helium_temperature: ThermodynamicTemperature,
) -> HeatTransfer {
    let d_p = pebble_diameter().get::<meter>();
    let (k_he, prandtl, viscosity_pa_s) = helium_transport(helium_temperature);

    let flow_floor = nominal_helium_flow_kg_per_s() * 0.01;
    let m_dot = MassRate::new::<kilogram_per_second>(
        helium_mass_flow
            .get::<kilogram_per_second>()
            .abs()
            .max(flow_floor),
    );

    let mass_flux = kta::superficial_mass_flux(m_dot, superficial_area());
    let reynolds = kta::packed_bed_reynolds(
        mass_flux,
        pebble_diameter(),
        DynamicViscosity::new::<pascal_second>(viscosity_pa_s),
    )
    .get::<ratio>();

    HeatTransfer::new::<watt_per_square_meter_kelvin>(wakao_nusselt(reynolds, prandtl) * k_he / d_p)
}

/// Two surface coefficients in series, `1/U = 1/h1 + 1/h2`.
///
/// Guards the degenerate case rather than dividing by a zero coefficient -- a
/// non-finite conductance would silently poison the whole energy balance
/// downstream.
///
/// Named `film`/`internal` rather than `first`/`second`: `uom::si::time::second`
/// is in scope here, and a parameter called `second` is silently taken as that
/// unit struct instead of a binding.
pub fn series_coefficient(film: HeatTransfer, internal: HeatTransfer) -> HeatTransfer {
    let a = film.get::<watt_per_square_meter_kelvin>();
    let b = internal.get::<watt_per_square_meter_kelvin>();
    if !(a > 0.0) || !(b > 0.0) {
        return HeatTransfer::new::<watt_per_square_meter_kelvin>(0.0);
    }
    HeatTransfer::new::<watt_per_square_meter_kelvin>(1.0 / (1.0 / a + 1.0 / b))
}

/// Helium isobaric specific heat \[J/(kg K)\] at `temperature` and the
/// primary-loop pressure, from the real CoolProp-derived helium EOS.
///
/// A general helium property helper, not specific to any one fidelity tier's
/// closure. Falls back to the ideal-gas-limit helium value if the density
/// solve declines, for the same reason [`helium_transport`] does.
///
/// **Formerly also used to form the capacity rate `m_dot c_p` for
/// `effective_conductance`, the effectiveness-NTU conductance the removed
/// `PebbleBedCore` closure needed** (`G_eff = m_dot c_p (1 - exp(-NTU))`,
/// bounding `T_out < T_bed` for every finite NTU -- see this file's module
/// doc comment "History" note, and the GitHub issue #22 comment recording
/// the full derivation). That helper was removed with `PebbleBedCore` on
/// 2026-08-17 since nothing else called it; this function survives because
/// it is still a general-purpose property lookup.
pub fn helium_specific_heat(temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    /// Ideal-gas-limit helium `c_p` \[J/(kg K)\].
    const IDEAL_CP: f64 = 5193.0;
    let t = temperature.get::<kelvin>();
    if !(t.is_finite() && t > 1.0) {
        return SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(IDEAL_CP);
    }
    let pressure_pa = design().primary_pressure.get::<uom::si::pressure::pascal>();
    match state_pt(Fluid::Helium, t, pressure_pa) {
        Ok(state) if state.cp.is_finite() && state.cp > 0.0 => {
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(state.cp)
        }
        _ => SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(IDEAL_CP),
    }
}

/// Specific enthalpy of helium at `temperature` and the primary pressure
/// \[J/kg\], from the CoolProp-fork Helmholtz EOS (Ortiz-Vega et al.).
///
/// **The one helium `h(T)` for the whole primary circuit** (gh:#393,
/// 2026-09-29): the bed's fluid row, the primary loop's hot-duct and
/// cold-return CVs and the steam generator's helium array all sit on this
/// EOS and datum, so an enthalpy handed from one CV to the next means the
/// same energy on both sides. Used to seed states and to convert prescribed
/// boundary temperatures; the balances themselves carry enthalpy.
///
/// # Panics
///
/// If the `(p, T)` flash fails -- the stale-state policy: no fallback
/// number stands in for an EOS that declined.
pub fn helium_enthalpy_at(temperature: ThermodynamicTemperature) -> AvailableEnergy {
    let pressure_pa = design().primary_pressure.get::<uom::si::pressure::pascal>();
    let state = state_pt(Fluid::Helium, temperature.get::<kelvin>(), pressure_pa)
        .unwrap_or_else(|e| panic!("helium (p,T) flash at {temperature:?}: {e:?}"));
    AvailableEnergy::new::<joule_per_kilogram>(state.enthalpy)
}

/// Helium state at specific enthalpy `enthalpy` and the primary pressure --
/// the **backward** `(p, h)` flash ([`outram_park_fork_coolprop::state_ph`])
/// that turns an integrated enthalpy back into a temperature, density and
/// `c_p`. The inverse of [`helium_enthalpy_at`] on the same EOS, so
/// `h -> T -> h` closes to the flash tolerance (`dT < 1e-11 T`).
///
/// # Panics
///
/// If the flash fails, for the reason [`helium_enthalpy_at`] gives.
pub fn helium_state_at(enthalpy: AvailableEnergy) -> FluidState {
    helium_state_at_seeded(enthalpy, None)
}

/// [`helium_state_at`] with an optional temperature seed \[K\].
///
/// **Why a seed.** [`outram_park_fork_coolprop::state_ph`] always starts its
/// outer Newton at `1.1 T_crit` -- 5.7 K for helium -- and damps each step to
/// 30 %, so reaching an HTGR state takes about 20 `(p, T)` flashes per call.
/// This runs **the same outer Newton** (`dT = -(h(T) - h)/c_p`, converged on
/// `|dT| <= 1e-11 T`, exactly `state_ph`'s `solve_pt_outer` criterion) from a
/// seed instead: the caller's previous iterate when it has one, otherwise an
/// ideal-gas extrapolation from a 300 K reference state. Helium is near-ideal
/// here, so it converges in one or two steps. If it does not converge in 50
/// it falls back to `state_ph` itself, so correctness never rests on the
/// seed. Measured 2026-09-29: the 60 000-step
/// `tests::two_node_balance_settles_with_all_power_leaving_via_helium_throughflow`
/// took 49.8 s with `state_ph` and 2.41 s with this, with the same settled
/// numbers to every printed digit.
///
/// # Panics
///
/// If both the seeded Newton and `state_ph` fail.
pub fn helium_state_at_seeded(enthalpy: AvailableEnergy, seed_k: Option<f64>) -> FluidState {
    let pressure_pa = design().primary_pressure.get::<uom::si::pressure::pascal>();
    let h = enthalpy.get::<joule_per_kilogram>();
    let seed = match seed_k {
        Some(t) if t.is_finite() && t > 2.0 => Some(t),
        _ => state_pt(Fluid::Helium, 300.0, pressure_pa)
            .ok()
            .filter(|r| r.cp > 0.0)
            .map(|r| 300.0 + (h - r.enthalpy) / r.cp)
            .filter(|t| t.is_finite() && *t > 2.0),
    };
    if let Some(mut t) = seed {
        for _ in 0..50 {
            let Ok(state) = state_pt(Fluid::Helium, t, pressure_pa) else {
                break;
            };
            if !(state.cp > 0.0) {
                break;
            }
            let step = -(state.enthalpy - h) / state.cp;
            if step.abs() <= 1.0e-11 * t {
                return state;
            }
            t += step.clamp(-0.3 * t, 0.3 * t);
        }
    }
    state_ph(Fluid::Helium, pressure_pa, h)
        .unwrap_or_else(|e| panic!("helium (p,h) flash at {enthalpy:?}: {e:?}"))
}

/// Helium thermal conductivity \[W/(m K)\], Prandtl number and dynamic
/// viscosity \[Pa s\] at `temperature` and the primary-loop pressure, from the
/// real CoolProp-derived helium models.
///
/// Falls back to representative helium values if a transport model or the
/// density solve declines (the same defensive shape
/// [`super::super::primary_loop`] uses): a GUI frame must not panic on a transient
/// excursion, and helium at these conditions is close enough to ideal that the
/// fallback is a sane bound rather than a fabricated number.
pub fn helium_transport(temperature: ThermodynamicTemperature) -> (f64, f64, f64) {
    /// Representative helium conductivity \[W/(m K)\] near 1000 K, 3 MPa.
    const FALLBACK_CONDUCTIVITY: f64 = 0.35;
    /// Helium Prandtl number is near 0.67 over this whole range.
    const FALLBACK_PRANDTL: f64 = 0.67;
    /// Representative helium dynamic viscosity \[Pa s\] near 1000 K.
    const FALLBACK_VISCOSITY: f64 = 4.5e-5;

    let t = temperature.get::<kelvin>();
    if !(t.is_finite() && t > 1.0) {
        return (FALLBACK_CONDUCTIVITY, FALLBACK_PRANDTL, FALLBACK_VISCOSITY);
    }

    // Read from the SAME published design point the rest of this module
    // derives its geometry from, so there is no second copy of the operating
    // pressure to drift out of step.
    let pressure_pa = design().primary_pressure.get::<uom::si::pressure::pascal>();
    match state_pt(Fluid::Helium, t, pressure_pa) {
        Ok(state) if state.density > 0.0 && state.cp > 0.0 => {
            let mu = viscosity(Fluid::Helium, t, state.density).unwrap_or(FALLBACK_VISCOSITY);
            let k = conductivity(Fluid::Helium, t, state.density).unwrap_or(FALLBACK_CONDUCTIVITY);
            let pr = if k > 0.0 {
                state.cp * mu / k
            } else {
                FALLBACK_PRANDTL
            };
            (k, pr, mu)
        }
        _ => (FALLBACK_CONDUCTIVITY, FALLBACK_PRANDTL, FALLBACK_VISCOSITY),
    }
}

/// Graphite specific enthalpy at `temperature` \[J/kg\], from
/// [`PEBBLE_GRAPHITE`]: the **exact** integral of Butland & Maddison's
/// polynomial cp from `tuas`'s 273.15 K datum.
///
/// ~~"`c_p (T - 298.15 K)` with the constant `GRAPHITE_CP_J_PER_KG_K`"~~
/// **CHANGED 2026-09-28** — only enthalpy *differences* are ever used, so the
/// datum moving from 298.15 K to 273.15 K changes no result.
///
/// A sibling `helium_specific_enthalpy_from_temperature` used to sit here and
/// computed **helium** enthalpy with the **graphite** cp. It had no caller;
/// it was deleted 2026-09-28 rather than left for someone to find.
///
/// # Panics
///
/// Outside 300-3000 K, for the reason [`graphite_specific_heat`] gives.
pub fn pebble_bed_specific_enthalpy_from_temperature(
    temperature: ThermodynamicTemperature,
) -> AvailableEnergy {
    try_get_h(
        Material::Solid(PEBBLE_GRAPHITE),
        temperature,
        property_pressure(),
    )
    .unwrap_or_else(|e| panic!("pebble graphite enthalpy at {temperature:?}: {e:?}"))
}

/// Inverse of [`pebble_bed_specific_enthalpy_from_temperature`], by `tuas`'s
/// Brent-Dekker inversion of the same analytic enthalpy (the old closed form
/// only existed because `c_p` was constant).
pub fn temperature_from_specific_enthalpy(
    specific_enthalpy: AvailableEnergy,
) -> ThermodynamicTemperature {
    try_get_temperature_from_h(
        Material::Solid(PEBBLE_GRAPHITE),
        specific_enthalpy,
        property_pressure(),
    )
    .unwrap_or_else(|e| panic!("pebble graphite temperature from {specific_enthalpy:?}: {e:?}"))
}

// ---------------------------------------------------------------------------
// Implicit two-temperature porous-media node (2026-08-17).
//
// **What this added over the removed `PebbleBedCore` (see this file's module
// doc comment "History" note).** That closure treated the helium row of the
// nodalisation table as `0 (external)`: the bulk mean was a boundary
// condition the caller supplied, and its `step` closed the bed-to-helium
// exchange with a CLOSED-FORM effectiveness-NTU balance rather than
// integrating the helium's own energy equation. That was valid exactly
// because the helium carried no independent thermal inertia in that
// formulation.
//
// This struct gives the helium a real node instead: **one control volume,
// two temperatures** -- the pebble (solid phase) and the helium filling the
// bed's void space (fluid phase). This is the standard two-equation Local
// Thermal Non-Equilibrium (LTNE) porous-media energy formulation (see e.g.
// Kaviany, *Principles of Heat Transfer in Porous Media*, 2nd ed., the
// two-equation model section; Nield & Bejan, *Convection in Porous Media*).
// It is still ONE spatial node -- no axial or radial split, the same
// limitation the module doc comment already states -- but within that one
// node the solid and the fluid are no longer forced into the same
// instantaneous balance: each phase carries its own capacitance and its own
// backward-Euler update, coupled through the interfacial conductance `h A`.
//
// **Why implicit, and why a 2x2 matrix.** The two phases are solved
// SIMULTANEOUSLY at the new time level `n+1`, not one after the other:
// advancing the solid first with the OLD fluid temperature (or vice versa)
// is a fractional-step scheme, and this file has already had to root out
// exactly that kind of sequencing error once -- the removed `PebbleBedCore`
// closure replaced an arithmetic-mean driving temperature for exactly this
// reason (see the module doc comment "History" note and GitHub issue #22).
// Backward Euler on both phases at once is unconditionally stable for any
// `dt`, which matters here because the fluid node's own time constant
// (`C_f / (h A + m_dot c_p)`, seconds) is expected to run orders of
// magnitude faster than the bed's (~184 s) -- an explicit scheme sized for
// the slow phase would be unstable on the fast one. With exactly two
// unknowns the implicit system is a 2x2 linear SOLVE, not an iteration.
//
// **The helium side stores the full thermodynamic state, not a bare
// temperature.** [`PebbleBedPorousMediaNode::helium_state`] is a
// [`FluidState`]: the fluid-phase balance needs density AND `c_p` (for
// `C_f` and `m_dot c_p`) as well as temperature, and reading all three off
// one evaluated state guarantees they are mutually consistent -- rather
// than re-deriving density and `c_p` from a bare Kelvin value with separate
// CoolProp calls that could evaluate at a slightly different point.
// ---------------------------------------------------------------------------

/// Implicit two-temperature (solid + fluid) porous-media node.
///
/// See the module comment immediately above for what this adds over the
/// removed `PebbleBedCore` closure and why the system is implicit; see
/// [`Self::step`]'s doc comment for the full 2x2 backward-Euler derivation
/// the solve below carries out.
///
/// ## Derivation -- the 2x2 backward-Euler system
///
/// **Governing equations (continuous, LTNE two-equation form).**
///
/// Solid (pebble) phase -- no throughflow:
///
/// ```text
/// C_s dT_s/dt = Q_fission - h A (T_s - T_f)
/// ```
///
/// Fluid (helium) phase -- WITH throughflow, the term a helium-as-external
/// closure does not carry:
///
/// ```text
/// C_f dT_f/dt = h A (T_s - T_f) - m_dot c_p (T_f - T_in)
/// ```
///
/// **CHANGED 2026-09-29 (gh:#393): the fluid row below is now written and
/// solved in enthalpy**, `M_f dh_f/dt = h A (T_s - T(h_f)) + m_dot (h_in -
/// h_f)`, with `M_f = rho V_void` ([`helium_void_mass`]) and `T(h)` the
/// CoolProp `(p, h)` inverse -- see [`Self::step`] and
/// [`assemble_backward_euler_system`]. The temperature form is kept below as
/// the derivation's history; with `C_f = M_f c_p` and `m_dot c_p` it is the
/// first-order approximation of the enthalpy form.
///
/// `C_s` is [`bed_heat_capacity`] (unchanged). `C_f` is the thermal mass of
/// the helium actually held in the bed's void space at the current density,
/// `rho_He(T_f, p) * V_void * c_p(T_f)`, with `V_void` this struct's own
/// [`Self`]`::pebble_bed_helium_volume` field (the 197 cm bed since
/// 2026-09-29). `rho_He` and `c_p(T_f)` are read straight
/// off [`Self::helium_state`] rather than re-evaluated, since that state was
/// itself solved for at the end of the previous step (or seeded at
/// construction).
///
/// The fluid term treats the node as well-mixed (a CSTR, not a plug-flow
/// slice): the outlet leaving the node is taken AT the node temperature
/// `T_f`. That is the accuracy an epsilon-NTU closed form avoids by
/// construction (its outlet is bounded strictly below `T_bed` for every
/// finite NTU); giving the fluid a real capacitance here trades that bound
/// away in exchange for a genuine transient on the helium side.
///
/// **Backward Euler.** Evaluate the right-hand side of both equations at
/// `T_s^{n+1}`, `T_f^{n+1}` instead of at the known `T_s^n`, `T_f^n`:
///
/// ```text
/// C_s (T_s^{n+1} - T_s^n) / dt = Q_fission - h A (T_s^{n+1} - T_f^{n+1})
/// C_f (T_f^{n+1} - T_f^n) / dt = h A (T_s^{n+1} - T_f^{n+1}) - m_dot c_p (T_f^{n+1} - T_in)
/// ```
///
/// **Collect into `A x = b`** with `x = [T_s^{n+1}, T_f^{n+1}]^T`:
///
/// ```text
/// (C_s/dt + hA) T_s^{n+1}  -  hA T_f^{n+1}                        = C_s/dt T_s^n + Q_fission
///     -hA T_s^{n+1}        +  (C_f/dt + hA + m_dot c_p) T_f^{n+1} = C_f/dt T_f^n + m_dot c_p T_in
/// ```
///
/// so the four matrix entries and two right-hand-side entries are
///
/// | | col 0: `T_s^{n+1}` | col 1: `T_f^{n+1}` | `b` |
/// |---|---|---|---|
/// | row 0 (solid balance) | `C_s/dt + hA` | `-hA` | `C_s/dt * T_s^n + Q_fission` |
/// | row 1 (fluid balance) | `-hA` | `C_f/dt + hA + m_dot c_p` | `C_f/dt * T_f^n + m_dot c_p * T_in` |
///
/// The matrix would be symmetric on `h A` alone -- the conduction-only
/// exchange between the two phases is self-adjoint -- and the throughflow
/// `m_dot c_p` term breaks that symmetry by adding to row 1 (the fluid
/// balance) only, never to row 0 or off-diagonal. Every diagonal entry is a
/// sum of strictly positive terms (a capacitance over a positive `dt` plus
/// a non-negative conductance), so the matrix is diagonally dominant and
/// [`SquareMatrix::solve`] never hits its singular case here.
///
/// **Solved with the workspace's own dense LU, not a hand-rolled 2x2
/// inverse.** [`outram_foam_basic_lib::matrix::square_matrix::SquareMatrix`]
/// already does exactly this in the crate -- `fhr_sim_v2`'s
/// `secondary_loop/vibe_code_mass_balance.rs` builds and solves a small
/// dense system the same way for its own mass balance; [`Self::step`] reuses
/// it via [`assemble_backward_euler_system`].
#[derive(Clone, Copy, Debug)]
pub struct PebbleBedPorousMediaNode {
    /// Pebble (solid-phase) temperature. ~~"carried directly rather than
    /// through a specific-enthalpy state, since this node's `c_p` is already
    /// constant"~~ **CHANGED 2026-09-28** — `c_p` is temperature dependent
    /// now; the temperature is still the stored state, but [`Self::step`]
    /// closes the solid balance on the graphite **enthalpy** (secant
    /// capacitance), so storing `T` loses nothing.
    pebble_temperature: ThermodynamicTemperature,
    /// Full thermodynamic state of the helium held in this node's void
    /// space -- not just a bare temperature. See the module comment above
    /// for why the whole state is stored: the fluid-phase balance needs
    /// density and `c_p` as well as temperature, all mutually consistent.
    ///
    /// **CHANGED 2026-09-29 (gh:#393)** -- this is now the `(p, h)` flash of
    /// [`Self::helium_enthalpy`], which is the integrated state. Its
    /// temperature, density and `c_p` are read off it; its own `enthalpy`
    /// field agrees with [`Self::helium_enthalpy`] only to the flash
    /// tolerance, so the balance never reads it.
    helium_state: FluidState,
    /// Specific enthalpy of the helium in the voids \[J/kg\] -- **the fluid
    /// row's integrated state** since 2026-09-29 (gh:#393). Stored exactly,
    /// not re-read from a flash, so the enthalpy the step closes its balance
    /// on is the enthalpy the next step starts from and the enthalpy the
    /// primary loop receives as the core outlet.
    helium_enthalpy: f64,
    /// Helium gas volume in the void space between packed pebbles.
    ///
    /// ~~from [`super::htr10_rz_geometry::pebble_bed_helium_volume`]~~ (zone
    /// 99, the 123.06 cm critical-loading bed) **CHANGED 2026-09-29
    /// (gh:#393)** -- now [`bed_void_volume`], the 197 cm operational bed the
    /// graphite mass ([`graphite_mass`], 27 000 pebbles) is taken from. The
    /// two nodes of this one control volume used to describe different core
    /// loadings: 1.2213 m^3 of void helium against the 1.9551 m^3 that the
    /// 27 000-pebble bed actually has. Fixed at construction: a geometric
    /// property of the core, not plant state.
    pebble_bed_helium_volume: Volume,
    /// Heat rate exchanged between the two phases across `h A` on the most
    /// recent [`Self::step`] (positive: solid phase heating the fluid).
    /// Distinct from the heat the fluid then carries OUT of the node by
    /// throughflow, which the fluid balance's `m_dot c_p` term accounts for
    /// separately -- see the struct doc comment's derivation.
    heat_to_helium: Power,
    /// Overall pebble-to-helium coefficient used on the most recent step --
    /// evaluated by [`overall_htc_at_flow`].
    overall_htc: HeatTransfer,
    /// Peak fuel-kernel temperature from the most recent step's resolved
    /// pebble solve, or `None` before the first step and whenever the solve
    /// was out of range. **Reported, not integrated**: nothing in this node's
    /// energy balance reads it, because the kernels are ~5 % of the ball by
    /// volume and their heat is already in the source term. See
    /// [`Self::peak_kernel_temperature`].
    peak_kernel_temperature: Option<ThermodynamicTemperature>,
    /// The **whole** resolved profile from the most recent step -- surface,
    /// fuelled-zone boundary, matrix centre, peak kernel and the hottest
    /// particle's own internal breakdown. `None` on the same terms as
    /// [`Self::peak_kernel_temperature`], which is a projection of this.
    ///
    /// Carried in full because the pebble solve produces all of it anyway, and
    /// the Map tab draws the real interior rather than interpolating between
    /// two endpoints -- an interpolated interior would be a hardcoded picture
    /// of a pebble, which this crate's "derive it from the physics" rule
    /// forbids.
    pebble_profile: Option<PebbleTemperatureProfile>,
    /// ~~Kernel-above-node thermal resistance ... so the Doppler channel can
    /// follow the kernel~~ **REPLACED 2026-09-28 (gh:#360)** by the
    /// fuel-to-bed coupling of the new fuel node: the resistance from the
    /// inventory-averaged kernel to this node, plus where the matrix, SiC and
    /// peak kernel sit on that line. `None` before the first step or when the
    /// resolved solve failed (now only above the 3000 K window). See
    /// [`FuelBedCoupling`].
    fuel_bed_coupling: Option<FuelBedCoupling>,
    /// Whether [`Self::fuel_bed_coupling`] came from THIS step's solve (`false`
    /// when the solve failed and the previous step's coupling was kept).
    fuel_bed_coupling_current: bool,
    /// Energy bookkeeping of the most recent [`Self::step`], from the same
    /// coefficients the solve used, so a caller can close an energy balance
    /// across the node without re-deriving any of it.
    last_step_energy: BedStepEnergy,
}

/// Energy terms of one [`PebbleBedPorousMediaNode::step`] \[J\], each computed
/// from the coefficients that step's solve actually used.
///
/// `source` = `solid_storage + fluid_storage + throughflow_out +
/// to_passive_path` to solver tolerance: that identity is the node's energy balance, pinned by
/// `tests::the_bed_step_closes_its_own_energy_balance`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BedStepEnergy {
    /// Heat the fuel delivered to the solid over the step, `Q dt`
    /// (~~`Q_net dt`, net of the passive loss~~ until 2026-09-29).
    pub source: f64,
    /// Change in graphite enthalpy, `m (h(T') - h(T))` (secant capacitance
    /// times the temperature change).
    pub solid_storage: f64,
    /// Change in helium stored in the voids, `M_f (h_f' - h_f)` (~~`C_f (T_f'
    /// - T_f)`~~, changed to enthalpy 2026-09-29, gh:#393).
    pub fluid_storage: f64,
    /// Enthalpy carried out by the throughflow above the inlet,
    /// `m_dot (h_f' - h_in) dt` (~~`m_dot c_p (T_f' - T_in) dt`~~, changed
    /// 2026-09-29, gh:#393). This is exactly what the primary loop's hot-duct
    /// CV receives less what its cold-return CV discharged into the bed.
    pub throughflow_out: f64,
    /// Heat leaving the bed to the passive path (reflector, RPV, RCCS) over
    /// the step, `[G_s (T_s' - T_nw') + G_f (T_f' - T_nw')] dt` (gh:#395,
    /// 2026-09-29). Before then the passive loss was netted out of `source`.
    pub to_passive_path: f64,
}

impl PebbleBedPorousMediaNode {
    /// Construct the node seeded at thermal equilibrium, both phases at
    /// [`SEED_BED_TEMPERATURE_K`] -- the same cold-start seed every fidelity
    /// tier in this simulator opens at.
    ///
    /// # Panics
    ///
    /// If the helium `(T, p)` flash at the seed conditions fails to
    /// converge. Per this workspace's stale-state policy, a failed flash
    /// panics rather than silently keeping an earlier (or fabricated)
    /// state -- there is no earlier state to fall back to here in any case,
    /// this is construction.
    pub fn new() -> Self {
        let helium_state = state_pt(Fluid::Helium, SEED_BED_TEMPERATURE_K, SEED_BED_PRESSURE_PA)
            .expect("helium (T,p) flash failed to converge at the seed temperature/pressure");
        Self {
            pebble_temperature: ThermodynamicTemperature::new::<kelvin>(SEED_BED_TEMPERATURE_K),
            helium_state,
            helium_enthalpy: helium_state.enthalpy,
            pebble_bed_helium_volume: bed_void_volume(),
            heat_to_helium: Power::new::<watt>(0.0),
            overall_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(
                LEGACY_LUMPED_HTC_W_PER_M2_K,
            ),
            peak_kernel_temperature: None,
            pebble_profile: None,
            // Seeded at the design point so the fuel node has a heat path on
            // the very first kinetics step, before any bed step has run.
            fuel_bed_coupling: Some(FuelBedCoupling::at_design_point()),
            fuel_bed_coupling_current: false,
            last_step_energy: BedStepEnergy::default(),
        }
    }

    /// Advance both phases by `dt` with one implicit step and return the
    /// heat rate exchanged between them. See the struct doc comment for the
    /// full derivation of the 2x2 backward-Euler system assembled and
    /// solved here.
    ///
    /// `helium_mass_flow` here drives a THROUGHFLOW term on the fluid
    /// phase's own balance (`m_dot c_p (T_f - T_in)`), not a capacity-rate
    /// cap on an externally-closed exchange -- see the derivation above for
    /// why the flow enters the fluid equation directly in this formulation.
    ///
    /// ~~`fission_power` and `decay_heat_power` are taken as SEPARATE
    /// arguments and summed internally into the single source term `Q`~~
    /// **CHANGED 2026-09-28 (gh:#360)** — the bed no longer receives the
    /// reactor's thermal power at all. Fission and decay heat are deposited in
    /// the **fuel node** (`physics::kinetics`), and the bed receives what the
    /// fuel conducts to it. So the two arguments are now:
    ///
    /// - ~~`net_heat_to_bed` — the fuel-to-bed conduction **less** the
    ///   passive loss to the reflector ... It can be negative~~ **CHANGED
    ///   2026-09-29 (gh:#395):** `heat_from_fuel`, the fuel-to-bed conduction
    ///   alone. The passive loss is no longer subtracted from the source: the
    ///   reflector and RPV (`passive_path`) are unknowns of this step's own
    ///   implicit solve -- see "One implicit solve with the passive path"
    ///   below.
    /// - `pebble_conduction_power` — the **gross** heat the kernels are
    ///   conducting out through the pebble this step, which is what the
    ///   resolved pebble profile must be solved at. ~~Solving it at the net
    ///   power~~ (the smaller finding recorded on gh:#360, ~0.94 K low at
    ///   11 MW) is fixed by passing it separately.
    ///
    /// # The solid balance is closed on ENTHALPY (2026-09-28)
    ///
    /// With `c_p(T)` from Butland & Maddison the capacitance moves inside a
    /// step, so a backward-Euler solve at the start-of-step capacitance would
    /// create or destroy `(dC/dT) dT^2 / 2` of energy every step. Instead the
    /// 2x2 system is solved with the **secant** capacitance
    ///
    /// ```text
    /// C_sec = m (h(T') - h(T)) / (T' - T)
    /// ```
    ///
    /// iterated to a fixed point (a handful of passes; `h` is analytic), so on
    /// exit `m (h(T') - h(T)) = dt (Q - h A (T_s' - T_f'))` holds to
    /// 1e-10 relative. ~~The fluid row is unchanged.~~
    ///
    /// # One implicit solve with the passive path (2026-09-29, gh:#395)
    ///
    /// The unknowns are `[T_s', y = h_f'/c_p,k, T_nw', T_r', T_v']`: the bed
    /// solid and helium, the algebraic near-wall node, and the reflector and
    /// RPV of [`CoreToRccsPath`]. The bed -> reflector transfer is Achenbach's
    /// network (see [`super::super::decay_heat_removal`]): a stagnant-ZBS
    /// branch `G_s` from the solid and a flow-dispersion branch `G_f` from the
    /// helium into the near-wall node, then the wall film and the inner
    /// annulus half `G_w` to the reflector, `G_2` on to the RPV and `G_3` to
    /// the fixed RCCS. Every branch is on the diagonal; nothing is subtracted
    /// from a source. The legs and the reflector/RPV secant capacities are
    /// re-evaluated at the latest iterate, inside the same fixed point as the
    /// solid secant and the helium `T(h)` linearisation, until every node moves
    /// less than 1e-9 K. The energy identity `source = solid + helium storage
    /// + throughflow + to_passive_path` holds exactly on every pass, and
    /// `to_passive_path = (reflector + RPV storage) + RCCS` to the secant
    /// convergence.
    ///
    /// # The fluid row is closed on ENTHALPY too (2026-09-29, gh:#393)
    ///
    /// ~~`C_f dT_f/dt = h A (T_s - T_f) - m_dot c_p (T_f - T_in)`~~, with
    /// `c_p` at the node and `C_f` frozen, was the `T + Q/(m c_p)` shortcut
    /// the maintainer rejected (engine `CLAUDE.md`, human-review item 3). The
    /// row is now the helium's own enthalpy balance,
    ///
    /// ```text
    /// M_f (h_f' - h_f) / dt = h A (T_s' - T(h_f')) + m_dot (h_in - h_f')
    /// ```
    ///
    /// with `M_f = rho(h_f) V_void` the start-of-step void helium mass,
    /// `h_in` the enthalpy of the helium the primary loop delivers, and
    /// `T(h)` the CoolProp helium `(p, h)` inverse
    /// ([`outram_park_fork_coolprop::state_ph`], the same Helmholtz EOS the
    /// steam generator's helium array and the primary loop's CVs use). The
    /// interfacial term is Newton-linearised about the current iterate,
    /// `T(h) ~ T_k + (h - h_k)/c_p,k`, and the pass is repeated with the solid
    /// secant until `|T(h_f') - T_lin| < 1e-9 K`. The energy identity
    /// `source = solid + fluid storage + throughflow` holds **exactly** for
    /// every pass, because the same interfacial term appears in both rows;
    /// only the temperature the interface is evaluated at converges.
    ///
    /// # Panics
    ///
    /// If the helium `(T, p)` flash at the solved fluid-node temperature
    /// fails to converge -- same stale-state policy as [`Self::new`] -- or if
    /// the bed leaves the graphite's 300-3000 K window
    /// ([`graphite_specific_heat`]).
    pub fn step(
        &mut self,
        dt: Time,
        heat_from_fuel: Power,
        pebble_conduction_power: Power,
        helium_inlet_enthalpy: AvailableEnergy,
        helium_mass_flow: MassRate,
        passive_path: &mut CoreToRccsPath,
    ) -> Power {
        // 1. Coefficient and the void helium mass at the start-of-step state.
        let helium_temperature_now =
            ThermodynamicTemperature::new::<kelvin>(self.helium_state.temperature);
        // One resolved-pebble solve per step, at the GROSS conduction power,
        // reused for the conduction leg, the fuel-to-bed coupling and the
        // reported profile.
        let pebble_power = pebble_conduction_power / pebble_count();
        let profile_node_temperature = self.pebble_temperature;
        // The RESISTANCES (conduction leg and fuel-to-bed coupling) are read
        // off the resolved pebble at the core-average pebble power, i.e. in
        // the linear-conduction limit, with k(T) evaluated about this node's
        // temperature. Why not at the instantaneous power (2026-09-28): during
        // a prompt burst the fuel conducts 20-45 kW per pebble, and a STEADY
        // profile whose volume average is the node temperature then needs a
        // surface below absolute zero (measured: -214 K at 45.6 kW) -- the
        // quasi-steady assumption, not the property data, is what fails. In
        // the regime where it holds the resistance is power-independent to
        // 0.688 % over a fourfold power range
        // (`tests::the_kernel_offset_is_linear_in_power`), so the reference
        // power loses nothing there and stays defined everywhere.
        //
        // Fail loud: no silent fallback conductivity. Below the 3000 K window
        // this always resolves; above it there is no property set, and the
        // panic carries the reason `tampines` gave.
        let reference_power = core_average_pebble_power();
        let reference_profile =
            resolved_pebble_profile_checked(profile_node_temperature, reference_power)
                .unwrap_or_else(|reason| panic!("pebble-bed step: {reason}"));
        let h_internal = conduction_coefficient_from(Some(&reference_profile), reference_power);
        // The DISPLAYED profile (Map tab interior, peak kernel) is the steady
        // profile at the actual conduction power when one exists, and `None`
        // in a burst where it does not -- display only; nothing in the energy
        // balance, the feedback or the release reads it.
        let profile = resolved_pebble_profile(profile_node_temperature, pebble_power);
        let htc = series_coefficient(
            film_htc_at_flow(helium_mass_flow, helium_temperature_now),
            h_internal,
        );
        let conductance: ThermalConductance = htc * heat_transfer_area();

        // Start-of-step void helium mass, frozen over the step (the same
        // treatment the solid's secant gives its capacitance's start point).
        let helium_mass = helium_void_mass(self.helium_state, self.pebble_bed_helium_volume);

        // 2. The throughflow, floored so a stopped circulator does not leave
        //    the fluid row without a sink. The primary loop never commands
        //    less than 1e-2 kg/s, so the floor does not bind in the plant.
        let flow_floor = MassRate::new::<kilogram_per_second>(1.0e-6);
        let mass_flow = helium_mass_flow.abs().max(flow_floor);

        // 3. Fixed point on every state function and temperature-dependent
        //    leg: the solid secant capacitance on the graphite enthalpy, the
        //    Newton linearisation of T(h) on the helium, and the passive
        //    path's legs and secant capacities (gh:#395).
        let mass = graphite_mass();
        let t_n = self.pebble_temperature;
        let h_n = pebble_bed_specific_enthalpy_from_temperature(t_n);
        let helium_enthalpy_now = self.helium_enthalpy;
        let mut solid_capacity = bed_heat_capacity(t_n);
        let mut linearisation = self.helium_state;
        let mut linearisation_enthalpy = self.helium_enthalpy;
        let mut solved_solid_k = t_n.get::<kelvin>();
        let mut solved_helium_enthalpy = helium_enthalpy_now;
        let mut helium_at_solution = self.helium_state;
        let mut near_wall_k =
            0.5 * (t_n.get::<kelvin>() + passive_path.reflector_temperature().get::<kelvin>());
        let mut reflector_k = passive_path.reflector_temperature().get::<kelvin>();
        let mut rpv_k = passive_path.rpv_temperature().get::<kelvin>();
        // The riser helium enters at the cold-return CV's state -- the inlet
        // enthalpy this step was handed (gh:#397).
        let riser_inlet = ThermodynamicTemperature::new::<kelvin>(
            helium_state_at(helium_inlet_enthalpy).temperature,
        );
        let mut coupling = passive_path.coupling(
            t_n,
            helium_temperature_now,
            riser_inlet,
            mass_flow,
            passive_path.reflector_temperature(),
            passive_path.rpv_temperature(),
        );
        let mut linearised_helium_k = helium_temperature_now.get::<kelvin>();
        for _ in 0..60 {
            let (matrix, rhs) = assemble_backward_euler_system(
                dt,
                heat_from_fuel,
                helium_inlet_enthalpy,
                t_n,
                helium_enthalpy_now,
                conductance,
                solid_capacity,
                helium_mass,
                mass_flow,
                linearisation,
                linearisation_enthalpy,
                passive_path,
                &coupling,
            );
            let solution = matrix.solve(&rhs).expect(
                "the backward-Euler matrix is diagonally dominant by construction (positive \
                 capacitance-over-dt and conductance terms on every diagonal) and is never \
                 singular -- see the struct doc comment",
            );
            let moved = (solution[0] - solved_solid_k)
                .abs()
                .max((solution[2] - near_wall_k).abs())
                .max((solution[3] - reflector_k).abs())
                .max((solution[4] - rpv_k).abs());
            solved_solid_k = solution[0];
            // The fluid unknown is h_f'/c_p,k (kelvin-scaled; see
            // `assemble_backward_euler_system`).
            solved_helium_enthalpy = solution[1] * linearisation.cp;
            near_wall_k = solution[2];
            reflector_k = solution[3];
            rpv_k = solution[4];

            let dt_s_k = solved_solid_k - t_n.get::<kelvin>();
            let secant =
                if dt_s_k.abs() > 1.0e-9 {
                    let h_new = pebble_bed_specific_enthalpy_from_temperature(
                        ThermodynamicTemperature::new::<kelvin>(solved_solid_k),
                    );
                    mass * (h_new - h_n)
                        / TemperatureInterval::new::<temperature_interval::kelvin>(dt_s_k)
                } else {
                    bed_heat_capacity(t_n)
                };
            let solid_converged = ((secant - solid_capacity) / solid_capacity)
                .get::<ratio>()
                .abs()
                < 1.0e-12;
            solid_capacity = secant;

            // The helium temperature this pass's interface terms were
            // evaluated at, against the EOS temperature of the enthalpy it
            // solved for.
            linearised_helium_k = linearisation.temperature
                + (solved_helium_enthalpy - linearisation_enthalpy) / linearisation.cp;
            helium_at_solution = helium_state_at_seeded(
                AvailableEnergy::new::<joule_per_kilogram>(solved_helium_enthalpy),
                Some(linearised_helium_k),
            );
            let helium_converged =
                (helium_at_solution.temperature - linearised_helium_k).abs() < 1.0e-9;
            if solid_converged && helium_converged && moved < 1.0e-9 {
                break;
            }
            linearisation = helium_at_solution;
            linearisation_enthalpy = solved_helium_enthalpy;
            coupling = passive_path.coupling(
                ThermodynamicTemperature::new::<kelvin>(solved_solid_k),
                ThermodynamicTemperature::new::<kelvin>(helium_at_solution.temperature),
                riser_inlet,
                mass_flow,
                ThermodynamicTemperature::new::<kelvin>(reflector_k),
                ThermodynamicTemperature::new::<kelvin>(rpv_k),
            );
        }

        // 4. Energy bookkeeping, from the coefficients the final solve used.
        let dt_s = dt.get::<second>();
        let m_dot = mass_flow.get::<kilogram_per_second>();
        let h_in = helium_inlet_enthalpy.get::<joule_per_kilogram>();
        let to_reflector_w = coupling.near_wall_to_reflector * (near_wall_k - reflector_k);
        let to_rccs_w = coupling.rpv_to_rccs * (rpv_k - coupling.rccs_temperature_k);
        let to_risers_w =
            coupling.reflector_to_risers * (reflector_k - coupling.riser_helium_temperature_k);
        self.last_step_energy = BedStepEnergy {
            source: heat_from_fuel.get::<watt>() * dt_s,
            solid_storage: solid_capacity.get::<joule_per_kelvin>()
                * (solved_solid_k - t_n.get::<kelvin>()),
            fluid_storage: helium_mass.get::<kilogram>()
                * (solved_helium_enthalpy - helium_enthalpy_now),
            throughflow_out: m_dot * (solved_helium_enthalpy - h_in) * dt_s,
            // Evaluated as the two branches INTO the near-wall node, exactly
            // as the solid and helium rows carry them (the linearised helium
            // temperature is the one the rows used).
            to_passive_path: (coupling.solid_to_near_wall * (solved_solid_k - near_wall_k)
                + coupling.helium_to_near_wall * (linearised_helium_k - near_wall_k))
                * dt_s,
        };

        // 5. Store the solved solid temperature, the helium enthalpy (the
        //    integrated state, exactly) with its flashed state, and the
        //    passive path's two nodes and boundary heat rates.
        self.pebble_temperature = ThermodynamicTemperature::new::<kelvin>(solved_solid_k);
        self.helium_enthalpy = solved_helium_enthalpy;
        self.helium_state = helium_at_solution;
        passive_path.commit(
            ThermodynamicTemperature::new::<kelvin>(reflector_k),
            ThermodynamicTemperature::new::<kelvin>(rpv_k),
            Power::new::<watt>(to_reflector_w),
            Power::new::<watt>(to_rccs_w),
            Power::new::<watt>(to_risers_w),
        );

        // 6. Publish the exchanged heat rate, the coefficient, and what the
        //    profile says about the fuel stack.
        let delta = TemperatureInterval::new::<temperature_interval::kelvin>(
            solved_solid_k - helium_at_solution.temperature,
        );
        self.heat_to_helium = conductance * delta;
        self.overall_htc = htc;
        self.peak_kernel_temperature = profile.map(|p| p.peak_kernel_centre);
        self.pebble_profile = profile;
        // Formed against the node temperature the profile was SOLVED at (see
        // the 2026-09-28 correction recorded on gh:#360 for why), from the
        // reference-power profile above.
        let fresh = FuelBedCoupling::from_profile(&reference_profile, reference_power);
        self.fuel_bed_coupling_current = fresh.is_some();
        if fresh.is_some() {
            self.fuel_bed_coupling = fresh;
        }

        self.heat_to_helium
    }

    /// Pebble (solid-phase) temperature. See the field doc comment on
    /// [`Self`] for how this differs from a specific-enthalpy state.
    ///
    /// This is the ball's **volume average**, because the node's capacitance
    /// is the whole graphite mass. It is emphatically not a fuel temperature:
    /// see [`Self::peak_kernel_temperature`], which at core-average power runs
    /// about 24 K hotter.
    pub fn pebble_temperature(&self) -> ThermodynamicTemperature {
        self.pebble_temperature
    }

    /// Peak fuel-kernel temperature from the most recent step, or `None`
    /// before the first step / outside the correlation range.
    ///
    /// ## What this is
    ///
    /// The centre of the hottest UO2 kernel in the *hottest* coated particle,
    /// which `tampines` places at the pebble centre -- the bounding position.
    /// This is the quantity a fuel-temperature limit applies to, and the one
    /// a real Doppler feedback wants.
    ///
    /// ## What it is still NOT
    ///
    /// It is the peak kernel **of a core-average pebble**, because this tier
    /// has one bed node and therefore no power peaking factor, no axial or
    /// radial shape, and no burnup distribution. A real HTR-10 peak-power
    /// pebble runs hotter than this, and an irradiated one hotter again
    /// (fluence is passed as zero -- this simulator has no burnup).
    ///
    /// ~~"**Nothing reads this yet.** `physics::kinetics` still runs its
    /// Doppler feedback off [`Self::pebble_temperature`]."~~
    /// **CORRECTED 2026-09-22 -- the Doppler channel now reads the kernel.**
    /// `KernelDopplerChannel` (removed 2026-09-28, gh:#360) takes the *fuel*
    /// share of the published isothermal coefficient and applies it to the
    /// kernel, leaving the graphite share on this node; and
    /// [`crate::physics::fission_product_release`] drives TRISO-ATOPS off the
    /// same temperature. ~~"Both reach it through
    /// [`Self::kernel_offset_resistance`] rather than this accessor"~~
    /// **CORRECTED 2026-09-28 (gh:#360)** -- only the Doppler channel does, and
    /// it adds the offset to the kinetics node `T_f`, not to this bed node. The
    /// release channel reads **this accessor** (`physics/mod.rs`, step 6), so it
    /// gets `T_bed + offset` at the start-of-step bed temperature, and the two
    /// absolute kernel temperatures differ by `T_f - T_bed` (235 K measured at
    /// t = 1500 s -- see gh:#360).
    ///
    /// This remains the *reported* kernel temperature -- what the diagnostics
    /// table, the Map tab and a fuel-temperature limit read.
    pub fn peak_kernel_temperature(&self) -> Option<ThermodynamicTemperature> {
        self.peak_kernel_temperature
    }

    /// The whole resolved pebble profile from the most recent step, `None`
    /// before the first step or outside the correlation window.
    ///
    /// [`Self::peak_kernel_temperature`] is the projection of this that the
    /// feedback and release channels use; this exists for the Map tab, which
    /// draws the interior, and for the hottest particle's own breakdown --
    /// whose SiC temperature governs fission-product retention.
    pub fn pebble_profile(&self) -> Option<PebbleTemperatureProfile> {
        self.pebble_profile
    }

    /// ~~The kernel-above-node thermal resistance from the most recent step
    /// \[K per watt of per-pebble power\] ... what crosses the boundary is the
    /// slope, not the value: `T_kernel(t) = T_node + R_kernel P_pebble(t)`~~
    /// **REPLACED 2026-09-28 (gh:#360).** The kernel is a node of its own now
    /// (`physics::kinetics`, the Nordheim-Fuchs fuel node), so what crosses
    /// the boundary is the fuel-to-bed **coupling**: the conduction
    /// resistance the fuel node loses heat through, and where the matrix,
    /// SiC and peak kernel sit between the two nodes. See [`FuelBedCoupling`].
    ///
    /// Seeded at the design point by [`Self::new`], refreshed by every
    /// [`Self::step`]. Kept from the previous step when a solve fails (only
    /// above 3000 K now); [`Self::fuel_bed_coupling_is_current`] says which.
    pub fn fuel_bed_coupling(&self) -> Option<FuelBedCoupling> {
        self.fuel_bed_coupling
    }

    /// `true` when [`Self::fuel_bed_coupling`] was solved on the most recent
    /// step, `false` when it was carried over (seed, or a failed solve).
    pub fn fuel_bed_coupling_is_current(&self) -> bool {
        self.fuel_bed_coupling_current
    }

    /// Energy terms of the most recent [`Self::step`]. See [`BedStepEnergy`].
    pub fn last_step_energy(&self) -> BedStepEnergy {
        self.last_step_energy
    }

    /// Full thermodynamic state of the helium held in this node -- density,
    /// `c_p`, pressure and more, not just temperature. See the module
    /// comment above the struct for why the whole state is stored.
    pub fn helium_state(&self) -> FluidState {
        self.helium_state
    }

    /// Helium (fluid-phase) temperature held in this node -- a quantity a
    /// helium-as-external closure would not have, since this struct gives
    /// the helium its own node. Convenience accessor over
    /// [`Self::helium_state`]'s temperature field.
    pub fn helium_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.helium_state.temperature)
    }

    /// Helium void volume this node's fluid row holds \[m^3\] -- the 197 cm
    /// operational bed's since 2026-09-29 (gh:#393).
    #[allow(dead_code)] // read by the one-core-state test
    pub fn helium_void_volume(&self) -> Volume {
        self.pebble_bed_helium_volume
    }

    /// Specific enthalpy of the helium leaving this node \[J/kg\] -- the
    /// well-mixed node's own state, exactly as the step closed its balance
    /// on it. The primary loop's hot-duct CV receives `m_dot` times this.
    pub fn helium_outlet_enthalpy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.helium_enthalpy)
    }

    /// Heat rate exchanged between the phases across `h A` on the most
    /// recent step.
    pub fn heat_to_helium(&self) -> Power {
        self.heat_to_helium
    }
}

impl Default for PebbleBedPorousMediaNode {
    fn default() -> Self {
        Self::new()
    }
}

/// Mass of the helium held in a bed void volume `void_volume` at
/// thermodynamic state `helium_state` \[kg\]: `rho V_void`. This is `M_f` in
/// the [`PebbleBedPorousMediaNode`] derivation.
///
/// ~~`fluid_node_heat_capacity`, `rho V_void c_p`~~ -- **replaced 2026-09-29
/// (gh:#393)**: the fluid row balances enthalpy, so it needs the mass, not a
/// capacitance built on a local `c_p`.
fn helium_void_mass(helium_state: FluidState, void_volume: Volume) -> Mass {
    Mass::new::<kilogram>(helium_state.density * void_volume.get::<cubic_meter>())
}

/// Assemble the 5x5 backward-Euler coefficient matrix and right-hand side
/// for one pass of [`PebbleBedPorousMediaNode::step`] (~~2x2~~ until
/// 2026-09-29; the passive path's nodes joined it for gh:#395).
///
/// Unknowns, in order: `x = [T_s', y, T_nw', T_r', T_v']` with `y = h_f' /
/// c_p,k` -- the helium enthalpy scaled by the linearisation point's `c_p` so
/// every unknown is in kelvin and the matrix keeps the diagonally dominant
/// shape of the old temperature form. `[1] * c_p,k` is the solved helium
/// enthalpy \[J/kg\]; the others are kelvin.
///
/// With `T(h) ~ T_k + (h - h_k)/c_p,k = y + a`, `a = T_k - h_k/c_p,k`, and
/// the passive path's legs `G_s` (solid -> near wall), `G_f` (helium -> near
/// wall), `G_w` (near wall -> reflector), `G_2` (reflector -> RPV), `G_3`
/// (RPV -> RCCS), `G_riser` (reflector -> riser helium entering at the
/// cold-return temperature `T_cold`, gh:#397) and secant capacities `C_r`,
/// `C_v`:
///
/// | | `T_s'` | `y` | `T_nw'` | `T_r'` | `T_v'` | `b` |
/// |---|---|---|---|---|---|---|
/// | solid | `C_s/dt + hA + G_s` | `-hA` | `-G_s` | | | `C_s/dt T_s + Q + hA a` |
/// | helium | `-hA` | `M_f c_p,k/dt + hA + m_dot c_p,k + G_f` | `-G_f` | | | `M_f/dt h_f + m_dot h_in - (hA + G_f) a` |
/// | near wall | `-G_s` | `-G_f` | `G_s + G_f + G_w` | `-G_w` | | `G_f a` |
/// | reflector | | | `-G_w` | `C_r/dt + G_w + G_2 + G_riser` | `-G_2` | `C_r/dt T_r + G_riser T_cold` |
/// | RPV | | | | `-G_2` | `C_v/dt + G_2 + G_3` | `C_v/dt T_v + G_3 T_rccs` |
///
/// Every exchange appears with opposite signs in the two rows it joins, which
/// is why the rows' sum is the exact energy identity.
#[allow(clippy::too_many_arguments)]
fn assemble_backward_euler_system(
    dt: Time,
    reactor_thermal_power: Power,
    helium_inlet_enthalpy: AvailableEnergy,
    pebble_temperature: ThermodynamicTemperature,
    helium_enthalpy: f64,
    conductance: ThermalConductance,
    solid_capacity: HeatCapacity,
    helium_mass: Mass,
    mass_flow: MassRate,
    linearisation: FluidState,
    linearisation_enthalpy: f64,
    passive_path: &CoreToRccsPath,
    coupling: &PassiveCoupling,
) -> (SquareMatrix, [f64; 5]) {
    let dt_s = dt.get::<second>();
    let h_a = conductance.get::<watt_per_kelvin>();
    let c_s = solid_capacity.get::<joule_per_kelvin>();
    let m_f = helium_mass.get::<kilogram>();
    let m_dot = mass_flow.get::<kilogram_per_second>();
    let cp_k = linearisation.cp;
    let offset = linearisation.temperature - linearisation_enthalpy / cp_k;

    let t_s_n = pebble_temperature.get::<kelvin>();
    let h_in = helium_inlet_enthalpy.get::<joule_per_kilogram>();
    let q_source = reactor_thermal_power.get::<watt>();

    let (g_s, g_f, g_w) = (
        coupling.solid_to_near_wall,
        coupling.helium_to_near_wall,
        coupling.near_wall_to_reflector,
    );
    let (g_2, g_3) = (coupling.reflector_to_rpv, coupling.rpv_to_rccs);
    let (c_r, c_v) = (coupling.reflector_capacity, coupling.rpv_capacity);
    let t_r_n = passive_path.reflector_temperature().get::<kelvin>();
    let t_v_n = passive_path.rpv_temperature().get::<kelvin>();

    let mut matrix = SquareMatrix::new(5);
    matrix.set(0, 0, c_s / dt_s + h_a + g_s);
    matrix.set(0, 1, -h_a);
    matrix.set(0, 2, -g_s);
    matrix.set(1, 0, -h_a);
    matrix.set(1, 1, m_f * cp_k / dt_s + h_a + m_dot * cp_k + g_f);
    matrix.set(1, 2, -g_f);
    matrix.set(2, 0, -g_s);
    matrix.set(2, 1, -g_f);
    matrix.set(2, 2, g_s + g_f + g_w);
    matrix.set(2, 3, -g_w);
    matrix.set(3, 2, -g_w);
    matrix.set(3, 3, c_r / dt_s + g_w + g_2 + coupling.reflector_to_risers);
    matrix.set(3, 4, -g_2);
    matrix.set(4, 3, -g_2);
    matrix.set(4, 4, c_v / dt_s + g_2 + g_3);

    let rhs = [
        c_s / dt_s * t_s_n + q_source + h_a * offset,
        m_f / dt_s * helium_enthalpy + m_dot * h_in - (h_a + g_f) * offset,
        g_f * offset,
        c_r / dt_s * t_r_n + coupling.reflector_to_risers * coupling.riser_helium_temperature_k,
        c_v / dt_s * t_v_n + g_3 * coupling.rccs_temperature_k,
    ];

    (matrix, rhs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A passive path in equilibrium with the seeded bed, stagnant -- what a
    /// bed-only test hands `step` now the reflector and RPV are rows of its
    /// solve (2026-09-29, gh:#395).
    fn seeded_passive_path(node: &PebbleBedPorousMediaNode) -> CoreToRccsPath {
        CoreToRccsPath::new_at_steady_state(
            node.pebble_temperature(),
            node.helium_temperature(),
            node.helium_temperature(),
            MassRate::new::<kilogram_per_second>(0.0),
        )
    }

    /// **The fuel-to-bed coupling is read off the profile solved at the
    /// start-of-step node temperature, at the gross conduction power**
    /// (gh:#360, 2026-09-28).
    ///
    /// Replaces ~~`the_kernel_offset_resistance_uses_the_profile_node_temperature`~~
    /// (its quantity, the peak-kernel offset resistance, no longer exists; the
    /// start-of-step rule it pinned carries over).
    ///
    /// **Methodology.** Step a design-point node once for 5 s with 50 MW of
    /// fuel-to-bed conduction (large enough to move the bed several kelvin),
    /// then require the published coupling to equal
    /// [`FuelBedCoupling::from_profile`] of the resolved profile at the
    /// start-of-step node temperature and the **core-average reference
    /// power** exactly (the resistances are evaluated in the linear-conduction
    /// limit -- see [`PebbleBedPorousMediaNode::step`]); require that profile's volume average to be the
    /// start-of-step node temperature (to 1e-6 K); and require the ordering
    /// `0 < matrix < SiC < 1 < peak kernel` on the fuel-bed line.
    ///
    /// **Results (2026-09-28).** Bed moved +24.77 K; profile average equal to
    /// the 950 K start to 1e-6 K; `R_fb = 1.0250e-6 K/W` (core), matrix
    /// 0.4777, SiC 0.4956, peak kernel 2.2881 of the fuel-bed offset. (A first
    /// reading the same day, 1.0562e-6 K/W / 0.4672 / 0.4853 / 2.2798, was
    /// taken at the 50 MW step power, before the resistances were moved to the
    /// reference power; the 3 % difference is the k(T) nonlinearity at 5x
    /// rated power.)
    /// matrix 0.4672, SiC 0.4853, peak kernel 2.2798 of the fuel-bed offset.
    #[test]
    fn the_fuel_bed_coupling_is_read_off_the_start_of_step_profile() {
        let mut node = PebbleBedPorousMediaNode::new();
        let t_start = node.pebble_temperature();
        let power = Power::new::<megawatt>(50.0);
        let inlet = helium_enthalpy_at(ThermodynamicTemperature::new::<kelvin>(673.15));
        let mut path = seeded_passive_path(&node);
        node.step(
            Time::new::<second>(5.0),
            power,
            power,
            inlet,
            nominal_helium_flow(),
            &mut path,
        );
        let moved_k = node.pebble_temperature().get::<kelvin>() - t_start.get::<kelvin>();
        // The coupling is read off the REFERENCE-power profile (core-average
        // pebble power) at the start-of-step node temperature -- see the step.
        let reference = core_average_pebble_power();
        let profile = resolved_pebble_profile(t_start, reference).expect("design point resolves");
        let average = resolved_pebble().volume_average_temperature(&profile);
        let expected = FuelBedCoupling::from_profile(&profile, reference).expect("positive rise");
        let got = node.fuel_bed_coupling().expect("coupling present");
        println!(
            "bed moved {moved_k:+.4} K; profile average {:.6} K vs start {:.6} K; \
             R_fb = {:.6e} K/W (core), matrix {:.4}, SiC {:.4}, peak kernel {:.4}",
            average.get::<kelvin>(),
            t_start.get::<kelvin>(),
            got.resistance.get::<kelvin_per_watt>(),
            got.fuelled_zone_matrix_fraction,
            got.silicon_carbide_fraction,
            got.peak_kernel_fraction
        );
        assert!(moved_k.abs() > 1.0);
        assert!((average.get::<kelvin>() - t_start.get::<kelvin>()).abs() < 1e-6);
        assert_eq!(got, expected);
        assert!(node.fuel_bed_coupling_is_current());
        assert!(
            0.0 < got.fuelled_zone_matrix_fraction
                && got.fuelled_zone_matrix_fraction < got.silicon_carbide_fraction
                && got.silicon_carbide_fraction < 1.0
                && 1.0 < got.peak_kernel_fraction
        );
    }

    /// **The fuel node's heat capacity is the TRISO particles and nothing
    /// else** (gh:#360, 2026-09-28).
    ///
    /// **Methodology.** Evaluate [`fuel_node_heat_capacity`] at 950 K and
    /// check it against a hand evaluation from the published particle
    /// (IAEA-TECDOC-1382 Table 4-17 radii and densities, 8335 particles x
    /// 27 000 pebbles) with the same property correlations; check the kernel
    /// heavy-metal mass implied by the kernel volume is the published 5.0 g
    /// per pebble to 0.1 %; and check fuel node + graphite (which now excludes
    /// the particle volume) against the retired all-graphite ball.
    ///
    /// **Results (2026-09-28).** `C_fuel = 2.67498e5 J/K` at 950 K (hand
    /// evaluation identical); cp UO2 312.06, SiC 1177.06, carbon 1729.94
    /// J/(kg K); UO2 5.6735 g per pebble (= 5.00 g U at 17 % enrichment); bed
    /// graphite 8.87312e6 J/K over 5129.16 kg -- the fuel node is 2.93 % of
    /// the core's heat capacity.
    #[test]
    fn the_fuel_node_capacity_is_the_particles_and_nothing_else() {
        use std::f64::consts::PI;
        let t = ThermodynamicTemperature::new::<kelvin>(950.0);
        let c = fuel_node_heat_capacity(t).get::<joule_per_kelvin>();
        let n = 8335.0 * 27_000.0;
        let v = |a: f64, b: f64| 4.0 / 3.0 * PI * (b.powi(3) - a.powi(3));
        let cp_uo2 = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(950.0));
        let cp_sic = HeatCapacityModel::SneadSiC.value(&MaterialState::fresh(950.0));
        let cp_c = graphite_specific_heat(t).get::<joule_per_kilogram_kelvin>();
        let hand = n
            * (v(0.0, 250e-6) * 10400.0 * cp_uo2
                + v(250e-6, 340e-6) * 1100.0 * cp_c
                + v(340e-6, 380e-6) * 1900.0 * cp_c
                + v(380e-6, 415e-6) * 3180.0 * cp_sic
                + v(415e-6, 455e-6) * 1900.0 * cp_c);
        let kernel_uo2_kg_per_pebble = 8335.0 * v(0.0, 250e-6) * 10400.0;
        let graphite_c = bed_heat_capacity(t).get::<joule_per_kelvin>();
        println!(
            "fuel node C = {c:.5e} J/K (hand {hand:.5e}); cp UO2 {cp_uo2:.2}, SiC {cp_sic:.2}, \
             C {cp_c:.2} J/(kg K); UO2 per pebble {:.4} g; bed graphite C = {graphite_c:.5e} J/K \
             (fuel share {:.2} %); graphite mass {:.2} kg",
            kernel_uo2_kg_per_pebble * 1e3,
            100.0 * c / (c + graphite_c),
            graphite_mass().get::<kilogram>()
        );
        assert!((c - hand).abs() < 1e-9 * hand);
    }

    /// **Why `peak_kernel_temperature` goes `None` on a hot core** — the answer
    /// to a question that cost real diagnosis time on 2026-09-27.
    ///
    /// # The chain being explained
    ///
    /// At the shipped opening condition the bed reaches ~2589 K within 8 s
    /// (gh:#318). From there:
    ///
    /// 1. `resolved_pebble_profile` calls `steady_state_temperatures` and discards
    ///    its error with `.ok()?`;
    /// 2. so `peak_kernel_temperature` is `None`;
    /// 3. so the TRISO-ATOPS release channel is handed nothing and ~~produces no
    ///    source term;~~ **CORRECTED 2026-09-28** — does not re-evaluate:
    ///    `TrisoAtopsReleaseChannel::update` (`fission_product_release.rs:477`)
    ///    returns early *without clearing* `latest`, so the channel keeps the
    ///    last release it computed at a resolved (cooler) kernel;
    /// 4. ~~so `AtmosphericDispersionChannel::update` returns early and the
    ///    dispersion model **does not run at all**.~~ **CORRECTED 2026-09-28** —
    ///    `AtmosphericDispersionChannel::update` returns early only when
    ///    `release.latest().is_empty()` (`atmospheric_dispersion.rs:1094`). That
    ///    is true only if the release channel has *never* evaluated. Once it has
    ///    evaluated at least once, the dispersion model **keeps running on that
    ///    stale, cooler release** while the core is too hot to resolve a kernel.
    ///
    /// ~~The dispersion output therefore goes **silent exactly when the core is
    /// hottest** — it does not report a large release, it reports nothing, and
    /// nothing on screen says why.~~ **CORRECTED 2026-09-28** — the dispersion
    /// output goes **stale exactly when the core is hottest**: it keeps showing a
    /// map drawn from the last release computed at a cooler kernel. It does not
    /// report the larger release a hotter core would give, and nothing on screen
    /// says the source term is out of date. It goes silent only if the kernel was
    /// never resolved (a freshly constructed release channel). Tracked as gh:#350;
    /// behaviour unchanged by this doc correction. That is the worst available failure direction
    /// for a safety-relevant readout, and it is why this test exists: to name the
    /// mechanism at the place it originates rather than leaving the next person to
    /// re-derive it from a `None`.
    ///
    /// # Methodology
    ///
    /// Call the pebble solve directly across a temperature sweep spanning the
    /// normal operating point and the excursion, and report for each whether it
    /// returns `Ok` or `Err` — **and what the error says**, which `.ok()?` throws
    /// away at the call site. The point is to establish whether the `None` is a
    /// property-correlation range refusal (the library behaving correctly) or a
    /// numerical failure (a defect in the solve).
    ///
    /// # Results
    ///
    /// Printed by this test.
    ///
    /// # Interpretation
    ///
    /// A range refusal is the property library doing the right thing: refusing to
    /// extrapolate is exactly the discipline the rest of this workspace insists
    /// on. The defect is in the **consumer** — `.ok()?` converts a
    /// "your temperature is outside my tabulated range" into an indistinguishable
    /// `None`, and every layer above then treats it as "no data" rather than
    /// "out of range". Whether to surface it, extrapolate with a stated caveat, or
    /// extend the correlation is a maintainer decision; losing the reason is not.
    ///
    /// **UPDATED 2026-09-28 (gh:#350, gh:#351) — the maintainer's decision was
    /// "extend, flagged".** [`resolved_pebble`] is now the high-temperature
    /// pebble, so the sweep resolves every surface up to 2589 K (measured in
    /// `tampines`' own test: peak kernel 2635.4 K at a 2589 K surface,
    /// **extrapolated**) and refuses only at a 3000 K surface, where the
    /// kernel would sit above the 3000 K window. The `.ok()?` still maps that
    /// refusal to `None`; above 3000 K there is no property set to report
    /// from. The assertion below now also pins that 2589 K resolves.
    #[test]
    fn why_the_resolved_kernel_goes_none_on_a_hot_core() {
        let pebble = resolved_pebble();
        let power = floored_pebble_power(Power::new::<watt>(
            design().thermal_power.get::<watt>() / pebble_count(),
        ));
        println!("PEBBLE SOLVE vs SURFACE TEMPERATURE (per-pebble power {power:?})");
        for surface_k in [900.0_f64, 1200.0, 1600.0, 2000.0, 2400.0, 2589.0, 3000.0] {
            let outcome = pebble.steady_state_temperatures(
                power,
                ThermodynamicTemperature::new::<kelvin>(surface_k),
                Ratio::new::<ratio>(0.0),
            );
            match outcome {
                Ok(profile) => println!(
                    "  surface {surface_k:>7.1} K -> Ok, peak kernel centre {:.1} K",
                    profile.peak_kernel_centre.get::<kelvin>()
                ),
                Err(e) => println!("  surface {surface_k:>7.1} K -> Err: {e:?}"),
            }
        }
        // Asserted only at the normal operating point, which must work.
        assert!(
            pebble
                .steady_state_temperatures(
                    power,
                    ThermodynamicTemperature::new::<kelvin>(900.0),
                    Ratio::new::<ratio>(0.0),
                )
                .is_ok(),
            "the pebble solve must succeed at a normal 900 K surface"
        );
        assert!(
            pebble
                .steady_state_temperatures(
                    power,
                    ThermodynamicTemperature::new::<kelvin>(2589.0),
                    Ratio::new::<ratio>(0.0),
                )
                .is_ok(),
            "the high-temperature pebble must resolve the gh:#350 bed temperature"
        );
    }
    use uom::si::power::megawatt;

    /// Methodology: the bed geometry is *derived* from three published HTR-10
    /// figures (27,000 fuel elements, 6.0 cm ball diameter, a 1.8 m x 1.97 m
    /// core) and then checked against two *other* published figures from the
    /// same document that the derivation never used -- the stated core volume
    /// of 5.0 m^3 and the stated volumetric filling fraction of balls of 0.61.
    /// If the geometry constants had been mistyped, these two independent
    /// checks would not close.
    ///
    /// Reference: IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled
    /// reactor performance: Benchmark analysis related to initial testing of
    /// the HTTR and HTR-10*, section 4.1 and Table 4-1; ingested at
    /// `iaea-tecdoc-1382-part2` (proprietary since 2026-09-22, held in the maintainer's private literature repository; see `crates/kovan-literature/CATALOGUE.md`).
    /// Pass criterion: core volume within 1% of 5.0 m^3, filling fraction
    /// within 2% of 0.61.
    ///
    /// Results (2026-08-12):
    ///
    /// | Quantity | Derived | Published | Error |
    /// |---|---|---|---|
    /// | Bed cylinder volume | 5.01304 m^3 | 5.0 m^3 | +0.26% |
    /// | Filling fraction of balls | 0.60914 | 0.61 | -0.14% |
    ///
    /// Both close to well under a percent, so the published core dimensions,
    /// pebble count and pebble diameter are mutually consistent and are
    /// correctly transcribed here. Derived quantities that follow: total
    /// graphite mass 5282.78 kg, total pebble surface area 305.363 m^2, helium
    /// void volume 1.95509 m^3, free-flow area 0.99243 m^2, single-pebble mass
    /// 0.19566 kg, lumped bed heat capacity 8.9807 MJ/K.
    ///
    /// **UPDATED 2026-09-28 (gh:#360):** the coated particles (3.29 cm^3 of
    /// each 113.10 cm^3 ball) are the fuel node now and are excluded from the
    /// graphite: graphite mass **5129.16 kg** (was 5282.78), and the bed's
    /// capacity is `m c_p(T)` -- **8.873 MJ/K at 950 K** (was a constant
    /// 8.9807). The two published-figure checks above are unaffected: they
    /// test the ball count, diameter and core volume, not the mass.
    ///
    /// Interpretation: this verifies the *geometry*, and nothing else. It says
    /// nothing about whether the lumped thermal model on top of that geometry
    /// represents an HTR-10 core -- it does not.
    #[test]
    fn bed_geometry_reproduces_the_published_core() {
        let volume = bed_volume().get::<cubic_meter>();
        assert!(
            (volume - 5.0).abs() / 5.0 < 0.01,
            "derived bed volume {volume} m^3 departs from the published 5.0 m^3"
        );

        let filling = derived_filling_fraction();
        assert!(
            (filling - 0.61).abs() / 0.61 < 0.02,
            "derived filling fraction {filling} departs from the published 0.61"
        );

        // The porosity constant must be the complement of the published
        // filling fraction, not an independently guessed number.
        assert!((bed_porosity().get::<ratio>() - (1.0 - 0.61)).abs() < 1e-12);

        // Sanity on the derived masses and areas the thermal model rests on.
        assert!(graphite_mass().get::<kilogram>() > 5000.0);
        assert!(heat_transfer_area().get::<square_meter>() > 300.0);
    }

    /// V&V (the reasoned case for keeping a lumped surface coefficient):
    /// conduction through the bed is negligible beside convection at power.
    ///
    /// **Methodology.** The workspace now has an effective pebble-bed
    /// conductivity -- the Zehner-Bauer-Schlunder tabulation in
    /// [`outram_park_digital_twin_engine::htr10::zbs`], 18 points from 300 K to
    /// 2000 K, transcribed from the Virtual Test Bed generic PBR deck (Open
    /// tier, CC-BY-4.0). The question this test settles is whether that
    /// conductivity belongs in *this* model's heat path. It computes, via
    /// [`conduction_only_axial_heat_rate`], the heat the bed could carry by
    /// conduction alone across the published 450 K inlet-to-outlet difference
    /// spread over the published 1.97 m bed height at the 2.545 m^2 bed
    /// cross-section, evaluated at the 748.15 K bulk mean, and compares it to
    /// the 10 MWth convective duty. Pass criterion: the conduction path is
    /// under 1% of the convective duty (so neglecting it at power is
    /// defensible), and `k_eff` sits inside the tabulated range.
    ///
    /// **Results (recorded 2026-08-12).** `k_eff(748.15 K)` = 20.195 W/(m K)
    /// (between the tabulated 18.895 at 700 K and 21.595 at 800 K), giving an
    /// axial conduction rate of **11.74 kW**, i.e. **0.117%** of the 10 MW the
    /// helium carries away by convection.
    ///
    /// **Interpretation -- why the lumped 160 W/(m^2 K) coefficient stays, and
    /// what that costs.** Three separate points, and the third is the honest
    /// cost:
    ///
    /// 1. **ZBS cannot replace the surface coefficient.** They are different
    ///    resistances. ZBS is the *bed-continuum* conductivity (solid contact +
    ///    gas + radiation between balls), the property of a porous medium with
    ///    an internal temperature gradient. The overall coefficient here spans
    ///    the *pebble surface*: intra-pebble conduction in series with the
    ///    convective film. Substituting one for the other would be a category
    ///    error, not a refinement.
    /// 2. **This model has no gradient for a conductivity to act on.** The bed
    ///    is one control volume by construction, so `k_eff` has nowhere to
    ///    appear. It becomes usable the moment the bed is nodalised -- which is
    ///    the refinement this module's docs already point at -- and the number
    ///    above says that at power it would then contribute about a tenth of a
    ///    percent of the heat removal. **Under loss of forced cooling it is not
    ///    negligible at all: it becomes the entire heat path**, and that is the
    ///    regime this single-node model cannot enter.
    /// 3. **The surface coefficient has since been fixed (2026-08-14), and
    ///    this test's conclusion is unchanged by it.** When this test was
    ///    written the coefficient was an invented 160 W/(m^2 K) that put the
    ///    bed 204.7 K above the helium -- about *twice* the published peak of
    ///    Gao & Shi (2002) Table 2 (peak surface-to-coolant 58.7 K plus peak
    ///    internal drop 42.0 K = 100.7 K). It is now evaluated as a Wakao film
    ///    in series with intra-pebble conduction and gives 67.4 K, correctly
    ///    below the peak; see
    ///    [`tests::the_evaluated_coefficient_beats_the_old_invented_one`].
    ///    That is a *surface* resistance either way, so points 1 and 2 above --
    ///    that ZBS is a different resistance, and that a one-node bed has no
    ///    gradient for it to act on -- still stand exactly as written.
    #[test]
    fn zbs_conduction_is_negligible_beside_convection_at_power() {
        let bulk_mean = ThermodynamicTemperature::new::<kelvin>(748.15);
        let k_eff = zbs_effective_conductivity(bulk_mean).get::<watt_per_meter_kelvin>();
        let published_rise_k = 450.0; // 250 -> 700 degC, published.
        let conduction = conduction_only_axial_heat_rate(bulk_mean, published_rise_k);
        let convection = Power::new::<megawatt>(10.0);
        let fraction = conduction.get::<watt>() / convection.get::<watt>();
        println!(
            "k_eff(748.15 K) = {:.3} W/(m K); axial conduction over the bed = {:.2} kW = {:.3}% \
             of the 10 MW convective duty",
            k_eff,
            conduction.get::<watt>() / 1.0e3,
            fraction * 100.0
        );
        assert!(
            fraction < 0.01,
            "bed conduction is {fraction} of the convective duty -- no longer negligible, so the \
             lumped surface coefficient needs revisiting"
        );
        // The tabulation spans 11.94 to 44.95 W/(m K); a value outside that
        // means the interpolant was fed the wrong temperature.
        assert!((11.94..=44.96).contains(&k_eff));
    }

    /// V&V: the evaluated two-resistance coefficient must put the bed-average
    /// pebble-to-helium difference **below the published peak**, which the old
    /// invented lumped constant did not.
    ///
    /// **Methodology.** [`overall_htc_at_flow`] is evaluated at the published
    /// rated point (4.3 kg/s, 748.15 K bulk mean, 3.0 MPa) and the settled
    /// bed-average difference `Q/(U A)` is formed at 10 MWth over the derived
    /// 305 m^2 of pebble surface. The reference is Gao & Shi (2002) Table 2 at
    /// 100% load on the equilibrium core: maximum fuel 918.7 degC, maximum fuel
    /// *surface* 876.7 degC, maximum coolant 818 degC -- a **peak** internal
    /// drop of 42.0 K, a **peak** surface-to-coolant drop of 58.7 K, and
    /// 100.7 K in total at the hottest point in the core.
    ///
    /// Pass criterion: a bed *average* must sit below the published *peak*, so
    /// the evaluated total difference must be under 100.7 K, and it must beat
    /// the legacy constant's 204.7 K.
    ///
    /// **Results (2026-08-14).** Helium at 748.15 K and 3.0 MPa came back as
    /// `k = 0.2961 W/(m K)`, `Pr = 0.6601`, `mu = 3.765e-5 Pa s` -- all
    /// physically right for helium at these conditions. That gives
    /// `Re_p = 2692.7` and `Nu = 111.49`, hence
    ///
    /// | Resistance | Coefficient \[W/(m^2 K)\] |
    /// |---|---|
    /// | Wakao surface film | 550.2 |
    /// | Intra-pebble conduction (`10 k/d`) | 4166.7 |
    /// | **Series total `U`** | **486.1** |
    ///
    /// The bed-average pebble-to-helium difference is therefore **67.4 K**,
    /// against **204.7 K** from the legacy constant and a published **peak** of
    /// 100.7 K. The film carries about 88% of the resistance, which is why the
    /// old flow-scaling shape was roughly right while its magnitude was not.
    ///
    /// **Interpretation.** The remaining gap to the published peak is expected
    /// and is *not* a defect: this is a bed **average** over one node against a
    /// **peak** in a real core with axial, radial and pebble-internal
    /// gradients and a power peaking factor. The average being comfortably
    /// under the peak is the correct ordering; the old constant violated it.
    #[test]
    fn the_evaluated_coefficient_beats_the_old_invented_one() {
        let helium = ThermodynamicTemperature::new::<kelvin>(748.15);
        let area = heat_transfer_area().get::<square_meter>();
        let q = 1.0e7;

        // The bed sits ~67 K above the helium at this duty, so the conduction
        // leg is evaluated there rather than at the helium temperature.
        let bed = ThermodynamicTemperature::new::<kelvin>(748.15 + 67.0);
        let rated = Power::new::<watt>(1.0e7);
        let u = overall_htc_at_flow(nominal_helium_flow(), helium, bed, rated)
            .get::<watt_per_square_meter_kelvin>();
        // Compared on the SURFACE resistance alone (`Q/(U A)`), which is what
        // the legacy constant also represented -- an apples-to-apples contrast
        // of the coefficient itself, separate from the epsilon-NTU capacity
        // limit that the full balance additionally imposes.
        let evaluated_dt = q / (u * area);
        let legacy_dt = q / (LEGACY_LUMPED_HTC_W_PER_M2_K * area);

        let (k_he, pr, mu) = helium_transport(helium);
        let mass_flux = kta::superficial_mass_flux(nominal_helium_flow(), superficial_area());
        let re = kta::packed_bed_reynolds(
            mass_flux,
            pebble_diameter(),
            DynamicViscosity::new::<pascal_second>(mu),
        )
        .get::<ratio>();
        let nu = wakao_nusselt(re, pr);
        let h_film = nu * k_he / pebble_diameter().get::<meter>();
        let h_int = intra_pebble_conduction_coefficient(bed, rated / pebble_count())
            .get::<watt_per_square_meter_kelvin>();

        println!(
            "helium at 748.15 K, 3.0 MPa: k = {k_he:.4} W/(m K), Pr = {pr:.4}, mu = {mu:.3e} Pa s\n\
             Re_p = {re:.1}, Nu = {nu:.2}\n\
             h_film = {h_film:.1}, h_internal = {h_int:.1}, U(series) = {u:.1} W/(m^2 K)\n\
             bed-average dT: evaluated {evaluated_dt:.1} K vs legacy {legacy_dt:.1} K \
             (published PEAK 100.7 K)"
        );

        assert!(
            evaluated_dt < 100.7,
            "bed-average difference {evaluated_dt:.1} K must sit below the published peak 100.7 K"
        );
        assert!(
            evaluated_dt < legacy_dt,
            "the evaluated coefficient {evaluated_dt:.1} K must beat the legacy {legacy_dt:.1} K"
        );
        // The film should be the dominant resistance at rated flow.
        assert!(
            h_film < h_int,
            "expected the film to dominate: h_film {h_film:.1} vs h_internal {h_int:.1}"
        );
    }

    /// V&V: the **resolved two-zone pebble** must be more resistive than the
    /// uniform-ball form it replaced, by the factor the resolved pebble model
    /// independently measures — and the surface-temperature approximation
    /// this wiring makes must be worth less than the change it buys.
    ///
    /// **Methodology.** Three things are measured at the published rated point
    /// (10 MWth, 4.3 kg/s, bed at 815.15 K):
    ///
    /// 1. `h_int` from [`intra_pebble_conduction_coefficient`], against the
    ///    retired `10 k / d` with `k = 25 W/(m K)`
    ///    ([`LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K`]). The resolved
    ///    value must be the **smaller** coefficient: the unfuelled shell is a
    ///    resistance the uniform ball does not have, so resolving the geometry
    ///    can only add resistance, never remove it. A resolved value that came
    ///    out *less* resistive would mean the wiring is wrong, not that the
    ///    pebble is better than thought.
    /// 2. What it costs the **overall** coefficient, which is the number that
    ///    actually enters the energy balance. The film is ~88 % of the
    ///    resistance, so the expectation is a few percent, not a factor.
    /// 3. The peak kernel temperature the resolved pebble now exposes, and how
    ///    far it sits above the node temperature the Doppler feedback still
    ///    reads.
    ///
    /// Pass criteria: resolved is more resistive than the uniform ball, and
    /// the total bed-to-helium difference still sits below the 100.7 K
    /// published **peak** of Gao & Shi (2002) Table 2, which a bed *average*
    /// must.
    ///
    /// **History — a claim this test refused to let stand.** The first cut
    /// passed the node temperature straight in as the pebble surface and the
    /// doc comment asserted the error was "under 0.5 %". This test measured
    /// **1.135 %** over a 15 K offset and failed. The number was invented, not
    /// measured, so the fix was to delete the approximation rather than
    /// restate it: [`resolved_pebble_profile`] now solves for the surface
    /// whose volume average is the node temperature, and
    /// [`tests::the_inverted_profile_reproduces_the_node_temperature`] pins
    /// that. Widening the gate to 1.2 % would have "passed" and left an error
    /// with a known sign in the heat path.
    ///
    /// **Results (2026-09-22), at 10 MWth, 4.3 kg/s, node 815.15 K:**
    ///
    /// | | resolved | uniform ball |
    /// |---|---|---|
    /// | `h_int` \[W/(m^2 K)\] | **3803.6** | 4166.7 |
    /// | `U` (series with the 550.2 film) | **480.7** | 486.1 |
    /// | bed-average pebble-to-helium dT | **68.12 K** | 67.37 K |
    ///
    /// Resolved pebble at that node temperature: surface **806.54 K**, zone
    /// boundary 812.23 K, centre 830.66 K, **peak kernel 836.45 K** — the
    /// kernel sits **21.30 K** above the node the Doppler feedback reads.
    ///
    /// **Interpretation.** Resolving the geometry makes the pebble leg 9.5 %
    /// more resistive, and moves the number that actually enters the energy
    /// balance by **1.1 %** — because the film is ~88 % of the resistance, the
    /// pebble-side correction is diluted almost out of existence. Anyone
    /// expecting the unfuelled shell and the TRISO particles to matter to
    /// *heat removal* should read this table: they do not, at rated flow.
    ///
    /// What the resolved pebble buys is not the coefficient. It is the
    /// **21.30 K** — a fuel temperature the uniform ball could not produce at
    /// all, and the quantity a Doppler feedback and a fuel-temperature limit
    /// both want.
    ///
    /// Note also that this is *less* resistive than the 1.25x the standalone
    /// `tampines` V&V records, and the difference is real physics rather than
    /// a discrepancy: that test runs at a 1000 K surface, this at ~807 K, and
    /// A3 graphite conducts better cold. The retired constant was frozen at
    /// 25 W/(m K) at every temperature, so the two disagree by more at some
    /// temperatures than others — which is the point of having replaced it.
    #[test]
    fn the_resolved_pebble_beats_the_uniform_ball() {
        let helium = ThermodynamicTemperature::new::<kelvin>(748.15);
        let bed = ThermodynamicTemperature::new::<kelvin>(815.15);
        let rated = Power::new::<watt>(1.0e7);
        let area = heat_transfer_area().get::<square_meter>();

        let resolved = intra_pebble_conduction_coefficient(bed, rated / pebble_count())
            .get::<watt_per_square_meter_kelvin>();
        let uniform_ball =
            10.0 * LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K / pebble_diameter().get::<meter>();

        let u_resolved = overall_htc_at_flow(nominal_helium_flow(), helium, bed, rated)
            .get::<watt_per_square_meter_kelvin>();
        let h_film =
            film_htc_at_flow(nominal_helium_flow(), helium).get::<watt_per_square_meter_kelvin>();
        let u_uniform = 1.0 / (1.0 / h_film + 1.0 / uniform_ball);

        let profile = resolved_pebble_profile(bed, rated / pebble_count())
            .expect("the rated point is inside the correlation window");

        println!(
            "h_int: resolved {resolved:.1} vs uniform-ball {uniform_ball:.1} W/(m^2 K) \
             (ratio {:.3})\n\
             h_film {h_film:.1}; U: resolved {u_resolved:.1} vs uniform-ball {u_uniform:.1} \
             W/(m^2 K)\n\
             bed-average dT: resolved {:.2} K vs uniform-ball {:.2} K (published PEAK 100.7 K)\n\
             pebble at node 815.15 K: surface {:.2} K, zone boundary {:.2} K, centre {:.2} K, \
             peak kernel {:.2} K (kernel is {:.2} K above the node)",
            uniform_ball / resolved,
            1.0e7 / (u_resolved * area),
            1.0e7 / (u_uniform * area),
            profile.surface.get::<kelvin>(),
            profile.fuelled_zone_boundary.get::<kelvin>(),
            profile.centre.get::<kelvin>(),
            profile.peak_kernel_centre.get::<kelvin>(),
            profile.peak_kernel_centre.get::<kelvin>() - bed.get::<kelvin>(),
        );

        assert!(
            resolved < uniform_ball,
            "resolving the geometry must ADD resistance: resolved {resolved:.1} should be \
             below the uniform ball's {uniform_ball:.1} W/(m^2 K)"
        );
        assert!(
            1.0e7 / (u_resolved * area) < 100.7,
            "the bed AVERAGE must stay below the published PEAK"
        );
        assert!(
            profile.peak_kernel_centre > bed,
            "the peak kernel must sit above the ball's volume average"
        );
    }

    /// V&V: the kernel offset must be **linear in power** to within a stated
    /// error, because the Doppler channel holds its slope fixed across a plant
    /// step and scales it by the instantaneous power.
    ///
    /// # What is actually being checked, and why it is not obvious
    ///
    /// `kernel_offset_resistance` (replaced 2026-09-28 by [`PebbleBedPorousMediaNode::fuel_bed_coupling`]) publishes
    /// `R_kernel = dT/P`, refreshed once per 0.1 s plant step, and
    /// `KernelDopplerChannel` (removed 2026-09-28, gh:#360) then evaluates
    /// `dT(t) = R_kernel * P(t)` on every 1 ms kinetics substep. That is only
    /// sound if `R_kernel` is genuinely constant *over the power excursion
    /// within one step*. Steady conduction is linear in power, so it would be
    /// exactly constant if `k` were -- but A3 graphite's conductivity and
    /// UO2's both depend on temperature, and more power raises the interior
    /// temperature and so changes `k`. The resistance is therefore **weakly
    /// power-dependent**, and this measures how weakly.
    ///
    /// The alternative -- re-solving the pebble on every substep -- was
    /// rejected on cost (100 inverted profile solves per plant step), so this
    /// number is the price of that decision and belongs on the record.
    ///
    /// **Methodology.** At each of four bed temperatures spanning the
    /// operating range (600-1100 K), solve the resolved pebble across
    /// 0.25x-2.0x of core-average pebble power and form
    /// `R_kernel = (T_peak_kernel - T_node) / P`. Report the spread relative
    /// to the rated value. Pass criterion: within one bed temperature,
    /// `R_kernel` varies by less than 10 % across a **fourfold** power range
    /// -- far wider than one 0.1 s step can produce, so the per-step error is
    /// smaller again by the ratio of the excursions. Also assert `R_kernel` is
    /// monotone *increasing* in power, since `k(T)` falls with temperature.
    ///
    /// **Results (2026-09-22).** `R_kernel` \[K/W per pebble\]:
    ///
    /// | Bed node | 0.25x | 1.0x | 2.0x | spread |
    /// |---|---|---|---|---|
    /// | 600 K | 4.79571e-2 | 4.82044e-2 | 4.85361e-2 | 0.69 % |
    /// | 800 K | 5.65582e-2 | 5.68444e-2 | 5.72284e-2 | 0.68 % |
    /// | 950 K | 6.30120e-2 | 6.33243e-2 | 6.37429e-2 | 0.66 % |
    /// | 1100 K | 6.92840e-2 | 6.96148e-2 | 7.00575e-2 | 0.64 % |
    ///
    /// **Worst spread 0.688 %** over a fourfold power range -- so the
    /// linearisation is good to well under a percent over an excursion far
    /// larger than any single step can produce.
    ///
    /// **Interpretation.** The resistance *rises* with power, because the
    /// hotter interior conducts worse. Holding it fixed across a step
    /// therefore **under-states** the offset during a power rise, and so
    /// under-states the negative Doppler this channel supplies: the sign of
    /// the approximation error is conservative for a prompt excursion, which
    /// is the direction one would want if it had to be wrong.
    ///
    /// Note also the far *stronger* dependence on bed temperature -- 0.0482
    /// K/W at 600 K against 0.0696 K/W at 1100 K, a **44 % rise**, which is
    /// 60x the power-dependence. That is exactly why `R_kernel` is refreshed
    /// every plant step from the bed's own temperature rather than fixed at a
    /// design-point value: what actually moves it around is where the bed is,
    /// not what power it is at.
    ///
    /// This is a linearisation claim, not a claim that the pebble is linear.
    #[test]
    fn the_kernel_offset_is_linear_in_power() {
        let rated_pebble = core_average_pebble_power();
        let mut worst_spread = 0.0f64;

        println!("R_kernel [K/W per pebble], by bed temperature and power fraction:");
        for node_k in [600.0, 800.0, 950.0, 1100.0] {
            let node = ThermodynamicTemperature::new::<kelvin>(node_k);
            let mut resistances = Vec::new();
            for fraction in [0.25, 0.5, 1.0, 1.5, 2.0] {
                let power = rated_pebble * fraction;
                let profile = resolved_pebble_profile(node, power).unwrap_or_else(|| {
                    panic!("no profile at {node_k} K, {fraction}x rated -- inside the window")
                });
                let r = (profile.peak_kernel_centre.get::<kelvin>() - node_k) / power.get::<watt>();
                resistances.push((fraction, r));
            }

            let at_rated = resistances
                .iter()
                .find(|(f, _)| (*f - 1.0).abs() < 1e-12)
                .map(|(_, r)| *r)
                .expect("the rated point is in the sweep");
            let spread = resistances
                .iter()
                .map(|(_, r)| (r / at_rated - 1.0).abs())
                .fold(0.0f64, f64::max);
            worst_spread = worst_spread.max(spread);

            let cells: Vec<String> = resistances
                .iter()
                .map(|(f, r)| format!("{f:.2}x {r:.5e}"))
                .collect();
            println!(
                "  node {node_k:>6.1} K: {}  (spread {:.2}% about rated)",
                cells.join("  "),
                spread * 100.0
            );

            assert!(
                resistances.windows(2).all(|w| w[1].1 >= w[0].1),
                "R_kernel must not FALL with power at {node_k} K -- k(T) decreases with \
                 temperature, so a falling resistance means the solve is wrong"
            );
        }

        println!(
            "worst relative spread over a FOURFOLD power range = {:.3}%",
            worst_spread * 100.0
        );
        assert!(
            worst_spread < 0.10,
            "R_kernel varies {:.2}% over 0.25-2.0x rated; the Doppler channel holds it fixed \
             across a 0.1 s step, so this must stay small",
            worst_spread * 100.0
        );
    }

    /// V&V: [`resolved_pebble_profile`] must return a profile whose **volume
    /// average is the node temperature it was asked for** — that is the whole
    /// point of the inversion, and the property that makes the node's
    /// capacitance and its resistance describe the same pebble.
    ///
    /// **Methodology.** Over a grid of node temperatures (500-1500 K) and
    /// powers (1 %, 50 %, 100 %, 150 % of rated), solve and check that
    /// `volume_average_temperature` of the returned profile reproduces the
    /// requested node temperature to better than 1e-4 K — two orders inside
    /// the 1e-6 K the iteration converges to, so the margin is the iteration's
    /// own, not a tolerance chosen to fit. Also check the returned surface is
    /// strictly *below* the node (heat flows outward) and that the ordering
    /// surface < boundary < centre < kernel holds at every point.
    ///
    /// **Results (2026-09-22):** over all 20 (temperature, power) points the
    /// worst reproduction error was **2.914e-9 K** — five orders inside the
    /// 1e-4 K criterion and comfortably at the iteration's own 1e-6 K
    /// tolerance, so the inversion is converging, not merely passing. Every
    /// point satisfied surface < boundary < centre < kernel, and every surface
    /// sat below its node temperature.
    #[test]
    fn the_inverted_profile_reproduces_the_node_temperature() {
        let pebble = resolved_pebble();
        let rated_pebble_power = core_average_pebble_power();
        let mut worst = 0.0f64;

        for node_k in [500.0, 750.0, 1000.0, 1250.0, 1500.0] {
            for fraction in [0.01, 0.5, 1.0, 1.5] {
                let node = ThermodynamicTemperature::new::<kelvin>(node_k);
                let profile = resolved_pebble_profile(node, rated_pebble_power * fraction)
                    .unwrap_or_else(|| {
                        panic!("no profile at {node_k} K, {fraction}x rated -- inside the window")
                    });

                let reproduced = pebble.volume_average_temperature(&profile).get::<kelvin>();
                let error = (reproduced - node_k).abs();
                worst = worst.max(error);

                assert!(
                    profile.surface.get::<kelvin>() < node_k,
                    "the surface must sit below the ball average at {node_k} K"
                );
                assert!(profile.surface < profile.fuelled_zone_boundary);
                assert!(profile.fuelled_zone_boundary < profile.centre);
                assert!(profile.centre < profile.peak_kernel_centre);
            }
        }

        println!("worst |volume average - requested node temperature| = {worst:.3e} K");
        assert!(
            worst < 1.0e-4,
            "the inversion must reproduce the node temperature; worst error {worst:.3e} K"
        );
    }

    /// The enthalpy/temperature relation must round-trip exactly (it is linear,
    /// so no iteration is involved), and the flow scaling must be monotone,
    /// equal to the nominal coefficient at nominal flow, and strictly positive
    /// at zero flow.
    #[test]
    fn enthalpy_round_trips_and_htc_scales_with_flow() {
        for t_k in [400.0, 750.0, 1200.0] {
            let t = ThermodynamicTemperature::new::<kelvin>(t_k);
            let round_tripped = temperature_from_specific_enthalpy(
                pebble_bed_specific_enthalpy_from_temperature(t),
            )
            .get::<kelvin>();
            assert!((round_tripped - t_k).abs() < 1e-9);
        }

        let helium = ThermodynamicTemperature::new::<kelvin>(748.15);
        let bed = ThermodynamicTemperature::new::<kelvin>(815.15);
        let rated = Power::new::<watt>(1.0e7);
        let at_nominal = overall_htc_at_flow(nominal_helium_flow(), helium, bed, rated)
            .get::<watt_per_square_meter_kelvin>();

        let half = overall_htc_at_flow(
            MassRate::new::<kilogram_per_second>(0.5 * nominal_helium_flow_kg_per_s()),
            helium,
            bed,
            rated,
        )
        .get::<watt_per_square_meter_kelvin>();
        let double = overall_htc_at_flow(
            MassRate::new::<kilogram_per_second>(2.0 * nominal_helium_flow_kg_per_s()),
            helium,
            bed,
            rated,
        )
        .get::<watt_per_square_meter_kelvin>();
        assert!(half < at_nominal && at_nominal < double);

        let stopped = overall_htc_at_flow(
            MassRate::new::<kilogram_per_second>(0.0),
            helium,
            bed,
            rated,
        )
        .get::<watt_per_square_meter_kelvin>();
        assert!(
            stopped > 0.0,
            "a stopped circulator must leave a residual coefficient"
        );
        // The series resistance can never exceed either branch alone.
        let internal = intra_pebble_conduction_coefficient(bed, rated / pebble_count())
            .get::<watt_per_square_meter_kelvin>();
        assert!(
            at_nominal < internal,
            "a series coefficient must be below the intra-pebble branch alone"
        );
    }

    /// V&V: the implicit two-node balance must settle so that, at steady
    /// state, ALL of the reactor thermal power ends up carried out of the
    /// node by the helium throughflow -- `Q = m_dot c_p (T_f - T_in)` -- the
    /// two-temperature analogue of the removed `PebbleBedCore`'s own
    /// steady-state conservation check (see this file's module doc comment
    /// "History" note).
    ///
    /// **Methodology.** [`PebbleBedPorousMediaNode`] is stepped at the
    /// published 10 MWth and 4.3 kg/s against a 673.15 K (400 degC) helium
    /// inlet, 0.05 s steps for 3000 s of simulated time -- long enough for
    /// both the bed's ~184 s time constant and the much faster fluid-node
    /// time constant to settle. `decay_heat_power` is zero here (see
    /// [`fission_power_and_decay_heat_power_sum_into_the_same_source_term`]
    /// for the case that exercises it). Pass criterion:
    /// [`PebbleBedPorousMediaNode::heat_to_helium`] within 0.1% of 10 MW,
    /// and `m_dot c_p (T_helium - T_in)` (formed from the settled
    /// [`PebbleBedPorousMediaNode::helium_state`]) within 0.5% of 10 MW.
    ///
    /// **Results (2026-08-17):** settled `T_pebble = 1181.7126 K`,
    /// `T_helium = 1120.8171 K`, `heat_to_helium = 9.993938 MW` (6.06e-4
    /// relative), throughflow duty `9.993933 MW` (6.07e-4 relative). Both
    /// close well inside the pass criteria, and the two independent routes
    /// to the duty agree with each other to 5e-7 relative.
    ///
    /// **Re-measured 2026-09-28** (graphite `c_p(T)` from `tuas`, particles
    /// moved to the fuel node, secant-inverted pebble; the profile now solved
    /// at the 10 MW conduction power it carries rather than at the 1 % floor
    /// the old zero `decay_heat_power` argument never affected): settled
    /// `T_pebble = 1184.6389 K` (1184.7463 K on the pre-change tree the same
    /// day), `T_helium = 1120.7008 K`, `heat_to_helium = 9.991343 MW`,
    /// throughflow `9.991337 MW`. The steady state does not depend on `c_p`,
    /// so a ~0.1 K move is what the change should give -- it comes from the
    /// resolved pebble, not the heat capacity.
    ///
    /// **Re-measured 2026-09-29 (gh:#393: the fluid row on enthalpy, the void
    /// volume from the 197 cm bed):** `T_pebble = 1184.6528 K` (+0.0139 K),
    /// `T_helium = 1120.7149 K` (+0.0141 K), `heat_to_helium = 9.991322 MW`,
    /// throughflow `m_dot (h_f - h_in) = 9.991311 MW`. The void volume cannot
    /// move a steady state; the +0.014 K is the `c_p` shortcut removed from
    /// the throughflow -- `m_dot c_p(T_f) (T_f - T_in)` against the exact
    /// enthalpy difference over a 448 K rise.
    ///
    /// **Re-measured 2026-09-29 (gh:#395: the passive path is rows of this
    /// solve, with Achenbach's dispersion and wall-film legs):** the bed now
    /// also loses heat to the reflector, so it settles COOLER -- `T_pebble =
    /// 1154.0664 K` (-30.6 K), `T_helium = 1091.7608 K` (-29.0 K). Passive
    /// loss 647.750 kW; throughflow 9.344928 MW; their sum 9.992678 MW
    /// against the 10 MW source (-0.07 %: the reflector is still warming, so
    /// the path is not yet at its own steady state). The interfacial
    /// exchange, 9.703171 MW, exceeds the throughflow by the heat the helium
    /// gives the reflector through the dispersion branch. The test's
    /// assertions were changed accordingly: the node balance is throughflow
    /// + passive, not interfacial exchange alone.
    ///
    /// **Interpretation.** The two independent routes to the same duty --
    /// the interfacial exchange `h A (T_s - T_f)` and the throughflow
    /// ~~`m_dot c_p (T_f - T_in)`~~ `m_dot (h_f - h_in)` (2026-09-29) -- agree at steady state, which is exactly
    /// the identity the solid and fluid balances jointly enforce (see the
    /// struct doc comment's derivation). This says nothing about whether
    /// 934.7746 K / 903.3439 K are themselves accurate -- the fluid-node
    /// capacitance and the well-mixed-outlet assumption are new physics this
    /// struct adds, not yet checked against a reference the way the removed
    /// `PebbleBedCore`'s coefficient was.
    #[test]
    fn two_node_balance_settles_with_all_power_leaving_via_helium_throughflow() {
        let mut node = PebbleBedPorousMediaNode::new();
        let power = Power::new::<megawatt>(10.0);
        let inlet_k = 673.15;
        let inlet = helium_enthalpy_at(ThermodynamicTemperature::new::<kelvin>(inlet_k));
        let flow = nominal_helium_flow();
        // 3000 s of settling (16 bed time constants). 0.1 s x 30 000 since
        // 2026-09-29 (was 0.05 s x 60 000): the steady state does not depend
        // on dt, and the 5x5 solve made the old count cost 45 s.
        let dt = Time::new::<second>(0.1);
        let mut path = seeded_passive_path(&node);

        for _ in 0..30_000 {
            // CHANGED 2026-09-28: (net source to the bed, gross conduction
            // power for the pebble profile) -- the same 10 MW for both here.
            node.step(dt, power, power, inlet, flow, &mut path);
        }

        // CHANGED 2026-09-29 (gh:#395): the passive path is a row of the
        // bed's solve. The settled node therefore balances THROUGHFLOW plus
        // passive loss against the source; and since the helium itself loses
        // heat to the reflector through Achenbach's dispersion branch, the
        // interfacial exchange must exceed the throughflow by that branch.
        let passive = path.heat_from_core().get::<watt>();
        let removed = node.heat_to_helium().get::<watt>();

        // The throughflow on ENTHALPY since 2026-09-29 (gh:#393), not
        // `m_dot c_p (T_f - T_in)`.
        let t_f = node.helium_temperature().get::<kelvin>();
        let throughflow_removed = flow.get::<kilogram_per_second>()
            * (node.helium_outlet_enthalpy() - inlet).get::<joule_per_kilogram>();
        assert!(
            (throughflow_removed + passive - 1.0e7).abs() / 1.0e7 < 5.0e-3,
            "settled throughflow duty {throughflow_removed} W + passive {passive} W departs from \
             the 10 MW source"
        );
        assert!(
            removed > throughflow_removed,
            "the helium loses heat to the reflector (dispersion branch), so the interfacial \
             exchange {removed} W must exceed the throughflow {throughflow_removed} W"
        );
        println!(
            "settled passive loss {:.3} kW (to the reflector)",
            passive / 1e3
        );

        println!(
            "settled T_pebble = {:.4} K, T_helium = {:.4} K, heat_to_helium = {:.6} MW, \
             throughflow duty = {:.6} MW",
            node.pebble_temperature().get::<kelvin>(),
            t_f,
            removed / 1.0e6,
            throughflow_removed / 1.0e6,
        );
    }

    /// **One core state (gh:#393, 2026-09-29): the fluid node's void helium
    /// and the graphite node describe the same 197 cm, 27 000-pebble bed.**
    ///
    /// **Methodology.** The fluid row's void volume must equal
    /// [`bed_void_volume`] (`eps pi D^2 H / 4` on the published 1.8 m x 1.97 m
    /// bed, the geometry [`graphite_mass`] and [`pebble_count`] are drawn
    /// from), to 1e-12 relative, and must differ from the zone-99
    /// critical-loading void ([`super::super::htr10_rz_geometry::pebble_bed_helium_volume`]).
    ///
    /// **Fails before the change**: the node took zone 99, 1.2213 m^3 against
    /// the bed's 1.9551 m^3 (-37.5 %). **Results (2026-09-29):** 1.955085 m^3
    /// on both sides.
    #[test]
    fn the_fluid_and_graphite_nodes_describe_the_same_bed() {
        let node = PebbleBedPorousMediaNode::new();
        let v_node = node.helium_void_volume().get::<cubic_meter>();
        let v_bed = bed_void_volume().get::<cubic_meter>();
        let v_zone99 =
            super::super::htr10_rz_geometry::pebble_bed_helium_volume().get::<cubic_meter>();
        println!("fluid-node void {v_node:.6} m^3, 197 cm bed void {v_bed:.6} m^3, zone 99 void {v_zone99:.6} m^3");
        assert!((v_node - v_bed).abs() / v_bed < 1e-12);
        assert!((v_node - v_zone99).abs() / v_bed > 0.1);
    }

    /// V&V: the bed step closes its own energy balance, on the graphite's
    /// **true** enthalpy (2026-09-28).
    ///
    /// Replaces ~~`fission_power_and_decay_heat_power_sum_into_the_same_source_term`~~:
    /// the bed no longer takes fission and decay heat as separate arguments
    /// (they are deposited in the fuel node now, gh:#360), so there is no
    /// split left to test.
    ///
    /// **Methodology.** Step a node 3000 times at 0.1 s through a deliberately
    /// rough source history (10 MW, then -2 MW net -- a scrammed bed with the
    /// passive path drawing -- then 30 MW), at nominal and then 1 % flow. At
    /// every step require (1)
    /// `source = solid_storage + fluid_storage + throughflow_out` to 1e-9
    /// of the step's gross energy, and (2) `solid_storage` equal to the
    /// graphite mass times the **tuas** enthalpy change `h(T') - h(T)` to 1e-9
    /// relative -- the secant-capacitance fixed point must have converged.
    ///
    /// **Results (2026-09-28).** Worst step residual 1.3e-11 of the step's
    /// gross energy; solid storage vs true enthalpy change 2.1e-16; final bed
    /// 1157.8 K.
    #[test]
    fn the_bed_step_closes_its_own_energy_balance() {
        use uom::si::available_energy::joule_per_kilogram;
        let mut node = PebbleBedPorousMediaNode::new();
        let inlet = helium_enthalpy_at(ThermodynamicTemperature::new::<kelvin>(523.15));
        let dt = Time::new::<second>(0.1);
        let mut worst_balance = 0.0f64;
        let mut worst_enthalpy = 0.0f64;
        let mut worst_path = 0.0f64;
        let mut path = seeded_passive_path(&node);
        for i in 0..3000 {
            let (source_mw, flow_fraction) = match i {
                0..=999 => (10.0, 1.0),
                1000..=1999 => (-2.0, 0.01),
                _ => (30.0, 1.0),
            };
            let source = Power::new::<megawatt>(source_mw);
            let flow = MassRate::new::<kilogram_per_second>(
                flow_fraction * nominal_helium_flow_kg_per_s(),
            );
            let t_before = node.pebble_temperature();
            let path_e0 = path.stored_energy().get::<uom::si::energy::joule>();
            node.step(dt, source, source.abs(), inlet, flow, &mut path);
            let e = node.last_step_energy();
            let gross = e.source.abs()
                + e.solid_storage.abs()
                + e.fluid_storage.abs()
                + e.throughflow_out.abs()
                + e.to_passive_path.abs();
            worst_balance = worst_balance.max(
                (e.source
                    - e.solid_storage
                    - e.fluid_storage
                    - e.throughflow_out
                    - e.to_passive_path)
                    .abs()
                    / gross,
            );
            // Across the seam: what left the bed = reflector + RPV storage + RCCS.
            let path_storage = path.stored_energy().get::<uom::si::energy::joule>() - path_e0;
            let rccs = path.heat_to_rccs().get::<watt>() * dt.get::<second>();
            let risers = path.heat_to_risers().get::<watt>() * dt.get::<second>();
            worst_path =
                worst_path.max((e.to_passive_path - path_storage - rccs - risers).abs() / gross);
            let dh = (pebble_bed_specific_enthalpy_from_temperature(node.pebble_temperature())
                - pebble_bed_specific_enthalpy_from_temperature(t_before))
            .get::<joule_per_kilogram>()
                * graphite_mass().get::<kilogram>();
            if dh.abs() > 1.0 {
                worst_enthalpy = worst_enthalpy.max((e.solid_storage - dh).abs() / dh.abs());
            }
        }
        println!(
            "bed step energy balance over 3000 steps: worst residual {worst_balance:.3e} of the \
             step's gross energy; worst solid-storage vs true enthalpy {worst_enthalpy:.3e}; \
             worst bed->passive-path seam {worst_path:.3e}; final bed {:.3} K",
            node.pebble_temperature().get::<kelvin>()
        );
        assert!(worst_balance < 1e-9);
        assert!(worst_enthalpy < 1e-9);
        assert!(worst_path < 1e-9);
    }
}
