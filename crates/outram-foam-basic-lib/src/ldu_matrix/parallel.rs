// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! The sparse LDU matrix-vector product and its companion vector operations, on
//! the hybrid execution backend — the hot path of every implicit finite-volume
//! solve.
//!
//! A Krylov solver (conjugate gradient, BiCGStab, GMRES) spends most of its wall
//! clock inside a handful of operations. This module provides each of them as
//! **one** public entry point that takes a
//! [`ComputeBackend`](crate::compute::ComputeBackend) parameter, in the shape the
//! hybrid-backend epic mandates: dispatch, not a `foo_parallel()` sibling beside
//! `foo()`.
//!
//! | Operation | Cost | Entry point |
//! |---|---|---|
//! | sparse product `y = A x` | `O(n_cells + n_faces)` | [`HybridLdu::spmv`] / [`HybridLdu::spmv_into`] |
//! | residual `r = b - A x` | `O(n_cells + n_faces)` | [`HybridLdu::residual`] / [`HybridLdu::residual_into`] |
//! | scaled residual norm | `O(n_cells + n_faces)` | [`HybridLdu::normalised_residual`] |
//! | diagonal reciprocal `1 / diag` | `O(n_cells)` | [`HybridLdu::diagonal_reciprocal`] |
//! | inner product `a . b` | `O(n)` | [`dot`] |
//! | `y := alpha x + y` | `O(n)` | [`axpy`] |
//! | `sqrt(sum x_i^2)` / `sum abs(x_i)` | `O(n)` | [`norm_l2`] / [`norm_l1`] |
//!
//! # The correctness problem this module solves
//!
//! [`LduMatrix`] stores its off-diagonal coefficients **per internal face**, so
//! the textbook product is a face-based *scatter*:
//!
//! ```text
//! for each face f:  y[owner[f]]     += upper[f] * x[neighbour[f]]
//!                   y[neighbour[f]] += lower[f] * x[owner[f]]
//! ```
//!
//! Two faces generally share a cell, so parallelising that face loop directly is
//! a **data race**: two threads read-modify-write the same `y[c]`. Where such a
//! thing can be written at all (atomics, unchecked split borrows) it produces
//! silently wrong, run-varying answers.
//!
//! This module uses the **cell-gather reformulation**. A one-off index build
//! ([`LduTopology`]) inverts the face addressing into a per-cell list of incident
//! faces, turning the product into
//!
//! ```text
//! for each cell c:  y[c] = diag[c] * x[c]
//!                        + sum over faces f incident on c of
//!                              (c is owner ? upper[f] * x[neighbour[f]]
//!                                          : lower[f] * x[owner[f]])
//! ```
//!
//! The loop is now over **cells**, and every output element `y[c]` is written by
//! exactly one thread. There is no race, no atomic, and no per-thread scratch
//! buffer.
//!
//! The two alternatives were considered and rejected. *Per-thread partial
//! accumulation* costs `threads * n_cells` extra memory and needs a reduction
//! pass whose association order varies with the schedule, so it is not
//! reproducible. *Face colouring* needs a graph colouring at build time, still
//! writes each cell once per colour, and its result depends on the colouring
//! produced. The cell-gather index costs one `O(n_cells + n_faces)` build that is
//! amortised over the thousands of products a solve performs, and it buys exact
//! reproducibility (below).
//!
//! # Determinism
//!
//! **Every kernel in this module returns bit-for-bit identical output on
//! [`ComputeBackend::Serial`](crate::compute::ComputeBackend::Serial) and
//! [`ComputeBackend::CpuMulti`](crate::compute::ComputeBackend::CpuMulti), at any
//! thread count, on every run.** That is stronger than the usual
//! parallel-reduction contract and it is deliberate, because
//! `ComputeBackend::Serial` is this workspace's documented deterministic oracle.
//!
//! Two separate mechanisms deliver it:
//!
//! - **Products and element-wise kernels** ([`HybridLdu::spmv`],
//!   [`HybridLdu::residual`], [`HybridLdu::diagonal_reciprocal`], [`axpy`]) are
//!   bitwise identical *also to the pre-existing serial reference*
//!   [`LduMatrix::multiply`] / [`LduMatrix::residual`]. [`LduTopology`] lists
//!   each cell's incident faces in **ascending face index**, which is exactly the
//!   order in which the serial scatter reaches that cell, so each `y[c]`
//!   accumulates the same additions in the same sequence.
//! - **Reductions** ([`dot`], [`norm_l1`], [`norm_l2`],
//!   [`HybridLdu::normalised_residual`]) sum in fixed-size blocks of
//!   [`REDUCTION_BLOCK`] elements and then combine the block partials in
//!   ascending block order. The association is a function of the array length and
//!   [`REDUCTION_BLOCK`] alone — never of the thread count or the work-stealing
//!   schedule — so a 1-thread run agrees bit for bit with a 64-thread run.
//!
//! The one thing a reduction here is **not** is bitwise equal to a flat
//! left-to-right sum such as [`crate::krylov::vecops::dot`] or
//! [`LduMatrix::normalised_residual`]. Blocked summation reassociates, and
//! floating-point addition is not associative. That difference is small,
//! bounded, and *measured* rather than asserted — see the "Measured deviation"
//! sections on [`dot`] and [`HybridLdu::normalised_residual`]. Blocked summation
//! is in fact the more accurate of the two (it is a two-level pairwise-style
//! sum), so this is not a loss of accuracy relative to the flat reference.
//!
//! # When multi-CPU is actually faster
//!
//! Threading is not free. Below [`SPMV_MIN_CELLS`] cells (products) or
//! [`VECOP_MIN_ELEMENTS`] elements (vector operations), a `CpuMulti` request runs
//! the serial kernel on the calling thread instead. Because the two paths are
//! bitwise identical, that size dispatch changes **no** number a caller can
//! observe — only the wall clock. Both constants were measured on this
//! workspace's development machine; the tables are on the constants themselves.
//!
//! # Cargo features
//!
//! The `rayon` code paths sit behind the crate's `parallel` feature, which is
//! **off by default**. With the feature off this module still compiles and every
//! entry point still works: `ComputeBackend::CpuMulti` resolves down to
//! `ComputeBackend::Serial` via
//! [`ComputeBackend::resolve`](crate::compute::ComputeBackend::resolve) and the
//! answer is unchanged. There is no `Gpu` kernel here yet, so a `Gpu` request
//! also degrades to the best available CPU path.
//!
//! # Portability
//!
//! `rayon` is pure Rust with no system component, so everything here compiles and
//! runs on `aarch64-linux-android` / Termux exactly as on desktop. Nothing in
//! this module is target-gated.
//!
//! # Units
//!
//! All slices are dimensionless `f64` in cell order: element `c` is the value at
//! cell `c`, in whatever units the assembled equation carries. No `uom` typing is
//! applied at this layer, for the same reason [`crate::krylov::vecops`] applies
//! none: a Krylov subspace mixes residuals, search directions and solution
//! increments that share no single physical dimension. Units belong on the field
//! and equation layer that assembles the matrix, and are not stripped there.
//!
//! # Example
//!
//! ```rust
//! use std::sync::Arc;
//! use outram_foam_basic_lib::compute::ComputeBackend;
//! use outram_foam_basic_lib::ldu_matrix::LduMatrix;
//! use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
//!
//! // 3-cell symmetric tridiagonal  [[2,-1,0],[-1,2,-1],[0,-1,2]]
//! let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
//! m.diag = vec![2.0, 2.0, 2.0];
//! m.upper = vec![-1.0, -1.0];
//! m.lower = vec![-1.0, -1.0];
//! let m = Arc::new(m);
//!
//! let ldu = HybridLdu::new(Arc::clone(&m));
//! let x = vec![1.0, 1.0, 1.0];
//!
//! assert_eq!(ldu.spmv(&x, ComputeBackend::Serial), vec![1.0, 0.0, 1.0]);
//!
//! // Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
//! // the `parallel` feature is compiled in.
//! assert_eq!(
//!     ldu.spmv(&x, ComputeBackend::CpuMulti),
//!     ldu.spmv(&x, ComputeBackend::Serial),
//! );
//! ```

