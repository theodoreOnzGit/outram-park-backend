// SPDX-License-Identifier: AGPL-3.0-only
//
// Translated from OpenMC at commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf:
// the pointwise half of `vectfit_nuclide` (`openmc/data/multipole.py:416-476`),
// the deck `openmc.data.njoy.make_ace` writes for `IncidentNeutron.from_njoy(
// temperatures=[0.0], broadr=False, heatr=False, purr=False)`
// (`openmc/data/njoy.py:239-295, 411-594`), and the cross-section extraction
// of `IncidentNeutron.from_ace` / `Reaction.from_ace` /
// `_get_redundant_reaction` / `get_reaction_components`
// (`openmc/data/neutron.py:346-368, 529-660, 877-906`,
// `openmc/data/reaction.py:1057-1165`), with `Tabulated1D.__call__`
// (`openmc/data/function.py:164-221`) and the `endf` package's `SUM_RULES`.
// OpenMC is MIT-licensed, Copyright (c) 2011-2025 Massachusetts Institute of
// Technology, UChicago Argonne LLC, and OpenMC contributors; the full
// permission notice is in this module's `vectfit.rs` header and in NOTICE.
//
// NJOY itself is not called: this crate's own RECONR -> GASPR -> ACER runs
// the same deck through `deck::run_deck`.

//! ENDF tape to the 0 K pointwise cross sections `vectfit_nuclide` fits.

use std::path::Path;

use crate::acer::read::{parse_type1, RawAceTable};
use crate::deck::run::{run_deck, DeckOptions};
use crate::endf::tape::Tape;
use crate::errorr::resprx::mf2::Mf2Resonances;
use crate::NjoyError;

use super::npy::searchsorted_right;
use super::PointwiseXs;

const EV_PER_MEV: f64 = 1.0e6;

/// The `endf` package's `SUM_RULES` entries the extraction can reach.
fn sum_rule(mt: i32) -> Option<Vec<i32>> {
    let v: Vec<i32> = match mt {
        1 => vec![2, 3],
        3 => vec![
            4, 5, 11, 16, 17, 22, 23, 24, 25, 27, 28, 29, 30, 32, 33, 34, 35, 36, 37, 41, 42, 44, 45, 152, 153, 154,
            156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176,
            177, 178, 179, 180, 181, 183, 184, 185, 186, 187, 188, 189, 190, 194, 195, 196, 198, 199, 200,
        ],
        4 => (50..92).collect(),
        16 => (875..892).collect(),
        18 => vec![19, 20, 21, 38],
        27 => vec![18, 101],
        101 => vec![
            102, 103, 104, 105, 106, 107, 108, 109, 111, 112, 113, 114, 115, 116, 117, 155, 182, 191, 192, 193, 197,
        ],
        103 => (600..650).collect(),
        104 => (650..700).collect(),
        105 => (700..750).collect(),
        106 => (750..800).collect(),
        107 => (800..850).collect(),
        501 => vec![502, 504, 516, 522],
        516 => vec![515, 517],
        522 => (534..573).collect(),
        _ => return None,
    };
    Some(v)
}

/// One reaction's tabulated cross section on the ACE grid.
#[derive(Debug, Clone)]
struct Rx {
    mt: i32,
    /// `_threshold_idx`, when the Tabulated1D carries one.
    threshold: Option<usize>,
    x: Vec<f64>,
    y: Vec<f64>,
}

/// `Tabulated1D.__call__` for an array argument, one lin-lin region.
fn tab_eval(xd: &[f64], yd: &[f64], x: &[f64]) -> Vec<f64> {
    let n = xd.len();
    x.iter()
        .map(|&xv| {
            let idx = searchsorted_right(xd, xv) as isize - 1;
            let mut y = 0.0;
            if idx >= 0 && (idx as usize) < n - 1 {
                let i = idx as usize;
                y = yd[i] + (xv - xd[i]) / (xd[i + 1] - xd[i]) * (yd[i + 1] - yd[i]);
            }
            if (xv - xd[0]).abs() <= 1e-14 {
                y = yd[0];
            }
            if (xv - xd[n - 1]).abs() <= 1e-14 {
                y = yd[n - 1];
            }
            y
        })
        .collect()
}

/// The `IncidentNeutron` reactions OpenMC builds from a 0 K ACE table.
struct AceReactions {
    energy: Vec<f64>,
    rx: Vec<Rx>,
}

