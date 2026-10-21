"""#413 check (c), part 2: overlay upstream TRISO-ATOPS cumulative release vs
time on Stoyer et al.'s Fig. 6 (Case A) and Fig. 7 (Case B).

DIAGNOSTIC ONLY. The reported #413 result stays the dense-PCHIP final
(median 0.90) under the rule fixed in advance. Nothing here is adopted as a
result.

Method (the same inputs and path as dev/mhtgr_stoyer_upstream.py):
- Upstream `accident_case` for each Fig. 5 curve (dense PCHIP 0.1 h file)
  applied to the whole core. Its totals are a time series on the curve's
  grid, truncated by `coolant_release` where venting stops (at the curve's
  peak on the dense grid). After the truncation the release is held at its
  last value: upstream's vent mask releases nothing more.
- Eq. (29) at every time: sum_p w_p S_p(t) with w = 0.05/0.2/0.25/0.5.
- As published: initial + (Eq29(t) - initial)/10 (s.III.A.5).
- Evaluated at each digitised Fig. 6/7 time, for the six plotted nuclides.
  Ratios are given for the total and for the increment (value - initial),
  with the paper's increment taken as figure value - Table 10/14 initial.
- Also at t = 60 h and at our final (last time of every curve), against
  Tables 10/14.

Output: verification_and_validation/mhtgr_stoyer/
  upstream_case_{a,b}_accident_pchip_series.csv   (our series, 1 h steps)
  fig67_overlay_ours_vs_paper.csv                 (at the digitised times)

Usage: python3 dev/mhtgr_stoyer_fig67_overlay.py   (needs numpy)
"""
import csv, sys, types, logging
from pathlib import Path
import numpy as np

CRATE = Path(__file__).resolve().parent.parent
DATA = CRATE / "verification_and_validation" / "mhtgr_stoyer"
PKG = CRATE / "upstream_source" / "TRISO-ATOPS" / "trisoatops"
NUCS = ["Kr-85", "Xe-133", "I-131", "Sr-90", "Cs-137", "Ag-110m"]
W = {"5": 0.05, "20": 0.2, "25": 0.25, "50": 0.5}


def rows(name):
    with open(DATA / name) as f:
        return list(csv.DictReader(l for l in f if not l.startswith("#")))


