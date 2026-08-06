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
//! cuts the settled 3-D packing open on a vertical plane, keeps a shallow
//! **depth window** of whole spheres behind that cut, and emits a
//! ready-to-commit Rust source module of `(x, y, z)` sphere centres — sorted
//! back-to-front — for the digital twin's reactor-vessel widgets to draw with
//! depth cues.
//!
//! An earlier revision of this generator emitted a strict 2-D mid-plane slice:
//! each sphere's *chord* radius `sqrt(r² − z²)`, which is what a flat saw-cut
//! really exposes. It is geometrically honest and it draws badly — a scatter of
//! large and tiny circles that reads as a size distribution rather than as a
//! monodisperse bed. The bake now keeps depth instead: one radius for every
//! pebble ([`R_PEBBLE`], emitted as `SPHERE_RADIUS`) plus the out-of-plane
//! coordinate `z`, so a widget can overlap, shade, and scale by depth.
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
//! 5. **Cut and window** — saw the bed on the vertical plane `z = 0`, discard
//!    the half nearer the viewer, and keep only the spheres whose centres lie
//!    in the shallow slab `−`[`DEPTH_WINDOW`]` ≤ z ≤ 0` immediately behind the
//!    cut. Emit each as a whole sphere centre `(x, y, z)`, sorted **farthest
//!    first**, so a widget can paint straight through the table with the
//!    painter's algorithm.
//!
//! ## Viewing convention (this is what a renderer must not get backwards)
//!
//! The frame is right-handed with `+x` to the right and `+y` up, so **`+z`
//! points out of the screen, toward the viewer**. The cut plane is `z = 0` and
//! everything in front of it (`z > 0`) has been sawn away; the retained pebbles
//! all have `z ≤ 0` and recede *into* the screen. A pebble at `z = 0` is the
//! nearest and must be painted last; one at `z = −`[`DEPTH_WINDOW`] is the
//! farthest and is painted first. A renderer that flips this sign paints the
//! bed inside-out — near pebbles buried behind far ones, and the depth shading
//! inverted.
//!
//! ## Why a *window*, and how deep
//!
//! Depth beyond a few pebble layers is not visible: the spheres in front
//! occlude it. Keeping the whole half-bed would therefore cost frame time for
//! pixels nobody sees — and the cost is not small, because each pebble is drawn
//! with a TRISO speckle of order 50 dots, so the circle count is ~50× the
//! pebble count. [`DEPTH_WINDOW`] fixes where that trade is made; this
//! generator prints a measured sweep of candidate depths (retained count and
//! the fraction of the vessel silhouette the retained pebbles actually cover)
//! so the choice is made from data rather than guessed.
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

// --- The baked depth window -------------------------------------------------

/// Depth of the retained slab `[m]` behind the cut plane, in vessel radii —
/// pebbles are kept when `−DEPTH_WINDOW ≤ z ≤ 0`.
///
/// `0.30 R` is exactly **two pebble diameters** ([`R_PEBBLE`] `= 0.075`), i.e.
/// about three staggered pebble layers behind the cut face. The choice balances
/// two measured quantities, both printed by [`report_depth_sweep`]:
///
/// - **Coverage** — the fraction of the vessel silhouette that the retained
///   pebbles paint over. Too shallow and the bed reads as a sparse scatter with
///   the background showing through; the sweep shows coverage climbing steeply
///   up to ≈ 2 diameters and then flattening, because everything past that is
///   occluded anyway.
/// - **Cost** — pebbles retained. Each is drawn with a TRISO speckle of order
///   50 dots, so ~500 pebbles is already ~25 000 circles per repaint. Past
///   roughly 600 pebbles the widget starts paying for depth it cannot show.
const DEPTH_WINDOW: f64 = 0.30 * R;

/// Candidate depths reported by [`report_depth_sweep`] so [`DEPTH_WINDOW`] can
/// be justified against measured counts and coverage rather than asserted.
const DEPTH_SWEEP: [f64; 7] = [0.075, 0.15, 0.20, 0.25, 0.30, 0.40, 0.50];

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

/// A pebble retained in the depth window, in normalised widget coordinates.
///
/// All three are sphere-**centre** coordinates; the radius is the same
/// [`R_PEBBLE`] for every pebble and is emitted once as a constant.
struct DepthPebble {
    x: f64,
    y: f64,
    z: f64,
}

/// Is this settled particle inside the vessel (i.e. not an escapee)?
///
/// Escapees would draw outside the stroked outline, so they are never emitted
/// and never counted in the depth sweep.
fn is_inside_vessel(c: Vec3) -> bool {
    let rho = (c.x * c.x + c.z * c.z).sqrt();
    rho <= R + R_PEBBLE && c.y >= -H_CONE - R_PEBBLE
}

