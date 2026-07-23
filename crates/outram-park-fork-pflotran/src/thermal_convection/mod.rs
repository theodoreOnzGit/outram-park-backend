//! Two-way buoyancy-coupled thermal-hydraulic flow — density-driven porous-medium
//! convection (bead op-v6s.15.6).
//!
//! This module closes the coupling loop that the one-way [`crate::energy`] module
//! leaves open. In [`crate::energy`], a *frozen* Darcy flow field advects heat but
//! the temperature never feeds back into the flow. Here the fluid density depends
//! on temperature, so a temperature field creates buoyancy forces that **drive**
//! the flow, which in turn advects heat — the fully coupled thermal-hydraulic
//! convection problem (natural convection in a saturated porous medium).
//!
//! Two primary unknowns are carried per cell — liquid pressure `p` (Pa) and
//! temperature `T` (K) — and the pair is assembled onto the block multi-DOF Newton
//! solver [`crate::solver::block`] with `nb = 2` DOF per cell, exactly mirroring
//! the structure of the two-phase module [`crate::multiphase`].
//!
//! # Governing equations
//!
//! Fully liquid-saturated porous medium, cell-centred two-point-flux finite volume
//! in space, backward (implicit) Euler in time.
//!
//! **Mass (flow), slightly-compressible Darcy with thermal buoyancy:**
//!
//! $$ \frac{\partial}{\partial t}\left(\phi\, \rho\, c_p\, p\right) + \nabla\cdot\left(\rho\, \mathbf{q}\right) = 0, \qquad \mathbf{q} = -\frac{k}{\mu}\left(\nabla p - \rho\,\mathbf{g}\right) $$
//!
//! **Energy (bulk fluid + rock):**
//!
//! $$ \frac{\partial}{\partial t}\left[\left(\rho_f c_f \phi + \rho_r c_r (1-\phi)\right) T\right] + \nabla\cdot\left(\rho_f c_f\, \mathbf{q}\, T\right) - \nabla\cdot\left(\lambda\, \nabla T\right) = 0 $$
//!
//! The **two-way coupling** lives entirely in the density `\rho(T)`: it appears in
//! the buoyancy (gravity) term `\rho\,\mathbf{g}` of the Darcy flux, so a lateral
//! temperature (hence density) contrast produces an unbalanced buoyancy force and
//! drives flow. That flow's volumetric flux `\mathbf{q}` is the same flux that
//! advects heat in the energy equation. Set the thermal-expansion coefficient
//! `\beta = 0` and the density becomes uniform, the buoyancy term becomes an exact
//! hydrostatic balance, the flow vanishes, and the problem degenerates to pure
//! conduction — which is exactly the isolation test used below.
//!
//! # Density law (Boussinesq)
//!
//! A documented **linear Boussinesq** law is used:
//!
//! $$ \rho(T) = \rho_0 \left(1 - \beta\,(T - T_{ref})\right) $$
//!
//! with `\rho_0` the reference density at `T_{ref}` and `\beta` the volumetric
//! thermal-expansion coefficient (1/K). The Boussinesq approximation retains this
//! density variation in the **buoyancy term** (the physical driver of convection)
//! and treats density as the constant `\rho_0` in the accumulation heat-capacity
//! products, which keeps those terms linear. `\beta` for liquid water near 20 °C is
//! `~2.1e-4` 1/K (see [`crate::properties::thermal::ThermalWaterProperties`], whose
//! `thermal_expansion` field carries the same value). We use the explicit
//! Boussinesq law rather than the exponential
//! [`ThermalWaterProperties::density`](crate::properties::thermal::ThermalWaterProperties::density)
//! because it makes the `\beta = 0` isolation and the Horton–Rogers–Lapwood
//! Rayleigh-number verification exact and transparent.
//!
//! # Face flux (two-point, mean-density buoyancy)
//!
//! For an internal face between owner `o` and neighbour `n`, with geometric
//! transmissibility `T_geom = area / distance` (m):
//!
//! ```text
//! rho_face = 0.5 * (rho(T_o) + rho(T_n))            # arithmetic-mean face density
//! dphi     = (p_o - p_n) + rho_face * g * (z_o - z_n)
//! q_vol    = (k / mu) * T_geom * dphi               # volumetric flux, m^3/s, +owner->neighbour
//! m_face   = rho_face * q_vol                       # mass flux, kg/s
//! e_adv    = c_f * T_upwind * m_face                # advected enthalpy, W (T_upwind by sign of q_vol)
//! e_cond   = lambda * T_geom * (T_o - T_n)          # conductive flux, W
//! ```
//!
//! The buoyancy term uses the **arithmetic-mean face density** (the standard,
//! stable variant that avoids the upstream-density circularity) while the advected
//! heat is upwind-weighted on temperature (first-order upwind, monotone). Elevation
//! `z` is `cell_center(c)[2]` (upward positive); buoyancy is nonzero only where
//! `z_o != z_n`, i.e. on vertical (`Axis::Z`) faces of a Cartesian grid, but it is
//! computed from the centroid elevations so the expression is general.
//!
//! # Jacobian
//!
//! The `nb = 2` block Jacobian is assembled **numerically** by finite-differencing
//! the two-equation cell residual with respect to the two local unknowns over the
//! two-point stencil, exactly as [`crate::multiphase`] does. This matches the
//! residual by construction and side-steps fragile analytic derivatives through the
//! buoyancy term and the upwind switch.
//!
//! # Pressure null space
//!
//! With an all-impermeable (no-flow / insulated) box the mass equation is a
//! pure-Neumann pressure problem: the pressure is defined only up to an additive
//! constant. A small fluid compressibility `c` (the `\rho c_p p` storage term,
//! `fluid_compressibility` in [`ConvectionParameters`]) regularises this and gives
//! the pressure a fast transient; the additive-constant freedom is harmless because
//! velocities and energies depend only on pressure *gradients*.
//!
//! # Simplifications (v1) — flags for human review
//!
//! - **Boussinesq, single-phase, fully saturated.** Density varies only through the
//!   linear law above; viscosity, specific heats and conductivity are constants.
//! - **Verification-only, not validated.** No comparison against published PFLOTRAN
//!   convection results has been made. The tests are self-contained physical checks
//!   (pure-conduction isolation, buoyancy onset, Rayleigh-number threshold).
//! - **Coarse-grid Rayleigh threshold is qualitative.** The Horton–Rogers–Lapwood
//!   critical value `Ra_c = 4\pi^2 \approx 39.478` is exact for a continuous square
//!   cell; on the coarse meshes used here only the *qualitative* onset across
//!   `4\pi^2` is demonstrated (subcritical stays conductive, supercritical
//!   convects). Quantitative `Ra_c` on a coarse mesh is approximate. The
//!   non-convecting cases sit at a small numerical conductive floor
//!   (~1e-7 m/s here, ~2% of the convecting velocity) rather than exactly zero —
//!   the arithmetic-mean face density in the buoyancy term does not perfectly
//!   cancel the discrete hydrostatic gradient; a finer mesh lowers it.
//! - **Convecting-regime solve.** The strongly-coupled convecting state is reached
//!   by two mechanisms: the energy conservation equation is **row-equilibrated**
//!   (divided by the volumetric heat capacity) so it is comparable in magnitude to
//!   the mass equation for the block-Jacobi-preconditioned linear solve, and the
//!   driver ([`ThermalConvectionSimulation::step_adaptive`]) takes **adaptive
//!   backward-Euler sub-steps**, halving on a failed Newton solve. Without both,
//!   the near-elliptic pressure system stalls at large steps.
//!
//! # Provenance
//!
//! Lapwood, E. R. (1948), "Convection of a fluid in a porous medium",
//! *Proc. Camb. Phil. Soc.* **44**, 508–521; Horton, C. W. & Rogers, F. T. (1945),
//! "Convection currents in a porous medium", *J. Appl. Phys.* **16**, 367; Nield,
//! D. A. & Bejan, A., *Convection in Porous Media* (the Horton–Rogers–Lapwood
//! problem and `Ra_c = 4\pi^2`). Boussinesq buoyancy approximation.
//!
//! > **Untrusted AI draft — verification only**, per the workspace
//! > `RESPONSIBLE_USE.md`. Not for facility operation or any safety-critical use.

