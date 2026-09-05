// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — the linear parallel-bond (cemented-bond)
// model below is STANDARD PUBLIC-LITERATURE DEM. It is reimplemented CLEAN-ROOM
// from the cited papers (see the module "References" below); this file contains
// NO GPL-2.0 LIGGGHTS/LAMMPS source and is not a translation of it. See the
// crate NOTICE.
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

//! **Bonded-particle model** — the *linear parallel bond* of Potyondy & Cundall
//! (2004) for cemented granular material, agglomerates, and TRISO-particle /
//! pebble-matrix modelling.
//!
//! Where [`crate::contact`] gives the *unbonded* normal/tangential contact that
//! only ever **pushes** overlapping particles apart, this module adds a
//! **cemented bond**: a finite-size elastic cylinder of "glue" spanning the gap
//! between two particles that transmits a **force and a moment** — resisting
//! tension, shear, bending, and twisting — until a stress-based breakage
//! criterion is met, after which the bond fails and transmits nothing.
//!
//! ## The parallel-bond idealisation
//!
//! A parallel bond is pictured as a cylinder of cementitious material of radius
//! `R̄` acting in parallel with the point contact, glued across the contact
//! plane. Its cross-section carries:
//!
//! - a **normal force** `F̄_n` (a signed scalar, **tension positive**) and a
//!   **shear force** `F̄_s` (a vector in the contact plane), and
//! - a **twisting moment** `M̄_t` (about the bond axis `n̂`) and a **bending
//!   moment** `M̄_b` (a vector in the contact plane).
//!
//! The bond is **history-dependent**: like a real spring it accumulates force
//! and moment from the *increments* of relative motion over each time step, so
//! the [`Bond`] carries this accumulated state and [`Bond::update_bond`]
//! advances it incrementally (Cundall & Strack's incremental small-strain DEM
//! philosophy, 1979).
//!
//! ## Geometry, stiffness, and stress (Potyondy & Cundall 2004)
//!
//! With bond radius `R̄` the cross-sectional geometric properties are the
//! textbook ones for a solid circular section:
//!
//! - area `A = π R̄²` `[m²]`,
//! - second moment of area (bending) `I = π R̄⁴ / 4` `[m⁴]`,
//! - polar moment of area (twisting) `J = π R̄⁴ / 2` `[m⁴]`.
//!
//! The bond has a **normal stiffness per unit area** `k̄_n` and a **shear
//! stiffness per unit area** `k̄_s`, both in `[Pa/m] = [N/m³]` (multiplying a
//! stiffness-per-area `[Pa/m]` by area `[m²]` and a displacement `[m]` gives a
//! force `[N]`). Over a step the elastic force/moment increments are
//!
//! ```text
//!   ΔF̄_n = +k̄_n · A · Δδ_n           (normal, tension positive)
//!   ΔF̄_s = −k̄_s · A · Δδ_s           (shear, vector)
//!   ΔM̄_t = −k̄_s · J · Δθ_t           (twisting, about n̂)
//!   ΔM̄_b = −k̄_n · I · Δθ_b           (bending, vector ⟂ n̂)
//! ```
//!
//! where `Δδ_n`, `Δδ_s` are the normal and shear relative-displacement
//! increments at the bond and `Δθ_t`, `Δθ_b` the twist and bending relative-
//! rotation increments (all over the step `Δt`). The maximum tensile normal
//! stress and maximum shear stress acting on the bond periphery are
//!
//! ```text
//!   σ_max = F̄_n / A + |M̄_b| · R̄ / I         (axial + bending fibre stress)
//!   τ_max = |F̄_s| / A + |M̄_t| · R̄ / J        (direct shear + torsional shear)
//! ```
//!
//! and the bond **breaks** the instant `σ_max > σ_c` (tensile strength) or
//! `τ_max > τ_c` (shear strength). A broken bond is permanent and transmits
//! zero force and zero moment thereafter ([`Bond::is_broken`]).
//!
//! ## Sign & geometry conventions (read once, applies everywhere)
//!
//! For a bonded pair `(a, b)`, following [`crate::contact`]:
//!
//! - The **bond axis** `n̂` is the unit vector from `a`'s centre toward `b`'s
//!   centre: `n̂ = (x_b − x_a) / ‖x_b − x_a‖`.
//! - The **normal displacement increment** `Δδ_n = [(v_b − v_a)·n̂]·Δt` is
//!   **positive when the particles separate**, so a stretched bond builds a
//!   **positive (tensile)** `F̄_n`, giving `σ_max > 0` in tension.
//! - The accumulated **`(F̄_n, F̄_s, M̄_t, M̄_b)` is bookkept as the load the bond
//!   exerts on particle `b`** (with `F̄_n` stored tension-positive); the load on
//!   `a` is its exact Newton-third-law reaction. Concretely the returned
//!   [`BondForce`] has `force_on_a = F̄_n·n̂ − F̄_s` and
//!   `force_on_b = −force_on_a`. A tensile bond therefore pulls `a` toward `b`
//!   and `b` toward `a`, as a real cement ligament would.
//! - The bond force acts at the contact point (offset `+r_a·n̂` from `a`'s centre
//!   and `−r_b·n̂` from `b`'s), so the shear force exerts a lever torque about
//!   each centre; the bond moment `M̄` is applied as an equal-and-opposite
//!   internal couple on the two particles.
//!
//! ## Unit convention
//!
//! Following the crate convention (see [`crate::particle`] and
//! [`crate::contact`]), the `uom` boundary sits at the constructor where a clean
//! named `uom` type exists: [`Bond::new`] takes the bond radius as a [`Length`]
//! and the tensile/shear strengths as [`Pressure`]. The stiffnesses-per-area
//! `k̄_n`, `k̄_s` have no ergonomic named `uom` alias (`[Pa/m] = [N/m³]`), so they
//! are documented `f64`, consistent with the crate's f64-internal rule. Every
//! stored field and method spells out its SI unit in its doc comment.
//!
//! ## Honest scope
//!
//! This is a **verified foundation, not a validated model** — no cross-code or
//! experimental benchmark comparison has been run yet (that is the later human
//! validation step). The inline tests below check the force/moment law against
//! **hand-computed analytical values** and invariants (Newton's third law, the
//! exact breakage threshold) only.
//!
//! It deliberately implements **only** the **linear parallel bond** and nothing
//! else:
//!
//! - **No contact-bond variant** (Potyondy & Cundall's alternative point-contact
//!   bond that carries force but no moment) — only the moment-carrying parallel
//!   bond is here.
//! - **No thermal or fluid bond degradation** — the strengths `σ_c`, `τ_c` are
//!   fixed constants; irradiation-, temperature-, or corrosion-driven weakening
//!   is out of scope.
//! - **Incremental small-strain** kinematics only: the force/moment are built
//!   from per-step relative-displacement and relative-rotation increments (valid
//!   for the small per-step motions of an explicit DEM loop). There is **no**
//!   large-rotation reference-frame update of the accumulated shear force /
//!   bending moment; the caller must keep the DEM time step small.
//! - **No bond-network solver.** [`Bond::update_bond`] is a *per-bond* force
//!   evaluation: it returns the force and torque on each particle for **one**
//!   bond and does **not** integrate the particles, assemble a bond network, or
//!   own connectivity. A caller's DEM loop applies the returned [`BondForce`]
//!   (e.g. via [`crate::particle::Particle::integrate`]) and owns which particle
//!   pairs are bonded.
//!
//! ## References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - D. O. Potyondy and P. A. Cundall, "A bonded-particle model for rock,"
//!   *Int. J. Rock Mech. Min. Sci.* **41**(8), 1329–1364 (2004) — the linear
//!   parallel-bond force/moment–displacement law and the σ/τ breakage criterion
//!   implemented here.
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for granular
//!   assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the incremental
//!   small-strain DEM force–displacement philosophy the bond update follows.

