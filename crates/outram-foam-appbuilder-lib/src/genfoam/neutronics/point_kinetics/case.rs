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
//! ## Refused by name (README round 63: the feedback zones, external source, decay power and GEM are no longer refused; see [`read_pk_case`])
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

use super::model::{ExternalSourceConfig, ExternalSourceMode, FeedbackCoefficients, PkConfig, ReadReferences};
use crate::genfoam::common::time_profile::{CycleRate, Oscillation, OscillationKind, RampShape};
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

/// A plain scalar entry (OpenFOAM's `Function1<scalar>::New("scale", dict)` of a primitive entry is a `Constant`; nested functions are refused).
fn plain_scalar(p: &FoamDict, key: &str, name: &str) -> Result<Option<f64>, String> {
    match p.get(key) {
        None => Ok(None),
        Some(FoamEntry::Scalar(v)) => Ok(Some(*v)),
        Some(_) => Err(format!("{name}.{key}: only a plain scalar is read (nested Function1 entries are not ported)")),
    }
}

/// `timeProfile(dict, name, runTime)` for the `Function1<scalar>` types listed in README round 63; `Ok(None)` when `name` is absent (`valid()` false).
///
/// `Function1<scalar>::New(type, dict, type)` (`timeProfile.C:60-100`), evaluated at `t - startTime` where `startTime` is the PROFILE dictionary's
/// own key (default 0; CORRECTED gh:#1927: round 61 shifted a table by the run's start time, which upstream does not; only one-row tables, which are
/// constants, were read then, so no number moved). Types translated from OpenFOAM-ESI v2506 `src/OpenFOAM/primitives/functions/Function1/`:
/// `constant`/`uniform` (`value`), `zero`, `one`, the five ramps (`start` default 0, `duration`), `sine`/`cosine`/`square` (`t0`, `amplitude`,
/// `period` or `frequency`, `scale`, `level`; `square`: `mark`, `space`), `polynomial` (`coeffs`), `table`.
fn time_profile(d: &FoamDict, name: &str) -> Result<Option<TimeProfile>, String> {
    let Some(entry) = d.get(name) else { return Ok(None) };
    let FoamEntry::SubDict(p) = entry else { return Err(format!("{name}: only the dictionary form `{{ type ...; }}` is read")) };
    let startt = p.get_scalar("startTime").unwrap_or(0.0);
    let need = |k: &str| p.get_scalar(k).ok_or_else(|| format!("{name}: type {} without {k}", p.get_word("type").unwrap_or("?")));
    let ramp = |shape: RampShape| -> Result<TimeProfile, String> {
        TimeProfile::ramp(shape, p.get_scalar("start").unwrap_or(0.0), need("duration")?).map_err(|e| format!("{name}: {e}"))
    };
    let osc = |kind: OscillationKind| -> Result<TimeProfile, String> {
        let rate = if let Some(period) = plain_scalar(p, "period", name)? {
            CycleRate::Period(period)
        } else {
            CycleRate::Frequency(plain_scalar(p, "frequency", name)?.ok_or_else(|| format!("{name}: neither period nor frequency"))?)
        };
        let mark = p.get_scalar("mark").or_else(|| p.get_scalar("markSpace")).unwrap_or(1.0);
        let space = p.get_scalar("space").unwrap_or(1.0);
        Ok(TimeProfile::oscillation(Oscillation {
            kind,
            t0: p.get_scalar("t0").unwrap_or(0.0),
            amplitude: plain_scalar(p, "amplitude", name)?,
            rate,
            scale: plain_scalar(p, "scale", name)?.ok_or_else(|| format!("{name}: no scale"))?,
            level: plain_scalar(p, "level", name)?.ok_or_else(|| format!("{name}: no level"))?,
            positive_fraction: mark / (mark + space),
        }))
    };
    let profile = match p.get_word("type") {
        Some("constant" | "uniform") => TimeProfile::constant(need("value")?),
        Some("zero") => TimeProfile::constant(0.0),
        Some("one") => TimeProfile::constant(1.0),
        Some("linearRamp") => ramp(RampShape::Linear)?,
        Some("quadraticRamp") => ramp(RampShape::Quadratic)?,
        Some("halfCosineRamp") => ramp(RampShape::HalfCosine)?,
        Some("quarterCosineRamp") => ramp(RampShape::QuarterCosine)?,
        Some("quarterSineRamp") => ramp(RampShape::QuarterSine)?,
        Some("sine") => osc(OscillationKind::Sine)?,
        Some("cosine") => osc(OscillationKind::Cosine)?,
        Some("square") => osc(OscillationKind::Square)?,
        Some("polynomial") => {
            let rows = p.get("coeffs").and_then(first_pair_list).ok_or_else(|| format!("{name}: no `coeffs` pairs"))?;
            TimeProfile::polynomial(rows).map_err(|e| format!("{name}: {e}"))?
        }
        Some("table") => {
            let rows = p.get("table").and_then(first_pair_list).ok_or_else(|| format!("{name}: no `table` rows"))?;
            if rows.is_empty() {
                return Err(format!("{name}: empty table"));
            }
            // OpenFOAM's `Function1::Table` of ONE row is that value at every time (a one-point table clamps); [`TimeProfile::table`] needs two
            if rows.len() == 1 {
                TimeProfile::constant(rows[0].1)
            } else {
                let times: Vec<Time> = rows.iter().map(|r| Time::new::<second>(r.0)).collect();
                let values: Vec<f64> = rows.iter().map(|r| r.1).collect();
                TimeProfile::table(times, values).map_err(|e| format!("{name}: {e}"))?
            }
        }
        Some(other) => return Err(format!("{name}: type {other} is not ported (README round 63 lists the types; fmi, tableFile, csvFile, coded, expression, scale, step are refused)")),
        None => return Err(format!("{name}: no type")),
    };
    Ok(Some(profile.with_start_time(Time::new::<second>(startt))))
}