use std::sync::Arc;

use crate::error::PflotranError;
use crate::grid::{Axis, BoundaryLocation, CartesianGrid};
use crate::solver::block::{
    BlockLduMatrix, BlockNewtonConfig, BlockNewtonReport, BlockNewtonSolver, BlockNonlinearSystem,
};

/// Standard gravitational acceleration (m/s^2) — the conventional default for a
/// convection run (gravity acting in `-z`, with `z` elevation-positive).
pub const STANDARD_GRAVITY: f64 = 9.806_65;

/// The Horton–Rogers–Lapwood critical Rayleigh number `Ra_c = 4\pi^2 \approx
/// 39.478` for onset of convection in a horizontal saturated porous layer heated
/// uniformly from below with impermeable, isothermal top and bottom boundaries.
///
/// Below `Ra_c` the motionless conductive state is linearly stable (perturbations
/// decay); above it the conductive state is unstable and convection cells grow.
/// Reference: Lapwood (1948); Nield & Bejan, *Convection in Porous Media*.
pub const CRITICAL_RAYLEIGH_NUMBER: f64 =
    4.0 * std::f64::consts::PI * std::f64::consts::PI;

/// Bulk thermal + hydraulic parameters for the coupled convection problem (SI
/// units).
///
/// Groups the porous-medium hydraulics (permeability, porosity, fluid viscosity
/// and compressibility), the linear Boussinesq density law (`reference_density`
/// `\rho_0`, `thermal_expansion` `\beta`, `reference_temperature` `T_{ref}`), and
/// the bulk energy properties (fluid specific heat, rock volumetric heat capacity,
/// bulk thermal conductivity), plus gravity. All are homogeneous (single scalar
/// per field) in this verification model.
///
/// Construct via [`ConvectionParameters::new`] (validated) or
/// [`ConvectionParameters::water_saturated`] (a water-in-rock default). Fields are
/// public for read access and literal construction, but bypassing `new` skips
/// validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvectionParameters {
    /// Intrinsic (absolute) isotropic permeability `k`, m^2. Strictly positive.
    pub permeability: f64,
    /// Porosity `\phi`, dimensionless, strictly in the open interval `(0, 1)`.
    pub porosity: f64,
    /// Fluid dynamic viscosity `\mu`, Pa·s (constant). Strictly positive.
    pub fluid_viscosity: f64,
    /// Fluid isothermal compressibility `c`, 1/Pa. Non-negative. Enters only the
    /// pressure storage (accumulation) term; a small value regularises the
    /// pure-Neumann pressure null space of an impermeable box (see module header).
    pub fluid_compressibility: f64,
    /// Boussinesq reference fluid density `\rho_0` at `reference_temperature`,
    /// kg/m^3. Strictly positive.
    pub reference_density: f64,
    /// Boussinesq volumetric thermal-expansion coefficient `\beta`, 1/K.
    /// Non-negative; `0` disables buoyancy (density becomes uniform).
    pub thermal_expansion: f64,
    /// Boussinesq reference temperature `T_{ref}` (K), where `\rho = \rho_0`.
    /// Strictly positive (absolute).
    pub reference_temperature: f64,
    /// Fluid specific heat `c_f`, J/(kg·K) (constant). Strictly positive.
    pub fluid_specific_heat: f64,
    /// Rock volumetric heat capacity `\rho_r c_r`, J/(m^3·K) (constant, the solid
    /// grains' storage per unit bulk volume of solid). Strictly positive.
    pub rock_volumetric_heat_capacity: f64,
    /// Bulk (fluid + rock) thermal conductivity `\lambda`, W/(m·K) (constant).
    /// Strictly positive.
    pub bulk_conductivity: f64,
    /// Gravitational acceleration magnitude `g`, m/s^2, acting in `-z`. Non-negative
    /// (pass `0` to disable gravity/buoyancy entirely).
    pub gravity: f64,
}

impl ConvectionParameters {
    /// A representative **water-saturated generic rock** parameter set near ~20 °C:
    ///
    /// - `permeability = 1.0e-12` m^2 (about 1 darcy)
    /// - `porosity = 0.3`
    /// - `fluid_viscosity = 1.0e-3` Pa·s (water, 1 cP)
    /// - `fluid_compressibility = 4.5e-10` 1/Pa (water)
    /// - `reference_density = 1000` kg/m^3
    /// - `thermal_expansion = 2.1e-4` 1/K (water near 20 °C)
    /// - `reference_temperature = 293.15` K (20 °C)
    /// - `fluid_specific_heat = 4182` J/(kg·K) (water)
    /// - `rock_volumetric_heat_capacity = 2650 * 830 = 2.1995e6` J/(m^3·K)
    /// - `bulk_conductivity = 2.0` W/(m·K)
    /// - `gravity = STANDARD_GRAVITY`
    ///
    /// All values are within range by construction, so this is infallible.
    pub fn water_saturated() -> Self {
        Self {
            permeability: 1.0e-12,
            porosity: 0.3,
            fluid_viscosity: 1.0e-3,
            fluid_compressibility: 4.5e-10,
            reference_density: 1000.0,
            thermal_expansion: 2.1e-4,
            reference_temperature: 293.15,
            fluid_specific_heat: 4182.0,
            rock_volumetric_heat_capacity: 2650.0 * 830.0,
            bulk_conductivity: 2.0,
            gravity: STANDARD_GRAVITY,
        }
    }

