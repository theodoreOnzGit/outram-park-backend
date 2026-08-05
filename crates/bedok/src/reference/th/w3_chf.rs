//! W-3 critical heat flux correlation and departure-from-nucleate-boiling
//! ratio.
//!
//! # Provenance
//!
//! Translated from `w3chf.m` and `w3chfhottest.m` by **Than Yan Ren**
//! (Singapore Nuclear Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; the two
//! defects noted below are recorded, not repaired.
//!
//! # The correlation
//!
//! Tong's W-3 correlation for critical heat flux in a uniformly heated
//! channel, written as a product of four factors:
//!
//! ```text
//! q_CHF = K1 * K2 * K3 * K4 / 10                       [W/cm^2]
//!
//! K1 = (2.022 - 0.06238 p) + (0.1722 - 0.01427 p) exp[(18.177 - 0.5987 p) X]
//! K2 = (0.1484 - 1.596 X + 0.1729 X|X|) * 2.326 * G * 10 + 3271
//! K3 = (1.157 - 0.869 X) * (0.2664 + 0.8357 exp(-124.1 De/100))
//! K4 = 0.8258 + 0.0003413 (h_f - h_in)
//! ```
//!
//! The published W-3 constants assume psia, `lb/(hr·ft²)`, inches and Btu/lb;
//! the constants above are the same correlation rewritten for **MPa,
//! g/(s·cm²), cm and kJ/kg**, giving `K1*K2*K3*K4` in kW/m² and hence the
//! final `/10` to reach W/cm². (For example `0.0004302 psia⁻¹ × 145.038 =
//! 0.06238 MPa⁻¹`, and `0.000794 (Btu/lb)⁻¹ ÷ 2.326 = 0.0003413 (kJ/kg)⁻¹`.)
//!
//! # Validity of W-3
//!
//! The published range is roughly 5.5–16 MPa, mass flux
//! 136–6800 kg/(m²·s) (13.6–680 g/(s·cm²)), quality −0.15 to +0.15, hydraulic
//! diameter 0.5–1.8 cm, and inlet subcooling below about 660 kJ/kg. Neither
//! the MATLAB nor this translation checks any of that — the correlation is
//! evaluated wherever it is called.
//!
//! # Defects carried over from the MATLAB
//!
//! Recorded, not repaired, per `docs/bedok-port-scoping.md` §1.0. See
//! [`critical_heat_flux`] and [`hottest_channel`].
//!
//! # Not translated
//!
//! `w3chf.m` ends with three unconditional `writematrix` calls dumping
//! `chf.csv`, `dnbr.csv` and `chfheatflux.csv` into the working directory.
//! Those are diagnostics with no effect on any returned value and are not
//! translated.

use super::steam;
use super::{fix_inf_nan, ThGeometry, ThermalHydraulicState};
use crate::reference::grid::Grid;

/// Critical heat flux and DNB ratio over the nodes the correlation was
/// evaluated on. MATLAB `chf` struct.
#[derive(Debug, Clone, PartialEq)]
pub struct CriticalHeatFluxResult {
    /// Predicted critical heat flux \[W/cm²\] at each node. MATLAB `chf.chf`.
    pub critical_heat_flux: Vec<f64>,
    /// Departure-from-nucleate-boiling ratio \[-\], `q_CHF / q_wall`, with
    /// non-finite entries (zero wall heat flux) replaced by zero.
    /// MATLAB `chf.dnbr`.
    pub dnbr: Vec<f64>,
}

/// The W-3 correlation evaluated at one point.
///
/// Factored out of the vectorised MATLAB expression so it can be checked
/// against a hand-worked value; the arithmetic is identical.
///
/// # Arguments
///
/// - `pressure_mpa` — \[MPa\] local coolant pressure.
/// - `quality` — \[-\] local equilibrium steam quality; may be negative
///   (subcooled) in the published correlation, although this code path is fed
///   the 0–1 clamped quality by the channel model.
/// - `mass_flux` — \[g/(s·cm²)\] local mixture mass flux `rho_m * v_m`.
/// - `hydraulic_diameter_cm` — \[cm\] subchannel hydraulic diameter.
/// - `subcooling` — \[kJ/kg\] `h_f(p) - h_in`, the enthalpy rise still
///   available to saturation.
///
/// # Returns
///
/// Critical heat flux in **W/cm²**.
#[must_use]
pub fn w3_correlation(
    pressure_mpa: f64,
    quality: f64,
    mass_flux: f64,
    hydraulic_diameter_cm: f64,
    subcooling: f64,
) -> f64 {
    let k1 = (2.022 - 0.06238 * pressure_mpa)
        + (0.1722 - 0.01427 * pressure_mpa) * ((18.177 - 0.5987 * pressure_mpa) * quality).exp();
    let k2 =
        (0.1484 - 1.596 * quality + 0.1729 * quality * quality.abs()) * 2.326 * mass_flux * 10.0
            + 3271.0;
    let k3 = (1.157 - 0.869 * quality)
        * (0.2664 + 0.8357 * (-124.1 * hydraulic_diameter_cm / 100.0).exp());
    let k4 = 0.8258 + 0.000_341_3 * subcooling;
    k1 * k2 * k3 * k4 / 10.0
}

