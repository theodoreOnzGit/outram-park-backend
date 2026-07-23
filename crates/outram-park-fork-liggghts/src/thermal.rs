// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — clean-room thermal-DEM contact conduction
// reimplemented from public heat-transfer / granular-conduction literature
// (Batchelor & O'Brien 1977; Vargas & McCarthy 2001/2002; Hertz 1882 /
// Johnson, Contact Mechanics 1985). NOT a translation of GPL-2.0
// LIGGGHTS/LAMMPS source — see the crate NOTICE.
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

//! Phase 4 — **Thermal DEM** (bead `op-t3l.4`).
//!
//! Particle–particle and particle–wall **contact conduction**: the rate at
//! which heat flows through the small circular contact spot where two touching
//! solids meet, plus an explicit-Euler temperature update that evolves each
//! particle's [`Particle::temperature`] field (passive in Phase 1) from the net
//! conductive heat rate. Clean-room implementation from public granular
//! heat-transfer literature — **not** derived from LIGGGHTS/LAMMPS source (see
//! the crate `NOTICE`).
//!
//! # Physical model
//!
//! When two solid spheres touch over a circular contact of radius `a_c`, heat
//! flows across the contact by conduction through the constriction. The
//! constriction resistance of a circular spot of radius `a_c` into a
//! semi-infinite solid of conductivity `k` is `R = 1/(4·k·a_c)` (the classical
//! Maxwell/Holm constriction result). Two solids meeting at the contact place
//! two such resistances in series, so the pair conductance is
//!
//! ```text
//!   h_c = 1 / (R_i + R_j) = 4·a_c / (1/k_i + 1/k_j) = 2·k_s·a_c,
//! ```
//!
//! where `k_s = 2·k_i·k_j / (k_i + k_j)` is the **harmonic mean** of the two
//! solid conductivities. This is the Batchelor & O'Brien (1977) single-contact
//! conductance, in the form used by Vargas & McCarthy (2001, 2002) for DEM heat
//! conduction. The conductive heat rate **into** particle `i` from a touching
//! partner `j` is then
//!
//! ```text
//!   Q_ij = h_c · (T_j − T_i)   [W],
//! ```
//!
//! which is antisymmetric (`Q_ji = −Q_ij`), so a two-body exchange conserves
//! energy exactly. Summing `Q` over a particle's contacts gives its net heat
//! rate `Q_net`, and an explicit forward-Euler step advances its temperature by
//!
//! ```text
//!   dT_i/dt = Q_net / (m_i·c_p)   ⇒   T_i(t+dt) = T_i(t) + Q_net·dt / (m_i·c_p).
//! ```
//!
//! # Contact radius
//!
//! The contact radius `a_c` couples this thermal model to the mechanical
//! contact. For **Hertzian** elastic contact the mutual approach (overlap) `δ`
//! and the contact radius are related by `δ = a_c² / R*`, i.e.
//! `a_c = sqrt(R*·δ)`, with the effective (reduced) radius
//! `R* = r_i·r_j / (r_i + r_j)` — see [`hertzian_contact_radius`] and
//! [`effective_radius`]. (A purely *geometric* truncation of two overlapping
//! spheres gives the slightly larger `a_c = sqrt(2·R*·δ)`; the elastic value is
//! smaller because the surfaces deform rather than interpenetrate.) The
//! conductance functions here take `a_c` **as an input** so the caller may
//! supply it from a Hertzian mechanical solve (Phase 2), from the geometric
//! overlap, or — for particle–wall contact — from the boundary geometry
//! (Phase 3) without this module depending on those phases.
//!
//! # Honest scope (Phase 4)
//!
//! This module implements **only** solid–solid **contact conduction** and the
//! single-step temperature update. It deliberately does **not** provide:
//!
//! - **thermal radiation** between particles or to walls (Stefan–Boltzmann,
//!   view factors) — a separate, later mode;
//! - **gas/film conduction** through the interstitial fluid or the near-contact
//!   gas gap (e.g. the Batchelor–O'Brien fluid-lens or Rong–Horio correction),
//!   nor any pressure/Knudsen dependence of it;
//! - **convection** or any CFD-DEM fluid coupling (that is Phase 5 / the CFD-DEM
//!   seam) — the surrounding fluid is treated as thermally inert here;
//! - internal temperature gradients within a particle (each particle is a
//!   single lumped, isothermal node — the Biot-number validity limit of the
//!   lumped-capacitance assumption is the caller's responsibility);
//! - any implicit or higher-order time integration (forward Euler only).
//!
//! No cross-code benchmark comparison has been run yet — the tests below are
//! **verification** against hand-computed closed forms (conductance, flux sign
//! and magnitude, energy balance, one Euler step), not **validation** against a
//! reference DEM code or experiment. That validation is the later human step in
//! this bead's Definition of Done.
//!
//! # Unit convention
//!
//! Following [`crate::particle`], the numeric API is plain `f64` in SI base
//! units, with every parameter's unit spelled out in its doc comment: thermal
//! conductivity `k` `[W/m/K]`, contact radius `a_c` and radius `r` `[m]`,
//! overlap `δ` `[m]`, temperature `T` `[K]`, conductance `h_c` `[W/K]`, heat
//! rate `Q` `[W]`, mass `m` `[kg]`, specific heat capacity `c_p` `[J/kg/K]`,
//! time step `dt` `[s]`.
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - G. K. Batchelor and R. W. O'Brien, "Thermal or electrical conduction
//!   through a granular material," *Proc. R. Soc. Lond. A* **355**(1682),
//!   313–333 (1977). — single-contact conductance `h_c = 2·k_s·a_c`.
//! - W. L. Vargas and J. J. McCarthy, "Heat conduction in granular materials,"
//!   *AIChE Journal* **47**(5), 1052–1059 (2001). — DEM particle-scale form of
//!   the contact conductance with harmonic-mean conductivity.
//! - W. L. Vargas and J. J. McCarthy, "Stress effects on the conductivity of
//!   particulate beds," *Chem. Eng. Sci.* **57**(15), 3119–3131 (2002). —
//!   Hertzian contact-radius coupling of conductance to load/overlap.
//! - H. Hertz, "Über die Berührung fester elastischer Körper," *J. reine angew.
//!   Math.* **92**, 156–171 (1882); K. L. Johnson, *Contact Mechanics*
//!   (Cambridge University Press, 1985), §4 — `δ = a_c²/R*`, `R* = r_i r_j/(r_i+r_j)`.

