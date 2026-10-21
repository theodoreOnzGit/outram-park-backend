// SPDX-License-Identifier: GPL-3.0

//! Newtypes for the three quantities this module reports.
//!
//! All three are dimensioned, and `uom` does carry a quantity that matches each
//! one dimensionally. They are newtypes anyway, for a reason worth stating: the
//! `uom` quantity that matches reads as something else entirely.
//!
//! | This | Unit | Dimensionally equal `uom` quantity | Why not that |
//! |---|---|---|---|
//! | [`DilutionFactor`] | s/m^3 | — (no named quantity) | `uom` has none |
//! | [`TimeIntegratedAirConcentration`] | Bq·s/m^3 | `VolumetricNumberDensity` (m^-3) | that counts *particles per volume*; this is *decays per volume*, integrated over time |
//! | [`GroundDeposition`] | Bq/m^2 | `ArealNumberRate` (m^-2 s^-1) | that is a *flux of countable things*; this is an activity per unit area |
//!
//! Activity itself is **not** newtyped: it crosses public boundaries as
//! [`uom::si::f64::Radioactivity`], which has a built-in `@curie` unit, so
//! `3.7e10` never has to be written down.
//!
//! Note that `boon-lay`'s `Activity` alias is `uom::si::f64::Frequency` — the
//! same dimension, a **different Rust type**. Converting between the two is
//! `sembawang`'s job and belongs in exactly one file there, not here.

use core::iter::Sum;
use core::ops::{Add, AddAssign, Mul};

use uom::si::f64::{Radioactivity, Time};
use uom::si::radioactivity::becquerel;
use uom::si::time::second;

/// A dilution factor, `chi/Q`, in **s/m^3**.
///
/// The time-integrated air concentration at a receptor per unit activity
/// released at the source. Multiplying by an activity in Bq gives Bq·s/m^3;
/// see [`TimeIntegratedAirConcentration`].
///
/// It depends only on geometry, wind and stability — never on *what* was
/// released — which is what makes it worth computing once and reusing across
/// every nuclide. [`super::chi_over_q`] relies on that, and
/// `chi_over_q::tests::dilution_is_independent_of_the_activity_released` pins
/// it.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct DilutionFactor(f64);

impl DilutionFactor {
    /// From a bare value in seconds per cubic metre.
    #[must_use]
    pub const fn new(seconds_per_cubic_meter: f64) -> Self {
        Self(seconds_per_cubic_meter)
    }

    /// The value in seconds per cubic metre.
    #[must_use]
    pub const fn seconds_per_cubic_meter(self) -> f64 {
        self.0
    }
}

impl Add for DilutionFactor {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for DilutionFactor {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sum for DilutionFactor {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|d| d.0).sum())
    }
}

/// `chi/Q` times an activity released is a time-integrated air concentration.
impl Mul<Radioactivity> for DilutionFactor {
    type Output = TimeIntegratedAirConcentration;
    fn mul(self, released: Radioactivity) -> TimeIntegratedAirConcentration {
        TimeIntegratedAirConcentration(self.0 * released.get::<becquerel>())
    }
}

/// Time-integrated air concentration at a receptor, in **Bq·s/m^3**.
///
/// The integral of activity concentration over the whole passage of the plume.
/// It is the quantity a dose model would consume, and this crate deliberately
/// stops here: **no dose quantity is computed anywhere in `changi`**.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct TimeIntegratedAirConcentration(f64);

impl TimeIntegratedAirConcentration {
    /// From a bare value in becquerel-seconds per cubic metre.
    #[must_use]
    pub const fn new(becquerel_seconds_per_cubic_meter: f64) -> Self {
        Self(becquerel_seconds_per_cubic_meter)
    }

    /// The value in becquerel-seconds per cubic metre.
    #[must_use]
    pub const fn becquerel_seconds_per_cubic_meter(self) -> f64 {
        self.0
    }

    /// The mean activity concentration over an averaging time, in Bq/m^3.
    ///
    /// The plume's actual peak is higher than this by however much it is
    /// concentrated inside the window; dividing by a long averaging time hides
    /// a short passage. Choose `over` to mean something.
    ///
    /// # Panics
    /// Panics if `over` is not positive.
    #[must_use]
    pub fn mean_concentration_becquerel_per_cubic_meter(self, over: Time) -> f64 {
        let t = over.get::<second>();
        assert!(t > 0.0, "averaging time must be positive; got {t} s");
        self.0 / t
    }
}

impl Add for TimeIntegratedAirConcentration {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for TimeIntegratedAirConcentration {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sum for TimeIntegratedAirConcentration {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|c| c.0).sum())
    }
}

/// Activity deposited on the ground, in **Bq/m^2**.
///
/// Dry deposition only — wet scavenging is not ported, so this is **not an
/// upper bound**: rain would raise it. That is the opposite of the usual
/// conservative framing and must not be glossed over when a number is quoted.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct GroundDeposition(f64);

impl GroundDeposition {
    /// From a bare value in becquerels per square metre.
    #[must_use]
    pub const fn new(becquerel_per_square_meter: f64) -> Self {
        Self(becquerel_per_square_meter)
    }

    /// The value in becquerels per square metre.
    #[must_use]
    pub const fn becquerel_per_square_meter(self) -> f64 {
        self.0
    }
}

impl Add for GroundDeposition {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for GroundDeposition {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sum for GroundDeposition {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.map(|d| d.0).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::radioactivity::curie;

    #[test]
    fn uom_carries_the_curie_so_the_conversion_never_has_to_be_written_out() {
        // The reason activity is not newtyped here. `boon-lay`'s
        // activities/mod.rs claims curies "are not part of uom"; this is the
        // counter-example (filed as GitHub issue #233).
        let a = Radioactivity::new::<curie>(1.0);
        assert!((a.get::<becquerel>() - 3.7e10).abs() < 1.0);
    }

    #[test]
    fn dilution_times_activity_is_a_time_integrated_concentration() {
        let chi_q = DilutionFactor::new(1.0e-5);
        let released = Radioactivity::new::<becquerel>(2.0e12);
        let tic = chi_q * released;
        assert!((tic.becquerel_seconds_per_cubic_meter() - 2.0e7).abs() < 1.0);
    }

    #[test]
    fn mean_concentration_divides_by_the_averaging_time() {
        let tic = TimeIntegratedAirConcentration::new(3600.0);
        let mean = tic.mean_concentration_becquerel_per_cubic_meter(Time::new::<second>(3600.0));
        assert!((mean - 1.0).abs() < 1e-12);
    }
}
