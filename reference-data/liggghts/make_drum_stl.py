#!/usr/bin/env python3
"""Writes drum_cylinder.stl: closed cylinder, axis y, R=0.05, y in [-0.05,0.05],
16 segments, side quads split in two facets, end caps as centre fans."""
import math
N = 16
R = 0.05
H = 0.05


def p(k, y):
    a = 2 * math.pi * k / N
    return (round(R * math.cos(a), 12), y, round(R * math.sin(a), 12))


tris = []
for k in range(N):
    a0, a1 = p(k, -H), p(k + 1, -H)
    b0, b1 = p(k, H), p(k + 1, H)
    tris.append((a0, b0, a1))
    tris.append((a1, b0, b1))
    tris.append(((0, -H, 0), a1, a0))
    tris.append(((0, H, 0), b0, b1))
with open("drum_cylinder.stl", "w") as f:
    f.write("solid drum\n")
    for t in tris:
        f.write("  facet normal 0 0 0\n    outer loop\n")
        for v in t:
            f.write("      vertex %.12g %.12g %.12g\n" % v)
        f.write("    endloop\n  endfacet\n")
    f.write("endsolid drum\n")
