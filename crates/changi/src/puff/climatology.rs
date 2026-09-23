// SPDX-License-Identifier: GPL-3.0

//! Illustrative wind conditions for examples and tests — Singapore.
//!
//! # What this is, and what it is NOT
//!
//! These are **illustrative climatological conditions for demonstrations**,
//! chosen so that the crate's examples run somewhere real rather than at an
//! arbitrary round number. CHANGI is named for Changi, and the repository
//! owner's local climate is the natural default.
//!
//! **This is not a site characterisation and not a design basis.** A real
//! assessment needs the site's own measured wind rose at the release height,
//! over a defined averaging period, with a stability joint-frequency
//! distribution — not four representative points read off a national
//! climatology. Nothing here may be used for emergency planning, emergency
//! response, dose assessment for real populations, Level 3 PSA, or any
//! safety-critical or licensing decision; the crate-level scope limits are
//! binding and this module does not relax them.
//!
//! # Why the numbers matter more than they look
//!
//! Singapore's mean surface wind is about **2 m/s** — roughly half the 4 m/s
//! the crate's examples used before. That is not merely "a bit calmer": 2 m/s
//! sits **exactly on a band edge** of the Pasquill lookup in
//! [`super::stability::stability_class`], which switches at 2, 3, 5 and 6 m/s.
//!
//! The consequences are sharp, and [`mod@tests`] pins both:
//!
//! * **By day, the class is knife-edge.** Just below 2 m/s the table returns
//!   the ambiguous pair `A/B`; at exactly 2 m/s and just above it returns a
//!   single `B`. A measurement uncertainty of a few cm/s straddles that.
//! * **By night, the class is ambiguous across the whole range Singapore
//!   normally sees.** Everything below 5 m/s at night returns two classes, so
//!   upstream `puff`'s mass-doubling defect — see
//!   [`super::simulate::EmissionPolicy`] — is **live in essentially every
//!   Singapore night-time condition**, not in some corner case.
//!
//! Light winds are also where a Gaussian puff model is weakest: the
//! Pasquill–Gifford fits come from tracer campaigns in steadier flow, and at
//! 1–2 m/s the wind direction wanders enough over a puff's lifetime that
//! holding it fixed from the moment of emission — which is exactly what
//! [`super::simulate`] does, following upstream — is a real approximation and
//! not a small one.
//!
//! # Provenance
//!
//! Figures attributed to the **Meteorological Service Singapore (MSS)**,
//! *Climate of Singapore*, <https://www.weather.gov.sg/climate-climate-of-singapore/>.
//!
//! **Accessed 2026-09-23 via web-search summaries of that page, NOT by
//! retrieving it directly** — `weather.gov.sg` and `nea.gov.sg` are both
//! blocked by this development environment's network egress policy. The
//! figures below are therefore recorded as **`Not re-checked against the
//! primary source`**, per the workspace rule that an unverifiable claim is
//! marked rather than left standing. They are round representative values in
//! any case, not precise climatological statistics, and they are used only to
//! make a demonstration concrete.
//!
//! Full record, with the corroborating sources: `docs/References.md`.

use uom::si::angle::degree;
use uom::si::f64::{Angle, Velocity};
use uom::si::velocity::meter_per_second;

use super::stability::{stability_class, StabilitySet};
use super::wind::{wind_vector_convert, WindComponents};

/// Singapore's mean surface wind speed, m/s.
///
/// About 2 m/s (roughly 4 knots) — light, and low enough that it sits on the
/// `U = 2 m/s` Pasquill band edge. See the module documentation for why that
/// matters.
///
/// Source: MSS, *Climate of Singapore*. Not re-checked against the primary
/// source — see the module's Provenance section.
pub const MEAN_WIND_SPEED_M_PER_S: f64 = 2.0;

/// A representative Singapore wind condition.
///
/// Dispatched as an enum rather than a trait object, per the workspace Rust
/// design rules: the set of monsoon regimes is closed and known at compile
/// time, so adding one forces every `match` to handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SingaporeWind {
    /// **Northeast Monsoon**, December to early March. Winds from the
    /// northerly-to-northeasterly sector; the windiest season, strongest in
    /// January and February.
    NortheastMonsoon,
    /// **Northeast Monsoon surge** — an episode within the NE monsoon when
    /// mean speeds reach 10 m/s or more. Included because it is the only
    /// common Singapore condition that clears the `U >= 6 m/s` band, where the
    /// Pasquill class is an unambiguous `D` day or night.
    NortheastMonsoonSurge,
    /// **Southwest Monsoon**, June to September. Winds from the
    /// southeasterly-to-southerly sector.
    SouthwestMonsoon,
    /// **Inter-monsoon**, April–May and October–November. Light and variable;
    /// the direction here is nominal, and a real study would not treat an
    /// inter-monsoon direction as persistent.
    InterMonsoon,
}

impl SingaporeWind {
    /// Every condition, for sweeping in an example or a test.
    pub const ALL: [Self; 4] = [
        Self::NortheastMonsoon,
        Self::NortheastMonsoonSurge,
        Self::SouthwestMonsoon,
        Self::InterMonsoon,
    ];

