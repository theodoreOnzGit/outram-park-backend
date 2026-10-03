// SPDX-License-Identifier: GPL-3.0-only
//! **The continuous-energy data `openmc.plot_xs` sees, built from an ACE
//! table.**
//!
//! `plot_xs` reads nuclides with `openmc.data.IncidentNeutron.from_hdf5` from a
//! library written by `IncidentNeutron.from_ace(...).export_to_hdf5(...)`.
//! That round trip is not transparent — `export_to_hdf5` drops some redundant
//! reactions and `from_hdf5` re-attaches the total fission yield to every
//! fission MT — so this module reproduces **both halves**, not only the ACE
//! read. What comes out is the reaction set, cross sections and neutron yields
//! upstream's plotter would find.
//!
//! Ported from OpenMC at commit `d7d3284a1` (0.16.1.dev25), MIT-licensed
//! (notice in `crates/outram-mc-libs/LICENSE.openmc`):
//!
//! | here | upstream |
//! |---|---|
//! | [`IncidentNeutronData::from_ace`] | `IncidentNeutron.from_ace`, `openmc/data/neutron.py:510-648` |
//! | reaction cross section and yield | `Reaction.from_ace`, `openmc/data/reaction.py:1004-1145` |
//! | fission yields | `_get_fission_products_ace`, `reaction.py:233-360` |
//! | redundant-reaction build | `IncidentNeutron._get_redundant_reaction`, `neutron.py:846-875` |
//! | what survives the HDF5 round trip | `export_to_hdf5`, `neutron.py:400-418`; `from_hdf5`, `neutron.py:487-495` |
//! | [`IncidentNeutronData::get_reaction_components`] | `neutron.py:331-353` |
//! | ZAID to name | `openmc/data/ace.py`, `get_metadata` (`nndc` scheme) |
//!
//! The ACE decode itself (ESZ, MTR/LQR/TYR/LSIG/SIG) is **reused** from
//! `njoy_outram_park_fork::acer::ce_decode::decode_ce`, the reader gh:#307
//! introduced; only what the plotter needs beyond it (neutron yields, photon
//! MT list, delayed-group yields) is read here.
//!
//! # What is deliberately not carried
//!
//! Angular and energy distributions, URR probability tables and photon yields
//! are read by upstream but never touched by `plot_xs`, so they are not read
//! here. Photon products are recorded only as *present*, because their
//! presence decides whether a redundant reaction survives `export_to_hdf5`.

use std::collections::BTreeMap;
use std::path::Path;

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::acer::read::RawAceTable;

use super::endf_tables::{gnds_name, sum_rule};
use super::function1d::{px, Function1D, Polynomial, Tabulated1D, EV_PER_MEV};
use super::numpy_ops::{python_round, union1d};
use super::PlotError;

/// `openmc.data.K_BOLTZMANN` \[eV/K\].
pub const K_BOLTZMANN: f64 = 8.617333262e-05;

/// `FISSION_MTS` (`reaction.py:84`).
pub const FISSION_MTS: [i32; 5] = [18, 19, 20, 21, 38];

/// Which particle a product is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Particle {
    /// A neutron.
    Neutron,
    /// A photon.
    Photon,
}

/// `Product.emission_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionMode {
    /// `'prompt'` (upstream's default).
    Prompt,
    /// `'delayed'`.
    Delayed,
    /// `'total'`.
    Total,
}

/// `openmc.data.Product`, reduced to what `plot_xs` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    /// Particle type.
    pub particle: Particle,
    /// Emission mode.
    pub emission_mode: EmissionMode,
    /// Yield. `None` for photons, whose yield the plotter never evaluates.
    pub yield_: Option<Function1D>,
}

/// `openmc.data.Reaction` at one temperature, reduced to what `plot_xs` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotReaction {
    /// ENDF MT.
    pub mt: i32,
    /// Cross section on `energy[threshold_idx..]` (after the HDF5 round trip
    /// the abscissae always run to the end of the nuclide grid).
    pub xs: Tabulated1D,
    /// Index of the first grid point of `xs`.
    pub threshold_idx: usize,
    /// Products, in upstream order.
    pub products: Vec<Product>,
    /// Derived products (the total fission neutron).
    pub derived_products: Vec<Product>,
    /// Whether upstream marks this reaction redundant.
    pub redundant: bool,
}

