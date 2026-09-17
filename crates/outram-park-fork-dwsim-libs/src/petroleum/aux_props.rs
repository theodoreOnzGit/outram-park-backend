//! Auxiliary DWSIM property correlations that the petroleum-characterization
//! path calls out to: the Rackett saturated-liquid density, the
//! corresponding-states critical compressibility and critical volume, and
//! Vetere's heat of vaporisation at the normal boiling point.
//!
//! # Why these live here
//!
//! `GenerateCompounds.vb` and `DistCurves.cs` finish each pseudo-component by
//! calling four helpers from **outside** the `PetroleumCharacterization`
//! directory — `PROPS.Zc1`, `PROPS.Vc`, `PROPS.liq_dens_rackett` and
//! `HYP.DHvb_Vetere`. Without them the pseudo-component record cannot be
//! completed (critical volume, Rackett `Z_RA`, `HVap_A`, and the three
//! Chao-Seader parameters all depend on them). They are ported here, in the
//! petroleum module, rather than added to `crate::thermo` — this port owns
//! only `src/petroleum/`.
//!
//! # Provenance
//!
//! Ported from DWSIM (GPL-3.0), pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! | Rust item | Upstream file and lines |
//! |---|---|
//! | [`critical_compressibility_zc1`] | `PropertyPackages/Models/FluidProperties.vb:711-714` (`Zc1`) |
//! | [`critical_volume`] | `PropertyPackages/Models/FluidProperties.vb:705-709` (`Vc(Tc, Pc, w, Zc)`) |
//! | [`critical_volume_from_acentric_factor`] | `PropertyPackages/Models/FluidProperties.vb:699-703` (`Vc(Tc, Pc, w)`) |
//! | [`liquid_density_rackett`] | `PropertyPackages/Models/FluidProperties.vb:299-348` (`liq_dens_rackett`) |
//! | [`heat_of_vaporization_vetere`] | `PropertyPackages/Models/Hypotheticals.vb:906-916` (`DHvb_Vetere`) |
//!
//! # Units
//!
//! `uom`-typed on the public surface. Internally each correlation runs in the
//! units it was published in — bar for Rackett's and Vetere's pressures, g/mol
//! for molecular weight, cm³/mol for Rackett's molar volume — with the
//! conversions written out inline exactly as upstream.
//!
//! # Excluded DWSIM behavior
//!
//! - `FluidProperties.vb`'s other ~30 correlations (Lucas viscosity, Latini
//!   conductivity, Ely-Hanley, surface tension, …) are **not** ported here —
//!   `crate::thermo::transport` already carries that tier. Only the four
//!   helpers the petroleum path actually reaches are included.
//! - `liq_dens_rackett`'s `Double.IsNaN(Pvp)` guard (`:302`) becomes the
//!   `Option` API below.

