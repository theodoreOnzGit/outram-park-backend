// SPDX-License-Identifier: GPL-3.0

//! **Depletion chain XML — read** — GitHub #270 scope row "Depletion chain XML
//! | read | Feeds `outram-mc-libs`' `depletion/chain.rs`".
//!
//! A depletion chain file is the graph of an inventory's evolution: per
//! nuclide, a half-life, its decay branches, its neutron-reaction channels and
//! their targets, and — for the fissionable ones — its fission product yields
//! at one or more incident energies.
//!
//! # Provenance and specification
//!
//! The format is OpenMC's. This reader is a port of
//! `openmc/deplete/nuclide.py`'s `Nuclide.from_xml` and
//! `FissionYieldDistribution.from_xml_element` (MIT), read at
//! `/opt/src/openmc` commit `afa7a14`, and it follows **upstream's own
//! defaults and sentinels** rather than re-deriving them:
//!
//! | Field | Upstream rule (`nuclide.py:229`…`:295`) |
//! |---|---|
//! | `half_life` | absent -> stable; `decay_energy` is read **only** when it is present, default `0.0` |
//! | `<decay>` `target` | absent, or `"nothing"` (case-insensitively), -> no in-chain daughter |
//! | `<decay>` `branching_ratio` | **required** — upstream `float(get_text(...))` with no default, so a missing one is an error here too |
//! | `<reaction>` `Q` | default `0.0` |
//! | `<reaction>` `branching_ratio` | default `1.0` |
//! | `<reaction>` `target` | as for decay, **except** `type="fission"`, where upstream forces `target = None` even if the file names one |
//! | `<neutron_fission_yields parent="X">` | borrows X's yields; X must be in the file and must have yields |
//! | `<source>` with an empty `<parameters>` | skipped |
//!
//! `decay_modes` and `reactions` count attributes are **not** read by upstream.
//! They are read here and checked against the children actually found, because
//! a file whose counts disagree with its content is internally inconsistent and
//! that is worth catching once rather than debugging later. The check is
//! skipped where the attribute is absent, which real files do.
//!
//! # What this does NOT do
//!
//! * It does not build a burnup matrix. Mapping these records onto a solver's
//!   reaction set is the consumer's job — see
//!   `outram-mc-libs::depletion::DepletionChain::from_chain_xml`, which models
//!   three of the ~70 reaction types and **reports** the rest rather than
//!   dropping them silently.
//! * It does not resolve reaction targets against a cross-section library, and
//!   it does not check that a named target is itself in the chain. A target
//!   outside the tracked set is legitimate (it is a sink) and the consumer
//!   decides what that means.
//! * It keeps `type` strings verbatim (`"(n,gamma)"`, `"(n,2n)"`, `"beta-"`, …)
//!   rather than mapping them to an enum. Upstream's `REACTIONS` table has ~70
//!   entries keyed by exactly these strings and grows with the format; an enum
//!   here would refuse a chain file written by a newer OpenMC for no gain,
//!   since this layer never branches on the value.

use std::collections::BTreeMap;
use std::path::Path;

use super::xml_scan::{scan, XmlElement};
use crate::error::NjoyError;

/// A single radioactive-decay branch.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainDecay {
    /// Decay mode as written, e.g. `"beta-"`, `"ec/beta+"`, `"IT"`. Upstream
    /// keeps this as a string and so does this reader; note that real files
    /// contain leading whitespace (`chain_simple.xml` has `type=" beta"`), so
    /// compare it trimmed.
    pub mode: String,
    /// Daughter nuclide, or `None` when the file gives no target or gives
    /// `"nothing"` — i.e. the branch leaves the tracked set.
    pub target: Option<String>,
    /// Fraction of decays following this branch.
    pub branching_ratio: f64,
}

/// A single neutron-reaction channel.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainReaction {
    /// Channel as written, e.g. `"(n,gamma)"`, `"(n,2n)"`, `"fission"`. Keyed
    /// to upstream's `REACTIONS` table; see the module note on why this stays a
    /// string.
    pub kind: String,
    /// Daughter nuclide, or `None` for fission, for `"nothing"`, or when the
    /// file names no target.
    pub target: Option<String>,
    /// Reaction Q value in **eV** (default `0.0`).
    pub q_ev: f64,
    /// Branching ratio (default `1.0`) — used where a channel splits between a
    /// ground-state and a metastable product.
    pub branching_ratio: f64,
}

/// Fission product yields at one incident energy.
#[derive(Debug, Clone, PartialEq)]
pub struct FissionYieldSet {
    /// Incident neutron energy in **eV**.
    pub energy_ev: f64,
    /// `(product, yield)` in the file's own order — atoms of the product per
    /// fission, dimensionless. Order is preserved rather than sorted so a
    /// reader can diff against the `<products>` line it came from.
    pub yields: Vec<(String, f64)>,
}

