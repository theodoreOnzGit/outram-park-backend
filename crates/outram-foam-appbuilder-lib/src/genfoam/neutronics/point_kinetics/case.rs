// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/pointKinetics/pointKineticNeutronics.C (the `nuclearData_` / `reactorState_` initialisers, lines 62-211, 285-290, 358-380)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # The point-kinetics case reader (gh:#1922, README round 61)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! [`read_pk_config`] builds a [`PkConfig`] from the `nuclearData` dictionary of the neutronics region the way
//! `pointKineticNeutronics`'s initialiser list does: `promptGenerationTime`, `Beta`, `lambda`, `fastNeutrons` and the seven `feedbackCoeff*` with
//! `absoluteDrivelineExpansionCoeff` are `get<>` (a missing key is a `FatalError` upstream and an `Err` here), `controlRodReactivityMap` and the
//! `externalReactivityTimeProfile` / `boronReactivityTimeProfile` are looked up with their defaults (empty / absent).
//!
//! ## Refused by name
//!
//! `GEMReactivityMap` with more than one knot (upstream: "does NOT work in GeN-Foam 2.0"), an `externalReactivityTimeProfile` of a `type` other than
//! `table` (with one row or more) or `constant`, and the `*FeedbackZones` keys of `setFeedbackCellField` (every shipped case sets none; the zones would
//! need the region's `cellZones`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamValue};
use uom::si::f64::Time;
use uom::si::time::second;

use super::model::{FeedbackCoefficients, PkConfig};
use crate::genfoam::common::TimeProfile;

/// A list of plain scalars `( a b c )`.
fn scalar_list(e: &FoamEntry) -> Option<Vec<f64>> {
    let FoamEntry::List(v) = e else { return None };
    v.iter()
        .map(|x| match x {
            FoamValue::Scalar(s) => Some(*s),
            _ => None,
        })
        .collect()
}

