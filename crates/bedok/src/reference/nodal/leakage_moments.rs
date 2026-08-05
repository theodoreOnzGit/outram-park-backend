//! The first and second transverse-leakage moments.
//!
//! # Provenance
//!
//! Original author: **Than Yan Ren**, Singapore Nuclear Research and Safety
//! Institute (SNRSI). Snapshot `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c`.
//! Translated under the permission recorded in `docs/bedok-port-scoping.md` §6.
//!
//! Sources: `calc_1sttransleakagexyz.m` (function `calc_1sttransleakagexyz`)
//! and `calc_2ndtransleakagexyz.m` (function `calc_2ndtransleakagexyz`).
//!
//! Both files implement the classic quadratic transverse-leakage fit: the
//! transverse leakage seen by the one-dimensional solution along one axis is
//! approximated by the parabola through the node-average leakages of the node
//! and its two neighbours along that axis, and these two routines return the
//! parabola's first and second moments.

use super::geometry::{Axis, BoundaryCondition, DirectionVectors, NodalGeometry, NodalParams};

/// Which moment of the transverse-leakage parabola to evaluate.
///
/// The two share their loop structure, their node-width ratios and their
/// scaling; they differ only in the interior numerator and in what happens on
/// a non-reflective outer face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    /// The first (odd) moment — `calc_1sttransleakagexyz.m`.
    First,
    /// The second (even) moment — `calc_2ndtransleakagexyz.m`.
    Second,
}

/// The first transverse-leakage moment — `calc_1sttransleakagexyz.m`.
///
/// See [`moment`] for the formulas. Result units follow the input: with a
/// zeroth-moment leakage in neutrons cm⁻³ s⁻¹ and `D` in cm, the moment is in
/// neutrons cm⁻² s⁻¹, the same units as the flux.
#[must_use]
pub fn first_moment(
    params: &NodalParams,
    geometry: &NodalGeometry,
    zeroth: &DirectionVectors,
    diffusion: &[f64],
) -> DirectionVectors {
    moment(params, geometry, zeroth, diffusion, Moment::First)
}

/// The second transverse-leakage moment — `calc_2ndtransleakagexyz.m`.
///
/// See [`moment`] for the formulas and units.
#[must_use]
pub fn second_moment(
    params: &NodalParams,
    geometry: &NodalGeometry,
    zeroth: &DirectionVectors,
    diffusion: &[f64],
) -> DirectionVectors {
    moment(params, geometry, zeroth, diffusion, Moment::Second)
}

