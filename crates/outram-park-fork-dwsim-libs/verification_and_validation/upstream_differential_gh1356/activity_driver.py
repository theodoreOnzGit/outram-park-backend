#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Transcription of DWSIM NRTL.GAMMA_MR and UNIQUAC.GAMMA_MR (GPL-3.0, Daniel
# Wagner O. de Medeiros), pinned commit 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766:
#   DWSIM.Thermodynamics/PropertyPackages/Models/NRTL.vb      lines 221-413
#   DWSIM.Thermodynamics/PropertyPackages/Models/UNIQUAC.vb   lines 245-448
#   DWSIM.ExtensionMethods/SIMDExtenders.vb  (ExpY, LogY, SumY, ... element-wise)
# The Inspector (IObj) logging lines are dropped; every arithmetic statement
# keeps upstream's operation order. VB `x ^ 2` -> math.pow (Math.Pow, libm).
# `Enumerable.Sum(double[])` is a sequential left fold from 0.0.
#
# Output: activity_upstream_output.txt  (hex bit patterns of every double)
#         activity_localisation.txt     (first divergent op, upstream vs port order)
# usage: activity_driver.py <outdir>
import math, struct, sys

def bits(v): return struct.pack('>d', v).hex()

# ---- SIMDExtenders.vb element-wise helpers (verbatim semantics) -------------
def ExpY(v): return [math.exp(a) for a in v]
def LogY(v): return [math.log(a) for a in v]
def SumY(v):
    s = 0.0
    for a in v: s += a
    return s
def NegateY(v): return [-a for a in v]
def MultiplyY(a, b): return [x * y for x, y in zip(a, b)]
def DivideY(a, b): return [x / y for x, y in zip(a, b)]
def SubtractY(a, b): return [x - y for x, y in zip(a, b)]
def AddY(a, b): return [x + y for x, y in zip(a, b)]
def MultiplyConstY(v, c): return [a * c for a in v]
def AddConstY(v, c): return [a + c for a in v]

# ---- interaction-parameter dictionary lookup, shared shape ------------------
# IP[id_i][id_j] = dict(A12,A21,B12,B21,C12,C21,alpha12)
def nrtl_gamma(T, Vx, Vids, IP, trace):
    n = len(Vx) - 1
    Gij = [None]*(n+1); Gji = [None]*(n+1)
    tau_ij = [[0.0]*(n+1) for _ in range(n+1)]
    tau_ji = [[0.0]*(n+1) for _ in range(n+1)]
    alpha12 = [[0.0]*(n+1) for _ in range(n+1)]
    S = [0.0]*(n+1); C = [0.0]*(n+1)
    R = 1.98721
    def P(a, b): return IP[Vids[a]][Vids[b]]
    def has(a, b): return Vids[a] in IP and Vids[b] in IP[Vids[a]]
    def tauf(p, A, B, Cc): return (p[A] + p[B] * T + p[Cc] * math.pow(T, 2)) / (R * T)
    for i in range(n+1):
        for j in range(n+1):
            # NRTL.vb:300-345 conditional chain, verbatim control flow
            if Vids[i] in IP:
                if has(i, j):
                    p = P(i, j)
                    tau_ij[i][j] = tauf(p, 'A12', 'B12', 'C12')
                    tau_ji[i][j] = tauf(p, 'A21', 'B21', 'C21')
                    alpha12[i][j] = p['alpha12']
                else:
                    if Vids[j] in IP:
                        if has(j, i):
                            p = P(j, i)
                            tau_ji[i][j] = tauf(p, 'A12', 'B12', 'C12')
                            tau_ij[i][j] = tauf(p, 'A21', 'B21', 'C21')
                            alpha12[i][j] = p['alpha12']
            elif Vids[j] in IP:
                if has(j, i):
                    p = P(j, i)
                    tau_ji[i][j] = tauf(p, 'A12', 'B12', 'C12')
                    tau_ij[i][j] = tauf(p, 'A21', 'B21', 'C21')
                    alpha12[i][j] = p['alpha12']
    for i in range(n+1):
        Gij[i] = ExpY(MultiplyY(NegateY(alpha12[i]), tau_ij[i]))
        Gji[i] = ExpY(MultiplyY(NegateY(alpha12[i]), tau_ji[i]))
    for i in range(n+1):
        S[i] = SumY(MultiplyY(Vx, Gji[i]))
        C[i] = SumY(MultiplyY(MultiplyY(Vx, Gji[i]), tau_ji[i]))
    lnVg = DivideY(C, S)
    for i in range(n+1):
        lnVg[i] += SumY(DivideY(MultiplyY(MultiplyY(Vx, Gij[i]), SubtractY(tau_ij[i], DivideY(C, S))), S))
    trace.update(tau_ij=tau_ij, tau_ji=tau_ji, Gij=Gij, Gji=Gji, S=S, C=C, lnVg=list(lnVg))
    return ExpY(lnVg)

