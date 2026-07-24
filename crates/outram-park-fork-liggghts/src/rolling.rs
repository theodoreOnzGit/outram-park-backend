// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — the rolling-resistance (directional
// constant torque / viscous) and cohesion (linear / JKR) contact extensions
// below are STANDARD PUBLIC-LITERATURE DEM. They are reimplemented CLEAN-ROOM
// from the cited papers (see the module "References"); this file contains NO
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

//! Phase 2 follow-up — **Rolling resistance & cohesion** contact extensions
//! (bead `op-t3l.2` follow-up).
//!
//! Two independent, composable additions to the base normal/tangential contact
//! force of [`crate::contact`]:
//!
//! - [`RollingModel`] — a resisting **torque** opposing the relative *rolling*
//!   of a contacting pair (directional constant torque, or viscous), and
//! - [`CohesionModel`] — an attractive **normal force** (a simple linear
//!   cohesive law, or the Johnson–Kendall–Roberts pull-off force).
//!
//! Both are `enum`s dispatched by `match` (no `dyn`, per the workspace design
//! rules). They are deliberately kept **separate and additive**: neither
//! replaces the base contact force. A caller computes the base
//! [`crate::contact::ContactForce`] first, then *adds* the rolling torque
//! ([`RollingModel::rolling_torque`]) to each particle's torque and *adds* the
//! cohesive normal scalar ([`CohesionModel::cohesive_force`]) to the base normal
//! force. This mirrors how DEM codes layer rolling/adhesion on top of a
//! Hooke/Hertz base — and keeps each piece unit-testable in isolation.
//!
//! # Sign & geometry conventions (read once, applies everywhere)
//!
//! These reuse the conventions of [`crate::contact`] so the pieces compose:
//!
//! - **Normal** `n̂` points from `a`'s centre toward `b`'s centre.
//! - **Overlap** `δ_n` `[m]` is `(r_a + r_b) − ‖x_b − x_a‖`: `δ_n > 0` in
//!   contact, `δ_n < 0` means a surface *gap* of magnitude `−δ_n`.
//! - **Cohesive normal scalar** uses the **same sign as
//!   [`crate::contact::ContactLaw::normal_force_scalar`]**: *positive is
//!   repulsive*, so a cohesive (attractive) force is returned **negative**. A
//!   caller adds it to the base normal scalar `F_n` and applies the total along
//!   `−n̂` on `a` exactly as the base contact does; a net-negative total then
//!   points along `+n̂` (a pulled toward b), i.e. attraction.
//! - **Relative rolling angular velocity** `ω_rel = ω_a − ω_b` `[rad/s]` is the
//!   pair's rolling rate; the resistance torque opposes it. The returned torque
//!   on `a` and on `b` form a **couple** (`τ_b = −τ_a`): rolling resistance is a
//!   pure moment that does no net work on the pair's centre of mass.
//!
//! # Unit convention
//!
//! Following the crate convention (see [`crate::particle`] and
//! [`crate::contact`]), physical scalars are plain `f64` in **SI base units**
//! with the unit spelled out in each doc comment. The model coefficients here
//! (`μ_r` dimensionless, `c_r` in `[N·m·s]`, `k_c` in `[N/m]`, surface energy in
//! `[J/m²]`) have no ergonomic named `uom` alias, so — exactly as the Hooke
//! stiffnesses in [`crate::contact::HookeContact`] — they are documented `f64`.
//! Every constructor still validates its inputs' physical range.
//!
//! # Honest scope
//!
//! These are **clean-room, unit-tested foundations, not benchmark-validated**
//! models — no cross-code (e.g. LIGGGHTS) or experimental comparison has been
//! run; that is a later human validation step. The inline tests check the laws
//! against **hand-computed analytical values** only.
//!
//! Deliberately **out of scope** (documented, not silently missing):
//!
//! - **No history-dependent rolling spring (Ai et al. "Model C" /
//!   elastic–plastic spring-dashpot rolling resistance).** Both rolling models
//!   here are *stateless snapshots*: the constant-torque model needs no history,
//!   and the viscous model depends only on the instantaneous `ω_rel`. A rolling
//!   spring that accumulates a rolling displacement over the contact lifetime
//!   (Iwashita & Oda 1998; Ai et al. Model C) is **not** implemented — it needs
//!   per-contact state the caller's loop would have to carry.
//! - **No liquid-bridge / capillary cohesion** (pendular-bridge, van der Waals,
//!   or electrostatic adhesion). The cohesion here is a dry linear law and the
//!   JKR elastic pull-off force only.
//! - **No full JKR force–displacement curve** and **no hysteresis**. Only the
//!   JKR **pull-off (maximum adhesive) force** `F_pull = (3/2)π γ R*` is
//!   modelled, applied as a constant attractive force while the pair is in
//!   contact. The hysteretic neck (contact radius from the JKR cubic, tension
//!   sustained across a gap up to snap-off) is not solved.
//! - **No plastic, bonded, or parallel-bond contacts.**
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - J. Ai, J.-F. Chen, J. M. Rotter, J. Y. Ooi, "Assessment of rolling
//!   resistance models in discrete element simulations," *Powder Technology*
//!   **206**(3), 269–282 (2011) — the canonical review classifying rolling
//!   resistance into the directional constant-torque ("Model A"), viscous, and
//!   elastic–plastic spring-dashpot ("Model C") families used here.
//! - K. Iwashita and M. Oda, "Rolling resistance at contacts in simulation of
//!   shear band development by DEM," *J. Eng. Mech.* **124**(3), 285–292
//!   (1998) — rolling resistance in granular DEM (the history-dependent rolling
//!   spring is cited but deliberately *not* implemented; see "Honest scope").
//! - K. L. Johnson, K. Kendall, A. D. Roberts, "Surface energy and the contact
//!   of elastic solids," *Proc. R. Soc. Lond. A* **324**(1558), 301–313
//!   (1971) — the JKR adhesion theory; pull-off force `F_pull = (3/2)π γ R*`.

