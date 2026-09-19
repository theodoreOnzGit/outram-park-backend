#![cfg(all(
    feature = "wgpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]

use petir::wgsl::gpu::{GpuContext, KernelParams};
use petir::wgsl::{
    mirror, mirror_airy, mirror_atanint, mirror_bessel, mirror_clausen, mirror_debye, mirror_dilog,
    mirror_erf, mirror_gamma, mirror_lambert, mirror_matrix, mirror_psi_zeta, mirror_transport,
    AIRY, ATANINT, BESSEL, CHEB, CLAUSEN, DEBYE, DILOG, ERF, GAMMA, LAMBERT, LEGENDRE, MATRIX,
    POLY, PSI_ZETA, TRANSPORT,
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

/// The whole Bessel family on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// All twelve entry points — `J_0`, `J_1`, `Y_0`, `Y_1`, `I_0`, `I_1`, `K_0`,
/// `K_1` and the four exponentially scaled modified forms — dispatched over
/// 512 probes on `(0, 40]`, one dispatch each, against
/// [`petir::wgsl::mirror_bessel`]. The window stops at 40 because `I_0`
/// leaves `f32` near `x = 88` and a comparison of two infinities establishes
/// nothing.
///
/// The comparison is **relative where the value is not near a zero and
/// absolute where it is**, for the reason `mirror_bessel`'s documentation
/// sets out: `J` and `Y` pass through zero, and a relative figure there is
/// dominated by how close a probe happened to land rather than by the device.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | kernel | worst | at |
/// |---|---|---|
/// | `I_0` | 1.821e-06 | 38.359 |
/// | `I_1` | 1.761e-06 | 35.781 |
/// | `K_0` | 2.242e-07 | 1.875 |
/// | `K_1` | 2.812e-07 | 1.406 |
/// | `I_0` scaled | 2.364e-07 | 2.813 |
/// | `I_1` scaled | 3.716e-07 | 2.813 |
/// | `K_0` scaled | 3.040e-07 | 0.938 |
/// | `K_1` scaled | 1.758e-07 | 14.375 |
/// | `J_0` | 2.246e-07 | 22.109 |
/// | `J_1` | 5.695e-07 | 3.516 |
/// | `Y_0` | 3.980e-07 | 0.703 |
/// | `Y_1` | 3.329e-07 | 1.875 |
///
/// **The two unscaled `I` kernels are an order worse than their scaled forms,
/// and that is the device's `exp`.** `I_0(x)` is `exp(x) * I_0_scaled(x)`;
/// the scaled form does not call `exp` at all and sits at 2.4e-07, while the
/// unscaled one picks up 1.8e-06 at `x = 38`. WGSL's allowance for `exp` is
/// `3 + 2|x|` ulp, which at `x = 38` is 79 ulp — the measured 15 ulp is well
/// inside it. Nothing is wrong; the scaled entry point is simply the one to
/// reach for when the device's `exp` is in the way.
///
/// **These cannot be bit-identical and it would be wrong to assert that they
/// are.** Every Bessel branch calls `sqrt`, `exp`, `log`, `sin` or `cos`, and
/// WGSL specifies its builtins to an **ULP bound rather than correct
/// rounding** — the device's `sin` and `libm`'s `sinf` are different functions
/// agreeing to about an ulp. The pure-arithmetic kernels in this file (Horner,
/// Clenshaw, BLAS) *are* held to zero; these are not, and the difference is
/// the builtins.
///
/// # The budgets are looser than the measurements, deliberately
///
/// WGSL's allowance for `sin`/`cos` is an **absolute** error of `2^-11`
/// (4.9e-04) on `[-pi, pi]`, and is left implementation-defined outside it.
/// A strictly conforming device could therefore be three orders worse than
/// llvmpipe on the `J`/`Y` asymptotic branches without violating anything. The
/// budgets below are set where a real device plausibly lands rather than at
/// that allowance, so a failure is worth investigating — but it would be a
/// finding about the device's builtins, not about this transcription.
///
/// This is GPU-vs-mirror and **not an accuracy claim**: `mirror_bessel`
/// measures its own distance from `f64` separately.
#[test]
fn gpu_bessel_family_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_bessel_family_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = (1..=512).map(|i| 40.0 * i as f32 / 512.0).collect();

    // (call, mirror, budget). One budget per function rather than one for the
    // family: the oscillatory four legitimately differ more, since a phase
    // disagreement of an ulp lands on a function whose slope near a zero is
    // steep, and flattening that into one number would hide it.
    let cases: [(&str, fn(f32) -> f32, f64); 12] = [
        // The two unscaled I kernels carry the device's exp() and get a
        // looser budget for it; everything else is held near one ulp.
        ("petir_bessel_i0(x)", mirror_bessel::i0, 2e-5),
        ("petir_bessel_i1(x)", mirror_bessel::i1, 2e-5),
        ("petir_bessel_k0(x)", mirror_bessel::k0, 5e-6),
        ("petir_bessel_k1(x)", mirror_bessel::k1, 5e-6),
        ("petir_bessel_i0_scaled(x)", mirror_bessel::i0_scaled, 5e-6),
        ("petir_bessel_i1_scaled(x)", mirror_bessel::i1_scaled, 5e-6),
        ("petir_bessel_k0_scaled(x)", mirror_bessel::k0_scaled, 5e-6),
        ("petir_bessel_k1_scaled(x)", mirror_bessel::k1_scaled, 5e-6),
        ("petir_bessel_j0(x)", mirror_bessel::j0, 5e-6),
        ("petir_bessel_j1(x)", mirror_bessel::j1, 5e-6),
        ("petir_bessel_y0(x)", mirror_bessel::y0, 5e-6),
        ("petir_bessel_y1(x)", mirror_bessel::y1, 5e-6),
    ];

    for (call, mirror_fn, budget) in cases {
        let got = gpu
            .eval_map(&[BESSEL], call, &[], &probes, KernelParams::default())
            .expect("non-empty probe");
        let mut worst = 0.0_f64;
        let mut at = 0.0_f32;
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_fn(x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            if !want.is_finite() || !have.is_finite() {
                continue;
            }
            // Relative away from a zero, absolute at one -- see the doc above.
            let d = if want.abs() < 0.1 {
                (have - want).abs() as f64
            } else {
                ((have - want) / want).abs() as f64
            };
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < budget,
            "GPU ({}) vs f32 mirror for {call}: {worst:e} at x = {at}, budget {budget:e}",
            gpu.adapter_name()
        );
    }
}

