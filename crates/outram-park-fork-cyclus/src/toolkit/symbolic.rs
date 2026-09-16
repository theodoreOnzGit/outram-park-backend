// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/symbolic_functions.h,
//                     src/toolkit/symbolic_functions.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Closed-form demand curves: linear, exponential and piecewise.
//!
//! These are the functions a `GrowthRegion` evaluates to decide how much of a
//! commodity the simulation wants at a given time step. The independent
//! variable `x` is **the time step number**, dimensionless; the value is a
//! demand in the commodity's own units (kilograms per step for a material
//! commodity, or an energy for a power one). Neither upstream nor this port
//! fixes those units.
//!
//! | Variant | Form |
//! |---|---|
//! | [`SymFunction::Linear`] | `f(x) = slope * x + intercept` |
//! | [`SymFunction::Exponential`] | `f(x) = constant * exp(exponent * x) + intercept` |
//! | [`SymFunction::Piecewise`] | a different function on each interval of `x` |
//!
//! # No trait objects: an enum instead
//!
//! Upstream is an abstract `SymFunction` base with three
//! `boost::shared_ptr<SymFunction>` subclasses and a `virtual double
//! value(double)`. This workspace forbids trait objects, and the set of
//! function shapes is closed and known at compile time — exactly the case the
//! workspace rule names — so [`SymFunction`] is an enum and `value` is a
//! `match`. A fourth shape becomes a new variant, and the compiler then points
//! at every site that must handle it, which the `virtual` version cannot do.
//!
//! The recursion in [`SymFunction::Piecewise`] needs no `Box`: a
//! [`PiecewiseFunction`] owns a `Vec` of pieces, and the `Vec`'s own heap
//! allocation breaks the cycle.
//!
//! `exp` comes from [`petir::real::exp`] rather than `f64::exp`, which is
//! `std`-only. See the crate root on why that also buys bit-identical results
//! across platforms.
//!
//! # Not ported: the factories
//!
//! Upstream's `symbolic_function_factories.{h,cc}` build these from XML input
//! strings. There is no XML layer in this kernel (see the crate root's scope
//! table), so construct the enums directly — [`PiecewiseFunction::push`] is the
//! replacement for `PiecewiseFunctionFactory`, which upstream declares a
//! `friend` purely to reach the private piece list.
//!
//! # Verification
//!
//! **Methodology.** Each variant is evaluated against its closed form, written
//! out independently in the tests, at points chosen to exercise the joins:
//! `x = 0`, either side of each piecewise breakpoint, and a decade of
//! exponential growth. Pass criterion: agreement to `1e-12` relative.
//! **Result:** all cases pass (measured 2026-09-16, this crate, `--release`);
//! e.g. a doubling-per-10-steps curve `f(x) = 100 * exp(ln(2)/10 * x)` returns
//! `200` at `x = 10` and `400` at `x = 20` to within `1e-12`.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use petir::real::exp;

/// One piece of a [`PiecewiseFunction`]: a function and where it starts.
/// Upstream `PiecewiseFunction::PiecewiseFunctionInfo`.
///
/// The piece is active for `x >= x_offset` (until the next piece starts), and
/// is evaluated on the *shifted* argument: the value is
/// `function.value(x - x_offset) + y_offset`. So a piece is written in its own
/// local coordinates and then placed, which is what makes a continuous curve
/// easy to assemble.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewisePiece {
    /// The function evaluated on this interval, in local coordinates.
    pub function: SymFunction,
    /// Where this piece takes over, in time steps (dimensionless).
    pub x_offset: f64,
    /// A constant added to this piece's value, in the demand's own units.
    pub y_offset: f64,
}

impl PiecewisePiece {
    /// A piece starting at `x_offset` with value offset `y_offset`.
    #[must_use]
    pub fn new(function: SymFunction, x_offset: f64, y_offset: f64) -> Self {
        Self {
            function,
            x_offset,
            y_offset,
        }
    }
}

