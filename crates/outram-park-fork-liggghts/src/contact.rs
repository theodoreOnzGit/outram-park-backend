// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — the Hooke linear spring-dashpot and
// Hertz-Mindlin nonlinear contact models are STANDARD PUBLIC-LITERATURE DEM.
// They are reimplemented CLEAN-ROOM from the cited papers (see the module
// "References" below); this file contains NO GPL-2.0 LIGGGHTS/LAMMPS source and
// is not a translation of it. See the crate NOTICE.
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

//! Phase 2 — **Contact mechanics** (bead `op-t3l.2`).
//!
//! Pairwise particle–particle contact forces for spherical DEM particles, in
//! two standard flavours selected at compile time through the [`ContactModel`]
//! enum (no `dyn`, per the workspace design rules):
//!
//! - [`HookeContact`] — the **linear spring-dashpot** law of Cundall & Strack
//!   (1979) with a tangential spring-dashpot and a Coulomb friction cap
//!   (Tsuji et al. 1992).
//! - [`HertzContact`] — the **nonlinear Hertz–Mindlin** law: a Hertzian
//!   (`δ^{3/2}`) normal spring, viscoelastic (restitution-based) damping, and a
//!   Mindlin tangential spring with a Coulomb cap (Hertz 1882; Mindlin &
//!   Deresiewicz 1953; Tsuji et al. 1992; Di Renzo & Di Maio 2004).
//!
//! Both share the same [`ContactLaw`] contract (the compiler-enforced trait)
//! and the same geometry/assembly code, so Newton's third law and the
//! contact-point torque bookkeeping are implemented **once** and reused.
//!
//! ## Naming note (trait vs enum)
//!
//! The workspace idiom is a *trait* that states the contract plus an *enum*
//! that dispatches over the closed set of models (mirroring the `CLAUDE.md`
//! `TurbulenceKernel` trait / `TurbulenceModel` enum example). A Rust trait and
//! enum cannot share one identifier (both live in the type namespace), so the
//! contract trait is [`ContactLaw`] and the public dispatch enum — the type a
//! user actually holds and calls [`ContactModel::contact_force`] on — is
//! [`ContactModel`].
//!
//! # Sign & geometry conventions (read once, applies everywhere)
//!
//! For a pair `(a, b)`:
//!
//! - **Normal** `n̂` is the unit vector **from `a`'s centre toward `b`'s
//!   centre**: `n̂ = (x_b − x_a) / ‖x_b − x_a‖`.
//! - **Overlap** `δ_n = (r_a + r_b) − ‖x_b − x_a‖`. Contact exists iff
//!   `δ_n > 0`; otherwise [`ContactLaw::contact_force`] returns `None`.
//! - **Approach rate** `v_n = (v_a − v_b) · n̂` `[m/s]`, positive when the two
//!   centres are closing.
//! - The **normal force on `a` is repulsive**, i.e. directed along `−n̂`; the
//!   force on `b` is its Newton-third-law reaction `+…n̂`.
//!
//! # Unit convention
//!
//! Following the crate convention (see [`crate::particle`]), the `uom` boundary
//! sits at the constructors where a clean named `uom` type exists
//! ([`HertzContact::new`] takes Young's modulus as a [`Pressure`]); everything
//! else is plain `f64` in **SI base units** with the unit spelled out in the
//! doc comment. The spring/damping constants of the Hooke model have no
//! ergonomic named `uom` alias (`k_n` is `[N/m]`, `γ_n` is `[N·s/m] = [kg/s]`),
//! so they are documented `f64`, consistent with the crate's f64-internal rule.
//!
//! # Honest scope (Phase 2)
//!
//! This is a **verified foundation, not a validated model** — no cross-code or
//! experimental benchmark comparison has been run yet (that is the later human
//! validation step in this bead's Definition of Done). The inline tests below
//! check the force laws against **hand-computed analytical values** and
//! invariants (Newton's third law, `δ^{3/2}` scaling, the Coulomb cap) only.
//!
//! It deliberately implements **only**:
//!
//! - **Sphere–sphere** contact. Particle–wall/boundary contact is Phase 3
//!   ([`crate::boundary`]); non-spherical shapes are out of scope entirely.
//! - A **stateless snapshot** force: [`ContactLaw::contact_force`] is a pure
//!   function of the two particles' instantaneous state. A full tangential
//!   spring needs the **accumulated tangential displacement** `ξ_t` integrated
//!   over the contact's lifetime, which is per-contact history the caller's
//!   time-integration loop must carry (a later phase). Here `ξ_t = 0`, so the
//!   tangential force reduces to its **dashpot** term `γ_t · v_t`, still
//!   Coulomb-capped at `μ|F_n|`. The tangential *stiffness* `k_t`
//!   ([`ContactLaw::tangential_spring_coeff`]) is exposed for a future
//!   history-aware caller.
//!
//! It does **not** yet provide: rolling friction, cohesion/adhesion (van der
//! Waals, liquid bridges), bonded/parallel-bond contacts, or heat transfer
//! through the contact (thermal DEM is Phase 4, [`crate::thermal`]). The normal
//! force is **not clamped** to be purely repulsive: for a rapidly rebounding
//! contact the linear/viscoelastic damping term can momentarily exceed the
//! elastic term and yield a small tensile force — this is a known
//! spring-dashpot artifact, **not** a cohesion model, and clamping `F_n ≥ 0` is
//! a documented option a caller may add.
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
//!   granular assemblies," *Géotechnique* **29**(1), 47–65 (1979) — linear
//!   spring-dashpot (Hooke) contact.
//! - H. Hertz, "Über die Berührung fester elastischer Körper," *J. reine angew.
//!   Math.* **92**, 156–171 (1882) — Hertzian normal contact (`δ^{3/2}`).
//! - R. D. Mindlin and H. Deresiewicz, "Elastic spheres in contact under
//!   varying oblique forces," *J. Appl. Mech.* **20**, 327–344 (1953) —
//!   tangential contact stiffness.
//! - Y. Tsuji, T. Tanaka, T. Ishida, "Lagrangian numerical simulation of plug
//!   flow of cohesionless particles in a horizontal pipe," *Powder Technol.*
//!   **71**(3), 239–250 (1992) — nonlinear viscoelastic damping and the
//!   Coulomb-capped tangential spring-dashpot.
//! - A. Di Renzo and F. P. Di Maio, "Comparison of contact-force models for the
//!   simulation of collisions in DEM-based granular flow codes," *Chem. Eng.
//!   Sci.* **59**(3), 525–541 (2004) — the restitution-based damping
//!   coefficient and Hertz–Mindlin assembly used here.