use std::sync::Arc;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::compute::{select_backend, ComputeBackend};
use crate::ldu_matrix::ldu_matrix::LduMatrix;

#[cfg(test)]
mod tests;

// ── Tuning constants ──────────────────────────────────────────────────────────

/// Number of cells in one block of the cell-parallel kernels.
///
/// `rayon` splits a chunked parallel iterator adaptively, so this is a *lower*
/// bound on task granularity rather than a fixed task size: it stops the
/// scheduler subdividing below a block that is too small to pay for itself. 1024
/// cells is about 8 KiB of output per block, comfortably inside an L1 data cache,
/// while still leaving hundreds of blocks on any mesh worth threading.
///
/// This constant affects wall time only. It cannot affect the value of any
/// kernel that uses it, because those kernels compute each cell independently.
///
/// # Units
///
/// A count of cells, dimensionless.
pub const CELL_BLOCK: usize = 1024;

/// Number of elements summed serially inside one block of a reduction, before
/// block partials are combined.
///
/// This constant **does** affect the last bits of every reduction in this module
/// ([`dot`], [`norm_l1`], [`norm_l2`], [`HybridLdu::normalised_residual`]),
/// because floating-point addition is not associative — but it does so
/// *reproducibly*. The summation tree is a function of the array length and this
/// constant alone, never of the thread count or the scheduler, which is exactly
/// what makes those kernels bitwise reproducible across backends and runs.
///
/// Treat it as part of the numerical contract rather than a free tuning knob:
/// changing it would perturb converged residual histories in the last few digits.
///
/// # Units
///
/// A count of vector elements, dimensionless.
pub const REDUCTION_BLOCK: usize = 1024;

/// Cell count below which a [`ComputeBackend::CpuMulti`] request runs the serial
/// product kernel instead.
///
/// # Why a threshold exists
///
/// Dispatching a `rayon` parallel iterator costs on the order of a microsecond of
/// scheduling and synchronisation. A sparse product on a small mesh finishes in
/// less than that, so threading it is a straight loss. Because the serial and
/// parallel kernels are **bitwise identical** (see the module documentation,
/// "Determinism"), dispatching on size changes no number a caller can observe.
///
/// # Measured crossover
///
/// Measured 2026-08-12 on this workspace's development machine
/// (`std::thread::available_parallelism()` = **4**, release build, `--features
/// parallel`, rayon's global pool), on a structured 7-point-stencil LDU matrix.
/// Each figure is the best of 5 timed repeats, reported as time per
/// [`HybridLdu::spmv_into`] call. Produced by the `#[ignore]`d
/// `spmv_crossover_benchmark` test in `parallel/tests.rs`, and transcribed from
/// its printed output.
///
/// | Cells | Faces | Serial | CpuMulti (4 threads) | Speed-up |
/// |---|---|---|---|---|
/// | 512 | 1 200 | 0.71 us | 15.98 us | 0.04x |
/// | 1 728 | 4 320 | 2.36 us | 18.90 us | 0.12x |
/// | 4 096 | 10 752 | 5.66 us | 20.36 us | 0.28x |
/// | 8 000 | 21 600 | 11.25 us | 24.35 us | 0.46x |
/// | 15 625 | 43 500 | 22.20 us | 30.02 us | 0.74x |
/// | 32 768 | 92 160 | 47.24 us | 40.55 us | 1.16x |
/// | 64 000 | 182 400 | 96.13 us | 63.83 us | 1.51x |
/// | 132 651 | 383 226 | 217.15 us | 118.13 us | 1.84x |
/// | 262 144 | 761 856 | 471.68 us | 224.24 us | 2.10x |
/// | 512 000 | 1 497 600 | 1005.30 us | 429.11 us | 2.34x |
///
/// The crossover — the smallest size at which multi-CPU stops losing — lies
/// between 15 625 and 32 768 cells on that 4-core machine, so this constant is
/// set to **32 768**: the first measured size that actually won, and a round
/// power of two. Setting it at the low end of the bracket would ship a value the
/// measurement does not support.
///
/// # This is not [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]
///
/// That crate-wide constant is documented as a placeholder awaiting measurement,
/// and is currently 4 096. **The measurement above says 4 096 is too low for this
/// kernel**: at 4 096 cells the parallel path was 3.6x *slower* than serial, and
/// it was still losing at 15 625 cells. This kernel therefore overrides it with
/// its own measured value, which `compute.rs` explicitly permits. A crate-wide
/// revision of `CPU_MULTI_MIN_WORK_ITEMS` is the maintainer's call and belongs
/// with bead `op-yvj.4.7`, not here.
///
/// # Limitation
///
/// One threshold cannot be right for every machine: a 64-core server pays more
/// dispatch cost and a 2-core phone less. This value was measured on exactly one
/// machine, with 4 logical cores, and has not been checked on any other — in
/// particular not on Android/Termux hardware, and not on a many-core server.
///
/// # Units
///
/// A count of cells, dimensionless.
pub const SPMV_MIN_CELLS: usize = 32_768;

/// Element count below which a [`ComputeBackend::CpuMulti`] request runs the
/// serial kernel instead, for the vector operations [`dot`], [`axpy`],
/// [`norm_l1`] and [`norm_l2`].
///
/// # Why this is larger than [`SPMV_MIN_CELLS`]
///
/// A vector operation does one or two floating-point operations per element
/// loaded, so it is limited by memory bandwidth rather than by arithmetic. Extra
/// cores do not add bandwidth, so there is much less to win and the fixed
/// dispatch cost is amortised much more slowly than it is for the sparse product,
/// which does roughly seven operations per cell.
///
/// # Measured crossover
///
/// Measured 2026-08-12 on the same machine and under the same conditions as
/// [`SPMV_MIN_CELLS`] (4 logical cores, release, `--features parallel`), best of
/// 5 timed repeats, per call. Produced by the `#[ignore]`d
/// `vecop_crossover_benchmark` test in `parallel/tests.rs`.
///
/// | Elements | `dot` serial | `dot` CpuMulti | Speed-up | `axpy` serial | `axpy` CpuMulti | Speed-up |
/// |---|---|---|---|---|---|---|
/// | 1 024 | 0.36 us | 16.61 us | 0.02x | 0.21 us | 15.10 us | 0.01x |
/// | 4 096 | 1.51 us | 18.05 us | 0.08x | 0.84 us | 16.36 us | 0.05x |
/// | 16 384 | 6.06 us | 20.79 us | 0.29x | 3.51 us | 19.19 us | 0.18x |
/// | 65 536 | 24.62 us | 32.51 us | 0.76x | 17.72 us | 30.65 us | 0.58x |
/// | 262 144 | 98.77 us | 65.87 us | 1.50x | 88.14 us | 79.72 us | 1.11x |
/// | 1 048 576 | 396.85 us | 213.87 us | 1.86x | 470.05 us | 348.55 us | 1.35x |
/// | 4 194 304 | 1601.10 us | 831.55 us | 1.93x | 2287.29 us | 1729.63 us | 1.32x |
///
/// Both operations cross over between 65 536 and 262 144 elements, so the
/// constant is set to **262 144**, the first measured size at which both won.
/// `axpy` gains far less than `dot` even well past the crossover (1.3x versus
/// 1.9x at 4M elements), which is the expected signature of a bandwidth-bound
/// kernel that also has to write its output.
///
/// # Limitation
///
/// Measured on one 4-core machine only; see [`SPMV_MIN_CELLS`].
///
/// # Units
///
/// A count of vector elements, dimensionless.
pub const VECOP_MIN_ELEMENTS: usize = 262_144;