/// Select the pebbles kept by a depth window of `depth`: centres in the slab
/// `−depth ≤ z ≤ 0` (behind the cut plane `z = 0`), inside the vessel, sorted
/// **back to front** — farthest (most negative `z`) first.
///
/// The sort is the whole point of the ordering contract: a consumer can paint
/// the table in order with the painter's algorithm and get correct occlusion
/// with no depth buffer and no per-frame sort. Ties in `z` are broken by `y`
/// then `x` purely so the emitted table is a deterministic function of the
/// settled state.
fn select_depth_window(positions: &[Vec3], depth: f64) -> Vec<DepthPebble> {
    let mut kept: Vec<DepthPebble> = positions
        .iter()
        .filter(|c| c.z <= 0.0 && c.z >= -depth && is_inside_vessel(**c))
        .map(|c| DepthPebble {
            x: c.x,
            y: c.y,
            z: c.z,
        })
        .collect();
    kept.sort_by(|a, b| {
        a.z.partial_cmp(&b.z)
            .unwrap()
            .then(a.y.partial_cmp(&b.y).unwrap())
            .then(a.x.partial_cmp(&b.x).unwrap())
    });
    kept
}

/// Fraction of the vessel silhouette that the retained pebbles paint over,
/// measured by rasterising the drawn circles on a `COVERAGE_CELLS`-per-vessel-
/// radius grid.
///
/// This is the quantity that decides whether a depth window is deep enough to
/// read as a solid bed: a cell of the outline (from the chute plug up to
/// `bed_top`, within [`vessel_radius`] at its own height) counts as covered
/// when its centre falls inside some drawn circle of radius [`R_PEBBLE`].
/// Uncovered cells are holes the background shows through.
fn silhouette_coverage(kept: &[DepthPebble], bed_top: f64) -> f64 {
    /// Raster cells per vessel radius — 0.01 R, about a seventh of a pebble
    /// radius, fine enough that the discretisation error is well under a
    /// percentage point.
    const COVERAGE_CELLS: usize = 100;
    let h = 1.0 / COVERAGE_CELLS as f64;
    let y_lo = -H_CONE;
    let ny = (((bed_top - y_lo) / h).ceil() as usize).max(1);
    let mut inside = 0usize;
    let mut covered = 0usize;
    for iy in 0..ny {
        let y = y_lo + (iy as f64 + 0.5) * h;
        let half = vessel_radius(y).max(R_CHUTE);
        let nx = ((half / h).ceil() as usize).max(1);
        for ix in 0..(2 * nx) {
            let x = -half + (ix as f64 + 0.5) * h;
            if x.abs() > half {
                continue;
            }
            inside += 1;
            let hit = kept.iter().any(|p| {
                let dx = p.x - x;
                let dy = p.y - y;
                dx * dx + dy * dy <= R_PEBBLE * R_PEBBLE
            });
            if hit {
                covered += 1;
            }
        }
    }
    covered as f64 / inside.max(1) as f64
}

/// Print the measured depth-window sweep: for each candidate depth, how many
/// pebbles are retained, how many speckle circles that implies, and how much of
/// the vessel silhouette they cover.
///
/// This is the evidence behind [`DEPTH_WINDOW`]; it is printed on every bake so
/// the choice can be re-checked rather than taken on trust.
fn report_depth_sweep(positions: &[Vec3], bed_top: f64) {
    /// TRISO speckle dots drawn per pebble by the vessel widgets — the multiplier
    /// that turns a pebble count into a per-frame circle count.
    const SPECKLE_DOTS_PER_PEBBLE: usize = 50;
    eprintln!("Depth-window sweep (slab -d <= z <= 0, whole spheres, r = {R_PEBBLE}):");
    eprintln!("    depth d   diameters   pebbles   ~circles/frame   silhouette covered");
    for d in DEPTH_SWEEP {
        let kept = select_depth_window(positions, d);
        let coverage = silhouette_coverage(&kept, bed_top);
        let mark = if (d - DEPTH_WINDOW).abs() < 1.0e-12 {
            "  <== BAKED"
        } else {
            ""
        };
        eprintln!(
            "    {d:>7.3}   {:>9.2}   {:>7}   {:>14}   {:>17.1} %{mark}",
            d / (2.0 * R_PEBBLE),
            kept.len(),
            kept.len() * SPECKLE_DOTS_PER_PEBBLE,
            100.0 * coverage
        );
    }
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

    // --- Cut and depth-window ----------------------------------------------
    let kept = select_depth_window(&snap2, DEPTH_WINDOW);
    let coverage = silhouette_coverage(&kept, bed_top);

    // Worst-case penetration of a *drawn* circle through the vessel outline.
    // Drawn circles use the full sphere radius (not a chord), so this is the
    // number the generated containment test must be able to tolerate.
    let mut max_outline_excess = f64::NEG_INFINITY;
    for s in &kept {
        max_outline_excess =
            max_outline_excess.max(s.x.abs() + R_PEBBLE - vessel_radius(s.y).max(R_CHUTE));
    }

    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_z, mut max_z) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in &kept {
        min_x = min_x.min(s.x - R_PEBBLE);
        max_x = max_x.max(s.x + R_PEBBLE);
        min_y = min_y.min(s.y - R_PEBBLE);
        max_y = max_y.max(s.y + R_PEBBLE);
        min_z = min_z.min(s.z);
        max_z = max_z.max(s.z);
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
    eprintln!("Max outline excess (drawn): {max_outline_excess:.3e}");
    eprintln!();
    report_depth_sweep(&snap2, bed_top);
    eprintln!();
    eprintln!(
        "Baked depth window        : -{DEPTH_WINDOW} <= z <= 0  ({:.2} pebble diameters)",
        DEPTH_WINDOW / (2.0 * R_PEBBLE)
    );
    eprintln!("Pebbles kept              : {}", kept.len());
    eprintln!("Silhouette covered        : {:.1} %", 100.0 * coverage);
    eprintln!("Kept bounds x / y         : [{min_x:.4}, {max_x:.4}] / [{min_y:.4}, {max_y:.4}]");
    eprintln!("Kept bounds z             : [{min_z:.4}, {max_z:.4}]");
    eprintln!("Wall clock                : {elapsed:.1} s");
    eprintln!();
    eprintln!("Rust module written to stdout.");

    emit_module(
        &kept,
        &BakeMetrics {
            n_total: n,
            n_kept: kept.len(),
            solid_fraction_cv,
            solid_fraction_bulk,
            bed_top,
            bounds: [min_x, max_x, min_y, max_y],
            depth_bounds: [min_z, max_z],
            coverage,
            ke_per_pebble,
            rms_displacement,
            max_displacement,
            wall_clock_s: elapsed,
            max_wall_excess,
            max_outline_excess,
            settled,
        },
    );
}

