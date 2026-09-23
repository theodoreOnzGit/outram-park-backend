// SPDX-License-Identifier: GPL-3.0

//! **Mesh, file, surface and constrained sources** — GitHub #264.
//!
//! Ported in structure from `include/openmc/source.h`,
//! `src/distribution_spatial.cpp` and the `settings::surf_source_*` machinery
//! at OpenMC `afa7a14`.
//!
//! # What each of these unlocks
//!
//! - [`MeshSource`] — a source distributed over a mesh with a per-element
//!   strength. This is how an activated-component or decay-photon source is
//!   specified, and how a fixed source is handed over from another code.
//! - [`SurfaceSource`] — record every particle crossing a surface, then replay
//!   it as the source of a second, decoupled calculation. **The standard
//!   two-stage shielding workflow**, and the cheapest large win for
//!   deep-penetration problems after weight windows.
//! - [`FileSource`] — a source bank held as data, for restarting a
//!   fixed-source run or driving one code from another's output.
//! - [`SourceConstraints`] — reject sampled sites outside a cell, a material
//!   or a fissionable region.
//! - [`IndependentSpatial`] — independent distributions per coordinate, rather
//!   than the three fixed shapes the crate had.
//!
//! # The one thing a surface source gets wrong silently
//!
//! A recorded crossing must carry the particle's **weight** as well as its
//! phase-space state. Replaying unit-weight particles from a run that used
//! variance reduction inflates the second stage by however much weight the
//! first stage had removed — and the answer stays plausible, because nothing
//! about it looks wrong. [`SurfaceCrossing`] therefore has no default weight
//! and [`SurfaceSource::total_weight`] exists so a caller can check the
//! bookkeeping rather than assume it.

use crate::geometry::position::{Direction, Position};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::prn;
use crate::source::source::SourceSite;
use crate::tally::mesh::MeshKind;

/// A source spread over mesh elements with per-element strengths —
/// `MeshSource` (`include/openmc/source.h:222`) with `MeshElementSpatial`
/// (`:238`).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSource {
    /// The mesh whose elements carry the source.
    pub mesh: MeshKind,
    /// Relative strength per element, length `mesh.n_bins()`. Need not sum to
    /// anything in particular — it is normalised on construction.
    pub strengths: Vec<f64>,
    /// Emission energy \[eV\]. One energy for now; a per-element spectrum is
    /// the natural extension and is **not** implemented, rather than being
    /// faked by reusing this for every element.
    pub energy_ev: f64,
    /// Cumulative strengths, for inversion sampling.
    cdf: Vec<f64>,
    /// Total strength before normalisation, carried so a caller can convert
    /// back to absolute emission rates.
    total: f64,
}

impl MeshSource {
    /// Build from per-element strengths.
    ///
    /// # Errors
    ///
    /// A strength array that does not match the mesh, a negative strength, or
    /// an all-zero one — the last would give a source that emits nothing while
    /// looking like a source.
    pub fn new(mesh: MeshKind, strengths: Vec<f64>, energy_ev: f64) -> Result<Self, String> {
        let n = mesh.n_bins();
        if strengths.len() != n {
            return Err(format!(
                "the mesh has {n} elements but {} strengths were given",
                strengths.len()
            ));
        }
        if let Some(i) = strengths.iter().position(|&s| s < 0.0) {
            return Err(format!(
                "element {i} has strength {}; a negative source strength is not a \
                 physical emission rate",
                strengths[i]
            ));
        }
        let total: f64 = strengths.iter().sum();
        if !(total > 0.0) {
            return Err(format!(
                "the total source strength is {total}; this source emits nothing while \
                 looking like a source"
            ));
        }
        let mut cdf = Vec::with_capacity(n);
        let mut acc = 0.0;
        for s in &strengths {
            acc += s / total;
            cdf.push(acc);
        }
        Ok(Self {
            mesh,
            strengths,
            energy_ev,
            cdf,
            total,
        })
    }

    /// Total strength before normalisation.
    pub fn total_strength(&self) -> f64 {
        self.total
    }

    /// Sample an element index by its strength.
    pub fn sample_element(&self, seed: &mut u64) -> usize {
        let xi = prn(seed);
        self.cdf.partition_point(|&c| c <= xi).min(self.cdf.len() - 1)
    }