// ── Backend dispatch ──────────────────────────────────────────────────────────

/// Resolve a requested backend to the one this module will actually run, given
/// how much work there is.
///
/// Three reductions happen here, in order:
///
/// 1. [`ComputeBackend::resolve`] degrades anything whose feature is off or whose
///    hardware is absent.
/// 2. `Gpu` degrades further, because this module has no GPU kernel yet.
/// 3. `CpuMulti` degrades to `Serial` below `min_work_items`, where thread
///    dispatch costs more than the work.
///
/// The result is only ever `Serial` or `CpuMulti`.
fn effective_backend(
    requested: ComputeBackend,
    work_items: usize,
    min_work_items: usize,
) -> ComputeBackend {
    // No GPU kernel exists in this module, so a resolved `Gpu` is re-resolved
    // down the CPU ladder rather than silently claiming to have run on a GPU.
    let cpu = match requested.resolve() {
        ComputeBackend::Gpu => ComputeBackend::CpuMulti.resolve(),
        other => other,
    };
    match cpu {
        ComputeBackend::CpuMulti if work_items >= min_work_items => ComputeBackend::CpuMulti,
        _ => ComputeBackend::Serial,
    }
}

/// The [`ComputeBackend`] this module would actually use for a sparse product
/// over `n_cells` cells if asked for `requested` — without running anything.
///
/// Useful for logging and for benchmark harnesses that need to report which path
/// a call took. It applies exactly the same three-step reduction the kernels do
/// (feature availability, no-GPU-kernel-here, and the [`SPMV_MIN_CELLS`] size
/// floor), so what it reports is what would run.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`HybridLdu::spmv`].
/// - `n_cells` — the matrix size, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
/// [`ComputeBackend::Gpu`], because no GPU kernel exists here yet.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::{spmv_backend_for, SPMV_MIN_CELLS};
///
/// // Too small to thread, whatever was asked for.
/// assert_eq!(spmv_backend_for(ComputeBackend::CpuMulti, 100), ComputeBackend::Serial);
///
/// // Big enough; the answer now depends only on whether `parallel` is compiled in.
/// let picked = spmv_backend_for(ComputeBackend::CpuMulti, SPMV_MIN_CELLS);
/// assert!(picked.is_available());
/// ```
#[must_use]
pub fn spmv_backend_for(requested: ComputeBackend, n_cells: usize) -> ComputeBackend {
    effective_backend(requested, n_cells, SPMV_MIN_CELLS)
}

/// The [`ComputeBackend`] this module would actually use for a vector operation
/// over `n` elements if asked for `requested` — without running anything.
///
/// The vector-operation counterpart of [`spmv_backend_for`], differing only in
/// using the [`VECOP_MIN_ELEMENTS`] size floor.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`dot`], [`axpy`],
///   [`norm_l1`] or [`norm_l2`].
/// - `n` — the vector length, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::vecop_backend_for;
///
/// assert_eq!(vecop_backend_for(ComputeBackend::CpuMulti, 64), ComputeBackend::Serial);
/// assert_eq!(vecop_backend_for(ComputeBackend::Serial, 1 << 24), ComputeBackend::Serial);
/// ```
#[must_use]
pub fn vecop_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, VECOP_MIN_ELEMENTS)
}

// ── Cell-gather index ─────────────────────────────────────────────────────────

/// The face addressing of an [`LduMatrix`], inverted into per-cell incident-face
/// lists so the matrix-vector product can be parallelised over cells.
///
/// This is a pure topology object: it depends only on `n_cells`, `owner` and
/// `neighbour`, **not** on any coefficient value. A finite-volume solver
/// reassembles coefficients every outer iteration while the mesh addressing stays
/// fixed, so build this once and reuse it — see [`HybridLdu::with_matrix`].
///
/// # Layout
///
/// Compressed-row: `row_start[c] .. row_start[c + 1]` selects cell `c`'s entries
/// out of the flat `entry_*` arrays. There are exactly `2 * n_internal_faces`
/// entries, because every internal face is incident on exactly two cells.
///
/// Each cell's entries are stored in **ascending internal-face index**. That is
/// not cosmetic: it is the property that makes the cell-gather product bitwise
/// reproduce the serial face-scatter of [`LduMatrix::multiply`], because it is
/// the order in which the scatter reaches that cell.
///
/// # Units
///
/// Pure indices and counts; dimensionless.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::parallel::LduTopology;
///
/// // 3 cells, 2 internal faces: (0,1) and (1,2).
/// let m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
/// let topo = LduTopology::from_matrix(&m);
///
/// assert_eq!(topo.n_cells(), 3);
/// assert_eq!(topo.n_internal_faces(), 2);
/// // The middle cell touches both faces; the end cells touch one each.
/// assert_eq!(topo.incident_face_count(0), 1);
/// assert_eq!(topo.incident_face_count(1), 2);
/// assert_eq!(topo.incident_face_count(2), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LduTopology {
    /// Number of cells the index was built for; matches `LduMatrix::n_cells`.
    n_cells: usize,
    /// Number of internal faces the index was built for.
    n_internal_faces: usize,
    /// Start offset of each cell's entry run, length `n_cells + 1`.
    row_start: Vec<usize>,
    /// Internal-face index of each entry, length `2 * n_internal_faces`.
    entry_face: Vec<usize>,
    /// The *other* cell across that face — the column index of the coefficient.
    entry_other: Vec<usize>,
    /// `true` when this cell is the face's owner, so the coefficient comes from
    /// `upper`; `false` when it is the neighbour, so it comes from `lower`.
    entry_uses_upper: Vec<bool>,
}