/// The Bessel shader agrees with the mirror on the *domain* errors too, not
/// only on the numbers.
///
/// `Y` and `K` are undefined for `x <= 0`, and both sides spell that test as
/// `!(x > 0.0)` so that `NaN` propagates rather than falling into a branch.
/// A shader that returned a plausible number there would pass every numerical
/// comparison above, because the probes are all positive.
#[test]
fn gpu_bessel_rejects_non_positive_arguments_exactly_as_the_mirror_does() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_bessel_rejects_non_positive_arguments_exactly_as_the_mirror_does: no GPU adapter");
        return;
    };
    let probes: Vec<f32> = vec![-4.0, -1.0, -0.001, 0.0, 1.0, 4.0];
    for (call, mirror_fn) in [
        ("petir_bessel_y0(x)", mirror_bessel::y0 as fn(f32) -> f32),
        ("petir_bessel_y1(x)", mirror_bessel::y1),
        ("petir_bessel_k0(x)", mirror_bessel::k0),
        ("petir_bessel_k1(x)", mirror_bessel::k1),
    ] {
        let got = gpu
            .eval_map(&[BESSEL], call, &[], &probes, KernelParams::default())
            .expect("non-empty probe");
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_fn(x);
            let have = got.get(k).copied().unwrap_or(0.0);
            assert_eq!(
                want.is_nan(),
                have.is_nan(),
                "{call} at x = {x}: mirror NaN = {}, GPU NaN = {} (GPU gave {have})",
                want.is_nan(),
                have.is_nan()
            );
        }
    }
}

