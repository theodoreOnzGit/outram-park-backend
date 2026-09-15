// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream file    : src/fix_nve_sphere.cpp, src/fix_nve_sphere.h
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz; Sandia Corporation
//                       (LAMMPS, fix_nve_sphere.cpp original).
// Do not strip this block during refactors (workspace CLAUDE.md).
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

//! # Velocity-Verlet integration for spheres (`nve/sphere`)
//!
//! Faithful translation of LIGGGHTS' `FixNVESphere`, which is a genuine
//! **kick–drift–kick** velocity-Verlet propagator split across two half steps
//! around the force evaluation:
//!
//! ```text
//!   initial_integrate:   v += (dt/2)·F(t)/m
//!                        ω += (dt/2)·τ(t)/I
//!                        x += dt·v
//!   ---- forces and torques are recomputed at x(t+dt) ----
//!   final_integrate:     v += (dt/2)·F(t+dt)/m
//!                        ω += (dt/2)·τ(t+dt)/I
//! ```
//!
//! with `I = (2/5) m r²` for a solid sphere (LIGGGHTS' `INERTIA = 0.4`, applied
//! as `dtirotate = (dt/2)/INERTIA / (r² m)`).
//!
//! ## Why this module exists: [`Particle::integrate`] is not velocity-Verlet
//!
//! [`Particle::integrate`](crate::particle::Particle::integrate) applies the
//! *same* acceleration `a(t)` to both the position and the velocity update:
//!
//! ```text
//!   x(t+dt) = x + v·dt + ½·a(t)·dt²
//!   v(t+dt) = v + a(t)·dt
//! ```
//!
//! That is **not** velocity-Verlet, and — despite what that method's own doc
//! comment claimed before 2026-09-15 — it is **not symplectic**. For a linear
//! restoring force `a = −ω²x` (which is exactly what a DEM contact spring is)
//! its one-step Jacobian is
//!
//! ```text
//!   M = [ 1 − ω²dt²/2    dt ]        det M = 1 + ω²dt²/2  >  1
//!       [ −ω²dt           1 ]
//! ```
//!
//! so phase-space volume — and with it the energy — **grows geometrically**,
//! by a factor `(1 + ω²dt²/2)` per step, no matter how well resolved the step
//! is. Measured on a unit oscillator at `dt = 0.1/ω` (≈ 63 steps per period, a
//! *comfortably* resolved DEM contact), `E/E₀ = 2.13 × 10⁴³` after 20 000
//! steps, against `0.99969` for the scheme in this module. See
//! `docs/verification-and-validation.md` § "Integrator".
//!
//! The old scheme is exact for a **constant** force (free flight under gravity,
//! constant-torque spin-up), which is all its original unit tests exercised —
//! which is why the defect survived. It is kept, with a corrected doc comment,
//! because it is still the right thing for a single constant-force kick; every
//! *contact* integration should use [`VelocityVerlet`].
//!
//! ## Honest scope
//!
//! Translation of the integrator only. Orientation (quaternions) is not
//! tracked, matching the base crate: only `angular_velocity` is advanced, which
//! is sufficient for spheres with isotropic inertia.

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// LIGGGHTS' `INERTIA` constant for a solid sphere: `I = INERTIA · m · r²`.
///
/// Dimensionless `[-]`. Value `2/5`, the moment of inertia of a uniform solid
/// sphere about a diameter.
pub const INERTIA: f64 = 0.4;

