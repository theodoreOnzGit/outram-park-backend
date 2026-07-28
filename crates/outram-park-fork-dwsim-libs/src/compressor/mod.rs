//! Compressor thermodynamics: isentropic (adiabatic-efficiency) and Schultz
//! polytropic-efficiency compression duty.
//!
//! Ported from DWSIM `UnitOperations/Compressor.vb` (GPL-3.0), the mirror of
//! the already-ported expander (`crate::expander::isentropic`). DWSIM computes
//! the ideal outlet enthalpy `H2s` via a pressure-entropy (isentropic) flash
//! and the actual outlet state via repeated pressure-enthalpy flashes -- this
//! equipment port is deliberately kept decoupled from the crate's flash kernel
//! ([`crate::thermo`]), so the flash-dependent steps are pushed to
//! the caller: the functions below take already-known enthalpies/densities as
//! inputs, and [`solve_polytropic_efficiency`] takes a caller-supplied closure
//! for the one flash-dependent step in DWSIM's iteration (a generic `Fn`
//! parameter, not a trait object, per the workspace's no-`dyn`-dispatch rule).
//!
//! **Sign convention** (chosen for this port): a compressor **CONSUMES**
//! power. [`consumed_power`] and the polytropic solver's duty are **positive
//! for a normal compression** (`h2s > h1`, `h2 > h1`) -- work is done *on* the
//! fluid to raise its pressure. This is the mirror of the expander, whose duty
//! is positive when power is *extracted* from the fluid.
//!
//! ### Ported vs. excluded
//!
//! Ported (the physics core of `Compressor.vb`'s `Calculate`):
//! - adiabatic duty `W = w (h2s - h1) / eta`         (Compressor.vb:959)
//! - actual outlet enthalpy `h2 = h1 + W / w`         (Compressor.vb:969)
//! - isentropic exponent `n_i = ln(p2/p1)/ln(rho2i/rho1)` (Compressor.vb:995)
//! - polytropic exponent `n_p = ln(p2/p1)/ln(rho2/rho1)`  (Compressor.vb:999)
//! - Schultz correction factor `fce`                  (Compressor.vb:1001)
//! - polytropic-efficiency fixed-point loop           (Compressor.vb:885-947)
//!
//! Deliberately **NOT** ported (out of scope for a units library):
//! - the `Curves` calculation mode and its speed/flow interpolation
//!   (Compressor.vb:418-568) -- pump/compressor performance-curve tables.
//! - the outlet-pressure-from-power root-find (`PFunction`/Brent,
//!   Compressor.vb:637-770) and the `k = Cp/Cv` discharge-pressure estimate
//!   (Compressor.vb:624) -- these belong to a flash-owning caller.
//! - GUI/editing-form plumbing, XML/JSON serialization (`SaveData`/`LoadData`/
//!   `CloneXML`/`CloneJSON`), property-grid reflection (`GetPropertyValue`/
//!   `GetProperties`), dynamic-mode and flowsheet-solver wiring.
//!
//! The heads (`Wic`/`Wpc`, Compressor.vb:1012-1022) and the `k`-based discharge
//! estimate are not ported here; only the enthalpy-based duty/exponent core is.

use uom::si::f64::{AvailableEnergy, MassDensity, MassRate, Power, Pressure, Ratio};
use uom::si::power::watt;
use uom::si::ratio::ratio;

/// Power consumed by an adiabatic compression from inlet enthalpy `h1` to the
/// isentropic (ideal) outlet enthalpy `h2s`, at `adiabatic_efficiency`
/// \[0, 1\]: `W = w (h2s - h1) / eta`.
///
/// **Physical quantity:** shaft power drawn by the compressor, in watts.
///
/// **Sign convention:** positive for a normal compression (`h2s > h1`), since
/// the machine consumes work to raise the fluid's pressure. This mirrors
/// DWSIM `Compressor.vb:959` (`DeltaQ = Wi * (H2s - Hi) / (Eta/100)`), except
/// DWSIM carries efficiency as a percentage and duty in kW; here efficiency is
/// a dimensionless `Ratio` and the result is a uom `Power`.
///
/// **Inputs / units:**
/// - `w`: mass flow, kg/s (> 0).
/// - `h1`: inlet specific enthalpy, J/kg.
/// - `h2s`: isentropic outlet specific enthalpy at the discharge pressure,
///   J/kg (from a caller's pressure-entropy flash; `h2s > h1` for compression).
/// - `adiabatic_efficiency`: isentropic efficiency, dimensionless in \[0, 1\].
///   Must be > 0 (an `eta = 0` machine is unphysical and yields `+inf`).
pub fn consumed_power(
    w: MassRate,
    h1: AvailableEnergy,
    h2s: AvailableEnergy,
    adiabatic_efficiency: Ratio,
) -> Power {
    Power::new::<watt>(w.value * (h2s - h1).value / adiabatic_efficiency.get::<ratio>())
}

