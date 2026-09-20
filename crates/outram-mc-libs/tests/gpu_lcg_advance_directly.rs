//! **Direct verification of `lcg_advance`** inside
//! `src/gpu/shaders/batched_flight.wgsl` — the state advance that every
//! batched free flight consumes exactly one of.
//!
//! # Why this exists
//!
//! `batched_flight.wgsl`'s own header calls this function "the reproducibility
//! linchpin" and claims its returned `(rng_hi, rng_lo)` equal the CPU's
//! [`outram_mc_libs::rng::lcg::future_seed`] `(1, seed)` for every particle.
//! Until this file, **nothing anywhere in the crate referenced
//! `lcg_advance`** — the claim was load-bearing and unchecked.
//!
//! It is also the worst kind of thing to leave unchecked. A wrong carry in the
//! emulated 64-bit add does not crash, does not produce out-of-range numbers,
//! and does not make a single history look wrong. It silently decorrelates
//! the GPU stream from the CPU one, so a run stops being reproducible and the
//! tally still converges to something plausible.
//!
//! # What each check is against, and why it is independent
//!
//! | check | reference | independent because |
//! |---|---|---|
//! | one step | `future_seed(1, seed)` | the shader emulates a 64-bit multiply from 32-bit halves; the reference just uses `u64` |
//! | `mul64_low` alone | `u64::wrapping_mul` | isolates the multiply from the add and the carry |
//! | a 4096-step chain | `future_seed(1, .)` iterated | a per-step error that cancels once will not cancel 4096 times |
//! | against `rng_next` | `batched_event.wgsl` | a *second, separately written* kernel that must draw the same stream |
//!
//! The last row is the one worth having. `batched_flight` and `batched_event`
//! implement the same LCG twice, in two files, and a particle's history
//! depends on which kernel drew its number. If the two ever disagree the run
//! is not reproducible no matter which one matches the CPU.
//!
//! # The uniform's divergence is NOT an `f32` effect, and both shader headers
//! # said it was
//!
//! The *integer state* is required to be bit-exact against the CPU. The
//! uniform **value** is not, and both `batched_flight.wgsl` and
//! `batched_event.wgsl` described that as "the accepted `f32` acceleration
//! divergence". **CORRECTED 2026-09-19** — it is structural, not a precision
//! effect, and the headers now say so:
//!
//! * the CPU's [`lcg::prn`] applies a **PCG-RXS-M-XS output permutation** to
//!   the advanced state (`lcg.rs:116-117`) and scales the permuted word by
//!   `2^-64`;
//! * the shaders apply **no permutation at all** and scale the raw top 24
//!   bits of the state by `2^-24`.
//!
//! These are different functions of the same integer. In exact arithmetic
//! they would still disagree. Measured over one million consecutive draws
//! from seed 1, the worst gap is **9.995e-01** — the two are effectively
//! unrelated uniforms, not one rounded copy of the other, and describing that
//! as an `f32` cost understated it by seven orders of magnitude.
//!
//! **The behaviour is nonetheless sound, and that is measured rather than
//! assumed.** An LCG's high bits are its good ones — the permutation exists
//! because OpenMC wanted quality across *all* bits, not because the top ones
//! lack it. Over the same million draws the raw top-24 stream has mean
//! 4.9977e-01, a chi-square of **44.67 on 63 degrees of freedom** over 64
//! equal buckets (the 5 % critical value is 82.5), and a sample covariance
//! against the CPU stream of -6.6e-05, which is inside one standard error of
//! zero. `the_reference_is_sound_without_a_gpu` re-measures all three.
//!
//! So `xi` is checked against the shader's *stated* rule —
//! `f32(state_hi >> 8) / 2^24`, exactly — and the distance from `prn` is
//! measured and recorded, not asserted away.
//!
//! # No adapter is not a pass
//!
//! Without a GPU these print `SKIP`. `the_reference_is_sound_without_a_gpu`
//! runs everywhere, so a device-less CI is not silently testing nothing.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::{probe, GpuContext};
use outram_mc_libs::rng::lcg;

