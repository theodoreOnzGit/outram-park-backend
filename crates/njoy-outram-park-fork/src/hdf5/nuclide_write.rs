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
//! This writes a **complete, transportable** file for a nuclide whose
//! reactions are **elastic scattering (MT=2) and capture (MT=102)**. That is
//! not the whole format, and the boundary is drawn where it is for a reason:
//!
//! - **Capture needs no product distribution at all** — the neutron is gone.
//! - **Elastic needs an angle distribution and no energy distribution**,
//!   because it is two-body and the outgoing energy follows from kinematics.
//!   So it is expressible with `UncorrelatedAngleEnergy` carrying `angle`
//!   only.
//! - **Fission, inelastic and (n,xn) need correlated energy-angle
//!   distributions** — `KalbachMann`, `CorrelatedAngleEnergy`,
//!   `NBodyPhaseSpace` and the rest of `openmc/data/`'s law hierarchy. Those
//!   are **not** written, and [`write_nuclide`] refuses an MT it cannot
//!   express rather than emitting a file that is missing the distribution
//!   OpenMC will then fail to find with a confusing error.
//!
//! A scattering-plus-capture nuclide is enough for a genuine cross-code
//! transport comparison on a fixed-source or subcritical problem, which is
//! what #270's acceptance asks for. It is not enough for an eigenvalue
//! problem, and this module says so rather than letting that be discovered.

use std::path::Path;

use hdf5_pure::{AttrValue, FileBuilder};

use crate::error::NjoyError;

/// OpenMC's `HDF5_VERSION` for neutron data.
pub const HDF5_VERSION: [i64; 2] = [3, 0];

/// A tabulated `(x, y)` function, written as OpenMC's `Tabulated1D`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabulated1D {
    /// Abscissae.
    pub x: Vec<f64>,
    /// Ordinates.
    pub y: Vec<f64>,
    /// Interpolation-region breakpoints (1-based indices into `x`).
    pub breakpoints: Vec<i64>,
    /// ENDF interpolation code per region (2 = lin-lin).
    pub interpolation: Vec<i64>,
}

impl Tabulated1D {
    /// A single lin-lin region over the whole grid — the common case.
    pub fn lin_lin(x: Vec<f64>, y: Vec<f64>) -> Result<Self, NjoyError> {
        if x.len() != y.len() {
            return Err(NjoyError::Hdf5(format!(
                "a tabulated function needs matching x and y; got {} and {}",
                x.len(),
                y.len()
            )));
        }
        if x.len() < 2 {
            return Err(NjoyError::Hdf5(
                "a tabulated function needs at least two points".into(),
            ));
        }
        let n = x.len() as i64;
        Ok(Self {
            x,
            y,
            breakpoints: vec![n],
            interpolation: vec![2],
        })
    }

    /// A constant, as the two-point function OpenMC expects for a yield.
    pub fn constant(value: f64, e_min: f64, e_max: f64) -> Result<Self, NjoyError> {
        Self::lin_lin(vec![e_min, e_max], vec![value, value])
    }
}

/// A nuclide's elastic angle distribution: one tabulated `p(mu)` per incident
/// energy.
#[derive(Debug, Clone, PartialEq)]
pub struct AngleDistribution {
    /// Incident energies \[eV\], ascending.
    pub energy: Vec<f64>,
    /// Per incident energy, the cosine grid. Each inner vector is one
    /// distribution's `mu` values.
    pub mu: Vec<Vec<f64>>,
    /// Per incident energy, the density at each `mu`.
    pub p: Vec<Vec<f64>>,
}

impl AngleDistribution {
    /// Isotropic at every energy in `energy` — `p(mu) = 1/2` on `[-1, 1]`.
    pub fn isotropic(energy: Vec<f64>) -> Self {
        let n = energy.len();
        Self {
            energy,
            mu: vec![vec![-1.0, 1.0]; n],
            p: vec![vec![0.5, 0.5]; n],
        }
    }

