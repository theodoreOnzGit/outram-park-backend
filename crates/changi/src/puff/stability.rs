// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/helpers.R` — `is_day`, `get_stab_class`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! Pasquill stability classification from wind speed and time of day.
//!
//! The Pasquill–Gifford scheme sorts the atmosphere into six turbulence
//! regimes, `A` (strongly unstable, vigorous daytime convection) through `F`
//! (strongly stable, calm clear night). The class selects the dispersion
//! coefficients in [`super::dispersion`], so it is the single largest control
//! on how fast a plume spreads.
//!
//! Upstream's rule is a lookup on wind speed crossed with day/night, and it
//! returns **one or two** classes: the original Pasquill table is a range, not
//! a point, and upstream preserves that ambiguity rather than picking a
//! midpoint. [`StabilitySet`] carries the one-or-two distinction in the type,
//! so a caller cannot forget the second class exists.

use uom::si::f64::Velocity;
use uom::si::velocity::meter_per_second;

/// A Pasquill–Gifford atmospheric stability class.
///
/// Ordered from most unstable to most stable. `A` is a hot, calm, sunny
/// afternoon — strong convection, rapid vertical mixing, a plume that fattens
/// quickly and dilutes fast. `F` is a clear, calm night — a stable layer that
/// suppresses vertical motion, so a plume stays narrow and travels far at high
/// concentration. `D` is neutral, typical of overcast or windy conditions, and
/// is the default when nothing better is known.
///
/// Dispatched with a `match` (no trait objects, per the workspace Rust design
/// rules), so adding a class would force every consumer to handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StabilityClass {
    /// Extremely unstable — strong daytime convection, light wind.
    A,
    /// Moderately unstable.
    B,
    /// Slightly unstable.
    C,
    /// Neutral — overcast or windy, day or night. The fallback class.
    D,
    /// Slightly stable — night, light-to-moderate wind.
    E,
    /// Moderately stable — clear, calm night.
    F,
}

impl StabilityClass {
    /// The single-letter label upstream uses (`"A"` … `"F"`).
    #[must_use]
    pub fn letter(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
        }
    }

    /// Parse a single-letter label, accepting either case.
    ///
    /// Upstream applies `toupper` before its lookup and yields `NA` for
    /// anything unrecognised; this returns `None` in that case rather than
    /// propagating a sentinel.
    ///
    /// # Arguments
    /// - `letter` — one of `A`–`F`, upper or lower case.
    #[must_use]
    pub fn from_letter(letter: &str) -> Option<Self> {
        match letter {
            "A" | "a" => Some(Self::A),
            "B" | "b" => Some(Self::B),
            "C" | "c" => Some(Self::C),
            "D" | "d" => Some(Self::D),
            "E" | "e" => Some(Self::E),
            "F" | "f" => Some(Self::F),
            _ => None,
        }
    }
}

/// The one or two stability classes the Pasquill table admits for a condition.
///
/// Upstream returns an R character vector of length 1 or 2. **Six** of its ten
/// (wind speed × day/night) regimes are ambiguous and return two classes; the
/// four that are not are `2 ≤ U < 3` by day (`B`), `5 ≤ U < 6` by night (`D`),
/// and `U ≥ 6` by day or night (`D`).
///
/// Representing that as an enum rather than a `Vec` matters: it is total (there
/// is no empty or three-element case to handle), it allocates nothing, and it
/// makes the ambiguity impossible to drop silently — which is exactly what
/// upstream's own `gpuff` does. See [`Self::primary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StabilitySet {
    /// The condition selects exactly one class.
    One(StabilityClass),
    /// The condition is ambiguous between two adjacent classes, in upstream's
    /// order (the more unstable of the pair first).
    Two(StabilityClass, StabilityClass),
}

impl StabilitySet {
    /// The first class, which is the one upstream's `gpuff` actually uses.
    ///
    /// `gpuff` calls `compute_sigma_vals(stab_class, total_dist)` and then
    /// indexes the resulting `2 × n` matrix with `sigma.vec[1]` and
    /// `sigma.vec[2]`. R matrices are column-major, so those are `sigma_y` and
    /// `sigma_z` **of the first class**, and the second is silently discarded.
    /// This accessor names that behaviour instead of leaving it implicit.
    #[must_use]
    pub fn primary(self) -> StabilityClass {
        match self {
            Self::One(c) | Self::Two(c, _) => c,
        }
    }

    /// The second class where the condition is ambiguous.
    #[must_use]
    pub fn secondary(self) -> Option<StabilityClass> {
        match self {
            Self::One(_) => None,
            Self::Two(_, c) => Some(c),
        }
    }

    /// How many classes this set carries — 1 or 2.
    ///
    /// Load-bearing for [`super::simulate`]: upstream builds its puff record
    /// with `data.frame(stab_class = as.character(stab_class), ...)`, and R
    /// recycles the length-1 columns against a length-2 `stab_class`, so an
    /// ambiguous condition emits **two** puff rows. See
    /// [`super::simulate::EmissionPolicy`].
    #[must_use]
    pub fn len(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Two(_, _) => 2,
        }
    }

    /// Always `false` — a set never has zero classes. Present because clippy
    /// asks for it alongside [`Self::len`].
    #[must_use]
    pub fn is_empty(self) -> bool {
        false
    }
}

