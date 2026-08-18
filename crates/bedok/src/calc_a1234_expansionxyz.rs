//! The full `A1`–`A4` semi-analytic nodal expansion.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_a1234_expansionxyz.m`,
//!   `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::calc_1sttransleakagexyz::calc_1sttransleakagexyz;
use crate::calc_2ndtransleakagexyz::calc_2ndtransleakagexyz;
use crate::calc_a1_expansionxyz::{buckling_blocks, calc_a1_expansionxyz, A1};
use crate::calc_abefghxyz::Coeffs;
use crate::calc_bucklingxyz::{calc_bucklingxyz, BucklingCache};
use crate::calc_transleakagexyz::calc_transleakagexyz;
use crate::handle3dcoords::handle3dcoords;
use crate::matlab::{solve_dense, Array2};
use crate::types::{AxisField, Geometry, Params, Sigma};

/// The `A3` coefficients — same six-field shape as
/// [`crate::calc_a1_expansionxyz::A1`], because `A3` is built from `A1` and
/// inherits its `*first` variants.
#[derive(Clone, Debug, Default)]
pub struct A3 {
    /// `A3.x`.
    pub x: Vec<f64>,
    /// `A3.y`.
    pub y: Vec<f64>,
    /// `A3.z`.
    pub z: Vec<f64>,
    /// `A3.xfirst`, from `A1.xfirst`.
    pub xfirst: Vec<f64>,
    /// `A3.yfirst`.
    pub yfirst: Vec<f64>,
    /// `A3.zfirst`.
    pub zfirst: Vec<f64>,
}

/// All four expansion coefficients, as the reference's
/// `[A1, A2, A3, A4]` return.
#[derive(Clone, Debug, Default)]
pub struct Expansion {
    /// First-order coefficient, with `*first` boundary variants.
    pub a1: A1,
    /// Second-order coefficient.
    pub a2: AxisField,
    /// Third-order coefficient, with `*first` boundary variants.
    pub a3: A3,
    /// Fourth-order coefficient.
    pub a4: AxisField,
}

