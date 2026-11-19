// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac
//   Upstream files, read line for line:
//     openmc/deplete/chain.py    (REACTIONS, Chain.export_to_xml,
//                                 Chain.get_branch_ratios,
//                                 Chain.set_branch_ratios, Chain.reduce,
//                                 Chain._follow)
//     openmc/deplete/nuclide.py  (Nuclide.to_xml_element,
//                                 FissionYieldDistribution.to_xml_element,
//                                 FissionYieldDistribution.restrict_products)
//
// An OUTRAM PARK translation for research, education and V&V only.

//! **Operations on a depletion chain** (gh:#1621): write it back to XML,
//! read and set reaction branching ratios, and reduce it to what a set of
//! starting nuclides can reach. The parts of OpenMC's `Chain` that need no
//! nuclear data beyond the chain itself. `Chain.from_endf`, which builds a
//! chain from decay, fission-yield and neutron sublibraries, is not here;
//! it waits on the data (gh:#1627).
//!
//! # Fidelity
//!
//! - [`export_to_xml`] writes what upstream's lxml `pretty_print` writes:
//!   two-space indent, self-closing empty elements, attributes in upstream's
//!   order, no XML declaration (lxml writes none for UTF-8), and every float
//!   as Python's `str(float)` ([`python_float_repr`]). Checked **byte for
//!   byte** against `chain_ni.xml`, a file OpenMC wrote
//!   (`tests` below). The other two committed chains were written by hand
//!   and are checked by round trip only.
//! - [`reduce`] follows `_follow` level by level, including its quirks: decay
//!   modes contribute no light secondaries (only `REACTIONS` keys do), and
//!   fission products are followed through the yields.
//! - [`set_branch_ratios`] keeps upstream's validation and its fill-in of a
//!   ground-state remainder when every given target is metastable.
//!
//! # Where this port departs, and why
//!
//! 1. **Errors instead of Python exceptions and warnings.** In strict mode
//!    each upstream `raise` is an `Err`; in lenient mode upstream's warnings
//!    are returned as strings.
//! 2. **A reaction target outside the chain**, reached by [`reduce`], is an
//!    error naming it. Upstream adds it to the frontier and then fails with
//!    a bare `KeyError` at `self[iso]` on the next level.
//! 3. **No ground target among the existing reactions** while the new ratios
//!    sum to less than `1 + tol`: upstream raises `KeyError` from
//!    `grounds[parent]`; here it is an error saying so. A reaction with no
//!    target (upstream's `"_m" in None` is a `TypeError`) is not counted as
//!    a ground target.
//! 4. **Yield product order on export** is the order held in memory (file
//!    order for a parsed chain, name order after [`reduce`]). Whether
//!    upstream re-sorts products of a parsed file was not checked.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::collections::BTreeSet;
use core::fmt::Write as _;

use super::depletion_chain_xml::{ChainDecay, ChainNuclide, ChainReaction, DepletionChainXml, FissionYieldSet};
use crate::acquire::{symbol_for_z, z_for_symbol};
use crate::error::NjoyError;

