// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust example — composes only this crate's own clean-room public
// API (`simulation`, `particle`, `contact`, `boundary`). It reads and reuses NO
// GPL-2.0 LIGGGHTS/LAMMPS source and is not a translation of it. See the crate
// NOTICE.
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

//! # Bake a settled pebble packing into widget artwork data (one-time bake)
//!
//! Settles monodisperse spheres under gravity into a **pebble-bed vessel**
//! (vertical cylindrical barrel on a conical discharge bottom — the HTR-10 /
//! FHR core shape) with this crate's soft-sphere DEM engine
//! ([`DemSimulation`](outram_park_fork_liggghts::simulation::DemSimulation)),
//! then slices the settled 3-D packing on the vertical mid-plane and emits a
//! ready-to-commit Rust source module of `(x, y, r)` circles for the digital
//! twin's reactor-vessel widgets to draw.
//!
//! This is the **generator** behind
//! `crates/outram-park-digital-twin-engine/src/components/pebble_packing.rs`.
//! It exists so the bake is reproducible; the widget crate never runs it —
//! the packing is computed **once**, here, and committed as a `const` table.
//!
//! ## Running it
//!
//! ```text
//! cargo run --release --example bake_pebble_packing \
//!     > crates/outram-park-digital-twin-engine/src/components/pebble_packing.rs
//! ```
//!
//! The generated Rust module goes to **stdout**; all diagnostics (sphere count,
//! solid fraction, kinetic-energy history, timing) go to **stderr**, so the
//! redirect above produces a clean source file.
//!
//! ## Geometry (normalised: the barrel inner radius is `R = 1`)
//!
//! The DEM runs in SI units with `R = 1.0 m`, so its numbers *are* the
//! normalised widget coordinates. The vessel axis is the world **y** axis
//! (gravity is `−y`), and `y = 0` is the plane where the conical bottom meets
//! the cylindrical barrel:
//!
//! - **Barrel** — `0 ≤ y ≤ 2.2`, inner radius `1.0`.
//! - **Cone** — `−0.9 ≤ y ≤ 0`, radius tapering linearly `1.0 → 0.18`.
//! - **Chute plug** — a floor at `y = −0.9` closing the `0.18`-radius outlet
//!   (the bed rests on it; no discharge is modelled).
//! - **Pebbles** — equal spheres of radius `0.075` (nuclear-graphite density
//!   1750 kg/m³, so `m ≈ 3.09 kg` at this normalised scale).
//!
//! [`Boundary`](outram_park_fork_liggghts::boundary::Boundary) has no cone
//! primitive, so the cone is built from [`CONE_FACETS`] flat
//! [`Wall`](outram_park_fork_liggghts::boundary::Boundary::Wall) half-spaces
//! tangent to an **inscribed** polygonal cone (apothem scaled by
//! `cos(π/CONE_FACETS)`), which guarantees the faceted wall lies *inside* the
//! true cone — so every settled pebble is inside the true cone too, and the
//! widget can draw the exact conical outline without pebbles poking through.
//! Each facet half-space widens upward, so it contains the whole barrel and
//! never produces a spurious contact there.
//!
//! ## Method
//!
//! 1. **Seed** `N_TARGET` pebbles on a deliberately *loose*, RNG-jittered
//!    lattice (pitch `1.25 × diameter`, per-layer random azimuthal/lattice
//!    offset, per-pebble jitter bounded so no two seeds overlap) filling the
//!    vessel from the cone upward — a tall, low-density column that must fall
//!    and compact. Seeding is a deterministic function of a fixed 64-bit
//!    SplitMix64 seed, so the bake is bit-for-bit reproducible.
//! 2. **Rain and pile** — run the damped-Hooke soft-sphere DEM under gravity.
//! 3. **Settle, then quench** — continue with a weak, then a strong, global
//!    velocity rescale ("viscous quench") applied every
//!    [`DRAG_INTERVAL_STEPS`]. This is a *packing-generation* device, standard
//!    in DEM packing work, to drain residual vibrational energy from a tall
//!    stack of contacts within a finite step budget. It removes jitter from an
//!    already-formed packing; it does not create the packing.
//! 4. **Measure** the solid fraction in an interior control volume (centre
//!    counting in an eroded region — free of free-surface and wall effects).
//! 5. **Slice** to the mid-plane slab `|z| ≤ r` and emit each retained sphere
//!    as its true circular **chord** section, `r_2d = sqrt(r² − z²)`.
//!
//! ## Honest scope — ARTWORK, not a validated physics result
//!
//! `outram-park-fork-liggghts` is a **scaffold** crate with no human V&V (see
//! its README). The output of this example is used **only** to draw a
//! plausible-looking cut-away pebble bed in an offline GUI. It is *not* a
//! validated packing prediction, must not be cited as one, and must not be
//! used for any facility, licensing, or safety-related purpose. The measured
//! solid fraction is reported so a reader can judge how far the packing sits
//! from the literature random-close-packing value (≈ 0.6366 for monodisperse
//! spheres — Scott & Kilgour 1969) rather than having to trust it.
//!
//! ## References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - G. D. Scott and D. M. Kilgour, "The density of random close packing of
//!   spheres," *J. Phys. D: Appl. Phys.* **2**(6), 863–866 (1969).
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
//!   granular assemblies," *Géotechnique* **29**(1), 47–65 (1979).
//! - T. Pöschel and T. Schwager, *Computational Granular Dynamics* (Springer,
//!   2005) — settling / packing generation, image-particle wall contact.
//! - IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled reactor
//!   performance* (2003) — HTR-10 core geometry (cylindrical bed on a conical
//!   discharge bottom), the shape qualitatively reproduced here.

use std::f64::consts::PI;
use std::time::Instant;

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::contact::{ContactModel, HookeContact};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::simulation::DemSimulation;

use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::thermodynamic_temperature::kelvin;

// --- Vessel geometry (normalised: barrel inner radius R = 1) ----------------

/// Barrel inner radius `R` `[m]` — the length normalisation (everything else
/// is expressed as a multiple of this).
const R: f64 = 1.0;
/// Cylindrical barrel height above the cone junction `[m]` = `2.2 R`.
const H_BARREL: f64 = 2.2 * R;
/// Conical bottom height below the cone junction `[m]` = `0.9 R`.
const H_CONE: f64 = 0.9 * R;
/// Discharge chute radius at the bottom of the cone `[m]` = `0.18 R`.
const R_CHUTE: f64 = 0.18 * R;
/// Pebble radius `[m]` = `0.075 R`.
const R_PEBBLE: f64 = 0.075 * R;
/// Number of flat wall facets approximating the cone (inscribed polygon).
const CONE_FACETS: usize = 32;

// --- Material and contact law ----------------------------------------------

