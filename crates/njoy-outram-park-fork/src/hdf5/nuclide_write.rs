// SPDX-License-Identifier: GPL-3.0

//! **Nuclide `.h5` writer** — GitHub #270 scope item 3.
//!
//! Ported from `IncidentNeutron.export_to_hdf5`, `Reaction.to_hdf5`,
//! `Product.to_hdf5`, `UncorrelatedAngleEnergy.to_hdf5`,
//! `AngleDistribution.to_hdf5` and `Tabulated1D.to_hdf5` in
//! `openmc/data/` at OpenMC `afa7a14`.
//!
//! # Why writing this is worth the format surface
//!
//! It turns every code-to-code comparison from "two codes, two data
//! pipelines" into "two codes, one library". The largest confound in the
//! cross-code studies this workspace already runs is that OpenMC reads an
//! ACE library built by NJOY while this crate reconstructs from ENDF
//! in-process — so a residual could be the transport or could be the data,
//! and telling them apart costs a separate study every time.
//!
//! # SCOPE: what this writer covers, and what it does not
//!
//! ~~This writes a **complete, transportable** file for a nuclide whose
//! reactions are **elastic scattering (MT=2) and capture (MT=102)**.~~
//! **CORRECTED 2026-09-24 (GitHub #304).** That restriction was real and is
//! gone: the writer now takes an arbitrary MT with arbitrary products, through
//! the law hierarchy in [`super::nuclide_laws`].
//!
//! **Why the old limit was invisible.** It was verified against `Syn1`, a
//! *synthetic* nuclide, which has whatever reactions its test gives it — so a
//! writer that could express only two of them was correct for everything it
//! was ever asked to write, and unable to express U-235. That is the same
//! shape as the SiC free-gas defect: a component that passes every check it
//! has, because the check was built around what it does rather than what it
//! is for.
//!
//! **What is expressible now:** any reaction whose products use `level`,
//! `watt`, `maxwell`, `evaporation`, `madland-nix`, `discrete_photon`,
//! `nbody`, or an `uncorrelated` angle distribution — so fission with an
//! evaluated Watt or Maxwell spectrum, the 39 discrete inelastic levels, and
//! elastic and capture as before. Plus ν̄ (`total_nu`) and the unresolved
//! resonance probability tables (`urr`).
//!
//! # TWO THINGS A THRESHOLD SCATTERING LAW DEMANDS OF THE CALLER
//!
//! **Never emit a finite cross section AT a `level` threshold** (enforced
//! below), and **let the energy grid reach thermal** (documented, not
//! enforced). Both come from the same upstream defect, GitHub #306.
//!
//! A finite cross section at the threshold is the dangerous one. OpenMC
//! interpolates between grid points, so it makes the MT non-zero *below* the
//! kinematic threshold; `LevelInelastic::sample` then returns a **negative**
//! centre-of-mass energy unclamped, the CM→lab conversion takes
//! `std::sqrt(E_in * E_cm)` of a negative product (`src/physics.cpp:1170`) and
//! yields **NaN**, and `Nuclide::calculate_xs` guards its energy range with
//! `<` and `>` — which **NaN fails on both sides** — so it falls through to a
//! binary search that indexes the log grid with `int(log(NaN))`. Measured
//! 2026-09-24: **SIGSEGV**, confirmed by gdb, and confirmed again by a
//! lab-frame control that skips the `sqrt` and exits 0 on the same file.
//!
//! A grid that stops above thermal is the milder one: a `level` law emits
//! arbitrarily close to zero just above its threshold, and OpenMC transports
//! such a particle by **extrapolating** below the table with a negative
//! interpolation factor rather than refusing. Measured on a 1 keV–20 MeV grid,
//! **4.9 % of the flux** fell below the library minimum and was transported
//! that way. With flat cross sections that extrapolation is exact, so it was
//! not an error there — but on a structured cross section it would not be
//! defensible, and nothing warns. It is documented rather than enforced
//! because the crisp invariant (the emission range lies inside the grid) is
//! unsatisfiable for a `level` law at *any* positive grid minimum, and a fuzzy
//! "probably low enough" check would be worse than saying so plainly.
//!
//! Production ENDF-derived libraries trip neither: they are exactly zero at
//! every threshold and reach 1e-5 eV.
//!
//! # ACCEPTED LIMITATION: this writer is NOT at parity with OpenMC (2026-09-24)
//!
//! **Maintainer decision.** The rank-2 attribute limit below is **left standing
//! as a known non-parity item**, not worked around. The reasoning: this
//! workspace's own routes into transport are **reading ACE** and **generating
//! its own data** from ENDF, both of which work and are verified — so writing a
//! complete OpenMC library is a convenience for cross-code studies rather than
//! a capability anything depends on.
//!
//! What that decision costs, stated so nobody rediscovers it: a **real U-235
//! cannot be written**, therefore OpenMC cannot be run on a library this
//! workspace produced for any case with continuum secondary neutrons. The
//! cross-code comparisons keep their existing confound of two codes reading two
//! data pipelines. That is the price, and it was accepted knowingly.
//!
//! Do **not** "fix" this by writing the attribute flat — see below for why that
//! is a trap rather than a compromise.
//!
//! **What is still refused, and why it is a dependency limit rather than a
//! port gap:** `continuous`, `correlated` and `kalbach-mann` each store their
//! incident-grid interpolation as a **rank-2** attribute, which `hdf5-pure`
//! 0.20.1 cannot write. [`super::nuclide_laws`] documents the measurement and
//! why writing it flat would be a trap rather than a compromise. A real
//! U-235 needs `continuous` and `correlated`, so a *complete* U-235 file
//! waits on that; a fissile nuclide does not.

