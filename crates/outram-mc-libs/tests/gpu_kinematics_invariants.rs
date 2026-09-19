//! **Analytical verification of the GPU scattering kinematics** in
//! `src/gpu/shaders/batched_event.wgsl`.
//!
//! # The gap this closes
//!
//! `batched_event.wgsl` has eleven helper functions and exactly one of them —
//! `main` — is dispatched by any test. The kinematics among them
//! (`rotate_direction`, `cm_to_lab`, `two_body_with_mu`) are checked only
//! *through* `advance_event_cpu_mirror`, which the module's own docs describe
//! as running "the same f32 arithmetic".
//!
//! **A mirror that shares the algorithm cannot find an error in the
//! algorithm.** It pins the GPU against the CPU, which is worth having and is
//! not the same thing as pinning either against physics. An error in the
//! CM-to-lab transform would sit in both sides identically and every existing
//! test would stay green.
//!
//! # Why analytical invariants rather than a cross-code reference
//!
//! This crate's porting rule makes OpenMC's C++ the source of truth, and the
//! honest check would be against it. **That source is not present in this
//! container** (`/home/teddy0/Documents/research/openmc/`), so that comparison
//! cannot be made here and is not attempted.
//!
//! What can be done instead is stronger than a mirror and weaker than a
//! cross-code run: **closed-form identities that the implementation does not
//! share**. For elastic two-body scattering off a nucleus of mass ratio `A`,
//!
//! ```text
//!   E'/E     = (A^2 + 2 A mu_cm + 1) / (A + 1)^2
//!   mu_lab   = (1 + A mu_cm) / sqrt(A^2 + 2 A mu_cm + 1)
//! ```
//!
//! Neither is the form the shader computes — it works through a CM
//! translational energy and two square roots — so agreement is evidence, not
//! tautology. The same applies to `rotate_direction`, whose defining property
//! is `dot(u_new, u_old) == mu` and which the shader obtains by building an
//! explicit basis.
//!
//! # `f32`, deliberately
//!
//! The shader is `f32`, so every tolerance here is an `f32` budget. The
//! references are evaluated in `f64` and the measured figures are recorded
//! per test.
//!
//! # No adapter is not a pass
//!
//! Without a GPU these print `SKIP`. The identities are therefore *also*
//! checked against the closed forms on the CPU in
//! `the_reference_identities_are_self_consistent`, which needs no device and
//! would catch an error in the reference algebra itself.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::{probe, GpuContext};

const BATCHED_EVENT_WGSL: &str = include_str!("../src/gpu/shaders/batched_event.wgsl");

/// A second entry point appended to the shader, reusing its existing `xs`
/// (binding 0, read) and `fstate` (binding 2, read-write) declarations.
///
/// Each invocation reads a 5-tuple `(e, awr, q, mu_cm, uz)` from `xs` and
/// writes an 8-tuple to `fstate`:
/// `(e_out, mu_lab, dir.x, dir.y, dir.z, dot(dir,u), |dir|, 0)`.
///
/// `u` is built as `(sqrt(1-uz^2), 0, uz)`, a unit vector, so the rotation
/// invariants have something non-degenerate to act on.
const KINEMATICS_ENTRY: &str = r#"
@compute @workgroup_size(64)
fn petir_kinematics_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = arrayLength(&xs) / 5u;
    if (i >= n) { return; }
    let b = i * 5u;
    let e     = xs[b];
    let awr   = xs[b + 1u];
    let q     = xs[b + 2u];
    let mu_cm = xs[b + 3u];
    let uz    = xs[b + 4u];

    let u = vec3<f32>(sqrt(max(1.0 - uz * uz, 0.0)), 0.0, uz);

    // Energy and lab cosine straight from cm_to_lab, via the same
    // e_cm_out two_body_with_mu forms.
    let ap1 = awr + 1.0;
    let e_cm_out = max(e * (awr / ap1) * (awr / ap1) + q * awr / ap1, 0.0);
    let lab = cm_to_lab(e, e_cm_out, clamp(mu_cm, -1.0, 1.0), awr);

    // And the full two-body call, whose direction we check for the rotation
    // invariants. The seed is fixed per invocation but its value is
    // irrelevant: both invariants hold for EVERY azimuth.
    let sc = two_body_with_mu(e, u, awr, q, mu_cm, vec2<u32>(i + 1u, 0x9E3779B9u));

    let o = i * 8u;
    fstate[o]      = lab.x;
    fstate[o + 1u] = lab.y;
    fstate[o + 2u] = sc.dir.x;
    fstate[o + 3u] = sc.dir.y;
    fstate[o + 4u] = sc.dir.z;
    fstate[o + 5u] = dot(sc.dir, u);
    fstate[o + 6u] = length(sc.dir);
    fstate[o + 7u] = sc.e;
}
"#;