impl PlotReaction {
    fn has_photon(&self) -> bool {
        self.products.iter().any(|p| p.particle == Particle::Photon)
    }
}

/// One nuclide's incident-neutron data as `openmc.plot_xs` reads it back from
/// an HDF5 library — the Rust stand-in for `openmc.data.IncidentNeutron`.
#[derive(Debug, Clone, PartialEq)]
pub struct IncidentNeutronData {
    /// GNDS name, e.g. `"U235"`.
    pub name: String,
    /// Atomic weight ratio.
    pub atomic_weight_ratio: f64,
    /// `kT` of the (single) temperature \[eV\].
    pub kt: f64,
    /// Energy grid \[eV\].
    pub energy: Vec<f64>,
    /// Reactions by MT, as they exist after `export_to_hdf5` / `from_hdf5`.
    pub reactions: BTreeMap<i32, PlotReaction>,
}

impl IncidentNeutronData {
    /// Read an ACE file (Type 1 or 2; `.gz` accepted by the reader).
    ///
    /// `name` overrides the GNDS name derived from the ZAID.
    pub fn from_ace_file(path: impl AsRef<Path>, name: Option<&str>) -> Result<Self, PlotError> {
        let t = njoy_outram_park_fork::acer::read::read(path.as_ref())
            .map_err(|e| PlotError::Ace(format!("{}: {e}", path.as_ref().display())))?;
        Self::from_ace(&t, name)
    }

