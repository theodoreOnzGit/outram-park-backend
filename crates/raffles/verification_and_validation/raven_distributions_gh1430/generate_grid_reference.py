# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference grid levels from RAVEN's real GridEntity (gh:#1430, case 2).

For each case the stepLength is computed exactly as `Grid`/`GridBase`
(`GridEntities.py:307`, `:349-356`) computes it from the XML <grid> node, then
the real `GridEntity.initialize` builds `gridVectors`, which are written out.

    RAVEN_DIR=<clone of idaholab/raven> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/raven_distributions_gh1430/generate_grid_reference.py
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
from ravenframework import GridEntities  # noqa: E402

EQUAL = [(0.1, 0.9, 2), (0.0, 1.0, 10), (0.05, 0.95, 4), (0.0, 0.8, 4), (0.0, 1.0, 3),
         (0.1, 0.9, 7), (0.01, 0.99, 13), (0.0, 1.0, 100), (0.0, 1.0, 7), (0.2, 0.7, 9),
         (0.001, 0.999, 29), (0.0, 1.0, 1000),
         (0.0, 0.99, 10), (0.0, 0.9, 3), (0.0, 0.97, 7), (0.0, 0.99, 100), (0.0, 0.999, 1000), (0.0, 0.95, 19)]
CUSTOM = [[0.0, 0.1, 0.35, 0.5, 0.9],
          [0.05, 0.123456789012345, 0.5, 0.87654321098765, 0.95],
          [0.0, 1.0 / 3.0, 2.0 / 3.0, 0.9],
          [0.02, 0.1, 0.4, 0.6, 0.9, 0.98]]


def vectors(lower, upper, step_len):
    g = GridEntities.GridEntity()
    g.initialize(initDictionary={"dimensionNames": ["v"], "lowerBounds": {"v": lower},
                                 "upperBounds": {"v": upper}, "stepLength": {"v": step_len}})
    return [float(v) for v in g.gridContainer["gridVectors"]["v"]]


out = {"equal": [], "custom": []}
for lo, hi, steps in EQUAL:
    lin = np.linspace(lo, hi, steps + 1)  # GridEntities.py:356
    step_len = [round(lin[1] - lin[0], 14)]  # :307
    try:
        lv = vectors(min(lin), max(lin), step_len)
        err = None
    except Exception as e:  # RAVEN's own consistency check (GridEntities.py:474-480)
        lv, err = None, str(e)
    out["equal"].append({"lower": lo, "upper": hi, "steps": steps, "levels": lv, "raven_error": err})
for lv in CUSTOM:
    step_len = [round(lv[k + 1] - lv[k], 14) for k in range(len(lv) - 1)]  # :307, custom
    try:
        got = vectors(min(lv), max(lv), step_len)
        err = None
    except Exception as e:
        got, err = None, str(e)
    out["custom"].append({"input": lv, "levels": got, "raven_error": err})

try:
    sha = subprocess.check_output(["git", "-C", os.environ["RAVEN_DIR"], "rev-parse", "HEAD"], text=True).strip()
except Exception as e:
    sha = "unknown: %s" % e
out["meta"] = {"raven_commit": sha, "python": platform.python_version(), "numpy": np.__version__,
               "stubbed_imports": sorted(ravenimport.STUBBED_SEEN)}
with open(os.path.join(HERE, "references", "grid.json"), "w") as fh:
    json.dump(out, fh, indent=1, allow_nan=False)
for r in out["equal"]:
    print(r["lower"], r["upper"], r["steps"], None if r["levels"] is None else len(r["levels"]), r["raven_error"])
for r in out["custom"]:
    print(r["input"], r["raven_error"])