/// Upstream `REACTIONS`: each reaction type with its light secondary
/// products (the MT numbers are not needed here). Transcribed from
/// `chain.py` at `e4febe9`, 84 entries, in upstream order.
pub const REACTION_SECONDARIES: &[(&str, &[&str])] = &[
    ("(n,2nd)", &["H2"]), ("(n,2n)", &[]), ("(n,3n)", &[]), ("(n,na)", &["He4"]),
    ("(n,n3a)", &["He4", "He4", "He4"]), ("(n,2na)", &["He4"]), ("(n,3na)", &["He4"]),
    ("(n,np)", &["H1"]), ("(n,n2a)", &["He4", "He4"]), ("(n,2n2a)", &["He4", "He4"]),
    ("(n,nd)", &["H2"]), ("(n,nt)", &["H3"]), ("(n,n3He)", &["He3"]),
    ("(n,nd2a)", &["H2", "He4", "He4"]), ("(n,nt2a)", &["H3", "He4", "He4"]), ("(n,4n)", &[]),
    ("(n,2np)", &["H1"]), ("(n,3np)", &["H1"]), ("(n,n2p)", &["H1", "H1"]),
    ("(n,npa)", &["H1", "He4"]), ("(n,gamma)", &[]), ("(n,p)", &["H1"]), ("(n,d)", &["H2"]),
    ("(n,t)", &["H3"]), ("(n,3He)", &["He3"]), ("(n,a)", &["He4"]), ("(n,2a)", &["He4", "He4"]),
    ("(n,3a)", &["He4", "He4", "He4"]), ("(n,2p)", &["H1", "H1"]), ("(n,pa)", &["H1", "He4"]),
    ("(n,t2a)", &["H3", "He4", "He4"]), ("(n,d2a)", &["H2", "He4", "He4"]),
    ("(n,pd)", &["H1", "H2"]), ("(n,pt)", &["H1", "H3"]), ("(n,da)", &["H2", "He4"]),
    ("(n,5n)", &[]), ("(n,6n)", &[]), ("(n,2nt)", &["H3"]), ("(n,ta)", &["H3", "He4"]),
    ("(n,4np)", &["H1"]), ("(n,3nd)", &["H2"]), ("(n,nda)", &["H2", "He4"]),
    ("(n,2npa)", &["H1", "He4"]), ("(n,7n)", &[]), ("(n,8n)", &[]), ("(n,5np)", &["H1"]),
    ("(n,6np)", &["H1"]), ("(n,7np)", &["H1"]), ("(n,4na)", &["He4"]), ("(n,5na)", &["He4"]),
    ("(n,6na)", &["He4"]), ("(n,7na)", &["He4"]), ("(n,4nd)", &["H2"]), ("(n,5nd)", &["H2"]),
    ("(n,6nd)", &["H2"]), ("(n,3nt)", &["H3"]), ("(n,4nt)", &["H3"]), ("(n,5nt)", &["H3"]),
    ("(n,6nt)", &["H3"]), ("(n,2n3He)", &["He3"]), ("(n,3n3He)", &["He3"]),
    ("(n,4n3He)", &["He3"]), ("(n,3n2p)", &["H1", "H1"]), ("(n,3n2a)", &["He4", "He4"]),
    ("(n,3npa)", &["H1", "He4"]), ("(n,dt)", &["H2", "H3"]), ("(n,npd)", &["H1", "H2"]),
    ("(n,npt)", &["H1", "H3"]), ("(n,ndt)", &["H2", "H3"]), ("(n,np3He)", &["H1", "He3"]),
    ("(n,nd3He)", &["H2", "He3"]), ("(n,nt3He)", &["H3", "He3"]), ("(n,nta)", &["H3", "He4"]),
    ("(n,2n2p)", &["H1", "H1"]), ("(n,p3He)", &["H1", "He3"]), ("(n,d3He)", &["H2", "He3"]),
    ("(n,3Hea)", &["He3", "He4"]), ("(n,4n2p)", &["H1", "H1"]), ("(n,4n2a)", &["He4", "He4"]),
    ("(n,4npa)", &["H1", "He4"]), ("(n,3p)", &["H1", "H1", "H1"]), ("(n,n3p)", &["H1", "H1", "H1"]),
    ("(n,3n2pa)", &["H1", "H1", "He4"]), ("(n,5n2p)", &["H1", "H1"]),
];

/// The light secondaries of a reaction type; empty for a type not in
/// upstream's table (decay modes, `fission`).
pub fn secondaries_of(kind: &str) -> &'static [&'static str] {
    REACTION_SECONDARIES.iter().find(|(k, _)| *k == kind).map_or(&[], |(_, s)| s)
}

fn invalid(msg: String) -> NjoyError {
    NjoyError::InvalidInput(msg)
}

/// `openmc.data.zam` for a GNDS name: `"Am242_m1"` → `(95, 242, 1)`.
///
/// # Errors
/// A name that is not `<symbol><A>[_m<k>]` with a known symbol.
pub fn zam(name: &str) -> Result<(u32, u32, u32), NjoyError> {
    let split = name.find(|c: char| c.is_ascii_digit()).ok_or_else(|| invalid(format!("`{name}` is not a GNDS nuclide name")))?;
    let (sym, rest) = name.split_at(split);
    let z = z_for_symbol(sym)
        .filter(|z| symbol_for_z(*z) == Some(sym))
        .ok_or_else(|| invalid(format!("`{name}`: unknown element `{sym}`")))?;
    let (a, m) = match rest.split_once("_m") {
        Some((a, m)) => (a, m.parse::<u32>().map_err(|_| invalid(format!("`{name}`: bad metastable index")))?),
        None => (rest, 0),
    };
    let a = a.parse::<u32>().map_err(|_| invalid(format!("`{name}`: bad mass number")))?;
    Ok((z, a, m))
}

