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
//! The I-value of the mixture (`fc.pot`, `fc.zav` and the
//! `poth`/`potgas`/`potcon` tables) which `egsCompoundsToElements` computes
//! but never returns is ported below as `fcalc`.
//!
//! **`egs_estar`** (`estarCalculation`, the density-effect correction) is
//! ported too (gh:#1696) and is bit-identical to the compiled upstream on 17
//! media (`docs/egsnrc_phase2_differential.md`), and so is
//! `outputDensityFile` (as `density_file_text`, whose text equals the file the
//! compiled upstream writes). Nothing of `estar.cpp` is left unported.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::EgsError;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use petir::mathf::RealMath;
use petir::real::Real;

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

// ---------------------------------------------------------------------------
// egs_estar: the density-effect calculation (`estar.cpp:estarCalculation`).
// ---------------------------------------------------------------------------

/// x87 80-bit `long double` arithmetic (64-bit mantissa, round to nearest
/// even), for the `vector<long double> q` that `estarCalculation` builds on
/// x86-64 Linux. Only the operations upstream uses on it are provided:
/// conversion from and to `double`, `+`, `-`, `*`, `/`. Values stay in the
/// normal range, so no subnormals or infinities are modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ext {
    neg: bool,
    exp: i32,
    mant: u64,
}

impl Ext {
    const ZERO: Ext = Ext { neg: false, exp: 0, mant: 0 };

    /// Normalise and round `m * 2^exp` (plus a positive fraction below one
    /// unit of `m` when `sticky`) to a 64-bit mantissa.
    fn pack(neg: bool, m: u128, exp: i32, sticky: bool) -> Ext {
        if m == 0 {
            return Ext::ZERO;
        }
        let p = 127 - m.leading_zeros() as i32;
        if p < 64 {
            let sh = 63 - p;
            return Ext { neg, exp: exp - sh, mant: (m << sh) as u64 };
        }
        let sh = (p - 63) as u32;
        let mut kept = (m >> sh) as u64;
        let rest = m & ((1u128 << sh) - 1);
        let half = 1u128 << (sh - 1);
        let up = rest > half || (rest == half && (sticky || kept & 1 == 1));
        let mut e = exp + sh as i32;
        if up {
            if kept == u64::MAX {
                kept = 1u64 << 63;
                e += 1;
            } else {
                kept += 1;
            }
        }
        Ext { neg, exp: e, mant: kept }
    }

    fn from_f64(x: f64) -> Ext {
        if x == 0.0 {
            return Ext::ZERO;
        }
        let bits = x.to_bits();
        let e = ((bits >> 52) & 0x7ff) as i32;
        let m = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
        Ext::pack(bits >> 63 == 1, m as u128, e - 1075, false)
    }

    fn to_f64(self) -> f64 {
        if self.mant == 0 {
            return 0.0;
        }
        let mut kept = self.mant >> 11;
        let rest = self.mant & 0x7ff;
        if rest > 0x400 || (rest == 0x400 && kept & 1 == 1) {
            kept += 1;
        }
        let mut e2 = self.exp + 11;
        if kept == 1u64 << 53 {
            kept = 1u64 << 52;
            e2 += 1;
        }
        let biased = (e2 + 52 + 1023) as u64;
        let bits = ((self.neg as u64) << 63) | (biased << 52) | (kept & ((1u64 << 52) - 1));
        f64::from_bits(bits)
    }

    fn mul(self, o: Ext) -> Ext {
        if self.mant == 0 || o.mant == 0 {
            return Ext::ZERO;
        }
        Ext::pack(self.neg != o.neg, self.mant as u128 * o.mant as u128, self.exp + o.exp, false)
    }

    fn div(self, o: Ext) -> Ext {
        if self.mant == 0 {
            return Ext::ZERO;
        }
        let num = (self.mant as u128) << 64;
        let d = o.mant as u128;
        let (q, rem) = (num / d, num % d);
        let r4 = rem << 2;
        let q2 = (q << 2) | (r4 / d);
        Ext::pack(self.neg != o.neg, q2, self.exp - 64 - o.exp - 2, r4 % d != 0)
    }

    fn add(self, o: Ext) -> Ext {
        if self.mant == 0 {
            return o;
        }
        if o.mant == 0 {
            return self;
        }
        let (big, small) = if (self.exp, self.mant) >= (o.exp, o.mant) { (self, o) } else { (o, self) };
        let d = (big.exp - small.exp) as u32;
        let bx = (big.mant as u128) << 62;
        let sx = (small.mant as u128) << 62;
        let (s, sticky) = if d == 0 {
            (sx, false)
        } else if d >= 127 {
            (0, true)
        } else {
            (sx >> d, sx & ((1u128 << d) - 1) != 0)
        };
        if big.neg == small.neg {
            Ext::pack(big.neg, bx + s, big.exp - 62, sticky)
        } else {
            let diff = bx - s - sticky as u128;
            Ext::pack(big.neg, diff, big.exp - 62, sticky)
        }
    }

