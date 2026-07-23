//! Aqueous activity-coefficient models — bead op-v6s.15.1.
//!
//! Corrects molar concentration to thermodynamic **activity** for dissolved
//! ions, replacing the ideal (`gamma = 1`) assumption used by the v1
//! geochemistry speciation core (see the [`crate::geochemistry`] module). The
//! activity of species `i` is
//!
//! ```text
//! a_i = gamma_i * c_i
//! ```
//!
//! where `c_i` is the free concentration in **mol/L** and `gamma_i` its
//! (dimensionless) activity coefficient. In the dilute limit `I -> 0` every
//! model returns `gamma -> 1`, recovering the ideal case.
//!
//! ## Ionic strength
//!
//! All charged-species corrections are driven by the ionic strength
//!
//! ```text
//! I = 0.5 * sum_i c_i * z_i^2      [mol/L]
//! ```
//!
//! with `c_i` in mol/L and `z_i` the (signed, integer-or-real) charge number.
//!
//! ## Models (all at ~25 degrees C)
//!
//! With Debye–Hückel constants at 25 degrees C in water,
//! `A ≈ 0.5085` and `B ≈ 0.3281` (units `1 / (Angstrom * sqrt(mol/L))`):
//!
//! - **[`ActivityModel::Ideal`]** — `gamma = 1` for every species.
//! - **[`ActivityModel::DebyeHuckel`]** — extended Debye–Hückel with an optional
//!   b-dot linear term:
//!
//!   ```text
//!   log10 gamma_i = -A z_i^2 sqrt(I) / (1 + B a_i sqrt(I)) + bdot * I
//!   ```
//!
//!   where `a_i` is the ion-size (a0) parameter in Angstrom and `bdot` the
//!   optional linear term (`bdot = 0` gives plain extended Debye–Hückel;
//!   `bdot ≈ 0.041` is the classic Helgeson NaCl B-dot value). Usable to higher
//!   ionic strength than Davies when a fitted `bdot` is supplied.
//! - **[`ActivityModel::Davies`]** — parameterless, valid to `I ~ 0.5 mol/L`:
//!
//!   ```text
//!   log10 gamma_i = -A z_i^2 ( sqrt(I) / (1 + sqrt(I)) - 0.3 I )
//!   ```
//!
//! ## Human-review flags (simplifications)
//!
//! - **25 degrees C only.** `A` and `B` are fixed at their 25 degrees C water
//!   values; there is no temperature dependence. Using these at other
//!   temperatures is a modelling error until temperature-dependent constants
//!   are added.
//! - **Common ion-size parameter.** [`ActivityModel::DebyeHuckel`] carries a
//!   single `a_i` applied to every ion, rather than per-species a0 values.
//! - **Neutral-species simplification.** For the charged models a species with
//!   `z = 0` is assigned `gamma = 1` (the setchenow / "salting-out" correction
//!   for neutral aqueous species is not modelled).
//!
//! ## Provenance
//!
//! Standard aqueous-geochemistry activity models as implemented in PFLOTRAN and
//! PHREEQC. References: Debye & Hückel (1923); C. W. Davies, *Ion Association*
//! (Butterworths, 1962); Helgeson (1969) for the B-dot extension; Parkhurst &
//! Appelo, *Description of Input and Examples for PHREEQC Version 3*, USGS
//! Techniques and Methods 6-A43 (2013). These are open, published models.
//!
//! Enum dispatch, no trait objects (workspace rule).

use crate::error::PflotranError;

/// Debye–Hückel `A` constant at 25 degrees C in water, in units of
/// `1 / sqrt(mol/L)` (dimensionless coefficient on `z^2 sqrt(I)`).
const DH_A: f64 = 0.5085;

/// Debye–Hückel `B` constant at 25 degrees C in water, in units of
/// `1 / (Angstrom * sqrt(mol/L))`.
const DH_B: f64 = 0.3281;

/// Aqueous activity-coefficient model. Relates free concentration to activity
/// by `a_i = gamma_i * c_i`, with `c_i` in mol/L.
///
/// The model set is closed and dispatched by `match` (no trait objects). All
/// variants are evaluated at ~25 degrees C; see the module header for the
/// physics, constants, and human-review simplifications.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivityModel {
    /// Ideal solution: `gamma = 1` for every species, at any ionic strength.
    Ideal,
    /// Extended Debye–Hückel with a single common ion-size parameter
    /// (`ion_size_angstrom`, the a0 in Angstrom) and an optional b-dot linear
    /// term (`bdot`, units `1 / (mol/L)`). Set `bdot = 0` for plain extended
    /// Debye–Hückel.
    DebyeHuckel {
        /// Common ion-size (a0) parameter in Angstrom, applied to every ion.
        /// Typical values are ~3–9 Angstrom (e.g. ~4 for Na+, ~9 for H+).
        ion_size_angstrom: f64,
        /// Optional b-dot linear term in `1 / (mol/L)`; `0.0` gives plain
        /// extended Debye–Hückel.
        bdot: f64,
    },
    /// Davies equation — parameterless, reasonable to ionic strength
    /// `I ~ 0.5 mol/L`.
    Davies,
}

