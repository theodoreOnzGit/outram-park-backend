"""Numerical rank of the pole-identification matrices under different LAPACK routines (gh:#1390).

Usage: python rank_null.py <dump_kernel_rows.py file> [...]
For the first record of each file (the pole-identification least squares, `cond = eps`): the rank DGELSD returned (the
dump), and the count of singular values above eps * s_max from DGELSS (scipy lstsq), DGESDD (numpy svd) and DGESVD
(scipy svd, lapack_driver='gesvd'), and the same count for DGELSD on the matrix with every entry changed by 1 ulp
(3 draws). Compare with the port's rank (`trace_kernel.rs::rank_differential`).
"""
import sys
import numpy as np
from scipy.linalg import lstsq, svd

eps = np.finfo(float).eps
rng = np.random.default_rng(5)
print("file rank_gelsd rank_gelss rank_gesdd rank_gesvd gelsd_1ulp(3)")
for p in sys.argv[1:]:
    v = np.fromfile(p, dtype='<f8')
    m, n = int(v[1]), int(v[2])
    q = 3
    a = v[q:q + m * n].reshape(m, n); q += m * n
    b = v[q:q + m]; q += m
    q += n
    ns = int(v[q]); q += 1
    sv = v[q:q + ns]; q += ns
    rank = int(v[q])
    r_ss = lstsq(a, b, lapack_driver='gelss')[2]
    s_dd = np.linalg.svd(a, compute_uv=False)
    s_vd = svd(a, compute_uv=False, lapack_driver='gesvd')
    r_dd = int(np.sum(s_dd > eps * s_dd.max()))
    r_vd = int(np.sum(s_vd > eps * s_vd.max()))
    pert = [int(lstsq(a * (1 + rng.choice([-1.0, 1.0], a.shape) * eps), b)[2]) for _ in range(3)]
    print(p.split("/")[-1], rank, int(r_ss), r_dd, r_vd, pert)