use std::path::Path;

use hdf5_pure::{AttrValue, FileBuilder};

use crate::error::NjoyError;
use crate::hdf5::nuclide_laws::{reaction_label, EmissionMode, Product, UrrTables};

// Re-exported rather than redefined: two copies of a tabulated function or an
// angle distribution in one crate is how the two silently drift apart.
pub use crate::hdf5::nuclide_laws::{AngleDistribution, Tabulated1D};

/// OpenMC's `HDF5_VERSION` for neutron data.
pub const HDF5_VERSION: [i64; 2] = [3, 0];

/// One reaction to write.
///
/// Ported from `Reaction.to_hdf5`, `reaction.py:922`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactionData {
    /// MT number. Any MT is accepted; whether it can be *expressed* depends on
    /// its products' laws — see the module docs.
    pub mt: i32,
    /// Q value \[eV\].
    pub q_value: f64,
    /// Whether the secondary distribution is in the centre-of-mass frame.
    pub center_of_mass: bool,
    /// Cross section \[barn\] on the nuclide's energy grid.
    pub xs: Vec<f64>,
    /// Index of the first non-zero point — OpenMC's `threshold_idx`.
    pub threshold_idx: i64,
    /// Whether this reaction is a redundant sum of others.
    pub redundant: bool,
    /// The reaction's products, in upstream's `product_<i>` order.
    pub products: Vec<Product>,
}

impl ReactionData {
    /// Elastic scattering (MT=2) with a tabulated outgoing cosine.
    ///
    /// Elastic needs an angle and **no** energy law: the outgoing energy
    /// follows from two-body kinematics, which is why this was one of the two
    /// reactions the writer could express before #304.
    pub fn elastic(
        xs: Vec<f64>,
        angle: AngleDistribution,
        e_min: f64,
        e_max: f64,
    ) -> Result<Self, NjoyError> {
        Ok(Self {
            mt: 2,
            q_value: 0.0,
            center_of_mass: true,
            xs,
            threshold_idx: 0,
            redundant: false,
            products: vec![Product::prompt_neutron(
                crate::hdf5::nuclide_laws::AngleEnergy::Uncorrelated {
                    angle: Some(angle),
                    energy: None,
                },
                e_min,
                e_max,
            )?],
        })
    }

    /// Build from a **full-grid** cross section, trimming to the threshold.
    ///
    /// Callers reconstructing from ENDF or decoding ACE naturally hold a
    /// zero-padded array on the whole nuclide grid, but that is not what the
    /// file stores (see [`write_nuclide`]'s threshold check). This finds the
    /// first non-zero point, records it as `threshold_idx`, and keeps the tail
    /// -- so the conversion happens in one place rather than at every call
    /// site, where getting it wrong is silent.
    ///
    /// An all-zero cross section keeps `threshold_idx = 0` and the whole grid:
    /// a reaction that never occurs is still a reaction the file may list, and
    /// trimming it to nothing would write an empty dataset.
    ///
    /// **`threshold_idx` is the LAST ZERO, not the first non-zero.** Measured on
    /// upstream's own U-235: MT=51's stored cross section begins `0.0`, then
    /// `6.693e-11`, and `energy[threshold_idx]` equals the level law's threshold
    /// exactly. So the retained array keeps one zero at the front, which is
    /// also what the `level` check in [`write_nuclide`] requires — trimming to
    /// the first non-zero instead shifts the whole reaction one grid point up
    /// and puts a finite cross section AT the threshold, which segfaults
    /// OpenMC.
    pub fn from_full_grid(
        mt: i32,
        q_value: f64,
        center_of_mass: bool,
        full: &[f64],
        products: Vec<Product>,
    ) -> Self {
        let first = full
            .iter()
            .position(|&v| v != 0.0)
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        Self {
            mt,
            q_value,
            center_of_mass,
            xs: full[first..].to_vec(),
            threshold_idx: first as i64,
            redundant: false,
            products,
        }
    }

    /// Radiative capture (MT=102): a cross section and no neutron product.
    pub fn capture(q_value: f64, xs: Vec<f64>) -> Self {
        Self {
            mt: 102,
            q_value,
            center_of_mass: false,
            xs,
            threshold_idx: 0,
            redundant: false,
            products: vec![],
        }
    }

