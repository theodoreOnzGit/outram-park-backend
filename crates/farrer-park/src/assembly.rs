// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Weak-form assembly: the element loop, the quadrature loop, the
//! strain-displacement operator, and the internal force and tangent they
//! produce.
//!
//! # What belongs in this module
//!
//! The discrete momentum balance
//!
//! `integral over V of B^T sigma dV = integral over V of N^T b dV + boundary tractions`
//!
//! assembled as a residual and a tangent: [`System`] owns the mesh, the
//! material and the per-quadrature-point state, and
//! [`System::assemble`] produces the internal force vector and the stiffness
//! matrix at a given displacement.
//!
//! # What does NOT belong here
//!
//! Boundary conditions ([`crate::bc`]), the Newton loop and load stepping
//! ([`crate::solver`]), and the constitutive integration itself
//! ([`crate::material`]). This module supplies strains to the material and
//! scatters what comes back.
//!
//! # Small strain, total Lagrangian on the reference configuration
//!
//! Everything is linearised about the undeformed geometry:
//! `eps = (grad u + grad u^T) / 2`, integration on the reference volume, no
//! geometric stiffness. That is the correct model up to strains of a percent or
//! so and rotations of a few degrees, and outside that it is simply wrong — not
//! inaccurate, wrong, because a rigid rotation would generate stress. Finite
//! strain is deliberately out of scope for this pass.
//!
//! # Two-dimensional analysis is **plane strain**
//!
//! A 2-D mesh is solved in plane strain: `eps_zz = eps_yz = eps_xz = 0`, so
//! `sigma_zz = nu (sigma_xx + sigma_yy)` falls out of the same
//! three-dimensional constitutive law with no special case. The
//! strain-displacement operator simply writes zeros into those three Voigt
//! slots.
//!
//! **Plane stress is not implemented.** It is not a matter of zeroing different
//! slots: it needs `sigma_zz = 0` enforced by condensing `eps_zz` out of the
//! constitutive law, which for J2 plasticity means a nested local solve. Doing
//! it badly would be worse than not doing it, so it is absent and says so.
//!
//! # Units
//!
//! Displacements in metres, strains dimensionless, stresses in pascals, the
//! internal force vector in newtons, the stiffness matrix in newton per metre,
//! body force in newton per cubic metre.

use std::sync::Arc;

use crate::dof::{DofMap, SparsityPattern};
use crate::element::{map_gradients, map_point, ElementType, MAX_ELEM_NODES};
use crate::error::{FemError, Result};
use crate::material::{Material, MaterialState, StressUpdate};
use crate::mesh::{ElemId, Mesh};
use crate::quadrature::{default_rule, error_rule, QuadraturePoint};
use crate::sparse::CsrMatrix;
use crate::tensor::{Tensor4, Voigt6};

/// Maximum element degrees of freedom: eight nodes times three components.
pub const MAX_ELEM_DOFS: usize = MAX_ELEM_NODES * 3;

/// The strain-displacement operator `B` of one element at one quadrature point.
///
/// `eps_V = B u_e`, with `u_e` the element nodal displacements in metres, in
/// node-major order (all components of node 0, then node 1, ...), and `eps_V`
/// the six-component engineering-shear strain of [`crate::tensor`].
///
/// # Units
///
/// Entries are in reciprocal metres.
///
/// # Layout
///
/// Row-major `6 x n_dofs`, with `n_dofs = n_nodes * dim`. Rows 2, 3 and 4
/// (`zz`, `yz`, `xz`) are identically zero for a 2-D element — that is the
/// plane-strain constraint, applied in the one place it belongs.
#[derive(Debug, Clone, Copy)]
pub struct BMatrix {
    /// `6 x MAX_ELEM_DOFS` row-major, reciprocal metres. Columns beyond
    /// `n_dofs` are zero.
    pub b: [[f64; MAX_ELEM_DOFS]; 6],
    /// Number of meaningful columns: `n_nodes * dim`.
    pub n_dofs: usize,
}

