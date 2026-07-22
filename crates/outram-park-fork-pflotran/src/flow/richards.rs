//! RICHARDS flow mode — variably-saturated single-phase groundwater flow.
//!
//! Solves the liquid-phase mass-conservation (Richards') equation
//!
//! $$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla \cdot \left(\rho_l\, \mathbf{q}_l\right) = Q_l $$
//!
//! with the Darcy flux
//!
//! $$ \mathbf{q}_l = -\frac{k\, k_{rl}}{\mu_l}\left(\nabla p_l - \rho_l\, \mathbf{g}\right) $$
//!
//! discretised in space by a **cell-centred two-point flux** finite volume on a
//! structured Cartesian [`grid`](crate::grid), and in time by **backward
//! (implicit) Euler**. The single primary unknown per cell is the liquid
//! pressure `p_l` (Pa). The nonlinear system `F(p) = 0` at each timestep is
//! handed to the crate's [`NewtonSolver`](crate::solver::NewtonSolver), whose
//! linear step uses `outram-foam-basic-lib`'s asymmetric Krylov solvers.
//!
//! Mobility (`k_rl`) is **upstream-weighted** on each face — the physically
//! correct choice for advective transport, and the reason the Jacobian is
//! nonsymmetric (hence BiCGStab / GMRES rather than CG).
//!
//! ## Sign & reference conventions
//!
//! - `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
//! - Flow potential on a connection: `Phi = p + rho_face * g * z`.
//! - Capillary pressure `p_c = p_gas - p_l`, with a fixed reference gas pressure
//!   `p_gas` (default atmospheric). `p_c <= 0` means fully saturated.
//! - Residual `F_i` is a mass rate (kg/s); positive face/boundary flux leaves
//!   the cell. Unspecified boundaries are **no-flow** (natural zero Neumann).
//!
//! > **Untrusted AI draft — verification only.** See `docs/ai-directed-decisions.md`
//! > (esp. D6, D8) and the crate README "Bookkeeping status". No validation
//! > against published PFLOTRAN results has been done.

use crate::error::PflotranError;
use crate::grid::{BoundaryLocation, CartesianGrid};
use crate::io::{BoundaryKindSpec, BoundaryLocationSpec, CurveModel, InputDeck};
use crate::properties::{BrooksCorey, CharacteristicCurves, LiquidWaterEos, VanGenuchten};
use crate::solver::{NewtonConfig, NewtonSolver, NonlinearSystem};
use outram_foam_basic_lib::ldu_matrix::LduMatrix;

use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Length, Pressure, Ratio};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// Standard gravitational acceleration (m/s^2), the default for a simulation.
pub const STANDARD_GRAVITY: f64 = 9.806_65;
/// Standard atmospheric pressure (Pa), the default reference gas pressure.
pub const ATMOSPHERIC_PRESSURE: f64 = 101_325.0;

/// What is prescribed on one exterior boundary location.
///
/// Pressures are in pascal (Pa); Neumann fluxes are a Darcy velocity in metres
/// per second (m/s), **positive = inflow into the domain**.
pub enum BoundaryConditionKind {
    /// Fixed liquid pressure `p` (Pa) on the boundary (first-type / Dirichlet).
    DirichletPressure(f64),
    /// Fixed normal Darcy flux `q` (m/s), positive pointing **into** the
    /// domain (second-type / Neumann). `q = 0` is a no-flow wall.
    NeumannFlux(f64),
}

/// A boundary condition applied to one of the six Cartesian domain faces.
pub struct BoundaryCondition {
    /// Which exterior face of the logical box this applies to.
    pub location: BoundaryLocation,
    /// What is prescribed there.
    pub kind: BoundaryConditionKind,
}

#[inline]
fn location_index(loc: &BoundaryLocation) -> usize {
    match loc {
        BoundaryLocation::XMin => 0,
        BoundaryLocation::XMax => 1,
        BoundaryLocation::YMin => 2,
        BoundaryLocation::YMax => 3,
        BoundaryLocation::ZMin => 4,
        BoundaryLocation::ZMax => 5,
    }
}