impl LduTopology {
    /// Build the cell-gather index from a matrix's face addressing.
    ///
    /// Cost is `O(n_cells + n_internal_faces)` in both time and memory (see
    /// [`Self::index_bytes`]). It is a one-off per mesh, not a per-iteration cost.
    ///
    /// Only `n_cells`, `owner` and `neighbour` are read. Coefficient values are
    /// ignored, so an index built from a freshly allocated matrix stays valid for
    /// every later reassembly of the same mesh.
    ///
    /// # Arguments
    ///
    /// - `matrix` — any [`LduMatrix`] with consistent addressing. Its
    ///   coefficients may be all zero.
    ///
    /// # Panics
    ///
    /// Panics if `owner` and `neighbour` have different lengths, or if any entry
    /// of either is `>= n_cells`. Both indicate corrupt addressing that would
    /// otherwise surface as an out-of-bounds access deep inside a kernel, or —
    /// worse — as a silently wrong answer.
    #[must_use]
    pub fn from_matrix(matrix: &LduMatrix) -> Self {
        let n_cells = matrix.n_cells;
        let n_faces = matrix.owner.len();
        assert_eq!(
            matrix.neighbour.len(),
            n_faces,
            "LduTopology::from_matrix: owner has {} entries but neighbour has {}; \
             there must be exactly one of each per internal face",
            n_faces,
            matrix.neighbour.len()
        );

        // Pass 1 — count how many faces touch each cell.
        let mut row_start = vec![0_usize; n_cells + 1];
        for f in 0..n_faces {
            let o = matrix.owner[f];
            let n = matrix.neighbour[f];
            assert!(
                o < n_cells && n < n_cells,
                "LduTopology::from_matrix: internal face {f} addresses cells ({o}, {n}) \
                 but the matrix has only {n_cells} cells"
            );
            row_start[o + 1] += 1;
            row_start[n + 1] += 1;
        }

        // Prefix sum turns per-cell counts into run offsets.
        for c in 0..n_cells {
            row_start[c + 1] += row_start[c];
        }

        // Pass 2 — scatter each face into both of its cells' runs. Faces are
        // visited in ascending order, so each cell's run ends up sorted by face
        // index: the property the bitwise-reproducibility guarantee rests on.
        let n_entries = 2 * n_faces;
        let mut entry_face = vec![0_usize; n_entries];
        let mut entry_other = vec![0_usize; n_entries];
        let mut entry_uses_upper = vec![false; n_entries];
        let mut cursor = row_start.clone();

        for f in 0..n_faces {
            let o = matrix.owner[f];
            let n = matrix.neighbour[f];

            // Owner side: y[owner] += upper[f] * x[neighbour].
            let e = cursor[o];
            entry_face[e] = f;
            entry_other[e] = n;
            entry_uses_upper[e] = true;
            cursor[o] += 1;

            // Neighbour side: y[neighbour] += lower[f] * x[owner].
            let e = cursor[n];
            entry_face[e] = f;
            entry_other[e] = o;
            entry_uses_upper[e] = false;
            cursor[n] += 1;
        }

        Self {
            n_cells,
            n_internal_faces: n_faces,
            row_start,
            entry_face,
            entry_other,
            entry_uses_upper,
        }
    }

    /// Number of cells this index was built for.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }

    /// Number of internal faces this index was built for.
    #[must_use]
    pub fn n_internal_faces(&self) -> usize {
        self.n_internal_faces
    }

    /// How many internal faces are incident on cell `c` — its off-diagonal count.
    ///
    /// # Panics
    ///
    /// Panics if `c >= n_cells()`.
    #[must_use]
    pub fn incident_face_count(&self, c: usize) -> usize {
        self.row_start[c + 1] - self.row_start[c]
    }

    /// Approximate heap footprint of the index, in bytes.
    ///
    /// Useful for judging whether the one-off build is affordable for a given
    /// mesh. It is roughly `8 * (n_cells + 1) + 17 * 2 * n_internal_faces` bytes
    /// on a 64-bit target, i.e. about 34 bytes per internal face. A
    /// 1-million-cell hexahedral mesh has roughly 3 million internal faces, so
    /// about 100 MB — substantial, and the reason the index is shared through an
    /// [`Arc`] rather than rebuilt per matrix.
    ///
    /// # Units
    ///
    /// Bytes, dimensionless.
    #[must_use]
    pub fn index_bytes(&self) -> usize {
        let usize_b = std::mem::size_of::<usize>();
        self.row_start.len() * usize_b
            + self.entry_face.len() * usize_b
            + self.entry_other.len() * usize_b
            + self.entry_uses_upper.len()
    }

    /// Whether this index describes `matrix`'s addressing exactly.
    ///
    /// Checks every entry against the matrix's own `owner`/`neighbour` arrays:
    /// for each cell `c` and each of its entries, the stored face must actually
    /// connect `c` to the stored other-cell, on the stored side. That is a
    /// complete verification, not a heuristic, and costs
    /// `O(n_cells + n_internal_faces)` — far less than rebuilding.
    ///
    /// Used by [`HybridLdu::with_matrix`] so a reassembled matrix over the same
    /// mesh can reuse an index instead of rebuilding it.
    ///
    /// # Returns
    ///
    /// `true` only if using this index with `matrix` would give exactly the same
    /// answer as an index freshly built from it.
    #[must_use]
    pub fn matches(&self, matrix: &LduMatrix) -> bool {
        if self.n_cells != matrix.n_cells
            || self.n_internal_faces != matrix.owner.len()
            || self.n_internal_faces != matrix.neighbour.len()
        {
            return false;
        }
        for c in 0..self.n_cells {
            for e in self.row_start[c]..self.row_start[c + 1] {
                let f = self.entry_face[e];
                if f >= self.n_internal_faces {
                    return false;
                }
                let (this_side, other_side) = if self.entry_uses_upper[e] {
                    (matrix.owner[f], matrix.neighbour[f])
                } else {
                    (matrix.neighbour[f], matrix.owner[f])
                };
                if this_side != c || other_side != self.entry_other[e] {
                    return false;
                }
            }
        }
        true
    }

    /// Gather row `c` of `A x` — that is, the single value `y[c]`.
    ///
    /// This is the one kernel both backends run; the only difference between them
    /// is which thread calls it. `#[inline]` so the serial and parallel block
    /// loops compile to the same inner code, which is part of why they agree
    /// bitwise.
    #[inline]
    fn gather_row(&self, matrix: &LduMatrix, x: &[f64], c: usize) -> f64 {
        // Start from 0.0 and accumulate, matching `LduMatrix::multiply`'s
        // `y = vec![0.0; n]; y[c] += diag[c] * x[c]` exactly.
        let mut acc = 0.0_f64;
        acc += matrix.diag[c] * x[c];
        for e in self.row_start[c]..self.row_start[c + 1] {
            let f = self.entry_face[e];
            let coef = if self.entry_uses_upper[e] {
                matrix.upper[f]
            } else {
                matrix.lower[f]
            };
            acc += coef * x[self.entry_other[e]];
        }
        acc
    }
}

// ── The hybrid-backend matrix ─────────────────────────────────────────────────

