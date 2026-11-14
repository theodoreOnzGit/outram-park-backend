// SPDX-License-Identifier: GPL-3.0

//! **Reading an OpenMC neutron `.h5` back into this crate** — GitHub #303.
//!
//! Before this, `outram-mc-libs` could obtain nuclear data only by
//! reconstructing it from ENDF tapes in process. That is a different task from
//! deserialising a pre-built library, and the 2026-09-24 LCT-008 timing sweep
//! measured the gap it leaves:
//!
//! | | nuclear data | transport |
//! |---|---|---|
//! | OpenMC + its ENDF/B-VIII.0 HDF5 library | 1.54 ± 0.06 s | 7.06 ± 0.05 s |
//! | outram-mc + rust-njoy from ENDF tapes | 158.57 ± 10.80 s | 59.52 ± 3.22 s |
//!
//! The 103× on nuclear data is **not a slowdown** — it is reconstruction
//! against deserialisation. The point is that the *comparable* measurement did
//! not exist, because our side could not start from a pre-built library. This
//! module is that missing half.
//!
//! # Upstream is the specification, and its OUTPUT is the oracle
//!
//! The layout is `IncidentNeutron.export_to_hdf5` at OpenMC `afa7a14`, and
//! [`super::nuclide_write`] is its mirror on the write side. Where the two
//! could disagree, the arbiter used here is not the Python source but a file
//! upstream itself produced: `reference-data/ace` → NJOY2016 ACE →
//! `openmc.data.IncidentNeutron.from_ace(...).export_to_hdf5(...)`. A
//! convention taken from a reference implementation's output cannot be
//! mis-transcribed the way one taken from its prose can — which is how the
//! `threshold_idx` convention was caught on the write side (see
//! [`super::nuclide_write::write_nuclide`]).
//!
//! # The rank-2 attribute is a WRITE limit only
//!
//! [`super::nuclide_laws`] documents why `continuous`, `correlated` and
//! `kalbach-mann` cannot currently be *written*: their incident-grid
//! interpolation is a rank-2 attribute and `hdf5-pure` 0.20.1 writes only
//! rank-1. **Reading them is unaffected.** `Group::attrs` hands back an
//! `AttrValue` without a shape, and the layout `(2, NR)` is row-major with
//! known extents, so splitting the flat array in half recovers
//! `(breakpoints, interpolation)` unambiguously. So this reader covers every
//! law, including the three the writer refuses.

use std::collections::BTreeMap;
use std::path::Path;

use hdf5_pure::{AttrValue, File};

use crate::error::NjoyError;
use crate::hdf5::nuclide_laws::{AngleDistribution, EnergyDist, Function1D, Polynomial, Tabulated1D};

/// A reaction as read back from a nuclide file.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadReaction {
    /// ENDF MT number.
    pub mt: i32,
    /// `Q` value \[eV\].
    pub q_value: f64,
    /// Whether the secondary distribution is in the centre-of-mass frame.
    pub center_of_mass: bool,
    /// Whether this reaction is a redundant sum of others.
    pub redundant: bool,
    /// Cross section \[barn\], **from the threshold onward** as the file stores
    /// it. Use [`Self::xs_on_grid`] for a zero-filled full-grid copy.
    pub xs: Vec<f64>,
    /// Index into the nuclide energy grid where `xs` begins.
    pub threshold_idx: usize,
    /// The `type` attribute of each product's first distribution, in
    /// `product_<i>` order — enough to report law coverage without decoding
    /// every table.
    pub product_laws: Vec<String>,
    /// Each product's particle name.
    pub product_particles: Vec<String>,
    /// Each product decoded as far as this reader supports (GitHub #303):
    /// yield, emission mode and the **uncorrelated** laws (`level`, `watt`,
    /// `maxwell`, `evaporation`, `madland-nix`, `discrete_photon`) with their
    /// angle distributions. Every other law is carried as
    /// [`ReadDistribution::Undecoded`], naming its type, so a consumer can
    /// refuse it by name rather than sample something else.
    pub products: Vec<ReadProduct>,
}

impl ReadReaction {
    /// The cross section on the full nuclide grid, zero below the threshold.
    ///
    /// The file does **not** store it this way — see the module docs — so this
    /// is the conversion a consumer wants, in one place.
    pub fn xs_on_grid(&self, n_energy: usize) -> Vec<f64> {
        let mut out = vec![0.0; n_energy];
        let end = (self.threshold_idx + self.xs.len()).min(n_energy);
        out[self.threshold_idx..end].copy_from_slice(&self.xs[..end - self.threshold_idx]);
        out
    }
}

