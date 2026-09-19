#![cfg(all(
    feature = "wgpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]

use petir::wgsl::gpu::{GpuContext, KernelParams};
use petir::wgsl::{mirror, mirror_erf, CHEB, ERF, LEGENDRE, POLY};

/// Largest absolute difference between two same-length slices.
fn worst_abs(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max)
}

// ---------------------------------------------------------------------------
// Dispatch — the real verification
// ---------------------------------------------------------------------------

/// `petir_poly_eval` on the GPU matches the `f32` CPU mirror.
///
/// # Methodology
///
/// A degree-8 polynomial with ascending coefficients, evaluated at 256 probe
/// points across `[-2, 2]` on the device and by
/// [`petir::wgsl::mirror::poly_eval`] on the host. Both run the same Horner
/// sweep at the same width, so any difference is the device's freedom to
/// contract or round differently — not an algorithmic one.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** across all 256 points,
/// measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// Interpretation: the transcription is exact, and this device did not
/// contract `acc * x + c` into an FMA — had it done so the two would differ in
/// the last ulp without either being wrong. Another device may contract; the
/// assertion allows a small tolerance so that would not be a false failure,
/// but the measured zero is what is recorded, and a regression away from it is
/// worth investigating even while still passing.
#[test]
fn gpu_poly_eval_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_poly_eval_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let coeffs: Vec<f32> = vec![2.0, -3.0, 1.5, 0.25, -0.5, 1.0, 0.125, -1.0, 0.75];
    let probes: Vec<f32> = (0..256).map(|i| -2.0 + 4.0 * i as f32 / 255.0).collect();

    let got = gpu
        .eval_map(
            &[POLY],
            "petir_poly_eval(0u, params.n, x)",
            &coeffs,
            &probes,
            KernelParams {
                n: coeffs.len() as u32,
                a: 0.0,
                b: 0.0,
                k: 0,
            },
        )
        .expect("non-empty probe");
    let want: Vec<f32> = probes
        .iter()
        .map(|&x| mirror::poly_eval(&coeffs, x))
        .collect();

    assert_eq!(got.len(), want.len());
    let worst = worst_abs(&got, &want);
    assert!(
        worst <= 1e-5,
        "GPU ({}) disagrees with the f32 mirror by {worst:e}",
        gpu.adapter_name()
    );
}

/// `petir_cheb_eval` on the GPU matches the `f32` CPU mirror.
///
/// # Methodology
///
/// An order-16 Chebyshev fit of `exp` on `[-1, 1]`, built by PETIR's `f64`
/// [`petir::cheb::ChebSeries`] and rounded to `f32`, evaluated at 256 points.
/// Clenshaw is the case where a transcription error is most likely and least
/// visible — the halved `c[0]`, and the `d1`/`d2` swap order.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** across all 256 points,
/// measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// This is the strongest of the three: Clenshaw over 17 coefficients is 34
/// dependent floating-point operations per point, and every one of them landed
/// on the same bit pattern as the CPU mirror.
#[test]
fn gpu_cheb_eval_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_cheb_eval_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let series = petir::cheb::ChebSeries::new(16, -1.0, 1.0, |x: f64| x.exp())
        .expect("order 16 fit on [-1, 1]");
    let c32: Vec<f32> = series.coefficients().iter().map(|&v| v as f32).collect();
    let probes: Vec<f32> = (0..256).map(|i| -1.0 + 2.0 * i as f32 / 255.0).collect();

    let got = gpu
        .eval_map(
            &[CHEB],
            "petir_cheb_eval(0u, params.n, params.a, params.b, x)",
            &c32,
            &probes,
            KernelParams {
                n: c32.len() as u32,
                a: -1.0,
                b: 1.0,
                k: 0,
            },
        )
        .expect("non-empty probe");
    let want: Vec<f32> = probes
        .iter()
        .map(|&x| mirror::cheb_eval(&c32, -1.0, 1.0, x))
        .collect();

    let worst = worst_abs(&got, &want);
    assert!(
        worst <= 1e-5,
        "GPU ({}) disagrees with the f32 mirror by {worst:e}",
        gpu.adapter_name()
    );
}