use crate::particle::Particle;
use crate::DemError;

/// Harmonic-mean solid thermal conductivity `k_s` `[W/m/K]` of two contacting
/// materials with conductivities `k_i`, `k_j` `[W/m/K]`.
///
/// `k_s = 2·k_i·k_j / (k_i + k_j)`. This is the conductivity that makes the two
/// series constriction resistances combine into the single-contact conductance
/// `h_c = 2·k_s·a_c` (Batchelor & O'Brien 1977; Vargas & McCarthy 2001). For
/// equal conductivities it reduces to their common value `k_s = k`.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if either conductivity is not strictly
/// positive (a non-positive thermal conductivity is unphysical for a solid).
pub fn harmonic_mean_conductivity(k_i: f64, k_j: f64) -> Result<f64, DemError> {
    if !(k_i > 0.0) || !(k_j > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "thermal conductivities must be strictly positive (W/m/K), got k_i={k_i}, k_j={k_j}"
        )));
    }
    Ok(2.0 * k_i * k_j / (k_i + k_j))
}

/// Effective (reduced) contact radius `R*` `[m]` of two spheres of radii
/// `r_i`, `r_j` `[m]`: `R* = r_i·r_j / (r_i + r_j)`.
///
/// This is the standard reduced radius of Hertzian contact (Johnson, *Contact
/// Mechanics*, 1985) and appears in the overlap–contact-radius relation used by
/// [`hertzian_contact_radius`]. For a sphere against a flat wall, pass the
/// wall's radius as `+∞`; the limit `R* → r_i` is recovered by
/// [`sphere_wall_effective_radius`].
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if either radius is not strictly positive.
pub fn effective_radius(r_i: f64, r_j: f64) -> Result<f64, DemError> {
    if !(r_i > 0.0) || !(r_j > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "sphere radii must be strictly positive (m), got r_i={r_i}, r_j={r_j}"
        )));
    }
    Ok(r_i * r_j / (r_i + r_j))
}

