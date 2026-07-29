// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/physicsSubSolvers/mechanicsSubSolver/smallStrain.C` and
// `mechanicsSubSolver.C`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Small-strain isotropic mechanics solver with eigenstrain loading.

use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::{Field, VolScalarField, VolSymmTensorField, VolVectorField};
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SymmTensor, Tensor, Vector3};

/// Isotropic linear-elastic material constants.
///
/// Stores Young's modulus and Poisson's ratio — the pair the fuel-performance
/// literature quotes — and derives the Lamé parameters the finite-volume
/// assembly actually needs. Constructed per cell from the correlations in
/// [`crate::materials::properties`].
///
/// # Units
///
/// Raw `f64` in strict SI: [`young`](Self::young) in pascal,
/// [`poisson`](Self::poisson) dimensionless. This type is built once per cell
/// per timestep inside the assembly loop, so it deliberately avoids `uom`
/// round-trips; see the crate-level units note.
///
/// # Valid range
///
/// `young > 0` and `−1 < poisson < 0.5`. The upper bound is not a convention:
/// at `poisson = 0.5` the material is incompressible, `λ` and `3K` diverge, and
/// the displacement formulation used here breaks down. [`Self::new`] rejects
/// values outside the open interval rather than returning infinities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearElastic {
    /// Young's modulus `E` \[Pa\].
    pub young: f64,
    /// Poisson's ratio `ν` \[-\], strictly between −1 and 0.5.
    pub poisson: f64,
}

impl LinearElastic {
    /// Build from Young's modulus \[Pa\] and Poisson's ratio \[-\].
    ///
    /// Returns [`OffbeatError::Unphysical`](crate::error::OffbeatError::Unphysical)
    /// for a non-positive modulus or a Poisson's ratio outside `(−1, 0.5)`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::mechanics::LinearElastic;
    ///
    /// // Room-temperature UO2, roughly.
    /// let m = LinearElastic::new(200.0e9, 0.32).unwrap();
    /// assert!(m.shear_modulus() > 0.0);
    ///
    /// // Incompressible is rejected, not silently infinite.
    /// assert!(LinearElastic::new(200.0e9, 0.5).is_err());
    /// ```
    pub fn new(young: f64, poisson: f64) -> crate::error::Result<Self> {
        if !(young > 0.0) {
            return Err(crate::error::OffbeatError::Unphysical {
                quantity: "Young's modulus",
                value: young,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(poisson > -1.0 && poisson < 0.5) {
            return Err(crate::error::OffbeatError::Unphysical {
                quantity: "Poisson's ratio",
                value: poisson,
                unit: "-",
                reason: "must lie strictly between -1 and 0.5; at 0.5 the material \
                         is incompressible and the displacement formulation diverges",
            });
        }
        Ok(Self { young, poisson })
    }

    /// Shear modulus `μ = E / (2(1+ν))` \[Pa\].
    #[must_use]
    pub fn shear_modulus(&self) -> f64 {
        self.young / (2.0 * (1.0 + self.poisson))
    }

    /// First Lamé parameter `λ = Eν / ((1+ν)(1−2ν))` \[Pa\].
    #[must_use]
    pub fn lame_lambda(&self) -> f64 {
        self.young * self.poisson / ((1.0 + self.poisson) * (1.0 - 2.0 * self.poisson))
    }

    /// Three times the bulk modulus, `3K = E / (1−2ν)` \[Pa\].
    ///
    /// Appears as a unit in its own right because the eigenstrain load is
    /// `∇(3K ε*)`: a linear eigenstrain `ε*` in a fully constrained body
    /// produces hydrostatic stress `−3K ε*`, and carrying `3K` avoids a
    /// scattering of factors of three.
    #[must_use]
    pub fn three_k(&self) -> f64 {
        self.young / (1.0 - 2.0 * self.poisson)
    }

    /// Implicit stiffness `2μ + λ` \[Pa\] used as the Laplacian coefficient.
    ///
    /// This is the coefficient of the implicit part of the segregated split; it
    /// is the one-dimensional (oedometric) stiffness, i.e. the stress per unit
    /// strain of a bar constrained against lateral contraction.
    #[must_use]
    pub fn two_mu_plus_lambda(&self) -> f64 {
        2.0 * self.shear_modulus() + self.lame_lambda()
    }
}

/// The stress-free strain a fuel material would undergo if unconstrained.
///
/// Every component is a **linear** (one-dimensional) strain \[-\], not a
/// volumetric one — the conversion from the volumetric quantities the
/// behavioural models return is the caller's, and
/// [`MaterialState::linear_swelling`](crate::materials::MaterialState::linear_swelling)
/// exists for exactly that.
///
/// # Sign convention
///
/// Positive means expansion. [`densification`](Self::densification) is therefore
/// normally **negative**. Getting that sign wrong makes densification reinforce
/// swelling instead of opposing it, and the resulting gap history looks
/// plausible while being wrong — hence the separate named fields rather than one
/// pre-summed number.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Eigenstrain {
    /// Linear thermal-expansion strain \[-\], `α(T − T_ref)`.
    pub thermal: f64,
    /// Linear fission-product swelling strain \[-\], positive.
    pub swelling: f64,
    /// Linear densification strain \[-\], normally negative.
    pub densification: f64,
    /// Linear relocation strain \[-\] from fuel cracking, positive.
    pub relocation: f64,
}

impl Eigenstrain {
    /// Purely thermal eigenstrain from `α ΔT`.
    ///
    /// `alpha` is the linear expansion coefficient \[1/K\] and `delta_t` the
    /// temperature rise above the reference \[K\].
    #[must_use]
    pub fn thermal(alpha: f64, delta_t: f64) -> Self {
        Self {
            thermal: alpha * delta_t,
            ..Self::default()
        }
    }

    /// Total linear eigenstrain \[-\] — the sum of all four contributions.
    ///
    /// This is the only quantity the momentum balance sees; the split into four
    /// named fields exists for the reader and for post-processing.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.thermal + self.swelling + self.densification + self.relocation
    }
}

/// Outcome of one mechanics solve.
///
/// Returned rather than logged so a caller can react — tighten the timestep,
/// report non-convergence, or drive an outer thermo-mechanical coupling loop on
/// [`converged`](Self::converged).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MechanicsReport {
    /// Outer corrector iterations performed.
    pub iterations: usize,
    /// Largest per-cell displacement change in the final iteration \[m\].
    pub final_change: f64,
    /// Whether `final_change` fell below the corrector tolerance.
    pub converged: bool,
    /// Largest displacement magnitude in the converged field \[m\].
    pub max_displacement: f64,
}

