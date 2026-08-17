//! The A, B, E, F, G, H coefficients of the semi-analytic nodal update.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_ABEFGHxyz.m`, `main_exec_diff3d_standalone`
//!   snapshot. The Rust module is `calc_abefghxyz` because Rust warns on
//!   non-snake-case module names.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::Array4;
use crate::types::{Geometry, Params, Sigma};

/// The six coefficients along one axis, one entry per `(group, node)`.
///
/// Each vector is `philen = G * maxix * maxiy * maxiz` long, ordered
/// `g*es + ix*maxiy*maxiz + iy*maxiz + iz`. Entries outside the core stay zero.
#[derive(Clone, Debug, Default)]
pub struct AxisCoeffs {
    /// `Aa` — coefficient of the second-order flux moment.
    pub aa: Vec<f64>,
    /// `Bb` — coefficient of the third-order flux moment.
    pub bb: Vec<f64>,
    /// `Ee` — surface-to-average flux ratio term.
    pub ee: Vec<f64>,
    /// `Ff` — surface-current term for the even moment.
    pub ff: Vec<f64>,
    /// `Gg` — odd-moment current ratio.
    pub gg: Vec<f64>,
    /// `Hh` — even-moment current ratio.
    pub hh: Vec<f64>,
}

impl AxisCoeffs {
    /// All six vectors zeroed to length `philen`.
    fn zeros(philen: usize) -> Self {
        Self {
            aa: vec![0.0; philen],
            bb: vec![0.0; philen],
            ee: vec![0.0; philen],
            ff: vec![0.0; philen],
            gg: vec![0.0; philen],
            hh: vec![0.0; philen],
        }
    }
}

/// `Coeffs` — the six coefficients on all three axes.
#[derive(Clone, Debug, Default)]
pub struct Coeffs {
    /// Coefficients along `x`.
    pub x: AxisCoeffs,
    /// Coefficients along `y`.
    pub y: AxisCoeffs,
    /// Coefficients along `z`.
    pub z: AxisCoeffs,
}

/// The six coefficients at a single value of `alpha`.
///
/// `alpha = 0.5 * L * sqrt(Sigma_r / D)` is the node's half-width measured in
/// diffusion lengths — the natural argument of the semi-analytic nodal
/// expansion, since the homogeneous solution of the 1-D diffusion equation over
/// a node goes as `sinh`/`cosh` of it.
///
/// # Arguments
///
/// - `a` — `alpha`, dimensionless and positive.
///
/// # Returns
///
/// `(Aa, Bb, Ee, Ff, Gg, Hh)`, all dimensionless.
///
/// # Numerical limitation at small `alpha` — not guarded by the reference
///
/// Both `ms` and `mc` are built from differences that cancel as `a` goes to
/// zero:
///
/// ```text
/// ms = 3*(ch/a - sh/a^2)
/// mc = 5*(sh/a - 3*ch/a^2 + 3*sh/a^3)
/// ```
///
/// Analytically `ms -> a/…` and `mc -> a^2/…` are perfectly finite, but each is
/// computed as a difference of terms that individually blow up like `1/a` and
/// `1/a^3`. In floating point the leading digits cancel, so the relative error
/// in `ms` and `mc` grows as `a` shrinks, and both appear in denominators
/// below.
///
/// A node is at small `alpha` when it is optically thin — a large diffusion
/// coefficient, a small removal cross section, or a fine mesh. The reference
/// does not switch to a series expansion there, and neither does this
/// translation. Recorded rather than repaired, per
/// `docs/bedok-port-scoping.md` §1.0.
///
/// At `a == 0` exactly the result is all `NaN`, from `0/0`. That case is
/// unreachable through [`calc_abefghxyz`], which filters to nodes with a
/// non-zero diffusion coefficient.
fn abefgh(a: f64) -> (f64, f64, f64, f64, f64, f64) {
    let sh = a.sinh();
    let ch = a.cosh();

    let ms = 3.0 * (ch / a - sh / a / a);
    let mc = 5.0 * (sh / a - 3.0 * ch / a / a + 3.0 * sh / a.powi(3));

    // Element order matches the reference's scalar formulas exactly.
    let aa = (sh - ms) / (a * a * ms);
    let bb = (ch - sh / a - mc) / (a * a * mc);
    let ee = sh / a / mc - 3.0 / a.powi(2);
    let ff = (a * ch - ms) / (a * a * ms);
    let gg = (a * sh - 3.0 * mc) / (ch - sh / a - mc);
    let hh = (a * ch - ms) / (sh - ms);

    (aa, bb, ee, ff, gg, hh)
}