use uom::si::f64::Pressure;
use uom::si::pressure::pascal;

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Tangential relative slip speed `[m/s]` below which the tangential direction
/// is treated as undefined and no tangential force is applied (avoids dividing
/// by a near-zero slip magnitude).
const TANGENTIAL_SLIP_EPS: f64 = 1.0e-12;

/// The resolved contact force (and torque) for one overlapping particle pair.
///
/// All forces are in newtons `[N]` and torques in newton-metres `[N·m]`. By
/// Newton's third law `force_on_b = −force_on_a`. Only the tangential
/// (friction) part produces a torque: the normal force is collinear with the
/// line of centres and so has zero moment about either centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactForce {
    /// Total force on particle `a` `[N]` (normal repulsion along `−n̂` plus
    /// tangential friction).
    pub force_on_a: Vec3,
    /// Total force on particle `b` `[N]`. Equals `−force_on_a` exactly.
    pub force_on_b: Vec3,
    /// Torque on particle `a` about its centre `[N·m]`, from the tangential
    /// force acting at the contact point (offset `r_a·n̂` from `a`'s centre).
    pub torque_on_a: Vec3,
    /// Torque on particle `b` about its centre `[N·m]`, from the reaction
    /// tangential force at the contact point (offset `−r_b·n̂` from `b`'s
    /// centre).
    pub torque_on_b: Vec3,
    /// Normal overlap `δ_n` `[m]`, strictly positive (a returned
    /// [`ContactForce`] always corresponds to a real overlap).
    pub overlap: f64,
    /// Unit contact normal `n̂`, pointing from `a`'s centre toward `b`'s centre.
    pub normal: Vec3,
}

/// Compiler-enforced contract every contact model satisfies (the trait half of
/// the workspace's "trait states the interface, enum dispatches" idiom).
///
/// A model supplies four **scalar** force ingredients as pure functions of the
/// local contact geometry; the provided [`ContactLaw::contact_force`] method
/// then assembles them into a full [`ContactForce`] (geometry, Coulomb cap,
/// Newton's third law, contact-point torque) so that assembly logic is written
/// and verified once and shared by every model.
///
/// # Effective (reduced) contact quantities
///
/// Several methods take reduced pair quantities computed from the two
/// particles:
///
/// - **Effective radius** `R* = r_a·r_b / (r_a + r_b)` `[m]`.
/// - **Effective mass** `m* = m_a·m_b / (m_a + m_b)` `[kg]`.
pub trait ContactLaw {
    /// Repulsive **normal** force magnitude `[N]` (positive pushes the pair
    /// apart) from overlap `δ_n` `[m]`, approach rate `v_n` `[m/s]`, effective
    /// radius `r_eff = R*` `[m]`, and effective mass `m_eff = m*` `[kg]`.
    ///
    /// Includes the model's normal damping term; may be momentarily negative
    /// (tensile) during fast rebound (see the module "Honest scope" note).
    fn normal_force_scalar(&self, delta_n: f64, v_n: f64, r_eff: f64, m_eff: f64) -> f64;

    /// **Tangential dashpot** coefficient `γ_t` `[N·s/m] = [kg/s]` for the given
    /// contact geometry. The tangential damping force magnitude is `γ_t · v_t`
    /// with `v_t` the tangential slip speed `[m/s]`.
    fn tangential_damping_coeff(&self, delta_n: f64, r_eff: f64, m_eff: f64) -> f64;