/// The shader under test, included verbatim.
const BATCHED_FLIGHT_WGSL: &str = include_str!("../src/gpu/shaders/batched_flight.wgsl");

/// The second implementation of the same LCG, for the cross-kernel check.
const BATCHED_EVENT_WGSL: &str = include_str!("../src/gpu/shaders/batched_event.wgsl");

/// Probe entry points appended to `batched_flight.wgsl`, reusing its existing
/// `state` (binding 4) buffer and nothing else.
///
/// `wgpu` derives the bind-group layout from the selected entry point, and
/// rejects a bind group carrying an entry the layout does not have. So each
/// probe reads exactly one binding and the harness supplies exactly that one.
const FLIGHT_PROBES: &str = r#"
// state[3i] = seed lo, state[3i+1] = seed hi  ->  overwritten with
// (new_lo, new_hi, bitcast<u32>(xi)).
@compute @workgroup_size(64)
fn probe_lcg_advance(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (3u * i + 2u >= arrayLength(&state)) { return; }
    let r = lcg_advance(state[3u * i], state[3u * i + 1u]);
    state[3u * i] = r.x;
    state[3u * i + 1u] = r.y;
    state[3u * i + 2u] = r.z;
}

// The emulated 64-bit multiply on its own, with no add and no carry:
// state[4i..4i+4] = (a_lo, a_hi, b_lo, b_hi) -> (lo, hi, lo, hi).
@compute @workgroup_size(64)
fn probe_mul64_low(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (4u * i + 3u >= arrayLength(&state)) { return; }
    let m = mul64_low(state[4u * i], state[4u * i + 1u], state[4u * i + 2u], state[4u * i + 3u]);
    state[4u * i] = m.x;
    state[4u * i + 1u] = m.y;
}

// 4096 steps from one seed, so a per-step defect cannot cancel.
@compute @workgroup_size(64)
fn probe_lcg_chain(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (3u * i + 2u >= arrayLength(&state)) { return; }
    var lo = state[3u * i];
    var hi = state[3u * i + 1u];
    var r = vec3<u32>(lo, hi, 0u);
    for (var k: u32 = 0u; k < 4096u; k = k + 1u) {
        r = lcg_advance(lo, hi);
        lo = r.x;
        hi = r.y;
    }
    state[3u * i] = lo;
    state[3u * i + 1u] = hi;
    state[3u * i + 2u] = r.z;
}
"#;

/// The same probe against `batched_event.wgsl`'s `rng_next`, which takes and
/// returns the pair packed differently. `istate` is that module's binding 1.
const EVENT_PROBE: &str = r#"
@compute @workgroup_size(64)
fn probe_rng_next_stream(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (3u * i + 2u >= arrayLength(&istate)) { return; }
    let r = rng_next(vec2<u32>(istate[3u * i], istate[3u * i + 1u]));
    istate[3u * i] = r.x;
    istate[3u * i + 1u] = r.y;
    istate[3u * i + 2u] = bitcast<u32>(r.z);
}
"#;

/// Dispatch one of the probes over `state`, returning it after the run.
///
/// `binding` is the index the entry point's single storage buffer sits at —
/// 4 in `batched_flight.wgsl`, 1 in `batched_event.wgsl`.
fn run(
    gpu: &GpuContext,
    shader: &str,
    probes: &str,
    entry: &str,
    binding: u32,
    state: &[u32],
    threads: usize,
) -> Vec<u32> {
    use wgpu::util::DeviceExt;

    let mut source = String::from(shader);
    source.push_str(probes);

    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(entry),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

    let bytes: Vec<u8> = state.iter().flat_map(|x| x.to_le_bytes()).collect();
    let buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state"),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
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

    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("state"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding,
            resource: buf.as_entire_binding(),
        }],
    });

    let read = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut enc = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(threads.div_ceil(64) as u32, 1, 1);
    }
    enc.copy_buffer_to_buffer(&buf, 0, &read, 0, bytes.len() as u64);
    gpu.queue.submit(Some(enc.finish()));

    let slice = read.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let data = slice.get_mapped_range().expect("mapped readback");
    let out: Vec<u32> = data
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    drop(data);
    read.unmap();
    out
}

