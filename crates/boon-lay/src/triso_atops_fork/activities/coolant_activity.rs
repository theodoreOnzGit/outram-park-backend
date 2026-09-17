// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/utility_functions/calculation_functions.py
//                    (`circulating_steadystate`, `circulating`, `plate_out_steadystate`,
//                     `plate_out`, `clean_up_steadystate`, `clean_up`)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Primary-loop activity pools — circulating, plate-out, clean-up
//!
//! Given a per-node **source rate** `S` (the rate at which a nuclide enters the
//! coolant, after graphite hold-up — see [`super::source_terms::base_activities`]),
//! these functions solve the linear activity-balance for the three primary-loop
//! pools of an HTGR/FHR:
//!
//! - **Circulating** `C` — activity carried in the flowing coolant.
//! - **Plate-out** `P` — activity deposited on primary-loop surfaces (rate
//!   constant `k_plate`).
//! - **Clean-up** `HPS` — activity removed by the helium-purification system
//!   (rate constant `k_clean`).
//!
//! All three share the total removal rate `β = λ + k_plate + k_clean`, where `λ`
//! is the nuclide decay constant. `uom` enforces that `β` is a sum of
//! frequencies and that every `β·t` / `λ·t` exponent is dimensionless.
//!
//! **Derivation:** step 6(iv) (crate-root `TRISO_ATOPS_DERIVATION.md` §6) — the
//! three linear activity balances driven by the coolant source rate `S`. Each
//! `*_steadystate` form is the `t → ∞` limit of its time-dependent counterpart.
//!
//! ## Units
//!
//! - `source_rate`, and the `*_parent` pool inputs / returned pool values are
//!   **effective-unit `f64`** (see the [`super`] module docs): the source/removal
//!   *rate* is in becquerels (`atoms/s`), the pool *amounts* are atom counts
//!   (`atoms`). They are converted to reportable curies once, downstream, by
//!   `× λ / 3.7e10`.
//! - `k_plate`, `k_clean` are plate-out / clean-up **rate constants**
//!   ([`uom::si::f64::Frequency`], `s^-1`).
//! - `decay_constant` is `λ` ([`DecayConstant`], `s^-1`).
//! - `time` is the elapsed reactor run time ([`uom::si::f64::Time`], `s`).

