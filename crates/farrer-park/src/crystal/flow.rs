// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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
//
// ---------------------------------------------------------------------------
// Ported from:
//   Project:  PRISMS-Plasticity (prisms-center/plasticity)
//   Source:   src/materialModels/crystalPlasticity/MaterialModels/
//             RateDependentModel/calculatePlasticity.cc
//               line 693  — power-law slip increment
//               lines 645-657, 698 — saturating hardening and the q-matrix
//               lines 700-708 — the clamp of s at the saturation stress
//   Version:  commit ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4 (2026-08-27)
//   Copyright (c) 2016 The Regents of the University of Michigan, PRISMS Center
//   Licence:  LGPL-2.1-or-later upstream; relicensed to GPL-3.0-only here under
//             LGPL-2.1 section 3. See crates/farrer-park/NOTICE and
//             crates/farrer-park/docs/upstream-provenance.md.
//   Model:    S. R. Kalidindi, "Polycrystal plasticity: constitutive modeling
//             and deformation processing", PhD thesis, MIT, 1992 — the
//             reference upstream's own header names.
// ---------------------------------------------------------------------------

//! The **rate-dependent** (viscoplastic) flow rule and the saturating
//! self-and-latent hardening law that drives the slip resistance.
//!
//! # Why rate dependent
//!
//! A rate-**in**dependent crystal plasticity model has to decide which subset
//! of the twelve systems is active, and that active set is not unique: on a
//! symmetric orientation several systems reach the critical resolved shear
//! stress together and the slip rates they carry are determined only up to a
//! null space. Resolving it needs a combinatorial search and a tie-breaking
//! rule. PRISMS-Plasticity ships both kinds; the rate-dependent model replaces
//! the active-set decision with a smooth, invertible, strictly increasing
//! function of the resolved shear stress, so **every** system always slips a
//! little and the local problem is an ordinary smooth root find. That
//! smoothness is also what makes a consistent algorithmic tangent exist in
//! closed form, which is why this crate implements only the rate-dependent
//! model.
//!
//! # Units
//!
//! Resolved shear stress and slip resistance are in pascals; slip and slip
//! increments are dimensionless (metre per metre of shear); the reference slip
//! rate is per second; the rate-sensitivity and hardening exponents and the
//! latent-hardening ratios are dimensionless; time is in seconds.

use crate::crystal::slip::MAX_SLIP_SYSTEMS;
use crate::error::{FemError, Result};

/// The power-law viscoplastic flow rule
///
/// `d gamma = gamma_dot_0 dt |tau / s|^(1/m) sign(tau)`.
///
/// # Reading the parameters
///
/// `m` is the **rate-sensitivity exponent**. Small `m` means a stiff,
/// nearly rate-independent response: at `m = 0.02` the slip rate changes by a
/// factor of `2^50` when `tau/s` doubles, so in practice `tau` never departs
/// far from `s` and the model behaves like a rate-independent one with `s` as
/// the critical resolved shear stress. PRISMS-Plasticity's FCC decks use
/// `m` between `0.02` and `0.1` with `gamma_dot_0 = 1e-3` per second.
///
/// # Units
///
/// `reference_slip_rate` per second, `rate_sensitivity_exponent`
/// dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLawFlow {
    reference_slip_rate: f64,
    rate_sensitivity_exponent: f64,
}