    /// Sample one source site: an element by strength, then a point uniformly
    /// inside it, then an isotropic direction.
    ///
    /// # Errors
    ///
    /// A mesh whose element bounds this crate cannot sample uniformly. Only
    /// [`MeshKind::Regular`] and [`MeshKind::Rectilinear`] have a uniform
    /// point sampler; the cylindrical and spherical ones would need their own
    /// Jacobians, and sampling those as if they were boxes would put the
    /// source in the wrong place while returning a point that is inside the
    /// element.
    pub fn sample(&self, seed: &mut u64) -> Result<SourceSite, String> {
        let element = self.sample_element(seed);
        let (lo, hi) = self.element_bounds(element)?;
        let r = Position::new(
            lo[0] + (hi[0] - lo[0]) * prn(seed),
            lo[1] + (hi[1] - lo[1]) * prn(seed),
            lo[2] + (hi[2] - lo[2]) * prn(seed),
        );
        let (dx, dy, dz) = isotropic_direction(seed);
        Ok(SourceSite {
            r,
            u: Direction::new(dx, dy, dz),
            e: self.energy_ev,
            wgt: 1.0,
        })
    }

    /// The Cartesian bounds of one element.
    fn element_bounds(&self, element: usize) -> Result<([f64; 3], [f64; 3]), String> {
        match &self.mesh {
            MeshKind::Regular(m) => {
                let d = m.dimension;
                let w = m.width();
                let i = element % d[0];
                let j = (element / d[0]) % d[1];
                let k = element / (d[0] * d[1]);
                let lo = [
                    m.lower_left[0] + i as f64 * w[0],
                    m.lower_left[1] + j as f64 * w[1],
                    m.lower_left[2] + k as f64 * w[2],
                ];
                Ok((lo, [lo[0] + w[0], lo[1] + w[1], lo[2] + w[2]]))
            }
            MeshKind::Rectilinear(m) => {
                let d = m.dimension();
                let i = element % d[0];
                let j = (element / d[0]) % d[1];
                let k = element / (d[0] * d[1]);
                Ok((
                    [m.grid[0][i], m.grid[1][j], m.grid[2][k]],
                    [m.grid[0][i + 1], m.grid[1][j + 1], m.grid[2][k + 1]],
                ))
            }
            other => Err(format!(
                "a {} has no uniform point sampler in this port. Sampling it as if it \
                 were a box would return a point INSIDE the element and in the wrong \
                 place within it, which nothing downstream would flag.",
                match other {
                    MeshKind::Cylindrical(_) => "cylindrical mesh",
                    MeshKind::Spherical(_) => "spherical mesh",
                    _ => unreachable!(),
                }
            )),
        }
    }
}

/// One recorded surface crossing — `settings::surf_source_write`.
///
/// # There is no `Default`, on purpose
///
/// A crossing with a defaulted weight of 1.0 is the failure mode this type
/// exists to avoid: replaying unit-weight particles from a run that used
/// variance reduction inflates the second stage by exactly the weight the
/// first stage removed, and the answer stays plausible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCrossing {
    /// Where it crossed \[cm\].
    pub r: Position,
    /// Direction of travel at the crossing.
    pub u: Direction,
    /// Energy \[eV\].
    pub energy: f64,
    /// Statistical weight — see the type docs.
    pub weight: f64,
    /// Which surface was crossed.
    pub surface_idx: usize,
}

/// A recorded set of crossings, usable as the source of a second run —
/// `settings::surf_source_write` / `surf_source_read`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceSource {
    /// The crossings, in the order they were recorded.
    pub crossings: Vec<SurfaceCrossing>,
    /// Surfaces being recorded. Empty means **every** surface, matching
    /// upstream's behaviour when no `ssw_cell_id` is given.
    pub surfaces: Vec<usize>,
    /// Cap on recorded crossings — upstream's `ssw_max_particles`.
    pub max_particles: usize,
    /// Crossings refused because the cap was reached.
    pub dropped: usize,
}

impl SurfaceSource {
    /// A recorder for the given surfaces (empty = all), capped at
    /// `max_particles`.
    pub fn recording(surfaces: Vec<usize>, max_particles: usize) -> Self {
        Self {
            crossings: Vec::new(),
            surfaces,
            max_particles,
            dropped: 0,
        }
    }

    /// Whether `surface_idx` is being recorded.
    pub fn records(&self, surface_idx: usize) -> bool {
        self.surfaces.is_empty() || self.surfaces.contains(&surface_idx)
    }