/// The RICHARDS nonlinear system for a single implicit-Euler timestep.
///
/// Owns the grid, fluid EOS, characteristic curves, and homogeneous material
/// data for the v1 slice (isotropic permeability, uniform porosity). Implements
/// [`NonlinearSystem`] so it can be driven by [`NewtonSolver`]. The Jacobian is
/// assembled **numerically** (local finite differences of the residual over the
/// two-point stencil) — this is exact-to-round-off, matches the residual by
/// construction, and side-steps the singular analytical `dk_r/dS` slope of the
/// van Genuchten–Mualem model near full saturation (decision D8/D6).
pub struct RichardsProblem {
    grid: CartesianGrid,
    eos: LiquidWaterEos,
    curves: CharacteristicCurves,
    porosity: f64,
    permeability: f64,
    gravity: f64,
    reference_gas_pressure: f64,
    viscosity: f64,
    // Precomputed helpers (built in `new`).
    z: Vec<f64>,
    cell_faces: Vec<Vec<usize>>,
    cell_boundary_faces: Vec<Vec<usize>>,
    bc: [Option<BoundaryConditionKind>; 6],
    // Per-cell volumetric mass source (kg/s), positive = injection. Default 0.
    source: Vec<f64>,
    // Time-stepping state.
    dt: f64,
    p_old: Vec<f64>,
}