impl ActivityModel {
    /// Ionic strength `I = 0.5 * sum_i c_i * z_i^2` in mol/L.
    ///
    /// `charges` holds the signed charge number `z_i` of each species and
    /// `concentrations` the matching free concentration `c_i` in mol/L; the two
    /// slices must have equal length.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if the slices differ in length,
    /// or if any concentration is negative (concentrations must be `>= 0`).
    ///
    /// # Example
    ///
    /// 0.01 M NaCl (Na+ and Cl- each at 0.01 mol/L, `z = +1` and `z = -1`):
    ///
    /// ```
    /// use outram_park_fork_pflotran::activity::ActivityModel;
    /// let i = ActivityModel::ionic_strength(&[1.0, -1.0], &[0.01, 0.01]).unwrap();
    /// assert!((i - 0.01).abs() < 1e-12);
    /// ```
    pub fn ionic_strength(charges: &[f64], concentrations: &[f64]) -> Result<f64, PflotranError> {
        if charges.len() != concentrations.len() {
            return Err(PflotranError::InvalidInput(format!(
                "ionic_strength: charges ({}) and concentrations ({}) must have equal length",
                charges.len(),
                concentrations.len()
            )));
        }
        let mut sum = 0.0;
        for (z, c) in charges.iter().zip(concentrations.iter()) {
            if *c < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "ionic_strength: negative concentration {c} mol/L"
                )));
            }
            sum += c * z * z;
        }
        Ok(0.5 * sum)
    }

    /// `log10(gamma_i)` for an ion of the given (signed, integer-or-real)
    /// `charge` at the supplied `ionic_strength` (mol/L).
    ///
    /// - [`ActivityModel::Ideal`] returns `0.0` (i.e. `gamma = 1`) always.
    /// - The charged models return `0.0` for a neutral species (`charge == 0`),
    ///   giving `gamma = 1` — the neutral-species simplification (see module
    ///   header).
    ///
    /// `ionic_strength` is clamped at `0` (a tiny negative round-off is treated
    /// as zero) so `sqrt(I)` is always real.
    pub fn log10_gamma(&self, charge: f64, ionic_strength: f64) -> f64 {
        match self {
            ActivityModel::Ideal => 0.0,
            ActivityModel::DebyeHuckel {
                ion_size_angstrom,
                bdot,
            } => {
                if charge == 0.0 {
                    return 0.0;
                }
                let i = ionic_strength.max(0.0);
                let sqrt_i = i.sqrt();
                let z2 = charge * charge;
                -DH_A * z2 * sqrt_i / (1.0 + DH_B * ion_size_angstrom * sqrt_i) + bdot * i
            }
            ActivityModel::Davies => {
                if charge == 0.0 {
                    return 0.0;
                }
                let i = ionic_strength.max(0.0);
                let sqrt_i = i.sqrt();
                let z2 = charge * charge;
                -DH_A * z2 * (sqrt_i / (1.0 + sqrt_i) - 0.3 * i)
            }
        }
    }

    /// Activity coefficient `gamma = 10^{log10_gamma}`.
    ///
    /// For the charged models a neutral species (`charge == 0`) returns
    /// `gamma = 1` (neutral-species simplification, see module header).
    /// [`ActivityModel::Ideal`] returns `1` for every species.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_pflotran::activity::ActivityModel;
    /// // Dilute limit: gamma -> 1 as I -> 0.
    /// let g = ActivityModel::Davies.gamma(2.0, 0.0);
    /// assert!((g - 1.0).abs() < 1e-12);
    /// ```
    pub fn gamma(&self, charge: f64, ionic_strength: f64) -> f64 {
        10f64.powf(self.log10_gamma(charge, ionic_strength))
    }

    /// The Debye–Hückel `A` constant used by the charged models
    /// (`≈ 0.5085` at 25 degrees C in water, units `1 / sqrt(mol/L)`).
    pub fn a_constant() -> f64 {
        DH_A
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAVIES: ActivityModel = ActivityModel::Davies;
    fn dh() -> ActivityModel {
        ActivityModel::DebyeHuckel {
            ion_size_angstrom: 4.0,
            bdot: 0.0,
        }
    }

    #[test]
    fn dilute_limit_gamma_one() {
        // gamma -> 1 as I -> 0 for every model and charge.
        for model in [ActivityModel::Ideal, dh(), DAVIES] {
            for z in [0.0, 1.0, -1.0, 2.0, -2.0, 3.0] {
                let g = model.gamma(z, 0.0);
                assert!((g - 1.0).abs() < 1e-12, "model {model:?} z={z} g={g}");
            }
        }
    }

    #[test]
    fn ideal_always_one() {
        for i in [0.0, 0.01, 0.1, 0.5, 1.0, 5.0] {
            for z in [0.0, 1.0, 2.0, -3.0] {
                assert_eq!(ActivityModel::Ideal.gamma(z, i), 1.0);
            }
        }
    }

    #[test]
    fn neutral_species_gamma_one() {
        // z = 0 -> gamma = 1 for the charged models at any I.
        for model in [dh(), DAVIES] {
            for i in [0.0, 0.1, 0.5, 1.0] {
                assert_eq!(model.gamma(0.0, i), 1.0, "model {model:?} I={i}");
            }
        }
    }

    #[test]
    fn gamma_decreases_with_ionic_strength() {
        // For a charged ion gamma should decrease monotonically as I rises
        // (over the valid range of each model).
        for model in [dh(), DAVIES] {
            for z in [1.0, -1.0, 2.0, -2.0] {
                let strengths = [0.001, 0.01, 0.05, 0.1, 0.2, 0.4];
                let mut prev = model.gamma(z, 0.0);
                assert!((prev - 1.0).abs() < 1e-12);
                for i in strengths {
                    let g = model.gamma(z, i);
                    assert!(g < prev, "model {model:?} z={z} I={i}: g={g} !< prev={prev}");
                    assert!(g > 0.0);
                    prev = g;
                }
            }
        }
    }

    #[test]
    fn divalent_depressed_more_than_monovalent() {
        // |log10 gamma| scales with z^2, so a +2 ion is depressed more than +1.
        for model in [dh(), DAVIES] {
            for i in [0.01, 0.1, 0.3] {
                let g1 = model.gamma(1.0, i);
                let g2 = model.gamma(2.0, i);
                assert!(g2 < g1, "model {model:?} I={i}: g2={g2} !< g1={g1}");
            }
        }
    }

    #[test]
    fn davies_spot_check_i_0p1() {
        // Davies for z=1 at I=0.1: log10 gamma = -A (sqrt(I)/(1+sqrt(I)) - 0.3 I).
        // sqrt(0.1) = 0.316228; term = 0.316228/1.316228 - 0.03 = 0.210231
        // log10 gamma = -0.5085 * 0.210231 = -0.106902 -> gamma = 0.7822.
        let g = DAVIES.gamma(1.0, 0.1);
        assert!((g - 0.7822).abs() < 0.78 * 0.02, "Davies z=1 I=0.1 g={g}");

        // Cross-check against the formula recomputed here (tight tolerance).
        let sqrt_i = 0.1f64.sqrt();
        let expected =
            10f64.powf(-ActivityModel::a_constant() * (sqrt_i / (1.0 + sqrt_i) - 0.3 * 0.1));
        assert!((g - expected).abs() < 1e-12, "g={g} expected={expected}");
    }

    #[test]
    fn debye_huckel_bdot_raises_gamma() {
        // A positive b-dot linear term increases log10 gamma vs plain extended DH.
        let plain = ActivityModel::DebyeHuckel {
            ion_size_angstrom: 4.0,
            bdot: 0.0,
        };
        let bdot = ActivityModel::DebyeHuckel {
            ion_size_angstrom: 4.0,
            bdot: 0.041,
        };
        let i = 0.5;
        assert!(bdot.gamma(1.0, i) > plain.gamma(1.0, i));
    }

    #[test]
    fn ionic_strength_nacl() {
        // 0.01 M NaCl: Na+ and Cl- at 0.01 mol/L -> I = 0.01.
        let i = ActivityModel::ionic_strength(&[1.0, -1.0], &[0.01, 0.01]).unwrap();
        assert!((i - 0.01).abs() < 1e-12, "I={i}");

        // 0.01 M CaCl2: Ca2+ 0.01 (z=2), Cl- 0.02 (z=1) -> I = 0.5(0.04 + 0.02) = 0.03.
        let i2 = ActivityModel::ionic_strength(&[2.0, -1.0], &[0.01, 0.02]).unwrap();
        assert!((i2 - 0.03).abs() < 1e-12, "I={i2}");
    }

    #[test]
    fn ionic_strength_length_mismatch_rejected() {
        let err = ActivityModel::ionic_strength(&[1.0, -1.0], &[0.01]);
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn ionic_strength_negative_concentration_rejected() {
        let err = ActivityModel::ionic_strength(&[1.0], &[-0.01]);
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn a_constant_value() {
        assert!((ActivityModel::a_constant() - 0.5085).abs() < 1e-12);
    }
}
