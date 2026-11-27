// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Reader of the `solvers` dictionary of a neutronics region's `fvSolution` (gh:#1887, README round 42).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream
//!
//! * OpenFOAM-v2506 `lduMatrixSolver.C:195-208` (`solver::readControls`): `tolerance` default
//!   `lduMatrix::defaultTolerance` = 1e-6, `relTol` 0, `maxIter` `lduMatrix::defaultMaxIter` = 1000, `minIter` 0,
//!   `norm` default / `L1_scaled` / `none`, `log`.
//! * `lduMatrixPreconditioner.C:42-63` (`preconditioner::getName`): a word, or a sub-dictionary holding the word.
//! * `dictionary::lookupEntry` with pattern matching: the exact keyword first, else the LAST declared pattern that
//!   matches (patterns are searched newest first).
//! * GeN-Foam `src/classes/neutronics` never sets `finalIteration` on the neutronics mesh (no `pimpleControl` loops
//!   it), so a solve never selects the `<field>Final` entry: only the plain name is looked up. Pinned by
//!   `final_entries_are_never_selected`.
//!
//! Refused by name: a regex feature other than `.`, `.*` and `|`; `norm none`; a solver / preconditioner pair other than
//! `PCG`/`DIC`, `PBiCG`/`DILU`, `PBiCGStab`/`DILU`; `GAMG` is the named deviation of round 40 (runs `PCG`/`DIC`) for the
//! flux family only, and is reported in [`NeutronFvSolution::gamg_substituted`]; a symmetric solver on an asymmetric
//! family and the converse (OpenFOAM selects from the symmetric table for a symmetric matrix).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile};

use crate::error::AppBuilderError;
use crate::genfoam::common::linear_solvers::{NeutronSolverEntries, NeutronSolverEntry, NeutronSolverKind};

fn err(file: &str, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Parse { file: file.to_string(), line: 0, msg: msg.into() }
}

/// Full-match of the OpenFOAM keyword `pattern` (a regular expression restricted to literals, `.`, `.*` and top-level
/// `|`) against `name`. `Err` names an unsupported regex feature.
///
/// # Errors
///
/// The unsupported character, when the pattern uses one.
pub fn keyword_matches(pattern: &str, name: &str) -> Result<bool, String> {
    for c in pattern.chars() {
        if matches!(c, '[' | ']' | '(' | ')' | '+' | '?' | '\\' | '^' | '$' | '{' | '}') {
            return Err(format!("regex character `{c}` in keyword `{pattern}` is not supported"));
        }
    }
    for alt in pattern.split('|') {
        let p: Vec<char> = alt.chars().collect();
        let s: Vec<char> = name.chars().collect();
        if match_here(&p, &s) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Backtracking full match of `p` (`.`, `.*`, literals) against `s`.
fn match_here(p: &[char], s: &[char]) -> bool {
    if p.is_empty() {
        return s.is_empty();
    }
    let star = p.len() >= 2 && p[1] == '*';
    if star {
        let mut k = 0;
        loop {
            if match_here(&p[2..], &s[k..]) {
                return true;
            }
            if k < s.len() && (p[0] == '.' || p[0] == s[k]) {
                k += 1;
            } else {
                return false;
            }
        }
    }
    if s.is_empty() {
        return false;
    }
    (p[0] == '.' || p[0] == s[0]) && match_here(&p[1..], &s[1..])
}

/// `dictionary::lookupEntry(name, patternMatch)`: the exact keyword, else the last declared matching pattern.
fn lookup<'a>(d: &'a FoamDict, name: &str, file: &str) -> Result<Option<&'a FoamDict>, AppBuilderError> {
    let entry = if let Some(e) = d.get(name) {
        Some(e)
    } else {
        let mut found = None;
        for (k, e) in d.iter() {
            if keyword_matches(k, name).map_err(|m| err(file, m))? {
                found = Some(e);
            }
        }
        found
    };
    match entry {
        None => Ok(None),
        Some(FoamEntry::SubDict(sd)) => Ok(Some(sd)),
        Some(_) => Err(err(file, format!("`solvers` entry for `{name}` is not a dictionary"))),
    }
}

fn word_of(d: &FoamDict, key: &str) -> Option<String> {
    match d.get(key) {
        Some(FoamEntry::Word(w)) | Some(FoamEntry::Str(w)) => Some(w.clone()),
        _ => None,
    }
}

fn number(d: &FoamDict, key: &str, file: &str) -> Result<Option<f64>, AppBuilderError> {
    match d.get(key) {
        None => Ok(None),
        Some(FoamEntry::Scalar(x)) => Ok(Some(*x)),
        Some(_) => Err(err(file, format!("`{key}` must be a number"))),
    }
}

/// Which symmetry the family's matrices have: OpenFOAM selects the solver from the symmetric or asymmetric table.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Flux,
    Precursor,
    AngularFlux,
}

