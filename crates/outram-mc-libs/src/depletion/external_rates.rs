// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (MIT License, Copyright (c) 2011-2026 Massachusetts
// Institute of Technology, UChicago Argonne LLC, and OpenMC contributors):
//   Upstream: https://github.com/openmc-dev/openmc
//   Upstream commit: e4febe929b210865af51d6bcf9beded7956af8ac (2026-11-16)
//   Upstream files, read line for line:
//     openmc/deplete/transfer_rates.py  (ExternalRates, TransferRates,
//                                        ExternalSourceRates)
//     openmc/deplete/chain.py           (Chain.form_rr_term,
//                                        Chain.add_redox_term,
//                                        Chain.form_ext_source_term)
//     openmc/deplete/pool.py            (deplete, _add_external_source)
//     openmc/deplete/results.py         (_SECONDS_PER_* constants)
//
// This is an OUTRAM PARK translation for research, education and V&V. It is
// not the official OpenMC software and not for reactor operation.

//! **Continuous removal, feed, material-to-material transfer and redox
//! control during depletion** (gh:#1622): OpenMC's `TransferRates` and
//! `ExternalSourceRates`, and the way `openmc.deplete.pool.deplete` folds
//! them into the burnup system.
//!
//! # Why
//!
//! Molten salt reactors process their salt while they run: fission products
//! are stripped, fresh fuel is fed, and in a thorium breeder Pa-233 is pulled
//! out before it captures a neutron. OpenMC's docstring: "MSRs inspired the
//! development of this class". Pebble recirculation is the same mathematics.
//!
//! # What upstream does, and this port does
//!
//! | Term | Upstream | Here |
//! |---|---|---|
//! | Removal (or growth) at a rate per atom | `form_rr_term`: a **diagonal** matrix of the summed rates, **subtracted** from each material's Bateman matrix. A component matches a chain nuclide by its **element** first, then by its own name. | [`form_rr_term`], [`deplete_with_external_rates`] |
//! | Transfer to another material | Every material is solved **together** as one block matrix: Bateman matrices on the diagonal, and for each `(destination, source)` pair the diagonal transfer term in the destination's row and the source's column. | [`deplete_with_external_rates`] |
//! | Constant external source | **Exact**, by augmenting the system: the source vector becomes an extra column, a zero row is added, and the nuclide vector gets a trailing `1.0`; one matrix exponential then solves the inhomogeneous problem. | [`form_ext_source_term`], [`deplete_with_external_rates`] |
//! | Redox control | Buffer nuclides absorb the net change in oxidation state: `row_buffer -= (Σ_j a_ji OS_j) · fraction / OS_buffer`. | [`add_redox_term`] |
//!
//! Units follow upstream: rates are per second after conversion from the
//! caller's unit, and a source is in **atoms per second** to match a nuclide
//! vector in **atoms**. A caller whose vector is a density must convert the
//! source the same way (divide by volume in barn-cm), as upstream's callers
//! do with its atom vectors.
//!
//! # Relation to the older single-material functions
//!
//! [`super::integrators::TransferRate`], [`super::integrators::apply_transfer_rates`]
//! and [`super::integrators::apply_feed`] stay as they are. The last applies
//! a feed **after** the step, first order in `dt`, and says so. This module
//! is the upstream-faithful replacement, added beside them (extension, not
//! modification); the analytic test
//! `exact_feed_matches_the_analytic_solution_and_apply_feed_is_first_order`
//! measures the old path's error.
//!
//! # Where this port departs from upstream, and why
//!
//! 1. **No MPI and no process pool.** Upstream gathers onto rank 0 for the
//!    block solve and maps single-material solves over a `multiprocessing`
//!    pool. Here everything runs in the caller's thread, in material order.
//!    The arithmetic per material is the same.
//! 2. **Stripping the augmented entry.** Upstream's
//!    `ExternalSourceRates.reformat_nuclide_vectors` removes the trailing
//!    entry from a material's result whenever that material has **any**
//!    external source at **any** timestep. But `_add_external_source`
//!    augments a material only if it has a source at **this** timestep
//!    (`form_ext_source_term` returns `None` otherwise). A material with a
//!    source at another timestep, depleted in a step where another
//!    material's source is active, is therefore not augmented yet loses its
//!    last nuclide. This port strips exactly the materials it augmented. The
//!    case is pinned by
//!    `a_material_without_a_source_this_step_keeps_every_nuclide`, and is a
//!    finding to report upstream; it was read from the source, not
//!    reproduced by running OpenMC.
//! 3. **Redox with a zero-oxidation-state buffer is refused.** Upstream
//!    divides by the buffer's oxidation state, so a buffer of oxidation state
//!    0 fills the row with `inf`/`nan` (numpy warns, the solve continues).
//!    Here it is an error.
//! 4. **Summation order in [`add_redox_term`].** Upstream's
//!    `sum(array[:, i]*os)` uses numpy's pairwise summation; this port sums
//!    rows in order. Results can differ in the last bits for chains of more
//!    than a few nuclides. ~~Not yet compared against OpenMC by running it.~~
//!    **CORRECTED 2026-11-17:** compared, redox included, against OpenMC
//!    `e4febe9` run on the same inputs (`tests/external_rates_vs_openmc_run.rs`,
//!    2.8e-15) and its transfer-rate regression files
//!    (`tests/transfer_rates_vs_openmc_regression.rs`, 2.0e-15); no difference
//!    from the summation order was resolved.
//! 5. **Material identifiers are strings.** Upstream also accepts an
//!    `openmc.Material` or its name; here the caller passes the burnable
//!    material's id, as upstream's `_get_material_id` returns it.
//!
//! # Verification status (2026-11-17)
//!
//! **Analytic only** (the tests in this file): removal, feed with removal,
//! two-material transfer and its conservation, element matching, unit
//! conversion, redox on a hand-worked matrix. **Not yet compared against
//! OpenMC by running it**: this session could not run OpenMC's Python
//! package (gh:#1622, gh:#1626 track the code-to-code arm).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::depletion::matrix::DepletionMatrix;
use crate::model::isotope::Element;
use crate::model::masses::AVOGADRO_PER_MOL;