use uom::si::f64::{
    MassDensity, MolarEnergy, MolarMass, MolarVolume, Pressure, Ratio, ThermodynamicTemperature,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::molar_energy::joule_per_mole;
use uom::si::molar_mass::gram_per_mole;
use uom::si::molar_volume::cubic_meter_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Universal gas constant used by these correlations, `R = 8.314 J/(mol·K)` —
/// upstream's own rounded value (`FluidProperties.vb:705`,
/// `Hypotheticals.vb:908`), kept for bit-fidelity rather than replaced by the
/// CODATA figure.
const R_DWSIM: f64 = 8.314;

/// Critical compressibility factor by the simple acentric-factor correlation
/// `Zc = 0.291 − 0.08·ω` [-].
///
/// Ported from `FluidProperties.vb:711-714` (`Zc1`). DWSIM uses the same
/// expression for the **Rackett parameter** `Z_RA` of a pseudo-component
/// (`GenerateCompounds.vb:334`, `DistCurves.cs:755`).
///
/// **Valid range:** `ω` from 0 to ~1.2; `Zc` becomes negative above
/// `ω = 3.6375`, which upstream guards against by clamping `Z_RA` to 0.2
/// (`GenerateCompounds.vb:335`).
#[must_use]
pub fn critical_compressibility_zc1(acentric_factor: Ratio) -> f64 {
    0.291 - 0.08 * acentric_factor.get::<ratio>()
}

/// Critical molar volume from a **given** critical compressibility:
/// `Vc = R·Zc·Tc / Pc` [m³/mol].
///
/// Ported from `FluidProperties.vb:705-709` (`Vc(Tc, Pc, w, Zc)`), which
/// returns m³/**kmol** (`8.314·Zc·Tc/Pc·1000`); this function returns the SI
/// molar basis, m³/mol, so it drops upstream's `×1000`.
#[must_use]
pub fn critical_volume(
    critical_temperature: ThermodynamicTemperature,
    critical_pressure: Pressure,
    critical_compressibility: f64,
) -> MolarVolume {
    MolarVolume::new::<cubic_meter_per_mole>(
        R_DWSIM * critical_compressibility * critical_temperature.get::<kelvin>()
            / critical_pressure.get::<pascal>(),
    )
}

/// Critical molar volume from the acentric factor alone, i.e.
/// [`critical_volume`] evaluated with `Zc` from [`critical_compressibility_zc1`]
/// [m³/mol].
///
/// Ported from `FluidProperties.vb:699-703` (`Vc(Tc, Pc, w)`), which returns
/// `0` when `Pc <= 0`; that guard is reproduced.
#[must_use]
pub fn critical_volume_from_acentric_factor(
    critical_temperature: ThermodynamicTemperature,
    critical_pressure: Pressure,
    acentric_factor: Ratio,
) -> MolarVolume {
    if critical_pressure.get::<pascal>() > 0.0 {
        critical_volume(
            critical_temperature,
            critical_pressure,
            critical_compressibility_zc1(acentric_factor),
        )
    } else {
        MolarVolume::new::<cubic_meter_per_mole>(0.0)
    }
}

/// Saturated-liquid mass density by the **Rackett** corresponding-states
/// equation, optionally with the **modified HBT (Thomson) compressed-liquid**
/// correction.
///
/// ```text
/// V_sat = (R' Tc / Pc) · Z_RA^(1 + (1 − Tr)^(2/7))        [cm³/mol]
/// ρ     = 1e-3 · M / (V_sat · 1e-6)                       [kg/m³]
/// ```
/// with `R' = 83.14 cm³·bar/(mol·K)`, `Pc` in bar, `M` in g/mol. When a
/// pressure **and** a vapour pressure are supplied, Thomson's correction
/// `V = V_sat·(1 − c·ln((β + P)/(β + Pvp)))` is applied first.
///
/// Ported from `FluidProperties.vb:299-348` (`liq_dens_rackett`).
///
/// # Inputs
///
/// - `temperature` — evaluation temperature `T` [K].
/// - `critical_temperature`, `critical_pressure` — `Tc` [K], `Pc` [Pa].
/// - `acentric_factor` — `ω` [-], used only to default `Z_RA`.
/// - `molar_mass` — `M`; the correlation runs in g/mol internally.
/// - `rackett_z` — `Z_RA` [-]. `None` defaults to `0.29056 − 0.08775·ω`
///   (upstream's `ZRa = 0` sentinel, `:318`).
/// - `pressure`, `vapor_pressure` — supply **both** to enable the Thomson
///   compressed-liquid correction; `None` (either one) gives the plain
///   saturated value.
///
/// # Valid range
///
/// Subcritical liquid, `Tr < 0.99`. Upstream substitutes `Tr = 0.5` outright
/// whenever `Tr > 0.99` ("estimation for supercritical gases solved in liquid
/// phase", `:316`); that substitution is reproduced. The Thomson branch also
/// requires `T < Tc`.
#[must_use]
pub fn liquid_density_rackett(
    temperature: ThermodynamicTemperature,
    critical_temperature: ThermodynamicTemperature,
    critical_pressure: Pressure,
    acentric_factor: Ratio,
    molar_mass: MolarMass,
    rackett_z: Option<f64>,
    pressure: Option<Pressure>,
    vapor_pressure: Option<Pressure>,
) -> MassDensity {
    // cm³·bar/(mol·K) — upstream's `R = 83.14` (`:307`).
    const R_CM3_BAR: f64 = 83.14;

    let t = temperature.get::<kelvin>();
    let tc = critical_temperature.get::<kelvin>();
    let pc_bar = critical_pressure.get::<pascal>() / 100_000.0;
    let w = acentric_factor.get::<ratio>();
    let mm_g = molar_mass.get::<gram_per_mole>();

    let mut tr = t / tc;
    if tr > 0.99 {
        tr = 0.5;
    }

    let z_ra = rackett_z
        .filter(|z| *z != 0.0)
        .unwrap_or(0.29056 - 0.08775 * w);
    // Saturated molar volume, cm³/mol.
    let v_sat = R_CM3_BAR * tc / pc_bar * z_ra.powf(1.0 + (1.0 - tr).powf(2.0 / 7.0));

    let v = match (pressure, vapor_pressure) {
        (Some(p), Some(pvp)) if pvp.get::<pascal>() != 0.0 && t < tc => {
            // Modified HBT (Thomson) compressed-liquid correction (`:322-338`).
            let a = -9.070217;
            let b = 62.45326;
            let d = -135.1102;
            let f = 4.79594;
            let g = 0.250047;
            let h = 1.14188;
            let j = 0.0861488;
            let k = 0.0344483;
            let e = (f + g * w + h * w * w).exp();
            let c = j + k * w;
            let beta = pc_bar
                * 100_000.0
                * (-1.0
                    + a * (1.0 - tr).powf(1.0 / 3.0)
                    + b * (1.0 - tr).powf(2.0 / 3.0)
                    + d * (1.0 - tr)
                    + e * (1.0 - tr).powf(4.0 / 3.0));
            v_sat * (1.0 - c * ((beta + p.get::<pascal>()) / (beta + pvp.get::<pascal>())).ln())
        }
        _ => v_sat,
    };

    // cm³/mol -> kg/m³ (`:340`, `:344`).
    MassDensity::new::<kilogram_per_cubic_meter>(0.001 * mm_g / (v * 0.000_001))
}

/// Molar heat of vaporisation at the normal boiling point by **Vetere's**
/// correlation.
///
/// ```text
/// ΔHvb = R·Tc·Tbr·(0.4343·ln Pc − 0.69431 + 0.8954·Tbr)
///        / (0.37691 − 0.37306·Tbr + 0.15075·Pc⁻¹·Tbr⁻²)
/// ```
/// with `Tbr = Tb/Tc` and `Pc` in **bar**. Ported from
/// `Hypotheticals.vb:906-916` (`DHvb_Vetere`), which documents the result as
/// kJ/kmol — numerically identical to **J/mol**, the unit returned here.
///
/// **Valid range:** `Tb < Tc` (a subcritical normal boiling point); Vetere's
/// regression basis is hydrocarbons and light organics with `Tbr` ≈ 0.5-0.75.
#[must_use]
pub fn heat_of_vaporization_vetere(
    critical_temperature: ThermodynamicTemperature,
    critical_pressure: Pressure,
    boiling_point: ThermodynamicTemperature,
) -> MolarEnergy {
    let tc = critical_temperature.get::<kelvin>();
    let pc_bar = critical_pressure.get::<pascal>() / 100_000.0;
    let tbr = boiling_point.get::<kelvin>() / tc;
    MolarEnergy::new::<joule_per_mole>(
        R_DWSIM * tc * tbr * (0.4343 * pc_bar.ln() - 0.69431 + 0.8954 * tbr)
            / (0.37691 - 0.37306 * tbr + 0.15075 * tbr.powi(-2) / pc_bar),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tk(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }
    fn pa(v: f64) -> Pressure {
        Pressure::new::<pascal>(v)
    }
    fn r(v: f64) -> Ratio {
        Ratio::new::<ratio>(v)
    }

    /// **Methodology.** `Zc1` and the two `Vc` overloads must be mutually
    /// consistent, and `Vc` must land near the tabulated critical volume of a
    /// real hydrocarbon. Benchmark: **n-heptane**, `Tc = 540.2 K`,
    /// `Pc = 2.74 MPa`, `ω = 0.350`, tabulated `Vc = 428 cm³/mol` (Poling,
    /// Prausnitz & O'Connell, *The Properties of Gases and Liquids*, 5th ed.,
    /// Appendix A — public literature). Pass criterion: within 25 % of the
    /// tabulated value (this is a crude one-parameter corresponding-states
    /// estimate, not a data lookup).
    ///
    /// **Results (2026-08-11, this port).** `Zc1(0.350) = 0.26300`;
    /// `Vc = 4.3109e-4 m³/mol = 431.09 cm³/mol`, **+0.72 %** versus the
    /// tabulated 428 cm³/mol. The two `Vc` entry points agree exactly. Test
    /// passes.
    #[test]
    fn critical_volume_matches_n_heptane_literature_value() {
        let tc = tk(540.2);
        let pc = pa(2.74e6);
        let w = r(0.350);
        let zc = critical_compressibility_zc1(w);
        assert!((zc - 0.263).abs() < 1.0e-3, "Zc1 = {zc}");

        let vc = critical_volume(tc, pc, zc).get::<cubic_meter_per_mole>();
        let vc2 = critical_volume_from_acentric_factor(tc, pc, w).get::<cubic_meter_per_mole>();
        assert!(
            (vc - vc2).abs() < 1.0e-15,
            "the two Vc entry points disagree"
        );
        let cm3_per_mol = vc * 1.0e6;
        assert!(
            ((cm3_per_mol - 428.0) / 428.0).abs() < 0.25,
            "Vc = {cm3_per_mol} cm³/mol vs literature 428"
        );
    }

    /// **Methodology.** The Rackett equation must reproduce the saturated
    /// liquid density of a well-characterised hydrocarbon. Benchmark:
    /// **n-heptane at 298.15 K**, literature saturated-liquid density
    /// **679.5 kg/m³** (Poling et al., 5th ed.; `Tc = 540.2 K`,
    /// `Pc = 2.74 MPa`, `ω = 0.350`, `M = 100.20 g/mol`). Pass criterion:
    /// within 5 %, the accuracy Rackett is normally quoted at.
    ///
    /// **Results (2026-08-11, this port).** Computed **686.84 kg/m³**,
    /// **+1.08 %** versus the literature value. Test passes.
    #[test]
    fn rackett_reproduces_n_heptane_liquid_density() {
        let rho = liquid_density_rackett(
            tk(298.15),
            tk(540.2),
            pa(2.74e6),
            r(0.350),
            MolarMass::new::<gram_per_mole>(100.20),
            None,
            None,
            None,
        )
        .get::<kilogram_per_cubic_meter>();
        assert!(
            ((rho - 679.5) / 679.5).abs() < 0.05,
            "Rackett density {rho} kg/m³ vs literature 679.5"
        );
    }

    /// **Methodology.** Vetere's correlation must reproduce a known heat of
    /// vaporisation at the normal boiling point. Benchmark: **n-heptane**,
    /// `Tb = 371.6 K`, experimental `ΔHvb = 31.77 kJ/mol` (Poling et al., 5th
    /// ed., Appendix A — public literature). Pass criterion: within 8 %, the
    /// band Vetere's method is quoted at for hydrocarbons.
    ///
    /// **Results (2026-08-11, this port).** Computed **31.838 kJ/mol**,
    /// **+0.21 %** versus the experimental value. Test passes.
    #[test]
    fn vetere_reproduces_n_heptane_heat_of_vaporization() {
        let dh =
            heat_of_vaporization_vetere(tk(540.2), pa(2.74e6), tk(371.6)).get::<joule_per_mole>();
        assert!(
            ((dh - 31_770.0) / 31_770.0).abs() < 0.08,
            "ΔHvb = {dh} J/mol vs literature 31770"
        );
    }
}
