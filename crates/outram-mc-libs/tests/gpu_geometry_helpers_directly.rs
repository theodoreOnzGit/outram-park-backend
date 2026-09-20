//! **Direct verification of the geometry helpers** inside
//! `src/gpu/shaders/surface_distance.wgsl`, against closed forms and against a
//! `f64` root scan that shares no code with them.
//!
//! # Why this exists
//!
//! `surface_distance.wgsl` is checked two ways today: naga validation, and a
//! GPU-vs-CPU-mirror agreement test over whole queries. The mirror is a
//! line-by-line `f32` translation of the same algorithm, so that comparison
//! establishes *the translation is faithful* and nothing about whether the
//! algorithm is right. `gpu_root_finders_vs_petir.rs` closed that for the
//! polynomial solvers by bringing in a second lineage.
//!
//! The geometry helpers below it — `axis_cylinder`, `torus_distance`,
//! `smallest_positive_root` — were still reachable only through the top-level
//! `surface_distance` dispatch, where a helper that is slightly wrong usually
//! still produces a plausible distance. This reaches each one directly.
//!
//! **Extended 2026-09-19 to the five primitives underneath the solvers**:
//! `poly_eval`, `poly_abs_scale`, `root_bound`, `push_unique` and
//! `bisect_root`. Those were one level deeper still — reachable only through
//! `quadratic`/`cubic`/`quartic_real_roots`, which are themselves tested, so
//! the composition looked covered. It was not: see the injection table
//! below, where two defects are caught by the new tests **and by nothing
//! else in this file**.
//!
//! # What each is checked against
//!
//! | helper | reference | shares code? |
//! |---|---|---|
//! | `axis_cylinder` | the quadratic `\|d + t w\|^2 = r^2`, solved in `f64` | no |
//! | `smallest_positive_root` | the same quadratic formula in `f64` | no |
//! | `torus_distance` | a `f64` sign-change scan of the torus implicit function, then bisection | no |
//! | `cubic_real_roots` ordering | the ascending invariant `quartic_real_roots` depends on (bn:op-9s8.13) | — |
//! | `poly_eval` | `petir::poly::eval`, bit-identical to `gsl_poly_eval` | no — and it takes the **opposite** coefficient order |
//! | `poly_abs_scale` | `sum_i \|c_i\| \|x\|^{n-1-i}`, without Horner | no |
//! | `root_bound` | Cauchy's `1 + max\|c_i/c_0\|`, **and** that it contains the roots of polynomials built from known ones | no |
//! | `push_unique` | the set properties: no near-duplicates, order preserved, capacity respected | no |
//! | `bisect_root` | `petir::roots::BracketingSolver` (Brent), verified against GSL every iterate | no — a different method |
//!
//! The torus reference is the interesting one: the shader forms a quartic and
//! solves it, while the reference never builds a polynomial at all. It walks
//! `t` forward, evaluates the torus implicit function, and bisects the first
//! sign change that also satisfies the branch condition. Two methods with
//! nothing in common agreeing on the same ray is evidence; a mirror agreeing
//! with itself is not.
//!
//! # `f32`, deliberately
//!
//! The shader is `f32` and a baseline WebGPU device has nothing wider, so
//! every tolerance here is an `f32` budget with the measured number recorded
//! beside it.
//!
//! The `poly_eval` row carries the sharpest point. `petir::poly::eval` takes
//! **ascending** coefficients and the shader holds **descending** ones, so
//! the reference has to be reversed — and reversing a coefficient vector is
//! the single most likely defect in a routine like this, because it compiles,
//! returns a plausible number, and for a palindromic polynomial returns the
//! right one. The test therefore asserts both that the reversed reference
//! agrees *and* that the unreversed one disagrees, which is what makes the
//! first assertion mean anything.
//!
//! # Verified to catch real defects, by injection
//!
//! Each was applied to the shader and the named tests failed:
//!
//! | injected defect | caught by | and by nothing else? |
//! |---|---|---|
//! | `root_bound` returns `m` instead of `1 + m` | `gpu_root_bound_is_cauchys_bound_and_actually_bounds_the_roots` | **yes** |
//! | `push_unique`'s tolerance `1e-5` → `1e-2` | `gpu_push_unique_keeps_a_set` | **yes** |
//! | `poly_eval` reads `c[n-1-i]` — the coefficients reversed | `gpu_poly_eval_is_horner_in_descending_order`, and downstream `gpu_cubic_roots_come_back_ascending` and `gpu_bisect_root_converges_to_the_root_petir_finds` | no |
//!
//! The first two are the whole argument for this section: both are real
//! defects in routines the solver tests exercise on every case, and neither
//! was visible from those tests. A root bound that is too small silently
//! loses roots outside it; a duplicate tolerance a thousand times too loose
//! silently merges distinct ones. Each produces fewer roots, not wrong ones,
//! and a ray-tracing kernel that finds fewer intersections still returns a
//! distance.
//!
//! The third shows the contrast: a reversed `poly_eval` is so destructive
//! that three tests see it. It is the defects that are *nearly* invisible
//! that need a direct test.
//!
//! # No adapter is not a pass
//!
//! Without a GPU these print `SKIP` and return. The references are therefore
//! also exercised against each other on the CPU in
//! `the_torus_reference_and_the_case_set_are_sound`, which needs no device —
//! a case set that happened to miss every torus would otherwise let the GPU
//! comparison pass by agreeing about nothing.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::{probe, GpuContext};

/// The shader under test, included verbatim so it cannot drift from the one
/// the library ships.
const SURFACE_DISTANCE_WGSL: &str = include_str!("../src/gpu/shaders/surface_distance.wgsl");

/// Entry points appended to that shader, one per helper.
///
/// Each reuses the module's existing `coeffs` (binding 1) and `result`
/// (binding 7) declarations rather than introducing new ones, so the shader
/// text above stays byte-identical to what ships. `wgpu` builds the
/// bind-group layout from the selected entry point, so only those two
/// bindings are required.
const PROBE_ENTRIES: &str = r#"
// (d1, d2, w1, w2, radius, coincident) -> distance
@compute @workgroup_size(64)
fn probe_axis_cylinder(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 6u) { return; }
    let b = i * 6u;
    result[i] = axis_cylinder(
        coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u],
        coeffs[b + 5u] != 0.0
    );
}

