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

//! **The SiC layer's geometry** — Eq (2)'s `r`, `d_o` and `d_act` (page -484-).
//!
//! Split out because these are the figure-independent facts about a particle:
//! every other module here consumes them and none of them depends on the rest.

use uom::si::f64::{Length, Ratio, Time, Velocity};
use uom::si::ratio::ratio;

/// The SiC layer's geometry, from which Eq (2)'s `r` and `d_o` are derived.
///
/// Both radii are to the SiC layer itself — `r_i` its inner surface (the outer
/// surface of the inner PyC) and `r_a` its outer surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SicLayer {
    /// Inner radius `r_i` of the SiC layer.
    pub inner_radius: Length,
    /// Outer radius `r_a` of the SiC layer.
    pub outer_radius: Length,
}

impl SicLayer {
    /// The report's **average radius** `r = (0.5·(r_a³ + r_i³))^(1/3)`
    /// (page -484-).
    ///
    /// Note this is a cube-root mean, not the arithmetic mean `(r_a + r_i)/2`.
    /// The two agree to well under a percent for a real TRISO layer, which is
    /// precisely why substituting the arithmetic mean would never show up as
    /// an obvious error — so the printed definition is kept.
    pub fn mean_radius(&self) -> Length {
        let ri = self.inner_radius.value;
        let ra = self.outer_radius.value;
        Length::new::<uom::si::length::meter>((0.5 * (ra.powi(3) + ri.powi(3))).cbrt())
    }

    /// The original layer thickness `d_o = r_a − r_i` (page -484-).
    pub fn initial_thickness(&self) -> Length {
        self.outer_radius - self.inner_radius
    }

    /// **Eq (7)** — the actual thickness after volume corrosion (page -492-):
    ///
    /// ```text
    /// d_act = d_o / (1 + v̇·t/d_o)
    /// ```
    ///
    /// ## Not the form printed on page -484-
    ///
    /// Page -484- writes the exact stress as `r·p / (2·d_o·(1 − v̇·t))`,
    /// implying `d_act = d_o·(1 − v̇·t)`. That form is **dimensionally
    /// inconsistent** — `v̇·t` is a length, so `1 − v̇·t` subtracts metres
    /// from a pure number — and it contradicts Eq (7) on page -492-, which is
    /// dimensionally sound and is what Fig. 4 plots. Eq (7) is implemented.
    ///
    /// This was originally written the -484- way and corrected on 2026-09-24
    /// when Fig. 4 was digitised: see
    /// `docs/panama-i-units-and-open-questions.md`.
    ///
    /// Always positive and monotonically decreasing — unlike the -484- form,
    /// which goes negative once `v̇·t > 1 m`. A layer thins asymptotically
    /// toward zero here, which is the physically right behaviour.
    pub fn actual_thickness(&self, corrosion_rate: Velocity, elapsed: Time) -> Length {
        let d_o = self.initial_thickness();
        let fkor: Ratio = Ratio::new::<ratio>(1.0) + corrosion_rate * elapsed / d_o;
        d_o / fkor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;

    /// A representative TRISO SiC layer: a 35 um shell whose inner surface
    /// sits at 250 um. Dimensions only; nothing is calibrated to them.
    fn layer() -> SicLayer {
        SicLayer {
            inner_radius: Length::new::<micrometer>(250.0),
            outer_radius: Length::new::<micrometer>(285.0),
        }
    }

    #[test]
    fn mean_radius_is_the_cube_root_mean() {
        let l = layer();
        let r_cbrt = l.mean_radius().get::<micrometer>();
        let arithmetic = 0.5 * (250.0 + 285.0);
        assert!(
            (r_cbrt - 268.639_995).abs() < 1e-4,
            "cube-root mean should be ~268.640 um, got {r_cbrt}"
        );
        assert!(
            (r_cbrt - arithmetic).abs() > 0.1,
            "must not silently be the arithmetic mean ({arithmetic})"
        );
        assert!((l.initial_thickness().get::<micrometer>() - 35.0).abs() < 1e-9);
    }
}
