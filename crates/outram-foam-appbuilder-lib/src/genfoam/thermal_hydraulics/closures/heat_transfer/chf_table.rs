// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//                    FSHeatTransferCoefficientModels/subModels/CHFModels/criticalHeatFluxModels/
//                    lookUpTableCHF/lookUpTableCHF.{H,C}
//   Upstream commit: 652b3da (GeN-Foam), built on OpenFOAM v2506
//   Upstream copyright: (C) 2015-2022 EPFL
//   Upstream authors: Yuhang Niu (EPFL), A. Scolaro (EPFL), C. Fiorina (TAMU)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `lookUpTableCHF` — the 2006 Groeneveld CHF look-up table (gh:#1223)
//!
//! A 3-D table `q_CHF(p, G, x_e)` interpolated linearly (or by steps) in pressure, then mass flux,
//! then equilibrium quality, and corrected for geometry. The interpolation is
//! [`crate::genfoam::common::interpolate_table_nd`]; this file is `lookUpTableCHF::value`.
//!
//! ## Data
//!
//! The table is NOT in the source: upstream reads it from the case (`#include "chfTables"` inside
//! the `criticalHeatFluxModel` dictionary), and so does this port
//! ([`LookUpTableChf::from_dict`] on the dictionary the reader hands it). The shipped file is
//! `tests/data/genfoam_chf_gh1223/chfTables`, bit-identical to GeN-Foam `652b3da`
//! `Tutorials/featureCases/1D_PSBT_SC/PhaseII_Ex2_04_6770/constant/fluidRegion/chfTables`
//! (provenance, licence and the original publication: `tests/data/genfoam_chf_gh1223/PROVENANCE.md`).
//! The maintainer's decision of 2026-11-26 (gh:#1223) allows it with that citation.
//!
//! ## What `value` does (read from upstream, in order)
//!
//! 1. `massFlow = rhoL uL aL + rhoV uV (1 - aL)` (`aL` the liquid's normalised fraction).
//! 2. `hLsat = he_liquid(p, T_sat)`, `L = |L|`, `hVsat = hLsat + L`.
//! 3. `hMix = xi hL + (1 - xi) hVsat`, with `xi` the LIQUID's flow quality (a mass fraction of the
//!    liquid: upstream's own comment says the vapour is assumed saturated).
//! 4. `x_e = (hMix - hLsat) / L`, `pk = p / 1000` (the table is in kPa, kg/(m^2 s), -).
//! 5. `q = 1e3 xe-table(gTable(pTable(pk))(massFlow))(x_e)` [W/m^2].
//! 6. If `PitchToDiameter != 1`: `q *= (2 P/D - 1.5) exp(-|x_e|^3 / 2)` (rod-bundle correction,
//!    `K2`); else `q *= max(0.6, sqrt(0.008 / Dh))` (large-diameter correction, `K1`).
//!
//! Out-of-range queries are upstream's default `ERROR` policy: [`ChfTableError::Table`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::io::{FoamDict, FoamEntry, FoamValue};
use petir::mathf::RealMath;

use crate::genfoam::common::interpolate_table::{InterpolateTableError, InterpolationMethod, OutOfBounds};
use crate::genfoam::common::interpolate_table_nd::{check_table, interpolate_table};

/// Errors of building or querying a [`LookUpTableChf`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ChfTableError {
    /// The dictionary lacks a key or holds something that is not the expected list.
    #[error("lookUpTableCHF dictionary: {0}")]
    Dict(String),
    /// The abscissae and the data do not match, or a query is out of range (upstream `FatalError`).
    #[error("lookUpTableCHF table: {0}")]
    Table(#[from] InterpolateTableError),
}

