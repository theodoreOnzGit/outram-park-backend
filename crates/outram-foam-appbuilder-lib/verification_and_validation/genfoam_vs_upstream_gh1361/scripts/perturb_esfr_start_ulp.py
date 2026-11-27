#!/usr/bin/env python3
"""Round 78: replace internal cell CELL of <case>/<time>/fluidRegion/T by nextafter(value, +inf) (one ulp), printed with 17 digits."""
import math, re, sys
path, cell = sys.argv[1], int(sys.argv[2])
t = open(path).read()
m = re.search(r'internalField\s+nonuniform\s+List<scalar>\s*(\d+)\s*\(\n', t)
n = int(m.group(1))
start = m.end()
end = t.index('\n)', start)
vals = t[start:end].split('\n')
assert len(vals) == n, (len(vals), n)
old = float(vals[cell])
new = math.nextafter(old, math.inf)
assert new != old
vals[cell] = repr(new)
print(f'cell {cell}: {old!r} -> {new!r} (delta {new-old:.3e}, rel {(new-old)/old:.3e})')
open(path, 'w').write(t[:start] + '\n'.join(vals) + t[end:])
