//! **Direct verification of the sampling helpers** inside
//! `src/gpu/shaders/batched_event.wgsl` — the random-number generator, the
//! grid search, the interpolation, and the anisotropic-scattering inverse.
//!
//! # Why this exists
//!
//! `batched_event.wgsl` is checked today through whole transported histories
//! and through `gpu_kinematics_invariants.rs`, which pins the elastic
//! kinematics against closed forms. Neither reaches the helpers *underneath*
//! that: `rng_next`, `locate`, `interp_channel`, `langevin_inverse` and
//! `exponential_mu` are exercised only in composition, where a helper that is
//! subtly wrong still produces a plausible history and the statistics absorb
//! it. A biased sampler is exactly the kind of defect that never crashes.
//!
//! # What each is checked against, and why it is independent
//!
//! | helper | check | independent because |
//! |---|---|---|
//! | `rng_next` | **bit-exact** against a `u64` LCG in Rust | the shader emulates 64-bit multiply from 32-bit halves; the reference just uses `u64` |
//! | `locate` | the bracket and interpolation fraction it claims | reconstructs `e` from `(i, f)` rather than re-searching |
//! | `interp_channel` | exact at grid points, linear between | closed form |
//! | `langevin_inverse` | the residual `L(lambda) - x` in `f64` | inverts nothing; evaluates the forward function |
//! | `exponential_mu` | **its mean over uniform input equals `mubar`** | the defining property of the law, not the formula |
//!
//! The `exponential_mu` row is the one that matters most. It samples a cosine
//! from `p(mu) ~ exp(lambda mu)` by inverse transform, and the whole point of
//! `lambda` is that the resulting mean is the `mubar` the cross-section data
//! asked for. Integrating `mu(xi)` over uniform `xi` recovers that mean
//! directly, and a wrong `lambda` — the most likely defect, since
//! `langevin_inverse` is a Newton solve — shows up immediately.
//!
//! # `f32`, deliberately
//!
//! The shader is `f32`. Tolerances are `f32` budgets with the measured number
//! recorded beside them, except `rng_next`, which is integer arithmetic and
//! is required to be exact.
//!
//! # No adapter is not a pass
//!
//! Without a GPU these print `SKIP`. `the_references_are_sound_without_a_gpu`
//! runs everywhere and checks that the references themselves are meaningful,
//! so a device-less CI is not silently testing nothing.
//!
//! # Verified to catch real defects, by injection
//!
//! Each of these was applied to the shader and the named test failed:
//!
//! | injected defect | caught by |
//! |---|---|
//! | `carry = 0u` in the LCG's 64-bit add | `gpu_rng_next_is_bit_identical_to_a_u64_lcg` |
//! | `langevin_inverse(mu_bar) * 1.05` | `gpu_exponential_mu_has_the_mean_it_was_asked_for`, by 1.698e-02 against a 2e-03 bound |
//! | `i_grid = lo + 1` in the binary search | `gpu_locate_brackets_and_reconstructs_the_energy` *and* the interpolation test |
//!
//! The second is the one worth noting: a 5 % error in `lambda` leaves every
//! sampled cosine inside `[-1, 1]`, monotone in `xi`, and entirely plausible.
//! It is a biased sampler and nothing else here would have seen it.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::{probe, GpuContext};

/// The shader under test, included verbatim.
const BATCHED_EVENT_WGSL: &str = include_str!("../src/gpu/shaders/batched_event.wgsl");

/// Entry points appended to it, reusing the module's existing bindings.
///
/// `xs` (0), `istate` (1), `fstate` (2) and `params` (4) are all already
/// declared; nothing new is introduced, so the shader text above stays
/// byte-identical to what ships. `wgpu` derives the bind-group layout from
/// the selected entry point, so each probe needs only the bindings it reads.
const PROBE_ENTRIES: &str = r#"
// istate[2i] = seed lo, istate[2i+1] = seed hi  ->  overwritten with
// (new_lo, new_hi); fstate[i] = the uniform draw.
@compute @workgroup_size(64)
fn probe_rng_next(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&fstate)) { return; }
    let s = vec2<u32>(istate[2u * i], istate[2u * i + 1u]);
    let r = rng_next(s);
    istate[2u * i] = r.x;
    istate[2u * i + 1u] = r.y;
    fstate[i] = bitcast<f32>(r.z);
}