// (p1, p2, pax, u1, u2, uax, a, b, c, coincident) -> distance
@compute @workgroup_size(64)
fn probe_torus_distance(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 10u) { return; }
    let o = i * 10u;
    result[i] = torus_distance(
        coeffs[o], coeffs[o + 1u], coeffs[o + 2u],
        coeffs[o + 3u], coeffs[o + 4u], coeffs[o + 5u],
        coeffs[o + 6u], coeffs[o + 7u], coeffs[o + 8u],
        coeffs[o + 9u] != 0.0
    );
}

// (a, b, c, eps) -> smallest root > eps, or INF
@compute @workgroup_size(64)
fn probe_smallest_positive_root(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 4u) { return; }
    let b = i * 4u;
    result[i] = smallest_positive_root(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u]);
}

// (a, b, c, d) -> n, r0, r1, r2, r3
@compute @workgroup_size(64)
fn probe_cubic_roots(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 4u) { return; }
    let b = i * 4u;
    let rts = cubic_real_roots(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u]);
    let o = i * 5u;
    result[o] = f32(rts.n);
    result[o + 1u] = rts.r[0];
    result[o + 2u] = rts.r[1];
    result[o + 3u] = rts.r[2];
    result[o + 4u] = rts.r[3];
}

// ---- the primitives underneath the solvers -------------------------------
//
// These five are what quadratic/cubic/quartic_real_roots are built from.
// Until 2026-09-19 they were exercised only through those solvers, where a
// primitive that is subtly wrong still yields a plausible root.

// (c0..c4, n, x) -> poly_eval. Coefficients are DESCENDING: c[0] is the
// leading one, so poly_eval(c, 3, x) evaluates c0 x^2 + c1 x + c2.
@compute @workgroup_size(64)
fn probe_poly_eval(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 7u) { return; }
    let b = i * 7u;
    let c = array<f32, 5>(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u]);
    result[i] = poly_eval(c, u32(coeffs[b + 5u]), coeffs[b + 6u]);
}

// (c0..c4, n, x) -> poly_abs_scale.
@compute @workgroup_size(64)
fn probe_poly_abs_scale(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 7u) { return; }
    let b = i * 7u;
    let c = array<f32, 5>(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u]);
    result[i] = poly_abs_scale(c, u32(coeffs[b + 5u]), coeffs[b + 6u]);
}

// (c0..c4, n) -> root_bound.
@compute @workgroup_size(64)
fn probe_root_bound(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 6u) { return; }
    let b = i * 6u;
    let c = array<f32, 5>(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u]);
    result[i] = root_bound(c, u32(coeffs[b + 5u]));
}

// (r0..r3, n, new) -> (n, r0..r3) after push_unique.
@compute @workgroup_size(64)
fn probe_push_unique(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 6u) { return; }
    let b = i * 6u;
    var acc: Roots;
    acc.n = u32(coeffs[b + 4u]);
    acc.r[0] = coeffs[b];
    acc.r[1] = coeffs[b + 1u];
    acc.r[2] = coeffs[b + 2u];
    acc.r[3] = coeffs[b + 3u];
    let out = push_unique(acc, coeffs[b + 5u]);
    let o = i * 5u;
    result[o] = f32(out.n);
    result[o + 1u] = out.r[0];
    result[o + 2u] = out.r[1];
    result[o + 3u] = out.r[2];
    result[o + 4u] = out.r[3];
}

// (c0..c4, nc, d0..d4, nd, lo, hi) -> bisect_root.
@compute @workgroup_size(64)
fn probe_bisect_root(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&coeffs) / 14u) { return; }
    let b = i * 14u;
    let c = array<f32, 5>(coeffs[b], coeffs[b + 1u], coeffs[b + 2u], coeffs[b + 3u], coeffs[b + 4u]);
    let d = array<f32, 5>(coeffs[b + 6u], coeffs[b + 7u], coeffs[b + 8u], coeffs[b + 9u], coeffs[b + 10u]);
    result[i] = bisect_root(c, u32(coeffs[b + 5u]), d, u32(coeffs[b + 11u]),
                            coeffs[b + 12u], coeffs[b + 13u]);
}
"#;

/// `INF` as the shader spells it — the "ray does not cross" sentinel.
const MISS: f32 = 1e30;

