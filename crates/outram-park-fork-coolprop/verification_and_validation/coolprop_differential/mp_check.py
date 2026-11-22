"""High-precision (60-digit) evaluation of alphar and derivatives from CoolProp's fluid JSON,
used to decide which of {this crate, CoolProp} is closer to the exact sum at a flagged point,
and to measure the cancellation scale sum|term|.  Handles Power, Gaussian, Exponential."""
import sys, json
sys.path.insert(0, '/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad/pylib')
from mpmath import mp, mpf, exp, diff
mp.dps = 60
FL = '/tmp/claude-0/-home-user-outram-park-backend/7cef3b0b-c239-561b-8dd4-50ffa1885b0f/scratchpad/cp/CoolProp/dev/fluids/'
dump = [l.strip().split(',') for l in open(sys.argv[1]) if l.strip()]
data = {}
for r in dump:
    data.setdefault((r[1], r[2]), {})[r[0]] = r[3:]

def terms(eos):
    out = []
    for t in eos['alphar']:
        ty = t['type']
        n = t['n']
        for i in range(len(n)):
            if ty == 'ResidualHelmholtzPower':
                l = t['l'][i]
                out.append(('P', n[i], t['d'][i], t['t'][i], l, 0))
            elif ty == 'ResidualHelmholtzExponential':
                out.append(('E', n[i], t['d'][i], t['t'][i], t['l'][i], t['g'][i]))
            elif ty == 'ResidualHelmholtzGaussian':
                out.append(('G', n[i], t['d'][i], t['t'][i], t['eta'][i], t['epsilon'][i], t['beta'][i], t['gamma'][i]))
            else:
                return None
    return out

def val(T, d, tau):
    d = mpf(d); tau = mpf(tau)
    k = T[0]
    base = mpf(T[1]) * d ** mpf(T[2]) * tau ** mpf(T[3])
    if k == 'P':
        return base * (exp(-d ** mpf(T[4])) if T[4] > 0 else 1)
    if k == 'E':
        return base * (exp(-mpf(T[5]) * d ** mpf(T[4])) if T[4] > 0 else 1)
    return base * exp(-mpf(T[4]) * (d - mpf(T[5])) ** 2 - mpf(T[6]) * (tau - mpf(T[7])) ** 2)

names = ['ar', 'ar_d', 'ar_t', 'ar_dd', 'ar_dt', 'ar_tt']
orders = [(0, 0), (1, 0), (0, 1), (2, 0), (1, 1), (0, 2)]
print('fluid,idx,term,rust_err_vs_exact,cp_err_vs_exact,scale_sum_abs_terms,rust_err_over_scale,cp_err_over_scale')
for (f, idx), v in sorted(data.items()):
    eos = json.load(open(FL + f + '.json'))['EOS'][0]
    tl = terms(eos)
    if tl is None:
        print(f, idx, 'unsupported term type'); continue
    d0, t0 = (mpf(x) for x in v['IN'])
    for k, (nm, o) in enumerate(zip(names, orders)):
        fn = lambda d, tau: sum(val(T, d, tau) for T in tl)
        ex = diff(fn, (d0, t0), o) if o != (0, 0) else fn(d0, t0)
        scale = sum(abs(val(T, d0, t0)) for T in tl) if o == (0, 0) else None
        rs = mpf(v['RUST'][k]); cp = mpf(v['CP'][k])
        er = abs(rs - ex) / abs(ex); ec = abs(cp - ex) / abs(ex)
        if scale is not None:
            print(f'{f},{idx},{nm},{float(er):.2e},{float(ec):.2e},{float(scale):.3e},{float(abs(rs-ex)/scale):.2e},{float(abs(cp-ex)/scale):.2e}')
        else:
            print(f'{f},{idx},{nm},{float(er):.2e},{float(ec):.2e}')
