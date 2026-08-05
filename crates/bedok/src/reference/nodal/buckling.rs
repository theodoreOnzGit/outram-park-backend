//! The node-wise buckling operators.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_bucklingxyz.m` (function `calc_bucklingxyz`).

use super::cross_sections::CrossSectionOperators;
use super::geometry::{Axis, NodalGeometry, NodalParams};
use super::sparse::SparseMatrix;

/// The three directional buckling operators, one per coordinate direction.
///
/// Each is `philen` square and **dimensionless**: it is the net removal
/// operator `sigma_tot - sigma_s - sigma_f/k_eff` \[cm⁻¹\] scaled by
/// `0.25*L²/D` \[cm\], i.e. the squared optical half-width of the node in that
/// direction. Rows and columns are state indices; the only nonzero columns of
/// row `idx` are the `G` group indices at `idx`'s own spatial node, so each
/// operator is block-diagonal in space with `G`×`G` blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct Buckling {
    /// x-direction buckling operator \[dimensionless\].
    pub x: SparseMatrix,
    /// y-direction buckling operator \[dimensionless\].
    pub y: SparseMatrix,
    /// z-direction buckling operator \[dimensionless\].
    pub z: SparseMatrix,
}

impl Buckling {
    /// The operator along `axis`.
    #[must_use]
    pub fn axis(&self, axis: Axis) -> &SparseMatrix {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
            Axis::Z => &self.z,
        }
    }
}