/// A nuclide read back from an OpenMC neutron `.h5`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadNuclide {
    /// GND-style name, e.g. `"U235"`.
    pub name: String,
    /// Atomic number.
    pub z: i32,
    /// Mass number.
    pub a: i32,
    /// Metastable state.
    pub metastable: i32,
    /// Target mass / neutron mass.
    pub atomic_weight_ratio: f64,
    /// `kT` \[eV\] per temperature label.
    pub kts: BTreeMap<String, f64>,
    /// Energy grid \[eV\] per temperature label.
    pub energy: BTreeMap<String, Vec<f64>>,
    /// Reactions by MT.
    pub reactions: BTreeMap<i32, ReadReaction>,
    /// Total ν̄ as `(x, y)` if the file carries `total_nu`, plus its `type`.
    pub total_nu: Option<(String, Vec<f64>, Vec<f64>)>,
    /// `(breakpoint, interpolation)` regions of a tabulated `total_nu`, as the
    /// file states them (1-based breakpoints, ENDF interpolation codes). Empty
    /// for a polynomial or when the file carries none.
    pub total_nu_regions: Vec<(i64, i64)>,
    /// Temperature labels carrying URR probability tables.
    pub urr_temperatures: Vec<String>,
}

impl ReadNuclide {
    /// Whether this nuclide has a fission channel.
    pub fn is_fissionable(&self) -> bool {
        self.reactions
            .keys()
            .any(|mt| matches!(mt, 18 | 19 | 20 | 21 | 38))
    }

    /// Total ν̄ interpolated lin-lin at `e` \[eV\], or `None` without `total_nu`.
    ///
    /// Clamped at both ends, as OpenMC's `Tabulated1D` evaluation is.
    pub fn nu_total(&self, e: f64) -> Option<f64> {
        let (kind, x, y) = self.total_nu.as_ref()?;
        if kind == "Polynomial" {
            // Horner on the coefficients, which is what upstream's
            // `Polynomial` does; `x` holds them in that case.
            return Some(x.iter().rev().fold(0.0, |acc, c| acc * e + c));
        }
        if x.is_empty() {
            return None;
        }
        if e <= x[0] {
            return Some(y[0]);
        }
        if e >= *x.last().unwrap() {
            return Some(*y.last().unwrap());
        }
        let i = x.partition_point(|&v| v <= e) - 1;
        let f = (e - x[i]) / (x[i + 1] - x[i]);
        Some(y[i] + f * (y[i + 1] - y[i]))
    }

    /// The energy grid for the single temperature this file carries, or the
    /// first in label order when it carries several.
    pub fn any_energy(&self) -> Option<&Vec<f64>> {
        self.energy.values().next()
    }
}

fn e(m: String) -> NjoyError {
    NjoyError::Hdf5(m)
}

fn attr_f64(attrs: &BTreeMap<String, AttrValue>, k: &str) -> Option<f64> {
    match attrs.get(k)? {
        AttrValue::F64(v) => Some(*v),
        AttrValue::F64Array(v) => v.first().copied(),
        AttrValue::I32(v) => Some(f64::from(*v)),
        AttrValue::I64(v) => Some(*v as f64),
        _ => None,
    }
}

fn attr_i64(attrs: &BTreeMap<String, AttrValue>, k: &str) -> Option<i64> {
    match attrs.get(k)? {
        AttrValue::I32(v) => Some(i64::from(*v)),
        AttrValue::I64(v) => Some(*v),
        AttrValue::U32(v) => Some(i64::from(*v)),
        AttrValue::U64(v) => Some(*v as i64),
        AttrValue::I64Array(v) => v.first().copied(),
        AttrValue::F64(v) => Some(*v as i64),
        _ => None,
    }
}

fn attr_str(attrs: &BTreeMap<String, AttrValue>, k: &str) -> Option<String> {
    match attrs.get(k)? {
        AttrValue::AsciiString(s) | AttrValue::String(s) => {
            Some(s.trim_end_matches('\0').to_string())
        }
        AttrValue::AsciiStringArray(v) | AttrValue::StringArray(v) | AttrValue::VarLenAsciiArray(v) => {
            v.first().map(|s| s.trim_end_matches('\0').to_string())
        }
        _ => None,
    }
}

fn sorted(m: std::collections::HashMap<String, AttrValue>) -> BTreeMap<String, AttrValue> {
    m.into_iter().collect()
}

/// A product as read back: see [`ReadReaction::products`].
#[derive(Debug, Clone, PartialEq)]
pub struct ReadProduct {
    /// `"neutron"`, `"photon"`, ...
    pub particle: String,
    /// `"prompt"`, `"delayed"` or `"total"`.
    pub emission_mode: String,
    /// Precursor decay constant \[1/s\], 0 when the file has none.
    pub decay_rate: f64,
    /// The yield as a function of incident energy, when the file has one.
    pub yield_fn: Option<Function1D>,
    /// The distributions, in `distribution_<i>` order.
    pub distributions: Vec<ReadDistribution>,
    /// Per-distribution applicability `p_k(E)`; empty when the file has none.
    pub applicability: Vec<Tabulated1D>,
}

