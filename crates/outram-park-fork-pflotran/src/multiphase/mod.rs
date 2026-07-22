//! Two-phase immiscible (air–water) isothermal flow — bead op-v6s.13, the first
//! cut toward the GENERAL multiphase mode.
//!
//! This module is the **coupled, two-unknowns-per-cell analogue** of the scalar
//! RICHARDS mode ([`crate::flow`]). Where RICHARDS carries one unknown per cell
//! (liquid pressure) and is solved on the scalar Newton–Krylov layer, two-phase
//! flow carries **two** primary unknowns per cell — liquid pressure `P_l` (Pa)
//! and liquid saturation `S_l` (dimensionless) — and is assembled onto the block
//! multi-DOF solver [`crate::solver::block`] with `nb = 2` DOF per cell.
//!
//! # Governing equations
//!
//! Two mass-conservation equations per cell, discretised by a cell-centred
//! **two-point flux** finite volume in space and **backward (implicit) Euler**
//! in time:
//!
//! $$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla\cdot\left(\rho_l\, \mathbf{q}_l\right) = 0, \qquad \mathbf{q}_l = -\frac{k\, k_{rl}(S_l)}{\mu_l}\left(\nabla P_l - \rho_l\,\mathbf{g}\right) $$
//!
//! $$ \frac{\partial}{\partial t}\left(\phi\, S_g\, \rho_g\right) + \nabla\cdot\left(\rho_g\, \mathbf{q}_g\right) = 0, \qquad \mathbf{q}_g = -\frac{k\, k_{rg}(S_g)}{\mu_g}\left(\nabla P_g - \rho_g\,\mathbf{g}\right) $$
//!
//! with the saturation and capillary closures
//!
//! $$ S_g = 1 - S_l, \qquad P_g = P_l + P_c(S_l), \qquad S_e = \frac{S_l - S_r}{1 - S_r} . $$
//!
//! Face mobility (`k_{rl}` / `k_{rg}`) is **upstream-weighted** per phase, as in
//! RICHARDS — the physically correct choice for advective transport and the
//! reason the block Jacobian is nonsymmetric.
//!
//! # Constitutive closures
//!
//! - **Liquid relative permeability** `k_{rl} = relative_permeability(S_e)` from
//!   the supplied [`CharacteristicCurves`] (van Genuchten / Brooks–Corey /
//!   Haverkamp).
//! - **Gas relative permeability** uses a documented closed-form **quadratic
//!   Corey** non-wetting curve with **zero residual gas saturation**:
//!
//!   $$ k_{rg}(S_e) = (1 - S_e)^2 . $$
//!
//!   This is `1` at `S_e = 0` (fully gas), `0` at `S_e = 1` (fully liquid), and
//!   monotone. It is computed directly from `S_e` and does **not** come from the
//!   `CharacteristicCurves` (whose relative permeability is the wetting phase's).
//! - **Capillary pressure** `P_c(S_l)` is obtained by **numerically inverting**
//!   the retention curve: `P_c` is the pressure at which
//!   `CharacteristicCurves::effective_saturation(P_c) = S_e`. Because
//!   `effective_saturation` is monotone non-increasing in `P_c`, the inverse is
//!   found by exponential bracketing + bisection (see
//!   [`TwoPhaseFlow::capillary_pressure`]). `S_e >= 1` maps to `P_c = 0`
//!   (saturated); the target `S_e` is floored at `1e-9` so the suction stays
//!   finite at the residual limit.
//!
//! # Simplifications (v1) — flags for human review
//!
//! - **Constant phase densities and viscosities.** Both liquid and gas are
//!   treated as incompressible with constant [`TwoPhaseFluids`] properties. Gas
//!   is therefore **constant density** — ideal-gas / slightly-compressible gas
//!   is a documented follow-up. This keeps the accumulation term linear in the
//!   unknowns and the residual well-scaled.
//! - **Isothermal.** No energy equation; adding temperature as a third unknown
//!   (the full air–water–energy GENERAL mode) is deferred.
//! - **Verification-only, not validated.** No comparison to published PFLOTRAN
//!   two-phase results has been done; the tests here are self-contained physical
//!   sanity checks (stationarity, phase-mass conservation, bounded imbibition).
//!
//! # State-vector layout
//!
//! All state vectors are interleaved, length `2 * n_cells`: cell `c` occupies
//! `[2c] = P_l` (Pa) and `[2c + 1] = S_l` (dimensionless), matching the block
//! solver's `nb = 2` convention. Where `S_l` feeds the property closures it is
//! clamped to `[S_r, 1]` to keep `S_e`, `k_r`, and `P_c` finite; the
//! accumulation term uses the raw `S_l` so the discrete mass balance stays exact.
//!
//! # Sign & reference conventions
//!
//! - `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
//! - Per-face potential for phase `p`: `Phi_p = P_p + rho_p * g * z`.
//! - A residual component is a phase mass rate (kg/s); a positive face/boundary
//!   flux **leaves** the cell. Unspecified boundary faces are **no-flow**.
//!
//! > **Untrusted AI draft — verification only**, per the workspace
//! > `RESPONSIBLE_USE.md`. Not for facility operation or any safety-critical use.