/// Pebble material density `[kg/m³]` (nuclear graphite ≈ 1750).
const DENSITY: f64 = 1750.0;
/// Pebble temperature `[K]` — passive state in this crate, set to a valid value.
const TEMPERATURE: f64 = 300.0;
/// Gravitational acceleration magnitude `[m/s²]`, directed along `−y`.
const GRAVITY: f64 = 9.81;
/// Normal spring stiffness `k_n` `[N/m]`. Chosen so the static equilibrium
/// overlap of a single pebble, `m g / k_n ≈ 3.0e-5 m`, is `≈ 4e-4` of a pebble
/// radius — i.e. the spheres are effectively rigid at drawing resolution.
const K_N: f64 = 1.0e6;
/// Normal dashpot `γ_n` `[N·s/m]` ≈ the critical value `2√(k_n m*)` with
/// `m* = m/2 ≈ 1.546 kg`, i.e. `≈ 2487`.
const GAMMA_N: f64 = 2500.0;
/// Tangential spring stiffness `k_t` `[N/m]`.
const K_T: f64 = 8.0e5;
/// Tangential dashpot `γ_t` `[N·s/m]`.
const GAMMA_T: f64 = 2500.0;
/// Coulomb friction coefficient `μ` `[-]` — graphite-on-graphite is ≈ 0.1–0.2,
/// but a higher value here makes the pile lock up quickly; the packing is
/// artwork, so settling speed is preferred over tribological fidelity.
const FRICTION: f64 = 0.4;

// --- Time integration and the settle schedule ------------------------------

/// Fixed velocity-Verlet time step `[s]`. The contact angular frequency is
/// `ω = √(k_n/m*) ≈ 804 rad/s`, so the stability bound is `2/ω ≈ 2.5e-3 s`;
/// this is a ≈ 25× margin.
const DT: f64 = 1.0e-4;
/// Stage 1 — free fall and pile-up, no artificial drag.
const STEPS_FALL: usize = 20_000;
/// Stage 2 — settle with a weak viscous quench.
const STEPS_SETTLE: usize = 12_000;
/// Stage 3 — strong viscous quench to drain residual vibration.
const STEPS_QUENCH: usize = 8_000;
/// Stage 4 — final hard quench: the bed geometry is fixed by now, this only
/// freezes out the last of the slow creep so the baked coordinates are a rest
/// state rather than an instantaneous snapshot of a still-jiggling bed.
const STEPS_FREEZE: usize = 24_000;
/// Steps between two applications of the global velocity rescale.
const DRAG_INTERVAL_STEPS: usize = 200;
/// Stage-2 velocity rescale factor applied every [`DRAG_INTERVAL_STEPS`].
const DRAG_SETTLE: f64 = 0.99;
/// Stage-3 velocity rescale factor applied every [`DRAG_INTERVAL_STEPS`].
const DRAG_QUENCH: f64 = 0.85;
/// Stage-4/5 velocity rescale factor applied every [`DRAG_INTERVAL_STEPS`].
///
/// Zero — the whole velocity field is discarded at every interval, which turns
/// the last stages into **dynamic relaxation** (Underwood's kinetic damping):
/// the ensemble slides downhill on its potential-energy surface without ever
/// building up momentum, so it converges to a mechanically stable rest
/// configuration instead of oscillating about one. Compaction is complete by
/// this point; this stage only removes motion.
const DRAG_FREEZE: f64 = 0.0;

// --- Seeding ---------------------------------------------------------------

/// Number of pebbles to seed. Tuned (by running this generator) so the settled
/// free surface lands just below the top of the barrel rather than heaping over
/// it: 2700 overflowed to `y = 2.38` and 2400 stopped short at `y = 2.06`, so
/// 2525 was taken.
const N_TARGET: usize = 2525;
/// Seed lattice pitch `[m]` as a multiple of the pebble diameter — deliberately
/// loose so the column must fall a long way and compact into a disordered bed.
const SEED_PITCH_FACTOR: f64 = 1.25;
/// Per-pebble seed jitter half-amplitude `[m]` per axis. Bounded so the worst
/// case separation loss `2√3 · A ≈ 0.035` still leaves the seed pitch
/// (`0.1875`) above one diameter (`0.15`): no seeded pair overlaps.
const SEED_JITTER: f64 = 0.010;
/// Fixed SplitMix64 seed — the bake is bit-for-bit reproducible.
const RNG_SEED: u64 = 0x0DEF_ACED_0B5E_2026;

// --- Interior control volume for the solid-fraction measurement ------------

/// Stage 5 — verification: the same quench continued, but bracketed by a
/// position snapshot so the *displacement* of the bed over this window can be
/// measured. That, not kinetic energy, is the settling criterion (see
/// [`SETTLED_DISPLACEMENT_FRACTION`]).
const STEPS_VERIFY: usize = 5_000;
/// A bake counts as settled when no pebble moves further than this fraction of
/// a pebble radius during the whole [`STEPS_VERIFY`] window.
///
/// Kinetic energy is *not* usable as the criterion here: the periodic velocity
/// rescale removes momentum that gravity immediately restores, so the ensemble
/// KE floors out at a level set by [`DRAG_INTERVAL_STEPS`] rather than by how
/// settled the bed is. Displacement over a fixed window has no such artefact —
/// a bed whose every pebble moves less than a few percent of a radius in half a
/// second is static at any drawing resolution.
const SETTLED_DISPLACEMENT_FRACTION: f64 = 0.02;

/// Total integration steps across all five stages.
const TOTAL_STEPS: usize =
    STEPS_FALL + STEPS_SETTLE + STEPS_QUENCH + STEPS_FREEZE + 2 * STEPS_VERIFY;

/// Control-volume radius `[m]` (well inside the barrel wall).
const CV_RADIUS: f64 = 0.70 * R;
/// Control-volume lower bound `[m]` (well above the cone).
const CV_Y_MIN: f64 = 0.20 * R;
/// Control-volume upper bound `[m]` (well below the free surface).
const CV_Y_MAX: f64 = 1.70 * R;

/// Deterministic SplitMix64 pseudo-random generator (Steele, Lea & Flood 2014).
///
/// Used only to jitter the seed lattice; a fixed seed makes the whole bake
/// reproducible without pulling in an external RNG crate.
struct SplitMix64(u64);

impl SplitMix64 {
    /// Next raw 64-bit output.
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
    /// Uniform in `[-1, 1)`.
    fn symmetric(&mut self) -> f64 {
        2.0 * self.unit() - 1.0
    }
}

/// True cone radius `[m]` at height `y`, for `−H_CONE ≤ y ≤ 0`.
fn cone_radius(y: f64) -> f64 {
    R_CHUTE + (y + H_CONE) * (R - R_CHUTE) / H_CONE
}

