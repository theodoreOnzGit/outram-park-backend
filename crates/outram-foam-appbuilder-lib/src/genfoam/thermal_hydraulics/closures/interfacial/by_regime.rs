// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/
//             templatedModels/byRegime/{byRegimeModel.{H,C},byRegimeModels.C},
//             regimeMapModels/regimeMapModelTemplates.C (`interpolateValue`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::by_regime` — the `byRegime` dispatch (gh:#1225)
//!
//! Upstream's `byRegimeModel<valueType, baseModel>` holds one sub-model per regime of a named
//! regime map and returns `sum_j c_j model_j.value(celli)` over the cell's `regimeLabelCoeffs_`
//! (`regimeMapModel::interpolateValue`). Here the run-time-selected sub-model is a closed enum
//! chosen by the caller: [`ByRegimeModel<M>`] is either one model (`type <model>`) or one model per
//! regime name (`type byRegime`), generic over the model enum `M` (a dispersion, a fluid diameter,
//! an interfacial-area, a contact-partition model, ...), and the caller supplies the evaluation of
//! `M` for the cell.
//!
//! The arithmetic is `interpolateValue`'s, term for term: ONE regime returns `models[r].value()`
//! unmultiplied; several return `c_0 v_0 + c_1 v_1 + ...` accumulated left to right. A regime of the
//! cell that has no sub-model is an error upstream (`subDict(regimeName)` is read for every regime
//! of the map at construction) and is reported by [`ByRegimeModel::per_regime`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

/// A model that is either a single selection or one selection per regime of a regime map.
#[derive(Debug, Clone, PartialEq)]
pub enum ByRegimeModel<M> {
    /// Not `byRegime`: the same model in every cell.
    Single(M),
    /// `type byRegime; regimeMap ...;` with one model per regime name, in the map's label order.
    PerRegime(Vec<(String, M)>),
}

impl<M> ByRegimeModel<M> {
    /// Build the `byRegime` form for the regimes `regime_names` of a map (label order), taking each
    /// regime's model from `lookup`.
    ///
    /// # Errors
    /// A regime of the map with no sub-model.
    pub fn per_regime(regime_names: &[String], mut lookup: impl FnMut(&str) -> Option<M>) -> Result<Self, String> {
        let mut v = Vec::new();
        for name in regime_names {
            let m = lookup(name).ok_or_else(|| format!("byRegime: no sub-model for regime `{name}`"))?;
            v.push((name.clone(), m));
        }
        Ok(Self::PerRegime(v))
    }

    /// `byRegimeModel::value(celli)` for the cell's regime coefficients `weights` (empty when the case
    /// has no regime map and the model is `Single`); `eval` evaluates one model for the cell.
    pub fn value(&self, weights: &[(String, f64)], eval: impl Fn(&M) -> f64) -> f64 {
        match self {
            Self::Single(m) => eval(m),
            Self::PerRegime(table) => {
                let model = |name: &str| -> f64 {
                    table
                        .iter()
                        .find(|(n, _)| n == name)
                        .map_or(0.0, |(_, m)| eval(m))
                };
                if weights.len() == 1 {
                    return model(&weights[0].0);
                }
                let mut value = weights[0].1 * model(&weights[0].0);
                for (name, c) in &weights[1..] {
                    value += c * model(name);
                }
                value
            }
        }
    }
}
