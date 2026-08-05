//! Benchmark case constructors — geometry, materials, cross sections and
//! thermal-hydraulic boundary conditions for the cases BEDOK is verified
//! against.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | `iaea3ds.m`, `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m`, `neacrpd1.m`, `neacrpd1t.m`, `geom2dxycase1.m`, plus the utilities `geometry_ends3d.m`, `handle2dcoords.m`, `handle3dcoords.m`, `convert_grid3d.m`, `convertindexc2d.m`, `convertsparseformat2d.m`, `convertsparsekey3d.m`, `fixinfnan.m`, `fixnegativematrix.m`, `calc_relpower3d.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # What belongs here
//!
//! Everything that *describes* a benchmark: node counts and dimensions,
//! material maps, cross-section tables and their feedback derivatives,
//! fuel-pin geometry, coolant inlet conditions, control-rod bank layouts, and
//! the transient forcing. Nothing that *solves* anything — the nodal-diffusion
//! kernels live in `reference::nodal`, the channel and rod models in
//! `reference::th`, and the drivers that call both in `reference::coupling`.
//!
//! The index/sparse utilities in [`sparse`] and the map-scanning helpers in
//! [`geometry`] are here because that is where the MATLAB keeps them and
//! because they are consumed while a case is being built; they are otherwise
//! independent of any particular case.
//!
//! # The cases
//!
//! | Constructor | MATLAB | Grid | Groups | Feedback | Transient |
//! |---|---|---|---|---|---|
//! | [`iaea_3d`] | `iaea3ds.m` | 17 × 17 × **19** | 2 | none | no |
//! | [`neacrp_a2`] | `neacrpa2.m` | 17 × 17 × 18 | 2 | boron, fuel T, coolant T, coolant density, rods | no |
//! | [`neacrp_a2_transient`] | `neacrpa2t.m` | 17 × 17 × 18 | 2 | as A2 | rod ejection, 5 s |
//! | [`neacrp_a1_transient`] | `neacrpa1t.m` | 17 × 17 × 18 | 2 | as A2 | rod ejection at hot zero power, 5 s |
//! | [`neacrp_d1()`] | `neacrpd1.m` | 17 × 17 × **14** | 2 | fuel T, coolant density | no |
//! | [`neacrp_d1_transient`] | `neacrpd1t.m` | 17 × 17 × **14** | 2 | as D1 | inlet cold water, 20 s |
//! | [`geom2d_xy_case1`] | `geom2dxycase1.m` | user × user × 1 | 1 | none | no |
//!
//! # Read the grid back from the case
//!
//! Three constructors overwrite the node counts the caller asked for:
//! `iaea3ds.m` forces `maxiz = 19` (it appends a top axial reflector plane),
//! `neacrpd1.m` forces `maxiz = 14`, and both force `maxix = maxiy = 17`.
//! [`BuiltCase::grid`] is the authority on the shape of the state vector;
//! whatever went in is not.
//!
//! # Faithfulness
//!
//! Per `docs/bedok-port-scoping.md` §1.0 the reference is translated **as it
//! is**, including the parts that are unfinished or wrong. Each such place
//! carries a doc comment saying so, and none of them is repaired here. Grep
//! for "Unfinished in the reference" and "Questionable in the reference" to
//! enumerate them.

pub mod csv_maps;
pub mod fuel;
pub mod geom2d_xy;
pub mod geometry;
pub mod iaea3d;
pub mod neacrp_a;
pub mod neacrp_d1;
pub mod params;
pub mod sigmas;
pub mod sparse;
pub mod th;

pub use geom2d_xy::geom2d_xy_case1;
pub use geometry::CaseGeometry;
pub use iaea3d::iaea_3d;
pub use neacrp_a::{neacrp_a1_transient, neacrp_a2, neacrp_a2_transient};
pub use neacrp_d1::{neacrp_d1, neacrp_d1_transient};
pub use params::CaseParams;
pub use sigmas::{CaseConstants, SigmaValues};
pub use th::ThermalHydraulics;

use crate::reference::grid::Grid;

