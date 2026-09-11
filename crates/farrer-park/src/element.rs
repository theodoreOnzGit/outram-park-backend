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

//! Reference elements: shape functions, their natural derivatives, and the
//! isoparametric mapping to physical coordinates.
//!
//! # What belongs in this module
//!
//! The closed set of supported reference elements ([`ElementType`]) and their
//! *interpolation*: `N_a(xi)`, `dN_a/dxi_j`, the Jacobian `J = dx/dxi`, its
//! determinant, and the physical gradients `dN_a/dx_i`. Also the boundary
//! counterpart, [`FacetType`], used to integrate surface tractions.
//!
//! # What does NOT belong here
//!
//! Quadrature points and weights ([`crate::quadrature`]), connectivity
//! ([`crate::mesh`]), and the B-matrix ([`crate::assembly`]). This module knows
//! about one element in isolation and nothing about the mesh it sits in.
//!
//! # Element set and node ordering
//!
//! | Element | Dim | Nodes | Interpolation | Reference domain |
//! |---|---|---|---|---|
//! | [`ElementType::Tri3`] | 2 | 3 | linear | `xi, eta >= 0`, `xi + eta <= 1` |
//! | [`ElementType::Tri6`] | 2 | 6 | quadratic | as Tri3 |
//! | [`ElementType::Quad4`] | 2 | 4 | bilinear | `[-1, 1]^2` |
//! | [`ElementType::Tet4`] | 3 | 4 | linear | `xi, eta, zeta >= 0`, sum `<= 1` |
//! | [`ElementType::Hex8`] | 3 | 8 | trilinear | `[-1, 1]^3` |
//!
//! Node ordering follows the usual convention and is stated per variant. It
//! matters: a mesh whose connectivity lists Quad4 nodes in a figure-of-eight
//! order produces a negative Jacobian, which is reported as
//! [`crate::error::FemError::DegenerateElement`] rather than silently
//! integrating a negative area.
//!
//! # Isoparametric, always
//!
//! The same `N_a` interpolate both the geometry and the displacement. For
//! [`ElementType::Tri6`] that means curved edges are representable — but every
//! mesh generator in [`crate::mesh`] emits **straight-sided** Tri6 with midside
//! nodes exactly at edge midpoints, because the verification cases do not need
//! curved elements and a straight-sided quadratic triangle has a constant
//! Jacobian, which makes the patch test exact.
//!
//! # Units
//!
//! Natural coordinates `xi` are dimensionless. Nodal coordinates and the
//! Jacobian are in metres; `det J` is in square metres (2-D) or cubic metres
//! (3-D); physical shape-function gradients `dN/dx` are in reciprocal metres.

use crate::error::{FemError, Result};

/// Maximum number of nodes any supported element has ([`ElementType::Hex8`]).
///
/// Fixed-size arrays of this length are used throughout so that element-local
/// work needs no heap allocation inside the quadrature loop.
pub const MAX_ELEM_NODES: usize = 8;

/// Maximum number of nodes any supported facet has
/// ([`FacetType::Quad4`], four).
pub const MAX_FACET_NODES: usize = 4;

/// The closed set of reference elements Farrer Park supports.
///
/// An enum rather than a trait object, per the workspace design rules: adding
/// an element type makes every `match` in the crate a compile error until it is
/// handled, which is the behaviour wanted for a set this small and this
/// load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementType {
    /// Three-node linear triangle. Nodes are the corners, counter-clockwise.
    ///
    /// Shape functions are the area coordinates
    /// `N = [1 - xi - eta, xi, eta]`; the gradient is constant over the
    /// element, so it produces constant strain (the classic CST element).
    Tri3,
    /// Six-node quadratic triangle. Nodes 0-2 are the corners
    /// counter-clockwise; nodes 3, 4, 5 are the midsides of edges 0-1, 1-2 and
    /// 2-0 respectively.
    Tri6,
    /// Four-node bilinear quadrilateral. Nodes counter-clockwise starting at
    /// `(-1, -1)`: `(-1,-1), (1,-1), (1,1), (-1,1)`.
    Quad4,
    /// Four-node linear tetrahedron. Nodes ordered so that the fourth lies on
    /// the positive side of the plane of the first three (positive volume).
    Tet4,
    /// Eight-node trilinear hexahedron. Nodes 0-3 are the `zeta = -1` face
    /// counter-clockwise from `(-1,-1,-1)`; nodes 4-7 are the matching
    /// `zeta = +1` face.
    Hex8,
}

