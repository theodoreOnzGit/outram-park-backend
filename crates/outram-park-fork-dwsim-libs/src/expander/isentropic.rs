//! Isentropic expansion + Schultz polytropic-efficiency correction, for a
//! turbine/expander.
//!
//! Ported from DWSIM `UnitOperations/Expander.vb`. DWSIM computes the ideal
//! outlet enthalpy `H2s` via a pressure-entropy (isentropic) flash and the
//! actual outlet state via repeated pressure-enthalpy flashes -- this
//! equipment port is deliberately kept decoupled from the crate's flash kernel
//! ([`crate::thermo`]), so the flash-dependent steps are pushed to the caller:
//! the functions below take already-known enthalpies/densities as inputs,
//! and [`solve_polytropic_efficiency`] takes a caller-supplied closure for
//! the one flash-dependent step in DWSIM's iteration (not a trait object --
//! a generic `Fn` parameter, per the workspace's no-`dyn`-dispatch rule).
//!
//! **Sign convention** (chosen for this port, not necessarily matching
//! DWSIM's own internal accounting byte-for-byte): [`generated_power`] and
//! the polytropic solver's duty are **positive when the expander extracts
//! power from the fluid** (the physically intuitive case, `H2 < H1`).

use uom::si::f64::{AvailableEnergy, MassDensity, MassRate, Power, Pressure, Ratio};
use uom::si::power::watt;
use uom::si::ratio::ratio;

/// Power generated (extracted from the fluid) by an adiabatic expansion from
/// `h1` to the isentropic outlet enthalpy `h2s`, at `adiabatic_efficiency`
/// \[0, 1\]: `W_gen = -w (h2s - h1) η`. Positive for a normal expansion
/// (`h2s < h1`).
pub fn generated_power(
    w: MassRate,
    h1: AvailableEnergy,
    h2s: AvailableEnergy,
    adiabatic_efficiency: Ratio,
) -> Power {
    Power::new::<watt>(-w.value * (h2s - h1).value * adiabatic_efficiency.get::<ratio>())
}

/// Actual outlet enthalpy `h2 = h1 - W_gen/w`, given the generated power
/// from [`generated_power`].
pub fn outlet_enthalpy(h1: AvailableEnergy, generated: Power, w: MassRate) -> AvailableEnergy {
    AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(
        h1.get::<uom::si::available_energy::joule_per_kilogram>()
            - generated.get::<watt>() / w.value,
    )
}

/// Isentropic polytropic exponent `n_isentropic = ln(p2/p1) / ln(ρ2i/ρ1)`,
/// from the inlet density `rho1` and the density at the *ideal* isentropic
/// outlet state `rho2_ideal`.
pub fn isentropic_exponent(
    p1: Pressure,
    p2: Pressure,
    rho1: MassDensity,
    rho2_ideal: MassDensity,
) -> Ratio {
    let pressure_ratio_ln = (p2 / p1).get::<ratio>().ln();
    let density_ratio_ln = (rho2_ideal / rho1).get::<ratio>().ln();
    Ratio::new::<ratio>(pressure_ratio_ln / density_ratio_ln)
}

/// Polytropic exponent `n_polytropic = ln(p2/p1) / ln(ρ2/ρ1)`, from the
/// inlet density `rho1` and the density at the *actual* outlet state `rho2`.
pub fn polytropic_exponent(
    p1: Pressure,
    p2: Pressure,
    rho1: MassDensity,
    rho2: MassDensity,
) -> Ratio {
    let pressure_ratio_ln = (p2 / p1).get::<ratio>().ln();
    let density_ratio_ln = (rho2 / rho1).get::<ratio>().ln();
    Ratio::new::<ratio>(pressure_ratio_ln / density_ratio_ln)
}