/// Per-cell inputs of [`LookUpTableChf::value`] (SI; `he` in J/kg).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookUpTableChfInputs {
    /// `fluid.flowQuality()` of the pair's fluid (the liquid).
    pub flow_quality: f64,
    /// Pressure [Pa].
    pub p: f64,
    /// `thermo:rho.liquid`.
    pub rho_liquid: f64,
    /// `thermo:rho.vapour`.
    pub rho_vapour: f64,
    /// `|U|` of the liquid.
    pub mag_u_liquid: f64,
    /// `|U|` of the vapour.
    pub mag_u_vapour: f64,
    /// The liquid's normalised fraction.
    pub alpha_liquid: f64,
    /// Structure hydraulic diameter [m].
    pub dh: f64,
    /// `fluid1.thermo().he(p, T_sat)`.
    pub h_liquid_saturated: f64,
    /// `mag(L)`.
    pub latent_heat: f64,
    /// `fluid1.thermo().he()` in the cell.
    pub h_liquid: f64,
}

/// `lookUpTableCHF`: the table and its three interpolation methods.
#[derive(Debug, Clone, PartialEq)]
pub struct LookUpTableChf {
    pitch_to_diameter: f64,
    pressure_values: Vec<f64>,
    mass_flow_rate_values: Vec<f64>,
    quality_values: Vec<f64>,
    data: Vec<Vec<Vec<f64>>>,
    pressure_method: InterpolationMethod,
    mass_flow_method: InterpolationMethod,
    quality_method: InterpolationMethod,
}

fn scalar_list(d: &FoamDict, key: &str) -> Result<Vec<f64>, ChfTableError> {
    match d.get(key) {
        Some(FoamEntry::List(v)) => v
            .iter()
            .map(|x| x.as_scalar().ok_or_else(|| ChfTableError::Dict(format!("`{key}` holds a non-scalar"))))
            .collect(),
        _ => Err(ChfTableError::Dict(format!("missing list `{key}`"))),
    }
}

fn nested_scalars(v: &FoamValue) -> Option<Vec<f64>> {
    match v {
        FoamValue::List(items) => items.iter().map(FoamValue::as_scalar).collect(),
        _ => None,
    }
}

fn method_of(d: &FoamDict, key: &str) -> Result<InterpolationMethod, ChfTableError> {
    // `InterpolateTableBaseGF::interpolationMethodNames_[lookupOrDefault<word>(key, "linear")]`
    match d.get_word(key) {
        None | Some("linear") => Ok(InterpolationMethod::Linear),
        Some("step") => Ok(InterpolationMethod::Step),
        Some(other) => Err(ChfTableError::Dict(format!("`{key}`: unknown interpolation method `{other}`"))),
    }
}

