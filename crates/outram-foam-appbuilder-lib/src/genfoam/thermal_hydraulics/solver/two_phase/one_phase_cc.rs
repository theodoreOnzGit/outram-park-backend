// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/onePhase/onePhase.C
//     (correctPhysics, correctTightlyCoupledPhysics, correctEnergy, correctModels,
//      correctCourant, correctContErr, maxDeltaT),
//     include/equations/EEqn_1p.H,
//   and the classes those call: src/phaseModels/fluid/fluid.C (`fluid::rho`, Boussinesq),
//   src/phaseModels/structureModels/{structure.C (correct, linearizedSemiImplicitHeatSource),
//     powerModels/fixedTemperature/fixedTemperature.C},
//   src/physicsModels/phasePairs/FSPair.C (correct), templatedModels/constant/constantModel.C,
//   heatTransferModels/FSHeatTransferCoefficientModels/FSHeatTransferCoefficientModel.C (correctField),
//   and OpenFOAM-ESI v2506 `heThermo::alphaEff`, `species::thermo::T` (THs), `hConstThermo`,
//   `Boussinesq`, `fvMatrix::relax(alpha)`, `PBiCGStab`.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `two_phase::one_phase_cc` — GeN-Foam's `onePhase` solver, `solveFluidMechanics false`
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The thermal-hydraulic half of the `2D_MSFR` `transient` stage (gh:#1773, gh:#1361 round 9): a
//! cell-centred, **energy-only** `onePhase` region. The flux is frozen at its restart value
//! (`alphaPhi`, `alphaRhoPhi`, `U` read from the start directory, never updated); `onePhase::correctPhysics`
//! then runs, per PIMPLE outer iteration, `correctModels(true, true)` (`FSPair::correct`) and
//! `correctEnergy` (`EEqn_1p.H`). **`turbulence->correct()` is NOT called** (it sits in `UEqn_1p.H`, which
//! only runs when `solveFluidMechanics` is true), so the energy equation's `alphaEff` is
//! `thermo.alpha + alphat` with `alphat` read once from the restart (gh:#1775: read, never solved).
//!
//! ## What upstream does that is easy to get wrong (all read from the source, none tuned)
//!
//! * **`fluid::rho()` is `rho0` for a `Boussinesq` equation of state** (`fluid.C:795`): `ddt(alpha, rho, he)`,
//!   `alpha rho (U.g)` and `ddt(alpha, rho, K)` use the CONSTANT `rho0`; only the thermo's own `rho(T)`
//!   (mapped to the neutronics as `rhoCool`) varies.
//! * `hConst` + `Boussinesq`: `hs = Cp (T - Tref) + Hsref` (`Boussinesq::H = 0` in v2506), `Tref = Tstd = 298.15`.
//!   `T` comes back from `he` by `species::thermo::T`'s Newton loop (`Ttol = T0 1e-4`), written out here with
//!   upstream's operation order.
//! * `alphaEff = CpByCpv (alpha_thermo + alphat)`, `CpByCpv = 1` for an enthalpy form, `alpha_thermo = mu/Pr`
//!   (`constTransport::alphah = mu rPr`).
//! * The `h` equation is solved by `PBiCGStab`/`diagonal` (the catch-all `".*"` solver wins over `"h.*"`: OpenFOAM
//!   takes the LAST matching pattern), with `fvMatrix::relax(0.9)` from `relaxationFactors.equations`.
//! * `fvc::ddt(p)` is exactly 0 (`p` is never modified) and `K.oldTime()` of a fresh local field is `K`.
//! * The wedge patches carry a ~1e-11 residual flux (read from the restart); like upstream it enters `div`
//!   (diagonal), `contErr` and the Courant number.
//!
//! ## Not ported (refused by name in [`OnePhaseCcCase::read`])
//!
//! ~~`solveFluidMechanics true` (UEqn/pEqn, `porousKEpsilon` solving: gh:#1775)~~ **CORRECTED 2026-11-26 (gh:#1837, round 19):** `solveFluidMechanics true` with `turbulence false` is ported ([`flow`]); `turbulence true` (wall functions, `porousKEpsilon::correct`: gh:#1838, gh:#1839) is still refused; heat exchangers, pumps (a `momentumSource` pump IS read when the flow is solved),
//! passive structures, tortuosity, local frames; power models other than `fixedTemperature`
//! (without a time profile); heat-transfer models other than `constant`; thermo other than `heRhoThermo`
//! `hConst` `Boussinesq` `const`; `residualControl` in `PIMPLE`. `dragModels` (`ReynoldsPower`) are read but
//! not evaluated: `Kd` is a momentum coefficient and does not enter the energy equation.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::path::Path;

use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

mod flow;
pub use flow::{OnePhaseFlowCase, PressureSolve, ReynoldsPowerDrag};
use flow::FlowState;

use super::fv::{self, Bc, Eqn, Geometry, Scalar, Vector};
use super::pbicgstab::{pbicgstab_diagonal, DEFAULT_MAX_ITER};
use crate::error::AppBuilderError;

#[cfg(feature = "std")]
use super::case::{bool_entry, scalar, word, Ctx};
#[cfg(feature = "std")]
use crate::io::field_reader::{read_vol_scalar_field, read_vol_vector_field};
#[cfg(feature = "std")]
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh};
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry};

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM `Tstd` (ESI v2506).
const TSTD: f64 = 298.15;

/// `fvSolution`/`controlDict` entries the stage reads.
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseCcControls {
    /// `PIMPLE.nOuterCorrectors`.
    pub n_outer: usize,
    /// `solvers.h.tolerance` (the entry that wins for `h`).
    pub h_tolerance: f64,
    /// `solvers.h.relTol`.
    pub h_rel_tol: f64,
    /// `solvers.h.minIter`.
    pub h_min_iter: usize,
    /// `relaxationFactors.equations` for `h` (`None` if absent).
    pub relax_h_equation: Option<f64>,
    /// `controlDict.includeKineticEnergy` (default true).
    pub include_kinetic_energy: bool,
    /// `controlDict.maxCo` (default 1).
    pub max_co: f64,
    /// `controlDict.maxDeltaT` (default `GREAT`).
    pub max_delta_t: f64,
}