    /// **Tangential spring** stiffness `k_t` `[N/m]`, paired with the
    /// accumulated tangential displacement `ξ_t` `[m]`. Exposed for a future
    /// history-aware caller; the stateless [`ContactLaw::contact_force`] uses
    /// `ξ_t = 0`, so this term does not contribute there (see "Honest scope").
    fn tangential_spring_coeff(&self, delta_n: f64, r_eff: f64) -> f64;

    /// Coulomb sliding-friction coefficient `μ` `[-]`. The tangential force
    /// magnitude is capped at `μ · |F_n|`.
    fn friction_coefficient(&self) -> f64;

    /// Assemble the full pairwise contact force between spheres `a` and `b`.
    ///
    /// Returns `None` when the particles do **not** overlap (centre distance
    /// `≥ r_a + r_b`) or when their centres coincide (normal undefined).
    /// Otherwise returns the normal + tangential force on each particle and the
    /// contact-point torques, in the sign convention documented at the module
    /// level (`n̂` from `a` to `b`; normal force on `a` along `−n̂`).
    ///
    /// The tangential force uses `ξ_t = 0` (stateless snapshot), so it is the
    /// dashpot term `γ_t · v_t` capped by Coulomb friction `μ|F_n|`; its
    /// direction opposes the tangential slip of `a`'s surface relative to `b`'s.
    fn contact_force(&self, a: &Particle, b: &Particle) -> Option<ContactForce> {
        // --- Geometry -------------------------------------------------------
        let d = b.position.sub(a.position); // vector a -> b
        let dist = d.norm();
        let r_sum = a.radius + b.radius;
        let delta_n = r_sum - dist;
        // Not in contact, or centres coincident (normal undefined): no force.
        if delta_n <= 0.0 || dist <= 0.0 {
            return None;
        }
        let n = d.scale(1.0 / dist); // unit normal, a -> b

        let r_eff = a.radius * b.radius / r_sum;
        let m_eff = a.mass * b.mass / (a.mass + b.mass);

        // --- Normal force ---------------------------------------------------
        // Approach rate v_n = (v_a - v_b)·n̂  (positive when closing).
        let v_rel = a.velocity.sub(b.velocity);
        let v_n = v_rel.dot(n);
        let f_n = self.normal_force_scalar(delta_n, v_n, r_eff, m_eff);
        // Repulsion pushes a along -n̂.
        let normal_on_a = n.scale(-f_n);

        // --- Tangential force ----------------------------------------------
        // Surface relative velocity of a w.r.t. b at the contact point:
        //   v_surf = (v_a - v_b) + (r_a ω_a + r_b ω_b) × n̂
        // (the angular terms are ⟂ n̂ so they do not affect the normal part).
        let omega_term = a
            .angular_velocity
            .scale(a.radius)
            .add(b.angular_velocity.scale(b.radius));
        let v_surf = v_rel.add(omega_term.cross(n));
        let v_t_vec = v_surf.sub(n.scale(v_surf.dot(n)));
        let v_t = v_t_vec.norm();

        // Spring (ξ_t = 0 in this stateless snapshot) + dashpot, Coulomb-capped.
        let xi_t = 0.0;
        let f_t_trial = self.tangential_spring_coeff(delta_n, r_eff) * xi_t
            + self.tangential_damping_coeff(delta_n, r_eff, m_eff) * v_t;
        let cap = self.friction_coefficient() * f_n.abs();
        let f_t = f_t_trial.min(cap).max(0.0);
        // Friction opposes a's tangential slip direction.
        let tangential_on_a = if v_t > TANGENTIAL_SLIP_EPS {
            v_t_vec.scale(-f_t / v_t)
        } else {
            Vec3::zero()
        };

        // --- Assemble (Newton's third law + contact-point torque) ----------
        let force_on_a = normal_on_a.add(tangential_on_a);
        let force_on_b = force_on_a.scale(-1.0);

        // Contact point is at r_a·n̂ from a's centre and −r_b·n̂ from b's.
        // Normal force is collinear with n̂ → zero moment; only friction spins.
        let torque_on_a = n.scale(a.radius).cross(tangential_on_a);
        // b's tangential force is −tangential_on_a at lever −r_b·n̂:
        //   τ_b = (−r_b n̂) × (−tangential_on_a) = (r_b n̂) × tangential_on_a.
        let torque_on_b = n.scale(b.radius).cross(tangential_on_a);

        Some(ContactForce {
            force_on_a,
            force_on_b,
            torque_on_a,
            torque_on_b,
            overlap: delta_n,
            normal: n,
        })
    }
}