use crate::particle::Vec3;
use crate::DemError;

/// Relative rolling angular-velocity magnitude `[rad/s]` below which the rolling
/// direction `ω̂_rel` is treated as undefined and **zero** rolling torque is
/// returned (avoids dividing by a near-zero magnitude, and encodes that a pair
/// which is not rolling feels no rolling resistance).
const ROLLING_OMEGA_EPS: f64 = 1.0e-12;

/// The resisting rolling torque applied to each particle of a contacting pair.
///
/// Both torques are in newton-metres `[N·m]`. They form a **couple**:
/// `torque_on_b = −torque_on_a` exactly, so the pair feels a pure moment
/// resisting its relative rolling with no net force on the centre of mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollingTorque {
    /// Rolling-resistance torque on particle `a` about its centre `[N·m]`,
    /// directed to oppose the relative rolling `ω_rel = ω_a − ω_b`.
    pub torque_on_a: Vec3,
    /// Rolling-resistance torque on particle `b` about its centre `[N·m]`.
    /// Equals `−torque_on_a` (the reaction of the couple).
    pub torque_on_b: Vec3,
}

impl RollingTorque {
    /// The zero couple (no rolling resistance).
    #[must_use]
    pub const fn zero() -> Self {
        Self { torque_on_a: Vec3::zero(), torque_on_b: Vec3::zero() }
    }
}

