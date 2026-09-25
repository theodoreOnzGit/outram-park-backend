// SPDX-License-Identifier: GPL-3.0

//! **The ACE delayed-neutron blocks: DNU, BDD, DNEDL, DNED** — GitHub #307.
//!
//! [`super::ce_decode`] gives cross sections and [`super::ce_laws`] the prompt
//! secondaries. Neither reads the **delayed** neutron data, so
//! `outram_mc_libs::Nuclide::from_ace` had `delayed: None` and the ACE route
//! could not do kinetics at all while the ENDF route could. Same asymmetry as
//! the URR omission this module's sibling closed.
//!
//! # Upstream is the specification
//!
//! Ported from `openmc/data/reaction.py:319-365` at OpenMC `afa7a14`, the
//! `if ace.jxs[24] > 0:` branch of `Reaction.from_ace`.
//!
//! | block | JXS | holds |
//! |---|---|---|
//! | DNU | 24 | `LNU`, then delayed ν̄_d as a TAB1 |
//! | BDD | 25 | per group: decay constant, then the group probability TAB1 |
//! | DNEDL | 26 | one locator per group into DNED |
//! | DNED | 27 | the delayed energy distributions |
//!
//! `NXS(8)` gives the number of precursor groups.
//!
//! # Two unit traps, both of which produce plausible numbers
//!
//! - **The decay constant is in inverse SHAKES**, not inverse seconds. A shake
//!   is 1e-8 s, so upstream multiplies by `1e8`. Skipping that gives λ eight
//!   orders of magnitude too small — a precursor with a half-life of years,
//!   which looks like a number rather than an error.
//! - **The group probabilities do not sum to 1** in an ACE file. Upstream
//!   renormalises by their sum (`reaction.py:362-365`). Without it the delayed
//!   fraction is wrong by however far off unity the file happens to be.
//!
//! # DNED is deliberately not read
//!
//! `DelayedData` carries no outgoing spectrum on **either** route: the ENDF
//! route's `DelayedData::from_tape` keeps `DelayedChi`'s `fraction` and drops
//! its `spectrum`. Reading DNED here would produce data nothing consumes, so
//! this decodes DNU and BDD and says why the other two are absent rather than
//! leaving a reader to wonder.

use crate::acer::ce_laws::read_tab1;
use crate::acer::read::RawAceTable;
use crate::acer::{jxs, nxs};
use crate::error::NjoyError;

/// Seconds per shake. ACE stores delayed decay constants per shake.
pub const SECONDS_PER_SHAKE: f64 = 1.0e-8;

/// The delayed-neutron data an ACE table carries.
#[derive(Debug, Clone, PartialEq)]
pub struct AceDelayed {
    /// Precursor decay constants λ \[s⁻¹\], in the file's group order.
    pub lambda: Vec<f64>,
    /// Incident-energy grid \[eV\] for delayed ν̄_d, ascending.
    pub energy: Vec<f64>,
    /// Total delayed ν̄_d aligned with [`Self::energy`].
    pub nu_delayed: Vec<f64>,
    /// Per group, that group's share `p_k(E)` as `(E [eV], fraction)`,
    /// **renormalised** so the shares sum to 1 — see the module docs.
    pub group_fraction: Vec<Vec<(f64, f64)>>,
}

/// Decode DNU + BDD, or `None` when the table carries no delayed data.
///
/// # Errors
///
/// A block whose declared extent runs past `XSS`, a group count that
/// disagrees with what BDD actually contains, or a TAB1 with no points.
///
/// `Ok(None)` means `JXS(24) == 0`, i.e. the table genuinely has no delayed
/// data — every non-fissionable nuclide, and a fissionable one processed
/// without it. That is not an error and must not be reported as one, but it
/// must also not look the same as "not decoded", which is the ambiguity #307
/// was filed about.
pub fn decode_delayed(t: &RawAceTable) -> Result<Option<AceDelayed>, NjoyError> {
    let dnu = t.jxs[jxs::DNU];
    if dnu <= 0 {
        return Ok(None);
    }
    let n_group = t.nxs[nxs::NDNF] as usize;
    if n_group == 0 {
        return Err(NjoyError::EndfParse(
            "the table has a DNU block but NXS(8) says zero precursor groups; a \
             delayed yield with no groups to distribute it over cannot be sampled, \
             and returning None here would hide that behind the same answer as a \
             nuclide with no delayed data at all"
                .into(),
        ));
    }

    // DNU: `LNU` at the locator, the TAB1 immediately after it. Only LNU=2
    // (tabular) occurs for delayed nu-bar; upstream reads it unconditionally as
    // a TAB1, and a polynomial delayed yield is not a form ACER emits.
    let (energy, nu_delayed, _) = read_tab1(t, (dnu - 1) as usize + 1, "DNU delayed nu-bar")?;

    // BDD: per group, the decay constant then the probability TAB1.
    let bdd = t.jxs[jxs::BDD];
    if bdd <= 0 {
        return Err(NjoyError::EndfParse(
            "the table has a DNU block but no BDD block, so the delayed yield \
             cannot be split into precursor groups"
                .into(),
        ));
    }
    let mut at = (bdd - 1) as usize;
    let mut lambda = Vec::with_capacity(n_group);
    let mut group_fraction = Vec::with_capacity(n_group);
    for g in 0..n_group {
        if at >= t.xss.len() {
            return Err(NjoyError::EndfParse(format!(
                "BDD block ended after {g} of {n_group} groups (NXS(8) says \
                 {n_group}); the file and its own header disagree"
            )));
        }
        // **Inverse shakes -> inverse seconds.** See the module docs: omitting
        // this is eight orders of magnitude and looks like a number.
        lambda.push(t.xss[at] / SECONDS_PER_SHAKE);
        let (e, p, next) = read_tab1(t, at + 1, &format!("BDD group {g} probability"))?;
        group_fraction.push(e.into_iter().zip(p).collect::<Vec<(f64, f64)>>());
        at = next;
    }

    // **Renormalise the shares.** The probabilities in an ACE file do not sum
    // to exactly 1 (`reaction.py:362-365` divides by their sum). Normalising on
    // the probability at each group's FIRST energy matches upstream's flat-case
    // treatment and is well defined for the energy-dependent case too, since
    // the shares are a partition at every energy.
    let total: f64 = group_fraction
        .iter()
        .filter_map(|g| g.first().map(|&(_, p)| p))
        .sum();
    if total > 0.0 {
        for g in group_fraction.iter_mut() {
            for (_, p) in g.iter_mut() {
                *p /= total;
            }
        }
    }

    Ok(Some(AceDelayed {
        lambda,
        energy,
        nu_delayed,
        group_fraction,
    }))
}
