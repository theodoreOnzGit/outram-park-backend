"""Plot the GeN-Foam twoPhase port's 1D_boiling run (gh:#1218).

Input: the CSVs written by
    TWO_PHASE_OUT=<dir> cargo test --release -p outram-foam-appbuilder-lib \
        --test genfoam_two_phase_1d_boiling one_d_boiling_full_transient
Usage: python plot_1d_boiling.py <dir> <out_dir>

Upstream ships no reference result for this case, so the figures show the
port only; there is no upstream curve to overlay (see README.md).
"""
import csv
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

src, out = sys.argv[1], sys.argv[2]
prof = list(csv.DictReader(open(f"{src}/profiles.csv")))
hist = list(csv.DictReader(open(f"{src}/history.csv")))


def at(t):
    rows = [r for r in prof if abs(float(r["t"]) - t) < 1e-6]
    x = [float(r["x"]) for r in rows]
    return rows, x


fig, ax = plt.subplots(figsize=(7, 4.2))
for t in [6.2, 6.5, 7.0, 8.0, 9.0, 9.5, 10.0]:
    rows, x = at(t)
    if rows:
        ax.plot(x, [float(r["alpha_vapour_normalized"]) for r in rows], label=f"t = {t:g} s")
for xb in (0.5, 1.5):
    ax.axvline(xb, color="0.6", lw=0.8, ls=":")
ax.set_xlabel("x along the channel [m]  (heated zone 0.5-1.5 m)")
ax.set_ylabel("void fraction alpha.vapour / (1 - alpha_s)")
ax.set_title("1D_boiling, OUTRAM port (no upstream reference shipped)")
ax.legend(fontsize=8)
fig.tight_layout()
fig.savefig(f"{out}/1d_boiling_void_profiles.png", dpi=130)

fig, axs = plt.subplots(1, 3, figsize=(13, 4.0), sharey=True)
for a, t in zip(axs, [3.0, 7.0, 9.0]):
    rows, x = at(t)
    for key, lab in [
        ("T_liquid", "liquid"),
        ("T_vapour", "vapour"),
        ("T_structure", "structure"),
        ("T_interface", "T.interface (T_sat)"),
    ]:
        v = [float(r[key]) for r in rows]
        # the structure has a temperature only where a power model sits
        v = [float("nan") if (key == "T_structure" and y == 0.0) else y for y in v]
        a.plot(x, v, label=lab, ls="--" if key == "T_vapour" else "-")
    a.set_title(f"t = {t:g} s")
    a.set_xlabel("x [m]")
axs[0].set_ylabel("temperature [K]")
axs[0].legend(fontsize=8)
fig.suptitle("1D_boiling temperatures, OUTRAM port (no upstream reference shipped)")
fig.tight_layout()
fig.savefig(f"{out}/1d_boiling_temperature_profiles.png", dpi=130)

t = [float(r["t"]) for r in hist]
fig, axs = plt.subplots(3, 1, figsize=(7, 7.5), sharex=True)
axs[0].plot(t, [float(r["u_liquid_inlet"]) for r in hist])
axs[0].axhline(0.71198, color="0.5", ls="--", lw=0.8, label="P1 closed form 0.712 m/s")
axs[0].set_ylabel("inlet U.liquid [m/s]")
axs[0].legend(fontsize=8)
axs[1].plot(t, [float(r["max_alpha_vapour_normalized"]) for r in hist])
axs[1].set_ylabel("max void fraction")
axs[2].plot(t, [float(r["max_T_structure"]) for r in hist])
axs[2].set_ylabel("max T structure [K]")
axs[2].set_xlabel("time [s]")
for a in axs:
    a.axvline(7.0, color="r", lw=0.8, ls=":")
axs[0].set_title("1D_boiling history (power off at 7 s, red)")
fig.tight_layout()
fig.savefig(f"{out}/1d_boiling_history.png", dpi=130)