/// Closed set of rolling-resistance models, dispatched by `match` with **no**
/// `dyn` / heap allocation (per the workspace design rules).
///
/// Each variant, given the contact normal-force magnitude `|F_n|` `[N]`, the
/// effective rolling radius `R*` `[m]`, and the relative rolling angular
/// velocity `ω_rel` `[rad/s]`, returns the resisting [`RollingTorque`] via
/// [`RollingModel::rolling_torque`].
///
/// # Effective rolling radius
///
/// `R*` is the reduced radius of the pair, `R* = r_a·r_b / (r_a + r_b)` `[m]`,
/// the same reduced radius used by the contact models — it is passed in by the
/// caller (who already has it from the contact geometry), not recomputed here.
///
/// # Variants and parameters
///
/// | Variant | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `None` | — | no rolling resistance | — | — |
/// | `ConstantDirectionalTorque` | `μ_r` | rolling-friction coefficient | `[-]` | `≥ 0` |
/// | `ViscousRolling` | `c_r` | rolling viscous-damping coefficient | `[N·m·s]` | `≥ 0` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RollingModel {
    /// No rolling resistance — always returns [`RollingTorque::zero`].
    None,
    /// **Directional constant-torque** rolling resistance (Ai et al. 2011
    /// "Model A"; Iwashita & Oda 1998).
    ///
    /// The resisting torque has a fixed magnitude set by the normal load and
    /// always points opposite the relative rolling direction:
    ///
    /// `M_r = −μ_r · R* · |F_n| · ω̂_rel`,  with  `ω̂_rel = ω_rel / ‖ω_rel‖`.
    ///
    /// When `‖ω_rel‖` is below [`ROLLING_OMEGA_EPS`] the direction is undefined
    /// and the torque is zero (a non-rolling pair feels no rolling resistance).
    /// This is the *directional* form: the magnitude does not depend on the
    /// rolling *speed*, only its direction — hence "constant torque".
    ConstantDirectionalTorque {
        /// Rolling-friction coefficient `μ_r` `[-]`. Non-negative.
        mu_r: f64,
    },
    /// **Viscous** rolling resistance (Ai et al. 2011, viscous family).
    ///
    /// The resisting torque is linear in the relative rolling angular velocity:
    ///
    /// `M_r = −c_r · ω_rel`.
    ///
    /// Unlike the constant-torque form this scales with rolling *speed* and its
    /// direction follows `−ω_rel` continuously (so it needs no `ω̂_rel`
    /// normalisation and is exactly zero at `ω_rel = 0`). The pure linear
    /// dashpot is used here; the optional cap of the viscous torque at the
    /// constant-torque limit `μ_r R*|F_n|` (Ai et al.) is **not** applied — see
    /// the module "Honest scope".
    ViscousRolling {
        /// Rolling viscous-damping coefficient `c_r` `[N·m·s]`
        /// (torque per unit rolling angular velocity). Non-negative.
        c_r: f64,
    },
}

impl RollingModel {
    /// Construct the no-resistance model.
    #[must_use]
    pub const fn none() -> Self {
        Self::None
    }

    /// Construct a validated directional constant-torque model (Ai et al.
    /// "Model A").
    ///
    /// `mu_r` is the dimensionless rolling-friction coefficient `μ_r`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `mu_r` is negative (a rolling-
    /// friction coefficient is a non-negative physical quantity).
    pub fn constant_directional_torque(mu_r: f64) -> Result<Self, DemError> {
        if mu_r < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "rolling-friction coefficient mu_r must be non-negative, got {mu_r}"
            )));
        }
        Ok(Self::ConstantDirectionalTorque { mu_r })
    }

    /// Construct a validated viscous rolling-resistance model.
    ///
    /// `c_r` is the rolling viscous-damping coefficient `[N·m·s]`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `c_r` is negative (a damping
    /// coefficient is non-negative; a negative value would inject rotational
    /// energy).
    pub fn viscous(c_r: f64) -> Result<Self, DemError> {
        if c_r < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "rolling viscous-damping coefficient c_r must be non-negative, got {c_r} N·m·s"
            )));
        }
        Ok(Self::ViscousRolling { c_r })
    }

    /// Resisting rolling torque on each particle of the pair.
    ///
    /// # Parameters
    ///
    /// - `normal_force` — the contact normal-force **magnitude** `|F_n|` `[N]`
    ///   (`≥ 0`; take it from the base contact result). Only used by the
    ///   constant-torque model.
    /// - `r_eff` — the effective rolling radius `R*` `[m]`, `R* = r_a r_b /
    ///   (r_a + r_b)`. Only used by the constant-torque model.
    /// - `omega_rel` — the relative rolling angular velocity `ω_rel = ω_a − ω_b`
    ///   `[rad/s]`.
    ///
    /// # Returns
    ///
    /// A [`RollingTorque`] couple (`τ_b = −τ_a`) resisting the relative rolling.
    /// Returns [`RollingTorque::zero`] for [`RollingModel::None`], and for
    /// either active model when `‖ω_rel‖ <` [`ROLLING_OMEGA_EPS`] (no rolling →
    /// no rolling resistance).
    #[must_use]
    pub fn rolling_torque(&self, normal_force: f64, r_eff: f64, omega_rel: Vec3) -> RollingTorque {
        let omega_mag = omega_rel.norm();
        if omega_mag < ROLLING_OMEGA_EPS {
            return RollingTorque::zero();
        }
        let torque_on_a = match self {
            Self::None => return RollingTorque::zero(),
            Self::ConstantDirectionalTorque { mu_r } => {
                // M_r = −μ_r R* |F_n| ω̂_rel.
                let magnitude = mu_r * r_eff * normal_force;
                omega_rel.scale(-magnitude / omega_mag)
            }
            Self::ViscousRolling { c_r } => {
                // M_r = −c_r ω_rel.
                omega_rel.scale(-c_r)
            }
        };
        RollingTorque { torque_on_a, torque_on_b: torque_on_a.scale(-1.0) }
    }
}

