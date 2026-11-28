// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/structure.C (constructor: the
//             `powerModel` table), structure.H (`setRegionField`), powerModels/{powerModel,heatedPin,nuclearFuelPin,
//             lumpedNuclearStructure,interpolatedNuclearFuelPin}/*.C (the dictionary reads),
//             lumpedNuclearStructure.C (`fvc::laplacian(Tmatrix_)`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `one_phase_cc::radial_power` — the four radial power models read from `structureProperties` (gh:#1879)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! `structure::structure` collects the `type` of every zone's `powerModel`, builds ONE dictionary per type (key = zone
//! name, the `a:b:c` keys of `structureProperties` split into one region per zone, value = the zone's `powerModel`
//! dictionary) and constructs one model per type through `powerModel::New`. [`read_radial_power_models`] does the same
//! for the four radial-conduction types and hands back the models in first-appearance order.
//!
//! ## Dictionary keys, as upstream reads them (`get` = required, `lookupOrDefault` = optional)
//!
//! | model | keys |
//! |---|---|
//! | all | `volumeFraction` (optional override of the model's alpha, `powerModel.C`) |
//! | `heatedPin` | `innerRadius`, `outerRadius`, `meshSize`, `rho`+`Cp` or `rhoCp`, `k`, `T`, `powerDensity` |
//! | `nuclearFuelPin` | `fractionOfPowerFromNeutronics` (1), `fuel/cladInner/OuterRadius`, `fuel/cladMeshSize`, `fuelRho`+`fuelCp` or `fuelRhoCp`, same for clad, `fuelK`, `cladK`, `fuelT`, `cladT`, `gapH` XOR `gapHPowerDensity table (...)`, `powerDensity` |
//! | `lumpedNuclearStructure` | `fractionOfPowerFromNeutronics` (1), `nodesNumber`, `nodeFuel`, `nodeClad`, `nodeMatrix` (0), `kappaMatrix` (0), `heatConductances`, `rhoCp`, `volumeFractions`, `powerFractions`, `T0`, `volumetricArea`, `powerDensity` |
//! | `interpolatedNuclearFuelPin` | `fuelOuterRadius`, `T`, `powerDensity` |
//!
//! `powerDensity` is `structure::setRegionField`'s key: for `nuclearFuelPin` and `lumpedNuclearStructure` it fills the
//! structure's `powerDensityNeutronics` of the region's cells when that field is NOT in the start directory (when it is,
//! the field is read and the dictionary value is ignored: upstream's "DO NOTHING" branch); `heatedPin` keeps its own
//! `powerDensity.heatedPin`. A `hollow` test (`innerRadius >= 1e-5`) and the interfacial areas are the models' (round 35).
//!
//! ## Refused by name
//! ~~`nuclearFuelPinTest` (gh:#1787)~~ **CORRECTED 2026-11-27 (gh:#1911, README round 71): ported as `RadialPowerModel::NuclearFuelPinTest`**, `nuclearFuelFMU`, `fixedPower` here (the two-phase solver owns it) and any other
//! type; a start directory that already holds a model's own written fields (`Trad.<model>`, `Ti/To.heatedPin`,
//! `Tfi/Tfo/Tci/Tco.nuclearFuelPin`, `T.`/`Tmatrix.`/`Tsurface.lumpedNuclearStructure`, `powerDensity.heatedPin`,
//! `T.interpolatedNuclearFuelPin`): upstream restarts from them, the port does not read them.
//! `laplacianSchemes` other than `Gauss linear uncorrected` for a `lumpedNuclearStructure` with `kappaMatrix != 0`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::fv::Geometry;

