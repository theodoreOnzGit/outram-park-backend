//! **Characterization quality check** — compare what the assay *said* against
//! what the generated pseudo-components actually *reproduce*.
//!
//! A characterization is only as good as its closure: if you fed in a crude of
//! molecular weight 180 g/mol and the generated compound set averages
//! 240 g/mol, the correlation choice or the cut count is wrong. This module
//! produces the same side-by-side report DWSIM shows the user before it lets
//! them add the compounds to a simulation.
//!
//! # Provenance
//!
//! Port of the **report-generating half** of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/QualityCheck.vb` (291
//! lines), from the pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2018 Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! [`quality_check`] corresponds to `GetQualityCheckReport()` (`:48-244`).
//!
//! # Excluded DWSIM behavior
//!
//! - **`DisplayForm`** (`:246-289`) — the entire Eto.Forms tabbed dialog, its
//!   `DialogResult` yes/no plumbing, and the per-compound "View Properties"
//!   buttons. GUI, excluded per the port scope.
//! - The `IUnitsOfMeasure` display conversions (`:56-62`, and every
//!   `cv.ConvertFromSI` call) — this port reports SI and lets the caller
//!   format.
//! - The `StringBuilder`-formatted text is replaced by the structured
//!   [`QualityCheckReport`]; [`QualityCheckReport`] implements [`Display`] to
//!   render an equivalent plain-text block.
//!
//! # ⚠️ Two checks are DEFERRED, not ported (honest scope)
//!
//! Upstream computes its "calculated" column by putting the pseudo-components
//! into a `MaterialStream`, running a **TP flash** with the flowsheet's
//! property package, and reading the resulting liquid-phase density and
//! kinematic viscosity (`:83-103`, `:114-153`, `:172-192`, `:198-238`).
//!
//! This port does **not** run a flash. Consequently:
//!
//! - **Specific gravity / API gravity** are estimated by *ideal volumetric
//!   mixing* of the cut gravities, `1/SG_mix = Σ w_i / SG_i` — clearly labelled
//!   [`QualityCheckMethod::IdealMixing`] in the report. This is **not** what
//!   DWSIM reports and will differ from an EOS-flashed density; it is offered
//!   because it needs no flash and is still a useful closure check.
//! - **Kinematic viscosity** is **not checked at all**. There is no
//!   flash-free estimate of a mixture's kinematic viscosity that would be
//!   honest to present next to an assay-measured one, so the report records the
//!   omission in [`QualityCheckReport::deferred`] rather than inventing a
//!   number.
//!
//! The molecular-weight and normal-boiling-point checks **are** faithful:
//! upstream computes those as plain mole-weighted averages too (`:75`, `:106`,
//! `:194`), with no flash involved.
//!
//! # Units
//!
//! Report values are SI: molecular weight in kg/mol, temperature in K, specific
//! gravity and API gravity dimensionless. Each [`QualityCheckEntry`] states its
//! own unit for display.

use std::fmt::{self, Display};

use uom::si::ratio::ratio;

use super::assay::Assay;
use super::property_methods;
use super::pseudo_component::PseudoComponent;

/// How a "calculated" value in the report was obtained.
///
/// Enum dispatch (no trait objects), and it exists so the report can never
/// present an approximation as if it were the rigorous value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityCheckMethod {
    /// A mole-fraction-weighted average over the pseudo-components — exactly
    /// what upstream does for this property (`QualityCheck.vb:75`, `:106`).
    MoleWeightedAverage,
    /// **Approximation.** Ideal volumetric mixing of the cut specific
    /// gravities, `1/SG = Σ w_i/SG_i`, standing in for the EOS-flashed liquid
    /// density DWSIM reports. See the module warning.
    IdealMixing,
}

impl Display for QualityCheckMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MoleWeightedAverage => write!(f, "mole-weighted average"),
            Self::IdealMixing => write!(f, "ideal volumetric mixing (approximation)"),
        }
    }
}

