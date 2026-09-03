//! # Petroleum characterization — crude assay → pseudo-components
//!
//! A crude oil is not a compound. It is a continuum of thousands of
//! hydrocarbons, and no equation of state can flash a continuum. **Petroleum
//! characterization** is the step that turns a refinery *assay* — a
//! distillation curve, or a handful of averaged bulk properties — into a finite
//! set of **pseudo-components**: fictitious pure compounds, each standing for a
//! narrow boiling range, each carrying the `Tc`, `Pc`, `ω`, `M` an EOS needs.
//!
//! This module is that step. Give it an [`Assay`], get back a
//! `Vec<`[`PseudoComponent`]`>` whose [`PseudoComponent::component`] field is
//! the crate's own [`Component`](crate::thermo::component::Component) — ready to
//! hand straight to [`crate::thermo::cubic_eos`],
//! [`crate::thermo::chao_seader_grayson`], [`crate::thermo::black_oil`], the
//! flash family, or a [`crate::columns`] distillation column.
//!
//! ## Quick start
//!
//! ```
//! use outram_park_fork_dwsim_libs::petroleum::{characterize, Assay, BulkAssay};
//! use uom::si::f64::{MolarMass, Ratio};
//! use uom::si::molar_mass::gram_per_mole;
//! use uom::si::ratio::ratio;
//!
//! // A light crude known only by its bulk properties.
//! let assay = Assay::Bulk(BulkAssay {
//!     molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
//!     specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
//!     ..Default::default()
//! });
//!
//! let cuts = characterize(&assay, 8).expect("a light crude is characterizable");
//! assert_eq!(cuts.len(), 8);
//!
//! // Every cut is a Component the thermo kernel can flash directly.
//! for cut in &cuts {
//!     let c = &cut.component;
//!     assert!(c.critical_temperature > 0.0 && c.critical_pressure > 0.0);
//! }
//! ```
//!
//! ## The two paths
//!
//! | You have | Use | Module |
//! |---|---|---|
//! | A measured **distillation curve** (TBP / D86 / D1160 / D2887) | [`characterize_curve`] | [`curve_characterization`] |
//! | Only **bulk averages** (`M`, `SG`, `Tb`) | [`generate_compounds`] | [`generate_compounds`](self::generate_compounds) |
//! | Bulk **C7+** properties, older method | [`distribute_riazi`] | [`riazi`] |
//!
//! [`characterize`] dispatches over the first two for you.
//!
//! Optional follow-up passes:
//!
//! - [`apply_parameter_fits`] — nudge each cut's acentric factor, Rackett
//!   `Z_RA` and PR/SRK volume-translation coefficients so the EOS reproduces
//!   the assay's own `Tb` and `SG`. Not run by default: it is expensive (a
//!   bubble-point solve per trial).
//! - [`quality_check`] — a specified-versus-reproduced closure report.
//!
//! ## Module map
//!
//! | Module | Role |
//! |---|---|
//! | [`assay`] | The input data model: [`Assay`], [`BulkAssay`], [`CurveAssay`], curve kinds and bases. |
//! | [`curve_conversion`] | D86 / D1160 / D2887 / sub-atmospheric → **TBP**, plus the 6th-degree TBP fit and its Newton inversion. |
//! | [`property_methods`] | The correlation library: `Tc`, `Pc`, `ω`, `M`, viscosity, SG conversions (Riazi-Daubert, Lee-Kesler, Farah, Winn, Abbott, Twu, Walther-ASTM). |
//! | [`gl`] | Ideal-gas formation properties from a PNA molecular-type analysis. |
//! | [`aux_props`] | Rackett density, `Zc`, `Vc`, Vetere `ΔHvb` — the DWSIM auxiliaries this path calls out to. |
//! | [`special`] | Gamma and regularized incomplete gamma functions. |
//! | [`lm`] | Levenberg-Marquardt least squares and the seven DWSIM model forms. |
//! | [`pseudo_component`] | [`PseudoComponent`], the correlation-selection enums, and the per-cut assembly. |
//! | [`generate_compounds`](self::generate_compounds) | The bulk (gamma-distribution) path. |
//! | [`curve_characterization`] | The distillation-curve path. |
//! | [`riazi`] | Riazi's older C7+ bulk distribution. |
//! | [`fitting`] | Brent minimisation and the four DWSIM parameter fits. |
//! | [`quality_check`](self::quality_check) | The closure report. |
//!
//! ## Provenance
//!
//! Pure-Rust port of DWSIM (GPL-3.0), pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. This port is GPL-3.0-only.
//! Source files:
//!
//! - `DWSIM.Thermodynamics/PetroleumCharacterization/` — `CurveConversion.vb`,
//!   `Fitting.vb`, `GL.vb`, `GenerateCompounds.vb`, `LM.vb`,
//!   `PropertyMethods.vb`, `QualityCheck.vb`, `Riazi.vb`.
//! - `DWSIM.SharedClasses/Misc/AssayClass.vb` — the assay data model.
//! - `DWSIM.UI.Desktop.Editors/Compounds/DistCurves.cs` — the **algorithmic
//!   core only** of the curve path (DWSIM keeps it in a GUI file; every widget,
//!   dialog and text parser around it is excluded).
//! - Four auxiliary correlations from
//!   `DWSIM.Thermodynamics/PropertyPackages/Models/` (`FluidProperties.vb`,
//!   `Hypotheticals.vb`) that the characterization path calls into.
//!
//! Each submodule's docs carry its own file/line mapping and its own
//! **"Excluded DWSIM behavior"** section.
//!
//! ## ⚠️ Status and known upstream defects
//!
//! > **Untrusted AI-assisted draft.** This is a translation, verified against
//! > closure identities and public-literature spot values, **not** validated
//! > against measured assay data. It is for education, research and
//! > verification work only — see `RESPONSIBLE_USE.md`.
//!
//! Three genuine **upstream defects** are reproduced faithfully rather than
//! silently corrected, each documented at its site and pinned by a test:
//!
//! 1. **[`property_methods::pc_lee_kesler`] returns ≈10× the correct
//!    pressure** (a wrong unit conversion upstream). Selecting
//!    [`CriticalPressureCorrelation::LeeKesler1976`] therefore also corrupts
//!    the acentric factor. The default is
//!    [`CriticalPressureCorrelation::RiaziDaubert1985`], which is correct.
//! 2. **The bulk "boiling point only" path cannot work** — upstream reads its
//!    specific-gravity array before writing it, so the molecular weight
//!    collapses to zero. This port reproduces that and then *reports* it as
//!    [`PseudoComponentError::NonPhysical`] instead of emitting absurd
//!    compounds. Always supply `SG` or `M` alongside `Tb`.
//! 3. **[`CriticalTemperatureCorrelation::Farah2006`]'s arguments are swapped
//!    at every upstream call site** (`Tb` into the `SG` slot and vice versa).
//!    Reproduced for bit-fidelity; the values it produces are not physically
//!    meaningful.
//!
//! Two smaller ones — Riazi's `SG_p` assigned a boiling-point expression, and
//! the SRK volume-translation fit using Peng-Robinson's `Ωb` — are documented
//! in [`riazi`] and [`fitting`].
//!
//! ## Design notes
//!
//! - **Enums, never trait objects.** Curve kind, curve basis, cut
//!   specification, and all four correlation choices are enums, so adding a
//!   variant is a compile error at every match site.
//! - **`uom` on every public boundary**, raw documented SI `f64` inside the
//!   distribution and correlation loops.
//! - **No new dependencies.** The Levenberg-Marquardt solver
//!   ([`lm::levenberg_marquardt`]), the Brent minimiser
//!   ([`fitting::brent_minimize`]) and the incomplete gamma functions
//!   ([`special`]) are written from their published mathematical definitions —
//!   DWSIM delegates all three to vendored ALGLIB, whose source was **not**
//!   consulted or copied. The Floater-Hormann interpolation reuses the crate's
//!   existing [`crate::interpolation::floater_hormann`].

