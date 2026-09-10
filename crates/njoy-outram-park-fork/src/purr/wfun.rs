//! PURR's complex probability integral evaluator (`uw2`) and the two-table
//! fast-lookup scheme (`uwtab2`) `unrest` uses to evaluate the Doppler line
//! shape at every Monte Carlo sample point without calling the exact
//! evaluator each time.
//!
//! [`uw2`] is ported from `purr.f90:2606-2781` — algorithmically identical to
//! [`crate::unresr::wfun::uw`] (same break-point regions, same asymptotic
//! continued-fraction / Taylor series, same `WRecurrence` step), reusing that
//! module's [`crate::unresr::wfun::WRecurrence`] rather than re-deriving the
//! delicate continued-fraction recurrence a second time. The one genuine
//! difference is documented on [`uw2`] itself.
//!
//! [`DopplerTable`] is ported from `uwtab2` (`purr.f90:2545-2604`) — two 41×27
//! grids (coarse, `y ∈ [0.4, 3.0]`; fine, `y ∈ [-0.02, 0.5]`), built from
//! [`uw2`] exactly the way [`crate::unresr::wfun::WTable::new`] builds its
//! single grid from `uw`. [`DopplerTable::lookup_coarse`] /
//! [`DopplerTable::lookup_fine`] reuse the identical 6-point biquadratic
//! interpolation formula as [`crate::unresr::wfun::WTable::lookup`] (same
//! `a1..a5`/`pq` weights), only the grid-index mapping differs per table.

use crate::unresr::wfun::WRecurrence;

const EPS: f64 = 1.0e-7;

/// `w(z) = e^{-z²}·erfc(-iz)`, the complex probability integral (Faddeeva
/// function), ported from `uw2` (`purr.f90:2606-2781`).
///
/// **The one difference from [`crate::unresr::wfun::uw`]:** once the real
/// part `Re(w)` has converged, if `Re(z) == 0` exactly, the imaginary part is
/// forced to `0.0` and returned immediately rather than also iterating its
/// own convergence check (`purr.f90:2693`, `2736`) — an exactness shortcut
/// for evaluating on the purely-imaginary axis (where `Im(w)` is analytically
/// zero) that `uw` does not have. Ported as the same shortcut, not unified
/// away, since it is a genuine (if small) behavioural difference between the
/// two upstream routines.
pub fn uw2(rez: f64, aim1: f64) -> (f64, f64) {
    let aimz = aim1.abs();
    let abrez = rez.abs();
    if abrez + aimz == 0.0 {
        return (1.0, 0.0);
    }

    let r2 = rez * rez;
    let ai2 = aimz * aimz;

    const BRK1: f64 = 1.25;
    const BRK2: f64 = 5.0;
    const BRK3: f64 = 1.863_636;
    const BRK4: f64 = 4.1;
    const BRK5: f64 = 1.71;
    const BRK6: f64 = 2.89;
    const BRK7: f64 = 1.18;
    const BRK8: f64 = 5.76;
    const BRK9: f64 = 1.5;

    // purr.f90:2648-2652 — region selection (kw=1 asymptotic, kw=2 taylor),
    // identical to uw's. The final arm is the FALL-THROUGH into label 340
    // (Taylor) when `aimz-brk9.ge.zero` is false — see the annotated listing
    // in [`crate::unresr::wfun::uw`]; this arm was inverted until 2026-09-10.
    let use_taylor = if abrez + BRK1 * aimz - BRK2 > 0.0 {
        false
    } else if abrez + BRK3 * aimz - BRK4 > 0.0 {
        false
    } else if r2 + BRK5 * ai2 - BRK6 < 0.0 {
        true
    } else if r2 + BRK7 * ai2 - BRK8 >= 0.0 {
        true
    } else {
        aimz - BRK9 < 0.0
    };

    if use_taylor {
        w_taylor2(rez, aim1, r2, ai2)
    } else if aim1 >= 0.0 {
        w_asymptotic2(rez, aimz, r2, ai2)
    } else {
        w_taylor2(rez, aim1, r2, ai2)
    }
}