    /// `IncidentNeutron.from_ace` followed by the `export_to_hdf5` /
    /// `from_hdf5` round trip. See the module docs for what that means.
    pub fn from_ace(t: &RawAceTable, name: Option<&str>) -> Result<Self, PlotError> {
        let ace = decode_ce(t).map_err(|e| PlotError::Ace(e.to_string()))?;
        let name = match name {
            Some(n) => n.to_string(),
            None => name_from_zaid(&ace.zaid)?,
        };
        let jxs = |i: usize| t.jxs[i - 1] as isize; // upstream's one-based JXS
        let nxs = |i: usize| t.nxs[i - 1] as isize; // upstream's one-based NXS
        let grid = ace.energy.clone();
        let kt = ace.kt_ev;

        // Photon-production MTs (`reaction.py:621-629`, `neutron.py:595-599`).
        let n_photon = nxs(6).max(0) as usize;
        let mut photon_mts: Vec<i32> = Vec::with_capacity(n_photon);
        for k in 0..n_photon {
            photon_mts.push(px(t, jxs(13) as usize + k)? as i64 as i32);
        }
        let photon_rx_mts: Vec<i32> = {
            let mut v: Vec<i32> = photon_mts.iter().map(|m| m.div_euclid(1000)).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let photon_products = |mt: i32| -> Vec<Product> {
            if photon_rx_mts.contains(&mt) {
                vec![Product {
                    particle: Particle::Photon,
                    emission_mode: EmissionMode::Prompt,
                    yield_: None,
                }]
            } else {
                Vec::new()
            }
        };

        let mut rx: BTreeMap<i32, PlotReaction> = BTreeMap::new();
        let full = |y: Vec<f64>| Tabulated1D::lin_lin(grid.clone(), y);

        // Redundant total (MT=1), absorption (MT=101), heating (MT=301):
        // `neutron.py:567-584`.
        rx.insert(1, redundant(1, full(ace.total.clone()), 0));
        if ace.absorption.iter().any(|&a| a != 0.0) {
            rx.insert(101, redundant(101, full(ace.absorption.clone()), 0));
        }
        let heating: Vec<f64> = ace
            .heating
            .iter()
            .zip(ace.total.iter())
            .map(|(h, s)| h * s)
            .collect();
        rx.insert(301, redundant(301, full(heating), 0));

        // Elastic, i_reaction = 0 (`reaction.py:1094-1115`): negatives zeroed.
        let elastic: Vec<f64> = ace
            .elastic
            .iter()
            .map(|&v| if v < 0.0 { 0.0 } else { v })
            .collect();
        let mut el_products = vec![Product {
            particle: Particle::Neutron,
            emission_mode: EmissionMode::Prompt,
            yield_: Some(Function1D::Polynomial(Polynomial { coef: vec![1.0] })),
        }];
        el_products.extend(photon_products(2));
        rx.insert(
            2,
            PlotReaction {
                mt: 2,
                xs: full(elastic),
                threshold_idx: 0,
                products: el_products,
                derived_products: Vec::new(),
                redundant: false,
            },
        );

        // i_reaction = 1..=NTR (`reaction.py:1014-1092`).
        let n_neutron_rx = nxs(5).max(0) as usize;
        for (i0, r) in ace.reactions.iter().enumerate() {
            let i_reaction = i0 + 1;
            let thr = r.threshold_index;
            // After `from_hdf5` the abscissae are `energy[threshold_idx:]`
            // (`reaction.py:985-987`), however many ordinates SIG carried.
            let x = grid[thr..].to_vec();
            if x.len() != r.xs.len() {
                return Err(PlotError::Ace(format!(
                    "MT={} carries {} cross-section values over {} grid points from its \
                     threshold; upstream's `Tabulated1D(energy[threshold_idx:], xs)` would \
                     index past the end of y",
                    r.mt,
                    r.xs.len(),
                    x.len()
                )));
            }
            let mut y = r.xs.clone();
            if r.mt == 444 {
                for v in y.iter_mut() {
                    *v *= EV_PER_MEV;
                }
            }
            let mut products = Vec::new();
            let mut derived = Vec::new();
            if i_reaction < n_neutron_rx + 1 {
                let ty = r.ty;
                if ty != 19 {
                    let yield_ = if ty.abs() > 100 {
                        let idx = jxs(11) + ty.abs() as isize - 101;
                        Function1D::Tabulated(Tabulated1D::from_ace(t, idx as usize)?)
                    } else {
                        Function1D::Polynomial(Polynomial {
                            coef: vec![ty.abs() as f64],
                        })
                    };
                    products.push(Product {
                        particle: Particle::Neutron,
                        emission_mode: EmissionMode::Prompt,
                        yield_: Some(yield_),
                    });
                } else {
                    if !FISSION_MTS.contains(&r.mt) {
                        return Err(PlotError::Ace(format!(
                            "TYR=19 on MT={}, which is not a fission MT",
                            r.mt
                        )));
                    }
                    let (p, d) = fission_products_ace(t)?;
                    products = p;
                    derived = d;
                }
            }
            products.extend(photon_products(r.mt));
            rx.insert(
                r.mt,
                PlotReaction {
                    mt: r.mt,
                    xs: Tabulated1D::lin_lin(x, y),
                    threshold_idx: thr,
                    products,
                    derived_products: derived,
                    redundant: false,
                },
            );
        }

        let mut data = Self {
            name,
            atomic_weight_ratio: ace.awr,
            kt,
            energy: grid,
            reactions: rx,
        };

        // Photon production on an MT that has no cross section
        // (`neutron.py:599-616`).
        for &mt in &photon_rx_mts {
            if data.reactions.contains_key(&mt) || sum_rule(mt).is_none() {
                continue;
            }
            let mts = data.get_reaction_components(mt);
            if mts.is_empty() {
                continue;
            }
            let mut r = data.redundant_reaction(mt, &mts);
            r.products.extend(photon_products(mt));
            data.reactions.insert(mt, r);
        }
        // Summed transmutation reactions from their levels (`neutron.py:623-632`).
        for mt in [16, 103, 104, 105, 106, 107] {
            if data.reactions.contains_key(&mt) {
                continue;
            }
            let mts = data.get_reaction_components(mt);
            if mts.is_empty() {
                continue;
            }
            let r = data.redundant_reaction(mt, &mts);
            data.reactions.insert(mt, r);
        }
        // Mark redundancy (`neutron.py:636-641`).
        let keys: Vec<i32> = data.reactions.keys().copied().collect();
        for mt in keys {
            let comps = data.get_reaction_components(mt);
            let red = comps != [mt] || [203, 204, 205, 206, 207, 444].contains(&mt);
            if red {
                data.reactions.get_mut(&mt).expect("key").redundant = true;
            }
        }

        // `export_to_hdf5` (`neutron.py:400-418`): a redundant reaction is
        // written only with photon products or when in `keep_mts`.
        const KEEP_MTS: [i32; 15] = [
            4, 16, 103, 104, 105, 106, 107, 203, 204, 205, 206, 207, 301, 444, 901,
        ];
        data.reactions
            .retain(|mt, r| !r.redundant || r.has_photon() || KEEP_MTS.contains(mt));
        // The first written reaction with derived products supplies
        // `total_nu`; `from_hdf5` appends it to every fission MT read back
        // (`neutron.py:414-417`, `:487-495`). `Reaction.to_hdf5` itself does
        // not write derived products, so everything else loses them.
        // Python-dict iteration order at export: 1, 101, 301, 2, MTR order,
        // then the redundant reactions built above.
        let total_nu = data.first_total_nu(&ace.reactions.iter().map(|r| r.mt).collect::<Vec<_>>());
        for (mt, r) in data.reactions.iter_mut() {
            r.derived_products.clear();
            if FISSION_MTS.contains(mt) {
                if let Some(tn) = &total_nu {
                    r.derived_products.push(tn.clone());
                }
            }
        }
        Ok(data)
    }

    fn first_total_nu(&self, mtr_order: &[i32]) -> Option<Product> {
        let order = [1, 101, 301, 2]
            .into_iter()
            .chain(mtr_order.iter().copied())
            .chain(self.reactions.keys().copied());
        for mt in order {
            if let Some(r) = self.reactions.get(&mt) {
                if let Some(d) = r.derived_products.first() {
                    return Some(d.clone());
                }
            }
        }
        None
    }

    /// `IncidentNeutron.get_reaction_components` (`neutron.py:331-353`).
    pub fn get_reaction_components(&self, mt: i32) -> Vec<i32> {
        let mut mts = Vec::new();
        if let Some(rule) = sum_rule(mt) {
            for &m in rule {
                mts.extend(self.get_reaction_components(m));
            }
        }
        if !mts.is_empty() {
            mts
        } else if self.reactions.contains_key(&mt) {
            vec![mt]
        } else {
            Vec::new()
        }
    }

    /// `_get_redundant_reaction` (`neutron.py:846-875`): the component
    /// cross sections summed (`Sum.__call__` is Python's `sum`, i.e. left to
    /// right from integer 0) on `energy[min threshold:]`.
    fn redundant_reaction(&self, mt: i32, mts: &[i32]) -> PlotReaction {
        let idx = mts
            .iter()
            .map(|m| self.reactions[m].threshold_idx)
            .min()
            .unwrap_or(0);
        let e = &self.energy[idx..];
        let mut acc = vec![0.0; e.len()];
        for m in mts {
            for (a, v) in acc.iter_mut().zip(self.reactions[m].xs.eval(e)) {
                *a += v;
            }
        }
        PlotReaction {
            mt,
            xs: Tabulated1D::lin_lin(e.to_vec(), acc),
            threshold_idx: idx,
            products: Vec::new(),
            derived_products: Vec::new(),
            redundant: true,
        }
    }

    /// `IncidentNeutron.temperatures`: `f"{int(round(kT / K_BOLTZMANN))}K"`.
    pub fn temperature_label(&self) -> String {
        format!("{}K", python_round(self.kt / K_BOLTZMANN) as i64)
    }
}

fn redundant(mt: i32, xs: Tabulated1D, thr: usize) -> PlotReaction {
    PlotReaction {
        mt,
        xs,
        threshold_idx: thr,
        products: Vec::new(),
        derived_products: Vec::new(),
        redundant: true,
    }
}

/// `get_metadata(zaid, 'nndc')` (`openmc/data/ace.py`), then `gnds_name`.
fn name_from_zaid(zaid: &str) -> Result<String, PlotError> {
    let head = zaid.trim().split('.').next().unwrap_or("");
    let z_a: i64 = head
        .parse()
        .map_err(|_| PlotError::Ace(format!("cannot read a ZAID from {zaid:?}")))?;
    let z = z_a / 1000;
    let mut a = z_a % 1000;
    let m = if a > 300 { 1 } else { 0 };
    while a > 3 * z {
        a -= 100;
    }
    gnds_name(z as u32, a as u32, m)
        .ok_or_else(|| PlotError::Ace(format!("ZAID {zaid:?} has no element symbol")))
}

/// `_get_fission_products_ace` (`reaction.py:233-360`), neutron yields only.
fn fission_products_ace(t: &RawAceTable) -> Result<(Vec<Product>, Vec<Product>), PlotError> {
    let jxs = |i: usize| t.jxs[i - 1] as usize;
    let nxs = |i: usize| t.nxs[i - 1] as isize;
    if jxs(2) == 0 {
        return Err(PlotError::Ace(
            "fission reaction (TYR=19) but no NU block; upstream fails here too".into(),
        ));
    }
    let nu_yield = |idx: usize| -> Result<Function1D, PlotError> {
        let lnu = px(t, idx)? as i64;
        match lnu {
            1 => {
                let nc = px(t, idx + 1)? as usize;
                let mut c = Vec::with_capacity(nc);
                for i in 0..nc {
                    // `coefficients[i] *= EV_PER_MEV**(-i)`
                    c.push(px(t, idx + 2 + i)? * EV_PER_MEV.powf(-(i as f64)));
                }
                Ok(Function1D::Polynomial(Polynomial { coef: c }))
            }
            2 => Ok(Function1D::Tabulated(Tabulated1D::from_ace(t, idx + 1)?)),
            other => Err(PlotError::Ace(format!("NU block LNU={other}"))),
        }
    };
    let mut products = Vec::new();
    let mut derived = Vec::new();
    let first = px(t, jxs(2))?;
    if first > 0.0 {
        let which = if jxs(24) > 0 {
            EmissionMode::Prompt
        } else {
            EmissionMode::Total
        };
        products.push(Product {
            particle: Particle::Neutron,
            emission_mode: which,
            yield_: Some(nu_yield(jxs(2))?),
        });
    } else if first < 0.0 {
        let prompt = nu_yield(jxs(2) + 1)?;
        let idx = jxs(2) + first.abs() as usize + 1;
        let total = nu_yield(idx)?;
        products.push(Product {
            particle: Particle::Neutron,
            emission_mode: EmissionMode::Prompt,
            yield_: Some(prompt),
        });
        derived.push(Product {
            particle: Particle::Neutron,
            emission_mode: EmissionMode::Total,
            yield_: Some(total),
        });
    }
    if jxs(24) > 0 {
        let yield_delayed = Tabulated1D::from_ace(t, jxs(24) + 1)?;
        let mut idx = jxs(25);
        for _group in 0..nxs(8).max(0) {
            let gp = Tabulated1D::from_ace(t, idx + 1)?;
            let y = if gp.y.iter().all(|&v| v == gp.y[0]) {
                let mut yd = yield_delayed.clone();
                for v in yd.y.iter_mut() {
                    *v *= gp.y[0];
                }
                yd
            } else {
                let max_energy = yield_delayed
                    .x
                    .last()
                    .copied()
                    .unwrap_or(0.0)
                    .min(gp.x.last().copied().unwrap_or(0.0));
                let e: Vec<f64> = union1d(&yield_delayed.x, &gp.x)
                    .into_iter()
                    .filter(|&v| v <= max_energy)
                    .collect();
                let a = yield_delayed.eval(&e);
                let b = gp.eval(&e);
                let y: Vec<f64> = a.iter().zip(b.iter()).map(|(p, q)| p * q).collect();
                Tabulated1D::lin_lin(e, y)
            };
            products.push(Product {
                particle: Particle::Neutron,
                emission_mode: EmissionMode::Delayed,
                yield_: Some(Function1D::Tabulated(y)),
            });
            let nr = px(t, idx + 1)? as usize;
            let ne = px(t, idx + 2 + 2 * nr)? as usize;
            idx += 3 + 2 * nr + 2 * ne;
        }
    }
    Ok((products, derived))
}

/// A continuous-energy library: the Rust stand-in for the `cross_sections.xml`
/// that `openmc.plot_xs(ce_cross_sections=...)` reads.
#[derive(Debug, Clone, Default)]
pub struct XsLibrary {
    nuclides: Vec<std::sync::Arc<IncidentNeutronData>>,
}

impl XsLibrary {
    /// An empty library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a nuclide. A second entry under the same name replaces the first.
    pub fn with_nuclide(mut self, data: IncidentNeutronData) -> Self {
        self.nuclides.retain(|n| n.name != data.name);
        self.nuclides.push(std::sync::Arc::new(data));
        self
    }

    /// Read an ACE file and add it.
    pub fn with_ace_file(self, path: impl AsRef<Path>) -> Result<Self, PlotError> {
        Ok(self.with_nuclide(IncidentNeutronData::from_ace_file(path, None)?))
    }

    /// `DataLibrary.get_by_material(name)`.
    pub fn get_by_material(&self, name: &str) -> Option<&IncidentNeutronData> {
        self.nuclides.iter().find(|n| n.name == name).map(|a| a.as_ref())
    }

    /// Names in the library, in insertion order.
    pub fn names(&self) -> Vec<&str> {
        self.nuclides.iter().map(|n| n.name.as_str()).collect()
    }
}