/// A photon/electron/positron source emitted on decay.
///
/// Read and kept rather than skipped: these carry the decay-photon spectra a
/// shielding or decay-heat calculation needs, and dropping them at parse time
/// would make the chain silently unable to answer for them.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainDecaySource {
    /// `"discrete"` or `"tabular"`.
    pub distribution: String,
    /// `"photon"`, `"electron"`, `"positron"`, …
    pub particle: String,
    /// Interpolation rule, present on a `"tabular"` distribution.
    pub interpolation: Option<String>,
    /// The `<parameters>` list verbatim: upstream's `Univariate` convention is
    /// the abscissae followed by the ordinates, so the first half is energies
    /// in eV and the second half is intensities. Use [`Self::split_xy`] rather
    /// than halving it by hand.
    pub parameters: Vec<f64>,
}

impl ChainDecaySource {
    /// `(energies, intensities)`, or `None` when the parameter list has an odd
    /// length and therefore cannot be an x/y pair.
    pub fn split_xy(&self) -> Option<(&[f64], &[f64])> {
        if self.parameters.len() % 2 != 0 {
            return None;
        }
        Some(self.parameters.split_at(self.parameters.len() / 2))
    }
}

/// One `<nuclide>` record.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainNuclide {
    /// GND name, e.g. `"U235"`, `"Xe135_m1"`.
    pub name: String,
    /// Half-life in **seconds**, `None` when the file gives none (stable).
    pub half_life_seconds: Option<f64>,
    /// Decay energy in **eV**. `None` unless the record carries a half-life,
    /// matching upstream, which reads it only in that branch.
    pub decay_energy_ev: Option<f64>,
    /// Decay branches, in file order.
    pub decays: Vec<ChainDecay>,
    /// Neutron-reaction channels, in file order.
    pub reactions: Vec<ChainReaction>,
    /// Fission yields, one entry per tabulated incident energy, sorted by
    /// energy. Empty for a nuclide with no `<neutron_fission_yields>`.
    pub fission_yields: Vec<FissionYieldSet>,
    /// When the record borrowed its yields via `<neutron_fission_yields
    /// parent="X">`, the name `X`. The borrowed yields are resolved into
    /// `fission_yields` above, so this is provenance, not a pending lookup.
    pub fission_yield_parent: Option<String>,
    /// Decay radiation sources, in file order.
    pub sources: Vec<ChainDecaySource>,
}

impl ChainNuclide {
    /// Decay constant `lambda = ln(2) / T_half` in `1/s`, or `0.0` when stable.
    pub fn decay_constant(&self) -> f64 {
        match self.half_life_seconds {
            Some(t) if t > 0.0 => std::f64::consts::LN_2 / t,
            _ => 0.0,
        }
    }

    /// The yields at `energy_ev`, if the record tabulates that exact energy.
    /// No interpolation: picking a neighbouring energy silently is the kind of
    /// nearest-point substitution this workspace has been bitten by.
    pub fn yields_at(&self, energy_ev: f64) -> Option<&FissionYieldSet> {
        self.fission_yields
            .iter()
            .find(|y| y.energy_ev == energy_ev)
    }
}

/// A parsed depletion chain file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DepletionChainXml {
    /// Nuclides in file order — which is the order a consumer should use for
    /// matrix rows if it wants to reproduce upstream's indexing.
    pub nuclides: Vec<ChainNuclide>,
    /// Name -> index into `nuclides`.
    by_name: BTreeMap<String, usize>,
}

