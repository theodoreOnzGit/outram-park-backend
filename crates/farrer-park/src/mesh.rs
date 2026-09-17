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

//! The finite-element mesh: nodes, connectivity, one element type, and the
//! structured generators the verification suite runs on.
//!
//! # What belongs in this module
//!
//! [`Mesh`] — a node coordinate list, a flat connectivity array and the single
//! [`ElementType`] every element in the mesh has — plus node/facet selection
//! helpers and the generators for the verification geometries: a unit square, a
//! unit cube, a quarter annulus and a rectangular beam block.
//!
//! # What does NOT belong here
//!
//! Degree-of-freedom numbering ([`crate::dof`]), fields, boundary-condition
//! *values* ([`crate::bc`]), and file I/O. A `Mesh` is geometry and topology
//! only.
//!
//! # One element type per mesh, on purpose
//!
//! A mixed-element mesh would need per-element type dispatch in the innermost
//! assembly loop and a ragged connectivity array. Nothing in the verification
//! suite needs it, and adding it before a consumer does would be speculative.
//! A mesh that needs two element types is two meshes today; if that becomes
//! wrong, the fix is a second connectivity block, not a `Box<dyn Element>`.
//!
//! # Index newtypes
//!
//! Topology is referenced by [`NodeId`], [`ElemId`] and (in [`crate::dof`])
//! `DofId`, never by references into the mesh. This is the workspace rule
//! against lifetime parameters in structs, and it is also what lets a `Mesh` be
//! shared with `Arc<Mesh>` and cloned freely.
//!
//! # Units
//!
//! All nodal coordinates are in metres. A 2-D mesh stores `z = 0`.

use std::sync::Arc;

use crate::element::{ElementType, FacetType};
use crate::error::{FemError, Result};

/// Index of a node in [`Mesh::coords`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// Index of an element in a [`Mesh`]'s connectivity array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElemId(pub usize);

/// A boundary facet on which a traction can be applied, given as an explicit
/// node list.
///
/// See [`crate::element::FacetType`] for why facets are node lists rather than
/// `(element, local face)` pairs.
#[derive(Debug, Clone, PartialEq)]
pub struct Facet {
    /// Which reference facet this is.
    pub facet_type: FacetType,
    /// Its nodes, in the facet's own local ordering. Length must equal
    /// `facet_type.n_nodes()`.
    pub nodes: Vec<NodeId>,
}

/// A finite-element mesh of a single element type.
///
/// # Units
///
/// Coordinates in metres.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// Spatial dimension, 2 or 3 (dimensionless). Equals
    /// `element_type.dim()`.
    dim: usize,
    /// The one element type every element in this mesh has.
    element_type: ElementType,
    /// Node coordinates in metres, `[x, y, z]`; `z = 0` throughout a 2-D mesh.
    coords: Vec<[f64; 3]>,
    /// Flat connectivity: element `e` owns
    /// `connectivity[e * n_nodes .. (e + 1) * n_nodes]`.
    connectivity: Vec<usize>,
}

impl Mesh {
    /// Build a mesh from a coordinate list and a flat connectivity array.
    ///
    /// # Arguments
    ///
    /// - `element_type` — the single element type of every element.
    /// - `coords` — node coordinates in metres. A 2-D mesh should carry
    ///   `z = 0`; this is not enforced, but a non-zero `z` is ignored by the
    ///   2-D mapping and will silently not do what the caller meant.
    /// - `connectivity` — `n_elements * element_type.n_nodes()` node indices.
    ///
    /// # Errors
    ///
    /// [`FemError::LengthMismatch`] if `connectivity.len()` is not a multiple
    /// of the nodes per element, and [`FemError::NodeOutOfRange`] if any index
    /// exceeds the node count.
    pub fn new(
        element_type: ElementType,
        coords: Vec<[f64; 3]>,
        connectivity: Vec<usize>,
    ) -> Result<Self> {
        let nn = element_type.n_nodes();
        if connectivity.len() % nn != 0 {
            return Err(FemError::LengthMismatch {
                context: "Mesh::new: connectivity length must be a multiple of nodes per element",
                expected: nn,
                actual: connectivity.len() % nn,
            });
        }
        let n_nodes = coords.len();
        for (i, &n) in connectivity.iter().enumerate() {
            if n >= n_nodes {
                return Err(FemError::NodeOutOfRange {
                    element: i / nn,
                    node: n,
                    n_nodes,
                });
            }
        }
        Ok(Self {
            dim: element_type.dim(),
            element_type,
            coords,
            connectivity,
        })
    }

