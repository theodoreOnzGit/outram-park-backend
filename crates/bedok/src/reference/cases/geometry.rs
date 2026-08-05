//! Case geometry: boundary conditions, cell centres, the fuelled-region
//! extents, and the utilities that derive them.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | `geometry_ends3d.m`, `convert_grid3d.m`, `calc_relpower3d.m`, and the `geometry` struct built by `iaea3ds.m` / `neacrpa2.m` / `neacrpd1.m` / `geom2dxycase1.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # Units
//!
//! All lengths are **centimetres** and all volumes **cubic centimetres** — the
//! units the benchmark specifications and the MATLAB use throughout. `uom`
//! types are deliberately not used in the reference translation, so the
//! arithmetic stays line-for-line comparable with the original.

use crate::error::{BedokError, Result};
use crate::reference::grid::{Geometry, Grid};

use super::csv_maps::NumericMatrix;
use super::fuel::FuelGeometry;

/// An outer boundary condition on one face of the domain.
///
/// MATLAB stores these as strings on `geometry.xmin` … `geometry.zmax`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Zero net current: a symmetry plane. MATLAB `'reflective'`.
    ///
    /// Both quadrant/octant cases use this on the two inner faces.
    Reflective,
    /// Zero incoming partial current, i.e. the usual extrapolated-boundary
    /// condition. MATLAB `'vacuum'`. Used by IAEA-3D on all outer faces.
    Vacuum,
    /// Flux forced to zero at the face. MATLAB `'zeroflux'`. Used by both
    /// NEACRP cases on all outer faces.
    ZeroFlux,
}

/// The six outer boundary conditions of a case.
///
/// # Note on the 2-D case
///
/// `geom2dxycase1.m` names its boundaries `left` / `right` / `top` / `bottom`
/// rather than `xmin` … `ymax`. They are mapped here as
/// left → `x_min`, right → `x_max`, bottom → `y_min`, top → `y_max`, and the
/// two z faces are filled with the same condition. The rename is recorded
/// because it means the 2-D case's `geometry` is **not** interchangeable with
/// a 3-D one in the MATLAB either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boundaries {
    /// Condition at `x = 0`. MATLAB `geometry.xmin`.
    pub x_min: Boundary,
    /// Condition at `x = Xtot`. MATLAB `geometry.xmax`.
    pub x_max: Boundary,
    /// Condition at `y = 0`. MATLAB `geometry.ymin`.
    pub y_min: Boundary,
    /// Condition at `y = Ytot`. MATLAB `geometry.ymax`.
    pub y_max: Boundary,
    /// Condition at `z = 0`. MATLAB `geometry.zmin`.
    pub z_min: Boundary,
    /// Condition at `z = Ztot`. MATLAB `geometry.zmax`.
    pub z_max: Boundary,
}

/// Node-refinement factors relative to each case's native mesh.
///
/// MATLAB `xscale`, `yscale`, `zscale`, computed as `int64(maxix/17)` and so
/// on. They let a case be run on a refined grid: the composition maps stay
/// 17 × 17 and are sampled with `ceil(ix/maxix*17)`.
///
/// `neacrpa2.m` and `neacrpd1.m` store these on `geometry`; `iaea3ds.m`
/// computes them but does not store them. They are stored uniformly here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridScale {
    /// `maxix / 17`, rounded. MATLAB `xscale`.
    pub x: usize,
    /// `maxiy / 17`, rounded. MATLAB `yscale`.
    pub y: usize,
    /// `maxiz / (native axial levels)`, rounded — 19 for IAEA-3D, 18 for the
    /// PWR cases, 14 for the BWR case. MATLAB `zscale`.
    pub z: usize,
}

