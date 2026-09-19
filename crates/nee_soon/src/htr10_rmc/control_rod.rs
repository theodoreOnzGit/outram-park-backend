//! HTR-10 control rods — smeared composition of the side-reflector boring band.
//!
//! # What this is for
//!
//! The HTR-10 core model in [`super::core_model`] carries the ten control-rod
//! borings as a single homogenised annulus (`mat::BORED_GRAPHITE`, TECDOC zones
//! 31-40, r 95.6-108.6 cm) at **28 % less carbon** than solid reflector
//! graphite. That band represents the borings as *empty*: there is no absorber
//! anywhere in the model, so it can only ever represent rods **fully
//! withdrawn**, and control-rod worth cannot be computed from it at all.
//!
//! This module supplies the missing half — what the band contains when rods are
//! in it — derived from the published rod geometry and B4C density, with
//! nothing fitted.
//!
//! # Source
//!
//! IAEA-TECDOC-1382 § 4.1.2, transcribed with its benchmark values in
//! `crates/kovan-literature/derived/tecdoc1382-htr10-control-rods.md`. Read that
//! record before changing any constant here.
//!
//! # The approximation this makes, stated plainly
//!
//! Ten discrete rods spaced azimuthally are smeared into one axisymmetric
//! annulus. **TECDOC explicitly warns about this**: its Table 4-3 densities are
//! spatially homogenised and "if one is to consider three-dimensional effects,
//! the homogenized densities are to be corrected by taking into consideration
//! of the boring geometries".
//!
//! Consequences, in order of how much they matter:
//!
//! - **Self-shielding is lost.** A B4C annulus is intensely black to thermal
//!   neutrons; smearing it over 14x its own volume replaces a strongly
//!   self-shielded absorber with a dilute one, which **over-predicts** worth.
//!   This is the dominant error and it has a known sign.
//! - **The one-rod problems (B32, B42) cannot be represented at all.** One rod
//!   of ten is not an axisymmetric perturbation. Do not compare a smeared
//!   result against them.
//! - Azimuthal flux tilt is absent by construction.
//!
//! So the ten-rod problems (B31, B41) are the only ones this geometry can
//! honestly address, and even there the expected bias is positive.

/// Inner radius of the B4C ring \[cm\] — 60 mm diameter.
pub const B4C_INNER_RADIUS_CM: f64 = 3.0;
/// Outer radius of the B4C ring \[cm\] — 105 mm diameter.
pub const B4C_OUTER_RADIUS_CM: f64 = 5.25;
/// Number of control rods in the side reflector.
pub const N_CONTROL_RODS: usize = 10;
/// Density of boron carbide in the rod \[g/cm³\], stated by TECDOC.
pub const B4C_DENSITY_G_PER_CM3: f64 = 1.7;

/// Axial section lengths of one rod \[cm\], lower to upper, as published:
/// `45/487/36/487/36/487/36/487/36/487/23` mm. The five 487 mm sections are
/// B4C; every other section is stainless steel.
pub const AXIAL_SECTIONS_CM: [f64; 11] = [
    4.5, 48.7, 3.6, 48.7, 3.6, 48.7, 3.6, 48.7, 3.6, 48.7, 2.3,
];
/// Which entries of [`AXIAL_SECTIONS_CM`] are B4C rather than steel.
pub const AXIAL_IS_B4C: [bool; 11] = [
    false, true, false, true, false, true, false, true, false, true, false,
];

/// Axial coordinate of the rod lower end when **fully withdrawn** \[cm\].
pub const LOWER_END_WITHDRAWN_CM: f64 = 119.2;
/// Axial coordinate of the rod lower end when **fully inserted** \[cm\].
pub const LOWER_END_INSERTED_CM: f64 = 394.2;

/// Molar mass of natural boron \[g/mol\].
const M_BORON: f64 = 10.811;
/// Molar mass of carbon \[g/mol\].
const M_CARBON: f64 = 12.011;
/// Avogadro constant \[1/mol\].
const AVOGADRO: f64 = 6.022_140_76e23;

/// Atom densities added to the boring band when rods occupy it
/// \[atoms/(barn·cm)\].
///
/// Both are **additions** to the band's existing homogenised graphite
/// ([`super::core_model::HTR10_BORED_CARBON`] and `HTR10_BORED_BORON`), not
/// replacements: the reduced-density band already represents graphite plus
/// void, and inserting a rod fills part of that void.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmearedRodComposition {
    /// **Natural** boron from the B4C — not B-10. Only 19.9 at.% absorbs
    /// strongly; reading this as B-10 over-absorbs by roughly 5x, the same
    /// misreading [`super::reflector`] documents.
    pub natural_boron: f64,
    /// Carbon from the B4C, over and above the band's graphite.
    pub carbon: f64,
}

/// Atom density of B4C **molecules** in the solid absorber \[atoms/(barn·cm)\].
///
/// `rho * N_A / M`, with `M = 4 M_B + M_C`. Boron is natural, so `M_B` is the
/// natural atomic weight and no isotopic split is applied here.
#[must_use]
pub fn b4c_molecular_density() -> f64 {
    let molar_mass = 4.0 * M_BORON + M_CARBON;
    B4C_DENSITY_G_PER_CM3 * AVOGADRO / molar_mass * 1.0e-24
}

