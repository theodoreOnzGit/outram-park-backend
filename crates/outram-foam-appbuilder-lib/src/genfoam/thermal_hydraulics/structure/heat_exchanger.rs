// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             heatExchanger/heatExchanger.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `heat_exchanger` — two-sided wall coupling of primary and secondary fluids
//!
//! Port of the **flux-conservation wall-temperature solve** at the heart of
//! GeN-Foam's `heatExchanger::correct`. A heat exchanger couples a primary and a
//! secondary fluid region across a conducting tube wall of series conductance
//! `H_w = k_wall / t_wall` ([`WallConductance`]). GeN-Foam requires the heat
//! flux through the wall to equal the convective flux into each fluid, giving a
//! 2×2 linear system for the two wall-surface temperatures `T_p`, `T_s`:
//!
//! ```text
//! T_s = A_p*T_p - B_p ,   T_p = A_s*T_s - B_s
//! ```
//!
//! with, on each side `i` (`p` or `s`),
//!
//! ```text
//! A_i = (H_w + h_sum_i) / H_w        (dimensionless)
//! B_i = ht_sum_i / H_w               (temperature)
//! ```
//!
//! where `h_sum_i`/`ht_sum_i` are that side's fluid-coupling sums (the same
//! `H`/`HT` pair used everywhere else in this module — see
//! [`super::heat_source`]). The closed-form solution is
//!
//! ```text
//! T_p = (B_p*A_s + B_s) / (A_p*A_s - 1)
//! T_s = (B_s*A_p + B_p) / (A_s*A_p - 1)
//! ```
//!
//! guarded against the degenerate `A_p*A_s = 1` case (both convective
//! coefficients zero ⇒ no heat exchanged) exactly as upstream, with a `1e-69`
//! floor on the denominator.
//!
//! ## Scope
//!
//! [`hx_wall_temperature`] ports this per-cell algebra, which is self-contained
//! and hand-verifiable. GeN-Foam additionally handles the **mesh-to-mesh
//! mapping** between (possibly non-conformal) primary and secondary meshes to
//! find the "opposite" cell's `h_sum`/`ht_sum`; that mapping is inseparable from
//! the finite-volume mesh machinery and belongs to the solver bead
//! (op-p6p.7.11). Here the caller supplies both sides' already-mapped coupling
//! sums.

use uom::si::f64::ThermodynamicTemperature;

use super::units::{HeatFlux, HeatTransferCoefficient, WallConductance};

/// A heat-exchanger wall: its series conductance.
///
/// Carries the tube-wall conductance `H_w = k_wall / t_wall`. Evaluate the
/// wall-surface temperatures with [`HeatExchanger::wall_temperature`], which
/// wraps [`hx_wall_temperature`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatExchanger {
    /// Wall conductance `H_w` [W/(m^2 K)].
    wall_conductance: WallConductance,
}

impl HeatExchanger {
    /// Build a heat exchanger from its wall conductance `H_w = k_wall/t_wall`.
    #[must_use]
    pub fn new(wall_conductance: WallConductance) -> Self {
        Self { wall_conductance }
    }

    /// Wall conductance `H_w` [W/(m^2 K)].
    #[must_use]
    pub fn wall_conductance(&self) -> WallConductance {
        self.wall_conductance
    }

    /// The **primary-side** wall-surface temperature `T_p` [K] for this cell.
    ///
    /// Convenience wrapper over [`hx_wall_temperature`] using this exchanger's
    /// wall conductance. Pass the primary side's coupling sums first, then the
    /// secondary side's (already mapped to this cell).
    #[must_use]
    pub fn wall_temperature(
        &self,
        h_sum_primary: HeatTransferCoefficient,
        ht_sum_primary: HeatFlux,
        h_sum_secondary: HeatTransferCoefficient,
        ht_sum_secondary: HeatFlux,
    ) -> ThermodynamicTemperature {
        hx_wall_temperature(
            self.wall_conductance,
            h_sum_primary,
            ht_sum_primary,
            h_sum_secondary,
            ht_sum_secondary,
        )
    }
}

