// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! LES filter widths (`LESdelta` classes) of OpenFOAM-v2506 (ESI), `LESdeltas/`: `maxDeltaxyz`,
//! `IDDESDelta` and `vanDriest` (gh:#1544, under #1377 and #1376).
//!
//! * [`max_delta_xyz`]: `deltaCoeff max_f |n_f . (Cf - C)|` over the cell's faces
//!   (`maxDeltaxyz.C`, default `deltaCoeff` 2; also the `hmax` of `IDDESDelta`).
//! * [`iddes_delta`]: `min(max(max(Cw y, Cw hmax), faceToFaceMax), hmax)` (`IDDESDelta.C`,
//!   default `Cw` 0.15), the wall-normal vector `n` and the wall distance `y` being INPUTS.
//! * [`van_driest_ystar_faces`], [`nearest_wall_face`], [`van_driest_yplus`], [`van_driest_delta`]:
//!   the pieces of `vanDriestDelta::calcDelta()`: `ystar = nu/sqrt((nu + nut)|snGrad U| + VSMALL)` on the
//!   wall faces, mapped to cells from the nearest wall face, `y+ = y/ystar` cut at 500, and
//!   `delta = min(geometric, (kappa/Cdelta)((1 + SMALL) - exp(-y+/Aplus)) y)`.
//!
//! **Status: AI draft.** Compared per cell with the real ESI classes on a frozen duct (README slice 9
//! of `verification_and_validation/upstream_turb_gh1362/`). Differences from upstream: the nearest wall
//! face for the `vanDriest` map is the nearest wall-face CENTRE (brute force), upstream uses the
//! `meshWave` addressing, so [`van_driest_delta`] takes `y` as an input and the port's map is a
//! documented approximation; coupled patches are not handled.

use outram_foam_basic_lib::prelude::{FvMesh, PatchKind, Vector3, VolVectorField};
use petir::mathf::RealMath;

/// Upstream `SMALL` and `VSMALL` for double precision.
const SMALL: f64 = 1e-15;
const VSMALL: f64 = 1e-300;
/// Upstream `GREAT`.
const GREAT: f64 = 1e15;

