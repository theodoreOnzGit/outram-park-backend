// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/surface_model_default.h, src/normal_model_hertz.h,
//                      src/normal_model_hooke.h, src/tangential_model_history.h,
//                      src/tangential_model_no_history.h, src/rolling_model_cdt.h,
//                      src/global_properties.cpp
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
//! ## Unit system: SI only, and why the port drops two conversion factors
//!
//! Upstream divides its stiffnesses by `force->nktv2p` and scales its force-to-
//! velocity step by `force->ftm2v`:
//!
//! ```text
//!   kn /= force->nktv2p;   kt /= force->nktv2p;      // normal models
//!   dtf = 0.5 * dt * force->ftm2v;                   // fix_nve_sphere
//! ```
//!
//! Both constants are **exactly `1.0` for `units si`** (`update.cpp`, the `si`
//! branch), which is the unit system every case in this crate uses and the only
//! one its `uom`-typed API admits. The port therefore omits both
//! multiplications rather than carrying a factor that is identically one.
//!
//! This is a deliberate simplification, recorded here because it is the one
//! place the translation is not literal: **it is exact for SI and wrong for any
//! other LIGGGHTS unit style** (`lj`, `real`, `metal`, `cgs`, … have
//! `nktv2p` = 1.0, 68568.415, 1.6021765e6, 2.94210108e13 respectively). If this
//! crate ever grows a non-SI path, both factors must come back.
//!
//! ## Honest scope
//!
//! - **Implemented:** default surface model; Hertz and Hooke normal models;
//!   history and no-history tangential models; per-pair shear-history storage
//!   with Coulomb rescaling of the stored displacement; the CDT
//!   (constant-directional-torque) rolling model.
//! - **Not implemented:** ~~cohesion models, the EPSD rolling family,~~
//!   **CORRECTED 2026-11-11 (gh:#1410, #1411): `sjkr`, `sjkr2` and the
//!   `easo`/`washino` capillary cohesion models, the EPSD family and
//!   multicontact are translated (checked: `GranularCohesion`,
//!   `crate::capillary`, `tests/*_cross_code.rs`)**;
//!   superquadrics,
//!   the `limitForce`/`viscous`/`heating`/elastic-potential switches, and
//!   ~~mixed-material property matrices~~ **(`MaterialTable` exists)**.
//! - **Verified against upstream** — see `docs/verification-and-validation.md`
//!   and `tests/liggghts_cross_code.rs`.

use petir::mathf::RealMath;
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
        // Upstream `createYeff` with i = j:
        //   1./((1.-pow(vi,2.))/Yi + (1.-pow(vj,2.))/Yj)
        // (g++ folds `pow(x, 2.)` to `x*x`). Not `E / (2(1-nu^2))`, which
        // rounds differently.
        let nu = self.poisson_ratio;
        let e = self.youngs_modulus;
        1.0 / ((1.0 - nu * nu) / e + (1.0 - nu * nu) / e)
    }

    /// Effective shear modulus `G_eff` `[Pa]`.
    ///
    /// Upstream `createGeff`: `1/(2(2−ν_i)(1+ν_i)/E_i + 2(2−ν_j)(1+ν_j)/E_j)`,
    /// reducing for one shared material to `E / (4(2 − ν)(1 + ν))`.
    #[must_use]
    pub fn g_eff(&self) -> f64 {
        // Upstream `createGeff` with i = j:
        //   1./(2.*(2.-vi)*(1.+vi)/Yi + 2.*(2.-vj)*(1.+vj)/Yj)
        let nu = self.poisson_ratio;
        let e = self.youngs_modulus;
        1.0 / (2.0 * (2.0 - nu) * (1.0 + nu) / e + 2.0 * (2.0 - nu) * (1.0 + nu) / e)
    }

    /// Damping ratio `β_eff` `[-]`, upstream `createBetaEff`.
    ///
    /// `β = ln(e) / √(ln²(e) + π²)`. Negative for `e < 1`, zero at `e = 1`.
    #[must_use]
    pub fn beta_eff(&self) -> f64 {
        let ln_e = self.restitution.r_ln();
        ln_e / (ln_e * ln_e + std::f64::consts::PI * std::f64::consts::PI).sqrt()
    }
}

/// Largest number of atom types a [`MaterialTable`] holds (fixed-size arrays
/// keep [`GranularContactModel`] `Copy`).
pub const MAX_TYPES: usize = 4;

/// The per-type-pair coefficients a contact needs: upstream's `Yeff`, `Geff`,
/// `coeffRestLog`, `betaeff` and `coeffFrict` matrices evaluated at one
/// `(itype, jtype)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairProps {
    /// Effective Young's modulus `Y_eff` `[Pa]`.
    pub y_eff: f64,
    /// Effective shear modulus `G_eff` `[Pa]`.
    pub g_eff: f64,
    /// `ln(e)`, upstream `coeffRestLog` `[-]`.
    pub ln_e: f64,
    /// Damping ratio `beta_eff` `[-]`.
    pub beta_eff: f64,
    /// Coulomb friction coefficient `mu` `[-]`.
    pub friction: f64,
}

impl PairProps {
    /// Coefficients of one material against itself (the single-material case).
    #[must_use]
    pub fn from_material(m: &GranularMaterial) -> Self {
        let ln_e = m.restitution.r_ln();
        Self {
            y_eff: m.y_eff(),
            g_eff: m.g_eff(),
            ln_e,
            beta_eff: ln_e / (ln_e * ln_e + std::f64::consts::PI * std::f64::consts::PI).sqrt(),
            friction: m.friction,
        }
    }
}

/// Per-atom-type material data and per-type-pair matrices: the port of
/// LIGGGHTS' `fix property/global youngsModulus|poissonsRatio peratomtype`
/// and `coefficientRestitution|coefficientFriction peratomtypepair`, and of
/// the `global_properties.cpp` mixing rules built from them:
///
/// - `Y_eff[i][j] = 1/((1-nu_i^2)/E_i + (1-nu_j^2)/E_j)` (`createYeff`);
/// - `G_eff[i][j] = 1/(2(2-nu_i)(1+nu_i)/E_i + 2(2-nu_j)(1+nu_j)/E_j)`
///   (`createGeff`);
/// - `ln e[i][j] = log(e_ij)`, `beta_eff = ln e / sqrt(ln^2 e + pi^2)`
///   (`createCoeffRestLog`, `createBetaEff`);
/// - `e_ij` and `mu_ij` are **user-supplied matrices**, not mixed from per-type
///   values (upstream requires them symmetric; so does [`MaterialTable::new`]).
///
/// At most [`MAX_TYPES`] types. Particle-wall contacts use the particle's own
/// type for both partners (the wall material is not modelled separately).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialTable {
    n_types: usize,
    youngs: [f64; MAX_TYPES],
    poisson: [f64; MAX_TYPES],
    restitution: [[f64; MAX_TYPES]; MAX_TYPES],
    friction: [[f64; MAX_TYPES]; MAX_TYPES],
}

impl MaterialTable {
    /// Build a table for `types.len()` atom types (index 0 is atom type 1).
    ///
    /// `types[t] = (E, nu)`; `restitution` and `friction` are square,
    /// symmetric `n x n` matrices.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for 0 or more than [`MAX_TYPES`] types, a
    /// matrix of the wrong shape or not symmetric, or a value outside the
    /// ranges of [`GranularMaterial::new`].
    pub fn new(
        types: &[(f64, f64)],
        restitution: &[&[f64]],
        friction: &[&[f64]],
    ) -> Result<Self, DemError> {
        let n = types.len();
        if n == 0 || n > MAX_TYPES {
            return Err(DemError::InvalidInput(format!(
                "a material table needs 1..={MAX_TYPES} types, got {n}"
            )));
        }
        let mut t = Self {
            n_types: n,
            youngs: [1.0; MAX_TYPES],
            poisson: [0.0; MAX_TYPES],
            restitution: [[1.0; MAX_TYPES]; MAX_TYPES],
            friction: [[0.0; MAX_TYPES]; MAX_TYPES],
        };
        for (i, &(e, nu)) in types.iter().enumerate() {
            // Validate through the single-material constructor.
            GranularMaterial::new(e, nu, 1.0, 0.0)?;
            t.youngs[i] = e;
            t.poisson[i] = nu;
        }
        for (name, src, dst) in [
            ("restitution", restitution, &mut t.restitution),
            ("friction", friction, &mut t.friction),
        ] {
            if src.len() != n || src.iter().any(|row| row.len() != n) {
                return Err(DemError::InvalidInput(format!(
                    "{name} matrix must be {n} x {n}"
                )));
            }
            for i in 0..n {
                for j in 0..n {
                    if src[i][j] != src[j][i] {
                        return Err(DemError::InvalidInput(format!(
                            "{name} matrix must be symmetric (LIGGGHTS fix property/global \
                             peratomtypepair requires it); [{i}][{j}] != [{j}][{i}]"
                        )));
                    }
                    dst[i][j] = src[i][j];
                }
            }
        }
        for i in 0..n {
            for j in 0..n {
                GranularMaterial::new(1.0, 0.0, t.restitution[i][j], t.friction[i][j])?;
            }
        }
        Ok(t)
    }

    /// Number of atom types.
    #[must_use]
    pub fn n_types(&self) -> usize {
        self.n_types
    }

    /// The coefficients for a contact between types `i` and `j` (0-based).
    ///
    /// # Panics
    ///
    /// If `i` or `j` is `>= n_types()` (a particle type outside the table is a
    /// caller bug; `GranularSystem::with_particle_types` checks it up front).
    #[must_use]
    pub fn pair_props(&self, i: usize, j: usize) -> PairProps {
        assert!(
            i < self.n_types && j < self.n_types,
            "atom type out of range"
        );
        let (yi, yj) = (self.youngs[i], self.youngs[j]);
        let (vi, vj) = (self.poisson[i], self.poisson[j]);
        let ln_e = self.restitution[i][j].r_ln();
        PairProps {
            y_eff: 1.0 / ((1.0 - vi * vi) / yi + (1.0 - vj * vj) / yj),
            g_eff: 1.0 / (2.0 * (2.0 - vi) * (1.0 + vi) / yi + 2.0 * (2.0 - vj) * (1.0 + vj) / yj),
            ln_e,
            beta_eff: ln_e / (ln_e * ln_e + std::f64::consts::PI * std::f64::consts::PI).sqrt(),
            friction: self.friction[i][j],
        }
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
/// | `omega_i` / `omega_j` | `ω_i`, `ω_j` | angular velocities | `[rad/s]` |
/// | `is_wall` | — | particle–wall contact flag | `[-]` |
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
    /// Angular velocity of `i` `[rad/s]` (needed by the rolling model).
    pub omega_i: Vec3,
    /// Angular velocity of `j` `[rad/s]`; zero for a wall contact.
    pub omega_j: Vec3,
    /// Whether this is a particle–wall contact (upstream's `is_wall`), which
    /// changes `R*`, `m*` and the rolling model's rolling-velocity branch.
    pub is_wall: bool,
    /// Atom type of `i` (0-based), upstream `sidata.itype`. Zero unless set by
    /// [`ContactKinematics::with_types`].
    pub itype: usize,
    /// Atom type of `j` (0-based), upstream `sidata.jtype`.
    pub jtype: usize,
    /// Contact-point rolling velocity `w_r` (upstream `sidata.wr1..3`):
    /// `(c_ri omega_i + c_rj omega_j) rinv` for a pair, `(c_r omega_i) rinv`
    /// for a wall. The wall branch of the rolling models reads it.
    pub wr: Vec3,
    /// Centre-to-centre distance for a pair, centre-to-surface distance for a
    /// wall (upstream `sidata.r`) `[m]`.
    pub r: f64,
    /// Radius of `i` (upstream `sidata.radi`) `[m]`.
    pub rad_i: f64,
    /// Radius of `j` (`sidata.radj`) `[m]`; zero for a wall contact.
    pub rad_j: f64,
    /// Mass of `i` (`sidata.mi`) `[kg]`.
    pub mass_i: f64,
    /// Mass of `j` (`sidata.mj`) `[kg]`; zero for a wall contact.
    pub mass_j: f64,
    /// `surfaceLiquidContent` of `i` (the capillary cohesion models' per-atom
    /// liquid volume fraction); zero unless set by [`ContactKinematics::with_liquid`].
    pub liquid_i: f64,
    /// `surfaceLiquidContent` of `j`.
    pub liquid_j: f64,
}

impl ContactKinematics {
    /// Resolve the kinematics of a particle–particle contact, or `None` if the
    /// pair does not overlap (or the centres coincide, leaving `ê_n` undefined).
    ///
    /// Translation of `SurfaceModel<SURFACE_DEFAULT>::surfacesIntersect` with
    /// `is_wall = false`.
    #[must_use]
    pub fn pair(i: &Particle, j: &Particle) -> Option<Self> {
        Self::pair_frozen(i, j, false, false)
    }

    /// As [`Self::pair`], with the `fix freeze` group flags of the two
    /// particles. Upstream (`pair_gran_base.h`, LIGGGHTS-PUBLIC `3d5c00f2`,
    /// `compute`): `meff = mi*mj/(mi+mj); if (mask[i] & freeze_group_bit)
    /// meff = mj; if (mask[j] & freeze_group_bit) meff = mi;` — a frozen
    /// particle does not reduce the effective mass (gh:#1478).
    #[must_use]
    pub fn pair_frozen(i: &Particle, j: &Particle, frozen_i: bool, frozen_j: bool) -> Option<Self> {
        Self::pair_radii(i, j, frozen_i, frozen_j, i.radius, j.radius, i.radius + j.radius)
    }

    /// [`Self::pair_frozen`] with upstream's three radii separated: the
    /// overlap test uses `radsum`, while the contact radii, `R*` and the
    /// recorded `rad_i`/`rad_j` use `radi`/`radj` (`sidata.radi`, `.radj`).
    /// They differ only under the multicontact model, where `pair_gran_base.h`
    /// sets `sidata.radi` once per particle (unexpanded) but `sidata.radj` and
    /// `radsum` per contact (expanded); that asymmetry is reproduced for parity.
    #[must_use]
    pub fn pair_radii(
        i: &Particle,
        j: &Particle,
        frozen_i: bool,
        frozen_j: bool,
        radi: f64,
        radj: f64,
        radsum: f64,
    ) -> Option<Self> {
        let delta = i.position.sub(j.position); // x_i - x_j
        let r = delta.norm();
        let delta_n = radsum - r;
        if delta_n <= 0.0 || r <= 0.0 {
            return None;
        }
        let rinv = 1.0 / r;
        let en = delta.scale(rinv);

        let vr = i.velocity.sub(j.velocity);
        let vn = vr.dot(en);
        let vt = vr.sub(en.scale(vn));

        let cri = radi - 0.5 * delta_n;
        let crj = radj - 0.5 * delta_n;
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
            r_eff: radi * radj / (radi + radj),
            m_eff: {
                let mut m = i.mass * j.mass / (i.mass + j.mass);
                if frozen_i {
                    m = j.mass;
                }
                if frozen_j {
                    m = i.mass;
                }
                m
            },
            omega_i: i.angular_velocity,
            omega_j: j.angular_velocity,
            is_wall: false,
            itype: 0,
            jtype: 0,
            wr,
            r,
            rad_i: radi,
            rad_j: radj,
            mass_i: i.mass,
            mass_j: j.mass,
            liquid_i: 0.0,
            liquid_j: 0.0,
        })
    }

    /// The same contact with atom types `itype` / `jtype` (0-based) attached,
    /// for a [`MaterialTable`] lookup.
    #[must_use]
    pub fn with_types(self, itype: usize, jtype: usize) -> Self {
        Self {
            itype,
            jtype,
            ..self
        }
    }

    /// The same contact with the two particles' `surfaceLiquidContent`
    /// attached, for the capillary cohesion models ([`crate::capillary`]).
    #[must_use]
    pub fn with_liquid(self, liquid_i: f64, liquid_j: f64) -> Self {
        Self {
            liquid_i,
            liquid_j,
            ..self
        }
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
        // Upstream: wr = cr * omega * rinv, i.e. (cr * omega) * rinv.
        let wr = i.angular_velocity.scale(cr).scale(rinv);
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
            omega_i: i.angular_velocity,
            omega_j: Vec3::zero(),
            is_wall: true,
            itype: 0,
            jtype: 0,
            wr,
            r: i.radius - delta_n,
            rad_i: i.radius,
            rad_j: 0.0,
            mass_i: i.mass,
            mass_j: 0.0,
            liquid_i: 0.0,
            liquid_j: 0.0,
        })
    }
}

