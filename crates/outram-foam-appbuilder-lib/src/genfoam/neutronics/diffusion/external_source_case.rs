// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.C (externalSourceNeutronics_, externalSource_),
//                    src/classes/neutronics/diffusion/include/createNeutronicsFields.H
//                    (defaultFlux, defaultExternalSourceFlux, externalSourceFlux<i>),
//                    Tutorials/featureCases/2D_externalSourceDiffusion
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! Reader of a single-region stationary-fuel diffusion case with an external neutron source (gh:#1863).
//!
//! AI-written, not human-reviewed. Research, education and V&V only. `std` only (it reads files).
//!
//! Reads, from the case directory `<case>/` (`startFrom` the `0` directory; the polyMesh files must be
//! uncompressed): `constant/<region>/{nuclearData, neutronicsProperties, polyMesh/}`,
//! `0/<region>/{defaultFlux, flux<g>?, defaultExternalSourceFlux?, externalSourceFlux<g>?}`,
//! `0/uniform/reactorState`, `system/controlDict` and `system/<region>/{fvSchemes, fvSolution}`.
//!
//! What it reads of each, as upstream's `lookupOrDefault` would:
//!
//! * `neutronicsProperties`: `externalSourceNeutronics` (default `false`); `eigenvalueNeutronics true` is
//!   refused (the eigenvalue step is `pseudo_time.rs`);
//! * `constant/<region>/externalSource` (READ_IF_PRESENT): its `externalSourceModulationTimeProfile` is read by
//!   [`parse_external_source_time_profile`] (upstream's `timeProfile` of `type table`, gh:#1863 round 32b); no
//!   file, or no such key, is modulation 1. `type fmi` and every other `Function1` are refused by name;
//! * `fvSchemes`: `ddtSchemes default` (`steadyState` | `Euler`) and `laplacianSchemes default`
//!   (`Gauss linear uncorrected` | `Gauss linear corrected`), anything else refused;
//! * `fvSolution`: `neutronTransport` `neutronIterationResidual` (default `1e-5`), `maxNeutronIterations`
//!   (default 10); `integralPredictor`, `implicitPredictor`, `aitkenAcceleration` set `true` are refused;
//! * `controlDict`: `deltaT`, `endTime`, `writeControl`/`writeInterval`, `adjustTimeStep`, `maxDeltaT`,
//!   `maxPowerVariation` (default 0.025), `liquidFuel true` refused;
//! * flux boundary conditions: those of `defaultFlux` for every group (the `wedge` patches are zero gradient
//!   for a scalar, as OpenFOAM's `wedgeFvPatchField`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::{FoamEntry, FoamValue};
use outram_foam_basic_lib::mesh::FvMesh;
use uom::si::f64::Time;
use uom::si::time::second;
use outram_foam_basic_lib::prelude::BoundaryCondition;

use super::external_source::{
    refuse_external_source_for_transport, ExternalSource, NeutronicsDdtScheme, NeutronicsLaplacianScheme,
    StationaryStepControls,
};
use super::{DiffusionNeutronics, DiffusionSettings};
use crate::error::AppBuilderError;
use crate::genfoam::common::TimeProfile;
use crate::genfoam::neutronics::pseudo_time::{PseudoTimeControls, GREAT, VSMALL};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::workflow::reactor_state::ReactorState;
use crate::io::field_reader::{read_vol_scalar_field, read_vol_scalar_field_full};
use crate::io::nuclear_data::read_nuclear_data;
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh, strip_foam_comments, zone_of_cell};

fn parse_error(file: &Path, msg: String) -> AppBuilderError {
    AppBuilderError::Parse { file: file.display().to_string(), line: 0, msg }
}

fn read_text(path: &Path) -> Result<String, AppBuilderError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppBuilderError::Io { path: path.to_path_buf(), source: e })?;
    Ok(strip_foam_comments(&text))
}

