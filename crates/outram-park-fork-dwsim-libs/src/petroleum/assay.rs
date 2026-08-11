//! The **crude-assay data model** — the input to petroleum characterization.
//!
//! An assay describes a crude oil or refinery stream in one of two ways:
//!
//! - **Bulk** ([`BulkAssay`]) — a handful of averaged numbers (molecular
//!   weight, specific gravity, average boiling point, two viscosities). The
//!   pseudo-component distribution is then *generated* from a gamma
//!   distribution; see [`crate::petroleum::generate_compounds`].
//! - **Curve** ([`CurveAssay`]) — a measured distillation curve (temperature
//!   versus cumulative distilled fraction) plus optional molecular-weight,
//!   specific-gravity and viscosity curves. The curve is converted to a TBP
//!   basis, fitted, and *cut*; see
//!   [`crate::petroleum::curve_characterization`].
//!
//! # Provenance
//!
//! Port of DWSIM (GPL-3.0),
//! `DWSIM.SharedClasses/Misc/AssayClass.vb` (358 lines, whole file — the
//! `Utilities.PetroleumCharacterization.Assay.Assay` class), from the pinned
//! upstream clone `/home/teddy0/Documents/research/dwsim-upstream`, branch
//! `windows`, commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream
//! copyright: 2012 Daniel Wagner O. de Medeiros and the DWSIM contributors.
//! GPL-3.0; this port is GPL-3.0-only.
//!
//! # Structural deviation from upstream (deliberate, documented)
//!
//! Upstream stores the curve as **six parallel `ArrayList`s** — `PX` (the
//! abscissa) alongside `PY_NBP`, `PY_MW`, `PY_SG`, `PY_V1`, `PY_V2` — whose
//! lengths are not enforced to match, with three `Boolean` flags
//! (`HasMWCurve`, `HasSGCurve`, `HasViscCurves`) recording which of the
//! optional columns were populated (`AssayClass.vb:44-58`, `:73-79`). This port
//! replaces them with a single `Vec<`[`AssayCurvePoint`]`>` of `Option`-valued
//! columns, so a row cannot go out of sync with its abscissa and the flags
//! become derived queries ([`CurveAssay::has_molar_mass_curve`], etc.). The
//! *information content* is identical.
//!
//! Upstream's two constructors also share one mutable class that can be
//! neither, either, or both of bulk and curve (`_isbulk` / `_iscurve` are
//! independent `Boolean`s). This port makes that a closed [`Assay`] enum, so
//! "neither" and "both" are unrepresentable.
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported (no physics; .NET plumbing per the port scope):
//! `ICloneable.Clone` and the `BinaryFormatter` round-trip it uses
//! (`AssayClass.vb:322-341`), and the `ICustomXMLSerialization`
//! `LoadData`/`SaveData` pair (`:343-353`). Rust's `#[derive(Clone)]` covers
//! the first; XML persistence is out of scope for the whole crate.
//!
//! # Units
//!
//! `uom`-typed throughout. Cumulative fractions are `Ratio` on a **0..1**
//! scale, *not* percent — upstream's curve editor divides its percent input by
//! 100 on entry (`DistCurves.cs:895`), and every downstream routine expects
//! `0..1`.

use uom::si::f64::{KinematicViscosity, MolarMass, Ratio, ThermodynamicTemperature};

use super::property_methods::SpecificGravity;

/// Which distillation method produced the assay's temperature curve —
/// upstream's `Assay.NBPType` integer (`AssayClass.vb:45`, `:223-230`), whose
/// values are decoded by `DistCurves.cs:365-452`.
///
/// Enum dispatch (no trait objects), per the workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistillationCurveKind {
    /// `NBPType = 0` — the curve is already a **TBP (True Boiling Point,
    /// ASTM D2892)** curve and needs no conversion.
    #[default]
    Tbp,
    /// `NBPType = 1` — **ASTM D86**, atmospheric-pressure Engler distillation.
    /// Converted by [`crate::petroleum::curve_conversion::d86_to_tbp_riazi`].
    D86,
    /// `NBPType = 2` — **ASTM D1160**, vacuum distillation. Converted by
    /// [`crate::petroleum::curve_conversion::d1160_to_subatmospheric_tbp_wauquier`]
    /// and then lifted to the atmospheric basis by
    /// [`crate::petroleum::curve_conversion::subatmospheric_tbp_to_atmospheric_maxwell_bonnell`].
    D1160Vacuum,
    /// `NBPType = 3` — **ASTM D2887**, simulated distillation by gas
    /// chromatography. Converted by
    /// [`crate::petroleum::curve_conversion::d2887_to_tbp_daubert`].
    D2887Simulated,
}