    fn sub(self, o: Ext) -> Ext {
        self.add(Ext { neg: !o.neg, ..o })
    }
}

/// The six tables `estar_dataTables.cpp` holds besides `atb`/`atomic_number`
/// (which are in this file). **Read from the upstream source text at run
/// time and never committed**: `elementData` alone is 14532 numbers of an
/// AGPL data file (AGENTS.md, "Differential testing").
#[derive(Debug, Clone)]
pub struct EstarTables {
    /// `energy_grid[113]`, MeV.
    pub energy_grid: Vec<f64>,
    /// `poth[100]`, I-values (eV) of the elements.
    pub poth: Vec<f64>,
    /// `potgas[9]`.
    pub potgas: Vec<f64>,
    /// `potcon[9]`.
    pub potcon: Vec<f64>,
    /// `elementData[14532]`.
    pub element_data: Vec<f64>,
}

/// The numbers of `const double <name>[...] = { ... };` in C++ source text.
fn extract_array(text: &str, name: &str) -> Result<Vec<f64>, EgsError> {
    let mut key = String::from("const double ");
    key.push_str(name);
    key.push('[');
    let start = text.find(key.as_str()).ok_or(EgsError::Fatal("estar tables: array not found"))?;
    let rest = &text[start..];
    let open = rest.find('{').ok_or(EgsError::Fatal("estar tables: no opening brace"))?;
    let close = rest.find("};").ok_or(EgsError::Fatal("estar tables: no closing brace"))?;
    let mut out = Vec::new();
    for line in rest[open + 1..close].lines() {
        let code = line.split("//").next().unwrap_or("");
        for tok in code.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()) {
            out.push(tok.parse::<f64>().map_err(|_| EgsError::Fatal("estar tables: bad number"))?);
        }
    }
    Ok(out)
}

impl EstarTables {
    /// Parse `HEN_HOUSE/egs++/estar/estar_dataTables.cpp`. Checks the array
    /// lengths the header declares (113, 100, 9, 9, 14532).
    pub fn from_upstream_source(text: &str) -> Result<Self, EgsError> {
        let t = EstarTables {
            energy_grid: extract_array(text, "energy_grid")?,
            poth: extract_array(text, "poth")?,
            potgas: extract_array(text, "potgas")?,
            potcon: extract_array(text, "potcon")?,
            element_data: extract_array(text, "elementData")?,
        };
        // C++ zero-fills an array whose initialiser is shorter than its
        // declared size: upstream supplies 14530 numbers for `[14532]`.
        let mut t = t;
        if t.element_data.len() <= 14532 {
            t.element_data.resize(14532, 0.0);
        }
        if t.energy_grid.len() != 113 || t.poth.len() != 100 || t.potgas.len() != 9 || t.potcon.len() != 9 || t.element_data.len() != 14532 {
            return Err(EgsError::Fatal("estar tables: unexpected array length"));
        }
        Ok(t)
    }
}

/// `formula_calc`: what `fcalc` and `mixtureCalculation` return.
#[derive(Debug, Clone)]
pub struct FormulaCalc {
    /// Atomic numbers of the elements present.
    pub jz: Vec<usize>,
    /// Their mass fractions.
    pub wt: Vec<f64>,
    /// `Z/A` of the substance.
    pub zav: f64,
    /// Mean excitation energy, eV.
    pub pot: f64,
}

/// `fcalc` (`estar_formulaCalcs.cpp`): I-value, `Z/A` and mass fractions of
/// one element or compound formula. `knmat`: 0 element, 1 compound, 2 mixture
/// component.
pub fn fcalc(knmat: i32, rho: f64, formula: &str, t: &EstarTables) -> Result<FormulaCalc, EgsError> {
    let parsed = parse_formula(formula)?;
    let mut jz = Vec::with_capacity(parsed.len());
    let mut nz = Vec::with_capacity(parsed.len());
    for (sym, n) in &parsed {
        jz.push(atom_num(sym)?);
        nz.push(*n);
    }
    let mmax = jz.len();
    let mut asum = 0.0_f64;
    for m in 0..mmax {
        asum += ATB[jz[m] - 1] * nz[m];
    }
    if asum == 0.0 {
        return Err(EgsError::Fatal("estar::fcalc: total atomic mass sum is zero"));
    }
    let wt: Vec<f64> = (0..mmax).map(|m| ATB[jz[m] - 1] * (nz[m] / asum)).collect();
    let (mut zav, mut potl) = (0.0_f64, 0.0_f64);
    let rhocut = 0.1_f64;
    for m in 0..mmax {
        let jm = jz[m];
        let za = jm as f64 / ATB[jm - 1];
        zav += wt[m] * za;
        let potm = if knmat >= 1 {
            if jm >= 10 {
                if mmax > 1 { 1.13 * t.poth[jm - 1] } else { t.poth[jm - 1] }
            } else if rho <= rhocut {
                t.potgas[jm - 1]
            } else {
                t.potcon[jm - 1]
            }
        } else {
            t.poth[jm - 1]
        };
        if potm <= 0.0 {
            return Err(EgsError::Fatal("estar::fcalc: I-value is non-positive"));
        }
        potl += wt[m] * za * RealMath::r_ln(potm);
    }
    if zav == 0.0 {
        return Err(EgsError::Fatal("estar::fcalc: mean Z/A is zero"));
    }
    Ok(FormulaCalc { jz, wt, zav, pot: RealMath::r_exp(potl / zav) })
}