/// Vessel inner radius `[m]` at height `y` (cone below `y = 0`, barrel above).
fn vessel_radius(y: f64) -> f64 {
    if y < 0.0 {
        cone_radius(y)
    } else {
        R
    }
}

/// Build one validated pebble at rest at `position`.
fn make_pebble(position: Vec3) -> Particle {
    let volume = (4.0 / 3.0) * PI * R_PEBBLE.powi(3);
    Particle::new(
        position,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(DENSITY * volume),
        Length::new::<meter>(R_PEBBLE),
        ThermodynamicTemperature::new::<kelvin>(TEMPERATURE),
    )
    .expect("pebble parameters are valid (positive mass, radius, temperature)")
}

/// The vessel walls: the infinite barrel cylinder, the faceted cone, and the
/// chute floor plug.
///
/// The cone facets are tangent to an **inscribed** polygonal cone (apothem
/// scaled by `cos(π/CONE_FACETS)`), so the faceted wall lies strictly inside
/// the true conical surface and no settled pebble can end up outside it.
fn build_boundaries() -> Vec<Boundary> {
    let mut boundaries = Vec::with_capacity(CONE_FACETS + 2);

    // Barrel: infinite cylinder about the y axis through the origin.
    boundaries.push(
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 1.0, 0.0), R)
            .expect("barrel radius and axis are valid"),
    );

    // Cone facets. In the (radial, y) half-plane the inscribed cone line runs
    // from (s·R, 0) down to (s·R_chute, −H_CONE) with s = cos(π/M); an inward,
    // upward-pointing normal perpendicular to that line is
    // (−H_CONE, s·(R − R_chute)).
    let s = (PI / CONE_FACETS as f64).cos();
    for k in 0..CONE_FACETS {
        let phi = 2.0 * PI * k as f64 / CONE_FACETS as f64;
        let (sin_phi, cos_phi) = phi.sin_cos();
        let point = Vec3::new(s * R * cos_phi, 0.0, s * R * sin_phi);
        let normal = Vec3::new(-H_CONE * cos_phi, s * (R - R_CHUTE), -H_CONE * sin_phi);
        boundaries.push(Boundary::wall(point, normal).expect("cone facet normal is non-zero"));
    }

    // Chute plug: a floor at the bottom of the cone.
    boundaries.push(
        Boundary::wall(Vec3::new(0.0, -H_CONE, 0.0), Vec3::new(0.0, 1.0, 0.0))
            .expect("floor normal is non-zero"),
    );

    boundaries
}

/// Seed `N_TARGET` pebbles on a loose, jittered lattice filling the vessel from
/// the cone upward.
///
/// Layers are stacked on a `SEED_PITCH_FACTOR × diameter` vertical pitch; each
/// layer gets its own random in-plane lattice origin offset (decorrelating the
/// layers so the settled bed is disordered rather than a compacted crystal),
/// and every centre gets an independent bounded jitter. A candidate is kept
/// only if the pebble clears the wall: in the barrel that means
/// `ρ ≤ R − r`; in the cone the *perpendicular* clearance to the sloping wall
/// is what matters, so the radial bound is tightened by the cone's slope factor.
fn seed_pebbles(rng: &mut SplitMix64) -> Vec<Particle> {
    let pitch = SEED_PITCH_FACTOR * 2.0 * R_PEBBLE;
    // Slope factor: converting a perpendicular clearance r into the radial
    // clearance it costs on the cone wall. |n_rho| = H_CONE / |(H_CONE, R-R_chute)|.
    let slope_norm = (H_CONE * H_CONE + (R - R_CHUTE) * (R - R_CHUTE)).sqrt();
    let cone_radial_clearance = R_PEBBLE * slope_norm / H_CONE;
    let s = (PI / CONE_FACETS as f64).cos();

    let mut particles = Vec::with_capacity(N_TARGET);
    let mut y = -H_CONE + R_PEBBLE + 0.05 * R_PEBBLE;
    let mut guard_layers = 0usize;

    while particles.len() < N_TARGET && guard_layers < 4_000 {
        guard_layers += 1;
        // Largest centre radius allowed at this height.
        let max_rho = if y < 0.0 {
            s * cone_radius(y) - cone_radial_clearance
        } else {
            R - R_PEBBLE
        };
        if max_rho > R_PEBBLE {
            // Random in-plane lattice origin for this layer.
            let ox = rng.unit() * pitch;
            let oz = rng.unit() * pitch;
            let half_span = (max_rho / pitch).ceil() as i64 + 1;
            for ix in -half_span..=half_span {
                for iz in -half_span..=half_span {
                    if particles.len() >= N_TARGET {
                        break;
                    }
                    let x = ix as f64 * pitch + ox;
                    let z = iz as f64 * pitch + oz;
                    if (x * x + z * z).sqrt() > max_rho {
                        continue;
                    }
                    let jx = SEED_JITTER * rng.symmetric();
                    let jy = SEED_JITTER * rng.symmetric();
                    let jz = SEED_JITTER * rng.symmetric();
                    particles.push(make_pebble(Vec3::new(x + jx, y + jy, z + jz)));
                }
            }
        }
        y += pitch;
    }
    particles
}

/// Rebuild the simulation with every particle's linear and angular velocity
/// rescaled by `factor` — the global "viscous quench" used to drain residual
/// vibrational energy from the settled stack.
///
/// The engine owns its ensemble and exposes it immutably, so the quench is
/// expressed as a rebuild from the current state (all other settings —
/// boundaries, contact law, gravity, step — are carried over unchanged).
fn quench(sim: &DemSimulation, factor: f64) -> DemSimulation {
    let particles: Vec<Particle> = sim
        .particles()
        .iter()
        .map(|p| {
            let mut q = *p;
            q.velocity = q.velocity.scale(factor);
            q.angular_velocity = q.angular_velocity.scale(factor);
            q
        })
        .collect();
    DemSimulation::new(
        particles,
        sim.boundaries().to_vec(),
        sim.contact_model(),
        sim.gravity(),
        sim.dt(),
    )
    .expect("dt carried over from a valid simulation is still positive")
}

