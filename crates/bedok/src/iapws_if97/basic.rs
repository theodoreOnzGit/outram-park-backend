//! Basic property functions — enthalpy from the region equations.
//!
//! # Provenance — third-party, BSD-2-Clause
//!
//! As [`crate::iapws_if97`]: translated from `IAPWS_IF97.m` by Mark Mikofski,
//! Copyright (c) 2013, BSD-2-Clause, terms reproduced in the crate `NOTICE`.
//! Source functions `h1_pT`, `h2_pT`, `hL_p`, `hV_p`.
//!
//! # What belongs here
//!
//! The reference's "basic and fundamental functions" block — the thin layer
//! that turns a region's dimensionless Gibbs derivative into a dimensioned
//! property. `h1_pT` is three lines around [`crate::iapws_if97::region1`], and
//! that is the whole pattern.
//!
//! Only the functions BEDOK actually calls are translated. The full block also
//! carries `v`, `u`, `s`, `cp`, `cv`, `w` for each region; those come in when a
//! caller needs them.
//!
//! # Units
//!
//! Pressure MPa, temperature K, specific enthalpy kJ/kg.

use super::region1::dgammadtau1_pt;
use super::region2::dgammadtau2_pt;
use super::region4::{p_b13_sat, tsat_p, P_CRIT, T_MIN};

/// Specific gas constant for water, kJ/(kg·K), as the reference declares it.
const R: f64 = 0.461_526;

/// Region 1 reducing temperature, K.
const TSTAR1: f64 = 1386.0;

/// Region 2 reducing temperature, K.
const TSTAR2: f64 = 540.0;

/// `h = h1_pT(p, T)` — specific enthalpy of **compressed liquid** (region 1).
///
/// # Arguments
///
/// - `p` — pressure, **MPa**.
/// - `t` — temperature, **K**.
///
/// Region 1 is valid for 273.15 K ≤ T ≤ 623.15 K up to 100 MPa, on the liquid
/// side of the saturation line. **The reference does not check this**, and
/// neither does this function: it evaluates the region-1 equation wherever it
/// is asked, so a caller outside the region gets an extrapolation rather than a
/// `NaN`. The region tests live in the callers.
///
/// # Returns
///
/// Specific enthalpy, **kJ/kg**.
pub fn h1_pt(p: f64, t: f64) -> f64 {
    R * TSTAR1 * dgammadtau1_pt(p, t)
}

/// `h = h2_pT(p, T)` — specific enthalpy of **superheated vapour** (region 2).
///
/// # Arguments
///
/// - `p` — pressure, **MPa**.
/// - `t` — temperature, **K**.
///
/// Same absence of range checking as [`h1_pt`].
///
/// # Returns
///
/// Specific enthalpy, **kJ/kg**.
pub fn h2_pt(p: f64, t: f64) -> f64 {
    R * TSTAR2 * dgammadtau2_pt(p, t)
}

/// `h = hL_p(p)` — specific enthalpy of **saturated liquid**, from pressure.
///
/// # Arguments
///
/// - `p` — pressure, **MPa**, on `[611.657e-6, 22.064]`.
///
/// # Returns
///
/// Specific enthalpy, **kJ/kg**, or `NaN` outside the range this translation
/// covers — see below.
///
/// # Partial: region 4b needs region 3, which is not translated
///
/// The reference splits the saturation line at the region 1/3 boundary,
/// `p_B13sat = 16.5291643 MPa`:
///
/// - **below it** (region 4a) the saturated liquid is region 1, so
///   `hL = h1_pT(p, Tsat(p))`;
/// - **above it** (region 4b) it is region 3, so `hL = h3_rhoT(1/vL_p(p),
///   Tsat(p))`.
///
/// Region 3 is not translated, so 4b returns `NaN` here where the reference
/// returns a number. This is a **real gap**, and the reason it is acceptable
/// for now is that both BEDOK operating points sit below the boundary — a PWR
/// at 15.5 MPa and a BWR at 7 MPa — so the benchmark cases never reach 4b. A
/// caller above 16.53 MPa gets `NaN`, which is loud rather than silent.
///
/// `NaN` outside the saturation line altogether is the reference's own
/// behaviour, not a gap.
pub fn hl_p(p: f64) -> f64 {
    let p_min = super::region4::psat_t(T_MIN);
    if !(p_min..=P_CRIT).contains(&p) {
        return f64::NAN;
    }
    if p <= p_b13_sat() {
        h1_pt(p, tsat_p(p))
    } else {
        // Region 4b — needs region 3. See the doc comment.
        f64::NAN
    }
}