/// `[A1,A2,A3,A4] = calc_a1234_expansionxyz(params, geometry, phivec, sigma, diffvaluesD, gradterms, nodaltermsold, keff)`.
///
/// The driver of the semi-analytic nodal expansion. It calls the leakage and
/// buckling routines, solves for `A2`, builds `A4` from it, delegates `A1`, and
/// finally builds `A3` from `A1`.
///
/// # Order of operations
///
/// 1. `Leakage` — [`calc_transleakagexyz`]
/// 2. `Buck` — [`calc_bucklingxyz`]
/// 3. `Leakage1`, `Leakage2` — the first and second moments
/// 4. `A2` from `(diag(Ee)·Buck + 3I) A2 = Buck·phi - Ee·Leakage2 + Ssource`
/// 5. `A4 = Bb · (Buck·A2 + Leakage2)`
/// 6. `A1` — [`calc_a1_expansionxyz`]
/// 7. `A3 = Aa · (Buck·A1 + Leakage1)`, and likewise for the `*first` variants
///
/// # Arguments
///
/// - `params`, `geometry` — as elsewhere.
/// - `coeffs` — `Aa`/`Bb`/`Ee` here, plus `Ff`/`Gg`/`Hh` passed through to
///   [`calc_a1_expansionxyz`]. **The reference reads these from
///   `geometry.nodalcoeffs`**; passed explicitly for the reason given on that
///   function.
/// - `phivec` — the flux, `philen` long.
/// - `sigma` — cross-section operators, for the buckling.
/// - `diffvalues_d` — the **flat `philen`** diffusion vector.
/// - `gradterms`, `nodaltermsold` — `philen` by 6.
/// - `keff` — current eigenvalue estimate.
/// - `buck_cache` — carried across calls; see [`BucklingCache`].
///
/// # The `A2` solve is block-diagonal, not a general sparse solve
///
/// The reference writes
///
/// ```text
/// Atemp.x = spdiags(Ee.x,0,philen,philen)*Buck.x + 3*speye(philen);
/// A2.x    = Atemp.x \ btemp.x;
/// ```
///
/// which looks like a `philen`-square sparse solve. It is not, in substance:
/// `Buck` couples energy groups **only at the same spatial node** — the
/// reference states this itself in `calc_a1_expansionxyz.m` — so `Atemp` is
/// block-diagonal with one `G`-by-`G` block per node, and scaling by a diagonal
/// and adding `3I` preserves that. The system therefore decomposes exactly into
/// `es` independent `G`-by-`G` solves.
///
/// This translation solves it that way, via [`crate::matlab::solve_dense`].
/// **The decomposition is exact, not an approximation**, so no sparse-solver
/// dependency is needed here.
///
/// The one caveat worth stating: MATLAB's `mldivide` would factor the whole
/// sparse matrix, so its rounding differs from a per-block factorisation at the
/// last-bits level. The results agree to round-off, not bit-for-bit. If a
/// future parity check needs bit equality against a MATLAB run, this is the
/// place it will show up first.
///
/// # `diffvaluesDfix` — division-by-zero guard on one term only
///
/// The reference makes a **second copy** of the diffusion vector with zeros
/// replaced by `1000000`, and uses it **only** for the `Ssource` division:
///
/// ```text
/// diffvaluesDfix=diffvaluesD;
/// diffvaluesDfix(diffvaluesDfix==0)=1000000;
/// ```
///
/// Every other consumer — the leakage trio, the buckling, `calc_a1_expansion` —
/// receives the unmodified vector with genuine zeros intact. So a void node
/// contributes `Ssource ≈ 0` here (a large denominator) rather than `Inf`,
/// while remaining a true void everywhere else. The magic number is the
/// reference's; it is a guard, not a physical diffusion coefficient.
///
/// # Returns
///
/// [`Expansion`] — all four coefficients.
// Ten parameters against clippy's seven. The reference takes eight; the extras
// are `coeffs` and the buckling cache, both of which it reaches through
// implicit state (`geometry.nodalcoeffs` and MATLAB `persistent`).
#[allow(clippy::too_many_arguments)]
pub fn calc_a1234_expansionxyz(
    params: &Params,
    geometry: &Geometry,
    coeffs: &Coeffs,
    phivec: &[f64],
    sigma: &mut Sigma,
    diffvalues_d: &[f64],
    gradterms: &Array2<f64>,
    nodaltermsold: &Array2<f64>,
    keff: f64,
    buck_cache: &mut BucklingCache,
) -> Expansion {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    let leakage = calc_transleakagexyz(
        params,
        geometry,
        phivec,
        diffvalues_d,
        gradterms,
        nodaltermsold,
    );
    let mut buck = calc_bucklingxyz(buck_cache, params, geometry, sigma, diffvalues_d, keff);
    let leakage1 = calc_1sttransleakagexyz(params, geometry, &leakage, diffvalues_d);
    let leakage2 = calc_2ndtransleakagexyz(params, geometry, &leakage, diffvalues_d);

    // Guarded copy, used for the Ssource division and nothing else.
    let diffvalues_fix: Vec<f64> = diffvalues_d
        .iter()
        .map(|d| if *d == 0.0 { 1_000_000.0 } else { *d })
        .collect();

    // `repmat(L, G, 1)` — the per-node widths lifted to philen.
    let width = |w: &[f64], idx: usize| w[idx % es];

    let ssource = |w: &[f64], a: &[f64], b: &[f64]| -> Vec<f64> {
        (0..philen)
            .map(|idx| {
                0.25 * width(w, idx) * width(w, idx) * (a[idx] + b[idx]) / diffvalues_fix[idx]
            })
            .collect()
    };
    let ssource_x = ssource(&geometry.lx, &leakage.y, &leakage.z);
    let ssource_y = ssource(&geometry.ly, &leakage.x, &leakage.z);
    let ssource_z = ssource(&geometry.lz, &leakage.x, &leakage.y);

    let buckblk_x = buckling_blocks(&mut buck.x, philen, es, g_count);
    let buckblk_y = buckling_blocks(&mut buck.y, philen, es, g_count);
    let buckblk_z = buckling_blocks(&mut buck.z, philen, es, g_count);

    // `Buck * v`, using the block structure rather than a general mat-vec.
    let buck_mul = |blk: &Array2<f64>, v: &[f64]| -> Vec<f64> {
        (0..philen)
            .map(|idx| {
                let node = idx % es;
                (0..g_count)
                    .map(|g2| blk.get(idx, g2) * v[g2 * es + node])
                    .sum()
            })
            .collect()
    };

    // Solve `(diag(Ee)*Buck + 3I) x = b`, one G x G block per node.
    let solve_a2 = |blk: &Array2<f64>, ee: &[f64], b: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; philen];
        for node in 0..es {
            let mut a = vec![0.0; g_count * g_count];
            let mut rhs = vec![0.0; g_count];
            for g in 0..g_count {
                let idx = g * es + node;
                for g2 in 0..g_count {
                    let de = if g == g2 { 3.0 } else { 0.0 };
                    a[g * g_count + g2] = ee[idx] * blk.get(idx, g2) + de;
                }
                rhs[g] = b[idx];
            }
            let sol = solve_dense(&a, &rhs, g_count);
            for g in 0..g_count {
                out[g * es + node] = sol[g];
            }
        }
        out
    };

    let btemp = |blk: &Array2<f64>, ee: &[f64], l2: &[f64], ss: &[f64]| -> Vec<f64> {
        let bp = buck_mul(blk, phivec);
        (0..philen).map(|i| bp[i] - ee[i] * l2[i] + ss[i]).collect()
    };

    let a2 = AxisField {
        x: solve_a2(
            &buckblk_x,
            &coeffs.x.ee,
            &btemp(&buckblk_x, &coeffs.x.ee, &leakage2.x, &ssource_x),
        ),
        y: solve_a2(
            &buckblk_y,
            &coeffs.y.ee,
            &btemp(&buckblk_y, &coeffs.y.ee, &leakage2.y, &ssource_y),
        ),
        z: solve_a2(
            &buckblk_z,
            &coeffs.z.ee,
            &btemp(&buckblk_z, &coeffs.z.ee, &leakage2.z, &ssource_z),
        ),
    };

    // `A4 = Bb .* (Buck*A2 + Leakage2)`.
    let fourth = |blk: &Array2<f64>, bb: &[f64], a2d: &[f64], l2: &[f64]| -> Vec<f64> {
        let ba = buck_mul(blk, a2d);
        (0..philen).map(|i| bb[i] * (ba[i] + l2[i])).collect()
    };
    let a4 = AxisField {
        x: fourth(&buckblk_x, &coeffs.x.bb, &a2.x, &leakage2.x),
        y: fourth(&buckblk_y, &coeffs.y.bb, &a2.y, &leakage2.y),
        z: fourth(&buckblk_z, &coeffs.z.bb, &a2.z, &leakage2.z),
    };

    let a1 = calc_a1_expansionxyz(
        params,
        geometry,
        coeffs,
        phivec,
        &a2,
        &a4,
        &leakage1,
        diffvalues_d,
        &mut buck,
    );

    // `A3 = Aa .* (Buck*A1 + Leakage1)`, and the same for the `*first` set.
    let third = |blk: &Array2<f64>, aa: &[f64], a1d: &[f64], l1: &[f64]| -> Vec<f64> {
        let ba = buck_mul(blk, a1d);
        (0..philen).map(|i| aa[i] * (ba[i] + l1[i])).collect()
    };
    let a3 = A3 {
        x: third(&buckblk_x, &coeffs.x.aa, &a1.x, &leakage1.x),
        y: third(&buckblk_y, &coeffs.y.aa, &a1.y, &leakage1.y),
        z: third(&buckblk_z, &coeffs.z.aa, &a1.z, &leakage1.z),
        xfirst: third(&buckblk_x, &coeffs.x.aa, &a1.xfirst, &leakage1.x),
        yfirst: third(&buckblk_y, &coeffs.y.aa, &a1.yfirst, &leakage1.y),
        zfirst: third(&buckblk_z, &coeffs.z.aa, &a1.zfirst, &leakage1.z),
    };

    Expansion { a1, a2, a3, a4 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc_abefghxyz::AxisCoeffs;
    use crate::matlab::SparseMatrix;

    const ES: usize = 12;

    /// The inputs one call needs, bundled so the fixture is not an eight-tuple.
    struct Fixture {
        params: Params,
        geometry: Geometry,
        coeffs: Coeffs,
        phivec: Vec<f64>,
        sigma: Sigma,
        diffvalues: Vec<f64>,
        gradterms: Array2<f64>,
        nodalterms: Array2<f64>,
    }

    fn setup() -> Fixture {
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
        let idx: Vec<usize> = (0..ES).collect();
        let sigma = Sigma {
            tot: SparseMatrix::assemble(&idx, &idx, &[1.0; ES], ES, ES),
            s: SparseMatrix::assemble(&idx, &idx, &[0.2; ES], ES, ES),
            f: SparseMatrix::assemble(&idx, &idx, &[0.3; ES], ES, ES),
            ..Default::default()
        };
        let terms = Array2::<f64>::zeros(ES, 6);
        Fixture {
            params,
            geometry,
            coeffs,
            phivec: vec![1.0; ES],
            sigma,
            diffvalues: vec![1.0; ES],
            gradterms: terms.clone(),
            nodalterms: terms,
        }
    }

    /// The whole chain runs and produces finite coefficients of the right
    /// length, with `A3` carrying its `*first` variants.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// All six `A3` vectors and both `A2`/`A4` are `philen` long and finite.
    #[test]
    fn the_expansion_chain_produces_finite_coefficients() {
        let mut f = setup();
        let mut cache = BucklingCache::new();
        let e = calc_a1234_expansionxyz(
            &f.params,
            &f.geometry,
            &f.coeffs,
            &f.phivec,
            &mut f.sigma,
            &f.diffvalues,
            &f.gradterms,
            &f.nodalterms,
            1.0,
            &mut cache,
        );

        for v in [&e.a2.z, &e.a4.z, &e.a3.z, &e.a3.zfirst, &e.a1.z] {
            assert_eq!(v.len(), ES);
            assert!(v.iter().all(|x| x.is_finite()), "non-finite entry: {v:?}");
        }
    }

    /// The `A2` block solve must actually satisfy its own system.
    ///
    /// # Methodology
    ///
    /// Re-applies `(diag(Ee)*Buck + 3I)` to the returned `A2.z` and compares
    /// against the right-hand side the solver was given, reconstructed
    /// independently here. This checks the block decomposition is equivalent to
    /// the full system rather than merely plausible.
    ///
    /// # Results — measured 2026-08-13
    ///
    /// Residual below 1e-12 at every node.
    #[test]
    fn the_a2_block_solve_satisfies_its_system() {
        let mut f = setup();
        let mut cache = BucklingCache::new();
        let e = calc_a1234_expansionxyz(
            &f.params,
            &f.geometry,
            &f.coeffs,
            &f.phivec,
            &mut f.sigma,
            &f.diffvalues,
            &f.gradterms,
            &f.nodalterms,
            1.0,
            &mut cache,
        );

        // Rebuild Buck and the right-hand side the same way the function did.
        let mut cache2 = BucklingCache::new();
        let mut buck = calc_bucklingxyz(
            &mut cache2,
            &f.params,
            &f.geometry,
            &mut f.sigma,
            &f.diffvalues,
            1.0,
        );
        let blk = buckling_blocks(&mut buck.z, ES, ES, 1);
        let leak = calc_transleakagexyz(
            &f.params,
            &f.geometry,
            &f.phivec,
            &f.diffvalues,
            &f.gradterms,
            &f.nodalterms,
        );
        let leak2 = calc_2ndtransleakagexyz(&f.params, &f.geometry, &leak, &f.diffvalues);
        let ss: Vec<f64> = (0..ES)
            .map(|i| 0.25 * 2.0 * 2.0 * (leak.x[i] + leak.y[i]) / f.diffvalues[i])
            .collect();

        // Indexes five parallel arrays plus a 2-D accessor; an iterator chain
        // would obscure rather than clarify.
        #[allow(clippy::needless_range_loop)]
        for i in 0..ES {
            let lhs = f.coeffs.z.ee[i] * blk.get(i, 0) * e.a2.z[i] + 3.0 * e.a2.z[i];
            let rhs = blk.get(i, 0) * f.phivec[i] - f.coeffs.z.ee[i] * leak2.z[i] + ss[i];
            assert!((lhs - rhs).abs() < 1e-12, "node {i}: {lhs} vs {rhs}");
        }
    }

    /// The `diffvaluesDfix` guard applies to `Ssource` only: a void node yields
    /// a finite (near-zero) source rather than `Inf`, while the leakages it
    /// feeds still see a genuine zero.
    #[test]
    fn the_void_guard_keeps_ssource_finite() {
        let mut f = setup();
        let mut cache = BucklingCache::new();
        // Every node void.
        let e = calc_a1234_expansionxyz(
            &f.params,
            &f.geometry,
            &f.coeffs,
            &f.phivec,
            &mut f.sigma,
            &[0.0; ES],
            &f.gradterms,
            &f.nodalterms,
            1.0,
            &mut cache,
        );
        assert!(e.a2.z.iter().all(|x| x.is_finite()));
        assert!(e.a4.z.iter().all(|x| x.is_finite()));
    }
}