impl ElementType {
    /// Number of nodes (dimensionless count).
    #[must_use]
    pub fn n_nodes(self) -> usize {
        match self {
            ElementType::Tri3 => 3,
            ElementType::Tri6 => 6,
            ElementType::Quad4 => 4,
            ElementType::Tet4 => 4,
            ElementType::Hex8 => 8,
        }
    }

    /// Spatial dimension of the reference domain: 2 or 3 (dimensionless).
    #[must_use]
    pub fn dim(self) -> usize {
        match self {
            ElementType::Tri3 | ElementType::Tri6 | ElementType::Quad4 => 2,
            ElementType::Tet4 | ElementType::Hex8 => 3,
        }
    }

    /// Highest complete polynomial degree the interpolation reproduces exactly.
    ///
    /// `1` for [`Tri3`](ElementType::Tri3), [`Quad4`](ElementType::Quad4),
    /// [`Tet4`](ElementType::Tet4) and [`Hex8`](ElementType::Hex8) (the last
    /// two carry extra bilinear/trilinear terms but only complete degree 1);
    /// `2` for [`Tri6`](ElementType::Tri6). This is what sets the expected
    /// convergence order in the manufactured-solution study: `L2` error should
    /// decay as `h^(p+1)`, `H1` as `h^p`.
    #[must_use]
    pub fn polynomial_order(self) -> usize {
        match self {
            ElementType::Tri6 => 2,
            _ => 1,
        }
    }

