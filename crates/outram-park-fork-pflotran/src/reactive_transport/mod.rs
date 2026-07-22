//! Reactive transport (v1) — operator-split (SNIA) coupling of solute
//! [`transport`](crate::transport) and equilibrium
//! [`geochemistry`](crate::geochemistry). Bead op-v6s.12.
//!
//! Multi-component reactive transport by **sequential non-iterative operator
//! splitting** (SNIA). Each time step advances `Nc` aqueous component *total*
//! concentrations in two decoupled sub-steps:
//!
//! 1. **Transport** — each component's total concentration is advected and
//!    dispersed by the same frozen flow field, exactly as in the single-solute
//!    [`crate::transport`] solver. Because the advection–dispersion operator
//!    depends only on the flow field (not on the species), the same implicit-
//!    Euler matrix `A` is assembled **once** and reused: for component `k` we
//!    solve `A * total_k = b_k`, changing only the right-hand side `b_k` (its
//!    accumulation term uses component `k`'s old totals, and its boundary term
//!    uses component `k`'s inflow total).
//! 2. **Reaction** — each cell is then re-speciated to instantaneous chemical
//!    equilibrium with [`ReactionNetwork::speciate`], redistributing each
//!    component's transported total over its free primary and its secondary
//!    (complexed) species. Equilibrium speciation conserves every component
//!    total by construction (mass balance), so the reaction sub-step changes the
//!    *speciation* of a cell but not its transported totals.
//!
//! # Governing form
//!
//! For component `k` with total aqueous concentration `T_k` (mol/L), water
//! content `theta_w`, Darcy flux `q`, and effective dispersion `D`:
//!
//! ```text
//! d/dt(theta_w T_k) + div(q T_k) - div(theta_w D grad T_k) = R_k
//! ```
//!
//! SNIA solves the transport terms (left side, `R_k = 0`) implicitly for the new
//! `T_k`, then applies the reaction operator (equilibrium re-speciation) as a
//! cell-local correction. The two are not iterated to convergence within a step.
//!
//! # Assumptions and simplifications (flagged for human review)
//!
//! - **SNIA splitting error is O(dt).** Because transport and reaction are
//!   applied sequentially without sub-iteration, the operator-splitting error is
//!   first order in the time step. Sharp reaction fronts require a small `dt`;
//!   this is a documented accuracy limit, not a bug. A Strang-split (O(dt^2)) or
//!   a global-implicit (GIRT) variant is deferred.
//! - **All species are aqueous and mobile.** Every component total is
//!   transported by the *same* advection–dispersion operator with the *same*
//!   dispersion coefficient — i.e. all mass is dissolved and moves with the
//!   water. Immobile phases (sorbed species, minerals) and species-specific
//!   diffusion coefficients are **not** modelled; adding minerals would make
//!   part of a component's total immobile and is deferred with the geochemistry
//!   mineral phases.
//! - **Equilibrium chemistry only**, inheriting every simplification of
//!   [`crate::geochemistry`] (ideal activities `gamma = 1`, no kinetics, no
//!   charge-balance constraint, no water activity).
//! - **First-order upwind advection**, inheriting the numerical cross-wind
//!   diffusion `~ |v| dx / 2` of the [`crate::transport`] scheme.
//!
//! # Units
//!
//! Concentrations (component totals, free primaries, secondaries) are mol/L;
//! volumetric fluxes are m^3/s; water content is dimensionless; the dispersion
//! coefficient is m^2/s; component "mass" reported by
//! [`ReactiveTransport::component_mass`] is in mol (a volume-integrated amount,
//! `sum_i V_i theta_w_i T_i`); time steps are seconds. Plain `f64` is used (not
//! `uom`) for the same reason as [`crate::transport`]: the solve mixes
//! quantities of differing dimension and callers apply units at case setup.
//!
//! Enum dispatch throughout, no trait objects, per the workspace design rules.

use crate::error::PflotranError;
use crate::geochemistry::{ReactionNetwork, Speciation};
use crate::grid::{BoundaryLocation, CartesianGrid};
use crate::transport::FlowField;
use outram_foam_basic_lib::krylov::{bicgstab, KrylovSettings, Preconditioner};
use outram_foam_basic_lib::ldu_matrix::LduMatrix;

