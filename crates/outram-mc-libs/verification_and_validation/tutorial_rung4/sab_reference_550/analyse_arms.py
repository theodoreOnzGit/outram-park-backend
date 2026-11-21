"""gh:#550: k of the node-grid arm E against arms A, B, the ENDF-route baseline
and OpenMC. Run with python -I. Unpaired (Welch) statistics: sem = sd/sqrt(n),
sigma_diff = sqrt(sem_a^2 + sem_b^2)."""
import csv
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
ENS = HERE.parent / "ensemble_550"


def load(path, kcol="k_inf", pitch_col="pitch_cm"):
    out = {}
    with open(path) as f:
        for r in csv.DictReader(f):
            out.setdefault(round(float(r[pitch_col]), 5), []).append(float(r[kcol]))
    return out


def stat(ks):
    n = len(ks)
    m = sum(ks) / n
    sd = math.sqrt(sum((k - m) ** 2 for k in ks) / (n - 1))
    return n, m, sd, sd / math.sqrt(n)


def diff(a, b):
    d = a[1] - b[1]
    s = math.hypot(a[3], b[3])
    return 1e5 * d, 1e5 * s, d / s


ARMS = {
    "E: h5 data, ENDF S(a,b) on THERMR's grid": load(HERE / "arms" / "E_h5_nodegrid.csv"),
    "A: h5 data, NJOY ACE S(a,b)": load(HERE / "arms" / "A_h5_aceSab.csv"),
    "B: h5 data, ENDF S(a,b) 384 log grid": load(HERE / "arms" / "B_h5_endfSab.csv"),
    "baseline: ENDF data, 384 log grid": load(ENS / "outram_seeds.csv"),
    "OpenMC": load(ENS / "openmc_seeds.csv", "k_gen_mean"),
}
KE = "E: h5 data, ENDF S(a,b) on THERMR's grid"
KA, KB = "A: h5 data, NJOY ACE S(a,b)", "B: h5 data, ENDF S(a,b) 384 log grid"
KBASE, KO = "baseline: ENDF data, 384 log grid", "OpenMC"
print("| pitch | arm | n | k | sd [pcm] |")
print("|---|---|---|---|---|")
for p in (2.4, 1.63576, 1.75):
    for name, d in ARMS.items():
        if p in d:
            s = stat(d[p])
            print(f"| {p} | {name} | {s[0]} | {s[1]:.5f} +- {s[3]:.5f} | {1e5 * s[2]:.0f} |")
print()
print("| pitch | difference | pcm | +- | z |")
print("|---|---|---|---|---|")
for p in (2.4, 1.63576, 1.75):
    e = stat(ARMS[KE][p])
    for lab, other in (("E - OpenMC (16 seeds)", KO), ("E - ENDF-route baseline (16 seeds)", KBASE)):
        r = diff(e, stat(ARMS[other][p]))
        print(f"| {p} | {lab} | {r[0]:+.0f} | {r[1]:.0f} | {r[2]:+.2f} |")
    r = diff(stat(ARMS[KBASE][p]), stat(ARMS[KO][p]))
    print(f"| {p} | baseline - OpenMC (16 seeds, recorded) | {r[0]:+.0f} | {r[1]:.0f} | {r[2]:+.2f} |")
print()
# OpenMC 32-seed 2.40 cm reference: 0.70652 +- 0.00020 (sd 113 pcm), pitch_sweep.md
o32 = (32, 0.70652, 0.00113, 0.00020)
e24, a24, b24 = stat(ARMS[KE][2.4]), stat(ARMS[KA][2.4]), stat(ARMS[KB][2.4])
for lab, s in (("E", e24), ("A", a24), ("B", b24)):
    r = diff(s, o32)
    print(f"2.40 cm {lab} - OpenMC (32 seeds, 0.70652 +- 0.00020): {r[0]:+.0f} +- {r[1]:.0f} pcm (z {r[2]:+.2f})")
for lab, x, y in (("E - A", e24, a24), ("E - B", e24, b24), ("A - B", a24, b24)):
    r = diff(x, y)
    print(f"2.40 cm {lab}: {r[0]:+.0f} +- {r[1]:.0f} pcm (z {r[2]:+.2f})")
