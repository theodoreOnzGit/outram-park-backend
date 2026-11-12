// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # The direction-norm guard of the GPU rotation kernel (gh:#1499)
//!
//! ## What is guarded
//!
//! gh:#1497 found that the f64 CPU rotation kernels let a direction's norm
//! error `eps = |u|^2 - 1` grow without bound near the `w` pole: `rotate_angle`
//! multiplies `eps` by about `1 + a^2/b^2` (`a = sqrt(1 - mu^2)`,
//! `b = sqrt(1 - w^2)`). It was fixed on the CPU by
//! `physics::scatter::renormalise_if_drifted` (tolerance `sqrt(f64::EPSILON)`).
//!
//! The fused-event WGSL kernel (`gpu/shaders/batched_event.wgsl`) has one
//! rotation, `rotate_direction`, called by `two_body_with_mu` (elastic) and
//! `continuum_inelastic`; its CPU mirror (`gpu::batched_event`) has the same
//! function. `batched_flight.wgsl` (flight only, the collision is on the f64
//! CPU), `surface_distance.wgsl` and `xs_interp.wgsl` rotate nothing. The
//! direction is never rebuilt within a generation of the fused path, so the
//! exposure is the CPU's, in f32. The fix puts the same guard on the input and
//! the output of `rotate_direction` in both the WGSL and the mirror, and keeps
//! the old body callable as `rotate_direction_unguarded`.
//!
//! ## Prediction and tolerance, written 2026-11-12 BEFORE any run
//!
//! **Tolerance.** `sqrt(f32::EPSILON) = 2^-11.5 = 3.4526698e-4` on
//! `| |u|^2 - 1 |`, the f32 analogue of the CPU's `sqrt(f64::EPSILON)` (half
//! of the 23 stored mantissa bits lost, as the CPU's 2^-26 is half of 52).
//! Fixed from that definition, not from any run. (The brief that scheduled
//! this work quoted 2.44e-4; that is `2^-12 = sqrt(f32::EPSILON / 2)`, the
//! square root of the unit round-off, not of `EPSILON`. The two differ by
//! `sqrt(2)`, which does not matter to the guard's function; the definition
//! the CPU uses is kept.)
//!
//! **P1, unguarded f32 walks leave the tolerance.** 64 walks of 1e5 rotations
//! by `mu = 2 xi - 1` through the LCG. In f32 a fresh direction carries
//! `|eps| ~ 1e-7`; a gain of `1/b^2` lifts that past 3.45e-4 once
//! `b^2 < ~3e-4`, i.e. `|w| > 1 - 1.5e-4`, which an isotropic walk visits with
//! probability ~1.5e-4 per step, ~15 times per walk. Prediction: every one of
//! the 64 unguarded walks leaves the tolerance, on the GPU and on the mirror.
//! Whether they go non-finite is not predicted and not asserted.
//!
//! **P2, guarded walks stay finite and unit.** The same walks through the
//! guarded kernel: every step finite and within the tolerance, GPU and mirror.
//!
//! **P3, how often the guard fires.** Of order 1e-4 per rotation (a decade
//! either side: 1e-5 to 1e-3), from the pole-visit estimate in P1.
//!
//! **P4, bit-identity where the guard does not fire.** At every step the
//! guarded and unguarded outputs (same input, same seed) differ in bits if and
//! only if the input or the unguarded output is outside the tolerance, on the
//! GPU and the mirror. So on healthy directions the kernel is the old kernel.
//!
//! **P5, one step, GPU vs mirror.** For inputs with `|w| <= 0.99`, healthy or
//! drifted by up to 1e-2, the guarded GPU and mirror outputs agree per
//! component within 1e-5: the shader's `sin`/`cos` and PETIR's differ by a few
//! ulp (`4 ulp ~ 4.8e-7`), amplified by `a / b <= 1 / sqrt(1 - 0.99^2) = 7.1`,
//! about 3.4e-6. Near-pole and grossly drifted inputs (including the vector
//! recorded on gh:#1497, `|u| = 2.9`) are judged by finiteness and the
//! tolerance only, since there the gain amplifies ulp differences without
//! bound.
//!
//! **P6, a graphite event walk.** A 100 cm sphere of `C0` (0.0853 /b-cm), 4096
//! neutrons at 2 MeV, run event by event through the mirror until all are dead
//! or 3000 events: after every event every live direction is finite and within
//! the tolerance. The same batch through the GPU generation: every final
//! direction finite and within the tolerance.
//!
//! **P7, existing pins.** `delta_tracking_bit_identity` and
//! `stats_move_fingerprints` run only f64 CPU code that this change does not
//! touch: unchanged. `gpu_fused_event_per_collision_banking` (Godiva, f32):
//! the guard fires only near the pole, ~1e-4 per rotation, on a few rotations
//! per history; its G1-G4 gates still pass, and its printed figures move at
//! most by the handful of histories the guard touches. `gpu_csg_parity` is in
//! `outram-blender`, which this change does not touch: unchanged.
//!
//! ## Results
//!
//! Filled in after the run, below this line.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
