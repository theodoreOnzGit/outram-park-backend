"""Replay of the gh:#1378 family-2 reaction-rate formulas on the platform libm.

Python's math.exp, math.log10, math.cbrt and float ** are glibc's functions, so
agreement with the ESI v2506 reference (also glibc) shows the translation of
each formula is exact; the residual of the Rust port is then PETIR against
glibc in the function named. Run: python3 -I replay_rates_glibc.py <reference.txt.gz>
"""
import gzip
import math
import struct
import sys

SMALL, VSMALL = 1e-15, 1e-300
TS = [250, 300, 500, 800, 1000, 1500, 2000, 3000]
CS = [(1e-2, 2e-2, 5e-3), (1e-4, 3e-4, 2e-4), (1.0, 0.5, 0.25), (1e-8, 1e-8, 1e-8)]
EFF = (1.0, 2.5, 0.4)

LOG = []


def _rec(name, args, res):
    LOG.append((name, args, res))
    return res


def pw(a, b):
    return _rec("pow", (float(a), float(b)), float(a) ** float(b))


def ex(x):
    return _rec("exp", (x,), math.exp(x))


def l10(x):
    return _rec("log10", (x,), math.log10(x))


def ln(x):
    return _rec("log", (x,), math.log(x))


def cb(x):
    return _rec("cbrt", (x,), math.cbrt(x))


def hx(v):
    return "%016x" % struct.unpack("<Q", struct.pack("<d", v))[0]



def sqr(x):
    return x * x


def M(c):
    m = 0.0
    for e, ci in zip(EFF, c):
        m += e * ci
    return m


def arr(a, beta, ta):
    def k(t, c):
        ak = a
        if abs(beta) > VSMALL:
            ak *= pw(t, beta)
        if abs(ta) > VSMALL:
            ak *= ex(-ta / t)
        return ak
    return k


def landau(a, beta, ta, b, cc):
    def k(t, c):
        lta = a
        if abs(beta) > VSMALL:
            lta *= pw(t, beta)
        e = 0.0
        if abs(ta) > VSMALL:
            e -= ta / t
        if abs(b) > VSMALL:
            e += b / cb(t)
        if abs(cc) > VSMALL:
            e += cc / pw(t, 2.0 / 3.0)
        if abs(e) > VSMALL:
            lta *= ex(e)
        return lta
    return k


def power_series(a, beta, coeffs):
    def k(t, c):
        lta = a
        if abs(beta) > VSMALL:
            lta *= pw(t, beta)
        e = 0.0
        for n, co in enumerate(coeffs):
            e += co / pw(t, n + 1)
        return lta * ex(e)
    return k


def janev(a, beta, ta, b):
    def k(t, c):
        lta = a
        if abs(beta) > VSMALL:
            lta *= pw(t, beta)
        e = 0.0
        if abs(ta) > VSMALL:
            e -= ta / t
        ln_t = ln(t)
        for n in range(9):
            e += b[n] * pw(ln_t, n)
        return lta * ex(e)
    return k


def lh(a, ta):
    def k(t, c):
        return a[0] * ex(-ta[0] / t) / (
            t
            * sqr(1 + a[1] * ex(-ta[1] / t) * c[0] + a[2] * ex(-ta[2] / t) * c[1])
            * (1 + a[3] * ex(-ta[3] / t) * sqr(c[0]) * sqr(c[1]))
            * (1 + a[4] * ex(-ta[4] / t) * pw(c[2], 0.7))
        )
    return k


def troe(alpha, tsss, ts, tss):
    def f(t, pr):
        lf = l10(max((1 - alpha) * ex(-t / tsss) + alpha * ex(-t / ts) + ex(-tss / t), SMALL))
        c = -0.4 - 0.67 * lf
        n = 0.75 - 1.27 * lf
        lp = l10(max(pr, SMALL))
        return pw(10.0, lf / (1.0 + sqr((lp + c) / (n - 0.14 * (lp + c)))))
    return f