// fstate[3i] = e  ->  fstate[3i+1] = grid index, fstate[3i+2] = fraction.
@compute @workgroup_size(64)
fn probe_locate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&fstate) / 3u) { return; }
    let loc = locate(fstate[3u * i]);
    fstate[3u * i + 1u] = f32(loc.i);
    fstate[3u * i + 2u] = loc.f;
}

// fstate[2i] = e  ->  fstate[2i+1] = interp_channel(base = n_grid, j = 0).
//
// `base = n_grid` is the second block of the packed xs array; the test lays
// the buffer out to match.
@compute @workgroup_size(64)
fn probe_interp_channel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&fstate) / 2u) { return; }
    let loc = locate(fstate[2u * i]);
    fstate[2u * i + 1u] = interp_channel(params.n_grid, 0u, loc);
}

// fstate[2i] = mu_bar  ->  fstate[2i+1] = lambda.
@compute @workgroup_size(64)
fn probe_langevin_inverse(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&fstate) / 2u) { return; }
    fstate[2u * i + 1u] = langevin_inverse(fstate[2u * i]);
}

// fstate[3i] = mubar, fstate[3i+1] = xi  ->  fstate[3i+2] = mu.
@compute @workgroup_size(64)
fn probe_exponential_mu(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&fstate) / 3u) { return; }
    fstate[3u * i + 2u] = exponential_mu(fstate[3u * i], fstate[3u * i + 1u]);
}
"#;

/// The LCG constants, as the shader spells them.
const MULT: u64 = 0x5851_F42D_4C95_7F2D;
const INC: u64 = 0x1405_7B7E_F767_814F;

/// One step of the 64-bit LCG, in `u64`.
///
/// The shader has no 64-bit integers and emulates this from 32-bit halves
/// (`mul_u32_full` / `mul64_low`). This is the same recurrence written the
/// obvious way, so agreement is evidence about that emulation and nothing
/// else. The uniform is the top 24 bits of the new state, matching
/// `f32(new_hi >> 8u) * (1.0 / 16777216.0)`.
fn lcg_step(s: u64) -> (u64, f32) {
    let next = s.wrapping_mul(MULT).wrapping_add(INC);
    let xi = ((next >> 40) as f32) * (1.0 / 16_777_216.0);
    (next, xi)
}

/// The Langevin function `L(lambda) = coth(lambda) - 1/lambda`, in `f64`.
///
/// `langevin_inverse` solves `L(lambda) = x`; this is the forward direction,
/// so checking the residual needs no second solver.
fn langevin(lambda: f64) -> f64 {
    if lambda.abs() < 1e-8 {
        // L(l) -> l/3 as l -> 0; the closed form cancels catastrophically.
        return lambda / 3.0;
    }
    1.0 / lambda.tanh() - 1.0 / lambda
}

/// The energy grid the `locate` and `interp_channel` probes run against.
///
/// Logarithmic, which is what a real cross-section grid looks like, and
/// deliberately non-uniform so an implementation that assumed even spacing
/// would fail.
fn grid() -> Vec<f32> {
    (0..64)
        .map(|k| (1.0e-5_f64 * 10.0_f64.powf(k as f64 * 7.0 / 63.0)) as f32)
        .collect()
}

/// The per-nuclide channel values packed after the grid: a simple ramp, so
/// the linear interpolation has an exact closed form.
fn channel(n: usize) -> Vec<f32> {
    (0..n).map(|k| 1.0 + 0.5 * k as f32).collect()
}

/// Dispatch one appended entry point.
///
/// `fstate` is uploaded to binding 2 and read back; `istate` to binding 1 and
/// read back. `xs` (binding 0) and `params` (binding 4) are supplied when the
/// entry point needs them.
///
/// # The binding list is explicit, and has to be
///
/// naga strips any binding the selected entry point does not reference, and
/// `create_bind_group` then rejects a descriptor with more entries than the
/// derived layout has — "Number of bindings in bind group descriptor (2) does
/// not match the number of bindings defined in the bind group layout (1)".
/// So each probe declares exactly what it reads, and adding a binding to a
/// probe means adding it here too. Supplying a superset does not work.
struct ProbeIo {
    fstate: Vec<f32>,
    istate: Vec<u32>,
}

