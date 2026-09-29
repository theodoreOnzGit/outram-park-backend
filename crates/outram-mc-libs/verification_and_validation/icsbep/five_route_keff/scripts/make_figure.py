#!/usr/bin/env python3
"""Five-route ICSBEP k_eff: per-seed CSV -> summary CSV + markdown table + figure.

    python3 make_figure.py [per_seed_keff.csv] [out_dir]

Defaults: ../data/per_seed_keff.csv, writes ../data/summary_keff.csv,
../data/summary_keff.md and ../figures/five_route_keff.png.

Statistics, and why these:

* per case x route: mean of the per-seed k over n independent seeds, the
  seed-to-seed sample sd, and sem = sd / sqrt(n). The sem is the uncertainty
  quoted. Each run's own internal sigma is NOT averaged in: independent power
  iterations disagree by more than each run thinks it knows (the outram-mc
  ICSBEP record measured this), and the sem from the scatter captures that.
* Delta vs experiment = (mean - 1) in pcm, sigma = sem (the benchmark's own
  uncertainty is drawn as a band on the figure, not folded in).
* Delta vs route 1 (OpenMC + NJOY2016) in pcm, sigma = sqrt(sem_r^2 + sem_1^2):
  the routes are independent runs, nothing cancels.

Presentation only: nothing here is fitted or adjusted. Matplotlib + numpy,
no RNG, no network, no clock -- byte-identical output on re-run.
"""
import csv
import math
import pathlib
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parent
PER_SEED = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else HERE.parent / "data" / "per_seed_keff.csv"
OUT = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else HERE.parent

CASES = [
    # key, title, reference band (half-width in k) or None, band provenance
    ("godiva", "Godiva  HEU-MET-FAST-001", 0.0010, "ICSBEP ±0.0010"),
    ("jemima", "Jemima  IEU-MET-FAST-002", 0.0030, "±0.003 stand-in, not the handbook value"),
    ("hst009", "HEU-SOL-THERM-009 case 1", 0.0060, "±0.006 stand-in, not the handbook value"),
    ("lct008", "LEU-COMP-THERM-008 case 1 (lattice, 11-nuclide tier)", 0.0060,
     "±0.006 stand-in, not the handbook value"),
]
ROUTES = [
    ("route1", "1  OpenMC\nNJOY2016 ACE"),
    ("route2", "2  OpenMC\nRust-NJOY ACE"),
    ("route3", "3  outram-mc\nNJOY2016 ACE"),
    ("route4", "4  outram-mc\nRust-NJOY ENDF"),
    ("route5", "5  outram-mc\nRust-NJOY ACE"),
]
# Categorical slots 1-5 of the dataviz reference palette (validated, light
# mode). Three sit below 3:1 on white, so identity is never colour-alone:
# every point also has its own labelled x position and its own marker.
COLORS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4"]
MARKERS = ["o", "s", "D", "^", "v"]
INK, INK2, GRID = "#0b0b0b", "#52514e", "#e4e3df"


def summarise(rows):
    out = {}
    for case, _, _, _ in CASES:
        for route, _ in ROUTES:
            k = np.array([float(r["k"]) for r in rows if r["case"] == case and r["route"] == route])
            if k.size == 0:
                continue
            sd = float(k.std(ddof=1)) if k.size > 1 else float("nan")
            internal = [float(r["k_std_internal"]) for r in rows
                        if r["case"] == case and r["route"] == route]
            sem = sd / math.sqrt(k.size) if k.size > 1 else internal[0]
            out[(case, route)] = dict(n=k.size, mean=float(k.mean()), sd=sd, sem=sem,
                                      internal_mean=float(np.mean(internal)))
    return out


