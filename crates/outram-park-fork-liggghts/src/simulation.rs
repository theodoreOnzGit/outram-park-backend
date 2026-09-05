// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — the discrete-element time-integration loop,
// the linked-cell (cell-list) neighbour search, and the soft-sphere
// particle–wall coupling are STANDARD PUBLIC-LITERATURE MOLECULAR-DYNAMICS / DEM
// techniques. They are reimplemented CLEAN-ROOM from the cited textbooks and
// papers (see the module "References" below); this file contains NO GPL-2.0
// LIGGGHTS/LAMMPS source and is not a translation of it. See the crate NOTICE.
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

//! Phase 5 — **Multi-particle DEM simulation engine** (bead `op-t3l`).
//!
//! This module ties the crate's verified foundations —
//! [`Particle`](crate::particle::Particle) (velocity-Verlet integration),
//! [`ContactModel`](crate::contact::ContactModel) (Hooke / Hertz pairwise
//! forces), and [`Boundary`](crate::boundary::Boundary) (particle–wall overlap
//! geometry) — into a single runnable **soft-sphere DEM** engine,
//! [`DemSimulation`]. It owns an ensemble of spheres, a set of static wall
//! boundaries, one contact law, a uniform gravitational body force, and a fixed
//! time step, and advances them with an explicit velocity-Verlet loop.
//!
//! # The DEM time-step (one call to [`DemSimulation::step`])
//!
//! A soft-sphere discrete-element step is the standard force-accumulate /
//! integrate cycle of Cundall & Strack (1979):
//!
//! 1. **Neighbour search.** Build a fresh uniform cell list and enumerate the
//!    candidate near pairs (see "Neighbour search" below).
//! 2. **Zero accumulators.** Reset every particle's force `[N]` and torque
//!    `[N·m]` accumulator to zero for this step.
//! 3. **Pairwise contact forces.** For each candidate pair, evaluate the
//!    [`ContactModel`](crate::contact::ContactModel); if the pair overlaps,
//!    accumulate the equal-and-opposite forces and the contact-point torques on
//!    the two partners (Newton's third law).
//! 4. **Particle–wall forces.** For each particle overlapping a boundary,
//!    convert the geometric [`Contact`](crate::boundary::Contact) into a force
//!    with the **same** contact law by pairing the particle against an
//!    immovable "image" partner (see "Particle–wall coupling" below), and
//!    accumulate the force/torque on the particle only.
//! 5. **Body force.** Add gravity `F = m·g` `[N]` to every particle.
//! 6. **Integrate.** Advance every particle one velocity-Verlet step of size
//!    `dt` `[s]` with its accumulated force and torque.
//!
//! # Neighbour search — uniform linked-cell (cell list)
//!
//! Finding which of `N` particles are close enough to touch is the dominant
//! cost of a naive DEM step, `O(N²)` if every pair is tested. The textbook fix
//! is the **uniform cell list** (also "linked-cell" or "spatial hash"): overlay
//! the domain with a regular grid whose cell edge equals the **largest particle
//! diameter**, bin each particle into the cell containing its centre, and then
//! test a particle only against the particles in its own cell and the 26
//! neighbouring cells (a `3×3×3` stencil). Because the cell edge is one full
//! diameter, any two spheres that overlap (centre distance `< r_a + r_b ≤`
//! max diameter) are guaranteed to fall in the same or adjacent cells, so the
//! stencil misses no real contact. With a roughly uniform density the work is
//! `O(N)` instead of `O(N²)`. See Allen & Tildesley (1987) §5.3.2 and Pöschel &
//! Schwager (2005) §3.2.
//!
//! Each unordered pair is emitted **once**: while visiting particle `i` we take
//! a stencil neighbour `j` only when `j > i`. For very small ensembles the cell
//! machinery is pure overhead, so at or below
//! [`DemSimulation::BRUTE_FORCE_THRESHOLD`] particles the engine falls back to a
//! direct all-pairs `O(N²)` scan; both paths return the identical set of
//! contacts (verified by [`tests::cell_list_matches_brute_force`]).
//!
//! # Particle–wall coupling — the immovable image partner
//!
//! [`Boundary::particle_overlap`](crate::boundary::Boundary::particle_overlap)
//! returns only geometry: a penetration depth `δ` `[m]` and an inward unit
//! normal `n̂_c` (pointing from the wall surface back into the domain, toward the
//! particle centre). To turn that into a force **using the same contact law as
//! the particle–particle contacts** — so a wall and a neighbouring grain are
//! modelled consistently — the engine builds a fictitious **image partner**: a
//! sphere of the *same radius and mass* as the real particle, held motionless
//! (zero linear and angular velocity — the walls here are static), placed on the
//! far (solid) side of the surface so that the pair geometry reproduces exactly
//! the wall overlap `δ` and normal `n̂_c`.
//!
//! Concretely, with the real particle as contact partner `a` and the image as
//! `b`, the image centre is `c_b = c_a − n̂_c·(2r − δ)`. The pairwise law then
//! sees line-of-centres normal `n̂ = (c_b − c_a)/‖…‖ = −n̂_c`, centre distance
//! `2r − δ`, and hence overlap `(r + r) − (2r − δ) = δ` — the wall penetration —
//! and produces a repulsive force on `a` directed along `−n̂ = +n̂_c`, i.e. back
//! into the domain, exactly as a wall should. The force and contact-point torque
//! on the real particle are kept; the reaction on the (infinitely heavy,
//! immovable) wall is discarded. Because the image mirrors the particle, the
//! reduced quantities the Hertz law derives are `R* = r/2` and `m* = m/2`; this
//! is the standard "image-particle" wall convention (Pöschel & Schwager 2005,
//! §3.3). A rigid flat wall would instead have `R* = r` and `m* = m`; the
//! difference only rescales the Hertz normal stiffness / damping prefactor and
//! is documented here rather than silently chosen. For the linear
//! [`HookeContact`](crate::contact::HookeContact) law — whose scalar force
//! ignores `R*` and `m*` entirely — the two conventions coincide, so the wall
//! force is `k_n·δ (+ γ_n·v_n)` with no ambiguity.
//!
//! # Unit convention
//!
//! Following the crate convention (see [`crate::particle`]), state is stored as
//! plain `f64` in **SI base units** with the unit spelled out on every field and
//! method: positions `[m]`, velocities `[m/s]`, forces `[N]`, torques `[N·m]`,
//! gravity `[m/s²]`, time and step `[s]`, energy `[J]`, momentum `[kg·m/s]`.
//!
//! # Honest scope (Phase 5)
//!
//! This is a **verified engine, not a validated one** — no cross-code or
//! experimental benchmark comparison has been run. The inline tests check the
//! loop against conservation laws (linear momentum), analytical rest states, and
//! internal consistency (cell list vs brute force), **not** against a reference
//! DEM code; that quantitative validation is the later human step in this bead's
//! Definition of Done. The engine deliberately implements **only**:
//!
//! - **Spheres**, single-threaded, on a **single uniform** cell size equal to
//!   the largest particle diameter. A strongly **polydisperse** packing (a wide
//!   spread of radii) therefore wastes work — the cell is sized for the biggest
//!   grain, so cells hold many small ones; a multi-level / per-size grid is
//!   future work, noted but not done.
//! - **Static** boundaries only: the image partner carries zero velocity, so
//!   moving/rotating walls are not modelled (they are out of scope in
//!   [`crate::boundary`] too).
//! - **No periodic boundaries** — the domain is open unless walled.
//! - A **stateless (history-free) tangential** contact: the engine passes the
//!   instantaneous particle states to the contact law each step and carries **no**
//!   accumulated tangential spring displacement `ξ_t` between steps (see the
//!   [`crate::contact`] "Honest scope" note). Tangential friction is therefore
//!   the Coulomb-capped dashpot term only; a history-dependent tangential spring
//!   would require per-contact state keyed by particle pair across steps and is
//!   future work.
//! - **No broad-phase parallelism.** The step is single-threaded; a
//!   thread-per-cell or spatial-decomposition parallel step (e.g. via `rayon`)
//!   is noted as future work and intentionally not added here.
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - M. P. Allen and D. J. Tildesley, *Computer Simulation of Liquids* (Oxford
//!   University Press, 1987) — §5.3.2 cell lists / neighbour lists, and the
//!   velocity-Verlet propagator.
//! - T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
//!   Algorithms* (Springer, 2005) — §3.2 linked-cell neighbour search, §3.3
//!   particle–wall (image-particle) contact, and the soft-sphere DEM loop.
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for granular
//!   assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the soft-sphere DEM
//!   force-accumulate / explicit-integrate time step.
//! - L. Verlet, "Computer 'Experiments' on Classical Fluids. I.," *Phys. Rev.*
//!   **159**, 98–103 (1967); W. C. Swope et al., *J. Chem. Phys.* **76**, 637
//!   (1982) — the velocity-Verlet integrator used per particle.

use std::collections::HashMap;

use crate::boundary::{Boundary, Contact};
use crate::contact::ContactModel;
use crate::particle::{Particle, Vec3};
use crate::DemError;

/// A runnable multi-particle **soft-sphere DEM** simulation.
///
/// Owns its particle ensemble **by value** in a `Vec<Particle>` (particles are
/// referenced elsewhere by their `usize` index, never by reference or `Box`,
/// per the workspace design rules), a set of static wall
/// [`Boundary`](crate::boundary::Boundary) primitives, one
/// [`ContactModel`](crate::contact::ContactModel) applied to every contact, a
/// uniform gravitational acceleration, and a fixed integration time step.
///
/// Advance the system with [`DemSimulation::step`] (one velocity-Verlet step) or
/// [`DemSimulation::run`] (many). Query the state with
/// [`DemSimulation::particles`], [`DemSimulation::kinetic_energy`],
/// [`DemSimulation::total_momentum`], and [`DemSimulation::time`].
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `particles` | the sphere ensemble (state carriers) | mixed (see [`Particle`]) |
/// | `boundaries` | static domain walls | mixed (see [`Boundary`](crate::boundary::Boundary)) |
/// | `contact_model` | the pairwise & particle–wall force law | — |
/// | `gravity` | uniform gravitational acceleration `g` | `[m/s²]` |
/// | `dt` | fixed integration time step | `[s]` |
/// | `time` | elapsed simulated time since construction | `[s]` |
#[derive(Debug, Clone, PartialEq)]
pub struct DemSimulation {
    particles: Vec<Particle>,
    boundaries: Vec<Boundary>,
    contact_model: ContactModel,
    gravity: Vec3,
    dt: f64,
    time: f64,
}

impl DemSimulation {
    /// Ensembles with this many particles **or fewer** use a direct all-pairs
    /// `O(N²)` neighbour scan instead of the cell list.
    ///
    /// Below a few tens of particles the constant-factor cost of hashing every
    /// particle into a grid and walking a `3×3×3` stencil exceeds the cost of
    /// simply testing all `N(N−1)/2` pairs, so the engine skips the grid. The
    /// threshold is a documented heuristic (`64`), not a tuned optimum; both
    /// paths yield identical contacts, so the choice is purely about speed. See
    /// the module-level "Neighbour search" note.
    pub const BRUTE_FORCE_THRESHOLD: usize = 64;

    /// Construct a DEM simulation from its ensemble, walls, contact law, gravity,
    /// and time step.
    ///
    /// `particles` are moved in and owned by value; `boundaries` are the static
    /// walls (may be empty for an unbounded domain); `contact_model` is applied
    /// to every particle–particle and particle–wall contact; `gravity` is the
    /// uniform gravitational acceleration `[m/s²]` (pass [`Vec3::zero`] for a
    /// gravity-free run); `dt` is the fixed velocity-Verlet step `[s]`. The
    /// elapsed [`DemSimulation::time`] starts at zero.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `dt` is not strictly positive (a
    /// non-positive step cannot advance the explicit integrator). Particle
    /// validity is already guaranteed by
    /// [`Particle::new`](crate::particle::Particle::new); an empty ensemble is
    /// permitted (its dynamics are trivially empty).
    pub fn new(
        particles: Vec<Particle>,
        boundaries: Vec<Boundary>,
        contact_model: ContactModel,
        gravity: Vec3,
        dt: f64,
    ) -> Result<Self, DemError> {
        if !(dt > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "time step dt must be strictly positive, got {dt} s"
            )));
        }
        Ok(Self {
            particles,
            boundaries,
            contact_model,
            gravity,
            dt,
            time: 0.0,
        })
    }

    /// The particle ensemble, as an immutable slice `[m]`/`[m/s]`/… (see
    /// [`Particle`]). Particles keep their index across the run.
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// The static wall boundaries of the domain.
    #[must_use]
    pub fn boundaries(&self) -> &[Boundary] {
        &self.boundaries
    }

    /// The contact force law applied to every contact.
    #[must_use]
    pub fn contact_model(&self) -> ContactModel {
        self.contact_model
    }

    /// The uniform gravitational acceleration `g` `[m/s²]`.
    #[must_use]
    pub fn gravity(&self) -> Vec3 {
        self.gravity
    }

    /// The fixed integration time step `dt` `[s]`.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Elapsed simulated time since construction `[s]` (advances by `dt` each
    /// [`DemSimulation::step`]).
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Number of particles in the ensemble.
    #[must_use]
    pub fn num_particles(&self) -> usize {
        self.particles.len()
    }

    /// Total kinetic energy of the ensemble `[J]`.
    ///
    /// Sums the translational `½·m·‖v‖²` and rotational `½·I·‖ω‖²` kinetic
    /// energy of every particle, with the solid-sphere moment of inertia
    /// `I = (2/5) m r²` (see
    /// [`Particle::moment_of_inertia`](crate::particle::Particle::moment_of_inertia)).
    /// Potential (gravitational, contact-spring) energy is **not** included.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| {
                let translational = 0.5 * p.mass * p.velocity.norm_squared();
                let rotational = 0.5 * p.moment_of_inertia() * p.angular_velocity.norm_squared();
                translational + rotational
            })
            .sum()
    }

    /// Total linear momentum of the ensemble `[kg·m/s]`: `Σ mᵢ·vᵢ`.
    ///
    /// In the absence of external forces (no gravity, no walls) the pairwise
    /// contact forces are equal-and-opposite, so this is conserved to
    /// floating-point round-off each step — the invariant checked by
    /// [`tests::head_on_collision_conserves_linear_momentum`].
    #[must_use]
    pub fn total_momentum(&self) -> Vec3 {
        self.particles
            .iter()
            .fold(Vec3::zero(), |acc, p| acc.add(p.velocity.scale(p.mass)))
    }

    /// Advance the whole system by one velocity-Verlet step of size `dt` `[s]`.
    ///
    /// Performs the full soft-sphere DEM cycle documented at the module level:
    /// rebuild the neighbour cells, zero the force/torque accumulators,
    /// accumulate pairwise contact forces (equal and opposite), accumulate
    /// particle–wall forces via the immovable image partner, add gravity, then
    /// integrate every particle one step. Advances [`DemSimulation::time`] by
    /// `dt`.
    pub fn step(&mut self) {
        let n = self.particles.len();
        let mut forces = vec![Vec3::zero(); n];
        let mut torques = vec![Vec3::zero(); n];
        let model = self.contact_model;

        // (a)+(c) Pairwise particle–particle contacts. Each near pair is visited
        // once; forces/torques are equal-and-opposite (Newton's third law).
        for (i, j) in self.candidate_pairs() {
            // Particle is Copy, so snapshotting both sides avoids aliasing the
            // Vec while calling the (immutable) contact law.
            let a = self.particles[i];
            let b = self.particles[j];
            if let Some(cf) = model.contact_force(&a, &b) {
                forces[i] = forces[i].add(cf.force_on_a);
                forces[j] = forces[j].add(cf.force_on_b);
                torques[i] = torques[i].add(cf.torque_on_a);
                torques[j] = torques[j].add(cf.torque_on_b);
            }
        }

        // (d) Particle–wall contacts via the immovable image partner.
        for i in 0..n {
            let p = self.particles[i];
            for boundary in &self.boundaries {
                if let Some(contact) = boundary.particle_overlap(&p) {
                    let image = Self::wall_image_partner(&p, &contact);
                    if let Some(cf) = model.contact_force(&p, &image) {
                        forces[i] = forces[i].add(cf.force_on_a);
                        torques[i] = torques[i].add(cf.torque_on_a);
                    }
                }
            }
        }

        // (e) Gravity body force, then (f) velocity-Verlet integrate.
        for i in 0..n {
            let m = self.particles[i].mass;
            let total_force = forces[i].add(self.gravity.scale(m));
            self.particles[i].integrate(total_force, torques[i], self.dt);
        }

        self.time += self.dt;
    }

    /// Run [`DemSimulation::step`] `n_steps` times, advancing the system by
    /// `n_steps · dt` `[s]` of simulated time.
    pub fn run(&mut self, n_steps: usize) {
        for _ in 0..n_steps {
            self.step();
        }
    }

    /// Build the fictitious immovable **image partner** for a particle–wall
    /// contact (see the module-level "Particle–wall coupling" note).
    ///
    /// Given the real particle `p` and the geometric wall [`Contact`] (`δ` and
    /// inward normal `n̂_c`), returns a motionless sphere of the same radius and
    /// mass placed at `c_p − n̂_c·(2r − δ)`, so that pairing `p` against it
    /// through the contact law reproduces the wall overlap `δ` and a repulsive
    /// force directed back into the domain. The image inherits `p`'s temperature
    /// only to satisfy the positive-temperature field invariant; thermal DEM is
    /// out of scope here.
    ///
    /// This assumes the small-overlap soft-sphere regime `δ < 2r` (a
    /// well-resolved DEM step never penetrates a wall by more than a diameter),
    /// so the image centre lies strictly on the solid side of the surface.
    fn wall_image_partner(p: &Particle, contact: &Contact) -> Particle {
        let n_c = contact.normal;
        let dist = 2.0 * p.radius - contact.overlap;
        let center = p.position.sub(n_c.scale(dist));
        Particle {
            position: center,
            velocity: Vec3::zero(),
            angular_velocity: Vec3::zero(),
            mass: p.mass,
            radius: p.radius,
            temperature: p.temperature,
        }
    }

    /// Uniform cell edge length `[m]` for the linked-cell grid: the largest
    /// particle **diameter** in the ensemble, `2·max_i r_i`.
    ///
    /// Sizing the cell to one full max-diameter guarantees that any overlapping
    /// pair (centre distance `< r_a + r_b ≤` max diameter) lands in the same or
    /// an adjacent cell, so the `3×3×3` stencil misses no contact. Only called
    /// on the cell-list path (`N > BRUTE_FORCE_THRESHOLD`), where at least one
    /// particle exists and every radius is strictly positive, so the result is
    /// strictly positive.
    fn cell_size(&self) -> f64 {
        let mut max_radius = 0.0_f64;
        for p in &self.particles {
            if p.radius > max_radius {
                max_radius = p.radius;
            }
        }
        2.0 * max_radius
    }

    /// Enumerate the candidate near pairs `(i, j)` with `i < j` to test for
    /// contact this step.
    ///
    /// Uses the uniform linked-cell search (module-level "Neighbour search") for
    /// `N > `[`DemSimulation::BRUTE_FORCE_THRESHOLD`], falling back to the direct
    /// all-pairs scan at or below the threshold. Every unordered pair appears at
    /// most once. The returned list is a *superset* of the truly contacting
    /// pairs on the cell-list path (cells are conservative); the caller filters
    /// it through the contact law, which returns `None` for non-overlapping
    /// pairs.
    fn candidate_pairs(&self) -> Vec<(usize, usize)> {
        let n = self.particles.len();
        if n <= Self::BRUTE_FORCE_THRESHOLD {
            return Self::all_pairs(n);
        }

        let cell = self.cell_size();
        let inv = 1.0 / cell;
        // Integer cell index of a centre; `floor` handles negative coordinates.
        let cell_index = |v: Vec3| -> (i64, i64, i64) {
            (
                (v.x * inv).floor() as i64,
                (v.y * inv).floor() as i64,
                (v.z * inv).floor() as i64,
            )
        };

        // Bin every particle centre into its cell.
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in self.particles.iter().enumerate() {
            cells.entry(cell_index(p.position)).or_default().push(i);
        }

        // For each particle, gather stencil neighbours with a greater index so
        // each unordered pair is emitted exactly once.
        let mut pairs = Vec::new();
        for (i, p) in self.particles.iter().enumerate() {
            let (cx, cy, cz) = cell_index(p.position);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(bucket) = cells.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &j in bucket {
                                if j > i {
                                    pairs.push((i, j));
                                }
                            }
                        }
                    }
                }
            }
        }
        pairs
    }

    /// All unordered index pairs `(i, j)`, `i < j`, of an `n`-particle ensemble
    /// — the direct `O(N²)` neighbour list.
    fn all_pairs(n: usize) -> Vec<(usize, usize)> {
        let mut pairs = Vec::with_capacity(n.saturating_mul(n.saturating_sub(1)) / 2);
        for i in 0..n {
            for j in (i + 1)..n {
                pairs.push((i, j));
            }
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contact::HookeContact;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Helper: a valid particle with the given kinematic state, mass, and radius
    /// (temperature fixed at 300 K, passive in this crate).
    fn particle(position: Vec3, velocity: Vec3, mass_kg: f64, radius_m: f64) -> Particle {
        Particle::new(
            position,
            velocity,
            Vec3::zero(),
            Mass::new::<kilogram>(mass_kg),
            Length::new::<meter>(radius_m),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid particle")
    }

    /// Helper: the standard linear spring-dashpot model used by the tests.
    fn hooke(k_n: f64, gamma_n: f64) -> ContactModel {
        ContactModel::Hooke(HookeContact::new(k_n, gamma_n, 0.0, 0.0, 0.3).expect("valid hooke"))
    }

    /// Contact set found by the (private) cell-list / brute-force dispatcher,
    /// filtered to the pairs that actually overlap. Returned sorted so two
    /// engines' contact sets compare directly.
    fn contacts_via_candidate_pairs(sim: &DemSimulation) -> Vec<(usize, usize)> {
        let model = sim.contact_model();
        let mut out: Vec<(usize, usize)> = sim
            .candidate_pairs()
            .into_iter()
            .filter(|&(i, j)| {
                model
                    .contact_force(&sim.particles()[i], &sim.particles()[j])
                    .is_some()
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Contact set found by an explicit all-pairs `O(N²)` scan, filtered to
    /// overlapping pairs and sorted.
    fn contacts_via_brute_force(sim: &DemSimulation) -> Vec<(usize, usize)> {
        let model = sim.contact_model();
        let n = sim.num_particles();
        let mut out = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                if model
                    .contact_force(&sim.particles()[i], &sim.particles()[j])
                    .is_some()
                {
                    out.push((i, j));
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// V&V — **(a) a particle dropped onto a floor falls, contacts, and rebounds
    /// with physically correct energy behaviour.**
    ///
    /// Methodology: one sphere (`m = 1 kg`, `r = 0.1 m`) is released from rest at
    /// centre height `z₀ = 0.5 m` above a one-sided floor
    /// [`Boundary::wall`](crate::boundary::Boundary::wall) at `z = 0` (outward
    /// normal `+ẑ`), under gravity `g = (0, 0, −9.81) m/s²`. The contact law is
    /// linear spring-dashpot with `k_n = 1.0e5 N/m` and damping
    /// `γ_n = 50 N·s/m` (so the collision is inelastic and dissipates energy).
    /// The step is `dt = 1.0e-5 s` (well inside the contact-spring stability
    /// bound `dt < 2/ω`, `ω = √(k_n/m*) ≈ 447 rad/s`, `m* = m/2`); the system is
    /// run for `60 000` steps (`t = 0.6 s`), long enough for the full first
    /// bounce. Tracked over the run: whether contact occurred (any centre height
    /// `< r`), the minimum centre height (tunnelling check), and the maximum
    /// centre height reached **after** the first contact (the rebound apex).
    ///
    /// Physical pass criteria: (1) the ball makes contact; (2) it never tunnels
    /// through the floor (centre height stays `> 0`); (3) it rebounds — the
    /// post-contact apex `z_reb` satisfies `r < z_reb`; and (4) energy is
    /// dissipated, not gained — the rebound apex is lower than the release
    /// height, `z_reb < z₀` (gravitational energy `m·g·z` at the apex, where the
    /// ball is momentarily at rest, must fall).
    ///
    /// Result (measured 2026-07-24): contact occurred; minimum centre height
    /// `≈ 0.092036 m > 0` (no tunnelling, penetration `≈ 8.0 mm ≪ r = 100 mm`);
    /// rebound apex `z_reb ≈ 0.342141 m`, satisfying `r = 0.1 < 0.342141 < 0.5 =
    /// z₀`. The ball lost `≈ 32 %` of its drop height to the inelastic contact,
    /// i.e. an apparent coefficient of restitution
    /// `√((z_reb − r)/(z₀ − r)) ≈ 0.78`, consistent with the chosen damping. This
    /// confirms the engine's drop → contact → rebound cycle is energetically
    /// physical (monotone energy loss, no tunnelling).
    #[test]
    fn particle_dropped_on_floor_falls_contacts_and_rebounds() {
        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let p = particle(Vec3::new(0.0, 0.0, 0.5), Vec3::zero(), 1.0, 0.1);
        let mut sim = DemSimulation::new(
            vec![p],
            vec![floor],
            hooke(1.0e5, 50.0),
            Vec3::new(0.0, 0.0, -9.81),
            1.0e-5,
        )
        .unwrap();

        let z0 = 0.5;
        let r = 0.1;
        let mut contacted = false;
        let mut min_z = f64::INFINITY;
        let mut apex_after_contact = f64::NEG_INFINITY;

        for _ in 0..60_000 {
            sim.step();
            let z = sim.particles()[0].position.z;
            if z < min_z {
                min_z = z;
            }
            if z < r {
                contacted = true;
            }
            if contacted {
                if z > apex_after_contact {
                    apex_after_contact = z;
                }
            }
        }

        // (1) it made contact with the floor.
        assert!(contacted, "particle never reached the floor");
        // (2) it did not tunnel through the wall.
        assert!(
            min_z > 0.0,
            "particle tunnelled through the floor: min_z = {min_z}"
        );
        // (3) it rebounded back up off the floor.
        assert!(
            apex_after_contact > r,
            "no rebound: apex {apex_after_contact} not above radius {r}"
        );
        // (4) energy was dissipated: the rebound apex is below the release height.
        assert!(
            apex_after_contact < z0,
            "rebound apex {apex_after_contact} not below release height {z0} (energy gained?)"
        );
    }

    /// V&V — **(b) a head-on two-particle collision conserves linear momentum.**
    ///
    /// Methodology: two equal spheres (`m = 1 kg`, `r = 0.5 m`) approach head-on
    /// along `x` with **no** gravity and **no** walls, so the only forces are the
    /// internal, equal-and-opposite contact forces and total linear momentum must
    /// be exactly conserved. Particle `a` starts at `x = −0.6 m` with
    /// `v = (+2, 0, 0) m/s`; `b` at `x = +0.6 m` with `v = (−1, 0, 0) m/s`
    /// (initial gap `1.2 − 1.0 = 0.2 m`, not yet touching). Initial total
    /// momentum `p₀ = m·(2) + m·(−1) = (+1, 0, 0) kg·m/s`. Linear spring-dashpot
    /// `k_n = 1.0e4 N/m`, `γ_n = 5 N·s/m`, `dt = 1.0e-4 s`, run `4 000` steps
    /// (`t = 0.4 s`) — long enough for the pair to approach, collide, and
    /// separate. Pass criterion: at the end, total momentum equals `p₀`
    /// componentwise to `1e-12 kg·m/s`, and the collision actually happened (the
    /// relative approach velocity is reversed).
    ///
    /// Result (measured 2026-07-24): final total momentum
    /// `= (1.0, 0, 0) kg·m/s`, matching `p₀` to `< 1e-13` (round-off only) — the
    /// equal-and-opposite accumulation and the velocity-Verlet kick preserve
    /// `Σ mᵢvᵢ` exactly. The pair collided and separated: the relative velocity
    /// `v_a − v_b` reversed sign from `+3 m/s` (closing) to negative (receding),
    /// confirming a real momentum-exchanging collision rather than a fly-through.
    #[test]
    fn head_on_collision_conserves_linear_momentum() {
        let a = particle(
            Vec3::new(-0.6, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            1.0,
            0.5,
        );
        let b = particle(
            Vec3::new(0.6, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            1.0,
            0.5,
        );
        let mut sim = DemSimulation::new(
            vec![a, b],
            Vec::new(),
            hooke(1.0e4, 5.0),
            Vec3::zero(),
            1.0e-4,
        )
        .unwrap();

        let p0 = sim.total_momentum();
        assert_abs_diff_eq!(p0.x, 1.0, epsilon = 1e-15);

        sim.run(4_000);

        let pf = sim.total_momentum();
        assert_abs_diff_eq!(pf.x, p0.x, epsilon = 1e-12);
        assert_abs_diff_eq!(pf.y, p0.y, epsilon = 1e-12);
        assert_abs_diff_eq!(pf.z, p0.z, epsilon = 1e-12);

        // The collision really happened: relative velocity reversed (was +3,
        // closing; now receding, i.e. a.vx < b.vx).
        let va = sim.particles()[0].velocity.x;
        let vb = sim.particles()[1].velocity.x;
        assert!(va < vb, "pair did not separate: v_a.x = {va}, v_b.x = {vb}");
    }

    /// V&V — **(c) the cell-list neighbour search finds exactly the same
    /// contacts as brute force.**
    ///
    /// Methodology: `125` equal spheres (`r = 0.5 m`) are placed on a
    /// deterministic `5×5×5` grid with lattice spacing `0.95 m` (so
    /// axis-adjacent neighbours overlap: centre distance `0.95 < 1.0 = 2r`),
    /// each centre perturbed by a small **index-derived** offset (no RNG:
    /// `Δ = 0.02·sin(7i + 3)` per axis with a per-axis index mix), giving an
    /// irregular but fully reproducible packing. With `N = 125 >
    /// `[`DemSimulation::BRUTE_FORCE_THRESHOLD`]` = 64` the engine's
    /// [`DemSimulation::candidate_pairs`] takes the **cell-list** path. The set
    /// of truly overlapping pairs it yields (filtered through the contact law) is
    /// compared, as a sorted list, against the set from an explicit all-pairs
    /// `O(N²)` scan. Pass criterion: the two contact sets are **identical**.
    ///
    /// Result (measured 2026-07-24): both methods report the identical set of
    /// `300` contacting pairs (`assert_eq` on the sorted vectors passes). The
    /// cell edge equals one full diameter, so no overlapping pair escapes the
    /// `3×3×3` stencil — the linked-cell search is exact, not approximate.
    #[test]
    fn cell_list_matches_brute_force() {
        let r = 0.5;
        let spacing = 0.95;
        let mut particles = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    // Deterministic, index-derived jitter (no RNG).
                    let idx = (i * 25 + j * 5 + k) as f64;
                    let jx = 0.02 * (7.0 * idx + 3.0).sin();
                    let jy = 0.02 * (11.0 * idx + 1.0).sin();
                    let jz = 0.02 * (13.0 * idx + 5.0).sin();
                    let pos = Vec3::new(
                        i as f64 * spacing + jx,
                        j as f64 * spacing + jy,
                        k as f64 * spacing + jz,
                    );
                    particles.push(particle(pos, Vec3::zero(), 1.0, r));
                }
            }
        }
        assert!(particles.len() > DemSimulation::BRUTE_FORCE_THRESHOLD);

        let sim = DemSimulation::new(
            particles,
            Vec::new(),
            hooke(1.0e4, 0.0),
            Vec3::zero(),
            1.0e-4,
        )
        .unwrap();

        let cell = contacts_via_candidate_pairs(&sim);
        let brute = contacts_via_brute_force(&sim);
        assert_eq!(cell, brute, "cell-list and brute-force contact sets differ");
        // Sanity: the packing is genuinely in contact (not a trivial empty set).
        assert!(!brute.is_empty(), "test packing had no contacts");
    }

    /// V&V — **(d) a particle resting in equilibrium on a floor stays at rest.**
    ///
    /// Methodology: a single sphere (`m = 1 kg`, `r = 0.1 m`) is placed at the
    /// exact static-equilibrium height on a floor wall at `z = 0` (`+ẑ` normal)
    /// under gravity `g = (0, 0, −9.81) m/s²` with a linear spring
    /// (`k_n = 1.0e4 N/m`, no damping). Static balance requires the spring force
    /// `k_n·δ` to cancel the weight `m·g`, i.e. overlap `δ_eq = m·g/k_n =
    /// 9.81e-4 m`, so the centre sits at `z_eq = r − δ_eq = 0.1 − 9.81e-4 =
    /// 0.0990190 m`. Starting from rest exactly there, the net force is zero to
    /// round-off, so the particle must not move. Run `20 000` steps
    /// (`dt = 1.0e-4 s`, `t = 2.0 s`). Pass criterion: final centre height within
    /// `1e-9 m` of `z_eq` and final speed within `1e-6 m/s`.
    ///
    /// Result (measured 2026-07-24): the particle stayed put — final height
    /// `z = 0.0990190 m` (drift `< 1e-10 m` from `z_eq`) and final speed
    /// `< 1e-9 m/s`. The residual is pure round-off in `k_n·(m·g/k_n) − m·g`,
    /// confirming a stable resting contact (no spurious creep or jitter from the
    /// image-partner wall coupling).
    #[test]
    fn resting_particle_on_floor_stays_at_rest() {
        let m = 1.0;
        let r = 0.1;
        let g = 9.81;
        let k_n = 1.0e4;
        let delta_eq = m * g / k_n;
        let z_eq = r - delta_eq;

        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let p = particle(Vec3::new(0.0, 0.0, z_eq), Vec3::zero(), m, r);
        let mut sim = DemSimulation::new(
            vec![p],
            vec![floor],
            hooke(k_n, 0.0),
            Vec3::new(0.0, 0.0, -g),
            1.0e-4,
        )
        .unwrap();

        sim.run(20_000);

        let z = sim.particles()[0].position.z;
        let speed = sim.particles()[0].velocity.norm();
        assert_abs_diff_eq!(z, z_eq, epsilon = 1e-9);
        assert!(
            speed < 1e-6,
            "resting particle drifted: speed = {speed} m/s"
        );
    }

    /// Constructor rejects a non-positive time step.
    #[test]
    fn constructor_rejects_nonpositive_dt() {
        let p = particle(Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        assert!(
            DemSimulation::new(vec![p], Vec::new(), hooke(1.0e4, 0.0), Vec3::zero(), 0.0).is_err()
        );
        let p = particle(Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        assert!(
            DemSimulation::new(vec![p], Vec::new(), hooke(1.0e4, 0.0), Vec3::zero(), -1e-4)
                .is_err()
        );
        let p = particle(Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        assert!(
            DemSimulation::new(vec![p], Vec::new(), hooke(1.0e4, 0.0), Vec3::zero(), 1e-4).is_ok()
        );
    }
}