use crate::error::PflotranError;
use crate::grid::{BoundaryLocation, CartesianGrid};
use crate::properties::CharacteristicCurves;
use crate::solver::block::{
    BlockLduMatrix, BlockNewtonConfig, BlockNewtonReport, BlockNewtonSolver, BlockNonlinearSystem,
};

use rayon::prelude::*;
use uom::si::f64::{Pressure, Ratio};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// Standard gravitational acceleration (m/s^2) — the conventional default for a
/// two-phase run (gravity acting in `-z`).
pub const STANDARD_GRAVITY: f64 = 9.806_65;

/// A pair of immiscible fluids (wetting liquid + non-wetting gas) with constant
/// SI properties.
///
/// All four fields must be strictly positive and finite. Densities are in
/// kg/m^3 and viscosities in Pa·s. For v1 the properties are **constant** (the
/// liquid is incompressible and the gas is treated as constant density); see the
/// module header for the simplifications this entails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoPhaseFluids {
    /// Liquid (wetting) mass density `rho_l`, kg/m^3. Strictly positive.
    pub liquid_density: f64,
    /// Liquid (wetting) dynamic viscosity `mu_l`, Pa·s. Strictly positive.
    pub liquid_viscosity: f64,
    /// Gas (non-wetting) mass density `rho_g`, kg/m^3. Strictly positive.
    pub gas_density: f64,
    /// Gas (non-wetting) dynamic viscosity `mu_g`, Pa·s. Strictly positive.
    pub gas_viscosity: f64,
}

impl TwoPhaseFluids {
    /// Representative **air–water at ~20 °C** properties: liquid water
    /// `rho_l = 1000 kg/m^3`, `mu_l = 1.0e-3 Pa·s`; air `rho_g = 1.2 kg/m^3`,
    /// `mu_g = 1.8e-5 Pa·s`. Constant (incompressible) for v1.
    pub fn air_water() -> Self {
        Self {
            liquid_density: 1000.0,
            liquid_viscosity: 1.0e-3,
            gas_density: 1.2,
            gas_viscosity: 1.8e-5,
        }
    }

    /// Build a validated fluid pair.
    ///
    /// # Parameters
    ///
    /// - `liquid_density`, `gas_density` — kg/m^3, each finite and `> 0`.
    /// - `liquid_viscosity`, `gas_viscosity` — Pa·s, each finite and `> 0`.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if any property is non-finite or `<= 0`.
    pub fn new(
        liquid_density: f64,
        liquid_viscosity: f64,
        gas_density: f64,
        gas_viscosity: f64,
    ) -> Result<Self, PflotranError> {
        for (name, v) in [
            ("liquid_density", liquid_density),
            ("liquid_viscosity", liquid_viscosity),
            ("gas_density", gas_density),
            ("gas_viscosity", gas_viscosity),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "TwoPhaseFluids {name} must be finite and > 0, got {v}"
                )));
            }
        }
        Ok(Self {
            liquid_density,
            liquid_viscosity,
            gas_density,
            gas_viscosity,
        })
    }
}

/// What is prescribed on one exterior boundary location for two-phase flow.
///
/// Unspecified faces default to [`TwoPhaseBoundaryKind::NoFlow`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwoPhaseBoundaryKind {
    /// Fixed liquid pressure (Pa) and liquid saturation (dimensionless) at the
    /// boundary. The ghost state's gas pressure follows the capillary closure
    /// `P_g = P_l + P_c(S_l)`, so both phases can exchange mass across the face.
    Dirichlet {
        /// Prescribed liquid pressure `P_l` at the boundary, Pa.
        liquid_pressure: f64,
        /// Prescribed liquid saturation `S_l` at the boundary, dimensionless in
        /// `[S_r, 1]`.
        liquid_saturation: f64,
    },
    /// No-flow (zero normal flux for both phases). The natural default.
    NoFlow,
}

/// A two-phase boundary condition applied to one of the six Cartesian faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoPhaseBoundaryCondition {
    /// Which exterior face of the logical box this applies to.
    pub location: BoundaryLocation,
    /// What is prescribed there.
    pub kind: TwoPhaseBoundaryKind,
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

