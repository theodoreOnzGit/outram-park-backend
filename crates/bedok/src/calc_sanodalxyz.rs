//! The semi-analytic nodal correction operator and its face terms.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_sanodalxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::calc_a1234_expansionxyz::calc_a1234_expansionxyz;
use crate::calc_abefghxyz::Coeffs;
use crate::calc_bucklingxyz::BucklingCache;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{Array2, Array4, SparseMatrix};
use crate::types::{Geometry, Params, Sigma};

/// The nodal correction operator and the face terms it was built from.
#[derive(Clone, Debug, Default)]
pub struct SaNodal {
    /// `nodal` — the correction operator, `philen` square. Assembled only;
    /// nothing in the reference solves against it.
    pub operator: SparseMatrix,
    /// `nodalterms` — `philen` by 6, `(minus, plus)` per axis: columns `0, 1`
    /// for `x`, `2, 3` for `y`, `4, 5` for `z`.
    pub terms: Array2<f64>,
}

/// `[nodal, nodalterms] = calc_sanodalxyz(params, geometry, phivec, sigma, diffvalues, gradterms, nodaltermsold, keff)`.
///
/// Runs the full expansion, converts it into per-face nodal corrections, and
/// assembles those into a correction operator that sits alongside the
/// finite-difference `gradD`.
///
/// # Arguments
///
/// - `coeffs` — `Hh` and `Gg` here, plus the rest passed through to
///   [`calc_a1234_expansionxyz`]. **The reference reads these from
///   `geometry.nodalcoeffs`**; passed explicitly, as in the other expansion
///   modules.
/// - `diffvalues` — the **4-D** `(ix, iy, iz, g)` array. This is the **third**
///   consumer of that shape, alongside
///   [`crate::calc_abefghxyz::calc_abefghxyz`] and
///   [`crate::makegrad_dxyz::makegrad_dxyz`]; it flattens internally before
///   calling the expansion.
/// - `gradterms` — face coefficients from
///   [`crate::makegrad_dxyz::makegrad_dxyz`].
/// - `nodaltermsold` — the previous iteration's `nodalterms`, fed to the
///   transverse-leakage chain.
/// - `buck_cache` — carried across calls.
///
/// # The ill-conditioning guard
///
/// The reference computes `phi_eps = 1e-8 * max(abs(phivec))` and skips any
/// nodal correction whose denominator is smaller than that, leaving the term at
/// zero. Its own comment explains why: near-zero or sign-cancelling flux makes
/// the expansion ill-conditioned, and the fallback is a **pure
/// finite-difference** correction of zero. `max(abs(phivec)) == 0` substitutes
/// `1`.
///
/// This is the reference's own defensive addition, not part of the underlying
/// method, and it is preserved as written.
///
/// # Two different interior ranges — do not conflate them
///
/// The face-term loop runs `low ..= high-1` (each **face**, owned by the node
/// on its low side), while the assembly loop runs `low+1 ..= high-1` (each
/// strictly **interior node**, since the boundary nodes get their own blocks).
/// The reference uses `zlow:zhi-1` in one and `zlow+1:zhi-1` in the other; that
/// difference is real and is preserved.
///
/// # The neighbour copy is unconditional
///
/// In the face loop:
///
/// ```text
/// if abs(denom_z) > phi_eps
///     nodalterms(idx,6)=...
/// end
/// nodalterms(idxplus,5)=nodalterms(idx,6);
/// ```
///
/// The copy sits **outside** the guard, so when the guard suppresses the
/// update, the neighbour still receives whatever `nodalterms(idx,6)` already
/// held — zero on the first pass. Preserved.
///
/// # The void test always reads group 1
///
/// Every skip test is `diffvalues(..., 1)` — group 1 — regardless of the `g`
/// being processed. A node void in group 1 but not in others would be skipped
/// for all groups. `calcdiffvalues3d` fills all groups of a node together, so
/// the case does not arise from it.
///
/// # Reference defect — a fuelled node outside the bounds crashes here
///
/// The `z` pass **creates** each node's diagonal triplet slot and records it in
/// `counteridx`; the `y` and `x` passes then accumulate into
/// `nodalele(counteridx(idx))`. If `z` never created a slot, `counteridx(idx)`
/// is `0` and MATLAB raises `Index must be a positive integer`.
///
/// `z` skips a node when it is void **or when it falls outside
/// `[zlow, zhi]`** — and the latter is reachable, because
/// [`crate::geometry_ends3d`] finds only the first contiguous run per grid
/// line, so material after an internal axial gap is fuelled yet out of bounds.
///
/// This is the same root cause as the defect in
/// [`crate::makegrad_dxyz::makegrad_dxyz`], with the opposite symptom: there it
/// silently leaves a spurious `+1` on the diagonal, here it aborts. Translated
/// as written — the panic below carries the same meaning as MATLAB's index
/// error — and pinned by a test.
///
/// # Panics
///
/// If a `y` or `x` pass reaches a node the `z` pass did not create a slot for
/// (see above), or if the triplet count exceeds `philen*10`, reproducing
/// `error('Error in calc_sanodal')`.
#[allow(clippy::too_many_arguments)]
pub fn calc_sanodalxyz(
    params: &Params,
    geometry: &Geometry,
    coeffs: &Coeffs,
    phivec: &[f64],
    sigma: &mut Sigma,
    diffvalues: &Array4<f64>,
    gradterms: &Array2<f64>,
    nodaltermsold: &Array2<f64>,
    keff: f64,
    buck_cache: &mut BucklingCache,
) -> SaNodal {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    let coords = |node: usize| (node / xstep, (node % xstep) / maxiz, node % maxiz);
    // Every void test in the reference reads group 1, whatever `g` is.
    let occupied = |node: usize| {
        let (ix, iy, iz) = coords(node);
        diffvalues.get(ix, iy, iz, 0) != 0.0
    };

    // Flatten to the standard `g*es + ix*xstep + iy*maxiz + iz` ordering.
    let mut diffvalues_d = vec![0.0; philen];
    for g in 0..g_count {
        for node in 0..es {
            let (ix, iy, iz) = coords(node);
            diffvalues_d[g * es + node] = diffvalues.get(ix, iy, iz, g);
        }
    }

    let e = calc_a1234_expansionxyz(
        params,
        geometry,
        coeffs,
        phivec,
        sigma,
        &diffvalues_d,
        gradterms,
        nodaltermsold,
        keff,
        buck_cache,
    );

    // `dummyplus` and `dummy.*first`; note the sign flips on 3*A2 and Gg*A4.
    let dummy_plus = |w: &[f64],
                      a1: &[f64],
                      a2: &[f64],
                      a3: &[f64],
                      a4: &[f64],
                      hh: &[f64],
                      gg: &[f64]|
     -> Vec<f64> {
        (0..philen)
            .map(|i| {
                2.0 * diffvalues_d[i] / w[i % es]
                    * (a1[i] + 3.0 * a2[i] + hh[i] * a3[i] + gg[i] * a4[i])
            })
            .collect()
    };
    let dummy_first = |w: &[f64],
                       a1f: &[f64],
                       a2: &[f64],
                       a3f: &[f64],
                       a4: &[f64],
                       hh: &[f64],
                       gg: &[f64]|
     -> Vec<f64> {
        (0..philen)
            .map(|i| {
                2.0 * diffvalues_d[i] / w[i % es]
                    * (a1f[i] - 3.0 * a2[i] + hh[i] * a3f[i] - gg[i] * a4[i])
            })
            .collect()
    };

    let dplus_x = dummy_plus(
        &geometry.lx,
        &e.a1.x,
        &e.a2.x,
        &e.a3.x,
        &e.a4.x,
        &coeffs.x.hh,
        &coeffs.x.gg,
    );
    let dplus_y = dummy_plus(
        &geometry.ly,
        &e.a1.y,
        &e.a2.y,
        &e.a3.y,
        &e.a4.y,
        &coeffs.y.hh,
        &coeffs.y.gg,
    );
    let dplus_z = dummy_plus(
        &geometry.lz,
        &e.a1.z,
        &e.a2.z,
        &e.a3.z,
        &e.a4.z,
        &coeffs.z.hh,
        &coeffs.z.gg,
    );
    let dfirst_x = dummy_first(
        &geometry.lx,
        &e.a1.xfirst,
        &e.a2.x,
        &e.a3.xfirst,
        &e.a4.x,
        &coeffs.x.hh,
        &coeffs.x.gg,
    );
    let dfirst_y = dummy_first(
        &geometry.ly,
        &e.a1.yfirst,
        &e.a2.y,
        &e.a3.yfirst,
        &e.a4.y,
        &coeffs.y.hh,
        &coeffs.y.gg,
    );
    let dfirst_z = dummy_first(
        &geometry.lz,
        &e.a1.zfirst,
        &e.a2.z,
        &e.a3.zfirst,
        &e.a4.z,
        &coeffs.z.hh,
        &coeffs.z.gg,
    );

    let phi_scale = phivec.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let phi_scale = if phi_scale == 0.0 { 1.0 } else { phi_scale };
    let phi_eps = 1e-8 * phi_scale;

    let bound = |a: &Option<Array2<usize>>, i: usize, j: usize, fallback: usize| -> usize {
        match a {
            Some(m) => m.get(i, j),
            None => fallback,
        }
    };

    // (low, high) node index per grid line.
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

    // ---- face terms ------------------------------------------------------
    let mut terms = Array2::<f64>::zeros(philen, 6);

    let face_terms = |lines: &[(usize, usize)],
                      stride: usize,
                      cm: usize,
                      cp: usize,
                      dplus: &[f64],
                      dfirst: &[f64],
                      terms: &mut Array2<f64>| {
        for &(low, high) in lines {
            // low face
            if occupied(low) {
                for g in 0..g_count {
                    let idx = g * es + low;
                    if phivec[idx].abs() > phi_eps {
                        terms.set(idx, cm, dfirst[idx] / phivec[idx] - gradterms.get(idx, cm));
                    }
                }
            }
            // interior faces: `low ..= high-1`, each owned by its low node
            let mut node = low;
            while node < high {
                if occupied(node) {
                    for g in 0..g_count {
                        let idx = g * es + node;
                        let ip = idx + stride;
                        let denom = phivec[idx] + phivec[ip];
                        if denom.abs() > phi_eps {
                            terms.set(
                                idx,
                                cp,
                                (gradterms.get(idx, cp) * (phivec[idx] - phivec[ip]) + dplus[idx])
                                    / denom,
                            );
                        }
                        // Outside the guard in the reference.
                        terms.set(ip, cm, terms.get(idx, cp));
                    }
                }
                node += stride;
            }
            // high face
            if occupied(high) {
                for g in 0..g_count {
                    let idx = g * es + high;
                    if phivec[idx].abs() > phi_eps {
                        terms.set(idx, cp, dplus[idx] / phivec[idx] + gradterms.get(idx, cp));
                    }
                }
            }
        }
    };

    face_terms(&z_lines, 1, 4, 5, &dplus_z, &dfirst_z, &mut terms);
    face_terms(&y_lines, maxiz, 2, 3, &dplus_y, &dfirst_y, &mut terms);
    face_terms(&x_lines, xstep, 0, 1, &dplus_x, &dfirst_x, &mut terms);

    // ---- operator assembly ----------------------------------------------
    let mut row: Vec<usize> = Vec::new();
    let mut col: Vec<usize> = Vec::new();
    let mut ele: Vec<f64> = Vec::new();
    // `counteridx` — which triplet slot holds each node's diagonal.
    let mut diag_slot: Vec<Option<usize>> = vec![None; philen];

    let assemble = |lines: &[(usize, usize)],
                    stride: usize,
                    cm: usize,
                    cp: usize,
                    widths: &[f64],
                    creates: bool,
                    row: &mut Vec<usize>,
                    col: &mut Vec<usize>,
                    ele: &mut Vec<f64>,
                    diag_slot: &mut Vec<Option<usize>>,
                    terms: &Array2<f64>| {
        let diagonal = |idx: usize,
                        v: f64,
                        ele: &mut Vec<f64>,
                        row: &mut Vec<usize>,
                        col: &mut Vec<usize>,
                        diag_slot: &mut Vec<Option<usize>>| {
            if creates {
                row.push(idx);
                col.push(idx);
                ele.push(v);
                diag_slot[idx] = Some(ele.len() - 1);
            } else {
                match diag_slot[idx] {
                    Some(slot) => ele[slot] += v,
                    None => panic!(
                        "Index must be a positive integer: node {idx} has no diagonal slot — \
                         the z pass skipped it (void, or outside its line bounds) while a \
                         later direction reached it"
                    ),
                }
            }
        };

        for &(low, high) in lines {
            // strictly interior nodes: `low+1 ..= high-1`
            let mut node = low + stride;
            while node < high {
                if occupied(node) {
                    for g in 0..g_count {
                        let idx = g * es + node;
                        diagonal(
                            idx,
                            (terms.get(idx, cm) - terms.get(idx, cp)) / widths[idx % es],
                            ele,
                            row,
                            col,
                            diag_slot,
                        );
                        row.push(idx);
                        col.push(idx + stride);
                        ele.push(-terms.get(idx, cp) / widths[(idx + stride) % es]);
                        row.push(idx);
                        col.push(idx - stride);
                        ele.push(terms.get(idx, cm) / widths[(idx - stride) % es]);
                    }
                }
                node += stride;
            }
            // low face: diagonal plus only the forward neighbour
            if occupied(low) {
                for g in 0..g_count {
                    let idx = g * es + low;
                    diagonal(
                        idx,
                        (terms.get(idx, cm) - terms.get(idx, cp)) / widths[idx % es],
                        ele,
                        row,
                        col,
                        diag_slot,
                    );
                    row.push(idx);
                    col.push(idx + stride);
                    ele.push(-terms.get(idx, cp) / widths[(idx + stride) % es]);
                }
            }
            // high face: diagonal plus only the backward neighbour
            if occupied(high) {
                for g in 0..g_count {
                    let idx = g * es + high;
                    diagonal(
                        idx,
                        (terms.get(idx, cm) - terms.get(idx, cp)) / widths[idx % es],
                        ele,
                        row,
                        col,
                        diag_slot,
                    );
                    row.push(idx);
                    col.push(idx - stride);
                    ele.push(terms.get(idx, cm) / widths[(idx - stride) % es]);
                }
            }
        }
    };

    // z creates the diagonal slots; y and x accumulate into them.
    assemble(
        &z_lines,
        1,
        4,
        5,
        &geometry.lz,
        true,
        &mut row,
        &mut col,
        &mut ele,
        &mut diag_slot,
        &terms,
    );
    assemble(
        &y_lines,
        maxiz,
        2,
        3,
        &geometry.ly,
        false,
        &mut row,
        &mut col,
        &mut ele,
        &mut diag_slot,
        &terms,
    );
    assemble(
        &x_lines,
        xstep,
        0,
        1,
        &geometry.lx,
        false,
        &mut row,
        &mut col,
        &mut ele,
        &mut diag_slot,
        &terms,
    );

    assert!(row.len() <= philen * 10, "Error in calc_sanodal");

    SaNodal {
        operator: SparseMatrix::assemble(&row, &col, &ele, philen, philen),
        terms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc_abefghxyz::AxisCoeffs;

    const ES: usize = 12;

    fn coeffs(v: f64) -> Coeffs {
        let a = || AxisCoeffs {
            aa: vec![v; ES],
            bb: vec![v; ES],
            ee: vec![v; ES],
            ff: vec![v; ES],
            gg: vec![v; ES],
            hh: vec![v; ES],
        };
        Coeffs {
            x: a(),
            y: a(),
            z: a(),
        }
    }

    fn setup() -> (Params, Geometry, Sigma, Array4<f64>, Array2<f64>) {
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
        let idx: Vec<usize> = (0..ES).collect();
        let sigma = Sigma {
            tot: SparseMatrix::assemble(&idx, &idx, &[1.0; ES], ES, ES),
            s: SparseMatrix::assemble(&idx, &idx, &[0.2; ES], ES, ES),
            f: SparseMatrix::assemble(&idx, &idx, &[0.3; ES], ES, ES),
            ..Default::default()
        };
        let mut diffd = Array4::<f64>::zeros(2, 2, 3, 1);
        for ix in 0..2 {
            for iy in 0..2 {
                for iz in 0..3 {
                    diffd.set(ix, iy, iz, 0, 1.0);
                }
            }
        }
        (params, geometry, sigma, diffd, Array2::<f64>::zeros(ES, 6))
    }

    /// The chain runs end to end and produces a finite operator and terms.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// `terms` is `philen` by 6 and finite; the operator is `philen` square.
    #[test]
    fn the_nodal_chain_produces_a_finite_operator() {
        let (params, geometry, mut sigma, diffd, terms) = setup();
        let c = coeffs(0.5);
        let mut cache = BucklingCache::new();
        let mut r = calc_sanodalxyz(
            &params, &geometry, &c, &[1.0; ES], &mut sigma, &diffd, &terms, &terms, 1.0, &mut cache,
        );
        assert_eq!(r.operator.rows(), ES);
        assert!(r.terms.as_slice().iter().all(|v| v.is_finite()));
        assert!(r.operator.find().iter().all(|t| t.v.is_finite()));
    }

    /// An all-zero flux drives `phi_scale` to its substituted `1`, and every
    /// guard then suppresses its update, leaving all face terms at zero.
    ///
    /// # Methodology
    ///
    /// With `phivec` all zero, `max(abs(phivec)) == 0` so the reference
    /// substitutes `phi_scale = 1` and `phi_eps = 1e-8`. Every `|phi|` and
    /// every `denom` is then `0`, below the threshold, so no term is written.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// All six columns are zero at every index — the documented pure
    /// finite-difference fallback.
    #[test]
    fn a_zero_flux_falls_back_to_no_nodal_correction() {
        let (params, geometry, mut sigma, diffd, terms) = setup();
        let c = coeffs(0.5);
        let mut cache = BucklingCache::new();
        let r = calc_sanodalxyz(
            &params, &geometry, &c, &[0.0; ES], &mut sigma, &diffd, &terms, &terms, 1.0, &mut cache,
        );
        assert!(r.terms.as_slice().iter().all(|v| *v == 0.0));
    }

    /// Pins the reference defect: a fuelled node the `z` pass never reached
    /// aborts when `y` or `x` tries to accumulate into its missing diagonal
    /// slot. MATLAB raises `Index must be a positive integer` at the same
    /// point.
    ///
    /// # Methodology
    ///
    /// Every node is fuelled, but the `z` bounds for the `(0, 0)` line are set
    /// to `[0, 1]`, excluding node 2 — exactly what `geometry_ends3d` produces
    /// for a line with an internal axial gap. Node 2 is still within the `y`
    /// and `x` bounds, so a later pass reaches it.
    #[test]
    #[should_panic(expected = "no diagonal slot")]
    fn a_fuelled_node_the_z_pass_missed_aborts_the_assembly() {
        let (params, mut geometry, mut sigma, diffd, terms) = setup();
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

        let c = coeffs(0.5);
        let mut cache = BucklingCache::new();
        let _ = calc_sanodalxyz(
            &params, &geometry, &c, &[1.0; ES], &mut sigma, &diffd, &terms, &terms, 1.0, &mut cache,
        );
    }
}
