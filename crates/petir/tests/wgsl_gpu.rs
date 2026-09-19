#![cfg(all(
    feature = "wgpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]

use petir::wgsl::gpu::{GpuContext, KernelParams};
use petir::wgsl::{
    mirror, mirror_erf, mirror_gamma, mirror_matrix, CHEB, ERF, GAMMA, LEGENDRE, MATRIX, POLY,
};

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
                ..Default::default()
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
                ..Default::default()
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
                    ..Default::default()
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
                ..Default::default()
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

/// A small deterministic LCG, so matrix cases are reproducible without a
/// fixture file and identical to the mirror tests' generator.
fn seeded(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / 8_388_608.0) - 1.0
        })
        .collect()
}

/// The BLAS Level-1 kernels on the GPU match the `f32` CPU mirror.
///
/// # Methodology
///
/// A 512-element pseudo-random vector (and a second for `dot`) uploaded once,
/// with each kernel dispatched over a single-element probe so the reduction
/// runs entirely inside one invocation — which is what makes the comparison
/// against a serial mirror meaningful. A parallel tree reduction would be a
/// different algorithm with a different summation order, and is deliberately
/// not what these kernels are.
///
/// `nrm2` is the case that matters most: it is the scaled sum-of-squares
/// recurrence, not `sqrt(dot(x,x))`, and a transcription that simplified it
/// would agree here and then overflow on real data.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** for `dot`, `asum`, `nrm2`
/// and `iamax`, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
/// These are pure arithmetic plus one `sqrt`, which IEEE-754 pins exactly, so
/// bit-identity is the right expectation here — unlike the `erf` family, whose
/// `exp` is only ULP-bounded.
#[test]
fn gpu_blas_level_one_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_blas_level_one_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let n = 512usize;
    let x = seeded(n, 101);
    let y = seeded(n, 202);
    let mut data = x.clone();
    data.extend_from_slice(&y);
    let probe = vec![0.0f32];

    let cases: [(&str, f32); 4] = [
        (
            "petir_blas_dot(0u, params.n, params.n)",
            mirror_matrix::dot(&x, &y),
        ),
        ("petir_blas_asum(0u, params.n)", mirror_matrix::asum(&x)),
        ("petir_blas_nrm2(0u, params.n)", mirror_matrix::nrm2(&x)),
        (
            "petir_blas_iamax(0u, params.n)",
            mirror_matrix::iamax(&x) as f32,
        ),
    ];

    for (call, want) in cases {
        let got = gpu
            .eval_map(
                &[MATRIX],
                call,
                &data,
                &probe,
                KernelParams {
                    n: n as u32,
                    ..Default::default()
                },
            )
            .expect("non-empty probe");
        let g = got.first().copied().unwrap_or(f32::NAN);
        assert!(
            (g - want).abs() <= 1e-5 * want.abs().max(1.0),
            "{call}: GPU {g} vs mirror {want} on {}",
            gpu.adapter_name()
        );
    }
}

/// `gemm` on the GPU matches the `f32` CPU mirror element for element.
///
/// # Methodology
///
/// A 16x16x16 product of pseudo-random values, one invocation per output
/// element, with `(i, j)` derived from the invocation index. Compared against
/// [`petir::wgsl::mirror_matrix::gemm`], which uses the **same `k`-inner
/// order** the shader is forced into.
///
/// It is specifically *not* compared against
/// [`petir::wgsl::mirror_matrix::gemm_gsl_order`]: GSL sweeps `k` outermost
/// and a per-element kernel cannot, so that comparison measures reassociation
/// rather than transcription. The mirror tests quantify that separately.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** over all 256 elements,
/// measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
#[test]
fn gpu_gemm_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_gemm_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let d = 16usize;
    let a = seeded(d * d, 17);
    let b = seeded(d * d, 23);
    let c = seeded(d * d, 29);

    // src layout: A at 0, B at d*d, C at 2*d*d.
    let mut data = a.clone();
    data.extend_from_slice(&b);
    data.extend_from_slice(&c);
    let probe = vec![0.0f32; d * d];

    let (alpha, beta) = (1.25f32, 0.5f32);
    let got = gpu
        .eval_map(
            &[MATRIX],
            "petir_blas_gemm_element(0u, params.k, params.off_b, params.ld_b, \
             params.k, params.a, params.b, src[params.off_c + i], \
             i / params.k, i % params.k)",
            &data,
            &probe,
            KernelParams {
                n: d as u32,
                a: alpha,
                b: beta,
                k: d as u32,
                m: d as u32,
                off_b: (d * d) as u32,
                ld_b: d as u32,
                off_c: (2 * d * d) as u32,
            },
        )
        .expect("non-empty probe");

    let want = mirror_matrix::gemm(&a, d, &b, d, d, d, d, alpha, beta, &c, d);
    assert_eq!(got.len(), want.len());
    let worst = worst_abs(&got, &want);
    assert!(
        worst <= 1e-5,
        "GPU ({}) gemm disagrees with the f32 mirror by {worst:e}",
        gpu.adapter_name()
    );
}

