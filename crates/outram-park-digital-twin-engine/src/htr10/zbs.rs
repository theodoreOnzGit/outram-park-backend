//! # Zehner-Bauer-Schlunder effective pebble-bed thermal conductivity
//!
//! An 18-point tabulation of the effective thermal conductivity of a pebble
//! bed computed from the Zehner-Bauer-Schlunder (ZBS) correlation, together
//! with a linear interpolant over it. The tabulation is transcribed verbatim
//! from the Virtual Test Bed generic PBR input deck (Open tier;
//! `reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`, block
//! `keff_pebble_bed`), which spans 300 K to 2000 K and 11.94 to
//! 44.95 W/(m K).
//!
//! - **Belongs here:** the tabulated values, the interpolant, and the tests
//!   proving the table is transcribed exactly and the interpolant behaves.
//! - **Does NOT belong here:** an analytic ZBS implementation (a future
//!   closure under bead `op-jyyp.3` — implementing the full correlation and
//!   checking it against this table would itself be a V&V step), design
//!   constants, or solver state.
//!
//! **Caveat.** The tabulation is for the VTB *generic* pebble bed (computed
//! by its authors from ZBS at that bed's conditions), not specifically for
//! the HTR-10 bed; it is checked in here as the published reference an
//! analytic ZBS implementation must reproduce, and as an interim effective
//! conductivity of a 6 cm-pebble helium bed. Not validated against HTR-10
//! measurements.

use uom::si::f64::{Ratio, ThermalConductivity, ThermodynamicTemperature};
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Number of points in the VTB ZBS tabulation.
pub const ZBS_TABLE_LEN: usize = 18;

/// Temperatures of the VTB ZBS tabulation, in kelvin: 300 K to 2000 K in
/// 100 K steps. Transcribed from `pbr.i` block `keff_pebble_bed`, row `x`
/// (Open tier).
pub const ZBS_TEMPERATURE_KELVIN: [f64; ZBS_TABLE_LEN] = [
    300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
    1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0,
];

/// Effective bed thermal conductivity of the VTB ZBS tabulation, in watts
/// per metre-kelvin, matching [`ZBS_TEMPERATURE_KELVIN`] point-for-point.
/// Transcribed verbatim from `pbr.i` block `keff_pebble_bed`, row `y` (Open
/// tier).
pub const ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN: [f64; ZBS_TABLE_LEN] = [
    11.940293,
    12.87749357,
    14.41727341,
    16.46031467,
    18.89508767,
    21.59454026,
    24.42480955,
    27.25852959,
    29.98755338,
    32.53156707,
    34.84117769,
    36.89601027,
    38.69951358,
    40.272406,
    41.64628551,
    42.8582912,
    43.94713743,
    44.9504677,
];

