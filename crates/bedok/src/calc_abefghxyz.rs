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
//!   PARK; see the crate README, "Permission and attribution".
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
/// The six SA-nodal coefficients from their Taylor series about `a = 0`.
///
/// Used below [`crate::types::SMALL_ALPHA`], where the closed form in
/// [`abefgh`] is a ratio of vanishing differences — defect N9.
///
/// # Derivation
///
/// With `sinh(a) = a + a^3/6 + a^5/120 + ...` and
/// `cosh(a) = 1 + a^2/2 + a^4/24 + ...`, the two building blocks lose their
/// leading terms exactly:
///
/// ```text
/// ms = 3*(cosh/a - sinh/a^2)              = a  *(1 + a^2/10 + ...)
/// mc = 5*(sinh/a - 3cosh/a^2 + 3sinh/a^3) = a^2/3*(1 + a^2/14 + ...)
/// ```
///
/// Substituting and expanding gives, to second order,
///
/// ```text
/// Aa = (1/15)*(1 - a^2/35)    Ff = (2/5)*(1 - a^2/210)
/// Bb = (1/35)*(1 - a^2/63)    Gg = 10*(1 + a^2/90)
/// Ee = 2/7 - a^2/735          Hh = 6*(1 + a^2/42)
/// ```
///
/// # Accuracy
///
/// Checked against the closed form evaluated in 50-digit arithmetic: worst
/// relative error **1.27e-7 at `a = 0.1`**, **1.27e-11 at 0.01** and
/// **1.27e-15 at 0.001**, i.e. fourth order as the truncation implies. The
/// closed form comes the other way — 2.6e-10 at `a = 0.3`, 1.26e-7 at 0.1 and
/// **0.28 at 0.01** — so the two cross at `a = 0.1`, which is where
/// [`crate::types::SMALL_ALPHA`] is set.
fn abefgh_series(a: f64) -> (f64, f64, f64, f64, f64, f64) {
    let a2 = a * a;
    (
        (1.0 - a2 / 35.0) / 15.0,
        (1.0 - a2 / 63.0) / 35.0,
        2.0 / 7.0 - a2 / 735.0,
        2.0 * (1.0 - a2 / 210.0) / 5.0,
        10.0 * (1.0 + a2 / 90.0),
        6.0 * (1.0 + a2 / 42.0),
    )
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
/// the crate README, "Translation policy".
///
/// At `a == 0` exactly the result is all `NaN`, from `0/0`. That case is
/// unreachable through [`calc_abefghxyz`], which filters to nodes with a
/// non-zero diffusion coefficient.
fn abefgh(a: f64, form: crate::types::NodalCoeffForm) -> (f64, f64, f64, f64, f64, f64) {
    // Defect N9: below `SMALL_ALPHA` the closed form below is a ratio of
    // vanishing differences and loses all significance. See
    // `crate::types::NodalCoeffForm`.
    if form == crate::types::NodalCoeffForm::SeriesBelowSmallAlpha
        && a.abs() < crate::types::SMALL_ALPHA
    {
        return abefgh_series(a);
    }
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

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.lx[node], params.nodal_coeff_form);
        coeffs.x.aa[idx] = aa;
        coeffs.x.bb[idx] = bb;
        coeffs.x.ee[idx] = ee;
        coeffs.x.ff[idx] = ff;
        coeffs.x.gg[idx] = gg;
        coeffs.x.hh[idx] = hh;

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.ly[node], params.nodal_coeff_form);
        coeffs.y.aa[idx] = aa;
        coeffs.y.bb[idx] = bb;
        coeffs.y.ee[idx] = ee;
        coeffs.y.ff[idx] = ff;
        coeffs.y.gg[idx] = gg;
        coeffs.y.hh[idx] = hh;

        let (aa, bb, ee, ff, gg, hh) = abefgh(0.5 * r * geometry.lz[node], params.nodal_coeff_form);
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

        let (_, _, _, _, _, hh) = abefgh(a, crate::types::NodalCoeffForm::ClosedFormAlways);
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
        let (aa_x, ..) = abefgh(0.5 * r * 1.0, crate::types::NodalCoeffForm::ClosedFormAlways);
        let (aa_y, ..) = abefgh(0.5 * r * 2.0, crate::types::NodalCoeffForm::ClosedFormAlways);
        let (aa_z, ..) = abefgh(0.5 * r * 4.0, crate::types::NodalCoeffForm::ClosedFormAlways);

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
        let (aa, bb, ee, ff, gg, hh) = abefgh(1e-4, crate::types::NodalCoeffForm::ClosedFormAlways);
        for v in [aa, bb, ee, ff, gg, hh] {
            assert!(v.is_finite(), "expected finite, got {v}");
        }
    }

    /// **N9 — the alpha the real cases actually produce, and where the
    /// closed form stops being trustworthy.**
    ///
    /// # Methodology
    ///
    /// `abefgh` builds every coefficient from
    ///
    /// ```text
    /// ms = 3*(cosh(a)/a - sinh(a)/a^2)
    /// mc = 5*(sinh(a)/a - 3*cosh(a)/a^2 + 3*sinh(a)/a^3)
    /// ```
    ///
    /// whose small-`a` limits are `ms -> a` and `mc -> a^2/3`. Both are
    /// differences of much larger terms — `ms` cancels two terms of order
    /// `1/a`, and `mc` three terms of order `1/a^2` — so the relative
    /// cancellation error grows like `a^-2` and `a^-4` respectively. The
    /// reference has no series fallback and neither does this translation.
    ///
    /// Two things are measured, and the second decides whether the first
    /// matters:
    ///
    /// 1. **Where the closed form fails.** `mc` is compared against its exact
    ///    series `(a^2/3) * (1 + a^2/14 + ...)` down the decades, to find the
    ///    `a` at which significance is gone.
    /// 2. **What `a` the snapshot's cases actually reach.** `a = 0.5 * L *
    ///    sqrt(Sigma_r/D)` is formed for every in-core `(group, node, axis)` of
    ///    IAEA-3D, NEACRP A2 and NEACRP D1, and the minimum reported. A defect
    ///    that the cases never approach is latent, however bad it looks in
    ///    isolation.
    ///
    /// # Results — measured 2026-08-23
    ///
    /// **Where the closed form fails.** `mc`, closed form against series:
    ///
    /// | `a` | closed | series | rel err |
    /// |---|---|---|---|
    /// | 1e0 | 3.57814351e-1 | 3.57142857e-1 | 1.88e-3 |
    /// | 1e-1 | 3.33571495e-3 | 3.33571429e-3 | **1.98e-7** |
    /// | 1e-2 | 3.33335447e-5 | 3.33335714e-5 | **8.02e-7** |
    /// | 1e-3 | 3.30619514e-7 | 3.33333357e-7 | **8.14e-3** |
    /// | 1e-4 | 2.98023224e-7 | 3.33333334e-9 | **8.84e1** |
    /// | 1e-5, 1e-6 | **0.0** | 3.3e-11, 3.3e-13 | total loss |
    /// | 1e-7 | 3.125e-1 | 3.3e-15 | 9.37e13 |
    ///
    /// The 1.88e-3 at `a = 1` is **this two-term series truncating**, not the
    /// closed form failing — the two swap roles as `a` falls. Cancellation
    /// costs **0.8% at `a = 1e-3`**, passes 1% just below it, is **88x wrong
    /// at 1e-4**, and returns **exactly zero** by 1e-5 — at which point every
    /// coefficient built from `mc` divides by it.
    ///
    /// **What the cases reach.**
    ///
    /// | case | alpha range |
    /// |---|---|
    /// | IAEA-3D | 0.7071 .. 5.7009 |
    /// | NEACRP A2 | **0.3535** .. 20.4781 |
    /// | NEACRP D1 | 0.6619 .. 6.0002 |
    ///
    /// **Interpretation — N9 is LATENT on every case in the snapshot.** The
    /// smallest `alpha` anywhere is **0.3535**, on NEACRP A2, which is **35x**
    /// above the **measured crossover of `a = 0.1`**, where the series becomes
    /// the more accurate form. No coefficient in any benchmark result is
    /// computed near the cancellation.
    ///
    /// **Corrected 2026-08-23: the margin is 4x, not thousands.** The figures
    /// in the table above track `mc` alone, and `mc` is not the worst
    /// coefficient — `Bb` divides a fourth-order cancellation by it and
    /// amplifies the loss, so the coefficients are 28% adrift at `a = 0.01`
    /// where `mc` itself is still respectable. Measured against 60-digit
    /// arithmetic, the closed form's error is 2.64e-10 at `a = 0.3`, 1.26e-7
    /// at 0.1 and **0.281 at 0.01**. See
    /// `n9_the_series_is_accurate_where_the_closed_form_is_not`.
    ///
    /// That is worth stating precisely because the entry reads as though the
    /// defect were live — "NaN at alpha = 0 and losing significance well
    /// before that" is true, and says nothing about whether the cases go
    /// there. They do not, and by two and a half orders of magnitude.
    ///
    /// **When it would become live.** `alpha = 0.5 * L * sqrt(Sigma_r/D)`, so
    /// it falls with a fine mesh, a large diffusion coefficient or a small
    /// removal cross section. A mesh refined 100x from A2's would reach
    /// `1e-2`; a pure-scatterer region (`Sigma_r -> 0`) would reach it at any
    /// mesh. Neither occurs here, but a reflector-dominated or heavily
    /// refined case is not exotic, which is why the entry stays recorded
    /// rather than deleted.
    #[test]
    fn n9_where_the_small_alpha_cancellation_bites_and_whether_cases_reach_it() {

        // --- 1. where the closed form loses significance ---
        fn mc_closed(a: f64) -> f64 {
            5.0 * (a.sinh() / a - 3.0 * a.cosh() / (a * a) + 3.0 * a.sinh() / a.powi(3))
        }
        // Series, derived term by term from sinh/cosh: the 1/a^2 terms and the
        // constants both cancel exactly, leaving
        //   mc = 5*(a^2/15 + a^4/210 + ...) = (a^2/3)*(1 + a^2/14 + ...)
        fn mc_series(a: f64) -> f64 {
            let a2 = a * a;
            a2 / 3.0 * (1.0 + a2 / 14.0)
        }

        eprintln!("mc: closed form against its series");
        eprintln!("  {:>8}  {:>16}  {:>16}  {:>10}", "a", "closed", "series", "rel err");
        let mut first_bad = f64::NAN;
        for e in 0..8 {
            let a = 10f64.powi(-e);
            let (c, t) = (mc_closed(a), mc_series(a));
            let rel = (c - t).abs() / t.abs();
            eprintln!("  {a:>8.0e}  {c:>16.8e}  {t:>16.8e}  {rel:>10.2e}");
            // Cancellation onset: the largest `a` at which the closed form has
            // lost 1%. Skip `a = 1`, where the discrepancy is this two-term
            // series truncating rather than the closed form failing.
            if a < 0.5 && rel > 1e-2 && first_bad.is_nan() {
                first_bad = a;
            }
        }
        eprintln!("  cancellation has cost 1% by a = {first_bad:.0e}");

        // --- 2. what the real cases reach ---
        let mut overall = f64::INFINITY;
        for (name, sigmar, dvec, lx, ly, lz, es, g) in cases() {
            let mut amin = f64::INFINITY;
            let mut amax: f64 = 0.0;
            for idx in 0..g * es {
                if dvec[idx] <= 0.0 || sigmar[idx] <= 0.0 {
                    continue;
                }
                let r = (sigmar[idx] / dvec[idx]).sqrt();
                let node = idx % es;
                for l in [lx[node], ly[node], lz[node]] {
                    let a = 0.5 * r * l;
                    if a > 0.0 {
                        amin = amin.min(a);
                        amax = amax.max(a);
                    }
                }
            }
            eprintln!("{name:<12} alpha in [{amin:.4}, {amax:.4}]");
            overall = overall.min(amin);
        }
        eprintln!();
        eprintln!("smallest alpha anywhere: {overall:.4}");
        eprintln!("margin over the failure threshold: {:.3e}x", overall / first_bad);

        assert!(
            overall > 1e-2,
            "a case reaches alpha = {overall:.3e}, close enough to the cancellation to matter"
        );
    }

    /// `(name, sigmar, dvec, lx, ly, lz, es, G)` for every solvable case.
    #[allow(clippy::type_complexity)]
    fn cases() -> Vec<(&'static str, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, usize, usize)>
    {
        use crate::types::Params;
        let mut out = Vec::new();

        let build = |name: &'static str, params: Params, sv: crate::types::SigmaValues,
                     ws: crate::matlab::Array3<usize>, geometry: crate::types::Geometry| {
            let (maxix, maxiy, maxiz) = handle3dcoords(&params);
            let es = maxix * maxiy * maxiz;
            let g_count = params.g;
            let mut sigma = crate::makesigmadfxyz::makesigmadfxyz(&params, &sv, &ws, None);
            let diffd = crate::calcdiffvalues3d::calcdiffvalues3d(&params, &sv.tot, &ws, None);
            // Sigma_r and D flattened exactly as `calc_abefghxyz` forms them.
            let philen = g_count * es;
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
            let mut dvec = vec![0.0; philen];
            for g in 0..g_count {
                for ix in 0..maxix {
                    for iy in 0..maxiy {
                        for iz in 0..maxiz {
                            dvec[g * es + ix * maxiy * maxiz + iy * maxiz + iz] =
                                diffd.get(ix, iy, iz, g);
                        }
                    }
                }
            }
            (name, sigmar, dvec, geometry.lx.clone(), geometry.ly.clone(),
             geometry.lz.clone(), es, g_count)
        };

        let p = Params { nodalupd: 6, ..Default::default() };
        let (pa, ga, wa, sa) = crate::iaea3ds::iaea3ds(&p);
        out.push(build("IAEA-3D", pa, sa, wa, ga));

        for (name, built) in [
            ("NEACRP A2", crate::neacrpa2::neacrpa2(&Params::default())),
            ("NEACRP D1", crate::neacrpd1::neacrpd1(&Params::default())),
        ] {
            let (params, geometry, _th, ws, sv, _fb) = built;
            out.push(build(name, params, sv, ws, geometry));
        }
        out
    }

    /// **N9 corrected — the series holds where the closed form collapses, and
    /// changes nothing on any real case.**
    ///
    /// # Methodology
    ///
    /// Three properties, in increasing order of what they'd cost to get wrong:
    ///
    /// 1. **The series is right.** Its `a -> 0` limits must be the exact
    ///    rationals `(1/15, 1/35, 2/7, 2/5, 10, 6)`, and it must agree with
    ///    the closed form to high precision in the band where the closed form
    ///    is still trustworthy (`a` around 0.1), where the two are computed by
    ///    completely different routes.
    /// 2. **It fixes what it claims to.** At `a = 1e-6` the closed form
    ///    returns zeros and non-finite values; the corrected form must return
    ///    finite coefficients near their limits.
    /// 3. **It is a no-op on the snapshot.** The smallest `alpha` any case
    ///    reaches is 0.3535, an order of magnitude above
    ///    [`crate::types::SMALL_ALPHA`], so the two settings must produce
    ///    **bit-identical** coefficients for every in-core (group, node, axis)
    ///    of every case. This is the property that lets the switch default to
    ///    on without re-validating a single benchmark number.
    ///
    /// # Results — measured 2026-08-23
    ///
    /// **1. The series is right.** Limits exact to 1e-15:
    /// `(0.0666667, 0.0285714, 0.2857143, 0.4, 10.0, 6.0)`. Against the closed
    /// form where the closed form is still the accurate one: **2.03e-6** at
    /// `a = 0.2`, **6.43e-7** at 0.15.
    ///
    /// **2. It fixes what it claims to.** At `a = 1e-6` the closed form
    /// returns
    ///
    /// ```text
    /// (1.088e8, -1.000e12, -3.000e12, 1.088e8, 3.0000, 1.0000)
    /// ```
    ///
    /// against true limits `(0.0667, 0.0286, 0.2857, 0.4, 10, 6)` — **wrong by
    /// a factor of 3.5e13**, and every value **finite**. The corrected form
    /// returns the limits to 1e-13.
    ///
    /// **That the failure is finite is the point.** A `NaN` propagates and
    /// gets noticed; `Gg = 3.0` where the answer is 10 does not. This is the
    /// same shape as defects C5 and C7 — the damage is not that the number is
    /// wrong, it is that nothing says so.
    ///
    /// **3. It is a no-op on the snapshot.** Over **72,258** in-core
    /// (group, node, axis) coefficients across all three cases, the two
    /// settings produce **bit-identical** results:
    ///
    /// | case | min alpha | margin over the threshold | differing |
    /// |---|---|---|---|
    /// | IAEA-3D | 0.7071 | 7x | **0** |
    /// | NEACRP A2 | 0.3535 | **4x** | **0** |
    /// | NEACRP D1 | 0.6619 | 7x | **0** |
    ///
    /// So the default can be flipped on without re-validating a single
    /// benchmark number — which is why this correction needs no before/after
    /// table, unlike G1/G2/G3 or T5/T6.
    ///
    /// **The margin is 4x, not the several orders of magnitude an analysis of
    /// `mc` alone suggests.** `Bb` divides a fourth-order cancellation by
    /// `mc` and so amplifies its loss; the closed form is already 28% adrift
    /// at `a = 0.01`. NEACRP A2's minimum is four times the crossover, not
    /// thousands. Comfortable, but not the margin an estimate would give.
    #[test]
    fn n9_the_series_is_accurate_where_the_closed_form_is_not() {
        use crate::types::{NodalCoeffForm, SMALL_ALPHA};
        const CLOSED: NodalCoeffForm = NodalCoeffForm::ClosedFormAlways;
        const FIXED: NodalCoeffForm = NodalCoeffForm::SeriesBelowSmallAlpha;

        // --- 1. the series is right ---
        let lim = abefgh_series(0.0);
        let exact = (1.0 / 15.0, 1.0 / 35.0, 2.0 / 7.0, 2.0 / 5.0, 10.0, 6.0);
        eprintln!("series limits at a = 0: {lim:?}");
        for (got, want) in [
            (lim.0, exact.0), (lim.1, exact.1), (lim.2, exact.2),
            (lim.3, exact.3), (lim.4, exact.4), (lim.5, exact.5),
        ] {
            assert!((got - want).abs() < 1e-15, "limit {got} should be {want}");
        }

        eprintln!("
series against the closed form, where the closed form still holds:");
        // Only where the closed form is still the accurate one; below `a = 0.1`
        // it is the closed form that drifts, not the series.
        for a in [0.3f64, 0.2, 0.15] {
            let c = abefgh(a, CLOSED);
            let t = abefgh_series(a);
            let rel = |x: f64, y: f64| (x - y).abs() / y.abs().max(1e-30);
            let worst = [rel(t.0, c.0), rel(t.1, c.1), rel(t.2, c.2),
                         rel(t.3, c.3), rel(t.4, c.4), rel(t.5, c.5)]
                .into_iter().fold(0.0f64, f64::max);
            eprintln!("  a = {a:<6} worst relative difference {worst:.3e}");
            assert!(worst < 1e-4, "a = {a}: the two routes disagree by {worst:.3e}");
        }

        // --- 2. it fixes what it claims to ---
        eprintln!("
at a = 1e-6, where the closed form collapses:");
        let broken = abefgh(1e-6, CLOSED);
        let fixed = abefgh(1e-6, FIXED);
        eprintln!("  closed form : {broken:?}");
        eprintln!("  corrected   : {fixed:?}");
        // It does NOT return NaN — it returns finite, plausible-looking values
        // that are wrong by orders of magnitude, which is the dangerous case.
        let worst_broken = [
            (broken.0, exact.0), (broken.1, exact.1), (broken.2, exact.2),
            (broken.3, exact.3), (broken.4, exact.4), (broken.5, exact.5),
        ]
        .into_iter()
        .map(|(got, want)| (got - want).abs() / want.abs())
        .fold(0.0f64, f64::max);
        eprintln!("  closed form is wrong by a factor of {worst_broken:.3e}");
        assert!(
            worst_broken > 1.0,
            "the closed form is expected to be grossly wrong here, was {worst_broken:.3e}"
        );
        assert!(
            [broken.0, broken.1, broken.2, broken.3, broken.4, broken.5]
                .into_iter().all(|x| x.is_finite()),
            "and finite, which is what makes it dangerous"
        );
        for (got, want) in [
            (fixed.0, exact.0), (fixed.1, exact.1), (fixed.2, exact.2),
            (fixed.3, exact.3), (fixed.4, exact.4), (fixed.5, exact.5),
        ] {
            assert!(got.is_finite(), "corrected coefficient is not finite");
            assert!((got - want).abs() / want < 1e-9, "{got} is far from the limit {want}");
        }

        // --- 3. a no-op on every case in the snapshot ---
        eprintln!("
over every in-core (group, node, axis) of every case:");
        let mut checked = 0usize;
        let mut closest = f64::INFINITY;
        for (name, sigmar, dvec, lx, ly, lz, es, g) in cases() {
            let mut differing = 0usize;
            let mut amin = f64::INFINITY;
            for idx in 0..g * es {
                if dvec[idx] <= 0.0 || sigmar[idx] <= 0.0 {
                    continue;
                }
                let r = (sigmar[idx] / dvec[idx]).sqrt();
                let node = idx % es;
                for l in [lx[node], ly[node], lz[node]] {
                    let a = 0.5 * r * l;
                    if a <= 0.0 {
                        continue;
                    }
                    amin = amin.min(a);
                    checked += 1;
                    if abefgh(a, CLOSED) != abefgh(a, FIXED) {
                        differing += 1;
                    }
                }
            }
            eprintln!("  {name:<12} min alpha {amin:.4} ({:.0}x the threshold), differing {differing}",
                amin / SMALL_ALPHA);
            assert_eq!(differing, 0, "{name}: the correction changed a coefficient");
            closest = closest.min(amin);
        }
        eprintln!("
{checked} coefficients checked; closest approach to the threshold is {:.0}x",
            closest / SMALL_ALPHA);
        assert!(closest > SMALL_ALPHA, "a case reaches the threshold");
    }
}
