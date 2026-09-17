//! Mk1 PB-FHR case data for the Xin Wang (2018) Figure-4.29 reproduction.
//!
//! This module holds the *typed, machine-readable* form of the case data
//! extracted into `docs/xin-wang-thesis/04-transients-fig4-29.md`: the 8-group
//! energy structure (Table 3.4), the control-rod-removal transient definition
//! (§4.5.2), and the digitised Figure-4.29 reference curve (max fuel temperature
//! vs time). These are the inputs the workflow stages target and the reference
//! the validation stage compares against.
//!
//! Source: Xin Wang, PhD dissertation, UC Berkeley, 2018,
//! <https://escholarship.org/uc/item/40q3985m> (open literature). All values are
//! AI-assisted extractions requiring human verification against the source PDF;
//! the Figure-4.29 curve in particular is **digitised by eye** and approximate.
//!
//! All public quantities are [`uom`]-dimensioned (never bare `f64`).

use uom::si::energy::electronvolt;
use uom::si::f64::{Energy, MassRate, Power, Ratio, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Number of energy groups in the Mk1 multi-group model (Table 3.4).
pub const NUM_ENERGY_GROUPS: usize = 8;

/// Number of delayed-neutron precursor groups (Eq. 2.20 / 2.22).
pub const NUM_DELAYED_GROUPS: usize = 6;

/// Lower energy boundary of each of the 8 groups, in electron-volts
/// (Table 3.4). Index 0 is group 1 (fast). Group 8's lower bound is 0 (thermal
/// cutoff). The upper bound of group 1 is the ENDF maximum (~20 MeV).
///
/// Returns `uom` [`Energy`] values; construction is not `const` because
/// `Energy::new` is not a `const fn`.
pub fn energy_group_lower_bounds() -> [Energy; NUM_ENERGY_GROUPS] {
    // Table 3.4 lower bounds, MeV -> eV.
    const LOWER_BOUNDS_EV: [f64; NUM_ENERGY_GROUPS] = [
        1.4e6,  // group 1
        2.5e4,  // group 2
        4.8e1,  // group 3
        4.0e0,  // group 4
        5.0e-1, // group 5
        1.9e-1, // group 6
        5.8e-2, // group 7
        0.0e0,  // group 8 (thermal cutoff)
    ];
    LOWER_BOUNDS_EV.map(Energy::new::<electronvolt>)
}

/// Fuel-failure safety limit for the graphite-based FHR fuel element
/// (dissertation §Abstract / §4.5.2): 1600 °C. The Fig. 4.29 result stays far
/// below this.
pub fn fuel_safety_limit() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(1600.0)
}

/// Mk1 core design parameters relevant to the coupled transient (Table 4.1).
///
/// Only the parameters the workflow needs at its API boundary are typed here;
/// the full table is in `docs/xin-wang-thesis/04-transients-fig4-29.md`.
#[derive(Debug, Clone, Copy)]
pub struct Mk1DesignPoint {
    /// Rated thermal power (236 MW).
    pub thermal_power: Power,
    /// Coolant (flibe) inlet temperature (600 °C).
    pub coolant_inlet_temperature: ThermodynamicTemperature,
    /// Coolant bulk-average outlet temperature (700 °C).
    pub coolant_outlet_temperature: ThermodynamicTemperature,
    /// Total coolant mass flow (976 kg/s).
    pub coolant_mass_flow: MassRate,
    /// U-235 fuel enrichment as a fraction (0.199).
    pub fuel_enrichment: Ratio,
    /// Pebble packing fraction (0.60).
    pub pebble_packing_fraction: Ratio,
    /// TRISO packing fraction inside the fuel annulus (0.40).
    pub triso_packing_fraction: Ratio,
}

impl Mk1DesignPoint {
    /// The nominal Mk1 operating point from Table 4.1.
    pub fn nominal() -> Self {
        Self {
            thermal_power: Power::new::<watt>(236.0e6),
            coolant_inlet_temperature: ThermodynamicTemperature::new::<degree_celsius>(600.0),
            coolant_outlet_temperature: ThermodynamicTemperature::new::<degree_celsius>(700.0),
            coolant_mass_flow: MassRate::new::<kilogram_per_second>(976.0),
            fuel_enrichment: Ratio::new::<ratio>(0.199),
            pebble_packing_fraction: Ratio::new::<ratio>(0.60),
            triso_packing_fraction: Ratio::new::<ratio>(0.40),
        }
    }
}

/// Definition of the control-rod-removal transient (§4.5.2), the scenario that
/// produces Figure 4.29.
#[derive(Debug, Clone, Copy)]
pub struct ControlRodRemovalTransient {
    /// Total number of control rods in the core (8, Table 4.6).
    pub total_control_rods: usize,
    /// Number of rods removed to trigger the transient (3 of 8).
    pub rods_removed: usize,
    /// Excess reactivity available if *all* rods were removed from the initial
    /// symmetric insertion (3941 pcm).
    pub all_rods_out_excess_reactivity: Ratio,
    /// Simulated transient duration (100 s).
    pub duration: Time,
}

impl ControlRodRemovalTransient {
    /// The Figure-4.29 transient as defined in §4.5.2: rods pre-inserted
    /// symmetrically to a 3941 pcm all-rods-out excess, then 3 of 8 rods removed
    /// promptly (bounding case), 100 s.
    pub fn fig_4_29() -> Self {
        Self {
            total_control_rods: 8,
            rods_removed: 3,
            all_rods_out_excess_reactivity: Ratio::new::<ratio>(3941.0e-5),
            duration: Time::new::<second>(100.0),
        }
    }
}

/// A single digitised point on the Figure-4.29 reference curve.
#[derive(Debug, Clone, Copy)]
pub struct Fig429Point {
    /// Time since transient start.
    pub time: Time,
    /// Maximum fuel temperature (centre of the hottest fuel kernel).
    pub max_fuel_temperature: ThermodynamicTemperature,
}

/// The digitised Figure-4.29 reference curve: maximum fuel temperature vs time
/// during the control-rod-removal transient.
///
/// **Approximate** — read by eye off the printed plot to ~5 °C resolution (the
/// thesis does not tabulate it). Shape: prompt jump to a ~988 °C peak near 8 s,
/// a shallow dip to ~975 °C near 28 s, then a slow climb to ~1006 °C at 100 s;
/// the maximum stays ~600 °C below the 1600 °C safety limit. A careful
/// re-digitisation should replace these before any quantitative pass/fail claim.
pub fn fig_4_29_reference_curve() -> Vec<Fig429Point> {
    // (t [s], T_fuel_max [°C]) digitised pairs — see 04-transients-fig4-29.md.
    const POINTS: [(f64, f64); 10] = [
        (0.0, 908.0),
        (2.0, 960.0),
        (5.0, 982.0),
        (8.0, 988.0), // peak
        (15.0, 982.0),
        (28.0, 975.0), // local minimum
        (40.0, 982.0),
        (60.0, 992.0),
        (80.0, 1000.0),
        (100.0, 1006.0),
    ];
    POINTS
        .iter()
        .map(|&(t, temp)| Fig429Point {
            time: Time::new::<second>(t),
            max_fuel_temperature: ThermodynamicTemperature::new::<degree_celsius>(temp),
        })
        .collect()
}
