//! Single-phase wall friction: the RELAP5 four-region Darcy friction factor
//! (gh:#1061, RELAP-7 parity epic gh:#1056). AI draft, clean room: written
//! from the RELAP-7 Theory Manual's description (INL/EXT-14-31366, §2.2.1,
//! Eqs 24–34; cited, not quoted), never from RELAP-7 or RELAP5 source.
//!
//! The four regions, by `Re = ρ|u|D_h/μ`:
//!
//! 1. `Re < 64`: RELAP-7 sets `f = f_max` (the manual names `f_max` without
//!    giving it). That cap makes the friction force quadratic in `u` as the
//!    flow stops, while the exact fully developed laminar solution, `64/(Re
//!    Φ_S)`, keeps it linear (Stokes) and is up to `64/Re` times larger. **The
//!    default here is the laminar law continued below 64**
//!    ([`LowReynolds::Laminar`]), correct physics first; RELAP-7's cap is
//!    [`LowReynolds::FMax`] (with `f_max = 1/Φ_S`, the laminar value at
//!    `Re = 64`), for parity runs.
//!
//!    Evidence for the default (2026-11-01): with the cap, the `tampines`
//!    two-fluid blowdown's volume constraint drifted to 1.68e-6 (gate 1e-6;
//!    8.0e-10 before), because nearly stagnant phases lost their linear drag;
//!    with the laminar law it is 8.0e-10 again.
//! 2. `64 ≤ Re < 2200`: laminar, `f = 64/(Re Φ_S)` (Eq 29), `Φ_S` the shape
//!    factor (1 for a circular pipe).
//! 3. `2200 ≤ Re < 3000`: reciprocal interpolation between the laminar value
//!    at 2200 and the turbulent value at 3000 (Eqs 30–33).
//! 4. `Re ≥ 3000`: Zigrang & Sylvester's explicit approximation to Colebrook
//!    (Eq 34), with the factor 1.14 that the manual says corrects the original
//!    1.114. Zigrang, D. J. & Sylvester, N. D. (1982), *AIChE J.* 28, 514.
//!
//! Lives here, beside the HEM pipe that uses it; `tampines` re-exports it as
//! `tampines::closures::wall_friction` for its two-phase solvers.

use petir::mathf::RealMath;

/// Lower edge of the laminar region (`f = f_max` below it).
pub const RE_F_MAX: f64 = 64.0;
/// Upper edge of the laminar region, `Re_min` of the transition.
pub const RE_LAMINAR_MAX: f64 = 2200.0;
/// Lower edge of the turbulent region, `Re_max` of the transition.
pub const RE_TURBULENT_MIN: f64 = 3000.0;

/// Zigrang–Sylvester turbulent Darcy friction factor at `re` for relative
/// roughness `eps_over_d = ε/D` (RELAP-7 Eq 34):
///
/// ```text
/// 1/√f = −2 log10( ε/(3.7D) + (2.51/Re)(1.14 − 2 log10(ε/D + 21.25/Re^0.9)) )
/// ```
pub fn zigrang_sylvester(re: f64, eps_over_d: f64) -> f64 {
    let inner = 1.14 - 2.0 * (eps_over_d + 21.25 / re.r_powf(0.9)).r_log10();
    let inv_sqrt_f = -2.0 * (eps_over_d / 3.7 + 2.51 / re * inner).r_log10();
    1.0 / (inv_sqrt_f * inv_sqrt_f)
}

/// What the friction factor does below `Re = 64` (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LowReynolds {
    /// The laminar law `64/(Re Φ_S)` continued to `Re → 0` (the default).
    #[default]
    Laminar,
    /// RELAP-7's cap, `f = f_max = 1/Φ_S`.
    FMax,
}

/// The four-region Darcy friction factor with the default low-Reynolds
/// treatment ([`LowReynolds::Laminar`]): see [`darcy_friction_factor_with`].
pub fn darcy_friction_factor(re: f64, eps_over_d: f64, shape_factor: f64) -> f64 {
    darcy_friction_factor_with(re, eps_over_d, shape_factor, LowReynolds::Laminar)
}

/// The RELAP5 four-region Darcy friction factor (module doc) at Reynolds
/// number `re` (`≥ 0`), relative roughness `eps_over_d` (`≥ 0`) and shape
/// factor `shape_factor` (`> 0`, 1 for a circular pipe), with the given
/// treatment below `Re = 64`. At `Re = 0` the laminar law is infinite; the
/// force it gives, `f u|u|`, is finite and is what
/// [`wall_friction_acceleration`] evaluates.
pub fn darcy_friction_factor_with(
    re: f64,
    eps_over_d: f64,
    shape_factor: f64,
    low: LowReynolds,
) -> f64 {
    let laminar = |re: f64| 64.0 / (re * shape_factor);
    if re < RE_F_MAX {
        match low {
            LowReynolds::Laminar => laminar(re),
            LowReynolds::FMax => laminar(RE_F_MAX),
        }
    } else if re < RE_LAMINAR_MAX {
        laminar(re)
    } else if re < RE_TURBULENT_MIN {
        let (lo, hi) = (RE_LAMINAR_MAX, RE_TURBULENT_MIN);
        let n = hi * lo / (hi - lo);
        let w = n / lo - n / re;
        (1.0 - w) * laminar(lo) + w * zigrang_sylvester(hi, eps_over_d)
    } else {
        zigrang_sylvester(re, eps_over_d)
    }
}

