//! GENERAL mode — non-isothermal air–water–energy 3-phase-DOF flow (bead
//! op-v6s.15.5).
//!
//! This module is the **non-isothermal extension** of the isothermal two-phase
//! air–water solver in [`crate::multiphase`]. Where [`crate::multiphase`] carries
//! **two** primary unknowns per cell — liquid pressure `P_l` (Pa) and liquid
//! saturation `S_l` (dimensionless) — and solves two coupled mass balances, this
//! module **adds temperature `T` (K) as a third unknown**, giving a coupled
//! **three-DOF** (`nb = 3`) air–water–energy system: two phase mass balances plus
//! one bulk (fluid + rock) energy balance. It is a deliberately *simplified*
//! rendering of PFLOTRAN's GENERAL flow mode.
//!
//! The physics reuses the two-phase closure from [`crate::multiphase`] verbatim
//! in spirit (capillary pressure by retention-curve inversion, upstream-weighted
//! per-phase mobilities, a quadratic-Corey gas relative permeability) and adds
//! **temperature-dependent liquid properties** — density `rho_l(P_l, T)` and
//! viscosity `mu_l(T)` from [`ThermalWaterProperties`] — so that temperature
//! genuinely feeds back into the flow, not just rides along it.
//!
//! # Governing equations (three per cell, backward-Euler in time)
//!
//! Discretised by a cell-centred **two-point flux** finite volume in space and
//! **backward (implicit) Euler** in time. Per cell:
//!
//! **1. Water (liquid) mass**
//!
//! $$ \frac{\partial}{\partial t}\!\left(\phi\, \rho_l\, S_l\right) + \nabla\cdot\!\left(\rho_l\, \lambda_l\,(\nabla P_l - \rho_l\, \mathbf{g})\right) = 0, \qquad \lambda_l = \frac{k\, k_{rl}(S_l)}{\mu_l(T)} $$
//!
//! **2. Air (gas) mass**
//!
//! $$ \frac{\partial}{\partial t}\!\left(\phi\, \rho_g\, S_g\right) + \nabla\cdot\!\left(\rho_g\, \lambda_g\,(\nabla P_g - \rho_g\, \mathbf{g})\right) = 0, \qquad S_g = 1 - S_l, \quad P_g = P_l + P_c(S_l) $$
//!
//! **3. Energy (bulk fluid + rock)**
//!
//! $$ \frac{\partial}{\partial t}\!\left[\phi\,(\rho_l S_l u_l + \rho_g S_g u_g) + (1-\phi)\, \rho_r c_r (T - T_{\text{ref}})\right] + \nabla\cdot\!\left(\rho_l h_l \lambda_l \mathbf{f}_l + \rho_g h_g \lambda_g \mathbf{f}_g\right) - \nabla\cdot\!\left(\lambda_{\text{bulk}}\, \nabla T\right) = 0 $$
//!
//! where `f_l`, `f_g` are the per-phase Darcy driving terms
//! `(grad P_p - rho_p g)`, `u_p`/`h_p` are the phase specific internal
//! energy / enthalpy, and `lambda_bulk` is the effective bulk thermal
//! conductivity. The energy advection is carried by **both** phase mass fluxes
//! (upstream-weighted on the phase enthalpy) and conduction by `lambda_bulk`.
//!
//! # Temperature coupling (why `T` is not inert)
//!
//! Temperature couples **back into the flow** through the temperature-dependent
//! liquid properties supplied by [`ThermalWaterProperties`]:
//! - `rho_l(P_l, T)` — density falls with `T` (thermal expansion), changing both
//!   the accumulation term and the gravity head;
//! - `mu_l(T)` — viscosity falls with `T`, *raising* the liquid mobility
//!   `lambda_l` and so speeding liquid flow in hot regions.
//! Gas properties (`rho_g`, `mu_g`) are constant in this v1.
//!
//! # Simplifications — flags for human review (this is an untrusted AI draft)
//!
//! Per the workspace `RESPONSIBLE_USE.md`, this is **untrusted AI-generated draft
//! material, verification-only**, not validated against any published PFLOTRAN
//! GENERAL-mode reference. It is a *simplified* GENERAL mode. Real GENERAL-mode
//! features deliberately **omitted** here:
//! - **No component partitioning between phases** — the air and water components
//!   do not dissolve/evaporate into each other; there is one air component in the
//!   gas and one water component in the liquid, each confined to its phase.
//! - **No phase appearance / disappearance handling** beyond clamping `S_l` into
//!   `[S_r, 1]` for the property closures; there is no primary-variable switching.
//! - **No latent heat / phase change** — no boiling, condensation, or evaporation
//!   enthalpy.
//! - **Ideal / constant-`c_p` energy** — internal energy and enthalpy are
//!   simplified to `u = h = c_p (T - T_ref)` with a constant per-phase `c_p` and a
//!   fixed reference temperature `T_ref` (25 °C by default). No real-gas EOS.
//! - **Constant gas density and viscosity** (constant-density air).
//! - **Bulk conductivity** is a porosity-weighted arithmetic mean
//!   `lambda_bulk = phi k_l + (1 - phi) k_r` (fully-saturated approximation,
//!   independent of `S_l`), matching [`crate::energy`].
//!
//! # State-vector layout
//!
//! All state vectors are interleaved, length `3 * n_cells`: cell `c` occupies
//! `[3c] = P_l` (Pa), `[3c + 1] = S_l` (dimensionless), `[3c + 2] = T` (K),
//! matching the block solver's `nb = 3` convention. Where `S_l` feeds the
//! property closures it is clamped to `[S_r, 1]`; the accumulation terms use the
//! raw `S_l`/`T` so the discrete balances stay exact.
//!
//! # Sign & reference conventions
//!
//! - `z` is elevation (upward positive); gravity magnitude `g >= 0` acts in `-z`.
//! - Per-face phase potential `Phi_p = P_p + rho_p g z`; a positive face/boundary
//!   flux **leaves** the cell.
//! - Unspecified boundary faces are **no-flow + adiabatic** (the natural default).
//!
//! # Jacobian
//!
//! The `3x3`-block Jacobian is assembled **numerically** — local finite
//! differences of the three residual equations with respect to the three local
//! unknowns over the two-point stencil (see [`GeneralFlow::assemble_jacobian`]).
//! This matches the residual by construction and side-steps fragile analytic
//! derivatives through the upstream switch, the capillary-pressure inversion, and
//! the temperature-dependent property closures — the standard tractable choice
//! for a strongly coupled `nb = 3` system.
//!
//! # Provenance
//!
//! PFLOTRAN GENERAL mode (documentation.pflotran.org; Lichtner et al.,
//! *PFLOTRAN Theory Guide*) and the standard black-oil / compositional
//! multiphase-with-energy formulation. Independent pure-Rust reimplementation,
//! not the official PFLOTRAN and not endorsed by its authors.
//!
//! > **Untrusted AI draft — verification only.** Not for facility operation or
//! > any safety-critical use.

use std::sync::Arc;

use crate::error::PflotranError;
use crate::grid::{BoundaryLocation, CartesianGrid};
use crate::properties::{CharacteristicCurves, RockThermalProperties, ThermalWaterProperties};
use crate::solver::block::{
    BlockLduMatrix, BlockNewtonConfig, BlockNewtonReport, BlockNewtonSolver, BlockNonlinearSystem,
};