/// Closed set of cohesion (attractive-normal-force) models, dispatched by
/// `match` with **no** `dyn` / heap allocation (per the workspace design rules).
///
/// Each variant, given the signed normal overlap `δ_n` `[m]` (`> 0` overlap,
/// `< 0` a gap of `−δ_n`) and the effective radius `R*` `[m]`, returns a
/// cohesive normal **scalar** `[N]` via [`CohesionModel::cohesive_force`]. The
/// scalar uses the base-contact sign convention (positive repulsive), so a
/// cohesive force is **negative** and a caller adds it to the base normal
/// scalar.
///
/// # Variants and parameters
///
/// | Variant | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `None` | — | no cohesion | — | — |
/// | `LinearCohesion` | `k_c` | cohesive stiffness | `[N/m]` | `≥ 0` |
/// | `LinearCohesion` | `max_gap` | cohesion cut-off separation | `[m]` | `> 0` |
/// | `Jkr` | `γ` | surface energy (work of adhesion) | `[J/m²]` | `≥ 0` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CohesionModel {
    /// No cohesion — always returns `0` from [`CohesionModel::cohesive_force`].
    None,
    /// Simple **linear cohesion**: an attractive force that ramps linearly with
    /// surface separation and vanishes beyond a cut-off gap.
    ///
    /// Let the surface gap be `g = max(−δ_n, 0)` `[m]` (`0` while the pair
    /// overlaps). The cohesive scalar is
    ///
    /// `F_c = −k_c · (max_gap − g)`  for  `0 ≤ g ≤ max_gap`,
    ///
    /// held at its peak `−k_c · max_gap` `[N]` while overlapping (`δ_n ≥ 0`,
    /// `g = 0`), and `0` once the gap exceeds `max_gap`. Thus attraction is
    /// strongest at/inside contact and falls **linearly** to zero at the cut-off
    /// — a dry, non-hysteretic cohesive law. `k_c` `[N/m]` is a cohesive
    /// *stiffness*; the peak attractive force is `k_c · max_gap` `[N]`.
    LinearCohesion {
        /// Cohesive stiffness `k_c` `[N/m]`. Non-negative.
        k_c: f64,
        /// Cohesion cut-off separation `max_gap` `[m]`: beyond this surface gap
        /// the attractive force is zero. Strictly positive.
        max_gap: f64,
    },
    /// **Johnson–Kendall–Roberts (JKR)** adhesion — pull-off force only.
    ///
    /// Applies the constant JKR **pull-off (maximum adhesive) force**
    ///
    /// `F_pull = (3/2) · π · γ · R*`  `[N]`
    ///
    /// as an attractive scalar `−F_pull` while the pair is in contact
    /// (`δ_n ≥ 0`), and `0` when separated (`δ_n < 0`). `γ` `[J/m²]` is the
    /// surface energy (work of adhesion). Only the pull-off magnitude is
    /// modelled — the full nonlinear JKR force–displacement curve and its
    /// hysteresis are out of scope (see the module "Honest scope").
    Jkr {
        /// Surface energy / work of adhesion `γ` `[J/m²]`. Non-negative.
        surface_energy: f64,
    },
}

impl CohesionModel {
    /// Construct the no-cohesion model.
    #[must_use]
    pub const fn none() -> Self {
        Self::None
    }

