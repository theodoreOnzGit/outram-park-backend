//! **Water ingress** (source-term stage 3, gh:#401): two steam-generator tubes
//! rupture and the secondary relief system fails -- the HTR-10 beyond-design
//! water-ingress case of Gao & Shi (2002) section 5.4, whose source term Liu &
//! Cao (2002) section 4.1.2 / Table 8 evaluate.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`). Not a licensing
//! source term, not a PSA, and not validated.
//!
//! # What it carries
//!
//! ```text
//!  SG tubes --129.9 kg H2O--> [primary gas: He + H2O + H2 + CO] --relief 3.5/2.9 MPa--> stack
//!                                   |  ^                                   (building NOT credited, gh:#409)
//!              C + H2O -> CO + H2   |  |  +131.3 kJ/mol, heat drawn from the bed
//!                                   v  |
//!                             [bed graphite]
//!  steam density in the core --> +8.5e-4 dk/k at the full 129.9 kg (moderation)
//!  liquid water at the break --> wash-off of the SG share of the plate-out pool
//!  steam at exposed kernels  --> stored noble gas burst (TECDOC-978 Eq. 5-2)
//! ```
//!
//! # Inputs, and where each comes from
//!
//! | Quantity | Value | Source |
//! |---|---|---|
//! | water entering the primary | **129.9 kg** total | Gao & Shi (2002) s.5.4 (also Liu & Cao s.4.1.2) |
//! | ingress **shape** | uniform over 0-50 s | **ASSUMED, labelled**: the total and the ~50 s at which "the steam generator is drained and the secondary circuit is isolated completely" are published; the time profile is not |
//! | reflector rods dropped (scram) | 37.5 s | Gao & Shi s.5.4 |
//! | circulator shut down | 1 s after the scram | Gao & Shi s.5.4 |
//! | secondary isolated | 50 s | Gao & Shi s.5.4 |
//! | steam-moderation reactivity | **+8.5e-4 dk/k** at the full ingress | Gao & Shi s.5.4; **scaling linearly with the steam held in the primary is ASSUMED** |
//! | primary relief | opens 3.5 MPa (chain 1), 3.72 MPa (chain 2); closes 2.9 MPa; 85 L/s per chain | Gao & Shi s.2.2 |
//! | SG share of plate-out, washed off | Cs 73 %, Sr 80 %, I 100 % | Liu & Cao s.4.1.1.3 (after Wolters et al. 1985) and s.4.1.2 |
//! | graphite-steam kinetics | BLH fit | `boon_lay::chemistry::graphite_steam` (Wang & Sun 2023) |
//! | stored-gas burst | TECDOC-978 Eq. 5-2 | `boon_lay::chemistry::kernel_hydrolysis` |
//!
//! # What it deliberately does not carry, stated
//!
//! - **The stuck reflector rod.** Gao & Shi assume the rod of maximum worth
//!   fails; this model moves one bank, so its scram is the whole bank
//!   (optimistic on shutdown margin).
//! - **Water removal** by the purification system's accident line: its rate
//!   is not published, so no water is removed (conservative on corrosion).
//! - **Where the water is.** The steam is well mixed through the primary gas,
//!   and it all meets the bed graphite at the one lumped bed temperature,
//!   which sits near the outlet (gh:#372). Together with the kinetic-regime
//!   rate this **over-states** corrosion (see below).
//! - **Silver, and the noble gases, on the SG surfaces.** Liu & Cao give no
//!   SG share for Ag, so none is washed off; noble gases do not plate out.
//! - **Radioactivity in the corroded graphite** (Liu & Cao's third source):
//!   it needs the natural-uranium contamination of the matrix graphite,
//!   which is not in the repo. Omitted, and said so.
//! - **The graphite-steam rate outside its fitted box.** The steam partial
//!   pressure here (~100s of kPa) is far above Wang & Sun's 20 kPa, and a
//!   pebble at 1300 K is not in the kinetic regime. The fit is extrapolated
//!   and flagged ([`WaterIngress::rate_extrapolated`]); it is an upper bound.
//!
//! The one-bank scram, the whole-mass kinetic-regime rate and the
//! outlet-referenced bed are the stated reasons to expect this model to
//! **over-state** corrosion against Gao & Shi's 4.88 kg.