/// Kick–drift–kick **velocity-Verlet** propagator for a sphere ensemble
/// (`fix nve/sphere`).
///
/// Holds only the time step, so it is `Copy` and carries no ensemble state; the
/// particles live in the caller's `Vec<Particle>` and are advanced in place.
///
/// # Usage
///
/// One full step is *always* three calls, in this order:
///
/// ```
/// # use outram_park_fork_liggghts::integrator::VelocityVerlet;
/// # use outram_park_fork_liggghts::particle::{Particle, Vec3};
/// # let mut particles = vec![Particle::new(
/// #     Vec3::zero(), Vec3::new(1.0, 0.0, 0.0), Vec3::zero(), 1.0, 0.1, 300.0).unwrap()];
/// # let compute_forces = |_: &[Particle]| (vec![Vec3::zero()], vec![Vec3::zero()]);
/// let vv = VelocityVerlet::new(1.0e-6).unwrap();
///
/// let (mut f, mut t) = compute_forces(&particles);   // F(t), τ(t)
/// vv.initial_integrate(&mut particles, &f, &t);      // half-kick + drift
/// let (f_new, t_new) = compute_forces(&particles);   // F(t+dt), τ(t+dt)
/// vv.final_integrate(&mut particles, &f_new, &t_new); // half-kick
/// # f = f_new; t = t_new; let _ = (f, t);
/// ```
///
/// Skipping [`VelocityVerlet::final_integrate`], or reusing the *old* forces in
/// it, degrades the scheme back to the non-symplectic form described in the
/// module docs — the energy will grow.
///
/// # Parameters and units
///
/// | Field | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `dt` | `Δt` | integration time step | `[s]` | `> 0` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocityVerlet {
    dt: f64,
}

