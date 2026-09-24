// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
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
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!

//! **Eqs (8a)/(8b)/(9a)/(9b)** — how irradiation degrades the SiC tensile
//! strength and the Weibull modulus (pages -493- and -494-, both attributed
//! to Allelein 1983).
//!
//! These are the two correlations that supply [`super::weibull`]'s `sigma_o`
//! and `m`, and they are **verified against the report's own Table 1** — see
//! `tests::table_1_is_reproduced_exactly`.

use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// **Eq (8b)** — the floor under the irradiated SiC tensile strength,
/// 196 MPa (page -494-).
pub const MIN_TENSILE_STRENGTH_MPA: f64 = 196.0;

/// **Eq (9b)** — the floor under the irradiated Weibull modulus, 2
/// (page -494-).
pub const MIN_WEIBULL_MODULUS: f64 = 2.0;

/// **Eq (8a)** — SiC tensile strength after irradiation (page -493-,
/// attributed to Allelein 1983), with Eq (8b)'s floor applied.
///
/// ```text
/// sigma_o = sigma_oo * (1 - Gamma / Gamma_s)
/// log10(Gamma_s) = 0.556 + 0.065e4 / T_B
/// ```
///
/// - `unirradiated` — `sigma_oo`, the measured strength before irradiation.
/// - `fluence_e25_per_m2` — `Gamma`, fast-neutron fluence **in the
///   correlation's own units of 10^25 m^-2 EDN**. Not `uom`-typed on
///   purpose: this is a `log10` fit whose intercept is only meaningful in
///   those units, so a dimensioned argument would imply a freedom of unit
///   choice the correlation does not have. The name carries the unit instead.
/// - `irradiation_temperature` — `T_B`, **in kelvin**. The report's symbol
///   list prints `T_B` in degC and never states the conversion; that it is
///   kelvin is established by [`table_1_is_reproduced_exactly`], which
///   reproduces all sixteen of the report's own calculated values only on
///   the kelvin reading.
///
/// Returns at least [`MIN_TENSILE_STRENGTH_MPA`].
pub fn irradiated_strength(
    unirradiated: Pressure,
    fluence_e25_per_m2: f64,
    irradiation_temperature: ThermodynamicTemperature,
) -> Pressure {
    let t_b = irradiation_temperature.get::<kelvin>();
    let gamma_s = 10f64.powf(0.556 + 0.065e4 / t_b);
    let sigma = unirradiated.get::<pascal>() * (1.0 - fluence_e25_per_m2 / gamma_s);
    Pressure::new::<pascal>(sigma.max(MIN_TENSILE_STRENGTH_MPA * 1.0e6))
}