/// MATLAB's `int64(x)` on a positive ratio: round half **away from zero**.
///
/// Rust's `as usize` truncates and `f64::round` rounds half away from zero,
/// which is what MATLAB does — so this is `round`, not `trunc`. Spelled out
/// because the difference decides the grid on a non-integer refinement.
///
/// # Errors
///
/// [`BedokError::EmptyGrid`] if the result is zero. The MATLAB has no such
/// guard: a zero scale makes its `for iz = 1:zscale` loops empty and its
/// `Zlengths(ceil(iz/zscale))` a division by zero. Rejecting it is an error
/// path, not a change to any computed value.
pub fn matlab_int64_scale(requested: usize, native: usize, grid: Grid) -> Result<usize> {
    let scale = ((requested as f64) / (native as f64)).round() as i64;
    if scale <= 0 {
        return Err(BedokError::EmptyGrid {
            nx: grid.nx,
            ny: grid.ny,
            nz: grid.nz,
            ngroups: grid.ngroups,
        });
    }
    Ok(scale as usize)
}

/// The first and last **fuelled** node along each axis, per transverse
/// position.
///
/// MATLAB `geometry.xlows` / `xhis` / `ylows` / `yhis` / `zlows` / `zhis`,
/// built by `geometry_ends3d.m`. The nodal solver uses them to skip the
/// void (`whichsigma == 0`) region outside the core outline, so that a
/// quadrant map with a stepped radial boundary does not spend unknowns on
/// nodes that are not there.
///
/// # Index convention
///
/// Values here are **0-based node indices**, one less than the MATLAB's. The
/// accessors take 0-based transverse coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEnds {
    grid: Grid,
    x_low: Vec<usize>,
    x_high: Vec<usize>,
    y_low: Vec<usize>,
    y_high: Vec<usize>,
    z_low: Vec<usize>,
    z_high: Vec<usize>,
}

impl DomainEnds {
    /// First fuelled `ix` at `(iy, iz)`. MATLAB `geometry.xlows(iy,iz) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn x_low(&self, iy: usize, iz: usize) -> usize {
        self.x_low[iy * self.grid.nz + iz]
    }

    /// Last fuelled `ix` at `(iy, iz)`. MATLAB `geometry.xhis(iy,iz) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn x_high(&self, iy: usize, iz: usize) -> usize {
        self.x_high[iy * self.grid.nz + iz]
    }

    /// First fuelled `iy` at `(ix, iz)`. MATLAB `geometry.ylows(ix,iz) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn y_low(&self, ix: usize, iz: usize) -> usize {
        self.y_low[ix * self.grid.nz + iz]
    }

    /// Last fuelled `iy` at `(ix, iz)`. MATLAB `geometry.yhis(ix,iz) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn y_high(&self, ix: usize, iz: usize) -> usize {
        self.y_high[ix * self.grid.nz + iz]
    }

    /// First fuelled `iz` at `(ix, iy)`. MATLAB `geometry.zlows(ix,iy) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn z_low(&self, ix: usize, iy: usize) -> usize {
        self.z_low[ix * self.grid.ny + iy]
    }

    /// Last fuelled `iz` at `(ix, iy)`. MATLAB `geometry.zhis(ix,iy) - 1`.
    ///
    /// # Panics
    ///
    /// If either index is out of range.
    #[must_use]
    pub fn z_high(&self, ix: usize, iy: usize) -> usize {
        self.z_high[ix * self.grid.ny + iy]
    }
}

