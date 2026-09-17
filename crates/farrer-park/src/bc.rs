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

//! Boundary conditions: prescribed displacement (Dirichlet) and prescribed
//! traction (Neumann).
//!
//! # What belongs in this module
//!
//! [`DirichletSet`] — a list of constrained degrees of freedom and their
//! values, applied by either **strong elimination** or a **penalty** — and
//! [`Traction`], the consistent nodal forces a surface traction produces.
//!
//! # What does NOT belong here
//!
//! The solve itself ([`crate::solver`]) and the assembly ([`crate::assembly`]).
//!
//! # Two Dirichlet methods, and when each is right
//!
//! | Method | Exactness | Conditioning | Symmetry | Use when |
//! |---|---|---|---|---|
//! | [`DirichletMethod::Elimination`] | exact | unchanged | preserved | almost always |
//! | [`DirichletMethod::Penalty`] | approximate, error `~ 1 / beta` | degraded by `beta` | preserved | a constraint that must not change the sparsity pattern, or a quick comparison |
//!
//! **Elimination** zeroes the constrained row *and column*, puts the diagonal
//! back, and moves the column's contribution onto the right-hand side. It
//! imposes the constraint to machine precision and keeps the matrix symmetric,
//! so the conjugate-gradient solver stays applicable. It is the default.
//!
//! **Penalty** adds `beta` to the diagonal and `beta * u_prescribed` to the
//! right-hand side. The constraint is then satisfied only to about
//! `k_max / beta` in relative terms, and the condition number grows in
//! proportion to `beta` — so there is a real trade-off and no value of `beta`
//! escapes it. Both are implemented because the patch test is a sharper check
//! when it is run through both: a patch test that passes under elimination but
//! fails under a moderate penalty is telling you about the penalty, whereas one
//! that fails under both is telling you about the element.
//!
//! # Units
//!
//! Prescribed displacements in metres, tractions in pascals (newton per square
//! metre in 3-D; in 2-D a line traction in newton per square metre acting
//! through unit thickness, which integrates to newton per metre of thickness),
//! nodal forces in newtons, stiffness in newton per metre.

use crate::dof::{DofId, DofMap};
use crate::element::FacetType;
use crate::error::{FemError, Result};
use crate::mesh::{Facet, Mesh, NodeId};
use crate::quadrature::facet_rule;
use crate::sparse::CsrMatrix;

/// How a prescribed displacement is imposed on the linear system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DirichletMethod {
    /// Zero the row and column, restore the diagonal, and move the eliminated
    /// column onto the right-hand side. Exact, symmetry-preserving; the
    /// default.
    Elimination,
    /// Add `beta` to the diagonal and `beta u_prescribed` to the right-hand
    /// side.
    ///
    /// `beta` is a **relative** factor: the actual penalty stiffness is
    /// `beta * max |K_ii|` \[N/m\], so the same number behaves the same way on
    /// a soft rubber and a stiff steel model. Values around `1e8` impose the
    /// constraint to roughly eight significant figures at a comparable cost in
    /// condition number.
    Penalty(f64),
}

/// A set of prescribed displacement degrees of freedom.
///
/// # Units
///
/// Values are displacements in metres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DirichletSet {
    /// Constrained degrees of freedom, in insertion order. Duplicates are
    /// permitted; the last value written wins, which makes "constrain the whole
    /// boundary, then override one corner" behave as expected.
    pub dofs: Vec<DofId>,
    /// Prescribed displacement \[m\] for each entry of `dofs`.
    pub values: Vec<f64>,
}

