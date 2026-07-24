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

//! # End-to-end pebble-bed settling demo
//!
//! A minimal, fully reproducible **pebble-bed settling** demonstration built
//! entirely on the crate's public [`DemSimulation`](outram_park_fork_liggghts::simulation::DemSimulation)
//! engine. It seeds `N ≈ 200` equal graphite-sized spheres on a deterministic
//! staggered lattice inside a vertical [`Cylinder`](outram_park_fork_liggghts::boundary::Boundary::Cylinder)
//! container closed by a floor [`Wall`](outram_park_fork_liggghts::boundary::Boundary::Wall),
//! releases them under gravity, and runs the soft-sphere DEM loop long enough
//! for them to collapse the small inter-layer gaps and settle into a packed
//! bed. It then reports the particle count, the final total kinetic energy
//! (which decays toward zero as the bed settles, thanks to the damped
//! [`Hooke`](outram_park_fork_liggghts::contact::ContactModel::Hooke) contact
//! law), the settled **bed height**, and the **packing fraction**.
//!
//! ## What this demonstrates
//!
//! - Composing [`Particle`], [`ContactModel`], and [`Boundary`] into a runnable
//!   multi-particle DEM case through the engine's public constructor.
//! - Deterministic, **RNG-free** particle seeding — every centre is a pure
//!   function of its integer lattice index, so the run is bit-for-bit
//!   reproducible on any platform (and on Android/Termux natively: this is a
//!   pure-compute example, no BLAS and no GUI, so a plain `fn main()` suffices).
//! - Qualitative settling behaviour: kinetic energy monotonically dissipates
//!   through the normal dashpot and Coulomb friction until the bed is at rest.
//!
//! ## Honest scope — this is a VERIFICATION DEMO, not a validated simulation
//!
//! This example exercises the engine end-to-end; it is **not** a validated
//! pebble-bed model and its numbers must not be read as physical predictions:
//!
//! - **Small `N`** (a few hundred spheres) with strong wall confinement, so the
//!   packing is dominated by boundary and finite-size effects, not bulk
//!   statistics.
//! - **Simple lattice seeding.** The initial arrangement is an ordered,
//!   half-offset lattice, not a poured random packing. A settled *ordered*
//!   lattice does not reproduce the random-close-packing statistics of real
//!   poured pebbles.
//! - **Illustrative packing fraction.** The reported packing fraction is
//!   diagnostic only. For monodisperse spheres, **random close packing (RCP) is
//!   ≈ 0.6366** (Scott & Kilgour, 1969 — see references); an ordered lattice
//!   settled here will differ (a simple/loose lattice packs looser; a perfectly
//!   ordered FCC/HCP would pack denser at ≈ 0.7405). The value below is a
//!   sanity readout of *this* small ordered case, not a prediction of pebble-bed
//!   void fraction.
//! - No history-dependent tangential spring, no polydispersity, single-threaded
//!   — see the [`simulation`](outram_park_fork_liggghts::simulation) module's
//!   own "Honest scope" note.
//!
//! ## Numerical parameters and why
//!
//! Contact stiffness `k_n = 1.0e5 N/m`. For the reduced contact mass
//! `m* = m/2 ≈ 0.10 kg` the contact angular frequency is
//! `ω = √(k_n/m*) ≈ 1.0e3 rad/s`, so the velocity-Verlet stability bound is
//! `dt < 2/ω ≈ 2.0e-3 s`. We use `dt = 1.0e-4 s` — a ~20× safety margin — and
//! run `10000` steps (`t = 1.0 s`), a few thousand steps as the task budgets,
//! which is ample for these short (~mm-scale) inter-layer gaps to close and the
//! kinetic energy to decay below the "settled" threshold. Normal damping
//! `γ_n = 200 N·s/m` is ~1× the critical value `2√(k_n·m*) ≈ 200 N·s/m`
//! (critically damped contacts, quick settling), and Coulomb friction `μ = 0.5`
//! with a tangential dashpot bleeds off tangential/rotational energy so the pile
//! locks up rather than ringing. The bed is deliberately kept **shallow** (a few
//! layers in a wide cylinder): a tall column of stacked contacts rings in a
//! low-frequency collective ("breathing") mode that a per-contact-critical
//! dashpot barely damps, so it would not quiesce inside this step budget.
//!
//! ## References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - G. D. Scott and D. M. Kilgour, "The density of random close packing of
//!   spheres," *J. Phys. D: Appl. Phys.* **2**(6), 863–866 (1969) — the random
//!   close packing fraction of monodisperse spheres, ≈ 0.6366.
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for granular
//!   assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the soft-sphere DEM
//!   step this engine implements.
//! - T. Pöschel and T. Schwager, *Computational Granular Dynamics* (Springer,
//!   2005) — settling / packing of granular assemblies.

use std::f64::consts::PI;

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::contact::{ContactModel, HookeContact};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use outram_park_fork_liggghts::simulation::DemSimulation;

use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::thermodynamic_temperature::kelvin;

// --- Physical / geometric parameters (all SI base units) -------------------

