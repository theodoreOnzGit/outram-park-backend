// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK, distributed under AGPL-3.0. See the crate root or
// <https://www.gnu.org/licenses/> for the full license text.

//! # From an OpenFOAM `polyMesh` to a `farrer-park` finite-element mesh (gh:#1782)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The finite-volume side holds cells as face addressing ([`FvMesh`]: `owner`, `neighbour`, geometry) and has
//! no vertex lists. A finite element needs the vertices of each cell, so [`FemTopology`] carries the `points`
//! and `faces` files of the same `polyMesh` and [`FemMesh::build`] turns the pair into a [`farrer_park::mesh::Mesh`]
//! of one element type:
//!
//! * **`Hex8`**, when every cell is a hexahedron (8 vertices, 6 quadrilateral faces). The eight nodes are ordered
//!   as `farrer-park`'s reference hexahedron wants them (bottom face counter-clockwise seen from inside, the top
//!   node of each bottom node found along the cell's own edges).
//! * **`Tet4`**, for any other cell (prism, tetrahedron, pyramid, polyhedron): the cell is cut into tetrahedra by
//!   the *cone from its lowest-numbered vertex* to the faces not containing that vertex, each face fanned from
//!   its own lowest-numbered vertex. Both rules depend only on the global vertex numbers of the cell and of the
//!   face, so two cells sharing a face cut it the same way: the tetrahedra join conformally (no hanging nodes) on
//!   any polyhedral mesh whose cells are star-shaped about their lowest vertex. A cell that is not is refused by
//!   name. Reference for the conforming-by-global-index rule: Dompierre, Labbe, Vallet and Camarero, "How to
//!   subdivide pyramids, prisms and hexahedra into tetrahedra", 8th International Meshing Roundtable (1999).
//!
//! One element type per mesh, as `farrer-park` requires: a mesh mixing hexahedra and other cells is cut entirely
//! into tetrahedra under [`FemElementChoice::Auto`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::vec::Vec;

use farrer_park::element::{map_gradients, ElementType, FacetType};
use farrer_park::mesh::{ElemId, Facet, Mesh, NodeId};
use outram_foam_basic_lib::mesh::FvMesh;

use crate::error::AppBuilderError;

/// The vertices of a `polyMesh`: the `points` and `faces` files, which [`FvMesh`] does not keep.
#[derive(Debug, Clone, PartialEq)]
pub struct FemTopology {
    /// Point coordinates (m).
    pub points: Vec<[f64; 3]>,
    /// Face vertex lists, each ordered so its normal points from the owner out of the owner cell (OpenFOAM).
    pub faces: Vec<Vec<usize>>,
}

impl FemTopology {
    /// Read the `points` and `faces` of a `polyMesh` directory (plain, not gzipped), with the same parsers
    /// [`crate::io::poly_mesh::read_poly_mesh`] uses.
    ///
    /// # Errors
    /// I/O and parse errors.
    #[cfg(feature = "std")]
    pub fn read(poly_mesh_dir: &crate::path::Path) -> Result<Self, AppBuilderError> {
        let read = |name: &str| -> Result<String, AppBuilderError> {
            let p = poly_mesh_dir.join(name);
            std::fs::read_to_string(&p).map_err(|e| AppBuilderError::Io { path: p, source: e })
        };
        let points = crate::io::poly_mesh::parse_points(&read("points")?, "points")?;
        let faces = crate::io::poly_mesh::parse_faces(&read("faces")?, "faces")?;
        Ok(Self { points, faces })
    }
}

/// Which element the cells are meshed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FemElementChoice {
    /// `Hex8` when every cell is a hexahedron, otherwise `Tet4` by the cone decomposition.
    #[default]
    Auto,
    /// `Hex8`; refuses a mesh with any non-hexahedral cell.
    Hex8,
    /// `Tet4` by the cone decomposition of every cell (also of hexahedra: six tetrahedra each).
    Tet4Decomposed,
}

/// A boundary facet and the patch face it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchFacet {
    /// The `farrer-park` facet (`Quad4` or `Tri3`).
    pub facet: Facet,
    /// Global face index in the polyMesh (for the outward normal in `FvMesh::face_area_vectors`).
    pub face: usize,
}

