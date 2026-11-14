// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Pointwise source terms translated from Moltres in upstream operation order
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7a60f59424d0d8f46748ba4e243d76b671
//   Upstream sources: src/kernels/CoupledFissionKernel.C (computeQpResidual,
//     `account_delayed` branch), src/kernels/PrecursorSource.C
//   Upstream license: LGPL-2.1, incorporated under the LGPL-2.1 section 3
//   GPL conversion option. Independent reimplementation, no code copied.
//
// This file is part of OUTRAM PARK, GPL-3.0-only (see crate LICENSE).

//! Pointwise source terms whose floating-point **association** follows
//! Moltres exactly, so the code-to-code differential
//! (`verification_and_validation/moltres_differential_gh1400/`) can be a
//! bit-identity check rather than a tolerance. Each function states the
//! upstream expression it reproduces.

/// Prompt-fission neutron source of one group in one cell, Moltres
/// `CoupledFissionKernel` with `account_delayed = true`.
///
/// Upstream accumulates `r = sum_i(-nsf_i * phi_i)`, then
/// `r *= (1 - beta) * chi_p[g]` (the product in the parentheses is formed
/// first), then `r /= eigenvalue_scaling`. With `sf = sum_i nsf_i phi_i` and
/// `eigenvalue_scaling = k` the source is the exact negation of that
/// residual: `sf * ((1 - beta) * chi_p) / k`.
///
/// `sf` in `1/(m^3 s)`, `beta`, `chi_p` dimensionless, `k` dimensionless.
#[must_use]
pub fn prompt_fission_source(sf: f64, beta: f64, chi_p: f64, k: f64) -> f64 {
    sf * ((1.0 - beta) * chi_p) / k
}

/// Delayed-precursor production `beta_i/k * S_f` of one family in one cell
/// (`1/(m^3 s)`).
///
/// Upstream `PrecursorSource` forms `sum_j(beta_i * nsf_j * phi_j) / k`,
/// which groups differently from `(beta_i / k) * S_f` whenever there is more
/// than one energy group; this crate's `PrecursorDrift` is handed `S_f`
/// (not the group fluxes), so it cannot reproduce upstream's grouping. The
/// difference is round-off only and is measured in the differential record.
#[must_use]
pub fn precursor_production(beta_i: f64, k: f64, sf: f64) -> f64 {
    beta_i / k * sf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_source_and_production_values() {
        // (1 - 0.5) * 1.0 = 0.5; 4 * 0.5 / 2 = 1
        assert_eq!(prompt_fission_source(4.0, 0.5, 1.0, 2.0), 1.0);
        assert_eq!(precursor_production(0.5, 2.0, 8.0), 2.0);
    }
}
