// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Read an upstream GeN-Foam two-phase case directory into a
//! [`TwoPhaseCase`], so the port runs on upstream's own input files rather
//! than re-typed numbers.
//!
//! Reads `constant/<region>/{polyMesh, g, phaseProperties,
//! thermophysicalProperties.<fluid>, turbulenceProperties.<fluid>}`,
//! `system/controlDict`, `system/<region>/fvSolution` and the `0/<region>`
//! fields, with basic-lib's `FoamFile` dictionary parser and this crate's
//! `polyMesh` reader. **Every selection the port does not implement is a hard
//! error naming it**. ~~Except the alpha solver (`MULES` → `implicitUpwind`), recorded
//! in [`TwoPhaseCase::deviations`].~~ **CORRECTED 2026-11-26** (gh:#1361 round 7): `MULES` is ported and is what
//! a case that selects it runs; the old substitution is the explicit
//! [`TwoPhaseCase::with_alpha_solver_implicit_upwind`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[cfg(feature = "std")]
use std::path::Path;
use alloc::sync::Arc;

use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::{FoamDict, FoamEntry, FoamValue};
use outram_foam_basic_lib::prelude::*;
use uom::si::f64::{Ratio, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::fv::{Bc, Geometry, Scalar, Vector, VectorBc};
use super::thermo::PhaseThermo;
use super::{
    ByRegime, HPbicgstab, PRghPcg, FfHtcModel, FsHtcModel, HeStabilization, Phase, PhaseChangeConfig, RegimeMapSelection, RegimeParameter,
    ChfSelection, Structure, TimeControls, TraceBoilingModel, TraceChfModel, TwoPhaseControls, TwoPhaseModels,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::cachard::{CachardLiquid, CachardVapour};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::LeidenfrostTemperature;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf_table::LookUpTableChf;
use super::mules::{AlphaConvection, LimiterControls};
use super::{AlphaSolver, MulesControls, MultiRegimeBoilingModel};
#[cfg(feature = "std")]
use crate::error::AppBuilderError;
use crate::genfoam::thermal_hydraulics::closures::ff_drag::{FfDragCoefficient, TwoPhaseDragMultiplier};
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::{
    FlowEnhancementFactor, OnsetOfNucleateBoilingTemperature, SubcooledBoilingFraction, SuppressionFactor,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::{
    FsForcedConvectionHtc, PoolBoilingHtc,
};
use crate::genfoam::thermal_hydraulics::closures::interfacial::by_regime::ByRegimeModel;
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map_2d::RegimeMap2D;
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map::{
    RegimeBound, RegimeInterpolationMode, RegimeMap1D,
};
use crate::genfoam::thermal_hydraulics::closures::phase_change::{
    LatentHeatModel, PhaseChangeRateModel, SaturationModel,
};
use crate::genfoam::thermal_hydraulics::structure::power_model::FixedPower;
use crate::genfoam::thermal_hydraulics::structure::power_off::PowerOffCriterion;
#[cfg(feature = "std")]
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh};

/// Everything [`super::TwoPhaseSolver::new`] needs, read from a case.
#[derive(Debug, Clone)]
pub struct TwoPhaseCase {
    /// The fluid region mesh.
    pub mesh: Arc<FvMesh>,
    /// Gravity [m/s^2].
    pub gravity: Vector3,
    /// `p` [Pa].
    pub p: Scalar,
    /// `p_rgh` [Pa].
    pub p_rgh: Scalar,
    /// The two fluids, `fluid1` first.
    pub fluids: [Phase; 2],
    /// The structure.
    pub structure: Structure,
    /// Closure selections.
    pub models: TwoPhaseModels,
    /// Solution controls.
    pub controls: TwoPhaseControls,
    /// Time controls.
    pub time_controls: TimeControls,
    /// `startTime` [s].
    pub start_time: f64,
    /// `endTime` [s].
    pub end_time: f64,
    /// Initial `deltaT` [s].
    pub delta_t: f64,
    /// Places where the port runs something other than what the case
    /// selects, one sentence each.
    pub deviations: Vec<String>,
}

#[cfg(feature = "std")]
fn err(case: &Path, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Case {
        case: case.to_path_buf(),
        msg: msg.into(),
    }
}

/// Reading context: the case root, for error messages.
pub(super) struct Ctx {
    pub(super) root: crate::path::PathBuf,
}

impl Ctx {
    #[cfg(feature = "std")]
    pub(super) fn read(&self, rel: &str) -> Result<FoamDict, AppBuilderError> {
        let p = self.root.join(rel);
        FoamFile::read_esi(&p)
            .map(|f| f.dict)
            .map_err(|e| err(&self.root, format!("{rel}: {e}")))
    }

    /// As [`Ctx::read`], following `#include "file"` (the `lookUpTableCHF` table is one).
    #[cfg(feature = "std")]
    pub(super) fn read_with_includes(&self, rel: &str) -> Result<FoamDict, AppBuilderError> {
        let p = self.root.join(rel);
        FoamFile::read_with_includes_esi(&p)
            .map(|f| f.dict)
            .map_err(|e| err(&self.root, format!("{rel}: {e}")))
    }

    #[cfg(feature = "std")]
    pub(super) fn e(&self, msg: impl Into<String>) -> AppBuilderError {
        err(&self.root, msg)
    }

    #[cfg(feature = "std")]
    pub(super) fn sub<'a>(&self, d: &'a FoamDict, key: &str) -> Result<&'a FoamDict, AppBuilderError> {
        d.get_dict(key).ok_or_else(|| self.e(format!("missing sub-dictionary `{key}`")))
    }

    #[cfg(feature = "std")]
    pub(super) fn f64(&self, d: &FoamDict, key: &str) -> Result<f64, AppBuilderError> {
        scalar(d, key).ok_or_else(|| self.e(format!("missing scalar `{key}`")))
    }

    #[cfg(feature = "std")]
    pub(super) fn word(&self, d: &FoamDict, key: &str) -> Result<String, AppBuilderError> {
        word(d, key).ok_or_else(|| self.e(format!("missing word `{key}`")))
    }

    #[cfg(feature = "std")]
    pub(super) fn list8(&self, d: &FoamDict, key: &str) -> Result<[f64; 8], AppBuilderError> {
        let v = match d.get(key) {
            Some(FoamEntry::List(v)) => v,
            _ => return Err(self.e(format!("missing list `{key}`"))),
        };
        let mut out = [0.0; 8];
        if v.len() != 8 {
            return Err(self.e(format!("`{key}` must have 8 coefficients")));
        }
        for (o, x) in out.iter_mut().zip(v) {
            *o = x.as_scalar().ok_or_else(|| self.e(format!("`{key}`: non-scalar")))?;
        }
        Ok(out)
    }
}

pub(super) fn scalar(d: &FoamDict, key: &str) -> Option<f64> {
    match d.get(key)? {
        FoamEntry::Scalar(x) => Some(*x),
        FoamEntry::Dimensioned(dm) => dm.value.first().and_then(FoamValue::as_scalar),
        _ => None,
    }
}

pub(super) fn word(d: &FoamDict, key: &str) -> Option<String> {
    match d.get(key)? {
        FoamEntry::Word(w) | FoamEntry::Str(w) => Some(w.clone()),
        _ => None,
    }
}

pub(super) fn bool_entry(d: &FoamDict, key: &str) -> Option<bool> {
    word(d, key).map(|w| matches!(w.as_str(), "true" | "on" | "yes" | "1"))
}

/// OpenFOAM keyword regex, restricted to literals and `.*` (all the cases
/// use): `alpha.*` matches `alpha.vapour`.
pub(super) fn key_matches(pattern: &str, name: &str) -> bool {
    // gh:#1870 round 62: groups, alternation, classes and `+`/`?` (`"(h|k|epsilon)"`) go to the full matcher; literal and `.*` patterns keep this path
    if super::foam_regex::needs_full_regex(pattern) {
        return super::foam_regex::foam_regex_full_match(pattern, name);
    }
    if !pattern.contains(".*") {
        return pattern == name;
    }
    let parts: Vec<&str> = pattern.split(".*").collect();
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            if !rest.starts_with(part) {
                return false;
            }
            rest = &rest[part.len()..];
        } else if i == parts.len() - 1 {
            return part.is_empty() || rest.ends_with(part);
        } else if let Some(pos) = rest.find(part) {
            rest = &rest[pos + part.len()..];
        } else {
            return false;
        }
    }
    true
}

/// OpenFOAM's dictionary lookup of `name`: the exact keyword if there is one, else the LAST declared pattern that matches (patterns
/// are searched from the newest entry back, so a catch-all `".*"` declared after `"h.*"` shadows it).
pub(crate) fn lookup_pattern_last<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().filter(|(k, _)| key_matches(k, name)).last().map(|(_, e)| e)
}

/// Dictionary lookup as OpenFOAM does it for `solvers`, `relaxationFactors` and `divSchemes`: the exact keyword, else the LAST
/// matching pattern ([`lookup_pattern_last`]). gh:#1902 (README round 49): this used to return the FIRST matching pattern
/// ([`lookup_regex_first`], kept as the named old form); OpenFOAM's `dictionary::lookupEntryPtr` walks its pattern list from the
/// newest entry back (`dictionary.C:105,127,665,693`: `patterns_.push_front`).
pub(super) fn lookup_regex<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    lookup_pattern_last(d, name)
}

/// The first entry of `d` whose (regex) key matches `name`: the form of [`lookup_regex`] before gh:#1902, kept as the named
/// old behaviour (it differs from OpenFOAM's when two patterns match).
pub(super) fn lookup_regex_first<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().find(|(k, _)| key_matches(k, name)).map(|(_, e)| e)
}

#[cfg(feature = "std")]
/// `internalField uniform v;` (scalar) or the first component of a vector.
pub(super) fn uniform_value(ctx: &Ctx, e: Option<&FoamEntry>) -> Result<FoamValue, AppBuilderError> {
    match e {
        Some(FoamEntry::Tokens(t)) if t.len() == 2 && t[0].as_word() == Some("uniform") => Ok(t[1].clone()),
        _ => Err(ctx.e("only `uniform` field values are supported")),
    }
}

/// A volume field file: internal value and per-patch BCs.
pub(super) struct FieldFile {
    /// The field's name (the file name), for error messages.
    pub(super) name: String,
    pub(super) internal: FoamValue,
    /// `internalField nonuniform List<scalar> N (...)`: the cell values (`read_alpha_field` only; `internal` then
    /// holds the first one). `None` for a `uniform` field.
    pub(super) internal_list: Option<Vec<f64>>,
    pub(super) bcs: Vec<(String, String, Option<FoamValue>)>,
    /// The whole dictionary of each patch (`phi`, `inletValue`, ...), by patch name.
    pub(super) dicts: Vec<(String, FoamDict)>,
}

#[cfg(feature = "std")]
pub(super) fn read_field(ctx: &Ctx, rel: &str) -> Result<FieldFile, AppBuilderError> {
    read_field_with(ctx, rel, false)
}

/// [`read_field`] for `alpha.<fluid>`: also accepts `internalField nonuniform List<scalar> N (...)` (gh:#1799). Every other
/// field still refuses a non-uniform internal field by name.
#[cfg(feature = "std")]
pub(super) fn read_alpha_field(ctx: &Ctx, rel: &str) -> Result<FieldFile, AppBuilderError> {
    read_field_with(ctx, rel, true)
}

/// [`read_field`] for a field of which only the BOUNDARY conditions are taken from this file (the cell-centred region reads its
/// internal field through `read_vol_scalar_field`, which accepts `nonuniform List<scalar>`): accepts a nonuniform internal field
/// (gh:#1903, README round 52: `T` of the 3-D exchanger case starts at 900 K and 800 K). The returned `internal` is the first cell and
/// must not be used.
#[cfg(feature = "std")]
pub(super) fn read_field_bcs_only(ctx: &Ctx, rel: &str) -> Result<FieldFile, AppBuilderError> {
    read_field_with(ctx, rel, true)
}

/// The cells of a `nonuniform List<scalar> N ( ... )` entry's tokens (`[nonuniform, List<scalar>, N, (...)]`), or `None`
/// when the tokens are not that form. `N` must equal the list length.
pub(super) fn nonuniform_scalar_list(tokens: &[FoamValue]) -> Option<Vec<f64>> {
    match tokens {
        [FoamValue::Word(w), FoamValue::Word(t), FoamValue::Scalar(n), FoamValue::List(items)]
            if w == "nonuniform" && t == "List<scalar>" && (*n - items.len() as f64).abs() == 0.0 =>
        {
            items.iter().map(FoamValue::as_scalar).collect()
        }
        _ => None,
    }
}

