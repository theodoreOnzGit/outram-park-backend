"""Maxwell-condition residuals, in 60-digit arithmetic, of a saturation triple (T, rhoL, rhoV)
from CoolProp and from this crate, for the same (fluid, T).  The exact solution has both
residuals zero: equal pressure and equal Gibbs energy (reduced residual Helmholtz energy)."""
import sys, json
sys.path.insert(0, '/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad/pylib')
from mpmath import mp, mpf, exp, log, diff
mp.dps = 60
SP = '/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad/'
FL = SP + 'cp/CoolProp/dev/fluids/'
sys.path.insert(0, SP + 'scripts')
meta = {}
cpsat = {}
for l in open(SP + 'cp/full.csv'):
    c = l.strip().split(',')
    if c[0] == 'META':
        meta[c[1]] = dict(M=float(c[2]), R=float(c[3]), Tr=float(c[4]), rhor=float(c[5]))
    if c[0] == __import__('os').environ.get('SATTAG','SAT'):
        cpsat[(c[1], c[2])] = [float(x) if x != 'nan' else None for x in c[3:6]]
rust = {}
for l in open(SP + 'cp/log.csv'):
    c = l.strip().split(',')
    if len(c) < 6 or c[1] not in (__import__('os').environ.get('SATTAG','SAT').lower()+'_p', __import__('os').environ.get('SATTAG','SAT').lower()+'_rhoL', __import__('os').environ.get('SATTAG','SAT').lower()+'_rhoV'):
        continue
    if c[4] == 'none':
        continue
    rust.setdefault((c[0], c[2]), {})[c[1]] = float(c[4])

def terms(eos):
    out = []
    for t in eos['alphar']:
        ty = t['type']; n = t['n']
        for i in range(len(n)):
            if ty == 'ResidualHelmholtzPower':
                out.append(('P', n[i], t['d'][i], t['t'][i], t['l'][i], 0))
            elif ty == 'ResidualHelmholtzExponential':
                out.append(('E', n[i], t['d'][i], t['t'][i], t['l'][i], t['g'][i]))
            elif ty == 'ResidualHelmholtzGaussian':
                out.append(('G', n[i], t['d'][i], t['t'][i], t['eta'][i], t['epsilon'][i], t['beta'][i], t['gamma'][i]))
            else:
                return None
    return out

def val(T, d, tau):
    k = T[0]
    base = mpf(T[1]) * d ** mpf(T[2]) * tau ** mpf(T[3])
    if k == 'P':
        return base * (exp(-d ** mpf(T[4])) if T[4] > 0 else 1)
    if k == 'E':
        return base * (exp(-mpf(T[5]) * d ** mpf(T[4])) if T[4] > 0 else 1)
    return base * exp(-mpf(T[4]) * (d - mpf(T[5])) ** 2 - mpf(T[6]) * (tau - mpf(T[7])) ** 2)

def resid(tl, d, tau):
    f = lambda x: sum(val(T, x, tau) for T in tl)
    return f(d), diff(f, d)

for spec in sys.argv[1:]:
    fl, T = spec.split('@')
    m = meta[fl]
    eos = json.load(open(FL + fl + '.json'))['EOS'][0]
    tl = terms(eos)
    if tl is None:
        print(fl, 'unsupported'); continue
    Tm = mpf(T); tau = mpf(m['Tr']) / Tm
    key = (fl, T)
    cands = {}
    c = cpsat.get(key)
    if c:
        cands['coolprop'] = c
    r = rust.get(key)
    if r and len(r) == 3:
        cands['rust'] = [r[__import__('os').environ.get('SATTAG','SAT').lower()+'_p'], r[__import__('os').environ.get('SATTAG','SAT').lower()+'_rhoL'], r[__import__('os').environ.get('SATTAG','SAT').lower()+'_rhoV']]
    for who, (p, rl, rv) in cands.items():
        dL = mpf(rl) / m['M'] / mpf(m['rhor']); dV = mpf(rv) / m['M'] / mpf(m['rhor'])
        aL, adL = resid(tl, dL, tau); aV, adV = resid(tl, dV, tau)
        pL = dL * (1 + dL * adL); pV = dV * (1 + dV * adV)   # p/(rho_r R T)
        eqp = abs(pL - pV) / max(abs(pL), abs(pV))
        gres = abs((aL + dL * adL + log(dL)) - (aV + dV * adV + log(dV)))
        print(f'{fl}@{T} {who:8s} p_given={p:.12e} p_from_L={float(pL*m["rhor"]*m["R"]*Tm):.12e} '
              f'rhoL={rl:.10g} rhoV={rv:.10g} rel(pL-pV)={float(eqp):.2e} |dg/RT|={float(gres):.2e}')