use rayon::prelude::*;
use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Standard gravitational acceleration (m/s^2) — the conventional default for a
/// GENERAL-mode run (gravity acting in `-z`). Matches
/// [`crate::multiphase::STANDARD_GRAVITY`].
pub const STANDARD_GRAVITY: f64 = 9.806_65;

/// Air–water–energy fluid + rock properties for the GENERAL mode.
///
/// Bundles everything the three governing equations need beyond the geometry and
/// the characteristic curves:
/// - **Liquid water** as a temperature-dependent [`ThermalWaterProperties`]
///   (density `rho_l(P_l, T)`, viscosity `mu_l(T)`, specific heat `c_pl`);
/// - **Gas (air)** as a *constant*-property phase (`rho_g`, `mu_g`, `c_pg`);
/// - **Rock** thermal properties [`RockThermalProperties`] (`rho_r`, `c_r`,
///   conductivity `k_r`) for the solid part of the bulk energy storage and
///   conduction;
/// - the **enthalpy reference temperature** `T_ref` (K) at which the simplified
///   internal energy / enthalpy `u = h = c_p (T - T_ref)` is zero.
///
/// The liquid specific heat `c_pl` and liquid thermal conductivity `k_l` are read
/// from the embedded [`ThermalWaterProperties`]; only the *gas* specific heat is
/// stored separately (the gas is otherwise constant-property).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneralFluids {
    /// Temperature-dependent liquid-water properties: `rho_l(P_l, T)`, `mu_l(T)`,
    /// liquid specific heat `c_pl` (J/(kg·K)) and conductivity `k_l` (W/(m·K)).
    pub water: ThermalWaterProperties,
    /// Gas (air) mass density `rho_g`, kg/m^3, constant. Strictly positive.
    pub gas_density: f64,
    /// Gas (air) dynamic viscosity `mu_g`, Pa·s, constant. Strictly positive.
    pub gas_viscosity: f64,
    /// Gas (air) specific heat at constant pressure `c_pg`, J/(kg·K). Strictly
    /// positive. Enters the gas internal energy / enthalpy `u_g = h_g = c_pg
    /// (T - T_ref)`.
    pub gas_specific_heat: f64,
    /// Solid-matrix (rock) thermal properties: density `rho_r`, specific heat
    /// `c_r`, conductivity `k_r`.
    pub rock: RockThermalProperties,
    /// Enthalpy / internal-energy reference temperature `T_ref`, K. The simplified
    /// per-phase internal energy and enthalpy are `u = h = c_p (T - T_ref)`.
    pub reference_temperature: f64,
}

impl GeneralFluids {
    /// Representative **air–water at ~20–25 °C** defaults: liquid water from
    /// [`ThermalWaterProperties::water`] (`rho_l ≈ 1000 kg/m^3`,
    /// `mu_l ≈ 1.0e-3 Pa·s`, `c_pl = 4182 J/(kg·K)`, `k_l = 0.6 W/(m·K)`); air
    /// `rho_g = 1.2 kg/m^3`, `mu_g = 1.8e-5 Pa·s`, `c_pg = 1005 J/(kg·K)`; rock
    /// from [`RockThermalProperties::generic_rock`]; enthalpy datum
    /// `T_ref = 298.15 K` (25 °C).
    pub fn air_water() -> Self {
        Self {
            water: ThermalWaterProperties::water(),
            gas_density: 1.2,
            gas_viscosity: 1.8e-5,
            gas_specific_heat: 1005.0,
            rock: RockThermalProperties::generic_rock(),
            reference_temperature: 298.15,
        }
    }

    /// Build a validated air–water–energy fluid/rock bundle.
    ///
    /// # Parameters
    ///
    /// - `water` — temperature-dependent liquid-water properties (already
    ///   validated by its own constructor).
    /// - `gas_density` — kg/m^3, finite and `> 0`.
    /// - `gas_viscosity` — Pa·s, finite and `> 0`.
    /// - `gas_specific_heat` — J/(kg·K), finite and `> 0`.
    /// - `rock` — solid-matrix thermal properties (already validated).
    /// - `reference_temperature` — K, finite and `> 0` (the enthalpy datum).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if any scalar is non-finite or out of the
    /// range above.
    pub fn new(
        water: ThermalWaterProperties,
        gas_density: f64,
        gas_viscosity: f64,
        gas_specific_heat: f64,
        rock: RockThermalProperties,
        reference_temperature: f64,
    ) -> Result<Self, PflotranError> {
        for (name, v) in [
            ("gas_density", gas_density),
            ("gas_viscosity", gas_viscosity),
            ("gas_specific_heat", gas_specific_heat),
            ("reference_temperature", reference_temperature),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "GeneralFluids {name} must be finite and > 0, got {v}"
                )));
            }
        }
        Ok(Self {
            water,
            gas_density,
            gas_viscosity,
            gas_specific_heat,
            rock,
            reference_temperature,
        })
    }

    /// Liquid specific heat `c_pl` (J/(kg·K)), read from the embedded thermal
    /// water properties.
    #[inline]
    pub fn liquid_specific_heat(&self) -> f64 {
        self.water.specific_heat()
    }

    /// Liquid mass density `rho_l(P_l, T)` (kg/m^3) at liquid pressure `p_l` (Pa)
    /// and temperature `t` (K). Uses [`ThermalWaterProperties::density`];
    /// increasing in `P_l`, decreasing in `T`. This is the sole route to `rho_l`
    /// and is used identically in the accumulation, flux, and total-mass terms.
    #[inline]
    pub fn liquid_density(&self, p_l: f64, t: f64) -> f64 {
        self.water
            .density(
                Pressure::new::<pascal>(p_l),
                ThermodynamicTemperature::new::<kelvin>(t),
            )
            .get::<kilogram_per_cubic_meter>()
    }

    /// Liquid dynamic viscosity `mu_l(T)` (Pa·s) at temperature `t` (K). Uses
    /// [`ThermalWaterProperties::viscosity`]; monotonically decreasing in `T`.
    #[inline]
    pub fn liquid_viscosity(&self, t: f64) -> f64 {
        self.water
            .viscosity(ThermodynamicTemperature::new::<kelvin>(t))
            .get::<pascal_second>()
    }

    /// Liquid specific internal energy / enthalpy `u_l = h_l = c_pl (T - T_ref)`
    /// (J/kg) at temperature `t` (K). Simplified constant-`c_p`, `T_ref`-referenced.
    #[inline]
    pub fn liquid_enthalpy(&self, t: f64) -> f64 {
        self.liquid_specific_heat() * (t - self.reference_temperature)
    }

    /// Gas specific internal energy / enthalpy `u_g = h_g = c_pg (T - T_ref)`
    /// (J/kg) at temperature `t` (K).
    #[inline]
    pub fn gas_enthalpy(&self, t: f64) -> f64 {
        self.gas_specific_heat * (t - self.reference_temperature)
    }
}

