// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream files:   HEN_HOUSE/egs++/estar/estar.cpp (egsCompoundsToElements,
//                     GetElements), estar_formulaCalcs.cpp (fcalc,
//                     getEgsMediaData, mixtureCalculation), estar_dataParser.cpp
//                     (parse), estar_dataTables.cpp (atb, atomic_number)
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Upstream licence: AGPL-3.0-or-later, Copyright (C) 2026 National Research
//                     Council Canada (authors: Sehmimul Hoque, Martin J. Berger,
//                     Johnathan S. Coursey, Reid Townson, Ernesto Mainegra-Hing)
//   This port:        AGPL-3.0-only, notice preserved here.
//! **`egs_compoundstoelements`**: expand a `mixture compounds` medium to
//! elements (`HEN_HOUSE/egs++/estar/estar.cpp`), the routine
//! `get_media_inputs.mortran:1720` calls. Upstream path
//! `HEN_HOUSE/egs++/estar/estar.cpp`.
//!
//! **Status: first port, not differentially tested (gh:#1691).** The source
//! was not found earlier because the clone held only `HEN_HOUSE/src` and
//! `pegs4`: it lives in the egs++ ESTAR sources (found 2026-11-21).
//!
//! Ported: `GetElements::getElemArray` (split the 50-column Fortran string on
//! blanks, capitalise the first character and lower-case the second when a
//! token is longer than one character, as upstream does: `NACL` becomes
//! `NaCL`, and `CH4` becomes `Ch4`, an unknown symbol, so a formula whose second
//! character is an uppercase letter must be written with a digit, `C1H4`; an
//! upstream oddity ported as it is), `parse` (a formula such as `MgCl2` into symbols and counts),
//! `fcalc`'s mass-fraction part, `getEgsMediaData` (normalise the fractions)
//! and `mixtureCalculation`'s element accumulation (ascending `Z`).
//!
//! Not ported: the I-value of the mixture (`fc.pot`, `fc.zav` and the
//! `poth`/`potgas`/`potcon` tables) which `egsCompoundsToElements` computes
//! but never returns, and `egs_estar` (the density-effect calculation).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::EgsError;
use alloc::string::String;
use alloc::vec::Vec;

/// `atb[]`: atomic mass of element `Z` at index `Z - 1`
/// (`estar_dataTables.cpp`), `Z = 1..=100`.
const ATB: [f64; 100] = [
    1.007940, 4.0026020, 6.9410, 9.0121820, 10.8110, 12.0110,
    14.006740, 15.99940, 18.99840320, 20.17970, 22.9897680, 24.30500,
    26.9815390, 28.08550, 30.9737620, 32.0660, 35.45270, 39.9480,
    39.09830, 40.0780, 44.9559100, 47.880, 50.94150, 51.99610,
    54.938050, 55.8470, 58.933200, 58.690, 63.5460, 65.390,
    69.7230, 72.610, 74.921590, 78.960, 79.9040, 83.800,
    85.46780, 87.620, 88.905850, 91.2240, 92.906380, 95.940,
    97.90720, 101.070, 102.90550, 106.420, 107.86820, 112.4110,
    114.820, 118.7100, 121.750, 127.600, 126.904470, 131.290,
    132.905430, 137.3270, 138.90550, 140.1150, 140.907650, 144.240,
    144.91270, 150.360, 151.9650, 157.250, 158.925340, 162.500,
    164.930320, 167.260, 168.934210, 173.040, 174.9670, 178.490,
    180.94790, 183.850, 186.2070, 190.20, 192.220, 195.080,
    196.966540, 200.590, 204.38330, 207.20, 208.980370, 208.98240,
    209.98710, 222.01760, 223.01970, 226.02540, 227.02780, 232.03810,
    231.035880, 238.02890, 237.04820, 239.05220, 243.06140, 247.07030,
    247.07030, 251.07960, 252.0830, 257.09510,
];

/// Element symbols, `SYMBOLS[Z - 1]` (`atomic_number` in `estar_dataTables.cpp`).
const SYMBOLS: [&str; 100] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne",
    "Na", "Mg", "Al", "Si", "P", "S", "Cl", "Ar", "K", "Ca",
    "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn",
    "Ga", "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr",
    "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In", "Sn",
    "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd",
    "Pm", "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb",
    "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg",
    "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th",
    "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk", "Cf", "Es", "Fm",
];

/// One element of an expanded mixture: atomic number and mass fraction of the
/// whole mixture (`zelem`, `rhoz`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixtureElement {
    /// Atomic number.
    pub z: i32,
    /// Mass fraction of the whole mixture.
    pub rhoz: f64,
}

/// `atom_num`: atomic number of an element symbol.
fn atom_num(symbol: &str) -> Result<usize, EgsError> {
    SYMBOLS.iter().position(|s| *s == symbol).map(|i| i + 1).ok_or(EgsError::Fatal("estar::atom_num: unrecognised element symbol"))
}

