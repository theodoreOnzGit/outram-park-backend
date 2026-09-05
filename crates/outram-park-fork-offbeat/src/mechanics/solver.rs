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

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;
use crate::rheology::{
    equivalent_strain, CreepTimeStepControl, IrradiationState, Rheology, RheologyInputs,
    RheologyState, StressCorrection,
};

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

    /// Build from a material's own correlations at a given state — **refusing
    /// the case** when the correlation pair leaves the admissible range.
    ///
    /// # The policy this encodes
    ///
    /// MATPRO fits Zircaloy's Young's and shear moduli as *independent* lines,
    /// so `ν = E/(2G) − 1` crosses the incompressible limit `0.5` at
    /// `T = 1354.84` K and reaches `0.912` at 1800 K — the top of upstream's
    /// own stated validity range. At 600 K a retained cold-work fraction above
    /// `0.1197` does the same. That is a faithful port of a real upstream
    /// defect; upstream neither detects nor guards it.
    ///
    /// Three policies were available (bead `op-6sl.7`): clamp `ν` below `0.5`,
    /// fall back to a constant ratio, or refuse. **This refuses**, and the
    /// reason is worth stating because clamping looks like the accommodating
    /// choice and is in fact the worst one: `λ = Eν/((1+ν)(1−2ν))` grows
    /// without bound as `ν → 0.5`, so a clamp at `0.5 − ε` makes `λ ≈ E/(3ε)`
    /// — **the clamp tolerance, not the material, would set the stiffness.**
    /// A silent answer that is wrong by however many orders of magnitude the
    /// author of `ε` happened to choose is worse than no answer. Falling back
    /// to a constant `ν` is defensible but substitutes a different material
    /// without saying so, which is the same failure wearing a hat.
    ///
    /// A caller that genuinely wants an approximation above the crossover can
    /// still build one explicitly with [`new`](Self::new) and document the
    /// choice at the call site, where a reader will see it.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`](crate::error::OffbeatError::Unphysical)
    /// naming Poisson's ratio and the offending value, when
    /// [`PoissonRatioModel::is_admissible`](crate::materials::properties::poisson_ratio::PoissonRatioModel::is_admissible)
    /// is false — plus whatever either
    /// model's own `value_checked` rejects (an out-of-range temperature, say).
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;
    /// use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
    /// use outram_park_fork_offbeat::mechanics::LinearElastic;
    ///
    /// let young = YoungModulusModel::MatproZircaloy;
    /// let poisson = PoissonRatioModel::MatproZircaloy;
    ///
    /// // Normal operating temperature: fine.
    /// let cold = MaterialState::fresh(600.0);
    /// assert!(LinearElastic::from_models(young, poisson, &cold).is_ok());
    ///
    /// // Above the 1354.84 K crossover the case is refused, not clamped.
    /// let hot = MaterialState::fresh(1500.0);
    /// assert!(LinearElastic::from_models(young, poisson, &hot).is_err());
    /// ```
    pub fn from_models(
        young: crate::materials::properties::young_modulus::YoungModulusModel,
        poisson: crate::materials::properties::poisson_ratio::PoissonRatioModel,
        state: &crate::materials::MaterialState,
    ) -> crate::error::Result<Self> {
        let e = young.value_checked(state)?;
        let nu = poisson.value_checked(state)?;
        if !poisson.is_admissible(state) {
            return Err(crate::error::OffbeatError::Unphysical {
                quantity: "Poisson's ratio from the material correlation",
                value: nu,
                unit: "-",
                reason: "outside (-1, 0.5), so the elasticity tensor is not positive \
                         definite. The case is refused rather than clamped: lambda \
                         diverges as nu approaches 0.5, so a clamp tolerance would set \
                         the stiffness instead of the material. See bead op-6sl.7",
            });
        }
        Self::new(e, nu)
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

/// Outcome of one inelastic (creep/plasticity) timestep.
///
/// Returned by [`MechanicsSolver::solve_creep_step`]. Carries the mechanics
/// convergence report plus the per-step inelastic increments a caller needs to
/// pick the next timestep and to decide whether the step was acceptable.
///
/// # Units — raw `f64`, strict SI
///
/// Strains dimensionless, time in second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreepStepReport {
    /// Convergence of the displacement/stress corrector loop.
    pub mechanics: MechanicsReport,

    /// Largest single-cell equivalent **inelastic** increment `Δε_c,eq +
    /// Δε_p,eq` \[-\] over the step.
    ///
    /// This is upstream's `maxCreep` diagnostic and the quantity
    /// [`CreepTimeStepControl::max_maximum_increment`] bounds.
    pub max_equivalent_inelastic_increment: f64,

    /// Volume-averaged equivalent inelastic increment \[-\] over the step,
    /// `Σ_c V_c Δε_eq,c / Σ_c V_c`.
    ///
    /// Upstream's `averageCreep`, bounded by
    /// [`CreepTimeStepControl::max_average_increment`].
    pub average_equivalent_inelastic_increment: f64,

    /// Largest single-cell equivalent **creep** increment \[-\] alone.
    pub max_equivalent_creep_increment: f64,

    /// Largest single-cell equivalent **plastic** increment \[-\] alone.
    pub max_equivalent_plastic_increment: f64,

    /// Number of cells that yielded plastically during the step.
    ///
    /// Upstream prints the same count as a percentage each step; a sudden jump
    /// is the usual first sign that the timestep is too large.
    pub yielding_cells: usize,

    /// Timestep \[s\] the creep control suggests for the **next** step.
    ///
    /// [`f64::INFINITY`] when no limit binds (the default control imposes
    /// none), so a coupling layer can `min` it against every other physics
    /// module's suggestion. Set a real bound with
    /// [`MechanicsSolver::set_creep_time_step_control`].
    pub suggested_next_time_step: f64,
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

    /// The inelastic driver, absent for a purely elastic case.
    rheology: Option<Rheology>,

    /// Per-cell constitutive history at the **start** of the current timestep.
    ///
    /// Advanced exactly once per completed
    /// [`solve_creep_step`](Self::solve_creep_step), never inside the corrector
    /// loop — see the module documentation of [`crate::rheology`], item 3.
    states: Vec<RheologyState>,

    /// Per-cell **total accumulated** inelastic strain `ε_p + ε_c` \[-\], the
    /// additional (tensor) eigenstrain fed back into the momentum balance.
    ///
    /// This is upstream's `additionalStrain`, and it is what makes the
    /// corrected, softer stress an equilibrium stress again.
    inelastic: Vec<SymmTensor>,

    /// Per-cell mechanical strain \[-\] at the end of the previous timestep,
    /// used only to form the equivalent strain rate the FRAPTRAN yield model
    /// asks for.
    mech_strain_old: Vec<SymmTensor>,

    /// Per-cell composition/temperature/irradiation history.
    material_states: Vec<MaterialState>,

    /// Per-cell instantaneous irradiation environment.
    irradiation: Vec<IrradiationState>,

    /// Bound on how much inelastic strain one step may accumulate.
    time_step_control: CreepTimeStepControl,

    n_correctors: usize,
    corrector_tol: f64,
    inelastic_tol: f64,
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
            rheology: None,
            states: vec![RheologyState::pristine(); n],
            inelastic: vec![SymmTensor::ZERO; n],
            mech_strain_old: vec![SymmTensor::ZERO; n],
            // Room temperature, unirradiated: the beginning-of-life state, and
            // the only one that can be assumed without knowing the case.
            // Override with `set_uniform_material_state` /
            // `set_material_state_field`.
            material_states: vec![MaterialState::fresh(293.15); n],
            irradiation: vec![IrradiationState::default(); n],
            time_step_control: CreepTimeStepControl::default(),
            mesh,
            material,
            n_correctors: 100,
            corrector_tol: 1.0e-10,
            // Absolute change in the accumulated inelastic strain tensor
            // between two outer correctors. Strain is dimensionless and the
            // increments of interest are micro-strain, so 1e-14 is several
            // orders below anything physical while remaining reachable in
            // double precision.
            inelastic_tol: 1.0e-14,
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

    /// Attach an inelastic driver, turning
    /// [`solve_creep_step`](Self::solve_creep_step) on.
    ///
    /// Every cell starts from [`RheologyState::pristine`]; the accumulated
    /// inelastic strain is reset to zero, because a history built against a
    /// different constitutive law is not meaningful.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Mesh`] if the driver's cell count differs from the
    /// mesh's — a case-setup error, caught once here rather than per cell per
    /// timestep.
    pub fn set_rheology(&mut self, rheology: Rheology) -> Result<()> {
        if rheology.n_cells() != self.mesh.n_cells {
            return Err(OffbeatError::Mesh(format!(
                "rheology covers {} cells but the mesh has {}",
                rheology.n_cells(),
                self.mesh.n_cells
            )));
        }
        self.rheology = Some(rheology);
        self.states = vec![RheologyState::pristine(); self.mesh.n_cells];
        self.inelastic = vec![SymmTensor::ZERO; self.mesh.n_cells];
        self.mech_strain_old = vec![SymmTensor::ZERO; self.mesh.n_cells];
        Ok(())
    }

    /// Give every cell the same composition, temperature and irradiation
    /// history.
    ///
    /// Convenient for a verification case at uniform temperature; a real rod
    /// uses [`set_material_state_field`](Self::set_material_state_field).
    pub fn set_uniform_material_state(&mut self, state: MaterialState) {
        for s in &mut self.material_states {
            *s = state;
        }
    }

    /// Set the per-cell composition, temperature and irradiation history.
    ///
    /// # Panics
    ///
    /// Panics if `values.len()` differs from the cell count.
    pub fn set_material_state_field(&mut self, values: &[MaterialState]) {
        assert_eq!(
            values.len(),
            self.mesh.n_cells,
            "material state must be supplied for every cell"
        );
        self.material_states.copy_from_slice(values);
    }

    /// Give every cell the same fast flux, fission rate and grain radius.
    pub fn set_uniform_irradiation(&mut self, irradiation: IrradiationState) {
        for s in &mut self.irradiation {
            *s = irradiation;
        }
    }

    /// Set the per-cell fast flux, fission rate and grain radius.
    ///
    /// # Panics
    ///
    /// Panics if `values.len()` differs from the cell count.
    pub fn set_irradiation_field(&mut self, values: &[IrradiationState]) {
        assert_eq!(
            values.len(),
            self.mesh.n_cells,
            "irradiation state must be supplied for every cell"
        );
        self.irradiation.copy_from_slice(values);
    }

    /// Bound how much inelastic strain a single step may accumulate.
    ///
    /// The default imposes no bound, matching upstream, and
    /// [`CreepStepReport::suggested_next_time_step`] is then always infinite.
    /// The creep integration is implicit in stress but **explicit in state** —
    /// the yield stress, the hardening and the creep-rate correlations are all
    /// evaluated at the start-of-step state — so an unbounded step silently
    /// loses accuracy rather than diverging. That is why the bound matters.
    pub fn set_creep_time_step_control(&mut self, control: CreepTimeStepControl) {
        self.time_step_control = control;
    }

    /// Set the convergence tolerance \[-\] on the accumulated inelastic strain
    /// between two outer correctors.
    ///
    /// The default is 1e-14. This is checked **in addition** to the
    /// displacement tolerance of
    /// [`set_corrector_control`](Self::set_corrector_control): a step in which
    /// the displacement has stopped moving but the inelastic strain has not is
    /// not converged.
    pub fn set_inelastic_tolerance(&mut self, tolerance: f64) {
        self.inelastic_tol = tolerance;
    }

    /// This cell's accumulated plastic and creep history.
    ///
    /// Valid between timesteps; during a
    /// [`solve_creep_step`](Self::solve_creep_step) it is still the
    /// start-of-step value, by construction.
    ///
    /// # Panics
    ///
    /// Panics if `cell` is outside the mesh.
    #[must_use]
    pub fn rheology_state(&self, cell: usize) -> RheologyState {
        self.states[cell]
    }

    /// This cell's total accumulated inelastic strain `ε_p + ε_c` \[-\].
    ///
    /// The additional eigenstrain the momentum balance is currently carrying —
    /// upstream's `additionalStrain`.
    ///
    /// # Panics
    ///
    /// Panics if `cell` is outside the mesh.
    #[must_use]
    pub fn inelastic_strain(&self, cell: usize) -> SymmTensor {
        self.inelastic[cell]
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
        let (report, _) = self
            .solve_inner(None, None)
            .expect("a solve with no rheology attached cannot fail");
        report
    }

    /// Solve the transient form `ρ ∂²D/∂t² = ∇·σ` over a timestep `dt` \[s\].
    ///
    /// Requires the displacement history, so call
    /// [`advance_time`](Self::advance_time) once per completed timestep.
    pub fn solve_transient(&mut self, dt: f64) -> MechanicsReport {
        let (report, _) = self
            .solve_inner(Some(dt), None)
            .expect("a solve with no rheology attached cannot fail");
        report
    }

    /// Advance one quasi-static timestep of length `dt` \[s\] **with the
    /// attached constitutive law integrated**, and commit the result.
    ///
    /// This is the wired-up version of
    /// [`solve_quasi_static`](Self::solve_quasi_static): the same segregated
    /// displacement solve, with the inelastic (creep + plastic) correction
    /// inside the corrector loop and the resulting inelastic strain fed back as
    /// an additional eigenstrain. It is the finite-volume analogue of
    /// upstream's `correctAdditionalStrain`.
    ///
    /// # What one call does, in order
    ///
    /// Repeat until the displacement and the inelastic strain both stop moving
    /// (or the corrector budget runs out):
    ///
    /// 1. Solve `∇·[(2μ+λ)∇D] + divSigmaExp − ∇(3K ε*) − ∇·[2μ ε_in + λ tr(ε_in) I] = 0`
    ///    for the displacement `D`, with `ε_in` the inelastic strain currently
    ///    believed.
    /// 2. Form the total strain `ε = ½(∇D + ∇Dᵀ)` and subtract the isotropic
    ///    eigenstrain: `ε_mech = ε − ε* I`. **This subtraction is the whole
    ///    point** — see the trap below.
    /// 3. Integrate the constitutive law in every cell, always from the *same*
    ///    start-of-step [`RheologyState`], and take the new inelastic strain to
    ///    be `ε_mech − ε_el`.
    ///
    /// Then, exactly once: write the corrected stress into
    /// [`stress`](Self::stress) and [`RheologyState::advance`] every cell.
    ///
    /// # The two traps this method exists to close
    ///
    /// **Eigenstrain subtraction.** A freely expanding, unconstrained pellet
    /// has a large total strain and *zero* stress. Hand the constitutive law
    /// the total strain instead of the mechanical strain and it sees a
    /// hydrostatic `3K ε*` — of order 1.5 GPa for a 300 K temperature rise —
    /// which is then fed back into the momentum balance as if it were real.
    ///
    /// **Advancing once.** [`RheologyState::advance`] is called after the
    /// corrector loop, never inside it. Advancing inside would compound the
    /// inelastic increment once per corrector, which over-predicts creep
    /// silently: the answer stays smooth and plausible and is simply wrong by
    /// the corrector count.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Mesh`] if no rheology has been attached (call
    ///   [`set_rheology`](Self::set_rheology) first).
    /// - [`OffbeatError::Unphysical`] if `dt` is negative.
    /// - Every error [`Rheology::correct`] can raise — in particular
    ///   [`OffbeatError::ConstitutiveNotConverged`], which normally means the
    ///   timestep is too large for the creep rate. Nothing is committed when
    ///   the step fails: the state is left exactly as it was.
    pub fn solve_creep_step(&mut self, dt: f64) -> Result<CreepStepReport> {
        if self.rheology.is_none() {
            return Err(OffbeatError::Mesh(
                "solve_creep_step needs a constitutive law; call set_rheology first".to_string(),
            ));
        }
        if !(dt >= 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must be non-negative",
            });
        }

        let (mechanics, corrections) = self.solve_inner(None, Some(dt))?;

        // Commit — once, after the corrector loop has converged.
        let mut max_inelastic = 0.0_f64;
        let mut max_creep = 0.0_f64;
        let mut max_plastic = 0.0_f64;
        let mut yielding_cells = 0usize;
        let mut volume_weighted = 0.0_f64;
        let mut total_volume = 0.0_f64;

        for (c, correction) in corrections.iter().enumerate() {
            let inelastic = correction.equivalent_creep_strain_increment
                + correction.equivalent_plastic_strain_increment;
            max_inelastic = max_inelastic.max(inelastic);
            max_creep = max_creep.max(correction.equivalent_creep_strain_increment);
            max_plastic = max_plastic.max(correction.equivalent_plastic_strain_increment);
            if correction.yielding {
                yielding_cells += 1;
            }
            let volume = self.mesh.cell_volumes[c];
            volume_weighted += volume * inelastic;
            total_volume += volume;

            self.sigma.internal[c] = correction.stress;
            self.states[c].advance(correction);
            self.mech_strain_old[c] = correction.elastic_strain
                + self.states[c].plastic_strain
                + self.states[c].creep_strain;
        }

        let average = if total_volume > 0.0 {
            volume_weighted / total_volume
        } else {
            0.0
        };

        Ok(CreepStepReport {
            mechanics,
            max_equivalent_inelastic_increment: max_inelastic,
            average_equivalent_inelastic_increment: average,
            max_equivalent_creep_increment: max_creep,
            max_equivalent_plastic_increment: max_plastic,
            yielding_cells,
            suggested_next_time_step: self.time_step_control.next_time_step(
                average,
                max_inelastic,
                dt,
            ),
        })
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

    /// Shared assembly for the quasi-static (`inertial_dt = None`), transient
    /// and inelastic forms.
    ///
    /// `creep_dt` selects the inelastic path: `None` leaves the material purely
    /// elastic (and the returned correction vector empty), `Some(dt)` runs the
    /// attached [`Rheology`] inside the corrector loop over a step of `dt`
    /// seconds. Nothing is committed here — the caller owns the decision to
    /// advance the per-cell state.
    fn solve_inner(
        &mut self,
        inertial_dt: Option<f64>,
        creep_dt: Option<f64>,
    ) -> Result<(MechanicsReport, Vec<StressCorrection>)> {
        let dt = inertial_dt;
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

        // Cloned once so the corrector loop can mutate `self` while dispatching
        // on the law. `Rheology` is a `Vec` of enum variants plus an `Arc`ed
        // cell map, so this copies a handful of words, not the mesh.
        let rheology = self.rheology.clone();

        let mut corrections: Vec<StressCorrection> = Vec::new();
        let mut final_change = f64::INFINITY;
        let mut inelastic_change = 0.0_f64;
        let mut iterations = 0;
        for iter in 0..self.n_correctors {
            iterations = iter + 1;

            // Explicit stress correction σ_e − (2μ+λ)∇D, which vanishes only in
            // the trivial case; it is what makes the segregated split exact at
            // convergence.
            let grad_d = fvc::grad_vec(&self.disp);
            let correction = Self::stress_correction_field(&grad_d, mu, lambda, two_mu_lam);
            let div_sigma_exp = fvc::div_tensor(&correction);
            // Additional (tensor) eigenstrain load from the inelastic strain:
            // −∇·[2μ ε_in + λ tr(ε_in) I]. Zero while no rheology is attached.
            let div_inelastic = self.inelastic_load(mu, lambda);

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

            // Body force (divSigmaExp − ∇(3K ε*) − ∇·σ_in) integrated over each
            // cell.
            for c in 0..n {
                let body =
                    div_sigma_exp.internal[c] - grad_load.internal[c] - div_inelastic.internal[c];
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

            // Integrate the constitutive law on the freshly solved strain, and
            // update the additional eigenstrain from it. Always from the
            // start-of-step state: `self.states` is untouched here.
            inelastic_change = 0.0;
            if let (Some(rheology), Some(step)) = (rheology.as_ref(), creep_dt) {
                let grad_d = fvc::grad_vec(&self.disp);
                corrections.clear();
                corrections.reserve(n);
                for c in 0..n {
                    let mechanical = Self::symm_grad(&grad_d.internal[c])
                        - self.eigenstrain.internal[c] * SymmTensor::IDENTITY;
                    let rate = if step > 0.0 {
                        equivalent_strain(mechanical - self.mech_strain_old[c]) / step
                    } else {
                        0.0
                    };
                    let inputs = RheologyInputs {
                        elastic: self.material,
                        mechanical_strain: mechanical,
                        material: self.material_states[c],
                        irradiation: self.irradiation[c],
                        dt: step,
                        equivalent_strain_rate: rate,
                    };
                    let corrected = rheology.correct(c, &inputs, &self.states[c])?;
                    // ε_in = ε_mech − ε_el is the *total* accumulated inelastic
                    // strain (history plus this step's increment), which is
                    // exactly the additional eigenstrain the momentum balance
                    // needs.
                    let updated = mechanical - corrected.elastic_strain;
                    let delta = (updated - self.inelastic[c]).mag();
                    if delta > inelastic_change {
                        inelastic_change = delta;
                    }
                    self.inelastic[c] = updated;
                    corrections.push(corrected);
                }
            }

            if final_change < self.corrector_tol && inelastic_change < self.inelastic_tol {
                break;
            }
        }

        if corrections.is_empty() {
            self.update_stress(mu, lambda, three_k);
        }

        let max_displacement = (0..n)
            .map(|c| self.disp.internal[c].mag())
            .fold(0.0_f64, f64::max);

        let report = MechanicsReport {
            iterations,
            final_change,
            converged: final_change < self.corrector_tol && inelastic_change < self.inelastic_tol,
            max_displacement,
        };
        Ok((report, corrections))
    }

    /// `∇·[2μ ε_in + λ tr(ε_in) I]` \[Pa/m\] — the divergence of the stress the
    /// accumulated inelastic strain would carry if it were elastic.
    ///
    /// Subtracting this from the momentum source is the finite-volume analogue
    /// of upstream's `correctAdditionalStrain`: it removes from the balance
    /// exactly the stress the material has *shed* by creeping or yielding, so
    /// the softer corrected stress is once again an equilibrium stress.
    ///
    /// Both inelastic increments are deviatoric under the von Mises flow rules
    /// used here, so `tr(ε_in)` is zero in practice; the trace term is kept so
    /// that a future volumetric inelastic mechanism cannot silently fall
    /// through.
    fn inelastic_load(&self, mu: f64, lambda: f64) -> VolVectorField {
        let n = self.mesh.n_cells;
        let mut field = VolSymmTensorField::new(
            "sigmaInelastic",
            self.mesh.clone(),
            Field::new(vec![SymmTensor::ZERO; n]),
            self.mesh
                .patches
                .iter()
                .map(|p| PatchField::zero_gradient_symm_tensor(p.size))
                .collect(),
        );
        for c in 0..n {
            let e = self.inelastic[c];
            field.internal[c] = 2.0 * mu * e + (lambda * e.tr()) * SymmTensor::IDENTITY;
        }
        fvc::div_symm_tensor(&field)
    }

    /// Symmetric part of the displacement gradient, `ε = ½(∇D + ∇Dᵀ)` \[-\].
    ///
    /// The small-strain (engineering) strain tensor. `grad_d` follows the
    /// OpenFOAM convention `(∇D)_ij = ∂D_j/∂x_i`, which the symmetrisation
    /// makes irrelevant.
    fn symm_grad(grad_d: &Tensor) -> SymmTensor {
        SymmTensor::new(
            grad_d.xx,
            0.5 * (grad_d.xy + grad_d.yx),
            0.5 * (grad_d.xz + grad_d.zx),
            grad_d.yy,
            0.5 * (grad_d.yz + grad_d.zy),
            grad_d.zz,
        )
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
mod rheology_tests;
#[cfg(test)]
mod tests;