#[cfg(feature = "std")]
fn read_field_with(ctx: &Ctx, rel: &str, allow_nonuniform: bool) -> Result<FieldFile, AppBuilderError> {
    let d = ctx.read(rel)?;
    // gh:#1926 (README round 66): a WRITTEN field holds `nonuniform List<vector>` (`U`) and `nonuniform` patch values; a reader that takes only the
    // boundary conditions from the file (`allow_nonuniform`) takes neither (the cells come from `read_vol_vector_field`)
    let vector_list = matches!(d.get("internalField"), Some(FoamEntry::Tokens(t)) if t.first().and_then(FoamValue::as_word) == Some("nonuniform") && t.get(1).and_then(FoamValue::as_word) == Some("List<vector>"));
    let list = match d.get("internalField") {
        Some(FoamEntry::Tokens(t)) if t.first().and_then(FoamValue::as_word) == Some("nonuniform") => {
            if !allow_nonuniform {
                return Err(ctx.e("`nonuniform` internalField is ported for alpha.<fluid> only"));
            }
            if vector_list {
                None
            } else {
                Some(nonuniform_scalar_list(t).ok_or_else(|| ctx.e("only `nonuniform List<scalar> N (...)` is read"))?)
            }
        }
        _ => None,
    };
    let internal = match &list {
        Some(l) => FoamValue::Scalar(*l.first().ok_or_else(|| ctx.e("empty nonuniform list"))?),
        None if vector_list => FoamValue::Scalar(0.0),
        None => uniform_value(ctx, d.get("internalField"))?,
    };
    let bf = ctx.sub(&d, "boundaryField")?;
    let mut bcs = Vec::new();
    let mut dicts = Vec::new();
    for (name, e) in bf.iter() {
        if let FoamEntry::SubDict(pd) = e {
            dicts.push((name.to_string(), pd.clone()));
            let ty = ctx.word(pd, "type")?;
            let value = match pd.get("value") {
                Some(FoamEntry::Word(w)) if w == "$internalField" => Some(internal.clone()),
                Some(FoamEntry::Tokens(t)) if allow_nonuniform && t.first().and_then(FoamValue::as_word) == Some("nonuniform") => None,
                Some(v @ FoamEntry::Tokens(_)) => Some(uniform_value(ctx, Some(v))?),
                _ => None,
            };
            bcs.push((name.to_string(), ty, value));
        }
    }
    Ok(FieldFile { name: rel.rsplit('/').next().unwrap_or(rel).to_string(), internal, internal_list: list, bcs, dicts })
}

/// The boundary conditions beyond `fixedValue`, `zeroGradient` and `calculated` that a scalar field may
/// carry (gh:#1801); each is accepted ONLY for the field the solver implements it for.
#[cfg(feature = "std")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtraBc {
    /// Only the three basic types.
    NoExtra,
    /// `fixedFluxPressure` (`p_rgh`).
    FixedFluxPressure,
    /// `inletOutlet` (`alpha.<fluid>`) with `phi phi.<fluid>`.
    InletOutlet,
}

#[cfg(feature = "std")]
/// Map a scalar field file to [`Bc`]s on the mesh patches: `fixedValue`, `zeroGradient` and `calculated`
/// only. **Any other type is refused by name** (before gh:#1801 it became `calculated` silently, so a case
/// selecting `fixedFluxPressure` or `inletOutlet` ran a different problem without a word).
pub(super) fn scalar_bcs(ctx: &Ctx, g: &Geometry, ff: &FieldFile) -> Result<Vec<Bc>, AppBuilderError> {
    scalar_bcs_with(ctx, g, ff, ExtraBc::NoExtra, &[])
}

#[cfg(feature = "std")]
/// [`scalar_bcs`] plus the one extra type `extra` names. `fluids` are the two fluid names (an
/// `inletOutlet` patch's `phi phi.<fluid>` is resolved against them).
pub(super) fn scalar_bcs_with(
    ctx: &Ctx,
    g: &Geometry,
    ff: &FieldFile,
    extra: ExtraBc,
    fluids: &[String],
) -> Result<Vec<Bc>, AppBuilderError> {
    let mut out = Vec::new();
    for p in &g.mesh.patches {
        if p.kind == PatchKind::Empty {
            out.push(Bc::Empty);
            continue;
        }
        let (_, ty, v) = ff
            .bcs
            .iter()
            .find(|(n, _, _)| n == &p.name)
            .ok_or_else(|| ctx.e(format!("{}: no BC for patch `{}`", ff.name, p.name)))?;
        let dict = ff.dicts.iter().find(|(n, _)| n == &p.name).map(|(_, d)| d);
        let refuse_keys = |allowed: &[&str]| -> Result<(), AppBuilderError> {
            if let Some(d) = dict {
                for (k, _) in d.iter() {
                    if k != "type" && !allowed.contains(&k) {
                        return Err(ctx.e(format!(
                            "{}.{}: entry `{k}` of a `{ty}` patch is not ported",
                            ff.name, p.name
                        )));
                    }
                }
            }
            Ok(())
        };
        out.push(match ty.as_str() {
            "fixedValue" => {
                refuse_keys(&["value"])?;
                let v = v
                    .as_ref()
                    .and_then(FoamValue::as_scalar)
                    .ok_or_else(|| ctx.e("fixedValue needs a uniform scalar value"))?;
                Bc::Fixed(vec![v; p.size])
            }
            "zeroGradient" => {
                // ESI `zeroGradientFvPatchField` reads no entry but `value`: the leftover `phi`/`inletValue` of an
                // `inletOutlet` patch the user switched to `zeroGradient` (PhaseII_Ex*'s `alpha.vapour.outlet`) are
                // never looked up, so they are ignored on a phase fraction (gh:#1802)
                if ff.name.starts_with("alpha.") {
                    refuse_keys(&["value", "phi", "inletValue"])?;
                } else {
                    refuse_keys(&["value"])?;
                }
                Bc::ZeroGradient
            }
            // `calculated` is set by field algebra (the misspelling `calulated`, which 1D_boiling's
            // `alpha.liquid` carries, is accepted for the file that upstream does not read as a field).
            "calculated" => {
                refuse_keys(&["value"])?;
                Bc::Calculated
            }
            "calulated" if ff.name.starts_with("alpha.") => Bc::Calculated,
            "fixedFluxPressure" if extra == ExtraBc::FixedFluxPressure => {
                // ESI `fixedFluxPressureFvPatchScalarField`: no `gradient` entry (a given one is not ported)
                // starts from `extrapolateInternal(); gradient() = Zero;`.
                refuse_keys(&["value"])?;
                Bc::FixedGradient(vec![0.0; p.size])
            }
            "inletOutlet" if extra == ExtraBc::InletOutlet => {
                refuse_keys(&["value", "phi", "inletValue"])?;
                let d = dict.ok_or_else(|| ctx.e("inletOutlet patch without a dictionary"))?;
                let phi = ctx.word(d, "phi")?;
                let phi_index = fluids
                    .iter()
                    .position(|n| phi == format!("phi.{n}"))
                    .ok_or_else(|| {
                        ctx.e(format!(
                            "{}.{}: inletOutlet `phi {phi}` is not one of the fluid fluxes `phi.<fluid>`",
                            ff.name, p.name
                        ))
                    })?;
                let iv = uniform_value(ctx, d.get("inletValue"))?
                    .as_scalar()
                    .ok_or_else(|| ctx.e("inletOutlet needs a uniform scalar inletValue"))?;
                Bc::InletOutlet { inlet_value: vec![iv; p.size], phi_index }
            }
            // a `symmetry` patch evaluates to the owner value for a scalar (`basicSymmetry<scalar>`; transformFvPatchField
            // coefficients are those of zeroGradient); `p` is set by algebra on every patch, so it stays `calculated` there
            "symmetry" if p.kind == PatchKind::Symmetry => {
                refuse_keys(&["value"])?;
                if ff.name == "p" { Bc::Calculated } else { Bc::ZeroGradient }
            }
            other => {
                return Err(ctx.e(format!(
                    "{}.{}: boundary condition `{other}` is not ported (the two-phase driver supports fixedValue, \
                     zeroGradient, calculated{})",
                    ff.name,
                    p.name,
                    match extra {
                        ExtraBc::NoExtra => "",
                        ExtraBc::FixedFluxPressure => ", fixedFluxPressure",
                        ExtraBc::InletOutlet => ", inletOutlet",
                    }
                )))
            }
        });
    }
    Ok(out)
}

#[cfg(feature = "std")]
/// Map a velocity field file to [`VectorBc`]s: `zeroGradient`, `fixedValue` (uniform), `slip` and
/// `pressureInletOutletVelocity` (`phi phi.<fluid>`, no `tangentialVelocity`). Anything else is refused by name.
pub(super) fn vector_bcs(
    ctx: &Ctx,
    g: &Geometry,
    ff: &FieldFile,
    fluids: &[String],
) -> Result<Vec<VectorBc>, AppBuilderError> {
    let mut out = Vec::new();
    for p in &g.mesh.patches {
        if p.kind == PatchKind::Empty {
            out.push(VectorBc::ZeroGradient);
            continue;
        }
        let (_, ty, v) = ff
            .bcs
            .iter()
            .find(|(n, _, _)| n == &p.name)
            .ok_or_else(|| ctx.e(format!("{}: no BC for patch `{}`", ff.name, p.name)))?;
        let dict = ff.dicts.iter().find(|(n, _)| n == &p.name).map(|(_, d)| d);
        let refuse_keys = |allowed: &[&str]| -> Result<(), AppBuilderError> {
            if let Some(d) = dict {
                for (k, _) in d.iter() {
                    if k != "type" && !allowed.contains(&k) {
                        return Err(ctx.e(format!(
                            "{}.{}: entry `{k}` of a `{ty}` patch is not ported",
                            ff.name, p.name
                        )));
                    }
                }
            }
            Ok(())
        };
        out.push(match ty.as_str() {
            "zeroGradient" => {
                refuse_keys(&["value"])?;
                VectorBc::ZeroGradient
            }
            "fixedValue" => {
                refuse_keys(&["value"])?;
                let v = v
                    .as_ref()
                    .and_then(FoamValue::as_vector3)
                    .ok_or_else(|| ctx.e("fixedValue needs a uniform vector value"))?;
                VectorBc::Fixed(vec![v; p.size])
            }
            "slip" => {
                refuse_keys(&["value"])?;
                VectorBc::Slip
            }
            // ESI `noSlipFvPatchVectorField` is a `fixedValue` patch with value zero (gh:#1878 item 4)
            "noSlip" => {
                refuse_keys(&["value"])?;
                VectorBc::Fixed(vec![Vector3::ZERO; p.size])
            }
            // ESI `symmetryFvPatchField` derives from `basicSymmetry`, as `slip` does (gh:#1802)
            "symmetry" if p.kind == PatchKind::Symmetry => {
                refuse_keys(&["value"])?;
                VectorBc::Slip
            }
            "pressureInletOutletVelocity" => {
                refuse_keys(&["value", "phi"])?;
                let d = dict.ok_or_else(|| ctx.e("pressureInletOutletVelocity patch without a dictionary"))?;
                let phi = ctx.word(d, "phi")?;
                let phi_index = fluids
                    .iter()
                    .position(|n| phi == format!("phi.{n}"))
                    .ok_or_else(|| {
                        ctx.e(format!(
                            "{}.{}: pressureInletOutletVelocity `phi {phi}` is not one of the fluid fluxes `phi.<fluid>`",
                            ff.name, p.name
                        ))
                    })?;
                VectorBc::PressureInletOutletVelocity { phi_index }
            }
            other => {
                return Err(ctx.e(format!(
                    "{}.{}: velocity boundary condition `{other}` is not ported (zeroGradient, fixedValue, slip and \
                     pressureInletOutletVelocity are)",
                    ff.name, p.name
                )))
            }
        });
    }
    Ok(out)
}