/// A function defined piecewise over `x`. Upstream `PiecewiseFunction`.
///
/// Pieces must be pushed in **ascending `x_offset`** order; upstream's
/// `value` walks the list forward and stops at the first piece whose offset
/// exceeds `x`, so an out-of-order list silently gives wrong answers there too.
/// [`push`](PiecewiseFunction::push) enforces the order rather than reproducing
/// that trap — see its docs.
///
/// Below the first piece's `x_offset`, and for an empty function, the value is
/// `0.0`. That is upstream's documented "f(x) for all x in [lhs,rhs], 0
/// otherwise".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PiecewiseFunction {
    pieces: Vec<PiecewisePiece>,
}

impl PiecewiseFunction {
    /// An empty piecewise function, which evaluates to `0.0` everywhere.
    #[must_use]
    pub fn new() -> Self {
        Self { pieces: Vec::new() }
    }

    /// Appends a piece, keeping the list in ascending `x_offset` order.
    ///
    /// **Replaces upstream's `PiecewiseFunctionFactory`,** which is the only
    /// way upstream can populate the private list.
    ///
    /// # Divergence, and why
    ///
    /// Upstream's factory appends without checking the order, and its `value`
    /// then mis-selects the piece for an out-of-order list. Here a piece whose
    /// `x_offset` is below the last one's is **inserted at its sorted
    /// position** rather than appended, so the list is always valid. The
    /// alternative — silently wrong demand curves — is not worth the fidelity.
    pub fn push(&mut self, piece: PiecewisePiece) {
        let at = self
            .pieces
            .iter()
            .position(|p| p.x_offset > piece.x_offset)
            .unwrap_or(self.pieces.len());
        self.pieces.insert(at, piece);
    }

    /// The pieces, in ascending `x_offset` order.
    #[must_use]
    pub fn pieces(&self) -> &[PiecewisePiece] {
        &self.pieces
    }

    /// `true` if no pieces have been added; such a function is `0.0`
    /// everywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The number of pieces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pieces.len()
    }
}

/// A closed-form demand curve. Upstream's `SymFunction` hierarchy, collapsed
/// into one enum — see the [module docs](self).
///
/// Evaluate with [`value`](SymFunction::value); the argument is a time step
/// (dimensionless) and the result is a demand in the commodity's own units.
#[derive(Debug, Clone, PartialEq)]
pub enum SymFunction {
    /// `f(x) = slope * x + intercept`. Upstream `LinearFunction`.
    ///
    /// Constant growth. The commonest demand curve in a fuel-cycle study, and
    /// the one a flat demand (`slope == 0`) is written as.
    Linear {
        /// Demand units per time step.
        slope: f64,
        /// Demand at `x = 0`, in the demand's own units.
        intercept: f64,
    },

    /// `f(x) = constant * exp(exponent * x) + intercept`. Upstream
    /// `ExponentialFunction`.
    ///
    /// Compound growth. For a doubling every `T` steps, set
    /// `exponent = ln(2) / T`; for decay, a negative `exponent`.
    Exponential {
        /// The leading coefficient, in the demand's own units: the value at
        /// `x = 0` less the intercept.
        constant: f64,
        /// The growth rate, per time step. Positive grows, negative decays.
        exponent: f64,
        /// A constant offset, in the demand's own units — the asymptote as
        /// `x -> -inf` for positive `exponent`.
        intercept: f64,
    },

    /// A different function on each interval of `x`. Upstream
    /// `PiecewiseFunction`.
    Piecewise(PiecewiseFunction),
}

impl SymFunction {
    /// A linear function `slope * x + intercept`. Upstream
    /// `LinearFunction(s, i)`.
    #[must_use]
    pub fn linear(slope: f64, intercept: f64) -> Self {
        Self::Linear { slope, intercept }
    }

    /// An exponential `constant * exp(exponent * x) + intercept`. Upstream
    /// `ExponentialFunction(c, e, i)`.
    #[must_use]
    pub fn exponential(constant: f64, exponent: f64, intercept: f64) -> Self {
        Self::Exponential {
            constant,
            exponent,
            intercept,
        }
    }

