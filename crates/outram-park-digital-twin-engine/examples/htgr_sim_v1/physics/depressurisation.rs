//! **DLOFC + ATWS + air ingress** (source-term stage 4, gh:#402; this is
//! stage 4a): the primary depressurises through a ruptured DN65 fuel-loading
//! tube, the circulator stops, the rods do NOT drop (anticipated transient
//! without scram), and air reaching the core oxidises the bed graphite.
//!
//! **Air ingress (since 2026-09-30, #420):** *air ingress via the Gao & Shi
//! cavity-ventilation rate, assumed to exchange the core gas; #420.* The core
//! gas is exchanged with air at **100 %/day for 72 h, then sealed** (Gao &
//! Shi 2002 s.5.3.2), which is `sembawang`'s
//! `Venting::gao_shi_htr10_cavity_ventilation()`; the rate and cut-off are
//! read from it, not copied. The ASSUMPTION, stated: Gao & Shi give the
//! reactor **cavity's** air change, and the core is taken to be well mixed
//! with the cavity through the break (maintainer's choice, 2026-09-30,
//! accepted for `sembawang` in be8d04ccfe, #447).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`). Not a licensing
//! source term, not a PSA, not validated.
//!
//! ```text
//!  primary gas --DN65 blowdown (6.58 kg/s initial, ~150 kg)--> stack (building not credited, gh:#409)
//!      ^  plate-out lift-off (I, metals: +2.4 x the circulating activity)
//!      ^  dust (Cs 5 %, Sr/Ag 20 %, I 1 % of plate-out; 10 % of it released)
//!      ^  purification system (noble 100 %, I and metals 10 %; isolation assumed to fail)
//!  air --O2 supply = lambda x V_void x 0.2095 x P/(R T), 100 %/day for 72 h (Gao & Shi cavity rate, #420)--> C + O2 -> CO2 in the bed, -393.7 kJ/mol
//!  primary gas <--exchange 1 - exp(-lambda dt), same rate and cut-off--> stack (circulating activity)
//! ```
//!
//! # Inputs, and where each comes from
//!
//! | Quantity | Value | Source |
//! |---|---|---|
//! | break | DN65 fuel-loading tube, top of the RPV | Gao & Shi (2002) s.5.3.1; Jiang et al. (2002) |
//! | initial discharge | **6.58 kg/s** | Gao & Shi s.5.3.1 |
//! | total discharge | **~150 kg** | Gao & Shi s.5.3.1 ("about 150 kg") |
//! | discharge **shape** | exponential, `tau = 150 / 6.58 = 22.8 s` | **ASSUMED, labelled**: the paper gives the initial rate, the total and "drained within 2 min" (5 tau = 114 s) |
//! | primary/secondary isolated | 28.06 s | Gao & Shi s.5.3.1 |
//! | scram | **none** (ATWS) | the scenario; Gao & Shi scram at 7.06 s in their DBA |
//! | plate-out desorption | +2.4 x the originally present coolant activity, iodine and metals | Liu & Cao (2002) s.4.1.1.2 (after INTERATOM 1989) |
//! | dust | Cs 5 %, Sr/Rb/Ag 20 %, I 1 % of the deposited activity on dust; 10 % of the dust released | Liu & Cao s.4.1.1.3 |
//! | purification system | 100 % of noble gas, 10 % of iodine and metals released (isolation fails) | Liu & Cao s.4.1.1.4 |
//! | graphite oxidation | IG-110 in air, Contescu (2011) Arrhenius, O2-supply limited | `boon_lay::chemistry::graphite_air` |
//! | ~~**O2 supply into the core**~~ | ~~**0 until a source is given**~~ | ~~**NOT PUBLISHED** in the held literature (no HTR-10 cavity volume or air-ingress rate; searched Gao & Shi, Jiang, JAERI-Conf 96-010, TECDOC-1382, -1694, NUREG/CR-6844, Oh & Kim). Tracked as **gh:#420**. Never derived from Gao & Shi's 319.2 kg of corrosion.~~ **SUPERSEDED 2026-09-30 (#420)** by the two rows below. Still true: no HTR-10 *core* air-ingress rate is published, and the 319.2 kg is still not used. |
//! | air exchange rate, cut-off | `lambda` = 1/day, 72 h, then sealed | Gao & Shi (2002) s.5.3.2, the reactor-**cavity** venting flow; via `sembawang::accident::release::Venting::gao_shi_htr10_cavity_ventilation()`. **ASSUMED to exchange the core gas** (#420, #447) |
//! | **O2 supply into the core** | `lambda x V_void x x_O2 x P / (R T)`, 72 h, then 0 | `V_void` = bed void volume (0.39 x 5.013 m^3 = 1.955 m^3, `one_node::bed_void_volume`); `x_O2` = 0.2095 (US Standard Atmosphere 1976, Table 3: 0.209476); `P` = 1 atm (post-blowdown, **assumed** equalised with the cavity); `T` = the bed temperature (the gas in the bed); `R` CODATA 2018 |
//! | circulating-activity exchange | `1 - exp(-lambda dt)` of the circulating pool per step, same window | the same ventilation; the whole primary circulating pool is taken as well mixed with the core gas (**assumed**, errs high on release) |
//!
//! # Not carried, stated
//!
//! - **The primary pressure is not a state of the thermal-hydraulic model**:
//!   the helium CVs keep their 3.0 MPa properties after the blowdown; only
//!   the release channel sees the mass leave. With the circulator stopped the
//!   helium's role is small, and the passive conduction-radiation path does
//!   not depend on pressure; the stored helium energy it misplaces is < 0.1 %
//!   of the graphite's.
//! - **Release height:** Jiang et al. (2002) route a DN65 depressurisation
//!   through a rupture disk and a **22.6 m** chimney without filtering; the
//!   dispersion uses the 40 m stack. Stated, not modelled (gh:#420).
//! - Liu & Cao's "might be overestimated by a factor of 3" on desorption is
//!   not applied (conservative, as they are).