/// Primary-side heat-exchanger wall-surface temperature `T_p` [K].
///
/// Port of the primary loop of `heatExchanger::correct`:
/// `T_p = (B_p*A_s + B_s) / max(A_p*A_s - 1, 1e-69)` (see the [module
/// documentation](self) for the derivation and the `A_i`, `B_i` definitions).
/// The `1e-69` floor reproduces upstream's guard against the no-heat-transfer
/// degeneracy where both convective coefficients vanish.
#[must_use]
pub fn hx_wall_temperature(
    wall_conductance: WallConductance,
    h_sum_primary: HeatTransferCoefficient,
    ht_sum_primary: HeatFlux,
    h_sum_secondary: HeatTransferCoefficient,
    ht_sum_secondary: HeatFlux,
) -> ThermodynamicTemperature {
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    let hw = wall_conductance.get::<watt_per_square_meter_kelvin>();
    let hp = h_sum_primary.get::<watt_per_square_meter_kelvin>();
    let hs = h_sum_secondary.get::<watt_per_square_meter_kelvin>();
    let htp = ht_sum_primary.get::<watt_per_square_meter>();
    let hts = ht_sum_secondary.get::<watt_per_square_meter>();

    let a_p = (hw + hp) / hw;
    let b_p = htp / hw;
    let a_s = (hw + hs) / hw;
    let b_s = hts / hw;

    let t_p = (b_p * a_s + b_s) / (a_p * a_s - 1.0).max(1e-69);
    ThermodynamicTemperature::new::<kelvin>(t_p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    fn hw(x: f64) -> WallConductance {
        WallConductance::new::<watt_per_square_meter_kelvin>(x)
    }
    fn h(x: f64) -> HeatTransferCoefficient {
        HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(x)
    }
    fn ht(coeff: f64, temp: f64) -> HeatFlux {
        HeatFlux::new::<watt_per_square_meter>(coeff * temp)
    }

    /// V&V — symmetric heat exchanger sits the wall midway.
    ///
    /// Methodology: with equal convective coefficients on both sides
    /// (`h_p = h_s = h`), symmetry demands the primary wall-surface temperature
    /// be the average of the two fluid temperatures. Derivation: A_p = A_s = A,
    /// B_p = A_h*T_p_fluid where B_i = h*T_i/H_w. With h = H_w = 1.0e4,
    /// T_p_fluid = 600 K, T_s_fluid = 400 K:
    /// A = (1e4+1e4)/1e4 = 2, B_p = 1e4*600/1e4 = 600, B_s = 400.
    /// T_p = (600*2 + 400)/(2*2 - 1) = 1600/3 = 533.333 K.
    /// Cross-check T_s = (400*2 + 600)/3 = 1400/3 = 466.667 K; the wall drop
    /// T_p - T_s = 66.667 K and the mean (T_p+T_s)/2 = 500 K = mean fluid temp.
    ///
    /// Result (2026-07-15): T_p = 533.333 K (|error| < 1e-3), mean of T_p and
    /// the analytic T_s = 500.000 K. Pass.
    #[test]
    fn symmetric_hx_wall_temperature() {
        let t_p = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 600.0),
            h(1.0e4),
            ht(1.0e4, 400.0),
        );
        assert!((t_p.get::<kelvin>() - 1600.0 / 3.0).abs() < 1e-3);

        // The symmetric secondary temperature (swap roles) confirms the mean.
        let t_s = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 400.0),
            h(1.0e4),
            ht(1.0e4, 600.0),
        );
        let mean = 0.5 * (t_p.get::<kelvin>() + t_s.get::<kelvin>());
        assert!((mean - 500.0).abs() < 1e-3);
    }

    /// Degenerate guard — zero convective coefficients do not divide by zero.
    ///
    /// With h_p = h_s = 0, A_p = A_s = 1 so A_p*A_s - 1 = 0; upstream floors the
    /// denominator at 1e-69. B_p = B_s = 0 too, so the result is a finite 0 K
    /// (no heat exchanged), not a NaN/inf.
    #[test]
    fn zero_coefficients_do_not_nan() {
        let t_p = hx_wall_temperature(hw(1.0e4), h(0.0), ht(0.0, 0.0), h(0.0), ht(0.0, 0.0));
        assert!(t_p.get::<kelvin>().is_finite());
    }

    #[test]
    fn struct_wrapper_matches_free_function() {
        let hx = HeatExchanger::new(hw(1.0e4));
        let via_struct =
            hx.wall_temperature(h(1.0e4), ht(1.0e4, 600.0), h(1.0e4), ht(1.0e4, 400.0));
        let via_fn = hx_wall_temperature(
            hw(1.0e4),
            h(1.0e4),
            ht(1.0e4, 600.0),
            h(1.0e4),
            ht(1.0e4, 400.0),
        );
        assert_eq!(via_struct.get::<kelvin>(), via_fn.get::<kelvin>());
    }
}