    /// Human-readable name, used in error messages.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ElementType::Tri3 => "Tri3",
            ElementType::Tri6 => "Tri6",
            ElementType::Quad4 => "Quad4",
            ElementType::Tet4 => "Tet4",
            ElementType::Hex8 => "Hex8",
        }
    }

    /// The facet (boundary) element type of this element: an edge in 2-D, a
    /// face in 3-D.
    ///
    /// [`ElementType::Hex8`] returns [`FacetType::Quad4`],
    /// [`ElementType::Tet4`] returns [`FacetType::Tri3`], the linear 2-D
    /// elements return [`FacetType::Line2`] and [`ElementType::Tri6`] returns
    /// [`FacetType::Line3`].
    #[must_use]
    pub fn facet_type(self) -> FacetType {
        match self {
            ElementType::Tri3 | ElementType::Quad4 => FacetType::Line2,
            ElementType::Tri6 => FacetType::Line3,
            ElementType::Tet4 => FacetType::Tri3,
            ElementType::Hex8 => FacetType::Quad4,
        }
    }

    /// Shape-function values `N_a(xi)` at a natural coordinate.
    ///
    /// # Arguments
    ///
    /// - `xi` — natural coordinates, dimensionless. Only the first
    ///   [`dim`](Self::dim) entries are read.
    ///
    /// # Returns
    ///
    /// A fixed array of [`MAX_ELEM_NODES`]; only the first
    /// [`n_nodes`](Self::n_nodes) entries are meaningful, the rest are zero.
    /// The meaningful entries sum to one (partition of unity) for every
    /// element type.
    #[must_use]
    pub fn shape_functions(self, xi: [f64; 3]) -> [f64; MAX_ELEM_NODES] {
        let mut n = [0.0; MAX_ELEM_NODES];
        match self {
            ElementType::Tri3 => {
                n[0] = 1.0 - xi[0] - xi[1];
                n[1] = xi[0];
                n[2] = xi[1];
            }
            ElementType::Tri6 => {
                let (l1, l2, l3) = (1.0 - xi[0] - xi[1], xi[0], xi[1]);
                n[0] = l1 * (2.0 * l1 - 1.0);
                n[1] = l2 * (2.0 * l2 - 1.0);
                n[2] = l3 * (2.0 * l3 - 1.0);
                n[3] = 4.0 * l1 * l2;
                n[4] = 4.0 * l2 * l3;
                n[5] = 4.0 * l3 * l1;
            }
            ElementType::Quad4 => {
                let (r, s) = (xi[0], xi[1]);
                n[0] = 0.25 * (1.0 - r) * (1.0 - s);
                n[1] = 0.25 * (1.0 + r) * (1.0 - s);
                n[2] = 0.25 * (1.0 + r) * (1.0 + s);
                n[3] = 0.25 * (1.0 - r) * (1.0 + s);
            }
            ElementType::Tet4 => {
                n[0] = 1.0 - xi[0] - xi[1] - xi[2];
                n[1] = xi[0];
                n[2] = xi[1];
                n[3] = xi[2];
            }
            ElementType::Hex8 => {
                let (r, s, t) = (xi[0], xi[1], xi[2]);
                let signs = HEX8_NODE_SIGNS;
                for (a, sg) in signs.iter().enumerate() {
                    n[a] = 0.125 * (1.0 + sg[0] * r) * (1.0 + sg[1] * s) * (1.0 + sg[2] * t);
                }
            }
        }
        n
    }

    /// Natural derivatives `dN_a/dxi_j` at a natural coordinate.
    ///
    /// # Returns
    ///
    /// `[a][j]` for node `a` and natural direction `j`, dimensionless. Only the
    /// first [`n_nodes`](Self::n_nodes) rows and [`dim`](Self::dim) columns are
    /// meaningful. Each column sums to zero over the nodes, which is the
    /// derivative of the partition-of-unity identity and is what makes a
    /// constant field produce zero strain.
    #[must_use]
    pub fn shape_derivatives(self, xi: [f64; 3]) -> [[f64; 3]; MAX_ELEM_NODES] {
        let mut d = [[0.0; 3]; MAX_ELEM_NODES];
        match self {
            ElementType::Tri3 => {
                d[0] = [-1.0, -1.0, 0.0];
                d[1] = [1.0, 0.0, 0.0];
                d[2] = [0.0, 1.0, 0.0];
            }
            ElementType::Tri6 => {
                let (l1, l2, l3) = (1.0 - xi[0] - xi[1], xi[0], xi[1]);
                // dL1/dxi = -1, dL1/deta = -1; dL2/dxi = 1; dL3/deta = 1.
                d[0] = [-(4.0 * l1 - 1.0), -(4.0 * l1 - 1.0), 0.0];
                d[1] = [4.0 * l2 - 1.0, 0.0, 0.0];
                d[2] = [0.0, 4.0 * l3 - 1.0, 0.0];
                d[3] = [4.0 * (l1 - l2), -4.0 * l2, 0.0];
                d[4] = [4.0 * l3, 4.0 * l2, 0.0];
                d[5] = [-4.0 * l3, 4.0 * (l1 - l3), 0.0];
            }
            ElementType::Quad4 => {
                let (r, s) = (xi[0], xi[1]);
                let sg = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                for (a, g) in sg.iter().enumerate() {
                    d[a] = [
                        0.25 * g[0] * (1.0 + g[1] * s),
                        0.25 * g[1] * (1.0 + g[0] * r),
                        0.0,
                    ];
                }
            }
            ElementType::Tet4 => {
                d[0] = [-1.0, -1.0, -1.0];
                d[1] = [1.0, 0.0, 0.0];
                d[2] = [0.0, 1.0, 0.0];
                d[3] = [0.0, 0.0, 1.0];
            }
            ElementType::Hex8 => {
                let (r, s, t) = (xi[0], xi[1], xi[2]);
                for (a, sg) in HEX8_NODE_SIGNS.iter().enumerate() {
                    d[a] = [
                        0.125 * sg[0] * (1.0 + sg[1] * s) * (1.0 + sg[2] * t),
                        0.125 * sg[1] * (1.0 + sg[0] * r) * (1.0 + sg[2] * t),
                        0.125 * sg[2] * (1.0 + sg[0] * r) * (1.0 + sg[1] * s),
                    ];
                }
            }
        }
        d
    }
}

