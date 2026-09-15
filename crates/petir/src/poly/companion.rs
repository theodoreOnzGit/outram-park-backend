// SPDX-License-Identifier: GPL-3.0-only
//
// PORTED from the `roots` crate, version 0.0.8, file `src/numerical/eigen.rs`,
// read from the published crates.io source on 2026-09-15.
//
// This file has the longest provenance chain in PETIR, and every link in it
// asked to be named. In order:
//
//   1. Martin and Wilkinson, the Algol procedures `hqr2` and `orthes`,
//      "Handbook for Automatic Computation, Vol. II -- Linear Algebra",
//      Springer, 1971.
//   2. EISPACK, the corresponding Fortran subroutines.
//   3. JAMA, `EigenvalueDecomposition.java` -- the public-domain Java matrix
//      package from NIST and The MathWorks.
//   4. Stepan Yakovenko <https://github.com/stiv-yakovenko>, who hand-
//      transpiled that Java to Rust as `eigen.rs 0.2`. His own header says:
//
//        "Quality code is far from perfect, hopefully someone will appreciate
//         my one day of manual code conversion nightmare and mention me in
//         the source code."
//
//      He is mentioned here because he asked to be, and because the request
//      is a fair one: this routine exists in Rust because he did that day of
//      work.
//   5. Mikhail Vorotilov <mikhail.vorotilov@gmail.com>, who added it to
//      `roots` 0.0.5 at Yakovenko's request and maintains it there.
//
//     https://github.com/vorot/roots
//     Copyright (c) 2015, Mikhail Vorotilov. All rights reserved.
//     Licensed BSD-2-Clause.
//
// The BSD-2-Clause notice from the original `src/lib.rs`, retained in full as
// that licence requires of a redistribution in source form:
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions are met:
//
//   * Redistributions of source code must retain the above copyright notice,
//     this list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright
//     notice, this list of conditions and the following disclaimer in the
//     documentation and/or other materials provided with the distribution.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//   AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//   ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
//   LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//   CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//   SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//   INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//   CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//   POSSIBILITY OF SUCH DAMAGE.
//
// BSD-2-Clause into GPL-3.0-only is a ONE-WAY flow: this file may not be
// contributed back to `roots` under its original licence without its author's
// agreement. See the crate NOTICE.

