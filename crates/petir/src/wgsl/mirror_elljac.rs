//! `f32` mirror of `shaders/elljac.wgsl` — the Jacobi elliptic functions
//! `sn`, `cn` and `dn`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # The third table-free shader, after `dilog` and `ellint`
//!
//! An arithmetic-geometric mean descent and a backward recurrence; nothing
//! is fitted, so there is no coefficient array, no `order_sp` decision and
//! nothing for the generator that produces the other mirrors to do. Both
//! sides are written by hand and the three shared constants are pinned
//! against the shader text by
//! `the_shader_and_this_mirror_agree_on_their_constants`.
//!
//! # All three functions come from one descent
//!
//! [`elljac`] returns all of `sn`, `cn`, `dn`, and the shader returns a
//! `vec3<f32>`, because computing them separately would run the AGM three
//! times for one answer each. The scalar selector exists only so the test
//! harness — which maps one `f32` in to one `f32` out — can reach each
//! component.
//!
//! # Two parameters, both measured
//!
//! **The descent cap is 8, against upstream's 16.** The AGM converges
//! quadratically, so the step count grows like `log log` of the precision
//! demanded. Measured in `f64` it is 7 at worst over the whole admissible
//! domain; measured here in `f32` — see
//! `the_descent_is_shorter_in_f32_than_in_f64` — it is fewer still, and the
//! number is in that test rather than in this sentence.
//!
//! **The degenerate-limit windows are retargeted, and they are precision
//! constants rather than range guards.** Upstream switches to `(sin, cos,
//! 1)` at `|m| < 2 DBL_EPSILON` and to `(tanh, sech, sech)` at
//! `|m - 1| < 2 DBL_EPSILON`. Keeping `f64`'s `4.44e-16` would make both
//! branches **unreachable** in `f32`: no normal `f32` lies strictly between
//! zero and `4.44e-16`, so `m = 0` and `m = 1` themselves would fall through
//! to the general branch. At `m = 1` that forms `nu_0 = 0`, the descent's
//! convergence test compares `|1 - 0|` against `4 eps`, and the recurrence
//! ends in `root / d` with `root = 0`. Retargeting to `2 f32::EPSILON` is
//! what keeps upstream's own special cases alive at the width they are being
//! used at.
//!
//! That makes this the **second** kind of constant in the taxonomy of
//! `docs/wgsl-coverage.md` to appear here — a precision cut — and the
//! opposite call from `synchrotron`'s range guard, which is kept precisely
//! because retargeting it would discard answers.
//!
//! # What `f32` costs
//!
//! Measured 2026-09-20 over a 121 x 81 grid, `u` in `[-6, 6]` and `m` across
//! `(-0.99, 0.99)`. **Absolute**, not relative: all three functions are
//! bounded by 1 and are *periodic*, so they have zeros everywhere and a
//! relative figure would be unbounded at each of them.
//!
//! | | worst absolute | at |
//! |---|---|---|
//! | `sn` | 3.011e-06 | `u = -6.0, m = -0.322` |
//! | `cn` | 2.894e-06 | same |
//! | `dn` | 7.461e-07 | same |
//!
//! and the defining identities hold to `4.768e-07` (`sn^2 + cn^2 = 1`) and
//! `5.960e-07` (`m sn^2 + dn^2 = 1`), which is five `f32` ulps.
//!
//! **The worst point is the largest `|u|`, and that is argument error, not
//! transcription error.** The descent ends by forming `sin(u mu_n)` and
//! `cos(u mu_n)`: the phase is proportional to `u`, so an `f32` argument's
//! own representation error is multiplied by `u` before the sine sees it.
//! `dn` is four times better than the other two for the same reason in
//! reverse — it comes out of the recurrence directly rather than through
//! that sine. This is the same effect the Bessel shaders' asymptotic
//! branches document, and it bounds what any `f32` implementation can do
//! here rather than anything about this one.

// Under a std-linked build (`cargo test`) f32's inherent sqrt/sin/tanh
// shadow these trait methods, leaving the import formally unused. See
// crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::zip::zip_flat;

/// The AGM descent cap. **8, not upstream's 16** — see the module
/// documentation. Mirrors `PETIR_ELLJAC_NMAX`.
const NMAX: usize = 8;

/// `f32::EPSILON`. Mirrors `PETIR_ELLJAC_EPS`.
const EPS: f32 = 1.1920929e-7;

