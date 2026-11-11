// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctree.{C,H}, meshOctreeI.H
//     (constructor, createInitialOctreeBox, setOctantVectorsAndPositions,
//      leaf access, containedTriangles / containedEdges)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeSlot.H
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeCubePatches.C
//     (findBoundaryPatchesForLeaf)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeCube/
//     meshOctreeCubeCoordinates.H, meshOctreeCubeCoordinatesI.H,
//     meshOctreeCubeCoordinatesIntersections.C, meshOctreeCubeBasic{.H,I.H},
//     meshOctreeCube{.C,.H,I.H}, meshOctreeCubeIntersections.C,
//     meshOctreeCubeRefine.C (refineCube, findContainedEdges),
//     meshOctreeCubeRecursiveFunctions.C (leavesInBox, leavesInSphere,
//     markLeavesInSphere, findLeaves, countChildCubes)
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (nearestPointOnTheTriangle, nearestPointOnTheEdge(Exact),
//      triLineIntersection, boundBoxLineIntersection)
//   meshLibrary/utilities/helperClasses/matrices/matrix3DI.H (Cramer solve)
//   meshLibrary/utilities/meshes/triSurf/triSurfAddressing.C (calculateEdges)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`meshOctree`**: the surface octree that every cfMesh workflow
//! is built on (gh:#1262), ported **beside** [`crate::octree`], not over it.
//!
//! [`crate::octree`] is this crate's own re-implementation: a `BTreeSet` of
//! leaves refined by distance to the surface, pinned bit for bit by
//! `pipeline::tests::octree_path_is_pinned_bit_for_bit`. This module is the
//! upstream structure, transcribed: a tree of [`OctreeCube`]s held in one
//! [`OctreeSlot`] (upstream's per-thread storage; the port is serial, so
//! there is one), each cube carrying the **surface triangles it intersects**
//! (`containedTriangles_`) and the **feature edges** crossing it
//! (`containedEdges_`). The leaves are listed in upstream's order (depth
//! first, children in Z order), so every container that reaches the output
//! is ordered.
//!
//! # What is here
//!
//! - [`CubeCoordinates`]: `meshOctreeCubeCoordinates` (position and level,
//!   refinement and coarsening of coordinates, cube boxes, the cube–triangle
//!   and cube–line intersection tests, Morton comparison).
//! - [`OctreeCube`] / [`CubeType`]: `meshOctreeCube` and
//!   `meshOctreeCubeBasic` (`UNKNOWN`, `OUTSIDE`, `DATA`, `INSIDE`).
//! - [`OctreeSlot`]: `meshOctreeSlot` (cubes, child links, contained
//!   triangles, contained edges).
//! - [`MeshOctree`]: `meshOctree` (root box, leaves, `refineCube`,
//!   `findLeaves`, the recursive box and sphere searches,
//!   `findBoundaryPatchesForLeaf`).
//! - [`TriSurf`]: the part of cfMesh's `triSurf` the octree reads (points,
//!   facets with a region each, edges, edge facets, facet edges, point
//!   edges).
//! - [`help`]: the geometric helpers the intersections call.
//!
//! Queries (`findLeafContainingVertex`, neighbour searches, nearest surface
//! point) are [`crate::mesh_octree_queries`]; refinement drivers are
//! [`crate::mesh_octree_modifier`] and [`crate::mesh_octree_creator`].
//!
//! # Not ported
//!
//! - ~~The **quadtree** (`isQuadtree_`, `refineCube2D`, negative `posZ`)~~
//!   **PORTED 2026-11-05 (gh:#1275):** [`MeshOctree::new_quadtree`]. The
//!   root cube is `(0, 0, -10, 0)`; a negative `posZ` is never refined,
//!   reduced or shifted in z, and its box spans the root box in z
//!   (`cubeBox`); [`MeshOctree::refine_cube`] makes 4 children
//!   (`refineCube2D`), the slots 4–7 holding [`NO_CUBE`] (upstream's
//!   `nullptr`); the neighbour searches drop the z directions.
//! - MPI: `procNo`, `refineMissingCube`, `purgeProcessorCubes`,
//!   `findCoordinatesOfMissingCubes`, the neighbour-processor ranges.
//! - The Hilbert tables `hOrder_` / `hOrient_`: declared upstream, used by no
//!   serial routine at this commit (grep of `meshLibrary/`).
//!
//! # Constants
//!
//! cfMesh compiles against OpenFOAM, whose `SMALL` is `1e-15`, `VSMALL`
//! `1e-300`, `ROOTVSMALL` `1e-150` and `VGREAT` `1e300` for double
//! precision; the same values are used here ([`SMALL`], [`VSMALL`],
//! [`ROOTVSMALL`], [`VGREAT`]).

use crate::math::Vec3;
use crate::mesh_subsets::{SubsetGroup, SubsetKind};
use std::collections::BTreeSet;
use std::sync::Arc;

/// OpenFOAM `SMALL` (double precision).
pub const SMALL: f64 = 1.0e-15;
/// OpenFOAM `VSMALL` (double precision).
pub const VSMALL: f64 = 1.0e-300;
/// OpenFOAM `ROOTVSMALL` (double precision).
pub const ROOTVSMALL: f64 = 1.0e-150;
/// OpenFOAM `VGREAT` (double precision).
pub const VGREAT: f64 = 1.0e300;

/// Component `i` (0, 1, 2) of a vector.
pub(crate) fn comp(v: Vec3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Component-wise minimum.
pub(crate) fn vmin(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}

/// Component-wise maximum.
pub(crate) fn vmax(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}

/// Squared length.
pub(crate) fn mag_sqr(v: Vec3) -> f64 {
    v.dot(v)
}

/// OpenFOAM's `boundBox`: an axis-aligned box with **inclusive** bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundBox {
    /// Lower corner.
    pub min: Vec3,
    /// Upper corner.
    pub max: Vec3,
}

impl BoundBox {
    /// Construct from the two corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        BoundBox { min, max }
    }

    /// The bounding box of a point set (`boundBox(const pointField&)`).
    /// An empty set gives the inverted box `[VGREAT, -VGREAT]`.
    pub fn of_points(points: &[Vec3]) -> Self {
        let mut bb = BoundBox::new(
            Vec3::new(VGREAT, VGREAT, VGREAT),
            Vec3::new(-VGREAT, -VGREAT, -VGREAT),
        );
        for &p in points {
            bb.min = vmin(bb.min, p);
            bb.max = vmax(bb.max, p);
        }
        bb
    }

    /// `boundBox::overlaps`: true if the two boxes share any point (bounds
    /// inclusive).
    pub fn overlaps(&self, bb: &BoundBox) -> bool {
        bb.max.x >= self.min.x
            && bb.min.x <= self.max.x
            && bb.max.y >= self.min.y
            && bb.min.y <= self.max.y
            && bb.max.z >= self.min.z
            && bb.min.z <= self.max.z
    }

    /// `boundBox::contains(point)`, bounds inclusive.
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// `max - min`.
    pub fn span(&self) -> Vec3 {
        self.max.sub(self.min)
    }
}

/// The part of cfMesh's `triSurf` the octree reads: points, triangles with a
/// region (patch) index each, and the edge addressing
/// (`triSurfAddressing`).
///
/// Edges are numbered as upstream's `calculateEdges` numbers them: point by
/// point in ascending order, and at each point the pairs `(p, q)` with
/// `q >= p` in ascending `q` (a `std::set` upstream, a `BTreeSet` here).
/// `edge_facets[e]` lists the facets of edge `e` in ascending facet order,
/// `facet_edges[f][i]` is the edge `(tri[i], tri[(i + 1) % 3])`, and
/// `point_edges[p]` the edges at point `p` in ascending edge order.
///
/// **Extended 2026-11-05 (gh:#1276):** this is now the crate's one port of
/// the whole `triSurf` (`triSurfPoints`, `triSurfFacets`,
/// `triSurfFeatureEdges`, `triSurfAddressing`): it also carries the patch
/// list ([`SurfacePatch`], upstream's `geometricSurfacePatchList`), the
/// stored feature edges (`featureEdges_`), the point, facet and
/// feature-edge subsets (as [`crate::mesh_subsets::SubsetGroup`]s,
/// upstream's `std::map<label, meshSubset>`) and `point_facets`. The
/// geometric addressing (`facetNormals`, `facetCentres`, `pointNormals`,
/// `facetFacetsEdges`) is computed on demand by the methods below. The
/// surface tools that use them are [`crate::surface`].
///
/// Upstream's addressing is demand-driven and cached, and some tools change
/// the facets without clearing it (see [`crate::surface`]'s defect list).
/// Here a tool that changes points or facets builds a new `TriSurf`
/// ([`TriSurf::from_parts`]), so the addressing always matches the facets.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurf {
    /// Vertex positions.
    pub points: Vec<Vec3>,
    /// Triangles (indices into `points`).
    pub triangles: Vec<[usize; 3]>,
    /// Region (patch) index of each triangle.
    pub regions: Vec<usize>,
    /// Edges `[p, q]`, `p < q`.
    pub edges: Vec<[usize; 2]>,
    /// Facets of each edge.
    pub edge_facets: Vec<Vec<usize>>,
    /// The three edges of each facet.
    pub facet_edges: Vec<[usize; 3]>,
    /// Edges at each point.
    pub point_edges: Vec<Vec<usize>>,
    /// Facets at each point, ascending (`pointFacets`, upstream's
    /// `reverseAddressing` of the facets).
    pub point_facets: Vec<Vec<usize>>,
    /// The patches (`patches_`); `regions[f]` indexes this list.
    pub patches: Vec<SurfacePatch>,
    /// The stored feature edges (`featureEdges_`), as upstream's `edge`:
    /// start and end point, not sorted. Not the same list as `edges`.
    pub feature_edges: Vec<[usize; 2]>,
    /// `pointSubsets_`.
    pub point_subsets: SubsetGroup,
    /// `facetSubsets_`.
    pub facet_subsets: SubsetGroup,
    /// `featureEdgeSubsets_` (labels index `feature_edges`).
    pub edge_subsets: SubsetGroup,
}

