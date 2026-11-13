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
//! error naming it**, except the alpha solver (`MULES` → `implicitUpwind`),
//! which is recorded in [`TwoPhaseCase::deviations`] because GeN-Foam ships
//! the alternative and the substitution is the whole point of this slice.

use std::path::{Path, PathBuf};
use std::sync::Arc;

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

use super::fv::{Bc, Geometry, Scalar, Vector};
use super::thermo::PhaseThermo;
use super::{
    ByRegime, FfHtcModel, FsHtcModel, HeStabilization, Phase, PhaseChangeConfig, Structure,
    TimeControls, TwoPhaseControls, TwoPhaseModels,
};
use crate::error::AppBuilderError;
use crate::genfoam::thermal_hydraulics::closures::ff_drag::{FfDragCoefficient, TwoPhaseDragMultiplier};
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::{
    FlowEnhancementFactor, SuppressionFactor,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::{
    FsForcedConvectionHtc, PoolBoilingHtc,
};
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map::{
    RegimeBound, RegimeInterpolationMode, RegimeMap1D,
};
use crate::genfoam::thermal_hydraulics::closures::phase_change::{
    LatentHeatModel, PhaseChangeRateModel, SaturationModel,
};
use crate::genfoam::thermal_hydraulics::structure::power_model::FixedPower;
use crate::genfoam::thermal_hydraulics::structure::power_off::PowerOffCriterion;
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

fn err(case: &Path, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Case {
        case: case.to_path_buf(),
        msg: msg.into(),
    }
}

/// Reading context: the case root, for error messages.
pub(super) struct Ctx {
    pub(super) root: PathBuf,
}

impl Ctx {
    pub(super) fn read(&self, rel: &str) -> Result<FoamDict, AppBuilderError> {
        let p = self.root.join(rel);
        FoamFile::read(&p)
            .map(|f| f.dict)
            .map_err(|e| err(&self.root, format!("{rel}: {e}")))
    }

    pub(super) fn e(&self, msg: impl Into<String>) -> AppBuilderError {
        err(&self.root, msg)
    }

    pub(super) fn sub<'a>(&self, d: &'a FoamDict, key: &str) -> Result<&'a FoamDict, AppBuilderError> {
        d.get_dict(key).ok_or_else(|| self.e(format!("missing sub-dictionary `{key}`")))
    }

    pub(super) fn f64(&self, d: &FoamDict, key: &str) -> Result<f64, AppBuilderError> {
        scalar(d, key).ok_or_else(|| self.e(format!("missing scalar `{key}`")))
    }

    pub(super) fn word(&self, d: &FoamDict, key: &str) -> Result<String, AppBuilderError> {
        word(d, key).ok_or_else(|| self.e(format!("missing word `{key}`")))
    }

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

/// The first entry of `d` whose (regex) key matches `name`, as OpenFOAM
/// looks up `relaxationFactors` and `solvers`.
pub(super) fn lookup_regex<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().find(|(k, _)| key_matches(k, name)).map(|(_, e)| e)
}

/// `internalField uniform v;` (scalar) or the first component of a vector.
pub(super) fn uniform_value(ctx: &Ctx, e: Option<&FoamEntry>) -> Result<FoamValue, AppBuilderError> {
    match e {
        Some(FoamEntry::Tokens(t)) if t.len() == 2 && t[0].as_word() == Some("uniform") => Ok(t[1].clone()),
        _ => Err(ctx.e("only `uniform` field values are supported")),
    }
}

/// A volume field file: internal value and per-patch BCs.
pub(super) struct FieldFile {
    pub(super) internal: FoamValue,
    pub(super) bcs: Vec<(String, String, Option<FoamValue>)>,
}

