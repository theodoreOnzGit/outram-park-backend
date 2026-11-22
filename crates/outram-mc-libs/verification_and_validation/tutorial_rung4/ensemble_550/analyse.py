# SPDX-License-Identifier: AGPL-3.0-only
"""gh:#550 seed-ensemble analysis: outram-mc vs OpenMC pin-cell k-infinity.

Reads outram_seeds.csv (column k_inf, k_std) and openmc_seeds.csv (column
k_gen_mean, k_gen_sem; the generation mean over the active generations, the
estimator the original single runs used), both in this folder.

Per pitch and code: n, mean, sd (ddof = 1) and sem = sd / sqrt(n) of the
per-seed k, plus the mean single-run standard error for comparison.
Difference = outram-mc - OpenMC, sigma = sqrt(sem_a^2 + sem_b^2), z = diff /
sigma. The gate is |z| <= 2 (Welch, unpaired: the seeds of the two codes are
not correlated). Output is markdown on stdout.

  python3 -I analyse.py
"""
import csv
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
PITCHES = [1.63576, 1.75, 2.4]


def load(name, kcol, scol):
    out = {}
    with (HERE / name).open() as f:
        for r in csv.DictReader(f):
            out.setdefault(round(float(r["pitch_cm"]), 5), {})[int(r["seed"])] = (
                float(r[kcol]), float(r[scol]))
    return out


def stats(d):
    ks = [d[s][0] for s in sorted(d)]
    n = len(ks)
    m = sum(ks) / n
    sd = math.sqrt(sum((k - m) ** 2 for k in ks) / (n - 1))
    single = sum(d[s][1] for s in d) / n
    return n, m, sd, sd / math.sqrt(n), single


def main():
    a = load("outram_seeds.csv", "k_inf", "k_std")
    b = load("openmc_seeds.csv", "k_gen_mean", "k_gen_sem")
    print("| pitch [cm] | code | n | mean k | sd | sem | mean single-run sigma | sd / single-run sigma |")
    print("|---|---|---|---|---|---|---|---|")
    rows = []
    for p in PITCHES:
        sa, sb = stats(a[p]), stats(b[p])
        for name, s in (("outram-mc", sa), ("OpenMC", sb)):
            print(f"| {p} | {name} | {s[0]} | {s[1]:.5f} | {s[2]*1e5:.0f} pcm | {s[3]*1e5:.0f} pcm | "
                  f"{s[4]*1e5:.0f} pcm | {s[2]/s[4]:.2f} |")
        rows.append((p, sa, sb))
    print()
    print("| pitch [cm] | outram-mc mean | OpenMC mean | outram - OpenMC [pcm] | sigma [pcm] | z | gate |")
    print("|---|---|---|---|---|---|---|")
    for p, sa, sb in rows:
        d = (sa[1] - sb[1]) * 1e5
        sg = math.hypot(sa[3], sb[3]) * 1e5
        z = d / sg
        print(f"| {p} | {sa[1]:.5f} +- {sa[3]:.5f} | {sb[1]:.5f} +- {sb[3]:.5f} | {d:+.0f} | {sg:.0f} | "
              f"{z:+.2f} | {'pass' if abs(z) <= 2 else 'MISS'} |")
    print()
    print("Per-seed k (seed: outram-mc / OpenMC):")
    for p in PITCHES:
        print(f"\npitch {p}:")
        for s in sorted(a[p]):
            ob = b[p].get(s)
            print(f"  seed {s:2d}: {a[p][s][0]:.5f} +- {a[p][s][1]:.5f} / "
                  + (f"{ob[0]:.5f} +- {ob[1]:.5f}" if ob else "missing"))


if __name__ == "__main__":
    main()
