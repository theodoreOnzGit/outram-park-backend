// SPDX-License-Identifier: GPL-3.0

//! **Tally filters added for GitHub #261**, ported from `src/tallies/` at
//! OpenMC `afa7a14`. (The commit the issue cites, `608a1c33`, is unavailable
//! here and not fetchable; see
//! `outram-mc-libs/verification_and_validation/white_boundary/`.)
//!
//! Covers, of the sixteen the issue lists:
//!
//! | filter | upstream | status |
//! |---|---|---|
//! | `ENERGY_FUNCTION` | `filter_energyfunc.cpp` | here |
//! | `REACTION` | `filter_reaction.cpp` | here, with full MT summation |
//! | `COLLISION` | `filter_collision.cpp` | here |
//! | `CELLFROM` | `filter_cellfrom.cpp` | here |
//! | `CELLBORN` | `filter_cellborn.cpp` | here |
//! | `MATERIALFROM` | `filter_materialfrom.cpp` | here |
//! | `WEIGHT` | `filter_weight.cpp` | here |
//! | `MUSURFACE` | `filter_musurface.cpp` | here |
//!
//! **Not here**, and still open on #261: `DISTRIBCELL`, `CELL_INSTANCE`,
//! `MESH_SURFACE`, `MESHBORN`, `MESH_MATERIAL`, `LEGENDRE`, `PARENT_NUCLIDE`
//! and `PARTICLE_PRODUCTION`. `DISTRIBCELL` is the expensive one — it needs the
//! distribcell offset tables upstream builds in `src/geometry_aux.cpp`.

use super::filter::{Filter, FilterEvent};

// ── MT summation rules (src/endf.cpp) ───────────────────────────────────────

/// `is_fission` (`src/endf.cpp:53`).
pub fn is_fission(mt: i32) -> bool {
    // N_FISSION, N_F, N_NF, N_2NF, N_3NF
    matches!(mt, 18 | 19 | 20 | 21 | 38)
}

/// `is_disappearance` (`src/endf.cpp:59`).
pub fn is_disappearance(mt: i32) -> bool {
    // N_DISAPPEAR..=N_DA, then the discrete charged-particle levels, then the
    // named multi-particle channels.
    (101..=117).contains(&mt)
        || (600..=849).contains(&mt)
        // N_TA, N_DT, N_P3HE, N_D3HE, N_3HEA, N_3P
        || matches!(mt, 155 | 182 | 191 | 192 | 193 | 197)
}

/// `mt_matches(event_mt, target_mt)` (`src/endf.cpp:90`) — does an event's MT
/// fall under a (possibly **summation**) target MT?
///
/// # Why this is ported in full rather than reduced to equality
///
/// A reaction filter asking for MT=4 means *every* inelastic level, 51 through
/// 91 — not a reaction literally labelled 4, which no event ever carries.
/// Likewise MT=1 is total, MT=18 covers the first-, second- and third-chance
/// fission channels, and MT=103 covers the 600–649 discrete `(n,p)` levels.
///
/// Reducing this to `event_mt == target_mt` would compile, run, and tally
/// **zero** for every summation MT anyone actually asks for. A silent zero in a
/// reaction-rate tally is the same failure shape as the aliased boundary
/// condition in #259: plausible output, different question.
pub fn mt_matches(event_mt: i32, target_mt: i32) -> bool {
    if event_mt == target_mt {
        return true;
    }
    match target_mt {
        // TOTAL_XS
        1 => event_mt == 2 || mt_matches(event_mt, 3),
        // N_NONELASTIC
        3 => {
            const COMPONENTS: [i32; 66] = [
                4, 5, 11, 16, 17, 22, 23, 24, 25, 27, 28, 29, 30, 32, 33, 34, 35, 36, 37, 41,
                42, 44, 45, 152, 153, 154, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165,
                166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180,
                181, 183, 184, 185, 186, 187, 188, 189, 190, 194, 195, 196, 198, 199, 200,
            ];
            COMPONENTS.iter().any(|&mt| mt_matches(event_mt, mt))
        }
        // N_LEVEL: inelastic scattering levels, 50..=N_NC
        4 => (50..=91).contains(&event_mt),
        // N_2N to excited states
        16 => (875..=891).contains(&event_mt),
        // N_FISSION
        18 => is_fission(event_mt),
        27 => is_fission(event_mt) || is_disappearance(event_mt),
        // N_DISAPPEAR
        101 => is_disappearance(event_mt),
        // Discrete charged-particle level bands.
        103 => (600..=649).contains(&event_mt),
        104 => (650..=699).contains(&event_mt),
        105 => (700..=749).contains(&event_mt),
        106 => (750..=799).contains(&event_mt),
        107 => (800..=849).contains(&event_mt),
        // Photon: total = coherent + incoherent + pair + photoelectric.
        501 => {
            event_mt == 502
                || event_mt == 504
                || mt_matches(event_mt, 516)
                || mt_matches(event_mt, 522)
        }
        // PAIR_PROD = electron-field + nuclear-field
        516 => event_mt == 515 || event_mt == 517,
        // PHOTOELECTRIC: subshells
        522 => (534..573).contains(&event_mt),
        _ => false,
    }
}

