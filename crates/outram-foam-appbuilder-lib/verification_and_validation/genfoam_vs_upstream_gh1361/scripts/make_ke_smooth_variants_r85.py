#!/usr/bin/env python3
"""gh:#1984 round 85: one-edit variants of round 65's `cases/fftf_turb_variants/w8_zg` (porousKEpsilon, tight) that select `smoothSolver`
(`symGaussSeidel`) for the onePhase `k`/`epsilon` (README "Round 85", Part B; written and committed with the predictions BEFORE any run).
Explicit `k`, `epsilon`, `kFinal`, `epsilonFinal` entries are appended AFTER the catch-all `".*"` (exact keywords win):
  k0: relTol 0                       k1: relTol 0.1
  k2: relTol 0.05; nSweeps 2; maxIter 5 (upstream stops at 6)
  k3: nSweeps -2 (two fixed sweeps, no residual)
  k4: plain entries relTol 0.1; Final entries relTol 0; nSweeps 3 (without residualControl the Final entries are never selected)
usage: make_ke_smooth_variants_r85.py <w8_zg case dir> <out dir>
"""
import os, shutil, sys

base, out = sys.argv[1], sys.argv[2]
V = {
    'k0': ('relTol 0;', None),
    'k1': ('relTol 0.1;', None),
    'k2': ('relTol 0.05; nSweeps 2; maxIter 5;', None),
    'k3': ('relTol 0; nSweeps -2;', None),
    'k4': ('relTol 0.1;', 'relTol 0; nSweeps 3;'),
}


def entry(name, extra):
    body = '        solver          smoothSolver;\n        smoother        symGaussSeidel;\n        tolerance       1e-14;\n'
    body += ''.join('        %s;\n' % k.strip() for k in extra.split(';') if k.strip())
    return '    %s\n    {\n%s    }\n' % (name, body)


for v, (plain, final) in V.items():
    d = os.path.join(out, v)
    shutil.rmtree(d, ignore_errors=True)
    shutil.copytree(base, d)
    p = os.path.join(d, 'system/fluidRegion/fvSolution')
    t = open(p).read()
    i = t.index('".*"')
    close = t.index('}', i)
    nl = t.index('\n', close) + 1
    add = entry('k', plain) + entry('epsilon', plain) + entry('kFinal', final or plain) + entry('epsilonFinal', final or plain)
    open(p, 'w').write(t[:nl] + add + t[nl:])
    print(v)
