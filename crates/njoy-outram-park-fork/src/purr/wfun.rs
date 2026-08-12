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
    // identical to uw's.
    let use_taylor = if abrez + BRK1 * aimz - BRK2 > 0.0 {
        false
    } else if abrez + BRK3 * aimz - BRK4 > 0.0 {
        false
    } else if r2 + BRK5 * ai2 - BRK6 < 0.0 {
        true
    } else if r2 + BRK7 * ai2 - BRK8 >= 0.0 {
        true
    } else {
        aimz - BRK9 >= 0.0
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

        // purr.f90:2052-2060 — y-side grid coordinate (j = jj-3 for the
        // coarse grid's y0=0.4 offset).
        let tempor_y = 10.0 * y;
        let jj = tempor_y as i64;
        let j = (jj - 3) as usize;
        let n = j - 1;
        let q = tempor_y - jj as f64;

        // purr.f90:2066-2069 — x-side grid coordinate (i = ii+2, shared by
        // both grids).
        let tempor_x = 10.0 * ax;
        let ii = tempor_x as i64;
        let i = (ii + 2) as usize;
        let p = tempor_x - ii as f64;

        Self::biquad(&self.tr_coarse, &self.ti_coarse, i, j, n, p, q, aki)
    }

    /// Look up `w(x, y)` on the fine grid (`y < 0.5`) — ported from
    /// `purr.f90:2092-2128`.
    pub fn lookup_fine(&self, x: f64, y: f64) -> (f64, f64) {
        let ax = x.abs();
        let aki = if x < 0.0 { -1.0 } else { 1.0 };

        // purr.f90:2093-2096 — y-side grid coordinate (j = jj+2, scale 50,
        // for the fine grid's y0=-0.02, step 0.02).
        let tempor_y = 50.0 * y;
        let jj = tempor_y as i64;
        let j = (jj + 2) as usize;
        let n = j - 1;
        let q = tempor_y - jj as f64;

        let tempor_x = 10.0 * ax;
        let ii = tempor_x as i64;
        let i = (ii + 2) as usize;
        let p = tempor_x - ii as f64;

        Self::biquad(&self.tr_fine, &self.ti_fine, i, j, n, p, q, aki)
    }
}

impl Default for DopplerTable {
    fn default() -> Self {
        Self::new()
    }
}