    /// Record a crossing, if it is on a watched surface and within the cap.
    pub fn record(&mut self, c: SurfaceCrossing) {
        if !self.records(c.surface_idx) {
            return;
        }
        if self.crossings.len() >= self.max_particles {
            self.dropped += 1;
            return;
        }
        self.crossings.push(c);
    }

    /// Total weight recorded.
    ///
    /// **The bookkeeping check a two-stage run needs.** Stage two's answer
    /// scales with this; if it does not match the weight stage one actually
    /// put across the surface, the two stages do not describe the same
    /// problem, and nothing else will say so.
    pub fn total_weight(&self) -> f64 {
        self.crossings.iter().map(|c| c.weight).sum()
    }

    /// Sample one site from the recorded crossings, uniformly.
    ///
    /// # Errors
    ///
    /// An empty set, or one that dropped crossings.
    ///
    /// **Dropping is fatal to a two-stage run, not a warning.** The recorded
    /// set is then a *truncated prefix* of the crossings — biased towards
    /// whatever the first histories did — and replaying it gives a
    /// systematically wrong second stage that looks converged. Upstream's cap
    /// has the same property; this refuses rather than leaving it to be
    /// noticed.
    pub fn sample(&self, seed: &mut u64) -> Result<SourceSite, String> {
        if self.crossings.is_empty() {
            return Err("this surface source recorded no crossings".into());
        }
        if self.dropped > 0 {
            return Err(format!(
                "this surface source hit its {} crossing cap and dropped {}. The kept \
                 set is a TRUNCATED PREFIX, biased towards whatever the first histories \
                 did, so replaying it gives a systematically wrong second stage that \
                 looks converged. Raise max_particles or record fewer surfaces.",
                self.max_particles, self.dropped
            ));
        }
        let i = ((prn(seed) * self.crossings.len() as f64) as usize).min(self.crossings.len() - 1);
        let c = self.crossings[i];
        Ok(SourceSite {
            r: c.r,
            u: c.u,
            e: c.energy,
            wgt: c.weight,
        })
    }
}

/// A source bank held as data — `FileSource` (`include/openmc/source.h:172`).
///
/// The **file** half belongs with the state-point writer in
/// `njoy-outram-park-fork` (see #271); this is the in-memory form that crosses
/// that boundary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileSource {
    /// The sites, sampled uniformly.
    pub sites: Vec<SourceSite>,
}

impl FileSource {
    /// Sample one site uniformly.
    ///
    /// # Errors
    ///
    /// An empty bank.
    pub fn sample(&self, seed: &mut u64) -> Result<SourceSite, String> {
        if self.sites.is_empty() {
            return Err("this file source holds no sites".into());
        }
        let i = ((prn(seed) * self.sites.len() as f64) as usize).min(self.sites.len() - 1);
        Ok(self.sites[i])
    }

    /// Total weight held — the same bookkeeping check as
    /// [`SurfaceSource::total_weight`].
    pub fn total_weight(&self) -> f64 {
        self.sites.iter().map(|s| s.wgt).sum()
    }
}

/// Where a sampled site must land for it to be accepted —
/// `settings::source_rejection_fraction` and the domain constraints.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceConstraints {
    /// Accept only sites in one of these cells. Empty = no cell constraint.
    pub cells: Vec<usize>,
    /// Accept only sites in one of these materials. Empty = no constraint.
    pub materials: Vec<usize>,
    /// Accept only sites in a fissionable material.
    pub fissionable_only: bool,
    /// Give up after this many rejections **per site**.
    ///
    /// Upstream expresses the same guard as a *fraction*; a count is used here
    /// because the failure it guards is "the constraint matches almost
    /// nothing", and a count is what a caller can reason about.
    pub max_rejections: usize,
}

impl SourceConstraints {
    /// Upstream's default: no constraint, and a generous rejection budget.
    pub fn none() -> Self {
        Self {
            max_rejections: 10_000,
            ..Default::default()
        }
    }

    /// Whether a site in `cell`/`material` is accepted.
    pub fn accepts(&self, cell: usize, material: Option<usize>, fissionable: bool) -> bool {
        if !self.cells.is_empty() && !self.cells.contains(&cell) {
            return false;
        }
        if !self.materials.is_empty() {
            match material {
                Some(m) if self.materials.contains(&m) => {}
                _ => return false,
            }
        }
        if self.fissionable_only && !fissionable {
            return false;
        }
        true
    }

