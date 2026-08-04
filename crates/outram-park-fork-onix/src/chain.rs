//! Nuclide-chain bookkeeping: decay data, reaction rates, and fission yields.
//!
//! These are the per-nuclide inputs the burnup-matrix assembly
//! ([`crate::matrix`]) consumes. They mirror the data ONIX attaches to each
//! `passport` (nuclide record) — decay constants + branching, one-group
//! reaction rates, and fission-yield tables — but in a caller-supplied,
//! precomputed (stand-alone) form rather than read from ONIX's data libraries.
//!
//! ## Provenance (GPLv3 relicensing of MIT upstream)
//!
//! Structure mirrors ONIX (open-source, MIT; commit `7328dc6`):
//!   * `onix/salameche/mat_builder.py:134` (`get_decay_mat` — total decay on
//!     the diagonal, partial decay constants off-diagonal),
//!   * `onix/salameche/mat_builder.py:5` (`get_xs_mat` — removal on the
//!     diagonal, production reaction rates off-diagonal, fission-yield term at
//!     lines 99–125), and
//!   * `onix/passport.py` (the per-nuclide `decay_a`, `current_xs`, `fy`
//!     records).
//!
//! Independent Rust re-implementation; OUTRAM PARK fork relicenses under
//! **GPL-3.0-only** (MIT is GPL-3.0-compatible).

use crate::nuclide::Nuclide;
use crate::reactions::{DecayMode, ReactionChannel};

/// Decay constant of one nuclide plus its branching among decay modes.
///
/// * `lambda_total` — total decay constant λ, **units `1/s`** (`λ = ln 2 / t½`).
///   A stable nuclide has `lambda_total = 0.0`. Valid range `>= 0`.
/// * `branches` — `(mode, fraction)` pairs; each `fraction` is the branching
///   ratio (dimensionless, in `[0, 1]`) of that mode. Physically the fractions
///   sum to ~1, but this is **not** enforced (an incomplete data set may list
///   only the tracked modes). The partial decay constant of a mode is
///   `lambda_total * fraction` (units `1/s`).
///
/// This matches ONIX's `decay_a` dict, whose `'total decay'` entry is the
/// diagonal loss (`mat_builder.py:162`) and whose per-mode entries are the
/// off-diagonal production rates (`mat_builder.py:187`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecayData {
    /// Total decay constant λ, units `1/s`. `0.0` ⇒ stable.
    pub lambda_total: f64,
    /// Branching ratios `(mode, fraction)`; `fraction` dimensionless in `[0,1]`.
    pub branches: Vec<(DecayMode, f64)>,
}

impl DecayData {
    /// A stable nuclide: zero decay constant, no branches.
    pub fn stable() -> Self {
        Self {
            lambda_total: 0.0,
            branches: Vec::new(),
        }
    }

    /// Build decay data from a half-life (seconds) and branching ratios.
    ///
    /// `half_life_s` must be `> 0` (units `s`); the total decay constant is
    /// `ln 2 / half_life_s` (units `1/s`). `branches` are `(mode, fraction)`
    /// with `fraction` dimensionless.
    pub fn from_half_life(half_life_s: f64, branches: Vec<(DecayMode, f64)>) -> Self {
        Self {
            lambda_total: std::f64::consts::LN_2 / half_life_s,
            branches,
        }
    }

    /// Single-mode decay: total constant λ (`1/s`) with 100 % branching.
    ///
    /// Convenience for pure decay chains where each nuclide has exactly one
    /// decay mode. `lambda` is the decay constant in `1/s`.
    pub fn single_mode(lambda: f64, mode: DecayMode) -> Self {
        Self {
            lambda_total: lambda,
            branches: vec![(mode, 1.0)],
        }
    }
}

/// One-group (or few-group collapsed) neutron-reaction rates for a nuclide.
///
/// Each entry is `(channel, rate)` where `rate` is the **reaction rate in
/// `1/s`** — i.e. microscopic cross section σ (barns) × `1e-24` (cm²/barn) ×
/// scalar flux φ (n·cm⁻²·s⁻¹), already collapsed to one group by the caller.
/// This is exactly ONIX's `A = B*1e-24*flux + C` construction
/// (`onix/salameche/burn.py:187`), except the caller supplies the finished
/// `1/s` rate rather than (σ, φ).
///
/// Use [`ReactionRates::from_xs_flux`] if you have barns + flux and want the
/// `1e-24` conversion done for you.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReactionRates {
    /// `(channel, rate_per_second)` — `rate` in `1/s`.
    pub channels: Vec<(ReactionChannel, f64)>,
}