    /// Spatial dimension, 2 or 3 (dimensionless).
    #[must_use]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// The element type of every element in this mesh.
    #[must_use]
    pub fn element_type(&self) -> ElementType {
        self.element_type
    }

    /// Number of nodes (dimensionless count).
    #[must_use]
    pub fn n_nodes(&self) -> usize {
        self.coords.len()
    }

    /// Number of elements (dimensionless count).
    #[must_use]
    pub fn n_elements(&self) -> usize {
        self.connectivity.len() / self.element_type.n_nodes()
    }

    /// All node coordinates in metres.
    #[must_use]
    pub fn coords(&self) -> &[[f64; 3]] {
        &self.coords
    }

    /// One node's coordinates in metres.
    #[must_use]
    pub fn node(&self, id: NodeId) -> [f64; 3] {
        self.coords[id.0]
    }

    /// The node indices of one element, in the element's local ordering.
    #[must_use]
    pub fn element_nodes(&self, e: ElemId) -> &[usize] {
        let nn = self.element_type.n_nodes();
        &self.connectivity[e.0 * nn..(e.0 + 1) * nn]
    }

    /// Gather one element's nodal coordinates in metres into `out`, which must
    /// have at least `element_type.n_nodes()` entries.
    pub fn element_coords(&self, e: ElemId, out: &mut [[f64; 3]]) {
        for (slot, &n) in out.iter_mut().zip(self.element_nodes(e)) {
            *slot = self.coords[n];
        }
    }

    /// Every node whose coordinates satisfy `predicate`.
    ///
    /// The predicate takes the coordinate in metres. Generic over `F` rather
    /// than taking a boxed closure, per the workspace rule against trait
    /// objects.
    ///
    /// Typical use is a geometric tolerance test, e.g.
    /// `mesh.nodes_where(|p| p[0].abs() < 1e-12)` for the `x = 0` plane. A
    /// tolerance is the caller's responsibility: exact float equality on a
    /// generated coordinate happens to work for these generators but is a trap
    /// in general.
    #[must_use]
    pub fn nodes_where<F: Fn([f64; 3]) -> bool>(&self, predicate: F) -> Vec<NodeId> {
        self.coords
            .iter()
            .enumerate()
            .filter(|(_, c)| predicate(**c))
            .map(|(i, _)| NodeId(i))
            .collect()
    }

    /// The nodes on the outer boundary of a structured generator's domain,
    /// found by a bounding-box test with tolerance `tol` metres.
    ///
    /// Only meaningful for the box-shaped generators ([`unit_square_tri3`],
    /// [`unit_square_quad4`], [`unit_cube_hex8`], ...). For the quarter
    /// annulus use [`Mesh::nodes_where`] with a radius test.
    #[must_use]
    pub fn bounding_box_boundary_nodes(&self, tol: f64) -> Vec<NodeId> {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for c in &self.coords {
            for i in 0..self.dim {
                lo[i] = lo[i].min(c[i]);
                hi[i] = hi[i].max(c[i]);
            }
        }
        self.coords
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                (0..self.dim).any(|i| (c[i] - lo[i]).abs() < tol || (c[i] - hi[i]).abs() < tol)
            })
            .map(|(i, _)| NodeId(i))
            .collect()
    }

    /// A copy of this mesh with every node coordinate passed through `f`.
    ///
    /// `f(node_index, coordinate_in_metres) -> new_coordinate_in_metres`. The
    /// connectivity is unchanged, so the caller is responsible for not turning
    /// an element inside out; the result is re-validated for node indices but
    /// **not** for positive Jacobians, which only [`crate::assembly`] can check
    /// at the quadrature points it actually uses.
    ///
    /// This exists for the patch test, which needs an *irregular* mesh: a patch
    /// test on a uniform grid is much weaker, because a uniform grid passes it
    /// for reasons that have nothing to do with the element being correct.
    ///
    /// # Errors
    ///
    /// As [`Mesh::new`].
    pub fn map_coords<F: Fn(usize, [f64; 3]) -> [f64; 3]>(&self, f: F) -> Result<Mesh> {
        let coords: Vec<[f64; 3]> = self
            .coords
            .iter()
            .enumerate()
            .map(|(i, c)| f(i, *c))
            .collect();
        Mesh::new(self.element_type, coords, self.connectivity.clone())
    }

    /// Share this mesh immutably across the assembly and solver layers.
    ///
    /// The mesh is read-only after construction, so it is shared with `Arc<T>`
    /// and never locked — the workspace rule for data that does not mutate.
    #[must_use]
    pub fn shared(self) -> Arc<Mesh> {
        Arc::new(self)
    }
}