/// The digamma and zeta families on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// Seven entry points dispatched over probe sets chosen per function — the
/// zeta family needs points on both sides of zero to reach the reflection
/// branch, the digamma family does not. `GAMMA` is concatenated ahead of
/// `PSI_ZETA` because `petir_zeta` calls `petir_gamma`; that dependency is
/// the reason the Lanczos table is not duplicated.
///
/// The comparison is relative away from a zero and absolute at one, as in
/// `gpu_bessel_family_matches_the_cpu_mirror` — `psi` and `zeta` both have
/// zeros, and a relative figure at one measures where a probe landed rather
/// than what the device did.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | kernel | worst | at |
/// |---|---|---|
/// | `zetam1` | 3.725e-09 | 5.45 |
/// | `psi` | 1.228e-07 | 3.125 |
/// | `hzeta`, over five `q` | 1.633e-07 .. 5.674e-07 | — |
/// | `psi_1` | 2.350e-07 | 8.375 |
/// | `psi_1piy` | 2.445e-07 | 1.375 |
/// | `eta` | **1.040e-05** | -27.25 |
/// | `zeta` | **1.079e-05** | -26.25 |
///
/// **The two outliers are the reflection branch, and they are `petir_gamma`
/// showing through.** `gpu_gamma_family_matches_the_cpu_mirror` independently
/// measures that kernel at 9.107e-06 against its own mirror; `zeta`'s
/// 1.079e-05 is that figure multiplied by the rest of the functional
/// equation. Nothing about `zeta`'s own transcription is implicated — its
/// positive branch, which calls no `Gamma`, sits at the same one ulp as
/// everything else here.
///
/// The budgets are set at 5e-04, two orders above the worst measurement, for
/// the reason `gpu_bessel_family_matches_the_cpu_mirror` gives: WGSL's
/// allowance for `sin` is an absolute 2^-11, so a conforming device could be
/// much further out without being wrong.
///
/// **Not bit-identical, and it would be wrong to assert that they are.**
/// Every branch here calls `log`, `exp`, `sin` or `pow`, which WGSL specifies
/// to an ULP bound rather than correct rounding.
///
/// This is GPU-vs-mirror and **not an accuracy claim**: `mirror_psi_zeta`
/// measures its own distance from `f64` separately, and records `zeta` below
/// zero at 9.391e-06 against `f64` for reasons that have nothing to do with
/// the device.
#[test]
fn gpu_psi_zeta_family_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_psi_zeta_family_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    // Positive axis, for the digamma family and hzeta's second argument.
    let positive: Vec<f32> = (1..=256).map(|i| 0.125 * i as f32).collect();
    // Both sides of zero for zeta and eta, skipping the pole at s = 1 and
    // staying above the s = -34 refusal.
    let signed: Vec<f32> = (0..256)
        .map(|i| -30.0 + 0.25 * i as f32)
        .filter(|s| (s - 1.0).abs() > 0.2)
        .collect();
    // zetam1's own domain.
    let above_five: Vec<f32> = (1..=256).map(|i| 5.0 + 0.15 * i as f32).collect();

    let cases: [(&str, fn(f32) -> f32, &Vec<f32>, f64); 6] = [
        ("petir_psi(x)", mirror_psi_zeta::psi, &positive, 5e-4),
        ("petir_psi_1(x)", mirror_psi_zeta::psi_1, &positive, 5e-4),
        (
            "petir_psi_1piy(x)",
            mirror_psi_zeta::psi_1piy,
            &positive,
            5e-4,
        ),
        ("petir_zeta(x)", mirror_psi_zeta::zeta, &signed, 5e-4),
        ("petir_eta(x)", mirror_psi_zeta::eta, &signed, 5e-4),
        (
            "petir_zetam1(x)",
            mirror_psi_zeta::zetam1,
            &above_five,
            5e-4,
        ),
    ];

    for (call, mirror_fn, probes, budget) in cases {
        let got = gpu
            .eval_map(
                &[GAMMA, PSI_ZETA],
                call,
                &[],
                probes,
                KernelParams::default(),
            )
            .expect("non-empty probe");
        let mut worst = 0.0_f64;
        let mut at = 0.0_f32;
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_fn(x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            if !want.is_finite() || !have.is_finite() {
                // NaN on one side and not the other is a real disagreement.
                assert_eq!(
                    want.is_finite(),
                    have.is_finite(),
                    "{call} at x = {x}: mirror finite = {}, GPU finite = {} \
                     (GPU gave {have})",
                    want.is_finite(),
                    have.is_finite()
                );
                continue;
            }
            let d = if want.abs() < 0.1 {
                (have - want).abs() as f64
            } else {
                ((have - want) / want).abs() as f64
            };
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < budget,
            "GPU ({}) vs f32 mirror for {call}: {worst:e} at x = {at}, budget {budget:e}",
            gpu.adapter_name()
        );
    }

    // hzeta takes two arguments, so it needs its own dispatch per q.
    for q in [0.25_f32, 0.5, 1.0, 2.0, 7.5] {
        let call = format!("petir_hzeta(x, {q:?})");
        let probes: Vec<f32> = (1..=200).map(|i| 1.05 + 0.15 * i as f32).collect();
        let got = gpu
            .eval_map(
                &[GAMMA, PSI_ZETA],
                &call,
                &[],
                &probes,
                KernelParams::default(),
            )
            .expect("non-empty probe");
        let mut worst = 0.0_f64;
        for (k, &s) in probes.iter().enumerate() {
            let want = mirror_psi_zeta::hzeta(s, q);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            if want.is_finite() && have.is_finite() && want.abs() > 1e-6 {
                worst = worst.max(((have - want) / want).abs() as f64);
            }
        }
        assert!(
            worst < 5e-5,
            "GPU ({}) vs f32 mirror for hzeta(s, {q}): {worst:e}",
            gpu.adapter_name()
        );
    }
}

/// The shader agrees with the mirror on `zeta`'s two refusals as well as on
/// its numbers: `NaN` at `s = 1`, and `NaN` below `s = -34` where `f32`
/// cannot carry the reflection branch.
///
/// The numerical sweep above cannot reach either — it filters both out — so
/// a shader that returned a plausible number at the pole would pass it.
#[test]
fn gpu_zeta_refuses_exactly_where_the_mirror_does() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_zeta_refuses_exactly_where_the_mirror_does: no GPU adapter");
        return;
    };
    // The pole, the f32 refusal, two trivial zeros that must survive it, and
    // ordinary points either side.
    let probes: Vec<f32> = vec![
        1.0, -34.5, -35.0, -99.5, -34.0, -100.0, -2.0, 0.5, 2.0, 20.0,
    ];
    let got = gpu
        .eval_map(
            &[GAMMA, PSI_ZETA],
            "petir_zeta(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    for (k, &s) in probes.iter().enumerate() {
        let want = mirror_psi_zeta::zeta(s);
        let have = got.get(k).copied().unwrap_or(0.0);
        assert_eq!(
            want.is_nan(),
            have.is_nan(),
            "petir_zeta at s = {s}: mirror NaN = {}, GPU NaN = {} (GPU gave {have})",
            want.is_nan(),
            have.is_nan()
        );
        if want == 0.0 {
            assert_eq!(have, 0.0, "the trivial zero at s = {s} must stay exact");
        }
    }
}