impl DirichletSet {
    /// An empty constraint set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of constrained entries (dimensionless count). Duplicates count
    /// separately.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dofs.len()
    }

    /// Whether no constraint has been added.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dofs.is_empty()
    }

    /// Constrain one component of one node to `value` metres.
    ///
    /// `component` is the spatial direction (0 = x, 1 = y, 2 = z).
    pub fn fix(&mut self, dofs: &DofMap, node: NodeId, component: usize, value: f64) {
        self.dofs.push(dofs.dof(node, component));
        self.values.push(value);
    }

    /// Constrain every component of `nodes` to zero — a clamped support.
    pub fn fix_all_components(&mut self, dofs: &DofMap, nodes: &[NodeId]) {
        for &n in nodes {
            for c in 0..dofs.n_components() {
                self.fix(dofs, n, c, 0.0);
            }
        }
    }

    /// Constrain every node in `nodes` to the value a field gives at its
    /// coordinate — the boundary condition a patch test or a manufactured
    /// solution needs.
    ///
    /// `field` maps a coordinate \[m\] to a displacement \[m\]; it is a generic
    /// bound, not a boxed closure.
    pub fn fix_from_field<F: Fn([f64; 3]) -> [f64; 3]>(
        &mut self,
        mesh: &Mesh,
        dofs: &DofMap,
        nodes: &[NodeId],
        field: F,
    ) {
        for &n in nodes {
            let v = field(mesh.node(n));
            for c in 0..dofs.n_components() {
                self.fix(dofs, n, c, v[c]);
            }
        }
    }

    /// A dense mask, `true` where the degree of freedom is constrained.
    #[must_use]
    pub fn mask(&self, n_dofs: usize) -> Vec<bool> {
        let mut m = vec![false; n_dofs];
        for d in &self.dofs {
            m[d.0] = true;
        }
        m
    }

    /// The prescribed values as a dense vector \[m\], zero where unconstrained.
    #[must_use]
    pub fn dense_values(&self, n_dofs: usize) -> Vec<f64> {
        let mut v = vec![0.0; n_dofs];
        for (d, val) in self.dofs.iter().zip(&self.values) {
            v[d.0] = *val;
        }
        v
    }

    /// Apply the constraints to a linear system `K du = r` in which the unknown
    /// is an **increment**, so the prescribed increment is
    /// `u_prescribed - u_current`.
    ///
    /// This is the form the Newton loop needs: the same routine serves the
    /// first iteration (where the increment carries the whole prescribed
    /// displacement) and every later one (where it carries nothing, because the
    /// constraint is already satisfied).
    ///
    /// # Arguments
    ///
    /// - `k` — the tangent matrix \[N/m\], modified in place.
    /// - `rhs` — the residual \[N\], modified in place.
    /// - `u_current` — the current total displacement \[m\], or an empty slice
    ///   to treat the current state as zero.
    /// - `method` — elimination or penalty.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] if the value list and the degree-of-freedom
    /// list disagree, or a slice is the wrong length.
    pub fn apply(
        &self,
        k: &mut CsrMatrix,
        rhs: &mut [f64],
        u_current: &[f64],
        method: DirichletMethod,
    ) -> Result<()> {
        let n = k.n_rows();
        if self.dofs.len() != self.values.len() {
            return Err(FemError::LengthMismatch {
                context: "DirichletSet::apply: dofs and values",
                expected: self.dofs.len(),
                actual: self.values.len(),
            });
        }
        if rhs.len() != n {
            return Err(FemError::LengthMismatch {
                context: "DirichletSet::apply: rhs",
                expected: n,
                actual: rhs.len(),
            });
        }
        if !u_current.is_empty() && u_current.len() != n {
            return Err(FemError::LengthMismatch {
                context: "DirichletSet::apply: current displacement",
                expected: n,
                actual: u_current.len(),
            });
        }

        // Required increment per constrained degree of freedom, last write wins.
        let mut mask = vec![false; n];
        let mut inc = vec![0.0; n];
        for (d, val) in self.dofs.iter().zip(&self.values) {
            mask[d.0] = true;
            inc[d.0] = val - if u_current.is_empty() { 0.0 } else { u_current[d.0] };
        }

        match method {
            DirichletMethod::Elimination => {
                // Move the constrained columns onto the right-hand side.
                for i in 0..n {
                    if mask[i] {
                        continue;
                    }
                    let mut acc = 0.0;
                    for kk in k.row_ptr()[i]..k.row_ptr()[i + 1] {
                        let j = k.col_idx()[kk];
                        if mask[j] {
                            acc += k.values()[kk] * inc[j];
                        }
                    }
                    rhs[i] -= acc;
                }
                // Zero the constrained rows and columns, restore the diagonal.
                let scale = k.max_abs_diagonal().max(1.0);
                for i in 0..n {
                    if !mask[i] {
                        // Zero the constrained entries of this row's columns.
                        for kk in k.row_ptr()[i]..k.row_ptr()[i + 1] {
                            let j = k.col_idx()[kk];
                            if mask[j] {
                                let col = j;
                                k.set(i, col, 0.0);
                            }
                        }
                        continue;
                    }
                    let d = k.get(i, i).abs().max(scale * 1e-12);
                    for kk in k.row_ptr()[i]..k.row_ptr()[i + 1] {
                        let j = k.col_idx()[kk];
                        k.set(i, j, 0.0);
                    }
                    k.set(i, i, d);
                    rhs[i] = d * inc[i];
                }
            }
            DirichletMethod::Penalty(beta) => {
                let big = beta * k.max_abs_diagonal().max(1.0);
                for i in 0..n {
                    if mask[i] {
                        k.add(i, i, big)?;
                        rhs[i] += big * inc[i];
                    }
                }
            }
        }
        Ok(())
    }
}