impl LookUpTableChf {
    /// The constructor from its parts (`pTable_`'s construction-time `check`, and the shape checks
    /// upstream makes at the first query).
    ///
    /// # Errors
    /// [`ChfTableError::Table`] when the axes and the data do not agree.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pitch_to_diameter: f64,
        pressure_values: Vec<f64>,
        mass_flow_rate_values: Vec<f64>,
        quality_values: Vec<f64>,
        data: Vec<Vec<Vec<f64>>>,
        pressure_method: InterpolationMethod,
        mass_flow_method: InterpolationMethod,
        quality_method: InterpolationMethod,
    ) -> Result<Self, ChfTableError> {
        check_table(&pressure_values, data.len())?;
        for plane in &data {
            check_table(&mass_flow_rate_values, plane.len())?;
            for row in plane {
                check_table(&quality_values, row.len())?;
            }
        }
        Ok(Self {
            pitch_to_diameter,
            pressure_values,
            mass_flow_rate_values,
            quality_values,
            data,
            pressure_method,
            mass_flow_method,
            quality_method,
        })
    }

    /// Read the `criticalHeatFluxModel` dictionary: `PitchToDiameter`, `pressureValues`,
    /// `massFlowRateValues`, `qualityValues`, `data` and the optional
    /// `{pressure,massFlowRate,quality}InterpolationMethod` (`linear` or `step`). `data` is a list
    /// of pressures of a list of mass fluxes of a list of qualities (the file's comments are
    /// skipped by the dictionary parser).
    ///
    /// # Errors
    /// [`ChfTableError::Dict`] for a missing key or a wrong shape, [`ChfTableError::Table`] for
    /// mismatched axes.
    pub fn from_dict(d: &FoamDict) -> Result<Self, ChfTableError> {
        let ptd = d
            .get_scalar("PitchToDiameter")
            .ok_or_else(|| ChfTableError::Dict("missing scalar `PitchToDiameter`".into()))?;
        let pressure_values = scalar_list(d, "pressureValues")?;
        let mass_flow_rate_values = scalar_list(d, "massFlowRateValues")?;
        let quality_values = scalar_list(d, "qualityValues")?;
        let Some(FoamEntry::List(planes)) = d.get("data") else {
            return Err(ChfTableError::Dict("missing list `data`".into()));
        };
        let mut data = Vec::with_capacity(planes.len());
        for plane in planes {
            let FoamValue::List(rows) = plane else {
                return Err(ChfTableError::Dict("`data`: a pressure plane is not a list".into()));
            };
            let mut p = Vec::with_capacity(rows.len());
            for row in rows {
                p.push(
                    nested_scalars(row)
                        .ok_or_else(|| ChfTableError::Dict("`data`: a row is not a list of scalars".into()))?,
                );
            }
            data.push(p);
        }
        Self::new(
            ptd,
            pressure_values,
            mass_flow_rate_values,
            quality_values,
            data,
            method_of(d, "pressureInterpolationMethod")?,
            method_of(d, "massFlowRateInterpolationMethod")?,
            method_of(d, "qualityInterpolationMethod")?,
        )
    }

    /// `PitchToDiameter`.
    #[must_use]
    pub fn pitch_to_diameter(&self) -> f64 {
        self.pitch_to_diameter
    }

    /// The equilibrium quality `x_e` of one cell (steps 2-4 of the module doc).
    #[must_use]
    pub fn equilibrium_quality(i: &LookUpTableChfInputs) -> f64 {
        let h_v_sat = i.h_liquid_saturated + i.latent_heat;
        let h_mix = i.flow_quality * i.h_liquid + (1.0 - i.flow_quality) * h_v_sat;
        (h_mix - i.h_liquid_saturated) / i.latent_heat
    }

    /// `lookUpTableCHF::value` for one cell [W/m^2].
    ///
    /// # Errors
    /// [`ChfTableError::Table`] when the pressure, mass flux or equilibrium quality is outside the table.
    pub fn value(&self, i: &LookUpTableChfInputs) -> Result<f64, ChfTableError> {
        let mass_flow =
            i.rho_liquid * i.mag_u_liquid * i.alpha_liquid + i.rho_vapour * i.mag_u_vapour * (1.0 - i.alpha_liquid);
        let xe = Self::equilibrium_quality(i);
        let pk = i.p / 1000.0;
        let p_data: Vec<Vec<f64>> =
            interpolate_table(&self.pressure_values, &self.data, self.pressure_method, OutOfBounds::Error, pk)?;
        let g_data: Vec<f64> =
            interpolate_table(&self.mass_flow_rate_values, &p_data, self.mass_flow_method, OutOfBounds::Error, mass_flow)?;
        let mut q_chf: f64 =
            interpolate_table(&self.quality_values, &g_data, self.quality_method, OutOfBounds::Error, xe)? * 1e3;
        if self.pitch_to_diameter != 1.0 {
            let k2 = (2.0 * self.pitch_to_diameter - 1.5) * (-xe.abs().r_powf(3.0) / 2.0).r_exp();
            q_chf *= k2;
        } else {
            let k1 = (0.008 / i.dh).sqrt().max(0.6);
            q_chf *= k1;
        }
        Ok(q_chf)
    }
}
