// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The Kalbach-86 slope systematics (`bach`).

use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
use crate::NjoyError;

/// Kalbach-Mann `a(E, E')` slope parameter (dimensionless).
///
/// Faithful port of NJOY2016 `bach` (`groupr.f90:8812-8932`), the Kalbach-86
/// systematics. Given the projectile, emitted-particle, and target `ZA` codes
/// (`ZA = 1000 Z + A`; `iza1i = 0` is treated as an incident neutron), the
/// incident energy `e` \[eV\], and the CM secondary energy `ep` \[eV\], returns
/// the slope `a` of the Kalbach angular form
/// `f(mu) ~ a (cosh(a mu) + r sinh(a mu))`.
///
/// Natural-element target codes (`Z000`) are mapped to their dominant isotope,
/// as NJOY does. Separation energies use the Kalbach mass formula (constants
/// `c1..c6`, `s2..s5`). For an incident neutron (`iza1i = 0`) the extra
/// low-energy `d1 / sqrt(E')` enhancement factor is applied.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the target's dominant isotope is unknown
/// (mirrors NJOY's `error('bach', 'dominant isotope not known ...')`).
///
/// Valid for `e > 0`, `ep > 0`.
pub fn bach(iza1i: i32, iza2: i32, izat: i32, e: f64, ep: f64) -> Result<f64, NjoyError> {
    // Kalbach-86 constants (groupr.f90:8826-8847).
    const THIRD: f64 = 0.333_333_333;
    const TWOTH: f64 = 0.666_666_667;
    const FOURTH: f64 = 1.333_333_33;
    const C1: f64 = 15.68;
    const C2: f64 = -28.07;
    const C3: f64 = -18.56;
    const C4: f64 = 33.22;
    const C5: f64 = -0.717;
    const C6: f64 = 1.211;
    const S2: f64 = 2.22;
    const S3: f64 = 8.48;
    const S4: f64 = 7.72;
    const S5: f64 = 28.3;
    const BRK1: f64 = 130.0;
    const BRK2: f64 = 41.0;
    const HALF: f64 = 0.5;
    const B1: f64 = 0.04;
    const B2: f64 = 1.8e-6;
    const B3: f64 = 6.7e-7;
    const D1: f64 = 9.3;
    const TOMEV: f64 = 1.0e-6;

    // Neutron rest-mass energy in MeV (NJOY `emc2 = tomev*amassn*amu*c^2/ev`).
    let emc2 = TOMEV * AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG;

    let iza1 = if iza1i == 0 { 1 } else { iza1i };

    // Map a natural-element target to its dominant isotope (groupr.f90:8853-8875).
    let iza = match izat {
        6000 => 6012,
        12000 => 12024,
        14000 => 14028,
        16000 => 16032,
        17000 => 17035,
        19000 => 19039,
        20000 => 20040,
        22000 => 22048,
        23000 => 23051,
        24000 => 24052,
        26000 => 26056,
        28000 => 28058,
        29000 => 29063,
        31000 => 31069,
        40000 => 40090,
        42000 => 42096,
        48000 => 48112,
        49000 => 49115,
        50000 => 50120,
        63000 => 63151,
        72000 => 72178,
        74000 => 74184,
        82000 => 82208,
        other => other,
    };

    let aa = (iza.rem_euclid(1000)) as f64;
    if aa == 0.0 {
        return Err(NjoyError::EndfParse(format!(
            "bach: dominant isotope not known for {iza}"
        )));
    }
    let za = (iza / 1000) as f64;
    let ac = aa + (iza1.rem_euclid(1000)) as f64;
    let zc = za + (iza1 / 1000) as f64;
    let ab = ac - (iza2.rem_euclid(1000)) as f64;
    let zb = zc - (iza2 / 1000) as f64;
    let na = (aa - za).round();
    let nb = (ab - zb).round();
    let nc = (ac - zc).round();

    let mut sa = C1 * (ac - aa)
        + C2 * ((nc - zc).powi(2) / ac - (na - za).powi(2) / aa)
        + C3 * (ac.powf(TWOTH) - aa.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (na - za).powi(2) / aa.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - za.powi(2) / aa.powf(THIRD))
        + C6 * (zc.powi(2) / ac - za.powi(2) / aa);
    match iza1 {
        1002 => sa -= S2,
        1003 => sa -= S3,
        2003 => sa -= S4,
        2004 => sa -= S5,
        _ => {}
    }

    let mut sb = C1 * (ac - ab)
        + C2 * ((nc - zc).powi(2) / ac - (nb - zb).powi(2) / ab)
        + C3 * (ac.powf(TWOTH) - ab.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (nb - zb).powi(2) / ab.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - zb.powi(2) / ab.powf(THIRD))
        + C6 * (zc.powi(2) / ac - zb.powi(2) / ab);
    match iza2 {
        1002 => sb -= S2,
        1003 => sb -= S3,
        2003 => sb -= S4,
        2004 => sb -= S5,
        _ => {}
    }

    let ecm = aa * e / ac;
    let ea = ecm * TOMEV + sa;
    let eb = TOMEV * ep * ac / ab + sb;
    let mut x1 = eb;
    if ea > BRK1 {
        x1 = BRK1 * eb / ea;
    }
    let mut x3 = eb;
    if ea > BRK2 {
        x3 = BRK2 * eb / ea;
    }
    let fa = if iza1 == 2004 { 0.0 } else { 1.0 };
    let fb = if iza2 == 1 {
        HALF
    } else if iza2 == 2004 {
        2.0
    } else {
        1.0
    };
    let mut bb = B1 * x1 + B2 * x1.powi(3) + B3 * fa * fb * x3.powi(4);
    if iza1i == 0 {
        let mut fact = D1 / (ep * TOMEV).sqrt();
        if fact < 1.0 {
            fact = 1.0;
        }
        if fact > 4.0 {
            fact = 4.0;
        }
        bb *= (TOMEV * e / (2.0 * emc2)).sqrt() * fact;
    }
    Ok(bb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `bach` Kalbach slope: positive, and monotonically increasing with both
    /// incident energy (at fixed E') and emission energy (at fixed E).
    ///
    /// **Methodology.** Neutron (`iza1i=0`) on Fe-56, neutron out (`iza2=1`).
    /// Evaluate `a` on a grid of incident energies (1, 5, 10, 20 MeV) at fixed
    /// E' = 1 MeV, and on a grid of E' (0.5, 1, 2, 4 MeV) at fixed E = 14 MeV.
    /// Reference: the Kalbach-86 closed form as coded in NJOY `bach`. Pass if `a`
    /// is finite, positive, and strictly increasing along both grids.
    ///
    /// **Result (2026-07-15, commit ac5adf5).**
    /// a(E=1,5,10,20 MeV; E'=1 MeV) = [0.04118, 0.09208, 0.13022, 0.18416];
    /// a(E=14 MeV; E'=0.5,1,2,4 MeV) = [0.14665, 0.15408, 0.16910, 0.19995].
    /// All positive, finite, strictly increasing along both grids.
    #[test]
    fn bach_slope_monotone_and_positive() {
        let izat = 26056;
        let e_grid = [1.0e6, 5.0e6, 10.0e6, 20.0e6];
        let mut last = f64::NEG_INFINITY;
        let mut ve = Vec::new();
        for &e in &e_grid {
            let a = bach(0, 1, izat, e, 1.0e6).unwrap();
            assert!(a.is_finite() && a > 0.0, "a({e}) = {a}");
            assert!(a > last, "not increasing in E: a={a}, last={last}");
            last = a;
            ve.push(a);
        }
        let ep_grid = [0.5e6, 1.0e6, 2.0e6, 4.0e6];
        let mut last = f64::NEG_INFINITY;
        let mut vp = Vec::new();
        for &ep in &ep_grid {
            let a = bach(0, 1, izat, 14.0e6, ep).unwrap();
            assert!(a.is_finite() && a > 0.0, "a(ep={ep}) = {a}");
            assert!(a > last, "not increasing in E': a={a}, last={last}");
            last = a;
            vp.push(a);
        }
        println!("bach a(E,E'=1MeV)={ve:?}  a(E=14MeV,E')={vp:?}");
    }

    /// `bach` maps a natural-element target code to its dominant isotope, giving
    /// the same slope as the explicit isotope code.
    ///
    /// **Methodology.** Assert `bach(0,1, 26000, ...) == bach(0,1, 26056, ...)`
    /// (natural Fe → Fe-56) at E = 14 MeV, E' = 1 MeV.
    ///
    /// **Result (2026-07-15).** Both return the identical slope (exact equality).
    #[test]
    fn bach_natural_element_maps_to_dominant_isotope() {
        let a_nat = bach(0, 1, 26000, 14.0e6, 1.0e6).unwrap();
        let a_iso = bach(0, 1, 26056, 14.0e6, 1.0e6).unwrap();
        assert_eq!(a_nat, a_iso);
    }
}
