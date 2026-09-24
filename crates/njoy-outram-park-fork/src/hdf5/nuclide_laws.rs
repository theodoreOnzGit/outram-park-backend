// SPDX-License-Identifier: GPL-3.0

//! **The secondary-distribution law hierarchy of an OpenMC neutron `.h5`** —
//! GitHub #304.
//!
//! [`super::nuclide_write`] could express exactly two reactions, elastic
//! (MT=2) and capture (MT=102), because those are the only two that need no
//! outgoing-**energy** law: capture emits nothing and elastic's outgoing
//! energy follows from two-body kinematics. Everything else — fission,
//! discrete inelastic, (n,xn) — carries a distribution, and this module is
//! that hierarchy.
//!
//! # Upstream is the specification
//!
//! Ported from `openmc/data/` at OpenMC `afa7a14`, one Rust variant per
//! upstream `to_hdf5`:
//!
//! | this module | upstream | `type` attribute |
//! |---|---|---|
//! | [`EnergyDist::Maxwell`] | `energy_distribution.py:269` | `maxwell` |
//! | [`EnergyDist::Evaporation`] | `energy_distribution.py:402` | `evaporation` |
//! | [`EnergyDist::Watt`] | `energy_distribution.py:548` | `watt` |
//! | [`EnergyDist::MadlandNix`] | `energy_distribution.py:720` | `madland-nix` |
//! | [`EnergyDist::DiscretePhoton`] | `energy_distribution.py:838` | `discrete_photon` |
//! | [`EnergyDist::Level`] | `energy_distribution.py:937` | `level` |
//! | [`EnergyDist::Continuous`] | `energy_distribution.py:1066` | `continuous` |
//! | [`AngleEnergy::Uncorrelated`] | `uncorrelated.py:57` | `uncorrelated` |
//! | [`AngleEnergy::Correlated`] | `correlated.py:107` | `correlated` |
//! | [`AngleEnergy::KalbachMann`] | `kalbach_mann.py:360` | `kalbach-mann` |
//! | [`AngleEnergy::NBody`] | `nbody.py:85` | `nbody` |
//!
//! **Three upstream classes have no writer at all** and are therefore not
//! gaps here: `ArbitraryTabulated.to_hdf5`, `GeneralEvaporation.to_hdf5` and
//! `LaboratoryAngleEnergy.to_hdf5` each `raise NotImplementedError`
//! (`energy_distribution.py:120`, `:189`, `laboratory.py:141`). Refusing them
//! is fidelity to upstream, not a shortfall against it.
//!
//! # THE RANK-2 ATTRIBUTE LIMIT (read this before adding a law)
//!
//! `continuous`, `correlated` and `kalbach-mann` all store their incident
//! energy grid's interpolation as a **rank-2** attribute — upstream writes
//! `dset.attrs['interpolation'] = np.vstack((breakpoints, interpolation))`,
//! shape `(2, NR)`. Measured on upstream's own U-235 file: every one of them
//! is `(2, 1)`.
//!
//! **`hdf5-pure` 0.20.1 cannot write a rank-2 attribute.** `AttrValue` offers
//! scalars and rank-1 arrays only (its README's attribute table is the
//! complete list), and a probe confirmed `AttrValue::I64Array(vec![3, 2])`
//! lands as shape `(2,)` where upstream writes `(2, 1)`.
//!
//! **Writing it flat anyway would be a trap, not a compromise.** OpenMC's
//! Python reader does `interp_data[0, :]`
//! (`energy_distribution.py::ContinuousTabular.from_hdf5`), which raises
//! `IndexError` on a rank-1 attribute — so `IncidentNeutron.from_hdf5` fails
//! outright. Worse, the **C++** reader would *silently accept* it: it reads
//! the attribute's real shape into a `Tensor<int>` and then takes
//! `temp.slice(0)` / `temp.slice(1)` (`src/distribution_energy.cpp:63-66`),
//! which on a rank-1 `[NR_value, interp_value]` yields the correct answer for
//! `NR = 1` **by coincidence** and the wrong one for `NR > 1`. A defect that
//! is invisible on every single-region law and wrong on the first
//! multi-region one is precisely the class this workspace refuses to ship.
//!
//! So [`EnergyDist::Continuous`], [`AngleEnergy::Correlated`] and
//! [`AngleEnergy::KalbachMann`] are **represented in full and refuse at
//! emission**, naming the dependency limit. They are not stubs: the data they
//! carry is complete and the row-packing is implemented and unit-tested, so
//! lifting the refusal is a rank-2 attribute away.
//!
//! # What IS writable, and why that is already a fissile nuclide
//!
//! Everything else needs only scalars, rank-1 arrays and datasets:
//! `level` (39 of U-235's neutron laws), `watt`/`maxwell`/`evaporation`
//! (evaluated fission spectra), `madland-nix`, `discrete_photon`, `nbody`,
//! the angle distributions, `Tabulated1D`, `Polynomial`, ν̄ and the URR
//! probability tables. A fissile nuclide whose χ is an evaluated Watt or
//! Maxwell law is therefore fully expressible and transportable — which is
//! what #304's "cannot write a fissile nuclide" actually blocks on.

