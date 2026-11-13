# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""What operation order does numpy's linear algebra actually use? (gh:#1191)

FilterPy is numpy: every `dot` is an OpenBLAS gemm / gemv / ddot, `inv` is
LAPACK dgesv, and `cholesky` (scipy) is dpotrf. Bit-identity with FilterPy
therefore needs each kernel's rounding order. That order was not read from
OpenBLAS source; it was MEASURED here, by comparing numpy's result bit for
bit with candidate orders computed exactly (`fractions.Fraction`, so an
emulated fused multiply-add rounds once). The Rust kernels in
crates/raffles/src/filtering/numpy_ops.rs implement the orders found.

    <venv>/bin/python -I crates/raffles/verification_and_validation/filterpy_gh1191/probe_numpy_kernels.py

Measured 2026-11-04 with numpy 2.4.6 / scipy 1.17.1 on OpenBLAS 0.3.31
(DYNAMIC_ARCH, core SkylakeX). Another core (Haswell, Zen) may dispatch
different kernels; the reference JSON records the core it was made on.
"""
from fractions import Fraction as Fr

import numpy as np
import scipy.linalg as sl


def fma(a, b, c):
    return float(Fr(a) * Fr(b) + Fr(c))


def chain(p, q, s=0.0):
    for a, b in zip(p, q):
        s = fma(a, b, s)
    return s


def gemm(A, B):  # every element: one fma chain over k, from 0
    return np.array([[chain(A[i, :], B[:, j]) for j in range(B.shape[1])] for i in range(A.shape[0])])


def gemvt_tail(p, q, y):
    k = len(p)
    if k == 0:
        return y
    if k == 1:
        return fma(p[0], q[0], y)
    if k == 2:
        return y + fma(p[0], q[0], p[1] * q[1])
    return y + fma(p[2], q[2], fma(p[0], q[0], p[1] * q[1]))


def gemvt(A, x):  # A (m, k) @ x (k): m == 1 is ddot; else 4-lane blocks + tail
    m, k = A.shape
    if m == 1:
        return np.array([chain(A[0], x)])
    m1 = k - k % 4
    out = []
    for r in range(m):
        y = 0.0
        if m1:
            lanes = [0.0] * 4
            for i in range(0, m1, 4):
                for lane in range(4):
                    lanes[lane] = fma(A[r, i + lane], x[i + lane], lanes[lane])
            y = (lanes[0] + lanes[2]) + (lanes[1] + lanes[3])
        out.append(gemvt_tail(A[r, m1:], x[m1:], y))
    return np.array(out)


def gemvn(w, S):  # w (k) @ S (k, m), m in 2..3
    k, m = S.shape
    out = []
    for j in range(m):
        t, i = 0.0, 0
        while i < (k & -4):
            t = t + fma(S[i, j], w[i], S[i + 1, j] * w[i + 1])
            t = t + fma(S[i + 2, j], w[i + 2], S[i + 3, j] * w[i + 3])
            i += 4
        while i < k:
            t = fma(S[i, j], w[i], t)
            i += 1
        out.append(t)
    return np.array(out)


def chol_upper(A):  # dpotf2, upper: ddot for the diagonal, gemv(alpha=-1) folded into x
    n = A.shape[0]
    U = np.zeros((n, n))
    for j in range(n):
        ajj = np.sqrt(A[j, j] - chain(U[:j, j], U[:j, j]))
        U[j, j] = ajj
        rinv = 1.0 / ajj
        for i in range(j + 1, n):
            y = A[j, i] if j == 0 else gemvt_tail(list(U[:j, i]), [-v for v in U[:j, j]], A[j, i])
            U[j, i] = y * rinv
    return U


def lu(A0):  # dgetf2, left-looking
    n = A0.shape[0]
    a = A0.copy()
    piv = list(range(n))
    for j in range(n):
        b = [a[i, j] for i in range(n)]
        for i in range(j):
            if piv[i] != i:
                b[i], b[piv[i]] = b[piv[i]], b[i]
        for i in range(1, j):
            b[i] = b[i] - chain(a[i, :i], b[:i])
        for r in range(j, n):
            b[r] = b[r] - chain(a[r, :j], b[:j])
        jp = j + int(np.argmax([abs(v) for v in b[j:]]))
        piv[j] = jp
        for i in range(n):
            a[i, j] = b[i]
        t1 = 1.0 / b[jp]
        if jp != j:
            for c in range(j + 1):
                a[j, c], a[jp, c] = a[jp, c], a[j, c]
        for r in range(j + 1, n):
            a[r, j] = a[r, j] * t1
    return a, piv


def blocks(m, forward):  # trsm splits m < 16 into power-of-two blocks
    out, r = [], 0
    if forward:
        i = 8
        while i > 0:
            if m & i:
                out.append((r, i))
                r += i
            i >>= 1
    else:
        i = 1
        while i < 16:
            if m & i:
                out.append(((m & ~(i - 1)) - i, i))
            i *= 2
    return out


def inv(A):  # dgesv on the identity: getf2, then trsm with inverse diagonals
    n = A.shape[0]
    a, piv = lu(A)
    X = np.eye(n)
    for i in range(n):
        if piv[i] != i:
            X[[i, piv[i]]] = X[[piv[i], i]]
    for col in range(n):
        c = list(X[:, col])
        for r0, sz in blocks(n, True):
            for r in range(r0, r0 + sz):
                if r0 > 0:
                    c[r] = fma(-1.0, chain(a[r, :r0], c[:r0]), c[r])
            for i in range(r0, r0 + sz):
                for k in range(i + 1, r0 + sz):
                    c[k] = fma(-c[i], a[k, i], c[k])
        for r0, sz in blocks(n, False):
            for r in range(r0, r0 + sz):
                if r0 + sz < n:
                    c[r] = fma(-1.0, chain(a[r, r0 + sz:], c[r0 + sz:]), c[r])
            for i in range(r0 + sz - 1, r0 - 1, -1):
                c[i] = c[i] * (1.0 / a[i, i])
                for k in range(r0, i):
                    c[k] = fma(-c[i], a[k, i], c[k])
        X[:, col] = c
    return X


def pairwise(a):  # numpy's pairwise_sum (loops_utils.h.src), from 0
    n = len(a)
    if n < 8:
        r = 0.0
        for v in a:
            r += v
        return r
    if n <= 128:
        r, i = list(a[:8]), 8
        while i < n - n % 8:
            for j in range(8):
                r[j] += a[i + j]
            i += 8
        res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]))
        while i < n:
            res += a[i]
            i += 1
        return res
    n2 = n // 2
    n2 -= n2 % 8
    return pairwise(a[:n2]) + pairwise(a[n2:])


def sequential_sum(stack):  # numpy's reduction over a non-contiguous axis 0
    acc = stack[0].copy()
    for row in stack[1:]:
        acc = acc + row
    return acc


def main():
    rng = np.random.default_rng(20261104)
    T = 200

    def count(f):
        return sum(bool(f()) for _ in range(T))

    rows = []
    for m, k, n in [(2, 2, 2), (3, 3, 3), (5, 5, 5), (3, 7, 3), (2, 4, 2), (4, 4, 4)]:
        rows.append((f"gemm {m}x{k} @ {k}x{n}", count(lambda: np.array_equal(
            np.dot(A := rng.standard_normal((m, k)), B := rng.standard_normal((k, n))), gemm(A, B)))))
    for m, k in [(2, 1), (2, 2), (3, 3), (4, 4), (2, 3), (3, 2), (4, 9), (1, 7), (1, 15)]:
        rows.append((f"gemv {m}x{k} @ {k}x1", count(lambda: np.array_equal(
            np.dot(A := rng.standard_normal((m, k)), x := rng.standard_normal((k, 1)))[:, 0], gemvt(A, x[:, 0])))))
    for k, m in [(3, 2), (5, 2), (7, 3), (21, 3), (101, 2), (3, 1), (7, 1)]:
        rows.append((f"1-D({k}) @ {k}x{m}", count(lambda: np.array_equal(
            np.dot(w := rng.standard_normal(k), S := rng.standard_normal((k, m))),
            gemvn(w, S) if m > 1 else np.array([chain(S[:, 0], w)])))))
    for n in [1, 2, 3, 4, 5]:
        def spd():
            A = rng.standard_normal((n, n))
            return A @ A.T + 0.1 * np.eye(n)
        rows.append((f"inv {n}x{n}", count(lambda: np.array_equal(np.linalg.inv(S := spd()), inv(S)))))
        if n <= 4:
            rows.append((f"cholesky {n}x{n}", count(lambda: np.array_equal(sl.cholesky(S := spd()), chol_upper(S)))))
    for N in [20, 50]:
        rows.append((f"mean axis 0, N={N}", count(lambda: np.array_equal(
            np.mean(S := rng.standard_normal((N, 3)), axis=0), sequential_sum(S) / N))))
        rows.append((f"sum axis 0 of (N,3,3), N={N}", count(lambda: np.array_equal(
            np.sum(O := rng.standard_normal((N, 3, 3)), axis=0), sequential_sum(O)))))
    # Added after D7 first missed (2026-11-04): a reduction whose slices
    # hold ONE element runs along a contiguous axis, and numpy then uses its
    # pairwise sum (zero-started), not the sequential one.
    for shape in [(20, 1), (50, 1), (20, 1, 1)]:
        rows.append((f"sum axis 0 of {shape}", count(lambda: (
            np.sum(S := rng.standard_normal(shape), axis=0).ravel()[0] == pairwise(list(S.ravel()))))))
        rows.append((f"  (sequential instead)", count(lambda: np.array_equal(
            np.sum(S := rng.standard_normal(shape), axis=0), sequential_sum(S)))))
    # Outside the envelope: kept to show where the emulation stops.
    rows.append(("OUTSIDE: gemv 2x8 @ 8x1", count(lambda: np.array_equal(
        np.dot(A := rng.standard_normal((2, 8)), x := rng.standard_normal((8, 1)))[:, 0], gemvt(A, x[:, 0])))))
    rows.append(("OUTSIDE: gemv 1x16 @ 16x1", count(lambda: np.array_equal(
        np.dot(A := rng.standard_normal((1, 16)), x := rng.standard_normal((16, 1)))[:, 0], gemvt(A, x[:, 0])))))
    rows.append(("OUTSIDE: inv 6x6", count(lambda: np.array_equal(
        np.linalg.inv(S := (lambda A: A @ A.T + 0.1 * np.eye(6))(rng.standard_normal((6, 6)))), inv(S)))))
    for name, hits in rows:
        print(f"{name:28s} {hits:4d} / {T} bit-identical")


if __name__ == "__main__":
    main()
