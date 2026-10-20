// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, read from the published
// crates.io source (faer-0.24.4 in the cargo registry) on 2026-10-20.
//
//     https://codeberg.org/sarah-quinones/faer
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors
//     Licensed MIT (faer's Cargo.toml `license = "MIT"`; the published crate
//     ships no LICENSE file, so the standard MIT text is reproduced here).
//
// Its notice, retained as that licence requires of a redistribution in source
// form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// Also ported, same author and licence: `faer-traits` 0.24.0 (faer's scalar
// `hypot`) and `pulp` 0.22.3 (the SIMD lane semantics), both "Copyright (c)
// sarah quiñones", MIT (`license = "MIT"` in each Cargo.toml).
//
// MIT into AGPL-3.0-only is a ONE-WAY flow: this file may not be contributed
// back to `faer` under its original licence without its authors' agreement.
// Maintainer direction for this port: gh #996, 2026-10-20 ("Whatever faer
// does, port into petir"). See the crate NOTICE and CLAUDE.md.

//! **faer's dense SVD**, ported so that its results are reproduced bit for
//! bit without depending on faer.
//!
//! # Why this exists next to [`crate::linalg::svd`]
//!
//! `outram-blender`'s ARAP solver (`arap::closest_rotation`) took the
//! closest rotation `R = U Vᵀ` of a `3 x 3` covariance from `faer`'s
//! `Mat::svd()`. PETIR already has GSL's Golub–Reinsch SVD
//! ([`crate::linalg::svd`], gh #993), but it is a different algorithm with a
//! different rounding and different sign conventions for `U` and `V`.
//! **Maintainer direction on gh #996, 2026-10-20: "Whatever faer does, port
//! into petir."** So this module ports the faer code path that call actually
//! runs, and `outram-blender` drops faer. The two SVDs are not merged and must
//! not be: each is pinned to its own upstream.
//!
//! # What is ported (faer 0.24.4, real `f64`, square, `n <= 16`)
//!
//! | here | faer |
//! |---|---|
//! | [`svd_square`], [`svd3`] | `Svd::new` -> `linalg::svd::svd` -> `svd_imp` |
//! | bidiagonalisation | `svd/bidiag.rs` `bidiag_in_place`, `bidiag_fused_op_simd` |
//! | bidiagonal SVD | `svd/bidiag_svd.rs` `qr_algorithm` (faer uses it below `n = 128`) |
//! | back-transformation | `householder.rs` block-size-1 reflector application |
//! | kernels | dot products, `matvec`, `norm_l2`, `norm_max`, `make_householder`, Givens rotations |
//! | lane model | `pulp` 0.22.3 `V3`: 4 lanes, `vfmadd`, `(l0 + l2) + (l1 + l3)` |
//!
//! faer chooses its Householder block size from the matrix size; up to
//! `16 x 16` it is 1, and that is the only regime ported. Above it faer
//! applies blocked reflectors through its GEMM kernels. [`svd_square`]
//! returns [`crate::PetirError::Invalid`] for `n > 16` rather than silently
//! running a different algorithm.
//!
//! # Bit-faithfulness, and to which faer
//!
//! faer's kernels are SIMD, and the order of a horizontal sum depends on the
//! instruction set faer picks at run time. This port models **one** of them,
//! `pulp`'s `V3` (x86-64 with AVX2 and FMA), in plain scalar code (the private `simd` module
//! documents the model). There is no `core::arch` and no CPU detection, so
//! the port gives the **same bits on every target**, and those bits are what
//! faer gives on an AVX2+FMA x86-64. faer itself on another instruction set
//! (AVX-512 under its `nightly` feature, aarch64, SSE-only x86) can differ in
//! the last bits; the port does not follow it there.
//!
//! `tests/faer_dense_svd_vs_faer.rs` pins it: `U`, `s` and `V` bit-identical
//! to faer's own compiled `Mat::svd()` (a dev-dependency) on every `3 x 3`
//! covariance `outram-blender`'s test suite hands `closest_rotation`, on
//! random matrices of every order 1..=16, and on structured edge cases.
//!
//! # Deviations
//!
//! - **Errors.** faer returns one `SvdError::NoConvergence` for both a
//!   non-finite bidiagonal and an exhausted iteration budget. Here the first
//!   is [`crate::PetirError::Domain`] and the second
//!   [`crate::PetirError::MaxIterations`].
//! - **Scope.** Square input, `1 <= n <= 16`, full `U` and `V`. faer's
//!   rectangular arm (QR first when `m / n > 11/6`), thin vectors,
//!   divide-and-conquer (`n >= 128`), complex arms and `rayon` paths are not
//!   ported; none is reached by a `3 x 3` real call.

mod kernels;
mod simd;
mod svd;

pub use svd::{svd3, svd_square, FaerSvd, MAX_ORDER};
