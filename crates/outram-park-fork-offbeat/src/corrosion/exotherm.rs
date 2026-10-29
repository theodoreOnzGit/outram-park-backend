// SPDX-License-Identifier: GPL-3.0
//
// Not a port. OFFBEAT has no counterpart to this module: see "Why upstream
// does not have this" below. The stoichiometry and the Pilling-Bedworth ratio
// it builds on ARE upstream's, via `super::hydrogen` and
// `super::PILLING_BEDWORTH_ZIRCONIUM`.

//! Heat released by the zirconium-steam reaction \[J/m^2 of clad surface\].
//!
//! ```text
//! Zr + 2 H2O  ->  ZrO2 + 2 H2        dH < 0
//! ```
//!
//! The same reaction [`super::kinetics`] grows oxide with and
//! [`super::hydrogen`] liberates hydrogen from. This module answers the third
//! question those two do not: **how much energy does it release?**
//!
//! # Why upstream does not have this, and why this port needs it
//!
//! OFFBEAT is a fuel-performance code. At normal-operation cladding
//! temperatures — 560 to 620 K — the oxide grows microns per *year* and the
//! exotherm is utterly negligible beside the fission power a few millimetres
//! away. Leaving it out is the right call for the problem upstream solves, and
//! [`super`]'s own module documentation says as much when it lists the three
//! reasons the oxide matters: insulation, metal loss, hydrogen pickup. Energy
//! is not among them.
//!
//! Above roughly 1200 K that judgement inverts. The reaction becomes the
//! dominant heat source in the core — larger than decay heat — and because it
//! is faster the hotter it gets, and it heats what drives it, it is
//! **self-accelerating**. A heat-up calculation that omits it does not merely
//! run a little cool: it cannot produce the runaway at all, and it produces no
//! hydrogen.
//!
//! **Measured** with this module, 2026-10-27, by
//! `oxidation_overtakes_decay_heat_between_1300_and_1500k`: Cathcart-Pawel for
//! one second from a 10 um oxide, against the decay-heat flux of a 17x17 PWR
//! rod at 1 % of a 20 kW/m rating (`200 W/m / (2*pi*4.75 mm) = 6.70e3 W/m^2`).
//!
//! | Clad temperature | Oxidation \[W/m^2\] | Ratio to decay heat |
//! |---|---|---|
//! | 600 K | 2.57e-5 | 3.8e-9 |
//! | 900 K | 1.94e0 | 2.9e-4 |
//! | 1200 K | 5.32e2 | 0.079 |
//! | 1300 K | 1.94e3 | 0.289 |
//! | 1500 K | 1.51e4 | 2.25 |
//! | 1700 K | 6.75e4 | 10.1 |
//!
//! **The crossover is between 1300 and 1500 K.** So this term is worth nothing
//! at operating temperature and worth more than decay heat in an accident,
//! which is exactly why a fuel-performance code can omit it and an accident
//! calculation cannot.
//!
//! # Inherited validity: this module is only as good as the kinetics
//!
//! Every number here is `oxide growth x chemistry`. The chemistry is a mass
//! balance and cannot be wrong; **the growth can**, and this module applies no
//! validity check of its own. In particular
//! [`super::kinetics::OxidationKinetics::CathcartPawel`] carries a documented
//! **upstream defect in the 1800-1900 K window**, reproduced deliberately,
//! where the rate constant comes out about nine orders of magnitude too large.
//! Feed that growth in here and you get a heat flux to match: at 1800 K this
//! module returns `1.96e12 W/m^2`, which is nonsense, because its input was.
//!
//! The table above stops at 1700 K for that reason. A caller near or above
//! 1800 K must obtain growth from
//! [`growth_checked`](super::kinetics::OxidationKinetics::growth_checked),
//! which refuses the window, rather than from the unchecked path.
//!
//! # What this module is NOT
//!
//! It is the reaction *enthalpy*, nothing more. It does **not** make this crate
//! a severe-accident code: there is still no phase change, no melting, no
//! candling, no relocation of molten material, and every material correlation
//! in [`crate::materials`] is still bounded at the melting point. See the crate
//! documentation. This module lets a caller's own heat-conduction solve see the
//! oxidation energy; it does not model anything that happens after the metal
//! melts.
//!
//! It also assumes **steam is available**. The reaction is limited by whichever
//! is scarcer, zirconium or steam, and nothing here knows about steam supply:
//! in a steam-starved node the real release is smaller than this returns. A
//! caller that cannot guarantee steam must apply its own limit.
//!
//! # Provenance of the enthalpy
//!
//! Derived from standard enthalpies of formation at 298.15 K, not fitted and
//! not taken from any severe-accident code:
//!
//! ```text
//! dH = dHf(ZrO2, monoclinic) - 2 * dHf(H2O, gas)
//!    = -1100.6 - 2*(-241.826)  kJ/mol
//!    = -616.95 kJ/mol Zr
//! ```
//!
//! giving **6.763 MJ per kg of zirconium consumed**.
//!
//! Published values used by accident codes sit a little below this — Baker-Just
//! is commonly quoted at 6.45 MJ/kg, and values from 5.8 to 6.5 MJ/kg appear in
//! the literature depending on the water phase assumed (liquid or vapour), the
//! ZrO2 polymorph, and the temperature the enthalpy is evaluated at. The spread
//! is about 5 %. **This module reports the derived value and does not tune it
//! toward any of them**; [`REACTION_ENTHALPY_PER_MOL_ZR`] is public so a caller
//! who needs a specific published basis can scale, and say that they did.
//!
//! The derivation is temperature-independent, which is an approximation: the
//! reaction enthalpy drifts by a few per cent over 300-2000 K as the heat
//! capacities of reactants and products differ. That drift is well inside the
//! 5 % spread above and is not modelled.

