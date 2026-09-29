//! **DLOFC + ATWS + air ingress** (source-term stage 4, gh:#402; this is
//! stage 4a): the primary depressurises through a ruptured DN65 fuel-loading
//! tube, the circulator stops, the rods do NOT drop (anticipated transient
//! without scram), and -- once air can reach the core -- the bed graphite
//! oxidises.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`). Not a licensing
//! source term, not a PSA, not validated.
//!
//! ```text
//!  primary gas --DN65 blowdown (6.58 kg/s initial, ~150 kg)--> stack (building not credited, gh:#409)
//!      ^  plate-out lift-off (I, metals: +2.4 x the circulating activity)
//!      ^  dust (Cs 5 %, Sr/Ag 20 %, I 1 % of plate-out; 10 % of it released)
//!      ^  purification system (noble 100 %, I and metals 10 %; isolation assumed to fail)
//!  air --O2 supply (NOT PUBLISHED for HTR-10: zero, gh:#420)--> C + O2 -> CO2 in the bed, -393.7 kJ/mol
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
//! | **O2 supply into the core** | **0 until a source is given** | **NOT PUBLISHED** in the held literature (no HTR-10 cavity volume or air-ingress rate; searched Gao & Shi, Jiang, JAERI-Conf 96-010, TECDOC-1382, -1694, NUREG/CR-6844, Oh & Kim). Tracked as **gh:#420**. Never derived from Gao & Shi's 319.2 kg of corrosion. |
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
/// The O2 supply into the core \[mol/s\]: **not published for HTR-10**
/// (module doc; gh:#420). Zero until a source is given.
pub const PUBLISHED_OXYGEN_SUPPLY_MOL_PER_S: Option<f64> = None;

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
    /// Fraction of the primary gas vented so far.
    pub vented_fraction: f64,
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
    /// this step.
    pub vented_fraction: f64,
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

    /// Advance by `dt` \[s\] with the bed graphite at `graphite_temperature`
    /// and `graphite_mass_kg`, and an O2 supply `oxygen_supply_mol_per_s`
    /// (zero while unpublished).
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
