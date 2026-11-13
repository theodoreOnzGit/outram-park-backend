#!/usr/bin/env python3
"""gh:#1459/#1466 round 3: extract the small committed references from an ESI
v2506 flange run (`snappyHexMeshDict.reduced`, `writePrecision 16`, debug
meshes written).

usage: python3 -I extract_esi_fixtures.py RUN_DIR OUT_DIR

Writes (plain text; the caller gzips with `gzip -n`):
  esi_baffle_cell_order.txt     cells of the mesh `baffleAndSplitMesh` runs on
                                (time 16), IN ESI CELL ORDER, one `level i j k`
                                per cell on the cell's own level lattice
  esi_problem_faces.txt         the 25295 faces of `16/polyMesh/sets/problemFaces`
                                as `levelA i j k levelB i j k` (the two cells
                                the face separates, smaller key first)
  esi_final_cells_ordered.txt   the final mesh (time 32) cells in ESI order
  esi_final_points.txt          its points as integers on the level-2 lattice
  esi_final_faces.txt           its faces in ESI order: `owner neighbour p0 p1 ..`
                                (neighbour -1-k for patch k of `boundary`)
  esi_final_boundary.txt        `name startFace nFaces` per patch

Cell key of a cell: level from `cellLevel`, centre from the bounding box of its
points (exact for the axis-aligned hexes with hanging points). Standard library
only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import sys

ORG = [-0.03, -0.03, -0.03]
SIZE0 = [0.06 / 20, 0.06 / 16, 0.04 / 12]
LATTICE_LEVEL = 2


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
        return [tuple(float(x) for x in a.split()) for a in re.findall(r"\(([^)]*)\)", s)][:n]
    if kind == "faces":
        return [[int(x) for x in a.split()] for _, a in re.findall(r"(\d+)\(([^)]*)\)", s)][:n]
    return [int(x) for x in s.split()][:n]


def read_boundary(path):
    out = []
    for m in re.finditer(r"(\w+)\s*\{([^}]*)\}", body(path)):
        d = m.group(2)
        if "nFaces" not in d:
            continue
        nf = int(re.search(r"nFaces\s+(\d+)", d).group(1))
        sf = int(re.search(r"startFace\s+(\d+)", d).group(1))
        out.append((m.group(1), sf, nf))
    return out


def load(d):
    return (
        read_list(d + "/points", "points"),
        read_list(d + "/faces", "faces"),
        read_list(d + "/owner", "label"),
        read_list(d + "/neighbour", "label"),
        read_boundary(d + "/boundary"),
        read_list(d + "/cellLevel", "label"),
    )


def cell_keys(P, F, O, N, cl):
    nc = max(max(O), max(N)) + 1
    lo = [[1e9] * 3 for _ in range(nc)]
    hi = [[-1e9] * 3 for _ in range(nc)]

    def upd(c, f):
        for p in F[f]:
            x = P[p]
            for a in range(3):
                lo[c][a] = min(lo[c][a], x[a])
                hi[c][a] = max(hi[c][a], x[a])

    for f in range(len(F)):
        upd(O[f], f)
        if f < len(N):
            upd(N[f], f)
    keys = []
    for c in range(nc):
        k = []
        for a in range(3):
            cen = 0.5 * (lo[c][a] + hi[c][a])
            h = SIZE0[a] / (1 << cl[c])
            k.append(round((cen - ORG[a]) / h - 0.5))
        keys.append((cl[c], k[0], k[1], k[2]))
    return keys


def main():
    run, out = sys.argv[1], sys.argv[2]
    P, F, O, N, B, cl = load(run + "/16/polyMesh")
    keys = cell_keys(P, F, O, N, cl)
    open(out + "/esi_baffle_cell_order.txt", "w").write("".join("%d %d %d %d\n" % k for k in keys))
    prob = read_list(run + "/16/polyMesh/sets/problemFaces", "label")
    lines = []
    for f in prob:
        assert f < len(N), "problem face must be internal"
        a, b = sorted([keys[O[f]], keys[N[f]]])
        lines.append("%d %d %d %d %d %d %d %d\n" % (a + b))
    open(out + "/esi_problem_faces.txt", "w").write("".join(lines))

    P, F, O, N, B, cl = load(run + "/32/polyMesh")
    keys = cell_keys(P, F, O, N, cl)
    assert max(cl) <= LATTICE_LEVEL
    open(out + "/esi_final_cells_ordered.txt", "w").write("".join("%d %d %d %d\n" % k for k in keys))
    lat = []
    for x in P:
        v = []
        for a in range(3):
            q = (x[a] - ORG[a]) / (SIZE0[a] / (1 << LATTICE_LEVEL))
            assert abs(q - round(q)) < 1e-6, "point off the lattice"
            v.append(round(q))
        lat.append(v)
    open(out + "/esi_final_points.txt", "w").write("".join("%d %d %d\n" % tuple(v) for v in lat))
    patch_of = {}
    for k, (name, sf, nf) in enumerate(B):
        for f in range(sf, sf + nf):
            patch_of[f] = k
    nint = len(N)
    lines = []
    for f in range(len(F)):
        nei = N[f] if f < nint else -1 - patch_of[f]
        lines.append(" ".join(str(x) for x in [O[f], nei] + F[f]) + "\n")
    open(out + "/esi_final_faces.txt", "w").write("".join(lines))
    open(out + "/esi_final_boundary.txt", "w").write("".join("%s %d %d\n" % b for b in B))
    print("cells", len(keys), "points", len(P), "faces", len(F), "internal", nint, B)


main()