use boon_lay::chemistry::graphite_air::{self, CO2_REACTION_ENTHALPY_J_PER_MOL};
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;

/// Initial blowdown rate \[kg/s\] (Gao & Shi 2002 s.5.3.1).
pub const INITIAL_DISCHARGE_KG_PER_S: f64 = 6.58;
/// Total discharge \[kg\] (Gao & Shi s.5.3.1, "about 150 kg").
pub const TOTAL_DISCHARGE_KG: f64 = 150.0;
/// Primary and secondary isolated \[s\] (Gao & Shi s.5.3.1, 28.06 s).
pub const ISOLATED_AT_S: f64 = 28.06;
/// Plate-out desorption multiplier on the originally present coolant
/// activity, iodine and metals (Liu & Cao s.4.1.1.2: "approximate 2.4 times").
pub const DESORPTION_MULTIPLE: f64 = 2.4;
/// Fraction of the dust released to the building during depressurisation
/// (Liu & Cao s.4.1.1.3, after INTERATOM 1989: 10 %).
pub const DUST_RELEASED_FRACTION: f64 = 0.10;
// ~~The O2 supply into the core [mol/s]: **not published for HTR-10**
// (module doc; gh:#420). Zero until a source is given.~~ REMOVED 2026-09-30
// (#420): was `PUBLISHED_OXYGEN_SUPPLY_MOL_PER_S = None`; the supply is now
// `Depressurisation::ventilation_oxygen_supply_mol_per_s`.

/// O2 mole fraction of dry air, 0.2095 (US Standard Atmosphere 1976,
/// NOAA/NASA/USAF, Table 3: 0.209476, rounded to four figures).
pub const O2_MOLE_FRACTION_OF_AIR: f64 = 0.2095;
/// Gas pressure in the core after the blowdown \[Pa\]: 1 atm, **assumed**
/// (the primary equalised with the cavity through the break).
pub const POST_BLOWDOWN_PRESSURE_PA: f64 = 101_325.0;
/// Molar gas constant \[J/(mol K)\], CODATA 2018 (exact).
const R_J_PER_MOL_K: f64 = 8.314_462_618;