/// `geometricSurfacePatch`: a patch name and its geometric type
/// (`"patch"`, `"wall"`, `"symmetryPlane"`, …). Upstream's `index()` is the
/// patch's position in [`TriSurf::patches`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfacePatch {
    /// `name()`.
    pub name: String,
    /// `geometricType()`.
    pub geometric_type: String,
}

impl SurfacePatch {
    /// A patch of geometric type `"patch"`.
    pub fn new(name: &str) -> Self {
        SurfacePatch { name: name.to_string(), geometric_type: "patch".to_string() }
    }
}

impl TriSurf {
    /// Build the surface and its edge addressing. `regions` is one region
    /// index per triangle; pass `None` to put every triangle in region 0.
    ///
    /// # Panics
    ///
    /// If `regions` is given with a length other than `triangles.len()`, or a
    /// triangle names a point that does not exist.
    pub fn new(points: Vec<Vec3>, triangles: Vec<[usize; 3]>, regions: Option<Vec<usize>>) -> Self {
        let regions = regions.unwrap_or_else(|| vec![0; triangles.len()]);
        assert_eq!(regions.len(), triangles.len(), "one region per triangle");
        let np = points.len();
        let mut point_facets: Vec<Vec<usize>> = vec![Vec::new(); np];
        for (f, t) in triangles.iter().enumerate() {
            for &p in t {
                assert!(p < np, "triangle {f} names point {p} of {np}");
                point_facets[p].push(f);
            }
        }
        // calculateEdges: per point, the set of (p, q >= p).
        let mut edges: Vec<[usize; 2]> = Vec::new();
        let mut edge_of: std::collections::BTreeMap<(usize, usize), usize> = Default::default();
        for (p, pf) in point_facets.iter().enumerate() {
            let mut at: BTreeSet<usize> = BTreeSet::new();
            for &f in pf {
                let t = triangles[f];
                for i in 0..3 {
                    if t[i] == p {
                        if t[(i + 1) % 3] >= p {
                            at.insert(t[(i + 1) % 3]);
                        }
                        if t[(i + 2) % 3] >= p {
                            at.insert(t[(i + 2) % 3]);
                        }
                    }
                }
            }
            for q in at {
                edge_of.insert((p, q), edges.len());
                edges.push([p, q]);
            }
        }
        let mut edge_facets: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
        let mut facet_edges: Vec<[usize; 3]> = Vec::with_capacity(triangles.len());
        for (f, t) in triangles.iter().enumerate() {
            let mut fe = [0usize; 3];
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                let e = edge_of[&(a.min(b), a.max(b))];
                fe[i] = e;
                if !edge_facets[e].contains(&f) {
                    edge_facets[e].push(f);
                }
            }
            facet_edges.push(fe);
        }
        let mut point_edges: Vec<Vec<usize>> = vec![Vec::new(); np];
        for (e, ed) in edges.iter().enumerate() {
            point_edges[ed[0]].push(e);
            if ed[1] != ed[0] {
                point_edges[ed[1]].push(e);
            }
        }
        // Default patch list: upstream's `triSurfFacets(triangles)` names its
        // one patch "patch"; a labelled soup gets "patch0", "patch1", … (the
        // port's choice: there is no file to read names from).
        let n_patches = regions.iter().copied().max().map_or(1, |m| m + 1);
        let patches = if n_patches == 1 {
            vec![SurfacePatch::new("patch")]
        } else {
            (0..n_patches).map(|i| SurfacePatch::new(&format!("patch{i}"))).collect()
        };
        TriSurf {
            points,
            triangles,
            regions,
            edges,
            edge_facets,
            facet_edges,
            point_edges,
            point_facets,
            patches,
            feature_edges: Vec::new(),
            point_subsets: SubsetGroup::new(SubsetKind::Point),
            facet_subsets: SubsetGroup::new(SubsetKind::Face),
            edge_subsets: SubsetGroup::new(SubsetKind::FeatureEdge),
        }
    }

    /// `triSurf(triangles, patches, featureEdges, points)`: a surface with a
    /// given patch list and stored feature edges, and no subsets. The
    /// addressing is built from scratch.
    ///
    /// # Panics
    ///
    /// As [`TriSurf::new`], and if a region or a feature-edge point is out
    /// of range (upstream's `topologyCheck`, which is fatal there too).
    pub fn from_parts(
        points: Vec<Vec3>,
        triangles: Vec<[usize; 3]>,
        regions: Vec<usize>,
        patches: Vec<SurfacePatch>,
        feature_edges: Vec<[usize; 2]>,
    ) -> Self {
        let np = points.len();
        for (f, &r) in regions.iter().enumerate() {
            assert!(r < patches.len().max(1), "facet {f} names patch {r} of {}", patches.len());
        }
        for fe in &feature_edges {
            assert!(fe[0] < np && fe[1] < np, "feature edge {fe:?} out of scope 0 {np}");
        }
        let mut s = TriSurf::new(points, triangles, Some(regions));
        s.patches = patches;
        s.feature_edges = feature_edges;
        s
    }

    /// A surface labelled by [`crate::patches::SurfaceRegions`]: the
    /// patches take its names and geometric types (gh:#1483).
    pub fn with_surface_regions(points: Vec<Vec3>, triangles: Vec<[usize; 3]>, regions: &crate::patches::SurfaceRegions) -> Self {
        let patches = regions.names.iter().zip(regions.types_for(regions.names.len())).map(|(n, t)| SurfacePatch { name: n.clone(), geometric_type: t }).collect();
        TriSurf::from_parts(points, triangles, regions.region_of_tri.clone(), patches, Vec::new())
    }

    /// The patch labelling as [`crate::patches::SurfaceRegions`].
    pub fn surface_regions(&self) -> crate::patches::SurfaceRegions {
        crate::patches::SurfaceRegions {
            names: self.patches.iter().map(|p| p.name.clone()).collect(),
            region_of_tri: self.regions.clone(),
            types: self.patches.iter().map(|p| p.geometric_type.clone()).collect(),
        }
    }

    /// Number of triangles.
    pub fn size(&self) -> usize {
        self.triangles.len()
    }

    /// `nPoints()`.
    pub fn n_points(&self) -> usize {
        self.points.len()
    }

    /// `patchNames()`.
    pub fn patch_names(&self) -> Vec<String> {
        self.patches.iter().map(|p| p.name.clone()).collect()
    }

    /// `findPatches(name)`: the patches called `name`. Upstream matches a
    /// regular expression; the port matches the name exactly.
    pub fn find_patches(&self, name: &str) -> Vec<usize> {
        self.patches.iter().enumerate().filter(|(_, p)| p.name == name).map(|(i, _)| i).collect()
    }

    /// The surface edge joining `a` and `b` (either order), found through
    /// `pointEdges` as upstream's tools find a feature edge's label.
    pub fn find_edge(&self, a: usize, b: usize) -> Option<usize> {
        self.point_edges.get(a)?.iter().copied().find(|&e| {
            let ed = self.edges[e];
            (ed[0] == a && ed[1] == b) || (ed[0] == b && ed[1] == a)
        })
    }

    /// `labelledTri::normal`: the area vector `½ (b − a) × (c − a)`.
    pub fn facet_area_vector(&self, f: usize) -> Vec3 {
        let t = self.triangles[f];
        let (a, b, c) = (self.points[t[0]], self.points[t[1]], self.points[t[2]]);
        b.sub(a).cross(c.sub(a)).scale(0.5)
    }

    /// `calculateFacetNormals` for one facet: `v / (|v| + VSMALL)`.
    pub fn facet_normal(&self, f: usize) -> Vec3 {
        let v = self.facet_area_vector(f);
        v.scale(1.0 / (v.length() + VSMALL))
    }

    /// `facetNormals()`.
    pub fn facet_normals(&self) -> Vec<Vec3> {
        (0..self.size()).map(|f| self.facet_normal(f)).collect()
    }

    /// `facetCentres()`: the vertex average.
    pub fn facet_centres(&self) -> Vec<Vec3> {
        self.triangles
            .iter()
            .map(|t| self.points[t[0]].add(self.points[t[1]]).add(self.points[t[2]]).scale(1.0 / 3.0))
            .collect()
    }

    /// `pointNormals()`: the normalised sum of the facet normals at each
    /// point (zero where the sum is below `VSMALL`).
    pub fn point_normals(&self) -> Vec<Vec3> {
        let fnrm = self.facet_normals();
        self.point_facets
            .iter()
            .map(|pf| {
                let n = pf.iter().fold(Vec3::ZERO, |s, &f| s.add(fnrm[f]));
                let d = n.length();
                if d > VSMALL {
                    n.scale(1.0 / d)
                } else {
                    Vec3::ZERO
                }
            })
            .collect()
    }

    /// `facetFacetsEdges()`: for each facet, every facet sharing one of its
    /// edges, itself included. Upstream lists them in `labelHashSet`
    /// order; the port lists them ascending.
    pub fn facet_facets_edges(&self) -> Vec<Vec<usize>> {
        self.facet_edges
            .iter()
            .map(|fe| {
                let s: BTreeSet<usize> = fe.iter().flat_map(|&e| self.edge_facets[e].iter().copied()).collect();
                s.into_iter().collect()
            })
            .collect()
    }

    /// The stored feature edges as flags on the surface edges (`triSurfacePatchManipulator::allocateFeatureEdges`,
    /// `triSurfaceClassifyEdges`' first loop): `true` for every surface edge
    /// equal to a stored feature edge.
    pub fn stored_feature_edge_flags(&self) -> Vec<bool> {
        let mut flags = vec![false; self.edges.len()];
        for fe in &self.feature_edges {
            if let Some(e) = self.find_edge(fe[0], fe[1]) {
                flags[e] = true;
            }
        }
        flags
    }

    /// Is edge `e` a **feature edge** in upstream's octree sense: shared by
    /// exactly two facets in different regions (`findContainedEdges`)?
    pub fn is_region_edge(&self, e: usize) -> bool {
        let ef = &self.edge_facets[e];
        ef.len() == 2 && self.regions[ef[0]] != self.regions[ef[1]]
    }
}

