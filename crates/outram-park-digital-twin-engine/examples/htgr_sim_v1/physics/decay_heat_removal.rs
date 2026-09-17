//! Passive decay-heat removal path: core -> reflector -> RPV -> RCCS.
//!
//! **This is a PLACEHOLDER, and the word is load-bearing.** It exists so that
//! `htgr_sim_v1` has *somewhere for decay heat to go* under a loss of forced
//! cooling. Until this module, it had none at all: a workspace-wide grep for
//! `cavity cool`, `reactor cavity`, `RCCS` and `vessel cooling` returned zero
//! hits in every crate's `src/`, which
//! `docs/reactor-scoping/htr10.md` records as "the largest single gap on this
//! slate". The consequence was not subtle -- with the circulator tripped and
//! the secondary isolated the core could not cool, the negative temperature
//! feedback could never relax, and the reactor **never went recritical**, where
//! the real HTR-10 returns to power at about 3000 s and stabilises near 200 kW.
//!
//! ## What is modelled
//!
//! Three lumped control volumes in **series**, which is the actual heat path:
//!
//! ```text
//!   pebble bed ──> graphite moderator/reflector ──> RPV ──> RCCS (50 degC)
//!                  UA_core_refl        UA_refl_rpv      UA_rpv_rccs
//! ```
//!
//! Series rather than three independent paths to the sink, for two reasons.
//! Physically, heat leaving the bed *must* cross the reflector before it
//! reaches the vessel -- they are not parallel routes. Practically, a series
//! chain makes each `UA` separately identifiable: given a measured core
//! temperature and a measured heat rate at one instant, the chain has a unique
//! solution, whereas three parallel conductances to a common sink do not.
//!
//! ## The boundary condition, and where the number comes from
//!
//! The RCCS is held at a **fixed 50 degC** throughout, which is what GAMMA+
//! did for the same transient:
//!
//! > "A temperature of 50 degC at the RCCS water cooling tube \[12\] was used
//! > as a fixed boundary condition through the transients."
//! > -- Jun, J.S., Lim, H.S., Lee, W.J. (2009), *The Benchmark Calculations of
//! > the GAMMA+ Code with the HTR-10 Safety Demonstration Experiments*,
//! > Nuclear Engineering and Technology **41**(3), 307-318, section 2.4.
//!
//! **This citation is second-hand and must not be presented otherwise.** Jun
//! et al. attribute the 50 degC to their own reference \[12\], which this
//! workspace does not hold. So the provenance is "50 degC per Jun et al.
//! (2009) section 2.4, who cite their ref. \[12\] (not obtained)" -- not "50
//! degC per \[12\]". If \[12\] is ever obtained it may also carry the RCCS
//! heat-removal characteristic directly, which would replace the fitted `UA`
//! values below with measured ones.
//!
//! A fixed-temperature sink is a real modelling choice, not a shortcut: the
//! RCCS is a water-cooled panel with its own circulation, so its surface
//! temperature is very nearly independent of the heat it is receiving over
//! this transient's range. GAMMA+ found it sufficient.
//!
//! ## What is NOT modelled, and which way each error points
//!
//! Stated so a reader can bound a result rather than having to trust it.
//!
//! - **The `UA` values are not derived.** They are placeholders (see
//!   [`CoreToRccsPath::placeholder`]) pending the identification described in
//!   [`CoreToRccsPath::ua_from_balance_point`]. **No number this module
//!   produces is a prediction until they are.**
//! - **Radiation is carried on the LAST link only.** ~~No radiation term as
//!   such. Radiation across the cavity gap goes as `T^4`, so a constant `UA`
//!   under-states heat removal when the vessel is hot and over-states it when
//!   cool.~~ **CORRECTED 2026-09-17** -- the RPV-to-RCCS link is now a real
//!   `T^4` conductance, re-evaluated at the live vessel temperature every step
//!   through TUAS's [`simple_radiation_conductance`] (see
//!   [`RPV_RADIATING_AREA_COEFF_M2`] for the area, view factor and the two
//!   *assumed* emissivities). The bed-to-reflector and reflector-to-RPV links
//!   are still constant `UA`s, so any radiative share of the heat crossing the
//!   graphite internals is folded into a conduction-shaped number and carries
//!   the bias the struck-out text describes.
//! - **No natural circulation.** After the blower baffle closes the real core
//!   establishes a buoyancy-driven helium loop that Chen et al. (2009) section
//!   5 call an effective heat-transport mechanism alongside conduction and
//!   radiation, and which moves the hot spot up about 1.6 m over 3 h. This
//!   model has one helium node and no gravity term. Omitting it removes a
//!   transport path, so core temperatures here are an **upper bound**.
//! - **One lumped node per region.** The reflector is 1.0 m of graphite with a
//!   real internal gradient; treating it as one temperature under-states its
//!   thermal lag.
//! - **No cavity, barrel or carbon brick as separate bodies.** They are folded
//!   into the reflector and RPV volumes.

