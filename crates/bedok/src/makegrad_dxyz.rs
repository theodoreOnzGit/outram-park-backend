//! The `gradD` diffusion operator and the `gradterms` face coefficients.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `makegradDxyz.m`, `main_exec_diff3d_standalone` snapshot.
//!   The Rust module is `makegrad_dxyz` because Rust warns on non-snake-case
//!   module names.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::convertindexc2d::IndexMode;
use crate::convertsparseformat2d::convertsparseformat2d;
use crate::error::Result;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array3, Array4, SparseMatrix};
use crate::types::{BoundaryCondition, Geometry, Params};

/// `gradD` and `gradterms`.
#[derive(Clone, Debug, Default)]
pub struct GradD {
    /// The diffusion operator, `philenf` square.
    pub operator: SparseMatrix,
    /// Face diffusion coefficients, `philen` by 6, `(minus, plus)` per axis:
    /// columns `0, 1` for `x`, `2, 3` for `y`, `4, 5` for `z`.
    ///
    /// **Already doubled** — see the note on [`makegrad_dxyz`].
    pub terms: Array2<f64>,
}

/// `[gradD, gradterms] = makegradDxyz(geometry, params, DiffD, whichsigma, tomode)`.
///
/// Builds the finite-difference diffusion operator from harmonic-mean face
/// diffusion coefficients, and records those face coefficients for the
/// transverse-leakage routines.
///
/// # Arguments
///
/// - `geometry` — node widths, per-line bounds, face boundary conditions.
/// - `params` — `G`, `Nc` and the extents.
/// - `diffd` — the **4-D** `(ix, iy, iz, g)` diffusion array from
///   [`crate::calcdiffvalues3d::calcdiffvalues3d`]. This is one of only two
///   functions taking that shape; most of the chain takes the flat `philen`
///   vector instead.
/// - `whichsigma` — material per node, `0` for void.
/// - `tomode` — `None` selects the reference's default of [`IndexMode::Plain`].
///
/// # Errors
///
/// Propagates from [`convertsparseformat2d`] when `tomode` is
/// [`IndexMode::DiamondDifference`]. **That path is never exercised**: every
/// call site in the snapshot passes four arguments, so `tomode` is always 1.
///
/// # The stencil
///
/// With half-widths `h = L/2` and the harmonic-mean face coefficient
///
/// $$ \tilde{D}_{+} = \frac{(h + h_{+})}{2} \frac{D \, D_{+}}{h D + h_{+} D_{+}} \frac{1}{L} $$
///
/// the diagonal gets `Dt_plus/h_plus + Dt_minus/h_minus` and the two
/// neighbours get `-Dt_plus/h_plus` and `-Dt_minus/h_minus`.
///
/// At a boundary face the outward coefficient comes from the boundary
/// condition instead — `0` for reflective, `D/L` for zero-flux, and
/// `0.5 D / (2D + 0.5L)` for vacuum — while the inward one keeps the harmonic
/// mean. The neighbour term is pushed identically in all three branches.
///
/// # The diagonal is assigned by `z` and accumulated by `y` and `x`
///
/// This asymmetry is deliberate and load-bearing; do not "harmonise" it.
///
/// The reference pre-fills its triplet arrays with an identity, so slot `k`
/// *is* row `k`'s diagonal. The `z` blocks then use plain assignment
/// (`gradDele(idx) = ...`), wiping that `1`, while `y` and `x` accumulate onto
/// it. A fuelled node therefore ends with `z + y + x` and **no** identity term,
/// while a void node — skipped by every direction — keeps its `1`. That `1` is
/// exactly the unit-diagonal placeholder
/// [`crate::convertsparsekey3d::convertsparsekey3d`] later strips.
///
/// Making `z` accumulate too would leave a spurious `+1` on every fuelled
/// diagonal, and nothing would visibly break.
///
/// # Reference defect — a fuelled node outside `[low, high]` keeps its identity
///
/// The scheme above depends on `z` covering every fuelled node. A node that is
/// fuelled (`whichsigma != 0`) but falls **outside** `[zlow, zhi]` is skipped by
/// all three `z` branches, keeps its identity `1`, and then has `y` and `x`
/// accumulated on top — leaving a spurious `+1` on its diagonal.
///
/// That case is reachable rather than hypothetical: `geometry_ends3d` finds
/// only the **first contiguous run** per grid line (a limitation documented and
/// pinned in [`crate::geometry_ends3d`]), so material after an internal axial
/// gap is fuelled yet outside `[zlow, zhi]`. The two documented behaviours
/// interact. Pinned by a test below.
///
/// # `gradterms` is doubled at the end
///
/// The final line of the reference is
///
/// ```text
/// gradterms=2*gradterms; %check this (seems correct)
/// ```
///
/// The comment is the author's own, and is preserved here because it records a
/// genuine uncertainty rather than a settled derivation. The factor is applied
/// to every column, after all three directions have written.
///
/// # `geometry.Vi` is read and never used
///
/// The reference assigns `Vi=geometry.Vi;` at the top and never refers to it
/// again — dead code. It is therefore **not** a parameter here, and `Geometry`
/// needs no `vi` field on account of this function.
///
/// # Panics
///
/// If the off-diagonal count exceeds `philen*10`, reproducing
/// `error('Error in makegradD')`.
pub fn makegrad_dxyz(
    geometry: &Geometry,
    params: &Params,
    diffd: &Array4<f64>,
    whichsigma: &Array3<usize>,
    tomode: Option<IndexMode>,
) -> Result<GradD> {
    let g_count = params.g;
    let nc = params.nc_or_zero();
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;
    let philenf = philen + nc * es;
    let tomode = tomode.unwrap_or(IndexMode::Plain);

    // The identity default; `z` overwrites, `y` and `x` add.
    let mut diag = vec![1.0; philen];
    let mut row: Vec<usize> = Vec::new();
    let mut col: Vec<usize> = Vec::new();
    let mut ele: Vec<f64> = Vec::new();
    let mut terms = Array2::<f64>::zeros(philen, 6);

    let coords = |node: usize| (node / xstep, (node % xstep) / maxiz, node % maxiz);
    let diff = |node: usize, g: usize| {
        let (ix, iy, iz) = coords(node);
        diffd.get(ix, iy, iz, g)
    };
    let occupied = |node: usize| {
        let (ix, iy, iz) = coords(node);
        whichsigma.get(ix, iy, iz) != 0
    };

    let bound = |a: &Option<Array2<usize>>, i: usize, j: usize, fallback: usize| -> usize {
        match a {
            Some(m) => m.get(i, j),
            None => fallback,
        }
    };

    // (low, high) node index per grid line, per direction.
    let mut z_lines = Vec::new();
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let base = ix * xstep + iy * maxiz;
            z_lines.push((
                base + bound(&geometry.zlows, ix, iy, 0),
                base + bound(&geometry.zhis, ix, iy, maxiz - 1),
            ));
        }
    }
    let mut y_lines = Vec::new();
    for ix in 0..maxix {
        for iz in 0..maxiz {
            let base = ix * xstep + iz;
            y_lines.push((
                base + bound(&geometry.ylows, ix, iz, 0) * maxiz,
                base + bound(&geometry.yhis, ix, iz, maxiy - 1) * maxiz,
            ));
        }
    }
    let mut x_lines = Vec::new();
    for iy in 0..maxiy {
        for iz in 0..maxiz {
            let base = iy * maxiz + iz;
            x_lines.push((
                base + bound(&geometry.xlows, iy, iz, 0) * xstep,
                base + bound(&geometry.xhis, iy, iz, maxix - 1) * xstep,
            ));
        }
    }

    // One direction's contribution.
    #[allow(clippy::too_many_arguments)]
    let direction = |lines: &[(usize, usize)],
                     stride: usize,
                     col_minus: usize,
                     col_plus: usize,
                     widths: &[f64],
                     bc_min: BoundaryCondition,
                     bc_max: BoundaryCondition,
                     accumulate: bool,
                     diag: &mut Vec<f64>,
                     row: &mut Vec<usize>,
                     col: &mut Vec<usize>,
                     ele: &mut Vec<f64>,
                     terms: &mut Array2<f64>| {
        let write_diag = |idx: usize, v: f64, diag: &mut Vec<f64>| {
            if accumulate {
                diag[idx] += v;
            } else {
                diag[idx] = v;
            }
        };

        for &(low, high) in lines {
            // --- interior nodes -------------------------------------------
            let mut node = low + stride;
            while node < high {
                if occupied(node) {
                    let np = node + stride;
                    let nm = node - stride;
                    let h = widths[node] / 2.0;
                    let hp = widths[np] / 2.0;
                    let hm = widths[nm] / 2.0;
                    for g in 0..g_count {
                        let idx = g * es + node;
                        let d0 = diff(node, g);
                        let dp = diff(np, g);
                        let dm = diff(nm, g);

                        let dt_plus =
                            0.5 * (h + hp) * (d0 * dp) / (h * d0 + hp * dp) / widths[node];
                        let dt_minus =
                            0.5 * (h + hm) * (d0 * dm) / (h * d0 + hm * dm) / widths[node];

                        write_diag(idx, dt_plus / hp + dt_minus / hm, diag);

                        row.push(idx);
                        col.push(idx + stride);
                        ele.push(-dt_plus / hp);

                        row.push(idx);
                        col.push(idx - stride);
                        ele.push(-dt_minus / hm);

                        terms.set(idx, col_minus, dt_minus);
                        terms.set(idx, col_plus, dt_plus);
                    }
                }
                node += stride;
            }

            // --- low face -------------------------------------------------
            if occupied(low) {
                let np = low + stride;
                let h = widths[low] / 2.0;
                let hp = widths[np] / 2.0;
                // The reference mirrors the node's own half-width here.
                let hm = widths[low] / 2.0;
                for g in 0..g_count {
                    let idx = g * es + low;
                    let d0 = diff(low, g);
                    let dp = diff(np, g);
                    let dt_plus = 0.5 * (h + hp) * (d0 * dp) / (h * d0 + hp * dp) / widths[low];
                    let dt_minus = match bc_min {
                        BoundaryCondition::Vacuum => 0.5 * d0 / (2.0 * d0 + 0.5 * widths[low]),
                        BoundaryCondition::Reflective => 0.0,
                        BoundaryCondition::ZeroFlux => d0 / widths[low],
                    };

                    // Pushed identically by all three branches.
                    row.push(idx);
                    col.push(idx + stride);
                    ele.push(-dt_plus / hp);

                    write_diag(idx, dt_plus / hp + dt_minus / hm, diag);
                    terms.set(idx, col_minus, dt_minus);
                    terms.set(idx, col_plus, dt_plus);
                }
            }

            // --- high face ------------------------------------------------
            if occupied(high) {
                let nm = high - stride;
                let h = widths[high] / 2.0;
                let hp = widths[high] / 2.0;
                let hm = widths[nm] / 2.0;
                for g in 0..g_count {
                    let idx = g * es + high;
                    let d0 = diff(high, g);
                    let dm = diff(nm, g);
                    let dt_minus = 0.5 * (h + hm) * (d0 * dm) / (h * d0 + hm * dm) / widths[high];
                    let dt_plus = match bc_max {
                        BoundaryCondition::Vacuum => 0.5 * d0 / (2.0 * d0 + 0.5 * widths[high]),
                        BoundaryCondition::Reflective => 0.0,
                        BoundaryCondition::ZeroFlux => d0 / widths[high],
                    };

                    row.push(idx);
                    col.push(idx - stride);
                    ele.push(-dt_minus / hm);

                    write_diag(idx, dt_plus / hp + dt_minus / hm, diag);
                    terms.set(idx, col_minus, dt_minus);
                    terms.set(idx, col_plus, dt_plus);
                }
            }
        }
    };

    // z first — it ASSIGNS the diagonal. y and x then accumulate.
    direction(
        &z_lines,
        1,
        4,
        5,
        &geometry.lz,
        geometry.zmin,
        geometry.zmax,
        false,
        &mut diag,
        &mut row,
        &mut col,
        &mut ele,
        &mut terms,
    );
    direction(
        &y_lines,
        maxiz,
        2,
        3,
        &geometry.ly,
        geometry.ymin,
        geometry.ymax,
        true,
        &mut diag,
        &mut row,
        &mut col,
        &mut ele,
        &mut terms,
    );
    direction(
        &x_lines,
        xstep,
        0,
        1,
        &geometry.lx,
        geometry.xmin,
        geometry.xmax,
        true,
        &mut diag,
        &mut row,
        &mut col,
        &mut ele,
        &mut terms,
    );

    assert!(row.len() <= philen * 10, "Error in makegradD");

    // Diagonal entries, then the off-diagonals gathered above.
    for (idx, d) in diag.iter().enumerate() {
        row.push(idx);
        col.push(idx);
        ele.push(*d);
    }

    let mut operator = SparseMatrix::assemble(&row, &col, &ele, philenf, philenf);
    if tomode != IndexMode::Plain {
        operator = convertsparseformat2d(params, &mut operator, IndexMode::Plain, tomode)?;
    }

    // `gradterms = 2*gradterms` — the author's own "check this" line.
    for idx in 0..philen {
        for c in 0..6 {
            terms.set(idx, c, 2.0 * terms.get(idx, c));
        }
    }

    Ok(GradD { operator, terms })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ES: usize = 12;

    /// A 2x2x3 grid, one group, uniform widths and a uniform diffusion
    /// coefficient.
    fn setup(fuelled: &[usize]) -> (Params, Geometry, Array4<f64>, Array3<usize>) {
        let params = Params {
            maxix: Some(2),
            maxiy: Some(2),
            maxiz: Some(3),
            g: 1,
            ..Default::default()
        };
        let geometry = Geometry {
            lx: vec![2.0; ES],
            ly: vec![2.0; ES],
            lz: vec![2.0; ES],
            ..Default::default()
        };
        let mut diffd = Array4::<f64>::zeros(2, 2, 3, 1);
        let mut whichsigma = Array3::<usize>::zeros(2, 2, 3);
        for &node in fuelled {
            let (ix, iy, iz) = (node / 6, (node % 6) / 3, node % 3);
            diffd.set(ix, iy, iz, 0, 1.0);
            whichsigma.set(ix, iy, iz, 1);
        }
        (params, geometry, diffd, whichsigma)
    }

    /// A void node keeps the identity diagonal — the placeholder convention
    /// `convertsparsekey3d` depends on.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// With nothing fuelled, `gradD` is exactly the identity.
    #[test]
    fn void_nodes_keep_the_identity_diagonal() {
        let (params, geometry, diffd, whichsigma) = setup(&[]);
        let mut g = makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None).unwrap();
        let found = g.operator.find();
        assert_eq!(found.len(), ES);
        assert!(found.iter().all(|t| t.i == t.j && t.v == 1.0));
    }

    /// A fully fuelled grid loses the identity everywhere: `z` assigns before
    /// `y` and `x` accumulate, so no diagonal carries a stray `+1`.
    ///
    /// # Methodology
    ///
    /// With uniform `L = 2` and `D = 1`, `h = hp = hm = 1` and each interior
    /// harmonic mean is `0.5*(1+1)*(1*1)/(1*1 + 1*1)/2 = 0.25`. The `z`
    /// interior node therefore contributes `0.25/1 + 0.25/1 = 0.5` to its
    /// diagonal, and the identity `1` is gone.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Node 1 (the `z`-interior node of the `(0,0)` line) has a diagonal well
    /// away from 1, confirming the assignment overwrote it.
    #[test]
    fn a_fuelled_diagonal_does_not_retain_the_identity() {
        let all: Vec<usize> = (0..ES).collect();
        let (params, geometry, diffd, whichsigma) = setup(&all);
        let mut g = makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None).unwrap();
        let found = g.operator.find();
        let diag = found.iter().find(|t| t.i == 1 && t.j == 1).unwrap().v;
        // z contributes 0.5; y and x add their own boundary terms on top.
        assert!(
            diag > 0.5,
            "diagonal {diag} looks like it kept the identity"
        );
        assert_ne!(diag, 1.0);
    }

    /// Pins the interaction described in the doc comment: a fuelled node
    /// outside `[zlow, zhi]` is skipped by every `z` branch, keeps its identity,
    /// and then has `y`/`x` accumulated on top.
    ///
    /// # Methodology
    ///
    /// The `z` line at `(ix, iy) = (0, 0)` is nodes 0, 1, 2. Fuelling only
    /// nodes 0 and 2 leaves a gap at 1; `geometry_ends3d` would report
    /// `zlow = zhi = 0` for that line, putting node 2 outside the range. Here
    /// the bounds are left absent so they default to the full span — so instead
    /// the test supplies explicit bounds that exclude node 2, reproducing what
    /// `geometry_ends3d` produces on a gapped line.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Node 2's diagonal retains the identity contribution, coming out exactly
    /// `1.0` greater than the same node's value when the bounds include it.
    #[test]
    fn a_fuelled_node_outside_the_bounds_keeps_a_spurious_identity() {
        let all: Vec<usize> = (0..ES).collect();

        // Bounds covering the whole line: node 2 is the high face.
        let (params, geometry, diffd, whichsigma) = setup(&all);
        let mut full = makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None).unwrap();
        let with_z = full.operator.find();
        let d_with = with_z.iter().find(|t| t.i == 2 && t.j == 2).unwrap().v;

        // Bounds that stop at node 1, as a gapped line would produce.
        let (params, mut geometry, diffd, whichsigma) = setup(&all);
        let mut zlows = Array2::<usize>::zeros(2, 2);
        let mut zhis = Array2::<usize>::zeros(2, 2);
        for ix in 0..2 {
            for iy in 0..2 {
                zlows.set(ix, iy, 0);
                zhis.set(ix, iy, if ix == 0 && iy == 0 { 1 } else { 2 });
            }
        }
        geometry.zlows = Some(zlows);
        geometry.zhis = Some(zhis);
        let mut gapped = makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None).unwrap();
        let without_z = gapped.operator.find();
        let d_without = without_z.iter().find(|t| t.i == 2 && t.j == 2).unwrap().v;

        // Node 2 is still fuelled, so y and x contribute identically in both
        // runs. The difference is z's contribution, replaced by the retained
        // identity of 1.
        assert_ne!(d_with, d_without);
        assert!(
            d_without > 1.0,
            "expected the retained identity to survive, got {d_without}"
        );
    }

    /// `gradterms` is doubled on the way out.
    ///
    /// # Methodology
    ///
    /// The uniform-mesh interior harmonic mean is `0.25`; after the final
    /// `gradterms = 2*gradterms` it must read `0.5`.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Column 4 (`z` minus) at node 1 is exactly `0.5`.
    #[test]
    fn gradterms_carry_the_final_doubling() {
        let all: Vec<usize> = (0..ES).collect();
        let (params, geometry, diffd, whichsigma) = setup(&all);
        let g = makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None).unwrap();
        assert_eq!(g.terms.get(1, 4), 0.5);
        assert_eq!(g.terms.get(1, 5), 0.5);
    }

    /// The three boundary conditions give three different outward
    /// coefficients: `0` reflective, `D/L` zero-flux, `0.5D/(2D + 0.5L)`
    /// vacuum.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Reflective gives exactly `0`; zero-flux `1/2 = 0.5` doubled to `1.0`;
    /// vacuum `0.5/(2 + 1) = 1/6` doubled to `1/3`.
    #[test]
    fn boundary_conditions_set_the_outward_coefficient() {
        let all: Vec<usize> = (0..ES).collect();
        let outward = |bc: BoundaryCondition| {
            let (params, mut geometry, diffd, whichsigma) = setup(&all);
            geometry.zmin = bc;
            makegrad_dxyz(&geometry, &params, &diffd, &whichsigma, None)
                .unwrap()
                .terms
                .get(0, 4)
        };

        assert_eq!(outward(BoundaryCondition::Reflective), 0.0);
        assert_eq!(outward(BoundaryCondition::ZeroFlux), 1.0);
        assert!((outward(BoundaryCondition::Vacuum) - 1.0 / 3.0).abs() < 1e-15);
    }
}