impl BMatrix {
    /// Build `B` from the physical shape-function gradients `dN_a/dx_i`
    /// \[1/m\].
    ///
    /// # Arguments
    ///
    /// - `grad` — `[a][i] = dN_a/dx_i`, as returned by
    ///   [`crate::element::map_gradients`].
    /// - `n_nodes` — nodes on the element.
    /// - `dim` — 2 (plane strain) or 3.
    #[must_use]
    pub fn from_gradients(grad: &[[f64; 3]; MAX_ELEM_NODES], n_nodes: usize, dim: usize) -> Self {
        let mut b = [[0.0_f64; MAX_ELEM_DOFS]; 6];
        for a in 0..n_nodes {
            let (gx, gy, gz) = (grad[a][0], grad[a][1], grad[a][2]);
            let base = a * dim;
            if dim == 2 {
                // eps_xx, eps_yy, gamma_xy; zz / yz / xz remain zero.
                b[0][base] = gx;
                b[1][base + 1] = gy;
                b[5][base] = gy;
                b[5][base + 1] = gx;
            } else {
                b[0][base] = gx;
                b[1][base + 1] = gy;
                b[2][base + 2] = gz;
                b[3][base + 1] = gz; // gamma_yz = du_y/dz + du_z/dy
                b[3][base + 2] = gy;
                b[4][base] = gz; // gamma_xz = du_x/dz + du_z/dx
                b[4][base + 2] = gx;
                b[5][base] = gy; // gamma_xy = du_x/dy + du_y/dx
                b[5][base + 1] = gx;
            }
        }
        BMatrix {
            b,
            n_dofs: n_nodes * dim,
        }
    }

    /// Strain at this point from the element nodal displacements \[m\]:
    /// `eps = B u_e` \[-\].
    #[must_use]
    pub fn strain(&self, ue: &[f64]) -> Voigt6 {
        let mut e = [0.0_f64; 6];
        for (i, ei) in e.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in 0..self.n_dofs {
                s += self.b[i][k] * ue[k];
            }
            *ei = s;
        }
        Voigt6(e)
    }

    /// Accumulate `w B^T sigma` into the element internal force `fe` \[N\].
    ///
    /// `w` is the physical integration weight `weight * det J` in cubic metres
    /// (2-D: square metres times unit thickness).
    pub fn accumulate_internal_force(&self, sigma: &Voigt6, w: f64, fe: &mut [f64]) {
        for k in 0..self.n_dofs {
            let mut s = 0.0;
            for i in 0..6 {
                s += self.b[i][k] * sigma.0[i];
            }
            fe[k] += w * s;
        }
    }

    /// Accumulate `w B^T D B` into the element stiffness `ke` \[N/m\],
    /// row-major of side `n_dofs`.
    pub fn accumulate_stiffness(&self, d: &Tensor4, w: f64, ke: &mut [f64]) {
        let n = self.n_dofs;
        // db[i][k] = sum_j D[i][j] B[j][k]
        let mut db = [[0.0_f64; MAX_ELEM_DOFS]; 6];
        for i in 0..6 {
            for k in 0..n {
                let mut s = 0.0;
                for j in 0..6 {
                    s += d.0[i][j] * self.b[j][k];
                }
                db[i][k] = s;
            }
        }
        for a in 0..n {
            for bcol in 0..n {
                let mut s = 0.0;
                for i in 0..6 {
                    s += self.b[i][a] * db[i][bcol];
                }
                ke[a * n + bcol] += w * s;
            }
        }
    }
}