pub mod assay;
pub mod aux_props;
pub mod curve_characterization;
pub mod curve_conversion;
/// Crude (atmospheric) distillation driven by a black-oil characterisation —
/// the petroleum counterpart of the benzene/toluene column. See the module
/// docs for what "based on black oil" can and cannot mean.
pub mod crude_distillation;
pub mod fitting;
pub mod generate_compounds;
pub mod gl;
pub mod lm;
pub mod property_methods;
pub mod pseudo_component;
pub mod quality_check;
pub mod riazi;
pub mod special;

pub use assay::{
    Assay, AssayCurvePoint, BulkAssay, CurveAssay, CurveBasis, DistillationCurveKind,
    SpecificGravityCurveKind,
};
pub use curve_characterization::{
    characterize_curve, CurveCharacterizationOptions, CurveCut, CutSpecification,
};
pub use curve_conversion::{CurveConversionError, TbpCurveFit};
pub use fitting::{apply_parameter_fits, ParameterFitOptions};
pub use generate_compounds::{generate_compounds, BulkCharacterizationOptions, CharacterizationError};
pub use gl::FormationProperties;
pub use property_methods::{AcentricFactor, SpecificGravity, WatsonK};
pub use pseudo_component::{
    AcentricFactorCorrelation, CorrelationSet, CriticalPressureCorrelation,
    CriticalTemperatureCorrelation, MolecularWeightCorrelation, PseudoComponent,
    PseudoComponentError,
};
pub use quality_check::{quality_check, QualityCheckReport};
pub use riazi::{distribute_riazi, RiaziDistributionCut, RiaziError};