/// Whether an hour of the day counts as daytime for the stability lookup.
///
/// Ports `is_day`. Upstream takes a timestamp and formats it with `"%H"`, then
/// tests `hour >= 7 & hour <= 18`; this takes the hour directly, since that is
/// the only thing the physics reads. The bounds are **inclusive at both ends**,
/// so 07:00 and 18:00 are both day and the daytime window is 12 hours long.
///
/// # Arguments
/// - `hour` — hour of day, 0–23 in local time.
///
/// # Note
/// This is a fixed clock window, not a solar calculation: it does not depend on
/// latitude, date or season. Upstream is a near-field oil-and-gas leak model
/// where that approximation is cheap and conventional. For a high-latitude or
/// seasonal application it is wrong, and a solar-zenith criterion should be
/// used instead — [`crate::flexpart`]'s upstream carries `zenithangle.f90` for
/// exactly this reason, though it is not yet ported.
#[must_use]
pub fn is_day(hour: u32) -> bool {
    (7..=18).contains(&hour)
}

/// Pasquill stability class(es) for a wind speed and hour of day.
///
/// Ports `get_stab_class`. The lookup, with upstream's own band edges:
///
/// | wind speed `U` (m/s) | day | night |
/// |---|---|---|
/// | `U < 2` | A, B | E, F |
/// | `2 ≤ U < 3` | B | E, F |
/// | `3 ≤ U < 5` | B, C | D, E |
/// | `5 ≤ U < 6` | C, D | D |
/// | `U ≥ 6` | D | D |
///
/// # Arguments
/// - `wind_speed` — scalar wind speed. `None` reproduces upstream's
///   missing-value path and yields neutral [`StabilityClass::D`]; upstream also
///   emits an R `warning()` there, which this port does not (a library that
///   prints is a library that cannot be used quietly — the `None` in the
///   argument is the signal).
/// - `hour` — hour of day, 0–23 local, passed to [`is_day`].
///
/// # Returns
/// One or two classes, per the table. See [`StabilitySet`] for why the
/// two-class case is not collapsed.
///
/// # Panics
/// Panics if `wind_speed` is negative, which is not a wind speed. Upstream
/// accepts it silently and lands in the `U ≥ 6` branch only for large
/// magnitudes — a negative speed falls in `U < 2` and is classified as calm,
/// which is a plausible-looking wrong answer rather than an error.
#[must_use]
pub fn stability_class(wind_speed: Option<Velocity>, hour: u32) -> StabilitySet {
    use StabilityClass::{A, B, C, D, E, F};

    let Some(u) = wind_speed else {
        // Upstream: warn and return neutral.
        return StabilitySet::One(D);
    };
    let u = u.get::<meter_per_second>();
    assert!(
        u >= 0.0,
        "wind speed must be non-negative; got {u} m/s (upstream would classify \
         this as calm rather than rejecting it)"
    );
    let day = is_day(hour);

    if u < 2.0 {
        if day {
            StabilitySet::Two(A, B)
        } else {
            StabilitySet::Two(E, F)
        }
    } else if u < 3.0 {
        if day {
            StabilitySet::One(B)
        } else {
            StabilitySet::Two(E, F)
        }
    } else if u < 5.0 {
        if day {
            StabilitySet::Two(B, C)
        } else {
            StabilitySet::Two(D, E)
        }
    } else if u < 6.0 {
        if day {
            StabilitySet::Two(C, D)
        } else {
            StabilitySet::One(D)
        }
    } else {
        StabilitySet::One(D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: f64) -> Option<Velocity> {
        Some(Velocity::new::<meter_per_second>(v))
    }

    #[test]
    fn daytime_window_is_inclusive_at_both_ends() {
        assert!(!is_day(6));
        assert!(is_day(7));
        assert!(is_day(18));
        assert!(!is_day(19));
        assert_eq!((0..24).filter(|h| is_day(*h)).count(), 12);
    }

    #[test]
    fn missing_wind_speed_gives_neutral() {
        assert_eq!(
            stability_class(None, 12),
            StabilitySet::One(StabilityClass::D)
        );
        assert_eq!(
            stability_class(None, 3),
            StabilitySet::One(StabilityClass::D)
        );
    }

    /// Six of the table's ten (wind speed x day/night) regimes return two
    /// classes. Pinned exactly, because the *significance* of the emission
    /// divergence in [`super::super::simulate::EmissionPolicy`] rests on
    /// ambiguity being the common case rather than an edge case.
    #[test]
    fn six_of_ten_regimes_are_ambiguous() {
        let regimes = [
            (1.0, 12),
            (2.5, 12),
            (4.0, 12),
            (5.5, 12),
            (10.0, 12),
            (1.0, 3),
            (2.5, 3),
            (4.0, 3),
            (5.5, 3),
            (10.0, 3),
        ];
        let ambiguous = regimes
            .iter()
            .filter(|(u, h)| stability_class(ms(*u), *h).len() == 2)
            .count();
        assert_eq!(
            ambiguous, 6,
            "the ambiguity is load-bearing for the emission-policy divergence"
        );
    }

    #[test]
    fn primary_is_the_more_unstable_of_an_ambiguous_pair() {
        let set = stability_class(ms(1.0), 12);
        assert_eq!(set, StabilitySet::Two(StabilityClass::A, StabilityClass::B));
        assert_eq!(set.primary(), StabilityClass::A);
        assert_eq!(set.secondary(), Some(StabilityClass::B));
    }

    #[test]
    #[should_panic(expected = "wind speed must be non-negative")]
    fn negative_wind_speed_is_rejected() {
        let _ = stability_class(ms(-1.0), 12);
    }
}
