# REVIEW MANIFEST — njoy-outram-park-fork optional wgpu GPU compute

> **⚠️ UNTRUSTED AI-GENERATED DRAFT — NOT YET HUMAN-REVIEWED ⚠️**
>
> **This change (the `gpu` module and its wiring) was authored by an AI fleet
> (Claude Opus 4.8) and is untrusted draft material until a human reviews it,**
> per the workspace `RESPONSIBLE_USE.md` / `AI_USAGE.md` policy. It compiles and
> its tests pass, but that proves types + one numeric case — not that the design
> is right for the crate. Do not describe the GPU path as validated or trusted
> until the human-verify checklist below is cleared.

Date: 2026-07-17 (Asia/Singapore). Bead: **op-wra**. Base: `origin/develop`
@ `148f48f`. Scope touched: **only** `crates/njoy-outram-park-fork/`.

---

## What was done

Wired **optional GPU compute (`wgpu` 29.0.3)** into `njoy-outram-park-fork` for
an embarrassingly-parallel kernel, with a **mandatory CPU fallback** and a
**pure-CPU Android build** (no `wgpu` on Android at all).

Files changed (all inside the njoy crate):

| File | Change |
|---|---|
| `src/gpu.rs` | **NEW** — the whole GPU module (CPU reference, `probe`, `GpuContext`, WGSL dispatch, blocking executor, tests). |
| `src/lib.rs` | Declared `pub mod gpu;` with a `///` summary of the contract. |
| `Cargo.toml` | Added `wgpu = { workspace = true }` under `[target.'cfg(not(target_os = "android"))'.dependencies]` (target-gated; **root Cargo.toml untouched**). |
| `CLAUDE.md` | New "Dependency posture" section: the "lean, no BLAS" note is now target-qualified — lean on Android, desktop carries `wgpu` behind the target gate. |
| `docs/ai-fleet-review/njoy-wgpu/REVIEW_MANIFEST.md` | This file. |

---

## The kernel chosen (REAL vs STUB — read carefully)

**Kernel: WMP curve-fit polynomial background evaluation across an energy grid.**

- This is a **genuine fragment** of `WindowedMultipole::evaluate` (`src/wmp.rs`,
  the raw non-Doppler-broadened `else`-branch: `sig(E) = c0/E + c1/√E + c2 +
  c3·√E + …` via the running-term recurrence). It is per-energy independent →
  embarrassingly parallel.
- **REAL:** the CPU reference (`curvefit_background_batch_cpu`, f64), the WGSL
  compute shader, the full wgpu dispatch (buffers → bind group → pipeline →
  submit → readback), and a live `probe()` that requests a real headless
  adapter/device.
- **STUB / honest TODO:** the **full** windowed-multipole sum — the complex
  **Faddeeva pole contributions** with per-window pole ranges — is **NOT** on
  the GPU. It is left as an explicit `TODO(op-wra)` in
  `GpuContext::curvefit_background_batch`. This module is **not** a full-fidelity
  WMP GPU evaluator and must not be read as one.

**Why the split is safe:** the CPU path (f64) is the trusted/deterministic
reference used for V&V. The GPU path runs in **f32** (WGSL has no f64 by
default), so its float reduction order will not bit-match the CPU — **GPU is
acceleration only; V&V stays on the CPU.**

---

## Build / test / Android output (measured 2026-07-17)

Commands (release; unit tests run under the crate's ~12 GB `ulimit -v` cap):

```
cargo check -p njoy-outram-park-fork --lib --release                       # clean
cargo check -p njoy-outram-park-fork --lib --target aarch64-linux-android  # clean (pure-CPU, NO wgpu)
cargo test  -p njoy-outram-park-fork --lib --tests --release               # no regression
```

- **Desktop lib check:** clean, no warnings from the new module.
- **Android cross-check (`aarch64-linux-android`):** **clean** — `wgpu` is not
  pulled (confirmed: the Android build's compiled-crate list contains no `wgpu`,
  `naga`, `ash`, etc.), and `gpu::probe()` reduces to the `None` shim. Android
  stays pure-CPU and always compiles.
- **Unit tests:** `377 passed; 0 failed` (375 pre-existing + **2 new** in
  `gpu::tests`). No regression to the baseline.
- Full `--lib --tests` integration-suite result: see the commit message / hand-off
  (integration binaries unaffected — they do not reference `gpu`).

---

## GPU-vs-CPU agreement (NOT a SKIP on this box)

The dev box has a **software Vulkan adapter (Mesa llvmpipe)**, so `probe()`
returned `Some` and `gpu::tests::gpu_agrees_with_cpu_or_skips` **actually ran the
GPU path** (it did not skip): all 1000 f32 GPU values matched the f64 CPU
reference within a **1e-3 relative tolerance**. On a genuinely headless box with
no adapter, `probe()` returns `None` and the test records a **SKIP** (prints a
notice, returns success) — never a failure. Both outcomes are a pass; a missing
adapter is expected, not an error.

> Note: the agreement was obtained on an **llvmpipe software rasterizer**, not
> real GPU silicon. It demonstrates the dispatch + numerics are correct, but does
> not by itself characterise real-hardware behaviour or performance.

---

## Design-rule compliance (self-checked, still needs human review)

- No `Box<T>`, no `dyn` / trait objects, no lifetime parameters. `Arc` used only
  for the `std` `Wake` waker in the blocking executor.
- `///` doc on every public item; `//!` module doc states the 4-point contract
  (target-gated / CPU fallback mandatory / CPU is trusted reference / scaffold
  honesty). V&V test docs carry **methodology + results**.
- No new workspace dependency: `wgpu` version inherited from the root
  `[workspace.dependencies]`; **no `pollster`/`bytemuck`** added (a small
  std-only `block_on` and manual `to_le_bytes`/`from_le_bytes` packing are used).
- File length: `src/gpu.rs` ≈ 566 lines (< 1000-line cap).

---

## Human-verify checklist (before this is trusted)

1. **wgpu 29 API correctness** — the host code was compiler-driven against
   wgpu 29.0.3 (e.g. `Instance::default()`, `PipelineLayoutDescriptor` now uses
   `immediate_size` and `bind_group_layouts: &[Option<&_>]`, `PollType::wait_indefinitely()`).
   Confirm these are used correctly and won't break on a real GPU backend.
2. **`block_on` soundness** — review the stack-pinned `unsafe { Pin::new_unchecked }`
   thread-parking executor for correctness (it drives exactly one future; the
   future is never moved before completion).
3. **f32 tolerance** — confirm `1e-3` relative is the right bound for the intended
   use, and decide whether real-hardware (non-llvmpipe) agreement should be
   characterised separately.
4. **Kernel choice** — confirm the curve-fit background is the right first GPU
   target, and prioritise the Faddeeva pole-sum GPU port (`TODO(op-wra)`) that
   would make this a full WMP evaluator (complex arithmetic in WGSL as `vec2<f32>`).
5. **Buffer-size / correctness edge cases** — empty `coeffs` (padded to one zero),
   `n_energy == 0` (early return), and very large grids (single-submit; no chunking yet).
6. **Whether the GPU path should ever feed anything other than acceleration** —
   currently it must not; V&V stays on CPU. Confirm no downstream consumer wires
   the f32 GPU output into a trusted/validated path.