use hdf5_pure::{AttrValue, GroupBuilder};

use crate::error::NjoyError;

/// A tabulated `(x, y)` function, written as OpenMC's `Tabulated1D`.
///
/// Ported from `function.py:378`. Note that `breakpoints` and `interpolation`
/// are **separate rank-1 attributes** here, not a vstack — so unlike the
/// tabulated laws this one is unaffected by the rank-2 limit in the module
/// docs.
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

    /// Write as `<name>` in `g`, mirroring `Tabulated1D.to_hdf5`.
    pub fn write(&self, g: &mut GroupBuilder, name: &str) {
        let mut flat = self.x.clone();
        flat.extend_from_slice(&self.y);
        g.create_dataset(name)
            .with_shape(&[2, self.x.len() as u64])
            .with_f64_data(&flat)
            .set_attr("type", AttrValue::AsciiString("Tabulated1D".into()))
            .set_attr("breakpoints", AttrValue::I64Array(self.breakpoints.clone()))
            .set_attr(
                "interpolation",
                AttrValue::I64Array(self.interpolation.clone()),
            );
    }
}

/// A polynomial in the incident energy, written as OpenMC's `Polynomial`.
///
/// Ported from `function.py:475`. This is how ACE stores a polynomial ν̄
/// (`NU` block `LNU = 1`).
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    /// Coefficients, lowest order first.
    pub coef: Vec<f64>,
}

impl Polynomial {
    /// Write as `<name>` in `g`.
    pub fn write(&self, g: &mut GroupBuilder, name: &str) {
        g.create_dataset(name)
            .with_f64_data(&self.coef)
            .set_attr("type", AttrValue::AsciiString("Polynomial".into()));
    }
}

/// Either representation OpenMC accepts for a scalar function of energy.
///
/// An enum rather than a trait object, per this workspace's Rust rules.
#[derive(Debug, Clone, PartialEq)]
pub enum Function1D {
    /// A tabulated function.
    Tabulated(Tabulated1D),
    /// A polynomial.
    Polynomial(Polynomial),
}

impl Function1D {
    /// Write as `<name>` in `g`.
    pub fn write(&self, g: &mut GroupBuilder, name: &str) {
        match self {
            Function1D::Tabulated(t) => t.write(g, name),
            Function1D::Polynomial(p) => p.write(g, name),
        }
    }
}

/// A nuclide's outgoing angle distribution: one tabulated `p(mu)` per
/// incident energy.
///
/// Ported from `angle_distribution.py:61`. Its `offsets` and `interpolation`
/// are rank-1, so this is writable.
#[derive(Debug, Clone, PartialEq)]
pub struct AngleDistribution {
    /// Incident energies \[eV\], ascending.
    pub energy: Vec<f64>,
    /// Per incident energy, the cosine grid.
    pub mu: Vec<Vec<f64>>,
    /// Per incident energy, the density at each `mu`.
    pub p: Vec<Vec<f64>>,
    /// Per incident energy, 1 for histogram and 2 for lin-lin. Empty means
    /// lin-lin everywhere.
    pub interpolation: Vec<i64>,
}

impl AngleDistribution {
    /// Isotropic at every energy in `energy` — `p(mu) = 1/2` on `[-1, 1]`.
    pub fn isotropic(energy: Vec<f64>) -> Self {
        let n = energy.len();
        Self {
            energy,
            mu: vec![vec![-1.0, 1.0]; n],
            p: vec![vec![0.5, 0.5]; n],
            interpolation: vec![],
        }
    }

    /// The `(mu, p, c)` triple rows OpenMC stores, plus the per-energy offsets.
    ///
    /// `c` is the **cumulative** distribution, which upstream computes on the
    /// Python side and stores. Writing it wrong is invisible to a reader that
    /// only checks shapes and produces a biased angular distribution at run
    /// time, so it is built here by trapezoid from `p` rather than left to a
    /// caller.
    pub(crate) fn rows(&self) -> Result<(Vec<f64>, Vec<i64>, Vec<i64>), NjoyError> {
        if self.mu.len() != self.energy.len() || self.p.len() != self.energy.len() {
            return Err(NjoyError::Hdf5(format!(
                "angle distribution has {} energies but {} mu grids and {} densities",
                self.energy.len(),
                self.mu.len(),
                self.p.len()
            )));
        }
        if !self.interpolation.is_empty() && self.interpolation.len() != self.energy.len() {
            return Err(NjoyError::Hdf5(format!(
                "angle distribution has {} energies but {} interpolation codes",
                self.energy.len(),
                self.interpolation.len()
            )));
        }
        let n_pairs: usize = self.mu.iter().map(Vec::len).sum();
        let mut mu_row = Vec::with_capacity(n_pairs);
        let mut p_row = Vec::with_capacity(n_pairs);
        let mut c_row = Vec::with_capacity(n_pairs);
        let mut offsets = Vec::with_capacity(self.energy.len());
        let interpolation = if self.interpolation.is_empty() {
            vec![2i64; self.energy.len()]
        } else {
            self.interpolation.clone()
        };
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

    /// Write into an already-created `angle` group.
    pub fn write(&self, ag: &mut GroupBuilder) -> Result<(), NjoyError> {
        let (flat, offsets, interp) = self.rows()?;
        let n_pairs = (flat.len() / 3) as u64;
        ag.create_dataset("energy").with_f64_data(&self.energy);
        ag.create_dataset("mu")
            .with_shape(&[3, n_pairs])
            .with_f64_data(&flat)
            .set_attr("offsets", AttrValue::I64Array(offsets))
            .set_attr("interpolation", AttrValue::I64Array(interp));
        Ok(())
    }
}

/// One incident energy's outgoing-energy table.
///
/// `p` is the density and `c` its cumulative, both as upstream stores them.
/// `n_discrete_lines` counts leading **discrete** lines, which upstream packs
/// ahead of the continuous part of a `Mixture`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyOutRow {
    /// Outgoing energies \[eV\].
    pub e_out: Vec<f64>,
    /// Density at each outgoing energy.
    pub p: Vec<f64>,
    /// Cumulative distribution at each outgoing energy.
    pub c: Vec<f64>,
    /// 1 for histogram, 2 for lin-lin.
    pub interpolation: i64,
    /// Leading discrete lines, 0 for a purely continuous row.
    pub n_discrete_lines: i64,
    /// Kalbach `r` per outgoing energy — `kalbach-mann` only.
    pub kalbach_r: Vec<f64>,
    /// Kalbach `a` per outgoing energy — `kalbach-mann` only.
    pub kalbach_a: Vec<f64>,
    /// Per outgoing energy, its own tabulated cosine — `correlated` only.
    pub cosines: Vec<CosineRow>,
}

