// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506: `quaternion`, `septernion`,
// `transformPoints` (src/OpenFOAM/primitives, fields/Fields/transformField),
// `solidBodyMotionFunctions` (src/meshTools), `solidBodyMotionSolver`
// (src/dynamicMesh/motionSolvers/displacement/solidBody), `face::centre`,
// `face::sweptVol`, `triangle::sweptVol` (src/OpenFOAM/meshes) and
// `fvGeometryScheme::setMeshPhi` (src/finiteVolume/fvMesh/fvGeometryScheme).

//! Dynamic mesh, first slice (gh:#1380): **solid-body mesh motion** and the
//! **mesh motion flux** `meshPhi`, ported from OpenFOAM.com v2506.
//!
//! > Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for operating,
//! > controlling or licensing a nuclear facility.
//!
//! What upstream does, and what is ported:
//!
//! | Upstream | Here |
//! |---|---|
//! | `solidBodyMotionFunctions::{linearMotion, rotatingMotion, axisRotationMotion, oscillatingLinearMotion}` | [`SolidBodyMotion`] |
//! | `quaternion`, `septernion` | [`Quaternion`], [`Septernion`] |
//! | `transformPoints(septernion, pointField)` | [`transform_points`] |
//! | `solidBodyMotionSolver::curPoints` | [`SolidBodyMeshMotion::cur_points`] |
//! | `dynamicMotionSolverFvMesh::update` (`fvMesh::movePoints`) | [`SolidBodyMeshMotion::advance`] |
//! | `face::sweptVol`, `fvGeometryScheme::setMeshPhi` | [`face_swept_vol`], [`mesh_phi`] |
//!
//! Every function repeats the upstream expression tree operation for operation
//! (dot product `x*x + y*y + z*z`, cross product component order, `vs/s`
//! normalisation, left-to-right sums), because the V&V target is bit-identity
//! with ESI v2506 (`verification_and_validation/dynamic_mesh_gh1380/`). Vectors
//! are plain `[f64; 3]` like the sibling `esi_*` parity layer
//! ([`crate::snappy_hex_mesh::esi_snap`]); no unit-carrying wrapper is applied
//! here because a wrapper would hide the operation order being matched.
//!
//! The geometry of the moved mesh is [`crate::check_mesh::esi_geometry`], already
//! bit-identical to `primitiveMesh::updateGeom` on static meshes (gh:#1364).
//!
//! **Not ported (child issues):** `displacementLaplacian` and the other
//! solver-based motion solvers, mesh-motion `pointPatchField` boundary
//! conditions, non-constant `Function1` coefficients, topology changers,
//! `sixDoFRigidBodyMotion`, `multiSolidBodyMotionSolver`.

use crate::check_mesh::{esi_geometry, EsiGeometry};
use crate::snappy_hex_mesh::esi_snap::{self, SnapMesh, V};
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamValue};
use petir::mathf::RealMath;

const SMALL: f64 = 1e-15;
const VSMALL: f64 = 1e-300;
const ROOT_V_SMALL: f64 = 1e-150;

/// Why a `dynamicMeshDict` could not be turned into a [`SolidBodyMotion`].
#[derive(Debug, Clone, PartialEq)]
pub enum DynamicMeshError {
    /// A required keyword is missing or has the wrong shape.
    Entry(String),
    /// A `solidBodyMotionFunction` that is not ported.
    UnsupportedFunction(String),
    /// A coefficient that upstream reads as a `Function1` but is not a constant.
    UnsupportedFunction1(String),
}

impl std::fmt::Display for DynamicMeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicMeshError::Entry(k) => write!(f, "dynamicMeshDict: missing or malformed entry '{k}'"),
            DynamicMeshError::UnsupportedFunction(n) => {
                write!(f, "solidBodyMotionFunction '{n}' is not ported (gh:#1380)")
            }
            DynamicMeshError::UnsupportedFunction1(k) => {
                write!(f, "entry '{k}': only a constant Function1 is ported (gh:#1380)")
            }
        }
    }
}