/// The reference temperature of the assay's specific-gravity curve — upstream's
/// `Assay.SGCurveType` string, `"SG20"` or `"SG60"` (`AssayClass.vb:48`,
/// `:187-194`).
///
/// All property correlations in this module want `SG` at **15.6/15.6 °C**
/// (= 60/60 °F), so an `Sg20` curve must be converted with
/// [`crate::petroleum::property_methods::d15_from_d20`] before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecificGravityCurveKind {
    /// Specific gravity referenced to **20 °C** (upstream's `"SG20"`, its
    /// default).
    #[default]
    Sg20,
    /// Specific gravity referenced to **15.6 °C / 60 °F** (upstream's
    /// `"SG60"`) — the basis every correlation here expects.
    Sg60,
}

/// What the assay's cumulative abscissa measures — upstream's
/// `Assay.CurveBasis` (`AssayClass.vb:50`, `:106-113`), decoded by
/// `DistCurves.cs:1021-1080` (`CalculateMolarFractions`).
///
/// This choice decides how a cut's share of the barrel becomes a **mole
/// fraction**, so it materially changes the generated composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveBasis {
    /// **Liquid volume** percent distilled (the refinery default, and
    /// upstream's `case 0`). Converted to mass via each cut's specific gravity,
    /// then to moles via its molecular weight.
    #[default]
    LiquidVolume,
    /// **Mole** percent distilled (upstream's `case 1`) — used directly as the
    /// mole fraction.
    Mole,
    /// **Weight** percent distilled (upstream's `case 2`). Converted to moles
    /// via each cut's molecular weight.
    Weight,
}

/// One measured row of a distillation assay: the cumulative fraction distilled
/// and every property measured at that point.
///
/// Replaces upstream's six parallel `ArrayList`s (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssayCurvePoint {
    /// Cumulative fraction distilled at this point [-], on a **0..1** scale
    /// (upstream `PX`, stored `percent/100`). Must be ascending across the
    /// curve and lie in `[0, 1]`.
    pub cumulative_fraction: Ratio,
    /// Boiling temperature at this point [K], on the method given by
    /// [`CurveAssay::curve_kind`] (upstream `PY_NBP`).
    pub temperature: ThermodynamicTemperature,
    /// Optional measured molecular weight of the material distilling here
    /// (upstream `PY_MW`, gated by `HasMWCurve`).
    pub molar_mass: Option<MolarMass>,
    /// Optional measured specific gravity here, on the basis given by
    /// [`CurveAssay::specific_gravity_kind`] (upstream `PY_SG`, gated by
    /// `HasSGCurve`).
    pub specific_gravity: Option<SpecificGravity>,
    /// Optional measured kinematic viscosity at the assay's first reference
    /// temperature (upstream `PY_V1`, gated by `HasViscCurves`; DWSIM's curve
    /// editor takes it at **100 °F = 311 K**).
    pub kinematic_viscosity_1: Option<KinematicViscosity>,
    /// Optional measured kinematic viscosity at the assay's second reference
    /// temperature (upstream `PY_V2`; DWSIM's editor takes it at
    /// **210 °F = 372 K**).
    pub kinematic_viscosity_2: Option<KinematicViscosity>,
}

/// A **bulk** assay: averaged properties with no distillation curve.
///
/// Upstream's second constructor, `Assay.New(mw, sg60, nbpavg, t1, t2, v1, v2)`
/// (`AssayClass.vb:83-93`), which sets `_isbulk = True`.
///
/// At least one of `molar_mass`, `specific_gravity_60f`, `average_boiling_point`
/// must be present — the generator rejects an assay with none
/// (`GenerateCompounds.vb:35-37`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BulkAssay {
    /// Bulk (mole-averaged) molecular weight of the stream. `None` = unknown.
    pub molar_mass: Option<MolarMass>,
    /// Bulk specific gravity at 60/60 °F [-]. `None` = unknown.
    pub specific_gravity_60f: Option<SpecificGravity>,
    /// Bulk average normal boiling point [K]. `None` = unknown.
    pub average_boiling_point: Option<ThermodynamicTemperature>,
    /// Temperature of the first bulk viscosity measurement [K] (upstream `T1`).
    pub viscosity_temperature_1: Option<ThermodynamicTemperature>,
    /// Temperature of the second bulk viscosity measurement [K] (upstream `T2`).
    pub viscosity_temperature_2: Option<ThermodynamicTemperature>,
    /// Bulk kinematic viscosity at [`Self::viscosity_temperature_1`]
    /// (upstream `V1`). `None` = estimate it from Abbott's correlation.
    pub kinematic_viscosity_1: Option<KinematicViscosity>,
    /// Bulk kinematic viscosity at [`Self::viscosity_temperature_2`]
    /// (upstream `V2`). `None` = estimate it from Abbott's correlation.
    pub kinematic_viscosity_2: Option<KinematicViscosity>,
}