/// `heatTransferModels."<zone>"` (closed set).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsHtcModel {
    /// `type constant; value v;` (`constantModel`): `htc[celli] = value`.
    Constant(f64),
}

impl FsHtcModel {
    /// `FSHeatTransferCoefficientModel::value(celli)` [W/m^2/K].
    #[must_use]
    pub fn value(&self) -> f64 {
        match self {
            Self::Constant(v) => *v,
        }
    }
}

/// `heRhoThermo<pureMixture<const<hConst<Boussinesq<specie>>>>>`, sensible enthalpy.
#[derive(Debug, Clone, PartialEq)]
pub struct BoussinesqHConst {
    /// `thermodynamics.Cp` [J/kg/K].
    pub cp: f64,
    /// `Tref` (default `Tstd`).
    pub tref: f64,
    /// `Href` (default 0).
    pub hsref: f64,
    /// `equationOfState.rho0`.
    pub rho0: f64,
    /// `equationOfState.T0`.
    pub t0: f64,
    /// `equationOfState.beta`.
    pub beta: f64,
    /// `transport.mu`.
    pub mu: f64,
    /// `1/Pr` (`constTransport::rPr_`).
    pub r_pr: f64,
}

impl BoussinesqHConst {
    /// `hConstThermo::Hs`: `Cp (T - Tref) + Hsref + Boussinesq::H` (`H = 0`).
    #[must_use]
    pub fn hs(&self, t: f64) -> f64 {
        self.cp * (t - self.tref) + self.hsref + 0.0
    }

    /// `Boussinesq::rho(p, T) = rho0 (1 - beta (T - T0))`.
    #[must_use]
    pub fn rho(&self, t: f64) -> f64 {
        self.rho0 * (1.0 - self.beta * (t - self.t0))
    }

    /// `constTransport::alphah(p, T) = mu rPr`.
    #[must_use]
    pub fn alpha_h(&self) -> f64 {
        self.mu * self.r_pr
    }

    /// `species::thermo::THs`: Newton from `t0` (`Ttol = T0 1e-4`, at most 100 iterations; upstream aborts
    /// past that, this returns the last iterate).
    #[must_use]
    pub fn t_from_hs(&self, f: f64, t0: f64) -> f64 {
        let mut test;
        let mut tnew = t0;
        let ttol = t0 * 1.0e-4;
        let mut iter = 0;
        loop {
            test = tnew;
            tnew = test - (self.hs(test) - f) / (self.cp + 0.0);
            iter += 1;
            if (tnew - test).abs() <= ttol || iter > 100 {
                break;
            }
        }
        tnew
    }
}

/// A read case (all fields at the start time).
#[derive(Debug, Clone)]
pub struct OnePhaseCcCase {
    /// Mesh.
    pub mesh: Arc<FvMesh>,
    /// Gravity [m/s^2].
    pub gravity: Vector3,
    /// Thermophysical package.
    pub thermo: BoussinesqHConst,
    /// Controls.
    pub controls: OnePhaseCcControls,
    /// `startTime`.
    pub start_time: f64,
    /// `endTime`.
    pub end_time: f64,
    /// `deltaT`.
    pub delta_t: f64,
    /// Structure volume fraction per cell (`volumeFraction` of the zone).
    pub alpha_structure: Vec<f64>,
    /// Structure hydraulic diameter per cell (`Dh`).
    pub dh: Vec<f64>,
    /// `volumetricArea.activeStructure`: the `fixedTemperature` power model's area per cell.
    pub ia_act: Vec<f64>,
    /// `T.fixedTemperature` (read from the start directory) per cell.
    pub t_fixed: Vec<f64>,
    /// `heatTransferModels` per cell.
    pub htc_model: Vec<Option<FsHtcModel>>,
    /// `htc` read from the start directory (cells outside the model zones keep it).
    pub htc0: Vec<f64>,
    /// `T`.
    pub t: Vec<f64>,
    /// `p`.
    pub p: Vec<f64>,
    /// `U`.
    pub u: Vec<Vector3>,
    /// `alphat` (turbulent thermal diffusivity, read, never solved).
    pub alphat: Vec<f64>,
    /// `alphaRhoPhi`, every face (boundary faces included, wedge residuals and all).
    pub alpha_rho_phi: Vec<f64>,
    /// `alphaPhi`, every face.
    pub alpha_phi: Vec<f64>,
    /// `powerDensityNeutronicsToLiquid`.
    pub power_density: Vec<f64>,
    /// `residualFluidStructureRe` (default 10).
    pub min_re: f64,
    /// `solveFluidMechanics true`: the momentum/pressure part of the case (`None` for the frozen-flux region).
    pub flow: Option<OnePhaseFlowCase>,
}

/// Parse `internalField` and the `value` of the named patches of a surface (or any scalar) field file.
///
/// `patches` is `(name, size)`; a patch without a `value` entry gets zeros. Pure text work, so it is
/// available without `std`.
///
/// # Errors
/// [`AppBuilderError::Case`]-free `String` messages are mapped by the caller; here a `String` error.
pub fn parse_scalar_field_text(
    text: &str,
    n_internal: usize,
    patches: &[(String, usize)],
) -> Result<(Vec<f64>, Vec<Vec<f64>>), String> {
    let ip = text.find("internalField").ok_or("no internalField")?;
    let (internal, _) = parse_value(&text[ip + "internalField".len()..], n_internal)?;
    let bp = text.find("boundaryField").ok_or("no boundaryField")?;
    let bf = &text[bp..];
    let mut out = Vec::with_capacity(patches.len());
    for (name, size) in patches {
        let mut found: Option<&str> = None;
        for (pos, _) in bf.match_indices(name.as_str()) {
            let before_ok = bf[..pos].chars().next_back().is_some_and(char::is_whitespace);
            let after = &bf[pos + name.len()..];
            let after_t = after.trim_start();
            if before_ok && after_t.starts_with('{') && after.chars().next().is_some_and(char::is_whitespace) {
                let close = after_t.find('}').ok_or("unterminated patch block")?;
                found = Some(&after_t[1..close]);
                break;
            }
        }
        let block = found.ok_or_else(|| format!("no patch block `{name}`"))?;
        let mut vals = vec![0.0; *size];
        for (pos, _) in block.match_indices("value") {
            let before_ok = block[..pos].chars().next_back().is_none_or(char::is_whitespace);
            if before_ok {
                let (v, _) = parse_value(&block[pos + 5..], *size)?;
                vals = v;
                break;
            }
        }
        out.push(vals);
    }
    Ok((internal, out))
}