impl RichardsProblem {
    /// Build a RICHARDS problem.
    ///
    /// - `porosity` in (0,1); `permeability` intrinsic isotropic k (m^2) > 0;
    /// - `gravity` magnitude (m/s^2, acting in `-z`; pass 0 to disable);
    /// - `reference_gas_pressure` (Pa) used in `p_c = p_gas - p_l`;
    /// - `boundary` conditions per face (any face omitted is no-flow).
    ///
    /// The timestep defaults to 1 s and `p_old` to zeros until
    /// [`set_timestep`](Self::set_timestep) / [`set_previous`](Self::set_previous)
    /// are called (the driver does this each step).
    pub fn new(
        grid: CartesianGrid,
        eos: LiquidWaterEos,
        curves: CharacteristicCurves,
        porosity: f64,
        permeability: f64,
        gravity: f64,
        reference_gas_pressure: f64,
        boundary: Vec<BoundaryCondition>,
    ) -> Result<Self, PflotranError> {
        if !(porosity > 0.0 && porosity < 1.0) {
            return Err(PflotranError::InvalidInput(format!(
                "porosity must be in (0,1), got {porosity}"
            )));
        }
        if !(permeability > 0.0) || !permeability.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "permeability must be positive, got {permeability}"
            )));
        }
        if gravity < 0.0 || !gravity.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "gravity magnitude must be >= 0, got {gravity}"
            )));
        }
        let n = grid.n_cells();
        let z: Vec<f64> = (0..n).map(|c| grid.cell_center(c)[2]).collect();

        let mut cell_faces = vec![Vec::new(); n];
        for (f, c) in grid.connections().iter().enumerate() {
            cell_faces[c.owner].push(f);
            cell_faces[c.neighbour].push(f);
        }
        let mut cell_boundary_faces = vec![Vec::new(); n];
        for (b, bf) in grid.boundary_faces().iter().enumerate() {
            cell_boundary_faces[bf.cell].push(b);
        }

        let mut bc: [Option<BoundaryConditionKind>; 6] = [None, None, None, None, None, None];
        for cond in boundary {
            bc[location_index(&cond.location)] = Some(cond.kind);
        }

        let viscosity = eos.viscosity().get::<pascal_second>();

        Ok(Self {
            grid,
            eos,
            curves,
            porosity,
            permeability,
            gravity,
            reference_gas_pressure,
            viscosity,
            z,
            cell_faces,
            cell_boundary_faces,
            bc,
            source: vec![0.0; n],
            dt: 1.0,
            p_old: vec![0.0; n],
        })
    }

    /// Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be
    /// positive.
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Set the previous-time-level pressure field (Pa), length `n_cells`.
    pub fn set_previous(&mut self, p_old: &[f64]) {
        self.p_old.copy_from_slice(p_old);
    }

    /// Set a per-cell volumetric mass source (kg/s), positive = injection.
    pub fn set_source(&mut self, source: &[f64]) {
        self.source.copy_from_slice(source);
    }

    /// Read access to the underlying grid.
    pub fn grid(&self) -> &CartesianGrid {
        &self.grid
    }

    /// Total fluid mass in the domain (kg) for a pressure field `p`:
    /// `sum_i V_i * phi * S_l(p_i) * rho_l(p_i)`.
    ///
    /// This is the invariant Celia's mixed (mass-conservative) form preserves:
    /// with no-flow boundaries and no source, it is unchanged across a timestep
    /// (to the nonlinear-solver tolerance), because the summed internal fluxes
    /// cancel and the summed residual is driven to zero.
    pub fn total_mass(&self, p: &[f64]) -> f64 {
        (0..self.grid.n_cells())
            .map(|i| self.grid.cell_volume(i) * self.theta(p[i]))
            .sum()
    }

    // --- pointwise property helpers (plain f64, SI units) ---

    #[inline]
    fn rho(&self, p: f64) -> f64 {
        self.eos
            .density(Pressure::new::<pascal>(p))
            .get::<kilogram_per_cubic_meter>()
    }

    /// Effective saturation Se in [0,1] from liquid pressure.
    #[inline]
    fn se(&self, p: f64) -> f64 {
        let pc = self.reference_gas_pressure - p;
        self.curves
            .effective_saturation(Pressure::new::<pascal>(pc))
            .get::<ratio>()
    }

    /// Physical liquid saturation `S_l = S_r + (1 - S_r) * Se`.
    #[inline]
    fn saturation(&self, p: f64) -> f64 {
        let sr = self.curves.residual_saturation();
        sr + (1.0 - sr) * self.se(p)
    }

    /// Relative permeability k_r(Se(p)) in [0,1].
    #[inline]
    fn kr(&self, p: f64) -> f64 {
        let se = self.se(p);
        self.curves
            .relative_permeability(Ratio::new::<ratio>(se))
            .get::<ratio>()
    }

    /// Storage term theta = phi * S_l * rho_l (kg/m^3).
    #[inline]
    fn theta(&self, p: f64) -> f64 {
        self.porosity * self.saturation(p) * self.rho(p)
    }

    /// Two-point mass flux from `p_o`,`z_o` to `p_n`,`z_n` across a face of
    /// geometric transmissibility `t_geom` (= area/distance, m). Returns kg/s,
    /// positive = flow from owner to neighbour. Mobility upstream-weighted.
    #[inline]
    fn two_point_flux(&self, p_o: f64, z_o: f64, p_n: f64, z_n: f64, t_geom: f64) -> f64 {
        let rho_o = self.rho(p_o);
        let rho_n = self.rho(p_n);
        let rho_face = 0.5 * (rho_o + rho_n);
        let dphi = (p_o - p_n) + rho_face * self.gravity * (z_o - z_n);
        let kr_up = if dphi >= 0.0 { self.kr(p_o) } else { self.kr(p_n) };
        let mobility = self.permeability * kr_up / self.viscosity;
        rho_face * mobility * t_geom * dphi
    }

    /// Residual `F_i(p)` (kg/s) for one cell: accumulation + net outflow across
    /// internal faces + boundary fluxes - source.
    fn cell_residual(&self, i: usize, p: &[f64]) -> f64 {
        // Accumulation (implicit Euler): V/dt * (theta(p_i) - theta(p_old_i)).
        let v = self.grid.cell_volume(i);
        let acc = v / self.dt * (self.theta(p[i]) - self.theta(self.p_old[i]));

        let mut flux = 0.0;

        // Internal faces incident to cell i.
        for &f in &self.cell_faces[i] {
            let c = &self.grid.connections()[f];
            let (o, n) = (c.owner, c.neighbour);
            let m = self.two_point_flux(p[o], self.z[o], p[n], self.z[n], c.geometric_transmissibility);
            // Positive m leaves the owner and enters the neighbour.
            if i == o {
                flux += m;
            } else {
                flux -= m;
            }
        }

        // Boundary faces.
        for &b in &self.cell_boundary_faces[i] {
            let bf = &self.grid.boundary_faces()[b];
            let li = location_index(&bf.location);
            match &self.bc[li] {
                None => {} // no-flow (natural zero Neumann)
                Some(BoundaryConditionKind::NeumannFlux(q)) => {
                    // q positive = inflow (m/s Darcy); mass in = rho * q * area,
                    // which lowers the residual (outflow-positive convention).
                    flux -= self.rho(p[i]) * q * bf.area;
                }
                Some(BoundaryConditionKind::DirichletPressure(p_bc)) => {
                    // Ghost cell at the boundary-face centroid.
                    let z_bc = match bf.location {
                        BoundaryLocation::ZMin => self.z[i] - bf.distance,
                        BoundaryLocation::ZMax => self.z[i] + bf.distance,
                        _ => self.z[i],
                    };
                    let t_geom = bf.area / bf.distance;
                    let m = self.two_point_flux(p[i], self.z[i], *p_bc, z_bc, t_geom);
                    flux += m; // positive leaves the cell
                }
            }
        }

        acc + flux - self.source[i]
    }
}