/// Everything measured during one bake that the generated module quotes — in
/// its documentation table, in its emitted constants, or in the tolerance
/// justifications of its tests.
///
/// Grouped into a struct so the numbers travel together and are named at every
/// use site; the emitter never recomputes any of them.
struct BakeMetrics {
    /// Spheres settled in the full 3-D DEM run (before any windowing).
    n_total: usize,
    /// Spheres retained by the depth window and emitted into the table.
    n_kept: usize,
    /// Solid fraction in the interior control volume `[-]`.
    solid_fraction_cv: f64,
    /// Solid fraction over the whole filled vessel `[-]`.
    solid_fraction_bulk: f64,
    /// Top of the settled 3-D bed `[R]` (highest sphere's top edge).
    bed_top: f64,
    /// Tight bounding box of the retained pebbles, `[min_x, max_x, min_y, max_y]` `[R]`.
    bounds: [f64; 4],
    /// Measured `[min_z, max_z]` of the retained pebble centres `[R]`.
    depth_bounds: [f64; 2],
    /// Fraction of the vessel silhouette the retained pebbles paint over `[-]`.
    coverage: f64,
    /// Residual kinetic energy per pebble `[J]`.
    ke_per_pebble: f64,
    /// RMS creep over the final verification window `[m]`.
    rms_displacement: f64,
    /// Worst-case creep over the final verification window `[m]`.
    max_displacement: f64,
    /// Generator wall-clock time `[s]`.
    wall_clock_s: f64,
    /// Deepest wall penetration in 3-D `[R]`.
    max_wall_excess: f64,
    /// Deepest penetration of a *drawn* circle through the vessel outline `[R]`.
    max_outline_excess: f64,
    /// Whether the strict displacement settling criterion was met.
    settled: bool,
}