/// Wall-friction deceleration `F_wall/(ρA) = f u|u|/(2 D_h)`, m/s² (RELAP-7
/// Eq 24 per unit mass), carrying the sign of `u` so it opposes the motion.
/// Zero for `u = 0` or a non-positive density, viscosity or diameter.
pub fn wall_friction_acceleration(
    u: f64,
    rho: f64,
    mu: f64,
    hydraulic_diameter: f64,
    eps_over_d: f64,
    shape_factor: f64,
) -> f64 {
    if u == 0.0 || !(mu > 0.0 && rho > 0.0 && hydraulic_diameter > 0.0) {
        return 0.0;
    }
    let re = rho * u.abs() * hydraulic_diameter / mu;
    if re < RE_F_MAX {
        // Laminar, written as the Stokes force so it stays finite at Re → 0:
        // (64/(Re Φ)) u|u|/(2D) = 32 μ u/(Φ ρ D²).
        return 32.0 * mu * u / (shape_factor * rho * hydraulic_diameter * hydraulic_diameter);
    }
    darcy_friction_factor(re, eps_over_d, shape_factor) * u.abs() * u / (2.0 * hydraulic_diameter)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Colebrook–White solved by fixed-point iteration to 1e-14.
    fn colebrook(re: f64, eps_over_d: f64) -> f64 {
        let mut x: f64 = 8.0;
        for _ in 0..200 {
            x = -2.0 * (eps_over_d / 3.7 + 2.51 * x / re).log10();
        }
        1.0 / (x * x)
    }

    /// **Against Colebrook and the region boundaries.** Methodology:
    /// Zigrang–Sylvester against the implicit Colebrook equation (solved by
    /// iteration) over `Re = 4e3 … 1e8` and `ε/D = 0 … 0.05`; laminar
    /// `64/Re` exact; `f` continuous at 64, 2200 and 3000 (the interpolation
    /// reaches the turbulent value at 3000); the shape factor divides the
    /// laminar value. Prediction: within 1 % of Colebrook everywhere.
    /// **Result (2026-11-01):** the worst error is printed and asserted.
    #[test]
    fn friction_factor_follows_colebrook_and_is_continuous() {
        let mut worst = 0.0_f64;
        for &eps in &[0.0, 1e-5, 1e-4, 1e-3, 1e-2, 0.05] {
            let mut re = 4.0e3;
            while re <= 1.0e8 {
                let e = (zigrang_sylvester(re, eps) / colebrook(re, eps) - 1.0).abs();
                worst = worst.max(e);
                re *= 1.5;
            }
        }
        println!(
            "Zigrang-Sylvester worst error against Colebrook: {:.4} %",
            100.0 * worst
        );
        assert!(worst < 0.01, "{worst}");
        assert_eq!(darcy_friction_factor(1000.0, 0.0, 1.0), 0.064);
        assert_eq!(darcy_friction_factor(1000.0, 0.0, 2.0), 0.032);
        assert_eq!(
            darcy_friction_factor(10.0, 0.0, 1.0),
            6.4,
            "laminar continued below 64"
        );
        assert_eq!(
            darcy_friction_factor_with(10.0, 0.0, 1.0, LowReynolds::FMax),
            darcy_friction_factor(64.0, 0.0, 1.0)
        );
        // The Stokes form below Re = 64 equals the laminar law times u|u|/(2D).
        let (u, rho, mu, d) = (1.0e-3, 1000.0, 1.0e-3, 0.01);
        let re = rho * u * d / mu;
        let a = wall_friction_acceleration(u, rho, mu, d, 0.0, 1.0);
        assert!((a / (64.0 / re * u * u / (2.0 * d)) - 1.0).abs() < 1e-12);
        let eps = 1e-4;
        let below = darcy_friction_factor(2200.0 - 1e-9, eps, 1.0);
        let at = darcy_friction_factor(2200.0, eps, 1.0);
        assert!(
            (below / at - 1.0).abs() < 1e-9,
            "continuous at 2200: {below} {at}"
        );
        let below = darcy_friction_factor(3000.0 - 1e-9, eps, 1.0);
        let at = darcy_friction_factor(3000.0, eps, 1.0);
        assert!(
            (below / at - 1.0).abs() < 1e-9,
            "continuous at 3000: {below} {at}"
        );
        assert_eq!(
            wall_friction_acceleration(0.0, 1000.0, 1e-3, 0.1, 0.0, 1.0),
            0.0
        );
        let a = wall_friction_acceleration(-2.0, 1000.0, 1e-3, 0.1, 0.0, 1.0);
        assert!(a < 0.0, "opposes the motion");
        assert_eq!(
            wall_friction_acceleration(2.0, 1000.0, 0.0, 0.1, 0.0, 1.0),
            0.0
        );
    }
}