/// One secondary distribution as read back.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadDistribution {
    /// `uncorrelated`: an angle part and/or an energy part. Either may be
    /// absent (elastic carries an angle only, an isotropic continuum an energy
    /// only).
    Uncorrelated {
        /// Outgoing cosine, independent of the outgoing energy.
        angle: Option<AngleDistribution>,
        /// The cumulative row the file stores beside each density, one vector
        /// per incident energy, aligned with `angle.mu`. Kept as stored, not
        /// rebuilt, so a consumer samples the file's own distribution.
        angle_cdf: Vec<Vec<f64>>,
        /// Outgoing energy, for the laws this reader decodes.
        energy: Option<EnergyDist>,
    },
    /// Anything this reader does not unpack into samplable form: `correlated`,
    /// `kalbach-mann`, `nbody`, or an uncorrelated law with a `continuous`
    /// energy part. Holds the `type` string (and, for an uncorrelated law, the
    /// energy part's type after a colon) so a refusal can name it.
    Undecoded(String),
}

fn attr_vec_i64(a: &BTreeMap<String, AttrValue>, k: &str) -> Vec<i64> {
    match a.get(k) {
        Some(AttrValue::I64Array(v)) => v.clone(),
        Some(AttrValue::I64(v)) => vec![*v],
        Some(AttrValue::I32(v)) => vec![i64::from(*v)],
        _ => Vec::new(),
    }
}

fn attr_regions(a: &BTreeMap<String, AttrValue>) -> Vec<(i64, i64)> {
    attr_vec_i64(a, "breakpoints")
        .into_iter()
        .zip(attr_vec_i64(a, "interpolation"))
        .collect()
}

/// A `Tabulated1D` or `Polynomial` dataset `name` in group `path`, if present.
fn read_function(f: &File, path: &str, name: &str) -> Result<Option<Function1D>, NjoyError> {
    let g = f.group(path).map_err(|x| e(format!("{path}: {x}")))?;
    let Ok(d) = g.dataset(name) else {
        return Ok(None);
    };
    let raw = d
        .read_f64()
        .map_err(|x| e(format!("{path}/{name}: {x}")))?;
    let da = sorted(d.attrs().unwrap_or_default());
    let kind = attr_str(&da, "type").unwrap_or_else(|| "Tabulated1D".into());
    if kind == "Polynomial" {
        return Ok(Some(Function1D::Polynomial(Polynomial { coef: raw })));
    }
    // Stored as vstack([x, y]), so the first half is x.
    let half = raw.len() / 2;
    Ok(Some(Function1D::Tabulated(Tabulated1D {
        x: raw[..half].to_vec(),
        y: raw[half..].to_vec(),
        breakpoints: attr_vec_i64(&da, "breakpoints"),
        interpolation: attr_vec_i64(&da, "interpolation"),
    })))
}

fn read_tabulated(f: &File, path: &str, name: &str) -> Result<Option<Tabulated1D>, NjoyError> {
    match read_function(f, path, name)? {
        Some(Function1D::Tabulated(t)) => Ok(Some(t)),
        Some(Function1D::Polynomial(_)) => Err(e(format!(
            "{path}/{name} is a Polynomial where a Tabulated1D is required"
        ))),
        None => Ok(None),
    }
}

fn need_tabulated(f: &File, path: &str, name: &str) -> Result<Tabulated1D, NjoyError> {
    read_tabulated(f, path, name)?.ok_or_else(|| e(format!("{path} has no '{name}' dataset")))
}

/// An `angle` group: `energy`, and `mu` of shape `(3, n_pairs)` holding the
/// cosine, density and cumulative rows, split per incident energy by the
/// `offsets` attribute. The cumulative row is returned beside the distribution
/// as stored.
fn read_angle(f: &File, path: &str) -> Result<(AngleDistribution, Vec<Vec<f64>>), NjoyError> {
    let g = f.group(path).map_err(|x| e(format!("{path}: {x}")))?;
    let energy = g
        .dataset("energy")
        .and_then(|d| d.read_f64())
        .map_err(|x| e(format!("{path}/energy: {x}")))?;
    let d = g.dataset("mu").map_err(|x| e(format!("{path}/mu: {x}")))?;
    let flat = d.read_f64().map_err(|x| e(format!("{path}/mu: {x}")))?;
    let da = sorted(d.attrs().unwrap_or_default());
    let offsets = attr_vec_i64(&da, "offsets");
    let interpolation = attr_vec_i64(&da, "interpolation");
    let n_pairs = flat.len() / 3;
    if flat.len() % 3 != 0 || offsets.len() != energy.len() {
        return Err(e(format!(
            "{path}: {} energies but {} offsets over {} values; an angle table is \
             (3, n_pairs)",
            energy.len(),
            offsets.len(),
            flat.len()
        )));
    }
    let (mu_row, rest) = flat.split_at(n_pairs);
    let (p_row, c_row) = rest.split_at(n_pairs);
    let mut mu = Vec::with_capacity(energy.len());
    let mut p = Vec::with_capacity(energy.len());
    let mut cdf = Vec::with_capacity(energy.len());
    for i in 0..energy.len() {
        let lo = offsets[i] as usize;
        let hi = offsets.get(i + 1).map_or(n_pairs, |&o| o as usize);
        if lo > hi || hi > n_pairs {
            return Err(e(format!("{path}: offsets are not ascending within the table")));
        }
        mu.push(mu_row[lo..hi].to_vec());
        p.push(p_row[lo..hi].to_vec());
        cdf.push(c_row[lo..hi].to_vec());
    }
    Ok((
        AngleDistribution {
            energy,
            mu,
            p,
            interpolation,
        },
        cdf,
    ))
}