/// `fvc::laplacian(vf)` with `laplacianSchemes Gauss linear uncorrected` (ESI `gaussLaplacianScheme::fvcLaplacian`):
/// `fvc::div(snGrad(vf)*mesh.magSf())` = `fvc::surfaceIntegrate`, i.e. the internal faces `deltaCoeffs (vf_N - vf_P) |Sf|`
/// added to the owner and subtracted from the neighbour, divided by the cell volume.
///
/// Boundary faces contribute exactly 0: `uncorrectedSnGrad` evaluates a boundary face with the PATCH field's own `snGrad()`,
/// which is 0 for `zeroGradient` and, for a scalar, for `wedge` (`(transform(T, pif) - pif) * 0.5 deltaCoeffs` with the
/// identity transform) and has no faces on `empty` patches. `Tmatrix` carries `zeroGradient` everywhere.
#[must_use]
pub fn fvc_laplacian_uncorrected(g: &Geometry, vf: &[f64]) -> Vec<f64> {
    let m = &g.mesh;
    let mut out = vec![0.0; g.n_cells()];
    for f in 0..g.n_internal() {
        let flux = (vf[m.neighbour[f]] - vf[m.owner[f]]) * g.delta[f] * m.face_areas[f];
        out[m.owner[f]] += flux;
        out[m.neighbour[f]] -= flux;
    }
    for (c, v) in out.iter_mut().enumerate() {
        *v /= m.cell_volumes[c];
    }
    out
}

#[cfg(feature = "std")]
mod reader {
    use std::path::Path;

    use outram_foam_basic_lib::io::{FoamDict, FoamEntry, FoamValue};
    use uom::si::f64::Time;
    use uom::si::time::second;

    use super::super::case::{scalar, Ctx};
    use crate::error::AppBuilderError;
    use crate::genfoam::common::TimeProfile;
    use crate::genfoam::thermal_hydraulics::structure::heated_pin::{HeatedPin, HeatedPinRegion};
    use crate::genfoam::thermal_hydraulics::structure::interpolated_nuclear_fuel_pin::{
        InterpolatedNuclearFuelPin, InterpolatedRegion,
    };
    use crate::genfoam::thermal_hydraulics::structure::lumped_nuclear_structure::{LumpedNuclearStructure, LumpedRegion};
    use crate::genfoam::thermal_hydraulics::structure::nuclear_fuel_pin::{GapConductance, NuclearFuelPin, NuclearFuelPinRegion};
    use crate::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;

    /// The radial power-model type names this reader builds.
    pub const RADIAL_TYPES: [&str; 5] =
        ["heatedPin", "nuclearFuelPin", "nuclearFuelPinTest", "lumpedNuclearStructure", "interpolatedNuclearFuelPin"];

    /// The models of a case and the structure's initial `powerDensityNeutronics`.
    #[derive(Debug, Clone, PartialEq)]
    pub struct RadialRead {
        /// One model per type, in first-appearance order of the types in `structureProperties`.
        pub models: Vec<RadialPowerModel>,
        /// `powerDensityNeutronics` per cell: the file if present, else `powerDensity` of the `setRegionField` models.
        pub q_neutronics: Vec<f64>,
        /// Whether a `lumpedNuclearStructure` region has `kappaMatrix != 0` (its Laplacian is then evaluated).
        pub needs_laplacian: bool,
    }