/// The shared body of the two moment routines.
///
/// For direction `d` the transverse source is the sum of the *other* two
/// directions' zeroth leakage moments: `S_x = L_y + L_z`, `S_y = L_x + L_z`,
/// `S_z = L_x + L_y`.
///
/// With `t+ = L(i+1)/L(i)` and `t- = L(i-1)/L(i)` the neighbour width ratios
/// \[dimensionless\] and `h = 2*(t+ + 1)*(t- + 1)*(t- + t+ + 1)`, the interior
/// value is
///
/// ```text
/// First:  [ (t-+1)(2t-+1)(S(i+1) - S(i)) + (t++1)(2t++1)(S(i) - S(i-1)) ] / h
/// Second: [ (t-+1)        (S(i+1) - S(i)) + (t++1)        (S(i-1) - S(i)) ] / h
/// ```
///
/// scaled in both cases by `0.25 * L(i)^2 / D(i)` \[cm²/cm = cm\].
///
/// On an outer face, with `h = 4*(t+1)*(t+2)`:
///
/// | | `Vacuum` / `ZeroFlux` | `Reflective` |
/// |---|---|---|
/// | First, low face | `(S(i+1) - S(i)) / (t+ + 1)` | `6*(S(i+1) - S(i)) / h` |
/// | First, high face | `(S(i) - S(i-1)) / (t- + 1)` | `6*(S(i) - S(i-1)) / h` |
/// | Second, low face | left at zero | `2*(S(i+1) - S(i)) / h` |
/// | Second, high face | left at zero | `2*(S(i-1) - S(i)) / h` |
///
/// Nodes with a zero diffusion coefficient are skipped and stay zero.
///
/// # Recorded asymmetry
///
/// The second moment's interior numerator uses `S(i-1) - S(i)` where the first
/// uses `S(i) - S(i-1)`, and its high-face reflective term likewise flips.
/// That is a genuine even/odd distinction, not a sign slip, and is reproduced
/// verbatim.
///
/// # Panics
///
/// If `diffusion.len()` differs from the neutronics state length, or if a
/// line's `low` and `high` coincide, which makes the boundary blocks index a
/// neighbour outside the grid. The MATLAB fails in the same place.
#[must_use]
pub fn moment(
    params: &NodalParams,
    geometry: &NodalGeometry,
    zeroth: &DirectionVectors,
    diffusion: &[f64],
    which: Moment,
) -> DirectionVectors {
    let grid = params.grid;
    let philen = params.philen();
    assert_eq!(diffusion.len(), philen, "diffusion length");

    let mut out = DirectionVectors::zeros(philen);

    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let source: Vec<f64> = match axis {
            Axis::X => (0..philen).map(|i| zeroth.y[i] + zeroth.z[i]).collect(),
            Axis::Y => (0..philen).map(|i| zeroth.x[i] + zeroth.z[i]).collect(),
            Axis::Z => (0..philen).map(|i| zeroth.x[i] + zeroth.y[i]).collect(),
        };
        let width = geometry.width_state_vector(axis, grid);
        let stride = axis.stride(grid);
        let (k1_len, k2_len) = axis.line_counts(grid);
        let range = geometry.range(axis);
        let result = out.axis_mut(axis);

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
                        let (ip, im) = (i + stride, i - stride);
                        let tp = width[ip] / width[i];
                        let tm = width[im] / width[i];
                        let h = 2.0 * (tp + 1.0) * (tm + 1.0) * (tm + tp + 1.0);
                        let ll = match which {
                            Moment::First => {
                                ((tm + 1.0) * (2.0 * tm + 1.0) * (source[ip] - source[i])
                                    + (tp + 1.0) * (2.0 * tp + 1.0) * (source[i] - source[im]))
                                    / h
                            }
                            Moment::Second => {
                                ((tm + 1.0) * (source[ip] - source[i])
                                    + (tp + 1.0) * (source[im] - source[i]))
                                    / h
                            }
                        };
                        result[i] = ll * 0.25 * width[i] * width[i] / diffusion[i];
                    }
                }

                // low outer face
                let (ix, iy, iz) = axis.coords(k1, k2, low);
                for g in 0..grid.ngroups {
                    let i = grid.index(g, ix, iy, iz);
                    if diffusion[i] == 0.0 {
                        continue;
                    }
                    let (nx, ny, nz) = axis.coords(k1, k2, low + 1);
                    let ip = grid.index(g, nx, ny, nz);
                    let tplus = width[ip] / width[i];
                    let h = 4.0 * (tplus + 1.0) * (tplus + 2.0);
                    let value = match (which, geometry.boundaries.low(axis)) {
                        (
                            Moment::First,
                            BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux,
                        ) => (source[ip] - source[i]) / (tplus + 1.0),
                        (Moment::First, BoundaryCondition::Reflective) => {
                            6.0 * (source[ip] - source[i]) / h
                        }
                        (
                            Moment::Second,
                            BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux,
                        ) => continue,
                        (Moment::Second, BoundaryCondition::Reflective) => {
                            2.0 * (source[ip] - source[i]) / h
                        }
                    };
                    result[i] = value * 0.25 * width[i] * width[i] / diffusion[i];
                }

                // high outer face
                let (ix, iy, iz) = axis.coords(k1, k2, high);
                for g in 0..grid.ngroups {
                    let i = grid.index(g, ix, iy, iz);
                    if diffusion[i] == 0.0 {
                        continue;
                    }
                    let (px, py, pz) = axis.coords(k1, k2, high - 1);
                    let im = grid.index(g, px, py, pz);
                    let tminus = width[im] / width[i];
                    let h = 4.0 * (tminus + 1.0) * (tminus + 2.0);
                    let value = match (which, geometry.boundaries.high(axis)) {
                        (
                            Moment::First,
                            BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux,
                        ) => (source[i] - source[im]) / (tminus + 1.0),
                        (Moment::First, BoundaryCondition::Reflective) => {
                            6.0 * (source[i] - source[im]) / h
                        }
                        (
                            Moment::Second,
                            BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux,
                        ) => continue,
                        (Moment::Second, BoundaryCondition::Reflective) => {
                            2.0 * (source[im] - source[i]) / h
                        }
                    };
                    result[i] = value * 0.25 * width[i] * width[i] / diffusion[i];
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::grid::Grid;
    use crate::reference::nodal::cross_sections::diffusion_coefficients;
    use crate::reference::nodal::geometry::BoundaryConditions;

    fn block(bc: BoundaryCondition) -> (NodalParams, NodalGeometry, Vec<f64>) {
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
            BoundaryConditions::uniform(bc),
        );
        let d = diffusion_coefficients(grid, &[vec![1.0 / 3.0]], &ws, 1.0);
        (params, geometry, d)
    }

    /// Zeroth-moment leakage rising linearly with `iz` in x and y, so the
    /// z-direction transverse source `S_z = L_x + L_y` is linear in `iz`.
    fn axially_linear_zeroth(params: &NodalParams) -> DirectionVectors {
        let grid = params.grid;
        let mut v = DirectionVectors::zeros(grid.state_len());
        for ix in 0..grid.nx {
            for iy in 0..grid.ny {
                for iz in 0..grid.nz {
                    let i = grid.index(0, ix, iy, iz);
                    v.x[i] = iz as f64;
                    v.y[i] = iz as f64;
                }
            }
        }
        v
    }

    #[test]
    fn a_uniform_mesh_reduces_the_interior_first_moment_to_a_central_difference() {
        // Equal widths: t+ = t- = 1, h = 2*2*2*3 = 24, and both numerator
        // weights are (2)(3) = 6, so the result is (S(i+1) - S(i-1))/4.
        let (params, geometry, d) = block(BoundaryCondition::Reflective);
        let grid = params.grid;
        let zeroth = axially_linear_zeroth(&params);
        let m = first_moment(&params, &geometry, &zeroth, &d);
        let i = grid.index(0, 0, 0, 1);
        // S_z = 2*iz, so S(i+1) - S(i-1) = 4, giving LL = 1.
        // Scale by 0.25*100/1 = 25.
        assert!((m.z[i] - 25.0).abs() < 1e-12, "got {}", m.z[i]);
    }

    #[test]
    fn a_linear_source_has_no_second_moment() {
        // Second moment of a parabola fitted to three collinear points is zero.
        let (params, geometry, d) = block(BoundaryCondition::Reflective);
        let grid = params.grid;
        let zeroth = axially_linear_zeroth(&params);
        let m = second_moment(&params, &geometry, &zeroth, &d);
        let i = grid.index(0, 0, 0, 1);
        assert!(m.z[i].abs() < 1e-12, "got {}", m.z[i]);
    }

    #[test]
    fn a_quadratic_source_has_a_nonzero_second_moment() {
        let (params, geometry, d) = block(BoundaryCondition::Reflective);
        let grid = params.grid;
        let mut zeroth = DirectionVectors::zeros(grid.state_len());
        for ix in 0..grid.nx {
            for iy in 0..grid.ny {
                for iz in 0..grid.nz {
                    let i = grid.index(0, ix, iy, iz);
                    zeroth.x[i] = (iz * iz) as f64;
                }
            }
        }
        let m = second_moment(&params, &geometry, &zeroth, &d);
        let i = grid.index(0, 0, 0, 1);
        // S_z = iz^2 -> 0, 1, 4. LL = (2*(4-1) + 2*(0-1))/24 = 4/24 = 1/6.
        // Scaled by 25 -> 25/6.
        assert!((m.z[i] - 25.0 / 6.0).abs() < 1e-12, "got {}", m.z[i]);
    }

    #[test]
    fn a_vacuum_outer_face_leaves_the_second_moment_at_zero() {
        let (params, geometry, d) = block(BoundaryCondition::Vacuum);
        let grid = params.grid;
        let zeroth = axially_linear_zeroth(&params);
        let m = second_moment(&params, &geometry, &zeroth, &d);
        assert_eq!(m.z[grid.index(0, 0, 0, 0)], 0.0);
        assert_eq!(m.z[grid.index(0, 0, 0, 2)], 0.0);
        // The first moment does fire there.
        let f = first_moment(&params, &geometry, &zeroth, &d);
        assert!(f.z[grid.index(0, 0, 0, 0)] != 0.0);
    }

    #[test]
    fn out_of_core_nodes_stay_zero() {
        let grid = Grid::new(2, 2, 3, 1).expect("valid grid");
        let params = NodalParams::with_matlab_defaults(grid);
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
            BoundaryConditions::uniform(BoundaryCondition::Reflective),
        );
        let d = diffusion_coefficients(grid, &[vec![1.0 / 3.0]], &ws, 1.0);
        let zeroth = axially_linear_zeroth(&params);
        let m = first_moment(&params, &geometry, &zeroth, &d);
        assert_eq!(m.z[grid.index(0, 0, 0, 0)], 0.0);
        assert_eq!(m.z[grid.index(0, 0, 0, 2)], 0.0);
    }
}