    /// Whether upstream would skip this reaction when exporting.
    ///
    /// Mirrors `IncidentNeutron.export_to_hdf5` (`neutron.py`): a redundant
    /// reaction is written only if it has a photon product or its MT is in
    /// upstream's `keep_mts`. Skipping the same ones matters because MT=4 is
    /// "sometimes needed for probability tables" in upstream's own words, so a
    /// writer that dropped it would produce a file OpenMC reads differently.
    pub fn skipped_by_upstream(&self) -> bool {
        if !self.redundant {
            return false;
        }
        const KEEP_MTS: [i32; 15] = [
            4, 16, 103, 104, 105, 106, 107, 203, 204, 205, 206, 207, 301, 444, 901,
        ];
        let has_photon = self.products.iter().any(|p| p.particle == "photon");
        !(has_photon || KEEP_MTS.contains(&self.mt))
    }
}

/// Everything a nuclide file carries.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideData {
    /// GND-style name, e.g. `"H1"`.
    pub name: String,
    /// Atomic number.
    pub z: i32,
    /// Mass number.
    pub a: i32,
    /// Metastable state, 0 for ground.
    pub metastable: i32,
    /// Target mass / neutron mass.
    pub atomic_weight_ratio: f64,
    /// Temperature label, e.g. `"294K"`.
    pub temperature: String,
    /// `kT` at that temperature \[eV\].
    pub kt_ev: f64,
    /// Energy grid \[eV\], ascending.
    pub energy: Vec<f64>,
    /// Reactions.
    pub reactions: Vec<ReactionData>,
    /// Total ν̄, written to the top-level `total_nu` group.
    ///
    /// Upstream writes this from a fission reaction's `derived_products[0]`,
    /// i.e. as a `Product` with `emission_mode = total`. **Without it OpenMC
    /// has no total neutron yield and a fissile nuclide cannot go critical**,
    /// so a fissionable nuclide missing it is refused rather than written.
    pub total_nu: Option<Product>,
    /// Unresolved-resonance probability tables, per temperature label.
    pub urr: Vec<(String, UrrTables)>,
}