    /// `(zone, powerModel dictionary)` of every region of one type, in `dicts.toc()` order.
    type Regions<'a> = Vec<(String, &'a FoamDict)>;

    fn scalar_list(ctx: &Ctx, d: &FoamDict, key: &str) -> Result<Vec<f64>, AppBuilderError> {
        match d.get(key) {
            Some(FoamEntry::List(v)) => v
                .iter()
                .map(|x| x.as_scalar().ok_or_else(|| ctx.e(format!("`{key}` must be a list of numbers"))))
                .collect(),
            _ => Err(ctx.e(format!("missing list `{key}`"))),
        }
    }

    fn label(ctx: &Ctx, d: &FoamDict, key: &str) -> Result<usize, AppBuilderError> {
        let v = ctx.f64(d, key)?;
        if v < 0.0 || v.fract() != 0.0 {
            return Err(ctx.e(format!("`{key}` must be a non-negative integer")));
        }
        Ok(v as usize)
    }

    /// `rho*Cp` or `rhoCp` (upstream: `if (found(rho) and found(Cp)) rho*Cp else if found(rhoCp) ... else FatalError`).
    fn rho_cp(ctx: &Ctx, d: &FoamDict, rho: &str, cp: &str, both: &str, what: &str) -> Result<f64, AppBuilderError> {
        if d.get(rho).is_some() && d.get(cp).is_some() {
            Ok(ctx.f64(d, rho)? * ctx.f64(d, cp)?)
        } else if d.get(both).is_some() {
            ctx.f64(d, both)
        } else {
            Err(ctx.e(format!("{what}: specify either {both} or both {rho} and {cp}")))
        }
    }

    /// `gapHPowerDensity table ((q h) ...)`: the `Function1` `table` form (ESI v2506), `clamp` outside the range.
    fn gap_table(ctx: &Ctx, e: &FoamEntry) -> Result<GapConductance, AppBuilderError> {
        let tokens = match e {
            FoamEntry::Tokens(t) => t.as_slice(),
            _ => return Err(ctx.e("gapHPowerDensity must read `table ((x y) ...)`")),
        };
        match tokens.first() {
            Some(FoamValue::Word(w)) if w == "table" => {}
            _ => return Err(ctx.e("gapHPowerDensity: only the `table` Function1 is ported")),
        }
        let rows = tokens
            .iter()
            .find_map(|v| if let FoamValue::List(l) = v { Some(l.clone()) } else { None })
            .ok_or_else(|| ctx.e("gapHPowerDensity.table holds no list"))?;
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for r in &rows {
            match r {
                FoamValue::List(p) if p.len() == 2 => match (p[0].as_scalar(), p[1].as_scalar()) {
                    (Some(x), Some(y)) => {
                        xs.push(Time::new::<second>(x));
                        ys.push(y);
                    }
                    _ => return Err(ctx.e("gapHPowerDensity.table rows must be (power density gap conductance)")),
                },
                _ => return Err(ctx.e("gapHPowerDensity.table rows must be (power density gap conductance)")),
            }
        }
        let t = TimeProfile::table(xs, ys).map_err(|er| ctx.e(format!("gapHPowerDensity.table: {er:?}")))?;
        Ok(GapConductance::TableOfPowerDensity(t))
    }

    fn model_err(ctx: &Ctx, ty: &str, e: crate::genfoam::thermal_hydraulics::structure::radial_solve::RadialError) -> AppBuilderError {
        ctx.e(format!("{ty}: {e}"))
    }

    /// Build the radial models of `structureProperties` `sp` (see the module docs).
    ///
    /// `zone_cells(zone)` is the cell list of one cellZone; `alpha_structure` the structure volume fraction per cell;
    /// `volumes` the cell volumes; `fdir` the start directory of the region (field files); `corrected_ring_areas` is the
    /// round-35 CONTROL for `nuclearFuelPin` (never the default).
    ///
    /// # Errors
    /// A missing key, an unported type or option, or a start directory with the models' own written fields.
    #[allow(clippy::too_many_arguments)]
    pub fn read_radial_power_models(
        ctx: &Ctx,
        sp: &FoamDict,
        zone_cells: &impl Fn(&str) -> Result<Vec<usize>, AppBuilderError>,
        alpha_structure: &[f64],
        volumes: &[f64],
        fdir: &Path,
        corrected_ring_areas: bool,
        q_file: Option<Vec<f64>>,
        heated_pin_q_file: Option<Vec<f64>>,
    ) -> Result<RadialRead, AppBuilderError> {
        let n = alpha_structure.len();
        // 1. the types in order of first appearance, and the regions of each (`dicts.add(zone, powerModelDict)`)
        let mut order: Vec<String> = Vec::new();
        let mut regions: Vec<Regions<'_>> = Vec::new();
        for (key, e) in sp.iter() {
            let FoamEntry::SubDict(zd) = e else { continue };
            let Some(pm) = zd.get_dict("powerModel") else { continue };
            let ty = ctx.word(pm, "type")?;
            if !RADIAL_TYPES.contains(&ty.as_str()) {
                continue;
            }
            let idx = match order.iter().position(|t| *t == ty) {
                Some(i) => i,
                None => {
                    order.push(ty.clone());
                    regions.push(Vec::new());
                    order.len() - 1
                }
            };
            for zone in key.split(':') {
                regions[idx].push((zone.to_string(), pm));
            }
        }
        let mut q = q_file.clone().unwrap_or_else(|| vec![0.0; n]);
        let mut models = Vec::new();
        let mut needs_laplacian = false;
        for (ty, regs) in order.iter().zip(&regions) {
            let own_files: &[&str] = match ty.as_str() {
                "heatedPin" => &["Trad.heatedPin", "Ti.heatedPin", "To.heatedPin", "powerDensity.heatedPin"],
                "nuclearFuelPin" => &["Trad.nuclearFuelPin", "Tfi.nuclearFuelPin", "Tfo.nuclearFuelPin", "Tci.nuclearFuelPin", "Tco.nuclearFuelPin"],
                "nuclearFuelPinTest" => &["Trad.nuclearFuelPinTest", "Tfi.nuclearFuelPinTest", "Tfo.nuclearFuelPinTest", "Tci.nuclearFuelPinTest", "Tco.nuclearFuelPinTest"],
                "lumpedNuclearStructure" => &["T.lumpedNuclearStructure", "Tmatrix.lumpedNuclearStructure", "Tsurface.lumpedNuclearStructure"],
                _ => &["T.interpolatedNuclearFuelPin"],
            };
            // `nuclearFuelPin`: `Trad.nuclearFuelPin` is READ (`foundTrad`, `nuclearFuelPin.C:212-215`) and then wins over `Tfi`, `Tfo`, `Tci`,
            // `Tco` (`foundBoundaryTemperatures` is only consulted when `Trad` is absent, `:441-537`: that reconstruction is not ported)
            // gh:#1950 (README round 75): `nuclearFuelPinTest.C:226-228,306,458` reads `Trad.nuclearFuelPinTest` exactly as `nuclearFuelPin` reads its own
            // (the two sources differ by comments and a commented-out `powerDensity`: `diff` after renaming the class), so a start directory that
            // holds it is a restart of the same kind
            let is_pin = ty == "nuclearFuelPin" || ty == "nuclearFuelPinTest";
            let trad_name = alloc::format!("Trad.{ty}");
            let trad_file = if is_pin && fdir.join(&trad_name).exists() { Some(fdir.join(&trad_name)) } else { None };
            for f in own_files {
                // `powerDensity.heatedPin` is READ when the caller supplies it (`heated_pin_q_file`): upstream's `setRegionField` then does nothing
                if *f == "powerDensity.heatedPin" && heated_pin_q_file.is_some() {
                    continue;
                }
                if trad_file.is_some() && is_pin {
                    continue;
                }
                if fdir.join(f).exists() {
                    return Err(ctx.e(format!("{ty}: the start directory holds `{f}` (a restart from the model's own state is not ported)")));
                }
            }
            let pm_alpha = |pm: &FoamDict| scalar(pm, "volumeFraction");
            // setRegionField(powerDensityNeutronics, "powerDensity") for the two models that use it
            let mut set_q = |ctx: &Ctx, pm: &FoamDict, cells: &[usize]| -> Result<(), AppBuilderError> {
                if q_file.is_none() {
                    let v = ctx.f64(pm, "powerDensity")?;
                    for &c in cells {
                        q[c] = v;
                    }
                }
                Ok(())
            };
            match ty.as_str() {
                "heatedPin" => {
                    let mut rs = Vec::new();
                    for (zone, pm) in regs {
                        let cells = zone_cells(zone)?;
                        rs.push(HeatedPinRegion {
                            cells,
                            inner_radius: ctx.f64(pm, "innerRadius")?,
                            outer_radius: ctx.f64(pm, "outerRadius")?,
                            mesh_size: label(ctx, pm, "meshSize")?,
                            rho_cp: rho_cp(ctx, pm, "rho", "Cp", "rhoCp", &format!("heatedPin region {zone}"))?,
                            k: ctx.f64(pm, "k")?,
                            initial_temperature: ctx.f64(pm, "T")?,
                            power_density: if heated_pin_q_file.is_some() { 0.0 } else { ctx.f64(pm, "powerDensity")? },
                            volume_fraction: pm_alpha(pm),
                        });
                    }
                    let mut m = HeatedPin::new(n, rs, alpha_structure, volumes, 0).map_err(|e| model_err(ctx, ty, e))?;
                    if let Some(file) = &heated_pin_q_file {
                        m.set_power_density_field(file);
                    }
                    models.push(RadialPowerModel::HeatedPin(m));
                }
                "nuclearFuelPin" | "nuclearFuelPinTest" => {
                    let mut rs = Vec::new();
                    for (zone, pm) in regs {
                        let cells = zone_cells(zone)?;
                        set_q(ctx, pm, &cells)?;
                        let table = pm.get("gapHPowerDensity");
                        let value = scalar(pm, "gapH");
                        let gap = match (table, value) {
                            (Some(_), Some(_)) => {
                                return Err(ctx.e(format!("nuclearFuelPin region {zone}: provide either a gapH value or a gapHPowerDensityTable but not both")))
                            }
                            (Some(t), None) => gap_table(ctx, t)?,
                            (None, v) => GapConductance::Constant(v.unwrap_or(0.0)),
                        };
                        rs.push(NuclearFuelPinRegion {
                            cells,
                            fraction_of_power: scalar(pm, "fractionOfPowerFromNeutronics").unwrap_or(1.0),
                            rfi: ctx.f64(pm, "fuelInnerRadius")?,
                            rfo: ctx.f64(pm, "fuelOuterRadius")?,
                            rci: ctx.f64(pm, "cladInnerRadius")?,
                            rco: ctx.f64(pm, "cladOuterRadius")?,
                            fuel_mesh_size: label(ctx, pm, "fuelMeshSize")?,
                            clad_mesh_size: label(ctx, pm, "cladMeshSize")?,
                            rho_cp_fuel: rho_cp(ctx, pm, "fuelRho", "fuelCp", "fuelRhoCp", &format!("nuclearFuelPin region {zone}"))?,
                            rho_cp_clad: rho_cp(ctx, pm, "cladRho", "cladCp", "cladRhoCp", &format!("nuclearFuelPin region {zone}"))?,
                            k_fuel: ctx.f64(pm, "fuelK")?,
                            k_clad: ctx.f64(pm, "cladK")?,
                            fuel_t0: ctx.f64(pm, "fuelT")?,
                            clad_t0: ctx.f64(pm, "cladT")?,
                            gap,
                            volume_fraction: pm_alpha(pm),
                        });
                    }
                    let m = if corrected_ring_areas {
                        NuclearFuelPin::with_corrected_ring_areas(n, rs, alpha_structure, volumes, 0)
                    } else {
                        NuclearFuelPin::new(n, rs, alpha_structure, volumes, 0)
                    }
                    .map_err(|e| model_err(ctx, ty, e))?;
                    let m = match &trad_file {
                        Some(path) => {
                            let trad = read_scalar_field_field(path).map_err(|e| ctx.e(format!("{}: {e}", path.display())))?;
                            m.with_read_trad(trad, volumes).map_err(|e| model_err(ctx, ty, e))?
                        }
                        None => m,
                    };
                    models.push(if ty == "nuclearFuelPinTest" { RadialPowerModel::NuclearFuelPinTest(m) } else { RadialPowerModel::NuclearFuelPin(m) });
                }
                "lumpedNuclearStructure" => {
                    let mut rs = Vec::new();
                    for (zone, pm) in regs {
                        let cells = zone_cells(zone)?;
                        set_q(ctx, pm, &cells)?;
                        let kappa = scalar(pm, "kappaMatrix").unwrap_or(0.0);
                        needs_laplacian |= kappa != 0.0;
                        rs.push(LumpedRegion {
                            cells,
                            volumetric_area: ctx.f64(pm, "volumetricArea")?,
                            fraction_of_power: scalar(pm, "fractionOfPowerFromNeutronics").unwrap_or(1.0),
                            nodes_number: label(ctx, pm, "nodesNumber")?,
                            node_fuel: label(ctx, pm, "nodeFuel")?,
                            node_clad: label(ctx, pm, "nodeClad")?,
                            node_matrix: if pm.get("nodeMatrix").is_some() { label(ctx, pm, "nodeMatrix")? } else { 0 },
                            kappa_matrix: kappa,
                            heat_conductances: scalar_list(ctx, pm, "heatConductances")?,
                            rho_cp: scalar_list(ctx, pm, "rhoCp")?,
                            volume_fractions: scalar_list(ctx, pm, "volumeFractions")?,
                            power_fractions: scalar_list(ctx, pm, "powerFractions")?,
                            t0: ctx.f64(pm, "T0")?,
                            volume_fraction: pm_alpha(pm),
                        });
                    }
                    let m = LumpedNuclearStructure::new(n, rs, alpha_structure, 0).map_err(|e| model_err(ctx, ty, e))?;
                    models.push(RadialPowerModel::LumpedNuclearStructure(m));
                }
                _ => {
                    let mut rs = Vec::new();
                    for (zone, pm) in regs {
                        // upstream's `setRegionField(interpolatedPowerDensity_, "powerDensity")` requires the key
                        ctx.f64(pm, "powerDensity")?;
                        rs.push(InterpolatedRegion {
                            cells: zone_cells(zone)?,
                            fuel_outer_radius: ctx.f64(pm, "fuelOuterRadius")?,
                            t0: ctx.f64(pm, "T")?,
                            volume_fraction: pm_alpha(pm),
                        });
                    }
                    let m = InterpolatedNuclearFuelPin::new(n, &rs, alpha_structure).map_err(|e| model_err(ctx, ty, e))?;
                    models.push(RadialPowerModel::InterpolatedNuclearFuelPin(m));
                }
            }
        }
        Ok(RadialRead { models, q_neutronics: q, needs_laplacian })
    }

    /// Whether `type` is a power model the one-phase region can neither build here nor treat as `fixedTemperature`:
    /// the name for the refusal message.
    #[must_use]
    pub fn refusal_reason(ty: &str) -> String {
        match ty {
            "nuclearFuelFMU" | "fixedPowerFMU" | "fixedTemperatureFMU" => format!("the FMU powerModel `{ty}` is not ported (needs FMU4FOAM, gh:#1786)"),
            "fixedPower" | "nuclearSteadyStatePebble" => format!("the `{ty}` powerModel is not ported in the one-phase cell-centred region"),
            other => format!("the `{other}` powerModel is not ported here"),
        }
    }