impl NonlinearSystem for RichardsProblem {
    fn n_dof(&self) -> usize {
        self.grid.n_cells()
    }

    fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
        self.grid.ldu_addressing()
    }

    fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
        if x.len() != self.grid.n_cells() || out.len() != self.grid.n_cells() {
            return Err(PflotranError::InvalidInput(
                "residual: state/output length mismatch with grid".into(),
            ));
        }
        for i in 0..self.grid.n_cells() {
            out[i] = self.cell_residual(i, x);
        }
        Ok(())
    }

    fn assemble_jacobian(&mut self, x: &[f64], jac: &mut LduMatrix) -> Result<(), PflotranError> {
        let n = self.grid.n_cells();
        if x.len() != n {
            return Err(PflotranError::InvalidInput(
                "jacobian: state length mismatch with grid".into(),
            ));
        }

        // Base residuals over all cells.
        let mut r0 = vec![0.0; n];
        for i in 0..n {
            r0[i] = self.cell_residual(i, x);
        }

        for v in jac.diag.iter_mut() {
            *v = 0.0;
        }
        for v in jac.lower.iter_mut() {
            *v = 0.0;
        }
        for v in jac.upper.iter_mut() {
            *v = 0.0;
        }

        // Local finite-difference columns: perturbing cell j only affects the
        // residual of j and its face-neighbours (the two-point stencil).
        let mut xp = x.to_vec();
        for j in 0..n {
            let h = 1.0e-8 * (1.0 + x[j].abs());
            xp[j] = x[j] + h;

            jac.diag[j] = (self.cell_residual(j, &xp) - r0[j]) / h;

            for &f in &self.cell_faces[j] {
                let c = &self.grid.connections()[f];
                if j == c.owner {
                    // dR_neighbour / dp_owner  -> lower[f]
                    jac.lower[f] = (self.cell_residual(c.neighbour, &xp) - r0[c.neighbour]) / h;
                } else {
                    // dR_owner / dp_neighbour  -> upper[f]
                    jac.upper[f] = (self.cell_residual(c.owner, &xp) - r0[c.owner]) / h;
                }
            }

            xp[j] = x[j];
        }

        if jac.diag.iter().any(|v| !v.is_finite()) {
            return Err(PflotranError::Convergence(
                "non-finite Jacobian diagonal (check curves near saturation)".into(),
            ));
        }
        Ok(())
    }
}

/// Timestep controller for a transient RICHARDS run.
///
/// All times in seconds. `dt` grows by `growth_factor` after a converged step
/// (capped at `max_dt`) and is cut by `cut_factor` on a failed nonlinear solve,
/// aborting if it would fall below `min_dt`.
pub struct TimeControls {
    /// Simulation end time (s).
    pub final_time: f64,
    /// Initial timestep (s).
    pub initial_dt: f64,
    /// Maximum timestep (s).
    pub max_dt: f64,
    /// Minimum timestep (s) before the run gives up.
    pub min_dt: f64,
    /// Multiplicative growth factor on a converged step (> 1).
    pub growth_factor: f64,
    /// Multiplicative cut factor on a failed step (in (0,1)).
    pub cut_factor: f64,
}

