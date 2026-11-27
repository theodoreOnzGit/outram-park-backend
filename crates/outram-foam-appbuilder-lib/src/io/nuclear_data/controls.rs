// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Readers for the discontinuity-factor keys of `nuclearData` and for `constant/<region>/CRmove`
//! (gh:#1785, gh:#1474). Separate from [`super::read_nuclear_data`] so that the public
//! [`NuclearDataInput`](crate::genfoam::neutronics::xs::NuclearDataInput) keeps its fields.
//!
//! Upstream: `XS.C:163-165` (`adjustDiscFactors`, `useGivenDiscFactors` default false; `groupsWoDF` a
//! `List<int>` default empty) and `XS.C:149-157` (`CRmove`, `zones` default empty).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use std::path::Path;

use outram_foam_basic_lib::io::dict::{FoamEntry, FoamFile, FoamValue};

use super::{named_dicts, optional_bool, optional_scalar, parse_err};
use crate::error::AppBuilderError;
use crate::genfoam::neutronics::xs::disc_factors::DiscFactorControls;
use crate::genfoam::neutronics::xs::driveline::ControlRod;

fn read_file(path: &Path) -> Result<FoamFile, AppBuilderError> {
    FoamFile::read_with_includes_esi(path).map_err(|e| AppBuilderError::Parse {
        file: path.display().to_string(),
        line: 0,
        msg: e.to_string(),
    })
}

/// Read `adjustDiscFactors`, `useGivenDiscFactors` and `groupsWoDF` from a `nuclearData` file.
///
/// # Errors
///
/// [`AppBuilderError`] if the file cannot be read or a key has the wrong type.
pub fn read_disc_factor_controls(path: &Path) -> Result<DiscFactorControls, AppBuilderError> {
    let file = read_file(path)?;
    let name = path.display().to_string();
    let d = &file.dict;
    let mut groups_wo_df = Vec::new();
    match d.get("groupsWoDF") {
        None => {}
        Some(FoamEntry::List(items)) => {
            for v in items {
                match v {
                    FoamValue::Scalar(x) if *x >= 0.0 && x.fract() == 0.0 => groups_wo_df.push(*x as usize),
                    _ => return Err(parse_err(&name, "`groupsWoDF` must hold group indices")),
                }
            }
        }
        Some(_) => return Err(parse_err(&name, "`groupsWoDF` is not a `( … )` list")),
    }
    Ok(DiscFactorControls {
        adjust_disc_factors: optional_bool(d, "adjustDiscFactors", &name)?.unwrap_or(false),
        use_given_disc_factors: optional_bool(d, "useGivenDiscFactors", &name)?.unwrap_or(false),
        groups_wo_df,
    })
}

/// Read `constant/<region>/CRmove`. A missing file, or no `zones` entry, is no rods (`READ_IF_PRESENT`,
/// `lookupOrDefault`). `followerName` is required (`CRdict.lookup`).
///
/// # Errors
///
/// [`AppBuilderError`] for a malformed file or a rod without `followerName`.
pub fn read_control_rods(path: &Path) -> Result<Vec<ControlRod>, AppBuilderError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = read_file(path)?;
    let name = path.display().to_string();
    let Some(entry) = file.dict.get("zones") else {
        return Ok(Vec::new());
    };
    let FoamEntry::List(items) = entry else {
        return Err(parse_err(&name, "`zones` is not a `( … )` list"));
    };
    let mut rods = Vec::new();
    for (rod_name, d) in named_dicts(items, "zones", &name)? {
        let follower_name = match d.get("followerName") {
            Some(FoamEntry::Word(w)) => w.clone(),
            _ => return Err(parse_err(&name, format!("rod `{rod_name}`: `followerName` is required"))),
        };
        rods.push(ControlRod {
            name: rod_name.clone(),
            start_time: optional_scalar(d, "startTime", &name)?.unwrap_or(0.0),
            end_time: optional_scalar(d, "endTime", &name)?.unwrap_or(0.0),
            speed: optional_scalar(d, "speed", &name)?.unwrap_or(0.0),
            follower_name,
            initial_distance_from_mesh_cr: optional_scalar(d, "initialDistanceFromMeshCR", &name)?.unwrap_or(0.0),
        });
    }
    Ok(rods)
}