/// Segregated small-strain mechanics solver on a single mesh region.
///
/// # Typical use
///
/// Build it with the mesh, the material constants and the displacement boundary
/// conditions; set the eigenstrain field each timestep from the current
/// temperature and irradiation state; solve; read the stress.
///
/// ```no_run
/// use std::sync::Arc;
/// use outram_park_fork_offbeat::mechanics::{Eigenstrain, LinearElastic, MechanicsSolver};
/// # use outram_foam_basic_lib::mesh::FvMesh;
/// # fn demo(mesh: Arc<FvMesh>, clamped: Vec<outram_foam_basic_lib::fields::boundary::bc::PatchField<outram_foam_basic_lib::primitives::Vector3>>) {
/// let material = LinearElastic::new(200.0e9, 0.3).unwrap();
/// let mut solver = MechanicsSolver::new(mesh, material, clamped);
///
/// // 300 K rise with a linear expansion coefficient of 1e-5 /K.
/// solver.set_uniform_eigenstrain(Eigenstrain::thermal(1.0e-5, 300.0));
///
/// let report = solver.solve_quasi_static();
/// assert!(report.converged);
/// let sigma = solver.stress();
/// # }
/// ```
///
/// # Threading
///
/// The mesh is shared as `Arc<FvMesh>` and never mutated. The solver owns its
/// fields by value; share a whole solver across threads with an external
/// `Arc<RwLock<_>>` if a coupling layer needs to, per the workspace rule.
#[derive(Debug)]
pub struct MechanicsSolver {
    mesh: Arc<FvMesh>,
    material: LinearElastic,

    /// Displacement `D` \[m\], the solved field.
    disp: VolVectorField,
    /// `D` at the previous timestep \[m\] (transient form only).
    disp_old: VolVectorField,
    /// `D` two timesteps back \[m\] (transient form only).
    disp_old_old: VolVectorField,

    /// Cauchy stress `σ` \[Pa\], updated from the converged displacement.
    sigma: VolSymmTensorField,