impl DepletionChainXml {
    /// Parse a chain from a string.
    ///
    /// # Errors
    ///
    /// A `<nuclide>` without a `name`; a duplicate name; a `<decay>` without a
    /// `branching_ratio`; an unparseable number; a count attribute that
    /// disagrees with the children found; a `parent=` yield borrow naming a
    /// nuclide that is absent or has no yields; or an element this reader does
    /// not recognise. The last is deliberate — see the module note.
    pub fn parse(xml: &str) -> Result<Self, NjoyError> {
        let els = scan(xml);
        let mut out = DepletionChainXml::default();
        // Where each `<nuclide>` starts, so a `parent=` borrow can be resolved
        // in a second pass regardless of the order the two records appear in.
        let mut pending_borrow: Vec<(usize, String)> = Vec::new();
        // Cursor state: which nuclide, and which `<fission_yields>` set, the
        // elements now being read belong to. The format is unambiguous about
        // this from element names alone, which is what makes a flat scan
        // sufficient (see `xml_scan`).
        let mut cur: Option<usize> = None;
        let mut cur_yield: Option<usize> = None;
        let mut in_yields = false;
        let mut declared: Vec<(Option<usize>, Option<usize>)> = Vec::new();
        // The redundant `<energies>` grid per nuclide, kept to cross-check
        // against the `<fission_yields energy=…>` sets actually found.
        let mut declared_energies: Vec<Option<Vec<f64>>> = Vec::new();

        for (i, el) in els.iter().enumerate() {
            match el.name.as_str() {
                "depletion_chain" => {}
                "nuclide" => {
                    let name = el
                        .attr("name")
                        .ok_or_else(|| err(i, "<nuclide> has no `name`"))?
                        .to_string();
                    let half_life_seconds = opt_f64(el, "half_life", i)?;
                    let decay_energy_ev = match half_life_seconds {
                        Some(_) => Some(opt_f64(el, "decay_energy", i)?.unwrap_or(0.0)),
                        None => None,
                    };
                    let idx = out.nuclides.len();
                    if out.by_name.insert(name.clone(), idx).is_some() {
                        return Err(err(
                            i,
                            &format!(
                                "the chain lists `{name}` more than once. Picking one \
                                 silently would make a burnup matrix depend on file order."
                            ),
                        ));
                    }
                    out.nuclides.push(ChainNuclide {
                        name,
                        half_life_seconds,
                        decay_energy_ev,
                        decays: Vec::new(),
                        reactions: Vec::new(),
                        fission_yields: Vec::new(),
                        fission_yield_parent: None,
                        sources: Vec::new(),
                    });
                    declared.push((
                        opt_usize(el, "decay_modes", i)?,
                        opt_usize(el, "reactions", i)?,
                    ));
                    declared_energies.push(None);
                    cur = Some(idx);
                    cur_yield = None;
                    in_yields = false;
                }
                "decay" => {
                    let n = &mut out.nuclides[need_cur(cur, i, "decay")?];
                    let branching_ratio = opt_f64(el, "branching_ratio", i)?.ok_or_else(|| {
                        err(
                            i,
                            &format!(
                                "<decay> on `{}` has no `branching_ratio`. Upstream reads \
                                 this with no default, so defaulting it here would invent \
                                 a decay rate the file does not state.",
                                n.name
                            ),
                        )
                    })?;
                    n.decays.push(ChainDecay {
                        mode: el.attr("type").unwrap_or("").to_string(),
                        target: target_of(el),
                        branching_ratio,
                    });
                }
                "reaction" => {
                    let n = &mut out.nuclides[need_cur(cur, i, "reaction")?];
                    let kind = el.attr("type").unwrap_or("").to_string();
                    // Upstream forces `target = None` for fission even when the
                    // file names one: products come from the yields.
                    let target = if kind == "fission" { None } else { target_of(el) };
                    n.reactions.push(ChainReaction {
                        kind,
                        target,
                        q_ev: opt_f64(el, "Q", i)?.unwrap_or(0.0),
                        branching_ratio: opt_f64(el, "branching_ratio", i)?.unwrap_or(1.0),
                    });
                }
                "source" => {
                    let n = &mut out.nuclides[need_cur(cur, i, "source")?];
                    n.sources.push(ChainDecaySource {
                        distribution: el.attr("type").unwrap_or("").to_string(),
                        particle: el.attr("particle").unwrap_or("").to_string(),
                        interpolation: el.attr("interpolation").map(str::to_string),
                        parameters: Vec::new(),
                    });
                }
                "parameters" => {
                    let n = &mut out.nuclides[need_cur(cur, i, "parameters")?];
                    let values = floats(&el.text, i)?;
                    match n.sources.last_mut() {
                        // Upstream skips a source whose parameters are empty.
                        Some(_) if values.is_empty() => {
                            n.sources.pop();
                        }
                        Some(s) => s.parameters = values,
                        None => {
                            return Err(err(i, "<parameters> outside a <source>"));
                        }
                    }
                }
                "neutron_fission_yields" => {
                    let idx = need_cur(cur, i, "neutron_fission_yields")?;
                    if let Some(parent) = el.attr("parent") {
                        out.nuclides[idx].fission_yield_parent = Some(parent.to_string());
                        pending_borrow.push((idx, parent.to_string()));
                    }
                    in_yields = true;
                    cur_yield = None;
                }
                "energies" => {
                    // The energy grid is repeated on each `<fission_yields
                    // energy=…>`, so this element is redundant. It is validated
                    // rather than ignored: a grid that disagrees with the sets
                    // means the file was assembled wrongly.
                    let idx = need_cur(cur, i, "energies")?;
                    if !in_yields {
                        return Err(err(i, "<energies> outside <neutron_fission_yields>"));
                    }
                    declared_energies[idx] = Some(floats(&el.text, i)?);
                }
                "fission_yields" => {
                    if !in_yields {
                        return Err(err(i, "<fission_yields> outside <neutron_fission_yields>"));
                    }
                    let idx = need_cur(cur, i, "fission_yields")?;
                    let energy_ev = opt_f64(el, "energy", i)?
                        .ok_or_else(|| err(i, "<fission_yields> has no `energy`"))?;
                    let n = &mut out.nuclides[idx];
                    cur_yield = Some(n.fission_yields.len());
                    n.fission_yields.push(FissionYieldSet {
                        energy_ev,
                        yields: Vec::new(),
                    });
                }
                "products" | "data" => {
                    let idx = need_cur(cur, i, &el.name)?;
                    let k = cur_yield
                        .ok_or_else(|| err(i, "<products>/<data> outside <fission_yields>"))?;
                    let set = &mut out.nuclides[idx].fission_yields[k];
                    if el.name == "products" {
                        if !set.yields.is_empty() {
                            return Err(err(i, "<products> appears twice in one <fission_yields>"));
                        }
                        for p in el.text.split_whitespace() {
                            set.yields.push((p.to_string(), f64::NAN));
                        }
                    } else {
                        let values = floats(&el.text, i)?;
                        if values.len() != set.yields.len() {
                            return Err(err(
                                i,
                                &format!(
                                    "<fission_yields> at {} eV lists {} products but {} \
                                     data values. Zipping the shorter of the two — which \
                                     is what a permissive reader does — would drop yields \
                                     without saying so.",
                                    set.energy_ev,
                                    set.yields.len(),
                                    values.len()
                                ),
                            ));
                        }
                        for (slot, v) in set.yields.iter_mut().zip(values) {
                            slot.1 = v;
                        }
                    }
                }
                other => {
                    return Err(err(
                        i,
                        &format!(
                            "unrecognised element `{other}`. Refusing rather than skipping: \
                             an ignored element would make this chain silently incomplete."
                        ),
                    ))
                }
            }
        }

        out.check_counts(&declared)?;
        for n in &mut out.nuclides {
            n.fission_yields
                .sort_by(|a, b| a.energy_ev.total_cmp(&b.energy_ev));
        }
        out.check_energy_grids(&declared_energies)?;
        out.check_yields_are_complete()?;
        out.resolve_borrows(&pending_borrow)?;
        Ok(out)
    }