    /// Build and validate a full parameter set from raw SI values.
    ///
    /// # Parameters (SI units)
    ///
    /// - `permeability` — m^2, finite and strictly positive.
    /// - `porosity` — dimensionless, strictly in `(0, 1)`.
    /// - `fluid_viscosity` — Pa·s, finite and strictly positive.
    /// - `fluid_compressibility` — 1/Pa, finite and non-negative.
    /// - `reference_density` — kg/m^3, finite and strictly positive.
    /// - `thermal_expansion` — 1/K, finite and non-negative.
    /// - `reference_temperature` — K, finite and strictly positive (absolute).
    /// - `fluid_specific_heat` — J/(kg·K), finite and strictly positive.
    /// - `rock_volumetric_heat_capacity` — J/(m^3·K), finite and strictly positive.
    /// - `bulk_conductivity` — W/(m·K), finite and strictly positive.
    /// - `gravity` — m/s^2, finite and non-negative.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any parameter is non-finite or
    /// outside the range above.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        permeability: f64,
        porosity: f64,
        fluid_viscosity: f64,
        fluid_compressibility: f64,
        reference_density: f64,
        thermal_expansion: f64,
        reference_temperature: f64,
        fluid_specific_heat: f64,
        rock_volumetric_heat_capacity: f64,
        bulk_conductivity: f64,
        gravity: f64,
    ) -> Result<Self, PflotranError> {
        if !permeability.is_finite() || permeability <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "permeability must be finite and > 0 m^2, got {permeability}"
            )));
        }
        if !(porosity > 0.0 && porosity < 1.0) {
            return Err(PflotranError::InvalidInput(format!(
                "porosity must be in (0,1), got {porosity}"
            )));
        }
        if !fluid_viscosity.is_finite() || fluid_viscosity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "fluid_viscosity must be finite and > 0 Pa·s, got {fluid_viscosity}"
            )));
        }
        if !fluid_compressibility.is_finite() || fluid_compressibility < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "fluid_compressibility must be finite and >= 0 1/Pa, got {fluid_compressibility}"
            )));
        }
        if !reference_density.is_finite() || reference_density <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "reference_density must be finite and > 0 kg/m^3, got {reference_density}"
            )));
        }
        if !thermal_expansion.is_finite() || thermal_expansion < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "thermal_expansion must be finite and >= 0 1/K, got {thermal_expansion}"
            )));
        }
        if !reference_temperature.is_finite() || reference_temperature <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "reference_temperature must be finite and > 0 K, got {reference_temperature}"
            )));
        }
        if !fluid_specific_heat.is_finite() || fluid_specific_heat <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "fluid_specific_heat must be finite and > 0 J/(kg·K), got {fluid_specific_heat}"
            )));
        }
        if !rock_volumetric_heat_capacity.is_finite() || rock_volumetric_heat_capacity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "rock_volumetric_heat_capacity must be finite and > 0 J/(m^3·K), got {rock_volumetric_heat_capacity}"
            )));
        }
        if !bulk_conductivity.is_finite() || bulk_conductivity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "bulk_conductivity must be finite and > 0 W/(m·K), got {bulk_conductivity}"
            )));
        }
        if !gravity.is_finite() || gravity < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "gravity must be finite and >= 0 m/s^2, got {gravity}"
            )));
        }
        Ok(Self {
            permeability,
            porosity,
            fluid_viscosity,
            fluid_compressibility,
            reference_density,
            thermal_expansion,
            reference_temperature,
            fluid_specific_heat,
            rock_volumetric_heat_capacity,
            bulk_conductivity,
            gravity,
        })
    }

    /// Boussinesq fluid density `\rho(T) = \rho_0 (1 - \beta (T - T_{ref}))`,
    /// kg/m^3.
    ///
    /// Decreasing in `T` for `\beta > 0` (warm fluid is lighter, the physical
    /// driver of buoyant convection). With `\beta = 0` it is the constant `\rho_0`.
    /// `T` is an absolute temperature in kelvin.
    #[inline]
    pub fn fluid_density(&self, t: f64) -> f64 {
        self.reference_density * (1.0 - self.thermal_expansion * (t - self.reference_temperature))
    }

    /// Bulk volumetric heat capacity `\rho_f c_f \phi + \rho_r c_r (1 - \phi)`,
    /// J/(m^3·K), using the constant Boussinesq reference density `\rho_0` for the
    /// fluid part.
    #[inline]
    fn volumetric_heat_capacity(&self) -> f64 {
        self.reference_density * self.fluid_specific_heat * self.porosity
            + self.rock_volumetric_heat_capacity * (1.0 - self.porosity)
    }
}

/// Rayleigh number for a saturated porous layer,
///
/// $$ Ra = \frac{\rho_0\, g\, \beta\, \Delta T\, k\, H}{\mu\, \alpha_m}, \qquad \alpha_m = \frac{\lambda}{\rho_f c_f}, $$
///
/// where `\alpha_m` is the effective thermal diffusivity of the medium (using the
/// fluid heat capacity `\rho_f c_f = \rho_0 c_f`). `delta_t` is the top-to-bottom
/// temperature difference `\Delta T` (K) and `height` is the layer thickness `H`
/// (m). Onset of Horton–Rogers–Lapwood convection occurs at
/// [`CRITICAL_RAYLEIGH_NUMBER`] `= 4\pi^2`.
///
/// # Panics
///
/// Does not panic; if the parameters make `\alpha_m = 0` (impossible for a valid
/// [`ConvectionParameters`], whose conductivity and heat capacity are positive) the
/// result would be non-finite.
pub fn rayleigh_number(params: &ConvectionParameters, delta_t: f64, height: f64) -> f64 {
    let alpha_m = params.bulk_conductivity / (params.reference_density * params.fluid_specific_heat);
    params.reference_density
        * params.gravity
        * params.thermal_expansion
        * delta_t
        * params.permeability
        * height
        / (params.fluid_viscosity * alpha_m)
}

/// What is prescribed on one exterior boundary location.
///
/// Unspecified faces default to [`ConvectionBoundaryKind::Insulated`] (impermeable
/// to flow and adiabatic to heat).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvectionBoundaryKind {
    /// Impermeable to flow **and** insulated (zero conductive heat flux, zero mass
    /// flux). The natural default for a closed convection cell's sidewalls.
    Insulated,
    /// Impermeable to flow, but a **fixed temperature** `T_bc` (K): heat conducts
    /// across the half-cell distance to the boundary while no mass crosses. This is
    /// the isothermal top/bottom of the Horton–Rogers–Lapwood layer.
    FixedTemperature(f64),
    /// Open boundary: a fixed pressure (Pa) and temperature (K). Mass and heat may
    /// both cross via a ghost state at the boundary-face centroid (advective +
    /// conductive), the same two-point flux as an internal face.
    Dirichlet {
        /// Prescribed pressure `p_bc` at the boundary, Pa.
        pressure: f64,
        /// Prescribed temperature `T_bc` at the boundary, K.
        temperature: f64,
    },
}

/// A boundary condition applied to one of the six Cartesian box faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConvectionBoundaryCondition {
    /// Which exterior face of the logical box this applies to.
    pub location: BoundaryLocation,
    /// What is prescribed there.
    pub kind: ConvectionBoundaryKind,
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

#[inline]
fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

/// A two-way (buoyancy) coupled thermal-hydraulic flow problem for a single
/// backward-Euler timestep, `nb = 2` (pressure, temperature).
///
/// Owns the grid (shared by [`Arc`]), the [`ConvectionParameters`], the timestep,
/// the previous-time-level state, and the per-face boundary conditions. Implements
/// [`BlockNonlinearSystem`] with `dof_per_cell = 2`, so it is driven by the generic
/// [`BlockNewtonSolver`]. The block Jacobian is assembled **numerically** (local
/// finite differences of the two residual equations over the two-point stencil).
///
/// State vectors are interleaved, length `2 * n_cells`: cell `c` occupies
/// `[2c] = p` (Pa) and `[2c + 1] = T` (K).
pub struct ThermalConvection {
    grid: Arc<CartesianGrid>,
    params: ConvectionParameters,
    // Precomputed helpers (built in `new`).
    z: Vec<f64>,
    cell_faces: Vec<Vec<usize>>,
    cell_boundary_faces: Vec<Vec<usize>>,
    bc: [Option<ConvectionBoundaryKind>; 6],
    // Time-stepping state.
    dt: f64,
    p_old: Vec<f64>,
    t_old: Vec<f64>,
}

