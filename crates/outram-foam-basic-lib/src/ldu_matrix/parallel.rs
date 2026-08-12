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

//! Multi-threaded CPU kernels for the sparse LDU matrix — the hot path of every
//! implicit finite-volume solve.
//!
//! A Krylov solver (conjugate gradient, BiCGStab, GMRES) spends most of its wall
//! clock inside three operations, all of which this module provides in both a
//! single-thread and a [`rayon`]-parallel form behind one [`ComputeType`] switch:
//!
//! | Operation | Cost | Entry point |
//! |---|---|---|
//! | sparse matrix-vector product `y = A x` | `O(n_cells + n_faces)` | [`ParallelLdu::multiply`] / [`ParallelLdu::multiply_into`] |
//! | residual `r = b - A x` | `O(n_cells + n_faces)` | [`ParallelLdu::residual`] |
//! | scaled residual norm | `O(n_cells + n_faces)` | [`ParallelLdu::normalised_residual`] |
//! | diagonal reciprocal `1/diag` (Jacobi / DIC setup) | `O(n_cells)` | [`ParallelLdu::diagonal_reciprocal`] |
//! | vector dot product | `O(n_cells)` | [`dot`] |
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
//! a **data race**: two threads read-modify-write the same `y[c]`. In Rust that
//! cannot even be written without `unsafe`, and where it can (atomics, split
//! borrows) it produces silently wrong, run-varying answers.
//!
//! This module instead uses the **cell-gather reformulation**. A one-off index
//! build ([`LduTopology`]) inverts the face addressing into a per-cell list of
//! incident faces, turning the product into
//!
//! ```text
//! for each cell c:  y[c] = diag[c] * x[c]
//!                        + Σ_{f incident on c}  (c is owner ? upper[f] * x[neighbour[f]]
//!                                                            : lower[f] * x[owner[f]])
//! ```
//!
//! Now the loop is over **cells**, and every output element `y[c]` is written by
//! exactly one thread. There is no race, no atomic, and no per-thread scratch
//! buffer. The alternatives were considered and rejected: per-thread partial
//! accumulation costs `threads x n_cells` extra memory and needs a reduction pass
//! whose association order varies; face colouring needs a graph colouring at
//! build time and still writes each cell from several colours in sequence.
//!
//! # Determinism
//!
//! **The matrix-vector product, the residual and the diagonal reciprocal are
//! bitwise identical to their single-thread counterparts, and to
//! [`LduMatrix::multiply`] / [`LduMatrix::residual`]** — not merely equal to
//! within a tolerance. The index build lists each cell's incident faces in
//! ascending face order, which is exactly the order in which the serial scatter
//! reaches that cell, so every `y[c]` is accumulated by the same additions in the
//! same sequence. The result therefore does not depend on the thread count, on
//! the work-stealing schedule, or on the run. See
//! [`ComputeType::CpuSingleThread`], which remains **the deterministic trusted
//! reference** in the sense used across this workspace.
//!
//! **The two reduction kernels ([`ParallelLdu::normalised_residual`] and
//! [`dot`]) are run-to-run and thread-count deterministic, but are *not* bitwise
//! equal to a flat serial sum.** They sum in fixed-size blocks of
//! [`REDUCTION_BLOCK`] elements and then combine the block partials in ascending
//! block order. That association is fixed by `n_cells` alone — never by the
//! scheduler — so repeated runs agree bit for bit and a 1-thread run agrees bit
//! for bit with a 64-thread run. It differs from
//! [`LduMatrix::normalised_residual`]'s flat left-to-right sum only through
//! floating-point non-associativity; the measured size of that difference is
//! recorded in the module's test suite (see `parallel/tests.rs`,
//! `normalised_residual_matches_serial_reference_within_tolerance`).
//!
//! # When parallel is actually faster
//!
//! Threading is not free: dispatching one `rayon` job costs on the order of a
//! microsecond, while a small matrix-vector product finishes in less than that.
//! Below [`PARALLEL_MIN_CELLS`] cells the parallel backend therefore runs the
//! serial kernel on the calling thread. Because the two kernels are bitwise
//! identical this size dispatch changes **no** numerical result — only the wall
//! clock. The measured crossover that motivates the constant is recorded on
//! [`PARALLEL_MIN_CELLS`] itself.
//!
//! # Portability
//!
//! `rayon` is pure Rust with no system dependency, so everything here compiles
//! and runs on `aarch64-linux-android` / Termux exactly as on desktop; nothing in
//! this module is target-gated. [`std::thread::available_parallelism`] works on
//! Android too — a phone simply reports fewer cores and
//! [`ThreadCount::Auto`] resolves smaller.
//!
//! # Example
//!
//! ```rust
//! use std::sync::Arc;
//! use outram_foam_basic_lib::ldu_matrix::LduMatrix;
//! use outram_foam_basic_lib::ldu_matrix::parallel::{ComputeType, ParallelLdu, ThreadCount};
//!
//! // 3-cell symmetric tridiagonal  [[2,-1,0],[-1,2,-1],[0,-1,2]]
//! let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
//! m.diag = vec![2.0, 2.0, 2.0];
//! m.upper = vec![-1.0, -1.0];
//! m.lower = vec![-1.0, -1.0];
//! let m = Arc::new(m);
//!
//! let par = ParallelLdu::new(Arc::clone(&m), ComputeType::CpuMultiThread(ThreadCount::Auto));
//! let x = vec![1.0, 1.0, 1.0];
//!
//! // Bitwise identical to the single-thread reference kernel.
//! assert_eq!(par.multiply(&x), m.multiply(&x));
//! assert_eq!(par.multiply(&x), vec![1.0, 0.0, 1.0]);
//! ```

