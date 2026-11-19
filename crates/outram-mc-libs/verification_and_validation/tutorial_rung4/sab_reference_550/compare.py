"""Compare the sampled moments of each processed table with the direct
integral of the evaluation (GitHub #550). Run with python -I.
Inputs (this folder): moments_550.csv (Rust example sab_moments_550),
reference_endf_law.csv / reference_njoy_law.csv (sab_reference.py)."""
import math
import os

here = os.path.dirname(os.path.abspath(__file__))
TABLES = ['endf', 'ace', 'node', 'fine']


def read_ref(name):
    out = {}
    for l in open(os.path.join(here, name)):
        if l.startswith('#') or l.startswith('law'):
            continue
        p = l.strip().split(',')
        out[float(p[1])] = dict(sigma=float(p[4]), ep=float(p[5]), var=float(p[6]),
                                mu=float(p[7]), cov=float(p[8]))
    return out


ref = read_ref('reference_endf_law.csv')
refn = read_ref('reference_njoy_law.csv')
tabs, sig, nodetab, nearest = {}, {}, [], {}
for l in open(os.path.join(here, 'moments_550.csv')):
    p = l.strip().split(',')
    if p[0] == 'sampled':
        tabs.setdefault(p[1], {})[float(p[2])] = [float(v) for v in p[3:]]
    elif p[0] == 'sigma':
        sig[float(p[1])] = [float(v) for v in p[2:]]
    elif p[0] == 'nodetable':
        nodetab.append(p[1:])
    elif p[0] == 'nearest':
        nearest[float(p[1])] = (float(p[2]), float(p[3]))

energies = [e for e in sorted(tabs['endf']) if e in ref]
print("## <E'> relative to the direct integral (ENDF INT=4 law); sampled sem relative in brackets")
print("E_eV | ref <E'> | njoy-law ref vs endf-law ref | nearest calcem node offset | " + ' | '.join(TABLES))
for e in energies:
    r = ref[e]
    cells = []
    for t in TABLES:
        m = tabs[t][e]
        cells.append(f"{m[0] / r['ep'] - 1:+.2e} ({m[1] / r['ep']:.0e})")
    print(f"{e} | {r['ep']:.6f} | {refn[e]['ep'] / r['ep'] - 1:+.1e} | {nearest[e][1]:+.3f} | " + ' | '.join(cells))
for q, idx, key, kind in [("sd(E') relative", 2, 'var', 'sqrt'), ('<mu> absolute', 4, 'mu', 'abs'),
                          ("cov(E',mu) relative", 6, 'cov', 'rel')]:
    print(f'\n## {q} difference from the direct integral')
    print('E_eV | ' + ' | '.join(TABLES))
    for e in energies:
        r = ref[e][key]
        cells = []
        for t in TABLES:
            v = tabs[t][e][idx]
            if kind == 'sqrt':
                cells.append(f'{math.sqrt(v) / math.sqrt(r) - 1:+.2e}')
            elif kind == 'abs':
                cells.append(f'{v - r:+.2e}')
            else:
                cells.append(f'{v / r - 1:+.2e}')
        print(f'{e} | ' + ' | '.join(cells))
print('\n## sigma_inel relative to the direct integral (inelastic_xs of: endf, ace, node)')
for e in sorted(sig):
    if e in ref:
        print(f"{e} | ref {ref[e]['sigma']:.5f} | " + ' | '.join(f"{v / ref[e]['sigma'] - 1:+.2e}" for v in sig[e]))
print("\n## node tables (no sampling), 8 vs 16 cosines: E, n_cos, <E'>, var, <mu>, cov")
for r in nodetab:
    print(' | '.join(r))