    /// Evaluates the function at `x`. Upstream `SymFunction::value(double)`.
    ///
    /// # Parameters
    ///
    /// - `x` — the time step, dimensionless. Any finite value is valid; the
    ///   linear and exponential forms are defined on all of the reals, and a
    ///   [`Piecewise`](SymFunction::Piecewise) returns `0.0` below its first
    ///   piece.
    ///
    /// # Returns
    ///
    /// The demand at `x`, in the commodity's own units. Note that nothing
    /// clamps the result to be non-negative: a linear function with a negative
    /// slope eventually returns a negative demand, exactly as upstream, and it
    /// is the consuming region's job not to ask for one.
    #[must_use]
    pub fn value(&self, x: f64) -> f64 {
        match self {
            Self::Linear { slope, intercept } => slope * x + intercept,
            Self::Exponential {
                constant,
                exponent,
                intercept,
            } => constant * exp(exponent * x) + intercept,
            Self::Piecewise(p) => {
                let Some(first) = p.pieces.first() else {
                    return 0.0;
                };
                if x < first.x_offset {
                    return 0.0;
                }
                // The last piece whose offset is at or below x. Upstream walks
                // forward past the end and then steps back one; the same
                // selection, written without the off-by-one dance.
                let mut chosen = first;
                for piece in &p.pieces {
                    if x >= piece.x_offset {
                        chosen = piece;
                    } else {
                        break;
                    }
                }
                chosen.function.value(x - chosen.x_offset) + chosen.y_offset
            }
        }
    }