/// The geometric helpers (`help::` in cfMesh) the octree calls, transcribed.
pub mod help {
    use super::{mag_sqr, BoundBox, ROOTVSMALL, SMALL, VGREAT, VSMALL};
    use crate::math::Vec3;

    /// `help::nearestPointOnTheEdge`: projection onto the **infinite** line
    /// through the edge.
    pub fn nearest_point_on_the_edge(e0: Vec3, e1: Vec3, p: Vec3) -> Vec3 {
        let e = e1.sub(e0);
        let d = e.length();
        let k = p.sub(e0);
        if d < ROOTVSMALL {
            return e0;
        }
        // `edgePoint0 + ((e /(d*d))*(e & k))`: divides, as OpenFOAM does
        e0.add(e.div(d * d).scale(e.dot(k)))
    }

    /// `help::distanceOfPointFromTheEdge`: `mag(nearestPointOnTheEdge(..) - p)`,
    /// which is the distance to the infinite **line** through the edge, as
    /// upstream's (gh:#1385).
    pub fn distance_of_point_from_the_edge(e0: Vec3, e1: Vec3, p: Vec3) -> f64 {
        nearest_point_on_the_edge(e0, e1, p).sub(p).length()
    }

    /// `help::nearestPointOnTheEdgeExact`: the nearest point of the closed
    /// segment.
    pub fn nearest_point_on_the_edge_exact(e0: Vec3, e1: Vec3, p: Vec3) -> Vec3 {
        let e = e1.sub(e0);
        let d_sq = mag_sqr(e);
        let k = p.sub(e0);
        if d_sq < VSMALL {
            return e0;
        }
        let t = e.dot(k) / (d_sq + VSMALL);
        if t > 1.0 {
            e1
        } else if t < 0.0 {
            e0
        } else {
            e0.add(e.scale(t))
        }
    }