/// The finite-element mesh of a polyMesh, with the maps back to the cells and patches.
#[derive(Debug, Clone)]
pub struct FemMesh {
    /// The element mesh (`Hex8` or `Tet4`).
    pub mesh: Mesh,
    /// The polyMesh cell each element belongs to.
    pub element_cell: Vec<usize>,
    /// Per patch: the nodes of its faces (sorted, unique).
    pub patch_nodes: Vec<Vec<NodeId>>,
    /// Per patch: its boundary facets.
    pub patch_facets: Vec<Vec<PatchFacet>>,
}

fn case_err(msg: String) -> AppBuilderError {
    AppBuilderError::Case { case: crate::path::PathBuf::new(), msg }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The outward-oriented vertex loops of every cell's faces.
fn cell_face_loops(mesh: &FvMesh, topo: &FemTopology) -> Vec<Vec<Vec<usize>>> {
    let mut cells: Vec<Vec<Vec<usize>>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.n_faces {
        cells[mesh.owner[f]].push(topo.faces[f].clone());
        if f < mesh.n_internal_faces {
            let mut rev = topo.faces[f].clone();
            rev.reverse();
            cells[mesh.neighbour[f]].push(rev);
        }
    }
    cells
}

/// The eight `Hex8` nodes of a hexahedral cell from its outward-oriented face loops, or `None` if the cell is
/// not a hexahedron.
fn hex_nodes(faces: &[Vec<usize>]) -> Option<[usize; 8]> {
    if faces.len() != 6 || faces.iter().any(|f| f.len() != 4) {
        return None;
    }
    let bottom = &faces[0];
    // the outward-oriented bottom face is clockwise seen from inside; the reference element wants counter-clockwise
    let b = [bottom[0], bottom[3], bottom[2], bottom[1]];
    let edge = |p: usize, q: usize| {
        faces.iter().any(|f| (0..4).any(|i| (f[i] == p && f[(i + 1) % 4] == q) || (f[i] == q && f[(i + 1) % 4] == p)))
    };
    let mut all: Vec<usize> = faces.iter().flat_map(|f| f.iter().copied()).collect();
    all.sort_unstable();
    all.dedup();
    if all.len() != 8 {
        return None;
    }
    let mut top = [0usize; 4];
    for i in 0..4 {
        let cand: Vec<usize> = all.iter().copied().filter(|&v| !b.contains(&v) && edge(b[i], v)).collect();
        if cand.len() != 1 {
            return None;
        }
        top[i] = cand[0];
    }
    Some([b[0], b[1], b[2], b[3], top[0], top[1], top[2], top[3]])
}

/// Fan triangles `(a, b, c)` of an outward-oriented face loop, fanned from the face's lowest-numbered vertex.
fn fan(face: &[usize]) -> Vec<[usize; 3]> {
    let n = face.len();
    let m = (0..n).min_by_key(|&i| face[i]).unwrap_or(0);
    (1..n - 1).map(|t| [face[m], face[(m + t) % n], face[(m + t + 1) % n]]).collect()
}

/// The conforming tetrahedra of one cell (module documentation), each as positively oriented `farrer-park`
/// `Tet4` nodes.
fn cone_tets(faces: &[Vec<usize>], pts: &[[f64; 3]], cell: usize) -> Result<Vec<[usize; 4]>, AppBuilderError> {
    let v = faces.iter().flat_map(|f| f.iter().copied()).min().unwrap_or(0);
    let mut out = Vec::new();
    for face in faces {
        if face.contains(&v) {
            continue;
        }
        for [a, b, c] in fan(face) {
            let (pa, pb, pc, pv) = (pts[a], pts[b], pts[c], pts[v]);
            let n = cross(sub(pb, pa), sub(pc, pa));
            let side = dot(n, sub(pv, pa)) / 6.0;
            // (longest edge)^3 sets the scale of "flat"
            let l2 = |p: [f64; 3]| dot(sub(p, pa), sub(p, pa));
            let m2 = l2(pb).max(l2(pc)).max(l2(pv));
            let scale = m2 * m2.sqrt();
            if side > 1.0e-12 * scale {
                return Err(case_err(format!(
                    "FEM: cell {cell} is not star-shaped about its lowest vertex (a face sees it from outside)"
                )));
            }
            if side.abs() <= 1.0e-12 * scale {
                continue; // a flat tetrahedron adds no volume
            }
            // (a, b, c) outward with v inside has negative signed volume; swapping b and c makes it positive
            out.push([a, c, b, v]);
        }
    }
    Ok(out)
}

impl FemMesh {
    /// Build the element mesh of `mesh` with the vertices `topo`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if `topo` does not match `mesh`, a requested `Hex8` mesh has a non-hexahedral cell,
    /// a cell is not star-shaped about its lowest vertex (`Tet4`), or an element has a non-positive Jacobian.
    pub fn build(mesh: &FvMesh, topo: &FemTopology, choice: FemElementChoice) -> Result<Self, AppBuilderError> {
        if topo.faces.len() != mesh.n_faces {
            return Err(case_err(format!(
                "FEM: the polyMesh has {} faces but the finite-volume mesh has {}",
                topo.faces.len(),
                mesh.n_faces
            )));
        }
        let loops = cell_face_loops(mesh, topo);
        let hexes: Vec<Option<[usize; 8]>> = loops.iter().map(|f| hex_nodes(f)).collect();
        let all_hex = hexes.iter().all(Option::is_some);
        let use_hex = match choice {
            FemElementChoice::Auto => all_hex,
            FemElementChoice::Hex8 => {
                if !all_hex {
                    return Err(case_err("FEM: Hex8 requested but the mesh has non-hexahedral cells".to_string()));
                }
                true
            }
            FemElementChoice::Tet4Decomposed => false,
        };
        let mut conn_pts: Vec<usize> = Vec::new();
        let mut element_cell: Vec<usize> = Vec::new();
        for (c, faces) in loops.iter().enumerate() {
            if use_hex {
                if let Some(h) = hexes[c] {
                    conn_pts.extend_from_slice(&h);
                    element_cell.push(c);
                }
            } else {
                for t in cone_tets(faces, &topo.points, c)? {
                    conn_pts.extend_from_slice(&t);
                    element_cell.push(c);
                }
            }
        }
        // compact to the points actually used (an unused node would leave a zero row in the stiffness matrix)
        let mut node_of = vec![usize::MAX; topo.points.len()];
        let mut coords: Vec<[f64; 3]> = Vec::new();
        let mut conn: Vec<usize> = Vec::with_capacity(conn_pts.len());
        for &p in &conn_pts {
            if node_of[p] == usize::MAX {
                node_of[p] = coords.len();
                coords.push(topo.points[p]);
            }
            conn.push(node_of[p]);
        }
        let et = if use_hex { ElementType::Hex8 } else { ElementType::Tet4 };
        let fe = Mesh::new(et, coords, conn).map_err(|e| case_err(format!("FEM: {e}")))?;
        // the Jacobian must be positive at every vertex of every hexahedron, and at the centroid of every tetrahedron
        let corners: [[f64; 3]; 8] = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let probes: &[[f64; 3]] = if use_hex { &corners } else { &[[0.25, 0.25, 0.25]] };
        let mut ec = vec![[0.0_f64; 3]; et.n_nodes()];
        for e in 0..fe.n_elements() {
            fe.element_coords(ElemId(e), &mut ec);
            for xi in probes {
                if map_gradients(et, &ec, *xi).is_err() {
                    return Err(case_err(format!(
                        "FEM: element {e} (cell {}) has a non-positive Jacobian",
                        element_cell[e]
                    )));
                }
            }
        }
        let mut patch_nodes = Vec::new();
        let mut patch_facets = Vec::new();
        for p in &mesh.patches {
            let mut nodes: Vec<NodeId> = Vec::new();
            let mut facets: Vec<PatchFacet> = Vec::new();
            for gf in p.start..p.start + p.size {
                let face_loop = &topo.faces[gf];
                for &v in face_loop {
                    if node_of[v] != usize::MAX {
                        nodes.push(NodeId(node_of[v]));
                    }
                }
                if use_hex {
                    let ns = face_loop.iter().map(|&v| NodeId(node_of[v])).collect();
                    facets.push(PatchFacet { facet: Facet { facet_type: FacetType::Quad4, nodes: ns }, face: gf });
                } else {
                    for t in fan(face_loop) {
                        let ns = t.iter().map(|&v| NodeId(node_of[v])).collect();
                        facets.push(PatchFacet { facet: Facet { facet_type: FacetType::Tri3, nodes: ns }, face: gf });
                    }
                }
            }
            nodes.sort_by_key(|n| n.0);
            nodes.dedup();
            patch_nodes.push(nodes);
            patch_facets.push(facets);
        }
        Ok(Self { mesh: fe, element_cell, patch_nodes, patch_facets })
    }
}
