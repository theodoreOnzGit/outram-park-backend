#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
"""DIAGNOSTIC ONLY (gh:#413 check (a)): temperature sensitivity of the MHTGR
final releases. NOTHING HERE IS ADOPTED -- the committed #413 result stays on
the Fig. 5 curves as digitised (kovan 066e691).

All four Fig. 5 curves are shifted uniformly by -15..+15 degC and the final
releases re-run through UPSTREAM TRISO-ATOPS (de374c8) with exactly the method
of `dev/mhtgr_stoyer_upstream.py` (whole-core curves, Eq. 29, the paper's x10
applied after the initial puff). Printed k_plate at every shift; the 7.5e-4
diagnostic at 0 and +/-15 degC only. No other input varies. The normal-
operation state is computed once (it does not depend on the accident curves).

A uniform shift is a crude proxy: real digitisation error varies along the
curve. Output: `verification_and_validation/mhtgr_stoyer/diagnostic_tshift_final_ratio.csv`.
The port is not re-run here: it matches upstream to < 4e-11 on the unshifted
accident path (tests/mhtgr_stoyer_workflow.rs), and nothing it computes
differently depends on the shift.
"""
import csv, sys, types, logging
from pathlib import Path
import numpy as np

CRATE = Path(__file__).resolve().parent.parent
DATA = CRATE / "verification_and_validation" / "mhtgr_stoyer"
PKG = CRATE / "upstream_source" / "TRISO-ATOPS" / "trisoatops"

def rows(name):
    with open(DATA / name) as f:
        return list(csv.DictReader(l for l in f if not l.startswith("#")))

def main():
    sys.modules.setdefault("pandas", types.ModuleType("pandas"))
    sys.path.insert(0, str(PKG))
    import trisoatops as tri
    from utility_functions.run_functions import convert_time
    tri.calc.vectorized_format = lambda x: x  # display rounding bypassed, as in the main driver
    log = logging.getLogger("tshift"); log.disabled = True
    consts = {r["key"]: r for r in rows("constants.csv")}
    temps_k = np.array([[float(r[f"ring{i}_k"]) for i in (1, 2, 3)] for r in rows("table07_core_temperature_k.csv")])
    curves = {}
    for r in rows("fig05_accident_temperature_c.csv"):
        curves.setdefault(r["core_fraction_percent"], []).append((max(0.0, float(r["time_h"])), float(r["temperature_c"])))
    weights = {"5": 0.05, "20": 0.2, "25": 0.25, "50": 0.5}
    out = [["case", "k_plate", "shift_c", "nuclide", "final_as_paper_ci", "paper_final_ci", "ratio",
            "vent_5", "vent_20", "vent_25", "vent_50"]]
    for case, inv_file, dT, paper in (("a", "table06_case_a_inventory_ci.csv", 0.0, "table10_case_a_accident_release_ci.csv"),
                                      ("b", "table11_case_b_inventory_ci.csv", 250.0, "table14_case_b_accident_release_ci.csv")):
        pap = {r["nuclide"]: float(r["final_release_ci_as_printed"]) for r in rows(paper)}
        for kp, shifts in ((None, (-15, -10, -5, 0, 5, 10, 15)), (7.5e-4, (-15, 0, 15))):
            c = lambda k: kp if (k == "k_plate" and kp is not None) else float(consts[k][f"case_{case}"])
            yr = convert_time("yr")
            constants = np.array([c("f_hm"), c("f_sic"), c("f_inc"), c("f_inc_sic"), c("a_graph"), c("a_grain"),
                                  c("k_plate"), c("run_time") * yr, c("irradiation_time") * yr, c("k_clean"),
                                  c("r_kernel"), c("a_SiC"), c("f_inc_acc"), c("f_inc_sic_acc"), c("x_liftoff")])
            inv_rows = rows(inv_file)
            names = np.vstack([r["nuclide"] for r in inv_rows])
            inventories = np.array([[float(r[f"ring{i}_ci"]) for i in (1, 2, 3)] for r in inv_rows])
            core_c = temps_k - 273.15 + dT
            output, nodal, fmt = tri.normal_operation(constants, True, names, inventories, core_c, core_c, log)
            idx = {n: i for i, n in enumerate(fmt[1])}
            for s in shifts:
                per, vents = {}, {}
                for pct, pts in curves.items():
                    times = np.array([t for t, _ in pts]) * 3600.0
                    temp = np.array([T for _, T in pts]) + s
                    acc = np.broadcast_to(temp[None, :, None], (3, temp.size, 14)).copy()
                    frac, _ = tri.calc.coolant_release(times, acc)
                    vents[pct] = float(frac[-1])
                    totals, _ = tri.accident_case(constants, names, nodal, acc, times, log, True)
                    per[pct] = {n: float(np.asarray(v)[-1]) for n, v in totals.items()}
                for n, pf in pap.items():
                    eq29 = sum(weights[p] * per[p][n] for p in weights)
                    k = idx[n]
                    initial = float(output[k][3]) + c("x_liftoff") * float(output[k][4])
                    final = initial + (eq29 - initial) / 10.0
                    out.append([case.upper(), c("k_plate"), s, n, repr(final), pf, f"{final / pf:.5f}"] +
                               [f"{vents[p]:.5f}" for p in ("5", "20", "25", "50")])
                print(f"case {case} k_plate {c('k_plate')} shift {s:+d}: done")
    dst = DATA / "diagnostic_tshift_final_ratio.csv"
    with open(dst, "w", newline="") as f:
        f.write("# DIAGNOSTIC ONLY (gh:#413 check (a)) -- NOT ADOPTED. All four Fig. 5 curves (kovan 066e691) shifted uniformly by shift_c;\n"
                "# upstream TRISO-ATOPS de374c8, same method as upstream_case_*_accident.csv (Eq. 29, paper x10 after the initial puff, flagged).\n"
                "# Generated by dev/mhtgr_stoyer_tshift_diagnostic.py. A uniform shift is a crude proxy for digitisation error.\n")
        csv.writer(f).writerows(out)
    print(dst.relative_to(CRATE))

if __name__ == "__main__":
    main()