/// Dispatch one appended entry point over `flat`, returning `out_per_case`
/// floats per case.
fn run(
    gpu: &GpuContext,
    entry: &str,
    flat: &[f32],
    n_cases: usize,
    out_per_case: usize,
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let mut source = String::from(SURFACE_DISTANCE_WGSL);
    source.push_str(PROBE_ENTRIES);

    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("surface_distance + geometry probes"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    let mut bytes = Vec::with_capacity(flat.len() * 4);
    for v in flat {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    let coeff_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coeffs"),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

    let out_len = n_cases * out_per_case;
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
            label: Some(entry),
            layout: None,
            module: &module,
            entry_point: Some(entry),
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

// ---------------------------------------------------------------------------
// References, all in f64 and none of them sharing code with the shader
// ---------------------------------------------------------------------------

/// The distance `axis_cylinder` is supposed to return, from the quadratic.
///
/// The shader solves `|d + t w|^2 = r^2` as `a t^2 + 2 k t + cc = 0` with
/// `a = |w|^2`, `k = d.w`, `cc = |d|^2 - r^2`, then picks a root by three
/// rules: on the surface (or flagged coincident) take the far root and only
/// if the ray is heading inward; inside take the far root; outside take the
/// near root and reject it if negative.
///
/// This reimplements the *selection* from that description rather than from
/// the code, in `f64`, so a transcription error in either shows up.
fn axis_cylinder_reference(
    d1: f64,
    d2: f64,
    w1: f64,
    w2: f64,
    radius: f64,
    coincident: bool,
) -> f64 {
    let a = w1 * w1 + w2 * w2;
    if a == 0.0 {
        return MISS as f64;
    }
    let k = d1 * w1 + d2 * w2;
    let cc = d1 * d1 + d2 * d2 - radius * radius;
    let quad = k * k - a * cc;
    if quad < 0.0 {
        return MISS as f64;
    }
    let sq = quad.sqrt();
    let far = (-k + sq) / a;
    let near = (-k - sq) / a;
    // The `abs(cc) < 1e-6` test is the shader's own on-surface tolerance and
    // is applied to the f32 value there; using the f64 cc here would put a
    // handful of cases on the other side of it. Cases are chosen away from
    // that boundary -- see `cylinder_cases`.
    if coincident || cc.abs() < 1.0e-6 {
        if k >= 0.0 {
            return MISS as f64;
        }
        return far;
    }
    if cc < 0.0 {
        return far;
    }
    if near < 0.0 {
        return MISS as f64;
    }
    near
}

/// The torus implicit function along a ray, in `f64`.
///
/// `F(t) = G(t)^2 - 4 a^2 rho^2(t)` with
/// `G(t) = rho^2(t) + a^2 - b^2 + (b/c)^2 z(t)^2`, which is the elliptic
/// torus `(rho - a)^2 + (b/c)^2 z^2 = b^2` squared to clear the root.
///
/// Derived from the geometry, not read off the shader's coefficients — that
/// is the point of it.
fn torus_f(t: f64, p: [f64; 3], u: [f64; 3], a: f64, b: f64, c: f64) -> (f64, f64) {
    let x = p[0] + t * u[0];
    let y = p[1] + t * u[1];
    let z = p[2] + t * u[2];
    let rho2 = x * x + y * y;
    let k = (b * b) / (c * c);
    let g = rho2 + a * a - b * b + k * z * z;
    (g * g - 4.0 * a * a * rho2, g)
}

/// The smallest `t > 0` at which the ray meets the torus, by scanning for a
/// sign change of [`torus_f`] and bisecting it.
///
/// No polynomial is formed anywhere. The `g >= 0` condition rejects the
/// spurious roots that squaring introduced, which is the same branch the
/// shader applies for the same reason.
fn torus_reference(p: [f64; 3], u: [f64; 3], a: f64, b: f64, c: f64) -> f64 {
    let t_max = 50.0_f64;
    let steps = 2_000_000usize;
    let h = t_max / steps as f64;
    let mut t0 = 1.0e-9_f64;
    let (mut f0, _) = torus_f(t0, p, u, a, b, c);
    for i in 1..=steps {
        let t1 = i as f64 * h;
        let (f1, _) = torus_f(t1, p, u, a, b, c);
        if (f0 < 0.0) != (f1 < 0.0) {
            // Bisect to f64 precision.
            let (mut lo, mut hi) = (t0, t1);
            let mut flo = f0;
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                let (fm, _) = torus_f(mid, p, u, a, b, c);
                if fm == 0.0 {
                    lo = mid;
                    hi = mid;
                    break;
                }
                if (fm < 0.0) == (flo < 0.0) {
                    lo = mid;
                    flo = fm;
                } else {
                    hi = mid;
                }
            }
            let root = 0.5 * (lo + hi);
            let (_, g) = torus_f(root, p, u, a, b, c);
            // The shader's own branch filter, with its own tolerance scale.
            if g >= -1.0e-9 * (1.0 + g.abs()) {
                return root;
            }
        }
        t0 = t1;
        f0 = f1;
    }
    MISS as f64
}

/// Smallest root of `a t^2 + b t + c` strictly above `eps`, or `MISS`.
fn smallest_positive_root_reference(a: f64, b: f64, c: f64, eps: f64) -> f64 {
    if a.abs() < 1.0e-7 {
        if b.abs() < 1.0e-7 {
            return MISS as f64;
        }
        let d = -c / b;
        return if d > eps { d } else { MISS as f64 };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return MISS as f64;
    }
    let sq = disc.sqrt();
    let (r1, r2) = ((-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a));
    let (lo, hi) = if r1 <= r2 { (r1, r2) } else { (r2, r1) };
    if lo > eps {
        return lo;
    }
    if hi > eps {
        return hi;
    }
    MISS as f64
}

// ---------------------------------------------------------------------------
// Case sets
// ---------------------------------------------------------------------------

/// `(d1, d2, w1, w2, radius, coincident)` spanning the three branches:
/// outside heading in, outside heading away, inside, on the surface, and a
/// degenerate zero direction.
///
/// Deliberately kept away from `|cc| = 1e-6`, the shader's on-surface
/// tolerance — a case sitting on that boundary would land on different sides
/// of it in `f32` and `f64` and would be testing the tolerance rather than
/// the arithmetic. The `coincident` cases reach that branch explicitly
/// instead.
fn cylinder_cases() -> Vec<[f64; 6]> {
    let mut v = Vec::new();
    for &radius in &[0.5_f64, 1.0, 3.7] {
        for &(d1, d2) in &[
            (-4.0_f64, 0.0), // outside, heading in along x
            (-4.0, 0.3),     // outside, offset
            (4.0, 0.0),      // outside, heading away
            (0.1, 0.05),     // inside
            (0.0, 0.0),      // on the axis
            (-2.5, 2.5),     // outside, diagonal
        ] {
            for &(w1, w2) in &[(1.0_f64, 0.0), (0.6, 0.8), (-1.0, 0.0), (0.0, 1.0)] {
                v.push([d1, d2, w1, w2, radius, 0.0]);
                v.push([d1, d2, w1, w2, radius, 1.0]);
            }
        }
    }
    // A zero direction, which must return MISS through the `a == 0` guard.
    v.push([1.0, 1.0, 0.0, 0.0, 1.0, 0.0]);
    v
}

/// `(p, u, a, b, c, coincident)` rays against elliptic tori.
///
/// `a` is the major radius and `b`, `c` the two minor semi-axes. Directions
/// are unit vectors; the reference scan assumes nothing about that but the
/// distances are then in the same units as the geometry.
fn torus_cases() -> Vec<([f64; 3], [f64; 3], f64, f64, f64, bool)> {
    let mut v = Vec::new();
    let norm = |u: [f64; 3]| {
        let n = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        [u[0] / n, u[1] / n, u[2] / n]
    };
    for &(a, b, c) in &[(3.0_f64, 1.0, 1.0), (5.0, 2.0, 1.0), (2.0, 0.5, 0.8)] {
        for &p in &[
            [-12.0_f64, 0.0, 0.0],
            [-12.0, 0.7, 0.3],
            [0.0, 0.0, -9.0],
            [-8.0, -8.0, 0.0],
            [0.0, 0.0, 0.0],   // the hole: may miss entirely
            [-12.0, 0.0, 2.5], // aimed over the top
        ] {
            for &u in &[
                [1.0_f64, 0.0, 0.0],
                [1.0, 0.05, 0.0],
                [1.0, 0.0, 0.1],
                [0.0, 0.0, 1.0],
                [1.0, 1.0, 0.0],
            ] {
                v.push((p, norm(u), a, b, c, false));
            }
        }
    }
    v
}

/// `(a, b, c, eps)` quadratics covering both roots positive, one positive,
/// none, the `a ~ 0` linear fallback, and `b ~ 0` on top of it.
fn quadratic_cases() -> Vec<[f64; 4]> {
    vec![
        [1.0, -5.0, 6.0, 0.0],  // roots 2, 3
        [1.0, -5.0, 6.0, 2.5],  // eps cuts the first root out
        [1.0, -1.0, -6.0, 0.0], // roots -2, 3
        [1.0, 0.0, 1.0, 0.0],   // no real roots
        [1.0, 4.0, 3.0, 0.0],   // both roots negative
        [2.0, -7.0, 3.0, 0.0],  // roots 0.5, 3
        [0.0, 2.0, -6.0, 0.0],  // linear fallback, root 3
        [0.0, 2.0, 6.0, 0.0],   // linear fallback, negative root
        [0.0, 0.0, 1.0, 0.0],   // fully degenerate -> MISS
        [1.0, -2.0, 1.0, 0.0],  // double root at 1
        [1.0, -1.0e-3, -1.0, 0.0],
    ]
}

/// Cubics for the ordering invariant, spanning three distinct roots, a
/// double root, one real root, and the degenerate leading coefficient that
/// falls through to the quadratic.
fn cubic_cases() -> Vec<[f64; 4]> {
    vec![
        [1.0, -6.0, 11.0, -6.0], // 1, 2, 3
        [1.0, 0.0, -1.0, 0.0],   // -1, 0, 1
        [1.0, -3.0, 3.0, -1.0],  // triple root at 1
        [1.0, -4.0, 5.0, -2.0],  // double at 1, single at 2
        [1.0, 0.0, 0.0, 1.0],    // one real root at -1
        [1.0, 0.0, 0.0, -8.0],   // one real root at 2
        [0.0, 1.0, -3.0, 2.0],   // quadratic fallback: 1, 2
        [0.0, 0.0, 2.0, -4.0],   // linear fallback: 2
        [2.0, 3.0, -11.0, -6.0],
        [1.0, 6.0, 11.0, 6.0], // -3, -2, -1
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The case sets and the torus reference are sound, checked without a device.
///
/// A GPU test that skips proves nothing, and one whose cases all miss the
/// geometry passes by agreeing about nothing. This runs everywhere and
/// guards both: it requires that a decent share of the rays actually hit,
/// and that where they do, the point the reference returns is genuinely on
/// the surface.
#[test]
fn the_torus_reference_and_the_case_set_are_sound() {
    let cases = torus_cases();
    let mut hits = 0usize;
    let mut worst_residual = 0.0_f64;
    for (p, u, a, b, c) in cases.iter().map(|(p, u, a, b, c, _)| (*p, *u, *a, *b, *c)) {
        let t = torus_reference(p, u, a, b, c);
        if t >= MISS as f64 {
            continue;
        }
        hits += 1;
        // On the surface: F(t) = 0, scaled by the size of its own terms.
        let (f, _) = torus_f(t, p, u, a, b, c);
        let scale = (a * a + b * b + 1.0).powi(2);
        worst_residual = worst_residual.max(f.abs() / scale);
    }
    assert!(
        hits >= cases.len() / 3,
        "only {hits} of {} torus cases hit anything -- the case set is not a test",
        cases.len()
    );
    assert!(
        worst_residual < 1e-12,
        "the torus reference returns points that are not on the torus: \
         worst scaled residual {worst_residual:e}"
    );

    // And the cylinder cases must produce a mix of hits and misses.
    let cyl = cylinder_cases();
    let misses = cyl
        .iter()
        .filter(|c| {
            axis_cylinder_reference(c[0], c[1], c[2], c[3], c[4], c[5] != 0.0) >= MISS as f64
        })
        .count();
    assert!(
        misses > 0 && misses < cyl.len(),
        "the cylinder case set is all hits or all misses ({misses} of {})",
        cyl.len()
    );
}

/// `axis_cylinder` on the GPU against the quadratic solved in `f64`.
///
/// # Results
///
/// Worst scaled difference **1.066e-07** over 145 cases, measured 2026-09-19
/// on `llvmpipe (LLVM 20.1.2, 256 bits)` — one `f32` ulp. The tolerance is
/// relative with an absolute floor, because a grazing ray legitimately
/// produces a large distance from a small discriminant and an `f32` square
/// root.
///
/// Verified to catch a real defect by injection: adding `1e-3` to the
/// shader's `cc` term fails this test and no other.
#[test]
fn gpu_axis_cylinder_matches_the_closed_form() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_axis_cylinder_matches_the_closed_form: no GPU adapter");
        return;
    };
    let cases = cylinder_cases();
    let flat: Vec<f32> = cases
        .iter()
        .flat_map(|c| c.iter().map(|v| *v as f32))
        .collect();
    let got = run(&gpu, "probe_axis_cylinder", &flat, cases.len(), 1);

    let mut worst = 0.0_f64;
    let mut at = 0usize;
    for (i, c) in cases.iter().enumerate() {
        let want = axis_cylinder_reference(c[0], c[1], c[2], c[3], c[4], c[5] != 0.0);
        let have = got.get(i).copied().unwrap_or(f32::NAN) as f64;
        let want_miss = want >= MISS as f64;
        let have_miss = have >= MISS as f64;
        assert_eq!(
            want_miss,
            have_miss,
            "case {i} {c:?}: reference {} the cylinder, GPU {} it (GPU gave {have})",
            if want_miss { "misses" } else { "hits" },
            if have_miss { "misses" } else { "hits" },
        );
        if want_miss {
            continue;
        }
        let d = (have - want).abs() / (1.0 + want.abs());
        if d > worst {
            worst = d;
            at = i;
        }
    }
    assert!(
        worst < 1e-5,
        "axis_cylinder GPU vs f64 closed form: {worst:e} at case {at} = {:?}",
        cases[at]
    );
}

/// `smallest_positive_root` on the GPU against the quadratic formula.
#[test]
fn gpu_smallest_positive_root_matches_the_closed_form() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_smallest_positive_root_matches_the_closed_form: no GPU adapter");
        return;
    };
    let cases = quadratic_cases();
    let flat: Vec<f32> = cases
        .iter()
        .flat_map(|c| c.iter().map(|v| *v as f32))
        .collect();
    let got = run(&gpu, "probe_smallest_positive_root", &flat, cases.len(), 1);

    let mut worst = 0.0_f64;
    for (i, c) in cases.iter().enumerate() {
        let want = smallest_positive_root_reference(c[0], c[1], c[2], c[3]);
        let have = got.get(i).copied().unwrap_or(f32::NAN) as f64;
        let want_miss = want >= MISS as f64;
        let have_miss = have >= MISS as f64;
        assert_eq!(
            want_miss, have_miss,
            "case {i} {c:?}: reference miss = {want_miss}, GPU miss = {have_miss} \
             (GPU gave {have})"
        );
        if !want_miss {
            worst = worst.max((have - want).abs() / (1.0 + want.abs()));
        }
    }
    assert!(
        worst < 1e-6,
        "smallest_positive_root GPU vs closed form: {worst:e}"
    );
}

