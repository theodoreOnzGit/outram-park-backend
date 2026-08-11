//! # HTR-10 published design constants
//!
//! The HTR-10 operating point, core/pebble geometry, fuel specification, and
//! fuel-temperature limits, transcribed from published literature. Every value
//! carries its citation and access tier. Nothing here is measured, fitted, or
//! invented by this project.
//!
//! - **Belongs here:** cited design constants and trivial derived quantities
//!   (bed porosity from filling fraction) whose derivation is stated.
//! - **Does NOT belong here:** correlations ([`super::kta`], [`super::zbs`]),
//!   solver state, or any uncited number.
//!
//! Sources (see [`super`] for the on-disk catalogue paths):
//!
//! - **IAEA HTGR benchmark document, Chapter 4** — Open tier. Design and
//!   operating data plus the benchmark-problem definitions.
//! - **Gao & Shi (2002), "Thermal hydraulic calculation of the HTR-10 for the
//!   initial and equilibrium core", Nucl. Eng. Des. 218, 51-64,
//!   doi 10.1016/S0029-5493(02)00198-X** — Proprietary tier (Elsevier). Cited
//!   with provenance; numbers used, text not reproduced.

use uom::si::f64::{
    Length, Mass, MassDensity, MassRate, Power, Pressure, Ratio, ThermodynamicTemperature,
    Volume,
};
use uom::si::length::{centimeter, meter};
use uom::si::mass::gram;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::megapascal;
use uom::si::ratio::{percent, ratio};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::volume::cubic_meter;

/// The HTR-10 design point as specified in the IAEA HTGR benchmark document,
/// Chapter 4 (Open tier; catalogued at
/// `crates/kovan-literature/open/reports/htr-10-iaea.json`).
///
/// All fields are `uom` quantities; the doc comment of each field spells out
/// the published value and unit. Construct with
/// [`Htr10DesignPoint::iaea_benchmark`]. The struct is plain data — it holds
/// no solver state and performs no physics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10DesignPoint {
    /// Reactor thermal power: 10 MW.
    pub thermal_power: Power,
    /// Primary helium pressure: 3.0 MPa.
    pub primary_pressure: Pressure,
    /// Reactor core diameter: 180 cm (1.8 m).
    pub core_diameter: Length,
    /// Average core height: 197 cm (1.97 m).
    pub average_core_height: Length,
    /// Core volume as stated in the source: 5.0 m^3. See the geometry
    /// self-consistency test for how well this closes against diameter and
    /// height.
    pub core_volume: Volume,
    /// Average helium temperature at reactor outlet, phase 1 operation: 700 degrees Celsius.
    pub helium_outlet_phase1: ThermodynamicTemperature,
    /// Average helium temperature at reactor inlet, phase 1 operation: 250 degrees Celsius.
    pub helium_inlet_phase1: ThermodynamicTemperature,
    /// Helium outlet temperature for the planned phase 2 (gas-turbine) operation: 900 degrees Celsius.
    pub helium_outlet_phase2: ThermodynamicTemperature,
    /// Helium inlet temperature for the planned phase 2 operation: 300 degrees Celsius.
    pub helium_inlet_phase2: ThermodynamicTemperature,
    /// Helium mass flow rate at full power: 4.3 kg/s. (Gao & Shi 2002, Table 2,
    /// carries 4.32 kg/s for the equilibrium core at 100% load.)
    pub helium_mass_flow: MassRate,
    /// Main steam pressure at the steam-generator outlet: 4.0 MPa.
    pub main_steam_pressure: Pressure,
    /// Main steam temperature at the steam-generator outlet: 440 degrees Celsius.
    pub main_steam_temperature: ThermodynamicTemperature,
    /// Feedwater temperature: 104 degrees Celsius.
    pub feedwater_temperature: ThermodynamicTemperature,
    /// Main steam flow rate: 12.5 t/hr = 12500/3600 kg/s (about 3.472 kg/s).
    pub main_steam_mass_flow: MassRate,
    /// Number of fuel elements in the equilibrium core: 27,000.
    pub fuel_element_count: u32,
    /// Fuel-element (pebble) outer diameter: 6.0 cm.
    pub pebble_diameter: Length,
    /// Diameter of the fuelled zone inside a pebble: 5.0 cm.
    pub fuelled_zone_diameter: Length,
    /// Graphite density in the fuelled zone and outer shell: 1.73 g/cm^3.
    pub graphite_density: MassDensity,
    /// Heavy-metal (uranium) loading per fuel pebble: 5.0 g.
    pub heavy_metal_per_ball: Mass,
    /// Enrichment of U-235 in fresh fuel, by weight: 17%.
    pub enrichment: Ratio,
    /// Radius of the UO2 fuel kernel inside a coated particle: 0.025 cm
    /// (0.25 mm).
    pub fuel_kernel_radius: Length,
    /// UO2 kernel density: 10.4 g/cm^3.
    pub uo2_density: MassDensity,
    /// Volumetric filling fraction of balls in the core, f = 0.61
    /// (dimensionless). Bed porosity follows as eps = 1 - f = 0.39; use
    /// [`Htr10DesignPoint::bed_porosity`].
    pub filling_fraction: Ratio,
    /// Side reflector thickness: 100 cm (1.0 m).
    pub side_reflector_thickness: Length,
    /// Reactor pressure vessel inner diameter: 4.2 m.
    pub rpv_inner_diameter: Length,
    /// Reactor pressure vessel height: 11.1 m.
    pub rpv_height: Length,
}