impl TwoPhaseCase {
    /// DEPRECATED (gh:#1361 round 13): give every `perfectGas` phase the CODATA 2019 universal gas constant the port
    /// used before round 13, in place of OpenFOAM-v2506's (`NA k` from `etc/controlDict`), which is the default.
    /// The CODATA value makes the vapour density 8.96e-7 relative larger than the real binary's.
    #[must_use]
    pub fn with_codata2019_gas_constant(mut self) -> Self {
        for f in self.fluids.iter_mut() {
            f.thermo = f.thermo.clone().with_gas_constant(outram_foam_basic_lib::prelude::GasConstant::Codata2019);
        }
        self
    }
}

impl TwoPhaseCase {
    /// DEPRECATED (gh:#1815): apply every `fixedPower` `powerTimeProfile` as the closed form `initial * prof(t)` the
    /// port used before it translated upstream's recursion `prof(t) * pdOld / prof(t - dt)`
    /// ([`FixedPower::with_closed_form_power_profile`]). Upstream's recursion is the default.
    #[must_use]
    pub fn with_closed_form_power_profile(mut self) -> Self {
        self.structure.power =
            self.structure.power.into_iter().map(|p| p.map(FixedPower::with_closed_form_power_profile)).collect();
        self
    }
}

impl TwoPhaseCase {
    /// The port's behaviour BEFORE gh:#1361 round 7: run GeN-Foam's `implicitUpwind` alpha solver
    /// (`solveAlphaImplicitUpwind.H`, `solvePhase` = the vapour unless the case names one) in place of
    /// the `MULES` the case selects, and record it in [`TwoPhaseCase::deviations`]. Not what upstream does for a
    /// case that selects MULES (the default now); needs an `alpha.<phase>` linear-solver entry in `fvSolution`.
    ///
    /// # Errors
    /// The case has no linear solver for `alpha.<solvePhase>`.
    pub fn with_alpha_solver_implicit_upwind(mut self) -> Result<Self, crate::error::AppBuilderError> {
        if self.controls.alpha != AlphaSolver::ImplicitUpwind {
            if self.controls.alpha_solver_tolerance.is_nan() {
                return Err(crate::error::AppBuilderError::Case {
                    case: crate::path::PathBuf::new(),
                    msg: "no linear solver for alpha.<phase> in fvSolution: implicitUpwind cannot run".into(),
                });
            }
            self.controls.alpha = AlphaSolver::ImplicitUpwind;
            self.deviations.push(format!(
                "alpha solver `MULES` is not run; ran GeN-Foam's implicitUpwind (solveAlphaImplicitUpwind.H) with solvePhase {}",
                self.fluids[self.controls.solve_phase].name
            ));
        }
        Ok(self)
    }

    /// The ablation of [`TwoPhaseControls::p_patch_from_file`]: the patch values of `p` start as `p_rgh`'s
    /// (and are reset to `p_rgh + rho gh` after the thermo construction), as the port did before gh:#1361
    /// round 5. Differs from GeN-Foam 652b3da in steps 1 and 2 only (the `U.vapour` inlet cell of an absent phase).
    #[must_use]
    pub fn without_p_patch_from_file(mut self) -> Self {
        self.controls.p_patch_from_file = false;
        for (i, v) in self.p.b.iter_mut().enumerate() {
            *v = self.p_rgh.b[i];
        }
        self
    }