/// `torus_distance` on the GPU against an `f64` scan that builds no
/// polynomial.
///
/// # Why this is the valuable one
///
/// The torus path has a recorded history: `bn:op-9s8.11` was a spurious
/// near-zero root that only appeared once the device limits were fixed and
/// the kernel actually ran, and the fix was a cancellation-noise snap on the
/// quartic's constant term. Everything guarding it since has compared the
/// shader against a mirror carrying the same snap. This compares it against
/// a method with no quartic at all, so a snap that is wrong in both would
/// show here and nowhere else.
///
/// # Results
///
/// **30 of the 90 rays hit**, and over those the worst scaled difference is
/// **3.531e-05**, measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`.
/// The hit count is asserted, because a case set that missed everything
/// would pass this test by comparing two `MISS` sentinels.
///
/// Verified to catch a real defect by injection: scaling the shader's `c2`
/// quartic coefficient by `1.0001` fails this test.
#[test]
fn gpu_torus_distance_matches_an_independent_root_scan() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_torus_distance_matches_an_independent_root_scan: no GPU adapter");
        return;
    };
    let cases = torus_cases();
    let flat: Vec<f32> = cases
        .iter()
        .flat_map(|(p, u, a, b, c, coin)| {
            [
                p[0] as f32,
                p[1] as f32,
                p[2] as f32,
                u[0] as f32,
                u[1] as f32,
                u[2] as f32,
                *a as f32,
                *b as f32,
                *c as f32,
                if *coin { 1.0 } else { 0.0 },
            ]
        })
        .collect();
    let got = run(&gpu, "probe_torus_distance", &flat, cases.len(), 1);

    let mut worst = 0.0_f64;
    let mut at = 0usize;
    let mut compared = 0usize;
    for (i, (p, u, a, b, c, _)) in cases.iter().enumerate() {
        let want = torus_reference(*p, *u, *a, *b, *c);
        let have = got.get(i).copied().unwrap_or(f32::NAN) as f64;
        let want_miss = want >= MISS as f64;
        let have_miss = have >= MISS as f64;
        assert_eq!(
            want_miss,
            have_miss,
            "case {i}: p = {p:?} u = {u:?} (a, b, c) = ({a}, {b}, {c}): \
             reference {} the torus, GPU {} it (GPU gave {have})",
            if want_miss { "misses" } else { "hits" },
            if have_miss { "misses" } else { "hits" },
        );
        if want_miss {
            continue;
        }
        compared += 1;
        let d = (have - want).abs() / (1.0 + want.abs());
        if d > worst {
            worst = d;
            at = i;
        }
    }
    assert!(compared > 0, "no torus case produced a comparable distance");
    assert!(
        worst < 1e-4,
        "torus_distance GPU vs f64 scan: {worst:e} at case {at} \
         ({compared} cases compared)"
    );
}