use std::sync::Arc;

use rayon::prelude::*;

use crate::ldu_matrix::ldu_matrix::LduMatrix;

#[cfg(test)]
mod tests;

// ── Backend selection ─────────────────────────────────────────────────────────

/// Which compute backend executes an LDU sparse-matrix kernel.
///
/// This mirrors the `ComputeType` switcher already used by `outram-mc-libs`
/// (`src/physics/compute.rs`) and `boon-lay` (`src/compute.rs`), so the three
/// crates present the same knob to a user. Enum dispatch is used deliberately —
/// no trait objects — so every `match self { … }` site is checked exhaustively at
/// compile time (workspace `CLAUDE.md`, "No trait objects").
///
/// | This enum | Meaning |
/// |---|---|
/// | [`CpuSingleThread`](Self::CpuSingleThread) | scalar, one thread — the reference |
/// | [`CpuMultiThread`](Self::CpuMultiThread)   | `rayon`-parallel over **cells** |
///
/// # Trust model
///
/// [`CpuSingleThread`](Self::CpuSingleThread) is the **deterministic trusted
/// reference** and is the default. Unusually for this workspace, the multi-thread
/// backend here is *also* bitwise deterministic and, for
/// [`ParallelLdu::multiply`] / [`ParallelLdu::residual`] /
/// [`ParallelLdu::diagonal_reciprocal`], bitwise **identical** to the reference —
/// the cell-gather reformulation fixes the summation order (see the module
/// documentation). It is still acceleration only: correctness questions are
/// settled against the single-thread path.
///
/// # No `Gpu` variant, deliberately
///
/// The sibling crates carry a third `Gpu` variant. This module implements no GPU
/// kernel for the LDU product, so no such variant is offered — a variant that
/// silently ran on the CPU would misreport what executed. Because dispatch is by
/// enum, adding one later turns every dispatch site into a compile error rather
/// than a silent fallthrough.
///
/// `Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
/// `f64`, which is only `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ComputeType {
    /// Scalar, single-thread kernels — the **deterministic trusted reference**.
    ///
    /// Runs on the calling thread with no pool and no synchronisation. This is
    /// the default, and it is the backend every other one is judged against.
    #[default]
    CpuSingleThread,

    /// `rayon`-parallel kernels, split over **cells**, sized by [`ThreadCount`].
    ///
    /// [`ParallelLdu`] builds a **dedicated** [`rayon::ThreadPool`] of the
    /// resolved size at construction time and reuses it for every subsequent
    /// call — the global implicit pool is never used, and no pool is built
    /// per-kernel (a solver calls these thousands of times, so per-call pool
    /// construction would dominate the runtime it is meant to save).
    ///
    /// Construct the usual form with `CpuMultiThread(ThreadCount::Auto)`.
    ///
    /// Two documented degradations to the serial path, neither of which changes
    /// any number produced: problems smaller than [`PARALLEL_MIN_CELLS`] cells
    /// run serially because thread dispatch would cost more than the work; and if
    /// the operating system refuses to build the thread pool, [`ParallelLdu`]
    /// records that and runs serially rather than failing
    /// ([`ParallelLdu::is_threaded`] reports which happened).
    CpuMultiThread(ThreadCount),
}

/// How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
/// sized to the CPU's strength.
///
/// [`ParallelLdu`] resolves this to a concrete positive thread count with
/// [`ThreadCount::resolve`] and builds a dedicated [`rayon::ThreadPool`] of that
/// size. The default is [`Auto`](Self::Auto), which reads the machine's logical
/// core count via [`std::thread::available_parallelism`] — a workstation
/// naturally gets many threads, a phone gets few, with no special-casing. All
/// variants resolve to **at least 1** thread.
///
/// The thread count affects wall time only: every kernel in this module produces
/// bit-identical output at any thread count (see the module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThreadCount {
    /// Use every logical core: [`std::thread::available_parallelism`]. Falls back
    /// to 1 if the query fails. The default.
    #[default]
    Auto,
    /// An explicit worker-thread count. Clamped up to a minimum of 1.
    Fixed(usize),
    /// A fraction of the available logical cores, e.g. `0.5` = half. The product
    /// `fraction * cores` is rounded to the nearest integer and clamped to at
    /// least 1, so any positive fraction yields a runnable pool.
    Fraction(f64),
}

