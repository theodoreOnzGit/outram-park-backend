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
//! external (gravity) contributions to the totals. The reference point follows the mesh's
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
    pub fn add_particle_contribution(
        &mut self,
        frc: [f64; 3],
        x: [f64; 3],
        delta: [f64; 3],
        tri: usize,
    ) {
        let frc = [-frc[0], -frc[1], -frc[2]];
        let contact = add(x, delta);
        self.f_tri[tri] = add(self.f_tri[tri], frc);
        self.f_total = add(self.f_total, frc);
        let tmp = [
            contact[0] - self.p_ref.x,
            contact[1] - self.p_ref.y,
            contact[2] - self.p_ref.z,
        ];
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

    /// The total torque on the mesh about `p_ref` summed so far this step
    /// `[N m]` (the `torque_total_ptr()` the torque servo reads).
    #[must_use]
    pub fn torque_total(&self) -> [f64; 3] {
        self.torque_total
    }

    /// The total force on the mesh summed so far this step `[N]` (the
    /// `f_total_ptr()` the servo module reads in `final_integrate`).
    #[must_use]
    pub fn f_total(&self) -> [f64; 3] {
        self.f_total
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

/// Controlled process value of `mesh_module_stress_servo` (`ctrlPV`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServoCtrl {
    /// `ctrlPV force`: the mesh translates along `axis`.
    Force,
    /// `ctrlPV torque`: the mesh rotates about `axis` through `com`.
    Torque,
}

/// How `MeshModuleStressServo::initial_integrate` moves the mesh this step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServoMove {
    /// `mesh->move(dX, dx)`: every node at its original position plus `d_x`.
    Translate {
        /// Total displacement `xcm - xcm_orig` `[m]`.
        d_x: [f64; 3],
    },
    /// `mesh->rotate(totalPhi, incrementalPhi, axis, xcm)`: every node at its
    /// original position rotated by `total_phi` about the axis through
    /// `origin`; cached edge vectors and normals rotate by `inc_phi`.
    Rotate {
        /// Total angle since the start `[rad]`.
        total_phi: f64,
        /// Angle of this step `[rad]`.
        inc_phi: f64,
        /// The (normalised) axis.
        axis: [f64; 3],
        /// Axis point `xcm` `[m]`.
        origin: [f64; 3],
    },
}

/// `mesh_module_stress_servo` (`mesh_module_stress_servo.cpp`,
/// LIGGGHTS-PUBLIC `3d5c00f2`): the mesh translates rigidly (`ctrlPV force`)
/// or rotates (`ctrlPV torque`) with a velocity chosen each step to drive the
/// force or torque `pv . axis` towards `-target_val`, by the PID controller or
/// by `mode auto` (a piecewise proportional gain on the number of mesh
/// contacts). gh:#1416.
///
/// **Not ported:** a variable set point (`target_val v_name`),
/// `fix_modify integrate/vel_max/target_val/ctrlParam/ratio`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshServo {
    ctrl: ServoCtrl,
    axis: [f64; 3],
    /// Mesh centre of mass `xcm_` `[m]`.
    xcm: [f64; 3],
    xcm_orig: [f64; 3],
    /// Servo velocity `vcm_` `[m/s]` (`ctrlPV force`).
    vcm: [f64; 3],
    /// Servo angular velocity `omegacm_` `[rad/s]` (`ctrlPV torque`).
    omegacm: [f64; 3],
    total_phi: f64,
    sp_mag: f64,
    sp_mag_inv: f64,
    kp: f64,
    ki: f64,
    kd: f64,
    vel_max: f64,
    /// `rPaMax` of the torque servo (largest node distance from the axis).
    r_pa_max: f64,
    /// `vel_min_ = ratio * r_min / dt` of `mode auto`; `None` is PID mode.
    vel_min: Option<f64>,
    err: f64,
    sum_err: f64,
    old_pv_mag: f64,
    dtv: f64,
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn mag3(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}

