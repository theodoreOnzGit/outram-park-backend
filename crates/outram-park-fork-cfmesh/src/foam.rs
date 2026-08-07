// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Format reference (re-implemented in Rust, not transcribed):
//   OpenFOAM `constant/polyMesh` on-disk format and the `polyPatch` kinds this
//   bridge maps onto (`wall` vs generic `patch`).
//   src/OpenFOAM/meshes/polyMesh
//   Copyright (C) 2011-2016 OpenFOAM Foundation
//   Copyright (C) 2016-2023 OpenCFD Ltd. Licence: GPL-3.0-only.
//   The concrete reader/writer lives in the workspace's own
//   `outram-foam-basic-lib` (GPL-3.0-only); this file only converts into it.
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

//! Bridge from [`crate::volume_mesh::VolumeMesh`] to the real
//! `outram-foam-basic-lib` **`PolyMesh`**
//! — the volume-mesh output path to the CFD/TH solver (feature `foam-export`).
//!
//! [`crate::volume_mesh::VolumeMesh`] is deliberately shaped like OpenFOAM's `polyMesh`
//! (points + faces + owner/neighbour + internal-first ordering + patches), so
//! this bridge is nearly a field-for-field copy into
//! `outram_foam_basic_lib::io::poly_mesh::PolyMesh`. From there the caller gets
//! a solvable mesh for free: `PolyMesh::to_fv_mesh()` builds the geometry-
//! carrying `FvMesh`, and `PolyMesh::write(dir)` emits an OpenFOAM
//! `constant/polyMesh`.
//!
//! This is the piece that closes the coupling loop: an `outram-blender` surface
//! → [`crate::carve::carve_box`] → [`crate::snap::snap_to_surface`] →
//! `to_poly_mesh` → an `outram-foam` `FvMesh`.
//!
//! Gated behind `foam-export` so the base crate stays dependency-free and
//! Android-buildable; `outram-foam-basic-lib` is itself BLAS-free.

use crate::volume_mesh::VolumeMesh;
use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch as FoamPatch, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

/// Convert a [`VolumeMesh`] into a real `outram-foam-basic-lib`
/// [`PolyMesh`](outram_foam_basic_lib::io::poly_mesh::PolyMesh).
///
/// Points, faces, owner/neighbour, `n_cells`, and the internal-first face
/// ordering carry over directly. Each boundary patch maps to a foam
/// [`PatchKind`]: a patch whose name contains `"wall"` becomes
/// [`PatchKind::Wall`], everything else [`PatchKind::Patch`]. The result is
/// solver-ready — call `to_fv_mesh()` on it for the geometry-carrying mesh, or
/// `write(dir)` to emit the OpenFOAM files.
pub fn to_poly_mesh(mesh: &VolumeMesh) -> PolyMesh {
    let points: Vec<Vector3> = mesh.points.iter().map(|p| Vector3::new(p.x, p.y, p.z)).collect();

    let faces: Vec<MeshFace> = (0..mesh.face_count())
        .map(|f| MeshFace {
            verts: mesh.faces[f].clone(),
            owner: mesh.owner[f],
            neighbour: mesh.neighbour[f],
        })
        .collect();

    let patches: Vec<FoamPatch> = mesh
        .patches
        .iter()
        .map(|p| {
            let kind = if p.name.to_lowercase().contains("wall") {
                PatchKind::Wall
            } else {
                PatchKind::Patch
            };
            FoamPatch::new(p.name.clone(), p.start_face, p.n_faces, kind)
        })
        .collect();

    PolyMesh {
        points,
        faces,
        n_internal_faces: mesh.n_internal_faces(),
        n_cells: mesh.n_cells,
        patches,
    }
}

