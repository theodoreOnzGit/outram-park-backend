// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/polyMeshGen2DEngine/polyMeshGen2DEngine.{C,H}
//   meshLibrary/utilities/meshes/polyMeshGen2DEngine/polyMeshGen2DEngineI.H
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

//! cfMesh **`polyMeshGen2DEngine`** (gh:#1275): which points of a mesh one
//! cell thick in z lie at its lowest z and which at its highest (within 5 %
//! of the height), which faces are **active** (hold both: the side faces,
//! not the front and back), the offset point across the thickness of every
//! point, and `correctPoints` (each back point under its front point, at
//! exactly the two z values). Computed eagerly here; upstream computes each
//! on demand. Upstream's `FatalError`s are the `Err`s.

use crate::math::Vec3;
use crate::volume_mesh::VolumeMesh;

/// The 2D addressing of one mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct PolyMeshGen2DEngine {
    /// Bounding box of the mesh points (`bb_`): (min, max).
    pub bb: (Vec3, Vec3),
    /// `zMinPoints`.
    pub z_min_point: Vec<bool>,
    /// `zMaxPoints`.
    pub z_max_point: Vec<bool>,
    /// `activeFace`: the face holds a z-min and a z-max point.
    pub active_face: Vec<bool>,
    /// `zMinPointLabels`.
    pub z_min_point_labels: Vec<usize>,
    /// `zMinToZMax`: the point across the thickness of each
    /// `z_min_point_labels` entry.
    pub z_min_to_z_max: Vec<usize>,
    /// `zMaxPointLabels`.
    pub z_max_point_labels: Vec<usize>,
    /// `zMaxToZMin`.
    pub z_max_to_z_min: Vec<usize>,
}

/// `addressingData().pointPoints()`: the points joined to each point by an
/// edge of a face.
fn point_points(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut pp: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for ring in &mesh.faces {
        let k = ring.len();
        for i in 0..k {
            let (a, b) = (ring[i], ring[(i + 1) % k]);
            if !pp[a].contains(&b) {
                pp[a].push(b);
            }
            if !pp[b].contains(&a) {
                pp[b].push(a);
            }
        }
    }
    pp
}

impl PolyMeshGen2DEngine {
    /// The engine of `mesh` (all of upstream's demand-driven data).
    pub fn new(mesh: &VolumeMesh) -> Result<Self, String> {
        let mut mn = Vec3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut mx = Vec3::new(-f64::MAX, -f64::MAX, -f64::MAX);
        for p in &mesh.points {
            mn = Vec3::new(mn.x.min(p.x), mn.y.min(p.y), mn.z.min(p.z));
            mx = Vec3::new(mx.x.max(p.x), mx.y.max(p.y), mx.z.max(p.z));
        }
        let t_z = 0.05 * (mx.z - mn.z);
        let z_min_point: Vec<bool> = mesh.points.iter().map(|p| (p.z - mn.z).abs() < t_z).collect();
        let z_max_point: Vec<bool> = mesh.points.iter().map(|p| (p.z - mx.z).abs() < t_z).collect();
        let active_face = mesh
            .faces
            .iter()
            .map(|f| f.iter().any(|&v| z_min_point[v]) && f.iter().any(|&v| z_max_point[v]))
            .collect();
        let pp = point_points(mesh);
        let labels = |flag: &[bool], what: &str| -> Result<Vec<usize>, String> {
            let l: Vec<usize> = (0..flag.len()).filter(|&i| flag[i]).collect();
            if 2 * l.len() != flag.len() {
                return Err(format!(
                    "The number of points at {what} z coordinate is not half of the total number of points. This is not a 2D mesh or is not aligned with the z axis"
                ));
            }
            Ok(l)
        };
        let offsets = |flag: &[bool], l: &[usize]| -> Result<Vec<usize>, String> {
            l.iter()
                .map(|&p| {
                    let other: Vec<usize> = pp[p].iter().copied().filter(|&q| !flag[q]).collect();
                    if other.len() == 1 {
                        Ok(other[0])
                    } else {
                        Err("This cannot be a 2D mesh".to_string())
                    }
                })
                .collect()
        };
        let z_min_point_labels = labels(&z_min_point, "smallest")?;
        let z_max_point_labels = labels(&z_max_point, "largest")?;
        let z_min_to_z_max = offsets(&z_min_point, &z_min_point_labels)?;
        let z_max_to_z_min = offsets(&z_max_point, &z_max_point_labels)?;
        Ok(PolyMeshGen2DEngine { bb: (mn, mx), z_min_point, z_max_point, active_face, z_min_point_labels, z_min_to_z_max, z_max_point_labels, z_max_to_z_min })
    }

    /// `correctPoints`: every z-max point takes the x, y of the z-min point
    /// across from it; z-min points go to `bb.min.z`, z-max to `bb.max.z`.
    pub fn correct_points(&self, points: &mut [Vec3]) {
        for (i, &p) in self.z_min_point_labels.iter().enumerate() {
            let o = self.z_min_to_z_max[i];
            points[o].x = points[p].x;
            points[o].y = points[p].y;
            points[p].z = self.bb.0.z;
            points[o].z = self.bb.1.z;
        }
    }
}