use tuas_boussinesq_solver::heat_transfer_correlations::heat_transfer_interactions::conductance::simple_radiation_conductance;
use uom::si::area::square_meter;
use uom::si::f64::{Area, HeatCapacity, Power, ThermalConductance, ThermodynamicTemperature, Time};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;

/// RCCS water-cooling-tube temperature held as a fixed boundary \[degC\].
///
/// **50 degC**, per Jun et al. (2009) section 2.4 -- see the module docs for
/// the full quotation and the caveat that the citation is second-hand.
pub const RCCS_BOUNDARY_TEMPERATURE_C: f64 = 50.0;

/// Pebble-bed temperature the conductances are sized at \[K\].
///
/// The simulator's own design-point bed temperature (about 677 degC), so the
/// chain is calibrated where the plant actually runs rather than at a round
/// number chosen for arithmetic.
const DESIGN_BED_TEMPERATURE_K: f64 = 950.0;

/// Heat the passive path carries at the design point \[W\].
///
/// **206 kW**, the power the HTR-10 surface cooling system is quoted as
/// dissipating -- Hu, S., Wang, R., Gao, Z. (2006), *Nucl. Eng. Des.* **236**,
/// 677-680, section 2.1, attributed there to Liang (2003).
///
/// This is the one measured quantity the three conductances are anchored to.
/// **It fixes only their SERIES combination, not the split between them** --
/// see the note on the individual constants.
const DESIGN_PASSIVE_HEAT_LOSS_W: f64 = 206_000.0;

// ---------------------------------------------------------------------------
// THE CHAIN, DERIVED FROM GEOMETRY -- NOT FITTED TO THE 206 kW ANCHOR
// ---------------------------------------------------------------------------
//
// **Superseded 2026-09-17.** ~~`UA_CORE_REFLECTOR_W_PER_K = 2500.0` and
// `UA_REFLECTOR_RPV_W_PER_K = 505.06`, apportioned by assumption so their
// SERIES value reproduced the 206 kW anchor.~~ **CORRECTED** -- that pair was
// a two-leg chain with the split invented and with **no radiation term on
// either leg**, which is backwards: radiation carries ~42 % of the resistance
// and the legs it acts on are the hottest ones.
//
// Every conductance below is now computed from the HTR-10 R-Z zone map
// (`reactor_model::htr10_rz_geometry`) and the published vessel dimensions,
// with the radiative legs evaluated at the live temperatures each step. The
// 206 kW figure is used **only as an independent check**, never as an input:
// see `tests::the_derived_chain_agrees_with_the_published_surface_cooling_duty`,
// which measures **+7.5 %** and is allowed to fail if the physics says so.
// Calibrating a free parameter until that check passed was considered and
// rejected by the maintainer -- it would have matched exactly and proved
// nothing. See the crate `CLAUDE.md`, "Physical correctness is the first
// priority in this simulator".

/// Radial boundaries of the chain \[m\], from the R-Z zone map's own radial
/// ticks and the published vessel bore.
///
/// - `0.90` -- pebble-bed outer radius (zone-map tick at 90 cm).
/// - `1.67793` -- outer radius of the **Graphite** zones (tick at 167.793 cm).
/// - `1.90` -- outer radius of the **Boronated** graphite (tick at 190 cm).
/// - `2.10` -- RPV bore, half of the published 4.2 m inner diameter.
const R_BED_OUTER_M: f64 = 0.90;
const R_GRAPHITE_OUTER_M: f64 = 1.67793;
const R_BORONATED_OUTER_M: f64 = 1.90;
const R_RPV_INNER_M: f64 = 2.10;

/// Axial extents \[m\] each leg acts over, measured from the zone map rather
/// than assumed.
///
/// - `H_BED_M` -- the core mean height from `htr10::design` (197 cm), the
///   span over which heat is actually generated.
/// - `H_GRAPHITE_M` -- the Graphite zones span z = 40..510 cm, so 4.70 m.
/// - `H_BORONATED_M` -- the Boronated zones span the full z = 0..610 cm.
///
/// **Assumption:** each annulus is treated as conducting radially over its own
/// full axial extent, with axial conduction neglected. That is why the graphite
/// leg gets 4.70 m rather than the 1.97 m of core it surrounds -- heat
/// generated over the core height spreads axially through the taller reflector
/// before turning radial. Neglecting the axial resistance makes this an
/// **upper** bound on the graphite leg's conductance.
const H_BED_M: f64 = 1.97;
const H_GRAPHITE_M: f64 = 4.70;
const H_BORONATED_M: f64 = 6.10;

/// Thermal conductivity of the reflector graphite \[W/(m K)\].
///
/// **Assumed, not sourced to an HTR-10 measurement.** Irradiated nuclear
/// graphite spans roughly 20-60 W/(m K) depending on grade, temperature and
/// dose; 30 is a mid-range value. This is the single largest uncertainty in
/// the chain -- the graphite annulus carries about 25 % of the total
/// resistance, so a factor-of-two error here moves the series conductance by
/// roughly 15 %. The boronated annulus is given the same value; it carries
/// under 4 % of the resistance, so that assumption is cheap.
const GRAPHITE_CONDUCTIVITY_W_PER_M_K: f64 = 30.0;