fn entry_of(d: &FoamDict, name: &str, family: Family, file: &str) -> Result<(NeutronSolverEntry, bool), AppBuilderError> {
    let solver = word_of(d, "solver").ok_or_else(|| err(file, format!("`{name}`: no `solver` word")))?;
    let precon = match d.get("preconditioner") {
        Some(FoamEntry::Word(w)) => Some(w.clone()),
        Some(FoamEntry::SubDict(sd)) => word_of(sd, "preconditioner"),
        _ => None,
    };
    if let Some(n) = word_of(d, "norm") {
        if n != "default" && n != "L1_scaled" {
            return Err(err(file, format!("`{name}`: `norm {n}` is not ported (default / L1_scaled are)")));
        }
    }
    let mut gamg = false;
    let kind = match (solver.as_str(), precon.as_deref(), family) {
        ("PCG", Some("DIC"), Family::Flux) => NeutronSolverKind::PcgDic,
        ("GAMG", _, Family::Flux) => {
            gamg = true;
            NeutronSolverKind::PcgDic
        }
        ("PBiCG", Some("DILU"), Family::Precursor | Family::AngularFlux) => NeutronSolverKind::PbicgDilu,
        ("PBiCGStab", Some("DILU"), Family::Precursor | Family::AngularFlux) => NeutronSolverKind::PbicgstabDilu,
        (s, p, _) => {
            return Err(err(
                file,
                format!("`{name}`: solver `{s}` with preconditioner `{p:?}` is not ported for this equation (see the module doc)"),
            ))
        }
    };
    let mut e = NeutronSolverEntry::with_defaults(kind);
    if let Some(x) = number(d, "tolerance", file)? {
        e.tolerance = x;
    }
    if let Some(x) = number(d, "relTol", file)? {
        e.rel_tol = x;
    }
    if let Some(x) = number(d, "maxIter", file)? {
        e.max_iter = x as usize;
    }
    if let Some(x) = number(d, "minIter", file)? {
        e.min_iter = x as usize;
    }
    Ok((e, gamg))
}

/// The resolved `solvers` entries of one neutronics region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutronFvSolution {
    /// The entries per equation family.
    pub entries: NeutronSolverEntries,
    /// Whether the flux entry selects `GAMG` (run as `PCG`/`DIC`, the named deviation of round 40, gh:#1843).
    pub gamg_substituted: bool,
}

/// Resolve the entries of the `solvers` dictionary of `fvSolution` text for a region with `groups` energy groups,
/// `prec_groups` precursor groups and (SN) `directions` ordinates (0 if none), by the names upstream gives its fields:
/// `flux<g>`, `fluxStar<g>`, `fluxStar2<g>`, `prec<k>`, `precStar<k>`, `angularFlux_<g>_<d>`. A family whose names do not
/// all resolve to ONE entry is refused (the settings carry one entry per family).
///
/// # Errors
///
/// [`AppBuilderError`] for an unreadable file, a missing entry, an unported selection or a family that splits.
pub fn read_neutron_fv_solution(
    path: &std::path::Path,
    groups: usize,
    prec_groups: usize,
    directions: usize,
) -> Result<NeutronFvSolution, AppBuilderError> {
    let file = FoamFile::read_with_includes(path)
        .map_err(|e| AppBuilderError::Parse { file: path.display().to_string(), line: 0, msg: e.to_string() })?;
    resolve_neutron_fv_solution(&file.dict, &path.display().to_string(), groups, prec_groups, directions)
}

