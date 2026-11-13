import math, os
HERE = os.path.dirname(os.path.abspath(__file__))
exec(open(os.path.join(HERE, 'sst_coverage_stats.py')).read().split("d = load('kOmegaSST')")[0])
d = load('kOmegaSST')
s = lambda k: [v[0] for v in d[k]]
k, w, y, nu, cd, f1, f23, s2 = s('k'), s('omega'), s('y'), s('nu'), s('T_CDkOmega'), s('T_F1'), s('T_F23'), s('T_S2')
bs, a2 = 0.09, 0.856
same1 = same2 = 0
for i in range(360):
    cdp = cd[i] if cd[i] > 1e-10 else 1e-10
    A = ((1.0 / bs) * math.sqrt(k[i])) / (w[i] * y[i])
    B = (500.0 * nu[i]) / ((y[i] * y[i]) * w[i])
    C = ((4.0 * a2) * k[i]) / (cdp * (y[i] * y[i]))
    m = max(A, B)
    m = m if m < C else C
    m = m if m < 10.0 else 10.0
    sq = m * m
    same1 += math.tanh(sq * sq) == f1[i]
    A2 = ((2.0 / bs) * math.sqrt(k[i])) / (w[i] * y[i])
    m2 = max(A2, B)
    m2 = m2 if m2 < 100.0 else 100.0
    same2 += math.tanh(m2 * m2) == f23[i]
print('glibc tanh replay (python math.tanh) reproduces upstream F1: %d/360, F23: %d/360' % (same1, same2))