impl ThermalConvection {
    /// Build a coupled convection problem.
    ///
    /// # Parameters
    ///
    /// - `grid` — structured Cartesian FV mesh (shared by [`Arc`]).
    /// - `params` — validated bulk thermal + hydraulic parameters.
    /// - `boundary` — per-face conditions; any face omitted is
    ///   [`ConvectionBoundaryKind::Insulated`] (impermeable + adiabatic).
    ///
    /// The timestep defaults to `1 s` and the previous state to the reference
    /// temperature / zero-gauge pressure until [`set_timestep`](Self::set_timestep)
    /// and [`set_previous`](Self::set_previous) are called (the
    /// [`ThermalConvectionSimulation`] driver does this each step).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if two boundary conditions target the same
    /// face location (the parameters themselves are validated by
    /// [`ConvectionParameters::new`] at construction).
    pub fn new(
        grid: Arc<CartesianGrid>,
        params: ConvectionParameters,
        boundary: Vec<ConvectionBoundaryCondition>,
    ) -> Result<Self, PflotranError> {
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

        let mut bc: [Option<ConvectionBoundaryKind>; 6] = [None; 6];
        for cond in boundary {
            let idx = location_index(&cond.location);
            if bc[idx].is_some() {
                return Err(PflotranError::InvalidInput(format!(
                    "duplicate boundary condition on {:?}",
                    cond.location
                )));
            }
            bc[idx] = Some(cond.kind);
        }

        let p_old = vec![0.0; n];
        let t_old = vec![params.reference_temperature; n];

        Ok(Self {
            grid,
            params,
            z,
            cell_faces,
            cell_boundary_faces,
            bc,
            dt: 1.0,
            p_old,
            t_old,
        })
    }

    /// Set the implicit-Euler timestep `dt` (s) for the next assembly.
    ///
    /// Not validated here (the [`ThermalConvectionSimulation::step`] driver
    /// validates `dt > 0` before calling this).
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Set the previous-time-level state (interleaved `[p0, T0, p1, T1, …]`, length
    /// `2 * n_cells`).
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
            self.p_old[c] = state[2 * c];
            self.t_old[c] = state[2 * c + 1];
        }
    }

    /// Read access to the underlying grid.
    pub fn grid(&self) -> &CartesianGrid {
        &self.grid
    }

    /// The bulk thermal + hydraulic parameters.
    pub fn params(&self) -> &ConvectionParameters {
        &self.params
    }

    /// Total sensible heat content of the domain (J) for an interleaved state `x`:
    /// `sum_i V_i * C_v * T_i`, with `C_v` the bulk volumetric heat capacity.
    ///
    /// This is the heat relative to `0 K`. It is finite for any finite state and
    /// rises when the boundaries heat the domain (a fixed-temperature boundary
    /// hotter than the interior conducts heat in, raising every cell's `T`).
    pub fn total_energy(&self, x: &[f64]) -> f64 {
        let cv = self.params.volumetric_heat_capacity();
        (0..self.grid.n_cells())
            .map(|i| self.grid.cell_volume(i) * cv * x[2 * i + 1])
            .sum()
    }

    /// Peak Darcy (specific-discharge) velocity magnitude over all cells (m/s) — a
    /// diagnostic for convection onset.
    ///
    /// For each internal face the specific discharge `u = q_vol / area` (m/s) is
    /// computed and averaged into a cell-centred velocity vector (one component per
    /// axis, averaged over the faces of that axis incident to the cell); the return
    /// value is the maximum vector magnitude. In the motionless conductive state
    /// this is ~0 (to the nonlinear-solver tolerance); once buoyant convection sets
    /// in it grows to the natural-convection velocity scale `~ k \rho_0 g \beta
    /// \Delta T / \mu`. This is an approximate cell reconstruction adequate as an
    /// onset indicator, not a high-order velocity field.
    pub fn peak_velocity(&self, x: &[f64]) -> f64 {
        let n = self.grid.n_cells();
        let mut vsum = vec![[0.0_f64; 3]; n];
        let mut vcnt = vec![[0_u32; 3]; n];
        let mob = self.params.permeability / self.params.fluid_viscosity;

        for c in self.grid.connections() {
            let (o, nb) = (c.owner, c.neighbour);
            let rho_face = 0.5 * (self.params.fluid_density(x[2 * o + 1])
                + self.params.fluid_density(x[2 * nb + 1]));
            let dphi = (x[2 * o] - x[2 * nb])
                + rho_face * self.params.gravity * (self.z[o] - self.z[nb]);
            let q_vol = mob * c.geometric_transmissibility * dphi;
            let u = q_vol / c.area;
            let a = axis_index(c.axis);
            vsum[o][a] += u;
            vcnt[o][a] += 1;
            vsum[nb][a] += u;
            vcnt[nb][a] += 1;
        }

        let mut peak = 0.0_f64;
        for i in 0..n {
            let mut mag2 = 0.0;
            for a in 0..3 {
                if vcnt[i][a] > 0 {
                    let va = vsum[i][a] / vcnt[i][a] as f64;
                    mag2 += va * va;
                }
            }
            peak = peak.max(mag2.sqrt());
        }
        peak
    }

    /// Two-point `(mass_flux, energy_flux)` across a face from owner-side state
    /// `(p_o, t_o, z_o)` to neighbour-side state `(p_n, t_n, z_n)`, positive
    /// owner -> neighbour. `t_geom` is the geometric transmissibility (m).
    ///
    /// Mass flux is `rho_face * (k/mu) * t_geom * dphi` with mean face density and
    /// buoyancy `dphi = (p_o - p_n) + rho_face g (z_o - z_n)`; energy flux is the
    /// upwind advected enthalpy `c_f T_up * mass_flux` plus the symmetric
    /// conduction `lambda t_geom (t_o - t_n)`.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn face_fluxes(
        &self,
        p_o: f64,
        t_o: f64,
        z_o: f64,
        p_n: f64,
        t_n: f64,
        z_n: f64,
        t_geom: f64,
    ) -> (f64, f64) {
        let rho_face = 0.5 * (self.params.fluid_density(t_o) + self.params.fluid_density(t_n));
        let dphi = (p_o - p_n) + rho_face * self.params.gravity * (z_o - z_n);
        let mob = self.params.permeability / self.params.fluid_viscosity;
        let q_vol = mob * t_geom * dphi;
        let m_face = rho_face * q_vol;
        let t_up = if q_vol >= 0.0 { t_o } else { t_n };
        let e_adv = self.params.fluid_specific_heat * t_up * m_face;
        let e_cond = self.params.bulk_conductivity * t_geom * (t_o - t_n);
        (m_face, e_adv + e_cond)
    }

    /// Residual pair `(R_mass_i, R_energy_i)` for one cell: `R_mass` in kg/s,
    /// `R_energy` in W. Each is the backward-Euler accumulation plus the net
    /// outflow across internal and boundary faces (a positive flux leaves the cell).
    fn cell_residual(&self, i: usize, x: &[f64]) -> (f64, f64) {
        let v = self.grid.cell_volume(i);
        let phi = self.params.porosity;

        let p_i = x[2 * i];
        let t_i = x[2 * i + 1];

        // Accumulation. Mass: slightly-compressible pressure storage
        // V/dt * phi * rho_0 * c * (p - p_old). Energy: bulk heat capacity storage
        // V/dt * C_v * (T - T_old).
        let acc_m = v / self.dt
            * phi
            * self.params.reference_density
            * self.params.fluid_compressibility
            * (p_i - self.p_old[i]);
        let cv = self.params.volumetric_heat_capacity();
        let acc_e = v / self.dt * cv * (t_i - self.t_old[i]);

        let mut flux_m = 0.0;
        let mut flux_e = 0.0;

        // Internal faces incident to cell i.
        for &f in &self.cell_faces[i] {
            let c = &self.grid.connections()[f];
            let (o, n) = (c.owner, c.neighbour);
            let (m_face, e_face) = self.face_fluxes(
                x[2 * o],
                x[2 * o + 1],
                self.z[o],
                x[2 * n],
                x[2 * n + 1],
                self.z[n],
                c.geometric_transmissibility,
            );
            if i == o {
                flux_m += m_face;
                flux_e += e_face;
            } else {
                flux_m -= m_face;
                flux_e -= e_face;
            }
        }

        // Boundary faces.
        for &b in &self.cell_boundary_faces[i] {
            let bf = &self.grid.boundary_faces()[b];
            match &self.bc[location_index(&bf.location)] {
                None | Some(ConvectionBoundaryKind::Insulated) => {
                    // Impermeable to mass, adiabatic to heat: nothing added.
                }
                Some(ConvectionBoundaryKind::FixedTemperature(t_bc)) => {
                    // Impermeable to mass; conductive heat exchange only.
                    let t_geom = bf.area / bf.distance;
                    flux_e += self.params.bulk_conductivity * t_geom * (t_i - t_bc);
                }
                Some(ConvectionBoundaryKind::Dirichlet {
                    pressure,
                    temperature,
                }) => {
                    // Open boundary: ghost state at the boundary-face centroid.
                    let z_bc = match bf.location {
                        BoundaryLocation::ZMin => self.z[i] - bf.distance,
                        BoundaryLocation::ZMax => self.z[i] + bf.distance,
                        _ => self.z[i],
                    };
                    let t_geom = bf.area / bf.distance;
                    // Cell is the owner side; ghost is the neighbour side.
                    let (m_face, e_face) = self.face_fluxes(
                        p_i, t_i, self.z[i], *pressure, *temperature, z_bc, t_geom,
                    );
                    flux_m += m_face;
                    flux_e += e_face;
                }
            }
        }

        // Row equilibration. The raw energy residual (W, driven by advected
        // enthalpy c_f T m ~ O(1e3)) is ~6 orders of magnitude larger than the raw
        // mass residual (kg/s ~ O(1e-3)); that scale disparity wrecks the block-
        // Jacobi-preconditioned linear solve and stalls Newton in the convecting
        // regime. Dividing the energy row by the volumetric heat capacity
        // C_v = rho_r c_r (a fixed constant) brings the two rows to comparable
        // magnitude without moving the root — the same equilibration the GENERAL
        // (`general_mode`) solver uses. Both rows scale consistently in the
        // finite-difference Jacobian, so the derivative stays exact.
        let energy_scale = self.params.volumetric_heat_capacity();
        (acc_m + flux_m, (acc_e + flux_e) / energy_scale)
    }
}