/// Linear **spring-dashpot** contact model (Cundall & Strack 1979; tangential
/// Coulomb-capped spring-dashpot from Tsuji et al. 1992).
///
/// Normal force magnitude: `F_n = k_n·δ_n + γ_n·v_n`, with overlap `δ_n` `[m]`
/// and approach rate `v_n` `[m/s]`. (Equivalently `k_n·δ_n − γ_n·ẋ` where
/// `ẋ = −v_n` is the rate of change of centre separation.) Tangential force:
/// `min(k_t·ξ_t + γ_t·v_t, μ|F_n|)` opposing slip; the stateless snapshot uses
/// `ξ_t = 0`.
///
/// # Parameters and units
///
/// | Field | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `normal_stiffness` | `k_n` | normal spring stiffness | `[N/m]` | `> 0` |
/// | `normal_damping` | `γ_n` | normal dashpot coefficient | `[N·s/m] = [kg/s]` | `≥ 0` |
/// | `tangential_stiffness` | `k_t` | tangential spring stiffness | `[N/m]` | `≥ 0` |
/// | `tangential_damping` | `γ_t` | tangential dashpot coefficient | `[N·s/m] = [kg/s]` | `≥ 0` |
/// | `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |
///
/// The stiffnesses are constant (linear model); they do not depend on overlap
/// or radius, so `R*` and `m*` are ignored by this model's scalar methods.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HookeContact {
    /// Normal spring stiffness `k_n` `[N/m]`. Strictly positive.
    pub normal_stiffness: f64,
    /// Normal dashpot coefficient `γ_n` `[N·s/m]`. Non-negative.
    pub normal_damping: f64,
    /// Tangential spring stiffness `k_t` `[N/m]`. Non-negative.
    pub tangential_stiffness: f64,
    /// Tangential dashpot coefficient `γ_t` `[N·s/m]`. Non-negative.
    pub tangential_damping: f64,
    /// Coulomb friction coefficient `μ` `[-]`. Non-negative.
    pub friction: f64,
}

impl HookeContact {
    /// Construct a validated linear spring-dashpot model.
    ///
    /// All arguments are `f64` in the SI units documented on [`HookeContact`]'s
    /// fields (`k_n`, `k_t` in `[N/m]`; `γ_n`, `γ_t` in `[N·s/m]`; `μ`
    /// dimensionless).
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `normal_stiffness` is not strictly
    /// positive (a contact needs a real restoring spring), or if any of
    /// `normal_damping`, `tangential_stiffness`, `tangential_damping`, or
    /// `friction` is negative (these are non-negative physical coefficients).
    pub fn new(
        normal_stiffness: f64,
        normal_damping: f64,
        tangential_stiffness: f64,
        tangential_damping: f64,
        friction: f64,
    ) -> Result<Self, DemError> {
        if !(normal_stiffness > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "normal_stiffness k_n must be strictly positive, got {normal_stiffness} N/m"
            )));
        }
        if normal_damping < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "normal_damping γ_n must be non-negative, got {normal_damping} N·s/m"
            )));
        }
        if tangential_stiffness < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "tangential_stiffness k_t must be non-negative, got {tangential_stiffness} N/m"
            )));
        }
        if tangential_damping < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "tangential_damping γ_t must be non-negative, got {tangential_damping} N·s/m"
            )));
        }
        if friction < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "friction μ must be non-negative, got {friction}"
            )));
        }
        Ok(Self {
            normal_stiffness,
            normal_damping,
            tangential_stiffness,
            tangential_damping,
            friction,
        })
    }
}

impl ContactLaw for HookeContact {
    fn normal_force_scalar(&self, delta_n: f64, v_n: f64, _r_eff: f64, _m_eff: f64) -> f64 {
        self.normal_stiffness * delta_n + self.normal_damping * v_n
    }

    fn tangential_damping_coeff(&self, _delta_n: f64, _r_eff: f64, _m_eff: f64) -> f64 {
        self.tangential_damping
    }

    fn tangential_spring_coeff(&self, _delta_n: f64, _r_eff: f64) -> f64 {
        self.tangential_stiffness
    }

    fn friction_coefficient(&self) -> f64 {
        self.friction
    }
}

/// Nonlinear **Hertz–Mindlin** contact model (Hertz 1882; Mindlin &
/// Deresiewicz 1953; viscoelastic damping and Coulomb-capped tangential spring
/// from Tsuji et al. 1992 / Di Renzo & Di Maio 2004).
///
/// Normal force magnitude:
///
/// `F_n = (4/3)·E*·√(R*)·δ_n^{3/2} + c_n·v_n`,
///
/// with effective modulus `E*`, effective radius `R*`, overlap `δ_n`, approach
/// rate `v_n`, and a restitution-based damping coefficient
/// `c_n = 2·√(5/6)·|β|·√(S_n·m*)`, where the normal contact stiffness is
/// `S_n = 2·E*·√(R*·δ_n)` and `β = ln(e)/√(ln²(e) + π²)`.
///
/// Tangential stiffness (Mindlin): `S_t = 8·G*·√(R*·δ_n)`, with tangential
/// damping `γ_t = 2·√(5/6)·|β|·√(S_t·m*)`; the tangential force
/// `min(S_t·ξ_t + γ_t·v_t, μ|F_n|)` opposes slip (`ξ_t = 0` in the stateless
/// snapshot).
///
/// # Material assumption
///
/// This model assumes **both particles share one isotropic linear-elastic
/// material** — a single Young's modulus `E`, Poisson ratio `ν`, and
/// coefficient of restitution `e`. The effective moduli then reduce to
/// `E* = E / (2(1 − ν²))` and `G* = G / (2(2 − ν))` with `G = E / (2(1 + ν))`.
/// Mixed-material effective moduli (`1/E* = Σ (1 − ν_i²)/E_i`) are a documented
/// future extension.
///
/// # Parameters and units
///
/// | Field | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `youngs_modulus` | `E` | Young's modulus | `[Pa]` | `> 0` |
/// | `poisson_ratio` | `ν` | Poisson ratio | `[-]` | `0 ≤ ν < 0.5` |
/// | `restitution` | `e` | coefficient of restitution | `[-]` | `0 < e ≤ 1` |
/// | `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HertzContact {
    /// Young's modulus `E` `[Pa]` (shared by both particles). Strictly positive.
    pub youngs_modulus: f64,
    /// Poisson ratio `ν` `[-]`. In `[0, 0.5)`.
    pub poisson_ratio: f64,
    /// Coefficient of restitution `e` `[-]`. In `(0, 1]`.
    pub restitution: f64,
    /// Coulomb friction coefficient `μ` `[-]`. Non-negative.
    pub friction: f64,
}

