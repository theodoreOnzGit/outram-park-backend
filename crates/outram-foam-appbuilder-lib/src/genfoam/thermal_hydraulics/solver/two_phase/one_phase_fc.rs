// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/onePhase/**
//     (onePhase.{C,H}; include/equations/{UEqn_1p,EEqn_1p,pEqnf_1p,pEqn_1p}.H;
//      include/equations/pEqnRoutines/*.H)
//   and the classes those files call: physicsModels/phasePairs/FSPair.C,
//   physicsModels/dragModels/FSDragFactor.C,
//   phaseModels/structureModels/structure.C ({correct, linearizedSemiImplicitHeatSource}),
//   phaseModels/structureModels/powerModels/fixedPower/fixedPower.C,
//   solvers/thermalHydraulicsModel.C, src/customPimpleControl/customPimpleControl.C,
//   and OpenFOAM-ESI v2506 fvMatrix::relax(alpha), Time::adjustDeltaT.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `two_phase::one_phase_fc` — GeN-Foam's `onePhase` solver, `faceCentered` momentum
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! A translation of GeN-Foam's `solvers/onePhase` (one fluid in a porous structure) in
//! `momentumMode faceCentered`, built on the finite-volume machinery and the `heRhoThermo`
//! packages of the [`super`] two-phase port (gh:#1361 round 4). It is **not** the older
//! [`crate::genfoam::thermal_hydraulics::solver::one_phase::OnePhaseSolver`] (constant properties,
//! no thermo package), which is left exactly as it was.
//!
//! The equations, in upstream's order within one PIMPLE outer iteration:
//!
//! 1. `FSPair::correct`: `Re`, the fluid-structure drag `Kd` and heat-transfer coefficient `Hs`.
//! 2. `UEqn_1p.H`: `div(alphaRhoPhi, U) + SuSp(-contErr, U) - laplacian(alpha rho nuEff, U)
//!    - div(alpha rho nuEff dev2(T(grad U))) + Sp(tr(Kd)/3, U)` (no `ddt` in the face-centred mode),
//!    then `UEqn.relax()`.
//! 3. `pEqnf_1p.H`: `rAUf = 1/(interpolate(A) + interpolate(alpha rho)/dt)`, `phiHbyA` with the old
//!    flux, the compressible `p_rgh` equation (`pEqnComp`), flux, `U` by reconstruction, and
//!    `updatePP_rghRho_1p.H`.
//! 4. `EEqn_1p.H`: `ddt(alpha rho he) + div(alphaRhoPhi he) + SuSp(-contErr, he) - laplacian(...)`
//!    `== alpha rho (U.g) + alpha ddt(p) - [ddt(alpha rho K) + div(alphaRhoPhi K) - contErr K]`
//!    `+ structure heat source`, `EEqn.relax()`, solve, `thermo.correct()`, `structure.correct`.
//!
//! ## Named options (the older forms stay callable)
//!
//! * [`OnePhaseFcSolver::without_first_step_lazy_old_time`]: the first step takes `p.oldTime()` as the
//!   initial field and the structure's old temperature as its start-of-step value (what the port
//!   did before round 3; upstream creates both lazily, see [`OnePhaseFcControls::lazy_old_time_first_step`]).
//! * [`OnePhaseFcSolver::without_equation_relaxation`]: skip `fvMatrix::relax(1)`'s
//!   diagonal-dominance step on `UEqn` and `EEqn`.
//! * [`OnePhaseFcSolver::without_kinetic_energy`]: `includeKineticEnergy false`.
//! * [`OnePhaseFcSolver::with_structure_after_solve_temperature`]: `structure.correct` with the
//!   temperature after the energy solve (what the twoPhase port does), not `T.prevIter()`.
//!
//! ## What this does not port (each refused by name in [`OnePhaseFcCase::read`])
//!
//! `momentumMode` other than `faceCentered`; `porousInterfaceSharpness != 0`; turbulence other than
//! `laminar`; heat exchangers, pumps, passive structures, tortuosity, local frames, power-off
//! criteria; velocity patches other than `zeroGradient`; closed volumes (no fixed-value `p_rgh`
//! patch: the reference-cell branch); `Boussinesq`.

use std::path::Path;
use std::sync::Arc;

use outram_foam_basic_lib::io::FoamEntry;
use outram_foam_basic_lib::prelude::*;
use uom::si::f64::{Length, Ratio, ThermalConductivity, Time};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::case::{
    bool_entry, lookup_regex, read_field, read_nusselt, read_thermo, scalar, scalar_bcs, word, Ctx,
};
use super::fv::{self, Bc, Eqn, Geometry, Scalar, Vector};
use super::{
    dev2_div, solve_krylov, temperature, Phase, Structure, TimeControls, VecEqn,
};
use crate::error::AppBuilderError;
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::FsForcedConvectionHtc;
use crate::genfoam::thermal_hydraulics::structure::power_model::FixedPower;
use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh};

