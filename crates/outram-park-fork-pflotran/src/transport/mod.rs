//! Conservative (non-reactive) solute transport (v1) — bead op-v6s.11.
//!
//! Advection–diffusion–dispersion of a **passive scalar** — a single solute
//! concentration `c` (mass of solute per unit fluid volume, kg/m^3) — on the
//! structured Cartesian grid ([`crate::grid::CartesianGrid`]), advected by a
//! frozen flow field from a RICHARDS solve and spread by molecular diffusion
//! plus velocity-driven longitudinal dispersion. No chemistry: the solute does
//! not react, sorb, decay, or feed back on the flow. Reactive geochemistry is
//! deferred to bead op-v6s.12.
//!
//! # Governing equation
//!
//! For volumetric water content `theta_w = phi * S_l` (dimensionless), Darcy
//! volumetric face flux `q` (m^3/s), and effective diffusion/dispersion
//! coefficient `D` (m^2/s):
//!
//! ```text
//! d/dt(theta_w * c) + div(q * c) - div(theta_w * D * grad c) = 0
//! ```
//!
//! With `theta_w`, `q`, and `D` frozen from the flow solve, this is **linear**
//! in `c`. One backward-Euler step therefore assembles a single linear system
//! `A c = b` and solves it once with the foam-basic-lib Krylov backend
//! (BiCGStab + ILU(0)) — no Newton iteration.
//!
//! # Discretisation
//!
//! Cell-centred finite volume, implicit (backward) Euler in time:
//!
//! - **Accumulation** `theta_w_i V_i / dt (c_i - c_i^old)` — diagonal mass term.
//! - **Advection** — first-order **upwind**: the face concentration is taken from
//!   the upstream cell. Upwind is unconditionally monotone (the assembled matrix
//!   is an M-matrix), so a passive scalar stays within its physical bounds with
//!   no over/undershoot, at the cost of numerical (cross-wind) diffusion of order
//!   `|v| dx / 2`. A TVD / higher-order scheme is deferred.
//! - **Dispersion** — symmetric two-point flux using the grid's geometric
//!   transmissibility `area / distance`, with a face-averaged water content and a
//!   velocity-dependent coefficient `D_face = D_mol + alpha_L |v_darcy|` (see
//!   [`DispersionModel`]).
//!
//! # Boundary conditions
//!
//! - **Default (no BC on a face location):** advective outflow only, with a zero
//!   dispersive gradient. An unspecified inflow face uses the interior
//!   concentration (a zero-gradient inflow) — document this if it matters for a
//!   case.
//! - **[`TransportBoundaryKind::InflowConcentration`]:** a fixed boundary
//!   concentration `c_bc`. The advective part is upwinded (inflow carries `c_bc`
//!   into the domain; outflow carries the interior concentration out) and the
//!   dispersive part always couples the near-boundary cell to `c_bc` across the
//!   half-cell distance. Applying the dispersive coupling on **both** inflow and
//!   outflow makes this a genuine Dirichlet concentration usable at either end of
//!   a domain, which the analytical verification tests (steady advection–
//!   diffusion and pure-diffusion linear profile) rely on. See the note in
//!   [`SoluteTransport::step`].
//!
//! # Units
//!
//! Concentration `c` is kg/m^3; volumetric fluxes are m^3/s; water content is
//! dimensionless; `D` is m^2/s; masses are kg; time steps are seconds. The API
//! uses plain `f64` (not `uom`) because the solute field and its Krylov solve mix
//! quantities of differing dimension; callers apply units at the case-setup layer.

use crate::error::PflotranError;
use crate::grid::{Axis, BoundaryLocation, CartesianGrid};
use outram_foam_basic_lib::krylov::{bicgstab, KrylovResult, KrylovSettings, Preconditioner};
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::limiters::FluxLimiter;

/// A frozen flow field feeding the solute-transport step, as produced by a flow
/// (RICHARDS) solve.
///
/// Face indexing matches the grid exactly: `face_flux[f]` corresponds to
/// `grid.connections()[f]` and to face `f` of `grid.ldu_addressing()`;
/// `boundary_flux[b]` corresponds to `grid.boundary_faces()[b]`;
/// `water_content[i]` is cell `i`. All volumetric fluxes are in m^3/s.
///
/// The three vectors must have lengths `grid.connections().len()`,
/// `grid.boundary_faces().len()`, and `grid.n_cells()` respectively;
/// [`SoluteTransport::new`] validates this.
pub struct FlowField {
    /// Signed Darcy volumetric flux per **internal** face, m^3/s. Positive means
    /// flow from `owner` to `neighbour` (i.e. in the `+x`/`+y`/`+z` face
    /// direction). Length `grid.connections().len()`.
    pub face_flux: Vec<f64>,
    /// Signed Darcy volumetric flux per **boundary** face, m^3/s. Positive means
    /// **outflow** (leaving the domain); negative means inflow. Length
    /// `grid.boundary_faces().len()`.
    pub boundary_flux: Vec<f64>,
    /// Volumetric water content `theta_w = phi * S_l` per cell, dimensionless in
    /// `[0, 1]`. Length `grid.n_cells()`. Frozen from the flow solve.
    pub water_content: Vec<f64>,
}