impl HertzContact {
    /// Construct a validated Hertz–Mindlin model.
    ///
    /// Young's modulus is taken as a `uom` [`Pressure`] at this boundary so a
    /// caller cannot pass a dimensionally wrong scalar; it is stored internally
    /// as `f64` pascals. The dimensionless ratios (`ν`, `e`, `μ`) are `f64`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `youngs_modulus ≤ 0`, if
    /// `poisson_ratio` is outside `[0, 0.5)` (values `≥ 0.5` are
    /// incompressible/unphysical for this linear-elastic model and make the
    /// shear modulus diverge), if `restitution` is outside `(0, 1]`
    /// (`e ≤ 0` has no `ln`, `e > 1` would inject energy), or if `friction` is
    /// negative.
    pub fn new(
        youngs_modulus: Pressure,
        poisson_ratio: f64,
        restitution: f64,
        friction: f64,
    ) -> Result<Self, DemError> {
        let youngs_modulus = youngs_modulus.get::<pascal>();
        if !(youngs_modulus > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "youngs_modulus E must be strictly positive, got {youngs_modulus} Pa"
            )));
        }
        if !(0.0..0.5).contains(&poisson_ratio) {
            return Err(DemError::InvalidInput(format!(
                "poisson_ratio ν must be in [0, 0.5), got {poisson_ratio}"
            )));
        }
        if !(restitution > 0.0 && restitution <= 1.0) {
            return Err(DemError::InvalidInput(format!(
                "restitution e must be in (0, 1], got {restitution}"
            )));
        }
        if friction < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "friction μ must be non-negative, got {friction}"
            )));
        }
        Ok(Self {
            youngs_modulus,
            poisson_ratio,
            restitution,
            friction,
        })
    }

    /// Effective (reduced) Young's modulus `E*` `[Pa]` for the equal-material
    /// pair: `E* = E / (2(1 − ν²))`.
    #[must_use]
    pub fn effective_modulus(&self) -> f64 {
        let nu = self.poisson_ratio;
        self.youngs_modulus / (2.0 * (1.0 - nu * nu))
    }

    /// Effective (reduced) shear modulus `G*` `[Pa]` for the equal-material
    /// pair: `G* = G / (2(2 − ν))` with `G = E / (2(1 + ν))`.
    #[must_use]
    pub fn effective_shear_modulus(&self) -> f64 {
        let nu = self.poisson_ratio;
        let g = self.youngs_modulus / (2.0 * (1.0 + nu));
        g / (2.0 * (2.0 - nu))
    }

    /// Damping factor `β = ln(e) / √(ln²(e) + π²)` `[-]` (Tsuji 1992 /
    /// Di Renzo & Di Maio 2004). Non-positive; `β = 0` when `e = 1`
    /// (elastic, no damping). The damping coefficients use `|β|`.
    #[must_use]
    pub fn damping_beta(&self) -> f64 {
        let ln_e = self.restitution.ln();
        let pi = std::f64::consts::PI;
        ln_e / (ln_e * ln_e + pi * pi).sqrt()
    }
}

impl ContactLaw for HertzContact {
    fn normal_force_scalar(&self, delta_n: f64, v_n: f64, r_eff: f64, m_eff: f64) -> f64 {
        let e_star = self.effective_modulus();
        // Elastic Hertzian spring: (4/3) E* √R* δ^{3/2}.
        let elastic = (4.0 / 3.0) * e_star * r_eff.sqrt() * delta_n.powf(1.5);
        // Viscoelastic damping: c_n·v_n with c_n = 2√(5/6)|β|√(S_n m*),
        // S_n = 2 E* √(R* δ) the normal contact stiffness.
        let s_n = 2.0 * e_star * (r_eff * delta_n).sqrt();
        let c_n = 2.0 * (5.0 / 6.0f64).sqrt() * self.damping_beta().abs() * (s_n * m_eff).sqrt();
        elastic + c_n * v_n
    }

