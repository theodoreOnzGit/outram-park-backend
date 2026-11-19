"""gh:#550 gap chase: 2.40 cm k of the default route, arm E and the per-nuclide
swaps, against OpenMC.  Run with python -I.  Unpaired (Welch) and paired (same
seed) statistics.  sem = sd/sqrt(n); sigma_diff = sqrt(sem_a^2 + sem_b^2)."""
import csv
import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
R4 = HERE.parent


def load(path, kcol="k_inf", pitch=2.4):
    out = {}
    with open(path) as f:
        for r in csv.DictReader(f):
            if abs(float(r["pitch_cm"]) - pitch) < 1e-9:
                out[int(r["seed"])] = float(r[kcol])
    return out


def merge(*ds):
    o = {}
    for d in ds:
        o.update(d)
    return o


def stat(v):
    ks = list(v.values()) if isinstance(v, dict) else v
    n = len(ks)
    m = sum(ks) / n
    sd = math.sqrt(sum((k - m) ** 2 for k in ks) / (n - 1)) if n > 1 else float("nan")
    return n, m, sd, sd / math.sqrt(n)


def unpaired(a, b):
    sa, sb = stat(a), stat(b)
    d = sa[1] - sb[1]
    s = math.hypot(sa[3], sb[3])
    return 1e5 * d, 1e5 * s, d / s


def paired(a, b):
    seeds = sorted(set(a) & set(b))
    d = [a[s] - b[s] for s in seeds]
    n = len(d)
    m = sum(d) / n
    sd = math.sqrt(sum((x - m) ** 2 for x in d) / (n - 1))
    sem = sd / math.sqrt(n)
    return n, 1e5 * m, 1e5 * sem, m / sem


def row(label, d):
    n, m, sd, sem = stat(d)
    print(f"{label:46s} n={n:2d} k={m:.5f} +- {sem:.5f} (sd {1e5*sd:5.0f} pcm)")


omc = load(R4 / "ensemble_550" / "openmc_seeds.csv", "k_gen_mean")
default = merge(load(R4 / "flip_550" / "pitch_flip.csv"), load(HERE / "def_s9.csv"))
armE = merge(
    load(R4 / "sab_reference_550" / "arms" / "E_h5_nodegrid.csv"), load(HERE / "E_s9.csv")
)
arms = {"default (tape data, node S(a,b))": default, "E (all .h5 data, node S(a,b))": armE}
for tag, fn in [
    ("swap H1,O16 -> .h5", "swapLight.csv"),
    ("swap U234,U235,U238,Mn55 -> .h5", "swapHeavy.csv"),
]:
    if (HERE / fn).is_file():
        arms[tag] = load(HERE / fn)
for k, v in arms.items():
    row(k, v)
row("OpenMC 16 seeds (k_gen_mean, ensemble_550)", omc)
OMC32 = (0.70652, 0.00020)  # 32 seeds, quoted in sab_reference_550.md
print()
for k, v in arms.items():
    n, m, sd, sem = stat(v)
    d = 1e5 * (m - OMC32[0])
    s = 1e5 * math.hypot(sem, OMC32[1])
    print(f"{k:46s} minus OpenMC(32): {d:+6.0f} +- {s:3.0f} pcm  z {d/s:+.2f}")
print()
E = armE
D = default
print("E - default  unpaired: %+.0f +- %.0f pcm (z %+.2f)" % unpaired(E, D))
print("E - default  paired  : n=%d %+.0f +- %.0f pcm (t %+.2f)" % paired(E, D))
for k, v in arms.items():
    if k.startswith("swap"):
        print(f"{k} - default unpaired: %+.0f +- %.0f pcm (z %+.2f)" % unpaired(v, D))
        print(f"{k} - default paired  : n=%d %+.0f +- %.0f pcm (t %+.2f)" % paired(v, D))
        print(f"{k} - E       unpaired: %+.0f +- %.0f pcm (z %+.2f)" % unpaired(v, E))
for lo, hi in [(1, 8), (9, 24)]:
    a = {s: x for s, x in E.items() if lo <= s <= hi}
    b = {s: x for s, x in D.items() if lo <= s <= hi}
    if a and b:
        print(f"seeds {lo}-{hi}: E - default unpaired %+.0f +- %.0f pcm (z %+.2f)" % unpaired(a, b))
# the pooled estimate: E and default carry the same neutron data (see record)
pool = merge({("d", s): x for s, x in D.items()}, {("e", s): x for s, x in E.items()})
n, m, sd, sem = stat(pool)
d = 1e5 * (m - OMC32[0])
s = 1e5 * math.hypot(sem, OMC32[1])
print(f"pooled default+E (n={n}) k={m:.5f} +- {sem:.5f}: minus OpenMC(32) {d:+.0f} +- {s:.0f} pcm (z {d/s:+.2f})")
