//! Bowman/Underwood multi-pass LMTD correction factor `F`, for a shell with
//! `N` shell passes in series (each with 2, 4, 6, ... tube passes).
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s
//! `ShellandTube_Rating` (the F-factor sub-step; the surrounding Tinker
//! shell-side correlations are not ported, see `op-qo2.7`). Standard TEMA
//! reference formula (Bowman, Mueller & Nagle 1940).

use uom::si::f64::{Ratio, ThermodynamicTemperature};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Multi-pass LMTD correction factor `F` \[0, 1\] for `n_shell_passes`
/// shells in series, given the four terminal temperatures. Multiply the
/// single-pass counter-current LMTD (see
/// [`crate::heat_exchanger::lmtd::lmtd`]) by this factor to get the
/// effective mean temperature difference for a multi-pass shell-and-tube
/// exchanger: `ΔT_m = F * LMTD`.
///
/// `t_shell_in`/`t_shell_out` are the shell-side fluid's inlet/outlet
/// temperatures; `t_tube_in`/`t_tube_out` the tube-side fluid's.
pub fn f_correction_factor(
    n_shell_passes: u32,
    t_shell_in: ThermodynamicTemperature,
    t_shell_out: ThermodynamicTemperature,
    t_tube_in: ThermodynamicTemperature,
    t_tube_out: ThermodynamicTemperature,
) -> Ratio {
    let n = n_shell_passes as f64;
    let t_s_in = t_shell_in.get::<kelvin>();
    let t_s_out = t_shell_out.get::<kelvin>();
    let t_t_in = t_tube_in.get::<kelvin>();
    let t_t_out = t_tube_out.get::<kelvin>();

    let r = (t_s_in - t_s_out) / (t_t_out - t_t_in);
    let p = (t_t_out - t_t_in) / (t_s_in - t_t_in);

    let f = if (r - 1.0).abs() > 1e-9 {
        let alpha = ((1.0 - r * p) / (1.0 - p)).powf(1.0 / n);
        let sf = (alpha - 1.0) / (alpha - r);
        let sqrt_r2_1 = (r * r + 1.0).sqrt();
        (sqrt_r2_1 * ((1.0 - sf) / (1.0 - r * sf)).ln())
            / ((r - 1.0)
                * ((2.0 - sf * (r + 1.0 - sqrt_r2_1)) / (2.0 - sf * (r + 1.0 + sqrt_r2_1))).ln())
    } else {
        let sf = p / (n * (1.0 - p) + p);
        (sf * std::f64::consts::SQRT_2)
            / ((1.0 - sf)
                * ((2.0 * (1.0 - sf) + sf * std::f64::consts::SQRT_2)
                    / (2.0 * (1.0 - sf) - sf * std::f64::consts::SQRT_2))
                    .ln())
    };
    Ratio::new::<ratio>(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_shell_pass_f_factor_within_unit_range() {
        let f = f_correction_factor(
            1,
            ThermodynamicTemperature::new::<kelvin>(373.15),
            ThermodynamicTemperature::new::<kelvin>(333.15),
            ThermodynamicTemperature::new::<kelvin>(293.15),
            ThermodynamicTemperature::new::<kelvin>(313.15),
        );
        assert!(f.get::<ratio>() > 0.0 && f.get::<ratio>() <= 1.0);
    }

    #[test]
    fn more_shell_passes_gives_higher_or_equal_f_for_same_duty() {
        // P/R chosen comfortably inside the achievable region for N=1 --
        // the F-chart method's monotonicity-in-N only holds there; near or
        // past the N=1 feasibility ceiling, F vs. N is not guaranteed
        // monotonic (a real limitation of the method, not this port).
        let f1 = f_correction_factor(
            1,
            ThermodynamicTemperature::new::<kelvin>(373.15),
            ThermodynamicTemperature::new::<kelvin>(350.0),
            ThermodynamicTemperature::new::<kelvin>(293.15),
            ThermodynamicTemperature::new::<kelvin>(320.0),
        );
        let f2 = f_correction_factor(
            2,
            ThermodynamicTemperature::new::<kelvin>(373.15),
            ThermodynamicTemperature::new::<kelvin>(350.0),
            ThermodynamicTemperature::new::<kelvin>(293.15),
            ThermodynamicTemperature::new::<kelvin>(320.0),
        );
        assert!(f2.get::<ratio>() >= f1.get::<ratio>());
    }

    #[test]
    fn balanced_r_equals_1_case_does_not_panic() {
        // Symmetric temperature spans -> R = 1 branch.
        let f = f_correction_factor(
            2,
            ThermodynamicTemperature::new::<kelvin>(373.15),
            ThermodynamicTemperature::new::<kelvin>(333.15),
            ThermodynamicTemperature::new::<kelvin>(293.15),
            ThermodynamicTemperature::new::<kelvin>(333.15),
        );
        assert!(f.get::<ratio>().is_finite());
    }
}
