//! The semi-analytic nodal correction operator.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_sanodalxyz.m` (function `calc_sanodalxyz`).
//!
//! This is the heart of the method. The semi-analytic expansion gives a surface
//! current on each face; dividing it by the flux turns it into a *correction*
//! to the finite-difference coupling coefficient, which is then assembled into
//! an operator added alongside `gradD`. The result is a coarse-mesh
//! finite-difference scheme whose coefficients reproduce the analytic
//! one-dimensional solution — the standard nodal-equivalence trick.

use super::cross_sections::CrossSectionOperators;
use super::expansion::{self, Expansion};
use super::geometry::{Axis, Face, FaceTerms, NodalGeometry, NodalParams};
use super::sparse::SparseMatrix;

/// The nodal correction operator and the face terms it was built from.
#[derive(Debug, Clone, PartialEq)]
pub struct NodalCorrection {
    /// The correction operator \[cm⁻¹\], `philen` square, added to `gradD` in
    /// the source iteration. MATLAB `nodal`.
    ///
    /// # Recorded dimension mismatch
    ///
    /// This is `philen` square while `gradD` and the cross-section operators
    /// are `philenf` square. With `Nc == 0` those coincide; with `Nc > 0` the
    /// MATLAB's `gradD+nodal+sigma.tot-sigma.s` raises a dimension error, and
    /// so does this port. See
    /// [`NodalParams::n_precursor_groups`](super::geometry::NodalParams::n_precursor_groups).
    pub operator: SparseMatrix,
    /// The per-face nodal correction coefficients \[cm\]. MATLAB
    /// `nodalterms`; fed back into the next iteration's transverse-leakage
    /// calculation.
    pub face_terms: FaceTerms,
    /// The `A1`–`A4` expansion this correction was derived from, kept because
    /// the caller occasionally wants to inspect it.
    pub expansion: Expansion,
}