use super::hydrogen::{surface_to_volume, ZIRCONIUM_ATOMIC_MASS};
use super::PILLING_BEDWORTH_ZIRCONIUM;

/// Standard enthalpy of formation of monoclinic ZrO2 at 298.15 K \[J/mol\].
pub const ZIRCONIA_FORMATION_ENTHALPY: f64 = -1_100_600.0;

/// Standard enthalpy of formation of gaseous water at 298.15 K \[J/mol\].
pub const WATER_VAPOUR_FORMATION_ENTHALPY: f64 = -241_826.0;

/// Moles of water consumed per mole of zirconium \[-\], from
/// `Zr + 2 H2O -> ZrO2 + 2 H2`.
///
/// Two. [`super::hydrogen::HYDROGEN_ATOMS_PER_ZIRCONIUM`] is the same
/// stoichiometry counted in atoms rather than molecules (four atoms, two
/// molecules); the two constants must stay consistent and a test asserts it.
pub const WATER_MOLES_PER_ZIRCONIUM: f64 = 2.0;

/// Enthalpy of `Zr + 2 H2O -> ZrO2 + 2 H2` at 298.15 K \[J/mol of Zr\].
///
/// **Negative**, because the reaction is exothermic. Roughly `-6.17e5` J/mol.
/// Derived from the two formation enthalpies above; see the
/// [module documentation](self) for why it is not tuned to a published
/// accident-code value.
pub const REACTION_ENTHALPY_PER_MOL_ZR: f64 =
    ZIRCONIA_FORMATION_ENTHALPY - WATER_MOLES_PER_ZIRCONIUM * WATER_VAPOUR_FORMATION_ENTHALPY;

/// Energy released per kilogram of zirconium consumed \[J/kg\], positive.
///
/// `|dH| / M_Zr`, about `6.76e6` J/kg. The sign is flipped from
/// [`REACTION_ENTHALPY_PER_MOL_ZR`] so that callers adding a heat source do not
/// have to remember to negate it.
pub const ENERGY_PER_KG_ZIRCONIUM: f64 =
    -REACTION_ENTHALPY_PER_MOL_ZR / (ZIRCONIUM_ATOMIC_MASS * 1.0e-3);

