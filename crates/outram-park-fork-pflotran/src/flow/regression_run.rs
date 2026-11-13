//! Run a genuine PFLOTRAN regression deck (RICHARDS) the way PFLOTRAN 669fbce
//! steps it, so its output can be compared with the shipped
//! `.regression.gold` file (gh:#1537, rows 3-5 of gh:#1422).
//!
//! The spatial operator is this crate's [`RichardsProblem`] (two-point flux,
//! upstream-weighted mobility, backward Euler). What this module adds is the
//! part that decides *which* time steps are taken and *when* a step is
//! converged, translated from PFLOTRAN so the two codes walk the same
//! sequence:
//!
//! | Piece | Upstream source (669fbce, `src/pflotran/`) |
//! |---|---|
//! | first dt 1 s, `dt_max` from `TIME`, growth factor 2, reduction factor 0.5, 16 cuts, 5 constant steps after a cut | `timestepper_base.F90` `TimestepperBaseInit` (lines 150-178) |
//! | waypoints: stop exactly on every `OUTPUT` time and `FINAL_TIME`, revert dt afterwards, tolerance 0.1 | `TimestepperBaseSetTargetTime` (469-680) |
//! | dt growth `ts_acceleration` 5 (`fac` 0.5 or 0.33), pressure-change governor 5e5 | `PMRichardsUpdateTimestep` (`pm_richards.F90` 446-507), `TimestepperSNESUpdateDT` (`timestepper_SNES.F90` 239-311) |
//! | cut on a failed Newton solve, re-target | `TimestepperSNESStepDT` (315-497), `TimestepperBaseCutDT` (713-803) |
//! | Newton convergence: PETSc `SNESConvergedDefault` (`rtol` 1e-8 against the first residual 2-norm, `stol` 1e-8, `atol` 1e-50), at least one iteration; the infinity-norm tolerances are UNINITIALIZED (-999) so inactive | `solver.F90` 136-147, `convergence.F90` (`ConvergenceTest`) |
//! | `LIQUID_SATURATION` condition to pressure through the saturation function | `PatchUpdateCouplerSaturation` (`patch.F90` 6071-6130) |
//!
//! # Assumptions that could not be checked against a run of this port
//!
//! * PETSc's default `bt` line search is reduced to its first test: the full
//!   Newton step is accepted when `||F(x - y)||^2 <= (1 - 2 alpha) ||F(x)||^2`,
//!   `alpha = 1e-4` (initial slope taken as `-||F||^2`, exact for an exact
//!   linear solve). If a step fails that test this module returns an error
//!   instead of backtracking: backtracking is **not translated**.
//! * Waypoints at or before the start time are dropped (upstream pops them
//!   while initialising; the exact code was not read).
//! * The Jacobian is [`RichardsProblem`]'s finite-difference one and the linear
//!   system is solved to a relative residual of 1e-13, where upstream uses an
//!   analytic Jacobian and one BiCGStab/ILU iteration.
//!
//! > Untrusted AI-generated draft until a human reviews it.

