// SPDX-License-Identifier: AGPL-3.0-only
//! The mechanical problem: weak form, assembly on `farrer-park`, and the
//! implicit (backward-Euler) irradiation-creep step.
//!
//! # Governing equation
//!
//! Static equilibrium, Eq (2.6) of the BISON Theory Manual with no body
//! force, under spherical symmetry is
//!
//! ```text
//! dσ_r/dr + 2(σ_r − σ_θ)/r = 0
//! ```
//!
//! with the linear strains of Eqs (5.2)–(5.4), `ε_r = du/dr`,
//! `ε_θ = ε_φ = u/r`, and the additive split
//!
//! ```text
//! ε = ε_el + ε_th + ε_irr + ε_cr,   σ = D ε_el,
//! ε_th = α (T − T_ref) [1, 1],      ε_irr = DimensionalChange::strain_at(Φ)
//! ```
//!
//! # Weak form
//!
//! Multiplying by a virtual displacement `δu`, integrating over the volume
//! `dV = 4π r² dr` and integrating by parts:
//!
//! ```text
//! ∫ (σ_r δε_r + 2 σ_θ δε_θ) 4π r² dr = 4π a² p_in δu(a) − 4π b² p_out δu(b)
//! ```
//!
//! The factor 2 counts the two equal tangential components. In the reduced
//! notation this is `∫ δεᵀ W σ dV` with `W = diag(1, 2)`; `W D` is
//! symmetric, so the stiffness is too.
//!
//! # The creep step
//!
//! Over a step the fluence grows by `ΔΦ`. Backward Euler on
//! `ε̇_cr = K Φ̇ M σ` gives `ε_cr,n+1 = ε_cr,n + c M σ_n+1` with `c = K ΔΦ`, so
//!
//! ```text
//! σ_n+1 = D (ε_n+1 − ε*_n+1 − c M σ_n+1),   ε* = ε_th + ε_irr + ε_cr,n
//! σ_n+1 = D_alg (ε_n+1 − ε*_n+1),           D_alg = (I + c D M)⁻¹ D
//! ```
//!
//! Every term is linear in `σ`, so a step is exactly one linear solve of
//! `K_alg u = f_ext + ∫ Bᵀ W D_alg ε* dV`. `W D_alg` stays symmetric
//! (both `D⁻¹` and `M` are of the form `[[1, −2ν], [−ν, 1 − ν]]` times a
//! scalar, and that form times `W⁻¹` is symmetric), so conjugate gradients
//! applies.
//!
//! # Volumetric locking, and mean dilatation (the default)
//!
//! PyC creep has `ν_c = 0.5` (Eq (10.4)), so as creep strain accumulates
//! `D_alg` tends to an incompressible response. Two-node elements with full
//! integration cannot satisfy `ε_r + 2ε_θ = 0` at every Gauss point, and the
//! pressure part of the stress then oscillates inside each element. Measured
//! on 2026-11-28 (`tests::incompressible_creep_keeps_the_lame_stress`, a
//! 128-element shell after a creep step of `KΔΦ·E ≈ 120`): the stress
//! moved by **32 % of the applied pressure** at the outer Gauss points while
//! the midpoint moved by 2.3e-4 and the displacement stayed within 4.7e-5 of
//! the closed form. The cure is the mean-dilatation (B-bar) strain, the
//! same remedy as `farrer_park::assembly::Formulation::BBar`: at each Gauss
//! point the volumetric strain `θ = ε_r + 2ε_θ` is replaced by its element
//! volume average `θ̄`, i.e. `ε̄ = ε + (θ̄ − θ)/3 · [1, 1]`, which is linear
//! in `u` (`B̄ = B + [1, 1]ᵀ (b̄_vol − b_vol)/3`). [`Formulation::MeanDilatation`]
//! is the default; [`Formulation::FullIntegration`] is the explicit
//! ablation, through [`ParticleSolver::with_formulation`].
//!
//! # Discretisation
//!
//! Two-node linear elements, three-point Gauss–Legendre rule
//! (`farrer_park::quadrature::line_rule(3)`), the matrix in
//! `farrer_park::sparse::CsrMatrix` on a tridiagonal
//! `farrer_park::dof::SparsityPattern`, solved directly by its exact ILU(0)
//! (= LU) factorisation (`direct_solve`, which says why not CG). A solid
//! centre (`a = 0`) takes `u(0) = 0`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use farrer_park::dof::SparsityPattern;
use farrer_park::quadrature::line_rule;
use farrer_park::sparse::{CsrMatrix, CsrPreconditioner};
use uom::si::areal_number_density::per_square_meter;
use uom::si::f64::{ArealNumberDensity, Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use super::geometry::ParticleMesh;
use super::material::reduced_creep_compliance;
use super::ParticleMechanicsError;

/// Gauss points per element.
pub const GAUSS_POINTS_PER_ELEMENT: usize = 3;

/// The temperature field of a step.
#[derive(Debug, Clone, PartialEq)]
pub enum StepTemperature {
    /// One temperature everywhere, kelvin.
    Uniform(f64),
    /// One temperature per mesh node, kelvin (for example from
    /// [`super::thermal::solve_steady_conduction`]), interpolated linearly to
    /// the Gauss points.
    Nodal(Vec<f64>),
}

/// The state at the END of one load step.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadStep {
    /// Cumulative fast fluence at the end of the step, n/m². Must not
    /// decrease.
    pub fast_fluence_m2: f64,
    /// Pressure on the innermost surface, Pa (ignored for a solid centre).
    pub inner_pressure_pa: f64,
    /// Pressure on the outermost surface, Pa.
    pub outer_pressure_pa: f64,
    /// Temperature field.
    pub temperature: StepTemperature,
}

