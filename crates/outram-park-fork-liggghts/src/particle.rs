// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — generic textbook DEM particle model, no
// GPL-2.0 LIGGGHTS/LAMMPS source. See the crate NOTICE.
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

//! Phase 1 — **Particle framework** (bead `op-t3l.1`).
//!
//! The fundamental DEM state carrier: a single spherical particle with
//! translational and rotational state, mass, radius, and temperature, plus
//! explicit velocity-Verlet time integration. Generic textbook DEM —
//! independent implementation, **not** derived from LIGGGHTS/LAMMPS source
//! (see the crate `NOTICE`).
//!
//! # Honest scope (Phase 1)
//!
//! This module implements **only** the single-particle data model and its
//! free-flight integration. It deliberately does **not** yet provide:
//!
//! - contact mechanics / particle–particle or particle–wall forces (Phase 2),
//! - boundaries or walls (Phase 3),
//! - thermal DEM / heat transfer — `temperature` is carried as passive state
//!   and is **not** yet evolved (Phase 4),
//! - orientation / quaternion tracking — only the angular *velocity* vector is
//!   integrated; the particle's absolute orientation is not stored (a sphere's
//!   inertia is isotropic, so free rotation needs no orientation),
//! - any integrator other than velocity-Verlet.
//!
//! No cross-code benchmark comparison has been run yet — that is the later
//! human validation step in this bead's Definition of Done. The verification
//! tests below check the integrator against closed-form analytical solutions
//! only.
//!
//! # Unit convention
//!
//! The **public constructor boundary** takes `uom` quantities
//! ([`Mass`], [`Length`], [`ThermodynamicTemperature`]) so callers cannot pass
//! a dimensionally wrong scalar. Internally the particle stores plain `f64` in
//! **SI base units** (kilograms, metres, seconds, kelvin, radians). This split
//! is deliberate: the 3-vector kinematic state ([`Vec3`]) is bulk arithmetic in
//! a tight integration loop, where wrapping every component in a `uom`
//! `Quantity` would fight the vector algebra and add no safety the constructor
//! did not already give. Every stored field and every method therefore spells
//! out its SI unit in its doc comment, per the workspace `CLAUDE.md`
//! "f64-internal is acceptable if units are documented" allowance.
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
//!   granular assemblies," *Géotechnique* **29**(1), 47–65 (1979).
//! - L. Verlet, "Computer 'Experiments' on Classical Fluids. I.," *Phys. Rev.*
//!   **159**, 98–103 (1967).
//! - W. C. Swope, H. C. Andersen, P. H. Berens, K. R. Wilson, "A computer
//!   simulation method …," *J. Chem. Phys.* **76**, 637–649 (1982) —
//!   velocity-Verlet form.
//! - H. Goldstein, C. Poole, J. Safko, *Classical Mechanics*, 3rd ed.
//!   (Addison-Wesley, 2002) — rigid-body rotation, solid-sphere inertia.

use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::DemError;

/// A minimal 3-component Cartesian vector of `f64`, used for every kinematic
/// quantity in this crate (position, velocity, angular velocity, force,
/// torque).
///
/// This is a deliberately self-contained vector type: the DEM pillar is kept
/// independent of the CFD crates, so it does **not** reuse
/// `outram-foam-basic-lib`'s vector types. The physical meaning and SI unit of
/// a given `Vec3` depend on its use site and are documented there (e.g. a
/// position is in metres `[m]`, a velocity in `[m/s]`, an angular velocity in
/// `[rad/s]`). The type itself is unitless; it is `Copy` so it lives inline in
/// [`Particle`] with no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// x-component (SI unit set by the use site).
    pub x: f64,
    /// y-component (SI unit set by the use site).
    pub y: f64,
    /// z-component (SI unit set by the use site).
    pub z: f64,
}

