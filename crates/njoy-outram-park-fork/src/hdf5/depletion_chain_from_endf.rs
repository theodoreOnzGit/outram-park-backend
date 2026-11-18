// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream files, read line for line:
//     openmc/deplete/chain.py  (Chain.from_endf, replace_missing,
//                               replace_missing_fpy)
//     openmc/data/decay.py     (_DECAY_MODES, get_decay_modes, DecayMode,
//                               Decay.__init__ up to the decay modes,
//                               Decay.decay_energy, FissionProductYields)
//     openmc/data/data.py      (DADZ, gnds_name, zam)
//     openmc/data/reaction.py  (FISSION_MTS)
//
// An OUTRAM PARK translation for research, education and V&V only.

//! **`Chain.from_endf`** (gh:#1621): a depletion chain built from the
//! decay, neutron fission-yield and incident-neutron sublibraries.
//!
//! For each decay evaluation (sorted by `zam`): its half-life, decay
//! energy (light + electromagnetic + heavy mean energies) and decay modes,
//! each daughter found from the mode sequence; for each transmutation
//! reaction whose MTs the neutron evaluation carries, the product and the
//! Q value of the lowest such MT; fission with MT=18's Q and the
//! independent yields of MF=8/MT=454, or another actinide's yields where
//! the parent has none. Missing daughters and products are replaced as
//! upstream does (`replace_missing`), and every replacement is reported.
//!
//! # Departures from upstream
//!
//! 1. **No decay sources.** Upstream attaches each nuclide's photon and
//!    electron spectra (`Decay.sources`); here `sources` is empty. They do
//!    not enter the burnup matrix.
//! 2. **Errors where upstream crashes**: an `unknown` decay mode with no
//!    daughter (upstream passes `None` to `replace_missing`), an element
//!    with no decay data in `replace_missing` (a `NameError` upstream), a
//!    fissionable nuclide without MT=18 (`KeyError`), a yield file without
//!    MT=454. The first is reported and given no target instead.
//! 3. **The report is returned**, where upstream prints it; upstream's
//!    `missing_daughter` list is never filled (it prints instead), and the
//!    report includes those cases.
//! 4. **Only the default transmutation reactions** (`(n,2n)`, `(n,3n)`,
//!    `(n,4n)`, `(n,gamma)`, `(n,p)`, `(n,a)`) carry their `DADZ` and MTs
//!    here; asking for another is an error.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::depletion_chain_ops::python_float_repr;
use super::depletion_chain_xml::{ChainDecay, ChainNuclide, ChainReaction, DepletionChainXml, FissionYieldSet};
use crate::acquire::symbol_for_z;
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::error::NjoyError;

fn bad(msg: String) -> NjoyError {
    NjoyError::InvalidInput(msg)
}

/// Upstream `_DECAY_MODES`, in its order: RTYP digit → (name, (ΔA, ΔZ)).
const DECAY_MODES: [(&str, Option<(i32, i32)>); 11] = [
    ("gamma", Some((0, 0))),
    ("beta-", Some((0, 1))),
    ("ec/beta+", Some((0, -1))),
    ("IT", Some((0, 0))),
    ("alpha", Some((-4, -2))),
    ("n", Some((-1, 0))),
    ("sf", None),
    ("p", Some((-1, -1))),
    ("e-", Some((0, 0))),
    ("xray", Some((0, 0))),
    ("unknown", None),
];

/// The default transmutation reactions with upstream's MTs and `DADZ`
/// `(ΔA, ΔZ)`.
const DEFAULT_REACTIONS: [(&str, (i32, i32)); 6] =
    [("(n,2n)", (-1, 0)), ("(n,3n)", (-2, 0)), ("(n,4n)", (-3, 0)), ("(n,gamma)", (1, 0)), ("(n,p)", (0, -1)), ("(n,a)", (-3, -2))];