fn run(
    gpu: &GpuContext,
    entry: &str,
    io: ProbeIo,
    xs: &[f32],
    n_grid: u32,
    threads: usize,
    bindings: &[u32],
) -> ProbeIo {
    use wgpu::util::DeviceExt;

    let mut source = String::from(BATCHED_EVENT_WGSL);
    source.push_str(PROBE_ENTRIES);

    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("batched_event + sampling probes"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    let bytes_f = |v: &[f32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
    let bytes_u = |v: &[u32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };

    // A zero-length storage buffer is invalid; pad the unused one to a single
    // element rather than omitting it, since the entry point may still name it.
    let f_src = if io.fstate.is_empty() {
        vec![0.0_f32]
    } else {
        io.fstate.clone()
    };
    let i_src = if io.istate.is_empty() {
        vec![0_u32]
    } else {
        io.istate.clone()
    };

    let f_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fstate"),
            contents: &bytes_f(&f_src),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
    let i_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("istate"),
            contents: &bytes_u(&i_src),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
    let xs_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("xs"),
            contents: &bytes_f(if xs.is_empty() { &[0.0] } else { xs }),
            usage: wgpu::BufferUsages::STORAGE,
        });
    // Params: (n_grid, n_particle, n_nuclide, pad0) then vec4<f32> sphere.
    let mut params_bytes: Vec<u8> = Vec::new();
    for v in [n_grid, threads as u32, 1_u32, 0_u32] {
        params_bytes.extend_from_slice(&v.to_le_bytes());
    }
    for v in [0.0_f32, 0.0, 0.0, 1.0] {
        params_bytes.extend_from_slice(&v.to_le_bytes());
    }
    let p_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: &params_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
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
    let layout = pipeline.get_bind_group_layout(0);

    let entries: Vec<wgpu::BindGroupEntry> = bindings
        .iter()
        .map(|b| wgpu::BindGroupEntry {
            binding: *b,
            resource: match b {
                0 => xs_buf.as_entire_binding(),
                1 => i_buf.as_entire_binding(),
                2 => f_buf.as_entire_binding(),
                4 => p_buf.as_entire_binding(),
                other => panic!("no buffer registered for binding {other}"),
            },
        })
        .collect();
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &entries,
    });

    let f_size = (f_src.len() * 4) as u64;
    let i_size = (i_src.len() * 4) as u64;
    let f_read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: f_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let i_read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: i_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
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
        pass.dispatch_workgroups(threads.div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&f_buf, 0, &f_read, 0, f_size);
    encoder.copy_buffer_to_buffer(&i_buf, 0, &i_read, 0, i_size);
    gpu.queue.submit(Some(encoder.finish()));

    let fs = f_read.slice(..);
    let is = i_read.slice(..);
    fs.map_async(wgpu::MapMode::Read, |_| {});
    is.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll");
    let fm = fs.get_mapped_range().expect("fstate mapped");
    let im = is.get_mapped_range().expect("istate mapped");
    let fout: Vec<f32> = fm
        .chunks_exact(4)
        .filter_map(|c| <[u8; 4]>::try_from(c).ok())
        .map(f32::from_le_bytes)
        .collect();
    let iout: Vec<u32> = im
        .chunks_exact(4)
        .filter_map(|c| <[u8; 4]>::try_from(c).ok())
        .map(u32::from_le_bytes)
        .collect();
    drop(fm);
    drop(im);
    f_read.unmap();
    i_read.unmap();
    ProbeIo {
        fstate: fout,
        istate: iout,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The references are meaningful, checked without a device.
///
/// A GPU test that skips proves nothing. This one runs everywhere and pins
/// the two properties the GPU comparisons rest on: the Langevin function is
/// invertible over the range tested, and the `exponential_mu` mean identity
/// is genuinely sensitive to `lambda` rather than being satisfied by any
/// monotone map.
#[test]
fn the_references_are_sound_without_a_gpu() {
    // L is strictly increasing on (0, inf) and maps onto (0, 1).
    let mut prev = -1.0_f64;
    for k in 1..=200 {
        let l = 0.05 * k as f64;
        let v = langevin(l);
        assert!(v > prev, "L is not increasing at lambda = {l}");
        assert!((0.0..1.0).contains(&v), "L({l}) = {v} is outside (0, 1)");
        prev = v;
    }

    // The mean identity is sensitive: computing mu(xi) with lambda scaled by
    // 1.05 must move the mean away from mubar by far more than the f32 noise
    // the GPU comparison allows. Otherwise the test below would pass on a
    // wrong lambda.
    let mubar = 0.5_f64;
    let exact =
        |lambda: f64, xi: f64| 1.0 + (xi + (1.0 - xi) * (-2.0 * lambda).exp()).ln() / lambda;
    // Find lambda such that L(lambda) = mubar, by bisection.
    let (mut lo, mut hi) = (1e-6_f64, 50.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if langevin(mid) < mubar {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let lambda = 0.5 * (lo + hi);
    let mean = |l: f64| {
        let n = 20_000;
        (0..n)
            .map(|k| exact(l, (k as f64 + 0.5) / n as f64))
            .sum::<f64>()
            / n as f64
    };
    let right = mean(lambda);
    let wrong = mean(lambda * 1.05);
    assert!(
        (right - mubar).abs() < 1e-4,
        "the mean identity does not hold for the correct lambda: {right} vs {mubar}"
    );
    assert!(
        (wrong - mubar).abs() > 1e-3,
        "a 5% error in lambda moves the mean by only {:e} -- the identity is \
         not a sensitive test and the GPU comparison below would not catch a \
         wrong Newton solve",
        (wrong - mubar).abs()
    );
}

/// `rng_next` on the GPU is **bit-identical** to a `u64` LCG.
///
/// # Why exact and not approximate
///
/// This is integer arithmetic. The shader has no 64-bit integers and
/// emulates the multiply from 32-bit halves; if that emulation is right the
/// state matches exactly, and if it is wrong the sequences diverge
/// completely after one step. There is no middle ground to allow for, so
/// the assertion is equality.
///
/// The uniform draw is also required exact: it is `(state >> 40)` scaled by
/// `2^-24`, which is representable in `f32` without rounding.
#[test]
fn gpu_rng_next_is_bit_identical_to_a_u64_lcg() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_rng_next_is_bit_identical_to_a_u64_lcg: no GPU adapter");
        return;
    };

    // Seeds chosen to exercise carry propagation out of the low word and the
    // high bit of each half, not just small numbers.
    let seeds: Vec<u64> = vec![
        0,
        1,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0xFFFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        0x0123_4567_89AB_CDEF,
        0xDEAD_BEEF_CAFE_F00D,
        42,
        0x7FFF_FFFF_8000_0001,
    ];
    // Several steps, so a defect that only appears once the state is large
    // is reached.
    let mut states: Vec<u64> = seeds.clone();
    for step in 0..8 {
        let istate: Vec<u32> = states
            .iter()
            .flat_map(|s| [*s as u32, (*s >> 32) as u32])
            .collect();
        let out = run(
            &gpu,
            "probe_rng_next",
            ProbeIo {
                fstate: vec![0.0; states.len()],
                istate,
            },
            &[],
            1,
            states.len(),
            // rng_next reads and writes the seed and the draw, nothing else.
            &[1, 2],
        );
        for (k, s) in states.iter_mut().enumerate() {
            let (want_state, want_xi) = lcg_step(*s);
            let lo = out.istate[2 * k] as u64;
            let hi = out.istate[2 * k + 1] as u64;
            let have_state = (hi << 32) | lo;
            assert_eq!(
                have_state, want_state,
                "step {step}, seed index {k}: GPU state {have_state:#018x} != \
                 u64 reference {want_state:#018x}"
            );
            let have_xi = out.fstate[k];
            assert_eq!(
                have_xi.to_bits(),
                want_xi.to_bits(),
                "step {step}, seed index {k}: GPU uniform {have_xi} != reference {want_xi}"
            );
            assert!(
                (0.0..1.0).contains(&have_xi),
                "the uniform draw {have_xi} is outside [0, 1)"
            );
            *s = want_state;
        }
    }
}

/// `locate` returns a bracket that actually contains the energy, and a
/// fraction that reconstructs it.
///
/// Checked by *reconstruction* rather than by re-running a search:
/// `xs[i] + f (xs[i+1] - xs[i])` must be `e`. An off-by-one in the binary
/// search gives a bracket that does not contain `e`, and the fraction then
/// leaves `[0, 1]` — both of which this catches.
#[test]
fn gpu_locate_brackets_and_reconstructs_the_energy() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_locate_brackets_and_reconstructs_the_energy: no GPU adapter");
        return;
    };
    let g = grid();
    let n = g.len();

    // Probe energies: inside every interval, on every grid point, and past
    // both ends where the shader clamps.
    let mut probes: Vec<f32> = Vec::new();
    for k in 0..n - 1 {
        probes.push(g[k]);
        probes.push(0.5 * (g[k] + g[k + 1]));
        probes.push(g[k] + 0.99 * (g[k + 1] - g[k]));
    }
    probes.push(g[n - 1]);
    probes.push(g[0] * 0.1);
    probes.push(g[n - 1] * 10.0);

    let fstate: Vec<f32> = probes.iter().flat_map(|e| [*e, 0.0, 0.0]).collect();
    let out = run(
        &gpu,
        "probe_locate",
        ProbeIo {
            fstate,
            istate: vec![],
        },
        &g,
        n as u32,
        probes.len(),
        // locate reads the grid and params, and writes fstate.
        &[0, 2, 4],
    );

    let mut worst = 0.0_f64;
    for (k, &e) in probes.iter().enumerate() {
        let i = out.fstate[3 * k + 1] as usize;
        let f = out.fstate[3 * k + 2];
        assert!(
            i + 1 < n,
            "energy {e}: locate returned index {i}, which has no upper neighbour"
        );
        let (lo, hi) = (g[i], g[i + 1]);
        let in_range = e >= g[0] && e <= g[n - 1];
        if in_range {
            assert!(
                (lo..=hi).contains(&e),
                "energy {e} is not inside its own bracket [{lo}, {hi}] (index {i})"
            );
            assert!(
                (-1e-5..=1.0 + 1e-5).contains(&f),
                "energy {e}: fraction {f} is outside [0, 1]"
            );
            let recon = lo as f64 + f as f64 * (hi as f64 - lo as f64);
            worst = worst.max((recon - e as f64).abs() / (1.0 + e as f64));
        } else {
            // Out of range clamps to an end interval and extrapolates.
            assert!(
                i == 0 || i == n - 2,
                "out-of-range energy {e} gave index {i}"
            );
        }
    }
    assert!(
        worst < 1e-6,
        "locate's fraction does not reconstruct the energy: {worst:e}"
    );
}