/// `cubic_real_roots` returns its roots **ascending**, on the device.
///
/// # Why this is a test and not a comment
///
/// `quartic_real_roots` passes the cubic's roots straight into
/// `collect_real_roots` as breakpoints, and that function is correct only if
/// the breakpoint array is increasing. The shader carries the invariant as a
/// comment (`// crit3 is already ascending from collect_real_roots /
/// quadratic fallback`) while the Rust mirror enforces it with an explicit
/// `sort_small_f32`. That asymmetry is `bn:op-9s8.13`: behaviourally
/// equivalent today, load-bearing, and enforced on one side by a claim.
///
/// This checks the claim on the hardware, across every exit path the cubic
/// has — `collect_real_roots`' own `sort_roots`, the quadratic fallback's
/// `min`/`max`, and the linear fallback's single root. 20 roots across 10
/// cases.
///
/// # The invariant was load-bearing, and that was measured
///
/// Reversing `cubic_real_roots`' output in the shader fails **this test and
/// `gpu_torus_distance_matches_an_independent_root_scan`** — the ordering
/// defect propagates into the torus distance through
/// `quartic_real_roots`' breakpoint array, exactly as `op-9s8.13` predicted
/// from reading the source.
///
/// `quartic_real_roots` now sorts explicitly, matching the Rust mirror's
/// `sort_small_f32`. Repeating the same injection afterwards fails only this
/// test: the defect is detected and no longer corrupts anything downstream.
#[test]
fn gpu_cubic_roots_come_back_ascending() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_cubic_roots_come_back_ascending: no GPU adapter");
        return;
    };
    let cases = cubic_cases();
    let flat: Vec<f32> = cases
        .iter()
        .flat_map(|c| c.iter().map(|v| *v as f32))
        .collect();
    let got = run(&gpu, "probe_cubic_roots", &flat, cases.len(), 5);

    let mut total_roots = 0usize;
    for (i, c) in cases.iter().enumerate() {
        let o = i * 5;
        let n = got.get(o).copied().unwrap_or(-1.0);
        assert!(
            (0.0..=3.0).contains(&n),
            "case {i} {c:?}: root count {n} is outside 0..=3, which would \
             overflow quartic_real_roots' 3-element crits array (bn:op-9s8.12)"
        );
        let n = n as usize;
        total_roots += n;
        let roots: Vec<f32> = (0..n).filter_map(|k| got.get(o + 1 + k).copied()).collect();
        for w in roots.windows(2) {
            assert!(
                w[0] <= w[1],
                "case {i} {c:?}: cubic_real_roots returned {roots:?}, which is \
                 not ascending -- quartic_real_roots feeds these straight into \
                 collect_real_roots as breakpoints and needs them increasing \
                 (bn:op-9s8.13)"
            );
        }
        // And each reported root really is one.
        for &r in &roots {
            let x = r as f64;
            let f = c[0] * x * x * x + c[1] * x * x + c[2] * x + c[3];
            let scale = 1.0
                + (c[0] * x * x * x).abs()
                + (c[1] * x * x).abs()
                + (c[2] * x).abs()
                + c[3].abs();
            assert!(
                f.abs() <= 1e-4 * scale,
                "case {i} {c:?}: reported root {r} has residual {f:e} (scale {scale:e})"
            );
        }
    }
    assert!(
        total_roots >= 18,
        "the cubic case set yielded only {total_roots} roots -- too few to \
         exercise the ordering"
    );
}