impl ContactKinematics {
    /// Kinematics of a particle–mesh-facet contact, translated from
    /// `FixWallGran::post_force_mesh` + `FixWallGranBase::compute_force` +
    /// the wall branch of `SurfaceModel<SURFACE_DEFAULT>::surfacesIntersect`.
    ///
    /// `delta` is upstream's `sidata.delta` (`centre − closest point`, so its
    /// length is the centre-to-surface distance), `overlap` is `sidata.deltan`
    /// (`radius − distance > 0`). Operation order follows upstream:
    /// `r = radi − deltan`, `rinv = 1/r`, `en = delta·rinv`, `cr = radi −
    /// ½·deltan` from the original overlap, and the overlap handed to the force
    /// models is re-derived as `radsum − r` (`deltan = radsum − r`).
    /// gh:#1416.
    #[must_use]
    pub fn mesh_wall(i: &Particle, delta: Vec3, overlap: f64, wall_velocity: Vec3) -> Self {
        let radi = i.radius;
        let r = radi - overlap;
        let rinv = 1.0 / r;
        let en = Vec3::new(delta.x * rinv, delta.y * rinv, delta.z * rinv);

        let vr = Vec3::new(
            i.velocity.x - wall_velocity.x,
            i.velocity.y - wall_velocity.y,
            i.velocity.z - wall_velocity.z,
        );
        let vn = vr.x * en.x + vr.y * en.y + vr.z * en.z;
        let vn1 = vn * en.x;
        let vn2 = vn * en.y;
        let vn3 = vn * en.z;
        let vt = Vec3::new(vr.x - vn1, vr.y - vn2, vr.z - vn3);

        let delta_n = radi - r; // `sidata.radsum - sidata.r`
        let cr = radi - 0.5 * overlap;
        let wr1 = cr * i.angular_velocity.x * rinv;
        let wr2 = cr * i.angular_velocity.y * rinv;
        let wr3 = cr * i.angular_velocity.z * rinv;
        let vtr = Vec3::new(
            vt.x - (delta.z * wr2 - delta.y * wr3),
            vt.y - (delta.x * wr3 - delta.z * wr1),
            vt.z - (delta.y * wr1 - delta.x * wr2),
        );

        Self {
            en,
            delta_n,
            vn,
            vtr,
            cri: cr,
            crj: 0.0,
            r_eff: i.radius,
            m_eff: i.mass,
            omega_i: i.angular_velocity,
            omega_j: Vec3::zero(),
            is_wall: true,
            itype: 0,
            jtype: 0,
            wr: Vec3::new(wr1, wr2, wr3),
            r,
            rad_i: i.radius,
            rad_j: 0.0,
            mass_i: i.mass,
            mass_j: 0.0,
            liquid_i: 0.0,
            liquid_j: 0.0,
        }
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

/// `f64` slots of normal-model per-contact state a contact can carry.
pub const NORMAL_STATE: usize = 12;

/// The slot of a contact's auxiliary records holding the rolling spring.
const ROLL_SLOT: u8 = 4;

/// Per-contact state beyond the shear spring: the normal model's history
/// values (`deltaMax`, ...) and the rolling model's spring torque. Each is
/// kept in the [`ShearHistory`] store under an [`ContactKey::Aux`] key so it
/// shares the contact's life cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactAux {
    /// Normal-model history values.
    pub normal: [f64; NORMAL_STATE],
    /// Rolling-model history, `r_torque_old` `[N m]`.
    pub roll: Vec3,
}

impl Default for ContactAux {
    fn default() -> Self {
        Self {
            normal: [0.0; NORMAL_STATE],
            roll: Vec3::zero(),
        }
    }
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
    /// Upstream `NormalModel<HOOKE_HYSTERESIS>` (`hooke/hysteresis`,
    /// `normal_model_hooke_hysteresis.h`): the Hooke stiffness with a
    /// piecewise-linear **plastic** loading/unloading law (Walton-Braun) that
    /// remembers each contact's maximal overlap `deltaMax`. The per-contact
    /// `deltaMax` lives in the [`ShearHistory`] store under the contact's
    /// [`ContactKey::aux`] key, so it follows the contact's life cycle (reset
    /// to 0 when the contact is lost, upstream `surfacesClose`). Contacts are
    /// resolved serially (the parallel staging path carries no per-contact
    /// scalar). gh:#1410.
    HookeHysteresis {
        /// `E`, `nu`, `e`, `mu` of the single material.
        material: GranularMaterial,
        /// Characteristic impact velocity `v_char` `[m/s]` (as `hooke`).
        characteristic_velocity: f64,
        /// Upstream `tangential_damping` (default on).
        tangential_damping: bool,
        /// `coefficientMaxElasticStiffness`: `k2Max / kn` `[-]` (> 1).
        kn2k2_max: f64,
        /// `coefficientAdhesionStiffness`: `kc / kn` `[-]` (cohesion stiffness).
        kn2kc: f64,
        /// `coefficientPlasticityDepth`: `phi_f` `[-]`, the overlap fraction
        /// (of the diameter) at which loading reaches `k2Max`.
        phi_f: f64,
    },
    /// Upstream `NormalModel<LUDING>` (`luding`, `normal_model_luding.h`):
    /// Luding's piecewise-linear elasto-plastic adhesive spring with load- and
    /// history-dependent unloading stiffness `k2`, a cohesion branch that
    /// rewrites `deltaMax` and a constant pull-off force `f_adh`. The state is
    /// `deltaMax`, plus the `kc` and `f_adh` values the Luding tangential and
    /// rolling models read back. Properties are single-type-pair values.
    /// gh:#1410.
    Luding {
        /// `e` for the damping (property `coefficientRestitution`) and the
        /// friction `mu` of the material.
        material: GranularMaterial,
        /// Loading stiffness `k1` `[N/m]` (property `LoadingStiffness`).
        k_elastic: f64,
        /// `k2Max / k1` (property `UnloadingStiffness`, ratio `> 1`).
        kn2k1: f64,
        /// `kc / k1` (property `coefficientAdhesionStiffness`).
        kn2kc: f64,
        /// `phi_f` (property `coefficientPlasticityDepth`).
        phi_f: f64,
        /// Pull-off force `f_0` `[N]` (property `pullOffForce`).
        f_adh: f64,
        /// Upstream `tangential_damping` (default on).
        tangential_damping: bool,
        /// Upstream `limitForce` (**default on** for Luding): clamp a
        /// negative `F_n` to zero when `kc = 0` and `f_0 = 0`.
        limit_force: bool,
    },
    /// Upstream `NormalModel<THORNTON_NING>` (`thornton_ning`,
    /// `normal_model_thornton_ning.h`): the elastic-perfectly-plastic adhesive
    /// Hertzian contact of Thornton and Ning (1998). Loading follows the JKR
    /// elastic branch until the contact radius reaches the yield radius
    /// `a_Y = r_i * yield_ratio`, then the plastic branch; unloading and
    /// reloading follow elastic JKR curves of a modified radius; a contact
    /// detaches at `f = -(5/9) f_c`. Eleven history values per contact
    /// (virgin flag, `delta_old`, `delta_max`, `force_old`, `force_max`,
    /// adhesion flag, detaching delta/flag/force, yielding flag, and the
    /// `f_c` read by the Luding tangential/rolling models). No normal
    /// dashpot. **Upstream oddities kept for parity:** `gammat` has no
    /// leading minus sign (it is negative, so the tangential dashpot adds
    /// force) and `sidata.kn` is not set (`kn` is zero here). gh:#1410.
    ThorntonNing {
        /// `Y_eff`, `G_eff`, `beta_eff` come from this material.
        material: GranularMaterial,
        /// Surface energy `gamma_s` `[J/m^2]` (property `surfaceEnergy`).
        gamma_surf: f64,
        /// Yield ratio `a_Y / r_i` `[-]` (property `coefficientYieldRatio`).
        yield_ratio: f64,
    },
    /// Upstream `NormalModel<EDINBURGH>` (`edinburgh`,
    /// `normal_model_edinburgh.h`) and, with `stiffness = Some(K)`,
    /// `NormalModel<EDINBURGH_STIFFNESS>` (`edinburgh/stiffness`): the
    /// Edinburgh elasto-plastic adhesive (EEPA) contact model of Thakur et
    /// al. With `fix_kc = false` the adhesive stiffness `k_c` is recomputed
    /// every step from the surface energy and the plastic overlap
    /// (`calculate_k_adh`, with upstream's `goto temp_calc` iteration); with
    /// `fix_kc = true` it is `kn2kc * k_n`. State: `deltaMax`, `old_delta`,
    /// `kc`, `f_0`. gh:#1410.
    Edinburgh {
        /// `Y_eff`, `G_eff` (Edinburgh) and `e` (both) come from this material.
        material: GranularMaterial,
        /// `Some(K_elastic)` selects `edinburgh/stiffness` (`k_n = k_t = K`).
        stiffness: Option<f64>,
        /// `k2 / k1` (property `UnloadingStiffness`, `> 1`).
        kn2k1: f64,
        /// `kc / kn` for `fixKc` (property `coefficientAdhesionStiffness`).
        kn2kc: f64,
        /// Adhesion exponent `cex` (property `adhesionExponent`).
        cex: f64,
        /// Overlap exponent `dex` (property `overlapExponent`); 1 is linear.
        dex: f64,
        /// Pull-off force `f_0` `[N]` (property `pullOffForce`).
        f_adh: f64,
        /// Surface energy `gamma_s` `[J/m^2]` (property `surfaceEnergy`).
        gamma_surf: f64,
        /// Upstream `tangential_damping` (default on).
        tangential_damping: bool,
        /// Upstream `limitForce` (**default on**).
        limit_force: bool,
        /// Upstream `fixKc` (default off).
        fix_kc: bool,
    },
    /// Upstream `NormalModel<HERTZ_STIFFNESS>` (`hertz/stiffness`): stiffness
    /// and damping given directly, scaled by `sqrt(r_eff δ_n)`
    /// (`normal_model_hertz_stiffness.h`). The `kn`, `kt`, `gamman`, `gammat`
    /// of the single type pair; `material` supplies only the friction
    /// coefficient (and keeps the enum uniform). gh:#1410.
    HertzStiffness {
        /// Supplies `mu` (the stiffness model has no `E`, `nu`, `e` of its own).
        material: GranularMaterial,
        /// `kn` `[N/m^{3/2}]` (property `kn`).
        k_n: f64,
        /// `kt` `[N/m^{3/2}]` (property `kt`).
        k_t: f64,
        /// `gamman` `[1/(m^{1/2} s)]` (property `gamman`): `gamma_n sqrt(r δ) m_eff`.
        gamma_n: f64,
        /// `gammat` (property `gammat`).
        gamma_t: f64,
        /// Upstream `tangential_damping` (default on).
        tangential_damping: bool,
    },
    /// Upstream `NormalModel<HOOKE_STIFFNESS>` (`hooke/stiffness`): a linear
    /// spring and dashpot with coefficients given directly
    /// (`normal_model_hooke_stiffness.h`). gh:#1410.
    HookeStiffness {
        /// Supplies `mu` only.
        material: GranularMaterial,
        /// `kn` `[N/m]`.
        k_n: f64,
        /// `kt` `[N/m]`.
        k_t: f64,
        /// `gamman` `[1/s]` (relative: multiplied by `m_eff`) or, with
        /// `absolute_damping`, `gamman_abs` `[kg/s]`.
        gamma_n: f64,
        /// `gammat` / `gammat_abs`, as `gamma_n`.
        gamma_t: f64,
        /// Upstream `absolute_damping` (default off): use the damping
        /// coefficients as they are instead of multiplying by `m_eff`.
        absolute_damping: bool,
        /// Upstream `tangential_damping` (default on).
        tangential_damping: bool,
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

    /// Build a `hooke/hysteresis` model with upstream's default settings
    /// (`tangential_damping on`, not viscous).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `characteristic_velocity <= 0`,
    /// `kn2k2_max <= 1` (upstream divides by `k2Max - kn`), `kn2kc < 0` or
    /// `phi_f <= 0`.
    pub fn hooke_hysteresis(
        material: GranularMaterial,
        characteristic_velocity: f64,
        kn2k2_max: f64,
        kn2kc: f64,
        phi_f: f64,
    ) -> Result<Self, DemError> {
        if !(characteristic_velocity > 0.0 && characteristic_velocity.is_finite())
            || !(kn2k2_max > 1.0 && kn2k2_max.is_finite())
            || !(kn2kc >= 0.0 && kn2kc.is_finite())
            || !(phi_f > 0.0 && phi_f.is_finite())
        {
            return Err(DemError::InvalidInput(format!(
                "hooke/hysteresis needs v_char > 0, kn2k2Max > 1, kn2kc >= 0, phiF > 0; got \
                 {characteristic_velocity}, {kn2k2_max}, {kn2kc}, {phi_f}"
            )));
        }
        Ok(Self::HookeHysteresis {
            material,
            characteristic_velocity,
            tangential_damping: true,
            kn2k2_max,
            kn2kc,
            phi_f,
        })
    }

    /// Build a `luding` model (see [`GranularNormalModel::Luding`]) with
    /// upstream's defaults (`tangential_damping on`, `limitForce on`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless `k_elastic > 0`, `kn2k1 > 1`,
    /// `kn2kc >= 0`, `phi_f > 0`, `f_adh >= 0` and the material's
    /// restitution is below 1 (so `ln e` is finite and nonzero).
    pub fn luding(
        material: GranularMaterial,
        k_elastic: f64,
        kn2k1: f64,
        kn2kc: f64,
        phi_f: f64,
        f_adh: f64,
    ) -> Result<Self, DemError> {
        if !(k_elastic > 0.0 && kn2k1 > 1.0 && kn2kc >= 0.0 && phi_f > 0.0 && f_adh >= 0.0)
            || !k_elastic.is_finite()
            || !kn2k1.is_finite()
            || !kn2kc.is_finite()
            || !phi_f.is_finite()
            || !f_adh.is_finite()
        {
            return Err(DemError::InvalidInput(format!(
                "luding needs K > 0, kn2k1 > 1, kn2kc >= 0, phiF > 0, f_adh >= 0; got \
                 {k_elastic}, {kn2k1}, {kn2kc}, {phi_f}, {f_adh}"
            )));
        }
        Ok(Self::Luding {
            material,
            k_elastic,
            kn2k1,
            kn2kc,
            phi_f,
            f_adh,
            tangential_damping: true,
            limit_force: true,
        })
    }

    /// Build a `thornton_ning` model (see [`GranularNormalModel::ThorntonNing`]).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless `gamma_surf >= 0` and
    /// `yield_ratio > 0` are finite.
    pub fn thornton_ning(
        material: GranularMaterial,
        gamma_surf: f64,
        yield_ratio: f64,
    ) -> Result<Self, DemError> {
        if !(gamma_surf >= 0.0 && yield_ratio > 0.0)
            || !gamma_surf.is_finite()
            || !yield_ratio.is_finite()
        {
            return Err(DemError::InvalidInput(format!(
                "thornton_ning needs gamma_surf >= 0 and yield_ratio > 0, got {gamma_surf}, {yield_ratio}"
            )));
        }
        Ok(Self::ThorntonNing {
            material,
            gamma_surf,
            yield_ratio,
        })
    }

    /// Build an `edinburgh` (`stiffness = None`) or `edinburgh/stiffness`
    /// model with upstream's defaults (`tangential_damping on`, `limitForce on`,
    /// `fixKc off`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless `kn2k1 > 1`, `cex > 0`, `dex > 0`,
    /// `gamma_surf >= 0`, `f_adh >= 0`, `kn2kc >= 0` (and `K > 0`) are finite.
    #[allow(clippy::too_many_arguments)]
    pub fn edinburgh(
        material: GranularMaterial,
        stiffness: Option<f64>,
        kn2k1: f64,
        kn2kc: f64,
        cex: f64,
        dex: f64,
        f_adh: f64,
        gamma_surf: f64,
    ) -> Result<Self, DemError> {
        let ok = kn2k1 > 1.0
            && cex > 0.0
            && dex > 0.0
            && gamma_surf >= 0.0
            && f_adh >= 0.0
            && kn2kc >= 0.0
            && stiffness.is_none_or(|k| k > 0.0)
            && [kn2k1, kn2kc, cex, dex, f_adh, gamma_surf]
                .iter()
                .all(|v| v.is_finite());
        if !ok {
            return Err(DemError::InvalidInput(format!(
                "edinburgh needs kn2k1 > 1, cex > 0, dex > 0 and finite non-negative \
                 adhesion data; got {kn2k1}, {kn2kc}, {cex}, {dex}, {f_adh}, {gamma_surf}"
            )));
        }
        Ok(Self::Edinburgh {
            material,
            stiffness,
            kn2k1,
            kn2kc,
            cex,
            dex,
            f_adh,
            gamma_surf,
            tangential_damping: true,
            limit_force: true,
            fix_kc: false,
        })
    }

    /// Set `fixKc` of an [`GranularNormalModel::Edinburgh`] model (other
    /// variants are returned unchanged).
    #[must_use]
    pub fn with_fix_kc(mut self, on: bool) -> Self {
        if let Self::Edinburgh { fix_kc, .. } = &mut self {
            *fix_kc = on;
        }
        self
    }

    /// The adhesive stiffness `kc` and pull-off force `f_0` this model stored
    /// for the Luding tangential and rolling models (upstream's `kc_offset`
    /// and `fo_offset` history values).
    #[must_use]
    pub fn kc_fo(&self, state: &[f64; NORMAL_STATE]) -> (f64, f64) {
        match self {
            Self::ThorntonNing { .. } => (state[10], state[11]),
            Self::Edinburgh { .. } => (state[2], state[3]),
            _ => (state[1], state[2]),
        }
    }

    /// Whether this model keeps a per-contact scalar (`deltaMax`) besides the
    /// shear spring; such a model is resolved serially.
    #[must_use]
    pub fn has_contact_state(&self) -> bool {
        self.state_len() > 0
    }

    /// Number of `f64` of per-contact state this model keeps (upstream's
    /// `add_history_value` count): 1 for hysteresis (`deltaMax`), else 0.
    #[must_use]
    pub fn state_len(&self) -> usize {
        match self {
            Self::HookeHysteresis { .. } => 1,
            Self::Luding { .. } => 3,
            Self::ThorntonNing { .. } => 12,
            Self::Edinburgh { .. } => 4,
            _ => 0,
        }
    }

    /// Evaluate with the per-contact state `state` (read and updated in place;
    /// `update` is upstream's `computeflag && shearupdate`). Models without
    /// state ignore it.
    #[must_use]
    pub fn evaluate_stateful(
        &self,
        k: &ContactKinematics,
        props: &PairProps,
        state: &mut [f64; NORMAL_STATE],
        update: bool,
    ) -> NormalOutcome {
        match self {
            Self::HookeHysteresis { .. } => {
                let (out, dmax) = self.evaluate_hysteretic(k, props, state[0], update);
                state[0] = dmax;
                out
            }
            Self::Luding { .. } => {
                let (out, st) = self.evaluate_luding(k, props, [state[0], state[1], state[2]]);
                state[..3].copy_from_slice(&st);
                out
            }
            Self::ThorntonNing { .. } => self.evaluate_thornton_ning(k, props, state),
            Self::Edinburgh { .. } => self.evaluate_edinburgh(k, props, state),
            _ => self.evaluate_with(k, props),
        }
    }

    /// Build a `hertz/stiffness` model with upstream's default settings
    /// (`tangential_damping on`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a coefficient is not finite and `>= 0`, or
    /// `k_n` is not `> 0`.
    pub fn hertz_stiffness(
        material: GranularMaterial,
        k_n: f64,
        k_t: f64,
        gamma_n: f64,
        gamma_t: f64,
    ) -> Result<Self, DemError> {
        check_stiffness(k_n, k_t, gamma_n, gamma_t)?;
        Ok(Self::HertzStiffness {
            material,
            k_n,
            k_t,
            gamma_n,
            gamma_t,
            tangential_damping: true,
        })
    }

    /// Build a `hooke/stiffness` model with upstream's default settings
    /// (`absolute_damping off`, `tangential_damping on`).
    ///
    /// # Errors
    ///
    /// As [`GranularNormalModel::hertz_stiffness`].
    pub fn hooke_stiffness(
        material: GranularMaterial,
        k_n: f64,
        k_t: f64,
        gamma_n: f64,
        gamma_t: f64,
    ) -> Result<Self, DemError> {
        check_stiffness(k_n, k_t, gamma_n, gamma_t)?;
        Ok(Self::HookeStiffness {
            material,
            k_n,
            k_t,
            gamma_n,
            gamma_t,
            absolute_damping: false,
            tangential_damping: true,
        })
    }

    /// Set upstream's `tangential_damping` switch (default on) on any variant.
    #[must_use]
    pub fn with_tangential_damping(mut self, on: bool) -> Self {
        match &mut self {
            Self::Hertz {
                tangential_damping, ..
            }
            | Self::Hooke {
                tangential_damping, ..
            }
            | Self::HertzStiffness {
                tangential_damping, ..
            }
            | Self::HookeStiffness {
                tangential_damping, ..
            }
            | Self::HookeHysteresis {
                tangential_damping, ..
            }
            | Self::Luding {
                tangential_damping, ..
            }
            | Self::Edinburgh {
                tangential_damping, ..
            } => *tangential_damping = on,
            Self::ThorntonNing { .. } => {}
        }
        self
    }

    /// Set `absolute_damping` of a [`GranularNormalModel::HookeStiffness`]
    /// (the other variants have no such switch and are returned unchanged).
    #[must_use]
    pub fn with_absolute_damping(mut self, on: bool) -> Self {
        if let Self::HookeStiffness {
            absolute_damping, ..
        } = &mut self
        {
            *absolute_damping = on;
        }
        self
    }

    /// The material this model carries.
    #[must_use]
    pub fn material(&self) -> GranularMaterial {
        match self {
            Self::Hertz { material, .. }
            | Self::Hooke { material, .. }
            | Self::HertzStiffness { material, .. }
            | Self::HookeStiffness { material, .. }
            | Self::HookeHysteresis { material, .. }
            | Self::Luding { material, .. }
            | Self::ThorntonNing { material, .. }
            | Self::Edinburgh { material, .. } => *material,
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
        self.evaluate_with(k, &PairProps::from_material(&self.material()))
    }

    /// As [`GranularNormalModel::evaluate`], with the type-pair coefficients
    /// `props` supplied by the caller (from a [`MaterialTable`]) instead of
    /// the model's own single material.
    #[must_use]
    pub fn evaluate_with(&self, k: &ContactKinematics, props: &PairProps) -> NormalOutcome {
        match *self {
            Self::Hertz {
                tangential_damping, ..
            } => {
                let sqrtval = (k.r_eff * k.delta_n).sqrt();
                let y_eff = props.y_eff;
                let g_eff = props.g_eff;
                let beta = props.beta_eff;

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
                characteristic_velocity,
                tangential_damping,
                kt_to_kn,
                ..
            } => {
                let sqrtval = k.r_eff.sqrt();
                let y_eff = props.y_eff;
                let char_vel = characteristic_velocity;

                let kn = 16.0 / 15.0
                    * sqrtval
                    * y_eff
                    * (15.0 * k.m_eff * char_vel * char_vel / (16.0 * sqrtval * y_eff)).r_powf(0.2);
                let kt = if kt_to_kn { kn * 0.285_714_286 } else { kn };

                let ln_e = props.ln_e;
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
            Self::HookeHysteresis { .. } => self.evaluate_hysteretic(k, props, 0.0, false).0,
            Self::Luding { .. } => self.evaluate_luding(k, props, [0.0; 3]).0,
            Self::ThorntonNing { .. } => {
                self.evaluate_thornton_ning(k, props, &mut [0.0; NORMAL_STATE])
            }
            Self::Edinburgh { .. } => self.evaluate_edinburgh(k, props, &mut [0.0; NORMAL_STATE]),
            Self::HertzStiffness {
                k_n,
                k_t,
                gamma_n,
                gamma_t,
                tangential_damping,
                ..
            } => {
                // normal_model_hertz_stiffness.h, surfacesIntersect
                let polyhertz = (k.r_eff * k.delta_n).sqrt();
                let kn = polyhertz * k_n;
                let kt = polyhertz * k_t;
                let gamman = polyhertz * k.m_eff * gamma_n;
                let gammat = if tangential_damping {
                    polyhertz * k.m_eff * gamma_t
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
            Self::HookeStiffness {
                k_n,
                k_t,
                gamma_n,
                gamma_t,
                absolute_damping,
                tangential_damping,
                ..
            } => {
                // normal_model_hooke_stiffness.h, surfacesIntersect
                let (gamman, mut gammat) = if absolute_damping {
                    (gamma_n, gamma_t)
                } else {
                    (k.m_eff * gamma_n, k.m_eff * gamma_t)
                };
                if !tangential_damping {
                    gammat = 0.0;
                }
                NormalOutcome {
                    fn_scalar: -gamman * k.vn + k_n * k.delta_n,
                    kn: k_n,
                    kt: k_t,
                    gamman,
                    gammat,
                }
            }
        }
    }
}

/// `calculate_fl` of `normal_model_thornton_ning.h`.
fn tn_fl(force_old: f64, fc: f64, adhesion_flag: i32) -> f64 {
    if adhesion_flag == 1 {
        force_old + 2.0 * (fc - (fc * (force_old + fc)).sqrt())
    } else {
        force_old + 2.0 * (fc + (fc * (force_old + fc)).sqrt())
    }
}

/// `calculate_elastic_force_differential`.
fn tn_elastic_df(a: f64, e: f64, fl: f64, fc: f64) -> f64 {
    if fc != 0.0 {
        let up = 3.0 * fl.sqrt() - 3.0 * fc.sqrt();
        let down = 3.0 * fl.sqrt() - fc.sqrt();
        2.0 * e * a * up / down
    } else {
        2.0 * e * a
    }
}

/// `calculate_plastic_force_differential`.
fn tn_plastic_df(a_yield: f64, e: f64, fl: f64, fc: f64, yield_stress: f64, r: f64) -> f64 {
    if fc != 0.0 {
        let up = 3.0 * std::f64::consts::PI * r * yield_stress * fl.sqrt()
            - 2.0 * a_yield * e * fc.sqrt();
        let down = 3.0 * fl.sqrt() - fc.sqrt();
        up / down
    } else {
        2.0 * e * a_yield
    }
}

/// `calculate_a`.
fn tn_a(fl: f64, e: f64, r: f64, delta: f64, gamma_s: f64) -> f64 {
    if gamma_s == 0.0 {
        (r * delta).sqrt()
    } else {
        ((3.0 * r * fl) / (4.0 * e)).r_powf(1.0 / 3.0)
    }
}

/// `branch_2_force_calc`: `(force_old + df, df)`.
#[allow(clippy::too_many_arguments)]
fn tn_branch_2(
    force_old: f64,
    fc: f64,
    adhesion_flag: i32,
    e: f64,
    rp: f64,
    delta: f64,
    dn: f64,
    gamma_s: f64,
) -> (f64, f64) {
    let fl_r = tn_fl(force_old, fc, adhesion_flag);
    let a = tn_a(fl_r, e, rp, delta, gamma_s);
    let df = tn_elastic_df(a, e, fl_r, fc) * dn;
    (force_old + df, df)
}

/// `calculate_k_adh` of `normal_model_edinburgh.h`.
#[allow(clippy::too_many_arguments)]
fn edi_k_adh(
    d: f64,
    risq: f64,
    rjsq: f64,
    g_surf: f64,
    f_min_lim: f64,
    deltan_pe_max: f64,
    deltan_p_max: f64,
    k2: f64,
    dex_i: f64,
    cex: f64,
) -> f64 {
    let dsq = d * d;
    let a =
        (1.0 / (2.0 * d)) * (4.0 * dsq * risq - ((dsq - rjsq + risq) * (dsq - rjsq + risq))).sqrt();
    let mut f_min = 1.5 * std::f64::consts::PI * g_surf * a;
    let delta_min;
    if f_min > f_min_lim {
        f_min = f_min_lim;
        delta_min = 0.5 * deltan_p_max;
    } else {
        delta_min = ((-f_min + k2 * deltan_pe_max) / k2).r_powf(dex_i);
    }
    f_min / delta_min.r_powf(cex)
}

impl GranularNormalModel {
    /// `NormalModel<EDINBURGH>` / `NormalModel<EDINBURGH_STIFFNESS>`
    /// `::surfacesIntersect` (LIGGGHTS-PUBLIC `3d5c00f2`), including the
    /// `goto temp_calc` loop (here capped at 10 000 passes, where upstream
    /// has no cap) and the upstream shadowing of `k_adh` under `fixKc` (the
    /// `limitForce` test then sees `k_adh == 0`). `state = [deltaMax,
    /// old_delta, kc, f_0]`.
    #[must_use]
    fn evaluate_edinburgh(
        &self,
        k: &ContactKinematics,
        props: &PairProps,
        state: &mut [f64; NORMAL_STATE],
    ) -> NormalOutcome {
        let Self::Edinburgh {
            stiffness,
            kn2k1,
            kn2kc,
            cex,
            dex,
            f_adh,
            gamma_surf,
            tangential_damping,
            limit_force,
            fix_kc,
            ..
        } = *self
        else {
            return self.evaluate_with(k, props);
        };
        use std::f64::consts::PI;
        let deltan = k.delta_n;
        let ri = k.rad_i;
        let rj = k.rad_j;
        let reff = k.r_eff;
        let meff = k.m_eff;
        let f_0 = f_adh;
        let (kn, kt) = if let Some(k_el) = stiffness {
            (k_el, k_el)
        } else {
            let sqrtval = (reff * k.delta_n).sqrt();
            let sn = 2.0 * props.y_eff * sqrtval;
            let st = 8.0 * props.g_eff * sqrtval;
            if dex == 1.0 {
                (2.0 * props.y_eff * reff, sn)
            } else {
                ((4.0 / 3.0) * props.y_eff * reff.sqrt(), st)
            }
        };
        let dex_i = 1.0 / dex;
        let k1 = kn;
        let k2 = kn * kn2k1;
        let ln_e = props.ln_e;
        let gamman = (4.0 * meff * kn / (1.0 + (PI / ln_e) * (PI / ln_e))).sqrt();
        let mut gammat = (4.0 * meff * kn / (1.0 + (PI / ln_e) * (PI / ln_e))).sqrt();
        if !tangential_damping {
            gammat = 0.0;
        }
        let mut k_adh_outer = 0.0;
        let fn_contact;
        if !fix_kc {
            let lambda = (1.0 - k1 / k2).r_powf(dex_i);
            let deltan_p = lambda * deltan;
            let deltan_e = deltan.r_powf(dex);
            let deltan_ce = deltan.r_powf(cex);
            // calculate_deltan_p_max, count_flag = 0
            if deltan_p > state[0] {
                state[0] = deltan_p;
            }
            let mut deltan_p_max = state[0];
            let g_surf = gamma_surf;
            let r_sum = ri + rj;
            let d1 = if k.is_wall { ri } else { r_sum - deltan };
            let d2 = if k.is_wall { ri } else { r_sum - deltan_p_max };
            let risq = ri * ri;
            let rjsq = rj * rj;
            let mut passes = 0;
            loop {
                let deltan_pe_max = deltan_p_max.r_powf(dex);
                let f_min_lim = (k2 * deltan_pe_max) * 0.5;
                let f_tmp = k2 * (deltan_e - deltan_pe_max);
                if f_tmp >= k1 * deltan_e {
                    fn_contact = k1 * deltan_e;
                    break;
                }
                // whichd: the loading test failed, so `d2`
                let d = d2;
                let _ = d1;
                k_adh_outer = edi_k_adh(
                    d,
                    risq,
                    rjsq,
                    g_surf,
                    f_min_lim,
                    deltan_pe_max,
                    deltan_p_max,
                    k2,
                    dex_i,
                    cex,
                );
                state[2] = k_adh_outer;
                if f_tmp > (-k_adh_outer * deltan_ce) {
                    fn_contact = f_tmp;
                    break;
                }
                passes += 1;
                if deltan > state[1] && passes < 10_000 {
                    // calculate_deltan_p_max, count_flag = 1
                    deltan_p_max = (state[1].r_powf(dex)
                        + ((k_adh_outer / k2) * state[1].r_powf(cex)))
                    .r_powf(dex_i);
                    state[0] = deltan_p_max;
                    continue;
                }
                fn_contact = -k_adh_outer * deltan_ce;
                break;
            }
            state[1] = deltan;
        } else {
            let k_adh = kn2kc * kn;
            state[2] = k_adh;
            let delta_max;
            if deltan > state[0] {
                state[0] = deltan;
                delta_max = deltan;
            } else {
                delta_max = state[0];
            }
            let deltan_e = deltan.r_powf(dex);
            let deltan_ce = deltan.r_powf(cex);
            let delta_max_e = delta_max.r_powf(dex);
            let f_tmp = k2 * (deltan_e - delta_max_e) + k1 * delta_max_e;
            if f_tmp >= k1 * deltan_e {
                fn_contact = k1 * deltan_e;
            } else if f_tmp > (-k_adh * deltan_ce) {
                fn_contact = f_tmp;
            } else {
                fn_contact = -k_adh * deltan_ce;
                state[0] = ((k2 + k_adh) / (k2 - k1)) * deltan;
            }
        }
        let fn_damping = -gamman * k.vn;
        let mut fn_total = fn_damping + fn_contact + f_0;
        state[3] = f_0;
        if limit_force && fn_total < 0.0 && k_adh_outer == 0.0 && f_0 == 0.0 {
            fn_total = 0.0;
        }
        NormalOutcome {
            fn_scalar: fn_total,
            kn,
            kt,
            gamman,
            gammat,
        }
    }

    /// `NormalModel<THORNTON_NING>::surfacesIntersect`
    /// (`normal_model_thornton_ning.h`, LIGGGHTS-PUBLIC `3d5c00f2`), the
    /// `goto UNLOADING_RELOADING` included. `state` holds upstream's eleven
    /// history values (`[0..10)`), `kc` (`[10]`) and `fo` (`[11]`). Like
    /// upstream it writes the state on every call (also at set-up).
    ///
    /// # Panics
    ///
    /// If the yield stress is not positive, where upstream aborts with
    /// `error->all("Invalid yield stress, please check surface energy and
    /// yield ratio!")`.
    #[must_use]
    fn evaluate_thornton_ning(
        &self,
        k: &ContactKinematics,
        props: &PairProps,
        state: &mut [f64; NORMAL_STATE],
    ) -> NormalOutcome {
        let Self::ThorntonNing {
            gamma_surf,
            yield_ratio,
            ..
        } = *self
        else {
            return self.evaluate_with(k, props);
        };
        use std::f64::consts::PI;
        let ri = k.rad_i;
        let mut reff = k.r_eff;
        let meff = k.m_eff;
        let e_eff = props.y_eff;
        let gamma_s = gamma_surf;

        let mut virgin_loading_flag = state[0] as i32;
        let mut delta_old = state[1];
        let mut delta_max = state[2];
        let mut force_old = state[3];
        let mut force_max = state[4];
        let mut adhesion_flag = state[5] as i32;
        let mut detaching_delta = state[6];
        let mut detaching_flag = state[7] as i32;
        let mut detaching_force = state[8];
        let mut yielding_flag = state[9] as i32;

        let mut fc = gamma_s * reff * PI * (3.0 / 2.0);
        let mut fo = fc;
        let delta = k.delta_n;
        let a_yield = ri * yield_ratio;
        let yield_stress = ((2.0 * e_eff * a_yield) / (PI * reff))
            - (2.0 * gamma_s * e_eff / (PI * a_yield)).sqrt();
        let limit_yield_stress =
            (2.0 * e_eff * e_eff * gamma_s / (PI * PI * reff)).r_powf(1.0 / 3.0);
        let plastic_from_start = yield_stress < limit_yield_stress;
        assert!(
            yield_stress > 0.0,
            "thornton_ning: invalid yield stress, please check surface energy and yield ratio!"
        );

        let sqrtval = (reff * k.delta_n).sqrt();
        let k_t = 8.0 * props.g_eff * sqrtval;
        let gammat = 2.0 * SQRT_FIVE_OVER_SIX * props.beta_eff * (k_t * meff).sqrt();

        if virgin_loading_flag != 1 {
            force_old = -(8.0 / 9.0) * fc;
            delta_old = 0.0;
            virgin_loading_flag = 1;
            force_max = force_old;
            delta_max = 0.0;
            adhesion_flag = 0;
            detaching_delta = 0.0;
            detaching_flag = 0;
            detaching_force = -(5.0 / 9.0) * fc;
            yielding_flag = 0;
        }

        let f;
        let mut run_unloading_reloading = true;
        let mut f_local = 0.0;
        if detaching_flag == 1 {
            if delta >= detaching_delta {
                detaching_flag = 0;
                delta_old = detaching_delta;
                force_old = detaching_force;
            } else {
                f_local = 0.0;
                run_unloading_reloading = false;
            }
        }
        if run_unloading_reloading {
            let dn = delta - delta_old;
            let mut df;
            let mut fv;
            if delta >= delta_max {
                delta_max = delta;
                let fl = tn_fl(force_old, fc, 0);
                let a = tn_a(fl, e_eff, reff, delta, gamma_s);
                if yielding_flag == 0 && a >= a_yield {
                    yielding_flag = 1;
                }
                if plastic_from_start || yielding_flag == 1 {
                    df = tn_plastic_df(a_yield, e_eff, fl, fc, yield_stress, reff) * dn;
                    fv = force_old + df;
                } else {
                    (fv, df) = tn_branch_2(
                        force_old,
                        fc,
                        adhesion_flag,
                        e_eff,
                        reff,
                        delta,
                        dn,
                        gamma_s,
                    );
                }
            } else if yielding_flag == 0 {
                (fv, df) = tn_branch_2(
                    force_old,
                    fc,
                    adhesion_flag,
                    e_eff,
                    reff,
                    delta,
                    dn,
                    gamma_s,
                );
            } else {
                let fl_max = tn_fl(force_max, fc, 0);
                reff = reff * fl_max / (force_max + (4.0 * fc * fl_max).sqrt());
                fc = (3.0 / 2.0) * PI * gamma_s * reff;
                fo = -fc;
                (fv, df) = tn_branch_2(
                    force_old,
                    fc,
                    adhesion_flag,
                    e_eff,
                    reff,
                    delta,
                    dn,
                    gamma_s,
                );
            }
            if gamma_s == 0.0 && fv < 0.0 {
                fv = 0.0;
                detaching_flag = 1;
                detaching_delta = delta;
                detaching_force = 0.0;
            }
            if fv < -fc {
                adhesion_flag = 1 - adhesion_flag;
                (fv, df) = tn_branch_2(
                    force_old,
                    fc,
                    adhesion_flag,
                    e_eff,
                    reff,
                    delta,
                    dn,
                    gamma_s,
                );
            }
            if adhesion_flag == 1 && fv + 3.0 * df >= -(5.0 / 9.0) * fc {
                fv = 0.0;
                detaching_flag = 1;
                detaching_delta = delta_old;
                detaching_force = force_old;
            }
            f_local = fv;
        }
        f = f_local;

        let force_max_new = if f > force_max { f } else { force_max };
        state[0] = virgin_loading_flag as f64;
        state[1] = delta;
        state[2] = delta_max;
        state[3] = f;
        state[4] = force_max_new;
        state[5] = adhesion_flag as f64;
        state[6] = detaching_delta;
        state[7] = detaching_flag as f64;
        state[8] = detaching_force;
        state[9] = yielding_flag as f64;
        state[10] = 0.0; // kc
        state[11] = fo;
        NormalOutcome {
            fn_scalar: f,
            kn: 0.0,
            kt: k_t,
            gamman: 0.0,
            gammat,
        }
    }

    /// `NormalModel<LUDING>::surfacesIntersect` (`normal_model_luding.h`,
    /// LIGGGHTS-PUBLIC `3d5c00f2`). `state = [deltaMax, kc, f_adh]` in and out
    /// (`deltaMax` is updated unconditionally, as upstream does).
    #[must_use]
    fn evaluate_luding(
        &self,
        k: &ContactKinematics,
        props: &PairProps,
        state: [f64; 3],
    ) -> (NormalOutcome, [f64; 3]) {
        let Self::Luding {
            k_elastic,
            kn2k1,
            kn2kc,
            phi_f,
            f_adh,
            tangential_damping,
            limit_force,
            ..
        } = *self
        else {
            return (self.evaluate_with(k, props), state);
        };
        let deltan = k.delta_n;
        let reff = k.r_eff;
        let meff = k.m_eff;
        let ln_e = props.ln_e;
        // `kn /= force->nktv2p` with nktv2p = 1 in SI
        let kn = k_elastic;
        let kt = kn;
        let k1 = kn;
        let k2_max = kn * kn2k1;
        let kc = kn2kc * kn;
        let f_0 = f_adh;
        let gamman = (4.0 * meff * kn
            / (1.0 + (std::f64::consts::PI / ln_e) * (std::f64::consts::PI / ln_e)))
            .sqrt();
        let mut gammat = (4.0 * meff * kn
            / (1.0 + (std::f64::consts::PI / ln_e) * (std::f64::consts::PI / ln_e)))
            .sqrt();
        if !tangential_damping {
            gammat = 0.0;
        }
        let mut history = state[0];
        let delta_max;
        if deltan > history {
            history = deltan;
            delta_max = deltan;
        } else {
            delta_max = history;
        }
        let delta_max_lim = (k2_max / (k2_max - k1)) * phi_f * 2.0 * reff;
        let f_hys;
        if delta_max >= delta_max_lim {
            let k2 = k2_max;
            let f_tmp = k2 * (deltan - delta_max_lim) + k1 * delta_max_lim;
            if f_tmp >= -kc * deltan {
                f_hys = f_tmp;
            } else {
                f_hys = -kc * deltan;
                history = ((k2 + kc) / (k2 - k1)) * deltan;
            }
        } else {
            let k2 = k1 + (k2_max - k1) * delta_max / delta_max_lim;
            let f_tmp = k2 * (deltan - delta_max) + k1 * delta_max;
            if f_tmp >= k1 * deltan {
                f_hys = k1 * deltan;
            } else if f_tmp > -kc * deltan {
                f_hys = f_tmp;
            } else {
                f_hys = -kc * deltan;
                history = ((k2 + kc) / (k2 - k1)) * deltan;
            }
        }
        let fn_damping = -gamman * k.vn;
        let mut fn_total = f_hys + fn_damping + f_0;
        if limit_force && fn_total < 0.0 && kc == 0.0 && f_0 == 0.0 {
            fn_total = 0.0;
        }
        (
            NormalOutcome {
                fn_scalar: fn_total,
                kn,
                kt,
                gamman,
                gammat,
            },
            [history, kc, f_0],
        )
    }

    /// `NormalModel<HOOKE_HYSTERESIS>::surfacesIntersect`: the outcome and the
    /// contact's new `deltaMax`. `delta_max` is the stored value (0 for a new
    /// contact); `update_history` is upstream's `computeflag && shearupdate`
    /// (gates only the cohesion-branch rewrite of `deltaMax`; the plain
    /// `deltan > deltaMax` update is unconditional upstream). Other variants
    /// return their outcome and the stored value unchanged.
    #[must_use]
    pub fn evaluate_hysteretic(
        &self,
        k: &ContactKinematics,
        props: &PairProps,
        delta_max: f64,
        update_history: bool,
    ) -> (NormalOutcome, f64) {
        let Self::HookeHysteresis {
            characteristic_velocity,
            tangential_damping,
            kn2k2_max,
            kn2kc,
            phi_f,
            ..
        } = *self
        else {
            return (self.evaluate_with(k, props), delta_max);
        };
        let reff = k.r_eff;
        let meff = k.m_eff;
        let y_eff = props.y_eff;
        let char_vel = characteristic_velocity;
        let ln_e = props.ln_e;
        let deltan = k.delta_n;

        let sqrtval = reff.sqrt();
        let kn = 16.0 / 15.0
            * sqrtval
            * y_eff
            * (15.0 * meff * char_vel * char_vel / (16.0 * sqrtval * y_eff)).r_powf(0.2);
        let kt = kn;
        let gamman = (4.0 * meff * kn
            / (1.0 + (std::f64::consts::PI / ln_e) * (std::f64::consts::PI / ln_e)))
            .sqrt();
        let gammat = if tangential_damping { gamman } else { 0.0 };

        let k2_max = kn * kn2k2_max;
        let kc = kn * kn2kc;

        let mut history = delta_max;
        if deltan > history {
            history = deltan;
        }
        let delta_max = history;

        let delta_max_lim = (k2_max / (k2_max - kn)) * phi_f * 2.0 * reff;
        let f_hys;
        if delta_max >= delta_max_lim {
            // big overlap ... no kn at all
            let f_tmp = k2_max * (deltan - delta_max_lim) + kn * delta_max_lim;
            if f_tmp >= -kc * deltan {
                f_hys = f_tmp;
            } else {
                f_hys = -kc * deltan;
                let new_delta_max = 0.5
                    * (deltan
                        + (deltan * deltan
                            + 4.0 * ((kn + kc) * deltan * delta_max_lim / (k2_max - kn)))
                            .sqrt());
                if update_history {
                    history = new_delta_max;
                }
            }
        } else {
            let k2 = kn + (k2_max - kn) * delta_max / delta_max_lim;
            let f_tmp = k2 * (deltan - delta_max) + kn * delta_max;
            if f_tmp >= kn * deltan {
                f_hys = kn * deltan; // loading part (kn)
            } else if f_tmp > -kc * deltan {
                f_hys = f_tmp; // un-/reloading part (k2)
            } else {
                f_hys = -kc * deltan; // cohesion part
                let new_delta_max = 0.5
                    * (deltan
                        + (deltan * deltan
                            + 4.0 * ((kn + kc) * deltan * delta_max_lim / (k2_max - kn)))
                            .sqrt());
                if update_history {
                    history = new_delta_max;
                }
            }
        }

        let fn_damping = -gamman * k.vn;
        let fn_total = f_hys + fn_damping;
        (
            NormalOutcome {
                fn_scalar: fn_total,
                kn,
                kt,
                gamman,
                gammat,
            },
            history,
        )
    }
}

/// Validate the four coefficients of a stiffness model.
fn check_stiffness(k_n: f64, k_t: f64, gamma_n: f64, gamma_t: f64) -> Result<(), DemError> {
    let ok = |v: f64| v.is_finite() && v >= 0.0;
    if !(k_n > 0.0 && k_n.is_finite() && ok(k_t) && ok(gamma_n) && ok(gamma_t)) {
        return Err(DemError::InvalidInput(format!(
            "stiffness model needs finite k_n > 0 and k_t, gamma_n, gamma_t >= 0, \
             got {k_n}, {k_t}, {gamma_n}, {gamma_t}"
        )));
    }
    Ok(())
}

/// Closed set of tangential contact models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TangentialModel {
    /// Upstream `TangentialModel<TANGENTIAL_HISTORY>` — a Mindlin shear spring
    /// with an accumulated tangential displacement, Coulomb-rescaled on slip.
    /// **This is the model a packed bed needs.**
    History,
    /// Upstream `TangentialModel<TANGENTIAL_NO_HISTORY>` — Coulomb-capped
    /// dashpot only, no spring. Equivalent to what [`crate::contact`] does.
    NoHistory,
    /// Upstream `TangentialModel<TANGENTIAL_LUDING>` (`tan_luding`,
    /// `tangential_model_luding_tn.h`): Luding's tangential model, used with
    /// `model luding`. A history spring of stiffness `k_t * kT2kcMax`, with a
    /// static friction limit and a lower dynamic one,
    /// `mu_s |F_n + k_c delta_n - f_adh|` and `mu_d |...|`, so the adhesive
    /// contact stiffness `k_c` and pull-off force `f_adh` (stored by the normal
    /// model) widen the friction cone. gh:#1410.
    LudingTn {
        /// `coeffMu` (property `FluidViscosity`): `mu_d / mu_s` `[-]`.
        mu_dynamic_ratio: f64,
        /// `coeffFricVisc` (property `FrictionViscosity`): multiplies the
        /// normal model's `gamma_t` `[-]`.
        fric_visc: f64,
        /// `kT2kcMax` (property `coeffFrictionStiffness`): `k_t` multiplier.
        kt_ratio: f64,
    },
}

impl TangentialModel {
    /// `tangential_model tan_luding` (see [`TangentialModel::LudingTn`]).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless all three coefficients are finite
    /// and `> 0` (upstream: `coeffMu > 0`, `coeffFricVisc > 0`).
    pub fn luding_tn(
        mu_dynamic_ratio: f64,
        fric_visc: f64,
        kt_ratio: f64,
    ) -> Result<Self, DemError> {
        for v in [mu_dynamic_ratio, fric_visc, kt_ratio] {
            if !(v > 0.0) || !v.is_finite() {
                return Err(DemError::InvalidInput(format!(
                    "tan_luding coefficients must be finite and > 0, got {mu_dynamic_ratio}, \
                     {fric_visc}, {kt_ratio}"
                )));
            }
        }
        Ok(Self::LudingTn {
            mu_dynamic_ratio,
            fric_visc,
            kt_ratio,
        })
    }
}

/// Key identifying one persistent contact in the [`ShearHistory`] store.
///
/// Particle–particle pairs are keyed by their **ordered** pair `(min, max)` so
/// the entry is found regardless of which way round the pair is visited.
/// Particle–wall contacts are keyed by the particle and a caller-chosen wall id.
///
/// # The identifier is a STABLE TAG, not a position in the particle array
///
/// This matters, and getting it wrong is silent. Upstream stores a contact's
/// partner as `partner_[i][m] = tag[j]` — LAMMPS' global **atom tag** — in
/// `fix_contact_history.cpp:393`, not the local index. That is precisely what
/// lets LAMMPS delete an atom by copying the last atom into the hole without
/// corrupting anybody's shear history.
///
/// [`GranularSystem`](crate::granular_system::GranularSystem) follows upstream
/// and passes its own per-particle tags here. **Do not pass array indices from
/// a system whose particle set can change**: after a removal every index
/// shifts, and each stored tangential spring silently re-attaches to a
/// different pair — plausible-looking forces that are entirely wrong.
///
/// ~~Particle–particle pairs are keyed by their ordered **index** pair~~
/// **CORRECTED 2026-09-17** — the store was index-keyed, which was safe only
/// while the particle set was fixed. It is tag-keyed now, matching upstream,
/// so insertion and removal are possible at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContactKey {
    /// Contact between two particles, stored with `lo < hi`.
    Pair {
        /// Lower particle tag.
        lo: usize,
        /// Higher particle tag.
        hi: usize,
    },
    /// Contact between a particle and a wall.
    Wall {
        /// Particle tag.
        particle: usize,
        /// Caller-assigned wall identifier.
        wall: usize,
    },
    /// A second per-contact scalar of another contact (the plastic model's
    /// `deltaMax`), held in the store under a key derived from the contact's
    /// own by [`ContactKey::aux`], so it shares the contact's life cycle.
    Aux {
        /// Which kind of contact it belongs to (0 pair, 1 wall, 2 facet).
        kind: u8,
        /// Which three-value slot of the contact's extra state this is
        /// (see [`ContactAux`]).
        slot: u8,
        /// First identifier of the owning contact.
        a: usize,
        /// Second identifier.
        b: usize,
        /// Third identifier.
        c: usize,
    },
    /// Contact between a particle and one facet of a mesh wall (upstream's
    /// per-triangle mesh contact history, `FixContactHistoryMesh`).
    Tri {
        /// Particle tag.
        particle: usize,
        /// Caller-assigned wall identifier.
        wall: usize,
        /// Facet index.
        tri: usize,
    },
}

impl ContactKey {
    /// Key for the particle pair with tags `(i, j)`, normalised so that
    /// `(i, j)` and `(j, i)` map to the same entry.
    ///
    /// `i` and `j` are **stable tags**, not array indices — see the type-level
    /// documentation for why the distinction is load-bearing.
    #[must_use]
    pub fn pair(i: usize, j: usize) -> Self {
        Self::Pair {
            lo: i.min(j),
            hi: i.max(j),
        }
    }

    /// The key of this contact's auxiliary scalar (see [`ContactKey::Aux`]).
    #[must_use]
    pub fn aux(self) -> Self {
        self.aux_slot(0)
    }

    /// The key of this contact's `slot`-th auxiliary three-value record.
    #[must_use]
    pub fn aux_slot(self, slot: u8) -> Self {
        match self {
            Self::Pair { lo, hi } => Self::Aux {
                kind: 0,
                slot,
                a: lo,
                b: hi,
                c: 0,
            },
            Self::Wall { particle, wall } => Self::Aux {
                kind: 1,
                slot,
                a: particle,
                b: wall,
                c: 0,
            },
            Self::Tri {
                particle,
                wall,
                tri,
            } => Self::Aux {
                kind: 2,
                slot,
                a: particle,
                b: wall,
                c: tri,
            },
            Self::Aux { .. } => self,
        }
    }

    /// Key for a particle–mesh-facet contact.
    #[must_use]
    pub fn tri(particle: usize, wall: usize, tri: usize) -> Self {
        Self::Tri {
            particle,
            wall,
            tri,
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
    /// One entry per contact carrying history.
    ///
    /// ~~Two parallel `HashMap`s, `shear` and `live`~~ **CORRECTED 2026-09-17**
    /// — splitting them meant every contact paid *two* hash lookups and two
    /// inserts per step, and both maps paid a full `retain` in
    /// [`ShearHistory::end_step`]. On the HTR-10 bed (66 500 live contacts)
    /// that was measured at **15.2 ms of a 33.6 ms timestep**, i.e. the
    /// dominant cost of the whole simulation once the contact arithmetic had
    /// been parallelised. Merged into one map of one struct, hashed by
    /// [`ContactHasher`].
    contacts: HashMap<ContactKey, ContactHistoryEntry, BuildContactHasher>,
}

/// One contact's stored state: its tangential spring, and whether it has been
/// refreshed during the current force evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ContactHistoryEntry {
    /// Tangential displacement `ξ_t` `[m]`.
    shear: Vec3,
    /// Refreshed since the last [`ShearHistory::begin_step`]. A contact that
    /// ends a step stale has separated and is dropped.
    live: bool,
}

/// `BuildHasher` for [`ContactKey`], replacing the standard library's SipHash.
///
/// # Why this is hand-rolled rather than a dependency
///
/// `ContactKey` is two small integers and a discriminant. SipHash is a
/// keyed, DoS-resistant hash designed for adversarial string keys, and it is
/// roughly an order of magnitude more expensive than what integer keys need.
/// The contact store is looked up ~370 000 times and written ~66 500 times per
/// timestep, so the hash *is* the cost.
///
/// A crate such as `rustc-hash` would do this, but adding one would mean a new
/// entry in the root `[workspace.dependencies]` for twenty lines of
/// multiply-xor. This is those twenty lines: a standard FxHash-style
/// multiply-and-rotate accumulator, pure Rust, `no_std`-compatible arithmetic,
/// and safe on every target the workspace builds for.
///
/// **This is not a security boundary.** Contact keys come from the
/// simulation's own particle tags, never from untrusted input, so hash-flooding
/// resistance buys nothing here.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildContactHasher;

impl std::hash::BuildHasher for BuildContactHasher {
    type Hasher = ContactHasher;
    fn build_hasher(&self) -> ContactHasher {
        ContactHasher(0)
    }
}

/// The FxHash-style accumulator built by [`BuildContactHasher`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ContactHasher(u64);

impl ContactHasher {
    /// FxHash's 64-bit multiplier: the odd constant from the golden-ratio
    /// reciprocal, which spreads low-entropy integer keys across the word.
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl std::hash::Hasher for ContactHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Not on the hot path — `ContactKey` hashes through the integer
        // methods below — but a `Hasher` must handle it.
        for &b in bytes {
            self.add(u64::from(b));
        }
    }

    #[inline]
    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
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
        self.contacts.len()
    }

    /// Whether the store holds no contacts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }

    /// The stored tangential displacement `ξ_t` `[m]` for a contact, if any.
    #[must_use]
    pub fn get(&self, key: ContactKey) -> Option<Vec3> {
        self.contacts.get(&key).map(|e| e.shear)
    }

    /// Insert a stored displacement for a contact that has none yet, without
    /// marking it live. Used to hand a new mesh-facet contact the history of a
    /// coplanar neighbour facet (upstream `checkCoplanarContactHistory`).
    pub fn seed(&mut self, key: ContactKey, shear: Vec3) {
        self.contacts
            .insert(key, ContactHistoryEntry { shear, live: false });
    }

    /// Re-orient a stored pair displacement: replace it by `0 - shear`
    /// (exact negation, `+0` stays `+0`). The store holds a pair in the
    /// (lower tag, higher tag) orientation; a caller that evaluates the pair
    /// the other way round flips before and after (upstream stores the
    /// partner's copy as `-hist`, `FixContactHistory::pre_exchange`). A no-op
    /// when the contact has no entry.
    pub fn flip(&mut self, key: ContactKey) {
        if let Some(e) = self.contacts.get_mut(&key) {
            e.shear = Vec3::zero().sub(e.shear);
        }
    }

    /// [`ShearHistory::flip`] for an auxiliary record (the rolling-torque
    /// history of the EPSD models is declared `"1"` = sign-flipping upstream).
    pub fn flip_aux(&mut self, key: ContactKey, slot: u8) {
        self.flip(key.aux_slot(slot));
    }

    /// Mark every stored contact stale, at the top of a force evaluation.
    pub fn begin_step(&mut self) {
        for e in self.contacts.values_mut() {
            e.live = false;
        }
    }

    /// [`ShearHistory::end_step`] for LIGGGHTS' list-life history: contacts
    /// **between particles** (and their auxiliary scalars) are kept even when
    /// not refreshed this step, because upstream never clears a pair's shear
    /// history when it separates inside a neighbour list's life
    /// (`surfacesClose` runs only for `rsq < contactDistanceFactor^2 radsum^2`,
    /// which for the default factor 1 is the same test as touching, so it never
    /// runs); history is dropped only at a list rebuild, by
    /// [`ShearHistory::retain_pairs`]. Wall and facet contacts keep the
    /// ordinary rule.
    pub fn end_step_keep_pairs(&mut self) {
        self.contacts.retain(|k, e| {
            e.live || matches!(k, ContactKey::Pair { .. } | ContactKey::Aux { kind: 0, .. })
        });
    }

    /// Keep a particle-pair entry (and its auxiliary scalar) only if
    /// `keep(lo_tag, hi_tag)`: the carry-over test of upstream's list build
    /// (`granular_bin_no_newton`: history is copied only when the pair
    /// overlaps at the rebuild, `rsq < radsum*radsum`). Other contacts are kept.
    pub fn retain_pairs(&mut self, mut keep: impl FnMut(usize, usize) -> bool) {
        self.contacts.retain(|k, _| match *k {
            ContactKey::Pair { lo, hi }
            | ContactKey::Aux {
                kind: 0,
                a: lo,
                b: hi,
                ..
            } => keep(lo, hi),
            _ => true,
        });
    }

    /// Drop the history of every contact not refreshed since
    /// [`ShearHistory::begin_step`] — i.e. every contact that has separated.
    pub fn end_step(&mut self) {
        self.contacts.retain(|_, e| e.live);
    }

    /// Forget a contact's tangential displacement: upstream's
    /// `TangentialModel::surfacesClose` (`shear[0..3] = 0`, tangential flag
    /// cleared) for a pair that is inside the neighbour list's contact
    /// distance factor but no longer touching. A zero entry and no entry are
    /// the same history. gh:#1411.
    pub fn forget_shear(&mut self, key: ContactKey) {
        self.contacts.remove(&key);
    }

    /// Read the current displacement and mark the contact live, inserting a
    /// zero entry for a contact seen for the first time.
    pub(crate) fn touch(&mut self, key: ContactKey) -> Vec3 {
        let entry = self.contacts.entry(key).or_insert(ContactHistoryEntry {
            shear: Vec3::zero(),
            live: true,
        });
        entry.live = true;
        entry.shear
    }

    /// Overwrite the stored displacement for a contact.
    pub(crate) fn set(&mut self, key: ContactKey, value: Vec3) {
        // The contact was made live by `touch` immediately before this in
        // `resolve`; an `entry` here keeps that invariant if it ever is not.
        self.contacts
            .entry(key)
            .or_insert(ContactHistoryEntry {
                shear: value,
                live: true,
            })
            .shear = value;
    }

    /// Record a contact's updated tangential displacement and mark it live —
    /// the public equivalent of the private `touch` + `set` pair that
    /// [`GranularContactModel::resolve`] performs internally.
    ///
    /// This exists for the **parallel force loop**, which evaluates contacts
    /// concurrently through
    /// [`GranularContactModel::resolve_pure`] (holding only `&ShearHistory`)
    /// and then writes the results back here, serially, in the same contact
    /// order the scalar path would have used. Calling it with the value
    /// `resolve_pure` returned is exactly equivalent to having called
    /// [`GranularContactModel::resolve`] on that contact.
    ///
    /// Call it **only** for contacts that actually carry history: a
    /// [`TangentialModel::NoHistory`] contact must not enter the store, or
    /// [`ShearHistory::end_step`] will retain an entry for a stateless contact.
    pub fn store(&mut self, key: ContactKey, shear: Vec3) {
        self.contacts
            .insert(key, ContactHistoryEntry { shear, live: true });
    }
}

/// Closed set of cohesion models (upstream `cohesion_model_*.h`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GranularCohesion {
    /// No cohesion (upstream `cohesion off`, the default).
    Off,
    /// Upstream `CohesionModel<COHESION_SJKR>` (`sjkr`,
    /// `cohesion_model_sjkr.h`): the **simplified JKR** model, a normal force
    /// `-k_A A_contact` proportional to the exact two-sphere (or sphere-wall)
    /// contact area, with `k_A` the cohesion energy density `[J/m^3]`.
    Sjkr {
        /// `cohesionEnergyDensity` `[J/m^3]`.
        energy_density: f64,
        /// Upstream `tangential_reduce` (default off): also add the
        /// cohesion force to `F_n`, which the tangential and rolling models
        /// read.
        tangential_reduce: bool,
    },
    /// Upstream `CohesionModel<COHESION_SJKR2>` (`sjkr2`): as `sjkr` with the
    /// spherical-cap area `2 pi R* delta` (`R* = 2 r_i r_j/(r_i + r_j)`).
    Sjkr2 {
        /// `cohesionEnergyDensity` `[J/m^3]`.
        energy_density: f64,
        /// Upstream `tangential_reduce` (default off).
        tangential_reduce: bool,
    },
    /// Upstream `CohesionModel<COHESION_EASO_CAPILLARY_VISCOUS>` or
    /// `CohesionModel<COHESION_WASHINO_CAPILLARY_VISCOUS>`: a **liquid bridge**
    /// with capillary and viscous forces ([`crate::capillary`], gh:#1411).
    /// Stateful: it needs the per-particle liquid content and per-pair bridge
    /// flag of [`crate::granular_system::GranularSystem::with_capillary`], and
    /// it acts out of contact too, so it is resolved by that system, not by a
    /// bare contact law (a bare law with no liquid supplied sees dry
    /// particles and adds nothing).
    Capillary(crate::capillary::CapillaryParams),
}

impl GranularCohesion {
    /// `cohesion sjkr` with upstream's defaults.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if the energy density is negative or not finite.
    pub fn sjkr(energy_density: f64) -> Result<Self, DemError> {
        Self::check(energy_density)?;
        Ok(Self::Sjkr {
            energy_density,
            tangential_reduce: false,
        })
    }

