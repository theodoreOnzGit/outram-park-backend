// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506: `particle` (src/lagrangian/basic/
// particle/{particle.C,particleI.H}), `tetIndices` (src/OpenFOAM/meshes/polyMesh/
// polyMeshTetDecomposition/tetIndicesI.H) and `polyMeshTetDecomposition::findFaceBasePts`
// (same directory), `BarycentricTensor` (src/OpenFOAM/primitives/Barycentric).

//! Barycentric particle tracking on a static `polyMesh` (gh:#1382): the core of
//! OpenFOAM's Lagrangian framework, ported from OpenFOAM.com v2506.
//!
//! > Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for operating,
//! > controlling or licensing a nuclear facility.
//!
//! What upstream does, and what is ported:
//!
//! | Upstream | Here |
//! |---|---|
//! | `polyMeshTetDecomposition::findFaceBasePts` (`tetBasePtIs`) | [`TrackingMesh::new`] |
//! | `tetIndices::faceTriIs`, `tetIndices::tet` | [`TrackingMesh::face_tri_is`], [`TrackingMesh::tet`] |
//! | `particle(mesh, position, celli)`, `particle::locate` | [`Particle::new_at`] |
//! | `particle::position` | [`Particle::position`] |
//! | `particle::track`, `trackToFace`, `trackToStationaryTri` | [`Particle::track`], [`Particle::track_to_face`], [`Particle::track_to_tri`] |
//! | `particle::changeTet`, `changeFace`, `changeCell`, `reflect`, `rotate` | private methods of [`Particle`] |
//! | `particle::reset` | [`Particle::reset`] |
//!
//! Every expression repeats the upstream operation order (dot product
//! `x*x + y*y + z*z`, cross product component order, `cmptSum` left to right,
//! `Foam::max(0, y)` as `0 > y ? 0 : y`), because the V&V target is bit-identity with
//! ESI v2506 (`verification_and_validation/particle_tracking_gh1382/`). Vectors are
//! plain `[f64; 3]` like the sibling `esi_*` parity layer; the cell centres are
//! [`crate::check_mesh::esi_geometry`], already bit-identical to `polyMesh::cellCentres`.
//!
//! **Not ported (child issues):** moving-mesh tracking (`trackToMovingTri`, needs
//! `cubicEqn` and `oldPoints`), `celli < 0` location through `mesh.cellTree()`
//! (`indexedOctree::findInside`), coupled (cyclic/processor) patches in the base-point
//! search and `correctAfterParallelTransfer`, `Cloud`, injection and the kinematic
//! parcel sub-models.

use crate::check_mesh::{esi_geometry, esi_tet_decomposition_min_quality, EsiGeometry};
use crate::snappy_hex_mesh::esi_snap::{self, SnapMesh, V};

/// `SMALL` of a double-precision OpenFOAM build.
const SMALL: f64 = 1e-15;
/// `VSMALL`.
const VSMALL: f64 = 1e-300;
/// `VGREAT`.
const VGREAT: f64 = 1e300;
/// `particle::maxNBehind_`.
const MAX_N_BEHIND: isize = 10;

/// Barycentric coordinates `(a, b, c, d)` in a tet: cell centre, then the three face vertices.
pub type Barycentric = [f64; 4];

/// Why a particle could not be created.
#[derive(Debug, Clone, PartialEq)]
pub enum ParticleError {
    /// No cell given: upstream would search `mesh.cellTree()`, which is not ported.
    CellNotGiven,
}

/// A static mesh with the tet decomposition `particle` tracks through.
#[derive(Debug, Clone)]
pub struct TrackingMesh {
    /// The mesh topology and points.
    pub mesh: SnapMesh,
    /// Cell centres and `primitiveMesh::cells()`.
    pub geometry: EsiGeometry,
    /// `polyMesh::tetBasePtIs()`: per face the fan base point, `-1` when none is good.
    pub tet_base_pt_is: Vec<isize>,
}

