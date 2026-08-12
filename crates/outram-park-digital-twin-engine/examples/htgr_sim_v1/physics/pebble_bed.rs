//! Pebble-bed core -- a **lumped placeholder**, HTR-10-shaped.
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
//! [`PebbleBedThermalHydraulics`]: ../../fhr_sim_v2/app/prke_backend/pebble_bed_thermal_hydraulics.rs
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
//! - a **peak fuel temperature** -- [`PebbleBedCore::temperature`] is a bed
//!   average, and quoting it as a fuel temperature limit would be wrong;
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
//! reflector node come after both, and each of those needs the effective bed
//! conductivity that this workspace does not yet have.
//!
//! ## What is real
//!
//! - **The bed geometry is the published HTR-10 core**, taken from
//!   IAEA-TECDOC-1382 (*Evaluation of high temperature gas cooled reactor
//!   performance: Benchmark analysis related to initial testing of the HTTR and
//!   HTR-10*), ingested in this workspace at
//!   `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`.
//!   Core diameter 1.8 m, mean height 1.97 m, 27,000 spherical fuel elements of
//!   6.0 cm diameter, volumetric filling fraction of balls 0.61 (void fraction
//!   0.39), graphite density in the matrix and outer shell 1.73 g/cm^3, heavy
//!   metal 5.0 g per ball. Everything geometric below is *derived from those
//!   figures*, not chosen: the bed volume, the free-flow area, the total pebble
//!   surface area, and the graphite mass.
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
//! - **There is no packed-bed friction correlation here.** KTA and Ergun are
//!   both still absent from this workspace; `docs/reactor-scoping/htr10.md`
//!   records that gap and it remains open. The primary loop's pressure drop is
//!   a quadratic loss anchored to the published circulator head -- see
//!   [`super::primary_loop`].
//! - **There is no effective bed conductivity.** No Zehner-Bauer-Schlunder, no
//!   solid/gas/contact/radiation split, no wall-region correction. The bed is
//!   one node, so it has no radial or axial temperature profile at all.
//! - **There is no radial pebble conduction.** The fuel zone, the graphite
//!   shell and the pebble surface are one temperature. The single
//!   [`NOMINAL_OVERALL_HTC_W_PER_M2_K`] therefore lumps the internal pebble
//!   conduction *and* the surface film together, and its value is chosen to put
//!   the pebble-to-helium difference in a plausible range -- it is **not**
//!   evaluated from a Nusselt correlation. Only its flow *exponent* (0.6) is
//!   taken from the Wakao packed-bed correlation.
//! - **Graphite `c_p` is one constant**, representative of graphite near
//!   1000 K. Real graphite `c_p` rises from about 710 J/(kg K) at 300 K to
//!   about 1700 J/(kg K) at 1000 K, so the constant is badly wrong cold and
//!   roughly right hot. No temperature- or fluence-dependent graphite property
//!   set exists in this workspace.
//! - **The illustrative constants are grouped**, deliberately, in the
//!   `Illustrative closure constants` block below, so no invented number is
//!   mixed in with the published geometry above it. Replacing the invented
//!   figures with sourced ones is tracked as bead `op-szmi.6`.
//! - **There is no multi-pass pebble flow, no burnup distribution, and no
//!   reflector, barrel or cavity-cooling path.** The HTR-10's passive
//!   decay-heat route is not modelled.
//!
//! It is an offline demonstration model. It is **not** a validated pebble-bed
//! core model and must not be used for any purpose `RESPONSIBLE_USE.md`
//! excludes.

// The geometry helpers and state accessors below are the module's public
// surface: they exist so the app layer can put bed quantities on the snapshot
// and so a future nodalised bed can reuse the derived geometry. Not every one
// has a caller inside this example yet.
#![allow(dead_code)]

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, HeatCapacity, HeatTransfer, Length, Mass, MassRate, Power, Ratio,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