    /// `cohesion sjkr2` with upstream's defaults.
    ///
    /// # Errors
    ///
    /// As [`GranularCohesion::sjkr`].
    pub fn sjkr2(energy_density: f64) -> Result<Self, DemError> {
        Self::check(energy_density)?;
        Ok(Self::Sjkr2 {
            energy_density,
            tangential_reduce: false,
        })
    }

    fn check(energy_density: f64) -> Result<(), DemError> {
        if !(energy_density >= 0.0) || !energy_density.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "cohesion energy density must be finite and >= 0, got {energy_density}"
            )));
        }
        Ok(())
    }

    /// Upstream's `tangential_reduce` switch (SJKR and SJKR2).
    #[must_use]
    pub fn with_tangential_reduce(mut self, on: bool) -> Self {
        match &mut self {
            Self::Sjkr {
                tangential_reduce, ..
            }
            | Self::Sjkr2 {
                tangential_reduce, ..
            } => {
                *tangential_reduce = on;
            }
            Self::Capillary(p) => p.tangential_reduce = on,
            Self::Off => {}
        }
        self
    }

    /// The cohesion normal force `F_coh` `[N]` of one contact (negative =
    /// attractive), along `+e_n` on `i`, and whether it is also added to
    /// `F_n` (`tangential_reduce`). Translation of the two
    /// `surfacesIntersect` bodies (`3d5c00f2`).
    #[must_use]
    pub fn normal_force(&self, k: &ContactKinematics) -> (f64, bool) {
        use std::f64::consts::PI;
        let (energy_density, reduce, second) = match *self {
            // the bridge force needs the liquid state: `CapillaryParams::intersect`
            Self::Capillary(_) => return (0.0, false),
            Self::Off => return (0.0, false),
            Self::Sjkr {
                energy_density,
                tangential_reduce,
            } => (energy_density, tangential_reduce, false),
            Self::Sjkr2 {
                energy_density,
                tangential_reduce,
            } => (energy_density, tangential_reduce, true),
        };
        let r = k.r;
        let ri = k.rad_i;
        let rj = k.rad_j;
        // `area_ratio` is 1 here and multiplying by it is exact
        let acont = if second {
            if k.is_wall {
                PI * 2.0 * ri * (ri - r)
            } else {
                PI * 2.0 * (2.0 * ri * rj / (ri + rj)) * (ri + rj - r)
            }
        } else if k.is_wall {
            (ri * ri - r * r) * PI
        } else {
            -PI / 4.0 * ((r - ri - rj) * (r + ri - rj) * (r - ri + rj) * (r + ri + rj)) / (r * r)
        };
        (-energy_density * acont, reduce)
    }
}