/// The `energy` group of an uncorrelated distribution, for the laws this
/// reader decodes; the inner `Err(type)` carries the type of one it does not.
fn read_energy(f: &File, path: &str) -> Result<Result<EnergyDist, String>, NjoyError> {
    let g = f.group(path).map_err(|x| e(format!("{path}: {x}")))?;
    let ga = sorted(g.attrs().unwrap_or_default());
    let kind = attr_str(&ga, "type").unwrap_or_else(|| "unknown".into());
    let u = attr_f64(&ga, "u").unwrap_or(0.0);
    Ok(Ok(match kind.as_str() {
        "maxwell" => EnergyDist::Maxwell {
            u,
            theta: need_tabulated(f, path, "theta")?,
        },
        "evaporation" => EnergyDist::Evaporation {
            u,
            theta: need_tabulated(f, path, "theta")?,
        },
        "watt" => EnergyDist::Watt {
            u,
            a: need_tabulated(f, path, "a")?,
            b: need_tabulated(f, path, "b")?,
        },
        "madland-nix" => EnergyDist::MadlandNix {
            efl: attr_f64(&ga, "efl").unwrap_or(0.0),
            efh: attr_f64(&ga, "efh").unwrap_or(0.0),
            tm: need_tabulated(f, path, "xy")?,
        },
        "discrete_photon" => EnergyDist::DiscretePhoton {
            primary_flag: attr_i64(&ga, "primary_flag").unwrap_or(0) as i32,
            energy: attr_f64(&ga, "energy").unwrap_or(0.0),
            atomic_weight_ratio: attr_f64(&ga, "atomic_weight_ratio").unwrap_or(0.0),
        },
        "level" => EnergyDist::Level {
            threshold: attr_f64(&ga, "threshold").unwrap_or(0.0),
            mass_ratio: attr_f64(&ga, "mass_ratio").unwrap_or(0.0),
        },
        other => return Ok(Err(other.to_string())),
    }))
}

fn read_product(f: &File, path: &str) -> Result<ReadProduct, NjoyError> {
    let pg = f.group(path).map_err(|x| e(format!("{path}: {x}")))?;
    let pa = sorted(pg.attrs().unwrap_or_default());
    let n_dist = attr_i64(&pa, "n_distribution").unwrap_or(0).max(0) as usize;
    let mut distributions = Vec::with_capacity(n_dist);
    let mut applicability = Vec::new();
    for k in 0..n_dist {
        let dpath = format!("{path}/distribution_{k}");
        let dg = f.group(&dpath).map_err(|x| e(format!("{dpath}: {x}")))?;
        let da = sorted(dg.attrs().unwrap_or_default());
        let kind = attr_str(&da, "type").unwrap_or_else(|| "unknown".into());
        if let Some(t) = read_tabulated(f, &dpath, "applicability")? {
            applicability.push(t);
        }
        if kind != "uncorrelated" {
            distributions.push(ReadDistribution::Undecoded(kind));
            continue;
        }
        let groups = dg.groups().unwrap_or_default();
        let (angle, angle_cdf) = if groups.iter().any(|g| g == "angle") {
            let (a, c) = read_angle(f, &format!("{dpath}/angle"))?;
            (Some(a), c)
        } else {
            (None, Vec::new())
        };
        let energy = if groups.iter().any(|g| g == "energy") {
            match read_energy(f, &format!("{dpath}/energy"))? {
                Ok(d) => Some(d),
                Err(t) => {
                    distributions.push(ReadDistribution::Undecoded(format!("uncorrelated:{t}")));
                    continue;
                }
            }
        } else {
            None
        };
        distributions.push(ReadDistribution::Uncorrelated {
            angle,
            angle_cdf,
            energy,
        });
    }
    Ok(ReadProduct {
        particle: attr_str(&pa, "particle").unwrap_or_else(|| "unknown".into()),
        emission_mode: attr_str(&pa, "emission_mode").unwrap_or_else(|| "prompt".into()),
        decay_rate: attr_f64(&pa, "decay_rate").unwrap_or(0.0),
        yield_fn: read_function(f, path, "yield")?,
        distributions,
        applicability,
    })
}

