"""Independent reference for H-in-H2O S(alpha,beta) (GitHub #550).

Integrates the double-differential cross section of ENDF/B-VIII.0
tsl-HinH2O (MF=7, MT=4, 293.6 K) directly, with no THERMR, no ACE and no
equiprobable binning, and prints sigma_inel, <E'>, var(E'), <mu>, cov(E',mu)
at the requested incident energies.

Conventions (read from NJOY2016 thermr.f90 `sig`, l. 2482-2615, and calcem,
l. 1650-1800; ENDF-6 manual sec. 7.4):
  LAT = 1 (alpha, beta in units of kT0 = 0.0253 eV), LASYM = 0 (beta >= 0
  stored, S symmetric; exp(-beta/2) restores the physical S), LLN = 0 (S, not
  ln S).  d2sigma/dE'dmu = sb sqrt(E'/E)/(2 kT) S(alpha,beta) exp(-beta/2),
  sb = B1/natom ((A+1)/A)^2.  Outside the tabulated (alpha,beta) box: the
  short-collision-time law with T_eff from the tape's second TAB1.
  Interpolation of S inside the box, two laws:
    "endf":  the evaluation's own stated law, INT = 4 (ln S linear in alpha
             and in beta), alpha < alpha_min handled by the diffusion
             (cliq) extrapolation as THERMR does for beta <= 0.2, else
             S(alpha_min, beta);
    "njoy":  THERMR's quadratic interpolation of ln S (terpq), for control.
Run with python -I.  Usage:
  python -I sab_reference.py TAPE [law] [refinement]
"""
import io
import sys

import numpy as np
from openmc.data.endf import get_head_record, get_list_record, get_tab1_record, get_tab2_record

BK = 8.617333262e-5  # eV/K, NJOY2016 phys.f90
TEVZ = 0.0253  # eV, thermr `therm`
SABFLG = -225.0
TEMP = 293.6


def read_tape(path, temp=TEMP):
    lines = [l for l in open(path) if l[70:72] == " 7" and l[72:75] == "  4"]
    f = io.StringIO("".join(lines))
    head = get_head_record(f)
    lat, lasym = head[3], head[4]
    lst = get_list_record(f)
    b = lst[1]
    lln = lst[0][2]
    t2hdr = get_tab2_record(f)
    nbeta = t2hdr[0][5]
    betas, rows = [], []
    for _ in range(nbeta):
        p, tab = get_tab1_record(f)
        t_first, beta, lt = p[0], p[1], p[2]
        alpha = np.asarray(tab.x)
        series = {t_first: np.asarray(tab.y)}
        for _ in range(lt):
            lp, data = get_list_record(f)
            series[lp[0]] = np.asarray(data)
        betas.append(beta)
        t_use = min(series, key=lambda t: abs(t - temp))
        assert abs(t_use - temp) < 0.01, (t_use, temp)
        rows.append(series[t_use])
    # effective temperature of the principal scatterer
    p, teff_tab = get_tab1_record(f)
    teff_k = float(np.interp(temp, teff_tab.x, teff_tab.y))
    return dict(
        lat=lat, lasym=lasym, lln=lln, b=b, alpha=alpha, beta=np.asarray(betas),
        S=np.vstack(rows), teff_k=teff_k,
    )


def lerp_idx(grid, x):
    i = np.searchsorted(grid, x, side="right") - 1
    return np.clip(i, 0, len(grid) - 2)


def lnS_endf(tab, a, b):
    """ln S by the evaluation's INT = 4 law (ln S linear in alpha and in beta)."""
    al, be, L = tab["alpha"], tab["beta"], tab["L"]
    ia = lerp_idx(al, a)
    ib = lerp_idx(be, b)
    fa = np.clip((a - al[ia]) / (al[ia + 1] - al[ia]), 0.0, 1.0)  # a<alpha1 clamps to S(alpha1)
    fb = (b - be[ib]) / (be[ib + 1] - be[ib])
    s_lo = L[ib, ia] * (1 - fa) + L[ib, ia + 1] * fa
    s_hi = L[ib + 1, ia] * (1 - fa) + L[ib + 1, ia + 1] * fa
    return s_lo * (1 - fb) + s_hi * fb


