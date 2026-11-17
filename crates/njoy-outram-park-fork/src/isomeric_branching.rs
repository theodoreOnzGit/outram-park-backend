// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Isomeric branching ratios from the evaluation, collapsed on a
//! spectrum** (gh:#1628, phase 2 of #1623). New work, not a port: OpenMC's
//! `Chain.from_endf` gives every reaction a branching ratio of 1 and leaves
//! the ground/isomer split to `set_branch_ratios` with a preset table.
//!
//! # What the evaluation carries
//!
//! For a reaction `MT` with radioactive products, MF=8 lists the product
//! states (`ZAP`, level energy `ELFS`, `LMF`, `LFS`). `LMF = 9` gives the
//! fraction of `σ_MT` to each state as a multiplicity `y_k(E)` in MF=9;
//! `LMF = 10` gives each state's production cross section `σ_k(E)` in MF=10.
//! [`production_states`] reads both, as GROUPR's `getsig`/`getyld` find them
//! (a subsection matched on `L1 = IZAP`, `L2 = LFS`).
//!
//! # The collapse
//!
//! [`collapse`] returns, for each state, `BR_k = ∫ σ_k φ dE / ∫ σ_MT φ dE`
//! with `σ_k = y_k σ_MT` (MF=9) or the MF=10 table. `σ_MT` is a pointwise
//! lin-lin table, normally RECONR's (resonances reconstructed: MF=3 alone is
//! only the background there). The flux is **group-wise**, `Φ_g` per group,
//! taken **flat in energy within each group**: a tallied group flux carries
//! no shape below its group width, and this is the assumption stated rather
//! than hidden.
//!
//! Integration is exact for the data: on the union of every table's
//! breakpoints and the group bounds, each factor is constant (INT = 1) or
//! linear (INT = 2), so the integrand is at most quadratic and Simpson's rule
//! is exact. Other interpolation laws are refused by name.
//!
//! # Naming the product
//!
//! The isomer index `m` of state `LFS > 0` is its rank among the nonzero
//! `LFS` of the same `ZAP` in MF=8, which is ENDF's isomer numbering when
//! MF=8 lists only the long-lived states (as it is meant to). A state that is
//! a short-lived level rather than an isomer would be misnamed; check MF=8.
//!
//! # Verification status (2026-11-17)
//!
//! Analytic and code-internal checks only (tests below). **Not yet compared
//! with GROUPR's grouped MF=10 production** on the same weight (gh:#1628,
//! verification 1), nor with OpenMC's preset ratios.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::error::NjoyError;

/// Where a state's production comes from.
#[derive(Debug, Clone)]
pub enum StateData {
    /// MF=9: fraction of `σ_MT` versus energy.
    Multiplicity(Tab1),
    /// MF=10: the state's production cross section \[b\] versus energy.
    CrossSection(Tab1),
}

/// One product state of a reaction, from MF=8 and MF=9/10.
#[derive(Debug, Clone)]
pub struct ProductionState {
    /// Reaction MT.
    pub mt: i32,
    /// Product `1000 Z + A`.
    pub zap: i32,
    /// Product level number in the evaluation (`LFS`, 0 = ground).
    pub lfs: i32,
    /// Isomer index `m` (0 = ground); see the module docs.
    pub isomer: u32,
    /// Level energy \[eV\] (`ELFS`).
    pub level_energy_ev: f64,
    /// The production data.
    pub data: StateData,
}

impl ProductionState {
    /// The product's GNDS name, e.g. `"Zn69_m1"`, or `None` for an unknown Z.
    pub fn product_name(&self) -> Option<String> {
        let z = (self.zap / 1000) as u32;
        let a = self.zap % 1000;
        let sym = crate::acquire::symbol_for_z(z)?;
        Some(if self.isomer > 0 { format!("{sym}{a}_m{}", self.isomer) } else { format!("{sym}{a}") })
    }
}