/// A reactive (multi-component) boundary condition bound to one of the six
/// domain-box face locations.
///
/// It fixes the inflow **total** concentration of every component at that face.
/// The advective part is upwinded by the boundary flux sign (inflow carries the
/// specified totals into the domain; outflow carries the interior totals out) and
/// the dispersive part couples the near-boundary cell to the specified totals
/// across the half-cell distance, so — like
/// [`crate::transport::TransportBoundaryKind::InflowConcentration`] — this acts
/// as a Dirichlet condition on each component total at an inflow or an outflow
/// face. A face location with no [`ReactiveBoundaryCondition`] uses the default
/// advective-outflow / zero-gradient behaviour for every component.
pub struct ReactiveBoundaryCondition {
    /// Which exterior face location this condition applies to.
    pub location: BoundaryLocation,
    /// The fixed inflow total concentration of each component, mol/L. Length must
    /// equal the number of components `Nc = network.n_primary()`; column `k`
    /// is component `k`'s boundary total.
    pub inflow_totals: Vec<f64>,
}

/// Per-boundary-face precomputed data for the (component-independent) part of the
/// right-hand-side boundary term.
///
/// For a face carrying a [`ReactiveBoundaryCondition`], the contribution to
/// `b[cell]` for component `k` is `factor * inflow_totals[k]`, where `factor`
/// gathers the advective inflow term (`-q` when `q < 0`) and the always-applied
/// dispersive Dirichlet coupling `d_b`. Storing `factor` once (it does not depend
/// on the component) lets each per-component RHS assembly be a single multiply.
struct BoundaryRhsTerm {
    /// The near-boundary cell index.
    cell: usize,
    /// The `0..6` location index used to look up the boundary's `inflow_totals`.
    loc_index: usize,
    /// The component-independent RHS multiplier `factor` (see the struct doc).
    factor: f64,
}

/// One SNIA reactive-transport stepper over a frozen flow field.
///
/// Holds the grid, the frozen [`FlowField`], the scalar dispersion parameters
/// (`molecular_diffusion` and `longitudinal_dispersivity`, shared by every
/// component), the [`ReactionNetwork`], the per-location boundary totals, the
/// time step `dt` (s), the current component totals `totals[cell][component]`
/// (mol/L), and the last equilibrium free-primary concentrations
/// `last_primary[cell][component]` (mol/L, used to warm-start each cell's Newton
/// speciation). [`ReactiveTransport::step`] transports every component total then
/// re-speciates every cell.
pub struct ReactiveTransport {
    grid: CartesianGrid,
    flow: FlowField,
    /// Molecular diffusion coefficient `D_mol`, m^2/s (`>= 0`).
    molecular_diffusion: f64,
    /// Longitudinal dispersivity `alpha_L`, m (`>= 0`).
    longitudinal_dispersivity: f64,
    network: ReactionNetwork,
    /// Boundary inflow totals indexed by the six face locations. `None` slots use
    /// the default (advective-outflow / zero-gradient) behaviour. Each `Some`
    /// vector has length `Nc`.
    bc_by_location: [Option<Vec<f64>>; 6],
    /// Time step, seconds (`> 0`, validated at [`ReactiveTransport::step`]).
    dt: f64,
    /// Current component totals, `n_cells` rows by `Nc` cols, mol/L.
    totals: Vec<Vec<f64>>,
    /// Last equilibrium free-primary concentrations, `n_cells` rows by `Nc` cols,
    /// mol/L. Seeded from the initial totals and updated by each re-speciation;
    /// used only as a Newton warm-start (never as a physical output).
    last_primary: Vec<Vec<f64>>,
}