/// A body force field, dispatched by enum rather than a boxed closure.
///
/// # Units
///
/// Newton per cubic metre (2-D: newton per square metre through unit
/// thickness).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyForce {
    /// No body force.
    None,
    /// A spatially uniform force per unit volume `[b_x, b_y, b_z]`
    /// \[N/m^3\] — gravity, for instance.
    Uniform([f64; 3]),
    /// The manufactured body force of the verification study,
    ///
    /// `f_x = f_y = pi^2 [(lambda + 3 mu) sin(pi x) sin(pi y)
    ///                    - (lambda + mu) cos(pi x) cos(pi y)]`,
    ///
    /// which is `-div sigma` for the exact plane-strain displacement field
    /// `u_x = u_y = sin(pi x) sin(pi y)` on the unit square. It lives here
    /// rather than in the test file because [`BodyForce`] is an enum and a
    /// closure variant would be a trait object.
    ///
    /// Carries the Lame constants `(lambda, mu)` in pascals.
    ManufacturedSine(f64, f64),
}

impl BodyForce {
    /// Evaluate the body force at a physical point `x` \[m\], returning
    /// \[N/m^3\].
    #[must_use]
    pub fn at(&self, x: [f64; 3]) -> [f64; 3] {
        match self {
            BodyForce::None => [0.0; 3],
            BodyForce::Uniform(b) => *b,
            BodyForce::ManufacturedSine(lambda, mu) => {
                let pi = std::f64::consts::PI;
                let a = (pi * x[0]).sin() * (pi * x[1]).sin();
                let c = (pi * x[0]).cos() * (pi * x[1]).cos();
                let f = pi * pi * ((lambda + 3.0 * mu) * a - (lambda + mu) * c);
                [f, f, 0.0]
            }
        }
    }

    /// The exact displacement field \[m\] this body force was manufactured
    /// from, and its gradient `du_i/dx_j` \[-\].
    ///
    /// Returns `None` for the variants that are not manufactured solutions.
    /// Used by the convergence study to form the `L2` and `H1` errors.
    #[must_use]
    pub fn manufactured_solution(&self, x: [f64; 3]) -> Option<([f64; 3], [[f64; 3]; 3])> {
        match self {
            BodyForce::ManufacturedSine(_, _) => {
                let pi = std::f64::consts::PI;
                let u = (pi * x[0]).sin() * (pi * x[1]).sin();
                let dudx = pi * (pi * x[0]).cos() * (pi * x[1]).sin();
                let dudy = pi * (pi * x[0]).sin() * (pi * x[1]).cos();
                let mut g = [[0.0; 3]; 3];
                g[0][0] = dudx;
                g[0][1] = dudy;
                g[1][0] = dudx;
                g[1][1] = dudy;
                Some(([u, u, 0.0], g))
            }
            _ => None,
        }
    }
}

/// The assembled finite-element problem: mesh, degrees of freedom, material,
/// quadrature-point history, and the matrix they fill.
///
/// The mesh is shared with `Arc<Mesh>` and never mutated, per the workspace
/// rule for read-only data. There are no lifetime parameters: everything else
/// is owned by value.
#[derive(Debug, Clone)]
pub struct System {
    mesh: Arc<Mesh>,
    dofs: DofMap,
    material: Material,
    body_force: BodyForce,
    rule: Vec<QuadraturePoint>,
    /// Committed history at every quadrature point, indexed
    /// `element * n_quadrature + point`.
    state: Vec<MaterialState>,
    /// Stress at every quadrature point at the last assembled displacement
    /// \[Pa\], same indexing. Post-processing reads it; assembly rewrites it.
    stress: Vec<Voigt6>,
    /// Trial history from the last assembly, not yet committed.
    trial_state: Vec<MaterialState>,
    pattern: SparsityPattern,
}

/// What one assembly produced.
#[derive(Debug, Clone)]
pub struct Assembled {
    /// Internal force vector `f_int` \[N\], length `n_dofs`.
    pub internal_force: Vec<f64>,
    /// External force from the body force \[N\], length `n_dofs`. Tractions are
    /// added by [`crate::bc`].
    pub body_force: Vec<f64>,
    /// How many quadrature points flowed plastically in this assembly.
    pub n_yielding: usize,
}