def sri(a, b, c, d, e):
    def f(t, pr):
        x = 1.0 / (1.0 + sqr(l10(max(pr, SMALL))))
        return d * pw(a * ex(-b / t) + ex(-t / c), x) * pw(t, e)
    return f


lind = lambda t, pr: 1.0
K0, KI = arr(6.366e20, -1.72, 524.8), arr(1.475e12, 0.6, 0.0)


def falloff(f):
    def k(t, c):
        k0, ki = K0(t, c), KI(t, c)
        pr = k0 * M(c) / ki
        return ki * (pr / (1 + pr)) * f(t, pr)
    return k


def chem(f):
    def k(t, c):
        k0, ki = K0(t, c), KI(t, c)
        pr = k0 * M(c) / ki
        return k0 * (1 / (1 + pr)) * f(t, pr)
    return k


MODELS = {
    "Arrhenius/full": arr(3.547e15, -0.406, 8353.0),
    "Arrhenius/noBeta": arr(1.0e10, 0.0, 5000.0),
    "Arrhenius/noTa": arr(1.475e12, 0.6, 0.0),
    "Arrhenius/plain": arr(2.0, 0.0, 0.0),
    "thirdBodyArrhenius": lambda t, c: M(c) * arr(2.65e16, -0.67, 17041.0)(t, c),
    "LandauTeller/full": landau(1.0e9, 0.3, 1200.0, 35.0, -80.0),
    "LandauTeller/noTa": landau(1.0e9, 0.0, 0.0, 35.0, 0.0),
    "LandauTeller/plain": landau(5.0, 0.0, 0.0, 0.0, 0.0),
    "powerSeries": power_series(1.0e3, 0.5, [-9000.0, 2.0e6, -3.0e8, 1.0e10]),
    "Janev": janev(1.0e-8, 0.4, 3000.0, [-1.0, 0.8, -0.05, 0.003, -1.0e-4, 5.0e-6, -2.0e-7, 1.0e-8, -3.0e-10]),
    "infinite": lambda t, c: 1.0,
    "LangmuirHinshelwood": lh([1.0e3, 2.0e1, 3.0e1, 4.0e4, 5.0e2], [100.0, 500.0, 700.0, 900.0, 300.0]),
    "FallOff/Lindemann": falloff(lind),
    "FallOff/Troe": falloff(troe(0.5, 1e-30, 1e30, 1e100)),
    "FallOff/Troe/finite": falloff(troe(0.7346, 94.0, 1756.0, 5182.0)),
    "FallOff/Troe/small": falloff(troe(1.0, 1e-30, 1e-30, 1e100)),
    "FallOff/SRI": falloff(sri(0.45, 797.0, 979.0, 1.0, 0.0)),
    "FallOff/SRI/e": falloff(sri(0.5, 300.0, 1000.0, 1.2, 0.1)),
    "ChemicallyActivated/Lindemann": chem(lind),
    "ChemicallyActivated/Troe": chem(troe(0.7346, 94.0, 1756.0, 5182.0)),
    "ChemicallyActivated/SRI": chem(sri(0.45, 797.0, 979.0, 1.0, 0.0)),
}

ref = {}
for line in gzip.open(sys.argv[1], "rt"):
    if line.startswith("#"):
        continue
    tag, it, ic, bits = line.split()
    ref[(tag, int(it), int(ic))] = int(bits, 16)

total = same = 0
for tag, model in MODELS.items():
    n = 0
    for it, t in enumerate(TS):
        for ic, c in enumerate(CS):
            v = model(float(t), c)
            n += struct.unpack("<Q", struct.pack("<d", v))[0] == ref[(tag, it, ic)]
    total += 32
    same += n
    print(f"{tag:34} {n}/32")
print(f"TOTAL {same}/{total}")
if len(sys.argv) > 2:
    # distinct libm calls (name, arg bits) -> result bits, for the Rust localisation probe
    seen = {}
    for name, args, res in LOG:
        seen[(name, tuple(hx(a) for a in args))] = hx(res)
    with open(sys.argv[2], "w") as fh:
        for (name, args), res in sorted(seen.items()):
            fh.write(" ".join((name,) + args + (res,)) + "\n")
