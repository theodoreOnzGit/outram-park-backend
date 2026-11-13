// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/cohesion_model_easo_capillary_viscous.h
//                      (`CohesionModel<COHESION_EASO_CAPILLARY_VISCOUS>`),
//                      src/cohesion_model_washino_capillary_viscous.h
//                      (`CohesionModel<COHESION_WASHINO_CAPILLARY_VISCOUS>`),
//                      src/fix_scalar_transport_equation.cpp
//                      (`initial_integrate`, `advanceQtyExplicit`, the
//                      `liquidtransfer` fix these models create),
//                      src/pair_gran_base.h (the `surfacesClose` dispatch)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2014- DCS Computing GmbH, Linz.
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

//! # Liquid-bridge cohesion: `easo/capillary/viscous` and
//! `washino/capillary/viscous` (gh:#1411)
//!
//! Both models exist in LIGGGHTS-PUBLIC `3d5c00f2` (checked: the files
//! `cohesion_model_easo_capillary_viscous.h` and
//! `cohesion_model_washino_capillary_viscous.h` are present, with
//! `COHESION_MODEL(..., easo/capillary/viscous, 8)` and
//! `COHESION_MODEL(..., washino/capillary/viscous, 7)`). Nothing here is
//! invented: every formula is the upstream line, in upstream's operation order
//! (so the result can be bit-identical, see
//! `tests/capillary_cross_code.rs`).
//!
//! ## What a liquid bridge is here
//!
//! Each particle carries a **surface liquid content** `L` (a volume fraction of
//! the particle, `surfaceLiquidContent`) and a **liquid flux** `Phi`
//! (`liquidFlux`, volume fraction per second). A bridge between `i` and `j`
//! has the volume
//!
//! ```text
//!   easo:     V_b = (1/2) V_Li (1 - sqrt(1 - r_j^2/(r_i+r_j)^2))
//!                 + (1/2) V_Lj (1 - sqrt(1 - r_i^2/(r_i+r_j)^2))
//!   washino:  V_b = f_vol (V_Li + V_Lj)          (f_vol = 0.05 by default)
//! ```
//!
//! and exerts a capillary force plus a viscous (lubrication) force:
//!
//! * **in contact** (`surfacesIntersect`): Easo uses Soulie et al. (2006),
//!   `F_c = -pi gamma sqrt(r_i r_j) (exp(B) + C)`; Washino uses Rabinovich et
//!   al. (2005), `F_c = -2 pi R* gamma cos(theta) (1 - s sqrt(pi R*/(2 V_b)))`;
//! * **out of contact** (`surfacesClose`, inside the neighbour list's contact
//!   distance factor): while the bridge exists and `s < min(s_max, (max_sep -
//!   1)(r_i + r_j))`, Easo's Soulie form with the separation term
//!   `exp(A s/R2 + B)`, Washino's Rabinovich form (Langmuir 21 (24), 2005,
//!   Eqns. 19, 20, A3);
//! * **viscous** (both): normal `F = -6 pi mu R* v_n / max(eps_min, s/R*)` and
//!   tangential `-6 pi mu R* (8/15 ln(1/max(eps_min, s/R*)) + 0.9588) v_tr`
//!   (Nase et al., cited in Shi and McCarthy, 2008), with the matching torque.
//!
//! When a bridge ruptures its liquid is redistributed to the two particles as
//! a flux (`split = 1/(1 + (r_j/r_i)^3)` of the bridge volume to `i`), and the
//! `liquidtransfer` scalar transport equation integrates `L += Phi dt` at
//! `final_integrate` (flux reset at `initial_integrate`).
//!
//! ## Differences between the two (as upstream has them)
//!
//! * Easo only creates a bridge at contact (`contflag` is set in
//!   `surfacesIntersect`); Washino also **creates** one out of contact whenever
//!   `s < d_max` (its `surfacesClose` has no early return on `contflag`).
//! * Washino has `limitLiquidContent` (clip `L` at the per-type
//!   `maxLiquidContent`, and overwrite `maxSeparationDistanceRatio` from it) and
//!   `modifyLbVolume` (`lbVolumeFraction`); Easo has neither.
//! * ~~Washino's wall branch (mesh `LiquidContent`) is not ported.~~
//!   **CORRECTED 2026-11-12 (gh:#1487):** both wall branches are ported, see
//!   below.
//!
//! ## Particle-wall bridges (gh:#1487)
//!
//! Translated from the `is_wall` branches of both headers, with the wall data
//! upstream's `FixWallGranBase::compute_force` (`fix_wall_gran_base.h`) hands
//! them: `radsum = radi`, `r = radi - deltan`, `area_ratio = 1`, and
//! `sidata.radj` **never set, so `0.0`** (`SurfacesCloseData()`).
//!
//! * **Easo** (upstream warns "only supports dry walls"): `radj = radi`,
//!   `V_Lj = 0`, so `V_b = 0.5 V_Li (1 - sqrt(1 - radi^2/radi^2)) = 0.5 V_Li`;
//!   a rupture at a wall moves no liquid.
//! * **Washino**: the wall's liquid volume is
//!   `V_Lj = LC_tri min(pi r^2, A_tri) 1000 t_wall` on a mesh carrying
//!   `fix mesh/surface/liquidtransfer` ([`MeshLiquid`]), else `0`. In contact
//!   upstream reads the raw `radj = 0`, so `rEff = 0` and the capillary and
//!   viscous wall forces are signed zeros (only the bridge flag and
//!   `limitLiquidContent` act); out of contact `radj = radi`. That asymmetry is
//!   upstream's and is reproduced, not repaired. A rupture on a liquid mesh
//!   moves `volFlux = (0.5 V_b - f V_Li)/dt` to the particle and
//!   `-volFlux/(1000 t A)` to the facet.
//!
//! A wall contact is only visited while `deltan < (cdf - 1) radius`
//! (`FixWallGran::post_force_*`); beyond that the history (the bridge flag) is
//! zeroed without a rupture flux, exactly as upstream.
//!
//! ## Not ported (honestly)
//!
//! * ~~**Walls.** Both upstream models also handle particle-wall contacts (Easo:
//!   dry walls only, with a warning; Washino: mesh liquid content through
//!   `MeshModuleLiquidTransfer`). [`crate::granular_system::GranularSystem::with_capillary`]
//!   refuses a system with walls rather than silently dropping those bridges.~~
//!   **CORRECTED 2026-11-12 (gh:#1487):** ported (section above). Still not
//!   representable: a wall atom type different from its particles' (the crate
//!   resolves every wall contact with the particle's own type), non-axis-aligned
//!   or one-sided primitive walls, and moving analytic walls;
//!   `with_capillary` refuses those.
//! * The pair-style `tangential_reduce` is ported; coarse graining is an
//!   upstream error (`error->cg`) and is not representable here.

use crate::granular::{ContactKinematics, MAX_TYPES};
use crate::particle::Vec3;
use crate::DemError;
use petir::mathf::RealMath;
use std::f64::consts::PI;

/// `4/3 * 1000` as upstream writes it (a truncated literal, not `4000/3`).
const FOUR_THIRDS_1000: f64 = 1333.333333;
/// `2 * 4/3 * 1000` (Washino's `limit_lqc_flag_` branch).
const TWO_FOUR_THIRDS_1000: f64 = 2666.666666;
/// `8/15` as upstream writes it.
const EIGHT_FIFTEENTHS: f64 = 0.5333333;
/// `skip if bond volume too small` (upstream `1e-14`).
const SMALL_BOND_VOLUME: f64 = 1e-14;

/// Which of the two upstream capillary models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapillaryKind {
    /// `cohesion_model easo/capillary/viscous`.
    Easo,
    /// `cohesion_model washino/capillary/viscous`.
    Washino,
}