use uom::si::f64::{Length, Pressure};
use uom::si::length::meter;
use uom::si::pressure::pascal;

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Centre distance `[m]` below which the bond axis reconstruction from
/// coincident particle centres is treated as degenerate and the bond is given
/// zero load for the step (avoids dividing by a near-zero centre distance).
const COINCIDENT_EPS: f64 = 1.0e-15;

/// The resolved force and torque a bond applies to its two particles over one
/// [`Bond::update_bond`] step.
///
/// All forces are in newtons `[N]` and torques in newton-metres `[N·m]`. By
/// Newton's third law `force_on_b = −force_on_a` exactly. The torques are **not**
/// equal-and-opposite in general: each is the moment of the bond force about
/// that particle's own centre (different lever arms `r_a` vs `r_b`) plus the
/// particle's share of the internal bond couple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BondForce {
    /// Total force on particle `a` `[N]` (`F̄_n·n̂ − F̄_s` in the module sign
    /// convention: a tensile bond pulls `a` toward `b`).
    pub force_on_a: Vec3,
    /// Total force on particle `b` `[N]`. Equals `−force_on_a` exactly.
    pub force_on_b: Vec3,
    /// Torque on particle `a` about its centre `[N·m]`: the moment of
    /// `force_on_a` at the contact point (`+r_a·n̂` from `a`'s centre) plus `a`'s
    /// half of the bond couple `−M̄`.
    pub torque_on_a: Vec3,
    /// Torque on particle `b` about its centre `[N·m]`: the moment of
    /// `force_on_b` at the contact point (`−r_b·n̂` from `b`'s centre) plus `b`'s
    /// half of the bond couple `+M̄`.
    pub torque_on_b: Vec3,
}

impl BondForce {
    /// The zero load — no force and no torque on either particle. Returned for a
    /// broken bond, a [`BondModel::None`], or a degenerate (coincident-centre)
    /// configuration.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            force_on_a: Vec3::zero(),
            force_on_b: Vec3::zero(),
            torque_on_a: Vec3::zero(),
            torque_on_b: Vec3::zero(),
        }
    }
}