    #[cfg(feature = "std")]
    /// Read the `fluidRegion` of a GeN-Foam case at `root` (the directory
    /// holding `0/`, `constant/`, `system/`).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] naming the first file, keyword or model
    /// selection the port cannot honour.
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path) -> Result<Self, AppBuilderError> {
        Self::read_with_radial_ring_areas(root, false)
    }

    #[cfg(feature = "std")]
    /// [`TwoPhaseCase::read`] with round 35's CONTROL for `nuclearFuelPin` (`corrected_ring_areas = true` fixes the two ring-area
    /// slips of upstream; never the default).
    ///
    /// # Errors
    /// As [`TwoPhaseCase::read`].
    #[allow(clippy::too_many_lines)]
    pub fn read_with_radial_ring_areas(root: &Path, corrected_ring_areas: bool) -> Result<Self, AppBuilderError> {
        let ctx = Ctx {
            root: root.to_path_buf(),
        };
        let region = "fluidRegion";
        let poly = root.join(format!("constant/{region}/polyMesh"));
        let mesh = read_poly_mesh(&poly)?;
        let zones = read_cell_zones(&poly)?;
        let g = Geometry::new_openfoam(mesh.clone());
        let n = g.n_cells();
        let deviations: Vec<String> = Vec::new();
        let zone_cells = |names: &str| -> Result<Vec<usize>, AppBuilderError> {
            let mut cells = Vec::new();
            for z in names.split(':') {
                let zone = zones
                    .iter()
                    .find(|cz| cz.name == z)
                    .ok_or_else(|| ctx.e(format!("unknown cellZone `{z}`")))?;
                cells.extend(zone.cells.iter().copied());
            }
            Ok(cells)
        };

        // gravity
        let gd = ctx.read(&format!("constant/{region}/g"))?;
        let gravity = match gd.get("value") {
            Some(FoamEntry::List(v)) if v.len() == 3 => Vector3::new(
                v[0].as_scalar().unwrap_or(0.0),
                v[1].as_scalar().unwrap_or(0.0),
                v[2].as_scalar().unwrap_or(0.0),
            ),
            _ => return Err(ctx.e("g: missing `value (x y z)`")),
        };

        // controlDict
        let cd = ctx.read("system/controlDict")?;
        let start_time = scalar(&cd, "startTime").unwrap_or(0.0);
        let end_time = ctx.f64(&cd, "endTime")?;
        let delta_t = ctx.f64(&cd, "deltaT")?;
        let write_interval = match word(&cd, "writeControl").as_deref() {
            Some("adjustableRunTime") => scalar(&cd, "writeInterval"),
            _ => None,
        };
        let time_controls = TimeControls {
            adjust_time_step: bool_entry(&cd, "adjustTimeStep").unwrap_or(false),
            max_co: scalar(&cd, "maxCo").unwrap_or(1.0),
            max_co_two_phase: scalar(&cd, "maxCoTwoPhase"),
            margin_to_phase_change: scalar(&cd, "marginToPhaseChange"),
            max_delta_t: scalar(&cd, "maxDeltaT").unwrap_or(f64::MAX),
            write_interval,
            start_time,
            esi_adjust_delta_t: true,
        };
        if let Some(rs) = cd.get_dict("regionSolvers") {
            if word(rs, region).as_deref() != Some("twoPhase") {
                return Err(ctx.e("regionSolvers.fluidRegion is not twoPhase"));
            }
        }

        // phaseProperties
        let pp = ctx.read_with_includes(&format!("constant/{region}/phaseProperties"))?;
        let names = [ctx.word(&pp, "fluid1")?, ctx.word(&pp, "fluid2")?];

        // 0/ fields
        let zero = format!("0/{region}");
        let p_rgh_f = read_field(&ctx, &format!("{zero}/p_rgh"))?;
        let p_rgh_bc = scalar_bcs_with(&ctx, &g, &p_rgh_f, ExtraBc::FixedFluxPressure, &names)?;
        let p_rgh = Scalar::uniform(&g, p_rgh_f.internal.as_scalar().unwrap_or(0.0), p_rgh_bc);
        let p_f = read_field(&ctx, &format!("{zero}/p"))?;
        let mut p = Scalar::uniform(&g, p_f.internal.as_scalar().unwrap_or(0.0), vec![Bc::Calculated; mesh.patches.len()]);
        // `p_` is `READ_IF_PRESENT`: its patch values are the file's (`calculated`, `value`), NOT `p_rgh`'s.
        // They are what the thermo packages are constructed from, so they set the construction-time
        // patch densities and the `oldTime()` copies of them (gh:#1361 round 5, `U.vapour` at the
        // inlet of an absent phase). `p_rgh`'s patch is the fallback when the file gives no uniform value.
        let patch_p_from_file = |f: usize| -> Option<f64> {
            let name = &g.mesh.patches[g.patch_of[f - g.n_internal()]].name;
            p_f.bcs
                .iter()
                .find(|(n, ty, _)| n == name && ty == "calculated")
                .and_then(|(_, _, v)| v.as_ref().and_then(FoamValue::as_scalar))
        };
        for f in g.n_internal()..g.n_faces() {
            if g.active[f] {
                p.b[f - g.n_internal()] = patch_p_from_file(f).unwrap_or(p_rgh.b[f - g.n_internal()]);
            }
        }

        // fluids
        let mut fluids = Vec::new();
        let mut alphas = Vec::new();
        let mut alpha_lists: Vec<Option<Vec<f64>>> = Vec::new();
        let mut alpha_bcs: Vec<Option<Vec<Bc>>> = Vec::new();
        for name in &names {
            let a_path = root.join(format!("{zero}/alpha.{name}"));
            alphas.push(if a_path.exists() {
                let af = read_alpha_field(&ctx, &format!("{zero}/alpha.{name}"))?;
                if let Some(l) = &af.internal_list {
                    if l.len() != g.n_cells() {
                        return Err(ctx.e(format!("alpha.{name}: {} values for {} cells", l.len(), g.n_cells())));
                    }
                }
                alpha_lists.push(af.internal_list.clone());
                alpha_bcs.push(Some(scalar_bcs_with(&ctx, &g, &af, ExtraBc::InletOutlet, &names)?));
                Some(af.internal.as_scalar().unwrap_or(0.0))
            } else {
                alpha_lists.push(None);
                alpha_bcs.push(None);
                None
            });
        }
        if alpha_lists.iter().any(Option::is_some) && alpha_lists.iter().zip(&alphas).any(|(l, a)| l.is_none() && a.is_none()) {
            return Err(ctx.e("a nonuniform alpha.<fluid> without the other fluid's file is not ported (the other would be `1 - alpha`)"));
        }
        if alphas[0].is_none() && alphas[1].is_none() {
            return Err(ctx.e("neither alpha field is present"));
        }
        let a0 = alphas[0].unwrap_or(1.0 - alphas[1].unwrap_or(0.0));
        let a1 = alphas[1].unwrap_or(1.0 - a0);
        let mut dd_tables: Vec<(usize, String, Vec<(String, f64)>)> = Vec::new();
        let mut turb_dicts: Vec<FoamDict> = Vec::new();
        for (k, name) in names.iter().enumerate() {
            let props = ctx.sub(&pp, &format!("{name}Properties"))?;
            let state = ctx.word(props, "stateOfMatter")?;
            let is_liquid = state == "liquid";
            let residual_alpha = scalar(props, "residualAlpha").unwrap_or(1e-6);
            let tra = scalar(props, "thermoResidualAlpha").unwrap_or(0.0);
            let dd = match props.get_dict("dispersedDiameterModel") {
                Some(d) if word(d, "type").as_deref() == Some("constant") => ctx.f64(d, "value")?,
                Some(d) if word(d, "type").as_deref() == Some("byRegime") => {
                    // gh:#1225: one constant sub-model per regime of the named map
                    let mut table = Vec::new();
                    for (rn, e) in d.iter() {
                        if let FoamEntry::SubDict(sd) = e {
                            if ctx.word(sd, "type")? != "constant" {
                                return Err(ctx.e(format!("{name}: byRegime diameter sub-models must be constant")));
                            }
                            table.push((rn.to_string(), ctx.f64(sd, "value")?));
                        }
                    }
                    dd_tables.push((k, ctx.word(d, "regimeMap")?, table));
                    0.0
                }
                Some(_) => {
                    return Err(ctx.e(format!("{name}: only a constant or byRegime dispersedDiameterModel is ported")))
                }
                None => 0.0,
            };
            let thermo = read_thermo(&ctx, &format!("constant/{region}/thermophysicalProperties.{name}"))?;
            let tp = ctx.read(&format!("constant/{region}/turbulenceProperties.{name}"))?;
            turb_dicts.push(tp);
            let t_f = read_field(&ctx, &format!("{zero}/T.{name}"))?;
            let t_bc = scalar_bcs(&ctx, &g, &t_f)?;
            let he_bc: Vec<Bc> = t_bc
                .iter()
                .map(|b| match b {
                    Bc::Fixed(v) => Bc::Fixed(v.clone()),
                    Bc::Empty => Bc::Empty,
                    _ => Bc::ZeroGradient,
                })
                .collect();
            let t = Scalar::uniform(&g, t_f.internal.as_scalar().unwrap_or(0.0), t_bc);
            let u_f = read_field(&ctx, &format!("{zero}/U.{name}"))?;
            let u0 = u_f.internal.as_vector3().ok_or_else(|| ctx.e("U must be uniform"))?;
            let u_bc = vector_bcs(&ctx, &g, &u_f, &names)?;
            let mut u = Vector::uniform(&g, u0);
            // Initial patch values as upstream's constructors give them: `fixedValue` its value; `slip` evaluated
            // (`basicSymmetry` dictionary constructor); `pressureInletOutletVelocity` the file's `value`.
            {
                let mut tmp = u.clone();
                tmp.bc = u_bc
                    .iter()
                    .map(|b| match b {
                        VectorBc::PressureInletOutletVelocity { .. } => VectorBc::ZeroGradient,
                        o => o.clone(),
                    })
                    .collect();
                tmp.correct_bc(&g);
                u.b = tmp.b;
                u.bc = u_bc;
                for (pi, patch) in g.mesh.patches.iter().enumerate() {
                    if matches!(u.bc[pi], VectorBc::PressureInletOutletVelocity { .. }) {
                        let file_value = u_f
                            .bcs
                            .iter()
                            .find(|(n, _, _)| n == &patch.name)
                            .and_then(|(_, _, v)| v.as_ref().and_then(FoamValue::as_vector3))
                            .unwrap_or(u0);
                        for kk in 0..patch.size {
                            u.b[patch.start + kk - g.n_internal()] = file_value;
                        }
                    }
                }
            }
            let a_bc = match alpha_bcs.get(k).cloned().flatten() {
                Some(b) => b
                    .into_iter()
                    .map(|b| match b {
                        // `calculated` on a phase fraction is algebra-set: the port's earlier reading
                        Bc::Calculated => Bc::ZeroGradient,
                        o => o,
                    })
                    .collect(),
                None => g.default_bcs(),
            };
            let mut alpha = Scalar::uniform(&g, if k == 0 { a0 } else { a1 }, g.default_bcs());
            if let Some(l) = &alpha_lists[k] {
                alpha.c = l.clone();
            }
            alpha.bc = a_bc;
            if alpha_lists[k].is_some() {
                alpha.correct_bc(&g);
            }
            fluids.push(Phase::new(
                &g,
                name,
                is_liquid,
                thermo,
                alpha,
                u,
                t,
                he_bc,
                residual_alpha,
                tra,
                dd,
            ));
        }
        let mut fluids: [Phase; 2] = [fluids[0].clone(), fluids[1].clone()];

        // structure
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_s = vec![0.0; n];
        // ~~`1.0` outside every structure zone~~ gh:#1967 round 83: `structure::Dh_` is constructed `SMALL` (`structure.C:74-84`) and set
        // only in the cells of a `structureProperties` zone (`:300-317`), as round 82 fixed for onePhase
        let mut dh_s = vec![super::turbulence::SMALL; n];
        // `structure::cells_`: every zone's cells, appended in `structureProperties` order (`structure.C:283-286`)
        let mut structure_cells: Vec<usize> = Vec::new();
        // gh:#1967 round 83: upstream reads `alpha.structure` from the start directory when present (`alphaHeaderOk`: the zone
        // `volumeFraction`s are then NOT applied, and `alpha.passiveStructure` starts from it, `structure.C:138-148`, `:300-317`)
        if root.join(format!("0/{region}/alpha.structure")).exists() {
            return Err(ctx.e("0/fluidRegion/alpha.structure (a structure fraction read from the start directory) is not ported"));
        }
        let mut power: Vec<Option<FixedPower>> = vec![None; n];
        let mut power_off = None;
        let mut power_density_file: Option<Vec<f64>> = None;
        for (key, e) in sp.iter() {
            let FoamEntry::SubDict(zd) = e else { continue };
            if key == "powerOffCriterionModel" {
                match word(zd, "type").as_deref() {
                    Some("timer") => {
                        power_off = Some(PowerOffCriterion::timer(Time::new::<second>(ctx.f64(zd, "time")?)));
                    }
                    other => return Err(ctx.e(format!("powerOffCriterionModel {other:?} not ported"))),
                }
                continue;
            }
            let cells = zone_cells(key)?;
            let vf = ctx.f64(zd, "volumeFraction")?;
            let dh = ctx.f64(zd, "Dh")?;
            for &c in &cells {
                alpha_s[c] = vf;
                dh_s[c] = dh;
            }
            structure_cells.extend(cells.iter().copied());
            for (k2, _) in zd.iter() {
                if !matches!(k2, "volumeFraction" | "Dh" | "powerModel") {
                    return Err(ctx.e(format!("structureProperties.{key}.{k2} is not ported")));
                }
            }
            if let Some(pm) = zd.get_dict("powerModel") {
                let ty = word(pm, "type");
                if ty.as_deref().is_some_and(|t| super::radial_power::RADIAL_TYPES.contains(&t)) {
                    // built after the loop by `read_radial_power_models`, which needs every zone's structure alpha (gh:#1884)
                    continue;
                }
                if let Some(t) = ty.as_deref().filter(|t| matches!(*t, "nuclearFuelFMU" | "fixedPowerFMU" | "fixedTemperatureFMU")) {
                    return Err(ctx.e(super::radial_power::refusal_reason(t)));
                }
                if ty.as_deref() != Some("fixedPower") {
                    return Err(ctx.e("only the fixedPower and the radial-conduction powerModels (heatedPin, nuclearFuelPin, lumpedNuclearStructure, interpolatedNuclearFuelPin) are ported"));
                }
                let power_profile = match pm.get_dict("powerTimeProfile") {
                    Some(d) => Some(read_table_time_profile(&ctx, d, "powerTimeProfile")?),
                    None if pm.get("powerTimeProfile").is_some() => {
                        return Err(ctx.e("powerTimeProfile must be a dictionary"))
                    }
                    None => None,
                };
                let pa = scalar(pm, "volumeFraction").unwrap_or(vf);
                let rho_cp = match (scalar(pm, "rho"), scalar(pm, "Cp"), scalar(pm, "rhoCp")) {
                    (Some(r), Some(cp), _) => r * cp,
                    (_, _, Some(rc)) => rc,
                    _ => return Err(ctx.e("fixedPower: specify rho and Cp, or rhoCp")),
                };
                let ia = ctx.f64(pm, "volumetricArea")?;
                let t0 = ctx.f64(pm, "T")?;
                // `powerDensity` of the dictionary, else the cell values of `0/<region>/powerDensity.fixedPower`
                // (`fixedPower::powerDensity_` is READ_IF_PRESENT; `setRegionField` overrides it only where the key exists; gh:#1802)
                let q_dict = scalar(pm, "powerDensity");
                if q_dict.is_none() && power_density_file.is_none() {
                    let path = root.join(format!("0/{region}/powerDensity.fixedPower"));
                    let (f, _) = outram_foam_basic_lib::io::read_vol_scalar_field_esi(&path, mesh.clone())
                        .map_err(|e| ctx.e(format!("fixedPower has no `powerDensity` and {}: {e}", path.display())))?;
                    power_density_file = Some(f.internal.as_slice().to_vec());
                }
                for &c in &cells {
                    let q = match (q_dict, &power_density_file) {
                        (Some(q), _) => q,
                        (None, Some(file)) => file[c],
                        (None, None) => unreachable!("loaded above"),
                    };
                    // fixedPower.C: alphaRhoCp = max(alpha rhoCp, 1e-69)
                    let model = FixedPower::new(
                        Ratio::new::<ratio>(pa),
                        uom::si::f64::ReciprocalLength::new::<reciprocal_meter>(ia),
                        uom::si::f64::VolumetricPowerDensity::new::<watt_per_cubic_meter>(q),
                        uom::si::f64::VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(
                            (pa * rho_cp).max(1e-69),
                        ),
                        ThermodynamicTemperature::new::<kelvin>(t0),
                    );
                    power[c] = Some(match &power_profile {
                        Some(pr) => model.with_power_profile(pr.clone()),
                        None => model,
                    });
                }
            }
        }
        // ---- the radial-conduction power models (gh:#1884): the one-phase reader, shared (`radial_power.rs`)
        let has_radial = sp.iter().any(|(_, e)| match e {
            FoamEntry::SubDict(zd) => zd
                .get_dict("powerModel")
                .and_then(|pm| word(pm, "type"))
                .is_some_and(|t| super::radial_power::RADIAL_TYPES.contains(&t.as_str())),
            _ => false,
        });
        let read_cells = |name: &str| -> Result<Option<Vec<f64>>, AppBuilderError> {
            let path = root.join(format!("0/{region}/{name}"));
            if !has_radial || !path.exists() {
                return Ok(None);
            }
            let (f, _) = outram_foam_basic_lib::io::read_vol_scalar_field_esi(&path, mesh.clone())
                .map_err(|e| ctx.e(format!("{name}: {e}")))?;
            Ok(Some(f.internal.as_slice().to_vec()))
        };
        let q_file = read_cells("powerDensityNeutronics")?;
        let hp_file = read_cells("powerDensity.heatedPin")?;
        let radial = super::radial_power::read_radial_power_models(
            &ctx,
            &sp,
            &zone_cells,
            &alpha_s,
            &mesh.cell_volumes,
            &root.join(format!("0/{region}")),
            corrected_ring_areas,
            q_file,
            hp_file,
        )?;
        if radial.needs_laplacian {
            // `fvc::laplacian(Tmatrix)` is the `uncorrected` form only
            let schemes = ctx.read(&format!("system/{region}/fvSchemes"))?;
            let d = ctx.sub(&schemes, "laplacianSchemes")?;
            let uncorrected = match d.get("default") {
                Some(FoamEntry::Tokens(vs)) => {
                    vs.iter().any(|v| matches!(v, outram_foam_basic_lib::io::FoamValue::Word(w) if w == "uncorrected"))
                }
                _ => false,
            };
            if !uncorrected {
                return Err(ctx.e("lumpedNuclearStructure with kappaMatrix != 0 needs laplacianSchemes `Gauss linear uncorrected` (the only fvc::laplacian ported)"));
            }
        }
        let structure = Structure::new(&g, alpha_s, dh_s, power, power_off)
            .with_radial_power_models(radial.models, radial.q_neutronics, radial.needs_laplacian)
            .with_cells(structure_cells);

        // regime map
        let mut regime_map_name: Option<String> = None;
        let mut regime_labels: Vec<String> = Vec::new();
        let regime_map = match pp.get_dict("regimeMapModels") {
            None => None,
            Some(rm) => {
                let maps: Vec<(&str, &FoamEntry)> = rm.iter().collect();
                if maps.is_empty() {
                    None
                } else if maps.len() != 1 {
                    return Err(ctx.e("exactly one regime map is supported"));
                } else {
                let FoamEntry::SubDict(md) = maps[0].1 else {
                    return Err(ctx.e("regime map must be a dictionary"));
                };
                regime_map_name = Some(maps[0].0.to_string());
                if word(md, "type").as_deref() == Some("twoParameters") {
                    let parse_param = |key: &str| -> Result<RegimeParameter, AppBuilderError> {
                        let w = ctx.word(md, key)?;
                        for (k, nm) in names.iter().enumerate() {
                            if w == format!("normalized.alpha.{nm}") {
                                return Ok(RegimeParameter::NormalizedAlpha(k));
                            }
                            if w == format!("alphaRhoMagU.{nm}") {
                                return Ok(RegimeParameter::AlphaRhoMagU(k));
                            }
                        }
                        Err(ctx.e(format!("regime parameter `{w}` not supported")))
                    };
                    let parameter1 = parse_param("parameter1")?;
                    let parameter2 = parse_param("parameter2")?;
                    let mut points: Vec<(String, [f64; 2])> = Vec::new();
                    for (pn, e) in ctx.sub(md, "regimePoints")?.iter() {
                        let FoamEntry::List(v) = e else {
                            return Err(ctx.e(format!("regimePoints.{pn} must be a list (x y)")));
                        };
                        match (v.first().and_then(|x| x.as_scalar()), v.get(1).and_then(|x| x.as_scalar())) {
                            (Some(x), Some(y)) => points.push((pn.to_string(), [x, y])),
                            _ => return Err(ctx.e(format!("regimePoints.{pn} must be two numbers"))),
                        }
                    }
                    let mut regimes: Vec<(String, Vec<String>)> = Vec::new();
                    for (rn, e) in ctx.sub(md, "regimes")?.iter() {
                        let FoamEntry::List(v) = e else {
                            return Err(ctx.e(format!("regimes.{rn} must be a list of point names")));
                        };
                        let mut pts = Vec::new();
                        for x in v {
                            match x {
                                FoamValue::Word(w) | FoamValue::Str(w) => pts.push(w.clone()),
                                _ => return Err(ctx.e(format!("regimes.{rn} must list point names"))),
                            }
                        }
                        regimes.push((rn.to_string(), pts));
                    }
                    let mode = match word(md, "interpolationMode").as_deref() {
                        Some("quadratic") => RegimeInterpolationMode::Quadratic,
                        _ => RegimeInterpolationMode::Linear,
                    };
                    let width = ctx.f64(md, "interpolationWidth")?;
                    let map = RegimeMap2D::new(&points, &regimes, width, mode).map_err(|e| ctx.e(e))?;
                    regime_labels = map.regime_names().to_vec();
                    Some(RegimeMapSelection::TwoParameters { map, parameter1, parameter2 })
                } else {
                if word(md, "type").as_deref() != Some("oneParameter") {
                    return Err(ctx.e("only the oneParameter and twoParameters regime maps are ported"));
                }
                let param = ctx.word(md, "parameter")?;
                let k = names
                    .iter()
                    .position(|nm| param == format!("normalized.alpha.{nm}"))
                    .ok_or_else(|| ctx.e(format!("regime parameter `{param}` not supported")))?;
                let mut bounds = Vec::new();
                for (rn, e) in ctx.sub(md, "regimeBounds")?.iter() {
                    if let FoamEntry::List(v) = e {
                        regime_labels.push(rn.to_string());
                        bounds.push(RegimeBound {
                            name: rn.to_string(),
                            lower: v[0].as_scalar().unwrap_or(0.0),
                            upper: v[1].as_scalar().unwrap_or(0.0),
                        });
                    }
                }
                let mode = match word(md, "interpolationMode").as_deref() {
                    Some("quadratic") => RegimeInterpolationMode::Quadratic,
                    _ => RegimeInterpolationMode::Linear,
                };
                Some(RegimeMapSelection::OneParameter(RegimeMap1D::new(bounds, mode), k))
                }
                }
            }
        };
        // `byRegime` diameters of the fluids: every regime of the (single) map needs a sub-model
        for (k, rm, table) in dd_tables {
            if regime_map_name.as_deref() != Some(rm.as_str()) {
                return Err(ctx.e(format!("regimeMap `{rm}` of the dispersedDiameterModel is not in regimeMapModels")));
            }
            let m = ByRegimeModel::per_regime(&regime_labels, |n| table.iter().find(|(t, _)| t == n).map(|(_, v)| *v))
                .map_err(|e| ctx.e(e))?;
            fluids[k].dispersed_diameter_by_regime = Some(m);
        }

        // physics models
        let pm = ctx.sub(&pp, "physicsModels")?;
        let pair = |a: &str, b: &str, d: &FoamDict| -> Option<FoamDict> {
            d.get_dict(&format!("{a}.{b}"))
                .or_else(|| d.get_dict(&format!("{b}.{a}")))
                .cloned()
        };
        let drag = ctx.sub(pm, "dragModels")?;
        let ffd = pair(&names[0], &names[1], drag).ok_or_else(|| ctx.e("no fluid-fluid drag model"))?;
        let ff_drag = match ctx.word(&ffd, "type")?.as_str() {
            "SchillerNaumann" => FfDragCoefficient::SchillerNaumann,
            "Wallis" => FfDragCoefficient::Wallis,
            "Bestion" => FfDragCoefficient::Bestion,
            "BestionTRACE" => FfDragCoefficient::BestionTrace,
            "Autruffe" => FfDragCoefficient::Autruffe,
            other => return Err(ctx.e(format!("FF drag `{other}` not wired"))),
        };
        let mut fs_drag: [Vec<Option<FsWallFriction>>; 2] = [vec![None; n], vec![None; n]];
        for (k, name) in names.iter().enumerate() {
            if let Some(d) = pair(name, "structure", drag) {
                for (zk, e) in d.iter() {
                    let FoamEntry::SubDict(md) = e else { continue };
                    let model = match ctx.word(md, "type")?.as_str() {
                        "ReynoldsPower" => FsWallFriction::ReynoldsPower {
                            coeff: ctx.f64(md, "coeff")?,
                            exp: ctx.f64(md, "exp")?,
                            constant: scalar(md, "const").unwrap_or(0.0),
                        },
                        other => return Err(ctx.e(format!("FS drag `{other}` not wired"))),
                    };
                    for c in zone_cells(zk)? {
                        fs_drag[k][c] = Some(model);
                    }
                }
            }
        }
        let two_phase_multiplier = match pm.get_dict("twoPhaseDragMultiplierModel") {
            None => None,
            Some(d) => {
                let mf = ctx.word(d, "multiplierFluid")?;
                let k = names.iter().position(|x| *x == mf).ok_or_else(|| ctx.e("unknown multiplierFluid"))?;
                let model = match ctx.word(d, "type")?.as_str() {
                    "Kaiser88" => TwoPhaseDragMultiplier::Kaiser88,
                    "Kaiser74" => TwoPhaseDragMultiplier::Kaiser74,
                    "ChenKalish" => TwoPhaseDragMultiplier::ChenKalish,
                    "KottowskiSavatteri" => TwoPhaseDragMultiplier::KottowskiSavatteri,
                    "LockhartMartinelli" => TwoPhaseDragMultiplier::LockhartMartinelli {
                        c: scalar(d, "C").unwrap_or(20.0),
                        max_phi2: scalar(d, "maxPhi2").unwrap_or(1000.0),
                    },
                    other => return Err(ctx.e(format!("multiplier `{other}` not wired"))),
                };
                Some((model, k, scalar(d, "maxPhi2").unwrap_or(1000.0)))
            }
        };
        let htm = ctx.sub(pm, "heatTransferModels")?;
        let mut chf_tables: Vec<LookUpTableChf> = Vec::new();
        let mut fs_htc: [Vec<Option<FsHtcModel>>; 2] = [vec![None; n], vec![None; n]];
        for (k, name) in names.iter().enumerate() {
            if let Some(d) = pair(name, "structure", htm) {
                for (zk, e) in d.iter() {
                    let FoamEntry::SubDict(md) = e else { continue };
                    let model = read_fs_htc(&ctx, md, &mut chf_tables)?;
                    for c in zone_cells(zk)? {
                        fs_htc[k][c] = Some(model);
                    }
                }
            }
        }
        let mut ff_htc = [None, None];
        if let Some(d) = pair(&names[0], &names[1], htm) {
            for (k, name) in names.iter().enumerate() {
                if let Some(md) = d.get_dict(name) {
                    ff_htc[k] = Some(match ctx.word(md, "type")?.as_str() {
                        "NusseltReynoldsPrandtlPower" | "Nusselt" => FfHtcModel::Nusselt {
                            a: ctx.f64(md, "const")?,
                            b: ctx.f64(md, "coeff")?,
                            c: ctx.f64(md, "expRe")?,
                            d: ctx.f64(md, "expPr")?,
                        },
                        "constant" => FfHtcModel::Constant(ctx.f64(md, "value")?),
                        other => return Err(ctx.e(format!("FF htc `{other}` not wired"))),
                    });
                }
            }
        }
        let virtual_mass = match pm.get_dict("virtualMassCoefficientModel") {
            Some(d) if word(d, "type").as_deref() == Some("constant") => Some(ctx.f64(d, "value")?),
            Some(_) => return Err(ctx.e("only a constant virtual-mass coefficient is wired")),
            None => None,
        };
        let geo = ctx.sub(pm, "pairGeometryModels")?;
        let ffg = pair(&names[0], &names[1], geo).ok_or_else(|| ctx.e("no fluid-fluid pairGeometryModels"))?;
        let dispersion_fluid1 = read_by_regime(&ctx, ctx.sub(&ffg, "dispersionModel")?, |d| {
            let disp = ctx.word(d, "dispersedPhase")?;
            let v = scalar(d, "value").unwrap_or(1.0);
            Ok(if disp == names[0] { v } else { 1.0 - v })
        })?;
        let ia = ctx.sub(&ffg, "interfacialAreaDensityModel")?;
        let mut interfacial_area_by_regime = None;
        let mut spherical_cutoff_alpha = 0.9;
        match word(ia, "type").as_deref() {
            Some("spherical") => spherical_cutoff_alpha = scalar(ia, "cutoffAlpha").unwrap_or(0.9),
            Some("byRegime") => {
                let rm = ctx.word(ia, "regimeMap")?;
                if regime_map_name.as_deref() != Some(rm.as_str()) {
                    return Err(ctx.e(format!("regimeMap `{rm}` of the interfacialAreaDensityModel is not in regimeMapModels")));
                }
                let mut table: Vec<(String, f64)> = Vec::new();
                for (rn, e) in ia.iter() {
                    if let FoamEntry::SubDict(sd) = e {
                        if word(sd, "type").as_deref() != Some("spherical") {
                            return Err(ctx.e("byRegime interfacialAreaDensityModel: only spherical sub-models are wired"));
                        }
                        table.push((rn.to_string(), scalar(sd, "cutoffAlpha").unwrap_or(0.9)));
                    }
                }
                interfacial_area_by_regime = Some(
                    ByRegimeModel::per_regime(&regime_labels, |n| table.iter().find(|(t, _)| t == n).map(|(_, v)| *v))
                        .map_err(|e| ctx.e(e))?,
                );
            }
            _ => return Err(ctx.e("only the spherical interfacial area model is wired (alone or byRegime)")),
        }
        let mut contact_partition = None;
        for (k, name) in names.iter().enumerate() {
            if let Some(d) = pair(name, "structure", geo) {
                let cp = read_by_regime(&ctx, ctx.sub(&d, "contactPartitionModel")?, |d| ctx.f64(d, "value"))?;
                contact_partition = Some((k, cp));
                break;
            }
        }
        let contact_partition = contact_partition.ok_or_else(|| ctx.e("no contactPartitionModel"))?;
        let phase_change = match pm.get_dict("phaseChangeModel") {
            None => None,
            Some(d) => {
                if ctx.word(d, "type")? != "heatDriven" {
                    return Err(ctx.e("only the heatDriven phase-change model is wired"));
                }
                let rate = match ctx.word(d, "mode")?.as_str() {
                    "conductionLimited" => PhaseChangeRateModel::HeatDrivenConductionLimited,
                    "twoPhaseDriven" => PhaseChangeRateModel::HeatDrivenTwoPhaseDriven,
                    other => return Err(ctx.e(format!("heatDriven mode `{other}` not wired"))),
                };
                let lh = ctx.sub(d, "latentHeatModel")?;
                let latent_heat = match ctx.word(lh, "type")?.as_str() {
                    "FinkLeibowitz" => LatentHeatModel::FinkLeibowitz,
                    "water" => LatentHeatModel::Water,
                    "fromThermophysicalProperties" => LatentHeatModel::FromThermophysicalProperties,
                    other => return Err(ctx.e(format!("latent heat `{other}` not wired"))),
                };
                let sm = ctx.sub(d, "saturationModel")?;
                let saturation = match ctx.word(sm, "type")?.as_str() {
                    "BrowningPotter" => SaturationModel::BrowningPotter,
                    "water" => SaturationModel::Water,
                    "waterTRACE" => SaturationModel::WaterTrace,
                    other => return Err(ctx.e(format!("saturation `{other}` not wired"))),
                };
                Some(PhaseChangeConfig {
                    rate,
                    latent_heat,
                    adjust_latent_heat: bool_entry(lh, "adjust").unwrap_or(false),
                    saturation,
                    residual_interfacial_area: scalar(d, "residualInterfacialArea").unwrap_or(1e-3),
                })
            }
        };
        let models = TwoPhaseModels {
            ff_drag,
            fs_drag,
            two_phase_multiplier,
            fs_htc,
            chf_tables,
            ff_htc,
            virtual_mass,
            regime_map,
            dispersion_fluid1,
            contact_partition,
            spherical_cutoff_alpha,
            interfacial_area_by_regime,
            phase_change,
        };

        // fvSolution
        let fs = ctx.read(&format!("system/{region}/fvSolution"))?;
        let pimple = ctx.sub(&fs, "PIMPLE")?;
        if word(pimple, "partialEliminationMode").as_deref() != Some("implicit") {
            return Err(ctx.e("only partialEliminationMode implicit is ported"));
        }
        if word(pimple, "momentumMode").as_deref() != Some("faceCentered") {
            return Err(ctx.e("only momentumMode faceCentered is ported"));
        }
        if scalar(pimple, "porousInterfaceSharpness").unwrap_or(0.0) != 0.0 {
            return Err(ctx.e("porousInterfaceSharpness != 0 is not ported"));
        }
        let solvers = ctx.sub(&fs, "solvers")?;
        let p_rgh_tol = match lookup_regex(solvers, "p_rgh") {
            Some(FoamEntry::SubDict(d)) => ctx.f64(d, "tolerance")?,
            _ => return Err(ctx.e("no p_rgh solver")),
        };
        let (h_tol, h_min) = match lookup_regex(solvers, &format!("h.{}", names[0])) {
            Some(FoamEntry::SubDict(d)) => (ctx.f64(d, "tolerance")?, scalar(d, "minIter").unwrap_or(0.0) as usize),
            _ => return Err(ctx.e("no h.* solver")),
        };
        // OpenFOAM resolves a name to the exact keyword, else the LAST matching pattern: for `h.liquid` a catch-all `".*"` declared after
        // `"h.*"` wins (upstream's log: `diagonalPBiCGStab: Solving for h.liquid`)
        let h_pbicgstab = match lookup_pattern_last(solvers, &format!("h.{}", names[0])) {
            Some(FoamEntry::SubDict(d))
                if word(d, "solver").as_deref() == Some("PBiCGStab") && word(d, "preconditioner").as_deref() == Some("diagonal") =>
            {
                Some(HPbicgstab {
                    tolerance: ctx.f64(d, "tolerance")?,
                    rel_tol: scalar(d, "relTol").unwrap_or(0.0),
                    min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
                })
            }
            _ => None,
        };
        // gh:#1957 round 83: `h.<fluid>` resolved per fluid (exact keyword, else the last matching pattern); a `smoothSolver` +
        // `symGaussSeidel` entry runs upstream's `smoothSolver::solve` with all its controls (`smoothSolver.C:73-77`: `nSweeps` default 1;
        // `lduMatrixSolver.C`: `relTol` 0, `maxIter` 1000, `minIter` 0). `suffix` "Final": the entry `fvMatrix::solve` selects in the
        // outer iteration `customPimpleControl::loop` flags final (`mesh.data().setFinalIteration(true)`, `customPimpleControl.C:250-257`;
        // `psi.select(finalIteration)`), found in round 83 (README addendum A).
        type HSmooth = [Option<crate::genfoam::common::smooth_solver::SmoothSolverControls>; 2];
        let resolve_h = |suffix: &str| -> Result<(Option<HPbicgstab>, HSmooth), AppBuilderError> {
            let mut pb0: Option<HPbicgstab> = None;
            let mut smooth: HSmooth = [None, None];
            for (k, nm) in names.iter().enumerate() {
                let fname = format!("h.{nm}{suffix}");
                let entry = lookup_pattern_last(solvers, &fname);
                let pb_k = match entry {
                    Some(FoamEntry::SubDict(d))
                        if word(d, "solver").as_deref() == Some("PBiCGStab") && word(d, "preconditioner").as_deref() == Some("diagonal") =>
                    {
                        Some(HPbicgstab {
                            tolerance: ctx.f64(d, "tolerance")?,
                            rel_tol: scalar(d, "relTol").unwrap_or(0.0),
                            min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
                        })
                    }
                    _ => None,
                };
                if let Some(FoamEntry::SubDict(d)) = entry {
                    if word(d, "solver").as_deref() == Some("smoothSolver") && word(d, "smoother").as_deref() == Some("symGaussSeidel") {
                        smooth[k] = Some(crate::genfoam::common::smooth_solver::SmoothSolverControls {
                            tolerance: ctx.f64(d, "tolerance")?,
                            rel_tol: scalar(d, "relTol").unwrap_or(0.0),
                            min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
                            sweeps: crate::genfoam::common::smooth_solver::SmoothSolverSweeps {
                                n_sweeps: scalar(d, "nSweeps").unwrap_or(1.0) as i64,
                                max_iter: scalar(d, "maxIter").unwrap_or(1000.0) as usize,
                            },
                        });
                    }
                }
                if k == 0 {
                    pb0 = pb_k;
                } else if pb_k != pb0 {
                    // the enthalpy solve takes ONE `PBiCGStab` entry for both fluids: a per-fluid mix is refused by name
                    return Err(ctx.e(format!(
                        "solvers: {fname} resolves to a different linear solver than h.{}{suffix} (a per-fluid enthalpy solver is not ported unless both are smoothSolver)",
                        names[0]
                    )));
                }
            }
            Ok((pb0, smooth))
        };
        let (h_pb_check, h_smooth) = resolve_h("")?;
        if h_pb_check != h_pbicgstab {
            return Err(ctx.e("solvers: the h.<fluid> PBiCGStab entries disagree (not ported)"));
        }
        let (h_pbicgstab_final, h_smooth_final) = resolve_h("Final")?;
        // upstream resolves `p_rgh` to the exact keyword, else the LAST matching pattern; `PCG` + `DIC` is run as OpenFOAM does
        let p_rgh_pcg = match lookup_pattern_last(solvers, "p_rgh") {
            Some(FoamEntry::SubDict(d))
                if word(d, "solver").as_deref() == Some("PCG") && word(d, "preconditioner").as_deref() == Some("DIC") =>
            {
                Some(PRghPcg {
                    tolerance: ctx.f64(d, "tolerance")?,
                    rel_tol: scalar(d, "relTol").unwrap_or(0.0),
                    min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
                    max_iter: scalar(d, "maxIter").unwrap_or(1000.0) as usize,
                })
            }
            _ => None,
        };
        // gh:#1936 (README round 72): `GAMG` for `p_rgh` runs upstream's own solver; `p_rghFinal` is not selected separately here, so a
        // differing `GAMG` Final entry is refused by name (every shipped twoPhase case has the same options in both)
        let p_rgh_gamg = match lookup_pattern_last(solvers, "p_rgh") {
            Some(FoamEntry::SubDict(d)) if word(d, "solver").as_deref() == Some("GAMG") => {
                let c = super::gamg_solve::read_gamg_controls("p_rgh", d, None).map_err(|e| ctx.e(e))?;
                if let Err(e) = super::gamg_solve::refuse_update_interval_without_time_index("p_rgh", &c) {
                    return Err(ctx.e(e));
                }
                if let Some(FoamEntry::SubDict(f)) = lookup_pattern_last(solvers, "p_rghFinal") {
                    if word(f, "solver").as_deref() == Some("GAMG") {
                        let cf = super::gamg_solve::read_gamg_controls("p_rghFinal", f, None).map_err(|e| ctx.e(e))?;
                        if cf != c {
                            return Err(ctx.e("solvers.p_rghFinal differs from solvers.p_rgh: the twoPhase solver does not select the Final entry (not ported)"));
                        }
                    }
                }
                Some(c)
            }
            _ => None,
        };
        let alpha_d = match lookup_regex(solvers, "alpha") {
            Some(FoamEntry::SubDict(d)) => d.clone(),
            _ => return Err(ctx.e("no alpha solver dictionary")),
        };
        let alpha_solver = ctx.word(&alpha_d, "solver")?;
        let solve_phase_name = word(&alpha_d, "solvePhase");
        // MULES also accepts `solvePhase both` (the default of solveAlphaMULES.H:418)
        let mules_both = alpha_solver == "MULES" && matches!(solve_phase_name.as_deref(), None | Some("both"));
        let solve_phase = match solve_phase_name.clone() {
            Some(_) if mules_both => {
                if fluids[0].is_liquid { 1 } else { 0 }
            }
            Some(sp) => names.iter().position(|x| *x == sp).ok_or_else(|| ctx.e("unknown solvePhase"))?,
            None => {
                // MULES solves both; implicitUpwind needs one: the vapour.
                if fluids[0].is_liquid { 1 } else { 0 }
            }
        };
        let linear_solver = match lookup_regex(solvers, &format!("alpha.{}", names[solve_phase])) {
            Some(FoamEntry::SubDict(d)) => Some((ctx.f64(d, "tolerance")?, scalar(d, "relTol").unwrap_or(0.0))),
            _ => None,
        };
        let (alpha_solver_tolerance, alpha_solver_rel_tol) = match (alpha_solver.as_str(), linear_solver) {
            (_, Some(t)) => t,
            // MULES is explicit: no linear solver is read; NaN marks "none" (implicitUpwind refuses it).
            ("MULES", None) => (f64::NAN, 0.0),
            _ => return Err(ctx.e("no linear solver for alpha.<phase>")),
        };
        let relax_dict = fs.get_dict("relaxationFactors").and_then(|r| r.get_dict("fields"));
        let alpha = match alpha_solver.as_str() {
            "implicitUpwind" => AlphaSolver::ImplicitUpwind,
            "MULES" => {
                if g.mesh.patches.iter().any(|p| p.kind == PatchKind::Cyclic) {
                    return Err(ctx.e("MULES with a `cyclic` (coupled) patch is not ported (syncTools::syncFaceList and the coupled limiter branches of myMULES)"));
                }
                let schemes = ctx.read(&format!("system/{region}/fvSchemes"))?;
                let div = ctx.sub(&schemes, "divSchemes")?;
                let conv = |key: &str| -> Result<AlphaConvection, AppBuilderError> {
                    let e = lookup_regex(div, key).ok_or_else(|| ctx.e(format!("fvSchemes: no divSchemes entry `{key}`")))?;
                    let words: Vec<String> = match e {
                        FoamEntry::Word(w) => vec![w.clone()],
                        FoamEntry::Tokens(vs) => vs
                            .iter()
                            .filter_map(|v| match v {
                                FoamValue::Word(w) => Some(w.clone()),
                                _ => None,
                            })
                            .collect(),
                        _ => Vec::new(),
                    };
                    let w: Vec<&str> = words.iter().map(String::as_str).collect();
                    AlphaConvection::from_words(&w)
                        .ok_or_else(|| ctx.e(format!("fvSchemes: `{key}` = {words:?} is not ported (upwind, linear, vanLeer)")))
                };
                let adjust = bool_entry(&alpha_d, "adjustSubCycles").unwrap_or(false);
                let relax_of = |name: &str| -> Option<f64> {
                    relax_dict.and_then(|r| lookup_regex(r, name)).and_then(|e| match e {
                        FoamEntry::Scalar(x) => Some(*x),
                        _ => None,
                    })
                };
                AlphaSolver::Mules(MulesControls {
                    min_alpha_sp: scalar(&alpha_d, "minAlphaSp").unwrap_or(1e-6),
                    adjust_sub_cycles: adjust,
                    alpha_max_co: if adjust { ctx.f64(&alpha_d, "alphaMaxCo")? } else { 0.49 },
                    max_sub_cycles: scalar(&alpha_d, "maxSubCycles").unwrap_or(50.0) as usize,
                    n_sub_cycles: scalar(&alpha_d, "nSubCycles").unwrap_or(1.0) as usize,
                    limiter: LimiterControls {
                        n_limiter_iter: scalar(&alpha_d, "nLimiterIter").unwrap_or(3.0) as usize,
                        smooth_limiter: scalar(&alpha_d, "smoothLimiter").unwrap_or(0.0),
                        extrema_coeff: scalar(&alpha_d, "extremaCoeff").unwrap_or(0.0),
                    },
                    phi_scheme: conv("div(phi,alpha)")?,
                    phir_scheme: conv("div(phir,alpha)")?,
                    relax: [
                        relax_of(&format!("alpha.{}", names[0])),
                        relax_of(&format!("alpha.{}", names[1])),
                    ],
                    solve_phase: if mules_both { None } else { Some(solve_phase) },
                })
            }
            other => return Err(ctx.e(format!("alpha solver `{other}` is not an upstream selection (MULES, implicitUpwind)"))),
        };
        let rc = pimple.get_dict("residualControl");
        let rc_tol = |name: &str| rc.and_then(|r| r.get_dict(name)).and_then(|d| scalar(d, "tolerance"));
        let relax = fs.get_dict("relaxationFactors").and_then(|r| r.get_dict("fields"));
        let rf = |name: &str| -> f64 {
            relax
                .and_then(|r| lookup_regex(r, name))
                .and_then(|e| match e {
                    FoamEntry::Scalar(x) => Some(*x),
                    _ => None,
                })
                .unwrap_or(1.0)
        };
        let eq_relax = |name: &str| -> Option<f64> {
            fs.get_dict("relaxationFactors")
                .and_then(|r| r.get_dict("equations"))
                .and_then(|r| lookup_regex(r, name))
                .and_then(|e| match e {
                    FoamEntry::Scalar(x) => Some(*x),
                    _ => None,
                })
        };
        let controls = TwoPhaseControls {
            alpha,
            n_outer: scalar(pimple, "nOuterCorrectors").unwrap_or(1.0) as usize,
            n_corr: scalar(pimple, "nCorrectors").unwrap_or(1.0) as usize,
            correct_until_convergence: bool_entry(pimple, "correctUntilConvergence").unwrap_or(false),
            p_rgh_tolerance: p_rgh_tol,
            h_tolerance: h_tol,
            h_min_iter: h_min,
            h_pbicgstab,
            h_smooth,
            h_pbicgstab_final,
            h_smooth_final,
            p_rgh_pcg,
            p_rgh_gamg,
            h_symmetry_constraint: true,
            residual_control_p_rgh: rc_tol("p_rgh"),
            residual_control_h1: rc_tol(&format!("h.{}", names[0])),
            mass_transfer_safety_factor: scalar(pimple, "massTransferSafetyFactor"),
            max_t_interface_ddt: scalar(pimple, "maxTInterfaceDdt"),
            he_stabilization: match word(pimple, "enthalpyStabilizationMode").as_deref() {
                Some("source") => HeStabilization::Source,
                _ => HeStabilization::Cutoff,
            },
            relax_alpha: rf(&format!("alpha.{}", names[solve_phase])),
            relax_dmdt: rf(&format!("dmdt.{}.{}", names[0], names[1])),
            relax_t_interface: rf("T.interface"),
            relax_he: [rf(&format!("h.{}", names[0])), rf(&format!("h.{}", names[1]))],
            relax_phi: [rf(&format!("phi.{}", names[0])), rf(&format!("phi.{}", names[1]))],
            relax_p_rgh_equation: eq_relax("p_rgh"),
            relax_he_equation: [eq_relax(&format!("h.{}", names[0])), eq_relax(&format!("h.{}", names[1]))],
            min_kd_ff: scalar(pimple, "minKdFF").unwrap_or(1.0),
            min_alpha_sp: scalar(&alpha_d, "minAlphaSp").unwrap_or(1e-4),
            solve_phase,
            p_min: scalar(&pp, "pMin").unwrap_or(0.0),
            max_dpdt: scalar(pimple, "maxDpdt").unwrap_or(1e6),
            dpdt: true,
            earlier_nusselt_association: false,
            converged_temperature_newton: false,
            live_thermo_residual_markers: false,
            earlier_interfacial_flux_ratio: false,
            lazy_old_time_first_step: true,
            stale_pimple_converged_flag: false,
            p_patch_from_file: true,
            t_interface_limiter_sees_new_value: true,
            alpha_solver_tolerance,
            alpha_solver_rel_tol,
            alpha_solver_openfoam: true,
        };
        if scalar(pimple, "oscillationLimiterFraction").unwrap_or(0.0) != 0.0 {
            return Err(ctx.e("oscillationLimiterFraction != 0 is not ported"));
        }
        // gh:#1801: the through-flow patches exist only where the solver implements them.
        for (k, fl) in fluids.iter().enumerate() {
            let has_inlet_outlet = fl.alpha.bc.iter().any(|b| matches!(b, Bc::InletOutlet { .. }));
            if has_inlet_outlet && k != controls.solve_phase {
                return Err(ctx.e(format!(
                    "alpha.{}: inletOutlet on the phase fraction that is NOT solved (solvePhase {}) is not ported",
                    fl.name, names[controls.solve_phase]
                )));
            }
            if has_inlet_outlet && !matches!(controls.alpha, AlphaSolver::ImplicitUpwind) {
                return Err(ctx.e(format!(
                    "alpha.{}: inletOutlet is ported for the implicitUpwind alpha solver only (the case selects MULES)",
                    fl.name
                )));
            }
            if fl.u.has_non_zero_gradient_patch() && models.virtual_mass.is_some() {
                return Err(ctx.e(format!(
                    "U.{}: non-zeroGradient velocity patches with a virtualMassCoefficientModel are not ported",
                    fl.name
                )));
            }
        }
        for (k, fl) in fluids.iter_mut().enumerate() {
            if k != controls.solve_phase {
                // the other fraction is `1 - alphas - alphaSolved` on the patches too
                for b in fl.alpha.bc.iter_mut() {
                    if !matches!(b, Bc::Empty) {
                        *b = Bc::ZeroGradient;
                    }
                }
            }
        }

        // turbulence (gh:#1802): the structure regions and their volume fractions are needed
        let mut regions: Vec<(String, Vec<usize>, f64)> = Vec::new();
        for (key, e) in sp.iter() {
            if let FoamEntry::SubDict(zd) = e {
                if key != "powerOffCriterionModel" {
                    regions.push((key.to_string(), zone_cells(key)?, ctx.f64(zd, "volumeFraction")?));
                }
            }
        }
        for (k, fl) in fluids.iter_mut().enumerate() {
            fl.turbulence = super::turbulence::read_porous_k_epsilon_2phase(
                &ctx, &g, region, &names[k], &turb_dicts[k], &fs, &regions,
            )?;
        }

        Ok(Self {
            mesh,
            gravity,
            p,
            p_rgh,
            fluids,
            structure,
            models,
            controls,
            time_controls,
            start_time,
            end_time,
            delta_t,
            deviations,
        })
    }
}