impl ReactiveTransport {
    /// Build a reactive-transport stepper.
    ///
    /// # Parameters
    ///
    /// - `grid`: the structured Cartesian grid.
    /// - `flow`: the frozen [`FlowField`] (face/boundary Darcy fluxes and water
    ///   content), indexed identically to `grid`.
    /// - `molecular_diffusion`: `D_mol`, m^2/s, finite and `>= 0`. Shared by all
    ///   components (all-aqueous, single-diffusivity assumption).
    /// - `longitudinal_dispersivity`: `alpha_L`, m, finite and `>= 0`. The face
    ///   dispersion coefficient is `D_mol + alpha_L * |v_darcy|`.
    /// - `network`: the equilibrium [`ReactionNetwork`]; its primary count
    ///   `Nc = network.n_primary()` fixes the number of transported components.
    /// - `boundary`: reactive boundary conditions; each fixes every component's
    ///   inflow total at one face location. A later entry for a location
    ///   overrides an earlier one.
    /// - `initial_totals`: initial component totals, `n_cells` rows each of
    ///   length `Nc`, mol/L.
    ///
    /// The time step defaults to `1.0` s (override with
    /// [`ReactiveTransport::set_timestep`]).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any flow-field vector has the
    /// wrong length, if any water content is non-finite or negative, if any flux
    /// is non-finite, if `molecular_diffusion` / `longitudinal_dispersivity` is
    /// non-finite or negative, if any boundary `inflow_totals` has length `!= Nc`
    /// or holds a non-finite value, or if `initial_totals` has the wrong number
    /// of rows, a row of the wrong length, or a non-finite entry.
    pub fn new(
        grid: CartesianGrid,
        flow: FlowField,
        molecular_diffusion: f64,
        longitudinal_dispersivity: f64,
        network: ReactionNetwork,
        boundary: Vec<ReactiveBoundaryCondition>,
        initial_totals: Vec<Vec<f64>>,
    ) -> Result<Self, PflotranError> {
        let n_cells = grid.n_cells();
        let n_int = grid.connections().len();
        let n_bnd = grid.boundary_faces().len();
        let nc = network.n_primary();

        // Flow-field shape and finiteness (mirrors crate::transport).
        if flow.face_flux.len() != n_int {
            return Err(PflotranError::InvalidInput(format!(
                "flow.face_flux has {} entries but the grid has {n_int} internal faces",
                flow.face_flux.len()
            )));
        }
        if flow.boundary_flux.len() != n_bnd {
            return Err(PflotranError::InvalidInput(format!(
                "flow.boundary_flux has {} entries but the grid has {n_bnd} boundary faces",
                flow.boundary_flux.len()
            )));
        }
        if flow.water_content.len() != n_cells {
            return Err(PflotranError::InvalidInput(format!(
                "flow.water_content has {} entries but the grid has {n_cells} cells",
                flow.water_content.len()
            )));
        }
        for (i, &tw) in flow.water_content.iter().enumerate() {
            if !tw.is_finite() || tw < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "flow.water_content[{i}] = {tw} must be finite and >= 0"
                )));
            }
        }
        for (f, &q) in flow.face_flux.iter().enumerate() {
            if !q.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "flow.face_flux[{f}] = {q} is not finite"
                )));
            }
        }
        for (b, &q) in flow.boundary_flux.iter().enumerate() {
            if !q.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "flow.boundary_flux[{b}] = {q} is not finite"
                )));
            }
        }

        // Dispersion parameters.
        if !molecular_diffusion.is_finite() || molecular_diffusion < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "molecular_diffusion = {molecular_diffusion} must be finite and >= 0 (m^2/s)"
            )));
        }
        if !longitudinal_dispersivity.is_finite() || longitudinal_dispersivity < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "longitudinal_dispersivity = {longitudinal_dispersivity} must be finite and >= 0 (m)"
            )));
        }

        // Boundary conditions: one inflow-total vector (length Nc) per location.
        let mut bc_by_location: [Option<Vec<f64>>; 6] = Default::default();
        for bc in &boundary {
            if bc.inflow_totals.len() != nc {
                return Err(PflotranError::InvalidInput(format!(
                    "ReactiveBoundaryCondition on {:?} has {} inflow totals but there are {nc} components",
                    bc.location,
                    bc.inflow_totals.len()
                )));
            }
            for (k, &c) in bc.inflow_totals.iter().enumerate() {
                if !c.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "ReactiveBoundaryCondition on {:?} has non-finite inflow total[{k}] = {c}",
                        bc.location
                    )));
                }
            }
            bc_by_location[location_index(bc.location)] = Some(bc.inflow_totals.clone());
        }

        // Initial totals.
        if initial_totals.len() != n_cells {
            return Err(PflotranError::InvalidInput(format!(
                "initial_totals has {} rows but the grid has {n_cells} cells",
                initial_totals.len()
            )));
        }
        for (i, row) in initial_totals.iter().enumerate() {
            if row.len() != nc {
                return Err(PflotranError::InvalidInput(format!(
                    "initial_totals[{i}] has {} entries but there are {nc} components",
                    row.len()
                )));
            }
            for (k, &t) in row.iter().enumerate() {
                if !t.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "initial_totals[{i}][{k}] = {t} is not finite"
                    )));
                }
            }
        }

        let last_primary = initial_totals.clone();

        Ok(Self {
            grid,
            flow,
            molecular_diffusion,
            longitudinal_dispersivity,
            network,
            bc_by_location,
            dt: 1.0,
            totals: initial_totals,
            last_primary,
        })
    }

    /// Set the time step `dt` (seconds). Validated (must be positive and finite)
    /// at [`ReactiveTransport::step`].
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Number of grid cells (the size of each per-component linear system).
    pub fn n_cells(&self) -> usize {
        self.grid.n_cells()
    }

    /// Number of transported components `Nc` (the reaction network's primary
    /// species count).
    pub fn n_components(&self) -> usize {
        self.network.n_primary()
    }

    /// Effective face dispersion coefficient `D_mol + alpha_L * v_darcy` (m^2/s)
    /// for a Darcy velocity magnitude `v_darcy` (m/s, `>= 0`). Shared by every
    /// component (all-aqueous, single-diffusivity assumption).
    fn dispersion_coefficient(&self, v_darcy: f64) -> f64 {
        self.molecular_diffusion + self.longitudinal_dispersivity * v_darcy
    }

    /// Advance one SNIA step: transport every component total through the shared
    /// advection–dispersion operator, then re-speciate every cell to equilibrium.
    ///
    /// # Assembly (shared operator, per-component RHS)
    ///
    /// The implicit-Euler matrix `A` is identical for all components (it depends
    /// only on the flow field, water content, geometry, and `dt`), so it is
    /// assembled once. With owner `o`, neighbour `n`, signed internal flux `q_f`
    /// (`+` = `o -> n`), and geometric transmissibility `T = area / distance`:
    ///
    /// - Accumulation: `diag[i] += theta_w_i V_i / dt`; RHS for component `k`
    ///   gets `b_k[i] += theta_w_i V_i / dt * total_old_k[i]`.
    /// - Advection (upwind): if `q_f >= 0`, `diag[o] += q_f`, `lower[f] -= q_f`;
    ///   else `upper[f] += q_f`, `diag[n] -= q_f`.
    /// - Dispersion: `d = D_face * theta_w_face * T` with `theta_w_face =
    ///   0.5(theta_w_o + theta_w_n)` and `D_face = D_mol + alpha_L |q_f| / area`;
    ///   then `diag[o] += d`, `upper[f] -= d`, `diag[n] += d`, `lower[f] -= d`.
    /// - Boundary face (cell `i`, signed `q_b`, `+` = outflow): a face with no
    ///   condition adds `diag[i] += q_b`. A [`ReactiveBoundaryCondition`] adds the
    ///   component-independent advective diagonal (`diag[i] += q_b` when `q_b >=
    ///   0`) plus the dispersive coupling `d_b = D_face theta_w_i area / distance`
    ///   (`diag[i] += d_b`), and — component-dependently — the RHS term `b_k[i] +=
    ///   factor * inflow_totals_k` with `factor = d_b` (`+ (-q_b)` when `q_b < 0`).
    ///
    /// Each component's system `A * total_k = b_k` is solved with BiCGStab +
    /// ILU(0) (the preconditioner is factorised once from the shared `A`).
    ///
    /// After transport, each cell whose component totals are all finite and
    /// strictly positive is re-speciated with [`ReactionNetwork::speciate`],
    /// warm-started from that cell's previous free-primary concentrations;
    /// equilibrium speciation leaves the transported totals unchanged. Cells with
    /// a non-positive total (e.g. an as-yet-unreached, solute-free cell) are
    /// skipped — there is no positive-concentration equilibrium for them — and
    /// their totals are still transported and conserved.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `dt` is non-positive or
    /// non-finite, or [`PflotranError::Convergence`] if any component's Krylov
    /// solve fails to converge. A failed cell re-speciation does not abort the
    /// step (the cell keeps its previous free-primary warm-start); query a cell
    /// explicitly with [`ReactiveTransport::speciate_cell`] to surface such a
    /// failure.
    pub fn step(&mut self) -> Result<(), PflotranError> {
        if !self.dt.is_finite() || self.dt <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "time step dt = {} must be finite and > 0 (s)",
                self.dt
            )));
        }

        let n_cells = self.grid.n_cells();
        let nc = self.network.n_primary();
        let (owner, neighbour) = self.grid.ldu_addressing();
        let mut a = LduMatrix::new(n_cells, owner, neighbour);

        // Per-cell accumulation coefficient theta_w_i V_i / dt (shared by all k).
        let inv_dt = 1.0 / self.dt;
        let mut acc = vec![0.0_f64; n_cells];
        for i in 0..n_cells {
            let coeff = self.flow.water_content[i] * self.grid.cell_volume(i) * inv_dt;
            acc[i] = coeff;
            a.diag[i] += coeff;
        }

        // Internal faces: advection (upwind) + dispersion (symmetric two-point).
        let connections = self.grid.connections();
        for (f, conn) in connections.iter().enumerate() {
            let o = conn.owner;
            let n = conn.neighbour;
            let q = self.flow.face_flux[f];

            if q >= 0.0 {
                a.diag[o] += q;
                a.lower[f] -= q;
            } else {
                a.upper[f] += q;
                a.diag[n] -= q;
            }

            let theta_face = 0.5 * (self.flow.water_content[o] + self.flow.water_content[n]);
            let v_darcy = q.abs() / conn.area;
            let d_face = self.dispersion_coefficient(v_darcy);
            let d = d_face * theta_face * conn.geometric_transmissibility;
            a.diag[o] += d;
            a.upper[f] -= d;
            a.diag[n] += d;
            a.lower[f] -= d;
        }

        // Boundary faces: component-independent diagonal contributions into A,
        // and a precomputed RHS `factor` for boundaries carrying a condition.
        let mut bc_rhs: Vec<BoundaryRhsTerm> = Vec::new();
        let boundary_faces = self.grid.boundary_faces();
        for (bidx, face) in boundary_faces.iter().enumerate() {
            let i = face.cell;
            let q = self.flow.boundary_flux[bidx];
            let loc = location_index(face.location);
            match &self.bc_by_location[loc] {
                None => {
                    // Default: advective outflow / zero-gradient inflow.
                    a.diag[i] += q;
                }
                Some(_) => {
                    // Advective diagonal part (outflow carries interior totals out).
                    let mut factor = 0.0;
                    if q < 0.0 {
                        factor += -q; // inflow brings the boundary total in
                    } else {
                        a.diag[i] += q;
                    }
                    // Dispersive Dirichlet coupling to the boundary total (always).
                    let v_darcy = q.abs() / face.area;
                    let d_face = self.dispersion_coefficient(v_darcy);
                    let d_b = d_face * self.flow.water_content[i] * (face.area / face.distance);
                    a.diag[i] += d_b;
                    factor += d_b;

                    bc_rhs.push(BoundaryRhsTerm {
                        cell: i,
                        loc_index: loc,
                        factor,
                    });
                }
            }
        }

        // Factorise the shared preconditioner once; reuse it for every component.
        let precond = Preconditioner::ilu0(&a);
        let settings = KrylovSettings {
            tolerance: 1.0e-12,
            max_iter: 2000,
            restart: 50,
        };

        // Solve A * total_k = b_k for each component, into a fresh totals buffer.
        let mut new_totals = self.totals.clone();
        for k in 0..nc {
            let mut b = vec![0.0_f64; n_cells];
            for i in 0..n_cells {
                b[i] += acc[i] * self.totals[i][k];
            }
            for term in &bc_rhs {
                // Safe: bc_rhs only holds Some-location terms.
                let c_bc = self.bc_by_location[term.loc_index].as_ref().unwrap()[k];
                b[term.cell] += term.factor * c_bc;
            }

            let guess: Vec<f64> = (0..n_cells).map(|i| self.totals[i][k]).collect();
            let (x, result) = bicgstab(&a, &b, Some(guess.as_slice()), &precond, &settings);
            if !result.converged {
                return Err(PflotranError::Convergence(format!(
                    "reactive-transport component {k} linear solve did not converge: {} iters, residual {:.3e}",
                    result.n_iterations, result.final_residual
                )));
            }
            for i in 0..n_cells {
                new_totals[i][k] = x[i];
            }
        }

        // Reaction sub-step: re-speciate each cell to equilibrium. Totals are
        // unchanged by speciation; only the free-primary warm-start is updated.
        let mut new_primary = self.last_primary.clone();
        for i in 0..n_cells {
            let cell_totals = &new_totals[i];
            let positive = cell_totals.iter().all(|t| t.is_finite() && *t > 0.0);
            if !positive {
                continue;
            }
            let guess = &self.last_primary[i];
            let use_guess = guess.iter().all(|g| g.is_finite() && *g > 0.0);
            let init = if use_guess { Some(guess.as_slice()) } else { None };
            if let Ok(sp) = self.network.speciate(cell_totals, init) {
                new_primary[i] = sp.primary;
            }
        }

        self.totals = new_totals;
        self.last_primary = new_primary;
        Ok(())
    }

    /// Current total concentration of one component across all cells (mol/L),
    /// returned as a length-`n_cells` column.
    ///
    /// # Panics
    ///
    /// Panics if `component >= self.n_components()`.
    pub fn totals(&self, component: usize) -> Vec<f64> {
        assert!(
            component < self.n_components(),
            "totals: component {component} out of range 0..{}",
            self.n_components()
        );
        (0..self.grid.n_cells())
            .map(|i| self.totals[i][component])
            .collect()
    }

    /// Solve the equilibrium speciation of one cell from its current component
    /// totals, returning the free-primary and secondary concentrations.
    ///
    /// Warm-started from the cell's stored free-primary concentrations. Unlike the
    /// in-`step` re-speciation (which silently skips non-positive cells), this
    /// surfaces a [`PflotranError`] so a caller can inspect a specific cell.
    ///
    /// # Errors
    ///
    /// Propagates any [`ReactionNetwork::speciate`] error — in particular
    /// [`PflotranError::InvalidInput`] if the cell's totals are not all strictly
    /// positive, or [`PflotranError::Convergence`] if the Newton speciation fails.
    ///
    /// # Panics
    ///
    /// Panics if `cell >= self.n_cells()`.
    pub fn speciate_cell(&self, cell: usize) -> Result<Speciation, PflotranError> {
        assert!(
            cell < self.grid.n_cells(),
            "speciate_cell: cell {cell} out of range 0..{}",
            self.grid.n_cells()
        );
        let guess = &self.last_primary[cell];
        let use_guess = guess.iter().all(|g| g.is_finite() && *g > 0.0);
        let init = if use_guess { Some(guess.as_slice()) } else { None };
        self.network.speciate(&self.totals[cell], init)
    }

    /// Total amount of one component over the whole domain (mol),
    /// `sum_i V_i theta_w_i totals[i][component]`.
    ///
    /// This is the conserved quantity for a closed domain under SNIA: transport
    /// conserves each component total (internal fluxes cancel pairwise) and
    /// equilibrium re-speciation preserves totals by mass balance.
    ///
    /// # Panics
    ///
    /// Panics if `component >= self.n_components()`.
    pub fn component_mass(&self, component: usize) -> f64 {
        assert!(
            component < self.n_components(),
            "component_mass: component {component} out of range 0..{}",
            self.n_components()
        );
        let mut mass = 0.0;
        for i in 0..self.grid.n_cells() {
            mass += self.grid.cell_volume(i) * self.flow.water_content[i] * self.totals[i][component];
        }
        mass
    }
}

