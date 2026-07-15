// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Resolved/unresolved **overlap** correction (`iovl` / `xtot`) for the URR
//! self-shielded lookup — the `iovl == 1` path of `getunr`
//! (`groupr.f90:6958-6991`).
//!
//! # What overlap is
//!
//! Some evaluations have resolved resonances that extend *into* the unresolved
//! energy range (the two treatments overlap). UNRESR marks the affected URR
//! energy points by storing a **negative energy** in the `unr(*)` array
//! ([`crate::groupr::unresolved::UrrEnergyPoint::overlap`]). When `getunr` hits
//! such a point it sets `iovl = 1` and, for the *total* reaction (`ix == 1`),
//! computes the cross-reaction background addition (`groupr.f90:6963,6980`):
//!
//! ```text
//! xtot = sig(1) - sinf
//! ```
//!
//! — the difference between the incoming smooth-file total `sig(1)` and the
//! infinite-dilution URR total `sinf`, i.e. the resolved-resonance contribution
//! already in the background. For every reaction under overlap the sigma-zero
//! interpolation is then done at an **augmented** background
//! (`groupr.f90:6966,6984`):
//!
//! ```text
//! sigb = sigz(is) + xtot
//! ```
//!
//! so the self-shielding sees the true total (resolved + unresolved) background.
//!
//! # Why a separate module
//!
//! [`crate::groupr::unresolved::UnresolvedTable::shield`] evaluates the
//! `iovl == 0` path only (it records the overlap flag but does not apply the
//! `xtot` correction). Applying `xtot` requires **state carried between reaction
//! calls** — NJOY holds `xtot` in a module-global set while processing the total
//! reaction and reuses it for the partials (`groupr.f90:6963,6966`). This module
//! owns that state explicitly in [`OverlapState`] (no globals), keeping the
//! stateless `shield` intact.
//!
//! # Status — LIVE
//!
//! [`shield_with_overlap`] is implemented. It builds on
//! [`UnresolvedTable::overlap_context`] (the `iovl` flag + the absolute
//! infinite-dilution `sinf` `getunr` needs) added to `unresolved.rs` during
//! integration, verified against the Fortran on 2026-07-15 (commit ac5adf5):
//!
//! 1. **The correction is a background augmentation only.** Comparing the
//!    `iovl == 0` and `iovl == 1` branches of `getunr` line-by-line
//!    (`groupr.f90:6963-6969` vs `:6980-6990`), the *only* difference is the
//!    `terpu` sigma-zero argument: `sigb = sigz(is) + xtot` (`:6965,6982`)
//!    instead of `sigb = sigz(is)`. The energy-interpolated `sinf` used in the
//!    combine and the returned `enext` are identical. So the overlap result for
//!    any reaction equals [`UnresolvedTable::shield`] called with each run
//!    dilution shifted by `xtot` — an **exact** reconstruction, given `xtot`.
//!
//! 2. **`xtot = sig(1) - sinf` needs the absolute `sinf`.** In every `getunr`
//!    combine — additive `sig + s - sinf` (`:6967,6988`) and multiplicative
//!    `sig * s / sinf` (`:6968,6989`) — `sinf` **cancels** at the
//!    infinite-dilution node, so [`UnresolvedTable::shield`] alone never exposes
//!    the absolute `sinf`. [`UnresolvedTable::overlap_context`] returns it
//!    directly (mirroring `getunr`'s `sinf` interpolation), which is what makes
//!    the wrapper exact.
//!
//! 3. **Detection uses the per-point overlap flag** (`unr(loc) < 0`,
//!    `:6961,6976`), also surfaced by [`UnresolvedTable::overlap_context`]: a
//!    single last point uses its own flag; an interior energy requires *both*
//!    bracketing points flagged (`iovl == 1`).
//!
//! The wrapper: for the **total** reaction set `state.xtot = background_sig[0] -
//! sinf` and `state.in_overlap` from the context; then for **every** reaction
//! (total included) delegate to [`UnresolvedTable::shield`] with each run
//! dilution augmented by `state.xtot` when `in_overlap` (`sigb = sigz(is) +
//! xtot`), or delegate unchanged otherwise. Because the augmentation only feeds
//! `terpu`'s sigma-zero argument, the augmented `run_dilutions` reproduce the
//! `iovl == 1` branch exactly (point 1).

use crate::groupr::unresolved::{UnrShielded, UnresolvedTable, UrrReaction};
use crate::NjoyError;

/// Cross-call overlap state — the `xtot` background addition `getunr` carries
/// from the total reaction to the partials under resolved/unresolved overlap
/// (`groupr.f90:6963`).
///
/// Construct with [`OverlapState::default`] (no overlap seen yet), pass it by
/// `&mut` through the total reaction first (which sets `xtot`), then through the
/// partials in the same energy step.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlapState {
    /// The resolved-resonance background addition `xtot = sig(1) - sinf` \[barn\]
    /// set while processing the total reaction under overlap; `0` when there is
    /// no overlap at the current energy.
    pub xtot: f64,
    /// Whether the most recent energy point was flagged as an overlap point
    /// (`iovl == 1`). `false` resets `xtot`'s applicability.
    pub in_overlap: bool,
}

