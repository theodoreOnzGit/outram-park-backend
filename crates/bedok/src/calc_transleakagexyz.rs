//! Transverse leakages — the base leakage operators applied to the flux.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_transleakagexyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, SparseMatrix};
use crate::types::{BoundaryCondition, Geometry, Params};

/// The transverse leakage on each axis, one entry per `(group, node)`.
#[derive(Clone, Debug, Default)]
pub struct Leakage {
    /// `Leakage.x`.
    pub x: Vec<f64>,
    /// `Leakage.y`.
    pub y: Vec<f64>,
    /// `Leakage.z`.
    pub z: Vec<f64>,
}

/// One grid line along the axis being assembled.
///
/// The reference finds these with a triple loop per axis, differing only in
/// which coordinate varies and which bounds array is consulted. Collecting them
/// into a common shape lets the triplet arithmetic — which is genuinely
/// identical across the three axes — be written once.
struct Line {
    /// Node indices (no group offset) strictly interior to the line, and with a
    /// non-zero diffusion coefficient.
    interior: Vec<usize>,
    /// Node index of the first fuelled node on the line.
    low: usize,
    /// Node index of the last fuelled node on the line.
    high: usize,
}

/// `Leakage = calc_transleakagexyz(params, geometry, phivec, diffvalues, gradterms, nodalterms)`.
///
/// Assembles a leakage operator on each axis and applies it to the flux,
/// returning the three transverse leakage vectors the nodal expansion needs.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `geometry` — node widths, the per-line active bounds from
///   [`crate::geometry_ends3d::geometry_ends3d`], and the six face boundary
///   conditions.
/// - `phivec` — the flux, `philen` long.
/// - `diffvalues` — **flat `philen` vector**, not the 4-D array. Same
///   convention as [`crate::calc_bucklingxyz::calc_bucklingxyz`]; see that
///   module for why the two shapes coexist.
/// - `gradterms`, `nodalterms` — `philen` rows by **6** columns. Columns pair
///   up per axis as `(minus, plus)`: `0, 1` for `x`, `2, 3` for `y`, `4, 5` for
///   `z`.
///
/// # Returns
///
/// [`Leakage`] — three `philen` vectors, each `L * phivec` for that axis.
///
/// # Structure
///
/// Per axis and per node, three coefficients:
///
/// - **diagonal** — `(grad_minus + grad_plus + nodal_minus - nodal_plus) / L`
/// - **plus neighbour** — `-(grad_plus + nodal_plus) / L_plus`
/// - **minus neighbour** — `-(grad_minus - nodal_minus) / L_minus`
///
/// At a boundary face the corresponding neighbour term is dropped and the
/// diagonal changes: under [`BoundaryCondition::Reflective`] it keeps only the
/// *inward* pair, while `Vacuum` and `ZeroFlux` keep the full interior form.
///
/// # Node widths are indexed by the global index, and wrap
///
/// The reference `repmat`s the per-node widths to `philen` and then indexes
/// them at `idx +/- stride`, which this translation reproduces as
/// `l[(idx +/- stride) % es]`. For an interior node the neighbour stays within
/// the same node column so the wrap never triggers. At a boundary face it can:
/// `idx + stride` for a node at the top of a group block reads the width of a
/// node in the *next* group block, which for uniform-in-group widths is the
/// same number. Faithful to the reference either way.
///
/// # At least two nodes are needed in every direction
///
/// The stencil cannot be assembled on a direction that is one node thick. Such
/// a node is simultaneously the low and the high face, so the high-face branch
/// asks for a minus neighbour that is off the end of the vector. The reference
/// fails on the same geometry from the other side — its low-face `idxplus`
/// runs past `philen` and `sparse` rejects the subscript.
///
/// This is a property of the discretisation rather than a defect, but it is
/// worth knowing because the failure is an index error rather than a
/// diagnostic.
///
/// # Reference quirk — only the `x` counter is bounds-checked
///
/// The reference preallocates `philen*5` entries per axis but tests only
/// `counterx`, with `error('Error in calc_transleakage')`. The `y` and `z`
/// counters are never checked. Reproduced: the assertion below covers `x`
/// alone.
///
/// # Panics
///
/// If the `x` triplet count exceeds `philen*5`, or if a boundary node's
/// minus-neighbour index would be negative — the latter mirrors MATLAB's
/// index-zero error.
pub fn calc_transleakagexyz(
    params: &Params,
    geometry: &Geometry,
    phivec: &[f64],
    diffvalues: &[f64],
    gradterms: &Array2<f64>,
    nodalterms: &Array2<f64>,
) -> Leakage {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    // `isfield(geometry,'zlows')` and friends — absent bounds default to the
    // full extent.
    let bound = |a: &Option<Array2<usize>>, i: usize, j: usize, fallback: usize| -> usize {
        match a {
            Some(m) => m.get(i, j),
            None => fallback,
        }
    };

    // --- z lines: fixed (ix, iy), varying iz ------------------------------
    let mut z_lines = Vec::new();
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let zlow = bound(&geometry.zlows, ix, iy, 0);
            let zhi = bound(&geometry.zhis, ix, iy, maxiz - 1);
            let base = ix * xstep + iy * maxiz;
            let mut interior = Vec::new();
            let mut iz = zlow + 1;
            while iz < zhi {
                let s = base + iz;
                if diffvalues[s] != 0.0 {
                    interior.push(s);
                }
                iz += 1;
            }
            z_lines.push(Line {
                interior,
                low: base + zlow,
                high: base + zhi,
            });
        }
    }

    // --- y lines: fixed (ix, iz), varying iy ------------------------------
    let mut y_lines = Vec::new();
    for ix in 0..maxix {
        for iz in 0..maxiz {
            let ylow = bound(&geometry.ylows, ix, iz, 0);
            let yhi = bound(&geometry.yhis, ix, iz, maxiy - 1);
            let mut interior = Vec::new();
            let mut iy = ylow + 1;
            while iy < yhi {
                let s = ix * xstep + iy * maxiz + iz;
                if diffvalues[s] != 0.0 {
                    interior.push(s);
                }
                iy += 1;
            }
            y_lines.push(Line {
                interior,
                low: ix * xstep + ylow * maxiz + iz,
                high: ix * xstep + yhi * maxiz + iz,
            });
        }
    }

    // --- x lines: fixed (iy, iz), varying ix ------------------------------
    let mut x_lines = Vec::new();
    for iy in 0..maxiy {
        for iz in 0..maxiz {
            let xlow = bound(&geometry.xlows, iy, iz, 0);
            let xhi = bound(&geometry.xhis, iy, iz, maxix - 1);
            let mut interior = Vec::new();
            let mut ix = xlow + 1;
            while ix < xhi {
                let s = ix * xstep + iy * maxiz + iz;
                if diffvalues[s] != 0.0 {
                    interior.push(s);
                }
                ix += 1;
            }
            x_lines.push(Line {
                interior,
                low: xlow * xstep + iy * maxiz + iz,
                high: xhi * xstep + iy * maxiz + iz,
            });
        }
    }

    let axis = |lines: &[Line],
                stride: usize,
                col_minus: usize,
                col_plus: usize,
                widths: &[f64],
                bc_min: BoundaryCondition,
                bc_max: BoundaryCondition|
     -> (Vec<usize>, Vec<usize>, Vec<f64>) {
        let mut row: Vec<usize> = Vec::new();
        let mut col: Vec<usize> = Vec::new();
        let mut ele: Vec<f64> = Vec::new();

        let width = |i: usize| widths[i % es];
        let minus = |idx: usize| {
            idx.checked_sub(stride)
                .expect("boundary node has no minus neighbour: index would be negative")
        };

        // Interior nodes: diagonal plus both neighbours.
        for line in lines {
            for &s in line.interior.iter() {
                for g in 0..g_count {
                    let idx = g * es + s;
                    let gm = gradterms.get(idx, col_minus);
                    let gp = gradterms.get(idx, col_plus);
                    let nm = nodalterms.get(idx, col_minus);
                    let np = nodalterms.get(idx, col_plus);

                    row.push(idx);
                    col.push(idx);
                    ele.push((gm + gp + nm - np) / width(idx));

                    row.push(idx);
                    col.push(idx + stride);
                    ele.push(-(gp + np) / width(idx + stride));

                    row.push(idx);
                    col.push(minus(idx));
                    ele.push(-(gm - nm) / width(minus(idx)));
                }
            }
        }

        // Low face.
        for line in lines {
            for g in 0..g_count {
                if diffvalues[line.low] == 0.0 {
                    continue;
                }
                let idx = g * es + line.low;
                let gm = gradterms.get(idx, col_minus);
                let gp = gradterms.get(idx, col_plus);
                let nm = nodalterms.get(idx, col_minus);
                let np = nodalterms.get(idx, col_plus);

                row.push(idx);
                col.push(idx);
                ele.push(match bc_min {
                    BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                        (gm + gp + nm - np) / width(idx)
                    }
                    BoundaryCondition::Reflective => (gp - np) / width(idx),
                });

                row.push(idx);
                col.push(idx + stride);
                ele.push(-(gp + np) / width(idx + stride));
            }
        }

        // High face.
        for line in lines {
            for g in 0..g_count {
                if diffvalues[line.high] == 0.0 {
                    continue;
                }
                let idx = g * es + line.high;
                let gm = gradterms.get(idx, col_minus);
                let gp = gradterms.get(idx, col_plus);
                let nm = nodalterms.get(idx, col_minus);
                let np = nodalterms.get(idx, col_plus);

                row.push(idx);
                col.push(idx);
                ele.push(match bc_max {
                    BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                        (gm + gp + nm - np) / width(idx)
                    }
                    BoundaryCondition::Reflective => (gm + nm) / width(idx),
                });

                row.push(idx);
                col.push(minus(idx));
                ele.push(-(gm - nm) / width(minus(idx)));
            }
        }

        (row, col, ele)
    };

    let (zr, zc, ze) = axis(
        &z_lines,
        1,
        4,
        5,
        &geometry.lz,
        geometry.zmin,
        geometry.zmax,
    );
    let (yr, yc, ye) = axis(
        &y_lines,
        maxiz,
        2,
        3,
        &geometry.ly,
        geometry.ymin,
        geometry.ymax,
    );
    let (xr, xc, xe) = axis(
        &x_lines,
        xstep,
        0,
        1,
        &geometry.lx,
        geometry.xmin,
        geometry.xmax,
    );

    // Only the x counter is checked in the reference.
    assert!(xr.len() <= philen * 5, "Error in calc_transleakage");

    let mut lx = SparseMatrix::assemble(&xr, &xc, &xe, philen, philen);
    let mut ly = SparseMatrix::assemble(&yr, &yc, &ye, philen, philen);
    let mut lz = SparseMatrix::assemble(&zr, &zc, &ze, philen, philen);

    Leakage {
        x: lx.mul_vec(phivec),
        y: ly.mul_vec(phivec),
        z: lz.mul_vec(phivec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Node count of the test grid: 2 x 2 x 3, one group, so `philen == ES`.
    const ES: usize = 12;

    /// A 2x2x3 grid, one group.
    ///
    /// Two nodes are needed in `x` and `y` because the stencil cannot be
    /// assembled on a one-node-thick direction — see the function's doc
    /// comment. The `z` direction has three, giving one genuinely interior node
    /// per line.
    ///
    /// `gradterms` and `nodalterms` are filled with distinguishable constants
    /// so a mis-selected column shows up immediately.
    fn setup(
        zmin: BoundaryCondition,
        zmax: BoundaryCondition,
    ) -> (Params, Geometry, Array2<f64>, Array2<f64>) {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(2),
            maxiz: Some(3),
            g: 1,
            ..Default::default()
        };
        let geometry = Geometry {
            lx: vec![1.0; ES],
            ly: vec![1.0; ES],
            lz: vec![2.0; ES],
            zmin,
            zmax,
            ..Default::default()
        };

        // Columns 4 and 5 are the z pair; give them 10 and 100.
        let mut gradterms = Array2::<f64>::zeros(ES, 6);
        let mut nodalterms = Array2::<f64>::zeros(ES, 6);
        for i in 0..ES {
            gradterms.set(i, 4, 10.0);
            gradterms.set(i, 5, 100.0);
            nodalterms.set(i, 4, 1.0);
            nodalterms.set(i, 5, 2.0);
        }
        (params, geometry, gradterms, nodalterms)
    }

    /// The interior node's three coefficients, checked by applying the operator
    /// to a unit flux.
    ///
    /// # Methodology
    ///
    /// With `Lz = 2` everywhere, `grad = (10, 100)` and `nodal = (1, 2)`:
    /// diagonal `(10 + 100 + 1 - 2)/2 = 54.5`, plus `-(100 + 2)/2 = -51`,
    /// minus `-(10 - 1)/2 = -4.5`. Against `phi = 1` everywhere the interior
    /// row sums to `54.5 - 51 - 4.5 = -1`.
    ///
    /// Node 1 is the interior node of the `z` line at `(ix, iy) = (0, 0)`.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Interior entry is exactly `-1`.
    #[test]
    fn interior_row_uses_all_three_coefficients() {
        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);

        let leak = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[1.0; ES],
            &gradterms,
            &nodalterms,
        );
        assert_eq!(leak.z[1], -1.0);
    }

    /// A reflective low face keeps only the inward pair on the diagonal, so it
    /// differs from the vacuum case by exactly the outward terms.
    ///
    /// # Methodology
    ///
    /// Vacuum diagonal is `(10 + 100 + 1 - 2)/2 = 54.5`; reflective is
    /// `(100 - 2)/2 = 49`. With the plus-neighbour term `-51` common to both,
    /// the low-face entry moves from `3.5` to `-2` under a unit flux.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Both values match.
    #[test]
    fn a_reflective_low_face_drops_the_outward_terms() {
        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let vac = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[1.0; ES],
            &gradterms,
            &nodalterms,
        );
        assert_eq!(vac.z[0], 3.5);

        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::Reflective, BoundaryCondition::Vacuum);
        let refl = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[1.0; ES],
            &gradterms,
            &nodalterms,
        );
        assert_eq!(refl.z[0], -2.0);
    }

    /// `zeroflux` and `vacuum` are grouped in every `switch`, so they must give
    /// identical results.
    #[test]
    fn zeroflux_and_vacuum_agree() {
        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let a = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[1.0; ES],
            &gradterms,
            &nodalterms,
        );

        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::ZeroFlux, BoundaryCondition::ZeroFlux);
        let b = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[1.0; ES],
            &gradterms,
            &nodalterms,
        );

        assert_eq!(a.z, b.z);
    }

    /// An all-void grid contributes nothing on any axis.
    #[test]
    fn void_lines_contribute_nothing() {
        let (params, geometry, gradterms, nodalterms) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_transleakagexyz(
            &params,
            &geometry,
            &[1.0; ES],
            &[0.0; ES],
            &gradterms,
            &nodalterms,
        );
        assert_eq!(leak.z, vec![0.0; ES]);
        assert_eq!(leak.x, vec![0.0; ES]);
    }
}