/// The settings and material coefficients of a capillary cohesion model
/// (upstream's `fix property/global` coefficients and model keywords).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapillaryParams {
    /// Which model.
    pub kind: CapillaryKind,
    /// `surfaceTension` `[N/m]`.
    pub surface_tension: f64,
    /// `fluidViscosity` `[Pa s]`.
    pub fluid_viscosity: f64,
    /// `contactAngle`, per atom type `[rad]` (a LIGGGHTS deck gives degrees and upstream stores `deg * PI / 180`; a bridge uses the mean of its two
    /// particles').
    pub contact_angle: [f64; MAX_TYPES],
    /// `minSeparationDistanceRatio` `[-]` (lubrication cut-off, `> 0`).
    pub min_separation_ratio: f64,
    /// `maxSeparationDistanceRatio` `[-]`: a bridge always breaks beyond
    /// `(ratio - 1)(r_i + r_j)` surface distance (`>= 1`).
    pub max_separation_ratio: f64,
    /// `surfaceLiquidContentInitial`: the liquid content every particle starts
    /// with (volume fraction of the particle).
    pub initial_liquid_content: f64,
    /// Upstream `tangential_reduce` (default off): also add the bridge's normal
    /// force to `F_n`, which the tangential and rolling models read.
    pub tangential_reduce: bool,
    /// Washino `limitLiquidContent` (default off).
    pub limit_liquid_content: bool,
    /// Washino `maxLiquidContent`, per atom type (read only with
    /// `limit_liquid_content`).
    pub max_liquid_content: [f64; MAX_TYPES],
    /// Washino bridge volume fraction (`volumeFraction`, upstream default
    /// `0.05`; replaced by `lbVolumeFraction` under `modifyLbVolume on`).
    pub volume_fraction: f64,
}