pub(super) fn read_field(ctx: &Ctx, rel: &str) -> Result<FieldFile, AppBuilderError> {
    let d = ctx.read(rel)?;
    let internal = uniform_value(ctx, d.get("internalField"))?;
    let bf = ctx.sub(&d, "boundaryField")?;
    let mut bcs = Vec::new();
    for (name, e) in bf.iter() {
        if let FoamEntry::SubDict(pd) = e {
            let ty = ctx.word(pd, "type")?;
            let value = match pd.get("value") {
                Some(FoamEntry::Word(w)) if w == "$internalField" => Some(internal.clone()),
                Some(v @ FoamEntry::Tokens(_)) => Some(uniform_value(ctx, Some(v))?),
                _ => None,
            };
            bcs.push((name.to_string(), ty, value));
        }
    }
    Ok(FieldFile { internal, bcs })
}

/// Map a scalar field file to [`Bc`]s on the mesh patches.
pub(super) fn scalar_bcs(ctx: &Ctx, g: &Geometry, ff: &FieldFile) -> Result<Vec<Bc>, AppBuilderError> {
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
            .ok_or_else(|| ctx.e(format!("no BC for patch `{}`", p.name)))?;
        out.push(match ty.as_str() {
            "fixedValue" => {
                let v = v
                    .as_ref()
                    .and_then(FoamValue::as_scalar)
                    .ok_or_else(|| ctx.e("fixedValue needs a uniform scalar value"))?;
                Bc::Fixed(vec![v; p.size])
            }
            "zeroGradient" => Bc::ZeroGradient,
            // `calculated` (and the `calulated` spelling 1D_boiling's
            // alpha.liquid carries) are set by field algebra.
            _ => Bc::Calculated,
        });
    }
    Ok(out)
}

impl TwoPhaseCase {
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

