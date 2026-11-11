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

//! `fix mesh/surface/stress`: force and torque the particles exert on a mesh,
//! and the per-facet normal and shear stress (gh:#1416). Translated from
//! `mesh_module_stress.cpp` of LIGGGHTS-PUBLIC `3d5c00f2`
//! (`MeshModuleStress::pre_force`, `add_particle_contribution`,
//! `calc_total_force`, `compute_vector`); the call site is
//! `fix_wall_gran_base.h` (`f_pw = f - force_old`, `delta = -sidata.delta`).
//!
//! Translated: the stress half (`stress on`, `reference_point`): total force
//! `f_total`, total torque about the reference point, per-facet force `f`,
//! `sigma_n`, `sigma_t` (both `|F.n|/A`, `|F - (F.n)n| / A`), and the
//! `compute_vector` layout (`0..3` force, `3..6` torque, `6..9` the reference
//! point, which moves with the mesh). **Not translated:** `wear finnie`,
//! `mesh_module_stress_servo` (the servo wall), external (gravity)
//! contributions to the totals. The reference point follows the mesh's
//! translation exactly and its rotation about the world origin as upstream's
//! `TrackingMesh::rotate` does (`move(-origin)`, rotate, `move(origin)`);
//! a rotation about a point other than the origin is not cross-coded.

use crate::particle::Vec3;

/// `fix mesh/surface/stress` state of one mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshStress {
    /// Torque reference point `p_ref` `[m]` (moves with the mesh).
    pub p_ref: Vec3,
    f_total: [f64; 3],
    torque_total: [f64; 3],
    f_total_old: [f64; 3],
    torque_total_old: [f64; 3],
    updated: bool,
    f_tri: Vec<[f64; 3]>,
    sigma_n: Vec<f64>,
    sigma_t: Vec<f64>,
    area: Vec<f64>,
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

impl MeshStress {
    /// A stress module for a mesh of `area.len()` facets with the given facet
    /// areas (`areaElem`) and torque reference point.
    #[must_use]
    pub fn new(p_ref: Vec3, area: Vec<f64>) -> Self {
        let n = area.len();
        Self {
            p_ref,
            f_total: [0.0; 3],
            torque_total: [0.0; 3],
            f_total_old: [0.0; 3],
            torque_total_old: [0.0; 3],
            updated: false,
            f_tri: vec![[0.0; 3]; n],
            sigma_n: vec![0.0; n],
            sigma_t: vec![0.0; n],
            area,
        }
    }

    /// `pre_force`: keep last step's totals, start a new sum.
    pub fn pre_force(&mut self) {
        self.f_total_old = self.f_total;
        self.torque_total_old = self.torque_total;
        self.f_total = [0.0; 3];
        self.torque_total = [0.0; 3];
        self.updated = false;
    }

    /// `add_particle_contribution(ip, frc, delta, iTri, v_wall)` for stress:
    /// `frc` is the contact force on the particle (`f_pw`), `x` the particle
    /// centre, `delta` the already negated `-sidata.delta`.
    pub fn add_particle_contribution(&mut self, frc: [f64; 3], x: [f64; 3], delta: [f64; 3], tri: usize) {
        let frc = [-frc[0], -frc[1], -frc[2]];
        let contact = add(x, delta);
        self.f_tri[tri] = add(self.f_tri[tri], frc);
        self.f_total = add(self.f_total, frc);
        let tmp = [contact[0] - self.p_ref.x, contact[1] - self.p_ref.y, contact[2] - self.p_ref.z];
        let tmp2 = [
            tmp[1] * frc[2] - tmp[2] * frc[1],
            tmp[2] * frc[0] - tmp[0] * frc[2],
            tmp[0] * frc[1] - tmp[1] * frc[0],
        ];
        self.torque_total = add(self.torque_total, tmp2);
    }

    /// `calc_total_force` (from `final_integrate`): mark the totals current and
    /// compute the facet stresses from the current unit normals `norms`.
    pub fn final_integrate(&mut self, norms: &[[f64; 3]]) {
        self.updated = true;
        for i in 0..self.f_tri.len() {
            let n = norms[i];
            let f = self.f_tri[i];
            let inv_area = 1.0 / self.area[i];
            let mut sn = n[0] * f[0] + n[1] * f[1] + n[2] * f[2];
            let temp = [n[0] * sn, n[1] * sn, n[2] * sn];
            let t = [f[0] - temp[0], f[1] - temp[1], f[2] - temp[2]];
            let mut st = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            sn = sn.abs();
            st = st.abs();
            sn *= inv_area;
            st *= inv_area;
            self.sigma_n[i] = sn;
            self.sigma_t[i] = st;
        }
    }

    /// `compute_vector(n)`: `0..3` total force on the mesh `[N]`, `3..6`
    /// total torque about `p_ref` `[N m]`, `6..9` `p_ref`.
    #[must_use]
    pub fn compute_vector(&self, n: usize) -> f64 {
        if n < 3 {
            if self.updated {
                self.f_total[n]
            } else {
                self.f_total_old[n]
            }
        } else if n < 6 {
            if self.updated {
                self.torque_total[n - 3]
            } else {
                self.torque_total_old[n - 3]
            }
        } else if n < 9 {
            [self.p_ref.x, self.p_ref.y, self.p_ref.z][n - 6]
        } else {
            0.0
        }
    }

    /// Force on facet `i` `[N]`. **Cumulative over the run**: upstream zeroes
    /// the per-facet `f` only in `post_create` (`zeroizeStress`), never per
    /// step (read from `mesh_module_stress.cpp`; the per-facet values are not
    /// cross-coded, only the totals are).
    #[must_use]
    pub fn facet_force(&self, i: usize) -> Vec3 {
        Vec3::new(self.f_tri[i][0], self.f_tri[i][1], self.f_tri[i][2])
    }

    /// Normal stress `|F.n| / A` of facet `i` `[Pa]`.
    #[must_use]
    pub fn sigma_n(&self, i: usize) -> f64 {
        self.sigma_n[i]
    }

    /// Shear stress `|F - (F.n) n| / A` of facet `i` `[Pa]`.
    #[must_use]
    pub fn sigma_t(&self, i: usize) -> f64 {
        self.sigma_t[i]
    }
}