/// Seconds per minute (upstream `results.py`, `_SECONDS_PER_MINUTE`).
pub const SECONDS_PER_MINUTE: f64 = 60.0;
/// Seconds per hour (upstream `_SECONDS_PER_HOUR = 60*60`).
pub const SECONDS_PER_HOUR: f64 = 60.0 * 60.0;
/// Seconds per day (upstream `_SECONDS_PER_DAY = 24*60*60`).
pub const SECONDS_PER_DAY: f64 = 24.0 * 60.0 * 60.0;
/// Seconds per Julian year (upstream `_SECONDS_PER_JULIAN_YEAR =
/// 365.25*24*60*60`, "365.25 due to the leap year").
pub const SECONDS_PER_JULIAN_YEAR: f64 = 365.25 * 24.0 * 60.0 * 60.0;

/// The unit of a transfer rate (upstream `transfer_rate_units`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateUnit {
    /// `'1/s'`
    PerSecond,
    /// `'1/min'`
    PerMinute,
    /// `'1/h'`
    PerHour,
    /// `'1/d'`
    PerDay,
    /// `'1/a'` (Julian year)
    PerJulianYear,
}

/// The unit of an external source mass rate (upstream `rate_units`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassRateUnit {
    /// `'g/s'`
    GramPerSecond,
    /// `'g/min'`
    GramPerMinute,
    /// `'g/h'`
    GramPerHour,
    /// `'g/d'`
    GramPerDay,
    /// `'g/a'` (Julian year)
    GramPerJulianYear,
}

/// Upstream's `unit_conv`: the rate is divided by this.
fn seconds_in(unit: RateUnit) -> f64 {
    match unit {
        RateUnit::PerSecond => 1.0,
        RateUnit::PerMinute => SECONDS_PER_MINUTE,
        RateUnit::PerHour => SECONDS_PER_HOUR,
        RateUnit::PerDay => SECONDS_PER_DAY,
        RateUnit::PerJulianYear => SECONDS_PER_JULIAN_YEAR,
    }
}

fn seconds_in_mass(unit: MassRateUnit) -> f64 {
    match unit {
        MassRateUnit::GramPerSecond => 1.0,
        MassRateUnit::GramPerMinute => SECONDS_PER_MINUTE,
        MassRateUnit::GramPerHour => SECONDS_PER_HOUR,
        MassRateUnit::GramPerDay => SECONDS_PER_DAY,
        MassRateUnit::GramPerJulianYear => SECONDS_PER_JULIAN_YEAR,
    }
}

/// Upstream's `re.split(r'\d+', name)[0]`: the leading run of non-digits,
/// which is the element symbol of a GNDS nuclide name (`"Am242_m1"` → `"Am"`)
/// and the whole string for an element (`"Pa"` → `"Pa"`).
pub fn element_of(name: &str) -> &str {
    match name.find(|c: char| c.is_ascii_digit()) {
        Some(i) => &name[..i],
        None => name,
    }
}