    /// Read the `fluidRegion` of a GeN-Foam case at `root` (the directory
    /// holding `0/`, `constant/`, `system/`).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] naming the first file, keyword or model
    /// selection the port cannot honour.
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path) -> Result<Self, AppBuilderError> {
        let ctx = Ctx {
            root: root.to_path_buf(),
        };
        let region = "fluidRegion";
        let poly = root.join(format!("constant/{region}/polyMesh"));
        let mesh = read_poly_mesh(&poly)?;
        let zones = read_cell_zones(&poly)?;
        let g = Geometry::new(mesh.clone());
        let n = g.n_cells();
        let mut deviations = Vec::new();
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
        let pp = ctx.read(&format!("constant/{region}/phaseProperties"))?;
        let names = [ctx.word(&pp, "fluid1")?, ctx.word(&pp, "fluid2")?];

        // 0/ fields
        let zero = format!("0/{region}");
        let p_rgh_f = read_field(&ctx, &format!("{zero}/p_rgh"))?;
        let p_rgh_bc = scalar_bcs(&ctx, &g, &p_rgh_f)?;
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
        for name in &names {
            let a_path = root.join(format!("{zero}/alpha.{name}"));
            alphas.push(if a_path.exists() {
                Some(read_field(&ctx, &format!("{zero}/alpha.{name}"))?.internal.as_scalar().unwrap_or(0.0))
            } else {
                None
            });
        }
        if alphas[0].is_none() && alphas[1].is_none() {
            return Err(ctx.e("neither alpha field is present"));
        }
        let a0 = alphas[0].unwrap_or(1.0 - alphas[1].unwrap_or(0.0));
        let a1 = alphas[1].unwrap_or(1.0 - a0);
        for (k, name) in names.iter().enumerate() {
            let props = ctx.sub(&pp, &format!("{name}Properties"))?;
            let state = ctx.word(props, "stateOfMatter")?;
            let is_liquid = state == "liquid";
            let residual_alpha = scalar(props, "residualAlpha").unwrap_or(1e-6);
            let tra = scalar(props, "thermoResidualAlpha").unwrap_or(0.0);
            let dd = match props.get_dict("dispersedDiameterModel") {
                Some(d) if word(d, "type").as_deref() == Some("constant") => ctx.f64(d, "value")?,
                Some(_) => return Err(ctx.e(format!("{name}: only a constant dispersedDiameterModel is ported"))),
                None => 0.0,
            };
            let thermo = read_thermo(&ctx, &format!("constant/{region}/thermophysicalProperties.{name}"))?;
            let tp = ctx.read(&format!("constant/{region}/turbulenceProperties.{name}"))?;
            if word(&tp, "simulationType").as_deref() != Some("laminar") {
                return Err(ctx.e(format!("{name}: only laminar turbulence is ported")));
            }
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
            for (pn, ty, _) in &u_f.bcs {
                if ty != "zeroGradient" && ty != "empty" {
                    return Err(ctx.e(format!("U.{name}.{pn}: only zeroGradient velocity patches are ported")));
                }
            }
            let alpha = Scalar::uniform(&g, if k == 0 { a0 } else { a1 }, g.default_bcs());
            fluids.push(Phase::new(
                &g,
                name,
                is_liquid,
                thermo,
                alpha,
                Vector::uniform(&g, u0),
                t,
                he_bc,
                residual_alpha,
                tra,
                dd,
            ));
        }
        let fluids: [Phase; 2] = [fluids[0].clone(), fluids[1].clone()];

        // structure
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_s = vec![0.0; n];
        let mut dh_s = vec![1.0; n];
        let mut power: Vec<Option<FixedPower>> = vec![None; n];
        let mut power_off = None;
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
            for (k2, _) in zd.iter() {
                if !matches!(k2, "volumeFraction" | "Dh" | "powerModel") {
                    return Err(ctx.e(format!("structureProperties.{key}.{k2} is not ported")));
                }
            }
            if let Some(pm) = zd.get_dict("powerModel") {
                if word(pm, "type").as_deref() != Some("fixedPower") {
                    return Err(ctx.e("only the fixedPower powerModel is ported"));
                }
                if pm.get("powerTimeProfile").is_some() {
                    return Err(ctx.e("fixedPower powerTimeProfile is not wired"));
                }
                let pa = scalar(pm, "volumeFraction").unwrap_or(vf);
                let rho_cp = match (scalar(pm, "rho"), scalar(pm, "Cp"), scalar(pm, "rhoCp")) {
                    (Some(r), Some(cp), _) => r * cp,
                    (_, _, Some(rc)) => rc,
                    _ => return Err(ctx.e("fixedPower: specify rho and Cp, or rhoCp")),
                };
                let ia = ctx.f64(pm, "volumetricArea")?;
                let t0 = ctx.f64(pm, "T")?;
                let q = ctx.f64(pm, "powerDensity")?;
                for &c in &cells {
                    // fixedPower.C: alphaRhoCp = max(alpha rhoCp, 1e-69)
                    power[c] = Some(FixedPower::new(
                        Ratio::new::<ratio>(pa),
                        uom::si::f64::ReciprocalLength::new::<reciprocal_meter>(ia),
                        uom::si::f64::VolumetricPowerDensity::new::<watt_per_cubic_meter>(q),
                        uom::si::f64::VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(
                            (pa * rho_cp).max(1e-69),
                        ),
                        ThermodynamicTemperature::new::<kelvin>(t0),
                    ));
                }
            }
        }
        let structure = Structure::new(&g, alpha_s, dh_s, power, power_off);

        // regime map
        let regime_map = match pp.get_dict("regimeMapModels") {
            None => None,
            Some(rm) => {
                let maps: Vec<(&str, &FoamEntry)> = rm.iter().collect();
                if maps.len() != 1 {
                    return Err(ctx.e("exactly one regime map is supported"));
                }
                let FoamEntry::SubDict(md) = maps[0].1 else {
                    return Err(ctx.e("regime map must be a dictionary"));
                };
                if word(md, "type").as_deref() != Some("oneParameter") {
                    return Err(ctx.e("only the oneParameter regime map is ported (twoParameters is a gap)"));
                }
                let param = ctx.word(md, "parameter")?;
                let k = names
                    .iter()
                    .position(|nm| param == format!("normalized.alpha.{nm}"))
                    .ok_or_else(|| ctx.e(format!("regime parameter `{param}` not supported")))?;
                let mut bounds = Vec::new();
                for (rn, e) in ctx.sub(md, "regimeBounds")?.iter() {
                    if let FoamEntry::List(v) = e {
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
                Some((RegimeMap1D::new(bounds, mode), k))
            }
        };

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
                    other => return Err(ctx.e(format!("multiplier `{other}` not wired"))),
                };
                Some((model, k, scalar(d, "maxPhi2").unwrap_or(1000.0)))
            }
        };
        let htm = ctx.sub(pm, "heatTransferModels")?;
        let mut fs_htc: [Vec<Option<FsHtcModel>>; 2] = [vec![None; n], vec![None; n]];
        for (k, name) in names.iter().enumerate() {
            if let Some(d) = pair(name, "structure", htm) {
                for (zk, e) in d.iter() {
                    let FoamEntry::SubDict(md) = e else { continue };
                    let model = read_fs_htc(&ctx, md)?;
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
        if word(ia, "type").as_deref() != Some("spherical") {
            return Err(ctx.e("only the spherical interfacial area model is wired"));
        }
        let spherical_cutoff_alpha = scalar(ia, "cutoffAlpha").unwrap_or(0.9);
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
            ff_htc,
            virtual_mass,
            regime_map,
            dispersion_fluid1,
            contact_partition,
            spherical_cutoff_alpha,
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
        let alpha_d = match lookup_regex(solvers, "alpha") {
            Some(FoamEntry::SubDict(d)) => d.clone(),
            _ => return Err(ctx.e("no alpha solver dictionary")),
        };
        let alpha_solver = ctx.word(&alpha_d, "solver")?;
        let solve_phase_name = word(&alpha_d, "solvePhase");
        let solve_phase = match solve_phase_name {
            Some(sp) => names.iter().position(|x| *x == sp).ok_or_else(|| ctx.e("unknown solvePhase"))?,
            None => {
                // MULES solves both; implicitUpwind needs one: the vapour.
                if fluids[0].is_liquid { 1 } else { 0 }
            }
        };
        let (alpha_solver_tolerance, alpha_solver_rel_tol) =
            match lookup_regex(solvers, &format!("alpha.{}", names[solve_phase])) {
                Some(FoamEntry::SubDict(d)) => (ctx.f64(d, "tolerance")?, scalar(d, "relTol").unwrap_or(0.0)),
                _ => return Err(ctx.e("no linear solver for alpha.<phase>")),
            };
        if alpha_solver != "implicitUpwind" {
            deviations.push(format!(
                "alpha solver `{alpha_solver}` is not ported; ran GeN-Foam's implicitUpwind \
                 (solveAlphaImplicitUpwind.H) with solvePhase {}",
                names[solve_phase]
            ));
        }
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
        let controls = TwoPhaseControls {
            n_outer: scalar(pimple, "nOuterCorrectors").unwrap_or(1.0) as usize,
            n_corr: scalar(pimple, "nCorrectors").unwrap_or(1.0) as usize,
            correct_until_convergence: bool_entry(pimple, "correctUntilConvergence").unwrap_or(false),
            p_rgh_tolerance: p_rgh_tol,
            h_tolerance: h_tol,
            h_min_iter: h_min,
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
            min_kd_ff: scalar(pimple, "minKdFF").unwrap_or(1.0),
            min_alpha_sp: scalar(&alpha_d, "minAlphaSp").unwrap_or(1e-4),
            solve_phase,
            p_min: scalar(&pp, "pMin").unwrap_or(0.0),
            max_dpdt: scalar(pimple, "maxDpdt").unwrap_or(1e6),
            dpdt: true,
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

fn read_fs_htc(ctx: &Ctx, md: &FoamDict) -> Result<FsHtcModel, AppBuilderError> {
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
        other => Err(ctx.e(format!("FS htc `{other}` not wired (multiRegimeBoilingTRACE etc. are gaps)"))),
    }
}

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
        other => Err(ctx.e(format!("{rel}: thermo stack {other:?} not wired"))),
    }
}