    /// Whether any constraint is active.
    pub fn is_unconstrained(&self) -> bool {
        self.cells.is_empty() && self.materials.is_empty() && !self.fissionable_only
    }
}

/// Independent distributions per Cartesian coordinate —
/// `CartesianIndependent` (`src/distribution_spatial.cpp`).
///
/// The three fixed shapes this crate had (point, box, sphere) cannot express a
/// source that is, say, uniform in `x` and `y` but exponential in `z`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndependentSpatial {
    /// Distribution for each of `x`, `y`, `z`.
    pub axes: [ScalarSpatial; 3],
}

/// One coordinate's distribution.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarSpatial {
    /// Always this value.
    Fixed(f64),
    /// Uniform on `[lo, hi]`.
    Uniform { lo: f64, hi: f64 },
    /// A tabulated density on an ascending grid, sampled by inverting its
    /// piecewise-constant CDF.
    Tabulated { grid: Vec<f64>, density: Vec<f64> },
}

impl ScalarSpatial {
    /// Sample one value.
    pub fn sample(&self, seed: &mut u64) -> f64 {
        match self {
            ScalarSpatial::Fixed(v) => *v,
            ScalarSpatial::Uniform { lo, hi } => lo + (hi - lo) * prn(seed),
            ScalarSpatial::Tabulated { grid, density } => {
                // Bin probabilities are density x width; within a bin, uniform.
                let mut cdf = Vec::with_capacity(density.len());
                let mut acc = 0.0;
                for (i, d) in density.iter().enumerate() {
                    acc += d * (grid[i + 1] - grid[i]);
                    cdf.push(acc);
                }
                if !(acc > 0.0) {
                    return grid[0];
                }
                let xi = prn(seed) * acc;
                let i = cdf.partition_point(|&c| c <= xi).min(density.len() - 1);
                grid[i] + (grid[i + 1] - grid[i]) * prn(seed)
            }
        }
    }

    /// Reject a malformed tabulated distribution at construction.
    ///
    /// # Errors
    ///
    /// A grid shorter than two points, a density array that does not have one
    /// fewer entry than the grid, a non-ascending grid, a negative density, or
    /// an all-zero density.
    pub fn tabulated(grid: Vec<f64>, density: Vec<f64>) -> Result<Self, String> {
        if grid.len() < 2 {
            return Err(format!("a tabulated axis needs at least 2 grid points, got {}", grid.len()));
        }
        if density.len() + 1 != grid.len() {
            return Err(format!(
                "a {}-point grid defines {} bins, but {} densities were given",
                grid.len(),
                grid.len() - 1,
                density.len()
            ));
        }
        if !grid.windows(2).all(|w| w[1] > w[0]) {
            return Err("the grid must be strictly ascending".into());
        }
        if density.iter().any(|&d| d < 0.0) {
            return Err("a negative density is not a distribution".into());
        }
        if !(density.iter().sum::<f64>() > 0.0) {
            return Err("an all-zero density samples nothing".into());
        }
        Ok(ScalarSpatial::Tabulated { grid, density })
    }
}