// ---------------------------------------------------------------------------
// The primitives underneath the solvers
// ---------------------------------------------------------------------------

/// A spread of test polynomials, as `(descending coefficients, degree + 1)`.
///
/// Chosen to include a quartic, a cubic with a large leading coefficient, one
/// with a tiny one, and a polynomial whose coefficients alternate in sign —
/// the case where Horner's cancellation is worst and where a scheme that
/// formed powers separately would part company.
fn test_polys() -> Vec<(Vec<f32>, usize)> {
    vec![
        (vec![1.0, -6.0, 11.0, -6.0, 0.0], 4),
        (vec![2.0, 0.0, -3.0, 1.0, 0.0], 4),
        (vec![1.0, -10.0, 35.0, -50.0, 24.0], 5),
        (vec![1e-3, 4.0, -7.0, 2.5, 0.0], 4),
        (vec![1e3, -1e3, 1e3, -1e3, 1e3], 5),
        (vec![1.0, 0.0, -2.0, 0.0, 0.0], 3),
        (vec![3.0, -1.0, 0.0, 0.0, 0.0], 2),
    ]
}

/// **`poly_eval` is Horner in DESCENDING order**, checked against
/// `petir::poly::eval` — which takes **ascending** coefficients and is
/// verified bit-identical against GSL's `gsl_poly_eval`.
///
/// # Why the order is the thing being tested
///
/// The two conventions differ by a reversal, and reversing a coefficient
/// vector is the single most likely defect in a routine like this: it
/// compiles, it returns a plausible number, and for a palindromic polynomial
/// it even returns the right one. So this test does two things — it agrees
/// with the reversed reference, and it **asserts that the unreversed reading
/// disagrees**, which is what makes the first assertion mean something.
///
/// Measured 2026-09-19 on llvmpipe over seven polynomials at eleven
/// abscissae: worst relative difference 2.4e-07, about two `f32` ulp, which
/// is the `f32`-versus-`f64` cost and not a discrepancy in the scheme.
#[test]
fn gpu_poly_eval_is_horner_in_descending_order() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_poly_eval_is_horner_in_descending_order: no GPU adapter");
        return;
    };
    let xs = [
        -3.0_f32, -1.7, -1.0, -0.3, 0.0, 0.2, 0.5, 1.0, 1.9, 2.5, 4.0,
    ];
    let mut flat = Vec::new();
    let mut cases = Vec::new();
    for (c, n) in test_polys() {
        for &x in &xs {
            flat.extend_from_slice(&c);
            flat.push(n as f32);
            flat.push(x);
            cases.push((c.clone(), n, x));
        }
    }
    let out = run(&gpu, "probe_poly_eval", &flat, cases.len(), 1);

    let (mut worst, mut at) = (0.0_f64, 0usize);
    let mut reversal_would_differ = 0;
    for (k, (c, n, x)) in cases.iter().enumerate() {
        // petir wants ASCENDING, the shader holds DESCENDING.
        let ascending: Vec<f64> = c[..*n].iter().rev().map(|&v| v as f64).collect();
        let want = petir::poly::eval(&ascending, *x as f64);
        let got = out[k] as f64;
        let scale = want.abs().max(1.0);
        let e = (got - want).abs() / scale;
        if e > worst {
            worst = e;
            at = k;
        }
        // The same coefficients read the wrong way round.
        let unreversed: Vec<f64> = c[..*n].iter().map(|&v| v as f64).collect();
        if (petir::poly::eval(&unreversed, *x as f64) - want).abs() > 1e-6 * scale {
            reversal_would_differ += 1;
        }
    }
    assert!(
        worst < 1e-5,
        "poly_eval against petir's Horner: {worst:e} at case {at} \
         {:?}. Documented at 2.4e-07",
        cases[at]
    );
    assert!(
        reversal_would_differ > cases.len() / 2,
        "the reversal must be OBSERVABLE for the agreement above to mean \
         anything; only {reversal_would_differ} of {} cases distinguish the \
         two orders",
        cases.len()
    );
}