/// Scan the material map for the fuelled extent along each axis.
///
/// Rust translation of `geometry_ends3d.m`.
///
/// For each transverse position the MATLAB walks the third index from 1
/// upwards, records the first node whose material is non-zero as the low end,
/// and the node **before** the first zero encountered *after* that as the high
/// end. Defaults are the full range (`1` and `maxi`).
///
/// # Two consequences of that rule, recorded not repaired
///
/// - A line that is **entirely** outside the core (all zeros) keeps the
///   defaults, so it reports the *whole* axis as fuelled. Downstream code that
///   trusts `xlows`/`xhis` without also checking `whichsigma` will therefore
///   see phantom nodes on such a line.
/// - Only the **first** contiguous run is found. A line whose material returns
///   to non-zero after a gap has the second run silently dropped. Neither
///   benchmark has such a line, so this never bites in the ported cases.
///
/// `which_sigma` is indexed as the flattened spatial map, `ix*ny*nz + iy*nz +
/// iz` — the same rule as [`Grid::index`](crate::reference::grid::Grid::index)
/// with `g = 0`.
///
/// # Errors
///
/// [`BedokError::Fixture`] if `which_sigma` is not `grid.nodes()` long.
pub fn geometry_ends_3d(grid: Grid, which_sigma: &[usize]) -> Result<DomainEnds> {
    if which_sigma.len() != grid.nodes() {
        return Err(BedokError::Fixture {
            path: "geometry_ends3d".to_string(),
            reason: format!(
                "material map has {} entries, expected {}",
                which_sigma.len(),
                grid.nodes()
            ),
        });
    }
    let at = |ix: usize, iy: usize, iz: usize| which_sigma[grid.index(0, ix, iy, iz)];

    let mut x_low = vec![0usize; grid.ny * grid.nz];
    let mut x_high = vec![grid.nx - 1; grid.ny * grid.nz];
    for iy in 0..grid.ny {
        for iz in 0..grid.nz {
            let mut started = false;
            for ix in 0..grid.nx {
                if !started && at(ix, iy, iz) != 0 {
                    started = true;
                    x_low[iy * grid.nz + iz] = ix;
                } else if started && at(ix, iy, iz) == 0 {
                    x_high[iy * grid.nz + iz] = ix - 1;
                    break;
                }
            }
        }
    }

    let mut y_low = vec![0usize; grid.nx * grid.nz];
    let mut y_high = vec![grid.ny - 1; grid.nx * grid.nz];
    for ix in 0..grid.nx {
        for iz in 0..grid.nz {
            let mut started = false;
            for iy in 0..grid.ny {
                if !started && at(ix, iy, iz) != 0 {
                    started = true;
                    y_low[ix * grid.nz + iz] = iy;
                } else if started && at(ix, iy, iz) == 0 {
                    y_high[ix * grid.nz + iz] = iy - 1;
                    break;
                }
            }
        }
    }

    let mut z_low = vec![0usize; grid.nx * grid.ny];
    let mut z_high = vec![grid.nz - 1; grid.nx * grid.ny];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let mut started = false;
            for iz in 0..grid.nz {
                if !started && at(ix, iy, iz) != 0 {
                    started = true;
                    z_low[ix * grid.ny + iy] = iz;
                } else if started && at(ix, iy, iz) == 0 {
                    z_high[ix * grid.ny + iy] = iz - 1;
                    break;
                }
            }
        }
    }

    Ok(DomainEnds {
        grid,
        x_low,
        x_high,
        y_low,
        y_high,
        z_low,
        z_high,
    })
}

/// A compaction of the full state vector down to only the fuelled nodes.
///
/// Rust translation of `convert_grid3d.m`. `key[full_index]` is the compacted
/// (1-based) position of that unknown, or `0` if the node is outside the core;
/// `reverse_key[compacted - 1]` is the full 1-based index it came from.
///
/// The 1-based values are kept because the sparse-matrix rewiring that
/// consumes them (`convertsparsekey3d.m`) tests `key(i) == 0` to mean
/// "dropped", which a 0-based key could not express.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridKey {
    /// `key(idx)`, one entry per full state slot: the 1-based compacted index,
    /// or `0` for a node outside the core.
    pub key: Vec<usize>,
    /// `reversekey(counter)`: the 1-based full index each compacted unknown
    /// came from. Entries beyond [`len`](Self::len) are `0`.
    pub reverse_key: Vec<usize>,
    /// Number of unknowns kept.
    pub kept: usize,
}

impl GridKey {
    /// Number of unknowns after compaction. MATLAB's final `counter`.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.kept
    }

    /// Whether every node was dropped — an all-void material map.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kept == 0
    }
}

