//! Energy-grid cross-section interpolation, CPU reference + GPU compute path.
//!
//! (Module-level prose lives in the parent `crate::gpu` `mod.rs`; the items in
//! this file each carry their own `///` documentation as required by the
//! human-interface-layer rule.)

// UNTRUSTED AI-DRAFT: this code has been drafted with AI assistance and is
// verified by the V&V gate in `mod tests` (CPU analytical checks + a
// GPU-vs-CPU agreement test), but still requires human review before it is
// promoted past the "Unit Tested" V&V stage.

/// Linearly interpolate a tabulated cross section onto a set of query energies.
///
/// This is the **trusted, deterministic f64 reference path** against which the
/// GPU path ([`interp_xs_gpu`]) is judged. It mirrors the energy-grid bracket +
/// linear interpolation performed by OpenMC's `Nuclide::calculate_xs`
/// (`/home/teddy0/Documents/research/openmc/src/nuclide.cpp:716-760`).
///
/// # Physical meaning
/// - `grid`   — monotonically increasing energy grid points, units **eV**.
/// - `sigma`  — the cross section tabulated on `grid`, units **barn**. Must be
///   the same length as `grid` (`sigma[i]` is the cross section at `grid[i]`).
/// - `queries` — the energies (eV) at which the cross section is wanted.
/// - returns — one interpolated cross section (barn) per query, in order.
///
/// # Bracketing / extrapolation (mirrors nuclide.cpp:716-740)
/// For each query energy `q`, with `n = grid.len()`:
/// - `q < grid[0]`         → `i_grid = 0`      (clamp: use the first interval,
///   `f` becomes negative so this linearly extrapolates below the grid — this
///   matches OpenMC, where the total XS grid always spans the problem range so
///   this branch is effectively a clamp to the first tabulated interval).
/// - `q > grid[n-1]`       → `i_grid = n-2`    (use the last interval; `f > 1`
///   linearly extrapolates above the grid, again mirroring OpenMC).
/// - otherwise             → `i_grid` such that `grid[i_grid] <= q < grid[i_grid+1]`
///   (lower-bound search).
///
/// A rare duplicate-energy guard (`grid[i] == grid[i+1]`) advances one index,
/// exactly as `nuclide.cpp:735`. The interpolation factor is
/// `f = (q - grid[i]) / (grid[i+1] - grid[i])` and the result is
/// `(1 - f) * sigma[i] + f * sigma[i+1]`.
///
/// # Degenerate grids
/// - `n == 0` → every query returns `0.0`.
/// - `n == 1` → every query returns `sigma[0]` (nothing to interpolate).
///
/// # Preconditions
/// `grid` must be sorted ascending and `grid.len() == sigma.len()`. Violating
/// these is not checked (hot path); results are then unspecified.
pub fn interp_xs_cpu(grid: &[f64], sigma: &[f64], queries: &[f64]) -> Vec<f64> {
    let n = grid.len();

    // Degenerate grids: no interpolation possible.
    if n == 0 {
        return vec![0.0; queries.len()];
    }
    if n == 1 {
        return vec![sigma[0]; queries.len()];
    }

    let mut out = Vec::with_capacity(queries.len());
    for &q in queries {
        // Bracket: pick the lower index i_grid of the containing interval.
        let mut i_grid: usize = if q < grid[0] {
            0
        } else if q > grid[n - 1] {
            n - 2
        } else {
            // lower_bound: largest i with grid[i] <= q, clamped to [0, n-2].
            lower_bound_index(grid, q)
        };

        // Rare case: two identical adjacent energies (nuclide.cpp:735).
        if grid[i_grid] == grid[i_grid + 1] {
            i_grid += 1;
            // If that pushed us off the end, fall back one interval so the
            // indices below stay in range (still degenerate, f handled below).
            if i_grid >= n - 1 {
                i_grid = n - 2;
            }
        }

        let lo = grid[i_grid];
        let hi = grid[i_grid + 1];
        let d = hi - lo;
        // Guard identical adjacent energies -> zero interpolation factor.
        let f = if d == 0.0 { 0.0 } else { (q - lo) / d };

        out.push((1.0 - f) * sigma[i_grid] + f * sigma[i_grid + 1]);
    }
    out
}