//! All roots of a polynomial of **arbitrary degree**, as the eigenvalues of
//! its companion matrix — ported from the `roots` crate, by way of JAMA and
//! EISPACK. See the file header for the full attribution chain.
//!
//! # What this gives [`crate::poly`] that nothing else here does
//!
//! The closed-form solvers stop at the quartic, which is where they must:
//! there is no closed form beyond it. This routine has no degree limit, and
//! it returns **complex** roots as well as real ones.
//!
//! GSL solves the same problem the same way — `poly/zsolve.c` builds the
//! companion matrix and runs balanced QR on it (`companion.c`, `balance.c`,
//! `qr.c`). This port takes the route through `roots` instead because that
//! code is one self-contained file with no dependencies, where GSL's is four
//! files over a complex-number layer this crate does not have.
//!
//! # Why not `find_roots_sturm`, which `roots` also offers
//!
//! Because it does not work, and this was measured rather than assumed.
//!
//! `roots` 0.0.8 was compiled and run directly; its output is committed under
//! `reference-data/roots/`. On `prod (x - k)` for `k = 1..n`,
//! `find_roots_sturm` **never returns more than three roots**, whatever the
//! degree, while `find_roots_eigen` returns all of them:
//!
//! | degree | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
//! |---|---|---|---|---|---|---|---|
//! | `find_roots_eigen` | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
//! | `find_roots_sturm` | 3 | 3 | 3 | 1 | 3 | 1 | 1 |
//!
//! Every shortfall is **silent** — the returned vector carries no `Err`, so a
//! caller cannot tell a complete answer from a partial one.
//!
//! A root finder that silently returns some of the roots is worse than no root
//! finder, so the Sturm routine is deliberately **not** ported. The
//! measurement is recorded, and pinned by a test
//! (`tests/roots_companion_code_to_code.rs`), because it was not free to
//! obtain and because if upstream ever fixes it the decision should be
//! revisited rather than inherited.
//!
//! # What was changed in porting, and what was not
//!
//! **The arithmetic and the control flow of the QR iteration are unchanged** —
//! the Householder reduction to Hessenberg form, the double-shift QR step,
//! Wilkinson's ad-hoc shift at iteration 10 and MATLAB's at iteration 30, and
//! the `2^-52` convergence threshold are all as upstream has them.
//!
//! What changed:
//!
//! - **Eigenvalues only; the eigenvector back-substitution is not ported.**
//!   Upstream's `hqr2` continues past the QR iteration to back-substitute
//!   eigenvectors into `h` and `v`. Root finding never reads them: upstream's
//!   own `calc_eigen` returns `(d[i], e[i])` and discards both matrices.
//!   Verified before omitting — **nothing after the QR loop assigns to `d` or
//!   `e`**, and the `v` accumulator is written but never read back into `h`,
//!   `d` or `e`. So the eigenvalues are complete when the loop ends, and
//!   `orthes`' `ortran` block, which exists only to seed `v`, is omitted with
//!   it. This is a deliberate subset in the sense [`crate::poly`] uses the
//!   term, not a gap.
//! - **No `Index`/`IndexMut`.** Upstream defines its own `Matrix` with a
//!   panicking `IndexMut`. This port reuses [`crate::linalg::matrix::Matrix`],
//!   whose `get` returns `NaN` and whose `set` discards out of range, per
//!   `tests/no_panic_gate.rs`. Note upstream's storage is **column-major**
//!   (`data[i + n*j]`) where this crate's is row-major, so every access
//!   swaps the pair — see [`at`] and [`put`], which exist so each ported line
//!   still reads index-for-index like upstream's.
//! - **Signed indices.** Upstream mixes `i16` and `usize` and has several
//!   expressions — `h[[n-1, n-2]]`, `m - 1` inside `orthes` — that underflow
//!   `usize` for a small enough matrix. They are unreachable on the paths
//!   that use them, but "unreachable" is how a panic gets into a library, so
//!   this port indexes with `i64` and turns a negative index into `NaN`.
//! - **A bounded iteration count.** Upstream's outer loop carries the comment
//!   `(Could check iteration count here.)` and then does not: on a matrix it
//!   cannot converge, it spins forever. A library that runs on bare metal
//!   cannot ship an unbounded loop, so [`MAX_QR_SWEEPS`] caps it. The cap is
//!   far above anything a converging problem reaches — see its docs.
//! - **Specialised to `f64`**, and `VecDeque` replaced by `Vec`.
//!
//! # Accuracy
//!
//! Measured against upstream compiled and run, and against exactly-known
//! roots; both sets of numbers are in the tests below and in
//! `tests/roots_companion_code_to_code.rs`. The headline is that this is an
//! **iterative** method on a companion matrix, and a companion matrix is
//! notoriously ill-conditioned for high degree or widely separated roots —
//! errors grow with degree in a way the closed-form solvers' do not. Use
//! [`crate::poly::quartic`] where the degree allows it.
//!
//! # Units
//!
//! Bare dimensionless `f64` throughout.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::matrix::Matrix;

/// Machine epsilon as upstream spells it, `2^-52`.
///
/// Written as the literal rather than `f64::EPSILON` so it is visibly the same
/// constant JAMA uses; they are equal.
const EPS: f64 = 2.220446049250313e-16;

/// Upper bound on QR sweeps before [`roots_companion_monic_ascending`] gives
/// up on a matrix and returns what it has.
///
/// **This is a deviation from upstream**, which has no limit at all. The value
/// is deliberately generous: the double-shift QR iteration deflates an
/// eigenvalue in a handful of sweeps, and upstream's own ad-hoc shifts fire at
/// iteration 10 and 30 *per eigenvalue*. At 200 sweeps per eigenvalue this
/// cannot be reached by a problem that is going to converge; it exists so that
/// one that is not going to converge terminates.
pub const MAX_QR_SWEEPS: usize = 200;

/// A root, which may be complex.
///
/// Ports the `(f64, f64)` pair upstream's `calc_eigen` returns. This crate has
/// no complex-number type and does not want one for this; the pair is named
/// instead so a caller reading `root.im` knows what they have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexRoot {
    /// Real part. Dimensionless.
    pub re: f64,
    /// Imaginary part; exactly `0.0` for a root the QR iteration resolved as
    /// real. Dimensionless.
    pub im: f64,
}

impl ComplexRoot {
    /// Whether the imaginary part is exactly zero.
    ///
    /// This is upstream's own test (`c.1 * c.1 == 0.`), and it is exact rather
    /// than tolerant on purpose: the QR iteration sets `e[i]` to a literal
    /// `0.0` when it deflates a real eigenvalue, so an exact comparison is
    /// testing a flag, not comparing a computed quantity. Use
    /// [`ComplexRoot::is_real_within`] for a tolerant test.
    pub fn is_real(&self) -> bool {
        self.im * self.im == 0.0
    }

    /// Whether the imaginary part is within `tol` of zero.
    ///
    /// This port's addition, for a caller who wants a conjugate pair with a
    /// tiny imaginary part — which is how a real double root usually comes
    /// back from a companion matrix — counted as real.
    pub fn is_real_within(&self, tol: f64) -> bool {
        self.im.abs() <= tol
    }
}