/// The air-ingress ventilation as `(rate [1/s], cut-off [s])`, read from
/// `sembawang`'s `Venting::gao_shi_htr10_cavity_ventilation()` (Gao & Shi
/// 2002 s.5.3.2: 100 %/day, sealed after 72 h) so that the live DLOFC and
/// the `sembawang` source-term chain use one number.
///
/// # Panics
/// If `sembawang` ever stops returning `Venting::Ventilation` there.
pub fn gao_shi_ventilation() -> (f64, f64) {
    use sembawang::accident::release::Venting;
    match Venting::gao_shi_htr10_cavity_ventilation() {
        Venting::Ventilation { rate, cut_off } => (
            rate.get::<uom::si::frequency::hertz>(),
            cut_off.map_or(f64::INFINITY, |t| t.get::<uom::si::time::second>()),
        ),
        other => panic!("gao_shi_htr10_cavity_ventilation returned {other:?}"),
    }
}

/// Seconds of `[t0, t0 + dt]` that fall inside the ventilation window
/// `[0, cut_off]`.
fn ventilated_seconds(t0: f64, dt: f64, cut_off: f64) -> f64 {
    ((t0 + dt).min(cut_off) - t0.max(0.0)).max(0.0)
}

/// Fraction of an element's deposited activity carried on dust (Liu & Cao
/// s.4.1.1.3: 5 % Cs, 20 % Sr, Rb and Ag, 1 % I). `None` for elements they do
/// not name, and for noble gases (which do not deposit).
pub fn dust_share(z: u32) -> Option<f64> {
    match z {
        55 => Some(0.05),
        38 | 37 | 47 => Some(0.20),
        53 => Some(0.01),
        _ => None,
    }
}

/// Fraction of the purification system's hold-up released when its
/// isolation fails (Liu & Cao s.4.1.1.4): 100 % of noble gases (and H-3,
/// C-14), 10 % of iodine and metal fission products.
pub fn purification_release_fraction(z: u32) -> f64 {
    match z {
        36 | 54 | 1 | 6 => 1.0,
        _ => 0.10,
    }
}

/// Whether a nuclide's element desorbs from surfaces on depressurisation
/// (Liu & Cao s.4.1.1.2: "metal fission products and iodine"), i.e. anything
/// but a noble gas.
pub fn desorbs(z: u32) -> bool {
    !matches!(z, 36 | 54)
}

/// The accident's own state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Depressurisation {
    /// Seconds since the rupture.
    pub elapsed_s: f64,
    /// Helium discharged so far \[kg\].
    pub discharged_kg: f64,
    /// Helium in the primary at the rupture \[kg\].
    pub initial_inventory_kg: f64,
    /// Fraction of the primary gas vented so far by the **blowdown**.
    pub vented_fraction: f64,
    /// Fraction of the primary gas exchanged so far by the **air-ingress
    /// ventilation** (#420): `1 - exp(-lambda min(t, 72 h))`.
    pub exchanged_fraction: f64,
    /// O2 delivered to the core so far \[mol\].
    pub oxygen_supplied_mol: f64,
    /// Graphite oxidised so far \[kg\].
    pub carbon_oxidised_kg: f64,
    /// Heat the oxidation released so far \[J\] (positive).
    pub oxidation_heat_j: f64,
    /// Whether the oxidation rate was set by the O2 supply at the last step.
    pub oxygen_limited: bool,
}

/// What one step asks of the plant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepressurisationStep {
    /// Fraction of the primary gas (and its circulating activity) vented
    /// this step by the blowdown.
    pub vented_fraction: f64,
    /// Fraction of the remaining primary gas (and its circulating activity)
    /// exchanged with air this step by the ventilation (#420).
    pub exchanged_fraction: f64,
    /// Heat the oxidation put into the bed graphite this step \[W\].
    pub oxidation_heat_w: f64,
    /// Whether the secondary is isolated.
    pub secondary_isolated: bool,
}

impl Depressurisation {
    /// The rupture, with `inventory_kg` of helium in the primary.
    pub fn start(inventory_kg: f64) -> Self {
        Self {
            elapsed_s: 0.0,
            discharged_kg: 0.0,
            initial_inventory_kg: inventory_kg,
            vented_fraction: 0.0,
            exchanged_fraction: 0.0,
            oxygen_supplied_mol: 0.0,
            carbon_oxidised_kg: 0.0,
            oxidation_heat_j: 0.0,
            oxygen_limited: false,
        }
    }