/// A **linear parallel bond** (Potyondy & Cundall 2004): a cemented, moment-
/// carrying elastic bond between two DEM particles.
///
/// The bond carries **history-dependent** accumulated state — a normal force, a
/// shear-force vector, a twisting moment, and a bending-moment vector — that
/// [`Bond::update_bond`] advances by the elastic increments of each time step
/// (see the module-level equations). Once the tensile or shear stress criterion
/// is exceeded the bond is permanently `broken` and carries no load.
///
/// # Parameters and units
///
/// | Field | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `k_n` | `k̄_n` | normal stiffness per unit area | `[Pa/m] = [N/m³]` | `> 0` |
/// | `k_s` | `k̄_s` | shear stiffness per unit area | `[Pa/m] = [N/m³]` | `> 0` |
/// | `radius` | `R̄` | bond (cement cylinder) radius | `[m]` | `> 0` |
/// | `sigma_c` | `σ_c` | tensile strength | `[Pa]` | `> 0` |
/// | `tau_c` | `τ_c` | shear strength | `[Pa]` | `> 0` |
///
/// The accumulated-state fields (`normal_force`, `shear_force`, `twist_moment`,
/// `bend_moment`, `broken`) are **not** set by the caller; they start at zero /
/// intact from [`Bond::new`] and evolve only through [`Bond::update_bond`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bond {
    /// Normal stiffness per unit area `k̄_n` `[Pa/m] = [N/m³]`. Strictly positive.
    k_n: f64,
    /// Shear stiffness per unit area `k̄_s` `[Pa/m] = [N/m³]`. Strictly positive.
    k_s: f64,
    /// Bond radius `R̄` `[m]` — the radius of the cement cylinder. Strictly
    /// positive.
    radius: f64,
    /// Tensile strength `σ_c` `[Pa]`. The bond breaks when `σ_max > σ_c`.
    sigma_c: f64,
    /// Shear strength `τ_c` `[Pa]`. The bond breaks when `τ_max > τ_c`.
    tau_c: f64,

    /// Accumulated normal force `F̄_n` `[N]`, **tension positive**. Starts at 0.
    normal_force: f64,
    /// Accumulated shear force `F̄_s` `[N]` (a vector in the contact plane, ⟂
    /// `n̂`). Starts at the zero vector.
    shear_force: Vec3,
    /// Accumulated twisting moment `M̄_t` `[N·m]` about the bond axis `n̂` (a
    /// signed scalar). Starts at 0.
    twist_moment: f64,
    /// Accumulated bending moment `M̄_b` `[N·m]` (a vector ⟂ `n̂`). Starts at the
    /// zero vector.
    bend_moment: Vec3,
    /// `true` once the bond has failed. A broken bond permanently transmits no
    /// force or moment.
    broken: bool,
}