impl TrackingMesh {
    /// Build the geometry and `polyMeshTetDecomposition::findFaceBasePts(mesh)` with its
    /// default tolerance `minTetQuality = sqr(SMALL)`. All boundary faces are treated as
    /// non-coupled (`findBasePoint`, owner cell only).
    pub fn new(mesh: SnapMesh) -> Self {
        let geometry = esi_geometry(&mesh);
        let tol = SMALL * SMALL;
        let n_internal = mesh.neighbour.len();
        let mut base = vec![-1isize; mesh.faces.len()];
        for (f, b) in base.iter_mut().enumerate() {
            let n = mesh.faces[f].len();
            let own_cc = geometry.cell_centres[mesh.owner[f]];
            for fp in 0..n {
                let own_q = esi_tet_decomposition_min_quality(&mesh, own_cc, f, true, fp);
                let good = if f < n_internal {
                    let nei_cc = geometry.cell_centres[mesh.neighbour[f]];
                    let nei_q = esi_tet_decomposition_min_quality(&mesh, nei_cc, f, false, fp);
                    // Foam::min(a, b) = (a < b) ? a : b
                    let m = if own_q < nei_q { own_q } else { nei_q };
                    m > tol
                } else {
                    own_q > tol
                };
                if good {
                    *b = fp as isize;
                    break;
                }
            }
        }
        TrackingMesh { mesh, geometry, tet_base_pt_is: base }
    }

    fn is_internal_face(&self, facei: isize) -> bool {
        facei >= 0 && (facei as usize) < self.mesh.neighbour.len()
    }

    /// `tetIndices(celli, facei, tetPti).faceTriIs(mesh)` (no warning when the face has no
    /// good base point: base 0, as upstream).
    pub fn face_tri_is(&self, celli: isize, facei: isize, tet_pti: isize) -> [usize; 3] {
        let f = &self.mesh.faces[facei as usize];
        let n = f.len();
        let mut base = self.tet_base_pt_is[facei as usize];
        if base < 0 {
            base = 0;
        }
        let face_pt = (tet_pti as usize + base as usize) % n;
        let other = (face_pt + 1) % n;
        let base = base as usize;
        if self.mesh.owner[facei as usize] as isize != celli {
            [f[base], f[other], f[face_pt]]
        } else {
            [f[base], f[face_pt], f[other]]
        }
    }

    /// `tetIndices::tet(mesh)`: the points (cell centre, then the face triangle).
    pub fn tet(&self, celli: isize, facei: isize, tet_pti: isize) -> [V; 4] {
        let tri = self.face_tri_is(celli, facei, tet_pti);
        let p = &self.mesh.points;
        [self.geometry.cell_centres[celli as usize], p[tri[0]], p[tri[1]], p[tri[2]]]
    }
}

/// `Foam::max(0, y)` for scalars: `(0 > y) ? 0 : y`.
fn max0(y: f64) -> f64 {
    if 0.0 > y {
        0.0
    } else {
        y
    }
}

/// `edge::compare`: `1` same direction, `-1` reversed, `0` different.
fn edge_compare(a: (isize, isize), b: (isize, isize)) -> isize {
    if a.0 == b.0 && a.1 == b.1 {
        1
    } else if a.0 == b.1 && a.1 == b.0 {
        -1
    } else {
        0
    }
}

/// `edge::otherVertex`.
fn other_vertex(e: (isize, isize), v: isize) -> isize {
    if v == e.0 {
        e.1
    } else if v == e.1 {
        e.0
    } else {
        -1
    }
}

/// Tet reverse transform of `particle::stationaryTetReverseTransform`: the cell-centre
/// vertex `a`, the determinant and the four columns of the inverse tensor.
fn stationary_tet_reverse_transform(tet: &[V; 4]) -> (V, f64, [V; 4]) {
    use esi_snap::{cross, dot, sub};
    let ab = sub(tet[1], tet[0]);
    let ac = sub(tet[2], tet[0]);
    let ad = sub(tet[3], tet[0]);
    let bc = sub(tet[2], tet[1]);
    let bd = sub(tet[3], tet[1]);
    let det_a = dot(ab, cross(ac, ad));
    (tet[0], det_a, [cross(bd, bc), cross(ac, ad), cross(ad, ab), cross(ab, ac)])
}

