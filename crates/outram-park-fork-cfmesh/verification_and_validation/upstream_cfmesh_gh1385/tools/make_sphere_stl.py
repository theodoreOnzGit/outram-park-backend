# SPDX-License-Identifier: AGPL-3.0-only
"""Write the gh:#1279 / gh:#1299 "setup S" sphere as an ASCII STL.

Same construction as `outram_park_fork_cfmesh::shapes::sphere_surface`
(north pole, n_lat-1 rings of n_lon points, south pole; cap fans and
two triangles per band quad, outward winding), centre (0,0,0), R = 3,
n_lat = 24, n_lon = 48 -> 2 + 23*48 = 1106 points, 2*48 + 2*22*48 = 2208
triangles.

Coordinates are rounded to float32 and printed with 9 significant digits,
which round-trips a float32 exactly: OpenFOAM's STL reader stores float,
so the upstream run and any Rust reader of this file see the same values.

Run:  python3 -I make_sphere_stl.py > sphere_r3_24x48.stl
"""
import math
import struct
import sys


def f32(v):
    return struct.unpack("f", struct.pack("f", v))[0]


def sphere(radius=3.0, n_lat=24, n_lon=48):
    pts = [(0.0, 0.0, radius)]
    for i in range(1, n_lat):
        th = math.pi * i / n_lat
        st, ct = math.sin(th), math.cos(th)
        for j in range(n_lon):
            ph = 2.0 * math.pi * j / n_lon
            pts.append((radius * st * math.cos(ph), radius * st * math.sin(ph), radius * ct))
    pts.append((0.0, 0.0, -radius))
    south = len(pts) - 1

    def ring(i, j):
        return 1 + (i - 1) * n_lon + (j % n_lon)

    tris = []
    for j in range(n_lon):
        tris.append((0, ring(1, j), ring(1, j + 1)))
    for i in range(1, n_lat - 1):
        for j in range(n_lon):
            a, b, c, d = ring(i, j), ring(i, j + 1), ring(i + 1, j + 1), ring(i + 1, j)
            tris.append((a, b, c))
            tris.append((a, c, d))
    for j in range(n_lon):
        tris.append((south, ring(n_lat - 1, j + 1), ring(n_lat - 1, j)))
    return [tuple(f32(c) for c in p) for p in pts], tris


def main():
    pts, tris = sphere()
    w = sys.stdout.write
    w("solid sphere\n")
    for t in tris:
        a, b, c = (pts[k] for k in t)
        u = [b[i] - a[i] for i in range(3)]
        v = [c[i] - a[i] for i in range(3)]
        n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        m = math.sqrt(sum(x * x for x in n)) or 1.0
        w("  facet normal %.9g %.9g %.9g\n    outer loop\n" % tuple(x / m for x in n))
        for p in (a, b, c):
            w("      vertex %.9g %.9g %.9g\n" % p)
        w("    endloop\n  endfacet\n")
    w("endsolid sphere\n")


if __name__ == "__main__":
    main()