    /// Cumulative discharge at `t` \[kg\]: `M (1 - exp(-t/tau))`,
    /// `tau = M / m0` (the labelled exponential shape).
    pub fn discharged_by(t: f64) -> f64 {
        let tau = TOTAL_DISCHARGE_KG / INITIAL_DISCHARGE_KG_PER_S;
        TOTAL_DISCHARGE_KG * (1.0 - (-t.max(0.0) / tau).exp())
    }

    /// The air-ingress O2 supply over the next `dt` \[mol/s\], averaged over
    /// the step: `lambda x V x x_O2 x P / (R T)` for the part of the step
    /// inside the 72 h window, 0 after it. `core_gas_volume_m3` is the core's
    /// free-gas volume, `gas_temperature` the gas's (the bed's).
    pub fn ventilation_oxygen_supply_mol_per_s(
        &self,
        dt: f64,
        core_gas_volume_m3: f64,
        gas_temperature: ThermodynamicTemperature,
    ) -> f64 {
        let (rate, cut_off) = gao_shi_ventilation();
        let t = gas_temperature.get::<uom::si::thermodynamic_temperature::kelvin>();
        let moles = POST_BLOWDOWN_PRESSURE_PA * core_gas_volume_m3 / (R_J_PER_MOL_K * t);
        rate * moles * O2_MOLE_FRACTION_OF_AIR * ventilated_seconds(self.elapsed_s, dt, cut_off)
            / dt
    }