impl LoadStep {
    /// A step with a uniform temperature, from `uom` quantities.
    #[must_use]
    pub fn uniform(
        fast_fluence: ArealNumberDensity,
        inner_pressure: Pressure,
        outer_pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Self {
        Self {
            fast_fluence_m2: fast_fluence.get::<per_square_meter>(),
            inner_pressure_pa: inner_pressure.get::<pascal>(),
            outer_pressure_pa: outer_pressure.get::<pascal>(),
            temperature: StepTemperature::Uniform(temperature.get::<kelvin>()),
        }
    }
}

/// How the strain is computed from the displacement (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formulation {
    /// The mean-dilatation (B-bar) strain. **The default.**
    MeanDilatation,
    /// The plain strain `B u` at every Gauss point. Locks volumetrically as
    /// creep makes the response incompressible; kept as the ablation.
    FullIntegration,
}

/// What is stored at one Gauss point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussPointState {
    /// Element index.
    pub element: usize,
    /// Layer index.
    pub layer: usize,
    /// Radius, m.
    pub radius_m: f64,
    /// The volume this point stands for, `4π r² |J| w`, m³.
    pub volume_m3: f64,
    /// Total strain `[ε_r, ε_θ]`, dimensionless.
    pub total_strain: [f64; 2],
    /// Thermal plus irradiation strain at the end of the last step.
    pub eigenstrain: [f64; 2],
    /// Accumulated creep strain.
    pub creep_strain: [f64; 2],
    /// Stress `[σ_r, σ_θ]`, Pa.
    pub stress_pa: [f64; 2],
    /// Temperature at the end of the last step, K.
    pub temperature_k: f64,
}

/// Extremes of the stress in one layer over its Gauss points, Pa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerStressRange {
    /// Smallest radial stress, Pa.
    pub radial_min_pa: f64,
    /// Largest radial stress, Pa.
    pub radial_max_pa: f64,
    /// Smallest tangential stress, Pa.
    pub tangential_min_pa: f64,
    /// Largest tangential stress, Pa.
    pub tangential_max_pa: f64,
}

/// The particle's mechanical state and the solver that advances it.
#[derive(Debug, Clone)]
pub struct ParticleSolver {
    mesh: ParticleMesh,
    stress_free_temperature_k: f64,
    formulation: Formulation,
    pattern: SparsityPattern,
    fluence_m2: f64,
    displacement_m: Vec<f64>,
    points: Vec<GaussPointState>,
}