    /// Total linear eigenstrain per cell \[-\].
    eigenstrain: VolScalarField,

    /// Density \[kg/m³\], used only by the transient (inertial) form.
    density: f64,

    n_correctors: usize,
    corrector_tol: f64,
    settings: SolverSettings,
}

impl MechanicsSolver {
    /// Build a solver on `mesh` for `material`, with `disp_boundary` giving the
    /// displacement boundary condition on every patch, in patch order.
    ///
    /// The displacement starts at zero everywhere, the eigenstrain at zero, and
    /// the stress at zero — i.e. the undeformed, unloaded reference state.
    ///
    /// # Panics
    ///
    /// Panics if `disp_boundary.len()` differs from the number of mesh patches.
    /// This is a programming error in the case setup, detectable immediately,
    /// rather than a condition a running solve could recover from.
    #[must_use]
    pub fn new(
        mesh: Arc<FvMesh>,
        material: LinearElastic,
        disp_boundary: Vec<PatchField<Vector3>>,
    ) -> Self {
        assert_eq!(
            disp_boundary.len(),
            mesh.patches.len(),
            "displacement boundary conditions must be supplied for every mesh patch"
        );
        let n = mesh.n_cells;
        let zero_vec = |name: &str| {
            VolVectorField::new(
                name,
                mesh.clone(),
                Field::new(vec![Vector3::new(0.0, 0.0, 0.0); n]),
                disp_boundary.clone(),
            )
        };
        let sigma = VolSymmTensorField::new(
            "sigma",
            mesh.clone(),
            Field::new(vec![SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0); n]),
            mesh.patches
                .iter()
                .map(|p| PatchField::zero_gradient_symm_tensor(p.size))
                .collect(),
        );