/// Upstream's test `len(re.split(r'\d+', component)) == 1`: a component with
/// no digit names an element, one with a digit names a nuclide.
fn names_an_element(component: &str) -> bool {
    !component.chars().any(|c| c.is_ascii_digit())
}

/// Upstream's `element not in ELEMENT_SYMBOL.values()` check.
fn is_element_symbol(symbol: &str) -> bool {
    (1..=118u8).any(|z| Element::from_z(z).is_some_and(|e| e.symbol() == symbol))
}

fn element_from_symbol(symbol: &str) -> Option<Element> {
    (1..=118u8).filter_map(Element::from_z).find(|e| e.symbol() == symbol)
}

/// One `(timesteps, rate, destination)` entry of upstream's
/// `external_rates[material][component]` list.
#[derive(Debug, Clone, PartialEq)]
struct RateEntry {
    timesteps: Vec<usize>,
    rate: f64,
    destination: Option<String>,
}

/// Upstream's `ExternalRates` base: rates per material and component, kept
/// in insertion order (upstream's `defaultdict(list)` preserves it).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalRates {
    burnable_mats: Vec<String>,
    number_of_timesteps: usize,
    /// Per burnable material (same order as `burnable_mats`): its components
    /// in insertion order, each with its entries.
    external_rates: Vec<Vec<(String, Vec<RateEntry>)>>,
    /// Sorted, unique (upstream `np.unique`).
    external_timesteps: Vec<usize>,
}

impl ExternalRates {
    fn new(burnable_mats: Vec<String>, number_of_timesteps: usize) -> Self {
        let n = burnable_mats.len();
        Self { burnable_mats, number_of_timesteps, external_rates: vec![Vec::new(); n], external_timesteps: Vec::new() }
    }

    /// Upstream `_get_material_id` for a string id: it must be burnable.
    fn material_index(&self, material: &str) -> Result<usize, String> {
        self.burnable_mats
            .iter()
            .position(|m| m == material)
            .ok_or_else(|| format!("material {material} is not a burnable material {:?}", self.burnable_mats))
    }

    /// Upstream's timestep handling: each given index is checked against
    /// `range(number_of_timesteps)`; `None` means every timestep.
    fn timesteps(&self, timesteps: Option<&[usize]>) -> Result<Vec<usize>, String> {
        match timesteps {
            Some(ts) => {
                for &t in ts {
                    if t >= self.number_of_timesteps {
                        return Err(format!("timestep {t} is not in range({})", self.number_of_timesteps));
                    }
                }
                Ok(ts.to_vec())
            }
            None => Ok((0..self.number_of_timesteps).collect()),
        }
    }

    fn add_external_timesteps(&mut self, ts: &[usize]) {
        self.external_timesteps.extend_from_slice(ts);
        self.external_timesteps.sort_unstable();
        self.external_timesteps.dedup();
    }

    fn push(&mut self, mat: usize, component: &str, entry: RateEntry) {
        let comps = &mut self.external_rates[mat];
        match comps.iter_mut().find(|(c, _)| c == component) {
            Some((_, list)) => list.push(entry),
            None => comps.push((component.to_owned(), vec![entry])),
        }
    }

    /// The burnable materials, in the order the block system uses.
    pub fn burnable_mats(&self) -> &[String] {
        &self.burnable_mats
    }

    /// Timesteps at which any rate is set (sorted).
    pub fn external_timesteps(&self) -> &[usize] {
        &self.external_timesteps
    }

    /// Upstream `get_external_rate`: the rates set for `component` in
    /// `material` that include `timestep`, filtered to `destination` when
    /// one is given.
    ///
    /// # Errors
    /// `material` (or `destination`) is not burnable.
    pub fn get_external_rate(
        &self,
        material: &str,
        component: &str,
        timestep: usize,
        destination: Option<&str>,
    ) -> Result<Vec<f64>, String> {
        let m = self.material_index(material)?;
        if let Some(d) = destination {
            self.material_index(d)?;
        }
        let Some((_, list)) = self.external_rates[m].iter().find(|(c, _)| c == component) else {
            return Ok(Vec::new());
        };
        Ok(list
            .iter()
            .filter(|e| e.timesteps.contains(&timestep))
            .filter(|e| destination.is_none_or(|d| e.destination.as_deref() == Some(d)))
            .map(|e| e.rate)
            .collect())
    }