impl System {
    /// Build a system on a mesh with one material and one body force.
    ///
    /// Uses [`crate::quadrature::default_rule`] for the element type; the
    /// per-point history is initialised pristine.
    #[must_use]
    pub fn new(mesh: Arc<Mesh>, material: Material, body_force: BodyForce) -> Self {
        let dofs = DofMap::displacement(&mesh);
        let rule = default_rule(mesh.element_type());
        let n_pts = mesh.n_elements() * rule.len();
        let pattern = SparsityPattern::from_mesh(&mesh, &dofs);
        Self {
            mesh,
            dofs,
            material,
            body_force,
            rule,
            state: vec![MaterialState::pristine(); n_pts],
            stress: vec![Voigt6::ZERO; n_pts],
            trial_state: vec![MaterialState::pristine(); n_pts],
            pattern,
        }
    }

    /// The mesh, shared.
    #[must_use]
    pub fn mesh(&self) -> Arc<Mesh> {
        Arc::clone(&self.mesh)
    }

    /// The degree-of-freedom map.
    #[must_use]
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }

    /// The material law.
    #[must_use]
    pub fn material(&self) -> Material {
        self.material
    }

    /// Total number of degrees of freedom (dimensionless count).
    #[must_use]
    pub fn n_dofs(&self) -> usize {
        self.dofs.n_dofs()
    }

    /// Quadrature points per element (dimensionless count).
    #[must_use]
    pub fn n_quadrature_points(&self) -> usize {
        self.rule.len()
    }

    /// Stress at every quadrature point from the last assembly \[Pa\], indexed
    /// `element * n_quadrature_points() + point`.
    #[must_use]
    pub fn quadrature_stress(&self) -> &[Voigt6] {
        &self.stress
    }

    /// Committed history at every quadrature point.
    #[must_use]
    pub fn quadrature_state(&self) -> &[MaterialState] {
        &self.state
    }

    /// Physical coordinates \[m\] of every quadrature point, in the same order
    /// as [`quadrature_stress`](Self::quadrature_stress).
    #[must_use]
    pub fn quadrature_coordinates(&self) -> Vec<[f64; 3]> {
        let et = self.mesh.element_type();
        let mut ec = [[0.0; 3]; MAX_ELEM_NODES];
        let mut out = Vec::with_capacity(self.stress.len());
        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            for q in &self.rule {
                out.push(map_point(et, &ec, q.xi));
            }
        }
        out
    }

    /// A fresh matrix on this system's sparsity pattern.
    #[must_use]
    pub fn new_matrix(&self) -> CsrMatrix {
        CsrMatrix::from_pattern(&self.pattern)
    }

    /// Commit the trial history produced by the last [`assemble`](Self::assemble)
    /// call.
    ///
    /// Call this **once** per converged load step, never inside the Newton
    /// loop: committing mid-iteration would let a rejected iterate leave
    /// permanent plastic strain behind.
    pub fn commit(&mut self) {
        self.state.copy_from_slice(&self.trial_state);
    }

    /// Assemble the internal force, the body force and (optionally) the
    /// tangent stiffness at displacement `u`.
    ///
    /// # Arguments
    ///
    /// - `u` — nodal displacements \[m\], length [`n_dofs`](Self::n_dofs).
    /// - `matrix` — `Some(k)` to assemble the tangent into `k` (which is zeroed
    ///   first); `None` to compute forces only, which is what a residual-only
    ///   evaluation inside a line search would want.
    ///
    /// # Returns
    ///
    /// The internal and body force vectors \[N\], and how many quadrature
    /// points yielded.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] on a wrong-length `u`,
    /// [`FemError::DegenerateElement`] (with the real element and point index)
    /// for a non-positive Jacobian, and whatever the constitutive update
    /// returns.
    pub fn assemble(&mut self, u: &[f64], matrix: Option<&mut CsrMatrix>) -> Result<Assembled> {
        if u.len() != self.n_dofs() {
            return Err(FemError::LengthMismatch {
                context: "System::assemble: displacement vector",
                expected: self.n_dofs(),
                actual: u.len(),
            });
        }
        let et = self.mesh.element_type();
        let nn = et.n_nodes();
        let dim = et.dim();
        let ned = nn * dim;
        let nq = self.rule.len();

        let mut f_int = vec![0.0; self.n_dofs()];
        let mut f_body = vec![0.0; self.n_dofs()];
        let mut n_yield = 0usize;

        let mut ec = [[0.0_f64; 3]; MAX_ELEM_NODES];
        let mut ed = vec![0usize; ned];
        let mut ue = vec![0.0_f64; ned];
        let mut fe = vec![0.0_f64; ned];
        let mut fb = vec![0.0_f64; ned];
        let mut ke = vec![0.0_f64; ned * ned];

        let mut mat = matrix;
        if let Some(k) = mat.as_deref_mut() {
            k.zero();
        }

        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.mesh, ElemId(e), &mut ed);
            for (i, &g) in ed.iter().enumerate() {
                ue[i] = u[g];
            }
            fe.iter_mut().for_each(|v| *v = 0.0);
            fb.iter_mut().for_each(|v| *v = 0.0);
            ke.iter_mut().for_each(|v| *v = 0.0);

            for (qi, q) in self.rule.iter().enumerate() {
                let mp = map_gradients(et, &ec, q.xi).map_err(|err| match err {
                    FemError::DegenerateElement { det_j, .. } => FemError::DegenerateElement {
                        element: e,
                        point: qi,
                        det_j,
                    },
                    other => other,
                })?;
                let w = q.weight * mp.det_j;
                let bm = BMatrix::from_gradients(&mp.grad, nn, dim);
                let eps = bm.strain(&ue);

                let gp = e * nq + qi;
                let up: StressUpdate = self.material.update(eps, &self.state[gp])?;
                if up.yielding {
                    n_yield += 1;
                }
                self.trial_state[gp] = up.state;
                self.stress[gp] = up.stress;

                bm.accumulate_internal_force(&up.stress, w, &mut fe);
                if mat.is_some() {
                    bm.accumulate_stiffness(&up.tangent, w, &mut ke);
                }

                if !matches!(self.body_force, BodyForce::None) {
                    let x = map_point(et, &ec, q.xi);
                    let bvec = self.body_force.at(x);
                    let n = et.shape_functions(q.xi);
                    for a in 0..nn {
                        for c in 0..dim {
                            fb[a * dim + c] += w * n[a] * bvec[c];
                        }
                    }
                }
            }

            for (i, &g) in ed.iter().enumerate() {
                f_int[g] += fe[i];
                f_body[g] += fb[i];
            }
            if let Some(k) = mat.as_deref_mut() {
                k.scatter(&ed, &ke)?;
            }
        }

        Ok(Assembled {
            internal_force: f_int,
            body_force: f_body,
            n_yielding: n_yield,
        })
    }

    /// `L2` and `H1`-seminorm errors of `u` against the manufactured solution
    /// carried by this system's [`BodyForce`].
    ///
    /// Integrated with [`crate::quadrature::error_rule`], deliberately richer
    /// than the assembly rule: the integrand `(u_h - u)^2` is not a polynomial,
    /// and using the assembly rule would measure the quadrature error of the
    /// norm rather than the discretisation error of the solution.
    ///
    /// # Returns
    ///
    /// `(l2, h1_seminorm)` where
    /// `l2 = sqrt(integral |u_h - u|^2 dV)` \[m * m^(dim/2)\] and
    /// `h1 = sqrt(integral |grad u_h - grad u|^2 dV)` \[m^(dim/2)\].
    /// Both are absolute, not relative; the convergence study takes ratios, for
    /// which the normalisation cancels.
    ///
    /// # Errors
    ///
    /// [`FemError::BoundaryCondition`] if this system has no manufactured
    /// solution, and [`FemError::DegenerateElement`] for a bad element.
    pub fn manufactured_errors(&self, u: &[f64]) -> Result<(f64, f64)> {
        if self
            .body_force
            .manufactured_solution([0.1, 0.2, 0.0])
            .is_none()
        {
            return Err(FemError::BoundaryCondition(
                "manufactured_errors: this system has no manufactured solution".into(),
            ));
        }
        let et = self.mesh.element_type();
        let (nn, dim) = (et.n_nodes(), et.dim());
        let rule = error_rule(et);
        let mut ec = [[0.0_f64; 3]; MAX_ELEM_NODES];
        let mut ed = vec![0usize; nn * dim];
        let mut l2 = 0.0;
        let mut h1 = 0.0;

        for e in 0..self.mesh.n_elements() {
            self.mesh.element_coords(ElemId(e), &mut ec);
            self.dofs.element_dofs(&self.mesh, ElemId(e), &mut ed);
            for q in &rule {
                let mp = map_gradients(et, &ec, q.xi)?;
                let w = q.weight * mp.det_j;
                let x = map_point(et, &ec, q.xi);
                let (u_ex, g_ex) = self.body_force.manufactured_solution(x).unwrap();
                let n = et.shape_functions(q.xi);

                let mut uh = [0.0_f64; 3];
                let mut gh = [[0.0_f64; 3]; 3];
                for a in 0..nn {
                    for c in 0..dim {
                        let v = u[ed[a * dim + c]];
                        uh[c] += n[a] * v;
                        for j in 0..dim {
                            gh[c][j] += mp.grad[a][j] * v;
                        }
                    }
                }
                for c in 0..dim {
                    l2 += w * (uh[c] - u_ex[c]).powi(2);
                    for j in 0..dim {
                        h1 += w * (gh[c][j] - g_ex[c][j]).powi(2);
                    }
                }
            }
        }
        Ok((l2.sqrt(), h1.sqrt()))
    }
}

