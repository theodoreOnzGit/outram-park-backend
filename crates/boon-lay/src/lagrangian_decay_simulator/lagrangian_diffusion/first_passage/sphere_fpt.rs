//! First-passage statistics for a diffusing atom inside a sphere.
//!
//! Consider a point Brownian walker with diffusion coefficient `D` started at
//! the **centre** of an absorbing sphere of radius `R`. The time it takes to
//! first reach the surface is the *first-passage time* `tau`. Working in the
//! dimensionless time `theta = D t / R^2`, the probability that the walker is
//! **still inside** at time `t` (the survival probability) is the eigenfunction
//! series
//!
//! ```text
//! S(theta) = 2 * sum_{k>=1} (-1)^(k+1) * exp(-k^2 * pi^2 * theta),
//! ```
//!
//! obtained by solving the diffusion equation in a sphere with an absorbing
//! surface and a point source at the centre. Its mean is `E[tau] = R^2 / (6 D)`
//! (equivalently `E[theta] = 1/6`).
//!
//! The Walk-on-Spheres engine ([`super::walk_on_spheres`]) advances a walker one
//! interface-free sphere at a time. Each hop sphere is centred on the walker, so
//! the **centre-start** distribution above is exactly the per-hop exit-time law:
//! the exit point is uniform on the sphere (isotropy) and the elapsed time is a
//! draw from `S`.
//!
//! Because `theta = D t / R^2` is dimensionless, the distribution of `theta` is
//! **universal** — independent of `R` and `D`. We therefore build one inverse-CDF
//! table for `theta` once and reuse it for every hop of every walker, converting
//! to a physical time with `t = theta * R^2 / D`.

use std::f64::consts::PI;
use std::sync::OnceLock;

use outram_mc_libs::rng::distributions::isotropic_direction;
use outram_mc_libs::rng::lcg::prn;
use uom::si::f64::{Area, DiffusionCoefficient, Length, Time};

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
pub fn mean_first_passage_time(radius: Length, diffusion_coefficient: DiffusionCoefficient) -> Time {
    let r_squared: Area = radius * radius;
    r_squared / (6.0 * diffusion_coefficient)
}

/// Survival probability `S(theta)` that a centre-started walker is still inside
/// the sphere at dimensionless time `theta = D t / R^2`.
///
/// Evaluates `2 * sum_{k>=1} (-1)^(k+1) exp(-k^2 pi^2 theta)`. The terms are
/// positive, strictly decreasing in `k`, and alternate in sign, so the
/// truncation error is bounded by the first omitted term; the sum runs until
/// that term drops below `1e-15`. Returns `1.0` for `theta <= 0` and is clamped
/// to `[0, 1]`.
#[inline]
pub fn survival_probability(theta: f64) -> f64 {
    if theta <= 0.0 {
        return 1.0;
    }
    let mut sum = 0.0_f64;
    let mut sign = 1.0_f64;
    for k in 1..=2_000_000_u64 {
        let kf = k as f64;
        let term = (-kf * kf * PI * PI * theta).exp();
        sum += sign * term;
        sign = -sign;
        if term < 1e-15 {
            break;
        }
    }
    (2.0 * sum).clamp(0.0, 1.0)
}

/// Universal inverse-CDF table for the dimensionless exit time `theta`.
///
/// Stores `F = 1 - S(theta)` (the CDF, ascending in `theta`) alongside
/// `ln(theta)` on a log-spaced `theta` grid. Sampling inverts `F` by binary
/// search + linear interpolation in `(F, ln theta)`.
struct ExitTimeInverseCdf {
    /// CDF values `F = 1 - S(theta_i)`, ascending.
    cdf: Vec<f64>,
    /// `ln(theta_i)` aligned with `cdf`.
    ln_theta: Vec<f64>,
}

impl ExitTimeInverseCdf {
    /// Build the table. `theta` runs log-spaced over `[1e-5, 8]`: below `1e-5`
    /// the survival is `~1` (exit that fast is astronomically unlikely) and
    /// above `8` it is `~1e-34` (essentially certain to have exited), so the
    /// clamped tails carry negligible probability.
    fn build() -> Self {
        const M: usize = 6001;
        const THETA_MIN: f64 = 1e-5;
        const THETA_MAX: f64 = 8.0;
        let ln_min = THETA_MIN.ln();
        let ln_max = THETA_MAX.ln();

        let mut cdf = Vec::with_capacity(M);
        let mut ln_theta = Vec::with_capacity(M);
        for i in 0..M {
            let lt = ln_min + (ln_max - ln_min) * (i as f64) / ((M - 1) as f64);
            let theta = lt.exp();
            cdf.push(1.0 - survival_probability(theta));
            ln_theta.push(lt);
        }
        Self { cdf, ln_theta }
    }

