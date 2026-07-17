# AI Fleet Review Manifest — outram-mc-libs optional wgpu GPU compute

> **⚠️ UNTRUSTED AI-DRAFTED CODE — NOT YET HUMAN-REVIEWED.**
> **This work was produced by an AI agent fleet (Claude Opus lead + two Opus
> subagents). Per `RESPONSIBLE_USE.md` / `AI_USAGE.md`, it is untrusted draft
> material at the "Unit Tested" V&V stage until a human maintainer reviews the
> code, the license/provenance, and the V&V results. Do not describe it as
> validated or trusted, and do not use it for anything safety-, licensing-, or
> operations-relevant.**

- **Bead:** `op-u6s.3` (child of `op-u6s` — openmc-libs Monte Carlo transport epic).
- **Date:** 2026-07-17.
- **Scope touched:** `crates/outram-mc-libs/` only. No root `Cargo.toml` edit, no
  other crate touched (njoy + outram-blender fleets ran concurrently).
- **wgpu version:** 29.0.3 (inherited from the root `[workspace.dependencies]`,
  matched to the eframe/egui 0.34 stack). Declared here only under
  `[target.'cfg(not(target_os = "android"))'.dependencies]`.

## Kernel chosen

**Batched pointwise cross-section interpolation on a shared energy grid** — the
inner cross-section lookup of a Monte Carlo neutron transport code. Given an
ascending energy grid `E[0..G]` (eV), a tabulated microscopic cross section
`sigma[0..G]` (barn), and a batch of `N` query energies, produce `N`
linearly-interpolated cross-section values. Each query is fully independent, so
the kernel is embarrassingly parallel (one GPU invocation per query energy),
branch-light (a binary search + a linear blend), and has no history state — a
textbook GPU-friendly MC sub-kernel.

**This is a genuine physics kernel, not a toy.** It is ported from OpenMC's C++
`Nuclide::calculate_xs` (`/home/teddy0/Documents/research/openmc/src/nuclide.cpp`,
lines ~716-760): the below-grid / above-grid clamp, the lower-bound binary
search for the bracket `grid[i_grid] <= q < grid[i_grid+1]`, the interpolation
factor `f = (E - grid[i_grid]) / (grid[i_grid+1] - grid[i_grid])`, and the blend
`sigma = (1-f)*sigma[i_grid] + f*sigma[i_grid+1]`. OpenMC additionally narrows
the binary-search range with a logarithmic union-grid pointer table; that is a
performance optimization over the *same* bracket, so the plain full-grid binary
search here yields the identical `i_grid`.

## Real vs. stub

| Part | Status |
|---|---|
| `src/gpu/mod.rs` — `GpuContext`, `probe()`, pure-std `block_on`, Android shim | **REAL** — probe requests a real headless Vulkan/Metal/DX12 adapter + device; no stub. |
| `src/gpu/shaders/xs_interp.wgsl` — WGSL compute shader | **REAL** — full bracket + binary-search + linear-blend kernel, `@workgroup_size(64)`. |
| `src/gpu/xs_interp.rs::interp_xs_cpu` — f64 CPU reference | **REAL** — trusted deterministic reference; OpenMC-mirrored bracket + interp. |
| `src/gpu/xs_interp.rs::interp_xs_gpu` — f32 GPU dispatch | **REAL** — uploads buffers, builds pipeline from the WGSL, dispatches, reads back. |

No `todo!()`, no fake-green, no hard-coded "expected" GPU numbers. The GPU output
is judged solely against the CPU reference computed at test time.

## Design contract honored

- **Compiles always + graceful CPU fallback.** `wgpu` is target-gated OFF Android;
  the `gpu` module compiles on every target (the CPU reference is pure `f64`/`f32`
  Rust). On Android, `probe()` is a shim returning `None`; on desktop/CI it returns
  `None` when no adapter exists. Callers treat `None` as "run CPU", never an error.
- **CPU is the trusted, deterministic reference.** The transport loop stays raw
  `f64` (crate `CLAUDE.md`). GPU runs `f32`; it will not bit-match, so V&V stays on
  CPU and GPU is compared only within a tolerance. GPU is acceleration only.
- **No new third-party dependency.** No `pollster`/async runtime added — `mod.rs`
  hand-rolls a ~15-line pure-`std` `Wake`-based `block_on` for the two wgpu setup
  futures; buffer read-back uses `Device::poll(PollType::wait_indefinitely())`.
- **House rules:** enums not trait objects; no `Box<T>`; no lifetime params; every
  public item has a `///` doc comment; the module has a `//!` map. (The pre-existing
  `Vec<Box<dyn Filter>>` tally deviation was left untouched; no new `dyn`/`Box`
  introduced.)

## Build / test / android output (measured 2026-07-17)