impl CapillaryParams {
    fn base(
        kind: CapillaryKind,
        surface_tension: f64,
        fluid_viscosity: f64,
        contact_angle: f64,
        min_separation_ratio: f64,
        max_separation_ratio: f64,
        initial_liquid_content: f64,
    ) -> Result<Self, DemError> {
        let bad = |name: &str, v: f64| {
            Err(DemError::InvalidInput(format!(
                "capillary cohesion: {name} must be finite and >= 0, got {v}"
            )))
        };
        for (name, v) in [
            ("surfaceTension", surface_tension),
            ("fluidViscosity", fluid_viscosity),
            ("contactAngle", contact_angle),
            ("surfaceLiquidContentInitial", initial_liquid_content),
        ] {
            if !(v >= 0.0) || !v.is_finite() {
                return bad(name, v);
            }
        }
        if !(min_separation_ratio > 0.0) || !min_separation_ratio.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "capillary cohesion: minSeparationDistanceRatio must be finite and > 0 \
                 (its log is taken), got {min_separation_ratio}"
            )));
        }
        // Easo: `maxSeparationDistanceRatio < 1.0` is `error->one`. Washino has
        // no check upstream, but a ratio below 1 makes every bridge break at
        // once, so it is refused for both.
        if !(max_separation_ratio >= 1.0) || !max_separation_ratio.is_finite() {
            return Err(DemError::InvalidInput(format!(
                "capillary cohesion requires maxSeparationDistanceRatio >= 1.0, got {max_separation_ratio}"
            )));
        }
        Ok(Self {
            kind,
            surface_tension,
            fluid_viscosity,
            contact_angle: [contact_angle; MAX_TYPES],
            min_separation_ratio,
            max_separation_ratio,
            initial_liquid_content,
            tangential_reduce: false,
            limit_liquid_content: false,
            max_liquid_content: [0.0; MAX_TYPES],
            volume_fraction: 0.05,
        })
    }

    /// `cohesion_model easo/capillary/viscous` with one `contactAngle` for every
    /// atom type and upstream's defaults (`tangential_reduce off`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a negative or non-finite coefficient, a
    /// non-positive `min_separation_ratio` or `max_separation_ratio < 1`.
    pub fn easo(
        surface_tension: f64,
        fluid_viscosity: f64,
        contact_angle: f64,
        min_separation_ratio: f64,
        max_separation_ratio: f64,
        initial_liquid_content: f64,
    ) -> Result<Self, DemError> {
        Self::base(
            CapillaryKind::Easo,
            surface_tension,
            fluid_viscosity,
            contact_angle,
            min_separation_ratio,
            max_separation_ratio,
            initial_liquid_content,
        )
    }

    /// `cohesion_model washino/capillary/viscous` with upstream's defaults
    /// (`limitLiquidContent off`, `modifyLbVolume off`, bridge volume fraction
    /// `0.05`, `tangential_reduce off`).
    ///
    /// # Errors
    ///
    /// As [`CapillaryParams::easo`].
    pub fn washino(
        surface_tension: f64,
        fluid_viscosity: f64,
        contact_angle: f64,
        min_separation_ratio: f64,
        max_separation_ratio: f64,
        initial_liquid_content: f64,
    ) -> Result<Self, DemError> {
        Self::base(
            CapillaryKind::Washino,
            surface_tension,
            fluid_viscosity,
            contact_angle,
            min_separation_ratio,
            max_separation_ratio,
            initial_liquid_content,
        )
    }

    /// Upstream's `tangential_reduce` switch.
    #[must_use]
    pub fn with_tangential_reduce(mut self, on: bool) -> Self {
        self.tangential_reduce = on;
        self
    }

    /// Per-atom-type contact angles (`peratomtype`).
    #[must_use]
    pub fn with_contact_angles(mut self, angles: [f64; MAX_TYPES]) -> Self {
        self.contact_angle = angles;
        self
    }

    /// Washino `limitLiquidContent on` with the per-type `maxLiquidContent`
    /// (each in `[0, 1]`, upstream `createPerTypeProperty(..., 0.0, 1.0)`).
    /// `maxSeparationDistanceRatio` is then overwritten at attach time
    /// ([`CapillaryParams::limited_max_separation_ratio`]).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for the Easo model (it has no such option) or
    /// a content outside `[0, 1]`.
    pub fn with_liquid_content_limit(mut self, max_content: [f64; MAX_TYPES]) -> Result<Self, DemError> {
        if self.kind != CapillaryKind::Washino {
            return Err(DemError::InvalidInput(
                "limitLiquidContent exists only on washino/capillary/viscous".into(),
            ));
        }
        if max_content.iter().any(|c| !(0.0..=1.0).contains(c)) {
            return Err(DemError::InvalidInput(
                "maxLiquidContent must lie in [0, 1]".into(),
            ));
        }
        self.limit_liquid_content = true;
        self.max_liquid_content = max_content;
        Ok(self)
    }

    /// Washino `modifyLbVolume on` with `lbVolumeFraction` in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for the Easo model or a fraction outside `[0, 1]`.
    pub fn with_lb_volume_fraction(mut self, fraction: f64) -> Result<Self, DemError> {
        if self.kind != CapillaryKind::Washino {
            return Err(DemError::InvalidInput(
                "modifyLbVolume exists only on washino/capillary/viscous".into(),
            ));
        }
        if !(0.0..=1.0).contains(&fraction) {
            return Err(DemError::InvalidInput(
                "lbVolumeFraction must lie in [0, 1]".into(),
            ));
        }
        self.volume_fraction = fraction;
        Ok(self)
    }

    /// Washino `limitLiquidContent` overwrites `maxSeparationDistanceRatio`
    /// (`connectToProperties`): `1 + max_i 0.5 d_max,i / r_min` with
    /// `d_max,i = (1 + theta_i/2) cbrt(V_b,i) / 10`,
    /// `V_b,i = 2 (4/3)(1000) pi r_max^3 L_max,i f_vol`, over the first
    /// `n_types` atom types. Returns the ratio unchanged for any other setting.
    #[must_use]
    pub fn limited_max_separation_ratio(&self, n_types: usize, max_rad: f64, min_rad: f64) -> f64 {
        if !(self.kind == CapillaryKind::Washino && self.limit_liquid_content) {
            return self.max_separation_ratio;
        }
        let mut max_dist_ratio = 0.0_f64;
        for i in 0..n_types.min(MAX_TYPES) {
            let vol_l1000 = TWO_FOUR_THIRDS_1000 * PI * max_rad * max_rad * max_rad * self.max_liquid_content[i];
            let vol_bond1000 = vol_l1000 * self.volume_fraction;
            let contact_angle_i = self.contact_angle[i];
            let dist_max = (1. + 0.5 * contact_angle_i) * vol_bond1000.r_cbrt() * 0.1;
            max_dist_ratio = (0.5 * dist_max / min_rad).max(max_dist_ratio);
        }
        1.0 + max_dist_ratio
    }

    /// `neighbor->register_contact_dist_factor`: `1.1 * maxSeparationDistanceRatio`
    /// (both models), for the given (possibly overwritten) maximum ratio.
    #[must_use]
    pub fn contact_distance_factor(max_separation_ratio: f64) -> f64 {
        max_separation_ratio * 1.1
    }

    /// `ln(1/minSeparationDistanceRatio)`, evaluated once at connect time upstream.
    fn ln_one_over_min(&self) -> f64 {
        (1. / self.min_separation_ratio).r_ln()
    }

    /// Washino `limitLiquidContent`: the content clipped at the type's maximum
    /// (`limitLiquidContent`: `if (c > max) c = max`). Identity for Easo or
    /// without the option.
    #[must_use]
    pub fn limit_content(&self, content: f64, atype: usize) -> f64 {
        if self.kind == CapillaryKind::Washino
            && self.limit_liquid_content
            && content > self.max_liquid_content[atype]
        {
            self.max_liquid_content[atype]
        } else {
            content
        }
    }

    /// The bridge volume `V_b * 1000` and the two per-particle shares
    /// `(V_Li,bond, V_Lj,bond) * 1000` (Easo; Washino has no shares and returns
    /// zeros) for particles of radii `radi`, `radj` and liquid contents
    /// `li`, `lj`. Upstream's `volLi1000` etc.
    fn bond_volumes(&self, radi: f64, radj: f64, radsum: f64, li: f64, lj: f64) -> BondVolumes {
        let vol_li1000 = FOUR_THIRDS_1000 * PI * radi * radi * radi * li;
        let vol_lj1000 = FOUR_THIRDS_1000 * PI * radj * radj * radj * lj;
        self.bond_volumes_from(radi, radj, radsum, vol_li1000, vol_lj1000)
    }

    /// [`Self::bond_volumes`] from the two liquid volumes `* 1000` directly
    /// (a wall's `volLj1000` is not a sphere's: `0` for a dry wall, the mesh
    /// facet's film for Washino, gh:#1487).
    fn bond_volumes_from(&self, radi: f64, radj: f64, radsum: f64, vol_li1000: f64, vol_lj1000: f64) -> BondVolumes {
        match self.kind {
            CapillaryKind::Easo => {
                let vol_li_bond1000 = 0.5 * vol_li1000 * (1. - (1. - radj * radj / (radsum * radsum)).sqrt());
                let vol_lj_bond1000 = 0.5 * vol_lj1000 * (1. - (1. - radi * radi / (radsum * radsum)).sqrt());
                BondVolumes {
                    vol_li1000,
                    vol_lj1000,
                    vol_li_bond1000,
                    vol_lj_bond1000,
                    vol_bond1000: vol_li_bond1000 + vol_lj_bond1000,
                }
            }
            CapillaryKind::Washino => BondVolumes {
                vol_li1000,
                vol_lj1000,
                vol_li_bond1000: 0.0,
                vol_lj_bond1000: 0.0,
                vol_bond1000: (vol_li1000 + vol_lj1000) * self.volume_fraction,
            },
        }
    }

    /// Viscous normal and tangential prefactors: `(stokes, ln-term)` for a
    /// separation surface distance `dist` (`None` in contact, where upstream
    /// uses the fixed `minSeparationDistanceRatio`).
    fn viscous(&self, r_eff: f64, vn: f64, dist: Option<f64>) -> (f64, f64) {
        let stokes_pre_factor = -6. * PI * self.fluid_viscosity * r_eff;
        match dist {
            None => {
                let f_visc_n = stokes_pre_factor * vn / self.min_separation_ratio;
                let f_visc_t_over_vt = (EIGHT_FIFTEENTHS * self.ln_one_over_min() + 0.9588) * stokes_pre_factor;
                (f_visc_n, f_visc_t_over_vt)
            }
            Some(dist) => {
                let m = self.min_separation_ratio.max(dist / r_eff);
                let f_visc_n = stokes_pre_factor * vn / m;
                let f_visc_t_over_vt = (EIGHT_FIFTEENTHS * (1. / m).r_ln() + 0.9588) * stokes_pre_factor;
                (f_visc_n, f_visc_t_over_vt)
            }
        }
    }

    /// `surfacesIntersect` of the capillary models for an overlapping pair or
    /// a particle-wall contact (`k.is_wall`, gh:#1487).
    ///
    /// `k.liquid_i`, `k.liquid_j` must hold the particles' (already limited)
    /// `surfaceLiquidContent`; for a wall, `k.wall_liquid_vol1000` holds the
    /// facet's liquid volume `* 1000` (Washino on a `liquidtransfer` mesh, else
    /// `0`). Returns `None` when upstream returns early (bond volume
    /// `< 1e-14`). The caller has already set the bridge flag (`contflag = 1`),
    /// which upstream does before the early return.
    ///
    /// On a wall upstream has `radsum = radi`; Easo sets `radj = radi` and a dry
    /// wall (`volLj = 0`), Washino reads the unset `sidata.radj = 0` (so
    /// `rEff = 0`: the wall forces in contact are signed zeros) and
    /// `dist = r - radi`.
    #[must_use]
    pub fn intersect(&self, k: &ContactKinematics) -> Option<CapillaryIntersect> {
        let radi = k.rad_i;
        let (radj, radsum) = if k.is_wall {
            match self.kind {
                CapillaryKind::Easo => (radi, radi),
                CapillaryKind::Washino => (k.rad_j, radi),
            }
        } else {
            (k.rad_j, k.rad_i + k.rad_j)
        };
        let vol_li1000 = FOUR_THIRDS_1000 * PI * radi * radi * radi * k.liquid_i;
        let vol_lj1000 = if k.is_wall {
            match self.kind {
                CapillaryKind::Easo => 0.0,
                CapillaryKind::Washino => k.wall_liquid_vol1000,
            }
        } else {
            FOUR_THIRDS_1000 * PI * radj * radj * radj * k.liquid_j
        };
        let bv = self.bond_volumes_from(radi, radj, radsum, vol_li1000, vol_lj1000);
        // skip if bond volume too small
        if bv.vol_bond1000 < SMALL_BOND_VOLUME {
            return None;
        }
        let r_eff = radi * radj / (radi + radj);
        let contact_angle_eff = 0.5 * (self.contact_angle[k.itype] + self.contact_angle[k.jtype]);

        let f_capilary = match self.kind {
            CapillaryKind::Easo => {
                // Soulie et al. (2006), Eqn. 13, 14; separation distance = 0
                let r2 = if radi >= radj { radi } else { radj };
                let r2_inv = 1. / r2;
                let vol_bond_scaled = bv.vol_bond1000 * r2_inv * 0.001 * r2_inv * r2_inv;
                let b_param = (-0.148 * vol_bond_scaled.r_ln() - 0.96) * contact_angle_eff * contact_angle_eff
                    - 0.0082 * vol_bond_scaled.r_ln()
                    + 0.48;
                let c_param = 0.0018 * vol_bond_scaled.r_ln() + 0.078;
                -PI * self.surface_tension * (radi * radj).sqrt() * (b_param.r_exp() + c_param)
            }
            CapillaryKind::Washino => {
                // Rabinovich et al. (2005), Eqn. A11
                let dist = if k.is_wall { k.r - radi } else { k.r - (radi + radj) };
                -2. * PI
                    * r_eff
                    * self.surface_tension
                    * contact_angle_eff.r_cos()
                    * (1. - dist * (1000. * PI * r_eff / (2. * bv.vol_bond1000)).sqrt())
            }
        };

        let (f_visc_n, f_visc_t_over_vt) = self.viscous(r_eff, k.vn, None);
        let ft = Vec3::new(
            f_visc_t_over_vt * k.vtr.x,
            f_visc_t_over_vt * k.vtr.y,
            f_visc_t_over_vt * k.vtr.z,
        );
        let tor = Vec3::new(
            k.en.y * ft.z - k.en.z * ft.y,
            k.en.z * ft.x - k.en.x * ft.z,
            k.en.x * ft.y - k.en.y * ft.x,
        );
        Some(CapillaryIntersect {
            f_n: f_capilary + f_visc_n,
            ft,
            tor,
        })
    }

    /// `surfacesClose` of the capillary models for a pair (or, with
    /// `c.is_wall`, a particle-wall contact, gh:#1487) that does not overlap
    /// but lies inside the contact distance factor. `bridge` is the contact's
    /// `contflag == 1` on entry.
    ///
    /// On a wall both models use `radj = radi` and `dist = r - radi`; `c.radsum`
    /// must be `radi` (upstream's wall `sidata.radsum`), `c.v_j` the wall
    /// velocity, and the relative spin is the particle's alone (Easo
    /// `radi omega_i rinv`, Washino `(radi omega_i + 0) rinv`, as each header
    /// writes it). A wall rupture returns [`CloseOutcome::WallBreaks`].
    #[must_use]
    pub fn close(&self, c: &CloseInput) -> CloseOutcome {
        // Easo: case (i), no bridge present -> nothing (upstream returns before
        // reading anything else). Washino has no such early return.
        if self.kind == CapillaryKind::Easo && !c.bridge {
            return CloseOutcome::NoBridge;
        }
        let radi = c.rad_i;
        let radj = if c.is_wall { radi } else { c.rad_j };
        let r = c.rsq.sqrt();
        let radsum = c.radsum;
        let dist = if c.is_wall { r - radi } else { r - (radi + radj) };
        let bv = if c.is_wall {
            let vol_li1000 = FOUR_THIRDS_1000 * PI * radi * radi * radi * c.liquid_i;
            let vol_lj1000 = match self.kind {
                CapillaryKind::Easo => 0.0,
                CapillaryKind::Washino => c.wall_liquid_vol1000.unwrap_or(0.0),
            };
            self.bond_volumes_from(radi, radj, radsum, vol_li1000, vol_lj1000)
        } else {
            self.bond_volumes(radi, radj, radsum, c.liquid_i, c.liquid_j)
        };
        let r_eff = radi * radj / (radi + radj);
        let contact_angle_eff = 0.5 * (self.contact_angle[c.itype] + self.contact_angle[c.jtype]);
        let dist_max = (1. + 0.5 * contact_angle_eff) * bv.vol_bond1000.r_cbrt() * 0.1;

        let rupture = (self.max_separation_ratio - 1.0) * (radi + radj);
        let (bridge_active, bridge_breaks);
        if dist > rupture && c.bridge {
            // in this case always break
            bridge_breaks = true;
            bridge_active = false;
        } else if dist < dist_max && dist < rupture {
            bridge_active = true;
            bridge_breaks = false;
        } else if c.bridge {
            // can only break if exists
            bridge_breaks = true;
            bridge_active = false;
        } else {
            bridge_active = false;
            bridge_breaks = false;
        }

        if bridge_active {
            // skip if bond volume too small (contflag stays 1, no force)
            if bv.vol_bond1000 < SMALL_BOND_VOLUME {
                return CloseOutcome::ActiveNoForce;
            }
            let f_capilary = match self.kind {
                CapillaryKind::Easo => {
                    // Soulie et al. (2006): separated form
                    let r2 = if radi >= radj { radi } else { radj };
                    let r2_inv = 1. / r2;
                    let vol_bond_scaled = bv.vol_bond1000 * r2_inv * 0.001 * r2_inv * r2_inv;
                    let a_param = -1.1 * vol_bond_scaled.r_powf(-0.53);
                    let b_param = (-0.148 * vol_bond_scaled.r_ln() - 0.96) * contact_angle_eff * contact_angle_eff
                        - 0.0082 * vol_bond_scaled.r_ln()
                        + 0.48;
                    let c_param = 0.0018 * vol_bond_scaled.r_ln() + 0.078;
                    -PI * self.surface_tension
                        * (radi * radj).sqrt()
                        * ((a_param * dist / r2 + b_param).r_exp() + c_param)
                }
                CapillaryKind::Washino => {
                    // Langmuir 21 (24), 2005, Eqns. 19, 20, A3
                    let prefactor = -1. + (1. + 2. * bv.vol_bond1000 / (PI * r_eff * 1000. * dist * dist)).sqrt();
                    let d_sp_sp = 0.5 * dist * prefactor;
                    let alpha = (dist / r_eff * prefactor).sqrt();
                    -2. * PI
                        * r_eff
                        * self.surface_tension
                        * (contact_angle_eff.r_cos() / (1. + dist / (2. * d_sp_sp))
                            + alpha.r_sin() * (alpha + contact_angle_eff).r_sin())
                }
            };

            // vn and vt are not in the struct for a close pair
            let rinv = 1.0 / r;
            let (dx, dy, dz) = (c.delta.x, c.delta.y, c.delta.z);
            let enx = dx * rinv;
            let eny = dy * rinv;
            let enz = dz * rinv;
            let vr1 = c.v_i.x - c.v_j.x;
            let vr2 = c.v_i.y - c.v_j.y;
            let vr3 = c.v_i.z - c.v_j.z;
            let vn = vr1 * enx + vr2 * eny + vr3 * enz;
            let vn1 = vn * enx;
            let vn2 = vn * eny;
            let vn3 = vn * enz;
            let vt1 = vr1 - vn1;
            let vt2 = vr2 - vn2;
            let vt3 = vr3 - vn3;
            let (wr1, wr2, wr3) = match (c.is_wall, self.kind) {
                (false, _) => (
                    (radi * c.omega_i.x + radj * c.omega_j.x) * rinv,
                    (radi * c.omega_i.y + radj * c.omega_j.y) * rinv,
                    (radi * c.omega_i.z + radj * c.omega_j.z) * rinv,
                ),
                // Easo: `wr1 = radi * omega_i[0] * rinv`
                (true, CapillaryKind::Easo) => (
                    radi * c.omega_i.x * rinv,
                    radi * c.omega_i.y * rinv,
                    radi * c.omega_i.z * rinv,
                ),
                // Washino: `wr1 = 0.0; wr1 = (radi * omega_i[0] + wr1) * rinv`
                (true, CapillaryKind::Washino) => (
                    (radi * c.omega_i.x + 0.0) * rinv,
                    (radi * c.omega_i.y + 0.0) * rinv,
                    (radi * c.omega_i.z + 0.0) * rinv,
                ),
            };
            let vtr1 = vt1 - (dz * wr2 - dy * wr3);
            let vtr2 = vt2 - (dx * wr3 - dz * wr1);
            let vtr3 = vt3 - (dy * wr1 - dx * wr2);

            let (f_visc_n, f_visc_t_over_vt) = self.viscous(r_eff, vn, Some(dist));
            let ft1 = f_visc_t_over_vt * vtr1;
            let ft2 = f_visc_t_over_vt * vtr2;
            let ft3 = f_visc_t_over_vt * vtr3;
            let tor1 = eny * ft3 - enz * ft2;
            let tor2 = enz * ft1 - enx * ft3;
            let tor3 = enx * ft2 - eny * ft1;

            let fx = (f_capilary + f_visc_n) * enx + ft1;
            let fy = (f_capilary + f_visc_n) * eny + ft2;
            let fz = (f_capilary + f_visc_n) * enz + ft3;
            // `delta_F`/`delta_torque` start at +0 and are accumulated into
            let force_i = Vec3::zero().add(Vec3::new(fx, fy, fz));
            let torque_i = Vec3::zero().add(Vec3::new(-radi * tor1, -radi * tor2, -radi * tor3));
            let force_j = Vec3::zero().sub(Vec3::new(fx, fy, fz));
            let torque_j = Vec3::zero().add(Vec3::new(-radj * tor1, -radj * tor2, -radj * tor3));
            CloseOutcome::Force {
                force_i,
                torque_i,
                force_j,
                torque_j,
            }
        } else if bridge_breaks && c.is_wall {
            // Easo: no transfer at a (dry) wall. Washino: only on a mesh that
            // carries `LiquidContent` (`else if (liquidContPtr)`).
            let transfer = match (self.kind, c.wall_liquid_vol1000) {
                (CapillaryKind::Washino, Some(_)) => {
                    let invdt = 1. / c.dt;
                    // liquid flux in vol per time
                    let vol_flux = invdt * (0.5 * bv.vol_bond1000 - self.volume_fraction * bv.vol_li1000);
                    // liquid flux in vol% per time for particle
                    let flux_i = vol_flux / (FOUR_THIRDS_1000 * PI * radi * radi * radi);
                    Some(WallTransfer { flux_i, vol_flux })
                }
                _ => None,
            };
            CloseOutcome::WallBreaks { transfer }
        } else if bridge_breaks {
            // liquid transfer: assume the liquid distributes evenly. The
            // flux is in volume fraction per time.
            let invdt = 1. / c.dt;
            let rad_ratio = radj / radi;
            let split_factor = 1.0 / (1.0 + rad_ratio * rad_ratio * rad_ratio);
            let (flux_i, flux_j) = match self.kind {
                CapillaryKind::Easo => (
                    invdt * (split_factor * bv.vol_bond1000 - bv.vol_li_bond1000)
                        / (FOUR_THIRDS_1000 * PI * radi * radi * radi),
                    invdt * ((1. - split_factor) * bv.vol_bond1000 - bv.vol_lj_bond1000)
                        / (FOUR_THIRDS_1000 * PI * radj * radj * radj),
                ),
                CapillaryKind::Washino => (
                    invdt * (split_factor * bv.vol_bond1000 - self.volume_fraction * bv.vol_li1000)
                        / (FOUR_THIRDS_1000 * PI * radi * radi * radi),
                    invdt * ((1. - split_factor) * bv.vol_bond1000 - self.volume_fraction * bv.vol_lj1000)
                        / (FOUR_THIRDS_1000 * PI * radj * radj * radj),
                ),
            };
            CloseOutcome::Breaks { flux_i, flux_j }
        } else {
            CloseOutcome::NoBridge
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BondVolumes {
    vol_li1000: f64,
    vol_lj1000: f64,
    vol_li_bond1000: f64,
    vol_lj_bond1000: f64,
    vol_bond1000: f64,
}

/// What an overlapping pair's bridge adds to the contact force
/// ([`CapillaryParams::intersect`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapillaryIntersect {
    /// Capillary plus viscous normal force `F_capillary + F_visc,n` `[N]`,
    /// along `+e_n` on `i` (negative = attractive).
    pub f_n: f64,
    /// Viscous tangential force on `i` `[N]`.
    pub ft: Vec3,
    /// `e_n x F_t` before the contact-radius scaling (`torque_i = -c_ri tor`,
    /// `torque_j = -c_rj tor`).
    pub tor: Vec3,
}

/// The geometry and state a close (non-overlapping) pair's bridge needs.
#[derive(Debug, Clone, Copy)]
pub struct CloseInput {
    /// `x_i - x_j` `[m]`.
    pub delta: Vec3,
    /// `|delta|^2` `[m^2]`.
    pub rsq: f64,
    /// Radius of `i` `[m]`.
    pub rad_i: f64,
    /// Radius of `j` `[m]`.
    pub rad_j: f64,
    /// `rad_i + rad_j` (upstream's `scdata.radsum`) `[m]`.
    pub radsum: f64,
    /// Atom type of `i` (0-based).
    pub itype: usize,
    /// Atom type of `j` (0-based).
    pub jtype: usize,
    /// Velocity of `i` `[m/s]`.
    pub v_i: Vec3,
    /// Velocity of `j` `[m/s]`.
    pub v_j: Vec3,
    /// Angular velocity of `i` `[rad/s]`.
    pub omega_i: Vec3,
    /// Angular velocity of `j` `[rad/s]`.
    pub omega_j: Vec3,
    /// `surfaceLiquidContent` of `i` (already limited).
    pub liquid_i: f64,
    /// `surfaceLiquidContent` of `j` (already limited).
    pub liquid_j: f64,
    /// The pair's `contflag == 1` on entry (a bridge exists).
    pub bridge: bool,
    /// The time step `[s]` (`update->dt`).
    pub dt: f64,
    /// A particle-wall contact (gh:#1487): `j` is the wall, `rad_j` and
    /// `liquid_j` are ignored, `radsum` must be `rad_i`, `v_j` is the wall
    /// velocity and `omega_j` is ignored.
    pub is_wall: bool,
    /// Washino on a mesh carrying `fix mesh/surface/liquidtransfer`: the
    /// facet's liquid volume `* 1000` ([`MeshLiquid::wall_volume1000`]);
    /// `None` for a dry wall (upstream `liquidContPtr == NULL`) and for pairs.
    pub wall_liquid_vol1000: Option<f64>,
}

/// The liquid a Washino wall bridge moves when it ruptures on a
/// `liquidtransfer` mesh (gh:#1487).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallTransfer {
    /// Added to the particle's `liquidFlux` `[1/s]`.
    pub flux_i: f64,
    /// `volFlux` `[m^3/s * 1000]`; the facet receives
    /// `-volFlux / (1000 t_wall A_tri)` ([`MeshLiquid::add_liquid_flux`]).
    pub vol_flux: f64,
}