/// Effective (reduced) contact radius `R*` `[m]` of a sphere of radius `r`
/// against a **flat wall**: the flat is the `r_wall → ∞` limit of
/// [`effective_radius`], for which `R* = r`.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if `r` is not strictly positive.
pub fn sphere_wall_effective_radius(r: f64) -> Result<f64, DemError> {
    if !(r > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "sphere radius must be strictly positive (m), got r={r}"
        )));
    }
    Ok(r)
}

/// Hertzian elastic contact radius `a_c` `[m]` for two spheres of radii `r_i`,
/// `r_j` `[m]` pressed together with mutual approach (overlap) `overlap` `[m]`.
///
/// Uses the Hertz relation `δ = a_c²/R*` ⇒ `a_c = sqrt(R*·δ)`, with the
/// effective radius `R* = r_i·r_j/(r_i+r_j)` from [`effective_radius`] (Hertz
/// 1882; Johnson, *Contact Mechanics*, 1985). Valid for small overlaps
/// `δ ≪ r_i, r_j` (the Hertzian small-strain assumption); at zero overlap the
/// contact radius — and hence the conductance — is zero.
///
/// A geometric truncation of two overlapping rigid spheres would instead give
/// `a_c = sqrt(2·R*·δ)`; the elastic value returned here is smaller by a factor
/// `sqrt(2)` because the surfaces deform. Callers wanting the geometric value
/// can scale by `sqrt(2)`.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if either radius is not strictly positive
/// or if `overlap` is negative (a negative overlap means the spheres are not in
/// contact, so no contact radius is defined). Zero overlap returns `0.0`.
pub fn hertzian_contact_radius(r_i: f64, r_j: f64, overlap: f64) -> Result<f64, DemError> {
    if overlap < 0.0 {
        return Err(DemError::InvalidInput(format!(
            "overlap must be non-negative (m); a negative overlap means no contact, got {overlap}"
        )));
    }
    let r_eff = effective_radius(r_i, r_j)?;
    Ok((r_eff * overlap).sqrt())
}

/// A contact-conduction thermal model.
///
/// Enum dispatch (no trait objects), per the workspace design rules: the set of
/// conductance laws is closed and known at compile time, so adding a variant
/// forces every `match` to handle it. The single method
/// [`ThermalModel::conductance`] maps a contact radius to a pair conductance
/// `h_c` `[W/K]`.
///
/// The same model type serves both particle–particle and particle–wall contact:
/// for a wall, build [`ThermalModel::ContactConduction`] from the particle's and
/// the wall's conductivities and pass the sphere–wall contact radius (e.g. from
/// [`hertzian_contact_radius`] with [`sphere_wall_effective_radius`], or from
/// Phase 3 boundary geometry).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalModel {
    /// Batchelor–O'Brien contact conduction. Stores the two solid thermal
    /// conductivities `k_i`, `k_j` `[W/m/K]`; the conductance for a contact of
    /// radius `a_c` `[m]` is `h_c = 2·k_s·a_c` with `k_s` the harmonic mean.
    ContactConduction {
        /// Thermal conductivity of body `i` `[W/m/K]` (a particle).
        k_i: f64,
        /// Thermal conductivity of body `j` `[W/m/K]` (the touching particle or wall).
        k_j: f64,
    },
    /// A directly-prescribed constant pair conductance `[W/K]`, independent of
    /// contact radius. Useful for a fixed-conductance boundary condition or for
    /// isolating the temperature-update logic in tests. The stored value is the
    /// conductance `h_c` itself.
    Constant(f64),
}