/// PIMPLE and solver controls of the `onePhase` region (`fvSolution`, `controlDict`).
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseFcControls {
    /// `nOuterCorrectors`.
    pub n_outer: usize,
    /// `nCorrectors` (PISO correctors per outer iteration).
    pub n_corr: usize,
    /// `correctUntilConvergence` (`customPimpleControl`).
    pub correct_until_convergence: bool,
    /// `solvers.p_rgh.tolerance`.
    pub p_rgh_tolerance: f64,
    /// `solvers."h.*".tolerance`.
    pub h_tolerance: f64,
    /// `solvers."h.*".minIter`.
    pub h_min_iter: usize,
    /// `residualControl.p_rgh.tolerance` (with `useFirstPISOInitialResidual`).
    pub residual_control_p_rgh: Option<f64>,
    /// `residualControl.h.tolerance`.
    pub residual_control_h: Option<f64>,
    /// `pMin` [Pa].
    pub p_min: f64,
    /// `includeKineticEnergy` (`controlDict`, default true).
    pub include_kinetic_energy: bool,
    /// Whether the thermo package carries `dpdt` (OpenFOAM default true).
    pub dpdt: bool,
    /// `minMagU` of `FSDragFactor` (`PIMPLE`, default 0).
    pub min_mag_u: f64,
    /// Equation relaxation factor of `U` (`relaxationFactors.equations`), `None` if absent.
    pub relax_u_equation: Option<f64>,
    /// Equation relaxation factor of `h`, `None` if absent.
    pub relax_h_equation: Option<f64>,
    /// Reproduce what GeN-Foam 652b3da does in the FIRST time step because OpenFOAM creates a
    /// field's old-time copy lazily (gh:#1361 round 3, found for twoPhase; the same two requests
    /// occur in `EEqn_1p.H` and `fixedPower::correct`): `fvc::ddt(p_)` is the first request for
    /// `p.oldTime()`, and `fixedPower::correct` writes `T_[celli]` through `operator[]` (no
    /// `storeOldTimes`). Default `true`.
    pub lazy_old_time_first_step: bool,
    /// `true` (default): `structure_.correct(Hs*T.prevIter(), Hs)`, the temperature before the
    /// energy solve. `false`: the temperature after it.
    pub structure_uses_prev_iter_t: bool,
    /// Reproduce `customPimpleControl::loop` as GeN-Foam 652b3da has it (gh:#1361 round 4, default
    /// `true`). When the residual criteria are first met in the LAST outer iteration
    /// (`corr == nOuterCorrectors`), `loop()` flags `converged_`, runs that iteration, and on the next
    /// call returns through the `corr == nCorr + 1` branch, which does not clear `converged_`. The
    /// next time step's first `loop()` call therefore sees `converged_` still true, prints
    /// "converged in 0 iterations", clears it and returns false: **that whole time step solves
    /// nothing** (the fields stay as they were, time advances). Measured in `onePhase` on the
    /// channel: every second step from step 71. `false`: the loop ends cleanly, as the code reads
    /// at a glance ([`OnePhaseFcSolver::without_stale_pimple_converged_flag`]).
    pub stale_pimple_converged_flag: bool,
}

/// A read single-phase case.
#[derive(Debug, Clone)]
pub struct OnePhaseFcCase {
    /// Mesh.
    pub mesh: Arc<FvMesh>,
    /// Gravity [m/s^2].
    pub gravity: Vector3,
    /// `p`.
    pub p: Scalar,
    /// `p_rgh`.
    pub p_rgh: Scalar,
    /// The fluid.
    pub fluid: Phase,
    /// The structure.
    pub structure: Structure,
    /// Fluid-structure friction model per cell.
    pub fs_drag: Vec<Option<FsWallFriction>>,
    /// Fluid-structure forced-convection htc model per cell.
    pub fs_htc: Vec<Option<FsForcedConvectionHtc>>,
    /// Controls.
    pub controls: OnePhaseFcControls,
    /// Time controls.
    pub time_controls: TimeControls,
    /// `startTime`.
    pub start_time: f64,
    /// `endTime`.
    pub end_time: f64,
    /// `deltaT`.
    pub delta_t: f64,
}

fn parse_zone_models<T>(
    ctx: &Ctx,
    d: &outram_foam_basic_lib::io::FoamDict,
    zone_cells: &dyn Fn(&str) -> Result<Vec<usize>, AppBuilderError>,
    n: usize,
    build: &dyn Fn(&outram_foam_basic_lib::io::FoamDict) -> Result<T, AppBuilderError>,
) -> Result<Vec<Option<T>>, AppBuilderError>
where
    T: Clone,
{
    let mut out: Vec<Option<T>> = vec![None; n];
    for (zk, e) in d.iter() {
        let FoamEntry::SubDict(md) = e else { continue };
        let model = build(md)?;
        for c in zone_cells(zk)? {
            out[c] = Some(model.clone());
        }
    }
    let _ = ctx;
    Ok(out)
}