/// `h = hV_p(p)` — specific enthalpy of **saturated vapour**, from pressure.
///
/// # Arguments
///
/// - `p` — pressure, **MPa**, on `[611.657e-6, 22.064]`.
///
/// # Returns
///
/// Specific enthalpy, **kJ/kg**, or `NaN` above the region 1/3 boundary
/// pressure — the same region-3 gap [`hl_p`] documents, with region 2 standing
/// in for region 1 on the vapour side.
pub fn hv_p(p: f64) -> f64 {
    let p_min = super::region4::psat_t(T_MIN);
    if !(p_min..=P_CRIT).contains(&p) {
        return f64::NAN;
    }
    if p <= p_b13_sat() {
        h2_pt(p, tsat_p(p))
    } else {
        f64::NAN
    }
}

/// `v = v1_pT(p, T)` — specific volume of compressed liquid (region 1), m³/kg.
///
/// # Arguments
///
/// - `p` — pressure, **MPa**. - `t` — temperature, **K**.
///
/// # The reducing pressure is 16.53 MPa, not 1
///
/// Region 1 reduces pressure by `pstar = 16.53 MPa`, where region 2 uses 1 MPa.
/// The `1e-3` converts `kJ/m³` to `MPa`. Getting either wrong scales the answer
/// by a large constant factor, so both are spelled out here.
///
/// # Returns
///
/// Specific volume, **m³/kg**. Note this is SI, unlike the cm-g-s units the
/// rest of BEDOK works in — a caller wanting g/cm³ takes `1/(1000*v)`.
pub fn v1_pt(p: f64, t: f64) -> f64 {
    const PSTAR1: f64 = 16.53;
    1e-3 * R * t / PSTAR1 * super::region1::dgammadpi1_pt(p, t)
}

/// `v = v2_pT(p, T)` — specific volume of superheated vapour (region 2), m³/kg.
///
/// As [`v1_pt`], but reducing by `pstar = 1 MPa`.
pub fn v2_pt(p: f64, t: f64) -> f64 {
    const PSTAR2: f64 = 1.0;
    1e-3 * R * t / PSTAR2 * super::region2::dgammadpi2_pt(p, t)
}

/// `cp = cp1_pT(p, T)` — isobaric specific heat of compressed liquid,
/// kJ/(kg·K).
pub fn cp1_pt(p: f64, t: f64) -> f64 {
    let tau = TSTAR1 / t;
    -R * tau * tau * super::region1::dgammadtautau1_pt(p, t)
}

/// `cp = cp2_pT(p, T)` — isobaric specific heat of superheated vapour,
/// kJ/(kg·K).
pub fn cp2_pt(p: f64, t: f64) -> f64 {
    let tau = TSTAR2 / t;
    -R * tau * tau * super::region2::dgammadtautau2_pt(p, t)
}

/// `v = vL_p(p)` — specific volume of **saturated liquid**, m³/kg.
///
/// Same region-4a/4b split, and the same region-3 gap above 16.5292 MPa, as
/// [`hl_p`].
pub fn vl_p(p: f64) -> f64 {
    let p_min = super::region4::psat_t(T_MIN);
    if !(p_min..=P_CRIT).contains(&p) {
        return f64::NAN;
    }
    if p <= p_b13_sat() {
        v1_pt(p, tsat_p(p))
    } else {
        f64::NAN
    }
}

/// `v = vV_p(p)` — specific volume of **saturated vapour**, m³/kg.
///
/// As [`vl_p`], through region 2.
pub fn vv_p(p: f64) -> f64 {
    let p_min = super::region4::psat_t(T_MIN);
    if !(p_min..=P_CRIT).contains(&p) {
        return f64::NAN;
    }
    if p <= p_b13_sat() {
        v2_pt(p, tsat_p(p))
    } else {
        f64::NAN
    }
}

