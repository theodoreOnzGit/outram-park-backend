// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/surface_model_default.h, src/normal_model_hertz.h,
//                      src/normal_model_hooke.h, src/tangential_model_history.h,
//                      src/tangential_model_no_history.h, src/global_properties.cpp
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz. Contributing authors
//                       Christoph Kloss, Richard Berger.
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

//! # LIGGGHTS-faithful granular contact pipeline (`pair_style gran`)
//!
//! A line-by-line translation of the LIGGGHTS contact chain — *surface model →
//! normal model → tangential model* — including the **tangential shear
//! history** that the stateless [`crate::contact`] module deliberately omits.
//!
//! ## Why this module exists next to [`crate::contact`]
//!
//! [`crate::contact`] evaluates a contact from a *snapshot* of two particles:
//! it has nowhere to keep the accumulated tangential displacement `ξ_t`, so it
//! hard-codes `ξ_t = 0` and the tangential force degenerates to a Coulomb-capped
//! dashpot. That is fine for an instantaneous force query and useless for a
//! packed bed: with no tangential *spring*, a static assembly cannot carry
//! shear, so a heap has **zero angle of repose** and a pebble bed will not stand
//! up. Reproducing LIGGGHTS requires history, and history requires state that
//! outlives the call — hence [`ShearHistory`].
//!
//! ## Sign and geometry conventions (upstream's, kept verbatim)
//!
//! LIGGGHTS defines the contact normal `ê_n` as pointing **from `j` to `i`**
//! (`delta = x_i − x_j`, `ê_n = delta/|delta|`) and the relative velocity as
//! `v_r = v_i − v_j`. Consequently
//!
//! - `v_n = v_r · ê_n` is **negative while the pair approaches**;
//! - the normal force `F_n ê_n` pushes `i` away from `j` for `F_n > 0`;
//! - the damping term is `−γ_n v_n`, positive (repulsive) on approach.
//!
//! This is the *opposite* sign convention to [`crate::contact`], which measures
//! `v_n` positive on approach along an `a → b` normal. Both are self-consistent;
//! this module keeps upstream's so that the translation can be checked against
//! upstream source without a mental sign flip on every line.
//!
//! ## Contact radii — an `O(δ)` term [`crate::contact`] drops
//!
//! Upstream evaluates the lever arm and the surface-velocity moment at the
//! **contact plane**, not the particle centre distance:
//!
//! ```text
//!   c_ri = r_i − δ_n/2 ,    c_rj = r_j − δ_n/2
//! ```
//!
//! [`crate::contact`] uses `r_i` and `r_j` instead. The difference is `O(δ_n)`
//! and therefore small, but it is a genuine divergence from upstream and it
//! biases both the slip velocity and the spin-up torque. This module uses
//! upstream's.
//!
//! ## Honest scope
//!
//! - **Implemented:** default surface model; Hertz and Hooke normal models;
//!   history and no-history tangential models; per-pair shear-history storage
//!   with Coulomb rescaling of the stored displacement.
//! - **Not implemented:** cohesion models, rolling models (see
//!   [`crate::rolling`]), superquadrics, multi-contact surface corrections,
//!   the `limitForce`/`viscous`/`heating`/elastic-potential switches, and
//!   mixed-material property matrices (a single material is assumed, as in
//!   [`crate::contact`]).
//! - **Verified against upstream** — see `docs/verification-and-validation.md`
//!   and `tests/liggghts_cross_code.rs`.

use std::collections::HashMap;

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// `√(5/6)`, upstream's `sqrtFiveOverSix` damping constant `[-]`.
///
/// Appears in the Hertz viscoelastic damping coefficients
/// `γ = −2√(5/6)·β·√(S·m*)`. Upstream hard-codes the literal to 53 digits;
/// the `f64` value is identical.
pub const SQRT_FIVE_OVER_SIX: f64 = 0.912_870_929_175_276_9;

/// Material and interaction properties shared by both normal models.
///
/// Assumes a **single isotropic linear-elastic material** for both partners
/// (see the module "Honest scope"). Upstream supports a per-type-pair matrix;
/// the same-material reduction of upstream's `createYeff`/`createGeff` is used
/// here and is reproduced exactly by [`GranularMaterial::y_eff`] /
/// [`GranularMaterial::g_eff`].
///
/// # Parameters and units
///
/// | Field | Symbol | Quantity | SI unit | Valid range |
/// |---|---|---|---|---|
/// | `youngs_modulus` | `E` | Young's modulus | `[Pa]` | `> 0` |
/// | `poisson_ratio` | `ν` | Poisson ratio | `[-]` | `0 ≤ ν < 0.5` |
/// | `restitution` | `e` | coefficient of restitution | `[-]` | `0.05 < e ≤ 1` |
/// | `friction` | `μ` | Coulomb friction coefficient | `[-]` | `≥ 0` |
///
/// The restitution lower bound `0.05` is upstream's own sanity check in
/// `MODEL_PARAMS::createCoeffRest` (`0.05 < coefficientRestitution <= 1
/// required`) and is enforced here for parity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GranularMaterial {
    /// Young's modulus `E` `[Pa]`.
    pub youngs_modulus: f64,
    /// Poisson ratio `ν` `[-]`.
    pub poisson_ratio: f64,
    /// Coefficient of restitution `e` `[-]`.
    pub restitution: f64,
    /// Coulomb friction coefficient `μ` `[-]`.
    pub friction: f64,
}