/// `mixtureCalculation`: components given as formulas with mass fractions.
fn mixture_calculation(rho: f64, formulas: &[String], mass_fraction: &[f64], t: &EstarTables) -> Result<FormulaCalc, EgsError> {
    if formulas.is_empty() || formulas.len() != mass_fraction.len() {
        return Err(EgsError::Fatal("estar::getEgsMediaData: number of components must be > 0"));
    }
    let mut sumf = 0.0_f64;
    for f in mass_fraction {
        if *f <= 0.0 {
            return Err(EgsError::Fatal("estar::getEgsMediaData: mass fractions must be > 0"));
        }
        sumf += *f;
    }
    let frac: Vec<f64> = mass_fraction.iter().map(|f| *f / sumf).collect();
    let (mut present, mut wate) = ([false; 100], [0.0_f64; 100]);
    let (mut zavs, mut pots) = (Vec::new(), Vec::new());
    for (i, formula) in formulas.iter().enumerate() {
        let fc = fcalc(2, rho, formula, t)?;
        for j in 0..fc.jz.len() {
            let k = fc.jz[j] - 1;
            if !present[k] {
                present[k] = true;
                wate[k] = frac[i] * fc.wt[j];
            } else {
                wate[k] += frac[i] * fc.wt[j];
            }
        }
        zavs.push(fc.zav);
        pots.push(fc.pot);
    }
    let jz: Vec<usize> = (0..100).filter(|k| present[*k]).map(|k| k + 1).collect();
    let wt: Vec<f64> = jz.iter().map(|z| wate[z - 1]).collect();
    let (mut zav, mut potl) = (0.0_f64, 0.0_f64);
    for i in 0..formulas.len() {
        zav += frac[i] * zavs[i];
        potl += frac[i] * zavs[i] * RealMath::r_ln(pots[i]);
    }
    if zav == 0.0 {
        return Err(EgsError::Fatal("estar::mixtureCalculation: mixture mean Z/A is zero"));
    }
    Ok(FormulaCalc { jz, wt, zav, pot: RealMath::r_exp(potl / zav) })
}

/// `compFormulaPreprocess::compRes` + `getCompFormula`: merge repeated
/// elements (float atom counts, ascending `Z`) and write the formula string.
fn compound_formula(elems: &[String], num_atoms: &[f32]) -> Result<String, EgsError> {
    let mut present = [false; 100];
    let mut atoms = [0.0_f32; 100];
    let mut name: [Option<String>; 100] = core::array::from_fn(|_| None);
    for (i, e) in elems.iter().enumerate() {
        let k = atom_num(e)? - 1;
        if !present[k] {
            present[k] = true;
            atoms[k] = num_atoms[i];
            name[k] = Some(e.clone());
        } else {
            atoms[k] += num_atoms[i];
        }
    }
    let mut out = String::new();
    for k in 0..100 {
        if present[k] {
            out.push_str(name[k].as_deref().unwrap_or(""));
            out.push_str(&(atoms[k] as i32).to_string());
        }
    }
    Ok(out)
}

/// `getDataFromFormulae`.
fn data_from_formulae(knmat: i32, rho: f64, elems: &[String], mass_fraction: &[f64], num_atoms: &[f32], t: &EstarTables) -> Result<FormulaCalc, EgsError> {
    match knmat {
        0 => fcalc(0, rho, &elems[0], t),
        1 => fcalc(1, rho, &compound_formula(elems, num_atoms)?, t),
        _ => mixture_calculation(rho, elems, mass_fraction, t),
    }
}