/// **`poly_abs_scale` is the sum of absolute Horner terms**, against its
/// closed form `sum_i |c_i| |x|^{n-1-i}`.
///
/// It exists as the relative-residual scale for the multiple-root test inside
/// `collect_real_roots`: comparing `|p(x)|` against it is what distinguishes
/// "small because `x` is a root" from "small because every term is small".
/// A wrong scale makes that test either blind or hysterical, and neither
/// shows up as a wrong root — it shows up as a missing or duplicated one.
#[test]
fn gpu_poly_abs_scale_is_the_sum_of_absolute_horner_terms() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_poly_abs_scale_is_the_sum_of_absolute_horner_terms: no GPU adapter");
        return;
    };
    let xs = [-3.0_f32, -1.0, 0.0, 0.5, 1.0, 2.5];
    let mut flat = Vec::new();
    let mut cases = Vec::new();
    for (c, n) in test_polys() {
        for &x in &xs {
            flat.extend_from_slice(&c);
            flat.push(n as f32);
            flat.push(x);
            cases.push((c.clone(), n, x));
        }
    }
    let out = run(&gpu, "probe_poly_abs_scale", &flat, cases.len(), 1);
    let mut worst = 0.0_f64;
    for (k, (c, n, x)) in cases.iter().enumerate() {
        // The closed form, in f64 and without Horner.
        let want: f64 = (0..*n)
            .map(|i| (c[i] as f64).abs() * (*x as f64).abs().powi((*n - 1 - i) as i32))
            .sum();
        let got = out[k] as f64;
        worst = worst.max((got - want).abs() / want.max(1.0));
        // It is a SCALE: never negative, and never below |p(x)|.
        assert!(got >= 0.0, "poly_abs_scale returned {got:e} at case {k}");
        let ascending: Vec<f64> = c[..*n].iter().rev().map(|&v| v as f64).collect();
        assert!(
            got + 1e-4 >= petir::poly::eval(&ascending, *x as f64).abs(),
            "the scale {got:e} is below |p(x)| at case {k}, which would make \
             the residual test inside collect_real_roots meaningless"
        );
    }
    assert!(
        worst < 1e-5,
        "poly_abs_scale against its closed form: {worst:e}"
    );
}

/// **`root_bound` is Cauchy's bound, and it actually bounds the roots** — two
/// separate claims, both checked.
///
/// The closed form is `1 + max_i |c_i / c_0|`, which is easy to verify and
/// says nothing about whether the routine is *useful*. The property that
/// matters is that every real root lies inside it, so the second half builds
/// polynomials from known roots and checks exactly that. A bound that is
/// merely arithmetically right but too small silently loses roots in
/// `collect_real_roots`, which scans `[-m, m]`.
#[test]
fn gpu_root_bound_is_cauchys_bound_and_actually_bounds_the_roots() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_root_bound_is_cauchys_bound_and_actually_bounds_the_roots: no GPU");
        return;
    };
    // Polynomials built from known roots, plus the generic spread.
    let from_roots: Vec<(Vec<f32>, usize, Vec<f64>)> = vec![
        (
            vec![1.0, -6.0, 11.0, -6.0, 0.0],
            4,
            vec![0.0, 1.0, 2.0, 3.0],
        ),
        (
            vec![1.0, -10.0, 35.0, -50.0, 24.0],
            5,
            vec![1.0, 2.0, 3.0, 4.0],
        ),
        (vec![1.0, 0.0, -25.0, 0.0, 0.0], 3, vec![-5.0, 5.0]),
        (vec![2.0, 2.0, -24.0, 0.0, 0.0], 3, vec![-4.0, 3.0]),
    ];
    let mut flat = Vec::new();
    for (c, n, _) in &from_roots {
        flat.extend_from_slice(c);
        flat.push(*n as f32);
    }
    for (c, n) in test_polys() {
        flat.extend_from_slice(&c);
        flat.push(n as f32);
    }
    let total = from_roots.len() + test_polys().len();
    let out = run(&gpu, "probe_root_bound", &flat, total, 1);

    // 1. Cauchy's bound, in closed form.
    let all: Vec<(Vec<f32>, usize)> = from_roots
        .iter()
        .map(|(c, n, _)| (c.clone(), *n))
        .chain(test_polys())
        .collect();
    for (k, (c, n)) in all.iter().enumerate() {
        let lead = c[0] as f64;
        let want = 1.0
            + (1..*n)
                .map(|i| (c[i] as f64 / lead).abs())
                .fold(0.0_f64, f64::max);
        let got = out[k] as f64;
        assert!(
            (got - want).abs() / want < 1e-5,
            "root_bound at case {k} is {got:e} against Cauchy's {want:e}"
        );
    }

    // 2. And it really contains every real root.
    for (k, (_, _, roots)) in from_roots.iter().enumerate() {
        let m = out[k] as f64;
        for &r in roots {
            assert!(
                r.abs() <= m,
                "root {r} lies outside the bound {m} for case {k}. \
                 collect_real_roots scans [-m, m], so a root outside it is a \
                 root that is never found"
            );
        }
    }
}

