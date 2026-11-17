"""Old-vs-new seed-ensemble analysis for gh:#1583 item 2.

    python3 -I analyse.py <driver> <old.out> <new.out>

<driver> is one of: fz (htr10_fuel_zone_kinf), pitch (lct008_pitch_sweep),
fhr (fhr_pebble_target_at_rest_ablation), tk (thermal_kernel_keff_worth).
Each output file is the driver's stdout+stderr (2>&1). Per arm it prints the
old and new mean, sd, sem (sample sd over seeds, sem = sd/sqrt(n)), and the
new-minus-old difference with sigma = sqrt(sem_old^2 + sem_new^2) and z. Then
the same for each within-build delta of an arm against the first arm (the
quantity a driver is usually run to measure). z is reported against both the
|z| <= 1 and |z| <= 2 criteria used in the prediction.
"""
import collections
import math
import re
import sys


def parse(driver, path):
    d = collections.OrderedDict()
    for line in open(path, errors="replace"):
        if driver == "fz":
            m = re.match(r"SEED (het|hom)\s+(\d+)\s+(\S+)\s+(\S+)", line)
            if m:
                d.setdefault(m.group(1), {})[int(m.group(2))] = float(m.group(3))
        elif driver == "pitch":
            m = re.match(r"SEED (\S+) (\d+) (\S+) (\S+)", line)
            if m:
                d.setdefault("pitch " + m.group(1), {})[int(m.group(2))] = float(m.group(3))
        elif driver == "fhr":
            m = re.match(r"\s+(MOVING|AT-REST)\s+seed\s+(\d+): k = (\S+) ", line)
            if m:
                d.setdefault(m.group(1), {})[int(m.group(2))] = float(m.group(3))
        elif driver == "tk":
            m = re.match(r"\s+replica (\d+):(.*)", line)
            if m:
                rep = int(m.group(1))
                for lab, k in re.findall(r"\s*(\d+x\d+\s+\([^)]*\)) k=(\S+?)\+/-", m.group(2)):
                    d.setdefault(lab, {})[rep] = float(k)
    return d


def stats(xs):
    m = sum(xs) / len(xs)
    v = sum((x - m) ** 2 for x in xs) / (len(xs) - 1)
    return m, math.sqrt(v), math.sqrt(v / len(xs))


def flag(z):
    a = abs(z)
    return f"|z|<=1:{'Y' if a <= 1 else 'N'} |z|<=2:{'Y' if a <= 2 else 'N'}"


def main():
    drv, po, pn = sys.argv[1:4]
    old, new = parse(drv, po), parse(drv, pn)
    labels = list(old.keys())
    assert labels and list(new.keys()) == labels, (list(old), list(new))
    seeds = sorted(set.intersection(*[set(v) for v in list(old.values()) + list(new.values())]))
    n = len(seeds)
    print(f"complete common seeds: {n}  {seeds[0]}..{seeds[-1]}")
    res = {}
    for lab in labels:
        o = [old[lab][s] for s in seeds]
        w = [new[lab][s] for s in seeds]
        res[lab] = (o, w)
        mo, so, eo = stats(o)
        mw, sw, ew = stats(w)
        dd = (mw - mo) * 1e5
        sd = math.hypot(eo, ew) * 1e5
        print(
            f"{lab:<24} old {mo:.5f} sd {so*1e5:5.0f} sem {eo*1e5:4.0f} | "
            f"new {mw:.5f} sd {sw*1e5:5.0f} sem {ew*1e5:4.0f} | "
            f"new-old {dd:+6.0f} +/- {sd:4.0f} pcm z {dd/sd:+.2f} {flag(dd/sd)}"
        )
    base = labels[0]
    for lab in labels[1:]:
        o = [res[lab][0][i] - res[base][0][i] for i in range(n)]
        w = [res[lab][1][i] - res[base][1][i] for i in range(n)]
        mo, so, eo = stats(o)
        mw, sw, ew = stats(w)
        dd = (mw - mo) * 1e5
        sd = math.hypot(eo, ew) * 1e5
        print(
            f"delta [{lab}] - [{base}]: old {mo*1e5:+7.0f} +/- {eo*1e5:4.0f}  "
            f"new {mw*1e5:+7.0f} +/- {ew*1e5:4.0f}  new-old {dd:+6.0f} +/- {sd:4.0f} "
            f"z {dd/sd:+.2f} {flag(dd/sd)}"
        )
    nonzero = sum(1 for lab in labels if True)
    print(f"({nonzero} arms; mean of arm new-old over arms:",
          f"{sum((stats(res[l][1])[0]-stats(res[l][0])[0])*1e5 for l in labels)/len(labels):+.0f} pcm)")


main()
