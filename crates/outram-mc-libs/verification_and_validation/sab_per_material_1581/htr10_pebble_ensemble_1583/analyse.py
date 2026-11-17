import sys, math, collections

def load(p):
    d = collections.OrderedDict()
    for line in open(p):
        if not line.startswith("SEED "):
            continue
        rest = line[5:].rstrip()
        parts = rest.split()
        # label has spaces: last 4 tokens are seed k sigma secs
        seed, k, sig = int(parts[-4]), float(parts[-3]), float(parts[-2])
        label = " ".join(parts[:-4])
        d.setdefault(label, {})[seed] = k
    return d

old = load(sys.argv[1]); new = load(sys.argv[2])
labels = list(old.keys())
seeds = sorted(set.intersection(*[set(v) for v in list(old.values()) + list(new.values())]))
n = len(seeds)
print("complete common seeds:", n, seeds[0], "..", seeds[-1])

def stats(xs):
    m = sum(xs) / len(xs)
    v = sum((x - m) ** 2 for x in xs) / (len(xs) - 1)
    return m, math.sqrt(v), math.sqrt(v / len(xs))

res = {}
for lab in labels:
    o = [old[lab][s] for s in seeds]; w = [new[lab][s] for s in seeds]
    res[lab] = (o, w)
    mo, so, eo = stats(o); mw, sw, ew = stats(w)
    d = (mw - mo) * 1e5; sd = math.hypot(eo, ew) * 1e5
    print(f"{lab:<30} old {mo:.5f} sd {so*1e5:5.0f} sem {eo*1e5:4.0f} | new {mw:.5f} sd {sw*1e5:5.0f} sem {ew*1e5:4.0f} | diff {d:+6.0f} +/- {sd:4.0f} pcm z {d/sd:+.2f}")
# paired (same seed) difference, informational only: URR stream differs so not expected to correlate
for lab in labels:
    o, w = res[lab]
    dd = [(b - a) for a, b in zip(o, w)]
    m, s, e = stats(dd)
    print(f"  paired {lab:<30} mean {m*1e5:+6.0f} sem {e*1e5:4.0f} pcm")
# boron-ablation deltas relative to first arm (natural, as specified)
base = labels[0]
for lab in labels[1:]:
    o = [res[lab][0][i] - res[base][0][i] for i in range(n)]
    w = [res[lab][1][i] - res[base][1][i] for i in range(n)]
    mo, so, eo = stats(o); mw, sw, ew = stats(w)
    d = (mw - mo) * 1e5; sd = math.hypot(eo, ew) * 1e5
    print(f"delta {lab:<30} old {mo*1e5:+6.0f} +/-{eo*1e5:4.0f} new {mw*1e5:+6.0f} +/-{ew*1e5:4.0f} diff {d:+6.0f} +/-{sd:4.0f} z {d/sd:+.2f}")