/// Everything a MATLAB case constructor returns, in one value.
///
/// The MATLAB signature is
/// `[params, geometry, th, constants, whichsigma, sigmavalues] = case(params)`
/// (or the same without `th` for the two cases with no thermal hydraulics).
/// `whichsigma` is not a separate field here because the MATLAB also writes it
/// onto `geometry` and the two are always the same array; it is reached
/// through [`which_sigma`](Self::which_sigma).
#[derive(Debug, Clone)]
pub struct BuiltCase {
    /// Solver controls and the **authoritative** grid. MATLAB `params`.
    pub params: CaseParams,
    /// Dimensions, material map, boundary conditions, fuel pin and rods.
    /// MATLAB `geometry`.
    pub geometry: CaseGeometry,
    /// Fission spectrum, neutron yield and prompt fraction. MATLAB
    /// `constants`.
    pub constants: CaseConstants,
    /// Cross sections and their feedback derivatives. MATLAB `sigmavalues`.
    pub sigmas: SigmaValues,
    /// Thermal-hydraulic boundary conditions. MATLAB `th`; absent for the two
    /// neutronics-only cases.
    pub th: Option<ThermalHydraulics>,
}

impl BuiltCase {
    /// The grid the case was actually built on.
    ///
    /// Use this, not the grid that was requested — see the module docs.
    #[must_use]
    pub const fn grid(&self) -> Grid {
        self.params.grid
    }

    /// Material index per spatial node, 1-based as in the MATLAB, `0` outside
    /// the modelled core. MATLAB `whichsigma`, flattened
    /// `ix*ny*nz + iy*nz + iz`.
    #[must_use]
    pub fn which_sigma(&self) -> &[usize] {
        &self.geometry.base.which_sigma
    }

    /// Material index at 0-based `(ix, iy, iz)`; `0` means the node is outside
    /// the modelled core and carries no unknowns.
    ///
    /// # Panics
    ///
    /// In debug builds, if any index is out of range.
    #[must_use]
    pub fn material_at(&self, ix: usize, iy: usize, iz: usize) -> usize {
        self.which_sigma()[self.grid().index(0, ix, iy, iz)]
    }

    /// Number of spatial nodes inside the modelled core, i.e. with a non-zero
    /// material index.
    #[must_use]
    pub fn active_nodes(&self) -> usize {
        self.which_sigma().iter().filter(|m| **m != 0).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every 3-D case builds, and reports a grid that matches its geometry and
    /// material map. This is the cross-case invariant that catches a
    /// constructor forgetting to write an overridden node count back.
    #[test]
    fn every_case_is_self_consistent() {
        let input = CaseParams::main_exec_defaults();
        let cases: Vec<(&str, BuiltCase)> = vec![
            ("iaea3ds", iaea_3d(&input).expect("iaea3ds builds")),
            ("neacrpa2", neacrp_a2(&input).expect("neacrpa2 builds")),
            (
                "neacrpa2t",
                neacrp_a2_transient(&input).expect("neacrpa2t builds"),
            ),
            (
                "neacrpa1t",
                neacrp_a1_transient(&input).expect("neacrpa1t builds"),
            ),
            ("neacrpd1", neacrp_d1(&input).expect("neacrpd1 builds")),
            (
                "neacrpd1t",
                neacrp_d1_transient(&input).expect("neacrpd1t builds"),
            ),
        ];

        for (name, case) in cases {
            let grid = case.grid();
            assert_eq!(case.geometry.base.grid, grid, "{name}: geometry grid");
            assert_eq!(case.which_sigma().len(), grid.nodes(), "{name}: map length");
            assert_eq!(case.geometry.centers.len(), grid.nodes(), "{name}: centres");
            assert_eq!(case.geometry.base.lx.len(), grid.nodes(), "{name}: Lx");
            assert_eq!(case.geometry.base.lz.len(), grid.nodes(), "{name}: Lz");
            assert_eq!(case.geometry.base.volume.len(), grid.nodes(), "{name}: Vi");
            assert!(case.active_nodes() > 0, "{name}: no active nodes");
            assert!(
                case.active_nodes() < grid.nodes(),
                "{name}: the quadrant map should have void corners"
            );
            let materials = case.sigmas.base.materials();
            assert!(
                case.which_sigma().iter().all(|m| *m <= materials),
                "{name}: a node names a material outside the tables"
            );
        }
    }
}