/// Interpolate a nodal field onto a physical point of one element, for
/// post-processing.
///
/// # Arguments
///
/// - `mesh` — the mesh.
/// - `e` — the element.
/// - `xi` — natural coordinate, dimensionless.
/// - `u` — the global nodal field \[m\] with `dim` components per node.
///
/// # Returns
///
/// The interpolated value \[m\], with entries beyond `dim` zero.
#[must_use]
pub fn interpolate(mesh: &Mesh, e: ElemId, xi: [f64; 3], u: &[f64]) -> [f64; 3] {
    let et: ElementType = mesh.element_type();
    let dim = et.dim();
    let n = et.shape_functions(xi);
    let mut out = [0.0; 3];
    for (a, &node) in mesh.element_nodes(e).iter().enumerate() {
        for c in 0..dim {
            out[c] += n[a] * u[node * dim + c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{unit_cube_hex8, unit_square_quad4};

    /// A rigid-body translation must produce exactly zero strain, zero stress
    /// and zero internal force. The cheapest possible sanity check on `B`.
    #[test]
    fn rigid_translation_produces_no_force() {
        let mesh = unit_square_quad4(3).unwrap().shared();
        let mat = Material::elastic(200.0e9, 0.3).unwrap();
        let mut sys = System::new(mesh, mat, BodyForce::None);
        let n = sys.n_dofs();
        let mut u = vec![0.0; n];
        for i in (0..n).step_by(2) {
            u[i] = 1.0e-3; // uniform x translation
            u[i + 1] = -2.0e-3;
        }
        let a = sys.assemble(&u, None).unwrap();
        let maxf = a.internal_force.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        assert!(maxf < 1e-6, "rigid translation gave force {} N", maxf);
    }

    /// The elastic stiffness matrix must be symmetric and positive
    /// semi-definite, with exactly `dim (dim + 1) / 2` rigid-body modes in its
    /// null space. Checked here through the translation modes.
    #[test]
    fn stiffness_is_symmetric_and_annihilates_translations() {
        for mesh in [
            unit_square_quad4(2).unwrap().shared(),
            unit_cube_hex8(2).unwrap().shared(),
        ] {
            let dim = mesh.dim();
            let mat = Material::elastic(70.0e9, 0.33).unwrap();
            let mut sys = System::new(mesh, mat, BodyForce::None);
            let mut k = sys.new_matrix();
            let u = vec![0.0; sys.n_dofs()];
            sys.assemble(&u, Some(&mut k)).unwrap();
            let asym = k.max_asymmetry() / k.max_abs_diagonal();
            assert!(asym < 1e-12, "asymmetry {}", asym);

            for c in 0..dim {
                let mut t = vec![0.0; sys.n_dofs()];
                for i in (c..sys.n_dofs()).step_by(dim) {
                    t[i] = 1.0;
                }
                let mut y = vec![0.0; sys.n_dofs()];
                k.spmv(&t, &mut y);
                let m = y.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
                assert!(
                    m / k.max_abs_diagonal() < 1e-12,
                    "translation mode {} not in the null space: {}",
                    c,
                    m
                );
            }
        }
    }

    /// The tangent must be the exact derivative of the internal force, checked
    /// by central differences on the global residual. This is the assembly-level
    /// counterpart of the material-level tangent check, and it catches a wrong
    /// `B^T D B` ordering that the material test cannot see.
    #[test]
    fn global_tangent_matches_finite_difference_of_internal_force() {
        let mesh = unit_square_quad4(2).unwrap().shared();
        let mat = Material::j2_linear_hardening(200.0e9, 0.3, 250.0e6, 2.0e9).unwrap();
        let mut sys = System::new(mesh, mat, BodyForce::None);
        let n = sys.n_dofs();
        // A displacement large enough that several points are plastic.
        let mut u = vec![0.0; n];
        for i in 0..n {
            u[i] = 3.0e-3 * (((i * 37) % 11) as f64 / 11.0 - 0.4);
        }
        let mut k = sys.new_matrix();
        let a0 = sys.assemble(&u, Some(&mut k)).unwrap();
        assert!(a0.n_yielding > 0, "the check point must be plastic");

        let h = 1e-9;
        let scale = k.max_abs_diagonal();
        for j in [0usize, 3, 7, n - 2] {
            let mut up = u.clone();
            let mut um = u.clone();
            up[j] += h;
            um[j] -= h;
            let fp = sys.assemble(&up, None).unwrap().internal_force;
            let fm = sys.assemble(&um, None).unwrap().internal_force;
            for i in 0..n {
                let fd = (fp[i] - fm[i]) / (2.0 * h);
                assert!(
                    (fd - k.get(i, j)).abs() / scale < 1e-5,
                    "K[{}][{}] = {} but fd = {}",
                    i,
                    j,
                    k.get(i, j),
                    fd
                );
            }
        }
    }

    /// The body-force integral of a uniform load must equal load times volume,
    /// component by component — a check on the `N^T b` quadrature.
    #[test]
    fn uniform_body_force_integrates_to_load_times_volume() {
        let mesh = unit_cube_hex8(2).unwrap().shared();
        let mat = Material::elastic(1.0e9, 0.25).unwrap();
        let b = [3.0, -5.0, 7.0];
        let mut sys = System::new(mesh, mat, BodyForce::Uniform(b));
        let u = vec![0.0; sys.n_dofs()];
        let a = sys.assemble(&u, None).unwrap();
        for c in 0..3 {
            let total: f64 = a.body_force.iter().skip(c).step_by(3).sum();
            assert!((total - b[c]).abs() < 1e-9, "component {}: {}", c, total);
        }
    }
}