impl ThermalModel {
    /// Pair conductance `h_c` `[W/K]` for a contact of radius
    /// `contact_radius` `[m]`.
    ///
    /// - [`ThermalModel::ContactConduction`] returns `h_c = 2·k_s·a_c`, with
    ///   `k_s` the harmonic-mean conductivity ([`harmonic_mean_conductivity`]).
    /// - [`ThermalModel::Constant`] returns its stored conductance and **ignores**
    ///   `contact_radius`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `contact_radius` is negative, or if
    /// a [`ThermalModel::ContactConduction`] has a non-positive conductivity
    /// (surfaced via [`harmonic_mean_conductivity`]). A [`ThermalModel::Constant`]
    /// conductance is returned as-is (its sign is the caller's responsibility).
    pub fn conductance(&self, contact_radius: f64) -> Result<f64, DemError> {
        match self {
            Self::ContactConduction { k_i, k_j } => {
                if contact_radius < 0.0 {
                    return Err(DemError::InvalidInput(format!(
                        "contact radius must be non-negative (m), got {contact_radius}"
                    )));
                }
                let k_s = harmonic_mean_conductivity(*k_i, *k_j)?;
                Ok(2.0 * k_s * contact_radius)
            }
            Self::Constant(h_c) => Ok(*h_c),
        }
    }
}

/// Conductive heat rate `Q` `[W]` flowing **into** the body at temperature
/// `t_into` from a body at temperature `t_from`, through a contact of
/// conductance `h_c` `[W/K]`: `Q = h_c·(t_from − t_into)`.
///
/// The sign convention is deliberate: `Q > 0` when `t_from > t_into` (heat
/// flows into the colder body), `Q = 0` at equal temperatures, and swapping the
/// two temperatures negates `Q` — so for a particle pair the two half-rates are
/// equal and opposite and a two-body exchange conserves energy. Temperatures in
/// `[K]` (any consistent absolute scale works since only their difference
/// enters).
///
/// This one function serves both particle–particle conduction (pass the two
/// particle temperatures) and particle–wall conduction (pass the particle
/// temperature as `t_into` and the prescribed wall temperature as `t_from`).
#[must_use]
pub fn conductive_heat_rate(h_c: f64, t_into: f64, t_from: f64) -> f64 {
    h_c * (t_from - t_into)
}

/// Conductive heat rate `Q` `[W]` flowing **into particle `i`** from a touching
/// particle `j`, for the given `model` and contact radius `a_c` `[m]`.
///
/// Convenience wrapper over [`ThermalModel::conductance`] +
/// [`conductive_heat_rate`] that reads the two particles' `temperature` fields:
/// `Q = h_c·(T_j − T_i)` with `h_c = model.conductance(a_c)`. The equal-and-
/// opposite rate into `j` is obtained by swapping the arguments (or negating).
///
/// # Errors
///
/// Propagates [`DemError::InvalidInput`] from [`ThermalModel::conductance`]
/// (negative contact radius or non-positive conductivity).
pub fn particle_pair_heat_rate(
    model: &ThermalModel,
    contact_radius: f64,
    particle_i: &Particle,
    particle_j: &Particle,
) -> Result<f64, DemError> {
    let h_c = model.conductance(contact_radius)?;
    Ok(conductive_heat_rate(
        h_c,
        particle_i.temperature,
        particle_j.temperature,
    ))
}

/// Conductive heat rate `Q` `[W]` flowing **into a particle** from a boundary
/// held at the prescribed wall temperature `wall_temperature` `[K]`, for the
/// given `model` and sphere–wall contact radius `a_c` `[m]`.
///
/// Same form as [`particle_pair_heat_rate`], with the wall as an isothermal
/// reservoir: `Q = h_c·(T_wall − T_particle)`. The wall's finite heat capacity
/// is not tracked — it is treated as a fixed-temperature source/sink, the usual
/// Dirichlet thermal boundary condition. Build `model` as a
/// [`ThermalModel::ContactConduction`] from the particle and wall conductivities
/// (or a [`ThermalModel::Constant`] wall conductance), and obtain `a_c` from the
/// boundary geometry (Phase 3) or [`hertzian_contact_radius`] with
/// [`sphere_wall_effective_radius`].
///
/// # Errors
///
/// Propagates [`DemError::InvalidInput`] from [`ThermalModel::conductance`].
pub fn wall_heat_rate(
    model: &ThermalModel,
    contact_radius: f64,
    particle: &Particle,
    wall_temperature: f64,
) -> Result<f64, DemError> {
    let h_c = model.conductance(contact_radius)?;
    Ok(conductive_heat_rate(
        h_c,
        particle.temperature,
        wall_temperature,
    ))
}