/// `openmc.data.gnds_name(Z, A, m)`.
fn gnds_name(z: u32, a: u32, m: u32) -> Option<String> {
    let sym = symbol_for_z(z)?;
    Some(if m > 0 { format!("{sym}{a}_m{m}") } else { format!("{sym}{a}") })
}

/// Python's `str(float)` (`repr`): the shortest digits that round-trip,
/// fixed notation for decimal exponents in `-4 ..= 15`, otherwise
/// scientific with a signed exponent of at least two digits (`1e-05`,
/// `1.5e+16`). `2224631.0`, `0.1`, `-0.0`, `inf`, `nan`.
pub fn python_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // Rust's `{:e}` is also the shortest round-trip representation.
    let e = format!("{x:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((e.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if (-4..16).contains(&exp) {
        if exp >= 0 {
            let point = (exp + 1) as usize;
            if digits.len() <= point {
                out.push_str(&digits);
                out.extend(core::iter::repeat_n('0', point - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            }
        } else {
            out.push_str("0.");
            out.extend(core::iter::repeat_n('0', (-exp - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let _ = write!(out, "e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    out
}

fn join_floats(v: &[f64]) -> String {
    v.iter().map(|x| python_float_repr(*x)).collect::<Vec<_>>().join(" ")
}

/// XML attribute escaping as lxml does for these values.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn write_nuclide(out: &mut String, n: &ChainNuclide) {
    let mut attrs = format!("name=\"{}\"", esc(&n.name));
    let mut children: Vec<String> = Vec::new();
    if let Some(t) = n.half_life_seconds {
        let _ = write!(attrs, " half_life=\"{}\"", python_float_repr(t));
        let _ = write!(attrs, " decay_modes=\"{}\"", n.decays.len());
        let _ = write!(attrs, " decay_energy=\"{}\"", python_float_repr(n.decay_energy_ev.unwrap_or(0.0)));
        for d in &n.decays {
            let mut c = format!("<decay type=\"{}\"", esc(&d.mode));
            if let Some(t) = &d.target {
                let _ = write!(c, " target=\"{}\"", esc(t));
            }
            let _ = write!(c, " branching_ratio=\"{}\"/>", python_float_repr(d.branching_ratio));
            children.push(c);
        }
    }
    for s in &n.sources {
        let mut c = format!("<source type=\"{}\"", esc(&s.distribution));
        if let Some(i) = &s.interpolation {
            let _ = write!(c, " interpolation=\"{}\"", esc(i));
        }
        let _ = write!(c, " particle=\"{}\">\n      <parameters>{}</parameters>\n    </source>", esc(&s.particle), join_floats(&s.parameters));
        children.push(c);
    }
    let _ = write!(attrs, " reactions=\"{}\"", n.reactions.len());
    for r in &n.reactions {
        let mut c = format!("<reaction type=\"{}\" Q=\"{}\"", esc(&r.kind), python_float_repr(r.q_ev));
        if let Some(t) = &r.target {
            let _ = write!(c, " target=\"{}\"", esc(t));
        }
        if r.branching_ratio != 1.0 {
            let _ = write!(c, " branching_ratio=\"{}\"", python_float_repr(r.branching_ratio));
        }
        c.push_str("/>");
        children.push(c);
    }
    if !n.fission_yields.is_empty() {
        let c = match &n.fission_yield_parent {
            Some(p) => format!("<neutron_fission_yields parent=\"{}\"/>", esc(p)),
            None => {
                let energies: Vec<f64> = n.fission_yields.iter().map(|y| y.energy_ev).collect();
                let mut c = format!("<neutron_fission_yields>\n      <energies>{}</energies>", join_floats(&energies));
                for y in &n.fission_yields {
                    let products: Vec<&str> = y.yields.iter().map(|(p, _)| p.as_str()).collect();
                    let data: Vec<f64> = y.yields.iter().map(|(_, v)| *v).collect();
                    let _ = write!(
                        c,
                        "\n      <fission_yields energy=\"{}\">\n        <products>{}</products>\n        <data>{}</data>\n      </fission_yields>",
                        python_float_repr(y.energy_ev),
                        products.join(" "),
                        join_floats(&data)
                    );
                }
                c.push_str("\n    </neutron_fission_yields>");
                c
            }
        };
        children.push(c);
    }
    if children.is_empty() {
        let _ = writeln!(out, "  <nuclide {attrs}/>");
    } else {
        let _ = writeln!(out, "  <nuclide {attrs}>");
        for c in children {
            let _ = writeln!(out, "    {c}");
        }
        out.push_str("  </nuclide>\n");
    }
}

/// Upstream `Chain.export_to_xml`, returning the file's text (the caller
/// writes it). Element and attribute order and float formatting follow
/// `Nuclide.to_xml_element` and lxml's pretty printer.
pub fn export_to_xml(chain: &DepletionChainXml) -> String {
    let mut out = String::from("<depletion_chain>\n");
    for n in &chain.nuclides {
        write_nuclide(&mut out, n);
    }
    out.push_str("</depletion_chain>\n");
    out
}

/// Upstream `Chain.get_branch_ratios`: per parent, the targets of
/// `reaction` whose branching ratio is not 1, in chain order.
pub fn get_branch_ratios(chain: &DepletionChainXml, reaction: &str) -> Vec<(String, Vec<(Option<String>, f64)>)> {
    let mut out = Vec::new();
    for n in &chain.nuclides {
        let mut capt: Vec<(Option<String>, f64)> = Vec::new();
        for r in &n.reactions {
            if r.kind == reaction && r.branching_ratio != 1.0 {
                // Upstream's dict: a repeated target keeps the last ratio.
                match capt.iter_mut().find(|(t, _)| *t == r.target) {
                    Some((_, b)) => *b = r.branching_ratio,
                    None => capt.push((r.target.clone(), r.branching_ratio)),
                }
            }
        }
        if !capt.is_empty() {
            out.push((n.name.clone(), capt));
        }
    }
    out
}

/// Upstream `Chain.set_branch_ratios`. `branch_ratios` is `(parent,
/// [(target, ratio)])`. In `strict` mode every inconsistency is an error;
/// otherwise the offending parent is left unchanged and the returned
/// warnings say why (upstream's `warn` calls).
///
/// The reactions of type `reaction` on each parent (other than those
/// producing a secondary of that reaction) are replaced by one per given
/// target, with the first replaced reaction's Q. When every given target is
/// metastable and the ratios sum to less than 1, the remainder goes to the
/// ground state: the existing ground target, or `Z, A+1, 0`.
///
/// # Errors
/// Strict mode: an unknown parent or product, a parent without the
/// reaction, a ratio sum outside tolerance. Either mode: no parent left to
/// modify (upstream's `IndexError`), and departure 3 in the module docs.
pub fn set_branch_ratios(
    chain: &mut DepletionChainXml,
    branch_ratios: &[(&str, &[(&str, f64)])],
    reaction: &str,
    strict: bool,
    tolerance: f64,
) -> Result<Vec<String>, NjoyError> {
    let tolerance = tolerance.abs();
    let secondary = secondaries_of(reaction);
    let mut warnings = Vec::new();
    let mut missing_parents: BTreeSet<String> = BTreeSet::new();
    let mut missing_products: Vec<(String, String)> = Vec::new();
    let mut missing_reaction: BTreeSet<String> = BTreeSet::new();
    let mut bad_sums: Vec<(String, f64)> = Vec::new();
    // (parent index, reaction indexes, sum, ground target)
    let mut plan: Vec<(usize, Vec<usize>, f64, Option<String>)> = Vec::new();

    'parents: for (parent, sub) in branch_ratios {
        let Some(pi) = chain.nuclides.iter().position(|n| n.name == *parent) else {
            if strict {
                return Err(invalid(format!("KeyError: {parent} is not in the chain")));
            }
            missing_parents.insert((*parent).to_owned());
            continue;
        };
        for (product, _) in *sub {
            if chain.get(product).is_none() {
                if strict {
                    return Err(invalid(format!("KeyError: product {product} is not in the chain")));
                }
                missing_products.push(((*parent).to_owned(), (*product).to_owned()));
                continue 'parents;
            }
        }
        let mut indexes = Vec::new();
        let mut ground: Option<String> = None;
        for (ix, rx) in chain.nuclides[pi].reactions.iter().enumerate() {
            let is_secondary = rx.target.as_deref().is_some_and(|t| secondary.contains(&t));
            if rx.kind == reaction && !is_secondary {
                indexes.push(ix);
                if let Some(t) = &rx.target {
                    if !t.contains("_m") {
                        ground = Some(t.clone());
                    }
                }
            }
        }
        if indexes.is_empty() {
            if strict {
                return Err(invalid(format!("Nuclide {parent} does not have {reaction} reactions")));
            }
            missing_reaction.insert((*parent).to_owned());
            continue;
        }
        let this_sum: f64 = sub.iter().map(|(_, b)| *b).sum();
        let too_low = if this_sum >= 1.0 + tolerance {
            false
        } else {
            let g = ground.as_deref().ok_or_else(|| {
                invalid(format!(
                    "{parent} has no ground-state {reaction} target; upstream raises KeyError from grounds[parent] here"
                ))
            })?;
            sub.iter().any(|(t, _)| *t == g) && this_sum <= 1.0 - tolerance
        };
        if this_sum >= 1.0 + tolerance || too_low {
            if strict {
                return Err(invalid(format!(
                    "Sum of {reaction} branching ratios for {parent} ({this_sum:7.3}) outside tolerance of 1 +/- {tolerance:5.3e}"
                )));
            }
            bad_sums.push(((*parent).to_owned(), this_sum));
        } else {
            plan.push((pi, indexes, this_sum, ground));
        }
    }
    if plan.is_empty() {
        return Err(invalid(format!("IndexError: No {reaction} reactions found in this Chain")));
    }
    if !missing_parents.is_empty() {
        warnings.push(format!("The following nuclides were not found in Chain: {}", missing_parents.into_iter().collect::<Vec<_>>().join(", ")));
    }
    if !missing_reaction.is_empty() {
        warnings.push(format!(
            "The following nuclides did not have {reaction} reactions: {}",
            missing_reaction.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if !missing_products.is_empty() {
        missing_products.sort();
        let tail: Vec<String> = missing_products.iter().map(|(k, v)| format!("{k} -> {v}")).collect();
        warnings.push(format!("The following products were not found in the Chain and parents were unmodified: \n{}", tail.join(", ")));
    }
    if !bad_sums.is_empty() {
        bad_sums.sort_by(|a, b| a.0.cmp(&b.0));
        let tail: Vec<String> = bad_sums.iter().map(|(k, s)| format!("{k}: {s:5.3}")).collect();
        warnings.push(format!(
            "The following parent nuclides were given {reaction} branch ratios with a sum outside tolerance of 1 +/- {tolerance:5.3e}:\n{}",
            tail.join("\n")
        ));
    }

    for (pi, indexes, sum, ground) in plan {
        let parent_name = chain.nuclides[pi].name.clone();
        let sub = branch_ratios.iter().find(|(p, _)| *p == parent_name).map(|(_, s)| *s).unwrap_or(&[]);
        let nuc = &mut chain.nuclides[pi];
        let q = nuc.reactions[indexes[0]].q_ev;
        for ix in indexes.iter().rev() {
            nuc.reactions.remove(*ix);
        }
        let mut all_meta = true;
        for (target, br) in sub {
            all_meta = all_meta && target.contains("_m");
            nuc.reactions.push(ChainReaction { kind: reaction.to_owned(), target: Some((*target).to_owned()), q_ev: q, branching_ratio: *br });
        }
        if all_meta && sum != 1.0 {
            let target = match ground {
                Some(g) => g,
                None => {
                    let (z, a, _) = zam(&parent_name)?;
                    gnds_name(z, a + 1, 0).ok_or_else(|| invalid(format!("no element for Z = {z}")))?
                }
            };
            nuc.reactions.push(ChainReaction { kind: reaction.to_owned(), target: Some(target), q_ev: q, branching_ratio: 1.0 - sum });
        }
    }
    Ok(warnings)
}

/// Upstream `Chain._follow`: every nuclide reachable from `start` within
/// `level` transmutations (`None` = no limit).
fn follow(chain: &DepletionChainXml, start: &BTreeSet<String>, level: Option<usize>) -> Result<BTreeSet<String>, NjoyError> {
    let all: BTreeSet<String> = chain.nuclides.iter().map(|n| n.name.clone()).collect();
    let missing: Vec<&String> = start.difference(&all).collect();
    if !missing.is_empty() {
        return Err(invalid(format!(
            "The following isotopes were not found in the chain: {}",
            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    let mut found = start.clone();
    if level == Some(0) {
        return Ok(found);
    }
    let mut remaining: BTreeSet<String> = all.difference(&found).cloned().collect();
    let mut isotopes = start.clone();
    let mut next_iso: BTreeSet<String> = BTreeSet::new();
    let mut depth = 0usize;
    while level.is_none_or(|l| depth < l) && !remaining.is_empty() {
        for iso in core::mem::take(&mut isotopes) {
            found.insert(iso.clone());
            let nuc = chain.get(&iso).ok_or_else(|| {
                invalid(format!("{iso} is a reaction or decay target but not a nuclide of the chain (upstream: KeyError)"))
            })?;
            let paths = nuc
                .reactions
                .iter()
                .map(|r| (r.kind.as_str(), r.target.as_deref()))
                .chain(nuc.decays.iter().map(|d| (d.mode.as_str(), d.target.as_deref())));
            for (kind, target) in paths {
                if kind == "fission" {
                    continue;
                }
                let secs = secondaries_of(kind).iter().copied().filter(|s| all.contains(*s));
                for product in target.into_iter().chain(secs) {
                    if next_iso.contains(product) || found.contains(product) || isotopes.contains(product) {
                        continue;
                    }
                    next_iso.insert(product.to_owned());
                }
            }
            for set in &nuc.fission_yields {
                for (product, _) in &set.yields {
                    if next_iso.contains(product) || found.contains(product) || isotopes.contains(product) {
                        continue;
                    }
                    next_iso.insert(product.clone());
                }
            }
        }
        if next_iso.is_empty() {
            return Ok(found);
        }
        depth += 1;
        for n in &next_iso {
            remaining.remove(n);
        }
        isotopes.append(&mut next_iso);
    }
    found.append(&mut isotopes);
    Ok(found)
}

/// Upstream `Chain.reduce`: the chain restricted to what `initial` can
/// reach within `level` transmutation or decay steps (`None` = all).
/// Nuclides come out in `zam` order. A decay or reaction whose target is
/// dropped keeps its rate with no target, so destruction and decay rates
/// are preserved; fission keeps only the yields to retained products, and
/// is dropped with its yields when none remain.
///
/// # Errors
/// A starting nuclide not in the chain; a name `zam` cannot parse; or
/// departure 2 of the module docs.
pub fn reduce(chain: &DepletionChainXml, initial: &[&str], level: Option<usize>) -> Result<DepletionChainXml, NjoyError> {
    let start: BTreeSet<String> = initial.iter().map(|s| (*s).to_owned()).collect();
    let all = follow(chain, &start, level)?;
    let mut ordered: Vec<(u32, u32, u32, String)> = Vec::with_capacity(all.len());
    for name in &all {
        let (z, a, m) = zam(name)?;
        ordered.push((z, a, m, name.clone()));
    }
    ordered.sort();
    let mut nuclides = Vec::with_capacity(ordered.len());
    for (_, _, _, name) in ordered {
        let prev = chain.get(&name).ok_or_else(|| invalid(format!("{name} not in chain")))?;
        let decays = prev
            .decays
            .iter()
            .map(|d| ChainDecay {
                mode: d.mode.clone(),
                target: d.target.clone().filter(|t| all.contains(t)),
                branching_ratio: d.branching_ratio,
            })
            .collect();
        let mut reactions = Vec::new();
        let mut fission_yields: Vec<FissionYieldSet> = Vec::new();
        for r in &prev.reactions {
            if r.target.as_ref().is_some_and(|t| all.contains(t)) {
                reactions.push(r.clone());
            } else if r.kind == "fission" {
                let restricted: Vec<FissionYieldSet> = prev
                    .fission_yields
                    .iter()
                    .map(|set| {
                        let mut y: Vec<(String, f64)> = set.yields.iter().filter(|(p, _)| all.contains(p)).cloned().collect();
                        y.sort_by(|a, b| a.0.cmp(&b.0));
                        FissionYieldSet { energy_ev: set.energy_ev, yields: y }
                    })
                    .collect();
                if restricted.iter().any(|s| !s.yields.is_empty()) {
                    fission_yields = restricted;
                    reactions.push(r.clone());
                } else {
                    fission_yields.clear();
                }
            } else {
                reactions.push(ChainReaction { target: None, ..r.clone() });
            }
        }
        nuclides.push(ChainNuclide {
            name: prev.name.clone(),
            half_life_seconds: prev.half_life_seconds,
            decay_energy_ev: prev.decay_energy_ev,
            decays,
            reactions,
            fission_yield_parent: prev.fission_yield_parent.clone(),
            fission_yields,
            sources: prev.sources.clone(),
        });
    }
    DepletionChainXml::from_nuclides(nuclides)
}

#[cfg(test)]
mod tests;