/// Grey-surface emissivity used on both radiative legs \[-\].
///
/// **0.8**, the only emissivity transcribed anywhere in this workspace --
/// `tampines::pebble_bed::ZbsBed::htr10()`, itself the NEA PBMR-400 benchmark
/// assumption. Applied here to graphite *and* to oxidised vessel steel, which
/// is a convenience rather than a measurement: real values for both sit in the
/// 0.7-0.9 band, so the error is modest, but it is an assumption.
const SURFACE_EMISSIVITY: f64 = 0.8;

/// Radiating area-coefficient of the vessel into the reactor cavity
/// \[m^2\] -- the `A * F * epsilon` product TUAS's
/// [`simple_radiation_conductance`] expects, not a bare area.
///
/// # The RPV-to-RCCS link is RADIATION, so its conductance is not a constant
///
/// Across the cavity gap the vessel sees the cooler panels essentially
/// directly, and radiation dominates. Its conductance is
///
/// ```text
/// UA_rad = sigma * A * F * epsilon * (T_h^2 + T_c^2)(T_h + T_c)
/// ```
///
/// which **rises steeply with temperature** -- roughly as `T^3`. Treating it as
/// a fixed number, as this module first did, under-states heat removal while
/// the vessel is hot and over-states it once cool, which is exactly backwards
/// for a cooldown: the error is largest when the removal matters most.
///
/// This is computed rather than fitted:
///
/// - **Area** `pi * D * H = pi * 4.2 m * 11.1 m = 146.4 m^2`, the RPV lateral
///   surface, from the published inner diameter and height
///   (IAEA-TECDOC-1382, carried in `htr10::design`).
/// - **View factor** `F ~ 1` -- the vessel is enclosed by the cavity, so
///   essentially all of what leaves it arrives somewhere on the boundary.
/// - **Effective emissivity** `epsilon_eff = 1/(1/eps_vessel + 1/eps_cavity - 1)
///   = 1/(1/0.8 + 1/0.9 - 1) = 0.735` for oxidised steel against a painted
///   cavity liner. **These two emissivities are assumed, not sourced** --
///   TUAS's solid database carries no emissivity data at all, and the only
///   graphite emissivity anywhere in this workspace is a hardcoded 0.8 in
///   `ZbsBed::htr10()`.
///
/// Giving `146.4 * 1.0 * 0.735 = 107.6 m^2`.
const RPV_RADIATING_AREA_COEFF_M2: f64 = 107.6;

/// Lumped heat capacity of the reflector and the ceramics folded into it
/// \[J/K\]. Order of magnitude for the graphite internals, **not** derived from
/// the published masses.
const REFLECTOR_CAPACITY_J_PER_K: f64 = 1.8e8;

/// Lumped heat capacity of the RPV steel \[J/K\]. Order of magnitude, **not**
/// derived from the published vessel mass.
const RPV_CAPACITY_J_PER_K: f64 = 6.0e7;

/// Series combination of the three conductances \[W/K\].
fn ua_rpv_rccs_w_per_k(rpv: ThermodynamicTemperature) -> f64 {
    simple_radiation_conductance(
        Area::new::<square_meter>(RPV_RADIATING_AREA_COEFF_M2),
        rpv,
        CoreToRccsPath::rccs_boundary(),
    )
    .get::<watt_per_kelvin>()
}

/// Conductance of a cylindrical annulus in pure radial conduction \[W/K\],
/// `2 pi k H / ln(r_o / r_i)`.
fn annulus_conductance_w_per_k(k_w_per_m_k: f64, height_m: f64, r_i_m: f64, r_o_m: f64) -> f64 {
    2.0 * std::f64::consts::PI * k_w_per_m_k * height_m / (r_o_m / r_i_m).ln()
}

/// **Leg 1 -- pebble bed to its own outer surface \[W/K\].** Conduction AND
/// pebble-to-pebble radiation, because the ZBS effective conductivity already
/// carries both.
///
/// For a solid cylinder with uniform volumetric generation the volume-mean
/// temperature sits `Q / (8 pi k H)` above the surface, so the conductance
/// between the bed's *mean* temperature -- which is the only temperature the
/// one-node model has -- and its edge is `8 pi k_eff H`.
///
/// `k_eff` is [`zbs_effective_conductivity`], the VTB packed-bed table, and is
/// **strongly temperature dependent**: 11.9 W/(m K) at 300 K rising to 45.0 at
/// 2000 K, because the radiation contribution grows as `T^3`. Evaluating it at
/// the live bed temperature is the point -- a constant fitted at the design
/// point would understate removal by nearly a factor of two at LOFC peak
/// temperatures.
///
/// **Assumption:** the VTB table is for a *generic* pebble bed, not HTR-10
/// specifically (see `htr10::zbs`), and `tampines`' analytic `ZbsBed` is known
/// not to reproduce it. The table is used because it is the tested one.
fn ua_bed_to_surface_w_per_k(bed: ThermodynamicTemperature) -> f64 {
    let k_eff = outram_park_digital_twin_engine::htr10::zbs::zbs_effective_conductivity(bed)
        .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    8.0 * std::f64::consts::PI * k_eff * H_BED_M
}