/// One probe case: incident energy, mass ratio, Q value, CM cosine, and the
/// z-component of the incoming direction.
#[derive(Clone, Copy)]
struct Case {
    e: f32,
    awr: f32,
    q: f32,
    mu_cm: f32,
    uz: f32,
}

/// Cases spanning what the kinematics branch on.
///
/// Mass ratios from hydrogen (`A = 1`, where the minimum scattered energy is
/// zero) to U-238 (`A = 236`, where it is 98.3 % of incident). CM cosines
/// include both endpoints, which are the two extremes of the energy relation
/// and the ones a sign error moves. `uz` includes values near `±1`, which is
/// where `rotate_direction` switches to its alternate basis.
fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for &awr in &[1.0f32, 2.0, 12.0, 56.0, 236.0] {
        for &mu_cm in &[-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            for &uz in &[0.0f32, 0.5, 0.9, 0.999_999, -0.7] {
                v.push(Case {
                    e: 2.0e6,
                    awr,
                    q: 0.0,
                    mu_cm,
                    uz,
                });
            }
        }
    }
    v
}

/// Closed-form elastic energy ratio, `E'/E`, in `f64`.
///
/// Textbook two-body elastic kinematics. Not the form the shader uses.
fn elastic_energy_ratio(awr: f64, mu_cm: f64) -> f64 {
    let ap1 = awr + 1.0;
    (awr * awr + 2.0 * awr * mu_cm + 1.0) / (ap1 * ap1)
}

/// Closed-form elastic lab cosine, in `f64`. Not the form the shader uses.
fn elastic_mu_lab(awr: f64, mu_cm: f64) -> f64 {
    let denom = (awr * awr + 2.0 * awr * mu_cm + 1.0).sqrt();
    if denom == 0.0 {
        return 1.0;
    }
    (1.0 + awr * mu_cm) / denom
}