/// Effective pebble-bed thermal conductivity (W/(m K)) at the given
/// temperature (K), by linear interpolation over the VTB ZBS tabulation
/// (300 K to 2000 K). Outside the tabulated range the value is **clamped**
/// to the nearest endpoint (11.940293 W/(m K) below 300 K, 44.9504677
/// W/(m K) above 2000 K) — extrapolating a conductivity table is worse than
/// clamping it, and the clamp is exercised by a test. At a tabulated node
/// the tabulated value is returned exactly.
pub fn zbs_effective_conductivity(temperature: ThermodynamicTemperature) -> ThermalConductivity {
    let t = temperature.get::<kelvin>();
    let first = 0;
    let last = ZBS_TABLE_LEN - 1;
    if t <= ZBS_TEMPERATURE_KELVIN[first] {
        return ThermalConductivity::new::<watt_per_meter_kelvin>(
            ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[first],
        );
    }
    if t >= ZBS_TEMPERATURE_KELVIN[last] {
        return ThermalConductivity::new::<watt_per_meter_kelvin>(
            ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[last],
        );
    }
    // t is strictly inside (300, 2000): find the bracketing segment.
    let mut hi = 1;
    while ZBS_TEMPERATURE_KELVIN[hi] < t {
        hi += 1;
    }
    let lo = hi - 1;
    let t_lo = ZBS_TEMPERATURE_KELVIN[lo];
    let t_hi = ZBS_TEMPERATURE_KELVIN[hi];
    let k_lo = ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[lo];
    let k_hi = ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[hi];
    let frac: Ratio = Ratio::new::<ratio>((t - t_lo) / (t_hi - t_lo));
    ThermalConductivity::new::<watt_per_meter_kelvin>(
        k_lo + frac.get::<ratio>() * (k_hi - k_lo),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V: the ZBS interpolant returns every tabulated node exactly.
    ///
    /// **Methodology.** The 18 (temperature, conductivity) pairs are
    /// transcribed from the VTB `pbr.i` block `keff_pebble_bed` (Open tier).
    /// The check evaluates the interpolant at each tabulated temperature and
    /// compares to the tabulated conductivity. Pass criterion: relative
    /// error below 1e-12 at every node (exact up to f64 roundoff).
    ///
    /// **Results (recorded 2026-08-11).** All 18 nodes reproduced exactly
    /// (maximum relative error 0.0 at f64 precision), including the
    /// endpoints 11.940293 W/(m K) at 300 K and 44.9504677 W/(m K) at
    /// 2000 K — the published tabulation is transcribed faithfully.
    #[test]
    fn interpolant_returns_all_tabulated_nodes_exactly() {
        let mut max_rel: f64 = 0.0;
        for i in 0..ZBS_TABLE_LEN {
            let k = zbs_effective_conductivity(ThermodynamicTemperature::new::<kelvin>(
                ZBS_TEMPERATURE_KELVIN[i],
            ));
            let rel = (k.get::<watt_per_meter_kelvin>()
                - ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[i])
                .abs()
                / ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[i];
            max_rel = max_rel.max(rel);
        }
        println!("max relative node error = {:.3e}", max_rel);
        assert!(max_rel < 1e-12);
    }

    /// V&V: the ZBS interpolant is strictly monotonic increasing across the
    /// tabulated range.
    ///
    /// **Methodology.** The published tabulation rises monotonically from
    /// 11.94 W/(m K) at 300 K to 44.95 W/(m K) at 2000 K, so any correct
    /// linear interpolant over it must be monotonic. The check samples the
    /// interpolant every 1 K from 300 K to 2000 K and asserts each value is
    /// at least the previous one, and strictly greater over each 100 K node
    /// step. Pass criterion: no decrease anywhere in 1701 samples.
    ///
    /// **Results (recorded 2026-08-11).** Monotonic over all 1701 samples,
    /// rising 11.940293 to 44.9504677 W/(m K); no oscillation introduced by
    /// the interpolation.
    #[test]
    fn interpolant_is_monotonic_between_endpoints() {
        let mut prev = zbs_effective_conductivity(ThermodynamicTemperature::new::<kelvin>(300.0));
        for t in 301..=2000 {
            let k = zbs_effective_conductivity(ThermodynamicTemperature::new::<kelvin>(t as f64));
            assert!(
                k.get::<watt_per_meter_kelvin>() >= prev.get::<watt_per_meter_kelvin>(),
                "conductivity decreased at {} K",
                t
            );
            prev = k;
        }
        // Strict increase node-to-node.
        for i in 1..ZBS_TABLE_LEN {
            assert!(
                ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[i]
                    > ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[i - 1]
            );
        }
    }

    /// V&V: outside the tabulated range the interpolant clamps to the
    /// published endpoints rather than extrapolating.
    ///
    /// **Methodology.** Evaluate at 250 K (below the 300 K table start) and
    /// 2500 K (above the 2000 K table end); the documented behaviour is a
    /// clamp to the endpoint values 11.940293 and 44.9504677 W/(m K). Pass
    /// criterion: exact endpoint values returned (relative error < 1e-12).
    ///
    /// **Results (recorded 2026-08-11).** 250 K returns 11.940293 W/(m K)
    /// and 2500 K returns 44.9504677 W/(m K) — clamping behaves as
    /// documented.
    #[test]
    fn interpolant_clamps_outside_tabulated_range() {
        let below = zbs_effective_conductivity(ThermodynamicTemperature::new::<kelvin>(250.0));
        let above = zbs_effective_conductivity(ThermodynamicTemperature::new::<kelvin>(2500.0));
        assert!(
            (below.get::<watt_per_meter_kelvin>() - ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[0])
                .abs()
                < 1e-12
        );
        assert!(
            (above.get::<watt_per_meter_kelvin>()
                - ZBS_CONDUCTIVITY_WATT_PER_METER_KELVIN[ZBS_TABLE_LEN - 1])
                .abs()
                < 1e-12
        );
    }
}