/// Closed set of rolling-resistance models.
///
/// Rolling resistance is what gives a granular heap a finite **angle of
/// repose**: without it, spheres roll off each other and a pile spreads until
/// it is flat. For a pebble bed it is the dominant knob on packing structure,
/// so it is part of this pipeline rather than an optional extra.
///
/// # Relationship to [`crate::rolling`]
///
/// [`crate::rolling::RollingModel`] has a similar constant-torque variant, but
/// it diverges from upstream in three ways, all reproduced correctly here:
///
/// 1. it scales the torque by the **total** normal force `|F_n|` (including the
///    viscous damping term), where upstream CDT uses the **elastic** part only,
///    `k_n·δ_n`;
/// 2. it does **not** remove the torsion (normal) component of the resisting
///    torque, which upstream does by default (`torsionTorque` is off unless
///    asked for);
/// 3. for a wall contact it uses `ω_i − ω_j`, where upstream uses the
///    contact-point rolling velocity `w_r`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RollingModel {
    /// No rolling resistance (upstream `rolling_model off`).
    Off,
    /// Upstream `RollingModel<ROLLING_CDT>` — **constant directional torque**.
    ///
    /// `M_r = µ_r · k_n·δ_n · R* · ŵ_r`, applied as `−M_r` to `i` and `+M_r` to
    /// `j`, with the component along `ê_n` removed unless `torsion_torque` is
    /// set. The magnitude is set by the elastic normal load and does not depend
    /// on rolling *speed* — only its direction, hence "constant torque".
    Cdt {
        /// Rolling-friction coefficient `µ_r` `[-]`, non-negative.
        mu_r: f64,
        /// Upstream `torsionTorque` switch. **Default `false`**, which *removes*
        /// the torque component along the contact normal.
        torsion_torque: bool,
    },
    /// Upstream `RollingModel<ROLLING_EPSD>` / `EPSD2` / `EPSD3`
    /// (`rolling_model_epsd*.h`): the **elastic-plastic spring-dashpot**
    /// rolling resistance of Ai et al. (2011). A rolling spring accumulates
    /// the torque `M += dt k_r w_r` in a per-contact history (three values,
    /// sign-flipping), which is capped at the Coulomb-like limit
    /// `|F_n| R* mu_r`; EPSD and EPSD3 add a rolling dashpot while below the
    /// cap. gh:#1410.
    Epsd {
        /// Which of the three upstream variants.
        kind: EpsdKind,
        /// `coeffRollFrict` `[-]`.
        mu_r: f64,
        /// `coeffRollVisc` `[-]` (EPSD, EPSD3; unused by EPSD2).
        mu_visc: f64,
        /// `coeffRollStiffness` `[-]` (EPSD3 only).
        stiffness: f64,
        /// Upstream `torsionTorque` (default off: remove the normal part of
        /// the relative rotation).
        torsion_torque: bool,
    },
    /// Upstream `RollingModel<ROLLING_LUDING>` (`rolling_model_luding.h`):
    /// Luding's rolling spring-dashpot with the friction limit
    /// `mu_r |F_n + k_c delta_n - f_adh| R*` (`kc`, `f_adh` stored by
    /// `model luding`). Unlike EPSD the spring history is written
    /// **unconditionally** (also at set-up). ~~`torsion off` only.~~
    /// **CORRECTED 2026-11-11**: `torsion on` is ported (field `torsion`,
    /// [`RollingModel::with_luding_torsion`]) and pinned bit-identical.
    /// gh:#1410.
    Luding {
        /// `coeffRollFrict` `[-]`.
        mu_r: f64,
        /// `coeffRollVisc` `[-]`.
        mu_visc: f64,
        /// `kR2kcMax` (property `coeffRollingStiffness`): `k_r = k_t kR2kcMax R*^2`.
        kr_ratio: f64,
        /// Upstream `torsion` (default off): add a torsion spring-dashpot on
        /// the normal component of the relative rotation, with the same
        /// coefficients. **Upstream quirks kept for parity:** its history
        /// aliases the rolling history (`c_tor_history = &contact_history
        /// [history_offset]`, not `+ 3`), and for a particle-particle contact
        /// `calcTorTorque` writes into `r_torque`, replacing the rolling
        /// torque; for a wall contact it writes a separate `r_tor_torque`
        /// that is applied in addition.
        torsion: bool,
    },
}

