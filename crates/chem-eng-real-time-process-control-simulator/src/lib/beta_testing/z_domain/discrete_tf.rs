// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// Ported in part from the GNU Octave control package ("LTI Syncope"),
// https://github.com/gnu-octave/pkg-control, control 4.2.2+,
// commit f39e2625bffc56864c3af746b4410bb71ce6bbb1: inst/filt.m (the
// DSP-format discrete transfer-function surface, SISO subset). Upstream is
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

//! SISO discrete-time transfer functions in DSP (`z^-1`) form, advanced by
//! an O(1) fixed-state recurrence — the `filt` surface of the Octave port.
//!
//! The stepping recurrence is the direct-form-II-transposed realisation of
//! the difference equation, which carries exactly `max(deg num, deg den)`
//! state values — a fixed number. This deliberately matches the crate-wide
//! rule that a block must never accumulate a growing history of its inputs
//! (bead `op-fm5`).

use uom::si::{f64::*, ratio::ratio, time::second};

use super::polynomial;
use super::ZDomainError;

/// A SISO discrete-time transfer function with sample time `T`, stored in
/// DSP format (Octave `filt` convention — ascending powers of `z^-1`):
///
/// ```text
///             b0 + b1 z^-1 + b2 z^-2 + ...
/// G(z^-1) = --------------------------------,   a0 = 1 after normalisation
///             a0 + a1 z^-1 + a2 z^-2 + ...
/// ```
///
/// equivalent to the difference equation (with `u` the input samples and
/// `y` the output samples)
///
/// ```text
/// y[n] = b0 u[n] + b1 u[n-1] + ... - a1 y[n-1] - a2 y[n-2] - ...
/// ```
///
/// # Physical quantity
///
/// Maps a dimensionless input sample stream to a dimensionless output
/// sample stream (`uom` `Ratio` both ways), one sample per `sample_time`.
/// The z-polynomial coefficients are genuinely dimensionless, so they are
/// plain `f64` by design; the sample time is a physical `uom`
/// [`Time`](uom::si::f64::Time) in seconds.
///
/// # Valid ranges and assumptions
///
/// - `sample_time` must be strictly positive.
/// - The `z^0` denominator coefficient `a0` must be nonzero (otherwise the
///   difference equation would need future inputs — an acausal system) and
///   is normalised to 1 on construction.
/// - Samples must be fed at the fixed sample interval; unlike the
///   continuous-time blocks in `stable_transfer_functions`, a discrete
///   transfer function has no meaning between samples and cannot absorb an
///   irregular timestep. If your simulator steps irregularly, keep using
///   the continuous blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteTransferFn {
    /// Numerator coefficients `b0, b1, ...` (ascending powers of `z^-1`,
    /// dimensionless), scaled so that the denominator's `a0` is 1.
    num: Vec<f64>,
    /// Denominator coefficients `a0 = 1, a1, ...` (ascending powers of
    /// `z^-1`, dimensionless).
    den: Vec<f64>,
    /// Sample interval `T` in seconds; strictly positive.
    sample_time: Time,
    /// Direct-form-II-transposed state; fixed length
    /// `max(num.len(), den.len()) - 1`, never grows.
    state: Vec<f64>,
}

impl DiscreteTransferFn {
    /// Builds a discrete transfer function from coefficients in **ascending
    /// powers of `z^-1`** (DSP format — Octave's `filt` convention).
    ///
    /// Octave's own docstring example, `filt([0, 3], [1, 4, 2])`, is
    ///
    /// ```text
    ///                 3 z^-1
    /// H(z^-1) = -------------------
    ///           1 + 4 z^-1 + 2 z^-2
    /// ```
    ///
    /// # Parameters
    ///
    /// - `num` — `b0, b1, ...`, dimensionless.
    /// - `den` — `a0, a1, ...`, dimensionless; `a0` must be nonzero and the
    ///   stored coefficients are normalised by it.
    /// - `sample_time` — `T` in seconds; strictly positive. (Octave allows
    ///   an "unspecified" sample time of -1; this port does not — a block
    ///   that will be stepped in a real-time simulator needs a real `T`.)
    ///
    /// # Errors
    ///
    /// [`ZDomainError::NonPositiveSampleTime`],
    /// [`ZDomainError::ZeroDenominator`], or
    /// [`ZDomainError::AcausalSystem`] (zero `a0`).
    pub fn from_z_inverse_coefficients(
        num: Vec<f64>,
        den: Vec<f64>,
        sample_time: Time,
    ) -> Result<Self, ZDomainError> {
        if sample_time.get::<second>() <= 0.0 {
            return Err(ZDomainError::NonPositiveSampleTime);
        }
        // Trailing zeros in z^-1 form are harmless but wasteful state:
        // trim them (this cannot change a0).
        let num = polynomial::trim(num);
        let den = polynomial::trim(den);
        if den.is_empty() {
            return Err(ZDomainError::ZeroDenominator);
        }
        let a0 = den[0];
        if a0 == 0.0 {
            return Err(ZDomainError::AcausalSystem);
        }
        let num = polynomial::scale(&num, 1.0 / a0);
        let den = polynomial::scale(&den, 1.0 / a0);
        let n_state = num.len().max(den.len()).saturating_sub(1);
        Ok(DiscreteTransferFn {
            num,
            den,
            sample_time,
            state: vec![0.0; n_state],
        })
    }

