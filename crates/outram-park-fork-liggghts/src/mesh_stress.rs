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
//! point, which moves with the mesh). ~~**Not translated:** `wear finnie`,~~
//! **`wear finnie` translated 2026-11-12 (gh:#1491)**: [`MeshStress::with_wear_finnie`],
//! per-facet `wear`. **Not translated:** external (gravity) contributions to
//! the totals, `store_wear_increment`. The reference point follows the mesh's
//! translation exactly and its rotation about the world origin as upstream's
//! `TrackingMesh::rotate` does (`move(-origin)`, rotate, `move(origin)`);
//! a rotation about a point other than the origin is not cross-coded.

use crate::particle::Vec3;
use crate::variable::{Expr, VarContext};
use crate::DemError;

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
    /// `wear finnie` state, when enabled.
    wear: Option<FinnieWear>,
}

/// `wear finnie` of `MeshModuleStress`: per-facet accumulated `wear` and this
/// step's `wear_step`.
#[derive(Debug, Clone, PartialEq)]
struct FinnieWear {
    /// `k_finnie[atom_type_mesh-1][type-1]`, indexed by the particle's 0-based
    /// atom type.
    k_finnie: Vec<f64>,
    wear: Vec<f64>,
    wear_step: Vec<f64>,
}

/// One contact as `MeshModuleStress::add_particle_contribution` sees it, for
/// the wear part: `frc` the contact force on the particle (`f_pw`), `x` and
/// `v` the particle's centre and velocity, `delta` the already negated
/// `-sidata.delta`, `v_wall` the mesh velocity at the contact, `norm` the
/// facet's unit normal, `atom_type` the particle's 0-based type, `dt` the
/// timestep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WearContact {
    /// Contact force on the particle `[N]`.
    pub frc: [f64; 3],
    /// Particle centre `[m]`.
    pub x: [f64; 3],
    /// Particle velocity `[m/s]`.
    pub v: [f64; 3],
    /// `-sidata.delta` `[m]`.
    pub delta: [f64; 3],
    /// Mesh velocity at the contact `[m/s]`.
    pub v_wall: [f64; 3],
    /// Facet index.
    pub tri: usize,
    /// Facet unit normal.
    pub norm: [f64; 3],
    /// 0-based particle atom type.
    pub atom_type: usize,
    /// Timestep `[s]`.
    pub dt: f64,
}

