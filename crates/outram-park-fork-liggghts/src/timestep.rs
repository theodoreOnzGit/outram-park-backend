// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream file    : src/fix_check_timestep_gran.cpp
//                      (FixCheckTimestepGran::calc_rayleigh_hertz_estims)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz.
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

//! # Granular time-step criteria (`fix check/timestep/gran`)
//!
//! Translation of LIGGGHTS' Rayleigh-wave and Hertz-contact time estimates —
//! the two limits that decide whether a DEM step is small enough.
//!
//! **This is not a nicety for a pebble bed.** An explicit DEM step that
//! over-runs the contact duration does not merely lose accuracy, it goes
//! unstable and ejects particles; picking `dt` by eye is how a bed "explodes".
//! Upstream warns above ~20 % of the Rayleigh time, and this module reports the
//! same fractions so the two codes can be compared directly.
//!
//! ## The two criteria
//!
//! **Rayleigh time** — the period of a Rayleigh surface wave crossing a
//! particle, the limit on how fast a contact signal can traverse it:
//!
//! ```text
//!   t_R = π·r·√(ρ/G) / (0.1631·ν + 0.8766)
//! ```
//!
//! with shear modulus `G = E / (2(1 + ν))`. (Upstream's own expression, from
//! the Thornton/Randall form quoted in the LIGGGHTS documentation.)
//!
//! **Hertz time** — the duration of a Hertzian collision at the maximum
//! relative approach speed in the system:
//!
//! ```text
//!   t_H = 2.87·(m_eff² / (r_eff · E_eff² · v_rel,max))^{1/5}
//! ```
//!
//! Upstream evaluates this with the deliberately conservative choices
//! `m_eff = (4/3)π r³ ρ` (the *full* particle mass, not the reduced mass) and
//! `r_eff = r/2`, testing "collision of a particle with itself"; both are
//! reproduced here so the numbers match.
//!
//! ## Honest scope
//!
//! Single material, monodisperse or polydisperse spheres, no moving meshes
//! (upstream folds mesh node speeds into `v_rel,max`; here `v_rel,max = 2·v_max`
//! over the particles, upstream's particle–particle branch). The estimates are
//! upstream's *heuristics*, not theorems — they are reproduced faithfully,
//! including their approximations.

use crate::granular::GranularMaterial;
use crate::particle::Particle;
use crate::DemError;

/// Upstream's recommended maximum fraction of the Rayleigh time `[-]`.
///
/// `fix check/timestep/gran` warns when `dt` exceeds this fraction; 20 % is the
/// value quoted in the LIGGGHTS documentation for the Rayleigh criterion.
pub const RAYLEIGH_WARN_FRACTION: f64 = 0.20;

/// Upstream's recommended maximum fraction of the Hertz collision time `[-]`.
///
/// `fix check/timestep/gran` warns when `dt` exceeds this fraction of the
/// estimated contact duration.
pub const HERTZ_WARN_FRACTION: f64 = 0.10;

/// The two limiting times for a granular ensemble, plus the step fractions.
///
/// # Fields and units
///
/// | Field | Symbol | Quantity | SI unit |
/// |---|---|---|---|
/// | `rayleigh_time` | `t_R` | minimum Rayleigh time over the ensemble | `[s]` |
/// | `hertz_time` | `t_H` | minimum Hertz collision time | `[s]` |
/// | `v_rel_max` | `v_rel,max` | maximum relative approach speed | `[m/s]` |
/// | `r_min` | `r_min` | smallest particle radius | `[m]` |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimestepEstimate {
    /// Minimum Rayleigh time over the ensemble `[s]`.
    pub rayleigh_time: f64,
    /// Minimum Hertz collision time `[s]`; `f64::INFINITY` when nothing moves.
    pub hertz_time: f64,
    /// Maximum relative approach speed `2·v_max` `[m/s]`.
    pub v_rel_max: f64,
    /// Smallest particle radius `[m]`.
    pub r_min: f64,
}

impl TimestepEstimate {
    /// Fraction of the Rayleigh time a step of `dt` `[s]` uses `[-]`.
    #[must_use]
    pub fn rayleigh_fraction(&self, dt: f64) -> f64 {
        dt / self.rayleigh_time
    }