impl std::error::Error for DynamicMeshError {}

/// OpenFOAM `quaternion`: scalar part `w`, vector part `v`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    /// Scalar part.
    pub w: f64,
    /// Vector part.
    pub v: V,
}

impl Quaternion {
    /// `quaternion::I`: `(1, (0 0 0))`.
    pub fn identity() -> Self {
        Quaternion { w: 1.0, v: [0.0; 3] }
    }

    /// `quaternion(d, theta)`: rotation by `theta` radians about direction `d`.
    pub fn from_axis_angle(d: V, theta: f64) -> Self {
        Quaternion {
            w: (0.5 * theta).r_cos(),
            v: esi_snap::scale((0.5 * theta).r_sin(), normalised_vector(d)),
        }
    }

    /// `conjugate(q)`.
    pub fn conjugate(self) -> Self {
        Quaternion { w: self.w, v: [-self.v[0], -self.v[1], -self.v[2]] }
    }

    /// `operator*(quaternion, quaternion)`.
    pub fn mul(self, q: Quaternion) -> Self {
        Quaternion {
            w: self.w * q.w - esi_snap::dot(self.v, q.v),
            v: esi_snap::add(
                esi_snap::add(esi_snap::scale(self.w, q.v), esi_snap::scale(q.w, self.v)),
                esi_snap::cross(self.v, q.v),
            ),
        }
    }

    /// `magSqr(q) = magSqr(w) + magSqr(v)`.
    fn mag_sqr(self) -> f64 {
        self.w * self.w + esi_snap::mag_sqr(self.v)
    }

    /// `normalised(q)`: zero below `ROOTVSMALL`, else `q/|q|`.
    pub fn normalised(self) -> Self {
        let s = self.mag_sqr().sqrt();
        if s < ROOT_V_SMALL {
            Quaternion { w: 0.0, v: [0.0; 3] }
        } else {
            Quaternion { w: self.w / s, v: esi_snap::div(self.v, s) }
        }
    }

    /// `quaternion::mulq0v(u) = quaternion(-(v & u), w*u + (v ^ u))`.
    fn mulq0v(self, u: V) -> Self {
        Quaternion {
            w: -esi_snap::dot(self.v, u),
            v: esi_snap::add(esi_snap::scale(self.w, u), esi_snap::cross(self.v, u)),
        }
    }

    /// `quaternion::transform(vector)`: `(mulq0v(u)*conjugate(q)).v`.
    pub fn transform(self, u: V) -> V {
        self.mulq0v(u).mul(self.conjugate()).v
    }

    /// `quaternion::invTransform(vector)`: `(conjugate(q).mulq0v(u)*q).v`.
    pub fn inv_transform(self, u: V) -> V {
        self.conjugate().mulq0v(u).mul(self).v
    }

    /// `quaternion::transform(quaternion)`: `normalised(q*r)`.
    pub fn transform_quaternion(self, q: Quaternion) -> Self {
        self.mul(q).normalised()
    }

    /// `quaternion::R()`: the rotation tensor, row-major.
    pub fn rotation_tensor(self) -> [f64; 9] {
        let (w, x, y, z) = (self.w, self.v[0], self.v[1], self.v[2]);
        let w2 = w * w;
        let x2 = x * x;
        let y2 = y * y;
        let z2 = z * z;
        let txy = 2.0 * x * y;
        let twz = 2.0 * w * z;
        let txz = 2.0 * x * z;
        let twy = 2.0 * w * y;
        let tyz = 2.0 * y * z;
        let twx = 2.0 * w * x;
        [
            w2 + x2 - y2 - z2,
            txy - twz,
            txz + twy,
            txy + twz,
            w2 - x2 + y2 - z2,
            tyz - twx,
            txz - twy,
            tyz + twx,
            w2 - x2 - y2 + z2,
        ]
    }
}