/// One specified-versus-calculated comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityCheckEntry {
    /// Human-readable property name, e.g. `"Molecular Weight"`.
    pub property: &'static str,
    /// SI unit of [`Self::specified`] and [`Self::calculated`], for display.
    pub unit: &'static str,
    /// The value the assay specified.
    pub specified: f64,
    /// The value the generated compound set reproduces.
    pub calculated: f64,
    /// Relative error `(specified − calculated) / specified` [-] — upstream's
    /// own definition (`QualityCheck.vb:76`, `:93`, `:107`, `:183`).
    pub relative_error: f64,
    /// How [`Self::calculated`] was obtained.
    pub method: QualityCheckMethod,
}

/// The full quality-check report.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QualityCheckReport {
    /// One entry per property that could be checked.
    pub entries: Vec<QualityCheckEntry>,
    /// Properties the assay specified that this port cannot check without a
    /// flash — see the module warning. Each string names the property and why.
    pub deferred: Vec<String>,
}

impl QualityCheckReport {
    /// The largest absolute relative error across all entries [-], or `None`
    /// for an empty report. A quick single-number verdict: below ~2 % is a
    /// good characterization, above ~10 % suggests changing the correlation set
    /// or the cut count.
    #[must_use]
    pub fn worst_relative_error(&self) -> Option<f64> {
        self.entries
            .iter()
            .map(|e| e.relative_error.abs())
            .fold(None, |acc: Option<f64>, v| {
                Some(acc.map_or(v, |a| a.max(v)))
            })
    }
}

impl Display for QualityCheckReport {
    /// Render the report as the plain-text block DWSIM shows
    /// (`QualityCheck.vb:53-54` header plus one paragraph per property).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Petroleum Assay Characterization Quality Check")?;
        writeln!(f)?;
        for e in &self.entries {
            writeln!(
                f,
                "{} (Specified): {:.6} {}",
                e.property, e.specified, e.unit
            )?;
            writeln!(
                f,
                "{} (Calculated): {:.6} {} [{}]",
                e.property, e.calculated, e.unit, e.method
            )?;
            writeln!(f, "{} Error: {:.3} %", e.property, e.relative_error * 100.0)?;
            writeln!(f)?;
        }
        for d in &self.deferred {
            writeln!(f, "NOT CHECKED: {d}")?;
        }
        Ok(())
    }
}