/// `parse` (`estar_dataParser.cpp`): symbols and atom counts of a formula.
pub fn parse_formula(formula: &str) -> Result<Vec<(String, f64)>, EgsError> {
    let b = formula.as_bytes();
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if out.len() >= 100 {
            return Err(EgsError::Fatal("estar::parse: element type count exceeded 100"));
        }
        if !b[i].is_ascii_uppercase() {
            return Err(EgsError::Fatal("estar::parse: formula is malformed (element symbols begin with an uppercase letter)"));
        }
        let sym_len = if i + 1 < b.len() && b[i + 1].is_ascii_lowercase() { 2 } else { 1 };
        let symbol = String::from(&formula[i..i + sym_len]);
        i += sym_len;
        let mut count = 1.0_f64;
        if i < b.len() && b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i - start > 9 {
                return Err(EgsError::Fatal("estar::parse: atom count has more than 9 digits"));
            }
            let val: i64 = formula[start..i].parse().map_err(|_| EgsError::Fatal("estar::parse: invalid atom count"))?;
            if val <= 0 || val > 99 {
                return Err(EgsError::Fatal("estar::parse: atom count must be 1..=99"));
            }
            count = val as f64;
        }
        out.push((symbol, count));
    }
    Ok(out)
}

/// `fcalc` mass-fraction part: `(Z, mass fraction)` of each element of one
/// formula, in formula order (upstream does not merge repeated symbols here).
fn formula_weights(formula: &str) -> Result<Vec<(usize, f64)>, EgsError> {
    let parsed = parse_formula(formula)?;
    let mut zs = Vec::with_capacity(parsed.len());
    let mut asum = 0.0_f64;
    for (sym, n) in &parsed {
        let z = atom_num(sym)?;
        zs.push((z, *n));
        asum += ATB[z - 1] * n;
    }
    if asum == 0.0 {
        return Err(EgsError::Fatal("estar::fcalc: total atomic mass sum is zero"));
    }
    Ok(zs.into_iter().map(|(z, n)| (z, ATB[z - 1] * (n / asum))).collect())
}

/// `GetElements::getElemArray`: split a blank-separated component string and
/// normalise the capitalisation of two-character-or-longer tokens.
pub fn element_array(formula_str: &str, nep: usize) -> Result<Vec<String>, EgsError> {
    let mut comps: Vec<String> = formula_str.split(' ').filter(|s| !s.is_empty()).map(String::from).collect();
    if comps.len() < nep {
        return Err(EgsError::Fatal("estar::getElemArray: list of elements is inconsistent with the number expected"));
    }
    comps.truncate(nep);
    for c in comps.iter_mut() {
        if c.len() > 1 {
            let mut chars: Vec<char> = c.chars().collect();
            chars[0] = chars[0].to_ascii_uppercase();
            chars[1] = chars[1].to_ascii_lowercase();
            *c = chars.into_iter().collect();
        }
    }
    Ok(comps)
}

/// `egsCompoundsToElements`: expand compounds with mass fractions into the
/// elements of the mixture, ascending `Z`. `compounds` are the formulas (for
/// example `H2O`), `mass_fractions` their (unnormalised) mass fractions.
///
/// Upstream also takes the medium density and requires it positive, and
/// requires `1..=100` compounds and at most 50 distinct elements
/// (`$MXEL`); all three are checked here.
pub fn compounds_to_elements(compounds: &[String], mass_fractions: &[f64], media_density: f32) -> Result<Vec<MixtureElement>, EgsError> {
    if compounds.is_empty() || compounds.len() > 100 || compounds.len() != mass_fractions.len() {
        return Err(EgsError::Fatal("estar::egsCompoundsToElements: ncomp out of range [1, 100]"));
    }
    if media_density <= 0.0 {
        return Err(EgsError::Fatal("estar::egsCompoundsToElements: mediaDensity must be > 0"));
    }
    // getEgsMediaData
    let mut sumf = 0.0_f64;
    for f in mass_fractions {
        if *f <= 0.0 {
            return Err(EgsError::Fatal("estar::getEgsMediaData: mass fractions must be > 0"));
        }
        sumf += *f;
    }
    // mixtureCalculation
    let mut present = [false; 100];
    let mut wate = [0.0_f64; 100];
    for (i, formula) in compounds.iter().enumerate() {
        let frac = mass_fractions[i] / sumf;
        for (z, wt) in formula_weights(formula)? {
            let k = z - 1;
            if !present[k] {
                present[k] = true;
                wate[k] = frac * wt;
            } else {
                wate[k] += frac * wt;
            }
        }
    }
    let out: Vec<MixtureElement> = (0..100).filter(|k| present[*k]).map(|k| MixtureElement { z: (k + 1) as i32, rhoz: wate[k] }).collect();
    if out.len() > 50 {
        return Err(EgsError::Fatal("estar::egsCompoundsToElements: more than 50 distinct elements"));
    }
    Ok(out)
}

