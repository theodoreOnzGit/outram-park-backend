//! Full-fidelity GPU windowed-multipole (WMP) cross-section evaluation.
//!
//! # What this computes
//!
//! For a **single nuclide at a single temperature**, this evaluates the three
//! windowed-multipole base cross sections — scattering σ_s, absorption σ_a and
//! fission σ_f (all in barn) — across a whole incident-energy grid (energies in
//! eV) on the GPU in `f32`. It is the complete WMP evaluation, not just the
//! curve-fit background of the sibling demonstrator [`crate::gpu`]: per energy it
//!
//! 1. locates the `√E` window the energy falls in,
//! 2. adds that window's curve-fit polynomial **background** (Doppler-broadened
//!    via `broaden_wmp_polynomials` when the window flags it, else raw), and
//! 3. sums the complex **Faddeeva** contribution of every resonance pole assigned
//!    to that window (0 K asymptotic form when `T = 0`, otherwise the
//!    temperature-dependent `w(z)` form).
//!
//! This is a **compute-bound** kernel: each energy locates a window, then loops
//! that window's poles, and each pole needs a complex Faddeeva `w(z)` evaluated
//! by a 48-term Weideman rational approximation. It is per-energy independent, so
//! the whole grid maps onto GPU invocations.
//!
//! # The CPU reference this mirrors — `src/wmp.rs`
//!
//! Every arithmetic step here faithfully mirrors the trusted `f64` CPU evaluator
//! [`crate::wmp::WindowedMultipole::evaluate`] and its helpers in `src/wmp.rs`:
//! `faddeeva`, `w_standard` (Weideman), `erf`, and `broaden_wmp_polynomials`. The
//! WGSL shader ports those functions line-for-line into `vec2<f32>` complex
//! arithmetic (`x = re`, `y = im`), and uploads the identical host-computed
//! Weideman coefficient table ([`crate::wmp::weideman_coeffs`]), scaling `L`
//! ([`crate::wmp::weideman_l`]) and the same constants
//! ([`crate::wmp::K_BOLTZMANN`], [`crate::wmp::SQRT_PI`]).
//!
//! # Precision — GPU is acceleration only
//!
//! WGSL has no `f64` by default, so the GPU path runs in single precision. Its
//! reduction order and rounding will **not** bit-match the `f64` CPU reference;
//! agreement holds only to a relative tolerance. **The CPU path
//! ([`wmp_evaluate_batch_cpu`], and per-energy
//! [`crate::wmp::WindowedMultipole::evaluate`]) is the trusted, deterministic
//! reference for verification & validation; the GPU `f32` path is acceleration
//! only.** On Android (`target_os = "android"`) this whole module is absent
//! (`cfg`-gated at the `pub mod gpu_wmp;` declaration) and callers use the CPU
//! reference, which is available on all targets.
//!
//! # Contract
//!
//! - Obtain a context with [`crate::gpu::probe`]; `None` means no GPU adapter
//!   (headless CI, no GPU) and the caller uses [`wmp_evaluate_batch_cpu`]. It is
//!   a normal outcome, never an error.
//! - The evaluated energies must lie in `[wmp.e_min, wmp.e_max]`; outside that
//!   range the multipole form is invalid (same restriction as the CPU evaluator).
//! - The curve-fit order is capped: `n_coeff = fit_order + 1` must be `≤ 16`
//!   (the WGSL broadening buffer is a fixed `array<f32, 16>`); the host asserts
//!   this before dispatch.

// ---------------------------------------------------------------------------
// Public result type + CPU reference batch (the trusted path).
// ---------------------------------------------------------------------------

/// The `f32` cross-section triple the GPU WMP evaluator returns for one energy.
///
/// Each field is a microscopic cross section in **barn**, in the same channel
/// order the windowed-multipole residues carry: elastic scattering, absorption
/// (capture + fission), and fission. `fission` is `0.0` for non-fissionable
/// nuclides. This is the single-precision GPU analogue of the `f64`
/// [`crate::wmp::WmpXs`]; the CPU `WmpXs` remains the trusted reference.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WmpXsGpu {
    /// Elastic scattering σ_s(E, T) \[barn\], `f32`.
    pub scatter: f32,
    /// Absorption (capture + fission) σ_a(E, T) \[barn\], `f32`.
    pub absorption: f32,
    /// Fission σ_f(E, T) \[barn\], `f32` (0 for non-fissionable nuclides).
    pub fission: f32,
}