/// Mass of zirconium consumed \[kg/m^2 of outer surface\] in growing
/// `oxide_growth` \[m\] of oxide, on cladding of density `zircaloy_density`
/// \[kg/m^3\].
///
/// The metal consumed is thinner than the oxide produced, by the
/// Pilling-Bedworth ratio — [`super::CorrosionModel::metal_loss`] is the same
/// division, and this function exists so the two cannot drift.
///
/// # Choosing a density
///
/// Zircaloy density falls with temperature and jumps across the alpha-beta
/// transition, so at accident temperatures a constant is a poor choice. Use
/// [`crate::materials::properties::DensityModel::IAEAZy`], which carries the
/// transition. A caller that genuinely wants a constant should say which one
/// and why.
///
/// Returns `0.0` for non-positive growth or non-positive density.
///
/// ```
/// use outram_park_fork_offbeat::corrosion::exotherm::zirconium_consumed;
///
/// // 100 um of oxide, Zircaloy near 6500 kg/m^3.
/// let m = zirconium_consumed(1.0e-4, 6500.0);
/// assert!((m - 0.4167).abs() < 1.0e-3);
/// ```
#[must_use]
pub fn zirconium_consumed(oxide_growth: f64, zircaloy_density: f64) -> f64 {
    if !(oxide_growth > 0.0) || !(zircaloy_density > 0.0) {
        return 0.0;
    }
    zircaloy_density * oxide_growth / PILLING_BEDWORTH_ZIRCONIUM
}

/// Heat released \[J/m^2 of outer surface\] in growing `oxide_growth` \[m\] of
/// oxide, on cladding of density `zircaloy_density` \[kg/m^3\].
///
/// A **surface** quantity, because the reaction happens at the metal/oxide
/// interface rather than through the wall. That is the opposite convention to
/// [`super::hydrogen::hydrogen_liberated`], which returns a wall-average
/// concentration because hydrogen *diffuses into* the metal while this energy
/// does not: it conducts away from the surface it was released on. Use
/// [`wall_volumetric_heat`] if a volumetric source in the metal is what your
/// discretisation wants.
///
/// Positive (a heat source). Returns `0.0` for degenerate input.
///
/// ```
/// use outram_park_fork_offbeat::corrosion::exotherm::heat_released;
///
/// // 100 um of oxide releases about 2.8 MJ per square metre of clad.
/// let q = heat_released(1.0e-4, 6500.0);
/// assert!((q - 2.818e6).abs() < 1.0e4);
/// // Nothing from nothing.
/// assert_eq!(heat_released(0.0, 6500.0), 0.0);
/// ```
#[must_use]
pub fn heat_released(oxide_growth: f64, zircaloy_density: f64) -> f64 {
    zirconium_consumed(oxide_growth, zircaloy_density) * ENERGY_PER_KG_ZIRCONIUM
}

/// Mean heat flux \[W/m^2\] over a step that grew `oxide_growth` \[m\] of oxide
/// in `time_step` \[s\].
///
/// The **mean** over the step, not the instantaneous value at either end — the
/// same convention as [`super::kinetics::OxidationKinetics::growth_rate`],
/// because it is that growth divided by that step.
///
/// Returns `0.0` for a non-positive step.
///
/// ```
/// use outram_park_fork_offbeat::corrosion::exotherm::heat_flux;
///
/// // 1 um in 10 s.
/// let q = heat_flux(1.0e-6, 6500.0, 10.0);
/// assert!((q - 2818.0).abs() < 10.0);
/// ```
#[must_use]
pub fn heat_flux(oxide_growth: f64, zircaloy_density: f64, time_step: f64) -> f64 {
    if !(time_step > 0.0) {
        return 0.0;
    }
    heat_released(oxide_growth, zircaloy_density) / time_step
}

/// Heat released as a volumetric source in the cladding metal \[J/m^3\].
///
/// [`heat_released`] spread over the wall with the same surface-to-volume
/// factor [`super::hydrogen`] uses, for a discretisation that wants a
/// volumetric source rather than a surface flux.
///
/// **This smears a surface release through the wall.** It is the right thing
/// for a thin wall resolved by one cell and the wrong thing for a wall resolved
/// by many, where the outermost cell should carry it. Prefer [`heat_released`]
/// as a boundary flux where the discretisation allows.
///
/// Returns `0.0` for degenerate input.
#[must_use]
pub fn wall_volumetric_heat(
    oxide_growth: f64,
    inner_radius: f64,
    outer_radius: f64,
    zircaloy_density: f64,
) -> f64 {
    heat_released(oxide_growth, zircaloy_density) * surface_to_volume(inner_radius, outer_radius)
}

#[cfg(test)]
mod tests {
    use super::super::hydrogen::{hydrogen_liberated, HYDROGEN_ATOMS_PER_ZIRCONIUM};
    use super::super::kinetics::OxidationKinetics;
    use super::*;