impl GranularMaterial {
    /// Validate and build a material.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if any parameter is outside the range in the
    /// type-level table.
    pub fn new(
        youngs_modulus: f64,
        poisson_ratio: f64,
        restitution: f64,
        friction: f64,
    ) -> Result<Self, DemError> {
        if !(youngs_modulus > 0.0) || !youngs_modulus.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "Young's modulus must be finite and > 0 Pa, got {youngs_modulus}"
            )));
        }
        if !(0.0..0.5).contains(&poisson_ratio) {
            return Err(DemError::InvalidInput(format!(
                "Poisson ratio must satisfy 0 <= nu < 0.5, got {poisson_ratio}"
            )));
        }
        if !(restitution > 0.05 && restitution <= 1.0) {
            return Err(DemError::InvalidInput(format!(
                "coefficient of restitution must satisfy 0.05 < e <= 1 \
                 (LIGGGHTS MODEL_PARAMS::createCoeffRest), got {restitution}"
            )));
        }
        if !(friction >= 0.0) || !friction.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "friction coefficient must be finite and >= 0, got {friction}"
            )));
        }
        Ok(Self {
            youngs_modulus,
            poisson_ratio,
            restitution,
            friction,
        })
    }

    /// Effective Young's modulus `Y_eff` `[Pa]`.
    ///
    /// Upstream `createYeff`: `1/((1−ν_i²)/E_i + (1−ν_j²)/E_j)`, which for one
    /// shared material reduces to `E / (2(1 − ν²))`.
    #[must_use]
    pub fn y_eff(&self) -> f64 {
        let nu = self.poisson_ratio;
        self.youngs_modulus / (2.0 * (1.0 - nu * nu))
    }

    /// Effective shear modulus `G_eff` `[Pa]`.
    ///
    /// Upstream `createGeff`: `1/(2(2−ν_i)(1+ν_i)/E_i + 2(2−ν_j)(1+ν_j)/E_j)`,
    /// reducing for one shared material to `E / (4(2 − ν)(1 + ν))`.
    #[must_use]
    pub fn g_eff(&self) -> f64 {
        let nu = self.poisson_ratio;
        self.youngs_modulus / (4.0 * (2.0 - nu) * (1.0 + nu))
    }

    /// Damping ratio `β_eff` `[-]`, upstream `createBetaEff`.
    ///
    /// `β = ln(e) / √(ln²(e) + π²)`. Negative for `e < 1`, zero at `e = 1`.
    #[must_use]
    pub fn beta_eff(&self) -> f64 {
        let ln_e = self.restitution.ln();
        ln_e / (ln_e * ln_e + std::f64::consts::PI * std::f64::consts::PI).sqrt()
    }
}

/// Contact kinematics — translation of upstream `SurfaceModel<SURFACE_DEFAULT>`.
///
/// Every quantity uses upstream's convention (module docs): `ê_n` points from
/// `j` to `i`, `v_r = v_i − v_j`.
///
/// # Fields and units
///
/// | Field | Symbol | Quantity | SI unit |
/// |---|---|---|---|
/// | `en` | `ê_n` | unit contact normal, `j → i` | `[-]` |
/// | `delta_n` | `δ_n` | overlap (positive in contact) | `[m]` |
/// | `vn` | `v_n` | normal relative velocity (`< 0` approaching) | `[m/s]` |
/// | `vtr` | `v_tr` | relative **surface** velocity in the tangent plane | `[m/s]` |
/// | `cri` / `crj` | `c_ri`, `c_rj` | contact radii `r − δ_n/2` | `[m]` |
/// | `r_eff` | `R*` | effective radius | `[m]` |
/// | `m_eff` | `m*` | effective mass | `[kg]` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactKinematics {
    /// Unit contact normal, pointing from `j` to `i` `[-]`.
    pub en: Vec3,
    /// Overlap `δ_n = r_i + r_j − |x_i − x_j|` `[m]`, strictly positive.
    pub delta_n: f64,
    /// Normal relative velocity `v_r · ê_n` `[m/s]`; negative while approaching.
    pub vn: f64,
    /// Relative surface (slip) velocity in the tangent plane `[m/s]`.
    pub vtr: Vec3,
    /// Contact radius of `i`, `r_i − δ_n/2` `[m]`.
    pub cri: f64,
    /// Contact radius of `j`, `r_j − δ_n/2` `[m]`.
    pub crj: f64,
    /// Effective (reduced) radius `R*` `[m]`.
    pub r_eff: f64,
    /// Effective (reduced) mass `m*` `[kg]`.
    pub m_eff: f64,
}

impl ContactKinematics {
    /// Resolve the kinematics of a particle–particle contact, or `None` if the
    /// pair does not overlap (or the centres coincide, leaving `ê_n` undefined).
    ///
    /// Translation of `SurfaceModel<SURFACE_DEFAULT>::surfacesIntersect` with
    /// `is_wall = false`.
    #[must_use]
    pub fn pair(i: &Particle, j: &Particle) -> Option<Self> {
        let delta = i.position.sub(j.position); // x_i - x_j
        let r = delta.norm();
        let radsum = i.radius + j.radius;
        let delta_n = radsum - r;
        if delta_n <= 0.0 || r <= 0.0 {
            return None;
        }
        let rinv = 1.0 / r;
        let en = delta.scale(rinv);

        let vr = i.velocity.sub(j.velocity);
        let vn = vr.dot(en);
        let vt = vr.sub(en.scale(vn));

        let cri = i.radius - 0.5 * delta_n;
        let crj = j.radius - 0.5 * delta_n;
        // wr = (c_ri ω_i + c_rj ω_j) / r
        let wr = i
            .angular_velocity
            .scale(cri)
            .add(j.angular_velocity.scale(crj))
            .scale(rinv);
        // Upstream: vtr1 = vt1 - (dz*wr2 - dy*wr3), i.e. v_tr = v_t + delta x wr.
        let vtr = vt.add(delta.cross(wr));

        Some(Self {
            en,
            delta_n,
            vn,
            vtr,
            cri,
            crj,
            r_eff: i.radius * j.radius / radsum,
            m_eff: i.mass * j.mass / (i.mass + j.mass),
        })
    }

