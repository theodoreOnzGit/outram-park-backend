//! First-passage statistics for a diffusing atom inside a sphere.
//!
//! Consider a point Brownian walker with diffusion coefficient `D` started at
//! the **centre** of an absorbing sphere of radius `R`. The time it takes to
//! first reach the surface is the *first-passage time* `tau`. Its probability
//! density is known in closed form (a theta-function series), and its mean is
//!
//! ```text
//! E[tau] = R^2 / (6 D).
//! ```
//!
//! The Walk-on-Spheres engine ([`super::walk_on_spheres`]) uses this to advance
//! a walker one interface-free sphere at a time: the exit point is uniform on
//! the sphere (isotropy of Brownian motion from the centre) and the elapsed
//! time is a draw from this first-passage distribution.
//!
//! This Phase-0 scaffold provides the analytic **mean** exit time, which is
//! both a building block and the reference the stochastic exit-time sampler
//! (added in the CPU engine) is verified against. The mean is exact — no series
//! truncation is involved.

use uom::si::f64::{Area, DiffusionCoefficient, Time};

/// Mean first-passage time for a walker started at the centre of a sphere.
///
/// Returns `E[tau] = R^2 / (6 D)`, the expected time for 3-D Brownian motion
/// with diffusion coefficient `diffusion_coefficient` to first reach the
/// surface of a sphere of radius `radius`, starting from its centre.
///
/// # Units
///
/// - `radius` — a [`uom`] `Length` (any unit; metres internally).
/// - `diffusion_coefficient` — a [`uom`] `DiffusionCoefficient` (m^2/s).
/// - returns a [`uom`] `Time` (seconds).
///
/// # Valid range
///
/// `radius > 0` and `diffusion_coefficient > 0`. The formula is exact for any
/// positive inputs; it carries no approximation.
#[inline]
pub fn mean_first_passage_time(
    radius: uom::si::f64::Length,
    diffusion_coefficient: DiffusionCoefficient,
) -> Time {
    let r_squared: Area = radius * radius;
    r_squared / (6.0 * diffusion_coefficient)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::f64::Length;
    use uom::si::length::micrometer;
    use uom::si::time::second;

    #[test]
    fn mean_fpt_matches_r2_over_6d() {
        // Buffer-layer numbers from docs/buffer_clt_failure_analysis.md:
        // R = 100 um, D = 1e-8 m^2/s  =>  E[tau] = (1e-4)^2 / (6*1e-8) s.
        let r = Length::new::<micrometer>(100.0);
        let d = DiffusionCoefficient::new::<square_meter_per_second>(1e-8);
        let tau = mean_first_passage_time(r, d);
        let expected = (1e-4_f64).powi(2) / (6.0 * 1e-8);
        assert_relative_eq!(tau.get::<second>(), expected, max_relative = 1e-12);
        // Sanity: sub-second crossing of the buffer, as claimed in the doc.
        assert!(tau.get::<second>() < 1.0);
    }
}