impl MeshServo {
    /// `ctrlPV force` servo: `com`, `axis`, `target_val`, `vel_max`, gains
    /// `[kp, ki, kd]` of the fix line, the time step and the neighbour skin.
    /// `axis` is normalised as `vectorNormalize3D` does.
    ///
    /// # Panics
    ///
    /// As upstream aborts: `target_val == 0`, `vel_max <= 0`, any gain
    /// negative or all gains zero, or `vel_max >= skin / (2 dt)`.
    #[must_use]
    pub fn new(
        com: [f64; 3],
        axis: [f64; 3],
        target_val: f64,
        vel_max: f64,
        gains: [f64; 3],
        dt: f64,
        skin: f64,
    ) -> Self {
        let sp_mag = -target_val;
        assert!(sp_mag != 0.0, "'target_val' has to be != 0.0");
        assert!(vel_max > 0.0, "vel_max > 0 required");
        let [kp, ki, kd] = gains;
        assert!(kp >= 0.0 && ki >= 0.0 && kd >= 0.0, "kp, ki, kd >= 0 required");
        assert!(kp != 0.0 || ki != 0.0 || kd != 0.0, "kp, ki, and kd are zero");
        assert!(vel_max < skin / (2.0 * dt), "vel_max < skin/(2.*dt) required");
        let norm = mag3(axis);
        let inv = if norm == 0. { 0. } else { 1. / norm };
        Self {
            ctrl: ServoCtrl::Force,
            axis: [axis[0] * inv, axis[1] * inv, axis[2] * inv],
            xcm: com,
            xcm_orig: com,
            vcm: [0.0; 3],
            omegacm: [0.0; 3],
            total_phi: 0.0,
            sp_mag,
            sp_mag_inv: 1. / sp_mag.abs(),
            kp,
            ki,
            kd,
            vel_max,
            r_pa_max: 1.0,
            vel_min: None,
            err: 0.0,
            sum_err: 0.0,
            old_pv_mag: 0.0,
            dtv: dt,
        }
    }

    /// `ctrlPV torque` (`init()`): the maximum angular velocity is
    /// `vel_max / rPaMax`, `rPaMax` the largest distance of a mesh node from
    /// the axis through `com` (`getMaxRad`). `nodes` are the mesh nodes at
    /// set-up.
    ///
    /// # Panics
    ///
    /// As upstream aborts: every node on the rotation axis.
    #[must_use]
    pub fn with_torque(mut self, nodes: &[[[f64; 3]; 3]]) -> Self {
        let mut r_pa_max = 0.0_f64;
        for nd in nodes {
            for node in nd {
                let r_pa = [
                    node[0] - self.xcm[0],
                    node[1] - self.xcm[1],
                    node[2] - self.xcm[2],
                ];
                let m = mag3(cross3(self.axis, r_pa));
                if m > r_pa_max {
                    r_pa_max = m;
                }
            }
        }
        assert!(
            r_pa_max != 0.0,
            "All mesh nodes are located at the rotation axis."
        );
        self.ctrl = ServoCtrl::Torque;
        self.r_pa_max = r_pa_max;
        self
    }

    /// `mode auto ratio <ratio>`: the piecewise proportional controller.
    /// `r_min` is the smallest particle radius at set-up (`init()`), so
    /// `vel_min = ratio * r_min / dt`.
    ///
    /// # Panics
    ///
    /// As upstream aborts: `ratio == 0`.
    #[must_use]
    pub fn with_auto(mut self, ratio: f64, r_min: f64) -> Self {
        assert!(
            ratio != 0.0,
            "please define 'ratio' for the mesh, since you use the auto mode"
        );
        self.vel_min = Some(ratio * r_min / self.dtv);
        self
    }

    /// The controlled process value.
    #[must_use]
    pub fn ctrl(&self) -> ServoCtrl {
        self.ctrl
    }

    /// `true` for `mode auto`.
    #[must_use]
    pub fn is_auto(&self) -> bool {
        self.vel_min.is_some()
    }

    /// `ctrl_op_max_`: `vel_max` (force) or `vel_max / rPaMax` (torque).
    fn ctrl_op_max(&self) -> f64 {
        match self.ctrl {
            ServoCtrl::Force => self.vel_max,
            ServoCtrl::Torque => self.vel_max / self.r_pa_max,
        }
    }

    /// `ctrl_op_min_`: `vel_min` (force) or `vel_min / rPaMax` (torque).
    fn ctrl_op_min(&self) -> f64 {
        let v = self.vel_min.unwrap_or(0.0);
        match self.ctrl {
            ServoCtrl::Force => v,
            ServoCtrl::Torque => v / self.r_pa_max,
        }
    }

    /// `initial_integrate`: advance the controlled state by one step and say
    /// how the mesh moves.
    pub fn initial_integrate(&mut self) -> ServoMove {
        match self.ctrl {
            ServoCtrl::Force => {
                let dx = [
                    self.dtv * self.vcm[0],
                    self.dtv * self.vcm[1],
                    self.dtv * self.vcm[2],
                ];
                self.xcm = [
                    self.xcm[0] + dx[0],
                    self.xcm[1] + dx[1],
                    self.xcm[2] + dx[2],
                ];
                ServoMove::Translate {
                    d_x: [
                        self.xcm[0] - self.xcm_orig[0],
                        self.xcm[1] - self.xcm_orig[1],
                        self.xcm[2] - self.xcm_orig[2],
                    ],
                }
            }
            ServoCtrl::Torque => {
                let inc_phi = self.dtv * dot3(self.omegacm, self.axis);
                self.total_phi += inc_phi;
                ServoMove::Rotate {
                    total_phi: self.total_phi,
                    inc_phi,
                    axis: self.axis,
                    origin: self.xcm,
                }
            }
        }
    }