/// Effective diffusion/dispersion model: `D_face = molecular_diffusion +
/// longitudinal_dispersivity * |v_darcy|`.
///
/// `v_darcy` is the Darcy velocity magnitude across a face (`|q| / area`, m/s),
/// so the second term is mechanical (velocity-driven) longitudinal dispersion and
/// the first is velocity-independent molecular diffusion. Transverse dispersion
/// is not modelled in v1.
pub struct DispersionModel {
    /// Molecular diffusion coefficient, m^2/s. Must be finite and `>= 0`.
    pub molecular_diffusion: f64,
    /// Longitudinal dispersivity `alpha_L`, m. Must be finite and `>= 0`.
    /// Multiplies the Darcy velocity magnitude to give the dispersive part of the
    /// face coefficient.
    pub longitudinal_dispersivity: f64,
}

impl DispersionModel {
    /// Construct a dispersion model from a molecular diffusion coefficient
    /// (m^2/s) and a longitudinal dispersivity (m).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if either argument is non-finite or
    /// negative.
    pub fn new(
        molecular_diffusion: f64,
        longitudinal_dispersivity: f64,
    ) -> Result<Self, PflotranError> {
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
        Ok(Self {
            molecular_diffusion,
            longitudinal_dispersivity,
        })
    }

    /// Effective face diffusion/dispersion coefficient `D_mol + alpha_L * v`
    /// (m^2/s) for a Darcy velocity magnitude `v_darcy` (m/s, `>= 0`).
    fn coefficient(&self, v_darcy: f64) -> f64 {
        self.molecular_diffusion + self.longitudinal_dispersivity * v_darcy
    }
}

/// The kind of solute boundary condition applied at a face location.
///
/// Enum dispatch (no trait objects), per the workspace design rules. Extended by
/// adding variants; every `match` on it is then checked for exhaustiveness.
pub enum TransportBoundaryKind {
    /// A fixed boundary solute concentration `c_bc` (kg/m^3). Advection is
    /// upwinded by the boundary flux sign; the dispersive flux always couples the
    /// near-boundary cell to `c_bc` across the half-cell distance, so this acts as
    /// a Dirichlet concentration at either an inflow or an outflow face.
    InflowConcentration(f64),
}

/// A solute boundary condition bound to one of the six domain-box face locations.
pub struct TransportBoundaryCondition {
    /// Which exterior face location this condition applies to.
    pub location: BoundaryLocation,
    /// The condition to impose there.
    pub kind: TransportBoundaryKind,
}

/// One implicit-Euler solute-transport step over a frozen flow field.
///
/// Holds the grid, the frozen [`FlowField`], the [`DispersionModel`], the solute
/// boundary conditions, the time step `dt` (s), and the previous-time
/// concentration `c_old` (kg/m^3). [`SoluteTransport::step`] assembles and solves
/// the linear system for the next-time concentration; repeated stepping (with
/// [`SoluteTransport::set_previous`] between steps, or by feeding the returned
/// field back) advances the solute in time or drives it to steady state.
pub struct SoluteTransport {
    grid: CartesianGrid,
    flow: FlowField,
    dispersion: DispersionModel,
    /// Boundary conditions, indexed for lookup by the six face locations. `None`
    /// slots use the default (advective-outflow / zero-gradient) behaviour.
    bc_by_location: [Option<f64>; 6],
    /// Time step, seconds (`> 0`, validated at [`SoluteTransport::step`]).
    dt: f64,
    /// Previous-time concentration per cell, kg/m^3. Length `n_cells`.
    c_old: Vec<f64>,
    /// Advection scheme. [`FluxLimiter::Upwind`] (the default) is pure first-order
    /// upwind; any other TVD limiter adds an explicit **deferred-correction**
    /// anti-diffusive flux (lagged one step), reducing numerical diffusion while
    /// staying monotone. The gradient ratio `r` is formed along the structured
    /// grid axis of each face; see [`SoluteTransport::step`].
    flux_limiter: FluxLimiter,
}

