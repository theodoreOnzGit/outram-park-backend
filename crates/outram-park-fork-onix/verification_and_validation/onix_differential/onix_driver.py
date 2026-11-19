#!/usr/bin/env python3
"""ONIX 7328dc6 reference driver for the outram-park-fork-onix differential (gh:#1404).

Run it with the isolated interpreter, from OUTSIDE the clone:

    python -I onix_driver.py --onix /path/to/ONIX --out /path/to/outdir [--real]

It never writes into the clone and never edits an upstream file. It imports the
upstream modules unchanged, with three driver-side accommodations, all recorded in
RESULTS.md:

1. `openmc` (and submodules) are replaced by `unittest.mock.MagicMock`: `import onix`
   imports `onix.cell`, `onix.couple` and `onix.data.read_lib_functions`, which
   `import openmc` at module level; OpenMC is only *called* in the coupled path.
2. `onix.salameche.cram.CRAM16` as committed builds `np.complex256` arrays and calls
   `np.linalg.solve` on them, which numpy rejects (`TypeError: array type complex256 is
   unsupported in linalg`). Two variants are therefore run on every case:
   * "ld": the upstream function untouched, with `np.linalg.solve` wrapped so that its
     arguments are cast to `complex128` (everything else, including the long-double
     `At - theta*I`, `alpha*N_0` and the final `2*_N.real + alpha_0*N_0`, stays upstream);
   * "d": the upstream source text of `cram.py` with the token `complex256` replaced by
     `complex128` before `exec` (all arithmetic in double, no shim on `solve`).
3. Nothing else is patched.

Outputs (all text; floats are Python `repr`, which round-trips a double exactly):
  cram_cases.csv     kind,case,dt,i,j,v,v2
  case2_inputs.csv   kind,zamid,key,value   (synthetic passport set, see SYNTHETIC)
  case2_matrix.csv   kind,row,col,value     (kind A = B*1e-24*flux + C, B, C)
  case3_burn.csv     step,zamid,value
"""
import argparse
import math as m
import os
import sys
import types
from unittest.mock import MagicMock


def parse():
    ap = argparse.ArgumentParser()
    ap.add_argument("--onix", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--real", action="store_true",
                    help="also dump the ONIX bundled-library case (not committed: data stays in njoy-outram-park-fork)")
    return ap.parse_args()


ARGS = parse()
for _n in ("openmc", "openmc.mgxs", "openmc.data", "openmc.deplete"):
    sys.modules[_n] = MagicMock()
sys.path.insert(0, ARGS.onix)

import numpy as np  # noqa: E402

import onix  # noqa: E402,F401
from onix.salameche import cram as up_cram  # noqa: E402
from onix.salameche import mat_builder as mb  # noqa: E402
from onix.passlist import Passlist  # noqa: E402
from onix import data as d  # noqa: E402

# ---------------------------------------------------------------- CRAM variants
_orig_solve = np.linalg.solve


def _solve_c128(a, b):
    return _orig_solve(np.asarray(a, dtype=np.complex128), np.asarray(b, dtype=np.complex128))


def cram_ld(At, N0):
    """Upstream CRAM16, untouched, with the solve argument cast shim."""
    np.linalg.solve = _solve_c128
    try:
        return up_cram.CRAM16(At, N0)
    finally:
        np.linalg.solve = _orig_solve


def _load_double_variant():
    path = os.path.join(ARGS.onix, "onix", "salameche", "cram.py")
    with open(path) as f:
        src = f.read()
    assert "complex256" in src
    ns = {}
    exec(compile(src.replace("complex256", "complex128"), "cram_c128", "exec"), ns)
    return ns["CRAM16"]


_cram_d = _load_double_variant()


def cram_d(At, N0):
    return _cram_d(At, N0)


