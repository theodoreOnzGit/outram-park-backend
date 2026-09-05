//! Optional **wgpu GPU acceleration** for large Walk-on-Spheres ensembles.
//!
//! Follows the workspace GPU rules and the `outram-park-fork-pflotran` /
//! `outram-mc-libs` precedent:
//!
//! 1. **Android-gated.** The whole module is compiled only under
//!    `cfg(not(target_os = "android"))` (wired in `lib.rs`); the Android/Termux
//!    build never sees `wgpu`, keeping the library headless-buildable there.
//! 2. **CPU is the trusted path; GPU is acceleration only.** The `f64` CPU
//!    reference is the rayon ensemble
//!    ([`parallel_kernel_release_fraction`]); the GPU kernel runs in `f32` and
//!    is an *approximation*. [`kernel_release_fraction_best_effort`] probes for a
//!    device and silently falls back to the CPU when there is no adapter
//!    (headless CI, no `/dev/dri`) or the submit fails.
//!
//! The kernel runs one **single-sphere Walk-on-Spheres history per GPU thread**
//! (the IAEA CRP-6 Case 1 bare-kernel release): each thread starts an atom
//! uniformly in the kernel, hops it to the perfect-sink surface using the same
//! first-passage-time table as the CPU engine (uploaded once), and writes its
//! release time. The host then reduces the release times to a release fraction.
//! This is the embarrassingly-parallel core that makes a very large, real-time
//! ensemble feasible; extending the kernel to the full multilayer geometry with
//! interfaces and depletion is the next buildout.
//!
//! > **GPU path unverified in this environment.** Developed where no GPU adapter
//! > was available, so only the CPU fallback was exercised here. The GPU dispatch
//! > must be validated on a GPU-equipped host before it is trusted. The kernel is
//! > written to mirror the verified CPU `walk_to_absorbing_sphere` logic; the
//! > per-thread `f32` RNG is a lightweight xorshift adequate for a demonstrator.

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::length::{meter, micrometer};
use uom::si::time::second;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::ensemble::{
    parallel_kernel_release_fraction, EnsembleConfig,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::sphere_fpt::dimensionless_exit_time_table;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    WalkParams, WoSWalker,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::kernel_diffusion_coefficient;

/// Number of points in the first-passage-time table uploaded to the GPU.
const THETA_TABLE_LEN: usize = 4096;
/// Per-thread hop cap (single-sphere Walk-on-Spheres converges in ~tens of hops).
const MAX_HOPS: u32 = 1024;
const WORKGROUP_SIZE: u32 = 64;

/// A ready wgpu device + queue for headless compute.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Adapter description (name/backend), for a UI to show which GPU is in use.
    info: wgpu::AdapterInfo,
}

impl GpuContext {
    /// Human-readable adapter name (e.g. the GPU model), for UI display.
    pub fn adapter_name(&self) -> &str {
        &self.info.name
    }
}

/// A recoverable GPU failure — the caller should fall back to the CPU path.
#[derive(Debug)]
pub enum GpuError {
    /// The device could not be polled to completion (device lost).
    Poll(String),
    /// The readback buffer could not be mapped.
    Map(String),
    /// The map callback never fired.
    MapCallbackMissing,
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::Poll(e) => write!(f, "gpu poll failed: {e}"),
            GpuError::Map(e) => write!(f, "gpu buffer map failed: {e}"),
            GpuError::MapCallbackMissing => write!(f, "gpu map callback missing"),
        }
    }
}

impl std::error::Error for GpuError {}

/// A process-wide cached [`GpuContext`], probed once on first use.
///
/// Opening a `wgpu` device is expensive, so the per-frame animation path must not
/// [`probe`] every frame. The first call probes; every later call reuses the same
/// device (or the same `None` when there is no adapter). The context is `Send +
/// Sync`, so the returned reference is safe to use from the render/worker thread.
pub fn cached_context() -> Option<&'static GpuContext> {
    static CTX: std::sync::OnceLock<Option<GpuContext>> = std::sync::OnceLock::new();
    CTX.get_or_init(probe).as_ref()
}