/// The product states of `mt` on `mat` with MF=9 or MF=10 data. Empty when
/// MF=8 has no section for `mt`; states whose `LMF` is neither 9 nor 10 are
/// left out and named in the second element, so nothing is silently dropped.
///
/// # Errors
/// An MF=8 state with `LMF = 9/10` whose MF=9/10 subsection is missing, or a
/// malformed record.
pub fn production_states(tape: &Tape, mat: i32, mt: i32) -> Result<(Vec<ProductionState>, Vec<String>), NjoyError> {
    let Some(mf8) = tape.section(mat, 8, mt) else { return Ok((Vec::new(), Vec::new())) };
    let mut cur = SectionCursor::new(&mf8.rows);
    let head = cur.read_cont()?;
    let (ns, no) = (head.n1.max(0), head.n2);
    let mut listed: Vec<(i32, f64, i32, i32)> = Vec::new(); // zap, elfs, lmf, lfs
    for _ in 0..ns {
        let c = cur.read_cont()?;
        listed.push((c.c1.round() as i32, c.c2, c.l1, c.l2));
        if no == 0 {
            cur.read_list()?;
        }
    }
    let mut states = Vec::new();
    let mut skipped = Vec::new();
    for &(zap, elfs, lmf, lfs) in &listed {
        if lmf != 9 && lmf != 10 {
            skipped.push(format!("MT={mt} ZAP={zap} LFS={lfs}: LMF={lmf} (no MF=9/10 production data)"));
            continue;
        }
        let tab = find_subsection(tape, mat, lmf, mt, zap, lfs)?;
        let mut levels: Vec<i32> = listed.iter().filter(|s| s.0 == zap && s.3 > 0).map(|s| s.3).collect();
        levels.sort_unstable();
        levels.dedup();
        let isomer = if lfs == 0 { 0 } else { levels.iter().position(|&l| l == lfs).map_or(0, |p| p as u32 + 1) };
        let data = if lmf == 9 { StateData::Multiplicity(tab) } else { StateData::CrossSection(tab) };
        states.push(ProductionState { mt, zap, lfs, isomer, level_energy_ev: elfs, data });
    }
    Ok((states, skipped))
}

fn find_subsection(tape: &Tape, mat: i32, mf: i32, mt: i32, zap: i32, lfs: i32) -> Result<Tab1, NjoyError> {
    let sec = tape.section(mat, mf, mt).ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    for _ in 0..head.n1.max(0) {
        let t = cur.read_tab1()?;
        if t.head.l1 == zap && t.head.l2 == lfs {
            return Ok(t);
        }
    }
    Err(NjoyError::InvalidInput(format!("MF={mf}/MT={mt}: no subsection for IZAP={zap}, LFS={lfs}")))
}

/// Check every region of `t` is INT = 1 or 2.
fn check_laws(t: &Tab1, what: &str) -> Result<(), NjoyError> {
    for &(_, law) in &t.interp {
        if law != 1 && law != 2 {
            return Err(NjoyError::NotPorted(if what == "MF=9" {
                "isomeric_branching: an MF=9 interpolation law other than 1 or 2"
            } else {
                "isomeric_branching: an MF=10 interpolation law other than 1 or 2"
            }));
        }
    }
    Ok(())
}

/// Value of a TAB1 (INT 1 or 2) at `x`, taking the limit from the right
/// (`right = true`) or the left at a breakpoint. Zero outside the table.
fn tab1_at(t: &Tab1, x: f64, right: bool) -> f64 {
    // The law of point i+1's region (NBT is 1-based, inclusive of its end).
    eval_pairs(&t.pairs, x, right, |i| t.interp.iter().find(|(nbt, _)| (i + 2) as u32 <= *nbt).map_or(2, |r| r.1))
}

fn lin_at(p: &[(f64, f64)], x: f64, right: bool) -> f64 {
    eval_pairs(p, x, right, |_| 2)
}

fn eval_pairs(p: &[(f64, f64)], x: f64, right: bool, law_of: impl Fn(usize) -> u32) -> f64 {
    if p.is_empty() || x < p[0].0 || x > p[p.len() - 1].0 {
        return 0.0;
    }
    if p.len() == 1 {
        return p[0].1;
    }
    let last = p.len() - 2;
    let i = if right { p.partition_point(|q| q.0 <= x) } else { p.partition_point(|q| q.0 < x) };
    let i = i.saturating_sub(1).min(last);
    let (x1, y1) = p[i];
    let (x2, y2) = p[i + 1];
    if law_of(i) == 1 || x2 == x1 {
        return y1;
    }
    y1 + (x - x1) * (y2 - y1) / (x2 - x1)
}

/// One state's collapsed result.
#[derive(Debug, Clone, PartialEq)]
pub struct CollapsedState {
    /// Product name (`None` for an unknown Z).
    pub product: Option<String>,
    /// `∫ σ_k φ / ∫ σ_MT φ`.
    pub branching_ratio: f64,
}

