//! The `A1` coefficient of the semi-analytic nodal expansion.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_a1_expansionxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use std::collections::HashMap;

use crate::calc_abefghxyz::Coeffs;
use crate::calc_bucklingxyz::Buckling;
use crate::calc_transleakagexyz::Leakage;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{solve_dense, Array2};
use crate::types::{AxisField, BoundaryCondition, Geometry, Params};

/// The `A1` expansion coefficients.
///
/// Six vectors, not three. The `*first` variants are computed at the **low**
/// boundary face of each grid line and are consumed separately by
/// `calc_a1234_expansionxyz` to build `A3.xfirst`/`yfirst`/`zfirst`.
#[derive(Clone, Debug, Default)]
pub struct A1 {
    /// `A1.x` — interior faces and the high-`x` boundary.
    pub x: Vec<f64>,
    /// `A1.y`.
    pub y: Vec<f64>,
    /// `A1.z`.
    pub z: Vec<f64>,
    /// `A1.xfirst` — the low-`x` boundary face only.
    pub xfirst: Vec<f64>,
    /// `A1.yfirst`.
    pub yfirst: Vec<f64>,
    /// `A1.zfirst`.
    pub zfirst: Vec<f64>,
}

/// Collapse a block-diagonal operator to its dense per-node group blocks.
///
/// The buckling operators couple energy groups only **at the same spatial
/// node**, so row `idx` has non-zeros only in the `G` columns
/// `g2 * es + (idx % es)`. This gathers those into a `philen`-by-`G` dense
/// array, so `Buck.d(idx, idxvec)` becomes a row read.
///
/// The reference does the same and says why: it "replaces ~2e5 expensive
/// sparse-matrix slice extractions per call with cheap dense row reads (the
/// dominant cost in this function)". Here it matters for a second reason —
/// a triplet-scan lookup per access would be far worse than MATLAB's sparse
/// indexing.
///
/// # Arguments
///
/// - `m` — the operator, `philen` square and block-diagonal in groups.
/// - `philen` — `G * es`.
/// - `es` — nodes per group.
/// - `groups` — `G`.
///
/// # Returns
///
/// A `philen`-by-`groups` array; entry `(idx, g2)` is the coupling from group
/// `g2` into `idx`, at `idx`'s node. Structural zeros read back as `0`.
pub fn buckling_blocks(
    m: &mut crate::matlab::SparseMatrix,
    philen: usize,
    es: usize,
    groups: usize,
) -> Array2<f64> {
    let lookup: HashMap<(usize, usize), f64> =
        m.find().into_iter().map(|t| ((t.i, t.j), t.v)).collect();
    let mut out = Array2::<f64>::zeros(philen, groups);
    for idx in 0..philen {
        let node = idx % es;
        for g2 in 0..groups {
            let v = lookup.get(&(idx, g2 * es + node)).copied().unwrap_or(0.0);
            out.set(idx, g2, v);
        }
    }
    out
}

/// One grid line along the axis being solved.
struct Line {
    /// Node indices owning an interior face — the reference's
    /// `low .. high-1`, filtered to nodes with a non-zero diffusion
    /// coefficient. Each face couples this node to the one a stride away.
    faces: Vec<usize>,
    /// Node index of the first fuelled node on the line.
    low: usize,
    /// Node index of the last fuelled node on the line.
    high: usize,
}

