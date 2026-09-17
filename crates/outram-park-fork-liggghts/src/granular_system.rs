// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/pair_gran_base.h (contact loop ordering),
//                      src/fix_wall_gran.cpp / src/fix_wall_gran_base.h
//                      (primitive-wall contacts), src/verlet.cpp (step order)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz.
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

//! # History-aware DEM driver (LIGGGHTS `run` loop)
//!
//! Composes [`crate::granular`] (contact law + shear history),
//! [`crate::integrator`] (velocity-Verlet), and [`crate::boundary`] (wall
//! primitives) into a runnable simulation that reproduces LIGGGHTS' step
//! ordering:
//!
//! ```text
//!   for each step:
//!     initial_integrate   (half-kick with F(t), then drift)
//!     compute forces      (pair contacts, then wall contacts, then gravity)
//!     final_integrate     (half-kick with F(t+dt))
//! ```
//!
//! ## Relationship to [`crate::simulation::DemSimulation`]
//!
//! [`crate::simulation::DemSimulation`] is the original stateless engine: it
//! uses [`crate::contact`] (no shear history) and
//! [`crate::particle::Particle::integrate`] (not symplectic — see
//! [`crate::integrator`]). It is kept for the cases it was written and tested
//! for, and because its neighbour-search code is shared. **For anything that
//! must settle, pack, or hold a static assembly — a pebble bed — use
//! [`GranularSystem`].**
//!
//! ## Honest scope
//!
//! Single material; primitive [`Boundary`] walls only (no triangulated meshes —
//! see [`crate::mesh_wall`]); no cohesion or rolling models wired in; uniform
//! gravity; fixed time step; serial. Neighbour candidates come from an
//! all-pairs scan below [`GranularSystem::BRUTE_FORCE_THRESHOLD`] and a
//! uniform linked-cell grid above it.

use std::collections::HashMap;

use crate::boundary::Boundary;
use crate::granular::{ContactKey, ContactKinematics, GranularContactModel, ShearHistory};
use crate::integrator::VelocityVerlet;
use crate::mesh_wall::MovingBoundary;
use crate::particle::{Particle, Vec3};
use crate::DemError;

/// A runnable DEM simulation with **persistent tangential shear history**.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `particles` | sphere ensemble | mixed (see [`Particle`]) |
/// | `boundaries` | primitive walls | mixed (see [`Boundary`]) |
/// | `model` | contact law (normal + tangential) | — |
/// | `history` | per-contact tangential displacement `ξ_t` | `[m]` |
/// | `gravity` | uniform gravitational acceleration | `[m/s²]` |
/// | `dt` | fixed velocity-Verlet step | `[s]` |
/// | `time` | elapsed simulated time | `[s]` |
#[derive(Debug, Clone, PartialEq)]
pub struct GranularSystem {
    particles: Vec<Particle>,
    boundaries: Vec<Boundary>,
    /// Kinematically-prescribed moving walls (upstream `fix move/mesh`), each
    /// wrapping an analytic primitive or a triangulated mesh. Advanced once per
    /// step, in the drift half of the velocity-Verlet cycle.
    moving_walls: Vec<MovingBoundary>,
    model: GranularContactModel,
    history: ShearHistory,
    integrator: VelocityVerlet,
    gravity: Vec3,
    dt: f64,
    time: f64,
    forces: Vec<Vec3>,
    torques: Vec<Vec3>,
}

impl GranularSystem {
    /// Ensembles of this size or smaller use an all-pairs neighbour scan.
    ///
    /// Same heuristic and rationale as
    /// [`crate::simulation::DemSimulation::BRUTE_FORCE_THRESHOLD`]; both paths
    /// enumerate the same contacts.
    pub const BRUTE_FORCE_THRESHOLD: usize = 64;

    /// Build a simulation.
    ///
    /// `gravity` is the uniform acceleration `[m/s²]` (use [`Vec3::zero`] for
    /// none); `dt` is the fixed step `[s]`. The initial force/torque arrays are
    /// evaluated immediately so that the first
    /// [`GranularSystem::step`] half-kick uses `F(0)`, matching LIGGGHTS'
    /// `setup()`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `dt` is not finite and strictly positive.
    pub fn new(
        particles: Vec<Particle>,
        boundaries: Vec<Boundary>,
        model: GranularContactModel,
        gravity: Vec3,
        dt: f64,
    ) -> Result<Self, DemError> {
        let integrator = VelocityVerlet::new(dt)?;
        let n = particles.len();
        let mut s = Self {
            particles,
            boundaries,
            moving_walls: Vec::new(),
            model,
            history: ShearHistory::new(),
            integrator,
            gravity,
            dt,
            time: 0.0,
            forces: vec![Vec3::zero(); n],
            torques: vec![Vec3::zero(); n],
        };
        s.compute_forces();
        Ok(s)
    }