impl PowerLawFlow {
    /// Construct, with both parameters range-checked.
    ///
    /// # Arguments
    ///
    /// - `reference_slip_rate` — `gamma_dot_0` per second, finite and strictly
    ///   positive. `1e-3` in PRISMS-Plasticity's decks.
    /// - `rate_sensitivity_exponent` — `m`, dimensionless, in `(0, 1]`.
    ///   The upper bound is not cosmetic: the derivative of the flow rule is
    ///   proportional to `|tau/s|^(1/m - 1)`, which is unbounded at `tau = 0`
    ///   once `m > 1`, and the local Newton iteration would then have an
    ///   infinite Jacobian entry at every unstressed system. For `m <= 1` the
    ///   derivative at `tau = 0` is finite (zero, when `m < 1`).
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] outside those ranges.
    pub fn new(reference_slip_rate: f64, rate_sensitivity_exponent: f64) -> Result<Self> {
        if !(reference_slip_rate > 0.0) || !reference_slip_rate.is_finite() {
            return Err(FemError::MaterialOutOfRange {
                parameter: "reference_slip_rate",
                value: reference_slip_rate,
                unit: "1/s",
                reason: "must be finite and strictly positive",
            });
        }
        if !(rate_sensitivity_exponent > 0.0 && rate_sensitivity_exponent <= 1.0) {
            return Err(FemError::MaterialOutOfRange {
                parameter: "rate_sensitivity_exponent",
                value: rate_sensitivity_exponent,
                unit: "dimensionless",
                reason: "must satisfy 0 < m <= 1; above 1 the flow-rule derivative \
                         is singular at zero resolved shear stress",
            });
        }
        Ok(Self {
            reference_slip_rate,
            rate_sensitivity_exponent,
        })
    }

    /// Reference slip rate `gamma_dot_0` \[1/s\].
    #[must_use]
    pub fn reference_slip_rate(&self) -> f64 {
        self.reference_slip_rate
    }

    /// Rate-sensitivity exponent `m` \[-\].
    #[must_use]
    pub fn rate_sensitivity_exponent(&self) -> f64 {
        self.rate_sensitivity_exponent
    }

    /// The slip increment `d gamma` \[-\] over a time increment `dt` \[s\] on a
    /// system carrying resolved shear stress `tau` \[Pa\] against slip
    /// resistance `slip_resistance` \[Pa\].
    ///
    /// Signed: positive `tau` drives positive slip. Exactly zero at
    /// `tau = 0`. May legitimately return a very large number, or `+/- inf`,
    /// when `|tau|` is far above `slip_resistance` and `1/m` is large — the
    /// caller's line search is what keeps the local Newton iteration out of
    /// that region, exactly as PRISMS-Plasticity's cubic line search does.
    ///
    /// `slip_resistance` must be strictly positive; it is guaranteed to be by
    /// [`SaturatingHardening`], which never lets a resistance fall to zero.
    #[must_use]
    pub fn slip_increment(&self, tau: f64, slip_resistance: f64, dt: f64) -> f64 {
        let ratio = (tau / slip_resistance).abs();
        if ratio == 0.0 {
            return 0.0;
        }
        self.reference_slip_rate
            * dt
            * ratio.powf(1.0 / self.rate_sensitivity_exponent)
            * tau.signum()
    }

    /// `d(d gamma) / d tau` \[1/Pa\], the exact derivative of
    /// [`PowerLawFlow::slip_increment`] with the slip resistance held fixed.
    ///
    /// Always non-negative — the flow rule is monotone in `tau`, which is what
    /// makes the local Jacobian non-singular.
    #[must_use]
    pub fn slip_increment_derivative(&self, tau: f64, slip_resistance: f64, dt: f64) -> f64 {
        let n = 1.0 / self.rate_sensitivity_exponent;
        let ratio = (tau / slip_resistance).abs();
        if ratio == 0.0 {
            // n > 1 in every admissible case except m = 1 exactly, where the
            // law is linear and the derivative is gdot0 dt / s.
            return if n > 1.0 {
                0.0
            } else {
                self.reference_slip_rate * dt / slip_resistance
            };
        }
        self.reference_slip_rate * dt * n * ratio.powf(n - 1.0) / slip_resistance
    }
}

/// The saturating self-and-latent hardening law
///
/// `d s_a = sum_b q_ab h_b |d gamma_b|`, with `h_b = h_0 (1 - s_b / s_sat)^A`,
///
/// followed by the clamp `s_a <- min(s_a, s_sat)`.
///
/// # Where each piece comes from, and why the clamp is not optional
///
/// The form is Kalidindi's, and the implementation follows
/// PRISMS-Plasticity's `calculatePlasticity.cc` line for line, including two
/// details that matter more than they look:
///
/// 1. **`h_b` is indexed by the system that is slipping**, not by the one
///    being hardened: it is `q[a][b] h(s_b)`, not `q[a][b] h(s_a)`. Upstream
///    writes `h_alpha_beta_t[i][j] = q[i][j]*h_beta(j)` and then accumulates
///    `s(j) += h_alpha_beta_t[j][i] * |delgam(i)|`.
/// 2. **The clamp at `s_sat`.** Upstream applies it unconditionally after the
///    update. Without it `s_b` can overshoot `s_sat`, at which point
///    `(1 - s_b/s_sat)` is negative and `pow(negative, A)` with a
///    non-integer `A` — `2.25` in upstream's own FCC deck — is **NaN**. The
///    clamp is the guard that keeps the law defined, not a cosmetic bound, and
///    it is the single most important thing to carry across from upstream.
///
/// # Units
///
/// `initial_hardening_modulus` and `saturation_resistance` in pascals;
/// `exponent`, `self_ratio` and `latent_ratio` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturatingHardening {
    initial_hardening_modulus: f64,
    saturation_resistance: f64,
    exponent: f64,
    self_ratio: f64,
    latent_ratio: f64,
}