/// A prescribed surface traction on a set of facets.
///
/// # Units
///
/// `traction` is in pascals (newton per square metre). For a 2-D plane-strain
/// model the facets are edges and the integral is taken through unit thickness,
/// so the resulting nodal forces are newton per metre of thickness — reported
/// simply as newtons, with the unit-thickness convention stated once here.
#[derive(Debug, Clone, PartialEq)]
pub struct Traction {
    /// The facets the traction acts on.
    pub facets: Vec<Facet>,
    /// The traction vector `[t_x, t_y, t_z]` \[Pa\], uniform over every facet
    /// in the set.
    pub traction: [f64; 3],
}

impl Traction {
    /// A uniform traction on a facet set.
    #[must_use]
    pub fn uniform(facets: Vec<Facet>, traction: [f64; 3]) -> Self {
        Self { facets, traction }
    }

    /// Accumulate the consistent nodal forces \[N\] into `f`.
    ///
    /// Integrates `N_a t` over each facet with [`crate::quadrature::facet_rule`].
    /// "Consistent" means the nodal forces are the weak-form work conjugates of
    /// the nodal displacements, not a lumped share of the total — which matters
    /// for a quadratic element, where lumping would put the wrong fraction on
    /// the midside node (it is 2/3, not 1/3).
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] for a facet whose node count does not match
    /// its declared [`FacetType`].
    pub fn accumulate(&self, mesh: &Mesh, dofs: &DofMap, f: &mut [f64]) -> Result<()> {
        let nc = dofs.n_components();
        for facet in &self.facets {
            let ft = facet.facet_type;
            if facet.nodes.len() != ft.n_nodes() {
                return Err(FemError::LengthMismatch {
                    context: "Traction::accumulate: facet node count",
                    expected: ft.n_nodes(),
                    actual: facet.nodes.len(),
                });
            }
            let coords: Vec<[f64; 3]> = facet.nodes.iter().map(|n| mesh.node(*n)).collect();
            for q in facet_rule(ft) {
                let jm = ft.measure_jacobian(&coords, q.xi);
                let w = q.weight * jm;
                let n = ft.shape_functions(q.xi);
                for (a, node) in facet.nodes.iter().enumerate() {
                    for c in 0..nc {
                        f[node.0 * nc + c] += w * n[a] * self.traction[c];
                    }
                }
            }
        }
        Ok(())
    }
}

