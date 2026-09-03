// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! **The cross-cutting parity gate for every hybrid-backend kernel in this
//! crate** — bead `op-yvj.4.7`.
//!
//! Each of the five kernel modules landed under the `op-yvj.4` epic asserts its
//! own parity internally. This file is the *uniform* gate: one harness that
//! drives every kernel through the same three backends and the same comparison
//! rules, so that
//!
//! - the per-module claims can be checked **together** rather than one module at
//!   a time, and
//! - a **new** kernel cannot land without meeting the same bar — adding it to
//!   [`run_all_kernels`] is the only work required, and it is then covered by
//!   every gate below by construction.
//!
//! It is an **integration** test rather than a `#[cfg(test)]` module, so it sees
//! exactly the public surface an external crate sees. That vantage is the point:
//! it cannot reach the `pub(crate)` `*_min` entry points the in-module tests use
//! to bypass the size floors, so what it measures is what a real caller gets.
//!
//! # The five modules covered
//!
//! | Module | Kernels exercised here |
//! |---|---|
//! | `ldu_matrix::parallel` | `HybridLdu::spmv`, `residual`, `normalised_residual`, `diagonal_reciprocal`; `dot`, `axpy`, `norm_l1`, `norm_l2` |
//! | `fields::parallel` | `add`, `sub`, `scale`, `axpy`, `axpy_assign`, `pointwise_mul`, `pointwise_div`, `scale_by_field`, `dot_field`, `min`, `max`, `sum`, `mean`, `l2_norm`, `dot`, `add_vol`, `scale_vol`, `add_surface`, `vol_integral`, `vol_average`, `vol_l2_norm`, `vol_min`, `vol_max` |
//! | `math::parallel` | `solve_bracketed_batch` (Brent, Bisection), `solve_newton_batch`, `linear_roots_batch`, `quadratic_roots_batch`, `cubic_roots_batch` |
//! | `math::minimise` | `golden_section_batch` (both [`Sense`] variants) |
//! | `ode::parallel` | `integrate_ensemble` (Rkf45, Euler), `integrate_ensemble_mixed` (Rkf45 + Rosenbrock23), `quadrature_batch` (Gauss-Legendre, Simpson, Trapezoid), `adaptive_quadrature_batch` |
//!
//! # The four gates, and what each proves
//!
//! 1. [`gate_parity_serial_vs_cpu_multi`] — every kernel run on
//!    [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`] on identical
//!    input. Kernels that claim bitwise identity are compared with
//!    [`f64::to_bits`], **not** a tolerance, so a regression from *bitwise* to
//!    *merely close* fails. Kernels that legitimately re-associate a sum are
//!    compared against the tolerance their own module documents, and the class
//!    is recorded per kernel in [`ParityClass`] so the two can never be confused.
//! 2. [`gate_parity_serial_vs_gpu`] — the same comparison for
//!    [`ComputeBackend::Gpu`].
//! 3. [`gate_gpu_degrades_and_does_not_lie`] — asserts that no module's
//!    **auto-select** policy ever reports `Gpu`.
//!
//!    Read this one carefully, because its meaning changed on 2026-09-03. It
//!    used to hold "because no GPU kernel exists in any of them". Real WGSL
//!    kernels now do exist for `ldu_matrix::parallel` (`spmv`, `axpy`,
//!    `scale`, `dot`, `norm_l1` — see that module's `gpu` submodule). What
//!    this gate now asserts is the *deliberate policy* that those kernels are
//!    **opt-in only**: they run when a caller names `ComputeBackend::Gpu`
//!    explicitly, and never because auto-select chose it for them. The reason
//!    is measured, not theoretical — see
//!    [`gate_gpu_kernels_match_the_oracle`] and
//!    `examples/hybrid_gpu_report.rs`.
//! 3b. [`gate_gpu_kernels_match_the_oracle`] — the parity gate for the kernels
//!    that *do* exist, run against the serial `f64` oracle at the tolerance
//!    each kernel's doc comment states. Skips with a printed note when no
//!    adapter is present, which is a valid outcome, not a pass by omission.
//! 4. [`gate_thread_count_invariance`] — the same input at 1, 2, 4 and 8 rayon
//!    workers must give byte-identical output. An integration test cannot build
//!    a rayon pool (rayon is an optional dependency of the *library*, not a
//!    dev-dependency), so this re-executes the test binary as a child process
//!    with `RAYON_NUM_THREADS` set, and compares per-kernel digests written by
//!    the child to a file.
//!
//! Two supporting gates: [`gate_dispatch_floors_are_enforced`] pins every
//! module's measured size floor as seen from outside the crate, and
//! [`machine_load_report`] records the load conditions under which any timing on
//! this machine would have been taken.
//!
//! # Machine load is recorded with every measurement
//!
//! Every crossover figure previously filed on `op-yvj.4.7` was measured without
//! controlling or reporting machine load, and one agent's runs showed batched
//! quadrature at `0.98-1.02x` under contention where an idle machine gives
//! `3.1-4.0x` — the speed-up erased entirely. A threshold tuned on an idle box
//! can therefore lose its own benefit on a busy one. [`MachineLoad`] reads
//! `/proc/loadavg` and [`std::thread::available_parallelism`] and is printed
//! beside **every** timing this file emits, together with an explicit
//! [`LoadVerdict`] that says in words whether the number is trustworthy. A
//! benchmark run under `LoadVerdict::Contended` prints its verdict rather than
//! quietly emitting an authoritative-looking figure.
//!
//! # Both feature settings are meaningful
//!
//! With `parallel` **off** — the default — [`ComputeBackend::CpuMulti`] resolves
//! to [`ComputeBackend::Serial`], so every parity assertion here still runs and
//! still passes, exercising one code path twice. Nothing is `#[cfg]`-ed out and
//! nothing silently skips; the gates are *trivially* satisfied rather than
//! absent, which is the honest behaviour for a default build. The assertions
//! that would otherwise become vacuous —
//! [`gate_dispatch_floors_are_enforced`] and
//! [`gate_gpu_degrades_and_does_not_lie`] — are written as
//! `assert_eq!(…, cfg!(feature = "parallel"))` so they test the *right* thing in
//! both configurations rather than being weakened to suit the weaker one.
//!
//! # Android / Termux
//!
//! Nothing here is desktop-only. `rayon` is pure Rust and is not target-gated;
//! the `gpu` feature is target-gated off Android in `Cargo.toml` and this file
//! only ever touches the always-present `compute` dispatch API, never `wgpu`.
//! [`MachineLoad`] degrades to "unavailable" if `/proc/loadavg` cannot be read
//! rather than failing. This file therefore carries **no**
//! `#![cfg(not(target_os = "android"))]`, and
//! `cargo check -p outram-foam-basic-lib --all-targets --target
//! aarch64-linux-android` passes with and without `--features parallel`
//! (verified 2026-08-13; see the results table below).
//!
//! # V&V results — measured 2026-08-13
//!
//! **Hardware.** One virtualised x86-64 sandbox, `available_parallelism() = 4`,
//! `cargo test --release`. Each run prints its own `/proc/loadavg` snapshot; the
//! ones for the recorded runs are given under "Load conditions" below.
//!
//! **Methodology.** [`run_all_kernels`] builds **47** kernel outputs from
//! fixed-seed pseudorandom inputs (xorshift64\*, no `rand` dependency, so every
//! input is reproducible run to run and machine to machine), at sizes at or above
//! each module's own measured dispatch floor so the parallel path is genuinely
//! entered. Outputs are flattened to `Vec<f64>` and compared per the kernel's
//! [`ParityClass`]: **40 are `Bitwise`, 7 are `Reassociating`** (the fixed-chunk
//! tree reductions in `fields::parallel`: `sum`, `mean`, `l2_norm`, `dot`,
//! `vol_integral`, `vol_average`, `vol_l2_norm`).
//!
//! **Results — all six gates pass under BOTH feature settings.** Every number
//! below was printed by the test and transcribed; none is predicted.
//!
//! | Gate | Default build | `--features parallel` |
//! |---|---|---|
//! | `gate_parity_serial_vs_cpu_multi` | pass — 40 bitwise, 7 within tolerance; worst rel. dev. `0.0000e0` | pass — 40 bitwise, 7 within tolerance; worst rel. dev. `9.3428e-13` at `fields::vol_average` |
//! | `gate_parity_serial_vs_gpu` | pass — same counts, worst `0.0000e0` | pass — same counts, worst `9.3428e-13` |
//! | `gate_gpu_degrades_and_does_not_lie` | pass — all 7 dispatchers report `serial` | pass — all 7 report `cpu-multi` |
//! | `gate_dispatch_floors_are_enforced` | pass — 16/32/256/256/1024/4096/262144, spread 16384x | pass |
//! | `gate_thread_count_invariance` | pass — 4 child processes x 94 digests, all identical | pass — 4 x 94, all identical |
//! | `machine_load_report` | pass | pass |
//!
//! In the default build the worst deviation is **exactly** `0.0000e0`, which is
//! the expected and honest outcome: `CpuMulti` resolves to `Serial`, so the
//! re-associating class is compared against itself. Under `--features parallel`
//! the worst deviation is `9.3428e-13` against a `1e-11` gate — the tolerance
//! `fields::parallel` documents for its own fixed-chunk tree reduction, not a
//! number chosen to make this file pass.
//!
//! **Load conditions of the recorded runs.** `cores=4`;
//! `loadavg=1.20/0.86/1.04` (`load1-per-core=0.300`) for the `--features
//! parallel` run and `loadavg=2.58/1.26/1.17` (`0.645`) for the default-build
//! run — verdict **`MODERATE (timings distorted)`** in both cases. The host
//! could not be brought to `IDLE`: `ps` showed a `bn daemon run` process holding
//! a steady ~36 % of one core for the whole session, on top of a concurrently
//! building sibling checkout. Parity results are load-independent, so this does
//! not qualify them; it is recorded because the crossover figures in
//! [`crossover_benchmark`] *are* load-dependent, and this file's whole point is
//! that such a figure without its load is incomplete.
//!
//! **No human V&V is claimed.** These are automated verification gates written
//! by an AI-assisted draft process. The crate README's `## Bookkeeping status`
//! axes stay ❌ until the maintainer personally signs off.
//!
//! # Running it
//!
//! ```text
//! cargo test -p outram-foam-basic-lib --release --test hybrid_parity
//! cargo test -p outram-foam-basic-lib --release --features parallel --test hybrid_parity
//!
//! # the crossover benchmarks (slow; see each one's doc comment for wall clock)
//! cargo test -p outram-foam-basic-lib --release --features parallel \
//!     --test hybrid_parity -- --ignored --nocapture --test-threads=1
//! ```

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;

use outram_foam_basic_lib::compute::{
    gpu_adapter_present, select_backend, ComputeBackend, GPU_MIN_WORK_ITEMS,
};
use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel as fp;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::ldu_matrix::parallel as lp;
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::math::minimise as mm;
use outram_foam_basic_lib::math::parallel as mp;
use outram_foam_basic_lib::matrix::SquareMatrix;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::ode::parallel as op;
use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
use outram_foam_basic_lib::polynomial::{CubicEqn, LinearEqn, QuadraticEqn, RootType, Roots};
use outram_foam_basic_lib::primitives::Vector3;

// ─────────────────────────────────────────────────────────────────────────────
// § 1  Machine load — recorded beside every measurement
// ─────────────────────────────────────────────────────────────────────────────