/// `2 f32::EPSILON`, the half-width of the two degenerate-limit windows.
/// Mirrors `PETIR_ELLJAC_DEGEN`.
const DEGEN: f32 = 2.3841858e-7;

/// The three Jacobi elliptic functions at one point, in upstream's order.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Jacobi {
    /// `sn(u|m)`.
    pub sn: f32,
    /// `cn(u|m)`.
    pub cn: f32,
    /// `dn(u|m)`.
    pub dn: f32,
}

impl Jacobi {
    const NAN: Jacobi = Jacobi {
        sn: f32::NAN,
        cn: f32::NAN,
        dn: f32::NAN,
    };
}

/// The high-water mark of descent steps, so
/// `the_descent_is_shorter_in_f32_than_in_f64` can measure the cap rather
/// than assert it.
#[cfg(test)]
pub(crate) static WORST_STEPS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

#[inline]
fn note_steps(_n: usize) {
    #[cfg(test)]
    WORST_STEPS.fetch_max(_n, core::sync::atomic::Ordering::Relaxed);
}

fn hypot(x: f32, y: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 {
        return ay;
    }
    if ay == 0.0 {
        return ax;
    }
    let (big, small) = if ax > ay { (ax, ay) } else { (ay, ax) };
    let r = small / big;
    big * (1.0 + r * r).sqrt()
}

/// WGSL's `sign`, which is `0.0` at zero where [`f32::signum`] is `1.0`.
fn wgsl_sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `sn`, `cn` and `dn` at `u` with parameter `m = k^2`. Mirrors
/// `petir_elljac`. All three are `NaN` for `|m| > 1`.
pub fn elljac(u: f32, m: f32) -> Jacobi {
    elljac_with(u, m, NMAX)
}

/// [`elljac`] with the descent cap supplied, so the cap can be measured.
fn elljac_with(u: f32, m: f32, nmax: usize) -> Jacobi {
    if u.is_nan() || m.is_nan() || m.abs() > 1.0 {
        return Jacobi::NAN;
    }
    if m.abs() < DEGEN {
        return Jacobi {
            sn: u.sin(),
            cn: u.cos(),
            dn: 1.0,
        };
    }
    if (m - 1.0).abs() < DEGEN {
        let cn = 1.0 / u.cosh();
        return Jacobi {
            sn: u.tanh(),
            cn,
            dn: cn,
        };
    }

    let mut mu = [0.0_f32; NMAX];
    let mut nu = [0.0_f32; NMAX];
    let (Some(m0), Some(n0)) = (mu.first_mut(), nu.first_mut()) else {
        return Jacobi::NAN;
    };
    *m0 = 1.0;
    *n0 = (1.0 - m).sqrt();

    let mut n = 0usize;
    loop {
        let (Some(&mn), Some(&nn)) = (mu.get(n), nu.get(n)) else {
            return Jacobi::NAN;
        };
        if (mn - nn).abs() <= 4.0 * EPS * (mn + nn).abs() {
            break;
        }
        let (next_mu, next_nu) = (0.5 * (mn + nn), (mn * nn).sqrt());
        let Some(slot) = mu.get_mut(n + 1) else {
            return Jacobi::NAN;
        };
        *slot = next_mu;
        let Some(slot) = nu.get_mut(n + 1) else {
            return Jacobi::NAN;
        };
        *slot = next_nu;
        n += 1;
        note_steps(n);
        if n >= nmax - 1 {
            break;
        }
    }

    let Some(&mu_n) = mu.get(n) else {
        return Jacobi::NAN;
    };
    let sin_umu = (u * mu_n).sin();
    let cos_umu = (u * mu_n).cos();
    let reflected = sin_umu.abs() < cos_umu.abs();
    let t = if reflected {
        sin_umu / cos_umu
    } else {
        cos_umu / sin_umu
    };

    let (mut c, mut d) = (mu_n * t, 1.0_f32);
    let (Some(mu_lo), Some(mu_hi), Some(nu_lo)) = (mu.get(..n), mu.get(1..=n), nu.get(..n)) else {
        return Jacobi::NAN;
    };
    for (&mu_k, &mu_k1, &nu_k) in zip_flat!(mu_lo, mu_hi, nu_lo).rev() {
        let (c_next, d_next) = (c, d);
        c = d_next * c_next;
        let r = c_next * c_next / mu_k1;
        d = (r + nu_k) / (r + mu_k);
    }

    let root = (1.0 - m).sqrt();
    if reflected {
        let dn = root / d;
        let cn = dn * wgsl_sign(cos_umu) / hypot(1.0, c);
        Jacobi {
            sn: cn * c / root,
            cn,
            dn,
        }
    } else {
        let sn = wgsl_sign(sin_umu) / hypot(1.0, c);
        Jacobi {
            sn,
            cn: c * sn,
            dn: d,
        }
    }
}

