// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/H2I.H
//     (`omega11_11`, `omega22_11`, `omega22_22`, `omega11_12`, `omega22_12`,
//     `A_12`, `B_12`, `C_12`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Kinetic-theory collision integrals for the H/H2 gas mixture
//!
//! Chapman-Enskog kinetic theory expresses the transport properties of a gas
//! mixture (viscosity, thermal conductivity, binary diffusivity) in terms of
//! reduced collision integrals `Omega^(l,s)` between each pair of species —
//! here H-H, H2-H2, and the H-H2 cross term. GeN-Foam fits each integral (and
//! the auxiliary ratios `A*`, `B*`, `C*` used in the Hirschfelder-Curtiss-Bird
//! mixture rules) as a piecewise polynomial/logarithmic function of `T`,
//! against tabulated data from Vanderslice et al. (1962) and, above 1000 K,
//! from Mason & Rice / Mason / Amdur & Mason.
//!
//! Naming follows the upstream superscript/subscript convention:
//! `omega{l,s}_{i,j}` is `Omega^(l,s)_{i,j}` for the species pair `i,j` in
//! `{1 = H2, 2 = H}` — e.g. [`omega22_12`] is `Omega^(2,2)` between H2 and H.
//! `T` is in K throughout; every function is a pure closed-form fit, no table
//! lookup (there is no [`ScalarInterpolateTable`](crate::genfoam::common::interpolate_table::ScalarInterpolateTable)
//! use in this package).
//!
//! References: [6] Vanderslice, Weissman, Mason, Fallon, *High-temperature
//! transport properties of dissociating hydrogen*, Phys. Fluids 5(2), 1962;
//! [11] Mason & Rice, J. Chem. Phys. 22(3), 1954; [12] Mason, J. Chem. Phys.
//! 22(2), 1954; [13] Amdur & Mason, Phys. Fluids 1(5), 1958.

// Note: upstream `H2I.H` also fits `Omega^(1,1)_{1,1}` and `Omega^(2,2)_{1,1}`
// (both H2-H2) as `omega11_11`/`omega22_11`. `omega11_11` is used only by the
// pure-H2 self-diffusivity `diffusionH2`, which this port does not translate
// (see [`diffusion`](super::diffusion)'s module doc — no caller and no
// upstream reference value); `omega22_11` has no caller anywhere in
// `H2I.H`, upstream or here. Per the "no orphaned dead code" rule, neither is
// ported.

/// `Omega^(2,2)_{2,2}` (H-H), single polynomial fit (no low-`T` branch;
/// atomic hydrogen only appears at high `T` in practice).
#[inline]
pub(super) fn omega22_22(t: f64) -> f64 {
    5e-16 * t.powi(4) - 2e-11 * t.powi(3) + 2e-7 * t * t - 0.0015 * t + 7.1214
}

/// `Omega^(1,1)_{1,2}` (H2-H cross term), logarithmic fit valid 1000-10000 K
/// and extended to cryogenic `T`.
#[inline]
pub(super) fn omega11_12(t: f64) -> f64 {
    -1.024 * t.ln() + 11.054
}

/// `Omega^(2,2)_{1,2}` (H2-H cross term), same fitting scheme as [`omega11_12`].
#[inline]
pub(super) fn omega22_12(t: f64) -> f64 {
    -1.227 * t.ln() + 13.42
}

/// `A*_{1,2}` ratio (Hirschfelder-Curtiss-Bird mixture-rule coefficient) for
/// the H2-H pair.
#[inline]
pub(super) fn a12(t: f64) -> f64 {
    0.0345 * t.ln() + 0.9917
}

/// `B*_{1,2}` ratio for the H2-H pair.
#[inline]
pub(super) fn b12(t: f64) -> f64 {
    0.0462 * t.ln() + 0.8701
}

// Note: upstream `H2I.H` also fits a `C_12` ratio (`Foam::H2::C_12`), but no
// method in `H2I.H` ever calls it — `mu()`/`kappa()`'s Hirschfelder-Curtiss-Bird
// mixture rules only need `A*` ([`a12`]) and `B*` ([`b12`]). Per this
// workspace's "no orphaned dead code" rule, it is intentionally not ported —
// there is nothing here to call it either.