/// One outgoing energy's tabulated cosine, for `correlated`.
#[derive(Debug, Clone, PartialEq)]
pub struct CosineRow {
    /// Cosine grid.
    pub mu: Vec<f64>,
    /// Density.
    pub p: Vec<f64>,
    /// Cumulative.
    pub c: Vec<f64>,
    /// 0 discrete, 1 histogram, 2 lin-lin — upstream's `eout[3]`.
    pub interpolation: i64,
}

/// A tabulated outgoing-energy law: the shared payload of `continuous`,
/// `correlated` and `kalbach-mann`.
///
/// Upstream writes these as three classes, but their incident-grid handling,
/// their `offsets` / `interpolation` / `n_discrete_lines` attributes and their
/// discrete-line packing are identical; only the number of parallel rows in
/// the packed array differs (3, 5 and 5). Keeping one payload here mirrors
/// that and avoids triplicating the discrete-line logic, which is the part
/// that is easy to get wrong — the same argument
/// [`crate::acer::ce_laws`] makes for the ACE side.
#[derive(Debug, Clone, PartialEq)]
pub struct TabulatedEnergyOut {
    /// Incident energy grid \[eV\].
    pub energy: Vec<f64>,
    /// Interpolation-region breakpoints over `energy`.
    pub breakpoints: Vec<i64>,
    /// ENDF interpolation code per region.
    pub interpolation: Vec<i64>,
    /// One row per incident energy.
    pub rows: Vec<EnergyOutRow>,
}

impl TabulatedEnergyOut {
    /// Check the payload is internally consistent, independent of whether it
    /// can currently be emitted.
    ///
    /// Separated from emission on purpose: the refusal in [`Self::refuse`] is
    /// a property of the *dependency*, so the data still gets validated and
    /// unit-tested, and lifting the refusal needs no new checking.
    pub fn validate(&self) -> Result<(), NjoyError> {
        if self.energy.len() != self.rows.len() {
            return Err(NjoyError::Hdf5(format!(
                "a tabulated law has {} incident energies but {} rows",
                self.energy.len(),
                self.rows.len()
            )));
        }
        if self.energy.is_empty() {
            return Err(NjoyError::Hdf5(
                "a tabulated law needs at least one incident energy".into(),
            ));
        }
        if self.breakpoints.len() != self.interpolation.len() {
            return Err(NjoyError::Hdf5(format!(
                "a tabulated law has {} breakpoints but {} interpolation codes",
                self.breakpoints.len(),
                self.interpolation.len()
            )));
        }
        for (i, r) in self.rows.iter().enumerate() {
            let n = r.e_out.len();
            if r.p.len() != n || r.c.len() != n {
                return Err(NjoyError::Hdf5(format!(
                    "row {i}: {n} outgoing energies but {} densities and {} cumulatives",
                    r.p.len(),
                    r.c.len()
                )));
            }
            if !r.kalbach_r.is_empty() && (r.kalbach_r.len() != n || r.kalbach_a.len() != n) {
                return Err(NjoyError::Hdf5(format!(
                    "row {i}: Kalbach r/a must be one per outgoing energy; got {} and {} \
                     against {n}",
                    r.kalbach_r.len(),
                    r.kalbach_a.len()
                )));
            }
            if !r.cosines.is_empty() && r.cosines.len() != n {
                return Err(NjoyError::Hdf5(format!(
                    "row {i}: a correlated law needs one cosine table per outgoing \
                     energy; got {} against {n}",
                    r.cosines.len()
                )));
            }
        }
        Ok(())
    }