def uniquac_gamma(T, Vx, Vids, VQ, VR, IP, trace):
    n = len(Vx) - 1
    z = 10.0
    z6 = lambda: [[0.0]*(n+1) for _ in range(n+1)]
    a12, a21, b12, b21, c12, c21 = z6(), z6(), z6(), z6(), z6(), z6()
    def has(a, b): return Vids[a] in IP and Vids[b] in IP[Vids[a]]
    for i in range(n+1):
        for j in range(n+1):
            # UNIQUAC.vb:352-388
            if Vids[i] in IP:
                if has(i, j):
                    p = IP[Vids[i]][Vids[j]]
                    a12[i][j], a21[i][j] = p['A12'], p['A21']
                    b12[i][j], b21[i][j] = p['B12'], p['B21']
                    c12[i][j], c21[i][j] = p['C12'], p['C21']
                else:
                    if Vids[j] in IP:
                        if has(j, i):
                            p = IP[Vids[j]][Vids[i]]
                            a12[i][j], a21[i][j] = p['A21'], p['A12']
                            b12[i][j], b21[i][j] = p['B21'], p['B12']
                            c12[i][j], c21[i][j] = p['C21'], p['C12']
            elif Vids[j] in IP:
                if has(j, i):
                    p = IP[Vids[j]][Vids[i]]
                    a12[i][j], a21[i][j] = p['A21'], p['A12']
                    b12[i][j], b21[i][j] = p['B21'], p['B12']
                    c12[i][j], c21[i][j] = p['C21'], p['C12']
    k = 1 / (1.98721 * T)
    T2 = math.pow(T, 2)
    tau_ij = [ExpY(MultiplyConstY(AddY(NegateY(a12[i]), AddY(MultiplyConstY(b12[i], T), MultiplyConstY(c12[i], T2))), k)) for i in range(n+1)]
    tau_ji = [ExpY(MultiplyConstY(AddY(NegateY(a21[i]), AddY(MultiplyConstY(b21[i], T), MultiplyConstY(c21[i], T2))), k)) for i in range(n+1)]
    r = SumY(MultiplyY(Vx, VR))
    q = SumY(MultiplyY(Vx, VQ))
    fi = MultiplyConstY(MultiplyY(Vx, VR), 1 / r)
    teta = MultiplyConstY(MultiplyY(Vx, VQ), 1 / q)
    S = [SumY(MultiplyY(teta, tau_ji[i])) for i in range(n+1)]
    sum1 = [SumY(MultiplyY(teta, DivideY(tau_ij[i], S))) for i in range(n+1)]
    lngc = AddY(AddConstY(MultiplyConstY(VR, -1 / r), 1),
                AddY(LogY(MultiplyConstY(VR, 1 / r)),
                     MultiplyY(MultiplyConstY(VQ, -z / 2),
                               AddY(AddConstY(NegateY(DivideY(fi, teta)), 1), LogY(DivideY(fi, teta))))))
    lngr = MultiplyY(VQ, AddConstY(SubtractY(NegateY(LogY(S)), sum1), 1))
    lng = AddY(lngc, lngr)
    trace.update(tau_ij=tau_ij, tau_ji=tau_ji, r=[r], q=[q], fi=fi, teta=teta, S=S, sum1=sum1, lngc=lngc, lngr=lngr, lnVg=list(lng))
    return ExpY(lng)

