//! The first-order (`A1`) coefficients of the semi-analytic expansion.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `calc_a1_expansionxyz.m` (function `calc_a1_expansionxyz`).
//!
//! This is where continuity is imposed. Every interior face gets a `2G`×`2G`
//! system enforcing current continuity (the first `G` rows) and
//! discontinuity-factor-weighted flux continuity (the second `G` rows) across
//! that face; every outer face gets a `G`×`G` system enforcing the boundary
//! condition. The unknowns are the two adjacent nodes' first-order expansion
//! coefficients, of which only the low node's are kept.

use super::buckling::Buckling;
use super::geometry::{Axis, BoundaryCondition, DirectionVectors, Face, NodalGeometry, NodalParams};
use super::sparse::{solve_dense_in_place, SparseMatrix};

/// An odd-order expansion coefficient set — MATLAB's `A1` and `A3` structs,
/// which share this shape.
///
/// Same units as the flux (neutrons cm⁻² s⁻¹). Each `x`/`y`/`z` entry belongs
/// to the face on the **high** side of its node; the `*_first` entries exist
/// only at the low-boundary node of each line, where the outer face has no
/// partner to share a coefficient with. The even orders `A2` and `A4` carry no
/// `*_first` variant and are plain [`DirectionVectors`].
#[derive(Debug, Clone, PartialEq)]
pub struct OddExpansion {
    /// Coefficient on the high-x face of each node. MATLAB `A1.x`.
    pub x: Vec<f64>,
    /// Coefficient on the high-y face of each node. MATLAB `A1.y`.
    pub y: Vec<f64>,
    /// Coefficient on the high-z face of each node. MATLAB `A1.z`.
    pub z: Vec<f64>,
    /// Coefficient on the low-x outer face. MATLAB `A1.xfirst`; zero except at
    /// the low-boundary node of each x line.
    pub x_first: Vec<f64>,
    /// Coefficient on the low-y outer face. MATLAB `A1.yfirst`.
    pub y_first: Vec<f64>,
    /// Coefficient on the low-z outer face. MATLAB `A1.zfirst`.
    pub z_first: Vec<f64>,
}

impl OddExpansion {
    /// All six vectors zeroed, for a state vector of length `n`.
    #[must_use]
    pub fn zeros(n: usize) -> Self {
        Self {
            x: vec![0.0; n],
            y: vec![0.0; n],
            z: vec![0.0; n],
            x_first: vec![0.0; n],
            y_first: vec![0.0; n],
            z_first: vec![0.0; n],
        }
    }

    /// The high-face coefficients along `axis`.
    #[must_use]
    pub fn axis(&self, axis: Axis) -> &[f64] {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
            Axis::Z => &self.z,
        }
    }

    /// The low-outer-face coefficients along `axis`.
    #[must_use]
    pub fn axis_first(&self, axis: Axis) -> &[f64] {
        match axis {
            Axis::X => &self.x_first,
            Axis::Y => &self.y_first,
            Axis::Z => &self.z_first,
        }
    }

    fn axis_mut(&mut self, axis: Axis) -> &mut Vec<f64> {
        match axis {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
            Axis::Z => &mut self.z,
        }
    }

    fn axis_first_mut(&mut self, axis: Axis) -> &mut Vec<f64> {
        match axis {
            Axis::X => &mut self.x_first,
            Axis::Y => &mut self.y_first,
            Axis::Z => &mut self.z_first,
        }
    }
}

