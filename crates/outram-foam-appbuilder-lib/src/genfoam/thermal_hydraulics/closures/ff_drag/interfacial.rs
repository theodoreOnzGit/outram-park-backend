// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dragModels/
//             FFDragCoefficientModels/ (Wallis, SchillerNaumann, Bestion,
//             BestionTRACE, Autruffe, NoKazimi); base class
//             FFDragCoefficientModel.C/.H
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (sradman@protonmail.com; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Fluid-fluid interfacial drag coefficient correlations
//!
//! Port of GeN-Foam's `FFDragCoefficientModels` family (base class
//! `FFDragCoefficientModel`). Each model maps the local two-fluid flow state
//! (vapour fraction, interfacial Reynolds number, phase densities, hydraulic
//! diameters, velocity magnitudes) to a **dimensionless interfacial drag
//! coefficient** `Cd`, used by GeN-Foam's `FFDragFactor::correctField` as
//!
//! ```text
//! Kd = (alpha1 + alpha2) * (0.5 / Dh_dispersed) * rho_continuous * |Ur| * Cd
//! ```
//!
//! Assembling `Kd` (the fluid-fluid interphase momentum-exchange coefficient)
//! from `Cd` and the mesh/field state is the solver's job (op-p6p.7.11) and
//! out of scope here; this module ports the **pure-algebra `Cd = value(celli)`
//! correlations**, which read only local scalar/vector-magnitude quantities
//! and are therefore independently verifiable. Several upstream models fold
//! the outer `0.5` into their own returned constant — see the
//! upstream-comment note reproduced on [`FfDragCoefficient::Wallis`],
//! [`FfDragCoefficient::Autruffe`], and [`FfDragCoefficient::NoKazimi`].
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Regime |
//! |---|---|---|
//! | [`FfDragCoefficient::Wallis`] | `Wallis` | Bubbly / churn-turbulent liquid-gas (Wallis 1969) |
//! | [`FfDragCoefficient::SchillerNaumann`] | `SchillerNaumann` | Single-sphere drag (Schiller & Naumann 1933) |
//! | [`FfDragCoefficient::Bestion`] | `Bestion` | Stratified/annular liquid-gas (Bestion 1990, CATHARE) |
//! | [`FfDragCoefficient::BestionTrace`] | `BestionTRACE` | `Bestion` variant used by TRACE |
//! | [`FfDragCoefficient::Autruffe`] | `Autruffe` | Bubbly liquid-gas |
//! | [`FfDragCoefficient::NoKazimi`] | `NoKazimi` | Rod-bundle subchannel void (No & Kazimi) |
//!
//! Evaluate with [`FfDragCoefficient::drag_coefficient`], which takes an
//! [`FfInterfacialState`] bundling the per-cell quantities upstream reads
//! from `FFPair`/`fluid`; not every model reads every field (see each
//! variant's rustdoc for exactly which fields it uses).
//!
//! All six models require the pair to be a liquid-gas system; GeN-Foam
//! enforces this with a `FatalError` in each model's constructor (checking
//! `fluid1.isLiquid() && fluid2.isGas()` or vice versa). That dictionary/
//! phase-registration check belongs to the solver's model-construction step,
//! not this pure-algebra port, and is therefore not reproduced here — callers
//! are responsible for only invoking these correlations on liquid-gas pairs.
//!
//! ## References
//!
//! - G.B. Wallis, *One-Dimensional Two-Phase Flow*, McGraw-Hill, 1969.
//! - L. Schiller, A. Naumann, "Über die grundlegenden Berechnungen bei der
//!   Schwerkraftaufbereitung", *Z. Ver. Deutsch. Ing.* 77, 1933, pp. 318-320.
//! - D. Bestion, "The physical closure laws in the CATHARE code", *Nucl. Eng.
//!   Des.* 124, 1990, pp. 229-245.
//! - H. No, M.S. Kazimi, subchannel void-fraction/interfacial-drag closure
//!   (MIT report basis for the upstream `NoKazimi` model).

use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;
use uom::si::f64::{Length, MassDensity, Ratio, ReciprocalLength, Velocity};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::velocity::meter_per_second;

/// OpenFOAM's documented `SMALL` scalar epsilon (`1e-15`), used upstream to
/// guard divisions that would otherwise be `0/0` in the single-phase limit.
/// Not a `uom` dependency; a bare numerical-safety constant reproduced from
/// OpenFOAM's `scalar.H` convention.
const OPENFOAM_SMALL: f64 = 1.0e-15;

/// Local two-fluid interfacial-drag state at a single mesh cell.
///
/// Bundles exactly the per-cell physical quantities the
/// [`FfDragCoefficient`] correlations read from GeN-Foam's `FFPair`/`fluid`
/// per-cell fields. Not every model uses every field — see each variant's
/// rustdoc on [`FfDragCoefficient`] for which subset it reads. Callers
/// (the solver) build this once per cell from mesh/field data and hand the
/// same struct to whichever model is configured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FfInterfacialState {
    /// Vapour volume fraction normalized within the fluid pair,
    /// `alpha_vapour / (alpha_vapour + alpha_liquid)` — dimensionless,
    /// `[0, 1]`. GeN-Foam's `fluid::normalized()` evaluated on the vapour
    /// phase.
    pub vapour_fraction: Ratio,
    /// Interfacial Reynolds number `Re = |U_r| * D_h,dispersed / nu_continuous`,
    /// clamped upstream to a minimum `minRe_`. GeN-Foam's `FFPair::Re()`.
    pub reynolds_number: ReynoldsNumber,
    /// Vapour-phase density `rho_vapour`.
    pub rho_vapour: MassDensity,
    /// Liquid-phase density `rho_liquid`.
    pub rho_liquid: MassDensity,
    /// Continuous-phase density (whichever of vapour/liquid is locally the
    /// continuous phase). GeN-Foam's `FFPair::rhoContinuous()`.
    pub rho_continuous: MassDensity,
    /// Hydraulic (characteristic) diameter of the dispersed phase.
    /// GeN-Foam's `FFPair::DhDispersed()`.
    pub dh_dispersed: Length,
    /// Hydraulic (characteristic) diameter of the continuous phase.
    /// GeN-Foam's `FFPair::DhContinuous()`.
    pub dh_continuous: Length,
    /// Vapour-phase velocity magnitude `|U_vapour|`.
    pub u_vapour: Velocity,
    /// Liquid-phase velocity magnitude `|U_liquid|`.
    pub u_liquid: Velocity,
    /// Relative (slip) velocity magnitude `|U_vapour - U_liquid|`.
    /// GeN-Foam's `FFPair::magUr()`.
    pub u_relative: Velocity,
}