    /// Advance by `dt` \[s\] with the bed graphite at `graphite_temperature`
    /// and `graphite_mass_kg`, and an O2 supply `oxygen_supply_mol_per_s`
    /// (the live plant passes
    /// [`Self::ventilation_oxygen_supply_mol_per_s`]). The ventilation's
    /// exchange of the primary gas is computed here, from the same window.
    pub fn step(
        &mut self,
        dt: f64,
        graphite_temperature: ThermodynamicTemperature,
        graphite_mass_kg: f64,
        oxygen_supply_mol_per_s: f64,
    ) -> DepressurisationStep {
        let t1 = self.elapsed_s + dt;
        let out = Self::discharged_by(t1) - Self::discharged_by(self.elapsed_s);
        let held = (self.initial_inventory_kg - self.discharged_kg).max(0.0);
        let vented = if held > 0.0 {
            (out / held).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.discharged_kg += out;
        self.vented_fraction = 1.0 - (1.0 - self.vented_fraction) * (1.0 - vented);
        let (rate, cut_off) = gao_shi_ventilation();
        let exchanged = -(-rate * ventilated_seconds(self.elapsed_s, dt, cut_off)).exp_m1();
        self.exchanged_fraction = 1.0 - (1.0 - self.exchanged_fraction) * (1.0 - exchanged);
        self.oxygen_supplied_mol += oxygen_supply_mol_per_s * dt;
        self.elapsed_s = t1;

        // Graphite oxidation: every O2 that arrives reacts unless the
        // kinetics are slower (they are not, at core temperatures). Air at
        // 21 % O2 is the kinetic reference pressure.
        let (mol_per_s, limit, _validity) = graphite_air::gasification_rate(
            graphite_temperature,
            Pressure::new::<pascal>(graphite_air::REFERENCE_O2_PA),
            graphite_mass_kg,
            oxygen_supply_mol_per_s,
        );
        self.oxygen_limited = limit == graphite_air::Limit::OxygenSupply;
        let mol = mol_per_s * dt;
        self.carbon_oxidised_kg += mol * 12.011e-3;
        let heat = -CO2_REACTION_ENTHALPY_J_PER_MOL * mol;
        self.oxidation_heat_j += heat;

        DepressurisationStep {
            vented_fraction: vented,
            exchanged_fraction: exchanged,
            oxidation_heat_w: heat / dt,
            secondary_isolated: t1 >= ISOLATED_AT_S,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    /// **The blowdown follows the published rate and total, and the vented
    /// fractions compound to the discharged mass.** Methodology: 300 s at
    /// 0.1 s from 210 kg; check the first-step rate against 6.58 kg/s, the
    /// total against 150 kg (5 tau = 114 s: > 99 % by 2 min, "drained within
    /// 2 min"), `1 - vented = (210 - discharged)/210`, and no oxidation with
    /// no O2 supply. Results (2026-09-29): pass.
    #[test]
    fn the_blowdown_follows_the_published_rate_and_total() {
        let mut d = Depressurisation::start(210.0);
        let bed = ThermodynamicTemperature::new::<kelvin>(1200.0);
        let first = d.step(0.1, bed, 5000.0, 0.0);
        let rate = d.discharged_kg / 0.1;
        assert!((rate - 6.58).abs() / 6.58 < 0.01, "{rate}");
        assert!(!first.secondary_isolated);
        let mut last = first;
        for _ in 1..3000 {
            last = d.step(0.1, bed, 5000.0, 0.0);
        }
        assert!(last.secondary_isolated);
        assert!((d.discharged_kg - 150.0).abs() < 1e-3);
        assert!(Depressurisation::discharged_by(120.0) > 0.99 * 150.0);
        assert!(((1.0 - d.vented_fraction) - (210.0 - d.discharged_kg) / 210.0).abs() < 1e-12);
        assert_eq!(d.carbon_oxidised_kg, 0.0);
        println!(
            "blowdown: {:.2} kg out, vented {:.4} of the primary gas",
            d.discharged_kg, d.vented_fraction
        );
    }

    /// **The air-ingress ventilation (#420)** is Gao & Shi's cavity rate read
    /// from `sembawang`, supplies O2 inside the 72 h window and none after
    /// it, and exchanges the primary gas to `1 - e^-3 = 0.9502` by 72 h,
    /// then holds.
    ///
    /// Methodology: the supply at 1200 K for the 1.955 m^3 bed void,
    /// against `lambda P V x_O2 / (R T)` by hand (19.855 mol of gas, 4.160 mol
    /// O2, 4.815e-5 mol/s); a step straddling 72 h gets half the rate; after
    /// 72 h zero. Results (2026-09-30): pass; printed below.
    ///
    /// The exchange runs at a **constant** 1200 K bed: the live air ingress
    /// vents by gas exchange whatever the temperature does, not by heat-up
    /// expansion (checked for #469 item 4; no change was needed here).
    #[test]
    fn the_ventilation_supplies_oxygen_for_72_hours_then_none() {
        let (rate, cut_off) = gao_shi_ventilation();
        assert!((rate - 1.0 / 86_400.0).abs() < 1e-18 && cut_off == 72.0 * 3600.0);
        let bed = ThermodynamicTemperature::new::<kelvin>(1200.0);
        let mut d = Depressurisation::start(210.0);
        let q = d.ventilation_oxygen_supply_mol_per_s(1.0, 1.955, bed);
        let by_hand = 101_325.0 * 1.955 / (8.314_462_618 * 1200.0) * 0.2095 / 86_400.0;
        assert!(q > 0.0 && (q - by_hand).abs() < 1e-15, "{q} {by_hand}");
        d.elapsed_s = cut_off - 50.0;
        let half = d.ventilation_oxygen_supply_mol_per_s(100.0, 1.955, bed);
        assert!((half - 0.5 * by_hand).abs() < 1e-15);
        d.elapsed_s = cut_off;
        assert_eq!(
            d.ventilation_oxygen_supply_mol_per_s(100.0, 1.955, bed),
            0.0
        );
        // The exchanged fraction reaches 1 - e^-3 at 72 h and then holds.
        let mut e = Depressurisation::start(210.0);
        for _ in 0..(80 * 36) {
            e.step(100.0, bed, 5000.0, 0.0);
        }
        assert!((e.exchanged_fraction - (1.0 - (-3.0f64).exp())).abs() < 1e-12);
        println!(
            "O2 supply {q:.4e} mol/s at 1200 K; exchanged by 80 h {:.4}",
            e.exchanged_fraction
        );
    }

    /// With an O2 supply, the oxidation is supply-limited at core
    /// temperature and its heat is exactly 393.7 kJ per mol of C.
    #[test]
    fn oxidation_is_supply_limited_and_releases_the_co2_enthalpy() {
        let mut d = Depressurisation::start(210.0);
        let bed = ThermodynamicTemperature::new::<kelvin>(1200.0);
        let st = d.step(10.0, bed, 5000.0, 0.5);
        assert!(d.oxygen_limited);
        assert!((d.carbon_oxidised_kg - 5.0 * 12.011e-3).abs() < 1e-12);
        assert!((st.oxidation_heat_w - 0.5 * 393.7e3).abs() < 1e-6);
    }
}