impl VelocityVerlet {
    /// Build a propagator with time step `dt` `[s]`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `dt` is not strictly positive or is not
    /// finite — an explicit integrator cannot advance on such a step.
    pub fn new(dt: f64) -> Result<Self, DemError> {
        if !(dt > 0.0) || !dt.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "time step dt must be finite and strictly positive, got {dt} s"
            )));
        }
        Ok(Self { dt })
    }

    /// The integration time step `[s]`.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// First half of the step: velocity/spin half-kick with the **current**
    /// force and torque, then the full position drift.
    ///
    /// `forces` `[N]` and `torques` `[N·m]` must be indexed like `particles`
    /// and evaluated at the *current* state. Entries beyond `particles.len()`
    /// are ignored; missing entries are treated as zero, so a shorter slice
    /// simply leaves those particles unforced.
    pub fn initial_integrate(
        &self,
        particles: &mut [Particle],
        forces: &[Vec3],
        torques: &[Vec3],
    ) {
        let half = 0.5 * self.dt;
        for (i, p) in particles.iter_mut().enumerate() {
            let f = forces.get(i).copied().unwrap_or_else(Vec3::zero);
            let t = torques.get(i).copied().unwrap_or_else(Vec3::zero);
            // v += (dt/2)·F/m
            p.velocity = p.velocity.add(f.scale(half / p.mass));
            // ω += (dt/2)·τ/I,  I = (2/5) m r²
            let inv_inertia = 1.0 / (INERTIA * p.mass * p.radius * p.radius);
            p.angular_velocity = p.angular_velocity.add(t.scale(half * inv_inertia));
            // x += dt·v   (drift with the half-kicked velocity)
            p.position = p.position.add(p.velocity.scale(self.dt));
        }
    }

    /// Second half of the step: velocity/spin half-kick with the force and
    /// torque **recomputed at the drifted positions**.
    ///
    /// Passing the same slices as [`VelocityVerlet::initial_integrate`] is a
    /// silent correctness bug (see the type-level docs) — recompute first.
    pub fn final_integrate(&self, particles: &mut [Particle], forces: &[Vec3], torques: &[Vec3]) {
        let half = 0.5 * self.dt;
        for (i, p) in particles.iter_mut().enumerate() {
            let f = forces.get(i).copied().unwrap_or_else(Vec3::zero);
            let t = torques.get(i).copied().unwrap_or_else(Vec3::zero);
            p.velocity = p.velocity.add(f.scale(half / p.mass));
            let inv_inertia = 1.0 / (INERTIA * p.mass * p.radius * p.radius);
            p.angular_velocity = p.angular_velocity.add(t.scale(half * inv_inertia));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    fn p(pos: Vec3, vel: Vec3, m: f64, r: f64) -> Particle {
        Particle::new(
            pos,
            vel,
            Vec3::zero(),
            Mass::new::<kilogram>(m),
            Length::new::<meter>(r),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    /// **Methodology.** Reject a non-positive / non-finite step.
    /// **Result (2026-09-15).** All four rejected; `1e-6` accepted.
    #[test]
    fn rejects_invalid_timestep() {
        assert!(VelocityVerlet::new(0.0).is_err());
        assert!(VelocityVerlet::new(-1e-6).is_err());
        assert!(VelocityVerlet::new(f64::NAN).is_err());
        assert!(VelocityVerlet::new(f64::INFINITY).is_err());
        assert!(VelocityVerlet::new(1e-6).is_ok());
    }

    /// **Methodology.** Free flight under constant gravity, `g = −9.81 ẑ`, from
    /// rest at the origin, 1000 steps of `dt = 1e-4 s` (`t = 0.1 s`). Compare
    /// against the closed-form parabola `z = −½gt²`, `v_z = −gt`. Velocity-
    /// Verlet is exact for constant acceleration, so the pass criterion is
    /// float round-off.
    ///
    /// **Result (2026-09-15).** `z = −0.0490500000000000 m` against the exact
    /// `−0.04905 m`, `v_z = −0.981 m/s` exact; both to `< 1e-14` relative.
    #[test]
    fn free_flight_matches_analytical_parabola() {
        let vv = VelocityVerlet::new(1.0e-4).unwrap();
        let m = 2.0;
        let mut ps = vec![p(Vec3::zero(), Vec3::zero(), m, 0.1)];
        let g = -9.81;
        let f = vec![Vec3::new(0.0, 0.0, m * g)];
        let t = vec![Vec3::zero()];
        for _ in 0..1000 {
            vv.initial_integrate(&mut ps, &f, &t);
            vv.final_integrate(&mut ps, &f, &t);
        }
        let time = 0.1;
        assert_abs_diff_eq!(ps[0].position.z, 0.5 * g * time * time, epsilon = 1e-14);
        assert_abs_diff_eq!(ps[0].velocity.z, g * time, epsilon = 1e-12);
    }

    /// **Methodology.** Constant torque `τ = 0.5 ẑ N·m` on a sphere of
    /// `m = 2 kg`, `r = 0.1 m` (`I = 0.4·2·0.01 = 8e-3 kg·m²`) from rest, 1000
    /// steps of `dt = 1e-4 s`. Exact solution `ω(t) = (τ/I)·t`.
    ///
    /// **Result (2026-09-15).** `ω_z = 6.25 rad/s` against exact `6.25 rad/s`,
    /// to `< 1e-12`.
    #[test]
    fn constant_torque_spin_up_matches_analytical() {
        let vv = VelocityVerlet::new(1.0e-4).unwrap();
        let mut ps = vec![p(Vec3::zero(), Vec3::zero(), 2.0, 0.1)];
        let tau = 0.5;
        let inertia = INERTIA * 2.0 * 0.01;
        let f = vec![Vec3::zero()];
        let t = vec![Vec3::new(0.0, 0.0, tau)];
        for _ in 0..1000 {
            vv.initial_integrate(&mut ps, &f, &t);
            vv.final_integrate(&mut ps, &f, &t);
        }
        assert_abs_diff_eq!(ps[0].angular_velocity.z, tau / inertia * 0.1, epsilon = 1e-12);
    }

    /// **Methodology — the defect this module fixes.** Integrate a 1-D harmonic
    /// oscillator `F = −k x` (`k = m ω²`, `ω = 1 rad/s`) for 20 000 steps at
    /// `dt = 0.1 s` (≈ 63 steps per period — a well-resolved DEM contact), from
    /// `x = 1 m`, `v = 0`. Compare the final energy ratio `E/E₀` for this
    /// velocity-Verlet against [`Particle::integrate`]'s single-shot scheme.
    /// Pass criterion: velocity-Verlet keeps `|E/E₀ − 1| < 1e-3` and stays
    /// *bounded*; the single-shot scheme is asserted to blow up, so the
    /// regression fires if anyone "fixes" it silently.
    ///
    /// **Result (2026-09-15).** velocity-Verlet `E/E₀ = 0.9996893`;
    /// single-shot `E/E₀ = 2.128e+43`. Ratio between the two schemes: `2.1e43`.
    #[test]
    fn velocity_verlet_conserves_oscillator_energy_where_single_shot_explodes() {
        let (m, k, dt, steps) = (1.0_f64, 1.0_f64, 0.1_f64, 20_000);
        let energy = |x: f64, v: f64| 0.5 * (m * v * v + k * x * x);

        // --- this module: kick-drift-kick ---
        let vv = VelocityVerlet::new(dt).unwrap();
        let mut ps = vec![p(Vec3::new(1.0, 0.0, 0.0), Vec3::zero(), m, 0.1)];
        let e0 = energy(1.0, 0.0);
        let mut e_max: f64 = e0;
        for _ in 0..steps {
            let f = vec![Vec3::new(-k * ps[0].position.x, 0.0, 0.0)];
            let t = vec![Vec3::zero()];
            vv.initial_integrate(&mut ps, &f, &t);
            let f2 = vec![Vec3::new(-k * ps[0].position.x, 0.0, 0.0)];
            vv.final_integrate(&mut ps, &f2, &t);
            e_max = e_max.max(energy(ps[0].position.x, ps[0].velocity.x));
        }
        let e_vv = energy(ps[0].position.x, ps[0].velocity.x) / e0;
        assert!(
            (e_vv - 1.0).abs() < 1.0e-3,
            "velocity-Verlet must conserve oscillator energy, got E/E0 = {e_vv:e}"
        );
        assert!(
            e_max / e0 < 1.0 + 1.0e-9,
            "velocity-Verlet energy must stay bounded, got max E/E0 = {:e}",
            e_max / e0
        );

        // --- the single-shot scheme, for contrast ---
        let mut q = p(Vec3::new(1.0, 0.0, 0.0), Vec3::zero(), m, 0.1);
        for _ in 0..steps {
            let f = Vec3::new(-k * q.position.x, 0.0, 0.0);
            q.integrate(f, Vec3::zero(), dt);
        }
        let e_single = energy(q.position.x, q.velocity.x) / e0;
        assert!(
            e_single > 1.0e10,
            "single-shot scheme is expected to be non-symplectic and blow up; \
             got E/E0 = {e_single:e}. If this now passes, Particle::integrate \
             changed — update the module docs and this test."
        );
    }

    /// **Methodology.** Verify the one-step Jacobian determinant claim in the
    /// module docs numerically, by finite-differencing the map `(x, v) ↦
    /// (x', v')` for the oscillator at `dt = 0.1`, `ω = 1`. Velocity-Verlet is
    /// symplectic so `det = 1`; the single-shot scheme has `det = 1 + ω²dt²/2`.
    ///
    /// **Result (2026-09-15).** velocity-Verlet `det = 1.0000000000` (to
    /// `1e-10`); single-shot `det = 1.0050000000` against the predicted
    /// `1 + 0.5·0.01 = 1.005` (to `1e-10`).
    #[test]
    fn jacobian_determinants_match_the_documented_algebra() {
        let (m, k, dt) = (1.0_f64, 1.0_f64, 0.1_f64);
        let h = 1.0e-6;

        let step_vv = |x0: f64, v0: f64| {
            let vv = VelocityVerlet::new(dt).unwrap();
            let mut ps = vec![p(Vec3::new(x0, 0.0, 0.0), Vec3::new(v0, 0.0, 0.0), m, 0.1)];
            let f = vec![Vec3::new(-k * x0, 0.0, 0.0)];
            let t = vec![Vec3::zero()];
            vv.initial_integrate(&mut ps, &f, &t);
            let f2 = vec![Vec3::new(-k * ps[0].position.x, 0.0, 0.0)];
            vv.final_integrate(&mut ps, &f2, &t);
            (ps[0].position.x, ps[0].velocity.x)
        };
        let step_single = |x0: f64, v0: f64| {
            let mut q = p(Vec3::new(x0, 0.0, 0.0), Vec3::new(v0, 0.0, 0.0), m, 0.1);
            q.integrate(Vec3::new(-k * x0, 0.0, 0.0), Vec3::zero(), dt);
            (q.position.x, q.velocity.x)
        };

        for (name, f, expected) in [
            ("velocity-Verlet", &step_vv as &dyn Fn(f64, f64) -> (f64, f64), 1.0),
            ("single-shot", &step_single, 1.0 + 0.5 * dt * dt),
        ] {
            let (xa, va) = f(h, 0.0);
            let (xb, vb) = f(0.0, h);
            // Columns of the Jacobian (the map is linear, so FD is exact).
            let det = (xa / h) * (vb / h) - (xb / h) * (va / h);
            assert_abs_diff_eq!(det, expected, epsilon = 1e-9);
            let _ = name;
        }
    }
}