def _solve_replica(a, b):
    """Pure-Python replica of the elimination in `petir::depletion::onix_cram::solve_pfd`
    (partial pivoting on hypot, textbook complex division, (ac-bd)+(ad+bc)i products).
    Driver-side only: it localises the first divergence between numpy's LAPACK `zgesv`
    and the Rust elimination. It is NOT upstream code."""
    n = len(b)
    mre = [[float(a[i][j].real) for j in range(n)] for i in range(n)]
    mim = [[float(a[i][j].imag) for j in range(n)] for i in range(n)]
    bre = [float(x.real) for x in b]
    bim = [float(x.imag) for x in b]

    def div(xr, xi, yr, yi):
        den = yr * yr + yi * yi
        return (xr * yr + xi * yi) / den, (xi * yr - xr * yi) / den

    def mul(xr, xi, yr, yi):
        return xr * yr - xi * yi, xr * yi + xi * yr

    for col in range(n):
        piv, best = col, m.hypot(mre[col][col], mim[col][col])
        for row in range(col + 1, n):
            mag = m.hypot(mre[row][col], mim[row][col])
            if mag > best:
                best, piv = mag, row
        mre[col], mre[piv] = mre[piv], mre[col]
        mim[col], mim[piv] = mim[piv], mim[col]
        bre[col], bre[piv] = bre[piv], bre[col]
        bim[col], bim[piv] = bim[piv], bim[col]
        dr, di = mre[col][col], mim[col][col]
        for row in range(col + 1, n):
            fr, fi = div(mre[row][col], mim[row][col], dr, di)
            if fr == 0.0 and fi == 0.0:
                continue
            for k in range(col, n):
                pr, pi = mul(fr, fi, mre[col][k], mim[col][k])
                mre[row][k] -= pr
                mim[row][k] -= pi
            pr, pi = mul(fr, fi, bre[col], bim[col])
            bre[row] -= pr
            bim[row] -= pi
    xr, xi = [0.0] * n, [0.0] * n
    for i in range(n - 1, -1, -1):
        ar, ai = bre[i], bim[i]
        for j in range(i + 1, n):
            pr, pi = mul(mre[i][j], mim[i][j], xr[j], xi[j])
            ar -= pr
            ai -= pi
        xr[i], xi[i] = div(ar, ai, mre[i][i], mim[i][i])
    return np.array([complex(r, i) for r, i in zip(xr, xi)], dtype=np.complex128)


def cram_r(At, N0):
    """The all-double upstream source (complex128) with the replica elimination as `solve`."""
    np.linalg.solve = _solve_replica
    try:
        return _cram_d(At, N0)
    finally:
        np.linalg.solve = _orig_solve


# --------------------------------------------------------------------- case 1
SEC = {"1s": 1.0, "1d": 86400.0, "30d": 30 * 86400.0}


def chain3():
    A = np.zeros((3, 3))
    A[0, 0] = -1e-2
    A[1, 0] = 1e-2
    A[1, 1] = -1e-3
    A[2, 1] = 1e-3
    return A, np.array([1.0, 0.0, 0.0])


def fp20():
    n = 20
    A = np.zeros((n, n))
    lam = [10.0 ** (-6.0 + 4.0 * k / (n - 1)) for k in range(n)]
    for k in range(n):
        cap = 1e-9 * (k + 1)  # capture loop to the next nuclide
        A[k, k] = -(lam[k] + cap)
        if k + 1 < n:
            A[k + 1, k] += 0.7 * lam[k] + cap
        if k + 2 < n:
            A[k + 2, k] += 0.3 * lam[k]
    A[n - 1, n - 1] = -lam[n - 1]
    n0 = np.zeros(n)
    n0[0] = 1.0
    n0[3] = 0.5
    return A, n0


def stiff():
    n = 8
    lam = [1e4, 1e2, 1.0, 1e-2, 1e-4, 1e-6, 1e-9, 1e-12]
    A = np.zeros((n, n))
    for k in range(n):
        A[k, k] = -lam[k]
        if k + 1 < n:
            A[k + 1, k] = lam[k]
    n0 = np.zeros(n)
    n0[0] = 1.0
    return A, n0


def scalar():
    return np.array([[-1e-3]]), np.array([2.5])


def write_case1(out):
    rows = ["kind,case,dt,i,j,v,v2"]
    for name, make in (("chain3", chain3), ("fp20", fp20), ("stiff", stiff), ("scalar", scalar)):
        A, n0 = make()
        n = len(n0)
        for i in range(n):
            for j in range(n):
                if A[i, j] != 0.0:
                    rows.append(f"A,{name},,{i},{j},{float(A[i, j])!r},")
        for i in range(n):
            rows.append(f"N0,{name},,{i},,{float(n0[i])!r},")
        for tag, dt in SEC.items():
            At = A * dt
            ld = cram_ld(At, n0)
            dd = cram_d(At, n0)
            rr = cram_r(At, n0)
            for i in range(n):
                rows.append(f"OUT_LD,{name},{dt!r},{i},,{float(ld[i])!r},{np.format_float_scientific(ld[i], precision=20, unique=False)}")
                rows.append(f"OUT_D,{name},{dt!r},{i},,{float(dd[i])!r},")
                rows.append(f"OUT_R,{name},{dt!r},{i},,{float(rr[i])!r},")
    with open(os.path.join(ARGS.out, "cram_cases.csv"), "w") as f:
        f.write("\n".join(rows) + "\n")