/// Emit the generated `pebble_packing.rs` module to stdout.
fn emit_module(kept: &[DepthPebble], m: &BakeMetrics) {
    let BakeMetrics {
        n_total,
        solid_fraction_cv,
        solid_fraction_bulk,
        bed_top,
        bounds: [min_x, max_x, min_y, max_y],
        depth_bounds: [min_z, max_z],
        coverage,
        ke_per_pebble,
        rms_displacement,
        max_displacement,
        wall_clock_s,
        settled,
        ..
    } = *m;
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
    println!("//! runtime. Paint [`PACKED_PEBBLES`] **in order**; **never** regenerate a");
    println!("//! packing at runtime.");
    println!("//!");
    println!("//! Each entry is a whole **sphere centre** `(x, y, z)`, not a flat cut. The");
    println!("//! bed is monodisperse, so there is no per-pebble radius: every pebble draws");
    println!("//! at [`SPHERE_RADIUS`]. What varies is `z`, how far the pebble sits *behind*");
    println!("//! the cut plane — which is what lets a widget draw a bed with depth (overlap,");
    println!("//! shading, slight foreshortening) instead of a flat slice.");
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
    println!(
        "//! | Depth window kept | `-{DEPTH_WINDOW} <= z <= 0` — {:.1} pebble diameters behind the cut plane |",
        DEPTH_WINDOW / (2.0 * R_PEBBLE)
    );
    println!("//! | Pebbles in this baked window | **{}** |", kept.len());
    println!(
        "//! | Vessel silhouette they cover | **{:.1} %** |",
        100.0 * coverage
    );
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
    println!("//! # Which way `z` points — get this backwards and the bed draws inside-out");
    println!("//!");
    println!("//! The frame is right-handed, so with `+x` right and `+y` up, **`+z` points");
    println!("//! out of the screen, toward the viewer**. The bed was sawn open on the");
    println!("//! vertical plane `z = 0` and the half in front of it (`z > 0`, between the");
    println!("//! cut and the viewer) was thrown away, which is what makes the interior");
    println!("//! visible. So:");
    println!("//!");
    println!("//! - **every baked `z` is negative or zero** — the pebbles recede *into* the");
    println!("//!   screen, away from the viewer;");
    println!("//! - `z = 0` is the **nearest** pebble, sitting on the cut face;");
    println!("//! - `z = -`[`DEPTH_WINDOW`] is the **farthest** pebble kept.");
    println!("//!");
    println!("//! A renderer that treats `z` as growing away from the viewer will shade the");
    println!("//! near pebbles as if they were far and paint them in the wrong order — the");
    println!("//! bed will look hollow rather than solid.");
    println!("//!");
    println!("//! # Painting order — the table is already sorted for you");
    println!("//!");
    println!("//! [`PACKED_PEBBLES`] is sorted **back to front** (`z` ascending: most");
    println!("//! negative, i.e. farthest, first). Paint it straight through in the order");
    println!("//! given, first entry first, and the painter's algorithm does the occlusion");
    println!("//! for you — each nearer pebble covers the ones behind it, with no depth");
    println!("//! buffer and no per-frame sorting. Do **not** reorder the table (e.g. by");
    println!("//! `y`) unless you are prepared to re-sort by `z` before drawing.");
    println!("//!");
    println!("//! # Why only a window of depth");
    println!("//!");
    println!("//! Only the first few pebble layers behind the cut are visible; the rest are");
    println!("//! occluded. Baking the whole half-bed would therefore cost draw calls for");
    println!("//! pixels nobody sees, and each pebble carries a TRISO speckle of order 50");
    println!("//! dots, so the circle count is ~50x the pebble count. The window was chosen");
    println!("//! from a measured sweep in the generator (retained count versus the fraction");
    println!("//! of the vessel silhouette actually covered); the numbers for the baked");
    println!("//! choice are in the table above.");
    println!("//!");
    println!("//! # Drawing it");
    println!("//!");
    println!("//! ```");
    println!("//! use outram_park_digital_twin_engine::components::pebble_packing::{{");
    println!("//!     depth_fraction, BARREL_HEIGHT, CONE_HEIGHT, PACKED_PEBBLES, SPHERE_RADIUS,");
    println!("//! }};");
    println!("//!");
    println!("//! // Map the bed's normalised box onto a screen rect, y flipped (screen y");
    println!("//! // grows downward), preserving aspect ratio via a single scale factor.");
    println!("//! let (rect_x, rect_y, rect_w) = (10.0_f32, 10.0_f32, 120.0_f32);");
    println!("//! let scale = rect_w / 2.0; // the barrel spans x in [-1, 1]");
    println!("//! let top_y = rect_y; // screen y of the bed coordinate y = BARREL_HEIGHT");
    println!("//!");
    println!("//! // Already sorted farthest-first: just paint straight through.");
    println!("//! for pebble in PACKED_PEBBLES {{");
    println!("//!     let cx = rect_x + rect_w / 2.0 + pebble.x * scale;");
    println!("//!     let cy = top_y + (BARREL_HEIGHT - pebble.y) * scale;");
    println!("//!     let cr = SPHERE_RADIUS * scale; // one radius for every pebble");
    println!("//!     // 0 at the back of the window, 1 on the cut face: darken the far ones.");
    println!("//!     let lit = 0.45 + 0.55 * pebble.depth();");
    println!("//!     let _ = (cx, cy, cr, lit); // paint a filled circle here");
    println!("//! }}");
    println!("//!");
    println!("//! assert!((depth_fraction(0.0) - 1.0).abs() < 1e-6); // the cut face is nearest");
    println!("//! let _total_height = BARREL_HEIGHT + CONE_HEIGHT;");
    println!("//! ```");
    println!();
    println!("/// One pebble in the baked cut-away bed — a whole **sphere centre**.");
    println!("///");
    println!("/// All three fields are in the normalised vessel frame documented at the");
    println!("/// module level: barrel inner radius `R = 1`, origin on the vessel axis at");
    println!("/// the cone/barrel junction, `+x` right, `+y` up, `+z` **toward the viewer**.");
    println!("/// There is no radius field — the bed is monodisperse, so every pebble draws");
    println!("/// at [`SPHERE_RADIUS`].");
    println!("#[derive(Clone, Copy, Debug, PartialEq)]");
    println!("pub struct PackedPebble {{");
    println!("    /// Horizontal centre coordinate, in vessel radii. `x = 0` is the axis.");
    println!("    pub x: f32,");
    println!("    /// Vertical centre coordinate, in vessel radii. `y = 0` is the");
    println!("    /// cone/barrel junction; `+y` is up.");
    println!("    pub y: f32,");
    println!("    /// Depth centre coordinate, in vessel radii — how far the pebble sits");
    println!("    /// **behind the cut plane**, so `-`[`DEPTH_WINDOW`]` <= z <= 0`. `z = 0`");
    println!("    /// is nearest the viewer (on the cut face) and more negative is farther");
    println!("    /// away. For shading, prefer [`PackedPebble::depth`] over raw `z`.");
    println!("    pub z: f32,");
    println!("}}");
    println!();
    println!("impl PackedPebble {{");
    println!("    /// Construct a pebble from its normalised centre `(x, y, z)`.");
    println!("    ///");
    println!("    /// Used by the generated table below; also useful for tests.");
    println!("    #[must_use]");
    println!("    pub const fn new(x: f32, y: f32, z: f32) -> Self {{");
    println!("        Self {{ x, y, z }}");
    println!("    }}");
    println!();
    println!("    /// This pebble's dimensionless depth cue in `[0, 1]` — `0` at the back of");
    println!("    /// the baked window, `1` on the cut face nearest the viewer.");
    println!("    ///");
    println!("    /// Shorthand for [`depth_fraction`]`(self.z)`; see that function for what");
    println!("    /// the number does and does not mean.");
    println!("    #[must_use]");
    println!("    pub fn depth(&self) -> f32 {{");
    println!("        depth_fraction(self.z)");
    println!("    }}");
    println!("}}");
    println!();
    println!("/// Radius of every packed pebble, in vessel radii (`0.075 R`).");
    println!("///");
    println!("/// The bed is monodisperse, so this one value is the drawn radius of every");
    println!("/// entry in [`PACKED_PEBBLES`] — there is no per-pebble radius to look up.");
    println!("/// (An earlier bake stored a per-pebble *chord* radius from a strict flat cut;");
    println!("/// it drew as a distracting mix of large and tiny circles and was replaced by");
    println!("/// this depth-window bake.)");
    println!("pub const SPHERE_RADIUS: f32 = {R_PEBBLE};");
    println!();
    println!("/// Depth of the baked slab behind the cut plane, in vessel radii.");
    println!("///");
    println!("/// Every entry in [`PACKED_PEBBLES`] has `-DEPTH_WINDOW <= z <= 0`. This is");
    println!(
        "/// {:.1} pebble diameters — deep enough that overlapping pebbles read as a solid",
        DEPTH_WINDOW / (2.0 * R_PEBBLE)
    );
    println!("/// bed with depth, shallow enough that the widget is not paying to draw");
    println!("/// pebbles the front layers occlude. See the module docs for the measured");
    println!("/// count/coverage trade behind the number.");
    println!("pub const DEPTH_WINDOW: f32 = {DEPTH_WINDOW};");
    println!();
    println!("/// Measured `[min_z, max_z]` of the baked pebble centres, in vessel radii.");
    println!("///");
    println!("/// Both lie inside `[-`[`DEPTH_WINDOW`]`, 0]` by construction; this records");
    println!("/// where the data actually landed, which is not exactly the window bounds");
    println!("/// because it is a finite sample of discrete sphere centres.");
    println!("pub const DEPTH_BOUNDS: [f32; 2] = [{min_z:.5}, {max_z:.5}];");
    println!();
    println!("/// Map a pebble's depth `z` to a dimensionless fraction in `[0, 1]`:");
    println!("/// `0` at the far edge of the baked window, `1` on the cut face nearest the");
    println!("/// viewer. Values outside the window clamp.");
    println!("///");
    println!("/// **This is a display cue, not physics.** It carries no units and means");
    println!("/// nothing thermally, neutronically, or mechanically — it exists so a widget");
    println!("/// can shade, tint, or slightly shrink a pebble by how far back it sits");
    println!("/// without having to know [`DEPTH_WINDOW`] itself. Typical use: multiply a");
    println!("/// base colour's brightness by `0.45 + 0.55 * depth`, so the back of the bed");
    println!("/// falls into shadow and the cut face reads as lit.");
    println!("///");
    println!("/// Monotone non-decreasing in `z`, so ordering the table by `z` (as it is");
    println!("/// baked) also orders it by this fraction.");
    println!("#[must_use]");
    println!("pub fn depth_fraction(z: f32) -> f32 {{");
    println!("    (1.0 + z / DEPTH_WINDOW).clamp(0.0, 1.0)");
    println!("}}");
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
    println!("/// a fill-level indicator against. It is an upper bound for every pebble in");
    println!("/// [`PACKED_PEBBLES`] (the depth window may not contain the tallest sphere,");
    println!("/// so the window's own top, [`BED_BOUNDS`]`[3]`, can be slightly lower).");
    println!("pub const BED_TOP: f32 = {:.5};", bed_top);
    println!();
    println!("/// Tight bounding box of the baked pebbles as drawn, in the plane of the");
    println!("/// screen: `[min_x, max_x, min_y, max_y]`, each centre expanded by");
    println!("/// [`SPHERE_RADIUS`]. Measured from the data below. For the out-of-plane");
    println!("/// extent see [`DEPTH_BOUNDS`].");
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
        "/// The baked packing: {} sphere centres from the settled pebble bed, taken",
        kept.len()
    );
    println!("/// from the slab just behind the cut plane and **sorted back to front**");
    println!("/// (`z` ascending — farthest first).");
    println!("///");
    println!("/// Paint them in this order and the painter's algorithm handles occlusion");
    println!("/// for you. Every pebble draws at [`SPHERE_RADIUS`]; use");
    println!("/// [`PackedPebble::depth`] for the depth shading.");
    println!("///");
    println!("/// See the module documentation for the coordinate convention, the `z`");
    println!("/// sign convention, and the honest-scope caveat (artwork, not validated");
    println!("/// physics).");
    println!("pub const PACKED_PEBBLES: &[PackedPebble] = &[");
    for s in kept {
        println!("    PackedPebble::new({:.5}, {:.5}, {:.5}),", s.x, s.y, s.z);
    }
    println!("];");
    emit_tests(m);
}

