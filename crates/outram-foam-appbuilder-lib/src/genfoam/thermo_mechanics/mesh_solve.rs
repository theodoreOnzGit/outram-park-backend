// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/legacyThermoMechanics/
//                    include/solveThermalMechanics.H  (segregated DEqn + TEqn),
//                    include/calculateStress.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   The DEqn/TEqn discretisation is derived from OpenFOAM's
//   solidDisplacementFoam (C) 2011 OpenFOAM Foundation.
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Displacement + structure-heat field solve on the mechanics `FvMesh`
//!
//! The finite-volume, segregated field solve of GeN-Foam's `legacyThermoMechanics`
//! model — the mesh-level counterpart of the pointwise constitutive kernel in
//! [`super::stress`] / [`super::material`]. Ported from
//! `include/solveThermalMechanics.H`, whose two solves are:
//!
//! - **`TEqn`** — the structure heat-diffusion equation for the temperature field
//!   `TStruct` on the mechanics mesh (feeds `ΔT` into the thermal stress), and
//! - **`DEqn`** — the small-strain linear-elastic displacement-momentum equation
//!   for the displacement field `D`, in the `solidDisplacementFoam`
//!   compact-normal-stress segregated form.
//!
//! ## The displacement equation (`DEqn`)
//!
//! The momentum balance of a linear-elastic solid is `∇·σ = ρ ∂²D/∂t²` with the
//! thermo-elastic Cauchy stress
//!
//! ```text
//!   σ = μ (∇D + ∇Dᵀ) + λ tr(∇D) I − 3K α (T − T_ref) I
//!     = 2μ ε + λ tr(ε) I − 3K α ΔT I
//! ```
//!
//! (`ε = symm(∇D)`; `μ`, `λ` the Lamé parameters; `3K = 3λ + 2μ` the bulk-modulus
//! term; `α` the linear thermal-expansion coefficient). Splitting the elastic
//! divergence into an implicit Laplacian on `D` plus an explicit correction
//! (OpenFOAM's *compact normal stress* form),
//!
//! ```text
//!   ∇·σ = ∇·[(2μ+λ) ∇D]                      (implicit, fvm::laplacian)
//!       + ∇·[σ_D − (2μ+λ) ∇D]                (explicit, divSigmaExp = fvc::div)
//!       − ∇(3K α ΔT)                          (explicit thermal load, fvc::grad)
//! ```
//!
//! with `σ_D = μ twoSymm(∇D) + λ tr(∇D) I`. The assembled system solved each
//! outer (segregated) corrector is
//!
//! ```text
//!   ρ ∂²D/∂t² − ∇·[(2μ+λ)∇D] = divSigmaExp − ∇(3K α ΔT)
//! ```
//!
//! and the explicit `divSigmaExp` is refreshed from the freshly-solved `D`,
//! iterating to convergence. In the **quasi-static** form the inertial
//! `ρ ∂²D/∂t²` term is dropped (a pure elliptic equilibrium solve, `∇·σ = 0`).
//!
//! ## Operators consumed from `outram_foam_basic_lib`
//!
//! `fvm::laplacian_vec`, `fvm::d2dt2_coeff`, `fvc::grad`, `fvc::grad_vec`,
//! `fvc::div_tensor`, and the scalar `fvm::laplacian` for `TEqn`.
//!
//! ## Known limitations (inherited from the basic-lib operator layer)
//!
//! - **Scalar-coefficient Laplacian only.** `fvm::laplacian_vec` takes an
//!   isotropic scalar `2μ+λ`; an anisotropic stiffness tensor is not expressible
//!   and would need a new operator.
//! - **Single Gauss gradient, no non-orthogonal correction.** `fvc::grad_vec`
//!   sets a *zero-gradient* output boundary, so the explicit `divSigmaExp`
//!   correction carries no boundary-gradient information; near a boundary it is
//!   approximate. On a 1-D column the correction is identically zero (its
//!   non-zero entries are transverse normal stresses with no transverse
//!   gradient), so the implicit Laplacian is exact there — which is what the V&V
//!   cases below exploit.
//! - **Constant-Δt `d2dt2`.** The transient inertial term assumes a fixed
//!   timestep and a non-moving mesh.
//!
//! A traction (Neumann) boundary condition on `D` that imposes a non-zero stress
//! (e.g. a stress-free surface under thermal load, `σ·n = 0 ⇒ ∂D/∂n ≠ 0`) is not
//! yet available — the field boundary layer offers only Dirichlet (`FixedValue`)
//! and zero-gradient. The stress-free free-expansion case is therefore reached
//! here through a non-uniform temperature field with Dirichlet ends rather than a
//! traction surface (see the V&V tests). A `tractionDisplacement` boundary
//! condition is a basic-lib follow-up.
//!
//! ## What this does not include
//!
//! The separate 1-D axial fuel / control-rod expansion solve (`fuelDispEqn` /
//! `CRDispEqn`) and its column-to-mesh mapping remain in [`super::feedback`] as
//! the closed-form cumulative integral; wiring those onto a shared multi-region
//! mesh belongs to `genfoam::multi_region`. The [`MechanicsMeshSolver`] exposes
//! [`MechanicsMeshSolver::mesh_displacement`] to assemble the coupled `meshDisp`
//! per cell via [`super::assemble_mesh_displacement`] once an axial-expansion
//! field is supplied.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolSymmTensorField, VolVectorField};
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::ldu_matrix::{SolverPerformance, SolverSettings};
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SymmTensor, Tensor, Vector3};

