#!/usr/bin/env python3
"""gh:#1467 round 6: extract the exact-double feature-snap references from the ESI
v2506 flange run of `snappyHexMeshDict.feature` (binary write, `blockMesh` run
first with the ASCII `controlDict`, `writePrecision 16`).

usage: python3 -I extract_feature_fixtures.py RUN_DIR OUT_DIR TIME[:kind] ...

kind `points` (default): `TIME/polyMesh/points`; kind `disp`: `TIME/displacement`
(`pointVectorField`, internalField). Writes little-endian float64 triples (no
header), `feat_points_TIME.bin` / `feat_disp_TIME.bin`, to be gzipped with
`gzip -n`. Morph iteration k writes its smoothed mesh and displacement at times
34, 36, 38, 40, 42, 44, 46, 54, 62, 70 and its last scaled mesh at 35, 37, 39, 41,
43, 45, 53, 61, 69, 77 (see README, round 6).

Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import sys


def vec_payload(path):
    data = open(path, "rb").read()
    m = re.search(rb"\n(\d+)\n\(", data)
    n = int(m.group(1))
    start = m.end()
    raw = data[start:start + 24 * n]
    assert len(raw) == 24 * n
    assert data[start + 24 * n:start + 24 * n + 1] == b")"
    return n, raw


def main():
    run, out = sys.argv[1], sys.argv[2]
    for spec in sys.argv[3:]:
        t, _, kind = spec.partition(":")
        kind = kind or "points"
        if kind == "points":
            src, dst = f"{run}/{t}/polyMesh/points", f"{out}/feat_points_{t}.bin"
        else:
            src, dst = f"{run}/{t}/displacement", f"{out}/feat_disp_{t}.bin"
        n, raw = vec_payload(src)
        open(dst, "wb").write(raw)
        print(dst, n)


if __name__ == "__main__":
    main()