    /// The stoichiometry here and in [`super::super::hydrogen`] must agree:
    /// four hydrogen *atoms* is two hydrogen *molecules*, from two water
    /// molecules. A mismatch would mean one of the two modules has the
    /// reaction wrong.
    #[test]
    fn stoichiometry_agrees_with_the_hydrogen_module() {
        assert!((HYDROGEN_ATOMS_PER_ZIRCONIUM - 2.0 * WATER_MOLES_PER_ZIRCONIUM).abs() < 1e-12);
    }

    /// **Methodology.** Derive the reaction enthalpy from standard formation
    /// enthalpies and check it against the band published for this reaction.
    ///
    /// **Result (2026-10-27).** `-616.95 kJ/mol Zr`, i.e. **6.763 MJ/kg Zr**.
    /// Literature and accident-code values span roughly 5.8-6.8 MJ/kg
    /// depending on water phase, ZrO2 polymorph and evaluation temperature;
    /// Baker-Just is commonly quoted at 6.45 MJ/kg, which this exceeds by
    /// 4.9 %. **Not tuned toward it** — the derived value is reported as is.
    #[test]
    fn reaction_enthalpy_is_derived_and_lands_in_the_published_band() {
        let per_mol_kj = -REACTION_ENTHALPY_PER_MOL_ZR / 1000.0;
        assert!(
            (per_mol_kj - 616.95).abs() < 0.1,
            "derived {per_mol_kj} kJ/mol"
        );
        let mj_per_kg = ENERGY_PER_KG_ZIRCONIUM / 1.0e6;
        assert!(
            (5.8..=6.8).contains(&mj_per_kg),
            "6.763 MJ/kg expected, inside the 5.8-6.8 published band; got {mj_per_kg}"
        );
        // Exothermic, and the sign flip is real.
        assert!(REACTION_ENTHALPY_PER_MOL_ZR < 0.0);
        assert!(ENERGY_PER_KG_ZIRCONIUM > 0.0);
    }

    /// Energy and hydrogen come from the same reaction, so their ratio is
    /// fixed by stoichiometry alone and must not depend on how much oxide
    /// grew. A drift here means one of the two is not counting the same moles.
    #[test]
    fn energy_per_hydrogen_is_a_constant_of_the_reaction() {
        let (r_i, r_o) = (4.18e-3, 4.75e-3);
        let mut ratios = vec![];
        for growth in [1.0e-7, 1.0e-6, 1.0e-5, 1.0e-4] {
            let h = hydrogen_liberated(growth, r_i, r_o);
            let q = wall_volumetric_heat(growth, r_i, r_o, 6500.0);
            ratios.push(q / h);
        }
        for r in &ratios {
            assert!(
                (r - ratios[0]).abs() / ratios[0] < 1e-12,
                "ratio drifted across growth size: {ratios:?}"
            );
        }
    }

    /// **Methodology.** Drive Cathcart-Pawel for one second from a 10 um oxide
    /// at four temperatures and compare the resulting oxidation heat flux with
    /// the decay-heat flux of a 17x17 PWR rod at 1 % of a 20 kW/m linear
    /// rating, i.e. `200 W/m / (2*pi*4.75 mm) = 6.70e3 W/m^2`.
    ///
    /// **Result (2026-10-27)** — the table in the [module documentation](self),
    /// pinned below. Oxidation is 3.8e-9 of decay heat at 600 K, 0.289 at
    /// 1300 K and 2.25 at 1500 K, so **the crossover lies between 1300 and
    /// 1500 K**.
    ///
    /// **Interpretation.** This is the quantitative statement of why a
    /// fuel-performance code may omit this term and an accident calculation may
    /// not. It is a comparison of magnitudes, not a validation of either
    /// number: the decay-heat figure is a nominal 1 % rating, and the oxidation
    /// figure inherits whatever Cathcart-Pawel's accuracy is.
    ///
    /// **Stops at 1700 K deliberately.** 1800 K sits in the window where the
    /// kinetics carry a known upstream defect — see
    /// `the_1800_1900k_window_poisons_this_module_too`. An earlier draft of
    /// this test asserted a large ratio at 1800 K and passed *because of* that
    /// defect, which is a test passing for a bad reason.
    #[test]
    fn oxidation_overtakes_decay_heat_between_1300_and_1500k() {
        const DECAY_HEAT_FLUX: f64 = 200.0 / (2.0 * std::f64::consts::PI * 4.75e-3);
        let cp = OxidationKinetics::CathcartPawel;
        let flux_at = |t: f64| heat_flux(cp.growth(1.0e-5, t, 0.0, 1.0), 6500.0, 1.0);

        // Pin the published table to 1 %.
        for (t, expected) in [
            (600.0, 2.5747e-5),
            (900.0, 1.9402e0),
            (1200.0, 5.3211e2),
            (1300.0, 1.9394e3),
            (1500.0, 1.5069e4),
            (1700.0, 6.7452e4),
        ] {
            let q = flux_at(t);
            assert!(
                (q - expected).abs() / expected < 0.01,
                "{t} K: got {q:.5e} W/m^2, module doc says {expected:.5e}"
            );
        }

        // The claim the module exists to support.
        assert!(flux_at(1300.0) < DECAY_HEAT_FLUX);
        assert!(flux_at(1500.0) > DECAY_HEAT_FLUX);
        // Monotone in temperature, as an Arrhenius rate must be.
        let qs: Vec<f64> = [600.0, 900.0, 1200.0, 1300.0, 1500.0, 1700.0]
            .iter()
            .map(|t| flux_at(*t))
            .collect();
        assert!(qs.windows(2).all(|w| w[0] < w[1]), "not monotone: {qs:?}");
    }