#[cfg(feature = "std")]
/// A `timeProfile` dictionary of GeN-Foam (`timeProfile.C`): `type table;`, optional `startTime` (default 0), and
/// `table table ((t0 v0) (t1 v1) ...);` (ESI `Function1::Table`, linear, clamped outside the table). Every other
/// entry and every other `type` (`fmi`, `constant`, `sine`, ...) is refused by name.
pub(super) fn read_table_time_profile(
    ctx: &Ctx,
    d: &FoamDict,
    what: &str,
) -> Result<crate::genfoam::common::TimeProfile, AppBuilderError> {
    for (k, _) in d.iter() {
        if !matches!(k, "type" | "startTime" | "table") {
            return Err(ctx.e(format!("{what}.{k} is not ported (only `type table`, `startTime` and `table`)")));
        }
    }
    let ty = ctx.word(d, "type")?;
    if ty != "table" {
        return Err(ctx.e(format!("{what}: time profile type `{ty}` is not ported (only `table`)")));
    }
    let start = scalar(d, "startTime").unwrap_or(0.0);
    let rows: Vec<FoamValue> = match d.get("table") {
        Some(FoamEntry::List(l)) => l.clone(),
        Some(FoamEntry::Tokens(t)) => t
            .iter()
            .find_map(|v| if let FoamValue::List(l) = v { Some(l.clone()) } else { None })
            .ok_or_else(|| ctx.e(format!("{what}.table holds no list")))?,
        _ => return Err(ctx.e(format!("{what}.table is missing"))),
    };
    let mut times = Vec::new();
    let mut values = Vec::new();
    for r in &rows {
        let FoamValue::List(pair) = r else {
            return Err(ctx.e(format!("{what}.table rows must be (time value)")));
        };
        match (pair.first().and_then(FoamValue::as_scalar), pair.get(1).and_then(FoamValue::as_scalar), pair.len()) {
            (Some(t), Some(v), 2) => {
                times.push(Time::new::<second>(t));
                values.push(v);
            }
            _ => return Err(ctx.e(format!("{what}.table rows must be (time value)"))),
        }
    }
    let profile = crate::genfoam::common::TimeProfile::table(times, values)
        .map_err(|e| ctx.e(format!("{what}.table: {e:?}")))?;
    Ok(profile.with_start_time(Time::new::<second>(start)))
}