/// `uniform x` or `nonuniform List<scalar> N ( ... )` at the start of `s`; returns the values and the rest.
fn parse_value(s: &str, n: usize) -> Result<(Vec<f64>, &str), String> {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix("uniform") {
        let end = r.find(';').ok_or("uniform without `;`")?;
        let v: f64 = r[..end].trim().parse().map_err(|_| format!("bad uniform `{}`", r[..end].trim()))?;
        return Ok((vec![v; n], &r[end..]));
    }
    if let Some(r) = s.strip_prefix("nonuniform") {
        let r = r.trim_start();
        let r = r.strip_prefix("List<scalar>").unwrap_or(r).trim_start();
        let open = r.find('(').ok_or("nonuniform without `(`")?;
        let count: usize = r[..open].trim().parse().map_err(|_| "bad list size".to_string())?;
        let close = r[open..].find(')').ok_or("unterminated list")? + open;
        let vals: Result<Vec<f64>, _> = r[open + 1..close].split_whitespace().map(str::parse::<f64>).collect();
        let vals = vals.map_err(|_| "bad number in list".to_string())?;
        if vals.len() != count {
            return Err(format!("list says {count} values, has {}", vals.len()));
        }
        return Ok((vals, &r[close + 1..]));
    }
    Err("neither uniform nor nonuniform".to_string())
}

#[cfg(feature = "std")]
fn last_match<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    // OpenFOAM: an exact keyword first, else the LAST matching pattern.
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().filter(|(k, _)| super::case::key_matches(k, name)).last().map(|(_, e)| e)
}

