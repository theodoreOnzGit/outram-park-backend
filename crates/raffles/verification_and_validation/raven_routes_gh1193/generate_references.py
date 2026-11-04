# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs for the RAVEN calibration and surrogate routes (gh:#1193).

Runs the UPSTREAM code itself (RAVEN's own classes, plus the numpy / scipy /
scikit-learn they delegate to) on fixed inputs and writes the results as JSON
next to this script, in references/. The Rust port in crates/raffles is
compared against these files by tests/raven_routes_gh1193.rs. Python is used
only here, to generate references; nothing in the crate runs it.

Command (from the workspace root; RAVEN_DIR is a clone of idaholab/raven):

    RAVEN_DIR=<clone> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/raven_routes_gh1193/generate_references.py

What is RAVEN's own code and what is a transcribed driver is stated per case.
In short:
  * MCMC: RAVEN's `Metropolis` / `AdaptiveMetropolis` objects are constructed
    for real, and their own `_useRealization`, `_checkAcceptance`,
    `tuneScalingParam` and `_updateAdaptiveParams` run. Only the sampler-loop
    plumbing that needs RAVEN's job handler and DataObjects
    (`localGenerateInput` / `localFinalizeActualSampling`) is transcribed
    below, line for line, with its upstream line numbers.
  * Polynomial chaos: RAVEN's `Distributions`, `Quadratures`, `IndexSets`,
    `OrthoPolynomials` and `GaussPolynomialRom` run unmodified; this script
    only wires them as `Samplers/SparseGridCollocation.py` does.
  * Gaussian process: RAVEN's `GaussianProcessRegressor` ROM picks the kernel
    (its own `pickKernel` / `customKernel`, over RAVEN's vendored
    scikit-optimize kernels) and sklearn's `GaussianProcessRegressor` does the
    numerics, as in RAVEN.
