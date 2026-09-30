#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
"""Code-to-code fixture for reading UPSTREAM-format TRISO-ATOPS run files (#449).

For each case this writes a run file in upstream's own format, with its CSVs,
to ``tests/data/upstream_run_file/<case>/``. It then runs the **upstream**
``run_functions.process_run_file`` (commit ``de374c8``) on it, from that
directory (upstream resolves CSV paths against the working directory), and
records what upstream parsed to ``expected.json``. That is the constants array,
the toggles, the node counts, nuclides, inventories, the temperature
profiles, times and accident temperatures, plus ``error_count``, or the
exception type if upstream raised.

``tests/triso_atops_upstream_run_file.rs`` reads every case with
``boon_lay::triso_atops_fork::run_file::upstream::read_upstream_run_file`` and
asserts agreement.

Same precedent as ``gen_triso_atops_reference.py``: this *executes* the
third-party upstream reference to produce V&V data; it is not documentation.

Usage (upstream clone at crates/boon-lay/upstream_source/TRISO-ATOPS @ de374c8):
    python3 dev/gen_upstream_run_file_reference.py
Requires numpy and pandas (upstream's run_functions really uses pandas here).
"""

from __future__ import annotations

import json
import logging
import os
import sys
from pathlib import Path

CRATE = Path(__file__).resolve().parent.parent
UPSTREAM = CRATE / "upstream_source" / "TRISO-ATOPS" / "trisoatops" / "utility_functions"
OUT = CRATE / "tests" / "data" / "upstream_run_file"

BASE = {
    "f_hm": [1e-5, ""], "f_sic": [2e-5, ""], "f_inc": [3e-5, ""], "f_inc_sic": [4e-5, ""],
    "a_graph": [4.5e-3, "m"], "a_grain": [1e-5, "m"], "k_plate": [7.5e-5, "s^-1"],
    "run_time": [40, "yr"], "irradiation_time": [1095, "d"], "k_clean": [8.77e-5, "s^-1"],
    "r_kernel": [2.13e-4, "m"], "a_SiC": [3.5e-5, "m"],
    "f_inc_acc": [6.6e-5, ""], "f_inc_sic_acc": [1.7e-4, ""], "x_liftoff": [0.05, ""],
    "n_radial": 2, "n_axial": 3,
}
NUCLIDES = ["Kr-85", "I-131", "Cs-137"]
INV = [[1.0e5, 2.0e5], [3.0e6, 4.0e6], [5.0e4, 6.0e4]]           # [nuclide][ring]
CORE = [[900.0, 850.0], [1000.0, 950.0], [950.0, 900.0]]           # [axial][ring]
GRAPH = [[890.0, 840.0], [990.0, 940.0], [940.0, 890.0]]
TIMES_H = [0.0, 1.0, 5.0, 20.0]
ACC = [[[900.0 + 10 * t + k for k in range(3)] for t in range(4)],  # [ring][time][axial]
       [[850.0 + 12 * t + k for k in range(3)] for t in range(4)]]


def write(d: Path, name: str, rows, header=None, index=None):
    lines = []
    if header is not None:
        lines.append(",".join(header))
    for i, r in enumerate(rows):
        cells = ([str(index[i])] if index is not None else []) + [repr(float(x)) for x in r]
        lines.append(",".join(cells))
    (d / name).write_text("\n".join(lines) + "\n")


def case_csv_all(d: Path) -> dict:
    """Accident on, HPS on; every table as a CSV path."""
    write(d, "nuclides.csv", [], header=["nuclide"])
    (d / "nuclides.csv").write_text("nuclide\n" + "\n".join(NUCLIDES) + "\n")
    write(d, "inventories.csv", INV, header=["nuclide", "ring1", "ring2"], index=NUCLIDES)
    write(d, "core.csv", CORE, header=["axial", "r1", "r2"], index=[0, 1, 2])
    write(d, "graphite.csv", GRAPH, header=["axial", "r1", "r2"], index=[0, 1, 2])
    write(d, "times.csv", [[t] for t in TIMES_H], header=["t"])
    for r in range(2):
        write(d, f"accident_ring{r + 1}.csv", ACC[r], header=["t", "a1", "a2", "a3"], index=TIMES_H)
    return {**BASE, "hps_tog": True, "accident_tog": True,
            "Nuclides": NUCLIDES, "Inventories": "inventories.csv",
            "Core_Temps": ["core.csv", True, True], "Graphite_Temps": ["graphite.csv", True, True],
            "Times": ["times.csv", "hr"],
            "Accident_Temps": ["accident_ring1.csv", "accident_ring2.csv"]}