/// **`push_unique` keeps a set**: it never admits a near-duplicate, never
/// exceeds four, and never reorders what is already there.
///
/// It is what stops a double root arriving twice from the two sub-intervals
/// that straddle it. Getting the tolerance wrong in either direction is a
/// real defect — too tight and a double root is reported twice, too loose and
/// two genuinely close roots collapse into one — and neither is visible from
/// the solver's output alone, which is why it is checked here.
#[test]
fn gpu_push_unique_keeps_a_set() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_push_unique_keeps_a_set: no GPU adapter");
        return;
    };
    // (existing r0..r3, n, candidate, expected n after)
    let cases: Vec<([f32; 4], u32, f32, u32)> = vec![
        ([0.0; 4], 0, 1.5, 1),                      // into an empty set
        ([1.5, 0.0, 0.0, 0.0], 1, 2.5, 2),          // a genuinely new root
        ([1.5, 0.0, 0.0, 0.0], 1, 1.5, 1),          // exactly equal
        ([1.5, 0.0, 0.0, 0.0], 1, 1.500_001, 1),    // inside the tolerance
        ([1.5, 0.0, 0.0, 0.0], 1, 1.501, 2),        // outside it
        ([1.0, 2.0, 3.0, 4.0], 4, 5.0, 4),          // already full
        ([1.0, 2.0, 3.0, 0.0], 3, 2.0, 3),          // duplicate, not at the end
        ([-1e4, 0.0, 0.0, 0.0], 1, -1e4 - 0.05, 1), // relative tolerance at scale
    ];
    let mut flat = Vec::new();
    for (r, n, cand, _) in &cases {
        flat.extend_from_slice(r);
        flat.push(*n as f32);
        flat.push(*cand);
    }
    let out = run(&gpu, "probe_push_unique", &flat, cases.len(), 5);

    for (k, (r, n, cand, want_n)) in cases.iter().enumerate() {
        let got_n = out[5 * k] as u32;
        assert_eq!(
            got_n,
            *want_n,
            "case {k}: pushing {cand} into {:?} (n = {n}) gave n = {got_n}, \
             expected {want_n}",
            &r[..*n as usize]
        );
        // What was already there is untouched and in order.
        for j in 0..*n as usize {
            assert_eq!(
                out[5 * k + 1 + j],
                r[j],
                "case {k}: existing entry {j} was modified"
            );
        }
        // No two entries within the tolerance of each other.
        let kept = &out[5 * k + 1..5 * k + 1 + got_n as usize];
        for a in 0..kept.len() {
            for b in a + 1..kept.len() {
                assert!(
                    (kept[a] - kept[b]).abs() > 1e-5 * (1.0 + kept[b].abs()),
                    "case {k}: {} and {} are duplicates within the tolerance",
                    kept[a],
                    kept[b]
                );
            }
        }
        assert!(got_n <= 4, "case {k}: the set overflowed to {got_n}");
    }
}

/// **`bisect_root` converges to the root `petir` finds**, on the same bracket.
///
/// `petir::roots::BracketingSolver` is verified against GSL every iterate, so
/// it is an independent answer to the same question rather than the same
/// algorithm written twice — the shader's routine is bisection followed by
/// eight Newton steps, which is a different method from Brent.
///
/// Measured 2026-09-19 on llvmpipe: every root within 3.5e-06 of `petir`'s,
/// and every returned point inside the bracket it was given.
#[test]
fn gpu_bisect_root_converges_to_the_root_petir_finds() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_bisect_root_converges_to_the_root_petir_finds: no GPU adapter");
        return;
    };
    // (descending coefficients, degree + 1, bracket) with one simple root in
    // each bracket.
    let cases: Vec<(Vec<f32>, usize, f32, f32)> = vec![
        (vec![1.0, -6.0, 11.0, -6.0, 0.0], 4, 0.5, 1.5),
        (vec![1.0, -6.0, 11.0, -6.0, 0.0], 4, 1.5, 2.5),
        (vec![1.0, -6.0, 11.0, -6.0, 0.0], 4, 2.5, 3.5),
        (vec![1.0, 0.0, -2.0, 0.0, 0.0], 3, 1.0, 2.0),
        (vec![1.0, 0.0, -2.0, 0.0, 0.0], 3, -2.0, -1.0),
        (vec![2.0, 0.0, -3.0, 1.0, 0.0], 4, -2.0, -0.5),
        (vec![1e-3, 4.0, -7.0, 2.5, 0.0], 4, 0.4, 0.9),
    ];
    let mut flat = Vec::new();
    for (c, n, lo, hi) in &cases {
        // The derivative, also descending: d_i = (n-1-i) c_i for i < n-1.
        let mut d = vec![0.0_f32; 5];
        for i in 0..*n - 1 {
            d[i] = (*n - 1 - i) as f32 * c[i];
        }
        flat.extend_from_slice(c);
        flat.push(*n as f32);
        flat.extend_from_slice(&d);
        flat.push((*n - 1) as f32);
        flat.push(*lo);
        flat.push(*hi);
    }
    let out = run(&gpu, "probe_bisect_root", &flat, cases.len(), 1);

    let mut worst = 0.0_f64;
    for (k, (c, n, lo, hi)) in cases.iter().enumerate() {
        let ascending: Vec<f64> = c[..*n].iter().rev().map(|&v| v as f64).collect();
        let f = |x: f64| petir::poly::eval(&ascending, x);
        let mut solver = petir::roots::BracketingSolver::new(
            petir::roots::BracketingMethod::Brent,
            f,
            *lo as f64,
            *hi as f64,
        )
        .expect("the bracket must straddle a root");
        let want = solver.solve(1e-12, 1e-12, 200).expect("Brent converges");

        let got = out[k] as f64;
        assert!(
            got >= *lo as f64 - 1e-4 && got <= *hi as f64 + 1e-4,
            "case {k}: bisect_root returned {got} outside its bracket \
             [{lo}, {hi}]"
        );
        let e = (got - want).abs() / (1.0 + want.abs());
        worst = worst.max(e);
        // And it really is a root, by residual against the polynomial itself.
        assert!(
            f(got).abs() < 1e-4 * (1.0 + f(got + 1e-3).abs()),
            "case {k}: p({got}) = {:e}, which is not a root",
            f(got)
        );
    }
    assert!(
        worst < 1e-4,
        "bisect_root against petir's Brent: {worst:e}. Documented at 3.5e-06"
    );
}
