#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Verbatim transcription of the Fenske-Underwood-Gilliland core of DWSIM
# `ShortcutColumn.Calculate` (GPL-3.0, Daniel Wagner O. de Medeiros), pinned
# commit 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766:
#   DWSIM.UnitOperations/UnitOperations/ShortcutColumn.vb  lines 198-415, 567-580
#   DWSIM.Math/BrentMinimize.vb  brentoptimize2, mysign          (176-304)
#   DWSIM.MathOps.Mapack/LuDecomposition.cs  ctor + Solve         (Matrix.Solve, square)
# K-values are SEEDED from the port (fug_inputs.txt): upstream computes them with
# its flash package, which this route cannot run. q is the feed liquid fraction
# (0 < q < 1, so the enthalpy re-computation of q at :168-187 is not entered).
# VB `x ^ y` -> math.pow; Log/Log10 -> libm. usage: fug_driver.py <inputs> <out>
import math, struct, sys

def hx(v): return struct.pack('>d', v).hex()
def un(h): return struct.unpack('>d', bytes.fromhex(h))[0]
def mysign(a, b): return abs(a) if b > 0 else -abs(a)

def brentoptimize2(a, b, epsilon, func):
    cgold = 0.381966
    bx = 0.5 * (a + b)
    ia = a if a < b else b
    ib = a if a > b else b
    v = bx; w = v; x = v
    e = 0.0; d = 0.0
    fx = func(x); fv = fx; fw = fx
    for it in range(1, 101):
        xm = 0.5 * (ia + ib)
        if abs(x - xm) <= epsilon * 2 - 0.5 * (ib - ia):
            break
        if abs(e) > epsilon:
            r = (x - w) * (fx - fv)
            q = (x - v) * (fx - fw)
            p = (x - v) * q - (x - w) * r
            q = 2 * (q - r)
            if q > 0: p = -p
            q = abs(q)
            etemp = e
            e = d
            if not (abs(p) >= abs(0.5 * q * etemp) or p <= q * (ia - x) or p >= q * (ib - x)):
                d = p / q
                u = x + d
                if u - ia < epsilon * 2 or ib - u < epsilon * 2:
                    d = mysign(epsilon, xm - x)
            else:
                e = ia - x if x >= xm else ib - x
                d = cgold * e
        else:
            e = ia - x if x >= xm else ib - x
            d = cgold * e
        u = x + d if abs(d) >= epsilon else x + mysign(epsilon, d)
        fu = func(u)
        if fu != fu: break
        if fu <= fx:
            if u >= x: ia = x
            else: ib = x
            v = w; fv = fw; w = x; fw = fx; x = u; fx = fu
        else:
            if u < x: ia = u
            else: ib = u
            if fu <= fw or w == x:
                v = w; fv = fw; w = u; fw = fu
            else:
                if fu <= fv or v == x or v == 2:
                    v = u; fv = fu
    return x

def rminfunc(x, alpha, z, q, n):
    value = 0.0
    for j in range(n + 1):
        if z[j] != 0.0:
            value += alpha[j] * z[j] / (alpha[j] - x)
    return math.pow(value - 1 + q, 2)

def mapack_solve(A, B):
    """Mapack LuDecomposition(A).Solve(B) for a single right-hand side."""
    rows = len(A); columns = rows
    lu = [row[:] for row in A]
    piv = list(range(rows)); sign = 1
    lucolj = [0.0] * rows
    for j in range(columns):
        for i in range(rows): lucolj[i] = lu[i][j]
        for i in range(rows):
            luri = lu[i]
            kmax = min(i, j); s = 0.0
            for k in range(kmax): s += luri[k] * lucolj[k]
            lucolj[i] -= s
            luri[j] = lucolj[i]
        p = j
        for i in range(j + 1, rows):
            if abs(lucolj[i]) > abs(lucolj[p]): p = i
        if p != j:
            for k in range(columns):
                lu[p][k], lu[j][k] = lu[j][k], lu[p][k]
            piv[p], piv[j] = piv[j], piv[p]; sign = -sign
        if j < rows and lu[j][j] != 0.0:
            for i in range(j + 1, rows): lu[i][j] /= lu[j][j]
    X = [B[piv[i]] for i in range(rows)]
    for k in range(columns):
        for i in range(k + 1, columns):
            X[i] -= X[k] * lu[i][k]
    for k in range(columns - 1, -1, -1):
        X[k] /= lu[k][k]
        for i in range(k):
            X[i] -= X[k] * lu[i][k]
    return X

