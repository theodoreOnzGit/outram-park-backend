// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// Ported in part from the GNU Octave control package ("LTI Syncope"),
// https://github.com/gnu-octave/pkg-control, control 4.2.2+,
// commit f39e2625bffc56864c3af746b4410bb71ce6bbb1 (2026-07-27):
//   inst/@lti/c2d.m, inst/@lti/d2c.m, inst/@tf/__c2d__.m,
//   inst/@tf/__d2c__.m, inst/@ss/__c2d__.m, inst/@ss/__d2c__.m,
//   inst/filt.m
// Copyright (C) 2009-2016 Lukas F. Reichlin
// Copyright (C) Torsten Lilge (inst/@ss/__c2d__.m)
// Those upstream files are licensed GPL-3.0-or-later, which permits
// distribution under GPL-3.0-only terms as part of this crate. No
// SLICOT-derived (BSD 3-Clause) upstream file was ported into this module:
// the SLICOT kernels the upstream calls (__sl_mb05nd__ matrix exponential,
// __sl_ab04md__ bilinear transformation) are replaced here by independent
// implementations of the textbook formulas, limited to system order <= 2
// where a closed form exists. See the crate NOTICE.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.

//! z-domain (discrete-time) transfer functions and continuous <-> discrete
//! conversion, ported from the GNU Octave control package.
//!
//! # What belongs here
//!
//! - [`ContinuousTransferFn`] — a SISO continuous-time transfer function
//!   `G(s) = num(s)/den(s)` held as real polynomial coefficients
//!   (Octave `tf` equivalent, SISO only).
//! - [`DiscreteTransferFn`] — a SISO discrete-time transfer function
//!   `G(z^-1)` with a sample time, held in DSP form (ascending powers of
//!   `z^-1`, Octave `filt` equivalent), advanced sample-by-sample by an
//!   O(1) fixed-state recurrence.
//! - [`C2dMethod`] / [`D2cMethod`] and the conversions
//!   [`ContinuousTransferFn::to_discrete`] (Octave `c2d`) and
//!   [`DiscreteTransferFn::to_continuous`] (Octave `d2c`).
//!
//! # What does NOT belong here
//!
//! - MIMO systems, state-space models as a public surface, frequency-domain
//!   plotting, and the SLICOT numerical library. Upstream's `c2d` reaches
//!   MIMO/state-space generality through the BSD-3-licensed SLICOT kernels;
//!   this module deliberately stays SISO and order <= 2 for the methods
//!   that need eigenvalues (`Zoh`, `MatchedPoleZero`), because every block
//!   this crate ships (first-order lag, first-order with zero, second-order)
//!   is order <= 2 and a closed form exists there.
//! - Discrete Riccati/Lyapunov machinery (`dlqr`, `dare`, `dlyap`): those
//!   pull in SLICOT Riccati solvers and are tracked as a follow-up bead,
//!   not half-ported here.
//! - Dead time / transport delay: the continuous blocks in
//!   `stable_transfer_functions` handle dead time themselves; this layer
//!   converts the rational part only.
//!
//! # Relation to the O(1) recurrence blocks
//!
//! The `stable_transfer_functions` blocks advance by the zero-order-hold
//! (step-invariant) discrete equivalent specialised to their own structure.
//! [`C2dMethod::Zoh`] is the *same mathematics* in general form: converting
//! a first-order lag with `Zoh` and stepping the result reproduces
//! `FirstOrderStableTransferFnNoZeroes` sample-for-sample (this is verified
//! in `verification_tests.rs`). What this module adds beyond that block is
//! the other discretisation methods (`Tustin`, `TustinPrewarp`,
//! `MatchedPoleZero`), the inverse direction (`d2c`), and an explicit
//! coefficient-level representation you can inspect.
//!
//! # Units (`uom`)
//!
//! Sample times are `uom` [`Time`](uom::si::f64::Time) (seconds) and block
//! input/output signals are dimensionless [`Ratio`](uom::si::f64::Ratio),
//! matching the rest of the crate. **Polynomial coefficients are plain
//! `f64`**: the coefficient of `s^k` carries units of `s^k` (SI seconds
//! implied) and the coefficients of a z-polynomial are genuinely
//! dimensionless, so a single `uom` type cannot represent a coefficient
//! vector — forcing one would misstate the physics rather than protect it.
//! This is a documented, deliberate exception to the uom-everywhere rule.

pub mod continuous_tf;
pub mod conversion;
pub(crate) mod cplx;
pub mod discrete_tf;
pub(crate) mod polynomial;

#[cfg(test)]
mod verification_tests;

pub use continuous_tf::ContinuousTransferFn;
pub use conversion::{C2dMethod, D2cMethod};
pub use discrete_tf::DiscreteTransferFn;

use thiserror::Error;

/// Errors from z-domain construction and conversion.
///
/// This is deliberately separate from
/// [`ChemEngProcessControlSimulatorError`](crate::beta_testing::errors::ChemEngProcessControlSimulatorError)
/// so that the z-domain module stays self-contained and does not touch the
/// twinned error files shared with `alpha_nightly`.
#[derive(Debug, Error, PartialEq)]
pub enum ZDomainError {
    /// A denominator polynomial was empty or identically zero.
    #[error("denominator polynomial is empty or zero")]
    ZeroDenominator,

    /// A time constant that must be strictly positive (seconds) was not.
    #[error("time constant must be strictly positive")]
    NonPositiveTimeConstant,

    /// A damping ratio that must be strictly positive (dimensionless) was not.
    #[error("damping ratio must be strictly positive")]
    NonPositiveDampingRatio,

    /// The sample time (seconds) must be strictly positive.
    #[error("sample time must be strictly positive")]
    NonPositiveSampleTime,

    /// A discrete transfer function whose leading denominator coefficient
    /// (the `z^0` term of the polynomial in `z^-1`) is zero describes an
    /// acausal system and cannot be simulated forward in time.
    #[error("acausal discrete system: leading denominator coefficient is zero")]
    AcausalSystem,

    /// The requested conversion method is only implemented for system order
    /// <= 2 (`Zoh`, `MatchedPoleZero` need eigenvalues, which this crate
    /// computes analytically). Use `Tustin`/`TustinPrewarp` for higher-order
    /// systems, or see the follow-up bead for a general-order port.
    #[error("conversion method implemented for system order <= 2 only; got order {order}")]
    UnsupportedOrder {
        /// The offending system order (max of numerator and denominator degree).
        order: usize,
    },

    /// `Zoh` discretisation requires a proper transfer function
    /// (numerator degree <= denominator degree).
    #[error("zero-order hold requires a proper transfer function")]
    ImproperTransferFunction,

    /// Matched pole/zero `d2c` cannot map a discrete pole or zero at exactly
    /// `z = 0`, because `ln(0)` diverges (mirrors the upstream Octave error).
    #[error("matched d2c: discrete pole or zero at z = 0 has no finite continuous image")]
    MatchedPoleZeroAtOrigin,

    /// A matched-method pole or zero mapped to a non-finite value
    /// (mirrors the upstream Octave error).
    #[error("matched method produced a non-finite pole or zero")]
    NonFinitePoleOrZero,

    /// The prewarp frequency must satisfy `0 < w0 < pi / T` (below the
    /// Nyquist angular frequency) for `tan(w0 T / 2)` to be positive and
    /// finite.
    #[error("prewarp frequency must lie in (0, pi/T)")]
    InvalidPrewarpFrequency,
}