/// New temperature `[K]` after one explicit forward-Euler step of the lumped
/// energy balance `dT/dt = Q_net / (m·c_p)`:
/// `T(t+dt) = T + Q_net·dt / (m·c_p)`.
///
/// - `temperature` — current lumped particle temperature `[K]`.
/// - `net_heat_rate` — net conductive heat rate `Q_net` `[W]` into the particle
///   (the sum `Σ Q` over all its contacts; positive heats it).
/// - `mass` — particle mass `[kg]` (`> 0`).
/// - `specific_heat` — specific heat capacity `c_p` `[J/kg/K]` (`> 0`).
/// - `dt` — time step `[s]` (`> 0`).
///
/// This is a **total** function mirroring [`Particle::integrate`]: it does not
/// return a `Result` and assumes the documented preconditions (`mass > 0`,
/// `c_p > 0`, `dt > 0`). Being explicit (first-order), it is only stable for a
/// step below the thermal relaxation limit — for a single contact of
/// conductance `h_c`, roughly `dt < m·c_p / h_c`; larger steps can overshoot
/// and oscillate. Use [`apply_temperature_step`] to write the result straight
/// back into a [`Particle`].
#[must_use]
pub fn explicit_euler_temperature(
    temperature: f64,
    net_heat_rate: f64,
    mass: f64,
    specific_heat: f64,
    dt: f64,
) -> f64 {
    debug_assert!(mass > 0.0, "mass must be strictly positive");
    debug_assert!(specific_heat > 0.0, "specific heat must be strictly positive");
    temperature + net_heat_rate * dt / (mass * specific_heat)
}