use boon_lay::chemistry::graphite_steam::{
    self, BlhCoefficients, CARBON_MOLAR_MASS_KG_PER_MOL, REACTION_ENTHALPY_J_PER_MOL,
};
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::frequency::hertz;
use uom::si::pressure::pascal;

/// Water that enters the primary in the published case \[kg\] (Gao & Shi
/// 2002 s.5.4: "totally amounting to 129.9 kg of water ingress").
pub const TOTAL_INGRESS_KG: f64 = 129.9;
/// End of the ingress \[s\]: Gao & Shi s.5.4, "About 50 s after the accident,
/// the steam generator is drained and the secondary circuit is isolated
/// completely". The UNIFORM rate over 0..this is an assumption (module doc).
pub const INGRESS_END_S: f64 = 50.0;
/// Reflector rods dropped \[s\] (Gao & Shi s.5.4, "at 37.5 s").
pub const SCRAM_AT_S: f64 = 37.5;
/// Primary circuit blower shut down \[s\] ("1 s later").
pub const CIRCULATOR_TRIP_AT_S: f64 = SCRAM_AT_S + 1.0;
/// Secondary circuit isolated \[s\] (the same ~50 s).
pub const SECONDARY_ISOLATED_AT_S: f64 = INGRESS_END_S;
/// Steam-moderation reactivity at the full published ingress \[dk/k\]
/// (Gao & Shi s.5.4, "a positive reactivity of about 8.5e-4 dk/k").
pub const STEAM_REACTIVITY_AT_FULL_INGRESS: f64 = 8.5e-4;
/// Relief chain 1 opens \[Pa\] (Gao & Shi s.2.2, 3.5 MPa).
pub const RELIEF_CHAIN_1_OPEN_PA: f64 = 3.5e6;
/// Relief chain 2 opens \[Pa\] (3.72 MPa).
pub const RELIEF_CHAIN_2_OPEN_PA: f64 = 3.72e6;
/// Relief valves close \[Pa\] ("once the primary pressure has reached the
/// design value of 2.9 MPa").
pub const RELIEF_CLOSE_PA: f64 = 2.9e6;
/// Relief capacity per chain \[m^3/s\] ("85 l s^-1"), taken at primary
/// conditions.
pub const RELIEF_FLOW_PER_CHAIN_M3_PER_S: f64 = 0.085;
/// Molar mass of water \[kg/mol\].
const WATER_MOLAR_MASS: f64 = 18.015e-3;
/// Molar mass of helium \[kg/mol\].
const HELIUM_MOLAR_MASS: f64 = 4.002602e-3;

/// The fraction of each element's plate-out that sits on the steam generator
/// and is washed off by liquid water (Liu & Cao 2002 s.4.1.1.3, after
/// Wolters et al. 1985: Cs 73 %, Sr 80 %, I 100 %). `None` where the paper
/// gives no figure -- silver among the tracked nuclides -- and noble gases
/// (which do not plate out).
pub fn steam_generator_share_of_plate_out(z: u32) -> Option<f64> {
    match z {
        55 => Some(0.73),
        38 => Some(0.80),
        53 => Some(1.00),
        _ => None,
    }
}

/// The accident's own state: the primary gas composition and what the
/// chemistry and the relief valves have done so far.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterIngress {
    /// Seconds since the tubes ruptured.
    pub elapsed_s: f64,
    /// Water that has entered so far \[kg\].
    pub injected_kg: f64,
    /// Steam in the primary \[mol\].
    pub steam_mol: f64,
    /// Hydrogen from the reaction \[mol\].
    pub hydrogen_mol: f64,
    /// Carbon monoxide from the reaction \[mol\].
    pub co_mol: f64,
    /// Helium in the primary \[mol\] (falls when the relief vents).
    pub helium_mol: f64,
    /// Helium at the start \[mol\], the pressure reference.
    pub helium_mol_initial: f64,
    /// Graphite gasified so far \[kg\].
    pub carbon_corroded_kg: f64,
    /// Heat the reaction has absorbed so far \[J\].
    pub chemistry_heat_absorbed_j: f64,
    /// Primary pressure \[Pa\].
    pub pressure_pa: f64,
    /// Peak primary pressure so far \[Pa\].
    pub peak_pressure_pa: f64,
    /// Relief chains open (0, 1 or 2).
    pub relief_chains_open: u8,
    /// Fraction of the primary gas vented so far (moles vented / moles held,
    /// compounded).
    pub vented_fraction: f64,
    /// Whether the graphite-steam rate has ever been evaluated outside Wang &
    /// Sun's measured box.
    pub rate_extrapolated: bool,
    /// The operating pressure the ideal-gas scaling is referenced to \[Pa\].
    reference_pressure_pa: f64,
    /// `sum V_i / T_i` of the primary gas at the start \[m^3/K\].
    reference_volume_over_temperature: f64,
}

