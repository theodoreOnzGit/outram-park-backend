// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`circulating`, `circulating_steadystate`, `plate_out`,
//                     `plate_out_steadystate`, `clean_up`, `clean_up_steadystate`,
//                     `release_rate`, `base_activities`, `attenuation_factor`)
//                    trisoatops/trisoatops.py (the `× λ / 3.7e10` Ci conversion,
//                     the `inventory / λ × 3.7e10` atom-count conversion)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Activity bookkeeping — coolant / plate-out / clean-up source terms
//!
//! This module ports the TRISO-ATOPS **coolant activity** functions that turn a
//! per-node fission-product *release rate* into the three primary-loop activity
//! pools — circulating (in the coolant), plate-out (deposited on loop surfaces),
//! and clean-up (removed by the helium-purification system, HPS) — plus the
//! **source-term** functions ([`source_terms`]) that produce the release rate and
//! the graphite hold-up activity from a node's radionuclide inventory.
//!
//! It ports `calculation_functions.py`'s `circulating*`, `plate_out*`,
//! `clean_up*`, `release_rate`, and `base_activities`. The infinite-series
//! `attenuation_factor` used by `base_activities` is already ported in
//! [`crate::triso_atops_fork::release_models::steady_state`].
//!
//! ## Units — the design decision (this is the deferred `uom` pass, op-b4a.2.2)
//!
//! The upstream 1989 NP-MHTGR bookkeeping mixed **atoms**, **atoms/s**,
//! **curies**, and **becquerels** through three hard-coded magic factors:
//! `× 3.7e10` (Ci→Bq), `÷ (1 − e^{−λt})` (birth-rate normalisation), and a
//! trailing `× λ / 3.7e10` at output (atom-count → Ci). This port makes each of
//! those explicit and, where the dimension is genuine, `uom`-checked. The chosen
//! representation:
//!
//! - **Radioactivity / activity → [`Activity`] (a `uom` [`uom::si::f64::Frequency`], SI unit `Bq`).**
//!   A becquerel is *one decay per second*, so activity is dimensionally a
//!   frequency (`s^-1`) — the same dimension this crate already uses for the
//!   decay constant ([`crate::triso_atops_fork::DecayConstant`]). Representing
//!   `Bq` as `Frequency` is therefore consistent with the physics core and is
//!   dimensionally honest: `A = λ N` has units `s^-1 · (dimensionless count) =
//!   s^-1 = Bq`. There is no separate SI base unit for "amount of a decaying
//!   species", so no wrong dimension is being invented. See
//!   [`activity_from_atom_count`].
//! - **Ci↔Bq** is the single documented constant [`BQ_PER_CI`] (`1 Ci = 3.7e10
//!   Bq`), replacing every hard-coded `3.7e10` / `/3.7e10` in the source. Use
//!   [`becquerels_from_curies`] and [`curies_from_becquerels`].
//! - **Atom counts / inventories → a documented plain `f64` count** (see
//!   [`atom_count_from_activity`]). A number of atoms is *dimensionless*; forcing
//!   a `uom` dimension onto it (e.g. `mol`) would be wrong, and — critically —
//!   "atoms/s" is dimensionally identical to a rate constant (`s^-1`), so `uom`
//!   cannot tell a release *rate* from a *decay constant*. The effective-unit
//!   bookkeeping quantities (release rate, source rate, and the three activity
//!   pools) are therefore carried as plain `f64` with their meaning spelled out
//!   in each function's docs, while the genuinely-dimensioned inputs — the decay
//!   constant, the plate-out / clean-up rate constants, and the elapsed time —
//!   are `uom` [`uom::si::f64::Frequency`] / [`uom::si::f64::Time`]. This gives
//!   real dimensional checking exactly where it is meaningful: the sum
//!   `β = λ + k_plate + k_clean` can only add frequencies, and every exponent
//!   `β·t`, `λ·t` is checked to be dimensionless.
//!
//! ### What "effective units" means for the pool quantities
//!
//! Following the upstream convention, the release/source **rates** are carried in
//! becquerels (`atoms/s`) and the three activity **pools** (circulating,
//! plate-out, clean-up) and the graphite hold-up are carried as atom **counts**
//! (`Bq·s = atoms`). Both are `f64`. The final report activity in curies is
//! recovered uniformly by `× λ / 3.7e10` — for a rate this yields `Ci/s`, for a
//! count it yields `Ci`, exactly as the upstream output columns are labelled.
//! [`crate::triso_atops_fork::normal_operation`] performs that final conversion.
//!
//! ## Dimensional-consistency check
//!
//! The relation `activity = decay_constant × atom_count` is verified in this
//! module's tests (both dimensionally, via `uom`, and numerically), and the
//! individual bookkeeping functions are verified against values produced by the
//! upstream Python on the same inputs (data taken 2026-07-15, commit `de374c8`).

pub mod coolant_activity;
pub mod source_terms;

use uom::si::f64::Frequency;
use uom::si::frequency::hertz;

use super::DecayConstant;

pub use coolant_activity::{
    circulating, circulating_steadystate, clean_up, clean_up_steadystate, plate_out,
    plate_out_steadystate,
};
pub use source_terms::{base_activities, release_rate, FailureFractions, SourceAndGraphite};