/// An [`LduMatrix`] bundled with its cell-gather index, exposing the
/// per-iteration kernels a Krylov solver needs on any [`ComputeBackend`].
///
/// Construct once per assembled matrix and call the kernels many times: the
/// `O(n_cells + n_faces)` index build happens in [`Self::new`], and every kernel
/// call after that is index-free. Both the matrix and the index are held behind
/// [`Arc`], so cloning is cheap and the value can be shared across threads.
///
/// # Which backend runs
///
/// Every kernel takes the backend as a parameter — there is no `_parallel`
/// sibling API. What actually runs is [`spmv_backend_for`] /
/// [`vecop_backend_for`] applied to the request: an unavailable backend degrades,
/// `Gpu` degrades (no GPU kernel here yet), and a problem below the measured
/// size floor runs serially. None of those degradations changes the answer.
/// [`Self::auto_backend`] applies the crate-wide policy
/// [`crate::compute::select_backend`] if you would rather not choose.
///
/// # Thread pool
///
/// The `parallel` kernels use `rayon`'s ambient pool — the global one by default.
/// No pool is built here, and none is built per call. A caller that wants a
/// specific worker count builds its own `rayon::ThreadPool` and calls these
/// kernels inside its `install(...)` scope; the parallel iterators then run on
/// that pool. Because every kernel is bitwise deterministic at any thread count,
/// that choice affects wall time only.
///
/// # Units
///
/// All vectors are dimensionless `f64` in cell order; see the module
/// documentation.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
///
/// let mut m = LduMatrix::new(4, vec![0, 1, 2], vec![1, 2, 3]);
/// m.diag = vec![4.0, 4.0, 4.0, 4.0];
/// m.upper = vec![-1.0, -1.0, -1.0];
/// m.lower = vec![-1.0, -1.0, -1.0];
/// let m = Arc::new(m);
///
/// let ldu = HybridLdu::new(Arc::clone(&m));
/// let x = vec![1.0, 2.0, 3.0, 4.0];
///
/// // Bit-for-bit agreement with the pre-existing serial reference kernel.
/// assert_eq!(ldu.spmv(&x, ComputeBackend::CpuMulti), m.multiply(&x));
///
/// // Reassembling the same mesh reuses the index instead of rebuilding it.
/// let mut m2 = (*m).clone();
/// m2.diag = vec![8.0, 8.0, 8.0, 8.0];
/// let ldu2 = ldu.with_matrix(Arc::new(m2)).expect("same addressing");
/// assert_eq!(ldu2.matrix().diag[0], 8.0);
/// ```
#[derive(Debug, Clone)]
pub struct HybridLdu {
    matrix: Arc<LduMatrix>,
    topology: Arc<LduTopology>,
}

impl HybridLdu {
    /// Build the cell-gather index for `matrix` and wrap it for hybrid execution.
    ///
    /// The `O(n_cells + n_internal_faces)` index build happens here, once.
    ///
    /// # Arguments
    ///
    /// - `matrix` — the assembled sparse system, shared by [`Arc`] so the caller
    ///   may keep its own handle.
    ///
    /// # Panics
    ///
    /// Panics if the matrix addressing is inconsistent — see
    /// [`LduTopology::from_matrix`].
    #[must_use]
    pub fn new(matrix: Arc<LduMatrix>) -> Self {
        let topology = Arc::new(LduTopology::from_matrix(&matrix));
        Self { matrix, topology }
    }

    /// Reuse this index with a **reassembled** matrix over the same mesh.
    ///
    /// A finite-volume solver rebuilds coefficients every outer iteration while
    /// the mesh addressing is unchanged. This returns a `HybridLdu` over the new
    /// coefficients that shares the existing index — an
    /// `O(n_cells + n_internal_faces)` addressing check instead of a full rebuild.
    ///
    /// # Returns
    ///
    /// `None` if `matrix` does not have exactly the same `owner`/`neighbour`
    /// addressing (see [`LduTopology::matches`]), in which case the caller must
    /// go through [`Self::new`]. Returning `None` rather than reusing a
    /// mismatched index is deliberate: a stale index would produce a wrong
    /// answer, not a crash.
    #[must_use]
    pub fn with_matrix(&self, matrix: Arc<LduMatrix>) -> Option<Self> {
        if !self.topology.matches(&matrix) {
            return None;
        }
        Some(Self {
            matrix,
            topology: Arc::clone(&self.topology),
        })
    }

    /// The matrix these kernels operate on.
    #[must_use]
    pub fn matrix(&self) -> &Arc<LduMatrix> {
        &self.matrix
    }

    /// The cell-gather index, shared behind an [`Arc`].
    #[must_use]
    pub fn topology(&self) -> &Arc<LduTopology> {
        &self.topology
    }

    /// The backend the crate-wide policy [`crate::compute::select_backend`] picks
    /// for a product on this matrix.
    ///
    /// Passes `n_cells` as the work-item count, because the cell-gather kernel's
    /// independent work items are cells. The returned backend is guaranteed
    /// available, so it can be handed straight to [`Self::spmv`].
    ///
    /// Note this is the *crate-wide* policy, whose `CpuMulti` threshold is
    /// documented as a placeholder; this module's own measured threshold
    /// [`SPMV_MIN_CELLS`] is applied afterwards by the kernel itself, so a
    /// too-eager `CpuMulti` from here still runs serially below that size.
    #[must_use]
    pub fn auto_backend(&self) -> ComputeBackend {
        select_backend(self.matrix.n_cells)
    }

    // ── Sparse matrix-vector product ─────────────────────────────────────────

    /// Sparse matrix-vector product `y = A x`, writing into a caller-owned buffer.
    ///
    /// This is the kernel a Krylov solver calls once (BiCGStab: twice) per
    /// iteration and where it spends most of its time. Prefer it over
    /// [`Self::spmv`] inside a solver loop — it reuses `y` instead of allocating
    /// a fresh `Vec` every iteration.
    ///
    /// # Arguments
    ///
    /// - `x` — input vector in cell order, exactly `n_cells` long, dimensionless.
    /// - `y` — output buffer, exactly `n_cells` long. Fully overwritten; its prior
    ///   contents are irrelevant. Aliasing with `x` is impossible because `y` is a
    ///   unique borrow.
    /// - `backend` — requested execution backend; see [`spmv_backend_for`] for
    ///   what will actually run.
    ///
    /// # Determinism
    ///
    /// Bitwise identical to [`LduMatrix::multiply`] on every backend and at any
    /// thread count. See the module documentation, "Determinism".
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `y.len()` differs from the matrix's `n_cells`.
    pub fn spmv_into(&self, x: &[f64], y: &mut [f64], backend: ComputeBackend) {
        self.spmv_into_min(x, y, backend, SPMV_MIN_CELLS);
    }

    /// [`Self::spmv_into`] with the size floor supplied by the caller.
    ///
    /// Exists so the crossover benchmark can measure the multi-CPU path *below*
    /// [`SPMV_MIN_CELLS`] — which is the only way to find where the crossover
    /// actually is — and so the cross-backend bitwise tests are not vacuous on
    /// small matrices. Not public: production callers get the measured floor.
    pub(crate) fn spmv_into_min(
        &self,
        x: &[f64],
        y: &mut [f64],
        backend: ComputeBackend,
        min_work_items: usize,
    ) {
        let n = self.matrix.n_cells;
        assert_eq!(
            x.len(),
            n,
            "HybridLdu::spmv_into: x has {} entries, expected n_cells = {n}",
            x.len()
        );
        assert_eq!(
            y.len(),
            n,
            "HybridLdu::spmv_into: y has {} entries, expected n_cells = {n}",
            y.len()
        );

        match effective_backend(backend, n, min_work_items) {
            #[cfg(feature = "parallel")]
            ComputeBackend::CpuMulti => {
                let (topology, matrix) = (&self.topology, &self.matrix);
                y.par_chunks_mut(CELL_BLOCK)
                    .enumerate()
                    .for_each(|(block, out)| {
                        let base = block * CELL_BLOCK;
                        for (k, slot) in out.iter_mut().enumerate() {
                            *slot = topology.gather_row(matrix, x, base + k);
                        }
                    });
            }
            _ => {
                for (c, slot) in y.iter_mut().enumerate() {
                    *slot = self.topology.gather_row(&self.matrix, x, c);
                }
            }
        }
    }