/// Schultz (ASME PTC-10-style) polytropic work-correction factor:
/// ```text
/// f_ce = [(p2/p1)^((n_p-1)/n_p) - 1] * [(n_p/(n_p-1)) * (n_i-1)/n_i]
///        / [(p2/p1)^((n_i-1)/n_i) - 1]
/// ```
/// Relates polytropic efficiency to adiabatic efficiency:
/// `η_adiabatic = η_polytropic / f_ce`.
pub fn schultz_correction_factor(
    p1: Pressure,
    p2: Pressure,
    n_isentropic: Ratio,
    n_polytropic: Ratio,
) -> Ratio {
    let pr = (p2 / p1).get::<ratio>();
    let n_i = n_isentropic.get::<ratio>();
    let n_p = n_polytropic.get::<ratio>();
    let numerator = (pr.powf((n_p - 1.0) / n_p) - 1.0) * ((n_p / (n_p - 1.0)) * (n_i - 1.0) / n_i);
    let denominator = pr.powf((n_i - 1.0) / n_i) - 1.0;
    Ratio::new::<ratio>(numerator / denominator)
}

/// Result of [`solve_polytropic_efficiency`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolytropicResult {
    /// Converged adiabatic efficiency `η_adiabatic = η_polytropic / f_ce`.
    pub adiabatic_efficiency: Ratio,
    /// Schultz correction factor at convergence.
    pub schultz_factor: Ratio,
    /// Generated power at convergence.
    pub generated: Power,
    /// Actual outlet enthalpy at convergence.
    pub h2: AvailableEnergy,
}