impl Bond {
    /// Construct a validated, intact parallel bond with zero accumulated load.
    ///
    /// The bond radius is taken as a `uom` [`Length`] and the two strengths as
    /// `uom` [`Pressure`] at this boundary so a caller cannot pass a
    /// dimensionally wrong scalar; they are stored internally as `f64` in SI
    /// base units (`m`, `Pa`). The stiffnesses-per-area `k_n`, `k_s` are `f64`
    /// in `[Pa/m] = [N/m³]` (no clean named `uom` alias — see the module "Unit
    /// convention").
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if any of `k_n`, `k_s`, `radius`,
    /// `sigma_c`, or `tau_c` is not strictly positive — a bond needs a real
    /// stiffness, a finite cross-section, and a positive strength to be
    /// physically meaningful.
    pub fn new(
        k_n: f64,
        k_s: f64,
        radius: Length,
        sigma_c: Pressure,
        tau_c: Pressure,
    ) -> Result<Self, DemError> {
        let radius = radius.get::<meter>();
        let sigma_c = sigma_c.get::<pascal>();
        let tau_c = tau_c.get::<pascal>();

        if !(k_n > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "bond normal stiffness k_n must be strictly positive, got {k_n} Pa/m"
            )));
        }
        if !(k_s > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "bond shear stiffness k_s must be strictly positive, got {k_s} Pa/m"
            )));
        }
        if !(radius > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "bond radius must be strictly positive, got {radius} m"
            )));
        }
        if !(sigma_c > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "bond tensile strength sigma_c must be strictly positive, got {sigma_c} Pa"
            )));
        }
        if !(tau_c > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "bond shear strength tau_c must be strictly positive, got {tau_c} Pa"
            )));
        }

        Ok(Self {
            k_n,
            k_s,
            radius,
            sigma_c,
            tau_c,
            normal_force: 0.0,
            shear_force: Vec3::zero(),
            twist_moment: 0.0,
            bend_moment: Vec3::zero(),
            broken: false,
        })
    }

    /// Bond cross-sectional area `A = π R̄²` `[m²]`.
    #[must_use]
    pub fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    /// Bond second moment of area (bending) `I = π R̄⁴ / 4` `[m⁴]` — the
    /// resistance of the circular cross-section to bending.
    #[must_use]
    pub fn bending_inertia(&self) -> f64 {
        std::f64::consts::PI * self.radius.powi(4) / 4.0
    }

    /// Bond polar moment of area (twisting) `J = π R̄⁴ / 2 = 2·I` `[m⁴]` — the
    /// resistance of the circular cross-section to torsion about `n̂`.
    #[must_use]
    pub fn polar_inertia(&self) -> f64 {
        std::f64::consts::PI * self.radius.powi(4) / 2.0
    }

    /// Accumulated normal force `F̄_n` `[N]`, tension positive.
    #[must_use]
    pub fn normal_force(&self) -> f64 {
        self.normal_force
    }

    /// Accumulated shear force `F̄_s` `[N]` (a vector in the contact plane).
    #[must_use]
    pub fn shear_force(&self) -> Vec3 {
        self.shear_force
    }

    /// Accumulated twisting moment `M̄_t` `[N·m]` about the bond axis `n̂`.
    #[must_use]
    pub fn twist_moment(&self) -> f64 {
        self.twist_moment
    }

    /// Accumulated bending moment `M̄_b` `[N·m]` (a vector ⟂ `n̂`).
    #[must_use]
    pub fn bend_moment(&self) -> Vec3 {
        self.bend_moment
    }

    /// Maximum tensile normal stress on the bond periphery `[Pa]`:
    /// `σ_max = F̄_n / A + |M̄_b|·R̄ / I` (axial plus bending fibre stress).
    ///
    /// A negative value means the bond is in net compression (no tensile fibre
    /// stress); the breakage criterion only ever fails a bond when this exceeds
    /// the positive `σ_c`.
    #[must_use]
    pub fn tensile_stress(&self) -> f64 {
        self.normal_force / self.area()
            + self.bend_moment.norm() * self.radius / self.bending_inertia()
    }

    /// Maximum shear stress on the bond periphery `[Pa]`:
    /// `τ_max = |F̄_s| / A + |M̄_t|·R̄ / J` (direct shear plus torsional shear).
    #[must_use]
    pub fn shear_stress(&self) -> f64 {
        self.shear_force.norm() / self.area()
            + self.twist_moment.abs() * self.radius / self.polar_inertia()
    }

    /// Whether the bond has failed. Once `true` it stays `true`, and
    /// [`Bond::update_bond`] returns [`BondForce::zero`] — a broken bond
    /// transmits no force and no moment.
    #[must_use]
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// Advance the bond's accumulated force/moment by the relative motion of the
    /// pair `(a, b)` over the time step `dt` `[s]`, apply the breakage criterion,
    /// and return the resolved force and torque on each particle.
    ///
    /// # Increment
    ///
    /// The bond axis is `n̂ = (x_b − x_a)/‖x_b − x_a‖`. The relative velocity of
    /// `b`'s bond-surface point with respect to `a`'s, at the contact point, is
    ///
    /// ```text
    ///   v_rel = (v_b − v_a) − (r_a·ω_a + r_b·ω_b) × n̂,
    /// ```
    ///
    /// decomposed into a normal part `v_n = (v_rel·n̂)` and a shear part
    /// `v_s = v_rel − v_n·n̂`; the relative angular velocity is
    /// `ω_rel = ω_b − ω_a`, split into a twist part `ω_rel·n̂` and a bending part
    /// `ω_rel − (ω_rel·n̂)·n̂`. The accumulated state is then incremented by
    ///
    /// ```text
    ///   F̄_n += +k̄_n · A · (v_n · dt)
    ///   F̄_s += −k̄_s · A · (v_s · dt)
    ///   M̄_t += −k̄_s · J · ((ω_rel·n̂) · dt)
    ///   M̄_b += −k̄_n · I · ((ω_rel − (ω_rel·n̂)n̂) · dt)
    /// ```
    ///
    /// # Breakage
    ///
    /// After the increment the stresses [`Bond::tensile_stress`] `σ_max` and
    /// [`Bond::shear_stress`] `τ_max` are evaluated; if `σ_max > σ_c` **or**
    /// `τ_max > τ_c` the bond is marked broken and [`BondForce::zero`] is
    /// returned. A bond already broken (or a degenerate coincident-centre
    /// configuration) returns [`BondForce::zero`] without changing state.
    ///
    /// # Assumptions and valid range
    ///
    /// Incremental small-strain kinematics: `dt` must be small enough that the
    /// per-step relative displacement/rotation is small (the explicit-DEM
    /// regime). No large-rotation frame update is applied to the accumulated
    /// shear force / bending moment (see the module "Honest scope").
    pub fn update_bond(&mut self, a: &Particle, b: &Particle, dt: f64) -> BondForce {
        if self.broken {
            return BondForce::zero();
        }

        // --- Bond axis n̂ = (x_b − x_a)/‖x_b − x_a‖ --------------------------
        let d = b.position.sub(a.position);
        let dist = d.norm();
        if dist <= COINCIDENT_EPS {
            // Degenerate: centres coincide, bond axis undefined — no load.
            return BondForce::zero();
        }
        let n = d.scale(1.0 / dist);

        let area = self.area();
        let i_bend = self.bending_inertia();
        let j_twist = self.polar_inertia();

        // --- Relative velocity of b's contact point w.r.t. a's --------------
        //   v_rel = (v_b − v_a) − (r_a ω_a + r_b ω_b) × n̂
        let rot = a
            .angular_velocity
            .scale(a.radius)
            .add(b.angular_velocity.scale(b.radius))
            .cross(n);
        let v_rel = b.velocity.sub(a.velocity).sub(rot);

        // Normal (tension positive when b separates from a) and shear parts.
        let v_n = v_rel.dot(n);
        let v_s = v_rel.sub(n.scale(v_n));

        // --- Relative angular velocity: twist (about n̂) and bending (⟂ n̂) ---
        let w_rel = b.angular_velocity.sub(a.angular_velocity);
        let w_t = w_rel.dot(n);
        let w_b = w_rel.sub(n.scale(w_t));

        // --- Elastic increments (Potyondy & Cundall 2004) ------------------
        self.normal_force += self.k_n * area * (v_n * dt);
        self.shear_force = self.shear_force.add(v_s.scale(-self.k_s * area * dt));
        self.twist_moment += -self.k_s * j_twist * (w_t * dt);
        self.bend_moment = self.bend_moment.add(w_b.scale(-self.k_n * i_bend * dt));

        // --- Breakage criterion --------------------------------------------
        let sigma_max = self.normal_force / area + self.bend_moment.norm() * self.radius / i_bend;
        let tau_max =
            self.shear_force.norm() / area + self.twist_moment.abs() * self.radius / j_twist;
        if sigma_max > self.sigma_c || tau_max > self.tau_c {
            self.broken = true;
            return BondForce::zero();
        }

        // --- Resolve force/torque on each particle -------------------------
        // Load bookkept on b: force = −F̄_n·n̂ + F̄_s (F̄_n tension-positive),
        // couple M̄ = M̄_t·n̂ + M̄_b. The force on a is the reaction.
        let force_on_a = n.scale(self.normal_force).sub(self.shear_force);
        let force_on_b = force_on_a.scale(-1.0);

        // Bond couple on b (equal-and-opposite on a).
        let m_couple_on_b = n.scale(self.twist_moment).add(self.bend_moment);

        // Contact point at +r_a·n̂ from a, −r_b·n̂ from b. The normal force is
        // collinear with n̂ → zero lever torque; only the shear force spins.
        let torque_on_a = n.scale(a.radius).cross(force_on_a).sub(m_couple_on_b);
        let torque_on_b = n.scale(-b.radius).cross(force_on_b).add(m_couple_on_b);

        BondForce {
            force_on_a,
            force_on_b,
            torque_on_a,
            torque_on_b,
        }
    }
}

