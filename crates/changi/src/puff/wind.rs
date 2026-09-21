// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/helpers.R` — `wind_vector_convert`,
// `interpolate_wind_data`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! Wind handling: meteorological convention to vector components, and
//! resampling a wind series onto the simulation time step.

use uom::si::angle::degree;
use uom::si::f64::{Angle, Time, Velocity};
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Wind as vector components in the site's local Cartesian frame.
///
/// `u` is the eastward component and `v` the northward one — i.e. the
/// direction the air is *going*, not the direction it comes *from*. That
/// inversion is the whole content of [`wind_vector_convert`], and it is the
/// single most common sign error in dispersion code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindComponents {
    /// Eastward (`+x`) component.
    pub u: Velocity,
    /// Northward (`+y`) component.
    pub v: Velocity,
}

/// Convert meteorological wind speed and direction to `(u, v)` components.
///
/// Ports `wind_vector_convert`. The meteorological convention names the
/// direction the wind blows **from**, measured clockwise from north: `0°` is a
/// northerly (air moving south), `90°` easterly, `180°` southerly, `270°`
/// westerly. The mathematical convention wanted by the advection step measures
/// anticlockwise from east and names where the air is **going**. Upstream's
/// one-line reconciliation is
///
/// ```text
///   theta = (270 - direction) * pi / 180
///   u = speed * cos(theta)
///   v = speed * sin(theta)
/// ```
///
/// # Arguments
/// - `speed` — scalar wind speed.
/// - `direction` — meteorological wind direction (blowing *from*).
///
/// # Returns
/// Components in the local Cartesian frame, in the direction the air travels.
///
/// # Note on exactness
/// The cardinal directions do not come out exactly zero: `270 - 0 = 270°` in
/// radians is not exactly `3*pi/2` in binary, so a due-northerly gives
/// `u = -9.18e-16 * speed` rather than `0`. Upstream has the same residue, to
/// the bit, because the arithmetic is identical. It is left alone rather than
/// snapped to zero — rounding it would be a silent divergence from upstream for
/// no physical gain, and `1e-16 m/s` advects a puff by `1e-13 m` over a day.
#[must_use]
pub fn wind_vector_convert(speed: Velocity, direction: Angle) -> WindComponents {
    // Upstream writes `theta <- (270 - wind_directions) * pi / 180`: multiply
    // by pi, THEN divide by 180. Going through `uom`'s degree-to-radian
    // conversion instead would multiply by a single pre-rounded `pi/180`
    // constant, and the two round differently -- measured at up to 1.3e-12
    // relative on the fixture, which is 4 000x above f64 epsilon and easily
    // visible in a code-to-code comparison. The arithmetic is therefore done
    // in the same order upstream does it.
    let theta_rad = (270.0 - direction.get::<degree>()) * core::f64::consts::PI / 180.0;
    WindComponents {
        u: speed * theta_rad.cos(),
        v: speed * theta_rad.sin(),
    }
}

/// Resample a wind series onto a regular simulation time step.
///
/// Ports `interpolate_wind_data`. Upstream converts speed/direction to `(u, v)`
/// **first** and interpolates the components, not the polar coordinates. That
/// ordering is deliberate and worth preserving: interpolating a direction
/// across the 360°/0° wrap would sweep the long way round, and interpolating a
/// speed through a calm would miss the reversal. Interpolating `u` and `v`
/// does both correctly.
///
/// The observation times are assumed **evenly spaced across the whole
/// simulation window** — upstream builds them with
/// `seq(sim_start, sim_end, length.out = length(wind_speeds))`, so the input
/// series is stretched to fit the window regardless of what its real sampling
/// interval was. A caller whose observations do not span exactly
/// `[sim_start, sim_end]` will get a silently time-shifted series. This port
/// keeps that behaviour and names it here.
///
/// # Arguments
/// - `components` — the observed wind, already in `(u, v)` form. Callers with
///   speed/direction data should map [`wind_vector_convert`] over it first,
///   which is what upstream does internally.
/// - `duration` — the simulation window length, `sim_end - sim_start`.
/// - `step` — the output interval (upstream's `puff_dt`).
///
/// # Returns
/// One [`WindComponents`] per output step, at times
/// `0, step, 2*step, …` up to and including `duration` if it falls on a step.
/// Matches R's `seq(from, to, by = step)`, which stops at or before `to`.
///
/// # Panics
/// Panics if `components` is empty, if `step` is not strictly positive, or if
/// `duration` is negative. R's `approx` errors on fewer than two points and
/// `seq` errors on a non-positive `by`; these are the same conditions, checked
/// up front.
#[must_use]
pub fn interpolate_wind(
    components: &[WindComponents],
    duration: Time,
    step: Time,
) -> Vec<WindComponents> {
    assert!(!components.is_empty(), "need at least one wind observation");
    let step_s = step.get::<second>();
    let duration_s = duration.get::<second>();
    assert!(step_s > 0.0, "step must be positive; got {step_s} s");
    assert!(
        duration_s >= 0.0,
        "duration must be non-negative; got {duration_s} s"
    );

    // Upstream: obs_times = seq(start, end, length.out = n). With n == 1 that
    // degenerates to the single start time and R's `approx` would error; this
    // port treats one observation as a constant wind, which is what a caller
    // supplying one value means.
    let n = components.len();
    let n_steps = (duration_s / step_s).floor() as usize + 1;
    let mut out = Vec::with_capacity(n_steps);

    for k in 0..n_steps {
        let t = (k as f64) * step_s;
        let c = if n == 1 {
            components[0]
        } else {
            // Position along the observation index axis.
            let spacing = duration_s / ((n - 1) as f64);
            let pos = if spacing > 0.0 { t / spacing } else { 0.0 };
            let i = (pos.floor() as usize).min(n - 2);
            let frac = pos - (i as f64);
            let (a, b) = (components[i], components[i + 1]);
            // R's `approx1` (src/library/stats/src/approx.c) short-circuits an
            // EXACT node hit -- `if(x[i] == v) return y[i];` -- before it ever
            // evaluates the linear formula. That is not a micro-optimisation:
            // the formula `a + (b - a) * frac` at `frac == 1` cancels
            // catastrophically when `|b| << |a|`, and returns `0` instead of
            // `b`. Measured on the fixture: observations of 2 m/s from the east
            // then 3 m/s from the south give `b.u = 1.8369701987210297e-16`,
            // and the linear form yields exactly `0`. Reproducing the
            // short-circuit is what makes the two agree.
            if frac == 0.0 {
                a
            } else if frac == 1.0 {
                b
            } else {
                WindComponents {
                    u: a.u + (b.u - a.u) * frac,
                    v: a.v + (b.v - a.v) * frac,
                }
            }
        };
        out.push(c);
    }
    out
}

