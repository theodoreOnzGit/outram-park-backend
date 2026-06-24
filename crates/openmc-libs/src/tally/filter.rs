/// Tally filters — constrain which phase-space events are scored.
///
/// C++ source: `src/tallies/filter_*.cpp` (30+ files), `include/openmc/tallies/filter.h`.
///
/// Filters work as a conjunction: a particle event is scored only if it passes
/// ALL filters attached to a tally.  Each filter maps the event to a bin index.
///
/// Implemented here: Cell, Material, Energy, Universe.
/// TODO: Mesh, Legendre, Zernike, SphericalHarmonics, MuFilter, PolarAzimuthal,
///       Surface, DelayedGroup, Time, Particle.

/// Base trait for all filters.  Maps to `openmc::Filter`.
pub trait Filter: Send + Sync {
    /// Number of bins this filter produces.
    fn n_bins(&self) -> usize;

    /// Map particle state to a bin index, or `None` if the event doesn't match.
    fn get_bin(&self, event: &FilterEvent) -> Option<usize>;
}

/// Snapshot of particle state passed to filters at scoring time.
pub struct FilterEvent {
    pub cell_idx: usize,
    pub material_idx: usize,
    pub universe_idx: usize,
    pub energy: f64,
    /// Surface crossed (usize::MAX if not a surface-crossing event).
    pub surface_idx: usize,
}

// ── Concrete filters ──────────────────────────────────────────────────────────

/// Filter by cell.  Maps to `openmc::CellFilter`.
pub struct CellFilter { pub cell_indices: Vec<usize> }
impl Filter for CellFilter {
    fn n_bins(&self) -> usize { self.cell_indices.len() }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.cell_indices.iter().position(|&c| c == ev.cell_idx)
    }
}

/// Filter by material.  Maps to `openmc::MaterialFilter`.
pub struct MaterialFilter { pub material_indices: Vec<usize> }
impl Filter for MaterialFilter {
    fn n_bins(&self) -> usize { self.material_indices.len() }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.material_indices.iter().position(|&m| m == ev.material_idx)
    }
}

/// Filter by energy bin (contiguous group boundaries in eV).
/// Maps to `openmc::EnergyFilter`.
pub struct EnergyFilter { pub bins: Vec<f64> }  // n+1 edges → n bins
impl Filter for EnergyFilter {
    fn n_bins(&self) -> usize {
        if self.bins.len() < 2 { 0 } else { self.bins.len() - 1 }
    }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        if ev.energy < self.bins[0] || ev.energy >= *self.bins.last().unwrap() {
            return None;
        }
        let idx = self.bins.partition_point(|&e| e <= ev.energy).saturating_sub(1);
        Some(idx)
    }
}

/// Filter by universe.  Maps to `openmc::UniverseFilter`.
pub struct UniverseFilter { pub universe_indices: Vec<usize> }
impl Filter for UniverseFilter {
    fn n_bins(&self) -> usize { self.universe_indices.len() }
    fn get_bin(&self, ev: &FilterEvent) -> Option<usize> {
        self.universe_indices.iter().position(|&u| u == ev.universe_idx)
    }
}