    /// Sparse matrix-vector product `y = A x`, allocating the result.
    ///
    /// Convenience wrapper over [`Self::spmv_into`] with identical semantics and
    /// the same bitwise guarantee. Inside a solver loop prefer
    /// [`Self::spmv_into`], which does not allocate.
    ///
    /// # Arguments
    ///
    /// - `x` — input vector in cell order, exactly `n_cells` long.
    /// - `backend` — requested execution backend.
    ///
    /// # Returns
    ///
    /// A fresh `Vec<f64>` of length `n_cells`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` differs from the matrix's `n_cells`.
    #[must_use]
    pub fn spmv(&self, x: &[f64], backend: ComputeBackend) -> Vec<f64> {
        let mut y = vec![0.0_f64; self.matrix.n_cells];
        self.spmv_into(x, &mut y, backend);
        y
    }

    // ── Residual ─────────────────────────────────────────────────────────────

    /// Residual `r = b - A x`, writing into a caller-owned buffer.
    ///
    /// # Arguments
    ///
    /// - `x` — current solution estimate, `n_cells` long.
    /// - `b` — the equation's source vector, `n_cells` long.
    /// - `r` — output buffer, `n_cells` long. Fully overwritten.
    /// - `backend` — requested execution backend.
    ///
    /// # Determinism
    ///
    /// Bitwise identical to [`LduMatrix::residual`] on every backend and at any
    /// thread count: the product is bitwise exact and the subtraction is
    /// element-wise, so no reassociation can occur.
    ///
    /// # Panics
    ///
    /// Panics if any of `x`, `b`, `r` has a length other than `n_cells`.
    pub fn residual_into(&self, x: &[f64], b: &[f64], r: &mut [f64], backend: ComputeBackend) {
        self.residual_into_min(x, b, r, backend, SPMV_MIN_CELLS);
    }

    /// [`Self::residual_into`] with a caller-supplied size floor; see
    /// [`Self::spmv_into_min`] for why this exists.
    pub(crate) fn residual_into_min(
        &self,
        x: &[f64],
        b: &[f64],
        r: &mut [f64],
        backend: ComputeBackend,
        min_work_items: usize,
    ) {
        let n = self.matrix.n_cells;
        assert_eq!(
            b.len(),
            n,
            "HybridLdu::residual_into: b has {} entries, expected n_cells = {n}",
            b.len()
        );
        assert_eq!(
            r.len(),
            n,
            "HybridLdu::residual_into: r has {} entries, expected n_cells = {n}",
            r.len()
        );

        // r <- A x, then r <- b - r, in place.
        self.spmv_into_min(x, r, backend, min_work_items);

        match effective_backend(backend, n, min_work_items) {
            #[cfg(feature = "parallel")]
            ComputeBackend::CpuMulti => {
                r.par_chunks_mut(CELL_BLOCK)
                    .zip(b.par_chunks(CELL_BLOCK))
                    .for_each(|(rc, bc)| {
                        for (ri, bi) in rc.iter_mut().zip(bc.iter()) {
                            *ri = bi - *ri;
                        }
                    });
            }
            _ => {
                for (ri, bi) in r.iter_mut().zip(b.iter()) {
                    *ri = bi - *ri;
                }
            }
        }
    }

    /// Residual `r = b - A x`, allocating the result.
    ///
    /// Convenience wrapper over [`Self::residual_into`]; identical semantics and
    /// the same bitwise guarantee against [`LduMatrix::residual`].
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `b.len()` differs from `n_cells`.
    #[must_use]
    pub fn residual(&self, x: &[f64], b: &[f64], backend: ComputeBackend) -> Vec<f64> {
        let mut r = vec![0.0_f64; self.matrix.n_cells];
        self.residual_into(x, b, &mut r, backend);
        r
    }

    /// OpenFOAM-style scaled residual norm,
    /// `||b - A x||_1 / (0.5 * (||A x||_1 + ||b||_1))`.
    ///
    /// This is the convergence measure the solvers in
    /// [`crate::ldu_matrix::solvers`] test against a tolerance, so it is evaluated
    /// once per iteration. The scaling makes the number dimensionless and roughly
    /// comparable across equations of very different magnitude. When the
    /// denominator underflows below [`f64::EPSILON`] the unscaled `||b - A x||_1`
    /// is returned instead, matching [`LduMatrix::normalised_residual`].
    ///
    /// # Arguments
    ///
    /// - `x` — current solution estimate, `n_cells` long.
    /// - `b` — source vector, `n_cells` long.
    /// - `backend` — requested execution backend.
    ///
    /// # Returns
    ///
    /// A dimensionless non-negative scalar.
    ///
    /// # Determinism, and how this differs from the serial reference
    ///
    /// This is a **reduction**, so unlike the product it cannot be bitwise
    /// identical to a flat left-to-right sum. It sums in blocks of
    /// [`REDUCTION_BLOCK`] elements and combines the block partials in ascending
    /// block order, an association fixed by `n_cells` alone. Therefore repeated
    /// runs agree bit for bit, and a 1-thread run agrees bit for bit with an
    /// N-thread run — but the value differs from
    /// [`LduMatrix::normalised_residual`] in the last bits.
    ///
    /// **Measured deviation.** Methodology: a fixed-seed xorshift64\* pseudorandom
    /// SPD-ish 7-point-stencil matrix on a 32x32x32 mesh (32 768 cells, 92 160
    /// internal faces), with pseudorandom `x` and `b` in `[-1, 1)`; compare this
    /// function against [`LduMatrix::normalised_residual`] on the same inputs and
    /// report the relative difference. Pass criterion: relative difference
    /// `<= 1e-13`. Result, measured 2026-08-12 by the test
    /// `normalised_residual_matches_flat_reference` in `parallel/tests.rs`:
    /// relative difference **1.4266e-16**, about 0.64 units in the last place —
    /// three orders of magnitude inside the gate. Interpretation: the difference
    /// is pure summation reassociation at rounding level, and the blocked form is
    /// if anything the more accurate of the two.
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `b.len()` differs from `n_cells`.
    #[must_use]
    pub fn normalised_residual(&self, x: &[f64], b: &[f64], backend: ComputeBackend) -> f64 {
        self.normalised_residual_min(x, b, backend, SPMV_MIN_CELLS)
    }

    /// [`Self::normalised_residual`] with a caller-supplied size floor; see
    /// [`Self::spmv_into_min`] for why this exists.
    pub(crate) fn normalised_residual_min(
        &self,
        x: &[f64],
        b: &[f64],
        backend: ComputeBackend,
        min_work_items: usize,
    ) -> f64 {
        let n = self.matrix.n_cells;
        assert_eq!(
            b.len(),
            n,
            "HybridLdu::normalised_residual: b has {} entries, expected n_cells = {n}",
            b.len()
        );

        let mut ax = vec![0.0_f64; n];
        self.spmv_into_min(x, &mut ax, backend, min_work_items);

        let partials: Vec<NormBlock> = match effective_backend(backend, n, min_work_items) {
            #[cfg(feature = "parallel")]
            ComputeBackend::CpuMulti => ax
                .par_chunks(REDUCTION_BLOCK)
                .zip(b.par_chunks(REDUCTION_BLOCK))
                .map(|(axc, bc)| NormBlock::of(axc, bc))
                .collect(),
            _ => ax
                .chunks(REDUCTION_BLOCK)
                .zip(b.chunks(REDUCTION_BLOCK))
                .map(|(axc, bc)| NormBlock::of(axc, bc))
                .collect(),
        };
        let combined = NormBlock::combine(&partials);

        let denom = (combined.ax_norm + combined.b_norm) * 0.5;
        if denom < f64::EPSILON {
            combined.r_norm
        } else {
            combined.r_norm / denom
        }
    }