    /// The packed `(rows, n)` array upstream stores, with `n_rows` parallel
    /// rows: 3 for `continuous`, 5 for `kalbach-mann` (`+ r, a`) and 3 for
    /// `correlated`'s `energy_out` before its two locator rows.
    ///
    /// Built and unit-tested even though emission is refused, so that the
    /// packing is not written blind on the day the dependency gains rank-2
    /// attributes.
    pub fn pack(&self, n_rows: usize) -> (Vec<f64>, Vec<i64>, Vec<i64>, Vec<i64>) {
        let total: usize = self.rows.iter().map(|r| r.e_out.len()).sum();
        let mut out = vec![0.0_f64; n_rows * total];
        let mut offsets = Vec::with_capacity(self.rows.len());
        let mut interp = Vec::with_capacity(self.rows.len());
        let mut n_disc = Vec::with_capacity(self.rows.len());
        let mut j = 0usize;
        for r in &self.rows {
            let n = r.e_out.len();
            offsets.push(j as i64);
            interp.push(r.interpolation);
            n_disc.push(r.n_discrete_lines);
            out[j..j + n].copy_from_slice(&r.e_out);
            out[total + j..total + j + n].copy_from_slice(&r.p);
            out[2 * total + j..2 * total + j + n].copy_from_slice(&r.c);
            if n_rows >= 5 && !r.kalbach_r.is_empty() {
                out[3 * total + j..3 * total + j + n].copy_from_slice(&r.kalbach_r);
                out[4 * total + j..4 * total + j + n].copy_from_slice(&r.kalbach_a);
            }
            j += n;
        }
        (out, offsets, interp, n_disc)
    }

    /// The refusal shared by the three tabulated laws.
    ///
    /// See the module docs: this is a `hdf5-pure` 0.20.1 limit, not a gap in
    /// the port, and writing the attribute flat would break upstream's Python
    /// reader outright while silently mis-reading in its C++ one.
    fn refuse(law: &str) -> NjoyError {
        NjoyError::Hdf5(format!(
            "the `{law}` law cannot be written: it stores its incident-energy \
             interpolation as a RANK-2 attribute (upstream writes \
             `np.vstack((breakpoints, interpolation))`, shape (2, NR)), and \
             hdf5-pure 0.20.1 can only write scalar and rank-1 attributes. \
             Writing it flat is NOT a workaround -- OpenMC's Python reader does \
             `interp_data[0, :]` and raises IndexError, while its C++ reader \
             would silently read the right answer for NR=1 and the WRONG one \
             for NR>1. The payload and its row packing are implemented and \
             tested (see TabulatedEnergyOut::pack), so this refusal is one \
             rank-2 attribute away from lifting. GitHub #304."
        ))
    }
}

/// An outgoing-energy distribution.
#[derive(Debug, Clone, PartialEq)]
pub enum EnergyDist {
    /// Maxwellian fission spectrum, `energy_distribution.py:269`.
    Maxwell {
        /// Restriction energy \[eV\].
        u: f64,
        /// `theta(E)` \[eV\].
        theta: Tabulated1D,
    },
    /// Evaporation spectrum, `energy_distribution.py:402`.
    Evaporation {
        /// Restriction energy \[eV\].
        u: f64,
        /// `theta(E)` \[eV\].
        theta: Tabulated1D,
    },
    /// Watt fission spectrum, `energy_distribution.py:548`.
    Watt {
        /// Restriction energy \[eV\].
        u: f64,
        /// `a(E)` \[eV\].
        a: Tabulated1D,
        /// `b(E)` \[1/eV\].
        b: Tabulated1D,
    },
    /// Madland-Nix fission spectrum, `energy_distribution.py:720`.
    MadlandNix {
        /// Average light-fragment kinetic energy \[eV\].
        efl: f64,
        /// Average heavy-fragment kinetic energy \[eV\].
        efh: f64,
        /// Maximum temperature `T_M(E)` \[eV\].
        tm: Tabulated1D,
    },
    /// Discrete photon, `energy_distribution.py:838`.
    DiscretePhoton {
        /// 1 primary, 2 discrete.
        primary_flag: i32,
        /// Photon energy \[eV\].
        energy: f64,
        /// Target mass / neutron mass.
        atomic_weight_ratio: f64,
    },
    /// Two-body discrete level, `energy_distribution.py:937`.
    ///
    /// This is ACE `LAW=3` and is 39 of U-235's 44 neutron laws. It needs only
    /// two scalar attributes, so it is fully writable.
    Level {
        /// `(A+1)/A * |Q|` \[eV\].
        threshold: f64,
        /// `(A/(A+1))^2`.
        mass_ratio: f64,
    },
    /// Continuous tabular, `energy_distribution.py:1066`. **Refused** — see
    /// the module docs' rank-2 section.
    Continuous(TabulatedEnergyOut),
}

