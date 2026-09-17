// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/part0.f90 (caller context: src/readreleases.f90:332)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Lognormal aerosol size distribution: mass fractions, gravitational settling
//! velocities, Cunningham slip correction and Schmidt-number factors.
//!
//! This is FLEXPART's `part0`, the routine that turns a two-parameter lognormal
//! aerosol description (mass median diameter and geometric standard deviation)
//! into the per-bin quantities the dry-deposition scheme needs.
//!
//! # Reuse note
//!
//! The error function comes from [`petir::specfunc::erf`], **not** from a port
//! of FLEXPART's own `erf.f90`. `petir` is this workspace's core numerics crate
//! and its `erf` is GSL-derived with its own test suite, so porting upstream's
//! Numerical-Recipes-style `erf` would have created a second, less-tested
//! implementation of a function the workspace already has — exactly the
//! duplication the workspace "reuse before porting" rule exists to prevent.
//!
//! This *is* a deliberate numerical divergence from upstream and is measured:
//! see `docs/flexpart-code-to-code.md`.

use super::constants::{GA, NI, PI};

/// Per-bin properties of a lognormal aerosol size distribution.
///
/// Every field is an `NI`-element array (`NI = 11`, from `par_mod.f90`), one
/// entry per diameter class, ordered from the smallest class upward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AerosolBins {
    /// Mass fraction in each diameter class, dimensionless.
    ///
    /// These sum to slightly **less than 1**: the distribution is truncated at
    /// ±3 geometric standard deviations, so the tails outside that range are
    /// discarded rather than redistributed. Upstream does not renormalise, and
    /// neither does this port.
    pub mass_fraction: [f64; NI],
    /// Schmidt-number factor `Sc^(-2/3)` for each class, dimensionless.
    ///
    /// This is the combination the dry-deposition resistance formula consumes
    /// directly, which is why upstream stores the power rather than `Sc` itself.
    pub schmidt_factor: [f64; NI],
    /// Gravitational settling velocity of each class, m/s, as a **positive**
    /// magnitude.
    ///
    /// Stokes' law with the Cunningham slip correction:
    /// `v_s = g rho d^2 C_c / (18 eta)`. Note the caller in `readreleases.f90`
    /// accumulates `vsetaver = -sum(v_s · fract)`, i.e. it flips the sign to
    /// make settling downward; this port keeps `part0`'s own positive
    /// convention and leaves that choice to the caller.
    pub settling_velocity: [f64; NI],
    /// Cunningham slip correction factor of each class, dimensionless.
    ///
    /// # Upstream quirk, preserved and exposed
    ///
    /// `part0.f90` declares `cun` as a **scalar** output but assigns it inside
    /// the per-class loop, so on return it holds **only the last (largest)
    /// class's value**. Its caller, `readreleases.f90:338`, then computes
    /// `cunningham = sum_j(cun · fract_j)` — mass-weighting a value that is not
    /// per-class, which collapses to `cun_last · sum(fract)`.
    ///
    /// The port returns the genuine per-class array here, which is what the
    /// physics needs, and exposes upstream's scalar separately as
    /// [`AerosolBins::upstream_scalar_cunningham`] so the code-to-code test can
    /// still compare like for like.
    pub cunningham: [f64; NI],
}

impl AerosolBins {
    /// The value upstream's scalar `cun` output actually carries: the Cunningham
    /// factor of the **largest** diameter class.
    ///
    /// Provided so the code-to-code verification can reproduce upstream's
    /// return value exactly. Prefer [`AerosolBins::cunningham`] for physics.
    #[must_use]
    pub fn upstream_scalar_cunningham(&self) -> f64 {
        self.cunningham[NI - 1]
    }

    /// Mass-weighted mean settling velocity, m/s, positive downward-magnitude.
    ///
    /// Reproduces `readreleases.f90`'s `vsetaver` up to sign: upstream stores it
    /// negative, this returns the magnitude.
    #[must_use]
    pub fn mean_settling_velocity(&self) -> f64 {
        (0..NI)
            .map(|i| self.settling_velocity[i] * self.mass_fraction[i])
            .sum()
    }
}