use crate::deck::regression::{
    RegressionCoupler, RegressionCurves, RegressionDeck, RegressionFace, RegressionFlowValue,
    RegressionPermeabilityFunction, RegressionSaturationFunction,
};
use crate::deck::FlowMode as DeckFlowMode;
use crate::error::PflotranError;
use crate::flow::richards::{
    BoundaryCondition, BoundaryConditionKind, RichardsProblem, ATMOSPHERIC_PRESSURE,
    STANDARD_GRAVITY,
};
use crate::grid::{BoundaryLocation, CartesianGrid};
use crate::properties::{CharacteristicCurves, LiquidWaterEos, Linear, VanGenuchten};
use crate::solver::NonlinearSystem;
use outram_foam_basic_lib::krylov::{self, KrylovSettings, Preconditioner};
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use uom::si::f64::{Length, Pressure};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// PFLOTRAN `pressure_change_governor` default (`pm_subsurface_flow.F90` line 128).
const PRESSURE_CHANGE_GOVERNOR: f64 = 5.0e5;
/// `iaccel` default of `TimestepperSNESInit` (`timestepper_SNES.F90` line 129).
const TS_ACCELERATION: usize = 5;
/// Maximum Newton iterations (PETSc default `snes_max_it` 50).
const NEWTON_MAX_ITERATIONS: usize = 50;
/// PETSc `snes_rtol` default.
const SNES_RTOL: f64 = 1.0e-8;
/// PETSc `snes_stol` default.
const SNES_STOL: f64 = 1.0e-8;
/// PETSc `snes_atol` default.
const SNES_ATOL: f64 = 1.0e-50;
/// PETSc `snes_divtol` default.
const SNES_DIVTOL: f64 = 1.0e4;
/// PETSc `bt` line-search Armijo constant `alpha`.
const LINESEARCH_ALPHA: f64 = 1.0e-4;

/// Result of running a regression deck.
#[derive(Debug, Clone)]
pub struct RegressionRunReport {
    /// Accepted time steps (`Time Steps` in the regression output).
    pub time_steps: usize,
    /// Newton iterations of accepted steps, plus wasted ones of cut steps
    /// (`Newton Iterations`).
    pub newton_iterations: usize,
    /// Total time-step cuts (`Time Step Cuts`).
    pub time_step_cuts: usize,
    /// Time reached (s).
    pub final_time: f64,
    /// Initial liquid pressure (Pa) derived from the initial condition.
    pub initial_pressure: f64,
    /// Final liquid pressure per cell (Pa), natural order.
    pub pressure: Vec<f64>,
    /// Final liquid saturation per cell.
    pub saturation: Vec<f64>,
    /// The dt of each accepted step (s).
    pub dt_history: Vec<f64>,
    /// Newton iterations of each accepted step.
    pub newton_per_step: Vec<usize>,
}

fn face_to_location(f: RegressionFace) -> Result<BoundaryLocation, PflotranError> {
    Ok(match f {
        RegressionFace::West => BoundaryLocation::XMin,
        RegressionFace::East => BoundaryLocation::XMax,
        RegressionFace::South => BoundaryLocation::YMin,
        RegressionFace::North => BoundaryLocation::YMax,
        RegressionFace::Bottom => BoundaryLocation::ZMin,
        RegressionFace::Top => BoundaryLocation::ZMax,
    })
}

fn uniform_width(w: &[f64], what: &str) -> Result<f64, PflotranError> {
    let first = w
        .first()
        .copied()
        .ok_or_else(|| PflotranError::InvalidInput(format!("empty {what} widths")))?;
    if w.iter().any(|v| (v - first).abs() > 1.0e-12 * first.abs()) {
        return Err(PflotranError::InvalidInput(format!(
            "non-uniform {what} cell widths are not supported by the regression runner"
        )));
    }
    Ok(first)
}

/// Build the characteristic curves of the (single) material's curve set.
fn build_curves(c: &RegressionCurves) -> Result<CharacteristicCurves, PflotranError> {
    match (&c.saturation_function, &c.liquid_permeability) {
        (
            RegressionSaturationFunction::Linear {
                residual_saturation,
                alpha,
                max_capillary_pressure,
            },
            RegressionPermeabilityFunction::BurdineLinearLiquid {
                residual_saturation: rpf_sr,
            },
        ) => {
            if rpf_sr != residual_saturation {
                return Err(PflotranError::InvalidInput(format!(
                    "saturation function Sr {residual_saturation} differs from permeability function Sr {rpf_sr}: not supported"
                )));
            }
            Ok(CharacteristicCurves::Linear(Linear::new(
                *alpha,
                *max_capillary_pressure,
                *residual_saturation,
            )?))
        }
        (
            RegressionSaturationFunction::VanGenuchten {
                m,
                alpha,
                residual_saturation,
            },
            RegressionPermeabilityFunction::MualemVgLiquid {
                m: rpf_m,
                residual_saturation: rpf_sr,
            },
        ) => {
            if rpf_m != m || rpf_sr != residual_saturation {
                return Err(PflotranError::InvalidInput(
                    "van Genuchten and Mualem parameters differ: not supported".into(),
                ));
            }
            Ok(CharacteristicCurves::VanGenuchten(VanGenuchten::new(
                *alpha,
                1.0 / (1.0 - m),
                *residual_saturation,
            )?))
        }
        _ => Err(PflotranError::InvalidInput(
            "unsupported saturation/permeability function pairing".into(),
        )),
    }
}