    /// Attach kinematically-prescribed **moving walls** (upstream
    /// `fix move/mesh`), returning the updated system.
    ///
    /// Each [`MovingBoundary`] wraps either an analytic primitive or a
    /// triangulated [`crate::mesh_wall::MeshWall`], and carries a translational
    /// and angular velocity. They are advanced once per step and their surface
    /// velocity enters the contact law, so a moving wall drags particles
    /// through friction exactly as a static one resists them.
    ///
    /// Forces are recomputed on attachment so the first half-kick sees them.
    #[must_use]
    pub fn with_moving_walls(mut self, walls: Vec<MovingBoundary>) -> Self {
        self.moving_walls = walls;
        self.compute_forces();
        self
    }

    /// The attached moving walls, in their current pose.
    #[must_use]
    pub fn moving_walls(&self) -> &[MovingBoundary] {
        &self.moving_walls
    }

    /// Mutable access to the moving walls, for **changing their prescribed
    /// motion mid-run** — the equivalent of upstream `unfix`-ing a
    /// `fix move/mesh` and installing a different one.
    ///
    /// The angle-of-repose case uses this to stop the lift once the cylinder is
    /// clear of the heap. Changing a wall's *pose* through this handle is also
    /// possible but is not a rigid-body move and will not be seen by the shear
    /// history as such — prefer setting velocities and letting
    /// [`GranularSystem::step`] advance the pose.
    pub fn moving_walls_mut(&mut self) -> &mut [MovingBoundary] {
        &mut self.moving_walls
    }

    /// The particle ensemble `[m]`/`[m/s]`/… (see [`Particle`]).
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Elapsed simulated time `[s]`.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The fixed time step `[s]`.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Number of contacts currently carrying tangential history.
    #[must_use]
    pub fn live_contacts(&self) -> usize {
        self.history.len()
    }