/// What is prescribed on one exterior boundary location for GENERAL-mode flow.
///
/// Enum dispatch (no trait objects) per the workspace design rules. Unspecified
/// faces default to [`GeneralBoundaryKind::NoFlow`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeneralBoundaryKind {
    /// Fixed liquid pressure (Pa), liquid saturation (dimensionless in `[S_r, 1]`)
    /// **and** temperature (K) at the boundary. The ghost state's gas pressure
    /// follows the capillary closure `P_g = P_l + P_c(S_l)`; both phases can
    /// exchange mass across the face, advecting enthalpy, and the conductive term
    /// couples the near-boundary cell to `T_bc` across the half-cell distance
    /// (a genuine fixed-temperature boundary regardless of flow direction — the
    /// same modelling choice [`crate::energy`] makes).
    Dirichlet {
        /// Prescribed liquid pressure `P_l` at the boundary, Pa.
        liquid_pressure: f64,
        /// Prescribed liquid saturation `S_l` at the boundary, dimensionless in
        /// `[S_r, 1]`.
        liquid_saturation: f64,
        /// Prescribed temperature `T` at the boundary, K.
        temperature: f64,
    },
    /// No-flow **and adiabatic** (zero normal flux for both phases and zero
    /// conductive heat flux). The natural default.
    NoFlow,
}

/// A GENERAL-mode boundary condition applied to one of the six Cartesian faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneralBoundaryCondition {
    /// Which exterior face of the logical box this applies to.
    pub location: BoundaryLocation,
    /// What is prescribed there.
    pub kind: GeneralBoundaryKind,
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

/// GENERAL-mode 3-DOF (`P_l`, `S_l`, `T`) non-isothermal air–water problem for a
/// single backward-Euler timestep.
///
/// Owns an `Arc<CartesianGrid>` (shared, read-only topology — no lifetimes, per
/// the workspace rules), the characteristic curves, the [`GeneralFluids`] bundle,
/// and homogeneous material data (isotropic permeability, uniform porosity).
/// Implements [`BlockNonlinearSystem`] with `dof_per_cell = 3`, so it is driven
/// by the generic [`BlockNewtonSolver`].
///
/// # Energy-residual scaling
///
/// The bulk energy residual (W-scale, dominated by `rho_r c_r`) is divided by a
/// fixed numerical scale `energy_scale = rho_r c_r` before it enters the block
/// system, so the three residual components are of comparable magnitude and the
/// Newton residual norm is not dominated by the energy equation. This is a
/// *numerical* row scaling: it does not change the root (a scaled residual driven
/// to zero is the same solution) and the reported total-energy diagnostic is
/// computed unscaled.
pub struct GeneralFlow {
    grid: Arc<CartesianGrid>,
    curves: CharacteristicCurves,
    fluids: GeneralFluids,
    porosity: f64,
    permeability: f64,
    gravity: f64,
    /// Effective bulk thermal conductivity `lambda_bulk = phi k_l + (1 - phi) k_r`
    /// (W/(m·K)); precomputed in `new`.
    lambda_bulk: f64,
    /// Numerical energy-residual row scale `rho_r c_r` (see the type docs).
    energy_scale: f64,
    // Precomputed topology helpers (built in `new`).
    z: Vec<f64>,
    cell_faces: Vec<Vec<usize>>,
    cell_boundary_faces: Vec<Vec<usize>>,
    bc: [Option<GeneralBoundaryKind>; 6],
    // Time-stepping state.
    dt: f64,
    /// Previous-time-level state, interleaved `[P_l, S_l, T]` per cell, length
    /// `3 * n_cells`. All three enter the accumulation terms (density and energy
    /// both depend on the full previous state).
    prev: Vec<f64>,
}

impl GeneralFlow {
    /// Build a GENERAL-mode air–water–energy flow problem.
    ///
    /// # Parameters
    ///
    /// - `grid` — shared structured Cartesian FV mesh (`Arc`, read-only).
    /// - `fluids` — the air–water–energy fluid/rock bundle.
    /// - `curves` — retention + wetting relative-permeability model.
    /// - `porosity` — dimensionless, in `(0, 1)`.
    /// - `permeability` — intrinsic isotropic `k`, m^2, `> 0`.
    /// - `gravity` — magnitude (m/s^2, acting in `-z`; pass `0` to disable).
    /// - `boundary` — per-face conditions; any face omitted is no-flow + adiabatic.
    ///
    /// The timestep defaults to `1 s` and the previous state to a uniform
    /// `(P_l = reference_pressure, S_l = 1, T = T_ref)` until
    /// [`set_timestep`](Self::set_timestep) / [`set_previous`](Self::set_previous)
    /// are called (the [`GeneralSimulation`] driver does this each step).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `porosity` is not in `(0, 1)`, or
    /// `permeability`/`gravity` are non-finite or out of range.
    pub fn new(
        grid: Arc<CartesianGrid>,
        fluids: GeneralFluids,
        curves: CharacteristicCurves,
        porosity: f64,
        permeability: f64,
        gravity: f64,
        boundary: Vec<GeneralBoundaryCondition>,
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

        let mut bc: [Option<GeneralBoundaryKind>; 6] = [None; 6];
        for cond in boundary {
            bc[location_index(&cond.location)] = Some(cond.kind);
        }

        let lambda_bulk = porosity * fluids.water.thermal_conductivity()
            + (1.0 - porosity) * fluids.rock.thermal_conductivity;
        let energy_scale = fluids.rock.density * fluids.rock.specific_heat;

        // Default previous state: fully liquid, at the density reference pressure
        // and the enthalpy reference temperature.
        let mut prev = vec![0.0; 3 * n];
        for c in 0..n {
            prev[3 * c] = fluids.water.reference_pressure;
            prev[3 * c + 1] = 1.0;
            prev[3 * c + 2] = fluids.reference_temperature;
        }

        Ok(Self {
            grid,
            curves,
            fluids,
            porosity,
            permeability,
            gravity,
            lambda_bulk,
            energy_scale,
            z,
            cell_faces,
            cell_boundary_faces,
            bc,
            dt: 1.0,
            prev,
        })
    }

    /// Set the implicit-Euler timestep `dt` (s) for the next assembly. Must be
    /// positive (validated by the [`GeneralSimulation`] driver at step time).
    pub fn set_timestep(&mut self, dt: f64) {
        self.dt = dt;
    }

    /// Set the previous-time-level state (interleaved `[P_l, S_l, T, …]`, length
    /// `3 * n_cells`).
    ///
    /// All three primary variables per cell are stored, because the liquid
    /// density and the bulk energy both depend on the full previous
    /// `(P_l, S_l, T)`.
    ///
    /// # Panics
    ///
    /// Panics if `state.len() != 3 * n_cells`.
    pub fn set_previous(&mut self, state: &[f64]) {
        let n = self.grid.n_cells();
        assert_eq!(
            state.len(),
            3 * n,
            "set_previous: state must be length 3*n_cells"
        );
        self.prev.copy_from_slice(state);
    }

    /// Read access to the shared underlying grid.
    pub fn grid(&self) -> &CartesianGrid {
        &self.grid
    }

    /// The [`GeneralFluids`] bundle (read-only).
    pub fn fluids(&self) -> &GeneralFluids {
        &self.fluids
    }

    /// Total **water** mass in the domain (kg) for an interleaved state `x`:
    /// `sum_i V_i phi rho_l(P_l_i, T_i) S_l_i`.
    ///
    /// With no-flow boundaries this is conserved across a timestep to the
    /// nonlinear-solver tolerance, because summed internal water fluxes cancel and
    /// every water residual is driven to zero. Uses exactly the same
    /// `rho_l(P_l, T)` as the accumulation term.
    pub fn total_water_mass(&self, x: &[f64]) -> f64 {
        (0..self.grid.n_cells())
            .map(|i| {
                let p_l = x[3 * i];
                let s_l = x[3 * i + 1];
                let t = x[3 * i + 2];
                self.grid.cell_volume(i) * self.porosity * self.fluids.liquid_density(p_l, t) * s_l
            })
            .sum()
    }