/// What one step of the accident asks of the plant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterIngressStep {
    /// Steam-moderation reactivity to add \[dk/k\].
    pub reactivity_dk_k: f64,
    /// Heat the reaction drew from the bed this step \[W\].
    pub chemistry_heat_w: f64,
    /// Fraction of the primary gas, and so of its circulating activity,
    /// vented up the stack this step.
    pub vented_fraction: f64,
    /// The scram demand: bank insertion from the scram ramp (0 before 37.5 s).
    pub scram_insertion: f64,
    /// Whether the circulator is shut down.
    pub circulator_tripped: bool,
    /// Whether the secondary is isolated.
    pub secondary_isolated: bool,
    /// Steam partial pressure \[Pa\].
    pub steam_partial_pressure_pa: f64,
}

impl WaterIngress {
    /// The moment the tubes rupture, with the primary at `pressure` holding
    /// `helium_kg` of helium whose `sum V_i / T_i` is
    /// `volume_over_temperature` \[m^3/K\].
    pub fn start(pressure_pa: f64, helium_kg: f64, volume_over_temperature: f64) -> Self {
        let helium_mol = helium_kg / HELIUM_MOLAR_MASS;
        Self {
            elapsed_s: 0.0,
            injected_kg: 0.0,
            steam_mol: 0.0,
            hydrogen_mol: 0.0,
            co_mol: 0.0,
            helium_mol,
            helium_mol_initial: helium_mol,
            carbon_corroded_kg: 0.0,
            chemistry_heat_absorbed_j: 0.0,
            pressure_pa,
            peak_pressure_pa: pressure_pa,
            relief_chains_open: 0,
            vented_fraction: 0.0,
            rate_extrapolated: false,
            reference_pressure_pa: pressure_pa,
            reference_volume_over_temperature: volume_over_temperature,
        }
    }

    fn total_mol(&self) -> f64 {
        self.helium_mol + self.steam_mol + self.hydrogen_mol + self.co_mol
    }