def case_inline(d: Path) -> dict:
    """Accident on, HPS off (k_clean ignored); inline arrays, headerless profiles, Times via read_profile."""
    write(d, "core.csv", CORE)
    write(d, "graphite.csv", GRAPH)
    write(d, "times.csv", [[t * 60] for t in TIMES_H])
    return {**BASE, "hps_tog": False, "accident_tog": True,
            "Nuclides": NUCLIDES, "Inventories": INV,
            "Core_Temps": ["core.csv", False, False], "Graphite_Temps": ["graphite.csv", False, False],
            "Times": [["times.csv", False, False], "min"], "Accident_Temps": ACC}


def case_normal_only(d: Path) -> dict:
    """Accident off: the three accident constants are not read (stay 0)."""
    write(d, "core.csv", CORE, header=["axial", "r1", "r2"], index=[0, 1, 2])
    base = {k: v for k, v in BASE.items() if k not in ("f_inc_acc", "f_inc_sic_acc", "x_liftoff")}
    return {**base, "hps_tog": True, "accident_tog": False, "Nuclides": NUCLIDES, "Inventories": INV,
            "Core_Temps": ["core.csv", True, True], "Graphite_Temps": ["core.csv", True, True]}


def case_wrong_unit(d: Path) -> dict:
    c = case_normal_only(d)
    c["a_graph"] = [0.45, "cm"]
    return c


def case_bad_time_unit(d: Path) -> dict:
    c = case_normal_only(d)
    c["run_time"] = [40, "week"]
    return c


def case_fraction_sum_over_one(d: Path) -> dict:
    c = case_inline(d)
    c["f_hm"] = [0.6, ""]
    c["f_inc_acc"] = [0.5, ""]
    return c


def case_fraction_sum_exactly_one(d: Path) -> dict:
    c = case_inline(d)
    for k in ("f_hm", "f_sic", "f_inc", "f_inc_sic", "f_inc_acc", "f_inc_sic_acc"):
        c[k] = [0.0, ""]
    c["f_hm"] = [0.25, ""]
    c["f_inc_acc"] = [0.75, ""]
    return c


CASES = [case_csv_all, case_inline, case_normal_only, case_wrong_unit, case_bad_time_unit,
         case_fraction_sum_over_one, case_fraction_sum_exactly_one]


def listify(x):
    import numpy as np
    if x is None:
        return None
    return np.asarray(x).tolist()


def main() -> int:
    if not UPSTREAM.is_dir():
        sys.exit(f"upstream clone not found at {UPSTREAM}")
    # run_functions imports `utility_functions.calculation_functions`, so the
    # package's parent (trisoatops/) goes on the path.
    sys.path.insert(0, str(UPSTREAM.parent))
    import utility_functions.run_functions as rf  # noqa: E402

    log = logging.getLogger("fixture")
    log.addHandler(logging.NullHandler())
    log.propagate = False
    here = os.getcwd()
    for fn in CASES:
        name = fn.__name__.removeprefix("case_")
        d = OUT / name
        d.mkdir(parents=True, exist_ok=True)
        run = fn(d)
        (d / "run.json").write_text(json.dumps(run, indent=1) + "\n")
        os.chdir(d)
        try:
            (constants, options, react, nuclides, inv, core, graph, times, acc, _acc_t,
             errors) = rf.process_run_file(json.loads((d / "run.json").read_text()), log, 0)
            expected = {
                "raised": None, "error_count": int(errors),
                "constants": listify(constants), "options": options,
                "react_config": listify(react),
                "nuclides": None if nuclides is None else [str(n[0]) for n in nuclides],
                "inventories": listify(inv), "core": listify(core), "graphite": listify(graph),
                "times": listify(times), "accident_temps": listify(acc),
            }
        except Exception as e:  # upstream raises for some malformed inputs
            expected = {"raised": type(e).__name__, "error_count": None}
        finally:
            os.chdir(here)
        (d / "expected.json").write_text(json.dumps(expected, indent=1) + "\n")
        print(f"{name}: raised={expected['raised']} error_count={expected['error_count']}")
    # -- nuclide-name probe: upstream's nuclide_import_accident outcome per name
    import utility_functions.calculation_functions as calc  # noqa: E402
    probe = {}
    for name in ["Cs-137", "cs137", "CS-137", "cs-137", "Ag-110m", "ag110m", "Xx-999", "Cs-999", "???", "137"]:
        try:
            out = calc.nuclide_import_accident([[name, 1.0]], 1.0, log)
            probe[name] = {"outcome": "kept" if out else "skipped", "canonical": list(out)}
        except Exception as e:
            probe[name] = {"outcome": "raised", "error": type(e).__name__}
    (OUT / "nuclide_names.json").write_text(json.dumps(probe, indent=1) + "\n")
    print("nuclide names:", {k: v["outcome"] for k, v in probe.items()})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
