#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 Theodore Ong and the outram-park contributors.
#
# Reference for PETIR's port of OpenMC's depletion integrators (gh:#950).
#
# Runs OpenMC's OWN `Integrator.integrate()` / `SIIntegrator.integrate()`
# (openmc/deplete/abc.py, unmodified, commit d7d3284a1) for all eight
# integrators on a three-nuclide problem with a fixed-rate operator -- no
# transport, so no Monte Carlo noise. Two shims, both outside the arithmetic:
#
#   * `StepResult.save` is replaced by an in-memory recorder (it writes HDF5
#     and needs a full CoupledOperator's bookkeeping);
#   * `pool.USE_MULTIPROCESSING = False`, so `deplete` uses `starmap` rather
#     than `Pool.starmap` -- the same function on the same arguments.
#
# Each integrator is run twice:
#
#   solver "openmc_cram48": upstream's CRAM48 (scipy SuperLU spsolve);
#   solver "dense_ipf48":   a pure-Python replica of PETIR's dense IPF CRAM
#                           (`crates/petir/src/depletion/cram.rs`), plugged in
#                           through upstream's documented `solver=` callable.
#
# With "dense_ipf48" every number the integrator logic produces should be
# BIT-IDENTICAL to PETIR (the solver is the same arithmetic, so any difference
# is in the integrator); "openmc_cram48" measures the solver DEVIATION.
#
# Usage (OpenMC venv):
#   python integrators_reference.py > integrators_reference.txt

import math
import sys

import numpy as np
from uncertainties import ufloat

import openmc
import openmc.deplete
import openmc.deplete.abc as abc
import openmc.deplete.pool as pool
from openmc.deplete import cram as ocram
from openmc.deplete.integrators import integrator_by_name
from openmc._sparse_compat import csc_array

pool.USE_MULTIPROCESSING = False

# ---- the problem (mirrored exactly in crates/petir/tests/depletion_common) --
LAM_A = 0.5     # decay constant of A [1/s], A -> C
LAM_B = 2.0     # decay constant of B [1/s], B -> C
SIG_F = 1.0     # fission "cross section" of A
SIG_G = 0.5     # capture "cross section" of A, A -> B
N0 = [1.0, 0.0, 0.0]
SOURCE = 0.3    # fission rate held constant (the "power")
TIMESTEPS = [0.1, 0.25, 0.2, 0.45]


def rates_at(n_a):
    """Constant-power flux phi = SOURCE / (SIG_F * n_a); per-atom rates."""
    phi = SOURCE / (SIG_F * n_a)
    return [SIG_F * phi, SIG_G * phi]


def matrix_from(rf, rg):
    a = np.zeros((3, 3))
    a[0, 0] = -(LAM_A + rf + rg)
    a[1, 0] = rg
    a[2, 0] = LAM_A + 2.0 * rf
    a[1, 1] = -LAM_B
    a[2, 1] = LAM_B
    return a


class Chain:
    fission_yields = [None]

    def form_matrix(self, rates, fission_yields=None):
        return csc_array(matrix_from(float(rates[0]), float(rates[1])))


class FixedRateOperator(abc.TransportOperator):
    def __init__(self):
        self.chain = Chain()
        self.prev_res = None
        self.output_dir = "."
        self.heavy_metal = 1.0
        self.calls = 0

    def __call__(self, vec, source_rate):
        self.calls += 1
        n = vec[0]
        n_a, n_b = float(n[0]), float(n[1])
        rf, rg = rates_at(n_a)
        rates = np.array([[rf, rg]])
        k = (2.4 * n_a) / (1.5 * n_a + 0.8 * n_b)
        return abc.OperatorResult(ufloat(k, 1.0e-3 * k), rates)

    def initial_condition(self):
        return [np.array(N0)]

    def write_bos_data(self, step):
        pass

    def finalize(self):
        pass

    def get_results_info(self):
        raise NotImplementedError


RECORDS = []


def recorder(op, x, op_results, t, source_rate, step_ind, proc_time=None,
             write_rates=False, keff_search_root=None, path=None):
    RECORDS.append((step_ind, [list(map(float, m)) for m in x],
                    op_results.k.nominal_value, op_results.k.std_dev,
                    [float(v) for v in np.asarray(op_results.rates).ravel()],
                    list(map(float, t)), float(source_rate)))