/// Radioactivity (activity) of a decaying species, SI unit becquerel (`Bq`).
///
/// A becquerel is **one nuclear decay per second**, so activity is dimensionally
/// a *frequency* (`s^-1`). This alias is deliberately the same `uom` quantity as
/// [`crate::triso_atops_fork::DecayConstant`]: the physics relation is
/// `A = λ N` (activity equals decay constant times atom count), and with `N`
/// dimensionless this reads `Bq = s^-1 · 1`. Construct with
/// `Activity::new::<hertz>(bq)` and read with `.get::<hertz>()` (`hertz == s^-1`;
/// the unit name is only a dimension label — the value is in becquerels).
///
/// Curies are **not** an SI unit and are not part of `uom`; convert with
/// [`becquerels_from_curies`] / [`curies_from_becquerels`] and the [`BQ_PER_CI`]
/// constant.
pub type Activity = Frequency;

/// Becquerels per curie: `1 Ci = 3.7 × 10^10 Bq` (exact, by definition).
///
/// This single constant replaces every hard-coded `3.7e10` (`Ci→Bq`) and
/// `/ 3.7e10` (`Bq→Ci`) scattered through the upstream `release_rate`,
/// `normal_operation`, and `accident_case`. The curie is defined as exactly
/// `3.7 × 10^10` disintegrations per second (originally the activity of 1 g of
/// Ra-226).
pub const BQ_PER_CI: f64 = 3.7e10;

/// Convert an activity given in **curies** to an [`Activity`] (becquerels).
///
/// `A[Bq] = curies × 3.7e10`. TRISO-ATOPS run files specify per-nuclide fuel
/// inventories in curies (User Manual §2.3.5, "inventories (in curies)"); this is
/// the boundary conversion into the SI-typed core.
///
/// # Arguments
/// - `curies` — activity in Ci (a dimensionless magnitude; must be ≥ 0 to be
///   physical, though the function does not clamp).
#[must_use]
pub fn becquerels_from_curies(curies: f64) -> Activity {
    Activity::new::<hertz>(curies * BQ_PER_CI)
}

/// Convert an [`Activity`] (becquerels) to curies.
///
/// `curies = A[Bq] / 3.7e10`. This is the report-side conversion used when
/// emitting the TRISO-ATOPS output columns, which are all in `Ci` or `Ci/s`.
///
/// # Arguments
/// - `activity` — an [`Activity`] (Bq).
#[must_use]
pub fn curies_from_becquerels(activity: Activity) -> f64 {
    activity.get::<hertz>() / BQ_PER_CI
}

/// Activity of a pool of `count` atoms of a nuclide: `A = λ N`.
///
/// This is the fundamental radioactivity relation and the dimensional anchor of
/// the whole activity layer: an [`Activity`] (`Bq = s^-1`) is a
/// [`DecayConstant`] (`λ`, `s^-1`) multiplied by a dimensionless atom **count**.
/// `uom` enforces that the result is a frequency; the count is a plain `f64`
/// because a number of atoms carries no SI dimension.
///
/// # Arguments
/// - `count` — number of atoms (dimensionless, ≥ 0 physically).
/// - `decay_constant` — the nuclide decay constant `λ = ln 2 / t½` ([`DecayConstant`], `s^-1`).
///
/// # Returns
/// The activity `A = λ · count` as an [`Activity`] (Bq).
#[must_use]
pub fn activity_from_atom_count(count: f64, decay_constant: DecayConstant) -> Activity {
    Activity::new::<hertz>(count * decay_constant.get::<hertz>())
}

/// Number of atoms whose activity is `activity`: `N = A / λ`.
///
/// The inverse of [`activity_from_atom_count`]. Ports the upstream
/// `inventory / λ × 3.7e10` step (`trisoatops.py::normal_operation` line 126),
/// which converts a per-node curie inventory into an atom count: pass
/// `activity = becquerels_from_curies(inventory_ci)`.
///
/// # Arguments
/// - `activity` — the pool activity ([`Activity`], Bq).
/// - `decay_constant` — `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
///
/// # Returns
/// The atom count `N = A / λ` as a plain `f64` (dimensionless).
#[must_use]
pub fn atom_count_from_activity(activity: Activity, decay_constant: DecayConstant) -> f64 {
    activity.get::<hertz>() / decay_constant.get::<hertz>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// Ci↔Bq round trips through the documented constant (1 Ci = 3.7e10 Bq).
    ///
    /// Methodology: convert 44.5 Ci (the User Manual §2.3.5 manual-entry example
    /// inventory) to Bq and back. Reference: 44.5 × 3.7e10 = 1.64650e12 Bq.
    /// Result 2026-07-15: exact to f64.
    #[test]
    fn curie_becquerel_conversion() {
        let a = becquerels_from_curies(44.5);
        assert_relative_eq!(a.get::<hertz>(), 1.646_5e12, max_relative = 1e-12);
        assert_relative_eq!(curies_from_becquerels(a), 44.5, max_relative = 1e-12);
        assert_relative_eq!(BQ_PER_CI, 3.7e10, max_relative = 0.0);
    }

    /// Dimensional-consistency anchor: `activity = decay_constant × atom_count`,
    /// and its inverse `N = A / λ`.
    ///
    /// Methodology: for Cs-137 (`λ = 7.3022e-10 s^-1`) a pool of `N = 1e18` atoms
    /// has `A = λN = 7.3022e8 Bq`. The `uom` types make the left side a
    /// `Frequency` and the right side `Frequency × f64`, so the equality is
    /// dimensionally checked by construction. Result 2026-07-15: round trips to
    /// f64 precision.
    #[test]
    fn activity_equals_lambda_times_count() {
        let lam = DecayConstant::new::<hertz>(7.302_186_793e-10);
        let n = 1.0e18_f64;
        let a = activity_from_atom_count(n, lam);
        assert_relative_eq!(a.get::<hertz>(), 7.302_186_793e8, max_relative = 1e-12);
        // Inverse recovers the count.
        assert_relative_eq!(atom_count_from_activity(a, lam), n, max_relative = 1e-12);
    }
}