impl IndependentSpatial {
    /// Sample a position.
    pub fn sample(&self, seed: &mut u64) -> Position {
        Position::new(
            self.axes[0].sample(seed),
            self.axes[1].sample(seed),
            self.axes[2].sample(seed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tally::mesh::RegularMesh;

    fn mesh() -> MeshKind {
        MeshKind::Regular(RegularMesh {
            lower_left: [0.0, 0.0, 0.0],
            upper_right: [2.0, 1.0, 1.0],
            dimension: [2, 1, 1],
        })
    }

    /// **Mesh sampling reproduces the requested per-element strengths** —
    /// #264's acceptance criterion for `MeshSource`.
    ///
    /// # Result, 2026-09-22
    ///
    /// A 3:1 split over two elements, 200 000 samples: printed at run time,
    /// gated at 4 sigma on the binomial.
    #[test]
    fn mesh_sampling_reproduces_the_requested_strengths() {
        let s = MeshSource::new(mesh(), vec![3.0, 1.0], 2.0e6).unwrap();
        assert_eq!(s.total_strength(), 4.0);
        const N: usize = 200_000;
        let mut seed = 0x0264;
        let mut in_first = 0usize;
        for _ in 0..N {
            let site = s.sample(&mut seed).unwrap();
            // Element 0 is x in [0, 1), element 1 is x in [1, 2).
            if site.r.x < 1.0 {
                in_first += 1;
            }
            assert!((0.0..2.0).contains(&site.r.x), "x = {}", site.r.x);
            assert!((0.0..1.0).contains(&site.r.y));
            assert_eq!(site.e, 2.0e6);
        }
        let p = in_first as f64 / N as f64;
        let want = 0.75;
        let sigma = (want * (1.0 - want) / N as f64).sqrt();
        let dev = (p - want).abs() / sigma;
        println!("element 0 share {p:.5} vs requested {want} ({dev:.2} sigma)");
        assert!(dev < 4.0, "strengths not reproduced: {dev:.2} sigma");
    }

    /// Malformed mesh sources are refused, including the all-zero one that
    /// would emit nothing while looking like a source.
    #[test]
    fn malformed_mesh_sources_are_refused() {
        assert!(MeshSource::new(mesh(), vec![1.0], 1.0).is_err());
        assert!(MeshSource::new(mesh(), vec![1.0, -1.0], 1.0).is_err());
        let err = MeshSource::new(mesh(), vec![0.0, 0.0], 1.0).unwrap_err();
        assert!(err.contains("emits nothing"), "{err}");
    }

    /// A mesh with no uniform point sampler is refused, not sampled as a box.
    #[test]
    fn a_curvilinear_mesh_source_is_refused() {
        use crate::tally::mesh::SphericalMesh;
        let s = MeshSource::new(
            MeshKind::Spherical(SphericalMesh {
                r_grid: vec![0.0, 1.0],
                theta_grid: vec![0.0, std::f64::consts::PI],
                phi_grid: vec![0.0, std::f64::consts::TAU],
                origin: Position::new(0.0, 0.0, 0.0),
            }),
            vec![1.0],
            1.0e6,
        )
        .unwrap();
        let err = s.sample(&mut 1).unwrap_err();
        assert!(err.contains("no uniform point sampler"), "{err}");
    }

    /// **A surface source carries weight, and replaying it preserves the
    /// total.** This is the bookkeeping a two-stage run depends on.
    #[test]
    fn a_surface_source_preserves_the_weight_it_recorded() {
        let mut ss = SurfaceSource::recording(vec![3], 100);
        for i in 0..10 {
            ss.record(SurfaceCrossing {
                r: Position::new(i as f64, 0.0, 0.0),
                u: Direction::new(1.0, 0.0, 0.0),
                energy: 1.0e6,
                weight: 0.5,
                surface_idx: 3,
            });
            // A crossing on an unwatched surface must not be recorded.
            ss.record(SurfaceCrossing {
                r: Position::new(i as f64, 0.0, 0.0),
                u: Direction::new(1.0, 0.0, 0.0),
                energy: 1.0e6,
                weight: 99.0,
                surface_idx: 4,
            });
        }
        assert_eq!(ss.crossings.len(), 10);
        assert!((ss.total_weight() - 5.0).abs() < 1e-12);

        // Replayed sites carry the recorded weight, not 1.0.
        let mut seed = 7;
        for _ in 0..50 {
            let site = ss.sample(&mut seed).unwrap();
            assert_eq!(
                site.wgt, 0.5,
                "a replayed site must carry its recorded weight; unit weight would \
                 inflate stage two by exactly what stage one removed"
            );
        }
    }

    /// **A truncated surface source is refused, not replayed.**
    ///
    /// The kept set is a prefix biased towards whatever the first histories
    /// did, so replaying it gives a systematically wrong second stage that
    /// looks converged.
    #[test]
    fn a_truncated_surface_source_refuses_to_be_replayed() {
        let mut ss = SurfaceSource::recording(vec![], 3);
        for i in 0..10 {
            ss.record(SurfaceCrossing {
                r: Position::new(i as f64, 0.0, 0.0),
                u: Direction::new(0.0, 0.0, 1.0),
                energy: 1.0e6,
                weight: 1.0,
                surface_idx: i,
            });
        }
        assert_eq!(ss.crossings.len(), 3);
        assert_eq!(ss.dropped, 7);
        let err = ss.sample(&mut 1).unwrap_err();
        assert!(err.contains("TRUNCATED PREFIX"), "{err}");

        // Empty is refused too.
        assert!(SurfaceSource::default().sample(&mut 1).is_err());
    }

    /// A file source replays its sites with their weights.
    #[test]
    fn a_file_source_replays_its_sites() {
        let fs = FileSource {
            sites: (0..4)
                .map(|i| SourceSite {
                    r: Position::new(i as f64, 0.0, 0.0),
                    u: Direction::new(1.0, 0.0, 0.0),
                    e: 1.0e6,
                    wgt: 0.25,
                })
                .collect(),
        };
        assert!((fs.total_weight() - 1.0).abs() < 1e-12);
        let mut seed = 3;
        let mut seen = [false; 4];
        for _ in 0..500 {
            let s = fs.sample(&mut seed).unwrap();
            assert_eq!(s.wgt, 0.25);
            seen[s.r.x as usize] = true;
        }
        assert!(seen.iter().all(|&b| b), "every site should be reachable");
        assert!(FileSource::default().sample(&mut 1).is_err());
    }

    /// Constraints accept and reject on every axis independently.
    #[test]
    fn constraints_apply_on_every_axis() {
        assert!(SourceConstraints::none().is_unconstrained());
        assert!(SourceConstraints::none().accepts(9, None, false));

        let c = SourceConstraints {
            cells: vec![1, 2],
            materials: vec![5],
            fissionable_only: true,
            max_rejections: 100,
        };
        assert!(!c.is_unconstrained());
        assert!(c.accepts(1, Some(5), true));
        assert!(!c.accepts(3, Some(5), true), "wrong cell");
        assert!(!c.accepts(1, Some(6), true), "wrong material");
        assert!(!c.accepts(1, None, true), "a void has no material to match");
        assert!(!c.accepts(1, Some(5), false), "not fissionable");
    }

    /// **The per-coordinate spatial reproduces its requested shape.**
    ///
    /// A source uniform in `x` and `y` but with a 3:1 tabulated split in `z`
    /// cannot be expressed by any of the three fixed shapes this crate had.
    #[test]
    fn an_independent_spatial_reproduces_a_per_axis_shape() {
        let s = IndependentSpatial {
            axes: [
                ScalarSpatial::Uniform { lo: -1.0, hi: 1.0 },
                ScalarSpatial::Fixed(0.5),
                ScalarSpatial::tabulated(vec![0.0, 1.0, 2.0], vec![3.0, 1.0]).unwrap(),
            ],
        };
        const N: usize = 200_000;
        let mut seed = 0x264_264;
        let (mut sum_x, mut lower_z) = (0.0_f64, 0usize);
        for _ in 0..N {
            let p = s.sample(&mut seed);
            assert_eq!(p.y, 0.5);
            assert!((-1.0..1.0).contains(&p.x));
            assert!((0.0..2.0).contains(&p.z));
            sum_x += p.x;
            if p.z < 1.0 {
                lower_z += 1;
            }
        }
        // x uniform on [-1, 1]: mean 0, sd 1/sqrt(3).
        let sigma_x = (1.0 / 3.0_f64 / N as f64).sqrt();
        assert!(
            (sum_x / N as f64).abs() / sigma_x < 4.0,
            "x mean {} is not 0",
            sum_x / N as f64
        );
        let p = lower_z as f64 / N as f64;
        let sigma = (0.75 * 0.25 / N as f64).sqrt();
        println!("lower-z share {p:.5} vs requested 0.75 ({:.2} sigma)", (p - 0.75).abs() / sigma);
        assert!((p - 0.75).abs() / sigma < 4.0, "z split not reproduced");
    }

    /// Malformed tabulated axes are refused.
    #[test]
    fn malformed_tabulated_axes_are_refused() {
        assert!(ScalarSpatial::tabulated(vec![0.0], vec![]).is_err());
        assert!(ScalarSpatial::tabulated(vec![0.0, 1.0], vec![1.0, 1.0]).is_err());
        assert!(ScalarSpatial::tabulated(vec![1.0, 0.0], vec![1.0]).is_err());
        assert!(ScalarSpatial::tabulated(vec![0.0, 1.0], vec![-1.0]).is_err());
        assert!(ScalarSpatial::tabulated(vec![0.0, 1.0], vec![0.0]).is_err());
    }
}
