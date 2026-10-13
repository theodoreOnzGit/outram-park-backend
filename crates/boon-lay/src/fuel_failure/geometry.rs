// SPDX-License-Identifier: GPL-3.0
//
// boon-lay fuel failure — provenance
// ----------------------------------
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
// Nature    : boon-lay fuel failure is boon-lay's own model: a Rust
//             implementation of the PANAMA-I FORMULAS, coded agentically (by an
//             AI coding agent, then reviewed) from the published equations. It
//             is not the PANAMA code and not a port of the PANAMA Fortran
//             (closed-source, never consulted); call it "boon-lay fuel failure",
//             and keep "PANAMA-I" for the report and its own printed results.

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
///
/// **Guarded (2026-10-13).** A layer is built only through [`SicLayer::new`],
/// which refuses radii that cannot describe a shell, so every method here may
/// rely on `0 < r_i < r_a`, both finite. ~~The two radii were public fields
/// and nothing stopped an impossible layer~~: an outer radius inside the inner
/// one gave a negative thickness, and with it a negative stress, with no
/// error anywhere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SicLayer {
    inner_radius: Length,
    outer_radius: Length,
}

/// Why a pair of radii is not a SiC layer. Each case carries the offending
/// value(s) in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SicLayerError {
    /// A radius is NaN or infinite (inner, outer).
    NotFinite(f64, f64),
    /// The inner radius is zero or negative: that is a solid sphere or
    /// nonsense, and Eq (2)'s thin-shell stress does not describe either.
    InnerRadiusNotPositive(f64),
    /// The outer radius is not beyond the inner one (inner, outer), so the
    /// layer has zero or negative thickness. Swapping the two arguments
    /// lands here.
    OuterNotBeyondInner(f64, f64),
}

impl std::fmt::Display for SicLayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite(ri, ra) => write!(f, "SiC layer radius is not finite: inner {ri} m, outer {ra} m"),
            Self::InnerRadiusNotPositive(ri) => write!(f, "SiC layer inner radius must be positive, got {ri} m"),
            Self::OuterNotBeyondInner(ri, ra) => {
                write!(f, "SiC layer outer radius ({ra} m) must exceed its inner radius ({ri} m)")
            }
        }
    }
}

impl std::error::Error for SicLayerError {}

/// Why a corrosion rate and an elapsed time cannot be used to thin a layer.
/// Each case carries the offending value(s) in SI units (m/s, s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CorrosionInputError {
    /// The rate or the time is NaN or infinite (rate, time).
    NotFinite(f64, f64),
    /// A negative corrosion rate: the layer would grow, and at
    /// `v̇·t ≤ −d_o` Eq (7) divides by zero or returns a negative thickness.
    NegativeRate(f64),
    /// A negative elapsed time, which has the same effect as a negative rate.
    NegativeTime(f64),
}

impl std::fmt::Display for CorrosionInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite(v, t) => write!(f, "corrosion rate ({v} m/s) or elapsed time ({t} s) is not finite"),
            Self::NegativeRate(v) => write!(f, "corrosion rate must not be negative, got {v} m/s"),
            Self::NegativeTime(t) => write!(f, "elapsed time must not be negative, got {t} s"),
        }
    }
}

impl std::error::Error for CorrosionInputError {}

impl SicLayer {
    /// A layer from its inner radius `r_i` and outer radius `r_a`, in that
    /// order. Refuses radii that are not finite, an inner radius that is not
    /// positive, and an outer radius that is not beyond the inner one.
    pub fn new(inner_radius: Length, outer_radius: Length) -> Result<Self, SicLayerError> {
        let (ri, ra) = (inner_radius.value, outer_radius.value);
        if !ri.is_finite() || !ra.is_finite() {
            return Err(SicLayerError::NotFinite(ri, ra));
        }
        if ri <= 0.0 {
            return Err(SicLayerError::InnerRadiusNotPositive(ri));
        }
        if ra <= ri {
            return Err(SicLayerError::OuterNotBeyondInner(ri, ra));
        }
        Ok(Self { inner_radius, outer_radius })
    }

    /// Inner radius `r_i` of the SiC layer.
    pub fn inner_radius(&self) -> Length {
        self.inner_radius
    }

    /// Outer radius `r_a` of the SiC layer.
    pub fn outer_radius(&self) -> Length {
        self.outer_radius
    }

