#!/usr/bin/env python3
"""Round 86 (gh:#1955): multiply EVERY internal value of <case>/<time>/fluidRegion/T by (1 + EPS) (default 1e-12), printed with repr (17 digits).

usage: perturb_esfr_start_rel.py <path to T> [EPS]
The real-vs-real sensitivity seed of README round 86 Part A (A-S); round 78's one-ulp seed (perturb_esfr_start_ulp.py) was absorbed by the
tight run. Prints the number of cells changed and the largest absolute and relative change actually written.
"""
import re, sys
path = sys.argv[1]
eps = float(sys.argv[2]) if len(sys.argv) > 2 else 1e-12
t = open(path).read()
m = re.search(r'internalField\s+nonuniform\s+List<scalar>\s*(\d+)\s*\(\n', t)
n = int(m.group(1))
start = m.end()
end = t.index('\n)', start)
vals = t[start:end].split('\n')
assert len(vals) == n, (len(vals), n)
changed, dmax, rmax = 0, 0.0, 0.0
for i, s in enumerate(vals):
    old = float(s)
    new = old * (1.0 + eps)
    if new != old:
        changed += 1
        dmax = max(dmax, abs(new - old))
        rmax = max(rmax, abs(new - old) / abs(old))
    vals[i] = repr(new)
print(f'{changed} of {n} cells changed; max |delta| {dmax:.3e}, max rel {rmax:.3e}')
open(path, 'w').write(t[:start] + '\n'.join(vals) + t[end:])