openmc.deplete.stepresult.StepResult.save = staticmethod(recorder)
abc.StepResult.save = staticmethod(recorder)


# ---- pure-Python replica of PETIR's dense IPF CRAM --------------------------
def solve_complex(m, b):
    n = len(b)
    for col in range(n):
        pivot = col
        best = m[col][col][0] * m[col][col][0] + m[col][col][1] * m[col][col][1]
        for row in range(col + 1, n):
            v = m[row][col]
            mag = v[0] * v[0] + v[1] * v[1]
            if mag > best:
                best = mag
                pivot = row
        if pivot != col:
            m[pivot], m[col] = m[col], m[pivot]
            b[pivot], b[col] = b[col], b[pivot]
        diag = m[col][col]
        if diag[0] == 0.0 and diag[1] == 0.0:
            raise ZeroDivisionError("singular")
        for row in range(col + 1, n):
            f = cdiv(m[row][col], diag)
            if f[0] == 0.0 and f[1] == 0.0:
                continue
            for k in range(col, n):
                m[row][k] = csub(m[row][k], cmul(f, m[col][k]))
            b[row] = csub(b[row], cmul(f, b[col]))
    x = [(0.0, 0.0)] * n
    for row in range(n - 1, -1, -1):
        acc = b[row]
        for k in range(row + 1, n):
            acc = csub(acc, cmul(m[row][k], x[k]))
        x[row] = cdiv(acc, m[row][row])
    return x


def csub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def cmul(a, b):
    return (a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0])


def cdiv(a, b):
    d = b[0] * b[0] + b[1] * b[1]
    return ((a[0] * b[0] + a[1] * b[1]) / d, (a[1] * b[0] - a[0] * b[1]) / d)


ALPHA48 = [(float(a.real), float(a.imag)) for a in ocram.Cram48Solver.alpha]
THETA48 = [(float(t.real), float(t.imag)) for t in ocram.Cram48Solver.theta]
ALPHA0_48 = float(ocram.Cram48Solver.alpha0)


def dense_ipf48(A, n0, dt, substeps=1):
    if substeps != 1:
        raise ValueError("reference uses one substep")
    dense = A.toarray()
    n = len(n0)
    if n == 0 or dt == 0.0 or not dense.any():
        return n0.copy()
    scaled = [[float(dense[i, j]) * dt for j in range(n)] for i in range(n)]
    y = [float(v) for v in n0]
    for (ar, ai), (tr, ti) in zip(ALPHA48, THETA48):
        m = [[(scaled[i][j], 0.0) for j in range(n)] for i in range(n)]
        for d in range(n):
            m[d][d] = (m[d][d][0] - tr, m[d][d][1] - ti)
        x = solve_complex(m, [(v, 0.0) for v in y])
        for r in range(n):
            y[r] += 2.0 * (ar * x[r][0] - ai * x[r][1])
    return np.array([v * ALPHA0_48 for v in y])


def main():
    out = sys.stdout
    out.write("# integrators_reference.py -- OpenMC %s, numpy %s\n"
              % (openmc.__version__, np.__version__))
    out.write("# columns: solver integrator step_ind t0 t1 source nA nB nC "
              "k k_std rate_f rate_g operator_calls\n")
    # pow(x, 2) vs x*x for the LE/QI coefficients (matrix_funcs.rs DEVIATION)
    for dt in TIMESTEPS:
        if dt ** 2 != dt * dt:
            out.write("# POW_DIFFERS %r\n" % dt)
    for solver_name, solver in (("openmc_cram48", "cram48"),
                                ("dense_ipf48", dense_ipf48)):
        for name in ("predictor", "cecm", "cf4", "celi", "epc_rk4", "leqi",
                     "si_celi", "si_leqi"):
            RECORDS.clear()
            op = FixedRateOperator()
            integ = integrator_by_name[name](
                op, TIMESTEPS, source_rates=SOURCE, timestep_units="s",
                solver=solver)
            if name.startswith("si_"):
                integ.integrate(output=False)
            else:
                integ.integrate(final_step=True, output=False)
            for (ind, x, k, ks, rates, t, src) in RECORDS:
                vals = [t[0], t[1], src] + x[0] + [k, ks] + rates
                out.write("%s %s %d %s %d\n" % (
                    solver_name, name, ind, " ".join(repr(v) for v in vals),
                    op.calls))


if __name__ == "__main__":
    main()
