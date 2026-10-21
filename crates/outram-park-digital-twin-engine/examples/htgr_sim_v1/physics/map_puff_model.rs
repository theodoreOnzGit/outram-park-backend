//! **The puff-model configuration of the whole Map tab** -- one source of
//! truth for every basis (Absolute, Per Ci, `chi/Q`, Dose rate) and every
//! table (maintainer direction, 2026-09-29).
//!
//! # What the maintainer asked for, in their words (2026-09-29)
//!
//! - *"limit wind speed to one regime … I don't want to change wind direction
//!   and suddenly the puff model changes on the map, looks incorrect."*
//! - *"let's do the inter-monsoon regime so we can get highest reading, for
//!   this, limit the upper limit of the slider to 5 m/s so we don't change
//!   regime."*
//! - *"this is just for the map, not changi itself"* and *"that is to be the
//!   puff model used for the map"*.
//!
//! So, **by maintainer decision** (the engine `CLAUDE.md`'s first permitted
//! reason for a hardcoded value):
//!
//! | Setting | Value | Where it comes from |
//! |---|---|---|
//! | regime | [`MAP_REGIME`] = `changi::puff::climatology::SingaporeWind::InterMonsoon` | maintainer's choice |
//! | stability class | [`MAP_CLASS`] = **B**, **fixed** | one of changi's two classes for `MAP_REGIME.stability(MAP_HOUR)` (A/B by day), chosen because it gives the highest reading (measured, see below) |
//! | hour | [`MAP_HOUR`] = 12 (day) | see its doc for why day |
//! | default speed | `MAP_REGIME.speed()` = 1 m/s | changi's representative inter-monsoon speed |
//! | speed slider | [`MIN_SPEED_M_PER_S`]..=[`MAX_SPEED_M_PER_S`] = 0.5..=5 m/s | maintainer's cap |
//! | direction | operator's, **rotation only** | see below |
//!
//! # Speed scales dilution only; the class does not move
//!
//! changi's Pasquill lookup (`stability_class`) switches class at 2, 3 and
//! 5 m/s, so deriving the class from the slider speed would still change the
//! regime inside 0.5-5 m/s. The map therefore passes
//! `StabilitySource::Fixed(map_stability_class())` and the speed only enters
//! the puff kernel's `1/u` dilution and the puffs' advection.
//! `tests::sliding_the_speed_never_changes_the_class` pins that.
//!
//! # Direction rotates the plume, nothing else
//!
//! The map's field is a marched Lagrangian puff population (gh:#344), and
//! under a genuine wind change the old puffs keep their old heading, so the
//! plume kinks for one puff lifetime. That is correct for a real wind shift
//! and is what the maintainer found looks incorrect on this map. For the map
//! the direction control is therefore a **rotation**: `HtgrPlant` rotates the
//! existing population about the stack by the change in bearing
//! (`AtmosphericDispersionChannel::rotate_population_to`) before applying the
//! new wind, so the plume is the one a steady wind from the new bearing would
//! have built. `atmospheric_dispersion::tests::rotating_the_direction_only_rotates_the_plume`
//! pins it. The channel's own `set_meteorology` keeps the Lagrangian
//! behaviour; the rotation is the map's policy, not a change to changi or to
//! the channel's physics.
//!
//! # Limitation -- gh:#384
//!
//! A single fixed regime with a held class is a **screening configuration**,
//! not a meteorology: no diurnal cycle, no class change with speed, no
//! direction history. The Gaussian-puff limitations it inherits are tracked in
//! **GitHub issue #384**. Research and education only.
//!
//! # Is inter-monsoon the highest reading? Measured, not assumed
//!
//! `atmospheric_dispersion::tests::peak_ring_dose_rate_by_regime` sweeps every
//! regime and class; the table and the verdict are recorded in its doc comment
//! and in `reference/References.md`.

use changi::activity::chi_over_q::StabilitySource;
use changi::puff::climatology::SingaporeWind;
use changi::puff::stability::StabilityClass;
use uom::si::angle::degree;
use uom::si::f64::{Angle, Velocity};
use uom::si::velocity::meter_per_second;

use super::atmospheric_dispersion::Meteorology;

/// The map's regime: **inter-monsoon**. Maintainer decision, 2026-09-29
/// ("let's do the inter-monsoon regime so we can get highest reading"). Do
/// not "fix" it back to a speed-derived regime.
pub const MAP_REGIME: SingaporeWind = SingaporeWind::InterMonsoon;

/// The hour the class is read at: **12, day**. The map has always run at
/// midday (`Meteorology::default().hour`), and by day the inter-monsoon's
/// 1 m/s gives changi's ambiguous pair `A/B`. Midday also holds the highest
/// reading in the regime table (night E is 4 % lower on the ring, F 5x).
pub const MAP_HOUR: u32 = 12;