    /// `final_integrate` (PID branch) followed by `limit_vel`, with no mesh
    /// contact count: the force servo's PID entry point. See
    /// [`MeshServo::final_integrate_pv`].
    pub fn final_integrate(&mut self, f_total: [f64; 3]) -> [f64; 3] {
        self.final_integrate_pv(f_total, 0)
    }

    /// `final_integrate` (PID or `mode auto`) followed by `limit_vel`: the new
    /// `vcm` (force) or `omegacm` (torque) from the process-value vector `pv`
    /// (`f_total` or `torque_total`) and the mesh's total contact count
    /// `num_contacts` (`getTotalNumContacts`, only read in `mode auto`).
    pub fn final_integrate_pv(&mut self, pv: [f64; 3], num_contacts: usize) -> [f64; 3] {
        let pv_mag = dot3(pv, self.axis);
        let auto = self.is_auto();
        let mut ctrl_op: [f64; 3];
        if auto {
            self.err = (self.sp_mag - pv_mag) * self.sp_mag_inv;
            let err_low = 0.9;
            let err_high = 1.0;
            let ctrl_scale = 0.1;
            let op_min = self.ctrl_op_min();
            let ctrl_kp = if num_contacts == 0 {
                self.ctrl_op_max()
            } else if self.err.abs() <= err_low {
                ctrl_scale * op_min
            } else if self.err.abs() >= err_high {
                op_min
            } else {
                ctrl_scale * op_min
                    + ((1.0 - ctrl_scale) * op_min) * (self.err.abs() - err_low)
                        / (err_high - err_low)
            };
            let s = -ctrl_kp * self.err;
            ctrl_op = [self.axis[0] * s, self.axis[1] * s, self.axis[2] * s];
        } else {
            self.err = self.sp_mag - pv_mag;
            self.sum_err += self.err * self.dtv;
            let dfdt = -(pv_mag - self.old_pv_mag) / self.dtv;
            let mag = -self.ctrl_op_max()
                * (self.err * self.kp + self.sum_err * self.ki + dfdt * self.kd)
                * self.sp_mag_inv;
            ctrl_op = [self.axis[0] * mag, self.axis[1] * mag, self.axis[2] * mag];
            self.old_pv_mag = pv_mag;
        }
        // limit_vel
        let vmag = mag3(ctrl_op);
        let max_output = if auto && num_contacts > 0 {
            self.ctrl_op_min()
        } else {
            self.ctrl_op_max()
        };
        if vmag > max_output && vmag != 0.0 {
            let factor = max_output / vmag;
            for c in &mut ctrl_op {
                *c *= factor;
            }
            if self.ki > 0.0 {
                let out = dot3(ctrl_op, self.axis);
                let sgn = f64::from(i32::from(0.0 < out) - i32::from(out < 0.0));
                self.sum_err = (-sgn * self.sp_mag - self.err * self.kp) / self.ki;
            }
        }
        match self.ctrl {
            ServoCtrl::Force => self.vcm = ctrl_op,
            ServoCtrl::Torque => self.omegacm = ctrl_op,
        }
        ctrl_op
    }

    /// `set_v_node_rotate`: velocity `omegacm x (node - xcm)` of a node.
    #[must_use]
    pub fn node_velocity_rotate(&self, node: [f64; 3]) -> [f64; 3] {
        let r_pa = [
            node[0] - self.xcm[0],
            node[1] - self.xcm[1],
            node[2] - self.xcm[2],
        ];
        cross3(self.omegacm, r_pa)
    }

    /// Mesh centre `xcm_` `[m]` (`f_<mesh>[10..12]` after the stress nine).
    #[must_use]
    pub fn xcm(&self) -> [f64; 3] {
        self.xcm
    }

    /// Servo velocity `vcm_` `[m/s]`.
    #[must_use]
    pub fn vcm(&self) -> [f64; 3] {
        self.vcm
    }