use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::pressure::pascal;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::stress::thermal_stress;
use super::{assemble_mesh_displacement, LegacyThermoMechanics};

/// Outcome of a segregated [`MechanicsMeshSolver::solve_displacement`] run.
///
/// The residual is the maximum per-cell change in the displacement field between
/// the last two outer correctors (metres); `converged` is `true` when that fell
/// below the configured tolerance before the corrector budget was spent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplacementReport {
    /// Number of outer (segregated) correctors actually performed.
    pub n_correctors: usize,
    /// Maximum per-cell displacement change on the final corrector (m).
    pub final_change: f64,
    /// Whether the segregated loop converged within the corrector budget.
    pub converged: bool,
    /// Peak displacement magnitude in the solved field (m).
    pub max_displacement: f64,
}

/// Segregated finite-volume solver for `legacyThermoMechanics` on one mechanics
/// [`FvMesh`]: the displacement field `D` and the structure temperature
/// `TStruct`, with the thermo-elastic stress `σ` as output.
///
/// The solver owns its fields so that boundary conditions (set once on the
/// supplied `disp`/`t_struct` fields) persist across the outer correctors and
/// across time steps — only the internal cell values are updated in place.
///
/// # Units of the owned fields (raw SI, per the FV operator layer)
///
/// `outram_foam_basic_lib`'s field layer is un-`uom`'d, so field element values
/// are plain `f64` in SI base units: displacement `D` in **metres**, temperature
/// `TStruct` in **kelvin** (absolute), stress `σ` in **pascals**. The public
/// *scalar* configuration (`t_struct_ref`, `dt`) and summary getters use `uom`.
///
/// # Example
///
/// ```no_run
/// # use std::sync::Arc;
/// # use outram_foam_basic_lib::mesh::FvMesh;
/// # use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
/// # use outram_foam_basic_lib::ldu_matrix::SolverSettings;
/// # use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
/// #     ElasticMaterial, LegacyThermoMechanics, RheologyOption, YoungsModulus, ThermalExpansionCoeff,
/// #     MechanicsMeshSolver,
/// # };
/// # use uom::si::f64::{
/// #     MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature, Time,
/// # };
/// # use uom::si::{
/// #     mass_density::kilogram_per_cubic_meter, pressure::gigapascal, ratio::ratio,
/// #     specific_heat_capacity::joule_per_kilogram_kelvin, temperature_coefficient::per_kelvin,
/// #     thermal_conductivity::watt_per_meter_kelvin, thermodynamic_temperature::kelvin, time::second,
/// # };
/// # fn build(mesh: Arc<FvMesh>, disp: VolVectorField, t: VolScalarField) {
/// let steel = ElasticMaterial::new(
///     YoungsModulus::new::<gigapascal>(200.0), Ratio::new::<ratio>(0.3),
///     ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
///     MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
///     SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
///     ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
/// ).unwrap();
/// let model = LegacyThermoMechanics::new(steel, RheologyOption::PlaneStrain);
/// let mut solver = MechanicsMeshSolver::new(
///     model, ThermodynamicTemperature::new::<kelvin>(300.0),
///     Time::new::<second>(1.0), SolverSettings::default(), disp, t,
/// );
/// solver.solve_temperature(None);
/// let report = solver.solve_displacement(true); // quasi-static
/// # let _ = report;
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MechanicsMeshSolver {
    mesh: Arc<FvMesh>,
    model: LegacyThermoMechanics,
    t_struct_ref: ThermodynamicTemperature,
    dt: Time,
    settings: SolverSettings,
    n_correctors: usize,
    corrector_tol: f64,
    disp: VolVectorField,
    disp_old: VolVectorField,
    disp_old_old: VolVectorField,
    t_struct: VolScalarField,
    sigma: VolSymmTensorField,
}

impl MechanicsMeshSolver {
    /// Build a solver on the mesh carried by `disp`.
    ///
    /// # Parameters
    ///
    /// - `model` — the linear-elastic model ([`LegacyThermoMechanics`]) whose
    ///   material and rheology supply `μ`, `λ`, `3K`, `α`, `ρ`, `k`.
    /// - `t_struct_ref` — the stress-free reference temperature `T_ref` (the
    ///   thermal stress uses `ΔT = TStruct − T_ref`).
    /// - `dt` — the timestep for the transient inertial term (ignored in the
    ///   quasi-static solve).
    /// - `settings` — linear-solver settings for the inner LDU solves.
    /// - `disp` — the initial displacement field (m); **its boundary conditions
    ///   are the mechanical BCs of the problem** and are preserved across the
    ///   solve. Typically zero internally with `FixedValue`/zero-gradient patches.
    /// - `t_struct` — the initial structure temperature field (K, absolute), with
    ///   its thermal boundary conditions.
    ///
    /// The displacement history (`D_old`, `D_oldold`) is initialised to `disp`.
    pub fn new(
        model: LegacyThermoMechanics,
        t_struct_ref: ThermodynamicTemperature,
        dt: Time,
        settings: SolverSettings,
        disp: VolVectorField,
        t_struct: VolScalarField,
    ) -> Self {
        let mesh = disp.mesh.clone();
        let sigma = zero_symm_tensor_field("sigma", mesh.clone());
        Self {
            mesh,
            model,
            t_struct_ref,
            dt,
            settings,
            n_correctors: 20,
            corrector_tol: 1e-9,
            disp_old: disp.clone(),
            disp_old_old: disp.clone(),
            disp,
            t_struct,
            sigma,
        }
    }