/// `Coeffs = calc_ABEFGHxyz(params, geometry, sigma, diffvalues)`.
///
/// Computes the semi-analytic nodal coefficients for every in-core
/// `(group, node)` on all three axes.
///
/// # Arguments
///
/// - `params` — supplies `G` and the extents.
/// - `geometry` — supplies the per-node widths `lx`, `ly`, `lz`.
/// - `sigma` — supplies `tot` and `s`; only their **diagonals** are read, as
///   the removal cross section `Sigma_r = Sigma_tot - Sigma_s`.
/// - `diffvalues` — diffusion coefficients from
///   [`crate::calcdiffvalues3d::calcdiffvalues3d`], indexed
///   `(ix, iy, iz, g)`.
///
/// # Returns
///
/// [`Coeffs`], with entries left at zero for every node outside the core.
///
/// # Which nodes are "in core"
///
/// The reference selects on `dvec ~= 0` — a node is in-core exactly when its
/// diffusion coefficient is non-zero. That is the same convention
/// `calcdiffvalues3d` establishes by leaving void nodes at zero, so the two
/// agree by construction. It does mean a genuine zero `D` would be read as
/// "outside the core", but `D = 1/(3*Sigma_tot)` cannot be zero for finite
/// `Sigma_tot`.
///
/// # Flattening
///
/// The reference writes `reshape(permute(diffvalues,[3 2 1 4]), philen, 1)`,
/// which reorders `(ix, iy, iz, g)` to `(iz, iy, ix, g)` and then reads it
/// column-major. That lands each element at
/// `g*es + ix*maxiy*maxiz + iy*maxiz + iz`, which is the ordering everything
/// else in the solver uses. Here the index is written out directly rather than
/// going through a permute.
pub fn calc_abefghxyz(
    params: &Params,
    geometry: &Geometry,
    sigma: &mut Sigma,
    diffvalues: &Array4<f64>,
) -> Coeffs {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let xstep = maxiy * maxiz;
    let es = maxix * maxiy * maxiz;
    let philen = g_count * es;

    // `sigmar = full(diag(sigma.tot - sigma.s))` — the removal cross section.
    // Only the diagonal is needed, so the difference is taken entry-wise there
    // rather than forming the whole matrix.
    let mut sigmar = vec![0.0; philen];
    for t in sigma.tot.find() {
        if t.i == t.j {
            sigmar[t.i] += t.v;
        }
    }
    for t in sigma.s.find() {
        if t.i == t.j {
            sigmar[t.i] -= t.v;
        }
    }

    // The reference's permute/reshape, written as the index it produces.
    let mut dvec = vec![0.0; philen];
    for g in 0..g_count {
        for ix in 0..maxix {
            for iy in 0..maxiy {
                for iz in 0..maxiz {
                    dvec[g * es + ix * xstep + iy * maxiz + iz] =
                        diffvalues.get(ix, iy, iz, g);
                }
            }
        }
    }

    let mut coeffs = Coeffs {
        x: AxisCoeffs::zeros(philen),
        y: AxisCoeffs::zeros(philen),
        z: AxisCoeffs::zeros(philen),
    };

    for idx in 0..philen {
        // `iv = find(dvec~=0)` — in-core node/group indices.
        if dvec[idx] == 0.0 {
            continue;
        }

        // sqrt(Sigma_r / D); the same value serves all three axes at a node.
        let r = (sigmar[idx] / dvec[idx]).sqrt();

        // `repmat(Lx, G, 1)` — the per-node widths repeat for every group, so
        // the group-carrying index folds back onto the node index.
        let node = idx % es;

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.lx[node]);
        coeffs.x.aa[idx] = aa;
        coeffs.x.bb[idx] = bb;
        coeffs.x.ee[idx] = ee;
        coeffs.x.ff[idx] = ff;
        coeffs.x.gg[idx] = gg;
        coeffs.x.hh[idx] = hh;

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.ly[node]);
        coeffs.y.aa[idx] = aa;
        coeffs.y.bb[idx] = bb;
        coeffs.y.ee[idx] = ee;
        coeffs.y.ff[idx] = ff;
        coeffs.y.gg[idx] = gg;
        coeffs.y.hh[idx] = hh;

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.lz[node]);
        coeffs.z.aa[idx] = aa;
        coeffs.z.bb[idx] = bb;
        coeffs.z.ee[idx] = ee;
        coeffs.z.ff[idx] = ff;
        coeffs.z.gg[idx] = gg;
        coeffs.z.hh[idx] = hh;
    }

    coeffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matlab::SparseMatrix;

    /// One node, one group, unit dimensions.
    fn setup(sigma_tot: f64, sigma_s: f64, d: f64) -> (Params, Geometry, Sigma, Array4<f64>) {
        let params = Params {
            maxix: Some(1),
            maxiy: Some(1),
            maxiz: Some(1),
            g: 1,
            ..Default::default()
        };
        let geometry = Geometry {
            lx: vec![1.0],
            ly: vec![2.0],
            lz: vec![4.0],
            ..Default::default()
        };
        let sigma = Sigma {
            tot: SparseMatrix::assemble(&[0], &[0], &[sigma_tot], 1, 1),
            s: SparseMatrix::assemble(&[0], &[0], &[sigma_s], 1, 1),
            f: SparseMatrix::zeros(1, 1),
            ..Default::default()
        };
        let mut diffvalues = Array4::<f64>::zeros(1, 1, 1, 1);
        diffvalues.set(0, 0, 0, 0, d);
        (params, geometry, sigma, diffvalues)
    }

    /// `Hh` has a closed form worth checking independently.
    ///
    /// # Methodology
    ///
    /// With `ms = 3*(ch/a - sh/a^2)`, the reference's
    /// `Hh = (a*ch - ms)/(sh - ms)` is evaluated directly here at `a = 1` from
    /// `sinh(1)` and `cosh(1)`, and compared against [`abefgh`]. This checks the
    /// transcription of the expression, not the physics.
    ///
    /// # Results — measured 2026-08-12
    ///
    /// Agrees to within 1e-12 relative.
    #[test]
    fn hh_matches_a_hand_evaluation_at_alpha_one() {
        let a: f64 = 1.0;
        let sh = a.sinh();
        let ch = a.cosh();
        let ms = 3.0 * (ch / a - sh / a / a);
        let expected = (a * ch - ms) / (sh - ms);

        let (_, _, _, _, _, hh) = abefgh(a);
        assert!((hh - expected).abs() / expected.abs() < 1e-12);
    }

    /// A void node — zero diffusion coefficient — must be skipped, leaving all
    /// six coefficients at zero rather than `NaN`.
    #[test]
    fn void_nodes_are_left_at_zero() {
        let (params, geometry, mut sigma, _) = setup(1.0, 0.5, 0.0);
        let diffvalues = Array4::<f64>::zeros(1, 1, 1, 1);
        let c = calc_abefghxyz(&params, &geometry, &mut sigma, &diffvalues);
        assert_eq!(c.x.aa[0], 0.0);
        assert_eq!(c.z.hh[0], 0.0);
    }

    /// The three axes differ only through their node width, so a node whose
    /// `Ly` is twice its `Lx` sees exactly twice the `alpha`.
    #[test]
    fn axes_differ_only_through_node_width() {
        let (params, geometry, mut sigma, diffvalues) = setup(1.0, 0.5, 1.0);
        let c = calc_abefghxyz(&params, &geometry, &mut sigma, &diffvalues);

        let r = (0.5f64 / 1.0).sqrt();
        let (aa_x, ..) = abefgh(0.5 * r * 1.0);
        let (aa_y, ..) = abefgh(0.5 * r * 2.0);
        let (aa_z, ..) = abefgh(0.5 * r * 4.0);

        assert!((c.x.aa[0] - aa_x).abs() < 1e-15);
        assert!((c.y.aa[0] - aa_y).abs() < 1e-15);
        assert!((c.z.aa[0] - aa_z).abs() < 1e-15);
    }

    /// Pins the documented small-`alpha` cancellation: the coefficients are
    /// still finite at `alpha = 1e-4`, but `Aa` should be compared against its
    /// analytic limit with low expectations. This test records that the values
    /// remain finite rather than asserting they are accurate.
    #[test]
    fn small_alpha_stays_finite_but_is_not_trusted() {
        let (aa, bb, ee, ff, gg, hh) = abefgh(1e-4);
        for v in [aa, bb, ee, ff, gg, hh] {
            assert!(v.is_finite(), "expected finite, got {v}");
        }
    }
}