    /// **The defect this module inherits, pinned so nobody is surprised by it.**
    ///
    /// `CathcartPawel`'s 1800-1900 K branch is arithmetically broken upstream
    /// and reproduced deliberately (see its own documentation). This module
    /// multiplies that growth by a mass balance, so it inherits the error
    /// intact and amplifies nothing.
    ///
    /// **Result (2026-10-27).** At 1800 K, one second from a 10 um oxide gives
    /// 69.6 **metres** of oxide and a heat flux of `1.96e12 W/m^2` — about
    /// 2.9e8 times decay heat. Physically impossible, and returned anyway,
    /// because the unchecked kinetics path is documented to extrapolate rather
    /// than clamp.
    ///
    /// The guard is upstream of here: use `growth_checked`, which refuses the
    /// window. This test exists so that a future fix to the kinetics shows up
    /// as a failure here rather than silently changing an accident result.
    #[test]
    fn the_1800_1900k_window_poisons_this_module_too() {
        let cp = OxidationKinetics::CathcartPawel;
        let growth = cp.growth(1.0e-5, 1800.0, 0.0, 1.0);
        assert!(growth > 1.0, "expected the defective metres-per-second growth, got {growth}");
        let q = heat_flux(growth, 6500.0, 1.0);
        assert!(q > 1.0e11, "expected the inherited nonsense, got {q:.4e} W/m^2");

        // The checked path is the guard, and it refuses.
        assert!(
            cp.growth_checked(1.0e-5, 1800.0, 0.0, 1.0).is_err(),
            "growth_checked must refuse the 1800-1900 K window"
        );
    }

    /// Degenerate input produces zero, never a negative heat source or a NaN.
    /// A negative one would be a heat *sink* at the clad surface, which this
    /// reaction can never be.
    #[test]
    fn degenerate_input_cannot_produce_a_heat_sink() {
        for (g, rho) in [
            (0.0, 6500.0),
            (-1.0e-6, 6500.0),
            (1.0e-6, 0.0),
            (1.0e-6, -6500.0),
            (f64::NAN, 6500.0),
        ] {
            assert_eq!(heat_released(g, rho), 0.0, "growth {g}, density {rho}");
            assert_eq!(heat_flux(g, rho, 1.0), 0.0);
            assert_eq!(wall_volumetric_heat(g, 4.18e-3, 4.75e-3, rho), 0.0);
        }
        assert_eq!(heat_flux(1.0e-6, 6500.0, 0.0), 0.0);
        assert_eq!(heat_flux(1.0e-6, 6500.0, -1.0), 0.0);
    }

    /// The heat scales linearly with oxide grown and with density, because the
    /// chemistry is a mass balance and nothing else.
    #[test]
    fn heat_is_linear_in_growth_and_density() {
        let a = heat_released(1.0e-6, 6500.0);
        assert!((heat_released(2.0e-6, 6500.0) - 2.0 * a).abs() / a < 1e-12);
        assert!((heat_released(1.0e-6, 13000.0) - 2.0 * a).abs() / a < 1e-12);
    }
}