/// `fscof`: natural cubic spline coefficients `(a, b, c, d)`.
fn fscof(nmax: usize, x: &[f64], f: &[f64]) -> Result<[Vec<f64>; 4], EgsError> {
    if nmax < 2 {
        return Err(EgsError::Fatal("estar::fscof: needs at least 2 points"));
    }
    let (mut a, mut b, mut c, mut d) = (vec![0.0_f64; nmax], vec![0.0_f64; nmax], vec![0.0_f64; nmax], vec![0.0_f64; nmax]);
    let m2 = nmax - 1;
    let mut s = 0.0_f64;
    let mut r;
    for m in 0..m2 {
        d[m] = x[m + 1] - x[m];
        if d[m] == 0.0 {
            return Err(EgsError::Fatal("estar::fscof: zero interval"));
        }
        r = (f[m + 1] - f[m]) / d[m];
        c[m] = r - s;
        s = r;
    }
    s = 0.0;
    r = 0.0;
    c[0] = 0.0;
    c[nmax - 1] = 0.0;
    for m in 1..m2 {
        c[m] += r * c[m - 1];
        b[m] = (x[m - 1] - x[m + 1]) * 2.0 - r * s;
        if b[m] == 0.0 {
            return Err(EgsError::Fatal("estar::fscof: zero pivot"));
        }
        s = d[m];
        r = s / b[m];
    }
    let mut mr = m2 - 1;
    for _ in 1..m2 {
        c[mr] = (d[mr] * c[mr + 1] - c[mr]) / b[mr];
        mr -= 1;
    }
    for m in 0..m2 {
        s = d[m];
        r = c[m + 1] - c[m];
        d[m] = r / s;
        c[m] *= 3.0;
        b[m] = (f[m + 1] - f[m]) / s - (c[m] + r) * s;
        a[m] = f[m];
    }
    Ok([a, b, c, d])
}

/// `fbspol`: evaluate the spline at `s`; returns the bracketing indices
/// (`lb`, `ub`). (Upstream's out-of-range `egsWarning` extrapolation
/// messages are not reproduced.)
fn fbspol(s: f64, x: &[f64], a: &[f64], b: &[f64], c: &[f64], d: &[f64]) -> Result<(usize, usize), EgsError> {
    let n = x.len() as i64;
    if n < 2 {
        return Err(EgsError::Fatal("estar::fbspol: need at least 2 points"));
    }
    let (idir, mlb, mub): (i64, i64, i64) = if x[0] <= x[(n - 1) as usize] { (0, 0, n) } else { (1, n, 0) };
    let mut mu;
    if s > x[(mub + idir - 1) as usize] {
        mu = mub + 2 * idir - 1;
    } else if s < x[(mlb + 1 - idir - 1) as usize] {
        mu = mlb - 2 * idir + 1;
    } else {
        let (mut ml, mut hi) = (mlb, mub);
        loop {
            let mav = (ml + hi) / 2;
            if s < x[mav as usize] {
                hi = mav;
            } else {
                ml = mav;
            }
            if (hi - ml).abs() <= 1 {
                break;
            }
        }
        mu = hi + idir - 1;
    }
    mu += 1;
    if mu > n - 1 {
        mu = n - 1;
    }
    let _ = (a, b, c, d); // g (the spline value) is `solver = 1`, unused with `solver = 2`
    Ok(((mu - 1) as usize, mu as usize))
}

/// `objective_function`.
fn objective_function(tau: f64, f: &[f64], eps: &[f64], nmax: usize, x: f64) -> Result<f64, EgsError> {
    let arg = tau * (tau + 2.0);
    if arg <= 0.0 {
        return Err(EgsError::Fatal("estar::objective_function: tau*(tau+2) is non-positive"));
    }
    let yl = RealMath::r_ln(arg);
    let mut yql = 0.0_f64;
    for i in 0..nmax {
        let denom = eps[i] + x;
        if denom == 0.0 {
            return Err(EgsError::Fatal("estar::objective_function: eps + x is zero"));
        }
        yql += f[i] / denom;
    }
    if yql <= 0.0 {
        return Err(EgsError::Fatal("estar::objective_function: yql is non-positive"));
    }
    Ok(yl - RealMath::r_ln(1.0 / yql))
}

/// `bisec`.
fn bisec(mut lower: f64, mut upper: f64, tolerance: f64, tau: f64, f: &[f64], eps: &[f64], nmax: usize) -> Result<f64, EgsError> {
    if tolerance <= 0.0 {
        return Err(EgsError::Fatal("estar::bisec: tolerance must be positive"));
    }
    let mut f_lower = objective_function(tau, f, eps, nmax, lower)?;
    let f_upper = objective_function(tau, f, eps, nmax, upper)?;
    if (f_lower > 0.0) == (f_upper > 0.0) {
        return Err(EgsError::Fatal("estar::bisec: bounds do not bracket a root"));
    }
    let mut x_mid = lower;
    let mut del = 1.0_f64;
    while del > tolerance {
        x_mid = (lower + upper) / 2.0;
        let f_mid = objective_function(tau, f, eps, nmax, x_mid)?;
        if (f_mid > 0.0) == (f_lower > 0.0) {
            lower = x_mid;
            f_lower = f_mid;
        } else {
            upper = x_mid;
        }
        del = f_mid.abs();
    }
    Ok(x_mid)
}