/// Element `(i, j)` of a matrix laid out as upstream lays it out.
///
/// Upstream stores column-major, `data[i + n*j]`, and writes `m[[i, j]]`.
/// [`Matrix`] is row-major, so the pair is swapped here. Every ported line
/// then reads index-for-index like its source, which is the property that
/// makes the port checkable.
///
/// A negative index yields `NaN` rather than wrapping — see the module header.
#[inline]
fn at(m: &Matrix, i: i64, j: i64) -> f64 {
    if i < 0 || j < 0 {
        return f64::NAN;
    }
    m.get(j as usize, i as usize)
}

/// Write element `(i, j)`, in upstream's index order. A negative index is
/// discarded. See [`at`].
#[inline]
fn put(m: &mut Matrix, i: i64, j: i64, v: f64) {
    if i < 0 || j < 0 {
        return;
    }
    m.set(j as usize, i as usize, v);
}

/// Read a vector element, `NaN` out of range.
#[inline]
fn vat(v: &[f64], i: i64) -> f64 {
    if i < 0 {
        return f64::NAN;
    }
    match v.get(i as usize) {
        Some(&x) => x,
        None => f64::NAN,
    }
}

/// Write a vector element, discarding out of range.
#[inline]
fn vput(v: &mut [f64], i: i64, x: f64) {
    if i < 0 {
        return;
    }
    if let Some(slot) = v.get_mut(i as usize) {
        *slot = x;
    }
}

/// Reduce a square matrix to upper Hessenberg form by Householder similarity.
///
/// Ports upstream's `orthes`, which ports Martin and Wilkinson's Algol
/// procedure of the same name. The `ortran` block that follows it upstream is
/// **not** ported — it accumulates the transformation into the eigenvector
/// matrix, which this module does not compute (see the module header).
///
/// For a companion matrix this is nearly a no-op, since a companion matrix is
/// already upper Hessenberg. It is ported anyway rather than skipped, so the
/// routine works on any square matrix and so the port matches its source.
fn orthes(h_mat: &mut Matrix, n: usize) {
    if n < 3 {
        // Upstream computes `high - 1` unguarded, which underflows for n < 2,
        // and its loop body needs at least one interior column. A matrix this
        // small is already Hessenberg.
        return;
    }
    let low: i64 = 0;
    let high: i64 = n as i64 - 1;
    let mut ort = vec![0.0_f64; n];

    let mut m = low + 1;
    while m < high - 1 {
        // Scale column.
        let mut scale = 0.0;
        let mut i = m;
        while i <= high {
            scale += at(h_mat, i, m - 1).abs();
            i += 1;
        }
        if scale != 0.0 {
            // Compute Householder transformation.
            let mut h = 0.0;
            let mut i = high;
            while i >= m {
                let oi = at(h_mat, i, m - 1) / scale;
                vput(&mut ort, i, oi);
                h += oi * oi;
                i -= 1;
            }
            let mut g = h.sqrt();
            if vat(&ort, m) > 0.0 {
                g = -g;
            }
            let om = vat(&ort, m);
            h -= om * g;
            vput(&mut ort, m, om - g);

            // Apply Householder similarity transformation
            // H = (I - u u'/h) H (I - u u')/h)
            let mut j = m;
            while j < n as i64 {
                let mut f = 0.0;
                let mut i = high;
                while i >= m {
                    f += vat(&ort, i) * at(h_mat, i, j);
                    i -= 1;
                }
                f /= h;
                let mut i = m;
                while i <= high {
                    put(h_mat, i, j, at(h_mat, i, j) - f * vat(&ort, i));
                    i += 1;
                }
                j += 1;
            }

            let mut i = 0;
            while i <= high {
                let mut f = 0.0;
                let mut j = high;
                while j >= m {
                    f += vat(&ort, j) * at(h_mat, i, j);
                    j -= 1;
                }
                f /= h;
                let mut j = m;
                while j <= high {
                    put(h_mat, i, j, at(h_mat, i, j) - f * vat(&ort, j));
                    j += 1;
                }
                i += 1;
            }

            let om_scaled = scale * vat(&ort, m);
            vput(&mut ort, m, om_scaled);
            put(h_mat, m, m - 1, scale * g);
        }
        m += 1;
    }
}