// ── structured generators ───────────────────────────────────────────────────

/// Check a positive integer division count.
fn check_div(name: &'static str, n: usize) -> Result<()> {
    if n == 0 {
        return Err(FemError::InvalidMeshParameter {
            parameter: name,
            value: 0.0,
            reason: "must be at least 1",
        });
    }
    Ok(())
}

/// Check a strictly positive length.
fn check_len(name: &'static str, v: f64) -> Result<()> {
    if !(v > 0.0) || !v.is_finite() {
        return Err(FemError::InvalidMeshParameter {
            parameter: name,
            value: v,
            reason: "must be a finite positive length in metres",
        });
    }
    Ok(())
}

/// Structured [`ElementType::Quad4`] mesh of the rectangle
/// `[0, lx] x [0, ly]` with `nx` by `ny` elements.
///
/// Nodes are numbered lexicographically, `x` fastest: node `(i, j)` is
/// `j * (nx + 1) + i`. Element node ordering is counter-clockwise, so `det J`
/// is positive.
///
/// # Arguments
///
/// - `lx`, `ly` — side lengths in metres, strictly positive.
/// - `nx`, `ny` — element divisions, at least 1.
///
/// # Errors
///
/// [`FemError::InvalidMeshParameter`] for a non-positive side or a zero
/// division count.
pub fn rectangle_quad4(lx: f64, ly: f64, nx: usize, ny: usize) -> Result<Mesh> {
    check_len("lx", lx)?;
    check_len("ly", ly)?;
    check_div("nx", nx)?;
    check_div("ny", ny)?;
    let mut coords = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            coords.push([
                lx * i as f64 / nx as f64,
                ly * j as f64 / ny as f64,
                0.0,
            ]);
        }
    }
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut conn = Vec::with_capacity(4 * nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[
                idx(i, j),
                idx(i + 1, j),
                idx(i + 1, j + 1),
                idx(i, j + 1),
            ]);
        }
    }
    Mesh::new(ElementType::Quad4, coords, conn)
}

/// Structured [`ElementType::Quad4`] mesh of the unit square.
///
/// Shorthand for `rectangle_quad4(1.0, 1.0, n, n)`. Side lengths are one metre.
pub fn unit_square_quad4(n: usize) -> Result<Mesh> {
    rectangle_quad4(1.0, 1.0, n, n)
}

/// Structured [`ElementType::Tri3`] mesh of the rectangle `[0, lx] x [0, ly]`.
///
/// Each structured quad cell is split along its `(i, j)-(i+1, j+1)` diagonal
/// into two triangles, giving `2 nx ny` elements. Node numbering is identical
/// to [`rectangle_quad4`].
pub fn rectangle_tri3(lx: f64, ly: f64, nx: usize, ny: usize) -> Result<Mesh> {
    let q = rectangle_quad4(lx, ly, nx, ny)?;
    let mut conn = Vec::with_capacity(6 * nx * ny);
    for e in 0..q.n_elements() {
        let n = q.element_nodes(ElemId(e));
        conn.extend_from_slice(&[n[0], n[1], n[2]]);
        conn.extend_from_slice(&[n[0], n[2], n[3]]);
    }
    Mesh::new(ElementType::Tri3, q.coords().to_vec(), conn)
}

/// Structured [`ElementType::Tri3`] mesh of the unit square, one metre a side.
pub fn unit_square_tri3(n: usize) -> Result<Mesh> {
    rectangle_tri3(1.0, 1.0, n, n)
}