/// Corner signs of the eight [`ElementType::Hex8`] nodes in the reference cube.
const HEX8_NODE_SIGNS: [[f64; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];

/// The isoparametric mapping evaluated at one natural coordinate.
///
/// Produced by [`map_gradients`]; consumed by [`crate::assembly`] to build the
/// strain-displacement matrix.
#[derive(Debug, Clone, Copy)]
pub struct MappedPoint {
    /// Determinant of the Jacobian `J = dx/dxi`, in square metres (2-D
    /// elements) or cubic metres (3-D elements). Strictly positive for a
    /// correctly ordered, non-degenerate element.
    pub det_j: f64,
    /// Physical shape-function gradients `dN_a/dx_i` in reciprocal metres,
    /// indexed `[a][i]`. Rows beyond `n_nodes` and columns beyond `dim` are
    /// zero.
    pub grad: [[f64; 3]; MAX_ELEM_NODES],
}

/// Map the natural-coordinate shape-function derivatives of one element to
/// physical space.
///
/// Computes `J_ij = sum_a (dN_a/dxi_j) x_a_i`, inverts it, and returns
/// `dN_a/dx_i = sum_j (dN_a/dxi_j) (J^-1)_ji` together with `det J`.
///
/// # Arguments
///
/// - `element` — the reference element type.
/// - `coords` — nodal coordinates in metres, `coords[a] = [x, y, z]`. Must have
///   at least `element.n_nodes()` entries; the `z` entry is ignored for 2-D
///   element types.
/// - `xi` — natural coordinate, dimensionless.
///
/// # Errors
///
/// [`FemError::LengthMismatch`] if `coords` is too short, and
/// [`FemError::DegenerateElement`] if `det J <= 0` — an inverted, collapsed or
/// wrongly-ordered element. The element and quadrature-point indices in that
/// error are filled with `0` here; [`crate::assembly`] re-raises it with the
/// real indices.
pub fn map_gradients(
    element: ElementType,
    coords: &[[f64; 3]],
    xi: [f64; 3],
) -> Result<MappedPoint> {
    let nn = element.n_nodes();
    if coords.len() < nn {
        return Err(FemError::LengthMismatch {
            context: "map_gradients: element coordinates",
            expected: nn,
            actual: coords.len(),
        });
    }
    let dim = element.dim();
    let dn = element.shape_derivatives(xi);

    // J[i][j] = dx_i / dxi_j
    let mut j = [[0.0_f64; 3]; 3];
    for (a, c) in coords.iter().enumerate().take(nn) {
        for i in 0..dim {
            for k in 0..dim {
                j[i][k] += dn[a][k] * c[i];
            }
        }
    }

    let (det, jinv) = invert_small(&j, dim);
    if !(det > 0.0) || !det.is_finite() {
        return Err(FemError::DegenerateElement {
            element: 0,
            point: 0,
            det_j: det,
        });
    }

    let mut grad = [[0.0_f64; 3]; MAX_ELEM_NODES];
    for a in 0..nn {
        for i in 0..dim {
            let mut s = 0.0;
            for k in 0..dim {
                s += dn[a][k] * jinv[k][i];
            }
            grad[a][i] = s;
        }
    }
    Ok(MappedPoint { det_j: det, grad })
}

/// Physical coordinates of a natural point: `x = sum_a N_a(xi) x_a`, in metres.
///
/// Used by the manufactured-solution study to evaluate the exact field at a
/// quadrature point, and by the traction integration to locate a facet point.
#[must_use]
pub fn map_point(element: ElementType, coords: &[[f64; 3]], xi: [f64; 3]) -> [f64; 3] {
    let n = element.shape_functions(xi);
    let mut x = [0.0; 3];
    for (a, c) in coords.iter().enumerate().take(element.n_nodes()) {
        for i in 0..3 {
            x[i] += n[a] * c[i];
        }
    }
    x
}

/// Invert a `dim x dim` matrix held in the top-left of a 3x3 array.
///
/// Returns `(det, inverse)`. Only the `dim x dim` block of the inverse is
/// meaningful. `dim` is 2 or 3; any other value returns a zero determinant,
/// which the caller reports as a degenerate element.
fn invert_small(m: &[[f64; 3]; 3], dim: usize) -> (f64, [[f64; 3]; 3]) {
    let mut inv = [[0.0_f64; 3]; 3];
    match dim {
        2 => {
            let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
            if det == 0.0 {
                return (det, inv);
            }
            let r = 1.0 / det;
            inv[0][0] = m[1][1] * r;
            inv[0][1] = -m[0][1] * r;
            inv[1][0] = -m[1][0] * r;
            inv[1][1] = m[0][0] * r;
            (det, inv)
        }
        3 => {
            let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
            let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
            let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
            let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
            if det == 0.0 {
                return (det, inv);
            }
            let r = 1.0 / det;
            inv[0][0] = c00 * r;
            inv[1][0] = c01 * r;
            inv[2][0] = c02 * r;
            inv[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * r;
            inv[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * r;
            inv[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * r;
            inv[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * r;
            inv[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * r;
            inv[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * r;
            (det, inv)
        }
        _ => (0.0, inv),
    }
}

/// The closed set of boundary facets on which a surface traction can be
/// integrated.
///
/// A facet is given to [`crate::bc`] as an explicit node list rather than as an
/// `(element, local face index)` pair. That choice removes the per-element
/// local-face numbering tables entirely, at the cost of the mesh generator
/// having to emit the lists — which it has to know anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FacetType {
    /// Two-node straight line: the edge of a [`ElementType::Tri3`] or
    /// [`ElementType::Quad4`]. Parametrised by `xi` in `[-1, 1]`.
    Line2,
    /// Three-node line: the edge of a [`ElementType::Tri6`], with node 2 the
    /// midside. Parametrised by `xi` in `[-1, 1]`.
    Line3,
    /// Three-node triangle: the face of a [`ElementType::Tet4`].
    Tri3,
    /// Four-node bilinear quadrilateral: the face of a [`ElementType::Hex8`].
    Quad4,
}

impl FacetType {
    /// Number of nodes on the facet (dimensionless count).
    #[must_use]
    pub fn n_nodes(self) -> usize {
        match self {
            FacetType::Line2 => 2,
            FacetType::Line3 | FacetType::Tri3 => 3,
            FacetType::Quad4 => 4,
        }
    }

    /// Parametric dimension of the facet: 1 for a line, 2 for a surface.
    #[must_use]
    pub fn dim(self) -> usize {
        match self {
            FacetType::Line2 | FacetType::Line3 => 1,
            FacetType::Tri3 | FacetType::Quad4 => 2,
        }
    }

    /// Shape-function values on the facet, dimensionless.
    ///
    /// Only the first [`n_nodes`](Self::n_nodes) entries are meaningful.
    #[must_use]
    pub fn shape_functions(self, xi: [f64; 3]) -> [f64; MAX_FACET_NODES] {
        let mut n = [0.0; MAX_FACET_NODES];
        match self {
            FacetType::Line2 => {
                n[0] = 0.5 * (1.0 - xi[0]);
                n[1] = 0.5 * (1.0 + xi[0]);
            }
            FacetType::Line3 => {
                let r = xi[0];
                n[0] = 0.5 * r * (r - 1.0);
                n[1] = 0.5 * r * (r + 1.0);
                n[2] = 1.0 - r * r;
            }
            FacetType::Tri3 => {
                n[0] = 1.0 - xi[0] - xi[1];
                n[1] = xi[0];
                n[2] = xi[1];
            }
            FacetType::Quad4 => {
                let (r, s) = (xi[0], xi[1]);
                n[0] = 0.25 * (1.0 - r) * (1.0 - s);
                n[1] = 0.25 * (1.0 + r) * (1.0 - s);
                n[2] = 0.25 * (1.0 + r) * (1.0 + s);
                n[3] = 0.25 * (1.0 - r) * (1.0 + s);
            }
        }
        n
    }

    /// Natural derivatives `dN_a/dxi_j` on the facet, dimensionless.
    #[must_use]
    pub fn shape_derivatives(self, xi: [f64; 3]) -> [[f64; 2]; MAX_FACET_NODES] {
        let mut d = [[0.0; 2]; MAX_FACET_NODES];
        match self {
            FacetType::Line2 => {
                d[0][0] = -0.5;
                d[1][0] = 0.5;
            }
            FacetType::Line3 => {
                let r = xi[0];
                d[0][0] = r - 0.5;
                d[1][0] = r + 0.5;
                d[2][0] = -2.0 * r;
            }
            FacetType::Tri3 => {
                d[0] = [-1.0, -1.0];
                d[1] = [1.0, 0.0];
                d[2] = [0.0, 1.0];
            }
            FacetType::Quad4 => {
                let (r, s) = (xi[0], xi[1]);
                let sg = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                for (a, g) in sg.iter().enumerate() {
                    d[a] = [
                        0.25 * g[0] * (1.0 + g[1] * s),
                        0.25 * g[1] * (1.0 + g[0] * r),
                    ];
                }
            }
        }
        d
    }

    /// Surface (or line) Jacobian at a facet natural coordinate.
    ///
    /// For a line facet this is `|dx/dxi|` in metres; for a surface facet it is
    /// `|dx/dxi cross dx/deta|` in square metres. Multiplying by the quadrature
    /// weight gives the physical measure, so a traction in pascals integrates
    /// to a force in newtons.
    ///
    /// # Arguments
    ///
    /// - `coords` — facet nodal coordinates in metres, at least
    ///   [`n_nodes`](Self::n_nodes) of them.
    /// - `xi` — facet natural coordinate, dimensionless.
    #[must_use]
    pub fn measure_jacobian(self, coords: &[[f64; 3]], xi: [f64; 3]) -> f64 {
        let d = self.shape_derivatives(xi);
        let nn = self.n_nodes();
        let mut t0 = [0.0_f64; 3];
        let mut t1 = [0.0_f64; 3];
        for (a, c) in coords.iter().enumerate().take(nn) {
            for i in 0..3 {
                t0[i] += d[a][0] * c[i];
                if self.dim() == 2 {
                    t1[i] += d[a][1] * c[i];
                }
            }
        }
        if self.dim() == 1 {
            (t0[0] * t0[0] + t0[1] * t0[1] + t0[2] * t0[2]).sqrt()
        } else {
            let cx = t0[1] * t1[2] - t0[2] * t1[1];
            let cy = t0[2] * t1[0] - t0[0] * t1[2];
            let cz = t0[0] * t1[1] - t0[1] * t1[0];
            (cx * cx + cy * cy + cz * cz).sqrt()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ElementType; 5] = [
        ElementType::Tri3,
        ElementType::Tri6,
        ElementType::Quad4,
        ElementType::Tet4,
        ElementType::Hex8,
    ];

    /// Sample points inside each reference domain.
    fn sample_points(e: ElementType) -> Vec<[f64; 3]> {
        match e {
            ElementType::Tri3 | ElementType::Tri6 => vec![
                [1.0 / 3.0, 1.0 / 3.0, 0.0],
                [0.1, 0.2, 0.0],
                [0.6, 0.3, 0.0],
                [0.0, 0.0, 0.0],
            ],
            ElementType::Quad4 => vec![
                [0.0, 0.0, 0.0],
                [-0.5, 0.3, 0.0],
                [0.9, -0.7, 0.0],
            ],
            ElementType::Tet4 => vec![
                [0.25, 0.25, 0.25],
                [0.1, 0.2, 0.3],
                [0.0, 0.0, 0.0],
            ],
            ElementType::Hex8 => vec![
                [0.0, 0.0, 0.0],
                [-0.5, 0.3, 0.7],
                [0.9, -0.7, -0.2],
            ],
        }
    }

    /// Shape functions must form a partition of unity, and their derivatives
    /// must sum to zero in every direction.
    #[test]
    fn partition_of_unity() {
        for e in ALL {
            for p in sample_points(e) {
                let n = e.shape_functions(p);
                let s: f64 = n.iter().take(e.n_nodes()).sum();
                assert!((s - 1.0).abs() < 1e-14, "{} sum {}", e.name(), s);
                let d = e.shape_derivatives(p);
                for k in 0..e.dim() {
                    let sd: f64 = (0..e.n_nodes()).map(|a| d[a][k]).sum();
                    assert!(sd.abs() < 1e-13, "{} d/dxi{} sum {}", e.name(), k, sd);
                }
            }
        }
    }

    /// Natural derivatives must match a central finite difference of the shape
    /// functions — the check that catches a sign slip in a hand-differentiated
    /// quadratic.
    #[test]
    fn derivatives_match_finite_difference() {
        let h = 1e-6;
        for e in ALL {
            for p in sample_points(e) {
                let d = e.shape_derivatives(p);
                for k in 0..e.dim() {
                    let mut pp = p;
                    let mut pm = p;
                    pp[k] += h;
                    pm[k] -= h;
                    let np = e.shape_functions(pp);
                    let nm = e.shape_functions(pm);
                    for a in 0..e.n_nodes() {
                        let fd = (np[a] - nm[a]) / (2.0 * h);
                        assert!(
                            (fd - d[a][k]).abs() < 1e-7,
                            "{} node {} dir {}: fd {} vs analytic {}",
                            e.name(),
                            a,
                            k,
                            fd,
                            d[a][k]
                        );
                    }
                }
            }
        }
    }

    /// A nodal shape function must be one at its own node and zero at the
    /// others (the Kronecker-delta property interpolation depends on).
    #[test]
    fn kronecker_delta_at_nodes() {
        let nodes: [(ElementType, Vec<[f64; 3]>); 5] = [
            (
                ElementType::Tri3,
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            ),
            (
                ElementType::Tri6,
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.5, 0.0, 0.0],
                    [0.5, 0.5, 0.0],
                    [0.0, 0.5, 0.0],
                ],
            ),
            (
                ElementType::Quad4,
                vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                ],
            ),
            (
                ElementType::Tet4,
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                ],
            ),
            (
                ElementType::Hex8,
                HEX8_NODE_SIGNS.iter().map(|s| [s[0], s[1], s[2]]).collect(),
            ),
        ];
        for (e, pts) in nodes {
            for (b, p) in pts.iter().enumerate() {
                let n = e.shape_functions(*p);
                for a in 0..e.n_nodes() {
                    let want = if a == b { 1.0 } else { 0.0 };
                    assert!(
                        (n[a] - want).abs() < 1e-13,
                        "{} N{} at node {} = {}",
                        e.name(),
                        a,
                        b,
                        n[a]
                    );
                }
            }
        }
    }

    /// A unit reference element mapped to itself has `det J` equal to the known
    /// reference measure factor, and a doubled element scales it as expected.
    #[test]
    fn jacobian_of_known_geometry() {
        // Quad4 on [0, 2] x [0, 3]: J = diag(1, 1.5), det = 1.5 m^2.
        let coords = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 3.0, 0.0],
            [0.0, 3.0, 0.0],
        ];
        let m = map_gradients(ElementType::Quad4, &coords, [0.0, 0.0, 0.0]).unwrap();
        assert!((m.det_j - 1.5).abs() < 1e-13);

        // Hex8 unit cube [0,1]^3: det J = 1/8 m^3.
        let mut hc = [[0.0; 3]; 8];
        for (a, s) in HEX8_NODE_SIGNS.iter().enumerate() {
            hc[a] = [
                0.5 * (1.0 + s[0]),
                0.5 * (1.0 + s[1]),
                0.5 * (1.0 + s[2]),
            ];
        }
        let mh = map_gradients(ElementType::Hex8, &hc, [0.0, 0.0, 0.0]).unwrap();
        assert!((mh.det_j - 0.125).abs() < 1e-13);
    }

    /// An inside-out element must be reported, not silently integrated.
    #[test]
    fn inverted_element_is_rejected() {
        let coords = [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ];
        let r = map_gradients(ElementType::Quad4, &coords, [0.0, 0.0, 0.0]);
        assert!(matches!(r, Err(FemError::DegenerateElement { .. })));
    }

    /// Facet measures: a line of length L gives `|dx/dxi| = L/2`; a unit square
    /// face gives `1/4` per unit parametric area.
    #[test]
    fn facet_measures() {
        let line = [[0.0, 0.0, 0.0], [3.0, 4.0, 0.0]];
        let j = FacetType::Line2.measure_jacobian(&line, [0.0, 0.0, 0.0]);
        assert!((j - 2.5).abs() < 1e-13, "line jacobian {}", j);

        let face = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        let jf = FacetType::Quad4.measure_jacobian(&face, [0.0, 0.0, 0.0]);
        assert!((jf - 1.0).abs() < 1e-13, "face jacobian {}", jf);
    }
}