/// Asymptotic-series branch with the `rez==0` shortcut — `purr.f90:2664-2695`
/// (labels 370-390).
fn w_asymptotic2(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    let (mut state, ak, rv) = WRecurrence::init_asymptotic(rez, aimz, r2, ai2);

    let mut aak: f64 = 1.0;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let ajtemp = 2.0 * aak;
        let temp4 = (1.0 - ajtemp) * ajtemp;
        let ajp = rv - (4.0 * aak + 1.0);
        state.step(ajp, temp4, ak);
        aak += 1.0;

        let (pr, pim) = (rew, aimw);
        let (new_rew, new_aimw) = state.ratio();
        rew = new_rew;
        aimw = new_aimw;
        if (rew - pr).abs() < EPS {
            // purr.f90:2693 — exactness shortcut, absent in `uw`.
            if rez == 0.0 {
                return (rew, 0.0);
            }
            if (aimw - pim).abs() < EPS {
                return (rew, aimw);
            }
        }
    }
}

/// Taylor-series branch with the `rez==0` shortcut — `purr.f90:2697-2741`
/// (labels 420-440).
fn w_taylor2(rez: f64, aimz: f64, r2: f64, ai2: f64) -> (f64, f64) {
    const C2: f64 = 1.5;
    let rpi = std::f64::consts::PI.sqrt();

    let temp1 = r2 + ai2;
    let temp2 = 2.0 * temp1 * temp1;
    let aj0 = -(r2 - ai2) / temp2;
    let ak = 2.0 * rez * aimz / temp2;

    let mut state = WRecurrence::init_taylor();

    let expon = (temp2 * aj0).exp();
    let expc = expon * (temp2 * ak).cos();
    let exps = -expon * (temp2 * ak).sin();

    let mut ajsig: f64 = 0.0;
    let mut sig2p: f64 = 2.0 * C2;
    let (mut rew, mut aimw) = (0.0, 0.0);
    loop {
        let aj4sig = 4.0 * ajsig;
        let aj4sm1 = aj4sig - 1.0;
        let temp3 = 1.0 / (aj4sm1 * (aj4sig + 3.0));
        let tt4 = sig2p * (2.0 * ajsig - 1.0);
        let temp4 = tt4 / (aj4sm1 * (aj4sig + 1.0) * (aj4sig - 3.0) * aj4sm1);
        let ajp = aj0 + temp3;
        state.step(ajp, temp4, ak);

        ajsig += 1.0;
        let temp7 = rpi * (state.am_el_mag_squared());
        let (c, d, am, el) = state.raw_cd_am_el();
        let ref_ = (aimz * (c * am + d * el) - rez * (am * d - c * el)) / temp7 / temp1;
        let aimf = (aimz * (am * d - c * el) + rez * (c * am + d * el)) / temp7 / temp1;

        let (pr, pim) = (rew, aimw);
        rew = expc - ref_;
        aimw = exps - aimf;
        if (rew - pr).abs() < EPS {
            // purr.f90:2736 — exactness shortcut, absent in `uw`.
            if rez == 0.0 {
                return (rew, 0.0);
            }
            if (aimw - pim).abs() < EPS {
                return (rew, aimw);
            }
        }
        sig2p = 2.0 * ajsig;
    }
}

/// The two precomputed 41×27 `w(z)` grids `unrest` looks up for its
/// small-`|x|`, small-`y` regime (`|x| ≤ 3.9`, `y ≤ 3.0`) — ported from
/// `uwtab2` (`purr.f90:2545-2604`).
///
/// Two grids, not one, because the *y*-resolution needed differs by an order
/// of magnitude depending on the Doppler width regime: **coarse**
/// (`y ∈ [0.4, 3.0]`, step `0.1`) for `y ≥ 0.5`, and **fine**
/// (`y ∈ [-0.02, 0.5]`, step `0.02`) for `y < 0.5`, where `w(z)` varies much
/// faster with `y`. Both share the same *x*-grid (`x ∈ [-0.1, 3.9]`, step
/// `0.1`, 41 points — sized exactly to the `|x| ≤ 3.9` classification range
/// this table is used for, see [`crate::purr::line_shape`]).
pub struct DopplerTable {
    /// `tr_coarse[i][j]` / `ti_coarse[i][j]` — Re/Im `w(x,y)` on the coarse
    /// grid, 0-indexed. `x[i] = -0.1 + i·0.1` (`i = 0..41`);
    /// `y[j] = 0.4 + j·0.1` (`j = 0..27`), except `y[0]` mirrors `y[2]` with
    /// negated imaginary part (`purr.f90:2578-2582`; the coarse grid's first
    /// row is a reflection, not a real sample — see [`Self::new`]).
    tr_coarse: Vec<Vec<f64>>,
    ti_coarse: Vec<Vec<f64>>,
    /// Same layout as the coarse grid, but `y[j] = -0.02 + j·0.02`
    /// (`j = 0..27`).
    tr_fine: Vec<Vec<f64>>,
    ti_fine: Vec<Vec<f64>>,
}