fn inverse2(a: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let det = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    [[a[1][1] / det, -a[0][1] / det], [-a[1][0] / det, a[0][0] / det]]
}

fn mul2(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let mut c = [[0.0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j];
        }
    }
    c
}

fn mulv2(a: [[f64; 2]; 2], v: [f64; 2]) -> [f64; 2] {
    [a[0][0] * v[0] + a[0][1] * v[1], a[1][0] * v[0] + a[1][1] * v[1]]
}

/// Solve `K x = b` for the tridiagonal `K` of a chain of two-node elements,
/// directly, with `farrer-park`'s ILU(0) factorisation.
///
/// On a tridiagonal pattern ILU(0) drops no fill, so it IS the exact LU
/// factorisation, and applying it is a direct solve (no pivoting is needed:
/// the assembled `K` is symmetric positive definite once the centre is
/// fixed or a shell is loaded). One step of iterative refinement follows.
///
/// **Why not `farrer_park::solver::solve_linear`.** Its CG gate is a
/// relative residual of 1e-12. A backward-stable direct solve leaves a
/// residual of order `ε_machine · κ(K)`, and `κ` grows with the element count
/// squared and, under creep with `ν_c = 0.5`, with the near-incompressibility
/// of `D_alg`. Measured on 2026-11-28: 2.1e-12 on a 64-element elastic SiC
/// shell, 2.6e-10 on a 128-element shell after a large creep step. The gate
/// then reported "not converged" for an exactly factorised system. A residual
/// tolerance is the wrong instrument for a direct solve, so none is applied;
/// the result is refused only if it is not finite. The accuracy that matters
/// is the displacement's, which the closed-form tests measure.
pub(crate) fn direct_solve(k: &CsrMatrix, b: &[f64]) -> Result<Vec<f64>, ParticleMechanicsError> {
    let lu = CsrPreconditioner::ilu0(k);
    let n = b.len();
    let mut x = vec![0.0; n];
    lu.apply_to(b, &mut x);
    let mut kx = vec![0.0; n];
    k.spmv(&x, &mut kx);
    let r: Vec<f64> = b.iter().zip(&kx).map(|(bi, ki)| bi - ki).collect();
    let mut dx = vec![0.0; n];
    lu.apply_to(&r, &mut dx);
    for i in 0..n {
        x[i] += dx[i];
    }
    if x.iter().all(|v| v.is_finite()) {
        Ok(x)
    } else {
        Err(ParticleMechanicsError::LinearSolve(String::from(
            "the direct tridiagonal solve produced a non-finite value (singular stiffness?)",
        )))
    }
}

/// The tridiagonal pattern of a chain of two-node elements.
fn chain_pattern(n_nodes: usize) -> SparsityPattern {
    let mut row_ptr = Vec::with_capacity(n_nodes + 1);
    let mut col_idx = Vec::new();
    row_ptr.push(0);
    for i in 0..n_nodes {
        if i > 0 {
            col_idx.push(i - 1);
        }
        col_idx.push(i);
        if i + 1 < n_nodes {
            col_idx.push(i + 1);
        }
        row_ptr.push(col_idx.len());
    }
    SparsityPattern { row_ptr, col_idx }
}