/// A spread of seeds: zero, one, the two 32-bit boundaries, `u64::MAX`, and a
/// deterministic scatter. The boundaries are where an emulated carry fails.
fn seeds() -> Vec<u64> {
    let mut s = vec![
        0,
        1,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0xFFFF_FFFF_FFFF_FFFF,
        0x0000_0001_FFFF_FFFF,
        0xFFFF_FFFF_0000_0000,
        0x5851_F42D_4C95_7F2D,
        0x1405_7B7E_F767_814F,
    ];
    let mut x = 0x2545_F491_4F6C_DD1D_u64;
    for _ in 0..247 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(x);
    }
    s
}

/// The shader's own stated rule for the uniform: the top 24 bits of the new
/// 64-bit state, scaled by `2^-24`. Exact in `f32` because the value is
/// below `2^24`.
fn xi_from_state(state: u64) -> f32 {
    ((state >> 40) as f32) * (1.0 / 16_777_216.0)
}

/// The CPU side is meaningful, with or without a device.
///
/// Three separate things, all of which every GPU comparison below rests on:
///
/// 1. `future_seed(1, .)` really is the LCG recurrence, so it is a reference
///    at all.
/// 2. The shader's uniform rule lands in `[0, 1)` and is **not** a rounding
///    of the CPU's `prn` — the headers used to say it was. Measured worst gap
///    9.995e-01 over a million draws.
/// 3. The raw top-24 stream is nonetheless a sound uniform, which is the
///    entire justification for dropping the PCG permutation. Mean
///    4.9977e-01; chi-square 44.67 on 63 dof over 64 buckets against a 5 %
///    critical value of 82.5; covariance against the CPU stream -6.6e-05.
///
/// Point 3 is the one that would matter if it failed. A biased uniform in the
/// flight kernel biases every free path, and nothing downstream would report
/// it as anything but a slightly different answer.
#[test]
fn the_reference_is_sound_without_a_gpu() {
    const MULT: u64 = 0x5851_F42D_4C95_7F2D;
    const INC: u64 = 0x1405_7B7E_F767_814F;

    // 1. The recurrence.
    for s in seeds() {
        assert_eq!(
            lcg::future_seed(1, s),
            s.wrapping_mul(MULT).wrapping_add(INC),
            "future_seed(1, {s:#x}) is not the LCG recurrence"
        );
        assert!(
            (0.0..1.0).contains(&xi_from_state(lcg::future_seed(1, s))),
            "xi out of range for {s:#x}"
        );
    }

    // 2 and 3, over one million consecutive draws from the same seed.
    const N: usize = 1_000_000;
    const BUCKETS: usize = 64;
    let (mut permuted_seed, mut raw_state) = (1_u64, 1_u64);
    let (mut worst, mut sum_x, mut sum_p, mut sum_xp) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut hist = [0_usize; BUCKETS];
    for _ in 0..N {
        let p = lcg::prn(&mut permuted_seed);
        raw_state = lcg::future_seed(1, raw_state);
        let x = xi_from_state(raw_state) as f64;
        worst = worst.max((p - x).abs());
        sum_x += x;
        sum_p += p;
        sum_xp += x * p;
        let b = ((x * BUCKETS as f64) as usize).min(BUCKETS - 1);
        hist[b] += 1;
    }
    let (mean_x, mean_p) = (sum_x / N as f64, sum_p / N as f64);

    // 2. The divergence is structural, not a rounding. If this ever came back
    // small, the shaders would have grown a permutation and the headers would
    // need rewriting -- so it is asserted LARGE, deliberately.
    assert!(
        worst > 0.9,
        "the shader uniform is documented as a DIFFERENT function of the \
         state from the CPU prn (no PCG-RXS-M-XS permutation), with a \
         measured worst gap of 9.995e-01. It measured {worst:e}, which would \
         mean the two now agree -- re-read both shader headers rather than \
         loosening this"
    );

    // 3. And the raw top-24 stream is a sound uniform in its own right.
    assert!(
        (mean_x - 0.5).abs() < 2.0e-3,
        "top-24 uniform mean {mean_x:.8}, documented 4.9977e-01"
    );
    let expect = N as f64 / BUCKETS as f64;
    let chi2: f64 = hist
        .iter()
        .map(|&b| {
            let d = b as f64 - expect;
            d * d / expect
        })
        .sum();
    assert!(
        chi2 < 92.0,
        "top-24 uniform chi-square {chi2:.2} on {} dof; documented 44.67, \
         and 92.0 is the 1 % critical value",
        BUCKETS - 1
    );
    // Two independent U(0,1) streams have sample covariance of order
    // 1/(12 sqrt(N)) = 8.3e-05 here, so 5e-04 is six standard errors.
    let cov = sum_xp / N as f64 - mean_x * mean_p;
    assert!(
        cov.abs() < 5.0e-4,
        "top-24 and CPU streams covary at {cov:e}; documented -6.6e-05, which \
         is inside one standard error of zero"
    );
}

