// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.


//! Shapes of the `indexedOctree` for snappyHexMesh: edges (`treeDataEdge`) and surface triangles
//! (`treeDataTriSurface`), over the generic `indexedOctree` translation that lives in
//! [`outram_foam_basic_lib::esi_octree`] (moved there for gh:#1564 so `viewFactorsGen` can use
//! `findLine`; everything is re-exported here unchanged).

use crate::snappy_hex_mesh::stl::Triangle;
pub use outram_foam_basic_lib::esi_octree::{
    cross, dot, sub, EsiRandom, NearestShapes, Octree, OctreeShapes, TreeBox, V,
};
use outram_foam_basic_lib::primitives::Vector3;

// ---------------------------------------------------------------------------------------
// shapes: edges (treeDataEdge) and surface triangles (treeDataTriSurface)
// ---------------------------------------------------------------------------------------

/// `linePointRef::nearestDist(p)`: the nearest point on the segment and its distance.
pub fn line_nearest(a: V, b: V, p: V) -> (V, f64) {
    let v = sub(b, a);
    let w = sub(p, a);
    let c1 = dot(v, w);
    if c1 <= 0.0 {
        let d = sub(p, a);
        return (a, dot(d, d).sqrt());
    }
    let c2 = dot(v, v);
    if c2 <= c1 {
        let d = sub(p, b);
        return (b, dot(d, d).sqrt());
    }
    let bb = c1 / c2;
    let pb = [a[0] + bb * v[0], a[1] + bb * v[1], a[2] + bb * v[2]];
    let d = sub(p, pb);
    (pb, dot(d, d).sqrt())
}

/// `treeDataEdge`: a subset (`ids`) of the edges of a point/edge list.
#[derive(Debug, Clone)]
pub struct EdgeShapes {
    /// Edge end points.
    pub edges: Vec<[usize; 2]>,
    /// Points.
    pub points: Vec<V>,
    /// Edge labels of the shapes (`objectIndex`).
    pub ids: Vec<usize>,
}

impl OctreeShapes for EdgeShapes {
    fn len(&self) -> usize {
        self.ids.len()
    }

    fn overlaps_box(&self, index: usize, bb: &TreeBox) -> bool {
        let e = self.edges[self.ids[index]];
        bb.intersects_segment(self.points[e[0]], self.points[e[1]])
    }
}

impl NearestShapes for EdgeShapes {
    fn find_nearest(&self, indices: &[usize], sample: V, nearest_dist_sqr: &mut f64, min_index: &mut Option<usize>, nearest_point: &mut V) {
        for &index in indices {
            let e = self.edges[self.ids[index]];
            let (pt, dist) = line_nearest(self.points[e[0]], self.points[e[1]], sample);
            let d2 = dist * dist;
            if d2 < *nearest_dist_sqr {
                *nearest_dist_sqr = d2;
                *min_index = Some(index);
                *nearest_point = pt;
            }
        }
    }
}

/// `treeDataTriSurface` (`treeDataPrimitivePatch<triSurface>`): the triangles of the surface.
#[derive(Debug, Clone)]
pub struct TriShapes {
    /// The triangles.
    pub tris: Vec<Triangle>,
}

fn v3(p: Vector3) -> V {
    [p.x, p.y, p.z]
}

impl OctreeShapes for TriShapes {
    fn len(&self) -> usize {
        self.tris.len()
    }

    fn overlaps_box(&self, index: usize, bb: &TreeBox) -> bool {
        let t = &self.tris[index];
        let (a, b, c) = (v3(t.a), v3(t.b), v3(t.c));
        // `treeDataPrimitivePatch::overlaps`: quick bound-box rejection, then the triangle test
        for d in 0..3 {
            let lo = a[d].min(b[d]).min(c[d]);
            let hi = a[d].max(b[d]).max(c[d]);
            if !(bb.min[d] <= hi && lo <= bb.max[d]) {
                return false;
            }
        }
        bb.intersects_triangle(a, b, c)
    }
}

impl NearestShapes for TriShapes {
    fn find_nearest(&self, indices: &[usize], sample: V, nearest_dist_sqr: &mut f64, min_index: &mut Option<usize>, nearest_point: &mut V) {
        let s = Vector3::new(sample[0], sample[1], sample[2]);
        for &index in indices {
            let q = v3(self.tris[index].closest_point(s));
            let d = sub(q, sample);
            let dist = dot(d, d).sqrt();
            let d2 = dist * dist;
            if d2 < *nearest_dist_sqr {
                *nearest_dist_sqr = d2;
                *min_index = Some(index);
                *nearest_point = q;
            }
        }
    }
}

/// The `triSurfaceSearch::tree()` of a surface: `maxTreeDepth 10`, leaf size 10, duplicity 3.
pub fn surface_tree(tris: &[Triangle]) -> Octree<TriShapes> {
    let mut pts = Vec::with_capacity(3 * tris.len());
    for t in tris {
        pts.push(v3(t.a));
        pts.push(v3(t.b));
        pts.push(v3(t.c));
    }
    let bb = TreeBox::search_domain(&pts);
    Octree::new(TriShapes { tris: tris.to_vec() }, bb, 10, 10, 3.0)
}

/// `refinementFeatures::buildTrees` / `regionEdgeTrees`: `maxLevel 8`, leaf size 10,
/// duplicity 3, over `ids` (all edges, or the region edges).
pub fn edge_tree(edges: &[[usize; 2]], points: &[V], ids: Vec<usize>) -> Octree<EdgeShapes> {
    let bb = TreeBox::search_domain(points);
    Octree::new(EdgeShapes { edges: edges.to_vec(), points: points.to_vec(), ids }, bb, 8, 10, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_tree_finds_the_first_of_two_tied_edges_it_visits() {
        // 30 unit edges along x at distinct y; the query is nearest to edge 7 only.
        let mut points = Vec::new();
        let mut edges = Vec::new();
        for i in 0..30usize {
            points.push([0.0, i as f64, 0.0]);
            points.push([1.0, i as f64, 0.0]);
            edges.push([2 * i, 2 * i + 1]);
        }
        let tree = edge_tree(&edges, &points, (0..30).collect());
        assert!(tree.n_nodes() > 1);
        let (idx, pt) = tree.find_nearest([0.5, 7.2, 0.1], 100.0).unwrap();
        assert_eq!(idx, 7);
        assert_eq!(pt, [0.5, 7.0, 0.0]);
        assert!(tree.find_nearest([0.5, 7.2, 0.1], 0.0001).is_none());
    }
}
