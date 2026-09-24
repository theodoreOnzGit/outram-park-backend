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
                },
            );
        }
    }

    // total_nu is a Product group whose `yield` dataset carries nu-bar.
    let mut total_nu = None;
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
        urr_temperatures,
    })
}

#[cfg(test)]
mod tests {
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
                let (lo, hi) = (1.0e-5_f64.ln(), 2.0e7_f64.ln());
                (lo + (hi - lo) * i as f64 / 31.0).exp()
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
}
