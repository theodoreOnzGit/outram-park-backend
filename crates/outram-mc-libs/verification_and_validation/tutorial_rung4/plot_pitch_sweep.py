# SPDX-License-Identifier: GPL-3.0
"""Plot k-infinity against V_mod/V_fuel for the LCT-008 pin-cell pitch sweep.

Reads the committed CSVs in data/ (outram-mc rows from
examples/lct008_pitch_sweep.rs, OpenMC rows from
openmc_inputs/lct008_pin_cell_openmc.py) and writes
figures/kinf_vs_moderator_ratio.png. Both codes are our calculations
(no literature curve), drawn with markers and thin lines; error bars are 1 sigma. Both codes are shown by the generation-mean estimator
(the one outram-mc reports), with the standard error over active generations.

  python plot_pitch_sweep.py
"""
import csv
import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = pathlib.Path(__file__).resolve().parent
D = HERE / "data"
VM_VF_LCT008 = 1.8413
BLUE, ORANGE = "#2a78d6", "#eb6834"  # dataviz reference palette slots 1 and 2


def rows(name):
    p = D / name
    return list(csv.DictReader(p.open())) if p.is_file() else []


def vm_vf(pitch):
    import math
    rf, rc = 0.514858, 0.602996
    return (pitch * pitch - math.pi * rc * rc) / (math.pi * rf * rf)


def series(name, kcol, scol):
    r = sorted(rows(name), key=lambda x: float(x["pitch_cm"]))
    return ([vm_vf(float(x["pitch_cm"])) for x in r], [float(x[kcol]) for x in r],
            [float(x[scol]) for x in r])


fig, axes = plt.subplots(1, 2, figsize=(11, 4.4), sharey=True)
for ax, tag, title in ((axes[0], "case1", "case-1 water, 1511 ppm soluble boron"),
                       (axes[1], "noboron", "soluble boron removed (ablation)")):
    x, y, s = series(f"openmc_cheap11_{tag}.csv", "k_gen_mean", "k_gen_sem")
    ax.errorbar(x, y, yerr=s, color=ORANGE, marker="s", ms=6, lw=1.2, capsize=2,
                label="OpenMC 0.16.1-dev25, NJOY2016 ACE (11 nuclides)")
    x, y, s = series(f"pitch_sweep_cheap11_{tag}.csv", "k_inf", "k_std")
    ax.errorbar(x, y, yerr=s, color=BLUE, marker="o", ms=6, lw=1.2, ls=":", capsize=2,
                label="outram-mc, ENDF/B-VIII.0 direct (11 nuclides)")
    if tag == "case1":
        x, y, s = series("pitch_sweep_full36_case1.csv", "k_inf", "k_std")
        ax.errorbar(x, y, yerr=s, color=BLUE, marker="^", ms=11, lw=0, elinewidth=1, mew=1.5,
                    capsize=2, mfc="none", label="outram-mc, all 36 nuclides")
    ax.axvline(VM_VF_LCT008, color="#52514e", lw=1, ls="--")
    ax.text(VM_VF_LCT008 + 0.15, 1.27 if tag == "case1" else 0.5, "LCT-008 pitch\n(1.63576 cm)", color="#52514e",
            fontsize=9)
    ax.set_title(title, fontsize=10)
    ax.set_xlabel("moderator-to-fuel volume ratio  V_mod / V_fuel")
    ax.grid(alpha=0.25, lw=0.5)
axes[0].set_ylabel("k-infinity (reflective pin cell)")
axes[0].legend(fontsize=8, loc="upper right")
fig.suptitle("LCT-008 pin in an infinite square lattice, 293.6 K, H in H2O S(a,b): "
             "k-infinity against moderator ratio (verification, not validation)", fontsize=10)
fig.tight_layout()
out = HERE / "figures" / "kinf_vs_moderator_ratio.png"
fig.savefig(out, dpi=130)
print("wrote", out)