def main():
    sys.modules.setdefault("pandas", types.ModuleType("pandas"))
    sys.path.insert(0, str(PKG))
    import trisoatops as tri  # noqa: E402
    from utility_functions.run_functions import convert_time  # noqa: E402
    tri.calc.vectorized_format = lambda x: x  # unrounded, as in the upstream driver
    log = logging.getLogger("mhtgr")
    consts = {r["key"]: r for r in rows("constants.csv")}
    temps_k = np.array([[float(r[f"ring{i}_k"]) for i in (1, 2, 3)]
                        for r in rows("table07_core_temperature_k.csv")])
    curves = {}
    for r in rows("fig05_accident_temperature_c_pchip_0p1h.csv"):
        curves.setdefault(r["core_fraction_percent"], []).append(
            (max(0.0, float(r["time_h"])), float(r["temperature_c"])))
    overlay = []
    for case, inv_file, dT, fig_file, table_file in (
            ("a", "table06_case_a_inventory_ci.csv", 0.0, "fig06_case_a_total_release_ci.csv",
             "table10_case_a_accident_release_ci.csv"),
            ("b", "table11_case_b_inventory_ci.csv", 250.0, "fig07_case_b_total_release_ci.csv",
             "table14_case_b_accident_release_ci.csv")):
        c = lambda k: float(consts[k][f"case_{case}"])
        yr = convert_time("yr")
        constants = np.array([c("f_hm"), c("f_sic"), c("f_inc"), c("f_inc_sic"), c("a_graph"),
                              c("a_grain"), c("k_plate"), c("run_time") * yr,
                              c("irradiation_time") * yr, c("k_clean"), c("r_kernel"), c("a_SiC"),
                              c("f_inc_acc"), c("f_inc_sic_acc"), c("x_liftoff")])
        inv_rows = rows(inv_file)
        order = [r["nuclide"] for r in inv_rows]
        names = np.vstack(order)
        inventories = np.array([[float(r[f"ring{i}_ci"]) for i in (1, 2, 3)] for r in inv_rows])
        core_c = temps_k - 273.15 + dT
        output, nodal, _ = tri.normal_operation(constants, True, names, inventories, core_c, core_c, log)
        initial = {n: float(output[order.index(n)][3]) + c("x_liftoff") * float(output[order.index(n)][4])
                   for n in NUCS}
        series = {}  # pct -> (hours array, {nuclide: totals array})
        for pct, pts in curves.items():
            hours = np.array([t for t, _ in pts])
            temp = np.array([T for _, T in pts])
            acc = np.broadcast_to(temp[None, :, None], (3, temp.size, 14)).copy()
            totals, _ = tri.accident_case(constants, names, nodal, acc, hours * 3600.0, log, True)
            kept = {n: np.asarray(totals[n], dtype=float) for n in NUCS}
            series[pct] = (hours[:len(kept[NUCS[0]])], kept)

        def at(n, t):
            eq = 0.0
            for pct, (h, s) in series.items():
                eq += W[pct] * float(np.interp(min(t, h[-1]), h, s[n]))
            return eq, initial[n] + (eq - initial[n]) / 10.0

        ends = {pct: h[-1] for pct, (h, _) in series.items()}
        with open(DATA / f"upstream_case_{case}_accident_pchip_series.csv", "w", newline="") as f:
            f.write(f"# Upstream TRISO-ATOPS de374c8, Stoyer Case {case.upper()}: cumulative release vs time [Ci], "
                    "dense PCHIP Fig. 5, Eq. (29); as_paper = initial + (eq29 - initial)/10. "
                    f"Each curve's series ends where upstream's vent mask truncates it (h): {ends}; held after. "
                    "Generated by dev/mhtgr_stoyer_fig67_overlay.py (#413 check (c), DIAGNOSTIC).\n")
            w = csv.writer(f)
            w.writerow(["time_h"] + [f"{n}_eq29" for n in NUCS] + [f"{n}_as_paper" for n in NUCS])
            for t in range(0, 141):
                v = [at(n, float(t)) for n in NUCS]
                w.writerow([t] + [repr(e) for e, _ in v] + [repr(p) for _, p in v])
        table = {r["nuclide"]: r for r in rows(table_file)}
        for r in rows(fig_file):
            n, t, paper = r["nuclide"], float(r["time_h"]), float(r["release_ci"])
            if n not in NUCS:
                continue
            _, ours = at(n, max(t, 0.0))
            t_init = float(table[n]["initial_release_ci"])
            overlay.append([case.upper(), n, t, paper, ours, ours / paper,
                            ours - initial[n], paper - t_init,
                            (ours - initial[n]) / (paper - t_init) if paper - t_init > 0 else ""])
        print(f"case {case}: curve ends (h) {ends}")
        for n in NUCS:
            t60 = at(n, 60.0)[1]
            fin = at(n, 1e9)[1]
            tab = float(table[n]["final_release_ci_as_printed"])
            print(f"  {n:8} ours(60 h) {t60:.4e}  ours(final) {fin:.4e}  Table final {tab:.3e}  "
                  f"ratio@60h {t60 / tab:.3f}  ratio@final {fin / tab:.3f}  initial ours {initial[n]:.3e} "
                  f"table {float(table[n]['initial_release_ci']):.3e}")
    with open(DATA / "fig67_overlay_ours_vs_paper.csv", "w", newline="") as f:
        f.write("# #413 check (c) part 2, DIAGNOSTIC: upstream (dense PCHIP, Eq. 29, x10 on the increment) vs the "
                "maintainer's Fig. 6/7 digitisation, at each digitised time. increment = value - initial "
                "(ours: upstream's initial; paper: Table 10/14 initial). Generated by dev/mhtgr_stoyer_fig67_overlay.py.\n")
        w = csv.writer(f)
        w.writerow(["case", "nuclide", "time_h", "paper_ci", "ours_ci", "ratio_total",
                    "ours_increment_ci", "paper_increment_ci", "ratio_increment"])
        for row in overlay:
            w.writerow([row[0], row[1], f"{row[2]:.4f}"] + [repr(x) if isinstance(x, float) else x for x in row[3:]])


if __name__ == "__main__":
    main()