impl OnePhaseFcCase {
    /// Read a GeN-Foam single-phase case directory (`fluidRegion`, `onePhase`).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] for a missing or unreadable file, and for every selection the
    /// port does not implement (named in the message; extend the port, not the refusal).
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path) -> Result<Self, AppBuilderError> {
        let ctx = Ctx { root: root.to_path_buf() };
        let region = "fluidRegion";
        let poly = root.join(format!("constant/{region}/polyMesh"));
        let mesh = read_poly_mesh(&poly)?;
        let zones = read_cell_zones(&poly)?;
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
        let write_interval = match word(&cd, "writeControl").as_deref() {
            Some("adjustableRunTime") => scalar(&cd, "writeInterval"),
            _ => None,
        };
        let time_controls = TimeControls {
            adjust_time_step: bool_entry(&cd, "adjustTimeStep").unwrap_or(false),
            max_co: scalar(&cd, "maxCo").unwrap_or(1.0),
            max_co_two_phase: None,
            margin_to_phase_change: None,
            max_delta_t: scalar(&cd, "maxDeltaT").unwrap_or(f64::MAX),
            write_interval,
            start_time,
            esi_adjust_delta_t: true,
        };
        match cd.get_dict("regionSolvers") {
            Some(rs) if word(rs, region).as_deref() == Some("onePhase") => {}
            _ => return Err(ctx.e("regionSolvers.fluidRegion is not onePhase")),
        }
        let include_kinetic_energy = bool_entry(&cd, "includeKineticEnergy").unwrap_or(true);

        let pp = ctx.read(&format!("constant/{region}/phaseProperties"))?;
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_s = vec![0.0; n];
        let mut dh_s = vec![1.0; n];
        let mut power: Vec<Option<FixedPower>> = vec![None; n];
        for (key, e) in sp.iter() {
            let FoamEntry::SubDict(zd) = e else { continue };
            if key == "powerOffCriterionModel" || key == "heatExchangers" {
                return Err(ctx.e(format!("structureProperties.{key} is not ported")));
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
                    power[c] = Some(FixedPower::new(
                        Ratio::new::<ratio>(pa),
                        uom::si::f64::ReciprocalLength::new::<reciprocal_meter>(ia),
                        uom::si::f64::VolumetricPowerDensity::new::<watt_per_cubic_meter>(q),
                        uom::si::f64::VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(
                            (pa * rho_cp).max(1e-69),
                        ),
                        uom::si::f64::ThermodynamicTemperature::new::<kelvin>(t0),
                    ));
                }
            }
        }
        let structure = Structure::new(&g, alpha_s, dh_s, power, None);

        let pm = ctx.sub(&pp, "physicsModels")?;
        let drag = ctx.sub(pm, "dragModels")?;
        let fs_drag = parse_zone_models(&ctx, drag, &zone_cells, n, &|md| {
            match ctx.word(md, "type")?.as_str() {
                "ReynoldsPower" => Ok(FsWallFriction::ReynoldsPower {
                    coeff: ctx.f64(md, "coeff")?,
                    exp: ctx.f64(md, "exp")?,
                    constant: scalar(md, "const").unwrap_or(0.0),
                }),
                other => Err(ctx.e(format!("FS drag `{other}` not wired"))),
            }
        })?;
        let htm = ctx.sub(pm, "heatTransferModels")?;
        let fs_htc = parse_zone_models(&ctx, htm, &zone_cells, n, &|md| read_nusselt(&ctx, md))?;

        let fs = ctx.read(&format!("system/{region}/fvSolution"))?;
        let pimple = ctx.sub(&fs, "PIMPLE")?;
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
        let (h_tol, h_min) = match lookup_regex(solvers, "h") {
            Some(FoamEntry::SubDict(d)) => (ctx.f64(d, "tolerance")?, scalar(d, "minIter").unwrap_or(0.0) as usize),
            _ => return Err(ctx.e("no h.* solver")),
        };
        let rc = pimple.get_dict("residualControl");
        let rc_tol = |name: &str| rc.and_then(|r| r.get_dict(name)).and_then(|d| scalar(d, "tolerance"));
        let eq_relax = |name: &str| -> Option<f64> {
            fs.get_dict("relaxationFactors")
                .and_then(|r| r.get_dict("equations"))
                .and_then(|r| lookup_regex(r, name))
                .and_then(|e| match e {
                    FoamEntry::Scalar(x) => Some(*x),
                    _ => None,
                })
        };
        let controls = OnePhaseFcControls {
            n_outer: scalar(pimple, "nOuterCorrectors").unwrap_or(1.0) as usize,
            n_corr: scalar(pimple, "nCorrectors").unwrap_or(1.0) as usize,
            correct_until_convergence: bool_entry(pimple, "correctUntilConvergence").unwrap_or(false),
            p_rgh_tolerance: p_rgh_tol,
            h_tolerance: h_tol,
            h_min_iter: h_min,
            residual_control_p_rgh: rc_tol("p_rgh"),
            residual_control_h: rc_tol("h"),
            p_min: scalar(&pp, "pMin").unwrap_or(0.0),
            include_kinetic_energy,
            dpdt: true,
            min_mag_u: scalar(pimple, "minMagU").unwrap_or(0.0),
            relax_u_equation: eq_relax("U"),
            relax_h_equation: eq_relax("h"),
            lazy_old_time_first_step: true,
            structure_uses_prev_iter_t: true,
            stale_pimple_converged_flag: true,
        };

        let zero = format!("0/{region}");
        let p_rgh_f = read_field(&ctx, &format!("{zero}/p_rgh"))?;
        let p_rgh_bc = scalar_bcs(&ctx, &g, &p_rgh_f)?;
        if !p_rgh_bc.iter().any(|b| matches!(b, Bc::Fixed(_))) {
            return Err(ctx.e("a closed volume (no fixedValue p_rgh patch, pressure reference cell) is not ported"));
        }
        let p_rgh = Scalar::uniform(&g, p_rgh_f.internal.as_scalar().unwrap_or(0.0), p_rgh_bc);
        // p is READ_IF_PRESENT with `calculated` patches carrying the file's own values.
        let p_f = read_field(&ctx, &format!("{zero}/p"))?;
        let p = Scalar::uniform(&g, p_f.internal.as_scalar().unwrap_or(0.0), vec![Bc::Calculated; mesh.patches.len()]);

        let thermo = read_thermo(&ctx, &format!("constant/{region}/thermophysicalProperties"))?;
        let tp = ctx.read(&format!("constant/{region}/turbulenceProperties"))?;
        if word(&tp, "simulationType").as_deref() != Some("laminar") {
            return Err(ctx.e("only laminar turbulence is ported"));
        }
        let t_f = read_field(&ctx, &format!("{zero}/T"))?;
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
        let u_f = read_field(&ctx, &format!("{zero}/U"))?;
        let u0 = u_f
            .internal
            .as_vector3()
            .ok_or_else(|| ctx.e("U must be uniform"))?;
        for (pn, ty, _) in &u_f.bcs {
            if ty != "zeroGradient" && ty != "empty" {
                return Err(ctx.e(format!("U.{pn}: only zeroGradient velocity patches are ported")));
            }
        }
        let mut alpha = Scalar::uniform(&g, 1.0, g.default_bcs());
        for c in 0..n {
            alpha.c[c] = 1.0 - structure.alpha.c[c];
        }
        alpha.correct_bc(&g);
        for i in 0..alpha.b.len() {
            alpha.b[i] = 1.0 - structure.alpha.b[i];
        }
        let fluid = Phase::new(&g, "", true, thermo, alpha, Vector::uniform(&g, u0), t, he_bc, 1e-6, 0.0, 0.0);
        Ok(Self {
            mesh,
            gravity,
            p,
            p_rgh,
            fluid,
            structure,
            fs_drag,
            fs_htc,
            controls,
            time_controls,
            start_time,
            end_time,
            delta_t,
        })
    }
}

/// Run-time diagnostics of one time step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OnePhaseStepReport {
    /// Time at the end of the step [s].
    pub time: f64,
    /// Step size [s].
    pub dt: f64,
    /// PIMPLE outer iterations performed.
    pub outer_iterations: usize,
    /// Initial residual of the last outer iteration's first `p_rgh` solve.
    pub p_rgh_initial_residual: f64,
    /// Initial residual of the last `h` solve.
    pub h_initial_residual: f64,
    /// Courant number (`correctCourant`, "avg max" is the second figure upstream prints).
    pub courant: f64,
}