/// The Debye functions `D_1` .. `D_6` on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// One dispatch per order over `x` in `[0.01, 16]`, which crosses all five
/// branches — the small-argument quadratic, the Chebyshev series, the
/// exponential sum, the closed form and the asymptote. `DEBYE` has no
/// dependency on another source; `petir_debye` carries its own Chebyshev
/// evaluation.
///
/// The comparison is relative throughout, with no absolute fallback: `D_n` is
/// strictly positive and decreasing on `[0, inf)`, with no zero anywhere, so
/// unlike `psi`, `zeta` and the Bessel functions there is no point where a
/// relative figure degenerates.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | order | worst | at |
/// |---|---|---|
/// | `D_1` | 2.994e-07 | 3.875 |
/// | `D_2` | 4.956e-07 | 4.0 |
/// | `D_3` | 6.381e-07 | 3.9375 |
/// | `D_4` | 6.913e-07 | 3.75 |
/// | `D_5` | 8.574e-07 | 3.9375 |
/// | `D_6` | 1.630e-06 | 4.5 |
///
/// **Not bit-identical, and the reason is not what the ledger's general rule
/// would predict.** Five of the six worst points sit at or just below
/// `x = 4`, which is the *Chebyshev* branch — pure arithmetic, calling no
/// transcendental builtin at all. By the rule stated in
/// `docs/wgsl-coverage.md` that branch should match exactly, and it does not.
///
/// `the_inline_coefficient_array_is_what_costs_bit_identity` isolates the
/// cause: it is the inline `array<f32, 17>` literal, not the recurrence. The
/// budget below is set at 1e-04, about sixty times the worst measurement.
///
/// This is GPU-vs-mirror and **not an accuracy claim** — `mirror_debye`
/// measures its own distance from `f64` separately, at 2.8e-07 .. 1.9e-06
/// over the same window, which is the same size. The device contributes
/// about as much as `f32` itself does here.
#[test]
fn gpu_debye_family_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_debye_family_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let probes: Vec<f32> = (1..=256).map(|i| 0.0625 * i as f32).collect();

    for n in 1..=6u32 {
        let call = format!("petir_debye({n}u, x)");
        let got = gpu
            .eval_map(&[DEBYE], &call, &[], &probes, KernelParams::default())
            .expect("non-empty probe");
        let mut worst = 0.0_f64;
        let mut at = 0.0_f32;
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_debye::debye_n(n, x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            assert!(
                want.is_finite() && have.is_finite(),
                "D_{n} at x = {x}: mirror {want}, GPU {have}"
            );
            let d = ((have - want) / want).abs() as f64;
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "GPU ({}) vs f32 mirror for D_{n}: {worst:e} at x = {at}",
            gpu.adapter_name()
        );
    }
}