/// Every `( a b )` pair of a list of pairs.
fn pair_list(v: &[FoamValue]) -> Option<Vec<(f64, f64)>> {
    v.iter()
        .map(|x| match x {
            FoamValue::List(p) if p.len() == 2 => match (&p[0], &p[1]) {
                (FoamValue::Scalar(a), FoamValue::Scalar(b)) => Some((*a, *b)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The first parenthesised list of pairs inside an entry (`table table ( ( t v ) ... );` parses as bare tokens, `( ( t v ) ... )` as a list).
fn first_pair_list(e: &FoamEntry) -> Option<Vec<(f64, f64)>> {
    match e {
        FoamEntry::List(v) => pair_list(v),
        FoamEntry::Tokens(t) => t.iter().find_map(|x| match x {
            FoamValue::List(v) => pair_list(v),
            _ => None,
        }),
        _ => None,
    }
}

/// `timeProfile(dict, name, runTime)` for a `constant` or `table` `Function1`; `Ok(None)` when `name` is absent (`valid()` false).
fn time_profile(d: &FoamDict, name: &str, start_time: f64) -> Result<Option<TimeProfile>, String> {
    let Some(entry) = d.get(name) else { return Ok(None) };
    let FoamEntry::SubDict(p) = entry else { return Err(format!("{name}: only the dictionary form `{{ type table; table table (...); }}` is read")) };
    match p.get_word("type") {
        Some("constant") => {
            let v = p.get_scalar("value").ok_or_else(|| format!("{name}: type constant without value"))?;
            Ok(Some(TimeProfile::constant(v)))
        }
        Some("table") => {
            let rows = p.get("table").and_then(first_pair_list).ok_or_else(|| format!("{name}: no `table` rows"))?;
            if rows.is_empty() {
                return Err(format!("{name}: empty table"));
            }
            // OpenFOAM's `Function1::Table` of ONE row is that value at every time (a one-point table clamps); [`TimeProfile::table`] needs two
            if rows.len() == 1 {
                return Ok(Some(TimeProfile::constant(rows[0].1)));
            }
            let times: Vec<Time> = rows.iter().map(|r| Time::new::<second>(r.0)).collect();
            let values: Vec<f64> = rows.iter().map(|r| r.1).collect();
            let t = TimeProfile::table(times, values).map_err(|e| format!("{name}: {e}"))?;
            Ok(Some(t.with_start_time(Time::new::<second>(start_time))))
        }
        Some(other) => Err(format!("{name}: type {other} is not ported (only constant and table)")),
        None => Err(format!("{name}: no type")),
    }
}

/// `pointKineticNeutronics`'s `nuclearData_` initialisers (solid fuel, no external source, no decay power) plus the `reactorState` `pTarget`.
///
/// `p_target` is `reactorState` `pTarget` of the start directory, `start_time` its time.
///
/// # Errors
/// A missing key that upstream `get`s, a malformed list, or one of the refusals in the module documentation.
pub fn read_pk_config(nuclear: &FoamDict, p_target: f64, start_time: f64) -> Result<PkConfig, String> {
    let scalar = |k: &str| nuclear.get_scalar(k).ok_or_else(|| format!("nuclearData: no scalar `{k}`"));
    let list = |k: &str| nuclear.get(k).and_then(scalar_list).ok_or_else(|| format!("nuclearData: no scalar list `{k}`"));
    for z in ["fuelFeedbackZones", "coolFeedbackZones", "structFeedbackZones", "structMechFeedbackZones", "drivelineFeedbackZones"] {
        if nuclear.get(z).is_some() {
            return Err(format!("nuclearData.{z}: feedback zones are not read by this reader (setFeedbackCellField would need the cellZones)"));
        }
    }
    let fast_neutrons = match nuclear.get_word("fastNeutrons") {
        Some("true" | "yes" | "on") => true,
        Some("false" | "no" | "off") => false,
        _ => return Err("nuclearData: no boolean `fastNeutrons`".to_string()),
    };
    let control_rod_map = match nuclear.get("controlRodReactivityMap") {
        Some(FoamEntry::List(v)) => pair_list(v).ok_or_else(|| "nuclearData.controlRodReactivityMap: not a list of pairs".to_string())?,
        Some(_) => return Err("nuclearData.controlRodReactivityMap: not a list".to_string()),
        None => Vec::new(),
    };
    let gem_map = match nuclear.get("GEMReactivityMap") {
        Some(FoamEntry::List(v)) => pair_list(v).ok_or_else(|| "nuclearData.GEMReactivityMap: not a list of pairs".to_string())?,
        _ => Vec::new(),
    };
    Ok(PkConfig {
        prompt_generation_time: scalar("promptGenerationTime")?,
        betas: list("Beta")?,
        lambdas: list("lambda")?,
        fast_neutrons,
        coefficients: FeedbackCoefficients {
            doppler: scalar("feedbackCoeffDoppler")?,
            t_fuel: scalar("feedbackCoeffTFuel")?,
            t_clad: scalar("feedbackCoeffTClad")?,
            t_cool: scalar("feedbackCoeffTCool")?,
            rho_cool: scalar("feedbackCoeffRhoCool")?,
            t_struct: scalar("feedbackCoeffTStruct")?,
            t_struct_mech: scalar("feedbackCoeffTStructMech")?,
            driveline_expansion: scalar("absoluteDrivelineExpansionCoeff")?,
        },
        control_rod_map,
        gem_map,
        external_reactivity: time_profile(nuclear, "externalReactivityTimeProfile", start_time)?,
        boron_reactivity: time_profile(nuclear, "boronReactivityTimeProfile", start_time)?,
        decay_power: None,
        p_target,
        subcritical_index: None,
        ksrc: None,
        precursor_powers: None,
        external_source: None,
        energy_groups: 1,
        start_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;

    const TEXT: &str = "FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }
fastNeutrons    true;
promptGenerationTime 1e-06;
Beta            ( 7.2315e-05 0.000609661 );
lambda          ( 0.0125371 0.0300828 );
feedbackCoeffDoppler 0;
feedbackCoeffTFuel -3e-06;
feedbackCoeffTClad 0;
feedbackCoeffTCool 0;
feedbackCoeffRhoCool 0;
feedbackCoeffTStruct 0;
feedbackCoeffTStructMech 0;
absoluteDrivelineExpansionCoeff 0.0;
controlRodReactivityMap ( ( 0.1 -0.01 ) ( 0 0 ) ( -0.1 0.01 ) );
externalReactivityTimeProfile
{
    type        table;
    table       table
    (
        (   0   0.0005767131    ) //- one row
    );
}
";

    #[test]
    fn the_shipped_2d_full_coupling_nuclear_data_reads_as_point_kinetics_input() {
        let f = FoamFile::parse(TEXT).expect("parse");
        let cfg = read_pk_config(&f.dict, 1.0e7, 100.0).expect("config");
        assert_eq!(cfg.betas, vec![7.2315e-05, 0.000609661]);
        assert_eq!(cfg.prompt_generation_time, 1e-6);
        assert_eq!(cfg.coefficients.t_fuel, -3e-6);
        assert_eq!(cfg.control_rod_map, vec![(0.1, -0.01), (0.0, 0.0), (-0.1, 0.01)]);
        assert!(cfg.fast_neutrons && cfg.p_target == 1.0e7 && cfg.start_time == 100.0);
        let ext = cfg.external_reactivity.expect("external reactivity");
        assert_eq!(ext.value(Time::new::<second>(150.0)), 0.0005767131);
    }

    #[test]
    fn a_missing_key_and_feedback_zones_are_refused_by_name() {
        let f = FoamFile::parse("FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }\nfastNeutrons true;\n").expect("parse");
        assert!(read_pk_config(&f.dict, 1.0, 0.0).unwrap_err().contains("promptGenerationTime"));
        let z = format!("{TEXT}\nfuelFeedbackZones ( zone0 );\n");
        let f = FoamFile::parse(&z).expect("parse");
        assert!(read_pk_config(&f.dict, 1.0, 0.0).unwrap_err().contains("fuelFeedbackZones"));
    }
}
