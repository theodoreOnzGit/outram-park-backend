// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149), not a port. This file is part of OUTRAM PARK,
// distributed under GPL-3.0 like the rest of this crate's GeN-Foam module.

//! Thin gas gap between two solid regions: gas conduction plus two-surface
//! radiation, as one conductance on the mesh faces between the regions.
//!
//! **New work, not port (gh:#1149).**
//!
//! ## The formula and its source
//!
//! Kugeler, Nabielek & Buckthorpe (2017), *The High Temperature Gas-cooled
//! Reactor: Safety considerations of the (V)HTR-Modul*, EUR 28712 EN
//! (open; `crates/kovan-literature/reactor-literature/kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf`),
//! §9.4, writes the heat flux across a gap in the reactor internals as
//! `q'' = (alpha_conv + alpha_rad)(T1 - T2)` (their p. 205), with the radiative
//! part for parallel flat surfaces carrying the exchange factor
//! `C12 = Cs / (1/eps1 + 1/eps2 - 1)` (their eq. 115). Written as a
//! conductance, the net grey-body exchange between two large parallel
//! surfaces is
//!
//! ```text
//!   q''_rad = sigma (T1^4 - T2^4) / (1/eps1 + 1/eps2 - 1)
//!   h_rad   = sigma (T1^2 + T2^2)(T1 + T2) / (1/eps1 + 1/eps2 - 1)
//! ```
//!
//! which is exact (the factorisation `T1^4 - T2^4 = (T1^2+T2^2)(T1+T2)(T1-T2)`),
//! not linearised. A stagnant gas layer of width `delta` adds
//! `h_gas = lambda_gas(T_mean) / delta` (pure conduction; Kugeler's
//! `alpha_conv` includes free convection, which this model does not).
//!
//! **Two notes on the source, for review.** (1) Kugeler quotes
//! `Cs = 5.77 W/(m^2 K^4)` (in the `(T/100)^4` form, i.e. `sigma = 5.77e-8`);
//! CODATA 2018 gives `sigma = 5.670374419e-8`, which is used here (the same
//! constant as the ported `blackbody_radiation` BC). The 1.8 % difference
//! looks like a transcription slip in the report and is **not re-checked**
//! against the report's primary source (Schürenkremer 1983). (2) For
//! concentric cylinders Kugeler's eq. 125 replaces the factor by
//! `1/eps1 + (A1/A2)(1/eps2 - 1)`; a gap carried on one mesh face has one area,
//! so `A1/A2 = 1` and the parallel-plate form applies. That is the thin-gap
//! limit `delta << r`.
//!
//! ## How the solver uses it
//!
//! On a face between a cell of region `a` and one of region `b` the
//! conductance is the series sum
//! `G = A / (d_P/lambda_P + 1/h_gap + d_N/lambda_N)`, with `h_gap` evaluated at
//! the two **surface** temperatures of the previous Picard iterate; at
//! convergence the radiative exchange is exact.

use uom::si::f64::{HeatTransfer, Length, Ratio, ThermodynamicTemperature};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use super::properties::ConductivityModel;
use super::SolidConductionError;
use crate::genfoam::thermal_hydraulics::boundary_conditions::blackbody_radiation::STEFAN_BOLTZMANN_SIGMA;

/// Gas conduction plus grey two-surface radiation across a thin gap.
#[derive(Debug, Clone)]
pub struct GapConductance {
    eps_a: f64,
    eps_b: f64,
    gas: ConductivityModel,
    width: f64,
}

impl GapConductance {
    /// A gap of the given width filled with a gas of conductivity
    /// `gas_conductivity` (use a constant zero for a vacuum), between a
    /// surface of emissivity `emissivity_a` (on region `a`) and one of
    /// `emissivity_b`.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for an emissivity outside
    /// `(0, 1]` or a non-positive width.
    pub fn new(
        emissivity_a: Ratio,
        emissivity_b: Ratio,
        gas_conductivity: ConductivityModel,
        width: Length,
    ) -> Result<Self, SolidConductionError> {
        let (ea, eb, w) = (
            emissivity_a.get::<ratio>(),
            emissivity_b.get::<ratio>(),
            width.get::<meter>(),
        );
        for e in [ea, eb] {
            if !(e > 0.0 && e <= 1.0) {
                return Err(SolidConductionError::InvalidInput(format!(
                    "gap emissivity must be in (0, 1], got {e}"
                )));
            }
        }
        if !(w > 0.0) || !w.is_finite() {
            return Err(SolidConductionError::InvalidInput(format!(
                "gap width must be positive, got {w} m"
            )));
        }
        Ok(Self {
            eps_a: ea,
            eps_b: eb,
            gas: gas_conductivity,
            width: w,
        })
    }

    /// `h_gas + h_rad` at surface temperatures `t_a`, `t_b`.
    #[must_use]
    pub fn coefficient(&self, t_a: ThermodynamicTemperature, t_b: ThermodynamicTemperature) -> HeatTransfer {
        HeatTransfer::new::<watt_per_square_meter_kelvin>(
            self.coefficient_si(t_a.get::<kelvin>(), t_b.get::<kelvin>()),
        )
    }

    /// The radiative part alone, `h_rad`, at `t_a`, `t_b`.
    #[must_use]
    pub fn radiative_coefficient(
        &self,
        t_a: ThermodynamicTemperature,
        t_b: ThermodynamicTemperature,
    ) -> HeatTransfer {
        HeatTransfer::new::<watt_per_square_meter_kelvin>(
            self.radiative_si(t_a.get::<kelvin>(), t_b.get::<kelvin>()),
        )
    }

    fn radiative_si(&self, ta: f64, tb: f64) -> f64 {
        STEFAN_BOLTZMANN_SIGMA * (ta * ta + tb * tb) * (ta + tb)
            / (1.0 / self.eps_a + 1.0 / self.eps_b - 1.0)
    }

    pub(crate) fn coefficient_si(&self, ta: f64, tb: f64) -> f64 {
        self.gas.0.value(0.5 * (ta + tb)) / self.width + self.radiative_si(ta, tb)
    }

    /// The same gap seen from the other side (emissivities swapped).
    #[must_use]
    pub(crate) fn swapped(&self) -> Self {
        Self {
            eps_a: self.eps_b,
            eps_b: self.eps_a,
            gas: self.gas.clone(),
            width: self.width,
        }
    }
}

/// A [`GapConductance`] on every mesh face between a cell of `region_a` and
/// a cell of `region_b` (either orientation).
#[derive(Debug, Clone)]
pub struct RegionGap {
    /// One side.
    pub region_a: usize,
    /// The other side.
    pub region_b: usize,
    /// The gap (its `emissivity_a` belongs to `region_a`).
    pub gap: GapConductance,
}