/// `lcg_advance` advances the 64-bit state **bit-exactly** against the CPU.
///
/// This is the claim `batched_flight.wgsl`'s header makes and the one the
/// reproducibility of a GPU run rests on. It is asserted as equality, not a
/// tolerance: the state is an integer and there is nothing to round.
///
/// The seed set deliberately includes `0xFFFF_FFFF`, `0x1_0000_0000` and
/// `u64::MAX`, which are where an emulated 32-bit carry goes wrong.
#[test]
fn gpu_lcg_advance_is_bit_exact_against_the_cpu_lcg() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_lcg_advance_is_bit_exact_against_the_cpu_lcg: no GPU adapter");
        return;
    };
    let s = seeds();
    let mut state = Vec::with_capacity(3 * s.len());
    for &seed in &s {
        state.push(seed as u32);
        state.push((seed >> 32) as u32);
        state.push(0);
    }
    let out = run(
        &gpu,
        BATCHED_FLIGHT_WGSL,
        FLIGHT_PROBES,
        "probe_lcg_advance",
        4,
        &state,
        s.len(),
    );

    for (k, &seed) in s.iter().enumerate() {
        let want = lcg::future_seed(1, seed);
        let lo = out[3 * k] as u64;
        let hi = out[3 * k + 1] as u64;
        assert_eq!(
            (hi << 32) | lo,
            want,
            "lcg_advance from {seed:#x}: GPU {:#x}, CPU {want:#x}",
            (hi << 32) | lo
        );
        // The uniform follows the shader's stated top-24-bit rule exactly.
        let xi = f32::from_bits(out[3 * k + 2]);
        assert_eq!(
            xi.to_bits(),
            xi_from_state(want).to_bits(),
            "xi from {seed:#x}: GPU {xi}, rule {}",
            xi_from_state(want)
        );
    }
}