// ---------------------------------------------------------------------------
// Published HTR-10 core geometry (IAEA-TECDOC-1382, Table 4-1 and section 4.1)
// ---------------------------------------------------------------------------

/// Pebble-bed core diameter \[m\]: 180 cm (published).
pub const CORE_DIAMETER_M: f64 = 1.80;

/// Mean pebble-bed height \[m\]: 197 cm (published).
pub const CORE_MEAN_HEIGHT_M: f64 = 1.97;

/// Number of spherical fuel elements in the equilibrium core: 27,000
/// (published).
pub const PEBBLE_COUNT: f64 = 27_000.0;

/// Spherical fuel-element diameter \[m\]: 6.0 cm (published).
pub const PEBBLE_DIAMETER_M: f64 = 0.06;

/// Bed void fraction (porosity), dimensionless: `1 - 0.61` from the published
/// volumetric filling fraction of balls in the core, 0.61.
///
/// The same 0.39 void fraction is what the benchmark participants were required
/// to preserve when idealising the random packing as a lattice.
pub const BED_POROSITY: f64 = 0.39;

/// Density of the graphite matrix and outer shell of a fuel element
/// \[kg/m^3\]: 1.73 g/cm^3 (published).
pub const GRAPHITE_DENSITY_KG_PER_M3: f64 = 1730.0;

/// Heavy-metal (uranium) loading per fuel element \[kg\]: 5.0 g (published).
/// Carried for completeness -- the lumped thermal model treats the pebble as
/// graphite, since the heavy metal is under 3% of the ball mass.
pub const HEAVY_METAL_PER_PEBBLE_KG: f64 = 5.0e-3;

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

/// Overall pebble-to-helium heat-transfer coefficient at the nominal helium
/// flow \[W/(m^2 K)\] (**illustrative**).
///
/// This single number lumps together the internal pebble conduction (fuel zone
/// to ball surface) and the surface film, because the bed is one node. Its
/// value is chosen so that at nominal power the bed sits a few hundred kelvin
/// above the bulk helium -- a plausible band for a graphite pebble at 10 MWth
/// over the derived 305 m^2 of pebble surface -- **not** computed from a
/// Nusselt correlation. Do not cite it as a heat-transfer result.
pub const NOMINAL_OVERALL_HTC_W_PER_M2_K: f64 = 160.0;

/// Reynolds-number exponent used to scale the overall coefficient with helium
/// flow, dimensionless.
///
/// 0.6 is the Reynolds exponent of the **Wakao** packed-bed particle-to-fluid
/// Nusselt correlation `Nu = 2 + 1.1 Re_p^0.6 Pr^(1/3)`, which is already
/// available in this workspace at
/// `crates/tuas_boussinesq_solver/.../nusselt_number_correlations/input_structs.rs:152`.
/// **Only the exponent is borrowed.** No Reynolds number, Prandtl number or gas
/// conductivity is evaluated here, so this is a flow-sensitivity shape, not an
/// evaluated Wakao Nusselt number.
pub const HTC_FLOW_EXPONENT: f64 = 0.6;

/// Nominal helium mass flow the overall coefficient is anchored at \[kg/s\]:
/// 4.3 kg/s at full power (published).
pub const NOMINAL_HELIUM_FLOW_KG_PER_S: f64 = 4.3;

/// Reference temperature for the pebble enthalpy scale \[K\]: enthalpy is
/// defined zero at 298.15 K.
const REFERENCE_TEMPERATURE_K: f64 = 298.15;

/// Bed temperature the model is seeded at \[K\] (illustrative, ~677 degC).
///
/// Chosen as the settled full-power bed average so the simulator opens near its
/// operating point instead of spending ten minutes of simulated time warming
/// 5.3 t of graphite up from cold.
const SEED_BED_TEMPERATURE_K: f64 = 950.0;

// ---------------------------------------------------------------------------
// Derived geometry -- computed from the published figures above
// ---------------------------------------------------------------------------

