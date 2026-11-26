#!/usr/bin/env python3
"""Differential test of the GeN-Foam finite-element thermo-elasticity (gh:#1782, round 25) against scikit-fem.

Reads the dump written by `fem_differential_dump` in tests/genfoam_fem_thermo_mechanics_gh1782.rs (mesh, per-element
material and temperature, constrained components, pressure facets, and the Rust solver's nodal displacement), assembles
the SAME weak form with scikit-fem's own elements (ElementHex1 with 2x2x2 Gauss, ElementTetP1), solves it with scipy's
sparse direct solver, and prints the relative difference to the Rust displacement.

Weak form: K u = f, K = int(lam div u div v + 2 mu sym grad u : sym grad v), f = int(s div v) + pressure facets,
s = 3K alpha (T - Tref) per element, lam, mu, 3K from E_eff, nu as in GeN-Foam's legacyThermoMechanics.

Usage: python3 fem_differential_scikit_fem.py <dump dir containing hex.txt, tet.txt>
Scratch environment: python3 -m venv v && v/bin/pip install numpy scipy scikit-fem   (scikit-fem 12.0.2, numpy 2.4.6).
Nothing here is imported by the workspace; it is a verification instrument only.
"""
import sys
import numpy as np
from scipy.sparse.linalg import spsolve
import skfem
from skfem import Basis, BilinearForm, LinearForm, ElementVector, MeshHex, MeshTet, ElementHex1, ElementTetP1, condense
from skfem.helpers import div, ddot, sym_grad


def read(path):
    X, E, D, P, U = [], [], [], [], []
    kind = None
    for line in open(path):
        w = line.split()
        if not w:
            continue
        if w[0] == "type":
            kind = w[1]
        elif w[0] == "X":
            X.append([float(v) for v in w[1:4]])
        elif w[0] == "E":
            E.append(w[1:])
        elif w[0] == "D":
            D.append((int(w[1]), int(w[2]), float(w[3])))
        elif w[0] == "P":
            P.append(w[1:])
        elif w[0] == "U":
            U.append([float(v) for v in w[1:4]])
    return kind, np.array(X), E, D, P, np.array(U)


def pressure_load(X, P, nd):
    """Consistent nodal forces of uniform tractions on Tri3 / Quad4 facets (independent numpy implementation)."""
    f = np.zeros(nd)
    g = 1.0 / np.sqrt(3.0)
    for rec in P:
        nodes = [int(v) for v in rec[:-3]]
        t = np.array([float(v) for v in rec[-3:]])
        x = X[nodes]
        if len(nodes) == 3:
            area = 0.5 * np.linalg.norm(np.cross(x[1] - x[0], x[2] - x[0]))
            for n in nodes:
                f[3 * n:3 * n + 3] += t * area / 3.0
        else:
            for r in (-g, g):
                for s in (-g, g):
                    N = 0.25 * np.array([(1 - r) * (1 - s), (1 + r) * (1 - s), (1 + r) * (1 + s), (1 - r) * (1 + s)])
                    dNr = 0.25 * np.array([-(1 - s), (1 - s), (1 + s), -(1 + s)])
                    dNs = 0.25 * np.array([-(1 - r), -(1 + r), (1 + r), (1 - r)])
                    j = np.linalg.norm(np.cross(dNr @ x, dNs @ x))
                    for a, n in enumerate(nodes):
                        f[3 * n:3 * n + 3] += N[a] * t * j
    return f


def solve(path):
    kind, X, E, D, P, U = read(path)
    t = np.array([[int(v) for v in e[: (8 if kind == "hex8" else 4)]] for e in E])
    par = np.array([[float(v) for v in e[(8 if kind == "hex8" else 4):]] for e in E])
    e_eff, nu, alpha, tref, temp = par[:, 0], par[:, 1], par[:, 2], par[:, 3], par[:, 4]
    lam = nu * e_eff / ((1 + nu) * (1 - 2 * nu))
    mu = e_eff / (2 * (1 + nu))
    s = e_eff / (1 - 2 * nu) * alpha * (temp - tref)
    if kind == "hex8":
        perm = [0, 3, 1, 4, 2, 7, 5, 6]  # farrer-park counter-clockwise order -> scikit-fem Hex1 vertex order (read off MeshHex.init_tensor)
        mesh = MeshHex(X.T.copy(), t[:, perm].T.copy())
        basis = Basis(mesh, ElementVector(ElementHex1()), intorder=3)  # 2x2x2 Gauss, as the Rust side
    else:
        mesh = MeshTet(X.T.copy(), t.T.copy())
        basis = Basis(mesh, ElementVector(ElementTetP1()))
    nq = basis.X.shape[1]
    ones = np.ones((1, nq))

    @BilinearForm
    def a(u, v, w):
        return w["lam"] * div(u) * div(v) + 2.0 * w["mu"] * ddot(sym_grad(u), sym_grad(v))

    @LinearForm
    def thermal(v, w):
        return w["s"] * div(v)

    K = a.assemble(basis, lam=lam[:, None] * ones, mu=mu[:, None] * ones)
    f = thermal.assemble(basis, s=s[:, None] * ones)
    nd = basis.N
    nodal = basis.nodal_dofs  # (3, n_nodes)
    # express the pressure load in this basis' dof numbering
    fp = pressure_load(X, P, 3 * len(X))
    for n in range(len(X)):
        for k in range(3):
            f[nodal[k, n]] += fp[3 * n + k]
    x = np.zeros(nd)
    dofs = []
    for n, k, v in D:
        x[nodal[k, n]] = v
        dofs.append(nodal[k, n])
    dofs = np.array(sorted(set(dofs)), dtype=int)
    Kc, fc, xc, I = condense(K, f, x=x, D=dofs)
    xc[I] = spsolve(Kc.tocsc(), fc)
    out = np.zeros((len(X), 3))
    for n in range(len(X)):
        for k in range(3):
            out[n, k] = xc[nodal[k, n]]
    return out, U, kind


def main(d):
    for tag in ("hex", "tet"):
        sk, rust, kind = solve(f"{d}/{tag}.txt")
        diff = np.abs(sk - rust).max()
        scale = np.abs(rust).max()
        print(f"{tag} ({kind}, {len(rust)} nodes): max|u_skfem - u_rust| = {diff:.3e} m, relative to max|u| = {scale:.4e} m: {diff / scale:.3e}")


if __name__ == "__main__":
    main(sys.argv[1])