"""

import json
import os
import platform
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ravenimport  # noqa: E402  (sets up the standalone RAVEN import)

import numpy as np  # noqa: E402
import scipy  # noqa: E402
import scipy.special  # noqa: E402
import sklearn  # noqa: E402
from sklearn.gaussian_process import GaussianProcessRegressor as SkGPR  # noqa: E402

from ravenframework import Distributions as D  # noqa: E402
from ravenframework import DistributionsND  # noqa: E402,F401
from ravenframework import IndexSets, OrthoPolynomials, Quadratures  # noqa: E402
from ravenframework.utils import randomUtils as R  # noqa: E402
from ravenframework.Samplers.MCMC.Metropolis import Metropolis  # noqa: E402
from ravenframework.Samplers.MCMC.AdaptiveMetropolis import AdaptiveMetropolis  # noqa: E402
from ravenframework.SupervisedLearning.GaussPolynomialRom import GaussPolynomialRom  # noqa: E402
from ravenframework.SupervisedLearning.ScikitLearn.GaussianProcess.GaussianProcessRegressor import (  # noqa: E402
    GaussianProcessRegressor as RavenGPR,
)

OUT = os.path.join(HERE, "references")


def f(x):
    """Floats as Python reprs: shortest round-trip decimal, exact in f64."""
    if isinstance(x, (list, tuple, np.ndarray)):
        return [f(v) for v in x]
    return float(x)


def dist(spec):
    kind, a, b = spec
    d = D.Uniform(a, b) if kind == "uniform" else D.Normal(a, b)
    d.initializeDistribution()
    return d


# ---------------------------------------------------------------------------
# MCMC
# ---------------------------------------------------------------------------


class Recorder:
    """Wraps a RAVEN distribution and records every `rvs()` it returns."""

    def __init__(self, inner, log):
        self._inner = inner
        self._log = log

    def rvs(self, *a):
        v = self._inner.rvs(*a)
        self._log.append(f(np.atleast_1d(v)))
        return v

    def __getattr__(self, name):
        return getattr(self._inner, name)


def mcmc_case(name, kind, priors, loglik, log_form, limit, burn_in, tune,
              tune_interval, initial, seed, adaptive_interval=20):
    R.randomSeed(seed)
    names = ["x%d" % i for i in range(len(priors))]
    S = Metropolis() if kind == "metropolis" else AdaptiveMetropolis()
    S._burnIn = burn_in
    S._tune = tune
    S._tuneInterval = tune_interval
    S._countsUntilTune = tune_interval
    S._adaptiveInterval = adaptive_interval
    S._likelihood = "L"
    S._logLikelihood = log_form
    S.distDict = {n: dist(p) for n, p in zip(names, priors)}
    S._updateValues = {n: (None if initial is None else initial[i]) for i, n in enumerate(names)}
    S._acceptDist.initializeDistribution()
    uniforms = []
    S._acceptDist = Recorder(S._acceptDist, uniforms)
    proposals = []
    # --- Metropolis.initialize, Metropolis.py:65-116 (no <proposal> given):
    #     a Normal(0, 0.2 * untruncated std) per variable; an initial value
    #     drawn from the prior when none is given.
    if kind == "metropolis":
        for var in S._updateValues:
            dd = S.distDict[var]
            std = S._stdProposalDefault * dd.untruncatedStdDev()
            p = D.Normal(0.0, std)
            p.initializeDistribution()
            S._proposal[var] = Recorder(p, proposals)
            if S._updateValues[var] is None:
                S._updateValues[var] = dd.rvs()
    else:
        # --- AdaptiveMetropolis.initialize, AdaptiveMetropolis.py:84-238, the
        #     branch with no <proposal> (line 178): diagonal ensemble
        #     covariance from the priors' untruncated std, lambda = 2.38^2/d,
        #     the ensemble mean taken from the CURRENT values (before any
        #     initial value is drawn, line 215), then the initial draw.
        d = len(names)
        S._lambda = 2.38 ** 2 / d
        S._ensembleCov = np.zeros((d, d))
        for i, var in enumerate(names):
            S._orderedVarsList.append(var)
            S._ensembleCov[i, i] = S.distDict[var].untruncatedStdDev() ** 2
            S._orderedVars[var] = [[var]]
            S.variables2distributionsMapping[var] = {"name": var, "dim": 1}
            S.distributions2variablesIndexList[var] = [1]
        S._ensembleMean = np.asarray([S._updateValues[var] for var in S._orderedVarsList])
        S._proposal = Recorder(
            S.constructProposalDistribution(np.zeros(d), S._lambda * S._ensembleCov.ravel()), proposals)
        for var in names:
            if S._updateValues[var] is None:
                S._updateValues[var] = S.distDict[var].rvs()
    init_vals = [float(S._updateValues[n]) for n in names]
    steps = []
    current_rlz = None
    S.counter = 0
    for c in range(1, limit + 1):
        S.counter = c
        values = dict(S._updateValues)
        n_prop = len(proposals)
        if c > 1:
            # --- localGenerateInput: Metropolis.py:118-142 /
            #     AdaptiveMetropolis.py:256-285 — add scaling * proposal draw,
            #     then CLAMP to the distribution bounds.
            if kind == "metropolis":
                for key, value in S._updateValues.items():
                    values[key] = value + S._proposal[key].rvs() * S._scaling
            else:
                newVal = S._proposal.rvs()
                for i, var in enumerate(S._orderedVarsList):
                    values[var] = S._updateValues[var] + newVal[i] * S._scaling
            for key in names:
                lo, hi = S.distDict[key].lowerBound, S.distDict[key].upperBound
                if lo is not None and values[key] < lo:
                    values[key] = lo
                if hi is not None and values[key] > hi:
                    values[key] = hi
        theta = [float(values[n]) for n in names]
        ll = loglik(np.array(theta))
        rlz = dict(values)
        rlz["L"] = ll if log_form else float(np.exp(ll))
        # --- MCMC.localFinalizeActualSampling, MCMC.py:351-394.
        rec = {"counter": c, "proposed": f(theta), "likelihood": f(rlz["L"])}
        if c == 1:
            current_rlz = rlz
        if c > 1:
            alpha = S._useRealization(rlz, current_rlz)  # RAVEN's own
            S.netLogPosterior = alpha
            S._accepted = S._checkAcceptance(alpha)  # RAVEN's own
            if S._accepted:
                current_rlz = rlz
            S._updateValues = dict((var, current_rlz[var]) for var in S._updateValues)
            rec["increment"] = proposals[n_prop:] if kind == "metropolis" else proposals[n_prop]
            if kind == "metropolis":
                rec["increment"] = [v[0] for v in rec["increment"]]
            rec["uniform"] = uniforms[-1][0]
            rec["alpha"] = f(alpha)
            rec["accepted"] = bool(S._accepted)
            rec["accept_rate"] = f(S._acceptRate)
        if S._tune:
            S._acceptInTune = S._acceptInTune + 1 if S._accepted else S._acceptInTune
            S._countsUntilTune -= 1
        if not S._countsUntilTune and S._tune:
            S._scaling = S.tuneScalingParam(S._scaling, S._acceptInTune / float(S._tuneInterval))
            S._countsUntilTune = S._tuneInterval
            S._acceptInTune = 0
        if kind == "adaptive" and c > 1:
            # --- AdaptiveMetropolis.localFinalizeActualSampling, line 326.
            S._updateAdaptiveParams(S.netLogPosterior, current_rlz)  # RAVEN's own
            if not isinstance(S._proposal, Recorder):
                S._proposal = Recorder(S._proposal, proposals)
            rec["lambda"] = f(S._lambda)
        rec["scaling"] = f(S._scaling)
        rec["current"] = f([current_rlz[n] for n in names])
        steps.append(rec)
    out = {
        "name": name, "kind": kind, "priors": [list(p) for p in priors],
        "log_likelihood_form": log_form, "limit": limit, "burn_in": burn_in,
        "tune": tune, "tune_interval": tune_interval,
        "adaptive_interval": adaptive_interval, "seed": seed,
        "initial_given": initial is not None, "initial": init_vals,
        "proposal_std_default": f(S._stdProposalDefault), "steps": steps,
    }
    if kind == "adaptive":
        out["ensemble_mean"] = f(S._ensembleMean)
        out["ensemble_cov"] = f(S._ensembleCov)
        out["lambda"] = f(S._lambda)
    return out


def gauss_conjugate_loglik(y, sigma):
    y = np.asarray(y)
    return lambda th: float(-0.5 * np.sum(((y - th[0]) / sigma) ** 2))


def m2_loglik(th):
    return float(-0.5 * ((th[0] - 0.1) / 0.2) ** 2 - 0.5 * ((th[1] - 0.8) / 0.3) ** 2)


def bivariate_loglik(th):
    m = np.array([1.0, -1.0])
    P = np.linalg.inv(np.array([[1.0, 0.8], [0.8, 1.0]]))
    d = th - m
    return float(-0.5 * d @ P @ d)


def mcmc_cases():
    cases = [
        mcmc_case("metropolis_conjugate_1d", "metropolis", [("normal", 0.0, 2.0)],
                  gauss_conjugate_loglik([1.2, 0.7, 1.9, 1.1, 0.4], 1.0), True,
                  400, 50, True, 100, [0.5], 20261104),
        mcmc_case("metropolis_bounded_linear_2d", "metropolis",
                  [("uniform", 0.0, 1.0), ("normal", 1.0, 0.5)], m2_loglik, False,
                  300, 20, True, 25, None, 7),
        mcmc_case("adaptive_correlated_2d", "adaptive",
                  [("normal", 0.0, 3.0), ("normal", 0.0, 3.0)], bivariate_loglik, True,
                  600, 100, True, 50, [0.0, 0.0], 11),
        mcmc_case("adaptive_no_burn_in_2d", "adaptive",
                  [("normal", 0.0, 3.0), ("normal", 0.0, 3.0)], bivariate_loglik, True,
                  300, 0, True, 100, [0.5, 0.5], 13),
    ]
    # AdaptiveMetropolis with no <initial>: the ensemble mean is built from the
    # still-unset values (AdaptiveMetropolis.py:215) before the initial draw.
    try:
        mcmc_case("adaptive_no_initial", "adaptive",
                  [("normal", 0.0, 3.0), ("normal", 0.0, 3.0)], bivariate_loglik, True,
                  60, 10, True, 50, None, 17)
        no_initial = "ran"
    except Exception as e:  # recorded, not hidden
        no_initial = "%s: %s" % (type(e).__name__, e)
    return cases, no_initial


# ---------------------------------------------------------------------------
# Polynomial chaos (GaussPolynomialRom over a Smolyak or tensor grid)
# ---------------------------------------------------------------------------


def pce_case(name, priors, index_type, order, grid, func, test_points, importance=None):
    features = ["x%d" % i for i in range(len(priors))]
    dists = {n: dist(p) for n, p in zip(features, priors)}
    quads, polys = {}, {}
    for n, p in zip(features, priors):
        # SparseGridCollocation._generateQuadsAndPolys: the DEFAULT choice
        # (preferredQuadrature / preferredPolynomials) for each distribution.
        qtype = dists[n].preferredQuadrature
        ptype = dists[n].preferredPolynomials
        q = Quadratures.factory.returnInstance(qtype)
        q.initialize(dists[n])
        quads[n] = q
        pl = OrthoPolynomials.factory.returnInstance(ptype)
        pl.initialize(q)
        polys[n] = pl
    imp = {n: (1.0 if importance is None else importance[i]) for i, n in enumerate(features)}
    iset = IndexSets.factory.returnInstance(index_type)
    iset.initialize(features, imp, order)
    sg = Quadratures.factory.returnInstance(grid)
    sg.initialize(features, iset, dists, quads, None)
    rom = GaussPolynomialRom()
    rom.features = features
    rom.target = ["y"]
    rom.initialize({"SG": sg, "dists": dists, "quads": quads, "polys": polys, "iSet": iset})
    X = np.array(sg.points())
    Y = np.array([[func(x)] for x in X])
    rom._train(X, Y)
    coeffs = rom.polyCoeffDict["y"]
    indices, partials = rom.getSensitivities()
    totals = rom.getTotalSensitivities(indices)
    preds = [rom.__evaluateLocal__([np.array(p)])["y"] for p in test_points]
    return {
        "name": name, "priors": [list(p) for p in priors], "index_set": index_type,
        "order": order, "grid": grid, "importance": [imp[n] for n in features],
        "quadratures": [quads[n].type for n in features],
        "polynomials": [polys[n].type for n in features],
        "index_points": [list(map(int, ix)) for ix in iset.points],
        "grid_points": f(X), "grid_weights": f(sg.weights()),
        "training_y": f(Y[:, 0]),
        "coefficients": [[list(map(int, k)), float(v)] for k, v in coeffs.items()],
        "mean": float(rom.__mean__()), "variance": float(rom.__variance__()),
        "sobol": [[[int(n[1:]) for n in k], float(v)] for k, v in indices.items()],
        "partial_variances": [[[int(n[1:]) for n in k], float(v)] for k, v in partials.items()],
        "total_sobol": [[[int(n[1:]) for n in k], float(v)] for k, v in totals.items()],
        "test_points": f(test_points), "predictions": f(preds),
    }


def poly3(x):
    return 1.0 + 2.0 * x[0] - 0.5 * x[1] + 0.75 * x[0] * x[1] - 0.3 * x[0] ** 3 + 0.2 * x[1] ** 2 * x[0]


def sudret(x):
    return np.prod(3.0 * x ** 2 + 1.0) / 2 ** len(x)


def ishigami(x):
    return np.sin(x[0]) + 7.0 * np.sin(x[1]) ** 2 + 0.1 * x[2] ** 4 * np.sin(x[0])


def pce_cases():
    pi = np.pi
    return [
        pce_case("total_degree_poly3_uniform_normal", [("uniform", -1.0, 3.0), ("normal", 2.0, 0.5)],
                 "TotalDegree", 3, "smolyak", poly3,
                 [[0.3, 1.7], [-0.9, 2.6], [2.5, 1.1]]),
        pce_case("tensor_product_sudret_3d", [("uniform", 0.0, 1.0)] * 3, "TensorProduct", 2,
                 "smolyak", sudret, [[0.1, 0.5, 0.9], [0.7, 0.2, 0.4]]),
        pce_case("hyperbolic_cross_ishigami", [("uniform", -pi, pi)] * 3, "HyperbolicCross", 8,
                 "smolyak", ishigami, [[0.5, -1.0, 2.0]]),
        pce_case("total_degree_ishigami_8", [("uniform", -pi, pi)] * 3, "TotalDegree", 8,
                 "smolyak", ishigami, [[0.5, -1.0, 2.0], [-2.0, 0.3, -0.7]]),
        pce_case("tensor_grid_anisotropic", [("uniform", 0.0, 2.0), ("normal", -1.0, 1.5)],
                 "TotalDegree", 3, "tensor", poly3, [[0.4, -0.2]], importance=[2.0, 1.0]),
    ]


def quadrature_tables():
    rows = []
    for n in range(1, 21):
        x, w = scipy.special.roots_legendre(n)
        hx, hw = scipy.special.roots_hermitenorm(n)
        rows.append({"n": n, "legendre_x": f(x), "legendre_w": f(w),
                     "hermitenorm_x": f(hx), "hermitenorm_w": f(hw)})
    pts = [-1.0, -0.73, -0.2, -3e-6, 0.0, 4e-6, 0.31, 0.999, 1.0, 2.4]
    evals = []
    for n in range(0, 13):
        evals.append({"n": n,
                      "legendre": f([scipy.special.eval_legendre(n, p) for p in pts]),
                      "hermitenorm": f([scipy.special.eval_hermitenorm(n, p) for p in pts])})
    return {"roots": rows, "eval_points": pts, "evals": evals}


# ---------------------------------------------------------------------------
# Gaussian process (RAVEN's GaussianProcessRegressor ROM -> sklearn)
# ---------------------------------------------------------------------------


def raven_kernel(name, features, anisotropic, custom=None):
    rom = RavenGPR()
    rom.features = features
    if name is None:
        return None
    if name == "Custom":
        return rom.customKernel(custom, anisotropic)
    return rom.pickKernel(name, anisotropic)


def kernel_record(k):
    return {"repr": repr(k), "theta": f(k.theta), "bounds": f(k.bounds) if k.n_dims else []}


def gp_case(name, X, y, kernel_name, anisotropic, optimizer, alpha, normalize_y,
            test, custom=None, set_params=None):
    X = np.asarray(X, dtype=float)
    y = np.asarray(y, dtype=float)
    k = raven_kernel(kernel_name, ["x%d" % i for i in range(X.shape[1])], anisotropic, custom)
    if set_params is not None:
        k = k.clone_with_theta(np.log(np.asarray(set_params, dtype=float)))
    # RAVEN's defaults (GaussianProcessRegressor.getInputSpecification):
    # alpha 1e-10, n_restarts_optimizer 0, normalize_y False,
    # optimizer 'fmin_l_bfgs_b', random_state None.
    gp = SkGPR(kernel=k, alpha=alpha, n_restarts_optimizer=0, normalize_y=normalize_y,
               optimizer=optimizer, random_state=None)
    gp.fit(X, y)
    mean, std = gp.predict(np.asarray(test, dtype=float), return_std=True)
    lml, grad = (gp.log_marginal_likelihood(gp.kernel_.theta, eval_gradient=True)
                 if gp.kernel_.n_dims else (gp.log_marginal_likelihood_value_, np.zeros(0)))
    return {
        "name": name, "kernel_name": kernel_name, "custom": custom, "anisotropic": anisotropic,
        "optimizer": optimizer, "alpha": alpha, "normalize_y": normalize_y,
        "x": f(X), "y": f(y), "test": f(test),
        "kernel_initial": kernel_record(gp.kernel) if gp.kernel is not None else None,
        "kernel_fitted": kernel_record(gp.kernel_),
        "lml": float(lml), "lml_gradient": f(grad),
        "mean": f(mean), "std": f(std),
        "train_mean": f(gp.predict(X)),
    }


def raven_multioutput_std():
    """RAVEN wraps the GPR in MultiOutputRegressor by default (<multioutput>
    defaults to True). ScikitLearnBase.__evaluateLocal__ asks for return_std
    and falls back to the mean alone on TypeError. Run it and record what
    RAVEN's own evaluate returns."""
    rom = RavenGPR()
    rom.features = ["x0"]
    rom.target = ["y"]
    rom.multioutputWrapper = True
    rom.initializeModel({"kernel": None, "alpha": 1e-10, "n_restarts_optimizer": 0,
                         "normalize_y": False, "random_state": None,
                         "optimizer": "fmin_l_bfgs_b"})
    X = np.array([[0.0], [1.0], [2.0]])
    rom.model.fit(X, np.array([[0.0], [1.0], [0.5]]))
    out = rom.__evaluateLocal__(np.array([[0.5]]))
    return {"model": type(rom.model).__name__, "keys": sorted(out.keys())}