/// Predict the critical heat flux and DNB ratio at every node of `th`.
///
/// MATLAB `w3chf(geometry, th)`.
///
/// # Arguments
///
/// - `geometry` — only `geometry.fuel.hydraulic_diameter` \[cm\] is used.
///   (The MATLAB also reads `subarea` and defines a gravity constant; neither
///   is used, and neither is translated.)
/// - `th` — the coolant state. Reads `heat_flux` \[W/cm²\], `coolant.pressure`
///   \[MPa\], `coolant.void_fraction`, `coolant.mixture_velocity` \[cm/s\],
///   `coolant.liquid_density` and `coolant.gas_density` \[g/cm³\],
///   `coolant.enthalpy` \[kJ/kg\], `coolant.quality`,
///   `coolant.inlet_temperature` \[K\] and `coolant.inlet_pressure` \[MPa\].
///
/// # Returns
///
/// One [`CriticalHeatFluxResult`] entry per node of the state passed in.
///
/// # Defects carried over from the MATLAB
///
/// 1. **`enthshift` is not the inlet enthalpy the W-3 `K4` factor calls for.**
///    W-3's `K4` uses `h_f - h_in` with `h_in` the **channel inlet**
///    enthalpy. The MATLAB instead builds
///    `enthshift(i) = (0.5*enth(i) + 0.5*enth(i-1))/2` — a *local* two-node
///    average, halved again by a stray outer `/2`. Only `enthshift(1)` is the
///    inlet enthalpy. The halving alone roughly doubles the apparent
///    subcooling and so inflates `K4`.
/// 2. **The `i-1` walk runs over the flat node index, not along a channel.**
///    Because `iz` varies fastest in the state vector, `enth(i-1)` is the node
///    below within a channel, but at every channel boundary it is the *top of
///    the previous channel*. The first node of each channel therefore mixes
///    two unrelated channels' enthalpies.
///
/// Both are left exactly as written.
#[must_use]
pub fn critical_heat_flux(
    geometry: &ThGeometry,
    th: &ThermalHydraulicState,
) -> CriticalHeatFluxResult {
    let nodes = th.coolant.enthalpy.len();
    let hydraulic_diameter = geometry.fuel.hydraulic_diameter;

    let inlet_enthalpy =
        steam::enthalpy_region1_pt(th.coolant.inlet_pressure, th.coolant.inlet_temperature);

    // Defect 1 and 2 above: reproduced verbatim.
    let mut enthalpy_shift = vec![0.0; nodes];
    if nodes > 0 {
        enthalpy_shift[0] = inlet_enthalpy;
    }
    for i in 1..nodes {
        enthalpy_shift[i] = (0.5 * th.coolant.enthalpy[i] + 0.5 * th.coolant.enthalpy[i - 1]) / 2.0;
    }

    let mut critical = vec![0.0; nodes];
    let mut dnbr = vec![0.0; nodes];
    for i in 0..nodes {
        let pressure = th.coolant.pressure[i];
        let quality = th.coolant.quality[i];
        let mass_flux = (th.coolant.void_fraction[i] * th.coolant.gas_density[i]
            + (1.0 - th.coolant.void_fraction[i]) * th.coolant.liquid_density[i])
            * th.coolant.mixture_velocity[i];
        let saturated_liquid_enthalpy = steam::saturated_liquid_enthalpy(pressure);
        critical[i] = w3_correlation(
            pressure,
            quality,
            mass_flux,
            hydraulic_diameter,
            saturated_liquid_enthalpy - enthalpy_shift[i],
        );
        dnbr[i] = critical[i] / th.heat_flux[i];
    }

    CriticalHeatFluxResult {
        dnbr: fix_inf_nan(&dnbr),
        critical_heat_flux: critical,
    }
}