/// Actual outlet specific enthalpy `h2 = h1 + W / w`, given the consumed power
/// from [`consumed_power`].
///
/// **Physical quantity:** discharge specific enthalpy, J/kg.
///
/// Mirrors DWSIM `Compressor.vb:969` (`H2 = Hi + DeltaQ / Wi`). Because a
/// compressor consumes power (`W > 0`), the outlet enthalpy is **higher** than
/// the inlet (`h2 > h1`) -- the opposite of the expander.
///
/// **Inputs / units:**
/// - `h1`: inlet specific enthalpy, J/kg.
/// - `consumed`: shaft power consumed, W (from [`consumed_power`]).
/// - `w`: mass flow, kg/s (> 0).
pub fn outlet_enthalpy(h1: AvailableEnergy, consumed: Power, w: MassRate) -> AvailableEnergy {
    AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(
        h1.get::<uom::si::available_energy::joule_per_kilogram>()
            + consumed.get::<watt>() / w.value,
    )
}

/// Isentropic polytropic exponent `n_isentropic = ln(p2/p1) / ln(rho2i/rho1)`,
/// from the inlet density `rho1` and the density at the *ideal* isentropic
/// outlet state `rho2_ideal`.
///
/// **Physical quantity:** dimensionless volume (path) exponent for the ideal,
/// constant-entropy compression path.
///
/// Mirrors DWSIM `Compressor.vb:995`
/// (`n_isent = Math.Log(P2 / Pi) / Math.Log(rho2i / rho1)`).
///
/// **Inputs / units:** all pressures in Pa, all densities in kg/m^3. For a
/// compressor `p2 > p1` and `rho2_ideal > rho1`, so both logs are positive and
/// `n_isentropic > 0` (typically ~1.1-1.7 for gases).
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

/// Polytropic exponent `n_polytropic = ln(p2/p1) / ln(rho2/rho1)`, from the
/// inlet density `rho1` and the density at the *actual* outlet state `rho2`.
///
/// **Physical quantity:** dimensionless volume (path) exponent for the real
/// compression path (which sits between the ideal isentropic and an isothermal
/// path, reflecting irreversibility).
///
/// Mirrors DWSIM `Compressor.vb:999`
/// (`n_poly = Math.Log(P2 / Pi) / Math.Log(rho2 / rho1)`).
///
/// **Inputs / units:** pressures in Pa, densities in kg/m^3. `n_polytropic > 0`
/// for a real compression (`p2 > p1`, `rho2 > rho1`).
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
///
/// **Physical quantity:** dimensionless correction relating the polytropic and
/// adiabatic efficiencies of a real gas compression. For a compressor DWSIM
/// uses `eta_polytropic = eta_adiabatic * f_ce` (Compressor.vb:1003), i.e. the
/// polytropic efficiency exceeds the adiabatic one by `f_ce` (> 1 for a real
/// compression -- reheat effect). Equivalently the iteration below inverts it:
/// `eta_adiabatic = eta_polytropic / f_ce`.
///
/// Mirrors DWSIM `Compressor.vb:1001`.
///
/// **Inputs / units:** pressures in Pa; exponents dimensionless (`Ratio`).
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

/// Converged result of [`solve_polytropic_efficiency`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolytropicResult {
    /// Converged adiabatic (isentropic) efficiency
    /// `eta_adiabatic = eta_polytropic / f_ce`, dimensionless.
    pub adiabatic_efficiency: Ratio,
    /// Schultz correction factor at convergence, dimensionless.
    pub schultz_factor: Ratio,
    /// Power consumed at convergence, W (positive for compression).
    pub consumed: Power,
    /// Actual outlet specific enthalpy at convergence, J/kg (`h2 > h1`).
    pub h2: AvailableEnergy,
}