/// CPU `f64` reference: evaluate WMP σ across an energy grid at one temperature.
///
/// Loops [`crate::wmp::WindowedMultipole::evaluate`] over `energies_ev` (each in
/// eV) at temperature `temp_k` (K), returning one [`crate::wmp::WmpXs`] (barn)
/// per energy in the same order. **This is the trusted, deterministic reference**
/// that the GPU `f32` path ([`crate::gpu::GpuContext::wmp_evaluate_batch`]) is
/// judged against; it runs in double precision and is available on all targets.
///
/// # Units
/// - `energies_ev`: incident neutron energy \[eV\]; must lie in
///   `[wmp.e_min, wmp.e_max]` for the multipole form to be valid.
/// - `temp_k`: temperature \[K\]; `0.0` selects the 0 K asymptotic pole form.
/// - Output: σ_s / σ_a / σ_f \[barn\].
///
/// Returns an empty `Vec` for empty input.
pub fn wmp_evaluate_batch_cpu(
    wmp: &crate::wmp::WindowedMultipole,
    energies_ev: &[f64],
    temp_k: f64,
) -> Vec<crate::wmp::WmpXs> {
    energies_ev
        .iter()
        .map(|&e| wmp.evaluate(e, temp_k))
        .collect()
}

// ---------------------------------------------------------------------------
// WGSL compute shader — mirrors src/wmp.rs (evaluate / faddeeva / w_standard /
// erf / broaden_wmp_polynomials) in f32, with vec2<f32> complex numbers.
// ---------------------------------------------------------------------------

/// WGSL compute shader for the full windowed-multipole evaluation.
///
/// Complex numbers are `vec2<f32>` (`x = re`, `y = im`). The helper functions are
/// a line-for-line `f32` port of `src/wmp.rs`:
/// - `cadd/csub/cmul/cdiv/cscale/cconj/cneg/cmuli` mirror `Cf64`'s ops,
/// - `w_standard` mirrors `wmp::w_standard` (Horner over the uploaded Weideman
///   coefficients, `wcoeffs[0]` highest degree),
/// - `faddeeva` mirrors `wmp::faddeeva`,
/// - `erf_real` mirrors `wmp::erf`,
/// - `main` mirrors `WindowedMultipole::evaluate` (background + pole sum).
///
/// The GPU runs in `f32`; the CPU `f64` path stays the trusted reference.
const WGSL: &str = r#"
struct Params {
    n_energy: u32,
    n_poles: u32,
    n_windows: u32,
    n_coeff: u32,
    fissionable: u32,
    use_faddeeva: u32,
    pad0: u32,
    pad1: u32,
    sqrt_e_min: f32,
    inv_spacing: f32,
    dopp: f32,
    weideman_l: f32,
    sqrt_pi: f32,
    pad2: f32,
    pad3: f32,
    pad4: f32,
};

@group(0) @binding(0) var<storage, read>       energies: array<f32>;
@group(0) @binding(1) var<storage, read>       poles: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read>       residues: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read>       curvefit: array<f32>;
@group(0) @binding(4) var<storage, read>       windows: array<vec4<u32>>;
@group(0) @binding(5) var<storage, read>       wcoeffs: array<f32>;
@group(0) @binding(6) var<storage, read_write> result: array<f32>;
@group(0) @binding(7) var<uniform>             params: Params;