/// Fields of [`RollingModel::Luding`] documented here to keep the enum short.
/// ~~Upstream `torsion` is **not** ported~~ **CORRECTED 2026-11-11**: it is
/// (see the `torsion` field); its history aliases the rolling one in upstream.
#[allow(dead_code)]
const _LUDING_ROLLING_NOTE: () = ();

/// The three EPSD rolling models of upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsdKind {
    /// `epsd`: `k_r = 2.25 k_n mu_r^2 R*^2`, spring and dashpot.
    One,
    /// `epsd2`: `k_r = k_t R*^2`, spring only.
    Two,
    /// `epsd3`: `k_r = c_s k_n mu_r^2 R*^2` with a user stiffness factor
    /// `c_s` (`coeffRollStiffness`), spring and dashpot.
    Three,
}

impl RollingModel {
    /// Whether the model stores a per-contact rolling torque.
    #[must_use]
    pub fn has_state(&self) -> bool {
        matches!(self, Self::Epsd { .. } | Self::Luding { .. })
    }

    /// `rolling_model luding` (see [`RollingModel::Luding`]).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a coefficient is negative or not finite.
    pub fn luding(mu_r: f64, mu_visc: f64, kr_ratio: f64) -> Result<Self, DemError> {
        for (name, v) in [("mu_r", mu_r), ("mu_visc", mu_visc), ("kR2kcMax", kr_ratio)] {
            if !(v >= 0.0) || !v.is_finite() {
                return Err(DemError::InvalidInput(format!(
                    "luding rolling coefficient {name} must be finite and >= 0, got {v}"
                )));
            }
        }
        Ok(Self::Luding {
            mu_r,
            mu_visc,
            kr_ratio,
            torsion: false,
        })
    }