/// `#define EPSILON 0.0001` of `mesh_module_stress.cpp`.
const WEAR_EPSILON: f64 = 0.0001;

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
            wear: None,
        }
    }

    /// `wear finnie` (`fix mesh/surface/stress ... wear finnie` with
    /// `fix ... property/global k_finnie peratomtypepair`): `k_finnie[t]` is the
    /// coefficient of this mesh's wall type against particle type `t`
    /// (0-based). `wear` starts at zero (`regWear`).
    #[must_use]
    pub fn with_wear_finnie(mut self, k_finnie: Vec<f64>) -> Self {
        let n = self.area.len();
        self.wear = Some(FinnieWear {
            k_finnie,
            wear: vec![0.0; n],
            wear_step: vec![0.0; n],
        });
        self
    }

    /// Start a set-up force evaluation: `wear_step` is zero (upstream's is,
    /// after `zeroizeWear` or the last `calc_total_force`), so a set-up that
    /// the port repeats does not count twice.
    pub fn begin_setup_wear(&mut self) {
        if let Some(w) = self.wear.as_mut() {
            w.wear_step.iter_mut().for_each(|x| *x = 0.0);
        }
    }

    /// The `trackWear()` part of `add_particle_contribution`, in upstream's
    /// operation order: `c = (x + delta) - x`, `v_rel = v - v_wall`; nothing
    /// if `c . v_rel < 0` or `|v_rel| < 1e-7`; `sin = |v_rel . n| / |v_rel|`,
    /// `cos = |v_rel x n| / |v_rel|` (each capped at 1);
    /// `E = 0.33333 cos^2` if `cos < 1e-4` or `3 sin > cos`, else
    /// `E = 2 sin cos - 3 sin^2`; `E *= 2 k |v_rel| |frc|`;
    /// `wear_step[tri] += E dt / area`. No-op without `wear finnie`.
    pub fn add_wear_contribution(&mut self, c: &WearContact) {
        let Some(w) = self.wear.as_mut() else {
            return;
        };
        let contact = add(c.x, c.delta);
        let cc = [contact[0] - c.x[0], contact[1] - c.x[1], contact[2] - c.x[2]];
        let v_rel = [c.v[0] - c.v_wall[0], c.v[1] - c.v_wall[1], c.v[2] - c.v_wall[2]];
        if dot3(cc, v_rel) < 0.0 {
            return;
        }
        let v_rel_mag = mag3(v_rel);
        if 0.0000001 > v_rel_mag {
            return;
        }
        let mut sin_gamma = dot3(v_rel, c.norm).abs() / v_rel_mag;
        let mut cos_gamma = mag3(cross3(v_rel, c.norm)).abs() / v_rel_mag;
        if cos_gamma > 1.0 {
            cos_gamma = 1.0;
        }
        if sin_gamma > 1.0 {
            sin_gamma = 1.0;
        }
        let mut e = if cos_gamma < WEAR_EPSILON || 3.0 * sin_gamma > cos_gamma {
            0.33333 * cos_gamma * cos_gamma
        } else {
            let sin_2gamma = 2.0 * sin_gamma * cos_gamma;
            sin_2gamma - 3.0 * sin_gamma * sin_gamma
        };
        e *= 2.0 * w.k_finnie[c.atom_type] * v_rel_mag * mag3(c.frc);
        let part = e * c.dt / self.area[c.tri];
        w.wear_step[c.tri] += part;
    }

    /// Accumulated Finnie wear of facet `i` (the mesh element property
    /// `wear`), or `None` without `wear finnie`.
    #[must_use]
    pub fn wear(&self, i: usize) -> Option<f64> {
        self.wear.as_ref().map(|w| w.wear[i])
    }

    /// Replace the facet areas (`areaElem`), as `recalcLocalSurfProperties`
    /// does when a run's set-up refreshes the mesh (gh:#1510).
    pub fn refresh_area(&mut self, area: Vec<f64>) {
        self.area = area;
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
        // add wear from this step to total wear
        if let Some(w) = self.wear.as_mut() {
            for (acc, step) in w.wear.iter_mut().zip(w.wear_step.iter_mut()) {
                *acc += *step;
                *step = 0.0;
            }
        }
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
/// ~~**Not ported:** a variable set point (`target_val v_name`),~~
/// **A variable set point (`target_val v_name`, an equal-style variable) is
/// ported since 2026-11-12 (gh:#1491):** [`MeshServo::with_target_variable`].
/// ~~**Not ported:** `fix_modify integrate/vel_max/target_val/ctrlParam/ratio`,
/// which only acts between two `run` commands, and the port has no emulation
/// of a second `run`'s `init()`/`setup()` (gh:#1491).~~
/// **PORTED 2026-11-13 (gh:#1510):** [`MeshServo::modify_param`] translates
/// `MeshModuleStressServo::modify_param`, and a new `run` is emulated by
/// [`crate::granular_system::GranularSystem::begin_run`], which calls
/// [`MeshServo::init_run`] (`init()`) and [`MeshServo::setup_pre_force`].
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
    /// `target_val v_name`: the equal-style set point, evaluated every
    /// `final_integrate` (`sp_style_ == EQUAL`); `None` is a constant.
    sp_var: Option<Expr>,
    /// `update->ntimestep` at the last `final_integrate` (counted from 0 at
    /// construction, one per step).
    step: i64,
    /// `int_flag_`: the servo moves the mesh (`fix_modify ... integrate
    /// start|stop`, default start).
    int_flag: bool,
    /// `ratio_` of `mode auto` (0 unless set), read by `init()`.
    ratio: f64,
    /// `neighbor->skin` `[m]`, for the `vel_max < skin/(2 dt)` check.
    skin: f64,
}