    /// Servo angular velocity `omegacm_` `[rad/s]`.
    #[must_use]
    pub fn omegacm(&self) -> [f64; 3] {
        self.omegacm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn servo() -> MeshServo {
        MeshServo::new(
            [0., 0., 1.],
            [0., 0., 2.],
            0.1,
            0.05,
            [1.0, 0.0, 0.0],
            1e-5,
            0.003,
        )
    }

    /// **Methodology.** `torque_total` returns the torque summed in
    /// `add_particle_contribution`: a unit force `(1,0,0)` on the particle
    /// from a contact at `(0,1,0)` about `p_ref = 0` acts on the mesh as
    /// `(-1,0,0)`, so `r x F = (0,0,1)`. **Result:** exact.
    #[test]
    fn torque_total_sums_contact_torque() {
        let mut st = MeshStress::new(Vec3::zero(), vec![1.0]);
        st.pre_force();
        st.add_particle_contribution([1., 0., 0.], [0., 1., 0.], [0., 0., 0.], 0);
        assert_eq!(st.torque_total(), [0., 0., 1.]);
    }

    /// **Methodology.** `with_torque`: nodes at radii 0.3 and 0.4 from the axis
    /// give `rPaMax = 0.4`, `ctrl_op_max = vel_max / 0.4 = 0.125`; the torque
    /// servo's `initial_integrate` rotates by `dt * omega . axis` and
    /// accumulates the total; `node_velocity_rotate` is `omega x (node - xcm)`.
    #[test]
    fn torque_servo_geometry_and_rotation() {
        let nodes = [[[0.3, 0., 1.], [0., 0.4, 1.], [0.1, 0.1, 1.]]];
        let mut s = servo().with_torque(&nodes);
        assert_eq!(s.ctrl(), ServoCtrl::Torque);
        assert!(!s.is_auto());
        // sp = -0.1, pv = 0 -> err = -0.1; mag = -0.125 * (-0.1) * 10
        let w = s.final_integrate_pv([0., 0., 0.], 0);
        assert!((w[2] - 0.125).abs() < 1e-12, "{w:?}");
        assert_eq!(s.omegacm(), w);
        match s.initial_integrate() {
            ServoMove::Rotate {
                total_phi,
                inc_phi,
                origin,
                ..
            } => {
                assert!((inc_phi - 1.25e-6).abs() < 1e-18 && total_phi == inc_phi);
                assert_eq!(origin, [0., 0., 1.]);
            }
            ServoMove::Translate { .. } => panic!("torque servo must rotate"),
        }
        let v = s.node_velocity_rotate([0.3, 0., 1.]);
        assert!((v[1] - 0.125 * 0.3).abs() < 1e-15 && v[0] == 0.0);
    }

    /// **Methodology.** `mode auto` gain with `vel_min = ratio r_min / dt =
    /// 0.1`: no contacts -> cruise at `ctrl_op_max` (0.05); contacts with
    /// `|err| >= 1` -> `vel_min`, capped by `limit_vel` at `vel_min`;
    /// `|err| <= 0.9` -> `0.1 vel_min`; between, the linear blend.
    #[test]
    fn auto_mode_piecewise_gain() {
        let mk = || servo().with_auto(2e-4, 0.005);
        let mut s = mk();
        assert!(s.is_auto());
        let v = s.final_integrate_pv([0., 0., 0.], 0);
        assert!((v[2] - 0.05).abs() < 1e-15, "{v:?}");
        let mut s = mk();
        let v = s.final_integrate_pv([0., 0., 0.], 3);
        assert!((v[2] - 0.1).abs() < 1e-15, "{v:?}");
        // pv = -0.05: err = (-0.1 + 0.05)/0.1 = -0.5, ctrl_kp = 0.01
        let mut s = mk();
        let v = s.final_integrate_pv([0., 0., -0.05], 3);
        assert!((v[2] - 0.01 * 0.5).abs() < 1e-15, "{v:?}");
        // pv = -0.005: err = -0.95, blend 0.01 + 0.09 * 0.5
        let mut s = mk();
        let v = s.final_integrate_pv([0., 0., -0.005], 3);
        let kp = 0.01 + 0.09 * (0.95 - 0.9) / 0.1;
        assert!((v[2] - kp * 0.95).abs() < 1e-12, "{v:?}");
    }

    /// **Methodology.** The force servo translates: once a velocity is set,
    /// `initial_integrate` advances `xcm` by `dt v` and returns
    /// `xcm - xcm_orig`.
    #[test]
    fn force_servo_translates() {
        let mut s = servo();
        let v = s.final_integrate([0.0; 3]);
        let ServoMove::Translate { d_x } = s.initial_integrate() else {
            panic!("force servo must translate")
        };
        assert!((d_x[2] - 1e-5 * v[2]).abs() < 1e-15);
        assert_eq!(s.xcm()[2], 1.0 + d_x[2]);
        assert_eq!(s.vcm(), v);
    }
}