#[cfg(feature = "std")]
/// `constant`/`byRegime` scalar model reader.
fn read_by_regime(
    ctx: &Ctx,
    d: &FoamDict,
    leaf: impl Fn(&FoamDict) -> Result<f64, AppBuilderError>,
) -> Result<ByRegime, AppBuilderError> {
    match ctx.word(d, "type")?.as_str() {
        "constant" => Ok(ByRegime::Constant(leaf(d)?)),
        "byRegime" => {
            let mut table = Vec::new();
            for (rn, e) in d.iter() {
                if let FoamEntry::SubDict(sd) = e {
                    if ctx.word(sd, "type")? != "constant" {
                        return Err(ctx.e("byRegime sub-models must be constant"));
                    }
                    table.push((rn.to_string(), leaf(sd)?));
                }
            }
            Ok(ByRegime::PerRegime(table))
        }
        other => Err(ctx.e(format!("model type `{other}` not wired"))),
    }
}

#[cfg(feature = "std")]
pub(super) fn read_nusselt(ctx: &Ctx, md: &FoamDict) -> Result<FsForcedConvectionHtc, AppBuilderError> {
    match ctx.word(md, "type")?.as_str() {
        "NusseltReynoldsPrandtlPower" | "Nusselt" => Ok(FsForcedConvectionHtc::Nusselt {
            a: ctx.f64(md, "const")?,
            b: ctx.f64(md, "coeff")?,
            c: ctx.f64(md, "expRe")?,
            d: ctx.f64(md, "expPr")?,
            e: scalar(md, "expTc").unwrap_or(0.0),
        }),
        other => Err(ctx.e(format!("forced-convection htc `{other}` not wired"))),
    }
}