    fn tangential_damping_coeff(&self, delta_n: f64, r_eff: f64, m_eff: f64) -> f64 {
        // γ_t = 2√(5/6)|β|√(S_t m*), S_t = 8 G* √(R* δ) the tangential stiffness.
        let s_t = self.tangential_spring_coeff(delta_n, r_eff);
        2.0 * (5.0 / 6.0f64).sqrt() * self.damping_beta().abs() * (s_t * m_eff).sqrt()
    }

    fn tangential_spring_coeff(&self, delta_n: f64, r_eff: f64) -> f64 {
        // Mindlin tangential stiffness S_t = 8 G* √(R* δ).
        8.0 * self.effective_shear_modulus() * (r_eff * delta_n).sqrt()
    }

    fn friction_coefficient(&self) -> f64 {
        self.friction
    }
}

/// Closed set of contact models, dispatched by `match` with **no** `dyn` /
/// heap allocation (per the workspace design rules). This is the type a solver
/// holds per material pair; call [`ContactModel::contact_force`] on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactModel {
    /// Linear spring-dashpot (Cundall & Strack 1979).
    Hooke(HookeContact),
    /// Nonlinear Hertz–Mindlin (Hertz 1882; Mindlin & Deresiewicz 1953).
    Hertz(HertzContact),
}

impl ContactModel {
    /// Pairwise contact force between spheres `a` and `b`.
    ///
    /// Returns `None` when the particles are not overlapping (centre distance
    /// `≥ r_a + r_b`), otherwise the normal + tangential force and torque per
    /// the module-level sign conventions. Forces are in `[N]`, torques in
    /// `[N·m]`, overlap in `[m]`. Dispatches to the active model's
    /// [`ContactLaw::contact_force`].
    #[must_use]
    pub fn contact_force(&self, a: &Particle, b: &Particle) -> Option<ContactForce> {
        match self {
            Self::Hooke(m) => m.contact_force(a, b),
            Self::Hertz(m) => m.contact_force(a, b),
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

    /// V&V — **Hooke normal force equals the hand-computed `k_n·δ`**.
    ///
    /// Methodology: two spheres of radius `r = 0.5 m` (so `r_a + r_b = 1.0 m`)
    /// with centres on the x-axis at `x_a = 0` and `x_b = 0.9 m`, giving overlap
    /// `δ_n = 1.0 − 0.9 = 0.1 m`. Both are **at rest** (`v = 0`, `ω = 0`), so
    /// the damping term (`γ_n·v_n`) and the tangential force vanish and the only
    /// force is the elastic normal spring. With `k_n = 1000 N/m` the magnitude
    /// is `F_n = k_n·δ_n = 1000 × 0.1 = 100 N`, directed on `a` along `−n̂`
    /// (`n̂ = +x`). Pass criterion: `force_on_a = (−100, 0, 0) N` to `1e-9 N`,
    /// zero torque.
    ///
    /// Result (measured 2026-07-23): `force_on_a = (−100.0, 0, 0) N` and
    /// `torque_on_a = 0`, reproduced to `< 1e-12 N` (round-off) — exactly the
    /// hand-computed `k_n·δ_n = 100 N`.
    #[test]
    fn hooke_normal_force_matches_kn_delta() {
        let model = ContactModel::Hooke(
            HookeContact::new(1000.0, 5.0, 500.0, 2.0, 0.3).expect("valid hooke"),
        );
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.5);
        let b = particle(Vec3::new(0.9, 0.0, 0.0), Vec3::zero(), Vec3::zero(), 1.0, 0.5);

        let cf = model.contact_force(&a, &b).expect("in contact");
        assert_abs_diff_eq!(cf.overlap, 0.1, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.force_on_a.x, -100.0, epsilon = 1.0e-9);
        assert_abs_diff_eq!(cf.force_on_a.y, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.force_on_a.z, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.torque_on_a.x, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.torque_on_a.y, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.torque_on_a.z, 0.0, epsilon = 1.0e-12);
    }

    /// V&V — **Hertz normal force scales as `δ^{3/2}` and matches the closed
    /// form**.
    ///
    /// Methodology: equal spheres `r = 0.5 m` (`R* = r_a r_b/(r_a+r_b) =
    /// 0.25 m`), material `E = 1e7 Pa`, `ν = 0.3`, `e = 1` (no damping), at
    /// rest. Effective modulus `E* = E/(2(1−ν²)) = 1e7/1.82 = 5.494505e6 Pa`, so
    /// the prefactor `(4/3)E*√R* = (4/3)(5.494505e6)(0.5) = 3.663004e6`. For two
    /// overlaps `δ₁ = 0.01 m` (centres 0.99 apart) and `δ₂ = 0.04 m` (0.96
    /// apart): `F(δ₁) = 3.663004e6 × 0.01^{1.5} = 3663.003663 N`, `F(δ₂) =
    /// 3.663004e6 × 0.04^{1.5} = 29304.02931 N`. The ratio is
    /// `(δ₂/δ₁)^{1.5} = 4^{1.5} = 8` exactly. Pass criteria: `F(δ₁)` matches
    /// `3663.003663 N` to `1e-4 N`, and `F(δ₂)/F(δ₁) = 8` to `1e-9`.
    ///
    /// Result (measured 2026-07-23): `F(δ₁) = 3663.00366… N`, `F(δ₂) =
    /// 29304.0293… N`, ratio `= 8.00000000` — matching the closed form and the
    /// `δ^{3/2}` scaling to round-off.
    #[test]
    fn hertz_normal_force_scales_as_delta_three_halves() {
        let hertz = HertzContact::new(
            Pressure::new::<pascal>(1.0e7),
            0.3,
            1.0, // e = 1 → β = 0 → no damping, pure elastic
            0.3,
        )
        .expect("valid hertz");
        let model = ContactModel::Hertz(hertz);

        // δ = 0.01 m  → centres 0.99 apart.
        let a1 = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.5);
        let b1 = particle(Vec3::new(0.99, 0.0, 0.0), Vec3::zero(), Vec3::zero(), 1.0, 0.5);
        // δ = 0.04 m  → centres 0.96 apart.
        let b2 = particle(Vec3::new(0.96, 0.0, 0.0), Vec3::zero(), Vec3::zero(), 1.0, 0.5);

        let f1 = model.contact_force(&a1, &b1).expect("in contact");
        let f2 = model.contact_force(&a1, &b2).expect("in contact");

        let mag1 = f1.force_on_a.norm();
        let mag2 = f2.force_on_a.norm();

        // Absolute value against the closed form.
        assert_abs_diff_eq!(mag1, 3663.003663, epsilon = 1.0e-4);
        // δ^{3/2} scaling: (0.04/0.01)^1.5 = 8.
        assert_abs_diff_eq!(mag2 / mag1, 8.0, epsilon = 1.0e-9);
    }