/// Solves for the first-order expansion coefficients —
/// `calc_a1_expansionxyz.m`.
///
/// # Interior faces
///
/// For the face between low node `l` and high node `h`, with `d = 2D/L`
/// \[cm⁻¹\], `B` the within-node `G`×`G` buckling block \[dimensionless\], and
/// `f` the low/high assembly discontinuity factors, the `2G`×`2G` system is
///
/// ```text
/// [ -d_l F_l B_l - d_l I     d_h F_h B_h + d_h I ] [ a_l ]   [  b_l + b'_h  ]
/// [  f_l A_l B_l + f_l I     f_h A_h B_h + f_h I ] [ a_h ] = [  b2'_h - b2_l ]
/// ```
///
/// The top block is current continuity, the bottom flux continuity across the
/// discontinuity factors. Only `a_l` is retained.
///
/// # Outer faces
///
/// A `G`×`G` system per node, whose form depends on the boundary condition; see
/// the reference for the three cases. The low-boundary result is stored in the
/// `*_first` vectors, the high-boundary result overwrites the corresponding
/// `x`/`y`/`z` entry.
///
/// # Unfinished / fragile in the reference, recorded not repaired
///
/// - **The outer-face system is assembled even for nodes with no material.**
///   The `if diffvalues(idx)==0, continue` guard skips filling the rows but not
///   the diagonal adjustment that follows, and the solve always runs. With the
///   default unit discontinuity factors the resulting matrix is `±I`, so the
///   answer is harmless; with a reflective outer face the diagonal term is
///   `diffvec`, which is zero there, making the system exactly singular. The
///   MATLAB then warns and produces `Inf`/`NaN`; this port produces the same
///   non-finite values (see [`solve_dense_in_place`]).
/// - **A comment mislabels the x high-boundary block as `%zhi node`.** Cosmetic
///   only; the arithmetic underneath is the x one.
///
/// # Panics
///
/// If any input vector is not of the neutronics state length, or if a line has
/// a single node so that a boundary block reaches past the end of the grid.
#[must_use]
#[allow(clippy::too_many_arguments)]
// The `for g in 0..ngroups` loops index several parallel arrays at once and
// mirror the MATLAB's own group loops; an iterator rewrite would obscure that.
#[allow(clippy::needless_range_loop)]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    flux: &[f64],
    second_order: &DirectionVectors,
    fourth_order: &DirectionVectors,
    leakage_first: &DirectionVectors,
    diffusion: &[f64],
    buckling: &Buckling,
) -> OddExpansion {
    let grid = params.grid;
    let philen = params.philen();
    let nodes = grid.nodes();
    let ngroups = grid.ngroups;
    assert_eq!(diffusion.len(), philen, "diffusion length");

    let coeffs = &geometry.nodal_coefficients;
    let mut out = OddExpansion::zeros(philen);

    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let width = geometry.width_state_vector(axis, grid);
        let stride = axis.stride(grid);
        let (k1_len, k2_len) = axis.line_counts(grid);
        let range = geometry.range(axis);
        let (low_face, high_face) = (Face::minus(axis), Face::plus(axis));

        // diffvec.d(i) = 2*D(i)/L_d(i)   [cm^-1]
        let diffvec: Vec<f64> = (0..philen).map(|i| 2.0 * diffusion[i] / width[i]).collect();

        // Dense within-node GxG buckling blocks, as the MATLAB pre-extracts
        // them: BuckBlk(i, gc) == Buck(i, gc-th group at i's spatial node).
        let buck = buckling.axis(axis);
        let blocks = within_node_blocks(buck, nodes, ngroups);

        let a2 = second_order.axis(axis);
        let a4 = fourth_order.axis(axis);
        let l1 = leakage_first.axis(axis);
        let aa = coeffs.aa.axis(axis);
        let ff = coeffs.ff.axis(axis);
        let gg = coeffs.gg.axis(axis);
        let hh = coeffs.hh.axis(axis);

        let bdummy: Vec<f64> = (0..philen)
            .map(|i| diffvec[i] * (3.0 * a2[i] + gg[i] * a4[i] + ff[i] * l1[i]))
            .collect();
        let bdummy_plus: Vec<f64> = (0..philen)
            .map(|i| diffvec[i] * (3.0 * a2[i] + gg[i] * a4[i] - ff[i] * l1[i]))
            .collect();
        let bdummy2: Vec<f64> = (0..philen)
            .map(|i| geometry.adf.get(i, high_face) * (a2[i] + a4[i] + flux[i] + aa[i] * l1[i]))
            .collect();
        let bdummy_plus2: Vec<f64> = (0..philen)
            .map(|i| geometry.adf.get(i, low_face) * (a2[i] + a4[i] + flux[i] - aa[i] * l1[i]))
            .collect();

        // ----- interior faces -----
        let n = 2 * ngroups;
        for k1 in 0..k1_len {
            for k2 in 0..k2_len {
                let low = range.low(k1, k2);
                let high = range.high(k1, k2);
                for pos in low..high {
                    let (ix, iy, iz) = axis.coords(k1, k2, pos);
                    if diffusion[grid.index(0, ix, iy, iz)] == 0.0 {
                        continue;
                    }
                    let mut mat = vec![0.0; n * n];
                    let mut rhs = vec![0.0; n];
                    for g in 0..ngroups {
                        let il = grid.index(g, ix, iy, iz);
                        let ih = il + stride;
                        let dl = diffvec[il];
                        let fl = ff[il];
                        let al = aa[il];
                        let adf_l = geometry.adf.get(il, high_face);
                        let dh = diffvec[ih];
                        let fh = ff[ih];
                        let ah = aa[ih];
                        let adf_h = geometry.adf.get(ih, low_face);
                        for g2 in 0..ngroups {
                            let de = f64::from(g == g2);
                            mat[g * n + g2] = -dl * fl * blocks[il * ngroups + g2] - de * dl;
                            mat[g * n + ngroups + g2] =
                                dh * fh * blocks[ih * ngroups + g2] + de * dh;
                            mat[(ngroups + g) * n + g2] =
                                adf_l * al * blocks[il * ngroups + g2] + de * adf_l;
                            mat[(ngroups + g) * n + ngroups + g2] =
                                adf_h * ah * blocks[ih * ngroups + g2] + de * adf_h;
                        }
                        rhs[g] = bdummy[il] + bdummy_plus[ih];
                        rhs[ngroups + g] = bdummy_plus2[ih] - bdummy2[il];
                    }
                    solve_dense_in_place(&mut mat, n, &mut rhs);
                    for g in 0..ngroups {
                        out.axis_mut(axis)[grid.index(g, ix, iy, iz)] = rhs[g];
                    }
                }
            }
        }

        // ----- outer faces -----
        for k1 in 0..k1_len {
            for k2 in 0..k2_len {
                let low = range.low(k1, k2);
                let high = range.high(k1, k2);

                // low boundary -> A1.*first
                {
                    let (ix, iy, iz) = axis.coords(k1, k2, low);
                    let bc = geometry.boundaries.low(axis);
                    let mut mat = vec![0.0; ngroups * ngroups];
                    let mut rhs = vec![0.0; ngroups];
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        if diffusion[i] == 0.0 {
                            continue;
                        }
                        let f = geometry.adf.get(i, low_face);
                        match bc {
                            BoundaryCondition::Vacuum => {
                                for g2 in 0..ngroups {
                                    let b = blocks[i * ngroups + g2];
                                    mat[g * ngroups + g2] =
                                        -f * aa[i] * b - 2.0 * diffvec[i] * aa[i] * b * hh[i];
                                }
                                rhs[g] = 2.0
                                    * diffvec[i]
                                    * (aa[i] * l1[i] * hh[i] - 3.0 * a2[i] - gg[i] * a4[i]);
                                rhs[g] -= f * (a2[i] + a4[i] + flux[i] - aa[i] * l1[i]);
                            }
                            BoundaryCondition::Reflective => {
                                for g2 in 0..ngroups {
                                    mat[g * ngroups + g2] =
                                        diffvec[i] * ff[i] * blocks[i * ngroups + g2];
                                }
                                rhs[g] = bdummy_plus[i];
                            }
                            BoundaryCondition::ZeroFlux => {
                                for g2 in 0..ngroups {
                                    mat[g * ngroups + g2] = f * aa[i] * blocks[i * ngroups + g2];
                                }
                                rhs[g] = f * (a2[i] + a4[i] + flux[i] - aa[i] * l1[i]);
                            }
                        }
                    }
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        let f = geometry.adf.get(i, low_face);
                        mat[g * ngroups + g] += match bc {
                            BoundaryCondition::Vacuum => -(2.0 * diffvec[i] + f),
                            BoundaryCondition::Reflective => diffvec[i],
                            BoundaryCondition::ZeroFlux => f,
                        };
                    }
                    solve_dense_in_place(&mut mat, ngroups, &mut rhs);
                    for g in 0..ngroups {
                        out.axis_first_mut(axis)[grid.index(g, ix, iy, iz)] = rhs[g];
                    }
                }

                // high boundary -> A1.{x,y,z}
                {
                    let (ix, iy, iz) = axis.coords(k1, k2, high);
                    let bc = geometry.boundaries.high(axis);
                    let mut mat = vec![0.0; ngroups * ngroups];
                    let mut rhs = vec![0.0; ngroups];
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        if diffusion[i] == 0.0 {
                            continue;
                        }
                        let f = geometry.adf.get(i, high_face);
                        match bc {
                            BoundaryCondition::Vacuum => {
                                for g2 in 0..ngroups {
                                    let b = blocks[i * ngroups + g2];
                                    mat[g * ngroups + g2] =
                                        f * aa[i] * b + 2.0 * diffvec[i] * aa[i] * b * hh[i];
                                }
                                rhs[g] = -2.0
                                    * diffvec[i]
                                    * (aa[i] * l1[i] * hh[i] + 3.0 * a2[i] + gg[i] * a4[i]);
                                rhs[g] -= f * (a2[i] + a4[i] + flux[i] + aa[i] * l1[i]);
                            }
                            BoundaryCondition::Reflective => {
                                for g2 in 0..ngroups {
                                    mat[g * ngroups + g2] =
                                        -diffvec[i] * ff[i] * blocks[i * ngroups + g2];
                                }
                                rhs[g] = bdummy[i];
                            }
                            BoundaryCondition::ZeroFlux => {
                                for g2 in 0..ngroups {
                                    mat[g * ngroups + g2] = f * aa[i] * blocks[i * ngroups + g2];
                                }
                                rhs[g] = -f * (a2[i] + a4[i] + flux[i] - aa[i] * l1[i]);
                            }
                        }
                    }
                    for g in 0..ngroups {
                        let i = grid.index(g, ix, iy, iz);
                        let f = geometry.adf.get(i, high_face);
                        mat[g * ngroups + g] += match bc {
                            BoundaryCondition::Vacuum => 2.0 * diffvec[i] + f,
                            BoundaryCondition::Reflective => -diffvec[i],
                            BoundaryCondition::ZeroFlux => f,
                        };
                    }
                    solve_dense_in_place(&mut mat, ngroups, &mut rhs);
                    for g in 0..ngroups {
                        out.axis_mut(axis)[grid.index(g, ix, iy, iz)] = rhs[g];
                    }
                }
            }
        }
    }

    out
}