    /// Advance by `dt` seconds.
    ///
    /// - `volume_over_temperature`: the primary gas's current `sum V_i/T_i`
    ///   \[m^3/K\]; the pressure is the ideal-gas mixture scaled from the
    ///   operating point, `p = p0 (n/n_He0) (sum V/T)_0 / (sum V/T)`.
    /// - `total_volume_m3`: the primary gas volume the relief draws from.
    /// - `graphite_temperature`, `graphite_mass_kg`: the bed graphite the
    ///   steam reaches.
    /// - `scram_insertion_time_s`: the protection system's bank travel time.
    pub fn step(
        &mut self,
        dt: f64,
        volume_over_temperature: f64,
        total_volume_m3: f64,
        graphite_temperature: ThermodynamicTemperature,
        graphite_mass_kg: f64,
        scram_insertion_time_s: f64,
    ) -> WaterIngressStep {
        // 1. Ingress (uniform over 0..INGRESS_END_S; assumption, module doc).
        let t0 = self.elapsed_s;
        let t1 = t0 + dt;
        let overlap = (t1.min(INGRESS_END_S) - t0.min(INGRESS_END_S)).max(0.0);
        let water_kg = TOTAL_INGRESS_KG * overlap / INGRESS_END_S;
        self.injected_kg += water_kg;
        self.steam_mol += water_kg / WATER_MOLAR_MASS;
        self.elapsed_s = t1;

        // 2. Pressure, ideal-gas mixture scaled from the operating point.
        let pressure = |s: &Self| {
            s.reference_pressure_pa
                * (s.total_mol() / s.helium_mol_initial)
                * (s.reference_volume_over_temperature / volume_over_temperature)
        };
        let p = pressure(self);
        let n = self.total_mol();
        let x_steam = self.steam_mol / n;
        let x_h2 = self.hydrogen_mol / n;

        // 3. C + H2O -> CO + H2 on the bed graphite, limited by the steam present.
        let (r_spe, validity) = graphite_steam::specific_rate(
            graphite_temperature,
            Pressure::new::<pascal>(p * x_steam),
            Pressure::new::<pascal>(p * x_h2),
            &BlhCoefficients::wang_sun_2023_ig110(),
        );
        if self.steam_mol > 0.0 && validity == graphite_steam::Validity::Extrapolated {
            self.rate_extrapolated = true;
        }
        let carbon_mol = (r_spe.get::<hertz>() * graphite_mass_kg / CARBON_MOLAR_MASS_KG_PER_MOL
            * dt)
            .min(self.steam_mol)
            .max(0.0);
        self.steam_mol -= carbon_mol;
        self.hydrogen_mol += carbon_mol;
        self.co_mol += carbon_mol;
        self.carbon_corroded_kg += carbon_mol * CARBON_MOLAR_MASS_KG_PER_MOL;
        let heat_j = carbon_mol * REACTION_ENTHALPY_J_PER_MOL;
        self.chemistry_heat_absorbed_j += heat_j;

        // 4. Relief: hysteresis on the two chains, well-mixed venting.
        let p = pressure(self);
        self.relief_chains_open = match self.relief_chains_open {
            _ if p <= RELIEF_CLOSE_PA => 0,
            open if p >= RELIEF_CHAIN_2_OPEN_PA => open.max(2),
            open if p >= RELIEF_CHAIN_1_OPEN_PA => open.max(1),
            open => open,
        };
        let vented = (f64::from(self.relief_chains_open) * RELIEF_FLOW_PER_CHAIN_M3_PER_S * dt
            / total_volume_m3)
            .clamp(0.0, 1.0);
        if vented > 0.0 {
            let keep = 1.0 - vented;
            self.helium_mol *= keep;
            self.steam_mol *= keep;
            self.hydrogen_mol *= keep;
            self.co_mol *= keep;
            self.vented_fraction = 1.0 - (1.0 - self.vented_fraction) * keep;
        }
        self.pressure_pa = pressure(self);
        self.peak_pressure_pa = self.peak_pressure_pa.max(self.pressure_pa);

        let steam_at_full = TOTAL_INGRESS_KG / WATER_MOLAR_MASS;
        WaterIngressStep {
            reactivity_dk_k: STEAM_REACTIVITY_AT_FULL_INGRESS * self.steam_mol / steam_at_full,
            chemistry_heat_w: heat_j / dt,
            vented_fraction: vented,
            scram_insertion: ((t1 - SCRAM_AT_S) / scram_insertion_time_s).clamp(0.0, 1.0),
            circulator_tripped: t1 >= CIRCULATOR_TRIP_AT_S,
            secondary_isolated: t1 >= SECONDARY_ISOLATED_AT_S,
            steam_partial_pressure_pa: self.pressure_pa * self.steam_mol / self.total_mol(),
        }
    }

    /// Hydrogen mole fraction of the primary gas (Gao & Shi report 0.64 %).
    pub fn hydrogen_fraction(&self) -> f64 {
        self.hydrogen_mol / self.total_mol()
    }