        Self {
            disp: zero_vec("D"),
            disp_old: zero_vec("D_0"),
            disp_old_old: zero_vec("D_00"),
            sigma,
            eigenstrain: VolScalarField::uniform("eigenstrain", mesh.clone(), 0.0),
            density: 10_960.0, // theoretical density of UO2; override with `set_density`
            mesh,
            material,
            n_correctors: 100,
            corrector_tol: 1.0e-10,
            // The library default (1e-7, 1000 sweeps) is tuned for a transient
            // CFD pressure solve that is warm-started from the previous step and
            // gets many chances to converge. This solve is different on both
            // counts: the displacement system is cold-started from zero at every
            // corrector, and the elastic Laplacian is stiff, so Gauss-Seidel needs
            // O(n^2) sweeps. Left at the default, the residual error grows with
            // mesh refinement and masquerades as a discretisation error — the
            // stress in a 80-cell column comes out ~1% high. These values are what
            // the analytic verification cases in `solver/tests.rs` require.
            settings: SolverSettings {
                tolerance: 1.0e-14,
                max_iter: 100_000,
            },
        }
    }

    /// Override the inner linear-solver settings.
    ///
    /// The default is a tolerance of 1e-14 over at most 100 000 Gauss-Seidel
    /// sweeps, which is deliberately much tighter than the library default —
    /// see the note in [`Self::new`]. Loosen it only if you have checked that
    /// the result is still mesh-converged: an under-converged elastic solve does
    /// not look wrong, it looks like a slightly different material.
    pub fn set_linear_solver(&mut self, settings: SolverSettings) {
        self.settings = settings;
    }

    /// Set the outer corrector budget and the displacement-change tolerance \[m\].
    ///
    /// The default is 100 iterations to 1e-10 m. The tolerance is an absolute
    /// displacement change, so scale it to the problem: 1e-10 m is
    /// sub-picometre, appropriate for a rod whose displacements are micrometres.
    pub fn set_corrector_control(&mut self, n_correctors: usize, tolerance: f64) {
        self.n_correctors = n_correctors;
        self.corrector_tol = tolerance;
    }

    /// Set the material density \[kg/m³\] used by the transient inertial term.
    ///
    /// Irrelevant to [`solve_quasi_static`](Self::solve_quasi_static), which
    /// drops inertia entirely.
    pub fn set_density(&mut self, density: f64) {
        self.density = density;
    }

    /// Impose the same total linear eigenstrain \[-\] in every cell.
    ///
    /// Convenient for verification cases and for a body at uniform temperature
    /// and burnup; a real rod uses
    /// [`set_eigenstrain_field`](Self::set_eigenstrain_field).
    pub fn set_uniform_eigenstrain(&mut self, eigenstrain: Eigenstrain) {
        let total = eigenstrain.total();
        for c in 0..self.mesh.n_cells {
            self.eigenstrain.internal[c] = total;
        }
    }

    /// Impose a per-cell total linear eigenstrain \[-\].
    ///
    /// `values` must have one entry per cell, each the *total* linear
    /// eigenstrain — i.e. [`Eigenstrain::total`] evaluated with that cell's
    /// temperature, burnup and irradiation state.
    ///
    /// # Panics
    ///
    /// Panics if `values.len()` differs from the cell count.
    pub fn set_eigenstrain_field(&mut self, values: &[f64]) {
        assert_eq!(
            values.len(),
            self.mesh.n_cells,
            "eigenstrain must be supplied for every cell"
        );
        for (c, &v) in values.iter().enumerate() {
            self.eigenstrain.internal[c] = v;
        }
    }

    /// The converged displacement field \[m\].
    #[must_use]
    pub fn displacement(&self) -> &VolVectorField {
        &self.disp
    }

    /// The Cauchy stress field \[Pa\], valid after a solve.
    ///
    /// Sign convention: **tension positive**, so a compressed pellet reports
    /// negative normal components.
    #[must_use]
    pub fn stress(&self) -> &VolSymmTensorField {
        &self.sigma
    }

    /// The material constants this solver was built with.
    #[must_use]
    pub fn material(&self) -> LinearElastic {
        self.material
    }

    /// Solve the quasi-static equilibrium `∇·σ = 0`.
    ///
    /// This is the form to use for fuel performance: a rod evolves over months,
    /// so inertia is irrelevant and including it merely adds a stiff transient
    /// that must be integrated through. Use
    /// [`solve_transient`](Self::solve_transient) only when a genuine dynamic
    /// event (a pellet-cladding impact, a reactivity pulse) is being modelled.
    pub fn solve_quasi_static(&mut self) -> MechanicsReport {
        self.solve_inner(None)
    }

    /// Solve the transient form `ρ ∂²D/∂t² = ∇·σ` over a timestep `dt` \[s\].
    ///
    /// Requires the displacement history, so call
    /// [`advance_time`](Self::advance_time) once per completed timestep.
    pub fn solve_transient(&mut self, dt: f64) -> MechanicsReport {
        self.solve_inner(Some(dt))
    }

    /// Rotate the displacement history (`D_00 ← D_0`, `D_0 ← D`).
    ///
    /// Call once per completed timestep, before the next
    /// [`solve_transient`](Self::solve_transient). Harmless but pointless for
    /// quasi-static solves.
    pub fn advance_time(&mut self) {
        for c in 0..self.mesh.n_cells {
            self.disp_old_old.internal[c] = self.disp_old.internal[c];
            self.disp_old.internal[c] = self.disp.internal[c];
        }
    }

    /// Shared assembly for the quasi-static (`dt = None`) and transient forms.
    fn solve_inner(&mut self, dt: Option<f64>) -> MechanicsReport {
        let mu = self.material.shear_modulus();
        let lambda = self.material.lame_lambda();
        let two_mu_lam = self.material.two_mu_plus_lambda();
        let three_k = self.material.three_k();
        let n = self.mesh.n_cells;

        let gamma = VolScalarField::uniform("2mu+lambda", self.mesh.clone(), two_mu_lam);
        let rho_field = VolScalarField::uniform("rho", self.mesh.clone(), self.density);

        // Eigenstrain load potential 3K ε*, whose gradient is the body force.
        // Built as its own field so `fvc::grad` applies the same boundary
        // treatment it would to any scalar.
        let load = {
            let mut f = VolScalarField::uniform("3K_eigenstrain", self.mesh.clone(), 0.0);
            for c in 0..n {
                f.internal[c] = three_k * self.eigenstrain.internal[c];
            }
            f
        };
        let grad_load = fvc::grad(&load);

        let mut final_change = f64::INFINITY;
        let mut iterations = 0;
        for iter in 0..self.n_correctors {
            iterations = iter + 1;

            // Explicit stress correction σ_e − (2μ+λ)∇D, which vanishes only in
            // the trivial case; it is what makes the segregated split exact at
            // convergence.
            let grad_d = fvc::grad_vec(&self.disp);
            let correction = Self::stress_correction_field(&grad_d, mu, lambda, two_mu_lam);
            let div_sigma_exp = fvc::div_tensor(&correction);

            let mut eqn = match dt {
                None => fvm::laplacian_vec(&gamma, &self.disp, self.mesh.clone()),
                Some(dt) => {
                    fvm::d2dt2_coeff(
                        &rho_field,
                        &self.disp,
                        &self.disp_old,
                        &self.disp_old_old,
                        dt,
                        self.mesh.clone(),
                    ) + fvm::laplacian_vec(&gamma, &self.disp, self.mesh.clone())
                }
            };

            // Body force (divSigmaExp − ∇(3K ε*)) integrated over each cell.
            for c in 0..n {
                let body = div_sigma_exp.internal[c] - grad_load.internal[c];
                eqn.source[c] += body * self.mesh.cell_volumes[c];
            }

            let (d_new, _perf) = eqn.solve("D", self.settings);

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

        self.update_stress(mu, lambda, three_k);

        let max_displacement = (0..n)
            .map(|c| self.disp.internal[c].mag())
            .fold(0.0_f64, f64::max);

        MechanicsReport {
            iterations,
            final_change,
            converged: final_change < self.corrector_tol,
            max_displacement,
        }
    }

    /// `σ_e − (2μ+λ)∇D`, the explicit part of the segregated split.
    ///
    /// With `σ_e = μ(∇D + ∇Dᵀ) + λ tr(∇D) I` this is
    /// `μ ∇Dᵀ + λ tr(∇D) I − (μ+λ) ∇D`. The eigenstrain term is deliberately
    /// absent — it enters as the separate `∇(3K ε*)` load, so that it is applied
    /// once and only once.
    fn stress_correction(grad_d: &Tensor, mu: f64, lambda: f64, two_mu_lam: f64) -> Tensor {
        let _ = two_mu_lam;
        let tr = grad_d.xx + grad_d.yy + grad_d.zz;
        let t = grad_d.transpose();
        let mu_lam = mu + lambda;
        Tensor::new(
            mu * t.xx + lambda * tr - mu_lam * grad_d.xx,
            mu * t.xy - mu_lam * grad_d.xy,
            mu * t.xz - mu_lam * grad_d.xz,
            mu * t.yx - mu_lam * grad_d.yx,
            mu * t.yy + lambda * tr - mu_lam * grad_d.yy,
            mu * t.yz - mu_lam * grad_d.yz,
            mu * t.zx - mu_lam * grad_d.zx,
            mu * t.zy - mu_lam * grad_d.zy,
            mu * t.zz + lambda * tr - mu_lam * grad_d.zz,
        )
    }

    /// Recompute `σ = μ(∇D + ∇Dᵀ) + λ tr(∇D) I − 3K ε* I` from the current
    /// displacement.
    fn update_stress(&mut self, mu: f64, lambda: f64, three_k: f64) {
        let grad_d = fvc::grad_vec(&self.disp);
        for c in 0..self.mesh.n_cells {
            let g = grad_d.internal[c];
            let tr = g.xx + g.yy + g.zz;
            let eig = three_k * self.eigenstrain.internal[c];
            let hydro = lambda * tr - eig;
            self.sigma.internal[c] = SymmTensor::new(
                2.0 * mu * g.xx + hydro,
                mu * (g.xy + g.yx),
                mu * (g.xz + g.zx),
                2.0 * mu * g.yy + hydro,
                mu * (g.yz + g.zy),
                2.0 * mu * g.zz + hydro,
            );
        }
    }
}

// The correction is written per-cell; this wrapper keeps the field-level call
// readable in `solve_inner`.
impl MechanicsSolver {
    fn stress_correction_field(
        grad_d: &outram_foam_basic_lib::fields::VolTensorField,
        mu: f64,
        lambda: f64,
        two_mu_lam: f64,
    ) -> outram_foam_basic_lib::fields::VolTensorField {
        let mut out = grad_d.clone();
        for c in 0..out.internal.len() {
            out.internal[c] = Self::stress_correction(&grad_d.internal[c], mu, lambda, two_mu_lam);
        }
        out
    }
}

#[cfg(test)]
mod tests;