/// `REACTIONS[name].mts` for the default reactions.
fn mts_of(name: &str) -> Vec<i32> {
    match name {
        "(n,2n)" => core::iter::once(16).chain(875..892).collect(),
        "(n,3n)" => vec![17],
        "(n,4n)" => vec![37],
        "(n,gamma)" => vec![102],
        "(n,p)" => core::iter::once(103).chain(600..650).collect(),
        "(n,a)" => core::iter::once(107).chain(800..850).collect(),
        _ => Vec::new(),
    }
}

/// Upstream `FISSION_MTS`.
const FISSION_MTS: [i32; 5] = [18, 19, 20, 21, 38];

/// Upstream `gnds_name(Z, A, m)`.
pub fn gnds_name(z: i32, a: i32, m: i32) -> Option<String> {
    // Upstream's ATOMIC_SYMBOL[0] is 'n' (the free neutron, e.g. dec-000_n_001).
    let sym = if z == 0 { "n" } else { symbol_for_z(u32::try_from(z).ok()?)? };
    Some(if m > 0 { format!("{sym}{a}_m{m}") } else { format!("{sym}{a}") })
}

/// Upstream `zam`.
fn zam(name: &str) -> Result<(i32, i32, i32), NjoyError> {
    if let Some(rest) = name.strip_prefix('n') {
        if let Ok(a) = rest.parse::<i32>() {
            return Ok((0, a, 0));
        }
    }
    let (z, a, m) = super::depletion_chain_ops::zam(name)?;
    Ok((z as i32, a as i32, m as i32))
}

/// Upstream `get_decay_modes`: the RTYP float as Python prints it, zeros
/// stripped at both ends, the point removed, one mode per digit.
/// (`RTYP = 0` therefore gives no modes at all, as upstream.)
pub fn get_decay_modes(rtyp: f64) -> Result<Vec<&'static str>, NjoyError> {
    if rtyp.trunc() == 10.0 {
        return Ok(vec!["unknown"]);
    }
    let s = python_float_repr(rtyp);
    let s = s.trim_matches('0').replace('.', "");
    s.chars()
        .map(|c| {
            c.to_digit(10)
                .and_then(|d| DECAY_MODES.get(d as usize))
                .map(|m| m.0)
                .ok_or_else(|| bad(format!("RTYP {rtyp}: no decay mode for '{c}'")))
        })
        .collect()
}

/// Upstream `DecayMode.daughter`.
fn daughter(parent: &str, modes: &[&str], state: i32) -> Result<Option<String>, NjoyError> {
    let (mut z, mut a, _) = zam(parent)?;
    for mode in modes {
        for (name, changes) in DECAY_MODES {
            if name == *mode {
                match changes {
                    Some((da, dz)) => {
                        a += da;
                        z += dz;
                        break;
                    }
                    None => return Ok(None),
                }
            }
        }
    }
    Ok(gnds_name(z, a, state))
}

/// One decay mode of an evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct DecayModeRecord {
    /// Successive modes, e.g. `["beta-", "n"]`.
    pub modes: Vec<&'static str>,
    /// Daughter (`None` for `sf` / `unknown`).
    pub daughter: Option<String>,
    /// Branching ratio (nominal).
    pub branching_ratio: f64,
}

/// What `from_endf` uses of an `openmc.data.Decay`.
#[derive(Debug, Clone, PartialEq)]
pub struct DecayRecord {
    /// GNDS name from MF=8/MT=457's head.
    pub name: String,
    /// Atomic number.
    pub z: i32,
    /// Mass number.
    pub a: i32,
    /// `NST == 1`.
    pub stable: bool,
    /// Half-life \[s\] (`inf` when stable, as upstream).
    pub half_life: f64,
    /// Light + electromagnetic + heavy mean decay energy \[eV\].
    pub decay_energy: f64,
    /// Decay modes in file order.
    pub modes: Vec<DecayModeRecord>,
}

fn single_mat(tape: &Tape) -> Result<i32, NjoyError> {
    let mats = tape.materials();
    match mats.as_slice() {
        [m] => Ok(*m),
        _ => Err(bad(format!("expected one material, found {mats:?}"))),
    }
}

