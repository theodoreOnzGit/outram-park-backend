// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/readreleases.f90:317 (half-life -> constant),
//                    src/timemanager.f90:261-275 (application to deposited mass)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Radioactive decay of airborne and deposited activity.
//!
//! FLEXPART treats decay as a per-species exponential applied to particle mass
//! and to deposited mass. It is the piece of FLEXPART that makes it a
//! *radionuclide* dispersion model rather than a generic tracer model, and it is
//! why FLEXPART is the right upstream for CHANGI.
//!
//! # Relationship to `boon-lay`
//!
//! `boon-lay`'s `triso_atops_fork` also models radioactive decay, from the
//! TRISO-ATOPS lineage, and its nuclide database carries half-lives from the
//! IAEA Live Chart. That database is the natural source for the half-lives fed
//! into these functions — this module deliberately holds **no nuclide data of
//! its own**, so the two cannot drift apart.

/// Convert a half-life to a decay constant, `lambda = ln2 / t_half`.
///
/// Ports `readreleases.f90:317`, `decay(i) = 0.693147/decay(i)`.
///
/// # Upstream's truncated `ln 2`
///
/// Upstream hard-codes `0.693147`, a six-decimal truncation of
/// `ln 2 = 0.6931471805…`. This port keeps that literal rather than using
/// [`core::f64::consts::LN_2`], so the decay constant matches FLEXPART term for
/// term; substituting the exact value would shift every decay constant by
/// ~2.6e-7 relative and silently break code-to-code agreement. Use
/// [`decay_constant_exact`] where accuracy matters more than fidelity.
///
/// # Arguments
/// - `half_life_seconds` — `t_half`, s. Must be `> 0`.
///
/// # Returns
/// Decay constant `lambda`, s⁻¹.
///
/// # Panics
/// Panics if `half_life_seconds <= 0.0`. Upstream has no guard and would divide
/// by zero, yielding an infinite decay constant that silently annihilates the
/// species.
#[must_use]
pub fn decay_constant(half_life_seconds: f64) -> f64 {
    assert!(
        half_life_seconds > 0.0,
        "half-life must be positive; got {half_life_seconds} s"
    );
    // Upstream's six-decimal truncation of ln 2, kept deliberately -- see the
    // doc comment. `decay_constant_exact` is the accurate alternative.
    #[allow(clippy::approx_constant)]
    const UPSTREAM_LN2: f64 = 0.693_147;
    UPSTREAM_LN2 / half_life_seconds
}

/// As [`decay_constant`], but with the exact `ln 2`.
///
/// Differs from upstream by about 2.6e-7 relative. Offered because the
/// truncation in [`decay_constant`] is a fidelity choice, not a physics one, and
/// a caller doing its own dose work should not be forced to inherit it.
#[must_use]
pub fn decay_constant_exact(half_life_seconds: f64) -> f64 {
    assert!(
        half_life_seconds > 0.0,
        "half-life must be positive; got {half_life_seconds} s"
    );
    core::f64::consts::LN_2 / half_life_seconds
}

/// Surviving fraction after `dt` seconds of decay: `exp(-lambda dt)`.
///
/// Ports the form applied in `timemanager.f90:275`,
/// `exp(-1.*outstep*decay(ks))`.
///
/// # Arguments
/// - `decay_constant` — `lambda`, s⁻¹, as returned by [`decay_constant`]. A
///   value of `0` means a stable species and returns exactly `1`.
/// - `dt_seconds` — elapsed time, s.
///
/// # Returns
/// Surviving fraction in `(0, 1]` for non-negative `dt`.
///
/// # Note on FLEXPART's guard
/// `timemanager.f90:267` applies decay only when `decay(ks) > 0`, treating
/// non-positive constants as "stable" rather than as growth. This function
/// mirrors that: a negative `lambda` would otherwise produce unphysical growth,
/// so it is rejected.
///
/// # Panics
/// Panics if `decay_constant` is negative.
#[must_use]
pub fn surviving_fraction(decay_constant: f64, dt_seconds: f64) -> f64 {
    assert!(
        decay_constant >= 0.0,
        "decay constant must be non-negative; got {decay_constant} s^-1"
    );
    if decay_constant == 0.0 {
        return 1.0;
    }
    (-dt_seconds * decay_constant).exp()
}

/// Apply decay to a mass or activity over `dt` seconds.
///
/// Convenience wrapper: `amount · exp(-lambda dt)`.
///
/// # Arguments
/// - `amount` — mass (kg) or activity (Bq); the function is linear so either
///   works, provided the caller is consistent.
/// - `decay_constant` — `lambda`, s⁻¹.
/// - `dt_seconds` — elapsed time, s.
#[must_use]
pub fn decayed(amount: f64, decay_constant: f64, dt_seconds: f64) -> f64 {
    amount * surviving_fraction(decay_constant, dt_seconds)
}