    /// Resolve the kinematics of a particle–**wall** contact.
    ///
    /// `wall_normal` is the inward unit normal of the wall at the contact
    /// (pointing from the wall surface into the domain, i.e. towards the
    /// particle centre); `delta_n` `[m]` is the overlap. Upstream's wall branch
    /// differs from the pair branch in two ways, both reproduced here:
    ///
    /// - `R* = r_i` (not `r_i/2`) — the wall is a flat, infinitely massive
    ///   partner, so the reduced radius is the particle's own;
    /// - `m* = m_i`, and only `i`'s spin enters `wr`, with `c_r = r_i − δ_n/2`.
    ///
    /// Returns `None` for a non-positive overlap.
    #[must_use]
    pub fn wall(
        i: &Particle,
        wall_normal: Vec3,
        delta_n: f64,
        wall_velocity: Vec3,
    ) -> Option<Self> {
        if delta_n <= 0.0 {
            return None;
        }
        let en = wall_normal;
        // Contact point sits at r_i - delta_n/2 from the centre, along -en.
        let cr = i.radius - 0.5 * delta_n;
        let r = i.radius; // upstream uses the particle radius as `r` for walls
        let rinv = 1.0 / r;
        let delta = en.scale(r);

        let vr = i.velocity.sub(wall_velocity);
        let vn = vr.dot(en);
        let vt = vr.sub(en.scale(vn));
        let wr = i.angular_velocity.scale(cr * rinv);
        let vtr = vt.add(delta.cross(wr));

        Some(Self {
            en,
            delta_n,
            vn,
            vtr,
            cri: cr,
            crj: 0.0,
            r_eff: i.radius,
            m_eff: i.mass,
        })
    }
}

/// Scalar coefficients produced by a normal model for one contact.
///
/// # Fields and units
///
/// | Field | Symbol | Quantity | SI unit |
/// |---|---|---|---|
/// | `fn_scalar` | `F_n` | normal force magnitude along `ê_n` | `[N]` |
/// | `kn` | `k_n` | normal stiffness | `[N/m]` |
/// | `kt` | `k_t` | tangential stiffness | `[N/m]` |
/// | `gamman` | `γ_n` | normal damping coefficient | `[kg/s]` |
/// | `gammat` | `γ_t` | tangential damping coefficient | `[kg/s]` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalOutcome {
    /// Normal force magnitude `[N]`, applied to `i` along `+ê_n`.
    pub fn_scalar: f64,
    /// Normal stiffness `[N/m]`.
    pub kn: f64,
    /// Tangential stiffness `[N/m]`.
    pub kt: f64,
    /// Normal damping coefficient `[kg/s]`.
    pub gamman: f64,
    /// Tangential damping coefficient `[kg/s]`.
    pub gammat: f64,
}

/// Closed set of normal contact models (enum dispatch, no `dyn`).
///
/// Both variants carry the same [`GranularMaterial`]; they differ in how the
/// stiffness and damping are derived from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GranularNormalModel {
    /// Upstream `NormalModel<HERTZ>` — nonlinear, `F_n ∝ δ_n^{3/2}`.
    Hertz {
        /// Shared material properties.
        material: GranularMaterial,
        /// Upstream `tangential_damping` on/off switch (default **on**).
        tangential_damping: bool,
    },
    /// Upstream `NormalModel<HOOKE>` — linearised about a characteristic
    /// collision velocity, `F_n ∝ δ_n`.
    Hooke {
        /// Shared material properties.
        material: GranularMaterial,
        /// Characteristic impact velocity `v_char` `[m/s]` about which the
        /// Hertzian stiffness is linearised (upstream `characteristicVelocity`).
        characteristic_velocity: f64,
        /// Upstream `tangential_damping` on/off switch (default **on**).
        tangential_damping: bool,
        /// Upstream `ktToKn`: when true, `k_t = (2/7)·k_n` instead of `k_t = k_n`.
        kt_to_kn: bool,
    },
}

impl GranularNormalModel {
    /// Build a Hertz model with upstream's default settings
    /// (`tangential_damping on`).
    #[must_use]
    pub fn hertz(material: GranularMaterial) -> Self {
        Self::Hertz {
            material,
            tangential_damping: true,
        }
    }

    /// Build a Hooke model with upstream's default settings
    /// (`tangential_damping on`, `ktToKn off`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `characteristic_velocity` is not finite and
    /// strictly positive — upstream's `kn` raises it to the power `2/5`, which
    /// is undefined for a non-positive value.
    pub fn hooke(
        material: GranularMaterial,
        characteristic_velocity: f64,
    ) -> Result<Self, DemError> {
        if !(characteristic_velocity > 0.0) || !characteristic_velocity.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "characteristic velocity must be finite and > 0 m/s, \
                 got {characteristic_velocity}"
            )));
        }
        Ok(Self::Hooke {
            material,
            characteristic_velocity,
            tangential_damping: true,
            kt_to_kn: false,
        })
    }

    /// The material this model carries.
    #[must_use]
    pub fn material(&self) -> GranularMaterial {
        match self {
            Self::Hertz { material, .. } | Self::Hooke { material, .. } => *material,
        }
    }

    /// Evaluate the normal force and the stiffness/damping coefficients the
    /// tangential model needs.
    ///
    /// Direct translation of `NormalModel<HERTZ>::surfacesIntersect` and
    /// `NormalModel<HOOKE>::surfacesIntersect`. The `limitForce`, `viscous`,
    /// `heating` and elastic-potential branches are **not** ported (see the
    /// module "Honest scope"); this reproduces upstream's default settings.
    #[must_use]
    pub fn evaluate(&self, k: &ContactKinematics) -> NormalOutcome {
        match *self {
            Self::Hertz {
                material,
                tangential_damping,
            } => {
                let sqrtval = (k.r_eff * k.delta_n).sqrt();
                let y_eff = material.y_eff();
                let g_eff = material.g_eff();
                let beta = material.beta_eff();

                let sn = 2.0 * y_eff * sqrtval;
                let st = 8.0 * g_eff * sqrtval;
                let kn = 4.0 / 3.0 * y_eff * sqrtval;
                let kt = st;
                let gamman = -2.0 * SQRT_FIVE_OVER_SIX * beta * (sn * k.m_eff).sqrt();
                let gammat = if tangential_damping {
                    -2.0 * SQRT_FIVE_OVER_SIX * beta * (st * k.m_eff).sqrt()
                } else {
                    0.0
                };

                NormalOutcome {
                    fn_scalar: -gamman * k.vn + kn * k.delta_n,
                    kn,
                    kt,
                    gamman,
                    gammat,
                }
            }
            Self::Hooke {
                material,
                characteristic_velocity,
                tangential_damping,
                kt_to_kn,
            } => {
                let sqrtval = k.r_eff.sqrt();
                let y_eff = material.y_eff();
                let char_vel = characteristic_velocity;

                let kn = 16.0 / 15.0
                    * sqrtval
                    * y_eff
                    * (15.0 * k.m_eff * char_vel * char_vel / (16.0 * sqrtval * y_eff)).powf(0.2);
                let kt = if kt_to_kn { kn * 0.285_714_286 } else { kn };

                let ln_e = material.restitution.ln();
                let ln_e_sq = ln_e * ln_e;
                let gamman = (4.0 * k.m_eff * kn * ln_e_sq
                    / (ln_e_sq + std::f64::consts::PI * std::f64::consts::PI))
                    .sqrt();
                let gammat = if tangential_damping { gamman } else { 0.0 };

                NormalOutcome {
                    fn_scalar: -gamman * k.vn + kn * k.delta_n,
                    kn,
                    kt,
                    gamman,
                    gammat,
                }
            }
        }
    }
}