#[cfg(feature = "std")]
impl OnePhaseCcCase {
    /// Read a GeN-Foam `onePhase` region case with `solveFluidMechanics false`: dictionaries under
    /// `root/constant/fluidRegion`, `root/system`, fields under `root/<startTime>/fluidRegion`, mesh from
    /// `poly_mesh_dir`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] for a missing or unreadable file and for every selection not ported (see the
    /// module documentation).
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path, poly_mesh_dir: &Path) -> Result<Self, AppBuilderError> {
        let ctx = Ctx { root: root.to_path_buf() };
        let region = "fluidRegion";
        let mesh = read_poly_mesh(poly_mesh_dir)?;
        let zones = read_cell_zones(poly_mesh_dir)?;
        let g = Geometry::new(mesh.clone());
        let n = g.n_cells();
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

        let gd = ctx.read(&format!("constant/{region}/g"))?;
        let gravity = match gd.get("value") {
            Some(FoamEntry::List(v)) if v.len() == 3 => Vector3::new(
                v[0].as_scalar().unwrap_or(0.0),
                v[1].as_scalar().unwrap_or(0.0),
                v[2].as_scalar().unwrap_or(0.0),
            ),
            _ => return Err(ctx.e("g: missing `value (x y z)`")),
        };

        let cd = ctx.read("system/controlDict")?;
        let start_time = scalar(&cd, "startTime").unwrap_or(0.0);
        let end_time = ctx.f64(&cd, "endTime")?;
        let delta_t = ctx.f64(&cd, "deltaT")?;
        let include_kinetic_energy = bool_entry(&cd, "includeKineticEnergy").unwrap_or(true);
        let max_co = scalar(&cd, "maxCo").unwrap_or(1.0);
        let max_delta_t = scalar(&cd, "maxDeltaT").unwrap_or(1.0e300);
        let is_one_phase = |name: &str| -> bool {
            cd.get_dict("regionSolvers").and_then(|rs| word(rs, name)).as_deref() == Some("onePhase")
        };
        let in_multi = cd
            .get_dict("regionSolvers")
            .map(|rs| rs.iter().any(|(_, e)| matches!(e, FoamEntry::Word(w) if w == "multiPhysicsSolver")))
            .unwrap_or(false);
        if !is_one_phase(region) && !in_multi {
            return Err(ctx.e("regionSolvers: fluidRegion is neither onePhase nor inside a multiPhysicsSolver"));
        }

        // ---- fvSolution
        let fs = ctx.read(&format!("system/{region}/fvSolution"))?;
        let pimple = ctx.sub(&fs, "PIMPLE")?;
        let solve_flow = match bool_entry(pimple, "solveFluidMechanics") {
            Some(b) => b,
            None => return Err(ctx.e("PIMPLE.solveFluidMechanics must be given")),
        };
        if bool_entry(pimple, "solveEnergy") != Some(true) {
            return Err(ctx.e("PIMPLE.solveEnergy must be true"));
        }
        if pimple.get("residualControl").is_some() || pimple.get_dict("residualControl").is_some() {
            return Err(ctx.e("PIMPLE.residualControl is not ported (customPimpleControl convergence in the energy-only region)"));
        }
        let n_outer = scalar(pimple, "nOuterCorrectors").unwrap_or(1.0) as usize;
        let solvers = ctx.sub(&fs, "solvers")?;
        let (h_tolerance, h_rel_tol, h_min_iter) = match last_match(solvers, "h") {
            Some(FoamEntry::SubDict(d)) => {
                if word(d, "solver").as_deref() != Some("PBiCGStab")
                    || word(d, "preconditioner").as_deref() != Some("diagonal")
                {
                    return Err(ctx.e("solvers for `h`: only PBiCGStab with the diagonal preconditioner is ported"));
                }
                (
                    scalar(d, "tolerance").unwrap_or(1e-6),
                    scalar(d, "relTol").unwrap_or(0.0),
                    scalar(d, "minIter").unwrap_or(0.0) as usize,
                )
            }
            _ => return Err(ctx.e("fvSolution: no solver entry matches `h`")),
        };
        let relax_h_equation = fs
            .get_dict("relaxationFactors")
            .and_then(|r| r.get_dict("equations"))
            .and_then(|e| match last_match(e, "h") {
                Some(FoamEntry::Scalar(x)) => Some(*x),
                _ => None,
            });
        let controls = OnePhaseCcControls {
            n_outer,
            h_tolerance,
            h_rel_tol,
            h_min_iter,
            relax_h_equation,
            include_kinetic_energy,
            max_co,
            max_delta_t,
        };

        // ---- turbulence: alphat is read, never solved
        let tp = ctx.read(&format!("constant/{region}/turbulenceProperties"))?;
        match word(&tp, "simulationType").as_deref() {
            Some("RAS") => {
                let ras = ctx.sub(&tp, "RAS")?;
                if word(ras, "RASModel").as_deref() != Some("porousKEpsilon") {
                    return Err(ctx.e("RASModel other than porousKEpsilon is not ported"));
                }
            }
            _ => return Err(ctx.e("simulationType other than RAS is not ported")),
        }

        // ---- thermo
        let th = ctx.read(&format!("constant/{region}/thermophysicalProperties"))?;
        let tt = ctx.sub(&th, "thermoType")?;
        for (k, want) in [
            ("type", "heRhoThermo"),
            ("mixture", "pureMixture"),
            ("transport", "const"),
            ("thermo", "hConst"),
            ("equationOfState", "Boussinesq"),
            ("energy", "sensibleEnthalpy"),
        ] {
            if word(tt, k).as_deref() != Some(want) {
                return Err(ctx.e(format!("thermoType.{k} must be {want} (the only package ported here)")));
            }
        }
        let mx = ctx.sub(&th, "mixture")?;
        let eos = ctx.sub(mx, "equationOfState")?;
        let thd = ctx.sub(mx, "thermodynamics")?;
        let tr = ctx.sub(mx, "transport")?;
        let thermo = BoussinesqHConst {
            cp: ctx.f64(thd, "Cp")?,
            tref: scalar(thd, "Tref").unwrap_or(TSTD),
            hsref: scalar(thd, "Href").unwrap_or(0.0),
            rho0: ctx.f64(eos, "rho0")?,
            t0: ctx.f64(eos, "T0")?,
            beta: ctx.f64(eos, "beta")?,
            mu: ctx.f64(tr, "mu")?,
            r_pr: 1.0 / ctx.f64(tr, "Pr")?,
        };

        // ---- phaseProperties: structure, power model, closures
        let pp = ctx.read(&format!("constant/{region}/phaseProperties"))?;
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_structure = vec![0.0; n];
        let mut dh = vec![1.0; n];
        let mut ia_act = vec![0.0; n];
        let mut fixed_cells = vec![false; n];
        let mut momentum_source = vec![Vector3::ZERO; n];
        let mut t_fixed_default = vec![0.0; n];
        for (key, e) in sp.iter() {
            let FoamEntry::SubDict(zd) = e else { continue };
            for refused in ["powerOffCriterionModel", "heatExchangers"] {
                if key == refused {
                    return Err(ctx.e(format!("structureProperties.{refused} is not ported")));
                }
            }
            let cells = zone_cells(key)?;
            if solve_flow {
                if let Some(FoamEntry::List(v)) = zd.get("momentumSource") {
                    if zd.get("momentumSourceTimeProfile").is_some() || zd.get_dict("momentumSourceTimeProfile").is_some() {
                        return Err(ctx.e("momentumSourceTimeProfile is not ported"));
                    }
                    if v.len() != 3 {
                        return Err(ctx.e("momentumSource must be a vector"));
                    }
                    let src = Vector3::new(
                        v[0].as_scalar().unwrap_or(0.0),
                        v[1].as_scalar().unwrap_or(0.0),
                        v[2].as_scalar().unwrap_or(0.0),
                    );
                    for &c in &cells {
                        momentum_source[c] = src;
                    }
                }
            }
            let vf = ctx.f64(zd, "volumeFraction")?;
            let d = ctx.f64(zd, "Dh")?;
            for &c in &cells {
                alpha_structure[c] = vf;
                dh[c] = d;
            }
            for (k2, _) in zd.iter() {
                if !matches!(k2, "volumeFraction" | "Dh" | "powerModel" | "momentumSource") {
                    return Err(ctx.e(format!("structureProperties.{key}.{k2} is not ported")));
                }
            }
            if let Some(pm) = zd.get_dict("powerModel") {
                if word(pm, "type").as_deref() != Some("fixedTemperature") {
                    return Err(ctx.e("only the fixedTemperature powerModel is ported here"));
                }
                if pm.get("temperatureTimeProfile").is_some() || pm.get_dict("temperatureTimeProfile").is_some() {
                    return Err(ctx.e("fixedTemperature temperatureTimeProfile is not ported"));
                }
                let ia = ctx.f64(pm, "volumetricArea")?;
                let t_pm = scalar(pm, "T");
                for &c in &cells {
                    ia_act[c] = ia;
                    fixed_cells[c] = true;
                    if let Some(t) = t_pm {
                        t_fixed_default[c] = t;
                    }
                }
            }
        }
        let pm_dict = ctx.sub(&pp, "physicsModels")?;
        let htcd = ctx.sub(pm_dict, "heatTransferModels")?;
        let mut htc_model: Vec<Option<FsHtcModel>> = vec![None; n];
        for (zk, e) in htcd.iter() {
            let FoamEntry::SubDict(md) = e else { continue };
            let model = match ctx.word(md, "type")?.as_str() {
                "constant" => FsHtcModel::Constant(ctx.f64(md, "value")?),
                other => return Err(ctx.e(format!("heatTransferModels.{zk}: `{other}` is not ported here"))),
            };
            for c in zone_cells(zk)? {
                htc_model[c] = Some(model);
            }
        }
        // dragModels: the energy equation does not see `Kd`; the solved flow does (`ReynoldsPower` only)
        let dragd = ctx.sub(pm_dict, "dragModels")?;
        let mut drag: Vec<Option<ReynoldsPowerDrag>> = vec![None; n];
        if solve_flow {
            for (zk, e) in dragd.iter() {
                let FoamEntry::SubDict(md) = e else { continue };
                if ctx.word(md, "type")? != "ReynoldsPower" {
                    return Err(ctx.e(format!("dragModels.{zk}: only ReynoldsPower is ported for the solved flow")));
                }
                let model = ReynoldsPowerDrag {
                    coeff: ctx.f64(md, "coeff")?,
                    exp: ctx.f64(md, "exp")?,
                    constant: scalar(md, "const").unwrap_or(0.0),
                };
                for c in zone_cells(zk)? {
                    drag[c] = Some(model);
                }
            }
        }
        let min_re = scalar(&pp, "residualFluidStructureRe").unwrap_or(10.0);

        // ---- fields at the start time
        let tdir = alloc::format!("{start_time}");
        let fdir = root.join(&tdir).join(region);
        let sfield = |name: &str| read_vol_scalar_field(&fdir.join(name), n);
        let t = sfield("T")?;
        let p = sfield("p")?;
        let alphat = sfield("alphat")?;
        // upstream creates what a ROOT case lacks: `T.fixedTemperature` (`T` of the power-model dictionary in its zone,
        // 0 elsewhere), zero `htc` and `powerDensityNeutronicsToLiquid`; only a case that solves the flow may start
        // from them (the frozen-flux region needs a restart, its flux comes from the file)
        let exists = |name: &str| fdir.join(name).exists();
        let t_fixed = if solve_flow && !exists("T.fixedTemperature") { t_fixed_default } else { sfield("T.fixedTemperature")? };
        let htc0 = if solve_flow && !exists("htc") { vec![0.0; n] } else { sfield("htc")? };
        let power_density = if solve_flow && !exists("powerDensityNeutronicsToLiquid") {
            vec![0.0; n]
        } else {
            sfield("powerDensityNeutronicsToLiquid")?
        };
        if solve_flow && pp.get_dict("fluidProperties").is_some() {
            return Err(ctx.e("fluidProperties/initialPowerDensity/initialAlpha are not ported for the solved flow"));
        }
        let u = read_vol_vector_field(&fdir.join("U"), n)?;
        let patches: Vec<(String, usize)> = mesh.patches.iter().map(|q| (q.name.clone(), q.size)).collect();
        let ni = g.n_internal();
        let surface = |name: &str| -> Result<Vec<f64>, AppBuilderError> {
            let path = fdir.join(name);
            let text = std::fs::read_to_string(&path).map_err(|e| AppBuilderError::Io { path: path.clone(), source: e })?;
            let (internal, per_patch) =
                parse_scalar_field_text(&text, ni, &patches).map_err(|m| ctx.e(format!("{name}: {m}")))?;
            let mut out = vec![0.0; g.n_faces()];
            out[..ni].copy_from_slice(&internal);
            for (q, vals) in mesh.patches.iter().zip(per_patch) {
                for (k, v) in vals.into_iter().enumerate() {
                    out[q.start + k] = v;
                }
            }
            Ok(out)
        };
        let flow = if solve_flow {
            Some(flow::read_flow_case(&ctx, &g, &tdir, pimple, &fs, &pp, &tp, momentum_source, drag)?)
        } else {
            None
        };
        let (alpha_rho_phi, alpha_phi) = if solve_flow && !exists("alphaPhi") {
            // `fluid::initAlphaPhis`: alphaPhi = interpolate(alpha) phi, alphaRhoPhi = interpolate(rho0) alphaPhi
            let fcase = flow.as_ref().expect("flow case");
            let mut uf = Vector::uniform(&g, Vector3::ZERO);
            uf.c = u.clone();
            uf.bc = fcase.u_bc.clone();
            uf.correct_bc(&g);
            let phi = fv::flux(&g, &uf);
            let alpha_s = Scalar {
                c: alpha_structure.iter().map(|s| 1.0 - s).collect(),
                b: (ni..g.n_faces()).map(|f| 1.0 - alpha_structure[mesh.owner[f]]).collect(),
                bc: vec![Bc::Calculated; mesh.patches.len()],
            };
            let af = fv::interp_linear(&g, &alpha_s);
            let ap: Vec<f64> = (0..g.n_faces()).map(|f| af[f] * phi[f]).collect();
            let arp: Vec<f64> = ap.iter().map(|v| thermo.rho0 * v).collect();
            (arp, ap)
        } else {
            (surface("alphaRhoPhi")?, surface("alphaPhi")?)
        };

        Ok(Self {
            mesh,
            gravity,
            thermo,
            controls,
            start_time,
            end_time,
            delta_t,
            alpha_structure,
            dh,
            ia_act,
            t_fixed,
            htc_model,
            htc0,
            t,
            p,
            u,
            alphat,
            alpha_rho_phi,
            alpha_phi,
            power_density,
            min_re,
            flow,
        })
    }
}