/// Builds the three buckling operators — `calc_bucklingxyz.m`.
///
/// Entry `(idx, c)` of the x operator is
///
/// ```text
/// (sigma_tot - sigma_s - sigma_f/k_eff)(idx, c) * 0.25 * Lx(idx)^2 / D(idx)
/// ```
///
/// and likewise for y and z, with the element order kept as the MATLAB writes
/// it (`Bt*0.25 .* L .* L ./ D`) so the rounding matches.
///
/// `k_eff` is the current eigenvalue estimate \[dimensionless\], typically
/// within a few percent of 1. `diffusion` is the flat diffusion-coefficient
/// state vector \[cm\]; nodes where its group-1 entry is zero are skipped
/// entirely, leaving the operators empty there.
///
/// # Caching omitted
///
/// The MATLAB caches the `k_eff`-independent part in `persistent` storage,
/// keyed on a fingerprint of the inputs, and rebuilds only when the
/// fingerprint changes. That is a pure speed optimisation with no effect on
/// results, and it is not reproduced — a `persistent` cache shared across
/// unrelated cases is also a correctness hazard this port has no reason to
/// inherit.
///
/// # Panics
///
/// If `diffusion.len()` differs from the neutronics state length.
#[must_use]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    sigma: &CrossSectionOperators,
    diffusion: &[f64],
    k_eff: f64,
) -> Buckling {
    let grid = params.grid;
    let philen = params.philen();
    assert_eq!(diffusion.len(), philen, "diffusion length");

    let lx = geometry.width_state_vector(Axis::X, grid);
    let ly = geometry.width_state_vector(Axis::Y, grid);
    let lz = geometry.width_state_vector(Axis::Z, grid);

    let a_term = sigma.total.sub(&sigma.scatter);
    let b_term = &sigma.fission;

    let mut tx: Vec<(usize, usize, f64)> = Vec::new();
    let mut ty: Vec<(usize, usize, f64)> = Vec::new();
    let mut tz: Vec<(usize, usize, f64)> = Vec::new();

    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                if diffusion[grid.index(0, ix, iy, iz)] == 0.0 {
                    continue;
                }
                for g in 0..grid.ngroups {
                    let idx = grid.index(g, ix, iy, iz);
                    let d = diffusion[idx];
                    for gc in 0..grid.ngroups {
                        let col = grid.index(gc, ix, iy, iz);
                        let bt = a_term.get(idx, col) - b_term.get(idx, col) / k_eff;
                        tx.push((idx, col, bt * 0.25 * lx[idx] * lx[idx] / d));
                        ty.push((idx, col, bt * 0.25 * ly[idx] * ly[idx] / d));
                        tz.push((idx, col, bt * 0.25 * lz[idx] * lz[idx] / d));
                    }
                }
            }
        }
    }

    Buckling {
        x: SparseMatrix::from_triplets(philen, philen, &tx),
        y: SparseMatrix::from_triplets(philen, philen, &ty),
        z: SparseMatrix::from_triplets(philen, philen, &tz),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::cross_sections::{
        assemble_operators, diffusion_coefficients, MaterialCrossSections,
    };
    use crate::reference::nodal::geometry::{BoundaryCondition, BoundaryConditions};

    fn single_node_case() -> (NodalParams, NodalGeometry, CrossSectionOperators, Vec<f64>) {
        let grid = Grid::new(1, 1, 1, 2).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let values = MaterialCrossSections {
            total: vec![vec![0.5, 1.0]],
            fission: vec![vec![0.0, 0.1]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.4, 0.0], vec![0.05, 0.9]]],
            nu: vec![vec![2.0, 2.0]],
            chi: vec![vec![1.0, 0.0]],
        };
        let geometry = NodalGeometry::new(
            grid,
            vec![20.0],
            vec![10.0],
            vec![40.0],
            vec![1],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let d = diffusion_coefficients(grid, &values.total, &[1], 1.0);
        let sigma = assemble_operators(&params, &values, &[1]);
        (params, geometry, sigma, d)
    }

    #[test]
    fn diagonal_entry_is_removal_times_squared_optical_half_width() {
        let (params, geometry, sigma, d) = single_node_case();
        let b = assemble(&params, &geometry, &sigma, &d, 1.0);
        // Fast group: sigma_tot 0.5, self-scatter 0.4, no fission source into
        // itself from itself (chi=(1,0) but sigma_f(fast)=0), so
        // Bt = 0.5 - 0.4 - 0 = 0.1. D = 1/(3*0.5) = 2/3.
        let expected = 0.1 * 0.25 * 20.0 * 20.0 / (1.0 / 1.5);
        assert!((b.x.get(0, 0) - expected).abs() < 1e-12);
        // z uses the 40 cm width: four times the x value.
        assert!((b.z.get(0, 0) - 4.0 * expected).abs() < 1e-12);
    }

    #[test]
    fn the_fission_term_scales_as_one_over_keff() {
        let (params, geometry, sigma, d) = single_node_case();
        let b1 = assemble(&params, &geometry, &sigma, &d, 1.0);
        let b2 = assemble(&params, &geometry, &sigma, &d, 2.0);
        // Off-diagonal (fast row, thermal column) is purely -sigma_f/keff.
        let e1 = b1.x.get(0, 1);
        let e2 = b2.x.get(0, 1);
        assert!(e1 < 0.0);
        assert!((e2 - e1 / 2.0).abs() < 1e-12);
    }

    #[test]
    fn out_of_core_nodes_produce_no_entries() {
        let grid = Grid::new(2, 1, 1, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let values = MaterialCrossSections {
            total: vec![vec![0.5]],
            fission: vec![vec![0.0]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.4]]],
            nu: vec![vec![2.0]],
            chi: vec![vec![1.0]],
        };
        let geometry = NodalGeometry::new(
            grid,
            vec![20.0, 20.0],
            vec![20.0, 20.0],
            vec![20.0, 20.0],
            vec![1, 0],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let d = diffusion_coefficients(grid, &values.total, &[1, 0], 1.0);
        let sigma = assemble_operators(&params, &values, &[1, 0]);
        let b = assemble(&params, &geometry, &sigma, &d, 1.0);
        assert_eq!(b.x.stored_entries(), 1);
        assert_eq!(b.x.get(1, 1), 0.0);
    }
}