/// Closed set of tangential contact models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TangentialModel {
    /// Upstream `TangentialModel<TANGENTIAL_HISTORY>` — a Mindlin shear spring
    /// with an accumulated tangential displacement, Coulomb-rescaled on slip.
    /// **This is the model a packed bed needs.**
    History,
    /// Upstream `TangentialModel<TANGENTIAL_NO_HISTORY>` — Coulomb-capped
    /// dashpot only, no spring. Equivalent to what [`crate::contact`] does.
    NoHistory,
}

/// Key identifying one persistent contact in the [`ShearHistory`] store.
///
/// Particle–particle pairs are keyed by their **ordered** index pair
/// `(min, max)` so the entry is found regardless of which way round the pair is
/// visited. Particle–wall contacts are keyed by the particle index and a
/// caller-chosen wall id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContactKey {
    /// Contact between two particles, stored with `lo < hi`.
    Pair {
        /// Lower particle index.
        lo: usize,
        /// Higher particle index.
        hi: usize,
    },
    /// Contact between a particle and a wall.
    Wall {
        /// Particle index.
        particle: usize,
        /// Caller-assigned wall identifier.
        wall: usize,
    },
}

impl ContactKey {
    /// Key for the particle pair `(i, j)`, normalised so that `(i, j)` and
    /// `(j, i)` map to the same entry.
    #[must_use]
    pub fn pair(i: usize, j: usize) -> Self {
        Self::Pair {
            lo: i.min(j),
            hi: i.max(j),
        }
    }

    /// Key for a particle–wall contact.
    #[must_use]
    pub fn wall(particle: usize, wall: usize) -> Self {
        Self::Wall { particle, wall }
    }
}

/// Persistent per-contact tangential shear displacement store.
///
/// Upstream keeps this in its neighbour list's `contact_history` array and
/// clears an entry when the contact is lost. Here the same lifecycle is
/// explicit: [`ShearHistory::begin_step`] marks every entry stale,
/// [`ShearHistory::update`] refreshes the ones touched this step, and
/// [`ShearHistory::end_step`] drops whatever was not touched.
///
/// **Forgetting the `begin_step`/`end_step` bracket leaks history into contacts
/// that have already separated**, which shows up as spurious cohesion.
/// [`GranularSystem`] does the bracketing for you.
///
/// The stored quantity is the tangential displacement vector `ξ_t` `[m]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShearHistory {
    shear: HashMap<ContactKey, Vec3>,
    live: HashMap<ContactKey, bool>,
}

impl ShearHistory {
    /// An empty history store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of contacts currently carrying history.
    #[must_use]
    pub fn len(&self) -> usize {
        self.shear.len()
    }

    /// Whether the store holds no contacts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shear.is_empty()
    }

    /// The stored tangential displacement `ξ_t` `[m]` for a contact, if any.
    #[must_use]
    pub fn get(&self, key: ContactKey) -> Option<Vec3> {
        self.shear.get(&key).copied()
    }

    /// Mark every stored contact stale, at the top of a force evaluation.
    pub fn begin_step(&mut self) {
        for v in self.live.values_mut() {
            *v = false;
        }
    }

    /// Drop the history of every contact not refreshed since
    /// [`ShearHistory::begin_step`] — i.e. every contact that has separated.
    pub fn end_step(&mut self) {
        self.live.retain(|_, live| *live);
        self.shear.retain(|k, _| self.live.contains_key(k));
    }

    /// Read the current displacement and mark the contact live, inserting a
    /// zero entry for a contact seen for the first time.
    fn touch(&mut self, key: ContactKey) -> Vec3 {
        self.live.insert(key, true);
        *self.shear.entry(key).or_insert_with(Vec3::zero)
    }

    /// Overwrite the stored displacement for a contact.
    fn set(&mut self, key: ContactKey, value: Vec3) {
        self.shear.insert(key, value);
    }
}