/// Result of [`CapillaryParams::close`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CloseOutcome {
    /// No bridge existed and none forms (case (i)); `contflag` stays 0.
    NoBridge,
    /// The bridge is active (`contflag = 1`) but its volume is below `1e-14`:
    /// no force.
    ActiveNoForce,
    /// The bridge is active and exerts forces and torques on both particles.
    Force {
        /// Force on `i` `[N]`.
        force_i: Vec3,
        /// Torque on `i` `[N m]`.
        torque_i: Vec3,
        /// Force on `j` `[N]`.
        force_j: Vec3,
        /// Torque on `j` `[N m]`.
        torque_j: Vec3,
    },
    /// The bridge ruptures this step (`contflag = 0`): the liquid flux
    /// `[1/s]` added to `i` and `j` (`liquidFlux`).
    Breaks {
        /// Added to `liquidFlux[i]`.
        flux_i: f64,
        /// Added to `liquidFlux[j]`.
        flux_j: f64,
    },
    /// A particle-wall bridge ruptures this step (`contflag = 0`); `transfer`
    /// is `Some` only for Washino on a `liquidtransfer` mesh (gh:#1487).
    WallBreaks {
        /// The liquid moved, if any.
        transfer: Option<WallTransfer>,
    },
}

/// `MeshModuleLiquidTransfer` (`mesh_module_liquidtransfer.cpp`, LIGGGHTS-PUBLIC
/// `3d5c00f2`): the per-facet `LiquidContent` and `LiquidFlux` of a mesh
/// declared `fix ... mesh/surface/liquidtransfer ... wall_thickness t
/// initial_liquid_content c` (gh:#1487). Only the Washino model reads or
/// writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshLiquid {
    /// `LiquidContent` per facet (volume fraction of the wall film).
    pub content: Vec<f64>,
    /// `LiquidFlux` per facet `[1/s]`.
    pub flux: Vec<f64>,
    /// `wall_thickness` `[m]`.
    pub wall_thickness: f64,
    /// `limit_liquid_content_`: set by the last `add_liquid_flux`.
    limit: bool,
    /// `max_liquid_content_`: set by the last `add_liquid_flux`.
    max_content: f64,
}