/// **Why the Debye Chebyshev branch is not bit-identical although it is pure
/// arithmetic: the inline coefficient array, not the recurrence.**
///
/// `docs/wgsl-coverage.md` states the general rule that a kernel built only
/// from arithmetic may be held to bit-identity, because IEEE-754 pins it to
/// one correctly-rounded answer. `petir_debye`'s `x <= 4` branch is exactly
/// such a kernel — Clenshaw over 17 constants, no `exp`, no `log`, no `sin` —
/// and it misses by up to 4 ulp. The rule needed a qualification, and this
/// test is the experiment that found it.
///
/// # Methodology
///
/// A controlled A/B over the *same* 17 coefficients (`adeb1_cs`), the same 64
/// arguments, and the same Clenshaw recurrence, differing in one thing only:
///
/// * `petir_debye_cheb1` holds them in an inline `array<f32, 17>` literal, so
///   the shader compiler sees compile-time constants;
/// * `petir_cheb_eval` reads them from the storage buffer, so it cannot.
///
/// `a = -1, b = 1` makes `petir_cheb_eval`'s argument scaling the identity,
/// which is what lets the two be compared at all.
///
/// # Result, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | coefficients | bit-exact against the CPU |
/// |---|---|
/// | storage buffer | **64 / 64** |
/// | inline literal | 34 / 64 |
///
/// The buffer-fed form is *exactly* right at every point. The inline form is
/// wrong at 30 of 64, always by one ulp downward at the Chebyshev step, which
/// compounds to 4 ulp by the end of `petir_debye`. So the recurrence, the
/// coefficients and the transcription are all correct; what differs is what
/// the shader compiler is allowed to do once the operands are constants.
///
/// # What this means for the rest of the ledger, stated as a prediction
///
/// `bessel.wgsl`, `gamma.wgsl` and `psi_zeta.wgsl` all embed their series as
/// inline literals, so they should show the same signature — and one figure
/// already on the ledger fits it: `I_0_scaled` calls no `exp` at all and
/// still sits 2.364e-07 from its mirror, which had no explanation before
/// this. That is a **hypothesis, not a result**; what would settle it is a
/// buffer-fed copy of the `bi0_cs` series compared the same way. Filed
/// rather than asserted here.
#[test]
fn the_inline_coefficient_array_is_what_costs_bit_identity() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP the_inline_coefficient_array_is_what_costs_bit_identity: no GPU adapter");
        return;
    };

    // GSL's adeb1_cs, the same 17 values debye.wgsl and mirror_debye hold.
    const ADEB1: [f32; 17] = [
        2.40065972,
        0.193721304,
        -0.00623291246,
        0.000351117477,
        -2.28222467e-05,
        1.58054679e-06,
        -1.1353782e-07,
        8.35833612e-09,
        -6.26442479e-10,
        4.76033489e-11,
        -3.6574154e-12,
        2.835431e-13,
        -2.21473e-14,
        1.7409e-15,
        -1.376e-16,
        1.09e-17,
        -9e-19,
    ];

    /// Clenshaw in GSL's convention, on the CPU, in `f32`.
    fn clenshaw(x: f32) -> f32 {
        let (mut d, mut dd) = (0.0_f32, 0.0_f32);
        let y2 = 2.0 * x;
        for j in (1..=16).rev() {
            let t = d;
            d = y2 * d - dd + ADEB1[j];
            dd = t;
        }
        x * d - dd + 0.5 * ADEB1[0]
    }

    // debye.wgsl's own argument mapping, so these are arguments the kernel
    // really sees.
    let ys: Vec<f32> = (1..=64)
        .map(|i| {
            let x = 0.0625 * i as f32;
            x * x / 8.0 - 1.0
        })
        .collect();

    let inline = gpu
        .eval_map(
            &[DEBYE],
            "petir_debye_cheb(1u, x)",
            &[],
            &ys,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    let buffered = gpu
        .eval_map(
            &[CHEB],
            "petir_cheb_eval(0u, 17u, -1.0, 1.0, x)",
            &ADEB1,
            &ys,
            KernelParams::default(),
        )
        .expect("non-empty probe");

    let exact_buffered = ys
        .iter()
        .enumerate()
        .filter(|(k, &y)| buffered.get(*k) == Some(&clenshaw(y)))
        .count();
    let exact_inline = ys
        .iter()
        .enumerate()
        .filter(|(k, &y)| inline.get(*k) == Some(&clenshaw(y)))
        .count();

    // The control: with the coefficients opaque to the compiler, the device
    // reproduces the CPU exactly. If this ever stops holding, the finding
    // below is no longer about inlining and the docs need rewriting.
    assert_eq!(
        exact_buffered,
        ys.len(),
        "GPU ({}) buffer-fed Clenshaw is documented as bit-identical to the          CPU at every one of {} points, and matched {exact_buffered}",
        gpu.adapter_name(),
        ys.len()
    );

    // The finding: the inline-literal form is not, although nothing else
    // about it differs.
    assert!(
        exact_inline < ys.len(),
        "GPU ({}) inline-literal Clenshaw matched the CPU at all {} points.          That contradicts the measurement this test records (34/64) and the          explanation built on it in docs/wgsl-coverage.md — re-measure and          rewrite those rather than deleting this assertion",
        gpu.adapter_name(),
        ys.len()
    );

    // And the disagreement is small: one ulp per Chebyshev step.
    let worst_ulp = ys
        .iter()
        .enumerate()
        .map(|(k, &y)| {
            let want = clenshaw(y).to_bits() as i64;
            let have = inline.get(k).copied().unwrap_or(f32::NAN).to_bits() as i64;
            (have - want).abs()
        })
        .max()
        .unwrap_or(0);
    assert!(
        worst_ulp <= 2,
        "the inline-literal Chebyshev branch is documented as differing by at          most one ulp per step, and differed by {worst_ulp}"
    );
}