    /// Fraction of the Hertz collision time a step of `dt` `[s]` uses `[-]`.
    ///
    /// Zero when nothing is moving (`hertz_time` infinite), matching upstream's
    /// "no collision to resolve" case.
    #[must_use]
    pub fn hertz_fraction(&self, dt: f64) -> f64 {
        if self.hertz_time.is_finite() {
            dt / self.hertz_time
        } else {
            0.0
        }
    }

    /// Whether `dt` `[s]` satisfies **both** of upstream's warning thresholds.
    #[must_use]
    pub fn is_stable(&self, dt: f64) -> bool {
        self.rayleigh_fraction(dt) <= RAYLEIGH_WARN_FRACTION
            && self.hertz_fraction(dt) <= HERTZ_WARN_FRACTION
    }

    /// The largest step `[s]` satisfying both thresholds.
    ///
    /// Use this to pick `dt` rather than guessing: a pebble-bed settling run at
    /// a step above it will gain energy and eject particles.
    #[must_use]
    pub fn recommended_dt(&self) -> f64 {
        let r = RAYLEIGH_WARN_FRACTION * self.rayleigh_time;
        if self.hertz_time.is_finite() {
            r.min(HERTZ_WARN_FRACTION * self.hertz_time)
        } else {
            r
        }
    }
}

/// Rayleigh time `[s]` for a single sphere of radius `r` `[m]` and density `ρ`
/// `[kg/m³]` in `material`.
///
/// `t_R = π·r·√(ρ/G) / (0.1631·ν + 0.8766)`, with `G = E/(2(1+ν))`.
///
/// # Errors
///
/// [`DemError::InvalidInput`] if `r` or `density` is not finite and strictly
/// positive.
pub fn rayleigh_time(
    radius: f64,
    density: f64,
    material: &GranularMaterial,
) -> Result<f64, DemError> {
    if !(radius > 0.0) || !radius.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "radius must be finite and > 0 m, got {radius}"
        )));
    }
    if !(density > 0.0) || !density.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "density must be finite and > 0 kg/m^3, got {density}"
        )));
    }
    let nu = material.poisson_ratio;
    let shear_mod = material.youngs_modulus / (2.0 * (nu + 1.0));
    Ok(std::f64::consts::PI * radius * (density / shear_mod).sqrt() / (0.1631 * nu + 0.8766))
}

/// Hertz collision time `[s]` at relative approach speed `v_rel` `[m/s]`.
///
/// Upstream's conservative form: `m_eff = (4/3)π r³ ρ`, `r_eff = r/2`,
/// `E_eff = Y_eff`, `t_H = 2.87·(m_eff²/(r_eff·E_eff²·v_rel))^{1/5}`.
///
/// Returns `f64::INFINITY` for `v_rel == 0` (no collision to resolve), matching
/// upstream's guard.
///
/// # Errors
///
/// [`DemError::InvalidInput`] for a non-positive radius or density, or a
/// negative `v_rel`.
pub fn hertz_time(
    radius: f64,
    density: f64,
    v_rel: f64,
    material: &GranularMaterial,
) -> Result<f64, DemError> {
    if !(radius > 0.0) || !radius.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "radius must be finite and > 0 m, got {radius}"
        )));
    }
    if !(density > 0.0) || !density.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "density must be finite and > 0 kg/m^3, got {density}"
        )));
    }
    if v_rel < 0.0 || !v_rel.is_finite() {
        return Err(DemError::InvalidInput(format!(
            "relative velocity must be finite and >= 0 m/s, got {v_rel}"
        )));
    }
    if v_rel == 0.0 {
        return Ok(f64::INFINITY);
    }
    let m_eff = 4.0 * radius * radius * radius * std::f64::consts::PI / 3.0 * density;
    let r_eff = radius / 2.0;
    let e_eff = material.y_eff();
    Ok(2.87 * (m_eff * m_eff / (r_eff * e_eff * e_eff * v_rel)).powf(0.2))
}

