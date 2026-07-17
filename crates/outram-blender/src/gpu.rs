//! GPU compute (optional, behind the `gpu` cargo feature).
//!
//! Headless GPU acceleration via [`wgpu`] for the *embarrassingly parallel*
//! parts of mesh authoring — per-vertex / per-face kernels, subdivision
//! evaluation, deformation. **No window or surface** is created; this is
//! compute-only (WGSL compute shaders).
//!
//! ## Non-negotiable contract for using this module
//!
//! 1. **Feature-gated.** This module only exists under `--features gpu`. The
//!    default build is wgpu-free and headless-Android-clean (per the workspace
//!    Android rule) — a plain `cargo build` never pulls the GPU stack.
//! 2. **Runtime CPU fallback is mandatory.** wgpu *compiles* for Android/CI but
//!    at runtime there may be **no usable GPU adapter** (headless servers, the
//!    Android emulator). Callers MUST treat [`probe`] returning `None` as
//!    "run the CPU path", never as an error.
//! 3. **CPU is the trusted / reference path.** GPU float reduction order will
//!    not bit-match the CPU (`faer` + [`crate::math`]) result, so anything that
//!    feeds V&V or a solver stays CPU-deterministic. GPU is *acceleration only*.
//!
//! Scaffold stage: the adapter request + compute pipeline are `TODO` — [`probe`]
//! currently always returns `None` (i.e. always falls back to CPU), which is the
//! safe default until a real hot kernel is wired. Tracked by bead `op-hzs.10`.

/// Re-export of the GPU backend so callers can build pipelines without adding
/// their own `wgpu` dependency. Only available under `--features gpu`.
pub use wgpu;

/// A live GPU compute context (device + queue). Placeholder — fields will be the
/// `wgpu::Device` / `wgpu::Queue` once a real compute pipeline is wired.
#[derive(Debug, Default)]
pub struct GpuContext {
    _private: (),
}

/// Probe for a usable GPU compute adapter.
///
/// Returns `Some(GpuContext)` when a headless compute device is available, or
/// `None` when the caller must fall back to the CPU path. **Scaffold: always
/// returns `None`** (adapter request is `TODO`, op-hzs.10) — so today every
/// caller correctly runs on CPU.
pub fn probe() -> Option<GpuContext> {
    // TODO(op-hzs.10): create `wgpu::Instance`, request a headless compute
    // adapter + device/queue, and return it. Until then, None ⇒ CPU fallback.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_falls_back_to_cpu_at_scaffold_stage() {
        // The safe default: no GPU wired yet, so callers must use CPU.
        assert!(probe().is_none());
    }
}
