//! `sigu` — the adaptively-linearised secondary-energy distribution at one
//! fixed scattering cosine `μ`, the building block of the `iform=1`
//! (continuous angle-energy, LAW=7) output format.
//!
//! Faithful port of NJOY2016 `thermr.f90::sigu` (lines 2874-2996). Walks the
//! signed beta grid outward from the elastic point (exactly like `calcem`'s
//! own `iform=0` outgoing-energy panel walk — see [`super::iform0`] — but
//! with `μ` fixed and `E'` as the abscissa instead of `μ` at fixed `E'`),
//! adaptively bisecting each panel against [`IncoherentInelastic::double_differential`]
//! (`sig` in the Fortran, reused here rather than reimplemented) until the
//! linear approximation is within tolerance.
//!
//! # The missing factor of 2 is intentional
//!
//! `sum` accumulates `(y(i)+yl)*(x(i)-xl)` with **no `half` factor** —
//! `thermr.f90:2969`, unlike every other trapezoid accumulation in this
//! module family. This is not a bug to "fix": NJOY's callers
//! (`thermr.f90:2367` `xsi(ie)=sum/2`, and `2404` `yu(2+2*ib)*2/sum`) apply
//! the missing `/2` themselves, so the factor cancels out down the line. This
//! port preserves the same convention — [`SiguResult::sum2`] is **twice** the
//! actual `∫σ dE'` at fixed `μ`, and callers must divide by 2 (or use it
//! directly as a normalisation denominator the way `thermr.f90:2404` does,
//! where the stray factor of 2 in the numerator cancels it).

use super::super::mf7::IncoherentInelastic;

/// `sigu`'s `imax`: bisection-stack depth cap, same constant as `sigl`.
const IMAX: usize = 20;
/// `sigu`'s `tolmin`: skip subdividing a panel whose trapezoid area already
/// undercuts this.
const TOLMIN: f64 = 1.0e-6;
/// `sigu`'s `bmax`: stop walking the beta grid outward once `β > bmax`.
const BMAX: f64 = 20.0;
/// `sigu`'s `nemax` cap on stored points (`calcem`'s `nemax` parameter,
/// `thermr.f90:1577`), used only as the same safety valve NJOY uses
/// (`j >= nemax-1`) — this port grows [`SiguResult::points`] with a `Vec`
/// rather than a fixed `s(2*nemax)` array, so the cap never truncates data
/// that would otherwise fit; it only reproduces NJOY's own early-stop rule.
const NEMAX: usize = 5000;

/// Reference temperature `kT₀ = 0.0253 eV`, matching [`super::sigl::TEVZ`].
const TEVZ: f64 = 0.0253;

/// Result of one [`sigu`] call at fixed `(e, u)`.
#[derive(Debug, Clone)]
pub struct SiguResult {
    /// **Twice** `∫σ(E→E')dE'` at fixed `μ = u` \[barn\] — NJOY's `s(1)`. See
    /// the module docs for why the factor of 2 is not divided out here.
    pub sum2: f64,
    /// `(E' \[eV\], σ(E→E',μ) \[barn/eV... strictly d²σ/dE'dμ\])` pairs,
    /// adaptively linearised, ascending in `E'`. NJOY's `s(3), s(5), …` /
    /// `s(4), s(6), …`; `s(2) = len(points)` is just `points.len()` here.
    pub points: Vec<(f64, f64)>,
}

/// Where [`label_160`] sends control after processing one accepted point —
/// mirrors the three `go to` targets thermr.f90's label 160 can take
/// (150/111/170).
enum Action {
    /// `go to 150`: keep bisecting the current panel at the (already
    /// decremented) `i`.
    Bisect,
    /// `go to 111`: this panel's stack is exhausted; set up the next one.
    Panels,
    /// `go to 170`: done — the beta grid (or the `nemax`/`bmax` safety cap)
    /// is exhausted.
    Finalize,
}

/// `sigu`'s label 160 body **in full**, including its trailing
/// `i`-decrement / `jbeta`-advance / goto decision (`thermr.f90:2960-2989`).
/// Written as its own function (rather than inlined at the loop's "accept"
/// site) because the Fortran jumps into this same code a *second* way — "if
/// (i.eq.1) go to 160" after the beta grid is exhausted at the collapsed
/// `i == 1` (`thermr.f90:2989`) — which this port models as one bounded
/// recursive call (it terminates after exactly one extra invocation: the
/// second call always decrements `i` to `0`, which cannot satisfy `i == 1`
/// again).
#[allow(clippy::too_many_arguments)]
fn label_160(
    i: &mut usize,
    x: &[f64],
    y: &[f64],
    jbeta: &mut i32,
    nbeta: i32,
    beta: &[f64],
    j: &mut usize,
    points: &mut Vec<(f64, f64)>,
    sum: &mut f64,
    xl: &mut f64,
    yl: &mut f64,
) -> Action {
    *j += 1;
    points.push((x[*i], y[*i]));
    if *j > 1 {
        *sum += (y[*i] + *yl) * (x[*i] - *xl); // NOTE: no `half` factor — see module docs.
    }
    *xl = x[*i];
    *yl = y[*i];
    if *j >= NEMAX - 1 {
        return Action::Finalize;
    }
    if *jbeta > 0 {
        let bj = beta[(*jbeta as usize) - 1];
        if bj > BMAX {
            return Action::Finalize;
        }
    }
    *i -= 1;
    if *i > 1 {
        return Action::Bisect;
    }
    *jbeta += 1;
    if *jbeta <= nbeta {
        return Action::Panels;
    }
    if *i == 1 {
        return label_160(i, x, y, jbeta, nbeta, beta, j, points, sum, xl, yl);
    }
    Action::Finalize
}

