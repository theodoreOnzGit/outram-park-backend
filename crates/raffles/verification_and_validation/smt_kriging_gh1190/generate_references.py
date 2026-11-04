# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs for the SMT kriging port (gh:#1190): KRG, KPLS, KPLSK, MFK.

Runs the UPSTREAM code itself (SMT, the Surrogate Modeling Toolbox, from a
clone, plus the numpy / scipy / scikit-learn it calls) on fixed inputs and
writes the results as JSON next to this script, in references/. The Rust port
in crates/raffles is compared against these files by
tests/smt_kriging_gh1190.rs. Python is used only here, to generate
references; nothing in the crate runs it.

Command (from the workspace root; SMT_DIR is a clone of SMTorg/smt, imported
from source: KRG/KPLS/KPLSK/MFK are pure Python and need no compiled part):

    SMT_DIR=<clone> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/smt_kriging_gh1190/generate_references.py

Every model below is SMT's own class, constructed with SMT's defaults except
where a case says otherwise. Two kinds of case:

  * FIXED theta: hyper_opt="NoOp", so SMT evaluates its likelihood, GLS
    weights, predictor and variance at theta0 without any optimiser. This is
    the optimiser-path-free comparison of the numerics. SMT's own
    `optimal_theta` (= 10**log10(theta0), which can differ from theta0 by an
    ulp) is recorded and is what the port is given.
  * FIT: SMT's default optimiser (TNC, multistart: theta0, one uniform draw
    and an n_start = 10 maximin LHS from SMT's seeded generator, seed 41,
    maxfun = max(12 * n_theta, 50) per start). The starting points SMT used
    are captured (by wrapping `HyperparamOptimizer.optimize`, which changes
    nothing it does) and recorded, and so is a "polished" optimum: scipy's TNC
    restarted from SMT's answer on SMT's own objective with maxfun = 10000,
    which shows how far SMT's 50-evaluation budget is from the optimum.
"""

import json
import os
import platform
import subprocess
import sys

SMT_DIR = os.environ["SMT_DIR"]
sys.path.insert(0, SMT_DIR)

import numpy as np  # noqa: E402
import scipy  # noqa: E402
from scipy import optimize  # noqa: E402
import sklearn  # noqa: E402

from smt.applications import MFK  # noqa: E402
from smt.problems import Branin, Rosenbrock  # noqa: E402
from smt.sampling_methods import LHS  # noqa: E402
from smt.surrogate_models import KPLS, KPLSK, KRG  # noqa: E402
from smt.surrogate_models.krg_based import hyperparam_optim as HO  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "references")

# --- capture the multistart points SMT hands its optimiser -----------------
STARTS = []
_orig_optimize = HO.HyperparamOptimizer.optimize


def _recording_optimize(self, objective, theta_starts, *a, **k):
    STARTS.append(np.array(theta_starts).tolist())
    return _orig_optimize(self, objective, theta_starts, *a, **k)


HO.HyperparamOptimizer.optimize = _recording_optimize


def f(x):
    """Floats as Python reprs: shortest round-trip decimal, exact in f64."""
    if isinstance(x, (list, tuple, np.ndarray)):
        return [f(v) for v in x]
    return float(x)


def design(xlimits, n, seed):
    return LHS(xlimits=np.array(xlimits), criterion="ese", seed=seed)(n)


def krg_record(m, xt, yt, xtest):
    """Everything the port is compared on, for a trained KRG/KPLS/KPLSK."""
    rec = {
        "xt": f(xt),
        "yt": f(yt.ravel()),
        "x_offset": f(m.X_offset),
        "x_scale": f(m.X_scale),
        "y_mean": f(np.atleast_1d(m.y_mean)[0]),
        "y_std": f(np.atleast_1d(m.y_std)[0]),
        "optimal_theta": f(np.atleast_1d(m.optimal_theta)),
        "rlf": f(m.optimal_rlf_value),
        "sigma2": f(np.atleast_1d(m.optimal_par["sigma2"])[0]),
        "beta": f(m.optimal_par["beta"].ravel()),
        "gamma": f(m.optimal_par["gamma"].ravel()),
        "x_test": f(xtest),
        "mean": f(m.predict_values(xtest).ravel()),
        "variance": f(m.predict_variances(xtest).ravel()),
    }
    rec["cond_r"] = cond_r(m.optimal_par["C"])
    if hasattr(m, "coeff_pls"):
        rec["coeff_pls"] = f(np.atleast_2d(m.coeff_pls))
    return rec


def cond_r(c):
    """cond(R) from SMT's own Cholesky factor (2-norm, numpy)."""
    return float(np.linalg.cond(c @ c.T))


