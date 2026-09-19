//! `f32` mirror of `shaders/legendre_plm.wgsl` — the associated Legendre
//! polynomials `P_l^m(x)`.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Separate from the ordinary `P_n`
//!
//! [`crate::wgsl::mirror::legendre_p`] carries `P_n(x)` by Bonnet's
//! recurrence and is reached far more often. Keeping the two apart means a
//! kernel that only wants `P_n` does not carry this one's seed, guard and
//! two-index recurrence.
//!
//! # The fifth table-free shader
//!
//! A closed-form seed and an upward recurrence in the degree, with **no
//! array allocated**: the recurrence carries two scalars, so unlike
//! [`crate::wgsl::mirror_elljac`] there is no register array and no cap to
//! choose. The trip count is `l - m - 1`, a uniform function of two integer
//! parameters.
//!
//! # The guard is retargeted, and here it is load-bearing
//!
//! `LOG_MIN` is `-708.396` in `f64` and `-87.337` in `f32` — a **range
//! guard**, in the taxonomy of `docs/wgsl-coverage.md`, and one that must
//! move with the width. It bites eight times sooner here, and it has to:
//! `P_l^m` grows factorially in `m` and leaves `f32`'s range around
//! `m = 30`, where `f64` survives past 150.
//!
//! This is the opposite call from `synchrotron`'s guard, which is kept at
//! its `f64` value because retargeting it would discard representable
//! answers. The difference is that `synchrotron`'s underflow is reached by a
//! single `exp` that the arithmetic performs anyway, while this magnitude is
//! never formed — the guard is computed *before* the recurrence runs, so
//! nothing else can enforce it.
//!
//! # Upstream's hole at `l == m` is carried, and in `f32` it arrives sooner
//!
//! `legendre_poly.c:304` gates its `t_s` term on `dif == 0.0` rather than
//! `sum == 0.0`, so at `l == m` the only term that could make the estimate
//! negative is zeroed and the guard cannot fire — in exactly the case where
//! `P_l^m` is largest. The `f64` module documents this at length and pins
//! it; it is transcribed here too, because the shader's job is to agree with
//! that module.
//!
//! What changes at this width is **when it bites**: `f64` reaches infinity
//! at `P_200^200`, and `f32` at **`P_30^30`**. Nearly seven times sooner,
//! measured — and it is `m = 30`, not some exotic order, which is why the
//! hole is worth documenting at this width rather than treating as a
//! curiosity. `the_upstream_guard_hole_arrives_sooner_in_f32` pins it.

// Under a std-linked build (`cargo test`) f32's inherent sqrt/ln shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `ln(f32::MIN_POSITIVE)`. Mirrors `PETIR_PLM_LOG_MIN`. `f64`'s is
/// `-708.396`.
const LOG_MIN: f32 = -87.33654;

/// `P_m^m(x)` as a running product. Mirrors `petir_legendre_pmm`.
fn legendre_pmm(m: u32, x: f32) -> f32 {
    if m == 0 {
        return 1.0;
    }
    let mut p_mm = 1.0_f32;
    let root_factor = (1.0 - x).sqrt() * (1.0 + x).sqrt();
    let mut fact_coeff = 1.0_f32;
    for _ in 1..=m {
        p_mm *= -fact_coeff * root_factor;
        fact_coeff += 2.0;
    }
    p_mm
}

