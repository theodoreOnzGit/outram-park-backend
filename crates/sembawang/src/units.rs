// SPDX-License-Identifier: GPL-3.0

//! The one place `boon-lay`'s activity type crosses into `changi`'s.
//!
//! # The problem in one paragraph
//!
//! `boon-lay` types activity as `Activity = uom::si::f64::Frequency`, because
//! the physics relation is `A = lambda * N` and with `N` dimensionless that
//! reads `Bq = s^-1 * 1`. `changi` types it as
//! [`uom::si::f64::Radioactivity`], which is the same **dimension** but a
//! different **Rust type**, and which carries a built-in `@curie` unit so the
//! `3.7e10` never has to be written down. Neither is wrong and neither is
//! going to change: `boon-lay`'s alias is code-to-code verified against
//! upstream TRISO-ATOPS and human-signed-off.
//!
//! So a conversion is unavoidable. **It lives here, in this file, and nowhere
//! else in this crate.** A conversion scattered across call sites is how two
//! type systems of the same dimension quietly start disagreeing about which
//! one a given number is in — and because they *are* the same dimension, the
//! compiler would not catch it if someone reached for `.get::<hertz>()` on the
//! wrong one.
//!
//! # Why there is no Bq/s type
//!
//! A becquerel is already `s^-1`, so a release *rate* has dimension `T^-2`.
//! `uom` will happily name that type and no human reads it as a release rate.
//! A release therefore travels as an **activity attached to a window**
//! (`changi`'s [`changi::activity::source::ReleaseWindow`]), and any consumer
//! wanting a rate divides by the window's own duration at the point of use.

use boon_lay::triso_atops_fork::Activity as BoonLayActivity;
use uom::si::f64::{Frequency, Radioactivity, Time};
use uom::si::frequency::hertz;
use uom::si::radioactivity::{becquerel, curie};
use uom::si::time::second;

/// Becquerels per curie, `1 Ci = 3.7e10 Bq`, exact by definition.
///
/// Re-exported from `boon-lay` rather than redefined, so the two cannot drift.
/// Prefer [`from_curies`], which goes through `uom`'s own `@curie` and does not
/// name the constant at all.
pub const BQ_PER_CI: f64 = boon_lay::triso_atops_fork::activities::BQ_PER_CI;

/// `boon-lay` activity (a `Frequency` carrying becquerels) into `changi`'s
/// [`Radioactivity`].
///
/// Numerically the identity — both store becquerels — so this costs nothing at
/// run time. Its whole job is to be the one place the type changes.
#[must_use]
pub fn to_radioactivity(activity: BoonLayActivity) -> Radioactivity {
    Radioactivity::new::<becquerel>(activity.get::<hertz>())
}

/// [`Radioactivity`] back into `boon-lay`'s activity type.
///
/// The inverse of [`to_radioactivity`], for feeding a `changi`-side figure back
/// into a `boon-lay` routine.
#[must_use]
pub fn to_boon_lay_activity(activity: Radioactivity) -> BoonLayActivity {
    Frequency::new::<hertz>(activity.get::<becquerel>())
}

/// An activity given in curies.
///
/// TRISO-ATOPS run files specify per-nuclide inventories in curies, and its
/// output columns are in Ci and Ci/s, so this is the boundary most inventory
/// numbers arrive through. It goes via `uom`'s own `@curie` unit rather than
/// multiplying by [`BQ_PER_CI`].
#[must_use]
pub fn from_curies(curies: f64) -> Radioactivity {
    Radioactivity::new::<curie>(curies)
}

/// An activity read back out in curies, for comparison against TRISO-ATOPS
/// output.
#[must_use]
pub fn in_curies(activity: Radioactivity) -> f64 {
    activity.get::<curie>()
}

/// A decay constant from a half-life, `lambda = ln(2) / t_half`.
///
/// Uses `core`'s [`core::f64::consts::LN_2`], **not** the truncated `0.693147`
/// that `changi::flexpart::decay::decay_constant` carries — that literal is
/// upstream FLEXPART's and is reproduced there deliberately, but it has no
/// business on this path.
///
/// # Panics
/// Panics if `half_life` is not strictly positive.
#[must_use]
pub fn decay_constant(half_life: Time) -> Frequency {
    let t = half_life.get::<second>();
    assert!(t > 0.0, "half-life must be positive; got {t} s");
    Frequency::new::<hertz>(core::f64::consts::LN_2 / t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crossing_is_numerically_the_identity() {
        let a = Frequency::new::<hertz>(4.2e13);
        let r = to_radioactivity(a);
        assert_eq!(r.get::<becquerel>(), 4.2e13);
        assert_eq!(to_boon_lay_activity(r).get::<hertz>(), 4.2e13);
    }

    #[test]
    fn curies_go_through_uoms_own_unit_and_match_boon_lays_constant() {
        let r = from_curies(1.0);
        // uom's @curie and boon-lay's BQ_PER_CI must agree, or two parts of the
        // workspace disagree about the definition of a curie.
        assert!((r.get::<becquerel>() - BQ_PER_CI).abs() < 1.0);
        assert!((in_curies(r) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_decay_constant_uses_exact_ln_two_not_flexparts_truncation() {
        let lambda = decay_constant(Time::new::<second>(189.0));
        let exact = core::f64::consts::LN_2 / 189.0;
        assert_eq!(lambda.get::<hertz>(), exact);
        // FLEXPART's truncated literal differs in the 7th significant figure.
        let truncated = 0.693_147 / 189.0;
        assert!(
            (lambda.get::<hertz>() - truncated).abs() > 0.0,
            "if these were equal the distinction this function exists for would be moot"
        );
    }

    #[test]
    #[should_panic(expected = "half-life must be positive")]
    fn a_zero_half_life_is_rejected() {
        let _ = decay_constant(Time::new::<second>(0.0));
    }
}