    /// Override the segregated outer-corrector budget and convergence tolerance
    /// (max per-cell displacement change, metres). Defaults: 20 correctors,
    /// `1e-9 m`.
    pub fn set_corrector_control(&mut self, n_correctors: usize, tol: f64) {
        self.n_correctors = n_correctors.max(1);
        self.corrector_tol = tol;
    }

    /// The solved displacement field `D` (m).
    pub fn displacement(&self) -> &VolVectorField {
        &self.disp
    }

    /// The structure temperature field `TStruct` (K, absolute).
    pub fn temperature(&self) -> &VolScalarField {
        &self.t_struct
    }

    /// The thermo-elastic stress field `σ` (Pa) from the most recent
    /// [`Self::solve_displacement`].
    pub fn stress(&self) -> &VolSymmTensorField {
        &self.sigma
    }

    // ── TEqn ────────────────────────────────────────────────────────────────

    /// Solve the steady structure heat-diffusion equation `−∇·(k ∇TStruct) = q'''`.
    ///
    /// Ports the diffusion + volumetric-source part of upstream's `TEqn`
    /// (`fvm::laplacian(rhoK, TStruct) + powerDensityNeutronics`). Conductivity
    /// `k` is taken from the model's material and assumed uniform and isotropic.
    /// The temperature boundary conditions are those set on the owned `t_struct`
    /// field; only its internal values are updated.
    ///
    /// # Parameters
    ///
    /// - `power_density` — optional volumetric heat source `q'''` (W/m³) as a
    ///   cell field (`powerDensityNeutronics`); `None` for pure conduction.
    ///
    /// # Returns
    ///
    /// The inner linear-solver [`SolverPerformance`].
    ///
    /// # Note
    ///
    /// This is the steady (elliptic) form. The transient `fvm::ddt(ρc, T)` term
    /// and the porous `TStructFromTH` clamping of the upstream equation are not
    /// assembled here; the steady solve is what the field-solve V&V below
    /// exercises. Adding the `ddt` term is a mechanical extension once transient
    /// TH coupling is wired.
    pub fn solve_temperature(
        &mut self,
        power_density: Option<&VolScalarField>,
    ) -> SolverPerformance {
        let k = self
            .model
            .material()
            .conductivity()
            .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
        // Uniform, isotropic conductivity as a face field. NOTE: build the
        // boundary face values explicitly as `fixed_value(k)` — the convenience
        // `SurfaceScalarField::uniform` leaves boundary faces at zero, which would
        // silently null the diffusion coefficient on every boundary face (and so
        // drop the Dirichlet contributions to the matrix).
        let k_surf = SurfaceScalarField::new(
            "k",
            self.mesh.clone(),
            Field::uniform(self.mesh.n_internal_faces, k),
            self.mesh
                .patches
                .iter()
                .map(|p| PatchField::fixed_value(p.size, k))
                .collect(),
        );

        // basic-lib `fvm::laplacian` assembles the operator `−∇·(k∇T)·V` (positive
        // diagonal, symmetric) with Dirichlet contributions already in its source,
        // so the system `M·x = source` solves `−∇·(k∇T) = 0`. A volumetric source
        // q''' adds `q'''·V` to the right-hand side. The matrix is SPD, so the
        // conjugate-gradient solver is used.
        let mut teqn = fvm::laplacian(&k_surf, &self.t_struct);
        if let Some(q) = power_density {
            for c in 0..self.mesh.n_cells {
                teqn.source[c] += q.internal[c] * self.mesh.cell_volumes[c];
            }
        }

        let (t_new, perf) = teqn.solve_cg("TStruct", self.settings);
        // Preserve the field's boundary conditions; update interior only.
        for c in 0..self.mesh.n_cells {
            self.t_struct.internal[c] = t_new.internal[c];
        }
        perf
    }

    // ── DEqn ────────────────────────────────────────────────────────────────