impl Htr10DesignPoint {
    /// The HTR-10 design point, transcribed from the IAEA HTGR benchmark
    /// document, Chapter 4 (Open tier). Every value is the published one; see
    /// each field's doc comment. The unit tests in this module check the set
    /// for internal consistency and reproduce the published relations.
    pub fn iaea_benchmark() -> Self {
        Self {
            thermal_power: Power::new::<megawatt>(10.0),
            primary_pressure: Pressure::new::<megapascal>(3.0),
            core_diameter: Length::new::<centimeter>(180.0),
            average_core_height: Length::new::<centimeter>(197.0),
            core_volume: Volume::new::<cubic_meter>(5.0),
            helium_outlet_phase1: ThermodynamicTemperature::new::<degree_celsius>(700.0),
            helium_inlet_phase1: ThermodynamicTemperature::new::<degree_celsius>(250.0),
            helium_outlet_phase2: ThermodynamicTemperature::new::<degree_celsius>(900.0),
            helium_inlet_phase2: ThermodynamicTemperature::new::<degree_celsius>(300.0),
            helium_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            main_steam_pressure: Pressure::new::<megapascal>(4.0),
            main_steam_temperature: ThermodynamicTemperature::new::<degree_celsius>(440.0),
            feedwater_temperature: ThermodynamicTemperature::new::<degree_celsius>(104.0),
            // 12.5 t/hr, converted: 12.5 * 1000 kg / 3600 s.
            main_steam_mass_flow: MassRate::new::<kilogram_per_second>(12.5e3 / 3600.0),
            fuel_element_count: 27_000,
            pebble_diameter: Length::new::<centimeter>(6.0),
            fuelled_zone_diameter: Length::new::<centimeter>(5.0),
            graphite_density: MassDensity::new::<gram_per_cubic_centimeter>(1.73),
            heavy_metal_per_ball: Mass::new::<gram>(5.0),
            enrichment: Ratio::new::<percent>(17.0),
            fuel_kernel_radius: Length::new::<centimeter>(0.025),
            uo2_density: MassDensity::new::<gram_per_cubic_centimeter>(10.4),
            filling_fraction: Ratio::new::<ratio>(0.61),
            side_reflector_thickness: Length::new::<centimeter>(100.0),
            rpv_inner_diameter: Length::new::<meter>(4.2),
            rpv_height: Length::new::<meter>(11.1),
        }
    }

    /// Bed porosity (helium void fraction of the pebble bed), dimensionless:
    /// eps = 1 - f, where f is the volumetric filling fraction of balls in
    /// the core (0.61, IAEA benchmark document). Hence eps = 0.39. This is
    /// the porosity the KTA pressure-drop correlation ([`super::kta`])
    /// expects for the HTR-10 bed.
    pub fn bed_porosity(&self) -> Ratio {
        Ratio::new::<ratio>(1.0) - self.filling_fraction
    }
}

