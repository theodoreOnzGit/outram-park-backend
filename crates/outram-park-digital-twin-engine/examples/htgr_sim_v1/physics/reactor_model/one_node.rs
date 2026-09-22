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
//! | Inside a single pebble | **1** | fuel kernel, graphite matrix and outer shell are one temperature |
//! | Helium in the bed | **0 (external)** | supplied by the caller as one bulk mean temperature |
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
//!   explicitly. The graphite thermal inertia it carries -- about 9.0 MJ/K over
//!   5.28 t of graphite -- is a genuine consequence of the published geometry
//!   and density, and it is what makes a pebble-bed core respond slowly.
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
//! - **Graphite `c_p` is one constant**, representative of graphite near
//!   1000 K. Real graphite `c_p` rises from about 710 J/(kg K) at 300 K to
//!   about 1700 J/(kg K) at 1000 K, so the constant is badly wrong cold and
//!   roughly right hot. No temperature- or fluence-dependent graphite property
//!   set exists in this workspace.
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
use uom::si::available_energy::joule_per_kilogram;
use uom::si::{f64::*, temperature_interval};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use outram_park_digital_twin_engine::htr10::kta;
use tampines::pebble_bed::pebble::{Pebble, PebbleTemperatureProfile};
use outram_foam_basic_lib::prelude::SquareMatrix;
use outram_park_fork_coolprop::{Fluid, FluidState, conductivity, state_pt, viscosity};
use uom::si::thermal_resistance::kelvin_per_watt;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::DynamicViscosity;
use uom::si::specific_heat_capacity::{kilojoule_per_kilogram_kelvin, joule_per_kilogram_kelvin};
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

/// Graphite isobaric specific heat \[J/(kg K)\], held **constant**
/// (illustrative). 1700 J/(kg K) is representative of nuclear graphite near
/// 1000 K, which is where this core operates; it is badly wrong below about
/// 600 K, where real graphite `c_p` falls toward 710 J/(kg K). A
/// temperature-dependent graphite property set is recorded as MISSING in
/// `docs/reactor-scoping/htr10.md` and is not implemented here.
pub const GRAPHITE_CP_J_PER_KG_K: f64 = 1700.0;

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

/// Reference temperature for the pebble enthalpy scale \[K\]: enthalpy is
/// defined zero at 298.15 K.
const REFERENCE_TEMPERATURE_K: f64 = 298.15;

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