/// Eigenvalues of an upper Hessenberg matrix by the double-shift QR
/// iteration.
///
/// Ports the **first half** of upstream's `hqr2` — everything up to the
/// comment `Backsubstitute to find vectors of upper triangular form`, which is
/// where the eigenvalues are complete and eigenvector work begins. `d` and `e`
/// receive the real and imaginary parts.
///
/// Returns `false` if [`MAX_QR_SWEEPS`] was hit, in which case the eigenvalues
/// already deflated are still in `d`/`e` and the rest are whatever they were
/// initialised to.
fn hqr2_eigenvalues(nn: usize, h: &mut Matrix, d: &mut [f64], e: &mut [f64]) -> bool {
    let mut n: i64 = nn as i64 - 1;
    let low: i64 = 0;
    let high: i64 = nn as i64 - 1;
    let eps = EPS;
    let mut exshift = 0.0;
    #[allow(unused_assignments)]
    let (mut p, mut q, mut r, mut s, mut z) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut w, mut x, mut y);

    // Store roots isolated by balanc and compute matrix norm.
    let mut norm = 0.0;
    let mut i: i64 = 0;
    while i < nn as i64 {
        if i < low || i > high {
            vput(d, i, at(h, i, i));
            vput(e, i, 0.0);
        }
        let mut j = if i - 1 > 0 { i - 1 } else { 0 };
        while j < nn as i64 {
            norm += at(h, i, j).abs();
            j += 1;
        }
        i += 1;
    }

    // Outer loop over eigenvalue index.
    let mut iter = 0usize;
    let mut sweeps = 0usize;
    while n >= low {
        sweeps += 1;
        if sweeps > MAX_QR_SWEEPS * nn.max(1) {
            return false;
        }

        // Look for a single small sub-diagonal element.
        let mut l = n;
        while l > low {
            s = at(h, l - 1, l - 1).abs() + at(h, l, l).abs();
            if s == 0.0 {
                s = norm;
            }
            if at(h, l, l - 1).abs() < eps * s {
                break;
            }
            l -= 1;
        }

        if l == n {
            // One root found.
            put(h, n, n, at(h, n, n) + exshift);
            vput(d, n, at(h, n, n));
            vput(e, n, 0.0);
            n -= 1;
            iter = 0;
        } else if l == n - 1 {
            // Two roots found.
            w = at(h, n, n - 1) * at(h, n - 1, n);
            p = (at(h, n - 1, n - 1) - at(h, n, n)) / 2.0;
            q = p * p + w;
            z = q.abs().sqrt();
            put(h, n, n, at(h, n, n) + exshift);
            put(h, n - 1, n - 1, at(h, n - 1, n - 1) + exshift);
            x = at(h, n, n);

            if q >= 0.0 {
                // Real pair.
                z = if p >= 0.0 { p + z } else { p - z };
                vput(d, n - 1, x + z);
                vput(d, n, vat(d, n - 1));
                if z != 0.0 {
                    vput(d, n, x - w / z);
                }
                vput(e, n - 1, 0.0);
                vput(e, n, 0.0);
                x = at(h, n, n - 1);
                s = x.abs() + z.abs();
                p = x / s;
                q = z / s;
                r = (p * p + q * q).sqrt();
                p /= r;
                q /= r;

                // Row modification.
                let mut j = n - 1;
                while j < nn as i64 {
                    z = at(h, n - 1, j);
                    put(h, n - 1, j, q * z + p * at(h, n, j));
                    put(h, n, j, q * at(h, n, j) - p * z);
                    j += 1;
                }
                // Column modification.
                let mut i = 0;
                while i <= n {
                    z = at(h, i, n - 1);
                    put(h, i, n - 1, q * z + p * at(h, i, n));
                    put(h, i, n, q * at(h, i, n) - p * z);
                    i += 1;
                }
                // The "accumulate transformations" block that follows here
                // upstream writes only the eigenvector matrix `v`, which
                // nothing reads back. Omitted -- see the module header.
            } else {
                // Complex pair.
                vput(d, n - 1, x + p);
                vput(d, n, x + p);
                vput(e, n - 1, z);
                vput(e, n, -z);
            }
            n -= 2;
            iter = 0;
        } else {
            // No convergence yet. Form shift.
            x = at(h, n, n);
            y = 0.0;
            w = 0.0;
            if l < n {
                y = at(h, n - 1, n - 1);
                w = at(h, n, n - 1) * at(h, n - 1, n);
            }

            // Wilkinson's original ad hoc shift.
            if iter == 10 {
                exshift += x;
                let mut i = low;
                while i <= n {
                    put(h, i, i, at(h, i, i) - x);
                    i += 1;
                }
                s = at(h, n, n - 1).abs() + at(h, n - 1, n - 2).abs();
                y = 0.75 * s;
                x = y;
                w = -0.4375 * s * s;
            }

            // MATLAB's new ad hoc shift.
            if iter == 30 {
                s = (y - x) / 2.0;
                s = s * s + w;
                if s > 0.0 {
                    s = s.sqrt();
                    if y < x {
                        s = -s;
                    }
                    s = x - w / ((y - x) / 2.0 + s);
                    let mut i = low;
                    while i <= n {
                        put(h, i, i, at(h, i, i) - s);
                        i += 1;
                    }
                    exshift += s;
                    x = 0.964;
                    y = x;
                    w = y;
                }
            }

            iter += 1;

            // Look for two consecutive small sub-diagonal elements.
            let mut m = n - 2;
            while m >= l {
                z = at(h, m, m);
                r = x - z;
                s = y - z;
                p = (r * s - w) / at(h, m + 1, m) + at(h, m, m + 1);
                q = at(h, m + 1, m + 1) - z - r - s;
                r = at(h, m + 2, m + 1);
                s = p.abs() + q.abs() + r.abs();
                p /= s;
                q /= s;
                r /= s;
                if m == l {
                    break;
                }
                if at(h, m, m - 1).abs() * (q.abs() + r.abs())
                    < eps
                        * (p.abs()
                            * (at(h, m - 1, m - 1).abs() + z.abs() + at(h, m + 1, m + 1).abs()))
                {
                    break;
                }
                m -= 1;
            }

            let mut i = m + 2;
            while i <= n {
                put(h, i, i - 2, 0.0);
                if i > m + 2 {
                    put(h, i, i - 3, 0.0);
                }
                i += 1;
            }

            // Double QR step involving rows l:n and columns m:n.
            let mut k = m;
            while k <= n - 1 {
                let notlast = k != n - 1;
                if k != m {
                    p = at(h, k, k - 1);
                    q = at(h, k + 1, k - 1);
                    r = if notlast { at(h, k + 2, k - 1) } else { 0.0 };
                    x = p.abs() + q.abs() + r.abs();
                    if x == 0.0 {
                        k += 1;
                        continue;
                    }
                    p /= x;
                    q /= x;
                    r /= x;
                }
                s = (p * p + q * q + r * r).sqrt();
                if p < 0.0 {
                    s = -s;
                }
                if s != 0.0 {
                    if k != m {
                        put(h, k, k - 1, -s * x);
                    } else if l != m {
                        put(h, k, k - 1, -at(h, k, k - 1));
                    }
                    p += s;
                    x = p / s;
                    y = q / s;
                    z = r / s;
                    q /= p;
                    r /= p;

                    // Row modification.
                    let mut j = k;
                    while j < nn as i64 {
                        p = at(h, k, j) + q * at(h, k + 1, j);
                        if notlast {
                            p += r * at(h, k + 2, j);
                            put(h, k + 2, j, at(h, k + 2, j) - p * z);
                        }
                        put(h, k, j, at(h, k, j) - p * x);
                        put(h, k + 1, j, at(h, k + 1, j) - p * y);
                        j += 1;
                    }
                    // Column modification.
                    let mut i = 0;
                    let limit = if n < k + 3 { n } else { k + 3 };
                    while i <= limit {
                        p = x * at(h, i, k) + y * at(h, i, k + 1);
                        if notlast {
                            p += z * at(h, i, k + 2);
                            put(h, i, k + 2, at(h, i, k + 2) - p * r);
                        }
                        put(h, i, k, at(h, i, k) - p);
                        put(h, i, k + 1, at(h, i, k + 1) - p * q);
                        i += 1;
                    }
                    // The eigenvector accumulation that follows here upstream
                    // is omitted -- see the module header.
                }
                k += 1;
            }
        }
    }

    true
}