/// Builds the nodal correction — `calc_sanodalxyz.m`.
///
/// For each face the surface current implied by the expansion is
///
/// ```text
/// J+ = (2D/L) * ( A1 + 3*A2 + H*A3 + G*A4 )        (high face)
/// J- = (2D/L) * ( A1first - 3*A2 + H*A3first - G*A4 )  (low outer face)
/// ```
///
/// and the correction coefficient on an interior face is
///
/// ```text
/// n+ = ( g+ * (phi - phi+) + J+ ) / (phi + phi+)
/// ```
///
/// with `g+` the finite-difference face term. On an outer face the flux of the
/// single adjacent node is used instead of the sum. The correction of a face is
/// then shared with the neighbour: `n-(i+1) = n+(i)`.
///
/// The operator row for node `i` is
///
/// ```text
/// (i, i)        += ( n- - n+ ) / L(i)
/// (i, i+stride)  = -n+ / L(i+stride)
/// (i, i-stride)  =  n- / L(i-stride)
/// ```
///
/// summed over the three directions, with z sweeping first and y and x
/// accumulating onto the diagonal it created.
///
/// # The near-zero flux guard, and what it hides
///
/// Dividing by the flux is only meaningful where the flux is not near zero, so
/// the reference skips the update when `|phi| <= 1e-8 * max|phi|` (or when the
/// two-node sum is), **leaving the correction at zero** — that is, falling back
/// to plain finite difference at that face. The guard is Yan Ren's own, added
/// with the comment "the nodal expansion is ill-conditioned (near-zero or
/// sign-cancelling flux)". Two consequences worth stating plainly:
///
/// - The threshold is relative to the **global** flux maximum, so in a case
///   with a strong axial or radial gradient a whole region can silently drop to
///   finite difference.
/// - The skipped entry keeps whatever it held from a previous sweep — for the
///   shared `n-(i+1) = n+(i)` assignment, which is *not* inside the guard, the
///   neighbour still receives the (possibly stale) value.
///
/// Left exactly as found.
///
/// # Panics
///
/// If the y or x sweep reaches a node the z sweep did not create a diagonal
/// entry for. That happens when the per-line active ranges disagree about which
/// nodes are in-core; the MATLAB indexes `nodalele(0)` and errors in the same
/// situation.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    flux: &[f64],
    sigma: &CrossSectionOperators,
    diffusion: &[f64],
    grad_terms: &FaceTerms,
    previous_nodal_terms: &FaceTerms,
    k_eff: f64,
) -> NodalCorrection {
    let grid = params.grid;
    let philen = params.philen();
    let ngroups = grid.ngroups;

    let expansion = expansion::assemble(
        params,
        geometry,
        flux,
        sigma,
        diffusion,
        grad_terms,
        previous_nodal_terms,
        k_eff,
    );

    let coeffs = &geometry.nodal_coefficients;
    let mut face_terms = FaceTerms::zeros(philen);

    // phi_eps: relative threshold for the nodal denominator guards.
    let phi_scale = flux.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let phi_scale = if phi_scale == 0.0 { 1.0 } else { phi_scale };
    let phi_eps = 1e-8 * phi_scale;

    // ----- surface currents implied by the expansion -----
    let mut widths = Vec::new();
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        widths.push(geometry.width_state_vector(axis, grid));
    }
    let axis_slot = |axis: Axis| match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };

    let mut current_plus = vec![vec![0.0; philen]; 3];
    let mut current_first = vec![vec![0.0; philen]; 3];
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let s = axis_slot(axis);
        let w = &widths[s];
        let a1 = expansion.first_order.axis(axis);
        let a1f = expansion.first_order.axis_first(axis);
        let a2 = expansion.second_order.axis(axis);
        let a3 = expansion.third_order.axis(axis);
        let a3f = expansion.third_order.axis_first(axis);
        let a4 = expansion.fourth_order.axis(axis);
        let hh = coeffs.hh.axis(axis);
        let gg = coeffs.gg.axis(axis);
        for i in 0..philen {
            let scale = 2.0 * diffusion[i] / w[i];
            current_plus[s][i] = scale * (a1[i] + 3.0 * a2[i] + hh[i] * a3[i] + gg[i] * a4[i]);
            current_first[s][i] = scale * (a1f[i] - 3.0 * a2[i] + hh[i] * a3f[i] - gg[i] * a4[i]);
        }
    }

    // ----- correction coefficients -----
    for axis in [Axis::Z, Axis::Y, Axis::X] {
        let s = axis_slot(axis);
        let stride = axis.stride(grid);
        let (k1_len, k2_len) = axis.line_counts(grid);
        let range = geometry.range(axis);
        let (minus, plus) = (Face::minus(axis), Face::plus(axis));

        for k1 in 0..k1_len {
            for k2 in 0..k2_len {
                let low = range.low(k1, k2);
                let high = range.high(k1, k2);

                // low node
                let (ix, iy, iz) = axis.coords(k1, k2, low);
                if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        if flux[i].abs() > phi_eps {
                            face_terms.set(
                                i,
                                minus,
                                current_first[s][i] / flux[i] - grad_terms.get(i, minus),
                            );
                        }
                    }
                }

                // faces from `low` up to `high - 1`
                for pos in low..high {
                    let (ix, iy, iz) = axis.coords(k1, k2, pos);
                    if diffusion[grid.index(0, ix, iy, iz)] == 0.0 {
                        continue;
                    }
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        let ip = i + stride;
                        let denom = flux[i] + flux[ip];
                        if denom.abs() > phi_eps {
                            face_terms.set(
                                i,
                                plus,
                                (grad_terms.get(i, plus) * (flux[i] - flux[ip])
                                    + current_plus[s][i])
                                    / denom,
                            );
                        }
                        // Outside the guard in the reference, deliberately.
                        let shared = face_terms.get(i, plus);
                        face_terms.set(ip, minus, shared);
                    }
                }

                // high node
                let (ix, iy, iz) = axis.coords(k1, k2, high);
                if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        if flux[i].abs() > phi_eps {
                            face_terms.set(
                                i,
                                plus,
                                current_plus[s][i] / flux[i] + grad_terms.get(i, plus),
                            );
                        }
                    }
                }
            }
        }
    }

    // ----- assemble the operator -----
    let mut diagonal = vec![0.0; philen];
    let mut has_diagonal = vec![false; philen];
    let mut off_diagonal: Vec<(usize, usize, f64)> = Vec::new();

    for axis in [Axis::Z, Axis::Y, Axis::X] {
        let s = axis_slot(axis);
        let w = &widths[s];
        let stride = axis.stride(grid);
        let (k1_len, k2_len) = axis.line_counts(grid);
        let range = geometry.range(axis);
        let (minus, plus) = (Face::minus(axis), Face::plus(axis));
        let first_sweep = axis == Axis::Z;

        for k1 in 0..k1_len {
            for k2 in 0..k2_len {
                let low = range.low(k1, k2);
                let high = range.high(k1, k2);

                let mut contribute = |i: usize| {
                    let v = (face_terms.get(i, minus) - face_terms.get(i, plus)) / w[i];
                    if first_sweep {
                        diagonal[i] = v;
                        has_diagonal[i] = true;
                    } else {
                        assert!(
                            has_diagonal[i],
                            "state index {i} has no nodal diagonal entry: the z sweep \
                             skipped a node the {axis:?} sweep visits"
                        );
                        diagonal[i] += v;
                    }
                };

                // interior nodes
                for pos in (low + 1)..high {
                    let (ix, iy, iz) = axis.coords(k1, k2, pos);
                    if diffusion[grid.index(0, ix, iy, iz)] == 0.0 {
                        continue;
                    }
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        contribute(i);
                        off_diagonal.push((
                            i,
                            i + stride,
                            -face_terms.get(i, plus) / w[i + stride],
                        ));
                        off_diagonal.push((
                            i,
                            i - stride,
                            face_terms.get(i, minus) / w[i - stride],
                        ));
                    }
                }

                // low node
                let (ix, iy, iz) = axis.coords(k1, k2, low);
                if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        contribute(i);
                        off_diagonal.push((
                            i,
                            i + stride,
                            -face_terms.get(i, plus) / w[i + stride],
                        ));
                    }
                }

                // high node
                let (ix, iy, iz) = axis.coords(k1, k2, high);
                if diffusion[grid.index(0, ix, iy, iz)] != 0.0 {
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        contribute(i);
                        off_diagonal.push((
                            i,
                            i - stride,
                            face_terms.get(i, minus) / w[i - stride],
                        ));
                    }
                }
            }
        }
    }

    let mut triplets: Vec<(usize, usize, f64)> = Vec::with_capacity(philen + off_diagonal.len());
    for i in 0..philen {
        if has_diagonal[i] {
            triplets.push((i, i, diagonal[i]));
        }
    }
    triplets.extend(off_diagonal);

    NodalCorrection {
        operator: SparseMatrix::from_triplets(philen, philen, &triplets),
        face_terms,
        expansion,
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
    use crate::reference::nodal::{gradient_diffusion, nodal_coefficients};

    fn setup(
        bc: BoundaryCondition,
    ) -> (
        NodalParams,
        NodalGeometry,
        CrossSectionOperators,
        Vec<f64>,
        FaceTerms,
    ) {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let ws = vec![1usize; n];
        let values = MaterialCrossSections {
            total: vec![vec![0.5]],
            fission: vec![vec![0.1]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.42]]],
            nu: vec![vec![2.4]],
            chi: vec![vec![1.0]],
        };
        let mut geometry = NodalGeometry::new(
            grid,
            vec![20.0; n],
            vec![20.0; n],
            vec![20.0; n],
            ws.clone(),
            BoundaryConditions::uniform(bc),
        );
        let d = diffusion_coefficients(grid, &values.total, &ws, 1.0);
        let sigma = assemble_operators(&params, &values, &ws);
        geometry.nodal_coefficients =
            nodal_coefficients::assemble(&params, &geometry, &sigma.total, &sigma.scatter, &d);
        let grad = gradient_diffusion::assemble(&params, &geometry, &d, &ws).face_terms;
        (params, geometry, sigma, d, grad)
    }

    #[test]
    fn the_correction_operator_stays_finite_on_the_flat_first_call() {
        // This is exactly how sanodaldiffusion_solverxyz seeds the loop:
        // a flat unit flux, zero previous nodal terms, k_eff = 1.
        let (params, geometry, sigma, d, grad) = setup(BoundaryCondition::Reflective);
        let philen = params.philen();
        let zero = FaceTerms::zeros(philen);
        let c = assemble(
            &params,
            &geometry,
            &vec![1.0; philen],
            &sigma,
            &d,
            &grad,
            &zero,
            1.0,
        );
        for i in 0..philen {
            for f in [Face::XMinus, Face::XPlus, Face::ZMinus, Face::ZPlus] {
                assert!(c.face_terms.get(i, f).is_finite(), "face term {i} {f:?}");
            }
            assert!(c.operator.get(i, i).is_finite());
        }
    }

    #[test]
    fn a_zero_flux_leaves_every_correction_at_zero() {
        // phi == 0 everywhere: phi_scale falls back to 1, phi_eps is 1e-8, and
        // every guard fails, so the correction is identically zero and the
        // scheme degenerates to plain finite difference.
        let (params, geometry, sigma, d, grad) = setup(BoundaryCondition::Reflective);
        let philen = params.philen();
        let zero = FaceTerms::zeros(philen);
        let c = assemble(
            &params,
            &geometry,
            &vec![0.0; philen],
            &sigma,
            &d,
            &grad,
            &zero,
            1.0,
        );
        for i in 0..philen {
            assert_eq!(c.operator.get(i, i), 0.0);
        }
    }

    #[test]
    fn the_shared_face_coefficient_is_written_to_both_neighbours() {
        let (params, geometry, sigma, d, grad) = setup(BoundaryCondition::Reflective);
        let grid = params.grid;
        let philen = params.philen();
        let zero = FaceTerms::zeros(philen);
        let mut flux = vec![1.0; philen];
        for ix in 0..2 {
            for iy in 0..2 {
                flux[grid.index(0, ix, iy, 2)] = 2.0;
            }
        }
        let c = assemble(&params, &geometry, &flux, &sigma, &d, &grad, &zero, 1.0);
        let i = grid.index(0, 0, 0, 0);
        assert_eq!(
            c.face_terms.get(i, Face::ZPlus),
            c.face_terms.get(i + 1, Face::ZMinus)
        );
    }

    #[test]
    fn out_of_core_nodes_get_no_operator_entries() {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let mut ws = vec![0usize; grid.nodes()];
        for ix in 0..2 {
            for iy in 0..2 {
                ws[ix * 6 + iy * 3 + 1] = 1;
            }
        }
        let values = MaterialCrossSections {
            total: vec![vec![0.5]],
            fission: vec![vec![0.1]],
            fission_prompt: Vec::new(),
            scatter: vec![vec![vec![0.42]]],
            nu: vec![vec![2.4]],
            chi: vec![vec![1.0]],
        };
        let mut geometry = NodalGeometry::new(
            grid,
            vec![20.0; grid.nodes()],
            vec![20.0; grid.nodes()],
            vec![20.0; grid.nodes()],
            ws.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let d = diffusion_coefficients(grid, &values.total, &ws, 1.0);
        let sigma = assemble_operators(&params, &values, &ws);
        geometry.nodal_coefficients =
            nodal_coefficients::assemble(&params, &geometry, &sigma.total, &sigma.scatter, &d);
        let grad = gradient_diffusion::assemble(&params, &geometry, &d, &ws).face_terms;
        let philen = params.philen();
        let zero = FaceTerms::zeros(philen);
        let c = assemble(
            &params,
            &geometry,
            &vec![1.0; philen],
            &sigma,
            &d,
            &grad,
            &zero,
            1.0,
        );
        let empty = grid.index(0, 0, 0, 0);
        assert_eq!(c.operator.get(empty, empty), 0.0);
    }
}