# --------------------------------------------------------------------- case 2/3
# SYNTHETIC passport set. The numbers are invented for this test (chosen to be of
# plausible magnitude); they are NOT ONIX library data and not evaluated nuclear data.
HL = {  # half-lives, seconds
    "922350": 2.2e16, "922360": 7.4e14, "922380": 1.4e17, "922390": 1407.0,
    "932390": 2.03e5, "942390": 7.6e11, "942400": 2.07e11, "942410": 4.5e8,
    "952410": 1.4e10, "952421": 4.4e9, "952420": 5.77e4, "952430": 2.3e11,
    "962420": 1.4e7, "942380": 2.8e9, "942420": 1.2e13, "932370": 6.7e13,
    "932361": 3.9e11, "932360": 4.9e12, "471100": 24.6, "471101": 2.16e7,
    "481101": 100.0, "471110": 6.4e5, "541350": 3.3e4, "531350": 2.4e4,
    "551350": 7e13, "611490": 1.9e5, "551370": 9.5e8,
}
DECAY_B = {  # fractions of the total decay constant, by upstream decay-lib key
    "922350": {"alpha": 1.0}, "922360": {"alpha": 1.0}, "922380": {"alpha": 1.0},
    "922390": {"betaneg": 1.0}, "932390": {"betaneg": 1.0},
    "942390": {"alpha": 1.0}, "942400": {"alpha": 1.0},
    "942410": {"betaneg": 0.99997, "alpha": 0.00003},
    "952410": {"alpha": 1.0},
    "952421": {"gamma": 0.995, "alpha": 0.005},
    "952420": {"betaneg": 0.827, "betapos": 0.173},
    "952430": {"alpha": 1.0}, "962420": {"alpha": 1.0}, "942380": {"alpha": 1.0},
    "942420": {"alpha": 1.0}, "932370": {"alpha": 1.0},
    "932361": {"betapos": 0.5, "betaposX": 0.0, "betaneg": 0.5},
    "932360": {"betapos": 1.0},
    "471100": {"betaneg": 1.0},
    "471101": {"betaneg": 0.98664, "betanegX": 0.0066, "gamma": 0.00676},
    "481101": {"gamma": 1.0}, "471110": {"betaneg": 1.0},
    "541350": {"betaneg": 1.0}, "531350": {"betaneg": 1.0},
    "551350": {"betaneg": 1.0}, "611490": {"betaneg": 1.0},
    "551370": {"betaneg": 1.0},
}
XS = {  # barns, in upstream xs-lib key order
    "922350": {"fission": 50.0, "(n,gamma)": 10.0, "(n,2n)": 0.003},
    "922360": {"(n,gamma)": 5.1},
    "922380": {"fission": 0.1, "(n,gamma)": 0.9, "(n,2n)": 0.004},
    "922390": {"(n,gamma)": 20.0}, "932390": {"(n,gamma)": 30.0},
    "942390": {"fission": 90.0, "(n,gamma)": 40.0, "(n,2n)": 0.01},
    "942400": {"(n,gamma)": 120.0},
    "942410": {"fission": 100.0, "(n,gamma)": 35.0},
    "952410": {"fission": 0.4, "(n,gamma)": 50.0, "(n,gamma)X": 5.0, "(n,2n)X": 0.001},
    "952421": {"fission": 500.0, "(n,gamma)": 100.0},
    "952420": {"fission": 2000.0, "(n,gamma)": 20.0},
    "952430": {"(n,gamma)": 70.0}, "962420": {"(n,gamma)": 5.0},
    "942380": {"(n,gamma)": 30.0}, "942420": {"(n,gamma)": 20.0},
    "932370": {"(n,gamma)": 80.0, "(n,2n)": 0.01, "(n,2n)X": 0.002},
    "932361": {"(n,gamma)": 10.0}, "932360": {"(n,gamma)": 3.0},
    "471090": {"(n,gamma)": 4.0, "(n,gamma)X": 87.0},
    "471100": {"(n,gamma)": 80.0}, "471101": {"(n,gamma)": 90.0},
    "481100": {"(n,gamma)": 11.0}, "481110": {"(n,gamma)": 6.0},
    "541350": {"(n,gamma)": 2.6e6}, "531350": {"(n,gamma)": 7.0},
    "621490": {"(n,gamma)": 4.0e4},
    "50100": {"(n,a)": 3840.0, "(n,gamma)": 0.5},
}
FY = {  # fission-product yields, percent, {parent: [pct, unc]} (synthetic)
    "541350": {"922350": [0.0017, 0.0], "942390": [0.0031, 0.0], "942410": [0.0019, 0.0], "922380": [0.0013, 0.0]},
    "531350": {"922350": [6.3, 0.0], "942390": [5.9, 0.0], "942410": [5.5, 0.0], "922380": [6.1, 0.0]},
    "551350": {"922350": [6.5, 0.0], "942390": [7.6, 0.0]},
    "611490": {"922350": [1.1, 0.0], "942390": [1.2, 0.0], "942410": [0.9, 0.0]},
    "551370": {"922350": [6.2, 0.0], "942390": [6.6, 0.0], "942410": [6.7, 0.0], "922380": [6.0, 0.0],
               "952421": [5.5, 0.0]},
    "471110": {"922350": [0.02, 0.0], "942390": [0.5, 0.0]},
}
ALL = sorted(set(HL) | set(XS) | set(FY) | {"471090", "481100", "481110", "621490", "561370", "541360",
                                            "50100", "30070", "50110"}, key=int)