    /// The `(mu, p, c)` triple rows OpenMC stores, plus the per-energy offsets.
    ///
    /// `c` is the **cumulative** distribution, which OpenMC computes on the
    /// Python side and stores. Writing it wrong is invisible to a reader that
    /// only checks shapes and produces a biased angular distribution at run
    /// time, so it is built here by trapezoid from `p` rather than left to a
    /// caller.
    fn rows(&self) -> Result<(Vec<f64>, Vec<i64>, Vec<i64>), NjoyError> {
        if self.mu.len() != self.energy.len() || self.p.len() != self.energy.len() {
            return Err(NjoyError::Hdf5(format!(
                "angle distribution has {} energies but {} mu grids and {} densities",
                self.energy.len(),
                self.mu.len(),
                self.p.len()
            )));
        }
        let n_pairs: usize = self.mu.iter().map(Vec::len).sum();
        let mut mu_row = Vec::with_capacity(n_pairs);
        let mut p_row = Vec::with_capacity(n_pairs);
        let mut c_row = Vec::with_capacity(n_pairs);
        let mut offsets = Vec::with_capacity(self.energy.len());
        let interpolation = vec![2i64; self.energy.len()];
        let mut j = 0i64;
        for (i, mu) in self.mu.iter().enumerate() {
            let p = &self.p[i];
            if p.len() != mu.len() {
                return Err(NjoyError::Hdf5(format!(
                    "energy {i}: {} cosines but {} densities",
                    mu.len(),
                    p.len()
                )));
            }
            offsets.push(j);
            let mut c = 0.0_f64;
            for k in 0..mu.len() {
                if k > 0 {
                    c += 0.5 * (p[k] + p[k - 1]) * (mu[k] - mu[k - 1]);
                }
                mu_row.push(mu[k]);
                p_row.push(p[k]);
                c_row.push(c);
            }
            // Normalise so the cumulative ends at exactly 1; an unnormalised
            // `c` samples a biased cosine and nothing checks it.
            if c > 0.0 {
                let start = j as usize;
                for v in &mut c_row[start..] {
                    *v /= c;
                }
            }
            j += mu.len() as i64;
        }
        let mut flat = Vec::with_capacity(3 * n_pairs);
        flat.extend_from_slice(&mu_row);
        flat.extend_from_slice(&p_row);
        flat.extend_from_slice(&c_row);
        Ok((flat, offsets, interpolation))
    }
}

/// One reaction to write.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactionData {
    /// MT number. Only 2 and 102 are supported — see the module docs.
    pub mt: i32,
    /// Q value \[eV\].
    pub q_value: f64,
    /// Whether the cross section is given in the centre-of-mass frame.
    pub center_of_mass: bool,
    /// Cross section \[barn\] on the nuclide's energy grid.
    pub xs: Vec<f64>,
    /// Index of the first non-zero point — OpenMC's `threshold_idx`.
    pub threshold_idx: i64,
    /// Elastic only: the outgoing angle distribution.
    pub angle: Option<AngleDistribution>,
}

/// Everything a nuclide file carries, in this writer's restricted scope.
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
}