    /// Upstream `get_components`: the components of `material` with a rate
    /// at `timestep`. With a destination, upstream keeps a component when
    /// `timestep` is in **any** of its entries and the destination is in
    /// **any** of its entries (not necessarily the same entry); this port
    /// does the same.
    ///
    /// # Errors
    /// `material` (or `destination`) is not burnable.
    pub fn get_components(&self, material: &str, timestep: usize, destination: Option<&str>) -> Result<Vec<String>, String> {
        let m = self.material_index(material)?;
        if let Some(d) = destination {
            self.material_index(d)?;
        }
        let mut out = Vec::new();
        for (component, list) in &self.external_rates[m] {
            let at_step = list.iter().any(|e| e.timesteps.contains(&timestep));
            let keep = match destination {
                Some(d) => at_step && list.iter().any(|e| e.destination.as_deref() == Some(d)),
                None => at_step,
            };
            if keep {
                out.push(component.clone());
            }
        }
        Ok(out)
    }

}

/// A redox control: buffer nuclides with their fractions, and oxidation
/// states by element (upstream `self.redox[material] = (buffer,
/// oxidation_states)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Redox {
    /// `(nuclide, fraction)`, fractions summing to 1.
    pub buffer: Vec<(String, f64)>,
    /// `(element symbol, oxidation state)`.
    pub oxidation_states: Vec<(String, i32)>,
}

/// Upstream `TransferRates`: continuous removal or feed per atom, optionally
/// into another material, and redox control.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferRates {
    rates: ExternalRates,
    /// Per timestep, the `(destination, source)` pairs in insertion order
    /// (upstream `index_transfer`, a `defaultdict(list)`).
    index_transfer: Vec<(usize, Vec<(String, String)>)>,
    chain_nuclides: Vec<String>,
    redox: Vec<(String, Redox)>,
}

impl TransferRates {
    /// Upstream `TransferRates(operator, materials, number_of_timesteps)`,
    /// with the operator's burnable material ids and its chain's nuclide
    /// names given directly.
    pub fn new(burnable_mats: Vec<String>, chain_nuclides: Vec<String>, number_of_timesteps: usize) -> Self {
        Self { rates: ExternalRates::new(burnable_mats, number_of_timesteps), index_transfer: Vec::new(), chain_nuclides, redox: Vec::new() }
    }

    /// The rates container (upstream's inherited attributes).
    pub fn rates(&self) -> &ExternalRates {
        &self.rates
    }

    /// `(destination, source)` pairs set at `timestep`, insertion order.
    pub fn index_transfer(&self, timestep: usize) -> &[(String, String)] {
        self.index_transfer.iter().find(|(t, _)| *t == timestep).map_or(&[], |(_, v)| v.as_slice())
    }

    /// The redox control of `material`, if set.
    pub fn redox(&self, material: &str) -> Option<&Redox> {
        self.redox.iter().find(|(m, _)| m == material).map(|(_, r)| r)
    }

    /// Upstream `set_transfer_rate`.
    ///
    /// `components` are elements (`"Pa"`) or nuclides (`"Xe135"`). A
    /// positive `transfer_rate` removes, a negative one feeds (grows the
    /// nuclide in proportion to itself, exactly as upstream's diagonal
    /// term does). With `destination`, what is removed goes into that
    /// material.
    ///
    /// # Errors
    /// As upstream: an unknown material or destination; a destination
    /// with only one burnable material; a timestep out of range; an
    /// unknown element; an element rate where one of its nuclides already
    /// has a rate in this material, or the reverse.
    pub fn set_transfer_rate(
        &mut self,
        material: &str,
        components: &[&str],
        transfer_rate: f64,
        unit: RateUnit,
        timesteps: Option<&[usize]>,
        destination: Option<&str>,
    ) -> Result<(), String> {
        let m = self.rates.material_index(material)?;
        if !transfer_rate.is_finite() {
            return Err(format!("transfer_rate must be a real number, got {transfer_rate}"));
        }
        if let Some(d) = destination {
            if self.rates.burnable_mats.len() > 1 {
                self.rates.material_index(d)?;
            } else {
                return Err(format!("Transfer to material {d} is set, but there is only one depletable material"));
            }
        }
        let unit_conv = seconds_in(unit);
        let ts = self.rates.timesteps(timesteps)?;
        for &component in components {
            let element = element_of(component);
            if !is_element_symbol(element) {
                return Err(format!("{component} is not a valid nuclide or element."));
            }
            let current: Vec<&str> = self.rates.external_rates[m].iter().map(|(c, _)| c.as_str()).collect();
            if names_an_element(component) {
                // Upstream: re.match(component + r'\d', c)
                let element_nucs: Vec<&str> = current
                    .iter()
                    .copied()
                    .filter(|c| c.len() > component.len() && c.starts_with(component) && c.as_bytes()[component.len()].is_ascii_digit())
                    .collect();
                if !element_nucs.is_empty() {
                    return Err(format!(
                        "Cannot add transfer rate for element {component} to material {material} with transfer rate(s) for nuclide(s) {}.",
                        element_nucs.join(", ")
                    ));
                }
            } else if current.contains(&element) {
                return Err(format!(
                    "Cannot add transfer rate for nuclide {component} to material {material} where element {element} already has a transfer rate."
                ));
            }
            self.rates.push(
                m,
                component,
                RateEntry { timesteps: ts.clone(), rate: transfer_rate / unit_conv, destination: destination.map(str::to_owned) },
            );
            if let Some(d) = destination {
                for &t in &ts {
                    let pair = (d.to_owned(), material.to_owned());
                    match self.index_transfer.iter_mut().find(|(tt, _)| *tt == t) {
                        Some((_, v)) => v.push(pair),
                        None => self.index_transfer.push((t, vec![pair])),
                    }
                }
            }
            self.rates.add_external_timesteps(&ts);
        }
        Ok(())
    }

