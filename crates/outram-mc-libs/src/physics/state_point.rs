// SPDX-License-Identifier: GPL-3.0

//! **State points and run summaries** — what a run leaves behind.
//! GitHub #271 scope items 3 and 4.
//!
//! Ported in structure from `src/state_point.cpp` and `src/summary.cpp` at
//! OpenMC `afa7a14`.
//!
//! # Why a state point is cheap in THIS crate
//!
//! Upstream has to record the RNG state of every particle in flight. This
//! crate does not: `rng::lcg::init_seed(id, offset, master)` means a
//! particle's entire stream is reconstructible from its **id and the master
//! seed**. So a state point here records the master seed and the generation
//! counter, not a stream — which is why [`StatePoint::rng_master_seed`] is a
//! single `u64` where upstream carries an array.
//!
//! That is also the reason the restart guarantee can be **exact** rather than
//! statistical: given the same seed, the same generation index and the same
//! fission bank, the continued run consumes the identical stream.
//!
//! # What is NOT here
//!
//! The HDF5 **file** layout. Per #271 the writer belongs in
//! `njoy-outram-park-fork` so this crate's inner loop stays free of file I/O;
//! this module captures the state and hands it over. [`StatePoint`] is
//! therefore a value, not a file, and nothing here opens one.

use crate::geometry::geometry::Geometry;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::tally::tally::Tally;

/// A run's state at a generation boundary — `src/state_point.cpp`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatePoint {
    /// Generations completed when this was taken. A restart resumes at this
    /// index, so it is the count of *finished* generations, not the index of
    /// the one in progress.
    pub generations_done: usize,
    /// Inactive generations the run was configured with, carried so a restart
    /// knows whether it is still converging the source.
    pub n_inactive: usize,
    /// Histories per generation.
    pub n_particles: usize,
    /// The master RNG seed. See the module docs for why one `u64` suffices.
    pub rng_master_seed: u64,
    /// Per-generation eigenvalue estimates, all generations so far.
    pub k_by_generation: Vec<f64>,
    /// The fission bank at this boundary, as `(x, y, z, u, v, w, E)` — the
    /// same seven numbers a `Site` carries.
    ///
    /// Stored as plain tuples rather than the crate-internal `Site` because a
    /// state point is an **interchange** artefact: it crosses into the HDF5
    /// writer in another crate, and pinning it to a `pub(crate)` type would
    /// make that impossible without leaking the type.
    pub source_bank: Vec<[f64; 7]>,
    /// Tallies as accumulated.
    pub tallies: Vec<Tally>,
}

impl StatePoint {
    /// Mean eigenvalue over the active generations.
    ///
    /// # Errors
    ///
    /// A state point taken before any active generation finished — there is no
    /// eigenvalue yet, and returning the inactive generations' mean would be a
    /// number with no meaning.
    pub fn k_mean(&self) -> Result<f64, String> {
        let active = self.k_by_generation.len().saturating_sub(self.n_inactive);
        if active == 0 {
            return Err(format!(
                "this state point has {} generations of which {} are inactive, so no \
                 eigenvalue has been accumulated yet",
                self.k_by_generation.len(),
                self.n_inactive
            ));
        }
        Ok(self.k_by_generation[self.n_inactive..].iter().sum::<f64>() / active as f64)
    }

    /// Whether this state point can be restarted from.
    ///
    /// A state point with an **empty fission bank** cannot: an eigenvalue run
    /// resumed with no source has nothing to transport, and would silently
    /// produce a run of zeros rather than an error.
    pub fn is_restartable(&self) -> bool {
        !self.source_bank.is_empty() && self.n_particles > 0
    }

    /// Bytes this state point occupies, approximately — for deciding how often
    /// to take one.
    pub fn approx_bytes(&self) -> usize {
        self.source_bank.len() * 7 * 8
            + self.k_by_generation.len() * 8
            + self.tallies.iter().map(|t| t.bins.len() * 24).sum::<usize>()
    }
}

