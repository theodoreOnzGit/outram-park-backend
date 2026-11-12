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

//! Translation of LIGGGHTS' triangle-mesh wall contact path (gh:#1416, gh:#1406).
//!
//! # Upstream provenance
//!
//! Translated, operation by operation, from LIGGGHTS-PUBLIC commit
//! `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` (GPLv3, DCS Computing GmbH /
//! JKU Linz):
//!
//! | this module | upstream |
//! |---|---|
//! | [`TriMeshState::resolve_tri_sphere_contact_bary`] and its corner / edge / face helpers | `src/tri_mesh_I.h`: `TriMesh::resolveTriSphereContactBary`, `resolveCornerContactBary`, `resolveEdgeContactBary`, `resolveFaceContactBary`, `calcDist` |
//! | [`TriMeshState::calc_bary_tri_coords`] | `src/math_extra_liggghts.h`: `calcBaryTriCoords` |
//! | [`TriMeshState::refresh_from_nodes`] | `src/surface_mesh_I.h`: `recalcLocalSurfProperties`, `calcEdgeVecLen`, `calcSurfaceNorm`, `calcEdgeNormals`, `calcObtuseAngleIndex` |
//! | [`TriMeshState::build_topology`] | `src/surface_mesh_I.h`: `buildNeighbours`, `shareEdge`, `handleSharedEdge`, `coplanarNeighsOverlap`, `handleCorner`, `checkNodeRecursive`, `areCoplanar`; `src/multi_node_mesh_I.h`: `share2Nodes`, `nodesAreEqual`, `containsNode` |
//! | [`TriMeshState::rotate_incremental`] | `src/multi_node_mesh_I.h`: `MultiNodeMesh::rotate(dAngle, axis, p)`; `src/general_container_I.h`: `GeneralContainer::rotate`; `src/math_extra_liggghts.h`: `vec_quat_rotate`; `src/math_extra.h`: `quatquat` |
//! | [`TriMeshState::are_coplanar_node_neighs`] | `src/surface_mesh_I.h`: `areCoplanarNodeNeighs` |
//!
//! # Fidelity notes (what is copied, including upstream oddities)
//!
//! * Every arithmetic expression keeps upstream's operation order, because the
//!   differential tests (`tests/rotating_mesh_cross_code.rs`) compare IEEE-754
//!   bit patterns. Division by a length is **multiplication by its reciprocal**
//!   (`vectorScalarDiv3D`, `vectorNormalize3D`).
//! * `edgeVec`, `edgeNorm` and `surfaceNorm` are **rotated incrementally** by
//!   the step quaternion between neighbour-list rebuilds and **recomputed from
//!   the nodes** on a rebuild step (`FixMesh::pre_force` → `pbcExchangeBorders`
//!   → `refreshOwned` → `recalcLocalSurfProperties`); `edgeLen` and
//!   `obtuseAngleIndex` change only at a rebuild. The rebuild schedule is the
//!   caller's ([`crate::granular_system::GranularSystem::with_neighbor_skin`]).
//! * **Upstream oddity, copied:** `calcObtuseAngleIndex` is called for
//!   `j = 0, 1, 2` and each call *overwrites* the stored index (with `NO_OBTUSE`
//!   when `dot <= 0`), so the stored value reflects node 2 only.
//! * **Not translated:** the `is_in_subdomain` test that skips corner activation
//!   for a node outside the simulation box (this port has no box; every node is
//!   treated as inside); `NUM_NEIGH_MAX` overflow handling; the mesh-element
//!   exclusion list; the order in which a non-manifold edge shared by more than
//!   two facets is processed (upstream's order follows a spatial bin list).
//!
//! No physics here depends on a tuned number; every threshold is upstream's
//! constant (`EPSILON_PRECISION = 1e-8`, `EPSILON_CURVATURE = 1e-5`,
//! `SMALL_TRIMESH = 1e-10`, `LARGE_TRIMESH = 1e6`).

use petir::mathf::RealMath;
use crate::particle::Vec3;

/// A plain 3-vector, as upstream's `double[3]`.
pub type V3 = [f64; 3];

/// Upstream `MultiNodeMesh::precision_` default (`EPSILON_PRECISION`).
const PRECISION: f64 = 1e-8;
/// Upstream `SurfaceMesh::curvature_` default, `1 - EPSILON_CURVATURE`.
const CURVATURE: f64 = 1. - 0.00001;
/// Upstream `SMALL_TRIMESH`.
const SMALL_TRIMESH: f64 = 1.0e-10;
/// Upstream `LARGE_TRIMESH`.
const LARGE_TRIMESH: f64 = 1000000.0;

// ---- vector_liggghts.h, operation for operation ---------------------------