/// Largest index `i` in `0..=n-2` with `grid[i] <= q < grid[i+1]`, assuming
/// `grid[0] <= q <= grid[n-1]` (the in-range branch of [`interp_xs_cpu`]).
///
/// Returns a value in `0..=n-2` so that `grid[i]` and `grid[i+1]` are both
/// valid. Mirrors OpenMC's `lower_bound_index` usage in `nuclide.cpp:730`.
#[inline]
fn lower_bound_index(grid: &[f64], q: f64) -> usize {
    // Standard binary search for the interval. grid is sorted ascending.
    // partition_point gives the count of elements <= q; subtract 1 for the
    // lower index, then clamp into [0, n-2].
    let n = grid.len();
    let count_le = grid.partition_point(|&g| g <= q);
    // count_le >= 1 because q >= grid[0] in this branch.
    let i = count_le.saturating_sub(1);
    i.min(n - 2)
}

// ---------------------------------------------------------------------------
// GPU compute path (non-Android only; wgpu is not a dependency on Android).
// ---------------------------------------------------------------------------

/// Reinterpret an `&[f32]` slice as native-endian bytes for buffer upload.
///
/// `bytemuck` is intentionally not a dependency of this crate, so byte views
/// are built by hand. Native-endian is correct here because both the CPU
/// upload and the GPU (same machine) agree on byte order.
#[cfg(not(target_os = "android"))]
fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for &x in data {
        bytes.extend_from_slice(&x.to_ne_bytes());
    }
    bytes
}

/// Reinterpret a `[u32; 4]` params block as native-endian bytes for upload.
#[cfg(not(target_os = "android"))]
fn u32x4_to_bytes(data: &[u32; 4]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    for &x in data {
        bytes.extend_from_slice(&x.to_ne_bytes());
    }
    bytes
}

/// Rebuild a `Vec<f32>` of length `n` from a native-endian byte view.
#[cfg(not(target_os = "android"))]
fn bytes_to_f32_vec(bytes: &[u8], n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(4).take(n) {
        out.push(f32::from_ne_bytes(chunk.try_into().unwrap()));
    }
    out
}