/// The two-phase immiscible flow problem for a single backward-Euler timestep.
///
/// Owns the grid, characteristic curves, fluid pair, and homogeneous material
/// data (isotropic permeability, uniform porosity). Implements
/// [`BlockNonlinearSystem`] with `dof_per_cell = 2`, so it is driven by the
/// generic [`BlockNewtonSolver`]. The block Jacobian is assembled **numerically**
/// — local finite differences of the two residual equations with respect to the
/// two local unknowns over the two-point stencil — which matches the residual by
/// construction and side-steps fragile analytic derivatives through the upstream
/// switch and the capillary-inversion closure.
pub struct TwoPhaseFlow {
    grid: CartesianGrid,
    curves: CharacteristicCurves,
    fluids: TwoPhaseFluids,
    porosity: f64,
    permeability: f64,
    gravity: f64,
    // Precomputed helpers (built in `new`).
    z: Vec<f64>,
    cell_faces: Vec<Vec<usize>>,
    cell_boundary_faces: Vec<Vec<usize>>,
    bc: [Option<TwoPhaseBoundaryKind>; 6],
    // Time-stepping state.
    dt: f64,
    // Previous-time-level liquid saturation per cell (the only accumulation term
    // that changes with constant densities). Length `n_cells`.
    s_old: Vec<f64>,
}

impl TwoPhaseFlow {
    /// Build a two-phase flow problem.
    ///
    /// # Parameters
    ///
    /// - `grid` — structured Cartesian FV mesh.
    /// - `curves` — retention + wetting relative-permeability model.
    /// - `fluids` — the constant-property liquid/gas pair.
    /// - `porosity` — dimensionless, in `(0, 1)`.
    /// - `permeability` — intrinsic isotropic `k`, m^2, `> 0`.
    /// - `gravity` — magnitude (m/s^2, acting in `-z`; pass `0` to disable).
    /// - `boundary` — per-face conditions; any face omitted is no-flow.
    ///
    /// The timestep defaults to `1 s` and the previous saturation to `1.0` until
    /// [`set_timestep`](Self::set_timestep) / [`set_previous`](Self::set_previous)
    /// are called (the [`TwoPhaseSimulation`] driver does this each step).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `porosity` is not in `(0, 1)`, or
    /// `permeability`/`gravity` are non-finite or out of range.
    pub fn new(
        grid: CartesianGrid,
        curves: CharacteristicCurves,
        fluids: TwoPhaseFluids,
        porosity: f64,
        permeability: f64,
        gravity: f64,
        boundary: Vec<TwoPhaseBoundaryCondition>,
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

        let mut bc: [Option<TwoPhaseBoundaryKind>; 6] = [None; 6];
        for cond in boundary {
            bc[location_index(&cond.location)] = Some(cond.kind);
        }

        Ok(Self {
            grid,
            curves,
            fluids,
            porosity,
            permeability,
            gravity,
            z,
            cell_faces,
            cell_boundary_faces,
            bc,
            dt: 1.0,
            s_old: vec![1.0; n],
        })
    }

    /// Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be
    /// positive.
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Set the previous-time-level state (interleaved `[P_l0, S_l0, P_l1, S_l1,
    /// …]`, length `2 * n_cells`).
    ///
    /// Only the liquid saturation enters the (constant-density) accumulation
    /// term, so the liquid pressures in `state` are accepted but not stored.
    ///
    /// # Panics
    ///
    /// Panics if `state.len() != 2 * n_cells`.
    pub fn set_previous(&mut self, state: &[f64]) {
        let n = self.grid.n_cells();
        assert_eq!(
            state.len(),
            2 * n,
            "set_previous: state must be length 2*n_cells"
        );
        for c in 0..n {
            self.s_old[c] = state[2 * c + 1];
        }
    }

    /// Read access to the underlying grid.
    pub fn grid(&self) -> &CartesianGrid {
        &self.grid
    }

    /// Total **water** mass in the domain (kg) for an interleaved state `x`:
    /// `sum_i V_i * phi * S_l_i * rho_l`.
    ///
    /// With no-flow boundaries this is conserved across a timestep to the
    /// nonlinear-solver tolerance, because the summed internal water fluxes
    /// cancel and every water residual is driven to zero.
    pub fn total_water_mass(&self, x: &[f64]) -> f64 {
        let rho = self.fluids.liquid_density;
        (0..self.grid.n_cells())
            .map(|i| self.grid.cell_volume(i) * self.porosity * x[2 * i + 1] * rho)
            .sum()
    }

    /// Total **gas** mass in the domain (kg) for an interleaved state `x`:
    /// `sum_i V_i * phi * (1 - S_l_i) * rho_g`. Conserved under no-flow like the
    /// water mass.
    pub fn total_gas_mass(&self, x: &[f64]) -> f64 {
        let rho = self.fluids.gas_density;
        (0..self.grid.n_cells())
            .map(|i| self.grid.cell_volume(i) * self.porosity * (1.0 - x[2 * i + 1]) * rho)
            .sum()
    }

    // --- pointwise property helpers (plain f64, SI units) ---