/// Pebble radius `[m]` (~3 cm — comparable to a real HTR pebble radius).
const PEBBLE_RADIUS: f64 = 0.03;
/// Pebble material density `[kg/m³]` (nuclear graphite, ≈ 1750).
const PEBBLE_DENSITY: f64 = 1750.0;
/// Inner radius of the cylindrical container `[m]`. Chosen wide and shallow (a
/// few pebble layers) rather than tall and narrow: a short stack's collective
/// (breathing-mode) contact oscillations are high enough in frequency for the
/// contact damping to kill them within the step budget, whereas a tall column of
/// stacked contacts rings in a low-frequency mode that never quiesces in time.
const CYLINDER_RADIUS: f64 = 0.25;
/// Pebble (constant) temperature `[K]` — passive state in this crate.
const PEBBLE_TEMPERATURE: f64 = 300.0;

// --- Seeding lattice (deterministic, RNG-free) -----------------------------

/// Horizontal lattice spacing `[m]`: a hair over one diameter so seeded pebbles
/// start just clear of each other (no initial overlap to blow up the integrator).
const SPACING_XY: f64 = 2.0 * PEBBLE_RADIUS * 1.03;
/// Vertical spacing between seeded layers `[m]`: a little over a diameter so
/// each layer has a small gap to fall through and compact, but not so large that
/// the released potential energy takes many seconds to dissipate.
const SPACING_Z: f64 = 2.0 * PEBBLE_RADIUS * 1.03;
/// Height of the first (lowest) seeded layer's centre above the floor `[m]`.
const FIRST_LAYER_Z: f64 = PEBBLE_RADIUS * 1.03;
/// Number of seeded layers stacked vertically (kept small for a shallow bed).
const N_LAYERS: usize = 8;

// --- Contact law and time integration --------------------------------------

/// Normal spring stiffness `k_n` `[N/m]`.
const K_N: f64 = 1.0e5;
/// Normal dashpot coefficient `γ_n` `[N·s/m]` (~critical, `2√(k_n·m*) ≈ 200`).
const GAMMA_N: f64 = 200.0;
/// Tangential spring stiffness `k_t` `[N/m]`.
const K_T: f64 = 8.0e4;
/// Tangential dashpot coefficient `γ_t` `[N·s/m]`.
const GAMMA_T: f64 = 200.0;
/// Coulomb friction coefficient `μ` `[-]`.
const FRICTION: f64 = 0.5;
/// Fixed velocity-Verlet time step `[s]` (~20× inside the stability bound).
const DT: f64 = 1.0e-4;
/// Number of integration steps to run (`t = N_STEPS · DT = 1.0 s`).
const N_STEPS: usize = 10_000;
/// Kinetic-energy threshold `[J]` below which we call the bed "settled".
const SETTLED_KE_THRESHOLD: f64 = 1.0e-4;

/// Build one validated pebble at `position` (at rest), with mass fixed by the
/// pebble radius and density.
fn make_pebble(position: Vec3) -> Particle {
    let volume = (4.0 / 3.0) * PI * PEBBLE_RADIUS.powi(3);
    let mass_kg = PEBBLE_DENSITY * volume;
    Particle::new(
        position,
        Vec3::zero(),
        Vec3::zero(),
        Mass::new::<kilogram>(mass_kg),
        Length::new::<meter>(PEBBLE_RADIUS),
        ThermodynamicTemperature::new::<kelvin>(PEBBLE_TEMPERATURE),
    )
    .expect("pebble parameters are valid (positive mass, radius, temperature)")
}

/// Seed the pebble ensemble on a deterministic, RNG-free staggered lattice
/// inside the cylinder.
///
/// Every centre is a pure function of its integer indices `(layer, ix, iy)`, so
/// the whole configuration is reproducible on any platform. Each layer is an
/// axis-aligned square grid on spacing [`SPACING_XY`], with layers stacked on a
/// slightly-gapped vertical pitch [`SPACING_Z`]; released under gravity the
/// columns compress and the small inter-layer gaps close, so the bed settles
/// cleanly (this ordered seed avoids the avalanche bursts a half-offset
/// staggered seed produces, which never fully quiesce inside a short step
/// budget). A grid point is kept only if its centre lies within the cylinder
/// with a full pebble radius of clearance (radial distance ≤ `R_cyl − r`), so no
/// pebble is seeded already overlapping the wall.
fn seed_lattice() -> Vec<Particle> {
    // Largest radial coordinate a pebble *centre* may take without the pebble
    // poking through the cylinder wall.
    let max_center_radius = CYLINDER_RADIUS - PEBBLE_RADIUS;
    // Half-width of the square grid, in lattice steps, that covers the disc.
    let half_span = (max_center_radius / SPACING_XY).ceil() as i64 + 1;

    let mut particles = Vec::new();
    for layer in 0..N_LAYERS {
        let z = FIRST_LAYER_Z + layer as f64 * SPACING_Z;
        for ix in -half_span..=half_span {
            for iy in -half_span..=half_span {
                let x = ix as f64 * SPACING_XY;
                let y = iy as f64 * SPACING_XY;
                // Keep only centres that fit inside the cylinder with clearance.
                if (x * x + y * y).sqrt() <= max_center_radius {
                    particles.push(make_pebble(Vec3::new(x, y, z)));
                }
            }
        }
    }
    particles
}

