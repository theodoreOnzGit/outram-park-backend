#!/usr/bin/env python3
"""gh:#1467 round 8: extract exact-double mesh references from an ESI v2506 flange run
with `addLayers` (`snappyHexMeshDict.layers`, binary write, `blockMesh` run first with the
ASCII controlDict at `writePrecision 16`).

usage: python3 -I extract_layer_fixtures.py RUN_DIR OUT_DIR TIME[:kind] ...

kind `mesh` (default) writes, for the polyMesh of TIME,
  `layer_mesh_TIME.txt`   one line per face: `owner neighbour p0 p1 ..` (neighbour = -1-k for
                          the k-th boundary patch of `boundary`), faces in ESI order
  `layer_points_TIME.bin` little-endian float64 triples, no header
  `layer_boundary_TIME.txt` `name startFace nFaces`
kind `points` writes only `layer_points_TIME.bin`.
The caller gzips with `gzip -n`. Standard library only. Tooling for V&V, reached by the
recorded run, not a test.
"""
import re
import struct
import sys


def lists(path):
    """All `\\n<n>\\n(` ... `)` binary blocks of a polyMesh file (label size 4)."""
    data = open(path, "rb").read()
    cls = re.search(rb"class\s+(\w+);", data).group(1).decode()
    out = []
    pos = data.index(b"}") + 1
    while True:
        m = re.compile(rb"\n(\d+)\n\(").search(data, pos)
        if not m:
            break
        n = int(m.group(1))
        start = m.end()
        out.append((n, start, data))
        # element size is decided by the caller; skip using the class
        if cls == "vectorField":
            size = 24
        else:
            size = 4
        pos = start + size * n + 1
    return cls, out


def int_list(path, which=0):
    cls, blocks = lists(path)
    n, start, data = blocks[which]
    return list(struct.unpack("<%di" % n, data[start:start + 4 * n]))


def points(path):
    cls, blocks = lists(path)
    n, start, data = blocks[0]
    raw = data[start:start + 24 * n]
    assert len(raw) == 24 * n
    return n, raw


def boundary(path):
    t = open(path).read()
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    out = []
    for m in re.finditer(r"(\w+)\s*\{([^}]*)\}", t):
        d = m.group(2)
        if "nFaces" not in d:
            continue
        out.append((m.group(1), int(re.search(r"startFace\s+(\d+)", d).group(1)),
                    int(re.search(r"nFaces\s+(\d+)", d).group(1))))
    return out


def main():
    run, out = sys.argv[1], sys.argv[2]
    for spec in sys.argv[3:]:
        t, _, kind = spec.partition(":")
        kind = kind or "mesh"
        pm = f"{run}/{t}/polyMesh"
        n, raw = points(f"{pm}/points")
        open(f"{out}/layer_points_{t}.bin", "wb").write(raw)
        print("points", t, n)
        if kind != "mesh":
            continue
        offs = int_list(f"{pm}/faces", 0)
        vals = int_list(f"{pm}/faces", 1)
        owner = int_list(f"{pm}/owner")
        nbr = int_list(f"{pm}/neighbour")
        bnd = boundary(f"{pm}/boundary")
        nf = len(offs) - 1
        assert len(owner) == nf
        patch_of = {}
        for k, (name, s, c) in enumerate(bnd):
            for f in range(s, s + c):
                patch_of[f] = k
        with open(f"{out}/layer_mesh_{t}.txt", "w") as fh:
            for f in range(nf):
                nb = nbr[f] if f < len(nbr) else -1 - patch_of[f]
                ring = " ".join(str(v) for v in vals[offs[f]:offs[f + 1]])
                fh.write(f"{owner[f]} {nb} {ring}\n")
        with open(f"{out}/layer_boundary_{t}.txt", "w") as fh:
            for name, s, c in bnd:
                fh.write(f"{name} {s} {c}\n")
        print("faces", t, nf)


if __name__ == "__main__":
    main()