/// The geometry and materials **as actually built** — `src/summary.cpp`.
///
/// # Why this is worth writing down
///
/// It doubles as a check that the model is the model intended. Several of this
/// crate's recorded defects were input errors that looked like physics: a
/// density read as atoms/cm³ where the field means atoms/barn-cm (the #266
/// near-miss, which would have "passed" a convergence study against a solution
/// that never moved), a two-nuclide Godiva standing in for a three-nuclide
/// one. A summary is what makes those visible without re-reading the code that
/// built the model.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    /// One row per cell: `(id, what it is filled with as a short name,
    /// material index where that applies)`.
    pub cells: Vec<(i32, &'static str, Option<usize>)>,
    /// One row per surface: its kind, as a short name.
    pub surfaces: Vec<&'static str>,
    /// One row per material: `(id, name, temperature [K], components)`, each
    /// component `(nuclide name, atom density [atoms/barn-cm])`.
    pub materials: Vec<MaterialSummary>,
    /// Universe ids, in order.
    pub universes: Vec<i32>,
    /// Number of lattices.
    pub n_lattices: usize,
}

/// One material as built.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialSummary {
    /// The material's id.
    pub id: i32,
    /// Its name.
    pub name: String,
    /// Its temperature \[K\].
    pub temperature_k: f64,
    /// `(nuclide name, atom density [atoms/barn-cm])`.
    pub components: Vec<(String, f64)>,
    /// Total atom density \[atoms/barn-cm\] — the number a units error shows
    /// up in first.
    pub total_atom_density: f64,
}

impl RunSummary {
    /// Build a summary from the model a run was given.
    pub fn of(geom: &Geometry, materials: &[Material], nuclides: &[Nuclide]) -> Self {
        use crate::geometry::cell::CellFill;
        use crate::geometry::surface::SurfaceKind;
        Self {
            cells: geom
                .cells
                .iter()
                .map(|c| match c.fill {
                    CellFill::Material(i) => (c.id, "material", Some(i)),
                    CellFill::Universe(_) => (c.id, "universe", None),
                    CellFill::Lattice(_) => (c.id, "lattice", None),
                    CellFill::Void => (c.id, "void", None),
                })
                .collect(),
            surfaces: geom
                .surfaces
                .iter()
                .map(|s| match s {
                    SurfaceKind::XPlane(_) => "x-plane",
                    SurfaceKind::YPlane(_) => "y-plane",
                    SurfaceKind::ZPlane(_) => "z-plane",
                    SurfaceKind::Plane(_) => "plane",
                    SurfaceKind::Sphere(_) => "sphere",
                    SurfaceKind::XCylinder(_) => "x-cylinder",
                    SurfaceKind::YCylinder(_) => "y-cylinder",
                    SurfaceKind::ZCylinder(_) => "z-cylinder",
                    SurfaceKind::XCone(_) => "x-cone",
                    SurfaceKind::YCone(_) => "y-cone",
                    SurfaceKind::ZCone(_) => "z-cone",
                    SurfaceKind::Quadric(_) => "quadric",
                    SurfaceKind::XTorus(_) => "x-torus",
                    SurfaceKind::YTorus(_) => "y-torus",
                    SurfaceKind::ZTorus(_) => "z-torus",
                })
                .collect(),
            materials: materials
                .iter()
                .map(|m| MaterialSummary {
                    id: m.id,
                    name: m.name.clone(),
                    temperature_k: m.temperature,
                    components: m
                        .components
                        .iter()
                        .map(|c| {
                            (
                                nuclides
                                    .get(c.nuclide_idx)
                                    .map(|n| n.name.clone())
                                    .unwrap_or_else(|| format!("<index {}>", c.nuclide_idx)),
                                c.atom_density,
                            )
                        })
                        .collect(),
                    total_atom_density: m.components.iter().map(|c| c.atom_density).sum(),
                })
                .collect(),
            universes: geom.universes.iter().map(|u| u.id).collect(),
            n_lattices: geom.lattices.len(),
        }
    }