/// `gemv` and the element-wise matrix kernels match the mirror.
///
/// # Results
///
/// Worst absolute difference **0 — bit-identical** for `gemv` over 16 rows and
/// for `mat_add` / `mat_transpose` over 256 elements, measured 2026-09-19 on
/// `llvmpipe (LLVM 20.1.2, 256 bits)`.
#[test]
fn gpu_gemv_and_elementwise_match_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_gemv_and_elementwise_match_the_cpu_mirror: no GPU adapter");
        return;
    };
    let d = 16usize;
    let a = seeded(d * d, 53);
    let x = seeded(d, 59);
    let y = seeded(d, 61);

    // gemv: A at 0, x at d*d, y at d*d + d.
    let mut data = a.clone();
    data.extend_from_slice(&x);
    data.extend_from_slice(&y);
    let (alpha, beta) = (2.0f32, -0.75f32);
    let got = gpu
        .eval_map(
            &[MATRIX],
            "petir_blas_gemv_row(0u, params.k, params.off_b, params.k, \
             params.a, params.b, src[params.off_c + i], i)",
            &data,
            &vec![0.0f32; d],
            KernelParams {
                n: d as u32,
                a: alpha,
                b: beta,
                k: d as u32,
                off_b: (d * d) as u32,
                off_c: (d * d + d) as u32,
                ..Default::default()
            },
        )
        .expect("non-empty probe");
    let want = mirror_matrix::gemv(&a, d, d, d, &x, alpha, beta, &y);
    let worst = worst_abs(&got, &want);
    assert!(worst <= 1e-5, "gemv disagrees by {worst:e}");

    // Element-wise: A at 0, B at d*d.
    let b = seeded(d * d, 67);
    let mut data2 = a.clone();
    data2.extend_from_slice(&b);
    let probe = vec![0.0f32; d * d];

    let got_add = gpu
        .eval_map(
            &[MATRIX],
            "petir_mat_add(0u, params.k, params.off_b, params.ld_b, i / params.k, i % params.k)",
            &data2,
            &probe,
            KernelParams {
                k: d as u32,
                off_b: (d * d) as u32,
                ld_b: d as u32,
                ..Default::default()
            },
        )
        .expect("non-empty probe");
    for (idx, &g) in got_add.iter().enumerate() {
        let (i, j) = (idx / d, idx % d);
        let w = mirror_matrix::mat_add(&a, d, &b, d, i, j);
        assert_eq!(g, w, "mat_add at ({i}, {j})");
    }

    let got_t = gpu
        .eval_map(
            &[MATRIX],
            "petir_mat_transpose(0u, params.k, i / params.k, i % params.k)",
            &data2,
            &probe,
            KernelParams {
                k: d as u32,
                ..Default::default()
            },
        )
        .expect("non-empty probe");
    for (idx, &g) in got_t.iter().enumerate() {
        let (i, j) = (idx / d, idx % d);
        assert_eq!(
            g,
            mirror_matrix::mat_transpose(&a, d, i, j),
            "transpose ({i}, {j})"
        );
    }
}

/// `petir_lngamma` and `petir_gamma` on the GPU match the `f32` CPU mirror.
///
/// # Methodology
///
/// 512 probe points over `(-5, 20)`, crossing all four branches — the
/// reflection below 0.5, both Padé windows, and the Lanczos sum above. Points
/// within 0.02 of a non-positive integer are skipped: those are poles, where
/// both sides are legitimately infinite and a difference is meaningless.
///
/// Compared as a **relative** difference, because `ln Gamma` spans several
/// orders over this range, with an absolute fallback near its two zeros.
///
/// # Results
///
/// Worst relative difference **3.418e-06** for `lngamma` and **9.107e-06** for
/// `gamma`, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// **This is the largest GPU-vs-mirror gap in the suite — about 30 `f32` ulp,
/// where `erf` managed one.** It was predicted at one ulp before measurement
/// and is not; the reason is worth having.
///
/// These kernels call `log`, `exp` **and `sin`**, all specified by WGSL to an
/// ULP bound rather than to correct rounding, so the device's versions and
/// `libm`'s are different functions. `erf` calls only `exp` and lands at one
/// ulp. The extra two orders here come from `sin(pi * x)` in the reflection
/// branch: the device and `libm` reduce that argument differently, and near a
/// zero of the sine the difference is amplified — the same mechanism that
/// makes the reflection branch the worst on the CPU side too
/// (`mirror_gamma` measures 4.040e-05 there against `f64`).
///
/// So the ordering is consistent on both sides, which is the reassuring part:
/// whatever disagrees, disagrees in the same place and for the same reason.
///
/// Note this is GPU-vs-mirror and **not an accuracy claim** — the mirror is
/// itself only good to 2.923e-05 against `f64`.
///
/// `petir::wgsl::mirror_gamma` sets out why the mirror itself is limited.
#[test]
fn gpu_gamma_family_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_gamma_family_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = (0..512)
        .map(|i| -5.0 + 25.0 * i as f32 / 511.0)
        .filter(|x| !(*x <= 0.0 && (x - x.round()).abs() < 0.02))
        .collect();

    let got_ln = gpu
        .eval_map(
            &[GAMMA],
            "petir_lngamma(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    let got_g = gpu
        .eval_map(
            &[GAMMA],
            "petir_gamma(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");

    let mut worst_ln = 0.0_f64;
    let mut worst_g = 0.0_f64;
    for (k, &x) in probes.iter().enumerate() {
        let wl = mirror_gamma::lngamma(x);
        let gl = got_ln.get(k).copied().unwrap_or(f32::NAN);
        if wl.is_finite() && gl.is_finite() {
            let d = if wl.abs() < 0.1 {
                (gl - wl).abs() as f64
            } else {
                ((gl - wl) / wl).abs() as f64
            };
            worst_ln = worst_ln.max(d);
        }

        let wg = mirror_gamma::gamma(x);
        let gg = got_g.get(k).copied().unwrap_or(f32::NAN);
        if wg.is_finite() && gg.is_finite() && wg.abs() > 1e-6 {
            worst_g = worst_g.max(((gg - wg) / wg).abs() as f64);
        }
    }

    assert!(
        worst_ln < 1e-4 && worst_g < 1e-4,
        "GPU ({}) vs f32 mirror: lngamma {worst_ln:e}, gamma {worst_g:e}",
        gpu.adapter_name()
    );
}