impl MeshLiquid {
    /// The module for a mesh of `n_tri` facets (`post_create`: every facet at
    /// `initial_liquid_content`, flux 0).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] as upstream's constructor: `wall_thickness`
    /// or `initial_liquid_content` below `1e-12` ("have to define ...").
    pub fn new(n_tri: usize, wall_thickness: f64, initial_liquid_content: f64) -> Result<Self, DemError> {
        if !(wall_thickness >= 1e-12) || !wall_thickness.is_finite() {
            return Err(DemError::InvalidInput("liquidtransfer: have to define 'wall_thickness'".into()));
        }
        if !(initial_liquid_content >= 1e-12) || !initial_liquid_content.is_finite() {
            return Err(DemError::InvalidInput(
                "liquidtransfer: have to define 'initial_liquid_content'".into(),
            ));
        }
        Ok(Self {
            content: vec![initial_liquid_content; n_tri],
            flux: vec![0.0; n_tri],
            wall_thickness,
            limit: false,
            max_content: 0.0,
        })
    }

    /// `pre_force` / `setup_pre_force`: `LiquidFlux = 0`.
    pub fn reset_flux(&mut self) {
        for f in &mut self.flux {
            *f = 0.;
        }
    }

    /// Washino's `volLj1000` for a wall contact on facet `tri` of area `area`:
    /// `LC * fmin(radi*radi*pi, area) * 1000 * wall_thickness`.
    #[must_use]
    pub fn wall_volume1000(&self, tri: usize, radi: f64, area: f64) -> f64 {
        self.content[tri] * (radi * radi * PI).min(area) * 1000.0 * self.wall_thickness
    }

