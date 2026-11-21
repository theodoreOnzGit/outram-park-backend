// SPDX-License-Identifier: AGPL-3.0-only
//! **EGSnrc numerics that live in PETIR, in one place** (gh:#1680).
//!
//! Nothing is re-ported here: this module re-exports what PETIR already holds
//! from EGSnrc (AGPL-3.0-or-later upstream), so `punggol::egsnrc` and any
//! other caller can find the EGSnrc generators and tables under one name.
//!
//! | EGSnrc | PETIR |
//! |---|---|
//! | `HEN_HOUSE/src/ranmar.mortran` | [`crate::rng::ranmar`] |
//! | `HEN_HOUSE/src/ranlux.mortran` | [`crate::rng::ranlux`] |
//! | `egs++/egs_alias_table.h` | [`crate::rng::alias_table`] |
//! | `egs++/egs_interpolator.h` | [`crate::interp_egs`] |
//!
//! Re-exports only; the code and its tests are where they were.

pub use crate::interp_egs::EgsInterpolator;
pub use crate::rng::alias_table::{
    AccuracyGoal, AliasMade, AliasTableKind, EgsAliasTable, EgsSimpleAliasTable, egs_alias_make, egs_alias_pick,
    egs_alias_value,
};
pub use crate::rng::ranlux::{Ranlux, RanluxError, RanluxState};
pub use crate::rng::ranmar::{Ranmar, RanmarError, RanmarState};

#[cfg(test)]
mod tests {
    use super::*;

    /// The names resolve and are the same types as the originals.
    #[test]
    fn reexports_are_the_originals() {
        let a: Option<EgsInterpolator> = None;
        let b: Option<crate::interp_egs::EgsInterpolator> = a;
        assert!(b.is_none());
        let k = AliasTableKind::Histogram;
        let made = egs_alias_make(k, &[0.0, 1.0, 2.0], &[1.0, 1.0]);
        let _ = made;
        let _: Option<(Ranlux, Ranmar, RanluxState, RanmarState)> = None;
        let _: Option<(RanluxError, RanmarError, AccuracyGoal)> = None;
    }
}
