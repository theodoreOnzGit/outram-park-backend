import sys
from decimal import Decimal, getcontext
getcontext().prec = 40
S = sys.argv[1]
MESH = sys.argv[2]

def rd(f):
    t = open(MESH + '/' + f).read()
    i = t.index('\n(\n')
    return [int(x) for x in t[i + 3:t.rindex('\n)')].split()]

own = rd('owner'); nei = rd('neighbour')

def load(path):
    d = {}
    for l in open(path):
        w = l.split()
        if w[1] == '1':
            d[w[2]] = [Decimal(x) for x in w[3:]]
    return d

u = load(S + '/probe_vfu.txt'); p = load(S + '/port_vfu.txt')
n = 477
dg_u = list(u['pdiag']); sr_u = list(u['psource'])
starts = [1104, 1113, 1122, 1440]; sizes = [9, 9, 318, 318]
for pi in range(4):
    for k in range(sizes[pi]):
        c = own[starts[pi] + k]
        dg_u[c] += u['pint%d' % pi][k]
        sr_u[c] += u['pbnd%d' % pi][k]

def solve(dg, up, lo, b):
    bw = max(abs(own[f] - nei[f]) for f in range(len(nei)))
    A = [dict() for _ in range(n)]
    for i in range(n):
        A[i][i] = dg[i]
    for f in range(len(nei)):
        o, m = own[f], nei[f]
        A[o][m] = A[o].get(m, Decimal(0)) + up[f]
        A[m][o] = A[m].get(o, Decimal(0)) + lo[f]
    b = list(b)
    for k in range(n):
        piv = A[k][k]
        for i in range(k + 1, min(n, k + bw + 1)):
            if k in A[i]:
                fct = A[i][k] / piv
                for j, v in A[k].items():
                    if j >= k:
                        A[i][j] = A[i].get(j, Decimal(0)) - fct * v
                b[i] -= fct * b[k]
                del A[i][k]
    x = [Decimal(0)] * n
    for i in range(n - 1, -1, -1):
        s = b[i]
        for j, v in A[i].items():
            if j > i:
                s -= v * x[j]
        x[i] = s / A[i][i]
    return x

ref = u['p_rgh_post']
def diff(x):
    return max(abs(x[i] - ref[i]) for i in range(n))
cases = {
 'port matrix+port source': (p['pdiag'], p['pupper'], p['plower'], p['psource']),
 'up matrix+up source': (dg_u, u['pupper'], u['pupper'], sr_u),
 'port matrix+up source': (p['pdiag'], p['pupper'], p['plower'], sr_u),
 'up diag+port rest': (dg_u, p['pupper'], p['plower'], p['psource']),
}
for name, (dg, up, lo, b) in cases.items():
    x = solve(dg, up, lo, b)
    print(name, 'max|x - upstream PCG|', float(diff(x)), ' max|x - port|', float(max(abs(x[i] - p['p_rgh_post'][i]) for i in range(n))))