def unblocked_sigma2(xn, yn, theta, nugget, dof, y_std):
    """sigma^2 from a pure-Python unblocked Cholesky in the port's loop order
    (crate::bayesian::transitional::cholesky), squar_exp, constant trend: the
    addendum's demonstration that an ill-conditioned level's gap is the
    factorisation order."""
    import math

    n = len(yn)
    r = [[(1.0 + nugget) if i == j else math.exp(-sum(t * abs(a - b) * abs(a - b) for t, a, b in zip(theta, xn[i], xn[j]))) for j in range(n)] for i in range(n)]
    lo = [[0.0] * n for _ in range(n)]
    for i in range(n):
        for j in range(i + 1):
            s = r[i][j]
            for k in range(j):
                s -= lo[i][k] * lo[j][k]
            lo[i][j] = math.sqrt(s) if i == j else s / lo[j][j]

    def fw(b):
        z = [0.0] * n
        for i in range(n):
            s = b[i]
            for k in range(i):
                s -= lo[i][k] * z[k]
            z[i] = s / lo[i][i]
        return z

    ft, yt = fw([1.0] * n), fw(list(yn))
    beta = sum(a * b for a, b in zip(ft, yt)) / sum(a * a for a in ft)
    rho = [a - b * beta for a, b in zip(yt, ft)]
    return sum(v * v for v in rho) / dof * y_std * y_std


def rlf_gradient(m, theta):
    g, _ = m._reduced_likelihood_gradient(np.atleast_2d(theta).T)
    return f(np.ravel(g))


def polish(m, theta, bounds):
    """SMT's own objective and TNC, restarted from `theta` with maxfun 10000."""

    def obj(lt):
        return -m._reduced_likelihood_function(theta=10.0**lt)[0]

    def grad(lt):
        lt2 = np.atleast_2d(lt).T
        return np.ravel(
            -np.log(10.0) * (10.0**lt2) * m._reduced_likelihood_gradient(10.0**lt2)[0]
        )

    lb = np.log10(bounds)
    res = optimize.minimize(
        obj,
        np.log10(theta),
        jac=grad,
        method="TNC",
        bounds=[tuple(lb)] * len(theta),
        options={"maxfun": 10000},
    )
    th = 10.0**res.x
    return {"theta": f(th), "rlf": f(m._reduced_likelihood_function(theta=th)[0])}


def train(cls, xt, yt, **opts):
    m = cls(print_global=False, **opts)
    m.set_training_values(xt, yt)
    m.train()
    return m


# ---------------------------------------------------------------------------
# KRG
# ---------------------------------------------------------------------------


