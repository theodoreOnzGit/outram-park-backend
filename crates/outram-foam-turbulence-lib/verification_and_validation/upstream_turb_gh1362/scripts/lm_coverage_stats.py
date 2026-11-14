# Branch coverage of the kOmegaSSTLM frozen duct, from the dumped INPUTS only
# (k, omega, nu, U, y, gradU, ReThetat, gammaInt, gammaIntEff_in); nothing from the model's own outputs.
import gzip, struct, math, collections, os
base = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'reference') + '/'
d = collections.defaultdict(list)
for line in gzip.open(base + 'ref_kOmegaSSTLM.txt.gz', 'rt'):
    t = line.split()
    if len(t) >= 3 and all(len(x) == 16 for x in t[2:]):
        d[t[0]].append([struct.unpack('>d', bytes.fromhex(x))[0] for x in t[2:]])
s = lambda k: [v[0] for v in d[k]]
n = 360
k, w, nu, y, rt = s('k'), s('omega'), s('nu'), s('y'), s('ReThetat')
gi, ge = s('gammaInt'), s('gammaIntEff_in')
U = [(v[0], v[1], v[2]) for v in d['U']]
G = d['gradU']
Us = [max(math.sqrt(a*a+b*b+c*c), 1e-15) for a, b, c in U]
Tu = [max(100*math.sqrt(2.0/3.0*k[i])/Us[i], 0.027) for i in range(n)]
RT = [k[i]/(nu[i]*w[i]) for i in range(n)]
Ry = [y[i]*math.sqrt(k[i])/nu[i] for i in range(n)]
def S(g):
    sxy, sxz, syz = 0.5*(g[1]+g[3]), 0.5*(g[2]+g[6]), 0.5*(g[5]+g[7])
    return math.sqrt(2*(g[0]**2+g[4]**2+g[8]**2+2*(sxy**2+sxz**2+syz**2)))
Sv = [S(G[i]) for i in range(n)]
Rev = [y[i]**2*Sv[i]/nu[i] for i in range(n)]
def dus(i):
    u, g = U[i], G[i]
    vg = (u[0]*g[0]+u[1]*g[3]+u[2]*g[6], u[0]*g[1]+u[1]*g[4]+u[2]*g[7], u[0]*g[2]+u[1]*g[5]+u[2]*g[8])
    return (u[0]*vg[0]+u[1]*vg[1]+u[2]*vg[2])/Us[i]**2
dUsds = [dus(i) for i in range(n)]
c = lambda f, a: sum(1 for x in a if f(x))
print('Tu <= 1.3:', c(lambda x: x <= 1.3, Tu), ' Tu > 1.3:', c(lambda x: x > 1.3, Tu), ' Tu == 0.027 clamp:', c(lambda x: x == 0.027, Tu))
print('dUsds <= 0:', c(lambda x: x <= 0, dUsds), ' > 0:', c(lambda x: x > 0, dUsds))
print('ReThetat <400 / <596 / <1200 / >=1200:', c(lambda x: x < 400, rt), c(lambda x: 400 <= x < 596, rt), c(lambda x: 596 <= x < 1200, rt), c(lambda x: x >= 1200, rt), ' > 1870:', c(lambda x: x > 1870, rt))
print('RT < 2.5:', c(lambda x: x < 2.5, RT), ' RT in (0.1,20):', c(lambda x: 0.1 < x < 20, RT), ' RT range', min(RT), max(RT))
print('Ry/120 in (0.3, 1.8) (F3 neither 1 nor 0):', c(lambda x: 0.3 < x/120 < 1.8, Ry))
fo = [Rev[i]/(2.193*rt[i]) for i in range(n)]
print('Fonset1 (with ReThetac ~ ReThetat) in (0.1, 1.2):', c(lambda x: 0.1 < x < 1.2, fo), ' >= 1.2:', c(lambda x: x >= 1.2, fo))
print('gammaIntEff_in < 0.1 / in [0.1,1] / > 1:', c(lambda x: x < 0.1, ge), c(lambda x: 0.1 <= x <= 1, ge), c(lambda x: x > 1, ge))
print('gammaInt range', min(gi), max(gi))
