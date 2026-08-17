//! First-moment transverse leakages — the linear term of the quadratic
//! transverse-leakage fit.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_1sttransleakagexyz.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::calc_transleakagexyz::Leakage;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::Array2;
use crate::types::{BoundaryCondition, Geometry, Params};

/// One grid line along the axis being fitted.
///
/// The reference rebuilds this line-finding loop in each of
/// `calc_transleakagexyz.m`, `calc_1sttransleakagexyz.m` and
/// `calc_2ndtransleakagexyz.m` — three near-identical copies. The translation
/// keeps a copy per module rather than sharing one, matching the reference's
/// own structure; the loops differ subtly between files (see the `diffvalues`
/// note on [`calc_1sttransleakagexyz`]) and merging them would hide that.
struct Line {
    /// Node indices (no group offset) strictly interior to the line, with a
    /// non-zero diffusion coefficient.
    interior: Vec<usize>,
    /// Node index of the first fuelled node on the line.
    low: usize,
    /// Node index of the last fuelled node on the line.
    high: usize,
}

/// `Leakage = calc_1sttransleakagexyz(params, geometry, Leakzero, diffvalues)`.
///
/// Fits the **first moment** of the transverse leakage on each axis from the
/// zeroth-moment leakages of the other two.
///
/// # The transverse coupling
///
/// The source on each axis is the sum of the leakages on the **other two**:
///
/// ```text
/// Ssource.x = Leakzero.y + Leakzero.z
/// Ssource.y = Leakzero.x + Leakzero.z
/// Ssource.z = Leakzero.x + Leakzero.y
/// ```
///
/// That is what makes the leakage *transverse* — the 1-D nodal equation along
/// `x` is driven by what leaks out through the `y` and `z` faces.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `geometry` — node widths, per-line active bounds, and the six face
///   boundary conditions.
/// - `leakzero` — the zeroth-moment leakages from
///   [`crate::calc_transleakagexyz::calc_transleakagexyz`].
/// - `diffvalues` — **flat `philen` vector**, as elsewhere in this chain.
///
/// # Returns
///
/// [`Leakage`] — three `philen` vectors of first-moment coefficients. Entries
/// for nodes outside the core stay zero.
///
/// # Interior stencil
///
/// With mesh ratios `tp = L_plus / L` and `tm = L_minus / L`:
///
/// ```text
/// h  = 2 (tp + 1)(tm + 1)(tm + tp + 1)
/// LL = [ (tm+1)(2tm+1)(S_plus - S) + (tp+1)(2tp+1)(S - S_minus) ] / h
/// ```
///
/// then scaled by `0.25 * L^2 / D`. On a uniform mesh `tp = tm = 1` and this
/// collapses to the centred difference `(S_plus - S_minus) / 4`, scaled the
/// same way.
///
/// # Boundary faces
///
/// One-sided, with `h = 4 (t + 1)(t + 2)`:
///
/// - `Vacuum` / `ZeroFlux` — `(S_plus - S) / (t + 1)`
/// - `Reflective` — `6 (S_plus - S) / h`
///
/// and the mirror image at the high face. Both are then scaled by
/// `0.25 * L^2 / D` exactly as the interior is.
///
/// # The `diffvalues` test differs from `calc_transleakagexyz`
///
/// Worth knowing when comparing the two files. At a boundary face this
/// reference tests `diffvalues(idx)` **with** the group offset, whereas
/// `calc_transleakagexyz.m` tests the bare node index — group 1 only. The
/// interior selection is the bare node index in both.
///
/// For cross sections that make every group of a node void together — which is
/// what `calcdiffvalues3d` produces — the two tests agree. They would diverge
/// only for a node void in some groups and not others. Translated as written in
/// each file rather than harmonised.
///
/// # Panics
///
/// If a boundary node's neighbour index falls outside the vector — the same
/// two-node-minimum constraint documented on
/// [`crate::calc_transleakagexyz::calc_transleakagexyz`].
pub fn calc_1sttransleakagexyz(
    params: &Params,
    geometry: &Geometry,
    leakzero: &Leakage,
    diffvalues: &[f64],
) -> Leakage {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    // The transverse coupling: each axis is driven by the other two.
    let add = |a: &[f64], b: &[f64]| -> Vec<f64> {
        a.iter().zip(b.iter()).map(|(p, q)| p + q).collect()
    };
    let ssource_x = add(&leakzero.y, &leakzero.z);
    let ssource_y = add(&leakzero.x, &leakzero.z);
    let ssource_z = add(&leakzero.x, &leakzero.y);

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
                widths: &[f64],
                ssource: &[f64],
                bc_min: BoundaryCondition,
                bc_max: BoundaryCondition|
     -> Vec<f64> {
        let mut out = vec![0.0; philen];
        let width = |i: usize| widths[i % es];
        let minus = |idx: usize| {
            idx.checked_sub(stride)
                .expect("boundary node has no minus neighbour: index would be negative")
        };

        // Interior: the three-point stencil.
        for line in lines {
            for &s in line.interior.iter() {
                for g in 0..g_count {
                    let idx = g * es + s;
                    let ip = idx + stride;
                    let im = minus(idx);

                    let tp = width(ip) / width(idx);
                    let tm = width(im) / width(idx);
                    let h = 2.0 * (tp + 1.0) * (tm + 1.0) * (tm + tp + 1.0);
                    let ll = ((tm + 1.0) * (2.0 * tm + 1.0) * (ssource[ip] - ssource[idx])
                        + (tp + 1.0) * (2.0 * tp + 1.0) * (ssource[idx] - ssource[im]))
                        / h;

                    out[idx] = ll * 0.25 * width(idx).powi(2) / diffvalues[idx];
                }
            }
        }

        // Low face.
        for line in lines {
            for g in 0..g_count {
                let idx = g * es + line.low;
                // Note: with the group offset, unlike `calc_transleakagexyz`.
                if diffvalues[idx] == 0.0 {
                    continue;
                }
                let idxplus = idx + stride;

                let tplus = width(idxplus) / width(idx);
                let h = 4.0 * (tplus + 1.0) * (tplus + 2.0);

                let value = match bc_min {
                    BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                        (ssource[idxplus] - ssource[idx]) / (tplus + 1.0)
                    }
                    BoundaryCondition::Reflective => {
                        6.0 * (ssource[idxplus] - ssource[idx]) / h
                    }
                };
                out[idx] = value * 0.25 * width(idx).powi(2) / diffvalues[idx];
            }
        }

        // High face.
        for line in lines {
            for g in 0..g_count {
                let idx = g * es + line.high;
                if diffvalues[idx] == 0.0 {
                    continue;
                }
                let idxminus = minus(idx);

                let tminus = width(idxminus) / width(idx);
                let h = 4.0 * (tminus + 1.0) * (tminus + 2.0);

                let value = match bc_max {
                    BoundaryCondition::Vacuum | BoundaryCondition::ZeroFlux => {
                        (ssource[idx] - ssource[idxminus]) / (tminus + 1.0)
                    }
                    BoundaryCondition::Reflective => {
                        6.0 * (ssource[idx] - ssource[idxminus]) / h
                    }
                };
                out[idx] = value * 0.25 * width(idx).powi(2) / diffvalues[idx];
            }
        }

        out
    };

    Leakage {
        z: axis(
            &z_lines,
            1,
            &geometry.lz,
            &ssource_z,
            geometry.zmin,
            geometry.zmax,
        ),
        y: axis(
            &y_lines,
            maxiz,
            &geometry.ly,
            &ssource_y,
            geometry.ymin,
            geometry.ymax,
        ),
        x: axis(
            &x_lines,
            xstep,
            &geometry.lx,
            &ssource_x,
            geometry.xmin,
            geometry.xmax,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Node count of the test grid: 2 x 2 x 3, one group.
    const ES: usize = 12;

    /// A 2x2x3 grid with uniform node widths `Lz = 2` and `D = 1`.
    ///
    /// `Leakzero.y` is set to the node index and the other two axes to zero, so
    /// `Ssource.z = Leakzero.x + Leakzero.y` is the node index — a ramp with a
    /// known centred difference.
    fn setup(
        zmin: BoundaryCondition,
        zmax: BoundaryCondition,
    ) -> (Params, Geometry, Leakage, Vec<f64>) {
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
            zmin,
            zmax,
            ..Default::default()
        };
        let leakzero = Leakage {
            x: vec![0.0; ES],
            y: (0..ES).map(|i| i as f64).collect(),
            z: vec![0.0; ES],
        };
        (params, geometry, leakzero, vec![1.0; ES])
    }

    /// The interior stencil on a uniform mesh reduces to a centred difference.
    ///
    /// # Methodology
    ///
    /// With `tp = tm = 1`: `h = 2*2*2*3 = 24` and
    /// `LL = (2*3*(S_p - S) + 2*3*(S - S_m))/24 = (S_p - S_m)/4`. The scale
    /// factor `0.25 * L^2 / D` is `0.25 * 4 / 1 = 1`.
    ///
    /// Node 1 is the interior node of the `z` line at `(0, 0)`, where
    /// `S_p = 2` and `S_m = 0`, so the result is `(2 - 0)/4 = 0.5`.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Exactly `0.5`.
    #[test]
    fn interior_stencil_is_a_centred_difference_on_a_uniform_mesh() {
        let (params, geometry, leakzero, diffvalues) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_1sttransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z[1], 0.5);
    }

    /// Both boundary faces are one-sided differences under a vacuum condition.
    ///
    /// # Methodology
    ///
    /// `tplus = tminus = 1`, so the low face is `(S(1) - S(0))/2 = 0.5` and the
    /// high face `(S(2) - S(1))/2 = 0.5`, each scaled by 1.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Both exactly `0.5`.
    #[test]
    fn vacuum_faces_are_one_sided_differences() {
        let (params, geometry, leakzero, diffvalues) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_1sttransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z[0], 0.5);
        assert_eq!(leak.z[2], 0.5);
    }

    /// A reflective face uses the `6/h` form instead.
    ///
    /// # Methodology
    ///
    /// `h = 4*(1+1)*(1+2) = 24`, so the low face is
    /// `6*(S(1) - S(0))/24 = 0.25`, scaled by 1. The high face is left vacuum
    /// and must be unchanged at `0.5`.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Low face `0.25`, high face `0.5`.
    #[test]
    fn a_reflective_face_uses_the_six_over_h_form() {
        let (params, geometry, leakzero, diffvalues) =
            setup(BoundaryCondition::Reflective, BoundaryCondition::Vacuum);
        let leak = calc_1sttransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z[0], 0.25);
        assert_eq!(leak.z[2], 0.5);
    }

    /// The `z` axis is driven by `x + y`, so a source carried purely on `z`
    /// contributes nothing to the `z` result — the transverse coupling.
    #[test]
    fn an_axis_is_not_driven_by_its_own_leakage() {
        let (params, geometry, _, diffvalues) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leakzero = Leakage {
            x: vec![0.0; ES],
            y: vec![0.0; ES],
            z: (0..ES).map(|i| i as f64).collect(),
        };
        let leak = calc_1sttransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z, vec![0.0; ES]);
        // But x and y both see it.
        assert_ne!(leak.x, vec![0.0; ES]);
    }

    /// Void nodes are skipped, leaving zeros rather than dividing by zero.
    #[test]
    fn void_nodes_stay_zero() {
        let (params, geometry, leakzero, _) =
            setup(BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_1sttransleakagexyz(&params, &geometry, &leakzero, &[0.0; ES]);
        assert_eq!(leak.z, vec![0.0; ES]);
    }
}