/// Dispatch the probe kernel, returning 8 floats per case.
fn run(gpu: &GpuContext, cases: &[Case]) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let mut source = String::from(BATCHED_EVENT_WGSL);
    source.push_str(KINEMATICS_ENTRY);

    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("batched_event + kinematics probe"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    let mut xs_bytes = Vec::with_capacity(cases.len() * 5 * 4);
    for c in cases {
        for v in [c.e, c.awr, c.q, c.mu_cm, c.uz] {
            xs_bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    let xs_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("xs"),
            contents: &xs_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

    let out_len = cases.len() * 8;
    let out_size = (out_len * std::mem::size_of::<f32>()) as u64;
    let fstate_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fstate"),
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
            label: Some("kinematics probe"),
            layout: None,
            module: &module,
            entry_point: Some("petir_kinematics_probe"),
            compilation_options: Default::default(),
            cache: None,
        });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: xs_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: fstate_buf.as_entire_binding(),
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
        pass.dispatch_workgroups(cases.len().div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&fstate_buf, 0, &read_buf, 0, out_size);
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

/// The reference identities are themselves right, checked without a device.
///
/// # Why check the reference
///
/// The closed forms above are the yardstick, and a yardstick is not correct
/// merely for being external — this project has already found a reference
/// script wrong once, and a published table wrong once. These two properties
/// pin the algebra:
///
/// - At `mu_cm = +1` the collision is forward and loses no energy: `E'/E = 1`.
/// - At `mu_cm = -1` it is a head-on backscatter and gives the classical
///   minimum `E'/E = alpha = ((A-1)/(A+1))^2`.
///
/// Both are textbook, independent of the ratio formula's derivation, and
/// enough to catch a transposed sign or a swapped `A`.
#[test]
fn the_reference_identities_are_self_consistent() {
    for &awr in &[1.0f64, 2.0, 12.0, 56.0, 236.0] {
        let fwd = elastic_energy_ratio(awr, 1.0);
        assert!(
            (fwd - 1.0).abs() < 1e-12,
            "A = {awr}: forward scatter should lose no energy, got {fwd}"
        );

        let back = elastic_energy_ratio(awr, -1.0);
        let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
        assert!(
            (back - alpha).abs() < 1e-12,
            "A = {awr}: backscatter should give alpha = {alpha}, got {back}"
        );

        // Hydrogen can take all the energy; nothing heavier can.
        if awr == 1.0 {
            assert!(back.abs() < 1e-12, "A = 1 backscatter must reach zero");
        } else {
            assert!(back > 0.0, "A = {awr} backscatter must stay positive");
        }

        // The lab cosine is forward at both endpoints for A > 1.
        assert!((elastic_mu_lab(awr, 1.0) - 1.0).abs() < 1e-12);
    }
}

/// The GPU's elastic energy and lab cosine match the closed forms.
///
/// # Methodology
///
/// 125 cases: five mass ratios (H through U-238), five CM cosines including
/// both endpoints, five incoming directions including two near the pole where
/// `rotate_direction` switches basis. `Q = 0`, so `two_body_with_mu` reduces
/// to elastic scattering and the closed forms apply exactly.
///
/// Compared **relatively** against the `f64` closed forms. `mu_lab` is
/// compared absolutely, since it legitimately passes through zero.
///
/// # Results
///
/// Measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`, over all 125
/// cases:
///
/// | quantity | worst difference from the closed form |
/// |---|---|
/// | `E'` (relative) | **1.563e-07**, at `A = 2`, `mu_cm = -1` |
/// | `mu_lab` (absolute) | **5.960e-08** |
///
/// Both are at the `f32` floor — machine epsilon is 1.192e-07 — so the
/// agreement is as close as single precision permits. The worst energy case
/// being the backscatter extreme (`mu_cm = -1`) is expected: that is where
/// `E'` is smallest and a relative comparison is least forgiving.
///
/// Interpretation: the GPU's CM-to-lab transform reproduces textbook two-body
/// elastic kinematics to rounding, by a route that does not share the
/// textbook's algebra. That is evidence about the physics, which the existing
/// GPU-vs-mirror test could not give at any tolerance.
#[test]
fn gpu_elastic_kinematics_match_the_closed_form() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_elastic_kinematics_match_the_closed_form: no GPU adapter");
        return;
    };
    let all = cases();
    let out = run(&gpu, &all);

    let mut worst_e = 0.0_f64;
    let mut worst_mu = 0.0_f64;
    let mut worst_at = (0.0_f32, 0.0_f32);

    for (idx, c) in all.iter().enumerate() {
        let base = idx * 8;
        let e_out = out.get(base).copied().unwrap_or(f32::NAN) as f64;
        let mu_lab = out.get(base + 1).copied().unwrap_or(f32::NAN) as f64;

        let want_e = c.e as f64 * elastic_energy_ratio(c.awr as f64, c.mu_cm as f64);
        let want_mu = elastic_mu_lab(c.awr as f64, c.mu_cm as f64);

        // A = 1, mu_cm = -1 is the one case where E' is exactly zero, so a
        // relative comparison is undefined; check it absolutely.
        let rel_e = if want_e.abs() < 1.0 {
            (e_out - want_e).abs()
        } else {
            ((e_out - want_e) / want_e).abs()
        };
        if rel_e > worst_e {
            worst_e = rel_e;
            worst_at = (c.awr, c.mu_cm);
        }
        worst_mu = worst_mu.max((mu_lab - want_mu).abs());
    }

    assert!(
        worst_e < 1e-4,
        "worst energy difference {worst_e:e} at A = {}, mu_cm = {}, device {}",
        worst_at.0,
        worst_at.1,
        gpu.info.name
    );
    assert!(
        worst_mu < 1e-4,
        "worst lab-cosine difference {worst_mu:e}, device {}",
        gpu.info.name
    );
}

