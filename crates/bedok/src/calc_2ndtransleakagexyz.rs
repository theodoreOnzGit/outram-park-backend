//! Second-moment transverse leakages — the quadratic term of the
//! transverse-leakage fit.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_2ndtransleakagexyz.m`, `main_exec_diff3d_standalone`
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
/// A third copy of the reference's line-finding loop; see
/// [`crate::calc_1sttransleakagexyz`] for why the copies are kept separate.
struct Line {
    /// Node indices (no group offset) strictly interior to the line, with a
    /// non-zero diffusion coefficient.
    interior: Vec<usize>,
    /// Node index of the first fuelled node on the line.
    low: usize,
    /// Node index of the last fuelled node on the line.
    high: usize,
}

/// `Leakage = calc_2ndtransleakagexyz(params, geometry, Leakzero, diffvalues)`.
///
/// Fits the **second moment** of the transverse leakage on each axis. The
/// transverse coupling is identical to
/// [`crate::calc_1sttransleakagexyz::calc_1sttransleakagexyz`] — each axis is
/// driven by the sum of the other two axes' zeroth-moment leakages — and so are
/// the arguments, returns and the two-node-minimum constraint.
///
/// # How it differs from the first moment
///
/// Three changes, all in the formulas:
///
/// **Interior.** The mesh-ratio weights lose their `(2t + 1)` factors, and the
/// minus term reverses sign:
///
/// ```text
/// first:   LL = [ (tm+1)(2tm+1)(S_p - S) + (tp+1)(2tp+1)(S - S_m) ] / h
/// second:  LL = [ (tm+1)        (S_p - S) + (tp+1)        (S_m - S) ] / h
/// ```
///
/// with the same `h = 2 (tp+1)(tm+1)(tm+tp+1)` and the same `0.25 L^2 / D`
/// scaling. On a uniform mesh the second-moment stencil collapses to
/// `(S_p + S_m - 2S) / 12` — a discrete second derivative, which is **exactly
/// zero for a linear source**. That is the sense in which it is the quadratic
/// term.
///
/// **Vacuum and zero-flux faces contribute nothing.** Where the first-moment
/// version computes a one-sided difference, this one's `switch` runs
/// `continue`, leaving the preallocated zero in place. Only a reflective face
/// gets a value.
///
/// **Reflective faces use `2/h` rather than `6/h`**, and the high face takes
/// `S_minus - S` rather than `S - S_minus` — matching the reversed interior
/// sign convention.
///
/// # Panics
///
/// If a boundary node's neighbour index falls outside the vector — the same
/// two-node-minimum constraint documented on
/// [`crate::calc_transleakagexyz::calc_transleakagexyz`].
pub fn calc_2ndtransleakagexyz(
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

        // Interior: the three-point stencil, without the (2t+1) weights.
        for line in lines {
            for &s in line.interior.iter() {
                for g in 0..g_count {
                    let idx = g * es + s;
                    let ip = idx + stride;
                    let im = minus(idx);

                    let tp = width(ip) / width(idx);
                    let tm = width(im) / width(idx);
                    let h = 2.0 * (tp + 1.0) * (tm + 1.0) * (tm + tp + 1.0);
                    let ll = ((tm + 1.0) * (ssource[ip] - ssource[idx])
                        + (tp + 1.0) * (ssource[im] - ssource[idx]))
                        / h;

                    out[idx] = ll * 0.25 * width(idx).powi(2) / diffvalues[idx];
                }
            }
        }

        // Low face — only reflective contributes; vacuum and zero-flux
        // `continue` in the reference, leaving zero.
        if bc_min == BoundaryCondition::Reflective {
            for line in lines {
                for g in 0..g_count {
                    let idx = g * es + line.low;
                    if diffvalues[idx] == 0.0 {
                        continue;
                    }
                    let idxplus = idx + stride;
                    let tplus = width(idxplus) / width(idx);
                    let h = 4.0 * (tplus + 1.0) * (tplus + 2.0);

                    let value = 2.0 * (ssource[idxplus] - ssource[idx]) / h;
                    out[idx] = value * 0.25 * width(idx).powi(2) / diffvalues[idx];
                }
            }
        }

        // High face — likewise.
        if bc_max == BoundaryCondition::Reflective {
            for line in lines {
                for g in 0..g_count {
                    let idx = g * es + line.high;
                    if diffvalues[idx] == 0.0 {
                        continue;
                    }
                    let idxminus = minus(idx);
                    let tminus = width(idxminus) / width(idx);
                    let h = 4.0 * (tminus + 1.0) * (tminus + 2.0);

                    let value = 2.0 * (ssource[idxminus] - ssource[idx]) / h;
                    out[idx] = value * 0.25 * width(idx).powi(2) / diffvalues[idx];
                }
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

    /// A 2x2x3 grid with uniform widths `L = 2` and `D = 1`.
    ///
    /// `Leakzero.y` carries the profile under test and the other two axes are
    /// zero, so `Ssource.z = Leakzero.x + Leakzero.y` is that profile.
    fn setup(
        profile: Vec<f64>,
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
            y: profile,
            z: vec![0.0; ES],
        };
        (params, geometry, leakzero, vec![1.0; ES])
    }

    /// A linear source has zero second moment — the defining property of this
    /// stencil, and the sharpest check that the weights are transcribed right.
    ///
    /// # Methodology
    ///
    /// On a uniform mesh the interior stencil is `(S_p + S_m - 2S)/12`. For a
    /// ramp `S = i` that is `(i+1) + (i-1) - 2i = 0` exactly. Node 1 is the
    /// interior node of the `z` line at `(0, 0)`.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Exactly zero. A mistyped weight would break this immediately, since the
    /// cancellation is exact rather than approximate.
    #[test]
    fn a_linear_source_has_zero_second_moment() {
        let ramp: Vec<f64> = (0..ES).map(|i| i as f64).collect();
        let (params, geometry, leakzero, diffvalues) =
            setup(ramp, BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z[1], 0.0);
    }

    /// A quadratic source gives the expected non-zero curvature.
    ///
    /// # Methodology
    ///
    /// For `S = i^2`, `S_p + S_m - 2S = (i+1)^2 + (i-1)^2 - 2i^2 = 2`, so the
    /// stencil gives `2/12 = 1/6`. The scale factor `0.25 * L^2 / D` is 1.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// `1/6` to within 1e-15.
    #[test]
    fn a_quadratic_source_gives_the_expected_curvature() {
        let quad: Vec<f64> = (0..ES).map(|i| (i * i) as f64).collect();
        let (params, geometry, leakzero, diffvalues) =
            setup(quad, BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert!((leak.z[1] - 1.0 / 6.0).abs() < 1e-15, "got {}", leak.z[1]);
    }

    /// Vacuum faces contribute nothing at all here, unlike the first moment.
    #[test]
    fn vacuum_faces_stay_zero() {
        let ramp: Vec<f64> = (0..ES).map(|i| i as f64).collect();
        let (params, geometry, leakzero, diffvalues) =
            setup(ramp, BoundaryCondition::Vacuum, BoundaryCondition::Vacuum);
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert_eq!(leak.z[0], 0.0);
        assert_eq!(leak.z[2], 0.0);
    }

    /// A reflective face uses the `2/h` form.
    ///
    /// # Methodology
    ///
    /// `h = 4*(1+1)*(1+2) = 24`, so the low face is
    /// `2*(S(1) - S(0))/24 = 2/24 = 1/12` for the ramp, scaled by 1. The high
    /// face is left vacuum and must stay zero.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Low face `1/12`, high face `0`.
    #[test]
    fn a_reflective_face_uses_the_two_over_h_form() {
        let ramp: Vec<f64> = (0..ES).map(|i| i as f64).collect();
        let (params, geometry, leakzero, diffvalues) = setup(
            ramp,
            BoundaryCondition::Reflective,
            BoundaryCondition::Vacuum,
        );
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &leakzero, &diffvalues);
        assert!((leak.z[0] - 1.0 / 12.0).abs() < 1e-15, "got {}", leak.z[0]);
        assert_eq!(leak.z[2], 0.0);
    }

    /// The transverse coupling, as an A/B on where the source sits.
    ///
    /// `Ssource.z = Leakzero.x + Leakzero.y`, so the same profile placed on `y`
    /// drives the `z` result while placed on `z` it does not.
    ///
    /// # Why this is not phrased as "the other axes see it"
    ///
    /// The obvious mirror check — assert `leak.x` is non-zero — would fail for
    /// a correct implementation on this grid. The `x` direction has only two
    /// nodes, so it has **no interior nodes**, and its vacuum faces contribute
    /// nothing in the second moment (they `continue`). All-zero is right there.
    /// The first-moment version does not share that property, because its
    /// vacuum faces do compute a one-sided difference.
    #[test]
    fn an_axis_is_not_driven_by_its_own_leakage() {
        let quad: Vec<f64> = (0..ES).map(|i| (i * i) as f64).collect();
        let (params, geometry, _, diffvalues) = setup(
            vec![0.0; ES],
            BoundaryCondition::Vacuum,
            BoundaryCondition::Vacuum,
        );

        // Source on z: excluded from Ssource.z, so no z response.
        let on_z = Leakage {
            x: vec![0.0; ES],
            y: vec![0.0; ES],
            z: quad.clone(),
        };
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &on_z, &diffvalues);
        assert_eq!(leak.z, vec![0.0; ES]);

        // The same profile on y does drive z.
        let on_y = Leakage {
            x: vec![0.0; ES],
            y: quad,
            z: vec![0.0; ES],
        };
        let leak = calc_2ndtransleakagexyz(&params, &geometry, &on_y, &diffvalues);
        assert_ne!(leak.z, vec![0.0; ES]);
    }
}
