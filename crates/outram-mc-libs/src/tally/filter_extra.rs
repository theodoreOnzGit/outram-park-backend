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