    /// Parse a chain from a file.
    pub fn read_file(path: &Path) -> Result<Self, NjoyError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| NjoyError::Hdf5(format!("reading {}: {e}", path.display())))?;
        Self::parse(&text)
    }

    /// Check the redundant `decay_modes` / `reactions` count attributes against
    /// the children actually parsed, where the file gives them.
    fn check_counts(&self, declared: &[(Option<usize>, Option<usize>)]) -> Result<(), NjoyError> {
        for (n, (decays, reactions)) in self.nuclides.iter().zip(declared) {
            if let Some(d) = decays {
                if *d != n.decays.len() {
                    return Err(NjoyError::Hdf5(format!(
                        "depletion chain: `{}` declares decay_modes=\"{d}\" but carries {} \
                         <decay> elements. The file is internally inconsistent.",
                        n.name,
                        n.decays.len()
                    )));
                }
            }
            if let Some(r) = reactions {
                if *r != n.reactions.len() {
                    return Err(NjoyError::Hdf5(format!(
                        "depletion chain: `{}` declares reactions=\"{r}\" but carries {} \
                         <reaction> elements. The file is internally inconsistent.",
                        n.name,
                        n.reactions.len()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Cross-check each nuclide's redundant `<energies>` grid against the
    /// energies its `<fission_yields>` sets actually carry. Skipped where the
    /// nuclide borrows its yields, since upstream replaces the whole element in
    /// that case and any local grid is dead text.
    ///
    /// # Why this compares to a tolerance rather than exactly
    ///
    /// `FissionYieldDistribution.to_xml_element` writes **only** the
    /// `<fission_yields energy=...>` attribute — it does not write an
    /// `<energies>` element at all. So the grid appears only in hand-written or
    /// legacy files (`chain_simple.xml` is one), where the same physical energy
    /// may be spelled two ways in the two places (`0.0253` and `2.53000e-02`
    /// happen to parse identically, but nothing guarantees that in general).
    /// Comparing exactly would refuse a legitimate file over a decimal spelling.
    /// `1e-9` relative is tight enough that any genuine disagreement — a missing
    /// set, an extra one, a different energy — still fails.
    fn check_energy_grids(&self, declared: &[Option<Vec<f64>>]) -> Result<(), NjoyError> {
        const REL_TOL: f64 = 1e-9;
        for (n, grid) in self.nuclides.iter().zip(declared) {
            let Some(grid) = grid else { continue };
            if n.fission_yield_parent.is_some() {
                continue;
            }
            let mut want = grid.clone();
            want.sort_by(f64::total_cmp);
            let have: Vec<f64> = n.fission_yields.iter().map(|y| y.energy_ev).collect();
            let agrees = want.len() == have.len()
                && want.iter().zip(&have).all(|(a, b)| {
                    let scale = a.abs().max(b.abs()).max(f64::MIN_POSITIVE);
                    (a - b).abs() / scale <= REL_TOL
                });
            if !agrees {
                return Err(NjoyError::Hdf5(format!(
                    "depletion chain: `{}` declares <energies> {want:?} eV but carries \
                     <fission_yields> at {have:?} eV (compared to {REL_TOL:e} relative). \
                     The file is internally inconsistent.",
                    n.name
                )));
            }
        }
        Ok(())
    }

    /// Every product listed in a `<products>` line must have received a value
    /// from the matching `<data>` line. A `<products>` with no `<data>` at all
    /// would otherwise leave `NaN` yields that only surface later, inside a
    /// burnup matrix, as a `NaN` inventory.
    fn check_yields_are_complete(&self) -> Result<(), NjoyError> {
        for n in &self.nuclides {
            for y in &n.fission_yields {
                if let Some((p, _)) = y.yields.iter().find(|(_, v)| !v.is_finite()) {
                    return Err(NjoyError::Hdf5(format!(
                        "depletion chain: `{}` lists product `{p}` at {} eV with no finite \
                         yield — the <fission_yields> element has <products> but no <data>.",
                        n.name, y.energy_ev
                    )));
                }
            }
        }
        Ok(())
    }

    /// Copy borrowed yields in, now that every nuclide has been seen — the
    /// parent may appear after the borrower.
    fn resolve_borrows(&mut self, pending: &[(usize, String)]) -> Result<(), NjoyError> {
        for (idx, parent) in pending {
            let p = *self.by_name.get(parent).ok_or_else(|| {
                NjoyError::Hdf5(format!(
                    "depletion chain: `{}` borrows fission yields from `{parent}`, which is \
                     not in the file.",
                    self.nuclides[*idx].name
                ))
            })?;
            let borrowed = self.nuclides[p].fission_yields.clone();
            if borrowed.is_empty() {
                return Err(NjoyError::Hdf5(format!(
                    "depletion chain: `{}` borrows fission yields from `{parent}`, which has \
                     none.",
                    self.nuclides[*idx].name
                )));
            }
            self.nuclides[*idx].fission_yields = borrowed;
        }
        Ok(())
    }

    /// A nuclide by name.
    pub fn get(&self, name: &str) -> Option<&ChainNuclide> {
        self.nuclides.get(*self.by_name.get(name)?)
    }

    /// Every name in the chain, in file order.
    pub fn names(&self) -> Vec<&str> {
        self.nuclides.iter().map(|n| n.name.as_str()).collect()
    }

    /// The nuclides carrying a `"fission"` reaction channel.
    pub fn fissionable(&self) -> Vec<&ChainNuclide> {
        self.nuclides
            .iter()
            .filter(|n| n.reactions.iter().any(|r| r.kind == "fission"))
            .collect()
    }

    /// Every distinct reaction `type` string in the file, sorted. Use this to
    /// see what a consumer would have to model — or report unmodelled.
    pub fn reaction_kinds(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .nuclides
            .iter()
            .flat_map(|n| n.reactions.iter().map(|r| r.kind.as_str()))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

fn err(i: usize, msg: &str) -> NjoyError {
    NjoyError::Hdf5(format!("depletion chain element {i}: {msg}"))
}

fn need_cur(cur: Option<usize>, i: usize, what: &str) -> Result<usize, NjoyError> {
    cur.ok_or_else(|| err(i, &format!("<{what}> outside a <nuclide>")))
}

/// `target`, with upstream's `"nothing"` sentinel (case-insensitive) and a
/// missing attribute both mapping to `None`.
fn target_of(el: &XmlElement) -> Option<String> {
    let t = el.attr("target")?.trim();
    if t.eq_ignore_ascii_case("nothing") {
        None
    } else {
        Some(t.to_string())
    }
}

fn opt_f64(el: &XmlElement, key: &str, i: usize) -> Result<Option<f64>, NjoyError> {
    match el.attr(key) {
        None => Ok(None),
        Some(s) => s
            .trim()
            .parse::<f64>()
            .map(Some)
            .map_err(|e| err(i, &format!("`{key}=\"{s}\"` is not a number: {e}"))),
    }
}

fn opt_usize(el: &XmlElement, key: &str, i: usize) -> Result<Option<usize>, NjoyError> {
    match el.attr(key) {
        None => Ok(None),
        Some(s) => s
            .trim()
            .parse::<usize>()
            .map(Some)
            .map_err(|e| err(i, &format!("`{key}=\"{s}\"` is not a count: {e}"))),
    }
}

fn floats(text: &str, i: usize) -> Result<Vec<f64>, NjoyError> {
    text.split_whitespace()
        .map(|t| {
            t.parse::<f64>()
                .map_err(|e| err(i, &format!("`{t}` is not a number: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OpenMC's `examples/pincell_depletion/chain_simple.xml`, verbatim
    /// (`/opt/src/openmc` commit `afa7a14`, MIT). Embedded rather than read from
    /// disk so these tests run on a machine with no OpenMC checkout; the
    /// file-backed comparison is a separate, skipping test below.
    ///
    /// Note the two deliberate roughnesses in the real file, both of which this
    /// reader has to survive: `type=" beta"` on Xe-135 carries a leading space,
    /// and Gd-157's `<nuclide>` tag has stray whitespace before its `>`.
    const CHAIN_SIMPLE: &str = r#"<?xml version="1.0"?>
<depletion_chain>
  <nuclide name="I135" decay_modes="1" reactions="1" half_life="2.36520E+04">
    <decay type="beta" target="Xe135" branching_ratio="1.0" />
    <reaction type="(n,gamma)" Q="0.0" target="Xe136" /> <!-- Not precisely true, but whatever -->
  </nuclide>
  <nuclide name="Xe135" decay_modes="1" reactions="1" half_life="3.29040E+04">
    <decay type=" beta" target="Cs135" branching_ratio="1.0" />
    <reaction type="(n,gamma)" Q="0.0" target="Xe136" />
  </nuclide>
  <nuclide name="Xe136" decay_modes="0" reactions="0" />
  <nuclide name="Cs135" decay_modes="0" reactions="0" />
  <nuclide name="Gd157" decay_modes="0" reactions="1"  >
    <reaction type="(n,gamma)" Q="0.0" target="Nothing" />
  </nuclide>
  <nuclide name="Gd156" decay_modes="0" reactions="1">
    <reaction type="(n,gamma)" Q="0.0" target="Gd157" />
  </nuclide>
  <nuclide name="U234" decay_modes="0" reactions="1">
    <reaction type="fission" Q="191840000."/>
    <neutron_fission_yields>
      <energies>2.53000e-02</energies>
      <fission_yields energy="2.53000e-02">
        <products>Gd157 Gd156 I135 Xe135 Xe136 Cs135</products>
        <data>1.093250e-04 2.087260e-04 2.780820e-02 6.759540e-03 2.392300e-02 4.356330e-05</data>
      </fission_yields>
    </neutron_fission_yields>
  </nuclide>
  <nuclide name="U235" decay_modes="0" reactions="1">
    <reaction type="fission" Q="193410000."/>
    <neutron_fission_yields>
      <energies>2.53000e-02</energies>
      <fission_yields energy="2.53000e-02">
        <products>Gd157 Gd156 I135 Xe135 Xe136 Cs135</products>
        <data>6.142710e-5 1.483250e-04 0.0292737 0.002566345 0.0219242 4.9097e-6</data>
      </fission_yields>
    </neutron_fission_yields>
  </nuclide>
  <nuclide name="U238" decay_modes="0" reactions="1">
    <reaction type="fission" Q="197790000."/>
    <neutron_fission_yields>
      <energies>2.53000e-02</energies>
      <fission_yields energy="2.53000e-02">
        <products>Gd157 Gd156 I135 Xe135 Xe136 Cs135</products>
        <data>4.141120e-04 7.605360e-04 0.0135457 0.00026864 0.0024432 3.7100E-07</data>
      </fission_yields>
    </neutron_fission_yields>
  </nuclide>
</depletion_chain>"#;

    /// Every field of the 9-nuclide reference chain, against the file's own
    /// text. This is the chain `outram-mc-libs` had transcribed by hand; the
    /// cross-check that the transcription and this reader agree lives in that
    /// crate, where both are visible.
    #[test]
    fn the_reference_chain_reads_back_field_for_field() {
        let c = DepletionChainXml::parse(CHAIN_SIMPLE).unwrap();
        assert_eq!(
            c.names(),
            ["I135", "Xe135", "Xe136", "Cs135", "Gd157", "Gd156", "U234", "U235", "U238"]
        );

        let i135 = c.get("I135").unwrap();
        assert_eq!(i135.half_life_seconds, Some(2.36520e4));
        assert_eq!(i135.decay_energy_ev, Some(0.0), "absent decay_energy is 0");
        assert_eq!(i135.decays.len(), 1);
        assert_eq!(i135.decays[0].mode, "beta");
        assert_eq!(i135.decays[0].target.as_deref(), Some("Xe135"));
        assert_eq!(i135.decays[0].branching_ratio, 1.0);
        assert_eq!(i135.reactions.len(), 1);
        assert_eq!(i135.reactions[0].kind, "(n,gamma)");
        assert_eq!(i135.reactions[0].target.as_deref(), Some("Xe136"));
        assert_eq!(i135.reactions[0].branching_ratio, 1.0, "default");
        // ln(2) / 23652 s, to the last bit of an f64.
        assert!(
            (i135.decay_constant() - 2.930_607_054_625_170_2e-5).abs() < 1e-20,
            "{}",
            i135.decay_constant()
        );

        // A leading space in `type=" beta"` survives verbatim — compare trimmed.
        assert_eq!(c.get("Xe135").unwrap().decays[0].mode, " beta");

        // Stable, inert nuclides: no half-life, hence no decay energy.
        let xe136 = c.get("Xe136").unwrap();
        assert_eq!(xe136.half_life_seconds, None);
        assert_eq!(xe136.decay_energy_ev, None);
        assert_eq!(xe136.decay_constant(), 0.0);
        assert!(xe136.decays.is_empty() && xe136.reactions.is_empty());

        // `target="Nothing"` is the out-of-chain sink, not a nuclide named
        // "Nothing" — this is the sentinel upstream applies case-insensitively.
        let gd157 = c.get("Gd157").unwrap();
        assert_eq!(gd157.reactions[0].kind, "(n,gamma)");
        assert_eq!(gd157.reactions[0].target, None);

        // Fission: target forced to None, Q read, yields in file order.
        let u235 = c.get("U235").unwrap();
        assert_eq!(u235.reactions[0].kind, "fission");
        assert_eq!(u235.reactions[0].target, None);
        assert_eq!(u235.reactions[0].q_ev, 193_410_000.0);
        let y = u235.yields_at(2.53000e-2).unwrap();
        assert_eq!(
            y.yields,
            vec![
                ("Gd157".to_string(), 6.142710e-5),
                ("Gd156".to_string(), 1.483250e-04),
                ("I135".to_string(), 0.0292737),
                ("Xe135".to_string(), 0.002566345),
                ("Xe136".to_string(), 0.0219242),
                ("Cs135".to_string(), 4.9097e-6),
            ]
        );
        assert!(u235.yields_at(2.53e-2 * 1.000_001).is_none(), "no interpolation");

        assert_eq!(
            c.fissionable().iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            ["U234", "U235", "U238"]
        );
        assert_eq!(c.reaction_kinds(), ["(n,gamma)", "fission"]);
    }

    /// The three defaults and the one *non*-default upstream specifies. A
    /// missing `branching_ratio` on a `<decay>` is an error, because upstream
    /// reads it with no default and inventing `1.0` would state a decay rate the
    /// file does not.
    #[test]
    fn upstreams_attribute_defaults_are_followed_including_the_absent_one() {
        let c = DepletionChainXml::parse(
            r#"<depletion_chain>
                 <nuclide name="Fe57"><reaction type="(n,2n)"/></nuclide>
               </depletion_chain>"#,
        )
        .unwrap();
        let r = &c.get("Fe57").unwrap().reactions[0];
        assert_eq!(r.q_ev, 0.0, "Q defaults to 0");
        assert_eq!(r.branching_ratio, 1.0, "branching_ratio defaults to 1");
        assert_eq!(r.target, None, "a reaction may name no target");

        let err = DepletionChainXml::parse(
            r#"<depletion_chain>
                 <nuclide name="Fe59" half_life="3844368.0">
                   <decay type="beta-" target="Co59"/>
                 </nuclide>
               </depletion_chain>"#,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("no `branching_ratio`"), "{err}");
    }

    /// A `<decay>` with no target at all — `ec/beta+` in `chain_ni.xml` — and a
    /// `<source>` whose parameters are read rather than dropped.
    #[test]
    fn a_targetless_decay_and_a_decay_source_are_both_kept() {
        let c = DepletionChainXml::parse(
            r#"<depletion_chain>
  <nuclide name="Fe55" half_life="86594050.0" decay_modes="1" decay_energy="5842.102" reactions="0">
    <decay type="ec/beta+" branching_ratio="1.0"/>
    <source type="discrete" particle="photon">
      <parameters>557.6 640.5 4.28e-11 8.11e-12</parameters>
    </source>
    <source type="tabular" interpolation="histogram" particle="electron">
      <parameters></parameters>
    </source>
  </nuclide>
</depletion_chain>"#,
        )
        .unwrap();
        let n = c.get("Fe55").unwrap();
        assert_eq!(n.decay_energy_ev, Some(5842.102));
        assert_eq!(n.decays[0].target, None, "ec/beta+ names no target");
        // The empty-parameter source is dropped, as upstream drops it; the other
        // is kept with its spectrum, not merely noted.
        assert_eq!(n.sources.len(), 1);
        assert_eq!(n.sources[0].distribution, "discrete");
        assert_eq!(n.sources[0].particle, "photon");
        let (e, p) = n.sources[0].split_xy().unwrap();
        assert_eq!(e, [557.6, 640.5]);
        assert_eq!(p, [4.28e-11, 8.11e-12]);
    }

    /// `<neutron_fission_yields parent="X">` borrows X's yields, and resolves
    /// even when X appears *after* the borrower in the file.
    #[test]
    fn borrowed_fission_yields_resolve_in_either_file_order() {
        let c = DepletionChainXml::parse(
            r#"<depletion_chain>
  <nuclide name="Pu241" reactions="1">
    <reaction type="fission" Q="2.1e8"/>
    <neutron_fission_yields parent="Pu239"/>
  </nuclide>
  <nuclide name="Pu239" reactions="1">
    <reaction type="fission" Q="2.0e8"/>
    <neutron_fission_yields>
      <energies>0.0253 5.0e5</energies>
      <fission_yields energy="5.0e5"><products>Xe135</products><data>0.008</data></fission_yields>
      <fission_yields energy="0.0253"><products>Xe135</products><data>0.007</data></fission_yields>
    </neutron_fission_yields>
  </nuclide>
</depletion_chain>"#,
        )
        .unwrap();
        let pu241 = c.get("Pu241").unwrap();
        assert_eq!(pu241.fission_yield_parent.as_deref(), Some("Pu239"));
        // Sorted by energy regardless of the order the file listed them in.
        let es: Vec<f64> = pu241.fission_yields.iter().map(|y| y.energy_ev).collect();
        assert_eq!(es, [0.0253, 5.0e5]);
        assert_eq!(pu241.yields_at(5.0e5).unwrap().yields[0].1, 0.008);

        let err = DepletionChainXml::parse(
            r#"<depletion_chain><nuclide name="Pu241">
                 <neutron_fission_yields parent="Cm244"/></nuclide></depletion_chain>"#,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("not in the file"), "{err}");
    }

    /// The five ways a chain file can be wrong that this reader refuses rather
    /// than papering over. Each would otherwise surface as a `NaN` inventory or
    /// a quietly missing transmutation path, far from its cause.
    #[test]
    fn an_inconsistent_chain_is_refused_rather_than_repaired() {
        let cases: [(&str, &str); 5] = [
            (
                "a count attribute that disagrees with the children",
                r#"<depletion_chain><nuclide name="I135" reactions="2">
                     <reaction type="(n,gamma)" target="Xe136"/></nuclide></depletion_chain>"#,
            ),
            (
                "products and data of different length",
                r#"<depletion_chain><nuclide name="U235"><neutron_fission_yields>
                     <fission_yields energy="0.0253"><products>Xe135 I135</products>
                     <data>0.007</data></fission_yields>
                   </neutron_fission_yields></nuclide></depletion_chain>"#,
            ),
            (
                "products with no data at all",
                r#"<depletion_chain><nuclide name="U235"><neutron_fission_yields>
                     <fission_yields energy="0.0253"><products>Xe135</products>
                     </fission_yields></neutron_fission_yields></nuclide></depletion_chain>"#,
            ),
            (
                "an <energies> grid that disagrees with the yield sets",
                r#"<depletion_chain><nuclide name="U235"><neutron_fission_yields>
                     <energies>0.0253 5.0e5</energies>
                     <fission_yields energy="0.0253"><products>Xe135</products>
                     <data>0.007</data></fission_yields>
                   </neutron_fission_yields></nuclide></depletion_chain>"#,
            ),
            (
                "the same nuclide twice",
                r#"<depletion_chain><nuclide name="U235"/><nuclide name="U235"/></depletion_chain>"#,
            ),
        ];
        for (what, xml) in cases {
            assert!(
                DepletionChainXml::parse(xml).is_err(),
                "should have been refused: {what}"
            );
        }

        // And an element the reader does not know is refused, not skipped.
        let err = DepletionChainXml::parse(
            r#"<depletion_chain><nuclide name="U235"><brand_new_thing x="1"/></nuclide></depletion_chain>"#,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("unrecognised element"), "{err}");
    }

    /// Read the real files from an OpenMC checkout when one is present, so the
    /// embedded copy above cannot drift from the format's own examples without
    /// this failing. **Skips** — it does not fail — where no checkout exists,
    /// per the workspace rule that reference data may be absent.
    #[test]
    fn the_openmc_checkouts_own_chain_files_read_if_present() {
        let files = [
            "/opt/src/openmc/examples/pincell_depletion/chain_simple.xml",
            "/opt/src/openmc/tests/chain_simple_decay.xml",
            "/opt/src/openmc/tests/chain_ni.xml",
        ];
        let mut seen = 0usize;
        for f in files {
            let path = Path::new(f);
            if !path.is_file() {
                println!("SKIP {f}: not present");
                continue;
            }
            let c = DepletionChainXml::read_file(path).unwrap_or_else(|e| panic!("{f}: {e}"));
            assert!(!c.nuclides.is_empty(), "{f} parsed to an empty chain");
            println!(
                "{f}: {} nuclides, reaction kinds {:?}",
                c.nuclides.len(),
                c.reaction_kinds()
            );
            seen += 1;
        }
        if seen == 0 {
            println!("SKIP: no OpenMC checkout — the embedded chain still ran");
        } else {
            // The example chain, when present, must match the embedded copy
            // byte-for-byte in what it parses to.
            let p = Path::new(files[0]);
            if p.is_file() {
                assert_eq!(
                    DepletionChainXml::read_file(p).unwrap(),
                    DepletionChainXml::parse(CHAIN_SIMPLE).unwrap(),
                    "the embedded chain_simple.xml has drifted from the checkout's"
                );
            }
        }
    }
}
