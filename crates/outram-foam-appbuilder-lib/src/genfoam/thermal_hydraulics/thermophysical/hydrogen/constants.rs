// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/H2.C
//     (the `Foam::H2::H2()` default-constructor member-initialiser list)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Thomas Guilbaud, Eymeric Simonnot (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Fixed physical constants for the H/H2 property package
//!
//! Every constant here is a literal from GeN-Foam's `H2` default constructor
//! (`H2.C`), which builds the base `liquidProperties` critical-point data and
//! then derives a handful of Soave-Redlich-Kwong-style equation-of-state
//! coefficients from it. GeN-Foam's `H2` also carries a set of NSRDS
//! correlation coefficients (`rho_`, `pv_`, `hl_`, `Cp_`, `h_`, `Cpg_`, `B_`,
//! `mu_`, `mug_`, `kappa_`, `kappag_`, `sigma_`, `D_`) inherited from the base
//! `liquidProperties` class; the upstream header labels them **"useless but
//! needed because liquidProperties is over constrained"** and no method in
//! `H2I.H` ever reads them (every physical property below is computed from a
//! bespoke closed-form correlation instead). This port therefore omits them —
//! they would be dead code with no caller, which the workspace rules forbid.
//!
//! ## Reference basis (molar vs. mass)
//!
//! `W`, `W_H`, `R`, `TC`, `PC`, `VC` are all on GeN-Foam's chosen basis of
//! **kmol** (`W`/`W_H` in kg/kmol, `R` in J/(kmol K), `VC` in m^3/kmol) — the
//! same basis OpenFOAM's `liquidProperties` uses. Downstream functions convert
//! to a mass basis locally where the upstream code does (see
//! [`equation_of_state`](super::equation_of_state) and
//! [`heat_capacity`](super::heat_capacity)); this module does not.

/// Molar mass of molecular hydrogen H2 — kg/kmol.
pub(super) const W: f64 = 2.01588;

/// Molar mass of atomic hydrogen H — kg/kmol.
pub(super) const W_H: f64 = 1.007647;

/// Universal gas constant on the kmol basis — J / (kmol K).
pub(super) const R: f64 = 8.314472e3;

/// Critical temperature of H2 — K.
pub(super) const TC: f64 = 33.145;

/// Critical pressure of H2 — Pa.
pub(super) const PC: f64 = 1.2965e6;

/// Critical molar volume of H2 — m^3 / kmol.
pub(super) const VC: f64 = 6.4481e-2;

/// Acentric factor of H2 — dimensionless.
pub(super) const OMEGA: f64 = 0.305;

/// SRK-style temperature exponent `n = 0.4986 + 1.1735*omega + 0.4754*omega^2`
/// (upstream `H2::n_`), computed fresh from [`OMEGA`] rather than cached, since
/// it is four flops and this module holds no state.
#[inline]
pub(super) fn n() -> f64 {
    0.4986 + 1.1735 * OMEGA + 0.4754 * OMEGA * OMEGA
}

/// SRK-style attraction coefficient `alpha0 = 0.42748 * (R*Tc)^2 / Pc`
/// (upstream `H2::alpha0_`), molar basis — J m^3 / kmol^2.
#[inline]
pub(super) fn alpha0() -> f64 {
    0.42748 * (R * TC) * (R * TC) / PC
}

/// SRK-style covolume `b = 0.08664 * R * Tc / Pc` (upstream `H2::b_`), molar
/// basis — m^3 / kmol.
#[inline]
pub(super) fn b() -> f64 {
    0.08664 * R * TC / PC
}

/// Volume-translation coefficient `c` (upstream `H2::c_`), molar basis —
/// m^3 / kmol. Depends on [`alpha0`] and [`b`], so it is not `const`.
#[inline]
pub(super) fn c() -> f64 {
    let alpha0_ = alpha0();
    let b_ = b();
    R * TC / (PC + alpha0_ / (VC * (VC + b_))) + b_ - VC
}