/// `normalised(vector)`: zero below `ROOTVSMALL`, else `v/|v|`.
fn normalised_vector(v: V) -> V {
    let s = esi_snap::mag(v);
    if s < ROOT_V_SMALL {
        [0.0; 3]
    } else {
        esi_snap::div(v, s)
    }
}

/// OpenFOAM `septernion`: translation `t` and rotation `r`; it maps a point as
/// `r.transform(p - t)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Septernion {
    /// Translation.
    pub t: V,
    /// Rotation.
    pub r: Quaternion,
}

impl Septernion {
    /// `septernion(vector)`: pure translation.
    pub fn from_translation(t: V) -> Self {
        Septernion { t, r: Quaternion::identity() }
    }

    /// `operator*(septernion, quaternion)`.
    pub fn mul_quaternion(self, r: Quaternion) -> Self {
        Septernion { t: r.inv_transform(self.t), r: self.r.mul(r) }
    }

    /// `operator*(septernion, septernion)`.
    pub fn mul(self, tr2: Septernion) -> Self {
        Septernion {
            t: esi_snap::add(tr2.r.inv_transform(self.t), tr2.t),
            r: self.r.transform_quaternion(tr2.r),
        }
    }
}

/// `transformPoints(septernion, pointField)`: subtract the translation when it
/// is larger than `VSMALL`, then rotate when `|R - I| > SMALL`.
pub fn transform_points(tr: &Septernion, fld: &[V]) -> Vec<V> {
    let mut result: Vec<V> = if esi_snap::mag(tr.t) > VSMALL {
        fld.iter().map(|&p| esi_snap::sub(p, tr.t)).collect()
    } else {
        fld.to_vec()
    };
    let rot = tr.r.rotation_tensor();
    let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let mut mag_sqr = 0.0;
    for k in 0..9 {
        let d = rot[k] - identity[k];
        mag_sqr = if k == 0 { d * d } else { mag_sqr + d * d };
    }
    if mag_sqr.sqrt() > SMALL {
        for p in result.iter_mut() {
            // `Tensor & Vector`
            *p = [
                rot[0] * p[0] + rot[1] * p[1] + rot[2] * p[2],
                rot[3] * p[0] + rot[4] * p[1] + rot[5] * p[2],
                rot[6] * p[0] + rot[7] * p[1] + rot[8] * p[2],
            ];
        }
    }
    result
}

/// `degToRad(deg) = deg*M_PI/180.0`.
fn deg_to_rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

/// The `solidBodyMotionFunction`s ported so far (constant coefficients only).
#[derive(Debug, Clone, PartialEq)]
pub enum SolidBodyMotion {
    /// `linearMotion`: constant `velocity`.
    Linear {
        /// Velocity of the body.
        velocity: V,
    },
    /// `rotatingMotion`: rotation about `axis` through `origin` at the constant
    /// angular speed `omega` (rad/s; upstream `Function1`, constant here).
    Rotating {
        /// Centre of rotation.
        origin: V,
        /// Rotation axis (normalised on use).
        axis: V,
        /// Angular speed in rad/s.
        omega: f64,
    },
    /// `axisRotationMotion`: rotation vector `t*degToRad(radialVelocity)`.
    AxisRotation {
        /// Centre of rotation.
        origin: V,
        /// Angular velocity vector in degrees per second.
        radial_velocity: V,
    },
    /// `oscillatingLinearMotion`: `amplitude*sin(omega*(t + phaseShift)) + verticalShift`.
    OscillatingLinear {
        /// Amplitude vector.
        amplitude: V,
        /// Angular frequency in rad/s.
        omega: f64,
        /// Phase shift in seconds (`0` when absent upstream).
        phase_shift: f64,
        /// Constant offset (`Zero` when absent upstream).
        vertical_shift: V,
    },
}