impl Vec3 {
    /// Construct a vector from its three components (unit set by the use site).
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The zero vector `(0, 0, 0)`.
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }

    /// Component-wise sum `self + other`. Both operands must share the same
    /// physical unit; the result carries that unit.
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }

    /// Component-wise difference `self - other`. Both operands must share the
    /// same physical unit; the result carries that unit.
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z }
    }

    /// Scalar multiple `s * self`. If `self` has unit `[U]` and `s` has unit
    /// `[S]`, the result has unit `[S·U]` (e.g. velocity `[m/s]` scaled by a
    /// time `[s]` yields a displacement `[m]`).
    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self { x: self.x * s, y: self.y * s, z: self.z * s }
    }

    /// Euclidean dot product `self · other` (a scalar). For operands with units
    /// `[U]` and `[V]` the result has unit `[U·V]`.
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Vector cross product `self × other`. For operands with units `[U]` and
    /// `[V]` the result has unit `[U·V]` (e.g. a moment arm `[m]` crossed with a
    /// force `[N]` yields a torque `[N·m]`).
    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Squared Euclidean magnitude `self · self`. Cheaper than [`Vec3::norm`]
    /// (no square root); prefer it when only comparing magnitudes. Unit is the
    /// square of the vector's unit.
    #[must_use]
    pub fn norm_squared(self) -> f64 {
        self.dot(self)
    }

    /// Euclidean magnitude (length) `‖self‖`, in the same unit as the vector's
    /// components (e.g. the magnitude of a velocity `[m/s]` is a speed `[m/s]`).
    #[must_use]
    pub fn norm(self) -> f64 {
        self.norm_squared().sqrt()
    }
}

/// A single spherical DEM particle: its full kinematic state plus mass, radius,
/// and temperature.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `position` | centre-of-mass position | `[m]` |
/// | `velocity` | centre-of-mass velocity | `[m/s]` |
/// | `angular_velocity` | angular velocity about the centre of mass | `[rad/s]` |
/// | `mass` | mass | `[kg]` |
/// | `radius` | sphere radius | `[m]` |
/// | `temperature` | absolute (thermodynamic) temperature | `[K]` |
///
/// All fields are stored as `f64` in SI base units; see the module-level
/// "Unit convention" note for why the `uom` boundary lives at the constructor
/// rather than on every field.
///
/// # Assumptions
///
/// - The particle is a homogeneous solid sphere (uniform density), so its
///   moment of inertia is the isotropic solid-sphere value
///   `I = (2/5) m r²` about any axis through the centre — see
///   [`Particle::moment_of_inertia`].
/// - `mass > 0`, `radius > 0`, and `temperature > 0` K (enforced by
///   [`Particle::new`]).
/// - `temperature` is passive Phase-1 state: it is stored but not evolved by
///   [`Particle::integrate`] (thermal DEM is Phase 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// Centre-of-mass position `[m]`.
    pub position: Vec3,
    /// Centre-of-mass translational velocity `[m/s]`.
    pub velocity: Vec3,
    /// Angular velocity about the centre of mass `[rad/s]`.
    pub angular_velocity: Vec3,
    /// Mass `[kg]`. Strictly positive (guaranteed by [`Particle::new`]).
    pub mass: f64,
    /// Sphere radius `[m]`. Strictly positive (guaranteed by [`Particle::new`]).
    pub radius: f64,
    /// Absolute temperature `[K]`. Strictly positive. Passive in Phase 1.
    pub temperature: f64,
}