def krg_cases():
    out = []
    br = Branin(ndim=2)
    xb = design(br.xlimits, 20, 1)
    yb = br(xb)
    rng = np.random.default_rng(7)
    lo, hi = br.xlimits[:, 0], br.xlimits[:, 1]
    xb_test = lo + (hi - lo) * rng.random((100, 2))
    yb_test = br(xb_test)
    ro = Rosenbrock(ndim=2)
    xr = design(ro.xlimits, 20, 2)
    yr = ro(xr)
    xr_test = -2.0 + 4.0 * np.random.default_rng(8).random((100, 2))
    yr_test = ro(xr_test)
    x1 = np.array([[0.0], [1.0], [2.0], [3.0], [4.0]])
    y1 = np.array([[0.0], [1.0], [1.5], [0.9], [1.0]])
    x1_test = np.linspace(-0.5, 4.5, 23).reshape(-1, 1)

    fixed = [
        ("doc_1d_squar_exp", x1, y1, x1_test, "squar_exp", "constant", [0.3]),
        ("branin_squar_exp", xb, yb, xb_test[:25], "squar_exp", "constant", [0.4, 1.7]),
        ("branin_abs_exp", xb, yb, xb_test[:25], "abs_exp", "constant", [0.4, 1.7]),
        ("branin_matern52", xb, yb, xb_test[:25], "matern52", "constant", [0.4, 1.7]),
        ("branin_matern32", xb, yb, xb_test[:25], "matern32", "constant", [0.4, 1.7]),
        ("branin_linear", xb, yb, xb_test[:25], "squar_exp", "linear", [0.4, 1.7]),
        ("branin_quadratic", xb, yb, xb_test[:25], "squar_exp", "quadratic", [0.4, 1.7]),
        ("rosenbrock_matern32_linear", xr, yr, xr_test[:25], "matern32", "linear", [0.8, 0.3]),
    ]
    for name, xt, yt, xtest, corr, poly, theta0 in fixed:
        m = train(KRG, xt, yt, corr=corr, poly=poly, theta0=theta0, hyper_opt="NoOp")
        rec = krg_record(m, xt, yt, xtest)
        rec.update(name=name, kind="fixed", corr=corr, poly=poly, theta0=f(theta0))
        rec["rlf_gradient"] = rlf_gradient(m, m.optimal_theta)
        out.append(rec)

    fits = [
        ("fit_doc_1d", x1, y1, x1_test, None, "squar_exp"),
        ("fit_branin_squar_exp", xb, yb, xb_test, yb_test, "squar_exp"),
        ("fit_branin_matern52", xb, yb, xb_test, yb_test, "matern52"),
        ("fit_rosenbrock_squar_exp", xr, yr, xr_test, yr_test, "squar_exp"),
    ]
    for name, xt, yt, xtest, ytest, corr in fits:
        STARTS.clear()
        m = train(KRG, xt, yt, corr=corr)
        rec = krg_record(m, xt, yt, xtest)
        rec.update(name=name, kind="fit", corr=corr, poly="constant", theta0=f([1e-2]))
        rec["starts_log10"] = STARTS[0]
        rec["polished"] = polish(m, m.optimal_theta, m.options["theta_bounds"])
        if ytest is not None:
            rec["y_test"] = f(ytest.ravel())
        out.append(rec)
    return out


# ---------------------------------------------------------------------------
# KPLS / KPLSK
# ---------------------------------------------------------------------------


def kpls_cases():
    out = []
    ro = Rosenbrock(ndim=6)
    xt = design(ro.xlimits, 40, 3)
    yt = ro(xt)
    xtest = -2.0 + 4.0 * np.random.default_rng(9).random((100, 6))
    ytest = ro(xtest)
    for name, corr, theta0 in (
        ("kpls_squar_exp_fixed", "squar_exp", [0.05, 0.2]),
        ("kpls_abs_exp_fixed", "abs_exp", [0.05, 0.2]),
    ):
        m = train(KPLS, xt, yt, n_comp=2, corr=corr, theta0=theta0, hyper_opt="NoOp")
        rec = krg_record(m, xt, yt, xtest[:25])
        rec.update(name=name, kind="fixed", model="KPLS", corr=corr, poly="constant", n_comp=2, theta0=f(theta0))
        rec["rlf_gradient"] = rlf_gradient(m, m.optimal_theta)
        out.append(rec)
    STARTS.clear()
    m = train(KPLS, xt, yt, n_comp=2)
    rec = krg_record(m, xt, yt, xtest)
    rec.update(name="kpls_fit", kind="fit", model="KPLS", corr="squar_exp", poly="constant", n_comp=2, theta0=f([1e-2]))
    rec["starts_log10"] = STARTS[0]
    rec["polished"] = polish(m, m.optimal_theta, m.options["theta_bounds"])
    rec["y_test"] = f(ytest.ravel())
    out.append(rec)
    STARTS.clear()
    m = train(KPLSK, xt, yt, n_comp=2)
    rec = krg_record(m, xt, yt, xtest)
    rec.update(name="kplsk_fit", kind="fit", model="KPLSK", corr="squar_exp", poly="constant", n_comp=2, theta0=f([1e-2]))
    rec["starts_log10_pls_pass"] = STARTS[0]
    rec["starts_log10_full_pass"] = STARTS[1]
    rec["y_test"] = f(ytest.ravel())
    out.append(rec)
    return out