/// How trustworthy a wall-clock measurement taken right now would be.
///
/// A speed-up figure is only meaningful relative to the contention it was
/// measured under. This is an enum rather than a bare `bool` so that the middle
/// case — busy enough to distort, not busy enough to be obviously wrong — has a
/// name and cannot be rounded away to "fine".
///
/// # Units
///
/// Dimensionless — a classification of the 1-minute load average divided by the
/// logical core count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadVerdict {
    /// `/proc/loadavg` could not be read. No claim either way.
    Unknown,
    /// 1-minute load below 0.25 per logical core. Timings are usable.
    Idle,
    /// Between 0.25 and 0.75 per logical core. Timings are distorted; report
    /// them as indicative only.
    Moderate,
    /// At or above 0.75 per logical core. Timings are **not** authoritative —
    /// this is the regime in which a previously-measured `3.1-4.0x` quadrature
    /// speed-up collapsed to `0.98-1.02x`.
    Contended,
}

impl LoadVerdict {
    /// A short label for a report line.
    fn label(self) -> &'static str {
        match self {
            Self::Unknown => "LOAD-UNKNOWN",
            Self::Idle => "IDLE",
            Self::Moderate => "MODERATE (timings distorted)",
            Self::Contended => "CONTENDED (timings NOT authoritative)",
        }
    }
}

/// A snapshot of how busy this machine is, taken beside every timing.
///
/// # Fields
///
/// - `cores` — [`std::thread::available_parallelism`], a dimensionless count;
///   falls back to 1 if the query fails.
/// - `load1` / `load5` / `load15` — the 1-, 5- and 15-minute load averages from
///   `/proc/loadavg`. Dimensionless (a count of runnable-or-blocked tasks), and
///   `f64::NAN` when the file is unreadable.
#[derive(Debug, Clone, Copy)]
struct MachineLoad {
    cores: usize,
    load1: f64,
    load5: f64,
    load15: f64,
}

impl MachineLoad {
    /// Probe the machine now. Never fails: an unreadable `/proc/loadavg` yields
    /// `NaN` load figures and [`LoadVerdict::Unknown`], which is the honest
    /// answer on a platform that has no such file.
    fn probe() -> Self {
        let cores = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        let (mut load1, mut load5, mut load15) = (f64::NAN, f64::NAN, f64::NAN);
        if let Ok(text) = std::fs::read_to_string("/proc/loadavg") {
            let mut it = text.split_whitespace();
            load1 = it.next().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
            load5 = it.next().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
            load15 = it.next().and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
        }
        Self {
            cores,
            load1,
            load5,
            load15,
        }
    }

    /// 1-minute load average per logical core, dimensionless. `NaN` when the
    /// load average is unavailable.
    fn per_core(self) -> f64 {
        self.load1 / self.cores as f64
    }

    /// Classify the current contention. See [`LoadVerdict`].
    fn verdict(self) -> LoadVerdict {
        let p = self.per_core();
        if !p.is_finite() {
            LoadVerdict::Unknown
        } else if p < 0.25 {
            LoadVerdict::Idle
        } else if p < 0.75 {
            LoadVerdict::Moderate
        } else {
            LoadVerdict::Contended
        }
    }

    /// One line, suitable for printing directly beside a timing.
    fn line(self) -> String {
        format!(
            "cores={} loadavg={:.2}/{:.2}/{:.2} load1-per-core={:.3} -> {}",
            self.cores,
            self.load1,
            self.load5,
            self.load15,
            self.per_core(),
            self.verdict().label(),
        )
    }
}

/// Print the load snapshot that brackets a benchmark, before and after.
///
/// Printed *both* sides because the 1-minute average lags: a benchmark that runs
/// for a minute contributes to its own "after" figure, and a machine that was
/// quiet at the start may not have stayed quiet.
fn print_load_bracket(tag: &str, before: MachineLoad, after: MachineLoad) {
    println!("[{tag}] load before: {}", before.line());
    println!("[{tag}] load after : {}", after.line());
    if before.verdict() == LoadVerdict::Contended || after.verdict() == LoadVerdict::Contended {
        println!(
            "[{tag}] *** WARNING: this machine was contended. The speed-ups below \
             are NOT authoritative and must not be transcribed into any constant's \
             documentation as a measured crossover. ***"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// § 2  Deterministic input source
// ─────────────────────────────────────────────────────────────────────────────

/// xorshift64\* pseudorandom generator — fixed seed, no crate dependency, so
/// every input in this file is reproducible run to run and machine to machine.
///
/// Reproducibility is load-bearing here rather than cosmetic: the whole gate is
/// "the same problem through two backends", and a non-reproducible input would
/// make a failure impossible to attribute.
struct Xorshift(u64);

impl Xorshift {
    /// Seed the generator. The seed is forced odd; xorshift64 has a fixed point
    /// at zero.
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform on `[0, 1)`, dimensionless.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Uniform on `[lo, hi)`, in whatever units the caller assigns.
    fn in_range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Uniform on `[-1, 1)`, dimensionless.
    fn signed(&mut self) -> f64 {
        self.unit().mul_add(2.0, -1.0)
    }

    /// `n` elements uniform on `[-1, 1)`.
    fn vector(&mut self, n: usize) -> Vec<f64> {
        (0..n).map(|_| self.signed()).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// § 3  Digest — the only thing that can cross a process boundary
// ─────────────────────────────────────────────────────────────────────────────

/// An order-sensitive 64-bit digest of a sequence of `f64` **bit patterns**.
///
/// Used only by [`gate_thread_count_invariance`], which compares results across
/// separate processes and so cannot pass a `Vec<f64>` back. Within one process
/// the gates compare element by element instead, because a digest cannot say
/// *which* element drifted.
///
/// Mixing is splitmix64's finaliser applied per word, which is order-dependent
/// and mixes well enough that an accidental collision between two genuinely
/// different `Vec<f64>` is not a realistic failure mode for a test oracle. It is
/// a change detector, not a cryptographic hash, and nothing security-relevant
/// depends on it.
#[derive(Debug, Clone, Copy)]
struct Digest(u64);

impl Digest {
    fn new() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }

    fn word(&mut self, v: u64) {
        let mut x = self.0 ^ v.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        self.0 = x;
    }

    /// Absorb a slice of `f64` by bit pattern, length first.
    fn values(&mut self, xs: &[f64]) {
        self.word(xs.len() as u64);
        for v in xs {
            self.word(v.to_bits());
        }
    }

    fn finish(self) -> u64 {
        self.0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// § 4  Parity classes and the kernel-output record
// ─────────────────────────────────────────────────────────────────────────────

/// How a kernel's two backends are allowed to differ.
///
/// The distinction is the substance of this gate. A tolerance applied to a
/// kernel that is *supposed* to be bitwise identical would silently accept a
/// regression from "identical" to "close", which is exactly the kind of drift a
/// parity gate exists to catch. So each kernel declares its class once, in
/// [`run_all_kernels`], and the comparison rule follows from the class.
///
/// # Units
///
/// `tolerance` is a **relative** deviation, dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParityClass {
    /// Every output element must be bit-for-bit equal across backends, compared
    /// with [`f64::to_bits`].
    ///
    /// This is achievable — and claimed by four of the five modules — whenever
    /// no cross-lane arithmetic exists (an element-wise map, or one independent
    /// problem per lane), or whenever the reduction is chunked identically on
    /// both paths, as `ldu_matrix::parallel` does with its fixed
    /// `REDUCTION_BLOCK`.
    Bitwise,
    /// The backends re-associate a floating-point sum and therefore *cannot* be
    /// bitwise identical. Compared against the stated relative `tolerance`,
    /// which must be the one the owning module documents — never a number chosen
    /// to make this file pass.
    Reassociating {
        /// Relative-deviation gate, dimensionless.
        tolerance: f64,
        /// Where that tolerance is documented, so a reader can check it.
        documented_in: &'static str,
    },
}

impl ParityClass {
    fn label(self) -> &'static str {
        match self {
            Self::Bitwise => "bitwise",
            Self::Reassociating { .. } => "re-associating",
        }
    }
}

/// One kernel's output on one backend, flattened for comparison.
///
/// `values` is the kernel's entire observable output flattened to `f64`: for a
/// field kernel that is the field data; for a batched solver it is every
/// observable field of every lane (iterate, residual, bracket width, iteration
/// count, status code), so a changed iteration count fails the gate exactly as a
/// changed answer would. That matters for the iterative kernels specifically —
/// the bead requires iteration counts to be compared, not just final answers.
struct KernelOutput {
    /// Stable kernel identifier, `module::function[variant]`.
    name: String,
    /// Comparison rule for this kernel.
    class: ParityClass,
    /// The kernel's flattened observable output.
    values: Vec<f64>,
}

/// A small helper so a status enum becomes a stable numeric code in the output
/// vector. Uses each status enum's own `label()`, hashed, so a renamed variant
/// changes the code and a reordered enum does not.
fn label_code(label: &str) -> f64 {
    let mut d = Digest::new();
    for b in label.as_bytes() {
        d.word(u64::from(*b));
    }
    // Bit-cast the low 52 bits into a finite f64. Any injective-enough map into
    // a comparable f64 works; this one is exactly reproducible.
    (d.finish() & ((1_u64 << 52) - 1)) as f64
}

/// [`RootType`] as a numeric code, so polynomial-root *types* are compared, not
/// only root values.
fn root_type_code(t: RootType) -> f64 {
    match t {
        RootType::Real => 0.0,
        RootType::Complex => 1.0,
        RootType::PosInf => 2.0,
        RootType::NegInf => 3.0,
        RootType::Nan => 4.0,
    }
}

/// Flatten a `Roots<N>` into value/type pairs.
fn push_roots<const N: usize>(out: &mut Vec<f64>, r: &Roots<N>) {
    for i in 0..N {
        let v = r.get(i);
        // NaN is a legitimate root slot value; map it to a fixed sentinel so
        // `to_bits` comparison is not defeated by NaN payload differences that
        // carry no information.
        out.push(if v.is_nan() { f64::MAX } else { v });
        out.push(root_type_code(r.root_type(i)));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// § 5  Problem sizes — at or above every module's own measured floor
// ─────────────────────────────────────────────────────────────────────────────
//
// The public API enforces each module's size floor, so a batch below the floor
// runs serially on BOTH backends and the parity comparison becomes vacuous. Every
// size below is therefore at or above the relevant floor, and
// `gate_dispatch_floors_are_enforced` pins the floors themselves.

/// Field element count: just above `fields::parallel::FIELD_PARALLEL_CROSSOVER`
/// (131 072) and deliberately **not** a multiple of `REDUCTION_CHUNK` (4096), so
/// the ragged final chunk of the tree reduction is exercised.
const FIELD_N: usize = 131_101;

/// LDU mesh: 16 x 16 x 16 = 4096 cells, exactly `SPMV_MIN_CELLS`, so the sparse
/// product runs its parallel path at the smallest size a caller can reach it.
const LDU_NX: usize = 16;

/// Vector length for the LDU vector operations: exactly `VECOP_MIN_ELEMENTS`.
const VECOP_N: usize = 262_144;

/// Root-finding / minimisation batch size: 4x `ROOT_BATCH_MIN_PROBLEMS` and
/// `MINIMISE_BATCH_MIN_PROBLEMS` (both 256), so rayon genuinely splits.
const BATCH_N: usize = 1_024;

/// Closed-form polynomial batch size: 2x `POLY_ROOTS_MIN_EQUATIONS` (1024), and
/// 8x `POLY_BLOCK` (256) so the block splitter is exercised.
const POLY_N: usize = 2_048;

/// ODE ensemble lanes: well above `ODE_ENSEMBLE_MIN_LANES` (16) while keeping
/// the whole harness inside a few hundred milliseconds.
const ODE_N: usize = 256;

/// Quadrature lanes: 32x `QUADRATURE_MIN_INTERVALS` (32).
const QUAD_N: usize = 1_024;

// ─────────────────────────────────────────────────────────────────────────────
// § 6  Fixtures
// ─────────────────────────────────────────────────────────────────────────────

/// Owner/neighbour addressing of a structured `n^3` hexahedral mesh with a
/// 7-point stencil — the canonical finite-volume connectivity, faces in
/// cell-major order with `owner < neighbour` as `LduMatrix` documents.
fn structured_faces(n: usize) -> (Vec<usize>, Vec<usize>) {
    let id = |i: usize, j: usize, k: usize| (k * n + j) * n + i;
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let c = id(i, j, k);
                if i + 1 < n {
                    owner.push(c);
                    neighbour.push(id(i + 1, j, k));
                }
                if j + 1 < n {
                    owner.push(c);
                    neighbour.push(id(i, j + 1, k));
                }
                if k + 1 < n {
                    owner.push(c);
                    neighbour.push(id(i, j, k + 1));
                }
            }
        }
    }
    (owner, neighbour)
}

/// A fixed-seed, diagonally dominant 7-point-stencil matrix on an `n^3` mesh.
///
/// Diagonal dominance is not decoration: it keeps `normalised_residual` away
/// from the catastrophic-cancellation regime where a tolerance comparison would
/// be dominated by conditioning rather than by backend differences.
fn random_matrix(n: usize, seed: u64) -> LduMatrix {
    let (owner, neighbour) = structured_faces(n);
    let n_cells = n * n * n;
    let mut m = LduMatrix::new(n_cells, owner, neighbour);
    let mut rng = Xorshift::new(seed);
    for f in 0..m.n_internal_faces {
        m.lower[f] = -rng.in_range(0.1, 1.0);
        m.upper[f] = -rng.in_range(0.1, 1.0);
    }
    let mut row: Vec<f64> = vec![0.0; n_cells];
    for f in 0..m.n_internal_faces {
        row[m.owner[f]] += m.upper[f].abs();
        row[m.neighbour[f]] += m.lower[f].abs();
    }
    for c in 0..n_cells {
        m.diag[c] = row[c] + rng.in_range(1.0, 2.0);
    }
    m
}

/// Field data with a wide dynamic range, so that summation order genuinely
/// matters — a flat left-to-right sum and a chunked tree sum disagree in the
/// last bits on this data, which is what makes the re-associating class
/// non-vacuous.
fn sample_field(n: usize) -> Field<f64> {
    Field::from_fn(n, |i| {
        let t = i as f64;
        (t * 0.618_033_988_749_89).sin() * 1.0e6 + (t * 0.123_456_789_012_345).cos() * 1.0e-7
    })
}

/// A second, differently-shaped field, kept away from zero so `pointwise_div`
/// is well posed.
fn sample_field_b(n: usize) -> Field<f64> {
    Field::from_fn(n, |i| {
        let t = i as f64 + 0.5;
        (t * 0.271_828_182_845_90).cos() * 1.0e3 + 1.5e3
    })
}

fn sample_vector_field(n: usize) -> Field<Vector3> {
    Field::from_fn(n, |i| {
        let t = i as f64;
        Vector3::new(t.sin(), t.cos(), (t * 0.5).sin())
    })
}

/// A `VolScalarField` on a periodic 1-D mesh with an explicit non-zero boundary
/// value, so the wrappers' boundary handling is genuinely exercised rather than
/// comparing `0 + 0 == 0`.
fn vol_scalar(name: &str, mesh: Arc<FvMesh>, internal: Field<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::fixed_value(p.size, 0.75))
        .collect();
    VolScalarField::new(name, mesh, internal, boundary)
}

/// `SurfaceScalarField` counterpart of [`vol_scalar`].
fn surface_scalar(name: &str, mesh: Arc<FvMesh>, internal: Field<f64>) -> SurfaceScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::fixed_value(p.size, -0.25))
        .collect();
    SurfaceScalarField::new(name, mesh, internal, boundary)
}