/// Bed cylinder volume `pi D^2 H / 4` \[m^3\], derived from the published core
/// diameter and mean height. Comes out at 5.0130 m^3 against the report's own
/// stated 5.0 m^3.
pub fn bed_volume() -> Volume {
    Volume::new::<cubic_meter>(
        std::f64::consts::PI * 0.25 * CORE_DIAMETER_M * CORE_DIAMETER_M * CORE_MEAN_HEIGHT_M,
    )
}

/// Helium-filled void volume in the bed, `epsilon * V_bed` \[m^3\].
pub fn bed_void_volume() -> Volume {
    bed_volume() * Ratio::new::<ratio>(BED_POROSITY)
}

/// Superficial (empty-cylinder) cross-sectional area of the bed \[m^2\].
pub fn superficial_area() -> Area {
    Area::new::<square_meter>(std::f64::consts::PI * 0.25 * CORE_DIAMETER_M * CORE_DIAMETER_M)
}

/// Free-flow (interstitial) area available to the helium, `epsilon * A`
/// \[m^2\].
pub fn free_flow_area() -> Area {
    superficial_area() * Ratio::new::<ratio>(BED_POROSITY)
}

/// Volume of one spherical fuel element `pi d^3 / 6` \[m^3\].
pub fn pebble_volume() -> Volume {
    Volume::new::<cubic_meter>(
        std::f64::consts::PI * PEBBLE_DIAMETER_M * PEBBLE_DIAMETER_M * PEBBLE_DIAMETER_M / 6.0,
    )
}

/// Mass of one spherical fuel element \[kg\], graphite only (the 5 g of heavy
/// metal is under 3% of the ball and is not counted in the thermal mass).
pub fn pebble_mass() -> Mass {
    Mass::new::<kilogram>(GRAPHITE_DENSITY_KG_PER_M3 * pebble_volume().get::<cubic_meter>())
}

/// Total graphite mass held in the bed \[kg\]: `N * m_pebble`.
pub fn graphite_mass() -> Mass {
    pebble_mass() * PEBBLE_COUNT
}

/// Total pebble surface area available for heat transfer to the helium
/// \[m^2\]: `N pi d^2`.
pub fn heat_transfer_area() -> Area {
    Area::new::<square_meter>(
        PEBBLE_COUNT * std::f64::consts::PI * PEBBLE_DIAMETER_M * PEBBLE_DIAMETER_M,
    )
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
    PEBBLE_COUNT * pebble_volume().get::<cubic_meter>() / bed_volume().get::<cubic_meter>()
}

/// Pebble diameter as a [`Length`], for callers that want it typed (it is the
/// characteristic length any future packed-bed correlation would use).
pub fn pebble_diameter() -> Length {
    Length::new::<meter>(PEBBLE_DIAMETER_M)
}

// ---------------------------------------------------------------------------
// The lumped core
// ---------------------------------------------------------------------------

/// Lumped pebble-bed core: one graphite-matrix enthalpy state for all 27,000
/// fuel elements.
///
/// Hold this next to the helium loop and step it *before* the loop, handing the
/// returned heat rate to the loop in place of the raw fission power -- that is
/// what gives the primary loop the graphite's thermal inertia (see
/// [`super::HtgrPlant::step`]).
#[derive(Clone, Copy, Debug)]
pub struct PebbleBedCore {
    /// Specific enthalpy of the lumped graphite pebble, zero at 298.15 K.
    specific_enthalpy: AvailableEnergy,
    /// Heat rate handed to the helium on the most recent step.
    heat_to_coolant: Power,
    /// Overall coefficient actually used on the most recent step (after the
    /// flow scaling).
    overall_htc: HeatTransfer,
}

impl PebbleBedCore {
    /// Construct the bed seeded at its nominal full-power average temperature
    /// (~677 degC), so the simulator opens near the operating point.
    pub fn new() -> Self {
        Self {
            specific_enthalpy: specific_enthalpy_from_temperature(ThermodynamicTemperature::new::<
                kelvin,
            >(
                SEED_BED_TEMPERATURE_K
            )),
            heat_to_coolant: Power::new::<watt>(0.0),
            overall_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(
                NOMINAL_OVERALL_HTC_W_PER_M2_K,
            ),
        }
    }