#[inline]
fn v_sub(a: &V3, b: &V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
fn v_add(a: &V3, b: &V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline]
fn v_dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn v_cross(a: &V3, b: &V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
fn v_mag(a: &V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
#[inline]
fn v_mag_sq(a: &V3) -> f64 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}
/// `vectorScalarDiv3D`: multiply by the reciprocal.
#[inline]
fn v_div(v: &V3, s: f64) -> V3 {
    let sinv = 1. / s;
    [sinv * v[0], sinv * v[1], sinv * v[2]]
}
/// `vectorScalarMult3D(v, s)`.
#[inline]
fn v_mul(v: &V3, s: f64) -> V3 {
    [s * v[0], s * v[1], s * v[2]]
}
/// `vectorAddMultiple3D(v1, f, v2)`.
#[inline]
fn v_add_mult(v1: &V3, f: f64, v2: &V3) -> V3 {
    [v1[0] + f * v2[0], v1[1] + f * v2[1], v1[2] + f * v2[2]]
}
/// `pointDistance`.
#[inline]
fn point_distance(p1: &V3, p2: &V3) -> f64 {
    ((p1[0] - p2[0]) * (p1[0] - p2[0])
        + (p1[1] - p2[1]) * (p1[1] - p2[1])
        + (p1[2] - p2[2]) * (p1[2] - p2[2]))
        .sqrt()
}
/// `MathExtraLiggghts::compDouble(a, b, prec)`.
#[inline]
fn comp_double(a: f64, b: f64, prec: f64) -> bool {
    if a == b {
        return true;
    }
    if b == 0. {
        return a < prec && a > -prec;
    }
    let x = a - b;
    x < prec && x > -prec
}

/// `MathExtra::quatquat(a, b, c)`.
#[inline]
fn quat_quat(a: &[f64; 4], b: &[f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + b[0] * a[1] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] + b[0] * a[2] + a[3] * b[1] - a[1] * b[3],
        a[0] * b[3] + b[0] * a[3] + a[1] * b[2] - a[2] * b[1],
    ]
}

/// `MathExtraLiggghts::vec_quat_rotate(vec, quat)`: rotate `vec` by the unit
/// quaternion `quat` as `q (0, v) q*`.
#[inline]
pub fn vec_quat_rotate(vec: &V3, quat: &[f64; 4]) -> V3 {
    let vec_q = [0., vec[0], vec[1], vec[2]];
    let quat_c = [quat[0], -quat[1], -quat[2], -quat[3]];
    let temp = quat_quat(quat, &vec_q);
    let result_q = quat_quat(&temp, &quat_c);
    [result_q[1], result_q[2], result_q[3]]
}

/// Convert a [`Vec3`] to upstream's array form.
#[inline]
#[must_use]
pub fn arr(v: Vec3) -> V3 {
    [v.x, v.y, v.z]
}

/// Convert an array back to a [`Vec3`].
#[inline]
#[must_use]
pub fn vec3(v: V3) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

/// Triangle nodes as upstream stores them, `node[iTri][iNode][xyz]`.
pub type Nodes = [V3; 3];

/// Per-triangle derived geometry: upstream's `edgeVec`, `edgeLen`,
/// `surfaceNorm`, `obtuseAngleIndex` element properties.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TriGeo {
    edge_vec: [V3; 3],
    edge_len: [f64; 3],
    surf_norm: V3,
    /// `obtuseAngleIndex`; `None` is upstream's `NO_OBTUSE_ANGLE`.
    obtuse: Option<usize>,
}

/// `TriMeshPlanar` element properties `nearestActiveEdgeID`,
/// `nearestActiveEdgeIndex` and `minActiveEdgeDist` (`-1` = unused slot).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanarEdgeLists {
    /// Facet that owns the stored edge (upstream stores the facet ID, which is
    /// the local index on a serial, non-parallel mesh).
    pub edge_facet: Vec<[i32; 6]>,
    /// Edge number within that facet.
    pub edge_index: Vec<[i32; 6]>,
    /// Smallest stored distance per facet `[m]`.
    pub min_dist: Vec<f64>,
}

/// Everything the upstream mesh object holds beside the nodes themselves.
///
/// The nodes live in [`crate::mesh_wall::MeshWall::triangles`]; every method
/// that needs them takes them as a parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TriMeshState {
    geo: Vec<TriGeo>,
    /// Upstream `rBound_`: set once from the original nodes, never refreshed.
    r_bound: Vec<f64>,
    /// Upstream `edgeActive_`.
    edge_active: Vec<[bool; 3]>,
    /// Upstream `cornerActive_`.
    corner_active: Vec<[bool; 3]>,
    /// Facets sharing at least one node with facet `i` (the relation
    /// `areCoplanarNodeNeighs` tests; includes the edge neighbours).
    node_neighs: Vec<Vec<u32>>,
    /// Per-node velocity `[m/s]`, upstream element property `v`. Zero until the
    /// first mover `initial_integrate`.
    pub(crate) node_velocity: Vec<[V3; 3]>,
    /// Node positions at the last neighbour-list rebuild (`nodesLastRe_`).
    nodes_last_rebuild: Vec<Nodes>,
}

impl TriMeshState {
    /// Build the full state from the original nodes: derived geometry,
    /// `rBound`, mesh topology (edge and corner activity), zero node velocity.
    #[must_use]
    pub fn new(nodes: &[Nodes]) -> Self {
        let n = nodes.len();
        let mut s = Self {
            geo: Vec::new(),
            r_bound: Vec::with_capacity(n),
            edge_active: vec![[true; 3]; n],
            corner_active: vec![[true; 3]; n],
            node_neighs: vec![Vec::new(); n],
            node_velocity: vec![[[0.; 3]; 3]; n],
            nodes_last_rebuild: nodes.to_vec(),
        };
        // addElement: centre = sum / NUM_NODES (via the reciprocal), rBound.
        for nd in nodes {
            let mut avg = [0.; 3];
            for node in nd {
                avg = v_add(node, &avg);
            }
            let avg = v_div(&avg, 3.0);
            let mut rb = 0.0_f64;
            for node in nd {
                let vec = v_sub(&avg, node);
                rb = rb.max(v_mag(&vec));
            }
            s.r_bound.push(rb);
        }
        s.refresh_from_nodes(nodes);
        s.build_topology(nodes);
        s
    }