/// A particle in barycentric coordinates: upstream `Foam::particle` without the mesh
/// reference (the [`TrackingMesh`] is passed to every method) and without the
/// processor/ID bookkeeping.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Barycentric coordinates in the current tet.
    pub coordinates: Barycentric,
    /// Current cell.
    pub celli: isize,
    /// Face of the current tet.
    pub tet_facei: isize,
    /// Tet point on that face.
    pub tet_pti: isize,
    /// Face the particle is on, `-1` when in the cell.
    pub facei: isize,
    /// Fraction of the time step completed.
    pub step_fraction: f64,
    /// Distance behind the maximum distance reached.
    pub behind: f64,
    /// Number of tracks that ended behind.
    pub n_behind: isize,
}

impl Particle {
    /// `particle(mesh, position, celli)`: the particle is placed by `locate` in the given cell.
    pub fn new_at(tm: &TrackingMesh, position: V, celli: isize) -> Result<Particle, ParticleError> {
        if celli < 0 {
            return Err(ParticleError::CellNotGiven);
        }
        let mut p = Particle {
            coordinates: [-VGREAT, -VGREAT, -VGREAT, -VGREAT],
            celli,
            tet_facei: -1,
            tet_pti: -1,
            facei: -1,
            step_fraction: 1.0,
            behind: 0.0,
            n_behind: 0,
        };
        p.locate(tm, position);
        Ok(p)
    }

    /// `particle::position()`: `currentTetTransform() & coordinates_`.
    pub fn position(&self, tm: &TrackingMesh) -> V {
        let t = tm.tet(self.celli, self.tet_facei, self.tet_pti);
        let y = self.coordinates;
        let comp = |k: usize| y[0] * t[0][k] + y[1] * t[1][k] + y[2] * t[2][k] + y[3] * t[3][k];
        [comp(0), comp(1), comp(2)]
    }

    /// `particle::reset()`.
    pub fn reset(&mut self) {
        self.step_fraction = 0.0;
        self.behind = 0.0;
        self.n_behind = 0;
    }

    fn on_face(&self) -> bool {
        self.facei >= 0
    }

    fn on_internal_face(&self, tm: &TrackingMesh) -> bool {
        self.on_face() && tm.is_internal_face(self.facei)
    }

    /// `particle::reflect`.
    fn reflect(&mut self) {
        self.coordinates.swap(2, 3);
    }

    /// `particle::rotate`.
    fn rotate(&mut self, reverse: bool) {
        let y = &mut self.coordinates;
        if !reverse {
            let temp = y[1];
            y[1] = y[2];
            y[2] = y[3];
            y[3] = temp;
        } else {
            let temp = y[3];
            y[3] = y[2];
            y[2] = y[1];
            y[1] = temp;
        }
    }

    /// `particle::changeTet`: move to the tet across triangle 1, 2 or 3 of the current tet.
    fn change_tet(&mut self, tm: &TrackingMesh, tet_tri_i: isize) {
        let is_owner = tm.mesh.owner[self.tet_facei as usize] as isize == self.celli;
        let first_tet_pt_i = 1isize;
        let last_tet_pt_i = tm.mesh.faces[self.tet_facei as usize].len() as isize - 2;
        match tet_tri_i {
            1 => self.change_face(tm, tet_tri_i),
            2 => {
                if is_owner {
                    if self.tet_pti == last_tet_pt_i {
                        self.change_face(tm, tet_tri_i);
                    } else {
                        self.reflect();
                        self.tet_pti += 1;
                    }
                } else if self.tet_pti == first_tet_pt_i {
                    self.change_face(tm, tet_tri_i);
                } else {
                    self.reflect();
                    self.tet_pti -= 1;
                }
            }
            3 => {
                if is_owner {
                    if self.tet_pti == first_tet_pt_i {
                        self.change_face(tm, tet_tri_i);
                    } else {
                        self.reflect();
                        self.tet_pti -= 1;
                    }
                } else if self.tet_pti == last_tet_pt_i {
                    self.change_face(tm, tet_tri_i);
                } else {
                    self.reflect();
                    self.tet_pti += 1;
                }
            }
            _ => panic!("Changing tet without changing cell should only happen when the track is on triangle 1, 2 or 3."),
        }
    }