/// **Eq (9a)** — the Weibull modulus after irradiation (page -494-,
/// Allelein 1983), with Eq (9b)'s floor applied.
///
/// ```text
/// m_o = m_oo * (1 - Gamma / Gamma_m)
/// log10(Gamma_m) = 0.394 + 0.065e4 / T_B
/// ```
///
/// Same `10^4/T_B` coefficient as [`irradiated_strength`]'s `Gamma_s`; only
/// the intercept differs (0.394 against 0.556), so the modulus degrades
/// **faster** than the strength -- the distribution widens as it weakens.
///
/// Eq (10a) then states `m = m_o`, i.e. this is the modulus that goes into
/// [`weibull_failure_fraction`].
///
/// Returns at least [`MIN_WEIBULL_MODULUS`].
pub fn irradiated_weibull_modulus(
    unirradiated: f64,
    fluence_e25_per_m2: f64,
    irradiation_temperature: ThermodynamicTemperature,
) -> f64 {
    let t_b = irradiation_temperature.get::<kelvin>();
    let gamma_m = 10f64.powf(0.394 + 0.065e4 / t_b);
    (unirradiated * (1.0 - fluence_e25_per_m2 / gamma_m)).max(MIN_WEIBULL_MODULUS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::megapascal;
    use uom::si::thermodynamic_temperature::degree_celsius;

    #[test]
    fn table_1_is_reproduced_exactly() {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let fluence = 1.0; // 1e25 m^-2 EDN

        // specimen, sigma_oo [MPa], m_oo, printed sigma_o, printed m_o
        let table = [
            ("EO 1674", 722.0, 7.0, 660.0, 6.1),
            ("EO 1607", 850.0, 8.0, 777.0, 7.0),
            ("HT 150-167", 600.0, 6.0, 549.0, 5.3),
            ("EO 249-251", 453.0, 5.0, 414.0, 4.4),
            ("EO 403-405", 867.0, 8.4, 793.0, 7.4),
            ("EUO 1551", 1060.0, 8.5, 969.0, 7.4),
            ("ECO 1541", 1080.0, 6.4, 987.0, 5.6),
            ("EC 1338/1339", 998.0, 7.4, 912.0, 6.5),
        ];

        for (name, sigma_oo, m_oo, want_sigma, want_m) in table {
            let got_sigma =
                irradiated_strength(Pressure::new::<megapascal>(sigma_oo), fluence, t_b)
                    .get::<megapascal>();
            let got_m = irradiated_weibull_modulus(m_oo, fluence, t_b);

            assert!(
                (got_sigma - want_sigma).abs() <= 1.0,
                "{name}: sigma_o {got_sigma:.1} MPa vs the table's {want_sigma}"
            );
            assert!(
                (got_m - want_m).abs() <= 0.05,
                "{name}: m_o {got_m:.2} vs the table's {want_m}"
            );
        }
    }

    #[test]
    fn the_celsius_reading_of_t_b_is_excluded() {
        // 1000 as if it were already kelvin -- i.e. reading the symbol
        // list's degC literally and not converting.
        let wrong = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let got =
            irradiated_strength(Pressure::new::<megapascal>(722.0), 1.0, wrong).get::<megapascal>();
        assert!(
            (got - 660.0).abs() > 5.0,
            "the degC reading gives {got:.1} MPa, which must be clearly \
             distinguishable from the table's 660"
        );
    }

    #[test]
    fn the_floors_hold_at_high_fluence() {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        // Gamma far past Gamma_s (~11.7) and Gamma_m (~8.0).
        let sigma = irradiated_strength(Pressure::new::<megapascal>(600.0), 100.0, t_b);
        assert_eq!(sigma.get::<megapascal>(), MIN_TENSILE_STRENGTH_MPA);
        assert_eq!(
            irradiated_weibull_modulus(6.0, 100.0, t_b),
            MIN_WEIBULL_MODULUS
        );
    }

    #[test]
    fn the_modulus_degrades_faster_than_the_strength() {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let f = 1.0;
        let sigma_ratio = irradiated_strength(Pressure::new::<megapascal>(1000.0), f, t_b)
            .get::<megapascal>()
            / 1000.0;
        let m_ratio = irradiated_weibull_modulus(10.0, f, t_b) / 10.0;
        assert!(
            m_ratio < sigma_ratio,
            "m kept {m_ratio:.4} but sigma kept {sigma_ratio:.4} -- the modulus \
             must fall faster"
        );
    }

    /// **Eqs (8a)/(9a) reproduce Table 2 as well as Table 1 — 4/4, at a
    /// different fluence and irradiation temperature.**
    ///
    /// Methodology: Table 2 (page -504-) lists `σ_oo/σ_o` and `m_oo/m_o` for
    /// the HTR-Module and HTR-500 cases, computed at `Γ = 1.4·10²⁵ m⁻² EDN`
    /// and the average irradiation temperatures it also states — 776 °C and
    /// 792 °C. Table 1's sixteen values were all at `Γ = 1` and 1000 °C, so
    /// this exercises the fluence and temperature dependence rather than
    /// repeating one operating point.
    ///
    /// | case | `T_B` | Table 2 `σ_o` | Eq (8a) | Table 2 `m_o` | Eq (9a) |
    /// |---|---|---|---|---|---|
    /// | HTR-Module | 776 °C | 756 | **756.1** | 6.93 | **6.932** |
    /// | HTR-500 | 792 °C | 754 | **754.4** | 6.91 | **6.908** |
    ///
    /// Recorded 2026-09-24. It matters beyond being a fourth data point: the
    /// `m`-dependent residual seen against Fig. 6 was at one stage suspected
    /// of being a defect in the degradation law, and this is independent
    /// evidence that the law is right — consistent with that residual turning
    /// out to be a digitisation artefact.
    ///
    /// Uses `σ_oo = 834`, `m_oo = 8.02` (EO 1607 as Fig. 5 and Table 2 give
    /// it, not Table 1's 850/8.0 — see
    /// `docs/panama-i-units-and-open-questions.md`).
    #[test]
    fn table_2_is_reproduced_at_a_different_fluence() {
        for (t_b_c, want_sigma, want_m) in [(776.0, 756.0, 6.93), (792.0, 754.0, 6.91)] {
            let t_b = ThermodynamicTemperature::new::<degree_celsius>(t_b_c);
            let sigma = irradiated_strength(Pressure::new::<megapascal>(834.0), 1.4, t_b)
                .get::<megapascal>();
            let m = irradiated_weibull_modulus(8.02, 1.4, t_b);
            assert!(
                (sigma - want_sigma).abs() < 0.5,
                "T_B = {t_b_c} degC: Eq (8a) gives {sigma:.1} MPa, Table 2 states {want_sigma}"
            );
            assert!(
                (m - want_m).abs() < 0.005,
                "T_B = {t_b_c} degC: Eq (9a) gives {m:.3}, Table 2 states {want_m}"
            );
        }
    }
}