/// `interp_channel` is exact at the grid points and linear between them.
#[test]
fn gpu_interp_channel_is_linear_and_exact_at_the_nodes() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_interp_channel_is_linear_and_exact_at_the_nodes: no GPU adapter");
        return;
    };
    let g = grid();
    let n = g.len();
    let ch = channel(n);
    // Packed as [grid..., channel...] so `base = n_grid`, `j = 0` selects it.
    let mut xs = g.clone();
    xs.extend_from_slice(&ch);

    let mut probes: Vec<f32> = Vec::new();
    for k in 0..n - 1 {
        probes.push(g[k]);
        probes.push(0.5 * (g[k] + g[k + 1]));
        probes.push(g[k] + 0.25 * (g[k + 1] - g[k]));
    }
    let fstate: Vec<f32> = probes.iter().flat_map(|e| [*e, 0.0]).collect();
    let out = run(
        &gpu,
        "probe_interp_channel",
        ProbeIo {
            fstate,
            istate: vec![],
        },
        &xs,
        n as u32,
        probes.len(),
        // interp_channel reads the packed grid + channel and params.
        &[0, 2, 4],
    );

    let mut worst_node = 0.0_f64;
    let mut worst_lin = 0.0_f64;
    for (k, &e) in probes.iter().enumerate() {
        let have = out.fstate[2 * k + 1] as f64;
        // The exact linear interpolant of `ch` on `g` at `e`.
        let mut i = 0usize;
        while i + 2 < n && g[i + 1] < e {
            i += 1;
        }
        let f = (e as f64 - g[i] as f64) / (g[i + 1] as f64 - g[i] as f64);
        let want = (1.0 - f) * ch[i] as f64 + f * ch[i + 1] as f64;
        let d = (have - want).abs() / (1.0 + want.abs());
        if g.contains(&e) {
            worst_node = worst_node.max(d);
        } else {
            worst_lin = worst_lin.max(d);
        }
    }
    assert!(
        worst_node < 1e-6,
        "interp_channel is not exact at the grid nodes: {worst_node:e}"
    );
    assert!(
        worst_lin < 1e-6,
        "interp_channel is not the linear interpolant between nodes: {worst_lin:e}"
    );
}

