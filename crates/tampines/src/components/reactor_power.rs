// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Reactor power source (gh:#1079; RELAP-7 Theory Manual §6.16, cited, not
//! quoted).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! [`ReactorPowerSource`] takes a total reactor power and distributes it over
//! core channels by a radial weight and, within each channel, over its cells
//! by an [`AxialPowerShape`]. The total comes from a [`TotalPower`]: a table
//! in time, or a value the caller sets each step. ~~**`tampines` does not
//! depend on `teh-o-prke`** (as `crate::pebble_bed` already states)~~
//! ~~**CORRECTED 2026-11-03 (gh:#1151):** `tampines` now depends on
//! `teh-o-prke` (maintainer decision D2: the HTR-10 coupled loop transient
//! lives here, in [`crate::htr10_primary_loop`]).~~ **CORRECTED again
//! 2026-11-03: maintainer moved coupled kinetics to nee_soon**
//! (`nee_soon::htr10_th_transient`); `tampines` is again free of
//! `teh-o-prke` outside a gated example dev-dependency. This component takes
//! no kinetics itself: power from a `PointReactor` (fission plus decay heat)
//! arrives through [`TotalPower::External`].
//!
//! The distribution is exact by construction: the chopped-cosine shape is
//! integrated cell by cell, and every set of fractions is normalised, so the
//! cell powers sum to the total to round-off. Power is deposited directly in
//! the fluid of a [`HemPipe`] through [`HemPipe::set_cell_heat_input`]; a heat
//! structure between fuel and coolant is not modelled here.

use petir::mathf::RealMath;
use tampines_steam_tables::hem::{HemEquationOfState, HemError, HemPipe, TimeTable};
use uom::si::f64::{Power, Time};
use uom::si::power::watt;
use uom::si::time::second;

/// The axial shape of the power within one channel.
#[derive(Debug, Clone, PartialEq)]
pub enum AxialPowerShape {
    /// Equal power in every cell.
    Uniform,
    /// `cos(π (z − H/2) / H_e)` over the heated length `H`, with the
    /// extrapolated length `H_e = ratio · H` (`ratio ≥ 1`; 1 is the bare
    /// cosine reaching zero at both ends).
    ChoppedCosine {
        /// `H_e / H`.
        extrapolated_length_ratio: f64,
    },
    /// A relative weight per cell (normalised; length must be the cell
    /// count, values ≥ 0 with a positive sum).
    Table(Vec<f64>),
}

impl AxialPowerShape {
    /// The fraction of the channel's power in each of `n_cells` equal cells;
    /// the fractions sum to 1.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] on a zero cell count, a ratio below 1, or a
    /// table of the wrong length or with a negative or zero-sum weight.
    pub fn cell_fractions(&self, n_cells: usize) -> Result<Vec<f64>, HemError> {
        if n_cells == 0 {
            return Err(HemError::InvalidInput("no cells".into()));
        }
        let w: Vec<f64> = match self {
            Self::Uniform => vec![1.0; n_cells],
            Self::ChoppedCosine { extrapolated_length_ratio: r } => {
                if !(r.is_finite() && *r >= 1.0) {
                    return Err(HemError::InvalidInput(format!("extrapolated length ratio {r}")));
                }
                // Exact integral over each cell of the unit-length channel.
                let k = core::f64::consts::PI / r;
                let s = |z: f64| (k * (z - 0.5)).r_sin();
                (0..n_cells)
                    .map(|i| {
                        let (z0, z1) = (i as f64 / n_cells as f64, (i + 1) as f64 / n_cells as f64);
                        (s(z1) - s(z0)) / k
                    })
                    .collect()
            }
            Self::Table(t) => {
                if t.len() != n_cells || t.iter().any(|x| !(x.is_finite() && *x >= 0.0)) {
                    return Err(HemError::InvalidInput(format!("axial table {t:?} for {n_cells} cells")));
                }
                t.clone()
            }
        };
        let sum: f64 = w.iter().sum();
        if !(sum > 0.0) {
            return Err(HemError::InvalidInput("axial shape sums to zero".into()));
        }
        Ok(w.iter().map(|x| x / sum).collect())
    }
}

/// One core channel's share of the power.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreChannelShare {
    /// Relative radial weight (normalised over the channels; ≥ 0).
    pub radial_weight: f64,
    /// Axial shape within the channel.
    pub axial_shape: AxialPowerShape,
}

/// Where the total power comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum TotalPower {
    /// A table of power \[W\] against time \[s\].
    Table(TimeTable),
    /// A value the caller sets with [`ReactorPowerSource::set_external_power`]
    /// each step, e.g. from `teh-o-prke`'s `PointReactor`.
    External(Power),
}

/// A reactor power source over a set of core channels.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactorPowerSource {
    channels: Vec<CoreChannelShare>,
    radial: Vec<f64>,
    total: TotalPower,
}

impl ReactorPowerSource {
    /// A source over `channels`, driven by `total`.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if there are no channels or the radial
    /// weights are negative or sum to zero.
    pub fn new(channels: Vec<CoreChannelShare>, total: TotalPower) -> Result<Self, HemError> {
        if channels.is_empty() || channels.iter().any(|c| !(c.radial_weight.is_finite() && c.radial_weight >= 0.0)) {
            return Err(HemError::InvalidInput("channels and their radial weights".into()));
        }
        let sum: f64 = channels.iter().map(|c| c.radial_weight).sum();
        if !(sum > 0.0) {
            return Err(HemError::InvalidInput("radial weights sum to zero".into()));
        }
        let radial = channels.iter().map(|c| c.radial_weight / sum).collect();
        Ok(Self { channels, radial, total })
    }

    /// Set the externally supplied total power (switches to
    /// [`TotalPower::External`]).
    pub fn set_external_power(&mut self, power: Power) {
        self.total = TotalPower::External(power);
    }

    /// The total power at time `t`.
    pub fn total_power(&self, t: Time) -> Power {
        match &self.total {
            TotalPower::Table(tab) => Power::new::<watt>(tab.at(t.get::<second>())),
            TotalPower::External(p) => *p,
        }
    }

    /// Each channel's power at time `t`; they sum to the total.
    pub fn channel_powers(&self, t: Time) -> Vec<Power> {
        let p = self.total_power(t);
        self.radial.iter().map(|f| p * *f).collect()
    }

    /// The power in each of channel `channel`'s `n_cells` cells at time `t`.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for an unknown channel or a bad shape.
    pub fn cell_powers(&self, channel: usize, n_cells: usize, t: Time) -> Result<Vec<Power>, HemError> {
        let c = self
            .channels
            .get(channel)
            .ok_or_else(|| HemError::InvalidInput(format!("no channel {channel}")))?;
        let p = self.total_power(t) * self.radial[channel];
        Ok(c.axial_shape.cell_fractions(n_cells)?.into_iter().map(|f| p * f).collect())
    }

    /// Deposit channel `channel`'s power at time `t` in the fluid of `pipe`
    /// (replacing its previous cell heat input).
    ///
    /// # Errors
    ///
    /// As [`Self::cell_powers`] and [`HemPipe::set_cell_heat_input`].
    pub fn apply_to_hem_pipe<E: HemEquationOfState>(
        &self,
        channel: usize,
        t: Time,
        pipe: &mut HemPipe<E>,
    ) -> Result<(), HemError> {
        let q = self.cell_powers(channel, pipe.geometry().n_cells(), t)?;
        pipe.set_cell_heat_input(&q)
    }

    /// The number of channels.
    pub fn n_channels(&self) -> usize {
        self.channels.len()
    }
}