impl AceReactions {
    fn from_ace(ace: &RawAceTable) -> Result<Self, NjoyError> {
        let xss = &ace.xss;
        let get = |one_based: i64| -> Result<f64, NjoyError> {
            xss.get((one_based - 1) as usize)
                .copied()
                .ok_or_else(|| NjoyError::WmpData(format!("ACE: XSS({one_based}) out of range")))
        };
        let n = ace.nxs[2] as usize;
        let esz = ace.jxs[0] as i64;
        let mut energy = Vec::with_capacity(n);
        let mut total = Vec::with_capacity(n);
        let mut absorption = Vec::with_capacity(n);
        let mut elastic = Vec::with_capacity(n);
        for i in 0..n as i64 {
            energy.push(get(esz + i)? * EV_PER_MEV);
            total.push(get(esz + n as i64 + i)?);
            absorption.push(get(esz + 2 * n as i64 + i)?);
            let e = get(esz + 3 * n as i64 + i)?;
            elastic.push(if e < 0.0 { 0.0 } else { e });
        }
        let mut rx = vec![Rx { mt: 1, threshold: None, x: energy.clone(), y: total }];
        if absorption.iter().any(|&a| a != 0.0) {
            rx.push(Rx { mt: 101, threshold: None, x: energy.clone(), y: absorption });
        }
        rx.push(Rx { mt: 2, threshold: None, x: energy.clone(), y: elastic });
        let (jmtr, jsig, jlsig) = (ace.jxs[2] as i64, ace.jxs[6] as i64, ace.jxs[5] as i64);
        for i in 1..=ace.nxs[3] as i64 {
            let mt = get(jmtr + i - 1)? as i32;
            let loc = get(jlsig + i - 1)? as i64;
            let thr = get(jsig + loc - 1)? as i64 - 1;
            let ne = get(jsig + loc)? as i64;
            let mut y = Vec::with_capacity(ne as usize);
            for k in 0..ne {
                let v = get(jsig + loc + 1 + k)?;
                y.push(if mt == 444 { v * EV_PER_MEV } else { v });
            }
            for v in &mut y {
                if *v < 0.0 {
                    *v = 0.0;
                }
            }
            let t = thr as usize;
            let x = energy.get(t..t + ne as usize).ok_or_else(|| NjoyError::WmpData("ACE: SIG past grid".into()))?;
            // a later reaction with the same MT replaces the earlier (dict)
            rx.retain(|r| r.mt != mt);
            rx.push(Rx { mt, threshold: Some(t), x: x.to_vec(), y });
        }
        Ok(Self { energy, rx })
    }

    fn has(&self, mt: i32) -> bool {
        self.rx.iter().any(|r| r.mt == mt)
    }

    /// `get_reaction_components`.
    fn components(&self, mt: i32) -> Vec<i32> {
        let mut mts = Vec::new();
        if let Some(rule) = sum_rule(mt) {
            for m in rule {
                mts.extend(self.components(m));
            }
        }
        if !mts.is_empty() {
            mts
        } else if self.has(mt) {
            vec![mt]
        } else {
            Vec::new()
        }
    }

    /// `nuc_ce[mt].xs['0K'](energy)`: a stored reaction, or a redundant sum.
    fn eval(&self, mt: i32, at: &[f64]) -> Option<Vec<f64>> {
        if let Some(r) = self.rx.iter().find(|r| r.mt == mt) {
            return Some(tab_eval(&r.x, &r.y, at));
        }
        let comps = self.components(mt);
        if comps.is_empty() {
            return None;
        }
        let parts: Vec<&Rx> = comps.iter().filter_map(|m| self.rx.iter().find(|r| r.mt == *m)).collect();
        let idx = parts.iter().map(|r| r.threshold.unwrap_or(0)).min().unwrap_or(0);
        let sub = &self.energy[idx..];
        let mut sum = vec![0.0_f64; sub.len()];
        for r in &parts {
            for (s, v) in sum.iter_mut().zip(tab_eval(&r.x, &r.y, sub)) {
                *s += v;
            }
        }
        Some(tab_eval(sub, &sum, at))
    }
}