    /// Construct a validated linear-cohesion model.
    ///
    /// `k_c` is the cohesive stiffness `[N/m]`; `max_gap` is the cut-off surface
    /// separation `[m]` beyond which the attraction is zero.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `k_c` is negative, or if `max_gap`
    /// is not strictly positive (a non-positive cut-off gap leaves no range over
    /// which the linear law is defined).
    pub fn linear(k_c: f64, max_gap: f64) -> Result<Self, DemError> {
        if k_c < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "cohesive stiffness k_c must be non-negative, got {k_c} N/m"
            )));
        }
        if !(max_gap > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "cohesion cut-off max_gap must be strictly positive, got {max_gap} m"
            )));
        }
        Ok(Self::LinearCohesion { k_c, max_gap })
    }

    /// Construct a validated JKR adhesion model.
    ///
    /// `surface_energy` is the work of adhesion `γ` `[J/m²]`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `surface_energy` is negative (a
    /// surface energy is a non-negative physical quantity).
    pub fn jkr(surface_energy: f64) -> Result<Self, DemError> {
        if surface_energy < 0.0 {
            return Err(DemError::InvalidInput(format!(
                "surface_energy γ must be non-negative, got {surface_energy} J/m²"
            )));
        }
        Ok(Self::Jkr { surface_energy })
    }

    /// The **maximum attractive force magnitude** `[N]` this model can exert for
    /// the given effective radius `R*` `[m]` — a non-negative quantity.
    ///
    /// - `None` → `0`.
    /// - `LinearCohesion` → the peak `k_c · max_gap` (reached at/inside
    ///   contact); independent of `R*`.
    /// - `Jkr` → the pull-off force `F_pull = (3/2) π γ R*`.
    #[must_use]
    pub fn max_attractive_force(&self, r_eff: f64) -> f64 {
        match self {
            Self::None => 0.0,
            Self::LinearCohesion { k_c, max_gap } => k_c * max_gap,
            Self::Jkr { surface_energy } => {
                1.5 * std::f64::consts::PI * surface_energy * r_eff
            }
        }
    }

    /// Cohesive normal **scalar** `[N]` for the given signed overlap and
    /// effective radius.
    ///
    /// # Parameters
    ///
    /// - `overlap` — signed normal overlap `δ_n` `[m]`: `> 0` overlapping,
    ///   `< 0` a surface gap of magnitude `−δ_n`.
    /// - `r_eff` — effective radius `R*` `[m]` (used by the JKR pull-off force;
    ///   ignored by the linear model).
    ///
    /// # Returns
    ///
    /// The cohesive scalar in the **base-contact sign convention** (positive
    /// repulsive), so the returned value is `≤ 0`: negative is attractive, `0`
    /// is no cohesion. A caller adds this to the base normal scalar `F_n` before
    /// applying the total along `−n̂`.
    #[must_use]
    pub fn cohesive_force(&self, overlap: f64, r_eff: f64) -> f64 {
        match self {
            Self::None => 0.0,
            Self::LinearCohesion { k_c, max_gap } => {
                // Surface gap g = max(−δ_n, 0): zero while overlapping.
                let gap = (-overlap).max(0.0);
                if gap > *max_gap {
                    0.0
                } else {
                    // −k_c (max_gap − g): peak −k_c·max_gap at contact, → 0 at cut-off.
                    -k_c * (max_gap - gap)
                }
            }
            Self::Jkr { .. } => {
                // Constant pull-off force while in contact; zero once separated.
                if overlap >= 0.0 {
                    -self.max_attractive_force(r_eff)
                } else {
                    0.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    /// V&V — **constant-torque rolling resistance opposes the rolling direction
    /// with the hand-computed magnitude**.
    ///
    /// Methodology: directional constant-torque model (Ai et al. "Model A") with
    /// `μ_r = 0.1`. Contact normal-force magnitude `|F_n| = 100 N`, effective
    /// rolling radius `R* = 0.25 m`, relative rolling angular velocity
    /// `ω_rel = (0, 0, 5) rad/s` (pure +z rolling). The closed form is
    /// `M_r = −μ_r R* |F_n| ω̂_rel`, so the magnitude is
    /// `μ_r R* |F_n| = 0.1 × 0.25 × 100 = 2.5 N·m` and the direction is `−ẑ`
    /// (opposing +z rolling). Pass criteria: `torque_on_a = (0, 0, −2.5) N·m`
    /// and `torque_on_b = (0, 0, +2.5) N·m = −torque_on_a` (the couple), each to
    /// `1e-12 N·m`; the magnitude is independent of the rolling *speed*.
    ///
    /// Result (measured 2026-07-24): `torque_on_a = (0, 0, −2.5) N·m`,
    /// `torque_on_b = (0, 0, +2.5) N·m`, reproduced to `< 1e-13 N·m` (round-off)
    /// — exactly the hand-computed `μ_r R* |F_n| = 2.5 N·m` opposing +z, and a
    /// clean action–reaction couple.
    #[test]
    fn constant_torque_opposes_rolling_with_hand_magnitude() {
        let model = RollingModel::constant_directional_torque(0.1).expect("valid mu_r");
        let omega_rel = Vec3::new(0.0, 0.0, 5.0);
        let t = model.rolling_torque(100.0, 0.25, omega_rel);

        assert_abs_diff_eq!(t.torque_on_a.x, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_a.y, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_a.z, -2.5, epsilon = 1.0e-12);
        // Couple: torque_on_b = −torque_on_a.
        assert_abs_diff_eq!(t.torque_on_b.x, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_b.y, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_b.z, 2.5, epsilon = 1.0e-12);

        // Magnitude is speed-independent: doubling ω_rel keeps |M_r| = 2.5.
        let t2 = model.rolling_torque(100.0, 0.25, omega_rel.scale(2.0));
        assert_abs_diff_eq!(t2.torque_on_a.z, -2.5, epsilon = 1.0e-12);
    }

    /// V&V — **constant-torque direction follows an off-axis rolling vector**.
    ///
    /// Methodology: `μ_r = 0.2`, `|F_n| = 50 N`, `R* = 0.1 m`, and a rolling
    /// vector `ω_rel = (3, 4, 0) rad/s` with `‖ω_rel‖ = 5 rad/s`. The magnitude
    /// is `μ_r R* |F_n| = 0.2 × 0.1 × 50 = 1.0 N·m`, and the direction is
    /// `−ω̂_rel = −(0.6, 0.8, 0)`, so `torque_on_a = (−0.6, −0.8, 0) N·m`. Pass
    /// criterion: componentwise to `1e-12 N·m`, and `‖torque_on_a‖ = 1.0 N·m`.
    ///
    /// Result (measured 2026-07-24): `torque_on_a = (−0.6, −0.8, 0) N·m`,
    /// `‖·‖ = 1.0 N·m`, to round-off — confirming the torque is anti-parallel to
    /// the rolling direction with the correct load-set magnitude.
    #[test]
    fn constant_torque_direction_is_antiparallel_to_rolling() {
        let model = RollingModel::constant_directional_torque(0.2).expect("valid mu_r");
        let omega_rel = Vec3::new(3.0, 4.0, 0.0); // ‖·‖ = 5
        let t = model.rolling_torque(50.0, 0.1, omega_rel);
        assert_abs_diff_eq!(t.torque_on_a.x, -0.6, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_a.y, -0.8, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_a.z, 0.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_a.norm(), 1.0, epsilon = 1.0e-12);
    }

    /// V&V — **viscous rolling resistance is linear in `ω_rel`**.
    ///
    /// Methodology: viscous model with `c_r = 0.4 N·m·s`. The closed form is
    /// `M_r = −c_r ω_rel`. For `ω_rel = (0, 0, 5) rad/s` the expected torque is
    /// `(0, 0, −2.0) N·m`; `|F_n|` and `R*` do not enter the viscous law (passed
    /// as `100 N`, `0.25 m` to confirm they are ignored). Doubling `ω_rel`
    /// doubles the torque (linearity). Pass criteria: `torque_on_a =
    /// (0, 0, −2.0) N·m` to `1e-12 N·m`, `torque_on_b = −torque_on_a`, and the
    /// doubled case gives `(0, 0, −4.0) N·m`.
    ///
    /// Result (measured 2026-07-24): `torque_on_a = (0, 0, −2.0) N·m`, doubling
    /// `ω_rel` → `(0, 0, −4.0) N·m`, to round-off — confirming the linear
    /// (viscous) scaling with rolling angular velocity and the couple reaction.
    #[test]
    fn viscous_rolling_is_linear_in_omega() {
        let model = RollingModel::viscous(0.4).expect("valid c_r");
        let omega_rel = Vec3::new(0.0, 0.0, 5.0);
        let t = model.rolling_torque(100.0, 0.25, omega_rel);
        assert_abs_diff_eq!(t.torque_on_a.z, -2.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t.torque_on_b.z, 2.0, epsilon = 1.0e-12);

        let t2 = model.rolling_torque(100.0, 0.25, omega_rel.scale(2.0));
        assert_abs_diff_eq!(t2.torque_on_a.z, -4.0, epsilon = 1.0e-12);
    }

    /// V&V — **zero rolling velocity → zero rolling torque** (both active
    /// models).
    ///
    /// Methodology: with `ω_rel = 0` there is no rolling to resist, so both the
    /// constant-torque model (whose direction `ω̂_rel` is undefined and guarded
    /// by [`ROLLING_OMEGA_EPS`]) and the viscous model (`−c_r · 0`) must return
    /// exactly the zero couple, regardless of the (non-zero) normal load.
    ///
    /// Result (measured 2026-07-24): both models return `torque_on_a =
    /// torque_on_b = (0, 0, 0) N·m` exactly for `ω_rel = 0`.
    #[test]
    fn zero_rolling_velocity_gives_zero_torque() {
        let constant = RollingModel::constant_directional_torque(0.3).expect("valid");
        let viscous = RollingModel::viscous(0.7).expect("valid");
        let zero = Vec3::zero();

        for model in [constant, viscous] {
            let t = model.rolling_torque(100.0, 0.25, zero);
            assert_eq!(t.torque_on_a, Vec3::zero());
            assert_eq!(t.torque_on_b, Vec3::zero());
        }
    }

    /// V&V — **`RollingModel::None` returns zero torque** for any inputs.
    ///
    /// Methodology: the `None` variant must be a true no-op — zero couple even
    /// with a large normal load and a fast, off-axis rolling velocity.
    ///
    /// Result (measured 2026-07-24): `torque_on_a = torque_on_b = 0` for
    /// `|F_n| = 1e4 N`, `R* = 0.5 m`, `ω_rel = (10, −20, 30) rad/s`.
    #[test]
    fn rolling_none_returns_zero() {
        let model = RollingModel::none();
        let t = model.rolling_torque(1.0e4, 0.5, Vec3::new(10.0, -20.0, 30.0));
        assert_eq!(t, RollingTorque::zero());
    }

    /// V&V — **JKR pull-off force matches `(3/2) π γ R*`** for a hand case.
    ///
    /// Methodology: JKR model with surface energy `γ = 0.5 J/m²` and effective
    /// radius `R* = 0.02 m`. The closed-form pull-off force is
    /// `F_pull = (3/2) π γ R* = 1.5 × π × 0.5 × 0.02 = 0.015 π = 0.0471238898…
    /// N`. Two checks: [`CohesionModel::max_attractive_force`] returns
    /// `+F_pull`, and [`CohesionModel::cohesive_force`] at a small positive
    /// overlap (`δ_n = 0.001 m`, in contact) returns the attractive
    /// `−F_pull`. Pass criterion: both to `1e-12 N`.
    ///
    /// Result (measured 2026-07-24): `max_attractive_force = 0.0471238898…N =
    /// (3/2)π·0.5·0.02` and `cohesive_force(0.001, 0.02) = −0.0471238898…N`, to
    /// round-off — matching the hand-computed JKR pull-off force.
    #[test]
    fn jkr_pull_off_matches_three_halves_pi_gamma_reff() {
        let model = CohesionModel::jkr(0.5).expect("valid γ");
        let r_eff = 0.02;
        let f_pull = 1.5 * std::f64::consts::PI * 0.5 * r_eff; // 0.015π

        assert_abs_diff_eq!(model.max_attractive_force(r_eff), f_pull, epsilon = 1.0e-12);
        // In contact → attractive (negative) scalar of magnitude F_pull.
        assert_abs_diff_eq!(model.cohesive_force(0.001, r_eff), -f_pull, epsilon = 1.0e-12);
        // Sanity against the literal number.
        assert_abs_diff_eq!(model.cohesive_force(0.001, r_eff), -0.047123889803, epsilon = 1.0e-9);
    }

    /// V&V — **JKR cohesion is zero once the pair separates** (honest scope: no
    /// hysteretic neck across a gap).
    ///
    /// Methodology: the same JKR model (`γ = 0.5`, `R* = 0.02 m`) evaluated at a
    /// negative overlap `δ_n = −0.001 m` (a gap) must return `0` — this
    /// implementation applies the pull-off force only while in contact and does
    /// not model tension sustained across a gap.
    ///
    /// Result (measured 2026-07-24): `cohesive_force(−0.001, 0.02) = 0`.
    #[test]
    fn jkr_cohesion_zero_when_separated() {
        let model = CohesionModel::jkr(0.5).expect("valid γ");
        assert_abs_diff_eq!(model.cohesive_force(-0.001, 0.02), 0.0, epsilon = 1.0e-15);
    }

    /// V&V — **linear cohesion is attractive within the gap and zero beyond
    /// `max_gap`**.
    ///
    /// Methodology: linear model `k_c = 100 N/m`, `max_gap = 0.01 m` (peak force
    /// `k_c·max_gap = 1.0 N`). The scalar is `F_c = −k_c (max_gap − g)` with
    /// surface gap `g = max(−δ_n, 0)`. Checked points:
    /// - overlap `δ_n = 0.003 m` (in contact, `g = 0`): `F_c = −100 × 0.01 =
    ///   −1.0 N` (peak attraction).
    /// - gap `δ_n = −0.005 m` (`g = 0.005 m`, half-way): `F_c = −100 ×
    ///   (0.01 − 0.005) = −0.5 N` (attractive).
    /// - gap `δ_n = −0.01 m` (`g = max_gap`): `F_c = 0` (cut-off edge).
    /// - gap `δ_n = −0.02 m` (`g = 0.02 m > max_gap`): `F_c = 0` (beyond range).
    /// Pass criterion: each to `1e-12 N`; every attractive value strictly `< 0`.
    ///
    /// Result (measured 2026-07-24): `F_c(δ=0.003) = −1.0 N`,
    /// `F_c(δ=−0.005) = −0.5 N`, `F_c(δ=−0.01) = 0 N`, `F_c(δ=−0.02) = 0 N`, to
    /// round-off — attractive within the gap, linearly decaying, and exactly
    /// zero at and beyond the `max_gap` cut-off.
    #[test]
    fn linear_cohesion_attractive_within_gap_zero_beyond() {
        let model = CohesionModel::linear(100.0, 0.01).expect("valid linear cohesion");
        // In contact → peak attraction −k_c·max_gap = −1.0 N.
        let f_contact = model.cohesive_force(0.003, 0.05);
        assert_abs_diff_eq!(f_contact, -1.0, epsilon = 1.0e-12);
        assert!(f_contact < 0.0);
        // Half-way gap → −0.5 N (attractive).
        let f_half = model.cohesive_force(-0.005, 0.05);
        assert_abs_diff_eq!(f_half, -0.5, epsilon = 1.0e-12);
        assert!(f_half < 0.0);
        // At the cut-off edge → 0.
        assert_abs_diff_eq!(model.cohesive_force(-0.01, 0.05), 0.0, epsilon = 1.0e-12);
        // Beyond max_gap → 0.
        assert_abs_diff_eq!(model.cohesive_force(-0.02, 0.05), 0.0, epsilon = 1.0e-12);
    }

    /// V&V — **`CohesionModel::None` returns zero cohesive force** for any
    /// inputs.
    ///
    /// Methodology: the `None` variant must give `0` both in contact and across
    /// a gap, for any effective radius.
    ///
    /// Result (measured 2026-07-24): `cohesive_force = 0` at `δ_n = +0.01 m` and
    /// `δ_n = −0.01 m` (`R* = 0.3 m`), and `max_attractive_force = 0`.
    #[test]
    fn cohesion_none_returns_zero() {
        let model = CohesionModel::none();
        assert_abs_diff_eq!(model.cohesive_force(0.01, 0.3), 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(model.cohesive_force(-0.01, 0.3), 0.0, epsilon = 1.0e-15);
        assert_abs_diff_eq!(model.max_attractive_force(0.3), 0.0, epsilon = 1.0e-15);
    }

    /// Constructor validation: both model families reject out-of-range
    /// parameters and accept valid ones.
    ///
    /// Methodology: `μ_r < 0`, `c_r < 0`, `k_c < 0`, `max_gap ≤ 0`, and
    /// `γ < 0` must each yield [`DemError::InvalidInput`]; non-negative
    /// parameters (with `max_gap > 0`) must succeed.
    ///
    /// Result (measured 2026-07-24): all invalid inputs rejected, all valid
    /// inputs accepted, as asserted below.
    #[test]
    fn constructors_reject_invalid_parameters() {
        // Rolling.
        assert!(RollingModel::constant_directional_torque(-0.1).is_err());
        assert!(RollingModel::constant_directional_torque(0.0).is_ok());
        assert!(RollingModel::viscous(-1.0).is_err());
        assert!(RollingModel::viscous(0.0).is_ok());
        // Cohesion.
        assert!(CohesionModel::linear(-1.0, 0.01).is_err());
        assert!(CohesionModel::linear(100.0, 0.0).is_err());
        assert!(CohesionModel::linear(100.0, -0.01).is_err());
        assert!(CohesionModel::linear(0.0, 0.01).is_ok());
        assert!(CohesionModel::jkr(-0.5).is_err());
        assert!(CohesionModel::jkr(0.0).is_ok());
    }
}