/// Advance a particle's `temperature` field one explicit forward-Euler step
/// under the net conductive heat rate `net_heat_rate` `[W]`, using the
/// particle's own `mass` `[kg]` and the supplied specific heat capacity
/// `specific_heat` `[J/kg/K]` over the step `dt` `[s]`.
///
/// In-place counterpart of [`explicit_euler_temperature`]; the temperature
/// update is `T += Q_net·dt / (m·c_p)`. Specific heat is passed per call rather
/// than stored on [`Particle`] because it is a material property that Phase 1's
/// particle model does not carry. Same stability caveat as
/// [`explicit_euler_temperature`].
pub fn apply_temperature_step(
    particle: &mut Particle,
    net_heat_rate: f64,
    specific_heat: f64,
    dt: f64,
) {
    particle.temperature = explicit_euler_temperature(
        particle.temperature,
        net_heat_rate,
        particle.mass,
        specific_heat,
        dt,
    );
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

    /// Helper: a valid particle at rest at the origin with a chosen mass,
    /// radius, and temperature.
    fn particle_at(mass_kg: f64, radius_m: f64, temperature_k: f64) -> Particle {
        Particle::new(
            Vec3::zero(),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(mass_kg),
            Length::new::<meter>(radius_m),
            ThermodynamicTemperature::new::<kelvin>(temperature_k),
        )
        .expect("valid particle")
    }

    /// V&V — **harmonic-mean conductivity closed form**.
    ///
    /// Methodology: check `k_s = 2·k_i·k_j/(k_i+k_j)` against hand values for an
    /// equal pair and a dissimilar pair. Pass criterion: agreement to `1e-12`.
    ///
    /// Result (measured 2026-07-23): equal pair `k_i=k_j=400` → `k_s=400.0`
    /// exactly; dissimilar pair `k_i=100, k_j=300` → `k_s = 2·100·300/400 =
    /// 150.0`. Both reproduced to `< 1e-13` (round-off). Confirms the series-
    /// resistance harmonic mean and its equal-conductivity limit.
    #[test]
    fn harmonic_mean_conductivity_closed_form() {
        let k_equal = harmonic_mean_conductivity(400.0, 400.0).unwrap();
        assert_abs_diff_eq!(k_equal, 400.0, epsilon = 1.0e-12);

        let k_diss = harmonic_mean_conductivity(100.0, 300.0).unwrap();
        assert_abs_diff_eq!(k_diss, 150.0, epsilon = 1.0e-12);

        // Non-positive conductivity rejected.
        assert!(harmonic_mean_conductivity(0.0, 300.0).is_err());
        assert!(harmonic_mean_conductivity(100.0, -1.0).is_err());
    }

    /// V&V — **Hertzian contact radius and conductance, hand-computed**.
    ///
    /// Methodology: two equal spheres `r_i=r_j=0.01 m` with overlap
    /// `δ=1e-4 m`. Effective radius `R* = r_i r_j/(r_i+r_j) = 0.01·0.01/0.02 =
    /// 0.005 m`, so the Hertzian contact radius is `a_c = sqrt(R*·δ) =
    /// sqrt(0.005·1e-4) = sqrt(5e-7) = 7.0710678e-4 m`. With equal
    /// conductivities `k=400 W/m/K` the harmonic mean is `k_s=400`, so the
    /// conductance is `h_c = 2·k_s·a_c = 2·400·7.0710678e-4 = 0.56568542 W/K`.
    /// Pass criterion: `R*`, `a_c`, `h_c` each within `1e-10` of the hand value.
    ///
    /// Result (measured 2026-07-23): `R* = 0.005 m`, `a_c = 7.071067811865e-4 m`
    /// (= `5e-7` under the root), `h_c = 0.565685424949 W/K` — all reproduced to
    /// `< 1e-12`. Confirms the `δ = a_c²/R*` Hertz relation and `h_c = 2 k_s a_c`.
    #[test]
    fn hertzian_contact_radius_and_conductance_hand_computed() {
        let r = 0.01_f64;
        let delta = 1.0e-4_f64;

        let r_eff = effective_radius(r, r).unwrap();
        assert_abs_diff_eq!(r_eff, 0.005, epsilon = 1.0e-12);

        let a_c = hertzian_contact_radius(r, r, delta).unwrap();
        assert_abs_diff_eq!(a_c, (5.0e-7_f64).sqrt(), epsilon = 1.0e-15);
        assert_abs_diff_eq!(a_c, 7.071_067_811_865_5e-4, epsilon = 1.0e-12);

        let model = ThermalModel::ContactConduction { k_i: 400.0, k_j: 400.0 };
        let h_c = model.conductance(a_c).unwrap();
        assert_abs_diff_eq!(h_c, 2.0 * 400.0 * a_c, epsilon = 1.0e-15);
        assert_abs_diff_eq!(h_c, 0.565_685_424_949_2, epsilon = 1.0e-12);

        // Zero overlap ⇒ zero contact radius ⇒ zero conductance.
        assert_abs_diff_eq!(hertzian_contact_radius(r, r, 0.0).unwrap(), 0.0, epsilon = 1.0e-15);
        // Negative overlap ⇒ no contact ⇒ error.
        assert!(hertzian_contact_radius(r, r, -1.0e-6).is_err());
    }

    /// V&V — **equal temperatures exchange zero heat**.
    ///
    /// Methodology: two particles at the same temperature `T=350 K` with a
    /// non-zero conductance (`h_c = 0.8 W/K`, model
    /// `ContactConduction{400,400}`, `a_c=1e-3 m`). The heat rate must be
    /// identically zero regardless of conductance. Pass criterion: `|Q| < 1e-15`.
    ///
    /// Result (measured 2026-07-23): `Q = 0.0` exactly (the temperature
    /// difference factor is `0`). Confirms no spurious heat flow at thermal
    /// equilibrium.
    #[test]
    fn equal_temperatures_zero_heat() {
        let model = ThermalModel::ContactConduction { k_i: 400.0, k_j: 400.0 };
        let a_c = 1.0e-3_f64;
        assert_abs_diff_eq!(model.conductance(a_c).unwrap(), 0.8, epsilon = 1.0e-12);

        let p_i = particle_at(0.001, 0.01, 350.0);
        let p_j = particle_at(0.001, 0.01, 350.0);
        let q = particle_pair_heat_rate(&model, a_c, &p_i, &p_j).unwrap();
        assert_abs_diff_eq!(q, 0.0, epsilon = 1.0e-15);
    }

    /// V&V — **flux sign and magnitude, hand-computed**.
    ///
    /// Methodology: cold particle `i` at `T_i=300 K`, hot particle `j` at
    /// `T_j=350 K`, conductance `h_c = 0.8 W/K` (`ContactConduction{400,400}`,
    /// `a_c=1e-3 m`). The heat rate into the cold particle must be positive and
    /// equal to `h_c·(T_j−T_i) = 0.8·50 = 40 W`. Pass criterion: within `1e-12`.
    ///
    /// Result (measured 2026-07-23): `Q_into_i = +40.0 W` (heat flows into the
    /// colder particle, correct sign) and `Q_into_j = −40.0 W`, each to
    /// `< 1e-12`. Confirms the flux magnitude `h_c·ΔT` and sign convention.
    #[test]
    fn flux_sign_and_magnitude_hand_computed() {
        let model = ThermalModel::ContactConduction { k_i: 400.0, k_j: 400.0 };
        let a_c = 1.0e-3_f64;

        let p_cold = particle_at(0.001, 0.01, 300.0);
        let p_hot = particle_at(0.001, 0.01, 350.0);

        // Into the cold particle from the hot one: +40 W.
        let q_into_cold = particle_pair_heat_rate(&model, a_c, &p_cold, &p_hot).unwrap();
        assert!(q_into_cold > 0.0, "heat must flow INTO the colder particle");
        assert_abs_diff_eq!(q_into_cold, 40.0, epsilon = 1.0e-12);

        // Into the hot particle from the cold one: -40 W.
        let q_into_hot = particle_pair_heat_rate(&model, a_c, &p_hot, &p_cold).unwrap();
        assert_abs_diff_eq!(q_into_hot, -40.0, epsilon = 1.0e-12);
    }

    /// V&V — **energy conservation in a two-body exchange** (`Q_ij = −Q_ji`).
    ///
    /// Methodology: for an arbitrary dissimilar pair (`T_i=290 K, T_j=410 K`,
    /// model `ContactConduction{k_i=100, k_j=300}`, `a_c=2e-3 m`) the two
    /// half-rates — heat into `i` from `j`, and heat into `j` from `i` — must sum
    /// to zero, since the same conductance and the same temperature difference
    /// (up to sign) drive both. Pass criterion: `|Q_ij + Q_ji| < 1e-12`.
    ///
    /// Result (measured 2026-07-23): with `k_s=150`, `h_c=2·150·2e-3=0.6 W/K`,
    /// `Q_ij = 0.6·(410−290) = +72.0 W` and `Q_ji = −72.0 W`; their sum is
    /// `0.0` to `< 1e-12`. Confirms the antisymmetric flux conserves energy
    /// exactly (no heat created or destroyed at the contact).
    #[test]
    fn energy_conserved_two_body_exchange() {
        let model = ThermalModel::ContactConduction { k_i: 100.0, k_j: 300.0 };
        let a_c = 2.0e-3_f64;
        // h_c = 2 * 150 * 2e-3 = 0.6 W/K
        assert_abs_diff_eq!(model.conductance(a_c).unwrap(), 0.6, epsilon = 1.0e-12);

        let p_i = particle_at(0.002, 0.01, 290.0);
        let p_j = particle_at(0.003, 0.01, 410.0);

        let q_ij = particle_pair_heat_rate(&model, a_c, &p_i, &p_j).unwrap();
        let q_ji = particle_pair_heat_rate(&model, a_c, &p_j, &p_i).unwrap();

        assert_abs_diff_eq!(q_ij, 72.0, epsilon = 1.0e-12);
        assert_abs_diff_eq!(q_ji, -72.0, epsilon = 1.0e-12);
        // Energy balance: the two half-rates cancel.
        assert_abs_diff_eq!(q_ij + q_ji, 0.0, epsilon = 1.0e-12);
    }

    /// V&V — **explicit Euler temperature step, hand-computed**.
    ///
    /// Methodology: a particle of mass `m=0.001 kg` at `T=300 K` receives a net
    /// heat rate `Q_net=40 W` for a step `dt=0.01 s` with specific heat
    /// `c_p=385 J/kg/K` (copper-like). The forward-Euler update is
    /// `ΔT = Q·dt/(m·c_p) = 40·0.01/(0.001·385) = 0.4/0.385 = 1.0389610... K`,
    /// so `T(t+dt) = 301.0389610... K`. Check both the free function
    /// [`explicit_euler_temperature`] and the in-place
    /// [`apply_temperature_step`]. Pass criterion: within `1e-9 K`.
    ///
    /// Result (measured 2026-07-23): `ΔT = 1.038961039 K`,
    /// `T = 301.038961039 K` from both the pure and the in-place helper, to
    /// `< 1e-12`. Confirms `dT = Q·dt/(m·c_p)` and that the in-place step writes
    /// the same value back into `Particle::temperature`.
    #[test]
    fn explicit_euler_temperature_step_hand_computed() {
        let mass = 0.001_f64;
        let c_p = 385.0_f64;
        let q_net = 40.0_f64;
        let dt = 0.01_f64;
        let t0 = 300.0_f64;

        let expected_dt = q_net * dt / (mass * c_p); // 1.0389610389...
        assert_abs_diff_eq!(expected_dt, 0.4 / 0.385, epsilon = 1.0e-15);

        let t_new = explicit_euler_temperature(t0, q_net, mass, c_p, dt);
        assert_abs_diff_eq!(t_new, t0 + expected_dt, epsilon = 1.0e-12);
        assert_abs_diff_eq!(t_new, 301.038_961_038_961, epsilon = 1.0e-9);

        // In-place counterpart writes the same value into the particle.
        let mut p = particle_at(mass, 0.01, t0);
        apply_temperature_step(&mut p, q_net, c_p, dt);
        assert_abs_diff_eq!(p.temperature, t_new, epsilon = 1.0e-15);
    }

    /// V&V — **particle–wall conduction and Constant model**.
    ///
    /// Methodology: (a) a particle at `T=300 K` against a wall at `T_wall=500 K`
    /// with a `Constant(1.5 W/K)` conductance must receive
    /// `Q = 1.5·(500−300) = +300 W`; reversing to a colder wall
    /// (`T_wall=250 K`) gives `Q = 1.5·(250−300) = −75 W` (heat leaves the
    /// particle). (b) The `Constant` model ignores the contact radius: the same
    /// conductance is returned for `a_c=0` and `a_c=5e-3`. Pass criterion:
    /// within `1e-12`.
    ///
    /// Result (measured 2026-07-23): `Q_hot_wall = +300.0 W`,
    /// `Q_cold_wall = −75.0 W`; `Constant` conductance `= 1.5 W/K` at both
    /// contact radii. Confirms the wall boundary uses the same `h_c·ΔT` form
    /// with a prescribed wall temperature, and the radius-independent `Constant`
    /// variant.
    #[test]
    fn particle_wall_and_constant_model() {
        let model = ThermalModel::Constant(1.5);
        // Constant conductance ignores contact radius.
        assert_abs_diff_eq!(model.conductance(0.0).unwrap(), 1.5, epsilon = 1.0e-15);
        assert_abs_diff_eq!(model.conductance(5.0e-3).unwrap(), 1.5, epsilon = 1.0e-15);

        let p = particle_at(0.001, 0.01, 300.0);

        let q_hot_wall = wall_heat_rate(&model, 0.0, &p, 500.0).unwrap();
        assert_abs_diff_eq!(q_hot_wall, 300.0, epsilon = 1.0e-12);

        let q_cold_wall = wall_heat_rate(&model, 0.0, &p, 250.0).unwrap();
        assert_abs_diff_eq!(q_cold_wall, -75.0, epsilon = 1.0e-12);

        // Sphere–wall effective radius is the sphere radius (flat-wall limit).
        assert_abs_diff_eq!(sphere_wall_effective_radius(0.01).unwrap(), 0.01, epsilon = 1.0e-15);
    }
}