/// The real dilogarithm on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// One dispatch over `x` in `[-15, 15]`, which crosses **all seven** branches
/// of `petir_dilog_xge0` plus the negative-axis duplication. `DILOG` has no
/// dependency on another source and no coefficient tables.
///
/// The comparison is **absolute, not relative**, and that is the whole point
/// of the window. `Li_2` has a real zero at `x = 12.595170`, inside the
/// probe range and squarely in the `x > 2` inversion branch, where
/// `(1/2) ln^2 x` passes through `pi^2/3`. A relative figure there measures
/// where a probe landed, not what the device did. `mirror_dilog` records the
/// per-branch relative numbers where they are meaningful.
///
/// # Result, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// Worst absolute difference **3.815e-06 at `x = -9.85`**, where `Li_2` is
/// about -5.9 — so roughly one `f32` ulp of the value, not of the unit.
///
/// **Not bit-identical**, and every branch here calls `log`, which WGSL
/// specifies to an ULP bound rather than correct rounding. The budget is
/// 1e-04, about twenty-six times the measurement.
///
/// Note the worst point is on the **negative** axis, which reaches
/// `petir_dilog_xge0` twice through the duplication formula and therefore
/// accumulates two branches' worth of `log` disagreement. That is the
/// expected place for it to be, and it is not the same place as the
/// mirror-vs-`f64` worst, which is at `Li_2`'s zero.
///
/// This is GPU-vs-mirror and **not an accuracy claim** — `mirror_dilog`
/// measures its own distance from `f64` per branch.
#[test]
fn gpu_dilog_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_dilog_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let probes: Vec<f32> = (0..=600).map(|i| -15.0 + 0.05 * i as f32).collect();
    let got = gpu
        .eval_map(
            &[DILOG],
            "petir_dilog(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");

    let (mut worst, mut at) = (0.0_f64, 0.0_f32);
    for (k, &x) in probes.iter().enumerate() {
        let want = mirror_dilog::dilog(x);
        let have = got.get(k).copied().unwrap_or(f32::NAN);
        assert!(
            want.is_finite() && have.is_finite(),
            "Li_2({x}): mirror {want}, GPU {have}"
        );
        let d = (have - want).abs() as f64;
        if d > worst {
            worst = d;
            at = x;
        }
    }
    assert!(
        worst < 1e-4,
        "GPU ({}) vs f32 mirror for Li_2: {worst:e} absolute at x = {at}",
        gpu.adapter_name()
    );
}

/// The Airy functions on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// Four entry points, each over the window where it is representable. The
/// comparison is **absolute for `Ai` and `Bi`** and relative for the scaled
/// forms, and the split is not arbitrary: `Ai` and `Bi` both oscillate
/// through zeros below `x = -1`, where a relative figure measures where the
/// probe grid fell rather than what the device did. The scaled forms on the
/// positive axis have no zeros and are `O(1)`, so relative works there.
///
/// `Bi` is probed only to `x = 25`, below where it overflows `f32` at
/// `x = 26.07`. `mirror_airy` pins the overflow behaviour itself.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | kernel | worst | at | measure |
/// |---|---|---|---|
/// | `Ai` | 1.839e-06 | -29.925 | absolute |
/// | `Bi` | 1.963e-06 | -21.525 | absolute |
/// | `Ai_scaled` | 3.375e-07 | 0.97 | relative |
/// | `Bi_scaled` | 3.137e-07 | -0.852 | relative |
///
/// Both unscaled worst points are at the far negative end, where `theta` is
/// largest — the same place `mirror_airy` measures the `f32` phase error, and
/// the expected place given that `cos` and `sin` are the builtins WGSL
/// specifies loosest (an absolute 2^-11 inside `[-pi, pi]`, implementation-
/// defined outside). The scaled forms, probed on `[-2, 25]` where no large
/// phase arises, come in at one `f32` ulp.
///
/// **Not bit-identical.** Every branch calls `sqrt` at minimum and the
/// oscillatory one calls `cos`/`sin`; the series are also held in inline
/// `array<f32, N>` literals, which `docs/wgsl-coverage.md` records as costing
/// bit-identity on its own. Budgets are 1e-04 absolute and 1e-04 relative,
/// about fifty times the worst measurement.
///
/// This is GPU-vs-mirror and **not an accuracy claim** — `mirror_airy`
/// measures its own distance from `f64` per branch, and records that the
/// oscillatory branch's error is irreducible in `f32`.
#[test]
fn gpu_airy_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_airy_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let osc: Vec<f32> = (0..=400).map(|i| -30.0 + 0.075 * i as f32).collect();
    let pos: Vec<f32> = (0..=400).map(|i| -2.0 + 0.0675 * i as f32).collect();

    for (call, mirror_fn, probes, relative) in [
        (
            "petir_airy_ai(x)",
            mirror_airy::airy_ai as fn(f32) -> f32,
            &osc,
            false,
        ),
        (
            "petir_airy_bi(x)",
            mirror_airy::airy_bi as fn(f32) -> f32,
            &osc,
            false,
        ),
        (
            "petir_airy_ai_scaled(x)",
            mirror_airy::airy_ai_scaled as fn(f32) -> f32,
            &pos,
            true,
        ),
        (
            "petir_airy_bi_scaled(x)",
            mirror_airy::airy_bi_scaled as fn(f32) -> f32,
            &pos,
            true,
        ),
    ] {
        let got = gpu
            .eval_map(&[AIRY], call, &[], probes, KernelParams::default())
            .expect("non-empty probe");
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_fn(x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            assert!(
                want.is_finite() && have.is_finite(),
                "{call} at x = {x}: mirror {want}, GPU {have}"
            );
            let d = if relative && want.abs() > 1e-3 {
                (((have - want) / want) as f64).abs()
            } else {
                (have - want).abs() as f64
            };
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "GPU ({}) vs f32 mirror for {call}: {worst:e} at x = {at}",
            gpu.adapter_name()
        );
    }
}