# ---- the order of operations of THIS PORT, as read in src/thermo/activity.rs
# (used only to localise the first divergent operation; the port itself is the
# thing under test in tests/upstream_activity_parity.rs) ----------------------
def port_nrtl(T, x, a, b, c, al, trace):
    n = len(x); R = 1.98721
    tau = [[(a[i][j] + b[i][j]*T + c[i][j]*T*T) / (R*T) for j in range(n)] for i in range(n)]
    g = [[math.exp(-al[i][j]*tau[i][j]) for j in range(n)] for i in range(n)]
    s = []; cc = []
    for i in range(n):
        si = 0.0; ci = 0.0
        for k in range(n):
            si += x[k]*g[k][i]; ci += x[k]*g[k][i]*tau[k][i]
        s.append(si); cc.append(ci)
    ln = []
    for i in range(n):
        l = cc[i]/s[i]
        for j in range(n):
            l += x[j]*g[i][j]*(tau[i][j]-cc[j]/s[j])/s[j]
        ln.append(l)
    trace.update(tau=tau, G=g, S=s, C=cc, lnVg=ln)
    return [math.exp(v) for v in ln]

def sci(v): return f"{v:.17e}"

# ---- systems ----------------------------------------------------------------
def pair(a12, a21, b12=0.0, b21=0.0, c12=0.0, c21=0.0, al=0.3):
    return dict(A12=a12, A21=a21, B12=b12, B21=b21, C12=c12, C21=c21, alpha12=al)

# pairs given as (id_p, id_q, params): stored ONLY as IP[id_p][id_q] (the
# fall-back orientation branch is exercised by listing p > q for some pairs)
NRTL_SYS = {
  'n2':  (['W','E'], [('W','E', pair(-120.5, 950.25))]),
  'n2t': (['W','E'], [('W','E', pair(-120.5, 950.25, 0.35, -0.82, 1.1e-4, -2.3e-4, 0.3))]),
  'n3':  (['A','B','C'], [('A','B', pair(300.5,-150.25, al=0.3)), ('C','A', pair(1200.0,80.5, al=0.2)), ('B','C', pair(-75.0,640.75, al=0.47))]),
  'n3t': (['A','B','C'], [('A','B', pair(300.5,-150.25,0.12,-0.2,3.0e-5,1.0e-4, 0.3)), ('C','A', pair(1200.0,80.5,-0.9,0.4,-2e-4,5e-5, 0.2)), ('B','C', pair(-75.0,640.75,0.05,0.7,1e-4,-1e-4, 0.47))]),
  'n4':  (['A','B','C','D'], [('A','B', pair(300.5,-150.25)), ('A','C', pair(1200.0,80.5,al=0.2)), ('D','A', pair(15.5,22.25,al=0.35)), ('B','C', pair(-75.0,640.75,al=0.47)), ('B','D', pair(410.0,-33.0,0.3,0.1,0.0,0.0,0.25)), ('D','C', pair(-210.0,512.0,al=0.4))]),
}
UNIQ_SYS = {
  'u2':  (['W','E'], [4.0, 2.1], [3.0, 1.97], [('W','E', pair(-95.5, 410.25))]),
  'u2t': (['W','E'], [4.0, 2.1], [3.0, 1.97], [('W','E', pair(-95.5, 410.25, 0.31, -0.42, 1.2e-4, -3.1e-4))]),
  'u3':  (['A','B','C'], [3.19,4.06,1.43], [2.97,3.24,1.4], [('A','B', pair(300.5,-150.25)), ('C','A', pair(-90.0,80.5)), ('B','C', pair(-75.0,640.75))]),
  'u3t': (['A','B','C'], [3.19,4.06,1.43], [2.97,3.24,1.4], [('A','B', pair(300.5,-150.25,0.12,-0.2,3.0e-5,1.0e-4)), ('C','A', pair(-90.0,80.5,-0.9,0.4,-2e-4,5e-5)), ('B','C', pair(-75.0,640.75,0.05,0.7,1e-4,-1e-4))]),
  'u4':  (['A','B','C','D'], [3.19,4.06,1.43,2.5], [2.97,3.24,1.4,2.2], [('A','B', pair(300.5,-150.25)), ('A','C', pair(-90.0,80.5)), ('D','A', pair(15.5,22.25)), ('B','C', pair(-75.0,640.75)), ('B','D', pair(410.0,-33.0)), ('D','C', pair(-210.0,512.0))]),
}
TEMPS = [298.15, 350.0, 410.37]