    /// V&V — **no force when the spheres are separated**.
    ///
    /// Methodology: two `r = 0.5 m` spheres with centres `1.2 m` apart, so the
    /// gap `1.2 − 1.0 = 0.2 m > 0` (overlap negative). `contact_force` must
    /// return `None` for both models.
    ///
    /// Result (measured 2026-07-23): `None` for Hooke and Hertz, as required.
    #[test]
    fn no_force_when_separated() {
        let hooke = ContactModel::Hooke(
            HookeContact::new(1000.0, 5.0, 500.0, 2.0, 0.3).expect("valid hooke"),
        );
        let hertz = ContactModel::Hertz(
            HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.8, 0.3).expect("valid hertz"),
        );
        let a = particle(Vec3::zero(), Vec3::zero(), Vec3::zero(), 1.0, 0.5);
        let b = particle(Vec3::new(1.2, 0.0, 0.0), Vec3::zero(), Vec3::zero(), 1.0, 0.5);

        assert!(hooke.contact_force(&a, &b).is_none());
        assert!(hertz.contact_force(&a, &b).is_none());
    }

    /// V&V — **Coulomb cap limits the tangential force to `μ|F_n|`**.
    ///
    /// Methodology: Hooke model, `k_n = 1000 N/m`, `μ = 0.3`, tangential damping
    /// `γ_t = 100 N·s/m`. Two `r = 0.5 m` spheres, centres `0.9 m` apart
    /// (`δ_n = 0.1 m`). Particle `a` slides tangentially with velocity
    /// `(0, 10, 0) m/s` while `b` is at rest; since this velocity is ⟂ `n̂`
    /// (`= +x`), the approach rate `v_n = 0`, so `F_n = k_n·δ_n = 100 N` and the
    /// Coulomb cap is `μ|F_n| = 0.3 × 100 = 30 N`. The uncapped tangential
    /// dashpot force would be `γ_t·v_t = 100 × 10 = 1000 N ≫ 30 N`, so it is
    /// capped. Friction opposes `a`'s slip (`+y`), giving `force_on_a.y = −30 N`
    /// with `force_on_a.x = −100 N` (normal). Pass criterion: `force_on_a.y =
    /// −30 N` to `1e-9 N`.
    ///
    /// Result (measured 2026-07-23): `force_on_a = (−100, −30, 0) N`; the
    /// tangential component is clamped to exactly `μ|F_n| = 30 N` (`< 1e-12 N`
    /// error), confirming the Coulomb cap.
    #[test]
    fn coulomb_cap_limits_tangential_force() {
        let model = ContactModel::Hooke(
            HookeContact::new(1000.0, 0.0, 500.0, 100.0, 0.3).expect("valid hooke"),
        );
        let a = particle(
            Vec3::zero(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::zero(),
            1.0,
            0.5,
        );
        let b = particle(Vec3::new(0.9, 0.0, 0.0), Vec3::zero(), Vec3::zero(), 1.0, 0.5);

        let cf = model.contact_force(&a, &b).expect("in contact");
        // Normal unchanged (velocity is ⟂ n̂).
        assert_abs_diff_eq!(cf.force_on_a.x, -100.0, epsilon = 1.0e-9);
        // Tangential capped at μ|F_n| = 30 N, opposing +y slip.
        assert_abs_diff_eq!(cf.force_on_a.y, -30.0, epsilon = 1.0e-9);
        assert_abs_diff_eq!(cf.force_on_a.z, 0.0, epsilon = 1.0e-12);
    }

    /// V&V — **two equal particles head-on obey Newton's third law**.
    ///
    /// Methodology: equal `r = 0.5 m`, `m = 1 kg` spheres approaching head-on:
    /// `a` at the origin with `v_a = (1, 0, 0) m/s`, `b` at `(0.9, 0, 0)` with
    /// `v_b = (−1, 0, 0) m/s`, `δ_n = 0.1 m`, approach rate
    /// `v_n = (v_a − v_b)·n̂ = 2 m/s`. Hooke `k_n = 1000`, `γ_n = 10`, so
    /// `F_n = 1000×0.1 + 10×2 = 120 N` on `a` along `−x`. There is no tangential
    /// slip, so the force is purely normal. Pass criterion: `force_on_b =
    /// −force_on_a` componentwise to `1e-12 N`, and `force_on_a.x = −120 N` to
    /// `1e-9 N`.
    ///
    /// Result (measured 2026-07-23): `force_on_a = (−120, 0, 0) N`,
    /// `force_on_b = (+120, 0, 0) N = −force_on_a` to round-off, confirming the
    /// action–reaction pairing.
    #[test]
    fn head_on_pair_obeys_newtons_third_law() {
        let model = ContactModel::Hooke(
            HookeContact::new(1000.0, 10.0, 500.0, 2.0, 0.3).expect("valid hooke"),
        );
        let a = particle(
            Vec3::zero(),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::zero(),
            1.0,
            0.5,
        );
        let b = particle(
            Vec3::new(0.9, 0.0, 0.0),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::zero(),
            1.0,
            0.5,
        );

        let cf = model.contact_force(&a, &b).expect("in contact");
        assert_abs_diff_eq!(cf.force_on_a.x, -120.0, epsilon = 1.0e-9);
        // Newton's third law: force_on_b = -force_on_a.
        assert_abs_diff_eq!(cf.force_on_b.x, -cf.force_on_a.x, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.force_on_b.y, -cf.force_on_a.y, epsilon = 1.0e-12);
        assert_abs_diff_eq!(cf.force_on_b.z, -cf.force_on_a.z, epsilon = 1.0e-12);
    }

    /// Effective-modulus helpers match hand-computed values for `E = 1e7 Pa`,
    /// `ν = 0.3`: `E* = 1e7/(2·0.91) = 5.494505e6 Pa`, `G = 1e7/2.6 =
    /// 3.846154e6 Pa`, `G* = G/(2·1.7) = 1.131222e6 Pa`. Also `β(e=0.8) =
    /// ln(0.8)/√(ln²0.8 + π²) = −0.070855…`.
    #[test]
    fn hertz_effective_moduli_match_hand_computed() {
        let hertz =
            HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.8, 0.3).expect("valid hertz");
        assert_abs_diff_eq!(hertz.effective_modulus(), 5.494505494505e6, epsilon = 1.0);
        assert_abs_diff_eq!(hertz.effective_shear_modulus(), 1.131221719457e6, epsilon = 1.0);
        // β = ln(0.8)/sqrt(ln(0.8)^2 + π^2)
        let ln_e = 0.8_f64.ln();
        let beta_expected = ln_e / (ln_e * ln_e + std::f64::consts::PI.powi(2)).sqrt();
        assert_abs_diff_eq!(hertz.damping_beta(), beta_expected, epsilon = 1.0e-15);
        assert!(hertz.damping_beta() < 0.0);
    }

    /// Constructor validation: both models reject out-of-range parameters.
    #[test]
    fn constructors_reject_invalid_parameters() {
        // Hooke: non-positive k_n, negative coefficients.
        assert!(HookeContact::new(0.0, 1.0, 1.0, 1.0, 0.3).is_err());
        assert!(HookeContact::new(1000.0, -1.0, 1.0, 1.0, 0.3).is_err());
        assert!(HookeContact::new(1000.0, 1.0, -1.0, 1.0, 0.3).is_err());
        assert!(HookeContact::new(1000.0, 1.0, 1.0, -1.0, 0.3).is_err());
        assert!(HookeContact::new(1000.0, 1.0, 1.0, 1.0, -0.1).is_err());
        assert!(HookeContact::new(1000.0, 1.0, 1.0, 1.0, 0.3).is_ok());

        // Hertz: E ≤ 0, ν out of [0,0.5), e out of (0,1], μ < 0.
        assert!(HertzContact::new(Pressure::new::<pascal>(0.0), 0.3, 0.8, 0.3).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.5, 0.8, 0.3).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), -0.1, 0.8, 0.3).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.0, 0.3).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 1.5, 0.3).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.8, -0.1).is_err());
        assert!(HertzContact::new(Pressure::new::<pascal>(1.0e7), 0.3, 0.8, 0.3).is_ok());
    }
}