/// An `IOFieldField<Field, scalar>` file (`class scalarFieldField`): `N ( M ( v ... ) M ( ... ) ... )`, one list per cell (a cell outside the
/// model holds an empty list), numbers read with `parse_foam_f64`.
///
/// # Errors
/// A message naming what is malformed.
pub fn read_scalar_field_field(path: &Path) -> Result<Vec<Vec<f64>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_scalar_field_field(&crate::io::poly_mesh::strip_foam_comments(&text))
}

/// [`read_scalar_field_field`] on the text (comments already stripped).
///
/// # Errors
/// A message naming what is malformed.
pub fn parse_scalar_field_field(text: &str) -> Result<Vec<Vec<f64>>, String> {
    let body = match text.find('}') {
        Some(i) => &text[i + 1..],
        None => text,
    };
    let toks: Vec<&str> = body.split(|c: char| c.is_whitespace()).filter(|t| !t.is_empty()).flat_map(split_parens).collect();
    let mut it = toks.into_iter();
    let n: usize = it.next().ok_or("empty file")?.parse().map_err(|_| "no list count".to_string())?;
    if it.next() != Some("(") {
        return Err("no `(` after the list count".to_string());
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let m: usize = it.next().ok_or_else(|| format!("entry {i}: missing"))?.parse().map_err(|_| format!("entry {i}: no size"))?;
        if m == 0 {
            // `0()` or `0` followed by `(` `)`
            if it.clone().next() == Some("(") {
                it.next();
                if it.next() != Some(")") {
                    return Err(format!("entry {i}: empty list not closed"));
                }
            }
            out.push(Vec::new());
            continue;
        }
        if it.next() != Some("(") {
            return Err(format!("entry {i}: no `(`"));
        }
        let mut v = Vec::with_capacity(m);
        for _ in 0..m {
            let t = it.next().ok_or_else(|| format!("entry {i}: short"))?;
            v.push(outram_foam_basic_lib::io::parse_foam_f64(t).ok_or_else(|| format!("entry {i}: `{t}` is not a number"))?);
        }
        if it.next() != Some(")") {
            return Err(format!("entry {i}: not closed"));
        }
        out.push(v);
    }
    Ok(out)
}

fn split_parens(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in t.char_indices() {
        if c == '(' || c == ')' {
            if start < i {
                out.push(&t[start..i]);
            }
            out.push(&t[i..i + 1]);
            start = i + 1;
        }
    }
    if start < t.len() {
        out.push(&t[start..]);
    }
    out
}

}

#[cfg(feature = "std")]
pub use reader::{read_radial_power_models, refusal_reason, RADIAL_TYPES};
