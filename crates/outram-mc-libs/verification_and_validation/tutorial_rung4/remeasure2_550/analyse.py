#!/usr/bin/env python3
"""gh:#550 second re-measure: compare two per-seed k lists.

usage: python3 -I analyse.py NEW.txt OLD.txt
       python3 -I analyse.py NEW.txt - OLD_MEAN OLD_SD_PCM OLD_N
Each file: one k per line, or "seed k" (the second field is taken).
Prints mean, sd, sem of each, new - old in pcm with
sigma = sqrt(sem_new^2 + sem_old^2) (unpaired) and, when the lists are equally
long, the paired difference with its own sem. With OLD_MEAN OLD_SD_PCM OLD_N
instead of an OLD file the comparison is with a record.
"""
import sys, math


def read(p):
    v = []
    for line in open(p):
        f = line.split()
        if f:
            v.append(float(f[-1]) if len(f) == 1 else float(f[1]))
    return v


def ms(v):
    n = len(v)
    m = sum(v) / n
    sd = math.sqrt(sum((x - m) ** 2 for x in v) / (n - 1)) if n > 1 else float("nan")
    return n, m, sd, sd / math.sqrt(n)


a = read(sys.argv[1])
n1, m1, s1, e1 = ms(a)
print(f"NEW  n={n1} mean={m1:.5f} sd={s1*1e5:.0f} pcm sem={e1*1e5:.0f} pcm")
p = None
if len(sys.argv) >= 6:
    m2, s2, n2 = float(sys.argv[3]), float(sys.argv[4]) / 1e5, int(sys.argv[5])
    e2 = s2 / math.sqrt(n2)
else:
    p = read(sys.argv[2])
    n2, m2, s2, e2 = ms(p)
print(f"OLD  n={n2} mean={m2:.5f} sd={s2*1e5:.0f} pcm sem={e2*1e5:.0f} pcm")
d = (m1 - m2) * 1e5
sg = math.sqrt(e1 ** 2 + e2 ** 2) * 1e5
print(f"new - old = {d:+.0f} +- {sg:.0f} pcm  z = {d/sg:+.2f}  (unpaired)")
if p is not None and len(p) == len(a) and len(a) > 1:
    dd = [(x - y) * 1e5 for x, y in zip(a, p)]
    n, m, sd, se = ms(dd)
    print(f"paired by seed: {m:+.0f} +- {se:.0f} pcm  z = {m/se:+.2f}")