    /// Number of facets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.geo.len()
    }

    /// Whether there are no facets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.geo.is_empty()
    }

    // -------- recalcLocalSurfProperties ------------------------------------

    /// `calcEdgeVecLen`.
    fn calc_edge_vec_len(nd: &Nodes) -> ([V3; 3], [f64; 3]) {
        let mut vec = [[0.; 3]; 3];
        let mut len = [0.; 3];
        for i in 0..3 {
            vec[i] = v_sub(&nd[(i + 1) % 3], &nd[i]);
            len[i] = v_mag(&vec[i]);
            vec[i] = v_div(&vec[i], len[i]);
        }
        (vec, len)
    }

    /// `calcSurfaceNorm`, including the fall-back chain for a degenerate cross
    /// product.
    fn calc_surface_norm(edge_vec: &[V3; 3]) -> V3 {
        let mut n = v_cross(&edge_vec[0], &edge_vec[1]);
        if v_mag(&n) < 1e-15 {
            n = v_cross(&edge_vec[1], &edge_vec[2]);
            if v_mag(&n) < 1e-15 {
                n = v_cross(&edge_vec[2], &edge_vec[0]);
                if v_mag(&n) < 1e-15 {
                    let tmpvec = [1.1233, 2.123231, -3.3343434];
                    n = v_cross(&edge_vec[0], &tmpvec);
                    if v_mag(&n) < 1e-15 {
                        n = v_cross(&edge_vec[1], &tmpvec);
                        if v_mag(&n) < 1e-15 {
                            let tmpvec2 = [1.1233, -2.123231, 3.3343434];
                            n = v_cross(&edge_vec[0], &tmpvec2);
                            if v_mag(&n) < 1e-15 {
                                n = v_cross(&edge_vec[1], &tmpvec2);
                            }
                        }
                    }
                }
            }
        }
        let m = v_mag(&n);
        v_div(&n, m)
    }

    /// `calcEdgeNormals`.
    fn calc_edge_normals(edge_vec: &[V3; 3], surf_norm: &V3) -> [V3; 3] {
        let mut out = [[0.; 3]; 3];
        for i in 0..3 {
            out[i] = v_cross(&edge_vec[i], surf_norm);
            if v_mag(&out[i]) < 1e-15 {
                let other = (i + 1) % 3;
                out[i] = edge_vec[other];
            } else {
                let m = v_mag(&out[i]);
                out[i] = v_div(&out[i], m);
            }
        }
        out
    }

    /// The loop of `calcObtuseAngleIndex(i, j, dot)` over `j = 0, 1, 2`, each
    /// call overwriting the stored index (an upstream oddity, see the module
    /// docs): the result reflects node 2 only.
    fn calc_obtuse(edge_vec: &[V3; 3]) -> Option<usize> {
        let mut obtuse = None;
        for j in 0..3 {
            let dot = v_dot(&edge_vec[j], &edge_vec[(j + 3 - 1) % 3]);
            obtuse = if dot > 0. { Some(j) } else { None };
        }
        obtuse
    }

    /// Recompute every derived property from the nodes: upstream
    /// `recalcLocalSurfProperties`, run on a neighbour-list rebuild step. Also
    /// stores the node positions as `nodesLastRe_` (`storeNodePosRebuild`).
    pub fn refresh_from_nodes(&mut self, nodes: &[Nodes]) {
        self.geo.clear();
        for nd in nodes {
            let (edge_vec, edge_len) = Self::calc_edge_vec_len(nd);
            let surf_norm = Self::calc_surface_norm(&edge_vec);
            let obtuse = Self::calc_obtuse(&edge_vec);
            self.geo.push(TriGeo {
                edge_vec,
                edge_len,
                surf_norm,
                obtuse,
            });
        }
        self.nodes_last_rebuild = nodes.to_vec();
    }

    /// `MultiNodeMesh::decideRebuild`: has any node moved further than half the
    /// neighbour skin since the last rebuild?
    #[must_use]
    pub fn decide_rebuild(&self, nodes: &[Nodes], skin: f64) -> bool {
        let triggersq = 0.25 * skin * skin;
        for (nd, old) in nodes.iter().zip(&self.nodes_last_rebuild) {
            for j in 0..3 {
                let d = v_sub(&nd[j], &old[j]);
                let dist_sq = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                if dist_sq > triggersq {
                    return true;
                }
            }
        }
        false
    }

    /// `TriMesh::resolveTriSphereNeighbuild(nTri, rSphere, cSphere,
    /// treshold)`: whether a sphere at `c` is a neighbour of facet `i` (within
    /// `r_sphere + threshold` of the facet's plane and not farther outside any
    /// edge plane). Used by the mesh neighbour list of `fix massflow/mesh`.
    #[must_use]
    pub fn neighbuild(&self, nd: &Nodes, i: usize, r_sphere: f64, c: &V3, threshold: f64) -> bool {
        let max_dist = r_sphere + threshold;
        let mut avg = [0.; 3];
        for node in nd {
            avg = v_add(node, &avg);
        }
        let center = v_div(&avg, 3.0);
        let g = &self.geo[i];
        let d_norm = v_dot(&g.surf_norm, &v_sub(c, &center)).abs();
        if d_norm > max_dist {
            return false;
        }
        let para_max = max_dist * max_dist;
        for k in 0..3 {
            // calcEdgeNormals
            let mut en = v_cross(&g.edge_vec[k], &g.surf_norm);
            let m = v_mag(&en);
            if m < 1e-15 {
                en = g.edge_vec[(k + 1) % 3];
            } else {
                en = v_div(&en, m);
            }
            let d = v_dot(&en, &v_sub(c, &nd[k]));
            if d > 0. && d * d > para_max {
                return false;
            }
        }
        true
    }

    /// `TriMesh::calcArea(i)` from the current cached edge vectors.
    #[must_use]
    pub fn area(&self, i: usize) -> f64 {
        let g = &self.geo[i];
        let c = v_cross(&g.edge_vec[0], &g.edge_vec[1]);
        0.5 * v_mag(&c) * g.edge_len[0] * g.edge_len[1]
    }

    /// `SurfaceMesh::edgePointDist(iSrf, iEdge, point)`: distance from `point`
    /// to the segment of edge `i_edge` of facet `i` (node `i_edge` to
    /// `i_edge + 1`), as upstream: node distance beyond either end, else the
    /// absolute dot product with the edge normal.
    #[must_use]
    pub fn edge_point_dist(&self, nodes: &[Nodes], i: usize, i_edge: usize, point: &V3) -> f64 {
        let g = &self.geo[i];
        let mut node_to_p = v_sub(point, &nodes[i][i_edge]);
        let dot = v_dot(&g.edge_vec[i_edge], &node_to_p);
        if dot < 0. {
            v_mag(&node_to_p)
        } else if dot > g.edge_len[i_edge] {
            node_to_p = v_sub(point, &nodes[i][(i_edge + 1) % 3]);
            v_mag(&node_to_p)
        } else {
            let edge_norm = Self::calc_edge_normals(&g.edge_vec, &g.surf_norm);
            v_dot(&edge_norm[i_edge], &node_to_p).abs()
        }
    }

    /// `TriMeshPlanar::buildEdgeLists`: for every facet the (up to) six nearest
    /// active edges, keeping the first maximum for replacement, and the
    /// minimum of those distances.
    #[must_use]
    pub fn build_planar_edge_lists(&self, nodes: &[Nodes]) -> PlanarEdgeLists {
        let n = nodes.len();
        let mut out = PlanarEdgeLists {
            edge_facet: vec![[-1; 6]; n],
            edge_index: vec![[-1; 6]; n],
            min_dist: vec![0.0; n],
        };
        for i in 0..n {
            let mut n_edges = 0usize;
            let mut dist_edges = [1e8_f64; 6];
            for j in 0..n {
                if !self.edge_active[j].iter().any(|&a| a) {
                    continue;
                }
                for i_edge in 0..3 {
                    for j_edge in 0..3 {
                        if !self.edge_active[j][j_edge] {
                            continue;
                        }
                        // edgeNodeDist(i, i_edge, j, j_edge)
                        let dist = self.edge_point_dist(nodes, i, i_edge, &nodes[j][j_edge]);
                        if n_edges < 6 {
                            out.edge_facet[i][n_edges] = j as i32;
                            out.edge_index[i][n_edges] = j_edge as i32;
                            dist_edges[n_edges] = dist;
                            n_edges += 1;
                        } else {
                            let mut which = 0;
                            let mut m = dist_edges[0];
                            for (k, &d) in dist_edges.iter().enumerate().skip(1) {
                                if d > m {
                                    m = d;
                                    which = k;
                                }
                            }
                            out.edge_facet[i][which] = j as i32;
                            out.edge_index[i][which] = j_edge as i32;
                            dist_edges[which] = dist;
                        }
                    }
                }
            }
            let mut m = dist_edges[0];
            for &d in dist_edges.iter().skip(1) {
                if d < m {
                    m = d;
                }
            }
            out.min_dist[i] = m;
        }
        out
    }

    /// Current unit surface normal of facet `i`.
    #[must_use]
    pub fn surface_norm(&self, i: usize) -> V3 {
        self.geo[i].surf_norm
    }

    /// Everything [`Self::resolve_tri_sphere_contact_bary`] reads about facet
    /// `i` beside its nodes, for the GPU mirror (`gpu::dem`, gh:#1503):
    /// `(edge_vec, edge_len, surf_norm, obtuse, r_bound, edge_active,
    /// corner_active)`.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub(crate) fn facet_data(
        &self,
        i: usize,
    ) -> ([V3; 3], [f64; 3], V3, Option<usize>, f64, [bool; 3], [bool; 3]) {
        let g = &self.geo[i];
        (
            g.edge_vec,
            g.edge_len,
            g.surf_norm,
            g.obtuse,
            self.r_bound[i],
            self.edge_active[i],
            self.corner_active[i],
        )
    }

    // -------- incremental rotation -----------------------------------------

    /// `MultiNodeMesh::rotate(dAngle, axis, p)` followed by the container
    /// rotation `customValues_.rotate(dQ)` of `edgeVec` and `surfaceNorm`:
    /// rotate the nodes about the axis through `origin` by `d_angle`, and
    /// rotate the cached edge vectors and normals by the same quaternion
    /// (**not** recomputing them).
    pub fn rotate_incremental(
        &mut self,
        nodes: &mut [Nodes],
        d_angle: f64,
        axis: &V3,
        origin: &V3,
    ) {
        let axis_norm = v_div(axis, v_mag(axis));
        let dq = [
            (d_angle * 0.5).r_cos(),
            axis_norm[0] * (d_angle * 0.5).r_sin(),
            axis_norm[1] * (d_angle * 0.5).r_sin(),
            axis_norm[2] * (d_angle * 0.5).r_sin(),
        ];
        let trans = v_mag_sq(origin) > 0.;
        for nd in nodes.iter_mut() {
            for node in nd.iter_mut() {
                if trans {
                    *node = v_sub(node, origin);
                }
                *node = vec_quat_rotate(node, &dq);
                if trans {
                    *node = v_add(node, origin);
                }
            }
        }
        for g in &mut self.geo {
            for e in &mut g.edge_vec {
                *e = vec_quat_rotate(e, &dq);
            }
            g.surf_norm = vec_quat_rotate(&g.surf_norm, &dq);
        }
    }

    /// `MultiNodeMesh::rotate(totalAngle, dAngle, axis, p)` (the servo's
    /// torque mode): `nodes` are reset to `orig` (`resetToOrig`) and rotated by
    /// the TOTAL quaternion about `origin`; the cached edge vectors and
    /// normals rotate by the INCREMENTAL quaternion (`customValues_.rotate
    /// (totalQ, dQ)` applies `dQ` to element properties). Unlike the
    /// incremental overload, `axis` is used as given (not re-normalised), as
    /// upstream does.
    pub fn rotate_total(
        &mut self,
        orig: &[Nodes],
        nodes: &mut [Nodes],
        total_angle: f64,
        d_angle: f64,
        axis: &V3,
        origin: &V3,
    ) {
        let total_q = [
            (total_angle * 0.5).r_cos(),
            axis[0] * (total_angle * 0.5).r_sin(),
            axis[1] * (total_angle * 0.5).r_sin(),
            axis[2] * (total_angle * 0.5).r_sin(),
        ];
        let dq = [
            (d_angle * 0.5).r_cos(),
            axis[0] * (d_angle * 0.5).r_sin(),
            axis[1] * (d_angle * 0.5).r_sin(),
            axis[2] * (d_angle * 0.5).r_sin(),
        ];
        let trans = v_mag_sq(origin) > 0.;
        for (nd, o) in nodes.iter_mut().zip(orig) {
            *nd = *o;
            for node in nd.iter_mut() {
                if trans {
                    *node = v_sub(node, origin);
                }
                *node = vec_quat_rotate(node, &total_q);
                if trans {
                    *node = v_add(node, origin);
                }
            }
        }
        for g in &mut self.geo {
            for e in &mut g.edge_vec {
                *e = vec_quat_rotate(e, &dq);
            }
            g.surf_norm = vec_quat_rotate(&g.surf_norm, &dq);
        }
    }

    /// Facets sharing at least one node with facet `i`.
    #[must_use]
    pub fn node_neighbours(&self, i: usize) -> &[u32] {
        &self.node_neighs[i]
    }

    // -------- coplanarity ---------------------------------------------------

    /// `areCoplanarNodeNeighs(a, b)`: the facets share a node (or an edge) and
    /// their current normals are parallel to within `curvature`.
    #[must_use]
    pub fn are_coplanar_node_neighs(&self, a: usize, b: usize) -> bool {
        if !self.node_neighs[a].contains(&(b as u32)) {
            return false;
        }
        let dot = v_dot(&self.geo[a].surf_norm, &self.geo[b].surf_norm);
        dot.abs() > CURVATURE
    }

    // -------- topology ------------------------------------------------------

    fn nodes_equal(a: &V3, b: &V3) -> bool {
        (0..3).all(|i| comp_double(a[i], b[i], PRECISION))
    }

    /// `containsNode(iElem, nodeToCheck)`: index of the first node equal to
    /// `p`, if any.
    fn contains_node(nd: &Nodes, p: &V3) -> Option<usize> {
        (0..3).find(|&j| {
            comp_double(nd[j][0], p[0], PRECISION)
                && comp_double(nd[j][1], p[1], PRECISION)
                && comp_double(nd[j][2], p[2], PRECISION)
        })
    }

    fn centre(nd: &Nodes) -> V3 {
        let mut avg = [0.; 3];
        for node in nd {
            avg = v_add(node, &avg);
        }
        v_div(&avg, 3.0)
    }

    /// `share2Nodes(iElem, jElem, ...)`: returns `(iNode1, jNode1, iNode2,
    /// jNode2)` for the first two shared node pairs found scanning `i` then `j`.
    fn share_2_nodes(
        &self,
        nodes: &[Nodes],
        centres: &[V3],
        i: usize,
        j: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let dist = v_sub(&centres[i], &centres[j]);
        let radsum = self.r_bound[i] + self.r_bound[j];
        if v_mag_sq(&dist) > radsum * radsum {
            return None;
        }
        let mut n_shared = 0;
        let (mut i1, mut j1) = (0, 0);
        for a in 0..3 {
            for b in 0..3 {
                if Self::nodes_equal(&nodes[i][a], &nodes[j][b]) {
                    if n_shared == 0 {
                        i1 = a;
                        j1 = b;
                    } else {
                        return Some((i1, j1, a, b));
                    }
                    n_shared += 1;
                }
            }
        }
        None
    }

    /// `nSharedNodes(iElem, jElem) != 0`, with the same bounding-sphere broad
    /// phase.
    fn shares_any_node(&self, nodes: &[Nodes], centres: &[V3], i: usize, j: usize) -> bool {
        let dist = v_sub(&centres[i], &centres[j]);
        let radsum = self.r_bound[i] + self.r_bound[j];
        if v_mag_sq(&dist) > radsum * radsum {
            return false;
        }
        for a in 0..3 {
            for b in 0..3 {
                if Self::nodes_equal(&nodes[i][a], &nodes[j][b]) {
                    return true;
                }
            }
        }
        false
    }

    /// `coplanarNeighsOverlap(iSrf, iEdge, jSrf, jEdge)`.
    fn coplanar_neighs_overlap(
        &self,
        nodes: &[Nodes],
        i: usize,
        i_edge: usize,
        j: usize,
        j_edge: usize,
    ) -> bool {
        let g = &self.geo[i];
        let edge_norm = Self::calc_edge_normals(&g.edge_vec, &g.surf_norm);
        let p_ref = nodes[i][i_edge];
        let edge_n = edge_norm[i_edge];
        let vec_i = v_sub(&nodes[i][(i_edge + 2) % 3], &p_ref);
        let vec_j = v_sub(&nodes[j][(j_edge + 2) % 3], &p_ref);
        let dot1 = v_dot(&vec_i, &edge_n);
        let dot2 = v_dot(&vec_j, &edge_n);
        dot1 * dot2 > 0.
    }

    /// `checkNodeRecursive`: collect the connected fan of facets around the
    /// node `p`, walking edge neighbours of facets that contain it.
    #[allow(clippy::too_many_arguments)]
    fn check_node_recursive(
        &self,
        nodes: &[Nodes],
        edge_neighs: &[Vec<usize>],
        i: usize,
        p: &V3,
        visited: &mut Vec<usize>,
        has_node: &mut Vec<usize>,
        edge_list: &mut Vec<V3>,
        edge_end_point: &mut Vec<V3>,
        any_active_edge: &mut bool,
    ) {
        if visited.contains(&i) {
            return;
        }
        visited.push(i);
        if let Some(i_node) = Self::contains_node(&nodes[i], p) {
            has_node.push(i);
            edge_list.push(self.geo[i].edge_vec[i_node]);
            edge_list.push(self.geo[i].edge_vec[(i_node + 3 - 1) % 3]);
            edge_end_point.push(nodes[i][(i_node + 1) % 3]);
            edge_end_point.push(nodes[i][(i_node + 3 - 1) % 3]);
            if self.edge_active[i][i_node] || self.edge_active[i][(i_node + 3 - 1) % 3] {
                *any_active_edge = true;
            }
            for &nb in &edge_neighs[i] {
                self.check_node_recursive(
                    nodes,
                    edge_neighs,
                    nb,
                    p,
                    visited,
                    has_node,
                    edge_list,
                    edge_end_point,
                    any_active_edge,
                );
            }
        }
    }

    /// `buildNeighbours`: edge neighbours, edge activity, corner activity (and
    /// the node-sharing relation used by [`Self::are_coplanar_node_neighs`]).
    ///
    /// Run once on the **original** nodes, as upstream does at mesh set-up.
    fn build_topology(&mut self, nodes: &[Nodes]) {
        let n = nodes.len();
        let centres: Vec<V3> = nodes.iter().map(Self::centre).collect();
        let mut edge_neighs: Vec<Vec<usize>> = vec![Vec::new(); n];

        // Node-sharing relation (areCoplanarNodeNeighs / nSharedNodes != 0).
        for i in 0..n {
            for j in 0..n {
                if i != j && self.shares_any_node(nodes, &centres, i, j) {
                    self.node_neighs[i].push(j as u32);
                }
            }
        }

        // Shared edges: pairs (i, j), j < i, as the bin list yields them.
        for i in 0..n {
            for j in 0..i {
                let Some((i1, j1, i2, j2)) = self.share_2_nodes(nodes, &centres, i, j) else {
                    continue;
                };
                // shareEdge
                let i_edge = if i1 + i2 == 2 { 2 } else { i1.min(i2) };
                let j_edge = if j1 + j2 == 2 { 2 } else { j1.min(j2) };
                // areCoplanar(id_i, id_j): fabs(dot) >= curvature
                let dot = v_dot(&self.geo[i].surf_norm, &self.geo[j].surf_norm);
                let coplanar = dot.abs() >= CURVATURE;
                // handleSharedEdge(i, iEdge, j, jEdge, coplanar, neighflag = true)
                edge_neighs[i].push(j);
                edge_neighs[j].push(i);
                if !coplanar || self.coplanar_neighs_overlap(nodes, i, i_edge, j, j_edge) {
                    if i < j {
                        self.edge_active[i][i_edge] = false;
                        self.edge_active[j][j_edge] = true;
                    } else {
                        self.edge_active[i][i_edge] = true;
                        self.edge_active[j][j_edge] = false;
                    }
                } else {
                    self.edge_active[i][i_edge] = false;
                    self.edge_active[j][j_edge] = false;
                }
            }
        }

        // handleCorner for every node of every facet.
        for i in 0..n {
            for i_node in 0..3 {
                let p = nodes[i][i_node];
                let mut visited = Vec::new();
                let mut has_node = Vec::new();
                let mut edge_list = Vec::new();
                let mut edge_end_point = Vec::new();
                let mut any_active_edge = false;
                self.check_node_recursive(
                    nodes,
                    &edge_neighs,
                    i,
                    &p,
                    &mut visited,
                    &mut has_node,
                    &mut edge_list,
                    &mut edge_end_point,
                    &mut any_active_edge,
                );
                // (upstream returns here, leaving the corner active, when the
                // node is outside the sub-domain; no box here: see module docs)
                let n_edge_list = 2 * has_node.len();
                let max_id = has_node.iter().copied().max();
                let mut has_two_colinear_edges = false;
                for a in 0..n_edge_list {
                    for b in (a + 1)..n_edge_list {
                        let colinear = v_dot(&edge_list[a], &edge_list[b]).abs() > CURVATURE;
                        if colinear && !Self::nodes_equal(&edge_end_point[a], &edge_end_point[b]) {
                            has_two_colinear_edges = true;
                        }
                    }
                }
                self.corner_active[i][i_node] = if has_two_colinear_edges || !any_active_edge {
                    false
                } else {
                    Some(i) == max_id
                };
            }
        }
    }

    /// Whether facet `i`'s edge `j` is active (`edgeActive`).
    #[must_use]
    pub fn edge_active(&self, i: usize, j: usize) -> bool {
        self.edge_active[i][j]
    }

    /// Whether facet `i`'s corner `j` is active (`cornerActive`).
    #[must_use]
    pub fn corner_active(&self, i: usize, j: usize) -> bool {
        self.corner_active[i][j]
    }

    // -------- contact resolution -------------------------------------------

    /// `MathExtraLiggghts::calcBaryTriCoords(ap, edgeVec, edgeLen, bary)`.
    fn calc_bary_tri_coords(ap: &V3, edge_vec: &[V3; 3], edge_len: &[f64; 3]) -> V3 {
        let a = v_dot(ap, &edge_vec[0]);
        let b = v_dot(ap, &edge_vec[2]);
        let c = v_dot(&edge_vec[0], &edge_vec[2]);
        let one_min_c_sqr = 1. - c * c;
        let b1 = (a - b * c) / (edge_len[0] * one_min_c_sqr);
        let b2 = (a * c - b) / (edge_len[2] * one_min_c_sqr);
        let b0 = 1. - b1 - b2;
        [b0, b1, b2]
    }

    /// `TriMesh::calcDist(cs, closestPoint, delta)`: sets `delta = closest - cs`
    /// and returns the distance.
    fn calc_dist(cs: &V3, closest: &V3, delta: &mut V3) -> f64 {
        *delta = v_sub(closest, cs);
        point_distance(cs, closest)
    }

    /// `TriMesh::resolveEdgeContactBary`.
    #[allow(clippy::too_many_arguments)]
    fn resolve_edge_contact_bary(
        &self,
        nd: &Nodes,
        i_tri: usize,
        i_edge: usize,
        p: &V3,
        delta: &mut V3,
        bary: &mut V3,
        skip_inactive: bool,
    ) -> f64 {
        let ip = (i_edge + 1) % 3;
        let ipp = (i_edge + 2) % 3;
        let g = &self.geo[i_tri];
        let node_to_p = v_sub(p, &nd[i_edge]);
        let dist_from_node = v_dot(&node_to_p, &g.edge_vec[i_edge]);
        let d;
        if dist_from_node < -SMALL_TRIMESH {
            if skip_inactive && !self.corner_active[i_tri][i_edge] {
                return LARGE_TRIMESH;
            }
            d = Self::calc_dist(p, &nd[i_edge], delta);
            bary[i_edge] = 1.;
            bary[ip] = 0.;
            bary[ipp] = 0.;
        } else if dist_from_node > g.edge_len[i_edge] + SMALL_TRIMESH {
            if skip_inactive && !self.corner_active[i_tri][ip] {
                return LARGE_TRIMESH;
            }
            d = Self::calc_dist(p, &nd[ip], delta);
            bary[i_edge] = 0.;
            bary[ip] = 1.;
            bary[ipp] = 0.;
        } else {
            if skip_inactive && !self.edge_active[i_tri][i_edge] {
                return LARGE_TRIMESH;
            }
            let closest = v_add_mult(&nd[i_edge], dist_from_node, &g.edge_vec[i_edge]);
            d = Self::calc_dist(p, &closest, delta);
            bary[ipp] = 0.;
            bary[i_edge] = 1. - dist_from_node / g.edge_len[i_edge];
            bary[ip] = 1. - bary[i_edge];
        }
        d
    }

    /// `TriMesh::resolveCornerContactBary`.
    #[allow(clippy::too_many_arguments)]
    fn resolve_corner_contact_bary(
        &self,
        nd: &Nodes,
        i_tri: usize,
        i_node: usize,
        obtuse: bool,
        p: &V3,
        delta: &mut V3,
        bary: &mut V3,
        skip_inactive: bool,
    ) -> f64 {
        let ip = (i_node + 1) % 3;
        let ipp = (i_node + 2) % 3;
        let n = nd[i_node];
        let g = &self.geo[i_tri];
        if obtuse {
            let edge = &g.edge_vec;
            let node_to_p = v_sub(p, &n);

            let mut dist_from_node = v_dot(&node_to_p, &edge[ipp]);
            if dist_from_node < SMALL_TRIMESH {
                if dist_from_node > -g.edge_len[ipp] {
                    if skip_inactive && !self.edge_active[i_tri][ipp] {
                        return LARGE_TRIMESH;
                    }
                    let closest = v_add_mult(&n, dist_from_node, &edge[ipp]);
                    bary[ip] = 0.;
                    bary[i_node] = 1. + dist_from_node / g.edge_len[ipp];
                    bary[ipp] = 1. - bary[i_node];
                    return Self::calc_dist(p, &closest, delta);
                } else {
                    if skip_inactive && !self.corner_active[i_tri][ipp] {
                        return LARGE_TRIMESH;
                    }
                    bary[ipp] = 1.;
                    bary[i_node] = 0.;
                    bary[ip] = 0.;
                    return Self::calc_dist(p, &nd[ipp], delta);
                }
            }

            dist_from_node = v_dot(&node_to_p, &edge[i_node]);
            if dist_from_node > -SMALL_TRIMESH {
                if dist_from_node < g.edge_len[i_node] {
                    if skip_inactive && !self.edge_active[i_tri][i_node] {
                        return LARGE_TRIMESH;
                    }
                    let closest = v_add_mult(&n, dist_from_node, &edge[i_node]);
                    bary[ipp] = 0.;
                    bary[i_node] = 1. - dist_from_node / g.edge_len[i_node];
                    bary[ip] = 1. - bary[i_node];
                    return Self::calc_dist(p, &closest, delta);
                } else {
                    if skip_inactive && !self.corner_active[i_tri][ip] {
                        return LARGE_TRIMESH;
                    }
                    bary[ip] = 1.;
                    bary[i_node] = 0.;
                    bary[ipp] = 0.;
                    return Self::calc_dist(p, &nd[ip], delta);
                }
            }
        }

        if skip_inactive && !self.corner_active[i_tri][i_node] {
            return LARGE_TRIMESH;
        }
        bary[i_node] = 1.;
        bary[ip] = 0.;
        bary[ipp] = 0.;
        Self::calc_dist(p, &nd[i_node], delta)
    }

    /// `TriMesh::resolveFaceContactBary`.
    fn resolve_face_contact_bary(
        &self,
        i_tri: usize,
        p: &V3,
        node0_to_sphere_centre: &V3,
        delta: &mut V3,
    ) -> f64 {
        let surf_norm = &self.geo[i_tri].surf_norm;
        let d_norm = v_dot(surf_norm, node0_to_sphere_centre);
        let tmp = v_mul(surf_norm, d_norm);
        let cs_plane = v_sub(p, &tmp);
        Self::calc_dist(p, &cs_plane, delta)
    }

    /// `TriMesh::resolveTriSphereContactBary(iPart, nTri, rSphere, cSphere,
    /// delta, bary, barySign, skip_inactive)`.
    ///
    /// Returns `(deltan, delta, bary, barysign)`: the signed gap
    /// `d - rSphere` (negative when overlapping), `delta = closest - centre`,
    /// the barycentric coordinates of the contact point and upstream's region
    /// code (`7` is a face contact, anything else an edge or corner).
    #[must_use]
    pub fn resolve_tri_sphere_contact_bary(
        &self,
        nd: &Nodes,
        n_tri: usize,
        r_sphere: f64,
        c_sphere: &V3,
        skip_inactive: bool,
    ) -> (f64, V3, V3, u8) {
        let g = &self.geo[n_tri];
        let obtuse_angle_index = g.obtuse;

        let node0_to_sphere_centre = v_sub(c_sphere, &nd[0]);
        let mut bary =
            Self::calc_bary_tri_coords(&node0_to_sphere_centre, &g.edge_vec, &g.edge_len);

        let invlen = 1. / (2. * self.r_bound[n_tri]);
        let thr = -PRECISION * invlen;
        let bary_sign: u8 =
            u8::from(bary[0] > thr) + 2 * u8::from(bary[1] > thr) + 4 * u8::from(bary[2] > thr);

        let mut delta = [0.; 3];
        let d = match bary_sign {
            1 => self.resolve_corner_contact_bary(
                nd,
                n_tri,
                0,
                obtuse_angle_index == Some(0),
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            2 => self.resolve_corner_contact_bary(
                nd,
                n_tri,
                1,
                obtuse_angle_index == Some(1),
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            3 => self.resolve_edge_contact_bary(
                nd,
                n_tri,
                0,
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            4 => self.resolve_corner_contact_bary(
                nd,
                n_tri,
                2,
                obtuse_angle_index == Some(2),
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            5 => self.resolve_edge_contact_bary(
                nd,
                n_tri,
                2,
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            6 => self.resolve_edge_contact_bary(
                nd,
                n_tri,
                1,
                c_sphere,
                &mut delta,
                &mut bary,
                skip_inactive,
            ),
            7 => {
                self.resolve_face_contact_bary(n_tri, c_sphere, &node0_to_sphere_centre, &mut delta)
            }
            // Upstream: error->one("Internal error"). All three barycentric
            // coordinates at or below -threshold cannot happen for a facet
            // whose barycentric coordinates sum to one.
            _ => LARGE_TRIMESH,
        };
        (d - r_sphere, delta, bary, bary_sign)
    }

    /// Node velocity weighted by barycentric coordinates, as in
    /// `FixWallGran::post_force_mesh`:
    /// `bary[0]*v[0] + bary[1]*v[1] + bary[2]*v[2]`, componentwise.
    #[must_use]
    pub fn wall_velocity(&self, i_tri: usize, bary: &V3) -> V3 {
        let v = &self.node_velocity[i_tri];
        let mut out = [0.; 3];
        for k in 0..3 {
            out[k] = bary[0] * v[0][k] + bary[1] * v[1][k] + bary[2] * v[2][k];
        }
        out
    }
}

/// `SMALL_TRIMESH` re-exported for tests that probe the thresholds.
pub const SMALL: f64 = SMALL_TRIMESH;

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(a: V3, b: V3, c: V3) -> Nodes {
        [a, b, c]
    }

    /// **Methodology.** Two coplanar triangles forming a square: the shared
    /// diagonal is a coplanar edge, so upstream deactivates it on **both**
    /// facets (`handleSharedEdge`, coplanar branch), and every outer edge stays
    /// active. A sphere above the diagonal therefore gets one face contact, not
    /// a second edge contact. **Result (2026-11-11):** diagonal inactive on both.
    #[test]
    fn coplanar_diagonal_is_inactive_on_both_facets() {
        let nodes = vec![
            tri([0., 0., 0.], [1., 0., 0.], [1., 1., 0.]),
            tri([0., 0., 0.], [1., 1., 0.], [0., 1., 0.]),
        ];
        let s = TriMeshState::new(&nodes);
        // facet 0 edge 2 (node2 -> node0) and facet 1 edge 0 (node0 -> node1)
        assert!(!s.edge_active(0, 2));
        assert!(!s.edge_active(1, 0));
        assert!(s.edge_active(0, 0) && s.edge_active(0, 1));
        assert!(s.edge_active(1, 1) && s.edge_active(1, 2));
        assert!(s.are_coplanar_node_neighs(0, 1));
    }

    /// **Methodology.** Two triangles folded 90 degrees along a shared edge: not
    /// coplanar, so exactly one of the two copies of the edge is active (the
    /// higher facet id), never both and never neither. **Result:** facet 1 holds it.
    #[test]
    fn folded_edge_is_active_on_exactly_one_facet() {
        let nodes = vec![
            tri([0., 0., 0.], [1., 0., 0.], [0., 1., 0.]),
            tri([1., 0., 0.], [0., 0., 0.], [0., 0., 1.]),
        ];
        let s = TriMeshState::new(&nodes);
        // shared edge: facet 0 edge 0 (node0 -> node1), facet 1 edge 0 (node0 -> node1)
        assert!(!s.edge_active(0, 0));
        assert!(s.edge_active(1, 0));
        assert!(!s.are_coplanar_node_neighs(0, 1));
    }

    /// **Methodology.** Face contact: sphere of radius 0.01 centred 0.009 above
    /// the interior of a horizontal facet; expected `deltan = d - r = -0.001`,
    /// `delta = (0, 0, -0.009)`, region code 7, barycentric coordinates summing
    /// to 1. **Result:** matches to 1e-15; a sphere over the neighbouring facet
    /// seen from this one resolves in an inactive edge region and reports
    /// `LARGE_TRIMESH`.
    #[test]
    fn face_contact_and_inactive_edge_skip() {
        let nodes = vec![
            tri([0., 0., 0.], [1., 0., 0.], [1., 1., 0.]),
            tri([0., 0., 0.], [1., 1., 0.], [0., 1., 0.]),
        ];
        let s = TriMeshState::new(&nodes);
        let (deltan, delta, bary, code) =
            s.resolve_tri_sphere_contact_bary(&nodes[0], 0, 0.01, &[0.6, 0.2, 0.009], true);
        assert_eq!(code, 7);
        assert!((deltan + 0.001).abs() < 1e-15, "{deltan}");
        assert!((delta[2] + 0.009).abs() < 1e-15);
        assert!((bary[0] + bary[1] + bary[2] - 1.0).abs() < 1e-14);
        let (far, _, _, code2) =
            s.resolve_tri_sphere_contact_bary(&nodes[0], 0, 0.01, &[0.2, 0.6, 0.009], true);
        assert_ne!(code2, 7);
        assert!(far > 1.0e5);
    }

    /// **Methodology.** A quarter-turn about z through the origin maps `(1,0,0)`
    /// to `(0,1,0)` and rotates the cached normal `(0,0,1)` onto itself;
    /// `rotate_incremental` rotates edge vectors with the nodes; the rebuild
    /// decision fires after the move and clears after a refresh.
    #[test]
    fn incremental_rotation_rotates_nodes_and_cached_vectors() {
        let mut nodes = vec![tri([1., 0., 0.], [2., 0., 0.], [1., 1., 0.])];
        let mut s = TriMeshState::new(&nodes);
        s.rotate_incremental(
            &mut nodes,
            std::f64::consts::FRAC_PI_2,
            &[0., 0., 1.],
            &[0.; 3],
        );
        assert!((nodes[0][0][0]).abs() < 1e-15 && (nodes[0][0][1] - 1.0).abs() < 1e-15);
        let n = s.surface_norm(0);
        assert!((n[2] - 1.0).abs() < 1e-15);
        assert!(s.decide_rebuild(&nodes, 0.1));
        s.refresh_from_nodes(&nodes);
        assert!(!s.decide_rebuild(&nodes, 0.1));
    }

    /// **Methodology.** `rotate_total` puts the nodes at `orig` rotated by the
    /// TOTAL angle (a quarter-turn about z through (1, 0, 0)) while the cached
    /// edge vectors and normal rotate by the INCREMENTAL angle (an eighth).
    /// **Result:** node `(0,0,0)` goes to `(1,-1,0)`; the edge `(1,0,0)` turns
    /// to `(cos 45, sin 45, 0)` to 1e-15; the normal stays `(0,0,1)`.
    #[test]
    fn rotate_total_uses_total_for_nodes_and_incremental_for_cache() {
        let orig = vec![tri([0., 0., 0.], [1., 0., 0.], [0., 1., 0.])];
        let mut s = TriMeshState::new(&orig);
        let mut nodes = orig.clone();
        let q = std::f64::consts::FRAC_PI_2;
        s.rotate_total(&orig, &mut nodes, q, q / 2.0, &[0., 0., 1.], &[1., 0., 0.]);
        assert!((nodes[0][0][0] - 1.0).abs() < 1e-15 && (nodes[0][0][1] + 1.0).abs() < 1e-15);
        let c = (0.5_f64).sqrt();
        let e = s.geo[0].edge_vec[0];
        assert!((e[0] - c).abs() < 1e-15 && (e[1] - c).abs() < 1e-15, "{e:?}");
        assert!((s.surface_norm(0)[2] - 1.0).abs() < 1e-15);
    }
}