/// Closed set of bond models, dispatched by `match` with **no** `dyn` / heap
/// allocation (per the workspace design rules). This is the per-pair bond state
/// a solver holds; call [`BondModel::update_bond`] on it each step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BondModel {
    /// A linear parallel bond (Potyondy & Cundall 2004) carrying force and
    /// moment until breakage.
    ParallelBond(Bond),
    /// No cohesive bond between the pair — transmits nothing. Provided so a
    /// caller can hold a uniform `BondModel` per pair and represent "unbonded"
    /// (or a bond that was never formed) without an `Option`.
    None,
}

impl BondModel {
    /// Advance the bond and return the force/torque on each particle, or
    /// [`BondForce::zero`] for [`BondModel::None`]. Dispatches to
    /// [`Bond::update_bond`].
    pub fn update_bond(&mut self, a: &Particle, b: &Particle, dt: f64) -> BondForce {
        match self {
            Self::ParallelBond(bond) => bond.update_bond(a, b, dt),
            Self::None => BondForce::zero(),
        }
    }

    /// Whether this bond transmits no load. `true` for a broken
    /// [`BondModel::ParallelBond`] and always `true` for [`BondModel::None`]
    /// (there is no ligament to transmit force).
    #[must_use]
    pub fn is_broken(&self) -> bool {
        match self {
            Self::ParallelBond(bond) => bond.is_broken(),
            Self::None => true,
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
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Helper: a particle at `position` with `velocity`, `angular_velocity`,
    /// mass `mass_kg`, radius `radius_m`, at 300 K.
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

    /// Helper: a valid parallel bond with the given `k_n`, `k_s` `[Pa/m]`,
    /// radius `r_m` `[m]`, and strengths `sig_c`, `tau_c` `[Pa]`.
    fn bond(k_n: f64, k_s: f64, r_m: f64, sig_c: f64, tau_c: f64) -> Bond {
        Bond::new(
            k_n,
            k_s,
            Length::new::<meter>(r_m),
            Pressure::new::<pascal>(sig_c),
            Pressure::new::<pascal>(tau_c),
        )
        .expect("valid bond")
    }

    /// V&V — **pure normal separation accumulates `F̄_n = k̄_n·A·δ_n`**.
    ///
    /// Methodology: a bond of radius `R̄ = 0.01 m` (area `A = π R̄² =
    /// 3.14159265e-4 m²`) with normal stiffness `k̄_n = 1e9 Pa/m` links two
    /// particles on the x-axis (so `n̂ = +x`). Particle `b` recedes along `+x` at
    /// `v = 1e-3 m/s` (pure separation, no rotation, no shear); `a` is at rest.
    /// Over `N = 100` steps of `dt = 1e-4 s` the accumulated normal displacement
    /// is `δ_n = v·N·dt = 1e-5 m`, so the parallel-bond law predicts
    /// `F̄_n = k̄_n·A·δ_n = 1e9 × 3.14159265e-4 × 1e-5 = π N`. The bond being in
    /// tension pulls `a` toward `b` (`+x`), so `force_on_a.x = +π N`, with no
    /// shear force and no moment. Strengths are set high so the bond does not
    /// break (`σ_max = F̄_n/A = 1e5 Pa ≪ σ_c = 1e12 Pa`). Pass criterion:
    /// `F̄_n = π N` and `force_on_a.x = +π N` to `1e-9 N`; shear and moments
    /// identically zero.
    ///
    /// Result (measured 2026-07-24): `F̄_n = 3.14159265358979 N`,
    /// `force_on_a = (+3.14159265…, 0, 0) N`, shear force and both moments `= 0`
    /// — matching the hand value `k̄_n·A·δ_n = π N` to round-off. Confirms the
    /// normal increment `ΔF̄_n = k̄_n·A·Δδ_n` accumulates correctly and that a
    /// tensile bond pulls the particles together.
    #[test]
    fn pure_normal_separation_accumulates_kn_a_delta() {
        let mut bnd = bond(1.0e9, 4.0e8, 0.01, 1.0e12, 1.0e12);
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(1.0e-3, 0.0, 0.0), // recede along +x → separation
            Vec3::zero(),
            1.0,
            0.1,
        );

        let dt = 1.0e-4;
        let n_steps = 100;
        let mut last = BondForce::zero();
        for _ in 0..n_steps {
            last = bnd.update_bond(&a, &b, dt);
        }

        let area = std::f64::consts::PI * 0.01 * 0.01;
        let delta_n = 1.0e-3 * dt * n_steps as f64; // = 1e-5 m
        let f_n_expected = 1.0e9 * area * delta_n; // = π N

        assert!(!bnd.is_broken());
        assert_abs_diff_eq!(bnd.normal_force(), f_n_expected, epsilon = 1.0e-9);
        assert_abs_diff_eq!(bnd.normal_force(), std::f64::consts::PI, epsilon = 1.0e-9);
        // Tensile bond pulls a toward b (+x).
        assert_abs_diff_eq!(last.force_on_a.x, std::f64::consts::PI, epsilon = 1.0e-9);
        assert_abs_diff_eq!(last.force_on_a.y, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(last.force_on_a.z, 0.0, epsilon = 1.0e-12);
        // No shear, no moment.
        assert_abs_diff_eq!(bnd.shear_force().norm(), 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(bnd.twist_moment(), 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(bnd.bend_moment().norm(), 0.0, epsilon = 1.0e-12);
    }

    /// V&V — **pure shear accumulates `|F̄_s| = k̄_s·A·δ_s`**.
    ///
    /// Methodology: the same bond (`R̄ = 0.01 m`, `A = 3.14159265e-4 m²`) with
    /// shear stiffness `k̄_s = 4e8 Pa/m`, `n̂ = +x`. Particle `b` slides along
    /// `+y` (perpendicular to `n̂`) at `v = 1e-3 m/s`, no rotation; `a` at rest.
    /// The relative motion is pure shear (`v_rel·n̂ = 0`), so over `N = 100`
    /// steps of `dt = 1e-4 s` the shear displacement is `δ_s = v·N·dt = 1e-5 m`
    /// and the law predicts `|F̄_s| = k̄_s·A·δ_s = 4e8 × 3.14159265e-4 × 1e-5 =
    /// 0.4π = 1.25663706 N`, with the accumulated shear force along `−y`
    /// (`F̄_s += −k̄_s·A·Δδ_s`) and the reaction on `a` dragging it along `+y`.
    /// The normal force stays zero. Pass criterion: `|F̄_s| = 0.4π N`,
    /// `shear_force.y = −0.4π N`, `force_on_a.y = +0.4π N` to `1e-9 N`; normal
    /// force zero.
    ///
    /// Result (measured 2026-07-24): `|F̄_s| = 1.25663706143592 N`,
    /// `shear_force = (0, −1.25663706…, 0) N`, `force_on_a.y = +1.25663706… N`,
    /// `F̄_n = 0` — matching the hand value `k̄_s·A·δ_s = 0.4π N` to round-off.
    /// Confirms the shear increment `ΔF̄_s = −k̄_s·A·Δδ_s` accumulates correctly.
    #[test]
    fn pure_shear_accumulates_ks_a_delta() {
        let mut bnd = bond(1.0e9, 4.0e8, 0.01, 1.0e12, 1.0e12);
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.0, 1.0e-3, 0.0), // slide along +y ⟂ n̂ → pure shear
            Vec3::zero(),
            1.0,
            0.1,
        );

        let dt = 1.0e-4;
        let n_steps = 100;
        let mut last = BondForce::zero();
        for _ in 0..n_steps {
            last = bnd.update_bond(&a, &b, dt);
        }

        let area = std::f64::consts::PI * 0.01 * 0.01;
        let delta_s = 1.0e-3 * dt * n_steps as f64; // = 1e-5 m
        let f_s_expected = 4.0e8 * area * delta_s; // = 0.4π N

        assert!(!bnd.is_broken());
        assert_abs_diff_eq!(bnd.shear_force().norm(), f_s_expected, epsilon = 1.0e-9);
        assert_abs_diff_eq!(
            bnd.shear_force().norm(),
            0.4 * std::f64::consts::PI,
            epsilon = 1.0e-9
        );
        // Accumulated shear force is along −y; reaction drags a along +y.
        assert_abs_diff_eq!(bnd.shear_force().y, -f_s_expected, epsilon = 1.0e-9);
        assert_abs_diff_eq!(last.force_on_a.y, f_s_expected, epsilon = 1.0e-9);
        // No normal force (motion ⟂ n̂).
        assert_abs_diff_eq!(bnd.normal_force(), 0.0, epsilon = 1.0e-9);
    }

    /// V&V — **an unstressed bond carries zero force and moment**.
    ///
    /// Methodology: a bond linking two particles that are both **at rest**
    /// (`v = 0`, `ω = 0`). One `update_bond` step must leave every accumulator at
    /// zero and return no force / no torque, and the bond must remain intact.
    ///
    /// Result (measured 2026-07-24): `F̄_n = 0`, `|F̄_s| = 0`, `M̄_t = 0`,
    /// `|M̄_b| = 0`, and `force_on_a = force_on_b = torque_on_a = torque_on_b =
    /// 0`; `is_broken() == false`. Confirms a quiescent bond adds no spurious
    /// load.
    #[test]
    fn unstressed_bond_carries_zero_load() {
        let mut bnd = bond(1.0e9, 4.0e8, 0.01, 1.0e6, 1.0e6);
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::zero(),
            Vec3::zero(),
            1.0,
            0.1,
        );

        let f = bnd.update_bond(&a, &b, 1.0e-3);

        assert!(!bnd.is_broken());
        assert_abs_diff_eq!(bnd.normal_force(), 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(bnd.shear_force().norm(), 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(bnd.twist_moment(), 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(bnd.bend_moment().norm(), 0.0, epsilon = 1.0e-15);
        assert_eq!(f, BondForce::zero());
    }

    /// V&V — **the bond breaks exactly when `σ_max` exceeds `σ_c`, and then
    /// transmits no force**.
    ///
    /// Methodology: bond `R̄ = 0.01 m` (`A = 3.14159265e-4 m²`), `k̄_n = 1e9
    /// Pa/m`, tensile strength `σ_c = 2.037e6 Pa`, driven in pure tension by `b`
    /// receding along `+x` at `v = 0.1 m/s`, `dt = 1e-4 s`. Each step adds
    /// `ΔF̄_n = k̄_n·A·(v·dt) = π N`, i.e. `Δσ = ΔF̄_n/A = 1e4 Pa` of tensile
    /// stress per step (there is no bending, so `σ_max = F̄_n/A`). Predictions:
    /// after `N = 100` steps `σ_max = 1.0e6 Pa < σ_c`, so the bond is **still
    /// intact**; the failure threshold is at `σ_c/Δσ = 2.037e6/1e4 = 203.7`
    /// steps, so by `N = 300` steps the bond has **broken**. Once broken it must
    /// return `BondForce::zero`. Pass criteria: `is_broken() == false` and
    /// `tensile_stress() = 1.0e6 Pa` at 100 steps; `is_broken() == true` by 300
    /// steps; a further `update_bond` returns all-zero force and torque.
    ///
    /// Result (measured 2026-07-24): at 100 steps `σ_max = 1.000000e6 Pa`
    /// (matching `100·Δσ`) and the bond is intact; the bond breaks at step 204
    /// (first `σ_max = 2.04e6 Pa > σ_c = 2.037e6 Pa`), consistent with the
    /// `203.7`-step hand prediction; after breaking, `update_bond` returns
    /// `force_on_a = force_on_b = torque_on_a = torque_on_b = 0`. Confirms the
    /// tensile breakage criterion `σ_max > σ_c` and the zero-load-after-failure
    /// behaviour.
    #[test]
    fn bond_breaks_when_tensile_stress_exceeds_strength() {
        let mut bnd = bond(1.0e9, 4.0e8, 0.01, 2.037e6, 1.0e12);
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0), // strong pure-tension drive
            Vec3::zero(),
            1.0,
            0.1,
        );
        let dt = 1.0e-4;

        // 100 steps: still intact, σ_max = 1.0 MPa (hand value).
        for _ in 0..100 {
            bnd.update_bond(&a, &b, dt);
        }
        assert!(!bnd.is_broken());
        assert_abs_diff_eq!(bnd.tensile_stress(), 1.0e6, epsilon = 1.0);

        // Continue to 300 steps total: the bond must have broken by now.
        for _ in 0..200 {
            bnd.update_bond(&a, &b, dt);
        }
        assert!(bnd.is_broken());

        // A broken bond transmits no force or moment.
        let f = bnd.update_bond(&a, &b, dt);
        assert_eq!(f, BondForce::zero());
    }

    /// V&V — **the two particles feel equal-and-opposite force (Newton's third
    /// law)**.
    ///
    /// Methodology: a bond under a rich combined state — `b` translating
    /// `(0.2, 0.3, 0.1) m/s` and spinning `(0.1, 0.05, 0.2) rad/s` while `a` is
    /// at rest — is advanced one step (`dt = 1e-3 s`) so it carries simultaneous
    /// normal, shear, bending, and twisting load. The resolved `force_on_b` must
    /// equal `−force_on_a` componentwise, and the load must be non-trivial
    /// (nonzero). Pass criterion: `force_on_b = −force_on_a` to `1e-15 N`, and
    /// `‖force_on_a‖ > 0`.
    ///
    /// Result (measured 2026-07-24): `force_on_b = −force_on_a` to `< 1e-15 N` on
    /// every component, with `‖force_on_a‖ > 0`. Confirms the action–reaction
    /// pairing of the bond force. (The torques are intentionally **not** asserted
    /// equal-and-opposite: each is the moment of the bond force about a different
    /// centre plus that particle's half of the internal couple, so they are not
    /// mirror images in general — see [`BondForce`].)
    #[test]
    fn bond_force_obeys_newtons_third_law() {
        let mut bnd = bond(1.0e9, 4.0e8, 0.01, 1.0e12, 1.0e12);
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.2, 0.3, 0.1),
            Vec3::new(0.1, 0.05, 0.2),
            1.0,
            0.1,
        );

        let f = bnd.update_bond(&a, &b, 1.0e-3);

        assert!(!bnd.is_broken());
        // Non-trivial load.
        assert!(f.force_on_a.norm() > 0.0);
        // Newton's third law on the force.
        assert_abs_diff_eq!(f.force_on_b.x, -f.force_on_a.x, epsilon = 1.0e-15);
        assert_abs_diff_eq!(f.force_on_b.y, -f.force_on_a.y, epsilon = 1.0e-15);
        assert_abs_diff_eq!(f.force_on_b.z, -f.force_on_a.z, epsilon = 1.0e-15);
    }