/// Fluid-fluid interfacial drag coefficient correlation.
///
/// Closed enum port of GeN-Foam's `FFDragCoefficientModel` run-time-selectable
/// family. Evaluate with [`FfDragCoefficient::drag_coefficient`], which takes
/// an [`FfInterfacialState`] and returns the dimensionless drag coefficient
/// `Cd` as a [`Ratio`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfDragCoefficient {
    /// Wallis (1969) bubbly/churn-turbulent correlation, a function of vapour
    /// fraction only: `Cd = 0.02 sqrt(a) (1 + 150(1 - sqrt(a)))`.
    ///
    /// Upstream note: "The coeff is 0.01 but, the `FFDragFactor` multiplies
    /// this value times 0.5, so the coeff here is doubled" — i.e. the `0.02`
    /// below is `2 x 0.01`, preserved verbatim from the source.
    Wallis,
    /// Schiller & Naumann (1933) single-sphere drag law, scaled `1.5x` by
    /// GeN-Foam (undocumented upstream; preserved faithfully):
    /// `Cd = 1.5 * (Re < 1000 ? 24(1+0.15 Re^0.687)/Re : 0.44)`.
    SchillerNaumann,
    /// Bestion (1990) CATHARE stratified/annular correlation. Needs vapour
    /// fraction, both hydraulic diameters, both densities, both phase
    /// velocity magnitudes, and the relative velocity magnitude.
    Bestion,
    /// `Bestion` variant used by TRACE — algebraically simpler, drops the
    /// velocity-slip `Ps` factor. Needs vapour fraction, both hydraulic
    /// diameters, vapour density, and continuous-phase density.
    BestionTrace,
    /// Autruffe et al. bubbly-flow correlation with a `0.005` floor clamp on
    /// the vapour-fraction factor.
    ///
    /// Upstream note: "The coeff is 2.155 but, the `FFDragFactor` multiplies
    /// this value times 0.5, so the coeff here is doubled" — the `4.31`
    /// below is `2 x 2.155`.
    Autruffe,
    /// No & Kazimi rod-bundle subchannel correlation. Carries a
    /// geometry-derived inverse characteristic length; build with
    /// [`FfDragCoefficient::no_kazimi_from_geometry`].
    ///
    /// Upstream note: "The coeff is 0.0025 but ... doubled [to 0.005]. Also,
    /// this model does not use the dispersed phase hydraulic diameter as
    /// char. dimension, rather, it uses its own char. dimension computed via
    /// the A_ thing. For this reason I need to multiply the coeff here by
    /// the dispersed phase dimension, to cancel it out" — reproduced as-is.
    NoKazimi {
        /// Geometry-derived inverse characteristic length `A`
        /// (`m^-1`), from [`FfDragCoefficient::no_kazimi_from_geometry`].
        a: ReciprocalLength,
    },
}

