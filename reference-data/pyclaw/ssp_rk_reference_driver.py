# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Code-to-code reference for petir::ssprk: runs PyClaw's OWN
`SharpClawSolver.step` (src/pyclaw/sharpclaw/solver.py, unmodified) on a small
method-of-lines system and prints every final state value as f64 bit patterns.

Usage (NumPy required; nothing of PyClaw needs to be built or installed):

    python3 -I ssp_rk_reference_driver.py <pyclaw>/src/pyclaw/sharpclaw/solver.py \
        > ssp-rk-pyclaw-reference.txt

Only the module's imports are stubbed (the Solver base class, a docstring
decorator and the WENO reconstructor, none of which the time-stepping branches
touch); the stepping code that runs is upstream's file, loaded from disk.

The right-hand side is pure arithmetic (no transcendental), so NumPy and Rust
evaluate it with identical IEEE-754 operations:

    L(t, q)_i = -c * (q_i - q_{i-1}) - 0.5 * q_i * q_i + 0.1 * t   (periodic)

PyClaw's `dq` contract is `dt * L` ("Evaluate dq/dt * (delta t)"), so the stub
`dq_hyperbolic` returns `self.dt * L(state.t, state.q)`.
"""
import importlib.util
import sys
import types
import copy

import numpy as np

N = 16
C = 4.0
NSTEPS = 40


def rhs(t, q):
    return -C * (q - np.roll(q, 1)) - 0.5 * q * q + 0.1 * t


def initial():
    return np.array([1.0 if 4 <= i < 10 else 0.25 + 0.01 * i for i in range(N)])


def load_upstream(path):
    clawpack = types.ModuleType("clawpack")
    pyclaw = types.ModuleType("clawpack.pyclaw")
    solver = types.ModuleType("clawpack.pyclaw.solver")
    util = types.ModuleType("clawpack.pyclaw.util")
    limiters = types.ModuleType("clawpack.pyclaw.limiters")

    class Solver(object):
        def __del__(self):  # upstream's __del__ chains to its base
            pass

    solver.Solver = Solver
    util.add_parent_doc = lambda parent: ""
    limiters.reconstruct = None
    limiters.recon = None
    for name, mod in [("clawpack", clawpack), ("clawpack.pyclaw", pyclaw),
                      ("clawpack.pyclaw.solver", solver), ("clawpack.pyclaw.util", util),
                      ("clawpack.pyclaw.limiters", limiters)]:
        sys.modules[name] = mod
    pyclaw.limiters = limiters
    spec = importlib.util.spec_from_file_location("sharpclaw_solver", path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


class State(object):
    def __init__(self, q, t):
        self.q = q
        self.t = t


class Cfl(object):
    def set_global_max(self, v):
        pass

    def get_cached_max(self):
        return 0.0


class Solution(object):
    def __init__(self, state):
        self.states = [state]
        self.t = state.t


def run(m, method, dt):
    s = object.__new__(m.SharpClawSolver)
    s.time_integrator = method
    s.dt = dt
    s.dt_old = dt
    s.cfl = Cfl()
    s.accept_step = True
    s.status = {"numsteps": 0}
    s.call_before_step_each_stage = False
    s.dq_src = None
    s.kernel_language = "Python"  # read only by upstream __del__
    s.get_dt = lambda t, tstart, tend, take_one_step: None
    s.dq_hyperbolic = lambda state: s.dt * rhs(state.t, state.q)
    state = State(initial(), 0.0)
    s._registers = [copy.deepcopy(state)]
    sol = Solution(state)
    for _ in range(NSTEPS):
        s.step(sol, True, state.t, state.t + dt)
        state.t = state.t + dt
        sol.t = state.t
    return state.q


def main():
    m = load_upstream(sys.argv[1])
    for method in ["SSP22", "SSP33", "SSP104"]:
        for dt in [0.0625, 0.03]:
            q = run(m, method, dt)
            for i, v in enumerate(q):
                print("%s %s %d %d %s" % (method, float(dt).hex(), NSTEPS, i,
                                          format(np.float64(v).view(np.uint64), "016x")))


if __name__ == "__main__":
    main()