# ---------------------------------------------------------------------------
# MFK (Forrester)
# ---------------------------------------------------------------------------


def forrester_hf(x):
    return ((x * 6 - 2) ** 2) * np.sin((x * 6 - 2) * 2)


def forrester_lf(x):
    return 0.5 * ((x * 6 - 2) ** 2) * np.sin((x * 6 - 2) * 2) + (x - 0.5) * 10.0 - 5


def forrester_lf_quadratic(x):
    """Addendum: an LF whose discrepancy is quadratic, so a linear rho and
    trend do not represent the HF exactly (not one of SMT's problems)."""
    return 0.5 * ((x * 6 - 2) ** 2) * np.sin((x * 6 - 2) * 2) + 10.0 * (x - 0.5) ** 2 - 5


def mfk_record(m, xs, ys, xtest):
    levels = []
    for i in range(m.nlvl):
        p = m.optimal_par[i]
        levels.append(
            {
                "optimal_theta": f(np.atleast_1d(m.optimal_theta[i])),
                "rlf": f(m.optimal_rlf_value[i]),
                "sigma2": f(np.atleast_1d(p["sigma2"])[0]),
                "beta": f(p["beta"].ravel()),
                "gamma": f(p["gamma"].ravel()),
                "cond_r": cond_r(p["C"]),
            }
        )
    mse, rhos = m.predict_variances_all_levels(xtest)
    return {
        "x_levels": [f(x) for x in xs],
        "y_levels": [f(y.ravel()) for y in ys],
        "x_offset": f(m.X_offset),
        "x_scale": f(m.X_scale),
        "y_mean": f(np.atleast_1d(m.y_mean)[0]),
        "y_std": f(np.atleast_1d(m.y_std)[0]),
        "levels": levels,
        "x_test": f(xtest),
        "mean": f(m.predict_values(xtest).ravel()),
        "variance": f(m.predict_variances(xtest).ravel()),
        "variance_all_levels": f(mse),
        "sigma2_rho": [f(r) for r in rhos],
    }


def mfk_train(xs, ys, **opts):
    m = MFK(print_global=False, **opts)
    for i, (x, y) in enumerate(zip(xs[:-1], ys[:-1])):
        m.set_training_values(x, y, name=i)
    m.set_training_values(xs[-1], ys[-1])
    m.train()
    return m


