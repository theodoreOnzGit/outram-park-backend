"""Distribution of single-step differences (gh:#1390): port vs OpenMC, OpenMC vs itself under a 1-ulp change of the
input poles, and OpenMC DGELSD vs OpenMC DGELSS.
Usage: python analyse_steps.py <step_port.tsv> <step_omc.tsv> <step_omc_gelss.tsv>
"""
import sys, collections
import numpy as np


def rd(p):
    L = open(p).read().splitlines()
    h = L[0].split("\t")
    return [dict(zip(h, l.split("\t"))) for l in L[1:]]


port = {int(r["row"]): (float(r["max_rel"]), float(r["median_rel"]), int(r["iter_from"])) for r in rd(sys.argv[1])}
pert = collections.defaultdict(list)
for r in rd(sys.argv[2]):
    pert[int(r["row"])].append((float(r["pert_max"]), float(r["pert_median"])))
drv = {int(r["row"]): (float(r["driver_max"]), float(r["driver_median"])) for r in rd(sys.argv[3])}
ks = sorted(set(port) & set(pert) & set(drv))
P = np.array([port[k][0] for k in ks]); Pm = np.array([port[k][1] for k in ks])
O = np.array([np.mean([x[0] for x in pert[k]]) for k in ks]); Om = np.array([np.mean([x[1] for x in pert[k]]) for k in ks])
G = np.array([drv[k][0] for k in ks]); Gm = np.array([drv[k][1] for k in ks])
it = np.array([port[k][2] for k in ks])


def q(a):
    return " ".join("%.1e" % np.percentile(a, p) for p in (10, 25, 50, 75, 90))


print(f"{len(ks)} steps: relocation call from the exact poles OpenMC logged at iteration k, next iteration's poles as reference")
print("percentiles 10/25/50/75/90 of max-over-poles relative difference (inf where a pole is lost/NaN):")
print("  port step vs OpenMC (DGELSD)            :", q(P))
print("  OpenMC DGELSS vs OpenMC DGELSD          :", q(G))
print("  OpenMC, input poles perturbed by 1 ulp  :", q(O))
print("percentiles of the median-over-poles difference:")
print("  port:", q(Pm), "| DGELSS:", q(Gm), "| 1-ulp:", q(Om))
for lo, hi in ((1, 3), (4, 8), (9, 15), (16, 29)):
    m = (it >= lo) & (it <= hi)
    print(f"  iterations {lo}-{hi} (n={int(m.sum())}): median max-diff port {np.median(P[m]):.1e}, DGELSS {np.median(G[m]):.1e}, 1-ulp {np.median(O[m]):.1e}")
for thr in (1e-9, 1e-6, 1e-3, 1e-1):
    print(f"  fraction of steps with max diff <= {thr:g}: port {np.mean(P <= thr):.3f}, DGELSS {np.mean(G <= thr):.3f}, 1-ulp {np.mean(O <= thr):.3f}")