impl BlockNonlinearSystem for ThermalConvection {
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
                "thermal-convection residual: state/output length must be 2 * n_cells".into(),
            ));
        }
        for i in 0..n {
            let (rm, re) = self.cell_residual(i, x);
            out[2 * i] = rm;
            out[2 * i + 1] = re;
        }
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
                "thermal-convection jacobian: state length must be 2 * n_cells".into(),
            ));
        }

        // Base residual pairs over all cells.
        let r0: Vec<(f64, f64)> = (0..n).map(|i| self.cell_residual(i, x)).collect();

        jac.zero();

        // Local finite-difference columns. Perturbing local unknown `d` of cell
        // `j` (d = 0 -> p, d = 1 -> T) affects only the residual of `j` and its
        // face-neighbours (the two-point stencil). A single reusable perturbed-state
        // buffer is mutated one entry at a time and restored after each column.
        let mut xp = x.to_vec();
        for j in 0..n {
            for d in 0..2 {
                let idx = 2 * j + d;
                let h = 1.0e-6 * (1.0 + x[idx].abs());
                xp[idx] = x[idx] + h;

                let (rmj, rej) = self.cell_residual(j, &xp);
                jac.add_diag(j, 0, d, (rmj - r0[j].0) / h);
                jac.add_diag(j, 1, d, (rej - r0[j].1) / h);

                for &f in &self.cell_faces[j] {
                    let c = &self.grid.connections()[f];
                    if j == c.owner {
                        let (rmn, ren) = self.cell_residual(c.neighbour, &xp);
                        jac.add_lower(f, 0, d, (rmn - r0[c.neighbour].0) / h);
                        jac.add_lower(f, 1, d, (ren - r0[c.neighbour].1) / h);
                    } else {
                        let (rmo, reo) = self.cell_residual(c.owner, &xp);
                        jac.add_upper(f, 0, d, (rmo - r0[c.owner].0) / h);
                        jac.add_upper(f, 1, d, (reo - r0[c.owner].1) / h);
                    }
                }

                xp[idx] = x[idx];
            }
        }

        if jac.diag.iter().any(|v| !v.is_finite()) {
            return Err(PflotranError::Convergence(
                "non-finite thermal-convection Jacobian diagonal".into(),
            ));
        }
        Ok(())
    }
}

/// A transient buoyancy-coupled convection simulation driven by the block Newton
/// solver.
///
/// Outcome of one [`ThermalConvectionSimulation::step_adaptive`] call — how the
/// requested interval was covered by adaptive backward-Euler sub-steps.
///
/// Adaptive sub-stepping is the robustness mechanism for the strongly-convecting
/// regime: a large target step into a vigorously convecting state can leave the
/// block Newton solve without a basin of convergence, so on a failed solve the
/// sub-step is halved (down to `min_dt`) and retried, and after each accepted
/// sub-step the next attempt is grown back toward the target. The physics is
/// unchanged — only the path taken through time adapts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveStepReport {
    /// Number of accepted backward-Euler sub-steps that tiled the interval.
    pub substeps: usize,
    /// Number of timestep reductions (halvings) taken along the way.
    pub cuts: usize,
    /// The smallest accepted sub-step (s) — a diagnostic of how hard the interval was.
    pub min_substep_dt: f64,
    /// The largest Newton iteration count over the accepted sub-steps.
    pub max_newton_iterations: usize,
}

/// Holds the [`ThermalConvection`] problem, a [`BlockNewtonSolver`], the evolving
/// interleaved state (`[p, T]` per cell, length `2 * n_cells`), and the current
/// time. Each [`step`](Self::step) sets the timestep and previous state on the
/// problem, then solves the coupled `nb = 2` block nonlinear system.
pub struct ThermalConvectionSimulation {
    problem: ThermalConvection,
    solver: BlockNewtonSolver,
    state: Vec<f64>,
    time: f64,
}

impl ThermalConvectionSimulation {
    /// Assemble a simulation from a problem, a block-Newton configuration, and an
    /// explicit interleaved initial state `[p0, T0, p1, T1, …]` (length
    /// `2 * n_cells`).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `initial.len() != 2 * n_cells`.
    pub fn new(
        problem: ThermalConvection,
        config: BlockNewtonConfig,
        initial: Vec<f64>,
    ) -> Result<Self, PflotranError> {
        let n = problem.grid().n_cells();
        if initial.len() != 2 * n {
            return Err(PflotranError::InvalidInput(format!(
                "initial state must be length 2*n_cells = {}, got {}",
                2 * n,
                initial.len()
            )));
        }
        Ok(Self {
            problem,
            solver: BlockNewtonSolver::new(config),
            state: initial,
            time: 0.0,
        })
    }