impl SolidBodyMotion {
    /// `solidBodyMotionFunction::transformation()` at time `t`.
    pub fn transformation(&self, t: f64) -> Septernion {
        match self {
            SolidBodyMotion::Linear { velocity } => {
                let displacement = esi_snap::scale(t, *velocity);
                // `velocity_*t` is Vector*scalar: component-wise, commutative
                Septernion::from_translation(neg(displacement)).mul_quaternion(Quaternion::identity())
            }
            SolidBodyMotion::Rotating { origin, axis, omega } => {
                // `Function1Types::Constant::integrate(0, t) = (t - 0)*value`
                let angle = (t - 0.0) * omega;
                let r = Quaternion::from_axis_angle(*axis, angle);
                rotation_about(*origin, r)
            }
            SolidBodyMotion::AxisRotation { origin, radial_velocity } => {
                let omega = [
                    t * deg_to_rad(radial_velocity[0]),
                    t * deg_to_rad(radial_velocity[1]),
                    t * deg_to_rad(radial_velocity[2]),
                ];
                let mag_omega = esi_snap::mag(omega);
                let r = Quaternion::from_axis_angle(esi_snap::div(omega, mag_omega), mag_omega);
                rotation_about(*origin, r)
            }
            SolidBodyMotion::OscillatingLinear { amplitude, omega, phase_shift, vertical_shift } => {
                let s = (omega * (t + phase_shift)).r_sin();
                let displacement = esi_snap::add(esi_snap::scale(s, *amplitude), *vertical_shift);
                Septernion::from_translation(neg(displacement)).mul_quaternion(Quaternion::identity())
            }
        }
    }

    /// Read the motion from a `dynamicMeshDict` whose `motionSolver` is
    /// `solidBody`: `solidBodyMotionFunction <name>;` and `<name>Coeffs { .. }`
    /// (`solidBodyMotionFunction` falls back to the dictionary itself when the
    /// `...Coeffs` sub-dictionary is absent, as `optionalSubDict` does).
    pub fn from_dynamic_mesh_dict(dict: &FoamDict) -> Result<Self, DynamicMeshError> {
        let outer = dict.get_dict("solidBodyCoeffs").unwrap_or(dict);
        let name = outer
            .get_word("solidBodyMotionFunction")
            .ok_or_else(|| DynamicMeshError::Entry("solidBodyMotionFunction".into()))?;
        let coeffs = outer.get_dict(&format!("{name}Coeffs")).unwrap_or(outer);
        match name {
            "linearMotion" => Ok(SolidBodyMotion::Linear { velocity: entry_vector(coeffs, "velocity")? }),
            "rotatingMotion" => Ok(SolidBodyMotion::Rotating {
                origin: entry_vector(coeffs, "origin")?,
                axis: entry_vector(coeffs, "axis")?,
                omega: entry_constant(coeffs, "omega")?,
            }),
            "axisRotationMotion" => Ok(SolidBodyMotion::AxisRotation {
                origin: entry_vector(coeffs, "origin")?,
                radial_velocity: entry_vector(coeffs, "radialVelocity")?,
            }),
            "oscillatingLinearMotion" => Ok(SolidBodyMotion::OscillatingLinear {
                amplitude: entry_vector(coeffs, "amplitude")?,
                omega: entry_constant(coeffs, "omega")?,
                phase_shift: if coeffs.get("phaseShift").is_some() { entry_constant(coeffs, "phaseShift")? } else { 0.0 },
                vertical_shift: if coeffs.get("verticalShift").is_some() {
                    entry_vector(coeffs, "verticalShift")?
                } else {
                    [0.0; 3]
                },
            }),
            other => Err(DynamicMeshError::UnsupportedFunction(other.to_string())),
        }
    }
}

fn neg(v: V) -> V {
    [-v[0], -v[1], -v[2]]
}

/// `septernion(-origin)*R*septernion(origin)`.
fn rotation_about(origin: V, r: Quaternion) -> Septernion {
    Septernion::from_translation(neg(origin))
        .mul_quaternion(r)
        .mul(Septernion::from_translation(origin))
}

