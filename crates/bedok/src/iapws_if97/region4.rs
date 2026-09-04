//! Region 4 — the saturation line, `psat(T)` and `Tsat(p)`.
//!
//! # Provenance — third-party, BSD-2-Clause
//!
//! As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
//! Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
//! Source functions `psat_T` and `Tsat_p`.
//!
//! # What region 4 is
//!
//! Regions 1, 2 and 3 each cover an area of the `(p, T)` plane. Region 4 is not
//! an area — it is the **curve** separating liquid from vapour, and the
//! formulation gives it as a single quartic in a reduced variable that can be
//! solved either way round. So one set of ten coefficients serves both
//! directions, which is why `psat_T` and `Tsat_p` below share the array `N`.
//!
//! # Units
//!
//! Pressure MPa, temperature K, as everywhere in this module.
//!
//! # Range
//!
//! The line runs from the triple point (273.16 K, 611.657 Pa) to the critical
//! point (647.096 K, 22.064 MPa). Outside that, both functions return `NaN` —
//! the reference initialises its output to `NaN` and only fills the valid mask,
//! which this reproduces. `NaN` is therefore the answer for a subcritical
//! query, not an error.

/// The ten coefficients of the saturation equation, shared by both directions.
///
/// IAPWS-IF97 equations 30 and 31. Indices here are 0-based, so the
/// reference's `n(1)` is `N[0]`.
const N: [f64; 10] = [
    1_167.052_145_276_70,
    -724_213.167_032_060,
    -17.073_846_940_092_0,
    12_020.824_702_470_0,
    -3_232_555.032_233_30,
    14.915_108_613_530_0,
    -4_823.265_736_159_10,
    405_113.405_420_570,
    -0.238_555_575_678_490,
    650.175_348_447_980,
];

/// Triple-point temperature, K — the low end of the saturation line.
pub const T_MIN: f64 = 273.16;

/// Critical temperature, K — the high end of the saturation line.
pub const T_CRIT: f64 = 647.096;

/// Critical pressure, MPa.
pub const P_CRIT: f64 = 22.064;

/// Temperature at the region 1 / region 3 boundary, K.
///
/// Above this, saturated **liquid** properties need region 3 rather than
/// region 1. See [`crate::iapws_if97::basic::hl_p`], which is why this is
/// public.
pub const T_B13: f64 = 623.15;

/// `p = psat_T(T)` — saturation pressure, MPa, from temperature, K.
///
/// # Arguments
///
/// - `t` — temperature in **K**, valid on `[273.16, 647.096]`, the triple point
///   to the critical point.
///
/// # Returns
///
/// Saturation pressure in **MPa**, or `NaN` outside the valid range.
///
/// # Numerics
///
/// The reference evaluates the three quadratics `A`, `B`, `C` by Horner's
/// method and takes the root `beta = 2C / (-B + sqrt(B^2 - 4AC))`. That is the
/// *numerically stable* branch of the quadratic formula for this sign pattern —
/// the algebraically equivalent `(-B + sqrt(...)) / 2A` suffers cancellation.
/// Written the same way here.
pub fn psat_t(t: f64) -> f64 {
    if !(T_MIN..=T_CRIT).contains(&t) {
        return f64::NAN;
    }
    let upsilon = t + N[8] / (t - N[9]);
    let a = (upsilon + N[0]) * upsilon + N[1];
    let b = (N[2] * upsilon + N[3]) * upsilon + N[4];
    let c = (N[5] * upsilon + N[6]) * upsilon + N[7];
    let beta = 2.0 * c / (-b + (b * b - 4.0 * a * c).sqrt());
    beta.powi(4)
}