/// `dy/dx = -k y`, closed form `y(x) = y0 exp(-k x)`. Implements `jacobian` so
/// the stiff Rosenbrock23 stepper can be used on it in the mixed ensemble.
struct Decay {
    k: f64,
}

impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize {
        1
    }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -self.k * y[0];
    }
    fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdy.set(0, 0, -self.k);
    }
}

/// A deliberately **imbalanced** ODE ensemble: decay rates and integration
/// spans vary by more than an order of magnitude across lanes, so per-lane cost
/// is wildly uneven and a work-stealing scheduler is forced to interleave lanes
/// differently at different thread counts. If per-lane state leaked between
/// lanes, this is the shape that would expose it.
fn ode_lanes(n: usize) -> Vec<op::OdeLane<Decay>> {
    let mut rng = Xorshift::new(0x0de_0de);
    (0..n)
        .map(|i| {
            let k = rng.in_range(0.2, 4.0);
            let span = if i % 17 == 0 { 12.0 } else { 1.0 };
            op::OdeLane::new(Decay { k }, vec![rng.in_range(0.5, 2.0)], 0.0, span, 0.01)
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// § 7  The kernel inventory — ONE place a new kernel must be added
// ─────────────────────────────────────────────────────────────────────────────

/// Run **every** hybrid-backend kernel in the crate on `backend` and return each
/// one's flattened output plus its parity class.
///
/// # This is the extension point
///
/// A new hybrid kernel is covered by every gate in this file the moment it is
/// pushed onto `out` here. Nothing else needs editing. That is the mechanism by
/// which "a future kernel cannot land without meeting the same bar" is enforced
/// structurally rather than by convention.
///
/// # Arguments
///
/// - `backend` — the backend to request. Every kernel is asked for exactly this
///   one; each module then applies its own documented degradation and size
///   floor, which is what makes the comparison a test of the *shipped* dispatch
///   policy rather than of a bypassed one.
///
/// # Returns
///
/// One [`KernelOutput`] per kernel, in a fixed order, so two calls with
/// different backends can be zipped positionally.
fn run_all_kernels(backend: ComputeBackend) -> Vec<KernelOutput> {
    let mut out: Vec<KernelOutput> = Vec::new();

    /// Relative-deviation gate for the fixed-chunk tree reductions in
    /// `fields::parallel`, taken from that module's own V&V test
    /// (`vv_parallel_sum_matches_serial_within_tolerance`, gate `1e-11`).
    const FIELD_REDUCTION_TOL: ParityClass = ParityClass::Reassociating {
        tolerance: 1e-11,
        documented_in: "fields::parallel — module docs, \"Reduction determinism\"; \
                        gate 1e-11 in vv_parallel_sum_matches_serial_within_tolerance",
    };

    let bitwise = |name: &str, values: Vec<f64>, out: &mut Vec<KernelOutput>| {
        out.push(KernelOutput {
            name: name.to_string(),
            class: ParityClass::Bitwise,
            values,
        });
    };

    // ── ldu_matrix::parallel ─────────────────────────────────────────────────
    //
    // Every kernel in this module is Bitwise, including the reductions: the
    // cell-gather visits each cell's incident faces in ascending face index —
    // the order the serial scatter reaches them — and `dot`/`norm_*` use the
    // same fixed `REDUCTION_BLOCK` chunking on both paths, combined in index
    // order. So there is no association to perturb.
    {
        let m = Arc::new(random_matrix(LDU_NX, 0x1d0_1d0));
        let ldu = lp::HybridLdu::new(Arc::clone(&m));
        let mut rng = Xorshift::new(0x5eed_1);
        let x = rng.vector(m.n_cells);
        let b = rng.vector(m.n_cells);

        bitwise("ldu::spmv", ldu.spmv(&x, backend), &mut out);
        bitwise("ldu::residual", ldu.residual(&x, &b, backend), &mut out);
        bitwise(
            "ldu::normalised_residual",
            vec![ldu.normalised_residual(&x, &b, backend)],
            &mut out,
        );
        bitwise(
            "ldu::diagonal_reciprocal",
            ldu.diagonal_reciprocal(backend),
            &mut out,
        );

        let mut rng = Xorshift::new(0x5eed_2);
        let u = rng.vector(VECOP_N);
        let v = rng.vector(VECOP_N);
        bitwise("ldu::dot", vec![lp::dot(&u, &v, backend)], &mut out);
        bitwise("ldu::norm_l1", vec![lp::norm_l1(&u, backend)], &mut out);
        bitwise("ldu::norm_l2", vec![lp::norm_l2(&u, backend)], &mut out);
        let mut y = v.clone();
        lp::axpy(-0.318_309_886, &u, &mut y, backend);
        bitwise("ldu::axpy", y, &mut out);
    }

    // ── fields::parallel ─────────────────────────────────────────────────────
    {
        let a = sample_field(FIELD_N);
        let b = sample_field_b(FIELD_N);
        let va = sample_vector_field(FIELD_N);
        let vb = sample_vector_field(FIELD_N);

        let flat = |f: &Field<f64>| f.as_slice().to_vec();
        let flat_vec = |f: &Field<Vector3>| {
            let mut v = Vec::with_capacity(f.len() * 3);
            for p in f.as_slice() {
                v.push(p.x);
                v.push(p.y);
                v.push(p.z);
            }
            v
        };

        // Element-wise: bit-identical on every backend, because each output
        // element is produced by the identical expression whichever thread
        // evaluates it. No map can re-associate.
        bitwise("fields::add", flat(&fp::add(backend, &a, &b)), &mut out);
        bitwise("fields::sub", flat(&fp::sub(backend, &a, &b)), &mut out);
        bitwise(
            "fields::scale",
            flat(&fp::scale(backend, &a, 2.718_281_828)),
            &mut out,
        );
        bitwise(
            "fields::axpy",
            flat(&fp::axpy(backend, &a, -1.414_213_562, &b)),
            &mut out,
        );
        bitwise(
            "fields::pointwise_mul",
            flat(&fp::pointwise_mul(backend, &a, &b)),
            &mut out,
        );
        bitwise(
            "fields::pointwise_div",
            flat(&fp::pointwise_div(backend, &a, &b)),
            &mut out,
        );
        bitwise(
            "fields::scale_by_field",
            flat_vec(&fp::scale_by_field(backend, &va, &b)),
            &mut out,
        );
        bitwise(
            "fields::dot_field",
            flat(&fp::dot_field(backend, &va, &vb)),
            &mut out,
        );
        {
            let mut y = a.clone();
            fp::axpy_assign(backend, &mut y, 0.577_215_664, &b);
            bitwise("fields::axpy_assign", flat(&y), &mut out);
        }
        {
            let mut y = a.clone();
            fp::add_assign(backend, &mut y, &b);
            bitwise("fields::add_assign", flat(&y), &mut out);
        }
        // min/max are associative, so they are bitwise even though they reduce.
        bitwise("fields::min", vec![fp::min(backend, &a)], &mut out);
        bitwise("fields::max", vec![fp::max(backend, &a)], &mut out);

        // The re-associating reductions. These are the ONLY kernels in the
        // crate that cannot be bitwise, and each is labelled as such rather
        // than being given a quiet tolerance.
        out.push(KernelOutput {
            name: "fields::sum".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::sum(backend, &a)],
        });
        out.push(KernelOutput {
            name: "fields::mean".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::mean(backend, &a)],
        });
        out.push(KernelOutput {
            name: "fields::l2_norm".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::l2_norm(backend, &a)],
        });
        out.push(KernelOutput {
            name: "fields::dot".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::dot(backend, &a, &b)],
        });

        // Vol / surface wrappers, plus the mesh-weighted reductions.
        let mesh = Arc::new(FvMesh::periodic_1d(FIELD_N, 1.0, 1.0));
        let phi = vol_scalar("phi", Arc::clone(&mesh), a.clone());
        let psi = vol_scalar("psi", Arc::clone(&mesh), b.clone());
        let sum_vol = fp::add_vol(backend, &phi, &psi);
        let mut vol_values = sum_vol.internal.as_slice().to_vec();
        for p in &sum_vol.boundary {
            vol_values.extend_from_slice(p.values.as_slice());
        }
        bitwise("fields::add_vol", vol_values, &mut out);

        let scaled_vol = fp::scale_vol(backend, &phi, -3.5);
        bitwise(
            "fields::scale_vol",
            scaled_vol.internal.as_slice().to_vec(),
            &mut out,
        );

        let n_faces = mesh.n_internal_faces;
        let sf_a = surface_scalar("phiA", Arc::clone(&mesh), sample_field(n_faces));
        let sf_b = surface_scalar("phiB", Arc::clone(&mesh), sample_field_b(n_faces));
        let sf_sum = fp::add_surface(backend, &sf_a, &sf_b);
        bitwise(
            "fields::add_surface",
            sf_sum.internal.as_slice().to_vec(),
            &mut out,
        );

        bitwise(
            "fields::vol_min",
            vec![fp::vol_min(backend, &phi)],
            &mut out,
        );
        bitwise(
            "fields::vol_max",
            vec![fp::vol_max(backend, &phi)],
            &mut out,
        );
        out.push(KernelOutput {
            name: "fields::vol_integral".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::vol_integral(backend, &phi)],
        });
        out.push(KernelOutput {
            name: "fields::vol_average".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::vol_average(backend, &phi)],
        });
        out.push(KernelOutput {
            name: "fields::vol_l2_norm".into(),
            class: FIELD_REDUCTION_TOL,
            values: vec![fp::vol_l2_norm(backend, &phi)],
        });
    }

    // ── math::parallel — batched root finding ────────────────────────────────
    //
    // Every observable field of every lane is compared, iteration count and
    // status included: the bead requires that a changed iteration count fails
    // even when the final answer is unchanged, because that is how a changed
    // algorithm shows up first.
    {
        let mut rng = Xorshift::new(0x0000_0072_0000_0074);
        let targets: Vec<f64> = (0..BATCH_N).map(|_| rng.in_range(0.2, 40.0)).collect();
        let problems: Vec<mp::RootProblem> = (0..BATCH_N)
            .map(|_| mp::RootProblem::new(0.0, 8.0))
            .collect();
        let guess_problems: Vec<mp::RootProblem> = (0..BATCH_N)
            .map(|_| mp::RootProblem::with_guess(0.0, 8.0, 1.0))
            .collect();
        let settings = mp::RootSettings::default();

        // Deliberately imbalanced: lanes whose index is a multiple of 11 pay a
        // much costlier residual, so per-lane cost is uneven.
        let residual = |i: usize, x: f64| {
            let t = targets[i];
            let mut acc = x * x - t;
            if i % 11 == 0 {
                for k in 0..64 {
                    acc += (x * (k as f64 + 1.0)).sin() * 1.0e-18;
                }
            }
            acc
        };

        for (label, method) in [
            ("brent", mp::RootMethod::Brent),
            ("bisection", mp::RootMethod::Bisection),
        ] {
            let batch = mp::solve_bracketed_batch(&problems, method, settings, backend, residual);
            let mut v = Vec::with_capacity(batch.len() * 5);
            for s in batch.solutions() {
                v.push(s.last_iterate());
                v.push(s.residual());
                v.push(s.bracket_width());
                v.push(f64::from(s.iterations()));
                v.push(label_code(s.status().label()));
            }
            bitwise(
                &format!("math::solve_bracketed_batch[{label}]"),
                v,
                &mut out,
            );
        }

        let batch = mp::solve_newton_batch(&guess_problems, settings, backend, |i, x| {
            (x * x - targets[i], 2.0 * x)
        });
        let mut v = Vec::with_capacity(batch.len() * 5);
        for s in batch.solutions() {
            v.push(s.last_iterate());
            v.push(s.residual());
            v.push(s.bracket_width());
            v.push(f64::from(s.iterations()));
            v.push(label_code(s.status().label()));
        }
        bitwise("math::solve_newton_batch", v, &mut out);

        // Closed-form polynomial roots.
        let mut rng = Xorshift::new(0x9911_2244);
        let linear: Vec<LinearEqn> = (0..POLY_N)
            .map(|_| LinearEqn {
                a: rng.in_range(0.5, 2.0),
                b: rng.signed(),
            })
            .collect();
        let quadratic: Vec<QuadraticEqn> = (0..POLY_N)
            .map(|_| QuadraticEqn {
                a: rng.in_range(0.5, 2.0),
                b: rng.signed(),
                c: rng.signed(),
            })
            .collect();
        let cubic: Vec<CubicEqn> = (0..POLY_N)
            .map(|_| CubicEqn {
                a: rng.in_range(0.5, 2.0),
                b: rng.signed(),
                c: rng.signed(),
                d: rng.signed(),
            })
            .collect();

        let mut v = Vec::new();
        for r in mp::linear_roots_batch(&linear, backend) {
            push_roots(&mut v, &r);
        }
        bitwise("math::linear_roots_batch", v, &mut out);

        let mut v = Vec::new();
        for r in mp::quadratic_roots_batch(&quadratic, backend) {
            push_roots(&mut v, &r);
        }
        bitwise("math::quadratic_roots_batch", v, &mut out);

        let mut v = Vec::new();
        for r in mp::cubic_roots_batch(&cubic, backend) {
            push_roots(&mut v, &r);
        }
        bitwise("math::cubic_roots_batch", v, &mut out);
    }

    // ── math::minimise — batched golden section ──────────────────────────────
    {
        let mut rng = Xorshift::new(0x60_1de_1);
        let centres: Vec<f64> = (0..BATCH_N).map(|_| rng.in_range(-2.0, 2.0)).collect();
        let problems: Vec<mm::MinProblem> = (0..BATCH_N)
            .map(|_| mm::MinProblem::new(-5.0, 5.0))
            .collect();
        let settings = mm::MinSettings::default();

        let objective = |i: usize, x: f64| {
            let d = x - centres[i];
            let mut acc = 1.0 + d * d;
            if i % 13 == 0 {
                for k in 0..48 {
                    acc += (x * (k as f64 + 1.0)).cos() * 1.0e-18;
                }
            }
            acc
        };

        for (label, sense) in [
            ("minimise", mm::Sense::Minimise),
            ("maximise", mm::Sense::Maximise),
        ] {
            let batch = mm::golden_section_batch(&problems, sense, settings, backend, objective);
            let mut v = Vec::with_capacity(batch.len() * 5);
            for s in batch.solutions() {
                v.push(s.last_iterate());
                v.push(s.last_value());
                v.push(s.bracket_width());
                v.push(f64::from(s.iterations()));
                v.push(label_code(s.status().label()));
            }
            bitwise(
                &format!("minimise::golden_section_batch[{label}]"),
                v,
                &mut out,
            );
        }
    }

    // ── ode::parallel — ensembles and quadrature ─────────────────────────────
    {
        let lanes = ode_lanes(ODE_N);

        let flatten_ensemble = |e: &op::OdeEnsemble| {
            let mut v = Vec::with_capacity(e.len() * 5);
            for lane in e.lanes() {
                v.extend_from_slice(lane.last_state());
                v.push(lane.x_reached());
                v.push(lane.dx_final());
                v.push(f64::from(lane.steps()));
                v.push(label_code(lane.status().label()));
            }
            v
        };

        let rkf = OdeSolver::rkf45(1, 1e-9, 1e-9);
        let e = op::integrate_ensemble(&lanes, &rkf, backend);
        bitwise(
            "ode::integrate_ensemble[rkf45]",
            flatten_ensemble(&e),
            &mut out,
        );

        let euler = OdeSolver::euler(1, 1e-6, 1e-6);
        let e = op::integrate_ensemble(&lanes, &euler, backend);
        bitwise(
            "ode::integrate_ensemble[euler]",
            flatten_ensemble(&e),
            &mut out,
        );

        // Mixed stiffness: a Rosenbrock23 stepper on every 5th lane. This is
        // the case where a shared scratch buffer would show up, because the two
        // steppers have different internal state.
        let e = op::integrate_ensemble_mixed(
            &lanes,
            |i| {
                if i % 5 == 0 {
                    OdeSolver::rosenbrock23(1, 1e-9, 1e-9)
                } else {
                    OdeSolver::rkf45(1, 1e-9, 1e-9)
                }
            },
            backend,
        );
        bitwise(
            "ode::integrate_ensemble_mixed[rkf45+rosenbrock23]",
            flatten_ensemble(&e),
            &mut out,
        );

        // Quadrature. One lane is one integral, summed by one thread, so this
        // is bitwise by construction — parallelism is over lanes, never within
        // a lane.
        let mut rng = Xorshift::new(0x0000_0071_0000_0064);
        let intervals: Vec<op::QuadratureInterval> = (0..QUAD_N)
            .map(|_| {
                let a = rng.in_range(0.0, 1.0);
                op::QuadratureInterval::new(a, a + rng.in_range(0.5, 3.0))
            })
            .collect();
        let integrand = |i: usize, x: f64| (-x).exp() * (x * (1.0 + (i % 7) as f64)).sin();

        let flatten_quad = |q: &op::QuadratureBatch| {
            let mut v = Vec::with_capacity(q.len() * 4);
            for s in q.solutions() {
                v.push(s.last_value());
                // The error estimate is NaN on the fixed-rule path by design;
                // map it to a sentinel so `to_bits` compares the *fact* of it
                // being NaN rather than a NaN payload.
                let e = s.error_estimate();
                v.push(if e.is_nan() { f64::MAX } else { e });
                v.push(f64::from(s.evaluations()));
                v.push(label_code(s.status().label()));
            }
            v
        };

        for (label, rule) in [
            (
                "gauss-legendre-g5x16",
                op::QuadratureRule::GaussLegendre {
                    order: op::GaussOrder::G5,
                    panels: 16,
                },
            ),
            ("simpson-64", op::QuadratureRule::Simpson { panels: 64 }),
            (
                "trapezoid-128",
                op::QuadratureRule::Trapezoid { panels: 128 },
            ),
        ] {
            let q = op::quadrature_batch(&intervals, rule, backend, integrand);
            bitwise(
                &format!("ode::quadrature_batch[{label}]"),
                flatten_quad(&q),
                &mut out,
            );
        }

        let q = op::adaptive_quadrature_batch(
            &intervals,
            op::AdaptiveSettings::default(),
            backend,
            integrand,
        );
        bitwise("ode::adaptive_quadrature_batch", flatten_quad(&q), &mut out);
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// § 8  Comparison
// ─────────────────────────────────────────────────────────────────────────────

/// Compare two runs of [`run_all_kernels`] and panic with a precise message on
/// the first violation.
///
/// Returns the worst relative deviation seen among the
/// [`ParityClass::Reassociating`] kernels, so a caller can print it.
fn compare_runs(reference: &[KernelOutput], candidate: &[KernelOutput], what: &str) -> f64 {
    assert_eq!(
        reference.len(),
        candidate.len(),
        "{what}: kernel inventory length differs between backends — \
         run_all_kernels must be deterministic in its kernel list"
    );

    let mut worst_rel = 0.0_f64;
    let mut worst_name = String::from("(none)");
    let mut n_bitwise = 0_usize;
    let mut n_tolerance = 0_usize;

    for (r, c) in reference.iter().zip(candidate.iter()) {
        assert_eq!(
            r.name, c.name,
            "{what}: kernel order differs between backends"
        );
        assert_eq!(
            r.values.len(),
            c.values.len(),
            "{what}: {} produced {} values on the reference backend and {} on the candidate",
            r.name,
            r.values.len(),
            c.values.len()
        );

        match r.class {
            ParityClass::Bitwise => {
                n_bitwise += 1;
                for (i, (a, b)) in r.values.iter().zip(c.values.iter()).enumerate() {
                    assert!(
                        a.to_bits() == b.to_bits(),
                        "{what}: {name} claims BITWISE parity but element {i} differs: \
                         reference {a:.17e} (bits {abits:#018x}) vs candidate {b:.17e} \
                         (bits {bbits:#018x}). A tolerance would have hidden this; that is \
                         why the comparison is on bits.",
                        name = r.name,
                        abits = a.to_bits(),
                        bbits = b.to_bits(),
                    );
                }
            }
            ParityClass::Reassociating {
                tolerance,
                documented_in,
            } => {
                n_tolerance += 1;
                for (i, (a, b)) in r.values.iter().zip(c.values.iter()).enumerate() {
                    let denom = a.abs().max(b.abs()).max(f64::MIN_POSITIVE);
                    let rel = (a - b).abs() / denom;
                    if rel > worst_rel {
                        worst_rel = rel;
                        worst_name = format!("{}[{i}]", r.name);
                    }
                    assert!(
                        rel <= tolerance,
                        "{what}: {name} element {i} deviates by {rel:.4e}, exceeding the \
                         {tolerance:.0e} tolerance documented in {documented_in}. \
                         reference {a:.17e} candidate {b:.17e}",
                        name = r.name,
                    );
                }
            }
        }
    }

    println!(
        "[{what}] {n_bitwise} kernels compared BITWISE (to_bits), \
         {n_tolerance} compared against a documented tolerance; \
         worst relative deviation {worst_rel:.4e} at {worst_name}"
    );
    worst_rel
}

// ─────────────────────────────────────────────────────────────────────────────
// § 9  Gate 1 — Serial vs CpuMulti
// ─────────────────────────────────────────────────────────────────────────────

/// **V&V — the primary parity gate.**
///
/// **Methodology.** Run every kernel in [`run_all_kernels`] on
/// [`ComputeBackend::Serial`] and again on [`ComputeBackend::CpuMulti`], on
/// byte-identical fixed-seed input, at sizes at or above each module's own
/// dispatch floor so the parallel path is genuinely entered. Compare per kernel
/// according to its declared [`ParityClass`]: `Bitwise` kernels element-wise on
/// [`f64::to_bits`], `Reassociating` kernels against the relative tolerance their
/// owning module documents. Batched-solver outputs include **iteration counts
/// and status codes**, so a changed algorithm fails here even when its final
/// answers are unchanged.
///
/// **Pass criterion.** Zero differing bits for every `Bitwise` kernel; relative
/// deviation within the stated tolerance for every `Reassociating` kernel.
///
/// **Result (2026-08-13, 4 logical cores, release).** Passes under both the
/// default build and `--features parallel`. The kernel counts and the worst
/// measured relative deviation are printed by this test; run it with
/// `--nocapture` to see them. With `parallel` **off** the comparison is exact by
/// construction — `CpuMulti` resolves to `Serial` — and that is stated in the
/// printed line rather than hidden.
#[test]
fn gate_parity_serial_vs_cpu_multi() {
    let load = MachineLoad::probe();
    println!("[serial-vs-cpu-multi] {}", load.line());
    println!(
        "[serial-vs-cpu-multi] parallel feature = {}, CpuMulti resolves to {}",
        cfg!(feature = "parallel"),
        ComputeBackend::CpuMulti.resolve().label()
    );

    let reference = run_all_kernels(ComputeBackend::Serial);
    let candidate = run_all_kernels(ComputeBackend::CpuMulti);
    println!(
        "[serial-vs-cpu-multi] {} kernels in the inventory:",
        reference.len()
    );
    for k in &reference {
        println!(
            "[serial-vs-cpu-multi]   {:<50} {:>14}  {} value(s)",
            k.name,
            k.class.label(),
            k.values.len()
        );
    }
    compare_runs(&reference, &candidate, "serial-vs-cpu-multi");
}

// ─────────────────────────────────────────────────────────────────────────────
// § 10  Gate 2 — Serial vs Gpu
// ─────────────────────────────────────────────────────────────────────────────

/// **V&V — the `Gpu` half of the parity gate.**
///
/// **Methodology.** Identical to [`gate_parity_serial_vs_cpu_multi`], requesting
/// [`ComputeBackend::Gpu`] instead.
///
/// **What this can and cannot prove.** **No GPU kernel exists in any of the five
/// modules.** Every one of them re-resolves a `Gpu` request down the CPU ladder
/// (`ldu_matrix::parallel::effective_backend` and its siblings; the same for
/// `fields::parallel::should_parallelise`). So this gate proves that requesting
/// `Gpu` produces the documented CPU result and nothing surprising — it does
/// **not** exercise any GPU arithmetic, and no f32-versus-f64 deviation is
/// measured here because none is incurred. The `Serial`-versus-`Gpu` parity
/// question the bead poses remains **entirely open** for all five modules; see
/// [`gate_gpu_degrades_and_does_not_lie`] for the tripwire that will force it to
/// be answered when a real GPU kernel lands.
///
/// **Pass criterion and result.** As
/// [`gate_parity_serial_vs_cpu_multi`]. Passes 2026-08-13 under both feature
/// settings on a host with no GPU adapter (`gpu_adapter_present()` is printed by
/// this test so the run's conditions are on the record).
#[test]
fn gate_parity_serial_vs_gpu() {
    println!(
        "[serial-vs-gpu] gpu feature = {}, adapter present = {}, Gpu resolves to {}",
        cfg!(feature = "gpu"),
        gpu_adapter_present(),
        ComputeBackend::Gpu.resolve().label()
    );
    println!(
        "[serial-vs-gpu] NOTE: no GPU kernel exists in any module; this gate checks \
         that a Gpu request yields the documented CPU result, and measures no GPU \
         arithmetic at all."
    );

    let reference = run_all_kernels(ComputeBackend::Serial);
    let candidate = run_all_kernels(ComputeBackend::Gpu);
    compare_runs(&reference, &candidate, "serial-vs-gpu");
}

// ─────────────────────────────────────────────────────────────────────────────
// § 11  Gate 3 — Gpu degrades, and does not lie
// ─────────────────────────────────────────────────────────────────────────────

/// **The tripwire: no module may report that it ran on the GPU, because none
/// can.**
///
/// **Methodology.** Every module publishes a pure dispatch-policy function that
/// reports the backend it *would* use without running anything —
/// [`lp::spmv_backend_for`], [`lp::vecop_backend_for`],
/// [`mp::root_batch_backend_for`], [`mp::poly_roots_backend_for`],
/// [`mm::minimise_backend_for`], [`op::ensemble_backend_for`],
/// [`op::quadrature_backend_for`], and `fields::parallel::should_parallelise`.
/// Ask each of them for [`ComputeBackend::Gpu`] at a size far above its floor and
/// assert the answer is **never** `Gpu`.
///
/// **Why this is the right assertion rather than trusting the docs.** The risk
/// the bead names is a future GPU kernel landing *without* a parity gate. The
/// day someone wires a real `Gpu` arm into any of these dispatchers, this test
/// goes red and the author must come back here, add the kernel to
/// [`run_all_kernels`], and state the precision it runs at and its measured
/// deviation from the f64 serial oracle — which is exactly the maintainer's
/// recorded requirement for accepting f32 on the GPU.
///
/// It also asserts the second half of "does not lie": that
/// [`select_backend`] — the crate-wide policy — *can* return `Gpu` on a machine
/// with an adapter, while no kernel honours it. That asymmetry is real and is
/// recorded here rather than left for a caller to discover.
///
/// **Pass criterion.** No dispatch helper returns `Gpu`; the field dispatcher
/// treats `Gpu` exactly as `CpuMulti`.
///
/// **Result (2026-08-13).** Passes under the default build and under
/// `--features parallel`, on a host reporting `gpu_adapter_present() == false`.
#[test]
fn gate_gpu_degrades_and_does_not_lie() {
    let big = 1 << 22;

    let dispatchers: [(&str, ComputeBackend); 7] = [
        (
            "ldu::spmv_backend_for",
            lp::spmv_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "ldu::vecop_backend_for",
            lp::vecop_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "math::root_batch_backend_for",
            mp::root_batch_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "math::poly_roots_backend_for",
            mp::poly_roots_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "minimise::minimise_backend_for",
            mm::minimise_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "ode::ensemble_backend_for",
            op::ensemble_backend_for(ComputeBackend::Gpu, big),
        ),
        (
            "ode::quadrature_backend_for",
            op::quadrature_backend_for(ComputeBackend::Gpu, big),
        ),
    ];

    for (name, picked) in dispatchers {
        println!("[gpu-degradation] {name}(Gpu, {big}) -> {}", picked.label());
        assert_ne!(
            picked,
            ComputeBackend::Gpu,
            "{name} auto-selected ComputeBackend::Gpu. Auto-select must never do that: \
             the GPU kernels in this crate are f32 (WGSL has no f64) and are opt-in only, \
             so a caller gets one by naming Gpu explicitly and never by default. \
             If a kernel has been measured to beat CpuMulti and should now be auto-selected: \
             record the crossover from examples/hybrid_gpu_report.rs, state the precision \
             and the measured max/RMS deviation from the f64 serial oracle in its doc \
             comment, add it to gate_gpu_kernels_match_the_oracle in this file, and only \
             then relax this assertion for that one module."
        );
        assert!(
            picked.is_available(),
            "{name} reported an unavailable backend {picked:?}"
        );
        assert_eq!(
            picked == ComputeBackend::CpuMulti,
            cfg!(feature = "parallel"),
            "{name} should pick CpuMulti above its floor exactly when the parallel \
             feature is on, and Serial otherwise"
        );
    }

    // The field module's dispatcher is a predicate rather than a backend.
    assert_eq!(
        outram_foam_basic_lib::fields::should_parallelise(ComputeBackend::Gpu, big),
        cfg!(feature = "parallel"),
        "fields::should_parallelise must treat a Gpu request as the best CPU path"
    );

    // The crate-wide policy may legitimately hand out `Gpu`; no kernel honours
    // it. Record the asymmetry explicitly.
    let policy = select_backend(GPU_MIN_WORK_ITEMS);
    println!(
        "[gpu-degradation] compute::select_backend({GPU_MIN_WORK_ITEMS}) -> {} \
         (adapter present = {})",
        policy.label(),
        gpu_adapter_present()
    );
    if policy == ComputeBackend::Gpu {
        println!(
            "[gpu-degradation] NOTE: the crate-wide policy selected Gpu, yet every kernel \
             module re-resolves it to a CPU path. A caller passing select_backend()'s answer \
             straight into a kernel therefore runs on the CPU. That is documented behaviour, \
             not a defect, but it is worth knowing."
        );
    }
    assert!(
        policy.is_available(),
        "select_backend must only return a runnable backend"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 12  Gate 4 — the size floors, as seen from outside the crate
// ─────────────────────────────────────────────────────────────────────────────

/// **V&V — every module's measured dispatch floor, pinned from the public API.**
///
/// **Methodology.** For each of the seven measured floors, ask the module's own
/// dispatch-policy function what it would do at `floor - 1` and at `floor`.
///
/// **Pass criterion.** `floor - 1` must give [`ComputeBackend::Serial`] on every
/// build; `floor` must give [`ComputeBackend::CpuMulti`] exactly when the
/// `parallel` feature is on.
///
/// **Why this belongs in the cross-cutting gate.** The seven floors span
/// **16 / 32 / 256 / 256 / 4 096 / 131 072 / 262 144** — a 16 384x spread — and
/// they are the *only* thing standing between a caller and the measured `0.05x`
/// regime (`ldu::axpy` on 4 096 elements at 4 threads). They are enforced inside
/// the public entry points, so an external caller **cannot** run the parallel
/// path below a floor even by naming `CpuMulti` explicitly. That is a real and
/// useful property, and it is asserted here so it cannot be weakened by accident.
///
/// It also bounds what any external benchmark can measure: the crossover
/// benchmarks in this file can only observe sizes at or above each floor, because
/// below them both backends run the identical serial code. See
/// [`crossover_benchmark`] for what follows from that.
///
/// **Result (2026-08-13).** Passes under both feature settings.
#[test]
fn gate_dispatch_floors_are_enforced() {
    struct Floor {
        name: &'static str,
        value: usize,
        below: ComputeBackend,
        at: ComputeBackend,
    }

    let floors = [
        Floor {
            name: "ode::ODE_ENSEMBLE_MIN_LANES",
            value: op::ODE_ENSEMBLE_MIN_LANES,
            below: op::ensemble_backend_for(
                ComputeBackend::CpuMulti,
                op::ODE_ENSEMBLE_MIN_LANES - 1,
            ),
            at: op::ensemble_backend_for(ComputeBackend::CpuMulti, op::ODE_ENSEMBLE_MIN_LANES),
        },
        Floor {
            name: "ode::QUADRATURE_MIN_INTERVALS",
            value: op::QUADRATURE_MIN_INTERVALS,
            below: op::quadrature_backend_for(
                ComputeBackend::CpuMulti,
                op::QUADRATURE_MIN_INTERVALS - 1,
            ),
            at: op::quadrature_backend_for(ComputeBackend::CpuMulti, op::QUADRATURE_MIN_INTERVALS),
        },
        Floor {
            name: "math::ROOT_BATCH_MIN_PROBLEMS",
            value: mp::ROOT_BATCH_MIN_PROBLEMS,
            below: mp::root_batch_backend_for(
                ComputeBackend::CpuMulti,
                mp::ROOT_BATCH_MIN_PROBLEMS - 1,
            ),
            at: mp::root_batch_backend_for(ComputeBackend::CpuMulti, mp::ROOT_BATCH_MIN_PROBLEMS),
        },
        Floor {
            name: "minimise::MINIMISE_BATCH_MIN_PROBLEMS",
            value: mm::MINIMISE_BATCH_MIN_PROBLEMS,
            below: mm::minimise_backend_for(
                ComputeBackend::CpuMulti,
                mm::MINIMISE_BATCH_MIN_PROBLEMS - 1,
            ),
            at: mm::minimise_backend_for(ComputeBackend::CpuMulti, mm::MINIMISE_BATCH_MIN_PROBLEMS),
        },
        Floor {
            name: "math::POLY_ROOTS_MIN_EQUATIONS",
            value: mp::POLY_ROOTS_MIN_EQUATIONS,
            below: mp::poly_roots_backend_for(
                ComputeBackend::CpuMulti,
                mp::POLY_ROOTS_MIN_EQUATIONS - 1,
            ),
            at: mp::poly_roots_backend_for(ComputeBackend::CpuMulti, mp::POLY_ROOTS_MIN_EQUATIONS),
        },
        Floor {
            name: "ldu::SPMV_MIN_CELLS",
            value: lp::SPMV_MIN_CELLS,
            below: lp::spmv_backend_for(ComputeBackend::CpuMulti, lp::SPMV_MIN_CELLS - 1),
            at: lp::spmv_backend_for(ComputeBackend::CpuMulti, lp::SPMV_MIN_CELLS),
        },
        Floor {
            name: "ldu::VECOP_MIN_ELEMENTS",
            value: lp::VECOP_MIN_ELEMENTS,
            below: lp::vecop_backend_for(ComputeBackend::CpuMulti, lp::VECOP_MIN_ELEMENTS - 1),
            at: lp::vecop_backend_for(ComputeBackend::CpuMulti, lp::VECOP_MIN_ELEMENTS),
        },
    ];

    for f in &floors {
        println!(
            "[floors] {:<38} floor={:>7}  below -> {:<9}  at -> {}",
            f.name,
            f.value,
            f.below.label(),
            f.at.label()
        );
        assert_eq!(
            f.below,
            ComputeBackend::Serial,
            "{}: a CpuMulti request one item below the floor must run serially",
            f.name
        );
        assert_eq!(
            f.at == ComputeBackend::CpuMulti,
            cfg!(feature = "parallel"),
            "{}: at the floor, CpuMulti must be chosen exactly when the parallel \
             feature is compiled in",
            f.name
        );
    }

    // The field crossover is a function rather than a constant, since its
    // module documents an override of the crate-wide placeholder.
    let fc = outram_foam_basic_lib::fields::field_parallel_crossover();
    println!("[floors] fields::field_parallel_crossover()   floor={fc:>7}");
    assert!(!outram_foam_basic_lib::fields::should_parallelise(
        ComputeBackend::CpuMulti,
        fc - 1
    ));
    assert_eq!(
        outram_foam_basic_lib::fields::should_parallelise(ComputeBackend::CpuMulti, fc),
        cfg!(feature = "parallel")
    );

    // The spread is the headline finding of this epic; assert it has not been
    // quietly collapsed back onto one number.
    let spread = lp::VECOP_MIN_ELEMENTS as f64 / op::ODE_ENSEMBLE_MIN_LANES as f64;
    println!("[floors] floor spread (largest / smallest) = {spread:.0}x");
    assert!(
        spread > 1000.0,
        "the measured floors used to span 16384x; if they have converged onto one \
         value, the measurements behind them need re-checking, not the constant"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 13  Gate 5 — thread-count invariance, across processes
// ─────────────────────────────────────────────────────────────────────────────

/// Environment variable naming the file a child process writes its digests to.
const CHILD_OUT_ENV: &str = "OUTRAM_HYBRID_PARITY_DIGEST_OUT";

/// The child half of [`gate_thread_count_invariance`].
///
/// Does nothing at all unless [`CHILD_OUT_ENV`] is set, so running the ignored
/// tests directly is harmless. When it *is* set, it runs the whole kernel
/// inventory on both backends and writes one `backend<TAB>kernel<TAB>digest` line
/// per kernel to the named path.
///
/// It is `#[ignore]`d because it is a subroutine of another test, not a test.
#[test]
#[ignore = "child process of gate_thread_count_invariance; does nothing standalone"]
fn digest_child() {
    let Ok(path) = std::env::var(CHILD_OUT_ENV) else {
        println!("digest_child: {CHILD_OUT_ENV} not set — nothing to do");
        return;
    };

    let mut text = String::new();
    for backend in [ComputeBackend::Serial, ComputeBackend::CpuMulti] {
        for k in run_all_kernels(backend) {
            let mut d = Digest::new();
            d.values(&k.values);
            text.push_str(&format!(
                "{}\t{}\t{:016x}\n",
                backend.label(),
                k.name,
                d.finish()
            ));
        }
    }
    std::fs::write(&path, text).expect("digest_child could not write its output file");
}

/// **V&V — the same input at 1, 2, 4 and 8 rayon workers must give identical
/// output.**
///
/// **Methodology.** rayon's global pool is sized once per process from
/// `RAYON_NUM_THREADS`, and rayon is an optional dependency of the *library*
/// rather than a dev-dependency, so an integration test can neither build a pool
/// nor resize one. This test therefore re-executes **its own test binary**
/// (`std::env::current_exe()`) four times as a child process, once per worker
/// count, with `RAYON_NUM_THREADS` set and [`CHILD_OUT_ENV`] pointing at a
/// scratch file; the child runs [`digest_child`], which digests every kernel's
/// full output on both backends and writes `backend/kernel/digest` lines. The
/// parent then requires all four children to agree, kernel by kernel.
///
/// A digest rather than the values themselves because only bytes cross a process
/// boundary; within one process the other gates compare element by element so a
/// failure names the offending element.
///
/// **Pass criterion.** For every kernel: the `cpu-multi` digest is identical at
/// 1, 2, 4 and 8 workers, and the `serial` digest is identical everywhere
/// (a sanity check that the input really is deterministic across processes).
/// Additionally, for every kernel declared [`ParityClass::Bitwise`], the
/// `cpu-multi` digest equals the `serial` digest — the same claim gate 1 makes,
/// re-checked here through a completely different comparison path.
///
/// **Why this is structural rather than incidental.** The four modules that claim
/// it get it from design decisions recorded in their own docs: fixed-chunk
/// reductions combined in index order (`fields`, `ldu_matrix`), one independent
/// problem per lane with no cross-lane arithmetic (`math::parallel`,
/// `math::minimise`), a per-lane clone of the stepper prototype (`ode`
/// ensembles), and parallelism over lanes but never within a lane (quadrature).
/// This gate checks the property, not the reasoning.
///
/// **Result (2026-08-13, 4 logical cores, release).** Passes under the default
/// build (where `RAYON_NUM_THREADS` is inert, so the four runs are trivially
/// identical — stated, not hidden) and under `--features parallel`, where 8
/// workers on 4 cores is a deliberate oversubscription.
///
/// **Wall clock (measured 2026-08-13, 4 logical cores, load verdict MODERATE).**
/// 2.08 s under `--features parallel`, 2.54 s in the default build — four child
/// processes, each running the full 47-kernel inventory on two backends.
#[test]
fn gate_thread_count_invariance() {
    let exe = std::env::current_exe().expect("test binary path");
    let dir = std::env::temp_dir();
    let worker_counts = [1_usize, 2, 4, 8];

    // Kernel name -> parity class, so the parent can apply the bitwise check to
    // the right subset without re-running the kernels itself.
    let classes: Vec<(String, ParityClass)> = run_all_kernels(ComputeBackend::Serial)
        .into_iter()
        .map(|k| (k.name, k.class))
        .collect();

    let mut per_workers: Vec<(usize, Vec<(String, String, u64)>)> = Vec::new();

    for workers in worker_counts {
        let out_path = dir.join(format!(
            "outram-hybrid-parity-{}-{workers}.tsv",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out_path);

        let output = Command::new(&exe)
            .args([
                "--exact",
                "digest_child",
                "--ignored",
                "--test-threads",
                "1",
            ])
            .env("RAYON_NUM_THREADS", workers.to_string())
            .env(CHILD_OUT_ENV, &out_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("could not spawn the child test process");

        assert!(
            output.status.success(),
            "child with RAYON_NUM_THREADS={workers} failed: {}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );

        let text = std::fs::read_to_string(&out_path).unwrap_or_else(|e| {
            panic!(
                "child with RAYON_NUM_THREADS={workers} wrote no digest file ({e}); \
                 stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let _ = std::fs::remove_file(&out_path);

        let rows: Vec<(String, String, u64)> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let mut it = l.split('\t');
                let backend = it.next().expect("backend column").to_string();
                let kernel = it.next().expect("kernel column").to_string();
                let digest = u64::from_str_radix(it.next().expect("digest column"), 16)
                    .expect("digest is hex");
                (backend, kernel, digest)
            })
            .collect();

        assert!(
            !rows.is_empty(),
            "child with RAYON_NUM_THREADS={workers} produced an empty digest file"
        );
        println!(
            "[thread-invariance] {workers} worker(s): {} digests",
            rows.len()
        );
        per_workers.push((workers, rows));
    }

    // Every worker count must produce byte-identical digests.
    let (ref_workers, ref_rows) = &per_workers[0];
    for (workers, rows) in per_workers.iter().skip(1) {
        assert_eq!(
            rows.len(),
            ref_rows.len(),
            "digest count differs between {ref_workers} and {workers} workers"
        );
        for (a, b) in ref_rows.iter().zip(rows.iter()) {
            assert_eq!(a.0, b.0, "backend column misaligned between child runs");
            assert_eq!(a.1, b.1, "kernel column misaligned between child runs");
            assert_eq!(
                a.2, b.2,
                "{}::{} differs between {ref_workers} and {workers} rayon workers \
                 (digest {:016x} vs {:016x}). Thread-count invariance is a hard \
                 property of every kernel in this crate; a work-stealing reduction \
                 or a shared scratch buffer would produce exactly this failure.",
                a.0, a.1, a.2, b.2
            );
        }
    }

    // Cross-check gate 1 through a different path: bitwise kernels must have
    // equal serial and cpu-multi digests.
    let mut checked = 0_usize;
    for (name, class) in &classes {
        if *class != ParityClass::Bitwise {
            continue;
        }
        let serial = ref_rows
            .iter()
            .find(|(b, k, _)| b == "serial" && k == name)
            .map(|(_, _, d)| *d);
        let multi = ref_rows
            .iter()
            .find(|(b, k, _)| b == "cpu-multi" && k == name)
            .map(|(_, _, d)| *d);
        match (serial, multi) {
            (Some(s), Some(m)) => {
                assert_eq!(
                    s, m,
                    "{name} is declared BITWISE but its serial and cpu-multi digests differ"
                );
                checked += 1;
            }
            _ => panic!("{name} missing from the child digest file"),
        }
    }
    println!(
        "[thread-invariance] {checked} bitwise kernels re-checked serial == cpu-multi \
         via digests, at {} worker counts",
        worker_counts.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 14  Machine-load report
// ─────────────────────────────────────────────────────────────────────────────

/// **Records the load conditions of this host, so every number this file prints
/// carries its precondition.**
///
/// **Methodology.** Read `/proc/loadavg` and
/// [`std::thread::available_parallelism`], classify with [`LoadVerdict`], print.
/// Then time one pass of the whole kernel inventory on each backend and print the
/// load again on the far side.
///
/// **Why this exists at all.** Every crossover figure previously filed on bead
/// `op-yvj.4.7` was measured without controlling or recording machine load, and
/// one agent's contended runs put batched quadrature at `0.98-1.02x` where an
/// idle machine gives `3.1-4.0x`. A threshold measured on an idle box can lose
/// its whole benefit on a busy one, so the load is now part of every
/// measurement's record rather than an unstated precondition.
///
/// **Pass criterion.** This test asserts only that the probe works and that the
/// harness runs; it deliberately does **not** fail on a busy machine, because a
/// contended CI host is a normal condition and turning it into a red test would
/// train people to ignore it. It prints the verdict instead.
///
/// **Result (2026-08-13).** Passes; the printed verdict is the run's record.
#[test]
fn machine_load_report() {
    let before = MachineLoad::probe();

    let t0 = Instant::now();
    let serial = run_all_kernels(ComputeBackend::Serial);
    let serial_dt = t0.elapsed();

    let t1 = Instant::now();
    let multi = run_all_kernels(ComputeBackend::CpuMulti);
    let multi_dt = t1.elapsed();

    let after = MachineLoad::probe();

    println!("[load-report] kernels in inventory: {}", serial.len());
    println!(
        "[load-report] whole-inventory wall clock: serial {:.1} ms, cpu-multi {:.1} ms \
         ({:.2}x) — an inventory, NOT a benchmark: it mixes sizes and kernels and \
         must not be quoted as a speed-up",
        serial_dt.as_secs_f64() * 1e3,
        multi_dt.as_secs_f64() * 1e3,
        serial_dt.as_secs_f64() / multi_dt.as_secs_f64()
    );
    print_load_bracket("load-report", before, after);

    assert_eq!(serial.len(), multi.len());
    assert!(before.cores >= 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// § 15  Crossover benchmarks (#[ignore]d — slow)
// ─────────────────────────────────────────────────────────────────────────────

/// Best-of-`reps` wall clock for one closure, in microseconds.
fn best_of<F: FnMut()>(reps: usize, mut f: F) -> f64 {
    // Two untimed warm-ups: the first touch of a rayon pool builds it, and the
    // first touch of a large buffer takes the page faults.
    f();
    f();
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1e6);
    }
    best
}

/// Print one benchmark row.
fn row(family: &str, kernel: &str, n: usize, serial_us: f64, multi_us: f64, load: MachineLoad) {
    println!(
        "{family:<14} {kernel:<26} n={n:>9}  serial {serial_us:>11.2} us  \
         cpu-multi {multi_us:>11.2} us  speedup {:>6.2}x   [{}]",
        serial_us / multi_us,
        load.verdict().label()
    );
}

/// **Crossover benchmark — every kernel family, at and above its own floor,
/// with machine load recorded on every row.**
///
/// **Methodology.** For each family, time one call on
/// [`ComputeBackend::Serial`] and one on [`ComputeBackend::CpuMulti`], best of 5
/// after 2 warm-ups, at the family's own documented floor and at two larger
/// sizes. [`MachineLoad`] is probed before and after the whole sweep and its
/// verdict is printed on **every row**, so no figure here can be transcribed
/// without its precondition.
///
/// **What this benchmark can and cannot measure — read before quoting it.**
/// The public API enforces each module's size floor (see
/// [`gate_dispatch_floors_are_enforced`]), so **below a floor both backends run
/// the identical serial code and the measured speed-up is 1.00x by construction,
/// carrying no information.** An integration test therefore **cannot re-derive a
/// crossover from below**; only the in-module benchmarks can, because only they
/// can reach the `pub(crate)` `*_min` entry points.
///
/// What it *can* do — and what the previously filed per-module measurements did
/// not — is check each floor **from the outside**: if a floor were set too low,
/// the row at that floor would show a speed-up below 1.00x. That is the useful
/// external question, and it is the one this benchmark answers.
///
/// **On the "kernel character" hypothesis** (compute-bound families crossing at
/// ~256, memory-bound at ~131 072+): this benchmark groups the families by that
/// hypothesis and prints the speed-up each achieves at its own floor and one to
/// two decades above. It **cannot test the hypothesis directly**, because doing
/// so would require running a memory-bound kernel at 256 elements on the
/// parallel path, which the public API forbids. Treat the grouping in the output
/// as a presentation of the existing hypothesis, not as an independent
/// confirmation of it.
///
/// **Pass criterion.** None — this is a measurement, and it asserts nothing about
/// timing. A benchmark that fails on a slow machine teaches people to disable
/// benchmarks. It asserts only that every configuration ran.
///
/// # Results — measured 2026-08-13, transcribed from this test's own output
///
/// **Hardware and conditions.** One virtualised x86-64 sandbox,
/// `available_parallelism() = 4`, `--release --features parallel`, two
/// independent runs. **Load verdict on every row of both runs:
/// `MODERATE (timings distorted)`** — `loadavg` 1.93/1.24/1.17 (run 1) and
/// 2.03/1.33/1.20 (run 2), i.e. `load1-per-core` 0.48 and 0.51. The host could
/// not be brought to `IDLE`: a `bn daemon run` process held a steady ~36 % of a
/// core throughout. **These figures are therefore indicative, not authoritative,
/// and must not be transcribed into any constant's documentation as a measured
/// crossover.** That statement is the whole reason this harness records load.
///
/// **Speed-up at each family's own floor** (`serial / cpu-multi`, best of 5,
/// run 1 | run 2):
///
/// | Family | Floor | Speed-up at the floor |
/// |---|---|---|
/// | `ode::ensemble[rkf45]` | 16 lanes | 2.68x \| 2.07x |
/// | `quadrature::g5x16` | 32 lanes | 1.93x \| 1.75x |
/// | `root::brent` | 256 problems | **1.02x** \| 1.88x |
/// | `minimise::golden` | 256 problems | 2.14x \| 2.25x |
/// | `ldu::spmv` | 4 096 cells | **0.82x** \| **1.00x** |
/// | `fields::add` | 131 072 elements | 1.92x \| 2.01x |
/// | `ldu::dot` | 262 144 elements | 2.75x \| 3.12x |
///
/// **Speed-up on the plateau** (largest size measured per family): `root::brent`
/// 2.69x \| 3.72x at 16 384; `minimise::golden` 3.79x \| 3.85x at 16 384;
/// `ode::ensemble` 3.73x \| 3.70x at 4 096; `quadrature` 3.39x \| 3.89x at
/// 65 536; `ldu::spmv` 3.28x \| 3.06x at 262 144 cells; `ldu::dot` 3.64x \|
/// 3.49x and `ldu::axpy` 3.74x \| 2.73x at 2^22; `fields::add` 2.36x \| 1.88x
/// and `fields::sum` 3.96x \| 2.38x at 2^22.
///
/// **Interpretation — two findings, both about the floors.**
///
/// 1. **`ldu::SPMV_MIN_CELLS = 4 096` did not win at its own floor in either
///    run** (0.82x, then 1.00x). That is not a new defect: `SPMV_MIN_CELLS`'s
///    own documentation already records that "on a loaded machine an earlier,
///    contended run of the same benchmark put 4 096 cells at 0.72x". This is an
///    independent external reproduction of that caveat, and it says the floor is
///    calibrated for an idle machine that this host is not. Every other family
///    won at its floor in both runs.
/// 2. **The near-floor region is dominated by run-to-run noise**, exactly as the
///    per-module measurements reported: `root::brent` at 256 gave 1.02x and then
///    1.88x on byte-identical input. Only the plateau (roughly 2.5x-3.9x, i.e.
///    62-97 % of the 4-core ideal) is firmly resolved. No floor should be moved
///    on the strength of these two runs.
///
/// **On the kernel-character hypothesis:** the grouping is *presented* above, not
/// *tested*. Both groups reach a similar 2.5x-3.9x plateau, and every family bar
/// SPMV is at least break-even at its own floor — consistent with the hypothesis
/// but not evidence for it, because confirming it would require running a
/// memory-bound kernel on the parallel path at 256 elements, which the public API
/// forbids. See the paragraph above on what an integration test can measure.
///
/// **Wall clock (measured).** 1.80 s per run under `--features parallel` on this
/// host — dominated by the 65 536-lane quadrature sweep (86 ms serial per call)
/// and the 4 096-lane ODE ensemble (17.9 ms serial per call).
///
/// **Run it with:**
/// ```text
/// cargo test -p outram-foam-basic-lib --release --features parallel \
///     --test hybrid_parity -- --ignored --nocapture --exact crossover_benchmark
/// ```
#[test]
#[ignore = "crossover benchmark: measured 1.8 s, and meaningless without --nocapture"]
fn crossover_benchmark() {
    let before = MachineLoad::probe();
    println!("=== crossover benchmark, {} ===", before.line());
    println!(
        "parallel feature = {}, CpuMulti resolves to {}",
        cfg!(feature = "parallel"),
        ComputeBackend::CpuMulti.resolve().label()
    );
    if !cfg!(feature = "parallel") {
        println!(
            "*** parallel is OFF: every 'cpu-multi' column below is the SERIAL path. \
             Speed-ups of ~1.00x here mean nothing. Re-run with --features parallel. ***"
        );
    }
    println!();
    println!("--- COMPUTE-BOUND families (hypothesis: crossover ~256) ---");

    const REPS: usize = 5;

    // Root finding, at the floor and above.
    for n in [mp::ROOT_BATCH_MIN_PROBLEMS, 1_024, 16_384] {
        let mut rng = Xorshift::new(0x11_22_33);
        let targets: Vec<f64> = (0..n).map(|_| rng.in_range(0.2, 40.0)).collect();
        let problems: Vec<mp::RootProblem> =
            (0..n).map(|_| mp::RootProblem::new(0.0, 8.0)).collect();
        let s = mp::RootSettings::default();
        let f = |i: usize, x: f64| x * x - targets[i];
        let ser = best_of(REPS, || {
            std::hint::black_box(mp::solve_bracketed_batch(
                &problems,
                mp::RootMethod::Brent,
                s,
                ComputeBackend::Serial,
                f,
            ));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(mp::solve_bracketed_batch(
                &problems,
                mp::RootMethod::Brent,
                s,
                ComputeBackend::CpuMulti,
                f,
            ));
        });
        row(
            "compute-bound",
            "root::brent",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    // Minimisation.
    for n in [mm::MINIMISE_BATCH_MIN_PROBLEMS, 1_024, 16_384] {
        let mut rng = Xorshift::new(0x44_55_66);
        let centres: Vec<f64> = (0..n).map(|_| rng.in_range(-2.0, 2.0)).collect();
        let problems: Vec<mm::MinProblem> =
            (0..n).map(|_| mm::MinProblem::new(-5.0, 5.0)).collect();
        let s = mm::MinSettings::default();
        let f = |i: usize, x: f64| {
            let d = x - centres[i];
            1.0 + d * d
        };
        let ser = best_of(REPS, || {
            std::hint::black_box(mm::golden_section_batch(
                &problems,
                mm::Sense::Minimise,
                s,
                ComputeBackend::Serial,
                f,
            ));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(mm::golden_section_batch(
                &problems,
                mm::Sense::Minimise,
                s,
                ComputeBackend::CpuMulti,
                f,
            ));
        });
        row(
            "compute-bound",
            "minimise::golden",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    // ODE ensembles.
    for n in [op::ODE_ENSEMBLE_MIN_LANES, 256, 4_096] {
        let lanes = ode_lanes(n);
        let solver = OdeSolver::rkf45(1, 1e-9, 1e-9);
        let ser = best_of(REPS, || {
            std::hint::black_box(op::integrate_ensemble(
                &lanes,
                &solver,
                ComputeBackend::Serial,
            ));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(op::integrate_ensemble(
                &lanes,
                &solver,
                ComputeBackend::CpuMulti,
            ));
        });
        row(
            "compute-bound",
            "ode::ensemble[rkf45]",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    // Fixed-rule quadrature.
    for n in [op::QUADRATURE_MIN_INTERVALS, 1_024, 65_536] {
        let mut rng = Xorshift::new(0x77_88_99);
        let intervals: Vec<op::QuadratureInterval> = (0..n)
            .map(|_| {
                let a = rng.in_range(0.0, 1.0);
                op::QuadratureInterval::new(a, a + rng.in_range(0.5, 3.0))
            })
            .collect();
        let rule = op::QuadratureRule::GaussLegendre {
            order: op::GaussOrder::G5,
            panels: 16,
        };
        let f = |i: usize, x: f64| (-x).exp() * (x * (1.0 + (i % 7) as f64)).sin();
        let ser = best_of(REPS, || {
            std::hint::black_box(op::quadrature_batch(
                &intervals,
                rule,
                ComputeBackend::Serial,
                f,
            ));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(op::quadrature_batch(
                &intervals,
                rule,
                ComputeBackend::CpuMulti,
                f,
            ));
        });
        row(
            "compute-bound",
            "quadrature::g5x16",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    println!();
    println!("--- MEMORY-BOUND families (hypothesis: crossover >= 4096, mostly >= 131072) ---");

    // Sparse matrix-vector product.
    for nx in [16_usize, 32, 64] {
        let m = Arc::new(random_matrix(nx, 0xabcd));
        let ldu = lp::HybridLdu::new(Arc::clone(&m));
        let mut rng = Xorshift::new(0xdcba);
        let x = rng.vector(m.n_cells);
        let mut y = vec![0.0; m.n_cells];
        let ser = best_of(REPS, || ldu.spmv_into(&x, &mut y, ComputeBackend::Serial));
        let par = best_of(REPS, || ldu.spmv_into(&x, &mut y, ComputeBackend::CpuMulti));
        row(
            "memory-bound",
            "ldu::spmv",
            m.n_cells,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    // LDU vector operations.
    for n in [lp::VECOP_MIN_ELEMENTS, 1 << 20, 1 << 22] {
        let mut rng = Xorshift::new(0x1357);
        let a = rng.vector(n);
        let b = rng.vector(n);
        let ser = best_of(REPS, || {
            std::hint::black_box(lp::dot(&a, &b, ComputeBackend::Serial));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(lp::dot(&a, &b, ComputeBackend::CpuMulti));
        });
        row(
            "memory-bound",
            "ldu::dot",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );

        let mut y = b.clone();
        let ser = best_of(REPS, || {
            lp::axpy(0.5, &a, &mut y, ComputeBackend::Serial);
        });
        let par = best_of(REPS, || {
            lp::axpy(0.5, &a, &mut y, ComputeBackend::CpuMulti);
        });
        row(
            "memory-bound",
            "ldu::axpy",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    // Field algebra.
    for n in [
        outram_foam_basic_lib::fields::field_parallel_crossover(),
        1 << 20,
        1 << 22,
    ] {
        let a = sample_field(n);
        let b = sample_field_b(n);
        let ser = best_of(REPS, || {
            std::hint::black_box(fp::add(ComputeBackend::Serial, &a, &b));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(fp::add(ComputeBackend::CpuMulti, &a, &b));
        });
        row(
            "memory-bound",
            "fields::add",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );

        let ser = best_of(REPS, || {
            std::hint::black_box(fp::sum(ComputeBackend::Serial, &a));
        });
        let par = best_of(REPS, || {
            std::hint::black_box(fp::sum(ComputeBackend::CpuMulti, &a));
        });
        row(
            "memory-bound",
            "fields::sum",
            n,
            ser,
            par,
            MachineLoad::probe(),
        );
    }

    let after = MachineLoad::probe();
    println!();
    print_load_bracket("crossover", before, after);
    println!(
        "REMINDER: rows below a family's floor cannot exist here — the public API runs \
         the serial path on both backends there. This benchmark validates the floors \
         from outside; it does not re-derive them."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Gate 3b — the parity gate for the GPU kernels that actually exist.
// ─────────────────────────────────────────────────────────────────────────────

/// Run every real WGSL kernel against the serial `f64` oracle at the tolerance
/// its doc comment states.
///
/// # Methodology
///
/// A 1-D Laplacian of 2^20 cells with **non-dyadic** coefficients
/// (`2 + i/sqrt(7)`, `-1 - i/sqrt(3)`, `-1 - i/sqrt(11)`) and a non-dyadic
/// operand (`sin(i*pi/1000)*e + i/3`). The choice is deliberate: dyadic values
/// such as `0.25` are exactly representable in `f32`, so a gate built from them
/// reports a deviation of exactly zero and proves nothing. An early version of
/// `examples/hybrid_gpu_report.rs` did exactly that and had to be fixed.
///
/// Each kernel is compared on **relative** deviation, `|got - want| /
/// max(|want|, 1)`, against the tolerance below. The tolerances are set one
/// order of magnitude above the measured value so that ordinary
/// adapter-to-adapter variation does not fail the gate, while a real
/// regression (a wrong index, a dropped term) moves the error by orders of
/// magnitude and does.
///
/// # Results — measured 2026-09-03
///
/// Hardware: Mesa Intel(R) Graphics (RPL-S), integrated, **OpenGL** backend.
/// (The machine also has an RTX A5000, but its kernel driver was not loaded,
/// so no Vulkan adapter was available. The adapter-scoring in
/// `compute::gpu::adapter_score` would prefer the discrete card where the
/// driver is present.)
///
/// | Kernel | n | max rel | RMS rel | gate |
/// |---|---:|---:|---:|---:|
/// | `spmv` | 1048576 | 2.81e-7 | 6.43e-8 | 1e-5 |
/// | `axpy` | 1048576 | 1.11e-7 | 3.70e-8 | 1e-6 |
/// | `scale` | 1048576 | 1.04e-7 | 3.76e-8 | 1e-6 |
/// | `dot` | 1048576 | 1.04e-11 | — | 1e-6 |
/// | `norm_l1` | 1048576 | 2.84e-10 | — | 1e-6 |
///
/// Interpretation: the elementwise kernels sit right at the `f32` epsilon
/// floor (~1.2e-7), which is the expected and irreducible cost of WGSL having
/// no `f64`. The two reductions come out far better because their per-lane
/// rounding errors partly cancel across the sum — that is **not** a general
/// guarantee and must not be read as one; a cancellation-heavy input (a
/// near-converged residual) would do much worse. This is why the reductions
/// are off auto-select despite these flattering numbers.
///
/// No human V&V is claimed by this gate.
#[test]
#[cfg(all(feature = "gpu", not(target_os = "android")))]
fn gate_gpu_kernels_match_the_oracle() {
    use outram_foam_basic_lib::compute::gpu;
    use outram_foam_basic_lib::ldu_matrix::ldu_matrix::LduMatrix;
    use outram_foam_basic_lib::ldu_matrix::parallel::gpu as kgpu;
    use outram_foam_basic_lib::ldu_matrix::parallel::{HybridLdu, LduTopology};

    let Some(ctx) = gpu::context() else {
        println!(
            "[gpu-parity] no GPU adapter on this machine — gate skipped. \
             This is a valid outcome, not a pass."
        );
        return;
    };
    println!("[gpu-parity] adapter: {}", ctx.adapter_label());

    let n = 1 << 20;
    let owner: Vec<usize> = (0..n - 1).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    let mut m = LduMatrix::new(n, owner, neighbour);
    for (i, d) in m.diag.iter_mut().enumerate() {
        *d = 2.0 + (i as f64) / 7.0_f64.sqrt();
    }
    for (i, v) in m.lower.iter_mut().enumerate() {
        *v = -1.0 - (i as f64) / 3.0_f64.sqrt();
    }
    for (i, v) in m.upper.iter_mut().enumerate() {
        *v = -1.0 - (i as f64) / 11.0_f64.sqrt();
    }
    let topo = LduTopology::from_matrix(&m);
    let x: Vec<f64> = (0..n)
        .map(|i| {
            let t = (i as f64) * std::f64::consts::PI / 1000.0;
            t.sin() * std::f64::consts::E + (i as f64) / 3.0
        })
        .collect();

    let rel = |g: f64, w: f64| (g - w).abs() / w.abs().max(1.0);
    let worst = |got: &[f64], want: &[f64]| {
        got.iter()
            .zip(want)
            .map(|(g, w)| rel(*g, *w))
            .fold(0.0_f64, f64::max)
    };

    // spmv
    let hybrid = HybridLdu::new(Arc::new(m.clone()));
    let want = hybrid.spmv(&x, ComputeBackend::Serial);
    let mut got = vec![0.0; n];
    kgpu::spmv_into(&m, &topo, &x, &mut got).expect("GPU spmv on a present adapter");
    let d = worst(&got, &want);
    println!("[gpu-parity] spmv    max rel = {d:.3e} (gate 1e-5)");
    assert!(d < 1e-5, "spmv deviated {d:e}, above the documented 1e-5 gate");

    // axpy
    let mut want_y = x.clone();
    lp::axpy(2.5, &x, &mut want_y, ComputeBackend::Serial);
    let mut got_y = x.clone();
    kgpu::axpy(2.5, &x, &mut got_y).expect("GPU axpy");
    let d = worst(&got_y, &want_y);
    println!("[gpu-parity] axpy    max rel = {d:.3e} (gate 1e-6)");
    assert!(d < 1e-6, "axpy deviated {d:e}");

    // scale
    let mut want_s = x.clone();
    lp::scale(-1.5, &mut want_s, ComputeBackend::Serial);
    let mut got_s = x.clone();
    kgpu::scale(-1.5, &mut got_s).expect("GPU scale");
    let d = worst(&got_s, &want_s);
    println!("[gpu-parity] scale   max rel = {d:.3e} (gate 1e-6)");
    assert!(d < 1e-6, "scale deviated {d:e}");

    // dot
    let b: Vec<f64> = x.iter().map(|v| v * std::f64::consts::LN_2).collect();
    let want_d = lp::dot(&x, &b, ComputeBackend::Serial);
    let got_d = kgpu::dot(&x, &b).expect("GPU dot");
    let d = rel(got_d, want_d);
    println!("[gpu-parity] dot     rel = {d:.3e} (gate 1e-6)");
    assert!(d < 1e-6, "dot deviated {d:e}");

    // norm_l1
    let want_l1 = lp::norm_l1(&x, ComputeBackend::Serial);
    let got_l1 = kgpu::norm_l1(&x).expect("GPU norm_l1");
    let d = rel(got_l1, want_l1);
    println!("[gpu-parity] norm_l1 rel = {d:.3e} (gate 1e-6)");
    assert!(d < 1e-6, "norm_l1 deviated {d:e}");
}
