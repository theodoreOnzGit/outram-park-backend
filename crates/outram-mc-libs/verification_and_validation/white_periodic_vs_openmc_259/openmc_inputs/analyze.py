"""Ensemble statistics for white_periodic_vs_openmc_259.

usage: analyze.py <openmc_dir> <ours_dir>
Each dir holds <case>_<bc>_s<first>.json files (openmc: ensemble.py; ours:
OUTRAM_BC259_OUT of tests/bc_vs_openmc_ensembles.rs). Blocks with the same
case/bc are pooled. Prints mean +/- sem over seeds (never the single-run
sigma), z = (a-b)/sqrt(sa^2+sb^2), and the boundary-condition difference d."""
import sys, json, glob, math, collections
import numpy as np


def load(d):
    out = collections.defaultdict(list)
    for f in sorted(glob.glob(d + '/*_s*.json')):
        j = json.load(open(f))
        out[(j['case'], j['bc'])] += j['k']
    return out


def ms(k):
    k = np.array(k)
    return k.mean(), k.std(ddof=1) / math.sqrt(len(k)), len(k)


def z(a, b):
    return (a[0] - b[0]) / math.hypot(a[1], b[1])


omc, ours = load(sys.argv[1]), load(sys.argv[2])
print('case  bc          N   openmc               ours                 z')
for key in sorted(omc):
    if key in ours:
        a, b = ms(omc[key]), ms(ours[key])
        print(f'{key[0]:5} {key[1]:10} {a[2]:2}/{b[2]:2} {a[0]:.5f}+/-{a[1]:.5f}  {b[0]:.5f}+/-{b[1]:.5f}  {z(b, a):+.2f}')
print()
print('case  d = k(other) - k(reflective)       openmc                 ours                  z')
for case, other in [('A0.5', 'white'), ('A1.0', 'white'), ('A3.0', 'white'), ('B1', 'periodic'), ('B0', 'periodic')]:
    if (case, other) not in omc or (case, other) not in ours:
        continue
    res = []
    for src in (omc, ours):
        r, o = ms(src[(case, 'reflective')]), ms(src[(case, other)])
        res.append((o[0] - r[0], math.hypot(r[1], o[1])))
    print(f'{case:5} {other:9}  {res[0][0]:+.5f}+/-{res[0][1]:.5f}   {res[1][0]:+.5f}+/-{res[1][1]:.5f}   z {z(res[1], res[0]):+.2f}   (openmc d/sem {res[0][0]/res[0][1]:+.1f}, ours {res[1][0]/res[1][1]:+.1f})')