    /// Solve the linear-elastic displacement equation `∇·σ = ρ ∂²D/∂t²` on the
    /// mesh, segregated (compact-normal-stress), iterating the explicit stress
    /// divergence to convergence, and update the stress field `σ`.
    ///
    /// # Parameters
    ///
    /// - `quasi_static` — when `true`, drop the inertial `ρ ∂²D/∂t²` term and
    ///   solve the elliptic equilibrium `∇·σ = 0`. When `false`, include the
    ///   constant-Δt Euler `fvm::d2dt2(ρ, D)` transient term using the stored
    ///   displacement history (advance it between steps with
    ///   [`Self::advance_time`]).
    ///
    /// # Returns
    ///
    /// A [`DisplacementReport`] with the corrector count, final per-cell change,
    /// convergence flag, and peak displacement.
    pub fn solve_displacement(&mut self, quasi_static: bool) -> DisplacementReport {
        let material = self.model.material();
        let rheology = self.model.rheology();
        let mu = material.shear_modulus().get::<pascal>();
        let lambda = material.lame_lambda(rheology).get::<pascal>();
        let two_mu_lam = 2.0 * mu + lambda;
        let three_k = material.bulk_modulus_three_k(rheology).get::<pascal>();
        let alpha = material
            .thermal_expansion()
            .get::<uom::si::temperature_coefficient::per_kelvin>();
        let rho = material
            .density()
            .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
        let t_ref = self.t_struct_ref.get::<kelvin>();
        let dt = self.dt.get::<second>();
        let n = self.mesh.n_cells;

        // Isotropic implicit stiffness γ = 2μ + λ.
        let gamma = VolScalarField::uniform("2mu+lam", self.mesh.clone(), two_mu_lam);
        // Density field for the transient inertial term.
        let rho_field = VolScalarField::uniform("rho", self.mesh.clone(), rho);

        // Explicit thermal load 3K α (TStruct − T_ref) as a scalar field, mapping
        // the same boundary values as TStruct, then its gradient ∇(3K α ΔT).
        let thermal_scalar = self.thermal_load_field(three_k * alpha, t_ref);
        let grad_thermal = fvc::grad(&thermal_scalar);

        let mut final_change = f64::INFINITY;
        let mut correctors = 0;
        for iter in 0..self.n_correctors {
            correctors = iter + 1;

            // Explicit stress-divergence correction:
            //   divSigmaExp = ∇·[σ_D − (2μ+λ) ∇D],  σ_D = μ twoSymm(∇D) + λ tr(∇D) I
            let grad_d = fvc::grad_vec(&self.disp);
            let correction = self.compact_stress_correction(&grad_d, mu, lambda, two_mu_lam);
            let div_sigma_exp = fvc::div_tensor(&correction);

            // Assemble the implicit system.
            let mut eqn = if quasi_static {
                fvm::laplacian_vec(&gamma, &self.disp, self.mesh.clone())
            } else {
                fvm::d2dt2_coeff(
                    &rho_field,
                    &self.disp,
                    &self.disp_old,
                    &self.disp_old_old,
                    dt,
                    self.mesh.clone(),
                ) + fvm::laplacian_vec(&gamma, &self.disp, self.mesh.clone())
            };

            // Body force to the right-hand side: (divSigmaExp − ∇(3K α ΔT))·V.
            for c in 0..n {
                let body = div_sigma_exp.internal[c] - grad_thermal.internal[c];
                eqn.source[c] += body * self.mesh.cell_volumes[c];
            }

            let (d_new, _perf) = eqn.solve("D", self.settings);

            // Max per-cell change, then update interior (preserving BCs).
            final_change = 0.0;
            for c in 0..n {
                let delta = (d_new.internal[c] - self.disp.internal[c]).mag();
                if delta > final_change {
                    final_change = delta;
                }
                self.disp.internal[c] = d_new.internal[c];
            }

            if final_change < self.corrector_tol {
                break;
            }
        }

        // Update the stress field σ from the converged displacement.
        self.update_stress(mu, lambda, three_k, alpha, t_ref);

        let mut max_disp = 0.0_f64;
        for c in 0..n {
            let m = self.disp.internal[c].mag();
            if m > max_disp {
                max_disp = m;
            }
        }

        DisplacementReport {
            n_correctors: correctors,
            final_change,
            converged: final_change < self.corrector_tol,
            max_displacement: max_disp,
        }
    }

    /// Rotate the displacement history for the next transient step
    /// (`D_oldold ← D_old`, `D_old ← D`). Call once per completed timestep before
    /// the next [`Self::solve_displacement`] with `quasi_static = false`.
    pub fn advance_time(&mut self) {
        for c in 0..self.mesh.n_cells {
            self.disp_old_old.internal[c] = self.disp_old.internal[c];
            self.disp_old.internal[c] = self.disp.internal[c];
        }
    }

    // ── meshDisp coupling ────────────────────────────────────────────────────