/// Build the compaction key over the fuelled nodes.
///
/// Rust translation of `convert_grid3d.m`.
///
/// # Unfinished in the reference
///
/// The `Nc /= 0` branch computes its index as
/// `(G+Nc-1)*energyindexstep + …` inside a loop over `nn = 1:Nc`, so every
/// extra unknown at a node is given the *same* index — plainly a typo for
/// `(G+nn-1)*…`. `params.Nc` is `0` in every case in the snapshot, so the
/// branch never runs. Reproduced as written, per
/// `docs/bedok-port-scoping.md` §1.0.
///
/// # Errors
///
/// [`BedokError::Fixture`] if `which_sigma` is not `grid.nodes()` long.
pub fn convert_grid_3d(
    grid: Grid,
    num_extra_unknowns: usize,
    which_sigma: &[usize],
) -> Result<GridKey> {
    if which_sigma.len() != grid.nodes() {
        return Err(BedokError::Fixture {
            path: "convert_grid3d".to_string(),
            reason: format!(
                "material map has {} entries, expected {}",
                which_sigma.len(),
                grid.nodes()
            ),
        });
    }
    let nodes = grid.nodes();
    let n_c = num_extra_unknowns;
    let phi_len_f = (grid.ngroups + n_c) * nodes;

    let mut key = vec![0usize; phi_len_f];
    let mut reverse_key = vec![0usize; phi_len_f];
    let mut counter = 0usize;

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                if which_sigma[grid.index(0, ix, iy, iz)] == 0 {
                    continue;
                }
                for g in 0..grid.ngroups {
                    counter += 1;
                    let idx = grid.index(g, ix, iy, iz);
                    key[idx] = counter;
                    reverse_key[counter - 1] = idx + 1;
                }
                for _nn in 0..n_c {
                    counter += 1;
                    // Faithful to the MATLAB: the energy offset does not
                    // advance with nn. See the "Unfinished" note above.
                    let idx = (grid.ngroups + n_c - 1) * nodes
                        + ix * (grid.ny * grid.nz)
                        + iy * grid.nz
                        + iz;
                    key[idx] = counter;
                    reverse_key[counter - 1] = idx + 1;
                }
            }
        }
    }

    Ok(GridKey {
        key,
        reverse_key,
        kept: counter,
    })
}

/// Collapse a power-density state vector to a normalised radial power map.
///
/// Rust translation of `calc_relpower3d.m`.
///
/// Sums the per-group power density into a single spatial field, integrates it
/// over `z`, then scales so that the **mean over the non-zero entries** is 1.
/// The result is the assembly-wise relative power, the quantity the IAEA-3D
/// and NEACRP benchmarks tabulate.
///
/// Returned row-major over `(ix, iy)`, length `nx*ny` \[dimensionless\].
///
/// # Note
///
/// The normalisation divides by `nnz(pwrdensxy)` — the count of radial
/// positions with non-zero power — so it is a mean over *fuelled* positions,
/// not over all of them. A position that happens to integrate to exactly zero
/// while being inside the core would be excluded; that cannot occur for a
/// converged flux.
///
/// # Errors
///
/// [`BedokError::Fixture`] if `power_density` is neither `grid.nodes()` nor
/// `grid.state_len()` long, or if the total power is zero (the scaling would
/// be `0/0`).
pub fn calc_relative_power_3d(grid: Grid, power_density: &[f64]) -> Result<Vec<f64>> {
    let nodes = grid.nodes();

    // MATLAB: if G>1 and the vector is longer than one spatial field, fold the
    // groups together.
    let folded: Vec<f64> = if grid.ngroups > 1 && power_density.len() > nodes {
        if power_density.len() != grid.state_len() {
            return Err(BedokError::Fixture {
                path: "calc_relpower3d".to_string(),
                reason: format!(
                    "power density has {} entries, expected {} or {}",
                    power_density.len(),
                    nodes,
                    grid.state_len()
                ),
            });
        }
        let mut acc = power_density[..nodes].to_vec();
        for g in 1..grid.ngroups {
            for (i, a) in acc.iter_mut().enumerate() {
                *a += power_density[g * nodes + i];
            }
        }
        acc
    } else {
        if power_density.len() != nodes {
            return Err(BedokError::Fixture {
                path: "calc_relpower3d".to_string(),
                reason: format!(
                    "power density has {} entries, expected {nodes}",
                    power_density.len()
                ),
            });
        }
        power_density.to_vec()
    };

    let mut radial = vec![0.0f64; grid.nx * grid.ny];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let mut sum = 0.0;
            for iz in 0..grid.nz {
                sum += folded[grid.index(0, ix, iy, iz)];
            }
            radial[ix * grid.ny + iy] = sum;
        }
    }

    let n_nonzero = radial.iter().filter(|v| **v != 0.0).count() as f64;
    let total: f64 = radial.iter().sum();
    if total == 0.0 {
        return Err(BedokError::Fixture {
            path: "calc_relpower3d".to_string(),
            reason: "total power is zero; relative power is 0/0".to_string(),
        });
    }
    for v in &mut radial {
        *v = *v * n_nonzero / total;
    }
    Ok(radial)
}