    /// Invert the CDF: given `u` in `[0, 1)`, return the corresponding `theta`.
    #[inline]
    fn sample_theta(&self, u: f64) -> f64 {
        let first = self.cdf[0];
        let last = *self.cdf.last().unwrap();
        if u <= first {
            return self.ln_theta[0].exp();
        }
        if u >= last {
            return self.ln_theta[self.ln_theta.len() - 1].exp();
        }
        // First index whose CDF value is >= u (cdf is ascending).
        let hi = self.cdf.partition_point(|&x| x < u);
        let lo = hi - 1;
        let (f_lo, f_hi) = (self.cdf[lo], self.cdf[hi]);
        let (l_lo, l_hi) = (self.ln_theta[lo], self.ln_theta[hi]);
        let w = if f_hi > f_lo {
            (u - f_lo) / (f_hi - f_lo)
        } else {
            0.0
        };
        (l_lo + w * (l_hi - l_lo)).exp()
    }
}

/// The process-wide exit-time table, built once on first use. It is a pure
/// deterministic lookup table (a constant), so a `OnceLock` is appropriate — it
/// is not shared mutable simulation state.
fn exit_time_cdf() -> &'static ExitTimeInverseCdf {
    static EXIT_TIME_CDF: OnceLock<ExitTimeInverseCdf> = OnceLock::new();
    EXIT_TIME_CDF.get_or_init(ExitTimeInverseCdf::build)
}

/// Sample the dimensionless exit time `theta = D * tau / R^2` for one hop.
///
/// Draws from the universal centre-start first-passage distribution using the
/// inverse-CDF table. `seed` is the caller's LCG state (advanced by one draw).
#[inline]
pub fn sample_dimensionless_exit_time(seed: &mut u64) -> f64 {
    exit_time_cdf().sample_theta(prn(seed))
}

/// Sample the physical first-passage time for a hop of radius `radius` in a
/// medium with diffusion coefficient `diffusion_coefficient`.
///
/// Returns `tau = theta * R^2 / D` with `theta` drawn from the universal
/// distribution. `seed` is the caller's LCG state (advanced by one draw).
///
/// # Units
///
/// `radius` is a `Length`, `diffusion_coefficient` a `DiffusionCoefficient`
/// (m^2/s); the result is a `Time` (seconds).
#[inline]
pub fn sample_first_passage_time(
    seed: &mut u64,
    radius: Length,
    diffusion_coefficient: DiffusionCoefficient,
) -> Time {
    let theta = sample_dimensionless_exit_time(seed);
    let r_squared: Area = radius * radius;
    theta * r_squared / diffusion_coefficient
}

/// Sample a direction uniform on the unit sphere, as a dimensionless
/// `[x, y, z]` unit vector. Used for the exit point of each Walk-on-Spheres hop
/// (uniform by the isotropy of Brownian motion started at the sphere centre).
#[inline]
pub fn sample_uniform_direction(seed: &mut u64) -> [f64; 3] {
    let (u, v, w) = isotropic_direction(seed);
    [u, v, w]
}

/// Tabulate the dimensionless exit time `theta` on a **uniform** CDF grid:
/// entry `j` is `theta` at `F = j / (m - 1)` for `j = 0..m`.
///
/// Unlike the internal inverse-CDF table (which is queried by binary search),
/// this is a directly-indexable lookup — `theta(u) ~ table[u*(m-1)]` with linear
/// interpolation — so it can be uploaded to a GPU and sampled without a search.
/// `m` must be at least 2.
pub fn dimensionless_exit_time_table(m: usize) -> Vec<f64> {
    assert!(m >= 2, "table needs at least two points");
    let cdf = exit_time_cdf();
    (0..m)
        .map(|j| {
            let f = j as f64 / (m - 1) as f64;
            cdf.sample_theta(f.min(0.999_999))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::diffusion_coefficient::square_meter_per_second;
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
        assert!(tau.get::<second>() < 1.0);
    }

    #[test]
    fn survival_endpoints_and_monotonicity() {
        assert_relative_eq!(survival_probability(0.0), 1.0);
        // Deep tail: essentially certain to have exited.
        assert!(survival_probability(8.0) < 1e-30);
        // Monotonically decreasing.
        let mut prev = 1.0;
        for i in 1..=200 {
            let theta = i as f64 * 0.02;
            let s = survival_probability(theta);
            assert!(s <= prev + 1e-12, "S not monotone at theta={theta}");
            prev = s;
        }
    }

    #[test]
    fn sampled_dimensionless_mean_is_one_sixth() {
        // E[theta] = 1/6 for the centre-start sphere exit time.
        let mut seed = 0x1234_5678_9abc_def0_u64;
        let n = 400_000;
        let mean = (0..n)
            .map(|_| sample_dimensionless_exit_time(&mut seed))
            .sum::<f64>()
            / n as f64;
        assert_relative_eq!(mean, 1.0 / 6.0, max_relative = 0.01);
    }

    #[test]
    fn sampled_physical_fpt_mean_matches_analytic() {
        // Ensemble mean of sampled tau should match R^2/(6D).
        let r = Length::new::<micrometer>(50.0);
        let d = DiffusionCoefficient::new::<square_meter_per_second>(6.3e-8);
        let analytic = mean_first_passage_time(r, d).get::<second>();
        let mut seed = 0xdead_beef_cafe_babe_u64;
        let n = 200_000;
        let mean = (0..n)
            .map(|_| sample_first_passage_time(&mut seed, r, d).get::<second>())
            .sum::<f64>()
            / n as f64;
        assert_relative_eq!(mean, analytic, max_relative = 0.02);
    }
}