/// `sigu(ii, e, u, natom, tolin)` — the continuous secondary-energy
/// distribution `σ(E→E', μ=u)` \[barn\] at incident energy `e` \[eV\] and
/// fixed scattering cosine `u`, for the `iform=1` output format.
///
/// `natom` is the number of principal-scatterer atoms per molecule; `tolin`
/// is `calcem`'s reconstruction tolerance (used directly, unhalved — unlike
/// `sigl`, `sigu` does not halve `tol`, matching `thermr.f90:2894`).
pub fn sigu(ii: &IncoherentInelastic, e: f64, u: f64, natom: f64, tolin: f64) -> SiguResult {
    let az = ii.mass_ratio();
    let temp_k = ii.temperature_k;
    let tev = crate::common::phys::BK_EV_PER_K * temp_k;
    let lat = ii.lat;
    let lasym = ii.lasym;
    let beta = &ii.beta;
    let nbeta = beta.len() as i32;
    let sig = |ep: f64| ii.double_differential(e, ep, u, temp_k, natom);
    let sigfig = crate::mixr::mix::sigfig;

    let tol = tolin;

    // NJOY also computes `root2 = (u*sqrt(e) - sqrt(...)) / (az+1)`
    // (thermr.f90:2904) but never reads it again anywhere in `sigu` — dead
    // code in the original, so it is not ported.
    let root1 = (u * e.sqrt() + (u * u * e + (az - 1.0) * (az + 1.0) * e).sqrt()) / (az + 1.0);

    let mut x = [0.0f64; IMAX + 1];
    let mut y = [0.0f64; IMAX + 1];
    let mut sum = 0.0f64;
    x[1] = 0.0;
    y[1] = sig(x[1]);
    let mut jbeta: i32 = if lasym > 0 { 1 } else { -nbeta };
    let mut j: usize = 0;
    let mut xl = 0.0f64;
    let mut yl = 0.0f64;
    let mut points: Vec<(f64, f64)> = Vec::new();

    'panels: loop {
        // label 111
        x[2] = x[1];
        y[2] = y[1];
        // label 113: walk the beta grid outward until a candidate exceeds x[2].
        loop {
            if jbeta == 0 {
                jbeta = 1;
            }
            let cand = if jbeta <= 0 {
                let bj = beta[(-jbeta) as usize - 1];
                let mut v = if lat == 1 {
                    e - bj * TEVZ
                } else {
                    e - bj * tev
                };
                v = sigfig(v, 8, 0);
                if v == e {
                    v = sigfig(e, 8, -1);
                }
                v
            } else {
                let bj = beta[jbeta as usize - 1];
                if lat == 1 {
                    e + bj * TEVZ
                } else {
                    e + bj * tev
                }
            };
            x[1] = cand;
            if x[1] > x[2] {
                break;
            }
            jbeta += 1;
        }
        // label 116
        if u < 0.0 && root1 * root1 > 1.01 * x[2] && root1 * root1 < x[1] {
            x[1] = root1 * root1;
        }
        x[1] = sigfig(x[1], 8, 0);
        y[1] = sig(x[1]);
        let mut i: usize = 2;

        'bisect: loop {
            // label 150
            let accept = if i == IMAX {
                true
            } else if i > 3 && 0.5 * (y[i - 1] + y[i]) * (x[i - 1] - x[i]) < TOLMIN {
                true
            } else {
                let xm = sigfig(0.5 * (x[i - 1] + x[i]), 8, 0);
                if xm <= x[i] || xm >= x[i - 1] {
                    true
                } else {
                    let ym = 0.5 * (y[i - 1] + y[i]);
                    let yt = sig(xm);
                    let test = tol * yt.abs();
                    if (yt - ym).abs() <= test {
                        true
                    } else {
                        i += 1;
                        x[i] = x[i - 1];
                        y[i] = y[i - 1];
                        x[i - 1] = xm;
                        y[i - 1] = yt;
                        false
                    }
                }
            };
            if !accept {
                continue 'bisect;
            }
            match label_160(
                &mut i,
                &x,
                &y,
                &mut jbeta,
                nbeta,
                beta,
                &mut j,
                &mut points,
                &mut sum,
                &mut xl,
                &mut yl,
            ) {
                Action::Bisect => continue 'bisect,
                Action::Panels => continue 'panels,
                Action::Finalize => break 'panels,
            }
        }
    }

    SiguResult { sum2: sum, points }
}