/// Map a [`BoundaryLocation`] to a fixed `0..6` index for the BC lookup array.
fn location_index(loc: BoundaryLocation) -> usize {
    match loc {
        BoundaryLocation::XMin => 0,
        BoundaryLocation::XMax => 1,
        BoundaryLocation::YMin => 2,
        BoundaryLocation::YMax => 3,
        BoundaryLocation::ZMin => 4,
        BoundaryLocation::ZMax => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::CartesianGrid;
    use crate::transport::{
        DispersionModel, FlowField as TFlowField, SoluteTransport, TransportBoundaryCondition,
        TransportBoundaryKind,
    };
    use crate::units::GeoLength;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(x: f64) -> GeoLength {
        Length::new::<meter>(x)
    }

    /// A 1D (x-only) uniform grid of `n` cells with unit cross-section, width
    /// `dx`, spanning `[0, n*dx]`.
    fn grid_1d(n: usize, dx: f64) -> CartesianGrid {
        CartesianGrid::uniform(n, 1, 1, m(dx), m(1.0), m(1.0)).unwrap()
    }

    /// Build a uniform +x Darcy flow field (`flux q` on every internal face,
    /// XMin inflow `-q`, XMax outflow `+q`, other boundaries zero, `theta_w = 1`).
    /// A closure builds a fresh value each call since [`FlowField`] is not `Clone`.
    fn uniform_x_flow(grid: &CartesianGrid, q: f64) -> FlowField {
        let n_int = grid.connections().len();
        let mut boundary_flux = vec![0.0; grid.boundary_faces().len()];
        for (b, face) in grid.boundary_faces().iter().enumerate() {
            boundary_flux[b] = match face.location {
                BoundaryLocation::XMin => -q,
                BoundaryLocation::XMax => q,
                _ => 0.0,
            };
        }
        FlowField {
            face_flux: vec![q; n_int],
            boundary_flux,
            water_content: vec![1.0; grid.n_cells()],
        }
    }

    fn uniform_x_flow_t(grid: &CartesianGrid, q: f64) -> TFlowField {
        let n_int = grid.connections().len();
        let mut boundary_flux = vec![0.0; grid.boundary_faces().len()];
        for (b, face) in grid.boundary_faces().iter().enumerate() {
            boundary_flux[b] = match face.location {
                BoundaryLocation::XMin => -q,
                BoundaryLocation::XMax => q,
                _ => 0.0,
            };
        }
        TFlowField {
            face_flux: vec![q; n_int],
            boundary_flux,
            water_content: vec![1.0; grid.n_cells()],
        }
    }

    /// Verification test 1 — **single conservative component reproduces
    /// [`SoluteTransport`] exactly.**
    ///
    /// Methodology: with `Nc = 1` and `Ns = 0` (no secondary species), the
    /// reaction sub-step is the identity (equilibrium leaves the sole total
    /// unchanged), so the reactive stepper must reduce to the passive-scalar
    /// [`SoluteTransport`] solver on the same flow, dispersion, boundary
    /// condition, and time step. A uniform +x advection (`v = 1 m/s`) with
    /// molecular diffusion `D = 0.1 m^2/s` on a 1D grid (`n = 25`, `dx = 0.04 m`),
    /// `InflowConcentration(1.0)` / `inflow_totals = [1.0]` at XMin and default
    /// outflow at XMax, is marched for five steps. Because both solvers assemble
    /// the identical implicit-Euler matrix and RHS and call the identical BiCGStab
    /// + ILU(0) backend, the per-cell totals must agree to the linear-solver
    /// tolerance.
    ///
    /// Results (checked in-test): the maximum per-cell difference stays below
    /// `1e-9` at every step, confirming the multi-component transport assembly is
    /// bit-for-bit the single-solute assembly for one inert component.
    #[test]
    fn single_component_matches_solute_transport() {
        let n = 25;
        let dx = 0.04;
        let v = 1.0;
        let d = 0.1;
        let dt = 0.02;

        let grid = grid_1d(n, dx);

        // Reactive: one component, no chemistry.
        let net = ReactionNetwork::new(vec!["A".into()], vec![], vec![], vec![]).unwrap();
        let react_bc = vec![ReactiveBoundaryCondition {
            location: BoundaryLocation::XMin,
            inflow_totals: vec![1.0],
        }];
        let mut react = ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, v),
            d,
            0.0,
            net,
            react_bc,
            vec![vec![0.0]; n],
        )
        .unwrap();
        react.set_timestep(dt);

        // Reference solute transport.
        let disp = DispersionModel::new(d, 0.0).unwrap();
        let sbc = vec![TransportBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: TransportBoundaryKind::InflowConcentration(1.0),
        }];
        let mut solute =
            SoluteTransport::new(grid.clone(), uniform_x_flow_t(&grid, v), disp, sbc).unwrap();
        solute.set_timestep(dt);

        let mut c = vec![0.0; n];
        for _ in 0..5 {
            react.step().unwrap();
            solute.step(&mut c).unwrap();
            solute.set_previous(&c);

            let totals = react.totals(0);
            let mut max_diff = 0.0_f64;
            for i in 0..n {
                max_diff = max_diff.max((totals[i] - c[i]).abs());
            }
            assert!(
                max_diff < 1e-9,
                "reactive single-component differs from SoluteTransport by {max_diff}"
            );
        }
    }

    /// Verification test 2 — **per-component domain mass conservation on a closed,
    /// reacting domain.**
    ///
    /// Methodology: a closed 3D box (`3 x 3 x 2` cells) with all fluxes zero and
    /// no boundary conditions, so the only transport process is internal
    /// molecular diffusion (`D = 0.2 m^2/s`) which exchanges no mass across the
    /// sealed boundary. A two-primary / one-secondary weak-acid network
    /// (`H+ + Ac- -> HAc`, `log10 K = 4.75`) reacts in every cell. Started from a
    /// cell-varying but strictly positive total field, each component's
    /// volume-integrated amount `sum_i V_i theta_w_i T_i` must be invariant across
    /// steps: transport conserves totals (dispersive fluxes cancel pairwise) and
    /// equilibrium re-speciation preserves totals by mass balance.
    ///
    /// Results (checked in-test): the relative drift of each component's total
    /// domain mass stays below `1e-9` over ten steps, at the Krylov-tolerance
    /// level — confirming the SNIA coupling is conservative in the transported
    /// totals.
    #[test]
    fn closed_domain_conserves_component_mass() {
        let grid = CartesianGrid::uniform(3, 3, 2, m(1.0), m(1.0), m(1.0)).unwrap();
        let n = grid.n_cells();
        let flow = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![0.4; n],
        };
        let net = ReactionNetwork::new(
            vec!["H+".into(), "Ac-".into()],
            vec!["HAc".into()],
            vec![vec![1.0, 1.0]],
            vec![4.75],
        )
        .unwrap();

        // Strictly positive, cell-varying initial totals.
        let initial: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                let a = 0.05 + 0.01 * i as f64;
                let b = 0.04 + 0.008 * i as f64;
                vec![a, b]
            })
            .collect();

        let mut react =
            ReactiveTransport::new(grid, flow, 0.2, 0.0, net, Vec::new(), initial).unwrap();
        react.set_timestep(0.5);

        let mass0: Vec<f64> = (0..react.n_components())
            .map(|k| react.component_mass(k))
            .collect();

        for _ in 0..10 {
            react.step().unwrap();
            for k in 0..react.n_components() {
                let mass = react.component_mass(k);
                let rel = (mass - mass0[k]).abs() / mass0[k].abs();
                assert!(
                    rel < 1e-9,
                    "component {k} mass drift {rel} exceeds 1e-9"
                );
            }
        }
    }

    /// Verification test 3 — **post-step per-cell speciation satisfies mass
    /// balance and mass action.**
    ///
    /// Methodology: the same closed reacting box as test 2 is advanced one step,
    /// then every cell is re-speciated with [`ReactiveTransport::speciate_cell`].
    /// For each cell the returned free-primary and secondary concentrations must
    /// satisfy, for every component `j`, mass balance
    /// `T[j] = C_prim[j] + sum_i stoich[i][j] C_sec[i]` against the cell's
    /// transported totals, and, for every secondary `i`, the law of mass action
    /// `C_sec[i] = 10^{log10 K[i]} prod_j C_prim[j]^{stoich[i][j]}`.
    ///
    /// Results (checked in-test): both residuals hold to `1e-8` in every cell,
    /// confirming the reaction sub-step returns a genuine equilibrium state
    /// consistent with the transported component totals.
    #[test]
    fn post_step_speciation_is_equilibrium() {
        let grid = CartesianGrid::uniform(3, 2, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let n = grid.n_cells();
        let flow = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![0.4; n],
        };
        // Kept locally so the test can verify balance/action without the private
        // network fields.
        let stoich = vec![vec![1.0, 1.0]];
        let log10_k = vec![4.75];
        let net = ReactionNetwork::new(
            vec!["H+".into(), "Ac-".into()],
            vec!["HAc".into()],
            stoich.clone(),
            log10_k.clone(),
        )
        .unwrap();

        let initial: Vec<Vec<f64>> = (0..n)
            .map(|i| vec![0.05 + 0.01 * i as f64, 0.04 + 0.006 * i as f64])
            .collect();

        let mut react =
            ReactiveTransport::new(grid, flow, 0.2, 0.0, net, Vec::new(), initial).unwrap();
        react.set_timestep(0.5);
        react.step().unwrap();

        for cell in 0..n {
            let sp = react.speciate_cell(cell).unwrap();
            // Reconstruct the cell's totals from the two component columns.
            let t0 = react.totals(0)[cell];
            let t1 = react.totals(1)[cell];
            let cell_totals = [t0, t1];

            // Mass balance for each component.
            for j in 0..2 {
                let mut bal = sp.primary[j];
                for (i, row) in stoich.iter().enumerate() {
                    bal += row[j] * sp.secondary[i];
                }
                assert!(
                    (bal - cell_totals[j]).abs() <= 1e-8 + 1e-8 * cell_totals[j].abs(),
                    "cell {cell} component {j}: balance {bal} != total {}",
                    cell_totals[j]
                );
            }
            // Mass action for each secondary.
            for (i, row) in stoich.iter().enumerate() {
                let mut ma = 10f64.powf(log10_k[i]);
                for j in 0..2 {
                    ma *= sp.primary[j].powf(row[j]);
                }
                assert!(
                    (sp.secondary[i] - ma).abs() <= 1e-8 + 1e-8 * ma.abs(),
                    "cell {cell} secondary {i}: {} != mass-action {ma}",
                    sp.secondary[i]
                );
            }
        }
    }

    /// Verification test 4 — **constructor input validation.**
    ///
    /// Rejects mismatched flow-field lengths, bad dispersion parameters, boundary
    /// inflow-total vectors of the wrong length, and initial-totals of the wrong
    /// shape; and rejects a non-positive `dt` at step time.
    #[test]
    fn constructor_validates_inputs() {
        let n = 4;
        let grid = grid_1d(n, 1.0);
        let net = || ReactionNetwork::new(vec!["A".into(), "B".into()], vec![], vec![], vec![]).unwrap();
        let good_initial = || vec![vec![1.0, 1.0]; n];

        // Wrong water_content length.
        let bad_flow = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![1.0; n - 1],
        };
        assert!(ReactiveTransport::new(
            grid.clone(),
            bad_flow,
            0.1,
            0.0,
            net(),
            Vec::new(),
            good_initial()
        )
        .is_err());

        // Negative molecular diffusion.
        assert!(ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, 0.0),
            -1.0,
            0.0,
            net(),
            Vec::new(),
            good_initial()
        )
        .is_err());

        // Boundary inflow_totals with the wrong number of components.
        let bad_bc = vec![ReactiveBoundaryCondition {
            location: BoundaryLocation::XMin,
            inflow_totals: vec![1.0], // need Nc = 2
        }];
        assert!(ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, 0.0),
            0.1,
            0.0,
            net(),
            bad_bc,
            good_initial()
        )
        .is_err());

        // initial_totals with a wrong-width row.
        let bad_initial = vec![vec![1.0]; n]; // width 1, need 2
        assert!(ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, 0.0),
            0.1,
            0.0,
            net(),
            Vec::new(),
            bad_initial
        )
        .is_err());

        // initial_totals with the wrong number of rows.
        let bad_rows = vec![vec![1.0, 1.0]; n - 1];
        assert!(ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, 0.0),
            0.1,
            0.0,
            net(),
            Vec::new(),
            bad_rows
        )
        .is_err());

        // Non-positive dt rejected at step time.
        let mut good = ReactiveTransport::new(
            grid.clone(),
            uniform_x_flow(&grid, 0.0),
            0.1,
            0.0,
            net(),
            Vec::new(),
            good_initial(),
        )
        .unwrap();
        good.set_timestep(0.0);
        assert!(good.step().is_err());
    }
}
