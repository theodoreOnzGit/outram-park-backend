//! **Cross-lineage verification of the GPU polynomial root finders** in
//! `src/gpu/shaders/surface_distance.wgsl`, against PETIR's closed-form
//! solvers.
//!
//! # Why this test exists
//!
//! `surface_distance.wgsl` carries its own `quadratic_real_roots`,
//! `cubic_real_roots` and `quartic_real_roots`, because ray-surface
//! intersection needs them on the device. Until now they were exercised only
//! *indirectly* — through the whole surface-distance pipeline, where a root
//! that is slightly wrong usually still produces a plausible distance and the
//! comparison is against this crate's own CPU mirror, which shares the
//! algorithm.
//!
//! This checks them **directly, against a different lineage**:
//!
//! | | algorithm | provenance |
//! |---|---|---|
//! | this crate, on the GPU | bracket the critical points, then bisect | written here for ray tracing |
//! | `petir::poly::quartic` | closed-form Ferrari / Vorotilov | ported from the `roots` crate, **bit-identical to upstream across 71 roots** |
//!
//! Two independent methods from two independent sources agreeing on the same
//! polynomials is evidence for both. Agreeing with yourself is not.
//!
//! # `f32`, deliberately
//!
//! The shader is `f32` — a baseline WebGPU device has nothing wider — so the
//! comparison is held to an `f32` budget, not PETIR's usual `1e-15`. The
//! reference is computed in `f64` and the tolerance is stated per test with
//! the measured number beside it.
//!
//! # No adapter is not a pass
//!
//! Without a GPU these print `SKIP` and return, per this crate's convention.
//! That proves nothing, which is why the coefficient cases are also checked
//! against PETIR on the CPU in `the_test_cases_are_solvable_at_all`, a test
//! that needs no device and would catch a badly-chosen case set.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::{probe, GpuContext};

/// The shader under test, included verbatim so it cannot drift from the one
/// the library ships.
const SURFACE_DISTANCE_WGSL: &str = include_str!("../src/gpu/shaders/surface_distance.wgsl");

/// A second entry point appended to that shader.
///
/// It reuses the module's existing `coeffs` (binding 1) and `result`
/// (binding 7) declarations rather than introducing new ones, so the shader
/// text stays unmodified above it. `wgpu` builds the bind-group layout from
/// the selected entry point, so only those two bindings are required.
///
/// Layout: `coeffs` holds 5 values per case (descending, `a x^4 + ... + e`),
/// `result` holds 5 per case — the root count followed by four roots.
const ROOTCHECK_ENTRY: &str = r#"
@compute @workgroup_size(64)
fn petir_rootcheck(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n_cases = arrayLength(&coeffs) / 5u;
    if (i >= n_cases) { return; }
    let b = i * 5u;
    let rts = quartic_real_roots(
        coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u]
    );
    let o = i * 5u;
    result[o] = f32(rts.n);
    result[o + 1u] = rts.r[0];
    result[o + 2u] = rts.r[1];
    result[o + 3u] = rts.r[2];
    result[o + 4u] = rts.r[3];
}
"#;

/// Quartics spanning the branch structure, as `(a, b, c, d, e)` descending.
///
/// Chosen to cover what the shader actually branches on: four distinct roots,
/// two, none, a degenerate leading coefficient that falls through to the
/// cubic, a biquadratic, and a case with a root at the origin.
fn cases() -> Vec<[f64; 5]> {
    vec![
        // (x-1)(x-2)(x-3)(x-4)
        [1.0, -10.0, 35.0, -50.0, 24.0],
        // 2(x-1)(x-2)(x-3)(x-4), scaled leading coefficient
        [2.0, -20.0, 70.0, -100.0, 48.0],
        // x^4 - 1: two real roots, two complex
        [1.0, 0.0, 0.0, 0.0, -1.0],
        // x^4 + 1: no real roots
        [1.0, 0.0, 0.0, 0.0, 1.0],
        // biquadratic (x^2-1)(x^2-4)
        [1.0, 0.0, -5.0, 0.0, 4.0],
        // a root at the origin: x(x-1)(x-2)(x-3)
        [1.0, -6.0, 11.0, -6.0, 0.0],
        // leading coefficient zero -> the cubic fallback
        [0.0, 1.0, -6.0, 11.0, -6.0],
        // (x+3)(x+1)(x-1)(x-2)
        [1.0, 1.0, -7.0, -1.0, 6.0],
        // well-separated, larger magnitudes
        [1.0, -20.0, 140.0, -400.0, 384.0],
        // a negative leading coefficient
        [-1.0, 10.0, -35.0, 50.0, -24.0],
    ]
}

