// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Reads the input format of GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/XS.{C,H} (the `states`/`zones`
//                    nuclearData dictionary schema and its defaults)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Reading a GeN-Foam `constant/<region>/nuclearData` dictionary
//!
//! Turns the on-disk dictionary into the plain-data
//! [`NuclearDataInput`](crate::genfoam::neutronics::xs::input::NuclearDataInput)
//! that [`CrossSectionData::from_input`] consumes. This is the one link that was
//! missing between an upstream GeN-Foam case and this port's neutronics: the
//! schema mirror and the solvers both existed, but nothing read the file.
//!
//! ```text
//!   nuclearData (text)  --[this module]-->  NuclearDataInput
//!       --> CrossSectionData --> DiffusionNeutronics --> k_eff
//! ```
//!
//! ## The format
//!
//! An ordinary OpenFOAM dictionary, parsed by
//! [`outram_foam_basic_lib::io::dict`]:
//!
//! ```text
//! energyGroups    6;
//! precGroups      8;
//! fastNeutrons    true;
//! xsVariables { TFuel log; rhoCool lin; }
//! states
//! (
//!     reference { TFuel 900; rhoCool 4125; zones ( hx { … } intermed { … } ); }
//!     TFuel1200K { TFuel 1200; zones ( #include "XSTFuel1200K" ); }
//! );
//! ```
//!
//! Two features of it are worth naming because they are why a plain
//! `FoamFile::read` is not enough:
//!
//! - **Dictionaries appear inside lists.** `states ( name { … } … )` and
//!   `zones ( name { … } … )` are lists whose elements alternate a name word and
//!   a braced body. That is handled by
//!   [`FoamValue::Dict`](outram_foam_basic_lib::io::dict::FoamValue::Dict).
//! - **`#include` is used in earnest.** Upstream's ESFR case keeps each state's
//!   zone data in a separate `XS…` file. This module reads through
//!   [`FoamFile::read_with_includes`], so those are spliced before parsing.
//!
//! ## Units
//!
//! Exactly as the file states them: MKSA throughout, so cross sections are per
//! **metre** (not per centimetre), `sigmaPow` is J/m, `IV` is s/m and `lambda`
//! is 1/s. Nothing is converted here — the values reach
//! [`NuclearDataInput`](crate::genfoam::neutronics::xs::input::NuclearDataInput)
//! as written, which is what the `xs` layer expects.
//!
//! ## Defaults, taken from upstream
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `polyharmonicSplineMode` | `1` | `phi(r) = |r|`, linear interpolation |
//! | `fastNeutrons` | `false` | log vs sqrt Doppler transform (metadata only) |
//! | `doNotParametrize` | empty | groups pinned to their reference values |
//! | `fuelFraction` | `1.0` | fuel volume fraction per lattice volume |
//! | `secondaryPowerVolumeFraction` | `1.0` | |
//! | `fractionToSecondaryPower` | `0.0` | |
//! | `dfAdjust` | `true` | |
//!
//! A perturbed state inherits any feedback parameter it does not restate from
//! the reference state, which is why upstream's `TFuel1200K` lists only `TFuel`.
//!
//! [`CrossSectionData::from_input`]: crate::genfoam::neutronics::xs::CrossSectionData::from_input

use std::collections::BTreeMap;
use std::path::Path;

use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamValue};

use crate::error::AppBuilderError;
use crate::genfoam::neutronics::xs::input::{
    NuclearDataInput, StateInput, ZoneConstantsInput, ZoneStateInput,
};
use crate::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};