fn main() {
    // --- Domain: a vertical cylinder closed by a floor ---------------------
    // Cylinder axis is the world z-axis through the origin; it is infinite along
    // z (no end caps), and the floor Wall at z = 0 (outward normal +z) closes
    // the bottom.
    let cylinder = Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), CYLINDER_RADIUS)
        .expect("cylinder radius and axis are valid");
    let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0))
        .expect("floor normal is non-zero");

    // --- Contact law: damped linear spring-dashpot (Hooke) -----------------
    // Damping in the normal (and tangential) dashpot is what lets the kinetic
    // energy decay to ~0 as the bed settles.
    let contact = ContactModel::Hooke(
        HookeContact::new(K_N, GAMMA_N, K_T, GAMMA_T, FRICTION)
            .expect("Hooke coefficients are valid (k_n > 0, others >= 0)"),
    );

    // --- Assemble and run --------------------------------------------------
    let particles = seed_lattice();
    let n = particles.len();
    // Total solid volume of the pebbles [m^3] (all equal spheres).
    let single_volume = (4.0 / 3.0) * PI * PEBBLE_RADIUS.powi(3);
    let total_pebble_volume = n as f64 * single_volume;

    let gravity = Vec3::new(0.0, 0.0, -9.81);
    let mut sim = DemSimulation::new(
        particles,
        vec![cylinder, floor],
        contact,
        gravity,
        DT,
    )
    .expect("dt is strictly positive");

    let ke_initial = sim.kinetic_energy();
    sim.run(N_STEPS);
    let ke_final = sim.kinetic_energy();

    // --- Diagnostics -------------------------------------------------------
    // Bed height = highest pebble top surface above the floor.
    let mut bed_height = 0.0_f64;
    let mut lowest_center = f64::INFINITY;
    for p in sim.particles() {
        let top = p.position.z + p.radius;
        if top > bed_height {
            bed_height = top;
        }
        if p.position.z < lowest_center {
            lowest_center = p.position.z;
        }
    }

    // Occupied bed volume = cylinder cross-section x bed height. This is the
    // container volume the settled bed fills; the packing fraction is the solid
    // fraction of that occupied region.
    let cross_section = PI * CYLINDER_RADIUS * CYLINDER_RADIUS;
    let occupied_bed_volume = cross_section * bed_height;
    let packing_fraction = total_pebble_volume / occupied_bed_volume;

    let settled = ke_final < SETTLED_KE_THRESHOLD;

    // --- Report ------------------------------------------------------------
    println!("=== OUTRAM PARK — pebble-bed settling demo (DEM verification demo) ===");
    println!();
    println!("Container      : vertical cylinder, inner radius {CYLINDER_RADIUS:.3} m, floor at z = 0");
    println!("Pebbles        : {n} equal spheres, radius {PEBBLE_RADIUS:.3} m, density {PEBBLE_DENSITY:.0} kg/m^3");
    println!(
        "Contact model  : Hooke spring-dashpot  k_n = {K_N:.1e} N/m, gamma_n = {GAMMA_N:.0} N.s/m, mu = {FRICTION}"
    );
    println!(
        "Integration    : velocity-Verlet, dt = {DT:.1e} s, {N_STEPS} steps  (t = {:.3} s)",
        N_STEPS as f64 * DT
    );
    println!();
    println!("--- Results ---------------------------------------------------------");
    println!("Particle count            : {n}");
    println!("Kinetic energy (initial)  : {ke_initial:.6e} J   (seeded at rest)");
    println!("Kinetic energy (final)    : {ke_final:.6e} J");
    println!("Settled KE threshold      : {SETTLED_KE_THRESHOLD:.6e} J");
    println!("Bed height (top of bed)   : {bed_height:.4} m");
    println!("Lowest pebble centre z    : {lowest_center:.4} m  (floor rest ~ {PEBBLE_RADIUS:.4} m)");
    println!("Total pebble volume       : {total_pebble_volume:.6e} m^3");
    println!("Occupied bed volume       : {occupied_bed_volume:.6e} m^3  (pi R^2 x bed height)");
    println!("Packing fraction          : {packing_fraction:.4}");
    println!();
    println!(
        "Reference: monodisperse random close packing (RCP) ~ 0.6366 (Scott & Kilgour 1969)."
    );
    println!(
        "This is a small, ordered-lattice, wall-confined VERIFICATION demo, so its"
    );
    println!(
        "packing fraction is illustrative only and will differ from bulk RCP."
    );
    println!();
    if settled {
        println!("VERDICT: settled  (final KE {ke_final:.3e} J < {SETTLED_KE_THRESHOLD:.3e} J threshold)");
    } else {
        println!("VERDICT: NOT settled  (final KE {ke_final:.3e} J >= {SETTLED_KE_THRESHOLD:.3e} J threshold)");
    }
}