impl TimeControls {
    /// Controls from a parsed deck's `TIME` card, with sensible growth/cut
    /// factors (1.5 / 0.5) and `min_dt = initial_dt * 1e-6`.
    pub fn from_time_spec(final_time: f64, initial_dt: f64, max_dt: f64) -> Self {
        Self {
            final_time,
            initial_dt,
            max_dt,
            min_dt: initial_dt * 1.0e-6,
            growth_factor: 1.5,
            cut_factor: 0.5,
        }
    }
}

/// Outcome of a single [`RichardsSimulation::step`].
#[derive(Debug, Clone, Copy)]
pub struct StepReport {
    /// Simulation time (s) reached at the end of this step.
    pub time: f64,
    /// The timestep size (s) actually taken (after any cutting).
    pub dt: f64,
    /// Newton iterations used by the accepted solve.
    pub newton_iterations: usize,
    /// Whether the step converged (always true on `Ok`).
    pub converged: bool,
}

/// Summary of a full [`RichardsSimulation::run`].
#[derive(Debug, Clone, Copy)]
pub struct RunReport {
    /// Number of accepted timesteps.
    pub steps: usize,
    /// Final simulation time reached (s).
    pub final_time: f64,
    /// Total Newton iterations across all accepted steps.
    pub total_newton_iterations: usize,
}

/// A transient RICHARDS simulation: a [`RichardsProblem`] plus a
/// [`NewtonSolver`], time controls, and the evolving pressure field.
pub struct RichardsSimulation {
    problem: RichardsProblem,
    solver: NewtonSolver,
    time: TimeControls,
    current_time: f64,
    dt: f64,
    pressure: Vec<f64>,
    step_count: usize,
    total_newton_iterations: usize,
}

impl RichardsSimulation {
    /// Assemble a simulation from a problem, Newton configuration, time
    /// controls, and a uniform initial liquid pressure (Pa).
    pub fn new(
        problem: RichardsProblem,
        newton: NewtonConfig,
        time: TimeControls,
        initial_pressure: f64,
    ) -> Self {
        let n = problem.n_dof();
        let dt = time.initial_dt;
        Self {
            problem,
            solver: NewtonSolver::new(newton),
            time,
            current_time: 0.0,
            dt,
            pressure: vec![initial_pressure; n],
            step_count: 0,
            total_newton_iterations: 0,
        }
    }

    /// Build a runnable simulation directly from a parsed input deck, wiring the
    /// grid, material, characteristic curves, boundary conditions, and time
    /// controls. Uses liquid-water EOS defaults, standard gravity, and
    /// atmospheric reference gas pressure.
    pub fn from_input_deck(deck: &InputDeck) -> Result<Self, PflotranError> {
        let g = &deck.grid;
        let grid = CartesianGrid::uniform(
            g.nx,
            g.ny,
            g.nz,
            Length::new::<meter>(g.dx),
            Length::new::<meter>(g.dy),
            Length::new::<meter>(g.dz),
        )?;

        let curves = match deck.curves.model {
            CurveModel::VanGenuchten => CharacteristicCurves::VanGenuchten(VanGenuchten::new(
                deck.curves.alpha,
                deck.curves.n_or_lambda,
                deck.curves.residual_saturation,
            )?),
            CurveModel::BrooksCorey => CharacteristicCurves::BrooksCorey(BrooksCorey::new(
                deck.curves.alpha,
                deck.curves.n_or_lambda,
                deck.curves.residual_saturation,
            )?),
        };

        let boundary = deck
            .boundary_conditions
            .iter()
            .map(|bc| BoundaryCondition {
                location: match bc.location {
                    BoundaryLocationSpec::XMin => BoundaryLocation::XMin,
                    BoundaryLocationSpec::XMax => BoundaryLocation::XMax,
                    BoundaryLocationSpec::YMin => BoundaryLocation::YMin,
                    BoundaryLocationSpec::YMax => BoundaryLocation::YMax,
                    BoundaryLocationSpec::ZMin => BoundaryLocation::ZMin,
                    BoundaryLocationSpec::ZMax => BoundaryLocation::ZMax,
                },
                kind: match bc.kind {
                    BoundaryKindSpec::DirichletPressure(p) => {
                        BoundaryConditionKind::DirichletPressure(p)
                    }
                    BoundaryKindSpec::NeumannFlux(q) => BoundaryConditionKind::NeumannFlux(q),
                },
            })
            .collect();

        let problem = RichardsProblem::new(
            grid,
            LiquidWaterEos::water(),
            curves,
            deck.material.porosity,
            deck.material.permeability,
            STANDARD_GRAVITY,
            ATMOSPHERIC_PRESSURE,
            boundary,
        )?;

        let time = TimeControls::from_time_spec(
            deck.time.final_time,
            deck.time.initial_dt,
            deck.time.max_dt,
        );

        Ok(Self::new(
            problem,
            NewtonConfig::default(),
            time,
            deck.initial_pressure,
        ))
    }