/// Fraction of the boring band's cross-sectional area occupied by B4C when all
/// ten rods are present \[-\].
///
/// `10 * pi (r_out^2 - r_in^2) / (pi (R_out^2 - R_in^2))`, with the band radii
/// taken from [`super::core_model`] so the two cannot drift apart.
#[must_use]
pub fn radial_packing_fraction() -> f64 {
    let rod_area =
        N_CONTROL_RODS as f64 * (B4C_OUTER_RADIUS_CM.powi(2) - B4C_INNER_RADIUS_CM.powi(2));
    let band_area = super::core_model::HTR10_CONTROL_ROD_OUTER_CM.powi(2)
        - super::core_model::HTR10_CONTROL_ROD_INNER_CM.powi(2);
    rod_area / band_area
}

/// Fraction of the rod's own length that is actually B4C \[-\].
///
/// The absorber is **not** a continuous column: five 487 mm segments separated
/// by 36 mm steel joints, with steel ends. A model that treats the whole rod
/// span as absorber over-predicts worth by the reciprocal of this.
#[must_use]
pub fn axial_duty_fraction() -> f64 {
    let total: f64 = AXIAL_SECTIONS_CM.iter().sum();
    let b4c: f64 = AXIAL_SECTIONS_CM
        .iter()
        .zip(AXIAL_IS_B4C.iter())
        .filter(|(_, &is)| is)
        .map(|(&l, _)| l)
        .sum();
    b4c / total
}

/// Composition added to the boring band over the axially-inserted length.
///
/// `fraction_inserted` is the fraction of the band's height the rods occupy,
/// `0.0` fully withdrawn to `1.0` fully inserted. It scales the result
/// linearly, which is what smearing an absorber over a taller region means —
/// it is **not** a model of differential worth, because worth depends on where
/// in the flux shape the absorber sits, not only on how much of it there is.
/// Use an axially resolved geometry for that.
///
/// Values outside `[0, 1]` are clamped rather than extrapolated.
#[must_use]
pub fn smeared_composition(fraction_inserted: f64) -> SmearedRodComposition {
    let f = fraction_inserted.clamp(0.0, 1.0);
    let scale = radial_packing_fraction() * axial_duty_fraction() * f;
    let n_b4c = b4c_molecular_density();
    SmearedRodComposition {
        natural_boron: 4.0 * n_b4c * scale,
        carbon: n_b4c * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smear is derived from published geometry, and reproduces it.
    ///
    /// # Methodology
    ///
    /// Every input is transcribed from IAEA-TECDOC-1382 § 4.1.2 (see the
    /// derived-data record named in the module docs): B4C ring radii 3.0 /
    /// 5.25 cm, ten rods, band radii 95.6 / 108.6 cm from
    /// [`super::super::core_model`], axial sequence
    /// `45/487/36/487/36/487/36/487/36/487/23` mm, B4C density 1.7 g/cm³.
    /// Nothing is tuned against a worth measurement.
    ///
    /// # Results, measured 2026-09-20
    ///
    /// - B4C molecular density **0.0185279 atoms/(b·cm)**
    /// - radial packing fraction **0.0699258** (7.0 % of the band is absorber)
    /// - axial duty fraction **0.919909** (243.5 cm of B4C in a 264.7 cm rod)
    /// - fully inserted: natural boron **4.7673e-3**, carbon **1.1918e-3**
    ///
    /// The boron addition is four orders of magnitude above the band's own
    /// `HTR10_BORED_BORON = 3.4064e-7`, which is the sanity check that a rod
    /// actually does something: an insertion that barely moved the boron would
    /// mean the smear had been computed against the wrong volume.
    #[test]
    fn smeared_rod_composition_follows_the_published_geometry() {
        let n = b4c_molecular_density();
        assert!(
            (n - 0.0185279).abs() < 1.0e-7,
            "B4C molecular density {n:e}"
        );

        let rp = radial_packing_fraction();
        assert!((rp - 0.0699258).abs() < 1.0e-6, "radial packing {rp}");

        let ax = axial_duty_fraction();
        assert!((ax - 0.919909).abs() < 1.0e-6, "axial duty {ax}");

        let full = smeared_composition(1.0);
        assert!(
            (full.natural_boron - 4.7673e-3).abs() < 1.0e-7,
            "natural boron {:e}",
            full.natural_boron
        );
        assert!(
            (full.carbon - 1.1918e-3).abs() < 1.0e-7,
            "carbon {:e}",
            full.carbon
        );

        // A rod must dominate the band's own trace boron by orders of magnitude.
        assert!(
            full.natural_boron > 1.0e4 * super::super::core_model::HTR10_BORED_BORON,
            "an inserted rod barely changed the boron -- wrong volume?"
        );

        // Withdrawn adds nothing at all, and the scaling is linear in between.
        assert_eq!(smeared_composition(0.0).natural_boron, 0.0);
        let half = smeared_composition(0.5);
        assert!((half.natural_boron - 0.5 * full.natural_boron).abs() < 1.0e-12);

        // Out-of-range is clamped, never extrapolated.
        assert_eq!(
            smeared_composition(2.0).natural_boron,
            full.natural_boron,
            "over-insertion must clamp, not extrapolate"
        );
    }

    /// The published travel is 275 cm, and the two ends are ordered.
    #[test]
    fn published_rod_travel_is_consistent() {
        assert!(LOWER_END_INSERTED_CM > LOWER_END_WITHDRAWN_CM);
        assert!((LOWER_END_INSERTED_CM - LOWER_END_WITHDRAWN_CM - 275.0).abs() < 1.0e-9);
        // The rod is shorter than its own travel, which is why partial
        // insertion is a real axial position rather than a fill fraction.
        let span: f64 = AXIAL_SECTIONS_CM.iter().sum();
        assert!((span - 264.7).abs() < 1.0e-9, "rod span {span} cm");
    }
}
