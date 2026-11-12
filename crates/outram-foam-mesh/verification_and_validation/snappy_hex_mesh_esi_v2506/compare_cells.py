#!/usr/bin/env python3
"""gh:#1465/#1466: compare an ESI v2506 polyMesh (ASCII) with the port's
`dump_castellation` text output, cell by cell.

usage: python3 -I compare_cells.py ESI_POLYMESH_DIR [CELLLEVEL_FILE] PORT_CELLS.txt

The ESI cell centre is computed the way OpenFOAM does (primitiveMeshCellCentresAndVols:
estimated centre, pyramid decomposition). A cell is matched to the port cell
whose centre is within 1e-9 of the cell edge length. Prints counts, per-level
histograms, the symmetric difference and boundary patch face counts.
Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import sys
from collections import Counter


def body(path):
    t = open(path).read()
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    t = re.sub(r"//[^\n]*", "", t)
    return t


def read_list(path, kind):
    t = body(path)
    m = re.search(r"\n(\d+)\s*\n\(", t)
    n = int(m.group(1))
    s = t[m.end():]
    s = s[: s.rindex(")")]
    if kind == "points":
        v = re.findall(r"\(([^)]*)\)", s)
        return [tuple(float(x) for x in a.split()) for a in v][:n]
    if kind == "faces":
        v = re.findall(r"(\d+)\(([^)]*)\)", s)
        return [[int(x) for x in a.split()] for _, a in v][:n]
    return [int(x) for x in s.split()][:n]


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def face_geom(pts, f):
    n = len(f)
    if n == 3:
        c = tuple(sum(pts[i][k] for i in f) / 3 for k in range(3))
        a = cross(sub(pts[f[1]], pts[f[0]]), sub(pts[f[2]], pts[f[0]]))
        return c, tuple(0.5 * x for x in a)
    est = tuple(sum(pts[i][k] for i in f) / n for k in range(3))
    sumA = (0.0, 0.0, 0.0)
    sumAc = (0.0, 0.0, 0.0)
    sumN = 0.0
    parts = []
    for i in range(n):
        p, q = pts[f[i]], pts[f[(i + 1) % n]]
        c3 = tuple((p[k] + q[k] + est[k]) / 3 for k in range(3))
        a = cross(sub(q, p), sub(est, p))
        a = tuple(0.5 * x for x in a)
        parts.append((a, c3))
        sumN += dot(a, a) ** 0.5
        sumA = tuple(sumA[k] + a[k] for k in range(3))
    mag = dot(sumA, sumA) ** 0.5
    nrm = tuple(x / mag for x in sumA) if mag > 0 else (0, 0, 0)
    sa = 0.0
    sc = [0.0, 0.0, 0.0]
    for a, c3 in parts:
        w = dot(a, nrm)
        sa += w
        for k in range(3):
            sc[k] += w * c3[k]
    if sa > 1e-300:
        return tuple(x / sa for x in sc), sumA
    return est, sumA


def main():
    mesh = sys.argv[1]
    if len(sys.argv) == 4:
        lvl_file, port_file = sys.argv[2], sys.argv[3]
    else:
        lvl_file, port_file = None, sys.argv[2]
    pts = read_list(mesh + "/points", "points")
    faces = read_list(mesh + "/faces", "faces")
    owner = read_list(mesh + "/owner", "ints")
    nei = read_list(mesh + "/neighbour", "ints")
    ncell = max(max(owner), max(nei)) + 1
    fg = [face_geom(pts, f) for f in faces]
    est = [[0.0, 0.0, 0.0] for _ in range(ncell)]
    cnt = [0] * ncell
    for i, (c, _) in enumerate(fg):
        for cell in ([owner[i], nei[i]] if i < len(nei) else [owner[i]]):
            cnt[cell] += 1
            for k in range(3):
                est[cell][k] += c[k]
    est = [[x / cnt[i] for x in e] for i, e in enumerate(est)]
    cv = [0.0] * ncell
    cc = [[0.0, 0.0, 0.0] for _ in range(ncell)]
    for i, (c, a) in enumerate(fg):
        for cell, sgn in ([(owner[i], 1), (nei[i], -1)] if i < len(nei) else [(owner[i], 1)]):
            d = sub(c, est[cell])
            pv = sgn * dot(a, d) / 3.0
            cv[cell] += pv
            for k in range(3):
                cc[cell][k] += pv * (0.75 * c[k] + 0.25 * est[cell][k])
    cc = [tuple(cc[i][k] / cv[i] for k in range(3)) for i in range(ncell)]
    if lvl_file:
        level = read_list(lvl_file, "ints")
    else:
        v0 = max(cv)
        level = [round(__import__("math").log2(v0 / v) / 3.0) for v in cv]
    port = []
    for line in open(port_file):
        p = line.split()
        port.append((int(p[0]), (float(p[1]), float(p[2]), float(p[3]))))
    edge = {l: (cv[level.index(l)]) ** (1 / 3) for l in set(level)}
    key = lambda c: tuple(round(x / 1e-9) for x in c)
    pm = {}
    for l, c in port:
        pm[key(c)] = l
    esi = Counter(level)
    prt = Counter(l for l, _ in port)
    print("ESI cells", ncell, "by level", sorted(esi.items()))
    print("port cells", len(port), "by level", sorted(prt.items()))
    only_esi, only_port, lvl_diff = [], [], []
    seen = set()
    for i in range(ncell):
        k = key(cc[i])
        if k in pm:
            seen.add(k)
            if pm[k] != level[i]:
                lvl_diff.append((cc[i], level[i], pm[k]))
        else:
            only_esi.append((cc[i], level[i]))
    for l, c in port:
        if key(c) not in seen:
            only_port.append((c, l))
    print("matched", len(seen), "level differs among matched", len(lvl_diff))
    print("only in ESI", len(only_esi), Counter(l for _, l in only_esi))
    print("only in port", len(only_port), Counter(l for _, l in only_port))
    with open(sys.argv[-1] + ".esi", "w") as f:
        for i in range(ncell):
            f.write("%d %.12e %.12e %.12e\n" % ((level[i],) + tuple(cc[i])))
    out = sys.argv[-1] + ".diff"
    with open(out, "w") as f:
        for c, l in only_esi:
            f.write("esi %d %.9e %.9e %.9e\n" % ((l,) + tuple(c)))
        for c, l in only_port:
            f.write("port %d %.9e %.9e %.9e\n" % ((l,) + tuple(c)))
        for c, a, b in lvl_diff:
            f.write("lvl esi=%d port=%d %.9e %.9e %.9e\n" % ((a, b) + tuple(c)))
    print("diff list written to", out)
    bt = body(mesh + "/boundary")
    for m in re.finditer(r"(\S+)\s*\{[^}]*?nFaces\s+(\d+);", bt):
        print("ESI patch", m.group(1), m.group(2))
    print("ESI faces", len(faces), "internal", len(nei))


main()