    /// `rolling_friction luding torsion on|off` (no-op for other models).
    #[must_use]
    pub fn with_luding_torsion(self, on: bool) -> Self {
        match self {
            Self::Luding {
                mu_r,
                mu_visc,
                kr_ratio,
                ..
            } => Self::Luding {
                mu_r,
                mu_visc,
                kr_ratio,
                torsion: on,
            },
            other => other,
        }
    }

    /// Torque and new history of any stateful rolling model (EPSD or Luding).
    /// `kc_fo` is the normal model's stored `(kc, f_adh)`. The middle value is
    /// the separate torsion torque of a Luding **wall** contact with `torsion
    /// on` (applied in addition to the first), `None` otherwise.
    #[must_use]
    pub fn stateful_torque(
        &self,
        k: &ContactKinematics,
        normal: &NormalOutcome,
        kc_fo: (f64, f64),
        prior: Vec3,
        dt: f64,
        update: bool,
    ) -> (Vec3, Option<Vec3>, Vec3) {
        match self {
            Self::Luding { .. } => self.luding_torque(k, normal, kc_fo, prior, dt),
            _ => {
                let (m, h) = self.epsd_torque(k, normal, prior, dt, update);
                (m, None, h)
            }
        }
    }

    /// `RollingModel<ROLLING_LUDING>::calcRollTorque` (`torsion off`).
    #[must_use]
    fn luding_torque(
        &self,
        k: &ContactKinematics,
        normal: &NormalOutcome,
        kc_fo: (f64, f64),
        prior: Vec3,
        dt: f64,
    ) -> (Vec3, Option<Vec3>, Vec3) {
        let Self::Luding {
            mu_r: rmu,
            mu_visc,
            kr_ratio,
            torsion,
        } = *self
        else {
            return (Vec3::zero(), None, prior);
        };
        let radi = k.rad_i;
        let radj = k.rad_j;
        let reff = if k.is_wall {
            radi
        } else {
            radi * radj / (radi + radj)
        };
        let (wr1, wr2, wr3, r_inertia);
        if k.is_wall {
            wr1 = k.wr.x;
            wr2 = k.wr.y;
            wr3 = k.wr.z;
            r_inertia = 1.4 * k.mass_i * reff * reff;
        } else {
            wr1 = k.omega_i.x - k.omega_j.x;
            wr2 = k.omega_i.y - k.omega_j.y;
            wr3 = k.omega_i.z - k.omega_j.z;
            let red_i = k.mass_i * radi * radi;
            let red_j = k.mass_j * radj * radj;
            r_inertia = 1.4 * red_i * red_j / (red_i + red_j);
        }
        let (enx, eny, enz) = (k.en.x, k.en.y, k.en.z);
        let wr_dot_delta = wr1 * enx + wr2 * eny + wr3 * enz;
        let wr_n = Vec3::new(enx * wr_dot_delta, eny * wr_dot_delta, enz * wr_dot_delta);
        let wr_t = Vec3::new(wr1 - wr_n.x, wr2 - wr_n.y, wr3 - wr_n.z);
        let kr = normal.kt * kr_ratio * reff * reff;
        let (kc, f_adh) = kc_fo;
        // calcRollTorque / calcTorTorque share one body: the spring history
        // `prior` integrates `w`, is capped at the modified Coulomb limit, is
        // stored, and below the cap a dashpot on `w` is added.
        let spring_dashpot = |w: Vec3, prior: Vec3| -> (Vec3, Vec3) {
            let dt_kr = dt * kr;
            let mut t = Vec3::new(
                prior.x + w.x * dt_kr,
                prior.y + w.y * dt_kr,
                prior.z + w.z * dt_kr,
            );
            let mag = t.norm();
            let max = rmu * (normal.fn_scalar + kc * k.delta_n - f_adh).abs() * reff;
            if mag > max {
                let factor = max / mag;
                t = Vec3::new(t.x * factor, t.y * factor, t.z * factor);
                (t, t)
            } else {
                let hist = t;
                let r_coef = normal.gammat * mu_visc * 2.0 * (r_inertia * kr).sqrt();
                t = Vec3::new(t.x + r_coef * w.x, t.y + r_coef * w.y, t.z + r_coef * w.z);
                (t, hist)
            }
        };
        let (mut r_torque, mut hist) = spring_dashpot(wr_t, prior);
        let mut tor = None;
        if torsion {
            let dot = |w: Vec3| enx * w.x + eny * w.y + enz * w.z;
            let c = if k.is_wall {
                dot(k.omega_i)
            } else {
                dot(k.omega_i) - dot(k.omega_j)
            };
            let omega_normal = Vec3::new(enx * c, eny * c, enz * c);
            // The torsion spring reads and writes the SAME history as the
            // rolling spring (upstream aliasing).
            let (t2, h2) = spring_dashpot(omega_normal, hist);
            hist = h2;
            if k.is_wall {
                tor = Some(t2);
            } else {
                r_torque = t2;
            }
        }
        (r_torque, tor, hist)
    }

    /// `rolling_model epsd` with upstream's defaults (`torsionTorque off`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a coefficient is negative or not finite.
    pub fn epsd(mu_r: f64, mu_visc: f64) -> Result<Self, DemError> {
        Self::epsd_variant(EpsdKind::One, mu_r, mu_visc, 2.25)
    }

    /// `rolling_model epsd2` (spring only, `k_r = k_t R*^2`).
    ///
    /// # Errors
    ///
    /// As [`RollingModel::epsd`].
    pub fn epsd2(mu_r: f64) -> Result<Self, DemError> {
        Self::epsd_variant(EpsdKind::Two, mu_r, 0.0, 0.0)
    }

    /// `rolling_model epsd3` (user stiffness factor).
    ///
    /// # Errors
    ///
    /// As [`RollingModel::epsd`].
    pub fn epsd3(mu_r: f64, mu_visc: f64, stiffness: f64) -> Result<Self, DemError> {
        Self::epsd_variant(EpsdKind::Three, mu_r, mu_visc, stiffness)
    }

    fn epsd_variant(
        kind: EpsdKind,
        mu_r: f64,
        mu_visc: f64,
        stiffness: f64,
    ) -> Result<Self, DemError> {
        for (name, v) in [
            ("mu_r", mu_r),
            ("mu_visc", mu_visc),
            ("stiffness", stiffness),
        ] {
            if !(v >= 0.0) || !v.is_finite() {
                return Err(DemError::InvalidInput(format!(
                    "EPSD rolling coefficient {name} must be finite and >= 0, got {v}"
                )));
            }
        }
        Ok(Self::Epsd {
            kind,
            mu_r,
            mu_visc,
            stiffness,
            torsion_torque: false,
        })
    }

    /// The EPSD rolling torque `M_r` and the rolling-spring history to store,
    /// for one contact: translation of `calcRollTorque` in
    /// `rolling_model_epsd.h`, `_epsd2.h` and `_epsd3.h` (LIGGGHTS-PUBLIC
    /// `3d5c00f2`), with the three-dimensional rotational inertia factor 1.4.
    /// `prior` is the stored `r_torque_old`; `update` is upstream's
    /// `computeflag && shearupdate` (whether the history is written).
    /// Returns `(torque, history)`; [`RollingModel::Off`] and CDT return a zero
    /// torque and the prior history (CDT has its own [`RollingModel::rolling_torque`]).
    #[must_use]
    pub fn epsd_torque(
        &self,
        k: &ContactKinematics,
        normal: &NormalOutcome,
        prior: Vec3,
        dt: f64,
        update: bool,
    ) -> (Vec3, Vec3) {
        let Self::Epsd {
            kind,
            mu_r: rmu,
            mu_visc,
            stiffness,
            torsion_torque,
        } = *self
        else {
            return (Vec3::zero(), prior);
        };
        let radi = k.rad_i;
        let radj = k.rad_j;
        let reff = if k.is_wall {
            radi
        } else {
            radi * radj / (radi + radj)
        };
        // relative rotational velocity and the inertia the dashpot scales with
        let (wr1, wr2, wr3, r_inertia);
        if k.is_wall {
            wr1 = k.wr.x;
            wr2 = k.wr.y;
            wr3 = k.wr.z;
            r_inertia = 1.4 * k.mass_i * reff * reff;
        } else {
            wr1 = k.omega_i.x - k.omega_j.x;
            wr2 = k.omega_i.y - k.omega_j.y;
            wr3 = k.omega_i.z - k.omega_j.z;
            let red_i = k.mass_i * radi * radi;
            let red_j = k.mass_j * radj * radj;
            r_inertia = 1.4 * red_i * red_j / (red_i + red_j);
        }
        let (enx, eny, enz) = (k.en.x, k.en.y, k.en.z);
        let wr_tot = if torsion_torque {
            Vec3::new(wr1, wr2, wr3)
        } else {
            let wr_dot_delta = wr1 * enx + wr2 * eny + wr3 * enz;
            let wr_n = Vec3::new(enx * wr_dot_delta, eny * wr_dot_delta, enz * wr_dot_delta);
            Vec3::new(wr1 - wr_n.x, wr2 - wr_n.y, wr3 - wr_n.z)
        };
        // spring
        let kr = match kind {
            EpsdKind::One => 2.25 * normal_kn(normal) * rmu * rmu * reff * reff,
            EpsdKind::Two => normal.kt * reff * reff,
            EpsdKind::Three => stiffness * normal_kn(normal) * rmu * rmu * reff * reff,
        };
        let dt_kr = dt * kr;
        let mut r_torque = Vec3::new(
            prior.x + wr_tot.x * dt_kr,
            prior.y + wr_tot.y * dt_kr,
            prior.z + wr_tot.z * dt_kr,
        );
        // limit max. torque
        let r_torque_mag = r_torque.norm();
        let r_torque_max = normal.fn_scalar.abs() * reff * rmu;
        let mut hist = prior;
        if r_torque_mag > r_torque_max {
            let factor = r_torque_max / r_torque_mag;
            r_torque = Vec3::new(
                r_torque.x * factor,
                r_torque.y * factor,
                r_torque.z * factor,
            );
            if update {
                hist = r_torque;
            }
            // no dashpot at full mobilisation
        } else {
            if update {
                hist = r_torque; // spring part, before the dashpot is added
            }
            if kind != EpsdKind::Two {
                let r_coef = mu_visc * 2.0 * (r_inertia * kr).sqrt();
                r_torque = Vec3::new(
                    r_torque.x + r_coef * wr_tot.x,
                    r_torque.y + r_coef * wr_tot.y,
                    r_torque.z + r_coef * wr_tot.z,
                );
            }
        }
        (r_torque, hist)
    }
}

