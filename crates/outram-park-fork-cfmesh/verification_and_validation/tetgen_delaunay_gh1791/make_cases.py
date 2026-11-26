#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# gh:#1791: write the stage-2 input point sets as TetGen .node files
# (0-based, %.17g, so strtod reads back the exact doubles).
# Usage: python3 -I make_cases.py <out_dir>
import math, random, sys, os

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

def write(name, pts):
    with open(os.path.join(out, name + ".node"), "w") as f:
        f.write("%d  3  0  0\n" % len(pts))
        for i, p in enumerate(pts):
            f.write("%d  %.17g  %.17g  %.17g\n" % (i, p[0], p[1], p[2]))

def rand_cloud(n, seed):
    r = random.Random(seed)
    return [(r.random(), r.random(), r.random()) for _ in range(n)]

for n in (10, 100, 1000, 20000):
    write("random_%d" % n, rand_cloud(n, 1791 + n))

# Regular 5x5x5 lattice: degenerate (coplanar, cospherical) everywhere.
write("lattice_5x5x5", [(float(i), float(j), float(k))
                        for i in range(5) for j in range(5) for k in range(5)])

# Exactly cospherical: every integer point with x^2 + y^2 + z^2 = 125.
sph = [(float(x), float(y), float(z)) for x in range(-12, 12) for y in range(-12, 12)
       for z in range(-12, 12) if x * x + y * y + z * z == 125]
random.Random(7).shuffle(sph)
write("sphere_exact", sph)

# Nearly cospherical: 400 random directions normalised onto the unit sphere.
r = random.Random(11)
sp = []
for _ in range(400):
    v = [r.gauss(0, 1) for _ in range(3)]
    n = math.sqrt(sum(c * c for c in v))
    sp.append(tuple(c / n for c in v))
write("sphere_unit", sp)

# Duplicates: 300 random points, then 40 exact copies of earlier ones,
# interleaved.
base = rand_cloud(300, 3)
r = random.Random(5)
dup = list(base)
for k in range(40):
    dup.insert(r.randrange(len(dup) + 1), base[r.randrange(300)])
write("duplicates", dup)

# Nearly flat: 500 points in the unit square, z in [0, 1e-4).
r = random.Random(13)
write("nearly_flat", [(r.random(), r.random(), 1e-4 * r.random()) for _ in range(500)])

# Large cavities (insert_vertex_bw's v_out >= 64 and >= 1024 branches): the
# centre of a sphere of 3000 nearly cospherical points; every tetrahedron's
# circumsphere holds it, so its cavity is the whole mesh at that time.
r = random.Random(17)
sc = []
for _ in range(3000):
    v = [r.gauss(0, 1) for _ in range(3)]
    n = math.sqrt(sum(c * c for c in v))
    sc.append(tuple(c / n for c in v))
sc.insert(2500, (0.0, 0.0, 0.0))
write("sphere_centre", sc)

# The same sphere with the centre last, run with `-b/1` (input order, no
# sort): the centre's cavity then has all 3000 sphere points on its
# boundary (the v_out >= 1024 branch).
sc.remove((0.0, 0.0, 0.0))
sc.append((0.0, 0.0, 0.0))
write("sphere_centre_last", sc)