/// `langevin_inverse` really inverts the Langevin function.
///
/// Checked by residual: for the returned `lambda`, evaluate
/// `L(lambda) = coth(lambda) - 1/lambda` in `f64` and compare against the
/// input. That needs no second solver, so a Newton iteration that converged
/// to the wrong root — or did not converge at all within its 30-step cap —
/// shows up directly.
///
/// # Results, and why the bound is 5e-05 rather than the `f32` floor
///
/// | `mu_bar` | 0.01 | 0.02 | 0.05 | 0.10 | >= 0.10 |
/// |---|---|---|---|---|---|
/// | residual | 2.146e-05 | 4.145e-06 | 2.808e-06 | 7.218e-08 | < 1e-06 |
///
/// The error is worst at the **smallest** `mu_bar` and collapses away from
/// it. That is not the solver: it is the function being solved. At
/// `mu_bar = 0.01` the root is `lambda = 0.0301`, where `coth(lambda)` is
/// 33.34 and `1/lambda` is 33.33 — a difference of 0.01 formed from two
/// numbers 3300 times larger, which discards about 3.5 of `f32`'s 7.2
/// decimal digits before Newton can use them. No iteration count fixes that.
///
/// **It is harmless where this is used.** A 1.610e-05 relative error in
/// `lambda` at `mu_bar = 0.01` shifts the sampled mean by about 1.6e-07,
/// and `gpu_exponential_mu_has_the_mean_it_was_asked_for` holds to 2e-03
/// across the whole range. The shader also short-circuits `|mu_bar| < 1e-4`
/// to isotropic, so the very smallest values never reach the solve at all.
#[test]
fn gpu_langevin_inverse_actually_inverts_the_langevin_function() {
    let Some(gpu) = probe() else {
        eprintln!(
            "SKIP gpu_langevin_inverse_actually_inverts_the_langevin_function: no GPU adapter"
        );
        return;
    };
    // mu_bar in (-1, 1). The shader switches its initial guess at 0.3, so
    // both sides of that are covered, and the odd symmetry is checked too.
    let mut probes: Vec<f32> = Vec::new();
    for k in 1..=95 {
        let x = 0.01 * k as f32;
        probes.push(x);
        probes.push(-x);
    }
    let fstate: Vec<f32> = probes.iter().flat_map(|x| [*x, 0.0]).collect();
    let out = run(
        &gpu,
        "probe_langevin_inverse",
        ProbeIo {
            fstate,
            istate: vec![],
        },
        &[],
        1,
        probes.len(),
        // langevin_inverse is pure arithmetic over fstate.
        &[2],
    );

    let mut worst = 0.0_f64;
    let mut at = 0.0_f32;
    for (k, &x) in probes.iter().enumerate() {
        let lambda = out.fstate[2 * k + 1] as f64;
        assert!(
            lambda.is_finite() && lambda != 0.0,
            "mu_bar = {x}: lambda = {lambda}"
        );
        assert_eq!(
            lambda > 0.0,
            x > 0.0,
            "mu_bar = {x}: lambda = {lambda} has the wrong sign"
        );
        let residual = (langevin(lambda) - x as f64).abs();
        if residual > worst {
            worst = residual;
            at = x;
        }
    }
    // The bound is 5e-05 rather than the f32 floor, and the reason is
    // cancellation in the function being inverted -- see the doc comment.
    assert!(
        worst < 5e-5,
        "langevin_inverse residual |L(lambda) - mu_bar| = {worst:e} at mu_bar = {at}"
    );
    assert_eq!(
        at.abs(),
        0.01,
        "the worst residual is documented as sitting at the SMALLEST |mu_bar|, \
         where coth(lambda) - 1/lambda cancels hardest; it is now at {at}. \
         Re-measure and rewrite the doc comment rather than loosening the bound."
    );

    // Away from zero it collapses to the f32 floor. Pinning both ends is what
    // makes the cancellation explanation falsifiable.
    let mut worst_away = 0.0_f64;
    for (k, &x) in probes.iter().enumerate() {
        if x.abs() < 0.1 {
            continue;
        }
        let lambda = out.fstate[2 * k + 1] as f64;
        worst_away = worst_away.max((langevin(lambda) - x as f64).abs());
    }
    assert!(
        worst_away < 1e-6,
        "for |mu_bar| >= 0.1 the residual is documented as reaching the f32 \
         floor; measured {worst_away:e}"
    );

    // Odd symmetry, which the shader carries through an explicit sign.
    for k in (0..probes.len()).step_by(2) {
        let pos = out.fstate[2 * k + 1];
        let neg = out.fstate[2 * (k + 1) + 1];
        assert_eq!(pos, -neg, "langevin_inverse is not odd at {}", probes[k]);
    }
}