/// `sidata.kn` as the rolling models read it.
fn normal_kn(n: &NormalOutcome) -> f64 {
    n.kn
}

impl RollingModel {
    /// Build a validated CDT model with upstream's default settings
    /// (`torsionTorque` off).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `mu_r` is negative or not finite.
    pub fn cdt(mu_r: f64) -> Result<Self, DemError> {
        if !(mu_r >= 0.0) || !mu_r.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "rolling-friction coefficient mu_r must be finite and >= 0, got {mu_r}"
            )));
        }
        Ok(Self::Cdt {
            mu_r,
            torsion_torque: false,
        })
    }

    /// Resisting rolling torque `M_r` `[N·m]` for one contact.
    ///
    /// The caller applies `−M_r` to `i` and `+M_r` to `j` (upstream's
    /// convention). `omega_i` / `omega_j` are the two particles' angular
    /// velocities `[rad/s]`; for a wall contact pass `Vec3::zero()` for
    /// `omega_j` — the wall branch uses the contact-point rolling velocity
    /// `w_r` derived from `k` instead, as upstream does.
    #[must_use]
    pub fn rolling_torque(
        &self,
        k: &ContactKinematics,
        normal: &NormalOutcome,
        omega_i: Vec3,
        omega_j: Vec3,
        is_wall: bool,
    ) -> Vec3 {
        let Self::Cdt {
            mu_r,
            torsion_torque,
        } = *self
        else {
            return Vec3::zero();
        };

        // Rolling velocity: omega_i - omega_j for a pair; the contact-point
        // rolling velocity w_r (`sidata.wr1..3`) for a wall.
        let wr = if is_wall { k.wr } else { omega_i.sub(omega_j) };
        let wr_mag = wr.norm();
        if wr_mag <= 0.0 {
            return Vec3::zero();
        }

        // Upstream uses the ELASTIC normal force, not sidata.Fn, with these
        // operation orders (`rolling_model_cdt.h`):
        //   wall: r_torque = rmu*Fn*wr/wrmag*reff, Fn = deltan*kn
        //   pair: r_torque = wr * (rmu*kn*deltan*reff/wrmag)
        let mut r_torque = if is_wall {
            let fn_elastic = k.delta_n * normal.kn;
            Vec3::new(
                mu_r * fn_elastic * wr.x / wr_mag * k.r_eff,
                mu_r * fn_elastic * wr.y / wr_mag * k.r_eff,
                mu_r * fn_elastic * wr.z / wr_mag * k.r_eff,
            )
        } else {
            wr.scale(mu_r * normal.kn * k.delta_n * k.r_eff / wr_mag)
        };
        if !torsion_torque {
            // Remove the component along the contact normal (torsion).
            let along = r_torque.dot(k.en);
            r_torque = r_torque.sub(k.en.scale(along));
        }
        r_torque
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
    /// The rolling-resistance model. Defaults to [`RollingModel::Off`] via
    /// [`GranularContactModel::new`]; set it with
    /// [`GranularContactModel::with_rolling`].
    pub rolling: RollingModel,
    /// Per-atom-type material table (`None` = the single material inside
    /// [`GranularContactModel::normal`]). Set with
    /// [`GranularContactModel::with_materials`].
    pub materials: Option<MaterialTable>,
    /// The cohesion model ([`GranularCohesion::Off`] by default); set it
    /// with [`GranularContactModel::with_cohesion`]. gh:#1411.
    pub cohesion: GranularCohesion,
    /// Per-type-pair sub-model coefficients (`None` = every pair uses the
    /// coefficients inside [`GranularContactModel::normal`] etc.). Set with
    /// [`GranularContactModel::with_pair_models`]. gh:#1410.
    pub pair_models: Option<PairModelTable>,
}

/// The sub-models of one atom-type pair: upstream's `peratomtypepair`
/// coefficient matrices of the normal, tangential, rolling and cohesion
/// models evaluated at one `(itype, jtype)`. See [`PairModelTable`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairModel {
    /// Normal model with this pair's coefficients.
    pub normal: GranularNormalModel,
    /// Tangential model with this pair's coefficients.
    pub tangential: TangentialModel,
    /// Rolling model with this pair's coefficients.
    pub rolling: RollingModel,
    /// Cohesion model with this pair's coefficients.
    pub cohesion: GranularCohesion,
}

/// Per-`(itype, jtype)` model coefficients: the Rust form of upstream's
/// `MatrixProperty` lookups (`coeff[itype][jtype]`, `PropertyRegistry`) for
/// every model-specific coefficient (stiffnesses, adhesion, plasticity depth,
/// rolling and cohesion coefficients, `peratomtype` values such as the
/// Thornton-Ning yield ratio). The entry for `(i, j)` is used when the
/// contact is resolved with `itype = i`, `jtype = j`; fill **both** `(i, j)`
/// and `(j, i)` (they differ only for a `peratomtype` value, which upstream
/// indexes by `itype`). `E`, `nu`, `e`, `mu` still come from the
/// [`MaterialTable`]. gh:#1410.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairModelTable {
    n_types: usize,
    entries: [[Option<PairModel>; MAX_TYPES]; MAX_TYPES],
}

impl PairModelTable {
    /// An empty table for `n_types` atom types.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless `1 <= n_types <= MAX_TYPES`.
    pub fn new(n_types: usize) -> Result<Self, DemError> {
        if n_types == 0 || n_types > MAX_TYPES {
            return Err(DemError::InvalidInput(format!(
                "a pair-model table needs 1..={MAX_TYPES} types, got {n_types}"
            )));
        }
        Ok(Self {
            n_types,
            entries: [[None; MAX_TYPES]; MAX_TYPES],
        })
    }

    /// Set the entry for `(itype, jtype)` (0-based).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a type is out of range.
    pub fn set(mut self, itype: usize, jtype: usize, model: PairModel) -> Result<Self, DemError> {
        if itype >= self.n_types || jtype >= self.n_types {
            return Err(DemError::InvalidInput(format!(
                "pair ({itype}, {jtype}) outside the {} types of the table",
                self.n_types
            )));
        }
        self.entries[itype][jtype] = Some(model);
        Ok(self)
    }

    /// Set `(i, j)` and `(j, i)` to the same model.
    ///
    /// # Errors
    ///
    /// As [`PairModelTable::set`].
    pub fn set_symmetric(self, i: usize, j: usize, model: PairModel) -> Result<Self, DemError> {
        self.set(i, j, model)?.set(j, i, model)
    }
}

impl GranularContactModel {
    /// Return a copy of this model with the given cohesion model.
    #[must_use]
    pub fn with_cohesion(self, cohesion: GranularCohesion) -> Self {
        Self { cohesion, ..self }
    }

    /// Whether this model stores a persistent tangential spring per contact.
    ///
    /// `true` for [`TangentialModel::History`], `false` for
    /// [`TangentialModel::NoHistory`], which is stateless and whose contacts
    /// must never enter the [`ShearHistory`] store. The parallel force loop
    /// asks once per step rather than per contact — the answer is a property of
    /// the model, not of any individual contact.
    #[must_use]
    pub fn has_tangential_history(&self) -> bool {
        matches!(
            self.tangential,
            TangentialModel::History | TangentialModel::LudingTn { .. }
        )
    }

    /// Assemble a contact model from its normal and tangential halves, with
    /// rolling resistance **off** (upstream's default `pair_style gran` has no
    /// rolling model unless one is named).
    #[must_use]
    pub fn new(normal: GranularNormalModel, tangential: TangentialModel) -> Self {
        Self {
            normal,
            tangential,
            rolling: RollingModel::Off,
            materials: None,
            cohesion: GranularCohesion::Off,
            pair_models: None,
        }
    }

    /// Return a copy of this model with per-type-pair sub-model coefficients
    /// ([`PairModelTable`]); the model's own `normal`, `tangential`,
    /// `rolling`, `cohesion` then only fix the model *kinds* and the state
    /// layout, and each contact uses the entry of its `(itype, jtype)`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if an entry of the `n x n` table is missing
    /// or names a different model kind than this model (the contact-state
    /// layout is fixed by the kind).
    pub fn with_pair_models(self, table: PairModelTable) -> Result<Self, DemError> {
        use std::mem::discriminant as d;
        for i in 0..table.n_types {
            for j in 0..table.n_types {
                let Some(e) = table.entries[i][j] else {
                    return Err(DemError::InvalidInput(format!(
                        "pair-model table entry ({i}, {j}) is missing"
                    )));
                };
                if d(&e.normal) != d(&self.normal)
                    || d(&e.tangential) != d(&self.tangential)
                    || d(&e.rolling) != d(&self.rolling)
                    || d(&e.cohesion) != d(&self.cohesion)
                {
                    return Err(DemError::InvalidInput(format!(
                        "pair-model entry ({i}, {j}) changes a model kind"
                    )));
                }
            }
        }
        Ok(Self {
            pair_models: Some(table),
            ..self
        })
    }

    /// Return a copy of this model with a per-atom-type [`MaterialTable`]
    /// (upstream's `peratomtype` / `peratomtypepair` properties). The normal
    /// model's own `material` is then ignored for `Y_eff`, `G_eff`, `e` and
    /// `mu`; its `tangential_damping` / `characteristic_velocity` settings
    /// still apply. Atom types are given to the system with
    /// `GranularSystem::with_particle_types`.
    #[must_use]
    pub fn with_materials(self, materials: MaterialTable) -> Self {
        Self {
            materials: Some(materials),
            ..self
        }
    }

    /// Return a copy of this model with the given rolling-resistance model.
    ///
    /// Rolling resistance is what gives a heap a finite angle of repose; a
    /// pebble-bed case almost certainly wants it (see [`RollingModel`]).
    #[must_use]
    pub fn with_rolling(self, rolling: RollingModel) -> Self {
        Self { rolling, ..self }
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
        // Read the prior spring (and mark the contact live) for the model that
        // has one. `NoHistory` must not touch the store at all — touching it
        // would insert a live entry for a stateless contact and change what
        // `end_step` retains.
        let prior = match self.tangential {
            TangentialModel::History | TangentialModel::LudingTn { .. } => history.touch(key),
            TangentialModel::NoHistory => Vec3::zero(),
        };
        let prior_aux = self.load_aux(key, history);
        let (force, updated, aux) = self.resolve_impl_x::<true>(prior, prior_aux, k, dt);
        if let Some(shear) = updated {
            history.set(key, shear);
        }
        self.store_aux(key, history, &aux);
        force
    }

    /// Whether any part of this contact law keeps per-contact state besides
    /// the shear spring (a stateful normal or rolling model); such a model is
    /// resolved serially.
    #[must_use]
    pub fn has_contact_state(&self) -> bool {
        self.normal.has_contact_state() || self.rolling.has_state()
    }

    /// Number of three-value slots of normal-model state.
    fn normal_slots(&self) -> u8 {
        (self.normal.state_len().div_ceil(3)) as u8
    }

    /// Read (and mark live) the auxiliary state of a contact.
    fn load_aux(&self, key: ContactKey, history: &mut ShearHistory) -> ContactAux {
        let mut aux = ContactAux::default();
        for slot in 0..self.normal_slots() {
            let v = history.touch(key.aux_slot(slot));
            let b = 3 * slot as usize;
            aux.normal[b] = v.x;
            aux.normal[b + 1] = v.y;
            aux.normal[b + 2] = v.z;
        }
        if self.rolling.has_state() {
            aux.roll = history.touch(key.aux_slot(ROLL_SLOT));
        }
        aux
    }

    /// Write the auxiliary state of a contact back.
    fn store_aux(&self, key: ContactKey, history: &mut ShearHistory, aux: &ContactAux) {
        for slot in 0..self.normal_slots() {
            let b = 3 * slot as usize;
            history.set(
                key.aux_slot(slot),
                Vec3::new(aux.normal[b], aux.normal[b + 1], aux.normal[b + 2]),
            );
        }
        if self.rolling.has_state() {
            history.set(key.aux_slot(ROLL_SLOT), aux.roll);
        }
    }

    /// Flip the sign-flipping auxiliary records of a pair contact visited the
    /// other way round (the rolling spring torque; see [`ShearHistory::flip`]).
    pub fn flip_aux(&self, key: ContactKey, history: &mut ShearHistory) {
        if self.rolling.has_state() {
            history.flip_aux(key, ROLL_SLOT);
        }
    }

    /// The **pure** core of [`GranularContactModel::resolve`]: the same contact
    /// law with the shear-history store passed in and out by value instead of
    /// being mutated in place.
    ///
    /// Returns the contact force/torque set, and the tangential displacement
    /// `ξ_t` `[m]` the caller must store for this contact — `None` for
    /// [`TangentialModel::NoHistory`], which is stateless and whose contacts
    /// must **not** appear in the store at all.
    ///
    /// # Why this exists
    ///
    /// Contact resolution is the expensive half of a DEM timestep and is
    /// otherwise trivially parallel: distinct contacts touch distinct
    /// [`ContactKey`]s, so the only thing serialising them is the `&mut
    /// ShearHistory` borrow. Splitting the borrow out lets
    /// [`ComputeType::CpuMultiThread`](crate::compute::ComputeType::CpuMultiThread)
    /// evaluate every contact concurrently and then apply the force
    /// accumulation and the history writes in the serial contact order — which
    /// is what makes the parallel backend **bit-identical** to the scalar one
    /// rather than merely close (floating-point addition is not associative,
    /// so the accumulation order, not the arithmetic, is what must be
    /// preserved).
    ///
    /// `prior_shear` is the stored `ξ_t` for this contact, or [`Vec3::zero`]
    /// for a contact seen for the first time — which is exactly what
    /// [`ShearHistory::get`] returning `None` means. Only for a model without
    /// [`GranularContactModel::has_contact_state`].
    #[must_use]
    pub fn resolve_pure(
        &self,
        prior_shear: Vec3,
        k: &ContactKinematics,
        dt: f64,
    ) -> (GranularForce, Option<Vec3>) {
        self.resolve_impl::<true>(prior_shear, k, dt)
    }

    /// The contact law evaluated **without advancing the shear history**:
    /// upstream's `computeflag = 0, shearupdate = 0` re-evaluation, which
    /// `compute pair/gran/local` performs (`PairGran::compute_pgl(0, 0)`). The
    /// stored displacement is used as it stands (no `v_tr*dt` increment, no
    /// projection, nothing written back). `dt` still enters the rolling
    /// spring increment (upstream reads `update->dt` there).
    #[must_use]
    pub fn resolve_frozen(
        &self,
        stored_shear: Vec3,
        k: &ContactKinematics,
        dt: f64,
    ) -> GranularForce {
        self.resolve_impl::<false>(stored_shear, k, dt).0
    }