    /// The rupture branch of Washino's `surfacesClose` on this mesh:
    /// `add_liquid_flux(j, -volFlux/(1000 t A), limit, max)`, which also
    /// (re)sets the module's limit flags (upstream: "TODO get from registry").
    pub fn add_liquid_flux(&mut self, tri: usize, vol_flux: f64, area: f64, limit: bool, max_content: f64) {
        let wall_liquid_flux = -vol_flux / (1000.0 * self.wall_thickness * area);
        self.limit = limit;
        self.max_content = max_content;
        self.flux[tri] += wall_liquid_flux;
    }

    /// `final_integrate` (`update_mesh_liquid_content`): `lc += flux dt`,
    /// clamped to `[0, 1]`, then to the maximum if limited.
    pub fn advance(&mut self, dt: f64) {
        for (lc, f) in self.content.iter_mut().zip(&self.flux) {
            let mut v = *lc;
            v += f * dt;
            v = 0.0_f64.max(1.0_f64.min(v));
            if self.limit {
                v = v.min(self.max_content);
            }
            *lc = v;
        }
    }
}

/// The `liquidtransfer` state a [`crate::granular_system::GranularSystem`]
/// keeps for a capillary model: per-particle liquid content and flux, and the
/// set of pairs with a bridge (`contflag == 1`, keyed by particle tags).
#[derive(Debug, Clone, PartialEq)]
pub struct CapillaryState {
    /// `surfaceLiquidContent` per particle.
    pub content: Vec<f64>,
    /// `liquidFlux` per particle `[1/s]`.
    pub flux: Vec<f64>,
    /// Tag pairs `(lo, hi)` whose history `contflag` is 1.
    pub bridges: std::collections::BTreeSet<(u64, u64)>,
    /// Particle-wall bridges `(tag, wall id, facet)` with `contflag == 1`
    /// (wall id as in the contact history: a primitive wall's index, or the
    /// static-boundary count plus a moving wall's index; facet 0 for a
    /// primitive wall). gh:#1487.
    pub wall_bridges: std::collections::BTreeSet<(u64, usize, usize)>,
    /// `fix mesh/surface/liquidtransfer` modules, by moving-wall index.
    pub mesh_liquid: std::collections::BTreeMap<usize, MeshLiquid>,
}

impl CapillaryState {
    /// `n` particles, each with `initial` liquid content and no flux or bridge.
    #[must_use]
    pub fn new(n: usize, initial: f64) -> Self {
        Self {
            content: vec![initial; n],
            flux: vec![0.0; n],
            bridges: std::collections::BTreeSet::new(),
            wall_bridges: std::collections::BTreeSet::new(),
            mesh_liquid: std::collections::BTreeMap::new(),
        }
    }

    /// `FixScalarTransportEquation::initial_integrate`: reset the flux.
    pub fn reset_flux(&mut self) {
        for f in &mut self.flux {
            *f = 0.;
        }
    }

    /// `FixScalarTransportEquation::advanceQtyExplicit` with no capacity and
    /// a zero source (`liquidSource`, never written by these models):
    /// `L += (Phi + 0 * nevery) dt`.
    pub fn advance(&mut self, dt: f64) {
        for (q, f) in self.content.iter_mut().zip(&self.flux) {
            *q += (*f + 0.0 * 1.0) * dt;
        }
    }