/// Pressure of a Dirichlet flow condition (saturation goes through the
/// saturation function, `PatchUpdateCouplerSaturation`).
fn condition_pressure(
    deck: &RegressionDeck,
    curves: &CharacteristicCurves,
    coupler: &RegressionCoupler,
) -> Result<f64, PflotranError> {
    let fc = deck
        .flow_conditions
        .iter()
        .find(|f| f.name == coupler.flow_condition)
        .ok_or_else(|| {
            PflotranError::InvalidInput(format!("unknown FLOW_CONDITION `{}`", coupler.flow_condition))
        })?;
    match fc.value {
        RegressionFlowValue::LiquidPressure(p) => Ok(p),
        RegressionFlowValue::LiquidSaturation(s) => match curves {
            CharacteristicCurves::Linear(l) => Ok(ATMOSPHERIC_PRESSURE - l.capillary_pressure(s)),
            _ => Err(PflotranError::InvalidInput(
                "LIQUID_SATURATION conditions are only translated for the LINEAR saturation function".into(),
            )),
        },
    }
}

/// Everything the stepper needs, resolved from the deck.
struct Setup {
    problem: RichardsProblem,
    curves: CharacteristicCurves,
    initial_pressure: f64,
    n_cells: usize,
}

fn build_setup(deck: &RegressionDeck) -> Result<Setup, PflotranError> {
    if deck.mode != DeckFlowMode::Richards {
        return Err(PflotranError::InvalidInput(
            "the regression runner supports MODE RICHARDS only".into(),
        ));
    }
    let g = &deck.grid;
    let dx = uniform_width(&g.dx, "x")?;
    let dy = uniform_width(&g.dy, "y")?;
    let dz = uniform_width(&g.dz, "z")?;
    let grid = CartesianGrid::uniform(
        g.nx,
        g.ny,
        g.nz,
        Length::new::<meter>(dx),
        Length::new::<meter>(dy),
        Length::new::<meter>(dz),
    )?;
    let n_cells = grid.n_cells();

    if deck.strata.len() != 1 || deck.materials.len() != 1 {
        return Err(PflotranError::InvalidInput(
            "exactly one MATERIAL_PROPERTY and one STRATA are supported (homogeneous domain)".into(),
        ));
    }
    let mat = &deck.materials[0];
    if deck.strata[0].material != mat.name {
        return Err(PflotranError::InvalidInput("STRATA names an unknown material".into()));
    }
    if mat.permeability[0] != mat.permeability[1] || mat.permeability[0] != mat.permeability[2] {
        return Err(PflotranError::InvalidInput("anisotropic permeability is not supported".into()));
    }
    let cc = deck
        .curves
        .iter()
        .find(|c| c.name == mat.characteristic_curves)
        .ok_or_else(|| PflotranError::InvalidInput("material names unknown CHARACTERISTIC_CURVES".into()))?;
    let curves = build_curves(cc)?;

    let density = deck.eos_water.density.ok_or_else(|| {
        PflotranError::InvalidInput(
            "EOS WATER DENSITY CONSTANT is required: PFLOTRAN's default IFC67 density is not translated".into(),
        )
    })?;
    let viscosity = deck.eos_water.viscosity.ok_or_else(|| {
        PflotranError::InvalidInput(
            "EOS WATER VISCOSITY CONSTANT is required: PFLOTRAN's default viscosity is not translated".into(),
        )
    })?;
    let eos = LiquidWaterEos::new(density, ATMOSPHERIC_PRESSURE, 0.0, viscosity)?;

    if deck.initial_conditions.len() != 1 {
        return Err(PflotranError::InvalidInput("exactly one INITIAL_CONDITION is supported".into()));
    }
    let ic = &deck.initial_conditions[0];
    let ic_region = deck
        .regions
        .iter()
        .find(|r| r.name == ic.region)
        .ok_or_else(|| PflotranError::InvalidInput("INITIAL_CONDITION names an unknown region".into()))?;
    if ic_region.face.is_some() || ic.region != deck.strata[0].region {
        return Err(PflotranError::InvalidInput(
            "the initial condition must cover the whole (strata) region".into(),
        ));
    }
    let initial_pressure = condition_pressure(deck, &curves, ic)?;

    let mut boundary = Vec::new();
    for bc in &deck.boundary_conditions {
        let region = deck
            .regions
            .iter()
            .find(|r| r.name == bc.region)
            .ok_or_else(|| PflotranError::InvalidInput("BOUNDARY_CONDITION names an unknown region".into()))?;
        let face = region
            .face
            .ok_or_else(|| PflotranError::InvalidInput("a boundary region needs a FACE".into()))?;
        boundary.push(BoundaryCondition {
            location: face_to_location(face)?,
            kind: BoundaryConditionKind::DirichletPressure(condition_pressure(deck, &curves, bc)?),
        });
    }

    let problem = RichardsProblem::new(
        grid,
        eos,
        curves,
        mat.porosity,
        mat.permeability[0],
        STANDARD_GRAVITY,
        ATMOSPHERIC_PRESSURE,
        boundary,
    )?;
    Ok(Setup {
        problem,
        curves,
        initial_pressure,
        n_cells,
    })
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Outcome of one nonlinear solve.
enum SolveOutcome {
    Converged(usize),
    Failed(usize),
}

/// PETSc `SNESSolve` (newtonls, full-step `bt` line search, `SNESConvergedDefault`
/// plus PFLOTRAN's one-iteration minimum). `x` is updated in place.
fn snes_solve(problem: &mut RichardsProblem, x: &mut [f64]) -> Result<SolveOutcome, PflotranError> {
    let n = x.len();
    let (owner, neighbour) = problem.ldu_addressing();
    let mut jac = LduMatrix::new(n, owner, neighbour);
    let settings = KrylovSettings {
        tolerance: 1.0e-13,
        max_iter: 1000,
        restart: 30,
    };
    let mut f = vec![0.0; n];
    problem.assemble_residual(x, &mut f)?;
    let mut fnorm = norm2(&f);
    let fnorm0 = fnorm;
    if !fnorm.is_finite() {
        return Ok(SolveOutcome::Failed(0));
    }
    let mut x_new = vec![0.0; n];
    let mut f_new = vec![0.0; n];
    for it in 1..=NEWTON_MAX_ITERATIONS {
        problem.assemble_jacobian(x, &mut jac)?;
        let precond = Preconditioner::ilu0(&jac);
        let (y, _res) = krylov::bicgstab(&jac, &f, None, &precond, &settings);
        if !y.iter().all(|v| v.is_finite()) {
            return Ok(SolveOutcome::Failed(it));
        }
        for i in 0..n {
            x_new[i] = x[i] - y[i];
        }
        problem.assemble_residual(&x_new, &mut f_new)?;
        let gnorm = norm2(&f_new);
        if !gnorm.is_finite() {
            return Ok(SolveOutcome::Failed(it));
        }
        // PETSc bt: accept lambda = 1 when 0.5 g^2 <= 0.5 f^2 + alpha * initslope,
        // initslope = -f^2.
        if 0.5 * gnorm * gnorm > 0.5 * fnorm * fnorm - LINESEARCH_ALPHA * fnorm * fnorm {
            return Err(PflotranError::InvalidInput(format!(
                "Newton iteration {it}: the full step failed PETSc's line-search decrease test; \
                 backtracking is not translated (||F|| {fnorm:e} -> {gnorm:e})"
            )));
        }
        let snorm = norm2(&y);
        let xnorm = norm2(&x_new);
        x.copy_from_slice(&x_new);
        f.copy_from_slice(&f_new);
        fnorm = gnorm;
        // SNESConvergedDefault
        if fnorm < SNES_ATOL || fnorm <= fnorm0 * SNES_RTOL || snorm < SNES_STOL * xnorm {
            return Ok(SolveOutcome::Converged(it));
        }
        if fnorm > SNES_DIVTOL * fnorm0 {
            return Ok(SolveOutcome::Failed(it));
        }
    }
    Ok(SolveOutcome::Failed(NEWTON_MAX_ITERATIONS))
}

/// The time-stepper state of `timestepper_base_type` / `timestepper_SNES_type`.
struct Stepper {
    dt: f64,
    dt_max: f64,
    dt_min: f64,
    prev_dt: f64,
    target_time: f64,
    revert_dt: bool,
    cut_flag: bool,
    num_constant: usize,
    waypoints: Vec<f64>,
    cur_waypoint: usize,
    prev_waypoint: usize,
}

impl Stepper {
    fn new(deck: &RegressionDeck) -> Self {
        let mut wp: Vec<f64> = deck
            .output_times_s
            .iter()
            .copied()
            .filter(|t| *t > 0.0)
            .collect();
        wp.push(deck.time.final_time_s);
        wp.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        wp.dedup();
        Self {
            dt: deck.time.initial_dt_s.unwrap_or(1.0),
            dt_max: deck.time.max_dt_s.unwrap_or(f64::MAX),
            dt_min: 1.0e-20,
            prev_dt: 0.0,
            target_time: 0.0,
            revert_dt: false,
            cut_flag: false,
            num_constant: 0,
            waypoints: wp,
            cur_waypoint: 0,
            prev_waypoint: 0,
        }
    }

    /// `TimestepperBaseSetTargetTime` with every waypoint forcing a match.
    /// Returns `true` when this is the last step.
    fn set_target_time(&mut self) -> bool {
        if self.cut_flag {
            self.cut_flag = false;
            self.cur_waypoint = self.prev_waypoint;
        } else if self.revert_dt {
            self.dt = self.prev_dt.min(self.dt_max);
        }
        self.revert_dt = false;
        let mut revert_due_to_waypoint = false;
        let mut dt = self.dt;
        self.prev_dt = dt;
        let mut cur = self.cur_waypoint;
        self.prev_waypoint = cur;
        let mut target_time;
        let tolerance = 0.1;
        loop {
            dt = dt.min(self.dt_max);
            target_time = self.target_time + dt;
            let wp = self.waypoints[cur];
            let exceeds = target_time + tolerance * dt >= wp;
            if exceeds {
                target_time -= dt;
                dt = wp - target_time;
                if dt > self.dt_max && (dt - self.dt_max).abs() > 1.0 {
                    dt = self.dt_max;
                    target_time += dt;
                } else {
                    target_time = wp;
                    revert_due_to_waypoint = true;
                    cur += 1;
                }
                break;
            } else if target_time > wp {
                cur += 1;
            } else {
                break;
            }
        }
        if revert_due_to_waypoint {
            self.revert_dt = true;
        }
        self.dt = dt;
        self.target_time = target_time;
        self.cur_waypoint = cur;
        cur >= self.waypoints.len()
    }

    /// `TimestepperSNESUpdateDT` + `PMRichardsUpdateTimestep` (positive
    /// `ts_acceleration`, no CFL governor).
    fn update_dt(&mut self, newton_iterations: usize, max_pressure_change: f64) {
        let mut update = true;
        if self.cut_flag {
            self.num_constant = 1;
        } else if self.num_constant > 0 {
            self.num_constant += 1;
        }
        if self.num_constant > 5 {
            self.num_constant = 0;
        } else if self.num_constant > 0 {
            update = false;
        }
        if update {
            let mut fac = 0.5;
            let ut;
            if newton_iterations >= TS_ACCELERATION {
                fac = 0.33;
                ut = 0.0;
            } else {
                // Fortran: `max_pressure_change+0.1` adds a default-REAL literal.
                ut = PRESSURE_CHANGE_GOVERNOR / (max_pressure_change + f64::from(0.1_f32));
            }
            let mut dtt = fac * self.dt * (1.0 + ut);
            dtt = dtt.min(2.0 * self.dt);
            if dtt > self.dt_max {
                dtt = self.dt_max;
            }
            self.dt = dtt.max(self.dt_min);
        }
    }
}

/// Saturation field from a pressure field (`RichardsAuxVarCompute`).
fn saturation_field(curves: &CharacteristicCurves, p: &[f64]) -> Vec<f64> {
    let sr = curves.residual_saturation();
    p.iter()
        .map(|pi| {
            let pc = ATMOSPHERIC_PRESSURE - pi;
            if pc <= 0.0 {
                1.0
            } else {
                let se = curves
                    .effective_saturation(Pressure::new::<pascal>(pc))
                    .get::<ratio>();
                sr + (1.0 - sr) * se
            }
        })
        .collect()
}

/// Run a parsed regression deck to its final time.
///
/// # Errors
///
/// [`PflotranError::InvalidInput`] for a deck outside the supported subset or
/// when a Newton step needs the (untranslated) line-search backtracking;
/// [`PflotranError::Convergence`] when the step cut limit (16) is exceeded.
pub fn run_regression_deck(deck: &RegressionDeck) -> Result<RegressionRunReport, PflotranError> {
    let Setup {
        mut problem,
        curves,
        initial_pressure,
        n_cells,
    } = build_setup(deck)?;
    let mut x = vec![initial_pressure; n_cells];
    let mut st = Stepper::new(deck);
    let mut time_steps = 0usize;
    let mut newton_total = 0usize;
    let mut cuts_total = 0usize;
    let mut dt_history = Vec::new();
    let mut newton_per_step = Vec::new();
    loop {
        let last = st.set_target_time();
        let t_before = st.target_time - st.dt;
        let x_old = x.clone();
        let mut icut = 0usize;
        let newton_this;
        loop {
            problem.set_timestep(st.dt);
            problem.set_previous(&x_old);
            let mut trial = x_old.clone();
            match snes_solve(&mut problem, &mut trial)? {
                SolveOutcome::Converged(it) => {
                    newton_total += it;
                    newton_this = it;
                    x = trial;
                    break;
                }
                SolveOutcome::Failed(it) => {
                    newton_total += it;
                    icut += 1;
                    cuts_total += 1;
                    st.cut_flag = true;
                    if icut > 16 || st.dt < st.dt_min {
                        return Err(PflotranError::Convergence(format!(
                            "time step cut criteria exceeded at t = {t_before}"
                        )));
                    }
                    st.dt *= 0.5;
                    st.target_time = t_before + st.dt;
                }
            }
        }
        time_steps += 1;
        dt_history.push(st.dt);
        newton_per_step.push(newton_this);
        let max_change = x
            .iter()
            .zip(x_old.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        st.update_dt(newton_this, max_change);
        if last {
            break;
        }
    }
    let saturation = saturation_field(&curves, &x);
    Ok(RegressionRunReport {
        time_steps,
        newton_iterations: newton_total,
        time_step_cuts: cuts_total,
        final_time: st.target_time,
        initial_pressure,
        pressure: x,
        saturation,
        dt_history,
        newton_per_step,
    })
}