/// The tokens after `key` up to the `;` that ends the entry, at the first whole-token match of `key`
/// (comments already stripped). `None` when the key is absent.
#[must_use]
pub fn dictionary_entry(text: &str, key: &str) -> Option<Vec<String>> {
    let spaced = text.replace(';', " ; ").replace('{', " { ").replace('}', " } ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    // the first occurrence that has a value (`stopAt endTime;` names the key without giving it one)
    for i in (0..tokens.len()).filter(|&i| tokens[i] == key) {
        let mut out = Vec::new();
        for t in &tokens[i + 1..] {
            if *t == ";" || *t == "{" || *t == "}" {
                break;
            }
            out.push((*t).to_string());
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// `dictionary_entry` restricted to the sub-dictionary `block` (`ddtSchemes { default ...; }`).
fn block_entry(text: &str, block: &str, key: &str) -> Option<Vec<String>> {
    let i = text.find(block)?;
    let rest = &text[i + block.len()..];
    let open = rest.find('{')?;
    let close = rest[open..].find('}')? + open;
    dictionary_entry(&rest[open + 1..close], key)
}

fn parse_bool(words: &[String], what: &str) -> Result<bool, String> {
    match words.first().map(String::as_str) {
        Some("true" | "on" | "yes" | "1") => Ok(true),
        Some("false" | "off" | "no" | "0") => Ok(false),
        other => Err(format!("{what}: not a switch: {other:?}")),
    }
}

fn parse_scalar(words: &[String], what: &str) -> Result<f64, String> {
    words
        .first()
        .and_then(|w| w.parse::<f64>().ok())
        .ok_or_else(|| format!("{what}: not a number: {words:?}"))
}

/// `externalSourceNeutronics` of the text of `neutronicsProperties` (`lookupOrDefault(..., false)`).
///
/// # Errors
///
/// A message if the entry is not a switch.
pub fn parse_external_source_neutronics(neutronics_properties: &str) -> Result<bool, String> {
    match dictionary_entry(neutronics_properties, "externalSourceNeutronics") {
        None => Ok(false),
        Some(w) => parse_bool(&w, "externalSourceNeutronics"),
    }
}

/// `ddtSchemes default` of the text of `fvSchemes`.
///
/// # Errors
///
/// A message naming the scheme if it is not `steadyState` or `Euler`.
pub fn parse_ddt_scheme(fv_schemes: &str) -> Result<NeutronicsDdtScheme, String> {
    match block_entry(fv_schemes, "ddtSchemes", "default").as_deref() {
        Some([w]) if w == "steadyState" => Ok(NeutronicsDdtScheme::SteadyState),
        Some([w]) if w == "Euler" => Ok(NeutronicsDdtScheme::Euler),
        other => Err(format!("ddtSchemes default {other:?} is not ported (steadyState and Euler are)")),
    }
}

/// `laplacianSchemes default` of the text of `fvSchemes`.
///
/// # Errors
///
/// A message naming the scheme if it is not `Gauss linear uncorrected` or `Gauss linear corrected`.
pub fn parse_laplacian_scheme(fv_schemes: &str) -> Result<NeutronicsLaplacianScheme, String> {
    let e = block_entry(fv_schemes, "laplacianSchemes", "default");
    match e.as_deref() {
        Some([a, b, c]) if a == "Gauss" && b == "linear" && c == "uncorrected" => {
            Ok(NeutronicsLaplacianScheme::Uncorrected)
        }
        Some([a, b, c]) if a == "Gauss" && b == "linear" && c == "corrected" => {
            Ok(NeutronicsLaplacianScheme::Corrected)
        }
        other => Err(format!(
            "laplacianSchemes default {other:?} is not ported (Gauss linear uncorrected and corrected are)"
        )),
    }
}

/// `externalSourceModulationTimeProfile` of the text of `constant/<region>/externalSource`: GeN-Foam's
/// `timeProfile(IOdictionary, name, Time)` constructor (`common/timeProfile/timeProfile.C:100-137`, 652b3da) of
/// `type table`, as OpenFOAM-ESI v2506's `Function1Types::Table` reads it (`Table.C:34-76`):
///
/// * no such key (the file has none): `Ok(None)`, an invalid profile, modulation 1 (`valid()` false);
/// * `type` is mandatory (`get<word>("type")`), `startTime` defaults to 0 and the profile is evaluated at
///   `t - startTime`;
/// * the rows come from the entry `table` read as a stream, `table table ((t v) ...)` (the leading word is
///   discarded; the form of GeN-Foam's shipped point-kinetics tutorials) or `table ((t v) ...)`; otherwise
///   from `values ((t v) ...)` (a `file` is refused);
/// * `outOfBounds clamp` and `interpolationScheme linear` are the defaults and are accepted when written.
///
/// Refused by name (not ported): `type fmi` (a live co-simulation coupling, out of scope by
/// `RESPONSIBLE_USE.md`), every other `type`, `file`, any other `outOfBounds` or `interpolationScheme`, a
/// table with fewer than two rows (the port's [`TimeProfile::table`]) and non-ascending times.
///
/// # Errors
///
/// A message naming the offending entry.
pub fn parse_external_source_time_profile(text: &str) -> Result<Option<TimeProfile>, String> {
    let file = FoamFile::parse_named(text, "externalSource").map_err(|e| format!("externalSource: {e}"))?;
    let key = "externalSourceModulationTimeProfile";
    let Some(entry) = file.dict.get(key) else {
        return Ok(None);
    };
    let FoamEntry::SubDict(d) = entry else {
        return Err(format!("{key} is not a sub-dictionary (upstream: subDict, fatal)"));
    };
    let ty = d.get_word("type").ok_or_else(|| format!("{key}.type is missing (upstream: get<word>(\"type\"), fatal)"))?;
    if ty == "fmi" {
        return Err(format!("{key}.type fmi: the FMI co-simulation profile is not ported (live coupling is out of scope)"));
    }
    if ty != "table" {
        return Err(format!("{key}.type {ty}: only `table` is ported"));
    }
    let start = match d.get("startTime") {
        None => 0.0,
        Some(FoamEntry::Scalar(x)) => *x,
        Some(_) => return Err(format!("{key}.startTime is not a number")),
    };
    if let Some(w) = d.get_word("outOfBounds") {
        if w != "clamp" {
            return Err(format!("{key}.outOfBounds {w}: only the default `clamp` is ported"));
        }
    }
    if let Some(w) = d.get_word("interpolationScheme") {
        if w != "linear" {
            return Err(format!("{key}.interpolationScheme {w}: only the default `linear` is ported"));
        }
    }
    let rows_of = |e: Option<&FoamEntry>| -> Option<Vec<FoamValue>> {
        match e {
            Some(FoamEntry::List(l)) => Some(l.clone()),
            Some(FoamEntry::Tokens(t)) => {
                // `table table ((t v) ...)`: the leading word `table` is discarded
                let rest: &[FoamValue] = match t.first() {
                    Some(FoamValue::Word(w)) if w == "table" => &t[1..],
                    _ => t.as_slice(),
                };
                match rest {
                    [FoamValue::List(l)] => Some(l.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    };
    let rows = if matches!(d.get("table"), Some(e) if !matches!(e, FoamEntry::SubDict(_))) {
        rows_of(d.get("table")).ok_or_else(|| format!("{key}.table holds no list of (time value) rows"))?
    } else if d.get("file").is_some() {
        return Err(format!("{key}.file: reading the table from a file is not ported"));
    } else {
        rows_of(d.get("values")).ok_or_else(|| format!("{key}: neither `table` nor `values` holds a list of rows"))?
    };
    let mut times = Vec::with_capacity(rows.len());
    let mut values = Vec::with_capacity(rows.len());
    for r in &rows {
        match r {
            FoamValue::List(p) if p.len() == 2 => match (p[0].as_scalar(), p[1].as_scalar()) {
                (Some(t), Some(v)) => {
                    times.push(Time::new::<second>(t));
                    values.push(v);
                }
                _ => return Err(format!("{key}: a table row is not (time value)")),
            },
            _ => return Err(format!("{key}: a table row is not (time value)")),
        }
    }
    let profile = TimeProfile::table(times, values).map_err(|e| format!("{key}: {e}"))?;
    Ok(Some(profile.with_start_time(Time::new::<second>(start))))
}

/// A single-region stationary-fuel diffusion case with its external source, ready to run.
#[derive(Debug)]
pub struct ExternalSourceCase {
    /// The diffusion model, flux initialised from the `0` directory, source set.
    pub model: DiffusionNeutronics,
    /// The clock controls (`controlDict`, `pTarget` of `reactorState`).
    pub clock: PseudoTimeControls,
    /// The step controls (`fvSchemes`, `fvSolution`).
    pub step: StationaryStepControls,
    /// The mesh.
    pub mesh: Arc<FvMesh>,
    /// `externalSourceNeutronics`.
    pub external_source_neutronics: bool,
}

impl ExternalSourceCase {
    /// [`Self::read`] with the linear solvers taken from the case's own `system/neutroRegion/fvSolution`: every key
    /// OpenFOAM's `lduMatrix::solver` reads (`tolerance`, `relTol`, `maxIter`, `minIter`, solver and preconditioner),
    /// upstream's defaults where an entry omits one (gh:#1887 round 42). `GAMG` runs `PCG`/`DIC` (named deviation,
    /// reported in the returned flag).
    ///
    /// # Errors
    ///
    /// As [`Self::read`], and an `fvSolution` selection the port does not run (named).
    #[cfg(feature = "std")]
    pub fn read_with_fv_solution(case_dir: &Path) -> Result<(Self, bool), AppBuilderError> {
        let linear = outram_foam_basic_lib::prelude::SolverSettings { tolerance: 1e-6, max_iter: 1000 };
        let mut case = Self::read(case_dir, linear)?;
        let path = case_dir.join("system").join("neutroRegion").join("fvSolution");
        let sol = crate::io::nuclear_data::read_neutron_fv_solution(
            &path,
            case.model.energy_groups(),
            case.model.prec_groups(),
            0,
        )?;
        case.model.set_linear_form(crate::genfoam::common::linear_solvers::LinearSolverForm::UpstreamFvSolution(sol.entries));
        Ok((case, sol.gamg_substituted))
    }

    /// Read the case under `case_dir` (region `neutroRegion`); `solver` is the `regionSolvers` entry of the
    /// `controlDict` (`diffusionNeutronics`).
    ///
    /// # Errors
    ///
    /// [`AppBuilderError`] for an unreadable file or a selection the port does not implement (named).
    pub fn read(case_dir: &Path, linear: outram_foam_basic_lib::prelude::SolverSettings) -> Result<Self, AppBuilderError> {
        let region = "neutroRegion";
        let constant: PathBuf = case_dir.join("constant").join(region);
        let zero: PathBuf = case_dir.join("0").join(region);
        let system: PathBuf = case_dir.join("system");

        let props_path = constant.join("neutronicsProperties");
        let props = read_text(&props_path)?;
        let eigen = match dictionary_entry(&props, "eigenvalueNeutronics") {
            None => false,
            Some(w) => parse_bool(&w, "eigenvalueNeutronics").map_err(|m| parse_error(&props_path, m))?,
        };
        if eigen {
            return Err(parse_error(&props_path, "eigenvalueNeutronics true: use the pseudo-time eigenvalue driver".into()));
        }
        let enabled = parse_external_source_neutronics(&props).map_err(|m| parse_error(&props_path, m))?;

        let control_path = system.join("controlDict");
        let control = read_text(&control_path)?;
        let solver_word = block_entry(&control, "regionSolvers", region)
            .and_then(|w| w.first().cloned())
            .unwrap_or_default();
        refuse_external_source_for_transport(&solver_word, enabled)
            .map_err(|e| parse_error(&control_path, e.to_string()))?;
        if solver_word != "diffusionNeutronics" {
            return Err(parse_error(&control_path, format!("regionSolvers {region} {solver_word}: not a diffusion region")));
        }
        if let Some(w) = dictionary_entry(&control, "liquidFuel") {
            if parse_bool(&w, "liquidFuel").map_err(|m| parse_error(&control_path, m))? {
                return Err(parse_error(&control_path, "liquidFuel true: circulating fuel is the upstream_transient_step form".into()));
            }
        }
        let scalar = |key: &str, default: Option<f64>| -> Result<f64, AppBuilderError> {
            match dictionary_entry(&control, key) {
                Some(w) => parse_scalar(&w, key).map_err(|m| parse_error(&control_path, m)),
                None => default.ok_or_else(|| parse_error(&control_path, format!("missing {key}"))),
            }
        };
        let delta_t = scalar("deltaT", None)?;
        let end_time = scalar("endTime", None)?;
        let max_delta_t = scalar("maxDeltaT", Some(GREAT))?;
        let min_delta_t = scalar("minDeltaT", Some(VSMALL))?;
        let max_power_variation = scalar("maxPowerVariation", Some(0.025))?;
        let adjust_time_step = match dictionary_entry(&control, "adjustTimeStep") {
            Some(w) => parse_bool(&w, "adjustTimeStep").map_err(|m| parse_error(&control_path, m))?,
            None => false,
        };
        let write_adjustable = dictionary_entry(&control, "writeControl")
            .is_some_and(|w| w.first().is_some_and(|x| x == "adjustableRunTime"));
        let write_interval = if write_adjustable { Some(scalar("writeInterval", None)?) } else { None };

        // neutronics.C:265-281: `externalSource` is READ_IF_PRESENT; only its time profile is read by the
        // diffusion solver (createFluxMatrices.H:3-10)
        let ext_dict = constant.join("externalSource");
        let modulation = if ext_dict.exists() {
            parse_external_source_time_profile(&read_text(&ext_dict)?).map_err(|m| parse_error(&ext_dict, m))?
        } else {
            None
        };

        let schemes_path = system.join(region).join("fvSchemes");
        let schemes = read_text(&schemes_path)?;
        let ddt = parse_ddt_scheme(&schemes).map_err(|m| parse_error(&schemes_path, m))?;
        let laplacian = parse_laplacian_scheme(&schemes).map_err(|m| parse_error(&schemes_path, m))?;

        let solution_path = system.join(region).join("fvSolution");
        let solution = read_text(&solution_path)?;
        let transport = |key: &str| dictionary_entry(&solution, key);
        for key in ["integralPredictor", "implicitPredictor", "aitkenAcceleration"] {
            if let Some(w) = transport(key) {
                if parse_bool(&w, key).map_err(|m| parse_error(&solution_path, m))? {
                    return Err(parse_error(&solution_path, format!("{key} true: not ported")));
                }
            }
        }
        let neutron_iteration_residual = match transport("neutronIterationResidual") {
            Some(w) => parse_scalar(&w, "neutronIterationResidual").map_err(|m| parse_error(&solution_path, m))?,
            None => 1.0e-5,
        };
        let max_neutron_iterations = match transport("maxNeutronIterations") {
            Some(w) => parse_scalar(&w, "maxNeutronIterations").map_err(|m| parse_error(&solution_path, m))? as usize,
            None => 10,
        };

        let reactor = ReactorState::read(&case_dir.join("0"))
            .map_err(|e| parse_error(&case_dir.join("0/uniform/reactorState"), e.to_string()))?;

        let mesh = read_poly_mesh(&constant.join("polyMesh"))?;
        let nuclear = read_nuclear_data(&constant.join("nuclearData"))?;
        let xs = CrossSectionData::from_input(&nuclear)
            .map_err(|e| parse_error(&constant.join("nuclearData"), e.to_string()))?;
        let zones = read_cell_zones(&constant.join("polyMesh"))?;
        let order: Vec<String> = (0..xs.zone_count())
            .map(|i| xs.zone(i).map(|z| z.name().to_string()))
            .collect::<Option<_>>()
            .ok_or_else(|| parse_error(&constant.join("nuclearData"), "zone index out of range".into()))?;
        let order_refs: Vec<&str> = order.iter().map(String::as_str).collect();
        let zone_map = zone_of_cell(&zones, &order_refs, mesh.n_cells)?;

        // defaultFlux: boundary conditions for every group and the start flux; flux<g> overrides the values
        let default_flux = read_vol_scalar_field_full(&zero.join("defaultFlux"), &mesh)?;
        let boundary: Vec<BoundaryCondition<f64>> = default_flux.boundary.iter().map(|p| p.bc.clone()).collect();
        let groups = xs.energy_groups();
        let n = mesh.n_cells;
        let mut start_flux = Vec::with_capacity(groups);
        for g in 0..groups {
            let p = zero.join(format!("flux{g}"));
            start_flux.push(if p.exists() { read_vol_scalar_field(&p, n)? } else { default_flux.internal.as_slice().to_vec() });
        }
        let settings = DiffusionSettings { linear, ..DiffusionSettings::default() };
        let mut model = DiffusionNeutronics::new(mesh.clone(), &xs, &zone_map, &[], &boundary, settings)
            .map_err(|e| parse_error(&constant.join("nuclearData"), e.to_string()))?;
        model.set_initial_guess(&start_flux, reactor.k_eff);

        // createNeutronicsFields.H: externalSourceFlux<g> else defaultExternalSourceFlux else 0
        let default_ext = zero.join("defaultExternalSourceFlux");
        let default_ext_values = if default_ext.exists() { read_vol_scalar_field(&default_ext, n)? } else { vec![0.0; n] };
        let mut ext_flux = Vec::with_capacity(groups);
        for g in 0..groups {
            let p = zero.join(format!("externalSourceFlux{g}"));
            ext_flux.push(if p.exists() { read_vol_scalar_field(&p, n)? } else { default_ext_values.clone() });
        }
        let mut source = if enabled { ExternalSource::new(ext_flux) } else { ExternalSource::switched_off(ext_flux) };
        if let Some(profile) = modulation {
            source = source.with_modulation(profile);
        }
        model.set_external_source(source).map_err(|e| parse_error(&zero, e.to_string()))?;

        let clock = PseudoTimeControls {
            delta_t,
            end_time,
            write_interval,
            adjust_time_step,
            max_delta_t,
            min_delta_t,
            max_power_variation,
            p_target: reactor.p_target,
        };
        let step = StationaryStepControls { ddt, laplacian, neutron_iteration_residual, max_neutron_iterations };
        Ok(Self { model, clock, step, mesh, external_source_neutronics: enabled })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_defaults_to_false_and_reads_a_switch() {
        assert_eq!(parse_external_source_neutronics("eigenvalueNeutronics false;"), Ok(false));
        assert_eq!(parse_external_source_neutronics("externalSourceNeutronics    true;   "), Ok(true));
        assert!(parse_external_source_neutronics("externalSourceNeutronics maybe;").is_err());
    }

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    fn profile(body: &str) -> Result<Option<TimeProfile>, String> {
        parse_external_source_time_profile(&format!(
            "FoamFile {{ version 2.0; format ascii; class dictionary; object externalSource; }}\n\
             externalSourceMode transient; // read by point kinetics only\n{body}"
        ))
    }

    #[test]
    fn the_time_profile_is_read_in_the_inline_and_the_values_forms_and_shifted_by_start_time() {
        // the form of GeN-Foam's shipped point-kinetics tutorial (`table table (...)`), with comments
        let inline = profile(
            "externalSourceModulationTimeProfile // c\n{ type table; startTime 5; /* c */ table table ((0 1) (15 1) (35 3)); }",
        )
        .expect("parses")
        .expect("present");
        assert_eq!(inline.value(s(0.0)), 1.0); // before start: clamp
        assert_eq!(inline.value(s(20.0)), 1.0); // shifted 15
        assert_eq!(inline.value(s(30.0)), 2.0); // shifted 25, half way up
        assert_eq!(inline.value(s(99.0)), 3.0); // after the table: clamp
        // `table ((t v) ...)` without the leading word, and the `values` form with no startTime
        let bare = profile("externalSourceModulationTimeProfile { type table; table ((0 1) (10 3)); }").unwrap().unwrap();
        let values = profile("externalSourceModulationTimeProfile { type table; values ((0 1) (10 3)); }").unwrap().unwrap();
        assert_eq!(bare, values);
        assert_eq!(values.value(s(5.0)), 2.0);
        assert_eq!(values.start_time(), s(0.0));
        // the written defaults are accepted
        assert!(profile(
            "externalSourceModulationTimeProfile { type table; outOfBounds clamp; interpolationScheme linear; values ((0 1) (1 2)); }"
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn no_profile_key_is_modulation_one_and_the_unported_forms_are_refused_by_name() {
        assert_eq!(profile("somethingElse 1;"), Ok(None));
        let refused = |body: &str, word: &str| {
            let e = profile(body).expect_err(body);
            assert!(e.contains(word), "{e}");
        };
        refused("externalSourceModulationTimeProfile { type fmi; nameFromFMU gf; initialValue 1; }", "fmi");
        refused("externalSourceModulationTimeProfile { type sine; }", "sine");
        refused("externalSourceModulationTimeProfile { startTime 1; values ((0 1) (1 2)); }", "type");
        refused("externalSourceModulationTimeProfile { type table; outOfBounds repeat; values ((0 1) (1 2)); }", "repeat");
        refused("externalSourceModulationTimeProfile { type table; interpolationScheme splineLinear; values ((0 1) (1 2)); }", "splineLinear");
        refused("externalSourceModulationTimeProfile { type table; file \"tbl\"; }", "file");
        refused("externalSourceModulationTimeProfile { type table; }", "values");
        refused("externalSourceModulationTimeProfile { type table; startTime later; values ((0 1) (1 2)); }", "startTime");
        refused("externalSourceModulationTimeProfile { type table; values ((0 1 2) (1 2)); }", "row");
        refused("externalSourceModulationTimeProfile { type table; values ((0 1) (0 2)); }", "ascending");
        refused("externalSourceModulationTimeProfile { type table; values ((0 1)); }", "2 points");
        refused("externalSourceModulationTimeProfile 3;", "sub-dictionary");
    }

    #[test]
    fn the_schemes_are_read_and_the_rest_refused_by_name() {
        let s = "ddtSchemes { default steadyState; } laplacianSchemes { default Gauss linear corrected; }";
        assert_eq!(parse_ddt_scheme(s), Ok(NeutronicsDdtScheme::SteadyState));
        assert_eq!(parse_laplacian_scheme(s), Ok(NeutronicsLaplacianScheme::Corrected));
        let u = "ddtSchemes { default Euler; } laplacianSchemes { default Gauss linear uncorrected; }";
        assert_eq!(parse_ddt_scheme(u), Ok(NeutronicsDdtScheme::Euler));
        assert_eq!(parse_laplacian_scheme(u), Ok(NeutronicsLaplacianScheme::Uncorrected));
        assert!(parse_ddt_scheme("ddtSchemes { default backward; }").is_err());
        assert!(parse_laplacian_scheme("laplacianSchemes { default Gauss linear limited 0.5; }").is_err());
    }
}