    /// Assemble the coupled mesh-displacement field `meshDisp` (m) per cell.
    ///
    /// Applies [`super::assemble_mesh_displacement`] cell-by-cell:
    /// `meshDisp = (D − (D·ê) ê) + axial ê`, i.e. the transverse-to-pin part of
    /// the elastic displacement plus the axially-resolved fuel/control-rod
    /// expansion `axial` along the (normalised) pin direction `ê`. This is the
    /// vector GeN-Foam maps onto the neutronics/TH meshes.
    ///
    /// # Parameters
    ///
    /// - `pin_direction` — the pin orientation `ê` (`fuelOrientation`), normalised
    ///   internally.
    /// - `axial` — the axial fuel+CR expansion field `fuelDisp + CRDisp` (m) along
    ///   `ê`, one value per cell (from [`super::axial_expansion_profile`] mapped
    ///   onto the column, or zero for pure elastic feedback).
    ///
    /// # Returns
    ///
    /// A `meshDisp` [`VolVectorField`] (m) with a zero-gradient boundary.
    pub fn mesh_displacement(
        &self,
        pin_direction: Vector3,
        axial: &VolScalarField,
    ) -> VolVectorField {
        use uom::si::f64::Length;
        use uom::si::length::meter;
        let n = self.mesh.n_cells;
        let internal = Field::from_fn(n, |c| {
            assemble_mesh_displacement(
                self.disp.internal[c],
                Length::new::<meter>(axial.internal[c]),
                pin_direction,
            )
        });
        let boundary = self
            .mesh
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        VolVectorField::new("meshDisp", self.mesh.clone(), internal, boundary)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Build the explicit thermal-load scalar field `3K α (TStruct − T_ref)`
    /// (Pa), mapping the temperature field's boundary values so its gradient is
    /// evaluated consistently at the boundaries.
    fn thermal_load_field(&self, three_k_alpha: f64, t_ref: f64) -> VolScalarField {
        let n = self.mesh.n_cells;
        let internal = Field::from_fn(n, |c| three_k_alpha * (self.t_struct.internal[c] - t_ref));
        // Preserve the temperature field's Dirichlet boundary *values* (mapped
        // through 3Kα(·−T_ref)) so `fvc::grad` interpolates the true boundary
        // thermal load rather than an owner-cell extrapolation; a non-Dirichlet
        // patch degrades to zero-gradient.
        let boundary = self
            .t_struct
            .boundary
            .iter()
            .map(|pf| {
                let values = pf.values.map(|t| three_k_alpha * (t - t_ref));
                let bc = match &pf.bc {
                    BoundaryCondition::FixedValue(v) => {
                        BoundaryCondition::FixedValue(three_k_alpha * (v - t_ref))
                    }
                    BoundaryCondition::FixedField(f) => {
                        BoundaryCondition::FixedField(f.map(|t| three_k_alpha * (t - t_ref)))
                    }
                    _ => BoundaryCondition::ZeroGradient,
                };
                PatchField { bc, values }
            })
            .collect();
        VolScalarField::new("3Kalpha.dT", self.mesh.clone(), internal, boundary)
    }

    /// Build the compact-normal-stress correction tensor field
    /// `σ_D − (2μ+λ) ∇D` (Pa), whose divergence is the explicit `divSigmaExp`.
    fn compact_stress_correction(
        &self,
        grad_d: &outram_foam_basic_lib::fields::vol_field::VolTensorField,
        mu: f64,
        lambda: f64,
        two_mu_lam: f64,
    ) -> outram_foam_basic_lib::fields::vol_field::VolTensorField {
        let corr = |g: Tensor| -> Tensor {
            // σ_D = μ (∇D + ∇Dᵀ) + λ tr(∇D) I
            let sigma_d = mu * g.two_symm() + (lambda * g.tr()) * SymmTensor::IDENTITY;
            Tensor::from(sigma_d) - two_mu_lam * g
        };
        let internal = grad_d.internal.map(|g| corr(*g));
        let boundary = grad_d
            .boundary
            .iter()
            .map(|pf| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: pf.values.map(|g| corr(*g)),
            })
            .collect();
        outram_foam_basic_lib::fields::vol_field::VolTensorField::new(
            "sigmaD-corr",
            self.mesh.clone(),
            internal,
            boundary,
        )
    }

    /// Recompute the full thermo-elastic stress field `σ` (Pa) from the current
    /// displacement and temperature, reusing the pointwise [`thermal_stress`]
    /// kernel per cell (`ε = symm(∇D)`, `ΔT = TStruct − T_ref`).
    fn update_stress(&mut self, mu: f64, lambda: f64, three_k: f64, alpha: f64, t_ref: f64) {
        use uom::si::f64::TemperatureInterval;
        let grad_d = fvc::grad_vec(&self.disp);
        let mu_q = super::ShearModulus::new::<pascal>(mu);
        let lam_q = super::LameLambda::new::<pascal>(lambda);
        let three_k_q = super::BulkModulusTerm::new::<pascal>(three_k);
        let alpha_q = super::ThermalExpansionCoeff::new::<
            uom::si::temperature_coefficient::per_kelvin,
        >(alpha);
        let n = self.mesh.n_cells;
        for c in 0..n {
            let strain = grad_d.internal[c].symm();
            let dt = TemperatureInterval::new::<kelvin_interval>(self.t_struct.internal[c] - t_ref);
            self.sigma.internal[c] = thermal_stress(strain, mu_q, lam_q, three_k_q, alpha_q, dt);
        }
    }
}