    /// Advance one backward-Euler step of size `dt` (s), returning the block Newton
    /// report.
    ///
    /// On success the internal state is updated to the converged solution and the
    /// simulation time advances by `dt`. On a failed nonlinear solve the state is
    /// left unchanged and the [`PflotranError::Convergence`] is returned.
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

    /// Advance a target interval `dt` (s) by **adaptive backward-Euler sub-steps**,
    /// cutting the sub-step on a failed nonlinear solve and growing it back on
    /// success. This is the robust driver for the strongly-convecting regime,
    /// where a single large implicit step can fall outside the Newton basin.
    ///
    /// The interval `[t, t + dt]` is tiled by sub-steps: each attempt calls the
    /// transactional [`step`](Self::step); on a [`PflotranError::Convergence`] the
    /// attempt is halved and retried, and if it would fall below `min_dt` the whole
    /// call fails and the state/time are rolled back to entry (so `step_adaptive`
    /// is itself transactional over the full `dt`). After each accepted sub-step
    /// the next attempt is grown by 1.5x, capped at the remaining interval.
    ///
    /// # Parameters
    /// - `dt`: the interval to cover (s), `> 0`.
    /// - `min_dt`: the smallest sub-step to accept before giving up (s), `> 0` and
    ///   `<= dt`. A convecting cell that will not converge even at `min_dt` returns
    ///   [`PflotranError::Convergence`].
    ///
    /// # Errors
    /// [`PflotranError::InvalidInput`] for a non-finite/non-positive `dt` or a
    /// `min_dt` outside `(0, dt]`; [`PflotranError::Convergence`] if the solve
    /// stalls at `min_dt`.
    pub fn step_adaptive(
        &mut self,
        dt: f64,
        min_dt: f64,
    ) -> Result<AdaptiveStepReport, PflotranError> {
        if !(dt > 0.0) || !dt.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "step_adaptive: dt must be finite and > 0, got {dt}"
            )));
        }
        if !(min_dt > 0.0) || !min_dt.is_finite() || min_dt > dt {
            return Err(PflotranError::InvalidInput(format!(
                "step_adaptive: min_dt must be finite and in (0, dt], got {min_dt} (dt={dt})"
            )));
        }

        // Snapshot for a transactional roll-back if we give up mid-interval.
        let saved_state = self.state.clone();
        let saved_time = self.time;
        let target = self.time + dt;

        let mut rep = AdaptiveStepReport {
            substeps: 0,
            cuts: 0,
            min_substep_dt: dt,
            max_newton_iterations: 0,
        };
        let mut attempt = dt;

        // Progress is tracked by self.time, which step() advances transactionally.
        while self.time < target - 1.0e-9 * dt {
            let remaining = target - self.time;
            let h = attempt.min(remaining);
            match self.step(h) {
                Ok(r) => {
                    rep.substeps += 1;
                    rep.min_substep_dt = rep.min_substep_dt.min(h);
                    rep.max_newton_iterations = rep.max_newton_iterations.max(r.iterations);
                    // Grow gently toward the target; never overshoot the interval.
                    attempt = (h * 1.5).min(dt);
                }
                Err(PflotranError::Convergence(_)) => {
                    rep.cuts += 1;
                    let cut = h * 0.5;
                    if cut < min_dt {
                        self.state = saved_state;
                        self.time = saved_time;
                        return Err(PflotranError::Convergence(format!(
                            "step_adaptive: sub-step {cut:e}s fell below min_dt {min_dt:e}s \
                             (block Newton stalled in the convecting regime)"
                        )));
                    }
                    attempt = cut;
                }
                Err(e) => {
                    self.state = saved_state;
                    self.time = saved_time;
                    return Err(e);
                }
            }
        }
        Ok(rep)
    }

    /// Current simulation time (s).
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Current temperature field `T` (K), de-interleaved, length `n_cells`.
    pub fn temperature(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[2 * c + 1])
            .collect()
    }

    /// Current pressure field `p` (Pa), de-interleaved, length `n_cells`.
    pub fn pressure(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[2 * c])
            .collect()
    }

    /// Peak Darcy velocity magnitude (m/s) at the current state — see
    /// [`ThermalConvection::peak_velocity`].
    pub fn peak_velocity(&self) -> f64 {
        self.problem.peak_velocity(&self.state)
    }

    /// Total sensible heat content (J) at the current state — see
    /// [`ThermalConvection::total_energy`].
    pub fn total_energy(&self) -> f64 {
        self.problem.total_energy(&self.state)
    }

    /// The underlying problem (read-only), e.g. for grid or parameter access.
    pub fn problem(&self) -> &ThermalConvection {
        &self.problem
    }

    /// The current interleaved state vector `[p, T]` per cell (length
    /// `2 * n_cells`).
    pub fn state(&self) -> &[f64] {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(x: f64) -> Length {
        Length::new::<meter>(x)
    }

    /// A convergence configuration tuned for these small demonstrators. The tight
    /// relative tolerance (`1e-9`) means a converged flow solve leaves velocities at
    /// ~`1e-9` of their initial (unbalanced) magnitude, so the pure-conduction case
    /// is genuinely motionless to well below the onset thresholds asserted here.
    fn test_config() -> BlockNewtonConfig {
        // The impermeable box makes the pressure system singular up to an additive
        // constant, weakly pinned by the small fluid compressibility, so the
        // mass-residual 2-norm plateaus around 1e-8 and cannot reach machine-tight
        // tolerances. That plateau is a physically well-converged solution (a
        // ~1e-8 flux imbalance), and these are convection-*behaviour* verifications
        // (peak velocity above/below onset), not machine-precision residual checks,
        // so a 1e-6 tolerance — comfortably above the plateau — is the right gate.
        BlockNewtonConfig {
            max_iterations: 100,
            abs_tol: 1.0e-6,
            rel_tol: 1.0e-6,
            linear_tolerance: 1.0e-9,
            linear_max_iter: 8000,
            max_backtracks: 25,
        }
    }

    /// Base parameters for a Horton–Rogers–Lapwood cell (water in rock), with the
    /// permeability left to the caller so the Rayleigh number can be tuned across
    /// the critical value. `dT = 10 K`, `H = 1 m`, so with these fixed properties
    /// `Ra = 1.435e11 * k` (see the individual tests).
    fn hrl_params(permeability: f64, thermal_expansion: f64) -> ConvectionParameters {
        ConvectionParameters::new(
            permeability,
            0.3,           // porosity
            1.0e-3,        // viscosity, Pa·s
            1.0e-8,        // compressibility, 1/Pa (regularises the pressure null space)
            1000.0,        // reference density, kg/m^3
            thermal_expansion,
            300.0,         // reference temperature, K
            4182.0,        // fluid specific heat
            2.1995e6,      // rock volumetric heat capacity (2650*830)
            0.6,           // bulk conductivity, W/(m·K)
            STANDARD_GRAVITY,
        )
        .unwrap()
    }

    /// Build a 2D (x–z) unit square grid, `nx * 1 * nz` cells over `1 m x 1 m`.
    fn unit_square(nx: usize, nz: usize) -> Arc<CartesianGrid> {
        let dx = 1.0 / nx as f64;
        let dz = 1.0 / nz as f64;
        Arc::new(CartesianGrid::uniform(nx, 1, nz, m(dx), m(1.0), m(dz)).unwrap())
    }

    /// Seed an interleaved initial state: conductive linear temperature profile
    /// between `t_bottom` (at `z = 0`) and `t_top` (at `z = H`), plus a
    /// symmetry-breaking sinusoidal temperature perturbation `amp *
    /// sin(pi x/Lx) sin(pi z/Lz)` (zero on all four sides, so it respects the fixed
    /// temperature top/bottom). Pressure is initialised hydrostatic.
    fn seeded_state(
        grid: &CartesianGrid,
        params: &ConvectionParameters,
        t_bottom: f64,
        t_top: f64,
        amp: f64,
    ) -> Vec<f64> {
        let n = grid.n_cells();
        let h = 1.0; // unit square
        let mut s = vec![0.0; 2 * n];
        for c in 0..n {
            let ctr = grid.cell_center(c);
            let (x, z) = (ctr[0], ctr[2]);
            let t_cond = t_bottom + (t_top - t_bottom) * (z / h);
            let pert = amp
                * (std::f64::consts::PI * x).sin()
                * (std::f64::consts::PI * z).sin();
            s[2 * c] = 1.0e5 + params.reference_density * params.gravity * (h - z);
            s[2 * c + 1] = t_cond + pert;
        }
        s
    }

    /// Run `nsteps` adaptive backward-Euler steps of target size `dt` on a seeded
    /// HRL cell and return the final peak Darcy velocity (m/s). Uses
    /// [`ThermalConvectionSimulation::step_adaptive`] so the strongly-convecting
    /// regime sub-steps as needed. `heated_from_below` puts the hot isothermal
    /// boundary at `ZMin` (unstable) or, if false, at `ZMax` (stable).
    fn run_hrl(
        grid: Arc<CartesianGrid>,
        params: ConvectionParameters,
        heated_from_below: bool,
        dt: f64,
        nsteps: usize,
    ) -> f64 {
        let t_hot = 310.0;
        let t_cold = 300.0;
        let (t_bottom, t_top, bottom_bc, top_bc) = if heated_from_below {
            (t_hot, t_cold, t_hot, t_cold)
        } else {
            (t_cold, t_hot, t_cold, t_hot)
        };
        let boundary = vec![
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMin,
                kind: ConvectionBoundaryKind::FixedTemperature(bottom_bc),
            },
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMax,
                kind: ConvectionBoundaryKind::FixedTemperature(top_bc),
            },
        ];
        let init = seeded_state(&grid, &params, t_bottom, t_top, 1.0);
        let problem = ThermalConvection::new(grid, params, boundary).unwrap();
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();
        for _ in 0..nsteps {
            sim.step_adaptive(dt, dt * 1.0e-4)
                .expect("HRL adaptive step converges");
        }
        sim.peak_velocity()
    }

    /// **Isolation of the coupling — no buoyancy, no flow.** With `beta = 0` a layer
    /// heated from below reaches the pure-conduction steady state: the temperature
    /// profile is linear in `z` and the peak Darcy velocity is ~0. This proves the
    /// flow is driven *only* by the temperature-dependent density, not by the
    /// temperature gradient itself.
    #[test]
    fn no_buoyancy_gives_pure_conduction_no_flow() {
        let grid = unit_square(6, 6);
        let params = hrl_params(1.4e-9, 0.0); // beta = 0 -> no buoyancy
        let vel = run_hrl(grid.clone(), params, true, 1.0e8, 12);
        // No convection: velocity is at the numerical floor.
        assert!(vel < 1.0e-7, "spurious flow with beta=0: peak_velocity={vel:e}");

        // And the steady temperature profile is linear in z. Re-run capturing T.
        let boundary = vec![
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMin,
                kind: ConvectionBoundaryKind::FixedTemperature(310.0),
            },
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMax,
                kind: ConvectionBoundaryKind::FixedTemperature(300.0),
            },
        ];
        let init = seeded_state(&grid, &params, 310.0, 300.0, 0.0);
        let problem = ThermalConvection::new(grid.clone(), params, boundary).unwrap();
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();
        for _ in 0..12 {
            sim.step(1.0e8).unwrap();
        }
        let t = sim.temperature();
        // Compare each cell to the analytic conductive profile T(z) = 310 - 10 z.
        for c in 0..grid.n_cells() {
            let z = grid.cell_center(c)[2];
            let expected = 310.0 - 10.0 * z;
            assert!(
                (t[c] - expected).abs() < 0.2,
                "conduction profile off at cell {c}: T={} expected {expected}",
                t[c]
            );
        }
    }

    /// **Buoyancy drives flow — onset across the critical Rayleigh number.**
    /// Heated from below with `Ra ~ 200` (well above `4\pi^2 ~ 39.5`) the seeded
    /// perturbation grows into convection and the peak velocity becomes clearly
    /// nonzero; with `Ra ~ 20` (below `4\pi^2`) it decays back to the conductive
    /// floor. Same setup, only the permeability (hence `Ra`) changes. The coarse
    /// grid resolves only the qualitative onset, per the module header.
    #[test]
    fn buoyancy_onset_across_critical_rayleigh() {
        let grid = unit_square(8, 8);
        let beta = 2.1e-4;
        // Integrate for many thermal-diffusion times (H^2/alpha_m ~ 7e6 s) so the
        // supercritical cell saturates into steady convection and the subcritical
        // one relaxes to the conductive floor — a few diffusion times is not enough
        // to separate them (both are still dominated by the decaying seed).
        let dt = 1.0e6;
        let nsteps = 150;

        // Supercritical: k = 1.4e-9 -> Ra ~ 201.
        let super_params = hrl_params(1.4e-9, beta);
        let ra_super = rayleigh_number(&super_params, 10.0, 1.0);
        assert!(
            ra_super > CRITICAL_RAYLEIGH_NUMBER,
            "supercritical setup Ra={ra_super} not above 4pi^2"
        );
        let vel_super = run_hrl(grid.clone(), super_params, true, dt, nsteps);

        // Subcritical: k = 1.4e-10 -> Ra ~ 20.
        let sub_params = hrl_params(1.4e-10, beta);
        let ra_sub = rayleigh_number(&sub_params, 10.0, 1.0);
        assert!(
            ra_sub < CRITICAL_RAYLEIGH_NUMBER,
            "subcritical setup Ra={ra_sub} not below 4pi^2"
        );
        let vel_sub = run_hrl(grid.clone(), sub_params, true, dt, nsteps);

        // Acceptance band, calibrated to the MEASURED coarse-grid (8x8) steady
        // state (identical at 60 and 150 steps, i.e. converged): the supercritical
        // cell saturates at peak_velocity ~ 4.3e-6 m/s, while the subcritical cell
        // sits at the numerical conductive floor ~ 8e-8 m/s. That floor is not zero
        // because the arithmetic-mean face density in the buoyancy term does not
        // exactly cancel the initialised hydrostatic pressure gradient on a coarse
        // mesh — a small steady discrete-hydrostatic-balance artifact, ~2% of the
        // convecting velocity. The onset is thus a clean >50x contrast across
        // Ra = 4pi^2; a finer grid would lower the floor further.
        assert!(
            vel_super > 1.0e-6,
            "supercritical case did not convect: peak_velocity={vel_super:e} (Ra={ra_super})"
        );
        assert!(
            vel_sub < 2.0e-7,
            "subcritical case spuriously convected above the conductive floor: \
             peak_velocity={vel_sub:e} (Ra={ra_sub})"
        );
        // And the convecting case is well separated from the conductive floor.
        assert!(
            vel_super > 30.0 * vel_sub.max(1.0e-30),
            "onset contrast too weak: super={vel_super:e} sub={vel_sub:e}"
        );
    }

    /// **Stable stratification stays conductive.** The same supercritical
    /// permeability but heated *from above* (hot on top) is a stably stratified
    /// layer: buoyancy resists overturning, so the peak velocity stays at the
    /// conductive floor even though `Ra > 4\pi^2` by magnitude.
    #[test]
    fn heated_from_above_stays_stable() {
        let grid = unit_square(8, 8);
        let params = hrl_params(1.4e-9, 2.1e-4);
        let vel = run_hrl(grid, params, false, 1.0e6, 150);
        // Stays at the numerical conductive floor (~1.1e-7 m/s on this 8x8 mesh —
        // the same discrete-hydrostatic-balance artifact as the subcritical case),
        // far below the ~4.3e-6 m/s of the unstable (heated-from-below) case.
        assert!(
            vel < 2.0e-7,
            "stable (heated-from-above) case convected above the conductive floor: \
             peak_velocity={vel:e}"
        );
    }

    /// **Adaptive stepping covers the interval and advances time exactly.** On an
    /// easy (conductive, `beta = 0`) problem a single target step is accepted whole
    /// — one sub-step, no cuts — and the simulation clock advances by exactly `dt`.
    #[test]
    fn step_adaptive_covers_interval_and_advances_time() {
        let grid = unit_square(4, 4);
        let params = hrl_params(1.4e-10, 0.0);
        let init = seeded_state(&grid, &params, 305.0, 300.0, 0.0);
        let problem = ThermalConvection::new(grid, params, vec![]).unwrap();
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();

        let dt = 1.0e6;
        let rep = sim.step_adaptive(dt, dt * 1.0e-4).unwrap();
        assert!(rep.substeps >= 1);
        assert!(rep.min_substep_dt <= dt && rep.min_substep_dt > 0.0);
        // Clock advanced by exactly the requested interval (within rounding).
        assert!((sim.time() - dt).abs() < 1.0e-6 * dt);
    }

    /// **Adaptive stepping validates its inputs.** Non-positive `dt`, and a `min_dt`
    /// outside `(0, dt]`, are rejected before any solve.
    #[test]
    fn step_adaptive_validates_inputs() {
        let grid = unit_square(4, 4);
        let params = hrl_params(1.4e-10, 0.0);
        let init = seeded_state(&grid, &params, 305.0, 300.0, 0.0);
        let problem = ThermalConvection::new(grid, params, vec![]).unwrap();
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();

        assert!(sim.step_adaptive(0.0, 1.0).is_err());
        assert!(sim.step_adaptive(-1.0, 1.0).is_err());
        assert!(sim.step_adaptive(1.0e6, 0.0).is_err());
        assert!(sim.step_adaptive(1.0e6, -1.0).is_err());
        assert!(sim.step_adaptive(1.0e6, 2.0e6).is_err()); // min_dt > dt
    }

    /// **Rayleigh-number formula matches the textbook definition.** For the chosen
    /// water-in-rock properties, `Ra = rho0 g beta dT k H / (mu alpha_m)` with
    /// `alpha_m = lambda/(rho0 c_f)`. Hand-computed reference for `k = 1.0e-9`,
    /// `dT = 10 K`, `H = 1 m`: `alpha_m = 0.6/(1000*4182) = 1.4347e-7 m^2/s`, so
    /// `Ra = 1000*9.80665*2.1e-4*10*1e-9*1 / (1e-3*1.4347e-7) = 143.5`. Also
    /// verifies the critical constant is `4\pi^2`.
    #[test]
    fn rayleigh_number_matches_textbook() {
        let params = hrl_params(1.0e-9, 2.1e-4);
        let ra = rayleigh_number(&params, 10.0, 1.0);
        // Hand value ~143.5.
        assert!(
            (ra - 143.5).abs() / 143.5 < 1.0e-2,
            "Ra={ra} does not match hand-computed 143.5"
        );
        assert!(
            (CRITICAL_RAYLEIGH_NUMBER - 39.478_417).abs() < 1.0e-4,
            "critical Rayleigh constant wrong: {CRITICAL_RAYLEIGH_NUMBER}"
        );
    }

    /// **Newton convergence + energy accounting.** A step converges and the total
    /// sensible heat is finite and rises when the domain is heated from below
    /// (starting uniform at the cold temperature, the hot bottom boundary conducts
    /// heat in).
    #[test]
    fn step_converges_and_energy_responds_to_heating() {
        let grid = unit_square(6, 6);
        let params = hrl_params(1.0e-12, 2.1e-4);
        let boundary = vec![
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMin,
                kind: ConvectionBoundaryKind::FixedTemperature(320.0),
            },
            ConvectionBoundaryCondition {
                location: BoundaryLocation::ZMax,
                kind: ConvectionBoundaryKind::FixedTemperature(300.0),
            },
        ];
        // Start uniform at the cold temperature.
        let n = grid.n_cells();
        let mut init = vec![0.0; 2 * n];
        for c in 0..n {
            init[2 * c] = 1.0e5;
            init[2 * c + 1] = 300.0;
        }
        let problem = ThermalConvection::new(grid, params, boundary).unwrap();
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();

        let e0 = sim.total_energy();
        assert!(e0.is_finite());
        for _ in 0..10 {
            let report = sim.step(1.0e6).expect("heated step converges");
            assert!(report.converged);
        }
        let e1 = sim.total_energy();
        assert!(e1.is_finite());
        assert!(
            e1 > e0,
            "total energy did not rise under bottom heating: {e0} -> {e1}"
        );
    }

    /// Constructor / input validation: bad permeability, porosity, and timestep are
    /// rejected.
    #[test]
    fn constructors_reject_bad_inputs() {
        // Negative permeability.
        assert!(ConvectionParameters::new(
            -1.0e-12, 0.3, 1.0e-3, 4.5e-10, 1000.0, 2.1e-4, 300.0, 4182.0, 2.2e6, 0.6, 9.8
        )
        .is_err());
        // Porosity outside (0,1).
        assert!(ConvectionParameters::new(
            1.0e-12, 1.5, 1.0e-3, 4.5e-10, 1000.0, 2.1e-4, 300.0, 4182.0, 2.2e6, 0.6, 9.8
        )
        .is_err());
        assert!(ConvectionParameters::new(
            1.0e-12, 0.0, 1.0e-3, 4.5e-10, 1000.0, 2.1e-4, 300.0, 4182.0, 2.2e6, 0.6, 9.8
        )
        .is_err());
        // Negative gravity.
        assert!(ConvectionParameters::new(
            1.0e-12, 0.3, 1.0e-3, 4.5e-10, 1000.0, 2.1e-4, 300.0, 4182.0, 2.2e6, 0.6, -9.8
        )
        .is_err());
        // A valid set builds.
        assert!(ConvectionParameters::water_saturated().permeability > 0.0);

        // Negative dt -> InvalidInput at step time.
        let grid = unit_square(3, 3);
        let problem =
            ThermalConvection::new(grid, ConvectionParameters::water_saturated(), vec![]).unwrap();
        let n = problem.grid().n_cells();
        let init = vec![300.0; 2 * n]; // interleaved (values immaterial here)
        let mut sim = ThermalConvectionSimulation::new(problem, test_config(), init).unwrap();
        assert!(sim.step(-1.0).is_err());
        assert!(sim.step(0.0).is_err());
    }
}