// ── Filters ─────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]

/// Bin by the **MT number** of the event. `openmc::ReactionFilter`.
///
/// An event may match **several** bins at once when the requested MTs overlap
/// (asking for both 1 and 2, say), which is why upstream scans every bin rather
/// than looking one up. Ported as such: see [`ReactionFilter::matching_bins`].
pub struct ReactionFilter {
    /// Requested MT numbers, one bin each, in order.
    pub bins: Vec<i32>,
}

impl ReactionFilter {
    /// Every bin this event falls into. Upstream pushes one match per bin
    /// (`filter_reaction.cpp:53`), so overlapping MTs each score.
    pub fn matching_bins(&self, event_mt: i32) -> Vec<usize> {
        self.bins
            .iter()
            .enumerate()
            .filter(|(_, &mt)| mt_matches(event_mt, mt))
            .map(|(i, _)| i)
            .collect()
    }
}

impl Filter for ReactionFilter {
    fn n_bins(&self) -> usize {
        self.bins.len()
    }
    /// The single-bin contract returns the **first** matching bin. A tally that
    /// needs the overlapping case must use [`Self::matching_bins`]; this is the
    /// same accommodation the expansion filters already make in this crate.
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.matching_bins(ev.event_mt).into_iter().next()
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the particle's **collision number**. `openmc::CollisionFilter`.
///
/// Upstream requires an **exact** match against the requested numbers
/// (`filter_collision.cpp:45`): a filter asking for `[1, 2, 5]` bins the first,
/// second and fifth collisions and drops every other. It is not a range.
pub struct CollisionFilter {
    pub bins: Vec<u32>,
}

impl Filter for CollisionFilter {
    fn n_bins(&self) -> usize {
        self.bins.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.bins.iter().position(|&n| n == ev.n_collision)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the cell the particle came **from**. `openmc::CellFromFilter`.
pub struct CellFromFilter {
    pub cells: Vec<usize>,
}

impl Filter for CellFromFilter {
    fn n_bins(&self) -> usize {
        self.cells.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        ev.cell_from
            .and_then(|c| self.cells.iter().position(|&x| x == c))
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the cell the particle was **born** in. `openmc::CellBornFilter`.
pub struct CellBornFilter {
    pub cells: Vec<usize>,
}

impl Filter for CellBornFilter {
    fn n_bins(&self) -> usize {
        self.cells.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        ev.cell_born
            .and_then(|c| self.cells.iter().position(|&x| x == c))
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the material the particle came **from**.
/// `openmc::MaterialFromFilter`.
pub struct MaterialFromFilter {
    pub materials: Vec<usize>,
}

impl Filter for MaterialFromFilter {
    fn n_bins(&self) -> usize {
        self.materials.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        ev.material_from
            .and_then(|m| self.materials.iter().position(|&x| x == m))
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the particle's **weight**. `openmc::WeightFilter`.
///
/// Only meaningful once variance reduction exists (#258) — in analog transport
/// every weight is exactly 1 and every event lands in whichever bin contains 1.
/// Ported now so the filter is not the thing blocking that work.
///
/// `bins` are ascending edges; a weight outside `[first, last]` is unbinned.
pub struct WeightFilter {
    pub bins: Vec<f64>,
}

impl Filter for WeightFilter {
    fn n_bins(&self) -> usize {
        self.bins.len().saturating_sub(1)
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let w = ev.weight;
        if self.bins.len() < 2 || w < self.bins[0] || w > self.bins[self.bins.len() - 1] {
            return None;
        }
        let mut lo = 0usize;
        let mut hi = self.bins.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.bins[mid] <= w {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Bin by the cosine between the particle direction and the **surface normal**
/// at a surface crossing. `openmc::MuSurfaceFilter`.
///
/// # How this differs from `MuFilter`, which this crate already has
///
/// `MuFilter` bins a **scattering** cosine — the change of direction at a
/// collision. This bins the angle of incidence **on a surface**, which is a
/// different quantity measured at a different event. Conflating them would
/// silently tally scattering angles into a surface-current tally.
///
/// Upstream flips the normal when the particle crosses the surface from the
/// negative side (`p.surface() < 0`, `filter_musurface.cpp:20`) so `mu` is
/// always measured against the normal the particle actually sees, and clamps
/// `|mu| > 1` to `+/-1` against round-off.
pub struct MuSurfaceFilter {
    /// Ascending cosine edges, normally spanning `[-1, 1]`.
    pub bins: Vec<f64>,
}

impl Filter for MuSurfaceFilter {
    fn n_bins(&self) -> usize {
        self.bins.len().saturating_sub(1)
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        if ev.surface_idx == usize::MAX {
            return None; // not a surface-crossing event
        }
        let mu = ev.surface_mu.clamp(-1.0, 1.0);
        if self.bins.len() < 2 || mu < self.bins[0] || mu > self.bins[self.bins.len() - 1] {
            return None;
        }
        let mut lo = 0usize;
        let mut hi = self.bins.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.bins[mid] <= mu {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// **Arbitrary response function of energy** — the ICRP flux-to-dose filter.
/// `openmc::EnergyFunctionFilter`.
///
/// One bin, whose **weight** is `y(E)` interpolated at the event's incoming
/// energy. Events outside `[energy.first(), energy.last()]` are dropped
/// entirely rather than clamped (`filter_energyfunc.cpp:96`) — extrapolating a
/// dose-response curve past its tabulated range is not a thing upstream will
/// do, and neither does this.
///
/// This is the filter the dose end of the #226 source-term chain needs.
///
/// Only **lin-lin** interpolation is ported. Upstream carries the full ENDF
/// interpolation set here; anything else is refused by
/// [`EnergyFunctionFilter::new`] rather than silently treated as lin-lin.
pub struct EnergyFunctionFilter {
    energy: Vec<f64>,
    y: Vec<f64>,
}

impl EnergyFunctionFilter {
    /// Build from an ascending energy grid and its response values.
    ///
    /// # Errors
    ///
    /// Grids of different lengths, fewer than two points, or a non-ascending
    /// energy grid.
    pub fn new(energy: Vec<f64>, y: Vec<f64>) -> Result<Self, String> {
        if energy.len() != y.len() {
            return Err(format!(
                "energy has {} points and y has {}",
                energy.len(),
                y.len()
            ));
        }
        if energy.len() < 2 {
            return Err("an energy-function filter needs at least two points".to_string());
        }
        if energy.windows(2).any(|w| w[1] <= w[0]) {
            return Err("the energy grid must be strictly ascending".to_string());
        }
        Ok(Self { energy, y })
    }

    /// The interpolated response at `e`, or `None` outside the grid.
    pub fn response(&self, e: f64) -> Option<f64> {
        if e < self.energy[0] || e > self.energy[self.energy.len() - 1] {
            return None;
        }
        let mut lo = 0usize;
        let mut hi = self.energy.len() - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.energy[mid] <= e {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let (e0, e1) = (self.energy[lo], self.energy[lo + 1]);
        let (y0, y1) = (self.y[lo], self.y[lo + 1]);
        if e1 == e0 {
            return Some(y0);
        }
        Some(y0 + (e - e0) * (y1 - y0) / (e1 - e0))
    }
}

impl Filter for EnergyFunctionFilter {
    fn n_bins(&self) -> usize {
        1
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.response(ev.energy).map(|_| 0)
    }
    /// One bin carrying the interpolated response as its weight. The crate's
    /// `expansion_moments` contract is a weight per bin, so a single-bin filter
    /// returns a one-element vector.
    fn expansion_moments(&self, ev: &FilterEvent) -> Option<Vec<f64>> {
        self.response(ev.energy).map(|w| vec![w])
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GitHub #261, second batch
// ═══════════════════════════════════════════════════════════════════════════

use crate::particle::particle::ParticleType;
use crate::tally::mesh::MeshKind;

/// **Legendre moment filter** — `LegendreFilter`
/// (`src/tallies/filter_legendre.cpp`) at OpenMC `afa7a14`.
///
/// Deposits into **every** moment bin at once with weight `P_n(mu)`, where
/// `mu` is the scattering cosine. It is a functional expansion, not a binning:
/// see [`crate::tally::filter::FilterKind::is_expansion`].
///
/// # What it is for
///
/// This is the tally that MGXS generation is built on. `Sigma_s,l,g->g'` is
/// the `l`-th Legendre moment of the scattering kernel, and this filter
/// crossed with an [`crate::tally::filter::EnergyFilter`] and an
/// [`crate::tally::filter::EnergyOutFilter`] is exactly how it is measured.
/// Without it a generated library can only ever be P0 — which is the defect
/// GitHub #265 priced at **−4371 pcm** on a leakage-dominated case.
///
/// # Why the weights are NOT `(l + 1/2) P_l`
///
/// `calc_pn_c` (`src/math_functions.cpp`) returns the bare `P_l(mu)`; the
/// `(l + 1/2)` normalisation belongs to *evaluating* an expansion, not to
/// accumulating its moments (see
/// [`crate::physics::scattdata::evaluate_legendre`], which applies it on the
/// way out). Folding it in here would double-apply it and silently rescale
/// every moment above P0 — a mistake that leaves the P0 term looking correct,
/// so a smoke test would not catch it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendreFilter {
    /// Highest moment; produces `order + 1` bins.
    pub order: usize,
}

impl Filter for LegendreFilter {
    fn n_bins(&self) -> usize {
        self.order + 1
    }
    fn get_bin(&self, _ev: &FilterEvent) -> Option<usize> {
        // An expansion filter has no single bin; the scoring path uses
        // `expansion_moments`.
        None
    }
    fn expansion_moments(&self, ev: &FilterEvent) -> Option<Vec<f64>> {
        // `calc_pn_c`: P_0 = 1, P_1 = mu, Bonnet recursion above.
        let mut p = vec![0.0; self.order + 1];
        p[0] = 1.0;
        if self.order >= 1 {
            p[1] = ev.mu;
        }
        for l in 1..self.order {
            let lf = l as f64;
            p[l + 1] = ((2.0 * lf + 1.0) * ev.mu * p[l] - lf * p[l - 1]) / (lf + 1.0);
        }
        Some(p)
    }
}

/// **Born-position mesh filter** — `MeshBornFilter`
/// (`src/tallies/filter_meshborn.cpp`).
///
/// Bins by the mesh element the particle was **born** in, not the one it is
/// in now. That is what separates "flux here" from "flux here *due to a source
/// there*", which is the quantity a source-importance or adjoint-like study
/// needs.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBornFilter {
    /// The mesh; `n_bins` is its element count.
    pub mesh: MeshKind,
}

impl Filter for MeshBornFilter {
    fn n_bins(&self) -> usize {
        self.mesh.n_bins()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.mesh.bin(ev.position_born)
    }
}

/// **Parent-nuclide filter** — `ParentNuclideFilter`
/// (`src/tallies/filter_parent_nuclide.cpp`).
///
/// Bins by which nuclide the particle descends from. An event whose parent is
/// unknown, or is not in the list, matches nothing — it is **not** folded into
/// a catch-all bin, because a decay-source study that silently attributed
/// unknown parents to one nuclide would be reporting a fabricated spectrum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentNuclideFilter {
    /// Nuclide indices, in bin order.
    pub nuclides: Vec<usize>,
}

impl Filter for ParentNuclideFilter {
    fn n_bins(&self) -> usize {
        self.nuclides.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let parent = ev.parent_nuclide?;
        self.nuclides.iter().position(|&n| n == parent)
    }
}

/// **Cell-instance filter** — `CellInstanceFilter`
/// (`src/tallies/filter_cell_instance.cpp`).
///
/// Bins explicit `(cell, instance)` pairs — the targeted form of a distribcell
/// tally. This is what gives a per-pebble or per-pin result out of a repeated
/// universe.
///
/// # Scope note
///
/// Upstream also walks the coordinate stack so that an *enclosing* cell can
/// match, and has a `material_cells_only_` switch for that. This port matches
/// the **lowest** coordinate level only, which is the `material_cells_only_`
/// behaviour, because this crate's `FilterEvent` carries the leaf cell rather
/// than the whole stack. A filter listing a non-leaf cell therefore matches
/// nothing here where upstream would match it — stated rather than left to be
/// discovered, and the reason [`Self::new`] cannot detect it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellInstanceFilter {
    /// `(cell index, instance)` pairs, in bin order.
    pub pairs: Vec<(usize, usize)>,
}

impl Filter for CellInstanceFilter {
    fn n_bins(&self) -> usize {
        self.pairs.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let inst = ev.cell_instance?;
        self.pairs
            .iter()
            .position(|&(c, i)| c == ev.cell_idx && i == inst)
    }
}

/// **Mesh-and-material filter** — `MeshMaterialFilter`
/// (`src/tallies/filter_meshmaterial.cpp`).
///
/// Bins `(mesh element, material)` pairs, for a homogenised-region tally where
/// one mesh cell contains more than one material and the split matters.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshMaterialFilter {
    /// The mesh.
    pub mesh: MeshKind,
    /// `(element, material)` pairs, in bin order.
    pub pairs: Vec<(usize, usize)>,
}

impl Filter for MeshMaterialFilter {
    fn n_bins(&self) -> usize {
        self.pairs.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let element = self.mesh.bin(ev.position)?;
        self.pairs
            .iter()
            .position(|&(e, m)| e == element && m == ev.material_idx)
    }
}

/// **Particle-production filter** — `ParticleProductionFilter`
/// (`src/tallies/filter_particle_production.cpp`).
///
/// Scores the **secondaries a collision produced**, by particle type and
/// optionally by their birth energy, each at its own weight. So unlike every
/// other filter here it can match an event **more than once**, which is why it
/// reports [`Self::matches`] rather than a single bin.
///
/// With `energy_bins` empty there is one bin per particle type; otherwise
/// `particle_index * n_energy + energy_index`, matching upstream's layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleProductionFilter {
    /// Particle types, in bin order.
    pub particles: Vec<ParticleType>,
    /// Ascending energy bounds \[eV\], or empty for no energy resolution.
    pub energy_bins: Vec<f64>,
}

impl ParticleProductionFilter {
    /// Number of energy bins (1 when unresolved).
    fn n_energy(&self) -> usize {
        if self.energy_bins.len() < 2 {
            1
        } else {
            self.energy_bins.len() - 1
        }
    }

    /// Every `(bin, weight)` this event produces — possibly none, possibly
    /// several.
    pub fn matches(&self, ev: &FilterEvent) -> Vec<(usize, f64)> {
        let mut out = Vec::new();
        for s in &ev.secondaries {
            let Some(pi) = self.particles.iter().position(|&p| p == s.particle) else {
                continue;
            };
            if self.energy_bins.len() < 2 {
                out.push((pi, s.weight));
                continue;
            }
            if s.energy < self.energy_bins[0] || s.energy > *self.energy_bins.last().unwrap() {
                continue;
            }
            let ei = self
                .energy_bins
                .partition_point(|&b| b <= s.energy)
                .saturating_sub(1)
                .min(self.n_energy() - 1);
            out.push((pi * self.n_energy() + ei, s.weight));
        }
        out
    }
}

impl Filter for ParticleProductionFilter {
    fn n_bins(&self) -> usize {
        self.particles.len() * self.n_energy()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        // A single-bin answer is genuinely wrong for this filter when a
        // collision produced several secondaries. The first match is returned
        // so a caller using the common `get_bin` path is not silently given
        // nothing, but `matches` is the correct entry point and the doc says
        // so rather than leaving the partial answer to be discovered.
        self.matches(ev).first().map(|&(b, _)| b)
    }
}

/// **Mesh-surface (current) filter** — `MeshSurfaceFilter`
/// (`src/tallies/filter_meshsurface.cpp`).
///
/// Bins the **faces** a track crosses rather than the elements it passes
/// through, giving a current rather than a flux. That is the quantity a
/// CMFD-style acceleration or a nodal coupling needs, and it is not
/// recoverable from a flux tally.
///
/// One event can cross many faces, so like
/// [`ParticleProductionFilter`] this filter reports [`Self::matches`] rather
/// than a single bin; `get_bin` returns only the first crossing and its doc
/// says so instead of leaving the partial answer to be found later.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSurfaceFilter {
    /// The mesh whose faces are binned. Only [`MeshKind::Regular`] is
    /// supported — the cylindrical and spherical meshes carry no face
    /// crossing routine yet, and [`Self::new`] refuses them rather than
    /// returning an empty bin list that reads as "this track crossed nothing".
    pub mesh: crate::tally::mesh::RegularMesh,
}

impl MeshSurfaceFilter {
    /// Build from a mesh.
    ///
    /// # Errors
    ///
    /// A non-regular mesh. See the field docs for why that is an error rather
    /// than a silent zero.
    pub fn new(mesh: MeshKind) -> Result<Self, String> {
        match mesh {
            MeshKind::Regular(m) => Ok(Self { mesh: m }),
            other => Err(format!(
                "mesh-surface currents need a regular mesh; {} has no face-crossing \
                 routine ported yet, and returning no crossings would read as a track \
                 that crossed nothing",
                match other {
                    MeshKind::Rectilinear(_) => "a rectilinear mesh",
                    MeshKind::Cylindrical(_) => "a cylindrical mesh",
                    MeshKind::Spherical(_) => "a spherical mesh",
                    MeshKind::Regular(_) => unreachable!(),
                }
            )),
        }
    }

    /// Every surface bin this track crosses, in order of travel.
    pub fn matches(&self, ev: &FilterEvent) -> Vec<usize> {
        self.mesh.surface_bins_crossed(ev.position_last, ev.position)
    }
}

impl Filter for MeshSurfaceFilter {
    fn n_bins(&self) -> usize {
        self.mesh.n_surface_bins()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        // Genuinely partial when a track crosses several faces. `matches` is
        // the correct entry point.
        self.matches(ev).first().copied()
    }
}

/// **Distribcell filter** — one bin per instance of a repeated cell.
/// `DistribcellFilter` (`src/tallies/filter_distribcell.cpp`).
///
/// A cell defined once inside a universe that a lattice repeats 400 times is
/// one cell and 400 instances; a [`CellFilter`] bins all 400 together and this
/// gives 400 bins. See [`crate::geometry::distribcell`] for how the instance
/// index is formed.
///
/// # The instance must be supplied by the transport
///
/// This filter reads [`FilterEvent::cell_instance`], which the transport sets
/// from [`crate::geometry::distribcell::DistribcellOffsets::instance_of`]. An
/// event with no instance matches **nothing** rather than bin 0 — binning an
/// unknown instance into the first bin would put every un-instanced event on
/// one pebble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistribcellFilter {
    /// The repeated cell.
    pub cell_idx: usize,
    /// How many instances it has — from `DistribcellOffsets::n_instances`.
    pub n_instances: usize,
}

impl Filter for DistribcellFilter {
    fn n_bins(&self) -> usize {
        self.n_instances
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        if ev.cell_idx != self.cell_idx {
            return None;
        }
        let i = ev.cell_instance?;
        (i < self.n_instances).then_some(i)
    }
}
