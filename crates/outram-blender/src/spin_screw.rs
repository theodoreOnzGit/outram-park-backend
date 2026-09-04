// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Spin and Screw. Follows the published behaviour of Blender's edit-mode spin
// and screw operators (source/blender/bmesh/operators/bmo_create.cc
// `bmo_spin_exec` and the screw modifier MOD_screw.cc, github.com/blender/
// blender, GPL-2.0-or-later): rotate-copy a profile selection around an axis
// (spin), optionally translating along the axis each step for a helix (screw).
// Concepts only — no upstream source copied; polygon-soup rebuild. Distinct
// from `revolve`, which sweeps an explicit polyline rather than a mesh
// selection.
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

//! **Spin / Screw** (`op-hzs.54.22`, GH issue #37 §C).
//!
//! - [`spin`] — rotate-copy an ordered profile of vertices `steps` times over
//!   `angle` about an axis through `center`, bridging consecutive copies into a
//!   surface. `use_duplicates` places the copies without bridging.
//! - [`screw`] — [`spin`] plus a translation of `screw_offset` along the axis
//!   spread over the whole sweep, so `turns` revolutions trace a helix.
//!
//! For an explicit polyline (rather than a mesh selection) use
//! [`crate::revolve`].

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};
use crate::selection::Axis;

/// Rotate-copy `profile` (an ordered vertex chain) around the `axis` line
/// through `center`, `steps` times over `angle` radians. When `!use_duplicates`
/// the consecutive copies are bridged into quads. Returns the mesh with the new
/// geometry appended.
pub fn spin(
    mesh: &Mesh,
    profile: &[VertexId],
    center: Vec3,
    axis: Axis,
    angle: f64,
    steps: usize,
    use_duplicates: bool,
) -> Mesh {
    sweep(
        mesh,
        profile,
        center,
        axis,
        angle,
        0.0,
        steps.max(1),
        use_duplicates,
    )
}