impl ParticleSolver {
    /// A stress-free particle at zero fluence, whose thermal strain is
    /// measured from `stress_free_temperature`.
    #[must_use]
    pub fn new(mesh: ParticleMesh, stress_free_temperature: ThermodynamicTemperature) -> Self {
        let n_nodes = mesh.n_nodes();
        let t_ref = stress_free_temperature.get::<kelvin>();
        let rule = line_rule(GAUSS_POINTS_PER_ELEMENT);
        let mut points = Vec::with_capacity(mesh.n_elements() * rule.len());
        for e in 0..mesh.n_elements() {
            let r1 = mesh.node_radius_m(e);
            let r2 = mesh.node_radius_m(e + 1);
            let jac = 0.5 * (r2 - r1);
            for q in &rule {
                let xi = q.xi[0];
                let r = r1 + 0.5 * (xi + 1.0) * (r2 - r1);
                points.push(GaussPointState {
                    element: e,
                    layer: mesh.element_layer(e),
                    radius_m: r,
                    volume_m3: 4.0 * core::f64::consts::PI * r * r * jac * q.weight,
                    total_strain: [0.0; 2],
                    eigenstrain: [0.0; 2],
                    creep_strain: [0.0; 2],
                    stress_pa: [0.0; 2],
                    temperature_k: t_ref,
                });
            }
        }
        Self {
            pattern: chain_pattern(n_nodes),
            formulation: Formulation::MeanDilatation,
            displacement_m: vec![0.0; n_nodes],
            mesh,
            stress_free_temperature_k: t_ref,
            fluence_m2: 0.0,
            points,
        }
    }

    /// With a strain formulation other than the default
    /// [`Formulation::MeanDilatation`]. Set it before the first step: the
    /// formulation of a history must not change part-way.
    #[must_use]
    pub fn with_formulation(mut self, formulation: Formulation) -> Self {
        self.formulation = formulation;
        self
    }

    /// The strain formulation in use.
    #[must_use]
    pub fn formulation(&self) -> Formulation {
        self.formulation
    }

    /// The strain-displacement matrices of element `e` at its Gauss points,
    /// rows `(r, θ)`, columns `(node e, node e+1)`, per the formulation.
    fn element_b(&self, e: usize) -> Vec<[[f64; 2]; 2]> {
        let ng = GAUSS_POINTS_PER_ELEMENT;
        let r1 = self.mesh.node_radius_m(e);
        let h = self.mesh.node_radius_m(e + 1) - r1;
        let rule = line_rule(ng);
        let mut bs = Vec::with_capacity(ng);
        for (iq, q) in rule.iter().enumerate() {
            let xi = q.xi[0];
            let r = self.points[e * ng + iq].radius_m;
            bs.push([[-1.0 / h, 1.0 / h], [0.5 * (1.0 - xi) / r, 0.5 * (1.0 + xi) / r]]);
        }
        match self.formulation {
            Formulation::FullIntegration => bs,
            Formulation::MeanDilatation => {
                let mut vol = 0.0;
                let mut bbar = [0.0; 2];
                for (iq, b) in bs.iter().enumerate() {
                    let dv = self.points[e * ng + iq].volume_m3;
                    vol += dv;
                    for a in 0..2 {
                        bbar[a] += (b[0][a] + 2.0 * b[1][a]) * dv;
                    }
                }
                bs.iter()
                    .map(|b| {
                        let mut out = *b;
                        for a in 0..2 {
                            let corr = (bbar[a] / vol - (b[0][a] + 2.0 * b[1][a])) / 3.0;
                            out[0][a] += corr;
                            out[1][a] += corr;
                        }
                        out
                    })
                    .collect()
            }
        }
    }

    /// The mesh.
    #[must_use]
    pub fn mesh(&self) -> &ParticleMesh {
        &self.mesh
    }

    /// Cumulative fast fluence reached, n/m².
    #[must_use]
    pub fn fluence_m2(&self) -> f64 {
        self.fluence_m2
    }

    /// Nodal radial displacements, m.
    #[must_use]
    pub fn displacement_m(&self) -> &[f64] {
        &self.displacement_m
    }

    /// Every Gauss point's state, element by element, inside out.
    #[must_use]
    pub fn points(&self) -> &[GaussPointState] {
        &self.points
    }