def fug(F, z, K, lki, hki, q, Rr, xd_hk_spec, xb_lk_spec):
    n = len(z) - 1
    alpha = [0.0] * (n + 1); xb = [0.0] * (n + 1); xd = [0.0] * (n + 1)
    lnk = []; dnk = []; hnk = []
    i = 0
    while True:
        alpha[i] = K[i] / K[hki]
        if K[i] > K[lki]: lnk.append(i)
        elif K[i] < K[lki] and K[i] > K[hki]: dnk.append(i)
        elif K[i] < K[hki]: hnk.append(i)
        i += 1
        if i == n + 1: break
    D = F * z[lki]
    if len(lnk) > 0:
        i = 0
        while True:
            D += F * z[lnk[i]]
            i += 1
            if i >= len(lnk): break
    while True:  # restart:
        B = F - D
        xd[hki] = xd_hk_spec
        xb[lki] = xb_lk_spec
        xb[hki] = (F * z[hki] - D * xd[hki]) / (F - D)
        xd[lki] = (F * z[lki] - (F - D) * xb[lki]) / D
        S = (xd[lki] / xd[hki]) * (xb[hki] / xb[lki])
        Nmin = math.log(S) / math.log(alpha[lki] / alpha[hki])
        C = (math.log10(alpha[lki]) * math.log10(xd[hki] / xb[hki]) - math.log10(alpha[hki]) * math.log10(xd[lki] / xb[lki])) / (math.log10(alpha[lki]) - math.log10(alpha[hki]))
        cte = [0.0] * (n + 1)
        for i in range(n + 1):
            if i != lki and i != hki:
                cte[i] = math.pow(10, Nmin * math.log10(alpha[i]) + C)
                xb[i] = F * z[i] / (B + D * cte[i])
                xd[i] = xb[i] * cte[i]
        Dant = D
        D = 0.0
        for i in range(n + 1):
            if z[i] != 0: D += Dant * xd[i]
        if D != D or D == 0.0: raise ValueError("Invalid value for Distillate Rate")
        if not abs((D - Dant) / D) < 0.0001: continue
        break
    L = Rr * D
    L_ = L + q * F
    V_ = L_ - B
    V = D + L
    if L_ < 0.0 or V_ < 0: raise ValueError("Invalid Reflux Ratio")
    mode2 = False; count = 0; indexes = []
    Dr = [0.0] * (n + 1)
    for i in range(n + 1):
        Dr[i] = (alpha[i] - 1) / (alpha[lki] - 1) * D * xd[lki] / (F * z[lki]) + (alpha[lki] - alpha[i]) / (alpha[lki] - 1) * D * xd[hki] / (F * z[hki])
        if Dr[i] > 0 and Dr[i] < 1 and z[i] != 0 and i != lki and i != hki:
            mode2 = True; count += 1; indexes.append(i)
    rf = lambda x: rminfunc(x, alpha, z, q, n)
    if not mode2:
        teta = brentoptimize2(alpha[hki] * 1.01, alpha[lki], 0.0000001, rf)
        s = 0.0
        for i in range(n + 1):
            if z[i] != 0: s += alpha[i] * xd[i] / (alpha[i] - teta)
        Rmin = s - 1
    else:
        teta = [0.0] * (count + 1)
        for i in range(count + 1):
            if i == 0:
                if alpha[lki] < alpha[indexes[i]]:
                    teta[i] = brentoptimize2(alpha[lki] * 1.01, alpha[indexes[i]] * 0.99, 0.0000001, rf)
                else:
                    teta[i] = brentoptimize2(alpha[indexes[i]] * 1.01, alpha[lki] * 0.99, 0.0000001, rf)
            elif i == count:
                if alpha[indexes[i - 1]] < alpha[hki]:
                    teta[i] = brentoptimize2(alpha[indexes[i - 1]] * 1.01, alpha[hki] * 0.99, 0.0000001, rf)
                else:
                    teta[i] = brentoptimize2(alpha[hki] * 1.01, alpha[indexes[i - 1]] * 0.99, 0.0000001, rf)
            else:
                teta[i] = brentoptimize2(alpha[indexes[i - 1]] * 1.01, alpha[indexes[i]] * 0.99, 0.0000001, rf)
        MA = [[0.0] * (count + 1) for _ in range(count + 1)]
        MB = [0.0] * (count + 1)
        for i in range(count + 1):
            MB[i] = -1.0
            MA[i][0] = 1.0
            j2 = 0
            for j in range(len(alpha)):
                if j not in indexes:
                    MB[i] += alpha[j] * xd[j] / (alpha[j] - teta[i])
                else:
                    MA[i][j2 + 1] = -alpha[j] / (alpha[j] - teta[i])
                    j2 += 1
        Rmin = mapack_solve(MA, MB)[0]
    if Rmin > Rr: raise ValueError("Defined Reflux Ratio lower than calculated minimum")
    xx = (Rr - Rmin) / (Rr + 1)
    yy = 0.75 * (1 - math.pow(xx, 0.5668))
    N = (yy + Nmin) / (1 - yy)
    Ss = z[lki] / z[hki] * xb[hki] / xb[lki]
    NminS = math.log(Ss) / math.log(alpha[lki])
    Ns = NminS * N / Nmin
    return dict(D=D, B=B, Nmin=Nmin, Rmin=Rmin, N=N, ofs=Ns, mode='distributed' if mode2 else 'single', xd=xd, xb=xb)

def main():
    out = open(sys.argv[2], 'w')
    for line in open(sys.argv[1]):
        if not line.startswith('CASE '): continue
        head, zs, ks = line.split('|')
        h = head.split()
        name, lk, hk = h[1], int(h[2]), int(h[3])
        q, F, Rr, xdhk, xblk = (un(v) for v in h[4:9])
        z = [un(v) for v in zs.split()]; K = [un(v) for v in ks.split()]
        try:
            r = fug(F, z, K, lk, hk, q, Rr, xdhk, xblk)
        except ValueError as ex:
            out.write(f"RESULT {name} error {str(ex).replace(' ', '_')}\n"); continue
        parts = [f"mode {r['mode']}"] + [f"{k} {hx(r[k])}" for k in ('D', 'B', 'Nmin', 'Rmin', 'N', 'ofs')]
        for i in range(len(z)):
            parts.append(f"xd{i} {hx(r['xd'][i])}"); parts.append(f"xb{i} {hx(r['xb'][i])}")
        out.write(f"RESULT {name} " + ' '.join(parts) + "\n")
    out.close()

if __name__ == '__main__':
    main()