/// `exponential_mu`'s **mean over uniform input is the `mubar` it was
/// given** — the defining property of the law it samples.
///
/// # Why this is the test that matters
///
/// `exponential_mu` samples a scattering cosine from `p(mu) ~ exp(lambda mu)`
/// by inverse transform, and `lambda` comes from a Newton solve. The entire
/// purpose of that solve is that the resulting distribution has mean
/// `mubar`, which is what the evaluated cross-section data specified. If
/// `lambda` is wrong the samples are still in `[-1, 1]`, still monotone in
/// `xi`, still perfectly plausible — and the transport is biased.
///
/// Integrating `mu(xi)` over a uniform grid of `xi` recovers that mean
/// directly. `the_references_are_sound_without_a_gpu` checks that a 5% error
/// in `lambda` moves it by more than the tolerance here, so this is a
/// sensitive test rather than one any monotone map would pass.
#[test]
fn gpu_exponential_mu_has_the_mean_it_was_asked_for() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_exponential_mu_has_the_mean_it_was_asked_for: no GPU adapter");
        return;
    };
    let n_xi = 4096usize;
    let mubars: Vec<f32> = (-9..=9).map(|k| 0.1 * k as f32).collect();

    let mut fstate: Vec<f32> = Vec::with_capacity(mubars.len() * n_xi * 3);
    for &mb in &mubars {
        for j in 0..n_xi {
            let xi = (j as f32 + 0.5) / n_xi as f32;
            fstate.extend_from_slice(&[mb, xi, 0.0]);
        }
    }
    let threads = mubars.len() * n_xi;
    let out = run(
        &gpu,
        "probe_exponential_mu",
        ProbeIo {
            fstate,
            istate: vec![],
        },
        &[],
        1,
        threads,
        // exponential_mu is pure arithmetic over fstate.
        &[2],
    );

    let mut worst = 0.0_f64;
    let mut at = 0.0_f32;
    for (b, &mb) in mubars.iter().enumerate() {
        let mut sum = 0.0_f64;
        let mut prev = f32::NEG_INFINITY;
        for j in 0..n_xi {
            let mu = out.fstate[3 * (b * n_xi + j) + 2];
            assert!(
                (-1.0..=1.0).contains(&mu),
                "mubar = {mb}, xi index {j}: mu = {mu} is outside [-1, 1]"
            );
            // Inverse-transform sampling is monotone in xi by construction.
            assert!(
                mu >= prev - 1e-6,
                "mubar = {mb}: mu is not monotone in xi ({prev} then {mu})"
            );
            prev = mu;
            sum += mu as f64;
        }
        let mean = sum / n_xi as f64;
        let d = (mean - mb as f64).abs();
        if d > worst {
            worst = d;
            at = mb;
        }
    }
    assert!(
        worst < 2e-3,
        "exponential_mu's sampled mean is {worst:e} away from the mubar it \
         was given (worst at mubar = {at})"
    );
}
