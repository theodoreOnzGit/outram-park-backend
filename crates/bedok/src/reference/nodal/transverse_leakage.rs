//! The zeroth transverse-leakage moment.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_transleakagexyz.m` (function `calc_transleakagexyz`).

use super::geometry::{
    Axis, BoundaryCondition, DirectionVectors, Face, FaceTerms, NodalGeometry, NodalParams,
};
use super::sparse::SparseMatrix;

/// Node-average leakage out of each face pair — `calc_transleakagexyz.m`.
///
/// Builds a three-point leakage operator per direction and applies it to the
/// flux, returning `L_x*phi`, `L_y*phi` and `L_z*phi` in neutrons cm⁻³ s⁻¹ if
/// the flux is in neutrons cm⁻² s⁻¹.
///
/// The interior row for state index `i` is
///
/// ```text
///  diag  = ( g- + g+ + n- - n+ ) / L(i)
///  plus  = -( g+ + n+ ) / L(i + stride)
///  minus = -( g- - n- ) / L(i - stride)
/// ```
///
/// with `g±` the finite-difference face terms from
/// [`super::gradient_diffusion`] and `n±` the nodal corrections from
/// [`super::nodal_correction`], both \[cm\]. Note that the neighbour
/// coefficients are divided by the **neighbour's** width, not the node's — that
/// asymmetry is in the reference and is preserved.
///
/// On an outer face the diagonal keeps the full four-term form for `Vacuum` and
/// `ZeroFlux` but drops to a single face for `Reflective`, while the
/// neighbour coefficient is unchanged. Nodes whose group-1 diffusion
/// coefficient is zero are skipped, contributing nothing.
///
/// # Panics
///
/// If the flux length does not match the operator width. That mismatch is
/// reachable in the reference only through `Nc > 0`, which does not work there
/// either — see [`NodalParams::n_precursor_groups`].
#[must_use]
pub fn zeroth_moment(
    params: &NodalParams,
    geometry: &NodalGeometry,
    flux: &[f64],
    diffusion: &[f64],
    grad_terms: &FaceTerms,
    nodal_terms: &FaceTerms,
) -> DirectionVectors {
    let mut out = DirectionVectors::zeros(params.philen());
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let op = assemble_axis(params, geometry, diffusion, grad_terms, nodal_terms, axis);
        *out.axis_mut(axis) = op.mul_vec(flux);
    }
    out
}