```
# release lib test suite (desktop)
cargo test -p outram-mc-libs --lib --release
  test result: ok. 76 passed; 0 failed; 0 ignored   (72 baseline + 4 new:
    gpu::xs_interp::tests::cpu_linear_uniform_grid_exact          ok
    gpu::xs_interp::tests::cpu_nonuniform_grid_handcomputed       ok
    gpu::xs_interp::tests::cpu_degenerate_grids                   ok
    gpu::xs_interp::tests::gpu_matches_cpu_reference              ok)

# Android cross-check (CPU-only; wgpu target-gated out)
cargo check -p outram-mc-libs --lib --tests --target aarch64-linux-android
  Finished — clean. No wgpu/naga in the Android dependency graph; the GPU
  function and GPU test are #[cfg(not(target_os = "android"))]-gated out.
```

No regression: the 72 pre-existing lib tests still pass; the 4 new tests are additive.

## GPU-vs-CPU agreement (actually executed, not skipped)

The `gpu_matches_cpu_reference` V&V gate **ran on real hardware** in this session
(it did not hit the CPU-only SKIP path):

- **Adapter:** `NVIDIA GeForce RTX 3050`, backend **Vulkan**, `DiscreteGpu`.
- **Case:** 256-point log-spaced energy grid `1e-3 … 2e7` eV; smooth synthetic
  `sigma(E) = 100/sqrt(E) + 5*exp(-((log10 E - 3)^2))` barn; ~4099 query energies
  spanning the grid (incl. exact grid-point queries).
- **Reference:** `interp_xs_cpu` in f64. **GPU:** `interp_xs_gpu` in f32 on the same
  inputs cast to f32.
- **Pass criterion:** `|gpu - cpu| <= 1e-4 * (1 + |cpu|)` for every query.
- **Result:** PASS for all ~4099 queries — the f32 WGSL kernel reproduces the
  OpenMC-mirrored f64 CPU reference within single-precision tolerance.

Note for CI: on a machine with **no** GPU adapter, `probe()` returns `None`, the
test prints a `SKIP` line and returns green (never fails). The three CPU analytical
tests still cover the algorithm there.

## Human-verify list (before promoting past "Unit Tested")

1. **OpenMC provenance.** Diff `interp_xs_cpu` + the WGSL against
   `openmc/src/nuclide.cpp:716-760`; confirm the bracket/clamp/interp-factor logic
   matches and that omitting the log union-grid pointer is acceptable (equivalent
   `i_grid`, only a search-speed difference).
2. **Out-of-range semantics.** The below/above-grid branches *linearly extrapolate*
   (matching OpenMC's `i_grid = 0` / `n-2`), they do not hard-clamp to the endpoint
   value. Confirm this is the intended behavior for how this kernel would be wired
   into real transport (in practice the total-XS grid spans the problem range).
3. **f32 tolerance.** Confirm `1e-4*(1+|cpu|)` is appropriate for the intended σ
   magnitudes; large resonance cross sections (10^3–10^5 barn) may want revisiting
   if this kernel is fed real resonance data rather than the smooth synthetic σ.
4. **wgpu 29 API.** Two spec facts were corrected against the vendored source:
   `PipelineLayoutDescriptor` uses `immediate_size: u32` (not `push_constant_ranges`);
   `bind_group_layouts` is `&[Option<&BindGroupLayout>]`. Sanity-check on a wgpu
   upgrade.
5. **`block_on`.** Review the hand-rolled pure-std executor in `mod.rs` (used only
   for `request_adapter`/`request_device`).
6. **Next step (not done here).** This kernel operates on a *pre-tabulated* σ(E)
   grid; the crate's live data path is WMP/MGXS/pointwise from `njoy`. Wiring this
   GPU batch lookup into an actual transport sweep (or feeding it a real unionized
   σ(E) table) is follow-up work — see the follow-up beads.

## Files added/changed

- `crates/outram-mc-libs/Cargo.toml` — target-gated `wgpu` dependency (desktop only).
- `crates/outram-mc-libs/src/lib.rs` — `pub mod gpu;`.
- `crates/outram-mc-libs/src/gpu/mod.rs` — GpuContext, probe, block_on, Android shim (lead).
- `crates/outram-mc-libs/src/gpu/xs_interp.rs` — CPU ref + GPU dispatch + tests (subagent).
- `crates/outram-mc-libs/src/gpu/shaders/xs_interp.wgsl` — WGSL compute kernel (subagent).
- `crates/outram-mc-libs/src/prelude.rs` — re-export `GpuContext`, `gpu_probe`, `interp_xs_cpu`, `interp_xs_gpu`.
- `crates/outram-mc-libs/docs/ai-fleet-review/outram-mc-wgpu/REVIEW_MANIFEST.md` — this file.