/// The `onePhase` solver, `faceCentered` momentum. Build with [`OnePhaseFcCase::read`] and
/// [`OnePhaseFcSolver::new`], advance with [`OnePhaseFcSolver::step`].
#[derive(Debug, Clone)]
pub struct OnePhaseFcSolver {
    /// Mesh geometry.
    pub geom: Geometry,
    /// Gravity [m/s^2].
    pub g: Vector3,
    gh: Scalar,
    ghf: Vec<f64>,
    /// Static pressure [Pa].
    pub p: Scalar,
    /// `p_rgh` [Pa].
    pub p_rgh: Scalar,
    p_old: Vec<f64>,
    p_old_stored: bool,
    structure_old_time_stale: bool,
    /// The fluid (`alpha` is `1 - alpha_structure`).
    pub fluid: Phase,
    /// The structure.
    pub structure: Structure,
    fs_drag: Vec<Option<FsWallFriction>>,
    fs_htc: Vec<Option<FsForcedConvectionHtc>>,
    /// Controls.
    pub controls: OnePhaseFcControls,
    /// Time controls.
    pub time_controls: TimeControls,
    /// Fluid-structure drag coefficient `Kd` (isotropic) [kg/(m3 s)].
    pub kd: Vec<f64>,
    /// Fluid-structure heat-transfer coefficient `Hs` [W/(m2 K)].
    pub htc: Vec<f64>,
    /// Fluid-structure Reynolds number.
    pub re: Vec<f64>,
    /// Boundary values of `psi`.
    psi_b: Vec<f64>,
    /// `phi_` of `thermalHydraulicsModel`: the REAL (alpha-weighted) volumetric flux [m3/s].
    pub phi_real: Vec<f64>,
    /// Current time [s].
    pub time: f64,
    /// Current step size [s].
    pub dt: f64,
    write_time_index: i64,
    end_time: f64,
    /// `customPimpleControl::converged_`, kept across time steps (see
    /// [`OnePhaseFcControls::stale_pimple_converged_flag`]).
    pimple_converged: bool,
}

impl OnePhaseFcSolver {
    /// Build the solver: `onePhase::onePhase` (fluid fractions from the structure, `initAlphaPhis`,
    /// the first `correctContErr`).
    #[must_use]
    pub fn new(case: OnePhaseFcCase) -> Self {
        let geom = Geometry::new(case.mesh.clone());
        let n = geom.n_cells();
        let nf = geom.n_faces();
        let ni = geom.n_internal();
        let m = geom.mesh.clone();
        let g = case.gravity;
        let mut gh = Scalar::uniform(&geom, 0.0, vec![Bc::Calculated; m.patches.len()]);
        for c in 0..n {
            gh.c[c] = g.dot(m.cell_centres[c]);
        }
        let mut ghf = vec![0.0; nf];
        for f in 0..nf {
            ghf[f] = g.dot(m.face_centres[f]);
            if f >= ni {
                gh.b[f - ni] = ghf[f];
            }
        }
        let mut s = Self {
            g,
            gh,
            ghf,
            p: case.p.clone(),
            p_rgh: case.p_rgh.clone(),
            p_old: case.p.c.clone(),
            p_old_stored: !case.controls.lazy_old_time_first_step,
            structure_old_time_stale: case.controls.lazy_old_time_first_step,
            fluid: case.fluid.clone(),
            structure: case.structure.clone(),
            fs_drag: case.fs_drag.clone(),
            fs_htc: case.fs_htc.clone(),
            controls: case.controls.clone(),
            time_controls: case.time_controls.clone(),
            kd: vec![0.0; n],
            htc: vec![0.0; n],
            re: vec![10.0; n],
            psi_b: vec![0.0; nf - ni],
            phi_real: vec![0.0; nf],
            time: case.start_time,
            dt: case.delta_t,
            write_time_index: 0,
            end_time: case.end_time,
            pimple_converged: false,
            geom,
        };
        for c in 0..n {
            s.fluid.normalized[c] = s.fluid.alpha.c[c] / (1.0 - s.structure.alpha.c[c]);
            s.fluid.dh[c] = s.structure.dh[c];
        }
        s.thermo_correct(true);
        let u = s.fluid.u.clone();
        s.fluid.phi = fv::flux(&s.geom, &u);
        s.fluid.mag_u = s.fluid.u.c.iter().map(|v| v.mag()).collect();
        // old-time copies are created by the first `oldTime()` request, in `correctContErr`
        s.fluid.alpha_old = s.fluid.alpha.c.clone();
        s.fluid.alpha_old_b = s.fluid.alpha.b.clone();
        s.fluid.rho_old = s.fluid.rho.c.clone();
        s.fluid.rho_old_b = s.fluid.rho.b.clone();
        s.fluid.he_old = s.fluid.he.c.clone();
        s.fluid.phi_old = s.fluid.phi.clone();
        s.fluid.u_old = s.fluid.u.c.clone();
        let af = fv::interp_linear(&s.geom, &s.fluid.alpha);
        s.update_alpha_phi(&af);
        s.phi_real = s.fluid.alpha_phi.clone();
        for c in 0..n {
            if let Some(pm) = &s.structure.power[c] {
                s.structure.t_act[c] = pm.surface_temperature().get::<kelvin>();
            }
        }
        s
    }

    /// Read a case directory and build the solver.
    ///
    /// # Errors
    /// As [`OnePhaseFcCase::read`].
    pub fn from_case_dir(root: &Path) -> Result<Self, AppBuilderError> {
        Ok(Self::new(OnePhaseFcCase::read(root)?))
    }