/// A zero symmetric-tensor field with a zero-gradient boundary on `mesh`.
fn zero_symm_tensor_field(name: &str, mesh: Arc<FvMesh>) -> VolSymmTensorField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_symm_tensor(p.size))
        .collect();
    VolSymmTensorField::new(
        name,
        mesh.clone(),
        Field::uniform(mesh.n_cells, SymmTensor::ZERO),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::field::Field;
    use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use uom::si::f64::{
        MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature,
        Time,
    };
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::gigapascal;
    use uom::si::ratio::ratio;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    use super::super::material::{ElasticMaterial, RheologyOption, YoungsModulus};
    use super::super::ThermalExpansionCoeff;

    /// A uniform 1-D "bar" mesh of `n` cells over `[0, L]`, faces normal to x,
    /// unit cross-section. Two boundary patches: `right` (x = L) then `left`
    /// (x = 0), matching the ordering the basic-lib operator tests use.
    fn bar_mesh(n: usize, length: f64) -> Arc<FvMesh> {
        let dx = length / n as f64;
        let n_int = n - 1;
        let mut owner = (0..n_int).collect::<Vec<_>>();
        let neighbour = (1..n).collect::<Vec<_>>();
        owner.push(n - 1); // right boundary owner
        owner.push(0); // left boundary owner
        let cell_volumes = vec![dx; n]; // unit cross-section
        let cell_centres = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0))
            .collect();
        let face_centres: Vec<_> = (0..n_int)
            .map(|f| Vector3::new((f as f64 + 1.0) * dx, 0.0, 0.0))
            .chain([Vector3::new(length, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0)])
            .collect();
        let face_area_vectors: Vec<_> = (0..n_int)
            .map(|_| Vector3::new(1.0, 0.0, 0.0))
            .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)])
            .collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_int)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("right", n_int, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", n_int + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(cell_volumes)
                .cell_centres(cell_centres)
                .face_area_vectors(face_area_vectors)
                .face_centres(face_centres)
                .build()
                .unwrap(),
        )
    }

    /// Steel-like material, but with a chosen Poisson ratio (ν = 0 decouples the
    /// transverse directions so a 1-D uniaxial-strain bar reduces to the textbook
    /// 1-D thermo-elastic problem: `2μ+λ = 3K = E`).
    fn material(nu: f64) -> ElasticMaterial {
        ElasticMaterial::new(
            YoungsModulus::new::<gigapascal>(200.0),
            Ratio::new::<ratio>(nu),
            ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
            MassDensity::new::<kilogram_per_cubic_meter>(7850.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
            ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
        )
        .unwrap()
    }

    /// Displacement field on `mesh` with fixed (`FixedValue`) end displacements
    /// `left`/`right` (m along x) and zero interior.
    fn clamped_disp(mesh: Arc<FvMesh>, left: f64, right: f64) -> VolVectorField {
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::new(right, 0.0, 0.0)),
                values: Field::new(vec![Vector3::new(right, 0.0, 0.0)]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::new(left, 0.0, 0.0)),
                values: Field::new(vec![Vector3::new(left, 0.0, 0.0)]),
            },
        ];
        VolVectorField::new(
            "D",
            mesh.clone(),
            Field::new(vec![Vector3::ZERO; mesh.n_cells]),
            boundary,
        )
    }

    /// Temperature field on `mesh` with fixed end temperatures (K) and a linear
    /// interior initial guess.
    fn end_temp_field(mesh: Arc<FvMesh>, t_left: f64, t_right: f64) -> VolScalarField {
        let n = mesh.n_cells;
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(t_right),
                values: Field::new(vec![t_right]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(t_left),
                values: Field::new(vec![t_left]),
            },
        ];
        let internal = Field::from_fn(n, |c| {
            let x = (c as f64 + 0.5) / n as f64;
            t_left + (t_right - t_left) * x
        });
        VolScalarField::new("TStruct", mesh, internal, boundary)
    }

    fn solver(
        _mesh: Arc<FvMesh>,
        nu: f64,
        t_ref: f64,
        disp: VolVectorField,
        t: VolScalarField,
    ) -> MechanicsMeshSolver {
        let model = LegacyThermoMechanics::new(material(nu), RheologyOption::PlaneStrain);
        MechanicsMeshSolver::new(
            model,
            ThermodynamicTemperature::new::<kelvin>(t_ref),
            Time::new::<second>(1.0),
            // The DEqn uses the Gauss-Seidel vector solve, which needs a generous
            // sweep budget to reach a tight tolerance on these ~40-cell 1-D
            // Poisson systems (the SPD scalar TEqn uses CG internally).
            SolverSettings {
                tolerance: 1e-11,
                max_iter: 40000,
            },
            disp,
            t,
        )
    }

    /// V&V — TEqn steady conduction with a volumetric source (parabolic profile).
    ///
    /// Methodology: a bar of length L = 1 m, conductivity k = 15 W/(m·K), both
    /// ends held at T_ref = 300 K, with a uniform volumetric source
    /// q''' = 3e6 W/m³. The steady conduction equation −k T'' = q''' with
    /// T(0) = T(L) = T_ref has the analytical solution
    /// `T(x) − T_ref = (q'''/2k)·x(L − x)`, peaking at mid-span with
    /// `ΔT_max = q''' L²/(8k) = 3e6·1/(8·15) = 25000 K`.
    ///
    /// Result (measured 2026-07-15, 40 cells, Δx = 0.025 m): the near-mid cell
    /// (index 20, centre x = 0.5125 m) reads ΔT = 25000.0 K; the continuous
    /// parabola at that x is 24984.4 K — the finite-volume cell-averaged value
    /// sits above it by q'''Δx²/(8k) = 15.6 K. Measured relative error against
    /// the continuous-at-x reference is 6.25e-4 (< 5e-3 pass criterion). Pass.
    #[test]
    fn teqn_conduction_with_source_matches_parabola() {
        let n = 40;
        let l = 1.0;
        let k = 15.0;
        let q = 3.0e6;
        let t_ref = 300.0;
        let mesh = bar_mesh(n, l);
        let disp = clamped_disp(mesh.clone(), 0.0, 0.0);
        let t = end_temp_field(mesh.clone(), t_ref, t_ref);
        let mut s = solver(mesh.clone(), 0.0, t_ref, disp, t);
        let q_field = VolScalarField::uniform("q", mesh.clone(), q);
        let perf = s.solve_temperature(Some(&q_field));
        assert!(perf.converged, "TEqn residual = {}", perf.final_residual);

        // Discrete parabola peak (cell-centred): analytical minus the FV defect.
        let dx = l / n as f64;
        let dt_analytic = q * l * l / (8.0 * k);
        let mid = n / 2;
        let dt_num = s.temperature().internal[mid] - t_ref;
        // Cell-centred value at x = L/2 - dx/2; compare against the continuous
        // parabola evaluated there, which the 2nd-order scheme reproduces exactly
        // up to the source discretisation.
        let x_mid = (mid as f64 + 0.5) * dx;
        let dt_exact_at_x = q / (2.0 * k) * x_mid * (l - x_mid);
        assert!(
            (dt_num - dt_exact_at_x).abs() / dt_analytic < 5e-3,
            "ΔT(mid) = {dt_num} K, exact-at-x = {dt_exact_at_x} K"
        );
    }

    /// V&V — coupled TEqn → DEqn, fully-constrained bar, linear temperature.
    ///
    /// Methodology: a bar of length L = 1 m clamped at both ends (D = 0 at x = 0
    /// and x = L), with a steady linear temperature rise from TEqn
    /// (T(0) = T_ref = 300 K, T(L) = 400 K, no source ⇒ ΔT(x) = T0·x/L,
    /// T0 = 100 K). The 1-D uniaxial-strain equilibrium
    /// `(2μ+λ) D'' = 3K α ΔT'(x)` with D(0) = D(L) = 0 and constant ΔT' = T0/L
    /// has the closed-form solution
    /// `D(x) = [3K α T0 / (2 (2μ+λ) L)]·x(x − L)`, minimised at mid-span with
    /// `D_mid = − 3K α T0 L / (8 (2μ+λ))`.
    ///
    /// With ν = 0 (so 2μ+λ = 3K = E) this collapses to `D_mid = − α T0 L / 8 =
    /// − 1.2e-5·100·1/8 = − 1.5e-4 m = − 0.15 mm`, independent of E.
    ///
    /// Result (measured 2026-07-15, 40 cells, ν = 0): the TEqn linear profile is
    /// exact to < 1e-6 K; the DEqn converges after 2 outer correctors (change
    /// = 0 on the 2nd — divSigmaExp ≡ 0 on a 1-D mesh, so the implicit Laplacian
    /// is exact in one solve). The near-mid cell (index 20) reads D = −0.1500000
    /// mm, matching the analytical mid-span −0.15 mm to 2e-12 mm; it sits
    /// 9.4e-8 m from the continuous parabola at the cell centre (the same
    /// cell-average FV defect). Pass.
    #[test]
    fn deqn_linear_temperature_constrained_bar() {
        let n = 40;
        let l = 1.0;
        let t_ref = 300.0;
        let t0 = 100.0;
        let alpha = 1.2e-5;
        let mesh = bar_mesh(n, l);
        let disp = clamped_disp(mesh.clone(), 0.0, 0.0);
        let t = end_temp_field(mesh.clone(), t_ref, t_ref + t0);
        let mut s = solver(mesh.clone(), 0.0, t_ref, disp, t);

        let perf = s.solve_temperature(None);
        assert!(perf.converged);
        // Linear profile: interior cell at x should read T_ref + T0·x/L.
        let dx = l / n as f64;
        for c in 0..n {
            let x = (c as f64 + 0.5) * dx;
            let expect = t_ref + t0 * x / l;
            assert!(
                (s.temperature().internal[c] - expect).abs() < 1e-6,
                "T[{c}] = {}",
                s.temperature().internal[c]
            );
        }

        let report = s.solve_displacement(true);
        assert!(report.converged, "DEqn change = {}", report.final_change);

        // Analytical mid-span displacement (ν = 0 ⇒ − α T0 L / 8).
        let d_mid_analytic = -alpha * t0 * l / 8.0; // metres
        let mid = n / 2;
        // Cell-centred: compare against the continuous parabola at x_mid.
        let x_mid = (mid as f64 + 0.5) * dx;
        let d_exact_at_x = alpha * t0 / (2.0 * l) * x_mid * (x_mid - l);
        let d_num = s.displacement().internal[mid].x;
        assert!(
            (d_num - d_exact_at_x).abs() < 2e-7,
            "D(mid) = {} m, exact-at-x = {} m",
            d_num,
            d_exact_at_x
        );
        // Sanity: within the discretisation the mid value tracks the closed form.
        assert!(
            (d_num - d_mid_analytic).abs() < 2e-6,
            "D(mid) = {} mm vs analytic {} mm",
            d_num * 1e3,
            d_mid_analytic * 1e3
        );

        // Equilibrium: the stress divergence vanishes ⇒ σ_xx nearly uniform and
        // small (traction-free interior balance), von Mises bounded.
        let max_vm = (0..n)
            .map(|c| {
                let sdev = s.stress().internal[c].dev();
                (1.5 * sdev.mag_sqr()).sqrt()
            })
            .fold(0.0_f64, f64::max);
        assert!(max_vm.is_finite());
    }

    /// V&V — DEqn fully-constrained bar, uniform heating (zero displacement,
    /// pure thermal stress) through the full field solve.
    ///
    /// Methodology: a bar clamped at both ends (D = 0) heated uniformly by
    /// ΔT = 100 K. With a spatially-uniform temperature the thermal-load gradient
    /// ∇(3K α ΔT) vanishes in the interior and both ends hold D = 0, so the field
    /// solve must return D ≈ 0 everywhere; the recovered stress is the textbook
    /// fully-constrained thermal stress σ_xx = −3K α ΔT. With ν = 0
    /// (3K = E = 200 GPa): σ_xx = −200e9·1.2e-5·100 = −240 MPa.
    ///
    /// Result (measured 2026-07-15, 20 cells): max |D| < 1e-12 m; interior
    /// σ_xx = −240.000 MPa (|err| < 1e-3 MPa). Pass.
    #[test]
    fn deqn_uniform_heating_constrained_bar_thermal_stress() {
        let n = 20;
        let l = 1.0;
        let t_ref = 300.0;
        let dt = 100.0;
        let alpha = 1.2e-5;
        let e = 200.0e9; // ν = 0 ⇒ 3K = E
        let mesh = bar_mesh(n, l);
        let disp = clamped_disp(mesh.clone(), 0.0, 0.0);
        // Uniform temperature field T = T_ref + ΔT everywhere (ends included).
        let t = end_temp_field(mesh.clone(), t_ref + dt, t_ref + dt);
        let mut s = solver(mesh.clone(), 0.0, t_ref, disp, t);
        // No conduction solve needed (uniform field); go straight to DEqn.
        let report = s.solve_displacement(true);
        assert!(report.converged);
        assert!(
            report.max_displacement < 1e-12,
            "max |D| = {} m should vanish",
            report.max_displacement
        );

        let sigma_xx = s.stress().internal[n / 2].xx;
        let expect = -e * alpha * dt; // −240 MPa
        assert!(
            (sigma_xx - expect).abs() / expect.abs() < 1e-6,
            "σ_xx = {} MPa vs {} MPa",
            sigma_xx / 1e6,
            expect / 1e6
        );
    }

    /// V&V — meshDisp assembly wired through the solver reproduces the feedback
    /// kernel per cell.
    ///
    /// Methodology: with a known displacement field and pin direction ê = +z, the
    /// solver's [`MechanicsMeshSolver::mesh_displacement`] must equal the
    /// per-cell [`super::super::assemble_mesh_displacement`]: keep the radial
    /// (x,y) part of D, drop its axial (z) part, and substitute the supplied
    /// axial expansion. Here D = (0,0,0) (undeformed) and a uniform axial field
    /// of 2 mm ⇒ meshDisp = (0,0,0.002) m in every cell.
    ///
    /// Result (measured 2026-07-15): meshDisp = (0, 0, 0.002) m in all cells,
    /// |err| < 1e-12 m. Pass.
    #[test]
    fn mesh_displacement_wires_axial_expansion() {
        let n = 8;
        let mesh = bar_mesh(n, 1.0);
        let disp = clamped_disp(mesh.clone(), 0.0, 0.0);
        let t = end_temp_field(mesh.clone(), 300.0, 300.0);
        let s = solver(mesh.clone(), 0.3, 300.0, disp, t);
        let axial = VolScalarField::uniform("axial", mesh.clone(), 0.002); // 2 mm
        let md = s.mesh_displacement(Vector3::new(0.0, 0.0, 1.0), &axial);
        for c in 0..n {
            assert!(md.internal[c].x.abs() < 1e-12);
            assert!(md.internal[c].y.abs() < 1e-12);
            assert!(
                (md.internal[c].z - 0.002).abs() < 1e-12,
                "z = {}",
                md.internal[c].z
            );
        }
    }
}
