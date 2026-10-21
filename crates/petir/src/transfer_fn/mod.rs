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
// PROVENANCE — LIFTED from
//     crates/chem-eng-real-time-process-control-simulator/
//       src/lib/beta_testing/z_domain/
// which is itself a port of the GNU Octave control package (GPL-3.0-or-later).
// Both are GPL-3.0 and in-workspace, so no new licence obligation arises.
//
// The five submodules (`cplx`, `polynomial`, `continuous_tf`, `discrete_tf`,
// `conversion`) are byte-for-byte copies with only the mechanical no_std edits:
// `std::ops` / `std::f64::consts` -> their `core` equivalents, plus the `alloc`
// imports `std`'s prelude supplied for free and the `crate::real::Real` import
// that restores `sqrt`/`exp`/`ln` method syntax. No numeric expression was
// touched. `tests/verbatim_provenance.rs` re-checks that mechanically.
//
// THIS FILE is the exception: `ZDomainError` is re-expressed WITHOUT
// `thiserror`, which requires `std`. The variants, their names, their order and
// their messages are unchanged; only the derive is replaced by a hand-written
// `Display`. See the type's own note.

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

// The O(1) fixed-state recurrence blocks, lifted from the source crate's
// `stable_transfer_functions`. Each advances a specific transfer-function shape
// sample by sample in constant time and constant memory, rather than through
// the general coefficient machinery in `discrete_tf`. They are what
// `verification_tests` checks `conversion`'s general `c2d` against: converting a
// first-order lag with `C2dMethod::Zoh` and stepping the result must reproduce
// `FirstOrderStableTransferFnNoZeroes` sample for sample.
pub mod ratio_ext;

/// Underdamped second-order step response as a decaying sinusoid, advanced by
/// an O(1) fixed-state recurrence.
pub mod decaying_sinusoid;
/// First-order lag `K / (1 + tau s)` with dead time, advanced by an O(1)
/// fixed-state recurrence. Also hosts `PendingStepInput`, the queued-step type
/// the other blocks share.
pub mod first_order_transfer_fn;
/// First-order transfer function WITH a numerator zero,
/// `K (1 + tau_z s) / (1 + tau_p s)`.
//
// `dead_code` is allowed because this file is a verbatim lift and one of its
// test helpers, `reference_response`, is exercised only by the source crate's
// `recurrence_tests` -- which is NOT lifted, because it depends on the
// `transfer_fn_wrapper_and_enums` layer and that layer writes CSV files through
// `csv::Writer`, a std-and-filesystem dependency with no place in a no_std
// numerics crate. Deleting the helper here would break the verbatim property
// for no gain; the alternative is dragging std into the crate.
#[allow(missing_docs, dead_code)]
pub mod first_order_transfer_fn_with_zeroes;
/// Second-order transfer function `K / (1 + 2 zeta tau s + tau^2 s^2)`,
/// covering the overdamped, critically damped and underdamped cases.
pub mod second_order_transfer_fn;
/// Unit step input helper shared by the recurrence blocks.
pub mod step_fn;


#[cfg(test)]
mod verification_tests;

pub use continuous_tf::ContinuousTransferFn;
pub use conversion::{C2dMethod, D2cMethod};
pub use discrete_tf::DiscreteTransferFn;


/// Errors from z-domain construction and conversion.
///
/// # Why this is hand-written rather than derived
///
/// The source crate derives this with `thiserror`, which requires `std` and so
/// cannot be used here — the same constraint that shapes [`crate::PetirError`].
/// The variants, their names, their order and their message text are carried
/// over unchanged; only the derive is replaced by an explicit
/// [`Display`](core::fmt::Display) impl, so a reader diffing this against the
/// source sees one substitution rather than a redesign.
///
/// It is deliberately separate from [`crate::PetirError`], mirroring the
/// source's own reasoning: the transfer-function layer stays self-contained,
/// and its failure modes ("acausal system", "prewarp frequency out of range")
/// are control-theory conditions with no sensible GSL counterpart. Where a
/// caller wants one error type, [`From<ZDomainError>`](core::convert::From) is
/// implemented for [`crate::PetirError`] below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ZDomainError {
    /// A denominator polynomial was empty or identically zero.
    ZeroDenominator,

    /// A time constant that must be strictly positive (seconds) was not.
    NonPositiveTimeConstant,

    /// A damping ratio that must be strictly positive (dimensionless) was not.
    NonPositiveDampingRatio,

    /// The sample time (seconds) must be strictly positive.
    NonPositiveSampleTime,

    /// A discrete transfer function whose leading denominator coefficient
    /// (the `z^0` term of the polynomial in `z^-1`) is zero describes an
    /// acausal system and cannot be simulated forward in time.
    AcausalSystem,

    /// The requested conversion method is only implemented for system order
    /// <= 2 (`Zoh`, `MatchedPoleZero` need eigenvalues, which this module
    /// computes analytically). Use `Tustin`/`TustinPrewarp` for higher-order
    /// systems.
    UnsupportedOrder {
        /// The offending system order (max of numerator and denominator degree).
        order: usize,
    },

    /// `Zoh` discretisation requires a proper transfer function
    /// (numerator degree <= denominator degree).
    ImproperTransferFunction,

    /// Matched pole/zero `d2c` cannot map a discrete pole or zero at exactly
    /// `z = 0`, because `ln(0)` diverges (mirrors the upstream Octave error).
    MatchedPoleZeroAtOrigin,

    /// A matched-method pole or zero mapped to a non-finite value
    /// (mirrors the upstream Octave error).
    NonFinitePoleOrZero,

    /// The prewarp frequency must satisfy `0 < w0 < pi / T` (below the
    /// Nyquist angular frequency) for `tan(w0 T / 2)` to be positive and
    /// finite.
    InvalidPrewarpFrequency,

    /// A damping ratio outside the range a STABLE second-order block is
    /// defined for.
    ///
    /// Carried over from the source crate's
    /// `ChemEngProcessControlSimulatorError`, which the lifted recurrence
    /// blocks raise. It is the only variant of that type those blocks actually
    /// use, so rather than lift a second error enum -- one carrying a
    /// `csv::Error` and a `String`, neither of which belongs in a `no_std`
    /// numerics crate -- the single live variant is folded in here and the old
    /// name is kept as the alias below.
    UnstableDampingFactorForStableTransferFunction,
}