/// Read and parse a GeN-Foam `nuclearData` dictionary.
///
/// Follows `#include` directives relative to `path`'s own directory, so a case
/// that splits its per-state cross sections across `XS…` files reads as one
/// dictionary.
///
/// # Errors
///
/// [`AppBuilderError::Io`] if the file (or an included file) cannot be read,
/// and [`AppBuilderError::Parse`] for a malformed dictionary, a missing
/// required key, or an array whose length disagrees with `energyGroups` /
/// `precGroups`. Length disagreement is checked here, at the point where the
/// file name is still known, rather than being left to surface later as a
/// shape mismatch deep inside the cross-section layer.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
/// use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
///
/// let input = read_nuclear_data(Path::new("constant/neutroRegion/nuclearData"))?;
/// let xs = CrossSectionData::from_input(&input)?;
/// assert_eq!(xs.energy_groups(), input.energy_groups);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn read_nuclear_data(path: &Path) -> Result<NuclearDataInput, AppBuilderError> {
    let file = FoamFile::read_with_includes(path).map_err(|e| AppBuilderError::Parse {
        file: path.display().to_string(),
        line: 0,
        msg: e.to_string(),
    })?;
    let name = path.display().to_string();
    let d = &file.dict;

    let energy_groups = required_count(d, "energyGroups", &name)?;
    let prec_groups = required_count(d, "precGroups", &name)?;
    let poly_spline_mode = optional_count(d, "polyharmonicSplineMode", &name)?.unwrap_or(1);
    let fast_neutrons = optional_bool(d, "fastNeutrons", &name)?.unwrap_or(false);

    let xs_variables = read_xs_variables(d, &name)?;
    let do_not_parametrize = read_do_not_parametrize(d, &name, energy_groups)?;

    // `scatteringMatrixP0`, `…P1`, … — the count is not declared anywhere in the
    // file, so it is taken from the reference state's first zone and then
    // required to be uniform. GeN-Foam always writes at least P0.
    let states_list = list_entry(d, "states", &name)?;
    let named = named_dicts(states_list, "states", &name)?;
    if named.is_empty() {
        return Err(parse_err(
            &name,
            "`states` is empty; a `reference` state is required",
        ));
    }
    if named[0].0 != "reference" {
        return Err(parse_err(
            &name,
            format!(
                "the first state must be named `reference`, found `{}`",
                named[0].0
            ),
        ));
    }
    let legendre_moments = legendre_moment_count(&named[0].1, &name)?;

    // Reference parameters, so a perturbed state can inherit what it omits.
    let mut reference_parameters: BTreeMap<String, f64> = BTreeMap::new();
    for v in &xs_variables {
        if let Some(x) = optional_scalar(&named[0].1, &v.name, &name)? {
            reference_parameters.insert(v.name.clone(), x);
        }
    }

    let mut states = Vec::with_capacity(named.len());
    for (state_name, state_dict) in &named {
        let is_reference = state_name == "reference";
        let mut parameters = reference_parameters.clone();
        for v in &xs_variables {
            if let Some(x) = optional_scalar(state_dict, &v.name, &name)? {
                parameters.insert(v.name.clone(), x);
            }
        }

        let zones_list = list_entry(state_dict, "zones", &name)?;
        let zone_pairs = named_dicts(zones_list, "zones", &name)?;
        let mut zones = Vec::with_capacity(zone_pairs.len());
        for (zone_name, zone_dict) in &zone_pairs {
            zones.push(read_zone(
                zone_dict,
                zone_name,
                &name,
                energy_groups,
                prec_groups,
                legendre_moments,
                is_reference,
            )?);
        }

        states.push(StateInput {
            name: state_name.clone(),
            parameters,
            zones,
        });
    }

    Ok(NuclearDataInput {
        energy_groups,
        prec_groups,
        legendre_moments,
        poly_spline_mode,
        fast_neutrons,
        xs_variables,
        do_not_parametrize,
        states,
    })
}

// ── one zone ────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn read_zone(
    zd: &FoamDict,
    zone_name: &str,
    file: &str,
    g: usize,
    p: usize,
    moments: usize,
    is_reference: bool,
) -> Result<ZoneStateInput, AppBuilderError> {
    let where_ = |k: &str| format!("zone `{zone_name}`, key `{k}`");

    let d = scalar_array(zd, "D", g, file, &where_("D"))?;
    let nu_sigma_eff = scalar_array(zd, "nuSigmaEff", g, file, &where_("nuSigmaEff"))?;
    let sigma_pow = scalar_array(zd, "sigmaPow", g, file, &where_("sigmaPow"))?;
    let sigma_removal = scalar_array(zd, "sigmaRemoval", g, file, &where_("sigmaRemoval"))?;
    let chi_prompt = scalar_array(zd, "chiPrompt", g, file, &where_("chiPrompt"))?;
    let chi_delayed = scalar_array(zd, "chiDelayed", g, file, &where_("chiDelayed"))?;

    let mut scattering = Vec::with_capacity(moments);
    for m in 0..moments {
        let key = format!("scatteringMatrixP{m}");
        scattering.push(scattering_matrix(zd, &key, g, file, zone_name)?);
    }

    let constants = if is_reference {
        Some(ZoneConstantsInput {
            fuel_fraction: optional_scalar(zd, "fuelFraction", file)?.unwrap_or(1.0),
            secondary_power_volume_fraction: optional_scalar(
                zd,
                "secondaryPowerVolumeFraction",
                file,
            )?
            .unwrap_or(1.0),
            fraction_to_secondary_power: optional_scalar(zd, "fractionToSecondaryPower", file)?
                .unwrap_or(0.0),
            df_adjust: optional_bool(zd, "dfAdjust", file)?.unwrap_or(true),
            iv: scalar_array(zd, "IV", g, file, &where_("IV"))?,
            disc_factor: scalar_array(zd, "discFactor", g, file, &where_("discFactor"))?,
            integral_flux: scalar_array(zd, "integralFlux", g, file, &where_("integralFlux"))?,
            beta: scalar_array(zd, "Beta", p, file, &where_("Beta"))?,
            lambda: scalar_array(zd, "lambda", p, file, &where_("lambda"))?,
        })
    } else {
        None
    };

    Ok(ZoneStateInput {
        name: zone_name.to_string(),
        d,
        nu_sigma_eff,
        sigma_pow,
        sigma_removal,
        chi_prompt,
        chi_delayed,
        scattering,
        constants,
    })
}