    /// Total **gas** mass in the domain (kg) for an interleaved state `x`:
    /// `sum_i V_i phi (1 - S_l_i) rho_g`. Conserved under no-flow like the water
    /// mass. Gas density is constant.
    pub fn total_gas_mass(&self, x: &[f64]) -> f64 {
        let rho_g = self.fluids.gas_density;
        (0..self.grid.n_cells())
            .map(|i| self.grid.cell_volume(i) * self.porosity * (1.0 - x[3 * i + 1]) * rho_g)
            .sum()
    }

    /// Total **bulk energy** in the domain (J) relative to the `T_ref` datum, for
    /// an interleaved state `x`:
    /// `sum_i V_i [ phi (rho_l S_l u_l + rho_g S_g u_g) + (1 - phi) rho_r c_r
    /// (T - T_ref) ]`.
    ///
    /// This is the invariant a closed (no-flow, adiabatic) domain must conserve;
    /// it uses exactly the per-cell energy density that enters the accumulation
    /// term, so conservation holds to solver tolerance. Computed **unscaled** (the
    /// energy-residual row scaling is internal to the block assembly).
    pub fn total_energy(&self, x: &[f64]) -> f64 {
        (0..self.grid.n_cells())
            .map(|i| {
                let p_l = x[3 * i];
                let s_l = x[3 * i + 1];
                let t = x[3 * i + 2];
                self.grid.cell_volume(i) * self.energy_density(p_l, s_l, t)
            })
            .sum()
    }

    // --- pointwise property helpers (plain f64, SI units) ---

    /// Effective saturation `S_e = (S_l - S_r)/(1 - S_r)`, clamped to `[0, 1]`.
    /// Mirrors `crate::multiphase`'s private `se_of_sl`, built on the public
    /// [`CharacteristicCurves::residual_saturation`].
    #[inline]
    fn se_of_sl(&self, s_l: f64) -> f64 {
        let sr = self.curves.residual_saturation();
        ((s_l - sr) / (1.0 - sr)).clamp(0.0, 1.0)
    }

    /// Retention-curve effective saturation `S_e(P_c)` at capillary pressure `pc`
    /// (Pa).
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
    /// residual gas saturation: `k_{rg}(S_e) = (1 - S_e)^2` in `[0, 1]`. Mirrors
    /// `crate::multiphase`.
    #[inline]
    fn krg(&self, se: f64) -> f64 {
        let sg_eff = (1.0 - se).clamp(0.0, 1.0);
        sg_eff * sg_eff
    }

    /// Capillary pressure `P_c(S_l)` (Pa), by numerically inverting the retention
    /// curve `S_e = effective_saturation(P_c)` (exponential bracket + bisection).
    /// Mirrors `crate::multiphase::TwoPhaseFlow::capillary_pressure`.
    ///
    /// `S_e >= 1` (fully saturated) returns `0`.
    pub fn capillary_pressure(&self, s_l: f64) -> f64 {
        let se = self.se_of_sl(s_l);
        if se >= 1.0 {
            return 0.0;
        }
        let target = se.max(1.0e-9);

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

    /// Per-cell bulk energy density `E` (J/m^3) relative to `T_ref`:
    /// `phi (rho_l S_l u_l + rho_g S_g u_g) + (1 - phi) rho_r c_r (T - T_ref)`,
    /// with `u_l = c_pl (T - T_ref)`, `u_g = c_pg (T - T_ref)`. Shared by the
    /// energy accumulation term and [`total_energy`](Self::total_energy) so the
    /// two are exactly consistent.
    #[inline]
    fn energy_density(&self, p_l: f64, s_l: f64, t: f64) -> f64 {
        let phi = self.porosity;
        let rho_l = self.fluids.liquid_density(p_l, t);
        let rho_g = self.fluids.gas_density;
        let s_g = 1.0 - s_l;
        let u_l = self.fluids.liquid_enthalpy(t);
        let u_g = self.fluids.gas_enthalpy(t);
        let fluid = phi * (rho_l * s_l * u_l + rho_g * s_g * u_g);
        let rock = (1.0 - phi)
            * self.fluids.rock.density
            * self.fluids.rock.specific_heat
            * (t - self.fluids.reference_temperature);
        fluid + rock
    }

    /// Two-point **water** mass flux across a face (kg/s, positive owner ->
    /// neighbour). Mobility upstream-weighted on the liquid potential, with a
    /// face-arithmetic-mean density in the gravity head (so the upstream switch is
    /// non-circular) and the **upstream** density × mobility (`rho_l k k_{rl}/mu_l`)
    /// carrying the mass. `mu_l` and `rho_l` are temperature-dependent.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn water_flux(
        &self,
        p_l_o: f64,
        t_o: f64,
        se_o: f64,
        z_o: f64,
        p_l_n: f64,
        t_n: f64,
        se_n: f64,
        z_n: f64,
        t_geom: f64,
    ) -> f64 {
        let rho_o = self.fluids.liquid_density(p_l_o, t_o);
        let rho_n = self.fluids.liquid_density(p_l_n, t_n);
        let rho_face = 0.5 * (rho_o + rho_n);
        let dphi = (p_l_o - p_l_n) + rho_face * self.gravity * (z_o - z_n);
        let (rho_up, mu_up, kr_up) = if dphi >= 0.0 {
            (rho_o, self.fluids.liquid_viscosity(t_o), self.krl(se_o))
        } else {
            (rho_n, self.fluids.liquid_viscosity(t_n), self.krl(se_n))
        };
        let mobility = self.permeability * kr_up / mu_up;
        rho_up * mobility * t_geom * dphi
    }

    /// Two-point **gas** mass flux across a face (kg/s, positive owner ->
    /// neighbour). Constant gas density/viscosity; mobility upstream-weighted on
    /// the gas potential `P_g = P_l + P_c(S_l)`.
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