    /// A short human label, e.g. `"NE monsoon"`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::NortheastMonsoon => "NE monsoon",
            Self::NortheastMonsoonSurge => "NE monsoon surge",
            Self::SouthwestMonsoon => "SW monsoon",
            Self::InterMonsoon => "inter-monsoon",
        }
    }

    /// The months this condition covers, as prose.
    #[must_use]
    pub fn season(self) -> &'static str {
        match self {
            Self::NortheastMonsoon => "December to early March",
            Self::NortheastMonsoonSurge => "episodes within December to early March",
            Self::SouthwestMonsoon => "June to September",
            Self::InterMonsoon => "April-May and October-November",
        }
    }

    /// Representative scalar wind speed.
    ///
    /// Round representative values, not climatological statistics:
    /// `3 m/s` for the NE monsoon (the windiest season, above the ~2 m/s
    /// annual mean), `10 m/s` for a surge, `2 m/s` for the SW monsoon (at the
    /// annual mean), and `1 m/s` for the light and variable inter-monsoon.
    #[must_use]
    pub fn speed(self) -> Velocity {
        let v = match self {
            Self::NortheastMonsoon => 3.0,
            Self::NortheastMonsoonSurge => 10.0,
            Self::SouthwestMonsoon => MEAN_WIND_SPEED_M_PER_S,
            Self::InterMonsoon => 1.0,
        };
        Velocity::new::<meter_per_second>(v)
    }

    /// Prevailing wind direction in the **meteorological convention** — the
    /// direction the wind blows *from*, degrees clockwise from north.
    ///
    /// `030°` for the northeasterlies and `160°` for the south-southeasterlies
    /// are mid-sector choices, not measured modes. The inter-monsoon value is
    /// nominal: those winds are by definition variable, and a single direction
    /// misrepresents them.
    #[must_use]
    pub fn direction(self) -> Angle {
        let d = match self {
            Self::NortheastMonsoon | Self::NortheastMonsoonSurge => 30.0,
            Self::SouthwestMonsoon => 160.0,
            Self::InterMonsoon => 90.0,
        };
        Angle::new::<degree>(d)
    }

    /// The wind as `(u, v)` components in the site's local Cartesian frame.
    ///
    /// Convenience over [`wind_vector_convert`], so an example need not repeat
    /// the from/towards inversion that is the commonest sign error in
    /// dispersion code.
    #[must_use]
    pub fn components(self) -> WindComponents {
        wind_vector_convert(self.speed(), self.direction())
    }

    /// The Pasquill class(es) this condition selects at a given hour.
    ///
    /// Provided so a caller cannot accidentally pair a condition with a class
    /// derived from some other wind speed.
    ///
    /// # Arguments
    /// - `hour` — hour of day, 0–23 local.
    #[must_use]
    pub fn stability(self, hour: u32) -> StabilitySet {
        stability_class(Some(self.speed()), hour)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puff::stability::StabilityClass;
    use uom::si::velocity::meter_per_second as mps;

    /// Every condition round-trips its speed through the component conversion.
    #[test]
    fn components_preserve_the_scalar_speed() {
        for c in SingaporeWind::ALL {
            let got = crate::puff::wind::wind_speed(c.components()).get::<mps>();
            let want = c.speed().get::<mps>();
            assert!((got - want).abs() < 1e-12, "{}", c.label());
        }
    }

    /// A northeasterly must carry a plume toward the **south-west**: both
    /// components negative. This is the sign convention the whole advection
    /// rests on, and getting it backwards would put the plume over the wrong
    /// half of the island.
    #[test]
    fn the_northeast_monsoon_blows_toward_the_southwest() {
        let c = SingaporeWind::NortheastMonsoon.components();
        assert!(c.u.get::<mps>() < 0.0, "u should be westward");
        assert!(c.v.get::<mps>() < 0.0, "v should be southward");
    }

    /// A south-southeasterly must carry a plume toward the **north-north-west**.
    #[test]
    fn the_southwest_monsoon_blows_toward_the_northwest() {
        let c = SingaporeWind::SouthwestMonsoon.components();
        assert!(c.u.get::<mps>() < 0.0, "u should be westward");
        assert!(c.v.get::<mps>() > 0.0, "v should be northward");
    }

    /// **The mean wind sits exactly on a Pasquill band edge.**
    ///
    /// `stability_class` switches at 2 m/s. Just below, a daytime condition is
    /// the ambiguous pair `A/B`; at exactly 2 m/s it is a single `B`. Pinned
    /// because it is the reason the module documentation warns that the
    /// daytime class is knife-edge at Singapore's mean, and because a change to
    /// the band edges would otherwise silently invalidate that warning.
    #[test]
    fn the_mean_wind_sits_on_a_pasquill_band_edge() {
        let just_below = Velocity::new::<mps>(MEAN_WIND_SPEED_M_PER_S - 1e-6);
        let exactly = Velocity::new::<mps>(MEAN_WIND_SPEED_M_PER_S);

        assert_eq!(
            stability_class(Some(just_below), 14),
            StabilitySet::Two(StabilityClass::A, StabilityClass::B),
            "just below the mean, daytime is ambiguous A/B"
        );
        assert_eq!(
            stability_class(Some(exactly), 14),
            StabilitySet::One(StabilityClass::B),
            "at the mean, daytime is an unambiguous B — the class changes \
             across a 1e-6 m/s step"
        );
    }

    /// **At night, every Singapore condition below a surge is ambiguous.**
    ///
    /// So upstream `puff`'s mass-doubling defect is live in essentially every
    /// Singapore night-time condition rather than in a corner case — which is
    /// the concrete reason this port's default emission policy matters here.
    #[test]
    fn night_time_singapore_conditions_are_ambiguous_except_in_a_surge() {
        for c in SingaporeWind::ALL {
            let set = c.stability(2); // 02:00, night
            let ambiguous = set.len() == 2;
            match c {
                SingaporeWind::NortheastMonsoonSurge => assert!(
                    !ambiguous,
                    "a surge clears U >= 6 m/s, so the night class is a single D"
                ),
                _ => assert!(
                    ambiguous,
                    "{} at night should be ambiguous, giving upstream's mass \
                     doubling; got {set:?}",
                    c.label()
                ),
            }
        }
    }
}
