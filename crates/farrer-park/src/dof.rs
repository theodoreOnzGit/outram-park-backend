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

//! Degree-of-freedom numbering and the sparsity pattern of the resulting
//! stiffness matrix.
//!
//! # What belongs in this module
//!
//! [`DofMap`] — the bijection between `(node, component)` and a global
//! [`DofId`] — and [`SparsityPattern`], the degree-of-freedom adjacency graph
//! that [`crate::sparse::CsrMatrix`] is built on.
//!
//! # What does NOT belong here
//!
//! Coefficients. A sparsity pattern records *where* an entry can be non-zero,
//! never what it is.
//!
//! # Numbering: node-major, component-minor
//!
//! Degree of freedom `d` of node `n` is `n * n_components + d`. Grouping a
//! node's components together is the layout that makes the element block
//! contiguous, which matters because assembly touches
//! `(n_nodes_per_element * n_components)^2` entries per element, and because a
//! block-diagonal preconditioner would want it. The alternative
//! (component-major: all `u_x`, then all `u_y`) scatters each element's
//! contributions across the matrix and is not used anywhere here.
//!
//! # Units
//!
//! Indices are dimensionless. The *values* a `DofMap` indexes are nodal
//! displacements in metres.

use crate::element::ElementType;
use crate::error::{FemError, Result};
use crate::mesh::{ElemId, Mesh, NodeId};

/// Index of a global degree of freedom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DofId(pub usize);

/// The map from `(node, component)` to a global degree of freedom, for one
/// vector field.
///
/// For solid mechanics the field is the displacement and `n_components` equals
/// the mesh dimension: two in plane strain, three in 3-D.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DofMap {
    n_nodes: usize,
    n_components: usize,
}

impl DofMap {
    /// Build the displacement degree-of-freedom map of a mesh: one component
    /// per spatial dimension at every node.
    #[must_use]
    pub fn displacement(mesh: &Mesh) -> Self {
        Self {
            n_nodes: mesh.n_nodes(),
            n_components: mesh.dim(),
        }
    }

    /// Build a map with an explicit component count, for a field that is not
    /// the displacement.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] if `n_components` is zero.
    pub fn new(n_nodes: usize, n_components: usize) -> Result<Self> {
        if n_components == 0 {
            return Err(FemError::LengthMismatch {
                context: "DofMap::new: n_components",
                expected: 1,
                actual: 0,
            });
        }
        Ok(Self {
            n_nodes,
            n_components,
        })
    }

    /// Components per node (dimensionless count).
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.n_components
    }

    /// Total number of degrees of freedom, `n_nodes * n_components`.
    #[must_use]
    pub fn n_dofs(&self) -> usize {
        self.n_nodes * self.n_components
    }

    /// Number of nodes (dimensionless count).
    #[must_use]
    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }

    /// The global degree of freedom for component `component` of node `node`.
    ///
    /// `component` must be less than [`n_components`](Self::n_components); it
    /// is the spatial direction (0 = x, 1 = y, 2 = z) for a displacement field.
    #[must_use]
    pub fn dof(&self, node: NodeId, component: usize) -> DofId {
        debug_assert!(component < self.n_components);
        DofId(node.0 * self.n_components + component)
    }

    /// The `(node, component)` a global degree of freedom belongs to — the
    /// inverse of [`dof`](Self::dof).
    #[must_use]
    pub fn node_component(&self, dof: DofId) -> (NodeId, usize) {
        (
            NodeId(dof.0 / self.n_components),
            dof.0 % self.n_components,
        )
    }

    /// Gather one element's global degrees of freedom into `out`, in
    /// node-major order matching the element's local node ordering.
    ///
    /// `out` must have `element_type.n_nodes() * n_components` entries. This is
    /// the scatter map assembly uses, so its ordering and the element matrix's
    /// ordering are the same thing by construction.
    pub fn element_dofs(&self, mesh: &Mesh, e: ElemId, out: &mut [usize]) {
        let nc = self.n_components;
        for (a, &n) in mesh.element_nodes(e).iter().enumerate() {
            for c in 0..nc {
                out[a * nc + c] = n * nc + c;
            }
        }
    }

    /// Element degree-of-freedom count, `nodes per element * n_components`.
    #[must_use]
    pub fn element_dof_count(&self, element: ElementType) -> usize {
        element.n_nodes() * self.n_components
    }
}