/// The `onePhase` region, energy only, frozen flux.
#[derive(Debug, Clone)]
pub struct OnePhaseCcSolver {
    case: OnePhaseCcCase,
    geom: Geometry,
    /// `alpha = 1 - structure` (cells).
    alpha: Vec<f64>,
    /// Interpolated `alpha` on faces (constant).
    alpha_f: Vec<f64>,
    he: Vec<f64>,
    he_old: Vec<f64>,
    t: Vec<f64>,
    /// `Tact`: `T.fixedTemperature` on the power-model cells, 0 elsewhere.
    t_act: Vec<f64>,
    htc: Vec<f64>,
    re: Vec<f64>,
    cont_err: Vec<f64>,
    power_density: Vec<f64>,
    dt: f64,
    time: f64,
    /// `U` (cells and boundary; frozen in the energy-only region).
    u: Vector,
    /// `p` (frozen in the energy-only region).
    p: Vec<f64>,
    /// `p.oldTime()`: `None` until first requested (OpenFOAM stores old times lazily).
    p_old: Option<Vec<f64>>,
    /// Reproduce the lazy first-step old-time pressure of `fvc::ddt(p_)` (default true).
    lazy_p_old: bool,
    steps_begun: usize,
    alpha_rho_phi: Vec<f64>,
    alpha_phi: Vec<f64>,
    /// The solver's `phi_` (Courant number).
    phi_total: Vec<f64>,
    flow: Option<FlowState>,
}