    /// Current liquid-pressure field (Pa), length `n_cells`.
    pub fn pressure(&self) -> &[f64] {
        &self.pressure
    }

    /// Current liquid-saturation field `S_l` (dimensionless), length `n_cells`.
    pub fn saturation(&self) -> Vec<f64> {
        (0..self.pressure.len())
            .map(|i| self.problem.saturation(self.pressure[i]))
            .collect()
    }

    /// Simulation time reached so far (s).
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// The underlying problem (read-only), e.g. for grid access.
    pub fn problem(&self) -> &RichardsProblem {
        &self.problem
    }

    /// Total fluid mass currently in the domain (kg). See
    /// [`RichardsProblem::total_mass`].
    pub fn total_mass(&self) -> f64 {
        self.problem.total_mass(&self.pressure)
    }

    /// Advance by one adaptive timestep. Cuts `dt` and retries on a failed
    /// nonlinear solve; errors only if `dt` would fall below `min_dt`.
    pub fn step(&mut self) -> Result<StepReport, PflotranError> {
        if self.current_time >= self.time.final_time {
            return Ok(StepReport {
                time: self.current_time,
                dt: 0.0,
                newton_iterations: 0,
                converged: true,
            });
        }
        loop {
            let dt = self.dt.min(self.time.final_time - self.current_time);
            self.problem.set_timestep(dt);
            self.problem.set_previous(&self.pressure);

            let mut trial = self.pressure.clone();
            match self.solver.solve(&mut self.problem, &mut trial) {
                Ok(report) => {
                    self.pressure = trial;
                    self.current_time += dt;
                    self.step_count += 1;
                    self.total_newton_iterations += report.iterations;
                    self.dt = (dt * self.time.growth_factor).min(self.time.max_dt);
                    return Ok(StepReport {
                        time: self.current_time,
                        dt,
                        newton_iterations: report.iterations,
                        converged: true,
                    });
                }
                Err(_) => {
                    self.dt = dt * self.time.cut_factor;
                    if self.dt < self.time.min_dt {
                        return Err(PflotranError::Convergence(format!(
                            "timestep cut below min_dt={} at t={}",
                            self.time.min_dt, self.current_time
                        )));
                    }
                }
            }
        }
    }