fn entry_vector(dict: &FoamDict, key: &str) -> Result<V, DynamicMeshError> {
    let bad = || DynamicMeshError::Entry(key.to_string());
    let items: &[FoamValue] = match dict.get(key).ok_or_else(bad)? {
        FoamEntry::List(v) => v,
        _ => return Err(bad()),
    };
    match items {
        [FoamValue::Scalar(x), FoamValue::Scalar(y), FoamValue::Scalar(z)] => Ok([*x, *y, *z]),
        _ => Err(bad()),
    }
}

fn entry_constant(dict: &FoamDict, key: &str) -> Result<f64, DynamicMeshError> {
    match dict.get(key) {
        Some(FoamEntry::Scalar(x)) => Ok(*x),
        Some(_) => Err(DynamicMeshError::UnsupportedFunction1(key.to_string())),
        None => Err(DynamicMeshError::Entry(key.to_string())),
    }
}

/// `triangle::sweptVol(t)` for the old triangle `old` swept onto `new`.
pub fn triangle_swept_vol(old: [V; 3], new: [V; 3]) -> f64 {
    let [a, b, c] = old;
    let [ta, tb, tc] = new;
    let s = esi_snap::sub;
    let cr = esi_snap::cross;
    let d = esi_snap::dot;
    let t1 = d(s(ta, a), cr(s(b, a), s(c, a)));
    let t2 = d(s(tb, b), cr(s(c, b), s(ta, b)));
    let t3 = d(s(c, tc), cr(s(tb, tc), s(ta, tc)));
    let t4 = d(s(ta, a), cr(s(b, a), s(c, a)));
    let t5 = d(s(b, tb), cr(s(ta, tb), s(tc, tb)));
    let t6 = d(s(c, tc), cr(s(b, tc), s(ta, tc)));
    (1.0 / 12.0) * (t1 + t2 + t3 + t4 + t5 + t6)
}

/// `face::centre(points)`: triangle direct; otherwise area-weighted average of
/// the triangle centres of the centre-point decomposition.
pub fn face_centre(points: &[V], face: &[usize]) -> V {
    let n = face.len();
    if n == 3 {
        let sum = esi_snap::add(esi_snap::add(points[face[0]], points[face[1]]), points[face[2]]);
        return esi_snap::scale(1.0 / 3.0, sum);
    }
    let mut centre_point = [0.0; 3];
    for &p in face {
        centre_point = esi_snap::add(centre_point, points[p]);
    }
    centre_point = esi_snap::div(centre_point, n as f64);
    let mut sum_a = 0.0;
    let mut sum_ac = [0.0; 3];
    for pi in 0..n {
        let next = points[face[(pi + 1) % n]];
        let ttc = esi_snap::add(esi_snap::add(points[face[pi]], next), centre_point);
        let ta = esi_snap::mag(esi_snap::cross(
            esi_snap::sub(points[face[pi]], centre_point),
            esi_snap::sub(next, centre_point),
        ));
        sum_a += ta;
        sum_ac = esi_snap::add(sum_ac, esi_snap::scale(ta, ttc));
    }
    if sum_a > VSMALL {
        esi_snap::div(sum_ac, 3.0 * sum_a)
    } else {
        centre_point
    }
}

/// `face::sweptVol(oldPoints, newPoints)`: central decomposition, the centre
/// point first in every triangle (as the v2506 code has it).
pub fn face_swept_vol(face: &[usize], old_points: &[V], new_points: &[V]) -> f64 {
    let n = face.len();
    let c_old = face_centre(old_points, face);
    let c_new = face_centre(new_points, face);
    let mut sv = 0.0;
    for pi in 0..n - 1 {
        sv += triangle_swept_vol(
            [c_old, old_points[face[pi]], old_points[face[pi + 1]]],
            [c_new, new_points[face[pi]], new_points[face[pi + 1]]],
        );
    }
    sv += triangle_swept_vol(
        [c_old, old_points[face[n - 1]], old_points[face[0]]],
        [c_new, new_points[face[n - 1]], new_points[face[0]]],
    );
    sv
}