/// `petir_legendre_p` on the GPU matches the `f32` CPU mirror, at every order
/// up to 16.
///
/// # Methodology
///
/// `P_n` for `n = 0..=16` at 256 points across `[-1, 1]`, one dispatch per
/// order. Legendre is swept across orders rather than tested at one because
/// the recurrence accumulates, so an off-by-one in the loop bound shows up
/// only at specific `n`.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** across all 17 orders and
/// 4352 points, measured 2026-09-19 on
/// `llvmpipe (LLVM 20.1.2, 256 bits)`.
#[test]
fn gpu_legendre_matches_the_cpu_mirror_at_every_order() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_legendre_matches_the_cpu_mirror_at_every_order: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = (0..256).map(|i| -1.0 + 2.0 * i as f32 / 255.0).collect();
    let mut worst = 0.0_f32;
    for n in 0..=16u32 {
        let got = gpu
            .eval_map(
                &[LEGENDRE],
                "petir_legendre_p(params.k, x)",
                &[],
                &probes,
                KernelParams {
                    n: 0,
                    a: 0.0,
                    b: 0.0,
                    k: n,
                },
            )
            .expect("non-empty probe");
        let want: Vec<f32> = probes.iter().map(|&x| mirror::legendre_p(n, x)).collect();
        worst = worst.max(worst_abs(&got, &want));
    }
    assert!(
        worst <= 1e-5,
        "GPU ({}) disagrees with the f32 mirror by {worst:e}",
        gpu.adapter_name()
    );
}

/// The GPU's Chebyshev evaluation tracks PETIR's `f64` answer to the `f32`
/// budget.
///
/// # Why this is separate from the mirror comparison
///
/// This is the end-to-end number a caller actually cares about: *if I move
/// this evaluation to the GPU, how much accuracy do I lose?* It cannot
/// substitute for the mirror comparison, because a failure here does not say
/// whether the shader or `f32` is responsible — but the mirror test answers
/// that, so together they do.
///
/// # Results
///
/// Worst relative difference from the `f64` series **1.415e-07**, measured
/// 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)` — about 1.2 `f32` ulp.
///
/// This is close to, but **not the same statistic as**, the 1.705e-07 recorded
/// for mirror-vs-`f64` in `petir::wgsl::mirror`: that test samples 51 points
/// and this one 256, so the two worst-case maxima are taken over different
/// grids. They agree in magnitude, which is the expected consequence of the
/// GPU being bit-identical to the mirror — if they had differed materially,
/// one of the two comparisons would be measuring something other than what it
/// claims.
#[test]
fn gpu_cheb_tracks_the_f64_reference_within_the_f32_budget() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_cheb_tracks_the_f64_reference_within_the_f32_budget: no GPU adapter");
        return;
    };
    let series = petir::cheb::ChebSeries::new(16, -1.0, 1.0, |x: f64| x.exp())
        .expect("order 16 fit on [-1, 1]");
    let c32: Vec<f32> = series.coefficients().iter().map(|&v| v as f32).collect();
    let probes: Vec<f32> = (0..256).map(|i| -1.0 + 2.0 * i as f32 / 255.0).collect();

    let got = gpu
        .eval_map(
            &[CHEB],
            "petir_cheb_eval(0u, params.n, params.a, params.b, x)",
            &c32,
            &probes,
            KernelParams {
                n: c32.len() as u32,
                a: -1.0,
                b: 1.0,
                k: 0,
            },
        )
        .expect("non-empty probe");

    let mut worst = 0.0_f64;
    for (&x, &g) in probes.iter().zip(got.iter()) {
        let exact = series.eval(x as f64);
        let rel = ((g as f64 - exact) / exact).abs();
        if rel > worst {
            worst = rel;
        }
    }
    assert!(
        worst < 1e-5,
        "GPU ({}) vs f64 reference: worst relative {worst:e}",
        gpu.adapter_name()
    );
}