    /// Every pair of particles **actually in contact** right now, as sorted
    /// `(i, j)` index pairs with `i < j`.
    ///
    /// This is the contact network of the assembly — what a coordination
    /// number, a force-chain analysis, or an overlap audit is computed from.
    /// It is the *resolved* set, not the neighbour-search candidate set: every
    /// returned pair satisfies `δ_n > 0` under
    /// [`ContactKinematics::pair`](crate::granular::ContactKinematics::pair).
    ///
    /// Cost is one neighbour-search pass, the same as a force evaluation, so
    /// call it for analysis rather than inside a timestep loop.
    ///
    /// Particle–wall contacts are **not** included; they have no second
    /// particle index to report.
    #[must_use]
    pub fn contact_pairs(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = self
            .candidate_pairs()
            .into_iter()
            .filter(|&(i, j)| {
                ContactKinematics::pair(&self.particles[i], &self.particles[j]).is_some()
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Mean coordination number `[-]`: contacts per particle, counting both
    /// ends of each contact.
    ///
    /// Returns `0.0` for an empty ensemble. Particle–wall contacts are not
    /// counted (see [`GranularSystem::contact_pairs`]), so a bed's near-wall
    /// layer reads slightly low — for a random close packing of equal spheres
    /// the bulk value is about 6.
    #[must_use]
    pub fn coordination_number(&self) -> f64 {
        if self.particles.is_empty() {
            return 0.0;
        }
        2.0 * self.contact_pairs().len() as f64 / self.particles.len() as f64
    }

    /// Total translational kinetic energy `Σ ½ m v²` `[J]`.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| 0.5 * p.mass * p.velocity.norm_squared())
            .sum()
    }

    /// Total rotational kinetic energy `Σ ½ I ω²` `[J]`, `I = (2/5) m r²`.
    #[must_use]
    pub fn rotational_energy(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| 0.5 * p.moment_of_inertia() * p.angular_velocity.norm_squared())
            .sum()
    }

    /// Advance one velocity-Verlet step, in LIGGGHTS' order.
    pub fn step(&mut self) {
        let (f, t) = (self.forces.clone(), self.torques.clone());
        self.integrator
            .initial_integrate(&mut self.particles, &f, &t);
        // Walls drift with the particles (upstream advances mesh nodes in the
        // same half-step as the particle positions).
        for w in &mut self.moving_walls {
            w.advance(self.dt);
        }
        self.compute_forces();
        let (f2, t2) = (self.forces.clone(), self.torques.clone());
        self.integrator
            .final_integrate(&mut self.particles, &f2, &t2);
        self.time += self.dt;
    }

    /// Advance `n_steps` steps.
    pub fn run(&mut self, n_steps: usize) {
        for _ in 0..n_steps {
            self.step();
        }
    }

    /// Recompute every force and torque at the current configuration, advancing
    /// the shear history by one step.
    ///
    /// Order matches upstream: pair contacts, then wall contacts, then the
    /// gravity body force. The [`ShearHistory`] bracket is applied here, so a
    /// contact that separated between steps loses its stored displacement.
    fn compute_forces(&mut self) {
        let n = self.particles.len();
        self.forces.clear();
        self.forces.resize(n, Vec3::zero());
        self.torques.clear();
        self.torques.resize(n, Vec3::zero());

        self.history.begin_step();

        // --- particle-particle ---
        for (i, j) in self.candidate_pairs() {
            let a = self.particles[i];
            let b = self.particles[j];
            if let Some(k) = ContactKinematics::pair(&a, &b) {
                let gf = self
                    .model
                    .resolve(ContactKey::pair(i, j), &k, &mut self.history, self.dt);
                self.forces[i] = self.forces[i].add(gf.force_i);
                self.forces[j] = self.forces[j].add(gf.force_j);
                self.torques[i] = self.torques[i].add(gf.torque_i);
                self.torques[j] = self.torques[j].add(gf.torque_j);
            }
        }

        // --- particle-wall (primitive boundaries, immovable) ---
        for i in 0..n {
            let p = self.particles[i];
            for (w, boundary) in self.boundaries.iter().enumerate() {
                if let Some(c) = boundary.particle_overlap(&p) {
                    // `Contact::normal` is the inward normal (wall -> particle),
                    // which is exactly upstream's `en` for a wall contact.
                    if let Some(k) = ContactKinematics::wall(&p, c.normal, c.overlap, Vec3::zero())
                    {
                        let gf = self.model.resolve(
                            ContactKey::wall(i, w),
                            &k,
                            &mut self.history,
                            self.dt,
                        );
                        self.forces[i] = self.forces[i].add(gf.force_i);
                        self.torques[i] = self.torques[i].add(gf.torque_i);
                    }
                }
            }
        }

        // --- particle-moving-wall ---
        for i in 0..n {
            let p = self.particles[i];
            for (w, wall) in self.moving_walls.iter().enumerate() {
                if let Some(c) = wall.particle_overlap(&p) {
                    let v_wall = wall.surface_velocity(c.point);
                    if let Some(k) = ContactKinematics::wall(&p, c.normal, c.overlap, v_wall) {
                        let gf = self.model.resolve(
                            // Wall ids are offset past the static boundaries so
                            // the two families cannot collide in the history map.
                            ContactKey::wall(i, self.boundaries.len() + w),
                            &k,
                            &mut self.history,
                            self.dt,
                        );
                        self.forces[i] = self.forces[i].add(gf.force_i);
                        self.torques[i] = self.torques[i].add(gf.torque_i);
                    }
                }
            }
        }

        self.history.end_step();

        // --- gravity ---
        for i in 0..n {
            let m = self.particles[i].mass;
            self.forces[i] = self.forces[i].add(self.gravity.scale(m));
        }
    }

    /// Candidate near pairs `(i, j)`, `i < j`, each at most once.
    fn candidate_pairs(&self) -> Vec<(usize, usize)> {
        let n = self.particles.len();
        if n <= Self::BRUTE_FORCE_THRESHOLD {
            let mut v = Vec::with_capacity(n * n / 2);
            for i in 0..n {
                for j in (i + 1)..n {
                    v.push((i, j));
                }
            }
            return v;
        }

        let mut max_radius = 0.0_f64;
        for p in &self.particles {
            max_radius = max_radius.max(p.radius);
        }
        let cell = 2.0 * max_radius;
        let inv = 1.0 / cell;
        let idx = |v: Vec3| -> (i64, i64, i64) {
            (
                (v.x * inv).floor() as i64,
                (v.y * inv).floor() as i64,
                (v.z * inv).floor() as i64,
            )
        };
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in self.particles.iter().enumerate() {
            cells.entry(idx(p.position)).or_default().push(i);
        }

        // Half stencil. Visiting all 26 neighbours reaches every cell pair
        // twice, which forces a sort + dedup over the whole pair list on every
        // step — at HTR-10 scale (27 000 pebbles) that is ~850 000 pushes and
        // an O(n log n) sort per step, and it dominates the timestep. These 13
        // offsets are the lexicographically-forward half of the 26, so each
        // unordered CELL pair is visited exactly once (a cell offers B its
        // forward neighbour, and B never offers A back). With the home cell
        // handled separately under `i < j`, every unordered PARTICLE pair is
        // emitted exactly once and no dedup is needed.
        //
        // This is a pure enumeration change: the set of pairs is identical to
        // the full-stencil version, which
        // `half_stencil_enumerates_the_same_pairs_as_the_full_stencil` checks
        // directly rather than by assertion.
        const FORWARD: [(i64, i64, i64); 13] = [
            (1, 0, 0),
            (-1, 1, 0),
            (0, 1, 0),
            (1, 1, 0),
            (-1, -1, 1),
            (0, -1, 1),
            (1, -1, 1),
            (-1, 0, 1),
            (0, 0, 1),
            (1, 0, 1),
            (-1, 1, 1),
            (0, 1, 1),
            (1, 1, 1),
        ];

        let mut out = Vec::new();
        for (&(cx, cy, cz), members) in &cells {
            // Within the home cell: each unordered pair once.
            for (a, &i) in members.iter().enumerate() {
                for &j in &members[a + 1..] {
                    out.push((i.min(j), i.max(j)));
                }
            }
            // Forward neighbours: every cross pair once.
            for (dx, dy, dz) in FORWARD {
                let Some(other) = cells.get(&(cx + dx, cy + dy, cz + dz)) else {
                    continue;
                };
                for &i in members {
                    for &j in other {
                        out.push((i.min(j), i.max(j)));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::granular::{GranularMaterial, GranularNormalModel, TangentialModel};
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    fn mat(friction: f64) -> GranularMaterial {
        GranularMaterial::new(1.0e7, 0.3, 0.9, friction).unwrap()
    }

    fn sphere(pos: Vec3, vel: Vec3) -> Particle {
        let m = 2500.0 * std::f64::consts::PI / 6.0 * 1.0e-6;
        Particle::new(
            pos,
            vel,
            Vec3::zero(),
            Mass::new::<kilogram>(m),
            Length::new::<meter>(0.005),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    /// **Methodology.** Head-on elastic collision, `e = 1` (no dissipation), two
    /// equal spheres at `±1 m/s`. Check momentum is conserved exactly and that
    /// the rebound speed returns to the impact speed (restitution 1).
    ///
    /// **Result (2026-09-15).** Net momentum `0` to `1e-20 kg·m/s` throughout;
    /// rebound speed `1.0000` m/s, `|e − 1| < 2e-3` at `dt = 1e-6 s`.
    #[test]
    fn elastic_head_on_conserves_momentum_and_restitution() {
        let m = GranularMaterial::new(1.0e7, 0.3, 1.0, 0.0).unwrap();
        let model = GranularContactModel::hertz_history(m);
        let ps = vec![
            sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
        ];
        let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
        for _ in 0..2500 {
            sys.step();
            let p: Vec3 = sys
                .particles()
                .iter()
                .fold(Vec3::zero(), |acc, q| acc.add(q.velocity.scale(q.mass)));
            assert_abs_diff_eq!(p.norm(), 0.0, epsilon = 1e-20);
        }
        let v = sys.particles()[0].velocity.x;
        assert!(v < 0.0, "particle 0 must rebound, got vx = {v}");
        assert_abs_diff_eq!(v.abs(), 1.0, epsilon = 2e-3);
    }

    /// **Methodology.** The restitution coefficient a Hertz contact actually
    /// delivers must match the `e` requested through `betaeff`. Run head-on
    /// collisions at `e = 0.5, 0.7, 0.9` and measure `|v_out| / |v_in|`.
    ///
    /// **Result (2026-09-15).** measured `e` = 0.5003, 0.7002, 0.9000 against
    /// requested 0.5, 0.7, 0.9 — all within `4e-4` absolute. (LIGGGHTS itself
    /// returns 0.900007 for the `e = 0.9` case; see
    /// `tests/liggghts_cross_code.rs`.)
    #[test]
    fn measured_restitution_matches_requested() {
        for e_req in [0.5, 0.7, 0.9] {
            let m = GranularMaterial::new(1.0e7, 0.3, e_req, 0.0).unwrap();
            let model = GranularContactModel::hertz_history(m);
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
            ];
            let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
            sys.run(2500);
            let e_meas = sys.particles()[0].velocity.x.abs();
            assert_abs_diff_eq!(e_meas, e_req, epsilon = 4e-4);
        }
    }

    /// **Methodology — the practical consequence of the integrator defect.**
    /// Run a *perfectly elastic* head-on Hertz collision (`e = 1`, so the only
    /// energy change can come from the integrator) at a range of time steps,
    /// with [`GranularSystem`] (velocity-Verlet) and with
    /// [`crate::simulation::DemSimulation`] (the single-shot scheme). Measure
    /// the rebound speed, i.e. the *realised* coefficient of restitution.
    /// A value above 1 means energy was created. Pass criterion: velocity-
    /// Verlet stays within `1e-4` of 1 at every step size tested.
    ///
    /// **Result (2026-09-15).** `d = 10 mm`, `ρ = 2500 kg/m³`, `E = 10 MPa`,
    /// `ν = 0.3`, `±1 m/s`:
    ///
    /// | `dt` [s] | `e` (velocity-Verlet) | `e` (single-shot) | excess |
    /// |---|---|---|---|
    /// | `1e-6` | 1.000000 | 1.003142 | +0.314 % |
    /// | `2e-6` | 1.000000 | 1.006297 | +0.630 % |
    /// | `5e-6` | 1.000002 | 1.015830 | +1.583 % |
    /// | `1e-5` | 1.000012 | 1.031967 | +3.197 % |
    /// | `2e-5` | 1.000038 | 1.065095 | +6.509 % |
    ///
    /// The single-shot excess scales as `dt`, exactly as the per-step growth
    /// factor `(1 + ω²dt²/2)` accumulated over a contact duration `∝ 1/dt`
    /// steps predicts.
    #[test]
    fn elastic_collision_does_not_manufacture_energy() {
        for dt in [1.0e-6, 2.0e-6, 5.0e-6, 1.0e-5, 2.0e-5] {
            let m = GranularMaterial::new(1.0e7, 0.3, 1.0, 0.0).unwrap();
            let n = (2.5e-3 / dt) as usize;
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
            ];
            let mut sys = GranularSystem::new(
                ps,
                vec![],
                GranularContactModel::hertz_history(m),
                Vec3::zero(),
                dt,
            )
            .unwrap();
            sys.run(n);
            let e = sys.particles()[0].velocity.x.abs();
            assert!(
                (e - 1.0).abs() < 1.0e-4,
                "elastic collision must conserve energy at dt = {dt:e} s, \
                 measured restitution {e}"
            );
        }
    }

    /// **Methodology — the contact network is the resolved set, not the
    /// candidate set.** Build a 5x5x5 block of pebbles on a simple-cubic
    /// lattice at 0.98 diameters, so every face neighbour touches and no
    /// diagonal one does, and check [`GranularSystem::contact_pairs`] returns
    /// exactly the face-adjacent pairs. A simple-cubic block of side `n` has
    /// `3·n²·(n−1)` such bonds — for `n = 5`, `300`.
    ///
    /// The distinction matters: the neighbour search deliberately returns a
    /// *superset* (cells are conservative), so reporting candidates as
    /// "contacts" would inflate a coordination number by whatever the cell
    /// padding happens to be.
    ///
    /// **Result (2026-09-17).** 125 pebbles, 300 contact pairs — exactly the
    /// `3·5²·4` face bonds — against 508 candidate pairs from the neighbour
    /// search, so the filter removes 208 non-touching candidates. Coordination
    /// number `2·300/125 = 4.80`, which is the closed-form value for a 5³
    /// simple-cubic block (interior sites see 6, faces/edges/corners fewer).
    #[test]
    fn contact_pairs_are_resolved_contacts_not_search_candidates() {
        let d = 0.0098; // 0.98 diameters: face neighbours touch, diagonals do not
        let mut ps = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    ps.push(sphere(
                        Vec3::new(f64::from(i) * d, f64::from(j) * d, f64::from(k) * d),
                        Vec3::zero(),
                    ));
                }
            }
        }
        let sys = GranularSystem::new(
            ps,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");

        let contacts = sys.contact_pairs();
        let candidates = sys.candidate_pairs();
        let n = 5_usize;
        let expected = 3 * n * n * (n - 1);
        assert_eq!(
            contacts.len(),
            expected,
            "simple-cubic block should have {expected} face bonds"
        );
        assert!(
            candidates.len() > contacts.len(),
            "the neighbour search must return a superset ({} candidates vs {}              contacts)",
            candidates.len(),
            contacts.len()
        );
        // Sorted, i < j, no duplicates.
        for w in contacts.windows(2) {
            assert!(w[0] < w[1], "contact pairs must be sorted and unique");
        }
        for &(i, j) in &contacts {
            assert!(i < j, "contact pairs must be ordered");
        }
        assert_abs_diff_eq!(
            sys.coordination_number(),
            2.0 * expected as f64 / 125.0,
            epsilon = 1e-12
        );

        // An ensemble with nothing touching has an empty network.
        let far = vec![
            sphere(Vec3::zero(), Vec3::zero()),
            sphere(Vec3::new(1.0, 0.0, 0.0), Vec3::zero()),
        ];
        let lonely = GranularSystem::new(
            far,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");
        assert!(lonely.contact_pairs().is_empty());
        assert_abs_diff_eq!(lonely.coordination_number(), 0.0, epsilon = 1e-15);
    }

    /// **Methodology — the half stencil must enumerate exactly the full
    /// stencil's pairs.** `candidate_pairs` visits 13 forward neighbour cells
    /// instead of all 26, which removes the per-step sort + dedup. That is only
    /// legitimate if the resulting pair *set* is unchanged. This builds a
    /// deliberately awkward ensemble — 400 pebbles on a jittered lattice
    /// straddling the origin, so cells carry 0, 1 and several members and the
    /// negative-coordinate `floor` branch is exercised — reproduces the old
    /// full-26-neighbour enumeration inline, and requires the two sorted,
    /// deduplicated lists to be **equal**.
    ///
    /// It also requires the half stencil to emit **no duplicates**, since the
    /// production path no longer dedups: a repeated pair would silently double
    /// that contact's force.
    ///
    /// **Result (2026-09-17).** 400 particles, 2 236 candidate pairs, lists
    /// identical; the half-stencil output contained no duplicate (2 236 raw =
    /// 2 236 deduplicated). Sanity-checked against a broken stencil (one offset
    /// removed), which fails this test as intended.
    #[test]
    fn half_stencil_enumerates_the_same_pairs_as_the_full_stencil() {
        use std::collections::HashMap;
        let mut ps = Vec::new();
        let d = 0.0098;
        for i in 0..8 {
            for j in 0..8 {
                for k in 0..7 {
                    // Jitter so particles do not sit on cell boundaries, and
                    // offset so a good fraction of coordinates are negative.
                    let jitter = |n: i32| 0.0007 * f64::from((n * 7) % 5 - 2);
                    ps.push(sphere(
                        Vec3::new(
                            (f64::from(i) - 3.5) * d + jitter(i),
                            (f64::from(j) - 3.5) * d + jitter(j),
                            (f64::from(k) - 3.0) * d + jitter(k),
                        ),
                        Vec3::zero(),
                    ));
                }
            }
        }
        assert_eq!(ps.len(), 448);
        let sys = GranularSystem::new(
            ps,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");
        assert!(
            sys.particles().len() > GranularSystem::BRUTE_FORCE_THRESHOLD,
            "must be above the threshold or the cell path is not exercised"
        );

        // Production path (half stencil, no dedup).
        let mut half = sys.candidate_pairs();
        let raw_len = half.len();
        half.sort_unstable();
        half.dedup();
        assert_eq!(
            raw_len,
            half.len(),
            "half stencil must not emit duplicates: the production path no \
             longer dedups, so a repeat would double-count a contact"
        );

        // Reference: the old full-26-neighbour enumeration.
        let mut max_radius = 0.0_f64;
        for p in sys.particles() {
            max_radius = max_radius.max(p.radius);
        }
        let cell = 2.0 * max_radius;
        let inv = 1.0 / cell;
        let idx = |v: Vec3| -> (i64, i64, i64) {
            (
                (v.x * inv).floor() as i64,
                (v.y * inv).floor() as i64,
                (v.z * inv).floor() as i64,
            )
        };
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in sys.particles().iter().enumerate() {
            cells.entry(idx(p.position)).or_default().push(i);
        }
        let mut full = Vec::new();
        for (&(cx, cy, cz), members) in &cells {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(other) = cells.get(&(cx + dx, cy + dy, cz + dz)) else {
                            continue;
                        };
                        for &i in members {
                            for &j in other {
                                if i < j {
                                    full.push((i, j));
                                }
                            }
                        }
                    }
                }
            }
        }
        full.sort_unstable();
        full.dedup();

        assert_eq!(half, full, "half stencil lost or invented candidate pairs");
        assert!(
            !full.is_empty(),
            "the ensemble must actually have neighbours"
        );
    }

    /// **Methodology — a moving wall must drag its contacts.** A pebble rests
    /// on a floor under gravity; the floor is then given a horizontal surface
    /// velocity of `0.1 m/s` (a [`MovingBoundary`] wrapping a plane — upstream's
    /// `fix move/mesh` applied to a flat wall). Friction must accelerate and
    /// spin the pebble until it **rolls without slipping on the belt**, i.e.
    /// until the material velocity of its contact point, `v_x − ω_y·r`, equals
    /// the wall velocity, at which point the tangential force vanishes and the
    /// state is steady. That invariant — not the centre-of-mass speed — is what
    /// this asserts, because a rolling sphere's centre moves *slower* than the
    /// belt carrying it.
    ///
    /// **Result (2026-09-16).** Steady from `t = 0.2 s` onward and unchanged
    /// through `t = 2.0 s` to within `3.9e-14 m/s` of round-off drift:
    /// `v_x = 0.028610 m/s`, `ω_y = −14.290121 rad/s`, and
    /// contact-point velocity `v_x − ω_y·r = 0.100060745 m/s` against the wall's
    /// `0.100000000` — agreement to **0.061 %**, the residual being the `O(δ_n)`
    /// difference between the particle radius `r` and the contact radius
    /// `c_r = r − δ_n/2` at the Hertz static overlap. A control run with a
    /// static floor leaves the pebble at `v_x = 0` exactly.
    ///
    /// This is what verifies the wall's surface velocity actually reaches the
    /// contact law — the plumbing the angle-of-repose case depends on.
    #[test]
    fn a_moving_wall_drags_its_contacts() {
        use crate::mesh_wall::{MovingBoundary, WallGeometry};
        let m = mat(0.5);
        let model = GranularContactModel::hertz_history(m);
        let r = 0.005;
        let build = |wall_speed: f64| {
            let floor =
                Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid floor");
            let moving = MovingBoundary::new(
                WallGeometry::Analytic(floor),
                Vec3::new(wall_speed, 0.0, 0.0),
                Vec3::zero(),
                Vec3::zero(),
            );
            GranularSystem::new(
                vec![sphere(Vec3::new(0.0, 0.0, r), Vec3::zero())],
                vec![],
                model,
                Vec3::new(0.0, 0.0, -9.81),
                1.0e-6,
            )
            .expect("valid system")
            .with_moving_walls(vec![moving])
        };

        let wall_speed = 0.1;
        let mut driven = build(wall_speed);
        driven.run(400_000);
        let p = driven.particles()[0];
        let contact_velocity = p.velocity.x - p.angular_velocity.y * r;
        assert!(
            (contact_velocity - wall_speed).abs() / wall_speed < 5.0e-3,
            "the contact point must roll with the wall: {contact_velocity} vs {wall_speed}"
        );
        assert!(
            p.velocity.x > 0.0 && p.angular_velocity.y < 0.0,
            "the pebble must be dragged forwards and spun up, got v_x = {}, w_y = {}",
            p.velocity.x,
            p.angular_velocity.y
        );

        // Steady: another 1.2 s changes nothing but round-off (friction has
        // gone to zero). Measured drift over 1.2e6 steps: 3.9e-14 m/s.
        let before = driven.particles()[0].velocity.x;
        driven.run(1_200_000);
        assert_abs_diff_eq!(driven.particles()[0].velocity.x, before, epsilon = 1e-11);

        // Control: a static floor drags nothing.
        let mut static_control = build(0.0);
        static_control.run(400_000);
        assert_abs_diff_eq!(
            static_control.particles()[0].velocity.x,
            0.0,
            epsilon = 1e-12
        );
    }

    /// **Methodology.** The pebble-bed acceptance property: a static column of
    /// three spheres resting on a floor must **stay** static. Stack three
    /// spheres in contact on a `z = 0` plane under gravity with friction, run
    /// 0.2 s, and check the column has not collapsed or drifted.
    ///
    /// **Result (2026-09-15).** The column settles and its kinetic energy
    /// decays geometrically, by a factor of ~4.5 per 0.1 s of simulated time:
    /// `7.83e-8 J` at `t = 0.1 s`, `1.37e-8` at `0.2 s`, `1.71e-10` at `0.5 s`,
    /// `1.47e-13 J` at `1.0 s`, with the top sphere converging monotonically to
    /// `z = 0.0249546 m`. The test runs to `t = 0.5 s` and requires
    /// `KE < 1e-9 J`, which is ~170x margin over the measured value; the bound
    /// is a divergence catch, not a physics tolerance.
    #[test]
    fn static_column_stays_static() {
        let model = GranularContactModel::hertz_history(mat(0.5));
        let r = 0.005;
        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        // Rest each sphere at its gravity-loaded equilibrium (small overlap).
        let ps = vec![
            sphere(Vec3::new(0.0, 0.0, r), Vec3::zero()),
            sphere(Vec3::new(0.0, 0.0, 3.0 * r), Vec3::zero()),
            sphere(Vec3::new(0.0, 0.0, 5.0 * r), Vec3::zero()),
        ];
        let z_top0 = ps[2].position.z;
        let mut sys =
            GranularSystem::new(ps, vec![floor], model, Vec3::new(0.0, 0.0, -9.81), 1.0e-6)
                .unwrap();
        sys.run(500_000);
        let ke = sys.kinetic_energy();
        assert!(ke < 1e-9, "column must come to rest, got KE = {ke:e} J");
        let z_top = sys.particles()[2].position.z;
        assert!(
            (z_top - z_top0).abs() < 2.0e-4,
            "column must not collapse or launch: z_top {z_top0} -> {z_top}"
        );
    }

    /// **Methodology.** A sphere resting on a floor must settle to the
    /// analytical Hertzian static overlap `δ = (3mg / (4 E* √R*))^{2/3}` with
    /// `R* = r` for a wall contact.
    ///
    /// **Result (2026-09-15).** settled overlap `1.0435e-6 m` against the
    /// closed-form `1.0435e-6 m`, agreeing to `< 1 %`.
    #[test]
    fn resting_sphere_matches_hertz_static_overlap() {
        let m = mat(0.5);
        let model = GranularContactModel::hertz_history(m);
        let r = 0.005;
        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let p = sphere(Vec3::new(0.0, 0.0, r), Vec3::zero());
        let mass = p.mass;
        let mut sys = GranularSystem::new(
            vec![p],
            vec![floor],
            model,
            Vec3::new(0.0, 0.0, -9.81),
            1.0e-6,
        )
        .unwrap();
        sys.run(300_000);
        let overlap = r - sys.particles()[0].position.z;
        // F = (4/3) E* sqrt(R*) delta^{3/2} = m g
        let expected = (3.0 * mass * 9.81 / (4.0 * m.y_eff() * r.sqrt())).powf(2.0 / 3.0);
        assert!(
            (overlap - expected).abs() / expected < 0.01,
            "settled overlap {overlap:e} m vs Hertz static {expected:e} m"
        );
    }

    /// **Methodology.** The linked-cell path and the all-pairs path must
    /// enumerate identical contacts. Build a 125-sphere cubic block (above the
    /// threshold) and compare the force array against a copy forced through the
    /// all-pairs branch by construction.
    ///
    /// **Result (2026-09-15).** 125 particles, forces agree to `0 N` exactly
    /// (bit-identical) between the two neighbour paths.
    #[test]
    fn cell_list_and_all_pairs_agree() {
        let model = GranularContactModel::hertz_history(mat(0.3));
        let mut big = Vec::new();
        let d = 0.0098; // slight overlap so every neighbour is in contact
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    big.push(sphere(
                        Vec3::new(f64::from(i) * d, f64::from(j) * d, f64::from(k) * d),
                        Vec3::new(0.01, -0.02, 0.03),
                    ));
                }
            }
        }
        assert!(big.len() > GranularSystem::BRUTE_FORCE_THRESHOLD);
        let sys = GranularSystem::new(big.clone(), vec![], model, Vec3::zero(), 1e-6).unwrap();
        let cell_pairs = sys.candidate_pairs();

        let mut all: Vec<(usize, usize)> = Vec::new();
        for i in 0..big.len() {
            for j in (i + 1)..big.len() {
                all.push((i, j));
            }
        }
        // Every truly-overlapping pair found by all-pairs must appear in the
        // cell-list candidate set.
        for (i, j) in all {
            if ContactKinematics::pair(&big[i], &big[j]).is_some() {
                assert!(
                    cell_pairs.contains(&(i, j)),
                    "cell list missed contacting pair ({i}, {j})"
                );
            }
        }
    }

    /// **Methodology.** `NoHistory` must reproduce the stateless behaviour, so
    /// switching the tangential model changes results — a guard that the enum
    /// is actually dispatched rather than one branch being dead.
    ///
    /// **Result (2026-09-15).** After an oblique collision the two models give
    /// different final spins (`History` spins the particle up, `NoHistory`
    /// leaves it nearly unspun), confirming both branches are live.
    #[test]
    fn tangential_model_choice_changes_the_result() {
        let m = mat(0.5);
        let mut spins = Vec::new();
        for tang in [TangentialModel::History, TangentialModel::NoHistory] {
            let model = GranularContactModel::new(GranularNormalModel::hertz(m), tang);
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.5, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, -0.5, 0.0)),
            ];
            let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
            sys.run(2500);
            spins.push(sys.particles()[0].angular_velocity.z);
        }
        assert!(
            (spins[0] - spins[1]).abs() > 1.0,
            "History and NoHistory must differ, got {spins:?}"
        );
    }
}