/// [`read_neutron_fv_solution`] on a parsed dictionary (`file` only labels errors).
///
/// # Errors
///
/// As [`read_neutron_fv_solution`].
pub fn resolve_neutron_fv_solution(
    dict: &FoamDict,
    file: &str,
    groups: usize,
    prec_groups: usize,
    directions: usize,
) -> Result<NeutronFvSolution, AppBuilderError> {
    let Some(FoamEntry::SubDict(solvers)) = dict.get("solvers") else {
        return Err(err(file, "no `solvers` dictionary"));
    };
    let mut flux_names = Vec::new();
    let mut prec_names = Vec::new();
    let mut ang_names = Vec::new();
    for g in 0..groups {
        flux_names.push(format!("flux{g}"));
        flux_names.push(format!("fluxStar{g}"));
        flux_names.push(format!("fluxStar2{g}"));
        for d in 0..directions {
            ang_names.push(format!("angularFlux_{g}_{d}"));
        }
    }
    for k in 0..prec_groups {
        prec_names.push(format!("prec{k}"));
        prec_names.push(format!("precStar{k}"));
    }
    let family_entry = |names: &[String], family: Family| -> Result<Option<(NeutronSolverEntry, bool)>, AppBuilderError> {
        let mut first: Option<(NeutronSolverEntry, bool)> = None;
        for name in names {
            let Some(d) = lookup(solvers, name, file)? else {
                continue; // never solved by this case (e.g. precursors of stationary fuel)
            };
            let e = entry_of(d, name, family, file)?;
            match first {
                None => first = Some(e),
                Some(f) if f == e => {}
                Some(_) => {
                    return Err(err(file, format!("`{name}` resolves to a different entry than its family's first name: not ported")))
                }
            }
        }
        Ok(first)
    };
    let flux = family_entry(&flux_names, Family::Flux)?;
    let prec = family_entry(&prec_names, Family::Precursor)?;
    let ang = family_entry(&ang_names, Family::AngularFlux)?;
    let flux_default = NeutronSolverEntry::with_defaults(NeutronSolverKind::PcgDic);
    let prec_default = NeutronSolverEntry::with_defaults(NeutronSolverKind::PbicgDilu);
    let ang_default = NeutronSolverEntry::with_defaults(NeutronSolverKind::PbicgstabDilu);
    Ok(NeutronFvSolution {
        entries: NeutronSolverEntries {
            flux: flux.map_or(flux_default, |x| x.0),
            precursor: prec.map_or(prec_default, |x| x.0),
            angular_flux: ang.map_or(ang_default, |x| x.0),
        },
        gamg_substituted: flux.is_some_and(|x| x.1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> FoamDict {
        let dir = std::env::temp_dir().join(format!("fvsol_{}", text.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("fvSolution");
        std::fs::write(&p, format!("FoamFile {{ version 2.0; format ascii; class dictionary; object fvSolution; }}\n{text}")).unwrap();
        FoamFile::read_with_includes(&p).unwrap().dict
    }

    /// The matcher: `.*`, `.`, alternation, full-match.
    #[test]
    fn keyword_matching() {
        assert!(keyword_matches("flux.*|adjoint_flux.*", "fluxStar20").unwrap());
        assert!(keyword_matches("prec.*|precStar.*", "precStar3").unwrap());
        assert!(!keyword_matches("flux.*", "xflux0").unwrap());
        assert!(keyword_matches("flux.", "flux0").unwrap());
        assert!(!keyword_matches("flux.", "flux10").unwrap());
        assert!(keyword_matches("h.*Final", "h.liquidFinal").unwrap());
        assert!(keyword_matches("flux[0-9]", "flux1").is_err());
    }

    /// Defaults where a key is absent, the last matching pattern wins, a `preconditioner` sub-dictionary is read, and a
    /// `Final` entry is never selected.
    #[test]
    fn defaults_last_pattern_and_final_entries_are_never_selected() {
        let d = parse(
            "solvers { \"flux.*\" { solver PCG; preconditioner DIC; tolerance 1e-9; relTol 0.1; minIter 2; }
              \"fluxStar.*Final\" { solver PCG; preconditioner DIC; tolerance 1e-3; }
              \"prec.*\" { solver PBiCG; preconditioner { preconditioner DILU; } }
              \"fluxStar.*\" { solver PCG; preconditioner DIC; tolerance 1e-9; relTol 0.1; minIter 2; } }",
        );
        let r = resolve_neutron_fv_solution(&d, "t", 2, 3, 0).unwrap();
        assert_eq!(r.entries.flux.tolerance, 1e-9);
        assert_eq!(r.entries.flux.rel_tol, 0.1);
        assert_eq!(r.entries.flux.min_iter, 2);
        assert_eq!(r.entries.flux.max_iter, 1000);
        assert_eq!(r.entries.precursor.tolerance, 1e-6);
        assert_eq!(r.entries.precursor.rel_tol, 0.0);
        assert_eq!(r.entries.precursor.kind, NeutronSolverKind::PbicgDilu);
        assert!(!r.gamg_substituted);
    }

    /// Refusals by name: a split family, `norm none`, an unported pair, GAMG reported (a name with no entry is skipped: never solved).
    #[test]
    fn refusals_and_gamg() {
        let split = parse("solvers { \"flux.*\" { solver PCG; preconditioner DIC; } fluxStar1 { solver PCG; preconditioner DIC; tolerance 1e-3; } }");
        assert!(resolve_neutron_fv_solution(&split, "t", 2, 0, 0).is_err());
        let norm = parse("solvers { \"flux.*\" { solver PCG; preconditioner DIC; norm none; } }");
        assert!(resolve_neutron_fv_solution(&norm, "t", 1, 0, 0).is_err());
        let sm = parse("solvers { \"flux.*\" { solver smoothSolver; smoother GaussSeidel; } }");
        assert!(resolve_neutron_fv_solution(&sm, "t", 1, 0, 0).is_err());
    
        let gamg = parse("solvers { \"flux.*\" { solver GAMG; tolerance 1e-8; smoother DICGaussSeidel; } }");
        let r = resolve_neutron_fv_solution(&gamg, "t", 1, 0, 0).unwrap();
        assert!(r.gamg_substituted);
        assert_eq!(r.entries.flux.kind, NeutronSolverKind::PcgDic);
    }
}