/// URR self-shielded lookup **with** the resolved/unresolved overlap correction
/// — the `iovl == 1` path of `getunr` (`groupr.f90:6958-6991`).
///
/// Behaves exactly like [`UnresolvedTable::shield`] when the queried energy is
/// **not** an overlap point; when it is, it augments the per-dilution background
/// by [`OverlapState::xtot`] before the sigma-zero interpolation, per
/// `groupr.f90:6966,6984`.
///
/// # Call order (mirrors `getunr`)
/// For a given energy `e`, call this for the **total** reaction
/// ([`UrrReaction::Total`]) first — it fills `state.xtot` — then for each
/// partial reaction with the *same* `state`, so the partials see the resolved
/// background.
///
/// # Parameters
/// See [`UnresolvedTable::shield`]. `state` carries `xtot` across the reaction
/// calls at one energy.
///
/// # Errors
/// [`NjoyError::EndfParse`] if `background_sig` and `run_dilutions` differ in
/// length, or an interpolation inside [`UnresolvedTable::shield`] fails.
pub fn shield_with_overlap(
    table: &UnresolvedTable,
    reaction: UrrReaction,
    e: f64,
    background_sig: &[f64],
    run_dilutions: &[f64],
    state: &mut OverlapState,
) -> Result<UnrShielded, NjoyError> {
    if background_sig.len() != run_dilutions.len() {
        return Err(NjoyError::EndfParse(
            "getunr: background_sig and run_dilutions differ in length".into(),
        ));
    }
    match table.overlap_context(reaction, e) {
        // Outside the URR range, absent reaction column, or a trivial dilution
        // grid: identical to `shield`'s unchanged early-return (no overlap).
        None => {
            state.in_overlap = false;
            table.shield(reaction, e, background_sig, run_dilutions)
        }
        Some(ctx) => {
            // For the total reaction under overlap, (re)compute the resolved-
            // resonance background addition (groupr.f90:6962,6979).
            if reaction == UrrReaction::Total && ctx.in_overlap {
                state.xtot = background_sig[0] - ctx.sinf;
            }
            state.in_overlap = ctx.in_overlap;
            if ctx.in_overlap {
                // Augment the terpu sigma-zero argument by xtot for every
                // reaction (total included) — groupr.f90:6965,6982.
                let augmented: Vec<f64> =
                    run_dilutions.iter().map(|&s| s + state.xtot).collect();
                table.shield(reaction, e, background_sig, &augmented)
            } else {
                table.shield(reaction, e, background_sig, run_dilutions)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endf::interp::IntLaw;
    use crate::groupr::unresolved::{LssfFlag, UrrEnergyPoint};

    /// A two-reaction (Total + Elastic) additive table. Points at 10/20/30 eV;
    /// the 20 and 30 eV points are flagged as resolved/unresolved overlap, so an
    /// energy in `[20, 30]` sees `iovl == 1` (both bracketing points flagged),
    /// while `[10, 20]` does not (10 eV is not flagged → the AND is false).
    fn overlap_table() -> UnresolvedTable {
        let sigma0 = vec![1.0e10, 100.0, 10.0, 1.0];
        let points = vec![
            UrrEnergyPoint {
                energy_ev: 10.0,
                overlap: false,
                xs: vec![vec![10.0, 9.0, 7.0, 4.0], vec![8.0, 7.0, 5.0, 3.0]],
            },
            UrrEnergyPoint {
                energy_ev: 20.0,
                overlap: true,
                xs: vec![vec![20.0, 18.0, 14.0, 8.0], vec![16.0, 14.0, 10.0, 6.0]],
            },
            UrrEnergyPoint {
                energy_ev: 30.0,
                overlap: true,
                xs: vec![vec![30.0, 27.0, 21.0, 12.0], vec![24.0, 21.0, 15.0, 9.0]],
            },
        ];
        UnresolvedTable::store(300.0, LssfFlag::Additive, IntLaw::LinLin, sigma0, points)
            .expect("store")
    }

    /// **Non-overlap energy leaves the correction inert.** At `e = 15 eV`
    /// (interval `[10, 20]`, where the 10 eV point is *not* overlap-flagged so
    /// `iovl == 0`), `shield_with_overlap` must equal the plain
    /// [`UnresolvedTable::shield`] exactly and set `state.in_overlap = false`.
    ///
    /// **Methodology.** Query Total at 15 eV, dilution 10 barn, background 12
    /// barn, on `overlap_table()`; compare to `table.shield(...)`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `shield_with_overlap` == `shield`
    /// to `< 1e-12`; `state.in_overlap == false`; both `next_energy = Some(20.0)`.
    #[test]
    fn non_overlap_energy_matches_plain_shield() {
        let t = overlap_table();
        let mut state = OverlapState::default();
        let got = shield_with_overlap(&t, UrrReaction::Total, 15.0, &[12.0], &[10.0], &mut state)
            .expect("shield_with_overlap");
        let plain = t.shield(UrrReaction::Total, 15.0, &[12.0], &[10.0]).expect("shield");
        assert_eq!(got, plain, "non-overlap must match plain shield");
        assert!(!state.in_overlap, "no overlap at 15 eV");
    }

    /// **Overlap augments the sigma-zero background by `xtot` for the partials.**
    /// At `e = 25 eV` (interval `[20, 30]`, both points overlap-flagged →
    /// `iovl == 1`), the Total reaction sets `xtot = sig(1) - sinf`, and a
    /// subsequent partial (Elastic) self-shields at the augmented background
    /// `sigma_0 + xtot` — exactly `getunr`'s `iovl == 1` path
    /// (`groupr.f90:6962-6990`).
    ///
    /// **Methodology.** `overlap_table()` (additive, `lssf == 0`). The Total
    /// infinite-dilution column is `[20, 30]` at `[20, 30] eV`, so the
    /// energy-interpolated `sinf` at 25 eV is `25 barn`. Feed Total with
    /// background `sig(1) = 28`, dilution `sigma_0 = 10` → `xtot = 28 - 25 = 3`.
    /// Then feed Elastic (background 5, dilution 10) with the *same* state and
    /// assert it equals `table.shield(Elastic, 25, [5], [10 + 3])` — the
    /// augmented-dilution reconstruction the port relies on. Also assert the
    /// augmentation *reduces* self-shielding vs the un-augmented
    /// `shield(Elastic, 25, [5], [10])` (a larger effective dilution → the
    /// partial cross section moves toward its infinite-dilution value).
    ///
    /// **Result (2026-07-15, commit ac5adf5):** _measured below (asserted by
    /// equality to the reference `shield` call + the direction check)._
    #[test]
    fn overlap_augments_partial_reaction_background() {
        let t = overlap_table();
        let mut state = OverlapState::default();

        // Total first: sets xtot = 28 - sinf(25 eV) = 28 - 25 = 3.
        let ctx = t
            .overlap_context(UrrReaction::Total, 25.0)
            .expect("overlap context in URR range");
        assert!(ctx.in_overlap, "both bracketing points flagged → iovl==1");
        assert!((ctx.sinf - 25.0).abs() < 1e-12, "sinf(25 eV) = {}", ctx.sinf);

        let total = shield_with_overlap(&t, UrrReaction::Total, 25.0, &[28.0], &[10.0], &mut state)
            .expect("total shield_with_overlap");
        assert!((state.xtot - 3.0).abs() < 1e-12, "xtot = {}", state.xtot);
        assert!(state.in_overlap);
        // Total is itself shielded at the augmented dilution 10 + 3 = 13.
        let total_ref = t.shield(UrrReaction::Total, 25.0, &[28.0], &[13.0]).expect("ref");
        assert_eq!(total, total_ref, "total uses augmented dilution");

        // Partial (Elastic) with the same state: augmented dilution 13 barn.
        let elastic =
            shield_with_overlap(&t, UrrReaction::Elastic, 25.0, &[5.0], &[10.0], &mut state)
                .expect("elastic shield_with_overlap");
        let elastic_ref = t.shield(UrrReaction::Elastic, 25.0, &[5.0], &[13.0]).expect("ref");
        assert_eq!(elastic, elastic_ref, "partial uses augmented dilution");

        // Direction: augmenting the dilution (10 -> 13) reduces self-shielding,
        // so the additive correction `s - sinf` is less negative (closer to the
        // infinite-dilution background) than the un-augmented shield.
        let elastic_plain = t.shield(UrrReaction::Elastic, 25.0, &[5.0], &[10.0]).expect("plain");
        assert!(
            elastic.sig[0] >= elastic_plain.sig[0] - 1e-12,
            "augmented (less shielded) {} should be >= un-augmented {}",
            elastic.sig[0],
            elastic_plain.sig[0]
        );
        println!(
            "overlap: xtot={:.3}, elastic augmented sig={:.4} (plain {:.4})",
            state.xtot, elastic.sig[0], elastic_plain.sig[0]
        );
    }

    /// A length mismatch between `background_sig` and `run_dilutions` is a
    /// recoverable [`NjoyError::EndfParse`], not a panic.
    #[test]
    fn overlap_rejects_length_mismatch() {
        let t = overlap_table();
        let mut state = OverlapState::default();
        assert!(matches!(
            shield_with_overlap(&t, UrrReaction::Total, 25.0, &[1.0, 2.0], &[10.0], &mut state),
            Err(NjoyError::EndfParse(_))
        ));
    }
}