    /// Residual triple `(R_water_i, R_gas_i, R_energy_i)` for one cell: phase
    /// accumulation (backward Euler) plus net phase outflow, and bulk-energy
    /// accumulation plus net advective + conductive heat outflow, across internal
    /// and boundary faces. A positive flux **leaves** the cell.
    ///
    /// `R_water`/`R_gas` are kg/s; the returned `R_energy` is the physical energy
    /// residual (W) **divided by `energy_scale`** (see the type-level docs on the
    /// numerical row scaling).
    fn cell_residual(&self, i: usize, x: &[f64]) -> [f64; 3] {
        let v = self.grid.cell_volume(i);
        let phi = self.porosity;
        let inv_dt = 1.0 / self.dt;

        let p_l_i = x[3 * i];
        let s_l_i = x[3 * i + 1];
        let t_i = x[3 * i + 2];

        let p0 = self.prev[3 * i];
        let s0 = self.prev[3 * i + 1];
        let t0 = self.prev[3 * i + 2];

        // --- Accumulation (backward Euler) ---
        let rho_l_i = self.fluids.liquid_density(p_l_i, t_i);
        let rho_l_old = self.fluids.liquid_density(p0, t0);
        let rho_g = self.fluids.gas_density;

        let acc_w = v * inv_dt * phi * (rho_l_i * s_l_i - rho_l_old * s0);
        let acc_g = v * inv_dt * phi * rho_g * ((1.0 - s_l_i) - (1.0 - s0));
        let acc_e =
            v * inv_dt * (self.energy_density(p_l_i, s_l_i, t_i) - self.energy_density(p0, s0, t0));

        // Cell-local face-state helpers.
        let se_i = self.se_of_sl(s_l_i);
        let p_g_i = p_l_i + self.capillary_pressure(s_l_i);

        let mut flux_w = 0.0;
        let mut flux_g = 0.0;
        let mut flux_e = 0.0;

        // --- Internal faces incident to cell i ---
        for &f in &self.cell_faces[i] {
            let c = &self.grid.connections()[f];
            let (o, n) = (c.owner, c.neighbour);
            let t_geom = c.geometric_transmissibility;

            let p_l_o = x[3 * o];
            let s_l_o = x[3 * o + 1];
            let t_o = x[3 * o + 2];
            let p_l_n = x[3 * n];
            let s_l_n = x[3 * n + 1];
            let t_n = x[3 * n + 2];

            let se_o = self.se_of_sl(s_l_o);
            let se_n = self.se_of_sl(s_l_n);
            let p_g_o = p_l_o + self.capillary_pressure(s_l_o);
            let p_g_n = p_l_n + self.capillary_pressure(s_l_n);

            let m_w = self.water_flux(
                p_l_o, t_o, se_o, self.z[o], p_l_n, t_n, se_n, self.z[n], t_geom,
            );
            let m_g = self.gas_flux(p_g_o, se_o, self.z[o], p_g_n, se_n, self.z[n], t_geom);

            // Advected enthalpy: upstream by each phase's mass-flux sign
            // (m >= 0 => owner is upstream). Conduction: symmetric two-point.
            let h_l_up = if m_w >= 0.0 {
                self.fluids.liquid_enthalpy(t_o)
            } else {
                self.fluids.liquid_enthalpy(t_n)
            };
            let h_g_up = if m_g >= 0.0 {
                self.fluids.gas_enthalpy(t_o)
            } else {
                self.fluids.gas_enthalpy(t_n)
            };
            let cond = self.lambda_bulk * t_geom * (t_o - t_n);
            let e_face = m_w * h_l_up + m_g * h_g_up + cond; // positive owner -> neighbour

            if i == o {
                flux_w += m_w;
                flux_g += m_g;
                flux_e += e_face;
            } else {
                flux_w -= m_w;
                flux_g -= m_g;
                flux_e -= e_face;
            }
        }

        // --- Boundary faces ---
        for &b in &self.cell_boundary_faces[i] {
            let bf = &self.grid.boundary_faces()[b];
            match &self.bc[location_index(&bf.location)] {
                None | Some(GeneralBoundaryKind::NoFlow) => {}
                Some(GeneralBoundaryKind::Dirichlet {
                    liquid_pressure,
                    liquid_saturation,
                    temperature,
                }) => {
                    let z_bc = match bf.location {
                        BoundaryLocation::ZMin => self.z[i] - bf.distance,
                        BoundaryLocation::ZMax => self.z[i] + bf.distance,
                        _ => self.z[i],
                    };
                    let t_geom = bf.area / bf.distance;
                    let se_bc = self.se_of_sl(*liquid_saturation);
                    let p_g_bc = *liquid_pressure + self.capillary_pressure(*liquid_saturation);

                    // Cell is the owner side; ghost is the neighbour side.
                    let m_w = self.water_flux(
                        p_l_i,
                        t_i,
                        se_i,
                        self.z[i],
                        *liquid_pressure,
                        *temperature,
                        se_bc,
                        z_bc,
                        t_geom,
                    );
                    let m_g = self.gas_flux(p_g_i, se_i, self.z[i], p_g_bc, se_bc, z_bc, t_geom);

                    // Advected enthalpy upstream by boundary flux sign
                    // (m >= 0 => leaving cell => interior is upstream).
                    let h_l_up = if m_w >= 0.0 {
                        self.fluids.liquid_enthalpy(t_i)
                    } else {
                        self.fluids.liquid_enthalpy(*temperature)
                    };
                    let h_g_up = if m_g >= 0.0 {
                        self.fluids.gas_enthalpy(t_i)
                    } else {
                        self.fluids.gas_enthalpy(*temperature)
                    };
                    // Conductive Dirichlet coupling to T_bc (always applied),
                    // positive leaves the cell when T_i > T_bc.
                    let cond = self.lambda_bulk * t_geom * (t_i - *temperature);

                    flux_w += m_w;
                    flux_g += m_g;
                    flux_e += m_w * h_l_up + m_g * h_g_up + cond;
                }
            }
        }

        [
            acc_w + flux_w,
            acc_g + flux_g,
            (acc_e + flux_e) / self.energy_scale,
        ]
    }
}

impl BlockNonlinearSystem for GeneralFlow {
    fn n_cells(&self) -> usize {
        self.grid.n_cells()
    }

    fn dof_per_cell(&self) -> usize {
        3
    }

    fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
        self.grid.ldu_addressing()
    }

    fn assemble_residual(&mut self, x: &[f64], out: &mut [f64]) -> Result<(), PflotranError> {
        let n = self.grid.n_cells();
        if x.len() != 3 * n || out.len() != 3 * n {
            return Err(PflotranError::InvalidInput(
                "GENERAL residual: state/output length must be 3 * n_cells".into(),
            ));
        }
        let this = &*self;
        out.par_chunks_mut(3).enumerate().for_each(|(i, chunk)| {
            let r = this.cell_residual(i, x);
            chunk[0] = r[0];
            chunk[1] = r[1];
            chunk[2] = r[2];
        });
        Ok(())
    }

    /// Assemble the `3x3`-block Jacobian **numerically** by local finite
    /// differences.
    ///
    /// Perturbing local unknown `d` of cell `j` (`d = 0 -> P_l`, `1 -> S_l`,
    /// `2 -> T`) affects only the residual of `j` and its face-neighbours (the
    /// two-point stencil). Each column's writes land in disjoint `3x3` block
    /// entries, so the residual evaluations run in parallel and the results
    /// scatter serially — identical to a serial assembly. The step is
    /// `h = 1e-7 (1 + |x|)`, tuned so the ~absolute temperatures (`T ~ 300 K`) and
    /// the large pressures (`P_l ~ 1e5 Pa`) both get a well-scaled perturbation.
    fn assemble_jacobian(
        &mut self,
        x: &[f64],
        jac: &mut BlockLduMatrix,
    ) -> Result<(), PflotranError> {
        let n = self.grid.n_cells();
        if x.len() != 3 * n {
            return Err(PflotranError::InvalidInput(
                "GENERAL jacobian: state length must be 3 * n_cells".into(),
            ));
        }

        let this = &*self;

        // Base residual triples over all cells (parallel; independent per cell).
        let r0: Vec<[f64; 3]> = (0..n)
            .into_par_iter()
            .map(|i| this.cell_residual(i, x))
            .collect();

        jac.zero();

        // (j, d, self-column [3], face contributions Vec<(f, is_lower, [3])>)
        type ColumnContrib = (usize, usize, [f64; 3], Vec<(usize, bool, [f64; 3])>);
        let contribs: Vec<ColumnContrib> = (0..3 * n)
            .into_par_iter()
            .map(|col| {
                let j = col / 3;
                let d = col % 3;
                let idx = 3 * j + d;
                let mut xp = x.to_vec();
                let h = 1.0e-7 * (1.0 + x[idx].abs());
                xp[idx] = x[idx] + h;

                let rj = this.cell_residual(j, &xp);
                let dj = [
                    (rj[0] - r0[j][0]) / h,
                    (rj[1] - r0[j][1]) / h,
                    (rj[2] - r0[j][2]) / h,
                ];

                let faces: Vec<(usize, bool, [f64; 3])> = this.cell_faces[j]
                    .iter()
                    .map(|&f| {
                        let c = &this.grid.connections()[f];
                        let (other, is_lower) = if j == c.owner {
                            (c.neighbour, true) // owner column affects neighbour row => lower
                        } else {
                            (c.owner, false) // neighbour column affects owner row => upper
                        };
                        let ro = this.cell_residual(other, &xp);
                        (
                            f,
                            is_lower,
                            [
                                (ro[0] - r0[other][0]) / h,
                                (ro[1] - r0[other][1]) / h,
                                (ro[2] - r0[other][2]) / h,
                            ],
                        )
                    })
                    .collect();
                (j, d, dj, faces)
            })
            .collect();

        for (j, d, dj, faces) in contribs {
            for row in 0..3 {
                jac.add_diag(j, row, d, dj[row]);
            }
            for (f, is_lower, vals) in faces {
                for row in 0..3 {
                    if is_lower {
                        jac.add_lower(f, row, d, vals[row]);
                    } else {
                        jac.add_upper(f, row, d, vals[row]);
                    }
                }
            }
        }

        if jac.diag.iter().any(|v| !v.is_finite()) {
            return Err(PflotranError::Convergence(
                "non-finite GENERAL Jacobian diagonal (check curves near saturation)".into(),
            ));
        }
        Ok(())
    }
}