/// Lambert `W`, both branches, on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// `W_0` over `[-1/e, 100]` and `W_{-1}` geometrically over `[-1/e, -1e-7)`,
/// which is where its interesting behaviour is — it runs to `-16` there and
/// is the branch upstream's stopping rule mishandles in `f32`.
///
/// Relative, with no absolute fallback: `W_0` has a zero at `x = 0`, so that
/// one point is skipped rather than the measure being changed for the whole
/// sweep.
///
/// **This kernel iterates**, which is unusual for this set — every other
/// shader here is a fixed-length evaluation. A device whose `exp` differs
/// from `libm`'s by an ulp can therefore take a *different number of steps*,
/// not merely a slightly different value, so the budget is set with that in
/// mind rather than at one ulp.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | kernel | worst relative | at |
/// |---|---|---|
/// | `W_0` | 1.383e-07 | 9.669 |
/// | `W_{-1}` | 5.792e-07 | -3.411e-01 |
///
/// One to five `f32` ulp — notably good for an iterating kernel, which says
/// the device took the same number of steps as the mirror at every probe. The
/// budget is 1e-04, about two hundred times the worst measurement, set loose
/// deliberately: a device whose `exp` differs by an ulp could take a
/// different step count somewhere and land further out without being wrong.
///
/// This is GPU-vs-mirror and **not an accuracy claim** — `mirror_lambert`
/// measures its own distance from `f64` (1.161e-07) and from the defining
/// identity separately.
#[test]
fn gpu_lambert_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_lambert_matches_the_cpu_mirror: no GPU adapter");
        return;
    };
    const ONE_OVER_E: f32 = 0.36787945;

    let w0_probes: Vec<f32> = (0..=400)
        .map(|i| -ONE_OVER_E + (100.0 + ONE_OVER_E) * (i as f32 / 400.0))
        .collect();
    let wm1_probes: Vec<f32> = (0..=400)
        .map(|i| {
            let t = i as f32 / 400.0;
            -ONE_OVER_E * (1e-7_f32 / ONE_OVER_E).powf(t)
        })
        .collect();

    for (call, mirror_fn, probes) in [
        (
            "petir_lambert_w0(x)",
            mirror_lambert::lambert_w0 as fn(f32) -> f32,
            &w0_probes,
        ),
        (
            "petir_lambert_wm1(x)",
            mirror_lambert::lambert_wm1 as fn(f32) -> f32,
            &wm1_probes,
        ),
    ] {
        let got = gpu
            .eval_map(&[LAMBERT], call, &[], probes, KernelParams::default())
            .expect("non-empty probe");
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_fn(x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            assert_eq!(
                want.is_finite(),
                have.is_finite(),
                "{call} at x = {x:e}: mirror {want}, GPU {have}"
            );
            if !want.is_finite() || want.abs() < 1e-6 {
                continue;
            }
            let d = (((have - want) / want) as f64).abs();
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "GPU ({}) vs f32 mirror for {call}: {worst:e} at x = {at:e}",
            gpu.adapter_name()
        );
    }
}

/// The Clausen function on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// Two sweeps: one over a single period `[-pi, pi]`, and one at large
/// argument (`[1e4, 5e5]`) which exercises the **redesigned `f32` argument
/// reduction** rather than the Chebyshev branch. The second is the point of
/// this test — the reduction is the part that is not a transcription.
///
/// Absolute, because `Cl_2` has zeros at `0` and `pi` and both sweeps cross
/// them.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | sweep | worst absolute | at |
/// |---|---|---|
/// | one period, `[-pi, pi]` | 7.339e-07 | -3.079 |
/// | large argument, `[1e4, 5e5]` | 9.947e-07 | 4.816e+05 |
///
/// **The two are within a factor of 1.4**, which is the result worth having:
/// the redesigned reduction costs essentially nothing on the GPU even at
/// `4.8e+05`, just below the refusal. Device and mirror agree on the period
/// count everywhere, so the eight-bit head is doing its job identically on
/// both.
///
/// Not bit-identical — the kernel calls `log` and the series is an inline
/// literal array. The budget is 1e-04, about a hundred times the worst
/// measurement.
///
/// This is GPU-vs-mirror and **not an accuracy claim**: `mirror_clausen`
/// measures its own distance from `f64` at 4.521e-07 over one period.
#[test]
fn gpu_clausen_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_clausen_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let one_period: Vec<f32> = (0..=400)
        .map(|i| -core::f32::consts::PI + core::f32::consts::TAU * (i as f32 / 400.0))
        .collect();
    let large: Vec<f32> = (0..=400)
        .map(|i| 1.0e4 + (5.0e5 - 1.0e4) * (i as f32 / 400.0))
        .collect();

    for (label, probes) in [("one period", &one_period), ("large argument", &large)] {
        let got = gpu
            .eval_map(
                &[CLAUSEN],
                "petir_clausen(x)",
                &[],
                probes,
                KernelParams::default(),
            )
            .expect("non-empty probe");
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_clausen::clausen(x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            assert_eq!(
                want.is_finite(),
                have.is_finite(),
                "Cl_2({x:e}): mirror {want}, GPU {have}"
            );
            if !want.is_finite() {
                continue;
            }
            let d = (have - want).abs() as f64;
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "GPU ({}) vs f32 mirror for Cl_2, {label}: {worst:e} at x = {at:e}",
            gpu.adapter_name()
        );
    }
}

