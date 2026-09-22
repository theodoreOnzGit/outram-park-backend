/// Tally filters — constrain which phase-space events are scored.
///
/// C++ source: `src/tallies/filter_*.cpp` (30+ files), `include/openmc/tallies/filter.h`.
///
/// Filters work as a conjunction: a particle event is scored only if it passes
/// ALL filters attached to a tally.  Each filter maps the event to a bin index.
///
/// Implemented here: Cell, Material, Energy, Universe, Mesh
/// ([`super::mesh::RegularMesh`]), Surface, Mu, PolarAzimuthal, Time, Particle,
/// DelayedGroup, and the functional expansions [`SpatialLegendreFilter`],
/// [`ZernikeFilter`] and [`SphericalHarmonicsFilter`].
///
/// # Dispatch is by enum, not by trait object (2026-09-16)
///
/// [`FilterKind`] is what a [`super::tally::Tally`] stores. The [`Filter`] trait
/// remains as the **compiler-enforced contract** each concrete filter satisfies,
/// which is exactly the split the workspace's Rust design rules prescribe:
/// traits for the contract, enums for dispatch. `Tally` previously held
/// `Vec<Box<dyn Filter>>`, which violated both the "no trait objects" and "no
/// `Box<T>`" rules and cost the exhaustiveness check that makes adding a filter
/// safe.
use super::mesh::MeshKind;
use crate::geometry::position::{Direction, Position};
use crate::particle::particle::ParticleType;

/// Base trait for all filters.  Maps to `openmc::Filter`.
pub trait Filter: Send + Sync {
    /// Number of bins this filter produces.
    fn n_bins(&self) -> usize;

    /// Map particle state to a bin index, or `None` if the event doesn't match.
    fn get_bin(&self, event: &FilterEvent) -> Option<usize>;