impl SoluteTransport {
    /// Build a solute-transport stepper from a grid, a frozen flow field, a
    /// dispersion model, and a list of solute boundary conditions.
    ///
    /// The time step defaults to `1.0` s (override with
    /// [`SoluteTransport::set_timestep`]) and the previous concentration defaults
    /// to zero everywhere (override with [`SoluteTransport::set_previous`]).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any flow-field vector has the
    /// wrong length for the grid, if any water content is non-finite or negative,
    /// if any flux is non-finite, or if a boundary condition carries a non-finite
    /// concentration. A later boundary condition for the same face location
    /// overrides an earlier one.
    pub fn new(
        grid: CartesianGrid,
        flow: FlowField,
        dispersion: DispersionModel,
        boundary: Vec<TransportBoundaryCondition>,
    ) -> Result<Self, PflotranError> {
        let n_cells = grid.n_cells();
        let n_int = grid.connections().len();
        let n_bnd = grid.boundary_faces().len();

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

        let mut bc_by_location: [Option<f64>; 6] = [None; 6];
        for bc in &boundary {
            match bc.kind {
                TransportBoundaryKind::InflowConcentration(c_bc) => {
                    if !c_bc.is_finite() {
                        return Err(PflotranError::InvalidInput(format!(
                            "InflowConcentration on {:?} carries non-finite c_bc = {c_bc}",
                            bc.location
                        )));
                    }
                    bc_by_location[location_index(bc.location)] = Some(c_bc);
                }
            }
        }

        Ok(Self {
            grid,
            flow,
            dispersion,
            bc_by_location,
            dt: 1.0,
            c_old: vec![0.0; n_cells],
            flux_limiter: FluxLimiter::Upwind,
        })
    }

    /// Select the advection scheme. [`FluxLimiter::Upwind`] (default) is
    /// first-order; a TVD limiter (e.g. [`FluxLimiter::VanLeer`],
    /// [`FluxLimiter::SuperBee`]) enables the deferred-correction higher-order
    /// scheme. The limiter is the OpenFOAM-derived `FluxLimiter` from
    /// `outram-foam-basic-lib`.
    pub fn set_flux_limiter(&mut self, limiter: FluxLimiter) {
        self.flux_limiter = limiter;
    }

    /// Set the time step `dt` (seconds). Validated (must be positive and finite)
    /// at [`SoluteTransport::step`].
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Set the previous-time concentration `c_old` (kg/m^3), one value per cell.
    ///
    /// # Panics
    ///
    /// Panics if `c_old.len() != self.n_cells()`.
    pub fn set_previous(&mut self, c_old: &[f64]) {
        assert_eq!(
            c_old.len(),
            self.c_old.len(),
            "set_previous: expected {} concentrations, got {}",
            self.c_old.len(),
            c_old.len()
        );
        self.c_old.copy_from_slice(c_old);
    }

    /// Number of grid cells (the size of the linear system).
    pub fn n_cells(&self) -> usize {
        self.grid.n_cells()
    }

