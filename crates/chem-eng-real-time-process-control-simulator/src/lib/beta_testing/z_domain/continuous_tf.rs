// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// Ported in part from the GNU Octave control package ("LTI Syncope"),
// https://github.com/gnu-octave/pkg-control, control 4.2.2+,
// commit f39e2625bffc56864c3af746b4410bb71ce6bbb1: inst/tf.m (SISO
// subset of the `tf` model surface). Upstream is
// Copyright (C) 2009-2016 Lukas F. Reichlin, GPL-3.0-or-later.
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

//! SISO continuous-time transfer functions as polynomial coefficient
//! vectors — the minimal `tf` surface the `c2d`/`d2c` port needs.

use uom::si::{f64::*, ratio::ratio, time::second};

use super::polynomial;
use super::ZDomainError;

/// A SISO continuous-time transfer function
///
/// ```text
///           num(s)     num[0] + num[1] s + num[2] s^2 + ...
/// G(s)  =  --------  = ------------------------------------
///           den(s)     den[0] + den[1] s + den[2] s^2 + ...
/// ```
///
/// # Physical quantity
///
/// Maps a dimensionless input signal to a dimensionless output signal in the
/// Laplace domain — the same unit-agnostic convention as the crate's
/// time-domain blocks, which sit between scaled signals in a control loop.
///
/// # Units
///
/// Coefficients are stored **ascending in powers of `s`** and are plain
/// `f64`: the coefficient of `s^k` implicitly carries units of `seconds^k`
/// (SI). A single `uom` type cannot span a coefficient vector whose entries
/// all have different dimensions, so the vector is deliberately untyped and
/// the `uom`-typed constructors ([`Self::first_order`],
/// [`Self::second_order`]) are the recommended entry points.
///
/// Note the **ascending** convention differs from Octave's `tf`, which
/// lists coefficients in descending powers; the conversion is a `reverse`.
///
/// # Valid ranges and assumptions
///
/// - The denominator must not be the zero polynomial
///   ([`ZDomainError::ZeroDenominator`]).
/// - Stability is *not* required or checked here: `d2c` of a discrete
///   system can legitimately produce an unstable continuous model. The
///   stability-guaranteed types live in `stable_transfer_functions`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousTransferFn {
    /// Numerator coefficients, ascending powers of `s` (coefficient of
    /// `s^k` in units of `s^k`).
    num: Vec<f64>,
    /// Denominator coefficients, ascending powers of `s`; never all zero.
    den: Vec<f64>,
}

impl ContinuousTransferFn {
    /// Builds `G(s) = num(s)/den(s)` from coefficient vectors in
    /// **ascending** powers of `s` (`num[k]` multiplies `s^k`).
    ///
    /// Trailing near-zero coefficients are trimmed so that degrees are
    /// meaningful.
    ///
    /// # Errors
    ///
    /// [`ZDomainError::ZeroDenominator`] if `den` is empty or identically
    /// zero.
    pub fn new(num: Vec<f64>, den: Vec<f64>) -> Result<Self, ZDomainError> {
        let num = polynomial::trim(num);
        let den = polynomial::trim(den);
        if den.is_empty() {
            return Err(ZDomainError::ZeroDenominator);
        }
        Ok(ContinuousTransferFn { num, den })
    }

    /// Builds the first-order lag `G(s) = K_p / (tau_p s + 1)` — the same
    /// block as
    /// [`FirstOrderStableTransferFnNoZeroes`](crate::beta_testing::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes),
    /// without dead time.
    ///
    /// # Parameters
    ///
    /// - `process_gain` — `K_p`, dimensionless.
    /// - `process_time` — `tau_p` in seconds; must be strictly positive.
    ///
    /// # Errors
    ///
    /// [`ZDomainError::NonPositiveTimeConstant`] if `process_time <= 0`.
    pub fn first_order(process_gain: Ratio, process_time: Time) -> Result<Self, ZDomainError> {
        let tau = process_time.get::<second>();
        if tau <= 0.0 {
            return Err(ZDomainError::NonPositiveTimeConstant);
        }
        Ok(ContinuousTransferFn {
            num: vec![process_gain.get::<ratio>()],
            den: vec![1.0, tau],
        })
    }

    /// Builds the stable second-order form used across this crate,
    ///
    /// ```text
    ///                     K_p
    /// G(s) = -------------------------------
    ///         tau_p^2 s^2 + 2 zeta tau_p s + 1
    /// ```
    ///
    /// # Parameters
    ///
    /// - `process_gain` — `K_p`, dimensionless.
    /// - `process_time` — `tau_p` in seconds; must be strictly positive.
    /// - `damping_ratio` — `zeta`, dimensionless; must be strictly positive
    ///   (underdamped, critically damped and overdamped are all fine — only
    ///   undamped/negative damping is rejected, matching the stable-block
    ///   convention).
    ///
    /// # Errors
    ///
    /// [`ZDomainError::NonPositiveTimeConstant`] or
    /// [`ZDomainError::NonPositiveDampingRatio`].
    pub fn second_order(
        process_gain: Ratio,
        process_time: Time,
        damping_ratio: Ratio,
    ) -> Result<Self, ZDomainError> {
        let tau = process_time.get::<second>();
        let zeta = damping_ratio.get::<ratio>();
        if tau <= 0.0 {
            return Err(ZDomainError::NonPositiveTimeConstant);
        }
        if zeta <= 0.0 {
            return Err(ZDomainError::NonPositiveDampingRatio);
        }
        Ok(ContinuousTransferFn {
            num: vec![process_gain.get::<ratio>()],
            den: vec![1.0, 2.0 * zeta * tau, tau * tau],
        })
    }

    /// Numerator coefficients, ascending powers of `s` (`s^k` coefficient
    /// in units of `s^k`).
    pub fn numerator_ascending_s(&self) -> &[f64] {
        &self.num
    }

    /// Denominator coefficients, ascending powers of `s`.
    pub fn denominator_ascending_s(&self) -> &[f64] {
        &self.den
    }

    /// System order: the larger of numerator and denominator degree.
    pub fn order(&self) -> usize {
        polynomial::degree(&self.num).max(polynomial::degree(&self.den))
    }

    /// Steady-state (DC) gain `G(0) = num[0]/den[0]`, dimensionless.
    /// Returns infinity for an integrating system (`den[0] = 0`).
    pub fn steady_state_gain(&self) -> f64 {
        let n0 = self.num.first().copied().unwrap_or(0.0);
        let d0 = self.den[0];
        n0 / d0
    }
}
