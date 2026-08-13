/// Tally filters — constrain which phase-space events are scored.
///
/// C++ source: `src/tallies/filter_*.cpp` (30+ files), `include/openmc/tallies/filter.h`.
///
/// Filters work as a conjunction: a particle event is scored only if it passes
/// ALL filters attached to a tally.  Each filter maps the event to a bin index.
///
/// Implemented here: Cell, Material, Energy, Universe, Mesh
/// ([`super::mesh::RegularMesh`]) and the functional-expansion
/// [`SpatialLegendreFilter`].
/// TODO: Zernike, SphericalHarmonics, MuFilter, PolarAzimuthal,
///       Surface, DelayedGroup, Time, Particle.
use crate::geometry::position::Position;
use super::mesh::RegularMesh;

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

/// Snapshot of particle state passed to filters at scoring time.
pub struct FilterEvent {
    pub cell_idx: usize,
    pub material_idx: usize,
    pub universe_idx: usize,
    pub energy: f64,
    /// Surface crossed (usize::MAX if not a surface-crossing event).
    pub surface_idx: usize,
    /// Representative spatial position of the event \[cm\] — the streamed
    /// segment's midpoint for the track-length estimator. Used by the spatial
    /// filters ([`MeshFilter`], [`SpatialLegendreFilter`]); ignored by the
    /// cell/material/universe/energy filters.
    pub position: Position,
}

// ── Concrete filters ──────────────────────────────────────────────────────────

/// Filter by cell.  Maps to `openmc::CellFilter`.
pub struct CellFilter {
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

/// Filter by material.  Maps to `openmc::MaterialFilter`.
pub struct MaterialFilter {
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

/// Filter by energy bin (contiguous group boundaries in eV).
/// Maps to `openmc::EnergyFilter`.
pub struct EnergyFilter {
    pub bins: Vec<f64>,
} // n+1 edges → n bins
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
pub struct MeshFilter {
    pub mesh: RegularMesh,
}
impl Filter for MeshFilter {
    fn n_bins(&self) -> usize {
        self.mesh.n_bins()
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.mesh.get_bin(ev.position)
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
}