    /// `heThermo::correct` for the one fluid: `T` from `he`, then `rho`, `psi`, `Cp`, `mu`, `kappa`
    /// (`init`: `he` from `T`, the thermo constructor). Boundary values follow the twoPhase port.
    fn thermo_correct(&mut self, init: bool) {
        let g = &self.geom;
        let n = g.n_cells();
        let ni = g.n_internal();
        let p = &self.p;
        let fl = &mut self.fluid;
        for c in 0..n {
            if init {
                fl.he.c[c] = fl.thermo.he(p.c[c], fl.t.c[c]);
            } else if let Some(t) = fl.thermo.t_from_he(fl.he.c[c], p.c[c], fl.t.c[c]) {
                fl.t.c[c] = t;
            }
            let pp = fl.thermo.props(p.c[c], fl.t.c[c]);
            fl.rho.c[c] = pp.rho;
            fl.psi[c] = pp.psi;
            fl.cp.c[c] = pp.cp;
            fl.mu.c[c] = pp.mu;
            fl.kappa.c[c] = pp.kappa;
        }
        fl.t.correct_bc(g);
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let i = f - ni;
            let pb = p.b[i];
            fl.he.b[i] = fl.thermo.he(pb, fl.t.b[i]);
            if let Bc::Fixed(v) = &mut fl.he.bc[g.patch_of[i]] {
                let pat = &g.mesh.patches[g.patch_of[i]];
                v[f - pat.start] = fl.he.b[i];
            }
            let pp = fl.thermo.props(pb, fl.t.b[i]);
            fl.rho.b[i] = pp.rho;
            self.psi_b[i] = pp.psi;
            fl.cp.b[i] = pp.cp;
            fl.mu.b[i] = pp.mu;
            fl.kappa.b[i] = pp.kappa;
        }
    }

    /// `updateAlphaPhiAndInitControls_1p.H`: `alphaPhi`, `alphaRhoPhi`, then `correctContErr`.
    fn update_alpha_phi(&mut self, alpha_f: &[f64]) {
        let nf = self.geom.n_faces();
        let rf = fv::interp_linear(&self.geom, &self.fluid.rho);
        for f in 0..nf {
            self.fluid.alpha_phi[f] = alpha_f[f] * self.fluid.phi[f];
            self.fluid.alpha_rho_phi[f] = rf[f] * self.fluid.alpha_phi[f];
        }
        self.correct_cont_err();
    }

    /// `onePhase::correctContErr`: `ddt(alpha, rho) + div(alphaRhoPhi)`.
    fn correct_cont_err(&mut self) {
        let n = self.geom.n_cells();
        let rdt = 1.0 / self.dt;
        let dv = fv::div(&self.geom, &self.fluid.alpha_rho_phi);
        let fl = &mut self.fluid;
        for c in 0..n {
            fl.cont_err[c] =
                rdt * (fl.alpha.c[c] * fl.rho.c[c] - fl.alpha_old[c] * fl.rho_old[c]) + dv[c];
        }
    }

    /// `FSPair::correct(true, true)`: `Re`, `Kd` and `Hs`.
    fn correct_models(&mut self) {
        let n = self.geom.n_cells();
        for c in 0..n {
            let fl = &self.fluid;
            let nu = fl.mu.c[c] / fl.rho.c[c];
            self.re[c] = (fl.normalized[c] * fl.u.c[c].mag() * self.structure.dh[c] / nu).max(10.0);
        }
        for c in 0..n {
            if let Some(model) = self.fs_drag[c] {
                let fl = &self.fluid;
                let fd = model.friction_factor(ReynoldsNumber::new::<ratio>(self.re[c])).get::<ratio>();
                self.kd[c] = 0.5 / self.structure.dh[c]
                    * (1.0 - self.structure.alpha.c[c])
                    * fl.rho.c[c]
                    * fl.mag_u[c].max(self.controls.min_mag_u)
                    * fd;
            }
        }
        for c in 0..n {
            if let Some(m) = self.fs_htc[c] {
                let fl = &self.fluid;
                let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
                self.htc[c] = m
                    .heat_transfer_coefficient(
                        ReynoldsNumber::new::<ratio>(self.re[c]),
                        Ratio::new::<ratio>(pr),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                        Length::new::<meter>(fl.dh[c]),
                        Some((temperature(self.structure.t_act[c]), temperature(fl.t.c[c]))),
                    )
                    .get::<watt_per_square_meter_kelvin>();
            }
        }
    }

    /// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`). With the boundary coefficients folded
    /// into the diagonal (`diag_fold = D + sum iCoeffs`), OpenFOAM's add-`cmptMax(cmptMag(iCoeffs))`,
    /// take-`max(|D|, sumOff)`, divide, subtract-`iCoeffs` sequence gives
    /// `diag_fold' = max(|diag_fold + 2 sum max(-iCoeffs, 0)|, sumOff) / alpha`: a boundary face with a
    /// NEGATIVE internal coefficient (convective inflow through a non-fixed patch) is not neutral.
    /// `inflow2[c]` is that `2 sum max(-iCoeffs, 0)` of cell `c`. Returns `D' - D` per cell, the
    /// coefficient of `psi` in the source correction.
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

    /// `2 sum max(-phi_b, 0)` per owner cell over the boundary faces of `phi` whose patch is not
    /// fixed-value for the transported field (`fixed(f)`): the convective inflow terms of
    /// [`Self::relax_diag`].
    fn convective_inflow2(&self, phi: &[f64], fixed: &dyn Fn(usize) -> bool) -> Vec<f64> {
        let g = &self.geom;
        let mut out = vec![0.0; g.n_cells()];
        for f in g.n_internal()..g.n_faces() {
            if g.active[f] && !fixed(f) {
                out[g.mesh.owner[f]] += 2.0 * (-phi[f]).max(0.0);
            }
        }
        out
    }

    /// `UEqn_1p.H` in `faceCentered` mode (laminar `nuEff`, no tortuosity).
    fn correct_u_eqn(&self) -> VecEqn {
        let g = &self.geom;
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let fl = &self.fluid;
        let mut e = VecEqn::new(g);
        let zg = Scalar::uniform(g, 0.0, g.default_bcs());
        e.eqn.div_upwind(g, &fl.alpha_rho_phi, &zg);
        let ce: Vec<f64> = fl.cont_err.iter().map(|v| -v).collect();
        e.su_sp(g, &ce, &fl.u.c);
        // alpha rho nuEff, nuEff = mu / rho (laminar Stokes)
        let mut arn = Scalar::uniform(g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
        for c in 0..n {
            arn.c[c] = fl.alpha.c[c] * fl.rho.c[c] * (fl.mu.c[c] / fl.rho.c[c]);
        }
        for i in 0..(nf - ni) {
            arn.b[i] = fl.alpha.b[i] * fl.rho.b[i] * (fl.mu.b[i] / fl.rho.b[i]);
        }
        let gamma = fv::interp_linear(g, &arn);
        e.eqn.neg_laplacian(g, &gamma, &zg);
        let dev2 = dev2_div(g, &fl.u, &arn);
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.src[0][c] += v * dev2[c].x;
            e.src[1][c] += v * dev2[c].y;
            e.src[2][c] += v * dev2[c].z;
        }
        e.eqn.sp(g, &self.kd);
        if let Some(fct) = self.controls.relax_u_equation {
            let inflow2 = self.convective_inflow2(&fl.alpha_rho_phi, &|_| false);
            let delta = self.relax_diag(&mut e.eqn, fct, &inflow2);
            for c in 0..n {
                e.src[0][c] += delta[c] * fl.u.c[c].x;
                e.src[1][c] += delta[c] * fl.u.c[c].y;
                e.src[2][c] += delta[c] * fl.u.c[c].z;
            }
        }
        e
    }

    /// Set `p_rgh = p - rho gh` on the cells and on the non-fixed patches.
    fn assign_p_rgh_from_p(&mut self) {
        let n = self.geom.n_cells();
        let ni = self.geom.n_internal();
        for c in 0..n {
            self.p_rgh.c[c] = self.p.c[c] - self.fluid.rho.c[c] * self.gh.c[c];
        }
        for f in ni..self.geom.n_faces() {
            if self.geom.active[f] && !self.p_rgh.is_fixed(&self.geom, f) {
                self.p_rgh.b[f - ni] = self.p.b[f - ni] - self.fluid.rho.b[f - ni] * self.ghf[f];
            }
        }
    }

    /// `pEqnf_1p.H` with its `pEqnRoutines`. Returns the first PISO initial residual.
    fn correct_pressure(&mut self, ueqn: &VecEqn) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let dt = self.dt;
        let nb = nf - ni;
        // updateAlphaPhiAndInitControls_1p.H
        let alpha_f = fv::interp_linear(&g, &self.fluid.alpha);
        self.update_alpha_phi(&alpha_f);
        let ar_by_dt = |alpha: &[f64], rho: &[f64], ab: &[f64], rb: &[f64]| -> Vec<f64> {
            let mut s = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                s.c[c] = alpha[c] * rho[c];
            }
            for i in 0..nb {
                s.b[i] = ab[i] * rb[i];
            }
            fv::interp_linear(&g, &s).into_iter().map(|x| x / dt).collect()
        };
        let fl = &self.fluid;
        let arf = ar_by_dt(&fl.alpha.c, &fl.rho.c, &fl.alpha.b, &fl.rho.b);
        let arf0 = ar_by_dt(&fl.alpha_old, &fl.rho_old, &fl.alpha_old_b, &fl.rho_old_b);
        let mut a = Scalar::uniform(&g, 0.0, g.default_bcs());
        a.c = ueqn.eqn.a(&g);
        a.correct_bc(&g);
        let ai = fv::interp_linear(&g, &a);
        let rauf: Vec<f64> = (0..nf).map(|f| 1.0 / (ai[f] + arf[f])).collect();
        let alpharauf: Vec<f64> = (0..nf).map(|f| alpha_f[f] * rauf[f]).collect();
        let rau_p: Vec<f64> = (0..nf).map(|f| (alpha_f[f] * alpharauf[f]).abs()).collect();

        let mut first_res = 0.0;
        for corr in 1..=self.controls.n_corr.max(1) {
            self.assign_p_rgh_from_p();
            // calcPhiSbyA_1p.H
            let sng = fv::sn_grad(&g, &self.fluid.rho);
            let mut phi_s = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_s[f] = -alpharauf[f] * self.ghf[f] * sng[f] * g.mesh.face_areas[f];
                }
            }
            // phiHbyA = alpharAUf (alphaRhof0ByDt phi.old + flux(H)) + alphaf phiSbyA
            let h = ueqn.h(&g, &self.fluid.u);
            let mut hv = Vector::uniform(&g, Vector3::ZERO);
            hv.c = h;
            hv.correct_bc(&g);
            let fh = fv::flux(&g, &hv);
            let mut phi_hbya = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_hbya[f] =
                        alpharauf[f] * (arf0[f] * self.fluid.phi_old[f] + fh[f]) + alpha_f[f] * phi_s[f];
                }
            }
            // pEqnComp
            let p_rgh_0 = self.p_rgh.c.clone();
            let p_rgh_0_b = self.p_rgh.b.clone();
            let d_ap = fv::div(&g, &self.fluid.alpha_phi);
            let rdt = 1.0 / dt;
            let mut comp_expl = vec![0.0; n];
            let mut cc = vec![0.0; n];
            let fl = &self.fluid;
            for c in 0..n {
                let ddt_alpha = rdt * (fl.alpha.c[c] - fl.alpha_old[c]);
                comp_expl[c] = (fl.cont_err[c] - (ddt_alpha + d_ap[c]) * fl.rho.c[c]) / fl.rho.c[c];
                cc[c] = fl.alpha.c[c] * fl.psi[c] / fl.rho.c[c];
            }
            let mut peqn = Eqn::new(&g);
            peqn.neg_laplacian(&g, &rau_p, &self.p_rgh);
            let dphi = fv::div(&g, &phi_hbya);
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                peqn.ldu.diag[c] += rdt * v * cc[c];
                peqn.source[c] += rdt * v * p_rgh_0[c] * cc[c];
                peqn.source[c] -= v * (comp_expl[c] + dphi[c]);
            }
            let mut x = self.p_rgh.c.clone();
            let res = solve_krylov(&peqn, &mut x, self.controls.p_rgh_tolerance);
            if corr == 1 {
                first_res = res;
            }
            self.p_rgh.c = x;
            self.p_rgh.correct_bc(&g);
            let p_flux = Eqn::laplacian_flux(&g, &rau_p, &self.p_rgh);
            for f in 0..nf {
                if g.active[f] {
                    self.phi_real[f] = phi_hbya[f] + p_flux[f];
                    self.fluid.phi[f] = self.phi_real[f] / alpha_f[f];
                }
            }
            for c in 0..n {
                self.fluid.dgdt[c] = -(comp_expl[c] + cc[c] * (self.p_rgh.c[c] - p_rgh_0[c]) * rdt);
            }
            // reconstructU_1p.H (porousInterfaceSharpness 0)
            let u = fv::reconstruct(&g, &self.fluid.phi);
            self.fluid.u.c = u;
            self.fluid.u.correct_bc(&g);
            self.fluid.mag_u = self.fluid.u.c.iter().map(|v| v.mag()).collect();
            // updatePP_rghRho_1p.H
            for c in 0..n {
                self.p.c[c] = (self.p_rgh.c[c] + self.fluid.rho.c[c] * self.gh.c[c]).max(self.controls.p_min);
            }
            for f in ni..nf {
                if g.active[f] {
                    let i = f - ni;
                    self.p.b[i] = (self.p_rgh.b[i] + self.fluid.rho.b[i] * self.ghf[f]).max(self.controls.p_min);
                }
            }
            for c in 0..n {
                self.fluid.rho.c[c] += self.fluid.psi[c] * (self.p_rgh.c[c] - p_rgh_0[c]);
            }
            for f in ni..nf {
                if g.active[f] {
                    let i = f - ni;
                    self.fluid.rho.b[i] += self.psi_b[i] * (self.p_rgh.b[i] - p_rgh_0_b[i]);
                }
            }
            self.assign_p_rgh_from_p();
            self.p_rgh.correct_bc(&g);
            if self.controls.correct_until_convergence && res < self.controls.p_rgh_tolerance {
                break;
            }
        }
        first_res
    }

    /// `onePhase::correctFluidMechanics`.
    fn correct_fluid_mechanics(&mut self) -> f64 {
        let ueqn = self.correct_u_eqn();
        let r = self.correct_pressure(&ueqn);
        self.correct_cont_err();
        r
    }

    /// `EEqn_1p.H`. Returns the initial residual of the `h` solve.
    fn correct_energy(&mut self) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let dt = self.dt;
        let rdt = 1.0 / dt;
        if !self.p_old_stored {
            self.p_old = self.p.c.clone();
            self.p_old_stored = true;
        }
        let fl = &self.fluid;
        let mut e = Eqn::new(&g);
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.ldu.diag[c] += rdt * fl.alpha.c[c] * fl.rho.c[c] * v;
            e.source[c] += rdt * fl.alpha_old[c] * fl.rho_old[c] * fl.he_old[c] * v;
        }
        e.div_upwind(&g, &fl.alpha_rho_phi, &fl.he);
        let ce: Vec<f64> = fl.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &fl.he.c);
        // - laplacian(interpolate(alpha) interpolate(alphaEff), he), alphaEff = kappa/Cp
        let mut ae = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
        for c in 0..n {
            ae.c[c] = fl.kappa.c[c] / fl.cp.c[c];
        }
        for i in 0..ae.b.len() {
            ae.b[i] = if fl.cp.b[i] > 0.0 { fl.kappa.b[i] / fl.cp.b[i] } else { 0.0 };
        }
        let a_f = fv::interp_linear(&g, &fl.alpha);
        let ae_f = fv::interp_linear(&g, &ae);
        let gam: Vec<f64> = a_f.iter().zip(&ae_f).map(|(a, b)| a * b).collect();
        e.neg_laplacian(&g, &gam, &fl.he);
        // right-hand side: alpha rho (U.g) + alpha ddt(p)
        let mut rhs = vec![0.0; n];
        for c in 0..n {
            rhs[c] = fl.alpha.c[c] * fl.rho.c[c] * fl.u.c[c].dot(self.g);
            if self.controls.dpdt {
                rhs[c] += fl.alpha.c[c] * (rdt * (self.p.c[c] - self.p_old[c]));
            }
        }
        e.su(&g, &rhs);
        if self.controls.include_kinetic_energy {
            let ni = g.n_internal();
            let mut k = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                let u = fl.u.c[c] * 0.5;
                k.c[c] = u.dot(fl.u.c[c]);
            }
            for i in 0..(g.n_faces() - ni) {
                let u = fl.u.b[i] * 0.5;
                k.b[i] = u.dot(fl.u.b[i]);
            }
            let kf = fv::interp_upwind(&g, &k, &fl.alpha_rho_phi);
            let flux: Vec<f64> = (0..g.n_faces()).map(|f| fl.alpha_rho_phi[f] * kf[f]).collect();
            let div_k = fv::div(&g, &flux);
            let mut s = vec![0.0; n];
            for c in 0..n {
                // K.oldTime() of a fresh local field is K itself
                let ddt_k = rdt
                    * (fl.alpha.c[c] * fl.rho.c[c] * k.c[c] - fl.alpha_old[c] * fl.rho_old[c] * k.c[c]);
                s[c] = -(ddt_k + div_k[c] - fl.cont_err[c] * k.c[c]);
            }
            e.su(&g, &s);
        }
        // structure: linearizedSemiImplicitHeatSource(fluid, Hs)
        let mut su = vec![0.0; n];
        let mut sp = vec![0.0; n];
        for c in 0..n {
            let ih = self.structure.ia_act[c] * self.htc[c];
            su[c] = ih * (self.structure.t_act[c] - fl.t.c[c] + fl.he.c[c] / fl.cp.c[c]);
            sp[c] = ih / fl.cp.c[c];
        }
        e.su(&g, &su);
        e.sp(&g, &sp);
        let t_prev = fl.t.c.clone();
        if let Some(fct) = self.controls.relax_h_equation {
            let inflow2 = self.convective_inflow2(&fl.alpha_rho_phi, &|f| fl.he.is_fixed(&g, f));
            let delta = self.relax_diag(&mut e, fct, &inflow2);
            for c in 0..n {
                e.source[c] += delta[c] * fl.he.c[c];
            }
        }
        let prev = fl.he.c.clone();
        let mut x = prev.clone();
        let (sweeps, ini, _) = super::super::one_phase::smooth_solve_sym_gauss_seidel(
            &e.ldu,
            &e.source,
            &mut x,
            SolverSettings { tolerance: self.controls.h_tolerance, max_iter: 1000 },
        );
        if sweeps == 0 && self.controls.h_min_iter > 0 {
            let _ = super::super::one_phase::smooth_solve_sym_gauss_seidel(
                &e.ldu,
                &e.source,
                &mut x,
                SolverSettings { tolerance: 0.0, max_iter: self.controls.h_min_iter },
            );
        }
        self.fluid.he.c = x;
        self.fluid.he.correct_bc(&g);
        self.thermo_correct(false);
        // structure_.correct(Hs*T.prevIter(), Hs)
        let t_used = if self.controls.structure_uses_prev_iter_t { t_prev } else { self.fluid.t.c.clone() };
        for c in 0..n {
            let h = self.htc[c];
            let ht = h * t_used[c];
            if let Some(old) = &self.structure.power_old[c] {
                let mut pm = old.clone();
                let tn = pm.correct(
                    uom::si::f64::HeatTransfer::new::<watt_per_square_meter_kelvin>(h),
                    uom::si::f64::HeatFluxDensity::new::<watt_per_square_meter>(ht),
                    Time::new::<second>(self.dt),
                    Time::new::<second>(self.time),
                );
                self.structure.t_act[c] = tn.get::<kelvin>();
                self.structure.power[c] = Some(pm);
            }
        }
        if self.structure_old_time_stale {
            self.structure.power_old = self.structure.power.clone();
            self.structure_old_time_stale = false;
        }
        ini
    }

    /// `customPimpleControl::criteriaSatisfied` for absolute tolerances (the ported cases'
    /// `p_rgh` uses `useFirstPISOInitialResidual`, whose relative branch compares with a stored
    /// first value of 0 and so never fires, and `h` has `relTol 0`).
    fn criteria_satisfied(&self, r: &OnePhaseStepReport) -> bool {
        let mut checked = false;
        let mut ok = true;
        if let Some(tol) = self.controls.residual_control_p_rgh {
            checked = true;
            ok &= r.p_rgh_initial_residual < tol;
        }
        if let Some(tol) = self.controls.residual_control_h {
            checked = true;
            ok &= r.h_initial_residual < tol;
        }
        checked && ok
    }

    /// Advance one time step (`onePhase::correctPhysics` in upstream's time loop), then set the
    /// next step size (`onePhase::maxDeltaT`, then `Time::adjustDeltaT`).
    ///
    /// # Errors
    /// [`AppBuilderError::Diverged`] when a field turns non-finite.
    pub fn step(&mut self) -> Result<OnePhaseStepReport, AppBuilderError> {
        {
            let f = &mut self.fluid;
            f.alpha_old = f.alpha.c.clone();
            f.alpha_old_b = f.alpha.b.clone();
            f.rho_old = f.rho.c.clone();
            f.rho_old_b = f.rho.b.clone();
            f.he_old = f.he.c.clone();
            f.u_old = f.u.c.clone();
            f.phi_old = f.phi.clone();
        }
        if self.p_old_stored {
            self.p_old = self.p.c.clone();
        }
        self.structure.power_old = self.structure.power.clone();
        self.time += self.dt;
        if let Some(wi) = self.time_controls.write_interval {
            let write_index = (((self.time - self.time_controls.start_time) + 0.5 * self.dt) / wi) as i64;
            if write_index > self.write_time_index {
                self.write_time_index = write_index;
            }
        }
        let mut report = OnePhaseStepReport { time: self.time, dt: self.dt, ..OnePhaseStepReport::default() };
        let n_outer = self.controls.n_outer.max(1);
        let mut corr = 0;
        // customPimpleControl::loop(), call by call
        loop {
            corr += 1;
            if corr == n_outer + 1 {
                if !self.controls.stale_pimple_converged_flag {
                    self.pimple_converged = false;
                }
                break;
            }
            if self.pimple_converged {
                self.pimple_converged = false;
                break;
            }
            if corr > 1 && self.criteria_satisfied(&report) {
                self.pimple_converged = true;
            }
            self.correct_models();
            report.p_rgh_initial_residual = self.correct_fluid_mechanics();
            // `calcCumulContErr` runs `if (pimple_.finalIter())`; the inherited `finalIter()` is
            // `converged_ || corr_ == nCorrPIMPLE` on the BASE class's `corr_`, which the custom loop
            // never advances, so it is true only in the iteration flagged as converged: a step that
            // never meets its criteria adds nothing (measured: 20 prints in 108 steps).
            if self.pimple_converged {
                self.calc_cumul_cont_err(report.dt);
            }
            report.h_initial_residual = self.correct_energy();
            report.outer_iterations = corr;
        }
        if self.fluid.t.c.iter().any(|v| !v.is_finite()) || self.p.c.iter().any(|v| !v.is_finite()) {
            return Err(AppBuilderError::Diverged { iter: corr, residual: f64::NAN });
        }
        report.courant = self.courant();
        if self.time_controls.adjust_time_step {
            self.dt = self.max_delta_t(report.courant);
        }
        self.adjust_to_write_time();
        Ok(report)
    }

    /// `onePhase::calcCumulContErr` (the iteration flagged converged, see the call): `sum(contErr V dt) / sum(V)` is added
    /// to [`Phase::cumul_cont_err`] [kg/m3]; `dt` is the step just taken.
    fn calc_cumul_cont_err(&mut self, dt: f64) {
        let m = &self.geom.mesh;
        let tot_v: f64 = m.cell_volumes.iter().sum();
        let d: f64 = (0..m.n_cells).map(|c| self.fluid.cont_err[c] * m.cell_volumes[c] * dt).sum();
        self.fluid.cumul_cont_err += d / tot_v;
    }

    /// Whether `Time::run()` would take another step (`value < endTime - 0.5 deltaT`).
    #[must_use]
    pub fn running(&self) -> bool {
        self.time < self.end_time - 0.5 * self.dt
    }

    /// `onePhase::correctCourant`: `0.5 max(sum|phi_| / alpha / V) dt`.
    #[must_use]
    pub fn courant(&self) -> f64 {
        let g = &self.geom;
        let sum = fv::surface_sum_mag(g, &self.phi_real);
        let mut co = 0.0_f64;
        for c in 0..g.n_cells() {
            let s = sum[c] / self.fluid.alpha.c[c];
            co = co.max(s / g.mesh.cell_volumes[c]);
        }
        0.5 * co * self.dt
    }

    /// `onePhase::maxDeltaT`.
    fn max_delta_t(&self, co: f64) -> f64 {
        let fact = self.time_controls.max_co / (co + 1e-15);
        let dfact = fact.min(1.0 + 0.1 * fact).min(1.2);
        (dfact * self.dt).min(self.time_controls.max_delta_t)
    }

    /// ESI `Time::adjustDeltaT` for `writeControl adjustableRunTime`.
    fn adjust_to_write_time(&mut self) {
        let Some(wi) = self.time_controls.write_interval else { return };
        let to_next = (((self.write_time_index + 1) as f64) * wi - (self.time - self.time_controls.start_time)).max(0.0);
        let n_steps = to_next / self.dt;
        if n_steps < f64::from(i32::MAX) {
            let n = n_steps.round().max(1.0);
            let new_dt = to_next / n;
            self.dt = if new_dt >= self.dt { new_dt.min(2.0 * self.dt) } else { new_dt.max(0.2 * self.dt) };
        }
    }

    /// Ablation of [`OnePhaseFcControls::lazy_old_time_first_step`].
    #[must_use]
    pub fn without_first_step_lazy_old_time(mut self) -> Self {
        self.controls.lazy_old_time_first_step = false;
        self.p_old_stored = true;
        self.structure_old_time_stale = false;
        self
    }

    /// The clean `customPimpleControl::loop` ending ([`OnePhaseFcControls::stale_pimple_converged_flag`]
    /// `= false`).
    #[must_use]
    pub fn without_stale_pimple_converged_flag(mut self) -> Self {
        self.controls.stale_pimple_converged_flag = false;
        self
    }

    /// Skip `fvMatrix::relax(1)` on `UEqn` and `EEqn`.
    #[must_use]
    pub fn without_equation_relaxation(mut self) -> Self {
        self.controls.relax_u_equation = None;
        self.controls.relax_h_equation = None;
        self
    }

    /// `includeKineticEnergy false`.
    #[must_use]
    pub fn without_kinetic_energy(mut self) -> Self {
        self.controls.include_kinetic_energy = false;
        self
    }

    /// `structure.correct` with the temperature after the energy solve.
    #[must_use]
    pub fn with_structure_after_solve_temperature(mut self) -> Self {
        self.controls.structure_uses_prev_iter_t = false;
        self
    }
}