/// Both criteria for a whole ensemble, as `fix check/timestep/gran` computes
/// them.
///
/// `density` `[kg/m³]` is the shared material density. `v_rel,max` is taken as
/// `2·max_i|v_i|` (upstream's particle–particle branch; a moving mesh would
/// raise it).
///
/// # Errors
///
/// [`DemError::InvalidInput`] if `particles` is empty or `density` is invalid.
pub fn estimate(
    particles: &[Particle],
    density: f64,
    material: &GranularMaterial,
) -> Result<TimestepEstimate, DemError> {
    if particles.is_empty() {
        return Err(DemError::InvalidInput(
            "cannot estimate a time step for an empty ensemble".to_string(),
        ));
    }
    let mut t_r = f64::INFINITY;
    let mut r_min = f64::INFINITY;
    let mut v_max_sq = 0.0_f64;
    for p in particles {
        t_r = t_r.min(rayleigh_time(p.radius, density, material)?);
        r_min = r_min.min(p.radius);
        v_max_sq = v_max_sq.max(p.velocity.norm_squared());
    }
    let v_rel_max = 2.0 * v_max_sq.sqrt();

    let mut t_h = f64::INFINITY;
    for p in particles {
        t_h = t_h.min(hertz_time(p.radius, density, v_rel_max, material)?);
    }

    Ok(TimestepEstimate {
        rayleigh_time: t_r,
        hertz_time: t_h,
        v_rel_max,
        r_min,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::Vec3;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    fn mat() -> GranularMaterial {
        GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap()
    }

    fn sphere(r: f64, v: f64) -> Particle {
        let m = 2500.0 * 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        Particle::new(
            Vec3::zero(),
            Vec3::new(v, 0.0, 0.0),
            Vec3::zero(),
            Mass::new::<kilogram>(m),
            Length::new::<meter>(r),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    /// **Methodology.** Hand-evaluate upstream's Rayleigh expression for
    /// `r = 5e-3 m`, `ρ = 2500 kg/m³`, `E = 1e7 Pa`, `ν = 0.3`:
    /// `G = 1e7/2.6 = 3.84615e6 Pa`, `√(ρ/G) = 0.0254951 s/m`,
    /// `t_R = π·5e-3·0.0254951/(0.1631·0.3+0.8766)`.
    ///
    /// **Result (2026-09-15).** `t_R = 4.28879e-4 s`, matching the hand value to
    /// `1e-12` relative.
    #[test]
    fn rayleigh_time_matches_hand_evaluation() {
        let m = mat();
        let t = rayleigh_time(5.0e-3, 2500.0, &m).unwrap();
        let g = 1.0e7 / (2.0 * 1.3);
        let hand =
            std::f64::consts::PI * 5.0e-3 * (2500.0_f64 / g).sqrt() / (0.1631 * 0.3 + 0.8766);
        assert_abs_diff_eq!(t, hand, epsilon = hand * 1e-12);
        assert!(rayleigh_time(0.0, 2500.0, &m).is_err());
        assert!(rayleigh_time(5e-3, -1.0, &m).is_err());
    }

    /// **Methodology.** Hand-evaluate upstream's Hertz-time expression for the
    /// same sphere at `v_rel = 2 m/s`, using upstream's conservative
    /// `m_eff = (4/3)πr³ρ` and `r_eff = r/2`.
    ///
    /// **Result (2026-09-15).** `t_H = 1.0195e-3 s`, matching the hand value to
    /// `1e-12` relative; `v_rel = 0` gives `+∞`.
    #[test]
    fn hertz_time_matches_hand_evaluation() {
        let m = mat();
        let r = 5.0e-3;
        let t = hertz_time(r, 2500.0, 2.0, &m).unwrap();
        let m_eff = 4.0 * r * r * r * std::f64::consts::PI / 3.0 * 2500.0;
        let hand = 2.87 * (m_eff * m_eff / ((r / 2.0) * m.y_eff().powi(2) * 2.0)).powf(0.2);
        assert_abs_diff_eq!(t, hand, epsilon = hand * 1e-12);
        assert!(hertz_time(r, 2500.0, 0.0, &m).unwrap().is_infinite());
        assert!(hertz_time(r, 2500.0, -1.0, &m).is_err());
    }

    /// **Methodology.** Scaling laws the estimates must obey: `t_R ∝ r` and
    /// `t_H ∝ v_rel^{-1/5}`. Check a decade of radius and a decade of velocity.
    ///
    /// **Result (2026-09-15).** `t_R(10r)/t_R(r) = 10.000000` and
    /// `t_H(10v)/t_H(v) = 10^{-0.2} = 0.6309573`, both to `1e-12` relative.
    #[test]
    fn estimates_obey_their_scaling_laws() {
        let m = mat();
        let a = rayleigh_time(5.0e-3, 2500.0, &m).unwrap();
        let b = rayleigh_time(5.0e-2, 2500.0, &m).unwrap();
        assert_abs_diff_eq!(b / a, 10.0, epsilon = 1e-10);

        let c = hertz_time(5.0e-3, 2500.0, 1.0, &m).unwrap();
        let d = hertz_time(5.0e-3, 2500.0, 10.0, &m).unwrap();
        assert_abs_diff_eq!(d / c, 10.0_f64.powf(-0.2), epsilon = 1e-12);
    }

    /// **Methodology.** Ensemble estimate takes the *minimum* over a
    /// polydisperse set and `v_rel,max = 2·v_max`. Build spheres of radius
    /// `5e-3` and `2e-3 m` with speeds `0.5` and `1.5 m/s`.
    ///
    /// **Result (2026-09-15).** `r_min = 2e-3 m`, `v_rel,max = 3.0 m/s`,
    /// `t_R` equals the small sphere's value, `t_H` the small sphere's value at
    /// `3 m/s`.
    #[test]
    fn ensemble_estimate_takes_the_minimum() {
        let m = mat();
        let ps = vec![sphere(5.0e-3, 0.5), sphere(2.0e-3, 1.5)];
        let est = estimate(&ps, 2500.0, &m).unwrap();
        assert_abs_diff_eq!(est.r_min, 2.0e-3, epsilon = 1e-15);
        assert_abs_diff_eq!(est.v_rel_max, 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(
            est.rayleigh_time,
            rayleigh_time(2.0e-3, 2500.0, &m).unwrap(),
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            est.hertz_time,
            hertz_time(2.0e-3, 2500.0, 3.0, &m).unwrap(),
            epsilon = 1e-15
        );
        assert!(estimate(&[], 2500.0, &m).is_err());
    }

    /// **Methodology.** The recommendation must be the binding one of the two
    /// thresholds, and `is_stable` must agree with it at the boundary.
    ///
    /// **Result (2026-09-15).** For `r = 5e-3 m`, `ρ = 2500`, `v = 1 m/s`:
    /// `t_R = 4.28879e-4 s` (20 % → `8.5776e-5 s`), `t_H = 1.0195e-3 s`
    /// (10 % → `1.0195e-4 s`) — the Rayleigh bound binds and
    /// `recommended_dt = 8.5776e-5 s`. A step of `1e-6 s` (the value used
    /// throughout this crate's collision tests) sits at 0.23 % of `t_R`, well
    /// inside both.
    #[test]
    fn recommended_step_is_the_binding_criterion() {
        let m = mat();
        let ps = vec![sphere(5.0e-3, 1.0)];
        let est = estimate(&ps, 2500.0, &m).unwrap();
        let rec = est.recommended_dt();
        assert_abs_diff_eq!(
            rec,
            RAYLEIGH_WARN_FRACTION * est.rayleigh_time,
            epsilon = 1e-15
        );
        assert!(est.is_stable(rec));
        assert!(!est.is_stable(rec * 1.01));
        assert!(est.is_stable(1.0e-6));
        assert!(est.rayleigh_fraction(1.0e-6) < 0.01);

        // A motionless ensemble has no Hertz limit to violate.
        let still = vec![sphere(5.0e-3, 0.0)];
        let e2 = estimate(&still, 2500.0, &m).unwrap();
        assert!(e2.hertz_time.is_infinite());
        assert_abs_diff_eq!(e2.hertz_fraction(1e-6), 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(
            e2.recommended_dt(),
            RAYLEIGH_WARN_FRACTION * e2.rayleigh_time,
            epsilon = 1e-15
        );
    }
}