/// Iteratively resolve the adiabatic efficiency that is consistent with a
/// specified polytropic efficiency, mirroring DWSIM's `Expander.vb`
/// `ProcessPathType.Polytropic` loop.
///
/// `evaluate_outlet` is the one flash-dependent step: given a trial
/// `adiabatic_efficiency`, it must return `(h2, rho2)` -- the actual outlet
/// enthalpy and density at `p2` for that trial efficiency (a caller with
/// flash access computes this via a pressure-enthalpy flash at
/// `(p2, outlet_enthalpy(h1, generated_power(w,h1,h2s,eta), w))`, then reads
/// density at that state).
pub fn solve_polytropic_efficiency<F>(
    w: MassRate,
    p1: Pressure,
    p2: Pressure,
    h1: AvailableEnergy,
    h2s: AvailableEnergy,
    rho1: MassDensity,
    rho2_ideal: MassDensity,
    polytropic_efficiency: Ratio,
    mut evaluate_outlet: F,
    tolerance: Ratio,
) -> PolytropicResult
where
    F: FnMut(Ratio) -> (AvailableEnergy, MassDensity),
{
    let n_isentropic = isentropic_exponent(p1, p2, rho1, rho2_ideal);
    let mut adiabatic_efficiency = polytropic_efficiency;

    for _ in 0..100 {
        let generated = generated_power(w, h1, h2s, adiabatic_efficiency);
        let h2_guess = outlet_enthalpy(h1, generated, w);
        let (h2, rho2) = evaluate_outlet(adiabatic_efficiency);
        // h2_guess and h2 should agree (both are the energy-balance outlet
        // enthalpy for this trial efficiency) -- evaluate_outlet is the
        // source of truth since it comes from a real flash.
        let _ = h2_guess;

        let n_polytropic = polytropic_exponent(p1, p2, rho1, rho2);
        let f_ce = schultz_correction_factor(p1, p2, n_isentropic, n_polytropic);
        let next_adiabatic_efficiency =
            Ratio::new::<ratio>(polytropic_efficiency.get::<ratio>() / f_ce.get::<ratio>());

        let delta =
            (next_adiabatic_efficiency.get::<ratio>() - adiabatic_efficiency.get::<ratio>()).abs();
        adiabatic_efficiency = next_adiabatic_efficiency;
        if delta < tolerance.get::<ratio>() {
            let generated = generated_power(w, h1, h2s, adiabatic_efficiency);
            return PolytropicResult {
                adiabatic_efficiency,
                schultz_factor: f_ce,
                generated,
                h2,
            };
        }
    }

    let generated = generated_power(w, h1, h2s, adiabatic_efficiency);
    let (h2, _rho2) = evaluate_outlet(adiabatic_efficiency);
    PolytropicResult {
        adiabatic_efficiency,
        schultz_factor: Ratio::new::<ratio>(f64::NAN),
        generated,
        h2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;

    #[test]
    fn generated_power_positive_for_normal_expansion() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let h1 = AvailableEnergy::new::<joule_per_kilogram>(3_000_000.0);
        let h2s = AvailableEnergy::new::<joule_per_kilogram>(2_800_000.0); // isentropic drop
        let eta = Ratio::new::<ratio>(0.85);
        let power = generated_power(w, h1, h2s, eta);
        assert!(power.get::<watt>() > 0.0);
    }

    #[test]
    fn outlet_enthalpy_lower_than_inlet_for_positive_generated_power() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let h1 = AvailableEnergy::new::<joule_per_kilogram>(3_000_000.0);
        let h2s = AvailableEnergy::new::<joule_per_kilogram>(2_800_000.0);
        let eta = Ratio::new::<ratio>(0.85);
        let generated = generated_power(w, h1, h2s, eta);
        let h2 = outlet_enthalpy(h1, generated, w);
        assert!(h2.get::<joule_per_kilogram>() < h1.get::<joule_per_kilogram>());
    }

    #[test]
    fn isentropic_and_polytropic_exponents_close_for_near_ideal_case() {
        let p1 = Pressure::new::<pascal>(1.0e6);
        let p2 = Pressure::new::<pascal>(5.0e5);
        let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(4.0);
        // Slightly different densities at ideal vs actual outlet.
        let rho2_ideal = MassDensity::new::<kilogram_per_cubic_meter>(2.3);
        let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(2.5);
        let n_i = isentropic_exponent(p1, p2, rho1, rho2_ideal);
        let n_p = polytropic_exponent(p1, p2, rho1, rho2);
        assert!(n_i.get::<ratio>() > 0.0);
        assert!(n_p.get::<ratio>() > 0.0);
        assert!((n_i.get::<ratio>() - n_p.get::<ratio>()).abs() < 0.5);
    }

    #[test]
    fn schultz_factor_close_to_1_for_nearly_identical_exponents() {
        let p1 = Pressure::new::<pascal>(1.0e6);
        let p2 = Pressure::new::<pascal>(5.0e5);
        let n_i = Ratio::new::<ratio>(1.30);
        let n_p = Ratio::new::<ratio>(1.301); // nearly identical
        let f_ce = schultz_correction_factor(p1, p2, n_i, n_p);
        assert!((f_ce.get::<ratio>() - 1.0).abs() < 0.05);
    }

    #[test]
    fn solve_polytropic_efficiency_converges() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let p1 = Pressure::new::<pascal>(1.0e6);
        let p2 = Pressure::new::<pascal>(5.0e5);
        let h1 = AvailableEnergy::new::<joule_per_kilogram>(3_000_000.0);
        let h2s = AvailableEnergy::new::<joule_per_kilogram>(2_800_000.0);
        let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(4.0);
        let rho2_ideal = MassDensity::new::<kilogram_per_cubic_meter>(2.3);
        let polytropic_efficiency = Ratio::new::<ratio>(0.85);

        // Toy "flash": pretend density tracks a fixed offset from the ideal
        // outlet density regardless of efficiency (stand-in for a real
        // property-package call in a caller like tampines).
        let evaluate_outlet = |eta: Ratio| -> (AvailableEnergy, MassDensity) {
            let generated = generated_power(w, h1, h2s, eta);
            let h2 = outlet_enthalpy(h1, generated, w);
            (
                h2,
                rho2_ideal + MassDensity::new::<kilogram_per_cubic_meter>(0.2),
            )
        };

        let result = solve_polytropic_efficiency(
            w,
            p1,
            p2,
            h1,
            h2s,
            rho1,
            rho2_ideal,
            polytropic_efficiency,
            evaluate_outlet,
            Ratio::new::<ratio>(1e-6),
        );
        assert!(result.adiabatic_efficiency.get::<ratio>().is_finite());
        assert!(result.schultz_factor.get::<ratio>().is_finite());
        assert!(result.generated.get::<watt>() > 0.0);
    }
}