/// How many `scatteringMatrixP<n>` entries the reference state's first zone
/// declares, counting up from `P0` until one is absent.
fn legendre_moment_count(reference: &FoamDict, file: &str) -> Result<usize, AppBuilderError> {
    let zones = list_entry(reference, "zones", file)?;
    let pairs = named_dicts(zones, "zones", file)?;
    let (_, first) = pairs
        .first()
        .ok_or_else(|| parse_err(file, "the `reference` state declares no zones"))?;
    let mut m = 0;
    while first.get(&format!("scatteringMatrixP{m}")).is_some() {
        m += 1;
    }
    if m == 0 {
        return Err(parse_err(
            file,
            "the reference state's first zone has no `scatteringMatrixP0`",
        ));
    }
    Ok(m)
}

// ── dictionary helpers ──────────────────────────────────────────────────────

fn parse_err(file: &str, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: msg.into(),
    }
}

/// The trailing `( … )` of a `key nonuniform List<scalar> N ( … );` entry, or of
/// a bare `key ( … );`.
///
/// GeN-Foam writes the long form, but the short one is valid OpenFOAM and costs
/// nothing to accept.
fn scalar_array(
    d: &FoamDict,
    key: &str,
    expected: usize,
    file: &str,
    context: &str,
) -> Result<Vec<f64>, AppBuilderError> {
    let entry = d
        .get(key)
        .ok_or_else(|| parse_err(file, format!("{context}: missing")))?;
    let list = match entry {
        FoamEntry::List(l) => l,
        FoamEntry::Tokens(t) => match t.last() {
            Some(FoamValue::List(l)) => l,
            _ => {
                return Err(parse_err(
                    file,
                    format!("{context}: expected the entry to end in a `( … )` list"),
                ))
            }
        },
        _ => {
            return Err(parse_err(
                file,
                format!("{context}: expected a list, found a scalar or word"),
            ))
        }
    };
    let values = scalars(list, file, context)?;
    if values.len() != expected {
        return Err(parse_err(
            file,
            format!(
                "{context}: expected {expected} values, found {}",
                values.len()
            ),
        ));
    }
    Ok(values)
}

/// A `scatteringMatrixP<n>  G  G ( ( … ) … )` entry as `m[from][to]`.
fn scattering_matrix(
    d: &FoamDict,
    key: &str,
    g: usize,
    file: &str,
    zone: &str,
) -> Result<Vec<Vec<f64>>, AppBuilderError> {
    let context = format!("zone `{zone}`, key `{key}`");
    let entry = d
        .get(key)
        .ok_or_else(|| parse_err(file, format!("{context}: missing")))?;
    let rows = match entry {
        FoamEntry::List(l) => l,
        FoamEntry::Tokens(t) => match t.last() {
            Some(FoamValue::List(l)) => l,
            _ => {
                return Err(parse_err(
                    file,
                    format!("{context}: expected the entry to end in a `( … )` list of rows"),
                ))
            }
        },
        _ => {
            return Err(parse_err(
                file,
                format!("{context}: expected a list of rows"),
            ))
        }
    };
    if rows.len() != g {
        return Err(parse_err(
            file,
            format!("{context}: expected {g} rows, found {}", rows.len()),
        ));
    }
    let mut out = Vec::with_capacity(g);
    for (i, row) in rows.iter().enumerate() {
        let FoamValue::List(cells) = row else {
            return Err(parse_err(
                file,
                format!("{context}: row {i} is not a `( … )` list"),
            ));
        };
        let values = scalars(cells, file, &context)?;
        if values.len() != g {
            return Err(parse_err(
                file,
                format!(
                    "{context}: row {i} has {} entries, expected {g}",
                    values.len()
                ),
            ));
        }
        out.push(values);
    }
    Ok(out)
}

fn scalars(items: &[FoamValue], file: &str, context: &str) -> Result<Vec<f64>, AppBuilderError> {
    items
        .iter()
        .map(|v| match v {
            FoamValue::Scalar(x) => Ok(*x),
            other => Err(parse_err(
                file,
                format!("{context}: expected a number, found `{other:?}`"),
            )),
        })
        .collect()
}