impl DopplerTable {
    const NX: usize = 41;
    const NY: usize = 27;

    /// Build both grids (`uwtab2`, `purr.f90:2545-2604`).
    pub fn new() -> Self {
        // x[i] = -0.1 + i*0.1, i = 0..41 (both grids share this axis).
        let x_at = |i: usize| -0.1 + i as f64 * 0.1;

        // Coarse grid: y[j] = 0.4 + j*0.1, j = 0..27.
        let mut tr_coarse = vec![vec![0.0; Self::NY]; Self::NX];
        let mut ti_coarse = vec![vec![0.0; Self::NY]; Self::NX];
        for i in 1..Self::NX {
            let x = x_at(i);
            for j in 0..Self::NY {
                let y = 0.4 + j as f64 * 0.1;
                let (rew, aimw) = uw2(x, y);
                tr_coarse[i][j] = rew;
                ti_coarse[i][j] = aimw;
            }
        }
        // purr.f90:2578-2582 — row 0 (x=-0.1) mirrors row 2 (x=0.1) with
        // negated imaginary part (w(-x+iy) symmetry), row 1's imaginary part
        // (x=0 exactly) is forced to zero.
        for j in 0..Self::NY {
            tr_coarse[0][j] = tr_coarse[2][j];
            ti_coarse[0][j] = -ti_coarse[2][j];
            ti_coarse[1][j] = 0.0;
        }

        // Fine grid: y[j] = -0.02 + j*0.02, j = 0..27.
        let mut tr_fine = vec![vec![0.0; Self::NY]; Self::NX];
        let mut ti_fine = vec![vec![0.0; Self::NY]; Self::NX];
        for i in 1..Self::NX {
            let x = x_at(i);
            for j in 0..Self::NY {
                let y = -0.02 + j as f64 * 0.02;
                let (rew, aimw) = uw2(x, y);
                tr_fine[i][j] = rew;
                ti_fine[i][j] = aimw;
            }
        }
        for j in 0..Self::NY {
            tr_fine[0][j] = tr_fine[2][j];
            ti_fine[0][j] = -ti_fine[2][j];
            ti_fine[1][j] = 0.0;
        }

        DopplerTable {
            tr_coarse,
            ti_coarse,
            tr_fine,
            ti_fine,
        }
    }

    /// 6-point biquadratic interpolation shared by both grids — the same
    /// formula as [`crate::unresr::wfun::WTable::lookup`]'s inner stencil
    /// (`unresr.f90`'s `quikw`), parameterised over which grid and which
    /// (already-computed) integer/fractional grid coordinates to use.
    ///
    /// `p, q` are the fractional parts of the (scaled) `x`/`y` coordinates
    /// within their grid cell; `i, j, n` are the 1-indexed-equivalent grid
    /// indices (`i` for `x`, `j`/`n=j-1` for `y`) already adjusted for each
    /// grid's own offset convention (see [`Self::lookup_coarse`] /
    /// [`Self::lookup_fine`]).
    #[allow(clippy::too_many_arguments)]
    fn biquad(
        tr: &[Vec<f64>],
        ti: &[Vec<f64>],
        i: usize,
        j: usize,
        n: usize,
        p: f64,
        q: f64,
        aki: f64,
    ) -> (f64, f64) {
        let p2 = p * p;
        let q2 = q * q;
        let hp = 0.5 * p;
        let hq = 0.5 * q;
        let hp2 = 0.5 * p2;
        let hq2 = 0.5 * q2;
        let pq = p * q;
        let a1 = hq2 - hq;
        let a2 = hp2 - hp;
        let a3 = 1.0 + pq - p2 - q2;
        let a4 = hp2 - pq + hp;
        let a5 = hq2 - pq + hq;

        let rew = a1 * tr[i][n]
            + a2 * tr[i - 1][j]
            + a3 * tr[i][j]
            + a4 * tr[i + 1][j]
            + a5 * tr[i][j + 1]
            + pq * tr[i + 1][j + 1];
        let mut aimw = a1 * ti[i][n]
            + a2 * ti[i - 1][j]
            + a3 * ti[i][j]
            + a4 * ti[i + 1][j]
            + a5 * ti[i][j + 1]
            + pq * ti[i + 1][j + 1];
        aimw *= aki;
        (rew, aimw)
    }