impl ThreadCount {
    /// Resolve to a concrete worker-thread count (always `>= 1`).
    ///
    /// - [`Auto`](Self::Auto) → the logical core count from
    ///   [`std::thread::available_parallelism`], or `1` if that query fails.
    /// - [`Fixed(n)`](Self::Fixed) → `n.max(1)`.
    /// - [`Fraction(f)`](Self::Fraction) → `round(f * cores)`, clamped to `>= 1`.
    ///   A non-finite or non-positive fraction resolves to `1`.
    ///
    /// Pure and Android-safe (`available_parallelism` is `std` and works on
    /// Android — a phone just reports fewer cores).
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ldu_matrix::parallel::ThreadCount;
    /// assert_eq!(ThreadCount::Fixed(0).resolve(), 1);
    /// assert_eq!(ThreadCount::Fixed(4).resolve(), 4);
    /// assert!(ThreadCount::Auto.resolve() >= 1);
    /// assert_eq!(ThreadCount::Fraction(f64::NAN).resolve(), 1);
    /// ```
    pub fn resolve(self) -> usize {
        fn logical_cores() -> usize {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        }
        match self {
            ThreadCount::Auto => logical_cores(),
            ThreadCount::Fixed(n) => n.max(1),
            ThreadCount::Fraction(f) => {
                if !f.is_finite() || f <= 0.0 {
                    return 1;
                }
                ((f * logical_cores() as f64).round() as usize).max(1)
            }
        }
    }
}

impl ComputeType {
    /// A short human-readable label for the selected backend, for UI display and
    /// log lines.
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ldu_matrix::parallel::ComputeType;
    /// assert_eq!(
    ///     ComputeType::CpuSingleThread.label(),
    ///     "CPU (single-thread, deterministic reference)"
    /// );
    /// ```
    pub fn label(self) -> &'static str {
        match self {
            ComputeType::CpuSingleThread => "CPU (single-thread, deterministic reference)",
            ComputeType::CpuMultiThread(_) => "CPU (multi-thread, rayon cell-gather)",
        }
    }
}

// ── Tuning constants ──────────────────────────────────────────────────────────

/// Number of cells handed to one `rayon` task by the cell-parallel kernels.
///
/// A larger block amortises the per-task scheduling cost over more arithmetic; a
/// smaller block gives the work-stealing scheduler more pieces to balance. 1024
/// cells is roughly 8 KiB of `y` per task — comfortably inside an L1 data cache
/// while still producing hundreds of tasks for any mesh worth threading.
///
/// This constant affects wall time only. It does **not** affect the value of
/// [`ParallelLdu::multiply`], [`ParallelLdu::residual`] or
/// [`ParallelLdu::diagonal_reciprocal`], because each cell is computed
/// independently.
pub const CELL_BLOCK: usize = 1024;

/// Number of elements summed serially before block partials are combined, in the
/// reduction kernels ([`ParallelLdu::normalised_residual`], [`dot`]).
///
/// This constant **does** affect the last bits of those two results, because
/// floating-point addition is not associative — but it does so *reproducibly*.
/// The summation tree is a function of the array length and this constant alone,
/// never of the thread count or the scheduler, which is what makes those kernels
/// run-to-run deterministic. Changing it would perturb converged residual
/// histories in the last few digits, so treat it as part of the numerical
/// contract rather than a free tuning knob.
pub const REDUCTION_BLOCK: usize = 1024;

/// Cell count below which [`ComputeType::CpuMultiThread`] runs the serial kernel.
///
/// # Why a threshold exists
///
/// Dispatching a `rayon` parallel iterator costs on the order of a microsecond of
/// scheduling and synchronisation. A sparse matrix-vector product on a small mesh
/// finishes in less than that, so threading it is a straight loss. Because the
/// serial and parallel kernels are **bitwise identical** (module documentation,
/// "Determinism"), dispatching on size changes no number a caller can observe —
/// only the wall clock.
///
/// # Measured crossover
///
/// Measured 2026-08-12 on the machine this was developed on
/// (`std::thread::available_parallelism()` = 4, `ThreadCount::Auto`, release
/// build), for a structured 7-point-stencil LDU matrix, best of several timed
/// repeats, reported per `multiply` call:
///
/// | Cells | Serial | Parallel (4 threads) | Speed-up |
/// |---|---|---|---|
/// | 1 000 | 1.42 us | 8.94 us | 0.16x |
/// | 8 000 | 12.6 us | 17.6 us | 0.72x |
/// | 27 000 | 45.9 us | 32.4 us | 1.42x |
/// | 64 000 | 111 us | 60.7 us | 1.83x |
/// | 216 000 | 405 us | 187 us | 2.17x |
/// | 512 000 | 1.03 ms | 462 us | 2.23x |
///
/// The crossover — the size at which the parallel path stops losing — sits
/// between 8 000 and 27 000 cells on that 4-core machine, so the threshold is set
/// to **16 384**, near the geometric middle of that bracket and a round power of
/// two. The bracketing measurement is reproducible with the `#[ignore]`d
/// `multiply_crossover_benchmark` test in `parallel/tests.rs`.
///
/// # Limitation
///
/// One threshold cannot be right for every machine: a 64-core server pays more
/// dispatch cost and would want a larger value, a 2-core phone a smaller one. The
/// value is conservative in the useful direction — a mesh below 16 384 cells is
/// too small for threading to matter on any machine, and above it the parallel
/// path was never slower than serial in the measurements above.
pub const PARALLEL_MIN_CELLS: usize = 16_384;