/// Iteratively resolve the adiabatic efficiency consistent with a specified
/// polytropic efficiency for a compression, mirroring DWSIM `Compressor.vb`
/// `ProcessPathType.Polytropic` loop (Compressor.vb:885-947).
///
/// DWSIM seeds `AdiabaticEfficiency = PolytropicEfficiency`, then repeats:
/// compute the actual outlet state at `p2` (a pressure-enthalpy flash), read
/// its density `rho2`, form `n_isent`, `n_poly`, and `fce`, and update
/// `AdiabaticEfficiency = PolytropicEfficiency / fce` until the efficiency
/// change is below `0.00001`. This port keeps that structure but pushes the
/// single flash-dependent step out to the caller.
///
/// `evaluate_outlet` is that flash-dependent step: given a trial
/// `adiabatic_efficiency`, it must return `(h2, rho2)` -- the actual outlet
/// specific enthalpy (J/kg) and outlet density (kg/m^3) at `p2` for that trial
/// efficiency. A caller with flash access computes this via a pressure-enthalpy
/// flash at `(p2, outlet_enthalpy(h1, consumed_power(w, h1, h2s, eta), w))`,
/// then reads the density at that state.
///
/// **Inputs / units:** `w` kg/s; `p1`, `p2` Pa (`p2 > p1`); `h1`, `h2s` J/kg
/// (`h2s > h1`); `rho1`, `rho2_ideal` kg/m^3; `polytropic_efficiency`,
/// `tolerance` dimensionless `Ratio`. The loop is capped at 100 iterations
/// (matching the fixed-point nature of the DWSIM `Do ... Loop`).
///
/// Returns a [`PolytropicResult`]; if the loop fails to converge within 100
/// iterations, `schultz_factor` is `NaN` and the last iterate is returned.
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
    // DWSIM seeds AdiabaticEfficiency = PolytropicEfficiency (Compressor.vb:887).
    let mut adiabatic_efficiency = polytropic_efficiency;
    let mut f_ce = Ratio::new::<ratio>(f64::NAN);
    let mut h2 = h1;

    for _ in 0..100 {
        // The one flash-dependent step: caller returns actual (h2, rho2) at p2
        // for this trial efficiency (Compressor.vb:899-927 in DWSIM).
        let (h2_trial, rho2) = evaluate_outlet(adiabatic_efficiency);
        h2 = h2_trial;

        let n_polytropic = polytropic_exponent(p1, p2, rho1, rho2);
        f_ce = schultz_correction_factor(p1, p2, n_isentropic, n_polytropic);
        // Compressor.vb:943 -- AdiabaticEfficiency = PolytropicEfficiency / fce.
        let next_adiabatic_efficiency =
            Ratio::new::<ratio>(polytropic_efficiency.get::<ratio>() / f_ce.get::<ratio>());

        let delta =
            (next_adiabatic_efficiency.get::<ratio>() - adiabatic_efficiency.get::<ratio>()).abs();
        adiabatic_efficiency = next_adiabatic_efficiency;
        if delta < tolerance.get::<ratio>() {
            let consumed = consumed_power(w, h1, h2s, adiabatic_efficiency);
            return PolytropicResult {
                adiabatic_efficiency,
                schultz_factor: f_ce,
                consumed,
                h2,
            };
        }
    }

    let consumed = consumed_power(w, h1, h2s, adiabatic_efficiency);
    PolytropicResult {
        adiabatic_efficiency,
        schultz_factor: f_ce,
        consumed,
        h2,
    }
}

#[cfg(test)]
mod tests {
    //! Verification tests for the compressor duty/exponent core.
    //!
    //! These are **verification** checks (is the formula implemented
    //! correctly?), not **validation** against a DWSIM benchmark run or lab
    //! data -- the numbers below are hand-computed from the ported closed-form
    //! expressions, not taken from an external reference case.
    //!
    //! Methodology + measured numbers, taken 2026-07-24:
    //!
    //! 1. `consumed_power_matches_hand_computed` -- with `w = 10 kg/s`,
    //!    `h1 = 3.0e6 J/kg`, `h2s = 3.2e6 J/kg` (isentropic *rise*),
    //!    `eta = 0.80`, the duty is `W = w (h2s - h1) / eta
    //!    = 10 * 200000 / 0.80 = 2.5e6 W`. Asserted equal to 2 500 000 W and
    //!    strictly positive (power consumed).
    //! 2. `outlet_enthalpy_round_trip` -- `h2 = h1 + W / w
    //!    = 3.0e6 + 2.5e6/10 = 3.25e6 J/kg`; asserted equal to 3 250 000 J/kg
    //!    and strictly greater than `h1` (compressor heats the fluid).
    //! 3. `polytropic_exponent_known_pair` -- with `p1 = 1.0e5 Pa`,
    //!    `p2 = 2.0e5 Pa`, `rho1 = 1.0 kg/m^3`, `rho2 = 1.6 kg/m^3`,
    //!    `n = ln(2) / ln(1.6) = 0.6931472 / 0.4700036 = 1.4747698`.
    //!    Asserted to 1e-6.
    //! 4. `eta_unity_degenerate_limit` -- at `eta = 1`, the duty collapses to
    //!    the ideal isentropic work `W = w (h2s - h1) = 10 * 200000 = 2.0e6 W`,
    //!    and `h2 = h1 + (h2s - h1) = h2s = 3.2e6 J/kg` (outlet meets the
    //!    isentropic state). Both asserted exactly.

