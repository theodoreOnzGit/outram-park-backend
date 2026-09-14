"""Densify U-234's unresolved parameter energy grid by lin-lin resampling.

NJOY forces the unresolved-parameter interpolation law to lin-lin
(unresr.f90:1057-1058 reads INT then overwrites it with 2), so inserting
lin-lin-interpolated points leaves every parameter function POINTWISE
IDENTICAL. The physics is unchanged by construction; only NJOY's `eunr`
grid gets finer, so RECONR evaluates where it previously interpolated.
"""
import sys, math

def endf_float(x):
    if x == 0.0:
        return " 0.000000+0"
    s = "-" if x < 0 else " "
    a = abs(x)
    e = int(math.floor(math.log10(a)))
    m = a / (10.0 ** e)
    m = round(m, 6)
    if m >= 10.0:
        m /= 10.0; e += 1
    es = f"{e:+d}"
    body = f"{m:.6f}"
    out = s + body + es
    if len(out) > 11:                      # two-digit exponent: drop a decimal
        body = f"{m:.{max(0, 6 - (len(out) - 11))}f}"
        out = s + body + es
    return out.rjust(11)

def parse_float(t):
    t = t.strip()
    if not t: return 0.0
    for i in range(1, len(t)):
        if t[i] in "+-" and t[i-1] not in "eE":
            return float(t[:i] + "e" + t[i:])
    return float(t)

src = sys.argv[1]; dst = sys.argv[2]; nsub = int(sys.argv[3])
lines = open(src).read().split("\n")

# locate MF=2/MT=151 record indices
idx = [i for i, l in enumerate(lines) if len(l) >= 75 and l[70:72].strip() == "2" and l[72:75].strip() == "151"]
lo, hi = idx[0], idx[-1]
mat = lines[lo][66:70]

def body6(l):
    return [parse_float(l[j*11:(j+1)*11]) for j in range(6)]

# walk to the LRU=2 range
i = lo + 1                       # skip material CONT
i += 1                           # isotope CONT
out_prefix_end = None
while i <= hi:
    # ENDF CONT fields: C1[0:11] C2[11:22] L1[22:33] L2[33:44] N1[44:55] N2[55:66]
    lru = int(lines[i][22:33].strip() or 0)
    lrf = int(lines[i][33:44].strip() or 0)
    if lru == 2:
        range_hdr = i
        break
    # resolved: CONT + CONT + LIST
    # resolved (LRU=1, LRF=1/2/3): SPI/AP CONT, then per-L LIST records
    i += 1                       # SPI/AP/.../NLS CONT
    nls_r = int(lines[i-1][44:55].strip() or 0)
    for _ in range(nls_r):
        npl = int(lines[i][44:55].strip())
        i += 1 + (npl + 5)//6
else:
    raise SystemExit("no LRU=2 range found")

j = range_hdr + 1                # SPI/AP/LSSF/NLS CONT -- NLS is N1[44:55]
nls = int(lines[j][44:55].strip())
out = lines[:j+1]
j += 1
for _ in range(nls):
    out.append(lines[j])         # AWRI/L/NJS CONT
    njs = int(lines[j][44:55].strip())   # NJS is N1[44:55]
    j += 1
    for _ in range(njs):
        head = lines[j]
        ne = int(head[55:66].strip())
        nbody = (6 + 6*ne + 5)//6
        rows = lines[j+1:j+1+nbody]
        vals = []
        for r in rows: vals += body6(r)
        fixed, pts = vals[:6], vals[6:6+6*ne]
        P = [pts[k*6:(k+1)*6] for k in range(ne)]
        # lin-lin resample: insert nsub-1 midpoints per interval
        Q = []
        for k in range(len(P)-1):
            a, b = P[k], P[k+1]
            Q.append(a)
            for s in range(1, nsub):
                t = s / nsub
                Q.append([a[c] + t*(b[c]-a[c]) for c in range(6)])
        Q.append(P[-1])
        ne2 = len(Q); npl2 = 6 + 6*ne2
        # NPL is N1[44:55], NE is N2[55:66]
        newhead = head[:44] + f"{npl2:>11d}" + f"{ne2:>11d}" + head[66:]
        out.append(newhead)
        flat = fixed + [v for q in Q for v in q]
        for k in range(0, len(flat), 6):
            chunk = flat[k:k+6]
            line = "".join(endf_float(v) for v in chunk).ljust(66)
            out.append(line + head[66:])
        j += 1 + nbody
out += lines[j:]

# The MF=1/MT=451 dictionary declares the record count (NC) of every section.
# Growing MF=2/151 without updating it makes NJOY read past the section --
# observed as a malloc heap-corruption abort, not a clean error.
delta = len(out) - len(lines)
for k, l in enumerate(out):
    if len(l) < 75 or l[70:72].strip() != "1" or l[72:75].strip() != "451":
        continue
    try:
        mf = int(l[22:33]); mt = int(l[33:44]); nc = int(l[44:55]); mod = int(l[55:66])
    except ValueError:
        continue
    if mf == 2 and mt == 151:
        out[k] = l[:44] + f"{nc + delta:>11d}" + f"{mod:>11d}" + l[66:]
        print(f"dictionary MF=2/151: NC {nc} -> {nc + delta}")
        break

open(dst, "w").write("\n".join(out))
print(f"wrote {dst}: {len(lines)} -> {len(out)} lines, nsub={nsub}")