/// OpenMC's GNDS name (`Mg26`, `Am242_m1`).
fn gnds_name(za: i64, liso: i64) -> String {
    let z = (za / 1000) as u32;
    let a = za % 1000;
    let sym = crate::acquire::symbol_for_z(z).unwrap_or("X");
    if liso > 0 { format!("{sym}{a}_m{liso}") } else { format!("{sym}{a}") }
}

/// The NJOY deck `make_ace` writes for a 0 K, no-BROADR/HEATR/PURR run.
fn make_ace_deck(mat: i32, njoy_error: f64) -> String {
    format!(
        "reconr\n20 21\n'PENDF for WMP generation'/\n{mat} 2/\n{njoy_error}/ err\n'WMP generation'/\n\
         'Processed by NJOY'/\n0/\ngaspr\n20 21 22 /\nacer\n20 22 0 23 24\n1 0 1 .01 /\n\
         'WMP generation at 0.0'/\n{mat} 0.0\n1 1 1/\n/\nstop\n"
    )
}

/// Run this crate's RECONR -> GASPR -> ACER on `endf_path` at 0 K, as
/// `IncidentNeutron.from_njoy(temperatures=[0.0], error=njoy_error,
/// broadr=False, heatr=False, purr=False)` runs NJOY, and return the ACE.
pub fn zero_kelvin_ace(endf_path: &Path, mat: i32, njoy_error: f64) -> Result<RawAceTable, NjoyError> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("wmp-generate-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let result = (|| {
        std::fs::copy(endf_path, dir.join("tape20"))?;
        let run = run_deck(&make_ace_deck(mat, njoy_error), &DeckOptions { workdir: Some(dir.clone()), parse_only: false, run_date: None })?;
        if let Some((m, why)) = run.blocked {
            return Err(NjoyError::WmpData(format!("deck stopped at {m}: {why}")));
        }
        let text = run.units.text_or_file(23).ok_or_else(|| NjoyError::WmpData("ACER wrote no unit 23".into()))?;
        parse_type1(&text)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// The pointwise half of `vectfit_nuclide` (`multipole.py:419-476`): 0 K
/// ACE data cut at the lower of the resolved-range top and the first
/// threshold, with elastic, absorption (MT=27) and, if present, fission.
pub fn pointwise_from_endf(endf_path: &Path, njoy_error: f64) -> Result<PointwiseXs, NjoyError> {
    let tape = Tape::read_file(endf_path)?;
    let mat = *tape.materials().first().ok_or_else(|| NjoyError::WmpData("tape holds no material".into()))?;
    let head = tape.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let za = head.rows.first().map_or(0.0, |r| r[0]) as i64;
    let liso = head.rows.get(1).map_or(0.0, |r| r[3]) as i64;

    let ace = zero_kelvin_ace(endf_path, mat, njoy_error)?;
    let data = AceReactions::from_ace(&ace)?;
    let grid = &data.energy;

    // upper energy (multipole.py:430-445)
    let res = Mf2Resonances::read(&tape, mat)?;
    let resolved: Vec<_> = res.ranges.iter().filter(|r| r.range.l1 != 2).collect();
    if resolved.len() > 1 {
        return Err(NjoyError::WmpData("more than one resolved range present".into()));
    }
    let unresolved = res.ranges.iter().find(|r| r.range.l1 == 2);
    let e_max = match (resolved.first(), unresolved) {
        (Some(r), _) if r.range.l1 != 0 => r.range.c2,
        (_, Some(u)) => u.range.c1,
        _ => grid[grid.len() - 1],
    };
    let mut e_max_idx = searchsorted_right(grid, e_max) as isize - 1;
    for r in &data.rx {
        if let Some(t) = r.threshold {
            if 0 < t as isize && (t as isize) < e_max_idx {
                e_max_idx = t as isize;
            }
        }
    }
    let energy = grid[..(e_max_idx + 1) as usize].to_vec();
    let total = data.eval(1, &energy).ok_or_else(|| NjoyError::WmpData("no MT=1".into()))?;
    let elastic = data.eval(2, &energy).ok_or_else(|| NjoyError::WmpData("no MT=2".into()))?;
    let absorption = data.eval(27, &energy).unwrap_or_else(|| vec![0.0; energy.len()]);
    let fission = data.eval(18, &energy);
    Ok(PointwiseXs { name: gnds_name(za, liso), awr: ace.header.awr, energy, total, elastic, absorption, fission })
}