    /// The set-up evaluation (`shearupdate = 0`): registers the contact in
    /// `history` (live, zero if new) and returns the force from the stored
    /// displacement without advancing it.
    pub fn resolve_setup(
        &self,
        key: ContactKey,
        k: &ContactKinematics,
        history: &mut ShearHistory,
        dt: f64,
    ) -> GranularForce {
        let prior = match self.tangential {
            TangentialModel::History | TangentialModel::LudingTn { .. } => history.touch(key),
            TangentialModel::NoHistory => Vec3::zero(),
        };
        let prior_aux = self.load_aux(key, history);
        let (force, _, aux) = self.resolve_impl_x::<false>(prior, prior_aux, k, dt);
        // upstream: the normal model's unconditional history writes
        // (`if (deltan > history[0]) history[0] = deltan;`) run at set-up too;
        // the rolling spring is gated on `shearupdate` and comes back unchanged
        self.store_aux(key, history, &aux);
        force
    }

    fn resolve_impl<const UPDATE: bool>(
        &self,
        prior_shear: Vec3,
        k: &ContactKinematics,
        dt: f64,
    ) -> (GranularForce, Option<Vec3>) {
        let (f, u, _) = self.resolve_impl_x::<UPDATE>(prior_shear, ContactAux::default(), k, dt);
        (f, u)
    }

    /// [`GranularContactModel::resolve_impl`] with the per-contact scalar of a
    /// stateful normal model (`deltaMax`) passed in and out.
    fn resolve_impl_x<const UPDATE: bool>(
        &self,
        prior_shear: Vec3,
        prior_aux: ContactAux,
        k: &ContactKinematics,
        dt: f64,
    ) -> (GranularForce, Option<Vec3>, ContactAux) {
        if let Some(table) = &self.pair_models {
            // Upstream indexes every `peratomtypepair` coefficient by
            // `[itype][jtype]`; resolve with this pair's sub-models.
            let e = table.entries[k.itype][k.jtype]
                .expect("pair-model table checked complete by with_pair_models");
            let pair = Self {
                normal: e.normal,
                tangential: e.tangential,
                rolling: e.rolling,
                cohesion: e.cohesion,
                materials: self.materials,
                pair_models: None,
            };
            return pair.resolve_impl_x::<UPDATE>(prior_shear, prior_aux, k, dt);
        }
        let props = match self.materials {
            Some(table) => table.pair_props(k.itype, k.jtype),
            None => PairProps::from_material(&self.normal.material()),
        };
        let mut aux = prior_aux;
        let mut normal = self
            .normal
            .evaluate_stateful(k, &props, &mut aux.normal, UPDATE);
        // cohesion (between the normal and the tangential model upstream)
        let fn_own = normal.fn_scalar;
        // liquid-bridge models: `surfacesIntersect` of the capillary models
        // (`None`: dry, or bond volume below 1e-14 -> upstream returns early)
        let cap = match self.cohesion {
            GranularCohesion::Capillary(p) => p.intersect(k),
            _ => None,
        };
        let (f_coh, coh_reduce) = match (&cap, self.cohesion) {
            (Some(c), GranularCohesion::Capillary(p)) => (c.f_n, p.tangential_reduce),
            _ => self.cohesion.normal_force(k),
        };
        if coh_reduce {
            normal.fn_scalar += f_coh;
        }
        let mu = props.friction;
        let mut updated: Option<Vec3> = None;

        let ft = match self.tangential {
            TangentialModel::History => {
                // 1. accumulate and re-project onto the tangent plane
                let mut shear = prior_shear;
                if UPDATE {
                    shear = prior_shear.add(k.vtr.scale(dt));
                    let rsht = shear.dot(k.en);
                    shear = shear.sub(k.en.scale(rsht));
                }

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
                        // Upstream: shear[i] = -Ft_i / kt (negate, then divide).
                        // `ft * (-1/kt)` rounds differently.
                        updated = Some(Vec3::new(-ft.x / kt, -ft.y / kt, -ft.z / kt));
                    } else {
                        ft = Vec3::zero();
                        updated = Some(shear);
                    }
                } else {
                    // 4. sticking: keep the updated displacement, add damping
                    updated = Some(shear);
                    ft = ft.sub(k.vtr.scale(normal.gammat));
                }
                ft
            }
            TangentialModel::LudingTn {
                mu_dynamic_ratio,
                fric_visc,
                kt_ratio,
            } => {
                // tangential_model_luding_tn.h, surfacesIntersect
                let kt = normal.kt * kt_ratio;
                let mut shear = prior_shear;
                if UPDATE {
                    shear = prior_shear.add(k.vtr.scale(dt));
                    let rsht = shear.dot(k.en);
                    shear = shear.sub(k.en.scale(rsht));
                }
                let shrmag = shear.norm();
                let xmu_s = mu;
                let xmu_d = xmu_s * mu_dynamic_ratio;
                let gammat = normal.gammat * fric_visc;
                let (kc, f_adh) = self.normal.kc_fo(&aux.normal);
                let mut ft = shear.scale(-kt);
                let ft_shear = kt * shrmag;
                let cone = (normal.fn_scalar + kc * k.delta_n - f_adh).abs();
                let ft_friction_s = xmu_s * cone;
                if ft_shear > ft_friction_s {
                    if shrmag != 0.0 {
                        let ft_friction_d = xmu_d * cone;
                        let ratio = ft_friction_d / ft_shear;
                        ft = ft.scale(ratio);
                        updated = Some(Vec3::new(-ft.x / kt, -ft.y / kt, -ft.z / kt));
                    } else {
                        ft = Vec3::zero();
                        updated = Some(shear);
                    }
                } else {
                    updated = Some(shear);
                    ft = Vec3::new(
                        ft.x - gammat * k.vtr.x,
                        ft.y - gammat * k.vtr.y,
                        ft.z - gammat * k.vtr.z,
                    );
                }
                ft
            }
            TangentialModel::NoHistory => {
                // Upstream `tangential_model_no_history.h`: a dashpot whose
                // coefficient is capped, not a force that is rescaled:
                //   vrel = |vtr|; Ft_friction = mu*|Fn|
                //   gamma = (Ft_friction < gammat*vrel) ? Ft_friction/vrel : gammat
                //   Ft = -gamma * vtr
                let vrel = k.vtr.norm();
                let ft_friction = mu * normal.fn_scalar.abs();
                let gamma = if ft_friction < normal.gammat * vrel {
                    ft_friction / vrel
                } else {
                    normal.gammat
                };
                k.vtr.scale(-gamma)
            }
        };

        // Normal force acts on i along +en; tangential force is Ft on i.
        // upstream: `delta_F = Fn*en`, then `+= Fn_coh*en` (cohesion), then
        // `+= Ft`; the Fn used for the force is the normal model's own
        let mut force_i = k.en.scale(fn_own);
        if let Some(c) = &cap {
            // capillary: `fx = (Fcap + Fvisc) * en + Ft_visc`, added as one term
            force_i = force_i.add(k.en.scale(f_coh).add(c.ft));
        } else if !matches!(
            self.cohesion,
            GranularCohesion::Off | GranularCohesion::Capillary(_)
        ) {
            force_i = force_i.add(k.en.scale(f_coh));
        }
        let force_i = force_i.add(ft);
        let force_j = force_i.scale(-1.0);

        // Upstream: tor = en x Ft ; torque_i = -cri*tor ; torque_j = -crj*tor.
        let tor = k.en.cross(ft);
        // `delta_torque` starts at +0 (`memset`) and the tangential model adds to it, so a
        // zero tangential force gives +0, never -0 (gh:#1418, wall/gran/local dump).
        // (the capillary models' viscous torque is added first: the cohesion
        // model runs between the normal and the tangential model)
        let (base_i, base_j) = match &cap {
            Some(c) => (
                Vec3::zero().add(c.tor.scale(-k.cri)),
                Vec3::zero().add(c.tor.scale(-k.crj)),
            ),
            None => (Vec3::zero(), Vec3::zero()),
        };
        let mut torque_i = base_i.add(tor.scale(-k.cri));
        let mut torque_j = base_j.add(tor.scale(-k.crj));

        // Rolling resistance: upstream applies -M_r to i and +M_r to j.
        let mut m_tor_out: Option<Vec3> = None;
        let m_r = if self.rolling.has_state() {
            let (m, m_tor, hist) = self.rolling.stateful_torque(
                k,
                &normal,
                self.normal.kc_fo(&aux.normal),
                aux.roll,
                dt,
                UPDATE,
            );
            aux.roll = hist;
            m_tor_out = m_tor;
            m
        } else {
            self.rolling
                .rolling_torque(k, &normal, k.omega_i, k.omega_j, k.is_wall)
        };
        torque_i = torque_i.sub(m_r);
        torque_j = torque_j.add(m_r);
        if let Some(m_tor) = m_tor_out {
            // Luding `torsion on`, wall contact: a second, separate torque.
            torque_i = torque_i.sub(m_tor);
            torque_j = torque_j.add(m_tor);
        }

        (
            GranularForce {
                force_i,
                force_j,
                torque_i,
                torque_j,
                fn_scalar: fn_own,
                ft,
            },
            updated,
            aux,
        )
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
        let ln_e = 0.9_f64.r_ln();
        assert_abs_diff_eq!(
            m.beta_eff(),
            ln_e / (ln_e * ln_e + std::f64::consts::PI.r_powi(2)).sqrt(),
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
            * (15.0 * k.m_eff * 4.0 / (16.0 * sqrtval * m.y_eff())).r_powf(0.2);
        let ln_e = 0.9_f64.r_ln();
        let gamman_hand = (4.0 * k.m_eff * kn_hand * ln_e * ln_e
            / (ln_e * ln_e + std::f64::consts::PI.r_powi(2)))
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

    /// **Methodology.** The CDT rolling torque must (a) scale with the
    /// **elastic** normal force `k_n·δ_n`, not the damped `|F_n|`; (b) oppose
    /// the relative rolling direction; (c) have **no component along the
    /// contact normal** with `torsion_torque` off (upstream's default). Set up
    /// an overlapping pair with `ω_i = −ω_j = 20 ŷ` and an approach velocity so
    /// that `|F_n| ≠ k_n·δ_n`, then hand-check.
    ///
    /// **Result (2026-09-15).** `|M_r| = µ_r·k_n·δ_n·R*` to `1e-15` relative;
    /// direction is `+ŵ_r` (the caller negates it for `i`); `M_r · ê_n = 0` to
    /// `1e-18`. Using `|F_n|` instead would have given a torque `1.21x` larger
    /// here, which is exactly the divergence `crate::rolling` carries.
    #[test]
    fn cdt_rolling_torque_uses_the_elastic_normal_force_and_drops_torsion() {
        let m = mat();
        let mut a = sphere(-0.0049, 1.0);
        let b = sphere(0.0049, -1.0);
        a.angular_velocity = Vec3::new(0.0, 20.0, 0.0);
        let mut k = ContactKinematics::pair(&a, &b).expect("in contact");
        k.omega_j = Vec3::new(0.0, -20.0, 0.0);
        let normal = GranularNormalModel::hertz(m).evaluate(&k);

        let mu_r = 0.1;
        let model = RollingModel::cdt(mu_r).unwrap();
        let m_r = model.rolling_torque(&k, &normal, k.omega_i, k.omega_j, false);

        let expected_mag = mu_r * normal.kn * k.delta_n * k.r_eff;
        assert_abs_diff_eq!(m_r.norm(), expected_mag, epsilon = expected_mag * 1e-15);
        // No torsion component (torsion_torque defaults off).
        assert_abs_diff_eq!(m_r.dot(k.en), 0.0, epsilon = 1e-18);
        // Direction follows the relative rolling velocity.
        assert!(m_r.y > 0.0, "torque must follow +w_r, got {m_r:?}");

        // The damped normal force is materially different here — this is the
        // quantity `crate::rolling` uses by mistake.
        let damped = mu_r * normal.fn_scalar.abs() * k.r_eff;
        assert!(
            (damped / expected_mag - 1.0).abs() > 0.1,
            "the test should exercise a case where |F_n| differs from kn*delta"
        );

        assert!(RollingModel::cdt(-0.1).is_err());
        assert!(RollingModel::cdt(f64::NAN).is_err());
    }

    /// **Methodology.** `RollingModel::Off` must produce exactly zero torque,
    /// and a non-rolling pair (`ω_i = ω_j`) must feel no rolling resistance
    /// whatever `µ_r` is — otherwise a settled bed would be spun by its own
    /// rolling model.
    ///
    /// **Result (2026-09-15).** Both are exactly `0 N·m`. With
    /// `torsion_torque` enabled the normal component is retained, confirming
    /// the switch is live.
    #[test]
    fn rolling_is_zero_when_off_or_not_rolling() {
        let m = mat();
        let mut a = sphere(-0.0049, 0.0);
        let b = sphere(0.0049, 0.0);
        a.angular_velocity = Vec3::new(0.0, 7.0, 0.0);
        let mut k = ContactKinematics::pair(&a, &b).expect("in contact");
        let normal = GranularNormalModel::hertz(m).evaluate(&k);

        // Off.
        let off = RollingModel::Off.rolling_torque(&k, &normal, k.omega_i, k.omega_j, false);
        assert_abs_diff_eq!(off.norm(), 0.0, epsilon = 1e-18);

        // Rolling together: w_r = 0.
        k.omega_j = k.omega_i;
        let none = RollingModel::cdt(0.5)
            .unwrap()
            .rolling_torque(&k, &normal, k.omega_i, k.omega_j, false);
        assert_abs_diff_eq!(none.norm(), 0.0, epsilon = 1e-18);

        // torsion_torque on retains the normal component.
        k.omega_j = Vec3::zero();
        k.omega_i = Vec3::new(3.0, 4.0, 0.0); // has a component along en (x)
        let with_torsion = RollingModel::Cdt {
            mu_r: 0.1,
            torsion_torque: true,
        }
        .rolling_torque(&k, &normal, k.omega_i, k.omega_j, false);
        assert!(
            with_torsion.dot(k.en).abs() > 1e-12,
            "torsion_torque=true must keep the normal component"
        );
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

    #[test]
    fn material_table_mixes_by_the_global_properties_formulas() {
        let t = MaterialTable::new(
            &[(1.0e7, 0.3), (3.0e7, 0.2)],
            &[&[0.9, 0.7], &[0.7, 0.5]],
            &[&[0.5, 0.3], &[0.3, 0.2]],
        )
        .unwrap();
        assert_eq!(t.n_types(), 2);
        let p = t.pair_props(0, 1);
        // createYeff / createGeff, evaluated by hand.
        let y = 1.0 / ((1.0 - 0.09) / 1.0e7 + (1.0 - 0.04) / 3.0e7);
        let g = 1.0 / (2.0 * 1.7 * 1.3 / 1.0e7 + 2.0 * 1.8 * 1.2 / 3.0e7);
        assert_abs_diff_eq!(p.y_eff, y, epsilon = 1e-6);
        assert_abs_diff_eq!(p.g_eff, g, epsilon = 1e-6);
        assert_eq!(p.friction, 0.3);
        assert_eq!(t.pair_props(1, 0), p, "the table is symmetric");
        // Same type twice reduces to the single-material formulas.
        let same = t.pair_props(0, 0);
        let m = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap();
        assert_eq!(same, PairProps::from_material(&m));
    }

    #[test]
    fn material_table_rejects_bad_shapes_and_values() {
        let ok = [&[0.9][..]];
        let mu = [&[0.5][..]];
        assert!(MaterialTable::new(&[], &[], &[]).is_err());
        assert!(MaterialTable::new(&[(1.0e7, 0.3); MAX_TYPES + 1], &[], &[]).is_err());
        assert!(MaterialTable::new(&[(1.0e7, 0.3), (1.0e7, 0.3)], &ok, &mu).is_err());
        let asym = [&[0.9, 0.7][..], &[0.6, 0.9][..]];
        let sym = [&[0.5, 0.3][..], &[0.3, 0.2][..]];
        assert!(MaterialTable::new(&[(1.0e7, 0.3), (1.0e7, 0.3)], &asym, &sym).is_err());
        assert!(MaterialTable::new(&[(-1.0, 0.3)], &ok, &mu).is_err());
        let bad_e = [&[0.01][..]];
        assert!(MaterialTable::new(&[(1.0e7, 0.3)], &bad_e, &mu).is_err());
    }

    #[test]
    fn contact_types_select_the_table_entry() {
        let a = sphere(0.0, 0.0);
        let b = sphere(0.0099, 0.0);
        let k = ContactKinematics::pair(&a, &b).unwrap();
        assert_eq!((k.itype, k.jtype), (0, 0));
        let k = k.with_types(1, 0);
        assert_eq!((k.itype, k.jtype), (1, 0));
    }
}