    /// Effective saturation `S_e = (S_l - S_r)/(1 - S_r)`, clamped to `[0, 1]`.
    #[inline]
    fn se_of_sl(&self, s_l: f64) -> f64 {
        let sr = self.curves.residual_saturation();
        ((s_l - sr) / (1.0 - sr)).clamp(0.0, 1.0)
    }

    /// Retention-curve effective saturation `S_e(P_c)` at capillary pressure
    /// `pc` (Pa).
    #[inline]
    fn se_of_pc(&self, pc: f64) -> f64 {
        self.curves
            .effective_saturation(Pressure::new::<pascal>(pc))
            .get::<ratio>()
    }

    /// Liquid relative permeability `k_{rl}(S_e)` in `[0, 1]` from the
    /// characteristic curves.
    #[inline]
    fn krl(&self, se: f64) -> f64 {
        self.curves
            .relative_permeability(Ratio::new::<ratio>(se))
            .get::<ratio>()
    }

    /// Gas relative permeability — quadratic Corey non-wetting curve with zero
    /// residual gas saturation: `k_{rg}(S_e) = (1 - S_e)^2` in `[0, 1]`.
    #[inline]
    fn krg(&self, se: f64) -> f64 {
        let sg_eff = (1.0 - se).clamp(0.0, 1.0);
        sg_eff * sg_eff
    }

    /// Capillary pressure `P_c(S_l)` (Pa), obtained by numerically inverting the
    /// retention curve `S_e = effective_saturation(P_c)`.
    ///
    /// `S_e >= 1` (fully saturated) returns `0`. Otherwise, since
    /// `effective_saturation` is monotone non-increasing in `P_c`, the inverse
    /// is bracketed by exponential growth from `P_c = 0` and refined by
    /// bisection. The target `S_e` is floored at `1e-9` so the suction stays
    /// finite at the residual limit.
    pub fn capillary_pressure(&self, s_l: f64) -> f64 {
        let se = self.se_of_sl(s_l);
        if se >= 1.0 {
            return 0.0;
        }
        let target = se.max(1.0e-9);

        // Bracket: se_of_pc(0) = 1 >= target; grow hi until se_of_pc(hi) <= target.
        let mut hi = 1.0_f64;
        let mut guard = 0;
        while self.se_of_pc(hi) > target {
            hi *= 4.0;
            guard += 1;
            if guard > 60 || hi > 1.0e13 {
                break;
            }
        }
        let mut lo = 0.0_f64;
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if self.se_of_pc(mid) > target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    /// Two-point **water** mass flux across a face (kg/s, positive owner ->
    /// neighbour). Mobility upstream-weighted on the liquid potential.
    ///
    /// `se_o`/`se_n` are the owner/neighbour effective saturations (for the
    /// upstream `k_{rl}`); `t_geom` is the geometric transmissibility (m).
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn water_flux(
        &self,
        p_l_o: f64,
        se_o: f64,
        z_o: f64,
        p_l_n: f64,
        se_n: f64,
        z_n: f64,
        t_geom: f64,
    ) -> f64 {
        let rho = self.fluids.liquid_density;
        let dphi = (p_l_o - p_l_n) + rho * self.gravity * (z_o - z_n);
        let kr_up = if dphi >= 0.0 {
            self.krl(se_o)
        } else {
            self.krl(se_n)
        };
        let mobility = self.permeability * kr_up / self.fluids.liquid_viscosity;
        rho * mobility * t_geom * dphi
    }

    /// Two-point **gas** mass flux across a face (kg/s, positive owner ->
    /// neighbour). Mobility upstream-weighted on the gas potential
    /// `P_g = P_l + P_c(S_l)`.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn gas_flux(
        &self,
        p_g_o: f64,
        se_o: f64,
        z_o: f64,
        p_g_n: f64,
        se_n: f64,
        z_n: f64,
        t_geom: f64,
    ) -> f64 {
        let rho = self.fluids.gas_density;
        let dphi = (p_g_o - p_g_n) + rho * self.gravity * (z_o - z_n);
        let kr_up = if dphi >= 0.0 {
            self.krg(se_o)
        } else {
            self.krg(se_n)
        };
        let mobility = self.permeability * kr_up / self.fluids.gas_viscosity;
        rho * mobility * t_geom * dphi
    }