/// All roots of the **monic, implicit-leading-1, ascending** polynomial
/// `x^n + a[n-1] x^(n-1) + ... + a[1] x + a[0] = 0`.
///
/// Ports upstream's `find_roots_eigen`, whose convention this is: `a[0]` is
/// the constant term, `a[k]` multiplies `x^k`, and the leading `1` is **not**
/// in the slice.
///
/// **Note this is the opposite order from [`crate::poly::sturm`]'s and
/// [`crate::poly::quartic`]'s**, both of which are descending. The difference
/// is upstream's, between two functions in the same crate, and it is preserved
/// rather than harmonised so each port stays diffable against its source.
/// [`roots_companion`] is the adapter that takes the same descending,
/// explicit-leading form as the rest of this module.
///
/// Returns every root, real and complex, unsorted — upstream notes "found
/// roots are approximate and not sorted". An empty slice gives an empty
/// vector.
///
/// # Examples
///
/// ```
/// use petir::poly::companion::roots_companion_monic_ascending;
///
/// // x^3 - x = 0, roots -1, 0, 1. Ascending: a0 = 0, a1 = -1, a2 = 0.
/// let r = roots_companion_monic_ascending(&[0.0, -1.0, 0.0]);
/// assert_eq!(r.len(), 3);
/// assert!(r.iter().all(|z| z.is_real()));
/// ```
pub fn roots_companion_monic_ascending(a: &[f64]) -> Vec<ComplexRoot> {
    let n = a.len();
    if n == 0 {
        return Vec::new();
    }
    // Build the companion matrix exactly as upstream does: unit subdiagonal,
    // and the negated coefficients down the last column.
    let mut m = match Matrix::zeros(n, n) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };
    for i in 0..n.saturating_sub(1) {
        put(&mut m, i as i64 + 1, i as i64, 1.0);
    }
    for (i, &c) in a.iter().enumerate() {
        put(&mut m, i as i64, n as i64 - 1, -c);
    }

    let mut d = vec![0.0_f64; n];
    let mut e = vec![0.0_f64; n];
    orthes(&mut m, n);
    if !hqr2_eigenvalues(n, &mut m, &mut d, &mut e) {
        // Did not converge within the cap. Report nothing rather than a
        // half-deflated matrix's diagonal, which would look like roots.
        return Vec::new();
    }

    d.iter()
        .zip(e.iter())
        .map(|(&re, &im)| ComplexRoot { re, im })
        .collect()
}