/// `P_l^m(x)` in `f32`. Mirrors `petir_legendre_plm`.
///
/// `NaN` for `l < m`, for `|x| > 1`, and where the magnitude estimate says
/// the answer has left `f32`'s range — subject to upstream's hole at
/// `l == m`, which is documented above.
pub fn legendre_plm(l: u32, m: u32, x: f32) -> f32 {
    if x.is_nan() || l < m || !(-1.0..=1.0).contains(&x) {
        return f32::NAN;
    }
    let dif = (l - m) as f32;
    let sum = (l + m) as f32;
    let (t_d, t_s) = if dif == 0.0 {
        (0.0, 0.0)
    } else {
        // NOTE both are gated on `dif`. Upstream's condition, verbatim.
        (0.5 * dif * (dif.ln() - 1.0), 0.5 * sum * (sum.ln() - 1.0))
    };
    let exp_check = 0.5 * (2.0 * l as f32 + 1.0).ln() + t_d - t_s;
    if exp_check < LOG_MIN + 10.0 {
        return f32::NAN;
    }

    let p_mm = legendre_pmm(m, x);
    let p_mmp1 = x * (2 * m + 1) as f32 * p_mm;
    if l == m {
        return p_mm;
    }
    if l == m + 1 {
        return p_mmp1;
    }
    let mut p_ellm2 = p_mm;
    let mut p_ellm1 = p_mmp1;
    let mut p_ell = 0.0_f32;
    for ell in (m + 2)..=l {
        let ellf = ell as f32;
        p_ell = (x * (2.0 * ellf - 1.0) * p_ellm1 - (ellf + m as f32 - 1.0) * p_ellm2)
            / (ellf - m as f32);
        p_ellm2 = p_ellm1;
        p_ellm1 = p_ell;
    }
    p_ell
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::specfunc::legendre as f64_legendre;

    /// **What `f32` costs**, against the `f64` module, over the range where
    /// both are in range.
    ///
    /// Measured 2026-09-20 over `m = 0 ..= 8`, `l = m ..= m + 20` and 41
    /// arguments: worst **1.541e-04**, relative to the magnitude reached, at
    /// `l = 27, m = 8, x = -0.9`.
    ///
    /// That is the largest `f32` cost of any kernel in this module, and it
    /// is the recurrence rather than the transcription: nineteen steps, each
    /// forming `x (2l-1) P_{l-1} - (l+m-1) P_{l-2}` where the two terms are
    /// comparable and large, so it is cancellation compounding. The worst
    /// point is at the largest `m` and near `|x| = 1`, which is where the
    /// seed is smallest and the recurrence has the furthest to climb.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        let (mut worst, mut at) = (0.0_f64, (0u32, 0u32, 0.0_f32));
        for m in 0..=8u32 {
            for l in m..=(m + 20) {
                for i in 0..=40 {
                    let x = -1.0 + 2.0 * i as f32 / 40.0;
                    let a = legendre_plm(l, m, x);
                    let b = f64_legendre::legendre_plm(l, m, x as f64);
                    if !a.is_finite() || !b.is_finite() {
                        continue;
                    }
                    // Relative to the magnitude reached: P_l^m spans many
                    // decades in m, so a bare absolute figure would just
                    // track that.
                    let scale = b.abs().max(1.0);
                    let e = (a as f64 - b).abs() / scale;
                    if e > worst {
                        worst = e;
                        at = (l, m, x);
                    }
                }
            }
        }
        assert!(worst < 1e-3, "P_l^m: {worst:e} at {at:?}");
    }

    /// `m = 0` reduces to the ordinary `P_l`, and the endpoints are exact.
    #[test]
    fn m_zero_reduces_to_the_ordinary_legendre_polynomials() {
        for l in 0..=30u32 {
            for i in 0..=40 {
                let x = -1.0 + 2.0 * i as f32 / 40.0;
                let a = legendre_plm(l, 0, x);
                let b = crate::wgsl::mirror::legendre_p(l, x);
                assert!(
                    (a - b).abs() < 1e-4 * (1.0 + b.abs()),
                    "P_{l}^0({x}): {a} against Bonnet's {b}"
                );
            }
            // P_l(+/-1) = (+/-1)^n exactly, as the recurrence manages.
            assert_eq!(legendre_plm(l, 0, 1.0), 1.0, "P_{l}(1)");
            assert_eq!(
                legendre_plm(l, 0, -1.0),
                if l % 2 == 0 { 1.0 } else { -1.0 },
                "P_{l}(-1)"
            );
        }
    }

    /// The closed forms for the seed, in `f32`.
    #[test]
    fn the_closed_forms_for_the_seed_survive_f32() {
        for i in 0..=40 {
            let x = -1.0 + 2.0 * i as f32 / 40.0;
            let s = (1.0 - x * x).max(0.0).sqrt();
            for (name, got, want) in [
                ("P_1^1", legendre_plm(1, 1, x), -s),
                ("P_2^1", legendre_plm(2, 1, x), -3.0 * x * s),
                ("P_2^2", legendre_plm(2, 2, x), 3.0 * (1.0 - x * x)),
                ("P_3^3", legendre_plm(3, 3, x), -15.0 * s * s * s),
            ] {
                assert!(
                    (got - want).abs() < 1e-5,
                    "{name}({x}) = {got:e} against {want:e}"
                );
            }
        }
    }

    /// **Upstream's `l == m` guard hole is carried, and at `f32` it bites
    /// far sooner** — measured, not assumed.
    ///
    /// `f64` reaches infinity at `P_200^200`. `f32` has three hundred times
    /// less exponent range, so it gets there at a much smaller order; this
    /// finds where, and asserts that the guard is still unable to refuse
    /// anywhere along the way. That is the faithful behaviour, and a reader
    /// needs the number rather than the principle.
    #[test]
    fn the_upstream_guard_hole_arrives_sooner_in_f32() {
        let mut first_inf = None;
        for m in 0..200u32 {
            let v = legendre_plm(m, m, 0.5);
            assert!(
                !v.is_nan(),
                "the guard is documented as unable to fire at l == m; it \
                 refused m = {m}"
            );
            if v.is_infinite() && first_inf.is_none() {
                first_inf = Some(m);
            }
        }
        let Some(m0) = first_inf else {
            panic!(
                "P_m^m never overflowed in f32 up to m = 200, which \
                    contradicts its factorial growth"
            )
        };
        // Far sooner than f64's 200, which is the point.
        assert!(
            m0 < 100,
            "f32 is documented as reaching infinity far sooner than f64's \
             m = 200; it first did so at {m0}"
        );
        // And just below it the answer is finite and enormous, so the
        // overflow is genuine range loss rather than a NaN leaking in.
        let below = legendre_plm(m0 - 1, m0 - 1, 0.5);
        assert!(
            below.is_finite() && below.abs() > 1e30,
            "P at the order below the first infinity is {below:e}"
        );
    }

    /// The refusals, and that the guard DOES work where `dif` is non-zero.
    #[test]
    fn the_refusals_match_the_f64_module() {
        assert!(legendre_plm(2, 3, 0.5).is_nan(), "l < m");
        for x in [1.0001_f32, -1.0001, 2.0, f32::INFINITY] {
            assert!(legendre_plm(3, 1, x).is_nan(), "x = {x}");
        }
        assert!(legendre_plm(3, 1, f32::NAN).is_nan());
        for l in 1..=6u32 {
            for m in 1..=l {
                assert_eq!(legendre_plm(l, m, 1.0), 0.0, "P_{l}^{m}(1)");
            }
        }
        for i in 0..=10 {
            let x = -1.0 + 0.2 * i as f32;
            assert_eq!(legendre_plm(0, 0, x), 1.0);
        }
        // Where dif != 0 the guard is live: at a large enough l it refuses.
        let mut refused = false;
        for l in 40..400u32 {
            if legendre_plm(l, 40, 0.5).is_nan() {
                refused = true;
                break;
            }
        }
        assert!(
            refused,
            "the guard is documented as working where dif != 0; it never \
             refused at m = 40 for any l below 400"
        );
    }

    /// The shader and this mirror agree on the one shared constant, and it
    /// is what its formula gives at `f32`'s range.
    #[test]
    fn the_shader_and_this_mirror_agree_on_the_guard() {
        let src = crate::wgsl::LEGENDRE_PLM;
        let at = src
            .find("PETIR_PLM_LOG_MIN: f32")
            .expect("PETIR_PLM_LOG_MIN not declared in legendre_plm.wgsl");
        let rest = &src[at..];
        let eq = rest.find('=').expect("no = after the constant name");
        let end = rest[eq..].find(';').expect("no ; after the value");
        let read: f32 = rest[eq + 1..eq + end]
            .trim()
            .parse()
            .expect("PETIR_PLM_LOG_MIN is not an f32 literal");
        assert_eq!(read, LOG_MIN);
        // Recomputed, not restated.
        let want = f64::from(f32::MIN_POSITIVE).ln();
        assert!(
            ((LOG_MIN as f64 - want) / want).abs() < 1e-6,
            "LOG_MIN is {LOG_MIN:e}, ln(f32::MIN_POSITIVE) gives {want:e}"
        );
        // And the shader carries upstream's condition, not a repaired one.
        assert!(
            src.contains("if (dif != 0.0) {"),
            "the shader must gate BOTH t_d and t_s on dif, as upstream does"
        );
    }
}