    // ── Diagonal reciprocal ──────────────────────────────────────────────────

    /// Element-wise reciprocal of the diagonal, `1 / diag[c]` for every cell.
    ///
    /// This is the Jacobi preconditioner, and the first step of the DIC/DILU
    /// preconditioner setups. It is recomputed whenever the matrix is
    /// reassembled, which for a transient solve is every timestep. `O(n_cells)`.
    ///
    /// A zero diagonal yields an infinity and a `NaN` diagonal propagates, exactly
    /// as the scalar expression `1.0 / diag[c]` would. No clamping or masking is
    /// applied, because silently substituting a finite value would hide a singular
    /// assembly. Callers needing a guard should test the result with
    /// [`f64::is_finite`].
    ///
    /// # Arguments
    ///
    /// - `backend` — requested execution backend.
    ///
    /// # Returns
    ///
    /// A fresh `Vec<f64>` of length `n_cells`, in cell order. Its units are the
    /// reciprocal of the diagonal's units.
    ///
    /// # Determinism
    ///
    /// Bitwise identical on every backend and at any thread count: each element is
    /// one independent division.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use outram_foam_basic_lib::compute::ComputeBackend;
    /// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
    /// use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
    ///
    /// let mut m = LduMatrix::new(3, vec![], vec![]);
    /// m.diag = vec![2.0, 4.0, 0.0];
    /// let ldu = HybridLdu::new(Arc::new(m));
    ///
    /// let rd = ldu.diagonal_reciprocal(ComputeBackend::CpuMulti);
    /// assert_eq!(rd[0], 0.5);
    /// assert_eq!(rd[1], 0.25);
    /// assert!(rd[2].is_infinite()); // a singular row is reported, not hidden
    /// ```
    #[must_use]
    pub fn diagonal_reciprocal(&self, backend: ComputeBackend) -> Vec<f64> {
        let mut out = vec![0.0_f64; self.matrix.n_cells];
        self.diagonal_reciprocal_into(&mut out, backend);
        out
    }

    /// Element-wise reciprocal of the diagonal, into a caller-owned buffer.
    ///
    /// Same semantics as [`Self::diagonal_reciprocal`] without the allocation.
    ///
    /// # Arguments
    ///
    /// - `out` — output buffer, exactly `n_cells` long. Fully overwritten.
    /// - `backend` — requested execution backend.
    ///
    /// # Panics
    ///
    /// Panics if `out.len()` differs from `n_cells`.
    pub fn diagonal_reciprocal_into(&self, out: &mut [f64], backend: ComputeBackend) {
        self.diagonal_reciprocal_into_min(out, backend, VECOP_MIN_ELEMENTS);
    }

    /// [`Self::diagonal_reciprocal_into`] with a caller-supplied size floor; see
    /// [`Self::spmv_into_min`] for why this exists.
    pub(crate) fn diagonal_reciprocal_into_min(
        &self,
        out: &mut [f64],
        backend: ComputeBackend,
        min_work_items: usize,
    ) {
        let n = self.matrix.n_cells;
        assert_eq!(
            out.len(),
            n,
            "HybridLdu::diagonal_reciprocal_into: out has {} entries, expected n_cells = {n}",
            out.len()
        );
        let diag = &self.matrix.diag;

        match effective_backend(backend, n, min_work_items) {
            #[cfg(feature = "parallel")]
            ComputeBackend::CpuMulti => {
                out.par_chunks_mut(CELL_BLOCK)
                    .zip(diag.par_chunks(CELL_BLOCK))
                    .for_each(|(oc, dc)| {
                        for (o, d) in oc.iter_mut().zip(dc.iter()) {
                            *o = 1.0 / d;
                        }
                    });
            }
            _ => {
                for (o, d) in out.iter_mut().zip(diag.iter()) {
                    *o = 1.0 / d;
                }
            }
        }
    }
}

// ── Vector operations ─────────────────────────────────────────────────────────

/// Inner product `sum_i a_i * b_i`, on the chosen backend.
///
/// Krylov methods evaluate two or three of these per iteration (conjugate
/// gradient: `r.r` and `p.Ap`), so on a large mesh they are worth threading even
/// though the operation is memory-bandwidth bound rather than arithmetic bound.
///
/// # Arguments
///
/// - `a`, `b` — equal-length dimensionless vectors in cell order.
/// - `backend` — requested execution backend; see [`vecop_backend_for`] for what
///   will actually run.
///
/// # Returns
///
/// The dimensionless scalar product. Returns `0.0` for empty inputs.
///
/// # Determinism, and how this differs from [`crate::krylov::vecops::dot`]
///
/// Sums in fixed blocks of [`REDUCTION_BLOCK`] elements, combining block partials
/// in ascending block order. The result is therefore **bitwise identical between
/// backends and at any thread count**, and reproducible run to run. It differs
/// from the flat left-to-right sum in [`crate::krylov::vecops::dot`] only by
/// floating-point non-associativity.
///
/// **Measured deviation.** Methodology: two fixed-seed xorshift64\* pseudorandom
/// vectors with elements in `[-1, 1)`, at lengths 1 024 through 4 194 304
/// (powers of four); compare against [`crate::krylov::vecops::dot`] on the same
/// inputs; report the worst relative difference over all lengths. Pass criterion:
/// worst relative difference `<= 1e-12`. Result, measured 2026-08-12 by the test
/// `dot_matches_flat_reference` in `parallel/tests.rs`: worst relative difference
/// **1.0819e-15** at length 4 194 304, i.e. about 5 units in the last place on a
/// 4-million-term sum. Interpretation: pure reassociation at rounding level. The
/// blocked sum is the two-level, more accurate form; the flat sum is the one that
/// drifts as `n` grows.
///
/// # Panics
///
/// Panics if `a` and `b` have different lengths.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::dot;
///
/// let a = [1.0, 2.0, 3.0];
/// let b = [4.0, -5.0, 6.0];
/// // 1*4 + 2*(-5) + 3*6 = 12
/// assert_eq!(dot(&a, &b, ComputeBackend::Serial), 12.0);
/// // Bitwise identical, not merely close.
/// assert_eq!(
///     dot(&a, &b, ComputeBackend::CpuMulti),
///     dot(&a, &b, ComputeBackend::Serial),
/// );
/// ```
#[must_use]
pub fn dot(a: &[f64], b: &[f64], backend: ComputeBackend) -> f64 {
    dot_min(a, b, backend, VECOP_MIN_ELEMENTS)
}

/// [`dot`] with a caller-supplied size floor; see [`HybridLdu::spmv_into_min`]
/// for why the `_min` variants exist.
pub(crate) fn dot_min(
    a: &[f64],
    b: &[f64],
    backend: ComputeBackend,
    min_work_items: usize,
) -> f64 {
    assert_eq!(
        a.len(),
        b.len(),
        "dot: length mismatch ({} vs {})",
        a.len(),
        b.len()
    );

    let partials: Vec<f64> = match effective_backend(backend, a.len(), min_work_items) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => a
            .par_chunks(REDUCTION_BLOCK)
            .zip(b.par_chunks(REDUCTION_BLOCK))
            .map(|(x, y)| block_dot(x, y))
            .collect(),
        _ => a
            .chunks(REDUCTION_BLOCK)
            .zip(b.chunks(REDUCTION_BLOCK))
            .map(|(x, y)| block_dot(x, y))
            .collect(),
    };

    combine_in_order(&partials)
}