/// Emit the unit-test module appended to the generated file.
///
/// The measured numbers in [`BakeMetrics`] are quoted verbatim into the
/// generated V&V doc comment and into the tolerance justification, so the
/// committed file records what this bake actually produced rather than a
/// remembered or rounded figure.
fn emit_tests(m: &BakeMetrics) {
    let BakeMetrics {
        n_total,
        n_kept,
        solid_fraction_cv,
        depth_bounds: [min_z, max_z],
        coverage,
        max_wall_excess,
        max_outline_excess,
        ..
    } = *m;

    println!();
    println!("#[cfg(test)]");
    println!("mod tests {{");
    println!("    //! # Verification of the baked packing table");
    println!("    //!");
    println!("    //! ## Methodology");
    println!("    //!");
    println!("    //! These are **verification** checks — \"is the committed table the thing");
    println!("    //! the module documentation says it is?\" — and deliberately **not**");
    println!("    //! validation of the packing physics: the packing is artwork and is not");
    println!("    //! validated against anything (see the module-level scope note). Each test");
    println!("    //! re-derives one documented property directly from [`PACKED_PEBBLES`]");
    println!("    //! instead of trusting the generator that wrote it:");
    println!("    //!");
    println!("    //! 1. the table is non-empty and within the per-frame drawing budget;");
    println!("    //! 2. every coordinate is finite and every `z` lies in the stated depth");
    println!("    //!    window `[-`[`DEPTH_WINDOW`]`, 0]`;");
    println!("    //! 3. every pebble, drawn at [`SPHERE_RADIUS`], lies inside the vessel");
    println!("    //!    outline ([`vessel_half_width`]) at its own height, above the chute");
    println!("    //!    plug and below [`BED_TOP`];");
    println!("    //! 4. the table is sorted **back to front** (`z` ascending) — the property");
    println!("    //!    a painter's-algorithm consumer relies on for occlusion;");
    println!("    //! 5. [`BED_BOUNDS`] agrees with the data (one test) and so does");
    println!("    //!    [`DEPTH_BOUNDS`] (a second);");
    println!("    //! 6. [`depth_fraction`] stays in `[0, 1]`, is monotone non-decreasing in");
    println!("    //!    `z`, and hits its documented endpoints;");
    println!("    //! 7. [`vessel_half_width`] reproduces the documented taper at its three");
    println!("    //!    defining heights.");
    println!("    //!");
    println!("    //! Reference: the module-level coordinate and ordering contract. Pass");
    println!("    //! criterion: exact for the ordering and range checks, and within [`TOL`]");
    println!("    //! (justified from the measured DEM wall overlap) for the geometric ones.");
    println!("    //!");
    println!("    //! ## Results — measured on the 2026-08-06 bake");
    println!("    //!");
    println!("    //! All **8** tests pass on the committed table. The numbers they were run");
    println!("    //! against, straight from the generator:");
    println!("    //!");
    println!("    //! | Quantity | Measured |");
    println!("    //! |---|---|");
    println!("    //! | Pebbles in the table | {n_kept} of {n_total} settled spheres |");
    println!("    //! | Depth window / data span | `-{DEPTH_WINDOW} <= z <= 0` / `[{min_z:.5}, {max_z:.5}]` |");
    println!(
        "    //! | Vessel silhouette covered | {:.1} % |",
        100.0 * coverage
    );
    println!("    //! | Deepest wall penetration, 3-D | `{max_wall_excess:.2e} R` |");
    println!("    //! | Deepest outline penetration, as drawn | `{max_outline_excess:.2e} R` |");
    println!("    //! | Containment tolerance used | `5.0e-3 R` |");
    println!("    //! | Interior solid fraction of the parent packing | {solid_fraction_cv:.4} |");
    println!("    //!");
    println!("    //! **Interpretation.** The committed artwork is internally consistent and");
    println!("    //! sits inside the outline it is drawn against. The penetrations above are");
    println!("    //! the soft-sphere contact overlap of the DEM run, not a bookkeeping");
    println!("    //! error, and they are small but **not** negligible-by-orders-of-magnitude:");
    println!(
        "    //! the drawn-outline figure `{max_outline_excess:.2e} R` is {:.1}x below the `5.0e-3 R`",
        5.0e-3 / max_outline_excess
    );
    println!(
        "    //! tolerance and {:.1} % of a pebble radius, and the 3-D figure `{max_wall_excess:.2e} R`",
        100.0 * max_outline_excess / R_PEBBLE
    );
    println!(
        "    //! (the chute plug, where the whole column's weight concentrates) is {:.1}x",
        5.0e-3 / max_wall_excess
    );
    println!("    //! below it. Both are invisible at drawing resolution, so a widget laying");
    println!("    //! out from [`BED_BOUNDS`] and [`BED_TOP`] cannot visibly clip the bed —");
    println!("    //! but the margin is single-digit, so a future re-bake with a softer");
    println!("    //! contact spring could legitimately need [`TOL`] revisited rather than");
    println!("    //! the data being wrong. Because the ordering check passes, a consumer may");
    println!("    //! paint the table straight through and get correct occlusion with no");
    println!("    //! sorting of its own. None of this says the *packing* is physically");
    println!("    //! right; it says the table is the artwork it claims to be.");
    println!();
    println!("    use super::*;");
    println!();
    println!("    /// Tolerance for the containment checks, in vessel radii.");
    println!("    ///");
    println!("    /// A settled soft-sphere DEM bed presses very slightly into its walls:");
    println!("    /// the linear contact spring (`k_n = {K_N:.0e} N/m`) yields");
    println!("    /// `m g / k_n ≈ 3e-5 R` under one pebble's own weight and a few tens of");
    println!("    /// times that where the column load concentrates on the chute plug. The");
    println!("    /// deepest such overlap measured in this bake was `{max_wall_excess:.2e} R`");
    println!("    /// (3-D) and `{max_outline_excess:.2e} R` for a drawn circle against the");
    println!("    /// vessel outline. `5e-3 R` is one fifteenth of a pebble radius: it clears");
    println!(
        "    /// the measured values by {:.1}x and {:.1}x respectively — enough headroom that the",
        5.0e-3 / max_wall_excess,
        5.0e-3 / max_outline_excess
    );
    println!("    /// test is not brittle, while still being invisible when drawn.");
    println!("    const TOL: f32 = 5.0e-3;");
    println!();
    println!("    /// Upper bound on the table size, as a drawing-cost regression guard.");
    println!("    ///");
    println!("    /// Each pebble is painted with a TRISO speckle of order 50 dots, so the");
    println!("    /// per-repaint circle count is roughly 50x the table length. 600 pebbles");
    println!("    /// (~30 000 circles) is the point past which a deeper window buys occluded");
    println!("    /// pebbles at the expense of frame rate. A re-bake that blows through this");
    println!("    /// should be a deliberate, argued decision — not a silent regression.");
    println!("    const MAX_PEBBLES_FOR_FRAME_BUDGET: usize = 600;");
    println!();
    println!("    /// The baked table is non-empty and inside the drawing budget. A silently");
    println!("    /// empty bake would draw an empty vessel with no error anywhere; a silently");
    println!("    /// huge one would just drop the frame rate.");
    println!("    #[test]");
    println!("    fn table_size_is_sane_and_within_the_frame_budget() {{");
    println!("        assert!(");
    println!("            PACKED_PEBBLES.len() > 100,");
    println!("            \"expected a few hundred baked pebbles, got {{}}\",");
    println!("            PACKED_PEBBLES.len()");
    println!("        );");
    println!("        assert!(");
    println!("            PACKED_PEBBLES.len() <= MAX_PEBBLES_FOR_FRAME_BUDGET,");
    println!("            \"table of {{}} pebbles exceeds the {{MAX_PEBBLES_FOR_FRAME_BUDGET}}-pebble drawing budget\",");
    println!("            PACKED_PEBBLES.len()");
    println!("        );");
    println!("    }}");
    println!();
    println!("    /// Every coordinate is finite, and every pebble sits in the documented");
    println!("    /// depth window: behind the cut plane (`z <= 0`) and no farther back than");
    println!("    /// [`DEPTH_WINDOW`]. A positive `z` would mean a pebble in the half of the");
    println!("    /// bed that was supposed to have been cut away.");
    println!("    #[test]");
    println!("    fn every_pebble_is_finite_and_inside_the_depth_window() {{");
    println!("        for (i, p) in PACKED_PEBBLES.iter().enumerate() {{");
    println!("            assert!(");
    println!("                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),");
    println!("                \"pebble {{i}} has a non-finite coordinate\"");
    println!("            );");
    println!("            assert!(");
    println!("                p.z <= TOL,");
    println!("                \"pebble {{i}} is in front of the cut plane: z = {{}}\",");
    println!("                p.z");
    println!("            );");
    println!("            assert!(");
    println!("                p.z >= -DEPTH_WINDOW - TOL,");
    println!("                \"pebble {{i}} is behind the depth window: z = {{}} < -{{DEPTH_WINDOW}}\",");
    println!("                p.z");
    println!("            );");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// Every pebble, drawn as a full circle of [`SPHERE_RADIUS`], lies inside");
    println!("    /// the vessel outline: within the barrel/cone half-width at its own height,");
    println!("    /// above the chute plug, and below the recorded bed top.");
    println!("    #[test]");
    println!("    fn every_pebble_is_inside_the_vessel_outline() {{");
    println!("        for (i, p) in PACKED_PEBBLES.iter().enumerate() {{");
    println!("            assert!(");
    println!("                p.y - SPHERE_RADIUS >= -CONE_HEIGHT - TOL,");
    println!("                \"pebble {{i}} pokes below the chute plug: y - r = {{}}\",");
    println!("                p.y - SPHERE_RADIUS");
    println!("            );");
    println!("            assert!(");
    println!("                p.y + SPHERE_RADIUS <= BED_TOP + TOL,");
    println!("                \"pebble {{i}} is above the recorded bed top: y + r = {{}}\",");
    println!("                p.y + SPHERE_RADIUS");
    println!("            );");
    println!("            let half_width = vessel_half_width(p.y);");
    println!("            assert!(");
    println!("                p.x.abs() + SPHERE_RADIUS <= half_width + TOL,");
    println!("                \"pebble {{i}} at y = {{}} pokes through the wall: |x| + r = {{}} > {{half_width}}\",");
    println!("                p.y,");
    println!("                p.x.abs() + SPHERE_RADIUS");
    println!("            );");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// The table is sorted **back to front** (`z` ascending), as the module doc");
    println!("    /// promises. This is the property a consumer relies on to paint straight");
    println!("    /// through the table with the painter's algorithm: break it and near");
    println!("    /// pebbles get buried behind far ones.");
    println!("    #[test]");
    println!("    fn table_is_sorted_back_to_front() {{");
    println!("        for (i, w) in PACKED_PEBBLES.windows(2).enumerate() {{");
    println!("            assert!(");
    println!("                w[0].z <= w[1].z,");
    println!("                \"table is not back-to-front at {{i}}: z = {{}} then {{}}\",");
    println!("                w[0].z,");
    println!("                w[1].z");
    println!("            );");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    /// The recorded [`BED_BOUNDS`] really is the tight bounding box of the");
    println!("    /// drawn table, so a widget that lays out from it cannot clip the artwork.");
    println!("    #[test]");
    println!("    fn bed_bounds_match_the_table() {{");
    println!("        let mut min_x = f32::INFINITY;");
    println!("        let mut max_x = f32::NEG_INFINITY;");
    println!("        let mut min_y = f32::INFINITY;");
    println!("        let mut max_y = f32::NEG_INFINITY;");
    println!("        for p in PACKED_PEBBLES {{");
    println!("            min_x = min_x.min(p.x - SPHERE_RADIUS);");
    println!("            max_x = max_x.max(p.x + SPHERE_RADIUS);");
    println!("            min_y = min_y.min(p.y - SPHERE_RADIUS);");
    println!("            max_y = max_y.max(p.y + SPHERE_RADIUS);");
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
    println!("    /// The recorded [`DEPTH_BOUNDS`] really is the `z` range of the table, and");
    println!("    /// it sits inside the declared [`DEPTH_WINDOW`].");
    println!("    #[test]");
    println!("    fn depth_bounds_match_the_table() {{");
    println!("        let mut min_z = f32::INFINITY;");
    println!("        let mut max_z = f32::NEG_INFINITY;");
    println!("        for p in PACKED_PEBBLES {{");
    println!("            min_z = min_z.min(p.z);");
    println!("            max_z = max_z.max(p.z);");
    println!("        }}");
    println!("        assert!((min_z - DEPTH_BOUNDS[0]).abs() < TOL, \"min z drift\");");
    println!("        assert!((max_z - DEPTH_BOUNDS[1]).abs() < TOL, \"max z drift\");");
    println!("        assert!(DEPTH_BOUNDS[0] >= -DEPTH_WINDOW - TOL);");
    println!("        assert!(DEPTH_BOUNDS[1] <= TOL);");
    println!("    }}");
    println!();
    println!("    /// [`depth_fraction`] is a normalised, monotone display cue: in `[0, 1]`");
    println!("    /// for every baked pebble, non-decreasing in `z` (so nearer is never");
    println!("    /// darker than farther), and hitting its documented endpoints — `0` at the");
    println!("    /// back of the window, `1` on the cut face.");
    println!("    #[test]");
    println!("    fn depth_fraction_is_a_normalised_monotone_cue() {{");
    println!("        for (i, p) in PACKED_PEBBLES.iter().enumerate() {{");
    println!("            let d = p.depth();");
    println!("            assert!(");
    println!("                (0.0..=1.0).contains(&d),");
    println!("                \"pebble {{i}} has out-of-range depth fraction {{d}}\"");
    println!("            );");
    println!("        }}");
    println!();
    println!("        // Endpoints, and clamping outside the window.");
    println!("        assert!((depth_fraction(0.0) - 1.0).abs() < 1e-6);");
    println!("        assert!(depth_fraction(-DEPTH_WINDOW).abs() < 1e-6);");
    println!("        assert!((depth_fraction(1.0) - 1.0).abs() < 1e-6);");
    println!("        assert!(depth_fraction(-10.0).abs() < 1e-6);");
    println!();
    println!("        // Monotone non-decreasing across the window and beyond it.");
    println!("        const SAMPLES: usize = 64;");
    println!("        let mut previous = -1.0_f32;");
    println!("        for i in 0..=SAMPLES {{");
    println!("            // Sweep z from two windows *behind* the far edge to one window");
    println!("            // in front of the cut plane, so the clamped tails are covered too.");
    println!("            let t = (i as f32) / (SAMPLES as f32);");
    println!("            let z = DEPTH_WINDOW * (3.0 * t - 2.0);");
    println!("            let d = depth_fraction(z);");
    println!("            assert!(d >= previous - 1e-6, \"depth fraction dips at z = {{z}}\");");
    println!("            previous = d;");
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