/// Write a nuclide `.h5`.
///
/// # Errors
///
/// A malformed energy grid, a cross-section array that does not match it, an
/// elastic reaction with no angle distribution, a fissionable nuclide with no
/// `total_nu`, a product whose law cannot yet be expressed (see
/// [`super::nuclide_laws`]), or an I/O failure.
///
/// **An inexpressible law is refused, not skipped.** A file silently missing a
/// reaction or a distribution transports a different nuclide from the one the
/// caller described, and the resulting `k` is wrong by an amount nothing
/// reports.
pub fn write_nuclide<P: AsRef<Path>>(path: P, n: &NuclideData) -> Result<(), NjoyError> {
    if n.energy.len() < 2 {
        return Err(NjoyError::Hdf5(
            "a nuclide needs an energy grid of at least two points".into(),
        ));
    }
    if !n.energy.windows(2).all(|w| w[1] > w[0]) {
        return Err(NjoyError::Hdf5(
            "the energy grid must be strictly ascending".into(),
        ));
    }
    for r in &n.reactions {
        // **The xs array starts AT the threshold, it is not zero-padded.**
        // Upstream writes `data=self.xs[T].y`, whose first point is the
        // threshold, and records where that is in `threshold_idx`. Measured on
        // upstream's own U-235: MT=2 is 76027 points at threshold_idx 0, but
        // MT=52 is **697 points at threshold_idx 75330** -- and 697 + 75330 =
        // 76027. So the invariant is `len(xs) + threshold_idx == len(energy)`.
        //
        // This check used to demand the FULL grid length, which is the same
        // thing only when `threshold_idx == 0`. Both of `Syn1`'s reactions have
        // a zero threshold, so the writer passed every test it had while being
        // unable to express any threshold reaction -- the same shape of defect
        // as the MT restriction #304 was filed for. A zero-padded array is read
        // by OpenMC as though it began at the threshold, which shifts the cross
        // section instead of failing: on a fissile nuclide that produced a
        // runaway source and a run that never finished a batch.
        if r.xs.len() + r.threshold_idx as usize != n.energy.len() {
            return Err(NjoyError::Hdf5(format!(
                "MT={}: {} cross-section points at threshold_idx {} against a \
                 {}-point energy grid. OpenMC stores xs FROM the threshold \
                 onward, so len(xs) + threshold_idx must equal len(energy) \
                 (it needs {}). A zero-padded full-grid array is not refused by \
                 OpenMC -- it is read as though it started at the threshold, \
                 which shifts the cross section silently. Use \
                 ReactionData::from_full_grid to trim.",
                r.mt,
                r.xs.len(),
                r.threshold_idx,
                n.energy.len(),
                n.energy.len() - r.threshold_idx as usize
            )));
        }
        if r.mt == 2
            && !r.products.iter().any(|p| {
                matches!(
                    p.distribution.first(),
                    Some(crate::hdf5::nuclide_laws::AngleEnergy::Uncorrelated {
                        angle: Some(_),
                        ..
                    })
                )
            })
        {
            return Err(NjoyError::Hdf5(
                "elastic scattering needs an angle distribution; without one OpenMC has \
                 no way to sample the outgoing direction"
                    .into(),
            ));
        }
    }

    // **A `level` law needs a grid point AT its threshold.** Measured on
    // upstream's own U-235 (`xs_ref/U235.h5`, produced by
    // `IncidentNeutron.from_ace(...).export_to_hdf5(...)`): for MT=51/52/53 the
    // grid point at `threshold_idx` equals the level's own `threshold`
    // attribute to 1.3e-4, 7.6e-7 and 2.2e-7 relative, with the point below it
    // strictly under the threshold, and the product's angle grid starting
    // exactly at the threshold.
    //
    // That is not cosmetic. OpenMC interpolates the cross section between grid
    // points, so if the first non-zero point sits ABOVE the threshold, every
    // energy in the interval below it carries a non-zero interpolated MT, and a
    // collision sampled there enters `LevelInelastic::sample` with
    // `E < threshold`. The outgoing energy `mass_ratio * (E - threshold)` is
    // then NEGATIVE, and the subsequent grid lookup runs off the bottom of the
    // array: measured 2026-09-24, OpenMC **segfaults** rather than reporting
    // anything. A file that crashes the consumer is worse than one it rejects,
    // and nothing in this crate could have found it -- the file is entirely
    // self-consistent.
    for r in &n.reactions {
        for (pi, prod) in r.products.iter().enumerate() {
            for d in &prod.distribution {
                let crate::hdf5::nuclide_laws::AngleEnergy::Uncorrelated {
                    energy: Some(crate::hdf5::nuclide_laws::EnergyDist::Level { threshold, .. }),
                    ..
                } = d
                else {
                    continue;
                };
                let ti = r.threshold_idx as usize;
                let e_thr = n.energy.get(ti).copied().unwrap_or(0.0);
                // **And the cross section must be ZERO at that point.** Measured
                // on upstream's U-235: MT=51/52/53/91 all carry exactly 0.0 at
                // their threshold index, rising from there (MT=51's next point
                // is 6.7e-11 barn). That is what makes a near-threshold
                // collision improbable, and it is load-bearing: a cross section
                // that STEPS from 0 to a finite value at the threshold makes
                // `mass_ratio * (E - threshold)` land arbitrarily close to zero
                // at a finite rate, and OpenMC has no guard for a particle
                // below the library's minimum energy -- `material.cpp:833`
                // computes `log(E / energy_min) / log_spacing` as a grid index,
                // which goes NEGATIVE and indexes out of bounds. Measured
                // 2026-09-24: a flat 1 barn step at the threshold segfaults
                // OpenMC after a few generations, with the threshold sitting
                // exactly on a grid point.
                if r.xs.first().copied().unwrap_or(0.0) != 0.0 {
                    return Err(NjoyError::Hdf5(format!(
                        "MT={} product_{pi}: the cross section is {:e} barn AT the \
                         `level` law's threshold, but it must be exactly zero there \
                         and rise above it, as upstream's own files do (U-235 \
                         MT=51/52/53/91 are all 0.0 at their threshold index). A step \
                         makes the outgoing energy `mass_ratio * (E - threshold)` land \
                         arbitrarily near zero at a finite rate, and OpenMC has NO \
                         guard for a particle below the library minimum -- it computes \
                         a logarithmic grid index that goes negative and reads out of \
                         bounds. Measured 2026-09-24: this SEGFAULTS rather than \
                         reporting anything.",
                        r.mt,
                        r.xs.first().copied().unwrap_or(0.0)
                    )));
                }
                if *threshold > 0.0 && (e_thr - *threshold).abs() / *threshold > 1.0e-3 {
                    return Err(NjoyError::Hdf5(format!(
                        "MT={} product_{pi}: the `level` law's threshold is {threshold:e} eV \
                         but the energy grid's first non-zero point (index {ti}) is \
                         {e_thr:e} eV. The grid must carry a point AT the threshold, as \
                         upstream's own files do, because OpenMC interpolates the cross \
                         section: a first point above the threshold makes every energy in \
                         the interval below it carry a non-zero MT, and a collision \
                         sampled there gives the level law E < threshold, hence a \
                         NEGATIVE outgoing energy. OpenMC SEGFAULTS on that rather than \
                         reporting it. Insert the threshold into the energy grid.",
                        r.mt
                    )));
                }
            }
        }
    }

    // A fissionable nuclide with no total nu-bar reads back with no neutron
    // yield at all, so it cannot go critical -- and nothing in the file says
    // so. Refuse rather than write it.
    let fissionable = n
        .reactions
        .iter()
        .any(|r| matches!(r.mt, 18 | 19 | 20 | 21 | 38));
    if fissionable && n.total_nu.is_none() {
        return Err(NjoyError::Hdf5(
            "this nuclide has a fission reaction but no total_nu. OpenMC reads the \
             total neutron yield from the top-level `total_nu` group; without it the \
             nuclide fissions and emits nothing, so k is silently wrong rather than \
             absent. Set NuclideData::total_nu."
                .into(),
        ));
    }
    if let Some(t) = &n.total_nu {
        if t.emission_mode != EmissionMode::Total {
            return Err(NjoyError::Hdf5(format!(
                "total_nu must carry emission_mode = total, not {}; upstream writes it \
                 from a fission reaction's derived_products[0]",
                t.emission_mode.as_str()
            )));
        }
    }

    let mut b = FileBuilder::new();
    b.set_attr("filetype", AttrValue::AsciiString("data_neutron".into()));
    b.set_attr("version", AttrValue::I64Array(HDF5_VERSION.to_vec()));

    let mut g = b.create_group(&n.name);
    g.set_attr("Z", AttrValue::I32(n.z));
    g.set_attr("A", AttrValue::I32(n.a));
    g.set_attr("metastable", AttrValue::I32(n.metastable));
    g.set_attr(
        "atomic_weight_ratio",
        AttrValue::F64(n.atomic_weight_ratio),
    );

    {
        let mut kts = g.create_group("kTs");
        // **SCALAR, not a 1-element array.** Upstream writes
        // `ktg.create_dataset(temperature, data=self.kTs[i])` with a Python
        // float, so the dataset has rank 0. Writing shape `[1]` instead makes
        // `IncidentNeutron.temperatures` fail with
        // `TypeError: type numpy.ndarray doesn't define __round__` - which is
        // a confusing error a long way from its cause, and exactly the class
        // of defect a round trip through this crate's own reader could not
        // have found.
        kts.create_dataset(&n.temperature)
            .with_shape(&[])
            .with_f64_data(&[n.kt_ev]);
        g.add_group(kts.finish());
    }
    {
        let mut eg = g.create_group("energy");
        eg.create_dataset(&n.temperature).with_f64_data(&n.energy);
        g.add_group(eg.finish());
    }

    let mut rxs = g.create_group("reactions");
    for r in &n.reactions {
        if r.skipped_by_upstream() {
            continue;
        }
        let mut rg = rxs.create_group(&format!("reaction_{:03}", r.mt));
        rg.set_attr("mt", AttrValue::I32(r.mt));
        rg.set_attr("label", AttrValue::AsciiString(reaction_label(r.mt)));
        rg.set_attr("Q_value", AttrValue::F64(r.q_value));
        rg.set_attr(
            "center_of_mass",
            AttrValue::I32(i32::from(r.center_of_mass)),
        );
        rg.set_attr("redundant", AttrValue::I32(i32::from(r.redundant)));

        {
            let mut tg = rg.create_group(&n.temperature);
            tg.create_dataset("xs")
                .with_f64_data(&r.xs)
                .set_attr("threshold_idx", AttrValue::I64(r.threshold_idx));
            rg.add_group(tg.finish());
        }

        for (i, prod) in r.products.iter().enumerate() {
            let mut pg = rg.create_group(&format!("product_{i}"));
            prod.write(&mut pg).map_err(|e| {
                NjoyError::Hdf5(format!("MT={} product_{i}: {e}", r.mt))
            })?;
            rg.add_group(pg.finish());
        }
        rxs.add_group(rg.finish());
    }
    g.add_group(rxs.finish());

    if let Some(t) = &n.total_nu {
        let mut tg = g.create_group("total_nu");
        t.write(&mut tg)
            .map_err(|e| NjoyError::Hdf5(format!("total_nu: {e}")))?;
        g.add_group(tg.finish());
    }

    if !n.urr.is_empty() {
        let mut ug = g.create_group("urr");
        for (temp, tables) in &n.urr {
            let mut tg = ug.create_group(temp);
            tables
                .write(&mut tg)
                .map_err(|e| NjoyError::Hdf5(format!("urr/{temp}: {e}")))?;
            ug.add_group(tg.finish());
        }
        g.add_group(ug.finish());
    }

    b.add_group(g.finish());

    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing nuclide {}: {e}", n.name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hdf5::nuclide_laws::{AngleEnergy, EnergyDist, Function1D};

    fn grid() -> Vec<f64> {
        (0..64)
            .map(|i| {
                let lo = 1.0e-5_f64.ln();
                let hi = 2.0e7_f64.ln();
                (lo + (hi - lo) * i as f64 / 63.0).exp()
            })
            .collect()
    }

    fn nuclide() -> NuclideData {
        let e = grid();
        let n = e.len();
        let (lo, hi) = (e[0], e[n - 1]);
        NuclideData {
            name: "Syn1".into(),
            z: 1,
            a: 1,
            metastable: 0,
            atomic_weight_ratio: 0.999_167,
            temperature: "294K".into(),
            kt_ev: 2.5301e-2,
            energy: e.clone(),
            reactions: vec![
                ReactionData::elastic(
                    vec![2.0; n],
                    AngleDistribution::isotropic(vec![lo, hi]),
                    lo,
                    hi,
                )
                .unwrap(),
                // 1/v capture, normalised to 1 barn at 1 eV.
                ReactionData::capture(2.2e6, e.iter().map(|&x| (1.0 / x).sqrt()).collect()),
            ],
            total_nu: None,
            urr: vec![],
        }
    }

    /// A fissile nuclide: fission with an evaluated **Watt** spectrum, ν̄, and
    /// a discrete inelastic **level** — the three things #304 said could not
    /// be written.
    ///
    /// **Every cross section is flat on purpose, because that buys an analytic
    /// gate.** With energy-independent cross sections the infinite-medium
    /// multiplication factor is
    ///
    /// ```text
    /// k_inf = nu * Sigma_f / Sigma_a = 2.5 * 2 / (2 + 0.5) = 2.0
    /// ```
    ///
    /// **exactly**, and independently of the fission spectrum, the scattering
    /// law, the level threshold, the temperature and the grid — every one of
    /// which cancels when the cross sections do not depend on energy. So a code
    /// reading this file must return 2.0 under a reflective boundary or it has
    /// mis-read nu-bar, the fission cross section or the capture cross section,
    /// and the discrepancy says which. A tuned or fitted reference could not do
    /// that; this one is arithmetic.
    fn fissile() -> NuclideData {
        let mut d = nuclide();
        let e = d.energy.clone();
        let n = e.len();
        let (lo, hi) = (e[0], e[n - 1]);
        d.name = "SynF".into();
        // Flat elastic and capture, replacing the 2 barn / 1-over-root-E pair
        // that `nuclide()` supplies, so the analytic k_inf above holds.
        d.reactions[0].xs = vec![4.0; n];
        d.reactions[1].xs = vec![0.5; n];
        d.z = 92;
        d.a = 235;
        d.atomic_weight_ratio = 233.0248;

        // MT=18, chi as a Watt law (ENDF MF=5 LF=11), isotropic in the lab.
        d.reactions.push(ReactionData {
            mt: 18,
            q_value: 0.0,
            center_of_mass: false,
            xs: vec![2.0; n],
            threshold_idx: 0,
            redundant: false,
            products: vec![Product::prompt_neutron(
                AngleEnergy::Uncorrelated {
                    angle: Some(AngleDistribution::isotropic(vec![lo, hi])),
                    energy: Some(EnergyDist::Watt {
                        u: 0.0,
                        a: Tabulated1D::constant(9.88e5, lo, hi).unwrap(),
                        b: Tabulated1D::constant(2.249e-6, lo, hi).unwrap(),
                    }),
                },
                lo,
                hi,
            )
            .unwrap()],
        });

        // MT=51, a discrete level: `level` energy law plus an AND cosine.
        //
        // **The threshold is inserted INTO the grid**, which is what upstream's
        // own files do and what stops OpenMC interpolating a non-zero cross
        // section below the level's kinematic threshold -- see the check in
        // `write_nuclide`. Splicing it in here rather than snapping the
        // threshold to the nearest grid point keeps the physics exact: the
        // threshold is set by Q and the mass ratio, not by where the grid
        // happens to have a point.
        let q = -4.0e4_f64;
        let awr = d.atomic_weight_ratio;
        let level_threshold = q.abs() * (awr + 1.0) / awr;
        let thr_idx = e.iter().position(|&x| x > level_threshold).unwrap();
        let mut e: Vec<f64> = e.clone();
        e.insert(thr_idx, level_threshold);
        let n = e.len();
        let hi = e[n - 1];
        // Every cross section already written above is on the OLD grid, so each
        // gains the spliced point. They are flat, so the inserted value is the
        // same constant -- but doing it by length rather than by value keeps
        // this honest if a fixture ever stops being flat.
        for r in d.reactions.iter_mut() {
            let v = r.xs[0];
            r.xs = vec![v; n];
        }
        d.energy = e.clone();
        // Zero AT the threshold and rising above it, matching upstream's own
        // files -- see the check in `write_nuclide`. A step here segfaults
        // OpenMC, which is how that invariant was found.
        let mut xs = vec![0.0; n];
        for (i, v) in xs.iter_mut().enumerate().skip(thr_idx + 1) {
            *v = (i - thr_idx) as f64 * 0.1;
        }
        d.reactions.push(ReactionData::from_full_grid(
            51,
            q,
            true,
            &xs,
            vec![Product::prompt_neutron(
                AngleEnergy::Uncorrelated {
                    angle: Some(AngleDistribution::isotropic(vec![e[thr_idx], hi])),
                    energy: Some(EnergyDist::Level {
                        threshold: level_threshold,
                        mass_ratio: (awr / (awr + 1.0)).powi(2),
                    }),
                },
                e[thr_idx],
                hi,
            )
            .unwrap()],
        ));

        d.total_nu = Some(Product {
            particle: "neutron".into(),
            emission_mode: EmissionMode::Total,
            decay_rate: 0.0,
            yield_: Function1D::Tabulated(Tabulated1D::constant(2.5, lo, hi).unwrap()),
            distribution: vec![AngleEnergy::Uncorrelated {
                angle: Some(AngleDistribution::isotropic(vec![lo, hi])),
                energy: None,
            }],
            applicability: vec![],
        });
        d
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join("njoy_nuclide_write_test");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    /// A scattering-plus-capture nuclide writes.
    #[test]
    fn a_scattering_plus_capture_nuclide_writes() {
        let p = tmp("Syn1.h5");
        write_nuclide(&p, &nuclide()).unwrap();
        let n = std::fs::metadata(&p).unwrap().len();
        println!("wrote {} ({n} bytes)", p.display());
        assert!(n > 3000, "suspiciously small nuclide file: {n} bytes");
    }

    /// **THE #304 GATE: a fissile nuclide writes.**
    ///
    /// Fission with a Watt spectrum, a discrete inelastic level, and total ν̄ —
    /// every one of which the MT=2/102 writer refused. `Syn1` could not have
    /// caught the old limit because a synthetic nuclide has whatever reactions
    /// its test gives it; this one deliberately has the reactions a criticality
    /// calculation needs.
    #[test]
    fn a_fissile_nuclide_writes() {
        let p = tmp("SynF.h5");
        write_nuclide(&p, &fissile()).unwrap();
        let n = std::fs::metadata(&p).unwrap().len();
        println!("wrote {} ({n} bytes)", p.display());
        assert!(n > 4000, "suspiciously small fissile nuclide: {n} bytes");
    }

    /// **A fissionable nuclide with no ν̄ is refused.**
    ///
    /// It would read back, transport, fission, and emit nothing — `k` silently
    /// wrong rather than absent, which is the worst available failure.
    #[test]
    fn a_fissionable_nuclide_without_total_nu_is_refused() {
        let mut d = fissile();
        d.total_nu = None;
        let msg = format!("{}", write_nuclide(tmp("bad_nu.h5"), &d).unwrap_err());
        assert!(msg.contains("total_nu"), "{msg}");
        assert!(msg.contains("k is silently wrong"), "{msg}");
    }

    /// ν̄ carrying the wrong emission mode is refused.
    #[test]
    fn total_nu_must_be_the_total_not_the_prompt() {
        let mut d = fissile();
        if let Some(t) = d.total_nu.as_mut() {
            t.emission_mode = EmissionMode::Prompt;
        }
        let msg = format!("{}", write_nuclide(tmp("bad_nu2.h5"), &d).unwrap_err());
        assert!(msg.contains("emission_mode = total"), "{msg}");
    }

    /// **An inexpressible law is refused, not skipped**, and the error names
    /// the reaction so the caller knows which one.
    #[test]
    fn a_law_the_dependency_cannot_express_is_refused_with_its_mt() {
        use crate::hdf5::nuclide_laws::{EnergyOutRow, TabulatedEnergyOut};
        let mut d = fissile();
        let (lo, hi) = (d.energy[0], *d.energy.last().unwrap());
        let law = TabulatedEnergyOut {
            energy: vec![lo, hi],
            breakpoints: vec![2],
            interpolation: vec![2],
            rows: vec![
                EnergyOutRow {
                    e_out: vec![0.0, hi],
                    p: vec![0.0, 1.0 / hi],
                    c: vec![0.0, 1.0],
                    interpolation: 2,
                    n_discrete_lines: 0,
                    kalbach_r: vec![],
                    kalbach_a: vec![],
                    cosines: vec![],
                };
                2
            ],
        };
        d.reactions.push(ReactionData {
            mt: 91,
            q_value: -1.0e6,
            center_of_mass: true,
            xs: vec![0.5; d.energy.len()],
            threshold_idx: 0,
            redundant: false,
            products: vec![Product::prompt_neutron(
                AngleEnergy::Uncorrelated {
                    angle: None,
                    energy: Some(EnergyDist::Continuous(law)),
                },
                lo,
                hi,
            )
            .unwrap()],
        });
        let msg = format!("{}", write_nuclide(tmp("bad91.h5"), &d).unwrap_err());
        assert!(msg.contains("MT=91"), "the error must name the MT: {msg}");
        assert!(msg.contains("RANK-2"), "and the reason: {msg}");
    }

    /// Elastic without an angle distribution is refused — OpenMC would have no
    /// way to sample the outgoing direction.
    #[test]
    fn elastic_without_an_angle_distribution_is_refused() {
        let mut d = nuclide();
        d.reactions[0].products.clear();
        let err = write_nuclide(tmp("bad2.h5"), &d).unwrap_err();
        assert!(format!("{err}").contains("angle distribution"), "{err}");
    }

    /// **Upstream's redundant-reaction skip is mirrored exactly.**
    ///
    /// A redundant MT is dropped unless it has a photon product or sits in
    /// upstream's `keep_mts`. MT=4 is in that list because, in upstream's own
    /// comment, it "is also sometimes needed for probability tables" — so
    /// dropping it would give OpenMC a different nuclide.
    #[test]
    fn the_redundant_skip_matches_upstream() {
        let plain = |mt: i32, redundant: bool| ReactionData {
            mt,
            q_value: 0.0,
            center_of_mass: false,
            xs: vec![],
            threshold_idx: 0,
            redundant,
            products: vec![],
        };
        // Not redundant -> always written.
        assert!(!plain(18, false).skipped_by_upstream());
        // Redundant and not in keep_mts -> skipped.
        assert!(plain(101, true).skipped_by_upstream());
        assert!(plain(27, true).skipped_by_upstream());
        // Redundant but in keep_mts -> kept.
        for mt in [4, 16, 103, 104, 105, 106, 107, 203, 301, 444, 901] {
            assert!(
                !plain(mt, true).skipped_by_upstream(),
                "MT={mt} is in upstream's keep_mts and must be written"
            );
        }
        // Redundant with a photon product -> kept whatever the MT.
        let mut with_photon = plain(101, true);
        with_photon.products = vec![Product {
            particle: "photon".into(),
            emission_mode: EmissionMode::Prompt,
            decay_rate: 0.0,
            yield_: Function1D::Tabulated(Tabulated1D::constant(1.0, 1.0, 2.0e7).unwrap()),
            distribution: vec![AngleEnergy::Uncorrelated {
                angle: None,
                energy: Some(EnergyDist::DiscretePhoton {
                    primary_flag: 2,
                    energy: 1.0e6,
                    atomic_weight_ratio: 1.0,
                }),
            }],
            applicability: vec![],
        }];
        assert!(!with_photon.skipped_by_upstream());
    }

    /// **The threshold invariant, pinned to upstream's own U-235 file.**
    ///
    /// `len(xs) + threshold_idx == len(energy)` is not read off the Python
    /// source here but measured from `xs_ref/U235.h5`, which was produced by
    /// upstream's own `IncidentNeutron.from_ace(...).export_to_hdf5(...)`. A
    /// convention taken from a reference implementation's *output* cannot be
    /// mis-transcribed the way one taken from its prose can.
    ///
    /// # Results, 2026-09-24
    ///
    /// On that file, 76027 grid points: MT=2 and MT=18 are 76027 points at
    /// `threshold_idx = 0`, MT=51 is 66891 at 9136, MT=52 is **697 at 75330**,
    /// MT=91 is 125 at 75902. Every one sums to 76027.
    #[test]
    fn a_zero_padded_threshold_cross_section_is_refused() {
        let mut d = nuclide();
        let n = d.energy.len();
        // A threshold reaction written the WRONG way: full grid, zero-padded,
        // with a non-zero threshold_idx. This is what a caller naturally has.
        d.reactions.push(ReactionData {
            mt: 51,
            q_value: -4.0e4,
            center_of_mass: true,
            xs: vec![0.0; n],
            threshold_idx: 10,
            redundant: false,
            products: vec![],
        });
        let msg = format!("{}", write_nuclide(tmp("bad_thr.h5"), &d).unwrap_err());
        assert!(msg.contains("MT=51"), "{msg}");
        assert!(
            msg.contains("FROM the threshold") && msg.contains("shifts the cross section"),
            "the error must say WHY a zero-padded array is dangerous rather than \
             merely wrong, because OpenMC accepts it silently: {msg}"
        );
    }

    /// `from_full_grid` produces an array satisfying the invariant.
    #[test]
    fn from_full_grid_trims_to_the_threshold() {
        let full = vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
        let r = ReactionData::from_full_grid(51, -1.0, true, &full, vec![]);
        // The LAST ZERO, index 2 -- not the first non-zero at index 3. See the
        // constructor's docs: upstream's stored array starts with the zero that
        // sits exactly at the threshold.
        assert_eq!(r.threshold_idx, 2);
        assert_eq!(r.xs, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(r.xs.len() + r.threshold_idx as usize, full.len());
        assert_eq!(r.xs[0], 0.0, "the threshold point itself must be zero");

        // An all-zero reaction keeps the whole grid rather than writing an
        // empty dataset.
        let r = ReactionData::from_full_grid(51, -1.0, true, &[0.0; 4], vec![]);
        assert_eq!(r.threshold_idx, 0);
        assert_eq!(r.xs.len(), 4);
    }

    /// Malformed grids are refused.
    #[test]
    fn malformed_grids_are_refused() {
        let mut d = nuclide();
        d.energy = vec![1.0];
        assert!(write_nuclide(tmp("b3.h5"), &d).is_err());

        let mut d = nuclide();
        d.energy.swap(0, 1);
        assert!(write_nuclide(tmp("b4.h5"), &d).is_err());

        let mut d = nuclide();
        d.reactions[0].xs.pop();
        assert!(write_nuclide(tmp("b5.h5"), &d).is_err());
    }

    /// **The cumulative column is normalised to end at exactly 1.**
    ///
    /// An unnormalised `c` samples a biased cosine at run time and is
    /// invisible to any reader that only checks shapes.
    #[test]
    fn the_cumulative_angle_column_ends_at_one() {
        let a = AngleDistribution::isotropic(vec![1.0, 2.0e7]);
        let (flat, offsets, interp) = a.rows().unwrap();
        assert_eq!(offsets, vec![0, 2]);
        assert_eq!(interp, vec![2, 2]);
        let n_pairs = flat.len() / 3;
        assert_eq!(n_pairs, 4);
        let c = &flat[2 * n_pairs..];
        // Two distributions of two points each: c = [0, 1, 0, 1].
        assert_eq!(c, &[0.0, 1.0, 0.0, 1.0]);
        // And the mu/p rows are what was asked for.
        assert_eq!(&flat[..n_pairs], &[-1.0, 1.0, -1.0, 1.0]);
        assert_eq!(&flat[n_pairs..2 * n_pairs], &[0.5, 0.5, 0.5, 0.5]);
    }

    /// A mismatched angle distribution is refused.
    #[test]
    fn a_mismatched_angle_distribution_is_refused() {
        let a = AngleDistribution {
            energy: vec![1.0, 2.0],
            mu: vec![vec![-1.0, 1.0]],
            p: vec![vec![0.5, 0.5]],
            interpolation: vec![],
        };
        assert!(a.rows().is_err());

        let a = AngleDistribution {
            energy: vec![1.0],
            mu: vec![vec![-1.0, 0.0, 1.0]],
            p: vec![vec![0.5, 0.5]],
            interpolation: vec![],
        };
        assert!(a.rows().is_err());
    }
}