    /// Geometry helpers match the closed forms for `R̄ = 0.01 m`:
    /// `A = π R̄² = 3.14159265e-4 m²`, `I = π R̄⁴/4 = 7.85398163e-9 m⁴`,
    /// `J = π R̄⁴/2 = 1.57079633e-8 m⁴`, and `J = 2·I`.
    #[test]
    fn bond_geometry_matches_closed_form() {
        let bnd = bond(1.0e9, 4.0e8, 0.01, 1.0e12, 1.0e12);
        assert_abs_diff_eq!(bnd.area(), 3.141592653589793e-4, epsilon = 1.0e-15);
        assert_abs_diff_eq!(
            bnd.bending_inertia(),
            7.853981633974483e-9,
            epsilon = 1.0e-20
        );
        assert_abs_diff_eq!(
            bnd.polar_inertia(),
            1.5707963267948966e-8,
            epsilon = 1.0e-20
        );
        assert_abs_diff_eq!(
            bnd.polar_inertia(),
            2.0 * bnd.bending_inertia(),
            epsilon = 1.0e-20
        );
    }

    /// Constructor validation: non-positive stiffness, radius, or strength is
    /// rejected with [`DemError::InvalidInput`]; a fully valid set succeeds.
    #[test]
    fn constructor_rejects_invalid_parameters() {
        let r = Length::new::<meter>(0.01);
        let s = Pressure::new::<pascal>(1.0e6);
        // k_n ≤ 0.
        assert!(Bond::new(0.0, 4.0e8, r, s, s).is_err());
        // k_s ≤ 0.
        assert!(Bond::new(1.0e9, -1.0, r, s, s).is_err());
        // radius ≤ 0.
        assert!(Bond::new(1.0e9, 4.0e8, Length::new::<meter>(0.0), s, s).is_err());
        // σ_c ≤ 0.
        assert!(Bond::new(1.0e9, 4.0e8, r, Pressure::new::<pascal>(0.0), s).is_err());
        // τ_c ≤ 0.
        assert!(Bond::new(1.0e9, 4.0e8, r, s, Pressure::new::<pascal>(-1.0)).is_err());
        // All valid.
        assert!(Bond::new(1.0e9, 4.0e8, r, s, s).is_ok());
    }

    /// [`BondModel::None`] transmits nothing and reports itself broken; the
    /// `ParallelBond` variant dispatches to the underlying [`Bond`].
    #[test]
    fn bond_model_enum_dispatch() {
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.1);
        let b = particle(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(1.0e-3, 0.0, 0.0),
            Vec3::zero(),
            1.0,
            0.1,
        );

        let mut none = BondModel::None;
        assert!(none.is_broken());
        assert_eq!(none.update_bond(&a, &b, 1.0e-4), BondForce::zero());

        let mut pb = BondModel::ParallelBond(bond(1.0e9, 4.0e8, 0.01, 1.0e12, 1.0e12));
        assert!(!pb.is_broken());
        let f = pb.update_bond(&a, &b, 1.0e-4);
        assert!(f.force_on_a.norm() > 0.0);
    }
}