    /// Advance the bed by `dt` and return the heat rate handed to the helium.
    ///
    /// The balance solved is, explicitly in time,
    ///
    /// `Q_removed = h(m_dot) A (T_pebble - T_helium)`
    ///
    /// `C dT_pebble/dt = Q_fission - Q_removed`
    ///
    /// with `C = m_graphite c_p` from the published geometry, `A` the total
    /// pebble surface area, and `h` the illustrative overall coefficient scaled
    /// by `(m_dot/m_dot_nominal)^0.6`.
    ///
    /// `helium_bulk_temperature` is the bulk mean helium temperature the bed
    /// sees, i.e. `(T_core_in + T_core_out)/2` from the primary loop. A
    /// negative return value is physically meaningful -- it means the helium is
    /// hotter than the graphite and is heating it, which is what happens on a
    /// cold start.
    ///
    /// The explicit step is unconditionally stable for the timesteps this
    /// simulator uses: the bed time constant `C/(hA)` is about 184 s against a
    /// 0.05 s step.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_bulk_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        let htc = overall_htc_at_flow(helium_mass_flow);
        self.overall_htc = htc;

        let conductance =
            htc.get::<watt_per_square_meter_kelvin>() * heat_transfer_area().get::<square_meter>();
        let t_pebble_k = self.temperature().get::<kelvin>();
        let t_helium_k = helium_bulk_temperature.get::<kelvin>();

        let removed_w = conductance * (t_pebble_k - t_helium_k);
        self.heat_to_coolant = Power::new::<watt>(removed_w);

        let net_w = fission_power.get::<watt>() - removed_w;
        let mass_kg = graphite_mass().get::<kilogram>();
        let d_specific_enthalpy = net_w * dt.get::<second>() / mass_kg;
        self.specific_enthalpy = AvailableEnergy::new::<joule_per_kilogram>(
            self.specific_enthalpy.get::<joule_per_kilogram>() + d_specific_enthalpy,
        );

        self.heat_to_coolant
    }

    /// Lumped pebble (graphite-matrix) temperature.
    ///
    /// This is a **bed average over one node**, not a peak fuel temperature and
    /// not a fuel-kernel temperature: with no radial pebble conduction, the
    /// kernel, the matrix and the ball surface are all this one value.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        temperature_from_specific_enthalpy(self.specific_enthalpy)
    }

    /// Specific enthalpy of the lumped pebble, zero at 298.15 K.
    pub fn specific_enthalpy(&self) -> AvailableEnergy {
        self.specific_enthalpy
    }

    /// Heat rate handed to the helium on the most recent step (negative if the
    /// helium was heating the graphite).
    pub fn heat_to_coolant(&self) -> Power {
        self.heat_to_coolant
    }

    /// Overall pebble-to-helium coefficient used on the most recent step.
    pub fn overall_heat_transfer_coefficient(&self) -> HeatTransfer {
        self.overall_htc
    }

    /// Stored sensible heat in the bed above the 298.15 K reference \[J\],
    /// `m c_p (T - T_ref)`.
    pub fn stored_heat_joules(&self) -> f64 {
        self.specific_enthalpy.get::<joule_per_kilogram>() * graphite_mass().get::<kilogram>()
    }
}

impl Default for PebbleBedCore {
    fn default() -> Self {
        Self::new()
    }
}

