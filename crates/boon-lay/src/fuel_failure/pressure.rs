// SPDX-License-Identifier: GPL-3.0
//
// boon-lay fuel failure — provenance
// ----------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : boon-lay fuel failure is boon-lay's own model: a Rust
//             implementation of the PANAMA-I FORMULAS, coded agentically (by an
//             AI coding agent, then reviewed) from the published equations. It
//             is not the PANAMA code and not a port of the PANAMA Fortran
//             (closed-source, never consulted); call it "boon-lay fuel failure",
//             and keep "PANAMA-I" for the report and its own printed results.

//! **Eq (3)** — internal gas pressure from the ideal gas law (page -484-),
//! with the constants printed alongside it on page -485-.
//!
//! The printed grouping is ambiguous; see [`internal_gas_pressure`] for the
//! dimensional argument that settles it.

use uom::si::f64::{MolarVolume, Pressure, Ratio, ThermodynamicTemperature, Volume};
use uom::si::molar_volume::cubic_meter_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Molar gas constant `R` in J/(mol·K), as printed with Eq (3) on page -485-.
///
/// The report's own value, kept rather than substituting a CODATA figure, so
/// the arithmetic reproduces the source exactly.
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.3143;

/// Fission yield of the stable fission gases `F_f`, printed with Eq (3)
/// (page -485-). Dimensionless, atoms per fission.
pub const STABLE_FISSION_GAS_YIELD: f64 = 0.31;

/// **Eq (3)** — internal gas pressure from the ideal gas law (page -484-).
///
/// ```text
/// p = (F_d·F_f + OPF) · F_b · R · T / [ (V_f/V_k) · V_m ]      [Pa]
/// ```
///
/// - `released_gas_fraction` — `F_d`, the relevant fraction of fission gas
///   released from the kernel (Eq (4), Allelein 1983 — **not** implemented
///   here; supply it).
/// - `stable_gas_yield` — `F_f`, atoms of stable fission gas per fission;
///   [`STABLE_FISSION_GAS_YIELD`] is the report's 0.31.
/// - `oxygen_per_fission` — `OPF`, CO-forming oxygen atoms per fission
///   (Eqs (5a)–(5f) — **not** implemented here; supply it, and see the module
///   docs on why).
/// - `burnup` — `F_b`, heavy-metal burnup in FIMA.
/// - `free_volume` / `kernel_volume` — `V_f` (buffer void) and `V_k`.
/// - `molar_volume` — `V_m`, the molar volume of the kernel compound
///   (Eqs (6a)–(6c) — supply it).
/// - `temperature` — `T`, in kelvin via `uom`.
///
/// ## The printed grouping is ambiguous; this is the dimensionally consistent
/// reading
///
/// As printed, the fraction bar appears to span `(V_f/V_k)·R·T/V_m`, which
/// would give `p ∝ 1/(R·T)` — dimensionally wrong, and it would make pressure
/// *fall* as the particle heats. Only one grouping is consistent, and it is
/// also just `p = nRT/V_f` with `n = (F_d·F_f + OPF)·F_b·V_k/V_m`:
/// `(F_d·F_f+OPF)·F_b` is dimensionless (moles of gas per mole of heavy metal),
/// `R·T` is Pa·m³/mol, `V_f/V_k` is dimensionless and `V_m` is m³/mol, leaving
/// **Pa**. That is what is implemented. [`pressure_is_the_ideal_gas_law`]
/// pins it against `nRT/V` computed independently.
pub fn internal_gas_pressure(
    released_gas_fraction: Ratio,
    stable_gas_yield: Ratio,
    oxygen_per_fission: Ratio,
    burnup: Ratio,
    free_volume: Volume,
    kernel_volume: Volume,
    molar_volume: MolarVolume,
    temperature: ThermodynamicTemperature,
) -> Pressure {
    let gas_per_heavy_metal = released_gas_fraction.get::<ratio>()
        * stable_gas_yield.get::<ratio>()
        + oxygen_per_fission.get::<ratio>();
    let void_ratio = free_volume.value / kernel_volume.value;
    let v_m = molar_volume.get::<cubic_meter_per_mole>();
    let p = gas_per_heavy_metal
        * burnup.get::<ratio>()
        * GAS_CONSTANT_J_PER_MOL_K
        * temperature.get::<kelvin>()
        / (void_ratio * v_m);
    Pressure::new::<pascal>(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::volume::cubic_meter;

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    #[test]
    fn pressure_is_the_ideal_gas_law() {
        let v_k = Volume::new::<cubic_meter>(1.8e-13); // ~350 um radius kernel
        let v_f = Volume::new::<cubic_meter>(9.0e-13); // buffer void
        let v_m = MolarVolume::new::<cubic_meter_per_mole>(2.46e-5); // UO2-ish
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let (f_d, f_f, opf, f_b) = (r(0.5), r(STABLE_FISSION_GAS_YIELD), r(0.4), r(0.10));

        let got = internal_gas_pressure(f_d, f_f, opf, f_b, v_f, v_k, v_m, t).get::<pascal>();

        // Independent route: moles of heavy metal, then moles of gas, then nRT/V.
        let n_hm = v_k.get::<cubic_meter>() / v_m.get::<cubic_meter_per_mole>();
        let n_gas = (0.5 * STABLE_FISSION_GAS_YIELD + 0.4) * 0.10 * n_hm;
        let expected =
            n_gas * GAS_CONSTANT_J_PER_MOL_K * (1600.0 + 273.15) / v_f.get::<cubic_meter>();

        assert!(
            (got - expected).abs() / expected < 1e-9,
            "Eq (3) should be nRT/V_f: got {got} Pa, expected {expected} Pa"
        );
        // Pressure must RISE with temperature -- the printed grouping would
        // have it fall, which is how the ambiguity was caught.
        let hotter = internal_gas_pressure(
            f_d,
            f_f,
            opf,
            f_b,
            v_f,
            v_k,
            v_m,
            ThermodynamicTemperature::new::<degree_celsius>(2000.0),
        );
        assert!(hotter.get::<pascal>() > got, "pressure must rise with T");
    }
}
