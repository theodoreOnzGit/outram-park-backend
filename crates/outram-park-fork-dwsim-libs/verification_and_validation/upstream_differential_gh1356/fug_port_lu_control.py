#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Localisation control for tests/upstream_shortcut_parity.rs: the upstream FUG
# transcription (fug_driver.py) with Mapack's LU replaced by the order of
# operations of this port's `columns::linalg::lu_solve`. It reproduces the port's
# two differing Rmin values bit for bit.  usage: python3 -I fug_port_lu_control.py
import importlib.util, os
here = os.path.dirname(os.path.abspath(__file__))
sp = importlib.util.spec_from_file_location('d', os.path.join(here, 'fug_driver.py'))
d = importlib.util.module_from_spec(sp); sp.loader.exec_module(d)
def port_lu(A, B):
    n = len(B); a = [r[:] for r in A]; x = B[:]
    for k in range(n):
        piv = k; best = abs(a[k][k])
        for i in range(k + 1, n):
            if abs(a[i][k]) > best: best = abs(a[i][k]); piv = i
        if piv != k:
            a[k], a[piv] = a[piv], a[k]; x[k], x[piv] = x[piv], x[k]
        akk = a[k][k]
        for i in range(k + 1, n):
            f = a[i][k] / akk
            if f == 0.0: continue
            a[i][k] = 0.0
            for j in range(k + 1, n): a[i][j] -= f * a[k][j]
            x[i] -= f * x[k]
    for i in range(n - 1, -1, -1):
        s = x[i]
        for j in range(i + 1, n): s -= a[i][j] * x[j]
        x[i] = s / a[i][i]
    return x
d.mapack_solve = port_lu
for line in open(os.path.join(here, 'fug_inputs.txt')):
    head, zs, ks = line.split('|'); h = head.split()
    if 'c3c6' not in h[1]: continue
    z = [d.un(v) for v in zs.split()]; K = [d.un(v) for v in ks.split()]
    q, F, Rr, xdhk, xblk = (d.un(v) for v in h[4:9])
    print(h[1], repr(d.fug(F, z, K, int(h[2]), int(h[3]), q, Rr, xdhk, xblk)['Rmin']))