/// Read an OpenMC neutron `.h5`.
///
/// # Errors
///
/// A file that is not `data_neutron`, a missing top-level nuclide group, a
/// missing `energy` or `kTs` group, or an I/O failure. A reaction whose
/// `threshold_idx` and cross-section extent contradict the energy grid is
/// **refused**, because that is the one defect in this format that a reader
/// cannot detect later: the numbers all parse and the cross section is simply
/// in the wrong place.
pub fn read_nuclide<P: AsRef<Path>>(path: P) -> Result<ReadNuclide, NjoyError> {
    let bytes = std::fs::read(path.as_ref())?;
    let f = File::from_bytes(bytes).map_err(|x| e(format!("open: {x}")))?;

    let root_attrs = sorted(f.root().attrs().map_err(|x| e(format!("root attrs: {x}")))?);
    if let Some(ft) = attr_str(&root_attrs, "filetype") {
        if ft != "data_neutron" {
            return Err(e(format!(
                "this is a '{ft}' file, not 'data_neutron'. Reading a thermal, \
                 photon or multigroup library through the neutron path would \
                 produce numbers rather than an error."
            )));
        }
    }

    let name = f
        .root()
        .groups()
        .map_err(|x| e(format!("list groups: {x}")))?
        .into_iter()
        .next()
        .ok_or_else(|| e("no nuclide group in the file".into()))?;

    let g = f
        .group(&format!("/{name}"))
        .map_err(|x| e(format!("group /{name}: {x}")))?;
    let ga = sorted(g.attrs().map_err(|x| e(format!("{name} attrs: {x}")))?);

    let mut kts = BTreeMap::new();
    if let Ok(kg) = f.group(&format!("/{name}/kTs")) {
        for ds in kg.datasets().map_err(|x| e(format!("kTs: {x}")))? {
            let v = kg
                .dataset(&ds)
                .and_then(|d| d.read_f64())
                .map_err(|x| e(format!("kTs/{ds}: {x}")))?;
            if let Some(&kt) = v.first() {
                kts.insert(ds, kt);
            }
        }
    }

    let mut energy = BTreeMap::new();
    let eg = f
        .group(&format!("/{name}/energy"))
        .map_err(|x| e(format!("energy group: {x}")))?;
    for ds in eg.datasets().map_err(|x| e(format!("energy: {x}")))? {
        let v = eg
            .dataset(&ds)
            .and_then(|d| d.read_f64())
            .map_err(|x| e(format!("energy/{ds}: {x}")))?;
        energy.insert(ds, v);
    }
    if energy.is_empty() {
        return Err(e(format!("{name} carries no energy grid")));
    }
    let n_energy = energy.values().next().unwrap().len();

    let mut reactions = BTreeMap::new();
    if let Ok(rxs) = f.group(&format!("/{name}/reactions")) {
        for rname in rxs.groups().map_err(|x| e(format!("reactions: {x}")))? {
            let path = format!("/{name}/reactions/{rname}");
            let rg = f.group(&path).map_err(|x| e(format!("{path}: {x}")))?;
            let ra = sorted(rg.attrs().map_err(|x| e(format!("{path} attrs: {x}")))?);
            let mt = attr_i64(&ra, "mt")
                .ok_or_else(|| e(format!("{path} has no mt attribute")))? as i32;

            // The cross section lives under a temperature sub-group.
            let mut xs = Vec::new();
            let mut threshold_idx = 0usize;
            for t in rg.groups().map_err(|x| e(format!("{path}: {x}")))? {
                if t.starts_with("product_") {
                    continue;
                }
                let tp = format!("{path}/{t}");
                if let Ok(tg) = f.group(&tp) {
                    if let Ok(d) = tg.dataset("xs") {
                        xs = d.read_f64().map_err(|x| e(format!("{tp}/xs: {x}")))?;
                        let da = sorted(d.attrs().unwrap_or_default());
                        threshold_idx = attr_i64(&da, "threshold_idx").unwrap_or(0).max(0) as usize;
                        break;
                    }
                }
            }
            if !xs.is_empty() && threshold_idx + xs.len() != n_energy {
                return Err(e(format!(
                    "MT={mt}: {} cross-section points at threshold_idx {threshold_idx} \
                     against a {n_energy}-point grid. OpenMC stores xs FROM the \
                     threshold, so the two must sum to the grid length; they sum to \
                     {}. This is the one corruption a reader cannot notice later -- \
                     every number parses and the cross section is simply in the wrong \
                     place.",
                    xs.len(),
                    threshold_idx + xs.len()
                )));
            }

            let mut product_laws = Vec::new();
            let mut product_particles = Vec::new();
            let mut products = Vec::new();
            let mut i = 0usize;
            while let Ok(pg) = f.group(&format!("{path}/product_{i}")) {
                let pa = sorted(pg.attrs().unwrap_or_default());
                product_particles
                    .push(attr_str(&pa, "particle").unwrap_or_else(|| "unknown".into()));
                let law = f
                    .group(&format!("{path}/product_{i}/distribution_0"))
                    .ok()
                    .and_then(|dg| sorted(dg.attrs().unwrap_or_default()).get("type").cloned())
                    .and_then(|v| match v {
                        AttrValue::AsciiString(s) | AttrValue::String(s) => {
                            Some(s.trim_end_matches('\0').to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "none".into());
                product_laws.push(law);
                products.push(read_product(&f, &format!("{path}/product_{i}"))?);
                i += 1;
            }

            reactions.insert(
                mt,
                ReadReaction {
                    mt,
                    q_value: attr_f64(&ra, "Q_value").unwrap_or(0.0),
                    center_of_mass: attr_i64(&ra, "center_of_mass").unwrap_or(0) != 0,
                    redundant: attr_i64(&ra, "redundant").unwrap_or(0) != 0,
                    xs,
                    threshold_idx,
                    product_laws,
                    product_particles,
                    products,
                },
            );
        }
    }

    // total_nu is a Product group whose `yield` dataset carries nu-bar.
    let mut total_nu = None;
    let mut total_nu_regions = Vec::new();
    if let Ok(tg) = f.group(&format!("/{name}/total_nu")) {
        if let Ok(d) = tg.dataset("yield") {
            let raw = d.read_f64().map_err(|x| e(format!("total_nu/yield: {x}")))?;
            let da = sorted(d.attrs().unwrap_or_default());
            let kind = attr_str(&da, "type").unwrap_or_else(|| "Tabulated1D".into());
            if kind == "Polynomial" {
                total_nu = Some((kind, raw, vec![]));
            } else {
                // Stored as vstack([x, y]), so the first half is x.
                let half = raw.len() / 2;
                total_nu_regions = attr_regions(&da);
                total_nu = Some((kind, raw[..half].to_vec(), raw[half..].to_vec()));
            }
        }
    }

    let urr_temperatures = f
        .group(&format!("/{name}/urr"))
        .ok()
        .and_then(|ug| ug.groups().ok())
        .unwrap_or_default();

    Ok(ReadNuclide {
        name: name.clone(),
        z: attr_i64(&ga, "Z").unwrap_or(0) as i32,
        a: attr_i64(&ga, "A").unwrap_or(0) as i32,
        metastable: attr_i64(&ga, "metastable").unwrap_or(0) as i32,
        atomic_weight_ratio: attr_f64(&ga, "atomic_weight_ratio").unwrap_or(0.0),
        kts,
        energy,
        reactions,
        total_nu,
        total_nu_regions,
        urr_temperatures,
    })
}

#[cfg(test)]
mod tests {
    use petir::mathf::RealMath;
    use super::*;

    /// **Round trip: what this crate writes, this crate reads back.**
    ///
    /// Necessary but *not* sufficient, and the module docs say why: a round
    /// trip proves two halves of one codec agree and cannot detect a format
    /// that is self-consistent and wrong. The cross-code tests in
    /// `tests/nuclide_h5_vs_openmc.rs` are the ones that can.
    #[test]
    fn a_file_this_crate_wrote_reads_back() {
        use crate::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};
        use crate::hdf5::nuclide_laws::AngleDistribution;

        let energy: Vec<f64> = (0..32)
            .map(|i| {
                let (lo, hi) = (1.0e-5_f64.r_ln(), 2.0e7_f64.r_ln());
                (lo + (hi - lo) * i as f64 / 31.0).r_exp()
            })
            .collect();
        let n = energy.len();
        let (lo, hi) = (energy[0], energy[n - 1]);
        let d = NuclideData {
            name: "RtN1".into(),
            z: 1,
            a: 1,
            metastable: 0,
            atomic_weight_ratio: 0.999_167,
            temperature: "294K".into(),
            kt_ev: 2.5301e-2,
            energy: energy.clone(),
            reactions: vec![
                ReactionData::elastic(
                    vec![3.0; n],
                    AngleDistribution::isotropic(vec![lo, hi]),
                    lo,
                    hi,
                )
                .unwrap(),
                ReactionData::capture(1.0e6, vec![0.25; n]),
            ],
            total_nu: None,
            urr: vec![],
        };
        let p = std::env::temp_dir().join("njoy_nuclide_read_rt.h5");
        write_nuclide(&p, &d).unwrap();

        let r = read_nuclide(&p).unwrap();
        assert_eq!(r.name, "RtN1");
        assert_eq!(r.z, 1);
        assert_eq!(r.a, 1);
        assert!((r.atomic_weight_ratio - 0.999_167).abs() < 1e-12);
        assert_eq!(r.kts.len(), 1);
        assert!((r.kts["294K"] - 2.5301e-2).abs() < 1e-12);
        assert_eq!(r.any_energy().unwrap().len(), n);
        assert_eq!(r.reactions.len(), 2);
        assert_eq!(r.reactions[&2].xs, vec![3.0; n]);
        assert_eq!(r.reactions[&2].threshold_idx, 0);
        assert!(r.reactions[&2].center_of_mass);
        assert_eq!(r.reactions[&2].product_laws, vec!["uncorrelated"]);
        assert_eq!(r.reactions[&102].xs, vec![0.25; n]);
        assert!(!r.reactions[&102].center_of_mass);
        assert!((r.reactions[&102].q_value - 1.0e6).abs() < 1.0);
        assert!(!r.is_fissionable());
        assert!(r.total_nu.is_none());
    }

    /// A threshold reaction round-trips with its threshold intact, and
    /// `xs_on_grid` puts it back where a consumer expects.
    #[test]
    fn a_threshold_reaction_round_trips_and_refills_the_grid() {
        use crate::hdf5::nuclide_laws::AngleDistribution;
        use crate::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};

        let energy: Vec<f64> = (1..=16).map(|i| f64::from(i) * 1.0e6).collect();
        let n = energy.len();
        let mut full = vec![0.0; n];
        for (i, v) in full.iter_mut().enumerate().skip(5) {
            *v = 1.0 + i as f64;
        }
        let d = NuclideData {
            name: "RtN2".into(),
            z: 92,
            a: 238,
            metastable: 0,
            atomic_weight_ratio: 236.0,
            temperature: "294K".into(),
            kt_ev: 2.5301e-2,
            energy: energy.clone(),
            reactions: vec![
                ReactionData::elastic(
                    vec![1.0; n],
                    AngleDistribution::isotropic(vec![energy[0], energy[n - 1]]),
                    energy[0],
                    energy[n - 1],
                )
                .unwrap(),
                ReactionData::from_full_grid(16, -6.0e6, true, &full, vec![]),
            ],
            total_nu: None,
            urr: vec![],
        };
        let p = std::env::temp_dir().join("njoy_nuclide_read_thr.h5");
        write_nuclide(&p, &d).unwrap();

        let r = read_nuclide(&p).unwrap();
        let rx = &r.reactions[&16];
        // 4, the LAST ZERO before the rise at index 5 -- see
        // `ReactionData::from_full_grid`, which mirrors upstream's convention.
        assert_eq!(rx.threshold_idx, 4, "the threshold must survive the trip");
        assert_eq!(rx.xs.len(), n - 4);
        assert_eq!(rx.xs[0], 0.0, "the stored array begins at the threshold zero");
        assert_eq!(rx.xs_on_grid(n), full, "refilling must restore the original");
    }

    /// A non-neutron file is refused rather than parsed into numbers.
    #[test]
    fn a_file_of_the_wrong_type_is_refused() {
        use hdf5_pure::{AttrValue, FileBuilder};
        let mut b = FileBuilder::new();
        b.set_attr("filetype", AttrValue::AsciiString("mgxs".into()));
        let mut g = b.create_group("Wrong");
        g.create_dataset("x").with_f64_data(&[1.0]);
        b.add_group(g.finish());
        let p = std::env::temp_dir().join("njoy_nuclide_read_wrong.h5");
        b.write(&p).unwrap();
        let msg = format!("{}", read_nuclide(&p).unwrap_err());
        assert!(msg.contains("not 'data_neutron'"), "{msg}");
        assert!(msg.contains("produce numbers rather than an error"), "{msg}");
    }

    /// ν̄ interpolates lin-lin and clamps at both ends.
    #[test]
    fn nu_total_interpolates_and_clamps() {
        let n = ReadNuclide {
            name: "X".into(),
            z: 92,
            a: 235,
            metastable: 0,
            atomic_weight_ratio: 233.0,
            kts: BTreeMap::new(),
            energy: BTreeMap::new(),
            reactions: BTreeMap::new(),
            total_nu: Some((
                "Tabulated1D".into(),
                vec![1.0e-5, 1.0e6, 2.0e7],
                vec![2.4, 2.6, 4.0],
            )),
            total_nu_regions: vec![],
            urr_temperatures: vec![],
        };
        assert!((n.nu_total(1.0e-5).unwrap() - 2.4).abs() < 1e-12);
        assert!((n.nu_total(1.0e-9).unwrap() - 2.4).abs() < 1e-12, "clamped low");
        assert!((n.nu_total(1.0e8).unwrap() - 4.0).abs() < 1e-12, "clamped high");
        // Halfway in energy between 1e6 and 2e7 is not halfway in nu.
        let mid = n.nu_total(0.5 * (1.0e6 + 2.0e7)).unwrap();
        assert!((mid - 3.3).abs() < 1e-9, "got {mid}");

        // A polynomial nu-bar is Horner on its coefficients.
        let p = ReadNuclide {
            total_nu: Some(("Polynomial".into(), vec![2.0, 3.0, 4.0], vec![])),
            ..n
        };
        // 2 + 3E + 4E^2 at E = 2
        assert!((p.nu_total(2.0).unwrap() - 24.0).abs() < 1e-12);
    }

    /// **Products decode as the laws that were written**: the elastic angle
    /// table (with its stored cumulative row), a `level` law, a Watt spectrum,
    /// and the `total_nu` interpolation regions (GitHub #303).
    #[test]
    fn products_read_back_as_the_laws_that_were_written() {
        use crate::hdf5::nuclide_laws::{
            AngleEnergy, EmissionMode, EnergyDist, Function1D, Product,
        };
        use crate::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};

        let energy = vec![1.0e-5, 1.0e6, 2.0e6, 2.0e7];
        let (lo, hi) = (energy[0], energy[3]);
        let angle = AngleDistribution {
            energy: vec![lo, hi],
            mu: vec![vec![-1.0, 0.0, 1.0], vec![-1.0, 1.0]],
            p: vec![vec![0.25, 0.5, 0.75], vec![0.5, 0.5]],
            interpolation: vec![2, 2],
        };
        let watt = Product::prompt_neutron(
            AngleEnergy::Uncorrelated {
                angle: None,
                energy: Some(EnergyDist::Watt {
                    u: -2.0e7,
                    a: Tabulated1D::constant(0.988e6, lo, hi).unwrap(),
                    b: Tabulated1D::constant(2.249e-6, lo, hi).unwrap(),
                }),
            },
            lo,
            hi,
        )
        .unwrap();
        let nu = Product {
            particle: "neutron".into(),
            emission_mode: EmissionMode::Total,
            decay_rate: 0.0,
            yield_: Function1D::Tabulated(
                Tabulated1D::lin_lin(vec![lo, hi], vec![2.0, 4.0]).unwrap(),
            ),
            distribution: vec![AngleEnergy::Uncorrelated {
                angle: Some(AngleDistribution::isotropic(vec![lo, hi])),
                energy: None,
            }],
            applicability: vec![],
        };
        let d = NuclideData {
            name: "RtN3".into(),
            z: 92,
            a: 233,
            metastable: 0,
            atomic_weight_ratio: 233.0,
            temperature: "294K".into(),
            kt_ev: 2.5e-2,
            energy: energy.clone(),
            reactions: vec![
                ReactionData::elastic(vec![5.0; 4], angle.clone(), lo, hi).unwrap(),
                ReactionData::capture(6.0e6, vec![1.0; 4]),
                ReactionData {
                    mt: 18,
                    q_value: 2.0e8,
                    center_of_mass: false,
                    xs: vec![2.0; 4],
                    threshold_idx: 0,
                    redundant: false,
                    products: vec![watt],
                },
            ],
            total_nu: Some(nu),
            urr: vec![],
        };
        let p = std::env::temp_dir().join("njoy_nuclide_read_products.h5");
        write_nuclide(&p, &d).unwrap();
        let r = read_nuclide(&p).unwrap();
        let _ = std::fs::remove_file(&p);

        let el = &r.reactions[&2].products[0];
        assert_eq!(el.particle, "neutron");
        let ReadDistribution::Uncorrelated {
            angle: Some(a),
            angle_cdf,
            energy: None,
        } = &el.distributions[0]
        else {
            panic!("elastic must read back as an angle-only uncorrelated law");
        };
        assert_eq!(a, &angle);
        // The stored cumulative row: trapezoid of the density, normalised to 1.
        assert_eq!(angle_cdf[0].first().copied(), Some(0.0));
        assert!((angle_cdf[0].last().unwrap() - 1.0).abs() < 1e-12);

        let ReadDistribution::Uncorrelated {
            energy: Some(EnergyDist::Watt { u, a, .. }),
            ..
        } = &r.reactions[&18].products[0].distributions[0]
        else {
            panic!("fission must read back as a Watt law");
        };
        assert_eq!(*u, -2.0e7);
        assert_eq!(a.y, vec![0.988e6, 0.988e6]);
        assert_eq!(r.total_nu_regions, vec![(2, 2)]);
        assert!(r.reactions[&102].products.is_empty());
    }
}