impl FfDragCoefficient {
    /// Evaluate the interfacial drag coefficient at the given local state.
    ///
    /// Faithful translation of each upstream model's `value(celli)` member.
    #[must_use]
    pub fn drag_coefficient(&self, state: &FfInterfacialState) -> Ratio {
        Ratio::new::<ratio>(self.value(state))
    }

    /// Core scalar evaluation, unit-stripped to bare `f64` (SI base units).
    fn value(&self, s: &FfInterfacialState) -> f64 {
        let a_vap = s.vapour_fraction.get::<ratio>();
        match *self {
            Self::Wallis => wallis(a_vap),
            Self::SchillerNaumann => schiller_naumann(s.reynolds_number.get::<ratio>()),
            Self::Bestion => bestion(
                a_vap,
                s.dh_dispersed.get::<meter>(),
                s.dh_continuous.get::<meter>(),
                s.rho_vapour.get::<kilogram_per_cubic_meter>(),
                s.rho_liquid.get::<kilogram_per_cubic_meter>(),
                s.u_vapour.get::<meter_per_second>(),
                s.u_liquid.get::<meter_per_second>(),
                s.u_relative.get::<meter_per_second>(),
            ),
            Self::BestionTrace => bestion_trace(
                a_vap,
                s.dh_dispersed.get::<meter>(),
                s.dh_continuous.get::<meter>(),
                s.rho_vapour.get::<kilogram_per_cubic_meter>(),
                s.rho_continuous.get::<kilogram_per_cubic_meter>(),
            ),
            Self::Autruffe => autruffe(
                a_vap,
                s.rho_vapour.get::<kilogram_per_cubic_meter>(),
                s.rho_continuous.get::<kilogram_per_cubic_meter>(),
                s.dh_dispersed.get::<meter>(),
                s.dh_continuous.get::<meter>(),
            ),
            Self::NoKazimi { a } => no_kazimi(
                a_vap,
                s.dh_dispersed.get::<meter>(),
                a.get::<reciprocal_meter>(),
            ),
        }
    }

    /// Build a [`FfDragCoefficient::NoKazimi`] from rod-bundle geometry,
    /// reproducing GeN-Foam's `A_` coefficient derivation exactly:
    /// `A = 4*pi / (D * (2*sqrt(3)*(P/D)^2 - pi))`.
    ///
    /// `pin_pitch` is the rod-bundle pin pitch `P`; `pin_diameter` is the pin
    /// (rod) outer diameter `D`.
    #[must_use]
    pub fn no_kazimi_from_geometry(pin_pitch: Length, pin_diameter: Length) -> Self {
        let p = pin_pitch.get::<meter>();
        let d = pin_diameter.get::<meter>();
        let ratio_pd = p / d;
        let a = (4.0 * core::f64::consts::PI)
            / (d * (2.0 * 3.0_f64.sqrt() * ratio_pd * ratio_pd - core::f64::consts::PI));
        Self::NoKazimi {
            a: ReciprocalLength::new::<reciprocal_meter>(a),
        }
    }
}