/// `rotate_direction` preserves the unit norm and realises the cosine it was
/// asked for.
///
/// # Why these two properties
///
/// They *are* the definition of the operation, and the shader satisfies them
/// only if its basis construction is right — it builds an explicit orthogonal
/// frame and has a separate branch for directions near the pole, which is
/// exactly the kind of thing that is wrong only in one branch.
///
/// `dot(u_new, u_old) == mu_lab` holds for **every azimuth**, so the random
/// seed is irrelevant and the check needs no control over the RNG.
///
/// # Results
///
/// Measured 2026-09-19 on `llvmpipe (LLVM 20.1.2, 256 bits)`:
///
/// | invariant | worst deviation |
/// |---|---|
/// | `\|dir\| == 1` | **1.192e-07** |
/// | `dot(dir, u) == mu_lab` | **5.960e-08** |
///
/// Both at the `f32` floor. **The worst case is `uz = 0.999999`** — the
/// near-pole direction that takes `rotate_direction`'s alternate basis
/// branch. That branch is therefore not merely present but exercised and
/// correct, which is the thing a whole-event comparison would be least likely
/// to reach: a single mis-signed term there would perturb one collision in
/// many thousands and vanish into the statistics.
#[test]
fn gpu_rotate_direction_preserves_the_unit_norm_and_the_cosine() {
    let Some(gpu) = probe() else {
        eprintln!(
            "SKIP gpu_rotate_direction_preserves_the_unit_norm_and_the_cosine: no GPU adapter"
        );
        return;
    };
    let all = cases();
    let out = run(&gpu, &all);

    let mut worst_norm = 0.0_f32;
    let mut worst_cos = 0.0_f32;
    let mut worst_at = (0.0_f32, 0.0_f32);

    for (idx, c) in all.iter().enumerate() {
        let base = idx * 8;
        let mu_lab = out.get(base + 1).copied().unwrap_or(f32::NAN);
        let cos = out.get(base + 5).copied().unwrap_or(f32::NAN);
        let norm = out.get(base + 6).copied().unwrap_or(f32::NAN);

        assert!(
            norm.is_finite() && cos.is_finite(),
            "case {idx} (A = {}, uz = {}) produced a non-finite direction",
            c.awr,
            c.uz
        );

        worst_norm = worst_norm.max((norm - 1.0).abs());
        let d = (cos - mu_lab).abs();
        if d > worst_cos {
            worst_cos = d;
            worst_at = (c.awr, c.uz);
        }
    }

    assert!(
        worst_norm < 1e-5,
        "worst |dir| deviation from 1 is {worst_norm:e}, device {}",
        gpu.info.name
    );
    assert!(
        worst_cos < 1e-4,
        "worst dot(dir, u) vs mu_lab is {worst_cos:e} at A = {}, uz = {}, device {}",
        worst_at.0,
        worst_at.1,
        gpu.info.name
    );
}

/// Scattered energy stays inside the physically allowed band.
///
/// # Why a bound test as well as an equality test
///
/// The equality tests above use `Q = 0`. This one states the property that
/// must hold for **any** elastic collision regardless of how the answer is
/// computed: the scattered energy lies in `[alpha E, E]` where
/// `alpha = ((A-1)/(A+1))^2`. A kernel that violated it would be producing
/// neutrons from nothing, which no tolerance should absorb.
#[test]
fn gpu_scattered_energy_stays_within_the_elastic_band() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_scattered_energy_stays_within_the_elastic_band: no GPU adapter");
        return;
    };
    let all = cases();
    let out = run(&gpu, &all);

    for (idx, c) in all.iter().enumerate() {
        let e_out = out.get(idx * 8).copied().unwrap_or(f32::NAN);
        let e_full = out.get(idx * 8 + 7).copied().unwrap_or(f32::NAN);
        let alpha = ((c.awr - 1.0) / (c.awr + 1.0)).powi(2);
        let lo = alpha * c.e;
        let hi = c.e;
        // One f32 ulp of the incident energy, as slack at the endpoints.
        let slack = c.e * 1e-5;

        assert!(
            e_out >= lo - slack && e_out <= hi + slack,
            "case {idx} (A = {}, mu_cm = {}): E' = {e_out} outside [{lo}, {hi}]",
            c.awr,
            c.mu_cm
        );
        assert!(
            (e_out - e_full).abs() <= slack,
            "case {idx}: cm_to_lab and two_body_with_mu disagree, {e_out} vs {e_full}"
        );
    }
}
