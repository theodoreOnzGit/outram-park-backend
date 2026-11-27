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

// `omega11_11` and `omega22_11` (both H2-H2) are used only by the pure-H2 self-diffusivity
// `diffusionH2` (omega11_11) and by nothing at all (omega22_11) inside `H2I.H`; they and `C_12` were left
// out until gh:#1872 round 50, whose differential against the real class compares every method of `H2`, so
// they are translated below.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
/// `Omega^(2,2)_{2,2}` (H-H), single polynomial fit (no low-`T` branch;
/// atomic hydrogen only appears at high `T` in practice).
#[inline]
pub(super) fn omega22_22(t: f64) -> f64 {
    5e-16 * t.r_powf(4.0) - 2e-11 * t.r_powf(3.0) + 2e-7 * (t * t) - 0.0015 * t + 7.1214
}

/// `Omega^(1,1)_{1,2}` (H2-H cross term), logarithmic fit valid 1000-10000 K
/// and extended to cryogenic `T`.
#[inline]
pub(super) fn omega11_12(t: f64) -> f64 {
    -1.024 * t.r_ln() + 11.054
}

/// `Omega^(2,2)_{1,2}` (H2-H cross term), same fitting scheme as [`omega11_12`].
#[inline]
pub(super) fn omega22_12(t: f64) -> f64 {
    -1.227 * t.r_ln() + 13.42
}

/// `A*_{1,2}` ratio (Hirschfelder-Curtiss-Bird mixture-rule coefficient) for
/// the H2-H pair.
#[inline]
pub(super) fn a12(t: f64) -> f64 {
    0.0345 * t.r_ln() + 0.9917
}

/// `B*_{1,2}` ratio for the H2-H pair.
#[inline]
pub(super) fn b12(t: f64) -> f64 {
    0.0462 * t.r_ln() + 0.8701
}

/// `C*_12` (upstream `H2::C_12`); no method of `H2I.H` calls it (gh:#1872 round 50 compares it).
#[inline]
pub(super) fn c12(t: f64) -> f64 {
    -0.031 * t.r_ln() + 1.1108
}

/// `Omega^(1,1)_{1,1}` of H2-H2 (upstream `H2::omega11_11`): a power law up to 1000 K, a quintic above.
/// Upstream's `pow(T, k)` is glibc `pow`, here `r_powf`; `sqr(T)` is `T*T`.
#[inline]
pub(super) fn omega11_11(t: f64) -> f64 {
    if t <= 1000.0 {
        return 44.13 * t.r_powf(-0.331);
    }
    -1.173958e-19 * t.r_powf(5.0) + 3.983904e-15 * t.r_powf(4.0) - 5.443976e-11 * t.r_powf(3.0)
        + 3.918410e-07 * (t * t)
        - 1.704143e-03 * t
        + 6.552751
}

/// `Omega^(2,2)_{1,1}` of H2-H2 (upstream `H2::omega22_11`); called by nothing in `H2I.H`.
#[inline]
pub(super) fn omega22_11(t: f64) -> f64 {
    if t <= 1000.0 {
        return 37.879 * t.r_powf(-0.268);
    }
    5.570961e-20 * t.r_powf(5.0) - 1.444695e-15 * t.r_powf(4.0)
        + 1.024643e-11 * t.r_powf(3.0)
        + 3.257067e-08 * (t * t)
        - 8.379299e-04 * t
        + 6.786617
}