// ── Cell-gather index ─────────────────────────────────────────────────────────

/// The face addressing of an [`LduMatrix`], inverted into per-cell incident-face
/// lists so the matrix-vector product can be parallelised over cells.
///
/// This is a pure topology object: it depends only on `owner`/`neighbour` and
/// `n_cells`, **not** on any coefficient value. A finite-volume solver reassembles
/// coefficients every outer iteration while the mesh addressing stays fixed, so
/// build this once and reuse it (see [`ParallelLdu::with_matrix`]).
///
/// Layout is compressed-row: `row_start[c] .. row_start[c + 1]` selects cell `c`'s
/// entries out of the flat `entry_*` arrays. There are exactly
/// `2 * n_internal_faces` entries in total, since every internal face is incident
/// on exactly two cells.
///
/// The entries of each cell are stored in **ascending face index**, which is what
/// makes the gather bitwise reproduce the serial scatter of
/// [`LduMatrix::multiply`].
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
/// // The middle cell touches both faces; the end cells touch one each.
/// assert_eq!(topo.incident_face_count(0), 1);
/// assert_eq!(topo.incident_face_count(1), 2);
/// assert_eq!(topo.incident_face_count(2), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LduTopology {
    /// Number of cells the index was built for; must match `LduMatrix::n_cells`.
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
    /// Cost is `O(n_cells + n_internal_faces)` time and
    /// `O(n_cells + n_internal_faces)` memory (see [`Self::index_bytes`]); it is a
    /// one-off, not a per-iteration cost.
    ///
    /// Only `n_cells`, `owner` and `neighbour` are read. Coefficient values are
    /// ignored, so an index built from a freshly allocated matrix is valid for
    /// every later reassembly of the same mesh.
    ///
    /// # Panics
    ///
    /// Panics if `owner` and `neighbour` have different lengths, or if any entry
    /// in either is `>= n_cells` — both indicate corrupt addressing that would
    /// otherwise surface as an out-of-bounds access deep inside a kernel.
    pub fn from_matrix(matrix: &LduMatrix) -> Self {
        let n_cells = matrix.n_cells;
        let n_faces = matrix.owner.len();
        assert_eq!(
            matrix.neighbour.len(),
            n_faces,
            "LduTopology::from_matrix: owner (len {}) and neighbour (len {}) must have one entry per internal face",
            n_faces,
            matrix.neighbour.len()
        );

        // Pass 1: how many faces touch each cell.
        let mut row_start = vec![0_usize; n_cells + 1];
        for f in 0..n_faces {
            let o = matrix.owner[f];
            let n = matrix.neighbour[f];
            assert!(
                o < n_cells && n < n_cells,
                "LduTopology::from_matrix: face {f} addresses cells ({o}, {n}) but the matrix has only {n_cells} cells"
            );
            row_start[o + 1] += 1;
            row_start[n + 1] += 1;
        }

        // Prefix sum turns counts into run offsets.
        for c in 0..n_cells {
            row_start[c + 1] += row_start[c];
        }

        // Pass 2: scatter each face into both of its cells' runs. Faces are
        // visited in ascending order, so each cell's run ends up sorted by face
        // index — the property the bitwise-reproducibility guarantee rests on.
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
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }

    /// Number of internal faces this index was built for.
    pub fn n_internal_faces(&self) -> usize {
        self.n_internal_faces
    }

    /// How many internal faces are incident on cell `c` — its off-diagonal count.
    ///
    /// # Panics
    ///
    /// Panics if `c >= n_cells()`.
    pub fn incident_face_count(&self, c: usize) -> usize {
        self.row_start[c + 1] - self.row_start[c]
    }

    /// Approximate heap footprint of the index, in bytes.
    ///
    /// Useful for judging whether the one-off index build is affordable for a
    /// given mesh: it is roughly `8 * (n_cells + 1) + 17 * 2 * n_internal_faces`
    /// bytes, i.e. about 34 bytes per internal face. A 1-million-cell hexahedral
    /// mesh has about 3 million internal faces, so about 100 MB — significant, and
    /// the reason the index is shared through an [`Arc`] rather than rebuilt.
    pub fn index_bytes(&self) -> usize {
        let usize_b = std::mem::size_of::<usize>();
        self.row_start.len() * usize_b
            + self.entry_face.len() * usize_b
            + self.entry_other.len() * usize_b
            + self.entry_uses_upper.len()
    }

    /// Whether this index describes `matrix`'s addressing exactly.
    ///
    /// Compares `n_cells`, the internal-face count, and every `owner`/`neighbour`
    /// entry — `O(n_internal_faces)`, far cheaper than rebuilding. Used by
    /// [`ParallelLdu::with_matrix`] to reuse an index across a reassembly.
    pub fn matches(&self, matrix: &LduMatrix) -> bool {
        if self.n_cells != matrix.n_cells || self.n_internal_faces != matrix.owner.len() {
            return false;
        }
        if matrix.neighbour.len() != self.n_internal_faces {
            return false;
        }
        // Re-derive: entry e of cell c is consistent iff the stored (face, other)
        // pair agrees with the matrix addressing.
        for e in 0..self.entry_face.len() {
            let f = self.entry_face[e];
            let other = self.entry_other[e];
            let expected = if self.entry_uses_upper[e] {
                matrix.neighbour[f]
            } else {
                matrix.owner[f]
            };
            if other != expected {
                return false;
            }
        }
        true
    }

    /// Gather row `c` of `A x` — the value of `y[c]`.
    ///
    /// This is the single kernel both backends run; the only difference between
    /// them is which thread calls it. Kept `#[inline]` so the block loops in the
    /// serial and parallel paths compile to the same inner code.
    #[inline]
    fn gather_row(&self, matrix: &LduMatrix, x: &[f64], c: usize) -> f64 {
        // Start from 0.0 and accumulate, matching `LduMatrix::multiply`'s
        // `y = vec![0.0]; y[c] += diag * x[c]` exactly (including the sign of a
        // zero result).
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

// ── The threaded matrix ───────────────────────────────────────────────────────

/// An [`LduMatrix`] bundled with its cell-gather index and a compute backend,
/// exposing the per-iteration kernels a Krylov solver needs.
///
/// Construct once per assembled matrix and call the kernels many times. Both the
/// matrix and the index are held behind [`Arc`], so cloning a `ParallelLdu` is
/// cheap and it can be shared across threads; the thread pool (when the
/// multi-thread backend is selected) is built once at construction and reused for
/// every kernel call.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::parallel::{ComputeType, ParallelLdu, ThreadCount};
///
/// let mut m = LduMatrix::new(4, vec![0, 1, 2], vec![1, 2, 3]);
/// m.diag = vec![4.0, 4.0, 4.0, 4.0];
/// m.upper = vec![-1.0, -1.0, -1.0];
/// m.lower = vec![-1.0, -1.0, -1.0];
/// let m = Arc::new(m);
///
/// let par = ParallelLdu::new(Arc::clone(&m), ComputeType::CpuMultiThread(ThreadCount::Fixed(2)));
/// let x = vec![1.0, 2.0, 3.0, 4.0];
///
/// // Every kernel agrees bit for bit with the single-thread reference.
/// let serial = ParallelLdu::new(Arc::clone(&m), ComputeType::CpuSingleThread);
/// assert_eq!(par.multiply(&x), serial.multiply(&x));
/// assert_eq!(par.multiply(&x), m.multiply(&x));
///
/// // Reassembling the same mesh reuses the index instead of rebuilding it.
/// let mut m2 = (*m).clone();
/// m2.diag = vec![8.0, 8.0, 8.0, 8.0];
/// let par2 = par.with_matrix(Arc::new(m2)).expect("same addressing");
/// assert_eq!(par2.matrix().diag[0], 8.0);
/// ```
#[derive(Debug, Clone)]
pub struct ParallelLdu {
    matrix: Arc<LduMatrix>,
    topology: Arc<LduTopology>,
    compute: ComputeType,
    /// `Some` only when the multi-thread backend was selected *and* the pool
    /// actually built. `None` means every kernel runs on the calling thread.
    pool: Option<Arc<rayon::ThreadPool>>,
}

impl ParallelLdu {
    /// Build the cell-gather index for `matrix` and select a compute backend.
    ///
    /// The index build is `O(n_cells + n_internal_faces)` and happens here, once.
    /// For [`ComputeType::CpuMultiThread`] a dedicated [`rayon::ThreadPool`] of
    /// [`ThreadCount::resolve`] threads is also built here and reused by every
    /// later kernel call — the global implicit `rayon` pool is never touched.
    ///
    /// If the operating system refuses to create the pool, construction still
    /// succeeds and every kernel runs serially; [`Self::is_threaded`] then returns
    /// `false`. No numerical result changes either way.
    ///
    /// # Panics
    ///
    /// Panics if the matrix addressing is inconsistent — see
    /// [`LduTopology::from_matrix`].
    pub fn new(matrix: Arc<LduMatrix>, compute: ComputeType) -> Self {
        let topology = Arc::new(LduTopology::from_matrix(&matrix));
        let pool = build_pool(compute);
        Self {
            matrix,
            topology,
            compute,
            pool,
        }
    }

    /// Reuse this index and thread pool with a **reassembled** matrix.
    ///
    /// A finite-volume solver rebuilds coefficients every outer iteration while
    /// the mesh addressing is unchanged. This returns a `ParallelLdu` over the new
    /// coefficients that shares the existing index and pool — an
    /// `O(n_internal_faces)` addressing check instead of a full index rebuild and
    /// a fresh set of OS threads.
    ///
    /// Returns `None` if `matrix` does not have exactly the same
    /// `owner`/`neighbour` addressing (see [`LduTopology::matches`]), in which
    /// case the caller must go through [`Self::new`].
    pub fn with_matrix(&self, matrix: Arc<LduMatrix>) -> Option<Self> {
        if !self.topology.matches(&matrix) {
            return None;
        }
        Some(Self {
            matrix,
            topology: Arc::clone(&self.topology),
            compute: self.compute,
            pool: self.pool.clone(),
        })
    }

    /// The matrix these kernels operate on.
    pub fn matrix(&self) -> &LduMatrix {
        &self.matrix
    }

    /// The cell-gather index, shared behind an [`Arc`].
    pub fn topology(&self) -> &Arc<LduTopology> {
        &self.topology
    }

    /// The selected backend, as requested at construction.
    ///
    /// This reports what was *asked for*. Use [`Self::is_threaded`] to find out
    /// whether a thread pool actually exists.
    pub fn compute(&self) -> ComputeType {
        self.compute
    }

    /// Whether a dedicated thread pool was successfully built.
    ///
    /// `false` for [`ComputeType::CpuSingleThread`], and also `false` if
    /// [`ComputeType::CpuMultiThread`] was requested but the pool could not be
    /// created. Note that a `true` here does not mean a given call *will* thread:
    /// calls on matrices below [`PARALLEL_MIN_CELLS`] cells still run serially.
    pub fn is_threaded(&self) -> bool {
        self.pool.is_some()
    }

    /// Number of worker threads in the dedicated pool, or 1 when running serially.
    pub fn thread_count(&self) -> usize {
        self.pool.as_ref().map(|p| p.current_num_threads()).unwrap_or(1)
    }

    /// Whether a kernel over `n_cells` cells will actually be threaded.
    fn threads_this_call(&self) -> bool {
        self.pool.is_some() && self.matrix.n_cells >= PARALLEL_MIN_CELLS
    }

    // ── Sparse matrix-vector product ─────────────────────────────────────────

    /// Sparse matrix-vector product `y = A x`, writing into a caller-owned buffer.
    ///
    /// This is the kernel a Krylov solver calls once (BiCGStab: twice) per
    /// iteration and where it spends most of its time. Prefer this over
    /// [`Self::multiply`] inside a solver loop — it reuses `y` instead of
    /// allocating a fresh `Vec` every iteration.
    ///
    /// `x` and `y` are dimensionless `f64` slices in cell order: element `c` is
    /// the value at cell `c`, in whatever units the assembled equation carries.
    /// Both must be exactly `n_cells` long. Aliasing is impossible — `y` is a
    /// unique borrow.
    ///
    /// Bitwise identical to [`LduMatrix::multiply`] at any thread count; see the
    /// module documentation.
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `y.len()` differs from the matrix's `n_cells`.
    pub fn multiply_into(&self, x: &[f64], y: &mut [f64]) {
        let n = self.matrix.n_cells;
        assert_eq!(x.len(), n, "multiply_into: x has {} entries, expected {n}", x.len());
        assert_eq!(y.len(), n, "multiply_into: y has {} entries, expected {n}", y.len());

        if self.threads_this_call() {
            let pool = self.pool.as_ref().expect("threads_this_call implies a pool");
            let (topology, matrix) = (&self.topology, &self.matrix);
            pool.install(|| {
                y.par_chunks_mut(CELL_BLOCK)
                    .enumerate()
                    .for_each(|(block, out)| {
                        let base = block * CELL_BLOCK;
                        for (k, slot) in out.iter_mut().enumerate() {
                            *slot = topology.gather_row(matrix, x, base + k);
                        }
                    });
            });
        } else {
            for (c, slot) in y.iter_mut().enumerate() {
                *slot = self.topology.gather_row(&self.matrix, x, c);
            }
        }
    }

    /// Sparse matrix-vector product `y = A x`, allocating the result.
    ///
    /// Convenience wrapper over [`Self::multiply_into`] with the same semantics
    /// and the same bitwise guarantee. Inside a solver loop prefer
    /// [`Self::multiply_into`], which does not allocate.
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` differs from the matrix's `n_cells`.
    pub fn multiply(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0_f64; self.matrix.n_cells];
        self.multiply_into(x, &mut y);
        y
    }

    // ── Residual ─────────────────────────────────────────────────────────────

    /// Residual `r = b - A x`, writing into a caller-owned buffer.
    ///
    /// `x` is the current solution estimate and `b` the equation's source, both in
    /// cell order and both `n_cells` long. Bitwise identical to
    /// [`LduMatrix::residual`] at any thread count: the product is bitwise exact
    /// (see the module documentation) and the subtraction is element-wise, so no
    /// reassociation can occur.
    ///
    /// # Panics
    ///
    /// Panics if any of `x`, `b`, `r` has a length other than `n_cells`.
    pub fn residual_into(&self, x: &[f64], b: &[f64], r: &mut [f64]) {
        let n = self.matrix.n_cells;
        assert_eq!(b.len(), n, "residual_into: b has {} entries, expected {n}", b.len());
        assert_eq!(r.len(), n, "residual_into: r has {} entries, expected {n}", r.len());

        // r <- A x, then r <- b - r, in place.
        self.multiply_into(x, r);

        if self.threads_this_call() {
            let pool = self.pool.as_ref().expect("threads_this_call implies a pool");
            pool.install(|| {
                r.par_chunks_mut(CELL_BLOCK)
                    .zip(b.par_chunks(CELL_BLOCK))
                    .for_each(|(rc, bc)| {
                        for (ri, bi) in rc.iter_mut().zip(bc.iter()) {
                            *ri = bi - *ri;
                        }
                    });
            });
        } else {
            for (ri, bi) in r.iter_mut().zip(b.iter()) {
                *ri = bi - *ri;
            }
        }
    }

    /// Residual `r = b - A x`, allocating the result.
    ///
    /// Convenience wrapper over [`Self::residual_into`]; same semantics and same
    /// bitwise guarantee against [`LduMatrix::residual`].
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `b.len()` differs from `n_cells`.
    pub fn residual(&self, x: &[f64], b: &[f64]) -> Vec<f64> {
        let mut r = vec![0.0_f64; self.matrix.n_cells];
        self.residual_into(x, b, &mut r);
        r
    }

    /// OpenFOAM-style scaled residual norm,
    /// `||b - A x||_1 / (0.5 * (||A x||_1 + ||b||_1))`.
    ///
    /// This is the convergence measure the solvers in
    /// [`crate::ldu_matrix::solvers`] test against a tolerance, so it is evaluated
    /// once per iteration. The scaling makes the number dimensionless and roughly
    /// comparable across equations of different magnitude. When the denominator
    /// underflows to below [`f64::EPSILON`] the unscaled `||b - A x||_1` is
    /// returned instead, matching [`LduMatrix::normalised_residual`].
    ///
    /// # Determinism, and how this differs from the serial reference
    ///
    /// Unlike the product and the residual, this kernel is a **reduction**, so it
    /// cannot be bitwise identical to a flat left-to-right sum. It sums in fixed
    /// blocks of [`REDUCTION_BLOCK`] elements and combines the block partials in
    /// ascending block order, an association fixed by `n_cells` alone. Therefore:
    ///
    /// - repeated runs agree **bit for bit**;
    /// - a 1-thread run agrees **bit for bit** with an N-thread run;
    /// - the value differs from [`LduMatrix::normalised_residual`] only by
    ///   floating-point non-associativity, at the level quantified in this
    ///   module's test suite.
    ///
    /// # Panics
    ///
    /// Panics if `x.len()` or `b.len()` differs from `n_cells`.
    pub fn normalised_residual(&self, x: &[f64], b: &[f64]) -> f64 {
        let n = self.matrix.n_cells;
        assert_eq!(b.len(), n, "normalised_residual: b has {} entries, expected {n}", b.len());

        let ax = self.multiply(x);

        let combined = if self.threads_this_call() {
            let pool = self.pool.as_ref().expect("threads_this_call implies a pool");
            pool.install(|| {
                let partials: Vec<NormBlock> = ax
                    .par_chunks(REDUCTION_BLOCK)
                    .zip(b.par_chunks(REDUCTION_BLOCK))
                    .map(|(axc, bc)| NormBlock::of(axc, bc))
                    .collect();
                NormBlock::combine(&partials)
            })
        } else {
            let partials: Vec<NormBlock> = ax
                .chunks(REDUCTION_BLOCK)
                .zip(b.chunks(REDUCTION_BLOCK))
                .map(|(axc, bc)| NormBlock::of(axc, bc))
                .collect();
            NormBlock::combine(&partials)
        };

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
    /// preconditioner setups: it is recomputed whenever the matrix is reassembled,
    /// which for a transient solve is every timestep. `O(n_cells)`.
    ///
    /// A zero diagonal yields `±inf` and a `NaN` diagonal propagates, exactly as
    /// the scalar expression `1.0 / diag[c]` would — no clamping or masking is
    /// applied, because silently substituting a finite value would hide a
    /// singular assembly. Callers that need a guard should check the result with
    /// [`f64::is_finite`].
    ///
    /// Bitwise identical between the two backends and at any thread count: each
    /// element is one independent division.
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
    /// use outram_foam_basic_lib::ldu_matrix::parallel::{ComputeType, ParallelLdu};
    ///
    /// let mut m = LduMatrix::new(3, vec![], vec![]);
    /// m.diag = vec![2.0, 4.0, 0.0];
    /// let par = ParallelLdu::new(Arc::new(m), ComputeType::CpuSingleThread);
    ///
    /// let rd = par.diagonal_reciprocal();
    /// assert_eq!(rd[0], 0.5);
    /// assert_eq!(rd[1], 0.25);
    /// assert!(rd[2].is_infinite()); // singular row is reported, not hidden
    /// ```
    pub fn diagonal_reciprocal(&self) -> Vec<f64> {
        let mut out = vec![0.0_f64; self.matrix.n_cells];
        self.diagonal_reciprocal_into(&mut out);
        out
    }

    /// Element-wise reciprocal of the diagonal, into a caller-owned buffer.
    ///
    /// Same semantics as [`Self::diagonal_reciprocal`] without the allocation.
    ///
    /// # Panics
    ///
    /// Panics if `out.len()` differs from `n_cells`.
    pub fn diagonal_reciprocal_into(&self, out: &mut [f64]) {
        let n = self.matrix.n_cells;
        assert_eq!(
            out.len(),
            n,
            "diagonal_reciprocal_into: out has {} entries, expected {n}",
            out.len()
        );
        let diag = &self.matrix.diag;

        if self.threads_this_call() {
            let pool = self.pool.as_ref().expect("threads_this_call implies a pool");
            pool.install(|| {
                out.par_chunks_mut(CELL_BLOCK)
                    .zip(diag.par_chunks(CELL_BLOCK))
                    .for_each(|(oc, dc)| {
                        for (o, d) in oc.iter_mut().zip(dc.iter()) {
                            *o = 1.0 / d;
                        }
                    });
            });
        } else {
            for (o, d) in out.iter_mut().zip(diag.iter()) {
                *o = 1.0 / d;
            }
        }
    }
}

// ── Free-standing vector kernel ───────────────────────────────────────────────

/// Dot product `a · b` over two cell-ordered vectors, on the chosen backend.
///
/// Krylov methods evaluate two or three of these per iteration (conjugate
/// gradient: `r·r` and `p·Ap`), so on a large mesh they are worth threading even
/// though the operation is memory-bandwidth bound. This is a free function rather
/// than a [`ParallelLdu`] method because it needs no matrix — but note that it
/// therefore builds a thread pool per call when given
/// [`ComputeType::CpuMultiThread`]. Inside a solver loop that is the wrong
/// trade: call it from within an existing
/// [`rayon::ThreadPool::install`] scope, or use the single-thread backend, rather
/// than paying pool construction thousands of times.
///
/// # Determinism
///
/// Reduces in fixed blocks of [`REDUCTION_BLOCK`] elements, combining block
/// partials in ascending block order. The result is therefore **bitwise identical
/// between the two backends and at any thread count**, and reproducible run to
/// run. It differs from a naive flat `a.iter().zip(b).map(..).sum()` only by
/// floating-point non-associativity.
///
/// # Panics
///
/// Panics if `a` and `b` have different lengths.
///
/// ```rust
/// use outram_foam_basic_lib::ldu_matrix::parallel::{dot, ComputeType, ThreadCount};
///
/// let a: Vec<f64> = (0..10_000).map(|i| (i as f64) * 1e-3).collect();
/// let b: Vec<f64> = (0..10_000).map(|i| 1.0 / (1.0 + i as f64)).collect();
///
/// let serial = dot(&a, &b, ComputeType::CpuSingleThread);
/// let parallel = dot(&a, &b, ComputeType::CpuMultiThread(ThreadCount::Fixed(4)));
/// assert_eq!(serial, parallel); // bitwise, not merely close
/// ```
pub fn dot(a: &[f64], b: &[f64], compute: ComputeType) -> f64 {
    assert_eq!(
        a.len(),
        b.len(),
        "dot: length mismatch ({} vs {})",
        a.len(),
        b.len()
    );

    let block_dot = |x: &[f64], y: &[f64]| -> f64 {
        let mut p = 0.0_f64;
        for (xi, yi) in x.iter().zip(y.iter()) {
            p += xi * yi;
        }
        p
    };

    let partials: Vec<f64> = match compute {
        ComputeType::CpuSingleThread => a
            .chunks(REDUCTION_BLOCK)
            .zip(b.chunks(REDUCTION_BLOCK))
            .map(|(x, y)| block_dot(x, y))
            .collect(),
        ComputeType::CpuMultiThread(_) if a.len() < PARALLEL_MIN_CELLS => a
            .chunks(REDUCTION_BLOCK)
            .zip(b.chunks(REDUCTION_BLOCK))
            .map(|(x, y)| block_dot(x, y))
            .collect(),
        ComputeType::CpuMultiThread(tc) => match build_pool(ComputeType::CpuMultiThread(tc)) {
            Some(pool) => pool.install(|| {
                a.par_chunks(REDUCTION_BLOCK)
                    .zip(b.par_chunks(REDUCTION_BLOCK))
                    .map(|(x, y)| block_dot(x, y))
                    .collect()
            }),
            None => a
                .chunks(REDUCTION_BLOCK)
                .zip(b.chunks(REDUCTION_BLOCK))
                .map(|(x, y)| block_dot(x, y))
                .collect(),
        },
    };

    // Combine in ascending block order — the association that makes this
    // reproducible independent of how the blocks were scheduled.
    let mut total = 0.0_f64;
    for p in &partials {
        total += p;
    }
    total
}

// ── Internals ─────────────────────────────────────────────────────────────────

/// Build the dedicated pool for a backend, or `None` for the serial backend / a
/// failed pool construction.
fn build_pool(compute: ComputeType) -> Option<Arc<rayon::ThreadPool>> {
    match compute {
        ComputeType::CpuSingleThread => None,
        ComputeType::CpuMultiThread(tc) => rayon::ThreadPoolBuilder::new()
            .num_threads(tc.resolve())
            .build()
            .ok()
            .map(Arc::new),
    }
}

/// The three L1 partial sums one reduction block contributes to
/// [`ParallelLdu::normalised_residual`].
///
/// Kept as a plain `Copy` struct so the parallel map can `collect()` it into an
/// ordered `Vec` and the combine step can walk that `Vec` in block order.
#[derive(Debug, Clone, Copy, Default)]
struct NormBlock {
    /// `Σ |b - A x|` over the block.
    r_norm: f64,
    /// `Σ |A x|` over the block.
    ax_norm: f64,
    /// `Σ |b|` over the block.
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
