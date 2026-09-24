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

/// The thin-shell stress written through the thickness, `σ_t = r·p/(2·d_act)`
/// with `d_act` from Eq (7) (pages -484- and -492-).
///
/// ## This EQUALS [`induced_stress`]; it is not an alternative to it
///
/// Substituting Eq (7)'s `d_act = d_o/(1 + v̇t/d_o)` gives
/// `σ_t = r·p·(1 + v̇t/d_o)/(2·d_o)`, which is Eq (2) **exactly**. The
/// report's description of Eq (2) as an approximation that "describes the
/// state of affairs more realistically" therefore understates it: given
/// Eq (7), Eq (2) is not an approximation at all.
///
/// What Eq (2) approximates is the *other* form printed on page -484-,
/// `d_act = d_o·(1 − v̇·t)`, which is dimensionally inconsistent and
/// contradicts Eq (7). See [`SicLayer::actual_thickness`].
///
/// Kept as its own function because computing the stress by two routes and
/// asserting they agree is a real check on the algebra —
/// `tests::the_two_routes_to_the_stress_agree_exactly`. `None` only if the
/// thickness is non-positive, which Eq (7) cannot produce; it guards against
/// a caller supplying a negative rate.
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
    use uom::si::length::micrometer;
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

    /// **The two routes to the stress are the same expression.**
    ///
    /// Eq (2) is `r·p/(2·d_o)·(1 + v̇t/d_o)`; going through Eq (7)'s
    /// thickness gives `r·p/(2·d_act)` with `d_act = d_o/(1 + v̇t/d_o)`.
    /// Those are algebraically identical, so they must agree to floating
    /// point -- not "closely". If this ever drifts, one of the two has been
    /// changed without the other.
    ///
    /// This test replaced one asserting the exact form should EXCEED the
    /// approximate under thinning, which was true only of the dimensionally
    /// inconsistent `d_o·(1 − v̇·t)` on page -484-. See
    /// [`SicLayer::actual_thickness`].
    #[test]
    fn the_two_routes_to_the_stress_agree_exactly() {
        let (l, p) = (layer(), Pressure::new::<megapascal>(30.0));
        let v = Velocity::new::<meter_per_second>(5.0e-11);
        for hours in [0.0, 1.0, 50.0, 300.0, 5000.0] {
            let t = Time::new::<hour>(hours);
            let a = induced_stress(&l, p, v, t).get::<pascal>();
            let e = induced_stress_exact(&l, p, v, t)
                .expect("Eq (7) is always positive")
                .get::<pascal>();
            assert!(
                (a - e).abs() / a < 1e-12,
                "at {hours} h: Eq (2) {a} vs r*p/(2*d_act) {e}"
            );
        }
    }

    /// With no corrosion the stress is the plain thin-shell result, computed
    /// here independently: r*p/(2*d_o) with r the cube-root mean.
    #[test]
    fn with_no_corrosion_it_is_the_plain_thin_shell_stress() {
        let (l, p) = (layer(), Pressure::new::<megapascal>(30.0));
        let got = induced_stress(
            &l,
            p,
            Velocity::new::<meter_per_second>(0.0),
            Time::new::<second>(0.0),
        )
        .get::<pascal>();
        let expected = 268.639_995e-6 * 30.0e6 / (2.0 * 35.0e-6); // 115.1314 MPa
        assert!(
            (got - expected).abs() / expected < 1e-3,
            "{got} vs {expected}"
        );
    }

    /// Stress rises as the layer thins, without bound and without ever
    /// changing sign. Eq (7) thins asymptotically toward zero thickness, so
    /// there is no time at which the stress is undefined -- which is exactly
    /// what the page -484- form got wrong (it crosses zero and goes
    /// negative).
    #[test]
    fn stress_rises_monotonically_as_the_layer_thins() {
        let (l, p) = (layer(), Pressure::new::<megapascal>(30.0));
        let v = Velocity::new::<meter_per_second>(1.0e-10);
        let mut last = 0.0;
        for hours in [0.0, 10.0, 100.0, 1000.0, 100_000.0] {
            let s = induced_stress(&l, p, v, Time::new::<hour>(hours)).get::<pascal>();
            assert!(s > last, "stress must rise: {s} after {last}");
            assert!(s.is_finite(), "and stay finite");
            last = s;
        }
        // The thickness never reaches zero, so the stress never blows up.
        let far = l
            .actual_thickness(v, Time::new::<hour>(1.0e9))
            .get::<micrometer>();
        assert!(far > 0.0, "Eq (7) thins asymptotically, never past zero");
    }
}