/// The compressed adjacency graph of the degrees of freedom: which entries of
/// the stiffness matrix can be non-zero.
///
/// Two degrees of freedom are adjacent when they belong to nodes sharing at
/// least one element, which is exactly the set of entries the element loop can
/// write. The diagonal is always present, even where it would be structurally
/// zero, so a Jacobi preconditioner and a Dirichlet elimination always have a
/// slot to write into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparsityPattern {
    /// Row offsets, length `n_rows + 1`, ascending, `row_ptr[0] == 0`.
    pub row_ptr: Vec<usize>,
    /// Column indices, **sorted ascending within each row**. Sortedness is
    /// relied on by [`crate::sparse::CsrMatrix::add`], which binary-searches.
    pub col_idx: Vec<usize>,
}

impl SparsityPattern {
    /// Number of rows (dimensionless count).
    #[must_use]
    pub fn n_rows(&self) -> usize {
        self.row_ptr.len() - 1
    }

    /// Number of stored entries (dimensionless count).
    #[must_use]
    pub fn nnz(&self) -> usize {
        self.col_idx.len()
    }

    /// Build the pattern from the degree-of-freedom graph of a mesh.
    ///
    /// # Cost
    ///
    /// `O(n_elements * (nodes per element * n_components)^2)` insertions, then
    /// one sort-and-dedup per row. For the meshes in this crate that is
    /// microseconds; a production mesh would want a bucketed build.
    #[must_use]
    pub fn from_mesh(mesh: &Mesh, dofs: &DofMap) -> Self {
        let n = dofs.n_dofs();
        let mut rows: Vec<Vec<usize>> = vec![Vec::new(); n];
        let nc = dofs.n_components();
        let nn = mesh.element_type().n_nodes();
        let mut ed = vec![0usize; nn * nc];
        for e in 0..mesh.n_elements() {
            dofs.element_dofs(mesh, ElemId(e), &mut ed);
            for &i in ed.iter() {
                rows[i].extend_from_slice(&ed);
            }
        }
        let mut row_ptr = Vec::with_capacity(n + 1);
        let mut col_idx = Vec::new();
        row_ptr.push(0);
        for (i, r) in rows.iter_mut().enumerate() {
            r.push(i); // the diagonal is always stored
            r.sort_unstable();
            r.dedup();
            col_idx.extend_from_slice(r);
            row_ptr.push(col_idx.len());
        }
        SparsityPattern { row_ptr, col_idx }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{unit_square_quad4, unit_square_tri3};

    /// Numbering must round-trip in both directions.
    #[test]
    fn dof_numbering_round_trips() {
        let m = unit_square_quad4(2).unwrap();
        let d = DofMap::displacement(&m);
        assert_eq!(d.n_components(), 2);
        assert_eq!(d.n_dofs(), 9 * 2);
        for n in 0..m.n_nodes() {
            for c in 0..2 {
                let g = d.dof(NodeId(n), c);
                assert_eq!(d.node_component(g), (NodeId(n), c));
            }
        }
    }

    /// The pattern must be symmetric (the stiffness matrix is), sorted, and
    /// must contain every diagonal.
    #[test]
    fn pattern_is_symmetric_sorted_and_has_a_full_diagonal() {
        let m = unit_square_tri3(3).unwrap();
        let d = DofMap::displacement(&m);
        let p = SparsityPattern::from_mesh(&m, &d);
        assert_eq!(p.n_rows(), d.n_dofs());
        for i in 0..p.n_rows() {
            let row = &p.col_idx[p.row_ptr[i]..p.row_ptr[i + 1]];
            assert!(row.windows(2).all(|w| w[0] < w[1]), "row {} unsorted", i);
            assert!(row.contains(&i), "row {} has no diagonal", i);
            for &j in row {
                let other = &p.col_idx[p.row_ptr[j]..p.row_ptr[j + 1]];
                assert!(other.contains(&i), "pattern not symmetric at ({}, {})", i, j);
            }
        }
    }

    /// On a single-element mesh every degree of freedom couples to every other,
    /// so the pattern is dense: `(n_nodes * n_components)^2` entries.
    #[test]
    fn single_element_pattern_is_dense() {
        let m = unit_square_quad4(1).unwrap();
        let d = DofMap::displacement(&m);
        let p = SparsityPattern::from_mesh(&m, &d);
        assert_eq!(p.nnz(), 8 * 8);
    }
}
