"""Nulls for the kernel differential (gh:#1390): how far does LAPACK move itself on the dumped matrices?

Usage: python kernel_null.py <dump file> [<dump file> ...]
For each dump (dump_kernel_inputs.py): the pole-identification least squares solved with
  DGELSD (the dump), DGELSS (another valid driver, same `cond = eps` rule), DGELSY (QR with pivoting),
  DGELSD on the matrix with every entry multiplied by 1 +- 2^-52 (5 draws);
and the eigenvalues of the dumped matrix by DGEEV (the dump) against DGEEV on the matrix with every entry
multiplied by 1 +- 2^-52 (5 draws). Prints max |x - x_dgelsd| / max|x_dgelsd| and the largest relative
eigenvalue difference (nearest match), plus s_17/s_max (the smallest kept singular value) and the gap to s_18.
"""
import sys
import numpy as np
from scipy.linalg import lstsq, eigvals


def load(p):
    v = np.fromfile(p, dtype='<f8')
    m, n = int(v[0]), int(v[1])
    q = 2
    a = v[q:q + m * n].reshape(m, n); q += m * n
    b = v[q:q + m]; q += m
    x = v[q:q + n]; q += n
    ns = int(v[q]); q += 1
    sv = v[q:q + ns]; q += ns
    rank = int(v[q]); q += 1
    N = int(v[q]); q += 1
    H = v[q:q + N * N].reshape(N, N); q += N * N
    ev = v[q:q + 2 * N].reshape(N, 2)
    return a, b, x, sv, rank, H, ev[:, 0] + 1j * ev[:, 1]


def nearest(a, b):
    return float(max(np.min(np.abs(b - z)) / abs(z) for z in a))


rng = np.random.default_rng(11)
for p in sys.argv[1:]:
    a, b, x, sv, rank, H, ev = load(p)
    xm = np.abs(x).max()
    out = {}
    out["gelss"] = np.abs(lstsq(a, b, lapack_driver='gelss')[0] - x).max() / xm
    out["gelsy"] = np.abs(lstsq(a, b, lapack_driver='gelsy')[0] - x).max() / xm
    d = []
    for _ in range(5):
        ap = a * (1 + rng.choice([-1.0, 1.0], a.shape) * 2.0**-52)
        d.append(np.abs(lstsq(ap, b)[0] - x).max() / xm)
    out["gelsd 1-ulp (5 draws, min..max)"] = (min(d), max(d))
    e = []
    for _ in range(5):
        Hp = H * (1 + rng.choice([-1.0, 1.0], H.shape) * 2.0**-52)
        e.append(nearest(ev, eigvals(Hp)))
    print(p.split("/")[-1], f"s_{rank}/s_max = {sv[rank - 1] / sv[0]:.2e}, s_{rank + 1}/s_max = {sv[rank] / sv[0]:.2e}")
    print("   x vs DGELSD: DGELSS %.2e, DGELSY %.2e, DGELSD with 1-ulp matrix: %.2e .. %.2e" % (out["gelss"], out["gelsy"], *out["gelsd 1-ulp (5 draws, min..max)"]))
    print("   eigenvalues, DGEEV vs DGEEV on a 1-ulp perturbed matrix: %.2e .. %.2e" % (min(e), max(e)))