def gp_cases():
    x1 = [[0.0], [0.8], [1.7], [2.4], [3.3], [4.1], [5.0]]
    y1 = [np.sin(v[0]) for v in x1]
    t1 = [[0.4], [1.2], [2.0], [2.9], [3.7], [4.6], [0.0]]
    x2 = [[a, b] for a in (0.0, 0.6, 1.3, 2.0) for b in (0.0, 0.9, 1.8)]
    y2 = [a * a + np.sin(3.0 * b) for a, b in x2]
    t2 = [[0.3, 0.4], [1.0, 1.5], [1.9, 0.2], [0.6, 0.9]]
    x5 = [[v] for v in (0.0, 0.45, 1.1, 1.6, 2.2, 2.9, 3.4, 4.0)]
    y5 = [np.sin(2.0 * v[0]) + 0.1 * v[0] for v in x5]
    return [
        gp_case("raven_default_kernel_1d", x1, y1, None, False, "fmin_l_bfgs_b", 1e-10, False, t1),
        gp_case("rbf_anisotropic_fixed_normalized_2d", x2, y2, "RBF", True, None, 1e-10, True, t2,
                set_params=[0.7, 1.6]),
        gp_case("matern25_fixed_2d", x2, y2, "Matern", False, None, 1e-6, False, t2,
                set_params=[1.3]),
        gp_case("constant_rbf_white_fixed_2d", x2, y2, "Custom", False, None, 1e-10, False, t2,
                custom="Constant*RBF+WhiteNoise", set_params=[2.0, 0.9, 0.01]),
        gp_case("rbf_fit_1d", x5, y5, "RBF", False, "fmin_l_bfgs_b", 1e-10, False, t1),
        gp_case("constant_rbf_anisotropic_fit_2d", x2, y2, "Custom", True, "fmin_l_bfgs_b",
                1e-10, False, t2, custom="Constant*RBF"),
        # Added after the first run (V&V record, addendum): the case above
        # ends on a numerically singular Gram matrix; this one is conditioned.
        gp_case("matern_fit_2d", x2, y2, "Matern", False, "fmin_l_bfgs_b", 1e-6, False, t2),
    ]