    /// Run to the configured final time, returning the run summary.
    pub fn run(&mut self) -> Result<RunReport, PflotranError> {
        // Guard against a pathological non-advancing loop.
        let max_steps = 10_000_000usize;
        while self.current_time < self.time.final_time * (1.0 - 1.0e-12) {
            self.step()?;
            if self.step_count > max_steps {
                return Err(PflotranError::Convergence(
                    "exceeded maximum step count".into(),
                ));
            }
        }
        Ok(RunReport {
            steps: self.step_count,
            final_time: self.current_time,
            total_newton_iterations: self.total_newton_iterations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::{LinearSolverKind, PreconditionerKind};
    use outram_foam_basic_lib::krylov::KrylovSettings;

    /// A saturated, gravity-free 1D column with Dirichlet ends must reach a
    /// LINEAR steady-state pressure profile — a closed-form verification of the
    /// two-point-flux spatial operator (Laplace's equation, constant mobility).
    #[test]
    fn saturated_1d_steady_state_is_linear() {
        let nx = 11usize;
        let dx = 0.1;
        let grid =
            CartesianGrid::uniform(nx, 1, 1, Length::new::<meter>(dx), Length::new::<meter>(1.0), Length::new::<meter>(1.0))
                .unwrap();
        // Fully saturated everywhere: keep pressures well above the reference
        // gas pressure so Se = 1, k_r = 1.
        let p_gas = 0.0; // gauge; p > 0 => saturated
        let curves = CharacteristicCurves::VanGenuchten(VanGenuchten::new(1.0e-4, 2.0, 0.0).unwrap());
        let p_left = 2.0e5;
        let p_right = 1.0e5;
        let boundary = vec![
            BoundaryCondition {
                location: BoundaryLocation::XMin,
                kind: BoundaryConditionKind::DirichletPressure(p_left),
            },
            BoundaryCondition {
                location: BoundaryLocation::XMax,
                kind: BoundaryConditionKind::DirichletPressure(p_right),
            },
        ];
        let mut problem = RichardsProblem::new(
            grid,
            LiquidWaterEos::water(),
            curves,
            0.35,
            1.0e-12,
            0.0, // no gravity
            p_gas,
            boundary,
        )
        .unwrap();

        // Steady state: drive dt -> infinity so accumulation vanishes.
        problem.set_timestep(1.0e30);
        let n = problem.n_dof();
        problem.set_previous(&vec![1.5e5; n]);

        let cfg = NewtonConfig {
            max_iterations: 50,
            abs_tol: 1.0e-6,
            rel_tol: 1.0e-10,
            linear: LinearSolverKind::BiCGStab,
            preconditioner: PreconditionerKind::Ilu0,
            linear_settings: KrylovSettings { tolerance: 1.0e-12, max_iter: 2000, restart: 50 },
            max_backtracks: 10,
        };
        let solver = NewtonSolver::new(cfg);
        let mut p = vec![1.5e5; n];
        solver.solve(&mut problem, &mut p).expect("steady solve converges");

        // Expected linear profile at cell centres x_i = (i+0.5)*dx over L=nx*dx.
        let length = nx as f64 * dx;
        for i in 0..nx {
            let x = (i as f64 + 0.5) * dx;
            let expected = p_left + (p_right - p_left) * (x / length);
            assert!(
                (p[i] - expected).abs() < 5.0,
                "cell {i}: got {}, expected {} (linear profile)",
                p[i],
                expected
            );
        }
    }

    /// **Mass conservation** (Celia's mixed-form invariant): a closed no-flow
    /// domain with NO source but a NON-uniform initial pressure actively
    /// redistributes fluid, yet total mass must be conserved to solver
    /// tolerance across many implicit steps.
    #[test]
    fn closed_domain_conserves_total_mass() {
        let n = 12usize;
        let grid = CartesianGrid::uniform(
            n,
            1,
            1,
            Length::new::<meter>(0.05),
            Length::new::<meter>(1.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let curves =
            CharacteristicCurves::VanGenuchten(VanGenuchten::new(2.0e-4, 2.0, 0.05).unwrap());
        let mut problem = RichardsProblem::new(
            grid,
            LiquidWaterEos::water(),
            curves,
            0.35,
            1.0e-12,
            0.0,
            ATMOSPHERIC_PRESSURE,
            vec![], // all no-flow
        )
        .unwrap();

        // Non-uniform initial field (a pressure ramp) drives redistribution.
        let mut p: Vec<f64> = (0..n).map(|i| 80_000.0 + 2_000.0 * i as f64).collect();
        let m0 = problem.total_mass(&p);

        let solver = NewtonSolver::new(NewtonConfig::default());
        problem.set_timestep(300.0);
        for _ in 0..8 {
            problem.set_previous(&p);
            let mut trial = p.clone();
            solver
                .solve(&mut problem, &mut trial)
                .expect("closed-box step converges");
            p = trial;
        }
        let m1 = problem.total_mass(&p);
        // Field must actually have moved (otherwise the test is vacuous).
        let moved = (0..n).map(|i| (p[i] - (80_000.0 + 2_000.0 * i as f64)).abs()).fold(0.0, f64::max);
        assert!(moved > 1.0, "field did not redistribute (moved={moved})");
        let rel = (m1 - m0).abs() / m0;
        assert!(rel < 1.0e-8, "mass not conserved: rel drift {rel:e} (m0={m0}, m1={m1})");
    }

    /// The Haverkamp (Celia 1990) sand model must drive a real RICHARDS solve:
    /// a dry column with a saturated inflow face wets up monotonically and stays
    /// physically bounded. (End-to-end integration of bead op-v6s.9.1; the full
    /// benchmark comparison is op-v6s.9.2/.9.3.)
    #[test]
    fn haverkamp_sand_column_runs_and_wets_up() {
        use crate::properties::Haverkamp;
        let grid = CartesianGrid::uniform(
            15,
            1,
            1,
            Length::new::<meter>(0.02),
            Length::new::<meter>(1.0),
            Length::new::<meter>(1.0),
        )
        .unwrap();
        let curves = CharacteristicCurves::Haverkamp(Haverkamp::celia_1977_sand());
        // Permeability from the Celia saturated conductivity Ks = 0.00944 cm/s:
        // k = Ks * mu / (rho g) with Ks in m/s.
        let ks = 0.00944e-2; // m/s
        let k = ks * 1.0e-3 / (1000.0 * STANDARD_GRAVITY);
        let boundary = vec![BoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: BoundaryConditionKind::DirichletPressure(ATMOSPHERIC_PRESSURE),
        }];
        let problem = RichardsProblem::new(
            grid,
            LiquidWaterEos::water(),
            curves,
            0.287,
            k,
            0.0,
            ATMOSPHERIC_PRESSURE,
            boundary,
        )
        .unwrap();
        // Dry initial state: pc = 5325 Pa (|psi| ~ 54 cm) -> low saturation.
        let mut sim = RichardsSimulation::new(
            problem,
            NewtonConfig::default(),
            TimeControls::from_time_spec(600.0, 5.0, 60.0),
            ATMOSPHERIC_PRESSURE - 5325.0,
        );
        let s0 = sim.saturation();
        sim.run().expect("Haverkamp column runs");
        let s = sim.saturation();
        for &v in &s {
            assert!((0.0..=1.0).contains(&v), "saturation out of bounds: {v}");
        }
        assert!(s[0] > s0[0] + 1e-3, "inflow cell did not wet up");
        for w in s.windows(2) {
            assert!(w[1] <= w[0] + 1e-6, "saturation not monotone along x");
        }
    }

    /// A closed (no-flow) box with a uniform initial pressure must stay put:
    /// zero flux everywhere, so every step leaves the field unchanged.
    #[test]
    fn closed_box_uniform_field_is_stationary() {
        let grid = CartesianGrid::uniform(
            4,
            3,
            2,
            Length::new::<meter>(0.25),
            Length::new::<meter>(0.25),
            Length::new::<meter>(0.25),
        )
        .unwrap();
        let curves = CharacteristicCurves::VanGenuchten(VanGenuchten::new(1.0e-4, 2.0, 0.1).unwrap());
        let problem = RichardsProblem::new(
            grid,
            LiquidWaterEos::water(),
            curves,
            0.3,
            1.0e-12,
            0.0,
            ATMOSPHERIC_PRESSURE,
            vec![], // all no-flow
        )
        .unwrap();
        let p0 = 9.0e4;
        let mut sim = RichardsSimulation::new(
            problem,
            NewtonConfig::default(),
            TimeControls::from_time_spec(3600.0, 60.0, 600.0),
            p0,
        );
        let report = sim.step().unwrap();
        assert!(report.converged);
        for &p in sim.pressure() {
            assert!((p - p0).abs() < 1.0e-3, "closed box drifted: {p} vs {p0}");
        }
    }
}