#[cfg(feature = "std")]
fn read_fs_htc(
    ctx: &Ctx,
    md: &FoamDict,
    chf_tables: &mut Vec<LookUpTableChf>,
) -> Result<FsHtcModel, AppBuilderError> {
    match ctx.word(md, "type")?.as_str() {
        "NusseltReynoldsPrandtlPower" | "Nusselt" => Ok(FsHtcModel::Nusselt(read_nusselt(ctx, md)?)),
        "superpositionNucleateBoiling" => {
            let fc = read_nusselt(ctx, ctx.sub(md, "forcedConvection")?)?;
            let pbd = ctx.sub(md, "poolBoiling")?;
            let pool_boiling = match ctx.word(pbd, "type")?.as_str() {
                "Shah" => {
                    if bool_entry(pbd, "useExplicitHeatFlux").unwrap_or(false) {
                        return Err(ctx.e("Shah useExplicitHeatFlux true is not wired"));
                    }
                    PoolBoilingHtc::Shah
                }
                other => return Err(ctx.e(format!("pool boiling `{other}` not wired"))),
            };
            let fd = ctx.sub(md, "flowEnhancementFactor")?;
            let max_value = Ratio::new::<ratio>(scalar(fd, "maxValue").unwrap_or(50.0));
            let flow_enhancement = match ctx.word(fd, "type")?.as_str() {
                "COBRA-TF" => FlowEnhancementFactor::CobraTf { max_value },
                "Chen" => FlowEnhancementFactor::Chen { max_value },
                other => return Err(ctx.e(format!("flow enhancement `{other}` not wired"))),
            };
            let sd = ctx.sub(md, "suppressionFactor")?;
            let suppression = match ctx.word(sd, "type")?.as_str() {
                "COBRA-TF" => SuppressionFactor::CobraTf,
                "Chen" => SuppressionFactor::Chen,
                other => return Err(ctx.e(format!("suppression `{other}` not wired"))),
            };
            Ok(FsHtcModel::SuperpositionNucleateBoiling {
                forced_convection: fc,
                pool_boiling,
                flow_enhancement,
                suppression,
            })
        }
        "multiRegimeBoiling" | "multiRegimeBoilingTRACE" | "multiRegimeBoilingTRACECHF" => {
            let with_chf = ctx.word(md, "type")? == "multiRegimeBoilingTRACECHF";
            let plain = ctx.word(md, "type")? == "multiRegimeBoiling";
            if md.get("filmCondensationModel").is_some() {
                return Err(ctx.e("multiRegimeBoiling(TRACE,CHF): filmCondensationModel is not ported"));
            }
            if plain && !bool_entry(md, "heatFluxSuperposition").unwrap_or(false) {
                return Err(ctx.e(
                    "multiRegimeBoiling: `heatFluxSuperposition false` (htc superposition with the cached htcNBi) is not ported",
                ));
            }
            let forced_convection = read_nusselt(ctx, ctx.sub(md, "forcedConvectionModel")?)?;
            let pbd = ctx.sub(md, "poolBoilingModel")?;
            let pool_boiling = match ctx.word(pbd, "type")?.as_str() {
                "Gorenflo" => {
                    if bool_entry(pbd, "useExplicitHeatFlux").unwrap_or(false) {
                        return Err(ctx.e("Gorenflo useExplicitHeatFlux true is not wired"));
                    }
                    PoolBoilingHtc::Gorenflo {
                        surface_roughness: uom::si::f64::Length::new::<uom::si::length::meter>(
                            scalar(pbd, "absoluteSurfaceRoughness").unwrap_or(4e-7),
                        ),
                    }
                }
                other => return Err(ctx.e(format!("multiRegimeBoilingTRACE pool boiling `{other}` not wired"))),
            };
            let fd = ctx.sub(md, "flowEnhancementFactorModel")?;
            let max_value = Ratio::new::<ratio>(scalar(fd, "maxValue").unwrap_or(50.0));
            let flow_enhancement = match ctx.word(fd, "type")?.as_str() {
                "RezkallahSims" => FlowEnhancementFactor::RezkallahSims { exponent: ctx.f64(fd, "exp")?, max_value },
                "Chen" => FlowEnhancementFactor::Chen { max_value },
                other => return Err(ctx.e(format!("multiRegimeBoilingTRACE flow enhancement `{other}` not wired"))),
            };
            // TRACE constructs the suppression model (and never evaluates it): it must exist; the plain
            // `multiRegimeBoiling` evaluates it when present (gh:#1802)
            let suppression = match md.get_dict("suppressionFactorModel") {
                None if plain => None,
                None => ctx.sub(md, "suppressionFactorModel").map(|_| None)?,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "Chen" => Some(SuppressionFactor::Chen),
                    "COBRA-TF" => Some(SuppressionFactor::CobraTf),
                    other => return Err(ctx.e(format!("multiRegimeBoiling suppression `{other}` not wired"))),
                },
            };
            let onset = match md.get_dict("nucleateBoilingOnsetModel") {
                None => None,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "Basu" => Some(OnsetOfNucleateBoilingTemperature::Basu {
                        surface_tension: uom::si::f64::SurfaceTension::new::<uom::si::surface_tension::newton_per_meter>(
                            ctx.f64(d, "surfaceTension")?,
                        ),
                        // BasuTONB.C: `contactAngle*2.0*pi/360.0`, evaluated in that order
                        contact_angle: uom::si::f64::Angle::new::<uom::si::angle::radian>(
                            ctx.f64(d, "contactAngle")? * 2.0 * core::f64::consts::PI / 360.0,
                        ),
                    }),
                    other => return Err(ctx.e(format!("nucleateBoilingOnsetModel `{other}` not wired"))),
                },
            };
            let sub_cooled_fraction = match md.get_dict("subCooledBoilingFractionModel") {
                None => None,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "SahaZuber" => Some(SubcooledBoilingFraction::SahaZuber),
                    "constant" => Some(SubcooledBoilingFraction::Constant { value: Ratio::new::<ratio>(ctx.f64(d, "value")?) }),
                    other => return Err(ctx.e(format!("subCooledBoilingFractionModel `{other}` not wired"))),
                },
            };
            let base = TraceBoilingModel {
                forced_convection,
                pool_boiling,
                flow_enhancement,
                onset,
                sub_cooled_fraction,
            };
            if plain {
                return Ok(FsHtcModel::MultiRegimeBoiling(MultiRegimeBoilingModel {
                    base,
                    suppression,
                    exponent: ctx.f64(md, "superpositionExponent")?,
                }));
            }
            if !with_chf {
                return Ok(FsHtcModel::MultiRegimeBoilingTrace(base));
            }
            // multiRegimeBoilingTRACECHF (gh:#1223, gh:#1224)
            let chf = match md.get_dict("criticalHeatFluxModel") {
                None => None,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "constantCHF" => Some(ChfSelection::Constant(ctx.f64(d, "value")?)),
                    "lookUpTableCHF" => {
                        let t = LookUpTableChf::from_dict(d).map_err(|e| ctx.e(e.to_string()))?;
                        chf_tables.push(t);
                        Some(ChfSelection::Table(chf_tables.len() - 1))
                    }
                    other => return Err(ctx.e(format!("criticalHeatFluxModel `{other}` not wired"))),
                },
            };
            let leidenfrost = match md.get_dict("leidenfrostModel") {
                None => None,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "GroeneveldStewart" => Some(LeidenfrostTemperature::GroeneveldStewart {
                        critical_pressure: uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(
                            ctx.f64(d, "criticalPressure")?,
                        ),
                    }),
                    other => return Err(ctx.e(format!("leidenfrostModel `{other}` not wired"))),
                },
            };
            let annular = match md.get_dict("annularFlowModel") {
                None => None,
                Some(d) => match ctx.word(d, "type")?.as_str() {
                    "CachardLiquid" => {
                        Some(CachardLiquid::new(ctx.f64(d, "wallEmissivity")?, ctx.f64(d, "liquidEmissivity")?))
                    }
                    other => {
                        return Err(ctx.e(format!("multiRegimeBoilingTRACECHF annularFlowModel `{other}` not wired")))
                    }
                },
            };
            Ok(FsHtcModel::MultiRegimeBoilingTraceChf(TraceChfModel {
                base,
                chf,
                leidenfrost,
                annular,
                roughness: scalar(md, "absoluteSurfaceRoughness").unwrap_or(4e-7),
            }))
        }
        "multiRegimeBoilingVapourTRACE" => {
            // upstream dereferences a null `htcAFPtr_` in the post-CHF branches when there is no
            // `annularFlowModel`: refuse rather than reproduce a crash
            let d = md.get_dict("annularFlowModel").ok_or_else(|| {
                ctx.e("multiRegimeBoilingVapourTRACE without annularFlowModel (upstream dereferences a null pointer post-CHF)")
            })?;
            match ctx.word(d, "type")?.as_str() {
                "CachardVapour" => Ok(FsHtcModel::MultiRegimeBoilingVapourTrace(CachardVapour::new())),
                other => Err(ctx.e(format!("multiRegimeBoilingVapourTRACE annularFlowModel `{other}` not wired"))),
            }
        }
        other => Err(ctx.e(format!("FS htc `{other}` not wired"))),
    }
}

