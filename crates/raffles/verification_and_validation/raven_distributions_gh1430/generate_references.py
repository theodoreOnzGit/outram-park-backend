# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs of RAVEN's 1-D distributions (gh:#1430, case 1).

Runs RAVEN's own `ravenframework/Distributions1D.py` classes (which wrap
scipy.stats and add truncation) on fixed grids and writes
references/distributions.json. Command, from the workspace root:

    RAVEN_DIR=<clone of idaholab/raven> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/raven_distributions_gh1430/generate_references.py

Python is used here only; nothing in the crate runs it.
"""
import json
import os
import platform
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ravenimport  # noqa: E402

import numpy as np  # noqa: E402
import scipy  # noqa: E402
from ravenframework import Distributions1D as D1  # noqa: E402

PS = [1e-8, 1e-4, 1e-3, 1e-2] + [round(0.05 * i, 2) for i in range(1, 20)] + [0.99, 0.999, 1 - 1e-4, 1 - 1e-8]
INF = float("inf")


def build(kind, p, trunc):
    lo, hi = (trunc if trunc else (None, None))
    if kind == "uniform":
        return D1.BasicUniformDistribution(p["lower"], p["upper"], lo, hi)
    if kind == "normal":
        return D1.BasicNormalDistribution(p["mu"], p["sigma"], lo, hi)
    if kind == "lognormal":
        return D1.BasicLogNormalDistribution(p["mu"], p["sigma"], p["low"], lo, hi)
    if kind == "triangular":
        return D1.BasicTriangularDistribution(p["apex"], p["lower"], p["upper"], lo, hi)
    if kind == "exponential":
        return D1.BasicExponentialDistribution(p["lambda"], p["low"], lo, hi)
    if kind == "weibull":
        return D1.BasicWeibullDistribution(p["k"], p["lambda"], p["low"], lo, hi)
    if kind == "gamma":
        # Distributions.py passes 1/beta (rate -> scale)
        return D1.BasicGammaDistribution(p["alpha"], 1.0 / p["beta"], p["low"], lo, hi)
    if kind == "beta":
        return D1.BasicBetaDistribution(p["alpha"], p["beta"], p["high"] - p["low"], p["low"], lo, hi)
    raise ValueError(kind)


CASES = [
    ("uniform", {"lower": -1.5, "upper": 3.0}, None),
    ("normal", {"mu": 2.0, "sigma": 0.5}, None),
    ("lognormal", {"mu": 0.3, "sigma": 0.6, "low": 0.0}, None),
    ("lognormal", {"mu": 0.3, "sigma": 0.6, "low": 1.5}, None),
    ("triangular", {"lower": 0.0, "apex": 1.0, "upper": 4.0}, None),
    ("exponential", {"lambda": 2.5, "low": 0.5}, None),
    ("weibull", {"k": 1.8, "lambda": 3.0, "low": 0.0}, None),
    ("weibull", {"k": 0.7, "lambda": 2.0, "low": 1.0}, None),
    ("gamma", {"alpha": 2.5, "beta": 1.5, "low": 0.0}, None),
    ("gamma", {"alpha": 0.6, "beta": 2.0, "low": -1.0}, None),
    ("beta", {"alpha": 2.0, "beta": 5.0, "low": 0.0, "high": 1.0}, None),
    ("beta", {"alpha": 0.8, "beta": 1.7, "low": -2.0, "high": 6.0}, None),
    ("normal", {"mu": 1.0, "sigma": 2.0}, [0.0, 3.0]),
    ("lognormal", {"mu": 0.0, "sigma": 1.0, "low": 0.0}, [0.5, 4.0]),
    ("exponential", {"lambda": 1.0, "low": 0.0}, [0.2, 2.0]),
    ("weibull", {"k": 2.0, "lambda": 1.5, "low": 0.0}, [0.3, 2.5]),
    ("gamma", {"alpha": 3.0, "beta": 1.0, "low": 0.0}, [1.0, 6.0]),
    ("beta", {"alpha": 2.0, "beta": 3.0, "low": 0.0, "high": 1.0}, [0.1, 0.8]),
]


def fl(x):
    x = float(x)
    return None if x != x or x in (INF, -INF) else x


out = {"cases": []}
for kind, p, trunc in CASES:
    d = build(kind, p, trunc)
    # x grid: RAVEN's own quantiles of the (truncated) distribution, the
    # bounds, and points just outside / inside the support
    xs = [float(d.inverseCdf(np.float64(q))) for q in PS]
    lo, hi = d._xMin, d._xMax
    extra = []
    for b in (lo, hi):
        if np.isfinite(b):
            extra += [b, b - 0.25, b + 0.25]
    xs = sorted(set(xs + extra))
    pdf = [fl(d.pdf(np.float64(x))) for x in xs]
    cdf = [float(d.cdf(np.float64(x))) for x in xs]
    ps = [0.0] + PS + [1.0]
    ppf = [float(d.inverseCdf(np.float64(q))) for q in ps]
    rec = {"kind": kind, "params": p, "trunc": trunc, "x": xs, "pdf": pdf, "cdf": cdf,
           "p": ps, "ppf": [fl(v) for v in ppf]}
    if trunc is None:
        rec["mean"] = fl(d.untrMean())
        rec["std"] = fl(d.untrStdDev())
    out["cases"].append(rec)

try:
    sha = subprocess.check_output(["git", "-C", os.environ["RAVEN_DIR"], "rev-parse", "HEAD"], text=True).strip()
except Exception as e:
    sha = "unknown: %s" % e
out["meta"] = {"raven_commit": sha, "python": platform.python_version(),
               "numpy": np.__version__, "scipy": scipy.__version__,
               "stubbed_imports": sorted(ravenimport.STUBBED_SEEN)}
with open(os.path.join(HERE, "references", "distributions.json"), "w") as fh:
    json.dump(out, fh, indent=0, allow_nan=False)
print("cases", len(out["cases"]), out["meta"])