impl ReactionRates {
    /// Empty reaction-rate set (nuclide sees no neutron flux / has no cross
    /// sections).
    pub fn none() -> Self {
        Self::default()
    }

    /// Build rates from one-group cross sections (barns) and a scalar flux.
    ///
    /// `channels_barns` are `(channel, sigma_barns)` with σ in **barns**;
    /// `flux` is the scalar neutron flux in **n·cm⁻²·s⁻¹**. Each stored rate is
    /// `sigma_barns * 1e-24 * flux` (units `1/s`), reproducing the `1e-24`
    /// barns→cm² conversion in ONIX `burn.py:187`.
    pub fn from_xs_flux(channels_barns: &[(ReactionChannel, f64)], flux: f64) -> Self {
        let channels = channels_barns
            .iter()
            .map(|&(ch, sigma_barns)| (ch, sigma_barns * 1e-24 * flux))
            .collect();
        Self { channels }
    }

    /// Total removal rate (sum over all channels including fission), units
    /// `1/s`. This is the neutron-reaction part of the burnup-matrix diagonal
    /// loss (the ONIX `removal` term, `mat_builder.py:32`).
    pub fn total_removal(&self) -> f64 {
        self.channels.iter().map(|&(_, r)| r).sum()
    }

    /// The fission rate for this nuclide (sum of any fission channels), `1/s`.
    pub fn fission_rate(&self) -> f64 {
        self.channels
            .iter()
            .filter(|&&(ch, _)| ch.is_fission())
            .map(|&(_, r)| r)
            .sum()
    }
}

/// Fission-yield table for one fissile parent.
///
/// `products` are `(product, yield_fraction)` where `yield_fraction` is the
/// number of that product produced **per fission** (dimensionless, atoms per
/// fission). ONIX stores yields in **percent** and multiplies by `1e-2`
/// (`mat_builder.py:121`); here the caller supplies the already-fractional
/// yield (atoms/fission), so no `1e-2` is applied. Cumulative or independent
/// yields may be used depending on how the chain is modelled — that choice is
/// the caller's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FissionYields {
    /// `(product_nuclide, atoms_per_fission)`; `atoms_per_fission` dimensionless.
    pub products: Vec<(Nuclide, f64)>,
}

impl FissionYields {
    /// An empty yield table.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a yield table from percent yields (ONIX's native units).
    ///
    /// `products_percent` are `(product, yield_percent)` with `yield_percent`
    /// in **percent per fission**; each stored fraction is `yield_percent *
    /// 1e-2` (atoms/fission), matching ONIX `mat_builder.py:121`.
    pub fn from_percent(products_percent: &[(Nuclide, f64)]) -> Self {
        let products = products_percent
            .iter()
            .map(|&(nuc, pct)| (nuc, pct * 1e-2))
            .collect();
        Self { products }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_life_to_lambda() {
        // t_half = ln2 s  => lambda = 1 /s.
        let d = DecayData::from_half_life(std::f64::consts::LN_2, vec![]);
        assert!((d.lambda_total - 1.0).abs() < 1e-15);
    }

    #[test]
    fn xs_flux_applies_1e_minus_24() {
        // 2 barns at flux 1e14 -> 2 * 1e-24 * 1e14 = 2e-10 /s.
        let r = ReactionRates::from_xs_flux(&[(ReactionChannel::NGamma, 2.0)], 1e14);
        assert!((r.channels[0].1 - 2e-10).abs() < 1e-24);
        assert!((r.total_removal() - 2e-10).abs() < 1e-24);
    }

    #[test]
    fn percent_yields_scaled() {
        let fy = FissionYields::from_percent(&[(Nuclide::new(54, 135, 0), 6.3)]);
        assert!((fy.products[0].1 - 0.063).abs() < 1e-15);
    }
}
