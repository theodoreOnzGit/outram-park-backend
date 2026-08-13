//! The finite-difference leakage operator and its per-face coupling terms.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Source: `makegradDxyz.m` (function `makegradDxyz`).

use super::geometry::{Axis, BoundaryCondition, Face, FaceTerms, NodalGeometry, NodalParams};
use super::sparse::SparseMatrix;

/// The finite-difference leakage operator and the face-coupling terms the
/// nodal correction is built on top of.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientDiffusion {
    /// `gradD` — the assembled leakage operator \[cm⁻¹\], `philenf` square.
    ///
    /// Its diagonal starts as the identity and is overwritten in-core, so rows
    /// for nodes outside the core keep a `1` and the operator stays
    /// nonsingular there.
    pub operator: SparseMatrix,
    /// `gradterms` — the six per-face coupled diffusion coefficients
    /// `2*Dtilde` \[cm\] used by the transverse-leakage and nodal-correction
    /// stages.
    ///
    /// The MATLAB doubles the whole table on the last line
    /// (`gradterms=2*gradterms; %check this (seems correct)`); that comment is
    /// the author's, and the doubling is reproduced without judgement.
    pub face_terms: FaceTerms,
}

/// Builds the finite-difference leakage operator — `makegradDxyz.m`.
///
/// For each interior face the coupled diffusion coefficient is
///
/// ```text
/// Dtilde+ = 0.5*(h + h+) * D*D+ / (h*D + h+*D+) / L
/// ```
///
/// with `h = L/2` the node half-width \[cm\], `D` the diffusion coefficient
/// \[cm\], and the result contributing `Dtilde+/h+` to the diagonal and
/// `-Dtilde+/h+` to the off-diagonal. Outer faces substitute a
/// boundary-condition-dependent `Dtilde`:
///
/// | Condition | Outer `Dtilde` |
/// |---|---|
/// | `Vacuum` | `0.5*D / (2*D + 0.5*L)` |
/// | `Reflective` | `0` |
/// | `ZeroFlux` | `D / L` |
///
/// Directions are swept z, then y, then x. **The z sweep assigns the diagonal;
/// the y and x sweeps accumulate onto it.** That asymmetry is in the original
/// and is preserved — it is why z must be swept first.
///
/// `diffusion` is the flat diffusion-coefficient state vector from
/// [`super::cross_sections::diffusion_coefficients`] \[cm\]; `which_sigma` is
/// the 1-based material index per spatial node, `0` skipping the node.
///
/// # Not ported
///
/// The `tomode ~= 1` branch calls `convertsparseformat2d`, which converts to
/// the 2-D half-index layout. No SANM call site uses it, so only `tomode = 1`
/// is translated.
///
/// # Panics
///
/// If a boundary node sits at the very edge of the grid so that its `+1`
/// neighbour does not exist — the MATLAB indexes out of bounds and errors in
/// exactly the same situation.
#[must_use]
pub fn assemble(
    params: &NodalParams,
    geometry: &NodalGeometry,
    diffusion: &[f64],
    which_sigma: &[usize],
) -> GradientDiffusion {
    let grid = params.grid;
    let philen = params.philen();
    let philenf = params.philenf();
    let ngroups = grid.ngroups;

    // The identity block the MATLAB seeds gradDele with.
    let mut diagonal = vec![1.0; philen];
    let mut off_diagonal: Vec<(usize, usize, f64)> = Vec::new();
    let mut face_terms = FaceTerms::zeros(philen);

    let node_of = |ix: usize, iy: usize, iz: usize| ix * (grid.ny * grid.nz) + iy * grid.nz + iz;

    // ----- z direction: assigns the diagonal -----
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            let zlow = geometry.z_range.low(ix, iy);
            let zhi = geometry.z_range.high(ix, iy);

            for iz in (zlow + 1)..zhi {
                if which_sigma[node_of(ix, iy, iz)] == 0 {
                    continue;
                }
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, iz);
                    let h = geometry.lz[node_of(ix, iy, iz)] / 2.0;
                    let hplus = geometry.lz[node_of(ix, iy, iz + 1)] / 2.0;
                    let hminus = geometry.lz[node_of(ix, iy, iz - 1)] / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, ix, iy, iz + 1)];
                    let dm = diffusion[grid.index(g, ix, iy, iz - 1)];
                    let l = geometry.lz[node_of(ix, iy, iz)];

                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;

                    diagonal[idx] = dtilde_plus / hplus + dtilde_minus / hminus;
                    off_diagonal.push((idx, idx + 1, -dtilde_plus / hplus));
                    off_diagonal.push((idx, idx - 1, -dtilde_minus / hminus));
                    face_terms.set(idx, Face::ZMinus, dtilde_minus);
                    face_terms.set(idx, Face::ZPlus, dtilde_plus);
                }
            }

            // low-z outer face
            if which_sigma[node_of(ix, iy, zlow)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, zlow);
                    let l = geometry.lz[node_of(ix, iy, zlow)];
                    let h = l / 2.0;
                    let hplus = geometry.lz[node_of(ix, iy, zlow + 1)] / 2.0;
                    let hminus = l / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, ix, iy, zlow + 1)];
                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = outer_dtilde(geometry.boundaries.z_min, d, l);
                    off_diagonal.push((idx, idx + 1, -dtilde_plus / hplus));
                    diagonal[idx] = dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::ZMinus, dtilde_minus);
                    face_terms.set(idx, Face::ZPlus, dtilde_plus);
                }
            }

            // high-z outer face
            if which_sigma[node_of(ix, iy, zhi)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, zhi);
                    let l = geometry.lz[node_of(ix, iy, zhi)];
                    let h = l / 2.0;
                    let hplus = l / 2.0;
                    let hminus = geometry.lz[node_of(ix, iy, zhi - 1)] / 2.0;
                    let d = diffusion[idx];
                    let dm = diffusion[grid.index(g, ix, iy, zhi - 1)];
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;
                    let dtilde_plus = outer_dtilde(geometry.boundaries.z_max, d, l);
                    off_diagonal.push((idx, idx - 1, -dtilde_minus / hminus));
                    diagonal[idx] = dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::ZMinus, dtilde_minus);
                    face_terms.set(idx, Face::ZPlus, dtilde_plus);
                }
            }
        }
    }

    // ----- y direction: accumulates onto the diagonal -----
    let ystep = grid.nz;
    for ix in 0..grid.nx {
        for iz in 0..grid.nz {
            let ylow = geometry.y_range.low(ix, iz);
            let yhi = geometry.y_range.high(ix, iz);

            for iy in (ylow + 1)..yhi {
                if which_sigma[node_of(ix, iy, iz)] == 0 {
                    continue;
                }
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, iz);
                    let l = geometry.ly[node_of(ix, iy, iz)];
                    let h = l / 2.0;
                    let hplus = geometry.ly[node_of(ix, iy + 1, iz)] / 2.0;
                    let hminus = geometry.ly[node_of(ix, iy - 1, iz)] / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, ix, iy + 1, iz)];
                    let dm = diffusion[grid.index(g, ix, iy - 1, iz)];

                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;

                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    off_diagonal.push((idx, idx + ystep, -dtilde_plus / hplus));
                    off_diagonal.push((idx, idx - ystep, -dtilde_minus / hminus));
                    face_terms.set(idx, Face::YMinus, dtilde_minus);
                    face_terms.set(idx, Face::YPlus, dtilde_plus);
                }
            }

            if which_sigma[node_of(ix, ylow, iz)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, ylow, iz);
                    let l = geometry.ly[node_of(ix, ylow, iz)];
                    let h = l / 2.0;
                    let hplus = geometry.ly[node_of(ix, ylow + 1, iz)] / 2.0;
                    let hminus = l / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, ix, ylow + 1, iz)];
                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = outer_dtilde(geometry.boundaries.y_min, d, l);
                    off_diagonal.push((idx, idx + ystep, -dtilde_plus / hplus));
                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::YMinus, dtilde_minus);
                    face_terms.set(idx, Face::YPlus, dtilde_plus);
                }
            }

            if which_sigma[node_of(ix, yhi, iz)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, yhi, iz);
                    let l = geometry.ly[node_of(ix, yhi, iz)];
                    let h = l / 2.0;
                    let hplus = l / 2.0;
                    let hminus = geometry.ly[node_of(ix, yhi - 1, iz)] / 2.0;
                    let d = diffusion[idx];
                    let dm = diffusion[grid.index(g, ix, yhi - 1, iz)];
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;
                    let dtilde_plus = outer_dtilde(geometry.boundaries.y_max, d, l);
                    off_diagonal.push((idx, idx - ystep, -dtilde_minus / hminus));
                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::YMinus, dtilde_minus);
                    face_terms.set(idx, Face::YPlus, dtilde_plus);
                }
            }
        }
    }

    // ----- x direction: accumulates onto the diagonal -----
    let xstep = grid.ny * grid.nz;
    for iy in 0..grid.ny {
        for iz in 0..grid.nz {
            let xlow = geometry.x_range.low(iy, iz);
            let xhi = geometry.x_range.high(iy, iz);

            for ix in (xlow + 1)..xhi {
                if which_sigma[node_of(ix, iy, iz)] == 0 {
                    continue;
                }
                for g in 0..ngroups {
                    let idx = grid.index(g, ix, iy, iz);
                    let l = geometry.lx[node_of(ix, iy, iz)];
                    let h = l / 2.0;
                    let hplus = geometry.lx[node_of(ix + 1, iy, iz)] / 2.0;
                    let hminus = geometry.lx[node_of(ix - 1, iy, iz)] / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, ix + 1, iy, iz)];
                    let dm = diffusion[grid.index(g, ix - 1, iy, iz)];

                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;

                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    off_diagonal.push((idx, idx + xstep, -dtilde_plus / hplus));
                    off_diagonal.push((idx, idx - xstep, -dtilde_minus / hminus));
                    face_terms.set(idx, Face::XMinus, dtilde_minus);
                    face_terms.set(idx, Face::XPlus, dtilde_plus);
                }
            }

            if which_sigma[node_of(xlow, iy, iz)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, xlow, iy, iz);
                    let l = geometry.lx[node_of(xlow, iy, iz)];
                    let h = l / 2.0;
                    let hplus = geometry.lx[node_of(xlow + 1, iy, iz)] / 2.0;
                    let hminus = l / 2.0;
                    let d = diffusion[idx];
                    let dp = diffusion[grid.index(g, xlow + 1, iy, iz)];
                    let dtilde_plus = 0.5 * (h + hplus) * (d * dp) / (h * d + hplus * dp) / l;
                    let dtilde_minus = outer_dtilde(geometry.boundaries.x_min, d, l);
                    off_diagonal.push((idx, idx + xstep, -dtilde_plus / hplus));
                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::XMinus, dtilde_minus);
                    face_terms.set(idx, Face::XPlus, dtilde_plus);
                }
            }

            if which_sigma[node_of(xhi, iy, iz)] != 0 {
                for g in 0..ngroups {
                    let idx = grid.index(g, xhi, iy, iz);
                    let l = geometry.lx[node_of(xhi, iy, iz)];
                    let h = l / 2.0;
                    let hplus = l / 2.0;
                    let hminus = geometry.lx[node_of(xhi - 1, iy, iz)] / 2.0;
                    let d = diffusion[idx];
                    let dm = diffusion[grid.index(g, xhi - 1, iy, iz)];
                    let dtilde_minus = 0.5 * (h + hminus) * (d * dm) / (h * d + hminus * dm) / l;
                    let dtilde_plus = outer_dtilde(geometry.boundaries.x_max, d, l);
                    off_diagonal.push((idx, idx - xstep, -dtilde_minus / hminus));
                    diagonal[idx] += dtilde_plus / hplus + dtilde_minus / hminus;
                    face_terms.set(idx, Face::XMinus, dtilde_minus);
                    face_terms.set(idx, Face::XPlus, dtilde_plus);
                }
            }
        }
    }

    let mut triplets: Vec<(usize, usize, f64)> = Vec::with_capacity(philen + off_diagonal.len());
    for (idx, &v) in diagonal.iter().enumerate() {
        triplets.push((idx, idx, v));
    }
    triplets.extend(off_diagonal);

    // gradterms=2*gradterms
    face_terms.scale(2.0);

    GradientDiffusion {
        operator: SparseMatrix::from_triplets(philenf, philenf, &triplets),
        face_terms,
    }
}