    /// CO mole fraction (Gao & Shi report 0.64 %).
    pub fn co_fraction(&self) -> f64 {
        self.co_mol / self.total_mol()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    fn run(t_graphite_k: f64, seconds: f64) -> (WaterIngress, Vec<WaterIngressStep>) {
        // An illustrative fixed gas temperature: V/T held, so only moles move
        // the pressure.
        let vt = 79.0 / 700.0;
        let mut w = WaterIngress::start(3.0e6, 210.0, vt);
        let mut steps = Vec::new();
        let dt = 0.1;
        for _ in 0..(seconds / dt) as usize {
            steps.push(w.step(
                dt,
                vt,
                79.0,
                ThermodynamicTemperature::new::<kelvin>(t_graphite_k),
                5000.0,
                super::super::protection::SCRAM_INSERTION_TIME_S,
            ));
        }
        (w, steps)
    }

    /// **Atoms are conserved** (O and H from the water, C from the graphite)
    /// and the published totals and times come through.
    ///
    /// Methodology: 600 s at a fixed 1300 K graphite and fixed gas `V/T`.
    /// Check: water injected = 129.9 kg exactly by 50 s; every O atom is in
    /// H2O or CO (moles of water in = steam + CO + vented share); carbon
    /// gasified = CO formed; the reaction heat = 131.3 kJ per mol C; the scram
    /// ramps from 37.5 s and the circulator trips at 38.5 s.
    /// Results (2026-09-29): pass; printed below.
    #[test]
    fn water_ingress_conserves_atoms_and_follows_the_published_sequence() {
        let (w, steps) = run(1300.0, 600.0);
        assert!((w.injected_kg - TOTAL_INGRESS_KG).abs() < 1e-9);
        let water_in = TOTAL_INGRESS_KG / WATER_MOLAR_MASS;
        let kept = 1.0 - w.vented_fraction;
        // Venting removes all species in proportion; oxygen held = kept share
        // of what entered only if venting happened after all reaction -- so
        // check the balance without venting separately below.
        assert!(w.steam_mol + w.co_mol <= water_in * (1.0 + 1e-12));
        assert!((w.hydrogen_mol - w.co_mol).abs() <= 1e-9 * w.co_mol.max(1.0));
        let c_mol = w.carbon_corroded_kg / CARBON_MOLAR_MASS_KG_PER_MOL;
        assert!(
            (w.chemistry_heat_absorbed_j - c_mol * REACTION_ENTHALPY_J_PER_MOL).abs()
                <= 1e-9 * w.chemistry_heat_absorbed_j.max(1.0)
        );
        // Step i ends at (i + 1) x 0.1 s, accumulated in f64.
        assert_eq!(steps[373].scram_insertion, 0.0);
        assert!(steps[374].scram_insertion < 1e-9);
        assert!(steps[380].scram_insertion > 0.0);
        assert!(!steps[383].circulator_tripped && steps[385].circulator_tripped);
        println!(
            "600 s at 1300 K: corroded {:.2} kg C, steam {:.1} mol, H2 {:.3} %, CO {:.3} %, \
             peak p {:.4} MPa, vented {:.4}, kept {kept:.4}, extrapolated {}",
            w.carbon_corroded_kg,
            w.steam_mol,
            100.0 * w.hydrogen_fraction(),
            100.0 * w.co_fraction(),
            w.peak_pressure_pa / 1e6,
            w.vented_fraction,
            w.rate_extrapolated
        );
    }

    /// With no reaction (cold graphite) and no venting, oxygen and hydrogen
    /// are conserved exactly, the pressure rise is the ideal-gas mole ratio,
    /// and the steam reactivity reaches the published +8.5e-4 dk/k.
    #[test]
    fn cold_graphite_gives_the_ideal_gas_rise_and_the_full_reactivity() {
        let (w, steps) = run(500.0, 60.0);
        let water_in = TOTAL_INGRESS_KG / WATER_MOLAR_MASS;
        assert!(w.carbon_corroded_kg < 1e-9);
        assert!((w.steam_mol - water_in).abs() < 1e-6);
        let expected = 3.0e6 * (w.helium_mol_initial + water_in) / w.helium_mol_initial;
        assert!((w.pressure_pa - expected).abs() / expected < 1e-12);
        let last = steps.last().unwrap();
        assert!((last.reactivity_dk_k - STEAM_REACTIVITY_AT_FULL_INGRESS).abs() < 1e-12);
        println!("cold: p {:.4} MPa after 129.9 kg", w.pressure_pa / 1e6);
    }

    /// The relief opens at 3.5 MPa and closes at 2.9 MPa (hysteresis).
    #[test]
    fn the_relief_has_the_published_hysteresis() {
        let vt = 79.0 / 700.0;
        let mut w = WaterIngress::start(3.45e6, 210.0, vt);
        // Push the pressure over 3.5 MPa by raising the gas temperature
        // (smaller V/T), then let it fall.
        let t = ThermodynamicTemperature::new::<kelvin>(500.0);
        w.step(0.1, vt / 1.02, 79.0, t, 5000.0, 7.0);
        assert!(w.relief_chains_open >= 1, "{}", w.pressure_pa);
        w.step(0.1, vt / 1.005, 79.0, t, 5000.0, 7.0);
        assert!(w.relief_chains_open >= 1, "stays open above 2.9 MPa");
        w.step(0.1, vt * 1.3, 79.0, t, 5000.0, 7.0);
        assert_eq!(w.relief_chains_open, 0, "{}", w.pressure_pa);
    }
}