/// One keyword of `fix_modify <mesh> servo/<keyword> ...`
/// (`MeshModuleStressServo::modify_param`), for [`MeshServo::modify_param`].
/// Upstream only acts on these between two `run` commands: the new values
/// take effect through the next `init()` ([`MeshServo::init_run`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ServoModify {
    /// `integrate start` (`true`) or `integrate stop` (`false`): whether the
    /// servo moves the mesh. Stopped, the mesh keeps its node velocities.
    Integrate(bool),
    /// `vel_max <v>` `[m/s]`.
    VelMax(f64),
    /// `target_val <value>`: a constant set point (`sp_mag = -value`); resets
    /// the integral term.
    TargetVal(f64),
    /// `target_val v_name`: an equal-style variable set point; resets the
    /// integral term.
    TargetVariable(Expr),
    /// `ctrlParam kp ki kd`; resets the integral term.
    CtrlParam([f64; 3]),
    /// `ratio <r>` of `mode auto` (upstream warns, and ignores it, in PID mode).
    Ratio(f64),
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
            sp_var: None,
            step: 0,
            int_flag: true,
            ratio: 0.0,
            skin,
        }
    }

    /// `fix_modify <mesh> servo/<keyword>` (`MeshModuleStressServo::
    /// modify_param`): `vel_max`, `target_val` and `ctrlParam` replace the
    /// fix-line values (`target_val` and `ctrlParam` also zero the integral
    /// term, `resetIntegrator`), `ratio` replaces `ratio_`, `integrate`
    /// starts or stops the servo. As upstream, the new values enter the
    /// controller through the next `init()`: call
    /// [`crate::granular_system::GranularSystem::begin_run`] before stepping.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] where upstream aborts: `vel_max <= 0`, a
    /// constant `target_val` of zero.
    pub fn modify_param(&mut self, m: ServoModify) -> Result<(), DemError> {
        match m {
            ServoModify::Integrate(on) => self.int_flag = on,
            ServoModify::VelMax(v) => {
                if v <= 0.0 {
                    return Err(DemError::InvalidInput("vel_max > 0 required".into()));
                }
                self.vel_max = v;
            }
            ServoModify::TargetVal(v) => {
                // the resultant force/torque acts in the opposite direction
                let sp_mag = -v;
                if sp_mag == 0.0 {
                    return Err(DemError::InvalidInput(
                        "'target_val' (desired force/torque) has to be != 0.0".into(),
                    ));
                }
                self.sp_mag = sp_mag;
                self.sp_mag_inv = 1. / sp_mag.abs();
                self.sp_var = None;
                self.sum_err = 0.0;
            }
            ServoModify::TargetVariable(expr) => {
                self.sp_var = Some(expr);
                self.sum_err = 0.0;
            }
            ServoModify::CtrlParam([kp, ki, kd]) => {
                self.kp = kp;
                self.ki = ki;
                self.kd = kd;
                self.sum_err = 0.0;
            }
            ServoModify::Ratio(r) => self.ratio = r,
        }
        Ok(())
    }

    /// `init()` of a new `run` (`MeshModuleStressServo::init`, with
    /// `error_checks`): `vel_min = ratio * r_min / dt` from the smallest
    /// particle radius now in the system (`r_min`, `BIG = 1e6` when there is
    /// none), `rPaMax` of the torque servo from the current `nodes`
    /// (`getMaxRad`), and the upstream aborts re-checked. `p_ref = xcm` is set
    /// by the caller on the stress module. `dtv_` is the system's fixed
    /// timestep and does not change.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] where upstream's `init()` aborts:
    /// `vel_max == 0`; `mode auto` with `ratio == 0`; in PID mode a negative
    /// gain or all gains zero; every node on the torque axis;
    /// `vel_max >= skin / (2 dt)`.
    pub fn init_run(&mut self, r_min: f64, nodes: &[[[f64; 3]; 3]]) -> Result<(), DemError> {
        let bad = |m: &str| Err(DemError::InvalidInput(m.into()));
        if self.vel_max == 0.0 {
            return bad("please define 'vel_max' for the mesh");
        }
        if self.is_auto() {
            if self.ratio == 0.0 {
                return bad("please define 'ratio' for the mesh, since you use the auto mode");
            }
        } else {
            if self.kp < 0.0 || self.ki < 0.0 || self.kd < 0.0 {
                return bad("kp, ki, and kd >= 0 required.");
            }
            if self.kp == 0.0 && self.ki == 0.0 && self.kd == 0.0 {
                return bad("kp, ki, and kd are zero. Please set a valid configuration");
            }
        }
        let r_pa_max = self.max_rad(nodes);
        if self.is_auto() {
            self.vel_min = Some(self.ratio * r_min / self.dtv);
        }
        if self.ctrl == ServoCtrl::Torque {
            if r_pa_max == 0.0 {
                return bad("All mesh nodes are located at the rotation axis.");
            }
            self.r_pa_max = r_pa_max;
        }
        if self.vel_max >= self.skin / (2. * self.dtv) {
            return bad("vel_max < skin/(2.*dt) required");
        }
        Ok(())
    }

    /// `setup_pre_force` of a run: the mesh's present position becomes the
    /// origin of the servo's total move (`xcm_orig = xcm`, `totalPhi = 0`);
    /// the caller makes the present nodes the original nodes
    /// (`FixMesh::setup_pre_force` -> `storeNodePosOrig`).
    pub fn setup_pre_force(&mut self) {
        self.xcm_orig = self.xcm;
        self.total_phi = 0.0;
    }

    /// `int_flag_`: whether the servo moves the mesh this run.
    #[must_use]
    pub fn is_integrating(&self) -> bool {
        self.int_flag
    }

    /// `getMaxRad`: the largest distance of a node from the axis through
    /// `xcm`.
    fn max_rad(&self, nodes: &[[[f64; 3]; 3]]) -> f64 {
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
        r_pa_max
    }

    /// `target_val v_name`: the set point is the equal-style variable `expr`,
    /// evaluated at the start of every `final_integrate` with
    /// `sp_mag = -value` (upstream aborts on a zero value). The constant
    /// `target_val` given to [`MeshServo::new`] is then never read (upstream's
    /// `sp_mag_` is overwritten before its first use). `step` and `time` are
    /// counted from the servo's construction; `ramp()` is not faithful (the
    /// run length is not known here).
    #[must_use]
    pub fn with_target_variable(mut self, expr: Expr) -> Self {
        self.sp_var = Some(expr);
        self
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
        let r_pa_max = self.max_rad(nodes);
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
        self.ratio = ratio;
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
    ///
    /// # Panics
    ///
    /// As upstream aborts: a variable set point that evaluates to zero or
    /// fails to evaluate.
    pub fn final_integrate_pv(&mut self, pv: [f64; 3], num_contacts: usize) -> [f64; 3] {
        self.step += 1;
        // only if the wall should move (`integrate stop`: nothing changes)
        if !self.int_flag {
            return match self.ctrl {
                ServoCtrl::Force => self.vcm,
                ServoCtrl::Torque => self.omegacm,
            };
        }
        if let Some(expr) = &self.sp_var {
            // variable force, wrap with clear/add
            let ctx = VarContext {
                step: self.step,
                begin_step: 0,
                end_step: i64::MAX,
                dt: self.dtv,
            };
            let v = expr.eval(&ctx).expect("servo target_val variable");
            self.sp_mag = -v;
            assert!(
                self.sp_mag != 0.0,
                "Set point (desired force/torque/shear) has to be != 0.0"
            );
            self.sp_mag_inv = 1. / self.sp_mag.abs();
        }
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