    /// Stress extremes in layer `layer`.
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::NoSuchLayer`] for an index past the last
    /// layer.
    pub fn layer_stress_range(&self, layer: usize) -> Result<LayerStressRange, ParticleMechanicsError> {
        if layer >= self.mesh.layers().len() {
            return Err(ParticleMechanicsError::NoSuchLayer(layer));
        }
        let mut out = LayerStressRange {
            radial_min_pa: f64::INFINITY,
            radial_max_pa: f64::NEG_INFINITY,
            tangential_min_pa: f64::INFINITY,
            tangential_max_pa: f64::NEG_INFINITY,
        };
        for p in self.points.iter().filter(|p| p.layer == layer) {
            out.radial_min_pa = out.radial_min_pa.min(p.stress_pa[0]);
            out.radial_max_pa = out.radial_max_pa.max(p.stress_pa[0]);
            out.tangential_min_pa = out.tangential_min_pa.min(p.stress_pa[1]);
            out.tangential_max_pa = out.tangential_max_pa.max(p.stress_pa[1]);
        }
        Ok(out)
    }

    /// Advance to the end of `step`: one backward-Euler creep step from the
    /// current fluence to `step.fast_fluence_m2`. A step with no fluence
    /// increment is an elastic re-solve (new pressure or temperature, no
    /// creep).
    ///
    /// # Errors
    ///
    /// [`ParticleMechanicsError::InvalidLoad`] if the fluence decreases or any
    /// input is non-finite, a temperature is non-positive, or a nodal field
    /// has the wrong length; [`ParticleMechanicsError::LinearSolve`] if the
    /// Krylov solve does not converge (the state is then left unchanged).
    pub fn step(&mut self, step: &LoadStep) -> Result<(), ParticleMechanicsError> {
        let phi = step.fast_fluence_m2;
        if !phi.is_finite() || phi < self.fluence_m2 {
            return Err(ParticleMechanicsError::InvalidLoad(format!(
                "fluence must be finite and not decrease: {} -> {} n/m^2",
                self.fluence_m2, phi
            )));
        }
        if !(step.inner_pressure_pa.is_finite() && step.outer_pressure_pa.is_finite()) {
            return Err(ParticleMechanicsError::InvalidLoad(String::from(
                "pressures must be finite",
            )));
        }
        match &step.temperature {
            StepTemperature::Uniform(t) => {
                if !(t.is_finite() && *t > 0.0) {
                    return Err(ParticleMechanicsError::InvalidLoad(format!(
                        "temperature must be positive kelvin, got {t}"
                    )));
                }
            }
            StepTemperature::Nodal(ts) => {
                if ts.len() != self.mesh.n_nodes() || ts.iter().any(|t| !(t.is_finite() && *t > 0.0)) {
                    return Err(ParticleMechanicsError::InvalidLoad(format!(
                        "nodal temperature needs {} positive values",
                        self.mesh.n_nodes()
                    )));
                }
            }
        }
        let d_phi = phi - self.fluence_m2;
        let n = self.mesh.n_nodes();
        let mut k = CsrMatrix::from_pattern(&self.pattern);
        let mut f = vec![0.0; n];
        let rule = line_rule(GAUSS_POINTS_PER_ELEMENT);
        let ng = rule.len();
        // Per-point algorithmic matrix and eigenstrain, kept for the update.
        let mut d_alg_all = Vec::with_capacity(self.points.len());
        let mut eps_star_all = Vec::with_capacity(self.points.len());
        let mut new_eig_all = Vec::with_capacity(self.points.len());
        let mut temp_all = Vec::with_capacity(self.points.len());
        for e in 0..self.mesh.n_elements() {
            let layer = self.mesh.element_layer(e);
            let mat = self.mesh.layers()[layer].material;
            let d = mat.reduced_elasticity();
            let bmats = self.element_b(e);
            let mut ke = [[0.0; 2]; 2];
            let mut fe = [0.0; 2];
            for (iq, q) in rule.iter().enumerate() {
                let ip = e * ng + iq;
                let xi = q.xi[0];
                let n1 = 0.5 * (1.0 - xi);
                let n2 = 0.5 * (1.0 + xi);
                let p = self.points[ip];
                let t = match &step.temperature {
                    StepTemperature::Uniform(t) => *t,
                    StepTemperature::Nodal(ts) => n1 * ts[e] + n2 * ts[e + 1],
                };
                let (kc, nu_eff) = mat.creep.coefficient_and_ratio(t);
                let c = kc * d_phi;
                let m = reduced_creep_compliance(nu_eff);
                let dm = mul2(d, m);
                let a = [[1.0 + c * dm[0][0], c * dm[0][1]], [c * dm[1][0], 1.0 + c * dm[1][1]]];
                let d_alg = mul2(inverse2(a), d);
                let eth = mat.thermal_expansion_per_k * (t - self.stress_free_temperature_k);
                let eirr = mat.dimensional_change.strain_at(phi);
                let eig = [eth + eirr[0], eth + eirr[1]];
                let eps_star = [eig[0] + p.creep_strain[0], eig[1] + p.creep_strain[1]];
                let b = bmats[iq];
                // S = W D_alg, W = diag(1, 2).
                let s = [[d_alg[0][0], d_alg[0][1]], [2.0 * d_alg[1][0], 2.0 * d_alg[1][1]]];
                let s_eps = mulv2(s, eps_star);
                for aa in 0..2 {
                    for bb in 0..2 {
                        let mut v = 0.0;
                        for i in 0..2 {
                            for j in 0..2 {
                                v += b[i][aa] * s[i][j] * b[j][bb];
                            }
                        }
                        ke[aa][bb] += v * p.volume_m3;
                    }
                    fe[aa] += (b[0][aa] * s_eps[0] + b[1][aa] * s_eps[1]) * p.volume_m3;
                }
                d_alg_all.push(d_alg);
                eps_star_all.push(eps_star);
                new_eig_all.push(eig);
                temp_all.push(t);
            }
            let dofs = [e, e + 1];
            match k.scatter(&dofs, &[ke[0][0], ke[0][1], ke[1][0], ke[1][1]]) {
                Ok(()) => {}
                Err(err) => return Err(ParticleMechanicsError::LinearSolve(format!("{err}"))),
            }
            f[e] += fe[0];
            f[e + 1] += fe[1];
        }
        let a = self.mesh.inner_radius_m();
        let b = self.mesh.outer_radius_m();
        let four_pi = 4.0 * core::f64::consts::PI;
        if a > 0.0 {
            f[0] += four_pi * a * a * step.inner_pressure_pa;
        } else {
            // Symmetry at the centre: u(0) = 0 by row-and-column elimination
            // (the prescribed value is zero, so no right-hand-side change).
            let diag = k.max_abs_diagonal();
            let _ = k.set(0, 0, diag);
            if n > 1 {
                let _ = k.set(0, 1, 0.0);
                let _ = k.set(1, 0, 0.0);
            }
            f[0] = 0.0;
        }
        f[n - 1] -= four_pi * b * b * step.outer_pressure_pa;
        let u = if f.iter().all(|v| *v == 0.0) {
            vec![0.0; n]
        } else {
            match direct_solve(&k, &f) {
                Ok(u) => u,
                Err(err) => return Err(err),
            }
        };
        for e in 0..self.mesh.n_elements() {
            let layer = self.mesh.element_layer(e);
            let mat = self.mesh.layers()[layer].material;
            let bmats = self.element_b(e);
            for iq in 0..ng {
                let ip = e * ng + iq;
                let b = bmats[iq];
                let eps = [b[0][0] * u[e] + b[0][1] * u[e + 1], b[1][0] * u[e] + b[1][1] * u[e + 1]];
                let es = eps_star_all[ip];
                let sigma = mulv2(d_alg_all[ip], [eps[0] - es[0], eps[1] - es[1]]);
                let (kc, nu_eff) = mat.creep.coefficient_and_ratio(temp_all[ip]);
                let c = kc * d_phi;
                let dcr = mulv2(reduced_creep_compliance(nu_eff), sigma);
                let pt = &mut self.points[ip];
                pt.total_strain = eps;
                pt.eigenstrain = new_eig_all[ip];
                pt.creep_strain = [pt.creep_strain[0] + c * dcr[0], pt.creep_strain[1] + c * dcr[1]];
                pt.stress_pa = sigma;
                pt.temperature_k = temp_all[ip];
            }
        }
        self.displacement_m = u;
        self.fluence_m2 = phi;
        Ok(())
    }
}
