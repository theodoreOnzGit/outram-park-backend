//! Stored fission gas released when water vapour reaches **exposed** UO2
//! kernels (the fuel of defective and failed particles).
//!
//! # Source
//!
//! IAEA-TECDOC-978, *Fuel performance and fission product behaviour in gas
//! cooled reactors* (IAEA, Vienna, 1997), section 5.3.1.1, **Eq. (5-2)**,
//! printed p. 223 (proprietary tier: cited, not redistributed):
//!
//! ```text
//! f = 2.13e13 P^(-4.353 + 6503/T) exp(-4.7257e4 / T)
//! ```
//!
//! `f` is the fraction of an exposed kernel's **noble-gas** inventory released
//! as "stored" gas when water vapour at partial pressure `P` \[Pa\] reaches it
//! at temperature `T` \[K\] (the stage-1 burst of the HFR-B1 / HRB-17
//! injection tests; `Q = 392.9 kJ/mol`). It is a **one-time** release per
//! exposure, not a rate: a caller applies the increase of `f` over what has
//! already been released.
//!
//! **Validity** (TECDOC-978, same page): the HFR-B1 data it was fitted to,
//! **820-1040 degC and 2.8-1051 Pa** water vapour, UO2 kernels. Outside it the
//! fit is an extrapolation and [`stored_gas_fraction`] says so; the result is
//! clamped to `[0, 1]` because it is a fraction of an inventory, and the
//! clamp is reported. The TECDOC itself notes the extrapolated line reaches
//! complete release near 2 kPa at 770 degC.
//!
//! **Known limitation (gh:#418):** in an HTGR water-ingress accident the
//! steam partial pressure is hundreds of kPa, three orders of magnitude above
//! the fit. There the function returns [`Validity::ClampedToWholeInventory`]:
//! every exposed kernel's whole stored noble gas. Callers must surface that
//! flag, not swallow it; a model valid at those pressures is gh:#418.
//!
//! Not modelled: the stage-2 steady enhancement of `R/B` under continued
//! water vapour (TECDOC-978 Eq. 5-3, the `h_o` factors), and iodine or metal
//! release from hydrolysed kernels.

use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Whether [`stored_gas_fraction`] was evaluated inside the fitted range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// 820-1040 degC and 2.8-1051 Pa.
    InsideFittedRange,
    /// Outside it: extrapolated.
    Extrapolated,
    /// Extrapolated past a whole inventory and clamped to 1.
    ClampedToWholeInventory,
}

/// TECDOC-978 Eq. (5-2): the stored-gas fraction of an exposed kernel's noble
/// gas released at water-vapour partial pressure `steam` and temperature
/// `temperature`. Zero for no steam.
pub fn stored_gas_fraction(
    temperature: ThermodynamicTemperature,
    steam: Pressure,
) -> (Ratio, Validity) {
    let t = temperature.get::<kelvin>();
    let p = steam.get::<pascal>();
    if !(p > 0.0 && t > 0.0) {
        return (Ratio::new::<ratio>(0.0), Validity::Extrapolated);
    }
    let f = 2.13e13 * p.powf(-4.353 + 6503.0 / t) * (-4.7257e4 / t).exp();
    let inside = (1093.15..=1313.15).contains(&t) && (2.8..=1051.0).contains(&p);
    let validity = if f > 1.0 {
        Validity::ClampedToWholeInventory
    } else if inside {
        Validity::InsideFittedRange
    } else {
        Validity::Extrapolated
    };
    (Ratio::new::<ratio>(f.clamp(0.0, 1.0)), validity)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Eq. (5-2) is transcribed.** Methodology: evaluate at 1000 degC and
    /// 1000 Pa, inside the fitted range, and compare with an independent
    /// hand evaluation (Python, 2026-09-29: exponent m = 0.754803, f =
    /// 0.296875) to 1e-4 relative; check the clamp and the flags.
    /// Results (2026-09-29): pass.
    #[test]
    fn eq_5_2_is_transcribed_and_clamped() {
        let at = |t_c: f64, p: f64| {
            stored_gas_fraction(
                ThermodynamicTemperature::new::<kelvin>(t_c + 273.15),
                Pressure::new::<pascal>(p),
            )
        };
        let (f, v) = at(1000.0, 1000.0);
        let f = f.get::<ratio>();
        println!("TECDOC-978 Eq. 5-2 at 1000 degC, 1000 Pa: f = {f:.5}");
        assert_eq!(v, Validity::InsideFittedRange);
        assert!((f - 0.296875).abs() / 0.296875 < 1e-4, "{f}");
        assert_eq!(at(1000.0, 0.0).0.get::<ratio>(), 0.0);
        assert_eq!(at(1000.0, 5.0e5).1, Validity::ClampedToWholeInventory);
        assert_eq!(at(1000.0, 5.0e5).0.get::<ratio>(), 1.0);
        assert_eq!(at(1100.0, 500.0).1, Validity::Extrapolated);
    }
}