/// Compatibility alias so the lifted recurrence blocks compile unchanged.
///
/// The source crate names this error `ChemEngProcessControlSimulatorError`. The
/// lift rewrites the *path* to point here but leaves every use site's spelling
/// alone, so those files still diff clean against their source.
pub use ZDomainError as ChemEngProcessControlSimulatorError;

impl core::fmt::Display for ZDomainError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ZDomainError::ZeroDenominator => {
                write!(f, "denominator polynomial is empty or zero")
            }
            ZDomainError::NonPositiveTimeConstant => {
                write!(f, "time constant must be strictly positive")
            }
            ZDomainError::NonPositiveDampingRatio => {
                write!(f, "damping ratio must be strictly positive")
            }
            ZDomainError::NonPositiveSampleTime => {
                write!(f, "sample time must be strictly positive")
            }
            ZDomainError::AcausalSystem => write!(
                f,
                "acausal discrete system: leading denominator coefficient is zero"
            ),
            ZDomainError::UnsupportedOrder { order } => write!(
                f,
                "conversion method implemented for system order <= 2 only; got order {order}"
            ),
            ZDomainError::ImproperTransferFunction => {
                write!(f, "zero-order hold requires a proper transfer function")
            }
            ZDomainError::MatchedPoleZeroAtOrigin => write!(
                f,
                "matched d2c: discrete pole or zero at z = 0 has no finite continuous image"
            ),
            ZDomainError::NonFinitePoleOrZero => {
                write!(f, "matched method produced a non-finite pole or zero")
            }
            ZDomainError::InvalidPrewarpFrequency => {
                write!(f, "prewarp frequency must lie in (0, pi/T)")
            }
            ZDomainError::UnstableDampingFactorForStableTransferFunction => {
                write!(f, "Unstable Damping Factor for Stable Transfer Function")
            }
        }
    }
}

impl core::error::Error for ZDomainError {}

impl From<ZDomainError> for crate::PetirError {
    /// Collapse a z-domain failure into the crate-wide error.
    ///
    /// The mapping is deliberately lossy and one-way: it exists so a caller
    /// mixing transfer functions with the numerics layer can use a single `?`
    /// type, not as the primary way to inspect what went wrong. Match on
    /// [`ZDomainError`] itself when the distinction matters — every variant
    /// here names a specific, actionable condition, and several collapse onto
    /// the same [`crate::PetirError`] variant.
    fn from(e: ZDomainError) -> Self {
        match e {
            // Arguments outside the domain the routine is defined on.
            ZDomainError::NonPositiveTimeConstant
            | ZDomainError::NonPositiveDampingRatio
            | ZDomainError::NonPositiveSampleTime
            | ZDomainError::InvalidPrewarpFrequency => crate::PetirError::Domain,
            // Structurally malformed systems.
            ZDomainError::ZeroDenominator
            | ZDomainError::AcausalSystem
            | ZDomainError::ImproperTransferFunction
            | ZDomainError::MatchedPoleZeroAtOrigin
            | ZDomainError::NonFinitePoleOrZero => crate::PetirError::Invalid,
            // A subset of upstream's generality that PETIR does not cover.
            ZDomainError::UnsupportedOrder { .. } => crate::PetirError::Unimplemented,
            ZDomainError::UnstableDampingFactorForStableTransferFunction => {
                crate::PetirError::Domain
            }
        }
    }
}