/// Whether a GPU adapter is available (probed once, cached). For a UI to show the
/// effective backend without re-opening a device each frame.
pub fn gpu_available() -> bool {
    cached_context().is_some()
}

/// Probe for a GPU adapter and open a device. Returns `None` when there is no
/// usable adapter — a **normal, expected** outcome on headless CI / no-GPU hosts,
/// not an error; the caller then uses the CPU path.
///
/// Prefer [`cached_context`] on any hot path — this opens a fresh device each
/// call.
pub fn probe() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .ok()?;
    let info = adapter.get_info();
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("boon-lay walk-on-spheres compute"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()?;
    Some(GpuContext {
        device,
        queue,
        info,
    })
}

/// WGSL: one single-sphere Walk-on-Spheres history per invocation.
///
/// Mirrors `walk_to_absorbing_sphere`: uniform-in-ball start, hop of radius
/// `R = radius - |pos|` to a uniform surface point, accrue `theta * R^2 / D`, stop
/// within `eps` of the surface. `theta` is sampled from the uploaded uniform-CDF
/// table by linear interpolation. `f32` precision throughout.
const WALK_ON_SPHERES_WGSL: &str = r#"
struct Params {
    radius: f32,
    d: f32,
    eps: f32,
    table_len: u32,
    max_hops: u32,
    n_hist: u32,
    seed: u32,
    pad: u32,
};
@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> theta_table: array<f32>;
@group(0) @binding(2) var<storage, read_write> release_time: array<f32>;

fn rng_next(state: ptr<function, u32>) -> f32 {
    var x = *state;
    x = x ^ (x << 13u);
    x = x ^ (x >> 17u);
    x = x ^ (x << 5u);
    *state = x;
    return f32(x) * (1.0 / 4294967296.0);
}

fn sample_theta(u: f32) -> f32 {
    let m = P.table_len;
    let f = clamp(u, 0.0, 0.999999);
    let xpos = f * f32(m - 1u);
    let j = u32(floor(xpos));
    let frac = xpos - f32(j);
    let a = theta_table[j];
    let b = theta_table[min(j + 1u, m - 1u)];
    return a + frac * (b - a);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n_hist) { return; }

    var state = P.seed ^ (i * 2654435761u) ^ (i << 7u) ^ 0x9e3779b9u;
    if (state == 0u) { state = 1u; }

    // Uniform-in-ball start: r = R * u^(1/3), isotropic direction.
    let rr = P.radius * pow(rng_next(&state), 1.0 / 3.0);
    let mu0 = 2.0 * rng_next(&state) - 1.0;
    let phi0 = 6.28318530718 * rng_next(&state);
    let s0 = sqrt(max(0.0, 1.0 - mu0 * mu0));
    var pos = vec3<f32>(rr * s0 * cos(phi0), rr * s0 * sin(phi0), rr * mu0);

    var t = 0.0;
    var released = false;
    for (var h: u32 = 0u; h < P.max_hops; h = h + 1u) {
        let rho = length(pos);
        let hop = P.radius - rho;
        if (hop <= P.eps) { released = true; break; }
        let theta = sample_theta(rng_next(&state));
        t = t + theta * hop * hop / P.d;
        let mu = 2.0 * rng_next(&state) - 1.0;
        let phi = 6.28318530718 * rng_next(&state);
        let s = sqrt(max(0.0, 1.0 - mu * mu));
        pos = pos + hop * vec3<f32>(s * cos(phi), s * sin(phi), mu);
    }

    if (released) { release_time[i] = t; } else { release_time[i] = -1.0; }
}
"#;