/// `fvGeometryScheme::setMeshPhi`: the swept volume of every face (internal
/// first, then the boundary faces in patch order) divided by `deltaT`
/// (multiplied by `rdt = 1.0/deltaT`).
pub fn mesh_phi(faces: &[Vec<usize>], old_points: &[V], new_points: &[V], delta_t: f64) -> Vec<f64> {
    let rdt = 1.0 / delta_t;
    faces.iter().map(|f| face_swept_vol(f, old_points, new_points) * rdt).collect()
}

/// One step of mesh motion: the new points, the mesh motion flux and the
/// geometry `fvMesh::movePoints` leaves behind.
#[derive(Debug, Clone)]
pub struct MovingMeshStep {
    /// Time after the step (accumulated as `Time::operator++` does).
    pub time: f64,
    /// Moved points.
    pub points: Vec<V>,
    /// `meshPhi` per face, internal faces first.
    pub mesh_phi: Vec<f64>,
    /// Face centres and areas, cell centres and volumes of the moved mesh.
    pub geometry: EsiGeometry,
}

/// `dynamicMotionSolverFvMesh` with a `solidBodyMotionSolver` (serial, all cells
/// moving): `points0` is the mesh as read, the points at time `t` are
/// `transformPoints(transformation(t), points0)`.
#[derive(Debug, Clone)]
pub struct SolidBodyMeshMotion {
    mesh: SnapMesh,
    points0: Vec<V>,
    motion: SolidBodyMotion,
    time: f64,
}

impl SolidBodyMeshMotion {
    /// Start at `start_time` from `mesh` (its points are `points0`).
    pub fn new(mesh: SnapMesh, motion: SolidBodyMotion, start_time: f64) -> Self {
        let points0 = mesh.points.clone();
        SolidBodyMeshMotion { mesh, points0, motion, time: start_time }
    }

    /// `solidBodyMotionSolver::curPoints` at time `t`.
    pub fn cur_points(&self, t: f64) -> Vec<V> {
        transform_points(&self.motion.transformation(t), &self.points0)
    }

    /// The mesh as it is now.
    pub fn mesh(&self) -> &SnapMesh {
        &self.mesh
    }

