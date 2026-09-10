#!/usr/bin/env python3
"""Screen ENDF-6 tapes for the format flags this port's V&V gaps are keyed on.

Prints, per tape: every (LRU, LRF) resonance-range pair present in MF=2/MT=151,
every MF=32/MT=151 LCOMP, and every MF=7/MT=4 (NI, NS, B(7)).

Written 2026-09-10 to screen all 68 tapes in the NJOY2016 upstream
tests/resources/ suite; kept here so the negative results in README.md's
"Still wanted" section can be re-derived rather than taken on trust.

    python3 screen_endf_flags.py *.endf

Caveat: range headers are identified by a field-shape heuristic (two ordered
energies followed by four small ints in the legal LRU/LRF/NRO/NAPS ranges), not
by a full sequential parse of the section. It over-matches on LRU=0/LRF=0 rows,
so an LCOMP reported against an "LRU0/LRF0" row is noise -- confirm any hit by
reading the record. It does not under-match: a real LRF=4 or LCOMP=1 range will
always be reported.
"""

import sys, os, re

def ff(s):
    s = s.strip().replace(' ', '')
    if not s: return None
    m = re.match(r'^([+-]?\d*\.?\d*)([+-]\d+)$', s)
    if m:
        try: return float(m.group(1) + 'e' + m.group(2))
        except: return None
    try: return float(s)
    except: return None

def ii(s):
    s = s.strip()
    if not s: return 0
    try: return int(s)
    except: return None

def flds(l): return [l[0:11], l[11:22], l[22:33], l[33:44], l[44:55], l[55:66]]

def is_range_hdr(l):
    c = flds(l)
    el, eh = ff(c[0]), ff(c[1])
    lru, lrf, nro, naps = ii(c[2]), ii(c[3]), ii(c[4]), ii(c[5])
    if el is None or eh is None: return None
    if None in (lru, lrf, nro, naps): return None
    if not (0 <= el < eh <= 1e9): return None
    if lru not in (0,1,2) or lrf not in (0,1,2,3,4,5,6,7): return None
    if nro not in (0,1) or naps not in (0,1,2): return None
    if lru == 0 and lrf != 0: return None
    if lru != 0 and lrf == 0: return None
    return (lru, lrf, el, eh)

def screen(path):
    res = {'lrf': set(), 'mf32': [], 'tsl': []}
    try:
        lines = open(path, encoding='latin-1').read().splitlines()
    except Exception as e:
        return None
    prev_mat_mf = None
    for n, l in enumerate(lines):
        if len(l) < 75: continue
        mat, mf, mt = l[66:70].strip(), l[70:72].strip(), l[72:75].strip()
        if mt != '151': 
            pass
        if mf == '2' and mt == '151':
            r = is_range_hdr(l)
            if r: res['lrf'].add((r[0], r[1]))
        elif mf == '32' and mt == '151':
            r = is_range_hdr(l)
            if r and n+1 < len(lines):
                lcomp = ii(flds(lines[n+1])[3])
                res['mf32'].append((mat, r[0], r[1], lcomp))
        elif mf == '7' and mt == '4':
            # section head then LIST head (0 0 LLN 0 NI NS)
            c = flds(l)
            if ff(c[0]) == 0.0 and ff(c[1]) == 0.0:
                ni, ns = ii(c[4]), ii(c[5])
                if ni and ni >= 6 and ns is not None and ns <= 3 and n+1 < len(lines):
                    bs = []
                    k = n+1
                    while len(bs) < ni and k < len(lines):
                        bs += [ff(x) for x in flds(lines[k])]
                        k += 1
                    b7 = bs[6] if ni >= 7 and len(bs) > 6 else None
                    res['tsl'].append((mat, ni, ns, b7))
    return res

rows = []
for p in sorted(sys.argv[1:]):
    if os.path.isdir(p): continue
    r = screen(p)
    if r is None: continue
    name = os.path.basename(p)
    lrf = ",".join(f"{a}/{b}" for a,b in sorted(r['lrf'])) or "-"
    mf32 = "; ".join(f"MAT{m} LRU{a}/LRF{b} LCOMP={c}" for m,a,b,c in r['mf32']) or "-"
    tsl = "; ".join(f"MAT{m} NI={i} NS={s} B7={b}" for m,i,s,b in r['tsl']) or "-"
    rows.append((name, lrf, mf32, tsl))

print(f"{'FILE':<42} {'LRU/LRF present':<26} MF=32")
print("-"*120)
for name, lrf, mf32, tsl in rows:
    print(f"{name:<42} {lrf:<26} {mf32}")
print()
print("=== tsl MF=7/MT=4 B-constants ===")
for name, lrf, mf32, tsl in rows:
    if tsl != "-": print(f"{name:<42} {tsl}")
