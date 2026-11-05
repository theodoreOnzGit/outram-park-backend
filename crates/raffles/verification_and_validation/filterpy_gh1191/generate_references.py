# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs for the FilterPy Kalman-filter port (gh:#1191).

Runs the UPSTREAM code itself -- FilterPy (R. Labbe, github.com/rlabbe/filterpy,
MIT), from a clone, plus the numpy / scipy it calls -- on fixed inputs, and
writes JSON into references/. The Rust port (crates/raffles/src/filtering/)
is compared against these by crates/raffles/tests/filterpy_gh1191.rs. Python
is used only here; nothing in the crate runs it.

Command (from the workspace root; FILTERPY_DIR is a clone of rlabbe/filterpy):

    FILTERPY_DIR=<clone> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/filterpy_gh1191/generate_references.py

Inputs are FIXED NUMBERS: measurement sequences come from closed-form
expressions (no random numbers), and the EnKF's random draws are replaced
by perturbation arrays that are written into the JSON and fed, in the same
order, to the Rust port (`multivariate_normal` in FilterPy's EnKF module is
monkeypatched to hand them out). The one seeded part is the separate
statistical EnKF comparison (`enkf_statistical`), which is never compared
bit for bit.

No fx / hx below calls a transcendental function except sqrt (correctly
rounded in both numpy and Rust), so any difference between the codes is in
the filters, not in a libm.
"""

import json
import os
import platform
import subprocess
import sys

FILTERPY_DIR = os.environ["FILTERPY_DIR"]
sys.path.insert(0, FILTERPY_DIR)

import numpy as np  # noqa: E402
import scipy  # noqa: E402
import filterpy  # noqa: E402
from filterpy.kalman import KalmanFilter, ExtendedKalmanFilter  # noqa: E402
from filterpy.kalman import UnscentedKalmanFilter, MerweScaledSigmaPoints  # noqa: E402
import filterpy.kalman.ensemble_kalman_filter as enkf_mod  # noqa: E402
from filterpy.kalman import EnsembleKalmanFilter  # noqa: E402
from filterpy.common import Q_discrete_white_noise, Q_continuous_white_noise  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "references")


def tl(a):
    """numpy -> nested lists of Python floats (repr round-trips exactly)."""
    return np.asarray(a, dtype=float).tolist()


def provenance():
    commit = subprocess.check_output(["git", "-C", FILTERPY_DIR, "rev-parse", "HEAD"]).decode().strip()
    core = "unknown"
    try:
        import ctypes
        import glob
        libs = glob.glob(os.path.join(os.path.dirname(np.__file__), "..", "numpy.libs", "*openblas*"))
        for lib in libs:
            f = ctypes.CDLL(lib).scipy_openblas_get_corename64_
            f.restype = ctypes.c_char_p
            core = f().decode()
    except Exception as e:  # noqa: BLE001
        core = f"unknown ({e})"
    return {
        "filterpy_commit": commit,
        "filterpy_version": filterpy.__version__,
        "numpy": np.__version__,
        "scipy": scipy.__version__,
        "python": platform.python_version(),
        "machine": platform.machine(),
        "openblas_core": core,
    }


# --------------------------------------------------------------------------
# Linear Kalman filter + RTS smoother
# --------------------------------------------------------------------------

def run_kf(name, F, H, Q, R, x0, P0, zs, B=None, u=None):
    dim_x, dim_z = F.shape[0], H.shape[0]
    kf = KalmanFilter(dim_x=dim_x, dim_z=dim_z)
    kf.F, kf.H, kf.Q, kf.R = F.copy(), H.copy(), Q.copy(), R.copy()
    kf.x, kf.P = x0.copy(), P0.copy()
    if B is not None:
        kf.B = B.copy()
    steps = []
    for z in zs:
        kf.predict(u=u)
        rec = {"x_prior": tl(kf.x), "P_prior": tl(kf.P)}
        kf.update(np.asarray(z, dtype=float).reshape(dim_z, 1))
        rec.update({"x": tl(kf.x), "P": tl(kf.P), "K": tl(kf.K), "S": tl(kf.S), "SI": tl(kf.SI), "y": tl(kf.y)})
        steps.append(rec)
    Xs = np.array([s["x"] for s in steps])
    Ps = np.array([s["P"] for s in steps])
    xs, ps, ks, pps = kf.rts_smoother(Xs, Ps)
    return {
        "name": name, "F": tl(F), "H": tl(H), "Q": tl(Q), "R": tl(R), "x0": tl(x0), "P0": tl(P0),
        "B": None if B is None else tl(B), "u": None if u is None else tl(u),
        "zs": [tl(np.asarray(z, dtype=float).reshape(dim_z)) for z in zs],
        "steps": steps,
        "rts": {"x": tl(xs), "P": tl(ps), "K": tl(ks), "Pp": tl(pps)},
    }


def kf_cases():
    out = []
    # 1. scalar random walk
    zs = [[0.05 * k + (0.37 * k % 1.0) - 0.5] for k in range(1, 61)]
    out.append(run_kf("kf_random_walk", np.array([[1.0]]), np.array([[1.0]]), np.array([[0.04]]), np.array([[0.25]]),
                      np.array([[0.0]]), np.array([[5.0]]), zs))
    # 2. constant velocity, FilterPy's own docstring model (dt 0.1, var 0.13, R 5)
    dt = 0.1
    F = np.array([[1.0, dt], [0.0, 1.0]])
    zs = [[2.0 + 0.7 * k * dt + 1.5 * (((0.9 * k) % 2.0) - 1.0)] for k in range(1, 51)]
    out.append(run_kf("kf_constant_velocity", F, np.array([[1.0, 0.0]]), Q_discrete_white_noise(2, dt=dt, var=0.13),
                      np.array([[5.0]]), np.array([[2.0], [0.0]]), np.array([[1000.0, 0.0], [0.0, 1000.0]]), zs))
    # 3. constant acceleration, two measurements with correlated noise, a control input
    dt = 0.5
    F = np.array([[1.0, dt, 0.5 * dt * dt], [0.0, 1.0, dt], [0.0, 0.0, 1.0]])
    H = np.array([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]])
    R = np.array([[0.5, 0.1], [0.1, 0.3]])
    B = np.array([[0.125], [0.5], [0.0]])
    zs = [[0.3 * (k * dt) ** 2 + (((1.3 * k) % 1.0) - 0.5), 0.6 + 0.4 * (((0.7 * k) % 1.0) - 0.5)] for k in range(1, 41)]
    out.append(run_kf("kf_constant_acceleration", F, H, Q_discrete_white_noise(3, dt=dt, var=0.02), R,
                      np.array([[0.0], [1.0], [0.0]]), np.diag([10.0, 4.0, 1.0]), zs, B=B, u=np.array([[0.2]])))
    # 4. 2-D constant velocity, state ordered by derivative (x, y, vx, vy)
    dt = 1.0
    F = np.array([[1.0, 0.0, dt, 0.0], [0.0, 1.0, 0.0, dt], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    H = np.array([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]])
    Q = Q_discrete_white_noise(2, dt=dt, var=0.05, block_size=2, order_by_dim=False)
    zs = [[1.0 * k + 0.8 * (((0.61 * k) % 1.0) - 0.5), -0.5 * k + 0.6 * (((0.83 * k) % 1.0) - 0.5)] for k in range(1, 31)]
    out.append(run_kf("kf_cv_2d", F, H, Q, np.diag([0.4, 0.6]), np.array([[0.0], [0.0], [0.0], [0.0]]),
                      np.eye(4) * 20.0, zs))
    return out


def q_cases():
    out = []
    for dim in (2, 3, 4):
        for dt in (0.1, 0.5, 1.3):
            for bs in (1, 2, 3):
                for obd in (True, False):
                    out.append({"fn": "discrete", "dim": dim, "dt": dt, "var": 0.7, "block_size": bs, "order_by_dim": obd,
                                "Q": tl(Q_discrete_white_noise(dim, dt=dt, var=0.7, block_size=bs, order_by_dim=obd))})
                    out.append({"fn": "continuous", "dim": dim, "dt": dt, "var": 0.7, "block_size": bs, "order_by_dim": obd,
                                "Q": tl(Q_continuous_white_noise(dim, dt=dt, spectral_density=0.7, block_size=bs, order_by_dim=obd))})
    return out


# --------------------------------------------------------------------------
# EKF: FilterPy book's radar (state pos, vel, alt; slant range measured)
# --------------------------------------------------------------------------

def radar_hx(x):
    return np.array([np.sqrt(x[0] ** 2 + x[2] ** 2)]).reshape(-1)


def radar_jac(x):
    horiz, alt = x[0, 0], x[2, 0]
    denom = np.sqrt(horiz ** 2 + alt ** 2)
    return np.array([[horiz / denom, 0.0, alt / denom]])


def radar_truth(k, dt):
    """Truth: 100 m/s ground speed at a constant 1000 m altitude."""
    return 100.0 * (k * dt), 1000.0


def radar_zs(n, dt):
    zs = []
    for k in range(1, n + 1):
        pos, alt = radar_truth(k, dt)
        zs.append([float(np.sqrt(pos * pos + alt * alt)) + 3.0 * (((1.3 * k) % 2.0) - 1.0)])
    return zs


def ekf_case():
    dt = 0.05
    F = np.eye(3) + np.array([[0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]]) * dt
    Q = np.zeros((3, 3))
    Q[0:2, 0:2] = Q_discrete_white_noise(2, dt=dt, var=0.1)
    Q[2, 2] = 0.1
    R = np.array([[25.0]])
    x0 = np.array([[-100.0], [200.0], [2000.0]])
    P0 = np.eye(3) * 50.0
    zs = radar_zs(40, dt)
    ekf = ExtendedKalmanFilter(dim_x=3, dim_z=1)
    ekf.F, ekf.Q, ekf.R, ekf.x, ekf.P = F.copy(), Q.copy(), R.copy(), x0.copy(), P0.copy()

    def hx_col(x):
        return np.array([[np.sqrt(x[0, 0] ** 2 + x[2, 0] ** 2)]])

    steps = []
    for z in zs:
        ekf.predict()
        rec = {"x_prior": tl(ekf.x), "P_prior": tl(ekf.P)}
        ekf.update(np.array([[z[0]]]), radar_jac, hx_col)
        rec.update({"x": tl(ekf.x), "P": tl(ekf.P), "K": tl(ekf.K), "S": tl(ekf.S), "SI": tl(ekf.SI), "y": tl(ekf.y)})
        steps.append(rec)
    return {"name": "ekf_radar", "dt": dt, "F": tl(F), "Q": tl(Q), "R": tl(R), "x0": tl(x0), "P0": tl(P0), "zs": zs, "steps": steps}


# --------------------------------------------------------------------------
# UKF with Merwe scaled sigma points
# --------------------------------------------------------------------------

def run_ukf(name, n, dim_z, fx, hx, x0, P0, Q, R, zs, alpha, beta, kappa, dt):
    pts = MerweScaledSigmaPoints(n, alpha=alpha, beta=beta, kappa=kappa)
    ukf = UnscentedKalmanFilter(dim_x=n, dim_z=dim_z, dt=dt, hx=hx, fx=fx, points=pts)
    ukf.x, ukf.P, ukf.Q, ukf.R = x0.copy(), P0.copy(), Q.copy(), R.copy()
    sig0 = pts.sigma_points(x0, P0)
    steps = []
    for z in zs:
        ukf.predict()
        rec = {"x_prior": tl(ukf.x), "P_prior": tl(ukf.P), "sigmas_f": tl(ukf.sigmas_f)}
        ukf.update(np.asarray(z, dtype=float))
        rec.update({"x": tl(ukf.x), "P": tl(ukf.P), "K": tl(ukf.K), "S": tl(ukf.S), "SI": tl(ukf.SI), "y": tl(ukf.y),
                    "sigmas_h": tl(ukf.sigmas_h)})
        steps.append(rec)
    return {"name": name, "n": n, "dim_z": dim_z, "alpha": alpha, "beta": beta, "kappa": kappa, "dt": dt,
            "Wm": tl(pts.Wm), "Wc": tl(pts.Wc), "sigmas_0": tl(sig0),
            "x0": tl(x0), "P0": tl(P0), "Q": tl(Q), "R": tl(R), "zs": [tl(np.asarray(z, dtype=float)) for z in zs], "steps": steps}


def ukf_cases():
    out = []
    # linear constant velocity (the KF case 2 model, 1-D state)
    dt = 0.1
    F = np.array([[1.0, dt], [0.0, 1.0]])
    zs = [[2.0 + 0.7 * k * dt + 1.5 * (((0.9 * k) % 2.0) - 1.0)] for k in range(1, 51)]
    out.append(run_ukf("ukf_constant_velocity", 2, 1, lambda x, dt: np.dot(F, x), lambda x: x[:1],
                       np.array([2.0, 0.0]), np.array([[1000.0, 0.0], [0.0, 1000.0]]), Q_discrete_white_noise(2, dt=dt, var=0.13),
                       np.array([[5.0]]), zs, 0.1, 2.0, 1.0, dt))
    # radar, nonlinear measurement
    dt = 0.05
    Fr = np.eye(3) + np.array([[0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]]) * dt
    Q = np.zeros((3, 3))
    Q[0:2, 0:2] = Q_discrete_white_noise(2, dt=dt, var=0.1)
    Q[2, 2] = 0.1
    out.append(run_ukf("ukf_radar", 3, 1, lambda x, dt: np.dot(Fr, x), radar_hx,
                       np.array([-100.0, 200.0, 2000.0]), np.eye(3) * 50.0, Q, np.array([[25.0]]), radar_zs(40, dt), 0.5, 2.0, 0.0, dt))
    # scalar, nonlinear process and measurement (polynomials only)
    def fx1(x, dt):
        return x + dt * (0.5 * x - 0.1 * x * x * x)

    def hx1(x):
        return np.array([0.05 * x[0] * x[0] + x[0]])

    zs = [[0.05 * v * v + v] for v in [1.6 + 0.4 * (((0.53 * k) % 1.0) - 0.5) for k in range(1, 41)]]
    out.append(run_ukf("ukf_scalar_cubic", 1, 1, fx1, hx1, np.array([0.5]), np.array([[2.0]]), np.array([[0.01]]),
                       np.array([[0.04]]), zs, 1e-3, 2.0, 2.0, 0.2))
    # two measurements (2 x 2 S): constant acceleration, linear
    dt = 0.5
    Fa = np.array([[1.0, dt, 0.5 * dt * dt], [0.0, 1.0, dt], [0.0, 0.0, 1.0]])
    zs = [[0.3 * (k * dt) ** 2 + (((1.3 * k) % 1.0) - 0.5), 0.6 + 0.4 * (((0.7 * k) % 1.0) - 0.5)] for k in range(1, 41)]
    out.append(run_ukf("ukf_constant_acceleration", 3, 2, lambda x, dt: np.dot(Fa, x), lambda x: np.array([x[0], x[2]]),
                       np.array([0.0, 1.0, 0.0]), np.diag([10.0, 4.0, 1.0]), Q_discrete_white_noise(3, dt=dt, var=0.02),
                       np.array([[0.5, 0.1], [0.1, 0.3]]), zs, 0.3, 2.0, 0.0, dt))
    return out


# --------------------------------------------------------------------------
# EnKF with injected perturbations
# --------------------------------------------------------------------------

class Injector:
    """Hands out pre-drawn arrays in call order in place of multivariate_normal."""

    def __init__(self, arrays):
        self.arrays = list(arrays)
        self.i = 0
        self.calls = []

    def __call__(self, mean, cov, size):
        a = self.arrays[self.i]
        self.i += 1
        self.calls.append([len(np.atleast_1d(mean)), int(size)])
        assert a.shape == (size, len(np.atleast_1d(mean))), (a.shape, size)
        return a.copy()


def run_enkf(name, n, dim_z, N, fx, hx, x0, P0, Q, R, zs, dt, draw_seed):
    rng = np.random.default_rng(draw_seed)
    draws = [rng.multivariate_normal(x0, P0, N)]
    for _ in zs:
        draws.append(rng.multivariate_normal(np.zeros(n), Q, N))
        draws.append(rng.multivariate_normal(np.zeros(dim_z), R, N))
    inj = Injector(draws)
    saved = enkf_mod.multivariate_normal
    enkf_mod.multivariate_normal = inj
    try:
        f = EnsembleKalmanFilter(x=x0.copy(), P=P0.copy(), dim_z=dim_z, dt=dt, N=N, hx=hx, fx=fx)
        f.Q, f.R = Q.copy(), R.copy()
        steps = []
        for z in zs:
            f.predict()
            rec = {"x_prior": tl(f.x), "P_prior": tl(f.P), "sigmas_prior": tl(f.sigmas)}
            f.update(np.asarray(z, dtype=float))
            rec.update({"x": tl(f.x), "P": tl(f.P), "K": tl(f.K), "S": tl(f.S), "SI": tl(f.SI), "sigmas": tl(f.sigmas)})
            steps.append(rec)
    finally:
        enkf_mod.multivariate_normal = saved
    assert inj.i == len(draws)
    return {"name": name, "n": n, "dim_z": dim_z, "N": N, "dt": dt, "x0": tl(x0), "P0": tl(P0), "Q": tl(Q), "R": tl(R),
            "zs": [tl(np.asarray(z, dtype=float)) for z in zs], "draws": [tl(d) for d in draws], "steps": steps}


def enkf_cases():
    out = []
    dt = 0.1
    F = np.array([[1.0, dt], [0.0, 1.0]])
    zs = [[2.0 + 0.7 * k * dt + 1.5 * (((0.9 * k) % 2.0) - 1.0)] for k in range(1, 31)]
    out.append(run_enkf("enkf_constant_velocity", 2, 1, 20, lambda x, dt: np.dot(F, x), lambda x: x[:1],
                        np.array([2.0, 0.0]), np.array([[10.0, 0.0], [0.0, 10.0]]), Q_discrete_white_noise(2, dt=dt, var=0.13),
                        np.array([[5.0]]), zs, dt, 11))
    dt = 0.5
    Fa = np.array([[1.0, dt, 0.5 * dt * dt], [0.0, 1.0, dt], [0.0, 0.0, 1.0]])
    zs = [[0.3 * (k * dt) ** 2 + (((1.3 * k) % 1.0) - 0.5), 0.6 + 0.4 * (((0.7 * k) % 1.0) - 0.5)] for k in range(1, 26)]
    out.append(run_enkf("enkf_constant_acceleration", 3, 2, 25, lambda x, dt: np.dot(Fa, x), lambda x: np.array([x[0], x[2]]),
                        np.array([0.0, 1.0, 0.0]), np.diag([10.0, 4.0, 1.0]), Q_discrete_white_noise(3, dt=dt, var=0.02),
                        np.array([[0.5, 0.1], [0.1, 0.3]]), zs, dt, 12))
    return out


def enkf_statistical():
    """FilterPy's EnKF with numpy's own draws (seeded replicates), constant
    velocity, N = 50: per replicate the final mean and covariance. Compared
    with the Rust EnKF on its own LCG draws only statistically, never bit
    for bit, and against the exact KF posterior."""
    dt = 0.1
    F = np.array([[1.0, dt], [0.0, 1.0]])
    zs = [[2.0 + 0.7 * k * dt + 1.5 * (((0.9 * k) % 2.0) - 1.0)] for k in range(1, 31)]
    x0 = np.array([2.0, 0.0])
    P0 = np.array([[10.0, 0.0], [0.0, 10.0]])
    Q = Q_discrete_white_noise(2, dt=dt, var=0.13)
    R = np.array([[5.0]])
    N, reps = 50, 400
    finals = []
    for r in range(reps):
        np.random.seed(1000 + r)
        f = EnsembleKalmanFilter(x=x0.copy(), P=P0.copy(), dim_z=1, dt=dt, N=N, hx=lambda x: x[:1], fx=lambda x, dt: np.dot(F, x))
        f.Q, f.R = Q.copy(), R.copy()
        for z in zs:
            f.predict()
            f.update(np.asarray(z, dtype=float))
        finals.append({"x": tl(f.x), "P": tl(f.P)})
    kf = KalmanFilter(dim_x=2, dim_z=1)
    kf.F, kf.H, kf.Q, kf.R, kf.x, kf.P = F, np.array([[1.0, 0.0]]), Q, R, x0.reshape(2, 1), P0.copy()
    for z in zs:
        kf.predict()
        kf.update(np.array([[z[0]]]))
    return {"N": N, "replicates": reps, "seeds": "np.random.seed(1000 + r)", "x0": tl(x0), "P0": tl(P0), "Q": tl(Q), "R": tl(R),
            "zs": zs, "dt": dt, "finals": finals, "kf_x": tl(kf.x.reshape(2)), "kf_P": tl(kf.P)}


def main():
    os.makedirs(OUT, exist_ok=True)
    data = {
        "provenance": provenance(),
        "kf": kf_cases(),
        "q": q_cases(),
        "ekf": [ekf_case()],
        "ukf": ukf_cases(),
        "enkf": enkf_cases(),
    }
    with open(os.path.join(OUT, "filterpy_filters.json"), "w") as fh:
        json.dump(data, fh, separators=(",", ":"))
    with open(os.path.join(OUT, "filterpy_enkf_statistical.json"), "w") as fh:
        json.dump({"provenance": data["provenance"], **enkf_statistical()}, fh, separators=(",", ":"))
    print(json.dumps(data["provenance"]))


if __name__ == "__main__":
    main()