def mfk_cases():
    out = []
    xe = np.array([[0.0], [0.4], [0.6], [1.0]])
    xc = np.linspace(0, 1, 11).reshape(-1, 1)
    xe6 = np.linspace(0, 1, 6).reshape(-1, 1)
    xtest = np.linspace(0, 1, 101).reshape(-1, 1)
    yt_test = forrester_hf(xtest)
    for name, xh, extra in (
        ("mfk_forrester_fixed", xe, {}),
        ("mfk_forrester_linear_fixed", xe6, {"rho_regr": "linear", "poly": "linear"}),
        ("mfk_forrester_no_propagation_fixed", xe, {"propagate_uncertainty": False}),
    ):
        xs, ys = [xc, xh], [forrester_lf(xc), forrester_hf(xh)]
        m = mfk_train(xs, ys, theta0=np.array([[0.5], [1.2]]), hyper_opt="NoOp", **extra)
        rec = mfk_record(m, xs, ys, xtest[::5])
        rec.update(name=name, kind="fixed", options={k: v for k, v in extra.items()}, theta0=[[0.5], [1.2]])
        if name == "mfk_forrester_fixed":
            xn = (xc - m.X_offset) / m.X_scale
            yn = (forrester_lf(xc).ravel() - m.y_mean[0]) / m.y_std[0]
            rec["level0_sigma2_unblocked_cholesky"] = unblocked_sigma2(
                xn.tolist(), yn.tolist(), [0.5], m.options["nugget"], len(yn) - 1, float(m.y_std[0])
            )
        out.append(rec)
    # Addendum cases (V&V record): well-conditioned theta.
    for name, xh, lf, extra in (
        ("mfk_forrester_wellcond_fixed", xe, forrester_lf, {}),
        ("mfk_forrester_quadratic_lf_linear_fixed", xe6, forrester_lf_quadratic, {"rho_regr": "linear", "poly": "linear"}),
    ):
        xs, ys = [xc, xh], [lf(xc), forrester_hf(xh)]
        m = mfk_train(xs, ys, theta0=np.array([[10.0], [10.0]]), hyper_opt="NoOp", **extra)
        rec = mfk_record(m, xs, ys, xtest[::5])
        rec.update(name=name, kind="fixed", options={k: v for k, v in extra.items()}, theta0=[[10.0], [10.0]])
        out.append(rec)
    xs, ys = [xc, xe], [forrester_lf(xc), forrester_hf(xe)]
    STARTS.clear()
    m = mfk_train(xs, ys)
    rec = mfk_record(m, xs, ys, xtest)
    rec.update(name="mfk_forrester_fit", kind="fit", options={}, theta0=[[1e-2], [1e-2]])
    rec["starts_log10"] = [list(s) for s in STARTS]
    rec["y_test"] = f(yt_test.ravel())
    out.append(rec)
    # Single-fidelity KRG at the same high-fidelity budget (SMT defaults).
    STARTS.clear()
    k = train(KRG, xe, forrester_hf(xe))
    kr = krg_record(k, xe, forrester_hf(xe), xtest)
    kr.update(name="krg_forrester_hf_only_fit", kind="fit", corr="squar_exp", poly="constant", theta0=f([1e-2]))
    kr["starts_log10"] = STARTS[0]
    kr["y_test"] = f(yt_test.ravel())
    out.append(kr)
    return out


def git_commit(path):
    return subprocess.check_output(["git", "-C", path, "rev-parse", "HEAD"], text=True).strip()


def main():
    os.makedirs(OUT, exist_ok=True)
    meta = {
        "smt_repo": "https://github.com/SMTorg/smt",
        "smt_commit": git_commit(SMT_DIR),
        "smt_commit_date": subprocess.check_output(
            ["git", "-C", SMT_DIR, "log", "-1", "--format=%cI"], text=True
        ).strip(),
        "python": platform.python_version(),
        "numpy": np.__version__,
        "scipy": scipy.__version__,
        "sklearn": sklearn.__version__,
        "machine": platform.machine(),
        "generator": "crates/raffles/verification_and_validation/smt_kriging_gh1190/generate_references.py",
    }
    for fname, cases in (
        ("krg.json", krg_cases()),
        ("kpls.json", kpls_cases()),
        ("mfk.json", mfk_cases()),
    ):
        with open(os.path.join(OUT, fname), "w") as fh:
            json.dump({"meta": meta, "cases": cases}, fh, indent=1)
    print(json.dumps(meta, indent=1))


if __name__ == "__main__":
    main()
