//! Region 1 — the Gibbs free energy `gamma` and its derivatives.
//!
//! Region 1 is the subcooled/compressed liquid: `273.15 K <= T <= 623.15 K`
//! with `p` between the saturation line and 100 MPa.
//!
//! # Provenance
//!
//! Translated from `IAPWS_IF97.m` by Mark Mikofski — see the crate `NOTICE`
//! for the full BSD-2-Clause terms this translation is made under, and
//! [`super`] for the module-level provenance block.

/// Number of terms in the region-1 residual sum. `Nterms` in the reference.
const NTERMS: usize = 34;

/// Reducing pressure for region 1, MPa. `pstar` in the reference.
const PSTAR: f64 = 16.53;

/// Reducing temperature for region 1, K. `Tstar` in the reference.
const TSTAR: f64 = 1386.0;

/// Pressure exponents `I`.
///
/// The reference re-declares this identical table inside each of the five
/// functions below. It is hoisted here because it is the same data — no
/// arithmetic is reordered by sharing it, so the no-optimisation rule in
/// the crate README's "Translation policy" is not engaged.
const I: [f64; NTERMS] = [
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0,
    3.0, 3.0, 3.0, 4.0, 4.0, 4.0, 5.0, 8.0, 8.0, 21.0, 23.0, 29.0, 30.0, 31.0, 32.0,
];

/// Temperature exponents `J`.
const J: [f64; NTERMS] = [
    -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, -9.0, -7.0, -1.0, 0.0, 1.0, 3.0, -3.0, 0.0, 1.0, 3.0,
    17.0, -4.0, 0.0, 6.0, -5.0, -2.0, 10.0, -8.0, -11.0, -6.0, -29.0, -31.0, -38.0, -39.0, -40.0,
    -41.0,
];

/// Coefficients `n`.
const N: [f64; NTERMS] = [
    0.146329712131670,
    -0.845481871691140,
    -3.75636036720400,
    3.38551691683850,
    -0.957919633878720,
    0.157720385132280,
    -0.0166164171995010,
    0.000812146299835680,
    0.000283190801238040,
    -0.000607063015658740,
    -0.0189900682184190,
    -0.0325297487705050,
    -0.0218417171754140,
    -5.28383579699300e-05,
    -0.000471843210732670,
    -0.000300017807930260,
    4.76613939069870e-05,
    -4.41418453308460e-06,
    -7.26949962975940e-16,
    -3.16796448450540e-05,
    -2.82707979853120e-06,
    -8.52051281201030e-10,
    -2.24252819080000e-06,
    -6.51712228956010e-07,
    -1.43417299379240e-13,
    -4.05169968601170e-07,
    -1.27343017416410e-09,
    -1.74248712306340e-10,
    -6.87621312955310e-19,
    1.44783078285210e-20,
    2.63357816627950e-23,
    -1.19476226400710e-23,
    1.82280945814040e-24,
    -9.35370872924580e-26,
];

/// Reduce `(p, T)` to region 1's dimensionless `(pi, tau)`.
///
/// `pi = p / 16.53 MPa`, `tau = 1386 K / T`.
fn reduce(p: f64, t: f64) -> (f64, f64) {
    (p / PSTAR, TSTAR / t)
}

/// `dgammadtau1_pT(p, T)` — first derivative of `gamma` with respect to `tau`.
///
/// # Arguments
/// - `p` — pressure, MPa.
/// - `t` — temperature, K.
///
/// # Returns
/// Dimensionless. Feeds `h1_pT` as `h = R * Tstar * dgammadtau`.
pub fn dgammadtau1_pt(p: f64, t: f64) -> f64 {
    let (pi, tau) = reduce(p, t);
    let mut total = 0.0;
    // Summed in table order, matching MATLAB's `sum(..., 2)` across the 34
    // columns. Accumulation order is part of the result at this precision, so
    // it is not reordered.
    for k in 0..NTERMS {
        total += N[k] * (7.1 - pi).powf(I[k]) * J[k] * (tau - 1.222).powf(J[k] - 1.0);
    }
    total
}

/// `dgammadpi1_pT(p, T)` — first derivative of `gamma` with respect to `pi`.
///
/// # Arguments
/// - `p` — pressure, MPa.
/// - `t` — temperature, K.
///
/// # Returns
/// Dimensionless. Feeds `v1_pT` as `v = 1e-3 * R * T / pstar * dgammadpi`.
pub fn dgammadpi1_pt(p: f64, t: f64) -> f64 {
    let (pi, tau) = reduce(p, t);
    let mut total = 0.0;
    for k in 0..NTERMS {
        total += -N[k] * I[k] * (7.1 - pi).powf(I[k] - 1.0) * (tau - 1.222).powf(J[k]);
    }
    total
}

/// `dgammadtautau1_pT(p, T)` — second derivative with respect to `tau`.
///
/// # Arguments
/// - `p` — pressure, MPa.
/// - `t` — temperature, K.
///
/// # Returns
/// Dimensionless. Feeds `cp1_pT` as `cp = -R * tau^2 * dgammadtautau`.
pub fn dgammadtautau1_pt(p: f64, t: f64) -> f64 {
    let (pi, tau) = reduce(p, t);
    let mut total = 0.0;
    for k in 0..NTERMS {
        total +=
            N[k] * (7.1 - pi).powf(I[k]) * J[k] * (J[k] - 1.0) * (tau - 1.222).powf(J[k] - 2.0);
    }
    total
}