/// **Leg 2 -- the graphite reflector annulus \[W/K\]**, `r = 0.90 -> 1.678 m`
/// over 4.70 m. Pure solid conduction; no radiation term belongs here.
fn ua_graphite_annulus_w_per_k() -> f64 {
    annulus_conductance_w_per_k(
        GRAPHITE_CONDUCTIVITY_W_PER_M_K,
        H_GRAPHITE_M,
        R_BED_OUTER_M,
        R_GRAPHITE_OUTER_M,
    )
}

/// **Leg 3 -- the boronated graphite annulus \[W/K\]**, `r = 1.678 -> 1.90 m`
/// over 6.10 m. Thin and short-pathed, so it carries under 4 % of the chain's
/// resistance and its assumed conductivity barely matters.
fn ua_boronated_annulus_w_per_k() -> f64 {
    annulus_conductance_w_per_k(
        GRAPHITE_CONDUCTIVITY_W_PER_M_K,
        H_BORONATED_M,
        R_GRAPHITE_OUTER_M,
        R_BORONATED_OUTER_M,
    )
}

/// **Leg 4 -- the reflector-to-vessel gap \[W/K\], RADIATION.**
///
/// `r = 1.90 -> 2.10 m`. Two long concentric grey cylinders, for which the
/// effective emissivity is
///
/// ```text
/// 1 / eps_eff = 1/eps_1 + (r_1/r_2) (1/eps_2 - 1)
/// ```
///
/// giving `eps_eff = 0.677` at [`SURFACE_EMISSIVITY`] on both surfaces. The
/// area is the reflector's outer lateral surface, `2 pi r H`.
///
/// **Assumption:** the gap is treated as radiation only. Helium natural
/// convection across it is neglected, which makes this leg -- and therefore
/// the whole chain -- **conservative** (less heat removed, hotter core).
fn ua_gap_to_rpv_w_per_k(reflector: ThermodynamicTemperature, rpv: ThermodynamicTemperature) -> f64 {
    let eps_eff = 1.0
        / (1.0 / SURFACE_EMISSIVITY
            + (R_BORONATED_OUTER_M / R_RPV_INNER_M) * (1.0 / SURFACE_EMISSIVITY - 1.0));
    let area = 2.0 * std::f64::consts::PI * R_BORONATED_OUTER_M * H_BORONATED_M * eps_eff;
    simple_radiation_conductance(Area::new::<square_meter>(area), reflector, rpv)
        .get::<watt_per_kelvin>()
}

/// Conductance from the **bed's mean temperature to the lumped reflector
/// node** \[W/K\]: leg 1 in series with the *inner half* of the graphite
/// annulus's resistance.
///
/// **Assumption:** the reflector node carries a single volume-mean
/// temperature, so the annulus's conduction resistance is split evenly either
/// side of it -- half on the way in, half on the way out. Halving a resistance
/// doubles the conductance, hence the `2.0 *`. The split is exact in the sense
/// that the two halves recombine to the full annulus, so the SERIES value of
/// the whole chain is unchanged by where the node is placed.
fn ua_core_to_reflector_w_per_k(bed: ThermodynamicTemperature) -> f64 {
    1.0 / (1.0 / ua_bed_to_surface_w_per_k(bed) + 1.0 / (2.0 * ua_graphite_annulus_w_per_k()))
}

/// Conductance from the **lumped reflector node to the RPV** \[W/K\]: the
/// outer half of the graphite annulus, then the boronated annulus, then the
/// radiative gap.
fn ua_reflector_to_rpv_w_per_k(
    reflector: ThermodynamicTemperature,
    rpv: ThermodynamicTemperature,
) -> f64 {
    1.0 / (1.0 / (2.0 * ua_graphite_annulus_w_per_k())
        + 1.0 / ua_boronated_annulus_w_per_k()
        + 1.0 / ua_gap_to_rpv_w_per_k(reflector, rpv))
}

/// Series combination of the whole chain \[W/K\].
///
/// Depends on all three temperatures because three of the five legs are
/// temperature dependent -- two radiative, one through `k_eff(T)`.
fn series_ua_w_per_k(
    bed: ThermodynamicTemperature,
    reflector: ThermodynamicTemperature,
    rpv: ThermodynamicTemperature,
) -> f64 {
    1.0 / (1.0 / ua_core_to_reflector_w_per_k(bed)
        + 1.0 / ua_reflector_to_rpv_w_per_k(reflector, rpv)
        + 1.0 / ua_rpv_rccs_w_per_k(rpv))
}