/// Everything a case constructor puts on its `geometry` struct.
///
/// The node counts, extents, per-node lengths, volumes and material map live
/// in [`Geometry`], which is shared with the solver; this type carries the
/// case-specific remainder.
#[derive(Debug, Clone)]
pub struct CaseGeometry {
    /// Grid, extents, per-node lengths/volumes, and the flattened material
    /// map.
    ///
    /// **Note on `lx` / `ly` / `lz`:** these are filled **per spatial node**
    /// (length `grid.nodes()`), matching MATLAB `geometry.Lx` etc., which the
    /// solvers index with the full node index. The doc comment on
    /// [`Geometry::lx`] describes them as "one per x index"; the per-node form
    /// is what the reference actually builds and what downstream indexing
    /// requires. Recorded here rather than changed — `grid.rs` is outside this
    /// module's ownership.
    pub base: Geometry,
    /// Refinement factors relative to the case's native mesh. MATLAB
    /// `geometry.xscale` / `yscale` / `zscale`.
    pub scale: GridScale,
    /// Centre of each spatial node, `[x, y, z]` \[cm\], flattened with the
    /// same rule as `base.which_sigma`. MATLAB `geometry.Ctr`.
    pub centers: Vec<[f64; 3]>,
    /// The six outer boundary conditions.
    pub boundaries: Boundaries,
    /// Fuelled extents per axis. `None` for `geom2dxycase1`, which does not
    /// call `geometry_ends3d`; the solvers test `isfield(geometry,'xlows')`
    /// and fall back to the full range.
    pub ends: Option<DomainEnds>,
    /// Fuel-pin geometry and material correlations. `None` for the two cases
    /// without thermal hydraulics.
    pub fuel: Option<FuelGeometry>,
    /// Control-rod bank layout and positions. `None` where the case defines no
    /// rods.
    pub control_rods: Option<ControlRodConfig>,
}

/// Control-rod bank geometry and the current bank positions.
///
/// MATLAB `geometry.crodn` / `crodbtm` / `crodstep` / `crodmaxstep` /
/// `crodtop` / `crodbanks` / `crod`, set by the NEACRP PWR cases.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlRodConfig {
    /// Number of banks. MATLAB `geometry.crodn`; `7` for the PWR cases.
    pub bank_count: usize,
    /// Axial height of a fully inserted rod tip above the bottom of the model
    /// \[cm\]. MATLAB `geometry.crodbtm`.
    pub bottom: f64,
    /// Axial travel per withdrawal step \[cm\]. MATLAB `geometry.crodstep`.
    pub step: f64,
    /// Steps from fully inserted to fully withdrawn \[steps\]. MATLAB
    /// `geometry.crodmaxstep`.
    pub max_steps: f64,
    /// Tip height at full withdrawal, `bottom + step*max_steps` \[cm\].
    /// MATLAB `geometry.crodtop`.
    pub top: f64,
    /// Bank number at each radial position, `0` = no rod. MATLAB
    /// `geometry.crodbanks`, read from `NEACRPA2_CRODBANKS.csv`.
    pub banks: NumericMatrix,
    /// Current position of each bank \[withdrawal steps\], `0` = fully
    /// inserted. MATLAB `geometry.crod`.
    pub positions: Vec<f64>,
}