/// Promote a [`ElementType::Tri3`] mesh to [`ElementType::Tri6`] by inserting a
/// node at the midpoint of every edge.
///
/// The resulting quadratic triangles are **straight-sided**: each midside node
/// is placed exactly at the arithmetic midpoint of its edge, so the Jacobian is
/// constant over the element. Node ordering is corners 0-2 then midsides of
/// edges 0-1, 1-2, 2-0, matching [`ElementType::Tri6`].
///
/// This is the only supported way to build a Tri6 mesh, which keeps the
/// straight-sidedness guarantee in one place.
///
/// # Errors
///
/// [`FemError::LengthMismatch`] if `tri3` is not a Tri3 mesh.
pub fn tri3_to_tri6(tri3: &Mesh) -> Result<Mesh> {
    if tri3.element_type() != ElementType::Tri3 {
        return Err(FemError::LengthMismatch {
            context: "tri3_to_tri6: input mesh is not Tri3",
            expected: 3,
            actual: tri3.element_type().n_nodes(),
        });
    }
    let mut coords = tri3.coords().to_vec();
    let mut edge_node: std::collections::HashMap<(usize, usize), usize> =
        std::collections::HashMap::new();
    let mut conn = Vec::with_capacity(6 * tri3.n_elements());
    for e in 0..tri3.n_elements() {
        let n: Vec<usize> = tri3.element_nodes(ElemId(e)).to_vec();
        let mut mids = [0usize; 3];
        for (k, (a, b)) in [(n[0], n[1]), (n[1], n[2]), (n[2], n[0])].iter().enumerate() {
            let key = if a < b { (*a, *b) } else { (*b, *a) };
            let id = *edge_node.entry(key).or_insert_with(|| {
                let pa = coords[*a];
                let pb = coords[*b];
                coords.push([
                    0.5 * (pa[0] + pb[0]),
                    0.5 * (pa[1] + pb[1]),
                    0.5 * (pa[2] + pb[2]),
                ]);
                coords.len() - 1
            });
            mids[k] = id;
        }
        conn.extend_from_slice(&[n[0], n[1], n[2], mids[0], mids[1], mids[2]]);
    }
    Mesh::new(ElementType::Tri6, coords, conn)
}

/// Structured [`ElementType::Tri6`] mesh of the rectangle `[0, lx] x [0, ly]`.
///
/// Built by [`rectangle_tri3`] followed by [`tri3_to_tri6`].
pub fn rectangle_tri6(lx: f64, ly: f64, nx: usize, ny: usize) -> Result<Mesh> {
    tri3_to_tri6(&rectangle_tri3(lx, ly, nx, ny)?)
}

/// Structured [`ElementType::Tri6`] mesh of the unit square, one metre a side.
pub fn unit_square_tri6(n: usize) -> Result<Mesh> {
    rectangle_tri6(1.0, 1.0, n, n)
}

/// Structured [`ElementType::Hex8`] mesh of the box
/// `[0, lx] x [0, ly] x [0, lz]` with `nx` by `ny` by `nz` elements.
///
/// Node `(i, j, k)` is `(k * (ny + 1) + j) * (nx + 1) + i`. Element node
/// ordering follows [`ElementType::Hex8`] so `det J` is positive.
pub fn box_hex8(
    lx: f64,
    ly: f64,
    lz: f64,
    nx: usize,
    ny: usize,
    nz: usize,
) -> Result<Mesh> {
    check_len("lx", lx)?;
    check_len("ly", ly)?;
    check_len("lz", lz)?;
    check_div("nx", nx)?;
    check_div("ny", ny)?;
    check_div("nz", nz)?;
    let mut coords = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                coords.push([
                    lx * i as f64 / nx as f64,
                    ly * j as f64 / ny as f64,
                    lz * k as f64 / nz as f64,
                ]);
            }
        }
    }
    let idx = |i: usize, j: usize, k: usize| (k * (ny + 1) + j) * (nx + 1) + i;
    let mut conn = Vec::with_capacity(8 * nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                conn.extend_from_slice(&[
                    idx(i, j, k),
                    idx(i + 1, j, k),
                    idx(i + 1, j + 1, k),
                    idx(i, j + 1, k),
                    idx(i, j, k + 1),
                    idx(i + 1, j, k + 1),
                    idx(i + 1, j + 1, k + 1),
                    idx(i, j + 1, k + 1),
                ]);
            }
        }
    }
    Mesh::new(ElementType::Hex8, coords, conn)
}

/// Structured [`ElementType::Hex8`] mesh of the unit cube, one metre a side.
pub fn unit_cube_hex8(n: usize) -> Result<Mesh> {
    box_hex8(1.0, 1.0, 1.0, n, n, n)
}