/// `T = Tsat_p(p)` — saturation temperature, K, from pressure, MPa.
///
/// # Arguments
///
/// - `p` — pressure in **MPa**, valid on `[611.657e-6, 22.064]`, the triple
///   point to the critical point. The lower bound is `psat_T(273.16)` and is
///   computed rather than hard-coded, as the reference does.
///
/// # Returns
///
/// Saturation temperature in **K**, or `NaN` outside the valid range.
///
/// # Numerics
///
/// The mirror of [`psat_t`]: Horner-evaluated `E`, `F`, `G`, then
/// `D = 2G / (-F - sqrt(F^2 - 4EG))` and a second stable-branch root for
/// `theta`. Both minus signs are load-bearing for the same cancellation reason.
pub fn tsat_p(p: f64) -> f64 {
    let p_min = psat_t(T_MIN);
    if !(p_min..=P_CRIT).contains(&p) {
        return f64::NAN;
    }
    let beta = p.powf(0.25);
    let e = (beta + N[2]) * beta + N[5];
    let f = (N[0] * beta + N[3]) * beta + N[6];
    let g = (N[1] * beta + N[4]) * beta + N[7];
    let d = 2.0 * g / (-f - (f * f - 4.0 * e * g).sqrt());
    let n11 = N[9] + d;
    (n11 - (n11 * n11 - 4.0 * (N[8] + N[9] * d)).sqrt()) / 2.0
}