/// Linearly interpolate a tabulated cross section onto query energies **on the
/// GPU**, via a WGSL compute shader. This is the f32 accelerated counterpart of
/// the f64 reference [`interp_xs_cpu`]; the two are held to agreement by the
/// V&V test in this module.
///
/// # Physical meaning (identical to [`interp_xs_cpu`], but single precision)
/// - `grid`    — ascending energy grid points, units **eV** (f32).
/// - `sigma`   — cross section tabulated on `grid`, units **barn** (f32).
/// - `queries` — energies (eV) at which the cross section is wanted (f32).
/// - returns   — one interpolated cross section (barn) per query, in order.
///
/// # Preconditions
/// - `grid.len() == sigma.len()`.
/// - `grid.len() >= 2` (the shader indexes `i_grid + 1`; a degenerate 0/1-point
///   grid is not supported on the GPU path — use [`interp_xs_cpu`] for those).
/// - `grid` sorted ascending.
///
/// An empty `queries` returns an empty `Vec` without touching the GPU.
///
/// # How it works
/// Uploads `grid`, `sigma`, `queries` as read-only storage buffers, a
/// `[n_grid, n_query, 0, 0]` uniform params block, dispatches
/// `ceil(n_query / 64)` workgroups of the `main` entry point, copies the result
/// storage buffer into a mappable staging buffer, blocks on
/// `device.poll(PollType::wait_indefinitely())`, and reads the result back.
#[cfg(not(target_os = "android"))]
pub fn interp_xs_gpu(
    ctx: &crate::gpu::GpuContext,
    grid: &[f32],
    sigma: &[f32],
    queries: &[f32],
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let n_query = queries.len();
    if n_query == 0 {
        return Vec::new();
    }

    let device = &ctx.device;
    let queue = &ctx.queue;

    let n_grid = grid.len();

    // --- Upload input buffers -------------------------------------------------
    let grid_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("xs_interp.grid"),
        contents: &f32_slice_to_bytes(grid),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let sigma_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("xs_interp.sigma"),
        contents: &f32_slice_to_bytes(sigma),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let query_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("xs_interp.queries"),
        contents: &f32_slice_to_bytes(queries),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let params: [u32; 4] = [n_grid as u32, n_query as u32, 0, 0];
    let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("xs_interp.params"),
        contents: &u32x4_to_bytes(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    // Result storage buffer (write from shader, copy to staging afterwards).
    let result_size = (n_query * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
    let result_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("xs_interp.result"),
        size: result_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("xs_interp.staging"),
        size: result_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // --- Pipeline -------------------------------------------------------------
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("xs_interp.shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/xs_interp.wgsl").into()),
    });

    let storage_ro = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("xs_interp.bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: storage_ro,
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: storage_ro,
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: storage_ro,
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("xs_interp.pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("xs_interp.pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("xs_interp.bg"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: grid_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: sigma_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: query_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: result_buf.as_entire_binding(),
            },
        ],
    });

    // --- Dispatch -------------------------------------------------------------
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("xs_interp.encoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("xs_interp.pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let wg = (n_query as u32).div_ceil(64);
        cpass.dispatch_workgroups(wg, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&result_buf, 0, &staging_buf, 0, result_size);
    queue.submit(Some(encoder.finish()));

    // --- Read back ------------------------------------------------------------
    let slice = staging_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |res| {
        res.unwrap();
    });
    // Block until the GPU has finished and the map callback has fired.
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let view = slice
        .get_mapped_range()
        .expect("staging buffer mapping failed after a completed poll");
    let out = bytes_to_f32_vec(&view, n_query);
    drop(view);
    staging_buf.unmap();

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (CPU reference, analytical): linear interpolation on a uniform grid.
    ///
    /// **Methodology.** Build a uniform grid `E = [0, 1, 2, 3, 4]` eV and a
    /// *linear* cross section `sigma(E) = 2 E + 1` barn tabulated on it. For a
    /// linear function, exact linear interpolation must reproduce `sigma(E)`
    /// with zero error at every query — grid points, interval midpoints, and
    /// (because the extrapolation is also linear) points outside the grid. This
    /// is a closed-form reference: pass criterion is exact agreement to `1e-12`.
    ///
    /// **Results (2026-07-17).** All queries agree with `2 E + 1` to `< 1e-12`:
    /// grid points, midpoints, an out-of-range-below query `E = -0.5`
    /// (extrapolates to `2*(-0.5)+1 = 0.0`), and out-of-range-above `E = 5.5`
    /// (extrapolates to `2*5.5+1 = 12.0`). This confirms the bracketing and the
    /// `(1-f)sigma_i + f sigma_{i+1}` blend match the analytic line, including
    /// the documented linear-extrapolation clamp behaviour.
    #[test]
    fn cpu_linear_uniform_grid_exact() {
        let grid = [0.0, 1.0, 2.0, 3.0, 4.0];
        let f = |e: f64| 2.0 * e + 1.0;
        let sigma: Vec<f64> = grid.iter().map(|&e| f(e)).collect();

        let queries = [
            0.0, 1.0, 2.0, 3.0, 4.0, // grid points
            0.5, 1.5, 2.5, 3.5,  // midpoints
            -0.5, // out of range below -> extrapolate
            5.5,  // out of range above -> extrapolate
        ];
        let out = interp_xs_cpu(&grid, &sigma, &queries);

        for (&q, &got) in queries.iter().zip(out.iter()) {
            let want = f(q);
            assert!((got - want).abs() < 1e-12, "q={q}: got {got}, want {want}");
        }
        // Spell out the two documented out-of-range clamps explicitly.
        assert!((out[9] - 0.0).abs() < 1e-12, "below-range extrapolation");
        assert!((out[10] - 12.0).abs() < 1e-12, "above-range extrapolation");
    }

    /// V&V (CPU reference, analytical): non-uniform grid, hand-computed value.
    ///
    /// **Methodology.** A monotonic non-uniform energy grid
    /// `E = [1, 10, 100, 1000]` eV with `sigma = [5, 8, 20, 3]` barn. Query
    /// `E = 55` eV falls in the interval `[10, 100]`. The exact interpolation
    /// factor is `f = (55 - 10)/(100 - 10) = 45/90 = 0.5`, so
    /// `sigma = (1-0.5)*8 + 0.5*20 = 14` barn. Pass criterion: match this
    /// hand-computed `14.0` barn to `1e-12`. A second query `E = 1` (exactly a
    /// grid point) must return `sigma[0] = 5` barn.
    ///
    /// **Results (2026-07-17).** `interp_xs_cpu` returns `14.0` barn at 55 eV
    /// and `5.0` barn at 1 eV, both within `1e-12`, confirming correct interval
    /// selection and the interpolation factor on a non-uniform grid.
    #[test]
    fn cpu_nonuniform_grid_handcomputed() {
        let grid = [1.0, 10.0, 100.0, 1000.0];
        let sigma = [5.0, 8.0, 20.0, 3.0];

        let out = interp_xs_cpu(&grid, &sigma, &[55.0, 1.0]);
        assert!((out[0] - 14.0).abs() < 1e-12, "got {}", out[0]);
        assert!((out[1] - 5.0).abs() < 1e-12, "got {}", out[1]);
    }

    /// V&V (CPU reference): degenerate grids return documented fallbacks.
    #[test]
    fn cpu_degenerate_grids() {
        // n == 0 -> zeros.
        let out0 = interp_xs_cpu(&[], &[], &[1.0, 2.0]);
        assert_eq!(out0, vec![0.0, 0.0]);
        // n == 1 -> constant sigma[0].
        let out1 = interp_xs_cpu(&[3.0], &[7.0], &[1.0, 5.0]);
        assert_eq!(out1, vec![7.0, 7.0]);
    }

    /// V&V GATE (GPU vs CPU agreement): the f32 GPU path must reproduce the
    /// trusted f64 CPU reference within single-precision tolerance.
    ///
    /// **Methodology.** A realistic 256-point log-spaced energy grid from
    /// `1e-3` to `2e7` eV carries a smooth synthetic cross section
    /// `sigma(E) = 100/sqrt(E) + 5*exp(-((log10 E - 3)^2))` barn (a `1/sqrt(E)`
    /// slowing-down trend plus a resonance-like bump near 1 keV). ~4096 query
    /// energies span the grid (log-spaced, with a couple of exact-grid-point
    /// queries). The reference is [`interp_xs_cpu`] evaluated in f64; the GPU
    /// path [`interp_xs_gpu`] is evaluated on the same inputs cast to f32.
    ///
    /// **Pass criterion.** For every query,
    /// `|gpu - cpu| <= 1e-4 * (1 + |cpu|)` — a mixed abs/rel tolerance sized for
    /// f32 accumulation of the interpolation blend over cross sections up to
    /// ~O(1e3) barn. No expected numbers are hard-coded; the GPU is judged
    /// solely against the CPU reference computed at test time.
    ///
    /// **What agreement demonstrates.** That the WGSL bracket + linear-blend
    /// kernel implements the same interpolation as the OpenMC-mirrored CPU
    /// reference, so the GPU can be used as an accelerator without changing
    /// physics results beyond f32 rounding.
    ///
    /// **Results.** Recorded by CI/developer run: on a machine with a GPU
    /// adapter this passes for all ~4096 queries; on CPU-only CI (no adapter)
    /// it prints a SKIP line and returns green (the GPU path is untested there,
    /// which the CPU analytical tests above still cover for the algorithm).
    #[cfg(not(target_os = "android"))]
    #[test]
    fn gpu_matches_cpu_reference() {
        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP interp_xs gpu test: no GPU adapter (CPU-only CI)");
            return;
        };

        // 256 log-spaced grid points from 1e-3 to 2e7 eV.
        let n_grid = 256usize;
        let e_lo = 1e-3f64;
        let e_hi = 2e7f64;
        let log_lo = e_lo.log10();
        let log_hi = e_hi.log10();
        let grid_f64: Vec<f64> = (0..n_grid)
            .map(|i| {
                let t = i as f64 / (n_grid - 1) as f64;
                10f64.powf(log_lo + t * (log_hi - log_lo))
            })
            .collect();

        // Smooth synthetic sigma(E): slowing-down trend + a bump near 1 keV.
        let sigma_of = |e: f64| {
            let l = e.log10();
            100.0 / e.sqrt() + 5.0 * (-(l - 3.0).powi(2)).exp()
        };
        let sigma_f64: Vec<f64> = grid_f64.iter().map(|&e| sigma_of(e)).collect();

        // ~4096 log-spaced query energies spanning the grid, plus a few exact
        // grid-point queries to exercise the f == 0 path.
        let n_query = 4096usize;
        let mut queries_f64: Vec<f64> = (0..n_query)
            .map(|i| {
                let t = i as f64 / (n_query - 1) as f64;
                10f64.powf(log_lo + t * (log_hi - log_lo))
            })
            .collect();
        queries_f64.push(grid_f64[0]);
        queries_f64.push(grid_f64[n_grid / 2]);
        queries_f64.push(grid_f64[n_grid - 1]);

        // Reference in f64.
        let cpu = interp_xs_cpu(&grid_f64, &sigma_f64, &queries_f64);

        // GPU in f32 on the same inputs.
        let grid_f32: Vec<f32> = grid_f64.iter().map(|&x| x as f32).collect();
        let sigma_f32: Vec<f32> = sigma_f64.iter().map(|&x| x as f32).collect();
        let queries_f32: Vec<f32> = queries_f64.iter().map(|&x| x as f32).collect();
        let gpu = interp_xs_gpu(&ctx, &grid_f32, &sigma_f32, &queries_f32);

        assert_eq!(gpu.len(), cpu.len(), "GPU returned wrong length");

        for (i, (&g, &c)) in gpu.iter().zip(cpu.iter()).enumerate() {
            let tol = 1e-4 * (1.0 + c.abs() as f32);
            assert!(
                (g - c as f32).abs() <= tol,
                "query {i} (E={}): gpu={g}, cpu={c}, |diff|={} > tol={tol}",
                queries_f64[i],
                (g - c as f32).abs()
            );
        }
    }
}