fn umax(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

fn umin(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

fn dot(a: Vector3, b: Vector3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn mag(a: Vector3) -> f64 {
    dot(a, a).sqrt()
}

/// Upstream `mesh.cells()`: for each cell the faces it owns or neighbours, in ascending face
/// index (the order `primitiveMesh::calcCells` builds).
pub fn cell_faces(mesh: &FvMesh) -> Vec<Vec<usize>> {
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.n_faces {
        cells[mesh.owner[f]].push(f);
        if f < mesh.n_internal_faces {
            cells[mesh.neighbour[f]].push(f);
        }
    }
    cells
}

/// `maxDeltaxyz::calcDelta()`: `delta_c = deltaCoeff * max_f |n_f . (Cf_f - C_c)|`,
/// `n_f = Sf/|Sf|`.
pub fn max_delta_xyz(mesh: &FvMesh, delta_coeff: f64) -> Vec<f64> {
    let cells = cell_faces(mesh);
    cells
        .iter()
        .enumerate()
        .map(|(c, faces)| {
            let cc = mesh.cell_centres[c];
            let mut max_tmp = 0.0_f64;
            for &f in faces {
                let sf = mesh.face_area_vectors[f];
                let m = mag(sf);
                let n = Vector3::new(sf.x / m, sf.y / m, sf.z / m);
                let tmp = dot(n, mesh.face_centres[f] - cc).abs();
                if tmp > max_tmp {
                    max_tmp = tmp;
                }
            }
            delta_coeff * max_tmp
        })
        .collect()
}

/// `IDDESDelta::calcDelta()` with the wall-normal `wall_n`, the wall distance `y` and `hmax`
/// (from [`max_delta_xyz`]) given: `min(max(max(Cw y, Cw hmax), faceToFaceMax), hmax)`, where
/// `faceToFaceMax = max_{i,j in cell faces} n_c . (Cf_j - Cf_i)`.
pub fn iddes_delta(mesh: &FvMesh, hmax: &[f64], wall_n: &[Vector3], y: &[f64], cw: f64) -> Vec<f64> {
    let cells = cell_faces(mesh);
    cells
        .iter()
        .enumerate()
        .map(|(c, faces)| {
            let mut max_delta = 0.0_f64;
            for &fi in faces {
                for &fj in faces {
                    let ndfc = dot(wall_n[c], mesh.face_centres[fj] - mesh.face_centres[fi]);
                    if ndfc > max_delta {
                        max_delta = ndfc;
                    }
                }
            }
            umin(umax(umax(cw * y[c], cw * hmax[c]), max_delta), hmax[c])
        })
        .collect()
}

/// The mesh faces of every `Wall` patch, in patch order (the order the per-wall-face arrays of
/// this module use).
pub fn wall_faces(mesh: &FvMesh) -> Vec<usize> {
    let mut out = Vec::new();
    for p in &mesh.patches {
        if p.kind == PatchKind::Wall {
            out.extend(p.start..p.start + p.size);
        }
    }
    out
}

/// `ystar = nu/sqrt((nu + nut) |snGrad(U)| + VSMALL)` on every wall face, with
/// `snGrad(U) = deltaCoeffs (U_wall - U_cell)` of the fixed-value patch and
/// `deltaCoeffs = 1/|Cf - C|`. `nu_w` and `nut_w` are the wall-face values in [`wall_faces`] order.
pub fn van_driest_ystar_faces(mesh: &FvMesh, u: &VolVectorField, nu_w: &[f64], nut_w: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    for (pi, p) in mesh.patches.iter().enumerate() {
        if p.kind != PatchKind::Wall {
            continue;
        }
        for fi in 0..p.size {
            let f = p.start + fi;
            let cell = mesh.owner[f];
            let k = out.len();
            let delta_coeffs = 1.0 / mag(mesh.face_centres[f] - mesh.cell_centres[cell]);
            let uw = u.boundary[pi].values[fi];
            let uc = u.internal[cell];
            let sn = Vector3::new(
                delta_coeffs * (uw.x - uc.x),
                delta_coeffs * (uw.y - uc.y),
                delta_coeffs * (uw.z - uc.z),
            );
            out.push(nu_w[k] / ((nu_w[k] + nut_w[k]) * mag(sn) + VSMALL).sqrt());
        }
    }
    out
}

/// For each cell the index (into [`wall_faces`]) of the nearest wall-face centre (brute force,
/// first of equals). Upstream maps from the `meshWave` nearest face instead; see the module docs.
pub fn nearest_wall_face(mesh: &FvMesh) -> Vec<usize> {
    let wf = wall_faces(mesh);
    (0..mesh.n_cells)
        .map(|c| {
            let cc = mesh.cell_centres[c];
            let mut best = 0;
            let mut best_d = f64::MAX;
            for (i, &f) in wf.iter().enumerate() {
                let d = (mesh.face_centres[f] - cc).mag();
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            best
        })
        .collect()
}

/// `y/ystar` per cell with upstream's cut: `GREAT` where `y` is unvisited (`> 0.5 GREAT`) or
/// `y/ystar > 500`.
pub fn van_driest_yplus(y: &[f64], ystar_mapped: &[f64]) -> Vec<f64> {
    y.iter()
        .zip(ystar_mapped)
        .map(|(&yc, &ys)| {
            let y_plus = yc / ys;
            if yc > 0.5 * GREAT || y_plus > 500.0 {
                GREAT
            } else {
                y_plus
            }
        })
        .collect()
}

/// `vanDriestDelta::calcDelta()` last step:
/// `min(geometric, (kappa/Cdelta)((1 + SMALL) - exp(-yplus/Aplus)) y)`; upstream defaults
/// `kappa` 0.41, `Aplus` 26, `Cdelta` 0.158.
pub fn van_driest_delta(
    geometric: &[f64],
    y: &[f64],
    yplus: &[f64],
    kappa: f64,
    a_plus: f64,
    c_delta: f64,
) -> Vec<f64> {
    (0..geometric.len())
        .map(|c| {
            umin(
                geometric[c],
                ((kappa / c_delta) * ((1.0 + SMALL) - ((-yplus[c]) / a_plus).r_exp())) * y[c],
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder};
    use std::sync::Arc;

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// Each cell is 1 wide, so `|n . (Cf - C)| = 0.5` for every face: `hmax = 2 * 0.5 = 1`; the
    /// cell-face lists follow ascending face index; IDDES clips by `hmax`.
    #[test]
    fn max_delta_xyz_and_iddes_on_a_line() {
        let m = line_mesh();
        assert_eq!(cell_faces(&m), vec![vec![0, 2], vec![0, 1], vec![1, 3]]);
        let h = max_delta_xyz(&m, 2.0);
        assert_eq!(h, vec![1.0, 1.0, 1.0]);
        let n = vec![vx(1.0); 3];
        let d = iddes_delta(&m, &h, &n, &[0.1, 10.0, 0.0], 0.15);
        // cell 0: max(0.015, 0.15) = 0.15 against faceToFace = 1.0 -> 1.0, clipped by hmax = 1
        // cell 1: Cw y = 1.5 > hmax -> hmax; cell 2: 0.15 vs faceToFace 1.0 -> 1.0
        assert_eq!(d, vec![1.0, 1.0, 1.0]);
    }

    /// The nearest wall face of every cell of the line mesh is the single wall face (index 0); a
    /// y+ above the cut-off or an unvisited `y` becomes `GREAT`; the van Driest damping vanishes
    /// at the wall (`y = 0`) and is capped by the geometric delta far away.
    #[test]
    fn van_driest_pieces() {
        let m = line_mesh();
        assert_eq!(wall_faces(&m), vec![2]);
        assert_eq!(nearest_wall_face(&m), vec![0, 0, 0]);
        let yp = van_driest_yplus(&[1.0, 1.0, 1e16], &[1.0, 0.001, 1.0]);
        assert_eq!(yp, vec![1.0, GREAT, GREAT]);
        let d = van_driest_delta(&[0.3, 0.3, 0.3], &[0.0, 1.0, 1.0], &[1.0, GREAT, 1.0], 0.41, 26.0, 0.158);
        assert_eq!(d[0], 0.0);
        assert_eq!(d[1], 0.3);
        assert!(d[2] < 0.3);
    }
}