impl EnergyDist {
    /// The `type` attribute upstream writes for this law.
    pub fn type_name(&self) -> &'static str {
        match self {
            EnergyDist::Maxwell { .. } => "maxwell",
            EnergyDist::Evaporation { .. } => "evaporation",
            EnergyDist::Watt { .. } => "watt",
            EnergyDist::MadlandNix { .. } => "madland-nix",
            EnergyDist::DiscretePhoton { .. } => "discrete_photon",
            EnergyDist::Level { .. } => "level",
            EnergyDist::Continuous(_) => "continuous",
        }
    }

    /// Write into an already-created `energy` group.
    pub fn write(&self, eg: &mut GroupBuilder) -> Result<(), NjoyError> {
        eg.set_attr(
            "type",
            AttrValue::AsciiString(self.type_name().into()),
        );
        match self {
            EnergyDist::Maxwell { u, theta } => {
                eg.set_attr("u", AttrValue::F64(*u));
                theta.write(eg, "theta");
            }
            EnergyDist::Evaporation { u, theta } => {
                eg.set_attr("u", AttrValue::F64(*u));
                theta.write(eg, "theta");
            }
            EnergyDist::Watt { u, a, b } => {
                eg.set_attr("u", AttrValue::F64(*u));
                a.write(eg, "a");
                b.write(eg, "b");
            }
            EnergyDist::MadlandNix { efl, efh, tm } => {
                eg.set_attr("efl", AttrValue::F64(*efl));
                eg.set_attr("efh", AttrValue::F64(*efh));
                // Upstream calls `self.tm.to_hdf5(group)`, i.e. the default
                // dataset name `xy` -- NOT 'tm'. Getting this wrong writes a
                // file whose reader finds no dataset.
                tm.write(eg, "xy");
            }
            EnergyDist::DiscretePhoton {
                primary_flag,
                energy,
                atomic_weight_ratio,
            } => {
                eg.set_attr("primary_flag", AttrValue::I32(*primary_flag));
                eg.set_attr("energy", AttrValue::F64(*energy));
                eg.set_attr(
                    "atomic_weight_ratio",
                    AttrValue::F64(*atomic_weight_ratio),
                );
            }
            EnergyDist::Level {
                threshold,
                mass_ratio,
            } => {
                eg.set_attr("threshold", AttrValue::F64(*threshold));
                eg.set_attr("mass_ratio", AttrValue::F64(*mass_ratio));
            }
            EnergyDist::Continuous(t) => {
                t.validate()?;
                return Err(TabulatedEnergyOut::refuse("continuous"));
            }
        }
        Ok(())
    }
}

/// A correlated angle-energy distribution's payload.
pub type CorrelatedData = TabulatedEnergyOut;
/// A Kalbach-Mann distribution's payload.
pub type KalbachData = TabulatedEnergyOut;

/// A secondary particle's joint angle-energy distribution.
#[derive(Debug, Clone, PartialEq)]
pub enum AngleEnergy {
    /// `uncorrelated.py:57`. Either part may be absent: elastic carries angle
    /// only, and an isotropic continuum carries energy only.
    Uncorrelated {
        /// Outgoing cosine, independent of outgoing energy.
        angle: Option<AngleDistribution>,
        /// Outgoing energy.
        energy: Option<EnergyDist>,
    },
    /// `correlated.py:107`. **Refused** — see the module docs.
    Correlated(CorrelatedData),
    /// `kalbach_mann.py:360`. **Refused** — see the module docs.
    KalbachMann(KalbachData),
    /// `nbody.py:85`. Four scalar attributes, so writable.
    NBody {
        /// Total mass of the emitted particles, in neutron masses.
        total_mass: f64,
        /// Number of particles in the phase space.
        n_particles: i32,
        /// Target mass / neutron mass.
        atomic_weight_ratio: f64,
        /// `Q` value \[eV\].
        q_value: f64,
    },
}

impl AngleEnergy {
    /// The `type` attribute upstream writes.
    pub fn type_name(&self) -> &'static str {
        match self {
            AngleEnergy::Uncorrelated { .. } => "uncorrelated",
            AngleEnergy::Correlated(_) => "correlated",
            AngleEnergy::KalbachMann(_) => "kalbach-mann",
            AngleEnergy::NBody { .. } => "nbody",
        }
    }

    /// Write into an already-created `distribution_<i>` group.
    pub fn write(&self, dg: &mut GroupBuilder) -> Result<(), NjoyError> {
        dg.set_attr(
            "type",
            AttrValue::AsciiString(self.type_name().into()),
        );
        match self {
            AngleEnergy::Uncorrelated { angle, energy } => {
                if angle.is_none() && energy.is_none() {
                    return Err(NjoyError::Hdf5(
                        "an uncorrelated distribution with neither an angle nor an \
                         energy part describes nothing; OpenMC would sample a \
                         secondary with no direction and no energy"
                            .into(),
                    ));
                }
                if let Some(a) = angle {
                    let mut ag = dg.create_group("angle");
                    a.write(&mut ag)?;
                    dg.add_group(ag.finish());
                }
                if let Some(e) = energy {
                    let mut eg = dg.create_group("energy");
                    e.write(&mut eg)?;
                    dg.add_group(eg.finish());
                }
            }
            AngleEnergy::Correlated(t) => {
                t.validate()?;
                return Err(TabulatedEnergyOut::refuse("correlated"));
            }
            AngleEnergy::KalbachMann(t) => {
                t.validate()?;
                return Err(TabulatedEnergyOut::refuse("kalbach-mann"));
            }
            AngleEnergy::NBody {
                total_mass,
                n_particles,
                atomic_weight_ratio,
                q_value,
            } => {
                dg.set_attr("total_mass", AttrValue::F64(*total_mass));
                dg.set_attr("n_particles", AttrValue::I32(*n_particles));
                dg.set_attr(
                    "atomic_weight_ratio",
                    AttrValue::F64(*atomic_weight_ratio),
                );
                dg.set_attr("q_value", AttrValue::F64(*q_value));
            }
        }
        Ok(())
    }
}