/// Extracts the within-node `G`×`G` blocks of a buckling operator into a dense
/// `philen`×`G` table, indexed `i * ngroups + g2`.
///
/// The buckling operators couple energy groups only at the same spatial node,
/// so row `i` has at most `G` nonzeros and they sit at the `G` group indices of
/// `i`'s own node. This mirrors the MATLAB's `BuckBlk*` pre-extraction, which
/// exists there purely to avoid repeated sparse slicing.
fn within_node_blocks(buck: &SparseMatrix, nodes: usize, ngroups: usize) -> Vec<f64> {
    let philen = nodes * ngroups;
    let mut blocks = vec![0.0; philen * ngroups];
    for i in 0..philen {
        let spatial = i % nodes;
        for (g2, slot) in blocks[i * ngroups..(i + 1) * ngroups]
            .iter_mut()
            .enumerate()
        {
            *slot = buck.get(i, g2 * nodes + spatial);
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::geometry::BoundaryConditions;

    #[test]
    fn within_node_blocks_pick_out_the_group_coupling_at_each_node() {
        // 2 spatial nodes, 2 groups -> philen 4. Only same-node couplings.
        let m = SparseMatrix::from_triplets(
            4,
            4,
            &[
                (0, 0, 1.0), // node 0, fast<-fast
                (0, 2, 2.0), // node 0, fast<-thermal
                (3, 1, 5.0), // node 1, thermal<-fast
            ],
        );
        let b = within_node_blocks(&m, 2, 2);
        assert_eq!(b[0], 1.0);
        assert_eq!(b[1], 2.0);
        assert_eq!(b[3 * 2], 5.0); // node 1 thermal row
        assert_eq!(b[3 * 2 + 1], 0.0);
    }

    /// A one-group 2×2×2 block whose nodal coefficients are all zero and whose
    /// buckling is empty. Then the interior 2×2 system reduces to
    /// `[-d_l, d_h; 1, 1] [a_l; a_h] = [0; phi_h - phi_l]`, which has the closed
    /// form `a_l = d_h (phi_h - phi_l) / (d_l + d_h)`.
    #[test]
    fn interior_face_reduces_to_a_known_two_by_two_solve() {
        let grid = Grid::new(2, 2, 2, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; n],
            vec![10.0; n],
            vec![10.0; n],
            vec![1; n],
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        let philen = params.philen();
        let d = vec![1.0; philen];
        let zero = DirectionVectors::zeros(philen);
        let buck = Buckling {
            x: SparseMatrix::from_triplets(philen, philen, &[]),
            y: SparseMatrix::from_triplets(philen, philen, &[]),
            z: SparseMatrix::from_triplets(philen, philen, &[]),
        };
        // phi = 1 at iz = 0, 3 at iz = 1.
        let mut phi = vec![0.0; philen];
        for ix in 0..2 {
            for iy in 0..2 {
                phi[grid.index(0, ix, iy, 0)] = 1.0;
                phi[grid.index(0, ix, iy, 1)] = 3.0;
            }
        }
        let a1 = assemble(&params, &geometry, &phi, &zero, &zero, &zero, &d, &buck);
        // d_l = d_h = 2*1/10 = 0.2, so a_l = 0.2*2/0.4 = 1.
        // The low node of each axial pair is the boundary node too, so A1.z
        // there is written by the interior batch and then left alone (the high
        // boundary block writes iz = 1).
        assert!((a1.z[grid.index(0, 0, 0, 0)] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_reflective_outer_face_over_dead_material_yields_non_finite_values() {
        // Documented reference fragility: reflective + D == 0 makes the GxG
        // outer system exactly singular.
        let grid = Grid::new(2, 2, 2, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; n],
            vec![10.0; n],
            vec![10.0; n],
            vec![0; n],
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        let philen = params.philen();
        let d = vec![0.0; philen];
        let zero = DirectionVectors::zeros(philen);
        let buck = Buckling {
            x: SparseMatrix::from_triplets(philen, philen, &[]),
            y: SparseMatrix::from_triplets(philen, philen, &[]),
            z: SparseMatrix::from_triplets(philen, philen, &[]),
        };
        let phi = vec![1.0; philen];
        let a1 = assemble(&params, &geometry, &phi, &zero, &zero, &zero, &d, &buck);
        assert!(a1.z_first.iter().any(|v| !v.is_finite()));
    }

    #[test]
    fn a_vacuum_outer_face_over_dead_material_stays_finite() {
        // With unit discontinuity factors the vacuum system degenerates to -I,
        // which is well conditioned, so the reference survives this case.
        let grid = Grid::new(2, 2, 2, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; n],
            vec![10.0; n],
            vec![10.0; n],
            vec![0; n],
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let philen = params.philen();
        let d = vec![0.0; philen];
        let zero = DirectionVectors::zeros(philen);
        let buck = Buckling {
            x: SparseMatrix::from_triplets(philen, philen, &[]),
            y: SparseMatrix::from_triplets(philen, philen, &[]),
            z: SparseMatrix::from_triplets(philen, philen, &[]),
        };
        let phi = vec![1.0; philen];
        let a1 = assemble(&params, &geometry, &phi, &zero, &zero, &zero, &d, &buck);
        assert!(a1.z_first.iter().all(|v| v.is_finite()));
    }
}