#[cfg(feature = "std")]
pub(super) fn read_thermo(ctx: &Ctx, rel: &str) -> Result<PhaseThermo, AppBuilderError> {
    let d = ctx.read(rel)?;
    let tt = ctx.sub(&d, "thermoType")?;
    if ctx.word(tt, "type")? != "heRhoThermo" || ctx.word(tt, "energy")? != "sensibleEnthalpy" {
        return Err(ctx.e(format!("{rel}: only heRhoThermo with sensibleEnthalpy is wired")));
    }
    let mix = ctx.sub(&d, "mixture")?;
    let w = ctx.f64(ctx.sub(mix, "specie")?, "molWeight")?;
    let th = ctx.sub(mix, "thermodynamics")?;
    let tr = ctx.sub(mix, "transport")?;
    let eos = ctx.word(tt, "equationOfState")?;
    let thermo = ctx.word(tt, "thermo")?;
    let transport = ctx.word(tt, "transport")?;
    match (eos.as_str(), thermo.as_str(), transport.as_str()) {
        ("icoPolynomial", "hPolynomial", "polynomial") => {
            let e = ctx.sub(mix, "equationOfState")?;
            Ok(PhaseThermo::polynomial(
                w,
                ctx.list8(e, "rhoCoeffs<8>")?,
                ctx.list8(th, "CpCoeffs<8>")?,
                ctx.f64(th, "Hf")?,
                ctx.f64(th, "Sf")?,
                ctx.list8(tr, "muCoeffs<8>")?,
                ctx.list8(tr, "kappaCoeffs<8>")?,
            ))
        }
        ("perfectGas", "hConst", "const") => Ok(PhaseThermo::perfect_gas_const(
            w,
            ctx.f64(th, "Cp")?,
            ctx.f64(th, "Hf")?,
            ctx.f64(tr, "mu")?,
            ctx.f64(tr, "Pr")?,
        )),
        ("Boussinesq", "hConst", "const") => {
            let e = ctx.sub(mix, "equationOfState")?;
            Ok(PhaseThermo::BoussinesqHConst(super::one_phase_cc::BoussinesqHConst {
                cp: ctx.f64(th, "Cp")?,
                tref: scalar(th, "Tref").unwrap_or(298.15),
                hsref: scalar(th, "Href").unwrap_or(0.0),
                rho0: ctx.f64(e, "rho0")?,
                t0: ctx.f64(e, "T0")?,
                beta: ctx.f64(e, "beta")?,
                mu: ctx.f64(tr, "mu")?,
                r_pr: 1.0 / ctx.f64(tr, "Pr")?,
            }))
        }
        other => Err(ctx.e(format!("{rel}: thermo stack {other:?} not wired"))),
    }
}

/// `structure::constructHeatExchangers` for a region whose secondary zones are in the same mesh (README round 44, gh:#1847):
/// reads each `heatExchangers.<name>` (`primary`, `secondary`, `volumetricArea`, `wallConductance`) and builds its
/// [`HeatExchangerZone`](crate::genfoam::thermal_hydraulics::structure::heat_exchanger::HeatExchangerZone).
///
/// Refused by name: `secondaryRegion` other than the region itself (the mapping would need the other region's mesh and its
/// `htc`/`T` fields) and `interpolationMethod` other than the default `2` (`imCellVolumeWeight`); both fall under the
/// "not ported" message for an unknown key.
///
/// # Errors
/// [`AppBuilderError::Case`] for a missing entry, an unknown zone, a refused key or upstream's volume `FatalError`.
#[cfg(feature = "std")]
pub(super) fn read_heat_exchangers<F>(
    ctx: &Ctx,
    hxd: &FoamDict,
    poly_dir: &Path,
    g: &Geometry,
    zone_cells: &F,
) -> Result<crate::genfoam::thermal_hydraulics::structure::heat_exchanger::HeatExchangerSet, AppBuilderError>
where
    F: Fn(&str) -> Result<Vec<usize>, AppBuilderError>,
{
    use crate::genfoam::thermal_hydraulics::structure::heat_exchanger::{
        HeatExchangerSet, HeatExchangerZone, HxMeshInput,
    };
    use crate::io::poly_mesh::{parse_faces, parse_index_list, parse_points};
    let read_text = |name: &str| -> Result<String, AppBuilderError> {
        let p = poly_dir.join(name);
        std::fs::read_to_string(&p).map_err(|e| AppBuilderError::Io { path: p, source: e })
    };
    let points: Vec<Vector3> = parse_points(&read_text("points")?, "points")?
        .into_iter()
        .map(|p| Vector3::new(p[0], p[1], p[2]))
        .collect();
    let faces = parse_faces(&read_text("faces")?, "faces")?;
    let owner = parse_index_list(&read_text("owner")?, "owner")?;
    let neighbour = parse_index_list(&read_text("neighbour")?, "neighbour")?;
    let input = HxMeshInput {
        points: &points,
        faces: &faces,
        owner: &owner,
        neighbour: &neighbour,
        cell_volumes: &g.mesh.cell_volumes,
        cell_centres: &g.mesh.cell_centres,
    };
    let mut zones = Vec::new();
    for (name, e) in hxd.iter() {
        let FoamEntry::SubDict(d) = e else { continue };
        for (k, _) in d.iter() {
            if !matches!(k, "primary" | "secondary" | "volumetricArea" | "wallConductance") {
                return Err(ctx.e(format!(
                    "structureProperties.heatExchangers.{name}.{k} is not ported (`secondaryRegion` in another region and \
                     `interpolationMethod` other than 2 are refused; gh:#1847)"
                )));
            }
        }
        let primary = ctx.word(d, "primary")?;
        let secondary = ctx.word(d, "secondary")?;
        let i_a = ctx.f64(d, "volumetricArea")?;
        let hw = ctx.f64(d, "wallConductance")?;
        let primary_cells = zone_cells(&primary)?;
        let secondary_cells = zone_cells(&secondary)?;
        let z = HeatExchangerZone::new(&input, primary_cells, &secondary_cells, i_a, hw)
            .map_err(|e| ctx.e(format!("heatExchangers.{name}: {e}")))?;
        zones.push(z);
    }
    Ok(HeatExchangerSet::new(g.n_cells(), zones))
}