/// Structured [`ElementType::Tet4`] mesh of the box `[0, lx] x [0, ly] x
/// [0, lz]`, six tetrahedra per structured cell.
///
/// Uses the standard Kuhn (six-tetrahedron) subdivision of the cube, which
/// produces positively oriented tetrahedra and tiles conformingly across cell
/// boundaries.
pub fn box_tet4(lx: f64, ly: f64, lz: f64, nx: usize, ny: usize, nz: usize) -> Result<Mesh> {
    let h = box_hex8(lx, ly, lz, nx, ny, nz)?;
    // Kuhn subdivision: the six permutations of walking the cube's edges from
    // corner 0 to corner 6 in local Hex8 numbering.
    const KUHN: [[usize; 4]; 6] = [
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
        [0, 5, 1, 6],
    ];
    let mut conn = Vec::with_capacity(4 * 6 * h.n_elements());
    for e in 0..h.n_elements() {
        let n = h.element_nodes(ElemId(e)).to_vec();
        for t in KUHN {
            conn.extend_from_slice(&[n[t[0]], n[t[1]], n[t[2]], n[t[3]]]);
        }
    }
    Mesh::new(ElementType::Tet4, h.coords().to_vec(), conn)
}

/// Structured [`ElementType::Tet4`] mesh of the unit cube, one metre a side.
pub fn unit_cube_tet4(n: usize) -> Result<Mesh> {
    box_tet4(1.0, 1.0, 1.0, n, n, n)
}

/// Structured [`ElementType::Quad4`] mesh of a quarter annulus, for the
/// thick-walled-cylinder verification case.
///
/// The domain is `r` in `[r_inner, r_outer]`, `theta` in `[0, pi/2]`, meshed
/// with `n_r` radial by `n_theta` circumferential elements. Node `(i, j)` is at
/// radius `r_inner + i (r_outer - r_inner) / n_r` and angle
/// `j (pi/2) / n_theta`, numbered `j * (n_r + 1) + i`.
///
/// # The geometry is faceted, and that matters
///
/// The elements are straight-sided, so the curved inner and outer surfaces are
/// approximated by chords. The chord of an arc subtending `d_theta` sits at a
/// mean radius `r (1 - d_theta^2 / 24)` — a systematic `O(h^2)` geometric
/// error, *in addition to* the discretisation error, and always in the same
/// direction. The Lame comparison reports it rather than meshing it away.
///
/// # Arguments
///
/// - `r_inner`, `r_outer` — radii in metres, `0 < r_inner < r_outer`.
/// - `n_r`, `n_theta` — element divisions, at least 1.
///
/// # Errors
///
/// [`FemError::InvalidMeshParameter`] if the radii are not ordered and
/// positive, or a division count is zero.
pub fn quarter_annulus_quad4(
    r_inner: f64,
    r_outer: f64,
    n_r: usize,
    n_theta: usize,
) -> Result<Mesh> {
    check_len("r_inner", r_inner)?;
    check_len("r_outer", r_outer)?;
    check_div("n_r", n_r)?;
    check_div("n_theta", n_theta)?;
    if r_outer <= r_inner {
        return Err(FemError::InvalidMeshParameter {
            parameter: "r_outer",
            value: r_outer,
            reason: "must exceed r_inner",
        });
    }
    let mut coords = Vec::with_capacity((n_r + 1) * (n_theta + 1));
    for j in 0..=n_theta {
        let th = std::f64::consts::FRAC_PI_2 * j as f64 / n_theta as f64;
        for i in 0..=n_r {
            let r = r_inner + (r_outer - r_inner) * i as f64 / n_r as f64;
            coords.push([r * th.cos(), r * th.sin(), 0.0]);
        }
    }
    let idx = |i: usize, j: usize| j * (n_r + 1) + i;
    let mut conn = Vec::with_capacity(4 * n_r * n_theta);
    for j in 0..n_theta {
        for i in 0..n_r {
            conn.extend_from_slice(&[
                idx(i, j),
                idx(i + 1, j),
                idx(i + 1, j + 1),
                idx(i, j + 1),
            ]);
        }
    }
    Mesh::new(ElementType::Quad4, coords, conn)
}