/// Overall pebble-to-helium coefficient at helium flow `m_dot`, the nominal
/// value scaled by `(m_dot/m_dot_nominal)^0.6`.
///
/// See [`HTC_FLOW_EXPONENT`] -- the exponent is Wakao's, the coefficient is
/// illustrative, and no Nusselt number is evaluated. The flow is floored at 1%
/// of nominal so a stopped circulator leaves a small residual coefficient
/// rather than zero; with no natural-circulation or radiation path modelled,
/// zero would let the bed heat without limit.
pub fn overall_htc_at_flow(helium_mass_flow: MassRate) -> HeatTransfer {
    let flow_ratio =
        (helium_mass_flow.get::<kilogram_per_second>() / NOMINAL_HELIUM_FLOW_KG_PER_S).max(0.01);
    HeatTransfer::new::<watt_per_square_meter_kelvin>(
        NOMINAL_OVERALL_HTC_W_PER_M2_K * flow_ratio.powf(HTC_FLOW_EXPONENT),
    )
}

/// Graphite specific enthalpy at `temperature`, `c_p (T - 298.15 K)` with the
/// constant [`GRAPHITE_CP_J_PER_KG_K`].
pub fn specific_enthalpy_from_temperature(
    temperature: ThermodynamicTemperature,
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
        assert!((BED_POROSITY - (1.0 - 0.61)).abs() < 1e-12);

        // Sanity on the derived masses and areas the thermal model rests on.
        assert!(graphite_mass().get::<kilogram>() > 5000.0);
        assert!(heat_transfer_area().get::<square_meter>() > 300.0);
    }

    /// Methodology: at a fixed helium temperature and nominal flow, the lumped
    /// bed must settle where the heat handed to the helium equals the fission
    /// power exactly (steady state of `C dT/dt = Q - hA dT`), and the settled
    /// pebble-to-helium temperature difference must equal the analytical
    /// `Q/(h A)`. Run at the published 10 MWth and 4.3 kg/s against a helium
    /// bulk mean of 750 K, 0.05 s steps for 3000 s of simulated time (about 16
    /// bed time constants). Pass criterion: removed heat within 0.1% of
    /// 10 MW, and the temperature difference within 0.5% of `Q/(hA)`.
    ///
    /// Results (2026-08-12): the bed settled at `T_pebble = 954.6746 K`
    /// (681.5 degC) against the 750 K helium, a difference of 204.6746 K. The
    /// analytical `Q/(hA) = 10e6/(160 x 305.363) = 204.6746 K` -- agreement to
    /// better than 1e-8 relative. Removed heat 9.99999998 MW against the
    /// 10 MW fission power, i.e. 2e-9 relative.
    ///
    /// Interpretation: the lumped balance is integrated correctly and conserves
    /// energy at steady state. The *value* 204.68 K is a consequence of the
    /// illustrative overall coefficient, not a prediction -- see the module
    /// docs.
    #[test]
    fn steady_state_removal_equals_fission_power() {
        let mut core = PebbleBedCore::new();
        let power = Power::new::<megawatt>(10.0);
        let helium = ThermodynamicTemperature::new::<kelvin>(750.0);
        let flow = MassRate::new::<kilogram_per_second>(NOMINAL_HELIUM_FLOW_KG_PER_S);

        for _ in 0..60_000 {
            core.step(Time::new::<second>(0.05), power, helium, flow);
        }

        let removed = core.heat_to_coolant().get::<watt>();
        assert!(
            (removed - 1.0e7).abs() / 1.0e7 < 1.0e-3,
            "settled removal {removed} W does not match the 10 MW fission power"
        );

        let measured_dt = core.temperature().get::<kelvin>() - 750.0;
        let expected_dt =
            1.0e7 / (NOMINAL_OVERALL_HTC_W_PER_M2_K * heat_transfer_area().get::<square_meter>());
        assert!(
            (measured_dt - expected_dt).abs() / expected_dt < 5.0e-3,
            "settled pebble-to-helium difference {measured_dt} K departs from Q/(hA) {expected_dt} K"
        );
    }

    /// Methodology: the graphite thermal inertia must show up as the analytical
    /// first-order time constant `tau = C/(hA)`. Starting from thermal
    /// equilibrium with the helium (zero temperature difference), 10 MWth is
    /// applied at nominal flow and the time for the pebble-to-helium difference
    /// to reach 63.2% of its final value is measured against
    /// `tau = m c_p/(h A)`. Pass criterion: within 2% of the analytical value.
    ///
    /// Results (2026-08-12): `C = 5282.78 kg x 1700 J/(kg K) = 8.9807 MJ/K`
    /// and `hA = 160 x 305.363 = 48858.05 W/K`, giving an analytical
    /// `tau = 183.81 s`. The measured 63.2% rise time was **183.75 s** --
    /// 0.03% low, which is within one 0.05 s sampling interval of the
    /// analytical value.
    ///
    /// Interpretation: the graphite inertia is real and is the dominant time
    /// constant of this core -- three minutes, against the helium loop's
    /// seconds. That much is a genuine consequence of the published pebble
    /// count, diameter and graphite density. The exact 184 s figure still
    /// depends on the illustrative `h` and the constant `c_p`.
    #[test]
    fn graphite_inertia_sets_the_bed_time_constant() {
        let helium_k = 750.0;
        let helium = ThermodynamicTemperature::new::<kelvin>(helium_k);
        let flow = MassRate::new::<kilogram_per_second>(NOMINAL_HELIUM_FLOW_KG_PER_S);
        let power = Power::new::<megawatt>(10.0);
        let dt = Time::new::<second>(0.05);

        // Start in equilibrium with the helium so the step response is clean.
        let mut core = PebbleBedCore::new();
        core.specific_enthalpy = specific_enthalpy_from_temperature(helium);

        let final_dt_k =
            1.0e7 / (NOMINAL_OVERALL_HTC_W_PER_M2_K * heat_transfer_area().get::<square_meter>());
        let target = 0.632 * final_dt_k;

        let mut elapsed = 0.0;
        let mut measured_tau = None;
        for _ in 0..200_000 {
            core.step(dt, power, helium, flow);
            elapsed += 0.05;
            if core.temperature().get::<kelvin>() - helium_k >= target {
                measured_tau = Some(elapsed);
                break;
            }
        }

        let measured_tau = measured_tau.expect("the bed must reach 63.2% of its step response");
        let analytical_tau = bed_heat_capacity().get::<joule_per_kelvin>()
            / (NOMINAL_OVERALL_HTC_W_PER_M2_K * heat_transfer_area().get::<square_meter>());
        assert!(
            (measured_tau - analytical_tau).abs() / analytical_tau < 0.02,
            "measured bed time constant {measured_tau} s departs from the analytical {analytical_tau} s"
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
            let round_tripped =
                temperature_from_specific_enthalpy(specific_enthalpy_from_temperature(t))
                    .get::<kelvin>();
            assert!((round_tripped - t_k).abs() < 1e-9);
        }

        let at_nominal = overall_htc_at_flow(MassRate::new::<kilogram_per_second>(
            NOMINAL_HELIUM_FLOW_KG_PER_S,
        ))
        .get::<watt_per_square_meter_kelvin>();
        assert!((at_nominal - NOMINAL_OVERALL_HTC_W_PER_M2_K).abs() < 1e-9);

        let half = overall_htc_at_flow(MassRate::new::<kilogram_per_second>(
            0.5 * NOMINAL_HELIUM_FLOW_KG_PER_S,
        ))
        .get::<watt_per_square_meter_kelvin>();
        let double = overall_htc_at_flow(MassRate::new::<kilogram_per_second>(
            2.0 * NOMINAL_HELIUM_FLOW_KG_PER_S,
        ))
        .get::<watt_per_square_meter_kelvin>();
        assert!(half < at_nominal && at_nominal < double);

        let stopped = overall_htc_at_flow(MassRate::new::<kilogram_per_second>(0.0))
            .get::<watt_per_square_meter_kelvin>();
        assert!(
            stopped > 0.0,
            "a stopped circulator must leave a residual coefficient"
        );
    }
}
