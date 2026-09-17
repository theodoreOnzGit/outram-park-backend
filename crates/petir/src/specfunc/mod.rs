// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.

//! Special functions — GSL's `specfunc/`, plus the OpenFOAM-derived kernels.
//!
//! # What is here
//!
//! - **Error function family**: [`erf`], [`erfc`] and the scaled complementary
//!   form [`erfc_scaled`], translated from GSL's `specfunc/erfc.c`; and
//!   [`erf_inv`], the Winitzki approximation lifted verbatim from
//!   `outram-foam-basic-lib`.
//! - **Gamma family**: [`ln_gamma`] and [`gamma`] (GSL's Lanczos form), the
//!   regularised incomplete gamma functions
//!   [`inc_gamma_ratio_p`] / [`inc_gamma_ratio_q`] and their unnormalised
//!   counterparts, and the inverse [`inv_inc_gamma`] — the last three lifted
//!   verbatim from `outram-foam-basic-lib`.
//!
//! # Accuracy, and how to read the two lineages
//!
//! The two sources are held to different standards and this matters when
//! choosing a routine:
//!
//! - GSL's `erf`/`erfc`/`ln_gamma` target close to machine precision and are
//!   documented with their error bounds.
//! - [`erf_inv`] is an *approximation* with maximum relative error of order
//!   `1e-4`. That is what OpenFOAM uses and it is fine for the sampling work it
//!   was written for, but it is emphatically not a machine-precision inverse.
//!   Do not reach for it where accuracy matters without refining the result —
//!   a couple of Newton steps against [`erf`] will do it.
//!
//! Each function's own doc comment states its accuracy and its upstream.

pub mod erf;
/// Inverse error function (Winitzki approximation), lifted verbatim from
/// outram-foam-basic-lib. Maximum relative error of order 1e-4 -- see the
/// module documentation above before relying on it.
pub mod erf_inv;
pub mod gamma;
/// Regularised and unnormalised incomplete gamma functions, lifted verbatim
/// from outram-foam-basic-lib (DiDonato & Morris, ACM TOMS 1986).
pub mod inc_gamma;
/// Inverse of the regularised lower incomplete gamma function, lifted verbatim
/// from outram-foam-basic-lib.
pub mod inv_inc_gamma;

pub use erf::{erf, erfc, erfc_scaled, erfcx};
pub use erf_inv::erf_inv;
pub use gamma::{
    beta, choose, factorial, gamma, ln_beta, ln_factorial, ln_gamma, ln_gamma_sgn,
};
pub use inc_gamma::{inc_gamma_p, inc_gamma_q, inc_gamma_ratio_p, inc_gamma_ratio_q};
pub use inv_inc_gamma::inv_inc_gamma;