/// [`spin`] with an axial translation: the profile advances `screw_offset`
/// along the axis over `turns` full revolutions in `steps` steps, tracing a
/// helix. Always bridged.
pub fn screw(
    mesh: &Mesh,
    profile: &[VertexId],
    center: Vec3,
    axis: Axis,
    screw_offset: f64,
    turns: f64,
    steps: usize,
) -> Mesh {
    let angle = turns * std::f64::consts::TAU;
    sweep(
        mesh,
        profile,
        center,
        axis,
        angle,
        screw_offset,
        steps.max(1),
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn sweep(
    mesh: &Mesh,
    profile: &[VertexId],
    center: Vec3,
    axis: Axis,
    angle: f64,
    axial: f64,
    steps: usize,
    use_duplicates: bool,
) -> Mesh {
    if profile.len() < 2 {
        return mesh.clone();
    }
    let src = mesh.positions();
    let base: Vec<Vec3> = profile
        .iter()
        .filter_map(|v| src.get(v.0).copied())
        .collect();
    if base.len() != profile.len() {
        return mesh.clone();
    }
    let k = axis_unit(axis);

    let mut positions = src.clone();
    let mut faces: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect();

    let full_turn = (angle - std::f64::consts::TAU).abs() < 1e-6;
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(steps + 1);
    // step 0 reuses the original profile vertex ids.
    rings.push(profile.iter().map(|v| v.0).collect());
    let ring_count = if full_turn && !use_duplicates {
        steps
    } else {
        steps + 1
    };
    for s in 1..ring_count {
        let a = angle * s as f64 / steps as f64;
        let t = axial * s as f64 / steps as f64;
        let ring: Vec<usize> = base
            .iter()
            .map(|&p| {
                let rel = p.sub(center);
                let rotated = rotate_about(rel, k, a);
                positions.push(center.add(rotated).add(k.scale(t)));
                positions.len() - 1
            })
            .collect();
        rings.push(ring);
    }

    if !use_duplicates {
        let seg_rings: Vec<&Vec<usize>> = if full_turn {
            rings.iter().chain(std::iter::once(&rings[0])).collect()
        } else {
            rings.iter().collect()
        };
        for w in seg_rings.windows(2) {
            let (r0, r1) = (w[0], w[1]);
            for i in 0..base.len() - 1 {
                faces.push(vec![r0[i], r0[i + 1], r1[i + 1], r1[i]]);
            }
        }
    } else {
        // Instances: keep the copies as isolated wire chains (a sliver tri per
        // segment records each new edge in the soup model).
        for ring in rings.iter().skip(1) {
            for i in 0..ring.len() - 1 {
                faces.push(vec![ring[i], ring[i + 1], ring[i]]);
            }
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

/// Rotate `v` about unit axis `k` by `theta` (Rodrigues).
fn rotate_about(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let (s, c) = theta.sin_cos();
    v.scale(c)
        .add(k.cross(v).scale(s))
        .add(k.scale(k.dot(v) * (1.0 - c)))
}

/// The unit vector for an [`Axis`].
fn axis_unit(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(1.0, 0.0, 0.0),
        Axis::Y => Vec3::new(0.0, 1.0, 0.0),
        Axis::Z => Vec3::new(0.0, 0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// A short vertical profile chain (a line of 3 verts along Z at radius 1).
    fn profile_mesh() -> (Mesh, Vec<VertexId>) {
        let mut m = Mesh::new();
        let v: Vec<VertexId> = (0..3)
            .map(|i| m.add_vertex(Vec3::new(1.0, 0.0, i as f64)))
            .collect();
        m.add_face(&[v[0], v[1], v[1]]); // dummy sliver to keep verts
        (m, v)
    }

    #[test]
    fn spin_a_profile_into_a_cylinder_band() {
        let (m, prof) = profile_mesh();
        let s = spin(
            &m,
            &prof,
            Vec3::ZERO,
            Axis::Z,
            std::f64::consts::TAU,
            12,
            false,
        );
        // 12 segments * (3-1)=2 quads = 24 new faces + dummy.
        assert_eq!(s.face_count(), 1 + 24);
        // A full turn reuses ring[0] as the closing ring: 11 new rings * 3.
        assert_eq!(s.vertex_count(), 3 + 11 * 3);
        // The vertical edges of the band are all shared by two quads.
        assert!(vertical_edges_paired(&s));
    }

    #[test]
    fn spin_half_turn_is_open() {
        let (m, prof) = profile_mesh();
        let s = spin(
            &m,
            &prof,
            Vec3::ZERO,
            Axis::Z,
            std::f64::consts::PI,
            6,
            false,
        );
        // 6 segments -> 7 rings -> 6*2 = 12 quads + dummy.
        assert_eq!(s.face_count(), 1 + 12);
    }

    #[test]
    fn screw_traces_a_helix() {
        let (m, prof) = profile_mesh();
        let s = screw(&m, &prof, Vec3::ZERO, Axis::Z, 4.0, 2.0, 24);
        // The last ring's first vertex should be ~4 units higher than the base.
        let last = s.vertex(VertexId(s.vertex_count() - 3)).unwrap().position;
        assert!(last.z > 3.5, "advanced ~4 along the axis over 2 turns");
    }

    #[test]
    fn use_duplicates_does_not_bridge() {
        let (m, prof) = profile_mesh();
        let dup = spin(
            &m,
            &prof,
            Vec3::ZERO,
            Axis::Z,
            std::f64::consts::TAU,
            4,
            true,
        );
        let bridged = spin(
            &m,
            &prof,
            Vec3::ZERO,
            Axis::Z,
            std::f64::consts::TAU,
            4,
            false,
        );
        let quads = |mm: &Mesh| mm.polygons().iter().filter(|p| p.len() == 4).count();
        assert_eq!(quads(&dup), 0, "duplicates path bridges nothing");
        assert!(quads(&bridged) > 0);
    }

    /// Every "vertical" quad edge (endpoints at different z) is shared by two
    /// quads — the band closed around the axis.
    fn vertical_edges_paired(m: &Mesh) -> bool {
        let pos = m.positions();
        let mut count: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();
        for poly in m.polygons() {
            if poly.len() != 4 {
                continue;
            }
            for i in 0..4 {
                let (a, b) = (poly[i].0, poly[(i + 1) % 4].0);
                if (pos[a].z - pos[b].z).abs() > 1e-9 {
                    *count.entry((a.min(b), a.max(b))).or_default() += 1;
                }
            }
        }
        count.values().all(|&c| c == 2)
    }

    #[test]
    fn short_profile_is_a_noop() {
        let mut m = primitives::cube(2.0);
        let before = m.face_count();
        m = spin(&m, &[VertexId(0)], Vec3::ZERO, Axis::Z, 1.0, 4, false);
        assert_eq!(m.face_count(), before);
    }
}
