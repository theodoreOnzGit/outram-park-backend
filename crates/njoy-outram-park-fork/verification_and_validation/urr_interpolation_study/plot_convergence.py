"""Figure 1: NJOY2016's unresolved-range output converges onto this crate's
directly-evaluated values under parameter-grid refinement.

Reads deviations.csv (from deviations.py) and writes fig1_convergence.{pdf,png}.

Design notes
------------
* Categorical palette: the seven validated light-mode slots. Three of them sit
  below 3:1 contrast on a white surface, which obligates visible direct labels
  rather than a legend box -- so every series is labelled at its right-hand end.
  Distinct marker shapes carry identity a second way, so the figure also
  survives greyscale printing, which matters for a preprint.
* The shaded band is NJOY's own output quantisation. Its PENDF carries seven
  significant figures, so for a cross section near 1.4e-2 b the last printed
  digit is 1e-8 b, i.e. ~7e-7 relative. Deviations inside that band are
  indistinguishable from zero *in the reference*, and are drawn but not
  claimed. Without it a log axis reaching 4e-10 would imply a precision the
  comparison cannot support.
"""
import csv, os
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import LogLocator, NullFormatter

HERE = os.path.dirname(os.path.abspath(__file__))

# Validated categorical slots (light mode). Do not reorder: the ordering is the
# CVD-safety mechanism, and this set was checked with the palette validator.
SERIES = "#2a78d6 #eb6834 #1baf7a #eda100 #e87ba4 #008300 #4a3aa7".split()
INK, INK2, INK3 = "#0b0b0b", "#52514e", "#8a8985"
MARKERS = ["o", "s", "^", "D", "v", "P", "X"]

LEVELS = ["base", "x2", "x4", "x8", "x16"]
XTICK = ["NJOY\nas-is", "×2", "×4", "×8", "×16"]
# NJOY prints 7 significant figures; near 1.4e-2 b that is ~7e-7 relative.
QUANTISATION = 7.0e-7

rows = list(csv.DictReader(open(os.path.join(HERE, "deviations.csv"))))
by_e = {}
for r in rows:
    by_e.setdefault(float(r["energy_ev"]), {})[r["level"]] = abs(float(r["rel_dev"]))
# 4.368748e4 and 4.368749e4 are the same energy shaded either side of an MF=3
# discontinuity; keep one so the figure has seven distinct energies.
energies = sorted(by_e)
energies = [e for i, e in enumerate(energies) if i == 0 or e / energies[i-1] > 1.0001]

plt.rcParams.update({
    "font.size": 8.5, "axes.labelsize": 9, "axes.titlesize": 9.5,
    "xtick.labelsize": 8.5, "ytick.labelsize": 8.5,
    "axes.edgecolor": INK3, "axes.linewidth": 0.6,
    "figure.dpi": 200, "savefig.bbox": "tight", "pdf.fonttype": 42,
})
fig, ax = plt.subplots(figsize=(6.2, 3.8))
ax.set_facecolor("white"); fig.patch.set_facecolor("white")

x = range(len(LEVELS))
ax.axhspan(1e-11, QUANTISATION, color="#f0efec", zorder=0, lw=0)
ax.axhline(QUANTISATION, color=INK3, lw=0.6, ls=(0, (3, 2)), zorder=1)

ends = []
for i, e in enumerate(energies):
    y = [by_e[e].get(l, float("nan")) for l in LEVELS]
    ax.plot(x, y, color=SERIES[i], lw=1.8, marker=MARKERS[i], ms=4.6,
            mew=0.8, mec="white", zorder=3, clip_on=False, solid_capstyle="round")
    ends.append((y[-1], f"{e/1e3:.4g} keV", SERIES[i]))

# Declutter the right-hand labels in log space, then draw them.
import math
ends.sort(key=lambda t: t[0])
pos = [math.log10(v) for v, _, _ in ends]
MIN_GAP = 0.34
for i in range(1, len(pos)):
    if pos[i] - pos[i-1] < MIN_GAP:
        pos[i] = pos[i-1] + MIN_GAP
for (v, lab, col), p in zip(ends, pos):
    ax.annotate(lab, xy=(len(LEVELS)-1, v), xytext=(len(LEVELS)-1 + 0.13, 10**p),
                textcoords="data", color=INK, fontsize=8, va="center", ha="left",
                annotation_clip=False,
                arrowprops=dict(arrowstyle="-", color=col, lw=0.9,
                                shrinkA=2, shrinkB=1))

ax.set_yscale("log")
ax.set_xlim(-0.25, len(LEVELS) - 1 + 0.10)
ax.set_ylim(1e-10, 2e-2)
ax.set_xticks(list(x)); ax.set_xticklabels(XTICK, color=INK2)
ax.yaxis.set_minor_locator(LogLocator(base=10.0, subs=tuple(range(2, 10))))
ax.yaxis.set_minor_formatter(NullFormatter())
ax.grid(axis="y", which="major", color="#e6e5e1", lw=0.6, zorder=0)
ax.set_axisbelow(True)
for s in ("top", "right"): ax.spines[s].set_visible(False)
ax.spines["left"].set_color(INK3); ax.spines["bottom"].set_color(INK3)
ax.tick_params(colors=INK3, length=3, width=0.6)

ax.set_xlabel("unresolved parameter-grid refinement", color=INK2, labelpad=6)
ax.set_ylabel("|relative deviation| from direct evaluation", color=INK2, labelpad=6)
ax.set_title("NJOY2016's unresolved-range output converges onto direct evaluation\n"
             "U-234 (MAT 9225), MF=3 MT=18, infinitely dilute, 0 K",
             color=INK, loc="left", pad=10)
# Placed low-left, the one region of the band no series crosses -- at the
# band edge it ran straight through the three converged lines.
ax.annotate("shaded: below NJOY's own output resolution\n"
            "(7 significant figures, ~7×10$^{-7}$ relative here)",
            xy=(0.02, 3.0e-9), xycoords=("axes fraction", "data"),
            color=INK2, fontsize=7.5, va="center", ha="left", linespacing=1.5)

for ext in ("pdf", "png"):
    fig.savefig(os.path.join(HERE, f"fig1_convergence.{ext}"))
print("wrote fig1_convergence.pdf and .png")