    /// Canonical tag key of a pair.
    #[must_use]
    pub fn key(a: u64, b: u64) -> (u64, u64) {
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn easo() -> CapillaryParams {
        CapillaryParams::easo(0.072, 1.0e-3, 0.0, 0.01, 1.2, 0.1).unwrap()
    }

    fn washino() -> CapillaryParams {
        CapillaryParams::washino(0.072, 1.0e-3, 0.0, 0.01, 1.2, 0.1).unwrap()
    }

    fn close_input(dist: f64, bridge: bool, li: f64) -> CloseInput {
        let r = 0.005;
        let d = 2.0 * r + dist;
        CloseInput {
            delta: Vec3::new(d, 0.0, 0.0),
            rsq: d * d,
            rad_i: r,
            rad_j: r,
            radsum: 2.0 * r,
            itype: 0,
            jtype: 0,
            v_i: Vec3::new(-0.1, 0.02, 0.0),
            v_j: Vec3::new(0.1, -0.02, 0.0),
            omega_i: Vec3::new(0.0, 0.0, 10.0),
            omega_j: Vec3::new(0.0, 0.0, -5.0),
            liquid_i: li,
            liquid_j: li,
            bridge,
            dt: 1e-6,
            is_wall: false,
            wall_liquid_vol1000: None,
        }
    }

    fn overlap_kin(li: f64) -> ContactKinematics {
        use crate::particle::Particle;
        use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
        use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};
        let mk = |x: f64, vx: f64| {
            Particle::new(
                Vec3::new(x, 0.0, 0.0),
                Vec3::new(vx, 0.01, 0.0),
                Vec3::new(0.0, 0.0, 3.0),
                Mass::new::<kilogram>(1e-3),
                Length::new::<meter>(0.005),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        };
        ContactKinematics::pair(&mk(0.0049, -0.2), &mk(-0.0049, 0.2))
            .unwrap()
            .with_liquid(li, li)
    }

    /// Hand-evaluated Easo contact bridge (Soulie): for `r = 5 mm`,
    /// `L = 0.1`, `gamma = 0.072`, `theta = 0` the bond volume, scaled volume
    /// and `F_c = -pi gamma r (exp(B) + C)` are computed here independently of
    /// the code under test.
    #[test]
    fn easo_contact_capillary_force_matches_hand_value() {
        let k = overlap_kin(0.1);
        let out = easo().intersect(&k).expect("bridge volume above 1e-14");
        let r: f64 = 0.005;
        let vol_l1000 = 1333.333333 * PI * r.powi(3) * 0.1;
        let share = 0.5 * vol_l1000 * (1.0 - (1.0 - 0.25_f64).sqrt());
        let vb1000 = 2.0 * share;
        let scaled = vb1000 / r / r / r * 0.001;
        let b = -0.0082 * scaled.ln() + 0.48; // theta = 0 kills the angle term
        let c = 0.0018 * scaled.ln() + 0.078;
        let f_c = -PI * 0.072 * r * (b.exp() + c);
        // viscous normal part at the fixed minimum separation
        let r_eff = r * r / (2.0 * r);
        let f_v = -6.0 * PI * 1.0e-3 * r_eff * k.vn / 0.01;
        assert_relative_eq!(out.f_n, f_c + f_v, max_relative = 1e-13);
        assert!(f_c < 0.0, "a wet bridge attracts");
    }

    /// Hand-evaluated Washino contact bridge (Rabinovich A11) with a
    /// negative surface distance `s = r - 2 r` (overlap): `F = -2 pi R* gamma
    /// cos(theta)(1 - s sqrt(pi R*/(2 V_b)))`.
    #[test]
    fn washino_contact_capillary_force_matches_hand_value() {
        let k = overlap_kin(0.1);
        let out = washino().intersect(&k).unwrap();
        let r: f64 = 0.005;
        let vb1000 = (1333.333333 * PI * r.powi(3) * 0.1) * 2.0 * 0.05;
        let r_eff = r * r / (2.0 * r);
        let s = k.r - 2.0 * r;
        let f_c = -2.0 * PI * r_eff * 0.072 * 1.0 * (1.0 - s * (1000.0 * PI * r_eff / (2.0 * vb1000)).sqrt());
        let f_v = -6.0 * PI * 1.0e-3 * r_eff * k.vn / 0.01;
        assert_relative_eq!(out.f_n, f_c + f_v, max_relative = 1e-12);
    }

    /// A dry pair (`L = 0`) has bond volume 0: d_max = 0, so an existing bridge "breaks" with zero flux (nothing to transfer); contact returns early
    /// and adds nothing (both models, both contact and close).
    #[test]
    fn dry_particles_exert_no_bridge_force() {
        for p in [easo(), washino()] {
            assert!(p.intersect(&overlap_kin(0.0)).is_none());
            let c = p.close(&close_input(0.0005, true, 0.0));
            assert!(
                matches!(c, CloseOutcome::ActiveNoForce | CloseOutcome::NoBridge)
                    || c == CloseOutcome::Breaks { flux_i: 0.0, flux_j: 0.0 },
                "{c:?}"
            );
        }
    }

    /// Easo case (i): without a bridge nothing happens out of contact; Washino
    /// creates one below `d_max` (the documented difference).
    #[test]
    fn only_washino_creates_a_bridge_out_of_contact() {
        let c = close_input(0.0005, false, 0.1);
        assert_eq!(easo().close(&c), CloseOutcome::NoBridge);
        assert!(matches!(washino().close(&c), CloseOutcome::Force { .. }));
    }

    /// The three-way rupture logic: an existing bridge is active below
    /// `min(d_max, (max-1)(ri+rj))`, breaks beyond `d_max`, and breaks beyond
    /// the hard limit even if `d_max` is larger.
    #[test]
    fn rupture_distance_branches() {
        let p = easo();
        // d_max for r = 5 mm, L = 0.1, theta = 0: cbrt(V_b*1000)*0.1
        let r: f64 = 0.005;
        let vb1000 = 2.0 * 0.5 * (1333.333333 * PI * r.powi(3) * 0.1) * (1.0 - (1.0 - 0.25_f64).sqrt());
        let d_max = vb1000.cbrt() * 0.1;
        let hard = (1.2 - 1.0) * 2.0 * r;
        assert!(d_max < hard, "test geometry: d_max-limited");
        assert!(matches!(
            p.close(&close_input(0.9 * d_max, true, 0.1)),
            CloseOutcome::Force { .. }
        ));
        assert!(matches!(
            p.close(&close_input(1.1 * d_max, true, 0.1)),
            CloseOutcome::Breaks { .. }
        ));
        // hard-limited: a tighter max ratio so the limit is below d_max
        let tight = CapillaryParams::easo(0.072, 1.0e-3, 0.0, 0.01, 1.05, 0.1).unwrap();
        let hard = 0.05 * 2.0 * r;
        assert!(hard < d_max);
        assert!(matches!(
            tight.close(&close_input(0.9 * hard, true, 0.1)),
            CloseOutcome::Force { .. }
        ));
        assert!(matches!(
            tight.close(&close_input(1.1 * hard, true, 0.1)),
            CloseOutcome::Breaks { .. }
        ));
    }

    /// Liquid is conserved at rupture: the fluxes times `dt` times the particle
    /// volumes sum to the bridge volume minus what the bridge already drew
    /// from the particles, i.e. the total change of liquid volume is zero for
    /// the Easo model (`split V_b - V_Li,bond` + `(1-split) V_b - V_Lj,bond`
    /// = `V_b - V_b = 0`).
    #[test]
    fn easo_rupture_conserves_liquid() {
        let p = easo();
        let c = close_input(0.5, true, 0.1); // far beyond the hard limit
        let CloseOutcome::Breaks { flux_i, flux_j } = p.close(&c) else {
            panic!("must break");
        };
        let r: f64 = 0.005;
        let vol = |f: f64| f * c.dt * (1333.333333 * PI * r.powi(3));
        assert_relative_eq!(vol(flux_i) + vol(flux_j), 0.0, epsilon = 1e-18);
    }

    /// Washino `limitLiquidContent` clips at the per-type maximum and
    /// overwrites the maximum separation ratio from it.
    #[test]
    fn washino_liquid_content_limit() {
        let p = washino().with_liquid_content_limit([0.2, 0.3, 0.4, 0.5]).unwrap();
        assert_eq!(p.limit_content(0.9, 1), 0.3);
        assert_eq!(p.limit_content(0.1, 1), 0.1);
        assert_eq!(washino().limit_content(0.9, 1), 0.9);
        let ratio = p.limited_max_separation_ratio(1, 0.005, 0.005);
        let vb1000 = 2666.666666 * PI * 0.005_f64.powi(3) * 0.2 * 0.05;
        let want = 1.0 + 0.5 * (vb1000.cbrt() * 0.1) / 0.005;
        assert_relative_eq!(ratio, want, max_relative = 1e-13);
        assert_eq!(washino().limited_max_separation_ratio(1, 0.005, 0.005), 1.2);
        assert!(easo().with_liquid_content_limit([0.1; MAX_TYPES]).is_err());
        assert!(easo().with_lb_volume_fraction(0.1).is_err());
    }

    /// The scalar transport equation: flux reset, then `L += Phi dt`.
    #[test]
    fn liquid_transfer_fix_integrates_the_flux() {
        let mut s = CapillaryState::new(2, 0.1);
        s.flux[0] = 2.0;
        s.flux[1] = -1.0;
        s.advance(0.01);
        assert_relative_eq!(s.content[0], 0.12, max_relative = 1e-14);
        assert_relative_eq!(s.content[1], 0.09, max_relative = 1e-14);
        s.reset_flux();
        s.advance(0.01);
        assert_relative_eq!(s.content[0], 0.12, max_relative = 1e-14);
        assert_eq!(CapillaryState::key(5, 2), (2, 5));
    }

    /// A wall contact of `overlap` on a z plane (gh:#1487), upstream's wall
    /// kinematics (`ContactKinematics::mesh_wall`).
    fn wall_kin(li: f64, overlap: f64) -> ContactKinematics {
        use crate::particle::Particle;
        use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
        use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};
        let r = 0.005;
        let p = Particle::new(
            Vec3::new(0.0, 0.0, r - overlap),
            Vec3::new(0.1, 0.02, -0.2),
            Vec3::new(1.0, 2.0, 3.0),
            Mass::new::<kilogram>(1e-3),
            Length::new::<meter>(r),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap();
        ContactKinematics::mesh_wall(&p, Vec3::new(0.0, 0.0, r - overlap), overlap, Vec3::zero())
            .with_liquid(li, 0.0)
    }

    /// Easo on a (dry) wall, hand-evaluated: `radj = radsum = radi`, so
    /// `V_b = 0.5 V_Li` and `rEff = r/2` (gh:#1487).
    #[test]
    fn easo_wall_contact_bridge_is_half_the_particle_liquid() {
        let k = wall_kin(0.1, 1e-5);
        let out = easo().intersect(&k).unwrap();
        let r: f64 = 0.005;
        let vb1000 = 0.5 * (1333.333333 * PI * r.powi(3) * 0.1);
        let scaled = vb1000 / r / r / r * 0.001;
        let b = -0.0082 * scaled.ln() + 0.48;
        let c = 0.0018 * scaled.ln() + 0.078;
        let f_c = -PI * 0.072 * r * (b.exp() + c);
        let f_v = -6.0 * PI * 1.0e-3 * (0.5 * r) * k.vn / 0.01;
        assert_relative_eq!(out.f_n, f_c + f_v, max_relative = 1e-12);
    }

    /// Washino in wall contact reads upstream's unset `sidata.radj = 0`: the
    /// bridge exists (not `None`) but its forces are zero (gh:#1487).
    #[test]
    fn washino_wall_contact_force_is_zero() {
        let out = washino().intersect(&wall_kin(0.1, 1e-5)).unwrap();
        assert_eq!(out.f_n, 0.0);
        assert_eq!(out.ft.norm(), 0.0);
        // dry particle on a dry wall: no bridge volume
        assert!(washino().intersect(&wall_kin(0.0, 1e-5)).is_none());
    }

    /// Wall `surfacesClose`: an active bridge pulls the particle to the wall;
    /// rupture moves liquid only for Washino on a liquid mesh, with the
    /// particle share `(0.5 V_b - f V_Li)/dt` (gh:#1487).
    #[test]
    fn wall_close_and_rupture() {
        let r = 0.005;
        let at = |gap: f64, bridge: bool, wall: Option<f64>| CloseInput {
            delta: Vec3::new(-0.0, -0.0, r + gap),
            rsq: (r + gap) * (r + gap),
            rad_i: r,
            rad_j: r,
            radsum: r,
            itype: 0,
            jtype: 0,
            v_i: Vec3::new(0.1, 0.0, 0.05),
            v_j: Vec3::zero(),
            omega_i: Vec3::new(0.0, 4.0, 0.0),
            omega_j: Vec3::zero(),
            liquid_i: 0.2,
            liquid_j: 0.0,
            bridge,
            dt: 1e-6,
            is_wall: true,
            wall_liquid_vol1000: wall,
        };
        for p in [easo(), washino()] {
            let CloseOutcome::Force { force_i, .. } = p.close(&at(0.0005, true, None)) else {
                panic!("active bridge");
            };
            assert!(force_i.z < 0.0, "attracts towards the wall");
            assert_eq!(p.close(&at(0.05, true, None)), CloseOutcome::WallBreaks { transfer: None });
        }
        assert_eq!(easo().close(&at(0.05, true, Some(1e-5))), CloseOutcome::WallBreaks { transfer: None });
        let CloseOutcome::WallBreaks { transfer: Some(tr) } = washino().close(&at(0.05, true, Some(1e-5))) else {
            panic!("liquid mesh rupture transfers");
        };
        let vli = 1333.333333 * PI * r * r * r * 0.2;
        let vb = (vli + 1e-5) * 0.05;
        assert_relative_eq!(tr.vol_flux, (0.5 * vb - 0.05 * vli) / 1e-6, max_relative = 1e-12);
        assert_relative_eq!(tr.flux_i, tr.vol_flux / (1333.333333 * PI * r * r * r), max_relative = 1e-12);
    }

    /// `MeshModuleLiquidTransfer`: film volume, flux, clamp to `[0, 1]` and
    /// the limit set by the last `add_liquid_flux` (gh:#1487).
    #[test]
    fn mesh_liquid_module() {
        assert!(MeshLiquid::new(2, 0.0, 0.1).is_err());
        assert!(MeshLiquid::new(2, 1e-3, 0.0).is_err());
        let mut m = MeshLiquid::new(2, 1e-3, 0.1).unwrap();
        // fmin(pi r^2, A): the facet area caps the wetted patch
        assert_relative_eq!(m.wall_volume1000(0, 0.005, 1.0), 0.1 * PI * 25e-6 * 1000.0 * 1e-3, max_relative = 1e-14);
        assert_relative_eq!(m.wall_volume1000(0, 0.005, 1e-6), 0.1 * 1e-6 * 1000.0 * 1e-3, max_relative = 1e-14);
        m.add_liquid_flux(0, -2.0, 4.0, false, 0.0);
        assert_relative_eq!(m.flux[0], 2.0 / (1000.0 * 1e-3 * 4.0), max_relative = 1e-14);
        m.advance(0.1);
        assert_relative_eq!(m.content[0], 0.15, max_relative = 1e-14);
        assert_eq!(m.content[1], 0.1);
        m.flux[1] = -100.0;
        m.advance(1.0);
        assert_eq!(m.content[1], 0.0, "clamped at 0");
        m.reset_flux();
        m.add_liquid_flux(0, -1e9, 1.0, true, 0.3);
        m.advance(1.0);
        assert_eq!(m.content[0], 0.3, "clamped to [0, 1], then to the limit");
    }

    /// Constructor validation (upstream's `error->one` and property checks).
    #[test]
    fn parameters_are_validated() {
        assert!(CapillaryParams::easo(0.07, 1e-3, 0.0, 0.01, 0.9, 0.1).is_err());
        assert!(CapillaryParams::easo(0.07, 1e-3, 0.0, 0.0, 1.2, 0.1).is_err());
        assert!(CapillaryParams::easo(-0.07, 1e-3, 0.0, 0.01, 1.2, 0.1).is_err());
        assert_eq!(CapillaryParams::contact_distance_factor(1.2), 1.2 * 1.1);
        let p = easo().with_tangential_reduce(true).with_contact_angles([0.1; MAX_TYPES]);
        assert!(p.tangential_reduce);
    }
}