impl ControlRodConfig {
    /// Tip height of each bank above the bottom of the model \[cm\].
    ///
    /// MATLAB `rodpos = geometry.crodbtm + geometry.crod*geometry.crodstep`,
    /// computed in `sigmavalupd3d_handler.m`. A node is rodded to the extent
    /// its axial span lies above this height.
    #[must_use]
    pub fn tip_heights(&self) -> Vec<f64> {
        self.positions
            .iter()
            .map(|steps| self.bottom + steps * self.step)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_grid() -> Grid {
        Grid::new(3, 3, 3, 1).expect("valid")
    }

    /// A single fuelled node in the middle of an otherwise void 3×3×3 grid:
    /// its own line reports it, and every other line keeps the defaults.
    #[test]
    fn ends_find_the_single_fuelled_node() {
        let grid = tiny_grid();
        let mut map = vec![0usize; grid.nodes()];
        map[grid.index(0, 1, 1, 1)] = 1;
        let ends = geometry_ends_3d(grid, &map).expect("built");

        assert_eq!(ends.x_low(1, 1), 1);
        assert_eq!(ends.x_high(1, 1), 1);
        assert_eq!(ends.y_low(1, 1), 1);
        assert_eq!(ends.z_low(1, 1), 1);

        // A wholly void line keeps the defaults — the behaviour flagged in the
        // docs of `geometry_ends_3d`.
        assert_eq!(ends.x_low(0, 0), 0);
        assert_eq!(ends.x_high(0, 0), grid.nx - 1);
    }

    /// A full line stays at the defaults: low 0, high nx-1, because the scan
    /// never meets a zero to close on.
    #[test]
    fn a_fully_fuelled_line_spans_the_axis() {
        let grid = tiny_grid();
        let map = vec![1usize; grid.nodes()];
        let ends = geometry_ends_3d(grid, &map).expect("built");
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                assert_eq!(ends.x_low(iy, iz), 0);
                assert_eq!(ends.x_high(iy, iz), grid.nx - 1);
            }
        }
    }

    #[test]
    fn grid_key_numbers_only_the_fuelled_nodes() {
        let grid = tiny_grid();
        let mut map = vec![0usize; grid.nodes()];
        map[grid.index(0, 1, 1, 1)] = 1;
        map[grid.index(0, 2, 2, 2)] = 2;
        let key = convert_grid_3d(grid, 0, &map).expect("built");
        assert_eq!(key.len(), 2, "two nodes x one group");
        assert_eq!(key.key[grid.index(0, 1, 1, 1)], 1);
        assert_eq!(key.key[grid.index(0, 2, 2, 2)], 2);
        assert_eq!(key.key[grid.index(0, 0, 0, 0)], 0);
        // reverse_key holds 1-based full indices.
        assert_eq!(key.reverse_key[0], grid.index(0, 1, 1, 1) + 1);
    }

    /// Relative power averages to one over the fuelled positions.
    #[test]
    fn relative_power_averages_to_one() {
        let grid = Grid::new(2, 2, 2, 1).expect("valid");
        let mut p = vec![0.0f64; grid.nodes()];
        // Two radial positions carry power, in a 3:1 ratio.
        p[grid.index(0, 0, 0, 0)] = 3.0;
        p[grid.index(0, 1, 1, 0)] = 1.0;
        let rel = calc_relative_power_3d(grid, &p).expect("computed");
        assert_eq!(rel.len(), 4);
        assert!((rel[0] - 1.5).abs() < 1e-15);
        assert!((rel[3] - 0.5).abs() < 1e-15);
        let mean: f64 = rel.iter().filter(|v| **v != 0.0).sum::<f64>() / 2.0;
        assert!((mean - 1.0).abs() < 1e-15);
    }

    #[test]
    fn scale_rounds_like_matlab_int64() {
        let grid = Grid::new(17, 17, 19, 2).expect("valid");
        assert_eq!(matlab_int64_scale(17, 17, grid).expect("ok"), 1);
        assert_eq!(matlab_int64_scale(34, 17, grid).expect("ok"), 2);
        // 26/17 = 1.529... rounds to 2, where truncation would give 1.
        assert_eq!(matlab_int64_scale(26, 17, grid).expect("ok"), 2);
        // A grid coarser than the native mesh is rejected rather than
        // producing an empty loop / division by zero.
        assert!(matlab_int64_scale(8, 17, grid).is_err());
    }
}