    /// Builds a discrete transfer function from coefficients in
    /// **descending powers of `z`** (Octave's `tf(num, den, tsam)`
    /// convention): `num[0]` multiplies the highest power.
    ///
    /// The polynomials are converted to `z^-1` form by dividing through by
    /// `z^N` (`N` the larger degree), which left-pads the lower-degree
    /// polynomial with zeros. A numerator of higher degree than the
    /// denominator is acausal and rejected.
    ///
    /// # Errors
    ///
    /// [`ZDomainError::NonPositiveSampleTime`],
    /// [`ZDomainError::ZeroDenominator`], or
    /// [`ZDomainError::AcausalSystem`] (numerator degree exceeds
    /// denominator degree).
    pub fn from_z_descending_coefficients(
        num_descending_z: Vec<f64>,
        den_descending_z: Vec<f64>,
        sample_time: Time,
    ) -> Result<Self, ZDomainError> {
        // Ascending-in-z form for trimming, then back to descending.
        let mut num_asc: Vec<f64> = num_descending_z.into_iter().rev().collect();
        let mut den_asc: Vec<f64> = den_descending_z.into_iter().rev().collect();
        num_asc = polynomial::trim(num_asc);
        den_asc = polynomial::trim(den_asc);
        if den_asc.is_empty() {
            return Err(ZDomainError::ZeroDenominator);
        }
        let deg_num = polynomial::degree(&num_asc);
        let deg_den = polynomial::degree(&den_asc);
        if !num_asc.is_empty() && deg_num > deg_den {
            return Err(ZDomainError::AcausalSystem);
        }
        // Divide both by z^deg_den: coefficient of z^k becomes coefficient
        // of z^-(deg_den - k). In z^-1-ascending form, index j = deg_den - k.
        let mut num_zinv = vec![0.0; deg_den + 1];
        for (k, &c) in num_asc.iter().enumerate() {
            num_zinv[deg_den - k] = c;
        }
        let mut den_zinv = vec![0.0; deg_den + 1];
        for (k, &c) in den_asc.iter().enumerate() {
            den_zinv[deg_den - k] = c;
        }
        Self::from_z_inverse_coefficients(num_zinv, den_zinv, sample_time)
    }

    /// Advances the block by exactly one sample interval, applying the
    /// input sample `u[n]` and returning the output sample `y[n]`.
    ///
    /// Input and output are dimensionless (`Ratio`). Cost is O(1) in the
    /// number of samples taken so far: one pass over the fixed-length
    /// coefficient arrays (direct-form-II-transposed update); no history is
    /// stored (bead `op-fm5` discipline).
    pub fn advance_one_sample(&mut self, input: Ratio) -> Ratio {
        let u = input.get::<ratio>();
        let b0 = self.num.first().copied().unwrap_or(0.0);
        let y = b0 * u + self.state.first().copied().unwrap_or(0.0);

        // state[i] <- b_{i+1} u + state[i+1] - a_{i+1} y
        let n = self.state.len();
        for i in 0..n {
            let b_next = self.num.get(i + 1).copied().unwrap_or(0.0);
            let a_next = self.den.get(i + 1).copied().unwrap_or(0.0);
            let carry = if i + 1 < n { self.state[i + 1] } else { 0.0 };
            self.state[i] = b_next * u + carry - a_next * y;
        }
        Ratio::new::<ratio>(y)
    }

    /// Resets the internal state to zero (a block at rest with zero past
    /// inputs and outputs).
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|s| *s = 0.0);
    }

    /// Numerator coefficients in ascending powers of `z^-1` (dimensionless;
    /// `a0`-normalised).
    pub fn numerator_z_inverse(&self) -> &[f64] {
        &self.num
    }

    /// Denominator coefficients in ascending powers of `z^-1`
    /// (dimensionless; leading coefficient is exactly 1).
    pub fn denominator_z_inverse(&self) -> &[f64] {
        &self.den
    }

    /// The sample interval `T` (seconds).
    pub fn sample_time(&self) -> Time {
        self.sample_time
    }

    /// Number of state values the block carries. Fixed at construction
    /// (`max(deg num, deg den)`); regression tests assert it never grows
    /// with the sample index.
    pub fn state_size(&self) -> usize {
        self.state.len()
    }
}
