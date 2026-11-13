# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs of RAVEN's BasicStatistics PostProcessor (gh:#1430, case 3).

Runs RAVEN's real `BasicStatistics._runLocal` on a fixed 10^4-row, 3-variable
dataset built only from exactly rounded IEEE operations (+, -, *, /), so the
Rust test rebuilds the SAME bits without reading a data file. Needs xarray and
pandas on top of the numpy/scipy/scikit-learn of the other generator.

    RAVEN_DIR=<clone of idaholab/raven> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/raven_distributions_gh1430/generate_statistics_reference.py
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
import xarray as xr  # noqa: E402
from ravenframework.Models.PostProcessors.BasicStatistics import BasicStatistics  # noqa: E402

N = 10000
VARS = ["x", "y", "z"]


def dataset():
    """Same formulas as `statistics_dataset()` in tests/raven_statistics_gh1430.rs."""
    x = np.empty(N)
    y = np.empty(N)
    z = np.empty(N)
    for i in range(N):
        a = float((i * 7919 + 13) % 10007) / 10007.0
        b = float((i * 104729 + 5) % 10009) / 10009.0
        x[i] = a * a + 0.5 * b
        y[i] = 3.0 * a - b * b
        z[i] = b * b * b - 0.25 * a
    return x, y, z


x, y, z = dataset()
tag = "RAVEN_sample_ID"
ds = xr.Dataset({"x": (tag, x), "y": (tag, y), "z": (tag, z)}, coords={tag: np.arange(N)})

bs = BasicStatistics()
bs.sampleTag = tag
bs.pbPresent = False
bs.biased = False
bs.dynamic = False
bs.pivotParameter = None
bs.sampleSize = N
bs.parameters = {"targets": set(VARS)}
tv = set(VARS)
bs.toDo = {
    "expectedValue": [{"targets": tv, "prefix": "mean"}],
    "variance": [{"targets": tv, "prefix": "var"}],
    "sigma": [{"targets": tv, "prefix": "sigma"}],
    "skewness": [{"targets": tv, "prefix": "skew"}],
    "kurtosis": [{"targets": tv, "prefix": "kurt"}],
    "median": [{"targets": tv, "prefix": "med"}],
    "minimum": [{"targets": tv, "prefix": "min"}],
    "maximum": [{"targets": tv, "prefix": "max"}],
    "covariance": [{"targets": tv, "features": tv, "prefix": "cov"}],
    "pearson": [{"targets": tv, "features": tv, "prefix": "pear"}],
    "spearman": [{"targets": tv, "features": tv, "prefix": "spear"}],
    "percentile": [{"targets": tv, "prefix": "pct", "percent": [0.05, 0.25, 0.5, 0.75, 0.95],
                    "strPercent": ["5", "25", "50", "75", "95"], "interpolation": "linear"}],
}
pbw = xr.Dataset()
out = bs._runLocal((ds, pbw))
if not isinstance(out, dict):
    out = {k: out[k] for k in out}


def scalar_per_var(name):
    v = out[name]
    return {k: float(v[k].values) for k in VARS}


def matrix(name):
    da = out[name]
    t = list(da.coords["targets"].values)
    f = list(da.coords["features"].values)
    return {"targets": t, "features": f, "values": da.values.tolist()}


res = {"n": N, "vars": VARS}
for k in ["expectedValue", "variance", "sigma", "skewness", "kurtosis", "median", "minimum", "maximum"]:
    res[k] = scalar_per_var(k)
for k in ["covariance", "pearson", "spearman"]:
    res[k] = matrix(k)
pc = out["percentile"]
res["percentile"] = {k: {"percent": [float(p) for p in pc[k].coords["percent"].values],
                         "values": [float(v) for v in pc[k].values]} for k in VARS}

try:
    sha = subprocess.check_output(["git", "-C", os.environ["RAVEN_DIR"], "rev-parse", "HEAD"], text=True).strip()
except Exception as e:
    sha = "unknown: %s" % e
res["meta"] = {"raven_commit": sha, "python": platform.python_version(), "numpy": np.__version__,
               "scipy": scipy.__version__, "xarray": xr.__version__,
               "stubbed_imports": sorted(ravenimport.STUBBED_SEEN), "keys": sorted(out.keys())}
with open(os.path.join(HERE, "references", "statistics.json"), "w") as fh:
    json.dump(res, fh, indent=1, allow_nan=False)
print(res["meta"])
print({k: res[k] for k in ["expectedValue", "variance", "skewness", "kurtosis"]})