    /// Materials whose total atom density is outside the plausible range for
    /// condensed matter, `[1e-6, 0.2]` atoms/barn-cm.
    ///
    /// # Why this check and not a schema validation
    ///
    /// Because this is the error that actually happens. GitHub #266's
    /// convergence study was written with `1.0e21` for an initial density,
    /// reading the field as atoms/cm³ when it means atoms/barn-cm — a factor
    /// of `1e24`. The flux collapsed to `5.5e-11`, nothing burned, every step
    /// size returned a bit-identical answer, and the study would have "passed"
    /// against a solution that never moved. A wrong atom density does not
    /// crash; it produces a plausible, wrong, self-consistent answer.
    ///
    /// A void material (no components) is not flagged: zero is a legitimate
    /// total there.
    pub fn implausible_densities(&self) -> Vec<&MaterialSummary> {
        self.materials
            .iter()
            .filter(|m| {
                !m.components.is_empty()
                    && !(1.0e-6..=0.2).contains(&m.total_atom_density)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tally::tally::{ScoreType, TallyBin};

    fn sp(gens: usize, inactive: usize, bank: usize) -> StatePoint {
        StatePoint {
            generations_done: gens,
            n_inactive: inactive,
            n_particles: 100,
            rng_master_seed: 1,
            k_by_generation: (0..gens).map(|i| 1.0 + i as f64 * 0.01).collect(),
            source_bank: vec![[0.0; 7]; bank],
            tallies: vec![],
        }
    }

    /// An eigenvalue is refused before any active generation has finished.
    #[test]
    fn a_state_point_before_the_active_generations_has_no_eigenvalue() {
        assert!(sp(5, 10, 100).k_mean().is_err());
        assert!(sp(10, 10, 100).k_mean().is_err());
        // One active generation: k = 1.0 + 10*0.01 = 1.10.
        let k = sp(11, 10, 100).k_mean().unwrap();
        assert!((k - 1.10).abs() < 1e-12, "k = {k}");
    }

    /// The mean uses only the active generations.
    #[test]
    fn the_eigenvalue_excludes_the_inactive_generations() {
        // 4 generations, 2 inactive: mean of 1.02 and 1.03.
        let k = sp(4, 2, 50).k_mean().unwrap();
        assert!((k - 1.025).abs() < 1e-12, "k = {k}");
    }

    /// A state point with no fission bank cannot be restarted from — resuming
    /// would transport nothing and report zeros.
    #[test]
    fn an_empty_bank_is_not_restartable() {
        assert!(!sp(20, 10, 0).is_restartable());
        assert!(sp(20, 10, 1).is_restartable());
        let mut s = sp(20, 10, 100);
        s.n_particles = 0;
        assert!(!s.is_restartable());
    }

    /// The size estimate scales with what is actually stored.
    #[test]
    fn the_size_estimate_tracks_the_contents() {
        let small = sp(10, 5, 100);
        let big = sp(10, 5, 10_000);
        assert!(big.approx_bytes() > 50 * small.approx_bytes());

        let mut with_tally = sp(10, 5, 100);
        with_tally.tallies.push(Tally {
            id: 1,
            name: "t".into(),
            filters: vec![],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); 1000],
        });
        assert!(with_tally.approx_bytes() > small.approx_bytes());
    }

    /// **The units check, on the error that actually happened.**
    ///
    /// `1.0e21` is what #266's convergence study used for an initial density,
    /// reading atoms/barn-cm as atoms/cm³. Nothing crashed; the flux collapsed
    /// to 5.5e-11 and every step size returned a bit-identical answer, so the
    /// study would have passed against a solution that never moved.
    #[test]
    fn an_atom_density_in_the_wrong_units_is_flagged() {
        let s = RunSummary {
            cells: vec![],
            surfaces: vec![],
            universes: vec![],
            n_lattices: 0,
            materials: vec![
                MaterialSummary {
                    id: 1,
                    name: "sane".into(),
                    temperature_k: 293.6,
                    components: vec![("U235".into(), 4.4994e-2), ("U238".into(), 2.4984e-3)],
                    total_atom_density: 4.4994e-2 + 2.4984e-3,
                },
                MaterialSummary {
                    id: 2,
                    name: "atoms-per-cc-by-mistake".into(),
                    temperature_k: 293.6,
                    components: vec![("U235".into(), 1.0e21)],
                    total_atom_density: 1.0e21,
                },
                MaterialSummary {
                    id: 3,
                    name: "void".into(),
                    temperature_k: 293.6,
                    components: vec![],
                    total_atom_density: 0.0,
                },
            ],
        };
        let bad = s.implausible_densities();
        assert_eq!(bad.len(), 1, "got {:?}", bad.iter().map(|m| &m.name).collect::<Vec<_>>());
        assert_eq!(bad[0].name, "atoms-per-cc-by-mistake");
    }
}