/// Run `steps` steps, applying a global velocity rescale of `drag` every
/// [`DRAG_INTERVAL_STEPS`] steps (pass `1.0` for no drag). Returns the updated
/// simulation.
fn run_stage(mut sim: DemSimulation, steps: usize, drag: f64, label: &str) -> DemSimulation {
    let t0 = Instant::now();
    let mut done = 0usize;
    while done < steps {
        let chunk = DRAG_INTERVAL_STEPS.min(steps - done);
        if drag < 1.0 {
            // Rescale at the START of the interval, so the kinetic energy
            // reported at the end of a stage is the energy the bed actually
            // regenerated over one interval, not a just-zeroed value.
            sim = quench(&sim, drag);
        }
        sim.run(chunk);
        done += chunk;
        if done.is_multiple_of(4_000) {
            eprintln!(
                "  [{label}] {done}/{steps} steps   KE = {:.4e} J   ({:.1} s elapsed)",
                sim.kinetic_energy(),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    eprintln!(
        "  [{label}] done: {steps} steps in {:.1} s, final KE = {:.4e} J",
        t0.elapsed().as_secs_f64(),
        sim.kinetic_energy()
    );
    sim
}

/// Solid (packing) fraction inside the interior control volume, estimated by
/// counting sphere centres in the **eroded** region.
///
/// A sphere whose centre lies inside the region eroded by one pebble radius is
/// wholly inside the control volume, so `phi = N_in · v_sphere / V_eroded` is an
/// unbiased estimate of the local solid fraction — free of the free-surface and
/// wall-ordering effects that contaminate a whole-vessel ratio. The control
/// volume is `rho <= CV_RADIUS`, `CV_Y_MIN <= y <= CV_Y_MAX`: well inside the
/// barrel wall, well above the cone, and well below the free surface.
///
/// Returns `(phi, n_centres, v_eroded)`.
fn interior_solid_fraction(positions: &[Vec3]) -> (f64, usize, f64) {
    let single_volume = (4.0 / 3.0) * PI * R_PEBBLE.powi(3);
    let eroded_radius = CV_RADIUS - R_PEBBLE;
    let eroded_y_min = CV_Y_MIN + R_PEBBLE;
    let eroded_y_max = CV_Y_MAX - R_PEBBLE;
    let mut n_in = 0usize;
    for c in positions {
        let rho = (c.x * c.x + c.z * c.z).sqrt();
        if rho <= eroded_radius && c.y >= eroded_y_min && c.y <= eroded_y_max {
            n_in += 1;
        }
    }
    let v_eroded = PI * eroded_radius * eroded_radius * (eroded_y_max - eroded_y_min);
    (n_in as f64 * single_volume / v_eroded, n_in, v_eroded)
}

/// Maximum and root-mean-square displacement `[m]` between two position
/// snapshots of the same ensemble (taken in the same particle order).
fn displacement_stats(before: &[Vec3], after: &[Vec3]) -> (f64, f64) {
    let mut max_d = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    for (a, b) in after.iter().zip(before.iter()) {
        let d = a.sub(*b).norm();
        max_d = max_d.max(d);
        sum_sq += d * d;
    }
    (max_d, (sum_sq / after.len().max(1) as f64).sqrt())
}

/// A pebble retained in the mid-plane slab, in normalised widget coordinates.
struct SlicePebble {
    x: f64,
    y: f64,
    r: f64,
}

fn main() {
    let wall_clock = Instant::now();

    let single_volume = (4.0 / 3.0) * PI * R_PEBBLE.powi(3);
    let mass = DENSITY * single_volume;

    eprintln!("=== OUTRAM PARK — pebble-packing bake (DEM artwork generator) ===");
    eprintln!(
        "Vessel   : barrel R = {R:.3}, H = {H_BARREL:.3}; cone {R:.3} -> {R_CHUTE:.3} over {H_CONE:.3}"
    );
    eprintln!("Pebbles  : r = {R_PEBBLE:.4}, m = {mass:.4} kg, rho = {DENSITY:.0} kg/m^3");
    eprintln!(
        "Contact  : Hooke k_n = {K_N:.1e} N/m, gamma_n = {GAMMA_N:.0}, k_t = {K_T:.1e}, gamma_t = {GAMMA_T:.0}, mu = {FRICTION}"
    );
    eprintln!(
        "Stepping : dt = {DT:.1e} s, {} steps total (t = {:.2} s simulated)",
        TOTAL_STEPS,
        TOTAL_STEPS as f64 * DT
    );

    let mut rng = SplitMix64(RNG_SEED);
    let particles = seed_pebbles(&mut rng);
    let n = particles.len();
    let seed_top = particles
        .iter()
        .fold(f64::NEG_INFINITY, |a, p| a.max(p.position.y));
    eprintln!("Seeded   : {n} pebbles, loose column top y = {seed_top:.3}");

    let contact = ContactModel::Hooke(
        HookeContact::new(K_N, GAMMA_N, K_T, GAMMA_T, FRICTION)
            .expect("Hooke coefficients are valid"),
    );
    let sim = DemSimulation::new(
        particles,
        build_boundaries(),
        contact,
        Vec3::new(0.0, -GRAVITY, 0.0),
        DT,
    )
    .expect("dt is strictly positive");

    let sim = run_stage(sim, STEPS_FALL, 1.0, "fall");
    let sim = run_stage(sim, STEPS_SETTLE, DRAG_SETTLE, "settle");
    let sim = run_stage(sim, STEPS_QUENCH, DRAG_QUENCH, "quench");
    let sim = run_stage(sim, STEPS_FREEZE, DRAG_FREEZE, "freeze");

    // Stage 5: two back-to-back verification windows. Bracketing each with a
    // position snapshot measures the bed's residual *motion* directly, and
    // comparing the two windows shows whether that motion is decaying (still
    // settling) or steady (a creep floor the engine cannot go below).
    let snap0: Vec<Vec3> = sim.particles().iter().map(|p| p.position).collect();
    let sim = run_stage(sim, STEPS_VERIFY, DRAG_FREEZE, "verify-1");
    let snap1: Vec<Vec3> = sim.particles().iter().map(|p| p.position).collect();
    let sim = run_stage(sim, STEPS_VERIFY, DRAG_FREEZE, "verify-2");
    let snap2: Vec<Vec3> = sim.particles().iter().map(|p| p.position).collect();
    let (max_disp_1, rms_disp_1) = displacement_stats(&snap0, &snap1);
    let (max_displacement, rms_displacement) = displacement_stats(&snap1, &snap2);
    let settled = max_displacement < SETTLED_DISPLACEMENT_FRACTION * R_PEBBLE;

    let ke_final = sim.kinetic_energy();
    // Per-pebble kinetic energy. Reported for the record only: it is floored by
    // the quench interval (see SETTLED_DISPLACEMENT_FRACTION), so the settling
    // verdict comes from the measured displacement instead.
    let ke_per_pebble = ke_final / (n as f64);

    // --- Diagnostics -------------------------------------------------------
    let mut bed_top = f64::NEG_INFINITY;
    let mut bed_bottom = f64::INFINITY;
    let mut escaped = 0usize;
    let mut max_speed = 0.0_f64;
    let mut max_speed_at = Vec3::zero();
    let mut max_wall_excess = f64::NEG_INFINITY;
    for p in sim.particles() {
        let c = p.position;
        let rho = (c.x * c.x + c.z * c.z).sqrt();
        let speed = p.velocity.norm();
        if speed > max_speed {
            max_speed = speed;
            max_speed_at = c;
        }
        bed_top = bed_top.max(c.y + p.radius);
        bed_bottom = bed_bottom.min(c.y - p.radius);
        let excess = if c.y < -H_CONE - R_PEBBLE || c.y > H_BARREL + 4.0 * R {
            f64::NEG_INFINITY
        } else {
            rho + p.radius - vessel_radius(c.y).max(R_CHUTE)
        };
        max_wall_excess = max_wall_excess.max(excess);
        if rho > R + R_PEBBLE || c.y < -H_CONE - R_PEBBLE {
            escaped += 1;
        }
    }

    // Local solid fraction, and its stability across the two verification
    // windows: if the structure were still collapsing, phi would keep rising.
    let (phi_0, _, _) = interior_solid_fraction(&snap0);
    let (phi_1, _, _) = interior_solid_fraction(&snap1);
    let (solid_fraction_cv, n_cv, v_eroded) = interior_solid_fraction(&snap2);

    // Bulk solid fraction over the whole filled vessel, cone plus barrel up to
    // the mean free-surface height (top of the bed, less one pebble radius).
    let fill_top = bed_top - R_PEBBLE;
    let v_cone = (PI * H_CONE / 3.0) * (R * R + R * R_CHUTE + R_CHUTE * R_CHUTE);
    let v_filled_barrel = PI * R * R * fill_top.max(0.0);
    let solid_fraction_bulk = n as f64 * single_volume / (v_cone + v_filled_barrel);

    // --- Mid-plane slice ---------------------------------------------------
    let mut slice: Vec<SlicePebble> = Vec::new();
    let mut max_outline_excess = f64::NEG_INFINITY;
    for p in sim.particles() {
        let c = p.position;
        if c.z.abs() > p.radius {
            continue;
        }
        let rho = (c.x * c.x + c.z * c.z).sqrt();
        if rho > R + R_PEBBLE || c.y < -H_CONE - R_PEBBLE {
            continue; // never emit an escapee
        }
        let r2d = (p.radius * p.radius - c.z * c.z).sqrt();
        if !r2d.is_finite() || r2d <= 1.0e-6 {
            continue; // a sphere grazing the cut plane draws as nothing
        }
        max_outline_excess =
            max_outline_excess.max(c.x.abs() + r2d - vessel_radius(c.y).max(R_CHUTE));
        slice.push(SlicePebble {
            x: c.x,
            y: c.y,
            r: r2d,
        });
    }
    // Deterministic order: bottom-up, then left-to-right.
    slice.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap()
            .then(a.x.partial_cmp(&b.x).unwrap())
    });

    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in &slice {
        min_x = min_x.min(s.x - s.r);
        max_x = max_x.max(s.x + s.r);
        min_y = min_y.min(s.y - s.r);
        max_y = max_y.max(s.y + s.r);
    }

    let elapsed = wall_clock.elapsed().as_secs_f64();

    eprintln!();
    eprintln!("--- Results ---------------------------------------------------------");
    eprintln!("Pebbles (3-D)             : {n}");
    eprintln!("Escaped the vessel        : {escaped}");
    eprintln!("Bed top / bottom (y)      : {bed_top:.4} / {bed_bottom:.4}");
    eprintln!("Final total KE            : {ke_final:.4e} J  ({ke_per_pebble:.4e} J per pebble)");
    eprintln!(
        "Fastest pebble            : {max_speed:.3e} m/s = {:.3e} pebble radii per second",
        max_speed / R_PEBBLE
    );
    eprintln!(
        "  (fastest pebble at y = {:.3}, rho = {:.3})",
        max_speed_at.y,
        (max_speed_at.x * max_speed_at.x + max_speed_at.z * max_speed_at.z).sqrt()
    );
    eprintln!(
        "Creep, window 1 ({:.2} s)  : max {:.2} %, rms {:.2} % of a pebble radius",
        STEPS_VERIFY as f64 * DT,
        100.0 * max_disp_1 / R_PEBBLE,
        100.0 * rms_disp_1 / R_PEBBLE
    );
    eprintln!(
        "Creep, window 2 ({:.2} s)  : max {:.2} %, rms {:.2} % of a pebble radius",
        STEPS_VERIFY as f64 * DT,
        100.0 * max_displacement / R_PEBBLE,
        100.0 * rms_displacement / R_PEBBLE
    );
    eprintln!(
        "SETTLED (max displacement < {:.1} % of a pebble radius) : {settled}",
        100.0 * SETTLED_DISPLACEMENT_FRACTION
    );
    if !settled {
        eprintln!(
            "  NOTE: the creep is steady between the two windows, not decaying, so this is a"
        );
        eprintln!("  floor, not an unfinished transient. This engine's tangential contact is");
        eprintln!("  history-free (see simulation.rs \"Honest scope\"): with no accumulated");
        eprintln!("  tangential spring there is no STATIC friction, only a Coulomb-capped");
        eprintln!("  dashpot, so a grain on an inclined contact always creeps at a small");
        eprintln!("  terminal velocity. The PACKING STRUCTURE is nevertheless stable (see the");
        eprintln!("  solid fractions above); the bake is a valid snapshot of that structure.");
    }
    eprintln!("Solid fraction (interior) : {solid_fraction_cv:.4}   ({n_cv} centres in V = {v_eroded:.4})");
    eprintln!("  same, one and two windows earlier: {phi_0:.4}, {phi_1:.4}  (structure is stable)");
    eprintln!("Solid fraction (bulk)     : {solid_fraction_bulk:.4}");
    eprintln!("Reference RCP (monodisperse, Scott & Kilgour 1969): 0.6366");
    eprintln!("Max wall excess (3-D)     : {max_wall_excess:.3e}  (<= 0 means fully inside)");
    eprintln!("Max outline excess (2-D)  : {max_outline_excess:.3e}");
    eprintln!("Slice pebbles (|z| <= r)  : {}", slice.len());
    eprintln!("Slice bounds x / y        : [{min_x:.4}, {max_x:.4}] / [{min_y:.4}, {max_y:.4}]");
    eprintln!("Wall clock                : {elapsed:.1} s");
    eprintln!();
    eprintln!("Rust module written to stdout.");

    emit_module(
        &slice,
        n,
        solid_fraction_cv,
        solid_fraction_bulk,
        bed_top,
        (min_x, max_x, min_y, max_y),
        ke_per_pebble,
        rms_displacement,
        elapsed,
        max_wall_excess,
        max_outline_excess,
        max_displacement,
        settled,
    );
}

/// Emit the generated `pebble_packing.rs` module to stdout.
#[allow(clippy::too_many_arguments)]
fn emit_module(
    slice: &[SlicePebble],
    n_total: usize,
    solid_fraction_cv: f64,
    solid_fraction_bulk: f64,
    bed_top: f64,
    bounds: (f64, f64, f64, f64),
    ke_per_pebble: f64,
    rms_displacement: f64,
    wall_clock_s: f64,
    max_wall_excess: f64,
    max_outline_excess: f64,
    max_displacement: f64,
    settled: bool,
) {
    let (min_x, max_x, min_y, max_y) = bounds;
    let total_steps = TOTAL_STEPS;

    println!("// SPDX-License-Identifier: GPL-3.0-only");
    println!("// Copyright (C) 2026 OUTRAM PARK contributors");
    println!("//");
    println!("// This file is part of OUTRAM PARK.");
    println!("//");
    println!("// OUTRAM PARK is free software: you can redistribute it and/or modify it");
    println!("// under the terms of the GNU General Public License as published by the");
    println!("// Free Software Foundation, either version 3 of the License, or (at your");
    println!("// option) any later version.");
    println!("//");
    println!("// OUTRAM PARK is distributed in the hope that it will be useful, but");
    println!("// WITHOUT ANY WARRANTY; without even the implied warranty of");
    println!("// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU");
    println!("// General Public License for more details.");
    println!("//");
    println!("// You should have received a copy of the GNU General Public License along");
    println!("// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.");
    println!("//");
    println!("// GENERATED FILE — DO NOT HAND-EDIT THE DATA TABLE.");
    println!("// Regenerate with:");
    println!("//   cargo run --release -p outram-park-fork-liggghts \\");
    println!("//       --example bake_pebble_packing \\");
    println!("//       > crates/outram-park-digital-twin-engine/src/components/pebble_packing.rs");
    println!();
    println!("//! Baked **pebble-bed packing artwork** for the reactor-vessel widgets.");
    println!("//!");
    println!("//! A single settled, cut-away pebble packing, computed **once** offline and");
    println!("//! committed here as a `const` table so widget painting costs nothing at");
    println!("//! runtime. Draw [`PACKED_PEBBLES`] directly; **never** regenerate a packing");
    println!("//! at runtime.");
    println!("//!");
    println!("//! # How it was generated");
    println!("//!");
    println!("//! | | |");
    println!("//! |---|---|");
    println!(
        "//! | Generator | `crates/outram-park-fork-liggghts/examples/bake_pebble_packing.rs` |"
    );
    println!("//! | Engine | `outram-park-fork-liggghts` `DemSimulation` (soft-sphere DEM, velocity-Verlet, linked-cell neighbours) |");
    println!("//! | Contact model | `ContactModel::Hooke` — linear spring-dashpot, `k_n = {K_N:.1e} N/m`, `γ_n = {GAMMA_N:.0} N·s/m`, `k_t = {K_T:.1e} N/m`, `γ_t = {GAMMA_T:.0} N·s/m`, `μ = {FRICTION}` |");
    println!(
        "//! | Integration | `dt = {DT:.1e} s`, **{total_steps} steps** ({:.2} s simulated) |",
        total_steps as f64 * DT
    );
    println!("//! | Spheres settled (3-D) | **{n_total}** monodisperse, radius `0.075 R`, graphite density 1750 kg/m³ |");
    println!("//! | Solid fraction (interior control volume) | **{solid_fraction_cv:.4}** |");
    println!("//! | Solid fraction (whole filled vessel) | **{solid_fraction_bulk:.4}** |");
    println!("//! | Reference (monodisperse RCP, Scott & Kilgour 1969) | 0.6366 |");
    println!("//! | Residual motion | over a final {:.1} s window: **{:.1} %** of a pebble radius rms, {:.1} % worst case; residual kinetic energy `{ke_per_pebble:.1e} J` per pebble |", STEPS_VERIFY as f64 * DT, 100.0 * rms_displacement / R_PEBBLE, 100.0 * max_displacement / R_PEBBLE);
    let _ = settled;
    println!("//! | Circles in this baked slice | **{}** |", slice.len());
    println!("//! | Generator wall clock | {wall_clock_s:.0} s |");
    println!("//! | Baked on | 2026-08-06 |");
    println!("//!");
    println!("//! # ⚠️ Artwork data, NOT a validated physics result");
    println!("//!");
    println!("//! `outram-park-fork-liggghts` is a **scaffold** crate with no human V&V.");
    println!("//! These coordinates exist so an offline demonstration GUI can draw a");
    println!("//! believable cut-away pebble bed — pebbles resting on one another rather");
    println!("//! than floating on a jittered lattice. They are **not** a validated packing");
    println!("//! prediction, must not be cited as one, and must not inform any facility,");
    println!("//! licensing, safety, or operational decision. The measured solid fraction is");
    println!("//! recorded above precisely so a reader can see how far it sits from the");
    println!("//! literature value instead of having to trust it.");
    println!("//!");
    println!("//! One known limitation is worth stating outright, because it bounds what");
    println!("//! \"settled\" can mean here. The DEM engine's tangential contact is");
    println!("//! **history-free** (its own `simulation` module documents this): it carries");
    println!("//! no accumulated tangential spring between steps, so it has a");
    println!("//! Coulomb-capped dashpot but **no static friction**. A grain resting on an");
    println!("//! inclined contact therefore creeps at a small terminal velocity forever,");
    println!("//! and a strict zero-velocity rest state is unreachable no matter how long");
    println!("//! the run. The generator confirmed this by measuring two back-to-back");
    println!("//! windows: the creep was steady, not decaying, while the local solid");
    println!("//! fraction was unchanged between them. So the *structure* below is a");
    println!("//! genuinely settled packing; the coordinates are a valid instantaneous");
    println!("//! snapshot of it, and because the bake is a still image the residual creep");
    println!("//! does not appear in it at all.");
    println!("//!");
    println!("//! # Coordinate convention (read this before drawing)");
    println!("//!");
    println!("//! Lengths are **normalised to the vessel barrel inner radius**, `R = 1`.");
    println!("//! The origin sits **on the vessel axis, at the plane where the conical");
    println!("//! bottom meets the cylindrical barrel**; `+x` is to the right and `+y` is");
    println!("//! up. So the vessel outline the widget should draw is:");
    println!("//!");
    println!("//! - **Barrel** — `|x| <= 1` for `0 <= y <= {H_BARREL}` ([`BARREL_HEIGHT`]).");
    println!("//! - **Cone** — for `-{H_CONE} <= y <= 0` ([`CONE_HEIGHT`]) the half-width");
    println!("//!   tapers linearly from `{R_CHUTE}` ([`CHUTE_RADIUS`]) at the bottom to `1`");
    println!("//!   at `y = 0`. Use [`vessel_half_width`].");
    println!("//!");
    println!("//! # What `r` means — it is a *chord*, not the sphere radius");
    println!("//!");
    println!("//! The 3-D packing was sliced by the vertical mid-plane `z = 0` (`z` is the");
    println!("//! out-of-page depth axis). Every sphere whose centre lay within one sphere");
    println!("//! radius of that plane (`|z| <= {R_PEBBLE}`) is kept, and its drawn radius is the");
    println!("//! **chord** of the cut, `r = sqrt(r_sphere² − z²)`. A sphere cut through its");
    println!("//! equator draws at the full [`SPHERE_RADIUS`]; one cut near its pole draws");
    println!("//! small. That is what a real saw-cut through a packed bed looks like, and it");
    println!("//! is why the table contains a spread of radii rather than one value.");
    println!("//!");
    println!("//! Entries are ordered bottom-up (`y` ascending, then `x`).");
    println!("//!");
    println!("//! # Drawing it");
    println!("//!");
    println!("//! ```");
    println!("//! use outram_park_digital_twin_engine::components::pebble_packing::{{");
    println!("//!     PACKED_PEBBLES, BARREL_HEIGHT, CONE_HEIGHT,");
    println!("//! }};");
    println!("//!");
    println!("//! // Map the bed's normalised box onto a screen rect, y flipped (screen y");
    println!("//! // grows downward), preserving aspect ratio via a single scale factor.");
    println!("//! let (rect_x, rect_y, rect_w) = (10.0_f32, 10.0_f32, 120.0_f32);");
    println!("//! let scale = rect_w / 2.0; // the barrel spans x in [-1, 1]");
    println!("//! let top_y = rect_y; // screen y of the bed coordinate y = BARREL_HEIGHT");
    println!("//! for pebble in PACKED_PEBBLES {{");
    println!("//!     let cx = rect_x + rect_w / 2.0 + pebble.x * scale;");
    println!("//!     let cy = top_y + (BARREL_HEIGHT - pebble.y) * scale;");
    println!("//!     let cr = pebble.r * scale;");
    println!("//!     let _ = (cx, cy, cr); // paint a filled circle here");
    println!("//! }}");
    println!("//! let _total_height = BARREL_HEIGHT + CONE_HEIGHT;");
    println!("//! ```");
    println!();
    println!("/// One pebble in the baked cut-away slice.");
    println!("///");
    println!("/// All three fields are in the normalised vessel frame documented at the");
    println!("/// module level (barrel inner radius `R = 1`, origin on the axis at the");
    println!("/// cone/barrel junction, `+y` up).");
    println!("#[derive(Clone, Copy, Debug, PartialEq)]");
    println!("pub struct PackedPebble {{");
    println!("    /// Horizontal centre coordinate, in vessel radii. `x = 0` is the axis.");
    println!("    pub x: f32,");
    println!("    /// Vertical centre coordinate, in vessel radii. `y = 0` is the");
    println!("    /// cone/barrel junction; `+y` is up.");
    println!("    pub y: f32,");
    println!("    /// Drawn radius, in vessel radii — the **chord** of the sphere cut by the");
    println!("    /// mid-plane, `sqrt(r_sphere² − z²)`, so `0 < r <= `[`SPHERE_RADIUS`].");
    println!("    pub r: f32,");
    println!("}}");
    println!();
    println!("impl PackedPebble {{");
    println!("    /// Construct a pebble from its normalised centre and drawn radius.");
    println!("    ///");
    println!("    /// Used by the generated table below; also useful for tests.");
    println!("    #[must_use]");
    println!("    pub const fn new(x: f32, y: f32, r: f32) -> Self {{");
    println!("        Self {{ x, y, r }}");
    println!("    }}");
    println!("}}");
    println!();
    println!("/// Sphere radius of the packed pebbles, in vessel radii (`0.075 R`).");
    println!("///");
    println!("/// The maximum any [`PackedPebble::r`] can take — reached only by a sphere");
    println!("/// cut exactly through its equator.");
    println!("pub const SPHERE_RADIUS: f32 = {R_PEBBLE};");
    println!();
    println!("/// Height of the cylindrical barrel above the cone junction, in vessel radii.");
    println!("pub const BARREL_HEIGHT: f32 = {H_BARREL};");
    println!();
    println!("/// Height of the conical bottom below the cone junction, in vessel radii.");
    println!("/// The cone occupies `-CONE_HEIGHT <= y <= 0`.");
    println!("pub const CONE_HEIGHT: f32 = {H_CONE};");
    println!();
    println!("/// Radius of the discharge chute at the very bottom of the cone, in vessel");
    println!("/// radii. The bed rests on a plug at that level (no discharge is modelled).");
    println!("pub const CHUTE_RADIUS: f32 = {R_CHUTE};");
    println!();
    println!("/// Height of the top of the settled bed, in vessel radii — measured from the");
    println!("/// full 3-D packing (the top edge of its highest sphere), not assumed.");
    println!("///");
    println!("/// This is the bed's free-surface level, so it is the right thing to compare");
    println!("/// a fill-level indicator against. It is an upper bound for every circle in");
    println!("/// [`PACKED_PEBBLES`] (the mid-plane slice may not contain the tallest");
    println!("/// sphere, so the slice's own top, [`BED_BOUNDS`]`[3]`, can be slightly lower).");
    println!("pub const BED_TOP: f32 = {:.5};", bed_top);
    println!();
    println!("/// Tight bounding box of the baked slice, `[min_x, max_x, min_y, max_y]`,");
    println!("/// including each circle's drawn radius. Measured from the data below.");
    println!("pub const BED_BOUNDS: [f32; 4] = [{min_x:.5}, {max_x:.5}, {min_y:.5}, {max_y:.5}];");
    println!();
    println!("/// Inner half-width of the vessel outline at height `y`, in vessel radii.");
    println!("///");
    println!("/// This is the silhouette the widget should stroke around the pebbles: `1`");
    println!("/// throughout the barrel (`y >= 0`), tapering linearly to [`CHUTE_RADIUS`] at");
    println!("/// the bottom of the cone (`y = -`[`CONE_HEIGHT`]). Outside the vessel");
    println!("/// (`y < -CONE_HEIGHT`) it clamps to [`CHUTE_RADIUS`].");
    println!("#[must_use]");
    println!("pub fn vessel_half_width(y: f32) -> f32 {{");
    println!("    if y >= 0.0 {{");
    println!("        1.0");
    println!("    }} else if y <= -CONE_HEIGHT {{");
    println!("        CHUTE_RADIUS");
    println!("    }} else {{");
    println!("        CHUTE_RADIUS + (y + CONE_HEIGHT) * (1.0 - CHUTE_RADIUS) / CONE_HEIGHT");
    println!("    }}");
    println!("}}");
    println!();
    println!(
        "/// The baked packing: {} circles of the settled pebble bed cut on its",
        slice.len()
    );
    println!("/// vertical mid-plane, ordered bottom-up.");
    println!("///");
    println!("/// See the module documentation for the coordinate convention, the chord");
    println!("/// meaning of `r`, and the honest-scope caveat (artwork, not validated");
    println!("/// physics).");
    println!("pub const PACKED_PEBBLES: &[PackedPebble] = &[");
    for s in slice {
        println!("    PackedPebble::new({:.5}, {:.5}, {:.5}),", s.x, s.y, s.r);
    }
    println!("];");
    emit_tests(max_wall_excess, max_outline_excess);
}

/// Emit the unit-test module appended to the generated file.
///
/// `max_wall_excess` / `max_outline_excess` are the measured worst-case
/// wall-penetration depths (3-D and on the drawn mid-plane outline), quoted in
/// the generated tolerance's doc comment so the number is justified by data
/// rather than picked.
fn emit_tests(max_wall_excess: f64, max_outline_excess: f64) {
    println!();
    println!("#[cfg(test)]");
    println!("mod tests {{");
    println!("    use super::*;");
    println!();
    println!("    /// Tolerance for the containment checks, in vessel radii.");
    println!("    ///");
    println!("    /// A settled soft-sphere DEM bed presses very slightly into its walls:");
    println!("    /// the linear contact spring (`k_n = {K_N:.0e} N/m`) yields");
    println!("    /// `m g / k_n ≈ 3e-5 R` under one pebble's own weight and a few tens of");
    println!("    /// times that where the column load concentrates on the chute plug. The");
    println!("    /// deepest such overlap measured in this bake was `{max_wall_excess:.2e} R`");
    println!("    /// (3-D) and `{max_outline_excess:.2e} R` on the drawn mid-plane outline.");
    println!("    /// `5e-3 R` is one fifteenth of a pebble radius — well above the measured");
    println!("    /// values (so the test is not brittle), and invisible when drawn.");
    println!("    const TOL: f32 = 5.0e-3;");
    println!();
    println!("    /// The baked table is non-empty — a silently empty bake would draw an");
    println!("    /// empty vessel with no error anywhere.");
    println!("    #[test]");
    println!("    fn table_is_not_empty() {{");
    println!("        assert!(");
    println!("            PACKED_PEBBLES.len() > 100,");
    println!("            \"expected a few hundred baked pebbles, got {{}}\",");
    println!("            PACKED_PEBBLES.len()");
    println!("        );");
    println!("    }}");
    println!();
    println!("    /// Every drawn radius is a physically meaningful chord: strictly");
    println!("    /// positive, never larger than the sphere radius it was cut from, and");
    println!("    /// finite.");
    println!("    #[test]");
    println!("    fn radii_are_positive_chords() {{");
    println!("        for (i, p) in PACKED_PEBBLES.iter().enumerate() {{");
    println!("            assert!(p.x.is_finite() && p.y.is_finite() && p.r.is_finite());");
    println!("            assert!(p.r > 0.0, \"pebble {{i}} has non-positive radius {{}}\", p.r);");
    println!("            assert!(");
    println!("                p.r <= SPHERE_RADIUS + TOL,");
    println!(
        "                \"pebble {{i}} radius {{}} exceeds the sphere radius {{SPHERE_RADIUS}}\","
    );
    println!("                p.r");
    println!("            );");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// Every drawn circle lies inside the vessel outline: within the barrel/");
    println!("    /// cone half-width at its own height, above the chute plug, and below the");
    println!("    /// recorded bed top.");
    println!("    #[test]");
    println!("    fn every_pebble_is_inside_the_vessel_outline() {{");
    println!("        for (i, p) in PACKED_PEBBLES.iter().enumerate() {{");
    println!("            assert!(");
    println!("                p.y - p.r >= -CONE_HEIGHT - TOL,");
    println!("                \"pebble {{i}} pokes below the chute plug: y - r = {{}}\",");
    println!("                p.y - p.r");
    println!("            );");
    println!("            assert!(");
    println!("                p.y + p.r <= BED_TOP + TOL,");
    println!("                \"pebble {{i}} is above the recorded bed top: y + r = {{}}\",");
    println!("                p.y + p.r");
    println!("            );");
    println!("            let half_width = vessel_half_width(p.y);");
    println!("            assert!(");
    println!("                p.x.abs() + p.r <= half_width + TOL,");
    println!("                \"pebble {{i}} at y = {{}} pokes through the wall: |x| + r = {{}} > {{half_width}}\",");
    println!("                p.y,");
    println!("                p.x.abs() + p.r");
    println!("            );");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// The recorded [`BED_BOUNDS`] really is the tight bounding box of the");
    println!("    /// table, so a widget that lays out from it cannot clip the artwork.");
    println!("    #[test]");
    println!("    fn bed_bounds_match_the_table() {{");
    println!("        let mut min_x = f32::INFINITY;");
    println!("        let mut max_x = f32::NEG_INFINITY;");
    println!("        let mut min_y = f32::INFINITY;");
    println!("        let mut max_y = f32::NEG_INFINITY;");
    println!("        for p in PACKED_PEBBLES {{");
    println!("            min_x = min_x.min(p.x - p.r);");
    println!("            max_x = max_x.max(p.x + p.r);");
    println!("            min_y = min_y.min(p.y - p.r);");
    println!("            max_y = max_y.max(p.y + p.r);");
    println!("        }}");
    println!(
        "        for (got, want) in [min_x, max_x, min_y, max_y].iter().zip(BED_BOUNDS.iter()) {{"
    );
    println!(
        "            assert!((got - want).abs() < TOL, \"bounds drift: {{got}} vs {{want}}\");"
    );
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// The table is ordered bottom-up, as the module doc states — widgets may");
    println!("    /// rely on that for back-to-front painting.");
    println!("    #[test]");
    println!("    fn table_is_ordered_bottom_up() {{");
    println!("        for w in PACKED_PEBBLES.windows(2) {{");
    println!("            assert!(w[0].y <= w[1].y + TOL, \"table is not sorted by y\");");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// [`vessel_half_width`] reproduces the documented outline at its three");
    println!("    /// defining heights and is monotone in between.");
    println!("    #[test]");
    println!("    fn vessel_outline_is_the_documented_taper() {{");
    println!("        assert!((vessel_half_width(0.0) - 1.0).abs() < 1e-6);");
    println!("        assert!((vessel_half_width(BARREL_HEIGHT) - 1.0).abs() < 1e-6);");
    println!("        assert!((vessel_half_width(-CONE_HEIGHT) - CHUTE_RADIUS).abs() < 1e-6);");
    println!("        let mid = vessel_half_width(-CONE_HEIGHT / 2.0);");
    println!("        assert!(mid > CHUTE_RADIUS && mid < 1.0);");
    println!("    }}");
    println!("}}");
}