/// Scalar wind speed from components, `sqrt(u^2 + v^2)`.
///
/// Upstream computes this inline in both simulate drivers. It is named here
/// because the stability lookup and the puff advection both need it and must
/// agree on it.
#[must_use]
pub fn wind_speed(c: WindComponents) -> Velocity {
    let u = c.u.get::<meter_per_second>();
    let v = c.v.get::<meter_per_second>();
    Velocity::new::<meter_per_second>(u.hypot(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: f64) -> Velocity {
        Velocity::new::<meter_per_second>(v)
    }
    fn deg(v: f64) -> Angle {
        Angle::new::<degree>(v)
    }
    fn s(v: f64) -> Time {
        Time::new::<second>(v)
    }

    /// A northerly (from the north) must push air **southward**: `v < 0`.
    /// This is the sign convention the whole advection depends on.
    #[test]
    fn a_northerly_pushes_air_south() {
        let c = wind_vector_convert(ms(5.0), deg(0.0));
        assert!(c.v.get::<meter_per_second>() < -4.9);
        assert!(c.u.get::<meter_per_second>().abs() < 1e-14);
    }

    /// An easterly (from the east) must push air **westward**: `u < 0`.
    #[test]
    fn an_easterly_pushes_air_west() {
        let c = wind_vector_convert(ms(10.0), deg(90.0));
        assert!(c.u.get::<meter_per_second>() < -9.9);
        assert!(c.v.get::<meter_per_second>().abs() < 1e-14);
    }

    #[test]
    fn speed_is_recovered_from_components() {
        for d in [0.0, 37.0, 90.0, 180.0, 271.5, 359.0] {
            let c = wind_vector_convert(ms(7.5), deg(d));
            assert!((wind_speed(c).get::<meter_per_second>() - 7.5).abs() < 1e-12);
        }
    }

    #[test]
    fn interpolation_hits_the_observations_it_passes_through() {
        let obs = vec![
            WindComponents {
                u: ms(0.0),
                v: ms(0.0),
            },
            WindComponents {
                u: ms(10.0),
                v: ms(-4.0),
            },
        ];
        let out = interpolate_wind(&obs, s(100.0), s(50.0));
        assert_eq!(out.len(), 3);
        assert!((out[0].u.get::<meter_per_second>() - 0.0).abs() < 1e-12);
        assert!((out[1].u.get::<meter_per_second>() - 5.0).abs() < 1e-12);
        assert!((out[2].u.get::<meter_per_second>() - 10.0).abs() < 1e-12);
        assert!((out[1].v.get::<meter_per_second>() + 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_single_observation_is_a_constant_wind() {
        let obs = vec![WindComponents {
            u: ms(3.0),
            v: ms(4.0),
        }];
        let out = interpolate_wind(&obs, s(30.0), s(10.0));
        assert_eq!(out.len(), 4);
        assert!(out
            .iter()
            .all(|c| (wind_speed(*c).get::<meter_per_second>() - 5.0).abs() < 1e-12));
    }
}