/// A default block-Newton configuration for GENERAL-mode steps.
///
/// These are the same convergence settings `crate::multiphase` uses for its
/// (proven-to-converge) two-phase verification runs on the identical block
/// Newton solver and numerical-Jacobian approach — a deliberately conservative
/// choice for the `nb = 3` system. The tiny `abs_tol` handles the exactly
/// stationary case (initial residual identically zero); the `rel_tol` is the
/// operative one for driven steps. Both are far tighter than the qualitative
/// assertions the tests make, and — combined with the energy-residual row
/// scaling that keeps the three components comparable — a converged solve implies
/// water mass, gas mass, and energy are each conserved to well under the asserted
/// bounds (a `~1e-8` relative drift in practice).
fn general_newton_config() -> BlockNewtonConfig {
    BlockNewtonConfig {
        max_iterations: 100,
        abs_tol: 1.0e-12,
        rel_tol: 1.0e-7,
        linear_tolerance: 1.0e-8,
        linear_max_iter: 5000,
        max_backtracks: 25,
    }
}

/// A transient GENERAL-mode air–water–energy simulation driven by the block
/// Newton solver.
///
/// Holds the [`GeneralFlow`] problem, a [`BlockNewtonSolver`], the evolving
/// interleaved state (`[P_l, S_l, T]` per cell, length `3 * n_cells`), and the
/// current time. Each [`step`](Self::step) sets the timestep and previous state
/// on the problem, then solves the coupled `nb = 3` block nonlinear system.
pub struct GeneralSimulation {
    problem: GeneralFlow,
    solver: BlockNewtonSolver,
    state: Vec<f64>,
    time: f64,
}