    /// The report's **average radius** `r = (0.5·(r_a³ + r_i³))^(1/3)`
    /// (page -484-).
    ///
    /// Note this is a cube-root mean, not the arithmetic mean `(r_a + r_i)/2`.
    /// The two agree to well under a percent for a real TRISO layer, which is
    /// precisely why substituting the arithmetic mean would never show up as
    /// an obvious error — so the printed definition is kept.
    ///
    /// Needs no check of its own: [`SicLayer::new`] has already refused
    /// impossible radii, so the result lies strictly between `r_i` and `r_a`.
    pub fn cbrt_mean_radius(&self) -> Length {
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
    ///
    /// **Guarded (2026-10-13).** "Always positive" holds only for a rate and a
    /// time that are not negative; ~~nothing checked that~~, and
    /// `v̇·t = −d_o` divided by zero. Such inputs are now refused through
    /// [`SicLayer::isothermal_thinning_factor`].
    pub fn actual_thickness(&self, corrosion_rate: Velocity, elapsed: Time) -> Result<Length, CorrosionInputError> {
        Ok(self.initial_thickness() / self.isothermal_thinning_factor(corrosion_rate, elapsed)?)
    }

    /// `FKOR = 1 + v̇·t/d_o` for one constant rate held for `elapsed`
    /// (page -492-), so `d_act = d_o/FKOR`. Never below 1.
    ///
    /// The one place a caller-supplied rate and time are checked: a value that
    /// is not finite, a negative rate and a negative time are refused.
    /// [`super::thinning_factor`] is the same quantity with the rate taken
    /// from the report's corrosion law at a temperature.
    pub fn isothermal_thinning_factor(
        &self,
        corrosion_rate: Velocity,
        elapsed: Time,
    ) -> Result<Ratio, CorrosionInputError> {
        let (v, t) = (corrosion_rate.value, elapsed.value);
        if !v.is_finite() || !t.is_finite() {
            return Err(CorrosionInputError::NotFinite(v, t));
        }
        if v < 0.0 {
            return Err(CorrosionInputError::NegativeRate(v));
        }
        if t < 0.0 {
            return Err(CorrosionInputError::NegativeTime(t));
        }
        Ok(Ratio::new::<ratio>(1.0) + corrosion_rate * elapsed / self.initial_thickness())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;

    /// A representative TRISO SiC layer: a 35 um shell whose inner surface
    /// sits at 250 um. Dimensions only; nothing is calibrated to them.
    fn layer() -> SicLayer {
        SicLayer::new(Length::new::<micrometer>(250.0), Length::new::<micrometer>(285.0)).unwrap()
    }

    #[test]
    fn cbrt_mean_radius_is_the_cube_root_mean() {
        let l = layer();
        let r_cbrt = l.cbrt_mean_radius().get::<micrometer>();
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
        let (ri, ra) = (l.inner_radius().get::<micrometer>(), l.outer_radius().get::<micrometer>());
        assert!(ri < r_cbrt && r_cbrt < ra, "the mean lies inside the layer");
    }

    /// **Methodology.** `SicLayer::new` is given each kind of impossible pair
    /// of radii and must refuse it with the matching error; a real layer must
    /// be accepted with its radii unchanged. The pass criterion is exact.
    ///
    /// **Result (2026-10-13).** All seven cases behave as stated: swapped
    /// radii, equal radii, a zero and a negative inner radius, NaN and
    /// infinity are refused; 250/285 um is accepted.
    #[test]
    fn impossible_radii_are_refused() {
        let um = Length::new::<micrometer>;
        let m = |x: f64| um(x).value;
        assert_eq!(SicLayer::new(um(285.0), um(250.0)), Err(SicLayerError::OuterNotBeyondInner(m(285.0), m(250.0))));
        assert_eq!(SicLayer::new(um(250.0), um(250.0)), Err(SicLayerError::OuterNotBeyondInner(m(250.0), m(250.0))));
        assert_eq!(SicLayer::new(um(0.0), um(35.0)), Err(SicLayerError::InnerRadiusNotPositive(0.0)));
        assert_eq!(SicLayer::new(um(-250.0), um(285.0)), Err(SicLayerError::InnerRadiusNotPositive(m(-250.0))));
        assert!(matches!(SicLayer::new(um(f64::NAN), um(285.0)), Err(SicLayerError::NotFinite(..))));
        assert!(matches!(SicLayer::new(um(250.0), um(f64::INFINITY)), Err(SicLayerError::NotFinite(..))));
        let ok = SicLayer::new(um(250.0), um(285.0)).unwrap();
        assert_eq!((ok.inner_radius(), ok.outer_radius()), (um(250.0), um(285.0)));
        let msg = SicLayer::new(um(285.0), um(250.0)).unwrap_err().to_string();
        assert!(msg.contains("must exceed"), "{msg}");
    }

    /// **Methodology.** The thinning of the 250/285 um layer is asked for
    /// with each kind of impossible rate or time, which must be refused with
    /// the matching error, and with a real pair, which must give Eq (7).
    /// The real pair is `v̇·t = 35 um` (1e-10 m/s for 3.5e5 s), chosen so
    /// that `FKOR = 2` and `d_act = d_o/2 = 17.5 um` by hand.
    ///
    /// **Result (2026-10-13).** Negative rate, negative time, NaN and
    /// infinity are refused; the real pair gives 17.5 um to 1e-9 um, and
    /// zero rate and zero time give the full 35 um.
    #[test]
    fn impossible_corrosion_inputs_are_refused() {
        use uom::si::time::second;
        use uom::si::velocity::meter_per_second;
        let (l, v, t) = (layer(), Velocity::new::<meter_per_second>, Time::new::<second>);
        assert_eq!(l.actual_thickness(v(-1.0e-10), t(3.5e5)), Err(CorrosionInputError::NegativeRate(-1.0e-10)));
        assert_eq!(l.actual_thickness(v(1.0e-10), t(-3.5e5)), Err(CorrosionInputError::NegativeTime(-3.5e5)));
        assert!(matches!(l.actual_thickness(v(f64::NAN), t(1.0)), Err(CorrosionInputError::NotFinite(..))));
        assert!(matches!(l.actual_thickness(v(1.0e-10), t(f64::INFINITY)), Err(CorrosionInputError::NotFinite(..))));
        let half = l.actual_thickness(v(1.0e-10), t(3.5e5)).unwrap().get::<micrometer>();
        assert!((half - 17.5).abs() < 1e-9, "{half}");
        let full = l.actual_thickness(v(0.0), t(0.0)).unwrap().get::<micrometer>();
        assert!((full - 35.0).abs() < 1e-9, "{full}");
        let msg = l.actual_thickness(v(-1.0), t(1.0)).unwrap_err().to_string();
        assert!(msg.contains("must not be negative"), "{msg}");
    }
}