/// `BR_k = ∫ σ_k φ / ∫ σ_MT φ` for each state, with `φ` flat in energy
/// within each group: `bounds` are the `G + 1` ascending group edges \[eV\]
/// and `group_flux` the `G` group fluxes. `sigma_mt` is the reaction's
/// pointwise lin-lin cross section (e.g. a RECONR section's `pairs`).
///
/// # Errors
/// Mismatched lengths, unsorted bounds, a law other than INT 1/2, or a zero
/// denominator (the reaction has no rate in this spectrum).
pub fn collapse(
    states: &[ProductionState],
    sigma_mt: &[(f64, f64)],
    bounds: &[f64],
    group_flux: &[f64],
) -> Result<Vec<CollapsedState>, NjoyError> {
    if bounds.len() != group_flux.len() + 1 || bounds.windows(2).any(|w| !(w[1] > w[0])) {
        return Err(NjoyError::InvalidInput("collapse: need G+1 ascending bounds for G fluxes".into()));
    }
    for s in states {
        match &s.data {
            StateData::Multiplicity(t) => check_laws(t, "MF=9")?,
            StateData::CrossSection(t) => check_laws(t, "MF=10")?,
        }
    }
    let mut num = vec![0.0; states.len()];
    let mut den = 0.0;
    for (g, (&lo, &hi)) in bounds.iter().zip(&bounds[1..]).enumerate() {
        let w = group_flux[g] / (hi - lo);
        if w == 0.0 {
            continue;
        }
        let mut xs: Vec<f64> = vec![lo, hi];
        xs.extend(sigma_mt.iter().map(|p| p.0).filter(|&e| e > lo && e < hi));
        for s in states {
            let t = match &s.data {
                StateData::Multiplicity(t) | StateData::CrossSection(t) => t,
            };
            xs.extend(t.pairs.iter().map(|p| p.0).filter(|&e| e > lo && e < hi));
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        xs.dedup();
        for seg in xs.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let m = 0.5 * (a + b);
            let h = (b - a) / 6.0;
            let sig = |x: f64, r: bool| lin_at(sigma_mt, x, r);
            let (sa, sm, sb) = (sig(a, true), sig(m, true), sig(b, false));
            den += w * h * (sa + 4.0 * sm + sb);
            for (k, s) in states.iter().enumerate() {
                let f = |x: f64, r: bool| match &s.data {
                    StateData::Multiplicity(t) => tab1_at(t, x, r) * sig(x, r),
                    StateData::CrossSection(t) => tab1_at(t, x, r),
                };
                num[k] += w * h * (f(a, true) + 4.0 * f(m, true) + f(b, false));
            }
        }
    }
    if !(den > 0.0) {
        return Err(NjoyError::InvalidInput("collapse: the reaction has no rate in this spectrum".into()));
    }
    Ok(states
        .iter()
        .zip(num)
        .map(|(s, n)| CollapsedState { product: s.product_name(), branching_ratio: n / den })
        .collect())
}

/// What [`apply_to_chain`] did for one parent.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedRatios {
    /// Parent nuclide.
    pub parent: String,
    /// `(product, ratio)` set on the chain.
    pub set: Vec<(String, f64)>,
    /// States collapsed but left out because the product is not in the
    /// chain (named, never dropped silently).
    pub not_in_chain: Vec<(String, f64)>,
}

/// **Spectrum-derived branching ratios applied to a chain** (gh:#1628):
/// collapse `states` of `parent`'s `reaction` (e.g. `"(n,gamma)"`) on the
/// group flux and write them through the ported
/// [`set_branch_ratios`](crate::hdf5::depletion_chain_ops::set_branch_ratios)
/// (strict, tolerance 1e-5), so the chain keeps its format and exports as
/// before. Products absent from the chain are reported, not set.
///
/// # Errors
/// The collapse's errors; no state whose product is in the chain; and
/// `set_branch_ratios`' strict checks (a sum outside 1 ± 1e-5 when a
/// ground state is among the products).
#[cfg(feature = "std")]
pub fn apply_to_chain(
    chain: &mut crate::hdf5::depletion_chain_xml::DepletionChainXml,
    parent: &str,
    reaction: &str,
    states: &[ProductionState],
    sigma_mt: &[(f64, f64)],
    bounds: &[f64],
    group_flux: &[f64],
) -> Result<AppliedRatios, NjoyError> {
    let collapsed = collapse(states, sigma_mt, bounds, group_flux)?;
    let mut set = Vec::new();
    let mut not_in_chain = Vec::new();
    for c in collapsed {
        let name = c.product.ok_or_else(|| NjoyError::InvalidInput("a product with an unknown Z".into()))?;
        if chain.get(&name).is_some() {
            set.push((name, c.branching_ratio));
        } else {
            not_in_chain.push((name, c.branching_ratio));
        }
    }
    if set.is_empty() {
        return Err(NjoyError::InvalidInput(format!("{parent} {reaction}: no product state is in the chain")));
    }
    let pairs: Vec<(&str, f64)> = set.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    crate::hdf5::depletion_chain_ops::set_branch_ratios(chain, &[(parent, &pairs)], reaction, true, 1e-5)?;
    Ok(AppliedRatios { parent: parent.to_owned(), set, not_in_chain })
}

#[cfg(test)]
mod tests;