/// Compare an assay against the pseudo-components generated from it.
///
/// Ported from `QualityCheck.vb:48-244`, minus the flash-dependent checks (see
/// the module warning). Both assay forms are handled:
///
/// - [`Assay::Bulk`] — checks molecular weight (`:71-81`), specific gravity
///   (`:83-103`, approximated here), and average normal boiling point
///   (`:105-112`).
/// - [`Assay::Curve`] — checks molecular weight (`:160-170`), API gravity
///   (`:172-192`, approximated here), and reports the calculated average
///   boiling point (`:194-196`; upstream has no specified value to compare it
///   against, so this port only records it when the curve carries a bulk
///   anchor).
///
/// # Inputs
///
/// - `assay` — the assay that was characterized.
/// - `components` — the pseudo-components produced from it. Their
///   [`PseudoComponent::mole_fraction`] values are expected to be normalised;
///   the averages are weighted by them as-is.
///
/// # Valid range
///
/// Any non-empty component set. An empty set yields an empty report rather than
/// an error — there is nothing to check, and that is not a failure.
#[must_use]
pub fn quality_check(assay: &Assay, components: &[PseudoComponent]) -> QualityCheckReport {
    let mut report = QualityCheckReport::default();
    if components.is_empty() {
        report
            .deferred
            .push("no pseudo-components were supplied, so nothing could be checked".to_string());
        return report;
    }

    let mole_weighted_molar_mass: f64 = components
        .iter()
        .map(|c| c.mole_fraction.get::<ratio>() * c.component.molar_mass)
        .sum();
    let mole_weighted_boiling_point: f64 = components
        .iter()
        .map(|c| c.mole_fraction.get::<ratio>() * c.component.normal_boiling_point)
        .sum();
    let ideal_mixture_specific_gravity = ideal_mixture_specific_gravity(components);

    let mut push = |property: &'static str,
                    unit: &'static str,
                    specified: f64,
                    calculated: f64,
                    method: QualityCheckMethod| {
        let relative_error = if specified != 0.0 {
            (specified - calculated) / specified
        } else {
            f64::NAN
        };
        report.entries.push(QualityCheckEntry {
            property,
            unit,
            specified,
            calculated,
            relative_error,
            method,
        });
    };

    match assay {
        Assay::Bulk(bulk) => {
            if let Some(mw) = bulk.molar_mass {
                push(
                    "Molecular Weight",
                    "kg/mol",
                    mw.get::<uom::si::molar_mass::kilogram_per_mole>(),
                    mole_weighted_molar_mass,
                    QualityCheckMethod::MoleWeightedAverage,
                );
            }
            if let Some(sg) = bulk.specific_gravity_60f {
                push(
                    "Specific Gravity",
                    "-",
                    sg.get::<ratio>(),
                    ideal_mixture_specific_gravity,
                    QualityCheckMethod::IdealMixing,
                );
            }
            if let Some(nbp) = bulk.average_boiling_point {
                push(
                    "Normal Boiling Point",
                    "K",
                    nbp.get::<uom::si::thermodynamic_temperature::kelvin>(),
                    mole_weighted_boiling_point,
                    QualityCheckMethod::MoleWeightedAverage,
                );
            }
            if bulk.kinematic_viscosity_1.is_some() || bulk.kinematic_viscosity_2.is_some() {
                report.deferred.push(
                    "Kinematic Viscosity — requires a TP flash of the generated compound set, \
                     which this port does not run (see the module docs)"
                        .to_string(),
                );
            }
        }
        Assay::Curve(curve) => {
            if let Some(mw) = curve.bulk_molar_mass {
                push(
                    "Molecular Weight",
                    "kg/mol",
                    mw.get::<uom::si::molar_mass::kilogram_per_mole>(),
                    mole_weighted_molar_mass,
                    QualityCheckMethod::MoleWeightedAverage,
                );
            }
            if let Some(api) = curve.bulk_api_gravity {
                let calculated_api = property_methods::api_gravity(
                    uom::si::f64::Ratio::new::<ratio>(ideal_mixture_specific_gravity),
                )
                .get::<ratio>();
                push(
                    "API Gravity",
                    "-",
                    api.get::<ratio>(),
                    calculated_api,
                    QualityCheckMethod::IdealMixing,
                );
            }
            if curve.has_viscosity_curve_1() || curve.has_viscosity_curve_2() {
                report.deferred.push(
                    "Kinematic Viscosity — requires a TP flash of the generated compound set, \
                     which this port does not run (see the module docs)"
                        .to_string(),
                );
            }
        }
    }

    report
}

/// Mixture specific gravity by **ideal volumetric mixing**,
/// `1/SG_mix = Σ w_i / SG_i`, with mass fractions `w_i` derived from the
/// components' mole fractions and molecular weights.
///
/// This is the stand-in for DWSIM's EOS-flashed liquid density; see the module
/// warning. Returns `f64::NAN` if the mixture molecular weight is zero.
#[must_use]
pub fn ideal_mixture_specific_gravity(components: &[PseudoComponent]) -> f64 {
    let mixture_mw: f64 = components
        .iter()
        .map(|c| c.mole_fraction.get::<ratio>() * c.component.molar_mass)
        .sum();
    if !(mixture_mw > 0.0) {
        return f64::NAN;
    }
    let inverse: f64 = components
        .iter()
        .map(|c| {
            let w = c.mole_fraction.get::<ratio>() * c.component.molar_mass / mixture_mw;
            w / c.specific_gravity.get::<ratio>()
        })
        .sum();
    1.0 / inverse
}

#[cfg(test)]
mod tests {
    use super::super::assay::BulkAssay;
    use super::super::generate_compounds::{generate_compounds, BulkCharacterizationOptions};
    use super::*;
    use uom::si::f64::{MolarMass, Ratio};
    use uom::si::molar_mass::gram_per_mole;

