// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!

//! **Eq (2)** — the SiC hoop stress induced by the internal gas pressure
//! (page -484-), in both the report's approximate and exact thin-shell forms.

use uom::si::f64::{Pressure, Ratio, Time, Velocity};
use uom::si::ratio::ratio;

use super::geometry::SicLayer;

/// **Eq (2)** — the SiC hoop stress induced by internal gas pressure, in the
/// report's preferred approximate form (page -484-).
///
/// ```text
/// σ_t = r·p / (2·d_o) · (1 + v̇·t/d_o)      [Pa]
/// ```
///
/// The report gives the exact thin-shell result first
/// ([`induced_stress_exact`]) and then states that this approximation
/// "describes the state of affairs more realistically, in particular for small
/// actual SiC layer thicknesses" — it is the linearisation of the exact form in
/// `v̇t/d_o`, and unlike the exact form it stays finite as the layer thins.
/// **This is the one to use**; the exact form is provided for comparison.
///
/// Note the model is **thin-shell throughout**. The report contains no
/// thick-wall (Lamé) formulation, so none is offered here.
pub fn induced_stress(
    layer: &SicLayer,
    pressure: Pressure,
    corrosion_rate: Velocity,
    elapsed: Time,
) -> Pressure {
    let d_o = layer.initial_thickness();
    let thinning: Ratio = corrosion_rate * elapsed / d_o;
    let base: Pressure = layer.mean_radius() * pressure / (2.0 * d_o);
    base * (Ratio::new::<ratio>(1.0) + thinning)
}

/// The **exact** thin-shell stress, `σ_t = r·p / (2·d_act)` (page -484-).
///
/// Returns `None` once the corroded thickness is non-positive, rather than
/// returning an infinite or negative stress. [`induced_stress`] is the form the
/// report actually recommends.
pub fn induced_stress_exact(
    layer: &SicLayer,
    pressure: Pressure,
    corrosion_rate: Velocity,
    elapsed: Time,
) -> Option<Pressure> {
    let d_act = layer.actual_thickness(corrosion_rate, elapsed);
    if d_act.value <= 0.0 {
        return None;
    }
    Some(layer.mean_radius() * pressure / (2.0 * d_act))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, Pressure};
    use uom::si::length::{meter, micrometer};
    use uom::si::pressure::{megapascal, pascal};
    use uom::si::time::{hour, second};
    use uom::si::velocity::meter_per_second;

    /// A representative TRISO SiC layer: a 35 um shell whose inner surface
    /// sits at 250 um. Dimensions only; nothing is calibrated to them.
    fn layer() -> SicLayer {
        SicLayer {
            inner_radius: Length::new::<micrometer>(250.0),
            outer_radius: Length::new::<micrometer>(285.0),
        }
    }

    #[test]
    fn approximate_and_exact_stress_agree_at_zero_corrosion() {
        let (l, p) = (layer(), Pressure::new::<megapascal>(30.0));
        let none = Velocity::new::<meter_per_second>(0.0);
        let t0 = Time::new::<second>(0.0);
        let approx = induced_stress(&l, p, none, t0).get::<pascal>();
        let exact = induced_stress_exact(&l, p, none, t0)
            .unwrap()
            .get::<pascal>();
        assert!((approx - exact).abs() < 1e-6, "{approx} vs {exact}");

        // r*p/(2*d_o) computed independently: 115.1314 MPa.
        let expected = 268.639_995e-6 * 30.0e6 / (2.0 * 35.0e-6);
        assert!(
            (approx - expected).abs() / expected < 1e-3,
            "{approx} vs {expected}"
        );

        // Under corrosion the exact form exceeds the approximation.
        let v = Velocity::new::<meter_per_second>(1.0e-10);
        let t = Time::new::<hour>(20.0);
        let a = induced_stress(&l, p, v, t).get::<pascal>();
        let e = induced_stress_exact(&l, p, v, t).unwrap().get::<pascal>();
        assert!(
            e > a,
            "exact {e} should exceed approximate {a} under thinning"
        );
    }

    #[test]
    fn a_consumed_layer_has_no_exact_stress() {
        let l = layer();
        let v = Velocity::new::<meter_per_second>(1.0e-8);
        let t = Time::new::<hour>(10_000.0);
        assert!(l.actual_thickness(v, t).get::<meter>() <= 0.0);
        assert!(induced_stress_exact(&l, Pressure::new::<megapascal>(30.0), v, t).is_none());
    }
}