/// A **curve** assay: a measured distillation curve plus optional property
/// curves and bulk anchors.
///
/// Upstream's first constructor,
/// `Assay.New(k_api, mw, api, t1, t2, nbptype, sgtype, px, pynbp, pymw, pysg,
/// pyv1, pyv2)` (`AssayClass.vb:62-81`), which sets `_iscurve = True`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CurveAssay {
    /// Human-readable assay name; becomes the pseudo-component name prefix
    /// (upstream `Assay.Name`, `:313-320`).
    pub name: String,
    /// Watson `K` characterisation factor of the whole crude [-] (upstream
    /// `K_API`, `:169-176`). Used by the Maxwell-Bonnell vacuum correction.
    pub watson_k: Option<Ratio>,
    /// Bulk molecular weight anchor. When present, every generated cut's
    /// molecular weight is rescaled so the mixture average matches it
    /// (`DistCurves.cs:636-648`).
    pub bulk_molar_mass: Option<MolarMass>,
    /// Bulk **API gravity** anchor [-] (upstream `API`, `:178-185`). When
    /// present, every cut's specific gravity is rescaled so the mass-averaged
    /// mixture gravity matches it (`DistCurves.cs:650-662`).
    pub bulk_api_gravity: Option<Ratio>,
    /// Temperature of the first viscosity curve [K] (upstream `T1`).
    pub viscosity_temperature_1: Option<ThermodynamicTemperature>,
    /// Temperature of the second viscosity curve [K] (upstream `T2`).
    pub viscosity_temperature_2: Option<ThermodynamicTemperature>,
    /// Which distillation method produced [`AssayCurvePoint::temperature`].
    pub curve_kind: DistillationCurveKind,
    /// Reference temperature of [`AssayCurvePoint::specific_gravity`].
    pub specific_gravity_kind: SpecificGravityCurveKind,
    /// What the cumulative abscissa measures (volume, mole, or weight).
    pub basis: CurveBasis,
    /// The measured rows, ascending in [`AssayCurvePoint::cumulative_fraction`].
    pub points: Vec<AssayCurvePoint>,
}

impl CurveAssay {
    /// Does the assay carry a measured **molecular-weight** curve?
    ///
    /// Upstream's `HasMWCurve` flag (`AssayClass.vb:214-221`), which it sets
    /// from `pymw.Count = 0` at construction (`:73`). Here it is derived: true
    /// when **every** point carries a molecular weight, so a partially-filled
    /// column cannot be silently interpolated across gaps.
    #[must_use]
    pub fn has_molar_mass_curve(&self) -> bool {
        !self.points.is_empty() && self.points.iter().all(|p| p.molar_mass.is_some())
    }

    /// Does the assay carry a measured **specific-gravity** curve?
    /// Upstream's `HasSGCurve` (`AssayClass.vb:205-212`, set at `:75`).
    #[must_use]
    pub fn has_specific_gravity_curve(&self) -> bool {
        !self.points.is_empty() && self.points.iter().all(|p| p.specific_gravity.is_some())
    }

    /// Does the assay carry the **first** viscosity curve?
    /// Upstream's `HasViscCurves` (`AssayClass.vb:196-203`, set at `:77`).
    #[must_use]
    pub fn has_viscosity_curve_1(&self) -> bool {
        !self.points.is_empty()
            && self
                .points
                .iter()
                .all(|p| p.kinematic_viscosity_1.is_some())
    }

    /// Does the assay carry the **second** viscosity curve?
    #[must_use]
    pub fn has_viscosity_curve_2(&self) -> bool {
        !self.points.is_empty()
            && self
                .points
                .iter()
                .all(|p| p.kinematic_viscosity_2.is_some())
    }