/// What one PIMPLE outer iteration of the energy equation reported.
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseCcReport {
    /// Initial residual of the `h` solve of each outer iteration.
    pub h_residuals: Vec<f64>,
    /// `T (avg min max)` after each outer iteration: (volume-weighted mean, min, max).
    pub t_stats: Vec<(f64, f64, f64)>,
}

impl OnePhaseCcSolver {
    /// Build the region at the case's start state (`onePhase::onePhase` constructor: phase fractions,
    /// `contErr`, thermo `he`).
    #[must_use]
    pub fn new(case: OnePhaseCcCase) -> Self {
        let geom = Geometry::new(case.mesh.clone());
        let n = geom.n_cells();
        // fluid_ = 1.0 - structure_
        let alpha: Vec<f64> = case.alpha_structure.iter().map(|s| 1.0 - s).collect();
        let ni0 = geom.n_internal();
        let sa = Scalar {
            c: alpha.clone(),
            b: (ni0..geom.n_faces()).map(|f| alpha[geom.mesh.owner[f]]).collect(),
            bc: vec![Bc::Calculated; geom.mesh.patches.len()],
        };
        let alpha_f = fv::interp_linear(&geom, &sa);
        let he: Vec<f64> = case.t.iter().map(|&t| case.thermo.hs(t)).collect();
        // `correctContErr`: ddt(alpha, rho0) is exactly 0 at construction (old time = current).
        let dv = fv::div(&geom, &case.alpha_rho_phi);
        let cont_err: Vec<f64> = (0..n).map(|c| 0.0 + dv[c]).collect();
        let mut u = Vector::uniform(&geom, Vector3::ZERO);
        u.c = case.u.clone();
        if let Some(fc) = case.flow.as_ref() {
            u.bc = fc.u_bc.clone();
        }
        u.correct_bc(&geom);
        let flow = case.flow.as_ref().map(|fc| flow::FlowState::new(&geom, &case, fc, &u));
        let (case_p, case_arp, case_ap) = (case.p.clone(), case.alpha_rho_phi.clone(), case.alpha_phi.clone());
        let t_act: Vec<f64> = (0..n).map(|c| if case.ia_act[c] != 0.0 { case.t_fixed[c] } else { 0.0 }).collect();
        let htc = case.htc0.clone();
        let re = vec![case.min_re; n];
        let (t, power_density, dt, time) = (case.t.clone(), case.power_density.clone(), case.delta_t, case.start_time);
        let mut s = Self {
            case,
            geom,
            alpha,
            alpha_f,
            he_old: he.clone(),
            he,
            t,
            t_act,
            htc,
            re,
            cont_err,
            power_density,
            dt,
            time,
            u,
            p: case_p,
            p_old: None,
            lazy_p_old: true,
            steps_begun: 0,
            alpha_rho_phi: case_arp,
            phi_total: case_ap.clone(),
            alpha_phi: case_ap,
            flow,
        };
        s.update_re();
        s
    }

    /// Read a case directory and build the solver.
    ///
    /// # Errors
    /// See [`OnePhaseCcCase::read`].
    #[cfg(feature = "std")]
    pub fn from_case_dir(root: &Path, poly_mesh_dir: &Path) -> Result<Self, AppBuilderError> {
        Ok(Self::new(OnePhaseCcCase::read(root, poly_mesh_dir)?))
    }

    /// The case as read.
    #[must_use]
    pub fn case(&self) -> &OnePhaseCcCase {
        &self.case
    }

    /// Temperature per cell [K].
    #[must_use]
    pub fn temperature(&self) -> &[f64] {
        &self.t
    }

    /// Sensible enthalpy per cell [J/kg].
    #[must_use]
    pub fn enthalpy(&self) -> &[f64] {
        &self.he
    }

    /// The thermo's own density `rho(T)` per cell (what the coupling maps as `thermo:rho`).
    #[must_use]
    pub fn thermo_density(&self) -> Vec<f64> {
        self.t.iter().map(|&t| self.case.thermo.rho(t)).collect()
    }

    /// `Re` per cell (`FSPair::correct`).
    #[must_use]
    pub fn reynolds(&self) -> &[f64] {
        &self.re
    }

    /// `htc` per cell.
    #[must_use]
    pub fn htc(&self) -> &[f64] {
        &self.htc
    }

    /// Replace `powerDensityNeutronicsToLiquid` (what `meshHandler::mapTheseFields` writes).
    pub fn set_power_density(&mut self, pd: &[f64]) {
        self.power_density.clear();
        self.power_density.extend_from_slice(pd);
    }

    /// Time of the last completed step start (`runTime.value()`).
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// `runTime++` with step `dt`: stores the old-time enthalpy (OpenFOAM stores it at the first
    /// modification in a time step; coupled iterations within the step reuse it).
    pub fn begin_time_step(&mut self, dt: f64) {
        self.dt = dt;
        self.time += dt;
        self.he_old = self.he.clone();
        let uc = self.u.c.clone();
        if let Some(fs) = self.flow.as_mut() {
            fs.u_old = uc;
            fs.solves.clear();
        }
        // `p.oldTime()` is stored at the first non-const access of a new time step; in the FIRST step OpenFOAM has no
        // old-time copy yet and `fvc::ddt(p_)` (the energy equation) is the first request (round 3's quirk).
        self.p_old = if self.steps_begun == 0 && self.lazy_p_old { None } else { Some(self.p.clone()) };
        self.steps_begun += 1;
    }

    /// DEPRECATED (gh:#1837): the old-time pressure of step 1 is the initial pressure, not the pressure after the first
    /// `p_rgh` solve that upstream's lazy `oldTime()` gives. The ablation of the default.
    #[must_use]
    pub fn without_first_step_lazy_old_pressure(mut self) -> Self {
        self.lazy_p_old = false;
        self
    }

    /// `U` per cell.
    #[must_use]
    pub fn velocity(&self) -> &[Vector3] {
        &self.u.c
    }

