#!/usr/bin/env python3
"""Round 78: compare two extract_esfr_transient logs (A = perturbed/new, B = reference): per step power (last pass), dt, passes, h counts,
TM solve count/order per pass, first TM flip.  usage: cmp_logs.py A B  (paths, .txt or .gz)"""
import gzip, sys

def load(p):
    op = gzip.open if p.endswith('.gz') else open
    steps = {}
    for line in op(p, 'rt'):
        w = line.split()
        if not w:
            continue
        if w[0] == 'STEP':
            k = int(w[1]); steps[k] = {'neut': [], 'tm': [], 'h': 0, 'dt': None, 'iter': 0}
        elif w[0] == 'DT':
            steps[int(w[1])]['dt'] = float(w[2])
        elif w[0] == 'ITER':
            steps[int(w[1])]['iter'] = int(w[2])
        elif w[0] == 'H':
            steps[int(w[1])]['h'] = int(w[2])
            steps[int(w[1])]['hres'] = [float(x) for x in w[3::2]]
        elif w[0] == 'NEUT':
            steps[int(w[1])]['neut'].append((float(w[3]), int(w[4])))
        elif w[0] == 'TM':
            n = int(w[3])
            steps[int(w[1])]['tm'].append([w[4 + 3 * i] for i in range(n)])
    return steps

a, b = load(sys.argv[1]), load(sys.argv[2])
n = min(len(a), len(b))
print(f'steps A {len(a)} B {len(b)}')
worst = 0.0; flip = None; iter_mm = []; h_mm = []; neut_mm = []; dt_w = 0.0
rows = {}
mx = 0.0
for k in range(1, n + 1):
    pa, pb = a[k]['neut'][-1][0], b[k]['neut'][-1][0]
    r = abs(pa - pb) / abs(pb)
    mx = max(mx, r)
    rows[k] = (r, mx)
    dt_w = max(dt_w, abs(a[k]['dt'] - b[k]['dt']) / abs(b[k]['dt']))
    if a[k]['iter'] != b[k]['iter']: iter_mm.append(k)
    if a[k]['h'] != b[k]['h']: h_mm.append(k)
    if [x[1] for x in a[k]['neut']] != [x[1] for x in b[k]['neut']]: neut_mm.append(k)
    if flip is None and a[k]['tm'] != b[k]['tm']:
        flip = k
print('power rel diff (step: value, running max):')
for k in (1, 2, 5, 10, 20, 25, 30, 40, 44, 45, 46, 47, 50, 60, 70, 73):
    if k in rows:
        print(f'  step {k}: {rows[k][0]:.3e}  running max {rows[k][1]:.3e}')
first = next((k for k in rows if rows[k][0] > 1e-9), None)
print(f'max power diff {mx:.3e}; first step above 1e-9: {first}')
print(f'max dt rel diff {dt_w:.3e}; pass-count mismatches at steps {iter_mm}; h-count mismatches {h_mm}; neutron-iteration mismatches {neut_mm}')
tmm = [k for k in range(1, n + 1) if a[k]['tm'] != b[k]['tm']]
print(f'TM solve count/order differs at steps {tmm} (first flip {flip})')
# bit identity of power
ident = sum(1 for k in range(1, n + 1) if a[k]['neut'][-1][0] == b[k]['neut'][-1][0])
print(f'power bit-identical in {ident} of {n} steps')