/// Evaluate the W-3 correlation over the axially hottest channel only.
///
/// MATLAB `w3chfhottest(params, geometry, th)`. The "hottest" channel is the
/// one whose **whole axial column** of wall heat flux sums highest.
///
/// # Arguments
///
/// - `grid` — the node grid, for the channel sweep.
/// - `geometry` — as [`critical_heat_flux`].
/// - `th` — the full-core coolant state.
///
/// # Returns
///
/// A [`CriticalHeatFluxResult`] with `grid.nz` entries — one per axial node of
/// the selected channel, bottom first.
///
/// # Defect carried over from the MATLAB
///
/// **`w3chfhottest.m:21` sets `highy = ix`, not `iy`.** The `y` index of the
/// hottest channel is therefore overwritten with its `x` index, so the channel
/// actually evaluated is `(ix, ix)` — the diagonal — rather than the one whose
/// heat flux was measured. For a symmetric quarter-core layout the two often
/// coincide, which is presumably why it went unnoticed. Reproduced verbatim.
///
/// Note also that the MATLAB slices `th` but leaves `th.coolant.inletpress`
/// and `inlettemp` at their full-core values, which is correct — they are
/// scalars.
#[must_use]
pub fn hottest_channel(
    grid: &Grid,
    geometry: &ThGeometry,
    th: &ThermalHydraulicState,
) -> CriticalHeatFluxResult {
    let mut high_x = 0usize;
    let mut high_y = 0usize;
    let mut highest = 0.0f64;

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let column_total: f64 = (0..grid.nz)
                .map(|iz| th.heat_flux[grid.index(0, ix, iy, iz)])
                .sum();
            if column_total > highest {
                highest = column_total;
                high_x = ix;
                // Defect: the MATLAB writes `highy = ix` here.
                high_y = ix;
            }
        }
    }

    let indices: Vec<usize> = (0..grid.nz)
        .map(|iz| grid.index(0, high_x, high_y, iz))
        .collect();

    let mut sub = th.clone();
    sub.heat_flux = gather(&th.heat_flux, &indices);
    sub.coolant.pressure = gather(&th.coolant.pressure, &indices);
    sub.coolant.void_fraction = gather(&th.coolant.void_fraction, &indices);
    sub.coolant.mixture_velocity = gather(&th.coolant.mixture_velocity, &indices);
    sub.coolant.liquid_density = gather(&th.coolant.liquid_density, &indices);
    sub.coolant.gas_density = gather(&th.coolant.gas_density, &indices);
    sub.coolant.enthalpy = gather(&th.coolant.enthalpy, &indices);
    sub.coolant.quality = gather(&th.coolant.quality, &indices);

    critical_heat_flux(geometry, &sub)
}