/// The outer-face coupled diffusion coefficient for one boundary condition.
///
/// `d` is the node's diffusion coefficient \[cm\], `l` its width along the
/// direction of the face \[cm\]. The result has units of \[cm\], matching the
/// interior `Dtilde`.
///
/// # Recorded oddity
///
/// The interior `Dtilde` is divided by `L`, but the vacuum and reflective outer
/// values here are not, while the zero-flux one is. The MATLAB carries a
/// commented-out vacuum expression that *did* divide by `L`
/// (`Dtildeminus=(DiffD)/(2*DiffD+0.5*h)/(Lzb)`), replaced by the undivided
/// form now in use. Left exactly as the reference has it.
fn outer_dtilde(bc: BoundaryCondition, d: f64, l: f64) -> f64 {
    match bc {
        BoundaryCondition::Vacuum => 0.5 * d / (2.0 * d + 0.5 * l),
        BoundaryCondition::Reflective => 0.0,
        BoundaryCondition::ZeroFlux => d / l,
    }
}

/// The neighbour stride along `axis` in the flat state vector.
///
/// `+1` in z, `+nz` in y, `+ny*nz` in x — the MATLAB's `1`, `maxiz` and
/// `xstep`.
#[must_use]
pub const fn neighbour_stride(axis: Axis, grid: crate::reference::grid::Grid) -> usize {
    match axis {
        Axis::X => grid.ny * grid.nz,
        Axis::Y => grid.nz,
        Axis::Z => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::cross_sections::diffusion_coefficients;
    use crate::reference::nodal::geometry::{BoundaryCondition, BoundaryConditions};

    /// A 2×2×3 block of identical 10 cm nodes with `D == 1` exactly, all six
    /// faces reflective.
    ///
    /// Two nodes is the **minimum** in every direction: with one node a line's
    /// `low` and `high` coincide, and both boundary blocks then reach for a
    /// neighbour outside the grid. The MATLAB errors there too (`Lxb(xlow+1,
    /// ...)` is out of bounds), so a single-node direction is simply not a
    /// supported mesh in the reference.
    fn uniform_block() -> (NodalParams, NodalGeometry, Vec<f64>, Vec<usize>) {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        let n = grid.nodes();
        let which_sigma = vec![1usize; n];
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; n],
            vec![10.0; n],
            vec![10.0; n],
            which_sigma.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        // sigma_tot = 1/3 -> D = 1 exactly, so the arithmetic below is clean.
        let d = diffusion_coefficients(grid, &[vec![1.0 / 3.0]], &which_sigma, 1.0);
        (params, geometry, d, which_sigma)
    }

    #[test]
    fn uniform_interior_row_is_the_textbook_three_point_stencil() {
        let (params, geometry, d, ws) = uniform_block();
        let grid = params.grid;
        let g = assemble(&params, &geometry, &d, &ws);
        // D = 1, L = 10, h = h+ = h- = 5.
        // Dtilde = 0.5*10*1/(5+5)/10 = 0.05; contribution 0.05/5 = 0.01 a side.
        let i = grid.index(0, 0, 0, 1);
        assert!((g.operator.get(i, i - 1) + 0.01).abs() < 1e-14);
        assert!((g.operator.get(i, i + 1) + 0.01).abs() < 1e-14);
        // Two axial faces plus one x face plus one y face: 4 x 0.01.
        assert!((g.operator.get(i, i) - 0.04).abs() < 1e-14);
        // Row sums to zero: a fully reflective block conserves neutrons.
        let row: f64 = (0..grid.state_len()).map(|c| g.operator.get(i, c)).sum();
        assert!(row.abs() < 1e-14);
    }

    #[test]
    fn reflective_outer_faces_contribute_nothing() {
        let (params, geometry, d, ws) = uniform_block();
        let grid = params.grid;
        let g = assemble(&params, &geometry, &d, &ws);
        // The low-z node sees only its +z face, plus one x and one y face.
        let i = grid.index(0, 0, 0, 0);
        assert!((g.operator.get(i, i) - 0.03).abs() < 1e-14);
        assert!((g.operator.get(i, i + 1) + 0.01).abs() < 1e-14);
        assert_eq!(g.face_terms.get(i, Face::ZMinus), 0.0);
        assert_eq!(g.face_terms.get(i, Face::XMinus), 0.0);
    }

    #[test]
    fn face_terms_carry_twice_dtilde() {
        let (params, geometry, d, ws) = uniform_block();
        let grid = params.grid;
        let g = assemble(&params, &geometry, &d, &ws);
        let i = grid.index(0, 0, 0, 1);
        // Dtilde = 0.05, doubled on the last line of makegradDxyz.
        assert!((g.face_terms.get(i, Face::ZPlus) - 0.1).abs() < 1e-14);
        assert!((g.face_terms.get(i, Face::ZMinus) - 0.1).abs() < 1e-14);
    }

    #[test]
    fn nodes_outside_the_core_keep_the_identity_diagonal() {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
        // Only the middle axial plane has material; the top and bottom planes
        // are empty, so every loop skips them.
        let mut ws = vec![0usize; grid.nodes()];
        for ix in 0..2 {
            for iy in 0..2 {
                ws[ix * 6 + iy * 3 + 1] = 1;
            }
        }
        let geometry = NodalGeometry::new(
            grid,
            vec![10.0; grid.nodes()],
            vec![10.0; grid.nodes()],
            vec![10.0; grid.nodes()],
            ws.clone(),
            BoundaryConditions::uniform(BoundaryCondition::Vacuum),
        );
        let d = diffusion_coefficients(grid, &[vec![1.0 / 3.0]], &ws, 1.0);
        let g = assemble(&params, &geometry, &d, &ws);
        let empty = grid.index(0, 0, 0, 0);
        assert_eq!(g.operator.get(empty, empty), 1.0);
        let empty = grid.index(0, 1, 1, 2);
        assert_eq!(g.operator.get(empty, empty), 1.0);
    }

    #[test]
    fn vacuum_outer_dtilde_matches_the_reference_expression() {
        // 0.5*D/(2*D + 0.5*L) with D = 1, L = 10 -> 0.5/7.
        assert!((outer_dtilde(BoundaryCondition::Vacuum, 1.0, 10.0) - 0.5 / 7.0).abs() < 1e-15);
        assert_eq!(outer_dtilde(BoundaryCondition::Reflective, 1.0, 10.0), 0.0);
        assert_eq!(outer_dtilde(BoundaryCondition::ZeroFlux, 1.0, 10.0), 0.1);
    }

    #[test]
    fn neighbour_strides_match_the_matlab_offsets() {
        let grid = Grid::new(17, 17, 19, 2).expect("valid grid");
        assert_eq!(neighbour_stride(Axis::Z, grid), 1);
        assert_eq!(neighbour_stride(Axis::Y, grid), 19);
        assert_eq!(neighbour_stride(Axis::X, grid), 17 * 19);
    }
}