def terpq(x1, y1, x2, y2, x3, y3, x):
    """thermr.f90 terpq, vectorised."""
    with np.errstate(all="ignore"):
        lo = x < x1
        hi = x > x3
        step = (np.abs(y1 - y2) > 2.0) | (np.abs(y2 - y3) > 2.0)
        # lo branch
        y_lo = np.where(y1 > y2, y1, y1 + (y2 - y1) * np.log(x / x1) / np.log(x2 / x1))
        y_hi = np.where(y3 > y2, y3, y2 + (y3 - y2) * (x - x2) / (x3 - x2))
        y_st = np.where(x < x2, y1 + (y2 - y1) * (x - x1) / (x2 - x1), y2 + (y3 - y2) * (x - x2) / (x3 - x2))
        bq = (y2 - y1) * (x3 - x1) / ((x2 - x1) * (x3 - x2)) - (y3 - y1) * (x2 - x1) / ((x3 - x1) * (x3 - x2))
        cq = (y3 - y1) / ((x3 - x1) * (x3 - x2)) - (y2 - y1) / ((x2 - x1) * (x3 - x2))
        y_q = y1 + bq * (x - x1) + cq * (x - x1) ** 2
        y = np.where(lo, y_lo, np.where(hi, y_hi, np.where(step, y_st, y_q)))
    return np.maximum(y, SABFLG)


def lnS_njoy(tab, a, b):
    al, be, L = tab["alpha"], tab["beta"], tab["L"]
    na, nb = len(al), len(be)
    ia = np.clip(np.searchsorted(al, a, side="right") - 1, 0, na - 2)
    ib = np.clip(np.searchsorted(be, b, side="right") - 1, 0, nb - 2)
    ia = np.where(ia + 1 == na - 1, ia - 1, ia)  # thermr: if (ia+1.eq.nalpha) ia=ia-1 (1-based)
    ib = np.where(ib + 1 == nb - 1, ib - 1, ib)
    s = []
    for k in range(3):
        s.append(terpq(al[ia], L[ib + k, ia], al[ia + 1], L[ib + k, ia + 1], al[ia + 2], L[ib + k, ia + 2], a))
    return terpq(be[ib], s[0], be[ib + 1], s[1], be[ib + 2], s[2], b)


def kernel(tab, law, E, Ep, mu, tev, teff):
    """d2sigma/dE'dmu [b/eV] per principal atom; arrays Ep (n,1), mu (1,m)."""
    az, sb = tab["az"], tab["sb"]
    rtev = 1.0 / tev
    bb = (Ep - E) * rtev
    a = (E + Ep - 2 * mu * np.sqrt(E * Ep)) / (az * tev)
    a = np.maximum(a, 1e-6)
    sigc = np.sqrt(Ep / E) * rtev / 2
    b = np.abs(bb)
    at = a * tev / TEVZ  # LAT = 1
    bt = b * tev / TEVZ
    al, be, L = tab["alpha"], tab["beta"], tab["L"]
    inside = (at <= al[-1]) & (bt <= be[-1])
    # corner floor test (thermr: sab(ia..) <= sabflg -> SCT) is only active when a*az>=30 or b>=30
    lnS_fn = lnS_endf if law == "endf" else lnS_njoy
    atc = np.clip(at, al[0], al[-1])
    s_tab = lnS_fn(tab, atc, np.clip(bt, be[0], be[-1]))
    # alpha below the table: diffusion extrapolation for beta <= 0.2 when cliq != 0
    cliq = tab["cliq"]
    low = at < al[0]
    if cliq != 0.0:
        s_dif = L[0, 0] + np.log(al[0] / np.maximum(at, 1e-300)) / 2 - cliq * bt**2 / np.maximum(at, 1e-300)
        s_dif = np.maximum(s_dif, SABFLG)
        use_dif = low & (bt <= 0.2)
        s_tab = np.where(use_dif, s_dif, s_tab)
    with np.errstate(all="ignore"):
        tab_val = np.where(s_tab - bb / 2 > SABFLG, np.exp(s_tab - bb / 2), 0.0)
        # short collision time
        arg = (a - b) ** 2 * tev / (4 * a * teff) + (b + bb) / 2
        sct = np.where(-arg > SABFLG, np.exp(-arg) / (np.sqrt(4 * np.pi) * np.sqrt(a * teff * rtev)), 0.0)
    s = np.where(inside, tab_val, sct)
    sig = sigc * sb * s
    return np.where(sig < 1e-10, 0.0, sig), inside


