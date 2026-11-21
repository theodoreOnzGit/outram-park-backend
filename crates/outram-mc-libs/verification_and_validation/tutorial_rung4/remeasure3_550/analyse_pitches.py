"""gh:#550 third re-measure, item 1: LCT-008 pin cell, default route (tape data,
node-grid S(a,b)), 1.63576 and 1.75 cm, 24 seeds, against OpenMC.
Run with `python3 -I analyse_pitches.py` from this directory.
Seeds 1-8: ../flip_550/pitch_flip.csv (the recorded flip run); seeds 9-24:
def_s9.csv, def_s17.csv (this pass).  OpenMC: ../ensemble_550/openmc_seeds.csv
(k_gen_mean), plus openmc_extra.csv here if present.
sem = sd/sqrt(n); sigma_diff = hypot(sem_a, sem_b); z = diff/sigma_diff."""
import csv
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
R4 = HERE.parent


def load(path, col, pitch):
    out = {}
    p = pathlib.Path(path)
    if not p.exists():
        return out
    for r in csv.DictReader(open(p)):
        if abs(float(r["pitch_cm"]) - pitch) < 1e-9:
            out[int(r["seed"])] = float(r[col])
    return out


def stat(v):
    ks = list(v.values())
    n = len(ks)
    m = sum(ks) / n
    sd = math.sqrt(sum((k - m) ** 2 for k in ks) / (n - 1))
    return n, m, sd, sd / math.sqrt(n)


def diff(a, b):
    sa, sb = stat(a), stat(b)
    d = sa[1] - sb[1]
    s = math.hypot(sa[3], sb[3])
    return 1e5 * d, 1e5 * s, d / s


# predicted difference, range low, range high (pcm), sab_reference_550_remeasure3_plan.md
PRED = {1.63576: (45, -30, 120), 1.75: (55, -20, 130)}
for pitch in (1.63576, 1.75):
    b1 = load(R4 / "flip_550/pitch_flip.csv", "k_inf", pitch)
    new = load(HERE / "def_s9.csv", "k_inf", pitch)
    new.update(load(HERE / "def_s17.csv", "k_inf", pitch))
    allk = dict(b1)
    allk.update(new)
    omc = load(R4 / "ensemble_550/openmc_seeds.csv", "k_gen_mean", pitch)
    extra = load(HERE / "openmc_extra.csv", "k_gen_mean", pitch)
    omc.update({1000 + s: k for s, k in extra.items()})
    print(f"== pitch {pitch} cm ==")
    for name, d in (("seeds 1-8 (recorded)", b1), ("seeds 9-24 (new)", new), ("seeds 1-24", allk)):
        if len(d) > 1:
            n, m, sd, sem = stat(d)
            print(f"  outram {name:22s} n={n:2d} k={m:.5f} +- {sem:.5f} sd {1e5*sd:.0f} pcm")
    n, m, sd, sem = stat(omc)
    print(f"  OpenMC n={n} k={m:.5f} +- {sem:.5f} sd {1e5*sd:.0f} pcm")
    for name, d in (("1-8", b1), ("9-24", new), ("1-24", allk)):
        if len(d) > 1:
            x, s, z = diff(d, omc)
            print(f"  outram {name} minus OpenMC: {x:+.0f} +- {s:.0f} pcm (z {z:+.2f})")
    if len(new) > 1:
        x, s, z = diff(new, b1)
        print(f"  seeds 9-24 minus seeds 1-8: {x:+.0f} +- {s:.0f} pcm (z {z:+.2f})")
    if len(allk) > 8:
        p, lo, hi = PRED[pitch]
        x, s, z = diff(allk, omc)
        print(f"  predicted {p:+d} ({lo:+d} .. {hi:+d}); measured {x:+.0f}; in range: {lo <= x <= hi}; gate |z|<=2: {abs(z) <= 2}")