    /// `p` per cell [Pa].
    #[must_use]
    pub fn pressure(&self) -> &[f64] {
        &self.p
    }

    /// `p_rgh` per cell [Pa] (`None` for the frozen-flux region).
    #[must_use]
    pub fn p_rgh(&self) -> Option<&[f64]> {
        self.flow.as_ref().map(|f| f.p_rgh.c.as_slice())
    }

    /// `alphaPhi` on every face, as last set (at the start of the last `pEqn`).
    #[must_use]
    pub fn alpha_phi(&self) -> &[f64] {
        &self.alpha_phi
    }

    /// The pressure solves of the last time step (empty without flow).
    #[must_use]
    pub fn pressure_solves(&self) -> &[PressureSolve] {
        self.flow.as_ref().map_or(&[], |f| f.solves.as_slice())
    }

    /// `FSPair::correct` quantities that do not need the fluid dynamics: `Re` (`nu = mu/rho(T)`).
    fn update_re(&mut self) {
        let th = &self.case.thermo;
        for c in 0..self.geom.n_cells() {
            let nu = th.mu / th.rho(self.t[c]);
            // normalized = alpha / (1 - structure)
            let norm = self.alpha[c] / (1.0 - self.case.alpha_structure[c]);
            let re = norm * self.u.c[c].mag() * self.case.dh[c] / nu;
            self.re[c] = re.max(self.case.min_re);
        }
    }

    /// `correctModels(true, true)`: `Re`, then `htc_[cell] = value` on the model cells.
    fn correct_models(&mut self) {
        self.update_re();
        self.correct_kd();
        for c in 0..self.geom.n_cells() {
            if let Some(m) = self.case.htc_model[c] {
                self.htc[c] = m.value();
            }
        }
    }

    /// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`), see `one_phase_fc::relax_diag`: boundary
    /// coefficients are folded into the diagonal; a boundary face with a negative convective internal
    /// coefficient is not neutral. Returns `D' - D` per cell.
    fn relax_diag(&self, e: &mut Eqn, factor: f64, inflow2: &[f64]) -> Vec<f64> {
        let n = self.geom.n_cells();
        let mut sum_off = vec![0.0; n];
        for f in 0..self.geom.n_internal() {
            sum_off[e.ldu.owner[f]] += e.ldu.upper[f].abs();
            sum_off[e.ldu.neighbour[f]] += e.ldu.lower[f].abs();
        }
        let mut delta = vec![0.0; n];
        for c in 0..n {
            let d0 = e.ldu.diag[c];
            let d = (d0 + inflow2[c]).abs().max(sum_off[c]) / factor;
            e.ldu.diag[c] = d;
            delta[c] = d - d0;
        }
        delta
    }