/// How a product is emitted, mirroring upstream's `emission_mode` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionMode {
    /// Emitted at the collision.
    Prompt,
    /// Emitted by a precursor, with a decay rate.
    Delayed,
    /// The total over prompt and delayed — used by `total_nu`.
    Total,
}

impl EmissionMode {
    /// Upstream's string form.
    pub fn as_str(self) -> &'static str {
        match self {
            EmissionMode::Prompt => "prompt",
            EmissionMode::Delayed => "delayed",
            EmissionMode::Total => "total",
        }
    }
}

/// A reaction product, ported from `product.py:118`.
#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    /// `"neutron"`, `"photon"`, and so on.
    pub particle: String,
    /// Prompt, delayed or total.
    pub emission_mode: EmissionMode,
    /// Precursor decay constant \[1/s\]. Upstream writes the attribute **only
    /// when positive**, so a zero here omits it rather than writing 0.0.
    pub decay_rate: f64,
    /// Yield per reaction as a function of incident energy.
    pub yield_: Function1D,
    /// One or more distributions; `n_distribution` is written from its length.
    pub distribution: Vec<AngleEnergy>,
    /// Per-distribution applicability. Empty means "always applicable", which
    /// is what upstream writes when `self.applicability` is empty.
    pub applicability: Vec<Tabulated1D>,
}

impl Product {
    /// A prompt neutron with unit yield and one distribution.
    pub fn prompt_neutron(
        distribution: AngleEnergy,
        e_min: f64,
        e_max: f64,
    ) -> Result<Self, NjoyError> {
        Ok(Self {
            particle: "neutron".into(),
            emission_mode: EmissionMode::Prompt,
            decay_rate: 0.0,
            yield_: Function1D::Tabulated(Tabulated1D::constant(1.0, e_min, e_max)?),
            distribution: vec![distribution],
            applicability: vec![],
        })
    }

    /// Write into an already-created `product_<i>` group.
    pub fn write(&self, pg: &mut GroupBuilder) -> Result<(), NjoyError> {
        if self.distribution.is_empty() {
            return Err(NjoyError::Hdf5(format!(
                "product '{}' has no distribution; OpenMC reads n_distribution \
                 and then looks for distribution_0, so a file written this way \
                 fails on read rather than transporting",
                self.particle
            )));
        }
        if !self.applicability.is_empty() && self.applicability.len() != self.distribution.len() {
            return Err(NjoyError::Hdf5(format!(
                "product '{}' has {} distributions but {} applicability functions; \
                 upstream indexes them together",
                self.particle,
                self.distribution.len(),
                self.applicability.len()
            )));
        }
        pg.set_attr(
            "particle",
            AttrValue::AsciiString(self.particle.clone()),
        );
        pg.set_attr(
            "emission_mode",
            AttrValue::AsciiString(self.emission_mode.as_str().into()),
        );
        if self.decay_rate > 0.0 {
            pg.set_attr("decay_rate", AttrValue::F64(self.decay_rate));
        }
        self.yield_.write(pg, "yield");
        pg.set_attr(
            "n_distribution",
            AttrValue::I64(self.distribution.len() as i64),
        );
        for (i, d) in self.distribution.iter().enumerate() {
            let mut dg = pg.create_group(&format!("distribution_{i}"));
            if let Some(a) = self.applicability.get(i) {
                a.write(&mut dg, "applicability");
            }
            d.write(&mut dg)?;
            pg.add_group(dg.finish());
        }
        Ok(())
    }
}

/// Unresolved-resonance probability tables, ported from `urr.py:132`.
///
/// `table` is rank 3 — `(n_energy, n_bands, 6)` — which is a **dataset**, not
/// an attribute, so the rank-2 attribute limit does not touch it.
#[derive(Debug, Clone, PartialEq)]
pub struct UrrTables {
    /// 2 lin-lin, 5 log-log, as upstream stores it.
    pub interpolation: i32,
    /// Inelastic competition flag.
    pub inelastic_flag: i32,
    /// Other-absorption flag.
    pub absorption_flag: i32,
    /// Whether the factors multiply the smooth cross section.
    pub multiply_smooth: bool,
    /// Incident energies \[eV\].
    pub energy: Vec<f64>,
    /// `(n_energy, n_bands, 6)` flattened in C order.
    pub table: Vec<f64>,
    /// Number of probability bands.
    pub n_bands: usize,
}