    /// Upstream `set_redox`. Upstream stores no timesteps with the control
    /// (they only extend `external_timesteps`), so it applies at every step
    /// at which transfer rates are active; this port does the same.
    ///
    /// # Errors
    /// An unknown material, a timestep out of range, a buffer nuclide not in
    /// the chain, or an unknown element.
    pub fn set_redox(
        &mut self,
        material: &str,
        buffer: &[(&str, f64)],
        oxidation_states: &[(&str, i32)],
        timesteps: Option<&[usize]>,
    ) -> Result<(), String> {
        self.rates.material_index(material)?;
        let ts = self.rates.timesteps(timesteps)?;
        for (nuc, _) in buffer {
            if !self.chain_nuclides.iter().any(|c| c == nuc) {
                return Err(format!("{nuc} is not a valid nuclide."));
            }
        }
        for (elm, _) in oxidation_states {
            if !is_element_symbol(elm) {
                return Err(format!("{elm} is not a valid element."));
            }
        }
        let r = Redox {
            buffer: buffer.iter().map(|(n, f)| ((*n).to_owned(), *f)).collect(),
            oxidation_states: oxidation_states.iter().map(|(e, o)| ((*e).to_owned(), *o)).collect(),
        };
        match self.redox.iter_mut().find(|(m, _)| m == material) {
            Some((_, old)) => *old = r,
            None => self.redox.push((material.to_owned(), r)),
        }
        self.rates.add_external_timesteps(&ts);
        Ok(())
    }
}

/// Upstream `ExternalSourceRates`: a constant source of nuclides into a
/// material, in atoms per second.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalSourceRates {
    rates: ExternalRates,
}

impl ExternalSourceRates {
    /// Upstream `ExternalSourceRates(operator, materials, number_of_timesteps)`.
    pub fn new(burnable_mats: Vec<String>, number_of_timesteps: usize) -> Self {
        Self { rates: ExternalRates::new(burnable_mats, number_of_timesteps) }
    }

    /// The rates container.
    pub fn rates(&self) -> &ExternalRates {
        &self.rates
    }

    /// Upstream `set_external_source_rate`: a mass rate of a composition
    /// (weight percents, normalised to 1), each component expanded to
    /// atoms per second as `rate / atomic_mass(nuc) * AVOGADRO * percent /
    /// unit_conv` (and `* frac` for an element's natural isotopes), in
    /// upstream's operation order. A positive rate feeds, a negative one
    /// removes.
    ///
    /// `atomic_mass` gives a nuclide's mass in g/mol (upstream
    /// `openmc.data.atomic_mass`, AME); the workspace's is
    /// [`crate::model::masses::AtomicMasses`].
    ///
    /// # Errors
    /// An unknown material or element, an element with no natural
    /// isotopes, a missing atomic mass, or a timestep out of range.
    pub fn set_external_source_rate(
        &mut self,
        material: &str,
        composition: &[(&str, f64)],
        rate: f64,
        unit: MassRateUnit,
        timesteps: Option<&[usize]>,
        atomic_mass: impl Fn(&str) -> Option<f64>,
    ) -> Result<(), String> {
        let m = self.rates.material_index(material)?;
        if !rate.is_finite() {
            return Err(format!("rate must be a real number, got {rate}"));
        }
        let unit_conv = seconds_in_mass(unit);
        let ts = self.rates.timesteps(timesteps)?;
        let total: f64 = composition.iter().map(|(_, p)| *p).sum();
        let mut atoms_per_nuc: Vec<(String, f64)> = Vec::new();
        let mut put = |nuc: String, val: f64| match atoms_per_nuc.iter_mut().find(|(n, _)| *n == nuc) {
            Some((_, v)) => *v = val,
            None => atoms_per_nuc.push((nuc, val)),
        };
        for (component, p) in composition {
            let percent = *p / total;
            let element = element_of(component);
            if !is_element_symbol(element) {
                return Err(format!("{component} is not a valid nuclide or element."));
            }
            if names_an_element(component) {
                let isotopes = element_from_symbol(component).map(|e| e.natural_abundance()).unwrap_or_default();
                if isotopes.is_empty() {
                    return Err(format!(
                        "Cannot add element {component} as it is not naturally abundant. Specify a nuclide vector instead."
                    ));
                }
                for (iso, frac) in isotopes {
                    let nuc = iso.name();
                    let mass = atomic_mass(&nuc).ok_or_else(|| format!("no atomic mass for {nuc}"))?;
                    put(nuc, rate / mass * AVOGADRO_PER_MOL * frac * percent / unit_conv);
                }
            } else {
                let mass = atomic_mass(component).ok_or_else(|| format!("no atomic mass for {component}"))?;
                put((*component).to_owned(), rate / mass * AVOGADRO_PER_MOL * percent / unit_conv);
            }
        }
        for (nuc, val) in atoms_per_nuc {
            self.rates.push(m, &nuc, RateEntry { timesteps: ts.clone(), rate: val, destination: None });
        }
        self.rates.add_external_timesteps(&ts);
        Ok(())
    }