/// PETIR's real roots of a descending quartic, ascending, in `f64`.
fn petir_roots(c: &[f64; 5]) -> Vec<f64> {
    let set = petir::poly::quartic::roots_quartic(c[0], c[1], c[2], c[3], c[4]);
    let mut v: Vec<f64> = set.as_slice().to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

/// Dispatch `petir_rootcheck` over the case set, returning 5 floats per case.
fn run_on_gpu(gpu: &GpuContext, flat: &[f32], n_cases: usize) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let mut source = String::from(SURFACE_DISTANCE_WGSL);
    source.push_str(ROOTCHECK_ENTRY);

    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("surface_distance + rootcheck"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    let mut coeff_bytes = Vec::with_capacity(flat.len() * 4);
    for v in flat {
        coeff_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let coeff_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coeffs"),
            contents: &coeff_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

    let out_len = n_cases * 5;
    let out_size = (out_len * std::mem::size_of::<f32>()) as u64;
    let result_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("result"),
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: out_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("rootcheck"),
            layout: None,
            module: &module,
            entry_point: Some("petir_rootcheck"),
            compilation_options: Default::default(),
            cache: None,
        });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: coeff_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: result_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(n_cases.div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&result_buf, 0, &read_buf, 0, out_size);
    gpu.queue.submit(Some(encoder.finish()));

    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll");
    let mapped = slice.get_mapped_range().expect("mapped range");
    let out: Vec<f32> = mapped
        .chunks_exact(4)
        .filter_map(|c| <[u8; 4]>::try_from(c).ok())
        .map(f32::from_le_bytes)
        .collect();
    drop(mapped);
    read_buf.unmap();
    out
}

/// The case set is solvable and non-trivial, checked without a device.
///
/// A GPU test that skips proves nothing, so this runs everywhere. It also
/// guards against the failure mode where every case quietly has zero real
/// roots and the GPU comparison passes by agreeing about nothing.
#[test]
fn the_test_cases_are_solvable_at_all() {
    let all = cases();
    let total: usize = all.iter().map(|c| petir_roots(c).len()).sum();
    assert!(
        total >= 25,
        "the case set yields only {total} real roots -- too few to be a test"
    );
    // At least one case must have no real roots, or the "n == 0" path is
    // never exercised.
    assert!(
        all.iter().any(|c| petir_roots(c).is_empty()),
        "no case exercises the zero-real-roots path"
    );
    // And at least one must have four.
    assert!(
        all.iter().any(|c| petir_roots(c).len() == 4),
        "no case exercises the four-real-roots path"
    );
}