    /// `EEqn_1p.H`; returns the initial residual of the `h` solve and the `T (avg min max)` it prints.
    fn correct_energy(&mut self) -> (f64, (f64, f64, f64)) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let rdt = 1.0 / self.dt;
        let th = self.case.thermo.clone();
        let rho0 = th.rho0;
        let cp = th.cp;
        let phi = &self.alpha_rho_phi;
        let he_scalar = Scalar {
            c: self.he.clone(),
            b: vec![0.0; g.n_faces() - ni],
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        // fvm::ddt(alpha, rho0, he)
        let mut e = Eqn::new(&g);
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.ldu.diag[c] += rdt * self.alpha[c] * rho0 * v;
            e.source[c] += rdt * self.alpha[c] * rho0 * self.he_old[c] * v;
        }
        e.div_upwind(&g, phi, &he_scalar);
        let ce: Vec<f64> = self.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &self.he);
        // - laplacian(interpolate(alpha) interpolate(alphaEff), he), alphaEff = alpha_thermo + alphat
        let ah = th.alpha_h();
        let ae = Scalar {
            c: (0..n).map(|c| ah + self.case.alphat[c]).collect(),
            b: vec![0.0; g.n_faces() - ni],
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        let ae_f = fv::interp_linear(&g, &ae);
        let gam: Vec<f64> = (0..g.n_faces()).map(|f| self.alpha_f[f] * ae_f[f]).collect();
        e.neg_laplacian(&g, &gam, &he_scalar);
        // right-hand side: alpha rho0 (U.g); alpha ddt(p) = 0 exactly (p is never modified)
        let mut rhs = vec![0.0; n];
        for c in 0..n {
            rhs[c] = self.alpha[c] * rho0 * self.u.c[c].dot(self.case.gravity);
        }
        e.su(&g, &rhs);
        // `EEqn -= alpha*fvc::ddt(p_)`: `p.oldTime()` is created by this very call in step 1 (see `begin_time_step`)
        {
            let p_now = &self.p;
            let p_old = self.p_old.get_or_insert_with(|| p_now.clone());
            let dp: Vec<f64> = (0..n).map(|c| self.alpha[c] * (rdt * (p_now[c] - p_old[c]))).collect();
            e.su(&g, &dp);
        }
        if self.case.controls.include_kinetic_energy {
            let mut k = Scalar {
                c: vec![0.0; n],
                b: vec![0.0; g.n_faces() - ni],
                bc: vec![Bc::Calculated; g.mesh.patches.len()],
            };
            for c in 0..n {
                let u = self.u.c[c] * 0.5;
                k.c[c] = u.dot(self.u.c[c]);
            }
            // wedge/wall boundary values of K: the cell value (a wedge patch value is the cell value
            // rotated; the rotation preserves |U|)
            for f in ni..g.n_faces() {
                k.b[f - ni] = if self.flow.is_some() {
                    let ub = self.u.b[f - ni];
                    (ub * 0.5).dot(ub)
                } else {
                    k.c[g.mesh.owner[f]]
                };
            }
            let kf = fv::interp_upwind(&g, &k, phi);
            let flux: Vec<f64> = (0..g.n_faces()).map(|f| phi[f] * kf[f]).collect();
            let div_k = fv::div(&g, &flux);
            let mut s = vec![0.0; n];
            for c in 0..n {
                // K.oldTime() of a fresh local field is K itself; rho = rho0 for both times
                let ddt_k = rdt * (self.alpha[c] * rho0 * k.c[c] - self.alpha[c] * rho0 * k.c[c]);
                s[c] = -(ddt_k + div_k[c] - self.cont_err[c] * k.c[c]);
            }
            e.su(&g, &s);
        }
        // structure_.linearizedSemiImplicitHeatSource(fluid, Hs)
        let mut su = vec![0.0; n];
        let mut sp = vec![0.0; n];
        for c in 0..n {
            let ih = self.case.ia_act[c] * self.htc[c];
            su[c] = ih * (self.t_act[c] - self.t[c] + self.he[c] / cp);
            sp[c] = ih / cp;
        }
        e.su(&g, &su);
        e.sp(&g, &sp);
        // - alpha * powerDensity
        let pd: Vec<f64> = (0..n).map(|c| self.alpha[c] * self.power_density[c]).collect();
        e.su(&g, &pd);
        // EEqn.relax()
        if let Some(fct) = self.case.controls.relax_h_equation {
            let mut inflow2 = vec![0.0; n];
            for f in ni..g.n_faces() {
                if g.active[f] {
                    inflow2[g.mesh.owner[f]] += 2.0 * (-phi[f]).max(0.0);
                }
            }
            let delta = self.relax_diag(&mut e, fct, &inflow2);
            for c in 0..n {
                e.source[c] += delta[c] * self.he[c];
            }
        }
        let mut x = self.he.clone();
        let perf = pbicgstab_diagonal(
            &e.ldu,
            &e.source,
            &mut x,
            self.case.controls.h_tolerance,
            self.case.controls.h_rel_tol,
            DEFAULT_MAX_ITER,
            self.case.controls.h_min_iter,
        );
        self.he = x;
        // thermo.correct(): T from he by Newton, seeded with the current T
        for c in 0..n {
            self.t[c] = th.t_from_hs(self.he[c], self.t[c]);
        }
        let vols = &g.mesh.cell_volumes;
        let (mut sv, mut st) = (0.0, 0.0);
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for c in 0..n {
            sv += vols[c];
            st += vols[c] * self.t[c];
            tmin = tmin.min(self.t[c]);
            tmax = tmax.max(self.t[c]);
        }
        (perf.initial_residual, (st / sv, tmin, tmax))
    }

    /// `onePhase::correctPhysics` with `solveEnergy true`, `solveFluidMechanics false`: `nOuterCorrectors`
    /// outer iterations, each `correctModels` then `correctEnergy`. Returns the residual
    /// (`max` of the initial residuals) and the per-iteration report.
    pub fn correct_physics(&mut self) -> (f64, OnePhaseCcReport) {
        let mut residual = 0.0_f64;
        let mut rep = OnePhaseCcReport { h_residuals: Vec::new(), t_stats: Vec::new() };
        let n_outer = self.case.controls.n_outer;
        for outer in 1..=n_outer {
            self.correct_models();
            if self.flow.is_some() {
                self.correct_fluid_mechanics(outer, outer == n_outer);
                if let Some(fs) = self.flow.as_ref() {
                    self.phi_total = fs.phi_solver.clone();
                }
            }
            let (r, st) = self.correct_energy();
            residual = residual.max(r);
            rep.h_residuals.push(r);
            rep.t_stats.push(st);
        }
        (residual, rep)
    }

    /// `onePhase::correctTightlyCoupledPhysics`: one `correctModels` and one `correctEnergy` (no PIMPLE loop).
    pub fn correct_tightly_coupled_physics(&mut self) -> f64 {
        self.correct_models();
        let (r, _) = self.correct_energy();
        r
    }

    /// `onePhase::correctCourant`: `(CoNum_, meanCoNum_)` = (max, mean) with the current `dt`.
    #[must_use]
    pub fn courant(&self) -> (f64, f64) {
        let g = &self.geom;
        let sum = fv::surface_sum_mag(g, &self.phi_total);
        let n = g.n_cells();
        let vols = &g.mesh.cell_volumes;
        let mut maxv = f64::MIN;
        let mut gsum = 0.0;
        let mut gvol = 0.0;
        for c in 0..n {
            let sp = sum[c] / self.alpha[c];
            maxv = maxv.max(sp / vols[c]);
            gsum += sp;
            gvol += vols[c];
        }
        (0.5 * maxv * self.dt, 0.5 * (gsum / gvol) * self.dt)
    }

    /// `onePhase::maxDeltaT`.
    #[must_use]
    pub fn max_delta_t(&self) -> f64 {
        let (co, _) = self.courant();
        let new_dt = self.case.controls.max_delta_t;
        let max_fact = self.case.controls.max_co / (co + SMALL);
        let fact = (max_fact.min(1.0 + 0.1 * max_fact)).min(1.2);
        (fact * self.dt).min(new_dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newton_recovers_temperature_of_a_linear_enthalpy() {
        let th = BoussinesqHConst { cp: 1600.0, tref: TSTD, hsref: 0.0, rho0: 4125.0, t0: 900.0, beta: 2e-4, mu: 0.01, r_pr: 0.125 };
        let t = 1036.2258577635569;
        let back = th.t_from_hs(th.hs(t), 1000.0);
        assert!((back - t).abs() / t < 1e-14);
        assert_eq!(th.rho(900.0), 4125.0);
        assert_eq!(th.alpha_h(), 0.01 * 0.125);
    }

    #[test]
    fn surface_field_text_with_wedge_values_parses() {
        let text = "FoamFile{}\ninternalField nonuniform List<scalar> 2\n(\n1.5\n-2\n)\n;\nboundaryField\n{\n    front\n    {\n        type wedge;\n        value nonuniform List<scalar> 2\n(\n1e-11\n2e-11\n)\n;\n    }\n    wall\n    {\n        type calculated;\n        value uniform 0;\n    }\n}\n";
        let patches = vec![("front".to_string(), 2), ("wall".to_string(), 3)];
        let (i, b) = parse_scalar_field_text(text, 2, &patches).expect("parse");
        assert_eq!(i, vec![1.5, -2.0]);
        assert_eq!(b[0], vec![1e-11, 2e-11]);
        assert_eq!(b[1], vec![0.0; 3]);
    }
}