    /// **Extension, not upstream API:** a source of one nuclide given
    /// directly in atoms per second (what upstream's mass form reduces to).
    /// For callers that already work in atoms, and for analytic tests.
    ///
    /// # Errors
    /// An unknown material or nuclide element, or a timestep out of range.
    pub fn set_external_source_atoms(
        &mut self,
        material: &str,
        nuclide: &str,
        atoms_per_second: f64,
        timesteps: Option<&[usize]>,
    ) -> Result<(), String> {
        let m = self.rates.material_index(material)?;
        if !is_element_symbol(element_of(nuclide)) || names_an_element(nuclide) {
            return Err(format!("{nuclide} is not a valid nuclide."));
        }
        let ts = self.rates.timesteps(timesteps)?;
        self.rates.push(m, nuclide, RateEntry { timesteps: ts.clone(), rate: atoms_per_second, destination: None });
        self.rates.add_external_timesteps(&ts);
        Ok(())
    }
}

/// Which transfer term to form (upstream `form_rr_term`'s `mats` argument:
/// a material id, or a `(destination, source)` pair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferTerm {
    /// Removal from one material: subtracted from its Bateman matrix.
    Removal(String),
    /// Transfer from `source` into `destination`: placed in the block
    /// matrix at the destination's row and the source's column.
    Between {
        /// Material receiving the nuclides.
        destination: String,
        /// Material losing them.
        source: String,
    },
}

/// Upstream `Chain.form_rr_term`: the diagonal matrix of summed transfer
/// rates over the chain's nuclides. A nuclide takes its **element's** rate
/// if the element is a component, else its own.
///
/// # Errors
/// An unknown material.
pub fn form_rr_term(nuclides: &[String], tr: &TransferRates, timestep: usize, mats: &TransferTerm) -> Result<DepletionMatrix, String> {
    let n = nuclides.len();
    let mut matrix = DepletionMatrix::zeros(n);
    let (mat, dest) = match mats {
        TransferTerm::Removal(m) => (m.as_str(), None),
        TransferTerm::Between { destination, source } => (source.as_str(), Some(destination.as_str())),
    };
    let components = tr.rates.get_components(mat, timestep, dest)?;
    for (i, nuc) in nuclides.iter().enumerate() {
        let elm = element_of(nuc);
        let key = if components.iter().any(|c| c == elm) {
            elm
        } else if components.iter().any(|c| c == nuc) {
            nuc.as_str()
        } else {
            continue;
        };
        let rates = tr.rates.get_external_rate(mat, key, timestep, dest)?;
        matrix.set(i, i, rates.iter().sum()).map_err(|e| e.to_string())?;
    }
    Ok(matrix)
}