/// `pointKineticNeutronics`'s `nuclearData_` initialisers (solid fuel, no external source, no decay power) plus the `reactorState` `pTarget`.
/// The feedback zones, the external source, the `reactorState` keys and GEM are read by [`read_pk_case`], which calls this.
///
/// `p_target` is `reactorState` `pTarget` of the start directory, `start_time` its time.
///
/// # Errors
/// A missing key that upstream `get`s, a malformed list, or one of the refusals in the module documentation.
pub fn read_pk_config(nuclear: &FoamDict, p_target: f64, start_time: f64) -> Result<PkConfig, String> {
    let scalar = |k: &str| nuclear.get_scalar(k).ok_or_else(|| format!("nuclearData: no scalar `{k}`"));
    let list = |k: &str| nuclear.get(k).and_then(scalar_list).ok_or_else(|| format!("nuclearData: no scalar list `{k}`"));
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
        gem_map: Vec::new(),
        external_reactivity: time_profile(nuclear, "externalReactivityTimeProfile")?,
        boron_reactivity: time_profile(nuclear, "boronReactivityTimeProfile")?,
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

/// The word list of a `wordList` entry `( a "b" )` (`lookupOrDefault(keyword, wordList())`).
fn word_list(e: &FoamEntry) -> Option<Vec<String>> {
    let FoamEntry::List(v) = e else { return None };
    v.iter()
        .map(|x| match x {
            FoamValue::Word(w) | FoamValue::Str(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

/// A scalar list written `( a b )` or `n ( a b )`.
fn counted_scalar_list(e: &FoamEntry) -> Option<Vec<f64>> {
    match e {
        FoamEntry::List(_) => scalar_list(e),
        FoamEntry::Tokens(t) => t.iter().find_map(|x| match x {
            FoamValue::List(v) => v
                .iter()
                .map(|y| match y {
                    FoamValue::Scalar(s) => Some(*s),
                    _ => None,
                })
                .collect(),
            _ => None,
        }),
        _ => None,
    }
}

fn entry_name(e: &FoamEntry) -> Option<String> {
    match e {
        FoamEntry::Word(w) | FoamEntry::Str(w) => Some(w.clone()),
        _ => None,
    }
}

fn is_true(w: Option<&str>) -> bool {
    matches!(w, Some("true" | "yes" | "on" | "1"))
}

/// The five `*FeedbackZones` lists of `nuclearData` (`setFeedbackCellField`: `lookupOrDefault(keyword, wordList())`); an empty list is "every cell".
/// Upstream reads `coolFeedbackZones`: a `coolantFeedbackZones` key is NOT read (round 36 finding 1), as upstream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackZoneNames {
    /// `fuelFeedbackZones`.
    pub fuel: Vec<String>,
    /// `coolFeedbackZones`.
    pub cool: Vec<String>,
    /// `structFeedbackZones`.
    pub structure: Vec<String>,
    /// `structMechFeedbackZones`.
    pub structure_mech: Vec<String>,
    /// `drivelineFeedbackZones`.
    pub driveline: Vec<String>,
}

/// GEM as the case asks for it: `GEMFlowFaceZone` of the fluid region `fluidRegionName`, plus the map and the reference level.
#[derive(Debug, Clone, PartialEq)]
pub struct GemRequest {
    /// `neutronicsProperties` `fluidRegionName`.
    pub fluid_region: String,
    /// `nuclearData` `GEMFlowFaceZone`.
    pub face_zone: String,
    /// `nuclearData` `GEMSodiumLevelRef`.
    pub sodium_level_ref: f64,
    /// `nuclearData` `GEMReactivityMap` (more than one knot).
    pub map: Vec<(f64, f64)>,
}

/// The dictionaries `pointKineticNeutronics`'s constructor reads (`neutronics.C:60-283`, `pointKineticNeutronics.C:62-905`).
#[derive(Debug, Clone, Copy)]
pub struct PkCaseDicts<'a> {
    /// `constant/<region>/nuclearData`.
    pub nuclear: &'a FoamDict,
    /// `constant/<region>/neutronicsProperties`.
    pub neutronics_properties: &'a FoamDict,
    /// `constant/<region>/externalSource`, when present.
    pub external_source: Option<&'a FoamDict>,
    /// `<start>/uniform/reactorState`, when present (upstream: `pTarget` is `get`, so a missing file is an error here).
    pub reactor_state: Option<&'a FoamDict>,
}

/// Everything [`read_pk_case`] reads.
#[derive(Debug, Clone)]
pub struct PkCase {
    /// The class's configuration (external source, decay power, restart powers, profiles).
    pub cfg: PkConfig,
    /// The feedback zone names.
    pub zones: FeedbackZoneNames,
    /// `reactorState`'s `T*Ref` keys.
    pub refs: ReadReferences,
    /// GEM, when `GEMReactivityMap` has more than one knot.
    pub gem: Option<GemRequest>,
}

/// The full constructor read of `pointKineticNeutronics` for the solid-fuel path (gh:#1927, README round 63): [`read_pk_config`] plus
/// `reactorState` (`pTarget`, `decayPowerTimeProfile`, `precursorPowers`, `subcriticalIndex`, `ksrc`, the `T*Ref` keys), the external source
/// (`externalSourceNeutronics`, `externalSource`, `nuFission`, `energyPerFission`), the feedback zones and GEM.
///
/// # Errors
/// As [`read_pk_config`]; `externalSourceNeutronics true` without an `externalSource` dictionary or with an `externalSourceMode` other than
/// `transient`/`powerMonitoring` (upstream `FatalError`); GEM without `GEMSodiumLevelRef`, `GEMFlowFaceZone` or `fluidRegionName`;
/// `liquidFuel`-style keys are not read here (refused by the caller).
pub fn read_pk_case(d: &PkCaseDicts<'_>, start_time: f64) -> Result<PkCase, String> {
    let rs = d.reactor_state.ok_or_else(|| "reactorState: not found in the start directory (pTarget is required)".to_string())?;
    let p_target = rs.get_scalar("pTarget").ok_or_else(|| "reactorState: no pTarget".to_string())?;
    let mut cfg = read_pk_config(d.nuclear, p_target, start_time)?;
    cfg.decay_power = time_profile(rs, "decayPowerTimeProfile")?;
    cfg.subcritical_index = rs.get_scalar("subcriticalIndex");
    cfg.ksrc = rs.get_scalar("ksrc");
    cfg.precursor_powers = match rs.get("precursorPowers") {
        None => None,
        Some(e) => Some(counted_scalar_list(e).ok_or_else(|| "reactorState.precursorPowers: not a scalar list".to_string())?),
    };
    if let Some(pp) = &cfg.precursor_powers {
        if pp.len() != cfg.betas.len() {
            return Err(format!("reactorState.precursorPowers: {} entries for {} delayed groups", pp.len(), cfg.betas.len()));
        }
    }
    if is_true(d.neutronics_properties.get_word("externalSourceNeutronics")) {
        let es = d.external_source.ok_or_else(|| "externalSourceNeutronics true without constant/<region>/externalSource".to_string())?;
        let mode = match es.get_word("externalSourceMode") {
            Some("transient") => ExternalSourceMode::Transient,
            Some("powerMonitoring") => ExternalSourceMode::PowerMonitoring,
            other => return Err(format!("externalSource.externalSourceMode {other:?} is not an allowed key word (transient, powerMonitoring)")),
        };
        cfg.external_source = Some(ExternalSourceConfig {
            mode,
            modulation_profile: time_profile(es, "externalSourceModulationTimeProfile")?,
            nu_source: es.get_scalar("nuSource").unwrap_or(0.0),
            beam_energy: es.get_scalar("beamEnergy").unwrap_or(0.0),
            nu_fission: d.nuclear.get_scalar("nuFission").unwrap_or(0.0),
            energy_per_fission: d.nuclear.get_scalar("energyPerFission").unwrap_or(0.0),
            modulation_factor: es.get_scalar("modulationFactor").unwrap_or(0.0),
            power_modulation_delay: es.get_scalar("powerModulationDelay").unwrap_or(0.0),
        });
    }
    let zone = |k: &str| d.nuclear.get(k).and_then(word_list).unwrap_or_default();
    let zones = FeedbackZoneNames {
        fuel: zone("fuelFeedbackZones"),
        cool: zone("coolFeedbackZones"),
        structure: zone("structFeedbackZones"),
        structure_mech: zone("structMechFeedbackZones"),
        driveline: zone("drivelineFeedbackZones"),
    };
    let r = |k: &str| rs.get_scalar(k);
    let refs = ReadReferences {
        t_fuel: r("TFuelRef"),
        t_clad: r("TladRef"),
        t_cool: r("TCoolRef"),
        rho_cool: r("rhoCoolRef"),
        t_struct: r("TStructRef"),
        t_struct_mech: r("TStructMechRef"),
        t_driveline: r("TDriveLineRef"),
    };
    let gem_map = match d.nuclear.get("GEMReactivityMap") {
        Some(FoamEntry::List(v)) => pair_list(v).ok_or_else(|| "nuclearData.GEMReactivityMap: not a list of pairs".to_string())?,
        _ => Vec::new(),
    };
    let gem = if gem_map.len() > 1 {
        Some(GemRequest {
            fluid_region: d.neutronics_properties.get("fluidRegionName").and_then(entry_name).ok_or_else(|| "GEM: neutronicsProperties has no fluidRegionName".to_string())?,
            face_zone: d.nuclear.get("GEMFlowFaceZone").and_then(entry_name).ok_or_else(|| "GEM: nuclearData has no GEMFlowFaceZone".to_string())?,
            sodium_level_ref: d.nuclear.get_scalar("GEMSodiumLevelRef").ok_or_else(|| "GEM: nuclearData has no GEMSodiumLevelRef".to_string())?,
            map: gem_map,
        })
    } else {
        None
    };
    Ok(PkCase { cfg, zones, refs, gem })
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;
    use petir::mathf::RealMath;

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

    fn parse(text: &str) -> FoamFile {
        FoamFile::parse(&format!("FoamFile {{ version 2.0; format ascii; class dictionary; object x; }}\n{text}")).expect("parse")
    }

    #[test]
    fn a_missing_key_is_refused_by_name() {
        let f = parse("fastNeutrons true;\n");
        assert!(read_pk_config(&f.dict, 1.0, 0.0).unwrap_err().contains("promptGenerationTime"));
    }

    #[test]
    fn feedback_zones_are_read_and_the_coolant_spelling_is_not() {
        let z = format!("{TEXT}\nfuelFeedbackZones ( zoneA );\ncoolantFeedbackZones ( \"zoneB\" );\nstructFeedbackZones ( zoneA zoneC );\n");
        let nd = FoamFile::parse(&z).expect("parse");
        let np = parse("eigenvalueNeutronics false;\n");
        let rs = parse("pTarget 1e7;\n");
        let case = read_pk_case(&PkCaseDicts { nuclear: &nd.dict, neutronics_properties: &np.dict, external_source: None, reactor_state: Some(&rs.dict) }, 100.0).expect("case");
        assert_eq!(case.zones.fuel, vec!["zoneA".to_string()]);
        assert!(case.zones.cool.is_empty(), "upstream reads coolFeedbackZones, not coolantFeedbackZones");
        assert_eq!(case.zones.structure, vec!["zoneA".to_string(), "zoneC".to_string()]);
        assert_eq!(case.cfg.p_target, 1e7);
        assert!(case.gem.is_none() && case.cfg.external_source.is_none() && case.cfg.decay_power.is_none());
        let cool = format!("{TEXT}\ncoolFeedbackZones ( \"zoneB\" );\n");
        let nd = FoamFile::parse(&cool).expect("parse");
        let case = read_pk_case(&PkCaseDicts { nuclear: &nd.dict, neutronics_properties: &np.dict, external_source: None, reactor_state: Some(&rs.dict) }, 100.0).expect("case");
        assert_eq!(case.zones.cool, vec!["zoneB".to_string()]);
    }

    #[test]
    fn reactor_state_external_source_decay_and_gem_are_read() {
        let gem = format!("{TEXT}\nnuFission 2.43;\nenergyPerFission 2e8;\nGEMSodiumLevelRef 2.2;\nGEMFlowFaceZone \"gemCut\";\nGEMReactivityMap ( (0.5 1e-4) (0 0) (-0.5 -1e-4) );\n");
        let nd = FoamFile::parse(&gem).expect("parse");
        let np = parse("externalSourceNeutronics true;\nfluidRegionName fluidRegion;\n");
        let es = parse("externalSourceMode powerMonitoring;\nmodulationFactor 0.5;\npowerModulationDelay 0.01;\nnuSource 20;\nbeamEnergy 6e8;\n");
        let rs = parse(
            "pTarget 1e7;\nksrc 0.9999;\nprecursorPowers 2 ( 1.5 2.5 );\nTFuelRef 700;\nTladRef 480;\nTDriveLineRef 1;\ndecayPowerTimeProfile { type table; table table ( (0 6e5) (100 5e5) ); }\n",
        );
        let d = PkCaseDicts { nuclear: &nd.dict, neutronics_properties: &np.dict, external_source: Some(&es.dict), reactor_state: Some(&rs.dict) };
        let case = read_pk_case(&d, 100.0).expect("case");
        let ext = case.cfg.external_source.expect("external source");
        assert_eq!(ext.mode, ExternalSourceMode::PowerMonitoring);
        assert_eq!((ext.nu_source, ext.beam_energy, ext.nu_fission, ext.energy_per_fission, ext.modulation_factor, ext.power_modulation_delay), (20.0, 6e8, 2.43, 2e8, 0.5, 0.01));
        assert_eq!(case.cfg.ksrc, Some(0.9999));
        assert_eq!(case.cfg.subcritical_index, None);
        assert_eq!(case.cfg.precursor_powers, Some(vec![1.5, 2.5]));
        assert_eq!((case.refs.t_fuel, case.refs.t_clad, case.refs.t_driveline, case.refs.t_cool), (Some(700.0), Some(480.0), Some(1.0), None));
        assert_eq!(case.cfg.decay_power.expect("decay").value(Time::new::<second>(50.0)), 5.5e5);
        let g = case.gem.expect("gem");
        assert_eq!((g.fluid_region.as_str(), g.face_zone.as_str(), g.sodium_level_ref, g.map.len()), ("fluidRegion", "gemCut", 2.2, 3));
        // refusals
        let bad = parse("externalSourceMode sometimes;\n");
        assert!(read_pk_case(&PkCaseDicts { external_source: Some(&bad.dict), ..d }, 100.0).unwrap_err().contains("externalSourceMode"));
        assert!(read_pk_case(&PkCaseDicts { external_source: None, ..d }, 100.0).unwrap_err().contains("externalSource"));
        assert!(read_pk_case(&PkCaseDicts { reactor_state: None, ..d }, 100.0).unwrap_err().contains("pTarget"));
        let np2 = parse("fluidRegionName fluidRegion;\n");
        let nogem = FoamFile::parse(&format!("{TEXT}\nGEMReactivityMap ( (0.5 1e-4) (0 0) );\n")).expect("parse");
        assert!(read_pk_case(&PkCaseDicts { nuclear: &nogem.dict, neutronics_properties: &np2.dict, external_source: None, reactor_state: Some(&rs.dict) }, 1.0).unwrap_err().contains("GEMFlowFaceZone"));
        let wrong_count = parse("pTarget 1e7;\nprecursorPowers 1 ( 1.5 );\n");
        assert!(read_pk_case(&PkCaseDicts { reactor_state: Some(&wrong_count.dict), ..d }, 100.0).unwrap_err().contains("precursorPowers"));
    }

    #[test]
    fn every_time_profile_type_reads_and_matches_its_closed_form() {
        let t = |x: f64| Time::new::<second>(x);
        let get = |text: &str, name: &str| time_profile(&parse(text).dict, name).expect("read").expect("present");
        let zero = get("p { type zero; }", "p");
        let one = get("p { type one; }", "p");
        assert_eq!((zero.value(t(3.0)), one.value(t(3.0))), (0.0, 1.0));
        assert_eq!(get("p { type uniform; value 4; }", "p").value(t(3.0)), 4.0);
        let lin = get("p { type linearRamp; start 10; duration 20; }", "p");
        assert_eq!((lin.value(t(5.0)), lin.value(t(20.0)), lin.value(t(40.0))), (0.0, 0.5, 1.0));
        assert_eq!(get("p { type quadraticRamp; start 10; duration 20; }", "p").value(t(20.0)), 0.25);
        assert_eq!(get("p { type halfCosineRamp; start 0; duration 2; }", "p").value(t(1.0)), 0.5 * (1.0 - (core::f64::consts::PI * 0.5).r_cos()));
        assert!((get("p { type quarterCosineRamp; duration 2; }", "p").value(t(1.0)) - (1.0 - (0.25 * core::f64::consts::PI).r_cos())).abs() < 1e-15);
        assert!((get("p { type quarterSineRamp; duration 2; }", "p").value(t(1.0)) - (0.25 * core::f64::consts::PI).r_sin()).abs() < 1e-15);
        // sine of period 20 (frequency 0.05) about 5: peak at a quarter period
        let sine = get("p { type sine; frequency 0.05; scale 2; level 5; }", "p");
        assert!((sine.value(t(5.0)) - 7.0).abs() < 1e-12 && (sine.value(t(0.0)) - 5.0).abs() < 1e-12);
        let by_period = get("p { type cosine; period 20; amplitude 3; scale 1; level 0; t0 5; }", "p");
        assert!((by_period.value(t(5.0)) - 3.0).abs() < 1e-12);
        // square: mark 1, space 3 -> positive for a quarter of the cycle
        let square = get("p { type square; frequency 1; scale 1; level 0; mark 1; space 3; }", "p");
        assert_eq!((square.value(t(0.1)), square.value(t(0.3))), (1.0, -1.0));
        let poly = get("p { type polynomial; coeffs ( (1 0) (2 1) (3 2) ); }", "p");
        assert_eq!(poly.value(t(2.0)), 1.0 + 4.0 + 12.0);
        // the profile's own startTime shifts it (upstream `value(t - startTime_)`), the run's start time does not
        let shifted = get("p { type linearRamp; duration 10; startTime 100; }", "p");
        assert_eq!((shifted.value(t(100.0)), shifted.value(t(105.0))), (0.0, 0.5));
        assert!(time_profile(&parse("p { type tableFile; }").dict, "p").unwrap_err().contains("tableFile"));
        assert!(time_profile(&parse("p { type sine; frequency 0.05; scale ( 1 2 ); level 0; }").dict, "p").is_err());
        assert!(time_profile(&parse("p { type linearRamp; duration 0; }").dict, "p").is_err());
        assert!(time_profile(&parse("q 1;").dict, "p").expect("absent").is_none());
    }
}