/// HTR-10 fuel-temperature limits and core-flow-distribution figures from
/// Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64,
/// doi 10.1016/S0029-5493(02)00198-X (Proprietary tier; catalogued at
/// `crates/kovan-literature/proprietary/papers/gao2002htr10th.json`).
///
/// **Do not conflate the two temperature figures in this struct with the
/// 1600 degrees Celsius coated-particle figure** — see
/// [`generic_coated_particle_retention_limit`]. Mixing them up misstates the
/// HTR-10 safety margin by 370 K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10FuelTemperatureLimits {
    /// Calculated maximum fuel temperature at 120% overload, equilibrium
    /// core: 1046.6 degrees Celsius (Gao & Shi 2002, Table 2; often quoted
    /// rounded as 1046 degrees Celsius). This is a *calculated best-estimate
    /// peak*, not a limit.
    pub max_fuel_temperature_at_120_percent_overload: ThermodynamicTemperature,
    /// The HTR-10's *own specified* maximum fuel temperature limit under
    /// normal and accident conditions: 1230 degrees Celsius (Gao & Shi 2002,
    /// set from the experimental demonstration that the coating retains
    /// fission products up to 1250 degrees Celsius). This is **not** the
    /// generic 1600 degrees Celsius figure of the modular-HTR literature.
    pub fuel_temperature_limit: ThermodynamicTemperature,
    /// Maximum bypass flow through the gaps between graphite components:
    /// less than 10% of rated flow (Gao & Shi 2002). Stored as the bounding
    /// fraction 0.10, dimensionless. A bed model that ignores bypass is
    /// therefore making up to a 10% error in core flow.
    pub max_bypass_flow_fraction: Ratio,
    /// Conservative minimum fraction of rated flow through the pebble-bed
    /// core: 86% (Gao & Shi 2002: less than 10% bypass, about 2.5% through
    /// control-rod tubes, at least 1% through the fuel discharge tube).
    /// Stored as 0.86, dimensionless.
    pub min_core_flow_fraction: Ratio,
}

impl Htr10FuelTemperatureLimits {
    /// The values published in Gao & Shi (2002) — see each field's doc
    /// comment for the individual citations. The initial core (graphite
    /// balls mixed with fuel balls) and the equilibrium core (no graphite
    /// balls) are two DISTINCT cases in that paper; the overload peak here
    /// is the equilibrium-core figure.
    pub fn gao_shi_2002() -> Self {
        Self {
            max_fuel_temperature_at_120_percent_overload:
                ThermodynamicTemperature::new::<degree_celsius>(1046.6),
            fuel_temperature_limit: ThermodynamicTemperature::new::<degree_celsius>(1230.0),
            max_bypass_flow_fraction: Ratio::new::<ratio>(0.10),
            min_core_flow_fraction: Ratio::new::<ratio>(0.86),
        }
    }
}