/// `A1 = calc_a1_expansionxyz(params, geometry, phivec, A2, A4, Leakone, diffvalues, Buck)`.
///
/// Solves for the first expansion coefficient on each axis, by imposing current
/// and flux continuity across every node face.
///
/// # Two kinds of system
///
/// - **Interior faces** — a `2G`-by-`2G` solve per face, coupling the node on
///   each side. The top `G` rows impose current continuity, the bottom `G`
///   impose flux continuity weighted by the assembly discontinuity factors.
///   Only the first `G` components of the solution are kept; they belong to the
///   node on the low side.
/// - **Boundary faces** — a `G`-by-`G` solve per face, with a different
///   right-hand side per boundary condition.
///
/// The reference batches the interior solves with `pagemldivide`; here they are
/// a loop over independent small systems, which is the same arithmetic.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `geometry` — per-line bounds, face boundary conditions, and `adf`.
/// - `coeffs` — the `Aa`/`Ff`/`Gg`/`Hh` coefficients from
///   [`crate::calc_abefghxyz::calc_abefghxyz`]. **The reference reads these
///   from `geometry.nodalcoeffs`**; passing them explicitly keeps
///   [`crate::types`] from having to depend on a translated module. Behaviour
///   is unchanged.
/// - `phivec` — the flux, `philen` long.
/// - `a2`, `a4` — the second and fourth expansion coefficients.
/// - `leakone` — first-moment transverse leakages from
///   [`crate::calc_1sttransleakagexyz::calc_1sttransleakagexyz`].
/// - `diffvalues` — **flat `philen` vector**, as elsewhere in this chain.
/// - `buck` — the buckling operators from
///   [`crate::calc_bucklingxyz::calc_bucklingxyz`].
///
/// # `Buck` is block-diagonal, and that is exploited
///
/// Energy groups couple only at the same spatial node, so `Buck.d` is
/// block-diagonal and `Buck.d(idx, idxvec)` — the `G` group entries at `idx`'s
/// node — is just one dense row. The reference pre-extracts those into
/// `philen`-by-`G` arrays, noting it "replaces ~2e5 expensive sparse-matrix
/// slice extractions per call with cheap dense row reads (the dominant cost in
/// this function)". This translation does the same, for the same reason: a
/// triplet-scan lookup per access would be far worse.
///
/// # Reference asymmetry — the high-face `zeroflux` sign
///
/// At a **high** face the two non-reflective branches differ in a way the low
/// face does not mirror:
///
/// ```text
/// vacuum:   btemp = ... - adf(idx,plus)*(A2 + A4 + phivec + Aa*Leakone)
/// zeroflux: btemp =       -adf(idx,plus)*(A2 + A4 + phivec - Aa*Leakone)
/// ```
///
/// The `Aa*Leakone` term flips sign between them. At the low face both the
/// `vacuum` and `zeroflux` branches use `- Aa*Leakone`, so the high-face
/// `zeroflux` line is the odd one out. Verified against the source rather than
/// inferred, and translated as written per the no-silent-repairs rule in
/// `docs/bedok-port-scoping.md` §1.0. Whether it is deliberate or a slip is a
/// physics question this translation does not attempt to settle.
///
/// # `zeroflux` is not grouped with `vacuum` here
///
/// Every other translated module treats them identically
/// (`case {'vacuum','zeroflux'}`). In this file all three boundary conditions
/// have distinct formulas. Do not carry the grouping over from the leakage
/// modules.
///
/// # Singular systems
///
/// A node with `diffvalues == 0` in every group leaves its row block untouched
/// by the per-group loop, so only the unconditional diagonal term survives. If
/// that diagonal is also zero the system is singular and
/// [`crate::matlab::solve_dense`] returns `NaN`, mirroring MATLAB's `mldivide`
/// warning-and-propagate behaviour rather than aborting.
// Nine parameters, against clippy's threshold of seven. The reference takes
// eight; the ninth is `coeffs`, which it reads off `geometry` instead. Bundling
// them into a context struct would depart from the reference's signature for no
// gain in a translation whose value is being diffable against the `.m` file.
#[allow(clippy::too_many_arguments)]
pub fn calc_a1_expansionxyz(
    params: &Params,
    geometry: &Geometry,
    coeffs: &Coeffs,
    phivec: &[f64],
    a2: &AxisField,
    a4: &AxisField,
    leakone: &Leakage,
    diffvalues: &[f64],
    buck: &mut Buckling,
) -> A1 {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    // `adf` defaults to all ones when the field is absent.
    let adf = |idx: usize, col: usize| -> f64 {
        match &geometry.adf {
            Some(a) => a.get(idx, col),
            None => 1.0,
        }
    };

    let buckblk_x = buckling_blocks(&mut buck.x, philen, es, g_count);
    let buckblk_y = buckling_blocks(&mut buck.y, philen, es, g_count);
    let buckblk_z = buckling_blocks(&mut buck.z, philen, es, g_count);

    // `diffvec.d = 2*diffvalues ./ repmat(L_d, G, 1)`.
    let diffvec = |widths: &[f64]| -> Vec<f64> {
        (0..philen)
            .map(|idx| 2.0 * diffvalues[idx] / widths[idx % es])
            .collect()
    };
    let diffvec_x = diffvec(&geometry.lx);
    let diffvec_y = diffvec(&geometry.ly);
    let diffvec_z = diffvec(&geometry.lz);

    let bound = |a: &Option<Array2<usize>>, i: usize, j: usize, fallback: usize| -> usize {
        match a {
            Some(m) => m.get(i, j),
            None => fallback,
        }
    };

    // --- grid lines -------------------------------------------------------
    // Faces run `low ..= high-1`; each owns the node on its low side.
    let mut z_lines = Vec::new();
    for ix in 0..maxix {
        for iy in 0..maxiy {
            let zlow = bound(&geometry.zlows, ix, iy, 0);
            let zhi = bound(&geometry.zhis, ix, iy, maxiz - 1);
            let base = ix * xstep + iy * maxiz;
            let mut faces = Vec::new();
            let mut iz = zlow;
            while iz < zhi {
                let s = base + iz;
                if diffvalues[s] != 0.0 {
                    faces.push(s);
                }
                iz += 1;
            }
            z_lines.push(Line {
                faces,
                low: base + zlow,
                high: base + zhi,
            });
        }
    }

    let mut y_lines = Vec::new();
    for ix in 0..maxix {
        for iz in 0..maxiz {
            let ylow = bound(&geometry.ylows, ix, iz, 0);
            let yhi = bound(&geometry.yhis, ix, iz, maxiy - 1);
            let mut faces = Vec::new();
            let mut iy = ylow;
            while iy < yhi {
                let s = ix * xstep + iy * maxiz + iz;
                if diffvalues[s] != 0.0 {
                    faces.push(s);
                }
                iy += 1;
            }
            y_lines.push(Line {
                faces,
                low: ix * xstep + ylow * maxiz + iz,
                high: ix * xstep + yhi * maxiz + iz,
            });
        }
    }

    let mut x_lines = Vec::new();
    for iy in 0..maxiy {
        for iz in 0..maxiz {
            let xlow = bound(&geometry.xlows, iy, iz, 0);
            let xhi = bound(&geometry.xhis, iy, iz, maxix - 1);
            let mut faces = Vec::new();
            let mut ix = xlow;
            while ix < xhi {
                let s = ix * xstep + iy * maxiz + iz;
                if diffvalues[s] != 0.0 {
                    faces.push(s);
                }
                ix += 1;
            }
            x_lines.push(Line {
                faces,
                low: xlow * xstep + iy * maxiz + iz,
                high: xhi * xstep + iy * maxiz + iz,
            });
        }
    }

    // --- per-axis solve ---------------------------------------------------
    let axis = |lines: &[Line],
                stride: usize,
                col_minus: usize,
                col_plus: usize,
                dvec: &[f64],
                ac: &crate::calc_abefghxyz::AxisCoeffs,
                buckblk: &Array2<f64>,
                a2d: &[f64],
                a4d: &[f64],
                leak: &[f64],
                bc_min: BoundaryCondition,
                bc_max: BoundaryCondition|
     -> (Vec<f64>, Vec<f64>) {
        let mut out = vec![0.0; philen];
        let mut out_first = vec![0.0; philen];

        // The four `bdummy` combinations, evaluated per index as needed.
        let bdummy = |i: usize| dvec[i] * (3.0 * a2d[i] + ac.gg[i] * a4d[i] + ac.ff[i] * leak[i]);
        let bdummyplus =
            |i: usize| dvec[i] * (3.0 * a2d[i] + ac.gg[i] * a4d[i] - ac.ff[i] * leak[i]);
        let bdummy2 =
            |i: usize| adf(i, col_plus) * (a2d[i] + a4d[i] + phivec[i] + ac.aa[i] * leak[i]);
        let bdummyplus2 =
            |i: usize| adf(i, col_minus) * (a2d[i] + a4d[i] + phivec[i] - ac.aa[i] * leak[i]);

        // Interior faces: one 2G x 2G solve each.
        let n = 2 * g_count;
        for line in lines {
            for &s in line.faces.iter() {
                let mut a = vec![0.0; n * n];
                let mut b = vec![0.0; n];

                for g in 0..g_count {
                    let ig = g * es + s;
                    let ipg = ig + stride;

                    let dl = dvec[ig];
                    let fl = ac.ff[ig];
                    let al = ac.aa[ig];
                    let a_plus = adf(ig, col_plus);
                    let dh = dvec[ipg];
                    let fh = ac.ff[ipg];
                    let ah = ac.aa[ipg];
                    let a_minus = adf(ipg, col_minus);

                    for g2 in 0..g_count {
                        let de = if g == g2 { 1.0 } else { 0.0 };
                        a[g * n + g2] = -dl * fl * buckblk.get(ig, g2) - de * dl;
                        a[g * n + g_count + g2] = dh * fh * buckblk.get(ipg, g2) + de * dh;
                        a[(g_count + g) * n + g2] = a_plus * al * buckblk.get(ig, g2) + de * a_plus;
                        a[(g_count + g) * n + g_count + g2] =
                            a_minus * ah * buckblk.get(ipg, g2) + de * a_minus;
                    }
                    b[g] = bdummy(ig) + bdummyplus(ipg);
                    b[g_count + g] = bdummyplus2(ipg) - bdummy2(ig);
                }

                let sol = solve_dense(&a, &b, n);
                for g in 0..g_count {
                    out[g * es + s] = sol[g];
                }
            }
        }

        // Boundary faces: one G x G solve each.
        let boundary = |node: usize, bc: BoundaryCondition, is_low: bool| -> Vec<f64> {
            let col = if is_low { col_minus } else { col_plus };
            let mut a = vec![0.0; g_count * g_count];
            let mut b = vec![0.0; g_count];

            for g in 0..g_count {
                let idx = g * es + node;
                if diffvalues[idx] == 0.0 {
                    continue;
                }
                let af = adf(idx, col);
                let (aa, ff, gg, hh) = (ac.aa[idx], ac.ff[idx], ac.gg[idx], ac.hh[idx]);
                let d = dvec[idx];

                match (bc, is_low) {
                    (BoundaryCondition::Vacuum, true) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = -af * aa * buckblk.get(idx, g2)
                                - 2.0 * d * aa * buckblk.get(idx, g2) * hh;
                        }
                        b[g] = 2.0 * d * (aa * leak[idx] * hh - 3.0 * a2d[idx] - gg * a4d[idx]);
                        b[g] -= af * (a2d[idx] + a4d[idx] + phivec[idx] - aa * leak[idx]);
                    }
                    (BoundaryCondition::Reflective, true) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = d * ff * buckblk.get(idx, g2);
                        }
                        b[g] = bdummyplus(idx);
                    }
                    (BoundaryCondition::ZeroFlux, true) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = af * aa * buckblk.get(idx, g2);
                        }
                        b[g] = af * (a2d[idx] + a4d[idx] + phivec[idx] - aa * leak[idx]);
                    }
                    (BoundaryCondition::Vacuum, false) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = af * aa * buckblk.get(idx, g2)
                                + 2.0 * d * aa * buckblk.get(idx, g2) * hh;
                        }
                        b[g] = -2.0 * d * (aa * leak[idx] * hh + 3.0 * a2d[idx] + gg * a4d[idx]);
                        b[g] -= af * (a2d[idx] + a4d[idx] + phivec[idx] + aa * leak[idx]);
                    }
                    (BoundaryCondition::Reflective, false) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = -d * ff * buckblk.get(idx, g2);
                        }
                        b[g] = bdummy(idx);
                    }
                    (BoundaryCondition::ZeroFlux, false) => {
                        for g2 in 0..g_count {
                            a[g * g_count + g2] = af * aa * buckblk.get(idx, g2);
                        }
                        // NOTE the `-` on `aa*leak`, where the high-face vacuum
                        // branch above uses `+`. See the module doc comment.
                        b[g] = -af * (a2d[idx] + a4d[idx] + phivec[idx] - aa * leak[idx]);
                    }
                }
            }

            // The diagonal term is added after the loop, unconditionally —
            // including for groups the loop skipped.
            for g in 0..g_count {
                let idx = g * es + node;
                let af = adf(idx, col);
                match (bc, is_low) {
                    (BoundaryCondition::Vacuum, true) => {
                        a[g * g_count + g] -= 2.0 * dvec[idx] + af;
                    }
                    (BoundaryCondition::Reflective, true) => {
                        a[g * g_count + g] += dvec[idx];
                    }
                    (BoundaryCondition::ZeroFlux, true) => {
                        a[g * g_count + g] += af;
                    }
                    (BoundaryCondition::Vacuum, false) => {
                        a[g * g_count + g] += 2.0 * dvec[idx] + af;
                    }
                    (BoundaryCondition::Reflective, false) => {
                        a[g * g_count + g] -= dvec[idx];
                    }
                    (BoundaryCondition::ZeroFlux, false) => {
                        a[g * g_count + g] += af;
                    }
                }
            }

            solve_dense(&a, &b, g_count)
        };

        for line in lines {
            let sol = boundary(line.low, bc_min, true);
            for g in 0..g_count {
                out_first[g * es + line.low] = sol[g];
            }
            let sol = boundary(line.high, bc_max, false);
            for g in 0..g_count {
                out[g * es + line.high] = sol[g];
            }
        }

        (out, out_first)
    };

    let (z, zfirst) = axis(
        &z_lines,
        1,
        4,
        5,
        &diffvec_z,
        &coeffs.z,
        &buckblk_z,
        &a2.z,
        &a4.z,
        &leakone.z,
        geometry.zmin,
        geometry.zmax,
    );
    let (y, yfirst) = axis(
        &y_lines,
        maxiz,
        2,
        3,
        &diffvec_y,
        &coeffs.y,
        &buckblk_y,
        &a2.y,
        &a4.y,
        &leakone.y,
        geometry.ymin,
        geometry.ymax,
    );
    let (x, xfirst) = axis(
        &x_lines,
        xstep,
        0,
        1,
        &diffvec_x,
        &coeffs.x,
        &buckblk_x,
        &a2.x,
        &a4.x,
        &leakone.x,
        geometry.xmin,
        geometry.xmax,
    );

    A1 {
        x,
        y,
        z,
        xfirst,
        yfirst,
        zfirst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc_abefghxyz::AxisCoeffs;
    use crate::matlab::SparseMatrix;

    const ES: usize = 12;

    /// A 2x2x3 grid, one group, with simple non-degenerate coefficients.
    fn setup() -> (
        Params,
        Geometry,
        Coeffs,
        Vec<f64>,
        AxisField,
        AxisField,
        Leakage,
        Vec<f64>,
        Buckling,
    ) {
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

        let ones = |v: f64| AxisCoeffs {
            aa: vec![v; ES],
            bb: vec![v; ES],
            ee: vec![v; ES],
            ff: vec![v; ES],
            gg: vec![v; ES],
            hh: vec![v; ES],
        };
        let coeffs = Coeffs {
            x: ones(0.5),
            y: ones(0.5),
            z: ones(0.5),
        };

        let field = |v: f64| AxisField {
            x: vec![v; ES],
            y: vec![v; ES],
            z: vec![v; ES],
        };
        let leak = Leakage {
            x: vec![0.1; ES],
            y: vec![0.1; ES],
            z: vec![0.1; ES],
        };

        // A diagonal buckling operator, so BuckBlk is a single column of 0.25.
        let idx: Vec<usize> = (0..ES).collect();
        let diag = SparseMatrix::assemble(&idx, &idx, &[0.25; ES], ES, ES);
        let buck = Buckling {
            x: diag.clone(),
            y: diag.clone(),
            z: diag,
        };

        (
            params,
            geometry,
            coeffs,
            vec![1.0; ES],
            field(0.2),
            field(0.05),
            leak,
            vec![1.0; ES],
            buck,
        )
    }

    /// The block extraction must recover the diagonal of a diagonal operator.
    ///
    /// # Methodology
    ///
    /// With `Buck.z = 0.25 * I` and one group, `BuckBlk[idx][0]` should be
    /// `0.25` at every index. Checked indirectly: with all-zero `Buck` the
    /// `A1.z` result must differ from the `0.25 * I` case, since `BuckBlk`
    /// enters every matrix row.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// The two differ, confirming `BuckBlk` is read rather than ignored.
    #[test]
    fn buckling_blocks_reach_the_assembled_system() {
        let (params, geometry, coeffs, phivec, a2, a4, leak, dv, mut buck) = setup();
        let with = calc_a1_expansionxyz(
            &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut buck,
        );

        let (params, geometry, coeffs, phivec, a2, a4, leak, dv, _) = setup();
        let mut zero = Buckling {
            x: SparseMatrix::zeros(ES, ES),
            y: SparseMatrix::zeros(ES, ES),
            z: SparseMatrix::zeros(ES, ES),
        };
        let without = calc_a1_expansionxyz(
            &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut zero,
        );

        assert_ne!(with.z, without.z);
    }

    /// `A1` carries six independent vectors; the `*first` variants are written
    /// only at the low boundary node of each line.
    ///
    /// # Methodology
    ///
    /// On the `z` lines of a 2x2x3 grid, node 0 is the low boundary of the
    /// `(0, 0)` line. `zfirst` must be non-zero there and zero at the interior
    /// node 1, which no boundary solve touches.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `zfirst[0]` is populated, `zfirst[1]` is exactly zero.
    #[test]
    fn first_variants_are_written_only_at_low_boundary_nodes() {
        let (params, geometry, coeffs, phivec, a2, a4, leak, dv, mut buck) = setup();
        let a1 = calc_a1_expansionxyz(
            &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut buck,
        );
        assert_ne!(a1.zfirst[0], 0.0);
        assert_eq!(a1.zfirst[1], 0.0);
        assert_eq!(a1.zfirst.len(), ES);
    }

    /// The three boundary conditions must give three different answers here —
    /// unlike the leakage modules, `zeroflux` is not grouped with `vacuum`.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// All three low-face results are distinct.
    #[test]
    fn all_three_boundary_conditions_differ() {
        let solve = |bc: BoundaryCondition| {
            let (params, mut geometry, coeffs, phivec, a2, a4, leak, dv, mut buck) = setup();
            geometry.zmin = bc;
            calc_a1_expansionxyz(
                &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut buck,
            )
            .zfirst[0]
        };

        let vac = solve(BoundaryCondition::Vacuum);
        let refl = solve(BoundaryCondition::Reflective);
        let zf = solve(BoundaryCondition::ZeroFlux);

        assert_ne!(vac, refl);
        assert_ne!(vac, zf);
        assert_ne!(refl, zf);
    }

    /// Absent `adf` behaves as all ones, so supplying an explicit ones matrix
    /// must change nothing.
    #[test]
    fn absent_adf_is_equivalent_to_ones() {
        let (params, geometry, coeffs, phivec, a2, a4, leak, dv, mut buck) = setup();
        let implicit = calc_a1_expansionxyz(
            &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut buck,
        );

        let (params, mut geometry, coeffs, phivec, a2, a4, leak, dv, mut buck) = setup();
        let mut ones = Array2::<f64>::zeros(ES, 6);
        for i in 0..ES {
            for j in 0..6 {
                ones.set(i, j, 1.0);
            }
        }
        geometry.adf = Some(ones);
        let explicit = calc_a1_expansionxyz(
            &params, &geometry, &coeffs, &phivec, &a2, &a4, &leak, &dv, &mut buck,
        );

        assert_eq!(implicit.z, explicit.z);
        assert_eq!(implicit.zfirst, explicit.zfirst);
    }
}