/// `hfg = hV_p(p) - hL_p(p)` — the latent heat of vaporisation, kJ/kg.
///
/// Not a function of the reference — the MATLAB writes the difference out at
/// each call site. Named here because the thermal hydraulics uses it often
/// enough that the subtraction is worth a name, and because it makes the shared
/// `NaN` range of its two operands explicit.
pub fn hfg_p(p: f64) -> f64 {
    hv_p(p) - hl_p(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `h1_pT` against IAPWS-IF97 Table 5.
    ///
    /// # Methodology
    ///
    /// The same three region-1 states [`crate::iapws_if97::region1`]'s own test
    /// uses, but through the dimensioned wrapper rather than the raw
    /// derivative — so this checks the `R * Tstar` scaling, which that test
    /// applies by hand. Pass criterion 1e-8 relative.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Relative deviation 1.859e-10, 1.443e-9 and 9.966e-11 at the three
    /// states — identical to the region-1 module's own figures, confirming the
    /// `R * Tstar` scaling adds no error.
    #[test]
    fn liquid_enthalpy_matches_published_values() {
        let cases = [
            (3.0, 300.0, 0.115_331_273e3),
            (80.0, 300.0, 0.184_142_828e3),
            (3.0, 500.0, 0.975_542_239e3),
        ];
        for (p, t, h_ref) in cases {
            let h = h1_pt(p, t);
            eprintln!(
                "h1_pT({p} MPa, {t} K) = {h}, expected {h_ref}, rel_err {:.3e}",
                (h - h_ref).abs() / h_ref
            );
            assert!((h - h_ref).abs() / h_ref < 1e-8, "got {h}, want {h_ref}");
        }
    }

    /// `h2_pT` against IAPWS-IF97 Table 15.
    ///
    /// # Methodology
    ///
    /// The three published region-2 states, through the dimensioned wrapper.
    /// Pass criterion 1e-8 relative.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Relative deviation 3.294e-10, 1.119e-9 and 1.841e-9 at the three states,
    /// against a 1e-8 pass criterion.
    #[test]
    fn vapour_enthalpy_matches_published_values() {
        let cases = [
            (0.0035, 300.0, 0.254_991_145e4),
            (0.0035, 700.0, 0.333_568_375e4),
            (30.0, 700.0, 0.263_149_474e4),
        ];
        for (p, t, h_ref) in cases {
            let h = h2_pt(p, t);
            eprintln!(
                "h2_pT({p} MPa, {t} K) = {h}, expected {h_ref}, rel_err {:.3e}",
                (h - h_ref).abs() / h_ref
            );
            assert!((h - h_ref).abs() / h_ref < 1e-8, "got {h}, want {h_ref}");
        }
    }

    /// Saturated enthalpies at 1 atm are the right magnitude — a units and
    /// wiring check on the full chain, not a precision verification.
    ///
    /// # Methodology
    ///
    /// This exercises `hL_p -> Tsat_p -> h1_pT` (and the region-2 equivalent)
    /// end to end, which none of the table tests above do — they each check one
    /// link. The comparison is against the **approximate** figures every
    /// steam table carries near 1 atm: `hL ~ 419 kJ/kg`, `hV ~ 2676 kJ/kg`,
    /// `hfg ~ 2257 kJ/kg`.
    ///
    /// **Pass criterion 0.5% relative, and the looseness is deliberate.** These
    /// reference values are recalled round numbers, not transcribed from a
    /// specific published table, so a tight tolerance here would be asserting
    /// more confidence in the constants than they carry. The precision claim
    /// for this module rests on the IAPWS Tables 5, 15, 35 and 36 tests, which
    /// agree to ~1e-9; this test exists to catch a units error or a
    /// miswired call, which 0.5% is ample for.
    ///
    /// One subtlety worth stating, because it caught this test out when it was
    /// first written with a 1e-4 tolerance: the familiar `hL = 419.04 kJ/kg` is
    /// the value at **exactly 100 °C**, whereas water at 1 atm saturates at
    /// 99.97 °C. The two differ by about 0.05 kJ/kg, which is 1.2e-4 relative —
    /// enough to fail a 1e-4 test for an entirely correct implementation.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `hL = 418.9907`, `hV = 2675.5315`, `hfg = 2256.5407` kJ/kg, at
    /// `Tsat(0.101325 MPa) = 373.1243 K` — which is **99.974 °C**, the normal
    /// boiling point to five figures, and a good independent check on the
    /// saturation line at a state no published table above touches.
    ///
    /// All three enthalpies land within 0.03% of the recalled figures, and
    /// `hfg` in particular reproduces the classic 2257 kJ/kg latent heat to
    /// 0.02% — a confirmation that the region-1 and region-2 branches are
    /// consistent with each other across the saturation line, since they come
    /// from separate coefficient tables.
    ///
    /// The 100 °C caveat above is borne out numerically: `hL` at 99.974 °C
    /// comes out 0.05 kJ/kg below the 419.04 kJ/kg quoted at exactly 100 °C,
    /// which is the 1.2e-4 relative offset predicted.
    #[test]
    fn saturated_enthalpies_at_one_atmosphere_are_the_right_magnitude() {
        let p = 0.101_325;
        let hl = hl_p(p);
        let hv = hv_p(p);
        let hfg = hfg_p(p);
        eprintln!(
            "at 1 atm (Tsat = {} K): hL = {hl}, hV = {hv}, hfg = {hfg} kJ/kg",
            tsat_p(p)
        );

        assert!((hl - 419.0).abs() / 419.0 < 5e-3, "hL = {hl}");
        assert!((hv - 2676.0).abs() / 2676.0 < 5e-3, "hV = {hv}");
        assert!((hfg - 2257.0).abs() / 2257.0 < 5e-3, "hfg = {hfg}");
    }

    /// The saturated-liquid and saturated-vapour curves converge as the
    /// critical point is approached — `hfg` goes to zero.
    ///
    /// # Methodology
    ///
    /// `hfg` is evaluated at 1, 5, 10 and 16 MPa. It must be positive and
    /// strictly decreasing: the two branches of the saturation dome meet at the
    /// critical point, so the latent heat must shrink monotonically towards it.
    /// This is a shape check on the pair, independent of any tabulated value.
    ///
    /// The sweep stops at 16 MPa because 16.53 MPa is where region 4b begins
    /// and [`hl_p`] returns `NaN` — see its doc comment.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `hfg` = 2014.44, 1639.73, 1317.61 and 931.13 kJ/kg at 1, 5, 10 and
    /// 16 MPa — positive and strictly decreasing, as the saturation dome
    /// requires. The 16 MPa value is already 59% below the 1 MPa one, with the
    /// critical point only 6 MPa further on.
    #[test]
    fn the_latent_heat_shrinks_towards_the_critical_point() {
        let mut previous = f64::INFINITY;
        for p in [1.0, 5.0, 10.0, 16.0] {
            let hfg = hfg_p(p);
            eprintln!("hfg({p} MPa) = {hfg} kJ/kg");
            assert!(hfg > 0.0, "hfg at {p} MPa is {hfg}");
            assert!(hfg < previous, "hfg at {p} MPa did not decrease");
            previous = hfg;
        }
    }

    /// Region 4b is `NaN`, loudly, because region 3 is not translated.
    ///
    /// Pins the documented gap so that translating region 3 later has to
    /// deliberately change this test rather than silently start returning
    /// numbers.
    #[test]
    fn above_the_region_boundary_is_nan_pending_region_three() {
        assert!(p_b13_sat() < 17.0 && p_b13_sat() > 16.0);
        assert!(hl_p(18.0).is_nan(), "region 4b should be NaN for now");
        assert!(hv_p(18.0).is_nan(), "region 4b should be NaN for now");
        // But the benchmark operating points are fine.
        assert!(hl_p(15.5).is_finite(), "PWR pressure must work");
        assert!(hl_p(7.0).is_finite(), "BWR pressure must work");
    }
}