    /// `runTime++` then `mesh.update()`: advance the time by `delta_t`, move the
    /// points, set `meshPhi` from the old and new points.
    pub fn advance(&mut self, delta_t: f64) -> MovingMeshStep {
        self.time += delta_t;
        let new_points = self.cur_points(self.time);
        let phi = mesh_phi(&self.mesh.faces, &self.mesh.points, &new_points, delta_t);
        self.mesh.points = new_points;
        let geometry = esi_geometry(&self.mesh);
        MovingMeshStep { time: self.time, points: self.mesh.points.clone(), mesh_phi: phi, geometry }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;

    fn unit_cube() -> SnapMesh {
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let faces = vec![
            vec![0, 4, 7, 3],
            vec![1, 2, 6, 5],
            vec![0, 1, 5, 4],
            vec![3, 7, 6, 2],
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
        ];
        SnapMesh { points, faces, owner: vec![0; 6], neighbour: vec![] }
    }

    fn parse(text: &str) -> FoamDict {
        FoamFile::parse(text).expect("parse").dict
    }

    #[test]
    fn rotation_by_quarter_turn_maps_x_to_y() {
        let m = SolidBodyMotion::Rotating { origin: [0.0; 3], axis: [0.0, 0.0, 1.0], omega: std::f64::consts::FRAC_PI_2 };
        let p = transform_points(&m.transformation(1.0), &[[1.0, 0.0, 0.0]]);
        assert!((p[0][0]).abs() < 1e-15 && (p[0][1] - 1.0).abs() < 1e-15);
    }

    #[test]
    fn linear_motion_translates_by_velocity_times_time() {
        let m = SolidBodyMotion::Linear { velocity: [1.0, 2.0, 3.0] };
        let p = transform_points(&m.transformation(0.5), &[[0.0, 0.0, 0.0]]);
        assert_eq!(p[0], [0.5, 1.0, 1.5]);
    }

    #[test]
    fn swept_volume_of_a_translated_unit_face_is_displacement_dot_area() {
        let mesh = unit_cube();
        let mut moved = mesh.points.clone();
        for p in moved.iter_mut() {
            p[0] += 0.25;
        }
        // face 0 (x = 0, area vector -x): moving +x by 0.25 sweeps -0.25
        let sv = face_swept_vol(&mesh.faces[0], &mesh.points, &moved);
        assert!((sv + 0.25).abs() < 1e-15, "{sv}");
        let sv1 = face_swept_vol(&mesh.faces[1], &mesh.points, &moved);
        assert!((sv1 - 0.25).abs() < 1e-15, "{sv1}");
    }

    #[test]
    fn advance_conserves_volume_for_rigid_motion() {
        let mesh = unit_cube();
        let motion = SolidBodyMotion::AxisRotation { origin: [0.1, 0.2, 0.3], radial_velocity: [30.0, -45.0, 60.0] };
        let mut sim = SolidBodyMeshMotion::new(mesh, motion, 0.0);
        let step = sim.advance(0.01);
        assert!((step.time - 0.01).abs() < 1e-18);
        let total: f64 = step.mesh_phi.iter().sum();
        // closed cell, every face owned by cell 0: the fluxes sum to dV/dt = 0
        assert!(total.abs() < 1e-12, "{total}");
        assert!((step.geometry.cell_volumes[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn dictionary_is_read_for_all_four_functions() {
        let lin = parse("solidBodyMotionFunction linearMotion;\nlinearMotionCoeffs { velocity (0.3 -0.2 0.1); }");
        assert_eq!(
            SolidBodyMotion::from_dynamic_mesh_dict(&lin).unwrap(),
            SolidBodyMotion::Linear { velocity: [0.3, -0.2, 0.1] }
        );
        let rot = parse("solidBodyMotionFunction rotatingMotion;\nrotatingMotionCoeffs { origin (0.1 0.2 0.3); axis (0.3 0.5 1); omega 2.5; }");
        assert_eq!(
            SolidBodyMotion::from_dynamic_mesh_dict(&rot).unwrap(),
            SolidBodyMotion::Rotating { origin: [0.1, 0.2, 0.3], axis: [0.3, 0.5, 1.0], omega: 2.5 }
        );
        let ax = parse("solidBodyMotionFunction axisRotationMotion;\naxisRotationMotionCoeffs { origin (0.1 0.2 0.3); radialVelocity (30 -45 60); }");
        assert!(matches!(
            SolidBodyMotion::from_dynamic_mesh_dict(&ax).unwrap(),
            SolidBodyMotion::AxisRotation { .. }
        ));
        let osc = parse("solidBodyMotionFunction oscillatingLinearMotion;\noscillatingLinearMotionCoeffs { amplitude (0.2 0.1 -0.3); omega 7.3; }");
        assert_eq!(
            SolidBodyMotion::from_dynamic_mesh_dict(&osc).unwrap(),
            SolidBodyMotion::OscillatingLinear {
                amplitude: [0.2, 0.1, -0.3],
                omega: 7.3,
                phase_shift: 0.0,
                vertical_shift: [0.0; 3]
            }
        );
    }

    #[test]
    fn unsupported_function_and_function1_are_reported() {
        let d = parse("solidBodyMotionFunction SDA;");
        assert_eq!(
            SolidBodyMotion::from_dynamic_mesh_dict(&d),
            Err(DynamicMeshError::UnsupportedFunction("SDA".into()))
        );
        let d = parse("solidBodyMotionFunction rotatingMotion;\nrotatingMotionCoeffs { origin (0 0 0); axis (0 0 1); omega table ((0 1) (1 2)); }");
        assert!(matches!(
            SolidBodyMotion::from_dynamic_mesh_dict(&d),
            Err(DynamicMeshError::UnsupportedFunction1(_))
        ));
        let d = parse("foo bar;");
        assert!(SolidBodyMotion::from_dynamic_mesh_dict(&d).unwrap_err().to_string().contains("solidBodyMotionFunction"));
    }
}