/// The GENERIC coated-particle fission-product retention limit that pervades
/// the modular-HTR literature: 1600 degrees Celsius. **This is NOT an HTR-10
/// limit.** The HTR-10's own specified maximum fuel temperature limit is
/// 1230 degrees Celsius
/// ([`Htr10FuelTemperatureLimits::fuel_temperature_limit`], Gao & Shi 2002).
/// This function exists precisely so the two numbers cannot be silently
/// conflated: any HTR-10 margin calculation must use 1230 degrees Celsius,
/// not this figure.
pub fn generic_coated_particle_retention_limit() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(1600.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::TemperatureInterval;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::{megawatt, watt};
    use uom::si::ratio::ratio;
    use uom::si::temperature_interval::kelvin as interval_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::volume::cubic_meter;

    /// V&V: HTR-10 operating-point energy balance against the real helium EOS.
    ///
    /// **Methodology.** The IAEA HTR-10 benchmark document (Open tier, see
    /// [`super`]) states thermal power 10 MW, helium mass flow 4.3 kg/s, and
    /// phase-1 helium temperatures 250 -> 700 degrees Celsius (dT = 450 K).
    /// The check computes Q = mdot * cp * dT with cp taken from
    /// `outram_park_fork_coolprop::state_pt` (Helmholtz EOS, helium of
    /// Ortiz-Vega et al.) at the primary pressure 3.0 MPa and the arithmetic
    /// mean coolant temperature 475 degrees Celsius (748.15 K), and compares
    /// it to the published 10 MW. Pass criterion: |Q - 10 MW| / 10 MW < 2%.
    /// Helium cp is nearly temperature-independent at these conditions, so
    /// the mean-temperature evaluation is representative.
    ///
    /// **Results (recorded 2026-08-11, coolprop workspace version).**
    /// cp(748.15 K, 3.0 MPa) = 5191.5 J/(kg K);
    /// Q = 4.3 kg/s * 5191.5 J/(kg K) * 450 K = 10.0455 MW, a closure of
    /// +0.455% against the published 10 MW. The published operating point is
    /// therefore self-consistent with the real helium EOS to well within the
    /// 2% criterion. The residual +0.5% is consistent with the source's
    /// 2-significant-figure mass flow (4.3 kg/s) and with the secondary duty
    /// sitting slightly below 10 MW (see the steam-side test): the balance
    /// does not close to zero, and no attempt is made to pretend it does.
    #[test]
    fn operating_point_energy_balance_closes_against_coolprop_helium_cp() {
        use outram_park_fork_coolprop::{state_pt, Fluid};

        let d = Htr10DesignPoint::iaea_benchmark();
        let t_mean_kelvin = 0.5
            * (d.helium_outlet_phase1.get::<kelvin>() + d.helium_inlet_phase1.get::<kelvin>());
        let p_pascal = d.primary_pressure.value;

        let state = state_pt(Fluid::Helium, t_mean_kelvin, p_pascal)
            .expect("helium (T, p) flash must converge at 748.15 K, 3.0 MPa");
        assert!(state.cp.is_finite() && state.cp > 0.0);

        // uom's `TemperatureKind` forbids subtracting absolute temperatures
        // directly, so the interval is formed from the kelvin scalars.
        let dt: TemperatureInterval = TemperatureInterval::new::<interval_kelvin>(
            d.helium_outlet_phase1.get::<kelvin>() - d.helium_inlet_phase1.get::<kelvin>(),
        );
        assert!((dt.get::<interval_kelvin>() - 450.0).abs() < 1e-9);

        // Q = mdot * cp * dT, in watts (all SI).
        let q_watt =
            d.helium_mass_flow.get::<kilogram_per_second>() * state.cp * dt.get::<interval_kelvin>();
        let q = Power::new::<watt>(q_watt);

        let closure = (q - d.thermal_power) / d.thermal_power;
        println!(
            "cp = {:.1} J/(kg K); Q = {:.4} MW; closure = {:+.3}%",
            state.cp,
            q.get::<megawatt>(),
            closure.get::<ratio>() * 100.0
        );
        assert!(
            closure.get::<ratio>().abs() < 0.02,
            "energy balance does not close: Q = {} MW vs published 10 MW",
            q.get::<megawatt>()
        );
    }

    /// V&V: HTR-10 secondary-side (steam) energy balance against IAPWS-IF97.
    ///
    /// **Methodology.** The IAEA HTR-10 benchmark document (Open tier) states
    /// main steam 4.0 MPa / 440 degrees Celsius at the steam-generator
    /// outlet, feedwater 104 degrees Celsius, and main steam flow 12.5 t/hr
    /// (3.4722 kg/s). The check computes the steam-generator duty
    /// Q = mdot_steam * (h_steam - h_feedwater) with both enthalpies from
    /// `tampines_steam_tables::h_tp_eqm_single_phase` (IAPWS-IF97), the
    /// feedwater evaluated at the main-steam pressure 4.0 MPa (its pressure
    /// is not stated in the source; the compressed-liquid enthalpy is
    /// insensitive to this choice). Pass criterion: |Q - 10 MW| / 10 MW < 3%
    /// (the secondary duty is slightly below thermal power because of primary
    /// circuit heat loss and circulator arrangement, neither quantified in
    /// the source).
    ///
    /// **Results (recorded 2026-08-11).**
    /// h(440 C, 4.0 MPa) = 3307.9 kJ/kg, h(104 C, 4.0 MPa) = 438.9 kJ/kg;
    /// Q = 3.4722 kg/s * 2869.0 kJ/kg = 9.9618 MW, a closure of -0.382%
    /// against the published 10 MW thermal power. The published steam-side
    /// figures are self-consistent with IF97 to well within the criterion.
    #[test]
    fn secondary_steam_energy_balance_closes_against_if97() {
        use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;

        let d = Htr10DesignPoint::iaea_benchmark();
        let h_steam = h_tp_eqm_single_phase(d.main_steam_temperature, d.main_steam_pressure);
        let h_feed = h_tp_eqm_single_phase(d.feedwater_temperature, d.main_steam_pressure);

        let q: Power = d.main_steam_mass_flow * (h_steam - h_feed);
        let closure = (q - d.thermal_power) / d.thermal_power;
        println!(
            "h_steam = {:.1} kJ/kg; h_feed = {:.1} kJ/kg; Q = {:.4} MW; closure = {:+.3}%",
            h_steam.value / 1e3,
            h_feed.value / 1e3,
            q.get::<megawatt>(),
            closure.get::<ratio>() * 100.0
        );
        assert!(
            closure.get::<ratio>().abs() < 0.03,
            "steam-side energy balance does not close: Q = {} MW vs published 10 MW",
            q.get::<megawatt>()
        );
    }

    /// V&V: HTR-10 core volume closes against cited diameter and height.
    ///
    /// **Methodology.** The IAEA HTR-10 benchmark document (Open tier) states
    /// core diameter 180 cm, average core height 197 cm, and core volume
    /// 5.0 m^3. The check computes V = (pi/4) d^2 h from the first two and
    /// compares to the third. Pass criterion: closure within 1%.
    ///
    /// **Results (recorded 2026-08-11).** (pi/4) * 1.8^2 * 1.97 =
    /// 5.0130 m^3, i.e. +0.261% against the stated 5.0 m^3. The stated volume
    /// is the cylinder volume rounded to 2 significant figures; the three
    /// numbers are mutually consistent.
    #[test]
    fn core_volume_closes_against_cited_diameter_and_height() {
        let d = Htr10DesignPoint::iaea_benchmark();
        let v_computed: Volume =
            d.core_diameter * d.core_diameter * d.average_core_height
                * Ratio::new::<ratio>(std::f64::consts::FRAC_PI_4);
        let closure = (v_computed - d.core_volume) / d.core_volume;
        println!(
            "V(d, h) = {:.4} m^3 vs stated {:.1} m^3; closure = {:+.3}%",
            v_computed.get::<cubic_meter>(),
            d.core_volume.get::<cubic_meter>(),
            closure.get::<ratio>() * 100.0
        );
        assert!(closure.get::<ratio>().abs() < 0.01);
    }

    /// V&V: pebble count, pebble diameter, filling fraction, and core volume
    /// are mutually consistent.
    ///
    /// **Methodology.** The IAEA HTR-10 benchmark document (Open tier) states
    /// 27,000 fuel elements of diameter 6.0 cm in the equilibrium core,
    /// core volume 5.0 m^3, and volumetric filling fraction f = 0.61. The
    /// check computes the total pebble volume 27000 * (pi/6) d^3 and divides
    /// by the stated core volume to get an implied filling fraction, compared
    /// to the published 0.61. Pass criterion: closure within 1%.
    ///
    /// **Results (recorded 2026-08-11).** Total pebble volume = 3.0536 m^3;
    /// implied f = 3.0536 / 5.0 = 0.6107, i.e. +0.119% against the published
    /// 0.61 (or f = 0.6091, -0.14%, against the unrounded cylinder volume
    /// 5.0130 m^3). The four numbers are mutually consistent to about one
    /// part in a thousand — the design set is coherent.
    #[test]
    fn pebble_count_and_filling_fraction_reproduce_core_volume() {
        let d = Htr10DesignPoint::iaea_benchmark();
        let pebble_volume: Volume = d.pebble_diameter * d.pebble_diameter * d.pebble_diameter
            * Ratio::new::<ratio>(std::f64::consts::PI / 6.0);
        let total_pebble_volume = pebble_volume * Ratio::new::<ratio>(d.fuel_element_count as f64);
        let implied_f: Ratio = total_pebble_volume / d.core_volume;
        let closure =
            (implied_f - d.filling_fraction) / d.filling_fraction;
        println!(
            "total pebble volume = {:.4} m^3; implied f = {:.4} vs published 0.61; closure = {:+.3}%",
            total_pebble_volume.get::<cubic_meter>(),
            implied_f.get::<ratio>(),
            closure.get::<ratio>() * 100.0
        );
        assert!(closure.get::<ratio>().abs() < 0.01);
    }

    /// V&V: bed porosity is exactly one minus the filling fraction.
    ///
    /// **Methodology.** The IAEA HTR-10 benchmark document (Open tier) states
    /// the volumetric filling fraction of balls in the core as f = 0.61; bed
    /// porosity is by definition eps = 1 - f = 0.39, the value the KTA
    /// pressure-drop correlation consumes (and the value the VTB generic
    /// pebble-bed tutorial uses for its bed, `bed_porosity = 0.39` in
    /// `step2.i`). Pass criterion: exact to floating-point roundoff (1e-12).
    ///
    /// **Results (recorded 2026-08-11).** eps = 1 - 0.61 = 0.39 exactly
    /// (residual 0.0 at f64 precision). The porosity used throughout this
    /// module is consistent with the published filling fraction.
    #[test]
    fn porosity_is_one_minus_filling_fraction() {
        let d = Htr10DesignPoint::iaea_benchmark();
        let eps = d.bed_porosity();
        println!("eps = {:.12}", eps.get::<ratio>());
        assert!((eps.get::<ratio>() - 0.39).abs() < 1e-12);
    }

    /// V&V: the HTR-10 fuel-temperature figures are distinct, correctly
    /// ordered, and give a positive margin.
    ///
    /// **Methodology.** Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64
    /// (Proprietary tier, cited not re-hosted) state: calculated maximum fuel
    /// temperature 1046.6 degrees Celsius at 120% overload (equilibrium
    /// core, Table 2), and the HTR-10's own specified fuel temperature limit
    /// 1230 degrees Celsius under normal and accident conditions. The
    /// generic coated-particle retention figure of the modular-HTR
    /// literature is 1600 degrees Celsius and is NOT the HTR-10 limit. The
    /// check asserts peak < HTR-10 limit < generic figure, and that the
    /// margin equals the published difference. Pass criterion: margin =
    /// 1230 - 1046.6 = 183.4 K to 0.01 K; limits differ by 370 K.
    ///
    /// **Results (recorded 2026-08-11).** Margin to the HTR-10 limit =
    /// 183.4 K (as published: "lower than the allowable value 1230 C").
    /// Distance from the HTR-10 limit to the generic 1600 C figure = 370 K —
    /// conflating the two would overstate the HTR-10 margin by a factor of
    /// three (553.4 K vs 183.4 K).
    #[test]
    fn fuel_temperature_limits_are_distinct_and_margin_is_positive() {
        let l = Htr10FuelTemperatureLimits::gao_shi_2002();
        let generic = generic_coated_particle_retention_limit();

        // uom's `TemperatureKind` forbids subtracting absolute temperatures
        // directly, so the intervals are formed from the kelvin scalars.
        let margin: TemperatureInterval = TemperatureInterval::new::<interval_kelvin>(
            l.fuel_temperature_limit.get::<kelvin>()
                - l.max_fuel_temperature_at_120_percent_overload.get::<kelvin>(),
        );
        let limit_gap: TemperatureInterval = TemperatureInterval::new::<interval_kelvin>(
            generic.get::<kelvin>() - l.fuel_temperature_limit.get::<kelvin>(),
        );
        println!(
            "margin = {:.1} K; HTR-10 limit vs generic figure gap = {:.1} K",
            margin.get::<interval_kelvin>(),
            limit_gap.get::<interval_kelvin>()
        );
        assert!((margin.get::<interval_kelvin>() - 183.4).abs() < 0.01);
        assert!((limit_gap.get::<interval_kelvin>() - 370.0).abs() < 0.01);
        // Flow-distribution fractions: bypass < 10%, core flow >= 86%.
        assert!((l.max_bypass_flow_fraction.get::<ratio>() - 0.10).abs() < 1e-12);
        assert!((l.min_core_flow_fraction.get::<ratio>() - 0.86).abs() < 1e-12);
    }
}