    /// Look up `w(x, y)` on the coarse grid (`y ≥ 0.5`) — ported from
    /// `purr.f90:2051-2088`. `x` may be negative; the sign is applied to the
    /// imaginary part after interpolating on `|x|` (`w(-x+iy)` symmetry).
    pub fn lookup_coarse(&self, x: f64, y: f64) -> (f64, f64) {
        let ax = x.abs();
        let aki = if x < 0.0 { -1.0 } else { 1.0 };

        // purr.f90:2052-2060 — y-side grid coordinate. Upstream: `j=jj-3`
        // into a 1-based array whose entry 1 is y=0.4 (so y=0.5 → jj=5 →
        // j=2). This port's rows are 0-based with the same origin (row 0 is
        // y=0.4), i.e. Fortran index k <-> our k-1, hence `jj-4`. (Found by
        // the `doppler_table_is_exact_at_its_own_nodes` test, 2026-09-10:
        // the literal `jj-3`/`ii+2` were one grid cell off in both axes.)
        let tempor_y = 10.0 * y;
        let jj = tempor_y as i64;
        let (j, q) = Self::clamp_row((jj - 4) as usize, tempor_y - jj as f64);
        let n = j - 1;

        let (i, p) = Self::x_coordinate(ax);
        Self::biquad(&self.tr_coarse, &self.ti_coarse, i, j, n, p, q, aki)
    }

    /// Look up `w(x, y)` on the fine grid (`y < 0.5`) — ported from
    /// `purr.f90:2092-2128`.
    pub fn lookup_fine(&self, x: f64, y: f64) -> (f64, f64) {
        let ax = x.abs();
        let aki = if x < 0.0 { -1.0 } else { 1.0 };

        // purr.f90:2093-2096 — y-side grid coordinate: upstream `j=jj+2`
        // (1-based, entry 2 is y=0), so `jj+1` here (see `lookup_coarse`).
        let tempor_y = 50.0 * y;
        let jj = tempor_y as i64;
        let (j, q) = Self::clamp_row((jj + 1) as usize, tempor_y - jj as f64);
        let n = j - 1;

        let (i, p) = Self::x_coordinate(ax);
        Self::biquad(&self.tr_fine, &self.ti_fine, i, j, n, p, q, aki)
    }

    /// x-side grid coordinate shared by both grids (`purr.f90:2066-2069`):
    /// upstream `i=ii+2` into a 1-based array whose entry 2 is x=0, so `ii+1`
    /// against this port's 0-based rows (row 1 is x=0).
    fn x_coordinate(ax: f64) -> (usize, f64) {
        let tempor_x = 10.0 * ax;
        let ii = tempor_x as i64;
        // The stencil reads column i+1. For |x| = 3.9 exactly (the table
        // tier's closed edge — `line_shape` sends |x| > 3.9 elsewhere)
        // upstream reads `tr(42,·)`, one past its 41 columns: an out-of-bounds
        // read Fortran does not check. Clamp to the last valid cell instead
        // (the quadratic then extrapolates by one step; the point is a set of
        // measure zero in the Monte Carlo).
        let mut i = (ii + 1) as usize;
        let mut p = tempor_x - ii as f64;
        if i > Self::NX - 2 {
            p += (i - (Self::NX - 2)) as f64; // keep the same physical x
            i = Self::NX - 2;
        }
        (i, p)
    }

    /// Clamp a y-row so the stencil's `j+1` stays inside the 27 rows. Only
    /// `y = 3.0` exactly on the coarse grid reaches this (upstream reads
    /// `tr(·,28)` there — the same out-of-bounds read as `x_coordinate`).
    fn clamp_row(j: usize, q: f64) -> (usize, f64) {
        if j > Self::NY - 2 {
            (Self::NY - 2, q + (j - (Self::NY - 2)) as f64)
        } else {
            (j.max(1), q)
        }
    }
}