    /// `particle::changeFace`: move to the face of the same cell that shares the edge of
    /// triangle `tet_tri_i`.
    fn change_face(&mut self, tm: &TrackingMesh, tet_tri_i: isize) {
        let old = tm.face_tri_is(self.celli, self.tet_facei, self.tet_pti);
        let o = [old[0] as isize, old[1] as isize, old[2] as isize];
        let shared_edge = match tet_tri_i {
            1 => (o[1], o[2]),
            2 => (o[2], o[0]),
            3 => (o[0], o[1]),
            _ => panic!("Changing face without changing cell should only happen when the track is on triangle 1, 2 or 3."),
        };

        self.tet_pti = -1;
        for &new_face_i in &tm.geometry.cell_faces[self.celli as usize] {
            let new_face = &tm.mesh.faces[new_face_i];
            let new_owner = tm.mesh.owner[new_face_i] as isize;
            if self.tet_facei == new_face_i as isize {
                continue;
            }
            let edge_comp = if new_owner == self.celli { -1 } else { 1 };
            let size = new_face.len();
            let mut edge_i = 0usize;
            while edge_i < size {
                let e = (new_face[edge_i] as isize, new_face[(edge_i + 1) % size] as isize);
                if edge_compare(shared_edge, e) == edge_comp {
                    break;
                }
                edge_i += 1;
            }
            if edge_i >= size {
                continue;
            }
            let new_base_i = tm.tet_base_pt_is[new_face_i].max(0) as usize;
            let edge_i = (edge_i + size - new_base_i) % size;
            // edgeI = min(max(1, edgeI), size - 2)
            let edge_i = edge_i.max(1).min(size - 2);
            self.tet_facei = new_face_i as isize;
            self.tet_pti = edge_i as isize;
            break;
        }
        if self.tet_pti == -1 {
            panic!("The search for an edge-connected face and tet-point failed.");
        }

        // Pre-rotation puts the shared edge opposite the base of the tetrahedron
        if other_vertex(shared_edge, o[1]) == -1 {
            self.rotate(false);
        } else if other_vertex(shared_edge, o[2]) == -1 {
            self.rotate(true);
        }

        let new = tm.face_tri_is(self.celli, self.tet_facei, self.tet_pti);
        let n = [new[0] as isize, new[1] as isize, new[2] as isize];

        self.reflect();

        // Post rotation puts the shared edge back in the correct location
        if other_vertex(shared_edge, n[1]) == -1 {
            self.rotate(true);
        } else if other_vertex(shared_edge, n[2]) == -1 {
            self.rotate(false);
        }
    }

    /// `particle::changeCell`: cross the internal face the particle is on.
    fn change_cell(&mut self, tm: &TrackingMesh) {
        let owner = tm.mesh.owner[self.tet_facei as usize] as isize;
        let is_owner = self.celli == owner;
        self.celli = if is_owner { tm.mesh.neighbour[self.tet_facei as usize] as isize } else { owner };
        self.reflect();
    }

    /// `particle::locate` for a given cell, `boundaryFail = false`.
    fn locate(&mut self, tm: &TrackingMesh, position: V) {
        let celli = self.celli;
        // Perturbing displacement to avoid zero in case position is the cellCentre
        let cc = tm.geometry.cell_centres[celli as usize];
        let d0 = esi_snap::sub(position, cc);
        let displacement = [d0[0] + VSMALL, d0[1] + VSMALL, d0[2] + VSMALL];

        let mut min_f = VGREAT;
        let mut min_tet_facei = -1isize;
        let mut min_tet_pti = -1isize;
        for &face_i in &tm.geometry.cell_faces[celli as usize] {
            let n = tm.mesh.faces[face_i].len();
            for tet_pti in 1..n - 1 {
                self.coordinates = [1.0, 0.0, 0.0, 0.0];
                self.tet_facei = face_i as isize;
                self.tet_pti = tet_pti as isize;
                self.facei = -1;
                let (f, tet_tri_i) = self.track_to_tri(tm, displacement, 0.0);
                if tet_tri_i == -1 {
                    return;
                }
                if f < min_f {
                    min_f = f;
                    min_tet_facei = self.tet_facei;
                    min_tet_pti = self.tet_pti;
                }
            }
        }

        // The particle must be (hopefully only slightly) outside the cell. Track into the
        // tet which got the furthest.
        self.coordinates = [1.0, 0.0, 0.0, 0.0];
        self.tet_facei = min_tet_facei;
        self.tet_pti = min_tet_pti;
        self.facei = -1;
        self.track(tm, displacement, 0.0);
    }