/// Characterize **any** assay into `cut_count` pseudo-components, dispatching
/// over the assay form.
///
/// The one-call entry point most callers want:
///
/// - [`Assay::Bulk`] → [`generate_compounds`] with the default correlation set
///   and distribution bounds.
/// - [`Assay::Curve`] → [`characterize_curve`] with `cut_count`
///   equal-temperature-width cuts.
///
/// For control over the correlation set, the distribution's lower bounds, or
/// explicit cut temperatures, call the underlying entry point directly with its
/// own options struct.
///
/// The optional parameter-fitting pass ([`apply_parameter_fits`]) is **not**
/// run — it is expensive and its value depends on how much you trust the
/// assay's own `Tb`/`SG`. Run it yourself on the returned slice if you want it.
///
/// # Inputs
///
/// - `assay` — the crude assay. A bulk assay needs at least one of `M`, `SG`,
///   `Tb` (and see the module warning about supplying `Tb` alone); a curve
///   assay needs at least 2 curve points.
/// - `cut_count` — how many pseudo-components to produce. At least 2; 8-15 is
///   the usual refinery range.
///
/// # Valid range
///
/// The resulting cuts should fall inside the correlation validity band
/// (`Tb` ≈ 300-850 K, `SG` ≈ 0.6-1.05, `M` ≈ 70-500 g/mol); a cut outside it is
/// rejected rather than silently emitted.
///
/// # Errors
///
/// [`CharacterizationError`] — an assay with nothing to distribute, a
/// degenerate cut count, or a cut whose correlated constants come out
/// non-physical.
pub fn characterize(
    assay: &Assay,
    cut_count: usize,
) -> Result<Vec<PseudoComponent>, CharacterizationError> {
    match assay {
        Assay::Bulk(bulk) => generate_compounds(&BulkCharacterizationOptions {
            prefix: "PseudoC".to_string(),
            cut_count,
            assay: *bulk,
            ..Default::default()
        }),
        Assay::Curve(curve) => characterize_curve(&CurveCharacterizationOptions {
            assay: curve.clone(),
            cuts: CutSpecification::EqualTemperatureWidth { count: cut_count },
            correlations: CorrelationSet::default(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{MolarMass, Ratio, ThermodynamicTemperature};
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::kelvin;

    /// **Methodology.** End-to-end integration over the whole module: take a
    /// synthetic light-crude **TBP curve** (public-literature-style, 330-700 K
    /// over 11 points — no proprietary assay data anywhere in this crate),
    /// characterize it through the top-level [`characterize`] entry point, run
    /// the optional [`apply_parameter_fits`] pass, and finish with a
    /// [`quality_check`]. The pipeline must:
    ///
    /// 1. produce exactly the requested number of cuts,
    /// 2. keep the mole fractions summing to 1 **after** the fitting pass,
    /// 3. leave every EOS constant finite and positive, and
    /// 4. produce a quality-check report with no `NaN`.
    ///
    /// **Results (2026-08-11, this port).** Reported by the assertion messages
    /// on failure. On the checked run all four hold for a 10-cut
    /// characterization with both optional fits enabled. Test passes.
    #[test]
    fn end_to_end_curve_characterization_pipeline() {
        let rows = [
            (0.00, 330.0),
            (0.10, 375.0),
            (0.20, 410.0),
            (0.30, 445.0),
            (0.40, 480.0),
            (0.50, 515.0),
            (0.60, 550.0),
            (0.70, 585.0),
            (0.80, 620.0),
            (0.90, 660.0),
            (1.00, 700.0),
        ];
        let assay = Assay::Curve(CurveAssay {
            name: "SyntheticLightCrude".to_string(),
            curve_kind: DistillationCurveKind::Tbp,
            basis: CurveBasis::LiquidVolume,
            bulk_molar_mass: Some(MolarMass::new::<gram_per_mole>(200.0)),
            points: rows
                .iter()
                .map(|&(fv, t)| AssayCurvePoint {
                    cumulative_fraction: Ratio::new::<ratio>(fv),
                    temperature: ThermodynamicTemperature::new::<kelvin>(t),
                    molar_mass: None,
                    specific_gravity: None,
                    kinematic_viscosity_1: None,
                    kinematic_viscosity_2: None,
                })
                .collect(),
            ..Default::default()
        });

        let mut cuts = characterize(&assay, 10).expect("synthetic light crude is characterizable");
        assert_eq!(cuts.len(), 10);

        apply_parameter_fits(
            &mut cuts,
            ParameterFitOptions {
                adjust_acentric_factor: true,
                adjust_rackett_z: true,
            },
        );

        let mole_sum: f64 = cuts.iter().map(|c| c.mole_fraction.get::<ratio>()).sum();
        assert!(
            (mole_sum - 1.0).abs() < 1.0e-10,
            "mole fractions sum to {mole_sum} after fitting"
        );
        for c in &cuts {
            assert!(c.component.critical_temperature > 0.0, "{c:?}");
            assert!(c.component.critical_pressure > 0.0, "{c:?}");
            assert!(c.component.critical_volume > 0.0, "{c:?}");
            assert!(c.component.molar_mass > 0.0, "{c:?}");
            assert!(c.component.acentric_factor.is_finite(), "{c:?}");
            assert!(c.rackett_z > 0.0, "{c:?}");
        }

        let report = quality_check(&assay, &cuts);
        let text = report.to_string();
        assert!(
            !text.contains("NaN"),
            "quality report contains NaN:\n{text}"
        );
        assert!(
            report.worst_relative_error().is_some(),
            "the bulk molecular-weight anchor should have been checked"
        );
    }

    /// **Methodology.** The bulk path must be reachable through the same
    /// top-level entry point and produce a usable set of
    /// [`crate::thermo::component::Component`]s.
    ///
    /// **Results (2026-08-11, this port).** A `M = 180 g/mol`, `SG = 0.82` bulk
    /// assay produces 8 cuts, mole fractions summing to 1 within 1e-12, all
    /// constants positive. Test passes.
    #[test]
    fn end_to_end_bulk_characterization_pipeline() {
        let assay = Assay::Bulk(BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
            specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
            ..Default::default()
        });
        let cuts = characterize(&assay, 8).expect("characterizable");
        assert_eq!(cuts.len(), 8);
        let sum: f64 = cuts.iter().map(|c| c.mole_fraction.get::<ratio>()).sum();
        assert!((sum - 1.0).abs() < 1.0e-12, "{sum}");
        for c in &cuts {
            assert!(c.component.molar_mass > 0.0);
            assert!(c.component.critical_temperature > 0.0);
        }
    }
}