impl Default for DopplerTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unresr::wfun::uw;

    /// `w(z)` at 16 points from a verbatim-`uw2` Fortran oracle: `uw2`
    /// (`purr.f90:2606-2781`) compiled unchanged with gfortran 13.3.0 on
    /// 2026-09-10 (`kr = selected_real_kind(12,300)`), driven by a 20-line
    /// program printing `(x, y, Re w, Im w)` to 17 significant figures. The
    /// points span every branch: the `Re(z)=0` shortcut, the Taylor region,
    /// the asymptotic region, the `brk*` break lines, a negative `x`, and
    /// `y = 0` (the pure Dawson-function edge).
    const UW2_ORACLE: [(f64, f64, f64, f64); 16] = [
        (0.0, 0.5, 6.1569034419458668e-01, 0.0),
        (0.3, 0.2, 7.5289479013687721e-01, 2.2965315234907427e-01),
        (1.0, 0.05, 3.7130529153152053e-01, 5.7164252979125396e-01),
        (2.0, 1.0, 1.4023958109725965e-01, 2.2221344043570285e-01),
        (0.0, 3.0, 1.7900115352231821e-01, 0.0),
        (1.5, 1.6, 1.9780568184586350e-01, 1.5377336487430945e-01),
        (3.0, 0.4, 3.0278754967723216e-02, 1.9573208858501706e-01),
        (4.0, 0.01, 3.9260791968046796e-04, 1.4595242271466977e-01),
        (6.0, 6.5, 4.7110591742681465e-02, 4.2935866991866405e-02),
        (10.0, 0.3, 1.7170131121579046e-03, 5.6653084897948486e-02),
        (50.0, 2.0, 4.5090031061670332e-04, 1.1268003276178750e-02),
        (150.0, 0.7, 1.7553353301890323e-05, 3.7612656681067507e-03),
        (0.5, 0.0, 7.7880078307140488e-01, 4.7892517290006542e-01),
        (-2.0, 1.2, 1.4654080316187895e-01, -1.9990385817383455e-01),
        (2.5, 2.9, 1.1381640052038888e-01, 9.1838041401174464e-02),
        (0.1, 120.0, 4.7014135033928116e-03, 3.9175725610314760e-06),
    ];

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1e-300) || (a - b).abs() <= 1e-300
    }

    #[test]
    fn uw2_matches_gfortran_oracle() {
        // Same arithmetic in the same order: agreement to a few ulps is the
        // expectation, 1e-12 the tolerance.
        for &(x, y, re, im) in &UW2_ORACLE {
            let (r, i) = uw2(x, y);
            assert!(close(r, re, 1e-12), "Re w({x},{y}): got {r:e}, oracle {re:e}");
            assert!(
                (i - im).abs() <= 1e-12 * im.abs().max(1e-12),
                "Im w({x},{y}): got {i:e}, oracle {im:e}"
            );
        }
        // Closed-form anchors independent of any NJOY code:
        // w(iy) = exp(y²)·erfc(y) → w(0.5i) = 0.6156903441945867.
        let (r, _) = uw2(0.0, 0.5);
        assert!(close(r, 0.615_690_344_194_586_7, 1e-9));
        // Re w(x) = exp(-x²) on the real axis.
        let (r, _) = uw2(0.5, 0.0);
        assert!(close(r, (-0.25f64).exp(), 1e-9));
    }

    #[test]
    fn uw2_agrees_with_unresr_uw_off_the_imaginary_axis() {
        // The two evaluators share their series; only the Re(z)=0 shortcut
        // differs (uw2 zeroes Im w exactly there). Everywhere else: identical
        // to round-off.
        let mut n = 0;
        for ix in 1..=40 {
            for iy in 0..=30 {
                let x = 0.1 * ix as f64;
                let y = 0.1 * iy as f64;
                let (r2, i2) = uw2(x, y);
                let (r1, i1) = uw(x, y);
                assert!(close(r2, r1, 1e-10), "Re at ({x},{y}): {r2:e} vs {r1:e}");
                assert!(close(i2, i1, 1e-10), "Im at ({x},{y}): {i2:e} vs {i1:e}");
                n += 1;
            }
        }
        assert!(n > 1000);
        // On the imaginary axis uw2 forces Im w = 0 (purr.f90:2693/2736).
        let (_, i2) = uw2(0.0, 1.3);
        assert_eq!(i2, 0.0);
    }

    #[test]
    fn doppler_table_is_exact_at_its_own_nodes() {
        // At a grid node p = q = 0, so the 6-point stencil collapses to the
        // node value itself: the lookup must return uw2 at that node to
        // round-off. This pins the index-base translation — the literal
        // Fortran `ii+2`/`jj-3`/`jj+2` (1-based) applied to 0-based rows
        // returned the neighbouring node (x+0.1, y+0.1) instead.
        let t = DopplerTable::new();
        for &(x, y) in &[(0.0, 0.5), (0.3, 0.7), (1.0, 1.0), (2.5, 2.9), (3.8, 0.5), (0.1, 2.0)] {
            let (r, i) = t.lookup_coarse(x, y);
            let (re, im) = uw2(x, y);
            assert!(close(r, re, 1e-9), "coarse Re at node ({x},{y}): {r:e} vs {re:e}");
            assert!(close(i, im, 1e-9), "coarse Im at node ({x},{y}): {i:e} vs {im:e}");
        }
        for &(x, y) in &[(0.0, 0.0), (0.3, 0.02), (1.0, 0.1), (2.5, 0.48), (3.8, 0.2), (0.1, 0.3)] {
            let (r, i) = t.lookup_fine(x, y);
            let (re, im) = uw2(x, y);
            assert!(close(r, re, 1e-9), "fine Re at node ({x},{y}): {r:e} vs {re:e}");
            assert!(close(i, im, 1e-9), "fine Im at node ({x},{y}): {i:e} vs {im:e}");
        }
        // Negative x: interpolate on |x|, flip the imaginary part.
        let (r, i) = t.lookup_coarse(-0.3, 0.7);
        let (re, im) = uw2(-0.3, 0.7);
        assert!(close(r, re, 1e-9) && close(i, im, 1e-9), "({r},{i}) vs ({re},{im})");
    }

    #[test]
    fn doppler_table_reproduces_uw2_to_biquadratic_interpolation_error() {
        // Off-node points across both grids, in the exact (|x| ≤ 3.9, y ≤ 3.0)
        // region `line_shape` routes to the table. Measured 2026-09-10 after
        // the index-base and break-line fixes: worst relative error 5.5e-3,
        // on the fine grid near y→0 where Im w is small and w varies fastest
        // in y; the bounds below carry margin over the measured values.
        let t = DopplerTable::new();
        let mut worst_c: f64 = 0.0;
        let mut worst_f: f64 = 0.0;
        for ix in 0..39 {
            for iy in 0..25 {
                let x = 0.1 * ix as f64 + 0.037;
                let yc = 0.5 + 0.1 * iy as f64 + 0.041;
                let (r, i) = t.lookup_coarse(x, yc);
                let (re, im) = uw2(x, yc);
                let e = ((r - re).abs() / re.abs()).max((i - im).abs() / im.abs().max(1e-3));
                worst_c = worst_c.max(e);
                let yf = 0.02 * iy as f64 + 0.007;
                let (r, i) = t.lookup_fine(x, yf);
                let (re, im) = uw2(x, yf);
                let e = ((r - re).abs() / re.abs()).max((i - im).abs() / im.abs().max(1e-3));
                worst_f = worst_f.max(e);
            }
        }
        assert!(worst_c < 1e-3, "coarse worst rel err {worst_c:e}");
        assert!(worst_f < 1e-2, "fine worst rel err {worst_f:e}");
    }

    #[test]
    fn doppler_table_tolerates_the_closed_tier_edges() {
        // |x| = 3.9 and y = 3.0 exactly: upstream reads one row/column past
        // its arrays; this port clamps and must neither panic nor return junk.
        let t = DopplerTable::new();
        for &(x, y) in &[(3.9, 3.0), (3.9, 0.49), (0.5, 3.0), (3.9, 0.0), (3.9, 2.95)] {
            let (r, i) = if y >= 0.5 { t.lookup_coarse(x, y) } else { t.lookup_fine(x, y) };
            let (re, im) = uw2(x, y);
            assert!(close(r, re, 5e-3) && (i - im).abs() <= 5e-3 * im.abs().max(1e-3),
                "edge ({x},{y}): ({r:e},{i:e}) vs ({re:e},{im:e})");
        }
    }
}