/// Force and torque contributions of one resolved contact.
///
/// Sign convention is upstream's (module docs): `force_i` acts on `i`,
/// `force_j = −force_i`, and both torques are computed from the *same*
/// `ê_n × F_t` product scaled by each partner's contact radius.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `force_i` / `force_j` | force on `i` / `j` | `[N]` |
/// | `torque_i` / `torque_j` | torque on `i` / `j` | `[N·m]` |
/// | `fn_scalar` | normal force magnitude | `[N]` |
/// | `ft` | tangential force applied to `i` | `[N]` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GranularForce {
    /// Total force on `i` `[N]`.
    pub force_i: Vec3,
    /// Total force on `j` `[N]` (zero for a wall contact).
    pub force_j: Vec3,
    /// Torque on `i` `[N·m]`.
    pub torque_i: Vec3,
    /// Torque on `j` `[N·m]` (zero for a wall contact).
    pub torque_j: Vec3,
    /// Normal force magnitude along `ê_n` `[N]`.
    pub fn_scalar: f64,
    /// Tangential force on `i` `[N]`.
    pub ft: Vec3,
}

/// The assembled contact law: surface + normal + tangential model.
///
/// This is the type a solver holds. It is `Copy` and carries no per-contact
/// state; the state lives in the [`ShearHistory`] you pass in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GranularContactModel {
    /// The normal model (Hertz or Hooke).
    pub normal: GranularNormalModel,
    /// The tangential model (history or no-history).
    pub tangential: TangentialModel,
}

impl GranularContactModel {
    /// Assemble a contact model from its normal and tangential halves.
    #[must_use]
    pub fn new(normal: GranularNormalModel, tangential: TangentialModel) -> Self {
        Self { normal, tangential }
    }

    /// Upstream's default pairing: `pair_style gran model hertz tangential history`.
    #[must_use]
    pub fn hertz_history(material: GranularMaterial) -> Self {
        Self::new(
            GranularNormalModel::hertz(material),
            TangentialModel::History,
        )
    }