/// Wallis (1969) interfacial drag coefficient, a function of vapour fraction
/// `a` only. See [`FfDragCoefficient::Wallis`] for the doubled-coefficient
/// note.
fn wallis(a_vap: f64) -> f64 {
    let sqrt_a = a_vap.sqrt();
    0.02 * sqrt_a * (1.0 + 150.0 * (1.0 - sqrt_a))
}

/// Schiller & Naumann (1933) single-sphere drag, GeN-Foam's `1.5x`-scaled
/// form. See [`FfDragCoefficient::SchillerNaumann`].
fn schiller_naumann(re: f64) -> f64 {
    1.5 * if re < 1000.0 {
        24.0 * (1.0 + 0.15 * re.powf(0.687)) / re
    } else {
        0.44
    }
}

/// Bestion (1990) CATHARE interfacial drag coefficient. See
/// [`FfDragCoefficient::Bestion`].
#[allow(clippy::too_many_arguments)]
fn bestion(
    a_vap: f64,
    dh_dispersed_m: f64,
    dh_continuous_m: f64,
    rho_vap: f64,
    rho_liq: f64,
    u_vap: f64,
    u_liq: f64,
    u_rel: f64,
) -> f64 {
    let cm = 0.188_f64;
    let co = 1.2 - 0.2 * (rho_vap / rho_liq).sqrt();
    let ps = if co == 1.0 {
        1.0
    } else {
        let numer = (1.0 - co * a_vap) * u_vap / (1.0 - a_vap).max(OPENFOAM_SMALL) - co * u_liq;
        let denom = u_rel.max(OPENFOAM_SMALL);
        ((numer / denom).powi(2)).min(0.05)
    };
    2.0 * a_vap
        * (1.0 - a_vap).powi(3)
        * (dh_dispersed_m / dh_continuous_m)
        * (rho_vap / rho_liq)
        * ps
        / cm.powi(2)
}

/// `BestionTRACE` interfacial drag coefficient — the `Bestion` variant with
/// the velocity-slip `Ps` factor dropped. See
/// [`FfDragCoefficient::BestionTrace`].
fn bestion_trace(
    a_vap: f64,
    dh_dispersed_m: f64,
    dh_continuous_m: f64,
    rho_vap: f64,
    rho_continuous: f64,
) -> f64 {
    let cm = 0.188_f64;
    2.0 * a_vap * (1.0 - a_vap).powi(3) * rho_vap / cm.powi(2) / dh_continuous_m / rho_continuous
        * dh_dispersed_m
}

/// Autruffe et al. bubbly-flow interfacial drag coefficient with a `0.005`
/// floor clamp. See [`FfDragCoefficient::Autruffe`].
fn autruffe(
    a_vap: f64,
    rho_vap: f64,
    rho_continuous: f64,
    dh_dispersed_m: f64,
    dh_continuous_m: f64,
) -> f64 {
    4.31 * ((1.0 - a_vap) * (1.0 + 75.0 * (1.0 - a_vap)))
        .powf(0.95)
        .max(0.005)
        * (rho_vap / rho_continuous)
        * (dh_dispersed_m / dh_continuous_m)
}

/// No & Kazimi rod-bundle subchannel interfacial drag coefficient. See
/// [`FfDragCoefficient::NoKazimi`].
fn no_kazimi(a_vap: f64, dh_dispersed_m: f64, a_coeff_per_m: f64) -> f64 {
    dh_dispersed_m
        * a_coeff_per_m
        * a_vap.min(0.99)
        * ((1.0 - a_vap) / 0.043).min(1.0)
        * 0.005
        * (1.0 + 234.3 * (1.0 - a_vap.sqrt()).powf(1.15))
}