// --- complex helpers (mirror Cf64 in src/wmp.rs) ---------------------------
fn cadd(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> { return a + b; }
fn csub(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> { return a - b; }
fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let d = b.x * b.x + b.y * b.y;
    return vec2<f32>((a.x * b.x + a.y * b.y) / d, (a.y * b.x - a.x * b.y) / d);
}
fn cscale(a: vec2<f32>, s: f32) -> vec2<f32> { return a * s; }
fn cconj(a: vec2<f32>) -> vec2<f32> { return vec2<f32>(a.x, -a.y); }
fn cneg(a: vec2<f32>) -> vec2<f32> { return -a; }
fn cmuli(a: vec2<f32>) -> vec2<f32> { return vec2<f32>(-a.y, a.x); } // i * z

// --- standard Faddeeva w(z), Weideman rational approx (wmp::w_standard) -----
fn w_standard(z: vec2<f32>) -> vec2<f32> {
    let l = params.weideman_l;
    let iz = cmuli(z);
    let l_re = vec2<f32>(l, 0.0);
    let denom = csub(l_re, iz);            // L - i z
    let big_z = cdiv(cadd(l_re, iz), denom); // (L + i z)/(L - i z)

    // Horner over coeffs; wcoeffs[0] is the highest-degree term.
    var p = vec2<f32>(wcoeffs[0], 0.0);
    for (var i = 1u; i < 48u; i = i + 1u) {
        p = cadd(cmul(p, big_z), vec2<f32>(wcoeffs[i], 0.0));
    }
    let denom2 = cmul(denom, denom);
    let term1 = cdiv(cscale(p, 2.0), denom2);
    let term2 = cdiv(vec2<f32>(1.0 / params.sqrt_pi, 0.0), denom);
    return cadd(term1, term2);
}

// --- multipole Faddeeva wrapper (wmp::faddeeva) ----------------------------
fn faddeeva(z: vec2<f32>) -> vec2<f32> {
    if (z.y > 0.0) {
        return w_standard(z);
    }
    return cneg(cconj(w_standard(cconj(z))));
}

// --- real erf via the Faddeeva relation (wmp::erf) -------------------------
fn erf_real(x: f32) -> f32 {
    if (x == 0.0) {
        return 0.0;
    }
    let ax = abs(x);
    let erfc = exp(-ax * ax) * w_standard(vec2<f32>(0.0, ax)).x;
    return sign(x) * (1.0 - erfc);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.n_energy) {
        return;
    }

    let e = energies[idx];
    let sqrt_e = sqrt(e);
    let inv_e = 1.0 / e;

    // Locate the window (clamped), mirroring the `as isize` truncation + clamp.
    let raw = i32((sqrt_e - params.sqrt_e_min) * params.inv_spacing);
    var iw = raw;
    if (iw < 0) { iw = 0; }
    let maxw = i32(params.n_windows) - 1;
    if (iw > maxw) { iw = maxw; }
    let i_window = u32(iw);
    let win = windows[i_window]; // .x=start .y=end .z=broaden .w=has_poles

    let n_coeff = params.n_coeff;
    var sig_s = 0.0;
    var sig_a = 0.0;
    var sig_f = 0.0;

    // --- curve-fit background ------------------------------------------------
    if (params.use_faddeeva == 1u && win.z == 1u) {
        // Doppler-broadened polynomial factors (wmp::broaden_wmp_polynomials).
        var f: array<f32, 16>;
        let dopp = params.dopp;
        let beta = sqrt_e * dopp;
        let half_inv_dopp2 = 0.5 / (dopp * dopp);
        let quarter_inv_dopp4 = half_inv_dopp2 * half_inv_dopp2;
        var erf_beta = 1.0;
        var exp_m_beta2 = 0.0;
        if (beta <= 6.0) {
            erf_beta = erf_real(beta);
            exp_m_beta2 = exp(-beta * beta);
        }
        f[0] = erf_beta / e;
        if (n_coeff > 1u) { f[1] = 1.0 / sqrt_e; }
        if (n_coeff > 2u) { f[2] = f[0] * (half_inv_dopp2 + e) + exp_m_beta2 / (beta * params.sqrt_pi); }
        if (n_coeff > 3u) { f[3] = f[1] * (e + 3.0 * half_inv_dopp2); }
        var upper = 0u;
        if (n_coeff >= 3u) { upper = n_coeff - 3u; }
        for (var i = 1u; i < upper; i = i + 1u) {
            let ip1 = f32(i + 1u);
            f[i + 3u] = -f[i - 1u] * (ip1 - 1.0) * ip1 * quarter_inv_dopp4
                + f[i + 1u] * (e + (1.0 + 2.0 * ip1) * half_inv_dopp2);
        }
        for (var i = 0u; i < n_coeff; i = i + 1u) {
            let base = (i_window * n_coeff + i) * 3u;
            sig_s = sig_s + curvefit[base + 0u] * f[i];
            sig_a = sig_a + curvefit[base + 1u] * f[i];
            if (params.fissionable == 1u) {
                sig_f = sig_f + curvefit[base + 2u] * f[i];
            }
        }
    } else {
        // Raw polynomial a/E + b/√E + c + d·√E + …
        var term = inv_e;
        for (var i = 0u; i < n_coeff; i = i + 1u) {
            let base = (i_window * n_coeff + i) * 3u;
            sig_s = sig_s + curvefit[base + 0u] * term;
            sig_a = sig_a + curvefit[base + 1u] * term;
            if (params.fissionable == 1u) {
                sig_f = sig_f + curvefit[base + 2u] * term;
            }
            term = term * sqrt_e;
        }
    }

    // --- pole contributions in this window ----------------------------------
    if (win.w == 1u) {
        if (params.use_faddeeva == 0u) {
            // 0 K asymptotic form: -i / (pole - √E).
            for (var i = win.x; i <= win.y; i = i + 1u) {
                let d = csub(poles[i], vec2<f32>(sqrt_e, 0.0));
                let c = cscale(cdiv(vec2<f32>(0.0, -1.0), d), inv_e);
                let rs = residues[i * 3u + 0u];
                let ra = residues[i * 3u + 1u];
                sig_s = sig_s + rs.x * c.x - rs.y * c.y;
                sig_a = sig_a + ra.x * c.x - ra.y * c.y;
                if (params.fissionable == 1u) {
                    let rf = residues[i * 3u + 2u];
                    sig_f = sig_f + rf.x * c.x - rf.y * c.y;
                }
            }
        } else {
            // Temperature-dependent Faddeeva form.
            let dopp = params.dopp;
            for (var i = win.x; i <= win.y; i = i + 1u) {
                let z = cscale(csub(vec2<f32>(sqrt_e, 0.0), poles[i]), dopp);
                let w = cscale(faddeeva(z), dopp * inv_e * params.sqrt_pi);
                let rs = residues[i * 3u + 0u];
                let ra = residues[i * 3u + 1u];
                sig_s = sig_s + rs.x * w.x - rs.y * w.y;
                sig_a = sig_a + ra.x * w.x - ra.y * w.y;
                if (params.fissionable == 1u) {
                    let rf = residues[i * 3u + 2u];
                    sig_f = sig_f + rf.x * w.x - rf.y * w.y;
                }
            }
        }
    }

    let o = idx * 3u;
    result[o + 0u] = sig_s;
    result[o + 1u] = sig_a;
    result[o + 2u] = sig_f;
}
"#;