/// Mass of one spherical fuel element \[kg\], graphite only (the 5 g of heavy
/// metal is under 3% of the ball and is not counted in the thermal mass).
pub fn pebble_mass() -> Mass {
    graphite_density() * pebble_volume()
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

/// Lumped thermal capacitance of the bed \[J/K\]: `m_graphite * c_p`, using the
/// constant graphite `c_p` above.
pub fn bed_heat_capacity() -> HeatCapacity {
    HeatCapacity::new::<joule_per_kelvin>(
        graphite_mass().get::<kilogram>() * GRAPHITE_CP_J_PER_KG_K,
    )
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
pub fn resolved_pebble() -> Pebble {
    Pebble::htr10()
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
/// Outside `tampines`' 300-2000 K correlation window, or if either fixed-point
/// iteration fails, this falls back to the retired uniform-ball form and says
/// so in the returned value's provenance only by being finite — the caller
/// cannot tell. That is deliberate: a plant step must not panic on a transient
/// excursion, and the fallback is the *old* model, which was usable. The V&V
/// test pins the normal path.
pub fn intra_pebble_conduction_coefficient(
    node_temperature: ThermodynamicTemperature,
    pebble_power: Power,
) -> HeatTransfer {
    let power = floored_pebble_power(pebble_power);
    conduction_coefficient_from(
        resolved_pebble_profile(node_temperature, power).as_ref(),
        power,
    )
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
/// volume-average rise `r`, set `T_s <- T_node - r`, repeat. The rise depends
/// on the surface only through `k(T)`, so this contracts hard and converges in
/// two or three passes; 1e-6 K is the tolerance and 12 passes the cap.
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
    const TOLERANCE_K: f64 = 1.0e-6;
    const MAX_PASSES: usize = 12;

    let pebble = resolved_pebble();
    let power = floored_pebble_power(pebble_power);
    let node_k = node_temperature.get::<kelvin>();
    let mut surface_k = node_k;

    for _ in 0..MAX_PASSES {
        let profile = pebble
            .steady_state_temperatures(
                power,
                ThermodynamicTemperature::new::<kelvin>(surface_k),
                Ratio::new::<ratio>(0.0),
            )
            .ok()?;
        let rise = pebble.volume_average_temperature(&profile).get::<kelvin>() - surface_k;
        if !rise.is_finite() {
            return None;
        }
        let next = node_k - rise;
        if (next - surface_k).abs() < TOLERANCE_K {
            // Return the profile solved AT the converged surface, not the one
            // that produced it -- otherwise the reported surface and the
            // reported interior are one pass out of step.
            return pebble
                .steady_state_temperatures(
                    power,
                    ThermodynamicTemperature::new::<kelvin>(next),
                    Ratio::new::<ratio>(0.0),
                )
                .ok();
        }
        surface_k = next;
    }
    None
}

/// The **kernel-above-node** thermal resistance implied by an already-solved
/// profile \[K/W of per-pebble power\], so a caller that needs the profile
/// anyway does not solve it twice.
///
/// ```text
/// R_kernel = (T_peak_kernel_centre - T_node) / P_pebble
/// ```
///
/// `T_node` is the ball's **volume average**, which is what
/// [`resolved_pebble_profile`] inverts for and therefore what the bed node's
/// capacitance describes -- so this resistance and the node temperature refer
/// to the same pebble, and their sum is the kernel. See
/// [`PebbleBedPorousMediaNode::kernel_offset_resistance`] for why the slope is
/// what crosses the module boundary rather than the temperature.
///
/// Returns `None` when there is no profile (outside the 300-2000 K correlation
/// window) or when the power is too small for the quotient to mean anything.
/// **There is no fallback here, deliberately**: the retired uniform ball had
/// no kernel at all, so inventing a resistance for it would be inventing the
/// very quantity this exists to supply. A `None` makes the Doppler channel
/// fall back to the pre-2026-09-22 behaviour -- the bed node carrying the
/// whole isothermal coefficient -- which is a model that was in service, not a
/// fabrication. See [`crate::physics::kinetics::KernelDopplerChannel`].
fn kernel_offset_resistance_from(
    profile: Option<&PebbleTemperatureProfile>,
    node_temperature: ThermodynamicTemperature,
    pebble_power: Power,
) -> Option<ThermalResistance> {
    // The same floor the conduction leg uses, so the two legs of one profile
    // are never evaluated at different powers.
    let power_w = floored_pebble_power(pebble_power).get::<watt>();
    if power_w <= 0.0 {
        return None;
    }
    let rise_k = profile?.peak_kernel_centre.get::<kelvin>() - node_temperature.get::<kelvin>();
    if !rise_k.is_finite() || rise_k <= 0.0 {
        return None;
    }
    Some(ThermalResistance::new::<kelvin_per_watt>(rise_k / power_w))
}

/// The conduction coefficient implied by an already-solved profile, so a
/// caller that needs the profile anyway does not solve it twice.
///
/// Falls back to the retired uniform-ball `10 k / d` when there is no profile.
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
        // Retired uniform-ball form, kept only as the out-of-range fallback.
        None => {
            let d = pebble_diameter().get::<meter>();
            HeatTransfer::new::<watt_per_square_meter_kelvin>(
                10.0 * LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K / d,
            )
        }
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

/// Helium thermal conductivity \[W/(m K)\], Prandtl number and dynamic
/// viscosity \[Pa s\] at `temperature` and the primary-loop pressure, from the
/// real CoolProp-derived helium models.
///
/// Falls back to representative helium values if a transport model or the
/// density solve declines (the same defensive shape
/// [`super::super::primary_loop`] uses): a GUI frame must not panic on a transient
/// excursion, and helium at these conditions is close enough to ideal that the
/// fallback is a sane bound rather than a fabricated number.
fn helium_transport(temperature: ThermodynamicTemperature) -> (f64, f64, f64) {
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

/// Graphite specific enthalpy at `temperature`, `c_p (T - 298.15 K)` with the
/// constant [`GRAPHITE_CP_J_PER_KG_K`].
pub fn pebble_bed_specific_enthalpy_from_temperature(
    temperature: ThermodynamicTemperature,
) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(
        GRAPHITE_CP_J_PER_KG_K * (temperature.get::<kelvin>() - REFERENCE_TEMPERATURE_K),
    )
}
pub fn helium_specific_enthalpy_from_temperature(
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(
        GRAPHITE_CP_J_PER_KG_K * (temperature.get::<kelvin>() - REFERENCE_TEMPERATURE_K),
    )
}

/// Inverse of [`specific_enthalpy_from_temperature`]: closed form, since the
/// constant-`c_p` relation is linear and needs no iteration.
pub fn temperature_from_specific_enthalpy(
    specific_enthalpy: AvailableEnergy,
) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(
        REFERENCE_TEMPERATURE_K
            + specific_enthalpy.get::<joule_per_kilogram>() / GRAPHITE_CP_J_PER_KG_K,
    )
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
/// `C_s` is [`bed_heat_capacity`] (unchanged). `C_f` is the thermal mass of
/// the helium actually held in the bed's void space at the current density,
/// `rho_He(T_f, p) * V_void * c_p(T_f)`, with `V_void` this struct's own
/// [`Self`]`::pebble_bed_helium_volume` field -- see
/// [`fluid_node_heat_capacity`]. `rho_He` and `c_p(T_f)` are read straight
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
    /// Pebble (solid-phase) temperature, carried directly rather than
    /// through a specific-enthalpy state, since this node's `c_p` is
    /// already constant ([`GRAPHITE_CP_J_PER_KG_K`]).
    pebble_temperature: ThermodynamicTemperature,
    /// Full thermodynamic state of the helium held in this node's void
    /// space -- not just a bare temperature. See the module comment above
    /// for why the whole state is stored: the fluid-phase balance needs
    /// density and `c_p` as well as temperature, all mutually consistent.
    helium_state: FluidState,
    /// Helium gas volume in the void space between packed pebbles -- from
    /// [`super::htr10_rz_geometry::pebble_bed_helium_volume`], with the
    /// same NOT-VALIDATED caveat that derivation carries. Fixed at
    /// construction: a geometric property of the benchmark core, not plant
    /// state.
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
    /// Kernel-above-node thermal resistance from the most recent step's
    /// resolved pebble solve \[K/W of *per-pebble* power\], or `None` when the
    /// solve was out of range. See [`Self::kernel_offset_resistance`] -- this
    /// is what lets the Doppler channel follow the kernel at the PROMPT
    /// timescale without re-solving the pebble on every kinetics substep.
    kernel_offset_resistance: Option<ThermalResistance>,
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
            pebble_bed_helium_volume: super::htr10_rz_geometry::pebble_bed_helium_volume(),
            heat_to_helium: Power::new::<watt>(0.0),
            overall_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(
                LEGACY_LUMPED_HTC_W_PER_M2_K,
            ),
            peak_kernel_temperature: None,
            pebble_profile: None,
            kernel_offset_resistance: None,
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
    /// `fission_power` and `decay_heat_power` are taken as SEPARATE
    /// arguments and summed internally into the single source term `Q` the
    /// derivation's solid balance uses (`C_s dT_s/dt = Q - h A (T_s - T_f)`
    /// becomes `Q = fission_power + decay_heat_power`). This mirrors
    /// `mod.rs`'s own wiring of this method -- see its "2.
    /// Pebble bed absorbs the core's THERMAL power" comment, which passes
    /// [`kinetics::Kinetics::core_thermal_power`]'s fission-plus-decay sum
    /// as that method's `fission_power` parameter -- but makes the
    /// requirement explicit in the signature here rather than relying on
    /// the caller to have pre-summed it under a fission-only name. Decay
    /// heat is what keeps this term (and hence `T_s`) nonzero after a trip,
    /// when [`kinetics::Kinetics::decay_heat_power`] is the only thing
    /// still heating the bed.
    ///
    /// # Panics
    ///
    /// If the helium `(T, p)` flash at the solved fluid-node temperature
    /// fails to converge -- same stale-state policy as [`Self::new`].
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        decay_heat_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        // 1. Coefficient and both capacitances are evaluated at the
        //    CURRENT (start-of-step) state -- same start-of-step evaluation
        //    PebbleBedCore::step uses for its own coefficient.
        let helium_temperature_now =
            ThermodynamicTemperature::new::<kelvin>(self.helium_state.temperature);
        // The intra-pebble leg needs the bed's own temperature and the power
        // actually being generated, so the source term is summed here rather
        // than at step 4 -- same value, just needed earlier now.
        let reactor_thermal_power = fission_power + decay_heat_power;
        // One resolved-pebble solve per step, reused for BOTH the conduction
        // leg and the peak kernel temperature. Going through
        // `overall_htc_at_flow` here would solve it a second time for a
        // number this already has.
        let pebble_power = reactor_thermal_power / pebble_count();
        let profile = resolved_pebble_profile(self.pebble_temperature, pebble_power);
        let h_internal = conduction_coefficient_from(profile.as_ref(), pebble_power);
        let htc = series_coefficient(
            film_htc_at_flow(helium_mass_flow, helium_temperature_now),
            h_internal,
        );
        let conductance: ThermalConductance = htc * heat_transfer_area();

        // 2. C_s (unchanged) and C_f, the latter read straight off the
        //    stored FluidState rather than re-evaluated.
        let solid_capacity = bed_heat_capacity();
        let fluid_capacity =
            fluid_node_heat_capacity(self.helium_state, self.pebble_bed_helium_volume);

        // 3. m_dot c_p, floored the same way PebbleBedCore::step floors its
        //    capacity rate so a stopped circulator does not divide by zero.
        let flow_floor = MassRate::new::<kilogram_per_second>(1.0e-6);
        let capacity_rate: ThermalConductance = helium_mass_flow.abs().max(flow_floor)
            * SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.helium_state.cp);

        // 4. Assemble and solve the 2x2 system. The source term is the SUM
        //    -- see the doc comment above for why decay heat is a separate
        //    argument rather than folded into `fission_power` by the caller.
        let (matrix, rhs) = assemble_backward_euler_system(
            dt,
            reactor_thermal_power,
            helium_inlet_temperature,
            self.pebble_temperature,
            helium_temperature_now,
            conductance,
            solid_capacity,
            fluid_capacity,
            capacity_rate,
        );
        let solved = matrix.solve(&rhs).expect(
            "the backward-Euler matrix is diagonally dominant by construction (positive \
             capacitance-over-dt and conductance terms on every diagonal) and is never \
             singular -- see the struct doc comment",
        );

        // 5. Store the solved solid temperature directly...
        self.pebble_temperature = ThermodynamicTemperature::new::<kelvin>(solved[0]);
        // ...and re-flash the FULL helium state at the solved fluid
        // temperature, so density and c_p going into the NEXT step's
        // coefficient and capacitance are consistent with the new
        // temperature rather than carried over stale from this step.
        let pressure_pa = design().primary_pressure.get::<uom::si::pressure::pascal>();
        self.helium_state = state_pt(Fluid::Helium, solved[1], pressure_pa)
            .expect("helium (T,p) flash failed to converge at the solved fluid-node temperature");

        // 6. Publish the exchanged heat rate and the coefficient used.
        let delta = TemperatureInterval::new::<temperature_interval::kelvin>(solved[0] - solved[1]);
        self.heat_to_helium = conductance * delta;
        self.overall_htc = htc;
        // The kernel sits on the profile solved at the START of the step, so
        // it is reported against that step's bed temperature, consistent with
        // the coefficient it was solved alongside.
        self.peak_kernel_temperature = profile.map(|p| p.peak_kernel_centre);
        self.pebble_profile = profile;
        // The same profile read as a RESISTANCE rather than a temperature, so
        // the Doppler channel can follow the kernel between pebble solves.
        self.kernel_offset_resistance =
            kernel_offset_resistance_from(profile.as_ref(), self.pebble_temperature, pebble_power);

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
    /// [`crate::physics::kinetics::KernelDopplerChannel`] takes the *fuel*
    /// share of the published isothermal coefficient and applies it to the
    /// kernel, leaving the graphite share on this node; and
    /// [`crate::physics::fission_product_release`] drives TRISO-ATOPS off the
    /// same temperature. Both reach it through
    /// [`Self::kernel_offset_resistance`] rather than this accessor, so they
    /// can follow the kernel between pebble solves.
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

    /// The kernel-above-node thermal resistance from the most recent step
    /// \[K per watt of **per-pebble** power\], or `None` when the resolved
    /// solve was out of range.
    ///
    /// ## What this is for
    ///
    /// [`Self::peak_kernel_temperature`] is a temperature at one instant,
    /// re-solved once per *plant* step. The Doppler feedback needs the kernel
    /// at the **prompt** timescale -- the whole reason the kernel is the right
    /// temperature for it is that a quarter-millimetre UO2 kernel follows a
    /// power change essentially instantly, while the 5.3-tonne graphite bed
    /// takes minutes (the separation `tampines::pebble_bed::feedback`'s module
    /// doc is built around). A kernel temperature refreshed at 0.1 s and held
    /// constant across 100 kinetics substeps would *lag* the power, which is
    /// the opposite of the physics being added.
    ///
    /// So what crosses the boundary is the **slope**, not the value:
    ///
    /// ```text
    /// T_kernel(t) = T_node + R_kernel * P_pebble(t)
    /// ```
    ///
    /// `R_kernel` carries the slow, weakly non-linear part -- the bed
    /// temperature and A3 graphite's `k(T)` -- and is refreshed once per plant
    /// step, exactly like every other coupling variable in
    /// [`crate::physics::HtgrPlant::step_with_correctors`]. `P_pebble(t)` is
    /// the *instantaneous* power, read on every kinetics substep. Steady
    /// conduction is linear in power, so this is exact in the linear limit and
    /// the only approximation is holding `k(T)` fixed over one 0.1 s step;
    /// [`tests::the_kernel_offset_is_linear_in_power`] measures what that
    /// costs.
    ///
    /// # Why a resistance and not a temperature interval
    ///
    /// Because it is one: kelvin per watt, the conduction resistance from the
    /// hottest kernel centre out to the ball's volume average. Typing it as
    /// [`ThermalResistance`] makes multiplying it by anything other than a
    /// power a compile error.
    pub fn kernel_offset_resistance(&self) -> Option<ThermalResistance> {
        self.kernel_offset_resistance
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

/// Thermal capacitance of the helium held in a bed void volume
/// `void_volume` at thermodynamic state `helium_state` \[J/K\]:
/// `rho * V_void * c_p`. This is `C_f` in the
/// [`PebbleBedPorousMediaNode`] derivation.
///
/// Takes the already-evaluated [`FluidState`] rather than a bare
/// temperature so the density and `c_p` it reads are guaranteed consistent
/// with each other and with whatever temperature that state was flashed
/// at -- see the module comment above [`PebbleBedPorousMediaNode`] for why
/// that matters.
fn fluid_node_heat_capacity(helium_state: FluidState, void_volume: Volume) -> HeatCapacity {
    HeatCapacity::new::<joule_per_kelvin>(
        helium_state.density * void_volume.get::<cubic_meter>() * helium_state.cp,
    )
}

/// Assemble the 2x2 backward-Euler coefficient matrix and right-hand side
/// for one [`PebbleBedPorousMediaNode::step`]. See the struct doc comment
/// for the derivation these four matrix entries and two right-hand-side
/// entries come from.
///
/// Row/column order is `[T_pebble^{n+1}, T_helium^{n+1}]` for both the
/// matrix and the returned right-hand side, so `matrix.solve(&rhs)[0]` is
/// the solved pebble temperature and `[1]` is the solved helium
/// temperature, both in kelvin.
///
/// `reactor_thermal_power` is the ALREADY-SUMMED source term `Q` -- fission
/// power plus fission-product decay heat, `Q_fission` in the derivation --
/// so this function does not itself know or care how that sum was formed;
/// see [`PebbleBedPorousMediaNode::step`]'s doc comment for why the split
/// is kept at the call site instead.
fn assemble_backward_euler_system(
    dt: Time,
    reactor_thermal_power: Power,
    helium_inlet_temperature: ThermodynamicTemperature,
    pebble_temperature: ThermodynamicTemperature,
    helium_temperature: ThermodynamicTemperature,
    conductance: ThermalConductance,
    solid_capacity: HeatCapacity,
    fluid_capacity: HeatCapacity,
    capacity_rate: ThermalConductance,
) -> (SquareMatrix, [f64; 2]) {
    let dt_s = dt.get::<second>();
    let h_a = conductance.get::<watt_per_kelvin>();
    let c_s = solid_capacity.get::<joule_per_kelvin>();
    let c_f = fluid_capacity.get::<joule_per_kelvin>();
    let m_dot_cp = capacity_rate.get::<watt_per_kelvin>();

    let t_s_n = pebble_temperature.get::<kelvin>();
    let t_f_n = helium_temperature.get::<kelvin>();
    let t_in = helium_inlet_temperature.get::<kelvin>();
    let q_source = reactor_thermal_power.get::<watt>();

    let mut matrix = SquareMatrix::new(2);
    matrix.set(0, 0, c_s / dt_s + h_a);
    matrix.set(0, 1, -h_a);
    matrix.set(1, 0, -h_a);
    matrix.set(1, 1, c_f / dt_s + h_a + m_dot_cp);

    let rhs = [
        c_s / dt_s * t_s_n + q_source,
        c_f / dt_s * t_f_n + m_dot_cp * t_in,
    ];

    (matrix, rhs)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    /// `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`.
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
        let uniform_ball = 10.0 * LEGACY_GRAPHITE_MATRIX_CONDUCTIVITY_W_PER_M_K
            / pebble_diameter().get::<meter>();

        let u_resolved = overall_htc_at_flow(nominal_helium_flow(), helium, bed, rated)
            .get::<watt_per_square_meter_kelvin>();
        let h_film = film_htc_at_flow(nominal_helium_flow(), helium)
            .get::<watt_per_square_meter_kelvin>();
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
    /// [`PebbleBedPorousMediaNode::kernel_offset_resistance`] publishes
    /// `R_kernel = dT/P`, refreshed once per 0.1 s plant step, and
    /// [`crate::physics::kinetics::KernelDopplerChannel`] then evaluates
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
                let r =
                    (profile.peak_kernel_centre.get::<kelvin>() - node_k) / power.get::<watt>();
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

                let reproduced = pebble
                    .volume_average_temperature(&profile)
                    .get::<kelvin>();
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

        let stopped =
            overall_htc_at_flow(MassRate::new::<kilogram_per_second>(0.0), helium, bed, rated)
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
    /// **Interpretation.** The two independent routes to the same duty --
    /// the interfacial exchange `h A (T_s - T_f)` and the throughflow
    /// `m_dot c_p (T_f - T_in)` -- agree at steady state, which is exactly
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
        let no_decay_heat = Power::new::<watt>(0.0);
        let inlet_k = 673.15;
        let inlet = ThermodynamicTemperature::new::<kelvin>(inlet_k);
        let flow = nominal_helium_flow();
        let dt = Time::new::<second>(0.05);

        for _ in 0..60_000 {
            node.step(dt, power, no_decay_heat, inlet, flow);
        }

        let removed = node.heat_to_helium().get::<watt>();
        assert!(
            (removed - 1.0e7).abs() / 1.0e7 < 1.0e-3,
            "settled exchanged heat {removed} W does not match the 10 MW source"
        );

        let cp = node.helium_state().cp;
        let t_f = node.helium_temperature().get::<kelvin>();
        let throughflow_removed = flow.get::<kilogram_per_second>() * cp * (t_f - inlet_k);
        assert!(
            (throughflow_removed - 1.0e7).abs() / 1.0e7 < 5.0e-3,
            "settled throughflow duty {throughflow_removed} W departs from the 10 MW source"
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

    /// V&V: `fission_power` and `decay_heat_power` must enter
    /// [`PebbleBedPorousMediaNode::step`]'s balance identically -- only
    /// their SUM matters, which is the whole point of
    /// [`PebbleBedPorousMediaNode::step`]'s doc comment taking decay heat as
    /// a separate argument rather than trusting the caller to have
    /// pre-summed it.
    ///
    /// **Methodology.** Two fresh nodes are stepped for the same 3000 s at
    /// the same 673.15 K inlet and nominal flow: one with the full 10 MW as
    /// `fission_power` and zero decay heat, the other with the same total
    /// split 4 MW fission / 6 MW decay heat. Pass criterion: the two end
    /// states agree to within floating-point roundoff on both `T_pebble`
    /// and `T_helium`.
    ///
    /// **Results (2026-08-17):** both temperatures agreed EXACTLY (0.0 K
    /// difference, bit-for-bit) after 60,000 steps. This is stronger than
    /// "close": `4.0 MW + 6.0 MW` and `10.0 MW + 0.0 MW` both round to the
    /// exact f64 value `1.0e7` (all four inputs are exactly representable
    /// integers of watts well under 2^53), so the two runs solve the
    /// IDENTICAL linear system at every one of the 60,000 steps, not merely
    /// a numerically close one.
    #[test]
    fn fission_power_and_decay_heat_power_sum_into_the_same_source_term() {
        let inlet = ThermodynamicTemperature::new::<kelvin>(673.15);
        let flow = nominal_helium_flow();
        let dt = Time::new::<second>(0.05);

        let mut all_fission = PebbleBedPorousMediaNode::new();
        let mut split = PebbleBedPorousMediaNode::new();
        for _ in 0..60_000 {
            all_fission.step(
                dt,
                Power::new::<megawatt>(10.0),
                Power::new::<watt>(0.0),
                inlet,
                flow,
            );
            split.step(
                dt,
                Power::new::<megawatt>(4.0),
                Power::new::<megawatt>(6.0),
                inlet,
                flow,
            );
        }

        let t_s_diff = (all_fission.pebble_temperature().get::<kelvin>()
            - split.pebble_temperature().get::<kelvin>())
        .abs();
        let t_f_diff = (all_fission.helium_temperature().get::<kelvin>()
            - split.helium_temperature().get::<kelvin>())
        .abs();
        println!("t_s_diff = {t_s_diff:e} K, t_f_diff = {t_f_diff:e} K");
        assert!(
            t_s_diff < 1.0e-6,
            "solid temperatures diverged: {t_s_diff} K -- the fission/decay-heat split must not matter"
        );
        assert!(
            t_f_diff < 1.0e-6,
            "fluid temperatures diverged: {t_f_diff} K -- the fission/decay-heat split must not matter"
        );
    }
}