    /// `particle::track`: track across internal faces until the displacement is used up or a
    /// boundary face is hit. Returns the fraction of the displacement not completed.
    pub fn track(&mut self, tm: &TrackingMesh, displacement: V, fraction: f64) -> f64 {
        let mut f = self.track_to_face(tm, displacement, fraction);
        while self.on_internal_face(tm) {
            self.change_cell(tm);
            let r = self.track_to_face(tm, esi_snap::scale(f, displacement), f * fraction);
            f *= r;
        }
        f
    }

    /// `particle::trackToFace`: track to the next face of the cell, stepping through the
    /// tets of the cell. Returns the fraction not completed.
    pub fn track_to_face(&mut self, tm: &TrackingMesh, displacement: V, fraction: f64) -> f64 {
        let mut f = 1.0;
        let mut tet_tri_i: isize = if self.on_face() { 0 } else { -1 };
        self.facei = -1;
        while self.n_behind < MAX_N_BEHIND {
            let (r, t) = self.track_to_tri(tm, esi_snap::scale(f, displacement), f * fraction);
            tet_tri_i = t;
            f *= r;
            if tet_tri_i == -1 {
                // The track has completed within the current tet
                return 0.0;
            } else if tet_tri_i == 0 {
                // The track has hit a face
                self.facei = self.tet_facei;
                return f;
            } else {
                self.change_tet(tm, tet_tri_i);
            }
        }
        let _ = tet_tri_i;
        // Stuck: upstream warns, advances the step fraction to completion and gives up.
        self.step_fraction += f * fraction;
        self.behind = 0.0;
        self.n_behind = 0;
        0.0
    }

    /// `particle::trackToTri` on a static mesh (`trackToStationaryTri`): track within the
    /// current tet to the point where it hits one of the tet's four triangles. Returns the
    /// fraction of the displacement not completed and the triangle hit (`-1` none).
    pub fn track_to_tri(&mut self, tm: &TrackingMesh, displacement: V, fraction: f64) -> (f64, isize) {
        use esi_snap::{dot, mag};
        let x1 = displacement;
        let y0 = self.coordinates;

        let tet = tm.tet(self.celli, self.tet_facei, self.tet_pti);
        let (_centre, det_a, t) = stationary_tet_reverse_transform(&tet);

        // Local tracking displacement x1 & T
        let tx1 = [dot(x1, t[0]), dot(x1, t[1]), dot(x1, t[2]), dot(x1, t[3])];

        let mut i_h: isize = -1;
        let mut mu_h = if det_a <= 0.0 { VGREAT } else { 1.0 / det_a };
        for i in 0..4 {
            if tx1[i] < -det_a * SMALL {
                let mu = -y0[i] / tx1[i];
                if 0.0 <= mu && mu < mu_h {
                    i_h = i as isize;
                    mu_h = mu;
                }
            }
        }

        let mut y_h = [0.0f64; 4];
        for i in 0..4 {
            y_h[i] = y0[i] + mu_h * tx1[i];
        }
        // Clamp to zero any negative coordinates generated by round-off error
        for (i, y) in y_h.iter_mut().enumerate() {
            *y = if i as isize == i_h { 0.0 } else { max0(*y) };
        }
        // Re-normalise if within the tet
        if i_h == -1 {
            let sum = y_h[0] + y_h[1] + y_h[2] + y_h[3];
            for y in y_h.iter_mut() {
                *y /= sum;
            }
        }

        self.coordinates = y_h;
        self.step_fraction += fraction * mu_h * det_a;

        // Accumulate displacement behind
        if det_a <= 0.0 || self.n_behind > 0 {
            self.behind += mu_h * det_a * mag(displacement);
            if self.behind > 0.0 {
                self.behind = 0.0;
                self.n_behind = 0;
            } else {
                self.n_behind += 1;
            }
        }

        let ret = if i_h != -1 { 1.0 - mu_h * det_a } else { 0.0 };
        (ret, i_h)
    }
}