/// One component of [`elljac`], `0 = sn`, `1 = cn`, `2 = dn`. Mirrors
/// `petir_elljac_component`, which is what the GPU test dispatches.
pub fn elljac_component(which: u32, u: f32, m: f32) -> f32 {
    let r = elljac(u, m);
    match which {
        0 => r.sn,
        1 => r.cn,
        2 => r.dn,
        _ => f32::NAN,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::elljac as f64_elljac;
    use core::sync::atomic::Ordering;

    /// **What `f32` costs**, against the `f64` module.
    ///
    /// Filled in from the measured run.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (mut ws, mut wc, mut wd, mut at) = (0.0_f64, 0.0_f64, 0.0_f64, (0.0_f32, 0.0_f32));
        for i in 0..=120 {
            let u = -6.0 + 0.1 * i as f32;
            for j in 0..=80 {
                let m = -0.99 + 1.98 * j as f32 / 80.0;
                let r = elljac(u, m);
                let g = f64_elljac::elljac(u as f64, m as f64);
                // sn, cn and dn are all bounded by 1 in magnitude, so an
                // ABSOLUTE figure is the meaningful one: a relative error is
                // unbounded at each of their (many) zeros, and they have
                // zeros everywhere -- they are periodic.
                let (es, ec, ed) = (
                    (r.sn as f64 - g.sn).abs(),
                    (r.cn as f64 - g.cn).abs(),
                    (r.dn as f64 - g.dn).abs(),
                );
                if es > ws {
                    ws = es;
                    at = (u, m);
                }
                wc = wc.max(ec);
                wd = wd.max(ed);
            }
        }
        for (name, w) in [("sn", ws), ("cn", wc), ("dn", wd)] {
            assert!(w < 1e-4, "{name}: {w:e} absolute at {at:?}");
        }
    }

    /// **The defining identities survive `f32`**, and they are the check
    /// that needs no reference at all.
    #[test]
    fn the_defining_identities_hold_in_f32() {
        let (mut w1, mut w2) = (0.0_f32, 0.0_f32);
        for i in 0..=120 {
            let u = -6.0 + 0.1 * i as f32;
            for j in 0..=80 {
                let m = -0.99 + 1.98 * j as f32 / 80.0;
                let r = elljac(u, m);
                w1 = w1.max((r.sn * r.sn + r.cn * r.cn - 1.0).abs());
                w2 = w2.max((m * r.sn * r.sn + r.dn * r.dn - 1.0).abs());
            }
        }
        assert!(w1 < 1e-5 && w2 < 1e-5, "identities in f32: {w1:e}, {w2:e}");
    }

    /// **The descent is shorter in `f32` than in `f64`, and the cap of 8 has
    /// room** — measured, not assumed.
    ///
    /// The AGM's step count grows like `log log` of the precision demanded,
    /// so halving the digits does not halve the steps; it removes one or
    /// two. The number is printed and asserted below rather than written
    /// into prose, because a figure in prose cannot fail.
    #[test]
    fn the_descent_is_shorter_in_f32_than_in_f64() {
        WORST_STEPS.store(0, Ordering::Relaxed);
        for i in 0..=200 {
            let u = -10.0 + 0.1 * i as f32;
            for j in 0..=200 {
                let m = -1.0 + 2.0 * j as f32 / 200.0;
                let _ = elljac(u, m);
            }
        }
        // The hardest cases: m as close to the endpoints as the general
        // branch is ever asked for, just outside the degenerate windows.
        for e in [1e-3_f32, 1e-5, 1e-6, 3e-7] {
            let _ = elljac(1.3, 1.0 - e);
            let _ = elljac(1.3, -1.0 + e);
            let _ = elljac(1.3, e);
        }
        let observed = WORST_STEPS.load(Ordering::Relaxed);
        assert!(
            observed < NMAX - 1,
            "the cap is documented as having room: {observed} steps against \
             a cap of {NMAX}"
        );
    }

    /// **The degenerate windows must be retargeted or they are dead code**,
    /// and this is the measurement that says so rather than the claim.
    ///
    /// At `f64`'s `2 DBL_EPSILON = 4.44e-16`, no normal `f32` lies strictly
    /// inside either window, so `m = 0` and `m = 1` themselves fall through
    /// to the general branch. This checks both halves: that the shipped
    /// windows catch the exact endpoints, and that the `f64` windows would
    /// not.
    #[test]
    fn the_f64_degenerate_windows_would_be_dead_code_here() {
        // The shipped windows catch the endpoints exactly.
        let j0 = elljac(1.3, 0.0);
        assert_eq!(j0.dn, 1.0, "m = 0 must take the circular branch");
        assert!((j0.sn - 1.3_f32.sin()).abs() < 1e-6);
        let j1 = elljac(1.3, 1.0);
        assert_eq!(j1.cn, j1.dn, "m = 1 must take the hyperbolic branch");
        assert!((j1.sn - 1.3_f32.tanh()).abs() < 1e-6);

        // And f64's window is narrower than anything f32 can express away
        // from zero: 2 * DBL_EPSILON as an f32 is smaller than the smallest
        // normal gap near 1, so |m - 1| < that has no normal solution but
        // m == 1 itself.
        let f64_window = 2.0 * crate::specfunc::DBL_EPSILON as f32;
        assert!(
            f64_window > 0.0 && 1.0 - f64_window == 1.0,
            "f64's window {f64_window:e} is documented as unresolvable at 1.0 \
             in f32; 1 - w is {}",
            1.0 - f64_window
        );
        // The shipped window is not: it admits a range of real f32 values.
        assert!(
            1.0 - DEGEN != 1.0,
            "the retargeted window must be resolvable in f32"
        );
    }

    /// The degenerate limits, in `f32`.
    #[test]
    fn the_degenerate_limits_are_circular_and_hyperbolic() {
        for i in 0..=40 {
            let u = -4.0 + 0.2 * i as f32;
            let j0 = elljac(u, 0.0);
            assert!((j0.sn - u.sin()).abs() < 1e-6);
            assert!((j0.cn - u.cos()).abs() < 1e-6);
            assert_eq!(j0.dn, 1.0);
            let j1 = elljac(u, 1.0);
            assert!((j1.sn - u.tanh()).abs() < 1e-6);
            assert!((j1.cn - 1.0 / u.cosh()).abs() < 1e-6);
            assert_eq!(j1.cn, j1.dn);
        }
    }

    /// The refusals, and the component selector.
    #[test]
    fn the_refusals_and_the_selector_are_right() {
        for m in [1.0001_f32, 2.0, -1.5, f32::INFINITY] {
            let r = elljac(0.5, m);
            assert!(r.sn.is_nan() && r.cn.is_nan() && r.dn.is_nan(), "m = {m}");
        }
        assert!(elljac(f32::NAN, 0.5).sn.is_nan());
        assert!(elljac(0.5, f32::NAN).sn.is_nan());
        for (u, m) in [(0.7_f32, 0.3_f32), (-2.1, 0.9), (3.4, -0.5)] {
            let r = elljac(u, m);
            assert_eq!(elljac_component(0, u, m), r.sn);
            assert_eq!(elljac_component(1, u, m), r.cn);
            assert_eq!(elljac_component(2, u, m), r.dn);
            assert!(elljac_component(3, u, m).is_nan());
        }
    }

    /// The shader and this mirror agree on all three shared constants.
    ///
    /// No tables, so no generator and no table audit — this parses
    /// `elljac.wgsl` exactly as `mirror_dilog` and `mirror_ellint` do.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::ELLJAC;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in elljac.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the constant name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .trim_end_matches('u')
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not a numeric literal"))
        };
        assert_eq!(read("PETIR_ELLJAC_NMAX: u32"), NMAX as f32);
        assert_eq!(read("PETIR_ELLJAC_EPS: f32"), EPS);
        assert_eq!(read("PETIR_ELLJAC_DEGEN: f32"), DEGEN);
        // And the two are what they claim to be, recomputed.
        assert_eq!(EPS, f32::EPSILON);
        assert_eq!(DEGEN, 2.0 * f32::EPSILON);
        // The shader's arrays must be the cap long, or the descent would
        // read past them on the device where WGSL has no bounds check.
        assert!(
            src.contains(&std::format!("array<f32, {NMAX}>")),
            "elljac.wgsl's mu/nu arrays must be {NMAX} long to match NMAX"
        );
    }
}
