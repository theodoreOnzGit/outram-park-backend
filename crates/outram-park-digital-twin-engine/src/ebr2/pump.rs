//! # EBR-II primary pump: the NATDEMO homologous head curve
//!
//! A226 Eqs 3.1-3.7 (pp.15-16), coefficients in
//! [`super::design::PUMP_HEAD_COEFFICIENTS`]. In normalised variables
//! `h = H/H_r`, `s = n/n_r`, `w = W/W_r` (per pump):
//!
//! ```text
//! h = b1 s^2 + b2 s w + b3 sign(w) |w|^b4
//!
//! b3, b4 = b3m, b4t   if s >  eps_m w                 (normal operation)
//!          b3t, b4t   if s <= eps_m w and w >= wt     (turbine region)
//!          b3l, b4l   if s <= eps_m w and w <  wt     (low flow, stopped rotor)
//! ```
//!
//! **The form is as transcribed in `reference-data/ebr2/arc226_pump_model.csv`
//! and gh:#1315, both of which ask for it to be checked against the rendered
//! page** (the PDF text layer is garbled around Eq 3.1). Nothing here is
//! fitted: the coefficients are the printed ones, and
//! `b1 + b2 + b3m = 1.0000` puts the rated point on the curve.
//!
//! New code (no workspace equivalent: `tampines::components::pump_coastdown`
//! is the speed-only special case and its module doc names this homologous
//! form as open follow-up, gh:#1322).

use petir::mathf::RealMath;

use super::design::PUMP_HEAD_COEFFICIENTS;

/// Which branch of the NATDEMO curve applies at `(s, w)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpRegime {
    /// `s > eps_m w`: `b3m`, `b4t`.
    Normal,
    /// `s <= eps_m w`, `w >= wt`: `b3t`, `b4t`.
    Turbine,
    /// `s <= eps_m w`, `w < wt`: `b3l`, `b4l`.
    LowFlow,
}

/// The regime at normalised speed `s` and flow `w`.
pub fn regime(s: f64, w: f64) -> PumpRegime {
    let [_, _, _, _, _, _, _, wt, eps_m] = PUMP_HEAD_COEFFICIENTS;
    if s > eps_m * w {
        PumpRegime::Normal
    } else if w >= wt {
        PumpRegime::Turbine
    } else {
        PumpRegime::LowFlow
    }
}

/// Normalised head `h(s, w) = H / H_r` (A226 Eq 3.1 with Eq 3.7's
/// coefficients).
pub fn head_ratio(s: f64, w: f64) -> f64 {
    let [b1, b2, b3m, b3t, b3l, b4t, b4l, _, _] = PUMP_HEAD_COEFFICIENTS;
    let (b3, b4) = match regime(s, w) {
        PumpRegime::Normal => (b3m, b4t),
        PumpRegime::Turbine => (b3t, b4t),
        PumpRegime::LowFlow => (b3l, b4l),
    };
    let flow_term = if w == 0.0 {
        0.0
    } else {
        w.signum() * w.abs().r_powf(b4)
    };
    b1 * s * s + b2 * s * w + b3 * flow_term
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At the rated point the curve gives the rated head exactly
    /// (`1.1740 + 0.0818 - 0.2558 = 1`), and at shut-off (`w = 0`) it gives
    /// `b1 s^2`.
    #[test]
    fn rated_point_and_shutoff() {
        assert!((head_ratio(1.0, 1.0) - 1.0).abs() < 1e-12);
        assert_eq!(regime(1.0, 1.0), PumpRegime::Normal);
        assert!((head_ratio(0.5, 0.0) - 1.1740 * 0.25).abs() < 1e-12);
    }

    /// A stopped rotor with forward flow is a resistance: the low-flow
    /// branch is linear, `h = b3l w`, and the turbine branch is `b3t w^1.9`.
    #[test]
    fn stopped_rotor_branches() {
        assert_eq!(regime(0.0, 0.03), PumpRegime::LowFlow);
        assert!((head_ratio(0.0, 0.03) - (-0.0471 * 0.03)).abs() < 1e-15);
        assert_eq!(regime(0.0, 0.5), PumpRegime::Turbine);
        assert!((head_ratio(0.0, 0.5) - (-0.5923 * 0.5_f64.r_powf(1.9))).abs() < 1e-12);
        // reverse flow through a running pump stays on the normal branch,
        // and the sign factor makes the b3 term add head
        assert_eq!(regime(0.5, -0.1), PumpRegime::Normal);
        let hand = 1.1740 * 0.25 + 0.0818 * 0.5 * (-0.1) + (-0.2558) * (-(0.1_f64.r_powf(1.9)));
        assert!((head_ratio(0.5, -0.1) - hand).abs() < 1e-15);
    }
}