/// L2 (Euclidean) norm `sqrt(sum_i x_i^2)`, on the chosen backend.
///
/// Computed as `dot(x, x, backend).sqrt()`, so it inherits that function's
/// determinism guarantee exactly: bitwise identical between backends and at any
/// thread count, and differing from [`crate::krylov::vecops::nrm2`] only by
/// summation reassociation.
///
/// # Arguments
///
/// - `x` — a dimensionless vector in cell order.
/// - `backend` — requested execution backend.
///
/// # Returns
///
/// A non-negative dimensionless scalar; `0.0` for an empty input. For very large
/// element magnitudes the intermediate sum of squares can overflow to infinity —
/// there is no scaling guard, matching [`crate::krylov::vecops::nrm2`], because
/// well-scaled linear systems stay far inside normal `f64` range.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::norm_l2;
///
/// assert_eq!(norm_l2(&[3.0, 4.0], ComputeBackend::CpuMulti), 5.0);
/// assert_eq!(norm_l2(&[], ComputeBackend::Serial), 0.0);
/// ```
#[must_use]
pub fn norm_l2(x: &[f64], backend: ComputeBackend) -> f64 {
    dot(x, x, backend).sqrt()
}

/// L1 norm `sum_i abs(x_i)`, on the chosen backend.
///
/// This is the norm OpenFOAM's solver convergence test uses, and the one behind
/// [`HybridLdu::normalised_residual`].
///
/// # Arguments
///
/// - `x` — a dimensionless vector in cell order.
/// - `backend` — requested execution backend.
///
/// # Returns
///
/// A non-negative dimensionless scalar; `0.0` for an empty input.
///
/// # Determinism
///
/// Blocked summation exactly as in [`dot`]: bitwise identical between backends
/// and at any thread count, differing from a flat sum only by reassociation.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::norm_l1;
///
/// assert_eq!(norm_l1(&[1.0, -2.0, 3.0], ComputeBackend::CpuMulti), 6.0);
/// ```
#[must_use]
pub fn norm_l1(x: &[f64], backend: ComputeBackend) -> f64 {
    norm_l1_min(x, backend, VECOP_MIN_ELEMENTS)
}

/// [`norm_l1`] with a caller-supplied size floor; see
/// [`HybridLdu::spmv_into_min`] for why the `_min` variants exist.
pub(crate) fn norm_l1_min(x: &[f64], backend: ComputeBackend, min_work_items: usize) -> f64 {
    let partials: Vec<f64> = match effective_backend(backend, x.len(), min_work_items) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => x.par_chunks(REDUCTION_BLOCK).map(block_asum).collect(),
        _ => x.chunks(REDUCTION_BLOCK).map(block_asum).collect(),
    };
    combine_in_order(&partials)
}

/// AXPY update `y := alpha * x + y`, in place, on the chosen backend.
///
/// The vector update every Krylov iteration performs several times (advancing the
/// solution, the residual and the search direction).
///
/// # Arguments
///
/// - `alpha` — dimensionless scalar multiplier.
/// - `x` — dimensionless input vector in cell order.
/// - `y` — dimensionless accumulator, same length as `x`, updated in place.
/// - `backend` — requested execution backend.
///
/// # Determinism
///
/// Bitwise identical between backends and at any thread count: each element is an
/// independent fused expression, so there is no reduction to reassociate. Unlike
/// the reductions, this also matches [`crate::krylov::vecops::axpy`] exactly.
///
/// # Panics
///
/// Panics if `x` and `y` have different lengths.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::parallel::axpy;
///
/// let x = [1.0, 1.0, 1.0];
/// let mut y = [10.0, 20.0, 30.0];
/// axpy(2.0, &x, &mut y, ComputeBackend::CpuMulti);
/// assert_eq!(y, [12.0, 22.0, 32.0]);
/// ```
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64], backend: ComputeBackend) {
    axpy_min(alpha, x, y, backend, VECOP_MIN_ELEMENTS);
}

/// [`axpy`] with a caller-supplied size floor; see [`HybridLdu::spmv_into_min`]
/// for why the `_min` variants exist.
pub(crate) fn axpy_min(
    alpha: f64,
    x: &[f64],
    y: &mut [f64],
    backend: ComputeBackend,
    min_work_items: usize,
) {
    assert_eq!(
        x.len(),
        y.len(),
        "axpy: length mismatch ({} vs {})",
        x.len(),
        y.len()
    );

    match effective_backend(backend, x.len(), min_work_items) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => {
            y.par_chunks_mut(CELL_BLOCK)
                .zip(x.par_chunks(CELL_BLOCK))
                .for_each(|(yc, xc)| {
                    for (yi, xi) in yc.iter_mut().zip(xc.iter()) {
                        *yi += alpha * xi;
                    }
                });
        }
        _ => {
            for (yi, xi) in y.iter_mut().zip(x.iter()) {
                *yi += alpha * xi;
            }
        }
    }
}

// ── Internals ─────────────────────────────────────────────────────────────────

/// Serial inner product over one reduction block.
#[inline]
fn block_dot(x: &[f64], y: &[f64]) -> f64 {
    let mut p = 0.0_f64;
    for (xi, yi) in x.iter().zip(y.iter()) {
        p += xi * yi;
    }
    p
}

/// Serial sum of absolute values over one reduction block.
#[inline]
fn block_asum(x: &[f64]) -> f64 {
    let mut p = 0.0_f64;
    for xi in x {
        p += xi.abs();
    }
    p
}

/// Combine block partials in ascending block order.
///
/// This one function is why the reductions in this module are reproducible: the
/// partials arrive in a `Vec` indexed by block, so the association is fixed by the
/// array length alone, never by which thread finished first.
#[inline]
fn combine_in_order(partials: &[f64]) -> f64 {
    let mut total = 0.0_f64;
    for p in partials {
        total += p;
    }
    total
}

/// The three L1 partial sums one reduction block contributes to
/// [`HybridLdu::normalised_residual`].
///
/// A plain `Copy` struct so the parallel map can `collect()` it into an ordered
/// `Vec` and the combine step can walk that `Vec` in block order.
#[derive(Debug, Clone, Copy, Default)]
struct NormBlock {
    /// `sum abs(b - A x)` over the block.
    r_norm: f64,
    /// `sum abs(A x)` over the block.
    ax_norm: f64,
    /// `sum abs(b)` over the block.
    b_norm: f64,
}

impl NormBlock {
    /// Sum one block of `A x` against the matching block of `b`.
    fn of(ax: &[f64], b: &[f64]) -> Self {
        let mut out = Self::default();
        for (a, bi) in ax.iter().zip(b.iter()) {
            out.r_norm += (bi - a).abs();
            out.ax_norm += a.abs();
            out.b_norm += bi.abs();
        }
        out
    }

    /// Combine block partials in ascending block order.
    fn combine(blocks: &[NormBlock]) -> Self {
        let mut out = Self::default();
        for blk in blocks {
            out.r_norm += blk.r_norm;
            out.ax_norm += blk.ax_norm;
            out.b_norm += blk.b_norm;
        }
        out
    }
}