/// `dgammadpipi1_pT(p, T)` — second derivative with respect to `pi`.
///
/// # Arguments
/// - `p` — pressure, MPa.
/// - `t` — temperature, K.
///
/// # Returns
/// Dimensionless. Feeds `kappaT1_pT`, the isothermal compressibility.
pub fn dgammadpipi1_pt(p: f64, t: f64) -> f64 {
    let (pi, tau) = reduce(p, t);
    let mut total = 0.0;
    for k in 0..NTERMS {
        total += N[k] * I[k] * (I[k] - 1.0) * (7.1 - pi).powf(I[k] - 2.0)
            * (tau - 1.222).powf(J[k]);
    }
    total
}

/// `dgammadpitau1_pT(p, T)` — mixed second derivative.
///
/// # Arguments
/// - `p` — pressure, MPa.
/// - `t` — temperature, K.
///
/// # Returns
/// Dimensionless. Feeds `alphav1_pT`, the isobaric cubic expansion
/// coefficient.
pub fn dgammadpitau1_pt(p: f64, t: f64) -> f64 {
    let (pi, tau) = reduce(p, t);
    let mut total = 0.0;
    for k in 0..NTERMS {
        total += -N[k] * I[k] * (7.1 - pi).powf(I[k] - 1.0) * J[k]
            * (tau - 1.222).powf(J[k] - 1.0);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Specific gas constant, kJ/kg/K, as the reference declares it.
    const R: f64 = 0.461526;

    /// IAPWS-IF97 Table 5 verification values for region 1.
    ///
    /// # Methodology
    ///
    /// The released formulation publishes three `(p, T)` states with reference
    /// values for `v`, `h`, `cp` and others. Here `h = R * Tstar * dgammadtau`
    /// and `v = 1e-3 * R * T / pstar * dgammadpi` are reconstructed from the
    /// derivatives in this module and compared against the published values.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Run on rustc 1.97.1, `stable-x86_64-pc-windows-gnu`, release profile.
    /// Relative deviation from the published values:
    ///
    /// | State | `h` | `v` |
    /// |---|---|---|
    /// | 3 MPa, 300 K | 1.859e-10 | 3.126e-10 |
    /// | 80 MPa, 300 K | 1.443e-9 | 2.227e-11 |
    /// | 3 MPa, 500 K | 9.966e-11 | 2.810e-9 |
    ///
    /// Worst case 2.810e-9, against a 1e-8 pass criterion.
    ///
    /// **Interpretation.** The published table gives its reference values to 9
    /// significant figures, so agreement to ~1e-9 is agreement *at the table's
    /// own stated precision* — the residual is consistent with the rounding of
    /// the printed reference values rather than with any error in this
    /// implementation. This verifies the region-1 `gamma` derivatives against
    /// the standard; it says nothing about the regions not yet translated.
    #[test]
    fn region1_matches_published_verification_values() {
        // IAPWS-IF97 Table 5: (p [MPa], T [K], v [m^3/kg], h [kJ/kg])
        let cases = [
            (3.0, 300.0, 0.100215168e-2, 0.115331273e3),
            (80.0, 300.0, 0.971180894e-3, 0.184142828e3),
            (3.0, 500.0, 0.120241800e-2, 0.975542239e3),
        ];

        for (p, t, v_ref, h_ref) in cases {
            let h = R * TSTAR * dgammadtau1_pt(p, t);
            let v = 1e-3 * R * t / PSTAR * dgammadpi1_pt(p, t);

            eprintln!(
                "region1 ({p} MPa, {t} K): rel_err h = {:.3e}, v = {:.3e}",
                (h - h_ref).abs() / h_ref,
                (v - v_ref).abs() / v_ref
            );

            assert!(
                (h - h_ref).abs() / h_ref < 1e-8,
                "h at ({p} MPa, {t} K): got {h}, expected {h_ref}"
            );
            assert!(
                (v - v_ref).abs() / v_ref < 1e-8,
                "v at ({p} MPa, {t} K): got {v}, expected {v_ref}"
            );
        }
    }

    /// `cp` at the same three states, from the second `tau` derivative.
    ///
    /// # Methodology
    ///
    /// `cp = -R * tau^2 * dgammadtautau`, compared against the IAPWS-IF97
    /// Table 5 reference values. Pass criterion 1e-8 relative.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Relative deviation: 9.748e-10 at (3 MPa, 300 K), 8.819e-11 at
    /// (80 MPa, 300 K), 4.535e-10 at (3 MPa, 500 K). Worst case 9.748e-10.
    ///
    /// **Interpretation.** As above — at the published table's own 9-figure
    /// precision, so the second temperature derivative is verified against the
    /// standard over the three tabulated states.
    #[test]
    fn region1_specific_heat_matches_published_values() {
        let cases = [
            (3.0, 300.0, 0.417301218e1),
            (80.0, 300.0, 0.401008987e1),
            (3.0, 500.0, 0.465580682e1),
        ];

        for (p, t, cp_ref) in cases {
            let tau = TSTAR / t;
            let cp = -R * tau.powi(2) * dgammadtautau1_pt(p, t);
            eprintln!(
                "region1 ({p} MPa, {t} K): rel_err cp = {:.3e}",
                (cp - cp_ref).abs() / cp_ref
            );
            assert!(
                (cp - cp_ref).abs() / cp_ref < 1e-8,
                "cp at ({p} MPa, {t} K): got {cp}, expected {cp_ref}"
            );
        }
    }
}