    /// `help::nearestPointOnTheTriangle(triangle, p)`, transcribed: the
    /// barycentric projection, clamped to the edge opposite the most negative
    /// coordinate. Upstream's routine, not Ericson's
    /// ([`crate::snap::closest_point_on_triangle`]); the two are compared in
    /// `crate::mesh_octree_queries`' tests.
    pub fn nearest_point_on_the_triangle(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> Vec3 {
        let v0 = b.sub(a);
        let v1 = c.sub(a);
        let v2 = p.sub(a);
        let dot00 = v0.dot(v0);
        let dot01 = v0.dot(v1);
        let dot02 = v0.dot(v2);
        let dot11 = v1.dot(v1);
        let dot12 = v1.dot(v2);
        let det = dot00 * dot11 - dot01 * dot01;
        if det.abs() < VSMALL {
            let mut nearest = p;
            let mut dist = VGREAT;
            for (s, e) in [(a, b), (b, c), (c, a)] {
                let np = nearest_point_on_the_edge(s, e, p);
                if mag_sqr(p.sub(np)) < dist {
                    nearest = np;
                    dist = mag_sqr(p.sub(np));
                }
            }
            return nearest;
        }
        let u = (dot11 * dot02 - dot01 * dot12) / det;
        let v = (dot00 * dot12 - dot01 * dot02) / det;
        let p_proj = a.add(v0.scale(u)).add(v1.scale(v));
        if u >= -SMALL && v >= -SMALL && (u + v) <= 1.0 + SMALL {
            return p_proj;
        }
        if u < -SMALL {
            let ed = p_proj.sub(a).dot(v1) / (mag_sqr(v1) + VSMALL);
            if ed > 1.0 {
                c
            } else if ed < 0.0 {
                a
            } else {
                a.add(v1.scale(ed))
            }
        } else if v < -SMALL {
            let ed = p_proj.sub(a).dot(v0) / (mag_sqr(v0) + VSMALL);
            if ed > 1.0 {
                b
            } else if ed < 0.0 {
                a
            } else {
                a.add(v0.scale(ed))
            }
        } else {
            let ev = b.sub(c);
            let ed = p_proj.sub(c).dot(ev) / (mag_sqr(ev) + VSMALL);
            if ed > 1.0 {
                b
            } else if ed < 0.0 {
                c
            } else {
                c.add(ev.scale(ed))
            }
        }
    }

    /// `help::triLineIntersection`: does the segment `s`–`e` cross triangle
    /// `abc`? Returns the intersection point. Solved by Cramer's rule as
    /// `matrix3D` does (`solveFirst` / `solveSecond` / `solveThird`).
    pub fn tri_line_intersection(a: Vec3, b: Vec3, c: Vec3, s: Vec3, e: Vec3) -> Option<Vec3> {
        let v = s.sub(e);
        let v0 = b.sub(a);
        let v1 = c.sub(a);
        let sp = s.sub(a);
        let col = |w: Vec3| [w.x, w.y, w.z];
        let (c0, c1, c2, src) = (col(v0), col(v1), col(v), col(sp));
        // mat[i] = [v0[i], v1[i], v[i]]
        let m = [[c0[0], c1[0], c2[0]], [c0[1], c1[1], c2[1]], [c0[2], c1[2], c2[2]]];
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        if det.abs() < SMALL {
            return None;
        }
        let solve_first = ((m[1][1] * m[2][2] - m[1][2] * m[2][1]) * src[0]
            + (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * src[1]
            + (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * src[2])
            / det;
        let solve_second = ((m[1][2] * m[2][0] - m[1][0] * m[2][2]) * src[0]
            + (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * src[1]
            + (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * src[2])
            / det;
        let solve_third = ((m[1][0] * m[2][1] - m[1][1] * m[2][0]) * src[0]
            + (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * src[1]
            + (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * src[2])
            / det;
        let t = solve_third;
        if t < -SMALL || t > 1.0 + SMALL {
            return None;
        }
        let u0 = solve_first;
        if u0 < -SMALL {
            return None;
        }
        let u1 = solve_second;
        if u1 < -SMALL || (u0 + u1) > 1.0 + SMALL {
            return None;
        }
        Some(s.sub(v.scale(t)))
    }

    /// `help::boundBoxLineIntersection`: does the segment `s`–`e` meet the
    /// box (slab test, with upstream's `SMALL` tolerances)?
    pub fn bound_box_line_intersection(s: Vec3, e: Vec3, bb: &BoundBox) -> bool {
        let mut t_max = 1.0 + SMALL;
        let mut t_min = -SMALL;
        let v = e.sub(s);
        let d = v.length();
        if d < VSMALL {
            return bb.contains(s);
        }
        for dir in 0..3 {
            let vd = super::comp(v, dir);
            let sd = super::comp(s, dir);
            let (pmin, pmax) = (super::comp(bb.min, dir), super::comp(bb.max, dir));
            if vd.abs() > SMALL * d {
                if vd >= 0.0 {
                    t_min = t_min.max((pmin - sd) / vd);
                    t_max = t_max.min((pmax - sd) / vd);
                } else {
                    t_min = t_min.max((pmax - sd) / vd);
                    t_max = t_max.min((pmin - sd) / vd);
                }
            } else if sd < pmin || sd > pmax {
                return false;
            }
        }
        (t_max - t_min) > -SMALL
    }
}

/// `meshOctreeCubeCoordinates`: a cube's position `(x, y, z)` at `level`.
/// A level-`l` cube spans `1 / 2^l` of the root box per axis.
///
/// The derived `Ord` (level, then x, y, z) is only a key order for ordered
/// containers. Upstream's comparison operators compare **Morton codes**; that
/// is [`CubeCoordinates::morton_less`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CubeCoordinates {
    /// Level in the tree (0 is the root).
    pub level: u8,
    /// Position along x.
    pub x: i64,
    /// Position along y.
    pub y: i64,
    /// Position along z.
    pub z: i64,
}

/// `meshOctreeCubeCoordinates::edgeNodes_`: the two nodes of each of the 12
/// cube edges (x edges, then y, then z). Node `n` sits at
/// `(n & 1, (n >> 1) & 1, (n >> 2) & 1)`.
pub const EDGE_NODES: [[usize; 2]; 12] = [
    [0, 1], [2, 3], [4, 5], [6, 7],
    [0, 2], [1, 3], [4, 6], [5, 7],
    [0, 4], [1, 5], [2, 6], [3, 7],
];
/// `faceNodes_`: the 4 nodes of each face (`-x, +x, -y, +y, -z, +z`),
/// wound outward.
pub const FACE_NODES: [[usize; 4]; 6] = [
    [0, 4, 6, 2],
    [1, 3, 7, 5],
    [0, 1, 5, 4],
    [2, 6, 7, 3],
    [0, 2, 3, 1],
    [4, 5, 7, 6],
];
/// `nodeFaces_`: the 3 faces at each node.
pub const NODE_FACES: [[usize; 3]; 8] = [
    [0, 2, 4], [1, 2, 4], [0, 3, 4], [1, 3, 4],
    [0, 2, 5], [1, 2, 5], [0, 3, 5], [1, 3, 5],
];
/// `faceEdges_`: the 4 edges of each face.
pub const FACE_EDGES: [[usize; 4]; 6] = [
    [8, 6, 10, 4],
    [5, 11, 7, 9],
    [0, 9, 2, 8],
    [10, 3, 11, 1],
    [4, 1, 5, 0],
    [2, 7, 3, 6],
];
/// `edgeFaces_`: the 2 faces at each edge.
pub const EDGE_FACES: [[usize; 2]; 12] = [
    [2, 4], [3, 4], [2, 5], [3, 5],
    [0, 4], [1, 4], [0, 5], [1, 5],
    [0, 2], [1, 2], [0, 3], [1, 3],
];
/// `oppositeFace_`.
pub const OPPOSITE_FACE: [usize; 6] = [1, 0, 3, 2, 5, 4];

impl CubeCoordinates {
    /// Construct from position and level.
    pub const fn new(x: i64, y: i64, z: i64, level: u8) -> Self {
        CubeCoordinates { level, x, y, z }
    }

    /// `refineForPosition(i)`: the child at Z-order position `i` (bit 0 is
    /// +x, bit 1 +y, bit 2 +z). A quadtree cube (negative `z`) keeps its `z`.
    pub fn refine_for_position(&self, i: usize) -> Self {
        let ax = (i & 1) as i64;
        let ay = ((i >> 1) & 1) as i64;
        let az = ((i >> 2) & 1) as i64;
        let z = if self.z < 0 { self.z } else { 2 * self.z + az };
        CubeCoordinates::new(2 * self.x + ax, 2 * self.y + ay, z, self.level + 1)
    }

    /// `reduceLevelBy(diff)`: the ancestor `diff` levels up (C integer
    /// division, as upstream).
    pub fn reduce_level_by(&self, diff: u8) -> Self {
        let d = 1i64 << diff;
        let z = if self.z < 0 { self.z } else { self.z / d };
        CubeCoordinates::new(self.x / d, self.y / d, z, self.level - diff)
    }

    /// `reduceToLevel(l)`: the ancestor at level `l`.
    pub fn reduce_to_level(&self, l: u8) -> Self {
        self.reduce_level_by(self.level - l)
    }

    /// `increaseToLevelMin(l)`: the minimal descendant at level `l`.
    pub fn increase_to_level_min(&self, l: u8) -> Self {
        let d = l - self.level;
        let z = if self.z < 0 { self.z } else { self.z << d };
        CubeCoordinates::new(self.x << d, self.y << d, z, l)
    }

    /// `increaseToLevelMax(l)`: the maximal descendant at level `l`.
    pub fn increase_to_level_max(&self, l: u8) -> Self {
        let d = l - self.level;
        CubeCoordinates::new(
            ((self.x + 1) << d) - 1,
            ((self.y + 1) << d) - 1,
            if self.z < 0 { self.z } else { ((self.z + 1) << d) - 1 },
            l,
        )
    }

    /// `operator+`: component-wise sum, levels added.
    pub fn plus(&self, o: &CubeCoordinates) -> Self {
        CubeCoordinates::new(self.x + o.x, self.y + o.y, self.z + o.z, self.level + o.level)
    }

    /// `cubeBox`: the cube's corners in the root box. A quadtree cube
    /// (negative `z`) spans the root box in z.
    pub fn cube_box(&self, root: &BoundBox) -> BoundBox {
        let shift = (1i64 << self.level) as f64;
        let dc = root.span();
        let dz = if self.z >= 0 { dc.z / shift } else { dc.z };
        let dc = Vec3::new(dc.x / shift, dc.y / shift, dz);
        let min = Vec3::new(
            root.min.x + dc.x * self.x as f64,
            root.min.y + dc.y * self.y as f64,
            if self.z >= 0 { root.min.z + dc.z * self.z as f64 } else { root.min.z },
        );
        BoundBox::new(min, min.add(dc))
    }

    /// `centre`.
    pub fn centre(&self, root: &BoundBox) -> Vec3 {
        let b = self.cube_box(root);
        b.max.add(b.min).scale(0.5)
    }

    /// `size`: the cube's edge length (x extent).
    pub fn size(&self, root: &BoundBox) -> f64 {
        let b = self.cube_box(root);
        b.max.x - b.min.x
    }

    /// `vertices`: the 8 corners, inflated by `SMALL * root span`.
    pub fn vertices(&self, root: &BoundBox) -> [Vec3; 8] {
        let tol = root.span().scale(SMALL);
        let b = self.cube_box(root);
        let (mn, mx) = (b.min.sub(tol), b.max.add(tol));
        let mut v = [Vec3::ZERO; 8];
        for (n, slot) in v.iter_mut().enumerate() {
            *slot = Vec3::new(
                if n & 1 == 0 { mn.x } else { mx.x },
                if n & 2 == 0 { mn.y } else { mx.y },
                if n & 4 == 0 { mn.z } else { mx.z },
            );
        }
        v
    }

    /// `edgeVertices`: the 12 edges from [`Self::vertices`].
    pub fn edge_vertices(&self, root: &BoundBox) -> [[Vec3; 2]; 12] {
        let v = self.vertices(root);
        let mut e = [[Vec3::ZERO; 2]; 12];
        for (i, en) in EDGE_NODES.iter().enumerate() {
            e[i] = [v[en[0]], v[en[1]]];
        }
        e
    }

    /// `intersectsTriangle`: the **bounding boxes** of cube and triangle
    /// overlap (both inflated by `SMALL * root span`).
    pub fn intersects_triangle(&self, surf: &TriSurf, root: &BoundBox, t: usize) -> bool {
        let tol = root.span().scale(SMALL);
        let mut c = self.cube_box(root);
        c.min = c.min.sub(tol);
        c.max = c.max.add(tol);
        let tri = surf.triangles[t];
        let mut tb = BoundBox::new(surf.points[tri[0]], surf.points[tri[0]]);
        for &p in &tri[1..] {
            tb.max = vmax(surf.points[p], tb.max);
            tb.min = vmin(surf.points[p], tb.min);
        }
        tb.min = tb.min.sub(tol);
        tb.max = tb.max.add(tol);
        c.overlaps(&tb)
    }

    /// `intersectsTriangleExact`: the triangle meets the cube. Upstream's
    /// three tests in order: a triangle vertex inside the cube, a triangle
    /// edge crossing the cube, a cube edge crossing the triangle.
    pub fn intersects_triangle_exact(&self, surf: &TriSurf, root: &BoundBox, t: usize) -> bool {
        if !self.intersects_triangle(surf, root, t) {
            return false;
        }
        let tol = root.span().scale(SMALL);
        let tri = surf.triangles[t];
        if tri.iter().any(|&p| self.is_vertex_inside(root, surf.points[p])) {
            return true;
        }
        let mut bb = self.cube_box(root);
        bb.min = bb.min.sub(tol);
        bb.max = bb.max.add(tol);
        for e in 0..3 {
            let s = surf.points[tri[e]];
            let en = surf.points[tri[(e + 1) % 3]];
            if help::bound_box_line_intersection(s, en, &bb) {
                return true;
            }
        }
        let (a, b, c) = (surf.points[tri[0]], surf.points[tri[1]], surf.points[tri[2]]);
        self.edge_vertices(root)
            .iter()
            .any(|e| help::tri_line_intersection(a, b, c, e[0], e[1]).is_some())
    }

    /// `isVertexInside`: `p` lies in the cube inflated by
    /// `SMALL * root span`.
    pub fn is_vertex_inside(&self, root: &BoundBox, p: Vec3) -> bool {
        let tol = root.span().scale(SMALL);
        let b = self.cube_box(root);
        let (mn, mx) = (b.min.sub(tol), b.max.add(tol));
        !(p.x - mx.x > 0.0
            || p.y - mx.y > 0.0
            || p.z - mx.z > 0.0
            || p.x - mn.x < 0.0
            || p.y - mn.y < 0.0
            || p.z - mn.z < 0.0)
    }

    /// `isPositionInside`: is `cc` this cube or a descendant of it? `None`
    /// where upstream aborts (`cc` coarser than this cube).
    pub fn is_position_inside(&self, cc: &CubeCoordinates) -> Option<bool> {
        if cc.level >= self.level {
            Some(cc.reduce_level_by(cc.level - self.level) == *self)
        } else {
            None
        }
    }

    /// `intersectsLine`: does the segment `s`–`e` cross a face of the cube
    /// (inflated by `SMALL * root x span`), or start inside it?
    pub fn intersects_line(&self, root: &BoundBox, s: Vec3, e: Vec3) -> bool {
        let tol = SMALL * (root.max.x - root.min.x);
        let b = self.cube_box(root);
        let tv = Vec3::new(tol, tol, tol);
        let (mn, mx) = (b.min.sub(tv), b.max.add(tv));
        let v = e.sub(s);
        for dir in 0..3 {
            let vd = comp(v, dir);
            if vd.abs() <= tol {
                continue;
            }
            let (o1, o2) = ((dir + 1) % 3, (dir + 2) % 3);
            // Upstream checks the two other axes in (x, y, z) order; the
            // conjunction is order-independent.
            for plane in [comp(mn, dir), comp(mx, dir)] {
                let t = (plane - comp(s, dir)) / vd;
                let i = s.add(v.scale(t));
                if t > -tol
                    && t < 1.0 + tol
                    && comp(i, o1) - comp(mn, o1) > -tol
                    && comp(i, o1) - comp(mx, o1) < tol
                    && comp(i, o2) - comp(mn, o2) > -tol
                    && comp(i, o2) - comp(mx, o2) < tol
                {
                    return true;
                }
            }
        }
        self.is_vertex_inside(root, s)
    }

    /// `neighbourRange`: the range of positions one level finer that can
    /// neighbour this cube.
    pub fn neighbour_range(&self) -> (CubeCoordinates, CubeCoordinates) {
        let mut mn = self.refine_for_position(0);
        if mn.x > 0 {
            mn.x -= 1;
        }
        if mn.y > 0 {
            mn.y -= 1;
        }
        if mn.z > 0 {
            mn.z -= 1;
        }
        let mut mx = self.refine_for_position(7);
        let maxc = (1i64 << mx.level) - 1;
        if mx.x < maxc {
            mx.x += 1;
        }
        if mx.y < maxc {
            mx.y += 1;
        }
        if mx.z < maxc {
            mx.z += 1;
        }
        (mn, mx)
    }

    /// Upstream's `operator<`: compare **Morton codes** after reducing both
    /// to the coarser level. The axis is the one with the most significant
    /// differing bit (`(max ^ res) < res` picks the larger leading bit),
    /// ties to the later axis.
    pub fn morton_less(&self, cc: &CubeCoordinates) -> bool {
        let min_l = self.level.min(cc.level);
        let a = self.reduce_to_level(min_l);
        let b = cc.reduce_to_level(min_l);
        let (rx, ry, rz) = (a.x ^ b.x, a.y ^ b.y, a.z ^ b.z);
        let mut max = rx;
        let mut dir = 0;
        if max <= ry || (max ^ ry) < ry {
            max = ry;
            dir = 1;
        }
        if max <= rz || (max ^ rz) < rz {
            dir = 2;
        }
        match dir {
            0 => a.x < b.x,
            1 => a.y < b.y,
            _ => a.z < b.z,
        }
    }
}

/// `meshOctreeCubeBasic::typesOfCubes`, as bit flags (upstream tests them
/// with `&`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CubeType;

impl CubeType {
    /// Not yet classified.
    pub const UNKNOWN: u8 = 1;
    /// Outside the surface.
    pub const OUTSIDE: u8 = 2;
    /// Intersects the surface.
    pub const DATA: u8 = 4;
    /// Inside the surface.
    pub const INSIDE: u8 = 8;
}

/// `meshOctreeCube`: a node of the tree. Indices refer to the octree's
/// [`OctreeSlot`].
#[derive(Debug, Clone, PartialEq)]
pub struct OctreeCube {
    /// Position and level.
    pub coords: CubeCoordinates,
    /// [`CubeType`] flags.
    pub cube_type: u8,
    /// Row of [`OctreeSlot::child_cubes`] holding the 8 children, or `None`
    /// for a leaf (`subCubesPtr_`).
    pub sub_cubes: Option<usize>,
    /// Position in the leaf list (`cubeLabel_`); `None` for a non-leaf.
    pub cube_label: Option<usize>,
    /// Row of [`OctreeSlot::contained_triangles`] (`containedElementsLabel_`).
    pub contained_elements: Option<usize>,
    /// Row of [`OctreeSlot::contained_edges`] (`containedEdgesLabel_`).
    pub contained_edges: Option<usize>,
}

impl OctreeCube {
    /// `meshOctreeCube(const meshOctreeCubeCoordinates&)`: type `UNKNOWN`,
    /// no children, nothing contained.
    pub fn new(coords: CubeCoordinates) -> Self {
        OctreeCube {
            coords,
            cube_type: CubeType::UNKNOWN,
            sub_cubes: None,
            cube_label: None,
            contained_elements: None,
            contained_edges: None,
        }
    }

    /// `isLeaf`.
    pub fn is_leaf(&self) -> bool {
        self.sub_cubes.is_none()
    }

    /// `hasContainedElements`.
    pub fn has_contained_elements(&self) -> bool {
        self.contained_elements.is_some()
    }

    /// `hasContainedEdges`.
    pub fn has_contained_edges(&self) -> bool {
        self.contained_edges.is_some()
    }
}

/// `meshOctreeSlot`: the storage of the tree. Upstream keeps one per
/// OpenMP thread; the port is serial and keeps one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OctreeSlot {
    /// All cubes ever created (`cubes_`); index 0 is the root.
    pub cubes: Vec<OctreeCube>,
    /// Children of refined cubes (`childCubes_`, `FRWGraph<cube*, 8>`).
    pub child_cubes: Vec<[usize; 8]>,
    /// Surface triangles in a cube (`containedTriangles_`, a `VRWGraph`).
    pub contained_triangles: Vec<Vec<usize>>,
    /// Feature edges in a cube (`containedEdges_`).
    pub contained_edges: Vec<Vec<usize>>,
    /// Triangles a refinement did not hand to any child (upstream prints
    /// "Triangle ... is not transferred to the child cubes!"). Expected 0.
    pub n_untransferred: usize,
}

/// `meshOctree`: the surface octree.
#[derive(Debug, Clone)]
pub struct MeshOctree {
    /// The surface the octree is built on (`surface_`).
    pub surface: Arc<TriSurf>,
    /// The root box (`rootBox_`).
    pub root_box: BoundBox,
    /// `isRootInitialised_`: the creator has fixed the root box size.
    pub is_root_initialised: bool,
    /// `searchRange_`: the search half-width used when a point lies in no
    /// leaf.
    pub search_range: f64,
    /// The storage (`dataSlots_[0]`).
    pub slot: OctreeSlot,
    /// The leaves (`leaves_`), as indices into `slot.cubes`.
    pub leaves: Vec<usize>,
    /// `octantVectors_`.
    pub octant_vectors: [[i64; 3]; 8],
    /// `regularityPositions_`: offsets to the 26 face, edge and node
    /// neighbours.
    pub regularity_positions: [CubeCoordinates; 26],
    /// `vrtLeavesPos_`: for each cube node, the 8 positions (relative to the
    /// cube, one level finer than the neighbours searched) of the leaves
    /// sharing it.
    pub vrt_leaves_pos: [[CubeCoordinates; 8]; 8],
    /// `isQuadtree_` (gh:#1275): the 2D workflow's quadtree.
    pub is_quadtree: bool,
}

/// An empty child slot of a quadtree cube (upstream's `nullptr` in
/// `childCubes_` slots 4–7).
pub const NO_CUBE: usize = usize::MAX;

/// Index of the root cube in [`OctreeSlot::cubes`].
pub const ROOT: usize = 0;

impl MeshOctree {
    /// `meshOctree(const triSurf&)`: the root cube over the surface, holding
    /// every triangle. The root box is centred on the surface's bounding box
    /// with half-width `1.5 x` the largest half-extent
    /// (`createInitialOctreeBox`).
    pub fn new(surface: Arc<TriSurf>) -> Self {
        let bb = BoundBox::of_points(&surface.points);
        let c = bb.max.add(bb.min).scale(0.5);
        let mut cs = 1.5 * (bb.max.x - bb.min.x) / 2.0;
        if cs < 1.5 * (bb.max.y - bb.min.y) / 2.0 {
            cs = 1.5 * (bb.max.y - bb.min.y) / 2.0;
        }
        if cs < 1.5 * (bb.max.z - bb.min.z) / 2.0 {
            cs = 1.5 * (bb.max.z - bb.min.z) / 2.0;
        }
        let root_box = BoundBox::new(c.sub(Vec3::new(cs, cs, cs)), c.add(Vec3::new(cs, cs, cs)));
        let n = surface.size();
        let mut slot = OctreeSlot::default();
        let mut root = OctreeCube::new(CubeCoordinates::new(0, 0, 0, 0));
        root.cube_label = Some(0);
        root.contained_elements = Some(0);
        slot.contained_triangles.push((0..n).collect());
        slot.cubes.push(root);
        let (octant_vectors, regularity_positions, vrt_leaves_pos) = octant_vectors_and_positions();
        MeshOctree {
            surface,
            root_box,
            is_root_initialised: false,
            search_range: 0.0,
            slot,
            leaves: vec![ROOT],
            octant_vectors,
            regularity_positions,
            vrt_leaves_pos,
            is_quadtree: false,
        }
    }

    /// `meshOctree(surf, true)` (gh:#1275): the quadtree of the 2D
    /// workflow. The root box is [`MeshOctree::new`]'s; the root cube is
    /// `(0, 0, -10, 0)`, so every cube spans the root box in z.
    pub fn new_quadtree(surface: Arc<TriSurf>) -> Self {
        let mut o = MeshOctree::new(surface);
        o.is_quadtree = true;
        o.slot.cubes[ROOT].coords = CubeCoordinates::new(0, 0, -10, 0);
        o
    }

    /// The children of cube `i` that exist (4 for a quadtree cube).
    pub fn existing_sub_cubes(&self, i: usize) -> Vec<usize> {
        self.sub_cubes(i).map_or(Vec::new(), |s| s.into_iter().filter(|&c| c != NO_CUBE).collect())
    }

    /// The cube at slot index `i`.
    pub fn cube(&self, i: usize) -> &OctreeCube {
        &self.slot.cubes[i]
    }

    /// The 8 children of cube `i`, if it is refined.
    pub fn sub_cubes(&self, i: usize) -> Option<[usize; 8]> {
        self.slot.cubes[i].sub_cubes.map(|r| self.slot.child_cubes[r])
    }

    /// `numberOfLeaves`.
    pub fn number_of_leaves(&self) -> usize {
        self.leaves.len()
    }

    /// `returnLeaf`: the leaf cube with leaf label `l`.
    pub fn return_leaf(&self, l: usize) -> &OctreeCube {
        &self.slot.cubes[self.leaves[l]]
    }

    /// `hasContainedTriangles(leafI)`.
    pub fn has_contained_triangles(&self, l: usize) -> bool {
        self.return_leaf(l).has_contained_elements()
    }

    /// `containedTriangles(leafI)`.
    pub fn contained_triangles(&self, l: usize) -> Vec<usize> {
        match self.return_leaf(l).contained_elements {
            Some(r) => self.slot.contained_triangles[r].clone(),
            None => Vec::new(),
        }
    }

    /// `hasContainedEdges(leafI)`.
    pub fn has_contained_edges(&self, l: usize) -> bool {
        self.return_leaf(l).has_contained_edges()
    }

    /// `containedEdges(leafI)`.
    pub fn contained_edges(&self, l: usize) -> Vec<usize> {
        match self.return_leaf(l).contained_edges {
            Some(r) => self.slot.contained_edges[r].clone(),
            None => Vec::new(),
        }
    }

    /// `findBoundaryPatchesForLeaf` (`meshOctreeCubePatches.C`): the regions
    /// of the triangles in leaf `l`, in first-appearance order.
    pub fn find_boundary_patches_for_leaf(&self, l: usize) -> Vec<usize> {
        let mut patches: Vec<usize> = Vec::new();
        if let Some(r) = self.return_leaf(l).contained_elements {
            for &t in &self.slot.contained_triangles[r] {
                let reg = self.surface.regions[t];
                if !patches.contains(&reg) {
                    patches.push(reg);
                }
            }
        }
        patches
    }

    /// `meshOctreeCube::refineCube`: split cube `ci` into its 8 children in Z
    /// order. Children inherit the parent's type; the parent's triangles go
    /// to every child they intersect (`intersectsTriangleExact`); a child
    /// with triangles collects its feature edges (`findContainedEdges`); a
    /// child of a `DATA` parent that receives none becomes `UNKNOWN`.
    ///
    /// The leaf list is **not** updated; call
    /// [`MeshOctree::create_list_of_leaves`] after a refinement pass, as
    /// upstream does.
    pub fn refine_cube(&mut self, ci: usize) {
        let parent = self.slot.cubes[ci].clone();
        self.slot.cubes[ci].cube_label = None;
        // refineCube2D: 4 children, slots 4-7 empty
        let n_sub = if self.is_quadtree { 4 } else { 8 };
        let mut subs = [NO_CUBE; 8];
        for (sc, s) in subs.iter_mut().enumerate().take(n_sub) {
            let mut child = OctreeCube::new(parent.coords.refine_for_position(sc));
            child.cube_type = parent.cube_type;
            *s = self.slot.cubes.len();
            self.slot.cubes.push(child);
        }
        let row = self.slot.child_cubes.len();
        self.slot.child_cubes.push(subs);
        self.slot.cubes[ci].sub_cubes = Some(row);

        let Some(er) = parent.contained_elements else {
            return;
        };
        let root = self.root_box;
        let surf = Arc::clone(&self.surface);
        let mut in_sub: [Vec<usize>; 8] = Default::default();
        let elements = self.slot.contained_triangles[er].clone();
        for &t in &elements {
            let mut used = false;
            for sc in 0..n_sub {
                if self.slot.cubes[subs[sc]].coords.intersects_triangle_exact(&surf, &root, t) {
                    used = true;
                    in_sub[sc].push(t);
                }
            }
            if !used {
                self.slot.n_untransferred += 1;
            }
        }
        for sc in 0..n_sub {
            if !in_sub[sc].is_empty() {
                self.slot.cubes[subs[sc]].contained_elements = Some(self.slot.contained_triangles.len());
                self.slot.contained_triangles.push(std::mem::take(&mut in_sub[sc]));
            }
        }
        for &s in subs.iter().take(n_sub) {
            if self.slot.cubes[s].has_contained_elements() {
                self.find_contained_edges(s);
            } else if self.slot.cubes[s].cube_type & CubeType::DATA != 0 {
                self.slot.cubes[s].cube_type = CubeType::UNKNOWN;
            }
        }
    }

    /// `meshOctreeCube::findContainedEdges`: the feature edges (two facets,
    /// two regions) of the cube's triangles that cross the cube
    /// (`intersectsLine`).
    fn find_contained_edges(&mut self, ci: usize) {
        let Some(er) = self.slot.cubes[ci].contained_elements else {
            return;
        };
        let surf = Arc::clone(&self.surface);
        let coords = self.slot.cubes[ci].coords;
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut added: Vec<usize> = Vec::new();
        for &f in &self.slot.contained_triangles[er] {
            for &e in &surf.facet_edges[f] {
                if seen.contains(&e) || !surf.is_region_edge(e) {
                    continue;
                }
                let [s, en] = surf.edges[e];
                if coords.intersects_line(&self.root_box, surf.points[s], surf.points[en]) {
                    seen.insert(e);
                    added.push(e);
                }
            }
        }
        if !added.is_empty() {
            self.slot.cubes[ci].contained_edges = Some(self.slot.contained_edges.len());
            self.slot.contained_edges.push(added);
        }
    }

    /// `meshOctreeModifier::createListOfLeaves` (`meshOctreeCube::findLeaves`
    /// from the root): depth first, children in Z order; every leaf gets its
    /// `cube_label`, every non-leaf `None`.
    pub fn create_list_of_leaves(&mut self) {
        let mut leaves = Vec::new();
        self.find_leaves(ROOT, &mut leaves);
        self.leaves = leaves;
    }

    fn find_leaves(&mut self, ci: usize, leaves: &mut Vec<usize>) {
        match self.sub_cubes(ci) {
            None => {
                self.slot.cubes[ci].cube_label = Some(leaves.len());
                leaves.push(ci);
            }
            Some(subs) => {
                self.slot.cubes[ci].cube_label = None;
                for s in subs {
                    if s != NO_CUBE {
                        self.find_leaves(s, leaves);
                    }
                }
            }
        }
    }

    /// `meshOctreeCube::leavesInBox` from the root: slot indices of the
    /// leaves whose box overlaps `bb`, in tree order.
    pub fn leaves_in_box(&self, bb: &BoundBox) -> Vec<usize> {
        let mut out = Vec::new();
        self.leaves_in_box_from(ROOT, bb, &mut out);
        out
    }

    fn leaves_in_box_from(&self, ci: usize, bb: &BoundBox, out: &mut Vec<usize>) {
        if self.slot.cubes[ci].coords.cube_box(&self.root_box).overlaps(bb) {
            match self.sub_cubes(ci) {
                None => out.push(ci),
                Some(subs) => {
                    for s in subs {
                        if s != NO_CUBE {
                            self.leaves_in_box_from(s, bb, out);
                        }
                    }
                }
            }
        }
    }

    /// `meshOctreeCube::leavesInSphere` from the root: leaf labels of the
    /// leaves whose centre is within `r + 1.732 x size` of `c` (upstream's
    /// conservative sphere test, `size` of the cube being tested).
    pub fn leaves_in_sphere(&self, c: Vec3, r: f64) -> Vec<usize> {
        let mut out = Vec::new();
        self.leaves_in_sphere_from(ROOT, c, r, &mut out);
        out
    }

    fn leaves_in_sphere_from(&self, ci: usize, c: Vec3, r: f64, out: &mut Vec<usize>) {
        let cube = &self.slot.cubes[ci];
        let cc = cube.coords.centre(&self.root_box);
        let size = 1.732 * cube.coords.size(&self.root_box);
        if mag_sqr(cc.sub(c)) < (r + size) * (r + size) {
            match self.sub_cubes(ci) {
                None => {
                    if let Some(l) = cube.cube_label {
                        out.push(l);
                    }
                }
                Some(subs) => {
                    for s in subs {
                        if s != NO_CUBE {
                            self.leaves_in_sphere_from(s, c, r, out);
                        }
                    }
                }
            }
        }
    }

    /// `meshOctreeCube::markLeavesInSphere` from the root: or `2` into
    /// `marked[leaf]` for every leaf [`MeshOctree::leaves_in_sphere`] finds.
    pub fn mark_leaves_in_sphere(&self, c: Vec3, r: f64, marked: &mut [u8]) {
        for l in self.leaves_in_sphere(c, r) {
            marked[l] |= 2;
        }
    }

    /// `meshOctreeCube::countChildCubes` from the root: every cube in the
    /// tree, the root included.
    pub fn count_child_cubes(&self) -> usize {
        fn count(o: &MeshOctree, ci: usize) -> usize {
            1 + o.existing_sub_cubes(ci).iter().map(|&c| count(o, c)).sum::<usize>()
        }
        count(self, ROOT)
    }
}

/// `setOctantVectorsAndPositions`.
type OctantTables = ([[i64; 3]; 8], [CubeCoordinates; 26], [[CubeCoordinates; 8]; 8]);

fn octant_vectors_and_positions() -> OctantTables {
    let ov: [[i64; 3]; 8] = [
        [-1, -1, -1],
        [1, -1, -1],
        [-1, 1, -1],
        [1, 1, -1],
        [-1, -1, 1],
        [1, -1, 1],
        [-1, 1, 1],
        [1, 1, 1],
    ];
    let c = |x, y, z| CubeCoordinates::new(x, y, z, 0);
    let rp = [
        c(-1, 0, 0), c(1, 0, 0), c(0, -1, 0), c(0, 1, 0), c(0, 0, -1), c(0, 0, 1),
        c(0, -1, -1), c(0, 1, -1), c(0, -1, 1), c(0, 1, 1),
        c(-1, 0, -1), c(1, 0, -1), c(-1, 0, 1), c(1, 0, 1),
        c(-1, -1, 0), c(1, -1, 0), c(-1, 1, 0), c(1, 1, 0),
        c(-1, -1, -1), c(1, -1, -1), c(-1, 1, -1), c(1, 1, -1),
        c(-1, -1, 1), c(1, -1, 1), c(-1, 1, 1), c(1, 1, 1),
    ];
    let mut vlp = [[c(0, 0, 0); 8]; 8];
    for (vrt, row) in vlp.iter_mut().enumerate() {
        let vc = [(vrt & 1) as i64, ((vrt >> 1) & 1) as i64, ((vrt >> 2) & 1) as i64];
        for (i, slot) in row.iter_mut().enumerate() {
            let mut pos = [0i64; 3];
            for j in 0..3 {
                pos[j] = if vc[j] == 0 && ov[i][j] == 1 {
                    0
                } else if vc[j] == 1 && ov[i][j] == 1 {
                    1
                } else {
                    vc[j] + ov[i][j]
                };
            }
            *slot = c(pos[0], pos[1], pos[2]);
        }
    }
    (ov, rp, vlp)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::shapes::{box_surface, sphere_surface};

    /// Refine every leaf holding triangles until `level` (the `DATA`
    /// refinement `meshOctreeCreator::refineBoundary` performs, without its
    /// size control), then list the leaves.
    pub(crate) fn data_octree(surf: Arc<TriSurf>, level: u8) -> MeshOctree {
        let mut o = MeshOctree::new(surf);
        for _ in 0..level {
            let to_refine: Vec<usize> = o
                .leaves
                .iter()
                .copied()
                .filter(|&c| o.cube(c).has_contained_elements())
                .collect();
            for c in to_refine {
                o.refine_cube(c);
            }
            o.create_list_of_leaves();
        }
        o
    }

    /// Exact triangle-box overlap by the separating-axis theorem
    /// (Akenine-Moller 2001): the 3 box normals, the triangle normal and the
    /// 9 edge cross products. An independent oracle for
    /// `intersectsTriangleExact`; it shares no code with it.
    pub(crate) fn sat_tri_box(a: Vec3, b: Vec3, c: Vec3, bmin: Vec3, bmax: Vec3) -> bool {
        let centre = bmin.add(bmax).scale(0.5);
        let h = bmax.sub(bmin).scale(0.5);
        let v = [a.sub(centre), b.sub(centre), c.sub(centre)];
        let e = [v[1].sub(v[0]), v[2].sub(v[1]), v[0].sub(v[2])];
        let axes_box = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        let separated = |ax: Vec3| -> bool {
            if ax.dot(ax) < 1e-300 {
                return false;
            }
            let p: Vec<f64> = v.iter().map(|w| w.dot(ax)).collect();
            let r = h.x * ax.x.abs() + h.y * ax.y.abs() + h.z * ax.z.abs();
            let (mn, mx) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            mn > r || mx < -r
        };
        for ab in axes_box {
            if separated(ab) {
                return false;
            }
            for ed in e {
                if separated(ab.cross(ed)) {
                    return false;
                }
            }
        }
        !separated(e[0].cross(e[1]))
    }

    fn surf_of((p, t): (Vec<Vec3>, Vec<[usize; 3]>)) -> Arc<TriSurf> {
        Arc::new(TriSurf::new(p, t, None))
    }

    const NL_SPHERE: usize = 1184;
    const ND_SPHERE: usize = 536;
    const NT_SPHERE: usize = 380;

    /// V&V (gh:#1262): the **DATA set** of the ported octree equals a
    /// brute-force triangle-box overlap over every leaf, exactly.
    ///
    /// # Methodology
    ///
    /// Build the octree over `box_surface([0, 2]^3)` (12 triangles) and
    /// `sphere_surface(R 3, 24 x 48)` (2 208 triangles), refining every leaf
    /// that holds triangles four times (levels 0 to 4). For **every leaf**
    /// and **every triangle** of the surface, test overlap with an
    /// independent separating-axis test ([`sat_tri_box`]) on the leaf's
    /// exact (uninflated) box, and compare with the triangles the octree
    /// stored in that leaf.
    ///
    /// **Prediction (written before the run):** identical sets on both
    /// surfaces. Upstream inflates by `1e-15 x` the root span, so the two
    /// can differ only for a triangle touching a leaf to within ~1e-15, and
    /// I expected no such touch ("neither surface is aligned with this
    /// octree's lattice"). Also predicted: no triangle is lost in refinement
    /// (`n_untransferred` 0).
    ///
    /// # Results (2026-11-05)
    ///
    /// - Box: as predicted, identical sets; 1 520 leaves, 728 holding
    ///   triangles, 0 untransferred.
    /// - Sphere: **the prediction missed.** Leaf `(4; 6, 5, 3)` holds
    ///   triangle 1834 and the uninflated separating-axis test says no. The
    ///   probe showed a genuine touch: the triangle's edge runs along
    ///   `x = y` and meets the leaf's edge at `(-1.125, -1.125)` to within
    ///   1e-15 (the uninflated SAT rejects; the SAT with any inflation from
    ///   1e-15 up accepts). The root box is centred on the sphere, so the
    ///   lattice planes `x = 0`, `y = 0`, `z = 0` and the diagonals carry
    ///   sphere vertices: my "not aligned" premise was wrong for the sphere.
    /// - The oracle was then set to **upstream's definition**, the leaf box
    ///   inflated by `SMALL x` root span (the set `intersectsTriangleExact`
    ///   is specified to test), and both surfaces match it exactly on every
    ///   leaf. The touches it admits are counted and pinned: 0 on the box,
    ///   380 (leaf, triangle) pairs on the sphere (1 184 leaves, 536 holding
    ///   triangles, 0 untransferred).
    #[test]
    fn data_leaves_equal_brute_force_separating_axis() {
        for (name, surf, n_leaves, n_data, n_touch) in [
            ("box", surf_of(box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0))), 1520, 728, 0),
            ("sphere", surf_of(sphere_surface(Vec3::ZERO, 3.0, 24, 48)), NL_SPHERE, ND_SPHERE, NT_SPHERE),
        ] {
            let o = data_octree(Arc::clone(&surf), 4);
            assert_eq!(o.slot.n_untransferred, 0, "{name}: triangles lost in refinement");
            let tol = o.root_box.span().scale(SMALL);
            let mut data = 0;
            let mut touch = 0;
            for l in 0..o.number_of_leaves() {
                let b = o.return_leaf(l).coords.cube_box(&o.root_box);
                let sat = |t: usize, g: Vec3| {
                    let tr = surf.triangles[t];
                    sat_tri_box(surf.points[tr[0]], surf.points[tr[1]], surf.points[tr[2]], b.min.sub(g), b.max.add(g))
                };
                let brute: Vec<usize> = (0..surf.size()).filter(|&t| sat(t, tol)).collect();
                touch += brute.iter().filter(|&&t| !sat(t, Vec3::ZERO)).count();
                let mut got = o.contained_triangles(l);
                got.sort_unstable();
                assert_eq!(got, brute, "{name}: leaf {l} {:?}", o.return_leaf(l).coords);
                if !got.is_empty() {
                    data += 1;
                }
            }
            println!("{name}: {} leaves, {data} with triangles, {touch} (leaf, triangle) touches within the tolerance", o.number_of_leaves());
            assert_eq!((o.number_of_leaves(), data, touch), (n_leaves, n_data, n_touch), "{name} counts");
            // Every leaf holding triangles is at level 4; the others are coarser or equal.
            for l in 0..o.number_of_leaves() {
                let c = o.return_leaf(l);
                assert!(c.coords.level <= 4);
                if c.has_contained_elements() {
                    assert_eq!(c.coords.level, 4);
                }
            }
        }
    }

    /// V&V (gh:#1262), differential: the octree's **DATA leaves** against the
    /// leaves [`crate::octree`]'s two criteria select, on the same lattice.
    ///
    /// # Methodology
    ///
    /// Sphere R 3 m (24 x 48). [`crate::octree::refine_near_boundary`] at
    /// base size 0.5 m works on the lattice with origin `min - 0.5`; here the
    /// ported octree is given the root box `[origin, origin + 16 x 0.5]`
    /// (upstream sets the root box this way in `meshOctreeCreator`) so its
    /// level-4 cubes are exactly that lattice's cells. Sets compared:
    /// - `A`: `TouchesWall` (centre inside, a face neighbour's centre
    ///   outside), the leaves `refine_near_boundary` splits;
    /// - `A1`: `DistanceBand(1.0)` (centre inside, distance to the surface
    ///   below 0.5 m), what `refine_near_boundary_banded` splits;
    /// - `B`: level-4 cubes intersecting the surface (DATA);
    /// - `B_in`: `B` with an inside centre.
    ///
    /// **Predictions (written before the run):**
    /// 1. `B_in` is a subset of `A1` (a DATA cube's centre is within half a
    ///    diagonal, 0.433 cell, of the surface) — exact, asserted.
    /// 2. Every cell of `A` is in `B` or has a face neighbour in `B` (the
    ///    surface crosses the segment between the two centres) — asserted.
    /// 3. `|B| > |A|` (DATA takes cells on both sides of the surface).
    ///
    /// # Results (2026-11-05)
    ///
    /// `|A|` 336, `|A1|` 360, `|B|` 680, `|B_in|` 312, `|A and B|` 288
    /// (pinned). All three predictions hold. So the existing
    /// `TouchesWall` shell misses 24 inside cells the surface crosses
    /// (`B_in \ A`) and refines 48 cells it does not cross (`A \ B`: the
    /// surface passes through the outside neighbour instead);
    /// `DistanceBand(1.0)` covers every inside DATA cell plus 48 more. The
    /// upstream criterion is the intersection test, on both sides of the
    /// surface (368 of the 680 DATA cells have an outside centre, which the
    /// crate's carve drops).
    #[test]
    fn data_leaves_against_octree_rs_refinement_criteria() {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        let surf = surf_of((p.clone(), t.clone()));
        let cs = 0.5;
        let lo = BoundBox::of_points(&p).min;
        let origin = lo.sub(Vec3::new(cs, cs, cs));
        let mut o = MeshOctree::new(Arc::clone(&surf));
        o.root_box = BoundBox::new(origin, origin.add(Vec3::new(16.0 * cs, 16.0 * cs, 16.0 * cs)));
        for _ in 0..4 {
            let to: Vec<usize> = o.leaves.iter().copied().filter(|&c| o.cube(c).has_contained_elements()).collect();
            for c in to {
                o.refine_cube(c);
            }
            o.create_list_of_leaves();
        }
        let centre = |i: i64, j: i64, k: i64| {
            Vec3::new(origin.x + cs * (i as f64 + 0.5), origin.y + cs * (j as f64 + 0.5), origin.z + cs * (k as f64 + 0.5))
        };
        let ins = |i, j, k| crate::octree::inside(centre(i, j, k), &p, &t);
        let n = 16i64;
        let mut a = BTreeSet::new();
        let mut a1 = BTreeSet::new();
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    if !ins(i, j, k) {
                        continue;
                    }
                    let d6 = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
                    if d6.iter().any(|(di, dj, dk)| !ins(i + di, j + dj, k + dk)) {
                        a.insert((i, j, k));
                    }
                    let c = centre(i, j, k);
                    if c.sub(crate::snap::closest_point_on_surface(c, &p, &t)).length() < cs {
                        a1.insert((i, j, k));
                    }
                }
            }
        }
        let b: BTreeSet<(i64, i64, i64)> = (0..o.number_of_leaves())
            .filter(|&l| o.has_contained_triangles(l))
            .map(|l| {
                let c = o.return_leaf(l).coords;
                assert_eq!(c.level, 4);
                (c.x, c.y, c.z)
            })
            .collect();
        let b_in: BTreeSet<_> = b.iter().copied().filter(|&(i, j, k)| ins(i, j, k)).collect();
        assert!(b_in.is_subset(&a1), "prediction 1");
        for &(i, j, k) in &a {
            let near = [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
                .iter()
                .any(|(di, dj, dk)| b.contains(&(i + di, j + dj, k + dk)));
            assert!(near, "prediction 2 at {:?}", (i, j, k));
        }
        let ab = a.intersection(&b).count();
        let got = (a.len(), a1.len(), b.len(), b_in.len(), ab);
        println!("|A| {} |A1| {} |B| {} |B_in| {} |A and B| {}", got.0, got.1, got.2, got.3, got.4);
        assert!(b.len() > a.len(), "prediction 3");
        assert_eq!(got, DIFF_PINS);
    }
    const DIFF_PINS: (usize, usize, usize, usize, usize) = (336, 360, 680, 312, 288);

    /// V&V (gh:#1262): coordinate algebra. `refineForPosition` then
    /// `reduceLevelBy(1)` is the identity for every child;
    /// `increaseToLevelMin/Max` bracket the descendants; `isPositionInside`
    /// accepts descendants and rejects a neighbour; the Morton order of the
    /// 8 children is their Z order; the 8 children tile the parent box.
    #[test]
    fn coordinate_algebra_matches_upstream_definitions() {
        let p = CubeCoordinates::new(3, 5, 6, 3);
        let root = BoundBox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let pb = p.cube_box(&root);
        let mut vol = 0.0;
        for i in 0..8 {
            let c = p.refine_for_position(i);
            assert_eq!(c.reduce_level_by(1), p);
            assert_eq!(p.is_position_inside(&c), Some(true));
            let cb = c.cube_box(&root);
            assert!(pb.contains(cb.min) && pb.contains(cb.max));
            let s = cb.span();
            vol += s.x * s.y * s.z;
            for j in 0..8 {
                let d = p.refine_for_position(j);
                assert_eq!(c.morton_less(&d), i < j, "Morton order of children {i} {j}");
            }
        }
        let ps = pb.span();
        assert!((vol - ps.x * ps.y * ps.z).abs() < 1e-15);
        assert_eq!(p.increase_to_level_min(5), CubeCoordinates::new(12, 20, 24, 5));
        assert_eq!(p.increase_to_level_max(5), CubeCoordinates::new(15, 23, 27, 5));
        assert_eq!(p.is_position_inside(&CubeCoordinates::new(4, 5, 6, 3)), Some(false));
        assert_eq!(p.is_position_inside(&CubeCoordinates::new(1, 2, 3, 2)), None);
        assert_eq!(p.reduce_to_level(1), CubeCoordinates::new(0, 1, 1, 1));
        let (mn, mx) = p.neighbour_range();
        assert_eq!((mn, mx), (CubeCoordinates::new(5, 9, 11, 4), CubeCoordinates::new(8, 12, 14, 4)));
        assert_eq!(p.plus(&CubeCoordinates::new(1, -1, 0, 0)), CubeCoordinates::new(4, 4, 6, 3));
        // Cube-table consistency: every face's edges join that face's nodes,
        // and edge_faces is the inverse of face_edges.
        for (f, fe) in FACE_EDGES.iter().enumerate() {
            for &e in fe {
                assert!(EDGE_NODES[e].iter().all(|n| FACE_NODES[f].contains(n)));
                assert!(EDGE_FACES[e].contains(&f));
            }
            assert_eq!(OPPOSITE_FACE[OPPOSITE_FACE[f]], f);
        }
        for (n, nf) in NODE_FACES.iter().enumerate() {
            for &f in nf {
                assert!(FACE_NODES[f].contains(&n));
            }
        }
    }

    /// V&V (gh:#1262): `TriSurf` addressing and the feature edges
    /// (`containedEdges`). The box `[0, 1]^3` has 8 points, 12 triangles and
    /// 18 edges (Euler: V - E + F = 2). With one region per box face, the 12
    /// cube edges are feature edges and the 6 face diagonals are not.
    /// Refining two levels, every feature edge is found in at least one
    /// leaf, and every leaf edge list holds only feature edges crossing it.
    #[test]
    fn trisurf_addressing_and_contained_feature_edges() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let regions: Vec<usize> = (0..12).map(|i| i / 2).collect();
        let s = Arc::new(TriSurf::new(p, t, Some(regions)));
        assert_eq!(s.edges.len(), 18);
        assert!(s.edge_facets.iter().all(|f| f.len() == 2));
        let n_feature = (0..s.edges.len()).filter(|&e| s.is_region_edge(e)).count();
        assert_eq!(n_feature, 12);
        let o = data_octree(Arc::clone(&s), 2);
        let mut found: BTreeSet<usize> = BTreeSet::new();
        for l in 0..o.number_of_leaves() {
            for e in o.contained_edges(l) {
                assert!(s.is_region_edge(e));
                let [a, b] = s.edges[e];
                assert!(o.return_leaf(l).coords.intersects_line(&o.root_box, s.points[a], s.points[b]));
                found.insert(e);
            }
            let patches = o.find_boundary_patches_for_leaf(l);
            let mut regs: Vec<usize> = o.contained_triangles(l).iter().map(|&t| s.regions[t]).collect();
            regs.sort_unstable();
            regs.dedup();
            let mut ps = patches.clone();
            ps.sort_unstable();
            assert_eq!(ps, regs);
        }
        assert_eq!(found.len(), 12);
        // The recursive searches agree with a scan of the leaf list.
        let bb = BoundBox::new(Vec3::new(0.2, 0.2, 0.2), Vec3::new(0.6, 0.6, 0.6));
        let by_tree: BTreeSet<usize> = o.leaves_in_box(&bb).into_iter().collect();
        let by_scan: BTreeSet<usize> =
            o.leaves.iter().copied().filter(|&c| o.cube(c).coords.cube_box(&o.root_box).overlaps(&bb)).collect();
        assert_eq!(by_tree, by_scan);
        let mut marked = vec![0u8; o.number_of_leaves()];
        o.mark_leaves_in_sphere(Vec3::new(0.5, 0.5, 0.5), 0.1, &mut marked);
        assert_eq!(marked.iter().filter(|&&m| m == 2).count(), o.leaves_in_sphere(Vec3::new(0.5, 0.5, 0.5), 0.1).len());
        assert_eq!(o.count_child_cubes(), o.slot.cubes.len());
    }