def moments(tab, law, E, n_mu, m_sub, tev, teff, e_hi_extra=60.0):
    bmax_e = tab["beta"][-1] * TEVZ
    edges = {E, 0.0}
    for bv in tab["beta"]:
        for sgn in (-1, 1):
            e = E + sgn * bv * TEVZ
            if 0 < e < E + e_hi_extra:
                edges.add(e)
    edges.add(E + e_hi_extra)
    # extra edges in the SCT tail
    for x in np.geomspace(max(bmax_e, 1e-3), e_hi_extra, 40):
        edges.add(E + x)
    edges = np.array(sorted(edges))
    gx, gw = np.polynomial.legendre.leggauss(8)
    nodes, wts = [], []
    for lo, hi in zip(edges[:-1], edges[1:]):
        sub = np.linspace(lo, hi, m_sub + 1)
        for s0, s1 in zip(sub[:-1], sub[1:]):
            nodes.append(0.5 * (s1 - s0) * gx + 0.5 * (s1 + s0))
            wts.append(0.5 * (s1 - s0) * gw)
    Ep = np.concatenate(nodes)[:, None]
    wE = np.concatenate(wts)
    # mu = 1 - 2 y^2, dmu = 4 y dy, y in [0,1]; split y at 0.1 for the near-forward peak
    yg, yw = np.polynomial.legendre.leggauss(n_mu)
    ys, wy = [], []
    for lo, hi in [(0.0, 0.02), (0.02, 0.1), (0.1, 0.4), (0.4, 1.0)]:
        ys.append(0.5 * (hi - lo) * yg + 0.5 * (hi + lo))
        wy.append(0.5 * (hi - lo) * yw)
    y = np.concatenate(ys)[None, :]
    wyy = np.concatenate(wy)[None, :] * 4 * y
    mu = 1 - 2 * y**2
    k, inside = kernel(tab, law, E, Ep, mu, tev, teff)
    W = k * wE[:, None] * wyy
    sigma = W.sum()
    epm = (W * Ep).sum() / sigma
    ep2 = (W * Ep**2).sum() / sigma
    mum = (W * mu).sum() / sigma
    cov = (W * Ep * mu).sum() / sigma - epm * mum
    sig_in = (W * inside).sum()
    sig_lowalpha = 0.0
    return dict(sigma=sigma, mean_ep=epm, var_ep=ep2 - epm**2, mean_mu=mum, cov=cov,
                corr=cov / np.sqrt((ep2 - epm**2) * ((W * mu**2).sum() / sigma - mum**2)),
                sct_fraction=1 - sig_in / sigma)


def main():
    tape = sys.argv[1]
    laws = [sys.argv[2]] if len(sys.argv) > 2 else ["endf", "njoy"]
    levels = [(32, 1), (64, 2), (128, 4)] if len(sys.argv) <= 3 else [tuple(int(v) for v in sys.argv[3].split(","))]
    t = read_tape(tape)
    assert t["lat"] == 1 and t["lasym"] == 0 and t["lln"] == 0, (t["lat"], t["lasym"], t["lln"])
    b = t["b"]
    natom = b[5]
    t["az"] = b[2]
    t["sb"] = b[0] / natom * ((b[2] + 1) / b[2]) ** 2
    t["L"] = np.log(np.maximum(t["S"], np.exp(SABFLG)))
    t["L"] = np.where(t["S"] > np.exp(SABFLG), t["L"], SABFLG)
    L = t["L"]
    t["cliq"] = (L[0, 0] - L[1, 0]) * t["alpha"][0] / t["beta"][1] ** 2 if L[0, 0] > L[0, 1] else 0.0
    tev = BK * TEMP
    teff = t["teff_k"] * BK
    print(f"# tape {tape}: nalpha {len(t['alpha'])} alpha[{t['alpha'][0]:.4g},{t['alpha'][-1]:.4g}]"
          f" nbeta {len(t['beta'])} beta max {t['beta'][-1]:.4g}; sb {t['sb']:.5f} b; A {t['az']:.7f};"
          f" T_eff {t['teff_k']:.3f} K; cliq {t['cliq']:.6g}; kT {tev:.8f} eV")
    energies = [float(v) for v in (sys.argv[4].split(",") if len(sys.argv) > 4 else
                ["0.025", "0.0253", "0.1", "0.3", "0.625", "1.0", "1.5", "3.5"])]
    print("law,E,nmu,msub,sigma_b,mean_Ep,var_Ep,mean_mu,cov_Ep_mu,corr,sct_frac")
    for law in laws:
        for E in energies:
            for (nm, ms) in levels:
                r = moments(t, law, E, nm, ms, tev, teff)
                print(f"{law},{E},{nm},{ms},{r['sigma']:.8g},{r['mean_ep']:.8g},{r['var_ep']:.8g},"
                      f"{r['mean_mu']:.8g},{r['cov']:.8g},{r['corr']:.6g},{r['sct_fraction']:.4g}", flush=True)


if __name__ == "__main__":
    main()