impl UrrTables {
    /// Write into an already-created temperature group under `urr`.
    pub fn write(&self, g: &mut GroupBuilder) -> Result<(), NjoyError> {
        let want = self.energy.len() * self.n_bands * 6;
        if self.table.len() != want {
            return Err(NjoyError::Hdf5(format!(
                "URR table has {} values but {} energies x {} bands x 6 needs {want}",
                self.table.len(),
                self.energy.len(),
                self.n_bands
            )));
        }
        g.set_attr("interpolation", AttrValue::I32(self.interpolation));
        g.set_attr("inelastic", AttrValue::I32(self.inelastic_flag));
        g.set_attr("absorption", AttrValue::I32(self.absorption_flag));
        g.set_attr(
            "multiply_smooth",
            AttrValue::I32(i32::from(self.multiply_smooth)),
        );
        g.create_dataset("energy").with_f64_data(&self.energy);
        g.create_dataset("table")
            .with_shape(&[self.energy.len() as u64, self.n_bands as u64, 6])
            .with_f64_data(&self.table);
        Ok(())
    }
}

/// The standard OpenMC label for an MT, mirroring `REACTION_NAME`.
///
/// Upstream falls back to the bare MT number when the map has no entry
/// (`reaction.py:926`), and so does this.
pub fn reaction_label(mt: i32) -> String {
    let fixed = match mt {
        2 => "(n,elastic)",
        4 => "(n,level)",
        5 => "(n,misc)",
        11 => "(n,2nd)",
        16 => "(n,2n)",
        17 => "(n,3n)",
        18 => "(n,fission)",
        19 => "(n,f)",
        20 => "(n,nf)",
        21 => "(n,2nf)",
        22 => "(n,na)",
        23 => "(n,n3a)",
        24 => "(n,2na)",
        25 => "(n,3na)",
        27 => "(n,absorption)",
        28 => "(n,np)",
        29 => "(n,n2a)",
        30 => "(n,2n2a)",
        32 => "(n,nd)",
        33 => "(n,nt)",
        34 => "(n,n3He)",
        35 => "(n,nd2a)",
        36 => "(n,nt2a)",
        37 => "(n,4n)",
        38 => "(n,3nf)",
        41 => "(n,2np)",
        42 => "(n,3np)",
        44 => "(n,n2p)",
        45 => "(n,npa)",
        91 => "(n,nc)",
        101 => "(n,disappear)",
        102 => "(n,gamma)",
        103 => "(n,p)",
        104 => "(n,d)",
        105 => "(n,t)",
        106 => "(n,3He)",
        107 => "(n,a)",
        301 => "heating",
        444 => "damage-energy",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    // The discrete levels: MT=51 is (n,n1) through MT=90 = (n,n40).
    if (51..=90).contains(&mt) {
        return format!("(n,n{})", mt - 50);
    }
    format!("{mt}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_law() -> TabulatedEnergyOut {
        TabulatedEnergyOut {
            energy: vec![1.0e3, 2.0e6],
            breakpoints: vec![2],
            interpolation: vec![2],
            rows: vec![
                EnergyOutRow {
                    e_out: vec![0.0, 1.0e3],
                    p: vec![0.0, 2.0e-3],
                    c: vec![0.0, 1.0],
                    interpolation: 2,
                    n_discrete_lines: 0,
                    kalbach_r: vec![0.1, 0.2],
                    kalbach_a: vec![1.0, 2.0],
                    cosines: vec![],
                },
                EnergyOutRow {
                    e_out: vec![0.0, 5.0e5, 2.0e6],
                    p: vec![0.0, 1.0e-6, 0.0],
                    c: vec![0.0, 0.5, 1.0],
                    interpolation: 2,
                    n_discrete_lines: 0,
                    kalbach_r: vec![0.3, 0.4, 0.5],
                    kalbach_a: vec![3.0, 4.0, 5.0],
                    cosines: vec![],
                },
            ],
        }
    }

    /// **The refusal is precise, and it is a refusal rather than a wrong file.**
    ///
    /// The whole point of #304's finding is that a component can pass every
    /// check it has while being wrong for what it is for. A law that wrote a
    /// rank-1 attribute would pass a round trip through this crate and fail in
    /// OpenMC, so the error is asserted here by content.
    #[test]
    fn the_three_tabulated_laws_refuse_and_say_why() {
        for (name, err) in [
            (
                "continuous",
                EnergyDist::Continuous(flat_law())
                    .write(&mut hdf5_pure::FileBuilder::new().create_group("e"))
                    .unwrap_err(),
            ),
            (
                "correlated",
                AngleEnergy::Correlated(flat_law())
                    .write(&mut hdf5_pure::FileBuilder::new().create_group("d"))
                    .unwrap_err(),
            ),
            (
                "kalbach-mann",
                AngleEnergy::KalbachMann(flat_law())
                    .write(&mut hdf5_pure::FileBuilder::new().create_group("d"))
                    .unwrap_err(),
            ),
        ] {
            let m = format!("{err}");
            assert!(m.contains(name), "the error must name the law: {m}");
            assert!(
                m.contains("RANK-2") && m.contains("hdf5-pure"),
                "the error must name the dependency limit, not just fail: {m}"
            );
            assert!(
                m.contains("IndexError") && m.contains("NR>1"),
                "the error must say why writing it flat is a trap rather than a \
                 compromise, or the next reader will just do that: {m}"
            );
        }
    }

    /// **The packing is tested even though emission is refused.**
    ///
    /// Otherwise the day the dependency gains rank-2 attributes, the row
    /// packing gets exercised for the first time against a real nuclide, which
    /// is the worst moment to find out it is transposed.
    #[test]
    fn the_packed_rows_are_column_major_by_quantity() {
        let law = flat_law();
        law.validate().unwrap();
        let (packed, offsets, interp, n_disc) = law.pack(5);
        // 2 + 3 = 5 outgoing energies in total, 5 parallel rows.
        assert_eq!(packed.len(), 25);
        assert_eq!(offsets, vec![0, 2]);
        assert_eq!(interp, vec![2, 2]);
        assert_eq!(n_disc, vec![0, 0]);
        // Row 0 is every e_out, concatenated across incident energies.
        assert_eq!(&packed[0..5], &[0.0, 1.0e3, 0.0, 5.0e5, 2.0e6]);
        // Row 1 is every density.
        assert_eq!(&packed[5..10], &[0.0, 2.0e-3, 0.0, 1.0e-6, 0.0]);
        // Row 2 every cumulative, ending at 1 for each incident energy.
        assert_eq!(&packed[10..15], &[0.0, 1.0, 0.0, 0.5, 1.0]);
        // Rows 3 and 4 are Kalbach r and a.
        assert_eq!(&packed[15..20], &[0.1, 0.2, 0.3, 0.4, 0.5]);
        assert_eq!(&packed[20..25], &[1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    /// A 3-row pack must not touch the Kalbach rows even when they are present.
    #[test]
    fn a_three_row_pack_ignores_kalbach_columns() {
        let (packed, ..) = flat_law().pack(3);
        assert_eq!(packed.len(), 15);
        assert_eq!(&packed[10..15], &[0.0, 1.0, 0.0, 0.5, 1.0]);
    }

    /// Inconsistent payloads are refused before anything is written.
    #[test]
    fn a_law_whose_rows_do_not_match_its_grid_is_refused() {
        let mut law = flat_law();
        law.rows.pop();
        let m = format!("{}", law.validate().unwrap_err());
        assert!(m.contains("2 incident energies but 1 rows"), "{m}");

        let mut law = flat_law();
        law.rows[0].p.pop();
        assert!(law.validate().is_err());

        let mut law = flat_law();
        law.rows[0].kalbach_r.pop();
        assert!(law.validate().is_err());
    }

    /// **`Level` is the law a fissile nuclide's 39 discrete channels need**,
    /// and it needs no rank-2 attribute — so it must actually write.
    #[test]
    fn the_level_law_writes_and_names_itself_level() {
        let mut fb = hdf5_pure::FileBuilder::new();
        let mut eg = fb.create_group("energy");
        EnergyDist::Level {
            threshold: 1.0e5,
            mass_ratio: 0.99,
        }
        .write(&mut eg)
        .expect("level needs only two scalar attributes");
        assert_eq!(
            EnergyDist::Level {
                threshold: 1.0,
                mass_ratio: 1.0
            }
            .type_name(),
            "level"
        );
    }

    /// An uncorrelated distribution carrying neither part is refused.
    #[test]
    fn an_empty_uncorrelated_distribution_is_refused() {
        let mut fb = hdf5_pure::FileBuilder::new();
        let mut dg = fb.create_group("d");
        let err = AngleEnergy::Uncorrelated {
            angle: None,
            energy: None,
        }
        .write(&mut dg)
        .unwrap_err();
        assert!(format!("{err}").contains("describes nothing"));
    }

    /// Labels must match upstream's `REACTION_NAME` exactly, including the
    /// discrete-level numbering where MT=51 is `(n,n1)`, not `(n,n51)`.
    #[test]
    fn reaction_labels_match_upstream() {
        assert_eq!(reaction_label(2), "(n,elastic)");
        assert_eq!(reaction_label(18), "(n,fission)");
        assert_eq!(reaction_label(16), "(n,2n)");
        assert_eq!(reaction_label(51), "(n,n1)");
        assert_eq!(reaction_label(52), "(n,n2)");
        assert_eq!(reaction_label(90), "(n,n40)");
        assert_eq!(reaction_label(91), "(n,nc)");
        assert_eq!(reaction_label(102), "(n,gamma)");
        assert_eq!(reaction_label(301), "heating");
        // No entry upstream -> the bare number, as upstream does.
        assert_eq!(reaction_label(9999), "9999");
    }

    /// A URR table whose extent does not match its own band count is refused
    /// rather than written at the wrong shape.
    #[test]
    fn a_urr_table_of_the_wrong_extent_is_refused() {
        let mut fb = hdf5_pure::FileBuilder::new();
        let mut g = fb.create_group("294K");
        let bad = UrrTables {
            interpolation: 2,
            inelastic_flag: -1,
            absorption_flag: -1,
            multiply_smooth: false,
            energy: vec![1.0e3, 1.0e4],
            table: vec![0.0; 11],
            n_bands: 1,
        };
        let err = bad.write(&mut g).unwrap_err();
        assert!(format!("{err}").contains("needs 12"), "{err}");
    }
}