INIT = {"922380": 2.0e22, "922350": 7.0e20, "942390": 1.0e19, "50100": 1e18, "471090": 1e17,
        "952410": 1e17, "932370": 1e17, "471100": 0.0}


def build_synthetic():
    pl = Passlist(ALL)
    for p in pl.passport_list:
        z = p.zamid
        if z in HL:
            tot = m.log(2) / HL[z]
            b = {"half-life": HL[z], "total decay": tot}
            b.update(DECAY_B[z])
            a = {"half-life": HL[z], "total decay": tot}
            for k, v in DECAY_B[z].items():
                a[k] = v * tot
            p.set_decay(a, b)
        else:
            p.set_decay("stable", "stable")
        if z in XS:
            xs = {k: [v, 0.0] for k, v in XS[z].items()}
            xs["removal"] = [0, 0]
            rem = 0
            for k in XS[z]:
                rem += XS[z][k]
            xs["removal"][0] = rem
            p._set_xs(xs)
        if z in FY:
            p.fy = {k: list(v) for k, v in FY[z].items()}
    pl._set_initial_dens({k: v for k, v in INIT.items()})
    return pl


class _Cell:
    def __init__(self, pl):
        self.passlist = pl


def dump_system(pl, flux, tag, out, inputs_file):
    plist = pl.passport_list
    idx = pl.get_index_dict()
    # --- inputs
    rows = ["kind,zamid,key,value", f"F,,flux,{flux!r}"]
    for p in plist:
        z = p.zamid
        rows.append(f"N,{z},,")
        da, db = p.decay_a, p.decay_b
        if db is not None and db != "stable":
            rows.append(f"D,{z},total,{da['total decay']!r}")
            for k, v in db.items():
                if k not in ("half-life", "total decay"):
                    rows.append(f"D,{z},{k},{v!r}")
        xs = p.current_xs
        if xs is not None:
            for k, v in xs.items():
                if k != "removal":
                    rows.append(f"X,{z},{k},{v[0]!r}")
        fy = p.fy
        if fy is not None:
            for parent, v in fy.items():
                rows.append(f"Y,{z},{parent},{v[0]!r}")
    with open(os.path.join(out, inputs_file), "w") as f:
        f.write("\n".join(rows) + "\n")
    # --- matrices (upstream mat_builder, unchanged)
    B = mb.get_xs_mat(pl)
    C = mb.get_decay_mat(pl)
    A = B * 1e-24 * flux + C
    mrows = ["kind,row,col,value"]
    for kind, M in (("A", A), ("B", B), ("C", C)):
        nz = np.argwhere(M != 0.0)
        for i, j in nz:
            mrows.append(f"{kind},{plist[i].zamid},{plist[j].zamid},{float(M[i, j])!r}")
    with open(os.path.join(out, tag + "_matrix.csv"), "w") as f:
        f.write("\n".join(mrows) + "\n")
    return A


def burn(pl, A, dt, steps, out, fname):
    plist = pl.passport_list
    N = np.array([p.current_dens for p in plist], dtype=float)
    cell = _Cell(pl)
    rows = ["step,zamid,value"]
    for i, p in enumerate(plist):
        rows.append(f"0,{p.zamid},{float(N[i])!r}")
    At = A * dt
    for s in range(1, steps + 1):
        N = cram_ld(At, N)
        # burn_microstep: CRAM_density_check(bucell, N) (burn.py:229)
        up_cram.CRAM_density_check(cell, N)
        for i, p in enumerate(plist):
            rows.append(f"{s},{p.zamid},{float(N[i])!r}")
    with open(os.path.join(out, fname), "w") as f:
        f.write("\n".join(rows) + "\n")


def main():
    os.makedirs(ARGS.out, exist_ok=True)
    write_case1(ARGS.out)
    pl = build_synthetic()
    flux = 3.0e14
    A = dump_system(pl, flux, "case2", ARGS.out, "case2_inputs.csv")
    burn(pl, A, 30 * 86400.0, 10, ARGS.out, "case3_burn.csv")
    if ARGS.real:
        nucl = list(d.list_and_dict.reduced_nucl_set)
        rp = Passlist(nucl)
        rp._set_decay(d.default_decay_lib_b, d.default_decay_lib_a)
        rp._set_xs(d.default_xs_lib)
        rp._set_fy(d.default_fy_lib)
        dump_system(rp, flux, "real", ARGS.out, "real_inputs.csv")
    print("done", len(pl.passport_list), "synthetic nuclides")


main()