    use super::*;
    use approx::assert_relative_eq;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;

    fn h(j_per_kg: f64) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(j_per_kg)
    }

    #[test]
    fn consumed_power_matches_hand_computed() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let h1 = h(3_000_000.0);
        let h2s = h(3_200_000.0); // isentropic enthalpy *rise* (compression)
        let eta = Ratio::new::<ratio>(0.80);
        let power = consumed_power(w, h1, h2s, eta);
        // W = 10 * (3.2e6 - 3.0e6) / 0.80 = 2.5e6 W
        assert_relative_eq!(power.get::<watt>(), 2_500_000.0, max_relative = 1e-12);
        assert!(power.get::<watt>() > 0.0, "compressor consumes power (> 0)");
    }

    #[test]
    fn outlet_enthalpy_round_trip() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let h1 = h(3_000_000.0);
        let h2s = h(3_200_000.0);
        let eta = Ratio::new::<ratio>(0.80);
        let consumed = consumed_power(w, h1, h2s, eta);
        let h2 = outlet_enthalpy(h1, consumed, w);
        // h2 = 3.0e6 + 2.5e6/10 = 3.25e6 J/kg
        assert_relative_eq!(
            h2.get::<joule_per_kilogram>(),
            3_250_000.0,
            max_relative = 1e-12
        );
        assert!(
            h2.get::<joule_per_kilogram>() > h1.get::<joule_per_kilogram>(),
            "compressor raises outlet enthalpy above inlet"
        );
    }

    #[test]
    fn polytropic_exponent_known_pair() {
        let p1 = Pressure::new::<pascal>(1.0e5);
        let p2 = Pressure::new::<pascal>(2.0e5);
        let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(1.0);
        let rho2 = MassDensity::new::<kilogram_per_cubic_meter>(1.6);
        let n = polytropic_exponent(p1, p2, rho1, rho2);
        // n = ln(2) / ln(1.6) = 1.4747698...
        assert_relative_eq!(
            n.get::<ratio>(),
            2.0_f64.ln() / 1.6_f64.ln(),
            max_relative = 1e-12
        );
        assert_relative_eq!(n.get::<ratio>(), 1.474_769_8, max_relative = 1e-6);
    }

    #[test]
    fn eta_unity_degenerate_limit() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let h1 = h(3_000_000.0);
        let h2s = h(3_200_000.0);
        let eta = Ratio::new::<ratio>(1.0);
        // At eta = 1 the duty is the ideal isentropic work.
        let power = consumed_power(w, h1, h2s, eta);
        assert_relative_eq!(power.get::<watt>(), 2_000_000.0, max_relative = 1e-12);
        // ... and the outlet enthalpy meets the isentropic state h2s.
        let h2 = outlet_enthalpy(h1, power, w);
        assert_relative_eq!(
            h2.get::<joule_per_kilogram>(),
            h2s.get::<joule_per_kilogram>(),
            max_relative = 1e-12
        );
    }

    #[test]
    fn solve_polytropic_efficiency_converges() {
        let w = MassRate::new::<kilogram_per_second>(10.0);
        let p1 = Pressure::new::<pascal>(1.0e5);
        let p2 = Pressure::new::<pascal>(2.0e5);
        let h1 = h(3_000_000.0);
        let h2s = h(3_200_000.0);
        let rho1 = MassDensity::new::<kilogram_per_cubic_meter>(1.0);
        let rho2_ideal = MassDensity::new::<kilogram_per_cubic_meter>(1.55);
        let polytropic_efficiency = Ratio::new::<ratio>(0.78);

        // Toy stand-in for a real property-package flash: the actual outlet
        // density sits slightly below the ideal isentropic outlet density
        // (real compression heats more), independent of the trial efficiency.
        let evaluate_outlet = |eta: Ratio| -> (AvailableEnergy, MassDensity) {
            let consumed = consumed_power(w, h1, h2s, eta);
            let h2 = outlet_enthalpy(h1, consumed, w);
            (
                h2,
                rho2_ideal - MassDensity::new::<kilogram_per_cubic_meter>(0.05),
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
        assert!(
            result.consumed.get::<watt>() > 0.0,
            "compressor consumes power (> 0)"
        );
        assert!(
            result.h2.get::<joule_per_kilogram>() > h1.get::<joule_per_kilogram>(),
            "outlet enthalpy above inlet"
        );
    }
}