/// Assembles the leakage operator for one direction \[cm⁻¹\].
fn assemble_axis(
    params: &NodalParams,
    geometry: &NodalGeometry,
    diffusion: &[f64],
    grad: &FaceTerms,
    nodal: &FaceTerms,
    axis: Axis,
) -> SparseMatrix {
    let grid = params.grid;
    let philen = params.philen();
    let stride = axis.stride(grid);
    let width = geometry.width_state_vector(axis, grid);
    let (minus, plus) = (Face::minus(axis), Face::plus(axis));
    let (k1_len, k2_len) = axis.line_counts(grid);
    let range = geometry.range(axis);

    let mut t: Vec<(usize, usize, f64)> = Vec::new();

    for k1 in 0..k1_len {
        for k2 in 0..k2_len {
            let low = range.low(k1, k2);
            let high = range.high(k1, k2);

            // interior
            for pos in (low + 1)..high {
                let (ix, iy, iz) = axis.coords(k1, k2, pos);
                if diffusion[grid.index(0, ix, iy, iz)] == 0.0 {
                    continue;
                }
                for g in 0..grid.ngroups {
                    let i = grid.index(g, ix, iy, iz);
                    let d = (grad.get(i, minus) + grad.get(i, plus) + nodal.get(i, minus)
                        - nodal.get(i, plus))
                        / width[i];
                    let p = -(grad.get(i, plus) + nodal.get(i, plus)) / width[i + stride];
                    let m = -(grad.get(i, minus) - nodal.get(i, minus)) / width[i - stride];
                    t.push((i, i, d));
                    t.push((i, i + stride, p));
                    t.push((i, i - stride, m));
                }
            }

            // low outer face
            let (ix, iy, iz) = axis.coords(k1, k2, low);
            if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                for g in 0..grid.ngroups {
                    let i = grid.index(g, ix, iy, iz);
                    let d = match geometry.boundaries.low(axis) {
                        BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                            (grad.get(i, minus) + grad.get(i, plus) + nodal.get(i, minus)
                                - nodal.get(i, plus))
                                / width[i]
                        }
                        BoundaryCondition::Reflective => {
                            (grad.get(i, plus) - nodal.get(i, plus)) / width[i]
                        }
                    };
                    t.push((i, i, d));
                    t.push((
                        i,
                        i + stride,
                        -(grad.get(i, plus) + nodal.get(i, plus)) / width[i + stride],
                    ));
                }
            }

            // high outer face
            let (ix, iy, iz) = axis.coords(k1, k2, high);
            if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                for g in 0..grid.ngroups {
                    let i = grid.index(g, ix, iy, iz);
                    let d = match geometry.boundaries.high(axis) {
                        BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                            (grad.get(i, minus) + grad.get(i, plus) + nodal.get(i, minus)
                                - nodal.get(i, plus))
                                / width[i]
                        }
                        BoundaryCondition::Reflective => {
                            (grad.get(i, minus) + nodal.get(i, minus)) / width[i]
                        }
                    };
                    t.push((i, i, d));
                    t.push((
                        i,
                        i - stride,
                        -(grad.get(i, minus) - nodal.get(i, minus)) / width[i - stride],
                    ));
                }
            }
        }
    }

    SparseMatrix::from_triplets(philen, philen, &t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::cross_sections::diffusion_coefficients;
    use crate::reference::nodal::geometry::BoundaryConditions;
    use crate::reference::nodal::gradient_diffusion;

    /// A 2×2×3 fully reflective block of 10 cm nodes with `D == 1`, so every
    /// interior face term is exactly `0.1` (see the `gradient_diffusion`
    /// tests).
    fn block() -> (NodalParams, NodalGeometry, Vec<f64>, FaceTerms) {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let ws = vec![1usize; n];
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; n],
            vec![10.0; n],
            vec![10.0; n],
            ws.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        let d = diffusion_coefficients(grid, &[vec![1.0 / 3.0]], &ws, 1.0);
        let grad = gradient_diffusion::assemble(&params, &geometry, &d, &ws).face_terms;
        (params, geometry, d, grad)
    }

    /// Flux rising linearly with `iz`: 1, 2, 3 in every axial column.
    fn axially_linear_flux(params: &NodalParams) -> Vec<f64> {
        let grid = params.grid;
        let mut phi = vec![0.0; grid.state_len()];
        for ix in 0..grid.nx {
            for iy in 0..grid.ny {
                for iz in 0..grid.nz {
                    phi[grid.index(0, ix, iy, iz)] = (iz + 1) as f64;
                }
            }
        }
        phi
    }

    #[test]
    fn a_flat_flux_leaks_nothing_in_a_reflective_block() {
        let (params, geometry, d, grad) = block();
        let zero = FaceTerms::zeros(params.philen());
        let flat = vec![1.0; params.philen()];
        let leak = zeroth_moment(&params, &geometry, &flat, &d, &grad, &zero);
        for v in leak.x.iter().chain(&leak.y).chain(&leak.z) {
            assert!(v.abs() < 1e-14, "flat flux should not leak");
        }
    }

    #[test]
    fn a_linear_flux_leaks_out_of_the_hot_end() {
        let (params, geometry, d, grad) = block();
        let grid = params.grid;
        let zero = FaceTerms::zeros(params.philen());
        let phi = axially_linear_flux(&params);
        let leak = zeroth_moment(&params, &geometry, &phi, &d, &grad, &zero);
        // Interior axial node is balanced; the ends are not.
        assert!(leak.z[grid.index(0, 0, 0, 1)].abs() < 1e-14);
        assert!(leak.z[grid.index(0, 0, 0, 0)] < 0.0, "low end gains");
        assert!(leak.z[grid.index(0, 0, 0, 2)] > 0.0, "high end loses");
        // The operator conserves: total axial leakage is zero.
        assert!(leak.z.iter().sum::<f64>().abs() < 1e-14);
        // The flux is uniform across x and y, so nothing leaks transversely.
        for v in leak.x.iter().chain(&leak.y) {
            assert!(v.abs() < 1e-14);
        }
    }

    #[test]
    fn a_nodal_correction_shifts_the_leakage() {
        let (params, geometry, d, grad) = block();
        let grid = params.grid;
        let phi = axially_linear_flux(&params);
        let zero = FaceTerms::zeros(params.philen());
        let base = zeroth_moment(&params, &geometry, &phi, &d, &grad, &zero);

        let mut nodal = FaceTerms::zeros(params.philen());
        let i = grid.index(0, 0, 0, 1);
        nodal.set(i, Face::ZPlus, 0.02);
        let bumped = zeroth_moment(&params, &geometry, &phi, &d, &grad, &nodal);
        // Row i changes by -n+/L * phi(i) - n+/L * phi(i+1)
        //              = -(0.02/10)*2 - (0.02/10)*3 = -0.01
        let delta = bumped.z[i] - base.z[i];
        assert!((delta + 0.01).abs() < 1e-14, "delta was {delta}");
    }
}