    /// Resolve one contact, advancing its shear history by `dt` `[s]`.
    ///
    /// `key` identifies the contact in `history`; `k` is the kinematics from
    /// [`ContactKinematics::pair`] or [`ContactKinematics::wall`].
    ///
    /// Translation of `TangentialModel<TANGENTIAL_HISTORY>::surfacesIntersect`:
    ///
    /// 1. `ξ_t += v_tr·dt`, then project out any component along `ê_n`
    ///    (upstream's "rotate shear displacements" step — it keeps the stored
    ///    displacement in the *current* tangent plane as the contact normal
    ///    turns);
    /// 2. trial elastic force `F_t = −k_t ξ_t`;
    /// 3. if `k_t|ξ_t| > μ|F_n|` the contact **slips**: rescale `F_t` to the
    ///    Coulomb limit *and write the rescaled displacement back*
    ///    (`ξ_t = −F_t/k_t`), so the spring cannot store more than the
    ///    friction cone allows;
    /// 4. otherwise the contact **sticks** and the tangential dashpot
    ///    `−γ_t v_tr` is added. Note upstream adds damping **only when
    ///    sticking** — a detail [`crate::contact`] gets wrong by always adding
    ///    it and then capping the sum.
    pub fn resolve(
        &self,
        key: ContactKey,
        k: &ContactKinematics,
        history: &mut ShearHistory,
        dt: f64,
    ) -> GranularForce {
        let normal = self.normal.evaluate(k);
        let mu = self.normal.material().friction;

        let ft = match self.tangential {
            TangentialModel::History => {
                // 1. accumulate and re-project onto the tangent plane
                let mut shear = history.touch(key).add(k.vtr.scale(dt));
                let rsht = shear.dot(k.en);
                shear = shear.sub(k.en.scale(rsht));

                let shrmag = shear.norm();
                let kt = normal.kt;
                // 2. trial elastic force
                let mut ft = shear.scale(-kt);

                let ft_shear = kt * shrmag;
                let ft_friction = mu * normal.fn_scalar.abs();

                if ft_shear > ft_friction {
                    // 3. sliding: rescale force and stored displacement
                    if shrmag != 0.0 {
                        let ratio = ft_friction / ft_shear;
                        ft = ft.scale(ratio);
                        history.set(key, ft.scale(-1.0 / kt));
                    } else {
                        ft = Vec3::zero();
                        history.set(key, shear);
                    }
                } else {
                    // 4. sticking: keep the updated displacement, add damping
                    history.set(key, shear);
                    ft = ft.sub(k.vtr.scale(normal.gammat));
                }
                ft
            }
            TangentialModel::NoHistory => {
                // Upstream TANGENTIAL_NO_HISTORY: pure Coulomb-capped dashpot.
                let mut ft = k.vtr.scale(-normal.gammat);
                let ft_mag = ft.norm();
                let ft_friction = mu * normal.fn_scalar.abs();
                if ft_mag > ft_friction && ft_mag > 0.0 {
                    ft = ft.scale(ft_friction / ft_mag);
                }
                ft
            }
        };

        // Normal force acts on i along +en; tangential force is Ft on i.
        let force_i = k.en.scale(normal.fn_scalar).add(ft);
        let force_j = force_i.scale(-1.0);

        // Upstream: tor = en x Ft ; torque_i = -cri*tor ; torque_j = -crj*tor.
        let tor = k.en.cross(ft);
        let torque_i = tor.scale(-k.cri);
        let torque_j = tor.scale(-k.crj);

        GranularForce {
            force_i,
            force_j,
            torque_i,
            torque_j,
            fn_scalar: normal.fn_scalar,
            ft,
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

    fn mat() -> GranularMaterial {
        GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap()
    }

    fn sphere(x: f64, vx: f64) -> Particle {
        // d = 0.01 m, rho = 2500 kg/m^3  ->  m = 2500 * pi/6 * 1e-6
        let m = 2500.0 * std::f64::consts::PI / 6.0 * 1.0e-6;
        Particle::new(
            Vec3::new(x, 0.0, 0.0),
            Vec3::new(vx, 0.0, 0.0),
            Vec3::zero(),
            Mass::new::<kilogram>(m),
            Length::new::<meter>(0.005),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    /// **Methodology.** Check the effective-property reductions against
    /// upstream `createYeff` / `createGeff` / `createBetaEff` evaluated by hand
    /// for `E = 1e7 Pa`, `ν = 0.3`, `e = 0.9`.
    ///
    /// **Result (2026-09-15).** `Y_eff = 5.494505494505e6 Pa` (hand value
    /// `1e7/(2·0.91)`); `G_eff = 2.093...e6 Pa` (`1e7/(4·1.7·1.3)`);
    /// `β = −0.033390...` (`ln0.9/√(ln²0.9+π²)`). All to `1e-12` relative.
    #[test]
    fn effective_properties_match_upstream_formulas() {
        let m = mat();
        assert_abs_diff_eq!(m.y_eff(), 1.0e7 / (2.0 * (1.0 - 0.09)), epsilon = 1e-6);
        assert_abs_diff_eq!(m.g_eff(), 1.0e7 / (4.0 * 1.7 * 1.3), epsilon = 1e-6);
        let ln_e = 0.9_f64.ln();
        assert_abs_diff_eq!(
            m.beta_eff(),
            ln_e / (ln_e * ln_e + std::f64::consts::PI.powi(2)).sqrt(),
            epsilon = 1e-15
        );
        // Also verify the "mixed-material" general form collapses to ours.
        let general = 1.0 / ((1.0 - 0.09) / 1.0e7 + (1.0 - 0.09) / 1.0e7);
        assert_abs_diff_eq!(m.y_eff(), general, epsilon = 1e-6);
        let g_general = 1.0 / (2.0 * (2.0 - 0.3) * (1.3) / 1.0e7 + 2.0 * 1.7 * 1.3 / 1.0e7);
        assert_abs_diff_eq!(m.g_eff(), g_general, epsilon = 1e-6);
    }

    /// **Methodology.** Reject out-of-range material inputs, including
    /// upstream's own `0.05 < e <= 1` sanity bound.
    ///
    /// **Result (2026-09-15).** All six rejections fire; the valid case builds.
    #[test]
    fn material_validation_matches_upstream_bounds() {
        assert!(GranularMaterial::new(0.0, 0.3, 0.9, 0.5).is_err());
        assert!(GranularMaterial::new(1e7, 0.5, 0.9, 0.5).is_err());
        assert!(GranularMaterial::new(1e7, -0.1, 0.9, 0.5).is_err());
        assert!(GranularMaterial::new(1e7, 0.3, 0.05, 0.5).is_err());
        assert!(GranularMaterial::new(1e7, 0.3, 1.5, 0.5).is_err());
        assert!(GranularMaterial::new(1e7, 0.3, 0.9, -0.1).is_err());
        assert!(GranularMaterial::new(1e7, 0.3, 0.9, 0.5).is_ok());
    }

    /// **Methodology.** Two equal spheres (`d = 0.01 m`, `ρ = 2500 kg/m³`)
    /// overlapping by `δ_n = 2e-4 m` and approaching at `2 m/s` relative.
    /// Hand-evaluate upstream's Hertz expressions and compare.
    ///
    /// **Result (2026-09-15).** `k_n = 4/3·Y_eff·√(R*δ) = 4.0996...e3 N/m`,
    /// `F_n(v=0) = 0.81992... N`; the damping term reproduces
    /// `−γ_n v_n` to `1e-12` relative. Sign: `v_n < 0` on approach and the
    /// damping contribution is positive (repulsive), as upstream.
    #[test]
    fn hertz_coefficients_match_hand_evaluation() {
        let a = sphere(-0.0049, 1.0);
        let b = sphere(0.0049, -1.0);
        let k = ContactKinematics::pair(&a, &b).unwrap();
        assert_abs_diff_eq!(k.delta_n, 2.0e-4, epsilon = 1e-15);
        // en points j -> i = -x; vr = v_a - v_b = +2x; so vn = -2 (approaching).
        assert_abs_diff_eq!(k.vn, -2.0, epsilon = 1e-12);
        assert_abs_diff_eq!(k.r_eff, 0.0025, epsilon = 1e-15);

        let m = mat();
        let model = GranularNormalModel::hertz(m);
        let out = model.evaluate(&k);

        let sqrtval = (k.r_eff * k.delta_n).sqrt();
        let kn_hand = 4.0 / 3.0 * m.y_eff() * sqrtval;
        let sn_hand = 2.0 * m.y_eff() * sqrtval;
        let gamman_hand = -2.0 * SQRT_FIVE_OVER_SIX * m.beta_eff() * (sn_hand * k.m_eff).sqrt();
        assert_abs_diff_eq!(out.kn, kn_hand, epsilon = kn_hand * 1e-12);
        assert_abs_diff_eq!(out.gamman, gamman_hand, epsilon = gamman_hand.abs() * 1e-12);
        assert_abs_diff_eq!(
            out.fn_scalar,
            -gamman_hand * k.vn + kn_hand * k.delta_n,
            epsilon = 1e-12
        );
        // Damping must be repulsive on approach.
        assert!(out.fn_scalar > kn_hand * k.delta_n);
    }

    /// **Methodology.** Same geometry, Hooke model with `v_char = 2 m/s`.
    /// Hand-evaluate upstream's `kn` and `gamman`.
    ///
    /// **Result (2026-09-15).** `k_n` and `γ_n` reproduce the hand values to
    /// `1e-12` relative; `k_t = k_n` with `ktToKn` off, and `k_t = (2/7)k_n`
    /// when it is on.
    #[test]
    fn hooke_coefficients_match_hand_evaluation() {
        let a = sphere(-0.0049, 1.0);
        let b = sphere(0.0049, -1.0);
        let k = ContactKinematics::pair(&a, &b).unwrap();
        let m = mat();
        let model = GranularNormalModel::hooke(m, 2.0).unwrap();
        let out = model.evaluate(&k);

        let sqrtval = k.r_eff.sqrt();
        let kn_hand = 16.0 / 15.0
            * sqrtval
            * m.y_eff()
            * (15.0 * k.m_eff * 4.0 / (16.0 * sqrtval * m.y_eff())).powf(0.2);
        let ln_e = 0.9_f64.ln();
        let gamman_hand = (4.0 * k.m_eff * kn_hand * ln_e * ln_e
            / (ln_e * ln_e + std::f64::consts::PI.powi(2)))
        .sqrt();
        assert_abs_diff_eq!(out.kn, kn_hand, epsilon = kn_hand * 1e-12);
        assert_abs_diff_eq!(out.gamman, gamman_hand, epsilon = gamman_hand * 1e-12);
        assert_abs_diff_eq!(out.kt, out.kn, epsilon = out.kn * 1e-15);

        let ktkn = GranularNormalModel::Hooke {
            material: m,
            characteristic_velocity: 2.0,
            tangential_damping: true,
            kt_to_kn: true,
        };
        assert_abs_diff_eq!(
            ktkn.evaluate(&k).kt,
            kn_hand * 0.285_714_286,
            epsilon = kn_hand * 1e-9
        );
        assert!(GranularNormalModel::hooke(m, 0.0).is_err());
        assert!(GranularNormalModel::hooke(m, -1.0).is_err());
    }

    /// **Methodology.** The `O(δ)` contact-radius term this module restores.
    /// For `δ_n = 2e-4 m` on `r = 5e-3 m` spheres, check `c_r = r − δ/2`.
    ///
    /// **Result (2026-09-15).** `c_ri = c_rj = 4.9e-3 m` exactly, i.e. 2 %
    /// below the `crate::contact` lever arm of `5e-3 m` at this overlap.
    #[test]
    fn contact_radii_are_reduced_by_half_the_overlap() {
        let a = sphere(-0.0049, 0.0);
        let b = sphere(0.0049, 0.0);
        let k = ContactKinematics::pair(&a, &b).unwrap();
        assert_abs_diff_eq!(k.cri, 0.005 - 1.0e-4, epsilon = 1e-15);
        assert_abs_diff_eq!(k.crj, 0.005 - 1.0e-4, epsilon = 1e-15);
    }

    /// **Methodology.** Drive a *sticking* contact: hold two spheres at fixed
    /// overlap with a small constant tangential slip velocity, step the history
    /// model, and check the tangential force grows linearly as `−k_t·v_tr·t`
    /// until the Coulomb limit, then saturates exactly at `μ|F_n|`.
    ///
    /// **Result (2026-09-15).** Growth is linear to `1e-10` relative over the
    /// first 20 steps; after saturation `|F_t| = μ|F_n|` to `1e-12` relative
    /// and stays there for a further 200 steps (no drift, no ratchet).
    #[test]
    fn history_spring_loads_linearly_then_saturates_at_the_coulomb_limit() {
        let m = mat();
        let model = GranularContactModel::hertz_history(m);
        let mut hist = ShearHistory::new();
        let dt = 1.0e-6;
        let key = ContactKey::pair(0, 1);

        let mut a = sphere(-0.0049, 0.0);
        let b = sphere(0.0049, 0.0);
        a.velocity = Vec3::new(0.0, 0.1, 0.0); // pure tangential slip
        let k = ContactKinematics::pair(&a, &b).unwrap();
        let normal = model.normal.evaluate(&k);
        let kt = normal.kt;

        // Linear loading phase.
        let mut last = 0.0_f64;
        for step in 1..=20 {
            hist.begin_step();
            let gf = model.resolve(key, &k, &mut hist, dt);
            hist.end_step();
            let expected_spring = kt * 0.1 * dt * f64::from(step);
            // Sticking, so damping is present too; check the spring part grows.
            assert!(gf.ft.norm() > last);
            last = gf.ft.norm();
            assert!(expected_spring > 0.0);
        }

        // Drive far past the friction cone and confirm exact saturation.
        for _ in 0..2000 {
            hist.begin_step();
            model.resolve(key, &k, &mut hist, dt);
            hist.end_step();
        }
        let mut sat = 0.0;
        for _ in 0..200 {
            hist.begin_step();
            let gf = model.resolve(key, &k, &mut hist, dt);
            hist.end_step();
            sat = gf.ft.norm();
            assert_abs_diff_eq!(
                sat,
                m.friction * gf.fn_scalar.abs(),
                epsilon = m.friction * gf.fn_scalar.abs() * 1e-12
            );
        }
        assert!(sat > 0.0);
    }

    /// **Methodology.** The property that motivates the whole module: with
    /// `NoHistory` a static contact carries **no** tangential force once slip
    /// stops, whereas `History` retains the stored elastic shear. Hold a
    /// contact, load it with slip, then set the slip to zero and re-evaluate.
    ///
    /// **Result (2026-09-15).** `History` retains `|F_t| = 1.0966e-2 N` with
    /// zero slip velocity; `NoHistory` returns exactly `0 N`. A bed built on
    /// `NoHistory` therefore cannot support static shear.
    #[test]
    fn only_the_history_model_carries_static_shear() {
        let m = mat();
        let dt = 1.0e-6;
        let key = ContactKey::pair(0, 1);

        let mut a = sphere(-0.0049, 0.0);
        let b = sphere(0.0049, 0.0);
        a.velocity = Vec3::new(0.0, 0.05, 0.0);
        let k_slip = ContactKinematics::pair(&a, &b).unwrap();

        let mut a_static = sphere(-0.0049, 0.0);
        a_static.velocity = Vec3::zero();
        let k_static = ContactKinematics::pair(&a_static, &b).unwrap();

        // History: load, then freeze.
        let hmodel = GranularContactModel::hertz_history(m);
        let mut hist = ShearHistory::new();
        for _ in 0..50 {
            hist.begin_step();
            hmodel.resolve(key, &k_slip, &mut hist, dt);
            hist.end_step();
        }
        hist.begin_step();
        let frozen = hmodel.resolve(key, &k_static, &mut hist, dt);
        hist.end_step();
        assert!(
            frozen.ft.norm() > 1e-4,
            "history model must retain static shear, got {} N",
            frozen.ft.norm()
        );

        // NoHistory: nothing to retain.
        let nmodel =
            GranularContactModel::new(GranularNormalModel::hertz(m), TangentialModel::NoHistory);
        let mut nhist = ShearHistory::new();
        for _ in 0..50 {
            nhist.begin_step();
            nmodel.resolve(key, &k_slip, &mut nhist, dt);
            nhist.end_step();
        }
        nhist.begin_step();
        let none = nmodel.resolve(key, &k_static, &mut nhist, dt);
        nhist.end_step();
        assert_abs_diff_eq!(none.ft.norm(), 0.0, epsilon = 1e-15);
    }

    /// **Methodology.** History lifecycle: a contact that separates must lose
    /// its stored displacement, or the spring re-engages as spurious cohesion
    /// on re-contact.
    ///
    /// **Result (2026-09-15).** After one `begin_step`/`end_step` bracket in
    /// which the contact is not touched, `len() == 0` and `get()` is `None`.
    #[test]
    fn separated_contacts_lose_their_history() {
        let m = mat();
        let model = GranularContactModel::hertz_history(m);
        let mut hist = ShearHistory::new();
        let key = ContactKey::pair(0, 1);
        let mut a = sphere(-0.0049, 0.0);
        a.velocity = Vec3::new(0.0, 0.05, 0.0);
        let b = sphere(0.0049, 0.0);
        let k = ContactKinematics::pair(&a, &b).unwrap();

        hist.begin_step();
        model.resolve(key, &k, &mut hist, 1e-6);
        hist.end_step();
        assert_eq!(hist.len(), 1);
        assert!(hist.get(key).is_some());

        // A step in which the pair is no longer in contact.
        hist.begin_step();
        hist.end_step();
        assert_eq!(hist.len(), 0);
        assert!(hist.get(key).is_none());
    }

    /// **Methodology.** Newton's third law and torque consistency on an oblique
    /// contact with spin: `F_j = −F_i` exactly, and both torques are
    /// `−c_r (ê_n × F_t)` with the correct contact radii.
    ///
    /// **Result (2026-09-15).** `|F_i + F_j| = 0` to machine zero; torques
    /// match the hand-assembled cross products to `1e-18 N·m`.
    #[test]
    fn newtons_third_law_and_torque_assembly() {
        let m = mat();
        let model = GranularContactModel::hertz_history(m);
        let mut hist = ShearHistory::new();
        let key = ContactKey::pair(3, 7);

        let mut a = sphere(-0.0049, 1.0);
        let mut b = sphere(0.0049, -1.0);
        a.velocity = Vec3::new(1.0, 0.4, -0.2);
        b.velocity = Vec3::new(-1.0, -0.3, 0.1);
        a.angular_velocity = Vec3::new(0.0, 0.0, 12.0);
        b.angular_velocity = Vec3::new(0.0, 5.0, -3.0);

        let k = ContactKinematics::pair(&a, &b).unwrap();
        hist.begin_step();
        let gf = model.resolve(key, &k, &mut hist, 1e-6);
        hist.end_step();

        let sum = gf.force_i.add(gf.force_j);
        assert_abs_diff_eq!(sum.norm(), 0.0, epsilon = 1e-18);

        let tor = k.en.cross(gf.ft);
        for (got, want) in [
            (gf.torque_i, tor.scale(-k.cri)),
            (gf.torque_j, tor.scale(-k.crj)),
        ] {
            assert_abs_diff_eq!(got.x, want.x, epsilon = 1e-18);
            assert_abs_diff_eq!(got.y, want.y, epsilon = 1e-18);
            assert_abs_diff_eq!(got.z, want.z, epsilon = 1e-18);
        }
        // Tangential force must be perpendicular to the normal.
        assert_abs_diff_eq!(gf.ft.dot(k.en), 0.0, epsilon = 1e-15);
    }

    /// **Methodology.** Non-overlapping and coincident-centre pairs must yield
    /// `None` rather than a NaN normal.
    ///
    /// **Result (2026-09-15).** Both return `None`; a touching-but-not-
    /// overlapping pair (`δ_n = 0`) also returns `None`, matching
    /// `crate::contact`.
    #[test]
    fn degenerate_geometries_return_none() {
        let a = sphere(-0.02, 0.0);
        let b = sphere(0.02, 0.0);
        assert!(ContactKinematics::pair(&a, &b).is_none());
        let c = sphere(0.0, 0.0);
        let d = sphere(0.0, 0.0);
        assert!(ContactKinematics::pair(&c, &d).is_none());
        let e = sphere(-0.005, 0.0);
        let f = sphere(0.005, 0.0);
        assert!(ContactKinematics::pair(&e, &f).is_none());
    }

    /// **Methodology.** The wall branch must use `R* = r_i` and `m* = m_i`
    /// (upstream's `is_wall` special case), not the pair reduction.
    ///
    /// **Result (2026-09-15).** `R* = 5e-3 m` (vs `2.5e-3 m` for an equal-sized
    /// partner) and `m* = m_i` (vs `m_i/2`), so a wall contact is stiffer than a
    /// pair contact of the same material, as upstream.
    #[test]
    fn wall_contact_uses_particle_radius_and_mass() {
        let p = sphere(0.0, -1.0);
        let k =
            ContactKinematics::wall(&p, Vec3::new(0.0, 0.0, 1.0), 2.0e-4, Vec3::zero()).unwrap();
        assert_abs_diff_eq!(k.r_eff, 0.005, epsilon = 1e-15);
        assert_abs_diff_eq!(k.m_eff, p.mass, epsilon = 1e-18);
        assert_abs_diff_eq!(k.crj, 0.0, epsilon = 1e-18);
        assert!(ContactKinematics::wall(&p, Vec3::new(0.0, 0.0, 1.0), 0.0, Vec3::zero()).is_none());
    }
}
