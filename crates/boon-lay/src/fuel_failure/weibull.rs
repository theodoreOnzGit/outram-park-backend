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

//! **Eq (1)** — the Weibull pressure-vessel failure law (page -483-).
//!
//! The `ln2` normalisation is the load-bearing detail; see the function's own
//! docs and [`super::strength`] for where its `m` comes from.

use uom::si::f64::{Pressure, Ratio};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

use super::FailureFraction;

/// **Eq (1)** — the fraction of particles failed by pressure-vessel overstress
/// (page -483-, attributed to Nabielek 1984).
///
/// ```text
/// φ₁(t,T) = 1 − exp[ −ln2 · (σ_t / σ_o)^m ]
/// ```
///
/// - `induced_stress` — `σ_t`, the SiC hoop stress from the internal gas
///   pressure ([`induced_stress`]).
/// - `median_strength` — `σ_o`, the SiC tensile strength **at the end of
///   irradiation**. See the module docs: because of the `ln2`, this is the
///   *median* of the strength distribution, not its characteristic value.
/// - `weibull_modulus` — `m`, dimensionless.
///
/// Returns a fraction in `[0, 1]`; it is `0.5` exactly when
/// `induced_stress == median_strength`, for any `m`.
pub fn weibull_failure_fraction(
    induced_stress: Pressure,
    median_strength: Pressure,
    weibull_modulus: f64,
) -> FailureFraction {
    let sigma_o = median_strength.get::<pascal>();
    if sigma_o <= 0.0 {
        // A zero or negative strength is total failure, not a NaN. This is the
        // limit the thinning and degradation laws walk toward.
        return Ratio::new::<ratio>(1.0);
    }
    let x = (induced_stress.get::<pascal>() / sigma_o).max(0.0);
    Ratio::new::<ratio>(1.0 - (-std::f64::consts::LN_2 * x.powf(weibull_modulus)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::megapascal;

    #[test]
    fn median_is_the_scale_parameter() {
        let s = Pressure::new::<megapascal>(200.0);
        for m in [1.0, 2.0, 5.0, 8.0, 15.0] {
            let phi = weibull_failure_fraction(s, s, m).get::<ratio>();
            assert!(
                (phi - 0.5).abs() < 1e-12,
                "m = {m}: expected exactly 0.5 at sigma_t == sigma_o, got {phi}"
            );
        }
        // And explicitly NOT the characteristic-strength convention.
        let stock = 1.0 - (-1.0_f64).exp();
        assert!((0.5 - stock).abs() > 0.1, "the two conventions must differ");
    }

    #[test]
    fn failure_is_monotone_in_stress() {
        let sigma_o = Pressure::new::<megapascal>(200.0);
        let mut last = -1.0;
        for mpa in [0.0, 50.0, 100.0, 200.0, 400.0, 800.0] {
            let phi = weibull_failure_fraction(Pressure::new::<megapascal>(mpa), sigma_o, 8.0)
                .get::<ratio>();
            assert!(phi >= last, "not monotone at {mpa} MPa: {phi} < {last}");
            assert!(
                (0.0..=1.0).contains(&phi),
                "out of range at {mpa} MPa: {phi}"
            );
            last = phi;
        }
        assert_eq!(
            weibull_failure_fraction(Pressure::new::<pascal>(0.0), sigma_o, 8.0).get::<ratio>(),
            0.0
        );
        assert!(last > 0.999_999, "should saturate well before 4x strength");
    }

    #[test]
    fn a_larger_modulus_sharpens_the_transition() {
        let sigma_o = Pressure::new::<megapascal>(200.0);
        let below = Pressure::new::<megapascal>(150.0);
        let above = Pressure::new::<megapascal>(260.0);
        let (lo_m, hi_m) = (3.0, 12.0);
        assert!(
            weibull_failure_fraction(below, sigma_o, hi_m).get::<ratio>()
                < weibull_failure_fraction(below, sigma_o, lo_m).get::<ratio>(),
            "a sharper distribution must fail FEWER particles below the median"
        );
        assert!(
            weibull_failure_fraction(above, sigma_o, hi_m).get::<ratio>()
                > weibull_failure_fraction(above, sigma_o, lo_m).get::<ratio>(),
            "a sharper distribution must fail MORE particles above the median"
        );
    }
}