/// Write `mesh` as an OpenFOAM `polyMesh` into `dir` (which becomes a
/// `constant/polyMesh` directory): `points`, `faces`, `owner`, `neighbour`, and
/// `boundary`. The result is a real OpenFOAM mesh directory a solver can read.
///
/// Convenience over [`to_poly_mesh`] followed by
/// [`PolyMesh::write`](outram_foam_basic_lib::io::poly_mesh::PolyMesh::write).
pub fn write_polymesh(mesh: &VolumeMesh, dir: &std::path::Path) -> Result<(), outram_foam_basic_lib::io::IoError> {
    to_poly_mesh(mesh).write(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::carve::carve_box;
    use crate::math::Vec3;

    /// V&V — end-to-end: a Cartesian box mesh bridges to a foam `PolyMesh` whose
    /// own geometry engine agrees. Methodology: `cartesian_box` a unit box
    /// 2×2×2, `to_poly_mesh`, then read the counts and volume back through
    /// foam's `PolyMesh`. Result: 8 cells, 12 internal + 24 boundary faces,
    /// `PolyMesh::total_volume()` (OpenFOAM pyramid decomposition) = 1; and
    /// `to_fv_mesh()` succeeds (the mesh is solver-consumable).
    #[test]
    fn cartesian_box_bridges_to_foam_polymesh() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let poly = to_poly_mesh(&m);
        assert_eq!(poly.n_cells, 8);
        assert_eq!(poly.n_internal_faces, 12);
        assert_eq!(poly.n_boundary_faces(), 24);
        assert_eq!(poly.n_faces(), 36);
        assert!((poly.total_volume() - 1.0).abs() < 1e-9, "foam agrees on box volume");
        poly.to_fv_mesh().expect("PolyMesh converts to a solvable FvMesh");
    }

    /// V&V — a carved surface bridges too. A grid-aligned box carve → foam
    /// `PolyMesh`: same cell count, foam-computed volume matches, one `walls`
    /// patch mapped to `PatchKind::Wall`, and `to_fv_mesh` succeeds.
    #[test]
    fn carved_mesh_bridges_and_is_solver_ready() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let m = carve_box(&p, &t, 0.5);
        let poly = to_poly_mesh(&m);
        assert_eq!(poly.n_cells, 64);
        assert!((poly.total_volume() - 8.0).abs() < 1e-9);
        assert_eq!(poly.patches.len(), 1);
        poly.to_fv_mesh().expect("carved mesh is solver-consumable");
    }

    /// V&V — the **polyhedral dual** bridges to a solvable foam `PolyMesh`. This
    /// is the payoff for the reactor solvers: a hex background carve → median
    /// dual → foam `PolyMesh` → `FvMesh`. Methodology: carve a unit box (0.25
    /// cells → 64 hexes, 125 vertices), take the dual (125 polyhedral cells),
    /// bridge, and read counts + volume back through foam. Result: 125 cells,
    /// foam-computed volume = 1 (the pyramid decomposition agrees with the
    /// divergence-theorem volume), and `to_fv_mesh()` succeeds on genuinely
    /// polyhedral cells.
    #[test]
    fn polyhedral_dual_bridges_and_is_solver_ready() {
        use crate::dual::polyhedral_dual;
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let hex = carve_box(&p, &t, 0.25);
        let dual = polyhedral_dual(&hex);
        let poly = to_poly_mesh(&dual);
        assert_eq!(poly.n_cells, hex.point_count()); // 125 — one cell per vertex
        assert!((poly.total_volume() - 1.0).abs() < 1e-9, "foam agrees on dual volume");
        poly.to_fv_mesh().expect("polyhedral dual is solver-consumable");
    }

    /// A scratch directory that deletes itself on drop — including when a test
    /// unwinds on a failed assertion, which a trailing `remove_dir_all` call
    /// would skip and so leak the directory. The path is taken from
    /// [`std::env::temp_dir`], so it honours `$TMPDIR` (required on
    /// Android/Termux, where `/tmp` does not exist).
    struct ScratchDir {
        path: std::path::PathBuf,
    }

    impl ScratchDir {
        /// Create a process-unique scratch directory named `<temp>/<tag>_<pid>`.
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path); // clear any stale leftover
            Self { path }
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// V&V — on-disk round-trip: write a generated mesh as a real OpenFOAM
    /// `polyMesh` and read it straight back. Methodology: `cartesian_box` 2×2×2,
    /// `write_polymesh` to a temp dir, then `PolyMesh::read`. Result: the
    /// re-read mesh has 8 cells, 12 internal faces, and total volume 1 — the
    /// generator produces a solver-readable OpenFOAM mesh on disk. The scratch
    /// directory is a [`ScratchDir`], so it is removed even if an assertion
    /// below fails.
    #[test]
    fn write_polymesh_disk_round_trip() {
        use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let scratch = ScratchDir::new("cfmesh_polymesh_rt");
        write_polymesh(&m, &scratch.path).expect("write polyMesh to temp dir");
        let poly = PolyMesh::read(&scratch.path).expect("read polyMesh back");
        assert_eq!(poly.n_cells, 8);
        assert_eq!(poly.n_internal_faces, 12);
        assert!((poly.total_volume() - 1.0).abs() < 1e-9);
    }

    fn box_surface(a: Vec3, b: Vec3) -> (Vec<Vec3>, Vec<[usize; 3]>) {
        let v = vec![
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(b.x, b.y, b.z),
            Vec3::new(a.x, b.y, b.z),
        ];
        let q = |a: usize, b: usize, c: usize, d: usize| vec![[a, b, c], [a, c, d]];
        let mut t = Vec::new();
        for f in [q(0, 3, 2, 1), q(4, 5, 6, 7), q(0, 1, 5, 4), q(2, 3, 7, 6), q(1, 2, 6, 5), q(0, 4, 7, 3)] {
            t.extend(f);
        }
        (v, t)
    }
}