/// The emulated multiply on its own, isolated from the add and the carry.
///
/// `lcg_advance` being right could in principle come from two errors
/// cancelling. This checks `mul64_low` against `u64::wrapping_mul` directly,
/// over pairs including both 32-bit boundaries, so the multiply is pinned
/// independently of what is done with it.
#[test]
fn gpu_mul64_low_is_exact_against_a_u64_multiply() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_mul64_low_is_exact_against_a_u64_multiply: no GPU adapter");
        return;
    };
    let s = seeds();
    let pairs: Vec<(u64, u64)> = s
        .iter()
        .enumerate()
        .map(|(i, &a)| (a, s[(i * 7 + 3) % s.len()]))
        .collect();

    let mut state = Vec::with_capacity(4 * pairs.len());
    for &(a, b) in &pairs {
        state.push(a as u32);
        state.push((a >> 32) as u32);
        state.push(b as u32);
        state.push((b >> 32) as u32);
    }
    let out = run(
        &gpu,
        BATCHED_FLIGHT_WGSL,
        FLIGHT_PROBES,
        "probe_mul64_low",
        4,
        &state,
        pairs.len(),
    );

    for (k, &(a, b)) in pairs.iter().enumerate() {
        let got = ((out[4 * k + 1] as u64) << 32) | out[4 * k] as u64;
        assert_eq!(
            got,
            a.wrapping_mul(b),
            "mul64_low({a:#x}, {b:#x}): GPU {got:#x}, CPU {:#x}",
            a.wrapping_mul(b)
        );
    }
}

/// 4096 consecutive steps stay locked to the CPU stream.
///
/// One step agreeing is necessary and not sufficient: a defect that happens to
/// cancel at a particular state would pass the single-step test. Over 4096
/// steps from 256 different seeds it cannot.
#[test]
fn gpu_a_long_chain_stays_locked_to_the_cpu_stream() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_a_long_chain_stays_locked_to_the_cpu_stream: no GPU adapter");
        return;
    };
    let s = seeds();
    let mut state = Vec::with_capacity(3 * s.len());
    for &seed in &s {
        state.push(seed as u32);
        state.push((seed >> 32) as u32);
        state.push(0);
    }
    let out = run(
        &gpu,
        BATCHED_FLIGHT_WGSL,
        FLIGHT_PROBES,
        "probe_lcg_chain",
        4,
        &state,
        s.len(),
    );

    for (k, &seed) in s.iter().enumerate() {
        let mut want = seed;
        for _ in 0..4096 {
            want = lcg::future_seed(1, want);
        }
        let got = ((out[3 * k + 1] as u64) << 32) | out[3 * k] as u64;
        assert_eq!(
            got, want,
            "4096 steps from {seed:#x}: GPU {got:#x}, CPU {want:#x}"
        );
    }
}

/// **The two kernels draw the same stream.**
///
/// `batched_flight.wgsl` and `batched_event.wgsl` each carry their own copy of
/// the LCG — `lcg_advance` and `rng_next` — and a particle's numbers come from
/// whichever kernel happens to be running. If they ever disagree the run is
/// not reproducible even though each might still match some reference.
///
/// Both the state **and** the `f32` uniform are required to agree exactly.
/// The uniform can be: both take the same top 24 bits, so this is integer
/// arithmetic wearing a float, not a rounding question.
#[test]
fn gpu_the_two_kernels_draw_the_same_stream() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_the_two_kernels_draw_the_same_stream: no GPU adapter");
        return;
    };
    let s = seeds();
    let mut state = Vec::with_capacity(3 * s.len());
    for &seed in &s {
        state.push(seed as u32);
        state.push((seed >> 32) as u32);
        state.push(0);
    }

    let flight = run(
        &gpu,
        BATCHED_FLIGHT_WGSL,
        FLIGHT_PROBES,
        "probe_lcg_advance",
        4,
        &state,
        s.len(),
    );
    let event = run(
        &gpu,
        BATCHED_EVENT_WGSL,
        EVENT_PROBE,
        "probe_rng_next_stream",
        1,
        &state,
        s.len(),
    );

    for (k, &seed) in s.iter().enumerate() {
        assert_eq!(
            (flight[3 * k], flight[3 * k + 1]),
            (event[3 * k], event[3 * k + 1]),
            "state from {seed:#x}: batched_flight and batched_event disagree"
        );
        assert_eq!(
            flight[3 * k + 2],
            event[3 * k + 2],
            "uniform from {seed:#x}: batched_flight gave {}, batched_event {}",
            f32::from_bits(flight[3 * k + 2]),
            f32::from_bits(event[3 * k + 2])
        );
    }
}