/// Accumulate the consistent nodal forces \[N\] of a **normal pressure** on a
/// set of facets whose outward normal points away from an axis.
///
/// This is the internal-pressure loading of the thick-walled-cylinder case: the
/// traction is `t = -p n` with `n` the outward normal of the solid, which on
/// the inner bore points towards the axis, so the load pushes outward.
///
/// The normal is computed **per facet from its own geometry**, so a faceted
/// approximation of a curved surface gets each chord's own normal rather than
/// an averaged one.
///
/// # Arguments
///
/// - `mesh` — the mesh.
/// - `dofs` — the displacement degree-of-freedom map.
/// - `facets` — the loaded facets. Currently only [`FacetType::Line2`] in 2-D
///   is supported; anything else returns an error rather than silently
///   applying nothing.
/// - `pressure` — `p` \[Pa\], positive meaning compression on the surface
///   (pushing into the solid).
/// - `f` — nodal force vector \[N\], accumulated into.
///
/// # Errors
///
/// [`FemError::BoundaryCondition`] for an unsupported facet type.
pub fn accumulate_pressure_2d(
    mesh: &Mesh,
    dofs: &DofMap,
    facets: &[Facet],
    pressure: f64,
    f: &mut [f64],
) -> Result<()> {
    let nc = dofs.n_components();
    for facet in facets {
        if facet.facet_type != FacetType::Line2 {
            return Err(FemError::BoundaryCondition(format!(
                "accumulate_pressure_2d supports Line2 facets only, got {:?}",
                facet.facet_type
            )));
        }
        let a = mesh.node(facet.nodes[0]);
        let b = mesh.node(facet.nodes[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        // Unit normal of the segment. With the facet running anticlockwise
        // around the solid's boundary, the outward normal is (dy, -dx) / len
        // for a bore whose interior is at larger radius; the sign is checked
        // against the midpoint's radial direction so the caller cannot get it
        // silently backwards.
        let mut nx = dy / len;
        let mut ny = -dx / len;
        let mid = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
        let rr = (mid[0] * mid[0] + mid[1] * mid[1]).sqrt();
        if rr > 0.0 && (nx * mid[0] + ny * mid[1]) / rr > 0.0 {
            // Points outward from the axis: for a bore we want the reverse.
            nx = -nx;
            ny = -ny;
        }
        // t = -p n, with n the outward normal of the SOLID (here -radial).
        let t = [-pressure * nx, -pressure * ny, 0.0];
        for q in facet_rule(FacetType::Line2) {
            let w = q.weight * 0.5 * len;
            let n = FacetType::Line2.shape_functions(q.xi);
            for (k, node) in facet.nodes.iter().enumerate() {
                for c in 0..nc {
                    f[node.0 * nc + c] += w * n[k] * t[c];
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dof::SparsityPattern;
    use crate::mesh::{quarter_annulus_quad4, quarter_annulus_inner_facets, unit_square_quad4};

    fn dense(n: usize) -> SparsityPattern {
        let mut row_ptr = vec![0];
        let mut col_idx = Vec::new();
        for _ in 0..n {
            for j in 0..n {
                col_idx.push(j);
            }
            row_ptr.push(col_idx.len());
        }
        SparsityPattern { row_ptr, col_idx }
    }

    /// Elimination must give the exact constrained solution and keep the matrix
    /// symmetric.
    #[test]
    fn elimination_is_exact_and_symmetric() {
        let n = 4;
        let mut k = CsrMatrix::from_pattern(&dense(n));
        for i in 0..n {
            for j in 0..n {
                let v = if i == j { 4.0 } else { -1.0 };
                k.add(i, j, v).unwrap();
            }
        }
        let mut rhs = vec![1.0; n];
        let set = DirichletSet {
            dofs: vec![DofId(0), DofId(3)],
            values: vec![0.5, -0.25],
        };
        set.apply(&mut k, &mut rhs, &[], DirichletMethod::Elimination)
            .unwrap();
        assert!(k.max_asymmetry() / k.max_abs_diagonal() < 1e-14);
        // Solve the 4x4 system densely by Gaussian elimination.
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                a[i * n + j] = k.get(i, j);
            }
        }
        let x = dense_solve(&mut a, &mut rhs.clone(), n);
        assert!((x[0] - 0.5).abs() < 1e-13);
        assert!((x[3] + 0.25).abs() < 1e-13);
    }

    /// The penalty method must approach the same answer, with an error that
    /// falls as `1 / beta` — measured, not assumed.
    #[test]
    fn penalty_error_falls_like_one_over_beta() {
        let n = 4;
        let build = || {
            let mut k = CsrMatrix::from_pattern(&dense(n));
            for i in 0..n {
                for j in 0..n {
                    k.add(i, j, if i == j { 4.0 } else { -1.0 }).unwrap();
                }
            }
            k
        };
        let mut prev = f64::INFINITY;
        for beta in [1e4_f64, 1e6, 1e8] {
            let mut k = build();
            let mut rhs = vec![1.0; n];
            let set = DirichletSet {
                dofs: vec![DofId(0)],
                values: vec![0.5],
            };
            set.apply(&mut k, &mut rhs, &[], DirichletMethod::Penalty(beta))
                .unwrap();
            let mut a = vec![0.0; n * n];
            for i in 0..n {
                for j in 0..n {
                    a[i * n + j] = k.get(i, j);
                }
            }
            let x = dense_solve(&mut a, &mut rhs, n);
            let err = (x[0] - 0.5).abs();
            if prev.is_finite() {
                assert!(err < prev / 50.0, "penalty error {} vs previous {}", err, prev);
            }
            prev = err;
        }
    }

    /// A uniform traction on an edge must integrate to traction times length,
    /// and distribute it consistently.
    #[test]
    fn uniform_traction_integrates_to_the_total_load() {
        let mesh = unit_square_quad4(4).unwrap();
        let dofs = DofMap::displacement(&mesh);
        let facets = crate::mesh::rectangle_right_edge_facets(4, 4);
        let t = Traction::uniform(facets, [0.0, -1.0e6, 0.0]);
        let mut f = vec![0.0; dofs.n_dofs()];
        t.accumulate(&mesh, &dofs, &mut f).unwrap();
        let total: f64 = f.iter().skip(1).step_by(2).sum();
        // Edge length 1 m times traction -1e6 Pa (unit thickness) = -1e6 N.
        assert!((total + 1.0e6).abs() < 1e-3, "total {}", total);
    }

    /// The internal pressure on the annulus bore must produce a net radial
    /// resultant equal to `p a` times the chord projection, pointing outward in
    /// both x and y for a quarter model.
    #[test]
    fn bore_pressure_pushes_outward() {
        let (a, b, nr, nt) = (0.5, 1.0, 3usize, 8usize);
        let mesh = quarter_annulus_quad4(a, b, nr, nt).unwrap();
        let dofs = DofMap::displacement(&mesh);
        let facets = quarter_annulus_inner_facets(nr, nt);
        let p = 100.0e6;
        let mut f = vec![0.0; dofs.n_dofs()];
        accumulate_pressure_2d(&mesh, &dofs, &facets, p, &mut f).unwrap();
        let fx: f64 = f.iter().step_by(2).sum();
        let fy: f64 = f.iter().skip(1).step_by(2).sum();
        assert!(fx > 0.0 && fy > 0.0, "resultant ({}, {}) N", fx, fy);
        // For a quarter bore the exact resultant is p * a in each direction.
        assert!((fx - p * a).abs() / (p * a) < 0.02, "fx {}", fx);
        assert!((fy - p * a).abs() / (p * a) < 0.02, "fy {}", fy);
    }

    /// Plain dense Gaussian elimination with partial pivoting, for the tiny
    /// systems in these tests only.
    fn dense_solve(a: &mut [f64], b: &mut [f64], n: usize) -> Vec<f64> {
        for c in 0..n {
            let mut piv = c;
            for r in c + 1..n {
                if a[r * n + c].abs() > a[piv * n + c].abs() {
                    piv = r;
                }
            }
            if piv != c {
                for j in 0..n {
                    a.swap(c * n + j, piv * n + j);
                }
                b.swap(c, piv);
            }
            let d = a[c * n + c];
            for r in c + 1..n {
                let m = a[r * n + c] / d;
                if m == 0.0 {
                    continue;
                }
                for j in c..n {
                    a[r * n + j] -= m * a[c * n + j];
                }
                b[r] -= m * b[c];
            }
        }
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let mut s = b[i];
            for j in i + 1..n {
                s -= a[i * n + j] * x[j];
            }
            x[i] = s / a[i * n + i];
        }
        x
    }
}