/// C++ `iostream` default floating output with `setprecision(prec)`: `%.{prec}g`.
fn fmt_g(x: f64, prec: usize) -> String {
    if x == 0.0 {
        return String::from(if x.is_sign_negative() { "-0" } else { "0" });
    }
    let sci = alloc::format!("{:.*e}", prec - 1, x);
    let (mant, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let ex: i32 = exp.parse().unwrap_or(0);
    let strip = |t: &str| -> String {
        if t.contains('.') { String::from(t.trim_end_matches('0').trim_end_matches('.')) } else { String::from(t) }
    };
    if ex < -4 || ex >= prec as i32 {
        let sign = if ex < 0 { '-' } else { '+' };
        let mut out = strip(mant);
        out.push('e');
        out.push(sign);
        if ex.abs() < 10 {
            out.push('0');
        }
        out.push_str(&ex.abs().to_string());
        out
    } else {
        strip(&alloc::format!("{:.*}", (prec as i32 - 1 - ex) as usize, x))
    }
}

/// C++ `iostream` `scientific` output with `setprecision(prec)`: `%.{prec}e`.
fn fmt_e(x: f64, prec: usize) -> String {
    let sci = alloc::format!("{:.*e}", prec, x);
    let (mant, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let ex: i32 = exp.parse().unwrap_or(0);
    let mut out = String::from(mant);
    out.push('e');
    out.push(if ex < 0 { '-' } else { '+' });
    if ex.abs() < 10 {
        out.push('0');
    }
    out.push_str(&ex.abs().to_string());
    out
}

/// `getFileNameWithoutExtension` (`estar.cpp:411`): the part after the last
/// `/` without its last extension, and `""` when the name has no `/`
/// (upstream's behaviour, kept).
fn file_name_without_extension(s: &str) -> String {
    match s.rfind('/') {
        Some(i) => {
            let name = &s[i + 1..];
            match name.rfind('.') {
                Some(d) => String::from(&name[..d]),
                None => String::from(name),
            }
        }
        None => String::new(),
    }
}

/// `outputDensityFile` (`estar.cpp:428-486`): the text of the density
/// correction file that `egsEstar` writes for `output_filename` (cut at its
/// first blank, as upstream does); `None` when the name is empty (upstream
/// then writes nothing). The caller writes the file, so the library does no I/O.
pub fn density_file_text(media_density: f32, r: &EstarResult, output_filename: &str) -> Option<String> {
    let name = output_filename.split(|c| c == ' ' || c == '\n' || c == '\r' || c == '\t').next().unwrap_or("");
    if name.is_empty() {
        return None;
    }
    let mut o = String::new();
    o.push_str(&file_name_without_extension(name));
    o.push('\n');
    o.push_str("113 ");
    o.push_str(&fmt_g(r.mean_ival as f64, 8));
    o.push(' ');
    o.push_str(&fmt_g(media_density as f64, 8));
    o.push(' ');
    o.push_str(&r.fc.jz.len().to_string());
    o.push('\n');
    let mmax = r.fc.jz.len();
    for k in 0..mmax {
        o.push_str(&r.fc.jz[k].to_string());
        o.push(' ');
        o.push_str(&fmt_g(r.fc.wt[k], 8));
        if (k + 1) % 6 == 0 || k + 1 == mmax {
            o.push('\n');
        } else {
            o.push(' ');
        }
    }
    for i in 0..113 {
        o.push_str(&fmt_e(r.en_grid[i], 2));
        o.push(',');
        o.push_str(&fmt_e(r.density_corr[i], 3));
        if (i + 1) % 4 == 0 || i + 1 == 113 {
            o.push('\n');
        } else {
            o.push_str("  ");
        }
    }
    Some(o)
}

/// What `estarCalculation` returns through its pointer arguments, plus the
/// `formula_calc` it computed on the way.
#[derive(Debug, Clone)]
pub struct EstarResult {
    /// `densityCorr[113]`.
    pub density_corr: Vec<f64>,
    /// `enGrid[113]`, MeV.
    pub en_grid: Vec<f64>,
    /// `*meanIval` (a `float` upstream).
    pub mean_ival: f32,
    /// The formula data (`fc`), with `pot` as upstream's I-value in eV.
    pub fc: FormulaCalc,
}

/// `estarCalculation` (`estar.cpp:492`) with `egsEstar`'s argument checks:
/// the ESTAR density-effect correction of a medium on the 113-point energy
/// grid. `elements` is `getElemArray`'s output ([`element_array`]),
/// `ipotval` is `-1` for "compute the I-value". The long-double `q` grid is
/// emulated with [`Ext`]; the `outputDensityFile` text is [`density_file_text`].
///
/// **First port, differentially tested against compiled upstream** (see
/// `docs/egsnrc_phase2_differential.md`).
#[allow(clippy::too_many_arguments)]
pub fn estar_calculation(
    is_compound: bool,
    media_density: f32,
    elements: &[String],
    mass_fraction: &[f64],
    num_of_atoms: &[f32],
    ipotval: f32,
    t: &EstarTables,
) -> Result<EstarResult, EgsError> {
    let nep = elements.len();
    if nep == 0 || nep > 100 || media_density <= 0.0 {
        return Err(EgsError::Fatal("estar::egsEstar: invalid NEP or density"));
    }
    let knmat = if is_compound { 1 } else if nep == 1 { 0 } else { 2 };
    let rho = media_density as f64;
    let mut fc = data_from_formulae(knmat, rho, elements, mass_fraction, num_of_atoms, t)?;
    if ipotval >= 0.0 {
        fc.pot = ipotval as f64;
    }
    let mean_ival = fc.pot as f32;

    let lkmax = 113usize;
    let lmax = 1101usize;
    let qfac = Ext::from_f64(RealMath::r_exp(RealMath::r_ln(10.0_f64) / 50.0));
    let mut q = vec![Ext::from_f64(1e-04); lmax];
    for i in 1..lmax {
        q[i] = q[i - 1].mul(qfac);
    }

    let mmax = fc.jz.len();
    if mmax == 0 || mmax > 50 {
        return Err(EgsError::Fatal("estar::estarCalculation: mmax out of range [1, 50]"));
    }
    let mut g = [0.0_f64; 50];
    for i in 0..mmax {
        let jz = fc.jz[i];
        if jz == 0 || jz > 100 {
            return Err(EgsError::Fatal("estar::estarCalculation: atomic number out of range"));
        }
        let a = ATB[jz - 1];
        g[i] = fc.wt[i] * (jz as f64 / a);
    }
    let zav = fc.zav;
    if zav <= 0.0 {
        return Err(EgsError::Fatal("estar::estarCalculation: zav is non-positive"));
    }
    let hom = 28.81593_f64 * Real::sqrt(rho * zav);
    let phil = 2.0 * RealMath::r_ln(fc.pot / hom);
    for gi in g.iter_mut().take(mmax) {
        *gi /= zav;
    }

    // parseData: per element nmax, 113, nc[nmax], bd[nmax], 113 loss values.
    let ed = &t.element_data;
    let mut nmax_z = [0usize; 100];
    let mut nc = [[0_i32; 27]; 100];
    let mut bd = [[0.0_f64; 27]; 100];
    let mut j = 0usize;
    for i in 0..100 {
        let nm = ed[j] as i32;
        if nm < 0 || j + 2 + 2 * nm as usize + 113 > 14532 || nm as usize > 26 {
            return Err(EgsError::Fatal("estar::parseData: bad data layout"));
        }
        nmax_z[i] = nm as usize;
        let mut k = j + 2;
        for a in 0..nm as usize {
            nc[i][a] = ed[k] as i32;
            k += 1;
        }
        for a in 0..nm as usize {
            bd[i][a] = ed[k];
            k += 1;
        }
        k += 113;
        j = k;
    }

    let mut nbas = 0usize;
    let mut f = vec![0.0_f64; 1000];
    let mut en = vec![0.0_f64; 1000];
    for m in 0..mmax {
        let iz = fc.jz[m];
        let rec = iz - 1;
        let mut nmax = nmax_z[rec];
        if iz == 6 && mmax == 1 {
            nc[rec][nmax - 1] = 1;
            nc[rec][nmax] = -1;
            bd[rec][nmax] = bd[rec][nmax - 1];
            nmax += 1;
        }
        if nc[rec][nmax - 1] < 0 {
            nc[rec][nmax - 1] = -nc[rec][nmax - 1];
            if mmax <= 1 {
                bd[rec][nmax - 1] = 0.0;
            }
        }
        let mut nsum = 0_i32;
        for p in 0..nmax {
            nsum += nc[rec][p];
        }
        let sum = nsum as f64;
        if nbas + nmax > 1000 {
            return Err(EgsError::Fatal("estar::estarCalculation: too many oscillators"));
        }
        if sum == 0.0 {
            return Err(EgsError::Fatal("estar::estarCalculation: electron sum is zero"));
        }
        for n in 0..nmax {
            f[n + nbas] = nc[rec][n] as f64 * g[m] / sum;
            en[n + nbas] = bd[rec][n];
        }
        nbas += nmax;
    }

    let nmax = nbas;
    let mut alf = vec![0.0_f64; 1000];
    let mut eps = vec![0.0_f64; 1000];
    for a in alf.iter_mut().take(nmax) {
        *a = 2.0 / 3.0;
    }
    if en[nmax - 1] <= 0.0 {
        alf[nmax - 1] = 1.0;
    }
    for n in 0..nmax {
        eps[n] = (en[n] / hom) * (en[n] / hom);
    }

    // Newton's method for the adjustment factor squared.
    let mut root = 1.0_f64;
    let mut droot = 1.0_f64;
    let mut iter = 0;
    while droot.abs() - 0.00001 > 0.0 {
        let mut fun = -phil;
        let mut der = 0.0_f64;
        for n in 0..nmax {
            let trm = root * eps[n] + alf[n] * f[n];
            if trm <= 0.0 {
                return Err(EgsError::Fatal("estar::estarCalculation: non-positive trm in Newton's method"));
            }
            fun += f[n] * RealMath::r_ln(trm);
            der += f[n] * eps[n] / trm;
        }
        if der == 0.0 {
            return Err(EgsError::Fatal("estar::estarCalculation: zero derivative in Newton's method"));
        }
        droot = fun / der;
        root -= droot;
        iter += 1;
        if iter >= 1000 {
            return Err(EgsError::Fatal("estar::estarCalculation: Newton's method did not converge"));
        }
    }
    if root < 0.0 {
        return Err(EgsError::Fatal("estar::estarCalculation: Newton's method gave a negative root"));
    }
    for e in eps.iter_mut().take(nmax) {
        *e *= root;
    }

    let mut yq = vec![0.0_f64; lmax];
    let mut yql = vec![0.0_f64; lmax];
    let mut d = vec![0.0_f64; lmax];
    for n in 0..lmax {
        let mut sum = 0.0_f64;
        for m in 0..nmax {
            let denom = Ext::from_f64(eps[m]).add(q[n]).to_f64();
            if denom == 0.0 {
                return Err(EgsError::Fatal("estar::estarCalculation: eps + q is zero"));
            }
            sum += f[m] / denom;
        }
        if sum == 0.0 {
            return Err(EgsError::Fatal("estar::estarCalculation: zero sum for yq"));
        }
        yq[n] = 1.0 / sum;
        if yq[n] <= 0.0 {
            return Err(EgsError::Fatal("estar::estarCalculation: yq is non-positive"));
        }
        yql[n] = RealMath::r_ln(yq[n]);
        let mut sum = 0.0_f64;
        for m in 0..nmax {
            let denom = eps[m] + alf[m] * f[m];
            if denom == 0.0 {
                return Err(EgsError::Fatal("estar::estarCalculation: zero denominator in arg"));
            }
            let arg = Ext::from_f64(1.0).add(q[n].div(Ext::from_f64(denom))).to_f64();
            sum += f[m] * RealMath::r_ln(arg);
        }
        d[n] = Ext::from_f64(sum).sub(q[n].div(Ext::from_f64(yq[n] + 1.0))).to_f64();
    }

    let rmass = 0.510999906_f64;
    let [sa, sb, sc, sd] = fscof(lmax, &yql, &d)?;
    let tol = 0.000000001_f64;
    let mut dlt = vec![0.0_f64; lkmax];
    for i in 0..lkmax {
        let tau = t.energy_grid[i] / rmass;
        let y = tau * (tau + 2.0);
        let mut delta = 0.0_f64;
        if y >= yq[0] {
            if y - yq[lmax - 1] <= 0.0 {
                let yl = RealMath::r_ln(y);
                let (lb, ub) = fbspol(yl, &yql, &sa, &sb, &sc, &sd)?;
                let xroot = bisec(q[lb].to_f64(), q[ub].to_f64(), tol, tau, &f, &eps, nmax)?;
                let mut yqn = 0.0_f64;
                let mut sum = 0.0_f64;
                for m in 0..nmax {
                    let denom = eps[m] + alf[m] * f[m];
                    if denom == 0.0 {
                        return Err(EgsError::Fatal("estar::estarCalculation: zero denominator in bisection arg"));
                    }
                    let arg = 1.0 + xroot / denom;
                    sum += f[m] * RealMath::r_ln(arg);
                    yqn += f[m] / (eps[m] + xroot);
                }
                if yqn == 0.0 {
                    return Err(EgsError::Fatal("estar::estarCalculation: yqn is zero"));
                }
                yqn = 1.0 / yqn;
                delta = sum - xroot / (yqn + 1.0);
            } else {
                return Err(EgsError::Fatal("estar::estarCalculation: energy out of range"));
            }
        }
        dlt[i] = delta;
    }
    Ok(EstarResult { density_corr: dlt, en_grid: t.energy_grid.clone(), mean_ival, fc })
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

    /// `Ext` against `f64` where the result is exactly representable, and
    /// the double-rounding behaviour `long double` has.
    #[test]
    fn ext_matches_f64_when_exact() {
        let a = Ext::from_f64(0.75);
        let b = Ext::from_f64(0.125);
        assert_eq!(a.add(b).to_f64(), 0.875);
        assert_eq!(a.sub(b).to_f64(), 0.625);
        assert_eq!(b.sub(a).to_f64(), -0.625);
        assert_eq!(a.mul(b).to_f64(), 0.09375);
        assert_eq!(a.div(b).to_f64(), 6.0);
        assert_eq!(Ext::from_f64(1e-4).to_f64(), 1e-4);
        assert_eq!(a.sub(a).to_f64(), 0.0);
        // 1 + 2^-60 is kept by the 64-bit mantissa but rounds away in f64
        let tiny = Ext::from_f64(1.0).add(Ext::from_f64(2f64.powi(-60)));
        assert_ne!(tiny.mant, 1u64 << 63);
        assert_eq!(tiny.to_f64(), 1.0);
    }

    /// `EGSNRC_REFERENCE_ESTAR_CALC` (lines `case iscomp nep rho ipot
    /// e:w:atoms ... | rc ival | density corrections`, written by
    /// `verification_and_validation/egsnrc_estar/harness_calc.cpp` linked to
    /// the upstream `estar*.cpp`) and `EGSNRC_ESTAR_TABLES` (the upstream
    /// `estar_dataTables.cpp`, read at run time, never committed). Prints
    /// the worst relative difference; results in
    /// `docs/egsnrc_phase2_differential.md`.
    #[test]
    fn differential_estar_calculation_against_compiled_egsnrc() {
        let (Ok(path), Ok(tabs)) = (std::env::var("EGSNRC_REFERENCE_ESTAR_CALC"), std::env::var("EGSNRC_ESTAR_TABLES")) else {
            eprintln!("EGSNRC_REFERENCE_ESTAR_CALC/EGSNRC_ESTAR_TABLES not set: differential test skipped");
            return;
        };
        let t = EstarTables::from_upstream_source(&std::fs::read_to_string(tabs).unwrap()).unwrap();
        let (mut n, mut exact, mut worst, mut worst_line, mut files_equal) = (0usize, 0usize, 0.0_f64, String::new(), 0usize);
        for l in std::fs::read_to_string(path).unwrap().lines() {
            let parts: Vec<&str> = l.split(" | ").collect();
            let head: Vec<&str> = parts[0].split_whitespace().collect();
            let iscomp = head[1] == "1";
            let rho: f32 = head[3].parse().unwrap();
            let ipot: f32 = head[4].parse().unwrap();
            let (mut el, mut w, mut a) = (Vec::new(), Vec::new(), Vec::new());
            for tok in &head[5..] {
                let f: Vec<&str> = tok.split(':').collect();
                el.push(String::from(f[0]));
                w.push(f[1].parse::<f64>().unwrap());
                a.push(f[2].parse::<f32>().unwrap());
            }
            let ival: f32 = parts[1].split_whitespace().nth(1).unwrap().parse().unwrap();
            let want: Vec<f64> = parts[2].split_whitespace().map(|x| x.parse().unwrap()).collect();
            let r = estar_calculation(iscomp, rho, &el, &w, &a, ipot, &t).unwrap();
            assert_eq!(r.mean_ival, ival, "{}", parts[0]);
            // `outputDensityFile`: the text must equal what the compiled upstream wrote
            if let Ok(dir) = std::env::var("EGSNRC_REFERENCE_ESTAR_DENSITY_DIR") {
                let fname = format!("{dir}/case{n}.density");
                let want = std::fs::read_to_string(&fname).unwrap();
                let got = density_file_text(rho, &r, &fname).unwrap();
                assert_eq!(got, want, "density file text of case {n}");
                files_equal += 1;
            }
            assert_eq!(want.len(), 113);
            for (k, wv) in want.iter().enumerate() {
                let g = r.density_corr[k];
                if g.to_bits() == wv.to_bits() {
                    exact += 1;
                }
                let rel = if *wv != 0.0 { (g - wv).abs() / wv.abs() } else { g.abs() };
                if rel > worst {
                    worst = rel;
                    worst_line = format!("{} grid {k}: {g:e} vs {wv:e}", parts[0]);
                }
            }
            n += 1;
        }
        eprintln!("{n} media x 113 energies: {exact} bit-identical of {}, worst relative difference {worst:e} ({worst_line}); density files identical to upstream: {files_equal}", n * 113);
        assert!(n > 0);
    }

    /// The C++ `iostream` number formats of `outputDensityFile` and its name rules.
    #[test]
    fn density_file_formats_match_iostream() {
        assert_eq!(fmt_g(82.0, 8), "82");
        assert_eq!(fmt_g(0.001205, 8), "0.001205");
        assert_eq!(fmt_g(19.200000762939453, 8), "19.200001");
        assert_eq!(fmt_g(1.0e-5, 8), "1e-05");
        assert_eq!(fmt_g(123456789.0, 8), "1.2345679e+08");
        assert_eq!(fmt_g(0.0, 8), "0");
        assert_eq!(fmt_e(1.0e-3, 2), "1.00e-03");
        assert_eq!(fmt_e(0.0, 3), "0.000e+00");
        assert_eq!(fmt_e(-1.25e12, 3), "-1.250e+12");
        assert_eq!(file_name_without_extension("/a/b/water.density"), "water");
        assert_eq!(file_name_without_extension("water.density"), "", "no '/': upstream returns an empty name");
        assert_eq!(file_name_without_extension("/a/b/noext"), "noext");
        let r = EstarResult { density_corr: vec![0.0; 113], en_grid: vec![1.0e-3; 113], mean_ival: 75.0, fc: FormulaCalc { jz: vec![1, 8], wt: vec![0.1111, 0.8889], zav: 0.55, pot: 75.0 } };
        assert!(density_file_text(1.0, &r, "").is_none());
        let t = density_file_text(1.0, &r, "/x/w.density trailing words").unwrap();
        assert!(t.starts_with("w\n113 75 1 2\n1 0.1111 8 0.8889\n1.00e-03,0.000e+00  "));
        assert_eq!(t.lines().count(), 2 + 1 + 29, "title, header, one element line, 29 grid lines");
    }
}