impl SaturatingHardening {
    /// Construct, with every parameter range-checked.
    ///
    /// # Arguments
    ///
    /// - `initial_hardening_modulus` — `h_0` \[Pa\], the hardening rate of a
    ///   virgin system, finite and non-negative. `180 MPa` in upstream's FCC
    ///   deck. Zero gives a perfectly plastic crystal.
    /// - `saturation_resistance` — `s_sat` \[Pa\], the resistance at which
    ///   hardening stops, strictly positive and strictly greater than the
    ///   initial slip resistance the material is built with (checked there, not
    ///   here). `148 MPa` upstream.
    /// - `exponent` — `A` \[-\], finite and non-negative; `2.25` upstream.
    ///   Larger values make the approach to saturation sharper.
    /// - `self_ratio` — `q` for a coplanar pair \[-\], non-negative; `1.0`
    ///   upstream.
    /// - `latent_ratio` — `q` for a non-coplanar pair \[-\], non-negative;
    ///   `1.4` upstream.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] outside those ranges.
    pub fn new(
        initial_hardening_modulus: f64,
        saturation_resistance: f64,
        exponent: f64,
        self_ratio: f64,
        latent_ratio: f64,
    ) -> Result<Self> {
        let check = |parameter: &'static str, value: f64, unit: &'static str, ok: bool,
                     reason: &'static str|
         -> Result<()> {
            if ok && value.is_finite() {
                Ok(())
            } else {
                Err(FemError::MaterialOutOfRange {
                    parameter,
                    value,
                    unit,
                    reason,
                })
            }
        };
        check(
            "initial_hardening_modulus",
            initial_hardening_modulus,
            "Pa",
            initial_hardening_modulus >= 0.0,
            "must be finite and non-negative",
        )?;
        check(
            "saturation_resistance",
            saturation_resistance,
            "Pa",
            saturation_resistance > 0.0,
            "must be finite and strictly positive",
        )?;
        check(
            "hardening_exponent",
            exponent,
            "dimensionless",
            exponent >= 0.0,
            "must be finite and non-negative",
        )?;
        check(
            "self_ratio",
            self_ratio,
            "dimensionless",
            self_ratio >= 0.0,
            "must be finite and non-negative",
        )?;
        check(
            "latent_ratio",
            latent_ratio,
            "dimensionless",
            latent_ratio >= 0.0,
            "must be finite and non-negative",
        )?;
        Ok(Self {
            initial_hardening_modulus,
            saturation_resistance,
            exponent,
            self_ratio,
            latent_ratio,
        })
    }

    /// Initial hardening modulus `h_0` \[Pa\].
    #[must_use]
    pub fn initial_hardening_modulus(&self) -> f64 {
        self.initial_hardening_modulus
    }

    /// Saturation slip resistance `s_sat` \[Pa\].
    #[must_use]
    pub fn saturation_resistance(&self) -> f64 {
        self.saturation_resistance
    }

    /// Hardening exponent `A` \[-\].
    #[must_use]
    pub fn exponent(&self) -> f64 {
        self.exponent
    }

    /// Latent-hardening ratio for a coplanar pair \[-\].
    #[must_use]
    pub fn self_ratio(&self) -> f64 {
        self.self_ratio
    }

    /// Latent-hardening ratio for a non-coplanar pair \[-\].
    #[must_use]
    pub fn latent_ratio(&self) -> f64 {
        self.latent_ratio
    }

    /// The single-system hardening rate `h_b = h_0 (1 - s_b / s_sat)^A`
    /// \[Pa\], the hardening one unit of slip on system `b` generates.
    ///
    /// Returns zero rather than a NaN when `s_b >= s_sat`, which cannot happen
    /// through [`SaturatingHardening::advance`] but can if a caller supplies a
    /// hand-made state.
    #[must_use]
    pub fn single_system_rate(&self, slip_resistance: f64) -> f64 {
        let x = 1.0 - slip_resistance / self.saturation_resistance;
        if x <= 0.0 {
            0.0
        } else {
            self.initial_hardening_modulus * x.powf(self.exponent)
        }
    }

    /// Advance every slip resistance by one step of the hardening law.
    ///
    /// # Arguments
    ///
    /// - `q` — the latent-hardening matrix from
    ///   [`crate::crystal::slip::SlipFamily::latent_hardening_matrix`],
    ///   dimensionless.
    /// - `resistance` — the slip resistances at the **start** of the step
    ///   \[Pa\]; `h_b` is evaluated on these, matching the
    ///   backward-Euler-in-slip, explicit-in-hardening scheme documented on
    ///   [`crate::crystal::CrystalPlasticity`].
    /// - `slip_increments` — the signed slip increments `d gamma_b` \[-\] of
    ///   the step; only their magnitudes enter.
    /// - `n_systems` — how many entries of the fixed-size arrays are live.
    ///
    /// # Returns
    ///
    /// The updated resistances \[Pa\], each clamped at `s_sat`.
    #[must_use]
    pub fn advance(
        &self,
        q: &[[f64; MAX_SLIP_SYSTEMS]; MAX_SLIP_SYSTEMS],
        resistance: &[f64; MAX_SLIP_SYSTEMS],
        slip_increments: &[f64; MAX_SLIP_SYSTEMS],
        n_systems: usize,
    ) -> [f64; MAX_SLIP_SYSTEMS] {
        let mut h = [0.0_f64; MAX_SLIP_SYSTEMS];
        for b in 0..n_systems {
            h[b] = self.single_system_rate(resistance[b]);
        }
        let mut out = *resistance;
        for a in 0..n_systems {
            let mut d = 0.0;
            for b in 0..n_systems {
                d += q[a][b] * h[b] * slip_increments[b].abs();
            }
            out[a] = (resistance[a] + d).min(self.saturation_resistance);
        }
        out
    }
}