/// The transport integrals on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// One dispatch per order over `x` in `(0, 35]`, which crosses all four
/// branches — the small-argument power, the Chebyshev series, the
/// exponential-image tail, and the saturation point where the tail is
/// discarded and `J(n, inf)` returned exactly (22.25 to 29.6, per order).
///
/// Relative, with a floor: `J(n, .)` is zero at the origin and grows
/// monotonically, so away from `x = 0` a relative figure is well defined.
///
/// **The tail branch runs a nested loop whose trip count depends on `x`**, so
/// like `lambert` this is a kernel where a device could in principle do a
/// different amount of work, not merely round differently.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// | order | worst relative | at |
/// |---|---|---|
/// | `J(2)` | 1.133e-07 | 2.45 |
/// | `J(3)` | 2.683e-07 | 4.69 |
/// | `J(4)` | 6.715e-07 | 4.13 |
/// | `J(5)` | 2.222e-06 | 4.41 |
///
/// Three of the four worst points sit just past `x = 4`, the Chebyshev/tail
/// join, which is also where `mirror_transport` measures its own worst
/// against `f64` — so the device is not adding a failure mode of its own,
/// it is amplifying the one the formulation already has there. The figures
/// track the mirror's own (7.092e-08 .. 1.295e-06) within a factor of two,
/// which says the device took the same number of tail images at every probe.
///
/// Not bit-identical: the kernel calls `exp` and `log`, and the series are
/// inline literal arrays. The budget is 1e-04, about fifty times the worst.
///
/// This is GPU-vs-mirror and **not an accuracy claim**.
#[test]
fn gpu_transport_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_transport_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let probes: Vec<f32> = (1..=500).map(|i| 0.07 * i as f32).collect();

    for n in 2..=5u32 {
        let call = format!("petir_transport({n}u, x)");
        let got = gpu
            .eval_map(&[TRANSPORT], &call, &[], &probes, KernelParams::default())
            .expect("non-empty probe");
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for (k, &x) in probes.iter().enumerate() {
            let want = mirror_transport::transport(n, x);
            let have = got.get(k).copied().unwrap_or(f32::NAN);
            assert!(
                want.is_finite() && have.is_finite(),
                "J({n}, {x}): mirror {want}, GPU {have}"
            );
            if want.abs() < 1e-6 {
                continue;
            }
            let d = (((have - want) / want) as f64).abs();
            if d > worst {
                worst = d;
                at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "GPU ({}) vs f32 mirror for J({n}): {worst:e} at x = {at}",
            gpu.adapter_name()
        );
    }
}

/// The inverse-tangent integral on the GPU against the `f32` CPU mirror.
///
/// # Methodology
///
/// One geometric sweep over `|x|` in `[1e-05, 1e+08]`, mirrored to negative
/// `x`, which crosses every branch: the small-argument identity, both
/// Chebyshev branches either side of `|x| = 1`, and the closed form past the
/// retargeted large cut at 2896.3.
///
/// Relative, with a floor near the origin where `Ti_2` has its only zero.
///
/// # Results, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`
///
/// Worst relative **1.444e-07 at `x = 7.516`** — one `f32` ulp, and just
/// inside the reflected Chebyshev branch rather than at any boundary.
///
/// Not bit-identical: the kernel calls `log` and the series is an inline
/// literal array. The budget is 1e-04, about seven hundred times the worst.
///
/// This is GPU-vs-mirror and **not an accuracy claim** — `mirror_atanint`
/// measures its own distance from `f64` at 1.595e-07.
#[test]
fn gpu_atanint_matches_the_cpu_mirror() {
    let Some(gpu) = GpuContext::probe() else {
        eprintln!("SKIP gpu_atanint_matches_the_cpu_mirror: no GPU adapter");
        return;
    };

    let mut probes: Vec<f32> = Vec::new();
    for i in 0..=250 {
        let t = i as f32 / 250.0;
        let x = 1e-5_f32 * (1e13_f32).powf(t);
        probes.push(x);
        probes.push(-x);
    }

    let got = gpu
        .eval_map(
            &[ATANINT],
            "petir_atanint(x)",
            &[],
            &probes,
            KernelParams::default(),
        )
        .expect("non-empty probe");
    let (mut worst, mut at) = (0.0_f64, 0.0_f32);
    for (k, &x) in probes.iter().enumerate() {
        let want = mirror_atanint::atanint(x);
        let have = got.get(k).copied().unwrap_or(f32::NAN);
        assert!(
            want.is_finite() && have.is_finite(),
            "Ti_2({x:e}): mirror {want}, GPU {have}"
        );
        if want.abs() < 1e-6 {
            continue;
        }
        let d = (((have - want) / want) as f64).abs();
        if d > worst {
            worst = d;
            at = x;
        }
    }
    assert!(
        worst < 1e-4,
        "GPU ({}) vs f32 mirror for Ti_2: {worst:e} at x = {at:e}",
        gpu.adapter_name()
    );
}