/// The symbol of element `z` (`1..=100`), as upstream writes it into
/// `elementStr`.
pub fn symbol(z: i32) -> Option<&'static str> {
    if (1..=100).contains(&z) { Some(SYMBOLS[(z - 1) as usize]) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_one_hundred_entries_in_order() {
        assert_eq!(ATB.len(), 100);
        assert_eq!(atom_num("H").unwrap(), 1);
        assert_eq!(atom_num("U").unwrap(), 92);
        assert_eq!(atom_num("Fm").unwrap(), 100);
        assert!(atom_num("Xx").is_err());
        assert_eq!(symbol(8), Some("O"));
        assert_eq!(symbol(0), None);
    }

    #[test]
    fn parse_reads_symbols_and_counts() {
        assert_eq!(parse_formula("MgCl2").unwrap(), vec![(String::from("Mg"), 1.0), (String::from("Cl"), 2.0)]);
        assert_eq!(parse_formula("H2O").unwrap(), vec![(String::from("H"), 2.0), (String::from("O"), 1.0)]);
        assert!(parse_formula("h2o").is_err());
        assert!(parse_formula("H0").is_err());
        assert!(parse_formula("H100").is_err());
    }

    #[test]
    fn element_array_normalises_case_like_upstream() {
        assert_eq!(element_array("  H2O   NACL  ", 2).unwrap(), vec![String::from("H2O"), String::from("NaCL")]);
        assert!(element_array("H2O", 2).is_err());
    }

    /// Water: mass fractions from the tabulated atomic masses.
    #[test]
    fn water_expands_to_hydrogen_and_oxygen() {
        let m = compounds_to_elements(&[String::from("H2O")], &[2.0], 1.0).unwrap();
        assert_eq!(m.iter().map(|e| e.z).collect::<Vec<_>>(), vec![1, 8]);
        let asum = 2.0 * ATB[0] + ATB[7];
        assert!((m[0].rhoz - ATB[0] * (2.0 / asum)).abs() < 1e-15);
        assert!((m[0].rhoz + m[1].rhoz - 1.0).abs() < 1e-14);
    }

    /// A mixture of two compounds sharing an element merges it and
    /// normalises the fractions.
    #[test]
    fn mixture_merges_shared_elements() {
        let m = compounds_to_elements(&[String::from("H2O"), String::from("C1H4")], &[1.0, 3.0], 1.0).unwrap();
        assert_eq!(m.iter().map(|e| e.z).collect::<Vec<_>>(), vec![1, 6, 8]);
        let total: f64 = m.iter().map(|e| e.rhoz).sum();
        assert!((total - 1.0).abs() < 1e-14);
        assert!(compounds_to_elements(&[String::from("H2O")], &[0.0], 1.0).is_err());
        assert!(compounds_to_elements(&[String::from("H2O")], &[1.0], 0.0).is_err());
        assert!(compounds_to_elements(&[], &[], 1.0).is_err());
    }

    /// Differential test against the compiled upstream `mixtureCalculation`
    /// (`EGSNRC_REFERENCE_ESTAR`: lines `case N f1:w1 ... | nel Z wt ...`,
    /// written by a small C++ harness linked to `estar_formulaCalcs.cpp`,
    /// `estar_dataParser.cpp`, `estar_dataTables.cpp` with `egsFatal` stubbed).
    /// Prints the worst relative difference; results in
    /// `docs/egsnrc_phase2_differential.md`.
    #[test]
    fn differential_estar_mixture_against_compiled_egsnrc() {
        let Ok(path) = std::env::var("EGSNRC_REFERENCE_ESTAR") else {
            eprintln!("EGSNRC_REFERENCE_ESTAR not set: differential test skipped");
            return;
        };
        let (mut n, mut worst) = (0usize, 0.0_f64);
        for l in std::fs::read_to_string(path).unwrap().lines() {
            let (lhs, rhs) = l.split_once(" | ").unwrap();
            let mut comps = Vec::new();
            let mut fr = Vec::new();
            for tok in lhs.split_whitespace().skip(2) {
                let (f, w) = tok.split_once(':').unwrap();
                comps.push(String::from(f));
                fr.push(w.parse::<f64>().unwrap());
            }
            let r: Vec<&str> = rhs.split_whitespace().collect();
            let nel: usize = r[0].parse().unwrap();
            let mine = compounds_to_elements(&comps, &fr, 1.0).unwrap();
            assert_eq!(mine.len(), nel, "{l}");
            for k in 0..nel {
                assert_eq!(mine[k].z, r[1 + 2 * k].parse::<i32>().unwrap(), "{l}");
                let want: f64 = r[2 + 2 * k].parse().unwrap();
                worst = worst.max((mine[k].rhoz - want).abs() / want);
            }
            n += 1;
        }
        eprintln!("{n} mixtures: worst relative difference in mass fraction {worst:e}");
        assert!(n > 0);
    }
}