    /// Residual pair `(R_water_i, R_gas_i)` (kg/s) for one cell: phase
    /// accumulation (backward Euler) plus net phase outflow across internal and
    /// boundary faces. A positive flux leaves the cell.
    fn cell_residual(&self, i: usize, x: &[f64]) -> (f64, f64) {
        let v = self.grid.cell_volume(i);
        let phi = self.porosity;

        let s_l_i = x[2 * i + 1];
        let s_l_old = self.s_old[i];

        // Accumulation (constant densities => linear in saturation).
        // Water:  V/dt * phi * rho_l * (S_l - S_l_old)
        // Gas:    V/dt * phi * rho_g * ((1 - S_l) - (1 - S_l_old))
        let acc_w = v / self.dt * phi * self.fluids.liquid_density * (s_l_i - s_l_old);
        let acc_g =
            v / self.dt * phi * self.fluids.gas_density * ((1.0 - s_l_i) - (1.0 - s_l_old));

        let p_l_i = x[2 * i];
        let se_i = self.se_of_sl(s_l_i);
        let p_g_i = p_l_i + self.capillary_pressure(s_l_i);

        let mut flux_w = 0.0;
        let mut flux_g = 0.0;

        // Internal faces incident to cell i.
        for &f in &self.cell_faces[i] {
            let c = &self.grid.connections()[f];
            let (o, n) = (c.owner, c.neighbour);
            let s_l_o = x[2 * o + 1];
            let s_l_n = x[2 * n + 1];
            let se_o = self.se_of_sl(s_l_o);
            let se_n = self.se_of_sl(s_l_n);
            let p_l_o = x[2 * o];
            let p_l_n = x[2 * n];
            let p_g_o = p_l_o + self.capillary_pressure(s_l_o);
            let p_g_n = p_l_n + self.capillary_pressure(s_l_n);
            let t = c.geometric_transmissibility;

            let mw = self.water_flux(p_l_o, se_o, self.z[o], p_l_n, se_n, self.z[n], t);
            let mg = self.gas_flux(p_g_o, se_o, self.z[o], p_g_n, se_n, self.z[n], t);
            // Positive m leaves the owner and enters the neighbour.
            if i == o {
                flux_w += mw;
                flux_g += mg;
            } else {
                flux_w -= mw;
                flux_g -= mg;
            }
        }

        // Boundary faces.
        for &b in &self.cell_boundary_faces[i] {
            let bf = &self.grid.boundary_faces()[b];
            match &self.bc[location_index(&bf.location)] {
                None | Some(TwoPhaseBoundaryKind::NoFlow) => {}
                Some(TwoPhaseBoundaryKind::Dirichlet {
                    liquid_pressure,
                    liquid_saturation,
                }) => {
                    // Ghost cell at the boundary-face centroid.
                    let z_bc = match bf.location {
                        BoundaryLocation::ZMin => self.z[i] - bf.distance,
                        BoundaryLocation::ZMax => self.z[i] + bf.distance,
                        _ => self.z[i],
                    };
                    let t = bf.area / bf.distance;
                    let se_bc = self.se_of_sl(*liquid_saturation);
                    let p_g_bc = *liquid_pressure + self.capillary_pressure(*liquid_saturation);
                    // Cell is the owner side; ghost is the neighbour side.
                    let mw =
                        self.water_flux(p_l_i, se_i, self.z[i], *liquid_pressure, se_bc, z_bc, t);
                    let mg = self.gas_flux(p_g_i, se_i, self.z[i], p_g_bc, se_bc, z_bc, t);
                    flux_w += mw; // positive leaves the cell
                    flux_g += mg;
                }
            }
        }

        (acc_w + flux_w, acc_g + flux_g)
    }
}

impl BlockNonlinearSystem for TwoPhaseFlow {
    fn n_cells(&self) -> usize {
        self.grid.n_cells()
    }

    fn dof_per_cell(&self) -> usize {
        2
    }

    fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
        self.grid.ldu_addressing()
    }

    fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
        let n = self.grid.n_cells();
        if x.len() != 2 * n || out.len() != 2 * n {
            return Err(PflotranError::InvalidInput(
                "two-phase residual: state/output length must be 2 * n_cells".into(),
            ));
        }
        // Per-cell residual pairs are independent — evaluate in parallel (op-v6s.14).
        let this = &*self;
        out.par_chunks_mut(2).enumerate().for_each(|(i, chunk)| {
            let (rw, rg) = this.cell_residual(i, x);
            chunk[0] = rw;
            chunk[1] = rg;
        });
        Ok(())
    }

    fn assemble_jacobian(
        &mut self,
        x: &[f64],
        jac: &mut BlockLduMatrix,
    ) -> Result<(), PflotranError> {
        let n = self.grid.n_cells();
        if x.len() != 2 * n {
            return Err(PflotranError::InvalidInput(
                "two-phase jacobian: state length must be 2 * n_cells".into(),
            ));
        }

        let this = &*self;

        // Base residual pairs over all cells (parallel; independent per cell).
        let r0: Vec<(f64, f64)> = (0..n).into_par_iter().map(|i| this.cell_residual(i, x)).collect();

        jac.zero();

        // Local finite-difference columns. Perturbing local unknown `d` of cell
        // `j` (d = 0 -> P_l, d = 1 -> S_l) affects only the residual of `j` and
        // its face-neighbours (the two-point stencil). Each column's writes land
        // in disjoint 2x2 block entries, so the expensive residual evaluations run
        // in parallel (op-v6s.14) and the results scatter serially — identical to
        // the serial assembly.
        type ColumnContrib = (usize, usize, f64, f64, Vec<(usize, bool, f64, f64)>);
        let contribs: Vec<ColumnContrib> = (0..2 * n)
            .into_par_iter()
            .map(|col| {
                let j = col / 2;
                let d = col % 2;
                let mut xp = x.to_vec();
                let idx = 2 * j + d;
                let h = 1.0e-8 * (1.0 + x[idx].abs());
                xp[idx] = x[idx] + h;

                let (rwj, rgj) = this.cell_residual(j, &xp);
                let dj0 = (rwj - r0[j].0) / h;
                let dj1 = (rgj - r0[j].1) / h;

                let faces: Vec<(usize, bool, f64, f64)> = this.cell_faces[j]
                    .iter()
                    .map(|&f| {
                        let c = &this.grid.connections()[f];
                        if j == c.owner {
                            let (rwn, rgn) = this.cell_residual(c.neighbour, &xp);
                            (f, true, (rwn - r0[c.neighbour].0) / h, (rgn - r0[c.neighbour].1) / h)
                        } else {
                            let (rwo, rgo) = this.cell_residual(c.owner, &xp);
                            (f, false, (rwo - r0[c.owner].0) / h, (rgo - r0[c.owner].1) / h)
                        }
                    })
                    .collect();
                (j, d, dj0, dj1, faces)
            })
            .collect();

        for (j, d, dj0, dj1, faces) in contribs {
            jac.add_diag(j, 0, d, dj0);
            jac.add_diag(j, 1, d, dj1);
            for (f, is_lower, v0, v1) in faces {
                if is_lower {
                    jac.add_lower(f, 0, d, v0);
                    jac.add_lower(f, 1, d, v1);
                } else {
                    jac.add_upper(f, 0, d, v0);
                    jac.add_upper(f, 1, d, v1);
                }
            }
        }

        if jac.diag.iter().any(|v| !v.is_finite()) {
            return Err(PflotranError::Convergence(
                "non-finite two-phase Jacobian diagonal (check curves near saturation)".into(),
            ));
        }
        Ok(())
    }
}

/// A transient two-phase immiscible simulation driven by the block Newton solver.
///
/// Holds the [`TwoPhaseFlow`] problem, a [`BlockNewtonSolver`], the evolving
/// interleaved state (`[P_l, S_l]` per cell, length `2 * n_cells`), and the
/// current time. Each [`step`](Self::step) sets the timestep and previous state
/// on the problem, then solves the coupled block nonlinear system.
pub struct TwoPhaseSimulation {
    problem: TwoPhaseFlow,
    solver: BlockNewtonSolver,
    state: Vec<f64>,
    time: f64,
}

impl TwoPhaseSimulation {
    /// Assemble a simulation from a problem, a block-Newton configuration, and a
    /// uniform initial liquid pressure (Pa) and liquid saturation
    /// (dimensionless). The initial state is the interleaved
    /// `[P_l0, S_l0, P_l0, S_l0, …]`.
    pub fn new(
        problem: TwoPhaseFlow,
        config: BlockNewtonConfig,
        initial_liquid_pressure: f64,
        initial_liquid_saturation: f64,
    ) -> Self {
        let n = problem.grid().n_cells();
        let mut state = vec![0.0; 2 * n];
        for c in 0..n {
            state[2 * c] = initial_liquid_pressure;
            state[2 * c + 1] = initial_liquid_saturation;
        }
        Self {
            problem,
            solver: BlockNewtonSolver::new(config),
            state,
            time: 0.0,
        }
    }

