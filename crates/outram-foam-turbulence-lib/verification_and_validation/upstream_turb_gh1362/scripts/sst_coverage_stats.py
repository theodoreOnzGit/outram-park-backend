import gzip, struct, math, collections
import os
base = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'reference') + '/'
def load(m):
    d = collections.defaultdict(list)
    for line in gzip.open(base + 'ref_%s.txt.gz' % m, 'rt'):
        t = line.split()
        if len(t) >= 3 and all(len(x) == 16 for x in t[2:]):
            try:
                d[t[0]].append([struct.unpack('>d', bytes.fromhex(x))[0] for x in t[2:]])
            except Exception:
                pass
    return d
d = load('kOmegaSST')
s = lambda k: [v[0] for v in d[k]]
n = 360
w, s2, f23, g0, g, pk, gb = s('omega'), s('T_S2'), s('T_F23'), s('T_GbyNu0'), s('T_G'), s('T_Pk'), s('T_GbyNu')
f1 = s('T_F1')
print('SST nut limiter active (b1 F23 sqrt(S2) > a1 omega):', sum(1 for i in range(n) if f23[i]*math.sqrt(s2[i]) > 0.31*w[i]))
print('SST Pk limited (Pk < G):', sum(1 for i in range(n) if pk[i] < g[i]))
print('SST GbyNu limited (GbyNu < GbyNu0):', sum(1 for i in range(n) if gb[i] < g0[i]))
print('SST F1 in (0.01,0.99):', sum(1 for x in f1 if 0.01 < x < 0.99), ' F1==1:', sum(1 for x in f1 if x == 1.0), ' F1<0.01:', sum(1 for x in f1 if x < 0.01))
cd = s('T_CDkOmega')
print('CDkOmega < 1e-10:', sum(1 for x in cd if x < 1e-10))
dv = s('divU')
print('divU range', min(dv), max(dv))
e = load('kEpsilon')
print('eps min', min(s2 for s2 in [v[0] for v in e['epsilon']]))
# near-wall y+ proxy: min y
y = s('y')
print('y min/max', min(y), max(y))