/// GPU single-sphere kernel release fraction (`f32`). Fallible — the caller
/// should fall back to [`parallel_kernel_release_fraction`] on `Err`.
pub fn try_kernel_release_fraction_gpu(
    ctx: &GpuContext,
    nuclide: Nuclide,
    kernel_radius: Length,
    temperature: ThermodynamicTemperature,
    time: Time,
    config: &EnsembleConfig,
) -> Result<f64, GpuError> {
    if config.n_histories == 0 {
        return Ok(0.0);
    }
    let device = &ctx.device;
    let queue = &ctx.queue;

    let d = kernel_diffusion_coefficient(nuclide, temperature).get::<square_meter_per_second>();
    let radius_m = kernel_radius.get::<meter>();
    let eps_m = radius_m * 1e-3;
    let query_s = time.get::<second>();

    let theta_table: Vec<f32> = dimensionless_exit_time_table(THETA_TABLE_LEN)
        .iter()
        .map(|&v| v as f32)
        .collect();
    let table_bytes = f32_slice_to_bytes(&theta_table);

    let n = config.n_histories as u32;
    let out_bytes = (config.n_histories * 4) as wgpu::BufferAddress;

    // Params: [radius, d, eps]f32 + [table_len, max_hops, n_hist, seed, pad]u32.
    let mut params_bytes = Vec::with_capacity(32);
    for f in [radius_m as f32, d as f32, eps_m as f32] {
        params_bytes.extend_from_slice(&f.to_le_bytes());
    }
    for u in [
        THETA_TABLE_LEN as u32,
        MAX_HOPS,
        n,
        config.base_seed as u32,
        0u32,
    ] {
        params_bytes.extend_from_slice(&u.to_le_bytes());
    }

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wos-params"),
        size: params_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &params_bytes);

    let table_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wos-theta-table"),
        size: table_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&table_buffer, 0, &table_bytes);

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wos-release-time"),
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wos-readback"),
        size: out_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("wos-wgsl"),
        source: wgpu::ShaderSource::Wgsl(WALK_ON_SPHERES_WGSL.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("wos-pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("wos-bind-group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: table_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("wos-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("wos-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(n.div_ceil(WORKGROUP_SIZE), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, out_bytes);
    queue.submit(Some(encoder.finish()));

    let mapped = Arc::new(Mutex::new(None));
    let mapped_cb = Arc::clone(&mapped);
    readback_buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |res| {
            *mapped_cb.lock().unwrap() = Some(res);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| GpuError::Poll(format!("{e:?}")))?;
    match mapped.lock().unwrap().take() {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(GpuError::Map(format!("{e:?}"))),
        None => return Err(GpuError::MapCallbackMissing),
    }
    let times: Vec<f32> = {
        let view = readback_buffer
            .slice(..)
            .get_mapped_range()
            .expect("staging buffer mapping failed after a completed poll");
        bytes_to_f32_vec(&view)
    };
    readback_buffer.unmap();

    let released = times
        .iter()
        .filter(|&&t| t >= 0.0 && t as f64 <= query_s)
        .count();
    Ok(released as f64 / config.n_histories as f64)
}

/// Single-sphere kernel release fraction using the GPU when available, otherwise
/// the rayon CPU reference. Never fails: falls back to
/// [`parallel_kernel_release_fraction`] when there is no GPU adapter or the GPU
/// submit errors. GPU results are `f32`-precision.
pub fn kernel_release_fraction_best_effort(
    nuclide: Nuclide,
    kernel_radius: Length,
    temperature: ThermodynamicTemperature,
    time: Time,
    config: &EnsembleConfig,
) -> f64 {
    if let Some(ctx) = probe() {
        if let Ok(f) =
            try_kernel_release_fraction_gpu(&ctx, nuclide, kernel_radius, temperature, time, config)
        {
            return f;
        }
    }
    parallel_kernel_release_fraction(nuclide, kernel_radius, temperature, time, config)
}

// ===========================================================================
// Full multilayer Walk-on-Spheres animation kernel
// ===========================================================================
//
// Advances a whole ensemble of walkers by a slice of simulated time through all
// five TRISO layers, honouring the linear-`D` interface rule, and reads back the
// updated positions/times/release flags. It mirrors the CPU
// `WoSWalker::step_multilayer` / `diffuse_until` exactly (region lookup, hop
// sizing, interface transmit/reflect + reinsertion, absorbing OPyC surface),
// in `f32` and in **micrometres / µm²·s⁻¹** (chosen over metres so the ~1e-4 m
// geometry and ~1e-8 m capture tolerance stay well-conditioned in `f32`).
//
// The kernel is the per-frame accelerator for the live GUI: the host keeps the
// authoritative walker state on the CPU, uploads it, advances to `until` on the
// GPU, and downloads the new state each frame. GPU `f32` is acceleration only;
// the CPU `f64` path is the trusted reference.

/// Length of the θ table uploaded for the multilayer kernel.
const ML_THETA_TABLE_LEN: usize = 4096;
/// Per-thread step cap for one frame's advance (matches the CPU step budget
/// scale; a frame advances at most this many hops per walker).
const ML_MAX_STEPS: u32 = 200_000;

const ADVANCE_MULTILAYER_WGSL: &str = r#"
struct Params {
    eps: f32,             // capture tolerance (um)
    reinsert: f32,        // reinsert distance (um) = eps * reinsert_factor
    partition_k: f32,
    until: f32,           // target simulated time (s)
    n: u32,
    max_steps: u32,
    seed: u32,
    table_len: u32,
};
@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> theta_table: array<f32>;
// geom[0..5) = layer outer radii (um): fuel,buffer,ipyc,sic,opyc
// geom[5..10) = per-layer diffusion coeff (um^2/s)
@group(0) @binding(2) var<storage, read> geom: array<f32>;
@group(0) @binding(3) var<storage, read_write> pos_time: array<vec4<f32>>; // xyz=pos(um), w=time(s)
@group(0) @binding(4) var<storage, read_write> rng_flags: array<vec2<u32>>; // x=rng, y=released

fn rng_next(state: ptr<function, u32>) -> f32 {
    var x = *state;
    x = x ^ (x << 13u);
    x = x ^ (x >> 17u);
    x = x ^ (x << 5u);
    *state = x;
    return f32(x) * (1.0 / 4294967296.0);
}

fn sample_theta(u: f32) -> f32 {
    let m = P.table_len;
    let f = clamp(u, 0.0, 0.999999);
    let xpos = f * f32(m - 1u);
    let j = u32(floor(xpos));
    let frac = xpos - f32(j);
    let a = theta_table[j];
    let b = theta_table[min(j + 1u, m - 1u)];
    return a + frac * (b - a);
}

// First region index k in 0..5 with rho < geom[k]; 5 means outside the particle.
fn region_of(rho: f32) -> u32 {
    for (var k: u32 = 0u; k < 5u; k = k + 1u) {
        if (rho < geom[k]) { return k; }
    }
    return 5u;
}

fn p_transmit(d1: f32, d2: f32, k: f32) -> f32 {
    let num = k * d2;
    let den = d1 + num;
    if (den <= 0.0) { return 0.0; }
    return num / den;
}

// NOTE: the radius parameter must NOT be named `target` -- that is a reserved
// keyword in WGSL, and naga rejects the whole shader module at parse time
// ("name `target` is a reserved keyword"), which takes down the entire GPU
// path on every adapter, not just some.
fn set_radius(p: ptr<function, vec3<f32>>, target_radius: f32) {
    let rho = length(*p);
    if (rho <= 0.0) {
        *p = vec3<f32>(target_radius, 0.0, 0.0);
    } else {
        *p = *p * (target_radius / rho);
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) { return; }

    var flags = rng_flags[i];
    if (flags.y != 0u) { return; } // already released — leave state untouched

    var pt = pos_time[i];
    var p = pt.xyz;
    var t = pt.w;
    var state = flags.x;
    if (state == 0u) { state = P.seed ^ (i * 2654435761u) ^ 0x9e3779b9u; }

    var released = false;
    for (var h: u32 = 0u; h < P.max_steps; h = h + 1u) {
        if (t >= P.until) { break; }
        let rho = length(p);
        let region = region_of(rho);
        if (region >= 5u) { released = true; break; }

        let r_out = geom[region];
        let has_inner = region > 0u;
        var r_in = 0.0;
        if (has_inner) { r_in = geom[region - 1u]; }
        let dist_out = r_out - rho;
        var dist_in = 1.0e30;
        if (has_inner) { dist_in = rho - r_in; }
        let hop = min(dist_out, dist_in);
        let d_cur = geom[5u + region];

        if (hop > P.eps) {
            let theta = sample_theta(rng_next(&state));
            t = t + theta * hop * hop / d_cur;
            let mu = 2.0 * rng_next(&state) - 1.0;
            let phi = 6.28318530718 * rng_next(&state);
            let s = sqrt(max(0.0, 1.0 - mu * mu));
            p = p + hop * vec3<f32>(s * cos(phi), s * sin(phi), mu);
            continue;
        }

        // Reached the nearest interface.
        let at_outer = dist_out <= dist_in;
        if (at_outer) {
            if (region == 4u) { released = true; break; } // OPyC outer = sink
            let d_next = geom[5u + region + 1u];
            if (rng_next(&state) < p_transmit(d_cur, d_next, P.partition_k)) {
                set_radius(&p, r_out + P.reinsert);
            } else {
                set_radius(&p, r_out - P.reinsert);
            }
        } else {
            let d_next = geom[5u + region - 1u];
            if (rng_next(&state) < p_transmit(d_cur, d_next, P.partition_k)) {
                set_radius(&p, r_in - P.reinsert);
            } else {
                set_radius(&p, r_in + P.reinsert);
            }
        }
    }

    pos_time[i] = vec4<f32>(p, t);
    var relu: u32 = 0u;
    if (released) { relu = 1u; }
    rng_flags[i] = vec2<u32>(state, relu);
}
"#;

/// Per-layer diffusion coefficients (µm²/s) for `nuclide` in `cell`, indexed
/// fuel, buffer, IPyC, SiC, OPyC — the same values the CPU path reads, sampled at
/// a representative radius in each layer.
fn layer_diffusion_um2_s(cell: &TrisoCell, nuclide: Nuclide) -> [f32; 5] {
    let radii = layer_radii_um(cell);
    // Representative radius (µm) inside each layer: kernel centre, then midpoints.
    let mids = [
        0.0_f64,
        0.5 * (radii[0] as f64 + radii[1] as f64),
        0.5 * (radii[1] as f64 + radii[2] as f64),
        0.5 * (radii[2] as f64 + radii[3] as f64),
        0.5 * (radii[3] as f64 + radii[4] as f64),
    ];
    let mut d = [0.0_f32; 5];
    for (k, &r_um) in mids.iter().enumerate() {
        let point = [
            Length::new::<micrometer>(r_um),
            Length::new::<meter>(0.0),
            Length::new::<meter>(0.0),
        ];
        let d_m2s = cell
            .try_get_diffusion_coefficient(point, nuclide)
            .map(|c| c.get::<square_meter_per_second>())
            .unwrap_or(1e-6);
        d[k] = (d_m2s * 1.0e12) as f32; // m²/s → µm²/s
    }
    d
}

/// The five layer outer radii (µm): fuel, buffer, IPyC, SiC, OPyC.
fn layer_radii_um(cell: &TrisoCell) -> [f32; 5] {
    [
        cell.get_fuel_radius().get::<micrometer>() as f32,
        cell.get_buffer_radius().get::<micrometer>() as f32,
        cell.get_ipyc_radius().get::<micrometer>() as f32,
        cell.get_sic_radius().get::<micrometer>() as f32,
        cell.get_opyc_radius().get::<micrometer>() as f32,
    ]
}

/// Advance an ensemble one frame on the GPU (fallible). On `Ok`, `walkers` and
/// `released` are updated in place with the GPU's `f32` result.
///
/// Mirrors [`WoSWalker::diffuse_until`] for every walker up to `until`. Already
/// released walkers are left untouched.
pub fn try_advance_multilayer_gpu(
    ctx: &GpuContext,
    cell: &TrisoCell,
    params: &WalkParams,
    walkers: &mut [WoSWalker],
    released: &mut [bool],
    nuclide: Nuclide,
    until: Time,
) -> Result<(), GpuError> {
    let n = walkers.len();
    if n == 0 {
        return Ok(());
    }
    let device = &ctx.device;
    let queue = &ctx.queue;

    let radii = layer_radii_um(cell);
    let diff = layer_diffusion_um2_s(cell, nuclide);
    let eps_um = params.capture_eps.get::<micrometer>() as f32;
    let reinsert_um = eps_um * params.reinsert_factor as f32;
    let until_s = until.get::<second>() as f32;

    // geom storage buffer: 5 radii + 5 D (µm, µm²/s).
    let mut geom = [0.0_f32; 10];
    geom[..5].copy_from_slice(&radii);
    geom[5..].copy_from_slice(&diff);
    let geom_bytes = f32_slice_to_bytes(&geom);

    let theta_table: Vec<f32> = dimensionless_exit_time_table(ML_THETA_TABLE_LEN)
        .iter()
        .map(|&v| v as f32)
        .collect();
    let table_bytes = f32_slice_to_bytes(&theta_table);

    // Per-walker state: pos_time (vec4 f32) + rng_flags (vec2 u32).
    let mut pos_time_bytes = Vec::with_capacity(n * 16);
    let mut rng_flags_bytes = Vec::with_capacity(n * 8);
    for (w, &r) in walkers.iter().zip(released.iter()) {
        let x = w.position[0].get::<micrometer>() as f32;
        let y = w.position[1].get::<micrometer>() as f32;
        let z = w.position[2].get::<micrometer>() as f32;
        let t = w.time.get::<second>() as f32;
        for v in [x, y, z, t] {
            pos_time_bytes.extend_from_slice(&v.to_le_bytes());
        }
        let rng = w.rng.0 as u32;
        let rel = u32::from(r);
        rng_flags_bytes.extend_from_slice(&rng.to_le_bytes());
        rng_flags_bytes.extend_from_slice(&rel.to_le_bytes());
    }

    // Params uniform: [eps, reinsert, k, until]f32 + [n, max_steps, seed, table_len]u32.
    let mut params_bytes = Vec::with_capacity(32);
    for f in [eps_um, reinsert_um, params.partition_k as f32, until_s] {
        params_bytes.extend_from_slice(&f.to_le_bytes());
    }
    for u in [
        n as u32,
        ML_MAX_STEPS,
        walkers[0].rng.0 as u32 ^ 0x51ed_u32,
        ML_THETA_TABLE_LEN as u32,
    ] {
        params_bytes.extend_from_slice(&u.to_le_bytes());
    }

    let mk_storage = |label, data: &[u8], rw: bool| {
        let usage = if rw {
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
        } else {
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
        };
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: data.len() as wgpu::BufferAddress,
            usage,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, data);
        buf
    };

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ml-params"),
        size: params_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &params_bytes);

    let table_buffer = mk_storage("ml-theta", &table_bytes, false);
    let geom_buffer = mk_storage("ml-geom", &geom_bytes, false);
    let pos_time_buffer = mk_storage("ml-pos-time", &pos_time_bytes, true);
    let rng_flags_buffer = mk_storage("ml-rng-flags", &rng_flags_bytes, true);

    let pos_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ml-pos-readback"),
        size: pos_time_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let flags_readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ml-flags-readback"),
        size: rng_flags_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ml-wgsl"),
        source: wgpu::ShaderSource::Wgsl(ADVANCE_MULTILAYER_WGSL.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ml-pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ml-bind-group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: table_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: geom_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: pos_time_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: rng_flags_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ml-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("ml-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups((n as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
    }
    encoder.copy_buffer_to_buffer(
        &pos_time_buffer,
        0,
        &pos_readback,
        0,
        pos_time_bytes.len() as wgpu::BufferAddress,
    );
    encoder.copy_buffer_to_buffer(
        &rng_flags_buffer,
        0,
        &flags_readback,
        0,
        rng_flags_bytes.len() as wgpu::BufferAddress,
    );
    queue.submit(Some(encoder.finish()));

    map_and_wait(device, &pos_readback)?;
    map_and_wait(device, &flags_readback)?;

    let pos_data: Vec<f32> = {
        let view = pos_readback
            .slice(..)
            .get_mapped_range()
            .expect("staging buffer mapping failed after a completed poll");
        bytes_to_f32_vec(&view)
    };
    pos_readback.unmap();
    let flags_data: Vec<u32> = {
        let view = flags_readback
            .slice(..)
            .get_mapped_range()
            .expect("staging buffer mapping failed after a completed poll");
        bytes_to_u32_vec(&view)
    };
    flags_readback.unmap();

    // Write the GPU result back into the authoritative CPU walker state.
    for (idx, (w, r)) in walkers.iter_mut().zip(released.iter_mut()).enumerate() {
        let x = pos_data[idx * 4];
        let y = pos_data[idx * 4 + 1];
        let z = pos_data[idx * 4 + 2];
        let t = pos_data[idx * 4 + 3];
        w.position = [
            Length::new::<micrometer>(x as f64),
            Length::new::<micrometer>(y as f64),
            Length::new::<micrometer>(z as f64),
        ];
        w.time = Time::new::<second>(t as f64);
        w.rng.0 = flags_data[idx * 2] as u64;
        *r = flags_data[idx * 2 + 1] != 0;
    }
    Ok(())
}

/// Advance an ensemble one frame using the GPU when available, updating
/// `walkers`/`released` in place. Returns `true` if the GPU handled the frame,
/// `false` if there is no adapter or the submit failed (the caller then advances
/// on the CPU). Never panics on a missing GPU.
pub fn advance_multilayer_best_effort(
    cell: &TrisoCell,
    params: &WalkParams,
    walkers: &mut [WoSWalker],
    released: &mut [bool],
    nuclide: Nuclide,
    until: Time,
) -> bool {
    if let Some(ctx) = cached_context() {
        if try_advance_multilayer_gpu(ctx, cell, params, walkers, released, nuclide, until).is_ok()
        {
            return true;
        }
    }
    false
}

/// Map a readback buffer and block until the copy is visible, or return the
/// appropriate [`GpuError`].
fn map_and_wait(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<(), GpuError> {
    let mapped = Arc::new(Mutex::new(None));
    let cb = Arc::clone(&mapped);
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
        *cb.lock().unwrap() = Some(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| GpuError::Poll(format!("{e:?}")))?;
    let result = mapped.lock().unwrap().take();
    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(GpuError::Map(format!("{e:?}"))),
        None => Err(GpuError::MapCallbackMissing),
    }
}

fn bytes_to_u32_vec(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4);
    for &v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Minimal in-crate `block_on` (no async-runtime dependency): drive a future to
/// completion via a [`std::task::Wake`] thread-park waker — the same pattern as
/// the other workspace GPU modules.
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl std::task::Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
        use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn crp6_inputs() -> (
        Nuclide,
        Length,
        ThermodynamicTemperature,
        Time,
        EnsembleConfig,
    ) {
        (
            Nuclide::Cs137,
            Length::new::<micrometer>(212.5),
            ThermodynamicTemperature::new::<degree_celsius>(1200.0),
            Time::new::<hour>(200.0),
            EnsembleConfig {
                n_histories: 20_000,
                base_seed: 0x0C1A_5EED,
            },
        )
    }

    /// The best-effort path (GPU if present, else CPU fallback) must reproduce
    /// the Crank analytical release fraction. With no adapter this exercises the
    /// CPU fallback; on a GPU host it exercises the `f32` kernel.
    #[test]
    fn best_effort_matches_crank() {
        let (nuclide, radius, temp, time, config) = crp6_inputs();
        let frac = kernel_release_fraction_best_effort(nuclide, radius, temp, time, &config);
        let d = kernel_diffusion_coefficient(nuclide, temp);
        let crank = calculate_analytical_fraction_released(d, radius, time, 200);
        assert!(
            (frac - crank).abs() < 0.02,
            "best-effort {frac:.4} vs Crank {crank:.4}"
        );
    }

    /// The uploaded first-passage-time table is monotonic and spans the expected
    /// dimensionless range (a smoke test of the data the GPU kernel samples).
    #[test]
    fn theta_table_is_monotonic() {
        let table = dimensionless_exit_time_table(THETA_TABLE_LEN);
        assert_eq!(table.len(), THETA_TABLE_LEN);
        for w in table.windows(2) {
            assert!(w[1] >= w[0] - 1e-12, "table must be non-decreasing in F");
        }
        assert!(table[0] >= 0.0);
    }
}