    /// Advance one backward-Euler step of size `dt` (s), returning the block
    /// Newton report.
    ///
    /// On success the internal state is updated to the converged solution and
    /// the simulation time advances by `dt`. On a failed nonlinear solve the
    /// state is left unchanged and the [`PflotranError::Convergence`] is
    /// returned (this driver does not adapt `dt`; the caller may retry smaller).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `dt` is non-finite or `<= 0`;
    /// [`PflotranError::Convergence`] if the block Newton solve fails.
    pub fn step(&mut self, dt: f64) -> Result<BlockNewtonReport, PflotranError> {
        if !(dt > 0.0) || !dt.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "timestep dt must be finite and > 0, got {dt}"
            )));
        }
        self.problem.set_timestep(dt);
        self.problem.set_previous(&self.state);

        let mut trial = self.state.clone();
        let report = self.solver.solve(&mut self.problem, &mut trial)?;
        self.state = trial;
        self.time += dt;
        Ok(report)
    }

    /// Current simulation time (s).
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Current liquid-pressure field `P_l` (Pa), de-interleaved, length
    /// `n_cells`.
    pub fn liquid_pressure(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[2 * c])
            .collect()
    }

    /// Current liquid-saturation field `S_l` (dimensionless), de-interleaved,
    /// length `n_cells`.
    pub fn liquid_saturation(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[2 * c + 1])
            .collect()
    }

    /// The underlying problem (read-only), e.g. for grid access or the
    /// phase-mass diagnostics.
    pub fn problem(&self) -> &TwoPhaseFlow {
        &self.problem
    }

    /// The current interleaved state vector `[P_l, S_l]` per cell (length
    /// `2 * n_cells`).
    pub fn state(&self) -> &[f64] {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::{BrooksCorey, VanGenuchten};
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(x: f64) -> Length {
        Length::new::<meter>(x)
    }

    fn vg_curves() -> CharacteristicCurves {
        CharacteristicCurves::VanGenuchten(VanGenuchten::new(2.0e-4, 2.0, 0.1).unwrap())
    }

    /// A convergence configuration tuned for these small demonstrators. The
    /// relative tolerance (`1e-7`) is the operative one for driven problems; the
    /// tiny absolute tolerance handles the exactly-stationary case (where the
    /// initial residual is identically zero). Both are far tighter than the
    /// qualitative assertions the tests make, so a converged solve implies phase
    /// masses are conserved to well under the asserted bound.
    fn test_config() -> BlockNewtonConfig {
        BlockNewtonConfig {
            max_iterations: 100,
            abs_tol: 1.0e-12,
            rel_tol: 1.0e-7,
            linear_tolerance: 1.0e-8,
            linear_max_iter: 5000,
            max_backtracks: 25,
        }
    }

    /// **Capillary-equilibrium / single-phase-limit stationarity:** a closed
    /// no-flow column with a UNIFORM initial state must stay exactly put — both
    /// phase residuals are zero (no fluxes, no accumulation), so a step leaves
    /// the field unchanged.
    #[test]
    fn closed_uniform_column_is_stationary() {
        let grid = CartesianGrid::uniform(8, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap();
        let problem = TwoPhaseFlow::new(
            grid,
            vg_curves(),
            TwoPhaseFluids::air_water(),
            0.35,
            1.0e-12,
            0.0, // no gravity
            vec![],
        )
        .unwrap();
        let p0 = 1.0e5;
        let s0 = 0.6;
        let mut sim = TwoPhaseSimulation::new(problem, test_config(), p0, s0);

        let report = sim.step(100.0).unwrap();
        assert!(report.converged);
        for &p in &sim.liquid_pressure() {
            assert!((p - p0).abs() < 1.0e-3, "pressure drifted: {p} vs {p0}");
        }
        for &s in &sim.liquid_saturation() {
            assert!((s - s0).abs() < 1.0e-9, "saturation drifted: {s} vs {s0}");
        }
    }

    /// **Phase-mass conservation:** a closed (no-flow) domain with a NON-uniform
    /// initial saturation actively redistributes fluid, yet total water mass and
    /// total gas mass must each be conserved to solver tolerance across several
    /// implicit steps.
    #[test]
    fn closed_domain_conserves_phase_masses() {
        let n = 10usize;
        let grid = CartesianGrid::uniform(n, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap();
        let mut problem = TwoPhaseFlow::new(
            grid,
            vg_curves(),
            TwoPhaseFluids::air_water(),
            0.35,
            1.0e-12,
            0.0,
            vec![], // all no-flow
        )
        .unwrap();
        problem.set_timestep(20.0);

        // Non-uniform initial saturation (a ramp) drives capillary redistribution.
        let mut state = vec![0.0; 2 * n];
        for c in 0..n {
            state[2 * c] = 1.0e5;
            state[2 * c + 1] = 0.4 + 0.4 * (c as f64) / (n as f64 - 1.0); // 0.4 .. 0.8
        }
        let s_init: Vec<f64> = (0..n).map(|c| state[2 * c + 1]).collect();

        let water0 = problem.total_water_mass(&state);
        let gas0 = problem.total_gas_mass(&state);

        let solver = BlockNewtonSolver::new(test_config());
        for _ in 0..5 {
            problem.set_previous(&state);
            let mut trial = state.clone();
            solver
                .solve(&mut problem, &mut trial)
                .expect("closed two-phase step converges");
            state = trial;
        }

        // The saturation field must actually have moved (else the test is vacuous).
        let moved = (0..n)
            .map(|c| (state[2 * c + 1] - s_init[c]).abs())
            .fold(0.0, f64::max);
        assert!(moved > 1.0e-4, "saturation did not redistribute (moved={moved})");

        let water1 = problem.total_water_mass(&state);
        let gas1 = problem.total_gas_mass(&state);
        let rel_w = (water1 - water0).abs() / water0;
        let rel_g = (gas1 - gas0).abs() / gas0;
        assert!(rel_w < 1.0e-5, "water mass not conserved: rel drift {rel_w:e}");
        assert!(rel_g < 1.0e-5, "gas mass not conserved: rel drift {rel_g:e}");
    }

    /// **Imbibition runs and stays bounded:** a column that starts drier, with a
    /// wetter, higher-liquid-pressure Dirichlet face at one end, must draw water
    /// in — the liquid saturation near the boundary rises toward the boundary
    /// value while every cell stays within `[S_r, 1]` and Newton converges.
    #[test]
    fn imbibition_column_wets_up_and_stays_bounded() {
        let n = 10usize;
        let grid = CartesianGrid::uniform(n, 1, 1, m(0.05), m(1.0), m(1.0)).unwrap();
        let curves = vg_curves();
        let sr = curves.residual_saturation();
        let boundary = vec![TwoPhaseBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: TwoPhaseBoundaryKind::Dirichlet {
                liquid_pressure: 1.2e5,
                liquid_saturation: 0.9,
            },
        }];
        let problem = TwoPhaseFlow::new(
            grid,
            curves,
            TwoPhaseFluids::air_water(),
            0.35,
            5.0e-12,
            0.0,
            boundary,
        )
        .unwrap();

        let s_init = 0.4;
        let mut sim = TwoPhaseSimulation::new(problem, test_config(), 1.0e5, s_init);

        for _ in 0..8 {
            let report = sim.step(20.0).expect("imbibition step converges");
            assert!(report.converged);
            let s = sim.liquid_saturation();
            for &v in &s {
                assert!(
                    (sr - 1.0e-6..=1.0 + 1.0e-6).contains(&v),
                    "saturation out of [S_r, 1]: {v}"
                );
            }
        }

        let s = sim.liquid_saturation();
        // The cell nearest the wet boundary must have imbibed.
        assert!(s[0] > s_init + 1.0e-3, "inflow cell did not wet up: {}", s[0]);
        // The wet boundary cell holds the saturation maximum (the front sits at
        // the boundary), and the profile is monotone decreasing away from it to
        // a small numerical slack.
        let s_max = s.iter().cloned().fold(f64::MIN, f64::max);
        assert!((s[0] - s_max).abs() < 1.0e-9, "front is not at the wet boundary");
        for w in s.windows(2) {
            assert!(w[1] <= w[0] + 1.0e-3, "saturation not monotone along x: {w:?}");
        }
    }

    /// Constructor validation: fluid properties and material parameters are
    /// range-checked.
    #[test]
    fn constructors_reject_bad_inputs() {
        assert!(TwoPhaseFluids::new(-1.0, 1.0e-3, 1.2, 1.8e-5).is_err());
        assert!(TwoPhaseFluids::new(1000.0, 0.0, 1.2, 1.8e-5).is_err());
        assert!(TwoPhaseFluids::new(1000.0, 1.0e-3, 1.2, f64::NAN).is_err());
        assert!(TwoPhaseFluids::air_water().liquid_density > 0.0);

        let grid = CartesianGrid::uniform(3, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap();
        // Bad porosity.
        assert!(TwoPhaseFlow::new(
            grid.clone(),
            vg_curves(),
            TwoPhaseFluids::air_water(),
            0.0,
            1.0e-12,
            0.0,
            vec![]
        )
        .is_err());
        // Bad permeability.
        assert!(TwoPhaseFlow::new(
            grid.clone(),
            vg_curves(),
            TwoPhaseFluids::air_water(),
            0.35,
            -1.0e-12,
            0.0,
            vec![]
        )
        .is_err());
        // Negative gravity.
        assert!(TwoPhaseFlow::new(
            grid,
            vg_curves(),
            TwoPhaseFluids::air_water(),
            0.35,
            1.0e-12,
            -9.8,
            vec![]
        )
        .is_err());
    }

    /// The capillary-pressure inversion round-trips the retention curve: for a
    /// Brooks–Corey model, `effective_saturation(P_c(S_l)) == S_e(S_l)`.
    #[test]
    fn capillary_pressure_inverts_retention_curve() {
        let curves = CharacteristicCurves::BrooksCorey(BrooksCorey::new(1.0e-4, 2.0, 0.1).unwrap());
        let grid = CartesianGrid::uniform(1, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let problem = TwoPhaseFlow::new(
            grid,
            curves,
            TwoPhaseFluids::air_water(),
            0.35,
            1.0e-12,
            0.0,
            vec![],
        )
        .unwrap();
        for &s_l in &[0.3, 0.5, 0.7, 0.85] {
            let se = problem.se_of_sl(s_l);
            let pc = problem.capillary_pressure(s_l);
            let se_back = problem.se_of_pc(pc);
            assert!(
                (se_back - se).abs() < 1.0e-4,
                "P_c inversion round-trip failed at S_l={s_l}: se={se}, se_back={se_back}"
            );
        }
        // Saturated limit maps to zero capillary pressure.
        assert_eq!(problem.capillary_pressure(1.0), 0.0);
    }
}