/// Write a nuclide `.h5`.
///
/// # Errors
///
/// An MT this writer cannot express (see the module docs), a cross-section
/// array that does not match the energy grid, a non-ascending energy grid, an
/// elastic reaction with no angle distribution, or an I/O failure.
///
/// **An unsupported MT is refused, not skipped.** A file silently missing a
/// reaction transports a different nuclide from the one the caller described,
/// and the resulting `k` is wrong by an amount nothing reports.
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
        if !matches!(r.mt, 2 | 102) {
            return Err(NjoyError::Hdf5(format!(
                "MT={} is not expressible by this writer, which covers elastic (2) and \
                 capture (102) only. Fission, inelastic and (n,xn) need the correlated \
                 energy-angle laws of `openmc/data/`, which are not ported. Skipping it \
                 would emit a file that transports a DIFFERENT nuclide from the one \
                 described, and the resulting k would be wrong by an amount nothing \
                 reports.",
                r.mt
            )));
        }
        if r.xs.len() != n.energy.len() {
            return Err(NjoyError::Hdf5(format!(
                "MT={}: {} cross-section points against a {}-point energy grid",
                r.mt,
                r.xs.len(),
                n.energy.len()
            )));
        }
        if r.mt == 2 && r.angle.is_none() {
            return Err(NjoyError::Hdf5(
                "elastic scattering needs an angle distribution; without one OpenMC has \
                 no way to sample the outgoing direction"
                    .into(),
            ));
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

    let (e_min, e_max) = (n.energy[0], *n.energy.last().unwrap());
    let mut rxs = g.create_group("reactions");
    for r in &n.reactions {
        let mut rg = rxs.create_group(&format!("reaction_{:03}", r.mt));
        rg.set_attr("mt", AttrValue::I32(r.mt));
        rg.set_attr(
            "label",
            AttrValue::AsciiString(
                match r.mt {
                    2 => "(n,elastic)",
                    102 => "(n,gamma)",
                    _ => unreachable!("checked above"),
                }
                .into(),
            ),
        );
        rg.set_attr("Q_value", AttrValue::F64(r.q_value));
        rg.set_attr(
            "center_of_mass",
            AttrValue::I32(i32::from(r.center_of_mass)),
        );
        rg.set_attr("redundant", AttrValue::I32(0));

        {
            let mut tg = rg.create_group(&n.temperature);
            tg.create_dataset("xs")
                .with_f64_data(&r.xs)
                .set_attr("threshold_idx", AttrValue::I64(r.threshold_idx));
            rg.add_group(tg.finish());
        }

        if let Some(angle) = &r.angle {
            let (flat, offsets, interp) = angle.rows()?;
            let n_pairs = (flat.len() / 3) as u64;
            let mut pg = rg.create_group("product_0");
            pg.set_attr("particle", AttrValue::AsciiString("neutron".into()));
            pg.set_attr("emission_mode", AttrValue::AsciiString("prompt".into()));
            pg.set_attr("n_distribution", AttrValue::I64(1));
            {
                let y = Tabulated1D::constant(1.0, e_min, e_max)?;
                let mut flat_y = y.x.clone();
                flat_y.extend_from_slice(&y.y);
                pg.create_dataset("yield")
                    .with_shape(&[2, y.x.len() as u64])
                    .with_f64_data(&flat_y)
                    .set_attr("type", AttrValue::AsciiString("Tabulated1D".into()))
                    .set_attr("breakpoints", AttrValue::I64Array(y.breakpoints.clone()))
                    .set_attr(
                        "interpolation",
                        AttrValue::I64Array(y.interpolation.clone()),
                    );
            }
            let mut dg = pg.create_group("distribution_0");
            dg.set_attr("type", AttrValue::AsciiString("uncorrelated".into()));
            {
                let mut ag = dg.create_group("angle");
                ag.create_dataset("energy").with_f64_data(&angle.energy);
                ag.create_dataset("mu")
                    .with_shape(&[3, n_pairs])
                    .with_f64_data(&flat)
                    .set_attr("offsets", AttrValue::I64Array(offsets))
                    .set_attr("interpolation", AttrValue::I64Array(interp));
                dg.add_group(ag.finish());
            }
            pg.add_group(dg.finish());
            rg.add_group(pg.finish());
        }
        rxs.add_group(rg.finish());
    }
    g.add_group(rxs.finish());
    b.add_group(g.finish());

    b.write(path.as_ref())
        .map_err(|e| NjoyError::Hdf5(format!("writing nuclide {}: {e}", n.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
                ReactionData {
                    mt: 2,
                    q_value: 0.0,
                    center_of_mass: true,
                    xs: vec![2.0; n],
                    threshold_idx: 0,
                    angle: Some(AngleDistribution::isotropic(vec![e[0], e[n - 1]])),
                },
                ReactionData {
                    mt: 102,
                    q_value: 2.2e6,
                    center_of_mass: false,
                    // 1/v capture, normalised to 1 barn at 1 eV.
                    xs: e.iter().map(|&x| (1.0 / x).sqrt()).collect(),
                    threshold_idx: 0,
                    angle: None,
                },
            ],
        }
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

    /// **An unsupported MT is refused, not skipped.**
    ///
    /// A file silently missing a reaction transports a different nuclide from
    /// the one described, and the resulting `k` is wrong by an amount nothing
    /// reports.
    #[test]
    fn an_unsupported_reaction_is_refused_rather_than_skipped() {
        let mut d = nuclide();
        d.reactions.push(ReactionData {
            mt: 18,
            q_value: 2.0e8,
            center_of_mass: false,
            xs: vec![1.0; d.energy.len()],
            threshold_idx: 0,
            angle: None,
        });
        let err = write_nuclide(tmp("bad.h5"), &d).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("MT=18"), "{msg}");
        assert!(msg.contains("transports a DIFFERENT nuclide"), "{msg}");
    }

    /// Elastic without an angle distribution is refused — OpenMC would have no
    /// way to sample the outgoing direction.
    #[test]
    fn elastic_without_an_angle_distribution_is_refused() {
        let mut d = nuclide();
        d.reactions[0].angle = None;
        let err = write_nuclide(tmp("bad2.h5"), &d).unwrap_err();
        assert!(format!("{err}").contains("angle distribution"), "{err}");
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
        };
        assert!(a.rows().is_err());

        let a = AngleDistribution {
            energy: vec![1.0],
            mu: vec![vec![-1.0, 0.0, 1.0]],
            p: vec![vec![0.5, 0.5]],
        };
        assert!(a.rows().is_err());
    }
}