/// `petir_erfc` and `petir_erf` on the GPU match the `f32` CPU mirror, across
/// every one of GSL's branches.
///
/// # Methodology
///
/// 512 probe points over `[-12, 12]`, which crosses all four `erfc` branch
/// boundaries (|x| = 1, 5, 10) and the sign reflection. Compared as an
/// **absolute** difference against the mirror, not a relative one: both sides
/// are the same `f32` computation, so what is being tested is whether the
/// device reproduces it, and in the tail both are legitimately zero.
///
/// `erf` is checked on the same grid.
///
/// # Results
///
/// Worst absolute difference **1.192e-07 for `erfc`** and **5.960e-08 for
/// `erf`**, over 512 points, measured 2026-09-19 on
/// `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// **These are the first kernels here that are NOT bit-identical, and the
/// reason is worth stating.** `poly`, `cheb` and `legendre` all match the
/// mirror exactly, because they are pure arithmetic — add, multiply, divide —
/// which IEEE-754 pins to a single correctly-rounded answer. `erf` and `erfc`
/// call `exp`, and WGSL specifies its builtins to an **ULP bound rather than
/// correct rounding**, so the device's `exp` and `libm::expf` are different
/// functions that agree to about an ulp. One or two ulp out of a composed
/// expression is exactly what that predicts.
///
/// The practical rule this establishes for the rest of the module: a kernel
/// built only from arithmetic can be held to bit-identity, and a kernel that
/// touches a transcendental builtin cannot. Asserting the former on the latter
/// would produce a test that fails on a conforming device.
///
/// Within that, this is still the strongest transcription evidence in the
/// suite. `erfc` is 4 branches, 3 Chebyshev tables, a 30-term series and a
/// degree-5/6 rational; landing within an ulp of the mirror everywhere means
/// the branch thresholds, the argument mappings and all 65 extracted
/// coefficients are right.
#[test]
fn gpu_erf_family_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_erf_family_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = (0..512).map(|i| -12.0 + 24.0 * i as f32 / 511.0).collect();

    let got_erfc = gpu
        .eval_map(
            &[ERF],
            "petir_erfc(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    let want_erfc: Vec<f32> = probes.iter().map(|&x| mirror_erf::erfc(x)).collect();
    let worst_erfc = worst_abs(&got_erfc, &want_erfc);

    let got_erf = gpu
        .eval_map(
            &[ERF],
            "petir_erf(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    let want_erf: Vec<f32> = probes.iter().map(|&x| mirror_erf::erf(x)).collect();
    let worst_erf = worst_abs(&got_erf, &want_erf);

    assert!(
        worst_erfc <= 1e-6 && worst_erf <= 1e-6,
        "GPU ({}) vs f32 mirror: erfc {worst_erfc:e}, erf {worst_erf:e}",
        gpu.adapter_name()
    );
}

/// The GPU's `erfc` tracks PETIR's `f64` `erfc` to the `f32` budget.
///
/// # Results
///
/// Worst relative difference **4.327e-06** over `[-9, 9]`, measured
/// 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// The CPU mirror records 8.368e-06 against the same `f64` reference, over
/// `[-12, 12]` at 2400 points rather than `[-9, 9]` at 512. The two are
/// different grids and so different maxima — the mirror's worst point, `x =
/// 8.1`, is simply not in this one's sample. They are the same statistic only
/// in magnitude, and that is all that should be read from the agreement.
///
/// See `petir::wgsl::mirror_erf`'s per-branch table for why this number is
/// `exp`'s argument amplification rather than a Chebyshev truncation, and why
/// raising the order would not improve it.
#[test]
fn gpu_erfc_tracks_the_f64_reference_within_the_f32_budget() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_erfc_tracks_the_f64_reference_within_the_f32_budget: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = (0..512).map(|i| -9.0 + 18.0 * i as f32 / 511.0).collect();
    let got = gpu
        .eval_map(
            &[ERF],
            "petir_erfc(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");

    let mut worst = 0.0_f64;
    for (&x, &g) in probes.iter().zip(got.iter()) {
        let exact = petir::specfunc::erfc(x as f64);
        if exact < 1e-30 {
            continue;
        }
        let rel = ((g as f64 - exact) / exact).abs();
        if rel > worst {
            worst = rel;
        }
    }
    assert!(
        worst < 1e-4,
        "GPU ({}) vs f64 erfc: worst relative {worst:e}",
        gpu.adapter_name()
    );
}