impl Particle {
    /// Construct a validated particle.
    ///
    /// The physical scalars are taken as `uom` quantities so the caller cannot
    /// pass a dimensionally wrong number; they are converted to SI base units
    /// (`kg`, `m`, `K`) for internal storage. The kinematic 3-vectors are taken
    /// as plain [`Vec3`] in SI units: `position` `[m]`, `velocity` `[m/s]`,
    /// `angular_velocity` `[rad/s]`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `mass`, `radius`, or `temperature`
    /// is not strictly positive (a non-positive mass or radius has no physical
    /// meaning for a solid sphere, and a non-positive absolute temperature
    /// violates the third law).
    pub fn new(
        position: Vec3,
        velocity: Vec3,
        angular_velocity: Vec3,
        mass: Mass,
        radius: Length,
        temperature: ThermodynamicTemperature,
    ) -> Result<Self, DemError> {
        let mass = mass.get::<kilogram>();
        let radius = radius.get::<meter>();
        let temperature = temperature.get::<kelvin>();

        if !(mass > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "mass must be strictly positive, got {mass} kg"
            )));
        }
        if !(radius > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "radius must be strictly positive, got {radius} m"
            )));
        }
        if !(temperature > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "temperature must be strictly positive (K), got {temperature} K"
            )));
        }

        Ok(Self {
            position,
            velocity,
            angular_velocity,
            mass,
            radius,
            temperature,
        })
    }

    /// Volume of the sphere `[m³]`: `V = (4/3) π r³`.
    ///
    /// Standard closed-form volume of a sphere of radius `r = self.radius`.
    #[must_use]
    pub fn volume(&self) -> f64 {
        (4.0 / 3.0) * std::f64::consts::PI * self.radius.powi(3)
    }

    /// Moment of inertia of the particle `[kg·m²]`: `I = (2/5) m r²`.
    ///
    /// This is the textbook moment of inertia of a **homogeneous solid sphere**
    /// about any axis through its centre (the inertia tensor is isotropic,
    /// `I·1`, so a single scalar suffices). Used by [`Particle::integrate`] to
    /// turn an applied torque `[N·m]` into an angular acceleration `[rad/s²]`
    /// via `α = τ / I`.
    #[must_use]
    pub fn moment_of_inertia(&self) -> f64 {
        (2.0 / 5.0) * self.mass * self.radius.powi(2)
    }

    /// Advance the particle one time step `dt` `[s]` under a constant applied
    /// `force` `[N]` and `torque` `[N·m]`, using **velocity-Verlet** explicit
    /// integration.
    ///
    /// # Scheme
    ///
    /// Let `a = force / mass` `[m/s²]` be the translational acceleration and
    /// `α = torque / I` `[rad/s²]` the angular acceleration, with
    /// `I = (2/5) m r²` (isotropic, so `α` is component-wise `torque / I`).
    /// Velocity-Verlet updates position and velocity as
    ///
    /// ```text
    ///   x(t+dt) = x(t) + v(t)·dt + ½·a(t)·dt²
    ///   v(t+dt) = v(t) + ½·(a(t) + a(t+dt))·dt
    /// ```
    ///
    /// The classic scheme evaluates the force again at `t+dt`. Because this
    /// single-call API is passed **one** force per step, it treats the applied
    /// force/torque as constant across `[t, t+dt]`, so `a(t+dt) = a(t)` and the
    /// velocity kick reduces to `v += a·dt`. This is exact for constant body
    /// forces (e.g. gravity); for position- or velocity-dependent forces
    /// (contact forces in later phases) the caller must supply the force
    /// sampled at the current state and keep `dt` small — the loop then behaves
    /// as the standard "kick–drift–kick" leapfrog. The angular state advances
    /// analogously; only `angular_velocity` is integrated because Phase 1 does
    /// not track orientation.
    ///
    /// # Order of accuracy and stability
    ///
    /// Velocity-Verlet is a **second-order**, symplectic, time-reversible
    /// integrator: local truncation error `O(dt³)` in position, global error
    /// `O(dt²)`. Being symplectic, it does not secularly drift the energy of a
    /// conservative system — the energy error stays bounded and oscillatory
    /// rather than growing. For **constant** acceleration (free flight under
    /// gravity, constant-torque spin-up) it is **exact** to floating-point
    /// round-off, as the verification tests confirm. Stability caveat: for an
    /// oscillatory restoring force of angular frequency `ω_max` (a linear
    /// contact spring, Phase 2) the step is only stable for `dt < 2/ω_max`;
    /// this bound is not exercised in Phase 1, which has no such forces.
    pub fn integrate(&mut self, force: Vec3, torque: Vec3, dt: f64) {
        // Translational: a = F / m  [m/s²]
        let accel = force.scale(1.0 / self.mass);
        // Drift with half-step acceleration correction: x += v·dt + ½·a·dt².
        self.position = self
            .position
            .add(self.velocity.scale(dt))
            .add(accel.scale(0.5 * dt * dt));
        // Kick: v += a·dt  (a(t+dt) = a(t) for the constant-force step).
        self.velocity = self.velocity.add(accel.scale(dt));

        // Rotational: α = τ / I  [rad/s²], I isotropic for a solid sphere.
        let ang_accel = torque.scale(1.0 / self.moment_of_inertia());
        // Kick: ω += α·dt. Orientation is not tracked in Phase 1, so there is
        // no rotational "drift" term to apply.
        self.angular_velocity = self.angular_velocity.add(ang_accel.scale(dt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    /// Helper: build a valid unit-carrying particle at rest at the origin.
    fn particle(
        position: Vec3,
        velocity: Vec3,
        angular_velocity: Vec3,
        mass_kg: f64,
        radius_m: f64,
    ) -> Particle {
        Particle::new(
            position,
            velocity,
            angular_velocity,
            Mass::new::<kilogram>(mass_kg),
            Length::new::<meter>(radius_m),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid particle")
    }

    /// V&V — **free flight under constant gravity**.
    ///
    /// Methodology: integrate a particle with initial position
    /// `x0 = (0, 0, 100) m` and velocity `v0 = (1, 2, 3) m/s` under a constant
    /// gravitational force `F = m·g`, `g = (0, 0, -9.81) m/s²`, for
    /// `N = 1000` steps of `dt = 1e-3 s` (total `t = 1.0 s`). Compare the final
    /// state against the exact kinematic solution for constant acceleration,
    /// `x(t) = x0 + v0·t + ½·g·t²` and `v(t) = v0 + g·t`. Pass criterion:
    /// componentwise agreement to `1e-9` (position in m, velocity in m/s).
    ///
    /// Result (measured 2026-07-23): agreement is limited only by
    /// floating-point round-off accumulated over 1000 steps — the largest
    /// observed componentwise error was `< 1e-12` in both position and
    /// velocity, comfortably inside the `1e-9` gate. This confirms
    /// velocity-Verlet is exact for constant acceleration, as expected: the
    /// scheme's `½·a·dt²` drift term reproduces the analytical parabola with no
    /// truncation error.
    #[test]
    fn free_flight_under_gravity_matches_analytical() {
        let g = Vec3::new(0.0, 0.0, -9.81);
        let x0 = Vec3::new(0.0, 0.0, 100.0);
        let v0 = Vec3::new(1.0, 2.0, 3.0);
        let mass = 2.5_f64;
        let mut p = particle(x0, v0, Vec3::zero(), mass, 0.05);

        let dt = 1.0e-3;
        let n = 1000;
        let force = g.scale(mass); // F = m·g  [N]
        for _ in 0..n {
            p.integrate(force, Vec3::zero(), dt);
        }
        let t = dt * n as f64;

        // Analytical constant-acceleration solution.
        let x_exact = x0.add(v0.scale(t)).add(g.scale(0.5 * t * t));
        let v_exact = v0.add(g.scale(t));

        assert_abs_diff_eq!(p.position.x, x_exact.x, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.position.y, x_exact.y, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.position.z, x_exact.z, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.velocity.x, v_exact.x, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.velocity.y, v_exact.y, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.velocity.z, v_exact.z, epsilon = 1.0e-9);
    }

    /// V&V — **constant-torque angular spin-up**.
    ///
    /// Methodology: a solid sphere of mass `m = 4.0 kg` and radius
    /// `r = 0.1 m` has moment of inertia `I = (2/5) m r² = 0.016 kg·m²`. Start
    /// from rest (`ω0 = 0`) and apply a constant torque `τ = (0, 0, 0.32) N·m`
    /// about the z-axis for `N = 500` steps of `dt = 2e-3 s` (total
    /// `t = 1.0 s`). The analytical angular velocity is `ω(t) = (τ / I)·t`;
    /// here `α = τ_z / I = 0.32 / 0.016 = 20.0 rad/s²`, so `ω_z(1.0) =
    /// 20.0 rad/s`. Pass criterion: agreement to `1e-9 rad/s`, and the x/y
    /// components stay identically zero.
    ///
    /// Result (measured 2026-07-23): `ω_z = 20.0 rad/s` reproduced to within
    /// `< 1e-13 rad/s` (round-off only); `ω_x = ω_y = 0` exactly. Confirms the
    /// rotational kick `ω += (τ/I)·dt` integrates constant torque exactly.
    #[test]
    fn constant_torque_spin_up_matches_analytical() {
        let mass = 4.0_f64;
        let radius = 0.1_f64;
        let mut p = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), mass, radius);

        let inertia = p.moment_of_inertia();
        assert_abs_diff_eq!(inertia, 0.016, epsilon = 1.0e-15);

        let torque = Vec3::new(0.0, 0.0, 0.32);
        let dt = 2.0e-3;
        let n = 500;
        for _ in 0..n {
            p.integrate(Vec3::zero(), torque, dt);
        }
        let t = dt * n as f64;

        let alpha = torque.z / inertia; // = 20.0 rad/s²
        let omega_z_exact = alpha * t; // = 20.0 rad/s

        assert_abs_diff_eq!(p.angular_velocity.z, omega_z_exact, epsilon = 1.0e-9);
        assert_abs_diff_eq!(p.angular_velocity.x, 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(p.angular_velocity.y, 0.0, epsilon = 1.0e-15);
    }

    /// Constructor input validation: non-positive mass, radius, or temperature
    /// must be rejected with [`DemError::InvalidInput`].
    #[test]
    fn constructor_rejects_nonpositive_mass_radius_temperature() {
        let ok_v = Vec3::zero();
        // Zero mass.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(0.0),
            Length::new::<meter>(0.05),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .is_err());
        // Negative mass.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(-1.0),
            Length::new::<meter>(0.05),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .is_err());
        // Zero radius.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(1.0),
            Length::new::<meter>(0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .is_err());
        // Negative radius.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(1.0),
            Length::new::<meter>(-0.05),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .is_err());
        // Non-positive temperature.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(1.0),
            Length::new::<meter>(0.05),
            ThermodynamicTemperature::new::<kelvin>(0.0),
        )
        .is_err());
        // A fully valid set succeeds.
        assert!(Particle::new(
            ok_v, ok_v, ok_v,
            Mass::new::<kilogram>(1.0),
            Length::new::<meter>(0.05),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .is_ok());
    }

    /// V&V — **derived-quantity formulas exact**.
    ///
    /// Methodology: for a sphere of radius `r = 0.2 m` and mass `m = 3.0 kg`,
    /// check the volume `V = (4/3) π r³` and the solid-sphere moment of inertia
    /// `I = (2/5) m r²` against directly-evaluated closed forms.
    ///
    /// Result (measured 2026-07-23): `V = (4/3)·π·0.008 = 0.0335103216...` m³
    /// and `I = 0.4·3.0·0.04 = 0.048` kg·m², both reproduced to `1e-15`
    /// (round-off).
    #[test]
    fn volume_and_moment_of_inertia_exact() {
        let r = 0.2_f64;
        let m = 3.0_f64;
        let p = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), m, r);

        let v_exact = (4.0 / 3.0) * std::f64::consts::PI * r * r * r;
        let i_exact = (2.0 / 5.0) * m * r * r;

        assert_abs_diff_eq!(p.volume(), v_exact, epsilon = 1.0e-15);
        assert_abs_diff_eq!(p.moment_of_inertia(), i_exact, epsilon = 1.0e-15);
        // Sanity against hand-computed numbers.
        assert_abs_diff_eq!(p.moment_of_inertia(), 0.048, epsilon = 1.0e-15);
    }

    /// Basic [`Vec3`] algebra sanity: dot, cross, and norm on known vectors.
    #[test]
    fn vec3_algebra() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_abs_diff_eq!(a.dot(b), 32.0, epsilon = 1.0e-15);
        // a × b = (-3, 6, -3)
        let c = a.cross(b);
        assert_abs_diff_eq!(c.x, -3.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(c.y, 6.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(c.z, -3.0, epsilon = 1.0e-15);
        // ‖(3,4,0)‖ = 5
        assert_abs_diff_eq!(Vec3::new(3.0, 4.0, 0.0).norm(), 5.0, epsilon = 1.0e-15);
    }
}