    /// Functional-expansion weights: for an expansion filter (e.g.
    /// [`SpatialLegendreFilter`]) return the per-moment weights `[w_0, …, w_order]`
    /// deposited into bins `0..n_bins`, or `None` if the event is outside the
    /// filter's domain. All non-expansion filters return `None` (the default) and
    /// are handled by the single-bin [`Filter::get_bin`] path instead.
    ///
    /// Maps to the multi-`(bin, weight)` return of OpenMC's
    /// `Filter::get_all_bins` for expansion filters
    /// (`src/tallies/filter_sptl_legendre.cpp`, `get_all_bins`).
    fn expansion_moments(&self, _event: &FilterEvent) -> Option<Vec<f64>> {
        None
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Snapshot of particle state passed to filters at scoring time.
///
/// Every field a filter in this module needs lives here; a filter that wants
/// something absent cannot be written honestly, which is the point. The fields
/// below `position` were added on 2026-09-16 alongside the filters that consume
/// them — before that the struct could not express an angle, a time or a
/// particle type at all, so those filters could not have been written even as
/// stubs.
pub struct FilterEvent {
    pub cell_idx: usize,
    pub material_idx: usize,
    pub universe_idx: usize,
    pub energy: f64,
    /// Surface crossed (usize::MAX if not a surface-crossing event).
    pub surface_idx: usize,
    /// Start of the track segment \[cm\], for the filters that need the whole
    /// segment rather than a representative point —
    /// [`super::filter_extra::MeshSurfaceFilter`].
    pub position_last: Position,
    /// Representative spatial position of the event \[cm\] — the streamed
    /// segment's midpoint for the track-length estimator. Used by the spatial
    /// filters ([`MeshFilter`], [`SpatialLegendreFilter`], [`ZernikeFilter`]);
    /// ignored by the cell/material/universe/energy filters.
    pub position: Position,
    /// Direction of travel (unit vector). Consumed by
    /// [`PolarAzimuthalFilter`] and [`SphericalHarmonicsFilter`].
    pub direction: Direction,
    /// Change-of-direction cosine `mu` of a scattering event, in the
    /// **laboratory** frame. Meaningful only for a scatter; `MuFilter` is a
    /// collision-estimator filter and this is what it bins.
    pub mu: f64,
    /// Time since the particle was born \[s\]. Consumed by [`TimeFilter`].
    pub time: f64,
    /// Particle type. Consumed by [`ParticleFilter`].
    pub particle: ParticleType,
    /// Delayed-neutron precursor group of a fission event, `0`-based, or `None`
    /// for a prompt neutron or a non-fission event. Consumed by
    /// [`DelayedGroupFilter`].
    pub delayed_group: Option<usize>,
    /// Post-collision (outgoing) neutron energy \[eV\] of a scattering event,
    /// or `None` for any event that produced no secondary — an absorption, a
    /// surface crossing, or a pure track-length segment. Consumed by
    /// [`EnergyOutFilter`].
    ///
    /// This is deliberately separate from [`Self::energy`], which is always the
    /// **incoming** energy. A group-to-group scattering matrix needs both at
    /// once, so a tally carrying an [`EnergyFilter`] and an [`EnergyOutFilter`]
    /// bins `(g_in, g_out)` from a single event.
    pub energy_out: Option<f64>,

    // ── Added for GitHub #261 ───────────────────────────────────────────────
    /// MT number of the event. Consumed by
    /// [`super::filter_extra::ReactionFilter`]; `0` for an event that is not a
    /// reaction (a surface crossing, a track-length segment).
    pub event_mt: i32,
    /// Number of collisions this particle has had. Consumed by
    /// [`super::filter_extra::CollisionFilter`], which matches it EXACTLY --
    /// it is a set of collision numbers, not a range.
    pub n_collision: u32,
    /// Cell the particle came **from**, or `None` when it has no previous cell
    /// (its first event). [`super::filter_extra::CellFromFilter`].
    pub cell_from: Option<usize>,
    /// Cell the particle was **born** in.
    /// [`super::filter_extra::CellBornFilter`].
    pub cell_born: Option<usize>,
    /// Material the particle came **from**.
    /// [`super::filter_extra::MaterialFromFilter`].
    pub material_from: Option<usize>,
    /// Particle weight at the event. [`super::filter_extra::WeightFilter`].
    ///
    /// ~~**1.0 in analog transport**, which is every run today -- see #258. A
    /// weight filter is therefore not useful yet~~ **CORRECTED 2026-09-22** —
    /// #258 landed, so this is now a real varying weight whenever survival
    /// biasing or weight windows are on, and a weight filter bins something.
    /// It is still exactly 1.0 on an analog run, which is still the default.
    /// The field defaults to 1.0 rather than 0.0 so that a filter binning it
    /// does not silently drop every event against a `[0, 1]` grid.
    pub weight: f64,
    /// Cosine between the direction of travel and the **surface normal** at a
    /// surface crossing, with the normal already flipped to the side the
    /// particle came from. [`super::filter_extra::MuSurfaceFilter`].
    ///
    /// Deliberately separate from [`Self::mu`], which is a SCATTERING cosine at
    /// a collision. They are different quantities at different events and
    /// conflating them would tally scattering angles into a surface tally.
    pub surface_mu: f64,

    // ── Added for GitHub #261, second batch ────────────────────────────────
    /// Position the particle was **born** at \[cm\].
    /// [`super::filter_extra::MeshBornFilter`].
    pub position_born: Position,
    /// Instance number of [`Self::cell_idx`] within its repeated universe, or
    /// `None` outside a lattice. [`super::filter_extra::CellInstanceFilter`].
    pub cell_instance: Option<usize>,
    /// Nuclide this particle descends from.
    /// [`super::filter_extra::ParentNuclideFilter`].
    pub parent_nuclide: Option<usize>,
    /// Secondaries this collision produced.
    /// [`super::filter_extra::ParticleProductionFilter`].
    ///
    /// A `Vec` on a per-event struct looks expensive and is not: an empty
    /// `Vec` does not allocate, and every event that is not a
    /// secondary-producing collision leaves it empty. The alternative — a
    /// borrowed slice — would need a lifetime parameter on `FilterEvent`,
    /// which the workspace Rust rules forbid.
    pub secondaries: Vec<SecondarySite>,
}

/// One secondary particle emitted by a collision, as
/// [`super::filter_extra::ParticleProductionFilter`] sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondarySite {
    /// What was emitted.
    pub particle: ParticleType,
    /// Its birth energy \[eV\].
    pub energy: f64,
    /// Its statistical weight.
    pub weight: f64,
}

impl Default for FilterEvent {
    /// An event with no geometry, no angle, zero energy and zero time.
    ///
    /// Provided so a caller adding one field to a constructed event does not
    /// have to spell out the rest, and so a test can build the one field it
    /// cares about. **Not** a physically meaningful event: `cell_idx` and the
    /// other indices are `0`, which is a real cell, so a `Default` event passed
    /// to a `CellFilter` will match bin 0.
    fn default() -> Self {
        FilterEvent {
            cell_idx: 0,
            material_idx: 0,
            universe_idx: 0,
            energy: 0.0,
            surface_idx: usize::MAX,
            position_last: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            direction: Direction {
                u: 0.0,
                v: 0.0,
                w: 1.0,
            },
            mu: 0.0,
            time: 0.0,
            particle: ParticleType::Neutron,
            delayed_group: None,
            energy_out: None,
            event_mt: 0,
            n_collision: 0,
            cell_from: None,
            cell_born: None,
            material_from: None,
            position_born: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            cell_instance: None,
            parent_nuclide: None,
            secondaries: Vec::new(),
            // 1.0, not 0.0: analog transport has unit weight, and a weight
            // filter binning a default event against a [0, 1] grid would
            // otherwise drop it silently.
            weight: 1.0,
            surface_mu: 0.0,
        }
    }
}

// ── Concrete filters ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]

/// Filter by cell.  Maps to `openmc::CellFilter`.
///
/// **These are 0-based indices into the geometry's cell array, not cell IDs.**
/// OpenMC's C++ filters bin by user-assigned global ID; this port bins by
/// array position, matching [`FilterEvent::cell_idx`]. Passing an ID here
/// silently produces wrong bins rather than an error, so the distinction
/// matters more than it looks.
///
/// ```
/// use outram_mc_libs::prelude::*;
/// // the first and third cells of the geometry, not cells with IDs 1 and 3
/// let f = CellFilter { cell_indices: vec![0, 2] };
/// assert_eq!(f.n_bins(), 2);
/// ```
pub struct CellFilter {
    /// 0-based positions in the cell array. One tally bin per entry.
    pub cell_indices: Vec<usize>,
}
impl Filter for CellFilter {
    fn n_bins(&self) -> usize {
        self.cell_indices.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.cell_indices.iter().position(|&c| c == ev.cell_idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by material.  Maps to `openmc::MaterialFilter`.
///
/// **These are 0-based indices into the material array, not material IDs** --
/// same convention as [`CellFilter`], and the same silent-wrong-answer trap if
/// you pass an ID.
pub struct MaterialFilter {
    /// 0-based positions in the material array. One tally bin per entry.
    pub material_indices: Vec<usize>,
}
impl Filter for MaterialFilter {
    fn n_bins(&self) -> usize {
        self.material_indices.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.material_indices
            .iter()
            .position(|&m| m == ev.material_idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by energy bin (contiguous group boundaries in eV).
/// Maps to `openmc::EnergyFilter`.
///
/// **`bins` holds bin EDGES, not bin centres and not counts.** `n + 1`
/// ascending edges define `n` bins. The name is short for "bin boundaries";
/// read it as edges every time.
///
/// Energies outside `[bins[0], bins[last])` are not scored at all -- the event
/// is dropped, not clamped into the end bin.
///
/// ```
/// use outram_mc_libs::prelude::*;
/// // 3 edges -> 2 bins: [0, 1) MeV and [1, 20) MeV, in eV
/// let f = EnergyFilter { bins: vec![0.0, 1.0e6, 20.0e6] };
/// assert_eq!(f.n_bins(), 2);
/// ```
pub struct EnergyFilter {
    /// Ascending bin EDGES in eV. `n + 1` edges produce `n` bins.
    pub bins: Vec<f64>,
}
impl Filter for EnergyFilter {
    fn n_bins(&self) -> usize {
        if self.bins.len() < 2 {
            0
        } else {
            self.bins.len() - 1
        }
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        if ev.energy < self.bins[0] || ev.energy >= *self.bins.last().unwrap() {
            return None;
        }
        let idx = self
            .bins
            .partition_point(|&e| e <= ev.energy)
            .saturating_sub(1);
        Some(idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by **outgoing** (post-collision) energy. Maps to
/// `openmc::EnergyoutFilter`.
///
/// Bins [`FilterEvent::energy_out`], the secondary neutron's energy, where
/// [`EnergyFilter`] bins the incoming energy. Pairing the two on one tally is
/// what produces a group-to-group scattering matrix `Sigma_s,g->g'`, which is
/// the form the deterministic solvers consume (GeN-Foam's `ZoneNuclearData`
/// stores `scattering[moment][g_out][g_in]`).
///
/// An event with no secondary — an absorption, a surface crossing, or a pure
/// track-length segment — has `energy_out == None` and does **not** pass this
/// filter, so it contributes to no bin.
///
/// # Units
///
/// Bin edges are in **eV**, ascending, exactly as [`EnergyFilter`]. `n + 1`
/// edges produce `n` bins. Note that multigroup structures are conventionally
/// quoted with group 0 as the *highest* energy; this filter does not reorder,
/// so bin 0 is the lowest-energy bin and the caller reverses if it wants
/// group-index order.
pub struct EnergyOutFilter {
    /// Ascending bin EDGES in eV. `n + 1` edges produce `n` bins.
    pub bins: Vec<f64>,
}
impl Filter for EnergyOutFilter {
    fn n_bins(&self) -> usize {
        if self.bins.len() < 2 {
            0
        } else {
            self.bins.len() - 1
        }
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        // No secondary produced: this event is not a scatter, so it belongs in
        // no outgoing-energy bin at all.
        let e_out = ev.energy_out?;
        if self.bins.len() < 2 || e_out < self.bins[0] || e_out >= *self.bins.last().unwrap() {
            return None;
        }
        let idx = self.bins.partition_point(|&e| e <= e_out).saturating_sub(1);
        Some(idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by universe.  Maps to `openmc::UniverseFilter`.
pub struct UniverseFilter {
    pub universe_indices: Vec<usize>,
}
impl Filter for UniverseFilter {
    fn n_bins(&self) -> usize {
        self.universe_indices.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.universe_indices
            .iter()
            .position(|&u| u == ev.universe_idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by a regular spatial mesh.  Maps to `openmc::MeshFilter`.
///
/// Bins an event by the [`RegularMesh`] cell that contains its representative
/// position ([`FilterEvent::position`], the segment midpoint for the track-length
/// estimator). `n_bins` is the mesh cell count `nx·ny·nz`; a position outside the
/// mesh box is unbinned (`get_bin` returns `None`, so the tally drops it).
///
/// Ported from `src/tallies/filter_mesh.cpp` (`MeshFilter::get_all_bins`, the
/// non-track-length branch: `mesh->get_bin(r)`; a single bin, weight 1). The
/// track-length "bins crossed" sub-segmentation
/// (`StructuredMesh::bins_crossed`) is a documented gap (bead op-6tz.13) — this
/// port scores the whole segment into the midpoint's cell, which is exact for a
/// mesh whose cells are large relative to the mean free path.
/// **CHANGED 2026-09-22 (GitHub #260, scope item 4).** `mesh` was a concrete
/// [`RegularMesh`]; it is now a [`MeshKind`], so the same filter serves the
/// regular, rectilinear, cylindrical and spherical meshes. Enum dispatch rather
/// than a trait object, per the workspace Rust design rule.
///
/// A cylindrical mesh filter is what an R-Z power profile actually needs, and
/// before this it had to be faked through a Cartesian mesh (wrong bin shapes at
/// the radial edge) or hand-built CSG cells (no mesh filter at all).
pub struct MeshFilter {
    pub mesh: MeshKind,
}
impl Filter for MeshFilter {
    fn n_bins(&self) -> usize {
        self.mesh.n_bins()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.mesh.bin(ev.position)
    }
}

/// Axis along which a [`SpatialLegendreFilter`] expands the flux.
///
/// Maps to `openmc::LegendreAxis` (`include/openmc/tallies/filter_sptl_legendre.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendreAxis {
    /// Expand along the x coordinate.
    X,
    /// Expand along the y coordinate.
    Y,
    /// Expand along the z coordinate.
    Z,
}
#[derive(Debug, Clone, PartialEq)]

/// Functional-expansion (Legendre) filter along one Cartesian axis.
///
/// Maps to `openmc::SpatialLegendreFilter` (`src/tallies/filter_sptl_legendre.cpp`).
/// Instead of binning an event into one spatial cell, this expands the flux into
/// Legendre moments along `axis` over the interval `[min, max]` \[cm\]: the axis
/// coordinate `x` is normalized to `ξ = 2·(x − min)/(max − min) − 1 ∈ [−1, 1]`
/// and each moment bin `n ∈ 0..=order` receives weight `P_n(ξ)` (the Legendre
/// polynomial, `src/tallies/filter_sptl_legendre.cpp:76-87` via
/// `calc_pn_c`, `src/math_functions.cpp:105`).
///
/// The moment stored in bin `n` is therefore the *raw* moment
/// `∫ φ(ξ) P_n(ξ) dξ` (track-length weighted). The reconstruction normalization
/// `(2n+1)/2` is applied at flux *reconstruction* time
/// (`evaluate_legendre`, `src/math_functions.cpp:118-128`), **not** stored in the
/// filter weight — this port mirrors that convention exactly.
///
/// # Bins and scoring path
/// `n_bins = order + 1`. Because the base [`Filter`] contract maps to a single
/// bin, the moment expansion is deposited via [`Filter::expansion_moments`] (the
/// faithful multi-`(bin, weight)` analogue of OpenMC's `get_all_bins`), which the
/// scoring path in [`super::scoring::score_track_length`] handles for a *lone*
/// expansion filter. Combining an expansion filter with other filters in one
/// tally is not yet supported (documented gap, bead op-6tz.14).
///
/// # Fields
/// - `order` — highest Legendre order retained (bins `P_0 … P_order`).
/// - `axis` — the Cartesian axis expanded ([`LegendreAxis`]).
/// - `min` / `max` — the axis interval \[cm\] mapped onto `ξ ∈ [−1, 1]`;
///   `max > min` required. Events outside `[min, max]` contribute nothing.
pub struct SpatialLegendreFilter {
    /// Highest Legendre order retained (⇒ `order + 1` moment bins).
    pub order: usize,
    /// Cartesian axis along which the flux is expanded.
    pub axis: LegendreAxis,
    /// Low end of the expansion interval \[cm\].
    pub min: f64,
    /// High end of the expansion interval \[cm\] (must exceed `min`).
    pub max: f64,
}

impl SpatialLegendreFilter {
    /// The axis coordinate \[cm\] of an event's position for this filter's axis.
    #[inline]
    fn axis_coord(&self, p: Position) -> f64 {
        match self.axis {
            LegendreAxis::X => p.x,
            LegendreAxis::Y => p.y,
            LegendreAxis::Z => p.z,
        }
    }
}

impl Filter for SpatialLegendreFilter {
    fn n_bins(&self) -> usize {
        self.order + 1
    }

    /// A single-bin fallback: bin 0 if the event lies in `[min, max]`, else `None`.
    /// The expansion path uses [`Filter::expansion_moments`]; this exists only so
    /// the filter still satisfies the [`Filter`] contract if misused as a plain
    /// single-bin filter.
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let x = self.axis_coord(ev.position);
        if x >= self.min && x <= self.max {
            Some(0)
        } else {
            None
        }
    }

    /// Legendre moment weights `P_0(ξ) … P_order(ξ)`, or `None` if the event's
    /// axis coordinate is outside `[min, max]`. Mirrors
    /// `SpatialLegendreFilter::get_all_bins` (`src/tallies/filter_sptl_legendre.cpp:63`).
    fn expansion_moments(&self, ev: &FilterEvent) -> Option<Vec<f64>> {
        let x = self.axis_coord(ev.position);
        if x < self.min || x > self.max {
            return None;
        }
        let xi = 2.0 * (x - self.min) / (self.max - self.min) - 1.0;
        Some(legendre_pn(self.order, xi))
    }
}

/// Legendre polynomials `P_0(x) … P_order(x)` via the standard recurrence
/// `(n+1) P_{n+1}(x) = (2n+1) x P_n(x) − n P_{n-1}(x)`.
///
/// Ports `calc_pn_c` (`src/math_functions.cpp:105-116`). Valid for any real `x`;
/// the spatial Legendre filter calls it with `x = ξ ∈ [−1, 1]`, where every
/// `|P_n(ξ)| ≤ 1`.
fn legendre_pn(order: usize, x: f64) -> Vec<f64> {
    let mut pnx = vec![0.0; order + 1];
    pnx[0] = 1.0;
    if order >= 1 {
        pnx[1] = x;
    }
    for l in 1..order {
        pnx[l + 1] = ((2 * l + 1) as f64 * x * pnx[l] - l as f64 * pnx[l - 1]) / (l + 1) as f64;
    }
    pnx
}

// ── Filters added 2026-09-16 ─────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]

/// Filter by the surface an event crossed. Maps to `openmc::SurfaceFilter`
/// (`src/tallies/filter_surface.cpp`).
///
/// **0-based indices into the geometry's surface array, not surface IDs** — the
/// same convention as [`CellFilter`], and the same trap: an ID passed here bins
/// silently and wrongly.
///
/// Only a surface-crossing event has a surface; every other event carries
/// [`FilterEvent::surface_idx`] `= usize::MAX` and is rejected, so attaching this
/// to a track-length or collision tally scores nothing rather than scoring
/// everything into bin 0.
pub struct SurfaceFilter {
    /// 0-based positions in the surface array. One tally bin per entry.
    pub surface_indices: Vec<usize>,
}

impl Filter for SurfaceFilter {
    fn n_bins(&self) -> usize {
        self.surface_indices.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        if ev.surface_idx == usize::MAX {
            return None;
        }
        self.surface_indices
            .iter()
            .position(|&s| s == ev.surface_idx)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by the **change-of-direction cosine** of a scatter. Maps to
/// `openmc::MuFilter` (`src/tallies/filter_mu.cpp`).
///
/// `bounds` is an ascending list of `mu` bin edges on `[-1, 1]`, so `n_bins` is
/// `bounds.len() - 1` and bin `i` covers `[bounds[i], bounds[i+1])`. The top
/// edge is inclusive, matching OpenMC's treatment of the last bin.
///
/// This bins [`FilterEvent::mu`], the **laboratory-frame** scattering cosine.
/// OpenMC's filter is documented against the same quantity; a CM cosine binned
/// here would be a different distribution entirely on a light nuclide.
pub struct MuFilter {
    /// Ascending `mu` bin edges on `[-1, 1]`; `n_bins = len() - 1`.
    pub bounds: Vec<f64>,
}

impl Filter for MuFilter {
    fn n_bins(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        bin_in_edges(&self.bounds, ev.mu)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by the particle's **polar and azimuthal angles of travel**. Maps to
/// `openmc::PolarAzimuthalFilter` (`src/tallies/filter_azimuthal.cpp` +
/// `filter_polar.cpp`, which OpenMC exposes as one filter).
///
/// `polar` holds ascending edges in `cos(theta)` on `[-1, 1]` — **cosine, not
/// the angle** — where `theta` is measured from `+z`, so `cos(theta)` is the
/// direction's `w` component. `azimuthal` holds ascending edges in `phi` on
/// `[-pi, pi]`, with `phi = atan2(v, u)`.
///
/// Bins are row-major with **polar slowest-varying**:
/// `bin = i_polar * n_azimuthal + i_azimuthal`, so `n_bins` is the product.
pub struct PolarAzimuthalFilter {
    /// Ascending `cos(theta)` edges on `[-1, 1]`.
    pub polar: Vec<f64>,
    /// Ascending `phi` edges on `[-pi, pi]` \[rad\].
    pub azimuthal: Vec<f64>,
}

impl Filter for PolarAzimuthalFilter {
    fn n_bins(&self) -> usize {
        self.polar.len().saturating_sub(1) * self.azimuthal.len().saturating_sub(1)
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let i_p = bin_in_edges(&self.polar, ev.direction.w)?;
        let phi = ev.direction.v.atan2(ev.direction.u);
        let i_a = bin_in_edges(&self.azimuthal, phi)?;
        Some(i_p * self.azimuthal.len().saturating_sub(1) + i_a)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by time since the particle was born. Maps to `openmc::TimeFilter`
/// (`src/tallies/filter_time.cpp`).
///
/// `bounds` is an ascending list of edges in **seconds**. A time-dependent tally
/// is only as good as the time the transport threads through
/// [`FilterEvent::time`]; the eigenvalue drivers in this crate do not track a
/// clock, so on those this bins every event into whichever bin contains `0.0`.
/// That is visible rather than hidden: see [`FilterEvent::default`].
pub struct TimeFilter {
    /// Ascending time bin edges \[s\]; `n_bins = len() - 1`.
    pub bounds: Vec<f64>,
}

impl Filter for TimeFilter {
    fn n_bins(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        bin_in_edges(&self.bounds, ev.time)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by particle type. Maps to `openmc::ParticleFilter`
/// (`src/tallies/filter_particle.cpp`).
///
/// This crate transports neutrons only (photon/electron transport is explicitly
/// out of scope, see the crate `CLAUDE.md`), so a tally filtering on
/// [`ParticleType::Photon`] scores nothing today. It is implemented because the
/// filter is cheap and because scoring zero for an absent particle is the
/// correct answer, where omitting the filter would have forced a caller to drop
/// the distinction.
pub struct ParticleFilter {
    /// Particle types to score. One bin per entry, in this order.
    pub particles: Vec<ParticleType>,
}

impl Filter for ParticleFilter {
    fn n_bins(&self) -> usize {
        self.particles.len()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.particles.iter().position(|&p| p == ev.particle)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Filter by delayed-neutron precursor group. Maps to
/// `openmc::DelayedGroupFilter` (`src/tallies/filter_delayedgroup.cpp`).
///
/// `groups` holds **0-based** precursor group indices. ENDF and most of the
/// literature number these 1..=6; this crate indexes them from zero throughout
/// (see `teh-o-prke`), and the two conventions differing by one is exactly the
/// sort of thing that produces a plausible wrong answer, so it is stated here
/// rather than left to the reader.
///
/// A prompt neutron or a non-fission event carries
/// [`FilterEvent::delayed_group`] `= None` and is rejected.
pub struct DelayedGroupFilter {
    /// 0-based precursor group indices. One bin per entry.
    pub groups: Vec<usize>,
}

impl DelayedGroupFilter {
    /// **This filter cannot currently tally anything, and saying so is the
    /// point of this constructor.** GitHub #262.
    ///
    /// # Why it is refused rather than left constructible
    ///
    /// Audited 2026-09-22
    /// (`verification_and_validation/delayed_neutrons/audit_2026_09_22.md`):
    /// **nothing in this crate ever sets [`FilterEvent::delayed_group`].**
    /// Every site that assigns `Some(..)` is a test. `physics::fission` folds
    /// delayed neutrons into the total ν̄ and treats them as prompt — a
    /// legitimate, clearly-labelled eigenvalue approximation — so no precursor
    /// group is ever sampled to put on an event.
    ///
    /// A filter in that state does not fail. It sees `None` at every event,
    /// bins nothing, and the tally returns **exactly zero** with no error and
    /// no empty-result diagnostic.
    ///
    /// That is worse than the usual silent-wrong-answer, because **zero is a
    /// value a physicist might accept**: a delayed-group tally over a
    /// non-fissile region genuinely should be zero, so the wrong answer is
    /// indistinguishable from a right one without knowing the geometry.
    ///
    /// # When this starts working
    ///
    /// When ν̄ is split into prompt and delayed in the transport loop and the
    /// sampled precursor group is recorded on the event. The data is already
    /// in the workspace and unused — `njoy-outram-park-fork`'s
    /// `nuclear_data::delayed` carries MF=1/455 (λ_k, ν̄_d(E)) and MF=5/455
    /// (abundances, delayed spectra). See #262 scope items 2 and 3. At that
    /// point this constructor returns `Ok` and the struct's `Filter` impl
    /// below already does the right thing unchanged.
    ///
    /// # Errors
    ///
    /// Always, until the above lands. The error names the audit so a caller
    /// who hits it can read why rather than guess.
    pub fn new(groups: Vec<usize>) -> Result<Self, String> {
        Err(format!(
            "a DelayedGroupFilter over {} group(s) would tally exactly ZERO: nothing in \
             this crate sets FilterEvent::delayed_group, because physics::fission folds \
             delayed neutrons into the total nu-bar and treats them as prompt. See \
             GitHub #262 and \
             verification_and_validation/delayed_neutrons/audit_2026_09_22.md. \
             Refused rather than returning a silent zero, which is indistinguishable \
             from a correct result over a non-fissile region.",
            groups.len()
        ))
    }
}

impl Filter for DelayedGroupFilter {
    fn n_bins(&self) -> usize {
        self.groups.len()
    }
    /// Correct as written, and deliberately left alone: the moment
    /// `delayed_group` is populated by the transport loop this bins properly
    /// with no change here. What is refused is *constructing* the filter while
    /// that field is always `None` — see [`Self::new`].
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let g = ev.delayed_group?;
        self.groups.iter().position(|&x| x == g)
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Functional-expansion filter in **Zernike polynomials** over a disc in the
/// `x`-`y` plane. Maps to `openmc::ZernikeFilter`
/// (`src/tallies/filter_zernike.cpp`).
///
/// The flux is expanded on the unit disc obtained by mapping
/// `rho = sqrt((x-x0)^2 + (y-y0)^2) / r`, `theta = atan2(y-y0, x-x0)`. Moments
/// run over the standard Zernike ordering
///
/// ```text
/// n = 0, 1, 2, ... order;   m = -n, -n+2, ... , n
/// ```
///
/// which gives `(order+1)(order+2)/2` moments, with
/// `Z_n^m = R_n^|m|(rho) * cos(m theta)` for `m >= 0` and
/// `R_n^|m|(rho) * sin(|m| theta)` for `m < 0` — the real-valued convention
/// OpenMC uses (`calc_zn`, `src/math_functions.cpp`).
///
/// As with [`SpatialLegendreFilter`], the reconstruction normalisation is
/// **not** folded into the stored weight; the raw moment is what is tallied.
/// Events with `rho > 1` contribute nothing.
pub struct ZernikeFilter {
    /// Highest radial order `n` retained.
    pub order: usize,
    /// Disc centre `x` \[cm\].
    pub x0: f64,
    /// Disc centre `y` \[cm\].
    pub y0: f64,
    /// Disc radius \[cm\]; must be positive.
    pub r: f64,
}

impl Filter for ZernikeFilter {
    fn n_bins(&self) -> usize {
        (self.order + 1) * (self.order + 2) / 2
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        let (dx, dy) = (ev.position.x - self.x0, ev.position.y - self.y0);
        if self.r <= 0.0 || (dx * dx + dy * dy).sqrt() / self.r > 1.0 {
            return None;
        }
        Some(0)
    }
    fn expansion_moments(&self, ev: &FilterEvent) -> Option<Vec<f64>> {
        let (dx, dy) = (ev.position.x - self.x0, ev.position.y - self.y0);
        if self.r <= 0.0 {
            return None;
        }
        let rho = (dx * dx + dy * dy).sqrt() / self.r;
        if rho > 1.0 {
            return None;
        }
        Some(zernike_zn(self.order, rho, dy.atan2(dx)))
    }
}
#[derive(Debug, Clone, PartialEq)]

/// Functional-expansion filter in **real spherical harmonics** of the particle's
/// direction. Maps to `openmc::SphericalHarmonicsFilter`
/// (`src/tallies/filter_sph_harm.cpp`).
///
/// Moments run `l = 0 ..= order`, `m = -l ..= l`, in that order, giving
/// `(order+1)^2` bins. The weight deposited in moment `(l, m)` is the real
/// spherical harmonic `Y_l^m(theta, phi)` evaluated on the direction of travel,
/// with `cos(theta) = w` and `phi = atan2(v, u)`.
///
/// Like the other expansions here, the stored moment is raw — the `(2l+1)/(4pi)`
/// reconstruction factor is applied when the flux is rebuilt, not when it is
/// scored.
pub struct SphericalHarmonicsFilter {
    /// Highest harmonic order `l` retained (⇒ `(order+1)^2` bins).
    pub order: usize,
}

impl Filter for SphericalHarmonicsFilter {
    fn n_bins(&self) -> usize {
        (self.order + 1) * (self.order + 1)
    }
    fn get_bin(&self, _ev: &FilterEvent) -> Option<usize> {
        Some(0)
    }
    fn expansion_moments(&self, ev: &FilterEvent) -> Option<Vec<f64>> {
        Some(real_spherical_harmonics(
            self.order,
            ev.direction.w,
            ev.direction.v.atan2(ev.direction.u),
        ))
    }
}

/// Locate `x` in an ascending edge list, returning the 0-based bin index.
///
/// Bin `i` is `[edges[i], edges[i+1])`, with the **final** edge inclusive so a
/// value exactly on the top of the range scores in the last bin rather than
/// falling out. Returns `None` outside the range, or when there are fewer than
/// two edges (no bins to score into).
fn bin_in_edges(edges: &[f64], x: f64) -> Option<usize> {
    let n = edges.len();
    if n < 2 || x < edges[0] || x > edges[n - 1] {
        return None;
    }
    if x == edges[n - 1] {
        return Some(n - 2);
    }
    // Ascending edges, so a linear walk is correct; bin counts here are small.
    (0..n - 1).find(|&i| x >= edges[i] && x < edges[i + 1])
}

/// Zernike moments `Z_n^m(rho, theta)` in OpenMC's ordering, for
/// `n = 0..=order` and `m = -n, -n+2, ..., n`.
///
/// Ports `calc_zn` (`src/math_functions.cpp`). The radial polynomial is built
/// from its defining sum
///
/// ```text
/// R_n^m(rho) = sum_{k=0}^{(n-m)/2} (-1)^k (n-k)! /
///              [ k! ((n+m)/2 - k)! ((n-m)/2 - k)! ] rho^(n-2k)
/// ```
///
/// which is exact in `f64` for the orders a tally uses (factorials stay well
/// inside the integer range until `n` is far larger than any expansion anyone
/// reconstructs from).
fn zernike_zn(order: usize, rho: f64, theta: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity((order + 1) * (order + 2) / 2);
    for n in 0..=order {
        let mut m = -(n as i32);
        while m <= n as i32 {
            let am = m.unsigned_abs() as usize;
            let r = zernike_radial(n, am, rho);
            let v = if m < 0 {
                r * (am as f64 * theta).sin()
            } else {
                r * (am as f64 * theta).cos()
            };
            out.push(v);
            m += 2;
        }
    }
    out
}

/// The Zernike radial polynomial `R_n^m(rho)`; `0` when `n - m` is odd, which is
/// the convention that makes the moment list above well defined.
fn zernike_radial(n: usize, m: usize, rho: f64) -> f64 {
    if n < m || (n - m) % 2 != 0 {
        return 0.0;
    }
    let half_minus = (n - m) / 2;
    let half_plus = (n + m) / 2;
    let mut acc = 0.0;
    for k in 0..=half_minus {
        let num = factorial(n - k);
        let den = factorial(k) * factorial(half_plus - k) * factorial(half_minus - k);
        let term = num / den * rho.powi((n - 2 * k) as i32);
        acc += if k % 2 == 0 { term } else { -term };
    }
    acc
}

/// `k!` as an `f64`. Exact for every `k` a tally expansion reaches (`f64`
/// represents factorials exactly to `22!`).
fn factorial(k: usize) -> f64 {
    (1..=k).map(|i| i as f64).product::<f64>().max(1.0)
}

/// Real spherical harmonics `Y_l^m(theta, phi)` for `l = 0..=order`,
/// `m = -l..=l`, in that order.
///
/// Ports `calc_rn` (`src/math_functions.cpp`). Uses the orthonormal real
/// convention
///
/// ```text
/// Y_l^0  = N_l^0 P_l(cos theta)
/// Y_l^m  = sqrt(2) N_l^m P_l^m(cos theta) cos(m phi)      m > 0
/// Y_l^-m = sqrt(2) N_l^m P_l^m(cos theta) sin(m phi)      m > 0
/// N_l^m  = sqrt( (2l+1)/(4 pi) * (l-m)!/(l+m)! )
/// ```
fn real_spherical_harmonics(order: usize, cos_theta: f64, phi: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity((order + 1) * (order + 1));
    let four_pi = 4.0 * std::f64::consts::PI;
    for l in 0..=order {
        for m in -(l as i32)..=(l as i32) {
            let am = m.unsigned_abs() as usize;
            let p = assoc_legendre(l, am, cos_theta);
            let norm =
                ((2.0 * l as f64 + 1.0) / four_pi * factorial(l - am) / factorial(l + am)).sqrt();
            let v = if m == 0 {
                norm * p
            } else if m > 0 {
                std::f64::consts::SQRT_2 * norm * p * (am as f64 * phi).cos()
            } else {
                std::f64::consts::SQRT_2 * norm * p * (am as f64 * phi).sin()
            };
            out.push(v);
        }
    }
    out
}

/// Associated Legendre function `P_l^m(x)` for `m >= 0`, by the standard
/// recurrences (Condon-Shortley phase included, as OpenMC's `calc_rn` assumes).
fn assoc_legendre(l: usize, m: usize, x: f64) -> f64 {
    if m > l {
        return 0.0;
    }
    // P_m^m = (-1)^m (2m-1)!! (1-x^2)^(m/2)
    let mut pmm = 1.0f64;
    if m > 0 {
        let somx2 = ((1.0 - x) * (1.0 + x)).max(0.0).sqrt();
        let mut fact = 1.0f64;
        for _ in 0..m {
            pmm *= -fact * somx2;
            fact += 2.0;
        }
    }
    if l == m {
        return pmm;
    }
    let mut pmmp1 = x * (2.0 * m as f64 + 1.0) * pmm;
    if l == m + 1 {
        return pmmp1;
    }
    let mut pll = 0.0;
    for ll in (m + 2)..=l {
        pll = ((2.0 * ll as f64 - 1.0) * x * pmmp1 - (ll + m - 1) as f64 * pmm) / (ll - m) as f64;
        pmm = pmmp1;
        pmmp1 = pll;
    }
    pll
}

/// Every filter this crate provides, as a closed enum.
///
/// This is what a [`super::tally::Tally`] stores. Dispatch is by `match`, so
/// adding a filter is a compile error at every site that must handle it — the
/// property `Box<dyn Filter>` cost, and the reason the workspace's design rules
/// ask for enums here. The [`Filter`] trait stays as the per-struct contract.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterKind {
    /// [`CellFilter`].
    Cell(CellFilter),
    /// [`MaterialFilter`].
    Material(MaterialFilter),
    /// [`EnergyFilter`] — incoming energy.
    Energy(EnergyFilter),
    /// [`EnergyOutFilter`] — outgoing (post-collision) energy.
    EnergyOut(EnergyOutFilter),
    /// [`UniverseFilter`].
    Universe(UniverseFilter),
    /// [`MeshFilter`].
    Mesh(MeshFilter),
    /// [`SurfaceFilter`].
    Surface(SurfaceFilter),
    /// [`MuFilter`].
    Mu(MuFilter),
    /// [`PolarAzimuthalFilter`].
    PolarAzimuthal(PolarAzimuthalFilter),
    /// [`TimeFilter`].
    Time(TimeFilter),
    /// [`ParticleFilter`].
    Particle(ParticleFilter),
    /// [`DelayedGroupFilter`].
    DelayedGroup(DelayedGroupFilter),
    /// [`SpatialLegendreFilter`] — a functional expansion.
    SpatialLegendre(SpatialLegendreFilter),
    /// [`ZernikeFilter`] — a functional expansion.
    Zernike(ZernikeFilter),
    /// [`SphericalHarmonicsFilter`] — a functional expansion.
    SphericalHarmonics(SphericalHarmonicsFilter),
    // ── GitHub #261, second batch ──────────────────────────────────────────
    //
    // These eight were written into `super::filter_extra` in an earlier
    // increment and **were not reachable**: they implemented `Filter` but had
    // no `FilterKind` variant, so no `Tally` could hold one. A filter that
    // cannot be put on a tally is not a ported filter, whatever its test
    // coverage says — `every_filter_kind_is_constructible_on_a_tally` now
    // fails if a variant is ever added without being dispatched here.
    /// [`super::filter_extra::ReactionFilter`] — by MT, with ENDF summation.
    Reaction(super::filter_extra::ReactionFilter),
    /// [`super::filter_extra::CollisionFilter`] — by collision number.
    Collision(super::filter_extra::CollisionFilter),
    /// [`super::filter_extra::CellFromFilter`] — the cell the particle left.
    CellFrom(super::filter_extra::CellFromFilter),
    /// [`super::filter_extra::CellBornFilter`] — the cell it was born in.
    CellBorn(super::filter_extra::CellBornFilter),
    /// [`super::filter_extra::MaterialFromFilter`] — the material it left.
    MaterialFrom(super::filter_extra::MaterialFromFilter),
    /// [`super::filter_extra::WeightFilter`] — by statistical weight.
    Weight(super::filter_extra::WeightFilter),
    /// [`super::filter_extra::MuSurfaceFilter`] — cosine to a surface normal.
    MuSurface(super::filter_extra::MuSurfaceFilter),
    /// [`super::filter_extra::EnergyFunctionFilter`] — a continuous weight in
    /// energy rather than a binning.
    EnergyFunction(super::filter_extra::EnergyFunctionFilter),
    /// [`super::filter_extra::LegendreFilter`] — a functional expansion in the
    /// scattering cosine; the quantity MGXS generation is built on.
    Legendre(super::filter_extra::LegendreFilter),
    /// [`super::filter_extra::MeshBornFilter`].
    MeshBorn(super::filter_extra::MeshBornFilter),
    /// [`super::filter_extra::ParentNuclideFilter`].
    ParentNuclide(super::filter_extra::ParentNuclideFilter),
    /// [`super::filter_extra::CellInstanceFilter`].
    CellInstance(super::filter_extra::CellInstanceFilter),
    /// [`super::filter_extra::MeshMaterialFilter`].
    MeshMaterial(super::filter_extra::MeshMaterialFilter),
    /// [`super::filter_extra::ParticleProductionFilter`] — can match one event
    /// in several bins at once; see its `matches`.
    ParticleProduction(super::filter_extra::ParticleProductionFilter),
    /// [`super::filter_extra::MeshSurfaceFilter`] — mesh-face currents; also
    /// matches one event in several bins.
    MeshSurface(super::filter_extra::MeshSurfaceFilter),
}

impl FilterKind {
    /// Number of tally bins this filter produces.
    pub fn n_bins(&self) -> usize {
        self.as_filter().n_bins()
    }

    /// Bin index for `event`, or `None` if the event does not pass.
    pub fn get_bin(&self, event: &FilterEvent) -> Option<usize> {
        self.as_filter().get_bin(event)
    }

    /// Functional-expansion weights, or `None` for a non-expansion filter.
    pub fn expansion_moments(&self, event: &FilterEvent) -> Option<Vec<f64>> {
        self.as_filter().expansion_moments(event)
    }

    /// Whether this filter deposits into every moment bin at once rather than
    /// into a single bin — true for the three functional expansions.
    ///
    /// Worth its own method because the scoring path needs the distinction
    /// *before* it has an event to test, and `expansion_moments` returning
    /// `None` is ambiguous between "not an expansion" and "this event is outside
    /// the expansion's domain".
    pub fn is_expansion(&self) -> bool {
        matches!(
            self,
            FilterKind::SpatialLegendre(_)
                | FilterKind::Zernike(_)
                | FilterKind::SphericalHarmonics(_)
                // GitHub #261: the scattering-cosine expansion. Leaving it out
                // here would make it silently score nothing — `get_bin`
                // returns `None` for every expansion filter by design, so the
                // scoring path would drop every event.
                | FilterKind::Legendre(_)
        )
    }

    /// A short, stable name for each variant.
    ///
    /// # Why this exists
    ///
    /// It is an **exhaustive match with no wildcard**, so adding a variant to
    /// [`FilterKind`] without touching this function is a compile error. That
    /// is deliberate: GitHub #261's first increment wrote eight filters into
    /// `filter_extra` that implemented [`Filter`] and had **no variant here**,
    /// so no `Tally` could hold one. They had tests, they passed, and they
    /// were unreachable. This function plus
    /// `every_filter_kind_is_reachable_and_binnable` is what makes that
    /// failure mode loud instead of silent.
    pub fn name(&self) -> &'static str {
        match self {
            FilterKind::Cell(_) => "cell",
            FilterKind::Material(_) => "material",
            FilterKind::Energy(_) => "energy",
            FilterKind::EnergyOut(_) => "energyout",
            FilterKind::Universe(_) => "universe",
            FilterKind::Mesh(_) => "mesh",
            FilterKind::Surface(_) => "surface",
            FilterKind::Mu(_) => "mu",
            FilterKind::PolarAzimuthal(_) => "polar-azimuthal",
            FilterKind::Time(_) => "time",
            FilterKind::Particle(_) => "particle",
            FilterKind::DelayedGroup(_) => "delayedgroup",
            FilterKind::SpatialLegendre(_) => "spatiallegendre",
            FilterKind::Zernike(_) => "zernike",
            FilterKind::SphericalHarmonics(_) => "sphericalharmonics",
            FilterKind::Reaction(_) => "reaction",
            FilterKind::Collision(_) => "collision",
            FilterKind::CellFrom(_) => "cellfrom",
            FilterKind::CellBorn(_) => "cellborn",
            FilterKind::MaterialFrom(_) => "materialfrom",
            FilterKind::Weight(_) => "weight",
            FilterKind::MuSurface(_) => "musurface",
            FilterKind::EnergyFunction(_) => "energyfunction",
            FilterKind::Legendre(_) => "legendre",
            FilterKind::MeshBorn(_) => "meshborn",
            FilterKind::ParentNuclide(_) => "parentnuclide",
            FilterKind::CellInstance(_) => "cellinstance",
            FilterKind::MeshMaterial(_) => "meshmaterial",
            FilterKind::ParticleProduction(_) => "particleproduction",
            FilterKind::MeshSurface(_) => "meshsurface",
        }
    }

    /// The concrete filter behind this variant, as its trait contract.
    ///
    /// Returning `&dyn Filter` here is *not* dynamic dispatch for the tally: the
    /// enum is still what is stored and matched, and this is a one-line internal
    /// adaptor so each variant's `impl Filter` is used rather than duplicated
    /// into three `match` arms per method.
    fn as_filter(&self) -> &dyn Filter {
        match self {
            FilterKind::Cell(f) => f,
            FilterKind::Material(f) => f,
            FilterKind::Energy(f) => f,
            FilterKind::EnergyOut(f) => f,
            FilterKind::Universe(f) => f,
            FilterKind::Mesh(f) => f,
            FilterKind::Surface(f) => f,
            FilterKind::Mu(f) => f,
            FilterKind::PolarAzimuthal(f) => f,
            FilterKind::Time(f) => f,
            FilterKind::Particle(f) => f,
            FilterKind::DelayedGroup(f) => f,
            FilterKind::SpatialLegendre(f) => f,
            FilterKind::Zernike(f) => f,
            FilterKind::SphericalHarmonics(f) => f,
            FilterKind::Reaction(f) => f,
            FilterKind::Collision(f) => f,
            FilterKind::CellFrom(f) => f,
            FilterKind::CellBorn(f) => f,
            FilterKind::MaterialFrom(f) => f,
            FilterKind::Weight(f) => f,
            FilterKind::MuSurface(f) => f,
            FilterKind::EnergyFunction(f) => f,
            FilterKind::Legendre(f) => f,
            FilterKind::MeshBorn(f) => f,
            FilterKind::ParentNuclide(f) => f,
            FilterKind::CellInstance(f) => f,
            FilterKind::MeshMaterial(f) => f,
            FilterKind::ParticleProduction(f) => f,
            FilterKind::MeshSurface(f) => f,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `legendre_pn` matches the closed forms P_0..P_3 at a sample point.
    #[test]
    fn legendre_recurrence_matches_closed_form() {
        let x = 0.3_f64;
        let p = legendre_pn(3, x);
        assert!((p[0] - 1.0).abs() < 1e-14);
        assert!((p[1] - x).abs() < 1e-14);
        assert!((p[2] - 0.5 * (3.0 * x * x - 1.0)).abs() < 1e-14);
        assert!((p[3] - 0.5 * (5.0 * x.powi(3) - 3.0 * x)).abs() < 1e-14);
    }

    /// The Legendre filter maps min→ξ=−1, max→ξ=+1, midpoint→ξ=0 and returns the
    /// expected moment weights (P_0=1 everywhere; P_1=ξ).
    #[test]
    fn legendre_filter_normalizes_axis() {
        let f = SpatialLegendreFilter {
            order: 2,
            axis: LegendreAxis::Z,
            min: -10.0,
            max: 10.0,
        };
        let ev = |z: f64| FilterEvent {
            cell_idx: 0,
            material_idx: 0,
            universe_idx: 0,
            energy: 1.0,
            surface_idx: usize::MAX,
            position: Position::new(0.0, 0.0, z),
            ..Default::default()
        };
        let mid = f.expansion_moments(&ev(0.0)).unwrap();
        assert!((mid[0] - 1.0).abs() < 1e-14 && mid[1].abs() < 1e-14);
        let top = f.expansion_moments(&ev(10.0)).unwrap();
        assert!(
            (top[1] - 1.0).abs() < 1e-14,
            "max → ξ=+1 ⇒ P_1=1, got {}",
            top[1]
        );
        let bot = f.expansion_moments(&ev(-10.0)).unwrap();
        assert!(
            (bot[1] + 1.0).abs() < 1e-14,
            "min → ξ=−1 ⇒ P_1=−1, got {}",
            bot[1]
        );
        assert!(
            f.expansion_moments(&ev(11.0)).is_none(),
            "outside [min,max] → None"
        );
    }

    // ── EnergyOutFilter ───────────────────────────────────────────────────

    /// A 3-bin outgoing-energy structure over [1, 10, 100, 1000] eV.
    fn eout_filter() -> EnergyOutFilter {
        EnergyOutFilter {
            bins: vec![1.0, 10.0, 100.0, 1000.0],
        }
    }

    /// `n + 1` edges produce `n` bins, and a degenerate edge list produces none.
    #[test]
    fn energy_out_filter_bin_count() {
        assert_eq!(eout_filter().n_bins(), 3);
        assert_eq!(EnergyOutFilter { bins: vec![1.0] }.n_bins(), 0);
        assert_eq!(EnergyOutFilter { bins: Vec::new() }.n_bins(), 0);
    }

    /// The filter bins the OUTGOING energy and ignores the incoming one.
    ///
    /// This is the property that makes a scattering matrix possible: the same
    /// event carries both energies, and this filter must key on `energy_out`.
    /// The incoming energies below are chosen to fall in a *different* bin from
    /// their outgoing partner, so a filter that mistakenly read `energy` would
    /// fail rather than coincidentally agree.
    #[test]
    fn energy_out_filter_bins_outgoing_not_incoming() {
        let f = eout_filter();
        let ev = |e_in: f64, e_out: f64| FilterEvent {
            energy: e_in,
            energy_out: Some(e_out),
            ..Default::default()
        };
        // incoming in bin 2, outgoing in bin 0
        assert_eq!(f.get_bin(&ev(500.0, 5.0)), Some(0));
        // incoming in bin 0, outgoing in bin 1
        assert_eq!(f.get_bin(&ev(2.0, 50.0)), Some(1));
        // incoming in bin 0, outgoing in bin 2
        assert_eq!(f.get_bin(&ev(2.0, 500.0)), Some(2));
    }

    /// An event with no secondary does not pass at all.
    ///
    /// Absorptions, surface crossings and pure track-length segments carry
    /// `energy_out == None`. They must contribute to NO outgoing-energy bin --
    /// binning them anywhere would put absorptions into the scattering matrix
    /// and silently inflate every scatter cross section.
    #[test]
    fn energy_out_filter_rejects_events_with_no_secondary() {
        let f = eout_filter();
        let absorption = FilterEvent {
            energy: 50.0,
            energy_out: None,
            ..Default::default()
        };
        assert_eq!(
            f.get_bin(&absorption),
            None,
            "an event with no secondary must bin nowhere"
        );
        // Default is the same case, and is what the track-length estimator builds.
        assert_eq!(f.get_bin(&FilterEvent::default()), None);
    }

    /// Bin edges are half-open [lo, hi): the lower edge is in, the upper is out.
    ///
    /// Matches `EnergyFilter`'s own convention, so an incoming/outgoing pair on
    /// one tally cannot disagree about which group a boundary energy lands in.
    #[test]
    fn energy_out_filter_edges_are_half_open() {
        let f = eout_filter();
        let at = |e_out: f64| FilterEvent {
            energy_out: Some(e_out),
            ..Default::default()
        };
        assert_eq!(f.get_bin(&at(1.0)), Some(0), "lower edge is included");
        assert_eq!(f.get_bin(&at(10.0)), Some(1), "interior edge -> upper bin");
        assert_eq!(f.get_bin(&at(1000.0)), None, "top edge is excluded");
        assert_eq!(f.get_bin(&at(0.5)), None, "below range");
        assert_eq!(f.get_bin(&at(5000.0)), None, "above range");
    }

    /// Reached through `FilterKind`, the enum the tally actually stores.
    #[test]
    fn energy_out_filter_dispatches_through_filter_kind() {
        let k = FilterKind::EnergyOut(eout_filter());
        assert_eq!(k.n_bins(), 3);
        let ev = FilterEvent {
            energy: 900.0,
            energy_out: Some(50.0),
            ..Default::default()
        };
        assert_eq!(k.get_bin(&ev), Some(1));
        assert!(
            !k.is_expansion(),
            "outgoing energy is a single-bin filter, not a functional expansion"
        );
        assert!(k.expansion_moments(&ev).is_none());
    }

    /// An incoming and an outgoing filter on the same event pick the (g_in,
    /// g_out) pair a scattering matrix is built from -- including up-scatter,
    /// where the outgoing group is higher in energy than the incoming one.
    #[test]
    fn incoming_and_outgoing_filters_together_give_a_matrix_element() {
        let f_in = EnergyFilter {
            bins: vec![1.0, 10.0, 100.0, 1000.0],
        };
        let f_out = eout_filter();

        // Down-scatter: 500 eV -> 5 eV is (g_in = 2, g_out = 0).
        let down = FilterEvent {
            energy: 500.0,
            energy_out: Some(5.0),
            ..Default::default()
        };
        assert_eq!(
            (f_in.get_bin(&down), f_out.get_bin(&down)),
            (Some(2), Some(0))
        );

        // Up-scatter: 5 eV -> 50 eV is (0, 1). Thermal up-scatter is real
        // physics (the free-gas and S(alpha,beta) paths both produce it), so
        // the pair must be representable rather than clamped to the diagonal.
        let up = FilterEvent {
            energy: 5.0,
            energy_out: Some(50.0),
            ..Default::default()
        };
        assert_eq!((f_in.get_bin(&up), f_out.get_bin(&up)), (Some(0), Some(1)));
    }
}