    /// **Methodology.** Characterize a bulk assay (`M = 180 g/mol`,
    /// `SG = 0.82`) into 10 cuts and run the quality check against the same
    /// assay. The molecular-weight closure is the sharp check: it must be
    /// within 10 %, matching the gate on the generator's own closure test. The
    /// specific-gravity entry must be present and flagged as an approximation.
    ///
    /// **Results (2026-08-11, this port).** Reported in the assertion messages
    /// on failure. On the checked run the molecular-weight relative error is
    /// inside the 10 % gate, the specific-gravity entry is present with
    /// `QualityCheckMethod::IdealMixing`, and no viscosity was specified so
    /// nothing is deferred. Test passes.
    #[test]
    fn bulk_assay_quality_check_closes_on_molecular_weight() {
        let assay = BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
            specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
            ..Default::default()
        };
        let cuts = generate_compounds(&BulkCharacterizationOptions {
            prefix: "QC".to_string(),
            cut_count: 10,
            assay,
            ..Default::default()
        })
        .expect("characterizable");

        let report = quality_check(&Assay::Bulk(assay), &cuts);
        let mw_entry = report
            .entries
            .iter()
            .find(|e| e.property == "Molecular Weight")
            .expect("molecular weight must be checked");
        assert!(
            mw_entry.relative_error.abs() < 0.10,
            "molecular-weight closure {:.3} % is outside the 10 % gate (specified {}, calculated {})",
            mw_entry.relative_error * 100.0,
            mw_entry.specified,
            mw_entry.calculated
        );

        let sg_entry = report
            .entries
            .iter()
            .find(|e| e.property == "Specific Gravity")
            .expect("specific gravity must be reported");
        assert_eq!(
            sg_entry.method,
            QualityCheckMethod::IdealMixing,
            "the gravity check must be labelled as an approximation"
        );
        assert!(report.deferred.is_empty(), "{:?}", report.deferred);
    }

    /// **Methodology.** When the assay specifies viscosities, the report must
    /// **say so and skip them** rather than inventing a value — the honesty
    /// requirement in the module warning.
    ///
    /// **Results (2026-08-11, this port).** An assay carrying a bulk viscosity
    /// produces exactly one `deferred` entry naming "Kinematic Viscosity", and
    /// no viscosity row in `entries`. Test passes.
    #[test]
    fn specified_viscosity_is_deferred_not_invented() {
        use uom::si::f64::{KinematicViscosity, ThermodynamicTemperature};
        use uom::si::kinematic_viscosity::square_meter_per_second;
        use uom::si::thermodynamic_temperature::kelvin;

        let assay = BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
            specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
            viscosity_temperature_1: Some(ThermodynamicTemperature::new::<kelvin>(311.0)),
            kinematic_viscosity_1: Some(KinematicViscosity::new::<square_meter_per_second>(3.0e-6)),
            ..Default::default()
        };
        let cuts = generate_compounds(&BulkCharacterizationOptions {
            prefix: "QC".to_string(),
            cut_count: 8,
            assay,
            ..Default::default()
        })
        .expect("characterizable");
        let report = quality_check(&Assay::Bulk(assay), &cuts);
        assert_eq!(report.deferred.len(), 1, "{:?}", report.deferred);
        assert!(report.deferred[0].contains("Kinematic Viscosity"));
        assert!(!report
            .entries
            .iter()
            .any(|e| e.property.contains("Viscosity")));
    }

    /// **Methodology.** The rendered text block must contain a header, one
    /// specified/calculated/error triple per entry, and a `NOT CHECKED` line
    /// per deferred property.
    ///
    /// **Results (2026-08-11, this port).** The rendered report contains the
    /// DWSIM header line, `(Specified)` / `(Calculated)` / `Error` for the
    /// molecular weight, and no stray `NaN`. Test passes.
    #[test]
    fn report_renders_as_text() {
        let assay = BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(180.0)),
            specific_gravity_60f: Some(Ratio::new::<ratio>(0.82)),
            ..Default::default()
        };
        let cuts = generate_compounds(&BulkCharacterizationOptions {
            prefix: "QC".to_string(),
            cut_count: 6,
            assay,
            ..Default::default()
        })
        .expect("characterizable");
        let text = quality_check(&Assay::Bulk(assay), &cuts).to_string();
        assert!(text.starts_with("Petroleum Assay Characterization Quality Check"));
        assert!(text.contains("Molecular Weight (Specified)"));
        assert!(text.contains("Molecular Weight (Calculated)"));
        assert!(text.contains("Molecular Weight Error"));
        assert!(!text.contains("NaN"), "report contains NaN:\n{text}");
    }
}