    /// V&V (gh:#1262): the helpers against hand-computed answers.
    /// `nearestPointOnTheTriangle` agrees with Ericson's
    /// `snap::closest_point_on_triangle` to 1e-15 on a grid of probes around
    /// a triangle in every Voronoi region; `triLineIntersection` finds the
    /// crossing of a segment through the face and rejects one that stops
    /// short; `boundBoxLineIntersection` is the slab test.
    #[test]
    fn helpers_match_hand_answers_and_ericson() {
        let (a, b, c) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.2, 0.9, 0.1));
        let mut worst: f64 = 0.0;
        for i in -4..=8 {
            for j in -4..=8 {
                for k in [-1.0, 0.0, 0.7] {
                    let p = Vec3::new(i as f64 * 0.2, j as f64 * 0.2, k);
                    let u = help::nearest_point_on_the_triangle(a, b, c, p);
                    let e = crate::snap::closest_point_on_triangle(p, a, b, c);
                    worst = worst.max(u.sub(e).length());
                }
            }
        }
        assert!(worst < 1e-15, "upstream vs Ericson: {worst:e}");
        let hit = help::tri_line_intersection(a, b, c, Vec3::new(0.3, 0.2, -1.0), Vec3::new(0.3, 0.2, 1.0));
        let h = hit.expect("segment crosses the triangle");
        let n = b.sub(a).cross(c.sub(a));
        assert!(h.sub(a).dot(n).abs() < 1e-15);
        assert!(help::tri_line_intersection(a, b, c, Vec3::new(0.3, 0.2, -1.0), Vec3::new(0.3, 0.2, -0.5)).is_none());
        let bb = BoundBox::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        assert!(help::bound_box_line_intersection(Vec3::new(-1.0, 0.5, 0.5), Vec3::new(2.0, 0.5, 0.5), &bb));
        assert!(!help::bound_box_line_intersection(Vec3::new(-1.0, 1.5, 0.5), Vec3::new(2.0, 1.5, 0.5), &bb));
        assert_eq!(
            help::nearest_point_on_the_edge_exact(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 0.0)),
            Vec3::new(1.0, 0.0, 0.0)
        );
    }
}