fn gather(values: &[f64], indices: &[usize]) -> Vec<f64> {
    indices.iter().map(|&i| values[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::super::single_flow_evap::test_support::single_channel_rig;
    use super::*;

    /// The W-3 correlation against a hand-worked point.
    ///
    /// **Methodology.** Evaluate [`w3_correlation`] at
    /// `p = 15.5 MPa`, `X = 0` (saturated liquid), `G = 350 g/(s·cm²)`,
    /// `De = 1.17 cm`, `h_f - h_in = 500 kJ/kg` — a representative PWR
    /// subchannel state well inside W-3's published validity range. Worked by
    /// hand from the four factors:
    ///
    /// ```text
    /// K1 = (2.022 - 0.06238*15.5) + (0.1722 - 0.01427*15.5)*exp(0)
    ///    = 1.055110 + (-0.048985)              = 1.006125
    /// K2 = 0.1484 * 2.326 * 350 * 10 + 3271    = 4479.124
    /// K3 = 1.157 * (0.2664 + 0.8357*exp(-1.45197))
    ///    = 1.157 * 0.462045                    = 0.534586
    /// K4 = 0.8258 + 0.0003413*500              = 0.996450
    /// q  = K1*K2*K3*K4 / 10                    = 240.1 W/cm^2
    /// ```
    ///
    /// Pass criterion: within 0.5 W/cm² of the hand-worked 240.1 W/cm².
    ///
    /// **Result (2026-08-05).** 240.06 W/cm², i.e. 0.04 W/cm² from the
    /// hand-worked value — the residual is the rounding in the hand
    /// arithmetic, not in the code. Interpretation: the SI-rescaled constants
    /// are transcribed correctly and the factor grouping matches the published
    /// W-3 form. `2.4 MW/m²` is also the right order of magnitude for PWR CHF
    /// at these conditions, so no unit slip survives.
    #[test]
    fn w3_matches_a_hand_worked_pwr_point() {
        let got = w3_correlation(15.5, 0.0, 350.0, 1.17, 500.0);
        assert!(
            (got - 240.1).abs() < 0.5,
            "got {got} W/cm2, hand-worked 240.1 W/cm2"
        );
    }

    /// Each factor separately reproduces its hand-worked value, so a
    /// transcription error in any one of the four is localised rather than
    /// hidden by cancellation in the product.
    #[test]
    fn each_w3_factor_matches_its_hand_worked_value() {
        let p: f64 = 15.5;
        let x: f64 = 0.0;
        let g: f64 = 350.0;
        let de: f64 = 1.17;
        let subcooling: f64 = 500.0;

        let k1 = (2.022 - 0.06238 * p) + (0.1722 - 0.01427 * p) * ((18.177 - 0.5987 * p) * x).exp();
        let k2 = (0.1484 - 1.596 * x + 0.1729 * x * x.abs()) * 2.326 * g * 10.0 + 3271.0;
        let k3 = (1.157 - 0.869 * x) * (0.2664 + 0.8357 * (-124.1 * de / 100.0).exp());
        let k4 = 0.8258 + 0.000_341_3 * subcooling;

        assert!((k1 - 1.006125).abs() < 1e-6, "K1 = {k1}");
        assert!((k2 - 4479.124).abs() < 1e-2, "K2 = {k2}");
        assert!((k3 - 0.534586).abs() < 1e-6, "K3 = {k3}");
        assert!((k4 - 0.996450).abs() < 1e-6, "K4 = {k4}");
        assert!((w3_correlation(p, x, g, de, subcooling) - k1 * k2 * k3 * k4 / 10.0).abs() < 1e-9);
    }

    /// CHF falls as quality rises — the qualitative behaviour W-3 exists to
    /// capture.
    #[test]
    fn chf_falls_with_rising_quality() {
        let dry = w3_correlation(15.5, 0.20, 350.0, 1.17, 500.0);
        let wet = w3_correlation(15.5, 0.0, 350.0, 1.17, 500.0);
        assert!(dry < wet, "CHF at X=0.2 is {dry}, at X=0 is {wet}");
    }

    /// CHF rises with mass flux at fixed quality.
    #[test]
    fn chf_rises_with_mass_flux() {
        let slow = w3_correlation(15.5, 0.0, 200.0, 1.17, 500.0);
        let fast = w3_correlation(15.5, 0.0, 500.0, 1.17, 500.0);
        assert!(fast > slow, "CHF at G=500 is {fast}, at G=200 is {slow}");
    }

    /// A zero wall heat flux gives a zeroed DNBR rather than an infinity.
    #[test]
    fn zero_wall_heat_flux_gives_a_zeroed_dnbr() {
        let (_, params, geometry, mut state, power_density) = single_channel_rig(6);
        super::super::single_flow_evap::solve_static(
            &params,
            &geometry,
            &mut state,
            &power_density,
        )
        .expect("marches");
        // heat_flux is still all zeros from the rig.
        let result = critical_heat_flux(&geometry, &state);
        assert!(result.dnbr.iter().all(|v| *v == 0.0), "{:?}", result.dnbr);
        assert!(result.critical_heat_flux.iter().all(|v| *v > 0.0));
    }

    /// The hottest-channel selector returns one entry per axial node.
    #[test]
    fn hottest_channel_returns_one_entry_per_axial_node() {
        let (grid, params, geometry, mut state, power_density) = single_channel_rig(6);
        super::super::single_flow_evap::solve_static(
            &params,
            &geometry,
            &mut state,
            &power_density,
        )
        .expect("marches");
        state.heat_flux = vec![40.0; grid.nodes()];
        let result = hottest_channel(&grid, &geometry, &state);
        assert_eq!(result.critical_heat_flux.len(), grid.nz);
        assert_eq!(result.dnbr.len(), grid.nz);
        assert!(result.dnbr.iter().all(|v| *v > 0.0), "{:?}", result.dnbr);
    }
}