/// Upstream `Chain.add_redox_term`: for every column `i`, the net change in
/// oxidation state `Σ_j a_ji · OS_j` is subtracted from each buffer
/// nuclide's row, scaled by its fraction over its own oxidation state.
/// Elements without a given oxidation state count as 0.
///
/// # Errors
/// A buffer nuclide not in `nuclides`, or one whose oxidation state is 0
/// (upstream divides by it; see the module docs, departure 3).
pub fn add_redox_term(matrix: &DepletionMatrix, nuclides: &[String], redox: &Redox) -> Result<DepletionMatrix, String> {
    let n = nuclides.len();
    if matrix.order() != n {
        return Err(format!("redox on an order-{} matrix with {n} nuclides", matrix.order()));
    }
    let os: Vec<f64> = nuclides
        .iter()
        .map(|nuc| {
            let elm = element_of(nuc);
            redox.oxidation_states.iter().find(|(e, _)| e == elm).map_or(0.0, |(_, o)| f64::from(*o))
        })
        .collect();
    let mut out = matrix.clone();
    let redox_change: Vec<f64> = (0..n)
        .map(|i| {
            let mut s = 0.0;
            for j in 0..n {
                s += matrix.get(j, i) * os[j];
            }
            s
        })
        .collect();
    for (nuc, fraction) in &redox.buffer {
        let idx = nuclides.iter().position(|c| c == nuc).ok_or_else(|| format!("{nuc} is not a valid nuclide."))?;
        if os[idx] == 0.0 {
            return Err(format!(
                "buffer nuclide {nuc} has oxidation state 0; upstream divides by it (inf/nan), so it is refused here"
            ));
        }
        for (k, rc) in redox_change.iter().enumerate() {
            let v = out.get(idx, k) - rc * fraction / os[idx];
            out.set(idx, k, v).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

/// Upstream `Chain.form_ext_source_term`: the source vector of `mat` at
/// `timestep` in chain order, or `None` when it has no source then.
///
/// # Errors
/// An unknown material.
pub fn form_ext_source_term(nuclides: &[String], es: &ExternalSourceRates, timestep: usize, mat: &str) -> Result<Option<Vec<f64>>, String> {
    let components = es.rates.get_components(mat, timestep, None)?;
    if components.is_empty() {
        return Ok(None);
    }
    let mut v = vec![0.0; nuclides.len()];
    for (i, nuc) in nuclides.iter().enumerate() {
        if components.iter().any(|c| c == nuc) {
            v[i] = es.rates.get_external_rate(mat, nuc, timestep, None)?.iter().sum();
        }
    }
    Ok(Some(v))
}

/// Upstream `_add_external_source` for one material: `[[A, s], [0, 0]]` and
/// `[n, 1]`.
fn augment(matrix: &DepletionMatrix, source: &[f64], n: &[f64]) -> Result<(DepletionMatrix, Vec<f64>), String> {
    let k = matrix.order();
    let mut a = DepletionMatrix::zeros(k + 1);
    for r in 0..k {
        for c in 0..k {
            a.set(r, c, matrix.get(r, c)).map_err(|e| e.to_string())?;
        }
        a.set(r, k, source[r]).map_err(|e| e.to_string())?;
    }
    let mut v = n.to_vec();
    v.push(1.0);
    Ok((a, v))
}

/// Embed `block` (order `rows` × `cols` taken from its top-left) into `big`
/// at `(r0, c0)`.
fn place(big: &mut DepletionMatrix, block: &DepletionMatrix, r0: usize, c0: usize, rows: usize, cols: usize) -> Result<(), String> {
    for r in 0..rows {
        for c in 0..cols {
            let v = block.get(r, c);
            if v != 0.0 {
                big.set(r0 + r, c0 + c, v).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Upstream `openmc.deplete.pool.deplete`, from the point where the
/// per-material Bateman matrices are formed: apply transfer rates, redox,
/// external sources and material-to-material transfers at `timestep`, solve
/// over `dt` seconds with `solve` (for example [`super::cram::cram16`]), and
/// return each material's new nuclide vector (augmented entries removed).
///
/// `matrices` and `n` are per burnable material, in the order of the rates'
/// `burnable_mats`; `nuclides` is the chain order both follow.
///
/// # Errors
/// Mismatched lengths, materials that differ between `transfer` and
/// `sources`, or an error from the terms above.
pub fn deplete_with_external_rates(
    solve: impl Fn(&DepletionMatrix, &[f64], f64) -> Vec<f64>,
    nuclides: &[String],
    matrices: Vec<DepletionMatrix>,
    n: Vec<Vec<f64>>,
    dt: f64,
    timestep: usize,
    transfer: Option<&TransferRates>,
    sources: Option<&ExternalSourceRates>,
) -> Result<Vec<Vec<f64>>, String> {
    if matrices.len() != n.len() {
        return Err(format!("{} matrices for {} nuclide vectors", matrices.len(), n.len()));
    }
    let mats: Vec<String> = match (transfer, sources) {
        (Some(t), Some(s)) => {
            if t.rates.burnable_mats != s.rates.burnable_mats {
                return Err("transfer rates and external sources name different burnable materials".into());
            }
            t.rates.burnable_mats.clone()
        }
        (Some(t), None) => t.rates.burnable_mats.clone(),
        (None, Some(s)) => s.rates.burnable_mats.clone(),
        (None, None) => (0..matrices.len()).map(|i| i.to_string()).collect(),
    };
    if mats.len() != matrices.len() {
        return Err(format!("{} burnable materials but {} matrices", mats.len(), matrices.len()));
    }
    let transfer_active = transfer.is_some_and(|t| t.rates.external_timesteps.contains(&timestep));
    let external_active = sources.is_some_and(|s| s.rates.external_timesteps.contains(&timestep));

    let mut matrices = matrices;
    let mut n_solve = n;
    // Which materials were augmented with a source column (departure 2).
    let mut augmented = vec![false; mats.len()];

    let add_sources = |matrices: &mut Vec<DepletionMatrix>, n_solve: &mut Vec<Vec<f64>>, augmented: &mut Vec<bool>| -> Result<(), String> {
        let Some(s) = sources else { return Ok(()) };
        for (i, m) in mats.iter().enumerate() {
            if let Some(src) = form_ext_source_term(nuclides, s, timestep, m)? {
                let (a, v) = augment(&matrices[i], &src, &n_solve[i])?;
                matrices[i] = a;
                n_solve[i] = v;
                augmented[i] = true;
            }
        }
        Ok(())
    };

    if transfer_active {
        let t = transfer.expect("transfer_active implies Some");
        for (i, m) in mats.iter().enumerate() {
            let term = form_rr_term(nuclides, t, timestep, &TransferTerm::Removal(m.clone()))?;
            let k = matrices[i].order();
            for r in 0..k {
                let v = matrices[i].get(r, r) - term.get(r, r);
                matrices[i].set(r, r, v).map_err(|e| e.to_string())?;
            }
        }
        for (i, m) in mats.iter().enumerate() {
            if let Some(rx) = t.redox(m) {
                matrices[i] = add_redox_term(&matrices[i], nuclides, rx)?;
            }
        }
        if external_active {
            add_sources(&mut matrices, &mut n_solve, &mut augmented)?;
        }
        let pairs = t.index_transfer(timestep);
        if !pairs.is_empty() {
            // One block system over every material, in burnable order.
            let sizes: Vec<usize> = matrices.iter().map(DepletionMatrix::order).collect();
            let offsets: Vec<usize> = sizes.iter().scan(0, |acc, s| { let o = *acc; *acc += s; Some(o) }).collect();
            let total: usize = sizes.iter().sum();
            let mut big = DepletionMatrix::zeros(total);
            for (i, a) in matrices.iter().enumerate() {
                place(&mut big, a, offsets[i], offsets[i], sizes[i], sizes[i])?;
            }
            // Upstream: dict.fromkeys(...) de-duplicates, keeping order.
            let mut seen: Vec<(String, String)> = Vec::new();
            for p in pairs {
                if !seen.contains(p) {
                    seen.push(p.clone());
                }
            }
            for (dest, src) in &seen {
                let term = form_rr_term(nuclides, t, timestep, &TransferTerm::Between { destination: dest.clone(), source: src.clone() })?;
                let term = match t.redox(dest) {
                    Some(rx) => add_redox_term(&term, nuclides, rx)?,
                    None => term,
                };
                let r = mats.iter().position(|m| m == dest).ok_or_else(|| format!("unknown material {dest}"))?;
                let c = mats.iter().position(|m| m == src).ok_or_else(|| format!("unknown material {src}"))?;
                // The padding rows / columns of augmented blocks stay zero,
                // as upstream's vstack / hstack of zeros.
                place(&mut big, &term, offsets[r], offsets[c], nuclides.len(), nuclides.len())?;
            }
            let n_multi: Vec<f64> = n_solve.iter().flatten().copied().collect();
            let n_result = solve(&big, &n_multi, dt);
            let mut out = Vec::with_capacity(mats.len());
            for i in 0..mats.len() {
                let v = &n_result[offsets[i]..offsets[i] + sizes[i]];
                out.push(if augmented[i] { v[..v.len() - 1].to_vec() } else { v.to_vec() });
            }
            return Ok(out);
        }
    } else if external_active {
        add_sources(&mut matrices, &mut n_solve, &mut augmented)?;
    }

    let mut out = Vec::with_capacity(mats.len());
    for i in 0..mats.len() {
        let mut v = solve(&matrices[i], &n_solve[i], dt);
        if augmented[i] {
            v.pop();
        }
        out.push(v);
    }
    // Upstream strips any material with a source at any timestep; this
    // port strips exactly the augmented ones (departure 2).
    Ok(out)
}

#[cfg(test)]
mod tests;