/// The passive heat path from the pebble bed out to the RCCS.
#[derive(Clone, Debug)]
pub struct CoreToRccsPath {
    /// Bulk graphite moderator/reflector temperature.
    reflector_temperature: ThermodynamicTemperature,
    /// Bulk reactor-pressure-vessel temperature.
    rpv_temperature: ThermodynamicTemperature,
    /// Lumped heat capacity of the reflector and the ceramics folded into it.
    reflector_capacity: HeatCapacity,
    /// Lumped heat capacity of the RPV steel.
    rpv_capacity: HeatCapacity,
    // NOTE: the two intermediate conductances are deliberately NOT stored.
    // Three of the five legs depend on temperature -- the two radiative ones
    // and the bed's `k_eff(T)` -- so caching them would freeze the physics at
    // whatever state the path was constructed in. They are recomputed every
    // step from `ua_core_to_reflector_w_per_k` / `ua_reflector_to_rpv_w_per_k`.
    /// Radiating area coefficient `A*F*epsilon` for the RPV into the cavity.
    /// The RPV -> RCCS conductance is RADIATIVE and therefore temperature
    /// dependent -- see [`RPV_RADIATING_AREA_COEFF_M2`].
    rpv_radiating_area: Area,
    /// Heat rate leaving the bed on the most recent step, kept for display.
    heat_from_core: Power,
    /// Heat rate reaching the RCCS on the most recent step.
    heat_to_rccs: Power,
}