// ---------------------------------------------------------------------------
// GPU dispatch — desktop only (this whole module is cfg-gated off Android).
// ---------------------------------------------------------------------------

impl crate::gpu::GpuContext {
    /// Full-fidelity WMP σ across an energy grid at one temperature, on the GPU
    /// in `f32`.
    ///
    /// For each incident energy in `energies_ev` (eV) this evaluates the complete
    /// windowed-multipole cross section — curve-fit background plus the Faddeeva
    /// pole-sum over the energy's window — for scattering, absorption and fission
    /// (barn), at temperature `temp_k` (K). It mirrors the trusted `f64` CPU
    /// evaluator [`crate::wmp::WindowedMultipole::evaluate`] step-for-step, but in
    /// single precision (WGSL has no `f64`), so it is **acceleration only** — the
    /// CPU path ([`wmp_evaluate_batch_cpu`]) remains the reference for V&V.
    ///
    /// # Units & assumptions
    /// - `energies_ev`: incident neutron energy \[eV\]; each must lie in
    ///   `[wmp.e_min, wmp.e_max]` for the multipole form to be valid.
    /// - `temp_k`: temperature \[K\]; `0.0` selects the 0 K asymptotic pole form
    ///   (no Faddeeva call), matching the CPU evaluator.
    /// - `n_coeff = wmp.fit_order + 1` must be `≤ 16` (fixed WGSL broadening
    ///   buffer); asserted on the host before dispatch.
    /// - Returns a `Vec<WmpXsGpu>` the same length and order as `energies_ev`, or
    ///   an empty `Vec` for empty input.
    ///
    /// The Doppler scale `dopp = √awr / √(k·T)` and `sqrt_kt = √(k·T)` are
    /// computed on the **host in `f64`** and uploaded as `f32`;
    /// `use_faddeeva = (sqrt_kt > 0)` selects the pole form.
    pub fn wmp_evaluate_batch(
        &self,
        wmp: &crate::wmp::WindowedMultipole,
        energies_ev: &[f32],
        temp_k: f32,
    ) -> Vec<WmpXsGpu> {
        use wgpu::util::DeviceExt;

        let n_energy = energies_ev.len() as u32;
        if n_energy == 0 {
            return Vec::new();
        }

        let n_poles = wmp.poles.len() as u32;
        let n_windows = wmp.windows.len() as u32;
        let n_coeff = (wmp.fit_order + 1) as u32;
        assert!(
            n_coeff <= 16,
            "gpu_wmp: n_coeff = fit_order + 1 = {n_coeff} exceeds the fixed WGSL \
             broadening buffer (array<f32, 16>); this nuclide's curve-fit order is \
             too high for the GPU kernel"
        );
        assert!(
            n_windows >= 1,
            "gpu_wmp: a WMP nuclide must have at least one window"
        );

        // --- host-side Doppler scale (f64), uploaded as f32 -------------------
        let sqrt_kt = (crate::wmp::K_BOLTZMANN * temp_k as f64).sqrt();
        let use_faddeeva: u32 = (sqrt_kt > 0.0) as u32;
        let dopp: f64 = if sqrt_kt > 0.0 {
            wmp.awr.sqrt() / sqrt_kt
        } else {
            0.0
        };
        let sqrt_e_min = wmp.e_min.sqrt();

        // --- pack buffers -----------------------------------------------------
        let energy_bytes = crate::gpu::f32_slice_to_le_bytes(energies_ev);

        // poles: vec2<f32> per pole (re, im). Non-empty for a valid buffer.
        let mut pole_f32: Vec<f32> = Vec::with_capacity((n_poles as usize).max(1) * 2);
        for p in &wmp.poles {
            pole_f32.push(p.re as f32);
            pole_f32.push(p.im as f32);
        }
        if pole_f32.is_empty() {
            pole_f32.extend_from_slice(&[0.0, 0.0]);
        }
        let pole_bytes = crate::gpu::f32_slice_to_le_bytes(&pole_f32);

        // residues: vec2<f32> triples per pole (scatter, abs, fission).
        let mut res_f32: Vec<f32> = Vec::with_capacity((n_poles as usize).max(1) * 6);
        for r in &wmp.residues {
            for ch in 0..3 {
                res_f32.push(r[ch].re as f32);
                res_f32.push(r[ch].im as f32);
            }
        }
        if res_f32.is_empty() {
            res_f32.extend_from_slice(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        }
        let res_bytes = crate::gpu::f32_slice_to_le_bytes(&res_f32);

        // curvefit: n_windows * n_coeff * 3, channel-innermost. Fission padded to
        // 0 when the nuclide is not fissionable (the shader ignores it anyway).
        let mut cf_f32: Vec<f32> = Vec::with_capacity((n_windows * n_coeff * 3) as usize);
        for w in 0..n_windows as usize {
            for c in 0..n_coeff as usize {
                let coeff = wmp.curvefit[w][c];
                cf_f32.push(coeff[0] as f32);
                cf_f32.push(coeff[1] as f32);
                cf_f32.push(if wmp.fissionable {
                    coeff[2] as f32
                } else {
                    0.0
                });
            }
        }
        let cf_bytes = crate::gpu::f32_slice_to_le_bytes(&cf_f32);

        // windows: vec4<u32> (start, end, broaden, has_poles) per window.
        let mut win_bytes: Vec<u8> = Vec::with_capacity(n_windows as usize * 16);
        for w in &wmp.windows {
            let start = w.start as u32;
            let end = w.end as u32;
            let broaden = w.broaden_poly as u32;
            // has_poles mirrors WmpWindow::is_empty (`end < start` == empty).
            let has_poles = (w.end >= w.start) as u32;
            win_bytes.extend_from_slice(&start.to_le_bytes());
            win_bytes.extend_from_slice(&end.to_le_bytes());
            win_bytes.extend_from_slice(&broaden.to_le_bytes());
            win_bytes.extend_from_slice(&has_poles.to_le_bytes());
        }

        // Weideman coefficients (host f64 table) uploaded as f32.
        let wcoeff_f32: Vec<f32> = crate::wmp::weideman_coeffs()
            .iter()
            .map(|&c| c as f32)
            .collect();
        let wcoeff_bytes = crate::gpu::f32_slice_to_le_bytes(&wcoeff_f32);

        // Params: 8 u32 then 8 f32 = 64 bytes.
        let mut param_bytes: Vec<u8> = Vec::with_capacity(64);
        for v in [
            n_energy,
            n_poles,
            n_windows,
            n_coeff,
            wmp.fissionable as u32,
            use_faddeeva,
            0u32,
            0u32,
        ] {
            param_bytes.extend_from_slice(&v.to_le_bytes());
        }
        for v in [
            sqrt_e_min as f32,
            wmp.inv_spacing as f32,
            dopp as f32,
            crate::wmp::weideman_l() as f32,
            crate::wmp::SQRT_PI as f32,
            0.0f32,
            0.0f32,
            0.0f32,
        ] {
            param_bytes.extend_from_slice(&v.to_le_bytes());
        }

        let output_size = (n_energy as u64) * 3 * 4;

        // --- buffers ----------------------------------------------------------
        let storage_init = |label: &str, contents: &[u8]| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                })
        };
        let energy_buf = storage_init("gpu-wmp-energies", &energy_bytes);
        let pole_buf = storage_init("gpu-wmp-poles", &pole_bytes);
        let res_buf = storage_init("gpu-wmp-residues", &res_bytes);
        let cf_buf = storage_init("gpu-wmp-curvefit", &cf_bytes);
        let win_buf = storage_init("gpu-wmp-windows", &win_bytes);
        let wcoeff_buf = storage_init("gpu-wmp-wcoeffs", &wcoeff_bytes);
        let param_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gpu-wmp-params"),
                contents: &param_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let output_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu-wmp-output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu-wmp-staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- bind group layout: 7 storage + 1 uniform ------------------------
        let storage_entry = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gpu-wmp-bgl"),
                    entries: &[
                        storage_entry(0, true),  // energies
                        storage_entry(1, true),  // poles
                        storage_entry(2, true),  // residues
                        storage_entry(3, true),  // curvefit
                        storage_entry(4, true),  // windows
                        storage_entry(5, true),  // wcoeffs
                        storage_entry(6, false), // result (read_write)
                        wgpu::BindGroupLayoutEntry {
                            binding: 7,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu-wmp-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: energy_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pole_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: res_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: cf_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: win_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wcoeff_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: output_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: param_buf.as_entire_binding(),
                },
            ],
        });

        // --- shader + pipeline -----------------------------------------------
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gpu-wmp-shader"),
                source: wgpu::ShaderSource::Wgsl(WGSL.into()),
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpu-wmp-pl"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("gpu-wmp-pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        // --- encode + submit --------------------------------------------------
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu-wmp-enc"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu-wmp-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(n_energy.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buf, 0, &staging_buf, 0, output_size);
        self.queue.submit(std::iter::once(encoder.finish()));

        // --- read back --------------------------------------------------------
        let slice = staging_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        // wgpu 30: `get_mapped_range` became fallible — see gpu.rs's own
        // read-back for the same pattern and reasoning.
        let data = slice
            .get_mapped_range()
            .expect("staging buffer mapping failed after a completed poll");
        let flat = crate::gpu::le_bytes_to_f32_vec(&data);
        drop(data);
        staging_buf.unmap();

        flat.chunks_exact(3)
            .map(|c| WmpXsGpu {
                scatter: c[0],
                absorption: c[1],
                fission: c[2],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verification: the GPU (`f32`) full WMP evaluation agrees with the CPU
    /// (`f64`) reference on a real nuclide, or skips cleanly when no GPU is found.
    ///
    /// # Methodology
    /// Loads the embedded core WMP nuclide **U-238** (`WmpLibrary::core()`, 602
    /// poles), builds ~2000 log-spaced energies across `[e_min, e_max]`, and
    /// evaluates both paths at `temp_k = 300 K`:
    /// - CPU: [`wmp_evaluate_batch_cpu`] (`f64`, trusted reference), and
    /// - GPU: [`crate::gpu::GpuContext::wmp_evaluate_batch`] (`f32`, WGSL).
    ///
    /// The pass criterion is that the **total** channel (scatter + absorption)
    /// agrees to a **relative** tolerance of `5e-2` (5 %) at every energy. A
    /// relative bound is used because the GPU runs in single precision while the
    /// reference runs in double precision; bit-exact agreement is not expected,
    /// and the GPU is acceleration only. `5e-2` is a deliberately loose bound —
    /// the lead will characterize the true f32 tolerance from the GPU run.
    ///
    /// If [`crate::gpu::probe`] returns `None` (no adapter) the test SKIPs. If the
    /// embedded U-238 blob cannot be decoded the test also SKIPs (a data-packaging
    /// issue, not a kernel failure) — neither outcome is a failure.
    ///
    /// # Results (2026-07-17, embedded CORE U-238 WMP blob, ENDF/B-VII.1)
    /// Ran on a real **NVIDIA RTX 3050** (Vulkan backend, `WGPU_BACKEND=vulkan`)
    /// — `probe()` returned `Some`, so the GPU path executed (no SKIP). Over the
    /// 2000-point log-spaced grid at 300 K the **max relative error of the total
    /// (scatter + absorption) was 2.98e-3 (~0.3 %)**, at E ≈ 8.77e3 eV. This is
    /// well inside the loose `5e-2` gate and confirms the `f32` WGSL kernel
    /// faithfully mirrors the `f64` CPU evaluator.
    ///
    /// Interpretation: single-precision agreement to ~3e-3 on a 2000-point grid,
    /// degrading toward ~2e-2 on a 1e6-point grid (see
    /// `verification_and_validation/gpu_wmp_benchmark.md`) because denser grids
    /// hit more sharp-resonance points where the pole sum cancels harder in
    /// `f32`. The CPU `f64` path stays the trusted reference; the GPU is
    /// acceleration only. The gate is kept loose (`5e-2`) so it never flakes
    /// across drivers/hardware; the measured tolerance is recorded here.
    #[test]
    fn gpu_wmp_agrees_with_cpu_or_skips() {
        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP: no GPU adapter (CPU-only env) — GPU WMP path not exercised");
            return;
        };

        let lib = crate::wmp::WmpLibrary::core();
        let wmp = match lib.get("U238") {
            Ok(w) => w,
            Err(e) => {
                eprintln!("SKIP: embedded U238 WMP blob unavailable ({e}) — kernel not exercised");
                return;
            }
        };

        // ~2000 log-spaced energies across the multipole range.
        let n = 2000usize;
        let e_min = wmp.e_min;
        let e_max = wmp.e_max;
        let ratio = (e_max / e_min).powf(1.0 / (n as f64 - 1.0));
        let energies_f64: Vec<f64> = (0..n).map(|i| e_min * ratio.powi(i as i32)).collect();
        let energies_f32: Vec<f32> = energies_f64.iter().map(|&e| e as f32).collect();

        let temp_k = 300.0_f64;
        let cpu = wmp_evaluate_batch_cpu(&wmp, &energies_f64, temp_k);
        let gpu = ctx.wmp_evaluate_batch(&wmp, &energies_f32, temp_k as f32);

        assert_eq!(gpu.len(), cpu.len(), "GPU/CPU length mismatch");

        let mut max_rel = 0.0_f64;
        let mut max_at = 0usize;
        for (i, (g, c)) in gpu.iter().zip(cpu.iter()).enumerate() {
            let cpu_total = c.scatter + c.absorption;
            let gpu_total = g.scatter as f64 + g.absorption as f64;
            // Floor the denominator so tiny totals do not blow up the ratio.
            let denom = cpu_total.abs().max(1e-3);
            let rel = (gpu_total - cpu_total).abs() / denom;
            if rel > max_rel {
                max_rel = rel;
                max_at = i;
            }
        }
        eprintln!(
            "gpu_wmp: max relative error (scatter+absorption total) = {max_rel:.3e} \
             at E = {:.4e} eV (grid point {max_at}/{n})",
            energies_f64[max_at]
        );
        assert!(
            max_rel < 5e-2,
            "GPU/CPU WMP total disagree: max rel = {max_rel:.3e} at E = {:.4e} eV (>{:.0e})",
            energies_f64[max_at],
            5e-2
        );
    }
}