use uom::si::f64::{Frequency, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;

use crate::triso_atops_fork::DecayConstant;

/// Steady-state circulating activity `C` for a nuclide.
///
/// Ports `circulating_steadystate(S, k_plate, lam, k_clean=0, C_parent=0)`.
/// Solves `0 = S + λ·C_parent − β·C` for `C`, i.e.
/// `C = (S + λ·C_parent) / β` with `β = λ + k_plate + k_clean`.
///
/// # Arguments
/// - `source_rate` — source rate `S` into the coolant (effective `f64`, Bq).
/// - `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
/// - `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`).
/// - `k_clean` — clean-up (HPS) rate constant ([`Frequency`], `s^-1`); pass
///   `Frequency::new::<hertz>(0.0)` when there is no HPS.
/// - `circulating_parent` — parent nuclide's circulating pool `C_parent`
///   (effective `f64`); `0.0` if parent decay is not tracked.
///
/// # Returns
/// The circulating pool `C` (effective `f64`, atom count).
#[must_use]
pub fn circulating_steadystate(
    source_rate: f64,
    k_plate: Frequency,
    decay_constant: DecayConstant,
    k_clean: Frequency,
    circulating_parent: f64,
) -> f64 {
    let beta = (decay_constant + k_plate + k_clean).get::<hertz>();
    let lam = decay_constant.get::<hertz>();
    (source_rate + lam * circulating_parent) / beta
}

/// Time-dependent circulating activity `C` at reactor run time `t`.
///
/// Ports `circulating(S, k_plate, lam, t, k_clean=0, C_parent=0)`:
/// `C = S·(1 − e^{−β t}) / β + λ·C_parent / β`, the solution of
/// `dC/dt = S + λ·C_parent − β·C` started from `C(0) = 0` with `S` held constant.
/// As `t → ∞` this tends to [`circulating_steadystate`].
///
/// # Arguments
/// See [`circulating_steadystate`], plus:
/// - `time` — elapsed reactor run time `t` ([`Time`], `s`).
///
/// # Returns
/// The circulating pool `C` at time `t` (effective `f64`, atom count).
#[must_use]
pub fn circulating(
    source_rate: f64,
    k_plate: Frequency,
    decay_constant: DecayConstant,
    time: Time,
    k_clean: Frequency,
    circulating_parent: f64,
) -> f64 {
    let beta_freq = decay_constant + k_plate + k_clean;
    let beta = beta_freq.get::<hertz>();
    let lam = decay_constant.get::<hertz>();
    let bt = (beta_freq * time).get::<ratio>(); // dimensionless, uom-checked
    source_rate * (1.0 - (-bt).exp()) / beta + lam * circulating_parent / beta
}

/// Steady-state plate-out activity `P` for a nuclide.
///
/// Ports `plate_out_steadystate(k_plate, S, lam, k_clean=0, P_parent=0)`:
/// `P = k_plate·S / (λ·β) + P_parent` with `β = λ + k_plate + k_clean`.
///
/// # Arguments
/// - `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
/// - `source_rate` — source rate `S` (effective `f64`, Bq).
/// - `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
/// - `k_clean` — clean-up rate constant ([`Frequency`], `s^-1`).
/// - `plate_out_parent` — parent nuclide's plate-out pool `P_parent` (effective `f64`).
///
/// # Returns
/// The plate-out pool `P` (effective `f64`, atom count).
#[must_use]
pub fn plate_out_steadystate(
    k_plate: Frequency,
    source_rate: f64,
    decay_constant: DecayConstant,
    k_clean: Frequency,
    plate_out_parent: f64,
) -> f64 {
    let beta = (decay_constant + k_plate + k_clean).get::<hertz>();
    let lam = decay_constant.get::<hertz>();
    k_plate.get::<hertz>() * source_rate / (lam * beta) + plate_out_parent
}

/// Time-dependent plate-out activity `P` at reactor run time `t`.
///
/// Ports `plate_out(k_plate, S, lam, t, C, k_clean=0, P_parent=0)`:
/// `P = k_plate/(β−λ)·(S/λ·(1 − e^{−λ t}) − C) + P_parent`.
/// The `β − λ = k_plate + k_clean` denominator degenerates to zero when there is
/// neither plate-out nor clean-up; matching upstream, `P` is then `0`.
///
/// # Arguments
/// See [`plate_out_steadystate`], plus:
/// - `time` — elapsed run time `t` ([`Time`], `s`).
/// - `circulating` — the coolant pool `C` at time `t` (effective `f64`), e.g.
///   from [`circulating`].
///
/// # Returns
/// The plate-out pool `P` at time `t` (effective `f64`, atom count).
#[must_use]
pub fn plate_out(
    k_plate: Frequency,
    source_rate: f64,
    decay_constant: DecayConstant,
    time: Time,
    circulating: f64,
    k_clean: Frequency,
    plate_out_parent: f64,
) -> f64 {
    let beta_minus_lam = k_plate + k_clean; // β − λ
    if beta_minus_lam.get::<hertz>() == 0.0 {
        return 0.0;
    }
    let lam = decay_constant.get::<hertz>();
    let lt = (decay_constant * time).get::<ratio>(); // dimensionless, uom-checked
    let k_plate_frac = (k_plate / beta_minus_lam).get::<ratio>();
    k_plate_frac * (source_rate / lam * (1.0 - (-lt).exp()) - circulating) + plate_out_parent
}

/// Steady-state clean-up (HPS) activity `HPS` for a nuclide.
///
/// Ports `clean_up_steadystate(k_plate, S, lam, k_clean, HPS_parent=0)`:
/// `HPS = k_clean·S / (λ·β)` with `β = λ + k_plate + k_clean`.
///
/// # Upstream note
/// The upstream steady-state form **does not** add the `HPS_parent` term (unlike
/// the circulating and plate-out steady-state forms, which add their parent
/// pool). This port preserves that behaviour for numerical fidelity — the
/// parameter is accepted for signature symmetry but ignored. Parent HPS
/// contributions are captured by the time-dependent [`clean_up`].
///
/// # Arguments
/// - `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
/// - `source_rate` — source rate `S` (effective `f64`, Bq).
/// - `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
/// - `k_clean` — clean-up rate constant ([`Frequency`], `s^-1`).
/// - `clean_up_parent` — parent HPS pool (accepted but **ignored**, per upstream).
///
/// # Returns
/// The clean-up pool `HPS` (effective `f64`, atom count).
#[must_use]
pub fn clean_up_steadystate(
    k_plate: Frequency,
    source_rate: f64,
    decay_constant: DecayConstant,
    k_clean: Frequency,
    clean_up_parent: f64,
) -> f64 {
    let _ = clean_up_parent; // upstream `clean_up_steadystate` ignores HPS_parent
    let beta = (decay_constant + k_plate + k_clean).get::<hertz>();
    let lam = decay_constant.get::<hertz>();
    k_clean.get::<hertz>() * source_rate / lam / beta
}

/// Time-dependent clean-up (HPS) activity `HPS` at reactor run time `t`.
///
/// Ports `clean_up(k_plate, S, lam, t, C, k_clean, HPS_parent=0)`:
/// `HPS = k_clean/(β−λ)·(S/λ·(1 − e^{−λ t}) − C) + HPS_parent`.
///
/// # Arguments
/// See [`clean_up_steadystate`], plus:
/// - `time` — elapsed run time `t` ([`Time`], `s`).
/// - `circulating` — coolant pool `C` at time `t` (effective `f64`).
/// - `clean_up_parent` — parent HPS pool `HPS_parent` (effective `f64`); here it
///   **is** added, matching upstream.
///
/// # Returns
/// The clean-up pool `HPS` at time `t` (effective `f64`, atom count).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn clean_up(
    k_plate: Frequency,
    source_rate: f64,
    decay_constant: DecayConstant,
    time: Time,
    circulating: f64,
    k_clean: Frequency,
    clean_up_parent: f64,
) -> f64 {
    let beta_minus_lam = (k_plate + k_clean).get::<hertz>(); // β − λ
    let lam = decay_constant.get::<hertz>();
    let lt = (decay_constant * time).get::<ratio>(); // dimensionless, uom-checked
    k_clean.get::<hertz>() / beta_minus_lam
        * (source_rate / lam * (1.0 - (-lt).exp()) - circulating)
        + clean_up_parent
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    // Shared test inputs; identical to the upstream-Python driver run on
    // 2026-07-15 (commit de374c8). See the module V&V note in the test docs.
    fn k(v: f64) -> Frequency {
        Frequency::new::<hertz>(v)
    }
    fn lam(v: f64) -> DecayConstant {
        DecayConstant::new::<hertz>(v)
    }
    fn t40yr() -> Time {
        Time::new::<uom::si::time::second>(1.262_304e9)
    }

    const S: f64 = 1.234e-3;
    const K_PLATE: f64 = 7.5e-4;
    const LAM: f64 = 3.2e-6;
    const K_CLEAN: f64 = 8.77e-5;
    const C_PARENT: f64 = 2.5e-2;
    const P_PARENT: f64 = 1.1e-2;
    const HPS_PARENT: f64 = 4.0e-3;

    /// Circulating steady-state matches upstream `circulating_steadystate`.
    ///
    /// Methodology: `C = (S + λ C_parent)/β`. Reference (upstream Python,
    /// 2026-07-15): 1.4675704602211916. Pass tol 1e-12 rel.
    #[test]
    fn circulating_steadystate_matches_upstream() {
        let c = circulating_steadystate(S, k(K_PLATE), lam(LAM), k(K_CLEAN), C_PARENT);
        assert_relative_eq!(c, 1.467_570_460_221_191_6, max_relative = 1e-12);
    }

    /// Time-dependent circulating tends to the steady state at t = 40 yr
    /// (β t ≈ 1e6 ⇒ 1 − e^{−βt} ≈ 1). Reference: 1.4675704602211916.
    #[test]
    fn circulating_transient_reaches_steady_state() {
        let c = circulating(S, k(K_PLATE), lam(LAM), t40yr(), k(K_CLEAN), C_PARENT);
        assert_relative_eq!(c, 1.467_570_460_221_191_6, max_relative = 1e-12);
    }

    /// Plate-out steady-state and transient match upstream.
    ///
    /// References (upstream Python, 2026-07-15):
    /// `plate_out_steadystate = 343.9505290759901`,
    /// `plate_out (t=40yr, C=steady) = 343.95044389976624`. Pass tol 1e-12 rel.
    #[test]
    fn plate_out_matches_upstream() {
        let p_ss = plate_out_steadystate(k(K_PLATE), S, lam(LAM), k(K_CLEAN), P_PARENT);
        assert_relative_eq!(p_ss, 343.950_529_075_990_1, max_relative = 1e-12);

        let c = circulating(S, k(K_PLATE), lam(LAM), t40yr(), k(K_CLEAN), C_PARENT);
        let p = plate_out(k(K_PLATE), S, lam(LAM), t40yr(), c, k(K_CLEAN), P_PARENT);
        assert_relative_eq!(p, 343.950_443_899_766_24, max_relative = 1e-12);
    }

    /// Plate-out degenerates to zero when there is neither plate-out nor clean-up
    /// (β − λ = 0), matching the upstream guard.
    #[test]
    fn plate_out_zero_when_no_removal() {
        let c = circulating(S, k(0.0), lam(LAM), t40yr(), k(0.0), 0.0);
        let p = plate_out(k(0.0), S, lam(LAM), t40yr(), c, k(0.0), 0.0);
        assert_eq!(p, 0.0);
    }

    /// Clean-up steady-state and transient match upstream.
    ///
    /// References (upstream Python, 2026-07-15):
    /// `clean_up_steadystate = 40.21799559995244` (HPS_parent ignored upstream),
    /// `clean_up (t=40yr, C=steady) = 40.22198564001266`. Pass tol 1e-12 rel.
    #[test]
    fn clean_up_matches_upstream() {
        let hps_ss = clean_up_steadystate(k(K_PLATE), S, lam(LAM), k(K_CLEAN), HPS_PARENT);
        assert_relative_eq!(hps_ss, 40.217_995_599_952_44, max_relative = 1e-12);

        let c = circulating(S, k(K_PLATE), lam(LAM), t40yr(), k(K_CLEAN), C_PARENT);
        let hps = clean_up(k(K_PLATE), S, lam(LAM), t40yr(), c, k(K_CLEAN), HPS_PARENT);
        assert_relative_eq!(hps, 40.221_985_640_012_66, max_relative = 1e-12);
    }
}