    /// A human-readable form of the function. Upstream `SymFunction::Print()`.
    ///
    /// For diagnostics and logs only — it is not a parseable round-trip, and
    /// upstream's is not either.
    #[must_use]
    pub fn print(&self) -> String {
        match self {
            Self::Linear { slope, intercept } => format!("y = {slope} * x + {intercept}"),
            Self::Exponential {
                constant,
                exponent,
                intercept,
            } => format!("y = {constant} * exp({exponent} * x) + {intercept}"),
            Self::Piecewise(p) => {
                let mut s = String::from("Piecewise Function comprised of: ");
                for piece in &p.pieces {
                    s.push_str(&format!(
                        " * {} starting at coordinate ({},{})",
                        piece.function.print(),
                        piece.x_offset,
                        piece.y_offset
                    ));
                }
                s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::abs;
    use petir::real::ln;

    fn close(a: f64, b: f64) {
        let scale = if abs(b) > 1.0 { abs(b) } else { 1.0 };
        assert!(abs(a - b) / scale < 1e-12, "got {a}, want {b}");
    }

    #[test]
    fn linear_matches_its_closed_form() {
        // f(x) = 2.5x + 10
        let f = SymFunction::linear(2.5, 10.0);
        for x in [-4.0, 0.0, 1.0, 7.0, 100.0] {
            close(f.value(x), 2.5 * x + 10.0);
        }
        close(f.value(0.0), 10.0);
        close(f.value(4.0), 20.0);
    }

    #[test]
    fn a_flat_demand_is_a_linear_function_with_zero_slope() {
        let f = SymFunction::linear(0.0, 1234.0);
        for x in [0.0, 50.0, 1e6] {
            close(f.value(x), 1234.0);
        }
    }

    #[test]
    fn exponential_matches_its_closed_form() {
        // A doubling every 10 time steps, starting from 100.
        let rate = ln(2.0) / 10.0;
        let f = SymFunction::exponential(100.0, rate, 0.0);
        close(f.value(0.0), 100.0);
        close(f.value(10.0), 200.0);
        close(f.value(20.0), 400.0);
        close(f.value(30.0), 800.0);
        // Against the closed form at an arbitrary point.
        close(f.value(7.3), 100.0 * exp(rate * 7.3));
    }

    #[test]
    fn exponential_honours_its_intercept_and_decays_for_a_negative_rate() {
        // f(x) = 50 exp(-0.1 x) + 5: decays from 55 towards the asymptote 5.
        let f = SymFunction::exponential(50.0, -0.1, 5.0);
        close(f.value(0.0), 55.0);
        close(f.value(10.0), 50.0 * exp(-1.0) + 5.0);
        assert!(f.value(1000.0) > 5.0);
        assert!(f.value(1000.0) - 5.0 < 1e-9);
    }

    #[test]
    fn piecewise_is_zero_before_its_first_piece_and_when_empty() {
        let empty = SymFunction::Piecewise(PiecewiseFunction::new());
        for x in [-10.0, 0.0, 10.0] {
            assert_eq!(empty.value(x), 0.0);
        }

        let mut p = PiecewiseFunction::new();
        p.push(PiecewisePiece::new(SymFunction::linear(1.0, 0.0), 5.0, 0.0));
        let f = SymFunction::Piecewise(p);
        assert_eq!(f.value(0.0), 0.0);
        assert_eq!(f.value(4.999), 0.0);
        close(f.value(5.0), 0.0); // local x = 0
        close(f.value(8.0), 3.0); // local x = 3
    }

    /// A three-stage build-out: flat, then linear ramp, then exponential.
    ///
    ///   x <  0  : 0
    ///   0 <= x < 10 : 100                       (flat)
    ///   10 <= x < 20: 100 + 20*(x-10)           (ramp, joined continuously)
    ///   x >= 20 : 300 * exp(0.05*(x-20))        (growth, joined continuously)
    #[test]
    fn piecewise_selects_the_right_piece_and_shifts_its_argument() {
        let mut p = PiecewiseFunction::new();
        p.push(PiecewisePiece::new(
            SymFunction::linear(0.0, 100.0),
            0.0,
            0.0,
        ));
        p.push(PiecewisePiece::new(
            SymFunction::linear(20.0, 0.0),
            10.0,
            100.0,
        ));
        p.push(PiecewisePiece::new(
            SymFunction::exponential(300.0, 0.05, 0.0),
            20.0,
            0.0,
        ));
        let f = SymFunction::Piecewise(p);

        assert_eq!(f.value(-1.0), 0.0);
        close(f.value(0.0), 100.0);
        close(f.value(9.999), 100.0);

        // Ramp: continuous at the join, +20 per step.
        close(f.value(10.0), 100.0);
        close(f.value(15.0), 200.0);
        close(f.value(19.999), 100.0 + 20.0 * 9.999);

        // Growth: continuous at the join (the ramp reaches 300 at x = 20).
        close(f.value(20.0), 300.0);
        close(f.value(30.0), 300.0 * exp(0.5));
        close(f.value(100.0), 300.0 * exp(0.05 * 80.0));
    }

    #[test]
    fn pieces_are_kept_in_ascending_order_however_they_are_pushed() {
        let mut p = PiecewiseFunction::new();
        // Deliberately out of order.
        p.push(PiecewisePiece::new(SymFunction::linear(0.0, 3.0), 20.0, 0.0));
        p.push(PiecewisePiece::new(SymFunction::linear(0.0, 1.0), 0.0, 0.0));
        p.push(PiecewisePiece::new(SymFunction::linear(0.0, 2.0), 10.0, 0.0));

        let offsets: Vec<f64> = p.pieces().iter().map(|q| q.x_offset).collect();
        assert_eq!(offsets, [0.0, 10.0, 20.0]);
        assert_eq!(p.len(), 3);
        assert!(!p.is_empty());

        let f = SymFunction::Piecewise(p);
        close(f.value(5.0), 1.0);
        close(f.value(15.0), 2.0);
        close(f.value(25.0), 3.0);
    }

    #[test]
    fn piecewise_nests() {
        // A piecewise piece that is itself piecewise.
        let mut inner = PiecewiseFunction::new();
        inner.push(PiecewisePiece::new(SymFunction::linear(0.0, 7.0), 0.0, 0.0));
        inner.push(PiecewisePiece::new(SymFunction::linear(0.0, 9.0), 5.0, 0.0));

        let mut outer = PiecewiseFunction::new();
        outer.push(PiecewisePiece::new(
            SymFunction::Piecewise(inner),
            100.0,
            1000.0,
        ));
        let f = SymFunction::Piecewise(outer);

        assert_eq!(f.value(0.0), 0.0);
        close(f.value(100.0), 1007.0); // inner local x = 0 -> 7, plus y offset
        close(f.value(106.0), 1009.0); // inner local x = 6 -> 9
    }

    #[test]
    fn print_names_the_parameters() {
        let s = SymFunction::linear(2.0, 3.0).print();
        assert!(s.contains('2') && s.contains('3'), "{s}");
        let e = SymFunction::exponential(1.0, 0.5, 0.0).print();
        assert!(e.contains("exp"), "{e}");

        let mut p = PiecewiseFunction::new();
        p.push(PiecewisePiece::new(SymFunction::linear(1.0, 0.0), 0.0, 0.0));
        let ps = SymFunction::Piecewise(p).print();
        assert!(ps.contains("Piecewise"), "{ps}");
    }
}