/// The [`FacetType::Line2`] facets on the inner radius of a mesh produced by
/// [`quarter_annulus_quad4`] with the same `n_r`, `n_theta`.
///
/// Node ordering within each facet runs in increasing `theta`, so the facet's
/// outward normal (pointing towards decreasing `r`) is consistent along the
/// arc. Used to apply the internal pressure.
#[must_use]
pub fn quarter_annulus_inner_facets(n_r: usize, n_theta: usize) -> Vec<Facet> {
    let idx = |i: usize, j: usize| j * (n_r + 1) + i;
    (0..n_theta)
        .map(|j| Facet {
            facet_type: FacetType::Line2,
            nodes: vec![NodeId(idx(0, j)), NodeId(idx(0, j + 1))],
        })
        .collect()
}

/// The [`FacetType::Line2`] facets on the `x = lx` edge of a mesh produced by
/// [`rectangle_quad4`], used to load the cantilever tip.
#[must_use]
pub fn rectangle_right_edge_facets(nx: usize, ny: usize) -> Vec<Facet> {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    (0..ny)
        .map(|j| Facet {
            facet_type: FacetType::Line2,
            nodes: vec![NodeId(idx(nx, j)), NodeId(idx(nx, j + 1))],
        })
        .collect()
}

/// The [`FacetType::Line3`] facets on the `x = lx` edge of a mesh produced by
/// [`rectangle_tri6`], with the midside node found by coordinate match.
///
/// # Arguments
///
/// - `mesh` — the Tri6 mesh.
/// - `lx` — the `x` coordinate of the edge in metres.
/// - `tol` — geometric tolerance in metres.
///
/// Facet node ordering is `[end, end, midside]`, matching
/// [`FacetType::Line3`].
#[must_use]
pub fn tri6_edge_facets_at_x(mesh: &Mesh, lx: f64, tol: f64) -> Vec<Facet> {
    // Collect the nodes on the line, sorted by y, then pair them up: on a
    // straight-sided Tri6 mesh the edge carries corner and midside nodes
    // alternately once sorted.
    let mut on: Vec<(f64, usize)> = mesh
        .coords()
        .iter()
        .enumerate()
        .filter(|(_, c)| (c[0] - lx).abs() < tol)
        .map(|(i, c)| (c[1], i))
        .collect();
    on.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut facets = Vec::new();
    let mut k = 0;
    while k + 2 < on.len() {
        facets.push(Facet {
            facet_type: FacetType::Line3,
            nodes: vec![NodeId(on[k].1), NodeId(on[k + 2].1), NodeId(on[k + 1].1)],
        });
        k += 2;
    }
    facets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::map_gradients;
    use crate::quadrature::default_rule;

    /// Every element of every generator must have a strictly positive Jacobian
    /// at every quadrature point — the check that a connectivity ordering
    /// mistake would fail.
    #[test]
    fn all_generators_produce_positively_oriented_elements() {
        let meshes = vec![
            unit_square_quad4(3).unwrap(),
            unit_square_tri3(3).unwrap(),
            unit_square_tri6(3).unwrap(),
            unit_cube_hex8(2).unwrap(),
            unit_cube_tet4(2).unwrap(),
            quarter_annulus_quad4(0.5, 1.0, 3, 5).unwrap(),
            box_hex8(4.0, 1.0, 1.0, 4, 2, 2).unwrap(),
        ];
        for m in meshes {
            let rule = default_rule(m.element_type());
            let mut ec = [[0.0; 3]; 8];
            for e in 0..m.n_elements() {
                m.element_coords(ElemId(e), &mut ec);
                for q in &rule {
                    let mp = map_gradients(m.element_type(), &ec, q.xi).unwrap_or_else(|err| {
                        panic!("{:?} element {} at {:?}: {}", m.element_type(), e, q.xi, err)
                    });
                    assert!(mp.det_j > 0.0);
                }
            }
        }
    }

    /// Summed element volumes must equal the analytic domain measure — a check
    /// on connectivity, quadrature and Jacobian together.
    #[test]
    fn measured_volumes_match_analytic() {
        fn measure(m: &Mesh) -> f64 {
            let rule = default_rule(m.element_type());
            let mut ec = [[0.0; 3]; 8];
            let mut v = 0.0;
            for e in 0..m.n_elements() {
                m.element_coords(ElemId(e), &mut ec);
                for q in &rule {
                    v += q.weight * map_gradients(m.element_type(), &ec, q.xi).unwrap().det_j;
                }
            }
            v
        }
        assert!((measure(&unit_square_quad4(4).unwrap()) - 1.0).abs() < 1e-12);
        assert!((measure(&unit_square_tri3(4).unwrap()) - 1.0).abs() < 1e-12);
        assert!((measure(&unit_square_tri6(4).unwrap()) - 1.0).abs() < 1e-12);
        assert!((measure(&unit_cube_hex8(3).unwrap()) - 1.0).abs() < 1e-12);
        assert!((measure(&unit_cube_tet4(3).unwrap()) - 1.0).abs() < 1e-12);
        assert!((measure(&box_hex8(4.0, 0.5, 0.25, 4, 2, 1).unwrap()) - 0.5).abs() < 1e-12);
    }

    /// The quarter-annulus area converges to `pi (b^2 - a^2) / 4` from below,
    /// at the second order the faceting predicts. This is the geometric error
    /// the Lame case has to live with, measured rather than assumed.
    #[test]
    fn annulus_area_converges_second_order() {
        let (a, b) = (0.5, 1.0);
        let exact = std::f64::consts::FRAC_PI_4 * (b * b - a * a);
        let mut prev_err = f64::INFINITY;
        for nt in [4usize, 8, 16] {
            let m = quarter_annulus_quad4(a, b, 4, nt).unwrap();
            let rule = default_rule(ElementType::Quad4);
            let mut ec = [[0.0; 3]; 8];
            let mut v = 0.0;
            for e in 0..m.n_elements() {
                m.element_coords(ElemId(e), &mut ec);
                for q in &rule {
                    v += q.weight * map_gradients(ElementType::Quad4, &ec, q.xi).unwrap().det_j;
                }
            }
            let err = (exact - v).abs();
            assert!(v < exact, "faceted area must under-estimate: {} vs {}", v, exact);
            if prev_err.is_finite() {
                let ratio = prev_err / err;
                assert!(ratio > 3.5 && ratio < 4.5, "area error ratio {}", ratio);
            }
            prev_err = err;
        }
    }

    /// Tri6 promotion must place every midside node exactly at the edge
    /// midpoint, and must share midside nodes between neighbouring triangles.
    #[test]
    fn tri6_midsides_are_shared_and_exact() {
        let m = unit_square_tri6(2).unwrap();
        // 3x3 corner grid = 9 corners; 8 triangles; edges = 9 + 8 - 1 = 16
        // by Euler, so 9 + 16 = 25 nodes.
        assert_eq!(m.n_nodes(), 25);
        for e in 0..m.n_elements() {
            let n = m.element_nodes(ElemId(e));
            let c: Vec<[f64; 3]> = n.iter().map(|&i| m.coords()[i]).collect();
            for (k, (a, b)) in [(0, 1), (1, 2), (2, 0)].iter().enumerate() {
                let mid = c[3 + k];
                for d in 0..2 {
                    assert!((mid[d] - 0.5 * (c[*a][d] + c[*b][d])).abs() < 1e-15);
                }
            }
        }
    }

    /// A wrong node index must be caught at construction.
    #[test]
    fn out_of_range_node_is_rejected() {
        let r = Mesh::new(
            ElementType::Tri3,
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![0, 1, 7],
        );
        assert!(matches!(r, Err(FemError::NodeOutOfRange { .. })));
    }

    /// Inner-annulus facets must cover the inner arc exactly once.
    #[test]
    fn annulus_inner_facets_cover_the_arc() {
        let (nr, nt) = (3usize, 6usize);
        let m = quarter_annulus_quad4(0.5, 1.0, nr, nt).unwrap();
        let facets = quarter_annulus_inner_facets(nr, nt);
        assert_eq!(facets.len(), nt);
        let mut total = 0.0;
        for f in &facets {
            let c: Vec<[f64; 3]> = f.nodes.iter().map(|n| m.node(*n)).collect();
            let d = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
            total += (d[0] * d[0] + d[1] * d[1]).sqrt();
            for p in &c {
                let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
                assert!((r - 0.5).abs() < 1e-12);
            }
        }
        // Chord total is slightly under the true arc length pi a / 2.
        let arc = std::f64::consts::FRAC_PI_2 * 0.5;
        assert!(total < arc && total > 0.99 * arc, "chords {} arc {}", total, arc);
    }
}