    /// The cumulative fractions as a plain `f64` slice on the 0..1 scale —
    /// upstream's `PX` array, the abscissa every interpolation is built on.
    #[must_use]
    pub fn cumulative_fractions(&self) -> Vec<f64> {
        use uom::si::ratio::ratio;
        self.points
            .iter()
            .map(|p| p.cumulative_fraction.get::<ratio>())
            .collect()
    }

    /// The curve temperatures in K — upstream's `PY_NBP` array, on whichever
    /// distillation basis [`Self::curve_kind`] names.
    #[must_use]
    pub fn temperatures_kelvin(&self) -> Vec<f64> {
        use uom::si::thermodynamic_temperature::kelvin;
        self.points
            .iter()
            .map(|p| p.temperature.get::<kelvin>())
            .collect()
    }
}

/// A petroleum assay — the closed set of two forms DWSIM accepts.
///
/// Enum, not a struct with `is_bulk`/`is_curve` booleans, so the "neither" and
/// "both" states upstream can reach (`AssayClass.vb:32-33`, independently
/// settable at `:295-311`) are unrepresentable here.
#[derive(Debug, Clone, PartialEq)]
pub enum Assay {
    /// Bulk averaged properties, no distillation curve.
    Bulk(BulkAssay),
    /// A measured distillation curve with optional property curves.
    Curve(CurveAssay),
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::kinematic_viscosity::square_meter_per_second;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::kelvin;

    fn point(fv: f64, t: f64) -> AssayCurvePoint {
        AssayCurvePoint {
            cumulative_fraction: Ratio::new::<ratio>(fv),
            temperature: ThermodynamicTemperature::new::<kelvin>(t),
            molar_mass: None,
            specific_gravity: None,
            kinematic_viscosity_1: None,
            kinematic_viscosity_2: None,
        }
    }

    /// **Methodology.** The three "has curve" flags must be derived, not
    /// stored: an assay with no optional columns reports `false` everywhere; an
    /// assay with **every** row populated reports `true`; and an assay with a
    /// *partially* filled column reports `false` (the deliberate tightening
    /// over upstream's count-based flag — see the module docs).
    ///
    /// **Results (2026-08-11, this port).** Bare curve → all four flags false.
    /// Fully-populated curve → all four true. One-row-missing MW → MW flag
    /// false while the others stay true. Test passes.
    #[test]
    fn curve_flags_are_derived_and_require_every_row() {
        let mut assay = CurveAssay {
            points: vec![point(0.0, 350.0), point(0.5, 450.0), point(1.0, 600.0)],
            ..Default::default()
        };
        assert!(!assay.has_molar_mass_curve());
        assert!(!assay.has_specific_gravity_curve());
        assert!(!assay.has_viscosity_curve_1());
        assert!(!assay.has_viscosity_curve_2());

        for p in &mut assay.points {
            p.molar_mass = Some(MolarMass::new::<gram_per_mole>(150.0));
            p.specific_gravity = Some(Ratio::new::<ratio>(0.8));
            p.kinematic_viscosity_1 =
                Some(KinematicViscosity::new::<square_meter_per_second>(1.0e-6));
            p.kinematic_viscosity_2 =
                Some(KinematicViscosity::new::<square_meter_per_second>(5.0e-7));
        }
        assert!(assay.has_molar_mass_curve());
        assert!(assay.has_specific_gravity_curve());
        assert!(assay.has_viscosity_curve_1());
        assert!(assay.has_viscosity_curve_2());

        assay.points[1].molar_mass = None;
        assert!(
            !assay.has_molar_mass_curve(),
            "a gap must disable the curve"
        );
        assert!(assay.has_specific_gravity_curve());
    }

    /// **Methodology.** The `f64` accessors must return the abscissa on the
    /// 0..1 scale and the temperatures in kelvin, in row order.
    ///
    /// **Results (2026-08-11, this port).** `[0.0, 0.5, 1.0]` and
    /// `[350.0, 450.0, 600.0]` returned exactly. Test passes.
    #[test]
    fn accessors_return_si_scalars_in_row_order() {
        let assay = CurveAssay {
            points: vec![point(0.0, 350.0), point(0.5, 450.0), point(1.0, 600.0)],
            ..Default::default()
        };
        assert_eq!(assay.cumulative_fractions(), vec![0.0, 0.5, 1.0]);
        assert_eq!(assay.temperatures_kelvin(), vec![350.0, 450.0, 600.0]);
    }
}