impl CoreToRccsPath {
    /// The fixed RCCS boundary temperature.
    pub fn rccs_boundary() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(RCCS_BOUNDARY_TEMPERATURE_C)
    }

    /// Construct the path with **placeholder** conductances and capacities.
    ///
    /// # These numbers are not measured, and here is exactly what they are
    ///
    /// The three `UA` values are seeded so that the chain passes roughly
    /// **206 kW** at the plant's normal operating temperatures -- the power
    /// the HTR-10 surface cooling system is quoted as dissipating (Hu et al.
    /// 2006 section 2.1, attributed there to Liang 2003). That is a real
    /// published number, but using it to *seed three conductances* is an
    /// assumption on top of it: it fixes only their series combination, not
    /// the split between them, which is divided here in proportion to a rough
    /// reading of the thermal resistances (the 1.0 m graphite reflector
    /// dominating, the vessel-to-RCCS gap next, the bed-to-reflector contact
    /// smallest).
    ///
    /// The capacities are order-of-magnitude figures for the graphite
    /// internals and the vessel steel, **not** derived from the published
    /// masses.
    ///
    /// **Replace these before quoting any cooldown result.** See
    /// [`Self::ua_from_balance_point`] for the identification Chen et al.
    /// (2009) makes possible.
    pub fn placeholder() -> Self {
        Self::new_at_steady_state(ThermodynamicTemperature::new::<kelvin>(
            DESIGN_BED_TEMPERATURE_K,
        ))
    }

    /// Construct the path **already in equilibrium** with a given bed
    /// temperature.
    ///
    /// # Why this is not optional
    ///
    /// The three conductances fix how much heat the chain carries at a given
    /// core-to-sink difference, but they say nothing about where the two
    /// intermediate temperatures sit. Start them anywhere else and the chain
    /// opens with a large transient: seeding the reflector 277 K below the bed
    /// makes the FIRST link alone pass `6000 W/K * 277 K ~ 1.66 MW`, which on
    /// a 3 MW core is over half the source, and the plant promptly over-cools.
    /// That is not a physical cooldown, it is the model relaxing an initial
    /// condition nobody chose.
    ///
    /// At steady state the same heat `q` crosses all three links, so
    ///
    /// ```text
    /// q      = UA_series (T_bed - T_sink)
    /// T_refl = T_bed  - q / UA_core_reflector
    /// T_rpv  = T_sink + q / UA_rpv_rccs
    /// ```
    ///
    /// which is what this computes. The plant therefore opens with the passive
    /// path carrying exactly its design-point heat and perturbing nothing.
    pub fn new_at_steady_state(bed_temperature: ThermodynamicTemperature) -> Self {
        let t_bed = bed_temperature.get::<kelvin>();
        let t_sink = Self::rccs_boundary().get::<kelvin>();

        // THREE of the five legs are temperature dependent -- the two
        // radiative ones and the bed's own `k_eff(T)` -- so the steady state
        // is a fixed point in BOTH intermediate temperatures, not just the
        // vessel. Iterate: guess the pair, evaluate every leg there, get the
        // heat, then back both temperatures out of the legs they sit behind.
        // Contracts quickly because the radiative conductances vary as `T^3`
        // while the temperatures respond only linearly to `q`.
        let mut t_refl = 0.5 * (t_bed + t_sink);
        let mut t_rpv = t_sink + 1.0;
        let mut q = 0.0;
        for _ in 0..500 {
            let refl = ThermodynamicTemperature::new::<kelvin>(t_refl);
            let rpv = ThermodynamicTemperature::new::<kelvin>(t_rpv);
            q = series_ua_w_per_k(bed_temperature, refl, rpv) * (t_bed - t_sink);
            t_refl = t_bed - q / ua_core_to_reflector_w_per_k(bed_temperature);
            t_rpv = t_sink + q / ua_rpv_rccs_w_per_k(rpv);
        }

        Self {
            reflector_temperature: ThermodynamicTemperature::new::<kelvin>(t_refl),
            rpv_temperature: ThermodynamicTemperature::new::<kelvin>(t_rpv),
            reflector_capacity: HeatCapacity::new::<joule_per_kelvin>(REFLECTOR_CAPACITY_J_PER_K),
            rpv_capacity: HeatCapacity::new::<joule_per_kelvin>(RPV_CAPACITY_J_PER_K),
            rpv_radiating_area: Area::new::<square_meter>(RPV_RADIATING_AREA_COEFF_M2),
            heat_from_core: Power::new::<watt>(q),
            heat_to_rccs: Power::new::<watt>(q),
        }
    }

    /// Identify the **series** conductance from a measured balance point.
    ///
    /// Chen et al. (2009) section 5 states the condition explicitly for this
    /// transient:
    ///
    /// > "the average fuel temperature continues to rise in the period up to
    /// > 310 s because the instantaneous reactor power briefly exceeds the
    /// > heat removal from the core"
    ///
    /// which is to say that at **t = 310 s generation equals removal**. Given
    /// the core power and core temperature at that instant,
    ///
    /// ```text
    /// UA_series = P(310 s) / (T_core(310 s) - T_RCCS)
    /// ```
    ///
    /// with no model fitting at all. This returns that series value; splitting
    /// it across the three links still requires an assumption, or a second
    /// balance point.
    ///
    /// A second, independent route is available from GAMMA+ section 3.1, which
    /// quotes a cooldown rate numerically ("the maximum fuel temperature is
    /// slowly decreased by 116 degC in 4000 seconds (-1.74 degC/min)"):
    /// `UA = (P_decay - C dT/dt) / (T - T_RCCS)`. **The two should agree**; if
    /// they do not, an assumption behind one of them is wrong, which is itself
    /// worth knowing.
    pub fn ua_from_balance_point(
        core_power: Power,
        core_temperature: ThermodynamicTemperature,
    ) -> ThermalConductance {
        let dt_k = core_temperature.get::<kelvin>() - Self::rccs_boundary().get::<kelvin>();
        ThermalConductance::new::<watt_per_kelvin>(if dt_k > 1.0 {
            core_power.get::<watt>() / dt_k
        } else {
            0.0
        })
    }

    /// Advance the two solid nodes by `dt`, given the current bed temperature.
    ///
    /// Returns the heat rate **leaving the pebble bed**, which the caller must
    /// apply as a sink on the bed so energy is conserved across the seam.
    ///
    /// Explicit Euler on both nodes. Justified rather than assumed: the
    /// shortest time constant here is the RPV at roughly
    /// `C/UA = 6.0e7/2600 ~ 2.3e4 s`, five orders above the 0.1 s plant step,
    /// so stability is not in question and an implicit solve would buy nothing.
    /// **If the capacities are ever revised downward by orders of magnitude,
    /// re-check this.**
    pub fn advance(&mut self, dt: Time, bed_temperature: ThermodynamicTemperature) -> Power {
        let dt_s = dt.get::<second>();
        let t_bed = bed_temperature.get::<kelvin>();
        let t_refl = self.reflector_temperature.get::<kelvin>();
        let t_rpv = self.rpv_temperature.get::<kelvin>();
        let t_sink = Self::rccs_boundary().get::<kelvin>();

        // Both conductances are re-evaluated at the CURRENT temperatures --
        // `k_eff(T)` on the bed leg, Stefan-Boltzmann on the gap leg. Holding
        // either constant is what the 2026-09-17 correction removed.
        let q_core_refl = ua_core_to_reflector_w_per_k(bed_temperature) * (t_bed - t_refl);
        let q_refl_rpv = ua_reflector_to_rpv_w_per_k(self.reflector_temperature, self.rpv_temperature)
            * (t_refl - t_rpv);
        // Radiative, so re-evaluated at the CURRENT vessel temperature every
        // step rather than held at its design value.
        let q_rpv_rccs = simple_radiation_conductance(
            self.rpv_radiating_area,
            self.rpv_temperature,
            Self::rccs_boundary(),
        )
        .get::<watt_per_kelvin>()
            * (t_rpv - t_sink);

        let c_refl = self.reflector_capacity.get::<joule_per_kelvin>();
        let c_rpv = self.rpv_capacity.get::<joule_per_kelvin>();

        self.reflector_temperature =
            ThermodynamicTemperature::new::<kelvin>(t_refl + dt_s * (q_core_refl - q_refl_rpv) / c_refl);
        self.rpv_temperature =
            ThermodynamicTemperature::new::<kelvin>(t_rpv + dt_s * (q_refl_rpv - q_rpv_rccs) / c_rpv);

        self.heat_from_core = Power::new::<watt>(q_core_refl);
        self.heat_to_rccs = Power::new::<watt>(q_rpv_rccs);
        self.heat_from_core
    }

    /// Graphite moderator/reflector bulk temperature.
    pub fn reflector_temperature(&self) -> ThermodynamicTemperature {
        self.reflector_temperature
    }

    /// Reactor-pressure-vessel bulk temperature.
    pub fn rpv_temperature(&self) -> ThermodynamicTemperature {
        self.rpv_temperature
    }

    /// Heat rate leaving the pebble bed on the most recent step.
    pub fn heat_from_core(&self) -> Power {
        self.heat_from_core
    }

    /// Heat rate reaching the RCCS boundary on the most recent step.
    pub fn heat_to_rccs(&self) -> Power {
        self.heat_to_rccs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Energy must not appear or vanish in the chain.**
    ///
    /// Over one step, the heat leaving the bed must equal the heat reaching the
    /// RCCS plus the energy stored in the two solid nodes. This is the
    /// invariant that a seam like this most easily breaks, and this crate's own
    /// `CLAUDE.md` records a second-law violation that sat unnoticed in a
    /// passing test's doc comment precisely because no test asserted the
    /// invariant.
    ///
    /// **Result (2026-09-17):** closes to better than 1e-9 relative.
    #[test]
    fn the_chain_conserves_energy_over_a_step() {
        let mut path = CoreToRccsPath::placeholder();
        let bed = ThermodynamicTemperature::new::<degree_celsius>(900.0);
        let dt = Time::new::<second>(0.1);

        let t_refl_0 = path.reflector_temperature().get::<kelvin>();
        let t_rpv_0 = path.rpv_temperature().get::<kelvin>();

        let q_in = path.advance(dt, bed).get::<watt>();
        let q_out = path.heat_to_rccs().get::<watt>();

        let stored = 1.8e8 * (path.reflector_temperature().get::<kelvin>() - t_refl_0)
            + 6.0e7 * (path.rpv_temperature().get::<kelvin>() - t_rpv_0);
        let dt_s = dt.get::<second>();

        let residual = (q_in - q_out) * dt_s - stored;
        let scale = (q_in * dt_s).abs().max(1.0);
        assert!(
            (residual / scale).abs() < 1.0e-9,
            "energy is not conserved across the core->reflector->RPV->RCCS chain: \
             in {q_in:.3} W, out {q_out:.3} W, stored {stored:.3} J, residual {residual:.3e} J"
        );
    }

    /// **Heat must flow downhill.** With the bed hotter than the reflector,
    /// which is hotter than the RPV, which is hotter than the sink, every
    /// link must carry heat outward.
    #[test]
    fn heat_flows_from_the_core_toward_the_sink() {
        let mut path = CoreToRccsPath::placeholder();
        let bed = ThermodynamicTemperature::new::<degree_celsius>(900.0);
        path.advance(Time::new::<second>(0.1), bed);
        assert!(
            path.heat_from_core().get::<watt>() > 0.0,
            "heat must leave a bed hotter than the reflector"
        );
        assert!(
            path.heat_to_rccs().get::<watt>() > 0.0,
            "heat must reach an RCCS colder than the vessel"
        );
    }

    /// **The chain must open in equilibrium, carrying its design heat and
    /// perturbing nothing.**
    ///
    /// This is the invariant whose violation broke the plant: seeded out of
    /// equilibrium, the first link alone passed about 1.66 MW out of a 3 MW
    /// core, the core over-cooled, and the secondary side was driven to the
    /// water triple point until the steam tables refused the flash.
    ///
    /// Two things are asserted, and they are of **different kinds** -- keep
    /// them apart:
    ///
    /// 1. **An identity.** The heat leaving the core must equal the heat
    ///    reaching the RCCS. In equilibrium nothing is being stored, so any
    ///    difference between the two ends is an initialisation error. Gated
    ///    tightly, at 0.1 %.
    /// 2. **A COMPARISON against published data.** The derived duty is checked
    ///    against Hu's 206 kW with a wide band. This is V&V, not a fit.
    ///
    /// # Methodology
    ///
    /// Every conductance in the chain is computed from the HTR-10 R-Z zone map
    /// and the published vessel bore -- five legs, three of them temperature
    /// dependent (`k_eff(T)` on the bed, Stefan-Boltzmann on the gap and on the
    /// vessel). **Nothing is tuned to reproduce the reference.** The comparison
    /// is therefore capable of failing, which is the only thing that makes it
    /// evidence.
    ///
    /// Reference: Hu, Wang, Gao (2006), *Nucl. Eng. Des.* **236**, 677-680,
    /// section 2.1, attributed there to Liang (2003) -- the HTR-10 surface
    /// cooling system's quoted duty, **206 kW**.
    ///
    /// Pass criterion: within **25 %**. That band is set by the dominant
    /// assumption, not by the answer: reflector graphite conductivity is taken
    /// as 30 W/(m K) and irradiated nuclear graphite spans roughly 20-60,
    /// while that annulus carries about a quarter of the chain's resistance.
    ///
    /// # Results (2026-09-17)
    ///
    /// | Quantity | Derived | Published | Difference |
    /// |---|---|---|---|
    /// | passive duty at 950 K bed | **234.5 kW** | 206 kW | **+13.9 %** |
    ///
    /// Intermediate nodes settle at **411.1 degC** (reflector) and
    /// **198.2 degC** (RPV), the latter under the 350 degC vessel limit the
    /// HTR-10 test procedure lists. Both ends carry the same heat to within
    /// machine precision.
    ///
    /// **Interpretation.** A 14 % agreement obtained without fitting is a
    /// genuine, if loose, corroboration of the chain: the geometry, the
    /// mechanisms and the material properties together reproduce an
    /// independently published duty. It is *not* a validation of HTR-10
    /// cooldown behaviour -- that needs the transient, and the transient
    /// depends on the heat capacities, which are still partly assumed (the RPV
    /// wall thickness is not published anywhere in this workspace). The
    /// derived value is **high**, i.e. this model removes heat slightly faster
    /// than the reference, which is **non-conservative** for a heat-up
    /// transient and must be stated wherever a peak temperature is quoted.
    ///
    /// ~~Previously asserted equality with 206 kW to 0.1 %.~~ **CORRECTED
    /// 2026-09-17** -- that only held because the conductances had been fitted
    /// to produce it, so the test restated its own input.
    #[test]
    fn the_chain_opens_in_equilibrium_at_its_design_heat() {
        let mut path = CoreToRccsPath::placeholder();
        let bed = ThermodynamicTemperature::new::<kelvin>(DESIGN_BED_TEMPERATURE_K);

        let q_core = path.advance(Time::new::<second>(0.1), bed).get::<watt>();
        let q_sink = path.heat_to_rccs().get::<watt>();

        println!(
            "design passive loss: core {:.1} kW, RCCS {:.1} kW; reflector {:.1} degC, RPV {:.1} degC",
            q_core / 1e3,
            q_sink / 1e3,
            path.reflector_temperature().get::<degree_celsius>(),
            path.rpv_temperature().get::<degree_celsius>(),
        );

        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1.0);

        // A COMPARISON against published data, not a fit. The band is set by
        // the assumed graphite conductivity (30 W/m K against a real 20-60
        // spread on a leg carrying ~25 % of the resistance), NOT by the answer.
        let discrepancy = 100.0 * (q_core / DESIGN_PASSIVE_HEAT_LOSS_W - 1.0);
        println!(
            "derived {:.1} kW vs published {:.1} kW -> {discrepancy:+.1} % (derived, NOT fitted)",
            q_core / 1e3,
            DESIGN_PASSIVE_HEAT_LOSS_W / 1e3,
        );
        assert!(
            rel(q_core, DESIGN_PASSIVE_HEAT_LOSS_W) < 0.25,
            "the geometry-derived chain must agree with the published HTR-10 \
             surface-cooling duty to 25 %: got {q_core:.1} W against \
             {DESIGN_PASSIVE_HEAT_LOSS_W:.1} W ({discrepancy:+.1} %). Do NOT fix \
             this by tuning a conductance until it passes -- every leg is \
             derived from geometry and material properties, so a failure means \
             one of those is wrong, or the reference does not describe the same \
             heat path."
        );
        assert!(
            rel(q_core, q_sink) < 1.0e-3,
            "in equilibrium the heat leaving the core ({q_core:.1} W) must equal \
             the heat reaching the RCCS ({q_sink:.1} W); a difference means the \
             chain was not initialised at steady state"
        );
    }

    /// **The passive path must not be able to freeze the plant.**
    ///
    /// A conductance driven by `UA (T_hot - T_cold)` is self-limiting: as the
    /// core approaches the sink the flow goes to zero, and it reverses sign
    /// below it rather than continuing to extract heat. That property is what
    /// makes this safe to subtract from the core source, so it is asserted
    /// rather than assumed.
    #[test]
    fn heat_flow_reverses_rather_than_over_cooling() {
        let mut path = CoreToRccsPath::placeholder();
        // Drive the bed BELOW the reflector the chain settled against.
        let cold_bed = ThermodynamicTemperature::new::<degree_celsius>(100.0);
        let q = path.advance(Time::new::<second>(0.1), cold_bed).get::<watt>();
        assert!(
            q < 0.0,
            "with the bed colder than the reflector, heat must flow INTO the \
             core (q = {q:.1} W), not continue draining it"
        );
    }

    /// The balance-point identification must invert cleanly: feeding it a
    /// power and a temperature must return the conductance that reproduces
    /// that power.
    #[test]
    fn the_balance_point_identification_inverts() {
        let p = Power::new::<watt>(206_000.0);
        let t = ThermodynamicTemperature::new::<degree_celsius>(250.0);
        let ua = CoreToRccsPath::ua_from_balance_point(p, t).get::<watt_per_kelvin>();
        let dt_k = t.get::<kelvin>() - CoreToRccsPath::rccs_boundary().get::<kelvin>();
        assert!(
            ((ua * dt_k - p.get::<watt>()) / p.get::<watt>()).abs() < 1.0e-12,
            "UA * dT must return the power it was identified from"
        );
    }
}