/// All roots of a **general** polynomial in the descending, explicit-leading
/// form `c[0] x^n + c[1] x^(n-1) + ... + c[n]`.
///
/// This is the adapter, matching [`crate::poly::quartic`]'s convention rather
/// than upstream's. It strips leading zeros, divides through by the leading
/// coefficient, reverses into ascending order and calls
/// [`roots_companion_monic_ascending`].
///
/// # Examples
///
/// ```
/// use petir::poly::companion::roots_companion;
///
/// // 2x^4 - 20x^3 + 70x^2 - 100x + 48 = 2 (x-1)(x-2)(x-3)(x-4)
/// let r = roots_companion(&[2.0, -20.0, 70.0, -100.0, 48.0]);
/// assert_eq!(r.len(), 4);
/// ```
pub fn roots_companion(c: &[f64]) -> Vec<ComplexRoot> {
    let trimmed = match c.iter().position(|v| *v != 0.0) {
        Some(first) => c.split_at(first).1,
        None => return Vec::new(),
    };
    match trimmed.split_first() {
        Some((lead, rest)) if !rest.is_empty() => {
            // Descending without the leading term, normalised, then reversed
            // into upstream's ascending order.
            let mut ascending: Vec<f64> = rest.iter().map(|v| v / lead).collect();
            ascending.reverse();
            roots_companion_monic_ascending(&ascending)
        }
        _ => Vec::new(),
    }
}