/// Split a lognormal aerosol into [`NI`] diameter classes and compute each
/// class's deposition-relevant properties.
///
/// Ports `part0.f90`. The distribution is cut at ±3 geometric standard
/// deviations about the mass median diameter and divided into `NI` equal
/// intervals in `log(d)`; for each interval the mass fraction comes from the
/// error function, and the settling velocity from Stokes' law with the
/// Cunningham slip correction:
///
/// ```text
///   Kn   = 2 lambda / d                       Knudsen number
///   C_c  = 1 + Kn (1.257 + 0.4 exp(-1.1/Kn))  Cunningham slip correction
///   D    = k_B T C_c / (3 pi eta d)           Brownian diffusivity
///   Sc   = nu / D                             Schmidt number
///   v_s  = g rho d^2 C_c / (18 eta)           settling velocity
/// ```
///
/// # Fixed reference state
///
/// Upstream evaluates this at a **fixed** reference state, not at the ambient
/// conditions of the release: `T = 293.15 K`, dynamic viscosity
/// `eta = 1.81e-5 Pa·s`, kinematic viscosity `nu = 0.15e-4 m²/s`, mean free path
/// `lambda = 6.53e-8 m`. The settling velocities this returns are therefore
/// reference-state values; FLEXPART rescales them to ambient conditions later,
/// in `get_settling.f90`. That is worth knowing before using the output
/// directly.
///
/// # Arguments
/// - `mass_median_diameter` — `dquer`, in **micrometres**. Upstream's caller
///   converts from metres immediately before the call
///   (`readreleases.f90:331`), so this unit is the routine's real interface.
/// - `geometric_std_dev` — `dsigma`, dimensionless, `> 1`.
/// - `density` — particle density, kg/m³.
///
/// # Returns
/// Per-class [`AerosolBins`].
///
/// # Panics
/// Panics if `geometric_std_dev <= 1.0` (`log` would be non-positive, making
/// the bin edges meaningless) or if `mass_median_diameter <= 0.0`. Upstream has
/// no such guard and would return garbage or NaN.
#[must_use]
pub fn part0(mass_median_diameter: f64, geometric_std_dev: f64, density: f64) -> AerosolBins {
    assert!(
        mass_median_diameter > 0.0,
        "mass median diameter must be positive (micrometres); got {mass_median_diameter}"
    );
    assert!(
        geometric_std_dev > 1.0,
        "geometric standard deviation must exceed 1; got {geometric_std_dev}"
    );

    /// Reference temperature at which the properties are evaluated, K (`tr`).
    const TR: f64 = 293.15;
    /// Dynamic viscosity of air at the reference state, Pa·s (`myl`).
    const MYL: f64 = 1.81e-5;
    /// Kinematic viscosity of air at the reference state, m²/s (`nyl`).
    const NYL: f64 = 0.15e-4;
    /// Mean free path in air, m (`lam`).
    const LAM: f64 = 6.53e-8;
    /// Boltzmann constant, J/K (`kb`).
    const KB: f64 = 1.38e-23;
    /// Underflow guard used in the slip-correction branch (`eps`).
    const EPS: f64 = 1.2e-38;

    let xdummy = core::f64::consts::SQRT_2 * geometric_std_dev.ln();
    let delta = 6.0 / NI as f64;

    let mut bins = AerosolBins {
        mass_fraction: [0.0; NI],
        schmidt_factor: [0.0; NI],
        settling_velocity: [0.0; NI],
        cunningham: [0.0; NI],
    };

    let mut d01 = mass_median_diameter * geometric_std_dev.powi(-3);
    for i in 0..NI {
        let d02 = d01;
        d01 = mass_median_diameter * geometric_std_dev.powf(-3.0 + delta * (i + 1) as f64);
        let x01 = (d01 / mass_median_diameter).ln() / xdummy;
        let x02 = (d02 / mass_median_diameter).ln() / xdummy;

        // petir's GSL-derived erf; upstream uses its own erf.f90 (see module docs).
        bins.mass_fraction[i] =
            0.5 * (petir::specfunc::erf::erf(x01) - petir::specfunc::erf::erf(x02));

        // Geometric-mean diameter of the class, converted micrometres -> metres.
        let dmean = 1.0e-6 * (0.5 * (d01 * d02).ln()).exp();
        let kn = 2.0 * LAM / dmean;
        // Upstream guards the exponential against underflow rather than letting
        // exp(-1.1/kn) flush to zero on its own.
        let alpha = if (-1.1 / kn) <= EPS.log10() * 10f64.ln() {
            1.257
        } else {
            1.257 + 0.4 * (-1.1 / kn).exp()
        };
        let cun = 1.0 + alpha * kn;
        let dc = KB * TR * cun / (3.0 * PI * MYL * dmean);
        let schmidt = NYL / dc;
        bins.schmidt_factor[i] = schmidt.powf(-2.0 / 3.0);
        bins.settling_velocity[i] = GA * density * dmean * dmean * cun / (18.0 * MYL);
        bins.cunningham[i] = cun;
    }
    bins
}
