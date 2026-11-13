"""Compare two `accident_only.py` outputs (same rows, two upstream versions).

Usage: python3 -I compare_versions.py old.csv new.csv

Rows are `function,args,expected,cond`; `accident_case.scenario:*` rows carry
the scenario constants and are skipped. Prints, per group, how many rows are
identical and the median and maximum relative change.
"""
import collections
import sys

a = open(sys.argv[1]).read().strip().split('\n')
b = open(sys.argv[2]).read().strip().split('\n')
assert len(a) == len(b)
groups = collections.defaultdict(list)
for x, y in zip(a, b):
    fx, fy = x.split(','), y.split(',')
    kind = fx[0].split(':')[0]
    assert fx[0] == fy[0]
    if kind.endswith('scenario'):
        continue
    ex, ey = float(fx[2]), float(fy[2])
    d = 0.0 if ex == ey else abs(ex - ey) / max(abs(ex), abs(ey), 1e-300)
    groups[fx[0]].append(d)
    groups[kind].append(d)
for k in sorted(groups):
    v = sorted(groups[k])
    n = len(v)
    print(f'{k:42} rows {n:4} identical {sum(1 for d in v if d == 0):4} '
          f'median {v[n // 2]:.3e} max {v[-1]:.3e}')