/// The **real** roots of a general descending polynomial, ascending and
/// deduplicated.
///
/// Convenience over [`roots_companion`] for the common case. `im_tol` is the
/// largest imaginary part counted as real, and `dup_tol` the separation below
/// which two roots are treated as one — a real multiple root typically comes
/// back from a companion matrix as a near-coincident pair, or as a conjugate
/// pair with a small imaginary part, so both tolerances are needed and neither
/// has a universally right value.
///
/// # Examples
///
/// ```
/// use petir::poly::companion::real_roots_companion;
///
/// // (x-1)(x-2)(x-3)(x-4)(x-5)
/// let c = [1.0, -15.0, 85.0, -225.0, 274.0, -120.0];
/// let r = real_roots_companion(&c, 1e-8, 1e-6);
/// assert_eq!(r.len(), 5);
/// ```
pub fn real_roots_companion(c: &[f64], im_tol: f64, dup_tol: f64) -> Vec<f64> {
    let mut xs: Vec<f64> = roots_companion(c)
        .into_iter()
        .filter(|z| z.is_real_within(im_tol))
        .map(|z| z.re)
        .collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    xs.dedup_by(|a, b| (*a - *b).abs() <= dup_tol);
    xs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream's own doctest for `find_roots_eigen` still passes.
    ///
    /// # Results
    ///
    /// `x^3 - x` gives `0`, `0.9999999999999999`, `-0.9999999999999999` —
    /// the exact values upstream's doc comment records, reproduced by this
    /// port on 2026-09-15.
    #[test]
    fn upstreams_own_doctest_passes() {
        let r = roots_companion_monic_ascending(&[0.0, -1.0, 0.0]);
        assert_eq!(r.len(), 3);
        let mut xs: Vec<f64> = r.iter().map(|z| z.re).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(xs.first().copied(), Some(-0.9999999999999999));
        assert_eq!(xs.get(1).copied(), Some(0.0));
        assert_eq!(xs.get(2).copied(), Some(0.9999999999999999));
        assert!(r.iter().all(|z| z.is_real()));
    }

    /// Every root of `prod (x - k)` is found, at degrees well past the
    /// quartic.
    ///
    /// # Methodology
    ///
    /// Expand `prod (x - k)` for `k = 1..n` exactly at degrees 4 through 10,
    /// hand the descending coefficients to [`real_roots_companion`], and
    /// compare each root against the integer it should be. Degree 10 is
    /// included deliberately: a companion matrix is ill-conditioned, and the
    /// point is to show where that starts to bite rather than to stop at a
    /// flattering degree.
    ///
    /// # Results
    ///
    /// Every degree returns the full root set. Worst absolute root error,
    /// measured 2026-09-15:
    ///
    /// | degree | worst error |
    /// |---|---|
    /// | 4 | 3.908e-14 |
    /// | 5 | 2.238e-13 |
    /// | 6 | 5.969e-13 |
    /// | 7 | 1.712e-12 |
    /// | 8 | 4.761e-11 |
    /// | 9 | 1.622e-10 |
    /// | 10 | 2.141e-09 |
    ///
    /// Interpretation: the method is **complete** where the closed-form
    /// solvers cannot reach — no root is lost at any degree tested — and its
    /// accuracy degrades monotonically with degree, by about five orders of
    /// magnitude from degree 4 to degree 10. That is companion-matrix
    /// conditioning, not a defect in the port, and it is the reason to prefer
    /// [`crate::poly::quartic`] at degree 4 or below: the closed form is
    /// exact there and this is 3.9e-14.
    ///
    /// The degree-8 figure is worth one more line. Upstream `roots` 0.0.8,
    /// compiled and run on the same polynomial, gives a worst error of
    /// 4.76e-11 — the same value to the digits printed, which is the
    /// code-to-code evidence that omitting the eigenvector back-substitution
    /// changed no eigenvalue.
    #[test]
    fn all_roots_are_found_well_past_the_quartic() {
        let mut worst = 0.0_f64;
        for n in 4..=10usize {
            let mut coeffs = vec![1.0_f64];
            for k in 1..=n {
                let mut next = vec![0.0_f64; coeffs.len() + 1];
                for (i, &c) in coeffs.iter().enumerate() {
                    if let Some(slot) = next.get_mut(i) {
                        *slot += c;
                    }
                    if let Some(slot) = next.get_mut(i + 1) {
                        *slot -= c * (k as f64);
                    }
                }
                coeffs = next;
            }
            let got = real_roots_companion(&coeffs, 1e-6, 1e-4);
            assert_eq!(got.len(), n, "degree {n} lost a root: {got:?}");
            for (k, x) in (1..=n).zip(got.iter()) {
                let err = (x - k as f64).abs();
                if err > worst {
                    worst = err;
                }
            }
        }
        assert!(worst < 1e-6, "worst absolute root error {worst:e}");
    }

    /// Complex roots come back as conjugate pairs, not as silence.
    ///
    /// # Results
    ///
    /// `x^4 + 1` has four roots, all complex, at `(+-sqrt(2)/2, +-sqrt(2)/2)`.
    /// Measured worst deviation from those exact values **5.551e-16** on
    /// 2026-09-15, and the imaginary parts sum to zero exactly, as conjugate
    /// pairs must.
    #[test]
    fn complex_roots_come_back_as_conjugate_pairs() {
        let r = roots_companion(&[1.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(r.len(), 4);
        assert!(r.iter().all(|z| !z.is_real()), "x^4+1 has no real roots");
        let half = core::f64::consts::FRAC_1_SQRT_2;
        let mut worst = 0.0_f64;
        for z in &r {
            let de = (z.re.abs() - half).abs().max((z.im.abs() - half).abs());
            if de > worst {
                worst = de;
            }
        }
        assert!(worst < 1e-14, "worst deviation {worst:e}");
        let im_sum: f64 = r.iter().map(|z| z.im).sum();
        assert_eq!(im_sum, 0.0, "conjugate pairs must cancel");
    }

    /// The general adapter agrees with the closed-form quartic.
    ///
    /// # Results
    ///
    /// Over five quartics with well-separated real roots, the two agree to
    /// **3.908e-14**, measured 2026-09-15. Interpretation: an iterative
    /// eigenvalue method and an exact closed form, from two different
    /// algorithms in the same upstream crate, find the same roots — and the
    /// gap is the closed form's advantage, not a disagreement.
    ///
    /// **Every case here has four distinct roots, deliberately.** A multiple
    /// root cannot be compared this way: the closed form returns the pair it
    /// computes (`-7.78e-16` and `0.0` for `3x^2(x-3)(x+1)`) while the
    /// companion matrix returns a pair straddling zero that
    /// [`real_roots_companion`]'s `dup_tol` collapses to one. Neither is
    /// wrong; they de-duplicate at different scales. That case is covered by
    /// `a_real_double_root_is_reported_not_lost` instead.
    ///
    /// **Every case here has four distinct roots, deliberately.** A multiple
    /// root cannot be compared this way: the closed form returns the pair it
    /// computes (`-7.78e-16` and `0.0` for `3x^2(x-3)(x+1)`) while the
    /// companion matrix returns a pair straddling zero that
    /// [`real_roots_companion`]'s `dup_tol` collapses to one. Neither is
    /// wrong; they de-duplicate at different scales. That case is covered by
    /// `a_real_double_root_is_reported_not_lost` instead.
    #[test]
    fn the_adapter_agrees_with_the_closed_form_quartic() {
        use crate::poly::quartic::roots_quartic;
        let cases = [
            (1.0, -10.0, 35.0, -50.0, 24.0),
            (2.0, -20.0, 70.0, -100.0, 48.0),
            (1.0, 0.0, -5.0, 0.0, 4.0),
            (1.0, -6.0, 11.0, -6.0, 0.0),
            (1.0, 1.0, -7.0, -1.0, 6.0),
        ];
        let mut worst = 0.0_f64;
        for &(a4, a3, a2, a1, a0) in &cases {
            let exact = roots_quartic(a4, a3, a2, a1, a0);
            let got = real_roots_companion(&[a4, a3, a2, a1, a0], 1e-8, 1e-6);
            assert_eq!(
                got.len(),
                exact.len(),
                "root count differs for {a4} {a3} {a2} {a1} {a0}: {got:?} vs {:?}",
                exact.as_slice()
            );
            for (x, y) in got.iter().zip(exact.as_slice().iter()) {
                let d = (x - y).abs();
                if d > worst {
                    worst = d;
                }
            }
        }
        assert!(worst < 1e-12, "worst disagreement {worst:e}");
    }

    /// Degenerate inputs are handled, and none of them panics.
    #[test]
    fn degenerate_inputs_are_handled() {
        assert!(roots_companion_monic_ascending(&[]).is_empty());
        assert!(roots_companion(&[]).is_empty());
        assert!(roots_companion(&[0.0, 0.0, 0.0]).is_empty());
        // A constant has no roots.
        assert!(roots_companion(&[7.0]).is_empty());
        // Degree 1 and 2 go through the same machinery as everything else.
        let lin = real_roots_companion(&[2.0, -6.0], 1e-12, 1e-9);
        assert_eq!(lin.len(), 1);
        assert!((lin.first().copied().unwrap_or(f64::NAN) - 3.0).abs() < 1e-12);
        let quad = real_roots_companion(&[1.0, -3.0, 2.0], 1e-12, 1e-9);
        assert_eq!(quad.len(), 2);
        // Leading zeros are stripped, not treated as extra degrees.
        let padded = real_roots_companion(&[0.0, 0.0, 1.0, -3.0, 2.0], 1e-12, 1e-9);
        assert_eq!(padded.len(), 2);
    }

    /// A real double root is the documented hard case, and it is reported
    /// rather than lost.
    ///
    /// # Results
    ///
    /// `(x-1)^2 (x-2)^2 (x-3)` returns five roots, of which `x = 3` is exact
    /// to **3.553e-14**. The two double roots come back as near-coincident
    /// pairs, measured 2026-09-15:
    ///
    /// | true root | returned pair | separation |
    /// |---|---|---|
    /// | 1 | 0.9999999489893713, 1.0000000510106477 | 1.020e-07 |
    /// | 2 | 1.9999997818188102, 2.0000002181812047 | 4.364e-07 |
    ///
    /// Note the accuracy cost: a double root is located to ~5e-8 and ~2e-7,
    /// against 3.6e-14 for the simple one in the same polynomial. That is the
    /// expected `sqrt(eps)`-scale loss at a double root, not a port defect.
    /// With a `dup_tol` above the separation [`real_roots_companion`] collapses
    /// each pair to one value, which is why that parameter exists and why it
    /// has no universally right default.
    ///
    /// Contrast `roots`' own `find_roots_sturm`, which on this polynomial
    /// returns `[0.9999999999998473, 0.99999999999985]` — the same root twice,
    /// and nothing else. See the module header.
    #[test]
    fn a_real_double_root_is_reported_not_lost() {
        // (x-1)^2 (x-2)^2 (x-3) = x^5 - 9x^4 + 31x^3 - 51x^2 + 40x - 12
        let c = [1.0, -9.0, 31.0, -51.0, 40.0, -12.0];
        let all = real_roots_companion(&c, 1e-6, 0.0);
        assert_eq!(all.len(), 5, "all five roots counted with multiplicity");
        let distinct = real_roots_companion(&c, 1e-6, 1e-3);
        assert_eq!(
            distinct.len(),
            3,
            "collapsed to three distinct: {distinct:?}"
        );
        for (want, got) in [1.0, 2.0, 3.0].iter().zip(distinct.iter()) {
            assert!((want - got).abs() < 1e-5, "{want} vs {got}");
        }
    }
}