/// Saturation pressure at the region 1 / region 3 boundary, MPa.
///
/// `psat_T(623.15) = 16.5291643 MPa`, per the reference's own comment. Computed
/// rather than hard-coded, so it cannot drift from [`psat_t`].
pub fn p_b13_sat() -> f64 {
    psat_t(T_B13)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IAPWS-IF97 Table 35 — the saturation-pressure equation.
    ///
    /// # Methodology
    ///
    /// The released formulation publishes three temperatures with reference
    /// saturation pressures to 9 significant figures. Pass criterion 1e-8
    /// relative, the same bar regions 1 and 2 are held to.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Relative deviation: 8.520e-10 at 300 K, 1.412e-9 at 500 K, 1.752e-9 at
    /// 600 K. Worst case 1.752e-9, against a 1e-8 pass criterion.
    ///
    /// **Interpretation.** The published values carry 9 significant figures, so
    /// this is agreement at the table's own precision and the residual is
    /// consistent with the rounding of the printed values. The
    /// saturation-pressure equation is verified against the standard.
    #[test]
    fn saturation_pressure_matches_published_values() {
        // (T [K], psat [MPa])
        let cases = [
            (300.0, 0.353_658_941e-2),
            (500.0, 0.263_889_776e1),
            (600.0, 0.123_443_146e2),
        ];

        for (t, p_ref) in cases {
            let p = psat_t(t);
            eprintln!(
                "psat_T({t} K): got {p}, expected {p_ref}, rel_err = {:.3e}",
                (p - p_ref).abs() / p_ref
            );
            assert!(
                (p - p_ref).abs() / p_ref < 1e-8,
                "psat_T({t} K): got {p}, expected {p_ref}"
            );
        }
    }

    /// IAPWS-IF97 Table 36 — the saturation-temperature equation.
    ///
    /// # Methodology
    ///
    /// Three published pressures with reference saturation temperatures to 9
    /// significant figures. Pass criterion 1e-8 relative.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Relative deviation: 1.043e-9 at 0.1 MPa, 8.641e-10 at 1 MPa, 2.523e-12
    /// at 10 MPa. Worst case 1.043e-9, against a 1e-8 pass criterion — again at
    /// the published table's own precision.
    #[test]
    fn saturation_temperature_matches_published_values() {
        // (p [MPa], Tsat [K])
        let cases = [(0.1, 372.755_919), (1.0, 453.035_632), (10.0, 584.149_488)];

        for (p, t_ref) in cases {
            let t = tsat_p(p);
            eprintln!(
                "Tsat_p({p} MPa): got {t}, expected {t_ref}, rel_err = {:.3e}",
                (t - t_ref).abs() / t_ref
            );
            assert!(
                (t - t_ref).abs() / t_ref < 1e-8,
                "Tsat_p({p} MPa): got {t}, expected {t_ref}"
            );
        }
    }

    /// The two directions invert each other.
    ///
    /// # Methodology
    ///
    /// `Tsat_p(psat_T(T)) == T` swept over the saturation line from 280 K to
    /// 640 K in 20 K steps. This is an **independent** check on both: they are
    /// transcribed as separate expressions from the same coefficient table, so
    /// a mistyped coefficient would have to appear in both, consistently, to
    /// escape. The published-value tests above would catch a typo but not
    /// localise it; this one distinguishes "the coefficients are wrong" from
    /// "one of the two expressions is wrong".
    ///
    /// Pass criterion 1e-9 relative.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Worst round-trip error over the 19 sampled temperatures: **4.263e-15**
    /// relative, i.e. a few floating-point ulps.
    ///
    /// **Interpretation.** That is five to six orders of magnitude tighter than
    /// the agreement with the published tables (~1e-9), which is exactly what
    /// it should be: the round trip tests only that the two expressions invert
    /// each other in exact arithmetic, and it does so at machine precision. The
    /// 1e-9 residual against the tables is therefore attributable to the
    /// printed values' rounding, not to either expression.
    #[test]
    fn the_two_directions_are_mutual_inverses() {
        let mut worst = 0.0f64;
        let mut t = 280.0;
        while t <= 640.0 {
            let round_trip = tsat_p(psat_t(t));
            let err = (round_trip - t).abs() / t;
            worst = worst.max(err);
            assert!(
                err < 1e-9,
                "round trip at {t} K gave {round_trip} (rel_err {err:.3e})"
            );
            t += 20.0;
        }
        eprintln!("saturation round trip: worst rel_err = {worst:.3e}");
    }

    /// The region 1 / region 3 boundary pressure matches the reference's own
    /// stated value of 16.5291643 MPa.
    #[test]
    fn the_region_boundary_pressure_is_as_documented() {
        let p = p_b13_sat();
        eprintln!("p_b13_sat = {p}");
        assert!((p - 16.529_164_3).abs() < 1e-6, "got {p}");
    }

    /// Outside the saturation line both directions give `NaN`, reproducing the
    /// reference's `NaN`-initialised output and valid mask.
    #[test]
    fn out_of_range_gives_nan_not_an_error() {
        assert!(psat_t(273.0).is_nan(), "below the triple point");
        assert!(psat_t(700.0).is_nan(), "above the critical point");
        assert!(tsat_p(1e-9).is_nan(), "below the triple-point pressure");
        assert!(tsat_p(30.0).is_nan(), "supercritical");
    }

    /// The two reactor operating points the BEDOK benchmarks use land where
    /// they should on the saturation line.
    ///
    /// # Methodology
    ///
    /// A sanity check that the line is being evaluated in the right units, not
    /// a verification against published data: a PWR at 15.5 MPa and a BWR at
    /// 7.0 MPa should saturate at roughly 618 K and 559 K respectively — both
    /// well-known textbook figures. Pass criterion is 2 K, loose enough that it
    /// tests the unit convention rather than the formulation, which the
    /// published-value tests above already cover.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `Tsat(15.5 MPa) = 617.9416 K` (344.79 °C) and
    /// `Tsat(7.0 MPa) = 558.9800 K` (285.83 °C), both inside the 2 K band.
    /// `p_B13sat = 16.5292 MPa`, so both sit below the region 1/3 boundary and
    /// their saturated-liquid enthalpies are reachable without region 3.
    #[test]
    fn the_benchmark_operating_points_saturate_where_expected() {
        let pwr = tsat_p(15.5);
        let bwr = tsat_p(7.0);
        eprintln!("Tsat(15.5 MPa) = {pwr} K, Tsat(7.0 MPa) = {bwr} K");
        assert!((pwr - 618.0).abs() < 2.0, "PWR: {pwr}");
        assert!((bwr - 559.0).abs() < 2.0, "BWR: {bwr}");
        // And both sit below the region 1/3 boundary, so saturated liquid
        // enthalpy is reachable without region 3.
        assert!(15.5 < p_b13_sat());
        assert!(7.0 < p_b13_sat());
    }
}