/// The GPU quartic root finder agrees with PETIR's closed form.
///
/// # Methodology
///
/// Ten quartics (see [`cases`]) dispatched through
/// `surface_distance.wgsl`'s `quartic_real_roots` in `f32`, compared against
/// `petir::poly::quartic::roots_quartic` in `f64`.
///
/// Matched by **nearest root** rather than by position, because the two
/// disagree about multiplicity bookkeeping: PETIR returns *distinct* roots,
/// while the shader's bisection can return a tight pair straddling a double
/// root. Every root PETIR finds must have a GPU root near it; the converse is
/// checked separately below, so a shader returning spurious extra roots is
/// still caught.
///
/// # Results
///
/// Worst scaled root difference **1.669e-06**, on `(x-1)(x-2)(x-3)(x-4)`,
/// measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
///
/// Every root PETIR finds, across all ten cases, has a GPU root within that.
///
/// **This is an order of magnitude looser than the ~1e-7 `f32` floor seen
/// elsewhere in this suite, and legitimately so.** The shader finds a root by
/// **bisecting to a tolerance**, not by evaluating a formula, so what is being
/// measured is the bracketing tolerance rather than a rounding of the answer.
/// Tightening it would mean more bisection steps, which is a cost decision for
/// ray tracing, not a correctness one — a surface intersection does not need
/// the root to `f32` machine precision, it needs it to well inside the
/// geometry's own tolerance.
///
/// That distinction matters for how a regression here should be read: a jump
/// to 1e-3 would mean the bracketing broke, while a drift to 3e-6 would not
/// necessarily mean anything at all.
#[test]
fn gpu_quartic_roots_agree_with_petirs_closed_form() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_quartic_roots_agree_with_petirs_closed_form: no GPU adapter");
        return;
    };

    let all = cases();
    let flat: Vec<f32> = all
        .iter()
        .flat_map(|c| c.iter().map(|&v| v as f32))
        .collect();
    let out = run_on_gpu(&gpu, &flat, all.len());

    let mut worst = 0.0_f64;
    let mut worst_case = 0usize;
    for (idx, c) in all.iter().enumerate() {
        let want = petir_roots(c);
        let base = idx * 5;
        let n_gpu = out.get(base).copied().unwrap_or(0.0) as usize;
        let got: Vec<f64> = (0..n_gpu.min(4))
            .filter_map(|k| out.get(base + 1 + k).copied())
            .map(|v| v as f64)
            .collect();

        for w in &want {
            let nearest = got
                .iter()
                .map(|g| (g - w).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(
                nearest.is_finite(),
                "case {idx} {c:?}: PETIR found root {w} but the GPU returned none"
            );
            let scale = w.abs().max(1.0);
            let rel = nearest / scale;
            if rel > worst {
                worst = rel;
                worst_case = idx;
            }
        }
    }

    assert!(
        worst < 1e-4,
        "worst scaled root difference {worst:e} on case {worst_case} ({:?}), device {}",
        all.get(worst_case),
        gpu.info.name
    );
}

/// Every root the GPU returns really is a root — no spurious extras.
///
/// # Why this is the other half
///
/// The test above checks that nothing PETIR finds is *missed*. This checks
/// that nothing the GPU returns is *invented*, by evaluating the polynomial
/// at each returned root. A finder that returned four arbitrary numbers would
/// pass the first test on the cases where PETIR finds fewer, and fail here.
///
/// # Results
///
/// Worst scaled residual **2.032e-09**, measured 2026-09-19 on
/// `llvmpipe (LLVM 20.1.2, 256 bits)`, and no case returned more roots than
/// the polynomial has.
///
/// The residual is scaled by the coefficient magnitude and `(1 + |x|)^4`, the
/// same measure `petir::poly::quartic` uses, so the two are directly
/// comparable: PETIR's `f64` closed form achieves **3.655e-18** on it. Nine
/// orders of magnitude separate them, which is the combined price of `f32` and
/// of bisection-to-tolerance rather than a closed form — and is still far
/// inside what a ray-surface intersection needs.
#[test]
fn every_root_the_gpu_returns_is_a_real_root() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP every_root_the_gpu_returns_is_a_real_root: no GPU adapter");
        return;
    };

    let all = cases();
    let flat: Vec<f32> = all
        .iter()
        .flat_map(|c| c.iter().map(|&v| v as f32))
        .collect();
    let out = run_on_gpu(&gpu, &flat, all.len());

    let mut worst = 0.0_f64;
    for (idx, c) in all.iter().enumerate() {
        let scale = c.iter().map(|v| v.abs()).fold(1.0_f64, f64::max);
        let base = idx * 5;
        let n_gpu = out.get(base).copied().unwrap_or(0.0) as usize;
        assert!(
            n_gpu <= 4,
            "case {idx}: the shader reported {n_gpu} roots, which a quartic cannot have"
        );
        for k in 0..n_gpu.min(4) {
            let x = match out.get(base + 1 + k) {
                Some(&v) => v as f64,
                None => continue,
            };
            // Horner on the descending coefficients.
            let mut v = 0.0_f64;
            for ci in c.iter() {
                v = v * x + ci;
            }
            let residual = v.abs() / (scale * (1.0 + x.abs()).powi(4));
            if residual > worst {
                worst = residual;
            }
        }
    }

    assert!(
        worst < 1e-5,
        "worst scaled residual {worst:e}, device {}",
        gpu.info.name
    );
}