/// The fixed Pasquill class: **B**.
///
/// changi's lookup returns the pair `A/B` for the inter-monsoon by day, and
/// the channel's old habit was the primary, `A`. The maintainer chose the
/// regime "so we can get highest reading", and the regime sweep
/// (`atmospheric_dispersion::tests::peak_ring_dose_rate_by_regime`, measured
/// 2026-09-29, 1200 K kernel) shows **B** is the higher of the two: peak ring
/// air dose rate 1.625e-7 against A's 9.587e-8 µSv/h (field peak 2.269e-7
/// against 2.178e-7). So B, following the maintainer's stated aim; the test
/// fails if B stops being the maximum or stops being one of changi's classes
/// for this regime and hour. Class A is the alternative to raise with the
/// maintainer if the goal changes.
pub const MAP_CLASS: StabilityClass = StabilityClass::B;

/// Slider floor \[m/s\]: a Gaussian puff has nothing to advect in a calm.
/// Unchanged from the previous slider.
pub const MIN_SPEED_M_PER_S: f64 = 0.5;

/// Slider cap \[m/s\]: **5 m/s**, maintainer decision 2026-09-29 ("limit the
/// upper limit of the slider to 5 m/s so we don't change regime").
pub const MAX_SPEED_M_PER_S: f64 = 5.0;

/// The fixed stability class, [`MAP_CLASS`].
///
/// # Panics
/// If [`MAP_CLASS`] is not one of the classes changi's lookup gives for
/// [`MAP_REGIME`] at [`MAP_HOUR`] -- the map must never run a class its own
/// regime cannot produce.
pub fn map_stability_class() -> StabilityClass {
    let set = MAP_REGIME.stability(MAP_HOUR);
    assert!(
        set.primary() == MAP_CLASS || set.secondary() == Some(MAP_CLASS),
        "MAP_CLASS {MAP_CLASS:?} is not a class of {:?} at hour {MAP_HOUR}",
        MAP_REGIME
    );
    MAP_CLASS
}

/// The regime's representative speed \[m/s\] (1 m/s), the slider default.
pub fn default_speed_m_per_s() -> f64 {
    MAP_REGIME.speed().get::<meter_per_second>()
}

/// **The** meteorology every map basis and table is computed with: the
/// operator's speed (clamped to the slider range) and direction, the fixed
/// hour and the fixed class. `plant_commands_from` builds the plant's
/// dispersion command through this and nothing else.
pub fn map_meteorology(speed_m_per_s: f64, wind_from_deg: f64) -> Meteorology {
    let speed = if speed_m_per_s.is_finite() {
        speed_m_per_s.clamp(MIN_SPEED_M_PER_S, MAX_SPEED_M_PER_S)
    } else {
        default_speed_m_per_s()
    };
    Meteorology {
        speed: Velocity::new::<meter_per_second>(speed),
        direction_from: Angle::new::<degree>(wind_from_deg),
        hour: MAP_HOUR,
        stability: StabilitySource::Fixed(map_stability_class()),
    }
}

/// The on-screen label, e.g. `"fixed regime: inter-monsoon, class A (day) --
/// maintainer's choice, gh:#384"`.
pub fn regime_label() -> String {
    format!(
        "Map puff model: fixed regime {} ({}), Pasquill class {} held fixed (day), speed \
         {MIN_SPEED_M_PER_S}-{MAX_SPEED_M_PER_S} m/s scales dilution only, direction only \
         rotates the plume -- maintainer's choice 2026-09-29 (limitations: gh:#384).",
        MAP_REGIME.label(),
        MAP_REGIME.season(),
        map_stability_class().letter()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Sliding the speed across 0.5-5 m/s never changes the class.** The
    /// speed-derived lookup would switch at 2, 3 and 5 m/s; the map's class is
    /// fixed, and the fixed class is the inter-monsoon's daytime primary.
    #[test]
    fn sliding_the_speed_never_changes_the_class() {
        assert_eq!(map_stability_class(), StabilityClass::B);
        let mut u = MIN_SPEED_M_PER_S;
        while u <= MAX_SPEED_M_PER_S + 1e-12 {
            let m = map_meteorology(u, 37.0);
            assert_eq!(
                m.stability,
                StabilitySource::Fixed(StabilityClass::B),
                "u = {u}"
            );
            assert_eq!(m.hour, MAP_HOUR);
            u += 0.05;
        }
        // Out-of-range commands are clamped into the slider range, never
        // allowed to leave the regime.
        let fast = map_meteorology(12.0, 0.0);
        assert_eq!(fast.speed.get::<meter_per_second>(), MAX_SPEED_M_PER_S);
        let calm = map_meteorology(0.0, 0.0);
        assert_eq!(calm.speed.get::<meter_per_second>(), MIN_SPEED_M_PER_S);
        assert_eq!(
            map_meteorology(f64::NAN, 0.0)
                .speed
                .get::<meter_per_second>(),
            1.0
        );
    }

    /// The regime is the maintainer's inter-monsoon, 1 m/s, and the cap is 5.
    #[test]
    fn the_regime_is_the_maintainers() {
        assert_eq!(MAP_REGIME, SingaporeWind::InterMonsoon);
        assert_eq!(default_speed_m_per_s(), 1.0);
        assert_eq!(MAX_SPEED_M_PER_S, 5.0);
        assert!(regime_label().contains("inter-monsoon") && regime_label().contains("#384"));
    }
}