def git_commit(path):
    try:
        return subprocess.check_output(["git", "-C", path, "rev-parse", "HEAD"], text=True).strip()
    except Exception:
        return "unknown"


def main():
    os.makedirs(OUT, exist_ok=True)
    meta = {
        "raven_commit": git_commit(ravenimport.RAVEN),
        "python": platform.python_version(),
        "numpy": np.__version__, "scipy": scipy.__version__, "sklearn": sklearn.__version__,
        "machine": platform.machine(),
    }
    mcmc, no_initial = mcmc_cases()
    meta["adaptive_no_initial"] = no_initial
    gp = gp_cases()
    meta["raven_gpr_multioutput"] = raven_multioutput_std()
    pce = pce_cases()
    meta["stubbed_imports"] = sorted(ravenimport.STUBBED_SEEN)
    for fname, obj in (("mcmc.json", {"meta": meta, "cases": mcmc}),
                       ("pce.json", {"meta": meta, "cases": pce}),
                       ("quadrature.json", {"meta": meta, **quadrature_tables()}),
                       ("gp.json", {"meta": meta, "cases": gp})):
        with open(os.path.join(OUT, fname), "w") as fh:
            json.dump(obj, fh, indent=1)
            fh.write("\n")
    print(json.dumps(meta, indent=1))


if __name__ == "__main__":
    main()