def compositions(n):
    s = 12345 + n
    pts = []
    for k in range(20):
        u = []
        for _ in range(n):
            s = (s * 1103515245 + 12345) % 2147483648
            u.append(0.02 + (s / 2147483648.0))
        tot = 0.0
        for a in u: tot += a
        pts.append([a / tot for a in u])
    edge = [0.97] + [0.03 / (n - 1)] * (n - 1)
    pts.append(edge)
    pts.append(list(reversed(edge)))
    pts.append([1.0 / n] * n)
    return pts

def build_ip(pairs):
    IP = {}
    for p, q, par in pairs:
        IP.setdefault(p, {})[q] = par
    return IP

def matrices(ids, pairs):
    n = len(ids); idx = {k: i for i, k in enumerate(ids)}
    def z(): return [[0.0]*n for _ in range(n)]
    a, b, c, al = z(), z(), z(), z()
    for p, q, par in pairs:
        i, j = idx[p], idx[q]
        a[i][j], a[j][i] = par['A12'], par['A21']
        b[i][j], b[j][i] = par['B12'], par['B21']
        c[i][j], c[j][i] = par['C12'], par['C21']
        al[i][j] = al[j][i] = par['alpha12']
    return a, b, c, al

def main():
    out = sys.argv[1]
    up = open(f"{out}/activity_upstream_output.txt", 'w')
    loc = open(f"{out}/activity_localisation.txt", 'w')
    up.write("# kind sys T_bits x_bits... | gamma_bits... | lnGamma_bits...\n")
    firstdiv = {}
    for name, (ids, pairs) in NRTL_SYS.items():
        IP = build_ip(pairs); a, b, c, al = matrices(ids, pairs)
        for T in TEMPS:
            for x in compositions(len(ids)):
                tr = {}; g = nrtl_gamma(T, x, ids, IP, tr)
                up.write(f"NRTL {name} {bits(T)} {' '.join(bits(v) for v in x)} | {' '.join(bits(v) for v in g)} | {' '.join(bits(v) for v in tr['lnVg'])}\n")
                pt = {}; gp = port_nrtl(T, x, a, b, c, al, pt)
                # first divergent operation, in evaluation order
                order = [('tau', [pt['tau'][i][j] for i in range(len(x)) for j in range(len(x))], [tr['tau_ij'][i][j] for i in range(len(x)) for j in range(len(x))]),
                         ('S', pt['S'], tr['S']), ('C', pt['C'], tr['C']), ('lnGamma', pt['lnVg'], tr['lnVg']), ('gamma', gp, g)]
                key = 'identical'
                for nm, pv, uv in order:
                    if any(bits(p) != bits(u) for p, u in zip(pv, uv)):
                        key = nm; break
                firstdiv[('NRTL', name, key)] = firstdiv.get(('NRTL', name, key), 0) + 1
    for name, (ids, VR, VQ, pairs) in UNIQ_SYS.items():
        IP = build_ip(pairs)
        for T in TEMPS:
            for x in compositions(len(ids)):
                tr = {}; g = uniquac_gamma(T, x, ids, VQ, VR, IP, tr)
                up.write(f"UNIQUAC {name} {bits(T)} {' '.join(bits(v) for v in x)} | {' '.join(bits(v) for v in g)} | {' '.join(bits(v) for v in tr['lnVg'])}\n")
    up.close()
    for k in sorted(firstdiv):
        loc.write(f"{k[0]} {k[1]} first-divergent={k[2]} count={firstdiv[k]}\n")
    loc.close()

if __name__ == '__main__':
    main()