/// Walk a `( name { … } name { … } )` list into `(name, dict)` pairs.
fn named_dicts<'a>(
    items: &'a [FoamValue],
    what: &str,
    file: &str,
) -> Result<Vec<(String, &'a FoamDict)>, AppBuilderError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let name = match &items[i] {
            FoamValue::Word(w) => w.clone(),
            FoamValue::Str(s) => s.clone(),
            other => {
                return Err(parse_err(
                    file,
                    format!("`{what}`: expected an entry name, found `{other:?}`"),
                ))
            }
        };
        let Some(FoamValue::Dict(d)) = items.get(i + 1) else {
            return Err(parse_err(
                file,
                format!("`{what}`: entry `{name}` is not followed by a `{{ … }}` body"),
            ));
        };
        out.push((name, d));
        i += 2;
    }
    Ok(out)
}

fn list_entry<'a>(
    d: &'a FoamDict,
    key: &str,
    file: &str,
) -> Result<&'a [FoamValue], AppBuilderError> {
    match d.get(key) {
        Some(FoamEntry::List(l)) => Ok(l.as_slice()),
        Some(_) => Err(parse_err(file, format!("`{key}` is not a `( … )` list"))),
        None => Err(parse_err(file, format!("missing required key `{key}`"))),
    }
}

fn read_xs_variables(d: &FoamDict, file: &str) -> Result<Vec<XsVariable>, AppBuilderError> {
    let Some(FoamEntry::SubDict(sub)) = d.get("xsVariables") else {
        // A case with no feedback parametrisation is legal: every state then
        // reduces to the reference one.
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(sub.len());
    for (name, entry) in sub.iter() {
        let keyword = match entry {
            FoamEntry::Word(w) => w.as_str(),
            _ => {
                return Err(parse_err(
                    file,
                    format!("xsVariables `{name}`: expected a law keyword (lin/log/sqrt)"),
                ))
            }
        };
        let law = VariableLaw::from_keyword(keyword).ok_or_else(|| {
            parse_err(
                file,
                format!(
                    "xsVariables `{name}`: unknown law `{keyword}` (expected lin, log or sqrt)"
                ),
            )
        })?;
        out.push(XsVariable::new(name, law));
    }
    Ok(out)
}

fn read_do_not_parametrize(
    d: &FoamDict,
    file: &str,
    g: usize,
) -> Result<Vec<usize>, AppBuilderError> {
    let Some(entry) = d.get("doNotParametrize") else {
        return Ok(Vec::new());
    };
    let FoamEntry::List(items) = entry else {
        return Err(parse_err(file, "`doNotParametrize` is not a `( … )` list"));
    };
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        let FoamValue::Scalar(x) = v else {
            return Err(parse_err(
                file,
                "`doNotParametrize` must contain group indices",
            ));
        };
        let i = *x as usize;
        if *x < 0.0 || i >= g {
            return Err(parse_err(
                file,
                format!("`doNotParametrize`: group index {x} is outside 0..{g}"),
            ));
        }
        out.push(i);
    }
    Ok(out)
}

fn required_count(d: &FoamDict, key: &'static str, file: &str) -> Result<usize, AppBuilderError> {
    optional_count(d, key, file)?
        .ok_or_else(|| parse_err(file, format!("missing required key `{key}`")))
}

fn optional_count(d: &FoamDict, key: &str, file: &str) -> Result<Option<usize>, AppBuilderError> {
    match d.get(key) {
        None => Ok(None),
        Some(FoamEntry::Scalar(x)) if *x >= 1.0 && x.fract() == 0.0 => Ok(Some(*x as usize)),
        Some(_) => Err(parse_err(
            file,
            format!("`{key}` must be a positive whole number"),
        )),
    }
}

fn optional_scalar(d: &FoamDict, key: &str, file: &str) -> Result<Option<f64>, AppBuilderError> {
    match d.get(key) {
        None => Ok(None),
        Some(FoamEntry::Scalar(x)) => Ok(Some(*x)),
        Some(_) => Err(parse_err(file, format!("`{key}` must be a number"))),
    }
}

fn optional_bool(d: &FoamDict, key: &str, file: &str) -> Result<Option<bool>, AppBuilderError> {
    match d.get(key) {
        None => Ok(None),
        Some(FoamEntry::Word(w)) if w == "true" || w == "yes" || w == "on" => Ok(Some(true)),
        Some(FoamEntry::Word(w)) if w == "false" || w == "no" || w == "off" => Ok(Some(false)),
        Some(_) => Err(parse_err(
            file,
            format!("`{key}` must be a boolean (true/false)"),
        )),
    }
}

#[cfg(test)]
mod tests;