def main():
    with PER_SEED.open(newline="") as fh:
        rows = list(csv.DictReader(fh))
    s = summarise(rows)

    (OUT / "data").mkdir(parents=True, exist_ok=True)
    lines = ["| case | route | n | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] | Δ vs route 1 [pcm] |",
             "|---|---|---|---|---|---|---|"]
    with (OUT / "data" / "summary_keff.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["case", "route", "n_seeds", "k_mean", "k_sem", "k_sd_seeds",
                    "mean_internal_sigma", "d_exp_pcm", "d_exp_sigma_pcm",
                    "d_route1_pcm", "d_route1_sigma_pcm", "d_route1_nsigma"])
        for case, _, _, _ in CASES:
            ref = s.get((case, "route1"))
            for route, _ in ROUTES:
                v = s.get((case, route))
                if v is None:
                    continue
                d_exp = 1e5 * (v["mean"] - 1.0)
                if ref is not None and route != "route1":
                    d1 = 1e5 * (v["mean"] - ref["mean"])
                    s1 = 1e5 * math.hypot(v["sem"], ref["sem"])
                    n1 = d1 / s1 if s1 > 0 else float("nan")
                    d1s = f"{d1:+.0f} ± {s1:.0f} ({n1:+.1f}σ)"
                else:
                    d1 = s1 = n1 = float("nan")
                    d1s = "reference" if route == "route1" else "—"
                w.writerow([case, route, v["n"], f"{v['mean']:.6f}", f"{v['sem']:.6f}",
                            f"{v['sd']:.6f}", f"{v['internal_mean']:.6f}", f"{d_exp:.1f}",
                            f"{1e5 * v['sem']:.1f}", f"{d1:.1f}", f"{s1:.1f}", f"{n1:.2f}"])
                dexp = f"{d_exp:+.0f} ± {1e5 * v['sem']:.0f}"
                lines.append(f"| {case} | {route} | {v['n']} | {v['mean']:.5f} ± {v['sem']:.5f} | "
                             f"{1e5 * v['sd']:.0f} | {dexp} | {d1s} |")
    (OUT / "data" / "summary_keff.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))

    plt.rcParams.update({"font.size": 8.5, "axes.edgecolor": INK2, "axes.labelcolor": INK,
                         "xtick.color": INK2, "ytick.color": INK2, "text.color": INK})
    fig, axes = plt.subplots(2, 4, figsize=(15, 7.4), sharex=True,
                             gridspec_kw={"height_ratios": [3, 2]}, constrained_layout=True)
    x = np.arange(len(ROUTES))
    for j, (case, title, band, band_note) in enumerate(CASES):
        top, bot = axes[0, j], axes[1, j]
        ref = s.get((case, "route1"))
        for i, (route, _) in enumerate(ROUTES):
            v = s.get((case, route))
            if v is None:
                top.text(x[i], 0.5, "not run", transform=top.get_xaxis_transform(),
                         ha="center", color=INK2, fontsize=7, rotation=90)
                continue
            top.errorbar(x[i], v["mean"], yerr=v["sem"], fmt=MARKERS[i], color=COLORS[i],
                         ms=8, mec="white", mew=1.0, elinewidth=2, capsize=4, zorder=3)
            if ref is not None and route != "route1":
                d1 = 1e5 * (v["mean"] - ref["mean"])
                s1 = 1e5 * math.hypot(v["sem"], ref["sem"])
                bot.errorbar(x[i], d1, yerr=s1, fmt=MARKERS[i], color=COLORS[i], ms=8,
                             mec="white", mew=1.0, elinewidth=2, capsize=4, zorder=3)
                bot.annotate(f"{d1:+.0f}±{s1:.0f}", (x[i], d1), xytext=(8, 0),
                             textcoords="offset points", va="center", fontsize=7, color=INK2)
        if band is not None:
            top.axhspan(1 - band, 1 + band, color=GRID, zorder=0, lw=0)
            top.axhline(1.0, color=INK2, lw=1, zorder=1)
            top.text(0.02, 0.97, f"experiment k = 1 ({band_note})", transform=top.transAxes,
                     va="top", fontsize=7, color=INK2)
        else:
            top.text(0.02, 0.97, band_note, transform=top.transAxes, va="top",
                     fontsize=7, color="#b3261e", fontweight="bold")
        bot.axhline(0.0, color=INK2, lw=1)
        n = max((s[(case, r)]["n"] for r, _ in ROUTES if (case, r) in s), default=0)
        top.set_title(f"{title}\n{n} seed{'s' if n != 1 else ''} per route; error bar = sem over seeds",
                      fontsize=9, loc="left")
        for ax in (top, bot):
            ax.grid(axis="y", color=GRID, lw=0.8)
            ax.set_axisbelow(True)
            for sp in ("top", "right"):
                ax.spines[sp].set_visible(False)
        top.ticklabel_format(axis="y", useOffset=False)
        bot.set_xticks(x, [f"route {i + 1}" for i in range(len(ROUTES))], fontsize=7.5)
        if j == 0:
            top.set_ylabel("k_eff")
            bot.set_ylabel("Δk vs route 1 (OpenMC + NJOY2016) [pcm]")
    handles = [plt.Line2D([], [], ls="", marker=MARKERS[i], color=COLORS[i], ms=8, mec="white",
                          label=lab.replace("\n", " + ")) for i, (_, lab) in enumerate(ROUTES)]
    fig.legend(handles=handles, loc="outside lower center", ncol=5, frameon=False, fontsize=8.5)
    fig.suptitle("Four ICSBEP cases by transport code and nuclear-data route — ENDF/B-VIII.0, 293.6 K\n"
                 "Routes 3/5 on Godiva and Jemima carry two outram-mc ACE-reader defects found by this "
                 "campaign\n(GitHub #366: MT=4 lump replaces the inelastic levels with Q = 0; U-234 fission "
                 "lost). HST-009 routes 3/5 not run (#365). Data as measured, not corrected.",
                 fontsize=10, x=0.01, ha="left", linespacing=1.5)
    (OUT / "figures").mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT / "figures" / "five_route_keff.png", dpi=200, metadata={"Software": None},
                bbox_inches="tight", pad_inches=0.15)
    print("wrote", OUT / "figures" / "five_route_keff.png")


if __name__ == "__main__":
    main()