/// Upstream `Decay(ev)`, up to the decay modes (departure 1).
///
/// # Errors
/// A tape without MF=8/MT=457 or with malformed records.
pub fn read_decay(tape: &Tape) -> Result<DecayRecord, NjoyError> {
    let mat = single_mat(tape)?;
    let sec = tape.section(mat, 8, 457).ok_or(NjoyError::SectionNotFound { mat, mf: 8, mt: 457 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let za = head.c1.round() as i32;
    let (z, a) = (za / 1000, za % 1000);
    let name = gnds_name(z, a, head.l2).unwrap_or_default();
    let stable = head.n1 == 1;
    if stable {
        return Ok(DecayRecord { name, z, a, stable, half_life: f64::INFINITY, decay_energy: 0.0, modes: Vec::new() });
    }
    let l1 = cur.read_list()?;
    let half_life = l1.head.c1;
    let v = &l1.data;
    let decay_energy = if v.len() >= 6 { (v[0] + v[2]) + v[4] } else { 0.0 };
    let l2 = cur.read_list()?;
    let n_modes = l2.head.n2.max(0) as usize;
    let mut modes = Vec::with_capacity(n_modes);
    for i in 0..n_modes {
        let w = &l2.data[6 * i..6 * i + 6];
        let m = get_decay_modes(w[0])?;
        let d = daughter(&name, &m, w[1] as i32)?;
        modes.push(DecayModeRecord { modes: m, daughter: d, branching_ratio: w[4] });
    }
    Ok(DecayRecord { name, z, a, stable, half_life, decay_energy, modes })
}

/// `Evaluation.gnds_name`: ZA from MF=1/MT=451's head, LISO from its
/// second record.
fn evaluation_name(tape: &Tape) -> Result<String, NjoyError> {
    let mat = single_mat(tape)?;
    let sec = tape.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let second = cur.read_cont()?;
    let za = head.c1.round() as i32;
    gnds_name(za / 1000, za % 1000, second.l2).ok_or_else(|| bad(format!("ZA {za}: unknown element")))
}

/// Upstream `FissionProductYields(ev)`: the independent yields (MT=454) as
/// `(energy, [(product, yield)])`, products in file order.
///
/// # Errors
/// No MT=454 (departure 2) or malformed records.
pub fn read_fission_yields(tape: &Tape) -> Result<(String, Vec<(f64, Vec<(String, f64)>)>), NjoyError> {
    let name = evaluation_name(tape)?;
    let mat = single_mat(tape)?;
    let sec = tape.section(mat, 8, 454).ok_or(NjoyError::SectionNotFound { mat, mf: 8, mt: 454 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let n_energy = cur.read_cont()?.l1.max(0) as usize;
    let mut out = Vec::with_capacity(n_energy);
    for _ in 0..n_energy {
        let l = cur.read_list()?;
        let n = l.head.n2.max(0) as usize;
        let mut ys = Vec::with_capacity(n);
        for j in 0..n {
            let zaf = l.data[4 * j].round() as i32;
            let p = gnds_name(zaf / 1000, zaf % 1000, l.data[4 * j + 1] as i32).unwrap_or_else(|| format!("ZA{zaf}"));
            ys.push((p, l.data[4 * j + 2]));
        }
        out.push((l.head.c1, ys));
    }
    Ok((name, out))
}

/// The neutron evaluation's MF=3 sections: `(MT, QI)` in tape order.
///
/// # Errors
/// Malformed records.
pub fn read_reaction_q(tape: &Tape) -> Result<(String, Vec<(i32, f64)>), NjoyError> {
    let name = evaluation_name(tape)?;
    let mut out = Vec::new();
    for s in tape.sections().iter().filter(|s| s.key.mf == 3) {
        let mut cur = SectionCursor::new(&s.rows);
        cur.read_cont()?;
        let c = cur.read_cont()?;
        out.push((s.key.mt, c.c2));
    }
    Ok((name, out))
}

/// Upstream `replace_missing`. `decay` is in upstream's dict order (the
/// order the files were given).
fn replace_missing(product: &str, decay: &[DecayRecord]) -> Result<Option<String>, NjoyError> {
    let (mut z, mut a, state) = zam(product)?;
    if z == 0 {
        return Ok(None);
    }
    let symbol = symbol_for_z(z as u32).ok_or_else(|| bad(format!("Z {z}")))?;
    let mut product = if state != 0 { format!("{symbol}{a}") } else { product.to_owned() };
    // re.match(symbol(\d+)(?:_m\d+)?, nuclide): the symbol, then digits.
    let mass_of = |n: &str| -> Option<i32> {
        let rest = n.strip_prefix(symbol)?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() { None } else { digits.parse().ok() }
    };
    let mut half_life = 0.0;
    let mut mass_longest_lived: Option<i32> = None;
    for d in decay {
        if let Some(m) = mass_of(&d.name) {
            if d.stable {
                mass_longest_lived = Some(m);
                break;
            }
            if d.half_life > half_life {
                mass_longest_lived = Some(m);
                half_life = d.half_life;
            }
        }
    }
    let longest = mass_longest_lived.ok_or_else(|| bad(format!("replace_missing({product}): no decay data for element {symbol}")))?;
    let beta_minus = longest < a;
    let has = |n: &str| decay.iter().any(|d| d.name == n);
    while !has(&product) {
        if z > 98 {
            z -= 2;
            a -= 4;
        } else if beta_minus {
            z += 1;
        } else {
            z -= 1;
        }
        product = gnds_name(z, a, 0).ok_or_else(|| bad(format!("replace_missing ran off the periodic table from {product}")))?;
    }
    Ok(Some(product))
}

/// Upstream `replace_missing_fpy`.
fn replace_missing_fpy(actinide: &str, fpy: &[(String, Vec<(f64, Vec<(String, f64)>)>)], decay: &[DecayRecord]) -> Result<String, NjoyError> {
    let has_fpy = |n: &str| fpy.iter().any(|(m, _)| m == n);
    let has_decay = |n: &str| decay.iter().any(|d| d.name == n);
    let (z0, a0, m) = zam(actinide)?;
    if m == 0 {
        if let Some(meta) = gnds_name(z0, a0, 1) {
            if has_fpy(&meta) {
                return Ok(meta);
            }
        }
    }
    for step in [1, -1] {
        let (mut z, mut a) = (z0, a0);
        let mut isotone = actinide.to_owned();
        while has_decay(&isotone) {
            z += step;
            a += step;
            match gnds_name(z, a, 0) {
                Some(n) => isotone = n,
                None => break,
            }
            if has_fpy(&isotone) {
                return Ok(isotone);
            }
        }
    }
    Ok("U235".to_owned())
}

/// Everything `from_endf` replaced or could not find (upstream prints it).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FromEndfReport {
    /// `(parent, mode, missing daughter, replacement)`.
    pub missing_daughter: Vec<(String, String, Option<String>, Option<String>)>,
    /// `(parent, reaction)` whose product had no decay data and no
    /// replacement.
    pub missing_rx_product: Vec<(String, String)>,
    /// `(parent, nuclide whose yields it borrows)`.
    pub missing_fpy: Vec<(String, String)>,
    /// `(parent, energy, total yield replaced)`.
    pub missing_fp: Vec<(String, f64, f64)>,
    /// Branch sets renormalised (upstream's `isclose` fix): `(parent, sum)`.
    pub renormalised: Vec<(String, f64)>,
}

/// `math.isclose(a, b)` with its default `rel_tol = 1e-9`.
fn isclose(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

/// Upstream `Chain.from_endf(decay_files, fpy_files, neutron_files,
/// reactions)`, on parsed tapes. `reactions = None` is upstream's default.
///
/// # Errors
/// See the module docs, departure 2; a reaction outside the defaults; or a
/// tape that cannot be read.
pub fn from_endf(
    decay_tapes: &[Tape],
    fpy_tapes: &[Tape],
    neutron_tapes: &[Tape],
    reactions: Option<&[&str]>,
) -> Result<(DepletionChainXml, FromEndfReport), NjoyError> {
    let transmutation: Vec<(&str, (i32, i32))> = match reactions {
        None => DEFAULT_REACTIONS.to_vec(),
        Some(list) => list
            .iter()
            .map(|r| DEFAULT_REACTIONS.iter().find(|(n, _)| n == r).copied().ok_or_else(|| bad(format!("reaction {r}: only upstream's default reactions are ported"))))
            .collect::<Result<_, _>>()?,
    };
    let mut rx: Vec<(String, Vec<(i32, f64)>)> = Vec::new();
    for t in neutron_tapes {
        rx.push(read_reaction_q(t)?);
    }
    let mut decay: Vec<DecayRecord> = Vec::new();
    for t in decay_tapes {
        let d = read_decay(t)?;
        if d.z == 0 {
            continue;
        }
        // A later file with the same name replaces the earlier (dict).
        match decay.iter_mut().find(|x| x.name == d.name) {
            Some(x) => *x = d,
            None => decay.push(d),
        }
    }
    let mut fpy: Vec<(String, Vec<(f64, Vec<(String, f64)>)>)> = Vec::new();
    for t in fpy_tapes {
        let f = read_fission_yields(t)?;
        match fpy.iter_mut().find(|x| x.0 == f.0) {
            Some(x) => *x = f,
            None => fpy.push(f),
        }
    }
    let has_decay = |n: &str| decay.iter().any(|d| d.name == n);

    let mut order: Vec<((i32, i32, i32), usize)> = Vec::with_capacity(decay.len());
    for (i, d) in decay.iter().enumerate() {
        order.push((zam(&d.name)?, i));
    }
    order.sort();
    let mut report = FromEndfReport::default();
    let mut nuclides = Vec::with_capacity(order.len());
    for (_, i) in order {
        let data = &decay[i];
        let parent = data.name.clone();
        let mut nuc = ChainNuclide {
            name: parent.clone(),
            half_life_seconds: None,
            decay_energy_ev: None,
            decays: Vec::new(),
            reactions: Vec::new(),
            fission_yields: Vec::new(),
            fission_yield_parent: None,
            sources: Vec::new(),
        };
        if !data.stable && data.half_life != 0.0 {
            nuc.half_life_seconds = Some(data.half_life);
            nuc.decay_energy_ev = Some(data.decay_energy);
            let mut brs = Vec::new();
            let mut ids = Vec::new();
            for mode in &data.modes {
                let type_ = mode.modes.join(",");
                let target = match &mode.daughter {
                    Some(d) if has_decay(d) => Some(d.clone()),
                    _ if type_.contains("sf") => None,
                    Some(d) => {
                        let r = replace_missing(d, &decay)?;
                        report.missing_daughter.push((parent.clone(), type_.clone(), Some(d.clone()), r.clone()));
                        r
                    }
                    None => {
                        report.missing_daughter.push((parent.clone(), type_.clone(), None, None));
                        None
                    }
                };
                brs.push(mode.branching_ratio);
                ids.push((type_, target));
            }
            let sum: f64 = brs.iter().sum();
            if !brs.is_empty() && !isclose(sum, 1.0) {
                let mut max_i = 0;
                for (k, b) in brs.iter().enumerate() {
                    if *b > brs[max_i] {
                        max_i = k;
                    }
                }
                brs[max_i] = brs[max_i] - sum + 1.0;
                report.renormalised.push((parent.clone(), sum));
            }
            for (br, (type_, target)) in brs.into_iter().zip(ids) {
                nuc.decays.push(ChainDecay { mode: type_, target, branching_ratio: br });
            }
        }
        let mut fissionable = false;
        if let Some((_, qs)) = rx.iter().find(|(n, _)| *n == parent) {
            for (name, (da, dz)) in &transmutation {
                let mts = mts_of(name);
                if !qs.iter().any(|(mt, _)| mts.contains(mt)) {
                    continue;
                }
                let mut target = gnds_name(data.z + dz, data.a + da, 0);
                if target.as_deref().is_none_or(|t| !has_decay(t)) {
                    target = match &target {
                        Some(t) => replace_missing(t, &decay)?,
                        None => None,
                    };
                    if target.is_none() {
                        report.missing_rx_product.push((parent.clone(), (*name).to_owned()));
                    }
                }
                let mut sorted = mts.clone();
                sorted.sort_unstable();
                let q = sorted.iter().find_map(|mt| qs.iter().find(|(m, _)| m == mt).map(|(_, q)| *q)).unwrap_or(0.0);
                nuc.reactions.push(ChainReaction { kind: (*name).to_owned(), target, q_ev: q, branching_ratio: 1.0 });
            }
            if qs.iter().any(|(mt, _)| FISSION_MTS.contains(mt)) {
                let q = qs.iter().find(|(mt, _)| *mt == 18).map(|(_, q)| *q).ok_or_else(|| bad(format!("{parent}: fission MTs without MT=18 (upstream KeyError)")))?;
                nuc.reactions.push(ChainReaction { kind: "fission".into(), target: None, q_ev: q, branching_ratio: 1.0 });
                fissionable = true;
            }
        }
        if fissionable {
            if let Some((_, sets)) = fpy.iter().find(|(n, _)| *n == parent) {
                let mut energies: Vec<(f64, Vec<(String, f64)>)> = Vec::new();
                for (e, table) in sets {
                    let mut replaced = 0.0;
                    let mut ys: Vec<(String, f64)> = Vec::new();
                    for (product, y) in table {
                        let p = if has_decay(product) {
                            product.clone()
                        } else {
                            replaced += y;
                            replace_missing(product, &decay)?.unwrap_or_else(|| "None".into())
                        };
                        match ys.iter_mut().find(|(q, _)| *q == p) {
                            Some((_, v)) => *v += y,
                            None => ys.push((p, *y)),
                        }
                    }
                    if replaced > 0.0 {
                        report.missing_fp.push((parent.clone(), *e, replaced));
                    }
                    energies.push((*e, ys));
                }
                nuc.fission_yields = distribution(energies);
            } else {
                let borrow = replace_missing_fpy(&parent, &fpy, &decay)?;
                report.missing_fpy.push((parent.clone(), borrow.clone()));
                nuc.fission_yield_parent = Some(borrow);
            }
        }
        nuclides.push(nuc);
    }
    // Upstream: nuclide.yield_data = chain[nuclide._fpy].yield_data.
    for i in 0..nuclides.len() {
        if let Some(p) = nuclides[i].fission_yield_parent.clone() {
            let ys = nuclides.iter().find(|n| n.name == p).map(|n| n.fission_yields.clone()).unwrap_or_default();
            // A lender with no yields of its own (not fissionable in this
            // set) lends nothing; upstream then exports no yields and no
            // `parent` either (`if self.yield_data:`), so the link is
            // dropped here too. The report keeps it.
            if ys.is_empty() {
                nuclides[i].fission_yield_parent = None;
            }
            nuclides[i].fission_yields = ys;
        }
    }
    Ok((DepletionChainXml::from_nuclides(nuclides)?, report))
}

/// Upstream `FissionYieldDistribution(dict)`: energies sorted, products the
/// sorted union, a product absent at an energy given 0.
fn distribution(mut sets: Vec<(f64, Vec<(String, f64)>)>) -> Vec<FissionYieldSet> {
    sets.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut products: Vec<String> = sets.iter().flat_map(|(_, y)| y.iter().map(|(p, _)| p.clone())).collect();
    products.sort();
    products.dedup();
    sets.into_iter()
        .map(|(e, ys)| FissionYieldSet {
            energy_ev: e,
            yields: products.iter().map(|p| (p.clone(), ys.iter().find(|(q, _)| q == p).map_or(0.0, |(_, v)| *v))).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