impl GeneralSimulation {
    /// Assemble a simulation from a problem and a full interleaved initial state
    /// `[P_l, S_l, T]` per cell (length `3 * n_cells`).
    ///
    /// Uses the module's [`general_newton_config`] internally.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `initial.len() != 3 * n_cells`, if any
    /// entry is non-finite, or if any liquid saturation is outside `[0, 1]`.
    pub fn new(problem: GeneralFlow, initial: Vec<f64>) -> Result<Self, PflotranError> {
        let n = problem.grid().n_cells();
        if initial.len() != 3 * n {
            return Err(PflotranError::InvalidInput(format!(
                "initial state has length {} but the grid has {n} cells (expected {})",
                initial.len(),
                3 * n
            )));
        }
        for (i, &v) in initial.iter().enumerate() {
            if !v.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "initial state entry {i} = {v} is not finite"
                )));
            }
        }
        for c in 0..n {
            let s_l = initial[3 * c + 1];
            if !(0.0..=1.0).contains(&s_l) {
                return Err(PflotranError::InvalidInput(format!(
                    "initial liquid saturation for cell {c} = {s_l} is outside [0, 1]"
                )));
            }
        }
        Ok(Self {
            problem,
            solver: BlockNewtonSolver::new(general_newton_config()),
            state: initial,
            time: 0.0,
        })
    }

    /// Advance one backward-Euler step of size `dt` (s), returning the block
    /// Newton report.
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

    /// Current simulation time (s).
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Current liquid-pressure field `P_l` (Pa), de-interleaved, length `n_cells`.
    pub fn liquid_pressure(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[3 * c])
            .collect()
    }

    /// Current liquid-saturation field `S_l` (dimensionless), de-interleaved,
    /// length `n_cells`.
    pub fn liquid_saturation(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[3 * c + 1])
            .collect()
    }

    /// Current temperature field `T` (K), de-interleaved, length `n_cells`.
    pub fn temperature(&self) -> Vec<f64> {
        (0..self.problem.grid().n_cells())
            .map(|c| self.state[3 * c + 2])
            .collect()
    }

    /// The underlying problem (read-only), e.g. for grid access or the
    /// phase-mass / energy diagnostics.
    pub fn problem(&self) -> &GeneralFlow {
        &self.problem
    }

    /// The current interleaved state vector `[P_l, S_l, T]` per cell (length
    /// `3 * n_cells`).
    pub fn state(&self) -> &[f64] {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::VanGenuchten;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(x: f64) -> Length {
        Length::new::<meter>(x)
    }

    fn vg_curves() -> CharacteristicCurves {
        CharacteristicCurves::VanGenuchten(VanGenuchten::new(2.0e-4, 2.0, 0.1).unwrap())
    }

    /// Build a uniform interleaved initial state.
    fn uniform_state(n: usize, p0: f64, s0: f64, t0: f64) -> Vec<f64> {
        let mut v = vec![0.0; 3 * n];
        for c in 0..n {
            v[3 * c] = p0;
            v[3 * c + 1] = s0;
            v[3 * c + 2] = t0;
        }
        v
    }

    /// **Closed domain conserves water mass, gas mass, AND energy.**
    ///
    /// Methodology: a sealed 1D column (`n = 10`, all boundaries no-flow +
    /// adiabatic) started from a NON-uniform saturation ramp AND a non-uniform
    /// temperature ramp, so capillary redistribution and thermal conduction are
    /// both active. The discrete scheme is conservative by construction (internal
    /// phase mass fluxes, advected enthalpy, and conductive fluxes each cancel
    /// pairwise across shared faces; accumulation telescopes), so total water
    /// mass, total gas mass, and total bulk energy must each stay constant to the
    /// nonlinear-solver tolerance across several implicit steps.
    ///
    /// Results: with the tight [`general_newton_config`], the observed relative
    /// drift of each conserved total across 5 steps is at the solver-tolerance
    /// level, comfortably inside the asserted `1e-6` (water/gas) / `1e-6`
    /// (energy). The state is also asserted to have actually moved, so the test is
    /// not vacuous.
    #[test]
    fn closed_domain_conserves_water_gas_and_energy() {
        let n = 10usize;
        let grid = Arc::new(CartesianGrid::uniform(n, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap());
        let problem = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            0.0,    // no gravity
            vec![], // all no-flow + adiabatic
        )
        .unwrap();

        // Non-uniform saturation ramp 0.4..0.8 and temperature ramp 300..320 K.
        let mut state = vec![0.0; 3 * n];
        for c in 0..n {
            state[3 * c] = 1.0e5;
            state[3 * c + 1] = 0.4 + 0.4 * (c as f64) / (n as f64 - 1.0);
            state[3 * c + 2] = 300.0 + 20.0 * (c as f64) / (n as f64 - 1.0);
        }
        let s_init: Vec<f64> = (0..n).map(|c| state[3 * c + 1]).collect();
        let t_init: Vec<f64> = (0..n).map(|c| state[3 * c + 2]).collect();

        let water0 = problem.total_water_mass(&state);
        let gas0 = problem.total_gas_mass(&state);
        let energy0 = problem.total_energy(&state);

        let mut sim = GeneralSimulation::new(problem, state).unwrap();
        for _ in 0..5 {
            let report = sim.step(20.0).expect("closed GENERAL step converges");
            assert!(report.converged);
        }
        let state = sim.state().to_vec();

        // Must have actually redistributed (else conservation is trivial).
        let moved_s = (0..n)
            .map(|c| (state[3 * c + 1] - s_init[c]).abs())
            .fold(0.0, f64::max);
        let moved_t = (0..n)
            .map(|c| (state[3 * c + 2] - t_init[c]).abs())
            .fold(0.0, f64::max);
        assert!(
            moved_s > 1.0e-4,
            "saturation did not redistribute (moved={moved_s})"
        );
        assert!(
            moved_t > 1.0e-3,
            "temperature did not redistribute (moved={moved_t})"
        );

        let water1 = sim.problem().total_water_mass(&state);
        let gas1 = sim.problem().total_gas_mass(&state);
        let energy1 = sim.problem().total_energy(&state);

        let rel_w = (water1 - water0).abs() / water0.abs();
        let rel_g = (gas1 - gas0).abs() / gas0.abs();
        let rel_e = (energy1 - energy0).abs() / energy0.abs();
        assert!(
            rel_w < 1.0e-6,
            "water mass not conserved: rel drift {rel_w:e}"
        );
        assert!(
            rel_g < 1.0e-6,
            "gas mass not conserved: rel drift {rel_g:e}"
        );
        assert!(rel_e < 1.0e-6, "energy not conserved: rel drift {rel_e:e}");
    }

    /// **Uniform closed column is stationary (all three DOF).**
    ///
    /// A sealed no-flow + adiabatic column with a UNIFORM initial state must stay
    /// exactly put — every residual (both phase masses and energy) is zero, so a
    /// step leaves `P_l`, `S_l`, and `T` unchanged.
    #[test]
    fn closed_uniform_column_is_stationary() {
        let n = 8usize;
        let grid = Arc::new(CartesianGrid::uniform(n, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap());
        let problem = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            0.0,
            vec![],
        )
        .unwrap();
        let (p0, s0, t0) = (1.0e5, 0.6, 310.0);
        let mut sim = GeneralSimulation::new(problem, uniform_state(n, p0, s0, t0)).unwrap();

        let report = sim.step(100.0).unwrap();
        assert!(report.converged);
        for &p in &sim.liquid_pressure() {
            assert!((p - p0).abs() < 1.0e-3, "pressure drifted: {p} vs {p0}");
        }
        for &s in &sim.liquid_saturation() {
            assert!((s - s0).abs() < 1.0e-9, "saturation drifted: {s} vs {s0}");
        }
        for &t in &sim.temperature() {
            assert!((t - t0).abs() < 1.0e-9, "temperature drifted: {t} vs {t0}");
        }
    }

    /// **Isothermal-limit reduction:** with a uniform initial temperature, no heat
    /// source, and adiabatic boundaries, `T` stays uniform (the energy equation is
    /// inert) and the `(P_l, S_l)` evolution matches the isothermal two-phase
    /// imbibition behaviour: a wetter/higher-pressure Dirichlet face draws water
    /// in, the front advances, and every saturation stays in `[0, 1]`.
    ///
    /// The boundary Dirichlet temperature is set equal to the interior
    /// temperature, so no heat is injected and `T` must remain flat — confirming
    /// the `nb = 3` system reduces to the two-phase behaviour when energy is inert.
    #[test]
    fn isothermal_limit_reduces_to_two_phase_imbibition() {
        let n = 10usize;
        let t_uniform = 300.0;
        let grid = Arc::new(CartesianGrid::uniform(n, 1, 1, m(0.05), m(1.0), m(1.0)).unwrap());
        let curves = vg_curves();
        let sr = curves.residual_saturation();
        let boundary = vec![GeneralBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: GeneralBoundaryKind::Dirichlet {
                liquid_pressure: 1.2e5,
                liquid_saturation: 0.9,
                temperature: t_uniform, // same as interior => energy inert
            },
        }];
        let problem = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            curves,
            0.35,
            5.0e-12,
            0.0,
            boundary,
        )
        .unwrap();

        let s_init = 0.4;
        let mut sim =
            GeneralSimulation::new(problem, uniform_state(n, 1.0e5, s_init, t_uniform)).unwrap();

        for _ in 0..8 {
            let report = sim
                .step(20.0)
                .expect("isothermal imbibition step converges");
            assert!(report.converged);
            for &s in &sim.liquid_saturation() {
                assert!(
                    (sr - 1.0e-6..=1.0 + 1.0e-6).contains(&s),
                    "saturation out of [S_r, 1]: {s}"
                );
            }
            // Energy inert => temperature stays uniform.
            for &t in &sim.temperature() {
                assert!(
                    (t - t_uniform).abs() < 1.0e-4,
                    "temperature drifted from uniform in the isothermal limit: {t}"
                );
            }
        }

        let s = sim.liquid_saturation();
        assert!(
            s[0] > s_init + 1.0e-3,
            "inflow cell did not wet up: {}",
            s[0]
        );
        // Front at the wet boundary; monotone decreasing away from it.
        let s_max = s.iter().cloned().fold(f64::MIN, f64::max);
        assert!(
            (s[0] - s_max).abs() < 1.0e-9,
            "front is not at the wet boundary"
        );
        for w in s.windows(2) {
            assert!(
                w[1] <= w[0] + 1.0e-3,
                "saturation not monotone along x: {w:?}"
            );
        }
    }

    /// **Thermal coupling is alive:** a hot Dirichlet boundary raises the interior
    /// temperature over time (monotonically inward from the hot face), and —
    /// because `rho_l`/`mu_l` are temperature-dependent — measurably changes the
    /// flow versus an otherwise-identical isothermal (cold) case.
    ///
    /// Methodology: two identical imbibition columns (`n = 12`, `dx = 0.05 m`,
    /// wetter/higher-pressure Dirichlet face at XMin, `k = 1e-12 m^2`), one with a
    /// **hot** boundary (`T_bc = 340 K`) and one **isothermal** (`T_bc = 300 K`,
    /// equal to the interior). Both start cold (`T = 300 K`) and are marched 8
    /// steps of `50 s`.
    ///
    /// Note on the transport of heat: pure conduction only penetrates
    /// `sqrt(alpha t) ≈ 1.6 cm` over this time (`alpha ≈ 8.5e-7 m^2/s`), so the
    /// warming is carried inward mainly by **advection of hot liquid at the
    /// imbibition front** — which is exactly why the warmed region and the wetting
    /// front coincide.
    ///
    /// Results / expected direction: (a) in the hot case every cell stays within
    /// `[300, 340] K`, the near-boundary cell warms monotonically in time, and the
    /// temperature is monotone non-increasing away from the hot face. (b) Because
    /// liquid viscosity `mu_l(340 K)/mu_l(300 K) ≈ 0.49` (mobility roughly
    /// doubles), the hot column imbibes **faster**: its pore-volume liquid content
    /// `sum_i S_l,i` is strictly greater than the cold column's. Comparing
    /// `sum S_l` (rather than mass) isolates the mobility feedback from the
    /// competing `~0.8%` liquid-density drop. This shows temperature genuinely
    /// feeds back into the flow, not merely rides along it.
    #[test]
    fn thermal_coupling_hot_boundary_warms_and_changes_flow() {
        let n = 12usize;
        let t_cold = 300.0;
        let t_hot = 340.0;

        let make = |t_bc: f64| {
            let grid = Arc::new(CartesianGrid::uniform(n, 1, 1, m(0.05), m(1.0), m(1.0)).unwrap());
            let boundary = vec![GeneralBoundaryCondition {
                location: BoundaryLocation::XMin,
                kind: GeneralBoundaryKind::Dirichlet {
                    liquid_pressure: 1.15e5,
                    liquid_saturation: 0.85,
                    temperature: t_bc,
                },
            }];
            let problem = GeneralFlow::new(
                grid,
                GeneralFluids::air_water(),
                vg_curves(),
                0.35,
                1.0e-12,
                0.0,
                boundary,
            )
            .unwrap();
            GeneralSimulation::new(problem, uniform_state(n, 1.0e5, 0.4, t_cold)).unwrap()
        };

        let mut hot = make(t_hot);
        let mut cold = make(t_cold);

        let mut prev_boundary_t = t_cold;
        for _ in 0..8 {
            hot.step(50.0).expect("hot step converges");
            cold.step(50.0).expect("cold step converges");

            let th = hot.temperature();
            // Every cell stays within [t_cold, t_hot].
            for &t in &th {
                assert!(
                    t >= t_cold - 1.0e-6 && t <= t_hot + 1.0e-6,
                    "hot-case temperature {t} left [{t_cold}, {t_hot}]"
                );
            }
            // Near-boundary cell warms monotonically in time.
            assert!(
                th[0] >= prev_boundary_t - 1.0e-6,
                "near-boundary temperature must be non-decreasing: {} < {prev_boundary_t}",
                th[0]
            );
            prev_boundary_t = th[0];
            // Temperature is monotone non-increasing away from the hot face.
            for w in th.windows(2) {
                assert!(
                    w[1] <= w[0] + 1.0e-6,
                    "hot-case temperature not monotone inward: {w:?}"
                );
            }
            // Cold (isothermal) column: temperature must stay flat at t_cold.
            for &t in &cold.temperature() {
                assert!(
                    (t - t_cold).abs() < 1.0e-4,
                    "cold column drifted from isothermal: {t}"
                );
            }
        }

        // (a) the near-boundary region actually warmed above the initial cold value.
        assert!(
            hot.temperature()[0] > t_cold + 1.0,
            "near-boundary cell did not warm from the hot boundary: {}",
            hot.temperature()[0]
        );
        // (b) thermal feedback changed the flow: hotter (less viscous) liquid
        // imbibes faster, so the hot column holds more pore-volume liquid.
        let sum_s_hot: f64 = hot.liquid_saturation().iter().sum();
        let sum_s_cold: f64 = cold.liquid_saturation().iter().sum();
        assert!(
            sum_s_hot > sum_s_cold + 1.0e-4,
            "hot column should imbibe faster (mu_l lower): sum S_l hot={sum_s_hot}, cold={sum_s_cold}"
        );
    }

    /// **Newton convergence + boundedness:** a driven step reports converged, the
    /// residual history is (weakly) monotone non-increasing, saturations stay in
    /// `[0, 1]`, and pressures/temperatures stay finite.
    #[test]
    fn step_converges_and_stays_bounded() {
        let n = 8usize;
        let grid = Arc::new(CartesianGrid::uniform(n, 1, 1, m(0.05), m(1.0), m(1.0)).unwrap());
        let boundary = vec![GeneralBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: GeneralBoundaryKind::Dirichlet {
                liquid_pressure: 1.25e5,
                liquid_saturation: 0.85,
                temperature: 330.0,
            },
        }];
        let problem = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            5.0e-12,
            0.0,
            boundary,
        )
        .unwrap();
        let mut sim = GeneralSimulation::new(problem, uniform_state(n, 1.0e5, 0.4, 300.0)).unwrap();

        let report = sim.step(25.0).expect("driven GENERAL step converges");
        assert!(report.converged);
        for w in report.residual_history.windows(2) {
            assert!(
                w[1] <= w[0] * (1.0 + 1.0e-6) + 1.0e-12,
                "residual increased: {w:?}"
            );
        }
        for &p in &sim.liquid_pressure() {
            assert!(p.is_finite(), "non-finite pressure");
        }
        for &s in &sim.liquid_saturation() {
            assert!(
                (-1.0e-6..=1.0 + 1.0e-6).contains(&s),
                "saturation out of [0,1]: {s}"
            );
        }
        for &t in &sim.temperature() {
            assert!(t.is_finite() && t > 0.0, "non-physical temperature: {t}");
        }
    }

    /// Constructor / input validation.
    #[test]
    fn constructors_reject_bad_inputs() {
        // GeneralFluids validation.
        assert!(GeneralFluids::new(
            ThermalWaterProperties::water(),
            -1.0,
            1.8e-5,
            1005.0,
            RockThermalProperties::generic_rock(),
            298.15,
        )
        .is_err());
        assert!(GeneralFluids::new(
            ThermalWaterProperties::water(),
            1.2,
            1.8e-5,
            1005.0,
            RockThermalProperties::generic_rock(),
            -298.15, // bad reference temperature
        )
        .is_err());
        assert!(GeneralFluids::air_water().gas_density > 0.0);

        let grid = Arc::new(CartesianGrid::uniform(3, 1, 1, m(0.1), m(1.0), m(1.0)).unwrap());

        // Bad porosity.
        assert!(GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.0,
            1.0e-12,
            0.0,
            vec![]
        )
        .is_err());
        // Bad permeability.
        assert!(GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            -1.0e-12,
            0.0,
            vec![]
        )
        .is_err());
        // Negative gravity.
        assert!(GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            -9.8,
            vec![]
        )
        .is_err());

        // GeneralSimulation initial-state validation.
        let good = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            0.0,
            vec![],
        )
        .unwrap();
        // Wrong length.
        assert!(GeneralSimulation::new(good, vec![0.0; 3 * 3 - 1]).is_err());

        let good2 = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            0.0,
            vec![],
        )
        .unwrap();
        // Saturation out of [0,1].
        let mut bad_state = uniform_state(3, 1.0e5, 0.5, 300.0);
        bad_state[1] = 1.5;
        assert!(GeneralSimulation::new(good2, bad_state).is_err());

        // Negative dt at step time.
        let good3 = GeneralFlow::new(
            Arc::clone(&grid),
            GeneralFluids::air_water(),
            vg_curves(),
            0.35,
            1.0e-12,
            0.0,
            vec![],
        )
        .unwrap();
        let mut sim = GeneralSimulation::new(good3, uniform_state(3, 1.0e5, 0.5, 300.0)).unwrap();
        assert!(sim.step(-1.0).is_err());
    }
}