    /// Assemble and solve `A c = b` for the next-time concentration, writing the
    /// solution into `c` (resized to `n_cells`).
    ///
    /// # Assembly (sign conventions)
    ///
    /// With owner `o`, neighbour `n`, signed internal flux `q_f` (`+` = `o -> n`),
    /// and geometric transmissibility `T = area / distance`:
    ///
    /// - Accumulation: `diag[i] += theta_w_i V_i / dt`, `b[i] += theta_w_i V_i /
    ///   dt * c_old_i`.
    /// - Advection (upwind): if `q_f >= 0`, `diag[o] += q_f`, `lower[f] -= q_f`;
    ///   else `upper[f] += q_f`, `diag[n] -= q_f`.
    /// - Dispersion: `d = D_face * theta_w_face * T` with `theta_w_face =
    ///   0.5(theta_w_o + theta_w_n)` and `D_face = D_mol + alpha_L |q_f| / area`;
    ///   then `diag[o] += d`, `upper[f] -= d`, `diag[n] += d`, `lower[f] -= d`.
    /// - Boundary face (cell `i`, signed `q_b`, `+` = outflow): default adds
    ///   `diag[i] += q_b` (advective outflow / zero-gradient inflow).
    ///   `InflowConcentration(c_bc)` adds the advective part (`b[i] -= q_b c_bc`
    ///   at inflow `q_b < 0`, else `diag[i] += q_b`) plus a dispersive Dirichlet
    ///   coupling `d_b = D_face * theta_w_i * area / distance` giving `diag[i] +=
    ///   d_b`, `b[i] += d_b c_bc`.
    ///
    /// **Note on the boundary dispersive term.** The dispersive coupling to
    /// `c_bc` is applied for `InflowConcentration` regardless of the boundary
    /// flux sign, so the condition behaves as a true Dirichlet concentration at an
    /// outflow or zero-flux face as well as at an inflow face. This is what makes
    /// a fixed outlet concentration (steady advection–diffusion) and a
    /// both-ends-fixed pure-diffusion profile representable; the default (no-BC)
    /// boundary keeps the pure advective-outflow / zero-gradient behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `dt` is non-positive or
    /// non-finite, or [`PflotranError::Convergence`] if the Krylov solve does not
    /// reach its tolerance.
    pub fn step(&mut self, c: &mut Vec<f64>) -> Result<KrylovResult, PflotranError> {
        if !self.dt.is_finite() || self.dt <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "time step dt = {} must be finite and > 0 (s)",
                self.dt
            )));
        }

        let n_cells = self.grid.n_cells();
        let (owner, neighbour) = self.grid.ldu_addressing();
        let mut a = LduMatrix::new(n_cells, owner, neighbour);
        let mut b = vec![0.0_f64; n_cells];

        // Accumulation (implicit Euler mass term).
        let inv_dt = 1.0 / self.dt;
        for i in 0..n_cells {
            let acc = self.flow.water_content[i] * self.grid.cell_volume(i) * inv_dt;
            a.diag[i] += acc;
            b[i] += acc * self.c_old[i];
        }

        // Internal faces: advection (upwind) + dispersion (symmetric two-point).
        let connections = self.grid.connections();
        for (f, conn) in connections.iter().enumerate() {
            let o = conn.owner;
            let n = conn.neighbour;
            let q = self.flow.face_flux[f];

            // Upwind advection.
            if q >= 0.0 {
                a.diag[o] += q;
                a.lower[f] -= q;
            } else {
                a.upper[f] += q;
                a.diag[n] -= q;
            }

            // Dispersion.
            let theta_face = 0.5 * (self.flow.water_content[o] + self.flow.water_content[n]);
            let v_darcy = q.abs() / conn.area;
            let d_face = self.dispersion.coefficient(v_darcy);
            let d = d_face * theta_face * conn.geometric_transmissibility;
            a.diag[o] += d;
            a.upper[f] -= d;
            a.diag[n] += d;
            a.lower[f] -= d;
        }

        // Boundary faces.
        let boundary_faces = self.grid.boundary_faces();
        for (bidx, face) in boundary_faces.iter().enumerate() {
            let i = face.cell;
            let q = self.flow.boundary_flux[bidx];
            let geom = face.area / face.distance;
            match self.bc_by_location[location_index(face.location)] {
                None => {
                    // Default: advective outflow / zero-gradient inflow.
                    a.diag[i] += q;
                }
                Some(c_bc) => {
                    // Advective part (upwind by flux sign).
                    if q < 0.0 {
                        b[i] -= q * c_bc; // inflow brings c_bc (q < 0 => b[i] += |q| c_bc)
                    } else {
                        a.diag[i] += q; // outflow carries interior concentration out
                    }
                    // Dispersive Dirichlet coupling to c_bc (always applied).
                    let v_darcy = q.abs() / face.area;
                    let d_face = self.dispersion.coefficient(v_darcy);
                    let d_b = d_face * self.flow.water_content[i] * geom;
                    a.diag[i] += d_b;
                    b[i] += d_b * c_bc;
                }
            }
        }

        // Deferred-correction TVD advection (explicit, lagged on c_old): add the
        // limited anti-diffusive flux (high-order minus first-order upwind) to the
        // RHS. The gradient ratio r is formed from the upstream and
        // upstream-upstream cells along the face's structured axis; a face with no
        // far-upstream cell (domain edge) falls back to first-order. The limiter
        // psi(r) is the OpenFOAM-derived `FluxLimiter` from outram-foam-basic-lib.
        if self.flux_limiter != FluxLimiter::Upwind {
            let (nx, ny, nz) = self.grid.dims();
            for (f, conn) in connections.iter().enumerate() {
                let q = self.flow.face_flux[f];
                if q == 0.0 {
                    continue;
                }
                let o = conn.owner;
                let n = conn.neighbour;
                // Owner has the lower index along the face axis (neighbour = owner
                // + axis stride), so q > 0 flows in the +axis direction.
                let (u, d, up_is_owner) = if q > 0.0 { (o, n, true) } else { (n, o, false) };
                let denom = self.c_old[d] - self.c_old[u];
                if denom.abs() < 1.0e-30 {
                    continue;
                }
                let (iu, ju, ku) = self.grid.cell_ijk(u);
                let uu = match conn.axis {
                    Axis::X if up_is_owner => (iu > 0).then(|| self.grid.cell_index(iu - 1, ju, ku)),
                    Axis::X => (iu + 1 < nx).then(|| self.grid.cell_index(iu + 1, ju, ku)),
                    Axis::Y if up_is_owner => (ju > 0).then(|| self.grid.cell_index(iu, ju - 1, ku)),
                    Axis::Y => (ju + 1 < ny).then(|| self.grid.cell_index(iu, ju + 1, ku)),
                    Axis::Z if up_is_owner => (ku > 0).then(|| self.grid.cell_index(iu, ju, ku - 1)),
                    Axis::Z => (ku + 1 < nz).then(|| self.grid.cell_index(iu, ju, ku + 1)),
                };
                let uu = match uu {
                    Some(c) => c,
                    None => continue, // no far-upstream cell -> first-order here
                };
                let r = (self.c_old[u] - self.c_old[uu]) / denom;
                let psi = self.flux_limiter.psi(r);
                if psi == 0.0 {
                    continue;
                }
                // Anti-diffusive flux (high-order minus upwind) from U to D:
                // |q| * 0.5 * psi(r) * (c_D - c_U). U loses it, D gains it.
                let corr = q.abs() * 0.5 * psi * denom;
                b[u] -= corr;
                b[d] += corr;
            }
        }

        // Solve A c = b once (linear problem) with BiCGStab + ILU(0).
        let precond = Preconditioner::ilu0(&a);
        let settings = KrylovSettings {
            tolerance: 1.0e-12,
            max_iter: 2000,
            restart: 50,
        };
        let (x, result) = bicgstab(&a, &b, Some(self.c_old.as_slice()), &precond, &settings);
        if !result.converged {
            return Err(PflotranError::Convergence(format!(
                "solute-transport linear solve did not converge: {} iters, residual {:.3e}",
                result.n_iterations, result.final_residual
            )));
        }

        c.clear();
        c.extend_from_slice(&x);
        Ok(result)
    }

    /// Total solute mass `sum_i V_i * theta_w_i * c_i` (kg).
    ///
    /// # Panics
    ///
    /// Panics if `c.len() != self.n_cells()`.
    pub fn total_mass(&self, c: &[f64]) -> f64 {
        assert_eq!(
            c.len(),
            self.grid.n_cells(),
            "total_mass: expected {} concentrations, got {}",
            self.grid.n_cells(),
            c.len()
        );
        let mut mass = 0.0;
        for i in 0..c.len() {
            mass += self.grid.cell_volume(i) * self.flow.water_content[i] * c[i];
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
    use crate::units::GeoLength;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(x: f64) -> GeoLength {
        Length::new::<meter>(x)
    }

    /// Build a 1D (x-only) uniform grid of `n` cells with unit cross-section,
    /// each of width `dx`, spanning `[0, n*dx]`.
    fn grid_1d(n: usize, dx: f64) -> CartesianGrid {
        CartesianGrid::uniform(n, 1, 1, m(dx), m(1.0), m(1.0)).unwrap()
    }

    /// A uniform +x Darcy flow: every internal face and the two x boundary faces
    /// carry volumetric flux `q` (m^3/s); XMin boundary is inflow (`-q`), XMax is
    /// outflow (`+q`). All other boundary faces (y/z) carry zero. `theta_w = 1`.
    fn uniform_x_flow(grid: &CartesianGrid, q: f64) -> FlowField {
        let n_int = grid.connections().len();
        let face_flux = vec![q; n_int];
        let mut boundary_flux = vec![0.0; grid.boundary_faces().len()];
        for (b, face) in grid.boundary_faces().iter().enumerate() {
            boundary_flux[b] = match face.location {
                BoundaryLocation::XMin => -q, // inflow
                BoundaryLocation::XMax => q,  // outflow
                _ => 0.0,
            };
        }
        FlowField {
            face_flux,
            boundary_flux,
            water_content: vec![1.0; grid.n_cells()],
        }
    }

    /// TVD deferred-correction (calling foam-basic-lib `FluxLimiter`) must reduce
    /// numerical diffusion versus first-order upwind on an advection-dominated
    /// front, while staying monotone (no over/undershoot). Methodology: 50-cell
    /// column, v = 1 m/s, D = 0.05 m^2/s (Pe = 20 — sharp front near the outlet),
    /// InflowConcentration(1) at XMin, (0) at XMax; marched to steady state with
    /// Upwind and with SuperBee, each compared to the exact exponential profile.
    /// Result (2026-07-22): SuperBee's max error is strictly below Upwind's, and
    /// SuperBee stays within [0, 1].
    #[test]
    fn tvd_reduces_numerical_diffusion_and_stays_bounded() {
        let n = 50usize;
        let dx: f64 = 0.02;
        let (v, d, length): (f64, f64, f64) = (1.0, 0.05, 1.0);
        let pe = v * length / d;
        let exact = |x: f64| (pe.exp() - (pe * x / length).exp()) / (pe.exp() - 1.0);

        let run = |limiter: FluxLimiter| -> Vec<f64> {
            let grid = grid_1d(n, dx);
            let flow = uniform_x_flow(&grid, v); // unit area => Darcy v = q
            let boundary = vec![
                TransportBoundaryCondition {
                    location: BoundaryLocation::XMin,
                    kind: TransportBoundaryKind::InflowConcentration(1.0),
                },
                TransportBoundaryCondition {
                    location: BoundaryLocation::XMax,
                    kind: TransportBoundaryKind::InflowConcentration(0.0),
                },
            ];
            let mut t = SoluteTransport::new(grid, flow, DispersionModel::new(d, 0.0).unwrap(), boundary)
                .unwrap();
            t.set_flux_limiter(limiter);
            t.set_timestep(1.0e6); // large dt -> steady per step
            let mut c = vec![0.0; n];
            for _ in 0..80 {
                t.set_previous(&c);
                t.step(&mut c).unwrap();
            }
            c
        };

        let up = run(FluxLimiter::Upwind);
        let sb = run(FluxLimiter::SuperBee);

        let err = |c: &[f64]| -> f64 {
            (0..n)
                .map(|i| (c[i] - exact((i as f64 + 0.5) * dx)).abs())
                .fold(0.0, f64::max)
        };
        let (e_up, e_sb) = (err(&up), err(&sb));
        println!("upwind max err = {e_up:.4}, superbee max err = {e_sb:.4}");
        assert!(e_sb < e_up, "TVD (SuperBee) did not beat upwind: {e_sb} vs {e_up}");
        for (i, &c) in sb.iter().enumerate() {
            assert!(
                (-1.0e-6..=1.0 + 1.0e-6).contains(&c),
                "SuperBee overshoot at cell {i}: {c}"
            );
        }
    }

    /// Verification test 1 — **steady 1D advection–diffusion, closed form.**
    ///
    /// Methodology: a uniform 1D grid (`n = 50`, `dx = 0.02 m`, `L = 1 m`, unit
    /// cross-section), constant Darcy velocity `v = q / A = 1 m/s`, constant
    /// molecular diffusion `D = 0.5 m^2/s` (dispersivity 0), `theta_w = 1`.
    /// Boundary: `InflowConcentration(1.0)` at XMin (`x = 0`, inflow) and
    /// `InflowConcentration(0.0)` at XMax (`x = L`, outflow — the Dirichlet
    /// dispersive coupling pins it). Peclet `Pe = v L / D = 2`. The system is
    /// marched with a large `dt` for enough steps to reach steady state, then the
    /// cell-centre profile is compared to the exact steady solution for Dirichlet
    /// ends `c(0) = 1`, `c(L) = 0`:
    ///
    /// ```text
    /// c(x) = (exp(Pe) - exp(Pe x / L)) / (exp(Pe) - 1)
    /// ```
    ///
    /// which equals `1 - (1 - exp(Pe x / L)) / (1 - exp(Pe))` (the task's stated
    /// form, reflected because the inflow value `1` is at XMin rather than XMax).
    ///
    /// Results: first-order upwind adds numerical diffusion `~ v dx / 2 = 0.01
    /// m^2/s`, an effective Peclet `~ v L / (D + v dx/2) = 1.96` versus the
    /// analytical `2.0`. The measured maximum absolute nodal error is well under
    /// the `0.05` (5 % of the unit concentration range) tolerance asserted below;
    /// it shrinks as `dx -> 0` (numerical diffusion vanishing), confirming a
    /// consistent advection–diffusion discretisation. Interpretation: the scheme
    /// reproduces the exponential boundary-layer profile to a few percent at this
    /// modest resolution, the residual being the documented first-order upwind
    /// numerical diffusion.
    #[test]
    fn steady_advection_diffusion_matches_closed_form() {
        let n = 50;
        let dx = 0.02;
        let l = n as f64 * dx; // 1.0
        let v = 1.0;
        let d = 0.5;
        let pe = v * l / d; // 2.0

        let grid = grid_1d(n, dx);
        let flow = uniform_x_flow(&grid, v); // area = 1 => q = v
        let disp = DispersionModel::new(d, 0.0).unwrap();
        let bcs = vec![
            TransportBoundaryCondition {
                location: BoundaryLocation::XMin,
                kind: TransportBoundaryKind::InflowConcentration(1.0),
            },
            TransportBoundaryCondition {
                location: BoundaryLocation::XMax,
                kind: TransportBoundaryKind::InflowConcentration(0.0),
            },
        ];
        let mut transport = SoluteTransport::new(grid, flow, disp, bcs).unwrap();
        transport.set_timestep(1.0e6); // large dt -> steady state in a few steps

        let mut c = vec![0.0; n];
        for _ in 0..20 {
            let res = transport.step(&mut c).unwrap();
            assert!(res.converged);
            transport.set_previous(&c);
        }

        let mut max_err = 0.0_f64;
        for i in 0..n {
            let x = (i as f64 + 0.5) * dx;
            let exact = (pe.exp() - (pe * x / l).exp()) / (pe.exp() - 1.0);
            max_err = max_err.max((c[i] - exact).abs());
        }
        assert!(
            max_err < 0.05,
            "steady advection-diffusion max nodal error {max_err} exceeds 0.05"
        );
    }

    /// Verification test 2 — **pure diffusion steady, linear profile.**
    ///
    /// Methodology: `v = 0` (all fluxes zero), `D = 1 m^2/s`, `theta_w = 1`,
    /// Dirichlet `InflowConcentration(1.0)` at XMin and `InflowConcentration(0.0)`
    /// at XMax on a uniform 1D grid (`n = 10`, `dx = 0.1 m`). With no advection the
    /// exact steady solution of `-D c'' = 0` with `c(0) = 1`, `c(L) = 0` is the
    /// straight line `c(x) = 1 - x / L`. A second-order finite-volume Laplacian
    /// (with the half-cell boundary distance) reproduces a linear field exactly at
    /// cell centres.
    ///
    /// Results: the measured maximum nodal error is at the round-off / Krylov-
    /// tolerance level, far below the `1e-9` assertion — confirming the diffusion
    /// operator and the Dirichlet boundary coupling are assembled correctly.
    #[test]
    fn pure_diffusion_steady_is_linear() {
        let n = 10;
        let dx = 0.1;
        let l = n as f64 * dx; // 1.0

        let grid = grid_1d(n, dx);
        // Zero flow everywhere.
        let flow = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![1.0; grid.n_cells()],
        };
        let disp = DispersionModel::new(1.0, 0.0).unwrap();
        let bcs = vec![
            TransportBoundaryCondition {
                location: BoundaryLocation::XMin,
                kind: TransportBoundaryKind::InflowConcentration(1.0),
            },
            TransportBoundaryCondition {
                location: BoundaryLocation::XMax,
                kind: TransportBoundaryKind::InflowConcentration(0.0),
            },
        ];
        let mut transport = SoluteTransport::new(grid, flow, disp, bcs).unwrap();
        transport.set_timestep(1.0e6);

        let mut c = vec![0.0; n];
        for _ in 0..5 {
            transport.step(&mut c).unwrap();
            transport.set_previous(&c);
        }

        for i in 0..n {
            let x = (i as f64 + 0.5) * dx;
            let exact = 1.0 - x / l;
            assert!(
                (c[i] - exact).abs() < 1e-9,
                "cell {i}: c = {}, exact linear = {exact}",
                c[i]
            );
        }
    }

    /// Verification test 3 — **mass conservation on a closed domain.**
    ///
    /// Methodology: a closed 3D box (`4 x 3 x 2` cells) with all internal and
    /// boundary fluxes zero and **no** boundary conditions, so the only active
    /// process is internal molecular diffusion (`D = 0.3 m^2/s`), which
    /// redistributes solute but exchanges none across the sealed boundary. Started
    /// from a non-uniform initial concentration, the total solute mass
    /// `sum_i V_i theta_w_i c_i` must be invariant: the discrete scheme is
    /// conservative by construction (internal dispersive fluxes cancel pairwise;
    /// accumulation telescopes) up to the linear-solver tolerance.
    ///
    /// Results: the observed relative mass drift across ten steps is at the
    /// Krylov-tolerance level (`~1e-11` at `tolerance = 1e-12`), comfortably inside
    /// the asserted `1e-9`.
    #[test]
    fn closed_domain_conserves_mass() {
        let grid = CartesianGrid::uniform(4, 3, 2, m(1.0), m(1.0), m(1.0)).unwrap();
        let n = grid.n_cells();
        let flow = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![0.4; n], // uniform porosity*saturation
        };
        let disp = DispersionModel::new(0.3, 0.0).unwrap();
        let mut transport = SoluteTransport::new(grid, flow, disp, Vec::new()).unwrap();
        transport.set_timestep(0.5);

        // Non-uniform initial concentration.
        let mut c: Vec<f64> = (0..n).map(|i| (i as f64) * 0.1 + 0.05).collect();
        transport.set_previous(&c);
        let mass0 = transport.total_mass(&c);

        for _ in 0..10 {
            transport.step(&mut c).unwrap();
            transport.set_previous(&c);
            let mass = transport.total_mass(&c);
            let rel = (mass - mass0).abs() / mass0.abs();
            assert!(rel < 1e-9, "relative mass drift {rel} exceeds 1e-9");
        }
    }

    /// Verification test 4 — **monotonicity of first-order upwind (no
    /// over/undershoot) and front advance.**
    ///
    /// Methodology: uniform +x advection (`v = 1 m/s`, `D = 0`) into an initially
    /// solute-free 1D domain (`n = 20`, `dx = 0.05 m`) with
    /// `InflowConcentration(1.0)` at XMin and a default (advective-outflow)
    /// boundary at XMax. Implicit-Euler upwind assembles an M-matrix, so the
    /// solution must stay within the physical bounds `[0, 1]` (no over/undershoot)
    /// and form a monotonically decreasing profile with the front advancing from
    /// the inlet.
    ///
    /// Results: after stepping, every cell value lies in `[0, 1]` (to round-off),
    /// the profile is non-increasing in `x`, and the inlet cell exceeds the outlet
    /// cell — confirming the scheme is bounded and advection-directed.
    #[test]
    fn upwind_advection_is_monotone_and_advances() {
        let n = 20;
        let dx = 0.05;
        let v = 1.0;

        let grid = grid_1d(n, dx);
        let flow = uniform_x_flow(&grid, v);
        let disp = DispersionModel::new(0.0, 0.0).unwrap(); // pure advection
        let bcs = vec![TransportBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: TransportBoundaryKind::InflowConcentration(1.0),
        }];
        let mut transport = SoluteTransport::new(grid, flow, disp, bcs).unwrap();
        transport.set_timestep(0.02); // Courant ~ v*dt/dx = 0.4 per step

        let mut c = vec![0.0; n];
        for _ in 0..5 {
            transport.step(&mut c).unwrap();
            transport.set_previous(&c);

            // Bounded in [0, 1] — no over/undershoot.
            for &ci in &c {
                assert!(
                    ci >= -1e-10 && ci <= 1.0 + 1e-10,
                    "concentration {ci} left the physical range [0, 1]"
                );
            }
            // Monotonically decreasing in x (front is sharp/diffused but ordered).
            for i in 0..n - 1 {
                assert!(
                    c[i] >= c[i + 1] - 1e-10,
                    "profile not monotone at cell {i}: {} < {}",
                    c[i],
                    c[i + 1]
                );
            }
        }
        // Front has advanced from the inlet but not filled the domain.
        assert!(c[0] > 0.5, "inlet cell should be well-wetted, got {}", c[0]);
        assert!(
            c[n - 1] < c[0],
            "outlet cell {} should trail the inlet {}",
            c[n - 1],
            c[0]
        );
    }

    /// Constructor rejects mismatched flow-field lengths and bad dispersion.
    #[test]
    fn constructor_validates_inputs() {
        let grid = grid_1d(4, 1.0);
        let good = FlowField {
            face_flux: vec![0.0; grid.connections().len()],
            boundary_flux: vec![0.0; grid.boundary_faces().len()],
            water_content: vec![1.0; grid.n_cells()],
        };
        // Wrong water_content length.
        let bad = FlowField {
            face_flux: good.face_flux.clone(),
            boundary_flux: good.boundary_flux.clone(),
            water_content: vec![1.0; 3],
        };
        let disp = DispersionModel::new(1.0, 0.0).unwrap();
        assert!(SoluteTransport::new(grid.clone(), bad, disp, Vec::new()).is_err());

        // Negative diffusion rejected.
        assert!(DispersionModel::new(-1.0, 0.0).is_err());
        assert!(DispersionModel::new(0.0, -1.0).is_err());

        // Non-positive dt rejected at step time.
        let disp2 = DispersionModel::new(1.0, 0.0).unwrap();
        let mut t = SoluteTransport::new(grid, good, disp2, Vec::new()).unwrap();
        t.set_timestep(0.0);
        let mut c = vec![0.0; 4];
        assert!(t.step(&mut c).is_err());
    }
}
